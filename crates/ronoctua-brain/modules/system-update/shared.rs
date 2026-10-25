//! THE UPDATE'S SHARED HALF: what the update's doors read of the same machine
//! and the same declaration as the module — the steps it draws, the battery
//! as the sequence weighs it, the word a held run waits for, and the
//! machine's own records beside a run.
//!
//! It is compiled into the brain's LIBRARY (`ronoctua-brain`), beside the
//! manifest, because the update's window and view (`update_window.rs`, which
//! this module's `namedBy` lists) read it; `module.rs`, the daemon's half,
//! sees it through `super::`. So the door's question and the module's hold
//! read one battery ([`Charge::read_at`]), and the page and the run one table
//! of steps ([`steps`]). Nothing here runs, holds or watches: that is
//! `module.rs`.

use std::path::PathBuf;

use crate::task::{Hold, Step};

/// Why the machine's own program refused a step (`ronoctua-machine`, whose
/// last line is `refused <word>`): the report keeps the word
/// (`failed-<word>`), and the view says the machine's sentence for it.
pub use ronoctua_machine::Why as MachineRefusal;
/// The boot trial's outcome and why an apply armed none, as the machine's
/// record says them — named here so the update's view reads them through
/// this module.
pub use ronoctua_machine::{NoTrial, TrialOutcome};

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

/// The hold a run gets on a METERED way out — this module's alone (the
/// general gate holds nothing for the network), so a door that asked for a
/// run by hand and sees its row held by it knows the brain is waiting for
/// another network, and asks the person whether to spend this one.
pub const HOLD_METERED: Hold = Hold::Network;

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

/// The key of a refusal a step reads as news (`declined-<word>` in its
/// report), or `None` for a word no step declares.
#[must_use]
pub fn declined_key(word: &str) -> Option<&'static str> {
    declared::declined_key(word)
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
    /// The machine directory.
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
        let mut out = vec![self.machine_state.join(ronoctua_person::names::STATE)];
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
        Self {
            conflicts: at.belongings.as_ref().map_or(0, plain_conflicts),
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

#[cfg(test)]
mod tests {
    use super::*;

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
    /// trial and the hold from the machine's record, the conflicts of the
    /// CURRENT repository from the belongings' last plain run — and an
    /// absent or broken file is an empty answer, never an invented one.
    #[test]
    fn the_records_say_the_trial_the_hold_and_the_conflicts() {
        let dir = Scratch::new("update-records");
        let machine_dir = dir.0.join("machine");
        let machine_state = dir.0.join("state");
        let home = dir.0.join("home");
        let belongings = ronoctua_belongings::places::Places::under(&home);
        let at = RecordPlaces {
            machine_dir,
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
