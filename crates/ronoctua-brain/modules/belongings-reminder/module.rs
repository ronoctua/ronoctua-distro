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
//!
//! # The plain part, sent by itself
//!
//! The person's declarations and brain modules travel IN CLEAR, with no
//! passphrase (the plain part): nothing to ask, so the machine may send them
//! by itself. This module watches the plain roots as TREES (inotify, never a
//! poll; `Source::Tree`: every folder under them, the ones made after the
//! brain started included — `software/` is made by the first declaration)
//! and, once they have stayed QUIET for [`QUIET_S`], starts the unit that
//! syncs the plain part alone (`sync-plain`, declared beside this file).
//! Only once this machine synced with the repository it has set (the plain
//! part of a repository nobody made yet has nowhere to go), never in a
//! rehearsal (the development loop's copy), and a change while it waits
//! waits again.
//!
//! # A send is kept until a run read it
//!
//! Starting the unit is not sending: a run of the belongings that holds
//! their lock (a sync the person started) refuses it (`Busy`), and a
//! `sync-plain` already running is only joined. So the move is kept, and
//! settled by what the state folder says when a run ends ([`Plain`]): a run
//! that took the lock after the send was asked read the plain part after the
//! move, and the move is done (sent, or failed: the next move or sync sends
//! it, and a send that fails never loops); no run took it since — the send
//! was refused, or left to the run that held the lock — and it is asked
//! again at once, a few times at most ([`AGAIN_MOST`]). A move heard while a
//! run reads is a move like any other: sent once quiet, after that run.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use ronoctua_belongings::places::{LOCK_FILE, OWN, RUNTIME_LEAF, Root, SESSION_DIR};
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

/// How long the plain part stays quiet before it is sent, in seconds: a
/// software centre that writes a list and then its icons is one send.
pub const QUIET_S: Moment = 30;

/// How many times one move is asked again of a run that never took the
/// lock — the send refused for a run that held it, or left to it. Past it,
/// the move waits for the next one, or the next sync: a send that never
/// reaches the lock never loops.
pub const AGAIN_MOST: u8 = 3;

/// What starts the unit: systemd's `--user` manager — a test's recorder.
type Starter = fn(&str) -> Result<(), crate::systemd::Trouble>;

/// The entry: a reminder reading the files under this birth's roots, and the
/// runtime folder of this instance.
#[must_use]
pub fn new(birth: &Birth) -> impl Module + use<> {
    let root = |r: Root| match r {
        Root::Config => birth.roots.config.clone(),
        Root::State => birth.roots.state.clone(),
        Root::Home => None,
    };
    // THE PLAIN ROOTS, as the one reader of the tables names them.
    let plain = ronoctua_belongings::scope::plain_roots()
        .iter()
        .filter_map(|p| {
            let (r, rest) = Root::split(p.path)?;
            Some(root(r)?.join(rest))
        })
        .collect();
    Reminder::reading(Paths {
        prefs: birth.roots.config.as_ref().map(|c| c.join(FILE)),
        own: birth.roots.state.as_ref().map(|s| s.join(OWN)),
        runtime: birth
            .roots
            .config
            .as_ref()
            .and_then(|_| ronoctua_xdg::runtime_file(RUNTIME_LEAF)),
        plain,
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
    /// The plain part's roots: what changes anywhere under them is sent once
    /// quiet.
    plain: Vec<PathBuf>,
}

impl Paths {
    /// The lock one run of the belongings at a time holds (`run.lock` of the
    /// state folder): held while a run reads and writes, its time set when
    /// one takes it, closed when one ends.
    fn lock(&self) -> Option<PathBuf> {
        self.own.as_ref().map(|o| o.join(LOCK_FILE))
    }
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

/// The plain part's send, as far as the reminder knows (see the module's
/// header, "A send is kept until a run read it").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Plain {
    /// When the plain part last moved, while no run is known to have read it
    /// since.
    moved: Option<Moment>,
    /// When a run was asked to send it — the unit started, or the run that
    /// held the lock left to end first — by the wall clock the lock's time
    /// is written in. `None` while none is asked: the quiet's clock runs.
    asked: Option<SystemTime>,
    /// How many times this move was asked again of a run that never took
    /// the lock.
    again: u8,
}

/// The reminder.
struct Reminder {
    paths: Paths,
    /// What was read, and the page's generation — moved whenever it changes.
    kept: Mutex<(Seen, u64)>,
    /// The plain part's send.
    plain: Mutex<Plain>,
    /// What starts the plain sync's unit.
    start: Starter,
}

impl Reminder {
    fn reading(paths: Paths) -> Self {
        let seen = Seen::read(&paths);
        Self {
            paths,
            kept: Mutex::new((seen, 1)),
            plain: Mutex::new(Plain::default()),
            start: crate::systemd::start,
        }
    }

    /// The plain part's send, as it is.
    fn plain(&self) -> Plain {
        self.plain.lock().map(|p| *p).unwrap_or_default()
    }

    /// Changes the plain part's send.
    fn with_plain(&self, change: impl FnOnce(&mut Plain)) {
        if let Ok(mut p) = self.plain.lock() {
            change(&mut p);
        }
    }

    /// The moment the plain part, moved and quiet since, is sent — while no
    /// run is asked to send it already.
    fn send_at(&self) -> Option<Moment> {
        let p = self.plain();
        p.moved
            .filter(|_| p.asked.is_none())
            .map(|at| at.saturating_add(QUIET_S))
    }

    /// The plain part moved at `now`: it is sent [`QUIET_S`] after the last
    /// move — when this machine has a repository it synced with. A send
    /// asked before is no answer for it: a run that read the plain part
    /// already did not read this.
    fn plain_moved(&self, now: Moment) {
        let (seen, _) = self.seen();
        let synced = seen.repository && seen.last_s.is_some();
        self.with_plain(|p| {
            *p = Plain {
                moved: synced.then_some(now),
                asked: None,
                again: 0,
            };
        });
    }

    /// Whether a run of the belongings holds their lock now.
    fn busy(&self) -> bool {
        self.paths
            .lock()
            .is_some_and(|l| ronoctua_belongings::running::busy_at(&l))
    }

    /// When a run last took the lock (it sets the file's time then).
    fn lock_taken(&self) -> Option<SystemTime> {
        let lock = self.paths.lock()?;
        std::fs::metadata(lock).and_then(|m| m.modified()).ok()
    }

    /// Asks a run to send the plain part when it has been quiet long enough:
    /// starts the unit — or, while another run holds the lock (a sync the
    /// person started, which would refuse it), leaves it to that run's end,
    /// heard in the state folder ([`Reminder::settle_send`]). Never in a
    /// rehearsal. Says whether it started the unit.
    fn send_if_quiet(&self, ctx: &Ctx) -> bool {
        if self.send_at().is_none_or(|at| ctx.now() < at) {
            return false;
        }
        if ctx.rehearsing() {
            self.with_plain(|p| *p = Plain::default());
            return false;
        }
        // ASKED BEFORE IT STARTS: the run sets the lock's time after this.
        self.with_plain(|p| p.asked = Some(SystemTime::now()));
        if self.busy() {
            return false;
        }
        match (self.start)(super::unit::PLAIN) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(error = %e, "the plain part of the belongings could not be sent");
                self.with_plain(|p| *p = Plain::default());
                false
            }
        }
    }

    /// Something moved in the state folder — a run of the belongings may
    /// have ended: settles the send asked of one, once no run holds the
    /// lock. Says whether the clock may have changed.
    fn settle_send(&self) -> bool {
        let Some(asked) = self.plain().asked else {
            return false;
        };
        if self.busy() {
            return false;
        }
        let taken_since = self.lock_taken().is_some_and(|at| at >= asked);
        self.with_plain(|p| {
            if !taken_since && p.again < AGAIN_MOST {
                // NO RUN TOOK THE LOCK since the send was asked: it was
                // refused for the run that held it, or left to it. Asked
                // again, at once.
                p.asked = None;
                p.again += 1;
            } else {
                // A RUN READ THE PLAIN PART after the move and ended: sent,
                // or failed — the next move or sync sends it, and a send
                // that fails never loops. Or asked too often already.
                *p = Plain::default();
            }
        });
        true
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

impl Reminder {
    /// What it listens to, and which are the plain part's — one list, so a
    /// source's number says which it is. The plain roots are TREES: a folder
    /// made under one after the brain started is heard inside too.
    fn listened(&self) -> Vec<(Source, bool)> {
        [&self.paths.prefs, &self.paths.own, &self.paths.runtime]
            .into_iter()
            .flatten()
            .cloned()
            .map(|p| (Source::Path(p), false))
            .chain(
                self.paths
                    .plain
                    .iter()
                    .cloned()
                    .map(|p| (Source::Tree(p), true)),
            )
            .collect()
    }
}

impl Watcher for Reminder {
    fn sources(&self) -> Vec<Source> {
        self.listened().into_iter().map(|(s, _)| s).collect()
    }

    /// What moved while the brain was off — a sync from another door, a
    /// repository set by the installer — read at once.
    fn started(&self, _now: Moment, ctx: &Ctx) {
        self.reread();
        ctx.changed();
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        match heard {
            // THE CLOCK: the reminder came due, or the plain part has been
            // quiet long enough. Nothing on disk moved, the face did.
            Heard::Wake => {
                self.send_if_quiet(ctx);
                ctx.changed();
            }
            // THE PLAIN PART MOVED: sent once it stays quiet — the clock is
            // asked for again (`next_wake`).
            Heard::Path { source, .. }
                if self.listened().get(source).is_some_and(|(_, plain)| *plain) =>
            {
                self.plain_moved(ctx.now());
                ctx.changed();
            }
            // THE STATE FOLDER OR THE SETTINGS: a run may have ended, and
            // the face may say something else.
            Heard::Path { .. } | Heard::Back { .. } => {
                let settled = self.settle_send();
                if self.reread() || settled {
                    ctx.changed();
                }
            }
            _ => {}
        }
    }

    /// TWO CLOCKS AT MOST, the earlier armed: `last + period`, only while a
    /// repository is set, a period is chosen and the sync is not yet due; and
    /// the plain part's send, only while a move waits for its quiet.
    fn next_wake(&self, now: Moment) -> Option<Moment> {
        let due = self.seen().0.next_due(now);
        match (due, self.send_at()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

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
            plain: Vec::new(),
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

    /// How many times a test's starter was asked — one counter per test,
    /// since tests run at once.
    static STARTED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static STARTED_LOCKED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    static STARTED_WATCHED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn recorded(unit: &str) -> Result<(), crate::systemd::Trouble> {
        assert_eq!(unit, super::super::unit::PLAIN);
        STARTED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn recorded_locked(unit: &str) -> Result<(), crate::systemd::Trouble> {
        assert_eq!(unit, super::super::unit::PLAIN);
        STARTED_LOCKED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn recorded_watched(unit: &str) -> Result<(), crate::systemd::Trouble> {
        assert_eq!(unit, super::super::unit::PLAIN);
        STARTED_WATCHED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn started() -> usize {
        STARTED.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The plain part moved (heard on the source `source`), at `now`.
    fn moved(r: &Reminder, source: usize, path: &std::path::Path, now: Moment) {
        let (ctx, _) = Ctx::kept("fixture", now);
        r.heard(Heard::Path { source, path }, &ctx);
    }

    /// The clock rang at `now`.
    fn wake(r: &Reminder, now: Moment) {
        let (ctx, _) = Ctx::kept("fixture", now);
        r.heard(Heard::Wake, &ctx);
    }

    /// FAILS IF: the plain part is sent before it has been quiet for
    /// `QUIET_S` (a second move restarts the wait), not sent once it has, sent
    /// twice for one move, sent from a machine that never synced with the
    /// repository it has set (there is nowhere to send it), or sent by a
    /// rehearsing brain — or the plain roots are not listened to as trees.
    #[test]
    fn the_plain_part_is_sent_once_quiet_and_only_where_it_has_somewhere_to_go() {
        let (root, mut paths) = scratch("plain");
        let person = root.join("state/person");
        paths.plain = vec![person.clone()];
        let mut r = Reminder::reading(paths.clone());
        r.start = recorded;
        assert_eq!(
            r.sources().last(),
            Some(&Source::Tree(person.clone())),
            "the plain root is not a tree"
        );
        let declared = person.join("software/declared.json");
        let moved = |r: &Reminder, now: Moment| moved(r, 3, &declared, now);

        // NEVER SYNCED: nowhere to send it.
        moved(&r, 100);
        assert_eq!(r.next_wake(100), None, "a clock with nowhere to send");

        prefs(&paths, REPO, None);
        last(&paths, REPO, 0);
        assert!(r.reread());
        let before = started();
        moved(&r, 100);
        assert_eq!(r.next_wake(100), Some(100 + QUIET_S));
        moved(&r, 120);
        assert_eq!(
            r.next_wake(120),
            Some(120 + QUIET_S),
            "a move did not restart the wait"
        );
        wake(&r, 140);
        assert_eq!(started(), before, "sent before it was quiet");
        wake(&r, 120 + QUIET_S);
        assert_eq!(started(), before + 1, "not sent once quiet");
        assert_eq!(r.next_wake(200), None, "a clock after it was sent");
        wake(&r, 300);
        assert_eq!(started(), before + 1, "one move, two sends");

        // A REHEARSING BRAIN (the development loop's) sends nothing.
        moved(&r, 400);
        let kept = Arc::new(crate::module::Kept::default());
        let sink: Arc<dyn crate::module::Sink> = kept;
        let rehearsing = Ctx::new(
            "fixture",
            400 + QUIET_S,
            crate::Where::default(),
            true,
            sink,
        );
        r.heard(Heard::Wake, &rehearsing);
        assert_eq!(started(), before + 1, "a rehearsal sent the plain part");
        assert_eq!(r.next_wake(500), None);
        let _ = std::fs::remove_dir_all(root);
    }

    /// The places whose lock the reminder of `scratch(name)` reads.
    fn places_of(root: &std::path::Path) -> ronoctua_belongings::places::Places {
        ronoctua_belongings::places::Places {
            home: root.join("home"),
            config: root.join("config"),
            state: root.join("state"),
            runtime: root.join("run"),
        }
    }

    /// FAILS IF: a send is started while another run holds the lock (it
    /// would be refused, `Busy`) or is not started once that run ended; a
    /// move heard while a run reads the plain part is not sent after it; a
    /// send a run took (it took the lock after the send was asked) is sent
    /// again — the end of a run that failed would loop — or a send that
    /// never reaches the lock is asked again more than `AGAIN_MOST` times.
    #[test]
    fn a_send_is_kept_until_a_run_took_it() {
        let (root, mut paths) = scratch("locked");
        let places = places_of(&root);
        assert_eq!(paths.own.as_deref(), Some(places.own().as_path()));
        let person = root.join("state/person");
        paths.plain = vec![person.clone()];
        prefs(&paths, REPO, None);
        last(&paths, REPO, 0);
        let mut r = Reminder::reading(paths);
        r.start = recorded_locked;
        let started = || STARTED_LOCKED.load(std::sync::atomic::Ordering::SeqCst);
        let own = places.own();
        // THE STATE FOLDER MOVED: a run took the lock, or let it go.
        let ended = |r: &Reminder, now: Moment| moved(r, 1, &own, now);
        let declared = person.join("software/declared.json");

        // A SYNC OF THE PERSON'S HOLDS THE LOCK when the quiet is over: no
        // start (it would be refused), no clock — and its end starts it.
        let held = ronoctua_belongings::running::hold(&places).expect("the lock");
        moved(&r, 3, &declared, 100);
        wake(&r, 100 + QUIET_S);
        assert_eq!(started(), 0, "started while another run held the lock");
        assert_eq!(r.next_wake(200), None, "a clock while the run holds it");
        ended(&r, 200);
        assert_eq!(started(), 0);
        drop(held);
        ended(&r, 210);
        assert!(
            r.next_wake(210).is_some_and(|at| at <= 210),
            "not asked again"
        );
        wake(&r, 210);
        assert_eq!(started(), 1, "not started once the lock was let go");

        // OURS RUNS (it takes the lock after the send was asked) and a move
        // lands meanwhile: sent again, after it — and once only.
        let ours = ronoctua_belongings::running::hold(&places).expect("the lock");
        ended(&r, 220);
        moved(&r, 3, &declared, 230);
        wake(&r, 230 + QUIET_S);
        assert_eq!(started(), 1, "started while ours still ran");
        drop(ours);
        ended(&r, 300);
        wake(&r, 300);
        assert_eq!(started(), 2, "the move made while ours ran was never sent");
        let ours = ronoctua_belongings::running::hold(&places).expect("the lock");
        drop(ours);
        ended(&r, 310);
        assert_eq!(r.next_wake(310), None, "a send a run took is asked again");
        wake(&r, 400);
        assert_eq!(started(), 2);

        // A SEND THAT NEVER REACHES THE LOCK (refused before it, every
        // time): asked again a few times, then left for the next move.
        moved(&r, 3, &declared, 500);
        wake(&r, 500 + QUIET_S);
        for n in 0..u64::from(AGAIN_MOST) {
            ended(&r, 600);
            wake(&r, 600);
            assert_eq!(started(), 4 + usize::try_from(n).unwrap());
        }
        ended(&r, 700);
        assert_eq!(
            r.next_wake(700),
            None,
            "a send that never reached the lock loops"
        );
        assert_eq!(started(), 3 + usize::from(AGAIN_MOST));
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a declaration written in a folder of the plain part made
    /// AFTER the reminder started listening is not heard — inotify is not
    /// recursive, and a list of the folders there at the start is deaf to
    /// `software/`, which the first declaration makes. The real watch
    /// thread, the real reminder.
    #[test]
    fn a_folder_of_the_plain_part_made_later_is_heard_inside() {
        use crate::fake::Hand;
        use std::time::{Duration, Instant};
        let (root, mut paths) = scratch("watched");
        let person = root.join("state/person");
        std::fs::create_dir_all(&person).expect("person");
        paths.plain = vec![person.clone()];
        prefs(&paths, REPO, None);
        last(&paths, REPO, 0);
        let mut r = Reminder::reading(paths);
        r.start = recorded_watched;
        let hand = Hand::at(1000);
        let clock = Arc::clone(&hand.0);
        let core = Arc::new(crate::hub::Core::new(
            Arc::new(crate::mind::Shared::of(crate::prefs::Prefs::default())),
            crate::registry::Registry::of(vec![Box::new(r) as Box<dyn Module>])
                .expect("the registry"),
            crate::Where::default(),
            Box::new(hand),
        ));
        let watching = crate::watch::Watch::new(Arc::clone(&core))
            .reading(None)
            .keeping_cursor_in(None)
            .start()
            .expect("start")
            .expect("a watch");
        let wants = || {
            let now = clock.load(std::sync::atomic::Ordering::SeqCst);
            core.modules()
                .find(super::super::MANIFEST.id)
                .and_then(|m| m.watcher())
                .and_then(|w| w.next_wake(now))
        };
        // Waits until the reminder wants to send at `at`, doing `write`
        // again meanwhile (a watch raised a moment after a write misses it).
        let heard_at = |at: Moment, write: &dyn Fn()| {
            clock.store(at - QUIET_S, std::sync::atomic::Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(10);
            while wants() != Some(at) {
                write();
                std::thread::sleep(Duration::from_millis(200));
                assert!(
                    Instant::now() < deadline,
                    "never heard: the reminder wants {:?}",
                    wants()
                );
            }
        };
        // `software/` IS MADE after the start: heard at the root.
        let software = person.join("software");
        heard_at(2000 + QUIET_S, &|| {
            let _ = std::fs::remove_dir(&software);
            std::fs::create_dir(&software).expect("software");
        });
        std::thread::sleep(Duration::from_millis(1600));
        // AND A DECLARATION WRITTEN INSIDE IT, later.
        heard_at(3000 + QUIET_S, &|| {
            std::fs::write(software.join("declared.json"), "[]").expect("declared");
        });
        drop(watching);
        let _ = std::fs::remove_dir_all(root);
    }
}
