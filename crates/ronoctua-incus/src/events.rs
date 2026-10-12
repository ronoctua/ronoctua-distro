//! The daemon's events — `/1.0/events`, a WebSocket over its unix socket.
//!
//! # Events, not a clock
//!
//! A door that shows the instances reads them once and then listens: an
//! instance started, a snapshot was taken, an operation moved. Nothing polls.
//!
//! # Only while shown
//!
//! [`Events`] is an iterator that blocks on its socket, so it lives on a
//! thread; the door that decides the feed is no longer shown lives on the
//! loop. It keeps the [`Stopper`] ([`Events::stopper`]): dropping it shuts
//! the socket down, and the blocked `next` answers `None` at once. One `drop` — no flag a
//! blocked reader would read only after the next event.

use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use ronoctua_http::ws::{Message, Socket};
use serde::Deserialize;
use serde_json::Value;

use crate::rest::{self, Operation};
use crate::{Refusal, Why};

/// How long one read waits before it looks again. Not a clock: the socket is
/// blocked on, and this only bounds how long a broken peer goes unnoticed.
const READ_SLICE: Duration = Duration::from_secs(30);

/// The types asked for: what changed, and what moved.
const PATH: &str = "/1.0/events?type=lifecycle,operation";

/// What changed, by what it is about — what a door reads again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topic {
    /// An instance (created, started, stopped, deleted, renamed…).
    Instances,
    /// An instance's snapshots.
    Snapshots,
    /// The images.
    Images,
    /// The networks.
    Networks,
    /// The pools and their volumes.
    Storage,
    /// The profiles.
    Profiles,
    /// The projects.
    Projects,
    /// Anything else (certificates, the server's configuration…).
    Other,
}

impl Topic {
    /// What a lifecycle action is about (`instance-snapshot-created` →
    /// snapshots). Pure.
    #[must_use]
    pub fn of_action(action: &str) -> Self {
        if action.starts_with("instance-snapshot-") {
            Self::Snapshots
        } else if action.starts_with("instance-") {
            Self::Instances
        } else if action.starts_with("image-") {
            Self::Images
        } else if action.starts_with("network-") {
            Self::Networks
        } else if action.starts_with("storage-pool-") || action.starts_with("storage-volume-") {
            Self::Storage
        } else if action.starts_with("profile-") {
            Self::Profiles
        } else if action.starts_with("project-") {
            Self::Projects
        } else {
            Self::Other
        }
    }
}

/// One event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Something changed.
    Changed {
        /// What it is about.
        topic: Topic,
        /// The daemon's action (`instance-started`) — data.
        action: String,
        /// The name of what changed (`web`, or `web/snap0` for a snapshot).
        name: String,
        /// Its project.
        project: String,
    },
    /// An operation moved.
    Operation(Operation),
}

#[derive(Debug, Deserialize)]
struct RawEvent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    metadata: Value,
    #[serde(default)]
    project: String,
}

/// A lifecycle event's metadata, as the daemon sends it: the action and the
/// path of what it is about (`source`), and no name — the VM's capture
/// (`incus-vm`) holds `action`, `requestor` and `source` only, with the
/// project on the event around it.
#[derive(Debug, Deserialize)]
struct RawLifecycle {
    #[serde(default)]
    action: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    project: String,
}

/// The name a lifecycle event is about, out of its `source`:
/// `/1.0/instances/web` is `web`, `/1.0/instances/web/snapshots/s0` is
/// `web/s0`, and a volume (`/1.0/storage-pools/default/volumes/custom/v`)
/// is `default/v` — after the collection, each name, without the
/// sub-collections between them nor a volume's type. Empty for a path that is
/// not the API's. Pure.
fn name_of(source: &str) -> String {
    let path = source.split('?').next().unwrap_or_default();
    let Some(rest) = path.strip_prefix("/1.0/") else {
        return String::new();
    };
    let mut segments = rest.split('/').skip(1);
    let mut names: Vec<&str> = segments.next().into_iter().collect();
    while let Some(sub) = segments.next() {
        if sub == "volumes" {
            // A VOLUME'S TYPE (`custom`, `container`…) comes before its name.
            segments.next();
        }
        names.extend(segments.next());
    }
    names.join("/")
}

/// An event out of one message. Pure. `None` for a message this build does
/// not read — a type it did not ask for, or something that is not one.
#[must_use]
pub(crate) fn event_of(text: &str) -> Option<Event> {
    let raw: RawEvent = serde_json::from_str(text).ok()?;
    match raw.kind.as_str() {
        "lifecycle" => {
            let life = RawLifecycle::deserialize(&raw.metadata).ok()?;
            Some(Event::Changed {
                topic: Topic::of_action(&life.action),
                name: name_of(&life.source),
                action: life.action,
                project: if life.project.is_empty() {
                    raw.project
                } else {
                    life.project
                },
            })
        }
        "operation" => rest::operation_of(&raw.metadata).map(Event::Operation),
        _ => None,
    }
}

/// The daemon's events, while somebody listens.
#[derive(Debug)]
pub struct Events {
    socket: Socket<UnixStream>,
    stopper: Option<Stopper>,
}

/// What ends a feed from another thread: dropped (or [`Stopper::stop`]),
/// the socket is shut down and the feed's `next` answers `None`.
#[derive(Debug)]
pub struct Stopper {
    stream: UnixStream,
}

impl Stopper {
    /// Ends the feed now.
    pub fn stop(self) {
        drop(self);
    }
}

impl Drop for Stopper {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl Events {
    /// Opens the feed.
    ///
    /// # Errors
    ///
    /// [`Why::NoIncus`], [`Why::NotInGroup`]; [`Why::Failed`] when the
    /// daemon did not upgrade the call.
    pub(crate) fn open(socket: &Path) -> Result<Self, Refusal> {
        let stream = rest::connect(socket, READ_SLICE)?;
        let stopper = stream.try_clone().map_err(|_| Refusal::of(Why::Failed))?;
        let socket = Socket::connect(stream, rest::HOST, PATH, &[]).map_err(|e| {
            tracing::warn!(error = %e, "incus's events could not be opened");
            Refusal::of(Why::Failed)
        })?;
        Ok(Self {
            socket,
            stopper: Some(Stopper { stream: stopper }),
        })
    }

    /// The handle that ends this feed from elsewhere — given once.
    pub fn stopper(&mut self) -> Option<Stopper> {
        self.stopper.take()
    }
}

/// The events, one after another; the iteration ends with the feed.
impl Iterator for Events {
    type Item = Event;

    /// The next event; `None` when the feed ended (the daemon went, or the
    /// stopper was dropped). Blocks: read it on a thread.
    fn next(&mut self) -> Option<Event> {
        loop {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    if let Some(event) = event_of(&text) {
                        return Some(event);
                    }
                }
                // SILENCE IS NOT AN END: the slice ran out and nothing moved.
                Ok(Message::Nothing | Message::Binary(_)) => {}
                Ok(Message::Closed) | Err(_) => return None,
            }
        }
    }
}

impl Drop for Events {
    fn drop(&mut self) {
        let _ = self.socket.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::rest::OpStatus;

    #[test]
    fn each_event_says_what_to_read_again() {
        assert_eq!(
            event_of(fixtures::EVENT_LIFECYCLE),
            Some(Event::Changed {
                topic: Topic::Instances,
                action: "instance-started".to_owned(),
                name: "web".to_owned(),
                project: "default".to_owned(),
            })
        );
        let Some(Event::Changed { topic, name, .. }) = event_of(fixtures::EVENT_SNAPSHOT) else {
            panic!("a snapshot event");
        };
        assert_eq!(
            (topic, name.as_str()),
            (Topic::Snapshots, "web/before-upgrade")
        );
        let Some(Event::Operation(op)) = event_of(fixtures::EVENT_OPERATION) else {
            panic!("an operation event");
        };
        assert_eq!(op.status, OpStatus::Running);
        assert_eq!(op.step, Some(crate::Step::Percent(23)));
        assert_eq!(event_of(r#"{"type":"logging","metadata":{}}"#), None);
        assert_eq!(event_of("not json"), None);
    }

    /// FAILS IF: the name of what changed stops being read out of the path
    /// the daemon sends — it sends no name of its own.
    #[test]
    fn the_name_is_read_from_the_source() {
        for (source, name) in [
            ("/1.0/instances/web", "web"),
            ("/1.0/instances/web?project=lab", "web"),
            (
                "/1.0/instances/web/snapshots/before-upgrade",
                "web/before-upgrade",
            ),
            (
                "/1.0/storage-pools/default/volumes/custom/blk",
                "default/blk",
            ),
            ("/1.0/networks/incusbr0", "incusbr0"),
            ("/1.0/instances", ""),
            ("instances/web", ""),
        ] {
            assert_eq!(name_of(source), name, "{source}");
        }
    }

    #[test]
    fn every_topic_is_read_from_its_action() {
        for (action, topic) in [
            ("instance-created", Topic::Instances),
            ("instance-snapshot-deleted", Topic::Snapshots),
            ("image-deleted", Topic::Images),
            ("network-updated", Topic::Networks),
            ("storage-volume-created", Topic::Storage),
            ("storage-pool-updated", Topic::Storage),
            ("profile-updated", Topic::Profiles),
            ("project-created", Topic::Projects),
            ("certificate-created", Topic::Other),
        ] {
            assert_eq!(Topic::of_action(action), topic, "{action}");
        }
    }

    /// FAILS IF: a feed blocked on its socket outlives the door that dropped
    /// its stopper — the feed would run for as long as the daemon is quiet.
    #[test]
    fn dropping_the_stopper_ends_a_blocked_feed() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-incus-events-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("unix.socket");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        // A DAEMON THAT UPGRADES AND THEN SAYS NOTHING.
        let server = std::thread::spawn(move || {
            use std::io::{BufRead as _, Write as _};
            let (stream, _) = listener.accept().unwrap();
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut key = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(k) = line.strip_prefix("Sec-WebSocket-Key: ") {
                    key = k.trim().to_owned();
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut out = stream;
            write!(
                out,
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                ronoctua_http::ws::accept_for(&key)
            )
            .unwrap();
            // Hold the socket open until the client goes.
            let mut sink = [0u8; 64];
            while std::io::Read::read(&mut reader, &mut sink).is_ok_and(|n| n > 0) {}
        });
        let mut events = Events::open(&path).unwrap();
        let stopper = events.stopper().unwrap();
        assert!(events.stopper().is_none(), "given once");
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            let _ = tx.send(events.next());
        });
        std::thread::sleep(Duration::from_millis(100));
        drop(stopper);
        let ended = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the feed ended");
        assert_eq!(ended, None);
        reader.join().unwrap();
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}
