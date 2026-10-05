//! The person's running containers, counted — a mark on the modeline while
//! any of them runs.
//!
//! Why the count comes from crun's folders and not from podman is in
//! `why.md`, beside this file. What is here is the shape of it.
//!
//! # A watch, and nothing that runs
//!
//! crun keeps one folder per running container under its state directory
//! (`ronoctua_containers::runtime_dir`, `$XDG_RUNTIME_DIR/crun`) and removes
//! it when the container is cleaned up. The watch thread watches that
//! directory — from its parent until it exists, from inside once it does —
//! and every change heard is one `readdir`: the count is exact, costs
//! microseconds, and nothing runs at rest.
//!
//! # The mark
//!
//! While at least one container runs, one mark in the colour of a watch,
//! with the count beside it; at rest there is none (invariant 14). A click
//! opens the containers' view — the mark names the shell's command
//! (`MarkId::opening`), which the shell runs by name, as `LEADER s o` does.
//!
//! # A folder that cannot be watched is said
//!
//! On the module's page, in the machine's words ([`Heard::Lost`]), rather
//! than a count of nothing that looks like no container running.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::module::{
    Birth, Cell, Ctx, Face, Heard, Look, Manifest, Mark, Module, Row, Screen, Section, Source,
    Tone, Watcher,
};
use crate::moment::Moment;

/// The shell's command a click on the mark runs: the containers' view.
const OPENS: &str = "containers-open";

/// How many of a container's id the page shows — podman's own short form.
const SHORT_ID: usize = 12;

/// The entry: a watch on crun's state directory, when the session has one.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    Containers::watching(ronoctua_containers::runtime_dir())
}

/// The watch, and what it counted.
struct Containers {
    /// crun's state directory — handed in, so a test watches a folder of its
    /// own. `None` in a session with no runtime directory.
    dir: Option<PathBuf>,
    /// What the last count found.
    kept: Mutex<Kept>,
    /// The page's generation: moves whenever the page would say something
    /// else.
    generation: AtomicU64,
}

/// What the last count found, and whether the folder can be watched.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Kept {
    /// The running containers' ids, sorted.
    running: Vec<String>,
    /// Why the folder cannot be watched, in the machine's words.
    lost: Option<String>,
}

impl Containers {
    fn watching(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            kept: Mutex::new(Kept::default()),
            generation: AtomicU64::new(0),
        }
    }

    /// What is kept now.
    fn kept(&self) -> Kept {
        self.kept.lock().map(|k| k.clone()).unwrap_or_default()
    }

    /// Changes what is kept; `true` when it changed, and then the page's
    /// generation has moved.
    fn keep(&self, change: impl FnOnce(&mut Kept)) -> bool {
        let Ok(mut kept) = self.kept.lock() else {
            return false;
        };
        let before = kept.clone();
        change(&mut kept);
        let changed = *kept != before;
        if changed {
            self.generation.fetch_add(1, Ordering::Relaxed);
        }
        changed
    }

    /// Counts again, and tells the brain when the count moved.
    fn recount(&self, ctx: &Ctx) {
        let running = self.dir.as_deref().map(running_in).unwrap_or_default();
        if self.keep(|k| k.running = running) {
            ctx.changed();
        }
    }
}

/// The running containers under crun's state directory: one folder each,
/// named by the container's id. A directory that is not there is none
/// running — crun makes it with the first container. Sorted, so two counts
/// of the same folders are equal.
fn running_in(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        // crun's own names never start with a dot; a folder that does is
        // somebody else's.
        .filter(|name| !name.starts_with('.'))
        .collect();
    out.sort();
    out
}

/// The count as the mark carries it.
fn count_of(kept: &Kept) -> u32 {
    u32::try_from(kept.running.len()).unwrap_or(u32::MAX)
}

/// A text key as a cell.
fn key(k: &'static str) -> Cell {
    Cell::Key(k.into())
}

/// The page's rows: how many run, and each one's short id — or why the
/// folder is not watched.
fn rows_of(kept: &Kept) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(why) = &kept.lost {
        rows.push(Row {
            id: "lost".to_owned(),
            label: key(super::key::CONTAINERS_ROW_UNWATCHED),
            value: Cell::Data(why.clone()),
            tone: Tone::Warning,
            ..Row::default()
        });
    }
    rows.push(Row {
        id: "count".to_owned(),
        label: key(super::key::CONTAINERS_ROW_RUNNING),
        value: Cell::Count(kept.running.len() as u64),
        tone: if kept.running.is_empty() {
            Tone::Quiet
        } else {
            Tone::Watching
        },
        ..Row::default()
    });
    for id in &kept.running {
        rows.push(Row {
            id: format!("running-{id}"),
            label: key(super::key::CONTAINERS_ROW_CONTAINER),
            value: Cell::Data(id.chars().take(SHORT_ID).collect()),
            ..Row::default()
        });
    }
    rows
}

impl Module for Containers {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }

    fn face(&self, _look: &Look<'_>) -> Face {
        let kept = self.kept();
        let mut face = Face {
            screen: self.generation.load(Ordering::Relaxed),
            ..Face::default()
        };
        // A MARK EXISTS ONLY WHEN IT SAYS SOMETHING (invariant 14).
        if !kept.running.is_empty() {
            face.marks.push(Mark {
                id: super::mark::RUNNING.opening(OPENS),
                tone: Tone::Watching,
                count: count_of(&kept),
                act: None,
            });
        }
        face
    }

    fn screen(&self, _look: &Look<'_>) -> Screen {
        Screen {
            sections: vec![Section {
                heading: super::key::CONTAINERS_HEADING_RUNNING.into(),
                rows: rows_of(&self.kept()),
            }],
        }
    }
}

impl Watcher for Containers {
    fn sources(&self) -> Vec<Source> {
        self.dir.clone().map(Source::Path).into_iter().collect()
    }

    /// THE FIRST COUNT, after the watch is raised: what it reads now plus
    /// what the watch hears after is everything.
    fn started(&self, _now: Moment, ctx: &Ctx) {
        self.recount(ctx);
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        match heard {
            // A FOLDER BORN OR GONE — or anything else under crun's
            // directory: one `readdir` says which.
            Heard::Path { .. } => self.recount(ctx),
            Heard::Lost { why, .. } => {
                if self.keep(|k| k.lost = Some(why.to_owned())) {
                    ctx.changed();
                }
            }
            Heard::Back { .. } => {
                if self.keep(|k| k.lost = None) {
                    ctx.changed();
                }
                self.recount(ctx);
            }
            Heard::Journal { .. } | Heard::Line { .. } | Heard::Wake => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::TaskPrefs;
    use crate::pulse::Pulse;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-brain-containers-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn face_of(m: &Containers) -> Face {
        let mine = TaskPrefs::default();
        let pulse = Pulse::default();
        m.face(&Look {
            now: 0,
            pulse: &pulse,
            mine: &mine,
            last: None,
        })
    }

    #[test]
    fn a_mark_while_one_runs_with_the_count_and_none_at_rest() {
        let root = scratch("mark");
        let crun = root.join("crun");
        let m = Containers::watching(Some(crun.clone()));
        let (ctx, kept) = Ctx::kept("containers", 0);
        // NO DIRECTORY YET: crun makes it with the first container.
        m.started(0, &ctx);
        assert!(face_of(&m).marks.is_empty(), "a mark with nothing running");
        std::fs::create_dir_all(crun.join("a1b2c3")).unwrap();
        std::fs::create_dir_all(crun.join("d4e5f6")).unwrap();
        // A FILE is not a container.
        std::fs::write(crun.join("lock"), "").unwrap();
        m.heard(
            Heard::Path {
                source: 0,
                path: &crun,
            },
            &ctx,
        );
        let face = face_of(&m);
        assert_eq!(face.marks.len(), 1);
        let mark = &face.marks[0];
        assert_eq!(mark.count, 2);
        assert_eq!(mark.tone, Tone::Watching);
        assert_eq!(
            mark.id.opens.as_deref(),
            Some(OPENS),
            "a click opens the view"
        );
        assert_eq!(kept.changes().len(), 1, "one change, told once");
        // THE SAME FOLDERS HEARD AGAIN change nothing, and tell nothing.
        m.heard(
            Heard::Path {
                source: 0,
                path: &crun,
            },
            &ctx,
        );
        assert_eq!(kept.changes().len(), 1);
        // BOTH GONE: the mark goes with them.
        std::fs::remove_dir(crun.join("a1b2c3")).unwrap();
        std::fs::remove_dir(crun.join("d4e5f6")).unwrap();
        m.heard(
            Heard::Path {
                source: 0,
                path: &crun,
            },
            &ctx,
        );
        assert!(face_of(&m).marks.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_folder_that_cannot_be_watched_is_said_on_the_page() {
        let m = Containers::watching(Some(PathBuf::from("/nonexistent/crun")));
        let (ctx, _) = Ctx::kept("containers", 0);
        let before = face_of(&m).screen;
        m.heard(
            Heard::Lost {
                source: 0,
                why: "no such directory",
            },
            &ctx,
        );
        assert_ne!(face_of(&m).screen, before, "the page did not move");
        let mine = TaskPrefs::default();
        let pulse = Pulse::default();
        let screen = m.screen(&Look {
            now: 0,
            pulse: &pulse,
            mine: &mine,
            last: None,
        });
        let rows = &screen.sections[0].rows;
        assert_eq!(rows[0].value, Cell::Data("no such directory".to_owned()));
        assert_eq!(rows[0].tone, Tone::Warning);
        m.heard(Heard::Back { source: 0 }, &ctx);
        assert_eq!(m.kept().lost, None);
    }

    #[test]
    fn no_runtime_directory_is_no_source_and_no_mark() {
        let m = Containers::watching(None);
        assert!(m.sources().is_empty());
        let (ctx, _) = Ctx::kept("containers", 0);
        m.started(0, &ctx);
        assert!(face_of(&m).marks.is_empty());
    }

    #[test]
    fn the_page_lists_what_runs_by_its_short_id() {
        let kept = Kept {
            running: vec!["79d267790a42e8aa455e39ce22a1b76a".to_owned()],
            lost: None,
        };
        let rows = rows_of(&kept);
        assert_eq!(rows[0].value, Cell::Count(1));
        assert_eq!(rows[1].value, Cell::Data("79d267790a42".to_owned()));
    }
}
