//! A terminal's whole state, as its holder writes it beside its socket.
//!
//! # Whole, every time
//!
//! The holder writes ALL of it on every change (`<id>.json`, atomically) and
//! sends all of it to its viewer: a reader that missed a version loses
//! nothing, because the next one says everything again. What is NOT in it is
//! the screen: a `cat` of a large file changes nothing here, so it wakes no
//! footer and no brain.
//!
//! # Later fields are defaults
//!
//! A field added after [`crate::PROTOCOL`] 1 is `serde(default)`, so a reader
//! of a newer holder (or a holder older than its reader, left running across
//! a switch) still reads the rest.
//!
//! # A program's text, bounded and plain
//!
//! A title, a notification and a status record are what a PROGRAM wrote.
//! They are kept as plain text ([`plain`]): no control character, nothing
//! that reorders the line around it (the bidirectional controls), and a
//! bound per field — the file is read by the footer on every change, and a
//! program that writes a megabyte of title must not make it a megabyte.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{GroupId, TerminalId};

/// The most characters a title keeps.
pub const TITLE_MAX_CHARS: usize = 256;

/// The most characters a notification's title or body keeps.
pub const NOTICE_MAX_CHARS: usize = 512;

/// A terminal, whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalState {
    /// The holder's [`crate::PROTOCOL`].
    pub protocol: u32,
    /// Which terminal.
    pub id: TerminalId,
    /// The group its window shows it in.
    pub group: GroupId,
    /// Its place in the group, from the left of the window's strip.
    pub order: u32,
    /// The process id of the window attached to it; `None` when it is in the
    /// background.
    #[serde(default)]
    pub viewer: Option<u32>,
    /// It is the one its window shows.
    #[serde(default)]
    pub front: bool,
    /// It is in front AND its window holds the keyboard: someone is looking.
    #[serde(default)]
    pub seen: bool,
    /// What the program called it (OSC 0/2), plain; empty if it said nothing.
    #[serde(default)]
    pub title: String,
    /// The program it was started with, by its file name (`zsh`, `claude`).
    pub program: String,
    /// Where it is: what the shell last reported (OSC 7), else where it
    /// started.
    #[serde(default, with = "raw_path::maybe")]
    pub cwd: Option<PathBuf>,
    /// When it started, in seconds since the epoch.
    pub started_s: i64,
    /// When its last window left it, in seconds since the epoch; `None`
    /// while one is attached.
    #[serde(default)]
    pub alone_since_s: Option<i64>,
    /// What it asks of the person since it was last seen.
    #[serde(default)]
    pub attention: Attention,
    /// The last desktop notification it asked for (OSC 9/777).
    #[serde(default)]
    pub notice: Option<Notice>,
    /// The last command the shell ran in it (OSC 133).
    #[serde(default)]
    pub command: Option<CommandMark>,
    /// How many times something asked for it to be put in front: its viewer
    /// fronts it whenever this moves.
    #[serde(default)]
    pub front_asked: u64,
    /// The program ended: the holder stays until a window has shown it once.
    #[serde(default)]
    pub ended: Option<Ended>,
    /// What programs in it report about themselves (OSC 7501), least recently
    /// reported first.
    #[serde(default)]
    pub statuses: Vec<StatusRecord>,
    /// A progress report (OSC 9;4).
    #[serde(default)]
    pub progress: Option<Progress>,
}

impl TerminalState {
    /// In the background: no window shows it, and its program still runs.
    #[must_use]
    pub const fn away(&self) -> bool {
        self.viewer.is_none() && self.ended.is_none()
    }

    /// It asks for the person: a bell, a notification or a program waiting
    /// on them, since it was last seen.
    #[must_use]
    pub const fn asks_attention(&self) -> bool {
        self.attention.bell > 0 || self.attention.notice > 0 || self.attention.waiting
    }

    /// The state as one line of JSON, newline included — what the file holds
    /// and what a watcher reads, built whole before it is written once.
    ///
    /// # Errors
    ///
    /// Serialisation failed, which a state made of strings and numbers does
    /// not.
    pub fn line(&self) -> serde_json::Result<Vec<u8>> {
        let mut out = serde_json::to_vec(self)?;
        out.push(b'\n');
        Ok(out)
    }
}

/// What a terminal asks of the person since it was last seen — cleared when
/// it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Attention {
    /// Bells rung while nobody was looking.
    pub bell: u64,
    /// Notifications asked for while nobody was looking.
    pub notice: u64,
    /// A program in it became blocked on the person while nobody was looking,
    /// and still is.
    pub waiting: bool,
}

/// A desktop notification a program asked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    /// Its title, plain; empty when the protocol has none (OSC 9).
    #[serde(default)]
    pub title: String,
    /// Its body, plain.
    #[serde(default)]
    pub body: String,
    /// Counts every notification of this terminal: a reader that showed
    /// generation N does not show it again after a reconnection.
    pub generation: u64,
}

/// The last command a shell reported (OSC 133).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandMark {
    /// When it started, in seconds since the epoch.
    pub since_s: i64,
    /// It still runs.
    pub running: bool,
    /// How it ended, when the shell said.
    #[serde(default)]
    pub last_exit: Option<i32>,
    /// How long it ran, once it ended.
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

/// How a terminal's program ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ended {
    /// Its exit code, a shell's way: the status it exited with, or 128 plus
    /// the signal that killed it.
    pub code: i32,
    /// When, in seconds since the epoch.
    pub at_s: i64,
}

/// What a program says it is doing (OSC 7501).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProgramState {
    /// At rest, waiting for the next instruction.
    Idle,
    /// Running on its own.
    Working,
    /// Finished; the result is ready to be looked at.
    Done,
    /// It cannot go on until the person does something.
    Blocked,
    /// It failed and stopped.
    Error,
}

/// What a blocked program needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockedOn {
    /// An approval.
    Permission,
    /// An answer to type.
    Question,
    /// A login, a password, a token.
    Auth,
}

/// One record of a program's status (OSC 7501), kept by the specification's
/// rules (`ronoctua-term`'s `status.rs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusRecord {
    /// Which record; empty for the program itself, `/` makes a child
    /// (an agent's subagents are `<agent>/<sub>`).
    pub id: String,
    /// A stable name a machine can match on (`claude-code`).
    #[serde(default)]
    pub app: String,
    /// What it is doing.
    pub state: ProgramState,
    /// What it needs, when blocked.
    #[serde(default)]
    pub kind: Option<BlockedOn>,
    /// A short label for people, plain.
    #[serde(default)]
    pub title: String,
    /// One line for people, plain.
    #[serde(default)]
    pub msg: String,
    /// How far along, 0–100, when it said.
    #[serde(default)]
    pub percent: Option<u8>,
    /// When it was reported, in milliseconds since the epoch.
    pub at_ms: u64,
}

/// A progress report's state (OSC 9;4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProgressState {
    /// Determinate.
    Set,
    /// The work failed.
    Error,
    /// Busy, with no percentage.
    Indeterminate,
    /// Paused.
    Pause,
}

/// A progress report (OSC 9;4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    /// Its state.
    pub state: ProgressState,
    /// The percentage, when it said.
    #[serde(default)]
    pub percent: Option<u8>,
}

/// Whether `c` reorders the text around it: the bidirectional embeddings,
/// overrides, isolates and marks. A title that carried one could make the
/// rest of a line it is drawn in read backwards.
const fn reorders(c: char) -> bool {
    matches!(
        c,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

/// A program's text as the state keeps it: no control character, no
/// bidirectional control, at most `max_chars` characters.
///
/// The bound is the FILE's, not a drawing's: whoever draws it cuts it again,
/// in graphemes, to what fits. What is cut here at a character boundary does
/// not leave a joiner or a variation selector dangling at the end, which
/// would join the text to whatever is drawn after it.
#[must_use]
pub fn plain(text: &str, max_chars: usize) -> String {
    let mut out: String = text
        .chars()
        .filter(|c| !c.is_control() && !reorders(*c))
        .take(max_chars)
        .collect();
    while out.ends_with(['\u{200D}', '\u{FE0E}', '\u{FE0F}']) {
        out.pop();
    }
    out
}

/// A path as JSON: text when it is text, bytes when it is not.
///
/// On Linux a directory's name is bytes, and `serde_json` refuses a
/// `PathBuf` that is not UTF-8 — so a terminal whose shell stood in such a
/// folder would have no state file at all. The usual case stays readable.
pub(crate) mod raw_path {
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    /// One path, either way.
    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Text(String),
        Bytes(Vec<u8>),
    }

    impl Raw {
        fn of(p: &std::path::Path) -> Self {
            use std::os::unix::ffi::OsStrExt as _;
            p.to_str().map_or_else(
                || Self::Bytes(p.as_os_str().as_bytes().to_vec()),
                |t| Self::Text(t.to_owned()),
            )
        }

        fn path(self) -> PathBuf {
            use std::os::unix::ffi::OsStringExt as _;
            match self {
                Self::Text(t) => PathBuf::from(t),
                Self::Bytes(b) => PathBuf::from(std::ffi::OsString::from_vec(b)),
            }
        }
    }

    /// An optional path, through [`Raw`].
    pub(crate) mod maybe {
        use serde::{Deserialize as _, Deserializer, Serialize as _, Serializer};

        use super::{PathBuf, Raw};

        #[allow(clippy::ref_option)]
        pub(crate) fn serialize<S: Serializer>(
            p: &Option<PathBuf>,
            s: S,
        ) -> Result<S::Ok, S::Error> {
            p.as_deref().map(Raw::of).serialize(s)
        }

        pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
            d: D,
        ) -> Result<Option<PathBuf>, D::Error> {
            Ok(Option::<Raw>::deserialize(d)?.map(Raw::path))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A state with every field set, for the round trips.
    pub(crate) fn full() -> TerminalState {
        TerminalState {
            protocol: crate::PROTOCOL,
            id: "0123456789abcdef".parse().unwrap(),
            group: "fedcba9876543210".parse().unwrap(),
            order: 2,
            viewer: Some(4242),
            front: true,
            seen: false,
            title: "vim notes.md".to_owned(),
            program: "zsh".to_owned(),
            cwd: Some(PathBuf::from("/home/someone/notes")),
            started_s: 1_790_000_000,
            alone_since_s: None,
            attention: Attention {
                bell: 1,
                notice: 2,
                waiting: true,
            },
            notice: Some(Notice {
                title: "build".to_owned(),
                body: "finished".to_owned(),
                generation: 7,
            }),
            command: Some(CommandMark {
                since_s: 1_790_000_100,
                running: false,
                last_exit: Some(1),
                duration_ms: Some(2500),
            }),
            front_asked: 3,
            ended: Some(Ended {
                code: 130,
                at_s: 1_790_000_200,
            }),
            statuses: vec![StatusRecord {
                id: "agent/sub".to_owned(),
                app: "claude-code".to_owned(),
                state: ProgramState::Blocked,
                kind: Some(BlockedOn::Permission),
                title: "Edit".to_owned(),
                msg: "Apply these changes?".to_owned(),
                percent: Some(40),
                at_ms: 1_790_000_150_000,
            }],
            progress: Some(Progress {
                state: ProgressState::Set,
                percent: Some(60),
            }),
        }
    }

    #[test]
    fn a_state_survives_its_own_file() {
        // FAILS IF a field is written in a form the reader does not read
        // back — the footer would see a terminal other than the one there is.
        let state = full();
        let line = state.line().unwrap();
        assert_eq!(line.last(), Some(&b'\n'), "one line");
        let back: TerminalState = serde_json::from_slice(&line).unwrap();
        assert_eq!(back, state);
    }

    #[test]
    fn a_state_of_the_first_protocol_reads_with_the_later_fields_empty() {
        // FAILS IF a field added later is required: a footer newer than a
        // holder left running across a switch would lose the terminal.
        let first = r#"{"protocol":1,"id":"0123456789abcdef","group":"fedcba9876543210",
            "order":0,"program":"zsh","started_s":5}"#;
        let state: TerminalState = serde_json::from_str(first).unwrap();
        assert!(state.away(), "no viewer and not ended is the background");
        assert!(state.statuses.is_empty() && state.cwd.is_none());
        assert!(!state.asks_attention());
    }

    #[test]
    fn a_folder_whose_name_is_not_text_is_kept() {
        // FAILS IF a cwd that is not UTF-8 makes the state unwritable: the
        // terminal would have no file, and no census would see it.
        use std::os::unix::ffi::OsStrExt as _;
        let mut state = full();
        state.cwd = Some(PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/\xff\xfe")));
        let back: TerminalState = serde_json::from_slice(&state.line().unwrap()).unwrap();
        assert_eq!(back.cwd, state.cwd);
    }

    #[test]
    fn a_programs_text_is_kept_plain_and_bounded() {
        // FAILS IF a control character, a bidirectional override or an
        // unbounded title reaches the file the footer reads on every change.
        assert_eq!(plain("a\u{1b}[31mb\u{7}c", 64), "a[31mbc");
        assert_eq!(plain("x\u{202E}gnp.exe\u{2066}", 64), "xgnp.exe");
        assert_eq!(plain(&"é".repeat(300), 256).chars().count(), 256);
        // A cut that would leave a joiner at the end drops it.
        assert_eq!(plain("ab\u{200D}cd", 3), "ab");
    }
}
