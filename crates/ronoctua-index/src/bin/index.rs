//! `ronoctua-index`: the unit that builds fast-access's index, and three
//! one-shot commands the checks drive.
//!
//! ```text
//! ronoctua-index serve  LIMITS [--dir D] [--root R]... [--sources LIST]
//! ronoctua-index build  LIMITS [--dir D] [--root R]... [--sources LIST]
//! ronoctua-index query  [--dir D] [--names] [--most N] WORD...
//! ronoctua-index wait   [--dir D] --timeout-s N WORD...
//!
//! LIMITS: --ceiling-mib N --content-file-mib N --content-kib N --memory-mib N [--skip NAME]...
//! ```
//!
//! The limits are `assets/cache.nix`'s and the unit is started with them
//! (`nix/modules/index.nix`); none has a default here, so a unit started
//! without one is refused instead of indexing to a number nobody chose.
//!
//! `wait` is a reader exactly as a door is one: it opens the index with no
//! reload clock and reloads when `meta.json` changes (`on_commit`), then
//! asks again; it answers when the words are found, or fails at its deadline.
//! It prints `waiting` once its watch is armed and the words are not there,
//! and `found after N reloads` with the hits when they are.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use ronoctua_index::crawl::Limits;
use ronoctua_index::serve::{Config, Sources, Unit};
use ronoctua_index::videos::Catalogue;
use ronoctua_index::{Index, Kind, Query, Why};

const USAGE: &str = "usage: ronoctua-index serve|build --ceiling-mib N --content-file-mib N --content-kib N --memory-mib N [--skip NAME]... [--dir D] [--root R]... [--sources files,manuals,packages,videos]
       ronoctua-index query [--dir D] [--names] [--most N] WORD...
       ronoctua-index wait [--dir D] --timeout-s N WORD...";

/// The command line, read.
#[derive(Debug, Default)]
struct Args {
    dir: Option<PathBuf>,
    ceiling_mib: Option<u64>,
    content_file_mib: Option<u64>,
    content_kib: Option<usize>,
    memory_mib: Option<usize>,
    skip: Vec<String>,
    roots: Vec<PathBuf>,
    sources: Option<Sources>,
    names: bool,
    most: Option<usize>,
    timeout_s: Option<u64>,
    words: Vec<String>,
}

fn parse(mut rest: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut args = Args::default();
    while let Some(arg) = rest.next() {
        let mut value = |name: &str| rest.next().ok_or_else(|| format!("{name} needs a value"));
        let number = |name: &str, v: String| {
            v.parse::<u64>()
                .map_err(|_| format!("{name}: {v} is not a whole number"))
        };
        match arg.as_str() {
            "--dir" => args.dir = Some(PathBuf::from(value("--dir")?)),
            "--ceiling-mib" => {
                args.ceiling_mib = Some(number("--ceiling-mib", value("--ceiling-mib")?)?)
            }
            "--content-file-mib" => {
                args.content_file_mib =
                    Some(number("--content-file-mib", value("--content-file-mib")?)?)
            }
            "--content-kib" => {
                args.content_kib = Some(
                    usize::try_from(number("--content-kib", value("--content-kib")?)?)
                        .map_err(|_| "--content-kib is too large".to_owned())?,
                );
            }
            "--memory-mib" => {
                args.memory_mib = Some(
                    usize::try_from(number("--memory-mib", value("--memory-mib")?)?)
                        .map_err(|_| "--memory-mib is too large".to_owned())?,
                );
            }
            "--skip" => {
                // `searchSkip`, checked as `index.json`'s skips are: an entry
                // that is neither a name nor an absolute path is refused, and
                // the unit does not start on a skip that skips nothing.
                let skip = value("--skip")?;
                ronoctua_index::settings::skip_entry(&skip)
                    .map_err(|why| format!("--skip: {why}"))?;
                args.skip.push(skip);
            }
            "--root" => {
                let root = PathBuf::from(value("--root")?);
                if !root.is_absolute() {
                    return Err(format!("--root {} is not an absolute path", root.display()));
                }
                args.roots.push(root);
            }
            "--sources" => {
                args.sources = Some(
                    Sources::parse(&value("--sources")?)
                        .map_err(|w| format!("--sources: {w} is not a source"))?,
                );
            }
            "--names" => args.names = true,
            "--most" => {
                args.most =
                    Some(usize::try_from(number("--most", value("--most")?)?).unwrap_or(usize::MAX))
            }
            "--timeout-s" => args.timeout_s = Some(number("--timeout-s", value("--timeout-s")?)?),
            other if other.starts_with("--") => return Err(format!("{other} is not an option")),
            word => args.words.push(word.to_owned()),
        }
    }
    Ok(args)
}

fn dir_of(args: &Args) -> Result<PathBuf, String> {
    args.dir
        .clone()
        .or_else(ronoctua_index::default_dir)
        .ok_or_else(|| "no --dir and no home to put the index in".to_owned())
}

/// The limits, every one of them given.
fn limits_of(args: &Args) -> Result<(Limits, usize), String> {
    let need = |name: &str| format!("{name} is required (assets/cache.nix gives it to the unit)");
    let ceiling = args.ceiling_mib.ok_or_else(|| need("--ceiling-mib"))?;
    let file = args
        .content_file_mib
        .ok_or_else(|| need("--content-file-mib"))?;
    let kib = args.content_kib.ok_or_else(|| need("--content-kib"))?;
    let memory = args.memory_mib.ok_or_else(|| need("--memory-mib"))?;
    if ceiling == 0 || memory == 0 {
        return Err("--ceiling-mib and --memory-mib must be more than zero".to_owned());
    }
    Ok((
        Limits {
            ceiling_bytes: ceiling << 20,
            content_file_bytes: file << 20,
            content_bytes: kib << 10,
            skip: args.skip.clone(),
        },
        memory << 20,
    ))
}

fn config_of(args: &Args) -> Result<Config, String> {
    let (limits, memory_bytes) = limits_of(args)?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".local/state")));
    let user = std::env::var("USER").ok();
    let man_candidates =
        ronoctua_index::man::candidates(user.as_deref(), home.as_deref(), state.as_deref());
    // What moves when the system switches or the person's profile changes.
    let mut system_triggers = vec![PathBuf::from("/run/current-system")];
    if let Some(home) = &home {
        system_triggers.push(home.join(".nix-profile"));
    }
    if let Some(state) = &state {
        system_triggers.push(state.join("nix/profile"));
    }
    Ok(Config {
        dir: dir_of(args)?,
        limits,
        memory_bytes,
        roots: (!args.roots.is_empty()).then(|| args.roots.clone()),
        sources: args.sources.unwrap_or(Sources::ALL),
        home,
        settings_dir: ronoctua_store::config_dir(),
        man_candidates,
        packages: std::env::var_os(ronoctua_index::packages::VARIABLE).map(PathBuf::from),
        videos: ronoctua_video_metrics::default_path()
            .map(|p| Box::new(Catalogue::at(&p)) as Box<dyn ronoctua_index::videos::Videos>),
        system_triggers,
    })
}

fn refused(what: &str, why: Why) -> String {
    format!("{what}: the index refused ({})", why.id())
}

fn build(args: &Args) -> Result<(), String> {
    let config = config_of(args)?;
    let dir = config.dir.clone();
    let mut unit = Unit::new(config).map_err(|r| refused("open", r.why))?;
    let pass = unit.pass(None).map_err(|r| refused("build", r.why))?;
    let writer = unit.into_writer();
    let documents = writer.documents();
    writer.finish().map_err(|r| refused("finish", r.why))?;
    println!(
        "indexed: {} entries walked, {} written, {} deleted, {} manual pages, {} packages, {} videos; {documents} documents; the folder holds {} bytes",
        pass.files.entries,
        pass.files.indexed,
        pass.files.deleted,
        pass.manuals,
        pass.packages,
        pass.videos,
        ronoctua_index::write::size_of_dir(&dir),
    );
    Ok(())
}

fn serve(args: &Args) -> Result<(), String> {
    let config = config_of(args)?;
    let mut unit = Unit::new(config).map_err(|r| refused("open", r.why))?;
    unit.serve().map_err(|r| refused("serve", r.why))
}

fn question(args: &Args) -> Query {
    let mut q = Query::of_text(&args.words.join(" "), args.most.unwrap_or(20));
    if args.names {
        q.contents = false;
    }
    q
}

fn print_hits(hits: &[ronoctua_index::Hit]) {
    for hit in hits {
        println!(
            "{}\t{}\t{}\t{:.3}",
            hit.kind.id(),
            hit.id,
            hit.name,
            hit.score
        );
    }
}

fn query(args: &Args) -> Result<(), String> {
    let index = Index::open_ro(&dir_of(args)?).map_err(|r| refused("open", r.why))?;
    let hits = index
        .query(&question(args))
        .map_err(|r| refused("query", r.why))?;
    print_hits(&hits);
    Ok(())
}

/// Whether every word is in the name of one of the hits (the checks plant a
/// name and wait for it, not for a prefix of something else).
fn answers(hits: &[ronoctua_index::Hit], words: &[String]) -> bool {
    hits.iter().any(|h| {
        matches!(h.kind, Kind::File | Kind::Folder | Kind::Content)
            && words.iter().all(|w| {
                ronoctua_index::words::folded(&h.name).contains(&ronoctua_index::words::folded(w))
            })
    })
}

fn wait(args: &Args) -> Result<(), String> {
    let dir = dir_of(args)?;
    let timeout = Duration::from_secs(args.timeout_s.ok_or("--timeout-s is required")?);
    if args.words.is_empty() {
        return Err("wait needs the words to wait for".to_owned());
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let deadline = Instant::now() + timeout;
    let (told, heard) = std::sync::mpsc::channel::<()>();
    // The door's watch: `meta.json` by its folder and name (the writer
    // renames it into place). No clock: the reader waits for the kernel.
    let _watch = ronoctua_index::on_commit(&dir, move || told.send(()).is_ok())
        .map_err(|r| refused("watch", r.why))?;
    let mut index: Option<Index> = None;
    let mut reloads = 0u32;
    let mut said_waiting = false;
    loop {
        match index.as_mut() {
            None => match Index::open_ro(&dir) {
                Ok(opened) => index = Some(opened),
                Err(r) if r.why == Why::NotBuilt => {}
                Err(r) => return Err(refused("open", r.why)),
            },
            Some(opened) => {
                opened.reload().map_err(|r| refused("reload", r.why))?;
                reloads += 1;
            }
        }
        if let Some(opened) = &index {
            let hits = opened
                .query(&question(args))
                .map_err(|r| refused("query", r.why))?;
            if answers(&hits, &args.words) {
                println!("found after {reloads} reloads");
                print_hits(&hits);
                return Ok(());
            }
        }
        // ONCE, when the watch is armed and the words are not there yet: a
        // check plants its file after this line, so what the reader finds
        // next it can only have seen through a reload.
        if !said_waiting {
            said_waiting = true;
            println!("waiting");
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || heard.recv_timeout(left).is_err() {
            return Err(format!(
                "no commit answered {:?} within {} s ({reloads} reloads)",
                args.words.join(" "),
                timeout.as_secs()
            ));
        }
    }
}

fn main() -> ExitCode {
    let mut argv = std::env::args().skip(1);
    let Some(command) = argv.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let args = match parse(argv) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("ronoctua-index: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if matches!(command.as_str(), "serve" | "build") {
        use tracing_subscriber::layer::SubscriberExt as _;
        use tracing_subscriber::util::SubscriberInitExt as _;
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "ronoctua_index=info".into()),
            )
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .init();
    }
    let result = match command.as_str() {
        "serve" => serve(&args),
        "build" => build(&args),
        "query" => query(&args),
        "wait" => wait(&args),
        other => Err(format!("{other} is not a command\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ronoctua-index: {e}");
            ExitCode::FAILURE
        }
    }
}
