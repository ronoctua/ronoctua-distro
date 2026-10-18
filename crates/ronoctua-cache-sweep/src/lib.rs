//! Walks a cache root and deletes what nothing has used for a while.
//!
//! # Why this exists
//!
//! Because nothing swept anything. The owner asked how this distro's caches
//! worked and the honest answer was that neither of the two on disk was ever
//! cleaned — no timer, no tmpfiles rule, no walk. His rule came back in one
//! sentence: nothing cached may stay for ever.
//!
//! Measured on his machine the day this was written: `$XDG_CACHE_HOME` held
//! 254 GB of cargo artefacts, 36 GB of a package manager's downloads and 2 GB
//! of a browser's — against 4,6 MB of our own thumbnails. **The thumbnails were
//! never the problem**, and that is why the reach is the whole root and not a
//! directory of ours.
//!
//! # WHAT IT LOOKS AT: the entries in the root, each one whole
//!
//! Not every file, weighed on its own. The owner stopped that, and the reason
//! is worth having here too: a cache is rarely a heap of independent files. It
//! is an index and the things the index names, a manifest and its blobs, a
//! database and its write-ahead log. Take the ones nobody read lately and leave
//! the rest, and you have not freed a cache — you have left a program holding
//! half of something, which is a class of failure nobody can debug from a
//! directory listing.
//!
//! Nearly every program keeps its cache in a directory of its own under the
//! root, which is what makes the coarse rule the SAFE one: the unit of deletion
//! becomes the unit the program created. **A directory is as old as its
//! youngest file**, so a cache anything still touches is never taken, however
//! much of it is ancient — and the walk still has to descend to find that out.
//!
//! # What it will not do, and why each one is here
//!
//! This binary deletes files for a living, so the refusals are the design:
//!
//! - **It never follows a symbolic link.** Every entry is looked at with
//!   [`std::fs::symlink_metadata`], never `metadata`. A `~/.cache/x -> /home`
//!   followed by a sweep is the worst thing this code could possibly do.
//! - **It never leaves the filesystem it started on.** A mount under the cache
//!   root is somebody else's disk with somebody else's rules. The device is
//!   the ROOT's own: on the distro the root is `~/.cache`'s own btrfs
//!   subvolume (`nix/modules/filesystem.nix`), so the whole cache is one
//!   device, and a subvolume nested inside it is another one, left alone. A
//!   bind mount keeps its device number, so the kernel is also asked where a
//!   mount begins, and one inside is left alone the same way.
//! - **It deletes regular files only.** A socket, a fifo or a device under
//!   there belongs to a program that is running now.
//! - **A timestamp in the FUTURE keeps the file.** The clock may have been
//!   wrong when it was written, and throwing away what one cannot date is worse
//!   than keeping it — the rule `ronoctua_notify`'s own prune already follows.
//! - **It rehearses unless told otherwise.** [`Act::Rehearse`] counts exactly
//!   what [`Act::Execute`] would take and removes nothing, which is the shape
//!   `nix run .#install` already has.
//! - **It never takes what Nix counts as a root.** A GC root can live in a
//!   cache — `nix develop --profile` leaves one, `~/.cache/<x>/…-link` — and a
//!   directory taken whole would take the link with it; the next collection
//!   would then free what the person's shell stands on. So Nix is ASKED which
//!   links are roots ([`Roots`]), and an entry holding one is kept. When Nix
//!   cannot answer — or answers in a shape this cannot read — every directory
//!   holding any link is kept instead. And every link is dated by its own
//!   stamp, so one made while the walk runs makes its directory young.
//!
//! # And the machine's load is NOT this crate's question
//!
//! It is answered before this is ever started, by `ronoctua_brain_daemon::runner::judge`
//! against a gate the person configured in one place. A second opinion here
//! would be a second policy that drifts from the first — the same refusal
//! the update's brain module writes out about itself.
//!
//! # Two walks, and the relief that runs them
//!
//! [`sweep`] takes whole entries of the cache by age, on the brain's schedule.
//! [`cargo::sweep`] looks INSIDE cargo's build directories, which the first
//! walk cannot shrink — a build tree touched this morning is young as a whole
//! however much of it is a year old — and it runs when the disk is short, from
//! `ronoctua-disk-relief` (`src/relief.rs`), beside a collection of the store.

pub mod cargo;

use std::collections::HashSet;
use std::io;
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Whether the walk is a rehearsal or the real thing.
///
/// The default is the harmless one on purpose: a caller that forgets to say
/// counts instead of deleting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Act {
    /// Count what would go, and take nothing.
    #[default]
    Rehearse,
    /// Take it.
    Execute,
}

/// What one walk found and did.
///
/// `bytes` is what the FILES weighed, not what the filesystem gave back:
/// on btrfs with compression or a reflink those two differ, and the honest
/// number to report is the one that was measured rather than one guessed at
/// from a `statvfs` before and after — which another process writing at the
/// same time would make a lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Swept {
    /// Files in the ROOT taken, or that would have been.
    ///
    /// A cache kept as a single file rather than a directory — `.tdb`, a
    /// sqlite database, an index. Most of what goes is a directory.
    pub files: u64,
    /// What they weighed.
    pub bytes: u64,
    /// Entries in the root looked at and left, because something still uses
    /// them — or because they could not be dated at all.
    pub kept: u64,
    /// Directories in the root taken WHOLE, or that would have been.
    ///
    /// The ordinary case: nearly every program keeps its cache in one of these,
    /// and a directory goes entire or not at all. See the module header.
    pub dirs: u64,
    /// Entries the walk could not read or could not remove.
    ///
    /// A count and not a failure: a cache directory being written by the
    /// program that owns it while this walks is ordinary, and a sweep that gave
    /// up on the first `EACCES` would be a sweep that never finished once.
    ///
    /// It also counts the directories kept for holding a link when Nix could
    /// not say which links are roots ([`Roots::Unknown`]): those were not
    /// answered either.
    pub refused: u64,
    /// Entries kept because a link Nix counts as a GC root is inside, whatever
    /// their age — see [`Roots`].
    pub rooted: u64,
}

/// Which links on this machine Nix counts as garbage-collector roots.
///
/// # Why the sweep has to ask
///
/// Because a root can live in a cache, and nothing about it looks special: a
/// `nix develop --profile ~/.cache/x/profile` leaves `profile-1-link` there,
/// pointing at the whole development closure. The whole-entry rule would take
/// the directory once nothing in it was touched for forty days, and the next
/// collection would free gigabytes somebody's shell stands on — which breaks
/// the owner's promise that nothing automatic removes what the person's Nix
/// relies on.
///
/// # Why by device and inode, and never by path
///
/// Nix prints a root by the path IT knows (`/home/x/.cache/…`), and the walk
/// reaches the same link by another spelling (`%C` of a unit, a home behind a
/// link, a bind mount). A path compared as text would miss exactly those. The
/// link's own `(st_dev, st_ino)`, read with `symlink_metadata` on both sides, is
/// the same whichever way it was reached — and it never resolves the link.
///
/// # Asking Nix and not reimplementing it
///
/// `nix-store --gc --print-roots` is the daemon's own answer, with its own idea
/// of an indirect root (invariant 5). Reading `gcroots/auto` here would be a
/// second answer that drifts from the first.
///
/// # And an answer this cannot read is no answer
///
/// The guard must never turn itself off in silence. A Nix that printed its
/// roots in a shape this parser was not written for would otherwise come back
/// as `Known` and EMPTY — no link a root, every devshell profile under the
/// cache fair game, and the journal saying "0 holding a Nix root" as if that
/// were a fact. So a line that looks like nothing this knows, or a listing
/// that names no root at all (a NixOS machine always has one: its system
/// profile), is [`Roots::Unknown`], which keeps more, never less. The shape
/// itself is proved against the pinned `nix-store` by a test of this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Roots {
    /// Nix answered, and these are the `(device, inode)` of the links it named.
    Known(HashSet<(u64, u64)>),
    /// Nix could not be asked, did not answer, or answered in a shape this
    /// cannot read — and the sentence that says which, for the journal. EVERY
    /// link then counts: a directory holding any link is kept, because any of
    /// them could be a root.
    Unknown(String),
}

/// What one line of a `--print-roots` listing is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RootLine {
    /// A root that is a link on this machine, by the path Nix knows it by.
    Link(PathBuf),
    /// A root that is no link anybody could meet — a process holding a path
    /// (`{censored}` when it is another user's, `{temp:…}`, `{memory:…}`).
    Held,
    /// Neither: a shape this was not written for.
    Unread,
}

impl Roots {
    /// No roots at all — what a test hands in when roots are not its subject.
    #[must_use]
    pub fn none() -> Self {
        Self::Known(HashSet::new())
    }

    /// Asks Nix, through the `nix-store` at this STORE PATH.
    ///
    /// By path and never by name, for the rule every program the system starts
    /// follows: a unit's `PATH` is minimal, and a name fails far from its cause.
    /// Any failure — the program missing, the daemon not answering, an exit
    /// that is not zero — is [`Roots::Unknown`], which keeps more, never less,
    /// and carries what went wrong: a refusal with no sentence is a journal
    /// line nobody can act on.
    #[must_use]
    pub fn asking(nix_store: &Path) -> Self {
        let said = std::process::Command::new(nix_store)
            .args(["--gc", "--print-roots"])
            .stdin(std::process::Stdio::null())
            .output();
        match said {
            Ok(out) if out.status.success() => Self::from_listing(&out.stdout),
            Ok(out) => Self::Unknown(format!(
                "{} --gc --print-roots ended with {}: {}",
                nix_store.display(),
                out.status,
                last_line(&out.stderr)
            )),
            Err(e) => Self::Unknown(format!("could not run {}: {e}", nix_store.display())),
        }
    }

    /// The roots a `--print-roots` listing names, as links on this machine.
    ///
    /// A line is `<link> -> <store path>`. Nix 2.34 prints the link QUOTED, the
    /// way C++ streams a `std::filesystem::path` (a backslash escapes the next
    /// byte); older ones print it bare. A root a process holds is a braced
    /// word (`{censored}` when it is another user's), not a link this sweep
    /// could meet. A link that is gone is skipped. Anything else — and a
    /// listing with no root in it — is [`Roots::Unknown`]: see the type.
    #[must_use]
    pub fn from_listing(listing: &[u8]) -> Self {
        let mut links = HashSet::new();
        let mut named = 0usize;
        for line in listing.split(|b| *b == b'\n') {
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            match root_line(line) {
                RootLine::Link(path) => {
                    named += 1;
                    if let Ok(meta) = std::fs::symlink_metadata(&path)
                        && meta.file_type().is_symlink()
                    {
                        links.insert((meta.dev(), meta.ino()));
                    }
                }
                RootLine::Held => named += 1,
                RootLine::Unread => {
                    return Self::Unknown(format!(
                        "a line of `nix-store --gc --print-roots` this cannot read: {}",
                        String::from_utf8_lossy(line).trim()
                    ));
                }
            }
        }
        if named == 0 {
            return Self::Unknown(
                "`nix-store --gc --print-roots` named no root at all, and a machine always has one"
                    .to_owned(),
            );
        }
        Self::Known(links)
    }

    /// Whether this entry, as `symlink_metadata` saw it, is a root.
    ///
    /// Only a LINK can be one. With [`Roots::Unknown`] every link answers yes.
    #[must_use]
    pub fn holds(&self, meta: &std::fs::Metadata) -> bool {
        if !meta.file_type().is_symlink() {
            return false;
        }
        match self {
            Self::Known(links) => links.contains(&(meta.dev(), meta.ino())),
            Self::Unknown(_) => true,
        }
    }

    /// Whether Nix answered.
    #[must_use]
    pub const fn known(&self) -> bool {
        matches!(self, Self::Known(_))
    }

    /// Why Nix's answer is missing — `None` when there is one.
    #[must_use]
    pub fn why_unknown(&self) -> Option<&str> {
        match self {
            Self::Known(_) => None,
            Self::Unknown(why) => Some(why),
        }
    }
}

/// What one line of `--print-roots` says — see [`RootLine`].
fn root_line(line: &[u8]) -> RootLine {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let (bytes, rest) = if let Some(quoted) = line.strip_prefix(b"\"") {
        // QUOTED: up to the first quote nobody escaped, with every escape
        // taken off. A link whose name holds a quote or a backslash is spelled
        // back exactly, which is the whole point of the quoting.
        let mut out = Vec::with_capacity(quoted.len());
        let mut at = quoted.iter();
        loop {
            match at.next() {
                Some(b'\\') => match at.next() {
                    Some(b) => out.push(*b),
                    None => return RootLine::Unread,
                },
                Some(b'"') => break,
                Some(b) => out.push(*b),
                None => return RootLine::Unread,
            }
        }
        (out, at.as_slice())
    } else {
        // BARE: up to the LAST arrow, since a store path has no space in it
        // and a link's name may.
        let Some(arrow) = line.windows(4).rposition(|w| w == b" -> ") else {
            return RootLine::Unread;
        };
        (line[..arrow].to_vec(), &line[arrow..])
    };
    // AND WHAT IT POINTS AT IS A STORE PATH, or this is not the line it looks
    // like.
    if !rest.starts_with(b" -> /") {
        return RootLine::Unread;
    }
    match bytes.first() {
        Some(b'/') => RootLine::Link(PathBuf::from(std::ffi::OsString::from_vec(bytes))),
        Some(b'{') if bytes.ends_with(b"}") => RootLine::Held,
        _ => RootLine::Unread,
    }
}

/// The last line a program said that is not blank.
#[must_use]
pub fn last_line(said: &[u8]) -> String {
    String::from_utf8_lossy(said)
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_owned()
}

/// Whether the directory at `path` is where a mount begins.
///
/// # Why the device number is not enough
///
/// "Never leave the filesystem it started on" was a comparison of `st_dev`,
/// and a BIND mount of the same filesystem keeps its device number: a
/// `/persist/x` bound onto `~/.cache/x` looks like one more directory of the
/// cache, and taking it whole would delete what `/persist` holds. The kernel
/// says where a mount begins (`STATX_ATTR_MOUNT_ROOT`, since 5.8), and a mount
/// inside is refused like another device.
///
/// `None` when the question could not be asked at all — which the walks treat
/// as a directory they cannot answer for. A kernel that does not know the
/// attribute answers `Some(false)`, and the device number is then the only
/// guard, as it always was.
pub(crate) fn mount_root(path: &Path) -> Option<bool> {
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

/// Whether an entry `symlink_metadata` saw at `path` is on the same filesystem
/// as `disk`, the device a walk started on — and, for a directory, not the
/// start of a mount of its own ([`mount_root`]).
pub(crate) fn on_disk(path: &Path, meta: &std::fs::Metadata, disk: u64) -> bool {
    meta.dev() == disk && (!meta.is_dir() || mount_root(path) == Some(false))
}

/// Why a tree could not be given an age.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unanswered {
    /// Something in it could not be read or dated, or a mount is inside.
    Unreadable,
    /// A link Nix counts as a root is inside (or, when Nix did not answer,
    /// any link).
    Rooted,
}

/// How long since a file was last USED, by the later of its two timestamps.
///
/// # Why the later of the two, when the rule says "last use"
///
/// Because `atime` alone is a trap on a filesystem mounted `noatime`, where it
/// is stamped once at creation and never moved again. There, a file WRITTEN
/// yesterday still carries an access time from the day it was made — so a rule
/// reading `atime` alone would delete this morning's work and call it a month
/// old. Taking the later of the two is what makes "last use" mean the same
/// thing on a machine that keeps access times and on one that does not.
///
/// Measured where this runs: btrfs with `relatime`, and access times move — an
/// entry written at 16:24 read back at 21:52 carries both.
///
/// `None` when the file's stamp is in the FUTURE, which keeps it.
#[must_use]
pub fn unused_for(meta: &std::fs::Metadata, now: SystemTime) -> Option<Duration> {
    let used = match (meta.accessed(), meta.modified()) {
        (Ok(a), Ok(m)) => a.max(m),
        (Ok(t), Err(_)) | (Err(_), Ok(t)) => t,
        // NEITHER STAMP READABLE IS NOT "INFINITELY OLD". A filesystem that
        // answers neither is one this has no business deciding about.
        (Err(_), Err(_)) => return None,
    };
    now.duration_since(used).ok()
}

/// Take every entry in `root` that nothing has used for `keep` or longer.
///
/// An entry is a file or a DIRECTORY, and a directory goes whole or not at all
/// — see the module header for why that is the safe rule and not the coarse
/// one. Its age is its youngest file's, which is what "nothing in here has been
/// touched" means.
///
/// `now` is a PARAMETER and not a clock: a sweep whose age arithmetic reads the
/// system clock cannot be tested against a file planted two days in the future,
/// and that file is the case this most needs to get right.
///
/// The root itself is never removed, and neither is an entry holding a link
/// `roots` names — see [`Roots`].
pub fn sweep(root: &Path, now: SystemTime, keep: Duration, act: Act, roots: &Roots) -> Swept {
    let mut out = Swept::default();
    let Ok(top) = std::fs::symlink_metadata(root) else {
        // NO ROOT IS NOT A FAILURE: a machine that has never cached anything
        // has nothing to sweep, and saying so with an error would make the
        // first boot look broken.
        return out;
    };
    if !top.is_dir() {
        out.refused += 1;
        return out;
    }
    let disk = top.dev();
    let Ok(entries) = std::fs::read_dir(root) else {
        out.refused += 1;
        return out;
    };

    for entry in entries {
        let Ok(entry) = entry else {
            out.refused += 1;
            continue;
        };
        let path = entry.path();
        // SYMLINK METADATA, ALWAYS. `metadata` here would follow a link out of
        // the cache root and make every rule below a rule about somebody else's
        // files.
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            out.refused += 1;
            continue;
        };
        // A LINK, A SOCKET, A FIFO, A DEVICE: left alone. The first is what this
        // must not follow and the rest belong to something running right now.
        if !meta.is_dir() && !meta.is_file() {
            continue;
        }
        // AND NEVER OFF THIS DISK. A mount under the cache root is another
        // filesystem with another owner's rules on it — a bind mount of this
        // same one included ([`mount_root`]).
        if !on_disk(&path, &meta, disk) {
            continue;
        }

        let (age, weight) = if meta.is_dir() {
            match measure(&path, now, disk, roots) {
                Ok(measured) => measured,
                Err(Unanswered::Rooted) if roots.known() => {
                    // A ROOT IS INSIDE: whatever its age, taking the directory
                    // would take the link, and the next collection would free
                    // what it holds.
                    out.rooted += 1;
                    out.kept += 1;
                    continue;
                }
                Err(_) => {
                    // SOMETHING IN THERE COULD NOT BE READ OR COULD NOT BE
                    // DATED, so this directory has no age — and a directory
                    // with no age is one this does not delete. Refusing to
                    // answer is the only safe answer about a tree one cannot
                    // see all of. A link when Nix could not say which links
                    // are roots is the same case: unanswered.
                    out.refused += 1;
                    out.kept += 1;
                    continue;
                }
            }
        } else {
            match unused_for(&meta, now) {
                Some(age) => (age, meta.len()),
                None => {
                    out.kept += 1;
                    continue;
                }
            }
        };

        if age < keep {
            out.kept += 1;
            continue;
        }
        if act == Act::Execute {
            let gone = if meta.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            match gone {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(_) => {
                    out.refused += 1;
                    continue;
                }
            }
        }
        if meta.is_dir() {
            out.dirs += 1;
        } else {
            out.files += 1;
        }
        out.bytes += weight;
    }
    out
}

/// How long since ANYTHING under `at` was used, and what it all weighs.
///
/// The age of a directory is its YOUNGEST file's, which is what "nothing in
/// here has been touched" means — one file somebody opened yesterday keeps the
/// whole thing, however ancient the rest of it is. That is the property that
/// makes taking a directory whole safe: a cache in use is never taken.
///
/// An error when any part of it could not be read or could not be dated
/// ([`Unanswered::Unreadable`]), or when a root is inside
/// ([`Unanswered::Rooted`]). A directory this cannot see all of is one it must
/// not delete, and refusing to answer is the only honest way to say so.
pub(crate) fn measure(
    at: &Path,
    now: SystemTime,
    disk: u64,
    roots: &Roots,
) -> Result<(Duration, u64), Unanswered> {
    let unreadable = |_| Unanswered::Unreadable;
    let mut youngest = Duration::MAX;
    let mut weight = 0u64;
    let mut empty = true;
    // AN EXPLICIT STACK AND NOT RECURSION. A cache directory's depth is not
    // ours to bound, and a deep one would be a stack overflow — which in this
    // process is a sweep that half ran.
    let mut stack = vec![at.to_path_buf()];
    while let Some(here) = stack.pop() {
        for entry in std::fs::read_dir(&here).map_err(unreadable)? {
            let entry = entry.map_err(unreadable)?;
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).map_err(unreadable)?;
            if meta.is_dir() {
                // A MOUNT INSIDE MAKES THE WHOLE THING UNANSWERABLE, because
                // what is under it is not this filesystem's to weigh — and a
                // `remove_dir_all` over it would be deleting across the
                // boundary this refuses to cross. A bind mount of this same
                // filesystem included, which the device alone cannot see.
                if !on_disk(&path, &meta, disk) {
                    return Err(Unanswered::Unreadable);
                }
                stack.push(path);
                empty = false;
                continue;
            }
            empty = false;
            // A ROOT INSIDE KEEPS THE WHOLE THING. It is a link, so it is
            // never followed: its own device and inode are what Nix named.
            if roots.holds(&meta) {
                return Err(Unanswered::Rooted);
            }
            // EVERYTHING INSIDE IS DATED — a link and a socket by their OWN
            // stamps, which `symlink_metadata` reads without following. The
            // roots were asked for once, before the walk, so a link made while
            // it runs (`nix build -o ~/.cache/p/result`) is in no answer; its
            // own fresh stamp is what makes the directory young, and keeps it.
            youngest = youngest.min(unused_for(&meta, now).ok_or(Unanswered::Unreadable)?);
            // Only a regular file is WEIGHED: a link's bytes are its target's
            // name, and a socket has none.
            if !meta.is_file() {
                continue;
            }
            weight = weight.saturating_add(meta.len());
        }
    }
    // AN EMPTY DIRECTORY HAS NO YOUNGEST FILE, so it is dated by ITSELF — and
    // by its MODIFICATION time alone, which is the one place in this crate that
    // does not take the later of the two.
    //
    // Because a directory's access time is not evidence about the person: it
    // moves when ANYBODY lists it, and the thing that lists it most reliably is
    // this walk. `read_dir` a few lines above stamps `atime` with now, so a
    // rule taking the later of the two would find every empty directory freshly
    // used the moment it looked — for ever, and the abandoned one would never
    // go. Found by a test that planted one three months old and watched it
    // survive.
    //
    // A directory's mtime moves when its CONTENTS change, which is exactly the
    // question being asked of an empty one.
    if empty {
        let mine = std::fs::symlink_metadata(at).map_err(unreadable)?;
        let made = mine.modified().map_err(unreadable)?;
        let age = now
            .duration_since(made)
            .map_err(|_| Unanswered::Unreadable)?;
        return Ok((age, 0));
    }
    Ok((youngest, weight))
}

/// One step of a run, as the brain's sheet carries it.
///
/// The shape of `ronoctua_brain::task::StepOutcome`, written by hand for the
/// reason the sheet is: a serialisation stack in a binary whose argument is
/// that it links almost nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StepSheet {
    /// The step's id — English, kebab-case.
    pub id: &'static str,
    /// Whether it worked.
    pub ok: bool,
    /// How long it took, in seconds.
    pub seconds: u32,
    /// What went wrong, verbatim. Empty when nothing did.
    pub trouble: String,
}

/// Writes the JSON sheet `ronoctua_brain::task::Sheet` reads.
///
/// `ended` is `None` — written as ZERO — while the run is still going, which
/// is the one thing the brain's watcher reads to tell "still working" from
/// "finished": a moment in 1970 is not a run that ended.
///
/// WRITTEN WHOLE AND RENAMED, so a read never lands on half a file; the draft
/// carries the PID, for the reason every file of this repository two processes
/// may write does.
///
/// FAILURE IS SILENCE ON PURPOSE: the report is how the brain WATCHES, and a
/// sweep that refused to run because it could not write a status file would be
/// a disk that fills up for want of a directory.
pub fn write_sheet(
    at: &Path,
    began: SystemTime,
    ended: Option<SystemTime>,
    steps: &[StepSheet],
    trouble: &str,
) {
    let stamp = |t: SystemTime| {
        t.duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
    };
    let steps: Vec<String> = steps
        .iter()
        .map(|s| {
            format!(
                "{{\"id\":{},\"ok\":{},\"seconds\":{},\"trouble\":{}}}",
                json_string(s.id),
                s.ok,
                s.seconds,
                json_string(&s.trouble)
            )
        })
        .collect();
    let body = format!(
        "{{\"began\":{},\"ended\":{},\"steps\":[{}],\"trouble\":{}}}",
        stamp(began),
        ended.map_or(0, stamp),
        steps.join(","),
        json_string(trouble)
    );
    if let Some(dir) = at.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut draft = at.as_os_str().to_owned();
    draft.push(format!(".{}.draft", std::process::id()));
    let draft = PathBuf::from(draft);
    if std::fs::write(&draft, body).is_err() || std::fs::rename(&draft, at).is_err() {
        let _ = std::fs::remove_file(&draft);
    }
}

/// A JSON string literal, quotes included.
fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `1.4 GiB`, in the units a person reads — for the journal, which is English.
#[must_use]
pub fn in_units(bytes: u64) -> String {
    const STEP: f64 = 1024.0;
    #[allow(clippy::cast_precision_loss)] // A disk is not 2^53 bytes.
    let mut left = bytes as f64;
    for unit in ["B", "KiB", "MiB", "GiB"] {
        if left < STEP {
            return format!("{left:.1} {unit}");
        }
        left /= STEP;
    }
    format!("{left:.1} TiB")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A tree of our own, under `/tmp`, that goes away with the test.
    ///
    /// Written rather than pulled in, for the reason `ronoctua-memory`'s own
    /// `mod tempdir` gives: a crate whose whole argument is that it links
    /// almost nothing does not grow a dependency to make a directory.
    pub(crate) struct Tree(pub(crate) PathBuf);

    impl Tree {
        pub(crate) fn new(name: &str) -> Self {
            let at = std::env::temp_dir().join(format!(
                "ronoctua-sweep-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&at);
            std::fs::create_dir_all(&at).expect("a scratch root");
            Self(at)
        }

        pub(crate) fn at(&self, rel: &str) -> PathBuf {
            self.0.join(rel)
        }

        /// A file with both stamps set to `age` ago.
        pub(crate) fn plant(&self, rel: &str, age: Duration) -> PathBuf {
            self.plant_with(rel, age, age)
        }

        /// A file whose two stamps DISAGREE — which is the whole of the
        /// `noatime` case.
        pub(crate) fn plant_with(&self, rel: &str, read: Duration, written: Duration) -> PathBuf {
            let at = self.at(rel);
            if let Some(up) = at.parent() {
                std::fs::create_dir_all(up).expect("the parent");
            }
            std::fs::write(&at, b"some cached bytes").expect("plant a file");
            let file = std::fs::File::options()
                .write(true)
                .open(&at)
                .expect("open to stamp");
            file.set_times(
                std::fs::FileTimes::new()
                    .set_accessed(SystemTime::now() - read)
                    .set_modified(SystemTime::now() - written),
            )
            .expect("stamp it");
            at
        }

        /// A symbolic link at `rel` to `target`, with its OWN stamps set to
        /// `age` ago — a link made now would make its directory young, which
        /// is the rule and not what a test of something else wants.
        pub(crate) fn link(&self, rel: &str, target: &str, age: Duration) -> PathBuf {
            let at = self.at(rel);
            if let Some(up) = at.parent() {
                std::fs::create_dir_all(up).expect("the parent");
            }
            std::os::unix::fs::symlink(target, &at).expect("a link");
            let then = SystemTime::now() - age;
            let since = then.duration_since(UNIX_EPOCH).expect("after 1970");
            let stamp = rustix::fs::Timespec {
                tv_sec: i64::try_from(since.as_secs()).expect("a date"),
                tv_nsec: 0,
            };
            rustix::fs::utimensat(
                rustix::fs::CWD,
                &at,
                &rustix::fs::Timestamps {
                    last_access: stamp,
                    last_modification: stamp,
                },
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .expect("stamp the link itself");
            at
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn days(n: u64) -> Duration {
        Duration::from_secs(n * SECONDS_IN_A_DAY)
    }

    const SECONDS_IN_A_DAY: u64 = 24 * 60 * 60;

    #[test]
    fn a_directory_is_as_old_as_its_youngest_file_and_goes_whole() {
        // THE RULE THE OWNER ASKED FOR, and the reason: a cache is rarely a
        // heap of independent files. Take the ones nobody read lately and leave
        // the rest, and a program is left holding half of an index — a class of
        // failure nobody can debug from a directory listing.
        //
        // So the unit is the entry in the ROOT, and one file somebody opened
        // yesterday keeps the whole thing.
        let tree = Tree::new("whole-directory");
        tree.plant("alive/ancient.bin", days(400));
        tree.plant("alive/deep/older.bin", days(400));
        tree.plant("alive/deep/read-yesterday.bin", days(1));
        tree.plant("dead/one.bin", days(90));
        tree.plant("dead/deep/two.bin", days(90));

        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(40),
            Act::Execute,
            &Roots::none(),
        );

        assert!(
            tree.at("alive/ancient.bin").exists(),
            "a four-hundred-day-old file was taken out from under a cache in use"
        );
        assert!(tree.at("alive/deep/read-yesterday.bin").exists());
        assert!(
            !tree.at("dead").exists(),
            "a directory nothing has touched in three months stayed"
        );
        assert_eq!(
            took.dirs, 1,
            "it took something other than the one directory"
        );
        assert_eq!(took.files, 0, "it went inside and took files one by one");
        assert_eq!(took.kept, 1, "it did not count the live one as kept");
        assert!(took.bytes > 0, "it weighed nothing");
    }

    #[test]
    fn a_directory_it_cannot_see_all_of_is_one_it_does_not_delete() {
        // REFUSING TO ANSWER IS THE ONLY SAFE ANSWER about a tree one cannot
        // read: an unreadable corner could hold the file that is in use, and
        // `remove_dir_all` would take it with everything else.
        let tree = Tree::new("unreadable");
        tree.plant("blind/old.bin", days(400));
        let shut = tree.at("blind/inner");
        std::fs::create_dir_all(&shut).unwrap();
        std::fs::write(shut.join("x.bin"), b"x").unwrap();
        // NO PERMISSION TO LOOK INSIDE, which is what a cache of another
        // program's can genuinely be.
        let mut mode = std::fs::metadata(&shut).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o000);
        std::fs::set_permissions(&shut, mode).unwrap();

        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(40),
            Act::Execute,
            &Roots::none(),
        );

        assert!(
            tree.at("blind").exists(),
            "a directory with an unreadable corner was deleted anyway"
        );
        assert_eq!(took.refused, 1, "it did not say it could not answer");
        assert_eq!(took.dirs, 0);

        let mut mode = std::fs::metadata(&shut).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o755);
        let _ = std::fs::set_permissions(&shut, mode);
    }

    #[test]
    fn a_sandbox_with_nothing_in_it_proves_nothing_so_this_plants() {
        // THE DEFECT THIS CATCHES IS IN THE TEST ITSELF, and this repository
        // has met it: `dev-exits-clean` ran in a sandbox whose `/tmp` was
        // empty, every loop of the prune returned 0 by not running, the rule
        // went in green and took the session down on the first real machine.
        //
        // So every case below plants, and every one of them asserts what was
        // NOT taken as well as what was.
        let tree = Tree::new("planted");
        tree.plant("old.bin", days(60));
        tree.plant("new.bin", days(1));
        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(40),
            Act::Execute,
            &Roots::none(),
        );
        assert_eq!(took.files, 1, "it did not take the old one");
        assert_eq!(took.kept, 1, "it did not leave the new one");
        assert!(!tree.at("old.bin").exists());
        assert!(tree.at("new.bin").exists(), "today's work went");
    }

    #[test]
    fn a_link_out_of_the_root_is_neither_followed_nor_taken() {
        // THE WORST THING THIS CODE COULD DO. A `~/.cache/x -> /home` walked by
        // a sweep is somebody's home directory deleted by age. Every entry is
        // looked at with `symlink_metadata`, so a link is never a directory and
        // never a file here — it is skipped, and what it points at is never
        // opened.
        let tree = Tree::new("link");
        let outside = Tree::new("outside");
        let precious = outside.plant("do-not-touch.txt", days(400));
        std::os::unix::fs::symlink(&outside.0, tree.at("escape")).expect("a link out");
        // AND A LINK TO A FILE, which is the other half: it is not a regular
        // file, so no rule below reaches it however old it looks.
        let old = tree.plant("real-old.bin", days(40));
        std::os::unix::fs::symlink(&old, tree.at("pointer")).expect("a link to a file");

        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(20),
            Act::Execute,
            &Roots::none(),
        );

        assert!(
            precious.exists(),
            "the sweep followed a link out of its root"
        );
        assert!(outside.0.exists(), "it took the directory the link led to");
        assert!(
            tree.at("pointer").symlink_metadata().is_ok(),
            "it took a symbolic link, which is not a regular file"
        );
        assert_eq!(took.files, 1, "it took something other than the one file");
        assert!(
            !old.exists(),
            "it did not take the file it was there to take"
        );
    }

    #[test]
    fn a_stamp_in_the_future_keeps_the_file() {
        // The clock may have been wrong when it was written, and throwing away
        // what one cannot date is worse than keeping it — the rule
        // `ronoctua_notify::store::prune` already follows about a record from
        // the future.
        let tree = Tree::new("future");
        let at = tree.at("tomorrow.bin");
        std::fs::write(&at, b"x").unwrap();
        let file = std::fs::File::options().write(true).open(&at).unwrap();
        let ahead = SystemTime::now() + days(2);
        file.set_times(
            std::fs::FileTimes::new()
                .set_accessed(ahead)
                .set_modified(ahead),
        )
        .unwrap();

        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(20),
            Act::Execute,
            &Roots::none(),
        );

        assert_eq!(took.files, 0, "a file from the future was taken");
        assert_eq!(took.kept, 1);
        assert!(at.exists());
    }

    #[test]
    fn a_file_written_today_survives_a_filesystem_with_no_access_times() {
        // THE DEFECT THIS CATCHES: reading `atime` alone. On a filesystem
        // mounted `noatime` it is stamped once at creation and never moves
        // again, so this morning's work still carries an access time from the
        // day the file was made — and a rule that read only that would delete
        // it and call it a month old.
        //
        // The later of the two is what makes "last use" mean the same thing on
        // a machine that keeps access times and on one that does not.
        let tree = Tree::new("noatime");
        let fresh = tree.plant_with("written-today.bin", days(40), days(0));
        let stale = tree.plant_with("really-old.bin", days(40), days(40));

        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(20),
            Act::Execute,
            &Roots::none(),
        );

        assert!(
            fresh.exists(),
            "a file written today was taken for an access time that never moves"
        );
        assert!(!stale.exists(), "the one nothing has touched stayed");
        assert_eq!(took.files, 1);
        assert_eq!(took.kept, 1);
    }

    #[test]
    fn a_rehearsal_counts_exactly_what_it_would_take_and_takes_nothing() {
        // The shape `nix run .#install` already has, and the reason is that
        // this is the only thing in the tree that deletes a person's files: the
        // number has to be checkable before anybody believes it.
        let tree = Tree::new("rehearsal");
        tree.plant("dead/one.bin", days(90));
        tree.plant("dead/two.bin", days(90));
        tree.plant("alive/new.bin", days(2));

        let said = sweep(
            &tree.0,
            SystemTime::now(),
            days(40),
            Act::Rehearse,
            &Roots::none(),
        );
        assert_eq!(said.dirs, 1);
        assert!(said.bytes > 0);
        assert!(tree.at("dead").exists(), "a rehearsal deleted a directory");

        let did = sweep(
            &tree.0,
            SystemTime::now(),
            days(40),
            Act::Execute,
            &Roots::none(),
        );
        assert_eq!(
            (did.dirs, did.bytes),
            (said.dirs, said.bytes),
            "the rehearsal's number and the real one disagree"
        );
    }

    #[test]
    fn an_empty_directory_is_dated_by_itself() {
        // THE ONE CASE where a directory's own stamp is the honest answer: it
        // has no youngest file to be as old as. A program that made its cache
        // directory this morning and has not written into it yet must keep it;
        // one abandoned three months ago is rubbish like any other.
        let tree = Tree::new("empty");
        let fresh = tree.at("just-made");
        std::fs::create_dir_all(&fresh).unwrap();
        let old = tree.at("abandoned");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::File::open(&old)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_accessed(SystemTime::now() - days(90))
                    .set_modified(SystemTime::now() - days(90)),
            )
            .unwrap();

        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(40),
            Act::Execute,
            &Roots::none(),
        );

        assert!(fresh.exists(), "a directory made this morning was taken");
        assert!(
            !old.exists(),
            "an abandoned empty directory stayed for ever"
        );
        assert_eq!(took.dirs, 1);
    }

    #[test]
    fn a_root_that_is_not_there_is_not_a_failure() {
        // A machine that has never cached anything has nothing to sweep, and
        // saying so with an error would make a first boot look broken.
        let took = sweep(
            Path::new("/nonexistent/cache/root"),
            SystemTime::now(),
            days(20),
            Act::Execute,
            &Roots::none(),
        );
        assert_eq!(took, Swept::default());
    }

    #[test]
    fn the_age_is_the_later_of_the_two_stamps() {
        // The unit under the rule above, on its own — so a change to `sweep`
        // cannot quietly make this true by accident somewhere else.
        let tree = Tree::new("age");
        let at = tree.plant_with("x.bin", days(9), days(3));
        let meta = std::fs::symlink_metadata(&at).unwrap();
        let age = unused_for(&meta, SystemTime::now()).expect("a readable stamp");
        assert!(
            age < days(4),
            "the earlier stamp won: the file reads as {} days old",
            age.as_secs() / SECONDS_IN_A_DAY
        );
    }

    /// The listing Nix would print for these links, quoted the 2.34 way.
    fn listing_of(links: &[&Path]) -> Vec<u8> {
        let mut out = Vec::new();
        for link in links {
            out.extend_from_slice(
                format!("\"{}\" -> /nix/store/aaaa-profile\n", link.display()).as_bytes(),
            );
        }
        out
    }

    #[test]
    fn an_old_directory_holding_a_root_is_kept_and_its_neighbour_goes() {
        // THE HAZARD FOUND ON THE OWNER'S MACHINE: a `nix develop --profile`
        // link under the cache, pointing at an 8 GiB development closure. The
        // whole-entry rule would take its directory after forty days of
        // nobody touching it, and the next collection would free the closure.
        let tree = Tree::new("root");
        tree.plant("devshell/old.bin", days(90));
        let link = tree.link(
            "devshell/profile-1-link",
            "/nix/store/aaaa-profile",
            days(90),
        );
        tree.plant("unrooted/old.bin", days(90));
        let roots = Roots::from_listing(&listing_of(&[&link]));
        assert_eq!(
            roots,
            Roots::Known(
                [{
                    let m = std::fs::symlink_metadata(&link).unwrap();
                    (m.dev(), m.ino())
                }]
                .into_iter()
                .collect()
            ),
            "the listing did not name the planted link"
        );

        let took = sweep(&tree.0, SystemTime::now(), days(40), Act::Execute, &roots);

        assert!(
            link.symlink_metadata().is_ok(),
            "a GC root inside the cache was deleted with its directory"
        );
        assert!(tree.at("devshell/old.bin").exists());
        assert!(
            !tree.at("unrooted").exists(),
            "the directory with no root stayed"
        );
        assert_eq!((took.dirs, took.rooted, took.refused), (1, 1, 0));
    }

    #[test]
    fn a_link_that_is_not_a_root_does_not_keep_its_directory() {
        // THE OTHER HALF: Nix answered, and this link is not one it named — a
        // cache's own `current -> v2` — so the directory ages like any other.
        let tree = Tree::new("not-a-root");
        tree.plant("app/old.bin", days(90));
        tree.link("app/current", "old.bin", days(90));
        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(40),
            Act::Execute,
            &Roots::none(),
        );
        assert!(!tree.at("app").exists());
        assert_eq!((took.dirs, took.rooted), (1, 0));
    }

    #[test]
    fn a_link_made_while_the_walk_runs_makes_its_directory_young() {
        // THE RACE THIS CLOSES: the roots are asked for once, BEFORE the walk,
        // so a `nix build -o ~/.cache/p/result` made while it runs is in no
        // answer. Beside ninety-day-old files it would have gone with its
        // directory — and the next collection would have freed what it holds.
        // Its own fresh stamp is what keeps it.
        let tree = Tree::new("fresh-link");
        tree.plant("p/old.bin", days(90));
        let fresh = tree.link("p/result", "/nix/store/aaaa-result", days(0));
        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(40),
            Act::Execute,
            &Roots::none(),
        );
        assert!(
            fresh.symlink_metadata().is_ok(),
            "a link made a moment ago went with its old directory"
        );
        assert_eq!((took.dirs, took.kept), (0, 1), "{took:?}");
    }

    #[test]
    fn when_nix_does_not_answer_every_link_keeps_its_directory() {
        // KEEP MORE, NEVER LESS: without the daemon's answer any link could be
        // a root, and the directory holding it is not this sweep's to judge.
        let tree = Tree::new("no-nix");
        tree.plant("app/old.bin", days(90));
        tree.link("app/current", "old.bin", days(90));
        tree.plant("plain/old.bin", days(90));
        let took = sweep(
            &tree.0,
            SystemTime::now(),
            days(40),
            Act::Execute,
            &Roots::Unknown("a test".to_owned()),
        );
        assert!(tree.at("app/old.bin").exists(), "a link was taken blind");
        assert!(
            !tree.at("plain").exists(),
            "a directory with no link stayed"
        );
        assert_eq!((took.dirs, took.refused, took.rooted), (1, 1, 0));
    }

    #[test]
    fn a_listing_is_read_quoted_bare_and_held() {
        // THE SHAPES a `--print-roots` line comes in: 2.34 quotes the link the
        // way C++ streams a path, older versions print it bare, and a root a
        // process holds is a braced word — `{censored}` when it is somebody
        // else's.
        assert_eq!(
            root_line(br#""/home/a/.cache/x/profile-1-link" -> /nix/store/a-b"#),
            RootLine::Link(PathBuf::from("/home/a/.cache/x/profile-1-link"))
        );
        assert_eq!(
            root_line(br#""/home/a/odd \"name\" -> \\ here" -> /nix/store/a-b"#),
            RootLine::Link(PathBuf::from(r#"/home/a/odd "name" -> \ here"#)),
            "an escape was not taken off"
        );
        assert_eq!(
            root_line(b"/home/a/result -> /nix/store/a-b"),
            RootLine::Link(PathBuf::from("/home/a/result"))
        );
        // BARE, an arrow in the name: the store path has no space, so the
        // last arrow is the one that separates.
        assert_eq!(
            root_line(b"/home/a/x -> y -> /nix/store/a-b"),
            RootLine::Link(PathBuf::from("/home/a/x -> y"))
        );
        assert_eq!(root_line(b"{censored} -> /nix/store/a-b"), RootLine::Held);
        assert_eq!(
            root_line(br#""{censored}" -> /nix/store/a-b"#),
            RootLine::Held
        );
        assert_eq!(root_line(b"{temp:1234} -> /nix/store/a-b"), RootLine::Held);
        // A GONE LINK IS NO ROOT OF OURS, and does not make the answer unknown.
        assert_eq!(
            Roots::from_listing(
                b"\"/nonexistent/link\" -> /nix/store/a-b\n{censored} -> /nix/store/c-d\n\n"
            ),
            Roots::none()
        );
    }

    #[test]
    fn an_answer_this_cannot_read_is_no_answer() {
        // THE GUARD MUST NEVER TURN ITSELF OFF IN SILENCE. A Nix that changed
        // how it prints would otherwise come back KNOWN and EMPTY — no link a
        // root, every devshell profile under the cache fair game, and the
        // journal saying "0 holding a Nix root" as if that were a fact.
        for odd in [
            &b"'/home/a/profile-1-link' -> /nix/store/a-b\n"[..],
            b"/home/a/profile-1-link => /nix/store/a-b\n",
            b"\"/home/a/unterminated -> /nix/store/a-b\n",
            b"/home/a/profile-1-link -> nix/store/a-b\n",
            b"roots:\n\"/home/a/p\" -> /nix/store/a-b\n",
        ] {
            let got = Roots::from_listing(odd);
            assert!(
                !got.known(),
                "{:?} was read as an answer: {got:?}",
                String::from_utf8_lossy(odd)
            );
            assert!(
                got.why_unknown().is_some_and(|w| w.contains("cannot read")),
                "{got:?} does not say why"
            );
        }
        // AND A LISTING WITH NO ROOT IN IT: a machine always has one.
        for empty in [&b""[..], b"\n\n", b"  \n"] {
            assert!(!Roots::from_listing(empty).known(), "{empty:?}");
        }
    }

    #[test]
    fn the_pinned_nix_prints_roots_this_reads() {
        // THE FORMAT IS NIX'S, so the proof is Nix's: the `nix-store` this
        // distro ships, against a store of its own in a temporary directory,
        // with one planted root whose name holds a quote and an arrow. The
        // `tests` derivation sets the variable (`nix/rust.nix`); without it
        // this says so and proves nothing, the ffmpeg tests' rule.
        let Some(nix_store) = std::env::var_os("RONOCTUA_NIX_STORE") else {
            eprintln!("RONOCTUA_NIX_STORE is not set; the listing's format was not checked");
            return;
        };
        let tree = Tree::new("pinned-nix");
        let file = tree.plant("content.txt", days(0));
        let nix = |args: &[&std::ffi::OsStr]| {
            let out = std::process::Command::new(&nix_store)
                .args(args)
                .env("HOME", tree.at("home"))
                .env("NIX_CONF_DIR", tree.at("conf"))
                .env("NIX_USER_CONF_FILES", "/dev/null")
                .env(
                    "NIX_REMOTE",
                    format!("local?root={}", tree.at("store").display()),
                )
                .output()
                .expect("run the pinned nix-store");
            assert!(
                out.status.success(),
                "nix-store {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            out.stdout
        };
        std::fs::create_dir_all(tree.at("home")).unwrap();
        std::fs::create_dir_all(tree.at("conf")).unwrap();
        let added = nix(&["--add".as_ref(), file.as_os_str()]);
        let path = String::from_utf8(added).unwrap().trim().to_owned();
        let link = tree.at("odd \"dir\"/profile -> 1-link");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        nix(&[
            "--realise".as_ref(),
            path.as_ref(),
            "--add-root".as_ref(),
            link.as_os_str(),
        ]);
        let listing = nix(&["--gc".as_ref(), "--print-roots".as_ref()]);
        let meta = std::fs::symlink_metadata(&link).expect("the root link");
        assert_eq!(
            Roots::from_listing(&listing),
            Roots::Known([(meta.dev(), meta.ino())].into_iter().collect()),
            "the pinned nix printed a listing this reads otherwise: {}",
            String::from_utf8_lossy(&listing)
        );
    }

    #[test]
    fn a_mount_is_told_from_a_directory() {
        // A BIND MOUNT KEEPS ITS DEVICE NUMBER, so the walks ask the kernel
        // where a mount begins. `/` begins one everywhere, a sandbox's
        // included; a directory this test made begins none.
        let tree = Tree::new("mount");
        std::fs::create_dir_all(tree.at("plain")).unwrap();
        assert_eq!(mount_root(&tree.at("plain")), Some(false));
        assert_eq!(mount_root(&tree.at("not-there")), None);
        let root = std::fs::symlink_metadata("/").unwrap();
        if mount_root(Path::new("/")) == Some(true) {
            assert!(
                !on_disk(Path::new("/"), &root, root.dev()),
                "the start of a mount was taken for a directory of the same one"
            );
        }
        let plain = std::fs::symlink_metadata(tree.at("plain")).unwrap();
        assert!(on_disk(&tree.at("plain"), &plain, plain.dev()));
    }

    #[test]
    fn a_program_that_is_not_there_is_an_unknown_answer() {
        let got = Roots::asking(Path::new("/nonexistent/bin/nix-store"));
        assert!(!got.known());
        assert!(
            got.why_unknown()
                .is_some_and(|w| w.contains("/nonexistent/bin/nix-store")),
            "the reason does not name what failed: {got:?}"
        );
    }

    #[test]
    fn the_sheet_is_json_the_brain_reads_and_a_trouble_is_escaped() {
        // THE BRAIN PARSES THIS with serde, and a verbatim error from a
        // program — quotes, a newline — must not break the document.
        let tree = Tree::new("sheet");
        let at = tree.at("r/sheet.json");
        let steps = [StepSheet {
            id: "store-garbage",
            ok: false,
            seconds: 3,
            trouble: "error: \"x\"\n\tfailed\u{1}".to_owned(),
        }];
        write_sheet(
            &at,
            UNIX_EPOCH + Duration::from_secs(10),
            None,
            &steps,
            "a \\ b",
        );
        let body = std::fs::read_to_string(&at).expect("the sheet");
        assert_eq!(
            body,
            r#"{"began":10,"ended":0,"steps":[{"id":"store-garbage","ok":false,"seconds":3,"trouble":"error: \"x\"\n\tfailed\u0001"}],"trouble":"a \\ b"}"#
        );
        let left: Vec<_> = std::fs::read_dir(tree.at("r")).unwrap().collect();
        assert_eq!(left.len(), 1, "a draft was left beside the sheet");
    }
}
