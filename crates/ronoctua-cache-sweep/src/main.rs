//! `ronoctua-cache-sweep` — takes what nothing has used, and says what it took.
//!
//! # Why a unit and not something the brain does itself
//!
//! Three reasons, and each one was measured:
//!
//! 1. `Task::run` blocks the brain's own thread. A walk of a quarter of a
//!    million files there is a daemon that draws nothing and cannot be stopped
//!    for as long as it lasts.
//! 2. A unit gets `CPUSchedulingPolicy=idle` and `IOSchedulingClass=idle` for
//!    nothing — the argument `nix/modules/memory.nix` already makes about
//!    collecting the store: it competes with everything for the disk, so it
//!    yields to whatever the person is actually doing. Deleting whole cache
//!    directories at once is a lot of `unlink`; without this it is a machine
//!    nobody can type on while it happens.
//! 3. It stays reachable by hand and in a VM.
//!
//! # What it is told, and what it decides
//!
//! Nothing. The root, the age and whether to act all arrive as arguments; the
//! policy lives in `assets/cache.nix` and the machine's load is the brain's
//! question, answered before this is started.
//!
//! # The report
//!
//! `--report <path>` writes the JSON sheet `ronoctua_brain::task::Sheet` reads —
//! `began` at the start and `ended` when it is over, which is what makes the
//! brain's patience measure silence rather than duration. Written BY HAND and
//! not by serde, because that would be a serialisation stack in a binary whose
//! whole argument is that it links almost nothing.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ronoctua_cache_sweep::{Act, sweep};

/// The policy, generated from `assets/cache.nix` by `nix/cache.nix`.
///
/// THE WHOLE TABLE AND NOT THE ONE NUMBER, deliberately. Three crates include
/// this same file and each uses a different part of it — the brain takes the
/// cadence and the window, the file manager takes the renewal age, this takes
/// the retention. Trimming it per consumer would be three tables to keep in
/// step, which is the thing `assets/cache.nix` exists to prevent.
#[allow(dead_code)]
mod policy {
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

const SECONDS_IN_A_DAY: u64 = 24 * 60 * 60;

fn main() -> std::process::ExitCode {
    let mut act = Act::Rehearse;
    let mut root: Option<PathBuf> = None;
    let mut report: Option<PathBuf> = None;
    let mut days = u64::from(policy::KEEP_DAYS);

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            // THE SAME WORD THE INSTALLER USES, and for the same reason: the
            // harmless thing is what happens when nobody says anything.
            "--execute" => act = Act::Execute,
            "--root" => root = args.next().map(PathBuf::from),
            "--report" => report = args.next().map(PathBuf::from),
            "--days" => {
                let Some(n) = args.next().and_then(|n| n.parse::<u64>().ok()) else {
                    eprintln!("ronoctua-cache-sweep: --days wants a whole number of days");
                    return std::process::ExitCode::FAILURE;
                };
                days = n;
            }
            "--help" | "-h" => {
                println!("{HELP}");
                return std::process::ExitCode::SUCCESS;
            }
            other => {
                eprintln!("ronoctua-cache-sweep: I do not know `{other}`. Try --help.");
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    // THE ROOT IS A PARAMETER FIRST AND A DEFAULT SECOND. `ronoctua_xdg`'s own
    // rule: a function that finds `$XDG_CACHE_HOME` on its own is a function a
    // test cannot stop from emptying the cache of whoever ran it.
    let Some(root) = root.or_else(ronoctua_xdg::cache_root) else {
        eprintln!("ronoctua-cache-sweep: no XDG_CACHE_HOME and no HOME, so no cache root");
        return std::process::ExitCode::FAILURE;
    };

    let began = SystemTime::now();
    if let Some(at) = &report {
        write_sheet(at, began, None);
    }
    let took = sweep(
        &root,
        began,
        Duration::from_secs(days * SECONDS_IN_A_DAY),
        act,
    );
    let ended = SystemTime::now();

    // TO STDOUT, WHICH IS THE JOURNAL. The brain's row says that it ran and
    // when; what it TOOK is here, where `journalctl --user -u
    // ronoctua-cache-sweep` finds it.
    println!(
        "{} {} directories and {} files ({}), kept {}, {} unanswerable, in {:.1}s — {}",
        if act == Act::Execute {
            "took"
        } else {
            "would take"
        },
        took.dirs,
        took.files,
        in_units(took.bytes),
        took.kept,
        took.refused,
        ended
            .duration_since(began)
            .unwrap_or_default()
            .as_secs_f64(),
        root.display()
    );
    if let Some(at) = &report {
        write_sheet(at, began, Some(ended));
    }
    std::process::ExitCode::SUCCESS
}

const HELP: &str = "\
ronoctua-cache-sweep — delete cached files nothing has used for a while

  --root <dir>      where to walk (default: $XDG_CACHE_HOME)
  --days <n>        how long unused is long enough (default: from assets/cache.nix)
  --execute         actually delete; without it, count and take nothing
  --report <file>   write the brain's JSON sheet here
  --help

It never follows a symbolic link, never leaves the filesystem it started on,
takes regular files only, and keeps anything stamped in the future.";

/// `1,4 GiB`, in the units a person reads.
fn in_units(bytes: u64) -> String {
    const STEP: f64 = 1024.0;
    #[allow(clippy::cast_precision_loss)] // A cache is not 2^53 bytes.
    let mut left = bytes as f64;
    for unit in ["B", "KiB", "MiB", "GiB"] {
        if left < STEP {
            return format!("{left:.1} {unit}");
        }
        left /= STEP;
    }
    format!("{left:.1} TiB")
}

/// The sheet, by hand.
///
/// `ended` is ZERO while it is still going, which is the one thing the brain's
/// watcher reads to tell "still working" from "finished" — a moment in 1970 is
/// not a run that ended.
fn write_sheet(at: &Path, began: SystemTime, ended: Option<SystemTime>) {
    let stamp = |t: SystemTime| {
        t.duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
    };
    let body = format!(
        "{{\"began\":{},\"ended\":{},\"steps\":[],\"trouble\":\"\"}}",
        stamp(began),
        ended.map_or(0, stamp)
    );
    if let Some(dir) = at.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // FAILURE IS SILENCE ON PURPOSE: the report is how the brain WATCHES, and a
    // sweep that refused to run because it could not write a status file would
    // be a cache that fills up for want of a directory.
    let _ = std::fs::write(at, body);
}
