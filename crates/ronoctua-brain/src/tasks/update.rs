//! The system update — the brain's first feature, and the shape of every one
//! after it.
//!
//! # What it does, exactly
//!
//! At the right moment, it asks systemd to start one unit and then reads what
//! the unit reported. That is all of it. What an update IS — which inputs,
//! which command, in which order — is declared in the flake and stays there
//! (invariant 5): this file names a unit and a table of steps for the screen to
//! draw, and neither of them is a script.
//!
//! # Why the steps are drawn at all
//!
//! Because "the machine will update itself this afternoon" is a sentence a
//! person should be able to open. Under the cursor the screen says the real
//! command, whether it needs root, and whether the person has changed it from
//! what the distro ships — the same rule the disk formatter's two menus follow,
//! where every line carries a second, quieter line saying what it really means.
//!
//! **The table and the unit have to agree.** They are two files, and nothing in
//! Rust can check that: the guard belongs in `tests/checks.nix`, comparing the
//! ids here against the ones the unit's script writes into its report. That
//! check is named in this wave's report; until it exists, changing one means
//! changing the other by hand.
//!
//! # `boot` and not `switch`
//!
//! The build step ends at the boot entry, and nothing changes under a running
//! session. A rebuild that switches while somebody is working restarts units
//! underneath them — and the one thing an unattended update must never do is
//! surprise the person who is using the machine. It takes effect the next time
//! they choose to reboot, which is a moment they picked.

use std::sync::Arc;

use crate::moment::{Moment, Offset};
use crate::prefs::Prefs;
use crate::pulse::Pulse;
use crate::schedule::next_moment;
use crate::systemd;
use crate::task::{Due, Outcome, Report, Step, StepOutcome, Task, Verdict, Watched};

/// The task's id — English, kebab-case, and the key of everything the person
/// sees about it.
pub const ID: &str = "system-update";

/// The unit the flake ships, and the only thing this task starts.
///
/// A CONSTANT, and deliberately not a preference. A name that came out of a
/// configuration file would be a configuration file that can start any unit the
/// polkit rule allows — and the rule is narrow precisely because the name is
/// ours.
pub const UNIT: &str = "ronoctua-update.service";

/// The same thing, changing nothing — what `m e` asks for.
///
/// A second unit and not a flag, for the same reason: an argument that crossed
/// from here would be an argument somebody else could cross with.
pub const REHEARSAL_UNIT: &str = "ronoctua-update-rehearsal.service";

/// The declared sequence, generated from `assets/updates.nix`.
///
/// # Why this is included and not written
///
/// Because it was written, and the two copies disagreed. This file used to
/// carry three steps of its own — `update-inputs`, `build`, `collect-garbage`
/// — with their own command strings, while the table the UNIT actually runs
/// declares twelve, named `flake-update`, `system-rebuild`,
/// `store-collect-garbage` and so on. The ids were not a subset of each other:
/// they were disjoint.
///
/// The screen paid for it twice. Every step drawn on the page behind carried a
/// name the sequence does not use; and the "it stopped at" row takes its id
/// from the report the unit writes, so a run that failed at `flake-update`
/// would look up a key that does not exist and draw `[brain-step-flake-update]`
/// where the reason should be.
///
/// Nothing could have caught it. Two files, both plausible, no compiler and no
/// check between them — the shape this repository calls a second truth. It is
/// one file now, and the drift is not a defect that got fixed, it is a defect
/// that has nowhere left to live.
mod declared {
    #![allow(dead_code, missing_docs)]
    include!(concat!(env!("OUT_DIR"), "/updates.rs"));
}

/// What the update does, in order.
///
/// The `command` is DATA — it is drawn as it is written, because a command line
/// is not translated and inventing a friendly rendering of one is how a screen
/// ends up lying about what it is running.
///
/// It is built from [`declared::STEPS`] at first use rather than being a
/// `static`: the generated table's `command` is a slice of arguments and this
/// one is the line a person reads.
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
            })
            .collect()
    })
}

/// The system update.
pub struct SystemUpdate {
    /// How far ahead of UTC this machine is — handed in, so [`Task::due`] stays
    /// pure. See [`crate::tasks::tasks`].
    zone: Arc<dyn Offset>,
}

impl SystemUpdate {
    /// One, in a given zone.
    #[must_use]
    pub fn with(zone: Arc<dyn Offset>) -> Self {
        Self { zone }
    }
}

impl Task for SystemUpdate {
    fn id(&self) -> &'static str {
        ID
    }

    fn due(&self, now: Moment, prefs: &Prefs, last: Option<Moment>) -> Due {
        let mine = prefs.of(ID);
        if !mine.enabled {
            return Due::Not { why: "off" };
        }
        // A BUILD THAT CANNOT ASK is not a task that is late — it is a task
        // that cannot happen, and the screen has to say which. Offering a
        // moment here would put a date on screen that nothing will ever
        // honour; this way the row says `brain-why-no-systemctl` instead.
        if !systemd::reachable() {
            return Due::Not {
                why: "no-systemctl",
            };
        }
        let at = next_moment(now, last, &mine, self.zone.as_ref());
        if at <= now { Due::Now } else { Due::At(at) }
    }

    fn needs(&self, _pulse: &Pulse, _prefs: &Prefs) -> Verdict {
        // NOTHING OF ITS OWN, and that is not an omission. The two things this
        // task cares about — a machine that is busy and a machine on battery —
        // are the general gate's, asked by the runner before this is reached,
        // and configured by the person in one place. A copy here would be a
        // second policy that drifts from the first.
        Verdict::Go
    }

    fn countdown(&self, prefs: &Prefs) -> u16 {
        prefs.of(ID).countdown_seconds
    }

    fn steps(&self) -> &'static [Step] {
        steps()
    }

    fn run(&self, report: &Report) -> Outcome {
        let unit = if report.is_rehearsal() {
            REHEARSAL_UNIT
        } else {
            UNIT
        };
        // WHATEVER IS THERE NOW BELONGS TO THE PREVIOUS RUN, and the mark is
        // taken BEFORE the unit is asked to start. Without it the first look
        // finds the old file, sees that it ended, and reports a run that
        // finished before it began.
        let before = report.mark();
        if let Err(e) = systemd::start(unit) {
            return Outcome::Failed {
                at: String::new(),
                // A `Missing` carries the VARIABLE's name, which is the fix,
                // and a refusal carries what polkit said. Both are sentences
                // from the machine, so both go through as text.
                why: e.to_string(),
            };
        }
        match report.watch(before) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moment::{DAY, Fixed};
    use crate::prefs::PartOfDay;

    fn a_task() -> SystemUpdate {
        SystemUpdate::with(Arc::new(Fixed(0)))
    }

    #[test]
    fn switched_off_it_says_so_instead_of_going_quiet() {
        // A row that vanished when it was switched off would be a feature
        // nobody switches back on. `Due::Not` carries the WORD the screen
        // draws.
        let task = a_task();
        let mut prefs = Prefs::default();
        prefs.mine(ID).enabled = false;
        assert_eq!(task.due(0, &prefs, None), Due::Not { why: "off" });
    }

    #[test]
    fn a_first_run_is_the_next_window_and_not_a_moment_in_the_past() {
        // THE DEFECT THIS CATCHES: `last: None` treated as "ran at the epoch",
        // which makes a fresh install overdue by fifty-six years — and starts a
        // rebuild the first time the daemon comes up.
        let task = a_task();
        let mut prefs = Prefs::default();
        prefs.mine(ID).part_of_day = PartOfDay::Night;
        // 09:00 on day 0: today's night window has closed, so the answer is
        // tomorrow at 02:00 — not "now".
        let due = task.due(9 * 3600, &prefs, None);
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
    fn a_failed_step_names_itself_and_carries_what_it_said() {
        let steps = vec![
            StepOutcome {
                id: "update-inputs".into(),
                ok: true,
                seconds: 12,
                trouble: String::new(),
            },
            StepOutcome {
                id: "build".into(),
                ok: false,
                seconds: 900,
                trouble: "error: builder for drv failed".into(),
            },
        ];
        assert_eq!(
            ended(steps, ""),
            Outcome::Failed {
                at: "build".to_owned(),
                why: "error: builder for drv failed".to_owned(),
            }
        );
    }

    #[test]
    fn every_step_working_is_the_only_way_to_be_done() {
        let ok = vec![StepOutcome {
            id: "build".into(),
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
        // step was off by default — "the garbage collector stopped being the
        // exception" — which was true of the three steps this file used to
        // WRITE, and meaningless once it started reading the twelve the
        // sequence actually declares. A count is a fact about a particular
        // table; what wants guarding is that there is only one table.
        //
        // So: id for id, in order, with the two booleans that decide whether a
        // row is drawn as on and whether it needs root. Reintroduce a
        // hand-written list and this fails on the first element.
        assert_eq!(
            steps().len(),
            declared::STEPS.len(),
            "a step was left over or missing"
        );
        for (mine, theirs) in steps().iter().zip(declared::STEPS) {
            assert_eq!(mine.id, theirs.id, "the id is not the table's");
            assert_eq!(mine.needs_root, theirs.root, "{}: root divergiu", mine.id);
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
        // would make `steps_off` unreachable from the screen, and one where
        // everything shipped off would make the sequence a no-op — either way
        // the toggle below would be testing nothing.
        assert!(
            steps().iter().any(|s| s.on_by_default),
            "no step is switched on"
        );
        assert!(
            steps().iter().any(|s| !s.on_by_default),
            "no step is switched off"
        );
    }
}
