//! The machine's parts, watched — a mark on the modeline while any is on its
//! way or needs the person, and a word when one arrives or needs them.
//!
//! Why a watch on the records and not a question to root is in `why.md`,
//! beside this file. What is here is the shape of it.
//!
//! # A watch, and nothing that runs
//!
//! The parts' realizer writes one record per part (`ronoctua_parts::record`),
//! whole, renamed into [`ronoctua_parts::names::RECORDS`]. The watch thread
//! watches that folder, and every change heard is one read of the records:
//! exact, cheap, and nothing runs at rest.
//!
//! # The mark
//!
//! One mark at a time, and only while it says something (invariant 14): a
//! part that needs the person, in the colour of a warning; else a part on its
//! way, in the colour of a watch; each with how many. A click opens the
//! parts' page — the mark names the shell's command (`MarkId::opening`).
//!
//! # The words
//!
//! A part that ARRIVED (its phase became `here`) and one that came to NEED
//! THE PERSON are said once each, with the part's name; the first reading is
//! the baseline and says nothing.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use ronoctua_parts::record::Phase;

use crate::module::{
    Arg, Birth, Cell, Ctx, Event, Face, Heard, Look, Manifest, Mark, Module, Row, Screen, Section,
    Source, Tone, Watcher,
};
use crate::moment::Moment;

/// The shell's command a click on the mark runs: the parts' page.
const OPENS: &str = "distro-parts-open";

/// The entry: a watch on the parts' records.
#[must_use]
pub fn new(_birth: &Birth) -> impl Module + use<> {
    PartsWatch::watching(
        PathBuf::from(ronoctua_parts::names::RECORDS),
        PathBuf::from(ronoctua_parts::names::PLAN),
    )
}

/// The watch, and what it read.
struct PartsWatch {
    /// The records' folder — handed in, so a test watches a folder of its
    /// own.
    records: PathBuf,
    /// The plan, for the parts' names.
    plan: PathBuf,
    /// What the last reading found.
    kept: Mutex<Kept>,
    /// The page's generation: moves whenever the page would say something
    /// else.
    generation: AtomicU64,
}

/// What the last reading found.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Kept {
    /// Each part's phase and name, by id.
    parts: BTreeMap<String, (Phase, String)>,
    /// Whether the first reading was taken: before it, nothing is news.
    read: bool,
    /// Why the folder cannot be watched, in the machine's words.
    lost: Option<String>,
}

impl PartsWatch {
    fn watching(records: PathBuf, plan: PathBuf) -> Self {
        Self {
            records,
            plan,
            kept: Mutex::new(Kept::default()),
            generation: AtomicU64::new(0),
        }
    }

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

    /// The records as they are now: each part's phase and the name the plan
    /// gives it (the id where the plan says none).
    fn read(&self) -> BTreeMap<String, (Phase, String)> {
        let names: BTreeMap<String, String> = ronoctua_parts::plan::read(&self.plan)
            .map(|plan| {
                plan.parts
                    .into_iter()
                    .map(|(id, wanted)| (id, wanted.name))
                    .collect()
            })
            .unwrap_or_default();
        ronoctua_parts::record::read_all(&self.records)
            .into_iter()
            .filter(|r| r.phase != Phase::Unwanted)
            .map(|r| {
                let name = names.get(&r.id).cloned().unwrap_or_else(|| r.id.clone());
                (r.id, (r.phase, name))
            })
            .collect()
    }

    /// Reads again; tells what arrived and what came to need the person
    /// since the last reading, and the brain when anything moved.
    fn reread(&self, ctx: &Ctx) {
        let now = self.read();
        let before = self.kept();
        if before.read {
            for (id, (phase, name)) in &now {
                let was = before.parts.get(id).map(|(p, _)| *p);
                if *phase == Phase::Here && was.is_some_and(|w| w != Phase::Here) {
                    ctx.tell(
                        Event::of(&super::event::ARRIVED).with("name", Arg::Text(name.clone())),
                    );
                }
                if *phase == Phase::NeedsYou && was != Some(Phase::NeedsYou) {
                    ctx.tell(
                        Event::of(&super::event::NEEDS_YOU).with("name", Arg::Text(name.clone())),
                    );
                }
            }
        }
        if self.keep(|k| {
            k.parts = now;
            k.read = true;
        }) {
            ctx.changed();
        }
    }
}

/// How many parts are in `phases`.
fn count(kept: &Kept, phases: &[Phase]) -> u32 {
    u32::try_from(
        kept.parts
            .values()
            .filter(|(p, _)| phases.contains(p))
            .count(),
    )
    .unwrap_or(u32::MAX)
}

/// The key of a phase, as the page says it.
const fn phase_key(phase: Phase) -> &'static str {
    match phase {
        Phase::Here => super::key::PARTS_WATCH_PHASE_HERE,
        Phase::Arriving => super::key::PARTS_WATCH_PHASE_ARRIVING,
        Phase::Waiting => super::key::PARTS_WATCH_PHASE_WAITING,
        Phase::NeedsYou => super::key::PARTS_WATCH_PHASE_NEEDS_YOU,
        Phase::Unwanted => super::key::PARTS_WATCH_PHASE_UNWANTED,
    }
}

/// The page's rows: each part with where it stands — or why the folder is
/// not watched.
fn rows_of(kept: &Kept) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(why) = &kept.lost {
        rows.push(Row {
            id: "lost".to_owned(),
            label: Cell::Key(super::key::PARTS_WATCH_ROW_UNWATCHED.into()),
            value: Cell::Data(why.clone()),
            tone: Tone::Warning,
            ..Row::default()
        });
    }
    for (id, (phase, name)) in &kept.parts {
        rows.push(Row {
            id: format!("part-{id}"),
            label: Cell::Data(name.clone()),
            value: Cell::Key(phase_key(*phase).into()),
            tone: match phase {
                Phase::NeedsYou => Tone::Warning,
                Phase::Arriving | Phase::Waiting => Tone::Watching,
                Phase::Here | Phase::Unwanted => Tone::Quiet,
            },
            ..Row::default()
        });
    }
    rows
}

impl Module for PartsWatch {
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
        // ONE MARK, the loudest, and none at rest (invariant 14).
        let needs = count(&kept, &[Phase::NeedsYou]);
        let arriving = count(&kept, &[Phase::Arriving, Phase::Waiting]);
        if needs > 0 {
            face.marks.push(Mark {
                id: super::mark::NEEDS_YOU.opening(OPENS),
                tone: Tone::Warning,
                count: needs,
                act: None,
            });
        } else if arriving > 0 {
            face.marks.push(Mark {
                id: super::mark::ARRIVING.opening(OPENS),
                tone: Tone::Watching,
                count: arriving,
                act: None,
            });
        }
        face
    }

    fn screen(&self, _look: &Look<'_>) -> Screen {
        Screen {
            sections: vec![Section {
                heading: super::key::PARTS_WATCH_HEADING_PARTS.into(),
                rows: rows_of(&self.kept()),
            }],
        }
    }
}

impl Watcher for PartsWatch {
    fn sources(&self) -> Vec<Source> {
        vec![Source::Path(self.records.clone())]
    }

    /// THE FIRST READING, after the watch is raised: the baseline, which
    /// tells nothing.
    fn started(&self, _now: Moment, ctx: &Ctx) {
        self.reread(ctx);
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        match heard {
            Heard::Path { .. } => self.reread(ctx),
            Heard::Lost { why, .. } => {
                if self.keep(|k| k.lost = Some(why.to_owned())) {
                    ctx.changed();
                }
            }
            Heard::Back { .. } => {
                if self.keep(|k| k.lost = None) {
                    ctx.changed();
                }
                self.reread(ctx);
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

    const OUT: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-x-1";
    const DRV: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-x-1.drv";

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-brain-parts-watch-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn write(dir: &std::path::Path, id: &str, phase: &str) {
        let record = format!(
            r#"{{"format":1,"id":"{id}","wanted":{{"drv":"{DRV}","out":"{OUT}","interface":1}},"current":null,"previous":null,"phase":"{phase}","waiting-for":null,"attempts":0,"last-failure":null,"missing":[]}}"#
        );
        std::fs::write(dir.join(format!("{id}.json")), record).expect("a record");
    }

    fn face_of(m: &PartsWatch) -> Face {
        let mine = TaskPrefs::default();
        let pulse = Pulse::default();
        m.face(&Look {
            now: 0,
            pulse: &pulse,
            mine: &mine,
            last: None,
        })
    }

    /// FAILS IF: the mark shows at rest, does not show while a part is on
    /// its way, hides one that needs the person behind one that is only
    /// arriving, or does not open the parts' page.
    #[test]
    fn a_mark_while_a_part_is_on_its_way_and_the_loudest_wins() {
        let records = scratch("mark");
        let m = PartsWatch::watching(records.clone(), records.join("no-plan.json"));
        let (ctx, _) = Ctx::kept("parts-watch", 0);
        write(&records, "a", "here");
        m.started(0, &ctx);
        assert!(face_of(&m).marks.is_empty(), "a mark with every part here");

        write(&records, "b", "arriving");
        m.heard(
            Heard::Path {
                source: 0,
                path: &records,
            },
            &ctx,
        );
        let face = face_of(&m);
        assert_eq!(
            (face.marks.len(), face.marks[0].tone, face.marks[0].count),
            (1, Tone::Watching, 1)
        );
        assert_eq!(face.marks[0].id.opens.as_deref(), Some(OPENS));

        write(&records, "c", "needs-you");
        m.heard(
            Heard::Path {
                source: 0,
                path: &records,
            },
            &ctx,
        );
        let face = face_of(&m);
        assert_eq!(
            (face.marks.len(), face.marks[0].tone, face.marks[0].count),
            (1, Tone::Warning, 1)
        );
        let _ = std::fs::remove_dir_all(records);
    }

    /// FAILS IF: the first reading is told as news, a part that arrived or
    /// came to need the person is not told, or told twice.
    #[test]
    fn what_arrived_and_what_needs_you_is_told_once() {
        let records = scratch("tell");
        let m = PartsWatch::watching(records.clone(), records.join("no-plan.json"));
        let (ctx, kept) = Ctx::kept("parts-watch", 0);
        write(&records, "a", "arriving");
        write(&records, "b", "here");
        m.started(0, &ctx);
        assert!(kept.events().is_empty(), "the baseline told something");

        write(&records, "a", "here");
        write(&records, "b", "needs-you");
        m.heard(
            Heard::Path {
                source: 0,
                path: &records,
            },
            &ctx,
        );
        m.heard(
            Heard::Path {
                source: 0,
                path: &records,
            },
            &ctx,
        );
        let told: Vec<String> = kept
            .events()
            .into_iter()
            .map(|(_, e)| format!("{e:?}"))
            .collect();
        assert_eq!(told.len(), 2, "{told:?}");
        assert!(told.iter().any(|e| e.contains("arrived")), "{told:?}");
        assert!(told.iter().any(|e| e.contains("needs-you")), "{told:?}");
        let _ = std::fs::remove_dir_all(records);
    }
}
