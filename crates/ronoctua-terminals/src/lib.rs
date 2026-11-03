//! The held terminals, without their engine.
//!
//! # One process per terminal, and this is everything around it
//!
//! A terminal lives in a HOLDER (`ronoctua-term-holder serve`, in
//! `ronoctua-term`): the PTY, the program and the authoritative engine, in a
//! transient scope of its own, so it outlives the window that shows it, the
//! footer and a `switch`. A window is a VIEWER: it attaches, decodes a
//! snapshot into a mirror and follows the bytes the holder's engine is fed.
//!
//! This crate is what everybody else needs of that, and NOTHING of the
//! engine — the footer counts the terminals nobody is looking at, the brain
//! reads what an agent inside one reports, the view lists them, and none of
//! them may link `libghostty-vt` for it:
//!
//! - [`wire`]: the one table of the holder's socket — the first line of a
//!   connection ([`wire::Ask`]) and the frames that follow an attach;
//! - [`state`]: a terminal's WHOLE state ([`state::TerminalState`]), which the
//!   holder writes beside its socket on every change;
//! - [`registry`]: where those live (`$XDG_RUNTIME_DIR/<name>-terms/`), the
//!   census over them, and the client's few questions;
//! - [`spawn`]: starting a holder in its own scope, as data first.
//!
//! # The file is the broadcast
//!
//! Each holder keeps `<id>.json` beside `<id>.sock`, written whole and
//! atomically whenever its state changes — never per byte of screen output.
//! A reader watches the directory (inotify) and re-reads the file that moved:
//! no thread per holder, no stream to keep, and a reader that starts late
//! reads the same thing one that never stopped did. The socket is for what a
//! file cannot do: attach, and the few orders (front, end).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::fmt;

use serde::{Deserialize, Serialize};

pub mod registry;
pub mod spawn;
pub mod state;
pub mod wire;

/// The version of the holder's wire and of its state file.
///
/// Carried in the state ([`state::TerminalState::protocol`]) and in every
/// attach: a viewer of another protocol is told so in the first frame, whose
/// shape never changes, and leaves. A field added later is `serde(default)`
/// and does not move this number; a field whose MEANING changes does.
pub const PROTOCOL: u32 = 1;

/// How many hex digits an id has.
const ID_DIGITS: usize = 16;

/// Whether `text` is an id: sixteen lowercase hex digits, nothing else.
///
/// Strict on purpose: an id becomes a file name and a unit name, and one
/// that passed with a `/` or a `..` in it would name another file.
fn an_id(text: &str) -> bool {
    text.len() == ID_DIGITS
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Sixteen random hex digits, from the kernel.
///
/// `/dev/urandom` and not a crate: an id needs to be unique, not secret (the
/// registry is `0700`), and the kernel already offers the bytes.
fn random_id() -> std::io::Result<String> {
    use std::io::Read as _;
    let mut bytes = [0u8; ID_DIGITS / 2];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Why a text is not an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotAnId(pub String);

impl fmt::Display for NotAnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not sixteen lowercase hex digits", self.0)
    }
}

impl std::error::Error for NotAnId {}

macro_rules! an_id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// A fresh one, from the kernel's randomness.
            ///
            /// # Errors
            ///
            /// `/dev/urandom` could not be read.
            pub fn fresh() -> std::io::Result<Self> {
                random_id().map(Self)
            }

            /// The id, as its sixteen digits.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = NotAnId;
            fn try_from(text: String) -> Result<Self, NotAnId> {
                if an_id(&text) { Ok(Self(text)) } else { Err(NotAnId(text)) }
            }
        }

        impl std::str::FromStr for $name {
            type Err = NotAnId;
            fn from_str(text: &str) -> Result<Self, NotAnId> {
                Self::try_from(text.to_owned())
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

an_id_type! {
    /// A terminal: one holder, one socket, one state file, one scope.
    ///
    /// The same sixteen digits name all four, and the program in the terminal
    /// is told them as `RONOCTUA_TERMINAL` — which is how an agent's hook
    /// says which terminal it runs in.
    TerminalId
}

an_id_type! {
    /// A group: the terminals one window shows, one in front. It exists only
    /// while some holder carries it.
    GroupId
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_sixteen_lowercase_hex_digits_and_nothing_else() {
        // FAILS IF an id that names another file passes: it becomes
        // `<id>.sock`, `<id>.json` and a unit's name.
        assert!("0123456789abcdef".parse::<TerminalId>().is_ok());
        for bad in [
            "",
            "0123456789ABCDEF",
            "0123456789abcde",
            "0123456789abcdef0",
            "../456789abcdef0",
            "0123456789abcdeg",
        ] {
            assert!(bad.parse::<TerminalId>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_fresh_id_is_an_id_and_two_differ() {
        // FAILS IF the generator writes something the parser refuses (every
        // terminal would be refused by its own census), or a constant.
        let a = TerminalId::fresh().unwrap();
        let b = TerminalId::fresh().unwrap();
        assert!(a.as_str().parse::<TerminalId>().is_ok(), "{a}");
        assert_ne!(a, b);
    }

    #[test]
    fn an_id_on_the_wire_is_checked_as_it_is_read() {
        // FAILS IF a file or a frame can carry an id the type would refuse:
        // the census names files by it.
        assert!(serde_json::from_str::<TerminalId>("\"0123456789abcdef\"").is_ok());
        assert!(serde_json::from_str::<GroupId>("\"../etc/passwd\"").is_err());
    }
}
