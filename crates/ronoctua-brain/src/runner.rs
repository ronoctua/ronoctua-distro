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
use crate::prefs::{Gate, Prefs, TaskPrefs, Where};
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

/// HOW MANY OF A TASK'S OWN RUNS A SCREEN IS TOLD ABOUT.
///
/// Ten, and the owner asked for ten in those words.
///
/// It is [`crate::journal::RECENT`] and not a second number, which is the
/// whole point of it living there: the journal's pruning uses it as a FLOOR —
/// a task keeps its last ten whatever else fills the list — and the screen
/// uses it as a count. Two constants would let the screen promise ten of
/// something the journal had already thrown away.
pub use crate::journal::RECENT;

/// How often a run in flight is looked at.
///
/// Two seconds. It is a `stat` of one file — no process spawned, no journal
/// parsed — and it is what decides how quickly `Esc` on the screen becomes a
/// unit that stops.
pub const BEAT: Duration = Duration::from_secs(2);

/// The directory a unit writes its report into, and the brain reads.
///
/// # Why the runtime directory and not `/run`
///
/// Because it was `/run/ronoctua-brain` and **nothing on the machine ever wrote
/// there.** The units are the person's — `%t` for them is `$XDG_RUNTIME_DIR`,
/// which is `/run/user/<uid>` — so the daemon watched a path that did not
/// exist, `Report::watch` never saw an mtime change, and every run the brain
/// started ended as [`crate::task::Watched::Runaway`] once the patience ran
/// out: two hours for an update, half an hour for a sweep. The screen then
/// said `failed / runaway` about a rebuild that had worked.
///
/// It had no symptom anybody could see quickly, because the tests all point
/// `Runner::reporting_in` at a scratch directory and never name this one. What
/// closes that is `the-brain-reads-what-the-units-write`, which reads the
/// argument out of `nix/modules/*.nix` and compares it with this function.
///
/// The brain only ever READS here, which is why the previous report is MARKED
/// rather than deleted ([`crate::task::Report::mark`]).
///
/// The environment may override it, for the same reason `RONOCTUA_YT_DLP`
/// exists: what sets it is the flake, inside the block that decides whether any
/// of this is installed at all.
#[must_use]
pub fn reports_dir() -> std::path::PathBuf {
    if let Some(said) = std::env::var_os("RONOCTUA_BRAIN_REPORTS") {
        return std::path::PathBuf::from(said);
    }
    // NO FALLBACK TO `/tmp`, and none to `/run` either. Without a runtime
    // directory there is nowhere a unit of this person's could have written,
    // so a path here would be a guess that reads as an answer. What this
    // returns then cannot exist, and `watch` says `runaway` — which is the
    // honest end for a run nobody can hear.
    std::env::var_os("XDG_RUNTIME_DIR").map_or_else(
        || std::path::PathBuf::from("/nonexistent/ronoctua-brain"),
        |dir| std::path::PathBuf::from(dir).join(REPORTS_LEAF),
    )
}

/// The one name the daemon and the units have to agree on.
///
/// Spelled once here and read by the check against the `.nix` that writes it.
pub const REPORTS_LEAF: &str = "ronoctua-brain";

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
    /// When each task last STARTED, a rehearsal included — which the journal
    /// never records. A task that wants itself run again after a rest counts
    /// the rest from here ([`Runner::enrol`]).
    started: std::collections::HashMap<String, Moment>,
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
        let reports = reports_dir();
        Self {
            shared,
            tasks,
            meter,
            clock,
            store,
            journal,
            reports,
            started: std::collections::HashMap::new(),
        }
    }

    /// Where the report of a task is written and read.
    ///
    /// # Why the rehearsal has a file of its own
    ///
    /// Because the units do: `ronoctua-cache-sweep-rehearsal.service` writes
    /// `cache-sweep-rehearsal.json`, beside the real one. Watching the real
    /// file during a rehearsal reads whatever the last true run left there —
    /// an mtime that never changes, so the rehearsal ran out its patience and
    /// came back `runaway` while the rehearsal unit finished in a second.
    #[must_use]
    pub fn report_path(&self, task: &str, rehearsal: bool) -> std::path::PathBuf {
        let leaf = if rehearsal {
            format!("{task}-rehearsal.json")
        } else {
            format!("{task}.json")
        };
        self.reports.join(leaf)
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
        // THE HEARTBEAT, one line a pass: what the gate is about to judge by,
        // and where the queue stands. The owner asked to see every tick on the
        // brain's page, and the page is a copy of this log (`crate::tail`).
        {
            let (mind, _) = self.shared.snapshot();
            tracing::info!(
                ram_pct = pulse.ram_used_pct,
                memory_stall = pulse.memory_stall,
                cpu_stall = pulse.cpu_stall,
                on_battery = pulse.on_battery,
                disk_free_per_mille = pulse.disk_free_per_mille,
                disk_short = pulse.disk_short,
                queued = mind.queue.len(),
                running = %mind.running,
                "tick"
            );
        }
        // BEFORE ANYTHING IS ENROLLED. A postponement that were written down
        // after the enrolment would put the task straight back in the queue it
        // was just taken out of, and the person would answer the same question
        // every half minute until the window closed.
        self.put_off(now);
        let prefs = self.publish(now, pulse);
        self.enrol(now, &pulse, &prefs);
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
        // AND WHAT EACH TASK CHOSE FOR ITSELF travels with its row, because the
        // row is built inside `change` where the tasks are no longer in reach —
        // and a row that resolved the preferences some other way would draw a
        // cadence the schedule does not use. See `Task::defaults`.
        let rows: Vec<(
            &'static str,
            crate::task::Kind,
            Due,
            &'static [crate::task::Step],
            TaskPrefs,
        )> = self
            .tasks
            .all()
            .map(|t| {
                let due = t.due(now, &prefs, self.journal.last(t.id()));
                (t.id(), t.kind(), due, t.steps(), prefs.of_task(t))
            })
            .collect();

        // THE LOG, COPIED ONCE PER PASS. It is bounded by `ACTIVITY` here and
        // not on the way out, so what a screen is told and what the socket
        // carries are the same list — the argument `wire` makes about sending
        // the mind whole only holds if the mind IS what is sent.
        let log: Vec<crate::task::RunRecord> = self
            .journal
            .all()
            .iter()
            .rev()
            .take(crate::journal::ACTIVITY)
            .rev()
            .cloned()
            .collect();
        self.shared.change(|mind| {
            mind.now = now;
            mind.pulse = pulse;
            mind.journal.clone_from(&log);
            let running = mind.running.clone();
            let queued: Vec<String> = mind.queue.clone();
            let old: Vec<TaskState> = std::mem::take(&mut mind.tasks);
            mind.tasks = rows
                .iter()
                .map(|(id, kind, due, steps, mine)| {
                    let mine = mine.clone();
                    let was = old.iter().find(|t| t.id == *id);
                    // AND THE LAST FEW RUNS OF THIS ONE, asked of the journal
                    // per TASK: the journal is one list for every task, so on a
                    // machine where one runs weekly and another monthly the
                    // second one's records fall off the end of a shared window
                    // while its own row still has to show them.
                    let history = self.journal.recent(id, RECENT);
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
                        kind: *kind,
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
                        last: history.first().cloned(),
                        history,
                        steps: steps
                            .iter()
                            .map(|s| StepState {
                                id: s.id.to_owned(),
                                command: s.command.to_owned(),
                                needs_root: s.needs_root,
                                // ONE LINE FOR BOTH ANSWERS, and it is the
                                // whole of the fix. `on` used to be
                                // `!contains(id)`, which reads the set as a
                                // list of switched-off steps and so assumes
                                // every step ships ON. Six of the update's
                                // twelve ship OFF, so on an untouched machine
                                // those six drew with a tick AND the note
                                // "you changed" — see `TaskPrefs::steps_flipped`,
                                // which carries the whole story.
                                on: s.on_by_default ^ mine.steps_flipped.contains(s.id),
                                on_by_default: s.on_by_default,
                            })
                            .collect(),
                    }
                })
                .collect();
        });
        prefs
    }

    /// Puts into the queue whatever has come due — by its calendar, or
    /// because the machine wants it now ([`crate::task::Task::wanted_now`]).
    ///
    /// A task the gate may not hold goes to the HEAD of the queue, since the
    /// runner looks at nothing else: behind a task the gate is holding it would
    /// be held all the same (`Shared::queue_first`).
    ///
    /// The calendar counts from the journal, where a rehearsal is not a run.
    /// `wanted_now` counts from the last START, a rehearsal included: a task
    /// that rests between runs rests after a rehearsal too — otherwise a brain
    /// that only rehearses (the development loop's) would start it on every
    /// tick, a walk of the cache and a question to the store twice a minute.
    fn enrol(&self, now: Moment, pulse: &Pulse, prefs: &Prefs) {
        for task in self.tasks.all() {
            let last = self.journal.last(task.id());
            let started = self.started.get(task.id()).copied().max(last);
            let wanted = matches!(task.due(now, prefs, last), Due::Now)
                || task.wanted_now(now, pulse, prefs, started);
            if !wanted {
                continue;
            }
            if task.gated() {
                self.shared.queue_up(task.id());
            } else {
                self.shared.queue_first(task.id());
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
            // NOTHING WAITING, NOTHING ANNOUNCED AND NOTHING HELD.
            //
            // An announcement outliving its queue would be a countdown on the
            // footer for something that was cancelled — and a HOLD outliving
            // it is the same defect one step quieter. A hold means "this is
            // waiting for a better moment"; with nothing waiting there is
            // nothing to be held, and the row went on saying there was.
            //
            // It showed up the moment the brain's screen grew a line that
            // alarms: a task held at two in the morning left "nothing will
            // start while the processor is this busy" on the page for the rest
            // of the day, on a machine doing nothing at all. The row carried
            // it all along; before there was a danger line, nobody could see
            // it.
            let stale = mind.tasks.iter().any(|t| t.hold.is_some());
            if mind.announcement.is_some() || stale {
                self.shared.change(|m| {
                    m.announcement = None;
                    for row in &mut m.tasks {
                        row.hold = None;
                    }
                });
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
        //
        // UNLESS THE TASK IS ONE THE GATE MAY NOT HOLD: a relief of the disk
        // waiting for the memory to calm down is a relief waiting for the
        // build that fills the disk to finish filling it.
        let verdict = if task.gated() {
            match judge(pulse, &prefs.gate) {
                Verdict::Go => task.needs(pulse, prefs),
                held => held,
            }
        } else {
            task.needs(pulse, prefs)
        };
        if let Verdict::Hold(hold) = verdict {
            // ONCE PER EPISODE, and never once per tick.
            //
            // This pass runs every thirty seconds, so an hour of a busy
            // machine is a hundred and twenty looks at the same fact. Writing
            // one down each time would make the activity log a wall of "RAM is
            // busy" with everything that actually happened buried under it —
            // and it would evict the real runs through the journal's ceiling.
            //
            // The episode is "the row was not already held by this". The row
            // is cleared the moment anything else happens to the task, so a
            // hold that lifts and comes back is two episodes, which is what a
            // person reading the log wants to know.
            let already = self
                .shared
                .snapshot()
                .0
                .task(&id)
                .is_some_and(|t| t.hold == Some(hold));
            if !already {
                let record = RunRecord::of(
                    &id,
                    now,
                    now,
                    crate::task::Outcome::Skipped {
                        why: hold.word().to_owned(),
                    },
                );
                // `Held` AND NOT `Skipped`, which is what `Outcome::Skipped`
                // would have made it: skipped is a run that was considered and
                // dropped, held is a run still waiting for a better moment.
                // The screen draws them in different words.
                let record = RunRecord {
                    kind: crate::task::OutcomeKind::Held,
                    ..record
                };
                self.journal.put(record);
                if let Some(dir) = self.store.state.as_deref()
                    && let Err(e) = self.journal.write_to(dir)
                {
                    tracing::error!(error = %e, "could not write the brain's journal");
                }
            }
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
        // A COUNTDOWN OF ZERO IS NO WARNING, and it is not published. It was:
        // the announcement stood for the few milliseconds until `perform`
        // took it down, and every screen drew it — the footer grew a line for
        // the brain and shrank again, so the modeline and the minibuffer
        // jumped every thirty seconds, as long as a watch (`emacs-daemon`,
        // `clock-sync`, both zero by design) kept its minute.
        if seconds == 0 {
            self.perform(&id, now);
            return;
        }
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
        // A SECOND COPY OF THIS SYSTEM ONLY EVER REHEARSES. The development
        // loop brings its own brain up, and on an installed machine that is a
        // second scheduler beside the real one: both would rebuild the
        // system, sweep the cache and restart units on their own clocks.
        // Rehearsing keeps every task exercisable from the loop and changes
        // nothing — see `ronoctua_xdg::INSTANCE`.
        rehearsal |= ronoctua_xdg::instance().is_some();
        // AND THE PATIENCE IS THE TASK'S OWN, for the reason `Task::defaults`
        // gives: two hours is right for a rebuild and absurd for a sweep, and a
        // number resolved differently here from the one on the row would be the
        // screen saying one thing and the watcher doing another.
        let patience = Duration::from_secs(
            u64::from(self.shared.snapshot().0.prefs.of_task(task).runaway_minutes) * 60,
        );
        let mut report = Report::at(
            self.report_path(id, rehearsal),
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
        self.started.insert(id.to_owned(), began);
        let outcome = task.run(&report);
        // AND WHAT IT WANTS SAID, which is not what it did. A watch's whole
        // output is a sentence; see `task::Notice`.
        let said = report.said();
        // AND WHAT A RELIEF OF THE DISK DID, stamped with this clock — never
        // a rehearsal's, which changed nothing.
        let relieved = report.relieved().filter(|_| !rehearsal);
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
            // THE NOTICE IS REPLACED AND NOT MERGED, including with `None`:
            // a watch that finds nothing wrong takes its own line down. Only
            // the task that raised one may clear it, so a sweep finishing
            // does not silence a warning about Emacs.
            if said.is_some() || m.notice.as_ref().is_some_and(|n| n.task == id) {
                m.notice.clone_from(&said);
            }
            if let Some(r) = relieved {
                m.relieved = Some(crate::task::Relieved { at: ended, ..r });
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
    fn a_hold_is_written_down_once_per_episode() {
        // THE DEFECT THIS CATCHES, and it is not the one this test used to
        // catch. It used to assert that a hold is NEVER written down, and the
        // reason was sound as far as it went: the machine being busy at two in
        // the morning is not an event, and recording one would push a weekly
        // update a whole week for something that never happened.
        //
        // What changed is that the home page now shows the last hundred things
        // the brain did, and "nothing, because the memory was busy" is one of
        // the most useful of them — on a machine where a task keeps not
        // running, it is the ONLY thing that explains why. So a hold is a
        // record, and it is a record that does not move the schedule: nothing
        // reads `Held` as a run.
        //
        // What it must not be is a record per TICK. This pass runs every
        // thirty seconds, so an hour of a busy machine would be a hundred and
        // twenty identical lines — a wall of "RAM is busy" with everything
        // that actually happened buried underneath it, and the journal's
        // ceiling evicting the real runs to make room.
        let busy = Pulse {
            ram_used_pct: Some(99),
            ..Pulse::default()
        };
        let (shared, mut runner) = a_runner(vec![Box::new(Always)], busy, 1000);
        runner.pump();
        runner.pump();
        runner.pump();
        assert_eq!(
            runner.journal.runs.len(),
            1,
            "three passes over one busy machine wrote {} lines",
            runner.journal.runs.len()
        );
        let wrote = &runner.journal.runs[0];
        assert_eq!(wrote.kind, crate::task::OutcomeKind::Held);
        assert_eq!(
            wrote.why, "ram",
            "the record does not say what was in the way"
        );
        assert_eq!(
            shared.snapshot().0.task("always").and_then(|t| t.hold),
            Some(Hold::Ram)
        );

        // AND IT DOES NOT OUTLIVE THE QUEUE.
        //
        // A hold means "this is waiting for a better moment"; with nothing
        // waiting there is nothing to be held, and a row that went on saying
        // so put "the processor is too busy" on the brain's screen for the
        // rest of the day on a machine doing nothing at all. The row carried
        // it all along — before there was a line that alarms, nobody could
        // see it.
        //
        // THE QUEUE IS CLEARED AND THE TASK IS TAKEN AWAY, because `enrol`
        // puts a still-due task straight back: emptying the queue under a
        // task that is due is not a state this daemon can be in. A task
        // ABORTED or POSTPONED is, and that is what this stands for.
        let (quiet, mut alone) = a_runner(Vec::new(), busy, 1000);
        quiet.change(|m| {
            m.tasks = shared.snapshot().0.tasks;
            m.queue.push("always".to_owned());
        });
        assert_eq!(
            quiet.snapshot().0.task("always").and_then(|t| t.hold),
            Some(Hold::Ram)
        );
        quiet.change(|m| m.queue.clear());
        alone.pump();
        assert_eq!(
            quiet.snapshot().0.task("always").and_then(|t| t.hold),
            None,
            "a hold outlived the queue it belonged to"
        );

        // AND IT DOES NOT MOVE THE SCHEDULE.
        //
        // `Journal::last` is what `due` counts from, and a wait is not a run.
        // With the hold as the only record this has to answer NOTHING — the
        // task has never run — and not "it ran at the moment it was refused",
        // which would push a weekly task a whole week every time the machine
        // was busy at its moment.
        assert_eq!(
            runner.journal.last("always"),
            None,
            "a wait was counted as a run"
        );
        // And the SCREEN still sees it, which is the half that makes the log
        // worth having: `latest` answers with the last thing that happened,
        // whatever it was.
        assert_eq!(
            runner.journal.latest("always").map(|r| r.kind),
            Some(crate::task::OutcomeKind::Held)
        );
    }

    /// A task the gate may not hold, wanted while it has never run — and it
    /// says what it freed, the way the disk's relief does.
    struct Relief(Arc<AtomicUsize>);

    impl Task for Relief {
        fn id(&self) -> &'static str {
            "relief"
        }
        fn due(&self, _: Moment, _: &Prefs, _: Option<Moment>) -> Due {
            Due::Not { why: "when-short" }
        }
        fn wanted_now(&self, _: Moment, _: &Pulse, _: &Prefs, last: Option<Moment>) -> bool {
            last.is_none()
        }
        fn gated(&self) -> bool {
            false
        }
        fn needs(&self, _: &Pulse, _: &Prefs) -> Verdict {
            Verdict::Go
        }
        fn countdown(&self, _: &Prefs) -> u16 {
            0
        }
        fn run(&self, report: &Report) -> Outcome {
            self.0.fetch_add(1, Ordering::Relaxed);
            report.relieve(crate::task::Relieved {
                at: 7,
                freed_bytes: 42,
                free_per_mille: 123,
                short: false,
            });
            Outcome::Done(Vec::new())
        }
    }

    #[test]
    fn a_relief_runs_past_a_busy_gate_and_ahead_of_the_queue() {
        // THE DEFECT THIS CATCHES, twice over. The gate holds everything while
        // the memory is busy — and a disk filled by a heavy build is exactly a
        // machine whose memory is busy, so a relief behind the gate would wait
        // for the disk to be full. And the runner looks only at the HEAD of the
        // queue: a relief appended behind a task the gate is holding would be
        // held all the same.
        let busy = Pulse {
            ram_used_pct: Some(99),
            ..Pulse::default()
        };
        let ran = Arc::new(AtomicUsize::new(0));
        let (shared, mut runner) = a_runner(
            vec![Box::new(Always), Box::new(Relief(Arc::clone(&ran)))],
            busy,
            1000,
        );
        // `Always` is at the head already, held by the gate.
        shared.queue_up("always");
        runner.pump();
        assert_eq!(
            ran.load(Ordering::Relaxed),
            1,
            "the relief waited behind the gate"
        );
        let (mind, _) = shared.snapshot();
        assert_eq!(
            mind.queue,
            vec!["always".to_owned()],
            "the held task lost its place"
        );
        // AND WHAT IT FREED REACHES THE MIND, stamped by the runner's clock
        // and not by whatever the task wrote.
        assert_eq!(
            mind.relieved,
            Some(crate::task::Relieved {
                at: 1000,
                freed_bytes: 42,
                free_per_mille: 123,
                short: false,
            })
        );
        // THE GATED ONE IS STILL HELD, by name.
        runner.pump();
        assert_eq!(
            ran.load(Ordering::Relaxed),
            1,
            "it ran again with nothing short"
        );
        assert_eq!(
            shared.snapshot().0.task("always").and_then(|t| t.hold),
            Some(Hold::Ram)
        );
    }

    #[test]
    fn a_rehearsed_relief_rests_like_a_real_one() {
        // THE DEFECT THIS CATCHES: a rehearsal is not written in the journal,
        // so a task that rests "since the last run" never rested after one —
        // and a brain that only rehearses (the development loop's) started
        // the disk's relief on every tick.
        let ran = Arc::new(AtomicUsize::new(0));
        let (shared, mut runner) = a_runner(
            vec![Box::new(Relief(Arc::clone(&ran)))],
            Pulse::default(),
            1000,
        );
        shared.change(|m| m.rehearsing = Some("relief".to_owned()));
        runner.pump();
        assert_eq!(ran.load(Ordering::Relaxed), 1);
        runner.pump();
        runner.pump();
        assert_eq!(
            ran.load(Ordering::Relaxed),
            1,
            "a relief rehearsed a moment ago ran again"
        );
    }

    #[test]
    fn a_rehearsed_relief_says_nothing_about_the_disk() {
        // A REHEARSAL CHANGED NOTHING, so "room made" about it would be a lie.
        let ran = Arc::new(AtomicUsize::new(0));
        let (shared, mut runner) = a_runner(
            vec![Box::new(Relief(Arc::clone(&ran)))],
            Pulse::default(),
            1000,
        );
        shared.change(|m| m.rehearsing = Some("relief".to_owned()));
        runner.pump();
        assert_eq!(ran.load(Ordering::Relaxed), 1);
        assert_eq!(shared.snapshot().0.relieved, None);
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

    /// A TASK WITH NO COUNTDOWN RUNS WITHOUT A WARNING — not even one that
    /// stands for the instant before it starts, which every screen drew.
    #[test]
    fn a_zero_countdown_is_never_announced() {
        struct Watch(Arc<Mutex<u8>>);
        impl Task for Watch {
            fn id(&self) -> &'static str {
                "watch"
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
            fn run(&self, _report: &Report) -> Outcome {
                *self.0.lock().expect("runs") += 1;
                Outcome::Done(Vec::new())
            }
        }
        let runs = Arc::new(Mutex::new(0));
        let shared = Arc::new(Shared::of(Prefs::default()));
        let mut runner = Runner::new(
            Arc::clone(&shared),
            Registry::of(vec![Box::new(Watch(Arc::clone(&runs)))]).expect("the registry"),
            Box::new(FakeMeter(Pulse::default())),
            Box::new(Hand(Arc::new(Mutex::new(1000)))),
            nowhere(),
        );
        shared.queue_up("watch");
        runner.pump();
        assert_eq!(
            *runs.lock().expect("runs"),
            1,
            "it did not run in the same pass"
        );
        assert!(
            !shared.ever_announced(),
            "a countdown of zero was published"
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
    fn a_step_nobody_touched_is_not_changed() {
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
        // FIRST, THE MACHINE NOBODY HAS TOUCHED. This is the case that was
        // wrong, and it was wrong in the direction that is hardest to notice:
        // the set of exceptions is empty, so `!contains(id)` answered "on" for
        // BOTH rows, and "changed" — which was `contains(id) == on_by_default`
        // — answered true for the one that ships off. The owner read six of
        // those on the update's page and asked what they meant.
        let (shared, mut runner) = a_runner(vec![Box::new(Stepped)], Pulse::default(), 1);
        runner.pump();
        let (mind, _) = shared.snapshot();
        let steps = &mind.task("stepped").expect("row").steps;
        assert!(
            steps[0].on,
            "a step that ships on is off with no file at all"
        );
        assert!(
            !steps[1].on,
            "a step that ships OFF is drawn as on with no file at all"
        );
        for s in steps {
            assert_eq!(
                s.on, s.on_by_default,
                "`{}` differs from its default on a machine nobody has touched",
                s.id
            );
        }

        // AND NOW ONE FLIP, which moves exactly one row and in one direction.
        let (shared, mut runner) = a_runner(vec![Box::new(Stepped)], Pulse::default(), 1);
        shared.change(|m| {
            m.prefs
                .mine("stepped")
                .steps_flipped
                .insert("on-by-default".into());
        });
        runner.pump();
        let (mind, _) = shared.snapshot();
        let steps = &mind.task("stepped").expect("row").steps;
        assert!(!steps[0].on, "the switched-off step is still on");
        assert_ne!(
            steps[0].on, steps[0].on_by_default,
            "the row the person moved does not read as moved"
        );
        assert!(!steps[1].on, "flipping one step moved another one with it");
        assert_eq!(
            steps[1].on, steps[1].on_by_default,
            "an untouched row reads as moved"
        );

        // AND THE OTHER DIRECTION, which the old shape could not express at
        // all: switching ON a step the distro ships OFF.
        let (shared, mut runner) = a_runner(vec![Box::new(Stepped)], Pulse::default(), 1);
        shared.change(|m| {
            m.prefs
                .mine("stepped")
                .steps_flipped
                .insert("off-by-default".into());
        });
        runner.pump();
        let (mind, _) = shared.snapshot();
        let steps = &mind.task("stepped").expect("row").steps;
        assert!(steps[1].on, "a step that was switched on is still off");
        assert_ne!(
            steps[1].on, steps[1].on_by_default,
            "the row the person moved does not read as moved"
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
