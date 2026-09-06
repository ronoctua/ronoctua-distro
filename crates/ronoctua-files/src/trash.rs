//! The freedesktop trash — `$XDG_DATA_HOME/Trash`, and the one on other
//! mounts.
//!
//! # Why the spec, and not a directory of our own
//!
//! Because the trash is shared. A file this manager puts away has to be one
//! that any other tool can list and restore, and vice versa: a private bin
//! would mean two piles of deleted files, one of which nobody empties.
//!
//! # The info file is written FIRST
//!
//! The spec asks for it, and the reason is a crash: a file in `files/` with no
//! `info/` entry is a file nobody can restore, because where it came from is
//! only written down there. Written the other way round, the worst outcome is
//! an orphan `.trashinfo`, which is a line in a directory nobody reads.
//!
//! # Crossing a filesystem is REFUSED, not hidden
//!
//! `rename` fails with `EXDEV` between mounts. Copying instead would turn "move
//! to trash" into forty gigabytes of I/O behind a gesture that promises to be
//! instant — so a device with no trash of its own gets a refusal that names the
//! problem, and the person chooses.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::when::{Zone, civil};

/// A trash directory, with its two halves.
#[derive(Debug, Clone)]
pub struct Bin {
    /// The directory itself.
    pub root: PathBuf,
}

impl Bin {
    /// The bin at an explicit root — the form a test uses.
    #[must_use]
    pub const fn at(root: PathBuf) -> Self {
        Self { root }
    }

    /// The home trash, from the environment.
    ///
    /// `$XDG_DATA_HOME` when it is set, `~/.local/share` otherwise, which is
    /// the spec's own fallback.
    #[must_use]
    pub fn of_home(data_home: Option<&Path>, home: Option<&Path>) -> Option<Self> {
        let base = match (data_home, home) {
            (Some(d), _) => d.to_path_buf(),
            (None, Some(h)) => h.join(".local/share"),
            (None, None) => return None,
        };
        Some(Self::at(base.join("Trash")))
    }

    /// Where the files themselves go.
    #[must_use]
    pub fn files(&self) -> PathBuf {
        self.root.join("files")
    }

    /// Where the notes about them go.
    #[must_use]
    pub fn info(&self) -> PathBuf {
        self.root.join("info")
    }

    /// Creates both halves if they are not there.
    ///
    /// # Errors
    ///
    /// When the directories cannot be created.
    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(self.files())
            .with_context(|| format!("create {}", self.files().display()))?;
        std::fs::create_dir_all(self.info())
            .with_context(|| format!("create {}", self.info().display()))?;
        Ok(())
    }

    /// Which device this bin lives on, for the `EXDEV` question.
    #[must_use]
    pub fn device(&self) -> Option<u64> {
        // The bin's PARENT when the bin itself is not there yet: an
        // `XDG_DATA_HOME` on another filesystem is the case this question
        // exists for, and refusing to answer until the directory has been
        // created would make the first trash of a session always say "another
        // filesystem".
        std::fs::metadata(&self.root)
            .ok()
            .or_else(|| std::fs::metadata(self.root.parent()?).ok())
            .map(|m| m.dev())
    }

    /// Puts a path away, and says under what name.
    ///
    /// # Errors
    ///
    /// When the path is on another filesystem, when the directories cannot be
    /// made, or when the rename fails.
    pub fn put(&self, path: &Path, now: i64, zone: &Zone) -> Result<String> {
        let absolute =
            std::path::absolute(path).with_context(|| format!("resolver {}", path.display()))?;
        let name = absolute
            .file_name()
            .context("a path with no name cannot go to the trash")?
            .to_string_lossy()
            .into_owned();
        self.ensure()?;

        let here = std::fs::symlink_metadata(&absolute)
            .with_context(|| format!("{} does not exist", absolute.display()))?
            .dev();
        if self.device() != Some(here) {
            bail!(
                "{} is on a different filesystem from the trash; \
                 moving it there would copy the whole content",
                absolute.display()
            );
        }

        let chosen = self.free_name(&name);
        // THE NOTE FIRST. A file in `files/` with no note is a file nobody can
        // put back, because where it came from is written down only there.
        let note = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            encoded(&absolute.to_string_lossy()),
            stamp(now, zone)
        );
        let note_at = self.info().join(format!("{chosen}.trashinfo"));
        std::fs::write(&note_at, note).with_context(|| format!("write {}", note_at.display()))?;

        let landing = self.files().join(&chosen);
        if let Err(e) = std::fs::rename(&absolute, &landing) {
            // The note is only useful with the file. Leaving it behind would
            // put a phantom in every other trash browser on the system.
            let _ = std::fs::remove_file(&note_at);
            return Err(e).with_context(|| format!("move {} to the trash", absolute.display()));
        }
        Ok(chosen)
    }

    /// Puts a trashed name back where it came from.
    ///
    /// # Errors
    ///
    /// When the note is missing or unreadable, when the original directory is
    /// gone, or when something is already in the way.
    pub fn restore(&self, name: &str) -> Result<PathBuf> {
        let note_at = self.info().join(format!("{name}.trashinfo"));
        let note = std::fs::read_to_string(&note_at)
            .with_context(|| format!("read {}", note_at.display()))?;
        let origin = origin_of(&note).with_context(|| {
            format!(
                "{} does not say where the file came from",
                note_at.display()
            )
        })?;
        if origin.exists() {
            bail!("there is already something at {}", origin.display());
        }
        if let Some(parent) = origin.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("recreate {}", parent.display()))?;
        }
        std::fs::rename(self.files().join(name), &origin)
            .with_context(|| format!("put {} back", origin.display()))?;
        let _ = std::fs::remove_file(&note_at);
        Ok(origin)
    }

    /// A name nothing in `files/` is using.
    fn free_name(&self, wanted: &str) -> String {
        let taken = |n: &str| {
            self.files().join(n).symlink_metadata().is_ok()
                || self.info().join(format!("{n}.trashinfo")).exists()
        };
        if !taken(wanted) {
            return wanted.to_owned();
        }
        // Both halves are checked, and not just `files/`: an orphan note left
        // by a crash would otherwise be overwritten, and with it the record of
        // where some other file belongs.
        for n in 1..10_000u32 {
            let candidate = format!("{wanted}.{n}");
            if !taken(&candidate) {
                return candidate;
            }
        }
        format!("{wanted}.{}", std::process::id())
    }
}

/// `Path=` out of a note, decoded.
fn origin_of(note: &str) -> Option<PathBuf> {
    for line in note.lines() {
        if let Some(rest) = line.strip_prefix("Path=") {
            return Some(PathBuf::from(decoded(rest.trim())));
        }
    }
    None
}

/// `DeletionDate` in the spec's shape: local time, no zone marker.
fn stamp(now: i64, zone: &Zone) -> String {
    let c = civil(now, zone);
    let seconds = now
        .saturating_add(i64::from(zone.offset_at(now)))
        .rem_euclid(60);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        c.year, c.month, c.day, c.hour, c.minute, seconds
    )
}

/// Percent-encoding, with `/` left alone — what the spec asks for.
///
/// Encoding the separator too would be legal and would make every note
/// unreadable to a person, which matters here: a `.trashinfo` is the only
/// record of where a file belongs, and reading one by hand is a real recovery
/// path.
fn encoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(char::from(b));
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// The other direction.
fn decoded(s: &str) -> String {
    let raw = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut at = 0;
    while at < raw.len() {
        if raw[at] == b'%' && at + 2 < raw.len() {
            let hex = std::str::from_utf8(&raw[at + 1..at + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                at += 3;
                continue;
            }
        }
        out.push(raw[at]);
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-trash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    #[test]
    fn a_path_survives_the_round_trip_through_the_encoding() {
        for original in [
            "/home/person/a file.txt",
            "/home/person/acentuação.md",
            "/home/person/100% mine#1.txt",
            "/home/person/plain",
        ] {
            assert_eq!(
                decoded(&encoded(original)),
                original,
                "it broke on {original}"
            );
        }
        // The separator stays legible, which is what makes a note readable by
        // hand when everything else has failed.
        assert!(encoded("/a/b").contains('/'));
        assert!(!encoded("/a b").contains(' '));
    }

    #[test]
    fn the_note_comes_before_the_file_and_says_where_it_was() {
        let root = scratch("note");
        let bin = Bin::at(root.join("Trash"));
        let file = root.join("relatorio.txt");
        std::fs::write(&file, b"conteudo").unwrap();

        let name = bin.put(&file, 1_755_000_000, &Zone::utc()).unwrap();
        assert_eq!(name, "relatorio.txt");
        assert!(!file.exists(), "the original is still there");
        assert!(bin.files().join(&name).exists());

        let note = std::fs::read_to_string(bin.info().join("relatorio.txt.trashinfo")).unwrap();
        assert!(note.starts_with("[Trash Info]"));
        assert!(note.contains(&format!("Path={}", file.display())));
        assert!(note.contains("DeletionDate=2025-08-12T"), "{note}");

        let back = bin.restore(&name).unwrap();
        assert_eq!(back, file);
        assert_eq!(std::fs::read(&file).unwrap(), b"conteudo");
        assert!(!bin.info().join("relatorio.txt.trashinfo").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_second_file_of_the_same_name_does_not_overwrite_the_first() {
        // THE DEFECT THIS CATCHES: two `notas.txt` from two directories. The
        // second overwriting the first destroys a file the person believes is
        // recoverable, which is worse than not having a trash at all.
        let root = scratch("collide");
        let bin = Bin::at(root.join("Trash"));
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::write(root.join("a/notas.txt"), b"first").unwrap();
        std::fs::write(root.join("b/notas.txt"), b"second").unwrap();

        let first = bin.put(&root.join("a/notas.txt"), 0, &Zone::utc()).unwrap();
        let second = bin.put(&root.join("b/notas.txt"), 0, &Zone::utc()).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(bin.files().join(&first)).unwrap(), b"first");
        assert_eq!(std::fs::read(bin.files().join(&second)).unwrap(), b"second");

        // And each goes back to its OWN directory.
        assert_eq!(bin.restore(&second).unwrap(), root.join("b/notas.txt"));
        assert_eq!(bin.restore(&first).unwrap(), root.join("a/notas.txt"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restoring_over_something_that_exists_is_refused() {
        let root = scratch("occupied");
        let bin = Bin::at(root.join("Trash"));
        let file = root.join("x.txt");
        std::fs::write(&file, b"old").unwrap();
        let name = bin.put(&file, 0, &Zone::utc()).unwrap();
        std::fs::write(&file, b"new").unwrap();

        let e = bin.restore(&name).unwrap_err();
        assert!(e.to_string().contains("already something at"), "{e}");
        // And the new file is untouched: a refusal that damaged something
        // would be worse than the overwrite it refused.
        assert_eq!(std::fs::read(&file).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_note_with_no_path_line_is_an_error_and_not_a_guess() {
        assert!(origin_of("[Trash Info]\nDeletionDate=2026-01-01T00:00:00\n").is_none());
        assert_eq!(
            origin_of("[Trash Info]\nPath=/tmp/a%20b\n"),
            Some(PathBuf::from("/tmp/a b"))
        );
    }
}
