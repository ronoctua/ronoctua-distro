//! Every module the brain knows — and the refusals that happen at
//! construction.
//!
//! # ONE registry, shared
//!
//! The daemon builds it once and every thread reads the same one: the runner,
//! the watch thread and every socket client (they all hold the one
//! [`crate::hub::Core`]). It was built TWICE — once for the runner and once
//! for the socket — which was harmless while a task held no state, and is a
//! defect the moment one does: a reminder set on the socket's copy would ring
//! in nobody's, and a relief the runner's copy remembered would be forgotten
//! by the copy that draws the page.
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

use std::collections::{BTreeMap, BTreeSet};

use crate::module::Module;

/// The word a module the SYSTEM switched off gives instead of a moment — its
/// row's reason, `brain-why-system-off`, and what every ask about it is
/// refused with.
pub const SYSTEM_OFF: &str = "system-off";

/// The variable the brain's unit is handed: the ids of the modules the flake
/// turned off (`ronoctua.brain.modules.<id>.enable = false`), comma-separated.
pub const OFF_VARIABLE: &str = "RONOCTUA_BRAIN_OFF";

/// Why a registry could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The id is not kebab-case.
    MalformedId(&'static str),
    /// Two modules answer to the same name.
    ///
    /// Accepting it would let the second one win, and the person's preferences
    /// — kept by id — would silently belong to whichever happened to be
    /// registered last.
    RepeatedId(&'static str),
    /// A step, action, notice, mark or event of a module has a name that is
    /// not kebab-case.
    MalformedName {
        /// Whose.
        module: &'static str,
        /// Which kind of thing: `step`, `action`, `notice`, `mark`, `event`.
        of: &'static str,
        /// Which.
        name: String,
    },
    /// Two steps, actions, notices, marks or events of one module answer to
    /// the same name.
    ///
    /// The person switches a step off BY ID, and a notice is dismissed by id,
    /// so two of them by one name is one switch controlling two things.
    RepeatedName {
        /// Whose.
        module: &'static str,
        /// Which kind of thing.
        of: &'static str,
        /// Which.
        name: String,
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
                "two modules are called `{id}`; the preferences of one would be \
                 applied to another with no warning at all"
            ),
            Self::MalformedName { module, of, name } => {
                write!(f, "the {of} `{name}` of `{module}` is not kebab-case")
            }
            Self::RepeatedName { module, of, name } => write!(
                f,
                "`{module}` has two of its {of}s called `{name}`; one switch \
                 would move both"
            ),
        }
    }
}

impl std::error::Error for Failure {}

/// Every module, by id, in a stable order.
///
/// `BTreeMap` and not `HashMap`, for the reason `ronoctua-cmd` gives: the
/// iteration order has to be the same every time, so the screen lists the same
/// way twice and a test does not depend on hashing.
pub struct Registry {
    modules: BTreeMap<&'static str, Box<dyn Module>>,
    /// The modules the SYSTEM switched off — see [`Registry::switched_off`].
    off: BTreeSet<&'static str>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("modules", &self.modules.keys().collect::<Vec<_>>())
            .field("off", &self.off)
            .finish()
    }
}

impl Registry {
    /// Builds one, refusing anything malformed.
    ///
    /// # Errors
    ///
    /// A malformed or repeated id, on a module or on one of the things its
    /// manifest names.
    pub fn of(modules: Vec<Box<dyn Module>>) -> Result<Self, Failure> {
        let mut out = BTreeMap::new();
        for module in modules {
            let manifest = module.manifest();
            let id = manifest.id;
            if !kebab(id) {
                return Err(Failure::MalformedId(id));
            }
            if out.contains_key(id) {
                return Err(Failure::RepeatedId(id));
            }
            names(id, "step", module.steps().iter().map(|s| s.id))?;
            names(id, "action", manifest.actions.iter().map(|a| a.id))?;
            names(id, "notice", manifest.notices.iter().map(|n| n.id))?;
            names(id, "mark", manifest.marks.iter().map(|m| m.name.as_ref()))?;
            names(id, "event", manifest.events.iter().map(|e| e.name))?;
            out.insert(id, module);
        }
        Ok(Self {
            modules: out,
            off: BTreeSet::new(),
        })
    }

    /// An empty one — what a daemon with no module at all has.
    #[must_use]
    pub fn none() -> Self {
        Self {
            modules: BTreeMap::new(),
            off: BTreeSet::new(),
        }
    }

    /// The same modules, with these switched off BY THE SYSTEM: the ids the
    /// flake turned off, which [`OFF_VARIABLE`] carries to the unit.
    ///
    /// # Why they stay registered
    ///
    /// Because a module that vanished from the page would read as one that was
    /// never there. The flake took its units away with it; what is left here is
    /// its ROW, drawn refusing with the reason ([`SYSTEM_OFF`]), and nothing
    /// else — the core never schedules it, never listens for it, never asks it
    /// for a face, and refuses every ask about it with that same word
    /// (`crate::hub::Core::system_off`). A person's own switch cannot turn it
    /// back on: what the system configuration decided, the configuration
    /// undoes.
    ///
    /// An id this build does not know is said in the log and ignored: a flake
    /// and a daemon of two different builds is a restart away from agreeing,
    /// and refusing to start over it would stop every other module too.
    #[must_use]
    pub fn switched_off<S: AsRef<str>>(mut self, ids: &[S]) -> Self {
        for id in ids {
            let id = id.as_ref();
            match self.modules.get_key_value(id) {
                Some((known, _)) => {
                    self.off.insert(*known);
                }
                None => {
                    tracing::warn!(
                        module = id,
                        "the system switched off a module this brain does not have"
                    );
                }
            }
        }
        self
    }

    /// Whether the SYSTEM switched this module off.
    #[must_use]
    pub fn off(&self, id: &str) -> bool {
        self.off.contains(id)
    }

    /// One module, by id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&dyn Module> {
        self.modules.get(id).map(std::convert::AsRef::as_ref)
    }

    /// All of them, in id order.
    pub fn all(&self) -> impl Iterator<Item = &dyn Module> {
        self.modules.values().map(std::convert::AsRef::as_ref)
    }

    /// Every id, in order.
    pub fn ids(&self) -> impl Iterator<Item = &'static str> {
        self.modules.keys().copied()
    }

    /// How many there are.
    #[must_use]
    pub fn amount(&self) -> usize {
        self.modules.len()
    }
}

/// The ids in [`OFF_VARIABLE`]'s value: comma-separated, blanks and empty
/// pieces ignored — the unit sets it to the empty string when nothing is off.
#[must_use]
pub fn off_of(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Refuses a malformed or repeated name among one kind of thing a module
/// declares.
fn names<'a>(
    module: &'static str,
    of: &'static str,
    all: impl Iterator<Item = &'a str>,
) -> Result<(), Failure> {
    let mut seen: Vec<&str> = Vec::new();
    for name in all {
        if !kebab(name) {
            return Err(Failure::MalformedName {
                module,
                of,
                name: name.to_owned(),
            });
        }
        if seen.contains(&name) {
            return Err(Failure::RepeatedName {
                module,
                of,
                name: name.to_owned(),
            });
        }
        seen.push(name);
    }
    Ok(())
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
    use crate::fake::{Fake, manifest};
    use crate::module::{Level, Manifest, NoticeKind};
    use crate::task::Step;

    fn one(id: &'static str) -> Box<dyn Module> {
        Fake::routine(id).boxed()
    }

    #[test]
    fn a_malformed_id_never_reaches_a_screen() {
        // `brain-task-Atualizar Sistema` is not a key anybody can write in a
        // `.ftl`, and the screen would draw it in brackets. Refusing here is
        // the daemon not starting, with the id in the message.
        for bad in [
            "",
            "-fixture",
            "fixture-",
            "fix--ture",
            "fixTure",
            "fix ture",
        ] {
            assert_eq!(
                Registry::of(vec![one(bad)]).err(),
                Some(Failure::MalformedId(bad)),
                "it accepted `{bad}`"
            );
        }
        assert!(Registry::of(vec![one("fixture")]).is_ok());
    }

    #[test]
    fn two_modules_by_one_name_are_refused() {
        let e = Registry::of(vec![one("fixture"), one("fixture")]).err();
        assert_eq!(e, Some(Failure::RepeatedId("fixture")));
    }

    #[test]
    fn a_steps_id_is_held_to_the_same_rule() {
        static BAD: &[Step] = &[Step {
            id: "Build",
            command: "a",
            needs_root: true,
            on_by_default: true,
            name_key: "",
            does_key: "",
        }];
        static TWICE: &[Step] = &[
            Step {
                id: "build",
                command: "a",
                needs_root: true,
                on_by_default: true,
                name_key: "",
                does_key: "",
            },
            Step {
                id: "build",
                command: "b",
                needs_root: true,
                on_by_default: true,
                name_key: "",
                does_key: "",
            },
        ];
        assert!(matches!(
            Registry::of(vec![Fake::routine("fixture").steps(BAD).boxed()]).err(),
            Some(Failure::MalformedName { of: "step", .. })
        ));
        assert!(matches!(
            Registry::of(vec![Fake::routine("fixture").steps(TWICE).boxed()]).err(),
            Some(Failure::RepeatedName { of: "step", .. })
        ));
    }

    #[test]
    fn two_notices_by_one_name_are_one_dismissal_too_many() {
        // A notice is dismissed BY ID: two by one name would be one press
        // putting two things away.
        let twice = Manifest {
            notices: &[
                NoticeKind {
                    id: "due",
                    level: Level::Info,
                    dismiss: true,
                    snooze: &[],
                },
                NoticeKind {
                    id: "due",
                    level: Level::Alarm,
                    dismiss: true,
                    snooze: &[],
                },
            ],
            ..manifest("fixture")
        };
        assert!(matches!(
            Registry::of(vec![Fake::new(twice).boxed()]).err(),
            Some(Failure::RepeatedName { of: "notice", .. })
        ));
    }

    #[test]
    fn the_order_is_the_same_every_time() {
        let r = Registry::of(vec![one("zebra"), one("alpha")]).expect("the registry");
        let ids: Vec<&str> = r.all().map(|m| m.manifest().id).collect();
        assert_eq!(ids, vec!["alpha", "zebra"]);
        assert_eq!(r.ids().collect::<Vec<_>>(), vec!["alpha", "zebra"]);
        assert_eq!(r.amount(), 2);
        assert!(r.find("alpha").is_some());
        assert!(r.find("nope").is_none());
        assert_eq!(Registry::none().amount(), 0);
    }

    #[test]
    fn the_system_switches_off_by_id_and_an_unknown_id_is_only_said() {
        // THE UNIT'S VALUE, as `nix/modules/brain.nix` writes it: empty when
        // nothing is off, and never a piece that is blank.
        assert!(off_of("").is_empty());
        assert_eq!(off_of("zebra, alpha,,"), ["zebra", "alpha"]);
        let r = Registry::of(vec![one("zebra"), one("alpha")])
            .expect("the registry")
            .switched_off(&off_of("alpha,gone"));
        assert!(r.off("alpha"));
        assert!(!r.off("zebra"));
        // A MODULE THIS BUILD DOES NOT HAVE is not a module that is off, and
        // it did not stop the others from registering.
        assert!(!r.off("gone"));
        assert_eq!(r.amount(), 2, "switching off took a module off the page");
    }
}
