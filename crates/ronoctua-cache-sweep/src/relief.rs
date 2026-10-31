//! `ronoctua-disk-relief` — makes room when the disk is short, and says what
//! it made.
//!
//! # Why this exists
//!
//! The owner's words (2026-09-28): the distro handles the disk "sem precisar de
//! intervenções minhas", on every machine it is installed on. Nix's own floor
//! (`min-free`, `nix/modules/store.nix`) makes room only while something BUILDS,
//! and only in the store; a disk filled by anything else — cargo, above all,
//! which can put hundreds of gigabytes under one directory of a cache — stays
//! full until somebody notices. The brain notices (its module for the disk:
//! the disk under its floor, read on the heartbeat it already has) and starts
//! this.
//!
//! # Two steps, and the order is the argument
//!
//! 1. **`cargo-builds`** — [`ronoctua_cache_sweep::cargo::sweep`]: whole
//!    directories of a compiler nobody uses, then, over the ceiling, what
//!    nothing reads again and the least recently built units, under cargo's
//!    own lock. First because it is the cheapest to make again and the most
//!    likely to be the cause — and only when the CACHE's filesystem is the
//!    short one: where `/nix` and the home are two, a full store is not made
//!    roomier by rebuilding somebody's crates.
//! 2. **`store-garbage`** — `nix-store --gc --max-freed <goal − free>`, through
//!    the daemon, as the person. Only garbage: a path anything roots stays,
//!    and no generation of anybody's is touched. It stops once the store's
//!    filesystem is back at the goal rather than emptying the store, so the
//!    next build does not download what was just thrown away.
//!
//!    THE DISK IS MEASURED AFTER EACH COLLECTION, and Nix is asked again for
//!    what is still missing. `--max-freed` is Nix's own count, and Nix counts
//!    as freed what the disk does not get back — a dead path on a read-only
//!    layer was 263.8 MiB "freed" and nothing in `df` — so one collection
//!    trusted that arithmetic and stopped short. It asks until the disk is at
//!    the goal, or a collection gave the disk nothing, and never more than
//!    [`MOST_COLLECTIONS`] times.
//!
//! The goal is a share of each filesystem and never more than a size
//! (`diskGoalCapGiB`): ten per cent of eight terabytes is not a disk that is
//! short.
//!
//! # And what it freed is there before it says so
//!
//! btrfs gives freed space back when its transaction commits, not when the
//! file goes. The brain measures the disk the moment this ends, so it asks
//! both filesystems to commit (`syncfs`) before its sheet says it is done —
//! or "freed" would come up short and "still short" would be said of a disk
//! that is not.
//!
//! # What it is told, and what it decides
//!
//! Every number arrives as an argument, from `assets/cache.nix` through the
//! unit the brain's module for the disk declares — the same table the brain's
//! floor comes from. What
//! it decides is only what its steps find.
//!
//! # It rehearses unless told otherwise
//!
//! Without `--execute` it counts what the cargo step would take and asks the
//! store what is dead (`--print-dead`), and deletes nothing — the shape every
//! program here that deletes has.

use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant, SystemTime};

use ronoctua_cache_sweep::cargo::{self, Cargo, Compilers};
use ronoctua_cache_sweep::{Act, Roots, StepSheet, in_units, last_line, write_sheet};

/// The policy, generated from `assets/cache.nix` by `nix/cache.nix` — for the
/// defaults of a run by hand. The unit passes every number anyway, so
/// `systemctl --user cat` shows them.
#[allow(dead_code)]
mod policy {
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

const SECONDS_IN_A_DAY: u64 = 24 * 60 * 60;

/// A gibibyte, for the goal's cap.
const GIB: u64 = 1024 * 1024 * 1024;

/// The most collections one relief asks for. Every one after the first is
/// asked only because the disk got back less than Nix counted; the bound is
/// for a disk that creeps up by a few bytes each time from somebody else's
/// work, which would otherwise be asked about for ever.
const MOST_COLLECTIONS: usize = 5;

/// What the command line said.
#[derive(Debug)]
struct Told {
    act: Act,
    root: Option<PathBuf>,
    cargo_root: Option<PathBuf>,
    compiler_prefix: Option<String>,
    current: Option<String>,
    nix_store: Option<PathBuf>,
    store: Option<PathBuf>,
    report: Option<PathBuf>,
    goal_percent: u8,
    goal_cap_gib: u32,
    ceiling_percent: u8,
    old_compiler_days: u16,
}

fn main() -> ExitCode {
    let told = match read_arguments(std::env::args().skip(1)) {
        Ok(Some(told)) => told,
        Ok(None) => {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Err(why) => {
            eprintln!("ronoctua-disk-relief: {why}. Try --help.");
            return ExitCode::FAILURE;
        }
    };
    // THE ROOT IS A PARAMETER FIRST, for `ronoctua-cache-sweep`'s reason.
    let Some(root) = told.root.clone().or_else(ronoctua_xdg::cache_root) else {
        eprintln!("ronoctua-disk-relief: no XDG_CACHE_HOME and no HOME, so no cache root");
        return ExitCode::FAILURE;
    };
    // THE COMPILERS' DIRECTORY IS THE CACHE'S OR NOTHING. A hand run with
    // `--cargo-root ~/projects` would otherwise offer every project there to
    // the compiler rule.
    if let Err(why) = cargo_root_within(&root, told.cargo_root.as_deref()) {
        eprintln!("ronoctua-disk-relief: {why}");
        return ExitCode::FAILURE;
    }

    let began = SystemTime::now();
    let sheet = |steps: &[StepSheet], ended: Option<SystemTime>, trouble: &str| {
        if let Some(at) = &told.report {
            write_sheet(at, began, ended, steps, trouble);
        }
    };
    sheet(&[], None, "");

    let roots = told.nix_store.as_deref().map_or_else(
        || Roots::Unknown("no --nix-store was given".to_owned()),
        Roots::asking,
    );
    let mut steps = vec![cargo_builds(&told, &root, &roots, began)];
    sheet(&steps, None, "");
    steps.push(store_garbage(&told));
    // WHAT WAS FREED IS FREE BEFORE THE SHEET SAYS "ENDED": see the header.
    if told.act == Act::Execute {
        for at in std::iter::once(root.as_path()).chain(told.store.as_deref()) {
            commit(at);
        }
    }

    let trouble: Vec<String> = steps
        .iter()
        .filter(|s| !s.ok)
        .map(|s| format!("{}: {}", s.id, s.trouble))
        .collect();
    let trouble = trouble.join("; ");
    sheet(&steps, Some(SystemTime::now()), &trouble);
    if trouble.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Step one: the cargo trees under the cache.
fn cargo_builds(told: &Told, root: &Path, roots: &Roots, now: SystemTime) -> StepSheet {
    let clock = Instant::now();
    let mut step = StepSheet {
        id: "cargo-builds",
        ok: true,
        ..StepSheet::default()
    };
    // THE CEILING IS A SHARE OF THE CACHE'S OWN FILESYSTEM, so the same number
    // means the same thing on a 256 GB laptop and a 2 TB desktop.
    let Some((total, free)) = room(root) else {
        step.ok = false;
        step.trouble = format!("could not measure the filesystem of {}", root.display());
        println!("cargo-builds: {}", step.trouble);
        return step;
    };
    // ONLY WHERE THE CACHE ITSELF IS SHORT. With `/nix` on a filesystem of its
    // own, a full store is not made roomier by taking somebody's builds.
    if missing(total, free, told.goal_percent, told.goal_cap_gib).is_none() {
        println!(
            "cargo-builds: the cache's filesystem has {} free of {}, at or above its goal of {}; no build taken",
            in_units(free),
            in_units(total),
            in_units(goal_of(total, told.goal_percent, told.goal_cap_gib))
        );
        step.seconds = seconds(clock);
        return step;
    }
    let ceiling_bytes = total / 100 * u64::from(told.ceiling_percent);
    let compilers = match (told.cargo_root.as_deref(), told.compiler_prefix.as_deref()) {
        (Some(root), Some(prefix)) => Some(Compilers {
            root,
            prefix,
            current: told.current.as_deref(),
        }),
        _ => None,
    };
    let swept = cargo::sweep(&Cargo {
        cache: root,
        compilers,
        ceiling_bytes,
        old_compiler: Duration::from_secs(u64::from(told.old_compiler_days) * SECONDS_IN_A_DAY),
        now,
        act: told.act,
        roots,
    });
    // TO STDOUT, WHICH IS THE JOURNAL — for the reason the sweep gives.
    println!(
        "cargo-builds: {} {} units and {} whole directories ({}); cargo trees {} -> {} against a ceiling of {}; {} busy, {} holding a Nix root, {} unanswerable{}",
        if told.act == Act::Execute {
            "took"
        } else {
            "would take"
        },
        swept.units,
        swept.dirs,
        in_units(swept.bytes),
        in_units(swept.before),
        in_units(swept.after),
        in_units(ceiling_bytes),
        swept.busy,
        swept.rooted,
        swept.refused,
        roots.why_unknown().map_or_else(String::new, |why| format!(
            "; Nix did not say which links are roots ({why}), so every tree holding a link was kept"
        )),
    );
    step.seconds = seconds(clock);
    step
}

/// Step two: the store's garbage, down to the goal and no further.
fn store_garbage(told: &Told) -> StepSheet {
    let clock = Instant::now();
    let mut step = StepSheet {
        id: "store-garbage",
        ok: true,
        ..StepSheet::default()
    };
    let (Some(store), Some(nix_store)) = (told.store.as_deref(), told.nix_store.as_deref()) else {
        println!("store-garbage: no --store or no --nix-store, so the store was left alone");
        return step;
    };
    let Some((total, free)) = room(store) else {
        step.ok = false;
        step.trouble = format!("could not measure the filesystem of {}", store.display());
        println!("store-garbage: {}", step.trouble);
        return step;
    };
    let goal = goal_of(total, told.goal_percent, told.goal_cap_gib);
    let Some(wanted) = missing(total, free, told.goal_percent, told.goal_cap_gib) else {
        println!(
            "store-garbage: {} free of {}, at or above the goal of {}; nothing collected",
            in_units(free),
            in_units(total),
            in_units(goal)
        );
        step.seconds = seconds(clock);
        return step;
    };

    if told.act == Act::Rehearse {
        // WHAT IS DEAD, and nothing deleted: the store's own answer.
        match Command::new(nix_store)
            .args(collect(Act::Rehearse, wanted))
            .stdin(Stdio::null())
            .output()
        {
            Ok(out) if out.status.success() => {
                let dead = out
                    .stdout
                    .split(|b| *b == b'\n')
                    .filter(|l| !l.is_empty())
                    .count();
                println!(
                    "store-garbage: would collect up to {} of {dead} dead paths, to bring {} free up to the goal of {}",
                    in_units(wanted),
                    in_units(free),
                    in_units(goal)
                );
            }
            Ok(out) => {
                step.ok = false;
                step.trouble = last_line(&out.stderr);
                println!("store-garbage: nix-store refused: {}", step.trouble);
            }
            Err(e) => {
                step.ok = false;
                step.trouble = e.to_string();
                println!("store-garbage: could not run {}: {e}", nix_store.display());
            }
        }
        step.seconds = seconds(clock);
        return step;
    }

    // ONLY GARBAGE, AND ONLY AS MUCH AS IS MISSING — as the DISK says, after
    // each collection: see the header. What the person's Nix relies on is a
    // root, and a root is never garbage.
    let collected = collect_to_the_goal(nix_store, goal, free, || {
        // COMMITTED FIRST, or btrfs counts what was freed as still taken.
        commit(store);
        room(store).map(|(_, f)| f)
    });
    if let Some(trouble) = collected.trouble {
        step.ok = false;
        step.trouble = trouble;
    } else if collected.free < goal {
        println!(
            "store-garbage: {} free after {} collections, short of the goal of {}: {}",
            in_units(collected.free),
            collected.asked.len(),
            in_units(goal),
            if collected.asked.len() < MOST_COLLECTIONS {
                "the last one gave the disk nothing more"
            } else {
                "no more are asked for in one relief"
            }
        );
    }
    step.seconds = seconds(clock);
    step
}

/// How the collections of [`collect_to_the_goal`] ended.
#[derive(Debug, PartialEq, Eq)]
struct Collected {
    /// What each collection was asked to free, in order.
    asked: Vec<u64>,
    /// The bytes free after the last one, as the disk said.
    free: u64,
    /// What went wrong, when something did: `nix-store` refused, would not
    /// run, or the disk could not be measured.
    trouble: Option<String>,
}

/// Asks the store for garbage until the disk is at `goal` bytes free — the
/// disk as `measure` reads it after each collection, NOT Nix's own count of
/// what it freed — or a collection gave the disk nothing more, or
/// [`MOST_COLLECTIONS`] were asked.
fn collect_to_the_goal(
    nix_store: &Path,
    goal: u64,
    free: u64,
    mut measure: impl FnMut() -> Option<u64>,
) -> Collected {
    let mut collected = Collected {
        asked: Vec::new(),
        free,
        trouble: None,
    };
    while collected.asked.len() < MOST_COLLECTIONS {
        let Some(wanted) = goal.checked_sub(collected.free).filter(|w| *w > 0) else {
            break;
        };
        collected.asked.push(wanted);
        let said = match Command::new(nix_store)
            .args(collect(Act::Execute, wanted))
            .stdin(Stdio::null())
            .output()
        {
            Ok(out) if out.status.success() => last_line(&out.stdout),
            Ok(out) => {
                let trouble = last_line(&out.stderr);
                println!("store-garbage: nix-store refused: {trouble}");
                collected.trouble = Some(trouble);
                break;
            }
            Err(e) => {
                println!("store-garbage: could not run {}: {e}", nix_store.display());
                collected.trouble = Some(e.to_string());
                break;
            }
        };
        let Some(after) = measure() else {
            let trouble = "could not measure the store's filesystem after a collection".to_owned();
            println!("store-garbage: {trouble}");
            collected.trouble = Some(trouble);
            break;
        };
        println!(
            "store-garbage: collection {} asked for {}; {} free before, {} after (goal {}) — {said}",
            collected.asked.len(),
            in_units(wanted),
            in_units(collected.free),
            in_units(after),
            in_units(goal)
        );
        // NOTHING MORE ON THE DISK is the end, whatever Nix said it freed:
        // asking again would only be told the same.
        let gained = after > collected.free;
        collected.free = after;
        if !gained {
            break;
        }
    }
    collected
}

/// The goal on a filesystem of `total` bytes: `goal_percent` of it, and never
/// more than `cap_gib` — the brain's floor has the same shape (its module for
/// the disk), and so has Nix's (`nix/modules/store.nix`).
fn goal_of(total: u64, goal_percent: u8, cap_gib: u32) -> u64 {
    (total / 100 * u64::from(goal_percent)).min(u64::from(cap_gib).saturating_mul(GIB))
}

/// How much has to be freed to bring `free` up to the goal, or `None` when it
/// is there already.
fn missing(total: u64, free: u64, goal_percent: u8, cap_gib: u32) -> Option<u64> {
    goal_of(total, goal_percent, cap_gib)
        .checked_sub(free)
        .filter(|w| *w > 0)
}

/// Commits the filesystem holding `path`, so what was freed is counted free.
/// A failure changes nothing that was done, so it is said and nothing more.
fn commit(path: &Path) {
    match std::fs::File::open(path) {
        Ok(file) => {
            if let Err(e) = rustix::fs::syncfs(&file) {
                println!("could not commit the filesystem of {}: {e}", path.display());
            }
        }
        Err(e) => println!("could not open {} to commit it: {e}", path.display()),
    }
}

/// Refuses a compilers' directory that is not the cache's: relative, climbing
/// out with `..`, outside `root`, or — when it exists — a link or on another
/// filesystem.
fn cargo_root_within(root: &Path, cargo_root: Option<&Path>) -> Result<(), String> {
    let Some(k) = cargo_root else {
        return Ok(());
    };
    let plain = k.components().all(|c| {
        matches!(
            c,
            std::path::Component::RootDir | std::path::Component::Normal(_)
        )
    });
    if !k.is_absolute() || !plain || !root.is_absolute() {
        return Err(format!(
            "--cargo-root {} and --root {} must be absolute, with no `.` or `..` in them",
            k.display(),
            root.display()
        ));
    }
    if k == root || !k.starts_with(root) {
        return Err(format!(
            "--cargo-root {} is not a directory inside the cache {}",
            k.display(),
            root.display()
        ));
    }
    if let Ok(meta) = std::fs::symlink_metadata(k) {
        let same = std::fs::symlink_metadata(root).is_ok_and(|r| r.dev() == meta.dev());
        if !meta.is_dir() || !same {
            return Err(format!(
                "--cargo-root {} is not a directory on the cache's own filesystem",
                k.display()
            ));
        }
    }
    Ok(())
}

/// What `nix-store` is asked: to collect `wanted` bytes of GARBAGE, or — in a
/// rehearsal — only to say what is dead.
///
/// ONE PLACE FOR THE WORDS, because they are the promise: `--gc` never removes
/// a path anything roots, and nothing here names a generation.
fn collect(act: Act, wanted: u64) -> Vec<String> {
    match act {
        Act::Execute => vec![
            "--gc".to_owned(),
            "--max-freed".to_owned(),
            wanted.to_string(),
        ],
        Act::Rehearse => vec!["--gc".to_owned(), "--print-dead".to_owned()],
    }
}

/// Total and free bytes of the filesystem holding `path` — free for this
/// user, after the reserve the filesystem keeps for root.
fn room(path: &Path) -> Option<(u64, u64)> {
    let s = rustix::fs::statvfs(path).ok()?;
    Some((
        s.f_blocks.saturating_mul(s.f_frsize),
        s.f_bavail.saturating_mul(s.f_frsize),
    ))
}

/// Whole seconds since `clock`, for the sheet.
fn seconds(clock: Instant) -> u32 {
    u32::try_from(clock.elapsed().as_secs()).unwrap_or(u32::MAX)
}

/// What the command line asks for; `None` for `--help`.
fn read_arguments(args: impl Iterator<Item = String>) -> Result<Option<Told>, String> {
    let mut told = Told {
        act: Act::Rehearse,
        root: None,
        cargo_root: None,
        compiler_prefix: None,
        current: None,
        nix_store: None,
        store: None,
        report: None,
        goal_percent: policy::DISK_GOAL_PERCENT,
        goal_cap_gib: policy::DISK_GOAL_CAP_GIB,
        ceiling_percent: policy::CARGO_CEILING_PERCENT,
        old_compiler_days: policy::OLD_COMPILER_DAYS,
    };
    let mut dry = false;
    let mut args = args;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} wants a value"));
        match arg.as_str() {
            // THE SAME WORD THE INSTALLER AND THE SWEEP USE: the harmless thing
            // is what happens when nobody says anything.
            "--execute" => told.act = Act::Execute,
            // AND THE HARMLESS ONE SAID OUT LOUD, for a person who wants the
            // command line to say it: the same rehearsal, refused beside
            // `--execute` rather than one of the two quietly winning.
            "--dry-run" => dry = true,
            "--root" => told.root = Some(PathBuf::from(value()?)),
            "--cargo-root" => told.cargo_root = Some(PathBuf::from(value()?)),
            "--compiler-prefix" => told.compiler_prefix = Some(value()?),
            "--keep-compiler" => told.current = Some(value()?),
            "--nix-store" => told.nix_store = Some(PathBuf::from(value()?)),
            "--store" => told.store = Some(PathBuf::from(value()?)),
            "--report" => told.report = Some(PathBuf::from(value()?)),
            "--goal-percent" => told.goal_percent = percent(&arg, &value()?)?,
            "--goal-cap-gib" => {
                let raw = value()?;
                told.goal_cap_gib =
                    raw.parse::<u32>().ok().filter(|g| *g > 0).ok_or_else(|| {
                        format!("{arg} wants a whole number of gibibytes, not `{raw}`")
                    })?;
            }
            "--cargo-ceiling-percent" => told.ceiling_percent = percent(&arg, &value()?)?,
            "--old-compiler-days" => {
                let raw = value()?;
                told.old_compiler_days =
                    raw.parse::<u16>().ok().filter(|d| *d > 0).ok_or_else(|| {
                        format!("{arg} wants a whole number of days, not `{raw}`")
                    })?;
            }
            "--help" | "-h" => return Ok(None),
            other => return Err(format!("I do not know `{other}`")),
        }
    }
    if dry && told.act == Act::Execute {
        return Err("--dry-run and --execute say opposite things".to_owned());
    }
    // A COMPILER IS KNOWN BY ITS NAME, and the name is said once: without the
    // prefix the compiler rule would not know which directories are
    // compilers', and the session's own must be one of them.
    match (&told.cargo_root, &told.compiler_prefix) {
        (Some(_), None) => return Err("--cargo-root wants --compiler-prefix".to_owned()),
        (_, Some(p)) if p.is_empty() || p.contains('/') => {
            return Err(format!(
                "--compiler-prefix `{p}` is not the start of a directory's name"
            ));
        }
        _ => {}
    }
    if let (Some(p), Some(current)) = (&told.compiler_prefix, &told.current)
        && !current.starts_with(p.as_str())
    {
        return Err(format!(
            "--keep-compiler `{current}` is not named with --compiler-prefix `{p}`"
        ));
    }
    Ok(Some(told))
}

/// A whole percentage between 1 and 99 — a floor, a goal or a ceiling of 0 or
/// 100 is a number somebody mistyped, refused rather than obeyed.
fn percent(flag: &str, raw: &str) -> Result<u8, String> {
    raw.parse::<u8>()
        .ok()
        .filter(|p| (1..100).contains(p))
        .ok_or_else(|| format!("{flag} wants a whole percentage between 1 and 99, not `{raw}`"))
}

const HELP: &str = "\
ronoctua-disk-relief — make room when the disk is short

  --root <dir>                 the cache walked for cargo builds (default: $XDG_CACHE_HOME)
  --cargo-root <dir>           the directory inside the cache holding one directory per compiler
  --compiler-prefix <word>     what every compiler's directory there is named with (rustc-)
  --keep-compiler <name>       the compiler directory the session builds with
  --nix-store <path>           the nix-store asked for roots and for garbage
  --store <dir>                the store, whose filesystem the goal is measured on
  --goal-percent <n>           collect the store's garbage until this much is free
  --goal-cap-gib <n>           and never aim for more than this many GiB free
  --cargo-ceiling-percent <n>  the most every cargo build may weigh together
  --old-compiler-days <n>      another compiler's builds, untouched this long, go whole
  --report <file>              write the brain's JSON sheet here
  --execute                    actually delete; without it, count and take nothing
  --dry-run                    say the default out loud: count, take nothing
  --help

It takes cargo builds only under cargo's own lock and only while the cache's
filesystem is short, never follows a link, keeps anything Nix counts as a root,
and collects only the store's garbage — never a generation.";

#[cfg(test)]
mod tests {
    use super::*;

    fn told(args: &[&str]) -> Result<Option<Told>, String> {
        read_arguments(args.iter().map(|a| (*a).to_owned()))
    }

    #[test]
    fn nothing_said_is_a_rehearsal_with_the_catalogues_numbers() {
        let t = told(&[]).expect("no arguments").expect("not help");
        assert_eq!(t.act, Act::Rehearse, "the harmless thing is the default");
        assert_eq!(t.goal_percent, policy::DISK_GOAL_PERCENT);
        assert_eq!(t.goal_cap_gib, policy::DISK_GOAL_CAP_GIB);
        assert_eq!(t.ceiling_percent, policy::CARGO_CEILING_PERCENT);
        assert_eq!(t.old_compiler_days, policy::OLD_COMPILER_DAYS);
    }

    #[test]
    fn a_number_out_of_range_is_refused_and_not_obeyed() {
        for bad in [
            &["--goal-percent", "0"][..],
            &["--goal-percent", "100"],
            &["--cargo-ceiling-percent", "ten"],
            &["--old-compiler-days", "0"],
            &["--old-compiler-days"],
            &["--everything"],
            &["--dry-run", "--execute"],
            &["--goal-cap-gib", "0"],
            &["--cargo-root", "/c/cargo-target"],
            &["--cargo-root", "/c/cargo-target", "--compiler-prefix", ""],
            &["--compiler-prefix", "rustc-", "--keep-compiler", "gcc-14"],
        ] {
            assert!(told(bad).is_err(), "{bad:?} was accepted");
        }
        let t = told(&[
            "--execute",
            "--goal-percent",
            "20",
            "--compiler-prefix",
            "rustc-",
            "--keep-compiler",
            "rustc-1.97.1",
        ])
        .expect("fine")
        .expect("not help");
        assert_eq!((t.act, t.goal_percent), (Act::Execute, 20));
        assert_eq!(t.current.as_deref(), Some("rustc-1.97.1"));
        let t = told(&["--dry-run"]).expect("fine").expect("not help");
        assert_eq!(t.act, Act::Rehearse);
    }

    #[test]
    fn a_compilers_directory_outside_the_cache_is_refused() {
        // A HAND RUN pointed at the wrong place: `--cargo-root ~/projects`
        // would offer every project there to the compiler rule.
        let tree = std::env::temp_dir().join(format!("ronoctua-relief-{}", std::process::id()));
        let cache = tree.join("cache");
        std::fs::create_dir_all(cache.join("cargo-target")).unwrap();
        std::fs::create_dir_all(tree.join("projects")).unwrap();
        std::os::unix::fs::symlink(tree.join("projects"), cache.join("linked")).unwrap();
        let ok = |k: &Path| cargo_root_within(&cache, Some(k));
        assert!(ok(&cache.join("cargo-target")).is_ok());
        assert!(
            ok(&cache.join("not-made-yet")).is_ok(),
            "a directory to come is fine"
        );
        assert!(cargo_root_within(&cache, None).is_ok());
        for bad in [
            tree.join("projects"),
            cache.clone(),
            cache.join("../projects"),
            PathBuf::from("cargo-target"),
            cache.join("linked"),
        ] {
            assert!(ok(&bad).is_err(), "{} was accepted", bad.display());
        }
        let _ = std::fs::remove_dir_all(&tree);
    }

    #[test]
    fn the_store_is_asked_for_garbage_down_to_the_goal_and_nothing_more() {
        // 1000 bytes, 100 free, a goal of 15%: 50 are missing.
        assert_eq!(missing(1000, 100, 15, 1), Some(50));
        // AT OR ABOVE THE GOAL, nothing is asked at all.
        assert_eq!(missing(1000, 150, 15, 1), None);
        assert_eq!(missing(1000, 900, 15, 1), None);
        // AND A BIG DISK'S GOAL IS CAPPED: 15% of 8 TiB would be 1.2 TiB, and
        // a disk with 100 GiB free is not short of a 75 GiB goal.
        let tib = 1024 * GIB;
        assert_eq!(goal_of(8 * tib, 15, 75), 75 * GIB);
        assert_eq!(missing(8 * tib, 100 * GIB, 15, 75), None);
        assert_eq!(missing(8 * tib, 70 * GIB, 15, 75), Some(5 * GIB));
        // A small one is governed by the share.
        assert_eq!(goal_of(100 * GIB, 15, 75), 100 * GIB / 100 * 15);
        assert_eq!(
            collect(Act::Execute, 50),
            ["--gc", "--max-freed", "50"],
            "the collection is not bounded by what is missing"
        );
        assert_eq!(
            collect(Act::Rehearse, 50),
            ["--gc", "--print-dead"],
            "a rehearsal collected"
        );
        // AND NEVER A WORD THAT REMOVES A GENERATION OR A LIVE PATH.
        for act in [Act::Execute, Act::Rehearse] {
            for word in collect(act, 1) {
                assert!(
                    !word.contains("delete") && !word.contains("ignore-liveness"),
                    "`{word}` asks for more than garbage"
                );
            }
        }
    }

    /// A `nix-store` of the test's own making, in `dir`: every call is a line
    /// of `calls` holding its arguments, and every collection SAYS it freed
    /// 263.80 MiB — what the disk got back is the test's to plant.
    ///
    /// Written by a `/bin/sh` child, never by this process: a file open for
    /// writing when another test's thread forks stays with that child until
    /// its `exec`, and running it then is `ETXTBSY` at random.
    fn lying_nix_store(dir: &Path) -> (PathBuf, PathBuf) {
        std::fs::create_dir_all(dir).unwrap();
        let program = dir.join("nix-store");
        let calls = dir.join("calls");
        let body = format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\necho \"deleting '/nix/store/x-dead'\"\necho '1 store paths deleted, 263.80 MiB freed'\n",
            calls.display()
        );
        let status = Command::new("/bin/sh")
            .args([
                "-c",
                "printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"",
                "sh",
            ])
            .arg(&program)
            .arg(body)
            .status()
            .unwrap();
        assert!(status.success(), "{}", program.display());
        (program, calls)
    }

    /// How many collections the lying `nix-store` was asked for, and with
    /// what.
    fn calls_of(calls: &Path) -> Vec<String> {
        std::fs::read_to_string(calls)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// FAILS IF: one collection is trusted — Nix says it freed what was
    /// asked, the disk got back less (a dead path on a read-only layer is
    /// counted and never returned), and the relief stops short of the goal
    /// instead of measuring the disk and asking for what is still missing.
    #[test]
    fn a_collection_nix_overcounts_is_followed_by_another_for_what_is_missing() {
        let dir = std::env::temp_dir().join(format!("ronoctua-relief-lie-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (program, calls) = lying_nix_store(&dir);
        // 100 free, a goal of 150. The first collection is asked for 50 and
        // the disk gets 20 of them back; the second, for the 30 still
        // missing, gets all 30.
        let disk = [120, 150];
        let collected = collect_to_the_goal(&program, 150, 100, || {
            disk.get(calls_of(&calls).len() - 1).copied()
        });
        assert_eq!(collected.trouble, None);
        assert_eq!(collected.asked, [50, 30], "{collected:?}");
        assert_eq!(collected.free, 150);
        assert_eq!(
            calls_of(&calls),
            ["--gc --max-freed 50", "--gc --max-freed 30"],
            "the store was not asked for what the disk still missed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a collection that gave the disk nothing is asked again — the
    /// same dead path counted for ever — or a disk that keeps creeping up is
    /// asked about past the bound.
    #[test]
    fn the_collections_end_when_the_disk_gains_nothing_and_at_their_bound() {
        let dir = std::env::temp_dir().join(format!("ronoctua-relief-end-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (program, calls) = lying_nix_store(&dir);
        // NOTHING MORE: 20 back from the first, nothing from the second.
        let disk = [120, 120];
        let collected = collect_to_the_goal(&program, 150, 100, || {
            disk.get(calls_of(&calls).len() - 1).copied()
        });
        assert_eq!(collected.asked, [50, 30], "{collected:?}");
        assert_eq!((collected.free, collected.trouble), (120, None));
        // A DISK THAT CREEPS UP a byte a time is asked at most the bound.
        std::fs::remove_file(&calls).unwrap();
        let collected = collect_to_the_goal(&program, 150, 100, || {
            Some(100 + calls_of(&calls).len() as u64)
        });
        assert_eq!(collected.asked.len(), MOST_COLLECTIONS, "{collected:?}");
        assert_eq!(calls_of(&calls).len(), MOST_COLLECTIONS);
        // AT THE GOAL ALREADY, nothing is asked at all.
        std::fs::remove_file(&calls).unwrap();
        let collected = collect_to_the_goal(&program, 150, 150, || Some(150));
        assert!(collected.asked.is_empty() && calls_of(&calls).is_empty());
        // AND A STORE THAT REFUSES, or a disk that cannot be measured, ends
        // it with the reason.
        let collected = collect_to_the_goal(&dir.join("absent"), 150, 100, || Some(150));
        assert!(collected.trouble.is_some(), "{collected:?}");
        let collected = collect_to_the_goal(&program, 150, 100, || None);
        assert_eq!(collected.asked.len(), 1);
        assert!(collected.trouble.is_some(), "{collected:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_program_that_speaks_last_is_read_by_its_last_line() {
        assert_eq!(
            last_line(b"deleting a\ndeleting b\n42 store paths deleted\n\n"),
            "42 store paths deleted"
        );
        assert_eq!(last_line(b""), "");
    }
}
