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
//! instant — so a file on another volume goes to THAT volume's own trash
//! ([`Bin::of_volume`]), which is what the spec's `$topdir` trash is for, and a
//! bin on the wrong device is still refused by name.
//!
//! # A volume's trash, and what it costs the person
//!
//! `$topdir/.Trash/$uid` when the volume carries a shared `.Trash` the spec
//! trusts — a real directory, not a link, with the sticky bit — and
//! `$topdir/.Trash-$uid`, made `0700`, otherwise. Its notes say `Path=`
//! RELATIVE to `$topdir`, as the spec asks: the same stick mounts under another
//! name tomorrow, and an absolute path would restore into a folder that is no
//! longer there. The price is the one a removable drive always charged:
//! deleting writes the file back ONTO the stick, so its space is not freed until
//! that trash is emptied — which the manager says when it happens and again
//! when the stick is unmounted with something still in it.

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

    /// The trash a VOLUME keeps for this user, at its mount point `topdir`.
    ///
    /// The spec's two places, in its order: `$topdir/.Trash/$uid` when the
    /// administrator made a shared `.Trash` there that can be trusted — a
    /// directory and not a link, sticky, so one user cannot take another's
    /// files out of it — and `$topdir/.Trash-$uid` otherwise. Nothing is
    /// created here; [`Bin::ensure`] makes the directory `0700` when a file is
    /// first put in it.
    #[must_use]
    pub fn of_volume(topdir: &Path, uid: u32) -> Self {
        let shared = topdir.join(".Trash");
        let trusted =
            std::fs::symlink_metadata(&shared).is_ok_and(|m| m.is_dir() && m.mode() & 0o1000 != 0);
        if trusted {
            Self::at(shared.join(uid.to_string()))
        } else {
            Self::at(topdir.join(format!(".Trash-{uid}")))
        }
    }

    /// The mount point this bin is the trash of, when it is a VOLUME's.
    ///
    /// Read off the bin's own name, so the queue — which receives only a root —
    /// knows the same thing the manager knew when it chose it: `.Trash-<n>`
    /// sits in `$topdir`, and `<n>` inside a `.Trash` sits one level lower. The
    /// home trash (`…/Trash`) has none.
    #[must_use]
    pub fn top(&self) -> Option<PathBuf> {
        let name = self.root.file_name()?.to_str()?;
        if name
            .strip_prefix(".Trash-")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        {
            return self.root.parent().map(Path::to_path_buf);
        }
        let parent = self.root.parent()?;
        (name.bytes().all(|b| b.is_ascii_digit())
            && !name.is_empty()
            && parent.file_name().is_some_and(|p| p == ".Trash"))
        .then(|| parent.parent().map(Path::to_path_buf))
        .flatten()
    }

    /// How many things are in it — nought for a bin that was never made.
    ///
    /// What unmounting asks: a stick that leaves with files in its own trash
    /// leaves with its space still taken, and the person is told so.
    #[must_use]
    pub fn count(&self) -> usize {
        std::fs::read_dir(self.files()).map_or(0, Iterator::count)
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
        // A VOLUME'S BIN IS THIS USER'S ALONE, and the spec says so: `0700`,
        // made with that mode rather than chmodded after (for the reason the
        // secrets' files give — between the two calls it would be readable).
        // On a filesystem with no owners (vfat) the mount's own options decide,
        // and the mode is what is asked for anyway.
        if self.top().is_some() && std::fs::symlink_metadata(&self.root).is_err() {
            use std::os::unix::fs::DirBuilderExt as _;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&self.root)
                .with_context(|| format!("create {}", self.root.display()))?;
        }
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
        // RELATIVE TO THE VOLUME, in a volume's bin — see the module's note.
        let written = self
            .top()
            .and_then(|top| absolute.strip_prefix(&top).ok().map(Path::to_path_buf))
            .unwrap_or_else(|| absolute.clone());
        let note = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            encoded(&written.to_string_lossy()),
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
        // A NOTE IS NOT TRUSTED TO CLIMB. A volume's notes come off a stick
        // that may have been written anywhere, and `Path=../../home/ana/.config`
        // joined to the mount point is a folder in somebody's home — made by
        // `create_dir_all` below before the rename's `EXDEV` could stop
        // anything. No note this manager or the spec writes has a `..` in it.
        if origin
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            bail!(
                "{} says the file came from {}, which climbs out with `..`",
                note_at.display(),
                origin.display()
            );
        }
        // A RELATIVE PATH IS THE VOLUME'S, wherever it is mounted today. One
        // in the home trash has no volume to be relative to, and is refused
        // rather than restored into whatever directory this process is in.
        let origin = if origin.is_absolute() {
            // AND AN ABSOLUTE ONE IN A VOLUME'S NOTE stays on that volume: the
            // spec allows one, and a stick's note naming `/home/ana/...` is a
            // stick asking to write into the home.
            if let Some(top) = self.top()
                && !origin.starts_with(&top)
            {
                bail!(
                    "{} says the file came from {}, outside the volume at {}",
                    note_at.display(),
                    origin.display(),
                    top.display()
                );
            }
            origin
        } else {
            self.top().map(|top| top.join(&origin)).with_context(|| {
                format!(
                    "{} says a relative path outside a volume's trash",
                    note_at.display()
                )
            })?
        };
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

    /// Throw away what has been in here for `days` or longer.
    ///
    /// # Why this is counted from `DeletionDate` and from nothing else
    ///
    /// Because it is the only timestamp that answers the question. A document
    /// last written a year ago and thrown away this morning has been in the
    /// trash for a MORNING; a rule reading the file's own `mtime` would delete
    /// it before the person had finished regretting it, and a rule reading its
    /// `atime` would be reading when some indexer last touched it.
    ///
    /// The spec already writes `DeletionDate` into every `.trashinfo`, in local
    /// time with no zone marker, and this reads it back through the same
    /// [`Zone`] that wrote it.
    ///
    /// # Why this is not the cache sweep
    ///
    /// Because nothing here can be made again. `ronoctua-cache-sweep` deletes
    /// what a program can rebuild and deliberately never goes near
    /// `$XDG_DATA_HOME`; this deletes the only copy, which is why its number is
    /// four days rather than forty and why the note is read rather than a
    /// timestamp guessed at.
    ///
    /// # The note goes LAST
    ///
    /// The mirror of [`Bin::put`], and the same crash argument: a file in `files/`
    /// with no note is a file nobody can restore, so the note must outlive the
    /// file it describes until the file is gone. Deleted the other way round, a
    /// crash in between leaves something unrestorable.
    ///
    /// An orphan note — one whose file is already gone — is taken whenever it
    /// is old enough, because a note about nothing is what a crash leaves.
    /// # `take` is what makes a rehearsal honest
    ///
    /// It counts exactly what it would throw away and throws nothing — one
    /// path, so the rehearsal cannot answer differently from the run. The shape
    /// `nix run .#install` and `ronoctua-cache-sweep` already have, and it
    /// matters most here: this is the only thing in the tree that deletes
    /// something nobody can make again.
    pub fn expire(&self, now: i64, days: u16, zone: &Zone, take: bool) -> Emptied {
        let mut out = Emptied::default();
        let Ok(notes) = std::fs::read_dir(self.info()) else {
            // NO TRASH IS NOT A FAILURE: a machine that has thrown nothing away
            // has nothing to empty.
            return out;
        };
        let cutoff = i64::from(days) * 24 * 60 * 60;
        for note in notes.flatten() {
            let at = note.path();
            if at.extension().is_none_or(|e| e != "trashinfo") {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(&at) else {
                out.refused += 1;
                continue;
            };
            let Some(gone_at) = deleted_at(&body, zone) else {
                // A NOTE WITH NO DATE IS NOT INFINITELY OLD. It is a note this
                // cannot judge, and the safe answer about something that cannot
                // be made again is to leave it.
                out.refused += 1;
                continue;
            };
            // AND A DATE IN THE FUTURE KEEPS IT, for the reason the cache
            // sweep's own rule gives: the clock may have been wrong.
            if now.saturating_sub(gone_at) < cutoff {
                out.kept += 1;
                continue;
            }
            let Some(name) = at.file_stem().and_then(|n| n.to_str()) else {
                out.refused += 1;
                continue;
            };
            let body = self.files().join(name);
            let weight = weigh(&body);
            if take {
                let gone = match std::fs::symlink_metadata(&body) {
                    Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&body).is_ok(),
                    Ok(_) => std::fs::remove_file(&body).is_ok(),
                    // ALREADY GONE: an orphan note, which a crash leaves behind.
                    Err(_) => true,
                };
                if !gone {
                    out.refused += 1;
                    continue;
                }
                let _ = std::fs::remove_file(&at);
            }
            out.taken += 1;
            out.bytes += weight;
        }
        out
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

/// The mount point `path` lives under, out of the ones given — the longest
/// that holds it.
///
/// The host hands the mount points it already knows (the disk border's), and
/// this chooses among them: no `/proc/self/mountinfo` read here and no `st_dev`
/// walk, which does not see a bind mount of the same filesystem (`AGENTS.md`).
/// `None` when none of them holds it.
#[must_use]
pub fn topdir_of<'a>(path: &Path, mounts: &'a [PathBuf]) -> Option<&'a Path> {
    mounts
        .iter()
        .filter(|m| path.starts_with(m))
        .max_by_key(|m| m.components().count())
        .map(PathBuf::as_path)
}

/// The volumes mounted directly under `media` — the folder udisks mounts a
/// person's drives in (`/run/media/<user>`) — for the trash's expiry, which
/// has no disk border to ask and needs none: a directory there that is the
/// ROOT OF A MOUNT is a volume. Asked of the kernel (`STATX_ATTR_MOUNT_ROOT`)
/// and not by comparing devices, which a bind mount of the same filesystem
/// would fool (`AGENTS.md`); a kernel too old to say falls back to the device.
///
/// Empty when `media` is not there — no drive was ever mounted.
#[must_use]
pub fn volumes_under(media: &Path) -> Vec<PathBuf> {
    let Ok(here) = std::fs::metadata(media) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(media) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let Ok(meta) = std::fs::symlink_metadata(p) else {
                return false;
            };
            meta.is_dir() && (mount_root(p) == Some(true) || meta.dev() != here.dev())
        })
        .collect();
    out.sort();
    out
}

/// Whether the kernel says `path` is where a mount begins. `None` when it
/// could not be asked; `Some(false)` from a kernel that does not know the
/// attribute.
fn mount_root(path: &Path) -> Option<bool> {
    use rustix::fs::{AtFlags, StatxAttributes, StatxFlags};
    let got = rustix::fs::statx(
        rustix::fs::CWD,
        path,
        AtFlags::SYMLINK_NOFOLLOW,
        StatxFlags::TYPE,
    )
    .ok()?;
    Some(
        got.stx_attributes_mask
            .contains(StatxAttributes::MOUNT_ROOT)
            && got.stx_attributes.contains(StatxAttributes::MOUNT_ROOT),
    )
}

/// What one emptying took.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Emptied {
    /// Entries thrown away for good.
    pub taken: u64,
    /// What they weighed.
    pub bytes: u64,
    /// Entries still young enough to stay.
    pub kept: u64,
    /// Notes that could not be read, or dated, or whose file would not go.
    ///
    /// A count and not a failure: this is the only copy of what it deletes, so
    /// anything it cannot judge is left alone and said out loud.
    pub refused: u64,
}

/// `DeletionDate=` out of a note, as epoch seconds.
///
/// The spec's shape is local time with no zone marker — `2026-01-31T09:05:00` —
/// which is what [`Bin::put`] writes. Reading it back needs the same zone, so
/// the one that wrote it is handed in rather than guessed at.
fn deleted_at(note: &str, zone: &Zone) -> Option<i64> {
    let line = note
        .lines()
        .find_map(|l| l.strip_prefix("DeletionDate="))?
        .trim();
    let (date, time) = line.split_once('T')?;
    let mut date = date.split('-');
    let year: i64 = date.next()?.parse().ok()?;
    let month: u32 = date.next()?.parse().ok()?;
    let day: u32 = date.next()?.parse().ok()?;
    let mut time = time.split(':');
    let hour: i64 = time.next()?.parse().ok()?;
    let minute: i64 = time.next()?.parse().ok()?;
    let second: i64 = time.next().unwrap_or("0").parse().ok()?;
    let local = crate::when::epoch_of(year, month, day)?
        .checked_mul(24 * 60 * 60)?
        .checked_add(hour * 3600 + minute * 60 + second)?;
    // BACK OUT OF LOCAL TIME through the same zone that put it in. The offset
    // is asked for at the instant being converted, which is what keeps a file
    // thrown away the hour the clocks moved from reading an hour out.
    Some(local - i64::from(zone.offset_at(local)))
}

/// What a file or a whole directory weighs.
fn weigh(at: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(at) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    let mut total = 0;
    let mut stack = vec![at.to_path_buf()];
    while let Some(here) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&here) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
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
    fn the_trash_empties_itself_of_what_has_been_in_it_long_enough() {
        // WHAT IS DELETED HERE IS THE ONLY COPY, which is the whole reason this
        // reads `DeletionDate` and not a timestamp: a document last written a
        // year ago and thrown away this morning has been in the trash for a
        // morning.
        let home = scratch("expira");
        let trash = Bin::of_home(Some(&home), None).expect("a trash");
        trash.ensure().unwrap();
        let zone = Zone::utc();
        let now = 1_800_000_000_i64;
        let day = 24 * 60 * 60;

        // THROWN AWAY LONG AGO, and its file written this morning: the note is
        // what decides, so this goes.
        let old = home.join("old.txt");
        std::fs::write(&old, b"gone").unwrap();
        let old_name = trash.put(&old, now - 9 * day, &zone).unwrap();

        // AND THROWN AWAY YESTERDAY, its file a year old: this stays.
        let fresh = home.join("fresh.txt");
        std::fs::write(&fresh, b"keep").unwrap();
        let fresh_name = trash.put(&fresh, now - day, &zone).unwrap();

        // AND A NOTE WITH NO DATE, which this cannot judge and must not take.
        std::fs::write(
            trash.info().join("mystery.trashinfo"),
            "[Trash Info]\nPath=/tmp/mystery\n",
        )
        .unwrap();
        std::fs::write(trash.files().join("mystery"), b"?").unwrap();

        // THE REHEARSAL FIRST, and it must count the same and take nothing.
        let said = trash.expire(now, 4, &zone, false);
        assert!(
            trash.files().join(&old_name).exists(),
            "a rehearsal threw something away"
        );

        let took = trash.expire(now, 4, &zone, true);
        assert_eq!(
            (said.taken, said.bytes),
            (took.taken, took.bytes),
            "the rehearsal's number and the real one disagree"
        );

        assert!(
            !trash.files().join(&old_name).exists(),
            "something thrown away nine days ago is still there"
        );
        assert!(
            !trash.info().join(format!("{old_name}.trashinfo")).exists(),
            "its note outlived it"
        );
        assert!(
            trash.files().join(&fresh_name).exists(),
            "something thrown away yesterday was taken"
        );
        assert!(
            trash.files().join("mystery").exists(),
            "a note it could not date was acted on anyway"
        );
        assert_eq!(took.taken, 1);
        assert_eq!(took.kept, 1);
        assert_eq!(took.refused, 1, "it did not say what it could not judge");
        assert!(took.bytes > 0, "it weighed nothing");

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_deletion_date_reads_back_as_the_instant_that_wrote_it() {
        // THE ROUND TRIP, because the spec's shape has no zone marker: a note
        // written by `put` in a zone has to come back out of `deleted_at` in
        // the same one, or a file is judged hours out — and at four days, hours
        // are what stands between a mistake and a deletion.
        let zone = Zone::utc();
        for at in [0_i64, 1_800_000_000, 1_700_000_000, 946_684_800] {
            let note = format!("[Trash Info]\nPath=/x\nDeletionDate={}\n", stamp(at, &zone));
            assert_eq!(deleted_at(&note, &zone), Some(at), "{note}");
        }
        // AND A NOTE THAT SAYS NOTHING READABLE IS NOT A DATE.
        assert_eq!(deleted_at("[Trash Info]\nPath=/x\n", &zone), None);
        assert_eq!(
            deleted_at("[Trash Info]\nDeletionDate=not a date\n", &zone),
            None
        );
        assert_eq!(
            deleted_at("[Trash Info]\nDeletionDate=2026-13-01T00:00:00\n", &zone),
            None,
            "a thirteenth month was accepted"
        );
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
    fn a_volume_keeps_its_own_trash_and_writes_where_from_relative_to_itself() {
        use std::os::unix::fs::PermissionsExt as _;
        // A STICK, as far as this module can tell: a directory standing for a
        // mount point, with no `.Trash` of the administrator's in it.
        let stick = scratch("volume");
        std::fs::create_dir_all(stick.join("fotos")).unwrap();
        let photo = stick.join("fotos/praia.jpg");
        std::fs::write(&photo, b"jpeg").unwrap();

        let bin = Bin::of_volume(&stick, 1000);
        assert_eq!(bin.root, stick.join(".Trash-1000"));
        assert_eq!(bin.top().as_deref(), Some(stick.as_path()));
        assert_eq!(bin.count(), 0, "a bin never made holds nothing");

        let name = bin.put(&photo, 1_755_000_000, &Zone::utc()).unwrap();
        let mode = std::fs::metadata(&bin.root).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "a volume's bin is this user's alone");
        let note = std::fs::read_to_string(bin.info().join(format!("{name}.trashinfo"))).unwrap();
        assert!(note.contains("Path=fotos/praia.jpg\n"), "{note}");
        assert_eq!(bin.count(), 1);

        // THE SAME STICK, MOUNTED ELSEWHERE: the note is relative, so it goes
        // back where the stick is now — which an absolute path could not do.
        let moved = scratch("volume-moved");
        std::fs::remove_dir_all(&moved).unwrap();
        std::fs::rename(&stick, &moved).unwrap();
        let again = Bin::of_volume(&moved, 1000);
        assert_eq!(again.restore(&name).unwrap(), moved.join("fotos/praia.jpg"));
        assert_eq!(
            std::fs::read(moved.join("fotos/praia.jpg")).unwrap(),
            b"jpeg"
        );
        let _ = std::fs::remove_dir_all(&moved);
    }

    #[test]
    fn a_shared_trash_is_used_only_when_it_is_sticky_and_real() {
        use std::os::unix::fs::PermissionsExt as _;
        let top = scratch("shared");
        std::fs::create_dir(top.join(".Trash")).unwrap();
        // NOT STICKY: somebody else could empty it, so the spec says no.
        assert_eq!(Bin::of_volume(&top, 7).root, top.join(".Trash-7"));
        std::fs::set_permissions(top.join(".Trash"), std::fs::Permissions::from_mode(0o1777))
            .unwrap();
        let bin = Bin::of_volume(&top, 7);
        assert_eq!(bin.root, top.join(".Trash/7"));
        assert_eq!(bin.top().as_deref(), Some(top.as_path()));
        // A LINK called `.Trash` is not trusted however it is marked.
        let linked = scratch("shared-link");
        std::os::unix::fs::symlink(top.join(".Trash"), linked.join(".Trash")).unwrap();
        assert_eq!(Bin::of_volume(&linked, 7).root, linked.join(".Trash-7"));
        // And the home trash is nobody's volume.
        assert_eq!(Bin::at(top.join("share/Trash")).top(), None);
        let _ = std::fs::remove_dir_all(&top);
        let _ = std::fs::remove_dir_all(&linked);
    }

    #[test]
    fn a_path_belongs_to_the_deepest_mount_that_holds_it() {
        let mounts = vec![
            PathBuf::from("/run/media/ana"),
            PathBuf::from("/run/media/ana/STICK"),
            PathBuf::from("/run/media/ana/STICK2"),
        ];
        assert_eq!(
            topdir_of(Path::new("/run/media/ana/STICK/a/b.txt"), &mounts),
            Some(Path::new("/run/media/ana/STICK"))
        );
        // A NAME THAT ONLY STARTS THE SAME is another directory: components,
        // not characters.
        assert_eq!(
            topdir_of(Path::new("/run/media/ana/STICK22/x"), &mounts),
            Some(Path::new("/run/media/ana"))
        );
        assert_eq!(topdir_of(Path::new("/home/ana/x"), &mounts), None);
    }

    #[test]
    fn a_relative_note_in_the_home_trash_is_refused_and_not_restored_here() {
        let root = scratch("relative-home");
        let bin = Bin::at(root.join("Trash"));
        bin.ensure().unwrap();
        std::fs::write(bin.files().join("x"), b"x").unwrap();
        std::fs::write(
            bin.info().join("x.trashinfo"),
            "[Trash Info]\nPath=x\nDeletionDate=2026-01-01T00:00:00\n",
        )
        .unwrap();
        assert!(bin.restore("x").is_err());
        assert!(bin.files().join("x").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_sticks_note_that_climbs_out_of_the_stick_touches_nothing() {
        // A STICK written somewhere else, whose notes say where its files came
        // from — and one of them says a folder in somebody's home.
        let root = scratch("climb");
        let stick = root.join("STICK");
        let home = root.join("home");
        std::fs::create_dir_all(&stick).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let bin = Bin::of_volume(&stick, 1000);
        bin.ensure().unwrap();
        for (name, path) in [
            ("up", "../home/.config/evil/x".to_owned()),
            ("inside", "a/../../home/.config/evil/y".to_owned()),
            ("absolute", format!("{}/.config/evil/z", home.display())),
        ] {
            std::fs::write(bin.files().join(name), b"x").unwrap();
            std::fs::write(
                bin.info().join(format!("{name}.trashinfo")),
                format!("[Trash Info]\nPath={path}\nDeletionDate=2026-01-01T00:00:00\n"),
            )
            .unwrap();
            assert!(bin.restore(name).is_err(), "{path} was restored");
            assert!(
                bin.files().join(name).exists(),
                "{path}: the file left the trash"
            );
        }
        assert!(
            !home.join(".config").exists(),
            "a folder was made outside the stick before anything refused"
        );
        // AND AN ABSOLUTE PATH ON THE STICK ITSELF is still one the spec allows.
        std::fs::write(bin.files().join("ok"), b"x").unwrap();
        std::fs::write(
            bin.info().join("ok.trashinfo"),
            format!(
                "[Trash Info]\nPath={}/fotos/ok\nDeletionDate=2026-01-01T00:00:00\n",
                stick.display()
            ),
        )
        .unwrap();
        assert_eq!(bin.restore("ok").unwrap(), stick.join("fotos/ok"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn only_a_directory_that_is_a_mount_is_a_volume() {
        // NOTHING MOUNTED under a scratch folder: a plain directory there is
        // not a volume, and a folder that is not there is no volumes at all.
        let media = scratch("media");
        std::fs::create_dir_all(media.join("LEFTOVER")).unwrap();
        std::fs::write(media.join("file"), b"x").unwrap();
        assert!(volumes_under(&media).is_empty());
        assert!(volumes_under(&media.join("nowhere")).is_empty());
        // AND ONE THAT IS: `/proc` is a mount of its own under `/`, on every
        // Linux this runs on, the build sandbox included.
        assert!(
            volumes_under(Path::new("/")).contains(&PathBuf::from("/proc")),
            "{:?}",
            volumes_under(Path::new("/"))
        );
        let _ = std::fs::remove_dir_all(&media);
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
