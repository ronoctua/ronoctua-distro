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
//! # The run activates nothing, and says to restart
//!
//! The routine IS the timer invariant 6 names (`module.nix` says why it is
//! not a systemd `.timer`), and its rebuild sets the new system for the NEXT
//! BOOT (`ronoctua-machine apply --next-boot`): nothing that runs is
//! stopped or restarted under the person. So a run whose apply set a system
//! for the next boot ends by TELLING them to restart ([`restart_told`]),
//! once: the new system is used only from then, and a machine nobody
//! restarts stacks one waiting system a week until the boot menu has no
//! room left for the last good one.
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
//!
//! # Two halves
//!
//! What the update's doors read too — the steps, the battery as the sequence
//! weighs it, the word a held run waits for, the machine's records beside a
//! run — is `shared.rs`, compiled into the library the footer links. This
//! file is the daemon's: when the update runs, what holds it, and what its
//! run reports.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

// THE SHARED HALF (`shared.rs`, compiled into the library beside the
// manifest): what the update's doors read of the same machine and table.
use super::{Charge, HOLD_METERED, steps};
use crate::module::{Birth, Event, Manifest, Module, Run, Scheduled};
use crate::moment::{Moment, Offset};
use crate::prefs::{TaskPrefs, Where};
use crate::pulse::Pulse;
use crate::schedule::next_moment;
use crate::systemd;
use crate::task::{Due, Hold, Outcome, Step, StepOutcome, Verdict, Watched};

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
            Watched::Ended(sheet) => {
                if !run.is_rehearsal()
                    && restart_told(&super::RecordPlaces::here(), sheet.began, |event| {
                        run.ctx().tell(event)
                    })
                {
                    tracing::info!("the new system waits for the next restart, said");
                }
                ended(sheet.steps, &sheet.trouble)
            }
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

/// Tells the person to restart when this run's apply — the machine's last,
/// at or after `began` — set a new system for the next boot, and a system
/// waits for it ([`super::next_boot`], from the profile and the system that
/// runs): the manifest's `restart` event, or `restart-unguarded` when no
/// boot trial could be armed for it, which promises no way back. Whether it
/// told.
fn restart_told(at: &super::RecordPlaces, began: Moment, tell: impl FnOnce(Event)) -> bool {
    let state = ronoctua_machine::state(&ronoctua_machine::Places {
        dir: at.machine_dir.clone(),
        state: at.machine_state.clone(),
        run: std::path::PathBuf::new(),
    })
    .ok();
    let this_run = state
        .as_ref()
        .and_then(|s| s.last_apply.as_ref())
        .is_some_and(|a| {
            a.result == ronoctua_machine::ApplyResult::Staged && a.at.unix_s() >= began
        });
    let Some(next) = this_run
        .then(|| super::next_boot(&at.root, state.as_ref()))
        .flatten()
    else {
        return false;
    };
    tell(Event::of(if next.guarded {
        &super::event::RESTART
    } else {
        &super::event::RESTART_UNGUARDED
    }));
    true
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
    use super::super::battery_floor;
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

    /// A RUN WHOSE APPLY SET A SYSTEM FOR THE NEXT BOOT says to restart,
    /// once — without promising a way back when no trial guards it — and a
    /// run whose apply switched, or set nothing, or a system set for the
    /// next boot by an earlier run, or one that no longer waits (it runs, or
    /// its trial failed and the profile went back), says nothing.
    #[test]
    fn a_run_that_set_the_next_boot_says_to_restart() {
        let dir = Scratch::new("update-restart");
        let state = dir.0.join("state");
        std::fs::create_dir_all(&state).expect("mkdir");
        let at = super::super::RecordPlaces {
            machine_dir: dir.0.join("machine"),
            machine_state: state.clone(),
            belongings: None,
            root: dir.0.clone(),
        };
        let store = |name: &str| {
            let top =
                format!("/nix/store/0000000000000000000000000000000{name}-nixos-system-{name}");
            std::fs::create_dir_all(dir.0.join(top.trim_start_matches('/'))).expect("mkdir");
            top
        };
        let (a, c) = (store("a"), store("c"));
        let link = |at: &str, to: &str| {
            let at = dir.0.join(at);
            std::fs::create_dir_all(at.parent().expect("parent")).expect("mkdir");
            let _ = std::fs::remove_file(&at);
            std::os::unix::fs::symlink(dir.0.join(to.trim_start_matches('/')), at).expect("link");
        };
        let file = ronoctua_machine::Places {
            dir: at.machine_dir.clone(),
            state,
            run: std::path::PathBuf::new(),
        }
        .state_file();
        let record = |result: &str, when: &str, trial: &str| {
            std::fs::write(
                &file,
                format!(
                    r#"{{"format": 1, "last-good": null,
                        "trial": {{"toplevel": "{c}", "outcome": "{trial}"}}, "hold": null,
                        "last-apply": {{"at": "{when}", "result": "{result}", "toplevel": "{c}"}}}}"#
                ),
            )
            .expect("state");
        };
        // 2026-10-30T12:00:00Z.
        let began: Moment = 1_793_361_600;
        let told = |at: &super::super::RecordPlaces| {
            let mut events = Vec::new();
            let said = restart_told(at, began, |e| events.push(e.tag.into_owned()));
            assert_eq!(said, !events.is_empty());
            events
        };
        link("run/current-system", &a);
        link("nix/var/nix/profiles/system", &c);
        record("staged", "2026-10-30T12:20:00Z", "pending");
        assert_eq!(told(&at), [super::super::event::RESTART.tag]);
        record("staged", "2026-10-30T12:20:00Z", "good");
        assert_eq!(
            told(&at),
            [super::super::event::RESTART_UNGUARDED.tag],
            "no trial guards it: no way back is promised"
        );
        record("switched", "2026-10-30T12:20:00Z", "pending");
        assert!(told(&at).is_empty(), "a switch needs no restart");
        record("staged", "2026-10-30T11:00:00Z", "pending");
        assert!(
            told(&at).is_empty(),
            "an earlier run's system was told then"
        );
        record("staged", "2026-10-30T12:20:00Z", "failed");
        link("nix/var/nix/profiles/system", &a);
        assert!(told(&at).is_empty(), "its trial failed: nothing waits");
        link("nix/var/nix/profiles/system", &c);
        link("run/current-system", &c);
        assert!(told(&at).is_empty(), "the system already runs");
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
}
