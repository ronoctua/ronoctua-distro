//! `ronoctua-stick-write [--stopped] <staging>` — the installer stick's root
//! unit (`ronoctua-stick-write.service`), and nothing else.
//!
//! Started by the sudo door's `stick-write` verb the moment the person gave
//! the password, it waits for the staged `ready`, re-reads the drive as the
//! kernel sees it, writes the image with `O_DIRECT` hashing what it writes,
//! and reads it back (`ronoctua_stick::write::unit`). It says where it is in
//! `progress.json`, beside the staging folder, and refuses to run as anyone
//! but root or with any argument but one absolute folder.
//!
//! `--stopped` is the unit's `ExecStopPost`: a last answer that is not final
//! — the unit stopped or killed at work — is turned into a failure
//! (`ronoctua_stick::write::stopped`), so no view waits on a writer that is
//! gone.

use std::path::Path;
use std::process::ExitCode;

use ronoctua_stick::device::Roots;
use ronoctua_stick::write::{self, Ended};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (stopping, staging) = match args.as_slice() {
        [flag, staging] if flag == "--stopped" => (true, staging),
        [staging] => (false, staging),
        _ => {
            eprintln!("usage: ronoctua-stick-write [--stopped] <staging folder>");
            return ExitCode::from(2);
        }
    };
    let staging = Path::new(staging);
    let Some(progress) = staging
        .parent()
        .filter(|_| staging.is_absolute())
        .map(|p| p.join("progress.json"))
    else {
        eprintln!("<3>the staging folder is not an absolute path");
        return ExitCode::from(2);
    };
    if !rustix::process::geteuid().is_root() {
        eprintln!("<3>ronoctua-stick-write runs as root, from its unit");
        return ExitCode::from(2);
    }
    if stopping {
        return match write::stopped(&progress) {
            Ok(true) => {
                eprintln!("<4>the writer stopped at work; its answer now says it failed");
                ExitCode::SUCCESS
            }
            Ok(false) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("<3>the last answer could not be made final: {e}");
                ExitCode::FAILURE
            }
        };
    }
    match write::unit(staging, &progress, &Roots::system()) {
        Ended::Done | Ended::Cancelled(_) => ExitCode::SUCCESS,
        Ended::Refused(_) => ExitCode::FAILURE,
    }
}
