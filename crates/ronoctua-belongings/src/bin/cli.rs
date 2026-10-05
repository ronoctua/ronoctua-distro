//! `ronoctua-belongings <prepare|apply|abandon|keygen|rekey> [--progress]` —
//! what the transfer queue runs as a child for `Work::Belongings`.
//!
//! Everything it needs is where both sides know to look
//! (`ronoctua_belongings::places`): the settings in `belongings.json`, the
//! passphrase in the `0600` runtime file a door wrote, the decisions beside
//! the plan. Nothing secret is an argument. With `--progress` it prints the
//! queue's lines (`ronoctua_belongings::order::Beat`) on stdout; either way it
//! leaves `status.json` for the door and the brain, and exits `0` when it did
//! what it was asked (a sync waiting for decisions included).

use std::io::Write as _;
use std::process::ExitCode;

use ronoctua_belongings::Why;
use ronoctua_belongings::engine;
use ronoctua_belongings::order::{Beat, Op};
use ronoctua_belongings::places::Places;

/// The usage line, in English (invariant 18: a command line's help).
const USAGE: &str = "usage: ronoctua-belongings <prepare|apply|abandon|keygen|rekey> [--progress]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let progress = args.iter().any(|a| a == "--progress");
    let words: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| *a != "--progress")
        .collect();
    let Some(op) = words
        .first()
        .and_then(|w| Op::of_word(w))
        .filter(|_| words.len() == 1)
    else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let mut out = std::io::stdout().lock();
    let mut beat = |b: Beat| {
        if progress {
            let _ = writeln!(out, "{}", b.line());
            let _ = out.flush();
        }
    };
    let Some(places) = Places::from_env() else {
        eprintln!("ronoctua-belongings: no home, no configuration root or no runtime directory");
        beat(Beat::Failed(Why::Failed));
        return ExitCode::FAILURE;
    };
    // EVERYTHING ELSE IS THE LIBRARY'S (`engine::order`), where a test reaches
    // it: which refusal deletes what, and under which lock.
    match engine::order(op, &places, &mut beat) {
        Ok(conflicts) => {
            beat(Beat::Done { conflicts });
            ExitCode::SUCCESS
        }
        Err(refusal) => {
            eprintln!("ronoctua-belongings {}: {refusal}", op.word());
            beat(Beat::Failed(refusal.why));
            ExitCode::FAILURE
        }
    }
}
