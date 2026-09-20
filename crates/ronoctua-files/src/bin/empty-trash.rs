//! `ronoctua-empty-trash` — throws away what has been in the trash long enough.
//!
//! # Why it is here and not in the cache sweep
//!
//! Because nothing in the trash can be made again. `ronoctua-cache-sweep`
//! deletes what a program can rebuild and deliberately never goes near
//! `$XDG_DATA_HOME`; this deletes the only copy. One binary doing both would be
//! one set of numbers and one set of refusals governing two things that differ
//! in exactly the way that matters.
//!
//! It is a `[[bin]]` of `ronoctua-files` because that crate owns the trash —
//! the freedesktop layout, the `.trashinfo` notes and the date inside them are
//! already there, and a second reader of that format is a second thing to keep
//! in step with the spec.
//!
//! # What it is told, and what it decides
//!
//! Nothing. The age arrives as an argument and the policy lives in
//! `assets/cache.nix`; whether this is a good moment is the brain's question,
//! answered before this is started.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ronoctua_files::trash::Bin;
use ronoctua_files::when::Zone;

/// The policy, generated from `assets/cache.nix` by `nix/cache.nix`.
#[allow(dead_code)]
mod policy {
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

fn main() -> std::process::ExitCode {
    let mut execute = false;
    let mut report: Option<PathBuf> = None;
    let mut days = policy::TRASH_DAYS;
    let mut home: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            // THE SAME WORD THE INSTALLER AND THE CACHE SWEEP USE: the harmless
            // thing is what happens when nobody says anything.
            "--execute" => execute = true,
            "--report" => report = args.next().map(PathBuf::from),
            "--data-home" => home = args.next().map(PathBuf::from),
            "--days" => {
                let Some(n) = args.next().and_then(|n| n.parse::<u16>().ok()) else {
                    eprintln!("ronoctua-empty-trash: --days wants a whole number of days");
                    return std::process::ExitCode::FAILURE;
                };
                days = n;
            }
            "--help" | "-h" => {
                println!("{HELP}");
                return std::process::ExitCode::SUCCESS;
            }
            other => {
                eprintln!("ronoctua-empty-trash: I do not know `{other}`. Try --help.");
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    // THE ROOTS COME FROM THE ENVIRONMENT ONLY WHEN NOBODY NAMED ONE, which is
    // `ronoctua_xdg`'s rule: a program that finds `$XDG_DATA_HOME` on its own is
    // one a test cannot stop from emptying the trash of whoever ran it.
    let data_home = home.or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from));
    let home_dir = std::env::var_os("HOME").map(PathBuf::from);
    let Some(bin) = Bin::of_home(data_home.as_deref(), home_dir.as_deref()) else {
        eprintln!("ronoctua-empty-trash: no XDG_DATA_HOME and no HOME, so no trash");
        return std::process::ExitCode::FAILURE;
    };
    // THE SAME ZONE THAT WROTE THE NOTE. The spec's date has no zone marker, so
    // reading one back in the wrong zone is a file judged hours out — and at
    // four days, hours are what stand between a mistake and a deletion.
    let zone = Zone::read_from(Path::new("/etc/localtime"));

    let began = SystemTime::now();
    if let Some(at) = &report {
        write_sheet(at, began, None);
    }
    let now = began
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    // A REHEARSAL COUNTS AND TAKES NOTHING, down ONE path, so it cannot answer
    // differently from the run.
    let took = bin.expire(now, days, &zone, execute);
    let ended = SystemTime::now();

    println!(
        "{} {} entries ({}), kept {}, {} unanswerable, in {:.1}s — {}",
        if execute { "took" } else { "would take" },
        took.taken,
        in_units(took.bytes),
        took.kept,
        took.refused,
        ended
            .duration_since(began)
            .unwrap_or_default()
            .as_secs_f64(),
        bin.files().display()
    );
    if let Some(at) = &report {
        write_sheet(at, began, Some(ended));
    }
    std::process::ExitCode::SUCCESS
}

const HELP: &str = "\
ronoctua-empty-trash — throw away what has been in the trash long enough

  --days <n>        how long is long enough (default: from assets/cache.nix)
  --data-home <dir> where the trash is (default: $XDG_DATA_HOME)
  --execute         actually delete; without it, count and take nothing
  --report <file>   write the brain's JSON sheet here
  --help

The age is counted from the `DeletionDate` the freedesktop spec writes into
every note — never from the file's own timestamps, because a document written a
year ago and thrown away this morning has been in the trash for a morning.";

/// `1,4 GiB`, in the units a person reads.
fn in_units(bytes: u64) -> String {
    const STEP: f64 = 1024.0;
    #[allow(clippy::cast_precision_loss)] // A trash is not 2^53 bytes.
    let mut left = bytes as f64;
    for unit in ["B", "KiB", "MiB", "GiB"] {
        if left < STEP {
            return format!("{left:.1} {unit}");
        }
        left /= STEP;
    }
    format!("{left:.1} TiB")
}

/// The sheet the brain watches, by hand.
///
/// `ended` is ZERO while it is still going — a moment in 1970 is not a run that
/// ended, and it is the one thing the watcher reads to tell the two apart.
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
    let _ = std::fs::write(at, body);
}
