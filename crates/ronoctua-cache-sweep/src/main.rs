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
//! not by serde ([`ronoctua_cache_sweep::write_sheet`]), because that would be
//! a serialisation stack in a binary whose whole argument is that it links
//! almost nothing.
//!
//! # The roots
//!
//! `--nix-store <path>` is the `nix-store` it asks which links are GC roots
//! ([`ronoctua_cache_sweep::Roots`]). Without it, every directory holding a
//! link is kept — the answer that keeps more, never less.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use ronoctua_cache_sweep::{Act, Roots, in_units, sweep, write_sheet};

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
    let mut nix_store: Option<PathBuf> = None;
    let mut days = u64::from(policy::KEEP_DAYS);

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            // THE SAME WORD THE INSTALLER USES, and for the same reason: the
            // harmless thing is what happens when nobody says anything.
            "--execute" => act = Act::Execute,
            "--root" => root = args.next().map(PathBuf::from),
            "--report" => report = args.next().map(PathBuf::from),
            "--nix-store" => nix_store = args.next().map(PathBuf::from),
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
        write_sheet(at, began, None, &[], "");
    }
    let roots = nix_store.as_deref().map_or_else(
        || Roots::Unknown("no --nix-store was given".to_owned()),
        Roots::asking,
    );
    let took = sweep(
        &root,
        began,
        Duration::from_secs(days * SECONDS_IN_A_DAY),
        act,
        &roots,
    );
    let ended = SystemTime::now();

    // TO STDOUT, WHICH IS THE JOURNAL. The brain's row says that it ran and
    // when; what it TOOK is here, where `journalctl --user -u
    // ronoctua-cache-sweep` finds it.
    println!(
        "{} {} directories and {} files ({}), kept {} ({} holding a Nix root{}), {} unanswerable, in {:.1}s — {}",
        if act == Act::Execute {
            "took"
        } else {
            "would take"
        },
        took.dirs,
        took.files,
        in_units(took.bytes),
        took.kept,
        took.rooted,
        roots.why_unknown().map_or_else(String::new, |why| format!(
            "; Nix did not say which links are roots ({why}), so every directory holding one was kept"
        )),
        took.refused,
        ended
            .duration_since(began)
            .unwrap_or_default()
            .as_secs_f64(),
        root.display()
    );
    if let Some(at) = &report {
        write_sheet(at, began, Some(ended), &[], "");
    }
    std::process::ExitCode::SUCCESS
}

const HELP: &str = "\
ronoctua-cache-sweep — delete cached files nothing has used for a while

  --root <dir>      where to walk (default: $XDG_CACHE_HOME)
  --days <n>        how long unused is long enough (default: from assets/cache.nix)
  --execute         actually delete; without it, count and take nothing
  --report <file>   write the brain's JSON sheet here
  --nix-store <p>   the nix-store to ask which links are GC roots (without it,
                    every directory holding a link is kept)
  --help

It never follows a symbolic link, never leaves the filesystem it started on,
takes regular files only, keeps anything stamped in the future, and keeps any
directory holding a link Nix counts as a GC root.";
