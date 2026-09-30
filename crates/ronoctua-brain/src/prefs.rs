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
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn save(&self, p: &Prefs) -> std::io::Result<()> {
        self.config.as_deref().map_or(Ok(()), |d| write_to(d, p))?;
        // AND THE TABLE THE SCRIPT READS, beside it. See [`steps_table`] for
        // why this exists and why it is not a second authority.
        self.state
            .as_deref()
            .map_or(Ok(()), |d| write_steps_to(d, p))
    }
}

/// The file the update's own script reads to find out which steps are on.
///
/// `$XDG_STATE_HOME/ronoctua/update-steps.conf`.
pub const STEPS_FILE: &str = "update-steps.conf";

/// The person's step switches, in the shape the shell script already reads.
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
/// already has — one line, `<id> on|off` — so the script that reads that can
/// read this with the loop it already has. The alternative was teaching a
/// `writeShellApplication` to parse JSON, which is either a new runtime
/// dependency or a `grep` that will one day answer confidently and wrongly
/// about somebody's machine.
///
/// It is the same relationship the theme's compositor fragment has with
/// `theme.json`: one file is the decision and the other is the decision
/// written where a different program can read it.
///
/// # Why the STATE directory
///
/// Because the person does not write it — the brain does. The config
/// directory is for what somebody edits by hand, and an entry they made there
/// would be silently overwritten the next time they pressed a key.
///
/// # Errors
///
/// The directory cannot be created, or the file cannot be written.
pub fn write_steps_to(dir: &Path, p: &Prefs) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut out = String::from(
        "# GENERATED by ronoctua-brain out of brain.json. Do not edit by hand.\n\
         # One line per step whose switch differs from what the distro ships.\n",
    );
    for (task, mine) in &p.tasks {
        for step in &mine.steps_flipped {
            // THE TASK IS IN THE LINE, because the file is one file and the
            // brain has more than one task. The reader filters by its own id
            // — the script knows which sequence it is.
            out.push_str(task);
            out.push(' ');
            out.push_str(step);
            out.push('\n');
        }
    }
    ronoctua_xdg::write_atomically(dir, STEPS_FILE, out.as_bytes())
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
}

impl Prefs {
    /// One task's preferences, or the distro's.
    #[must_use]
    pub fn of(&self, task: &str) -> TaskPrefs {
        self.tasks.get(task).cloned().unwrap_or_default()
    }

    /// The same, but falling back to what the TASK chose rather than to what
    /// every task shares.
    ///
    /// The person's file still wins: an entry here is something they changed,
    /// and a task's own default is only what fills the gap where they have not.
    /// See [`crate::task::Task::defaults`] for why a task gets a say at all.
    #[must_use]
    pub fn of_task(&self, task: &dyn crate::task::Task) -> TaskPrefs {
        self.tasks
            .get(task.id())
            .cloned()
            .unwrap_or_else(|| task.defaults())
    }

    /// The entry to change, created from the default if it is not there yet.
    pub fn mine(&mut self, task: &str) -> &mut TaskPrefs {
        self.tasks.entry(task.to_owned()).or_default()
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
    /// Sixty. Long enough to read a line on the footer, walk back to the
    /// keyboard and press one key; short enough that it is not a dialogue
    /// somebody has to dismiss. Zero means it just runs, which is what a task
    /// with nothing to interrupt should ask for.
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
    /// what it has is a number its own author chose, in `Task::defaults`.
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
    /// step ships ON. `assets/updates.nix` ships six of its twelve OFF, and
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
    #[must_use]
    pub fn from_word(word: &str) -> Self {
        match word {
            "night" => Self::Night,
            "morning" => Self::Morning,
            "afternoon" => Self::Afternoon,
            "evening" => Self::Evening,
            _ => Self::Any,
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

    #[test]
    fn a_task_with_no_entry_gets_the_distros_answer() {
        // THE DOOR: a task added tomorrow writes nothing here, and the file
        // written by today's version still round-trips.
        let p = Prefs::default();
        assert_eq!(p.of("something-that-does-not-exist"), TaskPrefs::default());
        assert_eq!(p.of("system-update").interval_days, 7);
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
        mine.mine("system-update").interval_days = 1;
        mine.mine("system-update")
            .steps_flipped
            .insert("build".into());
        write_to(&dir, &mine).expect("write");
        assert_eq!(read_from(&dir), mine);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
