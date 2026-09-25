//! The whole state, and the one lock everybody reads it through.
//!
//! # A word about the word
//!
//! In this repository a *buffer* is a `wasm32-wasip2` component with a screen.
//! What is here is not that: [`Mind`] is the STATE, on the wire and in memory,
//! and `buffers/brain/` is the screen that draws it. The two are kept apart in
//! every comment, because "the brain's buffer" would otherwise mean either.
//!
//! # Why one lock and a bell, and not a channel per client
//!
//! The same arrangement, and the same reasons, as the transfer queue's ledger.
//! There are several readers arriving at different times — the footer's
//! modeline, the brain's screen in the expanded region, the window that screen
//! opens in, and a person running `ronoctua-brain look` — and a channel per
//! client means the daemon has to know who is listening in order to send. One
//! state with a VERSION answers both halves: a new reader takes a snapshot and
//! is immediately correct, an old one waits on the bell for a version past the
//! one it drew.
//!
//! # Why the whole mind goes out on every change
//!
//! A delta needs both sides to agree on what they last saw, and a client that
//! dropped one would draw a wrong answer for the rest of the session with
//! nothing to notice it by. Here that answer is *when will my machine update
//! itself*, which is exactly the kind of wrongness nobody double-checks. The
//! whole thing is a few short records.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::moment::Moment;
use crate::prefs::{PartOfDay, Prefs};
use crate::pulse::Pulse;
use crate::task::{Hold, RunRecord};

/// Where a task is right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Standing {
    /// Waiting for its moment.
    #[default]
    Idle,
    /// Its moment came; it is waiting for the one that is running, or for the
    /// countdown to end.
    Queued,
    /// It is happening.
    Running,
}

impl Standing {
    /// The word the screen looks up, as `brain-standing-<word>`.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Queued => "queued",
            Self::Running => "running",
        }
    }
}

/// One step of a task, as the screen draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StepState {
    /// The step's id — `brain-step-<id>` on screen.
    pub id: String,
    /// What it really runs. DATA: a command line is not translated.
    pub command: String,
    /// Whether it needs root.
    pub needs_root: bool,
    /// Whether it will run.
    pub on: bool,
    /// Whether the distro ships it on.
    ///
    /// The DECLARED fact, and the only one of the two that is not derived —
    /// "changed" is `on != on_by_default`, which anybody holding both can
    /// work out. It travels because the screen wants to say what a step was
    /// changed FROM, and because the host has to know what flipping one means.
    pub on_by_default: bool,
}

/// One task, as everybody draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TaskState {
    /// The task's id. `brain-task-<id>` names it on screen.
    pub id: String,
    /// Whether it repeats or happens once.
    pub kind: crate::task::Kind,
    /// Where it is.
    pub standing: Standing,
    /// Whether the person left it switched on.
    pub enabled: bool,
    /// Days between runs, as resolved for THIS task.
    ///
    /// Resolved here, and not left in the preferences map, because the screen
    /// has no map type to look one up in — and because a screen that had to
    /// join two lists to draw a row is a screen that draws the wrong row the
    /// first time one of them is filtered.
    pub interval_days: u16,
    /// Which part of the day it prefers.
    pub part_of_day: PartOfDay,
    /// The next moment it will be considered, in epoch seconds. `0` when there
    /// is none — it is switched off, or it only runs when somebody asks.
    pub next: Moment,
    /// What stopped it the last time its moment came, if anything.
    ///
    /// The screen draws it as *"waiting: the RAM is busy"*. A scheduler that
    /// silently does nothing is one nobody trusts.
    pub hold: Option<Hold>,
    /// Why it is not due at all, as a WORD — `off`, `no-schedule`. Empty when
    /// it is due or scheduled.
    pub why_not: String,
    /// The last run, whatever it said.
    pub last: Option<RunRecord>,
    /// The last few runs of THIS task, newest first.
    ///
    /// `last` is the first of them. Both are here for the reason the wire's own
    /// record gives: a row on the list draws one and a page draws all of them,
    /// and deriving either from the other would put a length check in front of
    /// every reader.
    #[serde(default)]
    pub history: Vec<RunRecord>,
    /// What it runs, in order.
    pub steps: Vec<StepState>,
}

/// The warning that something is about to happen.
///
/// # Why the brain sends it ONCE and never a countdown
///
/// Because the footer already beats a one-second clock, and a daemon that
/// pushed a number every second would wake three processes to redraw a digit.
/// What crosses is the DEADLINE; whoever draws computes `deadline - now` on the
/// clock it already has, and reuses the countdown glyph the notification band
/// already draws with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Announcement {
    /// Which task is about to run.
    pub task: String,
    /// How long the whole warning lasts, in seconds — the denominator of the
    /// glyph, so it does not have to be inferred from a first sighting.
    pub total_seconds: u16,
    /// When it starts, in epoch seconds.
    pub deadline: Moment,
    /// Why it is running: `due` when its moment came, `asked` when somebody
    /// pressed a key. A word, so the screen says which of the two it is.
    pub why: String,
}

/// Everything there is to know, all at once.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Mind {
    /// The daemon's clock, in epoch seconds.
    ///
    /// It crosses because the screen has none: a component has no clock
    /// (invariant 11), so *"in three hours"* cannot be worked out on the other
    /// side without this number.
    pub now: Moment,
    /// How busy the machine is, as the gate last read it.
    pub pulse: Pulse,
    /// Every task the brain knows.
    pub tasks: Vec<TaskState>,
    /// What is waiting, in the order it will run.
    pub queue: Vec<String>,
    /// What is running, by id. Empty when nothing is.
    pub running: String,
    /// The warning in flight, if any.
    pub announcement: Option<Announcement>,
    /// The person's preferences, whole — what `Ask::Tune` sends back changed.
    pub prefs: Prefs,
    /// The task whose NEXT run is a rehearsal, if somebody asked for one.
    ///
    /// It lives in the state and not on the runner because the two are
    /// different threads: the ask arrives on a socket thread and the run
    /// happens on the runner's. One name and not a set — at most one thing
    /// runs, so at most one run can be a dry one — and it is cleared when the
    /// run starts, so a rehearsal asked for today cannot quietly make next
    /// week's scheduled run a dry one.
    pub rehearsing: Option<String>,
    /// A task somebody asked to put off, waiting for the runner to write it
    /// down.
    ///
    /// It crosses the threads the same way [`Mind::rehearsing`] does, and for
    /// the same reason: the ask arrives on a socket thread and the journal
    /// belongs to the runner's. It does NOT cross to the screen — by the time
    /// anything is drawn the runner has already turned it into a record and a
    /// new moment, and a field showing a decision in flight would be a field
    /// that is empty every time anybody looks.
    pub postponed: Option<String>,
    /// Everything the brain has done, OLDEST FIRST, as the journal keeps it.
    ///
    /// # Why the mind carries it at all
    ///
    /// Because the whole mind is what crosses the socket, and the home page
    /// shows one list of everything that has happened. The alternative was a
    /// second `Ask` for the journal, which would mean two answers that can
    /// disagree about the same moment — the argument this file's header makes
    /// about deltas, one level up.
    ///
    /// It is a COPY and not the journal itself: the journal belongs to the
    /// runner's thread and this is read by every screen. The ceiling is
    /// [`crate::journal::ACTIVITY`], so the copy is bounded however long the
    /// machine has been running.
    pub journal: Vec<crate::task::RunRecord>,
    /// Something the brain wants said, without anybody having asked.
    ///
    /// It is the brain's line under the minibuffer — the one the countdown
    /// already uses. A watch's whole output is a sentence, and a sentence that
    /// only appeared on a screen somebody had to open would be a watch nobody
    /// hears.
    ///
    /// ONE at a time, like the announcement beside it: the line is one line,
    /// and a queue of notices on it would be a second notification system next
    /// to the one this repository already has (invariant 14).
    pub notice: Option<crate::task::Notice>,
}

impl Mind {
    /// One task's row, by id.
    #[must_use]
    pub fn task(&self, id: &str) -> Option<&TaskState> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// One task's row, to change.
    pub fn task_mut(&mut self, id: &str) -> Option<&mut TaskState> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }
}

/// The state, behind one lock.
#[derive(Debug)]
struct Inner {
    mind: Mind,
    version: u64,
    /// Flipped by `abort` and read by whatever is running.
    stop: Arc<AtomicBool>,
    /// Whether any change EVER left an announcement standing — a witness for
    /// the tests, which otherwise see only the state after a pass and never
    /// one that lived for a few instructions inside it.
    #[cfg(test)]
    ever_announced: bool,
}

/// The mind, shared by the daemon's threads.
#[derive(Debug)]
pub struct Shared {
    state: Mutex<Inner>,
    bell: Condvar,
}

impl Shared {
    /// A fresh one, with the person's preferences already in it.
    #[must_use]
    pub fn of(prefs: Prefs) -> Self {
        Self {
            state: Mutex::new(Inner {
                mind: Mind {
                    prefs,
                    ..Mind::default()
                },
                version: 0,
                stop: Arc::new(AtomicBool::new(false)),
                #[cfg(test)]
                ever_announced: false,
            }),
            bell: Condvar::new(),
        }
    }

    /// The mind as everybody should draw it, and the version it is.
    #[must_use]
    pub fn snapshot(&self) -> (Mind, u64) {
        let state = self.locked();
        (state.mind.clone(), state.version)
    }

    /// Blocks until the mind is past `seen`, or until `patience` runs out.
    ///
    /// The timeout is what lets the runner use the same bell as its tick: it
    /// wakes on a change if there is one and on the clock if there is not, so
    /// there is one waiting place instead of a sleep and a channel.
    #[must_use]
    pub fn wait_past(&self, seen: u64, patience: Duration) -> (Mind, u64) {
        let state = self.locked();
        let (state, _) = self
            .bell
            .wait_timeout_while(state, patience, |s| s.version <= seen)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (state.mind.clone(), state.version)
    }

    /// The version as it stands, without waiting for anything.
    ///
    /// What it is FOR is the runner's loop: a pass writes, so the version it
    /// must then wait past is the one its own writes left behind — see
    /// `Runner::serve`, where reading it before the pass instead spun.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.locked().version
    }

    /// Changes the mind and tells everybody — WHEN THERE IS SOMETHING TO TELL.
    ///
    /// # Why it compares
    ///
    /// Because it did not, and every caller that touched the state rang the
    /// bell whether or not anything moved. `Runner::put_off` runs on every
    /// pass and takes a `postponed` that is almost always `None`; that alone
    /// was a version bump, twice a pass, for a no-op.
    ///
    /// A bump is not free: every `Ask::Watch` subscriber is woken and handed a
    /// whole `Mind`, and on the other end of that socket is the loop that
    /// DRAWS. The buffer's window went unresponsive for seconds at a time —
    /// the compositor's own resize queued behind a flood of minds saying
    /// nothing had happened.
    ///
    /// It is the same trade `buffers/memory` makes with `additional_derives:
    /// [PartialEq]`: the survey arrives on a clock and the screen only moves
    /// when it changed. One clone of a small record, against a wake-up of
    /// every reader.
    pub fn change(&self, what: impl FnOnce(&mut Mind)) {
        let mut state = self.locked();
        let before = state.mind.clone();
        what(&mut state.mind);
        #[cfg(test)]
        {
            state.ever_announced |= state.mind.announcement.is_some();
        }
        if state.mind == before {
            return;
        }
        self.rang(state);
    }

    /// Whether an announcement was ever published, however briefly.
    #[cfg(test)]
    #[must_use]
    pub fn ever_announced(&self) -> bool {
        self.locked().ever_announced
    }

    /// Puts a task in the queue, if it is not already somewhere.
    ///
    /// **THE QUEUE RULE LIVES HERE**, and not in any task: at most one thing
    /// runs, and whatever comes due meanwhile lines up behind it. A feature
    /// added tomorrow inherits that by existing.
    pub fn queue_up(&self, task: &str) {
        let mut state = self.locked();
        let mind = &mut state.mind;
        if mind.running == task || mind.queue.iter().any(|q| q == task) {
            return;
        }
        mind.queue.push(task.to_owned());
        if let Some(row) = mind.task_mut(task) {
            row.standing = Standing::Queued;
        }
        self.rang(state);
    }

    /// Takes a task out of the queue. `true` when it was there.
    pub fn unqueue(&self, task: &str) -> bool {
        let mut state = self.locked();
        let mind = &mut state.mind;
        let Some(at) = mind.queue.iter().position(|q| q == task) else {
            return false;
        };
        mind.queue.remove(at);
        if mind.announcement.as_ref().is_some_and(|a| a.task == task) {
            mind.announcement = None;
        }
        if let Some(row) = mind.task_mut(task) {
            row.standing = Standing::Idle;
        }
        self.rang(state);
        true
    }

    /// The switch whatever is running watches.
    #[must_use]
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.locked().stop)
    }

    /// Asks whatever is running to stop, and clears whatever was queued.
    ///
    /// A NEW switch is put in place for the next run rather than the old one
    /// being reset: a thread on its way out may still read the old one, and a
    /// switch that was pressed must stay pressed for as long as anybody is
    /// looking at it.
    pub fn abort(&self, task: &str) {
        let mut state = self.locked();
        if state.mind.running == task {
            state.stop.store(true, Ordering::Relaxed);
            state.stop = Arc::new(AtomicBool::new(false));
        }
        let mind = &mut state.mind;
        if let Some(at) = mind.queue.iter().position(|q| q == task) {
            mind.queue.remove(at);
        }
        if mind.announcement.as_ref().is_some_and(|a| a.task == task) {
            mind.announcement = None;
        }
        if let Some(row) = mind.task_mut(task)
            && row.standing == Standing::Queued
        {
            row.standing = Standing::Idle;
        }
        self.rang(state);
    }

    /// The lock, with a poisoned one taken anyway.
    ///
    /// A panicking thread must not take the brain down with it: what is on the
    /// other side is a list of records, and a half-written one is still a
    /// record. Refusing to read it would turn one failed run into a daemon that
    /// answers nothing — the reasoning `ronoctua-transfers` already writes out.
    fn locked(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Bumps the version and wakes every reader.
    fn rang(&self, mut state: std::sync::MutexGuard<'_, Inner>) {
        state.version += 1;
        drop(state);
        self.bell.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_row(id: &str) -> TaskState {
        TaskState {
            kind: crate::task::Kind::Routine,
            id: id.to_owned(),
            standing: Standing::Idle,
            enabled: true,
            interval_days: 7,
            part_of_day: PartOfDay::Afternoon,
            next: 0,
            hold: None,
            why_not: String::new(),
            history: Vec::new(),
            last: None,
            steps: Vec::new(),
        }
    }

    fn two_tasks() -> Shared {
        let shared = Shared::of(Prefs::default());
        shared.change(|m| m.tasks = vec![a_row("one"), a_row("two")]);
        shared
    }

    #[test]
    fn the_same_task_cannot_queue_twice() {
        // THE DEFECT THIS CATCHES: a key pressed twice, or a moment that comes
        // due while the same task is already waiting, running the update two
        // times in a row — the second one on a machine the first one just
        // rebuilt.
        let shared = two_tasks();
        shared.queue_up("one");
        shared.queue_up("one");
        let (mind, _) = shared.snapshot();
        assert_eq!(mind.queue, vec!["one".to_owned()]);
        assert_eq!(mind.task("one").map(|t| t.standing), Some(Standing::Queued));
    }

    #[test]
    fn what_is_running_does_not_join_its_own_queue() {
        let shared = two_tasks();
        shared.change(|m| m.running = "one".to_owned());
        shared.queue_up("one");
        let (mind, _) = shared.snapshot();
        assert!(mind.queue.is_empty(), "it joined the queue behind itself");
    }

    #[test]
    fn the_version_only_moves_forward_and_wakes_a_reader() {
        let shared = Arc::new(two_tasks());
        let (_, version) = shared.snapshot();
        let waiter = Arc::clone(&shared);
        let hand = std::thread::spawn(move || waiter.wait_past(version, Duration::from_secs(5)));
        shared.queue_up("two");
        let (mind, after) = hand.join().expect("thread");
        assert!(after > version);
        assert_eq!(mind.queue, vec!["two".to_owned()]);
    }

    #[test]
    fn a_reader_that_waits_for_nothing_comes_back_on_the_clock() {
        // The runner waits on this same bell for its tick. Without the timeout
        // it would sleep until somebody pressed a key — and a scheduler that
        // only wakes when asked is not a scheduler.
        let shared = two_tasks();
        let (_, version) = shared.snapshot();
        let (_, after) = shared.wait_past(version, Duration::from_millis(20));
        assert_eq!(after, version, "it woke up saying something changed");
    }

    #[test]
    fn aborting_replaces_the_switch_so_a_pressed_one_stays_pressed() {
        let shared = two_tasks();
        let held = shared.stop_flag();
        shared.change(|m| m.running = "one".to_owned());
        shared.abort("one");
        assert!(held.load(Ordering::Relaxed), "the switch was not pressed");
        assert!(
            !shared.stop_flag().load(Ordering::Relaxed),
            "the next run was born already aborted"
        );
    }
}
