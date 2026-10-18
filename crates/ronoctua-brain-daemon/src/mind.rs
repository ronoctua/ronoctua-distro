//! The mind, shared by the daemon's threads: one lock and a bell.
//!
//! The STATE itself — [`Mind`] and every row of it, what crosses the socket —
//! is the library's (`ronoctua_brain::mind`), re-exported here whole, so
//! `crate::mind::…` names both halves. What is here is the daemon's way of
//! holding it, and why.
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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub use ronoctua_brain::mind::*;

use crate::prefs::Prefs;

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

    /// Reads the mind through the lock, without copying it — for a question
    /// asked often (which modules are switched on, per journal line) whose
    /// answer is small.
    pub fn with<T>(&self, read: impl FnOnce(&Mind) -> T) -> T {
        read(&self.locked().mind)
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

    /// Puts a task at the HEAD of the queue — moving it there if it was
    /// already waiting further back.
    ///
    /// # Why a second door into the queue
    ///
    /// Because the runner only ever looks at the head, and the head can be a
    /// task the gate is holding: a nightly sweep waiting for the memory to
    /// calm down. Queued behind it, a relief of the disk would be held by the
    /// same busy machine it exists to rescue. Only a task the gate may not
    /// hold comes in this way (`gated` false in its manifest), so it jumps nothing that was
    /// ready to run.
    pub fn queue_first(&self, task: &str) {
        let mut state = self.locked();
        let mind = &mut state.mind;
        if mind.running == task || mind.queue.first().is_some_and(|q| q == task) {
            return;
        }
        mind.queue.retain(|q| q != task);
        mind.queue.insert(0, task.to_owned());
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
    use crate::prefs::PartOfDay;

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
            calendar: true,
            name_key: String::new(),
            does_key: String::new(),
            why_key: String::new(),
            screen_generation: 0,
            actions: Vec::new(),
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
    fn a_task_the_gate_may_not_hold_goes_to_the_head_of_the_queue() {
        // THE DEFECT THIS CATCHES: the runner looks only at the head, so a
        // relief appended behind a sweep the gate is holding would wait for
        // the same busy machine it exists to rescue.
        let shared = two_tasks();
        shared.queue_up("one");
        shared.queue_up("two");
        shared.queue_first("two");
        assert_eq!(
            shared.snapshot().0.queue,
            vec!["two".to_owned(), "one".to_owned()]
        );
        // AND ONCE AT THE HEAD, asking again changes nothing and rings nobody.
        let (_, before) = shared.snapshot();
        shared.queue_first("two");
        let (mind, after) = shared.snapshot();
        assert_eq!(mind.queue, vec!["two".to_owned(), "one".to_owned()]);
        assert_eq!(before, after, "a no-op woke every reader");
        // NOR DOES ONE THAT IS RUNNING queue behind itself.
        shared.change(|m| {
            m.queue.clear();
            m.running = "two".to_owned();
        });
        shared.queue_first("two");
        assert!(shared.snapshot().0.queue.is_empty());
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
