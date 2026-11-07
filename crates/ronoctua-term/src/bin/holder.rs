//! `ronoctua-term-holder serve …` — one terminal's own process.
//!
//! Started by a window through `ronoctua_terminals::spawn::start`, in a scope
//! of its own (`<name>-term-<id>`), never by hand. It says
//! `ronoctua_terminals::spawn::READY` on its stdout once it can be attached
//! to, then lets stdout go: the one who waited for the line has gone.
//!
//! Its exit status is its own, not the program's: the program's end is in
//! the terminal's state, where the window and the footer read it.

use std::io::Write as _;
use std::process::ExitCode;

use ronoctua_term::holder::{self, Outcome, Setup};
use ronoctua_terminals::registry::Registry;
use ronoctua_terminals::spawn::{READY, Serve};

fn main() -> ExitCode {
    ronoctua_log::init("info");
    let mut serve = match Serve::parse(std::env::args_os().skip(1)) {
        Ok(s) => s,
        Err(why) => {
            eprintln!("ronoctua-term-holder: {why}");
            return ExitCode::from(2);
        }
    };
    let Some(registry) = Registry::of_session() else {
        eprintln!(
            "ronoctua-term-holder: no XDG_RUNTIME_DIR, so no place for the terminal's socket"
        );
        return ExitCode::from(2);
    };
    // The program starts where the window was — and the holder itself
    // stands at `/`, so a terminal open for days does not keep a drive it
    // was started from busy.
    if serve.cwd.is_none() {
        serve.cwd = std::env::current_dir().ok();
    }
    let _ = std::env::set_current_dir("/");
    let id = serve.id.clone();
    let setup = Setup::served(serve, env!("CARGO_PKG_VERSION"), registry);
    let outcome = holder::serve(setup, || {
        let mut out = std::io::stdout();
        let _ = writeln!(out, "{READY}");
        let _ = out.flush();
        // Nothing more is written there, and the one who read the line is
        // gone: a later write would land on a closed pipe.
        if let Ok(null) = std::fs::File::options().write(true).open("/dev/null") {
            let _ = rustix::stdio::dup2_stdout(&null);
        }
    });
    match outcome {
        // The program was left to the kernel's hang-up, which comes when
        // this process exits — now, with nothing of ours to drop on the way.
        Ok(Outcome::HungUp) => std::process::exit(0),
        Ok(Outcome::Ended { .. }) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(%id, error = format!("{e:#}"), "the terminal could not start");
            ExitCode::FAILURE
        }
    }
}
