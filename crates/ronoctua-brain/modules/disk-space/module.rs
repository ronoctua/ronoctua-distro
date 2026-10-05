//! Making room on the disk when it is short, and marking the modeline while
//! it is nearly full.
//!
//! The whole story — why the brain, why now and not at night, why a relief
//! that could not reach the floor does not repeat itself — is in `why.md`
//! beside this file. What is here is the shape of it.
//!
//! # The brain owns the WHEN and nothing else
//!
//! The work is a unit and a binary of its own (`module.nix`,
//! `ronoctua-disk-relief` in the sweep's crate): it blocks nothing here, it
//! runs at idle priority, and it stays reachable by hand. What is left for the
//! brain is what it sees and nobody else does: its heartbeat already reads the
//! filesystems under the store and the home ([`Pulse::rooms`]).
//!
//! # Not a calendar, and not behind the gate
//!
//! A disk running short is a STATE, and the answer to it is "now". So this is
//! never due by the calendar ([`Scheduled::due`] says why not, on screen) and
//! comes due through [`Scheduled::wanted_now`]: under the floor, once the gap
//! since the last run — a rehearsal included — has passed. Its manifest takes
//! it out of the gate: the machine is busiest exactly while a build is
//! filling its disk.
//!
//! # The mark
//!
//! While a filesystem it measures is under its NEARLY-FULL line — a line under
//! the floor ([`rules`]) — the modeline carries one mark, in the warning's
//! colour. At rest there is none (invariant 14): the relief handles the floor
//! by itself, and the mark is for what it could not free. A click opens this
//! module's page, which says how much is free and where both lines are.
//!
//! # What it tells the person
//!
//! What a real relief freed and how much is free now, or — once, until room is
//! made again — that the disk is still short. Measured here, before and after,
//! and told through [`crate::module::Ctx::tell`]; a rehearsal changed nothing
//! and tells nothing.

mod rules;

use std::path::PathBuf;
use std::sync::Mutex;

use crate::module::{
    Arg, Birth, Cell, Event, Face, Look, Manifest, Mark, Module, Row, Run, Scheduled, Screen,
    Section, Tone,
};
use crate::moment::Moment;
use crate::prefs::TaskPrefs;
use crate::pulse::{Pulse, Room};
use crate::systemd;
use crate::task::{Due, Outcome, Watched};

use rules::Relief;

/// The entry: one relief, measuring the store's filesystem and the home's —
/// the same paths the pulse measures, so the rooms it reads on the heartbeat
/// and the ones it measures around a run are the same filesystems.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    DiskSpace::measuring(crate::pulse::disks())
}

/// The relief, and what it remembers between runs.
struct DiskSpace {
    /// The paths whose filesystems it measures before and after a run —
    /// handed in, so a test measures nothing of the machine.
    disks: Vec<PathBuf>,
    /// What it remembers. In memory: a brain that restarted runs one more
    /// relief and says "still short" once more, which is the cheaper mistake.
    kept: Mutex<Kept>,
}

/// What the last real relief left, and whether "still short" was said since
/// room was last made.
#[derive(Debug, Default)]
struct Kept {
    left: Option<Relief>,
    said_short: bool,
}

impl DiskSpace {
    /// One measuring the filesystems holding these paths.
    fn measuring(disks: Vec<PathBuf>) -> Self {
        Self {
            disks,
            kept: Mutex::new(Kept::default()),
        }
    }

    /// What the last real relief left, if this brain saw one.
    fn left(&self) -> Option<Relief> {
        self.kept.lock().map_or(None, |k| k.left)
    }

    /// Remembers a real relief, and says what it tells — if anything.
    fn relieved(&self, relief: Relief) -> Option<Event> {
        let Ok(mut kept) = self.kept.lock() else {
            return None;
        };
        kept.left = Some(relief);
        let (event, said_short) = told(relief, kept.said_short);
        kept.said_short = said_short;
        event
    }
}

/// What a real relief tells the footer, and whether "still short" has now
/// been said.
///
/// "STILL SHORT" ONCE: what fills the disk then is the person's to choose, and
/// the sentence that says so does not need saying again after every relief
/// that finds the same — unread, each one would pile up on the bell. It is said
/// again only after a relief has made room.
fn told(relief: Relief, said_short: bool) -> (Option<Event>, bool) {
    let free = Arg::PerMille(relief.free_per_mille);
    if relief.short {
        let event = (!said_short).then(|| Event::of(&super::event::STILL_SHORT).with("free", free));
        (event, true)
    } else {
        let event = Event::of(&super::event::RELIEVED)
            .with("freed", Arg::Bytes(relief.freed_bytes))
            .with("free", free);
        (Some(event), false)
    }
}

/// The page's generation: the moment the reading it draws was taken. It moves
/// when there is a new reading and only then, which is when the page could
/// say something else — and it only ever grows, which is what the host and the
/// buffer compare.
fn generation(pulse: &Pulse) -> u64 {
    u64::try_from(pulse.at).unwrap_or(0)
}

/// A text key as a cell.
fn key(k: &'static str) -> Cell {
    Cell::Key(k.into())
}

/// The page's rows: the filesystem that decides — how much of it is free, how
/// big it is, and where the two lines are on it.
///
/// NOTHING MEASURED IS SAID, in a row, rather than drawn as a page of zeros.
fn rows_of(rooms: &[Room]) -> Vec<Row> {
    let Some(room) = rules::deciding(rooms) else {
        return vec![Row {
            id: "unmeasured".to_owned(),
            label: key(super::key::DISK_SPACE_ROW_UNMEASURED),
            ..Row::default()
        }];
    };
    let row = |id: &str, label: &'static str, bytes: u64, tone: Tone| Row {
        id: id.to_owned(),
        label: key(label),
        value: Cell::Bytes(bytes),
        tone,
        ..Row::default()
    };
    let full = room.free < rules::nearly_full_of(room.total);
    vec![
        row(
            "free",
            super::key::DISK_SPACE_ROW_FREE,
            room.free,
            if full { Tone::Warning } else { Tone::Quiet },
        ),
        row(
            "size",
            super::key::DISK_SPACE_ROW_SIZE,
            room.total,
            Tone::Quiet,
        ),
        row(
            "floor",
            super::key::DISK_SPACE_ROW_FLOOR,
            rules::floor_of(room.total),
            Tone::Quiet,
        ),
        row(
            "nearly-full",
            super::key::DISK_SPACE_ROW_NEARLY_FULL,
            rules::nearly_full_of(room.total),
            Tone::Quiet,
        ),
    ]
}

impl Module for DiskSpace {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }

    fn face(&self, look: &Look<'_>) -> Face {
        let mut face = Face {
            screen: generation(look.pulse),
            ..Face::default()
        };
        // A MARK EXISTS ONLY WHEN IT SAYS SOMETHING (invariant 14).
        if rules::nearly_full(&look.pulse.rooms) {
            face.marks.push(Mark {
                id: super::mark::FULL,
                tone: Tone::Warning,
                count: 0,
                act: None,
            });
        }
        face
    }

    fn screen(&self, look: &Look<'_>) -> Screen {
        Screen {
            sections: vec![Section {
                heading: super::key::DISK_SPACE_HEADING_NOW.into(),
                rows: rows_of(&look.pulse.rooms),
            }],
        }
    }
}

impl Scheduled for DiskSpace {
    fn due(&self, _now: Moment, _mine: &TaskPrefs, _last: Option<Moment>) -> Due {
        if !systemd::reachable() {
            return Due::Not {
                why: "no-systemctl",
            };
        }
        // NEVER BY THE CALENDAR, and the row says so instead of a date.
        Due::Not {
            why: super::why::WHEN_SHORT,
        }
    }

    fn wanted_now(
        &self,
        now: Moment,
        pulse: &Pulse,
        _mine: &TaskPrefs,
        last: Option<Moment>,
    ) -> bool {
        systemd::reachable()
            && rules::short_and_rested(
                rules::short_of(&pulse.rooms),
                pulse.disk_free_per_mille,
                now,
                last,
                self.left(),
            )
    }

    fn run(&self, run: &Run) -> Outcome {
        let unit = if run.is_rehearsal() {
            super::unit::REHEARSE
        } else {
            super::unit::RUN
        };
        let before = crate::pulse::rooms(&self.disks);
        // THE MARK BEFORE THE START: a unit quick enough to write its report
        // before the watch began would otherwise be missed.
        let mark = run.mark();
        if let Err(e) = systemd::start(unit) {
            return Outcome::Failed {
                at: String::new(),
                why: e.to_string(),
            };
        }
        match run.watch(mark) {
            Watched::Ended(sheet) => {
                // A REHEARSAL CHANGED NOTHING, and says nothing about the disk.
                if !run.is_rehearsal()
                    && let Some(relief) =
                        rules::relief_of(&before, &crate::pulse::rooms(&self.disks))
                    && let Some(event) = self.relieved(relief)
                {
                    run.ctx().tell(event);
                }
                if sheet.trouble.is_empty() {
                    Outcome::Done(sheet.steps)
                } else {
                    // NO `at`: the steps' ids are the unit's words, and the
                    // screen draws `at` as the update's step names. The trouble
                    // names the step itself (`store-garbage: …`).
                    Outcome::Failed {
                        at: String::new(),
                        why: sheet.trouble,
                    }
                }
            }
            Watched::Runaway(_) => Outcome::Failed {
                at: String::new(),
                why: "runaway".to_owned(),
            },
            Watched::Aborted(_) => {
                // GIVING UP IS WHAT STOPS IT: a relief the brain stopped
                // watching would go on deleting behind a row that says aborted.
                if let Err(e) = systemd::stop(unit) {
                    tracing::warn!(error = %e, "could not stop the disk's relief");
                }
                Outcome::Failed {
                    at: String::new(),
                    why: "aborted".to_owned(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;
    const GIB: u64 = 1024 * 1024 * 1024;

    fn pulse(rooms: Vec<Room>) -> Pulse {
        Pulse {
            at: 42,
            disk_free_per_mille: rooms.iter().map(|r| r.free_per_mille()).min(),
            rooms,
            ..Pulse::default()
        }
    }

    fn face_of(m: &DiskSpace, pulse: &Pulse) -> Face {
        let mine = super::super::MANIFEST.defaults();
        m.face(&Look {
            now: 100,
            pulse,
            mine: &mine,
            last: None,
        })
    }

    #[test]
    fn it_is_never_due_by_the_calendar_and_says_why_in_its_own_words() {
        let m = DiskSpace::measuring(Vec::new());
        let why = match m.due(0, &super::super::MANIFEST.defaults(), None) {
            Due::Not { why } => why,
            other => panic!("a relief came due by the calendar: {other:?}"),
        };
        if systemd::reachable() {
            assert_eq!(why, super::super::why::WHEN_SHORT);
            // AND THE WORD IS ITS OWN, drawn with its own key — never a core
            // text that names it.
            assert_eq!(
                super::super::MANIFEST.why_key(why),
                Some(super::super::key::DISK_SPACE_WHY_WHEN_SHORT)
            );
        } else {
            assert_eq!(why, "no-systemctl");
        }
        assert!(
            !super::super::MANIFEST.gated(),
            "the gate would hold a relief until the disk is full"
        );
        assert!(
            super::super::MANIFEST
                .schedule
                .is_some_and(|s| !s.calendar && s.countdown_seconds == 0),
            "a relief drew a cadence it does not keep, or announced itself"
        );
    }

    #[test]
    fn it_is_wanted_only_when_short_and_rested() {
        let m = DiskSpace::measuring(Vec::new());
        let total = 100 * GIB;
        let short = pulse(vec![Room {
            total,
            free: rules::floor_of(total) - 1,
        }]);
        let roomy = pulse(vec![Room {
            total,
            free: total / 2,
        }]);
        let mine = super::super::MANIFEST.defaults();
        assert_eq!(
            m.wanted_now(10 * HOUR, &short, &mine, None),
            systemd::reachable()
        );
        assert!(!m.wanted_now(10 * HOUR, &roomy, &mine, None));
        assert!(
            !m.wanted_now(10 * HOUR, &pulse(Vec::new()), &mine, None),
            "a relief started on a disk nobody measured"
        );
        // AND AFTER A RELIEF THAT LEFT IT JUST AS SHORT, not again.
        let _ = m.relieved(Relief {
            freed_bytes: 0,
            free_per_mille: short.disk_free_per_mille.unwrap_or(0),
            short: true,
        });
        assert!(!m.wanted_now(10 * HOUR, &short, &mine, None));
    }

    #[test]
    fn the_mark_is_up_only_while_the_disk_is_nearly_full() {
        // THE OWNER'S ASK: a mark that stays on the modeline while the disk is
        // nearly full — and not a moment longer, and not while the relief is
        // simply doing its job between the two lines.
        let m = DiskSpace::measuring(Vec::new());
        let total = 256 * GIB;
        let at = |free| pulse(vec![Room { total, free }]);

        let full = face_of(&m, &at(rules::nearly_full_of(total) - 1));
        assert_eq!(full.marks.len(), 1, "a nearly full disk drew no mark");
        assert_eq!(full.marks[0].id, super::super::mark::FULL);
        assert_eq!(full.marks[0].tone, Tone::Warning);
        assert_eq!(full.marks[0].id.glyph, "disk-space-full");
        assert_eq!(
            full.marks[0].id.hint,
            super::super::key::DISK_SPACE_MARK_FULL
        );
        assert!(full.marks[0].act.is_none(), "a click opens its page");

        for (free, why) in [
            (rules::nearly_full_of(total), "exactly on the line"),
            (
                rules::floor_of(total) - 1,
                "short, which is the relief's to fix",
            ),
            (total / 2, "half free"),
        ] {
            assert!(
                face_of(&m, &at(free)).marks.is_empty(),
                "a mark stayed up with the disk {why}"
            );
        }
        assert!(
            face_of(&m, &pulse(Vec::new())).marks.is_empty(),
            "a mark about a disk nobody measured"
        );
        // AND THE FULLER OF TWO DECIDES: a separate home nearly full beside a
        // roomy store is a nearly full disk.
        let two = pulse(vec![
            Room {
                total,
                free: total / 2,
            },
            Room {
                total: 100 * GIB,
                free: GIB,
            },
        ]);
        assert_eq!(face_of(&m, &two).marks.len(), 1);
    }

    #[test]
    fn a_relief_tells_what_it_freed_and_still_short_only_once() {
        let m = DiskSpace::measuring(Vec::new());
        let short = Relief {
            freed_bytes: 5,
            free_per_mille: 42,
            short: true,
        };
        let roomy = Relief {
            freed_bytes: 12 * GIB,
            free_per_mille: 150,
            short: false,
        };
        // STILL SHORT, the first time: said, with how much is free.
        let first = m.relieved(short).expect("still short was not told");
        assert_eq!(first.tag, super::super::event::STILL_SHORT.tag);
        assert!(
            first
                .args
                .iter()
                .any(|(n, a)| n == "free" && *a == Arg::PerMille(42))
        );
        // AND THE SECOND TIME, not: nothing new to say, and the bell keeps it.
        assert_eq!(m.relieved(short), None, "still short was told twice");
        // ROOM MADE: said, with what was freed.
        let made = m
            .relieved(roomy)
            .expect("a relief that made room was not told");
        assert_eq!(made.tag, super::super::event::RELIEVED.tag);
        assert!(
            made.args
                .iter()
                .any(|(n, a)| n == "freed" && *a == Arg::Bytes(12 * GIB))
        );
        assert!(
            made.args
                .iter()
                .any(|(n, a)| n == "free" && *a == Arg::PerMille(150))
        );
        // AND AFTER ROOM WAS MADE, the next "still short" is news again.
        assert!(m.relieved(short).is_some(), "the next shortage went unsaid");
        // EVERY TAG IT TELLS IS ONE ITS MANIFEST DECLARES — the core refuses
        // any other.
        for tag in [first.tag, made.tag] {
            assert!(
                super::super::MANIFEST.events.iter().any(|e| e.tag == tag),
                "{tag} is not declared"
            );
        }
    }

    #[test]
    fn the_page_says_what_is_free_and_where_the_lines_are() {
        let m = DiskSpace::measuring(Vec::new());
        let total = 256 * GIB;
        let reading = pulse(vec![Room {
            total,
            free: rules::nearly_full_of(total) - 1,
        }]);
        let mine = super::super::MANIFEST.defaults();
        let look = Look {
            now: 100,
            pulse: &reading,
            mine: &mine,
            last: None,
        };
        let screen = m.screen(&look);
        assert_eq!(screen.sections.len(), 1);
        let rows = &screen.sections[0].rows;
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["free", "size", "floor", "nearly-full"]);
        assert_eq!(rows[0].value, Cell::Bytes(rules::nearly_full_of(total) - 1));
        assert_eq!(rows[0].tone, Tone::Warning, "a nearly full disk read calm");
        assert_eq!(rows[1].value, Cell::Bytes(total));
        assert_eq!(rows[2].value, Cell::Bytes(rules::floor_of(total)));
        assert_eq!(rows[3].value, Cell::Bytes(rules::nearly_full_of(total)));
        assert!(
            rows.iter().all(|r| r.act.is_none()),
            "a row that only tells acts"
        );
        // THE PAGE IS ASKED AGAIN WHEN THE READING MOVES, and only then.
        assert_eq!(m.face(&look).screen, 42);

        // NOTHING MEASURED: a sentence, not a page of zeros.
        let none = m.screen(&Look {
            pulse: &pulse(Vec::new()),
            ..look
        });
        assert_eq!(none.sections[0].rows.len(), 1);
        assert_eq!(
            none.sections[0].rows[0].label,
            Cell::Key(super::super::key::DISK_SPACE_ROW_UNMEASURED.into())
        );
    }
}
