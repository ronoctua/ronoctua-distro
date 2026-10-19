//! The one listing of what goes to the repository (feature `prefs`).
//!
//! # From the walk itself
//!
//! [`inventory`] meets every path the sync's walk meets ([`Scope::meet`]),
//! and says of each, one row per file: where it comes from (a rule of the
//! table, a target, what the person added here or on another machine, the
//! plain part — [`Origin`]), whether this machine keeps it ([`Here`]), its
//! size, and its state against what the last sync left ([`State`]). The page
//! "What goes to your repository" draws this and nothing else, so it cannot
//! promise a file the sync leaves out, nor leave out one it sends
//! (`belongings-list-is-the-walk`).
//!
//! # By `stat` alone
//!
//! A door computes it in a thread of its own, and reads no file's content —
//! a home can keep a lot. "Changed" is the file's `lstat` against its base:
//! another kind, size or mode is a change; the same is compared by the time
//! the file last changed with the one the run that left the base recorded
//! ([`Last::modified_s`], [`stamp_sealed`], [`stamp_plain`]) — and, for a base
//! that recorded none, with the time of that run. The sync itself reads
//! every byte; the page says what a `stat` can tell, and the next sync is
//! exact.
//!
//! What is never kept for what it IS (a link into the system's store, the
//! access key under another name) is found as the walk finds it
//! (`crate::sift`): a hard link by its inode, a copy by its SHA-256 — summed
//! only when its size is an access file's, and read into memory that is
//! wiped when it is dropped: the bytes are a private key's.
//!
//! # What leaves
//!
//! A file the last sync left that is not here any more is a row too
//! ([`State::Deleted`]): the next sync deletes it from the repository, and
//! every other machine deletes its own copy — the one change of this page
//! that reaches every machine is the last one to hide.

use std::collections::BTreeSet;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;

use crate::kept::{self, Verdict};
use crate::places::{Places, covers};
use crate::plan::Kind;
pub use crate::scope::Origin;
use crate::scope::{Met, Scope};
use crate::sift;
use crate::status::Last;

/// The reason of a plain file past `plainMostKiB`
/// (`belongings-reason-too-large`): left out of the plain part, on both
/// sides.
pub const TOO_LARGE: &str = "too-large";

/// Every reason this module gives — each a sentence in every language.
pub const REASONS: [&str; 1] = [TOO_LARGE];

/// Which part of the repository a path goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    /// Sealed to the person's recipient.
    Sealed,
    /// In clear, under `plain/`.
    Plain,
}

/// Whether this machine keeps a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Here {
    /// Kept: it goes to the repository.
    Kept,
    /// The person took it out on this machine (`excluded`): the repository's
    /// copy stays for the others.
    NotHere,
    /// Its target is kept on no machine any more (or the person asked that
    /// of the next sync).
    Nowhere,
    /// Never kept, and why (`belongings-reason-<id>`).
    Never(&'static str),
}

impl Here {
    /// Whether it travels.
    #[must_use]
    pub const fn travels(self) -> bool {
        matches!(self, Self::Kept)
    }
}

/// A path against what the last sync left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    /// As the last sync left it.
    Synced,
    /// Changed since: the next sync sends it (or takes it out).
    Changed,
    /// Not in the repository yet: the next sync sends it.
    New,
    /// A prepared sync waits on the person for it, or a plain run left it
    /// changed on both sides.
    Conflict,
    /// It cannot be read here: the sync leaves it where it is, on both
    /// sides.
    Unreadable,
    /// A lenient read repaired part of it, and nobody answered yet
    /// ("write the fix" or "leave it").
    Repaired,
    /// The last sync left it and it is not here any more: the next sync
    /// deletes it from the repository, and every other machine its copy.
    Deleted,
}

impl State {
    /// Every one, for the census of their words.
    pub const ALL: [Self; 7] = [
        Self::Synced,
        Self::Changed,
        Self::New,
        Self::Conflict,
        Self::Unreadable,
        Self::Repaired,
        Self::Deleted,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Synced => "synced",
            Self::Changed => "changed",
            Self::New => "new",
            Self::Conflict => "conflict",
            Self::Unreadable => "unreadable",
            Self::Repaired => "repaired",
            Self::Deleted => "deleted",
        }
    }

    /// Whether it is something the person should look at — a deletion
    /// included: it reaches every machine.
    #[must_use]
    pub const fn troubles(self) -> bool {
        matches!(
            self,
            Self::Conflict | Self::Unreadable | Self::Repaired | Self::Deleted
        )
    }
}

/// One row of the listing: one path the walk met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Its logical path.
    pub path: String,
    /// Which part of the repository.
    pub part: Part,
    /// Where it comes from.
    pub origin: Origin,
    /// Whether this machine keeps it.
    pub here: Here,
    /// Its size, in bytes (a link's: its target's length; a folder that
    /// cannot be listed: 0; a file deleted here: what the last sync left).
    pub size_bytes: u64,
    /// Against the last sync.
    pub state: State,
    /// Whether it is a FOLDER — one the walk did not enter: a folder that
    /// cannot be listed, or one a target skips. Drawn ending in `/`, like
    /// every folder of the page.
    pub folder: bool,
}

/// The listing of what goes to the repository from this machine, by path —
/// every path the walk meets that anything names, and every file the last
/// sync left that the walk does not meet any more. `stat` alone, but for
/// the sum of a file the size of an access file. Slow on a large home: a
/// door's thread's.
#[must_use]
pub fn inventory(scope: &Scope, places: &Places) -> Vec<Item> {
    let access = Access::of(places);
    let repaired: BTreeSet<String> = places.repaired().into_iter().collect();
    let mut out = Vec::new();
    let mut here = Present::default();
    scope.meet(places, &mut |logical, at, met| {
        here.met(logical, met);
        if let Some(item) = item_of(scope, &access, &repaired, logical, at, met) {
            out.push(item);
        }
    });
    out.extend(deleted(scope, &here));
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// What the walk met, as each part's walk takes it: a SEALED path is there
/// when it is a file or a link (`crate::walk::side_of`), a PLAIN one when it
/// is a regular file (the plain walk carries no link) — and nothing under a
/// folder that could not be listed is missing.
#[derive(Default)]
struct Present {
    sealed: BTreeSet<String>,
    plain: BTreeSet<String>,
    hidden: Vec<String>,
}

impl Present {
    fn met(&mut self, logical: &str, met: Met<'_>) {
        match met {
            Met::Hidden => self.hidden.push(logical.to_owned()),
            Met::Skipped(_) => {}
            Met::Entry(meta) => {
                if meta.is_file() || meta.file_type().is_symlink() {
                    self.sealed.insert(logical.to_owned());
                }
                if meta.is_file() {
                    self.plain.insert(logical.to_owned());
                }
            }
        }
    }

    /// Whether `path` is not here for its part's walk — and not hidden.
    fn gone(&self, path: &str, plain: bool) -> bool {
        let met = if plain { &self.plain } else { &self.sealed };
        !met.contains(path) && !self.hidden.iter().any(|h| covers(h, path))
    }
}

/// The files the last sync left that the walk did not meet, and this
/// machine still keeps: the merge takes a file missing here and unchanged in
/// the repository for a deletion (`crate::merge::plan`, `DeleteRemote`), and
/// every other machine follows it. A file of a prepared sync's conflicts is
/// in conflict; a target no machine keeps leaves with its drop, and is no
/// deletion of this machine's.
fn deleted(scope: &Scope, here: &Present) -> Vec<Item> {
    let Some(base) = scope.base() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (path, known) in &base.entries {
        if kept::plain_root(path).is_some()
            || !here.gone(path, false)
            || !scope.kept_here(path)
            || kept::target_of(path).is_some_and(|t| scope.dropped(t.id))
        {
            continue;
        }
        let Some(origin) = scope.origin_of(path) else {
            continue;
        };
        out.push(Item {
            path: path.clone(),
            part: Part::Sealed,
            origin,
            here: Here::Kept,
            size_bytes: known.size_bytes,
            state: if scope.waits_on(path) {
                State::Conflict
            } else {
                State::Deleted
            },
            folder: false,
        });
    }
    for (path, known) in &base.plain.files {
        let Some(root) = kept::plain_root(path) else {
            continue;
        };
        if !here.gone(path, true) || matches!(kept::by_default(path), Verdict::Never(_)) {
            continue;
        }
        let clashing = scope.waits_on(path) || base.plain.conflicts.iter().any(|c| c == path);
        out.push(Item {
            path: path.clone(),
            part: Part::Plain,
            origin: Origin::Plain(root.path),
            here: Here::Kept,
            size_bytes: known.size_bytes,
            state: if clashing {
                State::Conflict
            } else {
                State::Deleted
            },
            folder: false,
        });
    }
    out
}

/// The row of one path the walk met — a file, a link, or a folder a target
/// skips; `None` for what is not a file of the listing (a socket, a pipe,
/// any other folder, a link in the plain part) or that nothing names any
/// more.
fn item_of(
    scope: &Scope,
    access: &Access,
    repaired: &BTreeSet<String>,
    logical: &str,
    at: &Path,
    met: Met<'_>,
) -> Option<Item> {
    let plain = kept::plain_root(logical);
    let meta = match met {
        Met::Hidden => {
            // A FOLDER THAT CANNOT BE LISTED: the walk takes it for every path
            // under it — the plain walk always, the sealed one when it keeps
            // it.
            if plain.is_none() && !scope.kept_here(logical) {
                return None;
            }
            return Some(Item {
                path: logical.to_owned(),
                part: if plain.is_some() {
                    Part::Plain
                } else {
                    Part::Sealed
                },
                origin: scope.origin_of(logical)?,
                here: Here::Kept,
                size_bytes: 0,
                state: State::Unreadable,
                folder: true,
            });
        }
        // A FOLDER A TARGET SKIPS (`Scope::meet` goes no further into it):
        // one row, never kept, and why — never one per file in it.
        Met::Skipped(why) => {
            return Some(Item {
                path: logical.to_owned(),
                part: Part::Sealed,
                origin: scope.origin_of(logical)?,
                here: Here::Never(why),
                size_bytes: 0,
                state: State::Synced,
                folder: true,
            });
        }
        Met::Entry(meta) => meta,
    };
    let link = meta.file_type().is_symlink();
    if !link && !meta.is_file() {
        return None;
    }
    let base = scope.base();
    if let Some(root) = plain {
        // THE PLAIN PART carries regular files, never a link.
        if link {
            return None;
        }
        let here = match kept::by_default(logical) {
            Verdict::Never(n) => Here::Never(n.reason),
            _ if meta.len() > kept::PLAIN_MOST_BYTES => Here::Never(TOO_LARGE),
            _ => Here::Kept,
        };
        let known = base.and_then(|b| b.plain.files.get(logical));
        let state = if here.travels() {
            if std::fs::File::open(at).is_err() {
                State::Unreadable
            } else if scope.waits_on(logical)
                || base.is_some_and(|b| b.plain.conflicts.iter().any(|c| c == logical))
            {
                State::Conflict
            } else if repaired.contains(logical) {
                State::Repaired
            } else {
                match known {
                    None => State::New,
                    Some(k) if k.size_bytes != meta.len() => State::Changed,
                    Some(_) => by_time(base, logical, meta),
                }
            }
        } else if known.is_some() {
            State::Changed
        } else {
            State::Synced
        };
        return Some(Item {
            path: logical.to_owned(),
            part: Part::Plain,
            origin: Origin::Plain(root.path),
            here,
            size_bytes: meta.len(),
            state,
            folder: false,
        });
    }
    let origin = scope.origin_of(logical)?;
    let size_bytes = if link {
        std::fs::read_link(at).map_or(0, |t| t.as_os_str().as_bytes().len() as u64)
    } else {
        meta.len()
    };
    let here = if scope.kept_here(logical) {
        // WHAT A FILE IS, as the walk sifts it: a link into the system's
        // store, the access key under another name.
        if let Some(why) = never_for_what_it_is(at, meta, link, access) {
            Here::Never(why)
        } else if kept::target_of(logical).is_some_and(|t| scope.dropped(t.id)) {
            Here::Nowhere
        } else {
            Here::Kept
        }
    } else {
        match kept::by_default(logical) {
            Verdict::Never(n) if n.hard => Here::Never(n.reason),
            Verdict::Skipped(_, why) => Here::Never(why),
            _ if kept::excluded_here(logical, scope.prefs()) => Here::NotHere,
            Verdict::Never(n) => Here::Never(n.reason),
            _ => return None,
        }
    };
    let known = base.and_then(|b| b.entries.get(logical));
    let state = if here.travels() {
        let unreadable = if link {
            std::fs::read_link(at).is_err()
        } else {
            std::fs::File::open(at).is_err()
        };
        if scope.waits_on(logical) {
            State::Conflict
        } else if unreadable {
            State::Unreadable
        } else if repaired.contains(logical) {
            State::Repaired
        } else {
            match known {
                None => State::New,
                Some(k) => {
                    let (kind, mode) = if link {
                        (Kind::Link, 0o777)
                    } else {
                        (
                            Kind::File,
                            kept::kept_mode(logical, meta.permissions().mode() & 0o7777),
                        )
                    };
                    if k.kind != kind || k.size_bytes != size_bytes || k.mode != mode {
                        State::Changed
                    } else {
                        by_time(base, logical, meta)
                    }
                }
            }
        }
    } else if known.is_some() {
        // NOT KEPT, AND STILL IN THE BASE: the next sync takes it out.
        State::Changed
    } else {
        State::Synced
    };
    Some(Item {
        path: logical.to_owned(),
        part: Part::Sealed,
        origin,
        here,
        size_bytes,
        state,
        folder: false,
    })
}

/// A file alike with its base by kind, size and mode, judged by when it last
/// changed: the time the run that left the base recorded for it, else that
/// run's own.
fn by_time(base: Option<&Last>, logical: &str, meta: &std::fs::Metadata) -> State {
    let Some(base) = base else {
        return State::New;
    };
    let now = modified_s(meta);
    let alike = base
        .modified_s
        .get(logical)
        .map_or(now <= base.at_s, |was| *was == now);
    if alike { State::Synced } else { State::Changed }
}

/// When a file last changed, in seconds since the epoch.
fn modified_s(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Why a path the rules keep is never kept for what it IS — the walk's
/// `crate::sift`: a link into the system's store; the deploy key, its public
/// half or the token, by inode or by bytes.
fn never_for_what_it_is(
    at: &Path,
    meta: &std::fs::Metadata,
    link: bool,
    access: &Access,
) -> Option<&'static str> {
    if link {
        let target = std::fs::read_link(at).ok()?;
        return sift::into_the_store(at, &target).then_some(sift::SYSTEM_MANAGED);
    }
    access.is(at, meta).then_some(sift::ACCESS)
}

/// The access files of this machine, as the listing compares against them —
/// never their bytes: a door keeps this for the length of a count, and the
/// deploy key is a private key.
struct Access {
    /// Device and inode of each that is there.
    same: Vec<(u64, u64)>,
    /// The size and SHA-256 of each that could be read and holds anything: a
    /// copy is caught by its sum, and a file is summed only when its size is
    /// one of these.
    sums: Vec<(u64, String)>,
}

impl Access {
    fn of(places: &Places) -> Self {
        let mut out = Self {
            same: Vec::new(),
            sums: Vec::new(),
        };
        for path in [places.deploy_key(), places.deploy_public(), places.token()] {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            out.same.push((meta.dev(), meta.ino()));
            // AN EMPTY ACCESS FILE holds no secret, and every empty file
            // would be its copy (`crate::sift` says the same).
            if meta.len() > 0
                && let Some(sum) = sum_of(&path, meta.len())
            {
                out.sums.push((meta.len(), sum));
            }
        }
        out
    }

    /// Whether the file at `at` is an access file: the same one, or a copy.
    fn is(&self, at: &Path, meta: &std::fs::Metadata) -> bool {
        if self.same.contains(&(meta.dev(), meta.ino())) {
            return true;
        }
        let size = meta.len();
        if !self.sums.iter().any(|(s, _)| *s == size) {
            return false;
        }
        sum_of(at, size).is_some_and(|sum| self.sums.iter().any(|(s, x)| *s == size && *x == sum))
    }
}

/// The SHA-256 of the file at `at`, `size` bytes long, read into memory
/// wiped when it is dropped — it may be a private key. The room is the size
/// and one byte, so the read that finds the end grows nothing (a grown
/// buffer would leave its first copy behind, unwiped).
fn sum_of(at: &Path, size: u64) -> Option<String> {
    use std::io::Read as _;
    let room = usize::try_from(size).ok()?.checked_add(1)?;
    let mut file = std::fs::File::open(at).ok()?;
    let mut bytes = zeroize::Zeroizing::new(Vec::with_capacity(room));
    file.read_to_end(&mut bytes).ok()?;
    Some(ronoctua_store::repair::sha256_hex(&bytes))
}

/// The time each SEALED file of `last` last changed here, recorded as the run
/// that wrote `last` leaves it ([`Last::modified_s`]): a file that last
/// changed by the second the run began (`since_s`, what the run then read),
/// or one the run wrote (`written`) — and whose size is still its base's.
/// One changed while the run went on, later than that second and not by it,
/// is left without one: the page says it changed.
pub fn stamp_sealed(places: &Places, last: &mut Last, since_s: i64, written: &BTreeSet<String>) {
    last.modified_s
        .retain(|path, _| kept::plain_root(path).is_some());
    let stamped: Vec<(String, i64)> = last
        .entries
        .iter()
        .filter_map(|(path, k)| {
            let at = places.resolve(path)?;
            let when = alike_since(&at, k.size_bytes, k.kind == Kind::Link)?;
            (when <= since_s || written.contains(path)).then(|| (path.clone(), when))
        })
        .collect();
    last.modified_s.extend(stamped);
}

/// The same for the PLAIN part of `last` (its `plain.files`), the sealed
/// stamps as they were.
pub fn stamp_plain(places: &Places, last: &mut Last, since_s: i64, written: &BTreeSet<String>) {
    last.modified_s
        .retain(|path, _| kept::plain_root(path).is_none());
    let stamped: Vec<(String, i64)> = last
        .plain
        .files
        .iter()
        .filter_map(|(path, k)| {
            let at = places.resolve(path)?;
            let when = alike_since(&at, k.size_bytes, false)?;
            (when <= since_s || written.contains(path)).then(|| (path.clone(), when))
        })
        .collect();
    last.modified_s.extend(stamped);
}

/// When the file at `at` last changed, when it is of the `link` kind and
/// `size_bytes` long; `None` otherwise.
fn alike_since(at: &Path, size_bytes: u64, link: bool) -> Option<i64> {
    let meta = std::fs::symlink_metadata(at).ok()?;
    let size = if link {
        if !meta.file_type().is_symlink() {
            return None;
        }
        std::fs::read_link(at).ok()?.as_os_str().as_bytes().len() as u64
    } else {
        if !meta.is_file() {
            return None;
        }
        meta.len()
    };
    (size == size_bytes).then(|| modified_s(&meta))
}

/// The ledgers of the person's files a lenient read repaired that nobody
/// answered and whose file is still the one judged — what the page asks
/// "write the fix" or "leave it" of.
#[must_use]
pub fn repairs_waiting(places: &Places) -> Vec<ronoctua_store::Ledger> {
    let dir = places.repairs();
    ronoctua_store::repair::pending(&dir)
        .into_iter()
        .filter(|l| {
            places
                .resolve(&l.path)
                .and_then(|p| std::fs::read(p).ok())
                .is_some_and(|b| ronoctua_store::repair::sha256_hex(&b) == l.file_sha256)
        })
        .collect()
}

/// Writes by itself every repair nobody answered — only when this machine
/// runs the NEWEST distro among the person's machines, as the last sync left
/// `fleet.json` (the owner's rule): a value this machine cannot read is then
/// one no machine of theirs can, and the fix goes to all of them with the
/// next push. On any other machine the person decides on the page. A repair
/// the person left ("leave it") is never written. Says which files it fixed.
pub fn repair_when_newest(places: &Places) -> Vec<String> {
    let newest = crate::fleet::Fleet::read(&places.fleet()).is_some_and(|f| f.newest_here());
    if !newest {
        return Vec::new();
    }
    let dir = places.repairs();
    repairs_waiting(places)
        .into_iter()
        .filter_map(|ledger| {
            let file = places.resolve(&ledger.path)?;
            ronoctua_store::repair::apply(&dir, &file, &ledger)
                .is_ok()
                .then_some(ledger.path)
        })
        .collect()
}

// ── anchor: L21 ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::Belongings;
    use crate::requests::{Request, Requests};
    use crate::status::Known;

    fn scratch(name: &str) -> Places {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-belongings-inventory-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let p = Places::under(&root);
        std::fs::create_dir_all(&p.config).unwrap();
        std::fs::create_dir_all(&p.home).unwrap();
        p
    }

    fn write(p: &Places, logical: &str, bytes: &[u8]) {
        let at = p.resolve(logical).unwrap();
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, bytes).unwrap();
    }

    fn of<'a>(items: &'a [Item], path: &str) -> &'a Item {
        items
            .iter()
            .find(|i| i.path == path)
            .unwrap_or_else(|| panic!("{path} is not listed: {items:?}"))
    }

    /// FAILS IF: a file's state is not what a `stat` says against the base —
    /// new, as the run left it, changed (by size, or by time with the size
    /// alike), or a file of a prepared sync's conflicts — or a file taken out
    /// that the base still has is not said to change.
    #[test]
    fn a_state_is_what_a_stat_says_against_the_base() {
        let p = scratch("states");
        for (logical, bytes) in [
            ("config/same.json", b"{\"a\":1}".as_slice()),
            ("config/longer.json", b"{\"a\":12}"),
            ("config/touched.json", b"{\"a\":2}"),
            ("config/fresh.json", b"{}"),
            ("config/waits.json", b"{}"),
            ("config/out.json", b"{}"),
        ] {
            write(&p, logical, bytes);
        }
        let known = |size: u64| Known {
            size_bytes: size,
            mode: 0o644,
            ..Known::default()
        };
        let stamp = |logical: &str| {
            let meta = std::fs::metadata(p.resolve(logical).unwrap()).unwrap();
            modified_s(&meta)
        };
        let mut last = Last {
            repository: "git@example.org:a/b.git".into(),
            at_s: 0,
            ..Last::default()
        };
        for (logical, size) in [
            ("config/same.json", 7),
            ("config/longer.json", 7),
            ("config/touched.json", 7),
            ("config/waits.json", 2),
            ("config/out.json", 2),
        ] {
            last.entries.insert(logical.into(), known(size));
        }
        last.modified_s
            .insert("config/same.json".into(), stamp("config/same.json"));
        last.modified_s.insert(
            "config/touched.json".into(),
            stamp("config/touched.json") - 60,
        );
        for logical in [
            "config/same.json",
            "config/longer.json",
            "config/touched.json",
            "config/fresh.json",
            "config/waits.json",
            "config/out.json",
        ] {
            let at = p.resolve(logical).unwrap();
            std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        let prefs = Belongings {
            repository: last.repository.clone(),
            excluded: vec!["config/out.json".into()],
            ..Belongings::default()
        };
        let scope = Scope::of(&prefs, Some(&last), &Requests::default())
            .waiting_on(["config/waits.json".to_owned()]);
        let items = inventory(&scope, &p);
        assert_eq!(of(&items, "config/same.json").state, State::Synced);
        assert_eq!(of(&items, "config/longer.json").state, State::Changed);
        assert_eq!(of(&items, "config/touched.json").state, State::Changed);
        assert_eq!(of(&items, "config/fresh.json").state, State::New);
        assert_eq!(of(&items, "config/waits.json").state, State::Conflict);
        let out = of(&items, "config/out.json");
        assert_eq!((out.here, out.state), (Here::NotHere, State::Changed));
        // A BASE THAT RECORDED NO TIME: the run's own time judges.
        last.modified_s.clear();
        last.at_s = i64::MAX;
        let scope = Scope::of(&prefs, Some(&last), &Requests::default());
        let items = inventory(&scope, &p);
        assert_eq!(of(&items, "config/touched.json").state, State::Synced);
        let _ = std::fs::remove_dir_all(p.home.parent().unwrap());
    }

    /// FAILS IF: a target no machine keeps is not said so, what it skips is
    /// not said never with its reason, or a plain file past the line is not
    /// said too large.
    #[test]
    fn what_is_not_kept_says_why() {
        let p = scratch("why");
        write(&p, "home/.ssh/config", b"Host *\n");
        write(&p, "home/.ssh/agent.sock", b"");
        write(&p, "config/column-player.pid", b"1");
        write(
            &p,
            "state/person/software/icons/big.png",
            &vec![0u8; usize::try_from(kept::PLAIN_MOST_BYTES).unwrap() + 1],
        );
        write(&p, "state/person/options.json", b"{}");
        let prefs = Belongings::with_repository("git@example.org:a/b.git");
        let mut asked = Requests::default();
        asked.ask(Request::DropEverywhere {
            target: "ssh".into(),
        });
        let scope = Scope::of(&prefs, None, &asked);
        let items = inventory(&scope, &p);
        let config = of(&items, "home/.ssh/config");
        assert_eq!(config.here, Here::Nowhere);
        assert_eq!(config.origin, Origin::Target("ssh"));
        assert_eq!(
            of(&items, "home/.ssh/agent.sock").here,
            Here::Never("socket")
        );
        assert_eq!(
            of(&items, "config/column-player.pid").here,
            Here::Never("draft")
        );
        let big = of(&items, "state/person/software/icons/big.png");
        assert_eq!((big.part, big.here), (Part::Plain, Here::Never(TOO_LARGE)));
        let options = of(&items, "state/person/options.json");
        assert_eq!((options.here, options.state), (Here::Kept, State::New));
        let _ = std::fs::remove_dir_all(p.home.parent().unwrap());
    }

    /// FAILS IF: the person's Emacs configuration is not listed as going
    /// sealed with its target, a package folder Emacs made is walked (one
    /// row per file in it) instead of said once — as a folder, with its own
    /// reason — a compiled file beside the configuration is listed as going,
    /// or what is carried out of a skipped folder is not listed as going.
    #[test]
    fn a_folder_a_target_skips_is_one_row() {
        let p = scratch("skipped-folder");
        write(&p, "home/.config/emacs/init.el", b"(setq x 1)\n");
        write(&p, "home/.config/emacs/lisp/mine.el", b"(provide 'mine)\n");
        write(&p, "home/.config/emacs/lisp/mine.elc", b"compiled");
        for file in ["magit.el", "magit-pkg.el", "magit.elc"] {
            write(
                &p,
                &format!("home/.config/emacs/elpa/magit-4.1/{file}"),
                b";;",
            );
        }
        write(&p, "home/.config/emacs/eln-cache/30.2-1a/magit.eln", b"elf");
        write(
            &p,
            "home/.config/emacs/transient/values.el",
            b"((magit-log))",
        );
        write(&p, "home/.config/emacs/transient/history.el", b"()");
        write(&p, "home/.config/emacs/straight/versions/default.el", b"()");
        write(&p, "home/.config/emacs/straight/repos/x/x.el", b";;");
        let scope = Scope::of(&Belongings::default(), None, &Requests::default());
        let items = inventory(&scope, &p);
        for kept in [
            "home/.config/emacs/init.el",
            "home/.config/emacs/lisp/mine.el",
            "home/.config/emacs/transient/values.el",
            "home/.config/emacs/straight/versions/default.el",
        ] {
            let item = of(&items, kept);
            assert_eq!(
                (item.part, item.here, item.state, &item.origin, item.folder),
                (
                    Part::Sealed,
                    Here::Kept,
                    State::New,
                    &Origin::Target("emacs"),
                    false
                ),
                "{kept}"
            );
        }
        for (never, why, folder) in [
            ("home/.config/emacs/elpa", "packages", true),
            ("home/.config/emacs/eln-cache", "compiled", true),
            ("home/.config/emacs/straight/repos", "packages", true),
            ("home/.config/emacs/lisp/mine.elc", "compiled", false),
            (
                "home/.config/emacs/transient/history.el",
                "editor-state",
                false,
            ),
        ] {
            let item = of(&items, never);
            assert_eq!(
                (item.here, item.folder),
                (Here::Never(why), folder),
                "{never}"
            );
            assert_eq!(item.origin, Origin::Target("emacs"), "{never}");
        }
        let inside: Vec<&str> = items
            .iter()
            .map(|i| i.path.as_str())
            .filter(|path| {
                path.contains("/elpa/") || path.contains("/eln-cache/") || path.contains("/repos/")
            })
            .collect();
        assert!(inside.is_empty(), "a skipped folder was walked: {inside:?}");
        let _ = std::fs::remove_dir_all(p.home.parent().unwrap());
    }

    /// FAILS IF: a run's stamps vouch for a file that changed while it ran,
    /// for one whose size is not its base's, or the plain part's stamps take
    /// the sealed ones away (and the other way round).
    #[test]
    fn a_run_stamps_only_what_it_can_vouch_for() {
        let p = scratch("stamps");
        write(&p, "config/a.json", b"{}");
        write(&p, "config/b.json", b"{}");
        write(&p, "config/c.json", b"{}");
        write(&p, "state/person/options.json", b"{}");
        let mut last = Last::default();
        for (logical, size) in [
            ("config/a.json", 2),
            ("config/b.json", 2),
            ("config/c.json", 9),
        ] {
            last.entries.insert(
                logical.into(),
                Known {
                    size_bytes: size,
                    ..Known::default()
                },
            );
        }
        last.plain.files.insert(
            "state/person/options.json".into(),
            crate::plain::PlainKnown {
                size_bytes: 2,
                ..Default::default()
            },
        );
        let written: BTreeSet<String> = ["config/b.json".to_owned()].into_iter().collect();
        // A RUN THAT BEGAN LONG AGO: a and b changed since it began; only b
        // was written by it.
        stamp_sealed(&p, &mut last, 0, &written);
        assert!(!last.modified_s.contains_key("config/a.json"));
        assert!(last.modified_s.contains_key("config/b.json"));
        assert!(
            !last.modified_s.contains_key("config/c.json"),
            "a size not its base's"
        );
        stamp_plain(&p, &mut last, i64::MAX, &BTreeSet::new());
        assert!(last.modified_s.contains_key("state/person/options.json"));
        assert!(
            last.modified_s.contains_key("config/b.json"),
            "the plain stamps took the sealed"
        );
        stamp_sealed(&p, &mut last, i64::MAX, &BTreeSet::new());
        assert!(last.modified_s.contains_key("config/a.json"));
        assert!(last.modified_s.contains_key("state/person/options.json"));
        let _ = std::fs::remove_dir_all(p.home.parent().unwrap());
    }

    /// FAILS IF: a file the last sync left and the walk does not meet any
    /// more — sealed or plain — is not listed as deleted with the size it
    /// had, or one is said deleted that the sync would not delete: a file
    /// taken out here, one of a target no machine keeps, one inside a folder
    /// that cannot be listed, or one that is still here.
    #[test]
    fn a_file_deleted_here_is_said_to_leave_the_repository() {
        use std::os::unix::fs::PermissionsExt as _;
        let p = scratch("deleted");
        write(&p, "config/still.json", b"{}");
        write(&p, "config/hidden/inside.json", b"{}");
        let known = |size: u64| Known {
            size_bytes: size,
            ..Known::default()
        };
        let mut last = Last {
            repository: "git@example.org:a/b.git".into(),
            at_s: i64::MAX,
            dropped: ["gnupg".to_owned()].into_iter().collect(),
            ..Last::default()
        };
        for (logical, size) in [
            ("config/keyboards.json", 31),
            ("config/still.json", 2),
            ("config/out.json", 5),
            ("config/hidden/inside.json", 2),
            ("home/.gnupg/pubring.kbx", 6),
            ("config/waits.json", 3),
        ] {
            last.entries.insert(logical.into(), known(size));
        }
        last.plain.files.insert(
            "state/person/options.json".into(),
            crate::plain::PlainKnown {
                size_bytes: 13,
                ..Default::default()
            },
        );
        let hidden = p.resolve("config/hidden").unwrap();
        std::fs::set_permissions(&hidden, std::fs::Permissions::from_mode(0o000)).unwrap();
        let prefs = Belongings {
            repository: last.repository.clone(),
            excluded: vec!["config/out.json".into()],
            ..Belongings::default()
        };
        let scope = Scope::of(&prefs, Some(&last), &Requests::default())
            .waiting_on(["config/waits.json".to_owned()]);
        let items = inventory(&scope, &p);
        let gone = of(&items, "config/keyboards.json");
        assert_eq!(
            (gone.here, gone.state, gone.size_bytes, gone.origin.clone()),
            (Here::Kept, State::Deleted, 31, Origin::Rule("config"))
        );
        let plain = of(&items, "state/person/options.json");
        assert_eq!(
            (plain.part, plain.state, plain.size_bytes),
            (Part::Plain, State::Deleted, 13)
        );
        assert_eq!(of(&items, "config/waits.json").state, State::Conflict);
        assert_ne!(of(&items, "config/still.json").state, State::Deleted);
        for not in ["config/out.json", "home/.gnupg/pubring.kbx"] {
            assert!(
                !items.iter().any(|i| i.path == not),
                "{not} is said deleted: {items:?}"
            );
        }
        // ROOT READS EVERYTHING: the test says so instead of passing blind.
        if std::fs::read_dir(&hidden).is_err() {
            assert!(
                !items.iter().any(|i| i.path == "config/hidden/inside.json"),
                "a file a folder hides is said deleted"
            );
        }
        std::fs::set_permissions(&hidden, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(p.home.parent().unwrap());
    }

    /// FAILS IF: a copy of an access file is not found by its sum, a file of
    /// another content and the same size is taken for one, or an EMPTY access
    /// file makes every empty file a copy of it.
    #[test]
    fn an_access_file_is_found_by_its_sum() {
        let p = scratch("access");
        std::fs::create_dir_all(p.keys()).unwrap();
        std::fs::write(p.deploy_public(), b"ssh-ed25519 AAAA deploy\n").unwrap();
        std::fs::write(p.token(), b"").unwrap();
        write(&p, "home/.ssh/copy.pub", b"ssh-ed25519 AAAA deploy\n");
        write(&p, "home/.ssh/other.pub", b"ssh-ed25519 BBBB deploy\n");
        write(&p, "home/.ssh/empty", b"");
        let access = Access::of(&p);
        assert!(
            access.sums.iter().all(|(size, _)| *size > 0),
            "an empty access file is summed"
        );
        let is = |logical: &str| {
            let at = p.resolve(logical).unwrap();
            access.is(&at, &std::fs::symlink_metadata(&at).unwrap())
        };
        assert!(is("home/.ssh/copy.pub"));
        assert!(!is("home/.ssh/other.pub"));
        assert!(!is("home/.ssh/empty"));
        let _ = std::fs::remove_dir_all(p.home.parent().unwrap());
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Mode {
        #[default]
        Calm,
        Busy,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Prefs {
        mode: Mode,
    }

    /// FAILS IF: a repair is written by itself on a machine that does not run
    /// the newest distro of the person's machines, is not written on the one
    /// that does, or a repair the person left is written anyway.
    #[test]
    fn a_repair_writes_itself_only_on_the_newest_machine() {
        let p = scratch("repairs");
        let store = |file: &'static str| {
            ronoctua_store::Store::<Prefs>::at(Some(p.config.clone()), file)
                .with_roots(Some(p.config.clone()), Some(p.state.clone()))
        };
        for file in ["broken.json", "left.json"] {
            std::fs::write(p.config.join(file), br#"{"mode": "frantic"}"#).unwrap();
            let _ = store(file).read();
        }
        let left = store("left.json").ledger().unwrap();
        ronoctua_store::repair::dismiss(&p.repairs(), &p.config.join("left.json"), &left).unwrap();
        let fleet = |mine: i64, theirs: i64| {
            std::fs::create_dir_all(p.own()).unwrap();
            let text = format!(
                r#"{{"this":"a","machines":{{"a":{{"at_s":1,"built_s":{mine}}},"b":{{"at_s":1,"built_s":{theirs}}}}}}}"#
            );
            std::fs::write(p.fleet(), text).unwrap();
        };
        fleet(100, 200);
        assert!(
            repair_when_newest(&p).is_empty(),
            "an older machine wrote a repair"
        );
        assert_eq!(repairs_waiting(&p).len(), 1);
        fleet(300, 200);
        assert_eq!(repair_when_newest(&p), ["config/broken.json"]);
        let fixed = std::fs::read_to_string(p.config.join("broken.json")).unwrap();
        assert!(fixed.contains("calm"), "{fixed}");
        let still = std::fs::read_to_string(p.config.join("left.json")).unwrap();
        assert!(
            still.contains("frantic"),
            "a repair the person left was written"
        );
        assert!(repairs_waiting(&p).is_empty());
        let _ = std::fs::remove_dir_all(p.home.parent().unwrap());
    }
}
