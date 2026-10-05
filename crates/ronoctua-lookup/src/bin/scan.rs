//! `ronoctua-scan <file> <sha256>` — one file sent to VirusTotal and its
//! analysis waited for: the child the transfer queue runs for `Work::Scan`.
//!
//! # Why a child
//!
//! The queue's daemon carries no TLS (`transfers-unit-is-narrow`), and an
//! upload of hundreds of megabytes is the queue's kind of work: progress, a
//! cancel, a line in the list. So the queue starts this, reads its beats
//! (`ronoctua_lookup::virustotal::Beat`, one per line on standard output) and
//! kills it when told.
//!
//! # What it does, in order
//!
//! 1. The person's key (`player-keys/virustotal`), or it ends at once.
//! 2. The file's SHA-256 again: the question the person answered was about
//!    THAT file, and a file that changed since is not sent.
//! 3. The hash is asked once more — somebody may have sent the same file in
//!    the meantime, and then nothing is uploaded at all.
//! 4. The file, streamed; then the analysis, asked every fifteen seconds
//!    while it is pending and never past half an hour.
//!
//! Every state is written into `scans.json` as it is reached — the sending
//! one every [`KEPT_EVERY`] percent — so the security centre's row follows
//! the file without asking this process anything. A cancel is the queue's to
//! write ([`virustotal::cancelled`]): it kills this group, and a process
//! killed writes nothing.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use ronoctua_lookup::internet::Internet;
use ronoctua_lookup::keys::{self, Key};
use ronoctua_lookup::virustotal::{
    self, Analysis, Beat, Checked, POLL, Report, Scan, State, WAIT_MOST,
};
use ronoctua_lookup::{Refusal, Why};

fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// One beat, on its own line, flushed at once: the queue reads them as they
/// come.
fn say(beat: &Beat) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", beat.line());
    let _ = out.flush();
}

/// Keeps the file's state, saying (not failing) when it cannot.
fn keep(path: &str, sha256: &str, state: State) {
    let scan = Scan {
        sha256: sha256.to_owned(),
        path: path.to_owned(),
        at_s: now_s(),
        state,
    };
    if let Err(e) = virustotal::keep(&virustotal::scans(), scan) {
        eprintln!("<4>scans.json could not be written: {e}");
    }
}

/// A beat of progress at most once per percent, so a file of 650 MB is a
/// hundred lines and not ten thousand.
fn paced(make: fn(u64, u64) -> Beat) -> impl FnMut(u64, u64) -> bool {
    let mut last = u64::MAX;
    move |done, of| {
        let percent = done.saturating_mul(100) / of.max(1);
        if percent != last {
            last = percent;
            say(&make(done, of));
        }
        true
    }
}

/// How often, in percent, the sending state is written into `scans.json`:
/// ten writes for a whole file, and the page's row moves with them.
const KEPT_EVERY: u64 = 10;

fn run(file: &std::path::Path, shown: &str, asked_sha: &str) -> Result<virustotal::Verdict, Why> {
    let key = keys::needed(Key::VirusTotal).map_err(|r: Refusal| r.why)?;
    let sha = virustotal::sha256_of(file, &mut paced(|done, of| Beat::Hashing { done, of }))
        .map_err(|_| Why::Unreadable)?;
    // THE FILE THE PERSON SAID YES ABOUT, and no other.
    if sha != asked_sha {
        return Err(Why::Unreadable);
    }
    if let Checked::Known(report) = virustotal::lookup(&Internet, &key, &sha).map_err(|r| r.why)? {
        let verdict = report.stats.verdict();
        keep(shown, &sha, State::Checked { report });
        return Ok(verdict);
    }
    keep(shown, &sha, State::Sending { sent: 0, of: 0 });
    let mut beat = paced(|done, of| Beat::Sending { done, of });
    let mut kept = 0;
    let analysis = virustotal::send_file(&Internet, &key, file, &mut |done, of| {
        // THE ROW'S STATE, coarsely: a write per tenth, not per beat.
        let step = virustotal::sending_step(done, of, KEPT_EVERY);
        if step > kept {
            kept = step;
            keep(shown, &sha, State::Sending { sent: done, of });
        }
        beat(done, of)
    })
    .map_err(|r| r.why)?;
    keep(shown, &sha, State::Pending);
    let began = Instant::now();
    loop {
        say(&Beat::Waiting);
        // THE PACE WAITS FIFTEEN SECONDS between two requests to the host, so
        // this loop asks no faster than the free key allows.
        let request = virustotal::analysis_request(&key, &analysis).map_err(|r| r.why)?;
        let body = ronoctua_lookup::ask::answered(&Internet, &request).map_err(|r| r.why)?;
        if let Analysis::Done(stats) = virustotal::read_analysis(&body).map_err(|r| r.why)? {
            keep(
                shown,
                &sha,
                State::Checked {
                    report: Report {
                        stats,
                        analysed_s: Some(now_s()),
                    },
                },
            );
            return Ok(stats.verdict());
        }
        if began.elapsed() > WAIT_MOST {
            // STILL PENDING, and said so: the row keeps "sent, waiting", and a
            // check by hash later finds the analysis once it is done.
            return Err(Why::Unreachable);
        }
        std::thread::sleep(POLL.min(std::time::Duration::from_secs(1)));
    }
}

fn main() {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let [file, sha] = words.as_slice() else {
        eprintln!("usage: ronoctua-scan <file> <sha256>");
        std::process::exit(2);
    };
    if !virustotal::is_sha256(sha) {
        eprintln!("`{sha}` is not a SHA-256");
        std::process::exit(2);
    }
    let path = PathBuf::from(file);
    match run(&path, file, sha) {
        Ok(verdict) => say(&Beat::Done(verdict)),
        Err(why) => {
            // THE ROW SAYS WHY — except a pending analysis, which stays
            // pending.
            let pending = virustotal::scans()
                .parsed()
                .and_then(|s| s.of(sha).map(|x| x.state == State::Pending))
                .unwrap_or(false);
            if !(pending && why == Why::Unreachable) {
                keep(file, sha, State::Failed { why });
            }
            say(&Beat::Failed(why));
            std::process::exit(1);
        }
    }
}
