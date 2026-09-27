//! The footer's end of the bridge: a Unix socket the hosts connect to.
//!
//! # The footer is the server
//!
//! Like the control socket: one per instance (`ronoctua[-dev]-browser.sock`),
//! and any number of hosts — Chrome starts one per profile that has a kiosk
//! — connect to it. A host that finds no socket waits for it to appear, so a
//! footer that restarts is found again without anyone polling.
//!
//! # Threads, and nothing blocks the caller
//!
//! One thread accepts, one thread per connection reads lines; what they hear
//! goes to the caller's function, which for the footer is a calloop channel.
//! Writing an order is one line on the connection's stream, from the caller's
//! thread — small, and the host reads its socket continuously.

use crate::wire::{Envelope, Said};
use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The socket's leaf name, under the runtime directory and the instance.
pub const SOCKET: &str = "browser.sock";

/// The longest line a host may send: a state with hundreds of tabs fits many
/// times over; a line past it is a bug or an attempt, and ends the connection.
///
/// TWICE THE HOST'S FRAME (`host::CEILING`), and not the same number: the
/// host ENLARGES every state on its way here — each extension gains its
/// popup and its place among the pinned — so a frame the host took whole
/// would be a line this side refused, and the host would give it again on
/// every reconnection. The host also refuses to send a line past this one
/// (`host::Host::to_footer`), so the two can never disagree.
pub const CEILING: usize = 2 * crate::host::CEILING as usize;

/// Where the socket lives for this instance, or `None` without a runtime
/// directory (no fallback to `/tmp`: a socket in a shared directory is attack
/// surface).
#[must_use]
pub fn socket_path() -> Option<PathBuf> {
    ronoctua_xdg::runtime_file(SOCKET)
}

/// One host's connection, by a number that is never reused in this process.
pub type Conn = u64;

/// What the server hears.
#[derive(Debug, Clone, PartialEq)]
pub enum Heard {
    /// A host connected (it sends its whole state right after).
    Connected(Conn),
    /// A host relayed something the extension said.
    Said(Conn, Said),
    /// A host went away: Chrome closed its last kiosk of that profile, or
    /// Chrome itself ended.
    Gone(Conn),
}

/// The listening socket and the live connections.
pub struct Server {
    path: PathBuf,
    /// The socket file THIS server bound — its device and inode — so that
    /// leaving removes that file and never another server's at the same path.
    bound: Option<(u64, u64)>,
    conns: Arc<Mutex<HashMap<Conn, UnixStream>>>,
}

/// A file's device and inode: what tells two files at one path apart.
fn identity(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::symlink_metadata(path)
        .ok()
        .map(|m| (m.dev(), m.ino()))
}

impl Server {
    /// Binds the socket and starts accepting; every event goes to `on`.
    ///
    /// A socket file left by a dead footer is replaced; a LIVE one (another
    /// footer of the same instance answering) is not, and the bind fails.
    ///
    /// # Errors
    ///
    /// When the socket cannot be bound.
    pub fn listen(
        path: PathBuf,
        on: impl Fn(Heard) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        if path.exists() && UnixStream::connect(&path).is_err() {
            let _ = std::fs::remove_file(&path);
        }
        let listener = UnixListener::bind(&path)?;
        let bound = identity(&path);
        let conns: Arc<Mutex<HashMap<Conn, UnixStream>>> = Arc::default();
        let on = Arc::new(on);
        let next = Arc::new(AtomicU64::new(1));
        let accepting = Arc::clone(&conns);
        std::thread::Builder::new()
            .name("browser-accept".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    let Ok(writer) = stream.try_clone() else {
                        continue;
                    };
                    let id = next.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut map) = accepting.lock() {
                        map.insert(id, writer);
                    }
                    on(Heard::Connected(id));
                    let on = Arc::clone(&on);
                    let reading = Arc::clone(&accepting);
                    let _ = std::thread::Builder::new()
                        .name("browser-conn".into())
                        .spawn(move || {
                            read_lines(stream, id, on.as_ref());
                            if let Ok(mut map) = reading.lock() {
                                map.remove(&id);
                            }
                            on(Heard::Gone(id));
                        });
                }
            })?;
        Ok(Self { path, bound, conns })
    }

    /// Sends one order to one host.
    ///
    /// # Errors
    ///
    /// When the connection is gone or the write fails.
    pub fn send(&self, conn: Conn, envelope: &Envelope) -> std::io::Result<()> {
        let mut line = serde_json::to_vec(envelope)?;
        line.push(b'\n');
        let mut map = self
            .conns
            .lock()
            .map_err(|_| std::io::Error::other("the connection table is poisoned"))?;
        let stream = map
            .get_mut(&conn)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotConnected, "no such host"))?;
        stream.write_all(&line)
    }

    /// The socket's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Server {
    /// The socket file goes with its server — ONLY if it is still the one
    /// this server bound. A footer that is leaving while its successor has
    /// already bound the same path would otherwise take the successor's
    /// socket with it, and every host would wait for a file nobody makes.
    fn drop(&mut self) {
        if self.bound.is_some() && identity(&self.path) == self.bound {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Reads one connection until it ends, handing every line that parses to
/// `on`; a line that does not parse is logged and skipped (a newer
/// extension's word), a line past [`CEILING`] ends the connection.
fn read_lines(stream: UnixStream, id: Conn, on: &(impl Fn(Heard) + ?Sized)) {
    let mut reader = BufReader::new(stream);
    let mut line = Vec::new();
    loop {
        line.clear();
        let mut limited = (&mut reader).take(CEILING as u64 + 1);
        match limited.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if line.len() > CEILING {
            tracing::warn!(conn = id, "a browser host sent a line past the ceiling");
            return;
        }
        match serde_json::from_slice::<Said>(line.trim_ascii()) {
            Ok(said) => on(Heard::Said(id, said)),
            Err(e) => tracing::warn!(conn = id, error = %e, "a browser line that does not parse"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{Order, State};
    use std::sync::mpsc;

    #[test]
    fn a_host_is_heard_and_answered() {
        let dir = std::env::temp_dir().join(format!("ronoctua-serve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("b.sock");
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let server = Server::listen(path.clone(), move |h| {
            let _ = tx.lock().expect("lock").send(h);
        })
        .expect("listen");
        let mut host = UnixStream::connect(&path).expect("connect");
        let Heard::Connected(id) = rx.recv().expect("connected") else {
            panic!("connected first")
        };
        let state = Said::State(State {
            v: crate::wire::VERSION,
            ..State::default()
        });
        let mut line = serde_json::to_vec(&state).expect("json");
        line.push(b'\n');
        host.write_all(b"{\"said\":\"shout\"}\n").expect("write");
        host.write_all(&line).expect("write");
        assert_eq!(rx.recv().expect("said"), Heard::Said(id, state));
        server
            .send(
                id,
                &Envelope {
                    serial: 3,
                    order: Order::Look,
                },
            )
            .expect("send");
        let mut back = String::new();
        BufReader::new(&host).read_line(&mut back).expect("read");
        assert_eq!(back, "{\"serial\":3,\"order\":\"look\"}\n");
        drop(host);
        assert_eq!(rx.recv().expect("gone"), Heard::Gone(id));
        drop(server);
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a server that leaves removes the socket file ANOTHER server
    /// bound at the same path since — a footer handing over to the next one
    /// would leave every Chrome's host waiting for a socket nobody makes.
    #[test]
    fn a_server_that_leaves_takes_only_its_own_socket() {
        let dir = std::env::temp_dir().join(format!("ronoctua-serve-own-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("b.sock");
        let old = Server::listen(path.clone(), |_| {}).expect("the old footer listens");
        // THE NEXT FOOTER took the place: the old file is gone (its footer was
        // asked to leave), and a new socket is bound at the same path.
        std::fs::remove_file(&path).expect("the old file goes");
        let new = Server::listen(path.clone(), |_| {}).expect("the new footer listens");
        drop(old);
        assert!(
            path.exists(),
            "the leaving server took its successor's socket"
        );
        assert!(
            UnixStream::connect(&path).is_ok(),
            "a host still reaches the new footer"
        );
        drop(new);
        assert!(!path.exists(), "the new server's own file goes with it");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
