//! `ronoctua-cachix push [--rehearse]` — the update's last step: the push to
//! the person's binary cache, as the person, after an update that switched
//! the machine (`ronoctua_cachix::push`).
//!
//! Progress goes to standard error (the update's journal); the last line on
//! standard output is the verdict the update reads: `pushed <n>`, `would
//! push <n>` for a rehearsal, or `refused <why>` — three of whose words are
//! news to the step (`no-token`, `not-let`, `nothing-new`) and the rest a
//! failure that says why. It exits `0` only when it pushed (or rehearsed).
//! Nothing secret is an argument: the token is read from the person's sealed
//! folder and handed to `cachix` in its own environment.

use std::io::Write as _;
use std::process::ExitCode;

use ronoctua_cachix::Why;
use ronoctua_cachix::push::{self, Places, Tools};
use ronoctua_cachix::record::{Outcome, Record};
use ronoctua_person::Stamp;

/// The usage, in English (invariant 18: a command line's help).
const USAGE: &str = "usage: ronoctua-cachix push [--rehearse]
Pushes to the person's binary cache what this machine built, after an update
that switched it: a public cache receives only the distro's own free outputs,
a private one the system's closure, and neither what cache.nixos.org serves.
--rehearse says what would go, and pushes nothing.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let rehearse = match words.as_slice() {
        ["push"] => false,
        ["push", "--rehearse"] => true,
        ["-h" | "--help"] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let now = Stamp::from_unix_s(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX)),
    );
    // THE TOOLS ARE ASKED ONLY ONCE THERE IS SOMETHING TO PUSH: a machine
    // with no cache is not given `cachix`, and says `no-cache`, not that a
    // tool is missing.
    let tools = Tools::from_env().ok();
    let record = match Places::here() {
        Ok(places) => push::push(
            &places,
            tools.as_ref(),
            &ronoctua_consent::pass,
            rehearse,
            now,
            &mut |line| eprintln!("{line}"),
        ),
        Err(_) => Record {
            at: now,
            cache: None,
            visibility: None,
            applied_at: None,
            outcome: Outcome::Refused { why: Why::NoTools },
        },
    };
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", push::last_line(&record));
    let _ = out.flush();
    match record.outcome {
        Outcome::Pushed { .. } | Outcome::Rehearsed { .. } => ExitCode::SUCCESS,
        Outcome::Refused { .. } => ExitCode::FAILURE,
    }
}
