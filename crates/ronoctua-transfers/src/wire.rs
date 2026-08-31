//! The socket's vocabulary, and the one thing on it that is not text.
//!
//! # Why JSON, when the footer's own socket is words and spaces
//!
//! Because what crosses here is PATHS and, later, URLs. A path may contain a
//! space, a colon, a newline and bytes that are not text at all, and a
//! whitespace grammar loses half of one without saying anything — which is
//! exactly the defect `leader.rs`'s `rsplit_once(' ')` is a patch for, written
//! after a sequence and a command id came apart in the wrong place.
//!
//! `serde_json` is already in the workspace, and one object per line keeps the
//! stream readable with `socat` while it is being built.
//!
//! **Invariant 11 is untouched.** The JSON dies at the host: a wasm buffer
//! receives typed records through the WIT and has no parser, exactly as before.
//!
//! # Why a path is not simply a string
//!
//! Because on Linux a filename is a bag of bytes, not text. `serde_json`
//! serialises a `PathBuf` through `to_str()` and **fails** on one that is not
//! UTF-8 — so a file manager that carried paths as strings would refuse to copy
//! a file whose name came off a Windows disk, and would refuse it at the socket,
//! far from the person who asked.
//!
//! So a path goes as text when it IS text and as bytes when it is not. The
//! normal case stays readable; the odd one stays correct.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::job::{Job, Work};

/// A path on the wire: text when it can be, bytes when it cannot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum Raw {
    /// The usual case, and the one a person can read in a log.
    Text(String),
    /// A name that is not UTF-8. It still copies.
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
}

impl From<Raw> for PathBuf {
    fn from(r: Raw) -> Self {
        use std::os::unix::ffi::OsStringExt as _;
        match r {
            Raw::Text(t) => Self::from(t),
            Raw::Bytes(b) => Self::from(std::ffi::OsString::from_vec(b)),
        }
    }
}

/// One path, through [`Raw`]. Named in `#[serde(with = ...)]`.
pub(crate) mod path {
    use super::{PathBuf, Raw};
    use serde::{Deserialize as _, Deserializer, Serialize as _, Serializer};

    pub(crate) fn serialize<S: Serializer>(p: &std::path::Path, s: S) -> Result<S::Ok, S::Error> {
        Raw::of(p).serialize(s)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<PathBuf, D::Error> {
        Ok(Raw::deserialize(d)?.into())
    }
}

/// A list of paths, through [`Raw`].
pub(crate) mod paths {
    use super::{PathBuf, Raw};
    use serde::{Deserialize as _, Deserializer, Serialize as _, Serializer};

    pub(crate) fn serialize<S: Serializer>(p: &[PathBuf], s: S) -> Result<S::Ok, S::Error> {
        p.iter()
            .map(|one| Raw::of(one))
            .collect::<Vec<_>>()
            .serialize(s)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<PathBuf>, D::Error> {
        Ok(Vec::<Raw>::deserialize(d)?
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }
}

/// What a job acts on, for the sentence the row draws.
///
/// Three shapes because there genuinely are three: a set of paths, a set of
/// names inside somewhere already known (the trash, an archive), and a single
/// thing. Flattening them into one `Vec<PathBuf>` would mean inventing paths
/// for the names, and an invented path is one somebody can click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paths<'a> {
    /// A set of full paths.
    Some(&'a [PathBuf]),
    /// A set of names, relative to a root the work already carries.
    Named(&'a [String]),
    /// Exactly one.
    One(&'a std::path::Path),
    /// Not a path at all — a URL, or a magnet.
    ///
    /// A case of its own rather than a `PathBuf` holding text: a URL has a
    /// grammar, and the path-or-bytes encoding on the wire would happily carry
    /// a byte sequence that is not one. Here the type says which it is.
    Text(&'a str),
}

impl Paths<'_> {
    /// The name to show when there is room for one. The FIRST, because a set
    /// is drawn as *"`nome` e mais 5"* and the first is the one the person
    /// clicked.
    #[must_use]
    pub fn first(&self) -> String {
        let borrowed = match self {
            Self::Some(p) => p.first().map(|one| one.as_path()),
            Self::Named(n) => return n.first().cloned().unwrap_or_default(),
            Self::One(p) => Some(*p),
            // The whole thing, uncut: a URL's last segment is often a token or
            // an empty string, and `file_name` on it would print neither the
            // site nor the file.
            Self::Text(t) => return (*t).to_owned(),
        };
        borrowed
            .and_then(std::path::Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// What a client asks the daemon for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "ask")]
pub enum Ask {
    /// The whole list, once, and then the connection closes.
    List,
    /// The whole list, and then every change until the connection drops.
    ///
    /// It is how the modeline stays honest without polling: the footer holds
    /// one of these open on a thread and pushes what arrives into `calloop`,
    /// exactly as it already does with niri's event stream.
    Watch,
    /// Take this on.
    Add {
        /// What to do.
        work: Work,
    },
    /// Hold that one.
    Pause {
        /// Which.
        id: u64,
    },
    /// Let that one go on.
    Resume {
        /// Which.
        id: u64,
    },
    /// Give that one up.
    Cancel {
        /// Which.
        id: u64,
    },
    /// Take that finished one off the list.
    Forget {
        /// Which.
        id: u64,
    },
    /// Take every finished one off the list.
    Sweep,
}

/// What the daemon says back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "say")]
pub enum Say {
    /// The list, whole. Whole and not a delta, on purpose: a delta needs both
    /// sides to agree on what they last saw, and a client that missed one
    /// would draw a wrong number forever with nothing to notice it by.
    Ledger {
        /// Every job the daemon knows, running and finished.
        jobs: Vec<Job>,
    },
    /// The id the new job got, so the asker can name it later.
    Took {
        /// Its number.
        id: u64,
    },
    /// The ask made no sense, and this is why.
    Refused {
        /// In the daemon's own words.
        why: String,
    },
}

/// Where the socket lives.
///
/// # Why one per USER and not one per display
///
/// The footer's control socket carries the display in its name so that a
/// nested `nix run .#dev` does not send keys to the real session. This one is
/// the opposite case: a copy is a copy of the same person's files onto the same
/// disk, whichever compositor happens to be on screen. Two queues would mean
/// two answers to "what is my machine doing", and the second one would be
/// invisible from the first.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent. There is no fallback to `/tmp`, for the reason
/// `leader::socket_path` already gives: a control socket in a shared directory
/// is attack surface.
pub fn socket_path() -> Result<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .context("XDG_RUNTIME_DIR is not set; without it there is nowhere to put the socket")?;
    Ok(PathBuf::from(dir).join("ronoctua-transfers.sock"))
}

/// Writes one object and the newline that ends it.
///
/// # Errors
///
/// When the value cannot be encoded, or the other side has gone.
pub fn write_line<T: Serialize>(to: &mut impl Write, value: &T) -> Result<()> {
    let mut line = serde_json::to_string(value).context("encode the line")?;
    line.push('\n');
    to.write_all(line.as_bytes())
        .context("write to the transfers socket")?;
    to.flush().context("drain the transfer queue's socket")
}

/// Reads one object. `None` when the other side closed cleanly.
///
/// # Errors
///
/// When the read fails or the line is not the expected shape.
pub fn read_line<T: DeserializeOwned>(from: &mut impl BufRead) -> Result<Option<T>> {
    let mut line = String::new();
    if from
        .read_line(&mut line)
        .context("read from the transfers socket")?
        == 0
    {
        return Ok(None);
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .with_context(|| format!("a line I did not understand: {trimmed}"))
        .map(Some)
}

/// A job list, whole, as [`Say::Ledger`] carries it.
pub type Jobs = Vec<Job>;
