//! Cargo's build directories, swept from the inside.
//!
//! # Why the whole-entry sweep cannot do this
//!
//! Because a build tree is young as a whole while most of it is old. A
//! workspace built this morning touches a few hundred of its units and leaves
//! the other thousands — last month's features, a dependency a lockfile moved
//! past, the objects of a compiler nobody runs any more — exactly as they were.
//! Its youngest file is minutes old, so [`crate::sweep`] rightly never takes
//! it, and it grows for as long as the project lives: hundreds of gigabytes
//! under one directory is an ordinary result of a year of `cargo test`.
//!
//! # What goes, in this order
//!
//! 1. **A compiler's whole directory**, once another compiler is the one in
//!    use and nothing in it was touched for `old_compiler`. `nix/cargo-dirs.nix`
//!    names each toolchain's directory (`cargo-target/rustc-<version>`), so a
//!    new compiler starts a fresh one and the old one is a unit: nothing
//!    reuses an object another `rustc` wrote. Only a directory named like one
//!    ([`Compilers::prefix`]) is ever a compiler's.
//! 2. **Over the ceiling, units**, one at a time, until every cargo tree
//!    together weighs no more than `ceiling_bytes`. A unit is what one
//!    `.fingerprint/<name>-<hash>` names — its fingerprint, and the `deps/`,
//!    `build/` and `examples/` entries carrying its hash — or one directory of
//!    `incremental/`. What nothing reads again goes first ([`Candidate::rank`]):
//!    a unit a LATER build of the same crate and kind replaced, an output no
//!    fingerprint names, incremental state. Then the rest, least recently
//!    built first.
//!
//! # Why "least recently built", and why the replaced ones go before it
//!
//! A cache on the distro is mounted `noatime`, so a file's age is when it was
//! WRITTEN: a dependency compiled a year ago and linked into every build since
//! looks a year old. Taking it would rebuild it — and cargo rebuilds everything
//! that depends on an output newer than itself, so one old unit can cost the
//! whole workspace. A unit with a younger sibling of the same crate and the
//! same kind (`lib-serde`, `run-build-script-…`), a day or more younger, is the
//! one a lockfile or a feature change left behind; those go first.
//!
//! # Only under cargo's own lock
//!
//! Nothing is taken out of a cargo tree unless this holds EVERY lock file
//! cargo keeps anywhere in it (`.cargo-lock`, `.cargo-build-lock`,
//! `.cargo-artifact-lock`), exclusively and without waiting
//! (`File::try_lock`) — whatever the layout, whether this understands it or
//! not. Measured with cargo 1.97: holding any of the three makes a build print
//! "Blocking waiting for file lock" and wait — so nothing is ever taken out
//! from under a build, and a build that starts while this works waits for it.
//! A tree with a lock held is BUSY and left whole; so is one whose lock could
//! not even be tried.
//!
//! Units are taken one by one only out of a PROFILE this understands:
//! `.fingerprint/`, `deps/` and `.cargo-build-lock`, the lock cargo 1.96 and
//! later takes for the whole build. A tree with none — an older cargo, a
//! target directory, a tree some other tool made — is only ever taken WHOLE,
//! and only once nothing in it was touched for `old_compiler`.
//!
//! A rehearsal lets go of every lock as soon as it has tried it: it deletes
//! nothing, so it has nothing to keep a build out of.
//!
//! # The fingerprint first
//!
//! Removed before anything it names. Cargo rebuilds a unit whose fingerprint is
//! missing, so a run that stops half way leaves outputs nobody trusts — never a
//! fingerprint vouching for outputs that are gone.
//!
//! # What it never touches
//!
//! Anything that is not a cargo tree: the compilers' directory named by
//! `compilers`, and directories under the cache root whose `CACHEDIR.TAG` says
//! cargo wrote it. It never follows a link, never leaves the cache's
//! filesystem — a bind mount of the same one included — and keeps any unit
//! holding a link Nix counts as a root.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::{Act, Roots, Unanswered, measure, on_disk, unused_for};

/// The first line of every cache directory tag (`bford.info/cachedir`).
const SIGNATURE: &str = "Signature: 8a477f597d28d172789f06886806bc55";

/// And the line only cargo writes under it — the signature alone is shared by
/// every program that follows the convention, and their caches are not builds.
const BY_CARGO: &str = "created by cargo";

/// How deep under the cache root a build directory is looked for.
///
/// EIGHT, and the arithmetic is measured: `{workspace-path-hash}` expands to
/// TWO directory levels (`build/3a/eb282523db05b3`), so the distro's own build
/// directory sits five levels down — `cargo-target/rustc-<v>/build/ab/<hash>`.
/// A walk that stopped at four, which the first design said, would never have
/// found one.
pub const DEPTH: usize = 8;

/// The lock files cargo keeps in a profile directory.
///
/// Split by `build.build-dir` since 1.96: the build directory's profile holds
/// `.cargo-build-lock`, the target directory's the other two. A profile that
/// has all three is a tree built without a build directory.
const LOCKS: [&str; 3] = [".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock"];

/// The lock a profile must have for its units to be taken one by one.
const BUILD_LOCK: &str = ".cargo-build-lock";

/// How deep under a cargo tree a lock file is looked for: `<tree>/debug/…`
/// is two, `<tree>/<triple>/release/…` three.
const TREE_LOCK_DEPTH: usize = 3;

/// And under a compiler's directory, which holds whole trees:
/// `rustc-<v>/build/ab/<hash>/<triple>/release/…` is six.
const COMPILER_LOCK_DEPTH: usize = 6;

/// How much younger a sibling of the same crate and kind has to be for a unit
/// to count as REPLACED rather than as another part of the same build.
///
/// A DAY: one build writes every unit it needs within minutes of each other,
/// so two siblings a day apart are two builds — a lockfile or a feature set
/// that moved — and the older one is what the move left behind.
const REPLACED_BY: Duration = Duration::from_secs(24 * 60 * 60);

/// The directory of compilers, and how to tell one of them.
#[derive(Debug, Clone, Copy)]
pub struct Compilers<'a> {
    /// The directory holding one directory per compiler
    /// (`nix/cargo-dirs.nix`: `<cache>/cargo-target`).
    pub root: &'a Path,
    /// What every compiler's directory is named with (`rustc-`, from
    /// `nix/cargo-dirs.nix`). Nothing else in `root` is ever taken whole: a
    /// root given by mistake loses nothing that is not named like a compiler.
    pub prefix: &'a str,
    /// The compiler directory the session builds with, which is never taken
    /// whole — `rustc-<version>` of `nix/cargo-dirs.nix`.
    pub current: Option<&'a str>,
}

/// Where to look, how much is too much, and whether to act.
#[derive(Debug)]
pub struct Cargo<'a> {
    /// The cache root, walked for directories cargo tagged.
    pub cache: &'a Path,
    /// The compilers' directory. `None` skips the compiler rule.
    pub compilers: Option<Compilers<'a>>,
    /// The most every cargo tree may weigh together.
    pub ceiling_bytes: u64,
    /// How long a compiler's directory — or a tree without cargo's lock —
    /// must go untouched before it may go whole.
    pub old_compiler: Duration,
    /// When "now" is. A parameter, for the reason [`crate::sweep`] gives.
    pub now: SystemTime,
    /// Whether to take anything.
    pub act: Act,
    /// What Nix counts as a root.
    pub roots: &'a Roots,
}

/// What one sweep of the cargo trees found and did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CargoSwept {
    /// Every cargo tree together, before, in bytes — a hard link counted once.
    pub before: u64,
    /// And after (or after, had it acted).
    pub after: u64,
    /// Units taken one by one: a fingerprint and what it names, a directory
    /// of `incremental/`, a file cargo uplifted.
    pub units: u64,
    /// Directories taken whole: a compiler's, or a tree cargo's lock does not
    /// cover.
    pub dirs: u64,
    /// What removing them freed — only files with no other link count, since
    /// a hard link elsewhere keeps the blocks.
    pub bytes: u64,
    /// Trees a cargo was building in, left alone.
    pub busy: u64,
    /// Kept for holding a link Nix counts as a root.
    pub rooted: u64,
    /// Could not be read, locked or removed.
    pub refused: u64,
}

/// One thing that may go.
#[derive(Debug)]
struct Candidate {
    /// Since it was last used.
    age: Duration,
    /// Which goes first: `0` for what nothing reads again (a replaced unit, an
    /// output no fingerprint names, incremental state, a whole tree past its
    /// age), `1` for what a build may still use. Lower first, then oldest.
    rank: u8,
    /// What removing it frees.
    frees: u64,
    /// What goes, in order — a fingerprint first.
    paths: Vec<PathBuf>,
    /// A directory taken whole, rather than a unit.
    whole: bool,
}

/// Sweeps every cargo tree under the cache.
///
/// See the module header for the order and the refusals.
#[must_use]
pub fn sweep(c: &Cargo<'_>) -> CargoSwept {
    let mut out = CargoSwept::default();
    let Ok(top) = std::fs::symlink_metadata(c.cache) else {
        // NO CACHE IS NOTHING TO DO, not a failure — the rule `crate::sweep`
        // already follows.
        return out;
    };
    if !top.is_dir() {
        out.refused += 1;
        return out;
    }
    let disk = top.dev();

    let mut tagged = builds(c.cache);
    let mut trees: Vec<PathBuf> = Vec::new();
    if let Some(k) = c.compilers
        && is_dir_on(k.root, disk)
    {
        trees.push(k.root.to_path_buf());
    }
    trees.extend(
        tagged
            .iter()
            .filter(|b| !c.compilers.is_some_and(|k| b.starts_with(k.root)))
            .cloned(),
    );
    out.before = weight_of(&trees, disk);
    let mut total = out.before;

    // 1. THE COMPILERS NOBODY USES.
    let mut gone: Vec<PathBuf> = Vec::new();
    if let Some(k) = c.compilers {
        for (dir, frees, locks) in old_compilers(c, &k, disk, &mut out) {
            let failed = c.act == Act::Execute && std::fs::remove_dir_all(&dir).is_err();
            // THE LOCKS WERE HELD UNTIL NOW, so no build started in it between
            // the look and the removal.
            drop(locks);
            if failed {
                out.refused += 1;
                continue;
            }
            out.dirs += 1;
            out.bytes = out.bytes.saturating_add(frees);
            total = total.saturating_sub(frees);
            gone.push(dir);
        }
    }
    tagged.retain(|b| !gone.iter().any(|g| b.starts_with(g)));

    // 2. OVER THE CEILING: WHAT NOTHING READS AGAIN, THEN THE LEAST RECENTLY
    // BUILT.
    if total > c.ceiling_bytes {
        // THE LOCKS ARE HELD UNTIL THE END OF THIS BLOCK, for every tree at
        // once: which unit goes first is a question across all of them, and a
        // build starting half way through would be one this was deleting from.
        let mut held: Vec<File> = Vec::new();
        let mut candidates: Vec<Candidate> = Vec::new();
        for build in &tagged {
            candidates.extend(inside(c, build, disk, &mut held, &mut out));
        }
        candidates.sort_by_key(|c| (c.rank, std::cmp::Reverse(c.age)));
        for candidate in candidates {
            if total <= c.ceiling_bytes {
                break;
            }
            if c.act == Act::Execute && !take(&candidate.paths) {
                out.refused += 1;
                continue;
            }
            if candidate.whole {
                out.dirs += 1;
            } else {
                out.units += 1;
            }
            out.bytes = out.bytes.saturating_add(candidate.frees);
            total = total.saturating_sub(candidate.frees);
        }
        drop(held);
    }
    out.after = total;
    out
}

/// Every directory under `cache`, down to [`DEPTH`], that cargo tagged.
///
/// It never follows a link, never leaves the cache's filesystem, and never
/// descends into a tree it found: a build directory is one unit of this walk.
#[must_use]
pub fn builds(cache: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(top) = std::fs::symlink_metadata(cache) else {
        return out;
    };
    if !top.is_dir() {
        return out;
    }
    let disk = top.dev();
    // AN EXPLICIT STACK, for the reason `crate::measure` gives.
    let mut stack = vec![(cache.to_path_buf(), 0usize)];
    while let Some((here, depth)) = stack.pop() {
        if depth > 0 && tagged_by_cargo(&here) {
            out.push(here);
            continue;
        }
        if depth >= DEPTH {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&here) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() && on_disk(&path, &meta, disk) {
                stack.push((path, depth + 1));
            }
        }
    }
    out.sort();
    out
}

/// Whether `dir` holds a `CACHEDIR.TAG` that cargo wrote.
fn tagged_by_cargo(dir: &Path) -> bool {
    let tag = dir.join("CACHEDIR.TAG");
    // A REGULAR FILE, never a link to one somewhere else.
    if !std::fs::symlink_metadata(&tag).is_ok_and(|m| m.is_file() && m.len() < 4096) {
        return false;
    }
    // WITHOUT MOVING ITS ACCESS TIME. Reading it is this walk looking, not
    // anybody using the tree — and on a filesystem that keeps access times
    // the read made every tree a second old, so none was ever old enough to
    // go (found by the test of a ten-day-old compiler that stayed). `O_NOATIME`
    // is refused on a file somebody else owns; that one is read plainly,
    // which can only make it look younger — it keeps more, never less.
    let quiet = std::fs::File::options()
        .read(true)
        .custom_flags(no_atime())
        .open(&tag);
    let Ok(mut file) = quiet.or_else(|_| std::fs::File::open(&tag)) else {
        return false;
    };
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).is_ok()
        && text.starts_with(SIGNATURE)
        && text.contains(BY_CARGO)
}

/// `O_NOATIME`, as `OpenOptions::custom_flags` takes it.
fn no_atime() -> i32 {
    i32::try_from(rustix::fs::OFlags::NOATIME.bits()).unwrap_or(0)
}

/// The compilers' directories that may go whole, with what each frees and
/// every cargo lock inside — held until the caller has removed it (and let go
/// at once in a rehearsal).
fn old_compilers(
    c: &Cargo<'_>,
    k: &Compilers<'_>,
    disk: u64,
    out: &mut CargoSwept,
) -> Vec<(PathBuf, u64, Vec<File>)> {
    let Ok(entries) = std::fs::read_dir(k.root) else {
        return Vec::new();
    };
    let mut aged: Vec<(PathBuf, Duration)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        // NAMED LIKE A COMPILER'S, or it is not one: whatever else sits in
        // the directory — or the whole of a directory given by mistake — is
        // never this rule's to take.
        if !entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with(k.prefix) && n.len() > k.prefix.len())
        {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_dir() || !on_disk(&path, &meta, disk) {
            continue;
        }
        match measure(&path, c.now, disk, c.roots) {
            Ok((age, _)) => aged.push((path, age)),
            Err(Unanswered::Rooted) if c.roots.known() => out.rooted += 1,
            Err(_) => out.refused += 1,
        }
    }
    // THE YOUNGEST STAYS, whatever its name: it is the one somebody built
    // with last — a rollback to an older toolchain makes that one current.
    let youngest = aged.iter().map(|(_, age)| *age).min();
    let mut out_dirs = Vec::new();
    let mut spared_youngest = false;
    for (path, age) in aged {
        if !spared_youngest && Some(age) == youngest {
            spared_youngest = true;
            continue;
        }
        // AND THE SESSION'S OWN, whatever its age: it is the directory the
        // next `cargo build` writes into.
        if k.current
            .is_some_and(|name| path.file_name().is_some_and(|n| n == name))
        {
            continue;
        }
        if age < c.old_compiler {
            continue;
        }
        // NOBODY BUILDING IN IT: every lock anywhere inside is taken first,
        // and one that is held — or that could not even be tried — keeps the
        // whole directory.
        let locks = match hold_all(&path, disk, COMPILER_LOCK_DEPTH) {
            Hold::Held(files) => files,
            Hold::Busy => {
                out.busy += 1;
                continue;
            }
            Hold::Refused => {
                out.refused += 1;
                continue;
            }
        };
        let locks = if c.act == Act::Execute {
            locks
        } else {
            Vec::new()
        };
        let frees = frees_of(&path, disk);
        out_dirs.push((path, frees, locks));
    }
    out_dirs
}

/// What may go out of one cargo tree, taking every lock in it.
fn inside(
    c: &Cargo<'_>,
    build: &Path,
    disk: u64,
    held: &mut Vec<File>,
    out: &mut CargoSwept,
) -> Vec<Candidate> {
    match hold_all(build, disk, TREE_LOCK_DEPTH) {
        Hold::Held(files) => {
            // A REHEARSAL LETS GO AT ONCE: it deletes nothing, so it has no
            // build to keep out — and holding cargo's locks for a whole walk
            // would make a person's `cargo build` wait on a question.
            if c.act == Act::Execute {
                held.extend(files);
            }
        }
        Hold::Busy => {
            out.busy += 1;
            return Vec::new();
        }
        Hold::Refused => {
            out.refused += 1;
            return Vec::new();
        }
    }
    let profiles = profiles(build, disk);
    if profiles.is_empty() {
        // A TREE WHOSE LAYOUT THIS DOES NOT TAKE APART goes whole or not at
        // all, and only once it has been left alone long enough that nothing
        // is coming back for it. Its locks, if it has any, are held above.
        return match measure(build, c.now, disk, c.roots) {
            Ok((age, _)) if age >= c.old_compiler => vec![Candidate {
                age,
                rank: 0,
                frees: frees_of(build, disk),
                paths: vec![build.to_path_buf()],
                whole: true,
            }],
            Ok(_) => Vec::new(),
            Err(Unanswered::Rooted) if c.roots.known() => {
                out.rooted += 1;
                Vec::new()
            }
            Err(_) => {
                out.refused += 1;
                Vec::new()
            }
        };
    }
    let mut found = Vec::new();
    for profile in profiles {
        found.extend(units_of(c, &profile, disk, out));
    }
    found
}

/// The profiles of a tree whose units this may take one by one: itself or a
/// directory one or two levels down (`debug`, `<triple>/release`) holding
/// `.fingerprint/`, `deps/` and `.cargo-build-lock`.
fn profiles(build: &Path, disk: u64) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut level = vec![build.to_path_buf()];
    for _ in 0..=2 {
        let mut next = Vec::new();
        for dir in level {
            if is_dir_on(&dir.join(".fingerprint"), disk) {
                if is_dir_on(&dir.join("deps"), disk) && is_file(&dir.join(BUILD_LOCK)) {
                    out.push(dir);
                }
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if is_dir_on(&path, disk) {
                    next.push(path);
                }
            }
        }
        level = next;
    }
    out.sort();
    out
}

/// Whether `path` is a directory — not a link to one, not the start of a
/// mount — on this filesystem.
fn is_dir_on(path: &Path, disk: u64) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && on_disk(path, &m, disk))
}

/// Whether `path` is a regular file — not a link to one.
fn is_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}

/// How taking a tree's locks went.
#[derive(Debug)]
enum Hold {
    /// Every lock it has — none, perhaps — is ours until these are dropped.
    Held(Vec<File>),
    /// A cargo holds one: it is building here.
    Busy,
    /// A lock file could not be opened or tried: as good as busy, since
    /// nothing says it is not.
    Refused,
}

/// Takes every cargo lock under `tree`, down to `depth`, without waiting for
/// any — whatever the layout: a tree this does not understand is still one a
/// build may be running in.
fn hold_all(tree: &Path, disk: u64, depth: usize) -> Hold {
    let mut files = Vec::new();
    // AN EXPLICIT STACK, for the reason `crate::measure` gives.
    let mut stack = vec![(tree.to_path_buf(), 0usize)];
    while let Some((here, level)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&here) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                if level + 1 < depth && on_disk(&path, &meta, disk) {
                    stack.push((path, level + 1));
                }
                continue;
            }
            if !meta.is_file() || !LOCKS.iter().any(|l| entry.file_name() == *l) {
                continue;
            }
            // READ-ONLY: `flock` does not care how the file was opened, and
            // this never creates a lock file cargo did not.
            let Ok(file) = File::open(&path) else {
                return Hold::Refused;
            };
            match file.try_lock() {
                Ok(()) => files.push(file),
                Err(std::fs::TryLockError::WouldBlock) => return Hold::Busy,
                Err(std::fs::TryLockError::Error(_)) => return Hold::Refused,
            }
        }
    }
    Hold::Held(files)
}

/// Every unit of a profile this holds the locks of.
fn units_of(c: &Cargo<'_>, profile: &Path, disk: u64, out: &mut CargoSwept) -> Vec<Candidate> {
    let mut found = Vec::new();
    // WHAT EACH HASH NAMES, listed once per directory: asking `deps/` once per
    // fingerprint would be thousands of listings of thousands of entries.
    let mut named: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for dir in ["deps", "build", "examples"] {
        let Ok(entries) = std::fs::read_dir(profile.join(dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Some(hash) = entry.file_name().to_str().and_then(hash_of) {
                named.entry(hash.to_owned()).or_default().push(entry.path());
            }
        }
    }
    // THE FINGERPRINTED UNITS, each with the crate and the kind it builds, so
    // one that a later build of the same crate and kind replaced can go first.
    let mut units: Vec<((String, String), Candidate)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(profile.join(".fingerprint")) {
        for entry in entries.flatten() {
            let fingerprint = entry.path();
            let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(hash) = hash_of(&file_name).map(str::to_owned) else {
                continue;
            };
            if !is_dir_on(&fingerprint, disk) {
                continue;
            }
            let crate_name = file_name
                .strip_suffix(hash.as_str())
                .and_then(|n| n.strip_suffix('-'))
                .unwrap_or(&file_name)
                .to_owned();
            let kind = kind_of(&fingerprint);
            let mut paths = vec![fingerprint];
            paths.extend(named.remove(&hash).unwrap_or_default());
            if let Some(unit) = candidate(c, paths, disk, 1, out) {
                units.push(((crate_name, kind), unit));
            }
        }
    }
    // THE YOUNGEST OF EACH CRATE AND KIND, which is the one the last build of
    // it wrote.
    let mut newest: HashMap<(String, String), Duration> = HashMap::new();
    for (group, unit) in &units {
        newest
            .entry(group.clone())
            .and_modify(|young| *young = (*young).min(unit.age))
            .or_insert(unit.age);
    }
    for (group, mut unit) in units {
        if newest
            .get(&group)
            .is_some_and(|young| unit.age.saturating_sub(*young) >= REPLACED_BY)
        {
            unit.rank = 0;
        }
        found.push(unit);
    }
    // AND WHAT NO FINGERPRINT NAMES — what a run of this sweep leaves when it
    // stops between a fingerprint and its outputs. Cargo reads a unit's
    // outputs only through its fingerprint, so these are never read again.
    // (Measured on cargo 1.97: 768 fingerprints, every output named by one.)
    for (_, paths) in named {
        found.extend(candidate(c, paths, disk, 0, out));
    }
    // INCREMENTAL STATE, one directory per crate: `rustc` makes it again, and
    // nothing's fingerprint depends on it.
    if let Ok(entries) = std::fs::read_dir(profile.join("incremental")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_dir_on(&path, disk) {
                found.extend(candidate(c, vec![path], disk, 0, out));
            }
        }
    }
    // WHAT CARGO UPLIFTED beside them, in a tree built without a build
    // directory — a hard link to a file in `deps/`, so taking the unit alone
    // frees nothing while it stays. Only once it is as old as a compiler
    // nobody uses: it is the file a person runs.
    if let Ok(entries) = std::fs::read_dir(profile) {
        for entry in entries.flatten() {
            let path = entry.path();
            let hidden = entry
                .file_name()
                .to_str()
                .is_none_or(|n| n.starts_with('.'));
            if hidden || !is_file(&path) {
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if unused_for(&meta, c.now).is_some_and(|age| age >= c.old_compiler) {
                found.extend(candidate(c, vec![path], disk, 1, out));
            }
        }
    }
    found
}

/// What a fingerprint builds: the name of the file cargo keeps its hash in
/// (`lib-serde`, `run-build-script-build-script-build`, `test-lib-x`), or
/// nothing when it holds none.
fn kind_of(fingerprint: &Path) -> String {
    let Ok(entries) = std::fs::read_dir(fingerprint) else {
        return String::new();
    };
    entries
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| n != "invoked.timestamp" && !n.starts_with("dep-") && !n.ends_with(".json"))
        .min()
        .unwrap_or_default()
}

/// The hash a name in a profile carries: `libfoo-0123456789abcdef.rlib`,
/// `foo-0123456789abcdef`, `foo-0123456789abcdef.d`.
///
/// Up to the first dot, after the last dash, and all hexadecimal — cargo's
/// metadata hash. A name without one is not a unit's.
fn hash_of(name: &str) -> Option<&str> {
    let stem = name.split('.').next()?;
    let (_, hash) = stem.rsplit_once('-')?;
    (hash.len() >= 8 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hash)
}

/// A candidate out of these paths, of this `rank`, or `None` when one of them
/// holds a root or cannot be read.
fn candidate(
    c: &Cargo<'_>,
    paths: Vec<PathBuf>,
    disk: u64,
    rank: u8,
    out: &mut CargoSwept,
) -> Option<Candidate> {
    let mut age = Duration::MAX;
    let mut frees = 0u64;
    for path in &paths {
        let meta = std::fs::symlink_metadata(path).ok()?;
        if meta.is_dir() {
            // A MOUNT THAT BEGINS HERE — a bind of this same filesystem
            // included — is not the unit's to take: `measure` looks inside,
            // and this is the one question it does not ask of the top.
            if !on_disk(path, &meta, disk) {
                out.refused += 1;
                return None;
            }
            match measure(path, c.now, disk, c.roots) {
                Ok((a, _)) => age = age.min(a),
                Err(Unanswered::Rooted) if c.roots.known() => {
                    out.rooted += 1;
                    return None;
                }
                Err(_) => {
                    out.refused += 1;
                    return None;
                }
            }
            frees = frees.saturating_add(frees_of(path, disk));
        } else if meta.is_file() {
            age = age.min(unused_for(&meta, c.now)?);
            if meta.nlink() == 1 {
                frees = frees.saturating_add(meta.len());
            }
        } else if c.roots.holds(&meta) {
            // A ROOT NAMED BY THE HASH itself — a `result` link somebody made.
            out.rooted += u64::from(c.roots.known());
            out.refused += u64::from(!c.roots.known());
            return None;
        } else {
            // A LINK OR A SOCKET, dated by its own stamps: one made a moment
            // ago is a unit somebody is using now — `crate::measure`'s rule.
            age = age.min(unused_for(&meta, c.now)?);
        }
    }
    Some(Candidate {
        age,
        rank,
        frees,
        paths,
        whole: false,
    })
}

/// Removes these in order, and says whether all of them went.
fn take(paths: &[PathBuf]) -> bool {
    for path in paths {
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            continue;
        };
        let gone = if meta.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        };
        match gone {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return false,
        }
    }
    true
}

/// What removing a tree frees: its files with no link outside it counted, a
/// hard link inside counted once.
fn frees_of(at: &Path, disk: u64) -> u64 {
    let mut seen: HashMap<(u64, u64), u64> = HashMap::new();
    let mut frees = 0u64;
    walk_files(at, disk, &mut |meta| {
        if meta.nlink() == 1 {
            frees = frees.saturating_add(meta.len());
        } else {
            // EVERY LINK OF IT INSIDE means the blocks go with the tree.
            let links = seen.entry((meta.dev(), meta.ino())).or_insert(0);
            *links += 1;
            if *links == meta.nlink() {
                frees = frees.saturating_add(meta.len());
            }
        }
    });
    frees
}

/// What these trees weigh together, a hard link counted once.
fn weight_of(trees: &[PathBuf], disk: u64) -> u64 {
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    let mut weight = 0u64;
    for tree in trees {
        walk_files(tree, disk, &mut |meta| {
            if meta.nlink() == 1 || seen.insert((meta.dev(), meta.ino())) {
                weight = weight.saturating_add(meta.len());
            }
        });
    }
    weight
}

/// Every regular file under `at`, on this filesystem, never through a link
/// and never into a mount.
fn walk_files(at: &Path, disk: u64, each: &mut dyn FnMut(&std::fs::Metadata)) {
    let Ok(top) = std::fs::symlink_metadata(at) else {
        return;
    };
    if top.is_file() {
        each(&top);
        return;
    }
    let mut stack = vec![at.to_path_buf()];
    while let Some(here) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&here) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if !on_disk(&path, &meta, disk) {
                continue;
            }
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                each(&meta);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{Tree, days};

    const TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55\n\
        # This file is a cache directory tag created by cargo.\n\
        # For information about cache directory tags see https://bford.info/cachedir/\n";

    /// A build directory the way cargo 1.97 lays one out under `build-dir`:
    /// the tag, and a `debug` profile holding the build lock, two units and
    /// an incremental directory, each as old as asked.
    fn plant_build(tree: &Tree, rel: &str, old: Duration, new: Duration) -> PathBuf {
        tag(tree, rel, new);
        tree.plant(&format!("{rel}/.rustc_info.json"), new);
        tree.plant(&format!("{rel}/debug/{BUILD_LOCK}"), new);
        // THE OLD UNIT: a library, its fingerprint and a build script's output.
        tree.plant(
            &format!("{rel}/debug/.fingerprint/old-1111111111111111/lib-old"),
            old,
        );
        tree.plant(
            &format!("{rel}/debug/deps/libold-1111111111111111.rlib"),
            old,
        );
        tree.plant(&format!("{rel}/debug/deps/old-1111111111111111.d"), old);
        tree.plant(
            &format!("{rel}/debug/build/old-1111111111111111/out/gen.rs"),
            old,
        );
        // THE NEW ONE.
        tree.plant(
            &format!("{rel}/debug/.fingerprint/new-2222222222222222/lib-new"),
            new,
        );
        tree.plant(
            &format!("{rel}/debug/deps/libnew-2222222222222222.rlib"),
            new,
        );
        tree.plant(
            &format!("{rel}/debug/incremental/new-3abc/s-x/query.bin"),
            new,
        );
        tree.at(rel)
    }

    /// Cargo's tag in `rel`, as old as asked — a tag written NOW would make
    /// every planted tree a minute old.
    fn tag(tree: &Tree, rel: &str, age: Duration) {
        let at = tree.plant(&format!("{rel}/CACHEDIR.TAG"), age);
        std::fs::write(&at, TAG).unwrap();
        let file = File::options().write(true).open(&at).unwrap();
        file.set_times(
            std::fs::FileTimes::new()
                .set_accessed(SystemTime::now() - age)
                .set_modified(SystemTime::now() - age),
        )
        .unwrap();
    }

    fn cargo<'a>(tree: &'a Tree, roots: &'a Roots, ceiling: u64, act: Act) -> Cargo<'a> {
        Cargo {
            cache: &tree.0,
            compilers: None,
            ceiling_bytes: ceiling,
            old_compiler: days(7),
            now: SystemTime::now(),
            act,
            roots,
        }
    }

    #[test]
    fn the_distros_build_directory_five_levels_down_is_found() {
        // THE DEFECT THIS CATCHES is in the arithmetic of the first design:
        // "a CACHEDIR.TAG at depth four or less". `{workspace-path-hash}` is
        // two levels, so the distro's own build directory is at five, and that
        // walk would have swept every cache on the machine except the one it
        // was written for.
        let tree = Tree::new("deep");
        let deep = plant_build(
            &tree,
            "cargo-target/rustc-1.97.1/build/3a/eb282523db05b3",
            days(30),
            days(0),
        );
        // AND A TAG CARGO DID NOT WRITE is not a build: the signature is the
        // convention's, shared by every program that follows it.
        std::fs::create_dir_all(tree.at("thumbnails")).unwrap();
        std::fs::write(
            tree.at("thumbnails/CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55\n# created by a viewer\n",
        )
        .unwrap();
        tree.plant(
            "thumbnails/debug/.fingerprint/x-1111111111111111/f",
            days(90),
        );
        assert_eq!(builds(&tree.0), vec![deep]);
    }

    #[test]
    fn a_compiler_nobody_uses_goes_whole_and_the_one_in_use_stays() {
        let tree = Tree::new("compiler");
        tree.plant("cargo-target/rustc-1.90.0/target/debug/app", days(10));
        plant_build(
            &tree,
            "cargo-target/rustc-1.90.0/build/aa/bbbbbbbbbbbbbb",
            days(10),
            days(10),
        );
        tree.plant("cargo-target/rustc-1.95.0/target/debug/app", days(3));
        plant_build(
            &tree,
            "cargo-target/rustc-1.97.1/build/aa/cccccccccccccc",
            days(40),
            days(0),
        );
        let roots = Roots::none();
        let compilers = tree.at("cargo-target");
        let c = Cargo {
            compilers: Some(Compilers {
                root: &compilers,
                prefix: "rustc-",
                current: Some("rustc-1.97.1"),
            }),
            ceiling_bytes: u64::MAX,
            ..cargo(&tree, &roots, 0, Act::Execute)
        };

        let took = sweep(&c);

        assert!(
            !tree.at("cargo-target/rustc-1.90.0").exists(),
            "ten days unused stayed"
        );
        assert!(
            tree.at("cargo-target/rustc-1.95.0/target/debug/app")
                .exists(),
            "three days is younger than a week"
        );
        assert!(
            tree.at("cargo-target/rustc-1.97.1").exists(),
            "the compiler in use went"
        );
        assert_eq!((took.dirs, took.units), (1, 0), "{took:?}");
        assert!(took.bytes > 0);
        assert!(took.after < took.before);
    }

    #[test]
    fn an_old_compiler_somebody_is_building_with_stays() {
        // A WEEK UNTOUCHED IS NOT PROOF that nobody is about to build: an old
        // devshell opened this minute writes into its compiler's directory,
        // and cargo's lock is what says so.
        let tree = Tree::new("compiler-busy");
        let old = plant_build(
            &tree,
            "cargo-target/rustc-1.90.0/build/aa/bbbbbbbbbbbbbb",
            days(10),
            days(10),
        );
        tree.plant("cargo-target/rustc-1.97.1/target/debug/app", days(0));
        let building = File::open(old.join("debug").join(BUILD_LOCK)).unwrap();
        building.try_lock().expect("the test holds the build lock");
        let roots = Roots::none();
        let compilers = tree.at("cargo-target");
        let c = Cargo {
            compilers: Some(Compilers {
                root: &compilers,
                prefix: "rustc-",
                current: None,
            }),
            ceiling_bytes: u64::MAX,
            ..cargo(&tree, &roots, 0, Act::Execute)
        };
        let took = sweep(&c);
        assert!(
            old.exists(),
            "a compiler cargo was building with went whole"
        );
        assert_eq!((took.dirs, took.busy), (0, 1), "{took:?}");
    }

    #[test]
    fn the_session_compiler_stays_even_when_another_was_used_later() {
        // A ROLLBACK: the older devshell was the last one used, and the
        // session's own directory is a week old. Neither goes.
        let tree = Tree::new("session");
        tree.plant("cargo-target/rustc-1.97.1/target/debug/app", days(20));
        tree.plant("cargo-target/rustc-1.96.0/target/debug/app", days(1));
        let roots = Roots::none();
        let compilers = tree.at("cargo-target");
        let c = Cargo {
            compilers: Some(Compilers {
                root: &compilers,
                prefix: "rustc-",
                current: Some("rustc-1.97.1"),
            }),
            ceiling_bytes: u64::MAX,
            ..cargo(&tree, &roots, 0, Act::Execute)
        };
        let took = sweep(&c);
        assert!(
            tree.at("cargo-target/rustc-1.97.1/target/debug/app")
                .exists()
        );
        assert!(
            tree.at("cargo-target/rustc-1.96.0/target/debug/app")
                .exists()
        );
        assert_eq!(took.dirs, 0);
    }

    #[test]
    fn over_the_ceiling_the_oldest_unit_goes_and_its_fingerprint_with_it() {
        let tree = Tree::new("ceiling");
        let build = plant_build(
            &tree,
            "cargo-target/rustc-1.97.1/build/3a/eb282523db05b3",
            days(30),
            days(0),
        );
        let roots = Roots::none();
        let disk = std::fs::symlink_metadata(&build).unwrap().dev();
        let everything = weight_of(std::slice::from_ref(&build), disk);
        let incremental = frees_of(&build.join("debug/incremental/new-3abc"), disk);
        // A CEILING ONE BYTE UNDER WHAT THE INCREMENTAL STATE LEAVES: that
        // goes first — `rustc` makes it again and nothing's fingerprint needs
        // it — and the oldest unit after it is enough.
        let took = sweep(&cargo(
            &tree,
            &roots,
            everything - incremental - 1,
            Act::Execute,
        ));

        let profile = build.join("debug");
        assert!(
            !profile.join("incremental/new-3abc").exists(),
            "incremental state stayed while a unit went"
        );
        assert!(!profile.join(".fingerprint/old-1111111111111111").exists());
        assert!(!profile.join("deps/libold-1111111111111111.rlib").exists());
        assert!(!profile.join("deps/old-1111111111111111.d").exists());
        assert!(!profile.join("build/old-1111111111111111").exists());
        assert!(
            profile.join(".fingerprint/new-2222222222222222").exists()
                && profile.join("deps/libnew-2222222222222222.rlib").exists(),
            "it took more than the ceiling asked for"
        );
        assert!(build.join(".rustc_info.json").exists());
        assert_eq!((took.units, took.dirs, took.busy), (2, 0, 0), "{took:?}");
        assert!(took.after < everything);
    }

    #[test]
    fn a_unit_a_later_build_replaced_goes_before_an_older_one_in_use() {
        // THE ORDER UNDER `noatime`: a file's age is when it was WRITTEN, so a
        // dependency compiled a month ago and linked into every build since
        // looks a month old — and taking it rebuilds it and everything that
        // depends on it. The serde a lockfile moved past three days ago, with
        // this morning's serde of the same kind beside it, is the one nothing
        // reads again.
        let tree = Tree::new("replaced");
        let build = plant_build(&tree, "b/aa/bbbbbbbbbbbbbb", days(30), days(0));
        let profile = build.join("debug");
        for (hash, age) in [("4444444444444444", days(3)), ("5555555555555555", days(0))] {
            tree.plant(
                &format!("b/aa/bbbbbbbbbbbbbb/debug/.fingerprint/serde-{hash}/lib-serde"),
                age,
            );
            tree.plant(
                &format!("b/aa/bbbbbbbbbbbbbb/debug/.fingerprint/serde-{hash}/dep-lib-serde"),
                age,
            );
            tree.plant(
                &format!("b/aa/bbbbbbbbbbbbbb/debug/deps/libserde-{hash}.rlib"),
                age,
            );
        }
        // AND A BUILD SCRIPT'S RUN of the same crate, written today: another
        // KIND, so it replaces nothing.
        tree.plant(
            "b/aa/bbbbbbbbbbbbbb/debug/.fingerprint/serde-6666666666666666/run-build-script-build-script-build",
            days(0),
        );
        let roots = Roots::none();
        let disk = std::fs::symlink_metadata(&build).unwrap().dev();
        let everything = weight_of(std::slice::from_ref(&build), disk);
        let incremental = frees_of(&profile.join("incremental/new-3abc"), disk);
        let took = sweep(&cargo(
            &tree,
            &roots,
            everything - incremental - 1,
            Act::Execute,
        ));
        assert!(
            !profile.join(".fingerprint/serde-4444444444444444").exists()
                && !profile.join("deps/libserde-4444444444444444.rlib").exists(),
            "the replaced serde stayed: {took:?}"
        );
        assert!(
            profile.join(".fingerprint/old-1111111111111111").exists(),
            "an older unit nothing replaced went before the replaced one"
        );
        assert!(profile.join(".fingerprint/serde-5555555555555555").exists());
        assert!(profile.join(".fingerprint/serde-6666666666666666").exists());
        // AND IT WAS ENOUGH: the replaced one freed more than the ceiling
        // asked, so nothing else went — not even the incremental state.
        assert!(profile.join("incremental/new-3abc").exists());
        assert_eq!(took.units, 1, "{took:?}");
    }

    #[test]
    fn a_target_directory_is_locked_before_it_goes_whole() {
        // THE SPLIT LAYOUT: cargo tags `target/<triple>` and keeps
        // `.cargo-lock` and `.cargo-artifact-lock` in its profile, with no
        // fingerprint in sight. Not a layout this takes apart — but a build
        // may still be writing there, and its locks say so.
        let tree = Tree::new("target-dir");
        let triple = "cargo-target/rustc-1.97.1/target/wasm32-wasip2";
        tag(&tree, triple, days(30));
        tree.plant(&format!("{triple}/release/.cargo-lock"), days(30));
        tree.plant(&format!("{triple}/release/.cargo-artifact-lock"), days(30));
        tree.plant(&format!("{triple}/release/app.wasm"), days(30));
        let building =
            File::open(tree.at(&format!("{triple}/release/.cargo-artifact-lock"))).unwrap();
        building
            .try_lock()
            .expect("the test holds the artifact lock");
        let roots = Roots::none();

        let took = sweep(&cargo(&tree, &roots, 0, Act::Execute));
        assert!(
            tree.at(triple).exists(),
            "a target directory went from under the build writing it"
        );
        assert_eq!((took.busy, took.dirs), (1, 0), "{took:?}");

        drop(building);
        let took = sweep(&cargo(&tree, &roots, 0, Act::Execute));
        assert!(
            !tree.at(triple).exists(),
            "a month-old target stayed: {took:?}"
        );
        assert_eq!((took.busy, took.dirs), (0, 1), "{took:?}");
    }

    #[test]
    fn a_lock_that_cannot_be_tried_keeps_its_compiler() {
        // NOTHING SAYS NOBODY IS BUILDING when the lock cannot even be opened:
        // that is kept like a held one, never taken for a free one.
        let tree = Tree::new("lock-refused");
        let old = plant_build(
            &tree,
            "cargo-target/rustc-1.90.0/build/aa/bbbbbbbbbbbbbb",
            days(10),
            days(10),
        );
        tree.plant("cargo-target/rustc-1.97.1/target/debug/app", days(0));
        let lock = old.join("debug").join(BUILD_LOCK);
        let mode = |bits| {
            let mut m = std::fs::symlink_metadata(&lock).unwrap().permissions();
            std::os::unix::fs::PermissionsExt::set_mode(&mut m, bits);
            std::fs::set_permissions(&lock, m).unwrap();
        };
        mode(0o000);
        let roots = Roots::none();
        let compilers = tree.at("cargo-target");
        let took = sweep(&Cargo {
            compilers: Some(Compilers {
                root: &compilers,
                prefix: "rustc-",
                current: None,
            }),
            ceiling_bytes: u64::MAX,
            ..cargo(&tree, &roots, 0, Act::Execute)
        });
        mode(0o644);
        if File::open(&lock).is_ok() && took.refused == 0 {
            // RUN AS ROOT, the mode stops nothing, and this proves nothing.
            return;
        }
        assert!(
            old.exists(),
            "a compiler whose lock could not be tried went"
        );
        assert_eq!((took.dirs, took.refused), (0, 1), "{took:?}");
    }

    #[test]
    fn a_compilers_root_given_by_mistake_loses_only_compilers() {
        // A HAND RUN pointed at the wrong directory: everything in it that is
        // not named like a compiler's is not this rule's, however old.
        let tree = Tree::new("wrong-root");
        tree.plant("projects/novel/chapter-1.txt", days(90));
        tree.plant("projects/thesis/draft.tex", days(60));
        tree.plant("projects/rustc-1.80.0/target/debug/app", days(60));
        tree.plant("projects/rustc-1.97.1/target/debug/app", days(0));
        let roots = Roots::none();
        let root = tree.at("projects");
        let took = sweep(&Cargo {
            compilers: Some(Compilers {
                root: &root,
                prefix: "rustc-",
                current: None,
            }),
            ceiling_bytes: u64::MAX,
            ..cargo(&tree, &roots, 0, Act::Execute)
        });
        assert!(tree.at("projects/novel/chapter-1.txt").exists());
        assert!(tree.at("projects/thesis/draft.tex").exists());
        assert!(!tree.at("projects/rustc-1.80.0").exists());
        assert_eq!(took.dirs, 1, "{took:?}");
    }

    #[test]
    fn a_unit_that_stops_half_way_never_keeps_a_fingerprint_without_its_outputs() {
        // THE ORDER IS THE SAFETY: cargo trusts a fingerprint, so one left
        // behind by a removal that failed half way would vouch for an `.rlib`
        // that is gone, and the next build would link against nothing. The
        // fingerprint goes FIRST; whatever fails after it is only rebuilt.
        let tree = Tree::new("order");
        let build = plant_build(&tree, "b/aa/bbbbbbbbbbbbbb", days(300), days(300));
        let stuck = build.join("debug/build/old-1111111111111111/out");
        let mode = |bits| {
            let mut m = std::fs::symlink_metadata(&stuck).unwrap().permissions();
            std::os::unix::fs::PermissionsExt::set_mode(&mut m, bits);
            std::fs::set_permissions(&stuck, m).unwrap();
        };
        // A DIRECTORY THIS CANNOT EMPTY, the last of the unit's paths.
        mode(0o555);
        let roots = Roots::none();

        let took = sweep(&cargo(&tree, &roots, 0, Act::Execute));
        mode(0o755);

        let profile = build.join("debug");
        assert!(
            !profile.join(".fingerprint/old-1111111111111111").exists(),
            "a fingerprint outlived the outputs it vouches for: {took:?}"
        );
        assert!(!profile.join("deps/libold-1111111111111111.rlib").exists());
        assert!(
            took.refused >= 1,
            "the removal that failed was not said: {took:?}"
        );
    }

    #[test]
    fn under_the_ceiling_nothing_goes() {
        let tree = Tree::new("under");
        let build = plant_build(&tree, "b/aa/bbbbbbbbbbbbbb", days(300), days(0));
        let roots = Roots::none();
        let took = sweep(&cargo(&tree, &roots, u64::MAX, Act::Execute));
        assert!(
            build
                .join("debug/.fingerprint/old-1111111111111111")
                .exists()
        );
        assert_eq!((took.units, took.dirs), (0, 0));
        assert_eq!(took.before, took.after);
    }

    #[test]
    fn a_profile_cargo_is_building_in_is_left_whole() {
        // THE LOCK IS CARGO'S OWN: holding it here is what a running build
        // does, and a unit taken now would be taken out from under it.
        let tree = Tree::new("busy");
        let build = plant_build(&tree, "b/aa/bbbbbbbbbbbbbb", days(300), days(200));
        let building = File::open(build.join("debug").join(BUILD_LOCK)).unwrap();
        building.try_lock().expect("the test holds the build lock");
        let roots = Roots::none();

        let took = sweep(&cargo(&tree, &roots, 0, Act::Execute));

        assert!(
            build
                .join("debug/.fingerprint/old-1111111111111111")
                .exists()
        );
        assert!(
            build
                .join("debug/deps/libold-1111111111111111.rlib")
                .exists()
        );
        assert_eq!((took.busy, took.units, took.dirs), (1, 0, 0), "{took:?}");
        drop(building);

        // AND ONCE THE BUILD IS OVER, the same tree gives way.
        let took = sweep(&cargo(&tree, &roots, 0, Act::Execute));
        assert_eq!(took.busy, 0);
        assert!(
            !build
                .join("debug/.fingerprint/old-1111111111111111")
                .exists()
        );
    }

    #[test]
    fn a_unit_holding_a_gc_root_survives() {
        let tree = Tree::new("cargo-root-link");
        let build = plant_build(&tree, "b/aa/bbbbbbbbbbbbbb", days(300), days(0));
        let link = build.join("debug/build/old-1111111111111111/out/result");
        std::os::unix::fs::symlink("/nix/store/aaaa-x", &link).unwrap();
        let m = std::fs::symlink_metadata(&link).unwrap();
        let roots = Roots::Known([(m.dev(), m.ino())].into_iter().collect());

        let took = sweep(&cargo(&tree, &roots, 0, Act::Execute));

        assert!(
            link.symlink_metadata().is_ok(),
            "a GC root went with its unit"
        );
        assert!(
            build
                .join("debug/.fingerprint/old-1111111111111111")
                .exists()
        );
        assert_eq!(took.rooted, 1, "{took:?}");
        // THE REST STILL GOES: a root keeps its own unit, not the tree.
        assert!(
            !build
                .join("debug/.fingerprint/new-2222222222222222")
                .exists()
        );
    }

    #[test]
    fn a_tree_without_cargos_lock_goes_whole_and_only_when_old() {
        // AN OLDER CARGO: no `.cargo-build-lock`, so nothing here can keep a
        // build out, and a unit taken one by one could be taken from under
        // one. Whole, and only after a week of nobody touching it.
        let tree = Tree::new("no-lock");
        let old = plant_build(&tree, "old/aa/bbbbbbbbbbbbbb", days(30), days(30));
        std::fs::remove_file(old.join("debug").join(BUILD_LOCK)).unwrap();
        let young = plant_build(&tree, "young/aa/bbbbbbbbbbbbbb", days(30), days(1));
        std::fs::remove_file(young.join("debug").join(BUILD_LOCK)).unwrap();
        let roots = Roots::none();

        let took = sweep(&cargo(&tree, &roots, 0, Act::Execute));

        assert!(!old.exists(), "a month-old tree with no lock stayed");
        assert!(
            young
                .join("debug/.fingerprint/old-1111111111111111")
                .exists()
        );
        assert_eq!((took.dirs, took.units), (1, 0), "{took:?}");
    }

    #[test]
    fn nothing_outside_the_cargo_trees_is_touched() {
        let tree = Tree::new("outside-cargo");
        plant_build(&tree, "b/aa/bbbbbbbbbbbbbb", days(300), days(300));
        let beside = tree.plant("browser/cache/old.bin", days(900));
        let outside = Tree::new("outside-cargo-target");
        let precious = outside.plant("keep.txt", days(900));
        // A LINK OUT OF A UNIT: never followed, and what it names never read.
        std::os::unix::fs::symlink(
            &outside.0,
            tree.at("b/aa/bbbbbbbbbbbbbb/debug/deps/old-1111111111111111.escape"),
        )
        .unwrap();
        let roots = Roots::none();

        let took = sweep(&cargo(&tree, &roots, 0, Act::Execute));

        assert!(beside.exists(), "a cache that is not cargo's was touched");
        assert!(precious.exists(), "a link out of a build was followed");
        assert!(tree.at("b/aa/bbbbbbbbbbbbbb/CACHEDIR.TAG").exists());
        assert!(took.units >= 2, "{took:?}");
    }

    #[test]
    fn a_rehearsal_counts_what_it_would_take_and_takes_nothing() {
        let tree = Tree::new("cargo-rehearsal");
        let build = plant_build(&tree, "b/aa/bbbbbbbbbbbbbb", days(300), days(0));
        tree.plant("cargo-target/rustc-1.80.0/target/debug/app", days(60));
        tree.plant("cargo-target/rustc-1.97.1/target/debug/app", days(0));
        let roots = Roots::none();
        let compilers = tree.at("cargo-target");
        let c = |act| Cargo {
            compilers: Some(Compilers {
                root: &compilers,
                prefix: "rustc-",
                current: None,
            }),
            ..cargo(&tree, &roots, 1, act)
        };
        let said = sweep(&c(Act::Rehearse));
        assert!(
            build
                .join("debug/.fingerprint/old-1111111111111111")
                .exists()
        );
        assert!(tree.at("cargo-target/rustc-1.80.0").exists());
        let did = sweep(&c(Act::Execute));
        assert_eq!(
            (said.units, said.dirs, said.bytes, said.after),
            (did.units, did.dirs, did.bytes, did.after),
            "the rehearsal and the real run disagree"
        );
        assert!(!tree.at("cargo-target/rustc-1.80.0").exists());
    }

    #[test]
    fn a_hard_link_is_weighed_once() {
        // AN UPLIFTED BINARY is a hard link to its `deps/` twin: the disk holds
        // it once, and so does the total.
        let tree = Tree::new("hard-link");
        let a = tree.plant("t/deps/app-1111111111111111", days(1));
        std::fs::hard_link(&a, tree.at("t/app")).unwrap();
        let disk = std::fs::symlink_metadata(&tree.0).unwrap().dev();
        let one = std::fs::symlink_metadata(&a).unwrap().len();
        assert_eq!(weight_of(&[tree.at("t")], disk), one);
        // AND TAKING ONE OF THE TWO FREES NOTHING.
        assert_eq!(frees_of(&a, disk), 0);
        assert_eq!(frees_of(&tree.at("t"), disk), one);
    }

    #[test]
    fn a_hash_is_read_the_way_cargo_writes_it() {
        assert_eq!(
            hash_of("libserde-0123456789abcdef.rlib"),
            Some("0123456789abcdef")
        );
        assert_eq!(
            hash_of("serde-0123456789abcdef.d"),
            Some("0123456789abcdef")
        );
        assert_eq!(
            hash_of("ronoctua-shell-0123456789abcdef"),
            Some("0123456789abcdef")
        );
        assert_eq!(hash_of("build-script-build"), None);
        assert_eq!(hash_of("libc.rlib"), None);
        assert_eq!(hash_of("x-zz"), None);
    }
}
