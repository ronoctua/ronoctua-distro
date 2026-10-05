//! `ronoctua-video-metrics`: the scan the unit runs, and the few gestures a
//! test or a person at a terminal needs without the door.
//!
//! ```text
//! ronoctua-video-metrics [--catalog <path>] scan [--feeds-only] [--details <n>] [--report <file>]
//! ronoctua-video-metrics scan-stopped --report <file>
//! ronoctua-video-metrics [--catalog <path>] rescore
//! ronoctua-video-metrics [--catalog <path>] add <address | @handle | channel id>
//! ronoctua-video-metrics [--catalog <path>] hide <video id>
//! ronoctua-video-metrics [--catalog <path>] delete <video id>
//! ronoctua-video-metrics [--catalog <path>] list [--hidden] [--most <n>]
//! ```
//!
//! Every line it prints is for a log or a terminal: English, one line per
//! fact. `scan --report <file>` also writes the brain's sheet there — begun
//! once the scan HOLDS the catalogue, ended when it ends, what went wrong
//! verbatim — which is how the brain watches the unit (`video-metrics`
//! module); a catalogue that is not there yet is nothing to scan, and is not
//! made by a scan. `scan-stopped --report <file>` is the unit's
//! `ExecStopPost`: a scan killed before it ended (stopped, the memory killer)
//! never wrote its end, and this writes it, with systemd's `$SERVICE_RESULT`
//! as the trouble — a sheet that already ended is left as it is. `list`
//! READS — `Catalog::open_ro`, the readers' way in, while a scan may be
//! writing — and prints the videos by score, one per line. Exit status: 0
//! done; 1 refused (the reason on standard error); 2 a command it does not
//! know; 3 a scan stopped early (a bot check, a 429, no network, consent
//! taken back — recorded in the catalogue); 4 a scan in which every channel
//! failed.

use std::path::PathBuf;
use std::process::ExitCode;

use ronoctua_video_metrics::person::{NotAdded, Person};
use ronoctua_video_metrics::read::{Catalog, Filter, Hidden, Page};
use ronoctua_video_metrics::scan::{self, Options};
use ronoctua_video_metrics::{Refusal, VideoId};

const HELP: &str = "\
usage: ronoctua-video-metrics [--catalog <path>] <command>

  scan [--feeds-only] [--details <n>] [--report <file>]
                                        scan every channel of the catalogue
  scan-stopped --report <file>          end the report a killed scan left open
                                        (the unit's ExecStopPost)
  rescore                               recompute every video's static points
  add <address | @handle | channel id>  add a channel (and let its hosts be asked)
  hide <video id>                       hide a video
  delete <video id>                     delete a video for good (it never returns)
  list [--hidden] [--most <n>]          the videos by score, read-only: id, score, title
                                        (--hidden: the hidden ones too)

The catalogue is $XDG_DATA_HOME/ronoctua/video-metrics/catalog.sqlite unless
--catalog names another.";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let catalog = match take_value(&mut args, "--catalog") {
        Ok(Some(path)) => Some(PathBuf::from(path)),
        Ok(None) => ronoctua_video_metrics::default_path(),
        Err(flag) => return usage(&format!("{flag} needs a value")),
    };
    let Some(catalog) = catalog else {
        eprintln!("no home: neither XDG_DATA_HOME nor HOME is set");
        return ExitCode::from(1);
    };
    let Some(command) = (!args.is_empty()).then(|| args.remove(0)) else {
        return usage("a command is needed");
    };
    match command.as_str() {
        "scan" => scan_now(&catalog, args),
        "scan-stopped" => scan_stopped(args),
        "rescore" => done(Person::open(&catalog).and_then(|p| p.rescore()), |n| {
            println!("rescored: {n} videos changed");
        }),
        "add" => match args.as_slice() {
            [input] => add(&catalog, input),
            _ => usage("add takes one address, @handle or channel id"),
        },
        "hide" | "delete" => match args.as_slice() {
            [id] => {
                let id = VideoId(id.clone());
                let result = Catalog::open_ro(&catalog).and_then(|c| {
                    if command == "hide" {
                        c.hide(&id)
                    } else {
                        c.delete(&id)
                    }
                });
                done(result, |()| println!("{command}: {}", id.0))
            }
            _ => usage(&format!("{command} takes one video id")),
        },
        "list" => list(&catalog, args),
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        other => usage(&format!("unknown command `{other}`")),
    }
}

/// Takes `--flag <value>` out of `args`; `Err` names a flag without a value.
fn take_value(args: &mut Vec<String>, flag: &'static str) -> Result<Option<String>, &'static str> {
    let Some(at) = args.iter().position(|a| a == flag) else {
        return Ok(None);
    };
    if at + 1 >= args.len() {
        return Err(flag);
    }
    let value = args.remove(at + 1);
    args.remove(at);
    Ok(Some(value))
}

fn usage(why: &str) -> ExitCode {
    eprintln!("{why}\n\n{HELP}");
    ExitCode::from(2)
}

/// Prints a refusal as its reason and exits 1; runs `said` on success.
fn done<T>(result: Result<T, Refusal>, said: impl FnOnce(T)) -> ExitCode {
    match result {
        Ok(value) => {
            said(value);
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            eprintln!("refused: {}", refusal.why.id());
            ExitCode::from(1)
        }
    }
}

/// How many videos `list` prints unless `--most` says otherwise.
const LIST_MOST: u32 = 100;

/// `list`: the videos by score, read through the readers' own door
/// (`Catalog::open_ro`) — one line each, `<id> <score> <title>`, tab-separated.
fn list(catalog: &std::path::Path, mut args: Vec<String>) -> ExitCode {
    let mut filter = Filter::default();
    if let Some(at) = args.iter().position(|a| a == "--hidden") {
        args.remove(at);
        filter.hidden = Hidden::Any;
    }
    let most = match take_value(&mut args, "--most") {
        Ok(Some(n)) => match n.parse::<u32>() {
            Ok(n) if n > 0 => n,
            _ => return usage(&format!("--most wants a whole number above 0, not `{n}`")),
        },
        Ok(None) => LIST_MOST,
        Err(flag) => return usage(&format!("{flag} needs a value")),
    };
    if let Some(extra) = args.first() {
        return usage(&format!("list does not take `{extra}`"));
    }
    let page = Page {
        number: 0,
        per_page: most,
    };
    done(
        Catalog::open_ro(catalog).and_then(|c| c.ranked(&filter, page)),
        |rows| {
            for v in rows {
                println!("{}\t{}\t{}", v.id.0, v.score, v.title);
            }
        },
    )
}

fn scan_now(catalog: &std::path::Path, mut args: Vec<String>) -> ExitCode {
    let mut options = Options::default();
    if let Some(at) = args.iter().position(|a| a == "--feeds-only") {
        args.remove(at);
        options.feeds_only = true;
    }
    match take_value(&mut args, "--details") {
        Ok(Some(n)) => match n.parse() {
            Ok(n) => options.details_most = n,
            Err(_) => return usage("--details takes a whole number"),
        },
        Ok(None) => {}
        Err(flag) => return usage(&format!("{flag} needs a value")),
    }
    let sheet = match take_value(&mut args, "--report") {
        Ok(at) => at.map(PathBuf::from),
        Err(flag) => return usage(&format!("{flag} needs a value")),
    };
    if let Some(extra) = args.first() {
        return usage(&format!("scan does not take `{extra}`"));
    }
    // NO CATALOGUE, NO CHANNELS: nothing to scan, and a scan does not make
    // one — the door does, when the person adds a channel.
    if !catalog.exists() {
        println!("nothing to scan: {} is not there", catalog.display());
        if let Some(at) = &sheet {
            let now = now_s();
            write_sheet(at, now, Some(now), "");
        }
        return ExitCode::SUCCESS;
    }
    // "RUNNING" IS WRITTEN ONCE THE SCAN HOLDS THE CATALOGUE: the brain trusts
    // a begun report only while the scan's lock is held (`Catalog::scanning`),
    // and a report written before the lock would be one it calls dead.
    let began = std::cell::Cell::new(now_s());
    let report = match scan::run_announcing(catalog, &options, &mut || {
        began.set(now_s());
        if let Some(at) = &sheet {
            write_sheet(at, began.get(), None, "");
        }
    }) {
        Ok(report) => report,
        Err(refusal) => {
            eprintln!("refused: {}", refusal.why.id());
            if let Some(at) = &sheet {
                write_sheet(
                    at,
                    began.get(),
                    Some(now_s()),
                    &format!("refused: {}", refusal.why.id()),
                );
            }
            return ExitCode::from(1);
        }
    };
    let began = began.get();
    println!(
        "scanned {} channels ({} failed): {} added, {} skipped, {} detailed, {} pending, {} thumbnails",
        report.channels,
        report.channels_failed,
        report.added,
        report.skipped,
        report.detailed,
        report.pending,
        report.thumbs
    );
    let (trouble, code) = if let Some(stop) = report.stopped {
        (
            format!(
                "stopped: {}{}",
                stop.id(),
                report.said.map(|s| format!(" ({s})")).unwrap_or_default()
            ),
            3,
        )
    } else if report.channels > 0 && report.channels_failed == report.channels {
        ("every channel failed".to_owned(), 4)
    } else {
        (String::new(), 0)
    };
    if let Some(at) = &sheet {
        write_sheet(at, began, Some(now_s()), &trouble);
    }
    if code != 0 {
        eprintln!("{trouble}");
    }
    ExitCode::from(code)
}

/// `scan-stopped --report <file>`: the unit's `ExecStopPost`. A scan the
/// brain gave up on, `systemctl stop` or the memory killer ended never wrote
/// its end, and the brain would wait out its patience for it; this writes
/// the end, with what systemd says happened (`$SERVICE_RESULT`: `signal`,
/// `oom-kill`, …) as the trouble. A sheet that ended — the scan's own last
/// write — or none at all is left alone, and so is one that reads as no
/// sheet: this never invents a scan.
fn scan_stopped(mut args: Vec<String>) -> ExitCode {
    let at = match take_value(&mut args, "--report") {
        Ok(Some(at)) => PathBuf::from(at),
        Ok(None) => return usage("scan-stopped needs --report <file>"),
        Err(flag) => return usage(&format!("{flag} needs a value")),
    };
    if let Some(extra) = args.first() {
        return usage(&format!("scan-stopped does not take `{extra}`"));
    }
    let Some(sheet) = std::fs::read_to_string(&at)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
    else {
        println!("no report at {}: nothing to end", at.display());
        return ExitCode::SUCCESS;
    };
    let number = |k: &str| sheet.get(k).and_then(serde_json::Value::as_i64);
    match (number("began"), number("ended")) {
        (Some(began), Some(0)) if began != 0 => {
            let result = std::env::var("SERVICE_RESULT").unwrap_or_default();
            let trouble = if result.is_empty() || result == "success" {
                "stopped before it ended".to_owned()
            } else {
                format!("stopped before it ended: {result}")
            };
            write_sheet(&at, began, Some(now_s()), &trouble);
            println!("{trouble}");
        }
        _ => println!("the report at {} had ended", at.display()),
    }
    ExitCode::SUCCESS
}

/// Now, in seconds since the epoch.
fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// Writes the brain's sheet (`ronoctua_brain::task::Sheet`): when the scan
/// began, when it ended (ZERO while it runs), no steps of the brain's, and
/// what went wrong verbatim — WHOLE AND RENAMED, so a read never lands on
/// half a file, the draft named with the PID for the reason every file two
/// processes may write is.
///
/// FAILURE IS SILENCE, and on purpose: the sheet is how the brain watches,
/// and a scan that refused to run for want of a status file would be a scan
/// the brain waits out for nothing.
fn write_sheet(at: &std::path::Path, began: i64, ended: Option<i64>, trouble: &str) {
    let body = serde_json::json!({
        "began": began,
        "ended": ended.unwrap_or(0),
        "steps": [],
        "trouble": trouble,
    })
    .to_string();
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

fn add(catalog: &std::path::Path, input: &str) -> ExitCode {
    let person = match Person::open(catalog) {
        Ok(person) => person,
        Err(refusal) => {
            eprintln!("refused: {}", refusal.why.id());
            return ExitCode::from(1);
        }
    };
    // ADDING A CHANNEL IS THE CONSENT TO SCAN IT: its feeds' and thumbnails'
    // hosts are let by the same call, and a scan mints its passes from that.
    match person.add_from(input) {
        Ok(channel) => {
            println!("added: {} ({})", channel.id.0, channel.title);
            ExitCode::SUCCESS
        }
        Err(NotAdded::Unresolved { trouble, line, .. }) => {
            eprintln!(
                "refused: {}{}",
                trouble.id(),
                line.map(|l| format!(" ({l})")).unwrap_or_default()
            );
            ExitCode::from(1)
        }
        Err(NotAdded::Refused(refusal)) => {
            eprintln!("refused: {}", refusal.why.id());
            ExitCode::from(1)
        }
        Err(NotAdded::NotLet(unkept)) => {
            eprintln!(
                "consent not kept: {} — a scan will not ask YouTube",
                unkept.id()
            );
            ExitCode::from(1)
        }
    }
}
