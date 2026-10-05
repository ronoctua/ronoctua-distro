//! The belongings, reminded: a mark on the modeline when a sync is due, while
//! a sync waits for decisions, after one that failed, and before the first
//! sync of a machine that has a repository set.
//!
//! The whole story — why a reminder and not a sync, why one clock and only
//! while a repository is set — is in `why.md` beside this file. What is here
//! is the shape of it.
//!
//! # It runs nothing
//!
//! A sync asks the person's passphrase, and nothing the machine does by itself
//! may ask anybody anything. So this module is a WATCH: it reads the files the
//! belongings leave (`belongings.json`, the state folder's `status.json` and
//! `last.json`, the runtime folder's session) when they move, and arms ONE
//! clock, for `last + period`, while there is a repository and a period and
//! the sync is not yet due ([`Watcher::next_wake`]).
//!
//! # The marks
//!
//! One at a time, the most pressing first: decisions waiting (the warning's
//! colour, with how many), a sync that failed (the warning's), a first sync
//! (the accent: restore this machine), and a sync due (watching). A click on
//! the first opens the belongings, where the decisions are; on the others it
//! opens them asking the passphrase — a COMMAND of the footer's, which the
//! mark's id names ([`crate::module::MarkId::opens`]), not an action of this
//! module.

use std::path::PathBuf;
use std::sync::Mutex;

use ronoctua_belongings::places::{OWN, RUNTIME_LEAF, SESSION_DIR};
use ronoctua_belongings::plan::PLAN_FILE;
use ronoctua_belongings::prefs::{Belongings, FILE};
use ronoctua_belongings::status::{Last, Outcome, Status, due, due_at};

use crate::module::{
    Birth, Cell, Ctx, Face, Heard, Look, Manifest, Mark, Module, Row, Screen, Section, Source,
    Tone, Watcher,
};
use crate::moment::Moment;

/// The footer's command that opens the belongings asking the passphrase.
const SYNC: &str = "belongings-sync";
/// The footer's command that opens the belongings — where decisions are made.
const OPEN: &str = "belongings-open";

/// The entry: a reminder reading the files under this birth's roots, and the
/// runtime folder of this instance.
#[must_use]
pub fn new(birth: &Birth) -> impl Module + use<> {
    Reminder::reading(Paths {
        prefs: birth.roots.config.as_ref().map(|c| c.join(FILE)),
        own: birth.roots.state.as_ref().map(|s| s.join(OWN)),
        runtime: birth
            .roots
            .config
            .as_ref()
            .and_then(|_| ronoctua_xdg::runtime_file(RUNTIME_LEAF)),
    })
}

/// Where the files are — handed in, so a test reads a folder of its own.
#[derive(Debug, Clone, Default)]
struct Paths {
    /// `belongings.json`.
    prefs: Option<PathBuf>,
    /// The state folder: `status.json`, `last.json`.
    own: Option<PathBuf>,
    /// The runtime folder: the session a sync prepared.
    runtime: Option<PathBuf>,
}

/// What the files said when they were last read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Seen {
    /// Whether a repository is set.
    repository: bool,
    /// The period chosen, in days; `None` is never.
    period_days: Option<u16>,
    /// The last sync with the repository set now, seconds since the epoch.
    last_s: Option<i64>,
    /// A sync prepared and waiting for this many decisions.
    waiting: Option<u32>,
    /// The last run failed.
    failed: bool,
}

impl Seen {
    /// Reads the files. Missing files are a machine that never synced.
    fn read(paths: &Paths) -> Self {
        let prefs: Belongings = paths
            .prefs
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let last = paths
            .own
            .as_ref()
            .and_then(|o| Last::read(&o.join(ronoctua_belongings::places::LAST_FILE)))
            .filter(|l| l.repository == prefs.repository);
        let status = paths
            .own
            .as_ref()
            .and_then(|o| Status::read(&o.join(ronoctua_belongings::places::STATUS_FILE)));
        let prepared = paths
            .runtime
            .as_ref()
            .is_some_and(|r| r.join(SESSION_DIR).join(PLAN_FILE).is_file());
        let waiting = match status.as_ref().map(|s| &s.outcome) {
            // A SESSION THAT IS GONE waits for nothing: the runtime folder is
            // emptied by a reboot, and the plan with it.
            Some(Outcome::Waiting { conflicts }) if prepared => Some(*conflicts),
            _ => None,
        };
        Self {
            repository: prefs.has_repository(),
            period_days: prefs.period_days,
            last_s: last.map(|l| l.at_s),
            waiting,
            failed: status.as_ref().is_some_and(Status::failed),
        }
    }

    /// The one mark it draws now, if any.
    fn mark(&self, now: Moment) -> Option<Mark> {
        if !self.repository {
            return None;
        }
        let (id, tone, count, opens) = if let Some(n) = self.waiting {
            (super::mark::CONFLICTS, Tone::Warning, n, OPEN)
        } else if self.failed {
            (super::mark::FAILED, Tone::Warning, 0, SYNC)
        } else if self.last_s.is_none() {
            (super::mark::RESTORE, Tone::Accent, 0, SYNC)
        } else if due(now, self.last_s, self.period_days) {
            (super::mark::DUE, Tone::Watching, 0, SYNC)
        } else {
            return None;
        };
        Some(Mark {
            id: id.opening(opens),
            tone,
            count,
            act: None,
        })
    }

    /// When the reminder is next due, while it is not yet.
    fn next_due(&self, now: Moment) -> Option<Moment> {
        if !self.repository {
            return None;
        }
        due_at(self.last_s, self.period_days).filter(|at| *at > now)
    }
}

/// The reminder.
struct Reminder {
    paths: Paths,
    /// What was read, and the page's generation — moved whenever it changes.
    kept: Mutex<(Seen, u64)>,
}

impl Reminder {
    fn reading(paths: Paths) -> Self {
        let seen = Seen::read(&paths);
        Self {
            paths,
            kept: Mutex::new((seen, 1)),
        }
    }

    /// What it saw last.
    fn seen(&self) -> (Seen, u64) {
        self.kept
            .lock()
            .map_or_else(|_| (Seen::default(), 0), |k| k.clone())
    }

    /// Reads the files again; says whether anything changed.
    fn reread(&self) -> bool {
        let fresh = Seen::read(&self.paths);
        let Ok(mut kept) = self.kept.lock() else {
            return false;
        };
        if kept.0 == fresh {
            return false;
        }
        kept.0 = fresh;
        kept.1 += 1;
        true
    }
}

/// A text key as a cell.
fn key(k: &'static str) -> Cell {
    Cell::Key(k.into())
}

impl Module for Reminder {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }

    fn face(&self, look: &Look<'_>) -> Face {
        let (seen, generation) = self.seen();
        Face {
            marks: seen.mark(look.now).into_iter().collect(),
            screen: generation,
            ..Face::default()
        }
    }

    fn screen(&self, look: &Look<'_>) -> Screen {
        let (seen, _) = self.seen();
        let rows = if seen.repository {
            let mut rows = vec![
                Row {
                    id: "repository".to_owned(),
                    label: key(super::key::BELONGINGS_REMINDER_ROW_REPOSITORY),
                    value: key(super::key::BELONGINGS_REMINDER_ROW_SET),
                    ..Row::default()
                },
                Row {
                    id: "last".to_owned(),
                    label: key(super::key::BELONGINGS_REMINDER_ROW_LAST),
                    value: seen.last_s.map_or_else(
                        || key(super::key::BELONGINGS_REMINDER_ROW_NEVER),
                        Cell::Moment,
                    ),
                    ..Row::default()
                },
            ];
            if let Some(at) = seen.next_due(look.now) {
                rows.push(Row {
                    id: "next".to_owned(),
                    label: key(super::key::BELONGINGS_REMINDER_ROW_NEXT),
                    value: Cell::Moment(at),
                    ..Row::default()
                });
            }
            if let Some(n) = seen.waiting {
                rows.push(Row {
                    id: "waiting".to_owned(),
                    label: key(super::key::BELONGINGS_REMINDER_ROW_WAITING),
                    value: Cell::Count(u64::from(n)),
                    tone: Tone::Warning,
                    ..Row::default()
                });
            }
            if seen.failed {
                rows.push(Row {
                    id: "failed".to_owned(),
                    label: key(super::key::BELONGINGS_REMINDER_ROW_FAILED),
                    tone: Tone::Warning,
                    ..Row::default()
                });
            }
            rows
        } else {
            vec![Row {
                id: "unset".to_owned(),
                label: key(super::key::BELONGINGS_REMINDER_ROW_UNSET),
                ..Row::default()
            }]
        };
        Screen {
            sections: vec![Section {
                heading: super::key::BELONGINGS_REMINDER_HEADING.into(),
                rows,
            }],
        }
    }
}

impl Watcher for Reminder {
    fn sources(&self) -> Vec<Source> {
        [&self.paths.prefs, &self.paths.own, &self.paths.runtime]
            .into_iter()
            .flatten()
            .cloned()
            .map(Source::Path)
            .collect()
    }

    /// What moved while the brain was off — a sync from another door, a
    /// repository set by the installer — read at once.
    fn started(&self, _now: Moment, ctx: &Ctx) {
        self.reread();
        ctx.changed();
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        match heard {
            // THE CLOCK: the reminder came due. Nothing on disk moved, the
            // face did.
            Heard::Wake => ctx.changed(),
            Heard::Path { .. } | Heard::Back { .. } if self.reread() => ctx.changed(),
            _ => {}
        }
    }

    /// ONE CLOCK, for `last + period`, and only while a repository is set, a
    /// period is chosen and the sync is not yet due.
    fn next_wake(&self, now: Moment) -> Option<Moment> {
        self.seen().0.next_due(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_belongings::status::Known;

    const DAY: i64 = 86_400;

    /// A home of its own: the three folders, and the files a test writes.
    fn scratch(name: &str) -> (PathBuf, Paths) {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-brain-belongings-reminder-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["config", "state/belongings", "run/session"] {
            std::fs::create_dir_all(root.join(d)).expect("scratch");
        }
        let paths = Paths {
            prefs: Some(root.join("config").join(FILE)),
            own: Some(root.join("state/belongings")),
            runtime: Some(root.join("run")),
        };
        (root, paths)
    }

    fn write(path: &std::path::Path, value: &impl serde::Serialize) {
        std::fs::write(path, serde_json::to_vec(value).expect("json")).expect("write");
    }

    fn prefs(paths: &Paths, repository: &str, period: Option<u16>) {
        write(
            paths.prefs.as_ref().expect("prefs"),
            &Belongings {
                period_days: period,
                ..Belongings::with_repository(repository)
            },
        );
    }

    fn last(paths: &Paths, repository: &str, at_s: i64) {
        write(
            &paths.own.as_ref().expect("own").join("last.json"),
            &Last {
                repository: repository.to_owned(),
                at_s,
                entries: [("config/theme.json".to_owned(), Known::default())]
                    .into_iter()
                    .collect(),
                ..Last::default()
            },
        );
    }

    fn status(paths: &Paths, outcome: Outcome) {
        write(
            &paths.own.as_ref().expect("own").join("status.json"),
            &Status { at_s: 1, outcome },
        );
    }

    const REPO: &str = "git@example.org:someone/belongings.git";

    /// FAILS IF: a mark shows with no repository (invariant 14), a first sync
    /// is not marked, the sync is marked before its period has passed or not
    /// after, or the clock is armed when there is nothing to wait for.
    #[test]
    fn the_mark_comes_due_with_the_period_and_the_clock_waits_for_it() {
        let (root, paths) = scratch("due");
        let r = Reminder::reading(paths.clone());
        assert_eq!(r.seen().0.mark(0), None, "a mark with no repository");
        assert_eq!(r.next_wake(0), None, "a clock with no repository");

        prefs(&paths, REPO, Some(7));
        assert!(r.reread());
        let first = r.seen().0.mark(0).expect("the first sync is not marked");
        assert_eq!(first.id, super::super::mark::RESTORE.opening(SYNC));
        assert_eq!(first.tone, Tone::Accent);
        assert_eq!(first.id.opens.as_deref(), Some("belongings-sync"));
        assert_eq!(
            first.act, None,
            "a command is the id's to name, not an act's"
        );
        assert_eq!(r.next_wake(0), None, "a clock with nothing to count from");

        last(&paths, REPO, 10 * DAY);
        assert!(r.reread());
        assert_eq!(r.seen().0.mark(16 * DAY), None, "due before its period");
        assert_eq!(r.next_wake(16 * DAY), Some(17 * DAY));
        let due = r.seen().0.mark(17 * DAY).expect("not due after its period");
        assert_eq!(due.id, super::super::mark::DUE.opening(SYNC));
        assert_eq!(due.tone, Tone::Watching);
        assert_eq!(
            r.next_wake(17 * DAY),
            None,
            "a clock for what is already due"
        );

        // NEVER: no reminder, and no clock.
        prefs(&paths, REPO, None);
        assert!(r.reread());
        assert_eq!(r.seen().0.mark(900 * DAY), None);
        assert_eq!(r.next_wake(0), None);

        // ANOTHER REPOSITORY is a first sync again.
        prefs(&paths, "git@example.org:someone/other.git", Some(7));
        assert!(r.reread());
        assert_eq!(
            r.seen().0.mark(11 * DAY).map(|m| m.id),
            Some(super::super::mark::RESTORE.opening(SYNC))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: decisions waiting or a failed sync are not marked in the
    /// warning's colour, waiting outranks nothing, the count is not the
    /// conflicts', or a session a reboot emptied still counts.
    #[test]
    fn waiting_and_failing_are_warnings_and_waiting_opens_the_decisions() {
        let (root, paths) = scratch("waiting");
        prefs(&paths, REPO, Some(7));
        last(&paths, REPO, 0);
        status(&paths, Outcome::Waiting { conflicts: 3 });
        let r = Reminder::reading(paths.clone());
        assert_eq!(r.seen().0.waiting, None, "a session that is gone waits");
        std::fs::write(root.join("run/session/plan.json"), "{}").expect("plan");
        assert!(r.reread());
        let mark = r.seen().0.mark(100 * DAY).expect("no mark while waiting");
        assert_eq!(mark.id, super::super::mark::CONFLICTS.opening(OPEN));
        assert_eq!((mark.tone, mark.count), (Tone::Warning, 3));
        assert_eq!(mark.id.opens.as_deref(), Some("belongings-open"));

        status(
            &paths,
            Outcome::Refused {
                why: ronoctua_belongings::Why::Unreachable,
            },
        );
        assert!(r.reread());
        let mark = r.seen().0.mark(0).expect("no mark after a failure");
        assert_eq!(mark.id, super::super::mark::FAILED.opening(SYNC));
        assert_eq!(mark.tone, Tone::Warning);
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a file moving is not heard as a change of face, the clock's
    /// wake is not, or nothing moving is taken for a change.
    #[test]
    fn a_file_that_moves_and_the_clock_change_the_face() {
        let (root, paths) = scratch("heard");
        let r = Reminder::reading(paths.clone());
        assert_eq!(r.sources().len(), 3);
        let (ctx, kept) = Ctx::kept("fixture", 0);
        let path = paths.prefs.clone().expect("prefs");
        r.heard(
            Heard::Path {
                source: 0,
                path: &path,
            },
            &ctx,
        );
        assert!(
            kept.changes().is_empty(),
            "nothing moved and the face changed"
        );
        prefs(&paths, REPO, Some(1));
        r.heard(
            Heard::Path {
                source: 0,
                path: &path,
            },
            &ctx,
        );
        assert_eq!(kept.changes().len(), 1);
        r.heard(Heard::Wake, &ctx);
        assert_eq!(kept.changes().len(), 2);
        let (_, generation) = r.seen();
        assert!(generation > 1, "the page's generation did not move");
        let _ = std::fs::remove_dir_all(root);
    }
}
