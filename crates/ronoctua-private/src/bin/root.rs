//! `ronoctua-private-root` — the one program that opens, closes and changes
//! the `/private` vault, as root.
//!
//! # Who runs it
//!
//! Three fixed units (`nix/modules/private.nix`), and nobody else:
//!
//! - `ronoctua-private-root.service` — `run <staging>`: the one request the
//!   person staged. Started by the sudo door's `private-root` verb.
//! - `ronoctua-private-lock.service` — `lock`, before sleep.
//! - `ronoctua-private-status.service` — `status`, at boot and after a switch.
//!
//! It refuses to run as anybody but root. Its exit status is the verdict
//! (0 done, 1 refused); WHY it refused is in
//! `/run/ronoctua-private/verdict.json`, and the details in the journal.

use std::path::Path;
use std::process::ExitCode;

use ronoctua_private::root::{Machine, Tools};

const USAGE: &str = "usage: ronoctua-private-root run <staging> | lock | status";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !rustix::process::geteuid().is_root() {
        eprintln!("<3>ronoctua-private-root runs as root, from its units");
        return ExitCode::from(2);
    }
    let machine = Machine::system(Tools::from_env());
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let done = match words.as_slice() {
        ["run", staging] => machine.run_staged(Path::new(staging)),
        ["lock"] => machine.lock().map(|state| {
            eprintln!("<6>the vault before sleep: {state:?}");
        }),
        ["status"] => {
            let status = machine.publish();
            eprintln!("<6>the vault: {:?}", status.state);
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(refusal) => {
            eprintln!("<4>refused: {refusal}");
            ExitCode::FAILURE
        }
    }
}
