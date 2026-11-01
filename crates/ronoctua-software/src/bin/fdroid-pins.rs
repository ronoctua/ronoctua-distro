//! `ronoctua-fdroid-pins` — F-Droid's signed index, checked for
//! `nix run .#distro-pins`; see `ronoctua_software::pins::signed`.

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match ronoctua_software::pins::signed::run(&args, ronoctua_software::fdroid::SIGNER) {
        Ok(line) => {
            let mut out = std::io::stdout().lock();
            if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err((2, usage)) => {
            let _ = write!(std::io::stderr(), "{usage}");
            ExitCode::from(2)
        }
        Err((code, said)) => {
            let _ = writeln!(std::io::stderr(), "ronoctua-fdroid-pins: {said}");
            ExitCode::from(code)
        }
    }
}
