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
//! Redirects, proxies, keep-alive, compression, cookies. A caller that needs a
//! cookie carries it in a header, which is what [`crate::Call::headers`] is
//! for and what ClipCascade's session does.

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
    let address = addresses
        .first()
        .with_context(|| format!("`{host}` resolved to nothing"))?;
    let socket = TcpStream::connect_timeout(address, patience)
        .with_context(|| format!("could not reach {host}:{port}"))?;
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
    Read::take(&mut stream, MOST + 1)
        .read_to_end(&mut raw)
        .with_context(|| format!("could not read {host}'s answer"))?;
    if raw.len() as u64 > MOST {
        bail!("{host} answered more than {MOST} bytes; it is not the API this expects");
    }
    crate::answer_of(&raw)
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
    fn the_bundle_path_is_the_one_every_other_program_reads() {
        // If this ever moves, the symptom is a machine where the browser works
        // and this does not — so the path is stated once and asserted.
        assert_eq!(ROOTS, "/etc/ssl/certs/ca-certificates.crt");
    }
}
