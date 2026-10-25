//! `ronoctua-parts` — root's realizer of the machine's parts.
//!
//! ```text
//! ronoctua-parts plan [--start] [--workers N] [--plan FILE]
//! ronoctua-parts realize ID [--plan FILE]
//! ronoctua-parts view
//! ronoctua-parts trim ID
//! ronoctua-parts status
//! ```
//!
//! - `plan` (`ronoctua-parts.service`: every boot and switch, a network
//!   coming up, the person): the running system's plan read, every record
//!   brought to what it wants, the plan's derivations rooted, the view
//!   rebuilt — and with `--start`, the parts that are not here handed to
//!   their realizers, `--workers` at a time (2 by default), light ones first.
//! - `realize` (`ronoctua-part@<id>.service`): one part built and made its
//!   profile's next generation, or its record says why not. It exits
//!   [`exit::OFFLINE`] with no link, so the unit is not retried on a clock
//!   and the network coming up starts it again.
//! - `view`: the session's view rebuilt from the parts that are here.
//! - `trim`: a part's profile kept to its current generation and the one
//!   before it.
//! - `status`: every record, as JSON lines.
//!
//! The last line on standard output is the verdict; what went wrong in
//! detail goes to standard error, with the journal's priority prefix.
//! `RONOCTUA_NIX` and `RONOCTUA_SYSTEMCTL` name the programs by store path
//! (a unit's `PATH` is minimal); `nix-env` is the one beside that `nix`.

#![forbid(unsafe_code)]

mod disk;
mod plan;
mod realize;
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
        Some("trim") => trim(&args[1..]),
        Some("status") => status(),
        _ => Err(Usage),
    };
    match code {
        Ok(code) => ExitCode::from(code),
        Err(Usage) => {
            eprintln!(
                "usage: ronoctua-parts plan [--start] [--workers N] [--plan FILE]\n\
                 \x20      ronoctua-parts realize ID [--plan FILE]\n\
                 \x20      ronoctua-parts view\n\
                 \x20      ronoctua-parts trim ID\n\
                 \x20      ronoctua-parts status"
            );
            ExitCode::from(exit::USAGE)
        }
    }
}

/// A command line that is not one of the five.
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

/// `nix-env`, beside the `nix` the unit names: the same Nix, never a second
/// variable that could name another.
pub fn nix_env() -> String {
    std::path::Path::new(&program("RONOCTUA_NIX"))
        .with_file_name("nix-env")
        .to_string_lossy()
        .into_owned()
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
