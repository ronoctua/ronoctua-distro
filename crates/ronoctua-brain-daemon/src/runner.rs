//! The one thread that decides, and the gate everything passes through.
//!
//! # Why at most one thing runs, and why that lives HERE
//!
//! Because the machine is one machine. Two of the brain's tasks at once is two
//! things competing for the disk and the processor at the moment the brain
//! chose precisely because nothing else was — and the second one would be
//! running under conditions the gate never approved.
//!
//! It is a property of the RUNNER and not of any module: there is one worker
//! thread, so overlap is impossible by construction rather than by everybody
//! remembering. A module added tomorrow inherits the queue by existing.
//!
//! # What it runs
//!
//! Every module that answers [`crate::module::Module::scheduled`], with the
//! person's preferences resolved over the module's own declaration
//! ([`crate::prefs::Prefs::of_module`]). A module that only watches is on the
//! watch thread ([`crate::watch`]) and never here.
//!
//! # A quiet brain
//!
//! With `quiet` in the preferences nothing comes due by itself — neither by
//! its calendar nor because the machine wants it — and every row says so.
//! What somebody asks for still runs.
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

use crate::hub::Core;
use crate::mind::{Announcement, Standing};
use crate::module::Run;
use crate::moment::Moment;
use crate::prefs::{Gate, Prefs};
use crate::pulse::{Meter, Pulse};
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
/// argument out of `nix/modules/*.nix` and compares it with this function —
/// and each brain module's own check, for the units in its folder.
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
    core: Arc<Core>,
    meter: Box<dyn Meter>,
    reports: std::path::PathBuf,
    /// When each module last STARTED, a rehearsal included — which the journal
    /// never records. A module that wants itself run again after a rest counts
    /// the rest from here ([`Runner::enrol`]).
    started: std::collections::HashMap<String, Moment>,
}

impl Runner {
    /// A runner over the brain's one core, with the machine's meter.
    ///
    /// Every seam is a parameter — the clock is the core's, the meter and the
    /// reports' directory are here — for the reason the rest of this
    /// repository gives: a test that had to wait for a Tuesday, or make the
    /// machine busy, is a test nobody runs.
    #[must_use]
    pub fn new(core: Arc<Core>, meter: Box<dyn Meter>) -> Self {
        Self {
            core,
            meter,
            reports: reports_dir(),
            started: std::collections::HashMap::new(),
        }
    }

    /// Where the report of a module is written and read.
    ///
    /// # Why the rehearsal has a file of its own
    ///
    /// Because the units do: a module's rehearsal unit writes
    /// `<id>-rehearsal.json`, beside the real one. Watching the real
    /// file during a rehearsal reads whatever the last true run left there —
    /// an mtime that never changes, so the rehearsal ran out its patience and
    /// came back `runaway` while the rehearsal unit finished in a second.
    #[must_use]
    pub fn report_path(&self, task: &str, rehearsal: bool) -> std::path::PathBuf {
        report_in(&self.reports, task, rehearsal)
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
        let now = self.core.now();
        let pulse = self.meter.read(now);
        // THE HEARTBEAT, one line a pass: what the gate is about to judge by,
        // and where the queue stands. The owner asked to see every tick on the
        // brain's page, and the page is a copy of this log (`crate::tail`).
        {
            let (mind, _) = self.core.shared.snapshot();
            tracing::info!(
                ram_pct = pulse.ram_used_pct,
                memory_stall = pulse.memory_stall,
                cpu_stall = pulse.cpu_stall,
                on_battery = pulse.on_battery,
                disk_free_per_mille = pulse.disk_free_per_mille,
                filesystems = pulse.rooms.len(),
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
        // THE NOTES A WATCHER LEFT WAITING, when no watch thread is there to
        // write them: an action's, a run's.
        self.core.save_journal_if_due(std::time::Instant::now());
        self.core.publish(now, pulse.clone());
        let prefs = self.core.shared.snapshot().0.prefs;
        if !prefs.quiet {
            self.enrol(now, &pulse, &prefs);
        }
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
        self.core.shared.change(|m| task = m.postponed.take());
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
        self.core.put(record.clone());
        // AND IT LEAVES THE QUEUE HERE TOO. The socket already took it out,
        // and doing it again is not belt and braces: what makes a postponement
        // a postponement is the record plus the empty queue, and the two
        // belong in one place — otherwise a caller that set the flag without
        // unqueueing would have the task run in the very next pass, which is
        // the defect this whole path exists to close.
        self.core.shared.unqueue(&task);
        self.core.shared.change(|m| {
            if let Some(row) = m.task_mut(&task) {
                row.last = Some(record.clone());
                row.hold = None;
            }
        });
    }

    /// Puts into the queue whatever has come due — by its calendar, or
    /// because the machine wants it now
    /// ([`crate::module::Scheduled::wanted_now`]).
    ///
    /// A module the gate may not hold goes to the HEAD of the queue, since
    /// the runner looks at nothing else: behind a run the gate is holding it
    /// would be held all the same (`Shared::queue_first`).
    ///
    /// The calendar counts from the journal, where a rehearsal is not a run.
    /// `wanted_now` counts from the last START, a rehearsal included: a module
    /// that rests between runs rests after a rehearsal too — otherwise a brain
    /// that only rehearses (the development loop's) would start it on every
    /// tick, a walk of the cache and a question to the store twice a minute.
    fn enrol(&self, now: Moment, pulse: &Pulse, prefs: &Prefs) {
        for m in self.core.modules().all() {
            let Some(s) = m.scheduled() else { continue };
            let manifest = m.manifest();
            let mine = prefs.of_module(manifest);
            // SWITCHED OFF IS ASKED HERE, by the core, and never left to the
            // module: one that forgot to would run while its row said "off" —
            // by the person, or by the system (`Registry::switched_off`).
            if !mine.enabled || self.core.modules().off(manifest.id) {
                continue;
            }
            let last = self.core.last_run(manifest.id);
            let started = self.started.get(manifest.id).copied().max(last);
            let wanted = matches!(s.due(now, &mine, last), Due::Now)
                || s.wanted_now(now, pulse, &mine, started);
            if !wanted {
                continue;
            }
            if manifest.gated() {
                self.core.shared.queue_up(manifest.id);
            } else {
                self.core.shared.queue_first(manifest.id);
            }
        }
    }

    /// Starts the head of the queue, if everything says it may.
    fn maybe_run(&mut self, now: Moment, pulse: &Pulse, prefs: &Prefs) {
        let (mind, _) = self.core.shared.snapshot();
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
                self.core.shared.change(|m| {
                    m.announcement = None;
                    for row in &mut m.tasks {
                        row.hold = None;
                    }
                });
            }
            return;
        };
        let Some((m, s)) = self
            .core
            .modules()
            .find(&id)
            .and_then(|m| Some((m, m.scheduled()?)))
        else {
            // A queued id nothing runs: the person's file names a module that
            // no longer exists, a rename went half way, or it only watches.
            tracing::warn!(task = %id, "a queued task with no run; dropping it");
            self.core.shared.unqueue(&id);
            return;
        };
        let manifest = m.manifest();
        let mine = prefs.of_module(manifest);

        // THE GENERAL GATE FIRST, then what only the module knows. A hold is
        // written down and published — never swallowed: a scheduler that
        // quietly does nothing is one nobody trusts.
        //
        // UNLESS THE MODULE IS ONE THE GATE MAY NOT HOLD: a relief of the disk
        // waiting for the memory to calm down is a relief waiting for the
        // build that fills the disk to finish filling it.
        let verdict = if manifest.gated() {
            match judge(pulse, &prefs.gate) {
                Verdict::Go => s.needs(pulse, &mine),
                held => held,
            }
        } else {
            s.needs(pulse, &mine)
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
            let already = mind.task(&id).is_some_and(|t| t.hold == Some(hold));
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
                self.core.put(RunRecord {
                    kind: crate::task::OutcomeKind::Held,
                    ..record
                });
                self.core.refresh();
            }
            self.core.shared.change(|m| {
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
        let seconds = mine.countdown_seconds;
        // A COUNTDOWN OF ZERO IS NO WARNING, and it is not published. It was:
        // the announcement stood for the few milliseconds until `perform`
        // took it down, and every screen drew it — the footer grew a line for
        // the brain and shrank again, so the modeline and the minibuffer
        // jumped every thirty seconds, as long as a watch kept its minute.
        if seconds == 0 {
            self.perform(&id, now);
            return;
        }
        let deadline = match &mind.announcement {
            Some(a) if a.task == id => a.deadline,
            _ => {
                let deadline = now + i64::from(seconds);
                let last = self.core.last_run(&id);
                let why = if mine.enabled && matches!(s.due(now, &mine, last), Due::Now) {
                    "due"
                } else {
                    "asked"
                };
                self.core.shared.change(|m| {
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

    /// Runs one module's run, and writes down what happened.
    ///
    /// This is the one place in the daemon that blocks, and it blocks the
    /// runner's own thread. Every client of the socket has a thread of its own
    /// and reads through the lock, and the watch thread goes on hearing, so a
    /// rebuild that takes an hour is an hour in which the screens keep
    /// answering and a process the kernel killed is still written down.
    fn perform(&mut self, id: &str, began: Moment) {
        let core = Arc::clone(&self.core);
        let Some((m, s)) = core
            .modules()
            .find(id)
            .and_then(|m| Some((m, m.scheduled()?)))
        else {
            return;
        };
        let manifest = m.manifest();
        // THE FLAG IS IN THE STATE, because whoever asked for a rehearsal is
        // on a socket thread and this is the runner's. Taken out as it is read:
        // a rehearsal asked for today must not make next week's scheduled run a
        // dry one.
        let mut rehearsal = false;
        core.shared.change(|m| {
            rehearsal = m.rehearsing.as_deref() == Some(id);
            if rehearsal {
                m.rehearsing = None;
            }
        });
        // A SECOND COPY OF THIS SYSTEM ONLY EVER REHEARSES. The development
        // loop brings its own brain up, and on an installed machine that is a
        // second scheduler beside the real one: both would rebuild the
        // system, sweep the cache and restart units on their own clocks.
        // Rehearsing keeps every module exercisable from the loop and changes
        // nothing — see `ronoctua_xdg::INSTANCE`.
        rehearsal |= crate::hub::rehearsing_here();
        // AND THE PATIENCE IS THE MODULE'S OWN: two hours is right for a
        // rebuild and absurd for a sweep, and a number resolved differently
        // here from the one on the row would be the screen saying one thing
        // and the watcher doing another.
        let prefs = core.shared.snapshot().0.prefs;
        let patience =
            Duration::from_secs(u64::from(prefs.of_module(manifest).runaway_minutes) * 60);
        let mut report = Report::at(
            self.report_path(id, rehearsal),
            patience,
            BEAT,
            core.shared.stop_flag(),
        );
        if rehearsal {
            report = report.rehearsing();
        }

        core.shared.change(|m| {
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
        let run = Run::new(report, core.ctx(manifest.id, began, rehearsal));
        let outcome = s.run(&run);
        let ended = core.now();
        tracing::info!(task = id, ?outcome, "finished");

        // A REHEARSAL IS NOT A RUN. Writing it down would push the next real
        // one a whole cadence away — the person would have "updated" the
        // machine by asking what an update would do.
        let record = RunRecord::of(id, began, ended, outcome);
        if !rehearsal {
            core.put(record.clone());
        }

        core.shared.change(|m| {
            m.running.clear();
            if let Some(row) = m.task_mut(id) {
                row.standing = Standing::Idle;
                row.last = Some(record.clone());
            }
        });
        // AND ITS FACE AGAIN: what a run found — a daemon that is down, a
        // clock nobody keeps — is what its notices say, and they should say
        // it now rather than at the next pass.
        core.touch(manifest.id);
        core.refresh();
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
            self.rest(stop);
        }
    }

    /// Waits for something that could change what the next pass decides, or
    /// for the tick.
    ///
    /// # Why not every change of the mind
    ///
    /// Because most of them are not the runner's. A journal line a watcher
    /// wrote down, a notice put away, a module's face: each bumps the mind,
    /// and a pass after each would read the meter, publish a fresh pulse —
    /// a second whole `Mind` to every screen for every one the watch thread
    /// sent — and log a `tick` on the brain's page. What a pass decides from
    /// is the queue, what runs, the announcement, the preferences and the two
    /// flags a socket leaves for it ([`Decides`]); the rest waits for the
    /// tick, which publishes it anyway.
    fn rest(&self, stop: &AtomicBool) {
        let began = std::time::Instant::now();
        let (mind, mut seen) = self.core.shared.snapshot();
        let decided = Decides::of(&mind);
        loop {
            // RECOMPUTED ON EVERY WAKE: a notice snoozed meanwhile shortens
            // it, and the tick never grows.
            let wait = patience(self.core.now(), self.core.next_return(self.core.now()))
                .saturating_sub(began.elapsed());
            if wait.is_zero() || stop.load(Ordering::Relaxed) {
                return;
            }
            let (mind, now) = self.core.shared.wait_past(seen, wait);
            if now == seen || Decides::of(&mind) != decided {
                return;
            }
            seen = now;
        }
    }
}

/// What a pass of the runner decides FROM — the part of the mind whose change
/// is worth a pass before the tick. See [`Runner::rest`].
#[derive(Debug, PartialEq)]
struct Decides {
    queue: Vec<String>,
    running: String,
    announcement: Option<Announcement>,
    prefs: Prefs,
    rehearsing: Option<String>,
    postponed: Option<String>,
}

impl Decides {
    fn of(mind: &crate::mind::Mind) -> Self {
        Self {
            queue: mind.queue.clone(),
            running: mind.running.clone(),
            announcement: mind.announcement.clone(),
            prefs: mind.prefs.clone(),
            rehearsing: mind.rehearsing.clone(),
            postponed: mind.postponed.clone(),
        }
    }
}

/// How long the runner waits: the tick, or less when a snoozed notice comes
/// back sooner — a clock armed for that moment only while something is
/// snoozed.
#[must_use]
pub fn patience(now: Moment, comes_back: Option<Moment>) -> Duration {
    comes_back.map_or(TICK, |at| {
        let seconds = u64::try_from(at.saturating_sub(now)).unwrap_or(0);
        TICK.min(Duration::from_secs(seconds.max(1)))
    })
}

/// `<dir>/<task>.json`, or `<task>-rehearsal.json` for a rehearsal — the one
/// spelling the runner and `ronoctua-brain work` share.
#[must_use]
pub fn report_in(dir: &std::path::Path, task: &str, rehearsal: bool) -> std::path::PathBuf {
    let leaf = if rehearsal {
        format!("{task}-rehearsal.json")
    } else {
        format!("{task}.json")
    };
    dir.join(leaf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{Fake, Hand, ROUTINE, manifest};
    use crate::mind::Shared;
    use crate::module::{Manifest, Module, Schedule};
    use crate::moment::{Fixed, Offset};
    use crate::prefs::{PartOfDay, TaskPrefs, Where};
    use crate::pulse::Fake as FakeMeter;
    use crate::registry::Registry;
    use crate::task::{Outcome, Step};
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    fn a_core(modules: Vec<Box<dyn Module>>, prefs: Prefs, clock: Hand) -> Arc<Core> {
        Arc::new(Core::new(
            Arc::new(Shared::of(prefs)),
            Registry::of(modules).expect("the registry"),
            Where::default(),
            Box::new(clock),
        ))
    }

    fn a_runner(modules: Vec<Box<dyn Module>>, pulse: Pulse, at: Moment) -> (Arc<Shared>, Runner) {
        let core = a_core(modules, Prefs::default(), Hand::at(at));
        let shared = Arc::clone(&core.shared);
        (shared, Runner::new(core, Box::new(FakeMeter(pulse))))
    }

    /// A manifest with a schedule of this shape.
    fn with(id: &'static str, schedule: Schedule) -> Manifest {
        Manifest {
            schedule: Some(schedule),
            ..manifest(id)
        }
    }

    /// A module that writes down when it began and ended, and takes a while.
    fn slow(
        id: &'static str,
        log: &Arc<Mutex<Vec<(&'static str, char)>>>,
        takes: Duration,
    ) -> Box<dyn Module> {
        let log = Arc::clone(log);
        Fake::routine(id)
            .runs(move |_| {
                log.lock().expect("log").push((id, '('));
                std::thread::sleep(takes);
                log.lock().expect("log").push((id, ')'));
                Outcome::Done(Vec::new())
            })
            .boxed()
    }

    /// A module that is due until it has a record, and refuses to run.
    ///
    /// The pair with `always`: this one READS the journal the way a real
    /// module does, which is what makes a postponement provable — `always`
    /// would be due again on the very next pass whatever anybody wrote down.
    fn once() -> Box<dyn Module> {
        Fake::routine("once")
            .due(|_, _, last| {
                if last.is_some() {
                    Due::Not { why: "already" }
                } else {
                    Due::Now
                }
            })
            .runs(|_| panic!("it ran after being put off"))
            .boxed()
    }

    /// A module that is always due — for the gate's test.
    fn always() -> Box<dyn Module> {
        Fake::routine("always")
            .due(|_, _, _| Due::Now)
            .runs(|_| panic!("it ran with the machine busy"))
            .boxed()
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
        // every `Ask::Watch` subscriber.
        let counted = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&counted);
        let (shared, mut runner) = a_runner(
            vec![
                Fake::routine("counted")
                    .due(move |_, _, _| {
                        seen.fetch_add(1, Ordering::Relaxed);
                        Due::Not { why: "never" }
                    })
                    .boxed(),
            ],
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
    fn only_what_a_pass_decides_from_wakes_the_runner() {
        // THE DEFECT THIS CATCHES: every mind the watch thread published woke
        // the runner, whose pass read the meter and published a fresh pulse —
        // a second whole mind to every screen for each of the watch thread's,
        // and a `tick` on the brain's page.
        let counted = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&counted);
        let (shared, mut runner) = a_runner(
            vec![
                Fake::routine("counted")
                    .due(move |_, _, _| {
                        seen.fetch_add(1, Ordering::Relaxed);
                        Due::Not { why: "never" }
                    })
                    .boxed(),
            ],
            Pulse::default(),
            1000,
        );
        let stop = Arc::new(AtomicBool::new(false));
        let mine = Arc::clone(&stop);
        let hand = std::thread::spawn(move || runner.serve(&mine));
        std::thread::sleep(Duration::from_millis(200));
        let before = counted.load(Ordering::Relaxed);
        // WHAT A WATCHER DID: the mind moves, and no pass follows.
        for n in 0..50 {
            shared.change(|m| m.now = 5000 + n);
        }
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(
            counted.load(Ordering::Relaxed),
            before,
            "a change the runner does not decide from woke it"
        );
        // WHAT IT DECIDES FROM: the queue moves, and a pass follows at once.
        shared.queue_up("counted");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while counted.load(Ordering::Relaxed) == before {
            assert!(
                std::time::Instant::now() < deadline,
                "the queue did not wake the runner"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        stop.store(true, Ordering::Relaxed);
        shared.change(|m| m.now = 9000);
        hand.join().expect("the brain's pump");
    }

    #[test]
    fn the_wait_is_the_tick_unless_something_comes_back_sooner() {
        assert_eq!(patience(1000, None), TICK);
        assert_eq!(patience(1000, Some(1010)), Duration::from_secs(10));
        assert_eq!(patience(1000, Some(5000)), TICK);
        // A MOMENT ALREADY PAST is a second, never a spin at zero.
        assert_eq!(patience(1000, Some(900)), Duration::from_secs(1));
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
        let (shared, mut runner) = a_runner(vec![always()], busy, 1000);
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
    fn what_only_the_module_knows_holds_it_by_name_too() {
        let (shared, mut runner) = a_runner(
            vec![
                Fake::routine("offline")
                    .due(|_, _, _| Due::Now)
                    .needs(|_, _| Verdict::Hold(Hold::Network))
                    .runs(|_| panic!("it ran with no network"))
                    .boxed(),
            ],
            Pulse::default(),
            1000,
        );
        runner.pump();
        let (mind, _) = shared.snapshot();
        assert_eq!(
            mind.task("offline").and_then(|t| t.hold),
            Some(Hold::Network)
        );
        assert_eq!(mind.running, "");
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
    fn two_runs_do_not_overlap_and_the_second_waits_its_turn() {
        // THE QUEUE, proved the only way it can be: with a real thread and two
        // modules that record when they started and stopped. One pass runs at
        // most one thing, and the second one's brackets have to open after the
        // first one's closed.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (shared, mut runner) = a_runner(
            vec![
                slow("one", &log, Duration::from_millis(40)),
                slow("two", &log, Duration::from_millis(40)),
            ],
            Pulse::default(),
            1000,
        );
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
        // THE DEFECT THIS CATCHES: a record per TICK. This pass runs every
        // thirty seconds, so an hour of a busy machine would be a hundred and
        // twenty identical lines — a wall of "RAM is busy" with everything
        // that actually happened buried underneath it, and the journal's
        // ceiling evicting the real runs to make room.
        let busy = Pulse {
            ram_used_pct: Some(99),
            ..Pulse::default()
        };
        let (shared, mut runner) = a_runner(vec![always()], busy.clone(), 1000);
        runner.pump();
        runner.pump();
        runner.pump();
        let runs = runner.core.journal().runs.clone();
        assert_eq!(
            runs.len(),
            1,
            "three passes over one busy machine wrote {} lines",
            runs.len()
        );
        assert_eq!(runs[0].kind, crate::task::OutcomeKind::Held);
        assert_eq!(
            runs[0].why, "ram",
            "the record does not say what was in the way"
        );
        assert_eq!(
            shared.snapshot().0.task("always").and_then(|t| t.hold),
            Some(Hold::Ram)
        );
        // AND THE LOG ON SCREEN has it at once, not a pass later.
        assert_eq!(shared.snapshot().0.journal.len(), 1);

        // AND IT DOES NOT OUTLIVE THE QUEUE. A hold means "this is waiting
        // for a better moment"; with nothing waiting there is nothing to be
        // held. THE QUEUE IS CLEARED AND THE MODULE IS TAKEN AWAY, because
        // `enrol` puts a still-due module straight back: emptying the queue
        // under a module that is due is not a state this daemon can be in.
        let (quiet, mut alone) = a_runner(Vec::new(), busy.clone(), 1000);
        quiet.change(|m| {
            m.tasks = shared.snapshot().0.tasks;
            m.queue.push("always".to_owned());
        });
        quiet.change(|m| m.queue.clear());
        alone.maybe_run(1000, &busy, &Prefs::default());
        assert_eq!(
            quiet.snapshot().0.task("always").and_then(|t| t.hold),
            None,
            "a hold outlived the queue it belonged to"
        );

        // AND IT DOES NOT MOVE THE SCHEDULE: `Journal::last` is what `due`
        // counts from, and a wait is not a run.
        assert_eq!(
            runner.core.last_run("always"),
            None,
            "a wait was counted as a run"
        );
        assert_eq!(
            runner.core.journal().latest("always").map(|r| r.kind),
            Some(crate::task::OutcomeKind::Held)
        );
    }

    /// A module the gate may not hold, wanted while it has never run — the
    /// shape of a relief of the disk.
    fn relief(ran: &Arc<AtomicUsize>) -> Box<dyn Module> {
        let ran = Arc::clone(ran);
        Fake::new(with(
            "relief",
            Schedule {
                calendar: false,
                gated: false,
                ..ROUTINE
            },
        ))
        .due(|_, _, _| Due::Not { why: "when-short" })
        .wanted(|_, _, _, last| last.is_none())
        .runs(move |_| {
            ran.fetch_add(1, Ordering::Relaxed);
            Outcome::Done(Vec::new())
        })
        .boxed()
    }

    #[test]
    fn a_relief_runs_past_a_busy_gate_and_ahead_of_the_queue() {
        // THE DEFECT THIS CATCHES, twice over. The gate holds everything while
        // the memory is busy — and a disk filled by a heavy build is exactly a
        // machine whose memory is busy. And the runner looks only at the HEAD
        // of the queue: a relief appended behind a held run would be held all
        // the same.
        let busy = Pulse {
            ram_used_pct: Some(99),
            ..Pulse::default()
        };
        let ran = Arc::new(AtomicUsize::new(0));
        let (shared, mut runner) = a_runner(vec![always(), relief(&ran)], busy, 1000);
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
            "the held one lost its place"
        );
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
        // A ROW WITHOUT A CALENDAR says so, and says why it has no moment.
        let row = shared.snapshot().0.task("relief").cloned().expect("row");
        assert!(!row.calendar);
        assert_eq!(row.why_not, "when-short");
        assert_eq!(row.why_key, "brain-why-when-short");
    }

    #[test]
    fn a_rehearsed_relief_rests_like_a_real_one() {
        // A rehearsal is not written in the journal, so a module that rests
        // "since the last run" never rested after one — and a brain that only
        // rehearses (the development loop's) started the relief every tick.
        // (That a rehearsal TELLS nothing is the module's own rule, tested
        // where it is written.)
        let ran = Arc::new(AtomicUsize::new(0));
        let (shared, mut runner) = a_runner(vec![relief(&ran)], Pulse::default(), 1000);
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
    fn a_postponement_is_written_down_and_the_module_leaves_the_queue() {
        // "NOT NOW" HAS TO MEAN SOMETHING. Without a record the next tick, half
        // a minute later, finds it due again and asks the same question — and
        // it keeps asking until the window closes.
        let (shared, mut runner) = a_runner(vec![once()], Pulse::default(), 1000);
        shared.queue_up("once");
        shared.change(|m| m.postponed = Some("once".to_owned()));
        runner.pump();
        assert_eq!(
            runner.core.journal().runs.len(),
            1,
            "it did not note the postponement"
        );
        assert_eq!(runner.core.journal().runs[0].why, "postponed");
        assert_eq!(runner.core.last_run("once"), Some(1000));
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
        let (shared, mut runner) = a_runner(
            vec![
                slow("one", &log, Duration::from_millis(1)),
                slow("two", &log, Duration::from_millis(1)),
            ],
            Pulse::default(),
            1000,
        );
        shared.change(|m| m.running = "one".to_owned());
        shared.queue_up("two");
        runner.pump();
        let (mind, _) = shared.snapshot();
        assert_eq!(mind.task("two").and_then(|t| t.hold), Some(Hold::Busy));
        assert_eq!(mind.task("two").map(|t| t.standing), Some(Standing::Queued));
    }

    #[test]
    fn the_countdown_is_the_modules_and_holds_it_back_until_the_deadline() {
        let log = Arc::new(Mutex::new(0));
        let ran = Arc::clone(&log);
        let clock = Hand::at(1000);
        let core = a_core(
            vec![
                Fake::new(with(
                    "warned",
                    Schedule {
                        countdown_seconds: 60,
                        ..ROUTINE
                    },
                ))
                .runs(move |_| {
                    *ran.lock().expect("log") += 1;
                    Outcome::Done(Vec::new())
                })
                .boxed(),
            ],
            Prefs::default(),
            clock.clone(),
        );
        let shared = Arc::clone(&core.shared);
        let mut runner = Runner::new(core, Box::new(FakeMeter(Pulse::default())));
        shared.queue_up("warned");
        runner.pump();
        let announced = shared.snapshot().0.announcement.expect("it did not warn");
        assert_eq!((announced.deadline, announced.total_seconds), (1060, 60));
        assert_eq!(announced.why, "asked");
        assert_eq!(*log.lock().expect("log"), 0, "it ran without warning");
        clock.set(1060);
        runner.pump();
        assert_eq!(*log.lock().expect("log"), 1);
        assert!(
            shared.snapshot().0.announcement.is_none(),
            "the notice stayed"
        );
    }

    /// A MODULE WITH NO COUNTDOWN RUNS WITHOUT A WARNING — not even one that
    /// stands for the instant before it starts, which every screen drew.
    #[test]
    fn a_zero_countdown_is_never_announced() {
        let runs = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&runs);
        let (shared, mut runner) = a_runner(
            vec![
                Fake::routine("watch")
                    .runs(move |_| {
                        counting.fetch_add(1, Ordering::Relaxed);
                        Outcome::Done(Vec::new())
                    })
                    .boxed(),
            ],
            Pulse::default(),
            1000,
        );
        shared.queue_up("watch");
        runner.pump();
        assert_eq!(
            runs.load(Ordering::Relaxed),
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
        let (shared, mut runner) = a_runner(
            vec![always()],
            Pulse {
                ram_used_pct: Some(99),
                ..Pulse::default()
            },
            10,
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
                name_key: "",
                does_key: "",
            },
            Step {
                id: "off-by-default",
                command: "b",
                needs_root: false,
                on_by_default: false,
                name_key: "",
                does_key: "",
            },
        ];
        let stepped = || Fake::routine("stepped").steps(STEPS).boxed();
        // FIRST, THE MACHINE NOBODY HAS TOUCHED: every step is what it ships.
        let (shared, mut runner) = a_runner(vec![stepped()], Pulse::default(), 1);
        runner.pump();
        let steps = shared
            .snapshot()
            .0
            .task("stepped")
            .expect("row")
            .steps
            .clone();
        assert!(
            steps[0].on,
            "a step that ships on is off with no file at all"
        );
        assert!(
            !steps[1].on,
            "a step that ships OFF is drawn as on with no file at all"
        );

        // AND NOW ONE FLIP, which moves exactly one row and in one direction.
        let (shared, mut runner) = a_runner(vec![stepped()], Pulse::default(), 1);
        shared.change(|m| {
            m.prefs.tasks.insert(
                "stepped".to_owned(),
                TaskPrefs {
                    steps_flipped: ["on-by-default".to_owned()].into(),
                    ..TaskPrefs::default()
                },
            );
        });
        runner.pump();
        let steps = shared
            .snapshot()
            .0
            .task("stepped")
            .expect("row")
            .steps
            .clone();
        assert!(!steps[0].on, "the switched-off step is still on");
        assert!(!steps[1].on, "flipping one step moved another one with it");
    }

    #[test]
    fn a_row_carries_the_next_moment_and_the_word_when_there_is_none() {
        let zone: Arc<dyn Offset> = Arc::new(Fixed(0));
        let calendar = Fake::new(with(
            "scheduled",
            Schedule {
                part_of_day: PartOfDay::Night,
                ..ROUTINE
            },
        ))
        .due(move |now, mine, last| {
            let at = crate::schedule::next_moment(now, last, mine, zone.as_ref());
            if at <= now { Due::Now } else { Due::At(at) }
        });
        // 09:00 on day 0: the night window closed, so the next one is
        // tomorrow at 02:00 — the MODULE'S window, with no entry in the file.
        let (shared, mut runner) = a_runner(vec![calendar.boxed()], Pulse::default(), 9 * 3600);
        runner.pump();
        let row = shared.snapshot().0.task("scheduled").cloned().expect("row");
        assert_eq!(row.next, 86_400 + 2 * 3600);
        assert!(row.why_not.is_empty());

        // Switched off, there is no moment at all — and the row says the word
        // instead of a date nothing will honour. The CORE says it: the module
        // was not asked.
        runner
            .core
            .set("scheduled", &crate::wire::Knob::Enabled(false))
            .expect("off");
        runner.pump();
        let row = shared.snapshot().0.task("scheduled").cloned().expect("row");
        assert_eq!((row.next, row.why_not.as_str()), (0, "off"));
        assert!(!row.enabled);
    }

    #[test]
    fn a_quiet_brain_starts_nothing_by_itself_and_what_is_asked_still_runs() {
        // THE VIRTUAL MACHINES' LINE: `{"quiet": true}`, naming no module.
        let runs = Arc::new(AtomicUsize::new(0));
        let counting = Arc::clone(&runs);
        let wanted = Arc::new(AtomicUsize::new(0));
        let asked = Arc::clone(&wanted);
        let core = a_core(
            vec![
                Fake::routine("eager")
                    .due(|_, _, _| Due::Now)
                    .wanted(move |_, _, _, _| {
                        asked.fetch_add(1, Ordering::Relaxed);
                        true
                    })
                    .runs(move |_| {
                        counting.fetch_add(1, Ordering::Relaxed);
                        Outcome::Done(Vec::new())
                    })
                    .boxed(),
            ],
            serde_json::from_str(r#"{"quiet": true}"#).expect("prefs"),
            Hand::at(1000),
        );
        let shared = Arc::clone(&core.shared);
        let mut runner = Runner::new(core, Box::new(FakeMeter(Pulse::default())));
        runner.pump();
        runner.pump();
        assert_eq!(
            runs.load(Ordering::Relaxed),
            0,
            "a quiet brain started something"
        );
        assert_eq!(
            wanted.load(Ordering::Relaxed),
            0,
            "a quiet brain asked the machine"
        );
        assert!(shared.snapshot().0.queue.is_empty());
        assert_eq!(
            shared
                .snapshot()
                .0
                .task("eager")
                .map(|t| t.why_not.clone())
                .as_deref(),
            Some("quiet")
        );
        // ASKED FOR, it runs.
        shared.queue_up("eager");
        runner.pump();
        assert_eq!(runs.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_module_that_only_watches_is_never_queued_or_run() {
        let (shared, mut runner) =
            a_runner(vec![Fake::watch("looker").boxed()], Pulse::default(), 1000);
        shared.queue_up("looker");
        runner.pump();
        let (mind, _) = shared.snapshot();
        assert!(mind.queue.is_empty(), "a watch waited in the queue");
        assert_eq!(mind.running, "");
        assert_eq!(
            mind.task("looker").map(|t| t.kind),
            Some(crate::task::Kind::Watch)
        );
    }

    #[test]
    fn the_run_gets_its_door_out_and_the_face_follows_at_once() {
        // WHAT A RUN FOUND is what its notices say — and they say it as the
        // run ends, not half a minute later.
        let found = Arc::new(AtomicUsize::new(0));
        let (runs, face) = (Arc::clone(&found), Arc::clone(&found));
        let (shared, mut runner) = a_runner(
            vec![
                Fake::routine("finder")
                    .runs(move |run| {
                        assert_eq!(run.ctx().module(), "finder");
                        assert_eq!(run.ctx().now(), 1000);
                        runs.store(3, Ordering::Relaxed);
                        Outcome::Done(Vec::new())
                    })
                    .face(move |_| crate::module::Face {
                        screen: u64::try_from(face.load(Ordering::Relaxed)).unwrap_or(0),
                        ..crate::module::Face::default()
                    })
                    .boxed(),
            ],
            Pulse::default(),
            1000,
        );
        shared.queue_up("finder");
        runner.pump();
        assert_eq!(
            shared
                .snapshot()
                .0
                .task("finder")
                .map(|t| t.screen_generation),
            Some(3)
        );
    }
}
