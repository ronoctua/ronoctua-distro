//! What the brain's three threads share: ONE registry, ONE journal, the mind,
//! and the door every module goes out through.
//!
//! # Three threads, one of everything
//!
//! - the RUNNER decides and runs — one thread, and the only one that blocks
//!   for long (a rebuild is an hour);
//! - the WATCH thread hears the journal, paths and children for the modules
//!   that listen, and must never wait for the runner — a process the kernel
//!   killed while the machine rebuilds is still worth a line;
//! - a thread per SOCKET client answers what the footer and the buffer ask.
//!
//! All three hold one `Arc<Core>`: the registry was built twice before this
//! (once for the runner, once for the socket), which was harmless while a task
//! held no state and is a defect the moment one does. The journal is behind a
//! lock of its own, never held across a call into a module, so a watcher can
//! write a record while the runner is blocked inside a run.
//!
//! # Why a module's calls into the core never wait on the core
//!
//! A module answers the core (`face`, `due`, `heard`…) and calls back into it
//! through its [`Ctx`] (`record`, `tell`, `changed`). If the core held a lock
//! across the first while the second waited on it, a module holding its own
//! state lock in `heard` and calling `record` could meet the runner holding
//! that lock and asking for the module's `face`. So the door out ONLY NOTES:
//! a record is put in the journal (whose lock nobody holds across a module
//! call) and the rest — the mind's copy, the faces — is published by whoever
//! called the module, AFTER it returned ([`Core::refresh`]).
//!
//! # Notices a person put away
//!
//! Dismissing or snoozing is answered HERE and not only by the module: a
//! notice put away is gone from the strip at once, whatever the module does
//! with the news ([`crate::module::Module::dismissed`]). A dismissed notice
//! stays away while it says the same thing since the same moment; a new
//! occurrence (a new `since`) is news again. A snoozed one comes back when its
//! time is up — the runner's wait is shortened to that moment, and no clock is
//! armed while nothing is snoozed.
//!
//! # A change made from outside the watch thread reaches it
//!
//! The watch thread waits on its channel with no clock unless a module asked
//! for a wake, so a wake armed by an action, or a watcher switched on by a
//! preference, would never be seen: nothing it waits on would move. Every
//! change of that kind RINGS it ([`Core::ring_on`]) — a module's `changed`,
//! any preference written — and it looks again: who is on now, and when each
//! wants waking.
//!
//! # Notes are written down at most once a second
//!
//! A run's record is written at once: it is the memory of when the module
//! last ran, and a restart that lost it would run it again. A NOTE is what a
//! watcher heard, and a burst of five hundred journal lines was five hundred
//! rewrites of the whole file. So a note is written at once only when the
//! file has not been written for [`SAVE_EVERY`]; the ones behind it wait for
//! [`Core::journal_due`], which the watch thread and the runner keep — and the
//! watch thread writes them before it writes the journal's cursor, so a note
//! is never behind a cursor that says it was heard.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::journal::{ACTIVITY, Journal, RECENT};
use crate::mind::{ActionState, Posted, SAID_KEPT, Said, Shared, Standing, StepState, TaskState};
use crate::module::{Act, Acted, Ctx, Event, Face, Look, Module, Noted, Screen, Sink};
use crate::moment::{Clock, Moment};
use crate::prefs::{Gate, Prefs, TaskPrefs, Where};
use crate::pulse::Pulse;
use crate::registry::{Registry, SYSTEM_OFF};
use crate::task::{Due, Hold, OutcomeKind, RunRecord};
use crate::wire::Knob;

/// How often, at most, the journal's file is rewritten while NOTES keep
/// coming. A run's record never waits. See this module's header.
pub const SAVE_EVERY: Duration = Duration::from_secs(1);

/// What is told that something may have changed — the watch thread, while it
/// runs.
pub type Bell = Arc<dyn Fn() + Send + Sync>;

/// Why an action was not done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The ask itself cannot be done — no such module or action, a row or an
    /// answer missing, no run to queue — in the daemon's words.
    Ask(String),
    /// The MODULE declined, and this is the text key of its reason
    /// (`<module>-why-<word>`, `brain-why-<word>`), for the door to translate.
    Key(String),
}

/// When the journal's file was last written, and whether notes wait.
#[derive(Debug, Default)]
struct Saving {
    pending: bool,
    last: Option<Instant>,
}

/// A notice a person put away, and until when.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PutAway {
    /// The `since` of the notice when it was put away: a notice with another
    /// is another occurrence, and is news.
    since: Moment,
    /// `None`: dismissed. `Some`: snoozed until then.
    until: Option<Moment>,
}

/// The brain's shared state. See this module's header.
pub struct Core {
    /// The mind every screen reads, behind its one lock and bell.
    pub shared: Arc<Shared>,
    modules: Registry,
    journal: Mutex<Journal>,
    store: Where,
    clock: Box<dyn Clock>,
    /// Held while faces and rows are computed AND written, so a later
    /// computation is never overwritten by an earlier one. Module code runs
    /// under it, but module code has no door that takes it.
    publishing: Mutex<()>,
    /// Each module's last face, before anything was put away — what a
    /// dismissal reads the notice's `since` from.
    faces: Mutex<BTreeMap<&'static str, Face>>,
    /// Modules that said their face changed, since the last refresh.
    dirty: Mutex<BTreeSet<&'static str>>,
    /// Whether a record was written since the mind's copy of the journal.
    written: AtomicBool,
    /// Notices a person put away, by (module, notice).
    put_away: Mutex<BTreeMap<(String, String), PutAway>>,
    /// Who is rung when something may have changed. See this module's header.
    bell: Mutex<Option<Bell>>,
    /// When the journal's file was written, and whether notes wait for it.
    saving: Mutex<Saving>,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core")
            .field("modules", &self.modules)
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

/// The lock, with a poisoned one taken anyway — the reasoning of
/// `Shared::locked`: a module that panicked must not take the brain with it.
fn held<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether this brain only rehearses: the development loop's second copy of
/// the system (`ronoctua_xdg::INSTANCE`) changes nothing on the machine.
#[must_use]
pub fn rehearsing_here() -> bool {
    ronoctua_xdg::instance().is_some()
}

impl Core {
    /// One, over a mind, a registry and the two directories, with the clock
    /// handed in. The journal is read from the state directory — or starts
    /// empty when there is none, which is what a test hands in.
    #[must_use]
    pub fn new(
        shared: Arc<Shared>,
        modules: Registry,
        store: Where,
        clock: Box<dyn Clock>,
    ) -> Self {
        let journal = store
            .state
            .as_deref()
            .map_or_else(Journal::fresh, Journal::read_from);
        Self {
            shared,
            modules,
            journal: Mutex::new(journal),
            store,
            clock,
            publishing: Mutex::new(()),
            faces: Mutex::new(BTreeMap::new()),
            dirty: Mutex::new(BTreeSet::new()),
            written: AtomicBool::new(false),
            put_away: Mutex::new(BTreeMap::new()),
            bell: Mutex::new(None),
            saving: Mutex::new(Saving::default()),
        }
    }

    /// The modules.
    #[must_use]
    pub const fn modules(&self) -> &Registry {
        &self.modules
    }

    /// Where the preferences and the journal live.
    #[must_use]
    pub const fn store(&self) -> &Where {
        &self.store
    }

    /// Now, by the daemon's clock.
    #[must_use]
    pub fn now(&self) -> Moment {
        self.clock.now()
    }

    /// The journal, locked. NEVER hold it across a call into a module.
    pub fn journal(&self) -> MutexGuard<'_, Journal> {
        held(&self.journal)
    }

    /// When this module last RAN, as the schedule counts it.
    #[must_use]
    pub fn last_run(&self, id: &str) -> Option<Moment> {
        self.journal().last(id)
    }

    /// The door out, for one module at one moment.
    #[must_use]
    pub fn ctx(self: &Arc<Self>, module: &'static str, now: Moment, rehearsing: bool) -> Ctx {
        let sink: Arc<dyn Sink> = Arc::clone(self) as Arc<dyn Sink>;
        Ctx::new(module, now, self.store.clone(), rehearsing, sink)
    }

    /// Writes one record down, on disk too — a NOTE at most once per
    /// [`SAVE_EVERY`], see this module's header. The mind's copy follows at
    /// the next [`Core::refresh`].
    pub fn put(&self, record: RunRecord) {
        let noted = record.kind == OutcomeKind::Noted;
        let mut journal = self.journal();
        journal.put(record);
        let mut saving = held(&self.saving);
        let now = Instant::now();
        if noted && saving.last.is_some_and(|at| now < at + SAVE_EVERY) {
            saving.pending = true;
        } else {
            self.write_journal(&journal, &mut saving, now);
        }
        drop(saving);
        drop(journal);
        self.written.store(true, Ordering::Release);
    }

    /// The file, now. Under both locks, journal first — the one order every
    /// caller takes them in.
    fn write_journal(&self, journal: &Journal, saving: &mut Saving, now: Instant) {
        saving.pending = false;
        saving.last = Some(now);
        if let Some(dir) = self.store.state.as_deref()
            && let Err(e) = journal.write_to(dir)
        {
            // Not fatal: a read-only home is not a reason to stop deciding.
            // It IS worth a line, because what is lost is the memory of when
            // this last ran — and the next start would run it again.
            tracing::error!(error = %e, "could not write the brain's journal");
        }
    }

    /// When the notes waiting should be written; `None` when none wait, which
    /// arms no clock anywhere.
    #[must_use]
    pub fn journal_due(&self) -> Option<Instant> {
        let saving = held(&self.saving);
        saving
            .pending
            .then(|| saving.last.map_or_else(Instant::now, |at| at + SAVE_EVERY))
    }

    /// Writes the notes waiting, if their moment has come.
    pub fn save_journal_if_due(&self, now: Instant) {
        if self.journal_due().is_some_and(|at| at <= now) {
            self.save_journal();
        }
    }

    /// Writes the notes waiting, now — before a cursor says they were heard,
    /// and on the way out.
    pub fn save_journal(&self) {
        let journal = self.journal();
        let mut saving = held(&self.saving);
        if saving.pending {
            self.write_journal(&journal, &mut saving, Instant::now());
        }
    }

    /// Says a module's face must be computed again at the next refresh — and
    /// rings the watch thread, whose modules' wakes may have moved.
    pub fn touch(&self, module: &'static str) {
        held(&self.dirty).insert(module);
        self.ring();
    }

    /// Whether something waits to be published: a module said its face
    /// changed, or a record was written since the mind's copy.
    #[must_use]
    pub fn stale(&self) -> bool {
        !held(&self.dirty).is_empty() || self.written.load(Ordering::Acquire)
    }

    /// Hangs the bell that is rung when something may have changed. One at a
    /// time; the last one hung is rung.
    pub fn ring_on(&self, bell: Bell) {
        *held(&self.bell) = Some(bell);
    }

    /// Takes this bell down — and only this one: a watch thread that ended
    /// after another was started must not silence the new one.
    pub fn ring_off(&self, bell: &Bell) {
        let mut hung = held(&self.bell);
        if hung.as_ref().is_some_and(|b| Arc::ptr_eq(b, bell)) {
            *hung = None;
        }
    }

    /// Rings the bell, if one is hung. Never under a lock of the core's: the
    /// bell only sends, and whoever hears it looks for itself.
    fn ring(&self) {
        let bell = held(&self.bell).clone();
        if let Some(bell) = bell {
            bell();
        }
    }

    // -----------------------------------------------------------------------
    // Publishing.
    // -----------------------------------------------------------------------

    /// Redraws every row and every face from what is true now: what the
    /// runner does on each pass, and the socket after a change of mind.
    ///
    /// One place, and not one per event: a row rebuilt at each site that
    /// changes something is a row somebody forgets to rebuild.
    pub fn publish(&self, now: Moment, pulse: Pulse) {
        let _one = held(&self.publishing);
        held(&self.dirty).clear();
        self.written.store(false, Ordering::Release);
        // The preferences are read ONCE and every row is built from that one
        // reading: a `Set` arriving in the middle must not produce a screen
        // where two rows disagree about what the person chose.
        let prefs = self.shared.snapshot().0.prefs;
        let (log, histories) = self.journal_copy();
        // WHEN EACH LAST RAN, from the whole journal and not from the few
        // records a row shows: a module held ten times in a row has its last
        // run further back than those ten, and a row computed from them would
        // draw a first run's moment.
        let lasts: BTreeMap<&'static str, Option<Moment>> = {
            let journal = self.journal();
            self.modules
                .ids()
                .map(|id| (id, journal.last(id)))
                .collect()
        };
        let faces = self.faces_now(now, &pulse, &prefs, &histories);
        let rows: Vec<(&dyn Module, TaskPrefs, Option<Due>)> = self
            .modules
            .all()
            .map(|m| {
                let id = m.manifest().id;
                let mut mine = prefs.of_module(m.manifest());
                // SWITCHED OFF BY THE SYSTEM: its row says so, with the
                // system's word, whatever the person's file says — and the
                // module is not asked when it would run.
                if self.modules.off(id) {
                    mine.enabled = false;
                    return (m, mine, Some(Due::Not { why: SYSTEM_OFF }));
                }
                let last = lasts.get(id).copied().flatten();
                let due = due_of(m, now, &mine, last);
                (m, mine, due)
            })
            .collect();
        let posted = self.posted(now, &prefs, &faces);
        self.shared.change(|mind| {
            mind.now = now;
            mind.pulse = pulse;
            mind.journal.clone_from(&log);
            let running = mind.running.clone();
            let queued: Vec<String> = mind.queue.clone();
            let old: Vec<TaskState> = std::mem::take(&mut mind.tasks);
            mind.tasks = rows
                .iter()
                .map(|(m, mine, due)| {
                    let id = m.manifest().id;
                    let was = old.iter().find(|t| t.id == id);
                    row_of(
                        *m,
                        mine,
                        due.as_ref(),
                        prefs.quiet,
                        &running,
                        &queued,
                        was.and_then(|t| t.hold),
                        histories.get(id).cloned().unwrap_or_default(),
                        faces.get(id).map_or(0, |f| f.screen),
                    )
                })
                .collect();
            posted.apply(mind);
        });
    }

    /// Publishes again what a module said changed, and the journal when a
    /// record was written — after a call into a module has RETURNED.
    ///
    /// Cheap when nothing happened: no module said so and no record was
    /// written, and it touches nothing.
    pub fn refresh(&self) {
        let _one = held(&self.publishing);
        let touched = !std::mem::take(&mut *held(&self.dirty)).is_empty();
        let written = self.written.swap(false, Ordering::AcqRel);
        if !touched && !written {
            return;
        }
        let now = self.now();
        let (mind, _) = self.shared.snapshot();
        let (log, histories) = self.journal_copy();
        let faces = self.faces_now(now, &mind.pulse, &mind.prefs, &histories);
        let posted = self.posted(now, &mind.prefs, &faces);
        self.shared.change(|m| {
            if written {
                m.journal.clone_from(&log);
            }
            for row in &mut m.tasks {
                if written {
                    let history = histories.get(row.id.as_str()).cloned().unwrap_or_default();
                    row.last = history.first().cloned();
                    row.history = history;
                }
                if let Some(f) = faces.get(row.id.as_str()) {
                    row.screen_generation = f.screen;
                }
            }
            posted.apply(m);
        });
    }

    /// The activity log the mind carries, and each module's own last few —
    /// copied out of the journal's lock, so no module is ever called under
    /// it.
    fn journal_copy(&self) -> (Vec<RunRecord>, BTreeMap<&'static str, Vec<RunRecord>>) {
        let journal = self.journal();
        // BOUNDED HERE, by `ACTIVITY`, and not on the way out: what a screen
        // is told and what the socket carries are the same list.
        let all = journal.all();
        let log = all[all.len().saturating_sub(ACTIVITY)..].to_vec();
        // AND THE LAST FEW OF EACH MODULE, asked per module: the journal is
        // one list, so on a machine where one runs weekly and another every
        // few hours the first one's records fall off the end of a shared
        // window while its own row still has to show them.
        let histories = self
            .modules
            .ids()
            .map(|id| (id, journal.recent(id, RECENT)))
            .collect();
        (log, histories)
    }

    /// Every module's face, computed now, and remembered unfiltered.
    fn faces_now(
        &self,
        now: Moment,
        pulse: &Pulse,
        prefs: &Prefs,
        histories: &BTreeMap<&'static str, Vec<RunRecord>>,
    ) -> BTreeMap<&'static str, Face> {
        let faces: BTreeMap<&'static str, Face> = self
            .modules
            .all()
            .map(|m| {
                let id = m.manifest().id;
                // NOT ASKED AT ALL when the system switched it off: its face
                // could read a machine whose half of it the flake took away.
                if self.modules.off(id) {
                    return (id, Face::default());
                }
                let mine = prefs.of_module(m.manifest());
                let look = Look {
                    now,
                    pulse,
                    mine: &mine,
                    last: histories.get(id).and_then(|h| h.first()),
                };
                (id, m.face(&look))
            })
            .collect();
        held(&self.faces).clone_from(&faces);
        faces
    }

    /// What the mind shows of the faces: the notices ranked, what a person
    /// put away left out, and nothing of a module switched off.
    fn posted(&self, now: Moment, prefs: &Prefs, faces: &BTreeMap<&'static str, Face>) -> Posting {
        let mut put_away = held(&self.put_away);
        // A PUT-AWAY THAT NO LONGER MEANS ANYTHING GOES: its notice is not up
        // any more, or it is up again as a new occurrence, or its snooze is
        // over. Kept, it would hide the next occurrence that happens to share
        // a `since`.
        put_away.retain(|(module, notice), away| {
            faces.get(module.as_str()).is_some_and(|f| {
                f.notices
                    .iter()
                    .any(|n| n.id == notice.as_str() && n.since == away.since)
            }) && away.until.is_none_or(|until| until > now)
        });
        let mut out = Posting::default();
        for m in self.modules.all() {
            let manifest = m.manifest();
            let id = manifest.id;
            // A MODULE SWITCHED OFF SHOWS NOTHING: its row says "off", and a
            // mark or a notice from it would be a module that is off and
            // still talking.
            if self.modules.off(id) || !prefs.of_module(manifest).enabled {
                continue;
            }
            let Some(face) = faces.get(id) else { continue };
            for mark in &face.marks {
                out.marks.push(Posted {
                    module: id.to_owned(),
                    what: mark.clone(),
                });
            }
            for notice in &face.notices {
                let away = put_away.get(&(id.to_owned(), notice.id.to_string()));
                if away.is_some_and(|a| a.since == notice.since) {
                    continue;
                }
                out.notices.push(Posted {
                    module: id.to_owned(),
                    what: notice.clone(),
                });
            }
            for item in &face.items {
                out.items.push(Posted {
                    module: id.to_owned(),
                    what: item.clone(),
                });
            }
        }
        // THE STRIP'S ORDER: the loudest first, and among equals the newest.
        out.notices.sort_by(|a, b| {
            b.what
                .level
                .cmp(&a.what.level)
                .then(b.what.since.cmp(&a.what.since))
        });
        out.items.sort_by_key(|i| i.what.at);
        out
    }

    /// The earliest moment something put away comes back, when there is one:
    /// the runner shortens its wait to it, and arms nothing otherwise.
    #[must_use]
    pub fn next_return(&self, now: Moment) -> Option<Moment> {
        held(&self.put_away)
            .values()
            .filter_map(|a| a.until)
            .filter(|u| *u > now)
            .min()
    }

    // -----------------------------------------------------------------------
    // What a person asks of a module.
    // -----------------------------------------------------------------------

    /// The module by this id, or the sentence that says there is none.
    ///
    /// # Errors
    ///
    /// No module answers to it; the sentence names those that do, because the
    /// caller is often somebody typing into `socat`.
    pub fn module(&self, id: &str) -> Result<&dyn Module, String> {
        self.modules.find(id).ok_or_else(|| {
            let known: Vec<&str> = self.modules.ids().collect();
            format!(
                "I do not know the task `{id}`; I know: {}",
                known.join(", ")
            )
        })
    }

    /// Refuses an ask about a module the SYSTEM switched off, with the key of
    /// the word its row gives — `brain-why-system-off`, which the door
    /// translates ([`Registry::switched_off`]).
    ///
    /// # Errors
    ///
    /// The module is switched off by the system.
    pub fn system_off(&self, m: &dyn Module) -> Result<(), String> {
        if self.modules.off(m.manifest().id) {
            Err(why_key(m, SYSTEM_OFF))
        } else {
            Ok(())
        }
    }

    /// Changes ONE preference of one module, over what it declared, and
    /// writes the preferences down.
    ///
    /// # Errors
    ///
    /// No such module, or a knob it has nothing to turn: a cadence for a
    /// module that keeps no calendar, a step it does not have, a cadence of
    /// zero days.
    pub fn set(&self, task: &str, knob: &Knob) -> Result<(), String> {
        let m = self.module(task)?;
        // NOT EVEN ITS OWN SWITCH: what the system configuration decided, the
        // configuration undoes — a person's "on" here would be a row saying
        // on over a module whose units are gone.
        self.system_off(m)?;
        let manifest = m.manifest();
        let calendar = manifest.schedule.is_some_and(|s| s.calendar);
        match knob {
            Knob::IntervalDays(_) | Knob::PartOfDay(_) if !calendar => {
                return Err(format!(
                    "`{task}` keeps no calendar, so it has no cadence or window to set"
                ));
            }
            Knob::IntervalDays(0) => {
                return Err("a cadence of zero days is not a cadence".to_owned());
            }
            Knob::Step { id, .. } if !m.steps().iter().any(|s| s.id == id) => {
                return Err(format!("`{task}` has no step called `{id}`"));
            }
            _ => {}
        }
        // WHAT THE STEP SHIPS AS is asked before the lock: nothing of a
        // module is called while the mind is held.
        let ships_on = match knob {
            Knob::Step { id, .. } => m
                .steps()
                .iter()
                .find(|s| s.id == id)
                .is_some_and(|s| s.on_by_default),
            _ => false,
        };
        self.shared.change(|mind| {
            let entry = mind.prefs.chosen_mut(manifest);
            match knob {
                Knob::IntervalDays(days) => entry.interval_days = *days,
                Knob::PartOfDay(part) => entry.part_of_day = *part,
                Knob::Enabled(on) => entry.enabled = *on,
                // A SET OF EXCEPTIONS, and the state the screen wants — not a
                // toggle: a key that repeated would land back where it
                // started. What is written is whether it differs from what
                // the distro ships.
                Knob::Step { id, on } => {
                    if *on == ships_on {
                        entry.steps_flipped.remove(id);
                    } else {
                        entry.steps_flipped.insert(id.clone());
                    }
                }
                Knob::ResetSteps => entry.steps_flipped.clear(),
            }
        });
        self.keep(Some(m));
        Ok(())
    }

    /// Flips one step, whatever it is now — the old `Toggle`, through `Set`.
    ///
    /// # Errors
    ///
    /// As [`Core::set`].
    pub fn toggle(&self, task: &str, step: &str) -> Result<(), String> {
        let m = self.module(task)?;
        let Some(declared) = m.steps().iter().find(|s| s.id == step) else {
            return Err(format!("`{task}` has no step called `{step}`"));
        };
        let prefs = self.shared.snapshot().0.prefs;
        let on_now =
            declared.on_by_default ^ prefs.of_module(m.manifest()).steps_flipped.contains(step);
        self.set(
            task,
            &Knob::Step {
                id: step.to_owned(),
                on: !on_now,
            },
        )
    }

    /// Replaces the general gate, and writes the preferences down.
    pub fn gate(&self, gate: Gate) {
        self.shared.change(|m| m.prefs.gate = gate);
        self.keep(None);
    }

    /// Writes the preferences down, and hands each module whose preferences
    /// may have moved its resolved view (`None`: all of them).
    ///
    /// Not fatal: a read-only home is not a reason for the brain to stop
    /// deciding. It is loud, because what is lost is a choice the person made.
    fn keep(&self, which: Option<&dyn Module>) {
        let prefs = self.shared.snapshot().0.prefs;
        if let Err(e) = self.store.save(&prefs) {
            tracing::error!(error = %e, "could not write the brain's preferences");
        }
        let all: Vec<&dyn Module> = which.map_or_else(|| self.modules.all().collect(), |m| vec![m]);
        for m in all {
            if let Err(e) = m.prefs_saved(&prefs.of_module(m.manifest()), &self.store) {
                tracing::error!(module = m.manifest().id, error = %e, "a module could not keep its view of the preferences");
            }
        }
        // A WATCHER SWITCHED ON OR OFF is the watch thread's news: it starts
        // the one that came on, and stops telling the one that went off.
        self.ring();
    }

    /// Does one of a module's actions.
    ///
    /// # Errors
    ///
    /// [`Refused::Ask`]: no such module or action, a row action with no row,
    /// an action that asks with no answer, a queue for a module with no run.
    /// [`Refused::Key`]: the module declined, with the TEXT KEY of its reason
    /// (`<module>-why-<word>`, or `brain-why-<word>` for a core word) — two
    /// cases and not one string, so the door knows which to translate.
    pub fn act(
        self: &Arc<Self>,
        module: &str,
        action: &str,
        row: Option<String>,
        answer: Option<String>,
    ) -> Result<(), Refused> {
        let m = self.module(module).map_err(Refused::Ask)?;
        self.system_off(m).map_err(Refused::Key)?;
        let manifest = m.manifest();
        let Some(declared) = manifest.action(action) else {
            return Err(Refused::Ask(format!(
                "`{module}` has no action called `{action}`"
            )));
        };
        if declared.row && row.is_none() {
            return Err(Refused::Ask(format!(
                "`{module}-{action}` acts on a row, and no row was given"
            )));
        }
        if declared.asks.is_some() && answer.as_deref().is_none_or(|a| a.trim().is_empty()) {
            return Err(Refused::Ask(format!(
                "`{module}-{action}` needs an answer, and none was given"
            )));
        }
        let ctx = self.ctx(manifest.id, self.now(), rehearsing_here());
        let acted = m.act(
            &Act {
                action: declared.id.to_owned(),
                row,
                answer,
            },
            &ctx,
        );
        self.touch(manifest.id);
        match acted {
            Acted::Done => Ok(()),
            Acted::Queue => {
                if m.scheduled().is_none() {
                    return Err(Refused::Ask(format!("`{module}` has no run to queue")));
                }
                self.shared.queue_up(manifest.id);
                Ok(())
            }
            Acted::Refused(word) => Err(Refused::Key(why_key(m, word))),
        }
    }

    /// Puts a notice away for good — until it is news again.
    ///
    /// # Errors
    ///
    /// No such module or notice, a notice that may not be dismissed, or one
    /// that is not up.
    pub fn dismiss(self: &Arc<Self>, module: &str, notice: &str) -> Result<(), String> {
        let (m, since) = self.showing(module, notice)?;
        let manifest = m.manifest();
        if !manifest.notice(notice).is_some_and(|n| n.dismiss) {
            return Err(format!(
                "the notice `{notice}` of `{module}` may not be dismissed"
            ));
        }
        held(&self.put_away).insert(
            (module.to_owned(), notice.to_owned()),
            PutAway { since, until: None },
        );
        let ctx = self.ctx(manifest.id, self.now(), rehearsing_here());
        m.dismissed(notice, &ctx);
        self.touch(manifest.id);
        Ok(())
    }

    /// Puts a notice off for one of the snoozes it offers.
    ///
    /// # Errors
    ///
    /// No such module or notice, a snooze it does not offer, or a notice that
    /// is not up.
    pub fn snooze(
        self: &Arc<Self>,
        module: &str,
        notice: &str,
        minutes: u16,
    ) -> Result<(), String> {
        let (m, since) = self.showing(module, notice)?;
        let manifest = m.manifest();
        if !manifest
            .notice(notice)
            .is_some_and(|n| n.snooze.contains(&minutes))
        {
            return Err(format!(
                "the notice `{notice}` of `{module}` does not offer {minutes} minutes"
            ));
        }
        let now = self.now();
        let until = now.saturating_add(i64::from(minutes) * 60);
        held(&self.put_away).insert(
            (module.to_owned(), notice.to_owned()),
            PutAway {
                since,
                until: Some(until),
            },
        );
        let ctx = self.ctx(manifest.id, now, rehearsing_here());
        m.snoozed(notice, until, &ctx);
        self.touch(manifest.id);
        Ok(())
    }

    /// The module, and the `since` of its notice as it is up now.
    fn showing(&self, module: &str, notice: &str) -> Result<(&dyn Module, Moment), String> {
        let m = self.module(module)?;
        self.system_off(m)?;
        if m.manifest().notice(notice).is_none() {
            return Err(format!("`{module}` declares no notice called `{notice}`"));
        }
        let since = held(&self.faces)
            .get(m.manifest().id)
            .and_then(|f| f.notices.iter().find(|n| n.id == notice))
            .map(|n| n.since);
        since
            .map(|s| (m, s))
            .ok_or_else(|| format!("the notice `{notice}` of `{module}` is not up"))
    }

    /// A module's own page, now, with the generation it is.
    ///
    /// Built only when asked — never carried in the mind — and from the same
    /// look its face gets, so the generation and the rows agree.
    ///
    /// # Errors
    ///
    /// No such module.
    pub fn screen(&self, module: &str) -> Result<(u64, Screen), String> {
        let m = self.module(module)?;
        // A PAGE OF NOTHING for a module the system switched off: its row
        // already says why, and its screen could read what the flake took.
        if self.modules.off(m.manifest().id) {
            return Ok((0, Screen::default()));
        }
        let (mind, _) = self.shared.snapshot();
        let mine = mind.prefs.of_module(m.manifest());
        let last = self.journal().latest(m.manifest().id).cloned();
        let look = Look {
            now: self.now(),
            pulse: &mind.pulse,
            mine: &mine,
            last: last.as_ref(),
        };
        Ok((m.face(&look).screen, m.screen(&look)))
    }

    /// Whether a module is switched on, by the preferences the mind holds.
    #[must_use]
    pub fn enabled(&self, m: &dyn Module) -> bool {
        !self.modules.off(m.manifest().id)
            && self
                .shared
                .snapshot()
                .0
                .prefs
                .of_module(m.manifest())
                .enabled
    }
}

/// When a module wants to run, as the core sees it: a watch has no answer, and
/// a module switched off is `off` — the core asks, so a module cannot forget
/// to.
#[must_use]
pub fn due_of(m: &dyn Module, now: Moment, mine: &TaskPrefs, last: Option<Moment>) -> Option<Due> {
    let s = m.scheduled()?;
    if !mine.enabled {
        return Some(Due::Not { why: "off" });
    }
    Some(s.due(now, mine, last))
}

/// The text key of a word a module gave: its own (`<id>-why-<word>`) when its
/// manifest declares the word, else the core's (`brain-why-<word>`).
#[must_use]
pub fn why_key(m: &dyn Module, word: &str) -> String {
    m.manifest()
        .why_key(word)
        .map_or_else(|| format!("brain-why-{word}"), str::to_owned)
}

/// One module's row, as everybody draws it.
#[allow(clippy::too_many_arguments)]
fn row_of(
    m: &dyn Module,
    mine: &TaskPrefs,
    due: Option<&Due>,
    quiet: bool,
    running: &str,
    queued: &[String],
    held_by: Option<Hold>,
    history: Vec<RunRecord>,
    screen_generation: u64,
) -> TaskState {
    let manifest = m.manifest();
    let id = manifest.id;
    let standing = if running == id {
        Standing::Running
    } else if queued.iter().any(|q| q == id) {
        Standing::Queued
    } else {
        Standing::Idle
    };
    // WHAT IS WAITING BEHIND SOMETHING ELSE SAYS SO. Without it a queued
    // module shows the hold of the last time the gate looked at it, which is
    // a reason that has nothing to do with why it is waiting now.
    let hold = if standing == Standing::Queued && !running.is_empty() {
        Some(Hold::Busy)
    } else {
        held_by
    };
    // A QUIET BRAIN SAYS SO instead of a moment it will not keep: the row of
    // something that would come due reads "nothing starts by itself".
    let (next, why_not) = match due {
        None if !mine.enabled => (0, "off"),
        None => (0, ""),
        Some(Due::Not { why }) => (0, *why),
        Some(Due::At(_) | Due::Now) if quiet => (0, "quiet"),
        Some(Due::At(at)) => (*at, ""),
        // NOW is "no moment to draw" too, told apart from not-at-all by the
        // empty word — a date on screen for something already overdue would
        // be a date in the past.
        Some(Due::Now) => (0, ""),
    };
    TaskState {
        id: id.to_owned(),
        kind: manifest.kind(),
        standing,
        enabled: mine.enabled,
        interval_days: mine.interval_days,
        part_of_day: mine.part_of_day,
        next,
        hold,
        why_not: why_not.to_owned(),
        last: history.first().cloned(),
        history,
        steps: m
            .steps()
            .iter()
            .map(|s| StepState {
                id: s.id.to_owned(),
                command: s.command.to_owned(),
                needs_root: s.needs_root,
                // ONE LINE FOR BOTH ANSWERS: `on` is the default flipped by
                // the person's exception — see `TaskPrefs::steps_flipped`.
                on: s.on_by_default ^ mine.steps_flipped.contains(s.id),
                on_by_default: s.on_by_default,
                name_key: s.name_key.to_owned(),
                does_key: s.does_key.to_owned(),
            })
            .collect(),
        calendar: manifest.schedule.is_some_and(|s| s.calendar),
        name_key: manifest.name_key.to_owned(),
        does_key: manifest.does_key.to_owned(),
        why_key: if why_not.is_empty() {
            String::new()
        } else {
            why_key(m, why_not)
        },
        screen_generation,
        actions: manifest.actions.iter().map(ActionState::from).collect(),
    }
}

/// What the mind shows of every face.
#[derive(Debug, Default)]
struct Posting {
    notices: Vec<Posted<crate::module::Notice>>,
    marks: Vec<Posted<crate::module::Mark>>,
    items: Vec<Posted<crate::module::Item>>,
}

impl Posting {
    /// Into the mind — the whole of it, never a delta.
    fn apply(self, mind: &mut crate::mind::Mind) {
        mind.notices = self.notices;
        mind.marks = self.marks;
        mind.items = self.items;
    }
}

impl Sink for Core {
    fn record(&self, module: &'static str, at: Moment, noted: Noted) {
        self.put(RunRecord::noted(module, at, noted));
    }

    fn tell(&self, module: &'static str, event: Event) {
        // A TAG NOBODY DECLARED IS REFUSED, and said: the footer names what it
        // says by the tag, and one it has no sentence for would be a line of
        // nothing — or a key in brackets.
        let declared = self
            .modules
            .find(module)
            .is_some_and(|m| m.manifest().events.iter().any(|e| e.tag == event.tag));
        if !declared {
            tracing::warn!(module, tag = %event.tag, "an event its manifest does not declare; not told");
            return;
        }
        self.shared.change(|m| {
            // NUMBERED INSIDE THE LOCK, so two threads telling at once cannot
            // put a later number before an earlier one.
            let seq = m.events.last().map_or(0, |s| s.seq) + 1;
            m.events.push(Said {
                seq,
                tag: event.tag,
                args: event.args,
            });
            if m.events.len() > SAID_KEPT {
                let over = m.events.len() - SAID_KEPT;
                m.events.drain(..over);
            }
        });
    }

    fn changed(&self, module: &'static str) {
        self.touch(module);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{Fake, Hand, ROUTINE, manifest};
    use crate::module::{
        Action, Arg, Cell, EventKind, Level, Manifest, Mark, MarkId, Notice, NoticeKind, Row,
        Schedule, Section, Tone, Why,
    };
    use crate::prefs::PartOfDay;
    use std::sync::atomic::AtomicU64;

    fn a_core(modules: Vec<Box<dyn Module>>, at: Moment) -> (Arc<Core>, Hand) {
        let clock = Hand::at(at);
        let core = Arc::new(Core::new(
            Arc::new(Shared::of(Prefs::default())),
            Registry::of(modules).expect("the registry"),
            Where::default(),
            Box::new(clock.clone()),
        ));
        (core, clock)
    }

    const DUE: NoticeKind = NoticeKind {
        id: "due",
        level: Level::Alarm,
        dismiss: true,
        snooze: &[5, 10],
    };
    const STAYS: NoticeKind = NoticeKind {
        id: "stays",
        level: Level::Warn,
        dismiss: false,
        snooze: &[],
    };
    const SAY: Action = Action {
        id: "say",
        command: "noisy-say",
        label: "cmd-noisy-say",
        row: false,
        asks: Some("ask-noisy-say"),
    };
    const FORGET: Action = Action {
        id: "forget",
        command: "noisy-forget",
        label: "cmd-noisy-forget",
        row: true,
        asks: None,
    };
    const RANG: EventKind = EventKind {
        name: "rang",
        tag: "noisy-rang",
    };
    static FULL: MarkId = MarkId::of("full", "noisy-full", "noisy-mark-full");

    /// What a test holds of the noisy module: when its alarm began, and every
    /// word it was told.
    struct Noisy {
        since: Arc<AtomicU64>,
        told: Arc<Mutex<Vec<String>>>,
    }

    /// A watch that rings an alarm since `since`, warns since now, marks the
    /// modeline, and whose screen grows a row with every word it is told.
    fn noisy() -> (Fake, Noisy) {
        let since = Arc::new(AtomicU64::new(100));
        let told = Arc::new(Mutex::new(Vec::<String>::new()));
        let (s, t, u) = (Arc::clone(&since), Arc::clone(&told), Arc::clone(&told));
        let fake = Fake::new(Manifest {
            schedule: None,
            notices: &[DUE, STAYS],
            actions: &[SAY, FORGET],
            events: &[RANG],
            marks: std::slice::from_ref(&FULL),
            whys: &[Why {
                word: "when-empty",
                key: "noisy-why-when-empty",
            }],
            ..manifest("noisy")
        })
        .face(move |look| Face {
            marks: vec![Mark {
                id: FULL.clone(),
                tone: Tone::Warning,
                count: 2,
                act: None,
            }],
            notices: vec![
                Notice::of(
                    &DUE,
                    "noisy-says-due",
                    i64::try_from(s.load(Ordering::Relaxed)).unwrap_or(0),
                )
                .control(&SAY),
                Notice::of(&STAYS, "noisy-says-stays", look.now),
            ],
            items: Vec::new(),
            screen: u64::try_from(t.lock().expect("told").len()).unwrap_or(0),
        })
        .screen(move |_| Screen {
            sections: vec![Section {
                heading: "noisy-heading".into(),
                rows: u
                    .lock()
                    .expect("told")
                    .iter()
                    .map(|w| Row {
                        id: w.clone(),
                        label: Cell::Data(w.clone()),
                        ..Row::default()
                    })
                    .collect(),
            }],
        });
        let words = Arc::clone(&told);
        let fake = fake.acts(move |act, ctx| match act.action.as_str() {
            "say" => {
                words
                    .lock()
                    .expect("told")
                    .push(act.answer.clone().unwrap_or_default());
                ctx.tell(Event::of(&RANG).with("n", Arg::Number(1)));
                ctx.record(Noted::of("noisy-noted"));
                ctx.changed();
                Acted::Done
            }
            "forget" => Acted::Refused("when-empty"),
            _ => Acted::Refused("unknown"),
        });
        (fake, Noisy { since, told })
    }

    /// A routine with a cadence of its own and one step that ships off.
    fn fussy() -> Fake {
        static STEPS: &[crate::task::Step] = &[crate::task::Step {
            id: "extra",
            command: "x",
            needs_root: false,
            on_by_default: false,
            name_key: "",
            does_key: "",
        }];
        Fake::new(Manifest {
            schedule: Some(Schedule {
                interval_days: 5,
                part_of_day: PartOfDay::Night,
                countdown_seconds: 0,
                runaway_minutes: 30,
                ..ROUTINE
            }),
            ..manifest("fussy")
        })
        .steps(STEPS)
        .due(|now, _, _| Due::At(now + 60))
    }

    #[test]
    fn switched_off_and_on_a_module_keeps_what_it_declared() {
        // THE DEFECT THIS CLOSES: the page switched a watch off and on again
        // by sending the whole preferences, built from the SHARED default —
        // and a six-hour watch came back weekly with a minute of countdown.
        // `Set` applies one knob over what the module declared.
        let (core, _) = a_core(vec![fussy().boxed()], 1000);
        core.set("fussy", &Knob::Enabled(false)).expect("off");
        core.publish(1000, Pulse::default());
        let (mind, _) = core.shared.snapshot();
        let row = mind.task("fussy").expect("row");
        assert!(!row.enabled);
        assert_eq!(row.why_not, "off");
        assert_eq!(row.why_key, "brain-why-off");
        core.set("fussy", &Knob::Enabled(true)).expect("on");
        core.publish(1000, Pulse::default());
        let (mind, _) = core.shared.snapshot();
        let row = mind.task("fussy").expect("row");
        assert!(row.enabled);
        assert_eq!((row.interval_days, row.part_of_day), (5, PartOfDay::Night));
        let m = core.modules().find("fussy").expect("module");
        let mine = mind.prefs.of_module(m.manifest());
        assert_eq!(
            mine.countdown_seconds, 0,
            "the shared minute of countdown came back"
        );
        assert_eq!(mine.runaway_minutes, 30);
        assert_eq!(row.next, 1060, "the row lost its moment");
        assert!(row.calendar);
        assert_eq!(row.name_key, "brain-task-fussy");
        assert_eq!(row.does_key, "brain-task-fussy-does");

        // AND A STEP IS SET BY THE STATE WANTED, not flipped: twice is once.
        core.set(
            "fussy",
            &Knob::Step {
                id: "extra".to_owned(),
                on: true,
            },
        )
        .expect("on");
        core.set(
            "fussy",
            &Knob::Step {
                id: "extra".to_owned(),
                on: true,
            },
        )
        .expect("on again");
        core.publish(1000, Pulse::default());
        let steps = core
            .shared
            .snapshot()
            .0
            .task("fussy")
            .expect("row")
            .steps
            .clone();
        assert!(steps[0].on && !steps[0].on_by_default);
        core.toggle("fussy", "extra").expect("toggle");
        core.publish(1000, Pulse::default());
        assert!(!core.shared.snapshot().0.task("fussy").expect("row").steps[0].on);
        core.set("fussy", &Knob::IntervalDays(14))
            .expect("fortnight");
        core.set("fussy", &Knob::ResetSteps).expect("reset");
        let prefs = core.shared.snapshot().0.prefs;
        assert_eq!(prefs.tasks["fussy"].interval_days, 14);
        assert!(prefs.tasks["fussy"].steps_flipped.is_empty());
    }

    #[test]
    fn a_knob_a_module_does_not_have_is_refused_with_a_sentence() {
        let (fake, _) = noisy();
        let (core, _) = a_core(vec![fussy().boxed(), fake.boxed()], 1000);
        for (task, knob, says) in [
            ("noisy", Knob::IntervalDays(3), "no calendar"),
            ("noisy", Knob::PartOfDay(PartOfDay::Night), "no calendar"),
            ("fussy", Knob::IntervalDays(0), "zero days"),
            (
                "fussy",
                Knob::Step {
                    id: "nope".to_owned(),
                    on: true,
                },
                "no step",
            ),
            ("nobody", Knob::Enabled(true), "fussy, noisy"),
        ] {
            let why = core.set(task, &knob).expect_err("it was accepted");
            assert!(why.contains(says), "{task} {knob:?}: {why}");
        }
        // A WATCH CAN STILL BE SWITCHED OFF: it has no calendar, and it can be
        // silenced.
        core.set("noisy", &Knob::Enabled(false)).expect("off");
        assert!(core.shared.snapshot().0.prefs.tasks.contains_key("noisy"));
        assert_eq!(
            core.shared.snapshot().0.prefs.tasks.len(),
            1,
            "a refusal wrote"
        );
    }

    #[test]
    fn notices_are_ranked_loudest_first() {
        let (fake, _) = noisy();
        let (core, _) = a_core(vec![fake.boxed()], 1000);
        core.publish(1000, Pulse::default());
        let (mind, _) = core.shared.snapshot();
        // THE ALARM FIRST, older as it is: loudness beats age.
        let ids: Vec<&str> = mind.notices.iter().map(|n| n.what.id.as_ref()).collect();
        assert_eq!(ids, ["due", "stays"]);
        assert_eq!(mind.notices[0].module, "noisy");
        assert_eq!(mind.marks.len(), 1);
        assert_eq!(mind.marks[0].what.id, FULL);
        // A WATCH'S ROW: no moment, no word, and its actions offered.
        let row = mind.task("noisy").expect("row");
        assert_eq!(row.kind, crate::task::Kind::Watch);
        assert_eq!((row.next, row.why_not.as_str()), (0, ""));
        assert!(!row.calendar);
        assert_eq!(
            row.actions
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>(),
            ["say", "forget"]
        );
        assert_eq!(row.actions[0].asks.as_deref(), Some("ask-noisy-say"));
    }

    #[test]
    fn a_module_switched_off_shows_nothing() {
        let (fake, _) = noisy();
        let (core, _) = a_core(vec![fake.boxed()], 1000);
        core.set("noisy", &Knob::Enabled(false)).expect("off");
        core.publish(1000, Pulse::default());
        let (mind, _) = core.shared.snapshot();
        assert!(mind.notices.is_empty() && mind.marks.is_empty());
        assert_eq!(mind.task("noisy").map(|t| t.why_not.as_str()), Some("off"));
    }

    #[test]
    fn a_module_the_system_switched_off_is_drawn_refusing_and_refuses_every_ask() {
        // THE FLAKE'S `enable = false` (`RONOCTUA_BRAIN_OFF`): the row stays,
        // saying why, and nothing else of the module happens — its face is
        // never asked, nothing it would show is shown, and every ask about it
        // is refused with the key of that same word, which the door
        // translates. A module that vanished from the page would read as one
        // that was never there.
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = Arc::clone(&asked);
        let (noisy, _) = noisy();
        let noisy = noisy.face(move |look| {
            counted.fetch_add(1, Ordering::Relaxed);
            Face {
                notices: vec![Notice::of(&STAYS, "noisy-says-stays", look.now)],
                ..Face::default()
            }
        });
        let core = Arc::new(Core::new(
            Arc::new(Shared::of(Prefs::default())),
            Registry::of(vec![fussy().boxed(), noisy.boxed()])
                .expect("the registry")
                .switched_off(&["noisy", "fussy"]),
            Where::default(),
            Box::new(Hand::at(1000)),
        ));
        core.publish(1000, Pulse::default());
        let (mind, _) = core.shared.snapshot();
        assert_eq!(asked.load(Ordering::Relaxed), 0, "its face was asked");
        assert!(mind.notices.is_empty() && mind.marks.is_empty());
        for id in ["noisy", "fussy"] {
            let row = mind.task(id).expect("the row left the page");
            assert!(!row.enabled, "{id} reads as running on its own");
            assert_eq!(row.why_not, SYSTEM_OFF, "{id}");
            assert_eq!(row.why_key, "brain-why-system-off", "{id}");
            assert_eq!(row.next, 0, "{id} drew a moment it will not keep");
        }
        let fussy = core.modules().find("fussy").expect("module");
        assert!(!core.enabled(fussy));

        // EVERY ASK, refused with the key — even the person's own switch.
        let key = "brain-why-system-off".to_owned();
        assert_eq!(core.set("fussy", &Knob::Enabled(true)), Err(key.clone()));
        assert_eq!(core.toggle("fussy", "extra"), Err(key.clone()));
        assert_eq!(
            core.act("noisy", "forget", Some("row".to_owned()), None),
            Err(Refused::Key(key.clone()))
        );
        assert_eq!(core.dismiss("noisy", "due"), Err(key.clone()));
        assert_eq!(core.snooze("noisy", "due", 5), Err(key));
        assert!(
            core.shared.snapshot().0.prefs.tasks.is_empty(),
            "a refusal wrote the preferences"
        );
        // AND ITS PAGE IS EMPTY, not a reading of what the flake took away.
        assert_eq!(core.screen("noisy"), Ok((0, Screen::default())));
        assert_eq!(asked.load(Ordering::Relaxed), 0, "its face was asked");
    }

    #[test]
    fn a_dismissed_notice_stays_away_until_it_is_news_again() {
        let (fake, noisy) = noisy();
        let fake = Arc::new(fake);
        let (core, _) = a_core(vec![Box::new(Borrowed(Arc::clone(&fake)))], 1000);
        core.publish(1000, Pulse::default());
        core.dismiss("noisy", "due").expect("dismiss");
        core.refresh();
        let (mind, _) = core.shared.snapshot();
        assert!(
            mind.notices.iter().all(|n| n.what.id != "due"),
            "a dismissed notice stayed on the strip"
        );
        assert_eq!(
            *fake.put_away.lock().expect("put away"),
            [("due".to_owned(), None)]
        );
        // STILL AWAY on the next pass, since it says the same since the same.
        core.publish(1030, Pulse::default());
        assert!(
            core.shared
                .snapshot()
                .0
                .notices
                .iter()
                .all(|n| n.what.id != "due")
        );
        // A NEW OCCURRENCE IS NEWS.
        noisy.since.store(1040, Ordering::Relaxed);
        core.publish(1060, Pulse::default());
        assert!(
            core.shared
                .snapshot()
                .0
                .notices
                .iter()
                .any(|n| n.what.id == "due")
        );
    }

    #[test]
    fn a_snoozed_notice_comes_back_when_its_time_is_up() {
        let (fake, _) = noisy();
        let fake = Arc::new(fake);
        let (core, clock) = a_core(vec![Box::new(Borrowed(Arc::clone(&fake)))], 1000);
        core.publish(1000, Pulse::default());
        assert_eq!(
            core.next_return(1000),
            None,
            "a clock armed with nothing snoozed"
        );
        core.snooze("noisy", "due", 10).expect("snooze");
        core.refresh();
        assert!(
            core.shared
                .snapshot()
                .0
                .notices
                .iter()
                .all(|n| n.what.id != "due")
        );
        assert_eq!(core.next_return(1000), Some(1600));
        assert_eq!(
            *fake.put_away.lock().expect("put away"),
            [("due".to_owned(), Some(1600))]
        );
        clock.set(1599);
        core.publish(1599, Pulse::default());
        assert!(
            core.shared
                .snapshot()
                .0
                .notices
                .iter()
                .all(|n| n.what.id != "due")
        );
        clock.set(1600);
        core.publish(1600, Pulse::default());
        assert!(
            core.shared
                .snapshot()
                .0
                .notices
                .iter()
                .any(|n| n.what.id == "due"),
            "a snooze that never ends"
        );
        assert_eq!(core.next_return(1600), None);
    }

    #[test]
    fn what_a_notice_does_not_offer_is_refused() {
        let (fake, _) = noisy();
        let (core, _) = a_core(vec![fake.boxed()], 1000);
        // NOT UP YET: nothing has been published.
        assert!(core.dismiss("noisy", "due").is_err());
        core.publish(1000, Pulse::default());
        assert!(
            core.dismiss("noisy", "stays")
                .expect_err("dismissed")
                .contains("may not")
        );
        assert!(
            core.snooze("noisy", "due", 7)
                .expect_err("snoozed")
                .contains("7 minutes")
        );
        assert!(core.snooze("noisy", "stays", 5).is_err());
        assert!(
            core.dismiss("noisy", "gone")
                .expect_err("gone")
                .contains("declares no")
        );
        assert!(core.dismiss("nobody", "due").is_err());
        assert_eq!(core.next_return(1000), None, "a refusal put something away");
    }

    #[test]
    fn an_act_is_done_queued_or_refused_with_its_key() {
        let (fake, noisy) = noisy();
        let queue = Fake::routine("queued")
            .acts(|_, _| Acted::Queue)
            .declaring(Manifest {
                actions: &[FORGET],
                ..manifest("queued")
            });
        let (core, _) = a_core(vec![fake.boxed(), queue.boxed()], 1000);
        core.publish(1000, Pulse::default());
        // AN ACTION THAT ASKS NEEDS ITS ANSWER; ONE ON A ROW NEEDS ITS ROW.
        assert!(core.act("noisy", "say", None, None).is_err());
        assert!(
            core.act("noisy", "say", None, Some("  ".to_owned()))
                .is_err()
        );
        assert!(core.act("noisy", "forget", None, None).is_err());
        assert!(core.act("noisy", "fly", None, None).is_err());
        assert!(
            noisy.told.lock().expect("told").is_empty(),
            "a refused act ran"
        );

        core.act("noisy", "say", None, Some("hello".to_owned()))
            .expect("done");
        assert_eq!(*noisy.told.lock().expect("told"), ["hello"]);
        // REFUSED BY THE MODULE: the key of its own word — told apart from a
        // refusal of the ask, which is a sentence and is not translated.
        assert_eq!(
            core.act("noisy", "forget", Some("r-1".to_owned()), None),
            Err(Refused::Key("noisy-why-when-empty".to_owned()))
        );
        assert!(matches!(
            core.act("noisy", "fly", None, None),
            Err(Refused::Ask(why)) if why.contains("no action")
        ));
        // QUEUE puts the module's run in the queue — the gate and the
        // countdown apply as to any run.
        core.act("queued", "forget", Some("r".to_owned()), None)
            .expect("queued");
        assert_eq!(core.shared.snapshot().0.queue, ["queued"]);
    }

    #[test]
    fn a_screen_is_asked_for_by_its_generation() {
        let (fake, _) = noisy();
        let (core, _) = a_core(vec![fake.boxed()], 1000);
        core.publish(1000, Pulse::default());
        assert_eq!(
            core.shared
                .snapshot()
                .0
                .task("noisy")
                .expect("row")
                .screen_generation,
            0
        );
        let (generation, screen) = core.screen("noisy").expect("screen");
        assert_eq!(generation, 0);
        assert!(screen.sections[0].rows.is_empty());
        // SOMETHING HAPPENED: the generation moves in the mind — which is how
        // a page knows to ask again — and the screen it asks for is the new.
        core.act("noisy", "say", None, Some("one".to_owned()))
            .expect("say");
        core.refresh();
        assert_eq!(
            core.shared
                .snapshot()
                .0
                .task("noisy")
                .expect("row")
                .screen_generation,
            1
        );
        let (generation, screen) = core.screen("noisy").expect("screen");
        assert_eq!(generation, 1);
        assert_eq!(screen.sections[0].rows[0].id, "one");
        // AND THE SCREEN IS NEVER IN THE MIND.
        let text = serde_json::to_string(&core.shared.snapshot().0).expect("encode");
        assert!(
            !text.contains("noisy-heading"),
            "a screen travelled in the mind"
        );
        assert!(core.screen("nobody").is_err());
    }

    #[test]
    fn what_a_module_records_and_tells_reaches_the_mind() {
        let (fake, _) = noisy();
        let (core, _) = a_core(vec![fake.boxed()], 1000);
        core.publish(1000, Pulse::default());
        core.act("noisy", "say", None, Some("x".to_owned()))
            .expect("say");
        core.refresh();
        let (mind, _) = core.shared.snapshot();
        // THE RECORD: in the log and on the row, and it moves no schedule.
        assert_eq!(mind.journal.len(), 1);
        assert_eq!(mind.journal[0].kind, crate::task::OutcomeKind::Noted);
        assert_eq!(mind.journal[0].says, "noisy-noted");
        let row = mind.task("noisy").expect("row");
        assert_eq!(
            row.last.as_ref().map(|r| r.says.as_str()),
            Some("noisy-noted")
        );
        assert_eq!(core.last_run("noisy"), None, "a note counted as a run");
        // THE EVENT: numbered, once.
        assert_eq!(mind.events.len(), 1);
        assert_eq!(mind.events[0].seq, 1);
        assert_eq!(mind.events[0].tag, "noisy-rang");
    }

    #[test]
    fn what_a_module_notes_again_and_again_is_one_row_of_the_activity() {
        // THE GROUPED ACTIVITY is `journal::grouped` over the log the mind
        // carries: three notes in a row that say the same thing are one row
        // that opens into three.
        let (fake, _) = noisy();
        let (core, _) = a_core(vec![fake.boxed(), fussy().boxed()], 1000);
        for word in ["a", "b", "c"] {
            core.act("noisy", "say", None, Some(word.to_owned()))
                .expect("say");
        }
        core.put(RunRecord::of(
            "fussy",
            1001,
            1002,
            crate::task::Outcome::Done(Vec::new()),
        ));
        core.act("noisy", "say", None, Some("d".to_owned()))
            .expect("say");
        core.refresh();
        let (mind, _) = core.shared.snapshot();
        let rows = crate::journal::grouped(&mind.journal);
        assert_eq!(
            rows.iter()
                .map(|g| (g.newest.task.as_str(), g.repeats))
                .collect::<Vec<_>>(),
            [("noisy", 1), ("fussy", 1), ("noisy", 3)],
            "a fold crossed another module's record, or did not fold"
        );
        assert_eq!(rows[2].members.len(), 3);
    }

    #[test]
    fn a_row_counts_its_last_run_from_the_whole_journal() {
        // A module held more times than its row shows still ran when it ran:
        // its moment is counted from that run, not from nothing.
        let (core, _) = a_core(
            vec![
                fussy()
                    .due(|_, _, last| match last {
                        Some(at) => Due::At(at + 7),
                        None => Due::Now,
                    })
                    .boxed(),
            ],
            1000,
        );
        core.put(RunRecord::of(
            "fussy",
            500,
            501,
            crate::task::Outcome::Done(Vec::new()),
        ));
        for n in 0..(crate::journal::RECENT as i64 + 2) {
            core.put(RunRecord {
                kind: crate::task::OutcomeKind::Held,
                ..RunRecord::of(
                    "fussy",
                    600 + n,
                    600 + n,
                    crate::task::Outcome::Skipped {
                        why: "ram".to_owned(),
                    },
                )
            });
        }
        core.publish(1000, Pulse::default());
        assert_eq!(
            core.shared.snapshot().0.task("fussy").map(|t| t.next),
            Some(507)
        );
    }

    #[test]
    fn the_events_ring_is_bounded_and_refuses_a_tag_nobody_declared() {
        let (fake, _) = noisy();
        let (core, _) = a_core(vec![fake.boxed()], 1000);
        for _ in 0..(SAID_KEPT + 5) {
            core.tell("noisy", Event::of(&RANG));
        }
        let (mind, _) = core.shared.snapshot();
        assert_eq!(mind.events.len(), SAID_KEPT);
        assert_eq!(
            mind.events.last().map(|s| s.seq),
            Some(SAID_KEPT as u64 + 5)
        );
        assert_eq!(mind.events[0].seq, 6, "the ring dropped from the wrong end");
        core.tell(
            "noisy",
            Event {
                tag: "noisy-invented".into(),
                args: Vec::new(),
            },
        );
        assert_eq!(
            core.shared
                .snapshot()
                .0
                .events
                .last()
                .map(|s| s.tag.to_string()),
            Some("noisy-rang".to_owned()),
            "an undeclared tag was told"
        );
    }

    #[test]
    fn notes_are_written_down_at_most_so_often_and_a_run_at_once() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-brain-hub-saving-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let core = Arc::new(Core::new(
            Arc::new(Shared::of(Prefs::default())),
            Registry::of(vec![fussy().boxed()]).expect("the registry"),
            Where {
                config: None,
                state: Some(dir.clone()),
            },
            Box::new(Hand::at(1000)),
        ));
        let on_disk = || Journal::read_from(&dir).runs.len();
        let note = |n: i64| RunRecord::noted("fussy", 1000 + n, Noted::of("fussy-noted"));
        // THE FIRST NOTE IS WRITTEN AT ONCE: nothing was written lately.
        core.put(note(0));
        assert_eq!(on_disk(), 1);
        assert_eq!(core.journal_due(), None);
        // A BURST WAITS its turn — five hundred lines are not five hundred
        // rewrites of the file.
        for n in 1..500 {
            core.put(note(n));
        }
        assert_eq!(on_disk(), 1, "a burst of notes rewrote the file each time");
        let due = core.journal_due().expect("the notes waiting arm a clock");
        assert!(due > Instant::now() && due <= Instant::now() + SAVE_EVERY);
        core.save_journal_if_due(Instant::now());
        assert_eq!(on_disk(), 1, "written before its moment");
        core.save_journal_if_due(due);
        assert_eq!(on_disk(), crate::journal::HISTORY);
        assert_eq!(core.journal_due(), None);
        // A RUN NEVER WAITS: it is the memory of when the module last ran.
        core.put(note(500));
        core.put(RunRecord::of(
            "fussy",
            2000,
            2001,
            crate::task::Outcome::Done(Vec::new()),
        ));
        assert_eq!(Journal::read_from(&dir).last("fussy"), Some(2000));
        assert_eq!(core.journal_due(), None, "the run's write took the notes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_change_rings_the_bell_and_only_the_bell_hung_is_taken_down() {
        let (core, _) = a_core(vec![fussy().boxed()], 1000);
        let rung = Arc::new(AtomicU64::new(0));
        let counts = Arc::clone(&rung);
        let bell: Bell = Arc::new(move || {
            counts.fetch_add(1, Ordering::Relaxed);
        });
        core.ring_on(Arc::clone(&bell));
        core.touch("fussy");
        core.set("fussy", &Knob::Enabled(false)).expect("off");
        core.gate(Gate::default());
        assert_eq!(rung.load(Ordering::Relaxed), 3);
        // ANOTHER BELL hung since: taking the first down leaves it.
        let other: Bell = Arc::new(|| {});
        core.ring_on(Arc::clone(&other));
        core.ring_off(&bell);
        core.touch("fussy");
        assert_eq!(rung.load(Ordering::Relaxed), 3, "the old bell was rung");
        assert!(held(&core.bell).is_some(), "the new bell was taken down");
        core.ring_off(&other);
        assert!(held(&core.bell).is_none());
    }

    #[test]
    fn a_quiet_brain_says_so_instead_of_a_moment() {
        let (core, _) = a_core(vec![fussy().boxed()], 1000);
        core.shared.change(|m| m.prefs.quiet = true);
        core.publish(1000, Pulse::default());
        let row = core
            .shared
            .snapshot()
            .0
            .task("fussy")
            .cloned()
            .expect("row");
        assert_eq!((row.next, row.why_not.as_str()), (0, "quiet"));
        assert_eq!(row.why_key, "brain-why-quiet");
    }

    /// One module shared by the test and the registry.
    struct Borrowed(Arc<Fake>);

    impl Module for Borrowed {
        fn manifest(&self) -> &'static Manifest {
            self.0.manifest()
        }
        fn face(&self, look: &Look<'_>) -> Face {
            Module::face(&*self.0, look)
        }
        fn dismissed(&self, notice: &str, ctx: &Ctx) {
            self.0.dismissed(notice, ctx);
        }
        fn snoozed(&self, notice: &str, until: Moment, ctx: &Ctx) {
            self.0.snoozed(notice, until, ctx);
        }
    }
}
