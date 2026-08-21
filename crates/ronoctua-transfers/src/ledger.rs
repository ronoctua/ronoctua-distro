//! The queue itself: who is running, and the switch that stops them.
//!
//! # Why one lock and a bell, and not a channel per client
//!
//! Because there are three readers and they arrive at different times: the
//! footer's modeline, the panel in its expanded region, and — once the wasm
//! buffer exists — a second window. A channel per client means the daemon has
//! to know who is listening in order to send, and a client that connected
//! halfway through would have missed everything before it.
//!
//! One state with a VERSION answers both. A new reader takes a snapshot and is
//! immediately correct; an old one waits on the bell for a version past the one
//! it drew. Nothing is ever missed and nothing has to be replayed.
//!
//! # Why the whole list goes out on every change
//!
//! A delta needs both sides to agree on what they last saw. A client that
//! dropped one would draw a wrong number for the rest of the session with
//! nothing to notice it by — and the number in question is a percentage next to
//! a running copy, which is precisely the kind of wrongness nobody double
//! checks. The list is at most a few dozen short records; sending it whole is
//! cheaper than being subtly wrong.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};

use ronoctua_files::watch::{Control, Flow};

use crate::job::{Engine, Job, Stage, Work};

/// How many FINISHED jobs the list keeps.
///
/// A ceiling and not "forever": the panel is a place to watch things happen,
/// and a session that copies a thousand files would turn it into a log nobody
/// reads. Sixty-four is a few screens of history — enough to answer "did that
/// one work?" hours later, and small enough that the whole list crossing the
/// socket on every beat stays free.
pub const HISTORY: usize = 64;

/// The state, behind one lock.
#[derive(Debug, Default)]
struct State {
    jobs: Vec<Job>,
    /// One per job that is still afoot. It leaves when the job ends: a switch
    /// nobody can press is a switch nobody should be able to find.
    switches: HashMap<u64, Arc<Control>>,
    next: u64,
    version: u64,
}

/// The queue, shared by the daemon's threads.
#[derive(Debug, Default)]
pub struct Shared {
    state: Mutex<State>,
    bell: Condvar,
}

impl Shared {
    /// An empty one.
    #[must_use]
    pub fn fresh_one() -> Self {
        Self::default()
    }

    /// Adds a job and gives back its number and its switch.
    ///
    /// The switch comes back rather than being looked up afterwards, because
    /// between adding and looking up there is a window in which somebody could
    /// have cancelled — and the working thread would then start with a switch
    /// that says run when the answer was already no.
    pub fn take(&self, engine: Engine, work: Work) -> (u64, Arc<Control>) {
        let switch = Arc::new(Control::new());
        let mut state = self.locked();
        state.next += 1;
        let id = state.next;
        state.jobs.push(Job::fresh_one(id, engine, work));
        state.switches.insert(id, Arc::clone(&switch));
        prune(&mut state);
        self.rang(state);
        (id, switch)
    }

    /// Changes one job and tells everybody.
    ///
    /// Nothing happens for an id that is not there — a report arriving after a
    /// job was forgotten is normal, not an error: the thread was already on its
    /// way when the list was swept.
    pub fn touch(&self, id: u64, change: impl FnOnce(&mut Job)) {
        let mut state = self.locked();
        let Some(job) = state.jobs.iter_mut().find(|j| j.id == id) else {
            return;
        };
        change(job);
        let over = !job.afoot();
        // THE SWITCH GOES when the job does. Left behind, `pause 7` would
        // answer "done" on a job that finished an hour ago, and the panel would
        // show a paused row that nothing is holding.
        //
        // AND THE CEILING IS APPLIED HERE TOO, not only on the way in. This is
        // where the finished set GROWS — pruning only in `take` let the list
        // sit one over the ceiling for as long as nothing new was started,
        // which the test caught at sixty-five.
        if over {
            state.switches.remove(&id);
            prune(&mut state);
        }
        self.rang(state);
    }

    /// The switch of a job still afoot.
    #[must_use]
    pub fn switch(&self, id: u64) -> Option<Arc<Control>> {
        self.locked().switches.get(&id).map(Arc::clone)
    }

    /// The list as everybody should draw it, and the version it is.
    ///
    /// **The order is decided here**, once: what is still happening comes
    /// first, oldest at the top so a running row does not jump while somebody
    /// is aiming at it; then what has ended, most recent first. Two clients
    /// sorting for themselves is two lists that disagree about which row `3`
    /// is.
    #[must_use]
    pub fn snapshot(&self) -> (Vec<Job>, u64) {
        let state = self.locked();
        (ordered(&state.jobs), state.version)
    }

    /// Blocks until the list is past `seen`, then gives it back.
    ///
    /// A reader that has never drawn anything passes `0` and returns at once,
    /// which is what makes a client correct from its first frame.
    #[must_use]
    pub fn wait_past(&self, seen: u64) -> (Vec<Job>, u64) {
        let mut state = self.locked();
        while state.version <= seen {
            state = self
                .bell
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        (ordered(&state.jobs), state.version)
    }

    /// Takes one finished job off the list. `false` when it is still afoot —
    /// forgetting a running copy would leave a thread writing to a disk with
    /// nothing on screen saying so.
    pub fn forget(&self, id: u64) -> bool {
        let mut state = self.locked();
        let Some(at) = state.jobs.iter().position(|j| j.id == id && !j.afoot()) else {
            return false;
        };
        state.jobs.remove(at);
        self.rang(state);
        true
    }

    /// Takes every finished job off the list, and says how many went.
    pub fn sweep(&self) -> usize {
        let mut state = self.locked();
        let before = state.jobs.len();
        state.jobs.retain(Job::afoot);
        let gone = before - state.jobs.len();
        if gone > 0 {
            self.rang(state);
        }
        gone
    }

    /// Stops everything, for a daemon on its way out.
    ///
    /// The threads notice at the next bite and unwind, which for a copy means
    /// the half-written file is removed. Exiting without this would leave the
    /// destination littered with pieces nobody asked for.
    pub fn stop_everything(&self) {
        let state = self.locked();
        for switch in state.switches.values() {
            switch.set(Flow::Stop);
        }
    }

    /// The lock, with a poisoned one taken anyway.
    ///
    /// A panicking thread must not take the queue down with it: what is on the
    /// other side is a list of records, and a half-written `Job` is still a
    /// `Job`. Refusing to read it would turn one failed copy into a dead panel.
    fn locked(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Bumps the version and wakes every reader.
    fn rang(&self, mut state: std::sync::MutexGuard<'_, State>) {
        state.version += 1;
        drop(state);
        self.bell.notify_all();
    }
}

/// What is happening first, then what happened, most recent first.
fn ordered(jobs: &[Job]) -> Vec<Job> {
    let mut out: Vec<Job> = jobs.to_vec();
    out.sort_by_key(|j| {
        if j.afoot() {
            (0, j.id)
        } else {
            (1, u64::MAX - j.id)
        }
    });
    out
}

/// Drops the oldest finished jobs past [`HISTORY`].
fn prune(state: &mut State) {
    let finished = state.jobs.iter().filter(|j| !j.afoot()).count();
    if finished <= HISTORY {
        return;
    }
    let mut over = finished - HISTORY;
    state.jobs.retain(|j| {
        if j.afoot() || over == 0 {
            return true;
        }
        over -= 1;
        false
    });
}

/// How far along the whole queue is, in per mille — what the modeline shows.
///
/// # Why an AVERAGE and not a sum
///
/// Because the counters do not share a unit. A copy counts bytes and a delete
/// counts entries, and adding them gives a number in nothing: one file of a
/// gibibyte plus three deleted entries would read as `1073741827` of something.
/// The mean of the per-job percentages is unit-free and is what a person means
/// by "how far along is my machine".
///
/// `None` when nothing afoot knows its total — while everything is still being
/// weighed, an honest segment shows the count and no bar.
#[must_use]
pub fn along(jobs: &[Job]) -> Option<u16> {
    let known: Vec<u16> = jobs
        .iter()
        .filter(|j| j.afoot())
        .filter_map(Job::along)
        .collect();
    if known.is_empty() {
        return None;
    }
    let sum: u32 = known.iter().map(|p| u32::from(*p)).sum();
    u16::try_from(sum / known.len() as u32).ok()
}

/// How many are still happening — the number next to the icon when there is
/// more than one.
#[must_use]
pub fn afoot(jobs: &[Job]) -> usize {
    jobs.iter().filter(|j| j.afoot()).count()
}

/// Whether anything at all failed and has not been swept.
///
/// The modeline needs it: a copy that died two minutes ago leaves nothing
/// running, so a segment that only appears while something is afoot would take
/// the failure off screen before anybody saw it.
#[must_use]
pub fn troubled(jobs: &[Job]) -> bool {
    jobs.iter().any(|j| j.stage == Stage::Failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn some_work() -> Work {
        Work::Measure {
            path: PathBuf::from("/nao-existe"),
        }
    }

    fn ended(shared: &Shared, id: u64, stage: Stage) {
        shared.touch(id, |j| j.stage = stage);
    }

    #[test]
    fn the_switch_leaves_when_the_job_does() {
        // THE DEFECT THIS CATCHES: `pause 7` answering "done" on a job that
        // ended an hour ago, and the panel showing a paused row that nothing is
        // holding.
        let shared = Shared::fresh_one();
        let (id, _) = shared.take(Engine::Local, some_work());
        assert!(shared.switch(id).is_some(), "nasceu sem interruptor");
        ended(&shared, id, Stage::Done);
        assert!(shared.switch(id).is_none(), "o interruptor ficou");
    }

    #[test]
    fn a_running_job_cannot_be_forgotten() {
        // Forgetting one that is still going would leave a thread writing to a
        // disk with nothing on screen saying so.
        let shared = Shared::fresh_one();
        let (id, _) = shared.take(Engine::Local, some_work());
        assert!(!shared.forget(id), "esqueceu uma que ainda corre");
        ended(&shared, id, Stage::Cancelled);
        assert!(shared.forget(id), "nao esqueceu uma que acabou");
    }

    #[test]
    fn what_is_happening_comes_first_and_does_not_jump() {
        // A running row that moves while somebody is aiming at it is a click on
        // the wrong job. Running rows are ordered by id ASCENDING for exactly
        // that reason: a new one goes under, never above.
        let shared = Shared::fresh_one();
        let (first, _) = shared.take(Engine::Local, some_work());
        let (second, _) = shared.take(Engine::Local, some_work());
        ended(&shared, first, Stage::Done);
        let (third, _) = shared.take(Engine::Local, some_work());

        let (jobs, _) = shared.snapshot();
        let order: Vec<u64> = jobs.iter().map(|j| j.id).collect();
        assert_eq!(order, vec![second, third, first], "{order:?}");
    }

    #[test]
    fn the_version_only_moves_forward_and_wakes_a_reader() {
        // The bell is what keeps the modeline off a polling timer. A reader
        // that asked for "anything past 0" has to come back at once, or the
        // first frame of every client would be empty.
        let shared = Arc::new(Shared::fresh_one());
        let (_, version) = shared.snapshot();
        assert_eq!(version, 0, "uma fila vazia ja nasceu mudada");

        let waiter = Arc::clone(&shared);
        let hand = std::thread::spawn(move || waiter.wait_past(0));
        shared.take(Engine::Local, some_work());
        let (jobs, after) = hand.join().unwrap();
        assert!(after > 0 && jobs.len() == 1, "{after} {jobs:?}");
    }

    #[test]
    fn the_average_ignores_what_has_no_total() {
        // THE DEFECT THIS CATCHES: a job still being weighed counting as zero
        // per cent and dragging the modeline down to half while the only real
        // copy is nearly done.
        let shared = Shared::fresh_one();
        let (a, _) = shared.take(Engine::Local, some_work());
        let (_b, _) = shared.take(Engine::Local, some_work());
        shared.touch(a, |j| {
            j.stage = Stage::Running;
            j.total = 100;
            j.done = 90;
        });
        let (jobs, _) = shared.snapshot();
        assert_eq!(
            along(&jobs),
            Some(900),
            "a que nao sabe o total entrou na conta"
        );
        assert_eq!(afoot(&jobs), 2);
    }

    #[test]
    fn the_history_has_a_ceiling_and_running_jobs_are_never_the_ones_dropped() {
        let shared = Shared::fresh_one();
        let (running, _) = shared.take(Engine::Local, some_work());
        for _ in 0..(HISTORY + 10) {
            let (id, _) = shared.take(Engine::Local, some_work());
            ended(&shared, id, Stage::Done);
        }
        let (jobs, _) = shared.snapshot();
        assert!(
            jobs.iter().any(|j| j.id == running),
            "o teto derrubou uma que estava correndo"
        );
        assert_eq!(jobs.iter().filter(|j| !j.afoot()).count(), HISTORY);
    }
}
