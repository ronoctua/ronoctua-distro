//! The system update — the brain's first feature, and the shape of every one
//! after it.
//!
//! # What it does, exactly
//!
//! At the right moment, it asks systemd to start one unit and then reads what
//! the unit reported. That is all of it. What an update IS — which inputs,
//! which command, in which order — is declared beside this file (`steps.nix`)
//! and stays there (invariant 5): this file names a unit and a table of steps
//! for the screen to draw, and neither of them is a script. The whole story is
//! in `why.md`, beside this.
//!
//! # Why the steps are drawn at all
//!
//! Because "the machine will update itself this afternoon" is a sentence a
//! person should be able to open. Under the cursor the screen says the real
//! command, whether it needs root, and whether the person has changed it from
//! what the distro ships — the same rule the disk formatter's two menus follow,
//! where every line carries a second, quieter line saying what it really means.
//!
//! # The rebuild is borrowed
//!
//! The step that applies the configuration runs the machine's own apply
//! (`ronoctua-machine`, which the rescue console runs too) rather than
//! declaring a second `nixos-rebuild` (`steps.nix` says why), and the trim of
//! the system's generations is the store's. This module owns neither; it
//! owns WHEN, and what the screen says about it.
//!
//! # A scheduled run waits, and never asks
//!
//! On a metered way out, or on a battery below the sequence's floor, the run
//! is HELD — the row says why (`brain-hold-network`, `brain-hold-battery`) —
//! and it goes when the machine is plugged in or on another network. A run
//! started by hand is asked about by the door that started it, with the no
//! first: below the floor before anything starts ([`Charge::below_the_floor`]),
//! and on a metered way out when this module holds the run the door asked
//! for ([`HOLD_METERED`]). Nothing here asks.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::module::{Birth, Manifest, Module, Run, Scheduled};
use crate::moment::{Moment, Offset};
use crate::prefs::{TaskPrefs, Where};
use crate::pulse::Pulse;
use crate::schedule::next_moment;
use crate::systemd;
use crate::task::{Due, Hold, Outcome, Step, StepOutcome, Verdict, Watched};

/// Why the machine's own program refused a step (`ronoctua-machine`, whose
/// last line is `refused <word>`): the report keeps the word
/// (`failed-<word>`), and the view says the machine's sentence for it.
pub use ronoctua_machine::Why as MachineRefusal;
/// The boot trial's outcome and why an apply armed none, as the machine's
/// record says them — named here so the update's view reads them through
/// this module.
pub use ronoctua_machine::{NoTrial, TrialOutcome};
/// Why the verification held a declared program back.
pub use ronoctua_person::local::HeldWhy;

// The declared sequence, generated from `steps.nix` by `gen-updates.py`
// (`tables.nix`), with its own types.
//
// # Why this is included and not written
//
// Because it was written, and the two copies disagreed. The brain used to
// carry three steps of its own — `update-inputs`, `build`,
// `collect-garbage` — with their own command strings, while the table the
// unit actually runs declared twelve, named `flake-update`,
// `system-rebuild`, `store-collect-garbage` and so on. The ids were not a
// subset of each other: they were disjoint, and every step drawn on the page
// carried a name the sequence does not use. It is one table now, and the
// drift is not a defect that got fixed, it is a defect that has nowhere left
// to live.
use super::tables::updates as declared;

/// The file the sequence's own script reads to find out which steps the
/// person switched, in their state directory:
/// `$XDG_STATE_HOME/ronoctua/update-steps.conf`.
///
/// The script (`gen-updates.py`, the person's half) spells the same leaf, and
/// this module's check `…-switch-reaches-the-run` plants a file here and asks
/// the script's own reader about it.
pub const STEPS_FILE: &str = "update-steps.conf";

/// The entry: the update, in the birth's zone — [`Scheduled::due`] is PURE, and
/// "in the afternoon" is an hour in the person's own day.
#[must_use]
pub fn new(birth: &Birth) -> impl Module + use<> {
    SystemUpdate {
        zone: Arc::clone(&birth.zone),
        probe: Box::new(TheMachine {
            places: ronoctua_memory::Places::default(),
        }),
    }
}

/// The hold a run gets on a METERED way out — this module's alone (the
/// general gate holds nothing for the network), so a door that asked for a
/// run by hand and sees its row held by it knows the brain is waiting for
/// another network, and asks the person whether to spend this one.
pub const HOLD_METERED: Hold = Hold::Network;

/// What a run of the update waits on, read off the machine — a seam, so a
/// test is a metered network or a flat battery without having one.
trait Probe: Send + Sync {
    /// Whether the way out is metered; `None` when nobody could say.
    fn metered(&self) -> Option<bool>;
    /// The machine's battery, as the update weighs it.
    fn charge(&self) -> Charge;
}

/// The real machine: NetworkManager, and `/sys` under these places.
struct TheMachine {
    places: ronoctua_memory::Places,
}

impl Probe for TheMachine {
    fn metered(&self) -> Option<bool> {
        ronoctua_network::Manager::fresh().ok()?.metered()
    }

    fn charge(&self) -> Charge {
        Charge::read_at(&self.places)
    }
}

/// The sequence's battery floor, in per cent — declared once, in
/// `steps.nix`, on the steps that compile. `None` when no step needs power.
#[must_use]
pub const fn battery_floor() -> Option<u8> {
    declared::BATTERY_FLOOR_PERCENT
}

/// The battery, as the update weighs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Charge {
    /// Running off the battery now.
    pub discharging: bool,
    /// How charged it is, in per cent; `None` when no battery says.
    pub percent: Option<u8>,
}

impl Charge {
    /// The machine's, now: two reads of `/sys` through the crate that is the
    /// border with it — microseconds, so a door may ask on the loop that
    /// draws.
    #[must_use]
    pub fn read() -> Self {
        Self::read_at(&ronoctua_memory::Places::default())
    }

    /// The batteries under `places` — the machine's own, both readings over
    /// the same supplies (`ronoctua_memory::machine::machine_batteries`): a
    /// mouse's battery, always discharging, is neither the machine on
    /// battery nor a charge. The module's hold and the doors' question both
    /// come through here, so they cannot disagree about one machine.
    #[must_use]
    pub fn read_at(places: &ronoctua_memory::Places) -> Self {
        Self {
            discharging: ronoctua_memory::machine::on_battery(places) == Some(true),
            percent: ronoctua_memory::machine::battery_percent(places),
        }
    }

    /// Whether it is below the sequence's floor: running off the battery,
    /// and charged less than the floor — or by an amount nobody can read,
    /// which a machine about to compile must not take for enough. A run
    /// started here asks first, with the no first; a scheduled one waits.
    #[must_use]
    pub fn below_the_floor(self) -> bool {
        let Some(floor) = battery_floor() else {
            return false;
        };
        self.discharging && self.percent.is_none_or(|p| p < floor)
    }
}

/// What a run waits on, or `None` to go: a METERED way out first — a
/// rebuild downloads gigabytes, and a phone's hotspot is paid by the byte —
/// then a battery below the floor, read as the doors read it
/// ([`Charge::read_at`]).
fn waits(probe: &dyn Probe) -> Option<Hold> {
    if probe.metered() == Some(true) {
        return Some(HOLD_METERED);
    }
    probe.charge().below_the_floor().then_some(Hold::Battery)
}

/// The key of a refusal a step reads as news (`declined-<word>` in its
/// report), or `None` for a word no step declares.
#[must_use]
pub fn declined_key(word: &str) -> Option<&'static str> {
    declared::declined_key(word)
}

/// A declared program the verification held back, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The declared entry's id (`<source>:<id>`).
    pub id: String,
    /// Why it was held.
    pub why: HeldWhy,
}

/// The revision the machine is held at, and since when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldAt {
    /// The distro's revision, in hex.
    pub rev: String,
    /// Since when, `YYYY-MM-DDTHH:MM:SSZ`.
    pub since: String,
}

/// What the machine's own records say beside a run — what the update's
/// view draws above its steps, in both doors. Each is read through the
/// crate that writes it; an absent file is an empty answer (a machine that
/// never updated), and one that does not read says nothing rather than
/// something invented.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Records {
    /// How many plain files of the belongings the last pull left as this
    /// machine has them, changed here and elsewhere.
    pub conflicts: usize,
    /// The declared programs the verification held back.
    pub held: Vec<Held>,
    /// The boot trial, when one was armed.
    pub trial: Option<TrialOutcome>,
    /// Why the last apply armed no trial.
    pub no_trial: Option<NoTrial>,
    /// The revision the machine is held at.
    pub hold: Option<HeldAt>,
}

/// Where the records are — parameters, so a test and a photograph read
/// folders of their own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordPlaces {
    /// The machine directory (its `local/held.json`).
    pub machine_dir: PathBuf,
    /// The folder of the machine's record (`state.json`).
    pub machine_state: PathBuf,
    /// The person's belongings' places; `None` with no home.
    pub belongings: Option<ronoctua_belongings::places::Places>,
}

impl RecordPlaces {
    /// This machine's.
    #[must_use]
    pub fn here() -> Self {
        Self {
            machine_dir: PathBuf::from(env!("RONOCTUA_MACHINE_DIR")),
            machine_state: PathBuf::from(ronoctua_machine::STATE_DIR),
            belongings: ronoctua_belongings::places::Places::from_env(),
        }
    }

    /// Every file [`Records::read_at`] reads — what a door watches, to read
    /// them again only when one moved.
    #[must_use]
    pub fn files(&self) -> Vec<PathBuf> {
        let mut out = vec![
            self.machine_state.join(ronoctua_person::names::STATE),
            self.machine_dir.join(ronoctua_person::names::HELD),
        ];
        if let Some(b) = &self.belongings {
            out.extend([b.last(), b.prefs()]);
        }
        out
    }
}

impl Records {
    /// This machine's records.
    #[must_use]
    pub fn read() -> Self {
        Self::read_at(&RecordPlaces::here())
    }

    /// The records under `at`.
    #[must_use]
    pub fn read_at(at: &RecordPlaces) -> Self {
        let state = ronoctua_machine::state(&ronoctua_machine::Places {
            dir: at.machine_dir.clone(),
            state: at.machine_state.clone(),
            run: PathBuf::new(),
        })
        .ok();
        let held = std::fs::read(at.machine_dir.join(ronoctua_person::names::HELD))
            .ok()
            .and_then(|b| ronoctua_person::parse::<ronoctua_person::local::Held>(&b).ok())
            .map(|h| {
                h.held
                    .into_iter()
                    .map(|e| Held {
                        id: e.id,
                        why: e.reason,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            conflicts: at.belongings.as_ref().map_or(0, plain_conflicts),
            held,
            trial: state
                .as_ref()
                .and_then(|s| s.trial.as_ref())
                .map(|t| t.outcome),
            no_trial: state.as_ref().and_then(|s| s.no_trial),
            hold: state.and_then(|s| s.hold).map(|h| HeldAt {
                rev: h.rev,
                since: h.since.as_str().to_owned(),
            }),
        }
    }
}

/// How many plain files the last plain run left in conflict — of the
/// repository the person names now: a base of another one is a first run's.
fn plain_conflicts(places: &ronoctua_belongings::places::Places) -> usize {
    let Some(prefs) = ronoctua_store::Store::<ronoctua_belongings::prefs::Belongings>::at(
        Some(places.config.clone()),
        ronoctua_belongings::prefs::FILE,
    )
    .whole() else {
        return 0;
    };
    ronoctua_belongings::status::Last::read(&places.last())
        .and_then(|l| l.plain.of(&prefs.repository).map(|p| p.conflicts.len()))
        .unwrap_or(0)
}

/// What the update does, in order, with the keys its module declared.
///
/// The `command` is DATA — it is drawn as it is written, because a command line
/// is not translated and inventing a friendly rendering of one is how a screen
/// ends up lying about what it is running.
///
/// It is built from the generated table at first use rather than being a
/// `static`: the table's `command` is a slice of arguments and this one is the
/// line a person reads. The two keys come from the table, which the generator
/// derived from each id — nobody downstream builds one.
pub fn steps() -> &'static [Step] {
    use std::sync::OnceLock;
    static ONCE: OnceLock<Vec<Step>> = OnceLock::new();
    ONCE.get_or_init(|| {
        declared::STEPS
            .iter()
            .map(|d| Step {
                id: d.id,
                // JOINED HERE and not in the generator: the generator's job is
                // the argv, which is what the runner execs. What a person reads
                // is a sentence made of it, and the two must not become two
                // declarations.
                command: Box::leak(d.command.join(" ").into_boxed_str()),
                needs_root: d.root,
                on_by_default: d.enabled,
                name_key: d.name_key,
                does_key: d.does_key,
            })
            .collect()
    })
}

/// The key that says WHICH precondition kept a step from running, by the word
/// the runner wrote into its report (`skipped-<word>`), or `None` for a word
/// the sequence does not have.
#[must_use]
pub fn needs_key(word: &str) -> Option<&'static str> {
    declared::Needs::key_of_word(word)
}

/// The system update.
struct SystemUpdate {
    /// How far ahead of UTC this machine is — handed in, so
    /// [`Scheduled::due`] stays pure.
    zone: Arc<dyn Offset>,
    /// Whether the way out is metered and how charged the battery is.
    probe: Box<dyn Probe>,
}

impl Module for SystemUpdate {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn steps(&self) -> &'static [Step] {
        steps()
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }

    /// The person's step switches, where the update's own script reads them
    /// ([`STEPS_FILE`]) — see [`write_steps_to`].
    fn prefs_saved(&self, mine: &TaskPrefs, dirs: &Where) -> std::io::Result<()> {
        dirs.state.as_deref().map_or(Ok(()), |d| {
            write_steps_to(d, super::ID, &mine.steps_flipped)
        })
    }
}

impl Scheduled for SystemUpdate {
    fn due(&self, now: Moment, mine: &TaskPrefs, last: Option<Moment>) -> Due {
        // A BUILD THAT CANNOT ASK is not a task that is late — it is a task
        // that cannot happen, and the screen has to say which. Offering a
        // moment here would put a date on screen that nothing will ever
        // honour; this way the row says `brain-why-no-systemctl` instead.
        if !systemd::reachable() {
            return Due::Not {
                why: "no-systemctl",
            };
        }
        let at = next_moment(now, last, mine, self.zone.as_ref());
        if at <= now { Due::Now } else { Due::At(at) }
    }

    /// A METERED WAY OUT, OR A BATTERY BELOW THE FLOOR: held, and said on
    /// the row — never asked. The general gate (busy, on battery at all) is
    /// asked by the runner first and configured by the person in one place;
    /// these two are the update's own, and hold even a gate the person
    /// loosened: a rebuild downloads gigabytes and compiles for twenty
    /// minutes. The battery is read here, as the doors read it, and not
    /// taken from the pulse: one reading of one machine for the hold and
    /// for the question.
    fn needs(&self, _pulse: &Pulse, _mine: &TaskPrefs) -> Verdict {
        waits(self.probe.as_ref()).map_or(Verdict::Go, Verdict::Hold)
    }

    fn run(&self, run: &Run) -> Outcome {
        let unit = if run.is_rehearsal() {
            super::unit::REHEARSE
        } else {
            super::unit::RUN
        };
        // WHATEVER IS THERE NOW BELONGS TO THE PREVIOUS RUN, and the mark is
        // taken BEFORE the unit is asked to start. Without it the first look
        // finds the old file, sees that it ended, and reports a run that
        // finished before it began.
        let before = run.mark();
        if let Err(e) = systemd::start(unit) {
            return Outcome::Failed {
                at: String::new(),
                // A `Missing` carries the VARIABLE's name, which is the fix,
                // and a refusal carries what polkit said. Both are sentences
                // from the machine, so both go through as text.
                why: e.to_string(),
            };
        }
        match run.watch(before) {
            Watched::Ended(sheet) => ended(sheet.steps, &sheet.trouble),
            Watched::Runaway(sheet) => Outcome::Failed {
                at: last_step(&sheet.steps),
                // A WORD of ours, because this is the brain's own conclusion
                // and not the machine's: the unit may perfectly well still be
                // going. What is known is that we stopped hearing.
                why: "runaway".to_owned(),
            },
            Watched::Aborted(sheet) => {
                // GIVING UP IS WHAT STOPS IT. Dropping the watch would leave
                // the rebuild running with nothing on screen saying so.
                if let Err(e) = systemd::stop(unit) {
                    tracing::warn!(unit, error = %e, "could not stop the unit");
                }
                Outcome::Failed {
                    at: last_step(&sheet.steps),
                    why: "aborted".to_owned(),
                }
            }
        }
    }
}

/// A finished run, as its own report describes it.
fn ended(steps: Vec<StepOutcome>, trouble: &str) -> Outcome {
    if let Some(bad) = steps.iter().find(|s| !s.ok) {
        return Outcome::Failed {
            at: bad.id.clone(),
            // What the step said, when it said anything; the unit's own summary
            // otherwise. Empty on both is still a failure, and the screen draws
            // the step's name — which is more than "something went wrong".
            why: if bad.trouble.is_empty() {
                trouble.to_owned()
            } else {
                bad.trouble.clone()
            },
        };
    }
    if !trouble.is_empty() {
        return Outcome::Failed {
            at: String::new(),
            why: trouble.to_owned(),
        };
    }
    Outcome::Done(steps)
}

/// The last step that was reported — where a run that went quiet was standing.
fn last_step(steps: &[StepOutcome]) -> String {
    steps.last().map(|s| s.id.clone()).unwrap_or_default()
}

/// The person's step switches, in the shape the sequence's script already
/// reads.
///
/// # Why this file exists at all
///
/// Because the switch on the screen did **nothing**. `m t` wrote an entry into
/// `brain.json` and nothing on the machine ever read it: what decides whether
/// a step runs is `/etc/ronoctua/updates.conf`, generated from the NixOS
/// option `ronoctua.update.steps`, which needs a rebuild. So the page offered
/// a key, the key wrote a file, and the sequence went on running exactly what
/// it ran before — with no way for anybody to tell.
///
/// # Why a second file and not a second reader
///
/// There is ONE authority, and it is `brain.json`. This is a derived view of
/// it, rewritten whenever it changes, in the shape `/etc/ronoctua/updates.conf`
/// already has — one line per switch — so the script that reads that can read
/// this with the loop it already has. The alternative was teaching a
/// `writeShellApplication` to parse JSON, which is either a new runtime
/// dependency or a `grep` that will one day answer confidently and wrongly
/// about somebody's machine.
///
/// It is the same relationship the theme's compositor fragment has with
/// `theme.json`: one file is the decision and the other is the decision
/// written where a different program can read it.
///
/// # Why here, and why the STATE directory
///
/// Here because this is the one module with steps a script reads, and the core
/// that saves `brain.json` does not know which module keeps which file — it
/// calls [`Module::prefs_saved`] after every save. The state directory because
/// the person does not write it — the brain does. The config directory is for
/// what somebody edits by hand, and an entry they made there would be silently
/// overwritten the next time they pressed a key.
///
/// # Errors
///
/// The directory cannot be created, or the file cannot be written.
fn write_steps_to(dir: &Path, task: &str, flipped: &BTreeSet<String>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut out = String::from(
        "# GENERATED by ronoctua-brain out of brain.json. Do not edit by hand.\n\
         # One line per step whose switch differs from what the distro ships.\n",
    );
    for step in flipped {
        // THE TASK IS IN THE LINE, because the file's reader is a script that
        // filters by its own id — the shape it has always read.
        out.push_str(task);
        out.push(' ');
        out.push_str(step);
        out.push('\n');
    }
    ronoctua_xdg::write_atomically(dir, STEPS_FILE, out.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moment::{DAY, Fixed};
    use crate::prefs::PartOfDay;

    fn a_task() -> SystemUpdate {
        a_task_on(Probed {
            metered: None,
            charge: Charge {
                discharging: false,
                percent: None,
            },
        })
    }

    /// The machine a test says it is.
    struct Probed {
        metered: Option<bool>,
        charge: Charge,
    }

    impl Probe for Probed {
        fn metered(&self) -> Option<bool> {
            self.metered
        }

        fn charge(&self) -> Charge {
            self.charge
        }
    }

    /// A machine whose `/sys` is planted, read by the very function the
    /// real one and the doors call ([`Charge::read_at`]); the network is the
    /// test's.
    struct Planted {
        places: ronoctua_memory::Places,
        metered: Option<bool>,
    }

    impl Probe for Planted {
        fn metered(&self) -> Option<bool> {
            self.metered
        }

        fn charge(&self) -> Charge {
            Charge::read_at(&self.places)
        }
    }

    fn a_task_on(probe: impl Probe + 'static) -> SystemUpdate {
        SystemUpdate {
            zone: Arc::new(Fixed(0)),
            probe: Box::new(probe),
        }
    }

    /// A scratch directory of this test's own, gone when it is dropped.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(what: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "ronoctua-brain-{what}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn switched_off_it_says_so_instead_of_going_quiet() {
        // A row that vanished when it was switched off would be a feature
        // nobody switches back on. The CORE says `off`, so no module can
        // forget to.
        let task = a_task();
        let mut mine = super::super::MANIFEST.defaults();
        mine.enabled = false;
        assert_eq!(
            crate::hub::due_of(&task, 0, &mine, None),
            Some(Due::Not { why: "off" })
        );
    }

    #[test]
    fn a_first_run_is_the_next_window_and_not_a_moment_in_the_past() {
        // THE DEFECT THIS CATCHES: `last: None` treated as "ran at the epoch",
        // which makes a fresh install overdue by fifty-six years — and starts a
        // rebuild the first time the daemon comes up.
        let task = a_task();
        let mut mine = super::super::MANIFEST.defaults();
        mine.part_of_day = PartOfDay::Night;
        // 09:00 on day 0: today's night window has closed, so the answer is
        // tomorrow at 02:00 — not "now".
        let due = task.due(9 * 3600, &mine, None);
        if systemd::reachable() {
            assert_eq!(due, Due::At(DAY + 2 * 3600));
        } else {
            // In a devshell nothing baked the path, and the honest answer is
            // that this build cannot start anything.
            assert_eq!(
                due,
                Due::Not {
                    why: "no-systemctl"
                }
            );
        }
    }

    #[test]
    fn its_declaration_is_the_one_the_screen_draws() {
        // WEEKLY IN THE AFTERNOON, with a minute of warning — the manifest's,
        // and the preferences resolve to it with no file at all.
        let m = &super::super::MANIFEST;
        let mine = crate::prefs::Prefs::default().of_module(m);
        assert_eq!(
            (mine.interval_days, mine.part_of_day),
            (7, PartOfDay::Afternoon)
        );
        assert_eq!(mine.countdown_seconds, 60);
        assert_eq!(mine.runaway_minutes, 120);
        assert!(m.gated(), "an update the gate cannot hold");
        assert!(m.needs.network);
        // THE UNITS THE BRAIN STARTS ARE THE MANIFEST'S, and the run starts
        // the one and the rehearsal the other.
        assert_eq!(m.units.run, Some(super::super::unit::RUN));
        assert_eq!(m.units.rehearse, Some(super::super::unit::REHEARSE));
        assert_ne!(super::super::unit::RUN, super::super::unit::REHEARSE);
        // AND THE NAME IS ITS OWN KEY, not a core text.
        assert_eq!(
            m.name_key,
            super::super::key::BRAIN_TASK_SYSTEM_UPDATE,
            "its name is not its own"
        );
    }

    #[test]
    fn the_steps_file_is_written_when_the_preferences_are() {
        // THE SWITCH REACHES THE RUN: `update-steps.conf` is what the
        // sequence's script reads, and it is this module's to write.
        let dir = Scratch::new("update-prefs");
        let step = steps().first().expect("a step").id;
        let mut mine = super::super::MANIFEST.defaults();
        mine.steps_flipped.insert(step.to_owned());
        let dirs = Where {
            config: None,
            state: Some(dir.0.clone()),
        };
        a_task().prefs_saved(&mine, &dirs).expect("write");
        let text = std::fs::read_to_string(dir.0.join(STEPS_FILE)).expect("read");
        assert!(
            text.lines()
                .any(|l| l == format!("{} {step}", super::super::ID)),
            "{text}"
        );
        // AND NOWHERE TO WRITE IS NOT A FAILURE: a brain with no state
        // directory saves its preferences all the same.
        let nowhere = Where {
            config: None,
            state: None,
        };
        a_task()
            .prefs_saved(&mine, &nowhere)
            .expect("nothing to write");
    }

    #[test]
    fn the_steps_file_is_generated_whole_with_one_line_per_flip() {
        // Rewritten WHOLE every time, so a switch put back to its default
        // leaves the file instead of lingering in it.
        let dir = Scratch::new("update-steps");
        let flips: BTreeSet<String> = ["an-off-step", "an-on-step"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        write_steps_to(&dir.0, "made-up", &flips).expect("write");
        let text = std::fs::read_to_string(dir.0.join(STEPS_FILE)).expect("read");
        let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(lines, ["made-up an-off-step", "made-up an-on-step"]);
        write_steps_to(&dir.0, "made-up", &BTreeSet::new()).expect("rewrite");
        let text = std::fs::read_to_string(dir.0.join(STEPS_FILE)).expect("read");
        assert!(
            text.lines().all(|l| l.starts_with('#')),
            "a flip put back lingered: {text}"
        );
    }

    #[test]
    fn a_failed_step_names_itself_and_carries_what_it_said() {
        let steps = vec![
            StepOutcome {
                id: "a-step-that-worked".into(),
                ok: true,
                seconds: 12,
                trouble: String::new(),
            },
            StepOutcome {
                id: "a-step-that-failed".into(),
                ok: false,
                seconds: 900,
                trouble: "error: builder for drv failed".into(),
            },
        ];
        assert_eq!(
            ended(steps, ""),
            Outcome::Failed {
                at: "a-step-that-failed".to_owned(),
                why: "error: builder for drv failed".to_owned(),
            }
        );
    }

    #[test]
    fn every_step_working_is_the_only_way_to_be_done() {
        let ok = vec![StepOutcome {
            id: "a-step".into(),
            ok: true,
            seconds: 1,
            trouble: String::new(),
        }];
        assert!(matches!(ended(ok.clone(), ""), Outcome::Done(_)));
        // A unit that said everything worked AND left a complaint is not a
        // success: the complaint is the part somebody has to see.
        assert_eq!(
            ended(ok, "unit failed to write the boot entry"),
            Outcome::Failed {
                at: String::new(),
                why: "unit failed to write the boot entry".to_owned(),
            }
        );
    }

    #[test]
    fn the_steps_come_from_the_declared_table_and_nowhere_else() {
        // THIS TEST REPLACED ONE THAT COUNTED. The old one asserted exactly one
        // step was off by default — which was true of the three steps the brain
        // used to WRITE, and meaningless once it started reading the table the
        // sequence actually declares. What wants guarding is that there is
        // only one table.
        //
        // So: id for id, in order, with the two booleans that decide whether a
        // row is drawn as on and whether it needs root, and the two keys.
        // Reintroduce a hand-written list and this fails on the first element.
        assert_eq!(
            steps().len(),
            declared::STEPS.len(),
            "a step was left over or missing"
        );
        for (mine, theirs) in steps().iter().zip(declared::STEPS) {
            assert_eq!(mine.id, theirs.id, "the id is not the table's");
            assert_eq!(
                mine.needs_root, theirs.root,
                "{}: the root flag diverged",
                mine.id
            );
            assert_eq!(
                mine.on_by_default, theirs.enabled,
                "{}: the default diverged",
                mine.id
            );
            // The command is the argv joined for reading. Empty would be a row
            // that says a step exists and cannot say what it does.
            assert!(!mine.command.is_empty(), "{}: no command", mine.id);
            assert!(!mine.id.is_empty());
        }
        // AND BOTH ANSWERS ARE PRESENT. A table where everything shipped on
        // would make `steps_flipped` unreachable from the screen, and one where
        // everything shipped off would make the sequence a no-op.
        assert!(
            steps().iter().any(|s| s.on_by_default),
            "no step is switched on"
        );
        assert!(
            steps().iter().any(|s| !s.on_by_default),
            "no step is switched off"
        );
    }

    #[test]
    fn every_step_carries_its_own_two_keys_in_the_modules_namespace() {
        // THE KEYS TRAVEL WITH THE STEP, on the wire (`name_key`, `does_key`):
        // the buffer used to build `update-<id>` itself, a word no module
        // declared. The generator derives both from the id inside THIS
        // module's namespace, and the fragment carries them in every language
        // (`…-has-text` reads the list the generator wrote against the merged
        // texts); here, the shape and the table agree, and the first step's
        // two are constants of the generated `key::` table — made from the
        // fragment, so a key the fragment lacks does not compile.
        let id = super::super::ID;
        for s in steps() {
            assert_eq!(s.name_key, format!("{id}-step-{}", s.id), "{}", s.id);
            assert_eq!(s.does_key, format!("{id}-step-{}-does", s.id), "{}", s.id);
        }
        let first = steps().first().expect("a step");
        assert_eq!(
            (first.name_key, first.does_key),
            (
                super::super::key::SYSTEM_UPDATE_STEP_PERSON_FETCH,
                super::super::key::SYSTEM_UPDATE_STEP_PERSON_FETCH_DOES
            ),
            "the first step is not the one the fragment names first"
        );
        // AND THE REASON A STEP WAS SKIPPED, by the runner's word — every word
        // the table knows, each a constant of the fragment, and nothing else.
        for (word, key) in [
            ("always", super::super::key::SYSTEM_UPDATE_NEEDS_ALWAYS),
            ("binary", super::super::key::SYSTEM_UPDATE_NEEDS_BINARY),
            (
                "directory",
                super::super::key::SYSTEM_UPDATE_NEEDS_DIRECTORY,
            ),
            (
                "writable-file",
                super::super::key::SYSTEM_UPDATE_NEEDS_WRITABLE_FILE,
            ),
            ("network", super::super::key::SYSTEM_UPDATE_NEEDS_NETWORK),
            ("file", super::super::key::SYSTEM_UPDATE_NEEDS_FILE),
            ("power", super::super::key::SYSTEM_UPDATE_NEEDS_POWER),
            (
                "free-space",
                super::super::key::SYSTEM_UPDATE_NEEDS_FREE_SPACE,
            ),
            ("not-held", super::super::key::SYSTEM_UPDATE_NEEDS_NOT_HELD),
        ] {
            assert_eq!(needs_key(word), Some(key), "`{word}`");
        }
        assert_eq!(needs_key("a-word-no-runner-writes"), None);
        assert_eq!(needs_key(""), None);
    }

    /// THE SHEET THE SCRIPT WRITES IS THE SHEET THIS CRATE READS.
    ///
    /// The defect this pins: the brain watched a JSON file that **nothing on
    /// the machine wrote**. The update's script kept a TSV in another
    /// directory, `Report::watch` never saw an mtime move, and every run the
    /// brain started came back `runaway` once its patience was gone — two
    /// hours, on a rebuild that had worked.
    ///
    /// The literal below is the shape `gen-updates.py` emits, run against a
    /// planted report. It is a fixture and not a call, because the script is
    /// shell and this is a unit test; what keeps the two in step is this
    /// module's check `…-writes-where-the-brain-reads`, which reads the
    /// generated script and the runner.
    #[test]
    fn the_scripts_sheet_is_read_the_way_the_screen_needs_it() {
        let written = r#"{"began":1789868464,"ended":1789868999,"trouble":"","steps":[{"id":"first-step","ok":true,"seconds":12,"trouble":""},{"id":"second-step","ok":true,"seconds":0,"trouble":""}]}"#;
        let sheet: crate::task::Sheet = serde_json::from_str(written).expect("the sheet parses");
        assert_eq!(sheet.began, 1_789_868_464);
        assert_eq!(sheet.ended, 1_789_868_999, "a run that ended says so");
        assert_eq!(sheet.steps.len(), 2);
        assert!(matches!(
            ended(sheet.steps, &sheet.trouble),
            Outcome::Done(_)
        ));

        // AND A STEP THAT WAS MERELY SKIPPED IS NOT A FAILURE. The script
        // writes `skipped-network` for a machine with no connection and the
        // sheet carries `ok: true` for it — marking it false would report the
        // whole update as broken because `tldr` could not reach the network.
        let skipped = r#"{"began":1,"ended":2,"trouble":"","steps":[{"id":"a-step-that-was-skipped","ok":true,"seconds":0,"trouble":""}]}"#;
        let sheet: crate::task::Sheet = serde_json::from_str(skipped).expect("parses");
        assert!(matches!(
            ended(sheet.steps, &sheet.trouble),
            Outcome::Done(_)
        ));

        // AND ONE THAT FAILED NAMES THE STEP IT STOPPED AT.
        let failed = r#"{"began":1,"ended":2,"trouble":"","steps":[{"id":"first-step","ok":true,"seconds":1,"trouble":""},{"id":"the-step-that-failed","ok":false,"seconds":3,"trouble":""}]}"#;
        let sheet: crate::task::Sheet = serde_json::from_str(failed).expect("parses");
        match ended(sheet.steps, &sheet.trouble) {
            Outcome::Failed { at, .. } => assert_eq!(at, "the-step-that-failed"),
            other => panic!("a step that failed is a run that failed: {other:?}"),
        }
    }

    /// A SHEET STILL BEING WRITTEN SAYS SO, and the watcher keeps waiting.
    ///
    /// `ended: 0` is the whole of that contract. Without it — a script that
    /// stamped the end at the top, say — the first look would find a finished
    /// run and the brain would report a rebuild as over before it began.
    #[test]
    fn a_run_still_going_has_no_end() {
        let going = r#"{"began":10,"ended":0,"trouble":"","steps":[]}"#;
        let sheet: crate::task::Sheet = serde_json::from_str(going).expect("parses");
        assert_eq!(sheet.ended, 0, "zero is how the sheet says `still going`");
    }

    /// A SCHEDULED RUN WAITS, AND NEVER ASKS: on a metered way out, or on a
    /// battery below the floor, the run is HELD with the word the row draws
    /// — and on mains, or above the floor, or with nobody able to say, it
    /// goes. `Verdict` has no third answer: there is nothing here that could
    /// ask anybody anything.
    #[test]
    fn a_scheduled_run_waits_on_a_metered_network_and_a_low_battery() {
        let floor = battery_floor().expect("the steps that compile declare a floor");
        let mine = super::super::MANIFEST.defaults();
        let verdict = |metered, percent, discharging| {
            a_task_on(Probed {
                metered,
                charge: Charge {
                    discharging,
                    percent,
                },
            })
            .needs(&Pulse::default(), &mine)
        };
        assert_eq!(
            verdict(Some(true), Some(100), false),
            Verdict::Hold(HOLD_METERED)
        );
        assert_eq!(
            HOLD_METERED,
            Hold::Network,
            "the row's word is the network's"
        );
        assert_eq!(
            verdict(Some(false), Some(floor - 1), true),
            Verdict::Hold(Hold::Battery)
        );
        assert_eq!(
            verdict(None, None, true),
            Verdict::Hold(Hold::Battery),
            "a machine battery discharging by an amount nobody can read is not enough to compile on"
        );
        assert_eq!(
            verdict(Some(false), Some(floor), true),
            Verdict::Go,
            "at the floor"
        );
        assert_eq!(
            verdict(Some(false), Some(5), false),
            Verdict::Go,
            "on mains"
        );
        assert_eq!(
            verdict(None, None, false),
            Verdict::Go,
            "a desktop that cannot say"
        );
    }

    /// A planted `/sys`: the batteries given, by name, as
    /// `(status, capacity, scope)`.
    fn planted(
        what: &str,
        batteries: &[(&str, &str, u8, Option<&str>)],
    ) -> (Scratch, ronoctua_memory::Places) {
        let dir = Scratch::new(what);
        let supplies = dir.0.join("sys/class/power_supply");
        for (name, status, capacity, scope) in batteries {
            let at = supplies.join(name);
            std::fs::create_dir_all(&at).expect("mkdir");
            std::fs::write(at.join("type"), "Battery\n").expect("type");
            std::fs::write(at.join("status"), format!("{status}\n")).expect("status");
            std::fs::write(at.join("capacity"), format!("{capacity}\n")).expect("capacity");
            if let Some(scope) = scope {
                std::fs::write(at.join("scope"), format!("{scope}\n")).expect("scope");
            }
        }
        // AN ADAPTER IS A SUPPLY AND NO BATTERY.
        let mains = supplies.join("AC");
        std::fs::create_dir_all(&mains).expect("mkdir");
        std::fs::write(mains.join("type"), "Mains\n").expect("type");
        let places = ronoctua_memory::Places {
            procfs: dir.0.join("proc"),
            sysfs: dir.0.join("sys"),
            cgroup: dir.0.join("cgroup"),
            uid: 0,
        };
        (dir, places)
    }

    /// A MOUSE'S BATTERY IS NOT THE MACHINE'S, for the hold and for the
    /// question alike. The defect this catches: "discharging" was read over
    /// every battery — a Logitech mouse's included, `scope=Device` and always
    /// discharging — while the charge skipped it, so a desktop with a
    /// wireless mouse held every scheduled update for ever, and the door
    /// asked to plug in a machine that was plugged in.
    #[test]
    fn a_mouse_battery_neither_holds_the_run_nor_asks() {
        let floor = battery_floor().expect("a floor");
        let mine = super::super::MANIFEST.defaults();
        let judged = |places: &ronoctua_memory::Places| {
            let charge = Charge::read_at(places);
            let verdict = a_task_on(Planted {
                places: places.clone(),
                metered: Some(false),
            })
            .needs(&Pulse::default(), &mine);
            (verdict, charge.below_the_floor())
        };
        // A DESKTOP WITH A WIRELESS MOUSE: no battery of its own.
        let (_desk, places) = planted(
            "update-mouse",
            &[("hidpp_battery_0", "Discharging", 5, Some("Device"))],
        );
        assert_eq!(
            judged(&places),
            (Verdict::Go, false),
            "the mouse held a desktop"
        );
        // A LAPTOP CHARGING AT 30 %, WITH THE MOUSE: on mains.
        let (_laptop, places) = planted(
            "update-charging",
            &[
                ("BAT0", "Charging", 30, Some("System")),
                ("hidpp_battery_0", "Discharging", 5, Some("Device")),
            ],
        );
        assert_eq!(
            judged(&places),
            (Verdict::Go, false),
            "a plugged laptop was asked to plug in"
        );
        // AND THE SAME LAPTOP OFF THE MAINS BELOW THE FLOOR, the mouse full:
        // held, and asked — the mouse lifts nothing.
        let (_flat, places) = planted(
            "update-flat",
            &[
                ("BAT0", "Discharging", floor - 1, None),
                ("hidpp_battery_0", "Discharging", 100, Some("Device")),
            ],
        );
        assert_eq!(judged(&places), (Verdict::Hold(Hold::Battery), true));
    }

    /// THE DOOR'S QUESTION IS THE FLOOR'S: discharging below it, or by an
    /// amount nobody reads — and never on mains, however flat.
    #[test]
    fn the_charge_is_below_the_floor_only_off_the_mains() {
        let floor = battery_floor().expect("a floor");
        let charge = |discharging, percent| Charge {
            discharging,
            percent,
        };
        assert!(charge(true, Some(floor - 1)).below_the_floor());
        assert!(charge(true, None).below_the_floor());
        assert!(!charge(true, Some(floor)).below_the_floor());
        assert!(!charge(false, Some(1)).below_the_floor());
        assert!(!charge(false, None).below_the_floor());
    }

    /// A REFUSAL THAT IS NEWS has a sentence of the module's own, and a word
    /// no step declares has none — never a key made of the word.
    #[test]
    fn a_declined_step_says_why_in_its_own_words() {
        assert_eq!(
            declined_key("did-not-boot"),
            Some(super::super::key::SYSTEM_UPDATE_DECLINED_DID_NOT_BOOT)
        );
        assert_eq!(declined_key("a-word-no-program-writes"), None);
    }

    /// THE RECORDS ARE READ THROUGH THE CRATES THAT WRITE THEM: the boot
    /// trial and the hold from the machine's record, what the verification
    /// held from `local/held.json`, the conflicts of the CURRENT repository
    /// from the belongings' last plain run — and an absent or broken file is
    /// an empty answer, never an invented one.
    #[test]
    fn the_records_say_the_trial_the_hold_the_held_and_the_conflicts() {
        let dir = Scratch::new("update-records");
        let machine_dir = dir.0.join("machine");
        let machine_state = dir.0.join("state");
        let home = dir.0.join("home");
        let belongings = ronoctua_belongings::places::Places::under(&home);
        let at = RecordPlaces {
            machine_dir: machine_dir.clone(),
            machine_state: machine_state.clone(),
            belongings: Some(belongings.clone()),
        };
        assert_eq!(
            Records::read_at(&at),
            Records::default(),
            "nothing there is nothing said"
        );

        std::fs::create_dir_all(&machine_state).expect("mkdir");
        std::fs::write(
            machine_state.join(ronoctua_person::names::STATE),
            r#"{"format": 1, "last-good": null,
                "trial": {"toplevel": "/nix/store/00000000000000000000000000000001-nixos-system-b", "outcome": "failed"},
                "hold": {"rev": "613862c", "since": "2026-10-05T12:00:00Z"}, "last-apply": null}"#,
        )
        .expect("state");
        std::fs::create_dir_all(machine_dir.join("local")).expect("mkdir");
        std::fs::write(
            machine_dir.join(ronoctua_person::names::HELD),
            r#"{"format": 1, "held": [{"id": "nix:hello", "fingerprint": "abc",
                "reason": "build-failed", "since": "2026-10-05T12:00:00Z"}]}"#,
        )
        .expect("held");
        std::fs::create_dir_all(&belongings.config).expect("mkdir");
        std::fs::write(
            belongings.prefs(),
            r#"{"repository": "git@example.org:me/belongings.git"}"#,
        )
        .expect("prefs");
        std::fs::create_dir_all(belongings.own()).expect("mkdir");
        let mut last = ronoctua_belongings::status::Last::default();
        last.plain.repository = "git@example.org:me/belongings.git".to_owned();
        last.plain.conflicts = vec!["state/person/software/declared.json".to_owned()];
        std::fs::write(belongings.last(), serde_json::to_vec(&last).expect("json")).expect("last");

        let r = Records::read_at(&at);
        assert_eq!(r.trial, Some(TrialOutcome::Failed));
        assert_eq!(
            r.hold,
            Some(HeldAt {
                rev: "613862c".to_owned(),
                since: "2026-10-05T12:00:00Z".to_owned()
            })
        );
        assert_eq!(
            r.held,
            vec![Held {
                id: "nix:hello".to_owned(),
                why: HeldWhy::BuildFailed
            }]
        );
        assert_eq!(r.conflicts, 1);

        // A BASE OF ANOTHER REPOSITORY is a first run's: no conflict of it
        // is this one's.
        std::fs::write(
            belongings.prefs(),
            r#"{"repository": "git@example.org:me/another.git"}"#,
        )
        .expect("prefs");
        assert_eq!(Records::read_at(&at).conflicts, 0);
        // AND A RECORD THAT DOES NOT READ says nothing of the trial.
        std::fs::write(machine_state.join(ronoctua_person::names::STATE), b"{").expect("broken");
        let r = Records::read_at(&at);
        assert_eq!((r.trial, r.hold), (None, None));
    }
}
