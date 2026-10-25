//! `ronoctua-parts` — root's realizer of the machine's parts.
//!
//! ```text
//! ronoctua-parts plan [--start | --rehearse] [--workers N] [--plan FILE]
//! ronoctua-parts realize ID [--plan FILE]
//! ronoctua-parts view
//! ronoctua-parts accept
//! ronoctua-parts import
//! ronoctua-parts export
//! ronoctua-parts trim ID
//! ronoctua-parts status
//! ```
//!
//! - `plan` (`ronoctua-parts.service`: every boot and switch, a network
//!   coming up, the person): the running system's plan read, every record
//!   brought to what it wants, the plan's derivations rooted, the view
//!   rebuilt — and with `--start`, the parts that are not here handed to
//!   their realizers, `--workers` at a time (2 by default), light ones first.
//!   `--rehearse` says what it would do and writes nothing.
//! - `realize` (`ronoctua-part@<id>.service`): one part built and made its
//!   profile's next generation, or its record says why not. It exits
//!   [`exit::OFFLINE`] with no link, so the unit is not retried on a clock
//!   and the network coming up starts it again.
//! - `view`: the session's view rebuilt from the parts that are here.
//! - `accept` (`ronoctua-parts-accept.service`): the files the person handed
//!   in, each taken only when Nix finds the hash the flake pins.
//! - `import` (`ronoctua-parts-import.service`): the downloads a part is
//!   missing, from the stick folder the person named — downloads only.
//! - `export` (`ronoctua-parts-export.service`): the downloads this machine
//!   keeps, into `<folder>/ronoctua-parts` on the stick the person named,
//!   for another machine's `import`.
//! - `trim`: a part's profile kept to its current generation and the one
//!   before it.
//! - `status`: every record, as JSON lines.
//! - `seed --root DIR` (the installer): the parts the installation medium
//!   carries, into the store of the machine it installed under `DIR`.
//!
//! The last line on standard output is the verdict; what went wrong in
//! detail goes to standard error, with the journal's priority prefix.
//! `RONOCTUA_NIX`, `RONOCTUA_SYSTEMCTL` and `RONOCTUA_SYSTEMD_RUN` name the programs by store path
//! (a unit's `PATH` is minimal); `nix-env` is the one beside that `nix`.

#![forbid(unsafe_code)]

mod disk;
mod handin;
mod keep;
mod person;
mod plan;
mod realize;
mod retry;
mod seed;
mod stick;
mod view;

use std::path::PathBuf;
use std::process::ExitCode;

use ronoctua_parts::exit;
use ronoctua_parts::names;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("plan") => plan::run(&args[1..]),
        Some("realize") => realize::run(&args[1..]),
        Some("view") => view::run(),
        Some("accept") => handin::run(&args[1..]),
        Some("import") => stick::import(&args[1..]),
        Some("export") => stick::export(&args[1..]),
        Some("trim") => trim(&args[1..]),
        Some("seed") => seed::run(&args[1..]),
        Some("status") => status(),
        _ => Err(Usage),
    };
    match code {
        Ok(code) => ExitCode::from(code),
        Err(Usage) => {
            eprintln!(
                "usage: ronoctua-parts plan [--start | --rehearse] [--workers N] [--plan FILE]\n\
                 \x20      ronoctua-parts realize ID [--plan FILE]\n\
                 \x20      ronoctua-parts view\n\
                 \x20      ronoctua-parts accept\n\
                 \x20      ronoctua-parts import\n\
                 \x20      ronoctua-parts export\n\
                 \x20      ronoctua-parts trim ID\n\
                 \x20      ronoctua-parts seed --root DIR\n\
                 \x20      ronoctua-parts status"
            );
            ExitCode::from(exit::USAGE)
        }
    }
}

/// A command line that is not one of the nine.
pub struct Usage;

/// A program named by its store path in the unit's environment.
pub fn program(variable: &str) -> String {
    std::env::var(variable).unwrap_or_else(|_| {
        say(
            3,
            &format!("{variable} is not set: the unit names its programs"),
        );
        std::process::exit(i32::from(exit::USAGE));
    })
}

/// A program of Nix's beside the `nix` the unit names (`nix-env`,
/// `nix-store`): the same Nix, never a second variable that could name
/// another.
pub fn beside_nix(name: &str) -> String {
    std::path::Path::new(&program("RONOCTUA_NIX"))
        .with_file_name(name)
        .to_string_lossy()
        .into_owned()
}

/// `nix-env`, beside the unit's `nix`.
pub fn nix_env() -> String {
    beside_nix("nix-env")
}

/// A line for the journal, with its priority (`<3>` error, `<4>` warning,
/// `<6>` information).
pub fn say(priority: u8, line: &str) {
    eprintln!("<{priority}>{line}");
}

/// The plan file: `--plan FILE`, or the running system's.
pub fn plan_path(flags: &[String]) -> Result<PathBuf, Usage> {
    match flags.iter().position(|f| f == "--plan") {
        Some(at) => flags.get(at + 1).map(PathBuf::from).ok_or(Usage),
        None => Ok(PathBuf::from(names::PLAN)),
    }
}

fn trim(args: &[String]) -> Result<u8, Usage> {
    let [id] = args else { return Err(Usage) };
    if !ronoctua_parts::id::is_id(id) {
        return Err(Usage);
    }
    let Ok(record) = disk::read_record(id) else {
        println!("refused unknown-part");
        return Ok(exit::UNWANTED);
    };
    let interfaces: Vec<u32> = record
        .current
        .iter()
        .chain(record.previous.iter())
        .map(|g| g.interface)
        .collect();
    let nix_env = nix_env();
    for interface in interfaces {
        let profile = names::profile(id, interface);
        let argv = ronoctua_parts::argv::trim_profile(&nix_env, &profile.to_string_lossy());
        if !disk::run_quiet(&argv) {
            say(
                4,
                &format!(
                    "{id}: the generations of {} were not trimmed",
                    profile.display()
                ),
            );
        }
    }
    println!("trimmed {id}");
    Ok(exit::DONE)
}

fn status() -> Result<u8, Usage> {
    for record in disk::read_records() {
        println!("{}", serde_json::to_string(&record).unwrap_or_default());
    }
    Ok(exit::DONE)
}
