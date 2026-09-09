//! HTTPS by hand, over `std::net` — the first TLS in this workspace.
//!
//! # Why there was none, and why there is one now
//!
//! `ronoctua-transfers`'s `aria2.rs` refuses an HTTP client in writing: *"an
//! HTTP client would bring tokio, hyper and a TLS stack into a `Cargo.lock`
//! every crate in the workspace shares, to save about sixty lines that will
//! never change."* That was right, and it is still two thirds right — there is
//! no tokio here and no hyper. What changed is the third: `aria2` talks to
//! `127.0.0.1`, and this talks to somebody else's machine, where there is no
//! version of "no TLS" that is acceptable.
//!
//! So the cut is where the project always puts it: the dependency comes in
//! when its problem shows up, and it comes in as small as the problem.
//! `rustls` over a blocking `TcpStream`, with `ring` rather than the default
//! provider, which needs cmake and a Go toolchain to fetch two numbers.
//!
//! # The roots come from the SYSTEM
//!
//! Not `webpki-roots`. A root bundled into the binary ages inside it, and the
//! day one expires the symptom is every fetch failing on a machine whose
//! browser is fine. NixOS declares `security.pki` and puts the bundle where
//! every other program on the machine looks for it, so this looks there too —
//! and when it is not there, this crate says so once and fetches nothing,
//! which is a feature not working rather than a certificate being ignored.
//!
//! # What it does NOT do
//!
//! Redirects, proxies, keep-alive, compression, cookies, authentication. Two
//! hosts answer a `GET` with JSON and `Connection: close`. Anything else is a
//! problem that has not shown up.

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};

/// How long a host on the internet is given.
///
/// Ten seconds, against the two a socket in `/run` gets: this crosses a
/// network somebody else runs, and the thread it blocks is not a drawing one.
/// It is a ceiling and not a budget — the fetch that takes it has failed.
pub const PATIENCE: Duration = Duration::from_secs(10);

/// The most a body may be, before it is refused.
///
/// Both answers are a few hundred bytes. A megabyte is four orders of
/// magnitude of room and still a bound: without one, a host that answered for
/// ever would fill this process's memory, and the process is the footer.
const MOST: u64 = 1 << 20;

/// Where the system keeps the certificate authorities it trusts.
///
/// The NixOS path, which is also the Debian and Arch path — `security.pki`
/// assembles the bundle and every program on the machine reads this file.
const ROOTS: &str = "/etc/ssl/certs/ca-certificates.crt";

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

/// A configuration, built once and shared by every fetch.
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
                // ONCE, and loudly. It is not per-fetch: a machine with no
                // bundle would otherwise write this line every half hour for
                // ever.
                tracing::warn!(error = %e, "no TLS is possible; the glance keeps its clocks");
                None
            }
        }
    })
    .clone()
    .context("the system's certificate bundle could not be read")
}

/// `GET https://<host><path>`, and the body that came back.
///
/// # Errors
///
/// The name does not resolve, the host did not answer inside [`PATIENCE`], the
/// certificate did not verify, the status was not 200, or the body was larger
/// than this is willing to read.
pub fn get(host: &str, path: &str) -> Result<Vec<u8>> {
    let config = shared()?;
    let name = rustls_pki_types::ServerName::try_from(host.to_owned())
        .with_context(|| format!("`{host}` is not a server name"))?;
    let mut session = rustls::ClientConnection::new(config, name)
        .context("the TLS session could not be started")?;

    // THE DEADLINE IS ON THE CONNECT TOO, and it has to be: name resolution
    // and a SYN to a host that drops packets are both silent waits, and
    // `TcpStream::connect` has no timeout of its own.
    let addresses: Vec<std::net::SocketAddr> =
        std::net::ToSocketAddrs::to_socket_addrs(&(host, 443))
            .with_context(|| format!("`{host}` did not resolve"))?
            .collect();
    let address = addresses
        .first()
        .with_context(|| format!("`{host}` resolved to nothing"))?;
    let mut socket = TcpStream::connect_timeout(address, PATIENCE)
        .with_context(|| format!("could not reach {host}"))?;
    socket.set_read_timeout(Some(PATIENCE))?;
    socket.set_write_timeout(Some(PATIENCE))?;

    let mut tls = rustls::Stream::new(&mut session, &mut socket);
    write!(
        tls,
        "GET {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         User-Agent: ronoctua\r\n\
         Accept: application/json\r\n\
         Connection: close\r\n\r\n"
    )
    .with_context(|| format!("could not send the request to {host}"))?;
    tls.flush().ok();

    let mut raw = Vec::new();
    // BOUNDED, and by bytes rather than by trust. `Connection: close` means
    // read to the end, and "the end" is whatever the other side decides.
    std::io::Read::take(&mut tls, MOST + 1)
        .read_to_end(&mut raw)
        .with_context(|| format!("could not read {host}'s answer"))?;
    if raw.len() as u64 > MOST {
        bail!("{host} answered more than {MOST} bytes; it is not the API this expects");
    }

    // BYTES, NOT `&str`, up to the split. The head is ASCII by the grammar,
    // and the body is not ours to assume anything about — the panic that
    // taught this is written up in `ronoctua_tunnel::http`.
    let at = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .context("the answer has no header break; it is not HTTP")?;
    let head = String::from_utf8_lossy(&raw[..at]).into_owned();
    let body = &raw[at + 4..];

    match ronoctua_tunnel::http::status_of(&head) {
        Some(200) => {}
        Some(status) => bail!("{host} answered {status}"),
        None => bail!("{host}'s answer has no status line"),
    }
    // THE SAME DECHUNKER, and not a second one. See this crate's manifest for
    // why the dependency points where it does.
    Ok(ronoctua_tunnel::http::dechunked(&head, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing here reaches the network, and that is deliberate: a test that
    /// needs the internet is a test that fails in the sandbox this project
    /// builds in. What is worth holding is the SHAPE of the request.
    #[test]
    fn the_patience_is_longer_than_a_local_sockets_and_bounded() {
        assert!(PATIENCE > ronoctua_tunnel::http::PATIENCE);
        assert!(PATIENCE <= Duration::from_secs(30));
    }

    /// The ceiling is far above either answer and still a ceiling.
    ///
    /// A `const` compared with `const` is a constant assertion, and clippy is
    /// right to say so — the range is stated here as documentation of what the
    /// number is FOR, and the compile-time check is what enforces it.
    const _: () = {
        assert!(MOST >= 1 << 16, "too tight for a legitimate answer");
        assert!(MOST <= 1 << 24, "not a bound worth having");
    };

    /// A host that is not a name is refused before a socket is opened.
    #[test]
    fn a_name_that_is_not_a_name_never_reaches_the_network() {
        for bad in ["", "not a host", "http://example.com"] {
            assert!(
                rustls_pki_types::ServerName::try_from(bad.to_owned()).is_err(),
                "`{bad}` was accepted as a server name"
            );
        }
    }
}
