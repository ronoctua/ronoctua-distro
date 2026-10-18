//! The clipboard's history between its two screens — the footer, which KEEPS
//! it, and the window, which only draws it.
//!
//! # Why the window does not watch the clipboard
//!
//! Because a second watcher is a second history. The history is a `VecDeque`
//! in the footer, filled by that process's own `zwlr_data_control` watcher and
//! by what ClipCascade brings from other machines; a window with a watcher of
//! its own would hold a different list the moment it opened — the defect
//! `ronoctua-clipcascade` exists to avoid, one process over. So the window
//! asks the FOOTER, over the control socket: one history, two screens.
//!
//! # The two directions
//!
//! - The window says `clipboard-follow` and keeps the connection: the footer
//!   answers with the whole history at once and again after every change, ONE
//!   JSON per line, each line whole (the rule every daemon of this system
//!   follows — never a delta).
//! - What the window's keys do goes back as `clipboard-act <generation>
//!   <json>`: the footer carries it out on its own history, by the same
//!   errands its own view uses, and refuses one aimed at a generation that
//!   is gone — an index into a list that has moved is an index into another
//!   entry.
//!
//! A login's password crosses this socket, which lives in the person's own
//! runtime directory (`0700`), never an `argv`; the footer hands it to
//! ClipCascade, which derives the key and drops it. [`Act`]'s `Debug` never
//! prints it, so no log line can.

use base64::Engine as _;
use ronoctua_clipboard::{Content, Entry, Queue};
use ronoctua_clipcascade::{Snapshot, prefs::Mark, prefs::Settings};
use serde::{Deserialize, Serialize};

/// The word the window opens its connection with.
pub const FOLLOW: &str = "clipboard-follow";

/// The word an act travels under.
pub const ACT: &str = "clipboard-act";

/// One entry as it crosses — every kind of [`Content`], whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Wired {
    /// Text, as it came.
    Text {
        /// The text.
        text: String,
        /// The MIME types offered.
        kinds: Vec<String>,
        /// Pinned.
        pinned: bool,
    },
    /// An image's file, undecoded, in base64.
    Image {
        /// The bytes.
        data: String,
        /// The MIME types offered.
        kinds: Vec<String>,
        /// Pinned.
        pinned: bool,
    },
    /// What could not be read.
    Unreadable {
        /// The MIME type offered.
        what: String,
        /// How many bytes came.
        bytes: usize,
        /// The MIME types offered.
        kinds: Vec<String>,
        /// Pinned.
        pinned: bool,
    },
}

impl Wired {
    /// An entry, for the wire.
    fn of(e: &Entry) -> Self {
        let kinds = e.kinds.clone();
        let pinned = e.pinned;
        match &e.content {
            Content::Text(text) => Self::Text {
                text: text.clone(),
                kinds,
                pinned,
            },
            Content::Image(bytes) => Self::Image {
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
                kinds,
                pinned,
            },
            Content::Unreadable(u) => Self::Unreadable {
                what: u.kind.clone(),
                bytes: u.bytes,
                kinds,
                pinned,
            },
        }
    }

    /// The entry back. `None` for an image whose bytes do not decode — which
    /// a footer of this program never sends.
    fn entry(&self) -> Option<Entry> {
        Some(match self {
            Self::Text {
                text,
                kinds,
                pinned,
            } => Entry {
                content: Content::Text(text.clone()),
                kinds: kinds.clone(),
                pinned: *pinned,
            },
            Self::Image {
                data,
                kinds,
                pinned,
            } => Entry {
                content: Content::Image(
                    base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .ok()?,
                ),
                kinds: kinds.clone(),
                pinned: *pinned,
            },
            Self::Unreadable {
                what,
                bytes,
                kinds,
                pinned,
            } => Entry {
                content: Content::Unreadable(ronoctua_clipboard::Undecodable {
                    kind: what.clone(),
                    bytes: *bytes,
                }),
                kinds: kinds.clone(),
                pinned: *pinned,
            },
        })
    }
}

/// What ClipCascade says, as it crosses — every field the view draws.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WiredSync {
    /// See [`Snapshot::configured`].
    pub configured: bool,
    /// See [`Snapshot::enabled`].
    pub enabled: bool,
    /// See [`Snapshot::connected`].
    pub connected: bool,
    /// See [`Snapshot::server`].
    pub server: String,
    /// See [`Snapshot::username`].
    pub username: String,
    /// See [`Snapshot::mode`].
    pub mode: String,
    /// See [`Snapshot::most`].
    pub most: u64,
    /// See [`Snapshot::cap`].
    pub cap: u64,
    /// See [`Snapshot::images`].
    pub images: bool,
    /// See [`Snapshot::notify`].
    pub notify: bool,
    /// See [`Snapshot::salt`].
    pub salt: String,
    /// See [`Snapshot::rounds`].
    pub rounds: u32,
    /// See [`Snapshot::mark`].
    pub mark: Mark,
    /// See [`Snapshot::trouble`].
    pub trouble: Option<String>,
    /// See [`Snapshot::sent`].
    pub sent: u64,
    /// See [`Snapshot::received`].
    pub received: u64,
}

impl WiredSync {
    /// A snapshot, for the wire.
    #[must_use]
    pub fn of(s: &Snapshot) -> Self {
        Self {
            configured: s.configured,
            enabled: s.enabled,
            connected: s.connected,
            server: s.server.clone(),
            username: s.username.clone(),
            mode: s.mode.clone(),
            most: s.most,
            cap: s.cap,
            images: s.images,
            notify: s.notify,
            salt: s.salt.clone(),
            rounds: s.rounds,
            mark: s.mark,
            trouble: s.trouble.clone(),
            sent: s.sent,
            received: s.received,
        }
    }

    /// The snapshot back.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            configured: self.configured,
            enabled: self.enabled,
            connected: self.connected,
            server: self.server.clone(),
            username: self.username.clone(),
            mode: self.mode.clone(),
            most: self.most,
            cap: self.cap,
            images: self.images,
            notify: self.notify,
            salt: self.salt.clone(),
            rounds: self.rounds,
            mark: self.mark,
            trouble: self.trouble.clone(),
            sent: self.sent,
            received: self.received,
        }
    }
}

/// The whole history, as one line on the wire.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mirror {
    /// Which history this is: it moves on every change, and an act names it.
    pub generation: u64,
    /// The entries, newest first.
    pub entries: Vec<Wired>,
    /// What ClipCascade says.
    pub sync: WiredSync,
    /// What the footer SAID in answer to an act of a window — a copy made,
    /// a refusal, an act aimed at a history that had moved — for the window
    /// to say on its own line: what a door says is said in it.
    #[serde(default)]
    pub said: Option<String>,
}

impl Mirror {
    /// The footer's history and ClipCascade's reading, as they are now — for
    /// a test; the footer hands its followers a [`Shot`].
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn of(queue: &Queue, sync: &Snapshot, generation: u64) -> Self {
        Self::of_parts(queue.inputs().iter(), sync, generation)
    }

    /// The same, from the entries themselves — what a follower's thread
    /// holds, cloned off the footer's loop.
    #[must_use]
    pub fn of_parts<'a>(
        entries: impl IntoIterator<Item = &'a Entry>,
        sync: &Snapshot,
        generation: u64,
    ) -> Self {
        Self {
            generation,
            entries: entries.into_iter().map(Wired::of).collect(),
            sync: WiredSync::of(sync),
            said: None,
        }
    }

    /// The history, for the window's view.
    #[must_use]
    pub fn queue(&self) -> Queue {
        Queue::of_entries(self.entries.iter().filter_map(Wired::entry))
    }

    /// One line: the JSON, and the newline that ends it — written whole.
    ///
    /// # Errors
    ///
    /// When it cannot be serialised, which plain data never is.
    pub fn line(&self) -> serde_json::Result<String> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }
}

/// What the window's keys ask of the footer's history.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "kebab-case")]
pub enum Act {
    /// Put entry `n` back on the clipboard.
    Copy {
        /// The entry.
        n: usize,
    },
    /// Drop entry `n`.
    Remove {
        /// The entry.
        n: usize,
    },
    /// Pin or unpin entry `n`.
    Pin {
        /// The entry.
        n: usize,
    },
    /// Empty the history.
    Clear,
    /// Turn sync on or off.
    Sync {
        /// On.
        on: bool,
    },
    /// Forget the account.
    LogOut,
    /// Keep these settings — the whole record.
    Keep {
        /// The salt.
        salt: String,
        /// The rounds.
        rounds: u32,
        /// Images too.
        images: bool,
        /// A notification for what arrives.
        notify: bool,
        /// The largest entry sent.
        cap: u64,
        /// The modeline's mark.
        mark: Mark,
    },
    /// Log in. The password goes to ClipCascade and nowhere else.
    LogIn {
        /// The server.
        server: String,
        /// The account.
        username: String,
        /// The password.
        password: String,
    },
    /// One of the typed settings, answered on the window's own line — the
    /// id the footer's minibuffer asks under (`clipboard-salt`, `-rounds`,
    /// `-cap`), and what was typed.
    Typed {
        /// The question's id.
        id: String,
        /// What was typed.
        text: String,
    },
}

/// NEVER THE PASSWORD: a log line that printed an act would print it.
impl std::fmt::Debug for Act {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LogIn {
                server, username, ..
            } => f
                .debug_struct("LogIn")
                .field("server", server)
                .field("username", username)
                .finish_non_exhaustive(),
            Self::Copy { n } => write!(f, "Copy({n})"),
            Self::Remove { n } => write!(f, "Remove({n})"),
            Self::Pin { n } => write!(f, "Pin({n})"),
            Self::Clear => f.write_str("Clear"),
            Self::Sync { on } => write!(f, "Sync({on})"),
            Self::LogOut => f.write_str("LogOut"),
            Self::Keep { .. } => f.write_str("Keep"),
            Self::Typed { id, .. } => write!(f, "Typed({id})"),
        }
    }
}

impl Act {
    /// The settings a `Keep` carries.
    #[must_use]
    pub fn of_settings(s: &Settings) -> Self {
        Self::Keep {
            salt: s.salt.clone(),
            rounds: s.rounds,
            images: s.images,
            notify: s.notify,
            cap: s.cap,
            mark: s.mark,
        }
    }

    /// The order line: `clipboard-act <generation> <json>`.
    ///
    /// # Errors
    ///
    /// When it cannot be serialised, which plain data never is.
    pub fn line(&self, generation: u64) -> serde_json::Result<String> {
        Ok(format!(
            "{ACT} {generation} {}",
            serde_json::to_string(self)?
        ))
    }

    /// An order line's rest — what follows [`ACT`] — read back.
    #[must_use]
    pub fn parse(rest: &str) -> Option<(u64, Self)> {
        let (generation, json) = rest.trim().split_once(' ')?;
        Some((generation.parse().ok()?, serde_json::from_str(json).ok()?))
    }
}

/// One history, as the footer hands it to its followers: cloned once, shared
/// by all of them, and turned into a line on each follower's own thread — the
/// base64 of a picture is never made on the footer's loop.
#[derive(Debug)]
pub struct Shot {
    /// The entries, newest first.
    pub entries: Vec<Entry>,
    /// ClipCascade's reading.
    pub sync: Snapshot,
    /// Which history.
    pub generation: u64,
    /// What the footer said in answer to a window's act (`Mirror::said`).
    pub said: Option<String>,
}

/// The footer's history as a REGION of the footer hears it (`Region::heard`)
/// — the footer's own queue, SHARED and never copied for the telling: the
/// footer copies it only to change it while a region still holds this one —
/// with ClipCascade's reading and which history it is.
#[derive(Debug, Clone)]
pub struct Held {
    /// The entries, newest first: the footer's queue itself.
    pub queue: std::rc::Rc<Queue>,
    /// ClipCascade's reading.
    pub sync: Snapshot,
    /// Which history.
    pub generation: u64,
}

/// A follower: the window's connection, written to on a thread of its own
/// until it closes. The sender is what the footer keeps; when the window
/// goes, the write fails, the thread ends, and the next send says so.
#[must_use]
pub fn follower(
    stream: std::sync::Arc<std::os::unix::net::UnixStream>,
) -> Option<std::sync::mpsc::Sender<std::sync::Arc<Shot>>> {
    use std::io::Write as _;
    let (tx, rx) = std::sync::mpsc::channel::<std::sync::Arc<Shot>>();
    let born = std::thread::Builder::new()
        .name("clipboard-out".to_owned())
        .spawn(move || {
            for shot in rx {
                let mut mirror = Mirror::of_parts(shot.entries.iter(), &shot.sync, shot.generation);
                mirror.said.clone_from(&shot.said);
                let line = mirror.line();
                let Ok(line) = line else { continue };
                // ONE WRITE, the whole line: a reader that pauses gets the
                // line or nothing, never half of one.
                if (&*stream).write_all(line.as_bytes()).is_err() {
                    return;
                }
            }
        });
    match born {
        Ok(_) => Some(tx),
        Err(e) => {
            tracing::warn!(error = %e, "could not start a clipboard follower's thread");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the window's history is not the footer's — an entry of any
    /// kind lost, reordered, or changed on the way.
    #[test]
    fn the_window_draws_the_footers_history_whole() {
        let mut q = Queue::default();
        q.register(Entry::of_image(
            vec![0x89, b'P', b'N', b'G', 1, 2, 3],
            "image/png",
        ));
        q.register(Entry {
            content: Content::Unreadable(ronoctua_clipboard::Undecodable {
                kind: "text/x-odd".into(),
                bytes: 12,
            }),
            kinds: vec!["text/x-odd".into()],
            pinned: false,
        });
        q.register(Entry::of_text("olá, mundo"));
        q.toggle_pinned(1);
        let sync = Snapshot {
            server: "https://clip.example.org".into(),
            rounds: 7,
            ..Snapshot::default()
        };
        let wire = Mirror::of(&q, &sync, 3).line().expect("a line");
        assert!(wire.ends_with('\n') && wire.matches('\n').count() == 1);
        let back: Mirror = serde_json::from_str(wire.trim_end()).expect("it reads back");
        assert_eq!(back.generation, 3);
        assert_eq!(back.queue().inputs(), q.inputs());
        assert_eq!(back.sync.snapshot(), sync);
        assert_eq!(back.said, None);
        // AND WHAT THE FOOTER SAID crosses with it, and an older line with no
        // such field still reads.
        let mut told = Mirror::of(&q, &sync, 4);
        told.said = Some("copied".into());
        let back: Mirror =
            serde_json::from_str(told.line().expect("a line").trim_end()).expect("it reads back");
        assert_eq!(back.said.as_deref(), Some("copied"));
        let mut old: serde_json::Value = serde_json::from_str(wire.trim_end()).expect("json");
        old.as_object_mut().expect("an object").remove("said");
        assert!(serde_json::from_value::<Mirror>(old).is_ok());
    }

    /// FAILS IF: an act does not come back as it went, or a login's password
    /// can reach a log line.
    #[test]
    fn an_act_crosses_and_its_password_is_never_printed() {
        for act in [
            Act::Copy { n: 2 },
            Act::Clear,
            Act::Sync { on: true },
            Act::LogIn {
                server: "https://clip.example.org".into(),
                username: "someone".into(),
                password: "s3nh4 secreta".into(),
            },
            Act::Typed {
                id: "clipboard-rounds".into(),
                text: "100000".into(),
            },
        ] {
            let line = act.line(9).expect("a line");
            let rest = line.strip_prefix(ACT).expect("the word");
            assert_eq!(Act::parse(rest), Some((9, act.clone())));
            assert!(!format!("{act:?}").contains("s3nh4"), "{act:?}");
        }
        assert_eq!(Act::parse("x {}"), None);
        assert_eq!(Act::parse("4 {\"do\":\"burn\"}"), None);
    }
}
