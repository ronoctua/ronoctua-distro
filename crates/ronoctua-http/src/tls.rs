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
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
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

    let socket = reach(host, port, patience, Fence::Open)?;
    Ok(rustls::StreamOwned::new(session, socket))
}

/// Where a connection may go, by the addresses its name resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fence {
    /// Wherever the name resolves: every TLS connection (a certificate is
    /// checked), and the host a person pasted themselves — a box on their own
    /// network may serve their list.
    Open,
    /// The public internet only ([`is_public`]): a host a list's server sent
    /// a walk IN THE CLEAR to. A server does not get to make this machine ask
    /// its own neighbours — a router, a printer, a service on `localhost` —
    /// with no certificate in the way.
    Public,
}

/// Whether `ip` is on the public internet: not this machine, not a network
/// of its own (the private blocks, a tailnet's shared block), not a link's,
/// not a multicast group, a broadcast or a reserved block. Pure.
#[must_use]
pub fn is_public(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_unspecified()
                || v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_multicast()
                || v4.is_documentation()
                // "THIS NETWORK", the shared block (a carrier's NAT, a
                // tailnet), the benchmarking block and the reserved ones.
                || a == 0
                || (a == 100 && (b & 0xc0) == 64)
                || (a == 198 && (b & 0xfe) == 18)
                || a >= 240)
        }
        std::net::IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(std::net::IpAddr::V4(v4));
            }
            !(v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local())
        }
    }
}

/// A TCP connection to `host:port`, every address the name resolves to tried
/// in turn within `patience` — only the public ones behind
/// [`Fence::Public`] — with the read and write deadlines set: what both
/// [`connect`] and [`fetch_plain_abortable`] stand on.
///
/// # Errors
///
/// The name does not resolve, every address it resolves to is fenced off,
/// or no address answered inside `patience`.
fn reach(host: &str, port: u16, patience: Duration, fence: Fence) -> Result<TcpStream> {
    // THE DEADLINE IS ON THE CONNECT TOO, and it has to be: name resolution
    // and a SYN to a host that drops packets are both silent waits, and
    // `TcpStream::connect` has no timeout of its own.
    let resolved: Vec<std::net::SocketAddr> =
        std::net::ToSocketAddrs::to_socket_addrs(&(host, port))
            .with_context(|| format!("`{host}` did not resolve"))?
            .collect();
    // THE FENCE, on what the name RESOLVED to — a name is no promise of
    // where it leads — and before anything is sent.
    let addresses: Vec<std::net::SocketAddr> = match fence {
        Fence::Open => resolved.clone(),
        Fence::Public => resolved
            .iter()
            .copied()
            .filter(|a| is_public(a.ip()))
            .collect(),
    };
    if addresses.is_empty() && !resolved.is_empty() {
        bail!(
            "`{host}` is this machine or a network of its own, and a request in the clear is \
             never sent there on another host's word"
        );
    }
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
    Ok(socket)
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

/// A way to stop a request from ANOTHER thread: its socket is shut.
///
/// # Why a socket and not a flag
///
/// A request blocks in a read for up to its patience, and a flag is looked at
/// only between reads — so a search that a person has already typed past
/// would go on holding its thread, and the host, for ten seconds. Shutting the
/// socket ends the blocked read AT ONCE (it returns the end of the stream, or
/// an error), and the request answers [`crate::web::Kind::Failed`].
///
/// The search app hands every keystroke's request an `Abort` and aborts the
/// previous one when the next key arrives.
///
/// # What it cannot shorten
///
/// Resolving the name and the TCP connect: neither has a socket to shut yet,
/// and both are bounded by the patience. The abort is looked at before the
/// connect and as soon as it returns, so an aborted request never sends.
///
/// # One-way
///
/// Once aborted it stays aborted, and every request that holds it (a clone is
/// the same handle) is stopped: the next request takes a new one.
#[derive(Debug, Clone, Default)]
pub struct Abort(Arc<Stops>);

#[derive(Debug, Default)]
struct Stops {
    stopped: AtomicBool,
    /// Every socket open under this handle, by a number of its own, so two
    /// requests sharing one each take theirs out when they end.
    open: Mutex<Vec<(u64, TcpStream)>>,
    next: AtomicU64,
}

impl Abort {
    /// A handle nothing has aborted.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops every request holding this handle: the ones in flight have
    /// their socket shut, and the ones not yet started never connect.
    pub fn abort(&self) {
        // THE FLAG FIRST, THEN THE LOCK: `hold` looks at the flag under the
        // lock, so a socket is either seen here or refused there — never
        // registered after the sweep and left open.
        self.0.stopped.store(true, Ordering::SeqCst);
        let open = std::mem::take(&mut *self.0.open.lock().unwrap_or_else(PoisonError::into_inner));
        for (_, socket) in open {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }

    /// Whether [`Abort::abort`] was called.
    #[must_use]
    pub fn is_aborted(&self) -> bool {
        self.0.stopped.load(Ordering::SeqCst)
    }

    /// Registers `socket` until the returned guard drops.
    ///
    /// # Errors
    ///
    /// `Interrupted` when the handle is already aborted (the socket is shut
    /// then and there), or the socket could not be cloned.
    pub(crate) fn hold(&self, socket: &TcpStream) -> std::io::Result<Held<'_>> {
        let clone = socket.try_clone()?;
        let id = self.0.next.fetch_add(1, Ordering::SeqCst);
        let mut open = self.0.open.lock().unwrap_or_else(PoisonError::into_inner);
        if self.is_aborted() {
            drop(open);
            let _ = clone.shutdown(Shutdown::Both);
            return Err(aborted());
        }
        open.push((id, clone));
        Ok(Held { abort: self, id })
    }
}

/// A socket registered with an [`Abort`], taken out when this drops.
pub(crate) struct Held<'a> {
    abort: &'a Abort,
    id: u64,
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        self.abort
            .0
            .open
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(id, _)| *id != self.id);
    }
}

/// The error an aborted request ends in.
pub(crate) fn aborted() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Interrupted, "aborted by its caller")
}

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
    fetch_abortable(host, port, call, most, None)
}

/// [`fetch`], stopped by `abort` from another thread: its socket is shut
/// ([`Abort`]).
///
/// The crate's own: [`crate::web`] is the way in, and a transport that asks
/// a host without its `Pass` is not handed out a second time.
///
/// # Errors
///
/// As [`fetch`], and an `Interrupted` I/O error once aborted — whatever the
/// read said, since a shut socket reads as an end of stream and a chunked
/// body, or one with no length, would pass for a whole answer.
pub(crate) fn fetch_abortable(
    host: &str,
    port: u16,
    call: &crate::Call<'_>,
    most: u64,
    abort: Option<&Abort>,
) -> Result<crate::Reply> {
    let most = most.min(MOST_EVER);
    if abort.is_some_and(Abort::is_aborted) {
        return Err(aborted()).context(format!("the request to {host} was not sent"));
    }
    let mut stream = connect(host, port, call.patience)?;
    // HELD BEFORE THE FIRST BYTE: `StreamOwned` handshakes on its first write,
    // so the handshake too is under the abort.
    let _held = abort
        .map(|a| a.hold(stream.get_ref()))
        .transpose()
        .with_context(|| format!("the request to {host} was not sent"))?;
    std::io::Write::write_all(&mut stream, crate::request_of(call).as_bytes())
        .with_context(|| format!("could not send the request to {host}"))?;
    std::io::Write::flush(&mut stream).ok();
    reply_read(&mut stream, host, most, abort)
}

/// [`fetch_abortable`] over PLAIN TCP: one call to an `http` address, NOT
/// ENCRYPTED — whoever is on the way reads it and can change it — behind
/// `fence` ([`Fence::Public`] for a host the person did not name).
///
/// Only [`crate::web::request_unencrypted`] reaches it: an address a person
/// pasted and was told crosses the network in the clear (an IPTV provider's
/// list is `http` more often than not). Nothing of ours asks a host of its
/// own choosing this way.
///
/// # Errors
///
/// As [`fetch_abortable`].
pub(crate) fn fetch_plain_abortable(
    host: &str,
    port: u16,
    call: &crate::Call<'_>,
    most: u64,
    abort: Option<&Abort>,
    fence: Fence,
) -> Result<crate::Reply> {
    let most = most.min(MOST_EVER);
    if abort.is_some_and(Abort::is_aborted) {
        return Err(aborted()).context(format!("the request to {host} was not sent"));
    }
    let mut stream = reach(host, port, call.patience, fence)?;
    let _held = abort
        .map(|a| a.hold(&stream))
        .transpose()
        .with_context(|| format!("the request to {host} was not sent"))?;
    std::io::Write::write_all(&mut stream, crate::request_of(call).as_bytes())
        .with_context(|| format!("could not send the request to {host}"))?;
    std::io::Write::flush(&mut stream).ok();
    reply_read(&mut stream, host, most, abort)
}

/// One POST over TLS whose body is a FILE, streamed as a one-part
/// `multipart/form-data` ([`crate::multipart`]) and never held in memory —
/// and the reply, within `most` bytes. `call` carries the head (its body is
/// ignored, its content type is replaced by the form's).
///
/// The crate's own, like [`fetch_abortable`]: [`crate::web::upload_let`] is
/// the way in, with the person's pass.
///
/// # Errors
///
/// As [`fetch`]; a file that carries the boundary, is not the length said,
/// or was stopped ([`crate::multipart::Unsent`]); an `Interrupted` error once
/// aborted.
pub(crate) fn upload_abortable(
    host: &str,
    port: u16,
    call: &crate::Call<'_>,
    sending: crate::multipart::Sending<'_>,
    most: u64,
    abort: Option<&Abort>,
) -> Result<crate::Reply> {
    let most = most.min(MOST_EVER);
    if abort.is_some_and(Abort::is_aborted) {
        return Err(aborted()).context(format!("the upload to {host} was not sent"));
    }
    let boundary = crate::multipart::boundary();
    let kind = crate::multipart::content_type(&boundary);
    let head_call = crate::Call {
        content_type: &kind,
        body: None,
        ..*call
    };
    let head = crate::head_of(
        &head_call,
        crate::multipart::length(&boundary, &sending.part),
    );
    let mut stream = connect(host, port, call.patience)?;
    let _held = abort
        .map(|a| a.hold(stream.get_ref()))
        .transpose()
        .with_context(|| format!("the upload to {host} was not sent"))?;
    std::io::Write::write_all(&mut stream, head.as_bytes())
        .with_context(|| format!("could not send the upload's head to {host}"))?;
    crate::multipart::write(&mut stream, &boundary, sending)
        .with_context(|| format!("could not send the file to {host}"))?;
    reply_read(&mut stream, host, most, abort)
}

/// The reply read off `stream`, to its end, within `most` bytes of body —
/// and refused when `abort` was set meanwhile. Generic so a test can hand it
/// what a shut socket reads.
fn reply_read(
    stream: &mut impl Read,
    host: &str,
    most: u64,
    abort: Option<&Abort>,
) -> Result<crate::Reply> {
    let mut raw = Vec::new();
    // The head is not counted against the ceiling a caller thinks of as the
    // BODY's, so the read allows a little over it.
    let allowance = most + (64 << 10);
    let read = ended(
        Read::take(stream, allowance + 1).read_to_end(&mut raw),
        &raw,
    );
    // ABORTED IS ABORTED, before anything is made of what came: a shut
    // socket reads as an end of stream, and a chunked body — or one with no
    // length — cut there would pass `whole` as an answer.
    if abort.is_some_and(Abort::is_aborted) {
        return Err(aborted()).context(format!("the request to {host} was aborted"));
    }
    read.with_context(|| format!("could not read {host}'s answer"))?;
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

    /// FAILS IF: a fenced connection to this machine or a network of its own
    /// is tried at all — the refusal comes from the addresses, before a
    /// socket is opened (a literal address resolves with no network).
    #[test]
    fn a_fenced_connection_never_reaches_this_machine() {
        for host in ["127.0.0.1", "::1", "192.168.1.1"] {
            let refused =
                reach(host, 9, Duration::from_millis(50), Fence::Public).expect_err("fenced off");
            assert!(
                format!("{refused:#}").contains("a network of its own"),
                "{host}: {refused:#}"
            );
        }
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

    /// FAILS IF: an abort from another thread does not end a read that is
    /// blocked on a host that never answers — the search would hold its
    /// thread for the whole patience after the person typed past it.
    #[test]
    fn an_abort_ends_a_blocked_read_at_once() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let at = listener.local_addr().unwrap();
        // The host accepts and says nothing, for longer than the test waits.
        let quiet = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_secs(3));
            drop(socket);
        });
        let socket = TcpStream::connect(at).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let abort = Abort::new();
        let held = abort.hold(&socket).unwrap();
        let from_elsewhere = abort.clone();
        let t0 = std::time::Instant::now();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            from_elsewhere.abort();
        });
        let mut buf = [0u8; 16];
        let read = (&socket).read(&mut buf);
        assert!(
            matches!(read, Ok(0) | Err(_)),
            "the read ended with the end of the stream or an error: {read:?}"
        );
        assert!(
            t0.elapsed() < Duration::from_secs(2),
            "the abort ended the read, not the host: {:?}",
            t0.elapsed()
        );
        stopper.join().unwrap();
        drop(held);
        assert!(abort.is_aborted());
        // An aborted handle refuses the next socket, and shuts it.
        let late = TcpStream::connect(at);
        if let Ok(late) = late {
            assert_eq!(
                abort.hold(&late).err().map(|e| e.kind()),
                Some(std::io::ErrorKind::Interrupted)
            );
        }
        quiet.join().unwrap();
    }

    /// FAILS IF: a chunked answer cut by an abort (the shut socket reads as
    /// its end) is handed back as if it were whole.
    #[test]
    fn a_reply_cut_by_an_abort_is_no_reply() {
        let half = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhel";
        let got = reply_read(&mut &half[..], "h", MOST, None);
        assert!(got.is_ok(), "unaborted, a chunked body is taken as it came");
        let abort = Abort::new();
        abort.abort();
        let e = reply_read(&mut &half[..], "h", MOST, Some(&abort)).unwrap_err();
        let io = e
            .chain()
            .find_map(|c| c.downcast_ref::<std::io::Error>())
            .map(std::io::Error::kind);
        assert_eq!(io, Some(std::io::ErrorKind::Interrupted), "{e:#}");
    }

    #[test]
    fn a_request_aborted_before_it_starts_never_connects() {
        let abort = Abort::new();
        abort.abort();
        let call = crate::Call::new("GET", "/", "example.org");
        // `example.org` is never resolved: the refusal is before the connect.
        let e = fetch_abortable("example.org", 443, &call, MOST, Some(&abort)).unwrap_err();
        let io = e
            .chain()
            .find_map(|c| c.downcast_ref::<std::io::Error>())
            .map(std::io::Error::kind);
        assert_eq!(io, Some(std::io::ErrorKind::Interrupted), "{e:#}");
    }

    #[test]
    fn the_bundle_path_is_the_one_every_other_program_reads() {
        // If this ever moves, the symptom is a machine where the browser works
        // and this does not — so the path is stated once and asserted.
        assert_eq!(ROOTS, "/etc/ssl/certs/ca-certificates.crt");
    }
}
