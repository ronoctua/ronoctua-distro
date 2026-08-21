//! What the engines that go over the network have in common.
//!
//! # Why they do not share [`crate::local`]'s counter
//!
//! The local engine is told what it just did — *eight mebibytes went past* —
//! and adds up. A download is told where it IS: aria2 answers with
//! `completedLength`, yt-dlp prints the byte count so far, and libtorrent
//! reports `total_done`. Adding those up would count the same bytes over and
//! over, so this one SETS rather than adds, and throttles the same way.

use crate::job::Stage;
use crate::ledger::Shared;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How often a poll asks the engine where it is.
///
/// Three tenths of a second: fast enough that a bar moves, slow enough that a
/// download of a hundred files does not spend its time answering. The local
/// engine's beat is faster because it costs a comparison, not a round trip.
pub(crate) const POLL: Duration = Duration::from_millis(300);

/// How often at most the ledger is written to.
///
/// The same as [`crate::local::BEAT`], and for the same reason: every write
/// wakes every subscriber, and a screen cannot show more than this anyway.
const BEAT: Duration = Duration::from_millis(120);

/// Where a job is, reported to the ledger without flooding it.
pub(crate) struct Progress<'a> {
    shared: &'a Arc<Shared>,
    id: u64,
    done: u64,
    total: u64,
    now: String,
    said: Instant,
    fresh: bool,
}

impl<'a> Progress<'a> {
    pub(crate) fn new(shared: &'a Arc<Shared>, id: u64) -> Self {
        Self {
            shared,
            id,
            done: 0,
            total: 0,
            now: String::new(),
            said: Instant::now(),
            fresh: false,
        }
    }

    /// Where it is now. Cheap, and writes nothing most of the time.
    pub(crate) fn set(&mut self, done: u64, total: u64, now: &str) {
        if self.done == done && self.total == total && self.now == now {
            return;
        }
        self.done = done;
        self.total = total;
        if self.now != now {
            self.now.clear();
            self.now.push_str(now);
        }
        self.fresh = true;
        if self.said.elapsed() >= BEAT {
            self.flush();
        }
    }

    /// Writes what has been held back.
    ///
    /// Called at the end, always: without it the last frame of a download is
    /// whatever the throttle happened to let through, and a job that finished
    /// can show ninety-eight per cent forever.
    pub(crate) fn flush(&mut self) {
        if !self.fresh {
            return;
        }
        let (done, total) = (self.done, self.total);
        let now = self.now.clone();
        self.shared.touch(self.id, |j| {
            j.done = done;
            j.total = total;
            j.now = now;
        });
        self.said = Instant::now();
        self.fresh = false;
    }

    /// Says the job is held, or running again.
    ///
    /// The stage is written by whoever changed it and not guessed from the
    /// counters: a download that moves no byte for a minute is not paused, and
    /// a screen that decided otherwise would be lying about something the
    /// person can act on.
    pub(crate) fn stage(&mut self, stage: Stage) {
        self.shared.touch(self.id, |j| j.stage = stage);
    }
}

/// Where yt-dlp is, read from the ENVIRONMENT.
///
/// # Why not `option_env!`, which is what every other program path here uses
///
/// Because baked in, a store path is a REFERENCE, and this one would be a
/// reference to a package that is deliberately absent from half the
/// installations. yt-dlp lives under `ronoctua.media.enable`, which is
/// `mkDefault full` — so an `option_env!` would drag ffmpeg and yt-dlp into
/// the REDUCED profile through this binary, and `profile-minimal-closes` would
/// fail naming a package nobody asked for. It is the same trap
/// `RONOCTUA_EMACSCLIENT` fell into, and the same fix: the value is set by the
/// unit, inside the `mkIf` that decides whether the package exists at all.
///
/// The fallback is the bare name, which works in a devshell and fails inside a
/// unit with a minimal `PATH` — which is why the unit sets it.
pub(crate) fn yt_dlp() -> String {
    std::env::var("RONOCTUA_YT_DLP")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "yt-dlp".to_owned())
}

/// Whether this installation has yt-dlp at all.
///
/// The reduced profile does not, and a queue that accepted a media job there
/// would show a row that never moves.
pub(crate) fn media_afoot() -> bool {
    std::env::var("RONOCTUA_YT_DLP").is_ok_and(|v| !v.is_empty())
        || std::path::Path::new("/usr/bin/yt-dlp").exists()
}
