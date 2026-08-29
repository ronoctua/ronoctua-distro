//! Every task the brain knows — and the refusals that happen at construction.
//!
//! # Why the check is here and not at the first use
//!
//! Because an id is not only an id: it is the key of `brain-task-<id>` in both
//! `.ftl`, the key of the entry in the preferences file, and the name in the
//! journal. A malformed one compiles, runs, and shows up as a bracketed key on
//! somebody's screen — or worse, as a preferences entry that never matches and
//! silently gives every run the distro's defaults.
//!
//! It is exactly the argument `ronoctua-cmd::Registry::register` makes for
//! commands, including the part that matters most: these are PROGRAMMING
//! errors, not user errors, so they fail loudly at the one moment a programmer
//! is looking — the daemon refusing to start with the id in the message.

use std::collections::BTreeMap;

use crate::task::Task;

/// Why a registry could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The id is not kebab-case.
    MalformedId(&'static str),
    /// Two tasks answer to the same name.
    ///
    /// Accepting it would let the second one win, and the person's preferences
    /// — kept by id — would silently belong to whichever happened to be
    /// registered last.
    RepeatedId(&'static str),
    /// A step's id is not kebab-case.
    MalformedStep {
        /// Whose.
        task: &'static str,
        /// Which.
        step: &'static str,
    },
    /// Two steps of one task answer to the same name.
    ///
    /// The person switches a step off BY ID, so two of them by one name is one
    /// switch controlling two things — and the screen showing two rows that
    /// toggle together.
    RepeatedStep {
        /// Whose.
        task: &'static str,
        /// Which.
        step: &'static str,
    },
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedId(id) => write!(
                f,
                "`{id}` is not kebab-case; the key `brain-task-{id}` would be born \
                 impossible to write in the .ftl"
            ),
            Self::RepeatedId(id) => write!(
                f,
                "two tasks are called `{id}`; the preferences of one would be \
                 applied to another with no warning at all"
            ),
            Self::MalformedStep { task, step } => {
                write!(f, "the step `{step}` of `{task}` is not kebab-case")
            }
            Self::RepeatedStep { task, step } => write!(
                f,
                "`{task}` has two steps called `{step}`; switching one off \
                 desligaria os dois"
            ),
        }
    }
}

impl std::error::Error for Failure {}

/// Every task, by id, in a stable order.
///
/// `BTreeMap` and not `HashMap`, for the reason `ronoctua-cmd` gives: the
/// iteration order has to be the same every time, so the screen lists the same
/// way twice and a test does not depend on hashing.
pub struct Registry {
    tasks: BTreeMap<&'static str, Box<dyn Task>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("tasks", &self.tasks.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Registry {
    /// Builds one, refusing anything malformed.
    ///
    /// # Errors
    ///
    /// A malformed or repeated id, on a task or on one of its steps.
    pub fn of(tasks: Vec<Box<dyn Task>>) -> Result<Self, Failure> {
        let mut out = BTreeMap::new();
        for task in tasks {
            let id = task.id();
            if !kebab(id) {
                return Err(Failure::MalformedId(id));
            }
            if out.contains_key(id) {
                return Err(Failure::RepeatedId(id));
            }
            let mut seen: Vec<&'static str> = Vec::new();
            for step in task.steps() {
                if !kebab(step.id) {
                    return Err(Failure::MalformedStep {
                        task: id,
                        step: step.id,
                    });
                }
                if seen.contains(&step.id) {
                    return Err(Failure::RepeatedStep {
                        task: id,
                        step: step.id,
                    });
                }
                seen.push(step.id);
            }
            out.insert(id, task);
        }
        Ok(Self { tasks: out })
    }

    /// One task, by id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&dyn Task> {
        self.tasks.get(id).map(std::convert::AsRef::as_ref)
    }

    /// All of them, in id order.
    pub fn all(&self) -> impl Iterator<Item = &dyn Task> {
        self.tasks.values().map(std::convert::AsRef::as_ref)
    }

    /// How many there are.
    #[must_use]
    pub fn amount(&self) -> usize {
        self.tasks.len()
    }
}

/// `kebab-case`: lower case, digits and hyphens, with no hyphen at either end
/// and no double one.
///
/// The same rule, letter for letter, that `ronoctua-cmd` applies to a command
/// id — because the same thing depends on it: the text key is the id with a
/// prefix, and a spelling that has to be TRANSLATED between the two is a
/// spelling nothing can check.
fn kebab(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moment::Moment;
    use crate::prefs::Prefs;
    use crate::pulse::Pulse;
    use crate::task::{Due, Outcome, Report, Step, Verdict};

    struct Fake {
        id: &'static str,
        steps: &'static [Step],
    }

    impl Task for Fake {
        fn id(&self) -> &'static str {
            self.id
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
            self.steps
        }
        fn run(&self, _report: &Report) -> Outcome {
            Outcome::Done(Vec::new())
        }
    }

    fn one(id: &'static str) -> Box<dyn Task> {
        Box::new(Fake { id, steps: &[] })
    }

    #[test]
    fn a_malformed_id_never_reaches_a_screen() {
        // `brain-task-Atualizar Sistema` is not a key anybody can write in a
        // `.ftl`, and the screen would draw it in brackets. Refusing here is
        // the daemon not starting, with the id in the message.
        for bad in [
            "",
            "-update",
            "update-",
            "sys--update",
            "systemUpdate",
            "system update",
        ] {
            assert_eq!(
                Registry::of(vec![one(bad)]).err(),
                Some(Failure::MalformedId(bad)),
                "aceitou `{bad}`"
            );
        }
        assert!(Registry::of(vec![one("system-update")]).is_ok());
    }

    #[test]
    fn two_tasks_by_one_name_are_refused() {
        let e = Registry::of(vec![one("system-update"), one("system-update")]).err();
        assert_eq!(e, Some(Failure::RepeatedId("system-update")));
    }

    #[test]
    fn a_steps_id_is_held_to_the_same_rule() {
        static BAD: &[Step] = &[Step {
            id: "Build",
            command: "nixos-rebuild",
            needs_root: true,
            on_by_default: true,
        }];
        static TWICE: &[Step] = &[
            Step {
                id: "build",
                command: "a",
                needs_root: true,
                on_by_default: true,
            },
            Step {
                id: "build",
                command: "b",
                needs_root: true,
                on_by_default: true,
            },
        ];
        assert_eq!(
            Registry::of(vec![Box::new(Fake {
                id: "system-update",
                steps: BAD
            })])
            .err(),
            Some(Failure::MalformedStep {
                task: "system-update",
                step: "Build"
            })
        );
        assert_eq!(
            Registry::of(vec![Box::new(Fake {
                id: "system-update",
                steps: TWICE
            })])
            .err(),
            Some(Failure::RepeatedStep {
                task: "system-update",
                step: "build"
            })
        );
    }

    #[test]
    fn the_order_is_the_same_every_time() {
        let r = Registry::of(vec![one("zebra"), one("alpha")]).expect("registro");
        let ids: Vec<&str> = r.all().map(crate::task::Task::id).collect();
        assert_eq!(ids, vec!["alpha", "zebra"]);
        assert_eq!(r.amount(), 2);
        assert!(r.find("alpha").is_some());
        assert!(r.find("nope").is_none());
    }
}
