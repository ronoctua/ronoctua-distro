//! What the engines that go over the network have in common.
//!
//! # Why they do not share [`crate::local`]'s counter
//!
//! The local engine is told what it just did — *eight mebibytes went past* —
//! and adds up. A download is told where it IS: aria2 answers with
//! `completedLength`, yt-dlp prints the byte count so far, and libtorrent
//! reports `total_done`. Adding those up would count the same bytes over and
//! over, so this one SETS rather than adds, and throttles the same way.
//!
//! # Why none of them writes `Paused` or `Running` while it works
//!
//! Because the row already says it: the socket's pause and resume write the
//! switch and the row together ([`Shared::pause`]), and the hand-off to the
//! work says only that the job is [`Shared::under_way`] — the switch decides
//! the word. An engine that wrote the stage again after acting on the switch
//! wrote what the switch said a round trip AGO: a resume that landed while
//! aria2 was being asked to pause had its `Running` written over with
//! `Paused`, and the row said held over a switch at run until the loop's next
//! look unpaused aria2 and wrote `Running` — a lie of one poll (`POLL`),
//! not a lasting one, but about the key the person had just pressed. And the
//! stage is never guessed from the counters: a download that moves no byte
//! for a minute is not paused.

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
}
