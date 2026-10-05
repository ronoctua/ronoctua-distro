//! What the person chose — and the two directories that keep the choice apart
//! from what happened.
//!
//! # Two lifetimes, two files, two directories
//!
//! The PREFERENCES are configuration: how often the machine may update itself,
//! which part of the day it should prefer, how busy is too busy. They are
//! hand-editable, they are small, and they are the person's. They live under
//! `$XDG_CONFIG_HOME/ronoctua`, next to the notification filters that already
//! live there — and that directory is already in `nix/modules/persist.nix`, so
//! an impermanent root does not take them.
//!
//! What HAPPENED — the last run, what it did, what it refused — is state:
//! machine-written, growing, and worthless to hand-edit. It lives under
//! `$XDG_STATE_HOME/ronoctua`, in [`crate::journal`].
//!
//! It is the same split `ronoctua-notify`'s store makes, with the same
//! reasoning, and it is worth repeating because the alternative is seductive:
//! one file with everything in it, which is a configuration file the machine
//! rewrites — so the person's comments and formatting are lost the first time
//! a task runs.
//!
//! # Why a failure here is not fatal
//!
//! A read-only home, a full disk, a file somebody hand-edited into invalid
//! JSON: none of those is a reason for the machine to stop deciding. Everything
//! here degrades to the distro's defaults and says so in the log — loudly, at
//! `error`, because a file that stopped parsing means every choice the person
//! made is silently back to the default, and the symptom would be an update
//! running on a day they had turned it off.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::module::Manifest;

/// The file the preferences are read from, under `$XDG_CONFIG_HOME/ronoctua`.
pub const PREFERENCES_FILE: &str = "brain.json";

/// Where the brain reads and writes — and `None` means it does not.
///
/// The roots are PARAMETERS and not something found in here, which is the rule
/// the rest of this project already states (`trash::Bin`, `who::Roster`,
/// `when::Zone`, `sidebar::Sidebar`, `notify::store::Where`). The reason is
/// written in that last one: functions that found `$XDG_CONFIG_HOME` for
/// themselves are what let a test write into the real configuration of whoever
/// ran `cargo test`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Where {
    /// `$XDG_CONFIG_HOME/ronoctua`, for the preferences.
    pub config: Option<PathBuf>,
    /// `$XDG_STATE_HOME/ronoctua`, for the journal.
    pub state: Option<PathBuf>,
}

impl Where {
    /// The pair the DAEMON uses, found once from the environment.
    #[must_use]
    pub fn of_env() -> Self {
        Self {
            config: config_dir(),
            state: state_dir(),
        }
    }

    /// The person's preferences, or the distro's when there is nowhere to read.
    #[must_use]
    pub fn prefs(&self) -> Prefs {
        self.config
            .as_deref()
            .map_or_else(Prefs::default, read_from)
    }

    /// Writes the preferences. Writing nowhere is not a failure.
    ///
    /// Only `brain.json`. A module that keeps a view of its own preferences
    /// for a unit to read (a file of step switches a script reads) writes it
    /// in [`crate::module::Module::prefs_saved`], which the core calls after
    /// this — the core does not know which module keeps which file.
    ///
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn save(&self, p: &Prefs) -> std::io::Result<()> {
        self.config.as_deref().map_or(Ok(()), |d| write_to(d, p))
    }
}

/// `$XDG_CONFIG_HOME/ronoctua` and `$XDG_STATE_HOME/ronoctua`.
///
/// They were written here once, with a note saying that a third caller
/// should get a crate of its own rather than a dependency on whichever crate
/// happened to have them first. The theme was that caller, and
/// `ronoctua-xdg` is that crate: zero dependencies, so this daemon carries
/// nothing it did not before.
pub use ronoctua_xdg::{config_dir, state_dir};

/// Everything the person chose.
///
/// # Why the tasks are a MAP and not one field per task
///
/// Because the door has to stay one line wide. A record with a field per task
/// means adding a feature edits this struct, the serde default, the wire, and
/// every test that builds one — and a file written by an older version stops
/// round-tripping. A map keyed by the task's id means a new task needs nothing
/// here at all: an id that is absent gets [`TaskPrefs::default`], and an id
/// that is present and no longer exists is ignored with a line in the log
/// rather than a refusal to start.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Prefs {
    /// The general gate. Every task passes it, and the RUNNER asks — not the
    /// task, so a feature added tomorrow inherits it without writing a line.
    pub gate: Gate,
    /// One entry per task that has preferences, by its id.
    pub tasks: BTreeMap<String, TaskPrefs>,
    /// NOTHING STARTS BY ITSELF: no run comes due by its calendar or because
    /// the machine wants it, and every row says so. What a person asks for
    /// still runs, and every watch still listens.
    ///
    /// # Why a switch of its own
    ///
    /// Because the machines that want it are the TESTS: a virtual machine
    /// that boots the distro to photograph a window must not start a rebuild
    /// halfway through the photograph, and saying so used to mean naming the
    /// two tasks that could (the update and the disk's relief) in four test
    /// files — which is four places a module's id lives outside its folder,
    /// and four places the next module that starts something is forgotten.
    /// `{"quiet": true}` names nobody.
    pub quiet: bool,
}

impl Prefs {
    /// One module's preferences, RESOLVED: what the person chose, over what
    /// the module declared ([`Manifest::defaults`]).
    ///
    /// # Why this is the only way to read one
    ///
    /// Because there were four, and three of them fell back to the SHARED
    /// default (`TaskPrefs::default`: weekly, in the afternoon, a minute of
    /// countdown) about a module that declared its own. Switching the Emacs
    /// watch off and on again from its page turned a six-hour watch into a
    /// weekly run with a countdown. `Prefs::of` and `Prefs::mine` are gone,
    /// and this takes the manifest, so the defect cannot be written again.
    ///
    /// # What the person's entry decides, and what it does not
    ///
    /// The KNOBS (`wire::Knob`) — whether it runs, how often, when in the day
    /// and which steps — are the person's, and an entry wins for them. How
    /// long it warns, how patient the runner is and a cadence in minutes are
    /// the MODULE's (`schedule` in its manifest), whatever the entry says:
    /// no door writes them, and an entry that carries them was written whole
    /// by the old `Tune` from the shared default — which is the defect above,
    /// still on disk. Reading them from the manifest heals those files.
    #[must_use]
    pub fn of_module(&self, manifest: &Manifest) -> TaskPrefs {
        let declared = manifest.defaults();
        match self.tasks.get(manifest.id) {
            None => declared,
            Some(chosen) => TaskPrefs {
                enabled: chosen.enabled,
                interval_days: chosen.interval_days,
                part_of_day: chosen.part_of_day,
                steps_flipped: chosen.steps_flipped.clone(),
                ..declared
            },
        }
    }

    /// The person's entry for a module, to change — created from what the
    /// MODULE declared, never from the shared default.
    ///
    /// For the core's `Ask::Set`, which applies one knob to it; nothing else
    /// writes an entry.
    pub(crate) fn chosen_mut(&mut self, manifest: &Manifest) -> &mut TaskPrefs {
        self.tasks
            .entry(manifest.id.to_owned())
            .or_insert_with(|| manifest.defaults())
    }
}

/// How busy is too busy — the general precondition, in one place.
///
/// Every number here is a POLICY and not a measurement, which is why they are
/// in a file the person owns rather than in the code. The defaults err towards
/// NOT running: a skipped week costs a week, and a machine that starts
/// compiling while somebody is typing costs the session they were in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Gate {
    /// Refuse above this much RAM in use, in per cent.
    ///
    /// Seventy: above it the applications' slice on this machine is already
    /// near the ceiling `nix/modules/memory.nix` sets for it, and adding a
    /// build there is asking `systemd-oomd` to choose between the update and
    /// the browser. It is not a measurement of what a build needs — it is the
    /// line past which the machine belongs to whoever is at the keyboard.
    pub ram_ceiling_pct: u16,
    /// Refuse above this much processor stall, in per mille of a ten-second
    /// window.
    ///
    /// Two hundred is a fifth of ten seconds — two whole seconds, in the last
    /// ten, in which something runnable was waiting for a core. It is the same
    /// quantity and the same unit `ManagedOOMMemoryPressureLimit` already
    /// compares against, read from `/proc/pressure/cpu`: a fraction of TIME,
    /// never of hardware.
    pub cpu_stall_per_mille: u16,
    /// Refuse the same way when the machine is running off its battery.
    ///
    /// `true`, because the one task that exists COMPILES: a rebuild on this
    /// distro builds the Rust workspace, and a laptop that goes flat halfway
    /// through has spent its charge on work it threw away. A machine with no
    /// battery is not held — see [`crate::pulse::Pulse::on_battery`], where
    /// "there is no battery" and "it is plugged in" are different answers.
    pub abort_on_battery: bool,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            ram_ceiling_pct: 70,
            cpu_stall_per_mille: 200,
            abort_on_battery: true,
        }
    }
}

/// What one task's schedule looks like to the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TaskPrefs {
    /// Whether it may run at all. Turning it off leaves the row on screen,
    /// saying so: a feature that disappears when it is switched off is a
    /// feature nobody switches back on.
    pub enabled: bool,
    /// Days between runs.
    ///
    /// Seven. It is the cadence the machine already keeps for the `tealdeer`
    /// cache, and one generation a week is a rollback list a person can still
    /// read — the count of them the system keeps (`systemGenerations`, in
    /// `assets/cache.nix`) is about a month. There is no weekly `nix.gc` any
    /// more: the update's own collection is the weekly one, and Nix's floor
    /// covers the days between (`nix/modules/store.nix`). Daily is available
    /// and is one key away; it is not the default because nothing here is
    /// urgent.
    pub interval_days: u16,
    /// Which part of the day it should prefer.
    pub part_of_day: PartOfDay,
    /// How long the screen warns before it starts, in seconds.
    ///
    /// THE MODULE'S, and not written into `brain.json`: see
    /// [`Prefs::of_module`]. The same holds for `runaway_minutes` and
    /// `every_minutes` below.
    ///
    /// Sixty. Long enough to read a line on the footer, walk back to the
    /// keyboard and press one key; short enough that it is not a dialogue
    /// somebody has to dismiss. Zero means it just runs, which is what a task
    /// with nothing to interrupt should ask for.
    #[serde(skip_serializing)]
    pub countdown_seconds: u16,
    /// After how many minutes with nothing new in the report the run is called
    /// runaway.
    ///
    /// A hundred and twenty. A full rebuild of this machine downloads and
    /// compiles, and the honest worst case is well inside two hours — but the
    /// ceiling is not a measurement of a build, it is a ceiling on the QUEUE: a
    /// run that never ends would hold every other task for the rest of the
    /// session. Past it the run is recorded as `unknown` AND SAYS SO, because
    /// the unit is systemd's and may perfectly well still be going.
    #[serde(skip_serializing)]
    pub runaway_minutes: u16,
    /// A cadence in MINUTES, for a task that cannot wait for tomorrow.
    ///
    /// # Why a second unit exists at all
    ///
    /// Because [`Self::interval_days`] is the right unit for everything the
    /// brain did until now — a rebuild, a sweep, a bin — and the wrong one for
    /// a WATCH. The Emacs daemon can stop at ten in the morning, and a check
    /// that came round tomorrow would leave somebody without their editor's
    /// session for a day while the screen said everything was in hand.
    ///
    /// `None` for every task that thinks in days, which is all but one. When
    /// it is set, [`crate::schedule::next_moment`] counts in minutes and the
    /// part of the day is not consulted: a watch has no good hour, it has an
    /// interval.
    ///
    /// It is NOT a knob. The cadence ladder the screen walks is in days, and a
    /// task that thinks in minutes has nothing on that ladder to walk to —
    /// what it has is a number its own author chose, in its manifest.
    #[serde(skip_serializing)]
    pub every_minutes: Option<u32>,
    /// The steps the person moved AWAY FROM the default, by id.
    ///
    /// A set of exceptions and not a list of what to run: the order and the
    /// contents belong to the task, so a step added by a later version takes
    /// its declared default instead of being missing from everybody's file.
    ///
    /// # Why it is a FLIP and not an "off" list
    ///
    /// Because it was an "off" list, and an off list quietly assumes every
    /// step ships ON. The update's table ships six of its twelve OFF, and
    /// the arithmetic downstream was `on = !contains(id)`. So on a machine
    /// where nobody had touched anything — the set empty, no `brain.json` at
    /// all — every one of those six drew as SWITCHED ON, with a tick, and
    /// carried the note "you changed". The owner read six of those on a page
    /// where they had changed nothing and asked what it meant.
    ///
    /// Pressing the key made it worse in the other direction: the step went
    /// INTO the set, `on` became false and `changed` became false, so the row
    /// read "not touched, ships off" immediately after they touched it.
    ///
    /// As a flip both answers fall out of one line —
    /// `on = on_by_default ^ flipped`, `changed = flipped` — and the doc
    /// comment's promise above survives, which is what matters for a file on
    /// somebody's disk.
    ///
    /// The old spelling is kept as an alias. Under the old reading an entry
    /// could only ever be a step that ships on and was switched off, and under
    /// this one that is exactly what it still means, so nothing on disk has to
    /// be migrated or lost.
    #[serde(alias = "steps-off")]
    pub steps_flipped: BTreeSet<String>,
}

impl Default for TaskPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_days: 7,
            part_of_day: PartOfDay::Afternoon,
            countdown_seconds: 60,
            runaway_minutes: 120,
            every_minutes: None,
            steps_flipped: BTreeSet::new(),
        }
    }
}

/// Which stretch of the person's day a task should prefer.
///
/// # Why a name and not two numbers
///
/// Because "03:00" is a decision about a machine and "at night" is a decision
/// about a life. The person chooses the second; the first is arithmetic, and it
/// is here — in one place, readable, with the reason for each edge written
/// beside it — rather than in a pair of fields somebody has to reason about
/// twice a year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PartOfDay {
    /// Whenever, as soon as the gate lets it.
    Any,
    /// 02:00 to 05:00. Nobody is at the keyboard and the network is quiet — and
    /// the machine is awake, because this distro does not suspend on its own.
    Night,
    /// 08:00 to 12:00.
    Morning,
    /// 13:00 to 18:00. The default: the machine is on, somebody is nearby to
    /// see the countdown, and a rebuild that goes wrong is found in daylight
    /// rather than at the next boot.
    Afternoon,
    /// 19:00 to 23:00.
    Evening,
}

impl PartOfDay {
    /// The local hours it covers: from, and up to but not including.
    ///
    /// `Any` is the whole day, which makes the window arithmetic in
    /// [`crate::schedule`] one case instead of two.
    #[must_use]
    pub const fn hours(self) -> (u32, u32) {
        match self {
            Self::Any => (0, 24),
            Self::Night => (2, 5),
            Self::Morning => (8, 12),
            Self::Afternoon => (13, 18),
            Self::Evening => (19, 23),
        }
    }

    /// The window a word names — the inverse of [`PartOfDay::word`].
    ///
    /// # Why it takes a word at all
    ///
    /// Because `assets/cache.nix` declares WHEN the sweep runs, and a
    /// declaration in Nix can only say a word. `nix/cache.nix` refuses a word
    /// this enum does not know, at BUILD time, so the fallback below is
    /// unreachable from the catalogue — it is here for a preferences file
    /// somebody edited by hand, where "whenever the gate allows" is the
    /// harmless answer and a panic would be a daemon that will not start.
    ///
    /// A `const fn`, because a manifest is a `static` and the sweep's window
    /// is a word from `assets/cache.nix`.
    #[must_use]
    pub const fn from_word(word: &str) -> Self {
        if same(word, "night") {
            Self::Night
        } else if same(word, "morning") {
            Self::Morning
        } else if same(word, "afternoon") {
            Self::Afternoon
        } else if same(word, "evening") {
            Self::Evening
        } else {
            Self::Any
        }
    }

    /// The word the screen looks up, as `brain-part-<word>`.
    ///
    /// A word and not a sentence: this crate does not translate (invariant 8),
    /// and whoever has the bundle turns it into one.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Night => "night",
            Self::Morning => "morning",
            Self::Afternoon => "afternoon",
            Self::Evening => "evening",
        }
    }

    /// The next one, cycling — what one key press does.
    ///
    /// The order is the day's own, with `any` at the end: stepping through
    /// night, morning, afternoon and evening is a person walking forward
    /// through their day, and "whenever" is not a time of day at all.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Night => Self::Morning,
            Self::Morning => Self::Afternoon,
            Self::Afternoon => Self::Evening,
            Self::Evening => Self::Any,
            Self::Any => Self::Night,
        }
    }
}

/// Two words, byte for byte — `==` on `str` is not `const`.
const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The cadences one key steps through, in days.
///
/// A LADDER and not `+1`, because the difference that matters is daily against
/// weekly against monthly: stepping from thirty to twenty-nine costs a key
/// press and changes nothing anybody can feel, and getting from thirty to seven
/// would cost twenty-three of them.
pub const CADENCE: &[u16] = &[1, 2, 3, 7, 14, 30];

/// The next cadence up the ladder, or the same one at the top.
///
/// A value that is not ON the ladder — somebody typed `9` into the file — moves
/// to the first rung above it, which is what makes hand-editing safe rather
/// than something the screen fights.
#[must_use]
pub fn less_often(days: u16) -> u16 {
    CADENCE
        .iter()
        .copied()
        .find(|c| *c > days)
        .unwrap_or_else(|| CADENCE.last().copied().unwrap_or(days))
}

/// The next cadence down the ladder, or the same one at the bottom.
#[must_use]
pub fn more_often(days: u16) -> u16 {
    CADENCE
        .iter()
        .copied()
        .rev()
        .find(|c| *c < days)
        .unwrap_or_else(|| CADENCE.first().copied().unwrap_or(days))
}

/// Reads the preferences from an explicit directory.
#[must_use]
pub fn read_from(dir: &Path) -> Prefs {
    let path = dir.join(PREFERENCES_FILE);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Prefs::default();
    };
    match serde_json::from_str::<Prefs>(&raw) {
        Ok(p) => {
            tracing::info!(path = %path.display(), tarefas = p.tasks.len(), "the brain's preferences were read");
            p
        }
        Err(e) => {
            // LOUD, and not silent. A hand-edited file that stopped parsing
            // means every choice the person made is back to the distro's, and
            // the symptom would be a rebuild starting on a day they had
            // switched it off.
            tracing::error!(
                path = %path.display(),
                error = %e,
                "the brain's preferences are unreadable; carrying on with the distro's"
            );
            Prefs::default()
        }
    }
}

/// Writes the preferences into an explicit directory.
///
/// # Errors
///
/// The directory cannot be created, or the file cannot be written.
pub fn write_to(dir: &Path, p: &Prefs) -> std::io::Result<()> {
    // PRETTY, and the only file in this crate that is: a person opens this one
    // by hand. The journal is machine-written and stays compact.
    let text = serde_json::to_string_pretty(p).map_err(std::io::Error::other)?;
    // ATOMICALLY, like every other store in this tree. It was a plain
    // `fs::write` — this crate already linked `ronoctua-xdg` for the roots and
    // simply did not use the third function in it — so a kill or a full disk
    // in the middle left the person with half of their own preferences.
    ronoctua_xdg::write_atomically(dir, PREFERENCES_FILE, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module with a cadence of its own: every five days, at night, no
    /// countdown, half an hour of patience.
    fn fussy() -> crate::module::Manifest {
        let mut m = crate::fake::manifest("fussy");
        m.schedule = Some(crate::module::Schedule {
            interval_days: 5,
            part_of_day: PartOfDay::Night,
            countdown_seconds: 0,
            runaway_minutes: 30,
            ..crate::fake::ROUTINE
        });
        m
    }

    #[test]
    fn a_module_with_no_entry_gets_what_it_declared() {
        // THE DOOR: a module added tomorrow writes nothing here, and what it
        // gets is ITS declaration — not the shared default.
        let p = Prefs::default();
        let m = fussy();
        let mine = p.of_module(&m);
        assert_eq!(mine, m.defaults());
        assert_eq!(
            (mine.interval_days, mine.part_of_day),
            (5, PartOfDay::Night)
        );
        assert_eq!(mine.countdown_seconds, 0);
    }

    #[test]
    fn an_entry_decides_the_knobs_and_never_what_the_module_declared() {
        // THE DEFECT, AS IT SITS ON DISK: the old `Tune` wrote a whole entry
        // from the SHARED default — a minute of countdown, two hours of
        // patience — about a module that declared none of that. The person's
        // choices in it still count; the module's own fields do not.
        let m = fussy();
        let mut p = Prefs::default();
        p.tasks.insert(
            "fussy".to_owned(),
            TaskPrefs {
                enabled: false,
                interval_days: 14,
                ..TaskPrefs::default()
            },
        );
        let mine = p.of_module(&m);
        assert!(!mine.enabled, "the person switched it off");
        assert_eq!(mine.interval_days, 14, "the person chose a fortnight");
        assert_eq!(mine.countdown_seconds, 0, "the shared minute came back");
        assert_eq!(mine.runaway_minutes, 30, "the shared patience came back");

        // AND AN ENTRY BORN FROM A KNOB is born from the module, so switching
        // off and on again changes nothing else.
        let mut p = Prefs::default();
        p.chosen_mut(&m).enabled = false;
        p.chosen_mut(&m).enabled = true;
        assert_eq!(p.of_module(&m), m.defaults());
        assert_eq!(p.tasks["fussy"].part_of_day, PartOfDay::Night);
    }

    #[test]
    fn the_file_holds_the_persons_choices_and_not_the_modules() {
        // What `brain.json` says is what somebody can change: a countdown in
        // it would be a number a person edits and nothing reads.
        let mut p = Prefs::default();
        p.chosen_mut(&fussy()).enabled = false;
        let text = serde_json::to_string(&p).expect("encode");
        assert!(!text.contains("countdown"), "{text}");
        assert!(!text.contains("runaway"), "{text}");
        assert!(!text.contains("every-minutes"), "{text}");
        assert!(text.contains(r#""quiet":false"#), "{text}");
        // AND A TEST'S ONE LINE reads as a quiet brain, whatever else is not
        // there.
        let quiet: Prefs = serde_json::from_str(r#"{"quiet": true}"#).expect("parse");
        assert!(quiet.quiet);
        assert!(quiet.tasks.is_empty());
    }

    #[test]
    fn a_window_word_is_read_at_compile_time() {
        const NIGHT: PartOfDay = PartOfDay::from_word("night");
        assert_eq!(NIGHT, PartOfDay::Night);
        for p in [
            PartOfDay::Any,
            PartOfDay::Night,
            PartOfDay::Morning,
            PartOfDay::Afternoon,
            PartOfDay::Evening,
        ] {
            assert_eq!(PartOfDay::from_word(p.word()), p);
        }
        assert_eq!(PartOfDay::from_word("nightt"), PartOfDay::Any);
        assert_eq!(PartOfDay::from_word(""), PartOfDay::Any);
    }

    #[test]
    fn the_ladder_moves_by_rungs_and_stops_at_the_ends() {
        assert_eq!(less_often(7), 14);
        assert_eq!(more_often(7), 3);
        assert_eq!(less_often(30), 30, "it went past the top");
        assert_eq!(more_often(1), 1, "it went past the bottom");
        // A number somebody typed into the file, off the ladder: it climbs to
        // the next rung instead of standing still.
        assert_eq!(less_often(9), 14);
        assert_eq!(more_often(9), 7);
    }

    #[test]
    fn the_parts_of_the_day_do_not_overlap_and_the_cycle_closes() {
        let all = [
            PartOfDay::Night,
            PartOfDay::Morning,
            PartOfDay::Afternoon,
            PartOfDay::Evening,
        ];
        for (i, one) in all.iter().enumerate() {
            let (from, to) = one.hours();
            assert!(from < to, "{one:?} starts after it ends");
            for other in &all[i + 1..] {
                let (a, b) = other.hours();
                assert!(to <= a || b <= from, "{one:?} e {other:?} se sobrepoem");
            }
        }
        // Five steps come back to where they started, which is what makes one
        // key enough to reach every one of them.
        let mut at = PartOfDay::Afternoon;
        for _ in 0..5 {
            at = at.next();
        }
        assert_eq!(at, PartOfDay::Afternoon);
    }

    #[test]
    fn a_file_that_stopped_parsing_is_the_distros_answer_and_not_a_panic() {
        let dir = std::env::temp_dir().join(format!("ronoctua-brain-prefs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join(PREFERENCES_FILE), "{ this is not json").expect("write");
        assert_eq!(read_from(&dir), Prefs::default());

        // And what is written comes back the same, which is the property the
        // screen depends on when it changes one field.
        let mut mine = Prefs::default();
        let m = fussy();
        mine.chosen_mut(&m).interval_days = 1;
        mine.chosen_mut(&m).steps_flipped.insert("build".into());
        // THE MODULE'S FIELDS ARE NOT WRITTEN, so what comes back is the
        // person's choices and the shared default for the rest — which
        // `of_module` then replaces with the module's.
        let entry = mine.tasks.get_mut("fussy").expect("entry");
        entry.countdown_seconds = TaskPrefs::default().countdown_seconds;
        entry.runaway_minutes = TaskPrefs::default().runaway_minutes;
        write_to(&dir, &mine).expect("write");
        assert_eq!(read_from(&dir), mine);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
