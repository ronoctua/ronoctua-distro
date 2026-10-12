//! `ronoctua-incus-job` — one piece of long Incus work, for the queue.
//!
//! Reads ONE `ronoctua_incus::Job` as a JSON line on stdin, does it, and
//! prints where it is on stdout, a [`ronoctua_incus::Line`] per line, ending
//! with `done` or `refused <why>`. Its log — the daemon's own words for a
//! refusal — goes to stderr, which the queue keeps as the row's trouble
//! after the refusal in words: plain lines, no level and no time.
//!
//! `SIGTERM` (the queue's cancel) cancels the daemon's operation, removes
//! the half-made file, and the job leaves with no verdict line: the queue
//! knows why it ended.

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ronoctua_incus::job::{self, Ended, Hearing};
use ronoctua_incus::{Incus, Line};

/// The most one job may be, as a line.
const JOB_MOST_BYTES: u64 = 64 << 10;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .without_time()
        .with_level(false)
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();
    if std::env::args_os().len() > 1 {
        eprintln!("usage: ronoctua-incus-job < job.json (one ronoctua_incus::Job, as JSON)");
        return ExitCode::from(2);
    }
    let say = |line: &Line| println!("{}", line.text());
    let job = match job::job_of(&mut std::io::stdin().lock(), JOB_MOST_BYTES) {
        Ok(job) => job,
        Err(refusal) => {
            say(&Line::Refused(refusal.why));
            return ExitCode::FAILURE;
        }
    };
    let incus = match Incus::from_env() {
        Ok(incus) => incus,
        Err(refusal) => {
            say(&Line::Refused(refusal.why));
            return ExitCode::FAILURE;
        }
    };
    // THE QUEUE'S CANCEL, heard between two polls and two pieces.
    let stopping = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        if let Err(e) = signal_hook::flag::register(signal, Arc::clone(&stopping)) {
            tracing::warn!(error = %e, "a cancel could not be heard");
        }
    }
    let stop = || stopping.load(Ordering::Relaxed);
    let mut print = |line: &Line| say(line);
    let minter = |host: &str| ronoctua_consent::pass(host).ok();
    match job::run(&incus, &job, &minter, &mut Hearing::new(&stop, &mut print)) {
        Ok(()) => {
            say(&Line::Done);
            ExitCode::SUCCESS
        }
        Err(Ended::Refused(refusal)) => {
            say(&Line::Refused(refusal.why));
            ExitCode::FAILURE
        }
        Err(Ended::Stopped) => ExitCode::from(3),
    }
}
