//! Starting fast-access's super index when it is not running, and marking the
//! modeline only while it is built from scratch.
//!
//! The whole story — why the brain starts it and not the session, why the
//! gate holds the first walk, why one mark — is in `why.md` beside this file.
//! What is here is the shape of it.
//!
//! # The brain owns the WHEN and nothing else
//!
//! The index is the unit's: `ronoctua-index serve`, which reconciles with the
//! disk and then follows the kernel's events for as long as it runs
//! (`nix/modules/index.nix`). What is left for the brain is whether it runs:
//! a run of this module is `systemctl --user start` of that unit and nothing
//! more, wanted when the unit is not running and the last start was not a
//! moment ago — never by a calendar.
//!
//! # How it knows
//!
//! The unit writes `status.json` beside the index when its phase moves
//! (`ronoctua_index::status`): walking from a start, following once the pass
//! landed, and after every commit. The watch hears the folder change (inotify,
//! never a poll) and reads it; a status whose process is no longer the unit
//! is no unit running. While the index is built from scratch the module asks
//! for a wake every few minutes — a unit killed mid-walk writes nothing more,
//! and the mark must not outlive it — and asks for none otherwise.
//!
//! # What the page changes
//!
//! `index.json`, the person's settings (`ronoctua_index::Settings`): the
//! folders walked, whether what files say is read, hidden files, names and
//! paths left out. Each action writes it merged over what it says now
//! (`Store::parsed`, never `read`: a file that does not parse is left as it
//! is), and the unit walks again by it.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ronoctua_index::Settings;

use crate::module::{
    Act, Acted, Birth, Cell, Ctx, Face, Heard, Look, Manifest, Mark, Module, Row, Run, Scheduled,
    Screen, Section, Source, Tone, Watcher,
};
use crate::moment::Moment;
use crate::prefs::TaskPrefs;
use crate::systemd;
use crate::task::{Due, Outcome};

/// How long after a start the module waits before it wants another, when the
/// unit still does not run: a unit that fails at once (a disk it cannot
/// write) is not started again every half minute.
const AGAIN_AFTER_S: i64 = 30 * 60;

/// How often a walk from scratch is looked at while it lasts: a unit killed
/// in the middle writes nothing more, and the mark must not outlive it.
const LOOK_WHILE_BUILDING_S: i64 = 5 * 60;

/// The entry: the real machine, the instance's index folder.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    SearchIndex::of(Box::new(Machine), ronoctua_index::default_dir())
}

/// What the module asks of the machine — a trait, so a test hands in a
/// machine of its own choosing.
trait Probe: Send + Sync {
    /// What the unit last said, and whether its process still runs.
    fn status(&self, dir: &Path) -> Option<(ronoctua_index::Status, bool)>;
    /// How much the index's folder weighs.
    fn weight(&self, dir: &Path) -> u64;
    /// The person's settings: `Ok(None)` when there is no file yet, `Err`
    /// when one is there and does not parse.
    fn settings(&self) -> Result<Option<Settings>, ()>;
    /// Writes the person's settings.
    fn keep(&self, settings: &Settings) -> Result<(), ()>;
    /// Whether this is a folder that exists.
    fn is_folder(&self, path: &Path) -> bool;
    /// The home folder, walked when no folder is chosen.
    fn home(&self) -> Option<PathBuf>;
}

/// The real machine.
struct Machine;

impl Probe for Machine {
    fn status(&self, dir: &Path) -> Option<(ronoctua_index::Status, bool)> {
        let status = ronoctua_index::status::read(dir)?;
        let alive = status.alive();
        Some((status, alive))
    }

    fn weight(&self, dir: &Path) -> u64 {
        ronoctua_index::status::weight_of(dir)
    }

    fn settings(&self) -> Result<Option<Settings>, ()> {
        let store = Settings::store();
        let Some(path) = store.path() else {
            return Ok(None);
        };
        if !path.exists() {
            return Ok(None);
        }
        store.parsed().map(Some).ok_or(())
    }

    fn keep(&self, settings: &Settings) -> Result<(), ()> {
        Settings::store().write(settings).map_err(|_| ())
    }

    fn is_folder(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn home(&self) -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// What the module remembers between two looks — in memory: a brain that
/// restarted reads the status again when its watch starts.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Kept {
    /// What the unit last said.
    status: Option<ronoctua_index::Status>,
    /// Whether its process runs.
    running: bool,
    /// Moves with every change of the page.
    generation: u64,
}

impl Kept {
    /// Whether the index is being built from scratch now.
    fn building(&self) -> bool {
        self.running
            && self
                .status
                .as_ref()
                .is_some_and(|s| s.scratch && s.phase == ronoctua_index::Phase::Walking)
    }
}

/// The module.
struct SearchIndex {
    probe: Box<dyn Probe>,
    /// The index's folder — `None` with no home, and then nothing is read.
    dir: Option<PathBuf>,
    kept: Mutex<Kept>,
}

impl SearchIndex {
    fn of(probe: Box<dyn Probe>, dir: Option<PathBuf>) -> Self {
        Self {
            probe,
            dir,
            kept: Mutex::new(Kept::default()),
        }
    }

    fn kept(&self) -> Kept {
        self.kept
            .lock()
            .map_or_else(|_| Kept::default(), |k| k.clone())
    }

    /// Reads the unit's status again — publishing when it moved.
    fn reread(&self, ctx: &Ctx) {
        let seen = self.dir.as_deref().and_then(|d| self.probe.status(d));
        let (status, running) = match seen {
            Some((s, alive)) => (Some(s), alive),
            None => (None, false),
        };
        let changed = self.kept.lock().is_ok_and(|mut k| {
            let moved = k.status != status || k.running != running;
            if moved {
                k.status = status;
                k.running = running;
                k.generation = k.generation.wrapping_add(1);
            }
            moved
        });
        if changed {
            ctx.changed();
        }
    }

    /// The page moved without the unit saying anything (a setting written).
    fn page_moved(&self, ctx: &Ctx) {
        if let Ok(mut k) = self.kept.lock() {
            k.generation = k.generation.wrapping_add(1);
        }
        ctx.changed();
    }

    /// Changes the person's settings with `change`, merged over what the file
    /// says now. A file that does not parse is left as it is.
    fn change(
        &self,
        ctx: &Ctx,
        change: impl FnOnce(&mut Settings) -> Result<(), &'static str>,
    ) -> Acted {
        let Ok(now) = self.probe.settings() else {
            return Acted::Refused(super::why::UNKEPT);
        };
        let mut settings = now.unwrap_or_default();
        if let Err(why) = change(&mut settings) {
            return Acted::Refused(why);
        }
        if self.probe.keep(&settings).is_err() {
            return Acted::Refused(super::why::UNKEPT);
        }
        self.page_moved(ctx);
        Acted::Done
    }
}

/// The row a numbered list's entry is on — `root:2`, `skip:0` — read back.
fn numbered(row: Option<&str>, prefix: &str) -> Option<usize> {
    row?.strip_prefix(prefix)?.strip_prefix(':')?.parse().ok()
}

/// A key's cell.
fn key(k: &'static str) -> Cell {
    Cell::Key(k.into())
}

/// The page's rows of where the index stands.
fn now_rows(kept: &Kept, weight: u64) -> Vec<Row> {
    use super::key as k;
    let (state, tone) = match (&kept.status, kept.running) {
        (Some(s), true) if s.phase == ronoctua_index::Phase::Walking => {
            (k::SEARCH_INDEX_STATE_WALKING, Tone::Watching)
        }
        (Some(_), true) => (k::SEARCH_INDEX_STATE_FOLLOWING, Tone::Quiet),
        _ => (k::SEARCH_INDEX_STATE_STOPPED, Tone::Warning),
    };
    let mut rows = vec![Row {
        id: "state".to_owned(),
        label: key(k::SEARCH_INDEX_STATE),
        value: key(state),
        tone,
        ..Row::default()
    }];
    let documents = kept.status.as_ref().map_or(0, |s| s.documents);
    rows.push(Row {
        id: "documents".to_owned(),
        label: key(k::SEARCH_INDEX_DOCUMENTS),
        value: Cell::Count(documents),
        ..Row::default()
    });
    rows.push(Row {
        id: "size".to_owned(),
        label: key(k::SEARCH_INDEX_SIZE),
        value: Cell::Bytes(weight),
        ..Row::default()
    });
    rows.push(Row {
        id: "committed".to_owned(),
        label: key(k::SEARCH_INDEX_COMMITTED),
        value: kept
            .status
            .as_ref()
            .and_then(|s| s.committed_s)
            .map_or_else(|| key(k::SEARCH_INDEX_NEVER), Cell::Moment),
        ..Row::default()
    });
    rows
}

/// The page's rows of what it reads, each with what pressing it changes.
fn reads_rows(settings: Result<Option<Settings>, ()>, home: Option<&Path>) -> Vec<Row> {
    use super::action as a;
    use super::key as k;
    let Ok(settings) = settings else {
        return vec![Row {
            id: "unreadable".to_owned(),
            label: key(k::SEARCH_INDEX_UNREADABLE),
            tone: Tone::Danger,
            ..Row::default()
        }];
    };
    let s = settings.unwrap_or_default();
    let mut rows = Vec::new();
    if s.roots.is_empty() {
        rows.push(Row {
            id: "home".to_owned(),
            label: key(k::SEARCH_INDEX_ROOT),
            value: home.map_or_else(
                || key(k::SEARCH_INDEX_HOME),
                |h| Cell::Data(h.display().to_string()),
            ),
            note: key(k::SEARCH_INDEX_HOME),
            ..Row::default()
        });
    }
    for (n, root) in s.roots.iter().enumerate() {
        rows.push(Row {
            id: format!("root:{n}"),
            label: key(k::SEARCH_INDEX_ROOT),
            value: Cell::Data(root.display().to_string()),
            act: Some(a::FORGET_ROOT.id.into()),
            ..Row::default()
        });
    }
    rows.push(Row {
        id: "add-root".to_owned(),
        label: key(k::SEARCH_INDEX_ADD_ROOT),
        act: Some(a::ADD_ROOT.id.into()),
        tone: Tone::Accent,
        ..Row::default()
    });
    let on = |yes: bool| {
        key(if yes {
            k::SEARCH_INDEX_ON
        } else {
            k::SEARCH_INDEX_OFF
        })
    };
    rows.push(Row {
        id: "contents".to_owned(),
        label: key(k::SEARCH_INDEX_CONTENTS),
        value: on(s.contents),
        act: Some(a::CONTENTS.id.into()),
        ..Row::default()
    });
    rows.push(Row {
        id: "hidden".to_owned(),
        label: key(k::SEARCH_INDEX_HIDDEN),
        value: on(s.hidden),
        act: Some(a::HIDDEN.id.into()),
        ..Row::default()
    });
    for (n, skip) in s.skip.iter().enumerate() {
        rows.push(Row {
            id: format!("skip:{n}"),
            label: key(k::SEARCH_INDEX_SKIP),
            value: Cell::Data(skip.clone()),
            act: Some(a::FORGET_SKIP.id.into()),
            ..Row::default()
        });
    }
    rows.push(Row {
        id: "add-skip".to_owned(),
        label: key(k::SEARCH_INDEX_ADD_SKIP),
        act: Some(a::ADD_SKIP.id.into()),
        tone: Tone::Accent,
        ..Row::default()
    });
    rows
}

impl Module for SearchIndex {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }

    fn face(&self, _look: &Look<'_>) -> Face {
        let k = self.kept();
        let mut face = Face {
            screen: k.generation,
            ..Face::default()
        };
        // A MARK ONLY WHILE IT SAYS SOMETHING (invariant 14): a walk from
        // scratch going on. Following changes afterwards is no mark.
        if k.building() {
            face.marks.push(Mark {
                id: super::mark::BUILDING,
                tone: Tone::Watching,
                count: 0,
                act: None,
            });
        }
        face
    }

    /// Built off the footer's loop (`Ask::Screen`): the folder's weight is a
    /// directory read.
    fn screen(&self, _look: &Look<'_>) -> Screen {
        let k = self.kept();
        let weight = self.dir.as_deref().map_or(0, |d| self.probe.weight(d));
        Screen {
            sections: vec![
                Section {
                    heading: super::key::SEARCH_INDEX_HEADING_NOW.into(),
                    rows: now_rows(&k, weight),
                },
                Section {
                    heading: super::key::SEARCH_INDEX_HEADING_READS.into(),
                    rows: reads_rows(self.probe.settings(), self.probe.home().as_deref()),
                },
            ],
        }
    }

    fn act(&self, act: &Act, ctx: &Ctx) -> Acted {
        use super::action as a;
        use super::why as w;
        let row = act.row.as_deref();
        match act.action.as_str() {
            id if id == a::ADD_ROOT.id => {
                let typed = act.answer.as_deref().map(str::trim).unwrap_or_default();
                let path = PathBuf::from(typed);
                if !path.is_absolute() || !self.probe.is_folder(&path) {
                    return Acted::Refused(w::NOT_A_FOLDER);
                }
                self.change(ctx, |s| {
                    if !s.roots.contains(&path) {
                        s.roots.push(path);
                    }
                    Ok(())
                })
            }
            id if id == a::FORGET_ROOT.id => {
                let Some(n) = numbered(row, "root") else {
                    return Acted::Refused("unknown");
                };
                self.change(ctx, |s| {
                    if n < s.roots.len() {
                        s.roots.remove(n);
                    }
                    Ok(())
                })
            }
            id if id == a::CONTENTS.id => self.change(ctx, |s| {
                s.contents = !s.contents;
                Ok(())
            }),
            id if id == a::HIDDEN.id => self.change(ctx, |s| {
                s.hidden = !s.hidden;
                Ok(())
            }),
            id if id == a::ADD_SKIP.id => {
                let typed = act
                    .answer
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_owned();
                if ronoctua_index::settings::skip_entry(&typed).is_err() {
                    return Acted::Refused(w::NOT_A_SKIP);
                }
                self.change(ctx, |s| {
                    if !s.skip.contains(&typed) {
                        s.skip.push(typed);
                    }
                    Ok(())
                })
            }
            id if id == a::FORGET_SKIP.id => {
                let Some(n) = numbered(row, "skip") else {
                    return Acted::Refused("unknown");
                };
                self.change(ctx, |s| {
                    if n < s.skip.len() {
                        s.skip.remove(n);
                    }
                    Ok(())
                })
            }
            _ => Acted::Refused("unknown"),
        }
    }
}

impl Scheduled for SearchIndex {
    fn due(&self, _now: Moment, _mine: &TaskPrefs, _last: Option<Moment>) -> Due {
        if !systemd::reachable() {
            return Due::Not {
                why: "no-systemctl",
            };
        }
        // NEVER BY THE CALENDAR: the row says "when it is not running".
        Due::Not {
            why: super::why::WHEN_STOPPED,
        }
    }

    fn wanted_now(
        &self,
        now: Moment,
        _pulse: &crate::pulse::Pulse,
        _mine: &TaskPrefs,
        last: Option<Moment>,
    ) -> bool {
        systemd::reachable() && wanted(&self.kept(), now, last)
    }

    fn run(&self, run: &Run) -> Outcome {
        // A BRAIN THAT ONLY REHEARSES starts nothing, and says so.
        if run.is_rehearsal() {
            return Outcome::Skipped {
                why: "rehearsing".to_owned(),
            };
        }
        match systemd::start(super::unit::RUN) {
            Ok(()) => Outcome::Done(Vec::new()),
            Err(e) => Outcome::Failed {
                at: String::new(),
                why: e.to_string(),
            },
        }
    }
}

/// Whether a start is wanted: the unit does not run, and the last start was
/// not a moment ago. Pure.
fn wanted(kept: &Kept, now: Moment, last: Option<Moment>) -> bool {
    !kept.running && last.is_none_or(|l| now.saturating_sub(l) >= AGAIN_AFTER_S)
}

impl Watcher for SearchIndex {
    /// The index's folder: the unit writes its status there.
    fn sources(&self) -> Vec<Source> {
        self.dir.iter().map(|d| Source::Path(d.clone())).collect()
    }

    /// Catching up: whatever the unit last said, and whether it still runs.
    fn started(&self, _now: Moment, ctx: &Ctx) {
        self.reread(ctx);
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        match heard {
            Heard::Path { path, .. } if is_status(path) => self.reread(ctx),
            Heard::Wake => self.reread(ctx),
            _ => {}
        }
    }

    /// A clock only while the index is built from scratch.
    fn next_wake(&self, now: Moment) -> Option<Moment> {
        self.kept()
            .building()
            .then_some(now.saturating_add(LOOK_WHILE_BUILDING_S))
    }
}

/// Whether a change heard in the folder is the status — or the folder
/// itself, when the watch could not say which file.
fn is_status(path: &Path) -> bool {
    path.file_name()
        .is_none_or(|n| n == ronoctua_index::status::FILE)
        || path.extension().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A machine the test sets.
    struct Set {
        status: Mutex<Option<(ronoctua_index::Status, bool)>>,
        settings: Mutex<Result<Option<Settings>, ()>>,
        kept: Mutex<Vec<Settings>>,
        refuse_keeping: std::sync::atomic::AtomicBool,
    }

    impl Probe for Arc<Set> {
        fn status(&self, _: &Path) -> Option<(ronoctua_index::Status, bool)> {
            self.status.lock().unwrap().clone()
        }

        fn weight(&self, _: &Path) -> u64 {
            4096
        }

        fn settings(&self) -> Result<Option<Settings>, ()> {
            self.settings.lock().unwrap().clone()
        }

        fn keep(&self, settings: &Settings) -> Result<(), ()> {
            if self
                .refuse_keeping
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                return Err(());
            }
            *self.settings.lock().unwrap() = Ok(Some(settings.clone()));
            self.kept.lock().unwrap().push(settings.clone());
            Ok(())
        }

        fn is_folder(&self, path: &Path) -> bool {
            path.starts_with("/srv")
        }

        fn home(&self) -> Option<PathBuf> {
            Some(PathBuf::from("/home/someone"))
        }
    }

    fn set() -> Arc<Set> {
        Arc::new(Set {
            status: Mutex::new(None),
            settings: Mutex::new(Ok(None)),
            kept: Mutex::new(Vec::new()),
            refuse_keeping: std::sync::atomic::AtomicBool::new(false),
        })
    }

    fn module(set: &Arc<Set>) -> SearchIndex {
        SearchIndex::of(
            Box::new(Arc::clone(set)),
            Some(PathBuf::from("/nonexistent/index")),
        )
    }

    fn status(phase: ronoctua_index::Phase, scratch: bool) -> ronoctua_index::Status {
        ronoctua_index::Status {
            pid: 1,
            phase,
            scratch,
            since_s: 10,
            documents: 42,
            committed_s: Some(20),
        }
    }

    fn face_of(m: &SearchIndex) -> Face {
        let mine = super::super::MANIFEST.defaults();
        let pulse = crate::pulse::Pulse::default();
        m.face(&Look {
            now: 100,
            pulse: &pulse,
            mine: &mine,
            last: None,
        })
    }

    fn act(action: &str, row: Option<&str>, answer: Option<&str>) -> Act {
        Act {
            action: action.to_owned(),
            row: row.map(ToOwned::to_owned),
            answer: answer.map(ToOwned::to_owned),
        }
    }

    /// FAILS IF: a mark shows at rest, while the unit only follows changes,
    /// or for a walk from scratch whose process is gone — or none shows for
    /// one going on.
    #[test]
    fn a_mark_shows_only_while_the_index_is_built_from_scratch() {
        let s = set();
        let m = module(&s);
        let (ctx, kept) = Ctx::kept(super::super::ID, 100);
        m.started(100, &ctx);
        assert!(face_of(&m).marks.is_empty(), "a mark at rest");
        for (phase, scratch, alive, marked) in [
            (ronoctua_index::Phase::Walking, true, true, true),
            (ronoctua_index::Phase::Walking, false, true, false),
            (ronoctua_index::Phase::Following, true, true, false),
            (ronoctua_index::Phase::Walking, true, false, false),
        ] {
            *s.status.lock().unwrap() = Some((status(phase, scratch), alive));
            m.heard(
                Heard::Path {
                    source: 0,
                    path: Path::new("/nonexistent/index/status.json"),
                },
                &ctx,
            );
            let face = face_of(&m);
            assert_eq!(
                !face.marks.is_empty(),
                marked,
                "{phase:?} {scratch} {alive}"
            );
            if marked {
                assert_eq!(face.marks[0].id, super::super::mark::BUILDING);
                assert_eq!(face.marks[0].tone, Tone::Watching);
                assert_eq!(m.next_wake(100), Some(100 + LOOK_WHILE_BUILDING_S));
            } else {
                assert_eq!(m.next_wake(100), None, "no clock while nothing builds");
            }
        }
        assert!(!kept.changes().is_empty(), "the moves were published");
        assert_eq!(
            super::super::mark::BUILDING.glyph,
            "search-index-building",
            "the glyph token is the module's"
        );
    }

    /// FAILS IF: a start is wanted while the unit runs, never wanted when it
    /// does not, or wanted again a moment after the last start.
    #[test]
    fn a_start_is_wanted_only_when_the_unit_is_not_running() {
        let stopped = Kept::default();
        assert!(wanted(&stopped, 1_000, None), "never started: start it");
        assert!(!wanted(&stopped, 1_000, Some(990)), "a moment ago");
        assert!(wanted(&stopped, 1_000 + AGAIN_AFTER_S, Some(1_000)));
        let running = Kept {
            running: true,
            ..Kept::default()
        };
        assert!(!wanted(&running, 1_000, None));
        assert!(
            super::super::MANIFEST.gated(),
            "a first walk waits for a quiet machine"
        );
        assert!(!super::super::MANIFEST.needs.network);
    }

    /// FAILS IF: the page's actions stop writing `index.json` merged, accept
    /// a root that is not a folder or a skip that skips nothing, or write
    /// over a file that does not parse.
    #[test]
    fn the_page_changes_what_it_reads_and_refuses_what_means_nothing() {
        use super::super::action as a;
        let s = set();
        let m = module(&s);
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        assert_eq!(
            m.act(&act(a::ADD_ROOT.id, None, Some("docs")), &ctx),
            Acted::Refused(super::super::why::NOT_A_FOLDER)
        );
        assert_eq!(
            m.act(&act(a::ADD_ROOT.id, None, Some("/srv/a")), &ctx),
            Acted::Done
        );
        assert_eq!(
            m.act(&act(a::ADD_ROOT.id, None, Some("/srv/b")), &ctx),
            Acted::Done
        );
        assert_eq!(
            m.act(&act(a::FORGET_ROOT.id, Some("root:0"), None), &ctx),
            Acted::Done
        );
        assert_eq!(
            m.act(&act(a::CONTENTS.id, Some("contents"), None), &ctx),
            Acted::Done
        );
        assert_eq!(
            m.act(&act(a::ADD_SKIP.id, None, Some("Videos/raw")), &ctx),
            Acted::Refused(super::super::why::NOT_A_SKIP)
        );
        assert_eq!(
            m.act(&act(a::ADD_SKIP.id, None, Some("build")), &ctx),
            Acted::Done
        );
        let now = s.settings.lock().unwrap().clone().unwrap().unwrap();
        assert_eq!(now.roots, [PathBuf::from("/srv/b")]);
        assert!(!now.contents);
        assert_eq!(now.skip, ["build"]);
        assert_eq!(
            m.act(&act(a::FORGET_SKIP.id, Some("skip:0"), None), &ctx),
            Acted::Done
        );
        assert!(
            s.settings
                .lock()
                .unwrap()
                .clone()
                .unwrap()
                .unwrap()
                .skip
                .is_empty()
        );
        // A FILE THAT DOES NOT PARSE is left as it is.
        *s.settings.lock().unwrap() = Err(());
        let before = s.kept.lock().unwrap().len();
        assert_eq!(
            m.act(&act(a::HIDDEN.id, Some("hidden"), None), &ctx),
            Acted::Refused(super::super::why::UNKEPT)
        );
        assert_eq!(s.kept.lock().unwrap().len(), before, "nothing written");
        assert_eq!(
            m.act(&act("nothing", None, None), &ctx),
            Acted::Refused("unknown")
        );
    }

    /// FAILS IF: the page stops saying where the index stands and what it
    /// reads, offers no way to add a folder, or draws defaults over a file
    /// that does not parse.
    #[test]
    fn the_page_says_where_the_index_stands_and_what_it_reads() {
        let s = set();
        let m = module(&s);
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        *s.status.lock().unwrap() = Some((status(ronoctua_index::Phase::Following, false), true));
        m.started(100, &ctx);
        let mine = super::super::MANIFEST.defaults();
        let pulse = crate::pulse::Pulse::default();
        let look = Look {
            now: 100,
            pulse: &pulse,
            mine: &mine,
            last: None,
        };
        let screen = m.screen(&look);
        let ids: Vec<Vec<String>> = screen
            .sections
            .iter()
            .map(|s| s.rows.iter().map(|r| r.id.clone()).collect())
            .collect();
        assert_eq!(ids[0], ["state", "documents", "size", "committed"]);
        assert_eq!(
            ids[1],
            ["home", "add-root", "contents", "hidden", "add-skip"]
        );
        assert_eq!(screen.sections[0].rows[1].value, Cell::Count(42));
        assert_eq!(screen.sections[0].rows[2].value, Cell::Bytes(4096));
        *s.settings.lock().unwrap() = Err(());
        let ids: Vec<String> = m.screen(&look).sections[1]
            .rows
            .iter()
            .map(|r| r.id.clone())
            .collect();
        assert_eq!(ids, ["unreadable"]);
        assert_eq!(
            m.sources(),
            vec![Source::Path(PathBuf::from("/nonexistent/index"))]
        );
        assert_eq!(numbered(Some("root:3"), "root"), Some(3));
        assert_eq!(numbered(Some("skip:x"), "skip"), None);
    }
}
