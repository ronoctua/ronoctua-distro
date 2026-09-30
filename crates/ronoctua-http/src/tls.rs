//! TLS, over `std::net` — the transport for the hosts that are not this machine.
//!
//! # Why it moved here
//!
//! It was `ronoctua-world`'s, privately, and its own header said why it had to
//! exist: *"`aria2` talks to `127.0.0.1`, and this talks to somebody else's
//! machine, where there is no version of 'no TLS' that is acceptable."* That
//! argument was right and it was never about the world clock. ClipCascade's
//! server is also somebody else's machine, and writing a second rustls setup
//! beside the first is exactly the shape of defect this crate was carved out
//! to prevent — `one-http-writer`, one layer down.
//!
//! So the world clock keeps its policy (ten seconds, a megabyte, two hosts)
//! and loses its transport. What lives here is the part that would have been
//! typed twice: the roots, the provider, the session, and the connect with a
//! deadline on it.
//!
//! # The roots come from the SYSTEM
//!
//! Not `webpki-roots`. A root bundled into the binary ages inside it, and the
//! day one expires the symptom is every fetch failing on a machine whose
//! browser is fine. NixOS declares `security.pki` and puts the bundle where
//! every other program on the machine looks for it, so this looks there too —
//! and when it is not there, this says so once and connects to nothing, which
//! is a feature not working rather than a certificate being ignored.
//!
//! # What it does NOT do
//!
//! Redirects, proxies, keep-alive, compression, cookies. Redirects and gzip
//! are one layer up, in [`crate::web`], which walks a chain of these calls. A
//! caller that needs a cookie carries it in a header, which is what
//! [`crate::Call::headers`] is for and what ClipCascade's session does.

use std::io::Read;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};

/// How long a host on the internet is given.
///
/// Ten seconds, against the two a socket in `/run` gets: this crosses a
/// network somebody else runs, and the thread it blocks is not a drawing one.
/// It is a ceiling and not a budget — the call that takes it has failed.
pub const PATIENCE: Duration = Duration::from_secs(10);

/// The most a one-shot body may be, before it is refused.
///
/// A megabyte. Every answer this tree reads over TLS is a few hundred bytes to
/// a few kilobytes, so this is three orders of magnitude of room and still a
/// bound: without one, a host that answered for ever would fill the process's
/// memory, and the process is the footer.
///
/// It does NOT apply to [`connect`], which is a duplex stream the caller
/// frames itself — a WebSocket that stayed open for a week would trip any
/// ceiling, and its bound is per MESSAGE, in [`crate::ws`].
pub const MOST: u64 = 1 << 20;

/// Where the system keeps the certificate authorities it trusts.
///
/// The NixOS path, which is also the Debian and Arch path — `security.pki`
/// assembles the bundle and every program on the machine reads this file.
pub const ROOTS: &str = "/etc/ssl/certs/ca-certificates.crt";

/// A TLS connection, open, with both halves.
///
/// `StreamOwned` and not `rustls::Stream`: a borrowed stream cannot outlive
/// the function that made it, and a WebSocket has to live on a struct for as
/// long as the view is open.
pub type Stream = rustls::StreamOwned<rustls::ClientConnection, TcpStream>;

/// The trusted roots, read once.
///
/// # Errors
///
/// The bundle is not there, or holds no certificate. Both mean the same thing
/// in practice and are worth different sentences: a machine with no bundle is
/// misconfigured, and a bundle with nothing in it is corrupt.
fn roots() -> Result<rustls::RootCertStore> {
    let file = std::fs::File::open(ROOTS)
        .with_context(|| format!("no certificate bundle at {ROOTS}; nothing can be verified"))?;
    let mut reader = std::io::BufReader::new(file);
    let mut store = rustls::RootCertStore::empty();
    let mut held = 0usize;
    for certificate in rustls_pemfile::certs(&mut reader) {
        let certificate = certificate.context("a certificate in the bundle would not parse")?;
        // ONE BAD CERTIFICATE IS NOT A BAD BUNDLE. The store refuses what it
        // cannot use and the rest still verify; failing the whole read would
        // turn one expired root into no network at all.
        if store.add(certificate).is_ok() {
            held += 1;
        }
    }
    if held == 0 {
        bail!("{ROOTS} holds no usable certificate");
    }
    tracing::debug!(certificates = held, "the system's roots are loaded");
    Ok(store)
}

/// A configuration, built once and shared by every connection.
///
/// Building it reads and parses the whole bundle — about a hundred and fifty
/// certificates — and doing that per request would be the cost of the feature.
fn shared() -> Result<Arc<rustls::ClientConfig>> {
    static ONCE: std::sync::OnceLock<Option<Arc<rustls::ClientConfig>>> =
        std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        // THE PROVIDER IS INSTALLED HERE, and exactly once. rustls 0.23 has no
        // default when more than one is compiled in, and the error it gives
        // then names neither the crate nor the call — it says a process-wide
        // default was not set, from inside a builder.
        let _ = rustls::crypto::ring::default_provider().install_default();
        match roots() {
            Ok(store) => Some(Arc::new(
                rustls::ClientConfig::builder()
                    .with_root_certificates(store)
                    .with_no_client_auth(),
            )),
            Err(e) => {
                // ONCE, and loudly. It is not per-call: a machine with no
                // bundle would otherwise write this line every half hour for
                // ever.
                tracing::warn!(error = %e, "no TLS is possible on this machine");
                None
            }
        }
    })
    .clone()
    .context("the system's certificate bundle could not be read")
}

/// An open TLS connection to `host:port`.
///
/// The caller owns the framing from here: [`over_tls`] uses it for one call
/// and closes, [`crate::ws`] uses it for as long as the socket lives.
///
/// # Errors
///
/// The name is not a server name, it does not resolve, the host did not answer
/// inside `patience`, or the certificate did not verify.
pub fn connect(host: &str, port: u16, patience: Duration) -> Result<Stream> {
    let config = shared()?;
    // THE NAME IS CHECKED BEFORE A SOCKET IS OPENED. `ServerName` refuses a
    // URL, a bare scheme and an empty string, and refusing here means a
    // mistyped address never reaches the network at all.
    let name = rustls_pki_types::ServerName::try_from(host.to_owned())
        .with_context(|| format!("`{host}` is not a server name"))?;
    let session = rustls::ClientConnection::new(config, name)
        .context("the TLS session could not be started")?;

    // THE DEADLINE IS ON THE CONNECT TOO, and it has to be: name resolution
    // and a SYN to a host that drops packets are both silent waits, and
    // `TcpStream::connect` has no timeout of its own.
    let addresses: Vec<std::net::SocketAddr> =
        std::net::ToSocketAddrs::to_socket_addrs(&(host, port))
            .with_context(|| format!("`{host}` did not resolve"))?
            .collect();
    // EVERY ADDRESS, IN TURN. A host that lists IPv6 first on a network with
    // no IPv6 route answers the first attempt with "unreachable" and the
    // second with the page; taking only the first made the whole host look
    // down.
    if addresses.is_empty() {
        bail!("`{host}` resolved to nothing");
    }
    let mut last = None;
    let mut reached = None;
    for address in &addresses {
        match TcpStream::connect_timeout(address, patience) {
            Ok(s) => {
                reached = Some(s);
                break;
            }
            Err(e) => last = Some(e),
        }
    }
    let socket = match reached {
        Some(s) => s,
        None => {
            return Err(last.map_or_else(
                || anyhow::anyhow!("could not reach {host}:{port}"),
                |e| anyhow::Error::new(e).context(format!("could not reach {host}:{port}")),
            ));
        }
    };
    socket.set_read_timeout(Some(patience))?;
    socket.set_write_timeout(Some(patience))?;
    // NAGLE OFF. Every frame this writes is a whole message the other side is
    // waiting on; holding it for forty milliseconds to coalesce with a packet
    // that is not coming is latency for nothing.
    socket.set_nodelay(true).ok();
    Ok(rustls::StreamOwned::new(session, socket))
}

/// One call over TLS, and the answer.
///
/// The body is bounded by [`MOST`]: `Connection: close` means read to the end,
/// and "the end" is whatever the other side decides.
///
/// # Errors
///
/// The connection failed (see [`connect`]), the host answered more than
/// [`MOST`] bytes, or what came back is not an HTTP response.
pub fn over_tls(host: &str, port: u16, call: &crate::Call<'_>) -> Result<crate::Answer> {
    let mut stream = connect(host, port, call.patience)?;
    std::io::Write::write_all(&mut stream, crate::request_of(call).as_bytes())
        .with_context(|| format!("could not send the request to {host}"))?;
    std::io::Write::flush(&mut stream).ok();

    let mut raw = Vec::new();
    // BOUNDED, and by bytes rather than by trust.
    ended(
        Read::take(&mut stream, MOST + 1).read_to_end(&mut raw),
        &raw,
    )
    .with_context(|| format!("could not read {host}'s answer"))?;
    if raw.len() as u64 > MOST {
        bail!("{host} answered more than {MOST} bytes; it is not the API this expects");
    }
    crate::answer_of(&raw)
}

/// The most a caller may ask [`fetch`] to read, whatever it asks for.
///
/// Above this a body is a PACKAGE — an AppImage, an APK, an index — and goes
/// through the queue's downloader, which resumes, shows progress and can be
/// cancelled; holding it in memory here would be none of those.
pub const MOST_EVER: u64 = 32 << 20;

/// One call over TLS, and the whole reply as BYTES with its headers.
///
/// `most` is the caller's ceiling for this answer — a web manifest is small, a
/// catalogue is not — clamped to [`MOST_EVER`]. Redirects are NOT followed
/// here: one call is one hop, and [`crate::web::request`] walks the chain by
/// its rule (a downgrade to plain HTTP is the rule's refusal to make, not the
/// transport's).
///
/// # Errors
///
/// The connection failed (see [`connect`]), the host answered more than the
/// ceiling, or what came back is not an HTTP response.
pub fn fetch(host: &str, port: u16, call: &crate::Call<'_>, most: u64) -> Result<crate::Reply> {
    let most = most.min(MOST_EVER);
    let mut stream = connect(host, port, call.patience)?;
    std::io::Write::write_all(&mut stream, crate::request_of(call).as_bytes())
        .with_context(|| format!("could not send the request to {host}"))?;
    std::io::Write::flush(&mut stream).ok();
    let mut raw = Vec::new();
    // The head is not counted against the ceiling a caller thinks of as the
    // BODY's, so the read allows a little over it.
    let allowance = most + (64 << 10);
    ended(
        Read::take(&mut stream, allowance + 1).read_to_end(&mut raw),
        &raw,
    )
    .with_context(|| format!("could not read {host}'s answer"))?;
    if raw.len() as u64 > allowance {
        bail!("{host} answered more than {most} bytes");
    }
    let reply = crate::reply_of(&raw)?;
    if reply.body.len() as u64 > most {
        bail!("{host} answered more than {most} bytes");
    }
    whole(&reply, host)?;
    Ok(reply)
}

/// A read that ended, accepting the one "error" that is an ordinary end.
///
/// Many servers close a TLS connection without sending `close_notify`, and
/// rustls reports that as `UnexpectedEof` — after the whole answer arrived.
/// Treated as a failure, every such site read as "offline" (measured: WhatsApp
/// Web, google.com). What makes accepting it safe is [`whole`]: a body that
/// was really cut short is caught against its announced length.
fn ended(read: std::io::Result<usize>, raw: &[u8]) -> std::io::Result<()> {
    match read {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && !raw.is_empty() => Ok(()),
        Err(e) => Err(e),
    }
}

/// A body shorter than its `Content-Length` is a connection cut short, never
/// an answer.
fn whole(reply: &crate::Reply, host: &str) -> Result<()> {
    let chunked = reply
        .header("transfer-encoding")
        .is_some_and(|v| v.eq_ignore_ascii_case("chunked"));
    if chunked {
        return Ok(());
    }
    if let Some(length) = reply
        .header("content-length")
        .and_then(|v| v.trim().parse::<u64>().ok())
        && (reply.body.len() as u64) < length
    {
        bail!(
            "{host} announced {length} bytes and sent {}",
            reply.body.len()
        );
    }
    Ok(())
}

/// One GET over TLS whose body is WRITTEN AWAY as it arrives, never held.
///
/// For a package — an AppImage, an APK, an index — which is hundreds of
/// megabytes: [`fetch`] would hold all of it in memory, and a person watching
/// a bar needs it to move before the end. `heard` is told the head once
/// (`None` means the body is chunked or has no length) and then every chunk
/// as it is written; returning `false` stops the download, which is how a
/// cancel reaches the socket.
///
/// A redirect or any non-2xx answer is returned with an EMPTY body and nothing
/// written: following it is the caller's decision.
///
/// # Errors
///
/// The connection failed, the body passed `most`, the stop was asked for, or
/// what came back is not an HTTP response.
pub fn download(
    host: &str,
    port: u16,
    call: &crate::Call<'_>,
    most: u64,
    to: &mut dyn std::io::Write,
    heard: &mut dyn FnMut(Option<u64>, u64) -> bool,
) -> Result<crate::Reply> {
    let mut stream = connect(host, port, call.patience)?;
    std::io::Write::write_all(&mut stream, crate::request_of(call).as_bytes())
        .with_context(|| format!("could not send the request to {host}"))?;
    std::io::Write::flush(&mut stream).ok();

    // THE HEAD FIRST, byte by byte up to the blank line: it is small, and
    // reading past it would put body bytes where the head parser looks.
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > 64 << 10 {
            bail!("{host} sent a head longer than 64 KiB");
        }
        let n = stream
            .read(&mut byte)
            .with_context(|| format!("could not read {host}'s answer"))?;
        if n == 0 {
            bail!("{host} closed the connection before its head ended");
        }
        head.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    let status = crate::status_of(&text).ok_or_else(|| anyhow::anyhow!("no status line"))?;
    let headers = crate::headers_of(&text);
    let reply = crate::Reply {
        status,
        headers,
        body: Vec::new(),
    };
    if !reply.went_through() {
        return Ok(reply);
    }
    let chunked = reply
        .header("transfer-encoding")
        .is_some_and(|v| v.eq_ignore_ascii_case("chunked"));
    let length = if chunked {
        None
    } else {
        reply
            .header("content-length")
            .and_then(|v| v.trim().parse::<u64>().ok())
    };
    if length.is_some_and(|l| l > most) {
        bail!(
            "{host} announced {} bytes, more than {most}",
            length.unwrap_or(0)
        );
    }
    if !heard(length, 0) {
        bail!("stopped");
    }

    let mut written = 0u64;
    let mut buffer = vec![0u8; 64 << 10];
    if chunked {
        // CHUNKED: one hexadecimal length per line, the chunk, a CRLF; zero
        // ends it. Read through a small reader so a length line split across
        // two reads is still one line.
        let mut reader = std::io::BufReader::new(stream);
        loop {
            let mut line = String::new();
            std::io::BufRead::read_line(&mut reader, &mut line)
                .with_context(|| format!("could not read {host}'s answer"))?;
            let size = u64::from_str_radix(line.trim().split(';').next().unwrap_or(""), 16)
                .with_context(|| format!("{host} sent a chunk length that is not a number"))?;
            if size == 0 {
                break;
            }
            let mut left = size;
            while left > 0 {
                let want = usize::try_from(left.min(buffer.len() as u64)).unwrap_or(buffer.len());
                let n = reader
                    .read(&mut buffer[..want])
                    .with_context(|| format!("could not read {host}'s answer"))?;
                if n == 0 {
                    bail!("{host} closed the connection inside a chunk");
                }
                written += n as u64;
                if written > most {
                    bail!("{host} sent more than {most} bytes");
                }
                to.write_all(&buffer[..n])?;
                left -= n as u64;
                if !heard(None, written) {
                    bail!("stopped");
                }
            }
            let mut crlf = [0u8; 2];
            reader.read_exact(&mut crlf).ok();
        }
    } else {
        loop {
            // THE ANNOUNCED LENGTH IS THE END, when there is one: waiting for
            // the server to close would be waiting on a peer that may never
            // send `close_notify` (see `ended`).
            if length.is_some_and(|l| written >= l) {
                break;
            }
            let n = match stream.read(&mut buffer) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && length.is_none() => 0,
                Err(e) => {
                    return Err(
                        anyhow::Error::new(e).context(format!("could not read {host}'s answer"))
                    );
                }
            };
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > most {
                bail!("{host} sent more than {most} bytes");
            }
            to.write_all(&buffer[..n])?;
            if !heard(length, written) {
                bail!("stopped");
            }
        }
        if length.is_some_and(|l| l != written) {
            bail!(
                "{host} announced {} bytes and sent {written}",
                length.unwrap_or(0)
            );
        }
    }
    to.flush()?;
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing here reaches the network, and that is deliberate: a test that
    /// needs the internet is a test that fails in the sandbox this project
    /// builds in. What is worth holding is the SHAPE.
    #[test]
    fn the_patience_is_longer_than_a_local_sockets_and_bounded() {
        assert!(PATIENCE > crate::PATIENCE);
        assert!(PATIENCE <= Duration::from_secs(30));
    }

    /// The ceiling is far above any answer and still a ceiling.
    const _: () = {
        assert!(MOST >= 1 << 16, "too tight for a legitimate answer");
        assert!(MOST <= 1 << 24, "not a bound worth having");
    };

    #[test]
    fn a_name_that_is_not_a_name_never_reaches_the_network() {
        // `connect` checks this BEFORE resolving, so a mistyped address costs
        // a string comparison rather than a DNS timeout.
        for bad in ["", "not a host", "http://example.com", "host:443"] {
            assert!(
                rustls_pki_types::ServerName::try_from(bad.to_owned()).is_err(),
                "`{bad}` was accepted as a server name"
            );
        }
        for good in ["example.com", "127.0.0.1", "a.b.c.example.org"] {
            assert!(
                rustls_pki_types::ServerName::try_from(good.to_owned()).is_ok(),
                "`{good}` was refused as a server name"
            );
        }
    }

    #[test]
    fn a_close_without_notify_is_an_end_and_a_short_body_is_not() {
        let eof = || std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "peer closed");
        assert!(ended(Err(eof()), b"HTTP/1.1 200 OK\r\n\r\nx").is_ok());
        assert!(
            ended(Err(eof()), b"").is_err(),
            "nothing arrived: that is a failure"
        );
        let short = crate::reply_of(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nabc").unwrap();
        assert!(
            whole(&short, "h").is_err(),
            "a body cut short is never an answer"
        );
        let full = crate::reply_of(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc").unwrap();
        assert!(whole(&full, "h").is_ok());
    }

    #[test]
    fn the_bundle_path_is_the_one_every_other_program_reads() {
        // If this ever moves, the symptom is a machine where the browser works
        // and this does not — so the path is stated once and asserted.
        assert_eq!(ROOTS, "/etc/ssl/certs/ca-certificates.crt");
    }
}
