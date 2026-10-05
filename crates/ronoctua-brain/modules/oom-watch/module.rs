//! What died for lack of memory, written down and said — once per kill.
//!
//! The whole story is in `why.md` beside this file; what is here is its shape.
//!
//! # A watch on the journal the core already follows
//!
//! Three matches ([`Watcher::sources`]): the kernel's kill line, systemd's
//! word about a unit that lost a process (to the kernel or to oomd), and
//! oomd's own decision. They cost nothing at rest: the core runs ONE
//! follower for every module, and these only add what they ask for to what it
//! reads (`--priority` up to notice).
//!
//! # One kill, one record, one notification
//!
//! The two lines of one kill are put together by [`rules::Pairing`], and a
//! line heard again — the few seconds a restart re-reads — is not a second
//! kill: what was told is remembered on disk, by pid, cgroup or unit, with the
//! second it was written.
//!
//! # And the footer's own notice of oomd
//!
//! The footer already raises a notification when oomd's bus signal arrives.
//! This kill's event carries the tag of that notification's catalogue entry
//! and the cgroup and the second of the kill, so the footer ADDS these
//! details to the notification it raised instead of raising a second one —
//! the matching is the footer's (`ronoctua_notify::oom::Kills`).

mod rules;

#[cfg(test)]
mod fixtures;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::module::{
    Arg, Birth, Cell, Ctx, Event, Face, Field, Heard, Look, Manifest, Match, Module, Noted, Row,
    Screen, Section, Source, Watcher,
};
use crate::moment::Moment;

use rules::{Kill, Pairing, Piece, Waiting};

/// The file its memory is kept in, under its state directory.
const KEPT_FILE: &str = "kills.json";

/// `notice` in syslog's scale: systemd's word about a unit, and oomd's
/// decision, are written at it.
const NOTICE: crate::module::Priority = 5;

/// The source each match is, by its index in [`Watcher::sources`].
const KERNEL: usize = 0;

/// Reads the machine's memory stall now, in per mille of the time.
type Stall = Box<dyn Fn() -> Option<u16> + Send + Sync>;

/// The entry: the journal's matches, the memory of what was told, and the
/// machine's stall read through the one crate that reads `/proc`.
#[must_use]
pub fn new(birth: &Birth) -> impl Module + use<> {
    let dir = birth.state_dir(super::ID);
    OomWatch::keeping(
        dir,
        // `None` ON A KERNEL WITHOUT PSI: the sentence then says no stall was
        // measured, instead of a stall of zero.
        Box::new(|| {
            ronoctua_memory::machine::memory_pressure(&ronoctua_memory::Places::default())
                .map(|p| p.some10)
        }),
    )
}

/// The watch, and what it remembers.
struct OomWatch {
    /// Where its memory is kept; `None` keeps it nowhere (a test).
    dir: Option<PathBuf>,
    /// The machine's stall, read when a fresh kernel kill is heard.
    stall: Stall,
    state: Mutex<State>,
}

/// What it remembers.
#[derive(Debug, Default)]
struct State {
    pairing: Pairing,
    /// The last kills told, newest last — its page.
    recent: VecDeque<Told>,
    /// Why the journal is not followed, while it is not: a core word
    /// (`brain-journal-why-<word>`).
    lost: Option<String>,
    /// Its page's generation.
    generation: u64,
}

/// A kill as its page and the file keep it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Told {
    by: String,
    what: String,
    place: Option<String>,
    at: Moment,
}

/// What is kept on disk: what was told, by the keys of its lines, and the
/// page's kills.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Kept {
    told: Vec<(String, Moment)>,
    recent: Vec<Told>,
}

impl OomWatch {
    /// One keeping its memory in `dir`, reading the stall with `stall`.
    fn keeping(dir: Option<PathBuf>, stall: Stall) -> Self {
        let kept: Kept = dir
            .as_deref()
            .and_then(|d| std::fs::read_to_string(d.join(KEPT_FILE)).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let state = State {
            pairing: Pairing {
                told: kept.told.into_iter().collect(),
                ..Pairing::default()
            },
            recent: kept.recent.into_iter().collect(),
            ..State::default()
        };
        Self {
            dir,
            stall,
            state: Mutex::new(state),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Writes what is remembered — a kill is rare, so at once.
    fn save(&self, state: &State) {
        let Some(dir) = self.dir.as_deref() else {
            return;
        };
        let kept = Kept {
            told: state.pairing.told.iter().cloned().collect(),
            recent: state.recent.iter().cloned().collect(),
        };
        let written = serde_json::to_string(&kept)
            .map_err(std::io::Error::other)
            .and_then(|text| ronoctua_xdg::write_atomically(dir, KEPT_FILE, text.as_bytes()));
        if let Err(e) = written {
            tracing::warn!(error = %e, "what was closed for lack of memory could not be kept");
        }
    }

    /// Records a kill in the activity and tells it — each kill once.
    fn told(&self, state: &mut State, kills: Vec<Kill>, ctx: &Ctx) {
        if kills.is_empty() {
            return;
        }
        for kill in kills {
            tracing::info!(what = %kill.what, by = kill.by.word(), at = kill.at, "closed for lack of memory");
            let mut noted = Noted::of(super::key::OOM_WATCH_NOTED_KILLED);
            let mut event = Event::of(&super::event::KILLED);
            for (name, arg) in args_of(&kill) {
                noted = noted.with(name, arg.clone());
                event = event.with(name, arg);
            }
            ctx.record(noted);
            ctx.tell(event);
            state.recent.push_back(Told {
                by: kill.by.word().to_owned(),
                what: kill.what.clone(),
                place: kill.place().map(ToOwned::to_owned),
                at: kill.at,
            });
            while state.recent.len() > rules::RECENT_KEPT {
                state.recent.pop_front();
            }
        }
        state.generation += 1;
        self.save(state);
        ctx.changed();
    }
}

/// The values of a kill's sentences — the notification's, and the activity
/// record's. EVERY ONE IS ALWAYS THERE: a Fluent variable that is missing is
/// drawn as its own name in braces. What was not measured is a key that says
/// so, or a selector word.
fn args_of(kill: &Kill) -> Vec<(&'static str, Arg)> {
    let unmeasured = || Arg::Key(super::key::OOM_WATCH_UNMEASURED.into());
    vec![
        ("what", Arg::Text(kill.what.clone())),
        (
            "where",
            kill.place().map_or_else(
                || Arg::Key(super::key::OOM_WATCH_NOWHERE.into()),
                |p| Arg::Text(p.to_owned()),
            ),
        ),
        ("by", Arg::Text(kill.by.word().to_owned())),
        ("pid", Arg::Number(kill.pid.map_or(0, i64::from))),
        (
            "memory",
            kill.memory_bytes.map_or_else(unmeasured, Arg::Bytes),
        ),
        (
            "measured",
            Arg::Text(if kill.stall_read() { "yes" } else { "no" }.to_owned()),
        ),
        ("stall", Arg::PerMille(kill.stall_per_mille.unwrap_or(0))),
        ("limit", Arg::PerMille(kill.limit_per_mille.unwrap_or(0))),
        ("at", Arg::Moment(kill.at)),
        // FOR THE FOOTER'S MATCHING, not for a sentence: the cgroup oomd's bus
        // signal names, and the second of the kill.
        ("cgroup", Arg::Text(kill.cgroup.clone().unwrap_or_default())),
        ("epoch", Arg::Number(kill.at)),
    ]
}

/// Who killed it, in the page's words — from the word it was kept under.
fn by_key(word: &str) -> &'static str {
    use super::key as k;
    match word {
        "machine" => k::OOM_WATCH_BY_MACHINE,
        "limit" => k::OOM_WATCH_BY_LIMIT,
        "pressure" => k::OOM_WATCH_BY_PRESSURE,
        "full" => k::OOM_WATCH_BY_FULL,
        "oomd" => k::OOM_WATCH_BY_OOMD,
        _ => k::OOM_WATCH_BY_KERNEL,
    }
}

/// A text key as a cell.
fn key(k: &'static str) -> Cell {
    Cell::Key(k.into())
}

impl Module for OomWatch {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }

    fn face(&self, _look: &Look<'_>) -> Face {
        // NO MARK: a kill is told once, as it happens, and the activity keeps
        // it. Only the page's generation.
        Face {
            screen: self.state().generation,
            ..Face::default()
        }
    }

    fn screen(&self, _look: &Look<'_>) -> Screen {
        let state = self.state();
        let mut sections = Vec::new();
        // A JOURNAL THAT IS NOT FOLLOWED IS SAID, in the core's words — an
        // empty list under it would read as a machine that never ran short.
        if let Some(why) = &state.lost {
            sections.push(Section {
                heading: super::key::OOM_WATCH_HEADING_JOURNAL.into(),
                rows: vec![Row {
                    id: "journal".to_owned(),
                    label: Cell::Key(format!("brain-journal-why-{why}").into()),
                    ..Row::default()
                }],
            });
        }
        let rows = if state.recent.is_empty() {
            vec![Row {
                id: "none".to_owned(),
                label: key(super::key::OOM_WATCH_ROW_NONE),
                ..Row::default()
            }]
        } else {
            state
                .recent
                .iter()
                .rev()
                .enumerate()
                .map(|(n, t)| Row {
                    id: format!("kill-{n}"),
                    label: Cell::Moment(t.at),
                    value: Cell::Data(t.what.clone()),
                    note: key(by_key(&t.by)),
                    ..Row::default()
                })
                .collect()
        };
        sections.push(Section {
            heading: super::key::OOM_WATCH_HEADING_RECENT.into(),
            rows,
        });
        Screen { sections }
    }
}

impl Watcher for OomWatch {
    fn sources(&self) -> Vec<Source> {
        let field = |name: &str, any_of: &[&str]| Field {
            name: name.to_owned(),
            any_of: any_of.iter().map(|v| (*v).to_owned()).collect(),
        };
        vec![
            // KERNEL: the kernel's own kill line, at `err`.
            Source::Journal(Match {
                max_priority: Some(ronoctua_journal::ERR),
                fields: vec![field("_TRANSPORT", &["kernel"])],
                message_contains: Some(rules::KERNEL_SAYS.to_owned()),
                ..Match::default()
            }),
            // systemd's word about a unit that lost a process, to either.
            Source::Journal(Match {
                max_priority: Some(NOTICE),
                fields: vec![field("MESSAGE_ID", &[rules::UNIT_OOM, rules::UNIT_OOMD])],
                ..Match::default()
            }),
            // oomd's decision.
            Source::Journal(Match {
                max_priority: Some(NOTICE),
                fields: vec![field("SYSLOG_IDENTIFIER", &[rules::OOMD])],
                message_contains: Some(rules::OOMD_SAYS.to_owned()),
                ..Match::default()
            }),
        ]
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        let now = ctx.now();
        let mut state = self.state();
        match heard {
            Heard::Journal { source, entry } => {
                let Some(piece) = Piece::of(entry) else {
                    return;
                };
                let written = i64::try_from(entry.realtime_us / 1_000_000).unwrap_or(now);
                // THE STALL NOW says something about a kill only when the kill
                // IS now — and only the kernel's line needs it: oomd wrote
                // its own measure into its line.
                let stall = (source == KERNEL && rules::fresh(written, now))
                    .then(|| (self.stall)())
                    .flatten();
                let kill = state.pairing.heard(Waiting {
                    piece,
                    written,
                    heard: now,
                    stall_per_mille: stall,
                });
                self.told(&mut state, kill.into_iter().collect(), ctx);
            }
            Heard::Wake => {
                let due = state.pairing.due(now);
                self.told(&mut state, due, ctx);
            }
            Heard::Lost { why, .. } => {
                if state.lost.as_deref() != Some(why) {
                    state.lost = Some(why.to_owned());
                    state.generation += 1;
                    ctx.changed();
                }
            }
            Heard::Back { .. } => {
                if state.lost.take().is_some() {
                    state.generation += 1;
                    ctx.changed();
                }
            }
            Heard::Path { .. } | Heard::Line { .. } => {}
        }
    }

    fn next_wake(&self, _now: Moment) -> Option<Moment> {
        // A CLOCK ONLY WHILE A LINE WAITS FOR ITS PARTNER.
        self.state().pairing.next_due()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Entry;

    fn watch() -> OomWatch {
        OomWatch::keeping(None, Box::new(|| Some(123)))
    }

    fn hear(m: &OomWatch, ctx: &Ctx, source: usize, line: &str) {
        let entry = Entry::parse(line).expect("a fixture that is not a journal line");
        assert!(
            m.sources().iter().enumerate().any(|(n, s)| n == source
                && matches!(s, Source::Journal(wanted) if wanted.accepts(&entry))),
            "source {source} does not accept the fixture {line}"
        );
        m.heard(
            Heard::Journal {
                source,
                entry: &entry,
            },
            ctx,
        );
    }

    fn arg<'a>(args: &'a [(crate::module::Word, Arg)], name: &str) -> &'a Arg {
        &args
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no `{name}` among {args:?}"))
            .1
    }

    #[test]
    fn a_kernel_kill_in_a_small_group_is_one_record_and_one_notification() {
        let m = watch();
        // THE MOMENT OF THE FIXTURES: the kill is fresh, so the stall is read.
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT + 1);
        hear(&m, &ctx, KERNEL, fixtures::KERNEL_LIMIT);
        assert!(
            kept.events().is_empty(),
            "told before the unit's line could come"
        );
        hear(&m, &ctx, 1, fixtures::UNIT_KERNEL);
        let events = kept.events();
        assert_eq!(events.len(), 1, "one kill, {} events", events.len());
        let told = &events[0].1;
        assert_eq!(told.tag, super::super::event::KILLED.tag);
        assert_eq!(arg(&told.args, "what"), &Arg::Text("tail".to_owned()));
        assert_eq!(
            arg(&told.args, "where"),
            &Arg::Text("hog.service".to_owned())
        );
        assert_eq!(arg(&told.args, "by"), &Arg::Text("limit".to_owned()));
        assert_eq!(arg(&told.args, "pid"), &Arg::Number(4242));
        assert_eq!(
            arg(&told.args, "memory"),
            &Arg::Bytes((31_744 + 1_280) * 1024)
        );
        assert_eq!(arg(&told.args, "measured"), &Arg::Text("yes".to_owned()));
        assert_eq!(arg(&told.args, "stall"), &Arg::PerMille(123));
        assert_eq!(arg(&told.args, "at"), &Arg::Moment(fixtures::AT));
        assert_eq!(arg(&told.args, "epoch"), &Arg::Number(fixtures::AT));
        // AND ONE RECORD IN THE ACTIVITY, with the same values.
        let records = kept.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].2.says, super::super::key::OOM_WATCH_NOTED_KILLED);
        assert_eq!(records[0].2.args, told.args);
        // AND NO CLOCK LEFT ARMED.
        assert_eq!(m.next_wake(fixtures::AT + 1), None);
        // ITS PAGE LISTS IT.
        let screen = m.screen(&look_of(&crate::pulse::Pulse::default()));
        let rows = &screen.sections[0].rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, Cell::Moment(fixtures::AT));
        assert_eq!(rows[0].value, Cell::Data("tail".to_owned()));
        assert_eq!(rows[0].note, key(super::super::key::OOM_WATCH_BY_LIMIT));
    }

    fn look_of(pulse: &crate::pulse::Pulse) -> Look<'_> {
        static MINE: std::sync::OnceLock<crate::prefs::TaskPrefs> = std::sync::OnceLock::new();
        Look {
            now: 0,
            pulse,
            mine: MINE.get_or_init(|| super::super::MANIFEST.defaults()),
            last: None,
        }
    }

    #[test]
    fn an_oomd_kill_carries_its_cgroup_and_its_pressure_for_the_footer() {
        let m = watch();
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT + 1);
        hear(&m, &ctx, 2, fixtures::OOMD_PRESSURE);
        hear(&m, &ctx, 1, fixtures::UNIT_OOMD);
        let events = kept.events();
        assert_eq!(events.len(), 1);
        let args = &events[0].1.args;
        assert_eq!(
            arg(args, "cgroup"),
            &Arg::Text(fixtures::HOG_CGROUP.to_owned()),
            "the footer cannot match the bus's notification without the cgroup"
        );
        assert_eq!(arg(args, "by"), &Arg::Text("pressure".to_owned()));
        assert_eq!(arg(args, "stall"), &Arg::PerMille(586));
        assert_eq!(arg(args, "limit"), &Arg::PerMille(500));
        // THE UNIT IS WHAT DIED, so where it was is its cgroup, slice and all.
        assert_eq!(arg(args, "what"), &Arg::Text("hog.service".to_owned()));
        assert_eq!(
            arg(args, "where"),
            &Arg::Text(fixtures::HOG_CGROUP.to_owned())
        );
        assert_eq!(arg(args, "pid"), &Arg::Number(0));
        // oomd's measure is in its own sentence, not said again as the
        // machine's.
        assert_eq!(arg(args, "measured"), &Arg::Text("no".to_owned()));
        assert_eq!(
            arg(args, "memory"),
            &Arg::Key(super::super::key::OOM_WATCH_UNMEASURED.into()),
            "a pressure kill has no memory figure, and says so"
        );
    }

    #[test]
    fn a_line_alone_is_told_when_its_clock_rings_and_not_before() {
        let m = watch();
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT);
        hear(&m, &ctx, KERNEL, fixtures::KERNEL_MACHINE);
        let wake = m
            .next_wake(fixtures::AT)
            .expect("no clock for a waiting line");
        assert!(wake > fixtures::AT);
        m.heard(Heard::Wake, &Ctx::kept(super::super::ID, wake - 1).0);
        assert!(kept.events().is_empty(), "told before its patience ran out");
        let (later, told) = Ctx::kept(super::super::ID, wake);
        m.heard(Heard::Wake, &later);
        let events = told.events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            arg(&events[0].1.args, "by"),
            &Arg::Text("machine".to_owned())
        );
        assert_eq!(
            arg(&events[0].1.args, "where"),
            &Arg::Key(super::super::key::OOM_WATCH_NOWHERE.into())
        );
    }

    #[test]
    fn a_kill_heard_again_after_a_restart_is_not_told_again() {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-oom-watch-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let first = OomWatch::keeping(Some(dir.clone()), Box::new(|| None));
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT);
        hear(&first, &ctx, KERNEL, fixtures::KERNEL_LIMIT);
        hear(&first, &ctx, 1, fixtures::UNIT_KERNEL);
        assert_eq!(kept.events().len(), 1);
        // THE BRAIN RESTARTS, and its cursor was a few seconds behind: both
        // lines come again.
        let second = OomWatch::keeping(Some(dir.clone()), Box::new(|| None));
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT + 60);
        hear(&second, &ctx, KERNEL, fixtures::KERNEL_LIMIT);
        hear(&second, &ctx, 1, fixtures::UNIT_KERNEL);
        second.heard(
            Heard::Wake,
            &Ctx::kept(super::super::ID, fixtures::AT + 600).0,
        );
        assert!(
            kept.events().is_empty(),
            "one kill told twice across a restart"
        );
        assert_eq!(
            second.state().recent.len(),
            1,
            "the page forgot what it showed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_kill_counted_by_every_group_above_it_is_one_notification() {
        // AS A REAL JOURNAL HAS IT: the scope's line first (from the person's
        // manager), the kernel's, then the system's manager about the person's
        // manager — three lines, one kill, told with the scope.
        let m = watch();
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT + 101);
        hear(&m, &ctx, 1, fixtures::UNIT_KERNEL_SCOPE);
        hear(&m, &ctx, KERNEL, fixtures::KERNEL_IN_A_SCOPE);
        hear(&m, &ctx, 1, fixtures::UNIT_KERNEL_MANAGER_AFTER);
        m.heard(
            Heard::Wake,
            &Ctx::kept(super::super::ID, fixtures::AT + 200).0,
        );
        let events = kept.events();
        assert_eq!(events.len(), 1, "one kill, {} events", events.len());
        let args = &events[0].1.args;
        assert_eq!(arg(args, "what"), &Arg::Text("nix".to_owned()));
        assert_eq!(
            arg(args, "where"),
            &Arg::Text("run-p5150-i9745490.scope".to_owned()),
            "the kill was put in the group that holds its unit"
        );
        assert_eq!(m.next_wake(fixtures::AT + 200), None);
        // AND THE MANAGER'S LINE BEFORE THE UNIT'S — the other order a
        // journal has — takes nothing from the hog's kill.
        let m = watch();
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT + 1);
        hear(&m, &ctx, 1, fixtures::UNIT_KERNEL_MANAGER);
        hear(&m, &ctx, KERNEL, fixtures::KERNEL_LIMIT);
        hear(&m, &ctx, 1, fixtures::UNIT_KERNEL);
        m.heard(
            Heard::Wake,
            &Ctx::kept(super::super::ID, fixtures::AT + 100).0,
        );
        let events = kept.events();
        assert_eq!(events.len(), 1, "one kill, {} events", events.len());
        assert_eq!(
            arg(&events[0].1.args, "where"),
            &Arg::Text("hog.service".to_owned())
        );
    }

    #[test]
    fn a_reload_that_repeats_old_counts_tells_nothing() {
        // `daemon-reload` after a kill earlier in the boot: the slices say it
        // again, and so may a leaf — with no kernel line anywhere near.
        let m = watch();
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT + 5001);
        for line in [
            fixtures::RELOAD_USER_SLICE,
            fixtures::RELOAD_USER_1000_SLICE,
            fixtures::RELOAD_LEAF,
            fixtures::LONE_APP_SLICE,
        ] {
            hear(&m, &ctx, 1, line);
        }
        let (later, told) = Ctx::kept(super::super::ID, fixtures::AT + 6000);
        m.heard(Heard::Wake, &later);
        assert!(
            kept.events().is_empty() && told.events().is_empty(),
            "a reload was told as a kill"
        );
        assert!(kept.records().is_empty() && told.records().is_empty());
        assert_eq!(m.next_wake(fixtures::AT + 6000), None, "a clock left armed");
    }

    #[test]
    fn a_kernel_without_psi_says_no_stall_was_measured() {
        let m = OomWatch::keeping(None, Box::new(|| None));
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT + 1);
        hear(&m, &ctx, KERNEL, fixtures::KERNEL_LIMIT);
        hear(&m, &ctx, 1, fixtures::UNIT_KERNEL);
        let args = &kept.events()[0].1.args;
        assert_eq!(arg(args, "measured"), &Arg::Text("no".to_owned()));
    }

    #[test]
    fn an_old_kill_read_late_carries_no_stall_of_now() {
        let m = watch();
        let (ctx, kept) = Ctx::kept(super::super::ID, fixtures::AT + 3600);
        hear(&m, &ctx, KERNEL, fixtures::KERNEL_LIMIT);
        hear(&m, &ctx, 1, fixtures::UNIT_KERNEL);
        let args = &kept.events()[0].1.args;
        assert_eq!(arg(args, "measured"), &Arg::Text("no".to_owned()));
    }

    #[test]
    fn a_journal_not_followed_is_said_on_its_page_and_its_return_too() {
        let m = watch();
        let (ctx, kept) = Ctx::kept(super::super::ID, 1);
        let before = m.face(&look_of(&crate::pulse::Pulse::default())).screen;
        m.heard(
            Heard::Lost {
                source: 0,
                why: "unreadable",
            },
            &ctx,
        );
        let screen = m.screen(&look_of(&crate::pulse::Pulse::default()));
        assert_eq!(
            screen.sections[0].rows[0].label,
            Cell::Key("brain-journal-why-unreadable".into())
        );
        assert!(m.face(&look_of(&crate::pulse::Pulse::default())).screen > before);
        m.heard(Heard::Back { source: 0 }, &ctx);
        assert_eq!(
            m.screen(&look_of(&crate::pulse::Pulse::default()))
                .sections
                .len(),
            1
        );
        assert_eq!(kept.changes().len(), 2);
    }

    #[test]
    fn every_line_it_asks_for_is_one_the_follower_reads() {
        // THE FOLLOWER READS AT THE LOOSEST PRIORITY ASKED — and at no looser:
        // notice, never info, where the journal's bulk is.
        let m = watch();
        let matches: Vec<Match> = m
            .sources()
            .into_iter()
            .filter_map(|s| match s {
                Source::Journal(m) => Some(m),
                _ => None,
            })
            .collect();
        assert_eq!(ronoctua_journal::loosest(&matches), Some(NOTICE));
        // AND NONE ASKS FOR THE PAST: a kill from before it watched is not news
        // (and the core's pass over another module's past reads at the
        // follower's priority, so it leaves none of it for the follower).
        assert!(matches.iter().all(|m| m.backfill.most() == 0));
        // AND A LINE THAT IS NOT A KILL passes none of them.
        let noise = Entry::parse(fixtures::NOT_A_KILL).expect("fixture");
        assert!(matches.iter().all(|m| !m.accepts(&noise)));
    }
}
