//! THE CONTRACT A MODULE ANSWERS — the daemon's half of the module.
//!
//! What a module DECLARES and PUBLISHES — its manifest, its marks, notices,
//! items and page, the words and arguments of a sentence — is the library's
//! (`ronoctua_brain::module`), re-exported here whole, so `crate::module::…`
//! names both halves and a module's `module.rs` reads as it always did. What
//! is here is what only the daemon runs: the three traits ([`Module`],
//! [`Scheduled`], [`Watcher`]), what a module is handed while it answers
//! ([`Look`], [`Ctx`] and its [`Sink`], a [`Run`]), what a person asks of it
//! ([`Act`], [`Acted`]) and what it listens to ([`Source`], [`Heard`]).
//!
//! # Three faces, all optional
//!
//! - [`Scheduled`] — the old `Task`: a run the runner queues, gates, announces
//!   and journals. It receives the person's preferences ALREADY RESOLVED over the
//!   module's own defaults ([`Manifest::defaults`]), so a module cannot read a
//!   default the screen does not show — the defect `Prefs::mine` had.
//! - [`Watcher`] — what the watch thread feeds: journal entries, a path that
//!   changed, lines of a long-lived child, and a clock armed only while the
//!   module asks for one ([`Watcher::next_wake`]).
//! - [`Module::face`] and [`Module::screen`] — what is published: marks on the
//!   modeline, notices on the brain's strip, one-shot items, and the rows of the
//!   module's own page. DATA, never sentences: keys, numbers and moments, and
//!   whoever draws translates and formats (invariants 8 and 11).
//!
//! The deviations from the design (`brain.md` §2) are written beside the
//! vocabulary, in the library's own `module`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub use ronoctua_brain::module::*;

use crate::moment::Moment;
use crate::prefs::{TaskPrefs, Where};
use crate::pulse::Pulse;
use crate::task::{Due, Outcome, Report, RunRecord, Sheet, Step, Verdict, Watched};

// ---------------------------------------------------------------------------
// What a module may look at.
// ---------------------------------------------------------------------------

/// What a module may look at when it answers [`Module::face`] or
/// [`Module::screen`] — handed in, so both stay pure over the module's own
/// state and this.
#[derive(Debug, Clone, Copy)]
pub struct Look<'a> {
    /// Now, by the daemon's clock.
    pub now: Moment,
    /// The pulse the runner last took.
    pub pulse: &'a Pulse,
    /// The person's preferences for it, resolved over its defaults.
    pub mine: &'a TaskPrefs,
    /// Its last record in the journal, if any.
    pub last: Option<&'a RunRecord>,
}

// ---------------------------------------------------------------------------
// What a person asks, and where what a module tells goes.
// ---------------------------------------------------------------------------

/// A person asked for one of a module's actions (`Ask::Act`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Act {
    /// Which, by id — one of the manifest's `actions`.
    pub action: String,
    /// The row it was asked on, for an action with `row = true`.
    pub row: Option<String>,
    /// What the person typed, for an action that `asks` first.
    pub answer: Option<String>,
}

/// What became of an [`Act`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acted {
    /// Done; the module's face is published again.
    Done,
    /// Put the module's scheduled run in the queue — the gate, the countdown
    /// and the journal apply as to any run.
    Queue,
    /// Not done, and the WORD says why: a core word (`unknown`) or one of the
    /// module's `whys`.
    Refused(&'static str),
}

/// Where what a module does goes: the journal, the mind, the footer.
///
/// A trait so the core writes the real one and a test keeps everything in a
/// [`Kept`]. Every call is quick and never blocks on a module.
pub trait Sink: Send + Sync {
    /// Write a record into the activity log.
    fn record(&self, module: &'static str, at: Moment, noted: Noted);
    /// Tell the footer something once.
    fn tell(&self, module: &'static str, event: Event);
    /// The module's face changed; publish it again.
    fn changed(&self, module: &'static str);
}

/// What a module may do while it answers: the one door out of it.
pub struct Ctx {
    module: &'static str,
    now: Moment,
    roots: Where,
    rehearsing: bool,
    sink: Arc<dyn Sink>,
}

impl std::fmt::Debug for Ctx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx")
            .field("module", &self.module)
            .field("now", &self.now)
            .field("roots", &self.roots)
            .field("rehearsing", &self.rehearsing)
            .finish_non_exhaustive()
    }
}

impl Ctx {
    /// One, for one module, at one moment.
    #[must_use]
    pub fn new(
        module: &'static str,
        now: Moment,
        roots: Where,
        rehearsing: bool,
        sink: Arc<dyn Sink>,
    ) -> Self {
        Self {
            module,
            now,
            roots,
            rehearsing,
            sink,
        }
    }

    /// One that keeps everything, nowhere on disk — what a test hands in —
    /// and the keeper to read it back from.
    #[must_use]
    pub fn kept(module: &'static str, now: Moment) -> (Self, Arc<Kept>) {
        let kept = Arc::new(Kept::default());
        let sink: Arc<dyn Sink> = kept.clone();
        (Self::new(module, now, Where::default(), false, sink), kept)
    }

    /// Whose.
    #[must_use]
    pub const fn module(&self) -> &'static str {
        self.module
    }

    /// Now, by the daemon's clock.
    #[must_use]
    pub const fn now(&self) -> Moment {
        self.now
    }

    /// Whether this is a rehearsal: a dry run that changes nothing, and whose
    /// records say so.
    #[must_use]
    pub const fn rehearsing(&self) -> bool {
        self.rehearsing
    }

    /// `$XDG_STATE_HOME/ronoctua/brain/<id>`, or `None` for nowhere.
    #[must_use]
    pub fn state_dir(&self) -> Option<PathBuf> {
        own_dir(self.roots.state.as_deref(), self.module)
    }

    /// `$XDG_CONFIG_HOME/ronoctua/brain/<id>`, or `None` for nowhere.
    #[must_use]
    pub fn config_dir(&self) -> Option<PathBuf> {
        own_dir(self.roots.config.as_deref(), self.module)
    }

    /// Writes a record into the activity log.
    pub fn record(&self, noted: Noted) {
        self.sink.record(self.module, self.now, noted);
    }

    /// Tells the footer something once.
    pub fn tell(&self, event: Event) {
        self.sink.tell(self.module, event);
    }

    /// Says the face changed, so it is published again.
    pub fn changed(&self) {
        self.sink.changed(self.module);
    }

    /// The same door, for work that outlives the call it was handed to.
    ///
    /// An `act` is answered on the socket's thread, which a client waits on
    /// for half a second at most; work that may take longer — a call on the
    /// system bus that has to start a daemon first — goes to a thread of its
    /// own, and this is how that thread still says what came of it: the face
    /// moved, and what it noticed goes into the activity log. `now` stays the
    /// moment of the call.
    #[must_use]
    pub fn detached(&self) -> Self {
        Self {
            module: self.module,
            now: self.now,
            roots: self.roots.clone(),
            rehearsing: self.rehearsing,
            sink: Arc::clone(&self.sink),
        }
    }
}

/// A [`Sink`] that keeps what it was given, for a test to read back.
#[derive(Debug, Default)]
pub struct Kept {
    records: Mutex<Vec<(String, Moment, Noted)>>,
    events: Mutex<Vec<(String, Event)>>,
    changes: Mutex<Vec<String>>,
}

impl Kept {
    /// Every record, in order, with whose it was and when.
    #[must_use]
    pub fn records(&self) -> Vec<(String, Moment, Noted)> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Every event, in order, with whose it was.
    #[must_use]
    pub fn events(&self) -> Vec<(String, Event)> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Whose face changed, once per call.
    #[must_use]
    pub fn changes(&self) -> Vec<String> {
        self.changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Sink for Kept {
    fn record(&self, module: &'static str, at: Moment, noted: Noted) {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((module.to_owned(), at, noted));
    }

    fn tell(&self, module: &'static str, event: Event) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((module.to_owned(), event));
    }

    fn changed(&self, module: &'static str) {
        self.changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(module.to_owned());
    }
}

/// One run of a scheduled module: the old `Report` (the file the unit writes,
/// and the patience to watch it) and the [`Ctx`] (the door out).
///
/// `say` and `relieve` are gone: a notice is part of the module's [`Face`], and
/// a relief is an [`Event`] told through [`Ctx::tell`].
#[derive(Debug)]
pub struct Run {
    report: Report,
    ctx: Ctx,
}

impl Run {
    /// One run.
    #[must_use]
    pub const fn new(report: Report, ctx: Ctx) -> Self {
        Self { report, ctx }
    }

    /// The door out.
    #[must_use]
    pub const fn ctx(&self) -> &Ctx {
        &self.ctx
    }

    /// The report file and its watching.
    #[must_use]
    pub const fn report(&self) -> &Report {
        &self.report
    }

    /// Whether this run is a rehearsal.
    #[must_use]
    pub const fn is_rehearsal(&self) -> bool {
        self.report.is_rehearsal()
    }

    /// Where the unit writes — what it is told, and what a person can `cat`.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.report.path()
    }

    /// What the report file looks like now, before the unit is started.
    #[must_use]
    pub fn mark(&self) -> Option<std::time::SystemTime> {
        self.report.mark()
    }

    /// Watches until the unit says it ended, the patience runs out, or somebody
    /// aborts.
    #[must_use]
    pub fn watch(&self, since: Option<std::time::SystemTime>) -> Watched {
        self.report.watch(since)
    }

    /// What the report says now.
    #[must_use]
    pub fn sheet(&self) -> Option<Sheet> {
        self.report.sheet()
    }
}

// ---------------------------------------------------------------------------
// What a watcher listens to.
// ---------------------------------------------------------------------------

/// What a watcher listens to. Read ONCE, when the watch thread starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Journal entries that pass this match. One follower serves every module;
    /// it is started only if some module asked for one.
    Journal(Match),
    /// A path: something changed at it, or under it (inotify, never a poll).
    Path(PathBuf),
    /// A directory and everything under it, however deep: something changed
    /// at it or anywhere below — the folders made below it after the start
    /// included, each watched from the moment its birth is heard (inotify is
    /// not recursive, and a list of the folders there at the start is deaf
    /// to every one made later). Heard as [`Heard::Path`] with the source's
    /// path. For a small tree: each folder is a watch out of the user's
    /// `max_user_watches` (`ronoctua_files::notice::TREE_MOST` at most).
    Tree(PathBuf),
    /// The lines of a long-lived child, restarted with a backoff when it dies.
    /// `argv[0]` is a STORE PATH, never a name.
    Lines {
        /// The command line.
        argv: Vec<String>,
    },
}

/// What a watcher heard. `source` is the index into what
/// [`Watcher::sources`] returned.
#[derive(Debug, Clone, Copy)]
pub enum Heard<'a> {
    /// A journal entry that passed the source's match.
    Journal {
        /// Which source.
        source: usize,
        /// The entry.
        entry: &'a Entry,
    },
    /// Something changed at, or under, a watched path.
    Path {
        /// Which source.
        source: usize,
        /// What changed.
        path: &'a Path,
    },
    /// A line from a watched child.
    Line {
        /// Which source.
        source: usize,
        /// The line, without its newline.
        line: &'a str,
    },
    /// The clock [`Watcher::next_wake`] asked for.
    Wake,
    /// A source could not be followed — the journal is unreadable, the child
    /// keeps dying. Said, so the module's page can refuse with a sentence
    /// instead of going quiet.
    ///
    /// Said when a source is DOWN, not every time it restarts: a follower
    /// that ended after it had been following is started again from its
    /// cursor and nothing was missed. Said again when the words change.
    Lost {
        /// Which source.
        source: usize,
        /// What went wrong, in the machine's words.
        why: &'a str,
    },
    /// A source said [`Heard::Lost`] is followed again. Without it a page
    /// would go on refusing after the restart worked, until a matching line
    /// happened to arrive — which for a rare match is never.
    Back {
        /// Which source.
        source: usize,
    },
}

/// The journal's vocabulary, from the one crate that follows it.
///
/// `Match` is what a module asks for ([`Source::Journal`]), `Entry` what it
/// hears ([`Heard::Journal`]), and the rest is what those two are made of. They
/// were local copies of the same shapes while `ronoctua-journal` was being
/// written; nothing that names `module::Match` changed when they were swapped.
pub use ronoctua_journal::{Backfill, Cursor, Entry, Field, Match, Priority};

// ---------------------------------------------------------------------------
// The three traits.
// ---------------------------------------------------------------------------

/// One module of the brain.
///
/// Every method but [`Module::manifest`] has a default: a module answers only
/// the questions it has.
pub trait Module: Send + Sync {
    /// What the manifest declared — `&super::MANIFEST`, generated.
    fn manifest(&self) -> &'static Manifest;

    /// What its run does, in order — DATA, so the page can draw it and the
    /// person can switch one off.
    fn steps(&self) -> &'static [Step] {
        &[]
    }

    /// Its run, when it has a schedule.
    fn scheduled(&self) -> Option<&dyn Scheduled> {
        None
    }

    /// What it listens to, when it watches.
    fn watcher(&self) -> Option<&dyn Watcher> {
        None
    }

    /// What it shows now. PURE over its own state and `look`.
    fn face(&self, _look: &Look<'_>) -> Face {
        Face::default()
    }

    /// Its own page. Built only when the host asks (`Ask::Screen`), off the
    /// footer's loop.
    fn screen(&self, _look: &Look<'_>) -> Screen {
        Screen::default()
    }

    /// A person asked for one of its actions. MUST NOT BLOCK: what takes long
    /// is its run, which [`Acted::Queue`] puts in the queue.
    fn act(&self, _act: &Act, _ctx: &Ctx) -> Acted {
        Acted::Refused("unknown")
    }

    /// A person dismissed one of its notices, by id.
    fn dismissed(&self, _notice: &str, _ctx: &Ctx) {}

    /// A person put one of its notices off until `until`.
    fn snoozed(&self, _notice: &str, _until: Moment, _ctx: &Ctx) {}

    /// The person's preferences for it were written; a module that keeps a
    /// derived view of them for a unit (the update's steps file) writes it
    /// here.
    ///
    /// # Errors
    ///
    /// The view could not be written.
    fn prefs_saved(&self, _mine: &TaskPrefs, _dirs: &Where) -> std::io::Result<()> {
        Ok(())
    }

    /// `ronoctua-brain work <id> [args…]`: the work of a long unit, in the
    /// brain's binary, writing its report at `report`.
    ///
    /// # Errors
    ///
    /// The module has no work of its own, or the work failed.
    fn work(&self, _args: &[String], _report: &Path) -> anyhow::Result<()> {
        anyhow::bail!("the module `{}` has no work of its own", self.manifest().id)
    }
}

/// A module that runs on a schedule: the old `Task`, minus what the manifest
/// now declares (id, defaults, steps, countdown, kind, gate).
///
/// `mine` is ALWAYS the person's preferences resolved over the module's own
/// defaults: there is no way here to read a default the page does not show.
pub trait Scheduled: Send + Sync {
    /// When it wants to run. PURE: `now` is handed in and nothing is opened.
    /// `last` is `None` when it never ran — a first run, not an overdue one.
    fn due(&self, now: Moment, mine: &TaskPrefs, last: Option<Moment>) -> Due;

    /// Whether it may run now, beyond the general gate — for what only it
    /// knows.
    fn needs(&self, _pulse: &Pulse, _mine: &TaskPrefs) -> Verdict {
        Verdict::Go
    }

    /// Whether the MACHINE wants it now, whatever the calendar says (a disk
    /// running short). PURE, like `due`.
    fn wanted_now(
        &self,
        _now: Moment,
        _pulse: &Pulse,
        _mine: &TaskPrefs,
        _last: Option<Moment>,
    ) -> bool {
        false
    }

    /// Do it. On the runner's thread, alone, and it may block.
    fn run(&self, run: &Run) -> Outcome;
}

/// A module that listens.
pub trait Watcher: Send + Sync {
    /// What it listens to. Read once, when the watch thread starts.
    fn sources(&self) -> Vec<Source>;

    /// The watch thread started, or the module was switched on: catch up on
    /// what was missed while it was off. A module switched off hears nothing
    /// until then.
    fn started(&self, _now: Moment, _ctx: &Ctx) {}

    /// Something it listens to happened. On the watch thread: NEVER BLOCKS.
    fn heard(&self, heard: Heard<'_>, ctx: &Ctx);

    /// When it next wants [`Heard::Wake`]. `None` arms no clock at all — the
    /// watch thread blocks until something is heard.
    ///
    /// Asked again after everything the thread hands out, and whenever the
    /// module says [`Ctx::changed`] from any thread — so a wake armed by an
    /// action is seen at once.
    fn next_wake(&self, _now: Moment) -> Option<Moment> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FICTIONAL IDS, on purpose: the core's fixtures never name a real module
    // (the sealed check refuses a module's name outside its folder). The
    // declarations the vocabulary's own tests read are the library's; what is
    // asked here is the contract's.
    static EMPTIED: EventKind = EventKind {
        name: "emptied",
        tag: "fixture-emptied",
    };

    struct Bare;

    impl Module for Bare {
        fn manifest(&self) -> &'static Manifest {
            static ONE: std::sync::OnceLock<Manifest> = std::sync::OnceLock::new();
            ONE.get_or_init(|| crate::fake::manifest("fixture"))
        }
    }

    #[test]
    fn a_module_that_answers_only_its_manifest_is_a_module() {
        // THE DEFAULTS ARE THE CONTRACT'S PROMISE: a module answers only the
        // questions it has, and every other one is a quiet no.
        let m: Box<dyn Module> = Box::new(Bare);
        assert!(m.scheduled().is_none());
        assert!(m.watcher().is_none());
        assert!(m.steps().is_empty());
        let pulse = Pulse::default();
        let mine = TaskPrefs::default();
        let look = Look {
            now: 1,
            pulse: &pulse,
            mine: &mine,
            last: None,
        };
        assert_eq!(m.face(&look), Face::default());
        assert_eq!(m.screen(&look), Screen::default());
        let (ctx, kept) = Ctx::kept("fixture", 1);
        assert_eq!(
            m.act(&Act::default(), &ctx),
            Acted::Refused("unknown"),
            "an action nobody declared was not refused"
        );
        assert!(kept.records().is_empty() && kept.changes().is_empty());
        assert!(m.work(&[], Path::new("/nonexistent")).is_err());
    }

    #[test]
    fn the_door_out_goes_where_the_sink_says_and_the_dirs_are_the_modules() {
        let (ctx, kept) = Ctx::kept("fixture", 42);
        ctx.record(Noted::of("fixture-noted").with("n", Arg::Number(2)));
        ctx.tell(Event::of(&EMPTIED).with("freed", Arg::Bytes(9)));
        ctx.changed();
        assert_eq!(kept.records().len(), 1);
        assert_eq!(kept.records()[0].1, 42, "the record was not stamped now");
        assert_eq!(kept.events()[0].0, "fixture");
        assert_eq!(kept.events()[0].1.tag, "fixture-emptied");
        assert_eq!(kept.changes(), vec!["fixture".to_owned()]);
        assert_eq!(ctx.state_dir(), None, "a test's context wrote somewhere");

        let rooted = Ctx::new(
            "fixture",
            0,
            Where {
                config: Some(PathBuf::from("/c/ronoctua")),
                state: Some(PathBuf::from("/s/ronoctua")),
            },
            true,
            Arc::new(Kept::default()),
        );
        assert_eq!(
            rooted.state_dir(),
            Some(PathBuf::from("/s/ronoctua/brain/fixture"))
        );
        assert_eq!(
            rooted.config_dir(),
            Some(PathBuf::from("/c/ronoctua/brain/fixture"))
        );
        assert!(rooted.rehearsing());
    }

    #[test]
    fn a_match_is_every_field_and_the_priority_and_the_text() {
        let wanted = Match {
            max_priority: Some(3),
            fields: vec![Field {
                name: "_SYSTEMD_UNIT".to_owned(),
                any_of: vec!["a.service".to_owned(), "b.service".to_owned()],
            }],
            message_contains: Some("Killed".to_owned()),
            backfill: Backfill::ThisBoot { most: 10 },
        };
        let mut entry = Entry {
            priority: Some(3),
            message: "Out of memory: Killed process 1".to_owned(),
            ..Entry::default()
        };
        entry
            .fields
            .insert("_SYSTEMD_UNIT".to_owned(), "b.service".to_owned());
        assert!(wanted.accepts(&entry));
        let quieter = Entry {
            priority: Some(6),
            ..entry.clone()
        };
        assert!(
            !wanted.accepts(&quieter),
            "an info line passed an err match"
        );
        let unsaid = Entry {
            priority: None,
            ..entry.clone()
        };
        assert!(
            !wanted.accepts(&unsaid),
            "a line with no priority read as an error"
        );
        let elsewhere = Entry {
            fields: std::collections::BTreeMap::new(),
            ..entry
        };
        assert!(!wanted.accepts(&elsewhere), "another unit's line passed");
    }

    /// THE `#[path]` PILOT (`brain.md` §3, "Step 1").
    ///
    /// `build.rs` writes the shim the generator will write for every module —
    /// an inline module that `include!`s the manifest and loads `module.rs` by
    /// an ABSOLUTE `#[path]` — and `module.rs` declares `mod rules;`. This
    /// compiling at all is half the proof (a `rules.rs` looked for anywhere
    /// else is a missing file); `file!()` is the other half.
    #[test]
    fn a_module_file_loaded_by_path_finds_its_siblings_beside_it() {
        use crate::pilot::pilot_sample;

        // BOXED AND OUTLIVING ITS BIRTH, which is what the registry does with
        // every module: the entry's `use<>` is what lets this compile.
        let m: Box<dyn Module> = Box::new(pilot_sample::new(&Birth::detached()));
        assert_eq!(m.manifest().id, "pilot-sample");
        assert_eq!(m.manifest().id, pilot_sample::ID);
        let beside = pilot_sample::rules_live_in();
        assert!(
            beside.ends_with("modules/pilot-sample/rules.rs"),
            "`mod rules;` resolved to {beside}, not beside module.rs"
        );

        // AND THE SAMPLE ANSWERS THROUGH THE CONTRACT, rules and all.
        let pulse = Pulse {
            disk_free_per_mille: Some(40),
            ..Pulse::default()
        };
        let mine = m.manifest().defaults();
        let look = Look {
            now: 100,
            pulse: &pulse,
            mine: &mine,
            last: None,
        };
        let face = m.face(&look);
        assert_eq!(face.marks.len(), 1, "a nearly full disk drew no mark");
        assert_eq!(face.marks[0].id, pilot_sample::mark::FULL);
        let calm = Pulse {
            disk_free_per_mille: Some(400),
            ..Pulse::default()
        };
        assert!(
            m.face(&Look {
                pulse: &calm,
                ..look
            })
            .marks
            .is_empty(),
            "a mark stayed up with the disk three-fifths free"
        );
        let Some(scheduled) = m.scheduled() else {
            panic!("the sample has a schedule");
        };
        assert_eq!(
            scheduled.due(100, &mine, None),
            Due::Not { why: "when-empty" }
        );
        assert_eq!(
            m.manifest().why_key("when-empty"),
            Some(pilot_sample::key::PILOT_SAMPLE_WHY_WHEN_EMPTY)
        );
    }
}
