//! The one thread that decides, and the gate everything passes through.
//!
//! # Why at most one thing runs, and why that lives HERE
//!
//! Because the machine is one machine. Two of the brain's tasks at once is two
//! things competing for the disk and the processor at the moment the brain
//! chose precisely because nothing else was — and the second one would be
//! running under conditions the gate never approved.
//!
//! It is a property of the RUNNER and not of any task: there is one worker
//! thread, so overlap is impossible by construction rather than by everybody
//! remembering. A feature added tomorrow inherits the queue by existing.
//!
//! # Why the pass never sleeps
//!
//! [`Runner::pump`] does one pass and comes back: it never waits out a
//! countdown and never blocks on a clock. Waiting happens in ONE place — the
//! bell in [`crate::mind::Shared::wait_past`], which wakes on a change or on
//! the tick, whichever comes first. That is what makes a key press felt at once
//! (the bell rings) without the tick becoming a spin, and it is what lets a
//! test drive a whole day by moving a fake clock and calling `pump`.
//!
//! The one thing that does block is the run itself, and it blocks this thread
//! only. Nothing that draws is on the other end: the socket's clients each have
//! a thread of their own, and every one of them reads the state through the
//! lock.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::journal::Journal;
use crate::mind::{Announcement, Shared, Standing, StepState, TaskState};
use crate::moment::{Clock, Moment};
use crate::prefs::{Gate, Prefs, Where};
use crate::pulse::{Meter, Pulse};
use crate::registry::Registry;
use crate::task::{Due, Hold, Report, RunRecord, Verdict};

/// How often the runner looks, when nothing rings the bell.
///
/// Half a minute. The finest thing this daemon decides is a part of the DAY, so
/// a half-minute tick is already three orders of magnitude finer than anything
/// it schedules — and the pulse it publishes is the same one the gate judged
/// by, so a screen showing a fresher number would be showing one the decision
/// never used. A one-second tick would wake three processes sixty times a
/// minute to redraw an identical line.
pub const TICK: Duration = Duration::from_secs(30);

/// How often a run in flight is looked at.
///
/// Two seconds. It is a `stat` of one file — no process spawned, no journal
/// parsed — and it is what decides how quickly `Esc` on the screen becomes a
/// unit that stops.
pub const BEAT: Duration = Duration::from_secs(2);

/// Where the unit writes what it did.
///
/// `/run` and not the person's state directory, because the unit is a SYSTEM
/// one: a system unit has no `%h` and cannot be told where somebody's home is.
/// `RuntimeDirectory=` in the unit creates it, and the brain only ever reads —
/// which is why the previous report is MARKED rather than deleted
/// ([`crate::task::Report::mark`]).
///
/// The environment may override it, for the same reason `RONOCTUA_YT_DLP`
/// exists: what sets it is the flake, inside the block that decides whether any
/// of this is installed at all.
pub const REPORTS: &str = "/run/ronoctua-brain";

/// The general gate: can the machine afford anything at all right now.
///
/// # A reading that is absent is SKIPPED, and the absence is on screen
///
/// A kernel with no `/proc/pressure/cpu` reports nothing, and there are three
/// things one could do about it. Refusing would be a machine that never updates
/// itself and never says why. Reading it as zero would be a gate with a hole in
/// it that looks exactly like a gate. Skipping the clause and PUBLISHING the
/// pulse is the third: the screen draws a dash where a number should be, and
/// the person can see which half of the gate is watching.
///
/// The order is the order of expense, cheapest true thing first: the battery is
/// a state of the machine, and memory and processor are measurements.
#[must_use]
pub fn judge(pulse: &Pulse, gate: &Gate) -> Verdict {
    if gate.abort_on_battery && pulse.on_battery == Some(true) {
        return Verdict::Hold(Hold::Battery);
    }
    if pulse.ram_used_pct.is_some_and(|p| p > gate.ram_ceiling_pct) {
        return Verdict::Hold(Hold::Ram);
    }
    if pulse
        .cpu_stall
        .is_some_and(|s| s > gate.cpu_stall_per_mille)
    {
        return Verdict::Hold(Hold::Cpu);
    }
    Verdict::Go
}

/// The brain's one worker.
pub struct Runner {
    shared: Arc<Shared>,
    tasks: Registry,
    meter: Box<dyn Meter>,
    clock: Box<dyn Clock>,
    store: Where,
    journal: Journal,
    reports: std::path::PathBuf,
}

impl Runner {
    /// A runner over a mind, with everything it depends on handed in.
    ///
    /// Every seam is a parameter — the clock, the meter, the two directories —
    /// for the reason the rest of this repository gives: a test that had to
    /// wait for a Tuesday, or make the machine busy, is a test nobody runs.
    #[must_use]
    pub fn new(
        shared: Arc<Shared>,
        tasks: Registry,
        meter: Box<dyn Meter>,
        clock: Box<dyn Clock>,
        store: Where,
    ) -> Self {
        let journal = store
            .state
            .as_deref()
            .map_or_else(Journal::fresh, Journal::read_from);
        let reports = std::env::var_os("RONOCTUA_BRAIN_REPORTS").map_or_else(
            || std::path::PathBuf::from(REPORTS),
            std::path::PathBuf::from,
        );
        Self {
            shared,
            tasks,
            meter,
            clock,
            store,
            journal,
            reports,
        }
    }

    /// Where the report of a task is written and read.
    #[must_use]
    pub fn report_path(&self, task: &str) -> std::path::PathBuf {
        self.reports.join(format!("{task}.json"))
    }

    /// Points the reports somewhere else — what a test does.
    #[must_use]
    pub fn reporting_in(mut self, dir: std::path::PathBuf) -> Self {
        self.reports = dir;
        self
    }

    /// One pass: look, decide, and at most start one thing.
    ///
    /// It comes back rather than waiting, always. See this module's header.
    pub fn pump(&mut self) {
        let now = self.clock.now();
        let pulse = self.meter.read(now);
        // BEFORE ANYTHING IS ENROLLED. A postponement that were written down
        // after the enrolment would put the task straight back in the queue it
        // was just taken out of, and the person would answer the same question
        // every half minute until the window closed.
        self.put_off(now);
        let prefs = self.publish(now, pulse);
        self.enrol(now, &prefs);
        self.maybe_run(now, &pulse, &prefs);
    }

    /// Writes down a postponement somebody asked for on the socket.
    ///
    /// It is an EVENT and not a hold: a person said not now, and the next
    /// moment moves because of it. A hold — the machine being busy — is
    /// published on the row and never written down, which is what keeps a busy
    /// Tuesday from pushing a weekly update a week away.
    fn put_off(&mut self, now: Moment) {
        let mut task = None;
        self.shared.change(|m| task = m.postponed.take());
        let Some(task) = task else {
            return;
        };
        tracing::info!(task = %task, "postponed");
        let record = RunRecord::of(
            &task,
            now,
            now,
            crate::task::Outcome::Skipped {
                why: "postponed".to_owned(),
            },
        );
        self.journal.put(record.clone());
        if let Some(dir) = self.store.state.as_deref()
            && let Err(e) = self.journal.write_to(dir)
        {
            tracing::error!(error = %e, "could not write the brain's journal");
        }
        // AND IT LEAVES THE QUEUE HERE TOO. The socket already took it out,
        // and doing it again is not belt and braces: what makes a postponement
        // a postponement is the record plus the empty queue, and the two
        // belong in one place — otherwise a caller that set the flag without
        // unqueueing would have the task run in the very next pass, which is
        // the defect this whole path exists to close.
        self.shared.unqueue(&task);
        self.shared.change(|m| {
            if let Some(row) = m.task_mut(&task) {
                row.last = Some(record.clone());
                row.hold = None;
            }
        });
    }

    /// Redraws every row from what is true right now, and gives back the
    /// preferences it used.
    ///
    /// One place, and not one per event: a row rebuilt at each site that
    /// changes something is a row somebody forgets to rebuild.
    fn publish(&self, now: Moment, pulse: Pulse) -> Prefs {
        // The preferences are read ONCE and every row is built from that one
        // reading. Reading them per task would let a `Tune` arriving in the
        // middle produce a screen where two rows disagree about what the
        // person chose.
        let prefs = self.shared.snapshot().0.prefs;
        let rows: Vec<(&'static str, Due, &'static [crate::task::Step])> = self
            .tasks
            .all()
            .map(|t| {
                let due = t.due(now, &prefs, self.journal.last(t.id()));
                (t.id(), due, t.steps())
            })
            .collect();

        self.shared.change(|mind| {
            mind.now = now;
            mind.pulse = pulse;
            let running = mind.running.clone();
            let queued: Vec<String> = mind.queue.clone();
            let old: Vec<TaskState> = std::mem::take(&mut mind.tasks);
            mind.tasks = rows
                .iter()
                .map(|(id, due, steps)| {
                    let mine = prefs.of(id);
                    let was = old.iter().find(|t| t.id == *id);
                    let standing = if running == *id {
                        Standing::Running
                    } else if queued.iter().any(|q| q == *id) {
                        Standing::Queued
                    } else {
                        Standing::Idle
                    };
                    // WHAT IS WAITING BEHIND SOMETHING ELSE SAYS SO. Without
                    // it a queued task shows the hold of the last time the gate
                    // looked at it, which is a reason that has nothing to do
                    // with why it is waiting now.
                    let hold = if standing == Standing::Queued && !running.is_empty() {
                        Some(Hold::Busy)
                    } else {
                        was.and_then(|t| t.hold)
                    };
                    TaskState {
                        id: (*id).to_owned(),
                        standing,
                        enabled: mine.enabled,
                        interval_days: mine.interval_days,
                        part_of_day: mine.part_of_day,
                        next: match due {
                            Due::At(at) => *at,
                            // NOW and NOT-AT-ALL are both "no moment to draw",
                            // and they are told apart by `why_not` — a date on
                            // screen for something that is already overdue
                            // would be a date in the past.
                            Due::Now | Due::Not { .. } => 0,
                        },
                        hold,
                        why_not: match due {
                            Due::Not { why } => (*why).to_owned(),
                            _ => String::new(),
                        },
                        last: self.journal.latest(id).cloned(),
                        steps: steps
                            .iter()
                            .map(|s| StepState {
                                id: s.id.to_owned(),
                                command: s.command.to_owned(),
                                needs_root: s.needs_root,
                                on: !mine.steps_off.contains(s.id),
                                changed: mine.steps_off.contains(s.id) == s.on_by_default,
                            })
                            .collect(),
                    }
                })
                .collect();
        });
        prefs
    }

    /// Puts into the queue whatever has come due.
    fn enrol(&self, now: Moment, prefs: &Prefs) {
        for task in self.tasks.all() {
            let last = self.journal.last(task.id());
            if matches!(task.due(now, prefs, last), Due::Now) {
                self.shared.queue_up(task.id());
            }
        }
    }

    /// Starts the head of the queue, if everything says it may.
    fn maybe_run(&mut self, now: Moment, pulse: &Pulse, prefs: &Prefs) {
        let (mind, _) = self.shared.snapshot();
        if !mind.running.is_empty() {
            return;
        }
        let Some(id) = mind.queue.first().cloned() else {
            // NOTHING WAITING, NOTHING ANNOUNCED. An announcement outliving its
            // queue would be a countdown on the footer for something that was
            // cancelled.
            if mind.announcement.is_some() {
                self.shared.change(|m| m.announcement = None);
            }
            return;
        };
        let Some(task) = self.tasks.find(&id) else {
            // A queued id nothing answers to: the person's file names a task
            // that no longer exists, or a rename went half way.
            tracing::warn!(task = %id, "a queued task that does not exist; dropping it");
            self.shared.unqueue(&id);
            return;
        };

        // THE GENERAL GATE FIRST, then what only the task knows. A hold is
        // written down and published — never swallowed: a scheduler that
        // quietly does nothing is one nobody trusts.
        let verdict = match judge(pulse, &prefs.gate) {
            Verdict::Go => task.needs(pulse, prefs),
            held => held,
        };
        if let Verdict::Hold(hold) = verdict {
            self.shared.change(|m| {
                if let Some(row) = m.task_mut(&id) {
                    row.hold = Some(hold);
                }
                // AND THE WARNING GOES. A countdown that keeps running while
                // the machine is refusing would end at zero with nothing
                // happening.
                m.announcement = None;
            });
            return;
        }

        // The warning, once. `deadline` and not a ticking number: whoever draws
        // has a one-second clock already, and a daemon that pushed a digit
        // every second would wake three processes to redraw one.
        let seconds = task.countdown(prefs);
        let deadline = match &mind.announcement {
            Some(a) if a.task == id => a.deadline,
            _ => {
                let deadline = now + i64::from(seconds);
                let why = if matches!(task.due(now, prefs, self.journal.last(&id)), Due::Now) {
                    "due"
                } else {
                    "asked"
                };
                self.shared.change(|m| {
                    m.announcement = Some(Announcement {
                        task: id.clone(),
                        total_seconds: seconds,
                        deadline,
                        why: why.to_owned(),
                    });
                    if let Some(row) = m.task_mut(&id) {
                        row.hold = None;
                    }
                });
                deadline
            }
        };
        if now < deadline {
            return;
        }
        self.perform(&id, now);
    }

    /// Runs one task, and writes down what happened.
    ///
    /// This is the one place in the daemon that blocks, and it blocks the
    /// runner's own thread. Every client of the socket has a thread of its own
    /// and reads through the lock, so a rebuild that takes an hour is an hour
    /// in which the screens keep answering.
    fn perform(&mut self, id: &str, began: Moment) {
        let Some(task) = self.tasks.find(id) else {
            return;
        };
        // THE FLAG IS IN THE STATE, because whoever asked for a rehearsal is
        // on a socket thread and this is the runner's. Taken out as it is read:
        // a rehearsal asked for today must not make next week's scheduled run a
        // dry one.
        let mut rehearsal = false;
        self.shared.change(|m| {
            rehearsal = m.rehearsing.as_deref() == Some(id);
            if rehearsal {
                m.rehearsing = None;
            }
        });
        let patience = Duration::from_secs(
            u64::from(self.shared.snapshot().0.prefs.of(id).runaway_minutes) * 60,
        );
        let mut report = Report::at(
            self.report_path(id),
            patience,
            BEAT,
            self.shared.stop_flag(),
        );
        if rehearsal {
            report = report.rehearsing();
        }

        self.shared.change(|m| {
            m.running = id.to_owned();
            if let Some(at) = m.queue.iter().position(|q| q == id) {
                m.queue.remove(at);
            }
            m.announcement = None;
            if let Some(row) = m.task_mut(id) {
                row.standing = Standing::Running;
                row.hold = None;
            }
        });

        tracing::info!(task = id, rehearsal, "starting");
        let outcome = task.run(&report);
        let ended = self.clock.now();
        tracing::info!(task = id, ?outcome, "finished");

        // A REHEARSAL IS NOT A RUN. Writing it down would push the next real
        // one a whole cadence away — the person would have "updated" the
        // machine by asking what an update would do.
        let record = RunRecord::of(id, began, ended, outcome);
        if !rehearsal {
            self.journal.put(record.clone());
            if let Some(dir) = self.store.state.as_deref()
                && let Err(e) = self.journal.write_to(dir)
            {
                // Not fatal: a read-only home is not a reason to stop deciding.
                // It IS worth a line, because what is lost is the memory of
                // when this last ran — and the next start would run it again.
                tracing::error!(error = %e, "could not write the brain's journal");
            }
        }

        self.shared.change(|m| {
            m.running.clear();
            if let Some(row) = m.task_mut(id) {
                row.standing = Standing::Idle;
                row.last = Some(record.clone());
            }
        });
    }

    /// Pumps forever, waiting on the bell between passes.
    ///
    /// It comes back when `stop` is set — which is what the daemon does when it
    /// is on its way out.
    pub fn serve(&mut self, stop: &AtomicBool) {
        while !stop.load(Ordering::Relaxed) {
            self.pump();
            // ONE WAITING PLACE. The bell wakes on a change and the timeout
            // wakes on the clock, so a key press is felt at once and a machine
            // nobody touches still decides.
            //
            // AND IT WAITS PAST ITS OWN WRITES. This read the version from
            // BEFORE the pass, and a pass writes — so `wait_past` was handed a
            // number the pump had already gone past, returned immediately, and
            // pumped again. A spin, at whatever rate the CPU allowed.
            //
            // Nothing crashed and nothing was logged. What it looked like was
            // the brain's BUFFER: every turn of the spin woke every `Watch`
            // subscriber with a whole `Mind`, and the loop that draws spent its
            // time on those instead of on the compositor. The window stopped
            // resizing when the minibuffer opened, `Enter` took seconds, and
            // closing it took seconds — none of which is a slow buffer, and all
            // of which is a starved event loop.
            let mine = self.shared.version();
            let _ = self.shared.wait_past(mine, TICK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moment::{Fixed, Offset};
    use crate::prefs::{PartOfDay, TaskPrefs};
    use crate::pulse::Fake as FakeMeter;
    use crate::task::{Outcome, Step, Task};
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    /// A clock the test moves by hand.
    #[derive(Debug, Default)]
    struct Hand(Arc<Mutex<Moment>>);

    impl Clock for Hand {
        fn now(&self) -> Moment {
            *self.0.lock().expect("the clock")
        }
    }

    /// A task that writes down when it began and ended, and takes a while.
    struct Slow {
        id: &'static str,
        log: Arc<Mutex<Vec<(&'static str, char)>>>,
        takes: Duration,
    }

    impl Task for Slow {
        fn id(&self) -> &'static str {
            self.id
        }
        fn due(&self, _now: Moment, _prefs: &Prefs, _last: Option<Moment>) -> Due {
            // Never on its own: these tests ask by hand, so the queue is the
            // only thing under test.
            Due::Not { why: "test" }
        }
        fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
            Verdict::Go
        }
        fn countdown(&self, _prefs: &Prefs) -> u16 {
            0
        }
        fn steps(&self) -> &'static [Step] {
            &[]
        }
        fn run(&self, _report: &Report) -> Outcome {
            self.log.lock().expect("log").push((self.id, '('));
            std::thread::sleep(self.takes);
            self.log.lock().expect("log").push((self.id, ')'));
            Outcome::Done(Vec::new())
        }
    }

    /// A task that is due until it has a record, and refuses to run.
    ///
    /// The pair with `Always`: this one READS the journal the way a real task
    /// does, which is what makes a postponement provable — `Always` would be
    /// due again on the very next pass whatever anybody wrote down.
    struct Once;

    impl Task for Once {
        fn id(&self) -> &'static str {
            "once"
        }
        fn due(&self, _now: Moment, _prefs: &Prefs, last: Option<Moment>) -> Due {
            if last.is_some() {
                Due::Not { why: "already" }
            } else {
                Due::Now
            }
        }
        fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
            Verdict::Go
        }
        fn countdown(&self, _prefs: &Prefs) -> u16 {
            0
        }
        fn run(&self, _report: &Report) -> Outcome {
            panic!("it ran after being put off");
        }
    }

    /// A task that is always due — for the gate's test.
    struct Always;

    impl Task for Always {
        fn id(&self) -> &'static str {
            "always"
        }
        fn due(&self, _now: Moment, _prefs: &Prefs, _last: Option<Moment>) -> Due {
            Due::Now
        }
        fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
            Verdict::Go
        }
        fn countdown(&self, _prefs: &Prefs) -> u16 {
            0
        }
        fn run(&self, _report: &Report) -> Outcome {
            panic!("it ran with the machine busy");
        }
    }

    fn nowhere() -> Where {
        Where::default()
    }

    fn a_runner(tasks: Vec<Box<dyn Task>>, pulse: Pulse, at: Moment) -> (Arc<Shared>, Runner) {
        let shared = Arc::new(Shared::of(Prefs::default()));
        let runner = Runner::new(
            Arc::clone(&shared),
            Registry::of(tasks).expect("the registry"),
            Box::new(FakeMeter(pulse)),
            Box::new(Hand(Arc::new(Mutex::new(at)))),
            nowhere(),
        );
        (shared, runner)
    }

    /// A task that counts how many times the pass looked at it.
    struct Counted(Arc<AtomicUsize>);

    impl Task for Counted {
        fn id(&self) -> &'static str {
            "counted"
        }
        fn due(&self, _: Moment, _: &Prefs, _: Option<Moment>) -> Due {
            self.0.fetch_add(1, Ordering::Relaxed);
            Due::Not { why: "never" }
        }
        fn needs(&self, _: &Pulse, _: &Prefs) -> Verdict {
            Verdict::Go
        }
        fn countdown(&self, _: &Prefs) -> u16 {
            0
        }
        fn run(&self, _: &Report) -> Outcome {
            Outcome::Skipped {
                why: "never".to_owned(),
            }
        }
    }

    #[test]
    fn a_quiet_machine_pumps_once_and_then_waits() {
        // THE DEFECT THIS CATCHES, and it was found from the SCREEN rather
        // than from here: the brain's buffer had seconds of lag, its window
        // stopped resizing when the minibuffer opened, and closing it took
        // seconds.
        //
        // None of that was the buffer. `serve` waited past the version it read
        // BEFORE the pass — and a pass writes, so the wait was already
        // satisfied when it began and came back at once. It span at whatever
        // rate the processor allowed, and every turn handed a whole `Mind` to
        // every `Ask::Watch` subscriber. On the other end of that socket is the
        // loop that draws, and it spent its time on those instead of on the
        // compositor.
        //
        // Nothing crashed, nothing was logged, and no test noticed: every other
        // test here calls `pump` directly, which is correct on its own. What was
        // wrong was the WAITING, and only `serve` waits.
        let counted = Arc::new(AtomicUsize::new(0));
        let (shared, mut runner) = a_runner(
            vec![Box::new(Counted(Arc::clone(&counted)))],
            Pulse::default(),
            1000,
        );
        let stop = Arc::new(AtomicBool::new(false));
        let mine = Arc::clone(&stop);
        let hand = std::thread::spawn(move || runner.serve(&mine));

        // Long enough that a spin would be thousands of passes, and short
        // enough that `TICK` (thirty seconds) cannot have fired.
        std::thread::sleep(Duration::from_millis(300));
        stop.store(true, Ordering::Relaxed);
        // The thread is inside `wait_past` until the bell or the timeout; ring
        // it so the stop is seen without waiting out `TICK`.
        shared.change(|m| m.now = 2000);
        hand.join().expect("the brain's pump");

        let passes = counted.load(Ordering::Relaxed);
        assert!(
            passes <= 4,
            "the brain spun: {passes} passes in 300 ms. \
             `serve` is waiting past a version it has itself already gone beyond."
        );
    }

    #[test]
    fn the_gate_refuses_by_name_and_nothing_runs() {
        // THE DEFECT THIS CATCHES: a rebuild starting on a machine that is
        // already at its knees — and, just as bad, a refusal nobody can see.
        // The row has to come back saying WHICH threshold stopped it.
        let busy = Pulse {
            ram_used_pct: Some(94),
            ..Pulse::default()
        };
        let (shared, mut runner) = a_runner(vec![Box::new(Always)], busy, 1000);
        runner.pump();
        let (mind, _) = shared.snapshot();
        assert_eq!(mind.running, "", "it ran all the same");
        assert_eq!(mind.task("always").and_then(|t| t.hold), Some(Hold::Ram));
        assert_eq!(mind.queue, vec!["always".to_owned()], "it left the queue");
        // And the pulse is published, so the screen can show the number that
        // decided.
        assert_eq!(mind.pulse.ram_used_pct, Some(94));
    }

    #[test]
    fn a_reading_the_machine_does_not_have_does_not_refuse_and_does_not_pass_silently() {
        // No processor reading at all: the clause is skipped, and the absence
        // is what the screen draws. A `None` treated as zero would be a gate
        // with a hole in it that looks exactly like a gate.
        let gate = Gate::default();
        assert_eq!(judge(&Pulse::default(), &gate), Verdict::Go);
        let stalled = Pulse {
            cpu_stall: Some(gate.cpu_stall_per_mille + 1),
            ..Pulse::default()
        };
        assert_eq!(judge(&stalled, &gate), Verdict::Hold(Hold::Cpu));
        let flat = Pulse {
            on_battery: Some(true),
            ..Pulse::default()
        };
        assert_eq!(judge(&flat, &gate), Verdict::Hold(Hold::Battery));
        // A machine with NO battery is not on battery.
        assert_eq!(judge(&Pulse::default(), &gate), Verdict::Go);
    }

    #[test]
    fn two_tasks_do_not_overlap_and_the_second_waits_its_turn() {
        // THE QUEUE, proved the only way it can be: with a real thread and two
        // tasks that record when they started and stopped. One pass runs at
        // most one thing, and the second task's brackets have to open after the
        // first one's closed.
        let log = Arc::new(Mutex::new(Vec::new()));
        let tasks: Vec<Box<dyn Task>> = vec![
            Box::new(Slow {
                id: "one",
                log: Arc::clone(&log),
                takes: Duration::from_millis(40),
            }),
            Box::new(Slow {
                id: "two",
                log: Arc::clone(&log),
                takes: Duration::from_millis(40),
            }),
        ];
        let (shared, mut runner) = a_runner(tasks, Pulse::default(), 1000);
        shared.queue_up("one");
        shared.queue_up("two");
        // Two passes, because one pass starts at most one thing — which is the
        // property under test.
        runner.pump();
        let after_one = log.lock().expect("log").clone();
        runner.pump();
        let both = log.lock().expect("log").clone();

        assert_eq!(after_one, vec![("one", '('), ("one", ')')]);
        assert_eq!(
            both,
            vec![("one", '('), ("one", ')'), ("two", '('), ("two", ')')],
            "the two crossed each other"
        );
        let (mind, _) = shared.snapshot();
        assert!(mind.queue.is_empty());
        assert_eq!(mind.running, "");
    }

    #[test]
    fn a_hold_is_published_and_never_written_down() {
        // THE DEFECT THIS CATCHES: the machine being busy at two in the
        // morning recorded as an event, and a weekly update pushed a whole week
        // — for something that never happened. A hold is a state of the
        // MACHINE; it belongs on the row and nowhere else.
        let busy = Pulse {
            ram_used_pct: Some(99),
            ..Pulse::default()
        };
        let (shared, mut runner) = a_runner(vec![Box::new(Always)], busy, 1000);
        runner.pump();
        runner.pump();
        assert!(
            runner.journal.runs.is_empty(),
            "it wrote the wait into the journal"
        );
        assert_eq!(
            shared.snapshot().0.task("always").and_then(|t| t.hold),
            Some(Hold::Ram)
        );
    }

    #[test]
    fn a_postponement_is_written_down_and_the_task_leaves_the_queue() {
        // "NOT NOW" HAS TO MEAN SOMETHING. Without a record the next tick, half
        // a minute later, finds the task due again and asks the same question —
        // and it keeps asking until the window closes.
        let (shared, mut runner) = a_runner(vec![Box::new(Once)], Pulse::default(), 1000);
        shared.queue_up("once");
        shared.change(|m| m.postponed = Some("once".to_owned()));
        runner.pump();
        assert_eq!(
            runner.journal.runs.len(),
            1,
            "it did not note the postponement"
        );
        assert_eq!(runner.journal.runs[0].why, "postponed");
        assert_eq!(runner.journal.last("once"), Some(1000));
        // AND IT IS NOT ASKED AGAIN. `Once` panics if it runs, and without the
        // record it would be due on this very pass — which is the defect: the
        // person answers "not now" and is asked again half a minute later.
        runner.pump();
        let (mind, _) = shared.snapshot();
        assert!(mind.queue.is_empty(), "it went back into the queue");
        assert_eq!(
            mind.task("once").map(|t| t.why_not.clone()).as_deref(),
            Some("already")
        );
        assert!(mind.task("once").and_then(|t| t.last.as_ref()).is_some());
    }

    #[test]
    fn what_waits_behind_something_else_says_so() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let tasks: Vec<Box<dyn Task>> = vec![
            Box::new(Slow {
                id: "one",
                log: Arc::clone(&log),
                takes: Duration::from_millis(1),
            }),
            Box::new(Slow {
                id: "two",
                log: Arc::clone(&log),
                takes: Duration::from_millis(1),
            }),
        ];
        let (shared, mut runner) = a_runner(tasks, Pulse::default(), 1000);
        // `two` is queued while `one` is marked as running: the row has to say
        // it is waiting for the brain and not for the machine.
        shared.change(|m| m.running = "one".to_owned());
        shared.queue_up("two");
        runner.pump();
        let (mind, _) = shared.snapshot();
        assert_eq!(mind.task("two").and_then(|t| t.hold), Some(Hold::Busy));
        assert_eq!(mind.task("two").map(|t| t.standing), Some(Standing::Queued));
    }

    #[test]
    fn the_countdown_holds_it_back_until_the_deadline() {
        let log = Arc::new(Mutex::new(Vec::new()));
        struct Warned(Arc<Mutex<Vec<(&'static str, char)>>>);
        impl Task for Warned {
            fn id(&self) -> &'static str {
                "warned"
            }
            fn due(&self, _now: Moment, _prefs: &Prefs, _last: Option<Moment>) -> Due {
                Due::Not { why: "test" }
            }
            fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
                Verdict::Go
            }
            fn countdown(&self, _prefs: &Prefs) -> u16 {
                60
            }
            fn run(&self, _report: &Report) -> Outcome {
                self.0.lock().expect("log").push(("warned", '('));
                Outcome::Done(Vec::new())
            }
        }
        let clock = Arc::new(Mutex::new(1000));
        let shared = Arc::new(Shared::of(Prefs::default()));
        let mut runner = Runner::new(
            Arc::clone(&shared),
            Registry::of(vec![Box::new(Warned(Arc::clone(&log)))]).expect("the registry"),
            Box::new(FakeMeter(Pulse::default())),
            Box::new(Hand(Arc::clone(&clock))),
            nowhere(),
        );
        shared.queue_up("warned");
        runner.pump();
        let (mind, _) = shared.snapshot();
        let announced = mind.announcement.expect("it did not warn");
        assert_eq!(announced.deadline, 1060);
        assert_eq!(announced.total_seconds, 60);
        assert!(
            log.lock().expect("log").is_empty(),
            "it ran without warning"
        );

        // The clock reaches the deadline, and only then does it run.
        *clock.lock().expect("clock") = 1060;
        runner.pump();
        assert_eq!(log.lock().expect("log").len(), 1);
        assert!(
            shared.snapshot().0.announcement.is_none(),
            "the notice stayed"
        );
    }

    #[test]
    fn a_hold_takes_the_countdown_off_the_screen() {
        // A warning that kept counting while the gate was refusing would end at
        // zero with nothing happening — which is the shape of a promise a
        // screen must not make.
        let shared = Arc::new(Shared::of(Prefs::default()));
        let mut runner = Runner::new(
            Arc::clone(&shared),
            Registry::of(vec![Box::new(Always)]).expect("the registry"),
            Box::new(FakeMeter(Pulse {
                ram_used_pct: Some(99),
                ..Pulse::default()
            })),
            Box::new(Hand(Arc::new(Mutex::new(10)))),
            nowhere(),
        );
        shared.change(|m| {
            m.announcement = Some(Announcement {
                task: "always".to_owned(),
                total_seconds: 60,
                deadline: 70,
                why: "due".to_owned(),
            });
        });
        runner.pump();
        assert!(shared.snapshot().0.announcement.is_none());
    }

    #[test]
    fn a_step_the_person_switched_off_is_drawn_as_changed() {
        static STEPS: &[Step] = &[
            Step {
                id: "on-by-default",
                command: "a",
                needs_root: true,
                on_by_default: true,
            },
            Step {
                id: "off-by-default",
                command: "b",
                needs_root: false,
                on_by_default: false,
            },
        ];
        struct Stepped;
        impl Task for Stepped {
            fn id(&self) -> &'static str {
                "stepped"
            }
            fn due(&self, _now: Moment, _prefs: &Prefs, _last: Option<Moment>) -> Due {
                Due::Not { why: "test" }
            }
            fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
                Verdict::Go
            }
            fn countdown(&self, _prefs: &Prefs) -> u16 {
                0
            }
            fn steps(&self) -> &'static [Step] {
                STEPS
            }
            fn run(&self, _report: &Report) -> Outcome {
                Outcome::Done(Vec::new())
            }
        }
        let (shared, mut runner) = a_runner(vec![Box::new(Stepped)], Pulse::default(), 1);
        // The person switches off the one that ships on, and switches ON the
        // one that ships off — both are "changed", and the screen says so.
        shared.change(|m| {
            m.prefs
                .mine("stepped")
                .steps_off
                .insert("on-by-default".into());
        });
        runner.pump();
        let (mind, _) = shared.snapshot();
        let steps = &mind.task("stepped").expect("row").steps;
        assert!(!steps[0].on, "the switched-off step is still on");
        assert!(
            steps[0].changed,
            "the default was switched off and the screen did not say so"
        );
        assert!(steps[1].on, "the step the person switched on is still off");
        assert!(
            steps[1].changed,
            "the step that comes switched off is on and the screen did not say so"
        );
    }

    #[test]
    fn a_row_carries_the_next_moment_and_the_word_when_there_is_none() {
        struct Scheduled(Arc<dyn Offset>);
        impl Task for Scheduled {
            fn id(&self) -> &'static str {
                "scheduled"
            }
            fn due(&self, now: Moment, prefs: &Prefs, last: Option<Moment>) -> Due {
                let mine = prefs.of("scheduled");
                if !mine.enabled {
                    return Due::Not { why: "off" };
                }
                let at = crate::schedule::next_moment(now, last, &mine, self.0.as_ref());
                if at <= now { Due::Now } else { Due::At(at) }
            }
            fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
                Verdict::Go
            }
            fn countdown(&self, _prefs: &Prefs) -> u16 {
                0
            }
            fn run(&self, _report: &Report) -> Outcome {
                Outcome::Done(Vec::new())
            }
        }
        let zone: Arc<dyn Offset> = Arc::new(Fixed(0));
        let shared = Arc::new(Shared::of(Prefs {
            tasks: [(
                "scheduled".to_owned(),
                TaskPrefs {
                    part_of_day: PartOfDay::Night,
                    ..TaskPrefs::default()
                },
            )]
            .into_iter()
            .collect(),
            ..Prefs::default()
        }));
        let mut runner = Runner::new(
            Arc::clone(&shared),
            Registry::of(vec![Box::new(Scheduled(zone))]).expect("the registry"),
            Box::new(FakeMeter(Pulse::default())),
            // 09:00 on day 0: the night window closed, so the next one is
            // tomorrow at 02:00.
            Box::new(Hand(Arc::new(Mutex::new(9 * 3600)))),
            nowhere(),
        );
        runner.pump();
        let (mind, _) = shared.snapshot();
        let row = mind.task("scheduled").expect("row");
        assert_eq!(row.next, 86_400 + 2 * 3600);
        assert!(row.why_not.is_empty());

        // Switched off, there is no moment at all — and the row says the word
        // instead of a date nothing will honour.
        shared.change(|m| m.prefs.mine("scheduled").enabled = false);
        runner.pump();
        let (mind, _) = shared.snapshot();
        let row = mind.task("scheduled").expect("row");
        assert_eq!(row.next, 0);
        assert_eq!(row.why_not, "off");
        assert!(!row.enabled);
    }
}
