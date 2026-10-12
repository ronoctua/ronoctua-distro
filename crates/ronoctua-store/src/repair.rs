//! The repair ledger: what a lenient read changed in a person's file to read
//! it, said where a screen can show it — and the two answers a person gives.
//!
//! # What a repair is
//!
//! A read that met a value it could not take — a variant a newer machine
//! wrote, a number where a word belongs, an element of a list that is not
//! one — keeps the rest of the file and puts something it CAN take in that
//! one place: the type's default there, or nothing. Each such change is a
//! [`Repair`]: where ([`Repair::pointer`]), what the file said
//! ([`Repair::found`]), what was read instead ([`Repair::kept`]) and why.
//!
//! # The ledger is a file, and the file is the broadcast
//!
//! `$XDG_STATE_HOME/ronoctua/repairs/<escaped logical path>.json`, one per
//! person's file that has repairs, written by whichever process read it and
//! DELETED when a read finds nothing to repair. The view that lists them
//! watches the folder, as every door watches what it shows. The name is the
//! file's LOGICAL path — `config/theme.json` — the same name the belongings
//! give it ([`file_name`], [`logical_of`]).
//!
//! # A repair is never written unasked
//!
//! Reading changes nothing on disk but this ledger. A WRITE of the store puts
//! every repair back where the writer left the value as the read gave it
//! (`crate::Store::write`), so a machine that does not know a newer value
//! never erases it by saving something else. Only [`apply`] writes a repair
//! into the file — the person's "write the fix", or the machine that runs the
//! newest distro of the person's machines doing it for them — and only while
//! the file is the one the ledger judged ([`Ledger::file_sha256`]).
//! [`dismiss`] is "leave it": the ledger stays, marked, until the file moves.
//!
//! A value that was RENAMED is not a repair: it is a `#[serde(alias)]` on the
//! type, read as itself.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use crate::json::{self, Node};

/// The ledger's folder, under the state root.
pub const DIR: &str = "repairs";

/// Why a value was repaired — a closed set, each a sentence on the screen that
/// lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// A word this system does not know where it knows a few — most often a
    /// newer machine's.
    UnknownVariant,
    /// A value of the wrong kind: a word for a number, a list for a word.
    WrongType,
    /// A value the type needs was not there.
    Missing,
    /// A value of the right kind that is not allowed: out of range, refused
    /// by the file's own check.
    Invalid,
}

impl Why {
    /// Every one, for the census of their sentences.
    pub const ALL: [Self; 4] = [
        Self::UnknownVariant,
        Self::WrongType,
        Self::Missing,
        Self::Invalid,
    ];

    /// Its kebab-case name — the wire's, and the suffix of its sentence.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::UnknownVariant => "unknown-variant",
            Self::WrongType => "wrong-type",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
        }
    }
}

/// One change a read made to take the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repair {
    /// Where, as an RFC 6901 pointer into the document AS IT WAS when this
    /// repair was made: the repairs of one read are a SEQUENCE, and an element
    /// dropped from a list moves the ones after it.
    pub pointer: String,
    /// What the file said there (`null` for [`Why::Missing`]: nothing).
    pub found: Node,
    /// What was read instead; `None` when the value or element was taken out.
    pub kept: Option<Node>,
    /// Why.
    pub why: Why,
}

impl Repair {
    /// Makes the change in `doc` — what the read did, and what [`apply`]
    /// does to the file. `false` when the place is not there any more.
    pub fn redo(&self, doc: &mut Node) -> bool {
        match &self.kept {
            Some(kept) => doc.put(&self.pointer, kept.clone()),
            None => doc.take(&self.pointer).is_some(),
        }
    }

    /// Reverses the change in `doc` — the document the READ repaired, at the
    /// step this repair was made — when what is there is still what the read
    /// put. What folds the repairs below a climbed one into it
    /// (`lenient::Mender::unwind`); a WRITE judges against the value read
    /// instead (`lenient::kept_over`), because a type writes its own default
    /// where the read took a value out.
    pub(crate) fn undo(&self, doc: &mut Node) -> bool {
        let in_an_array = json::split(&self.pointer)
            .and_then(|(parent, _)| doc.at(&parent).map(|p| matches!(p, Node::Array(_))));
        match (&self.kept, self.why) {
            // THE WHOLE DOCUMENT was replaced: only an untouched default
            // gives the file back.
            _ if self.pointer.is_empty() => {
                if self.kept.as_ref() == Some(&*doc) {
                    *doc = self.found.clone();
                    true
                } else {
                    false
                }
            }
            // AN ELEMENT DROPPED from a list goes back where it was: the
            // writer never saw it, so it cannot have meant to remove it.
            (None, _) if in_an_array == Some(true) => doc.insert(&self.pointer, self.found.clone()),
            // A FIELD TAKEN OUT goes back unless the writer wrote one.
            (None, _) => {
                if doc.at(&self.pointer).is_some() {
                    false
                } else {
                    doc.put(&self.pointer, self.found.clone())
                }
            }
            // A FIELD FILLED IN goes away again if it is still the filling.
            (Some(kept), Why::Missing) => {
                if doc.at(&self.pointer) == Some(kept) {
                    doc.take(&self.pointer).is_some()
                } else {
                    false
                }
            }
            // A VALUE REPLACED comes back if it is still the replacement.
            (Some(kept), _) => {
                if doc.at(&self.pointer) == Some(kept) {
                    doc.put(&self.pointer, self.found.clone())
                } else {
                    false
                }
            }
        }
    }
}

/// What one file's ledger holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    /// The file's logical path (`config/theme.json`).
    pub path: String,
    /// The SHA-256 of the file's bytes that were judged, hexadecimal: a ledger
    /// whose file moved since is stale, and nothing is applied from it.
    pub file_sha256: String,
    /// The repairs, in the order they were made.
    pub repairs: Vec<Repair>,
    /// The person said "leave it" ([`dismiss`]) about THIS content of the
    /// file; a read of the same bytes keeps it said.
    #[serde(default)]
    pub dismissed: bool,
}

/// The ledger has gone stale: the file is not the one it judged any more (or
/// it is gone, or it is not JSON). Nothing was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stale;

impl std::fmt::Display for Stale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the file changed since its repairs were listed")
    }
}

impl std::error::Error for Stale {}

/// The SHA-256 of `bytes`, hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = sha2::Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// The ledger's file name for a logical path: `/` and `%` escaped, so the
/// whole path is one name, and `.json` after it.
#[must_use]
pub fn file_name(logical: &str) -> String {
    let mut out = String::with_capacity(logical.len() + 8);
    for c in logical.chars() {
        match c {
            '%' => out.push_str("%25"),
            '/' => out.push_str("%2F"),
            c => out.push(c),
        }
    }
    out.push_str(".json");
    out
}

/// The logical path a ledger's file name stands for; `None` for a name that
/// is not one ([`file_name`]'s inverse).
#[must_use]
pub fn logical_of(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".json")?;
    let mut out = String::with_capacity(stem.len());
    let mut rest = stem;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let code = rest.get(i + 1..i + 3)?;
        out.push(match code {
            "25" => '%',
            "2F" => '/',
            _ => return None,
        });
        rest = &rest[i + 3..];
    }
    out.push_str(rest);
    (!out.is_empty()).then_some(out)
}

/// The ledger of `logical` in the folder `dir`, if it has one that reads.
#[must_use]
pub fn read(dir: &Path, logical: &str) -> Option<Ledger> {
    let bytes = std::fs::read(dir.join(file_name(logical))).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Every ledger in the folder `dir`, by logical path.
#[must_use]
pub fn all(dir: &Path) -> Vec<Ledger> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Ledger> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name();
            let logical = logical_of(name.to_str()?)?;
            read(dir, &logical).filter(|l| l.path == logical)
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// The ledgers still waiting for an answer: not dismissed.
#[must_use]
pub fn pending(dir: &Path) -> Vec<Ledger> {
    all(dir).into_iter().filter(|l| !l.dismissed).collect()
}

/// Writes `ledger` as the ledger of its path — or, with no repairs, removes
/// the one there. Unchanged bytes are not written: the ledger is read by
/// every reader of the store, and a rename per read would wake every door
/// watching the folder for nothing.
///
/// # Private from the first byte
///
/// A ledger holds VALUES of the file it judged — an element dropped whole, a
/// whole document replaced — and that file may hold a secret: the player's
/// pairing keys are kept `0600` because every key in them lets a machine in.
/// The ledger cannot know what the file was worth, so it is never readable
/// by anybody but its person: the folder `0700`, each ledger a draft created
/// `0600` and renamed over it ([`write_private`]), never written and then
/// narrowed.
pub(crate) fn note(dir: &Path, logical: &str, ledger: Option<&Ledger>) {
    let path = dir.join(file_name(logical));
    let Some(ledger) = ledger.filter(|l| !l.repairs.is_empty()) else {
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::info!(file = logical, "nothing left to repair; its ledger is gone"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(file = logical, error = %e, "could not remove a ledger"),
        }
        return;
    };
    let Ok(bytes) = serde_json::to_vec_pretty(ledger) else {
        return;
    };
    if std::fs::read(&path).is_ok_and(|had| had == bytes) {
        return;
    }
    if let Err(e) = write_private(dir, &file_name(logical), &bytes) {
        tracing::warn!(file = logical, error = %e, "could not write the repair ledger");
    }
}

/// `bytes` as `dir/name`, private from the first byte: the folder made
/// `0700` (and narrowed to it if it was wider), the file through
/// [`write_with_mode`] at `0600`.
fn write_private(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    // A FOLDER MADE WIDER before — by hand, or by a build that wrote ledgers
    // with the ordinary atomic write — is narrowed, not trusted.
    if std::fs::metadata(dir)?.permissions().mode() & 0o077 != 0 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    write_with_mode(dir, name, bytes, 0o600)
}

/// `bytes` as `dir/name` with exactly `mode`, atomically: a draft created
/// `0600` (`create_new`), given `mode` BEFORE a byte is in it, written and
/// renamed over the file — at no moment are the bytes under a wider mode
/// than the file's. The draft is `<name>.<pid>.new`, the shape the
/// belongings never keep.
fn write_with_mode(dir: &Path, name: &str, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    let draft = dir.join(format!("{name}.{}.new", std::process::id()));
    // A DRAFT A DEAD PROCESS OF THIS PID LEFT is half of something, under a
    // mode nobody checked: gone, so `create_new` makes a fresh one.
    let _ = std::fs::remove_file(&draft);
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&draft)
        .and_then(|mut f| {
            f.set_permissions(std::fs::Permissions::from_mode(mode))?;
            f.write_all(bytes)
        });
    let renamed = written.and_then(|()| std::fs::rename(&draft, dir.join(name)));
    if renamed.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    renamed
}

/// The bytes of `file`, if they are still the ones `ledger` judged.
fn judged(file: &Path, ledger: &Ledger) -> Result<Vec<u8>, Stale> {
    let bytes = std::fs::read(file).map_err(|_| Stale)?;
    if sha256_hex(&bytes) == ledger.file_sha256 {
        Ok(bytes)
    } else {
        Err(Stale)
    }
}

/// "Write the fix": makes every repair of `ledger` in `file` — the same
/// changes, in the same order, as the read made — writes it atomically, keeps
/// its key order and its MODE (a `0600` file stays `0600` from the draft's
/// first byte), and removes the ledger from `dir` (nothing is left to
/// repair).
///
/// # Errors
///
/// [`Stale`]: the file is not the one the ledger judged — moved, gone, or
/// not JSON. Nothing was written; the next read writes a fresh ledger.
pub fn apply(dir: &Path, file: &Path, ledger: &Ledger) -> Result<(), Stale> {
    use std::os::unix::fs::PermissionsExt as _;
    let bytes = judged(file, ledger)?;
    let mode = std::fs::metadata(file)
        .map_err(|_| Stale)?
        .permissions()
        .mode()
        & 0o7777;
    let mut doc = Node::parse(&bytes).map_err(|_| Stale)?;
    for repair in &ledger.repairs {
        repair.redo(&mut doc);
    }
    let (Some(folder), Some(name)) = (file.parent(), file.file_name().and_then(|n| n.to_str()))
    else {
        return Err(Stale);
    };
    write_with_mode(folder, name, &doc.pretty(), mode).map_err(|e| {
        tracing::warn!(file = %file.display(), error = %e, "could not write the repaired file");
        Stale
    })?;
    tracing::info!(
        file = ledger.path,
        repairs = ledger.repairs.len(),
        "the repairs were written"
    );
    note(dir, &ledger.path, None);
    Ok(())
}

/// "Leave it": the ledger stays, marked dismissed, for as long as the file is
/// the one it judged — the screen stops asking. A change of the file makes a
/// fresh ledger, asking again.
///
/// # Errors
///
/// [`Stale`]: the file moved since the ledger was written.
pub fn dismiss(dir: &Path, file: &Path, ledger: &Ledger) -> Result<(), Stale> {
    judged(file, ledger)?;
    let marked = Ledger {
        dismissed: true,
        ..ledger.clone()
    };
    note(dir, &ledger.path, Some(&marked));
    Ok(())
}

/// The ledger folder under a state root.
#[must_use]
pub fn dir_under(state: &Path) -> PathBuf {
    state.join(DIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-repair-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_ledger_is_named_by_its_logical_path_and_named_back() {
        for logical in [
            "config/theme.json",
            "config/brain/reminders/reminders.json",
            "state/100%/x.json",
        ] {
            let name = file_name(logical);
            assert!(!name.contains('/'), "{name}");
            assert_eq!(logical_of(&name).as_deref(), Some(logical));
        }
        assert_eq!(logical_of("config%2Ftheme"), None, "no .json");
        assert_eq!(logical_of("bad%zz.json"), None);
        for why in Why::ALL {
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
    }

    #[test]
    fn the_fix_is_written_only_over_the_file_it_judged_and_leaving_it_is_remembered() {
        let root = scratch("apply");
        let book = root.join("repairs");
        let file = root.join("theme.json");
        let text = br#"{"theme": "owl", "borders": "Discreet", "list": [1, "x", 3]}"#;
        std::fs::write(&file, text).unwrap();
        let ledger = Ledger {
            path: "config/theme.json".to_owned(),
            file_sha256: sha256_hex(text),
            repairs: vec![
                Repair {
                    pointer: "/borders".to_owned(),
                    found: Node::String("Discreet".to_owned()),
                    kept: Some(Node::String("standard".to_owned())),
                    why: Why::UnknownVariant,
                },
                Repair {
                    pointer: "/list/1".to_owned(),
                    found: Node::String("x".to_owned()),
                    kept: None,
                    why: Why::WrongType,
                },
            ],
            dismissed: false,
        };
        note(&book, &ledger.path, Some(&ledger));
        assert_eq!(all(&book), vec![ledger.clone()]);

        // "Leave it": marked, still there, no longer pending.
        dismiss(&book, &file, &ledger).unwrap();
        assert!(read(&book, &ledger.path).unwrap().dismissed);
        assert!(pending(&book).is_empty());

        // A STALE ledger writes nothing.
        let mut stale = ledger.clone();
        stale.file_sha256 = sha256_hex(b"something else");
        assert_eq!(apply(&book, &file, &stale), Err(Stale));
        assert_eq!(std::fs::read(&file).unwrap(), text);

        // "Write the fix": the same changes, in order, and the key order kept.
        apply(&book, &file, &ledger).unwrap();
        let fixed = Node::parse(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(
            fixed.to_string(),
            r#"{"theme":"owl","borders":"standard","list":[1,3]}"#
        );
        assert!(all(&book).is_empty(), "the ledger outlived its fix");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a ledger — which holds the values of the file it judged,
    /// a pairing key among them — is ever readable by anybody but its
    /// person (a folder wider than `0700`, a file wider than `0600`, a draft
    /// left behind), or "write the fix" loosens a private file's mode.
    #[test]
    fn a_ledger_is_private_and_the_fix_keeps_the_files_mode() {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let root = scratch("private");
        let book = root.join("repairs");
        // A FOLDER SOMETHING MADE WIDER BEFORE is narrowed, not trusted.
        std::fs::create_dir_all(&book).unwrap();
        std::fs::set_permissions(&book, std::fs::Permissions::from_mode(0o755)).unwrap();
        let file = root.join("player-peers.json");
        let text = br#"{"guests": [{"name": null, "key": "a-secret-key"}]}"#;
        std::fs::write(&file, text).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        let ledger = Ledger {
            path: "state/player-peers.json".to_owned(),
            file_sha256: sha256_hex(text),
            repairs: vec![Repair {
                pointer: "/guests/0".to_owned(),
                found: Node::parse(br#"{"name": null, "key": "a-secret-key"}"#).unwrap(),
                kept: None,
                why: Why::WrongType,
            }],
            dismissed: false,
        };
        note(&book, &ledger.path, Some(&ledger));
        assert_eq!(
            mode(&book),
            0o700,
            "the ledgers' folder is readable by others"
        );
        assert_eq!(
            mode(&book.join(file_name(&ledger.path))),
            0o600,
            "a ledger holding a key is readable by others"
        );
        // A CHANGED LEDGER is written private again, and leaves no draft.
        let marked = Ledger {
            dismissed: true,
            ..ledger.clone()
        };
        note(&book, &ledger.path, Some(&marked));
        assert_eq!(mode(&book.join(file_name(&ledger.path))), 0o600);
        let names: Vec<String> = std::fs::read_dir(&book)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, [file_name(&ledger.path)], "a draft was left");

        // "WRITE THE FIX" keeps the file `0600`.
        apply(&book, &file, &ledger).unwrap();
        assert_eq!(mode(&file), 0o600, "the fix loosened a private file");
        assert_eq!(
            Node::parse(&std::fs::read(&file).unwrap())
                .unwrap()
                .to_string(),
            r#"{"guests":[]}"#
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
