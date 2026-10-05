//! Alarms and reminders: once, every day, on days of the week, on a day of the
//! month, or once a year — asked for in the person's own words.
//!
//! Why each piece is the way it is lives in `why.md` beside this file; this is
//! the shape of it.
//!
//! # A watch whose clock is armed only for the next one
//!
//! Nothing here polls. The watch thread asks [`Watcher::next_wake`] and sleeps
//! until the earliest reminder (or the next chime of one that is ringing), and
//! no clock at all when there is none. The moment is WALL time: the watch
//! thread caps its sleep and looks at the clock again, so a laptop that slept
//! through a reminder's moment rings it when it wakes, however late. Only what
//! came due before this brain started — the machine was off — is missed.
//!
//! # It rings until somebody answers
//!
//! A reminder that comes due raises the `due` notice, at the alarm's level —
//! first on the brain's line, ahead of any countdown — and it stays there
//! until it is dismissed or put off: from the line, from the brain's notices,
//! or from this module's page, which offers every snooze the manifest
//! declares. A once-only reminder leaves the list when it is answered; each
//! ring is written into the activity log.
//!
//! # What the person typed is data
//!
//! The text of a reminder crosses as [`Arg::Text`] and [`Cell::Data`] — never
//! a key (invariant 8). The words the answer is read with are keys of this
//! module's texts (`grammar.rs`).
//!
//! # Where it is kept
//!
//! `reminders.json` in the module's configuration directory, through
//! `ronoctua-store`: written whole and atomically, read back with `parsed` so
//! a file that will not parse is never written over — the strip and the page
//! say so, nothing changes until the file is fixed, and what was read before
//! it broke still rings (and can be answered) from memory. The file is also WATCHED: an edit
//! by hand is heard and taken, and the module's own writes are recognised and
//! let by.

mod book;
mod calendar;
mod grammar;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use book::{Book, Happened, Reminder};
use calendar::{Repeat, Weekday};
use grammar::Grammar;

use crate::module::{
    Act, Acted, Arg, Birth, Cell, Ctx, Face, Heard, Item, Look, Manifest, Mark, Module, Noted,
    Notice, Row, Screen, Section, Source, Tone, Watcher,
};
use crate::moment::{Moment, Offset};

/// The file, in the module's configuration directory.
const FILE: &str = "reminders.json";

/// How often a ringing reminder with a sound plays its chime again, in
/// seconds…
const CHIME_EVERY: Moment = 60;

/// …and how many times in all: an alarm nobody is near does not go on
/// sounding into an empty room. It goes on RINGING — on the line — until it
/// is answered.
const CHIMES_MOST: u8 = 5;

/// What plays the chime — `ronoctua_audio::chime` on the machine, and a
/// counter in a test, which must not make a sound.
type Chime = Arc<dyn Fn() -> anyhow::Result<()> + Send + Sync>;

/// The entry: the reminders in the module's configuration directory, in the
/// machine's zone, read with the words of every declared language.
#[must_use]
pub fn new(birth: &Birth) -> impl Module + use<> {
    let dir = birth.config_dir(super::ID);
    // THE FOLDER FIRST: the file is watched from its folder, and a folder that
    // does not exist yet cannot be watched — an edit by hand would never be
    // heard.
    if let Some(dir) = &dir
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        tracing::warn!(error = %e, dir = %dir.display(), "could not make the reminders' folder");
    }
    Reminders::at(
        dir,
        Arc::clone(&birth.zone),
        Arc::new(ronoctua_audio::chime),
    )
}

/// The module.
struct Reminders {
    store: ronoctua_store::Store<Book>,
    zone: Arc<dyn Offset>,
    grammar: Grammar,
    chime: Chime,
    state: Mutex<State>,
}

/// What it holds.
#[derive(Debug, Default)]
struct State {
    book: Book,
    /// Whether the file could be read. When it could not, nothing is written
    /// over it and every change is refused — but what was read before it
    /// broke still rings, and is dismissed or put off in memory.
    readable: bool,
    /// Since when this brain has been watching: what came due after it rings
    /// however late (the machine slept), what came due before it may be missed
    /// (the machine was off). Set by `started`, or by the first look.
    running: Option<Moment>,
    /// The page's generation: it moves with every change.
    generation: u64,
    /// Which ringing reminders have chimed, how often and when last — in
    /// memory: a brain that restarts does not start an old alarm's chime
    /// again.
    chimes: BTreeMap<u64, Chimed>,
}

#[derive(Debug, Clone, Copy)]
struct Chimed {
    times: u8,
    last: Moment,
}

fn held(m: &Mutex<State>) -> MutexGuard<'_, State> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a reminder says, as a value of a sentence.
fn text_arg(text: &str) -> Arg {
    if text.is_empty() {
        Arg::Key(super::key::REMINDERS_UNTITLED.into())
    } else {
        Arg::Text(text.to_owned())
    }
}

/// What a reminder says, as a cell.
fn text_cell(text: &str) -> Cell {
    if text.is_empty() {
        Cell::Key(super::key::REMINDERS_UNTITLED.into())
    } else {
        Cell::Data(text.to_owned())
    }
}

fn key(k: &'static str) -> Cell {
    Cell::Key(k.into())
}

/// A row's id: the reminder's, and which of its rows.
fn row_id(id: u64, part: &str) -> String {
    format!("{id}:{part}")
}

/// A row's id, read back.
fn row_of(row: Option<&str>) -> Option<(u64, &str)> {
    let (id, part) = row?.split_once(':')?;
    Some((id.parse().ok()?, part))
}

const fn weekday_key(w: Weekday) -> &'static str {
    match w {
        Weekday::Monday => super::key::REMINDERS_WEEKDAY_MONDAY,
        Weekday::Tuesday => super::key::REMINDERS_WEEKDAY_TUESDAY,
        Weekday::Wednesday => super::key::REMINDERS_WEEKDAY_WEDNESDAY,
        Weekday::Thursday => super::key::REMINDERS_WEEKDAY_THURSDAY,
        Weekday::Friday => super::key::REMINDERS_WEEKDAY_FRIDAY,
        Weekday::Saturday => super::key::REMINDERS_WEEKDAY_SATURDAY,
        Weekday::Sunday => super::key::REMINDERS_WEEKDAY_SUNDAY,
    }
}

const fn repeat_key(r: &Repeat) -> &'static str {
    match r {
        Repeat::Once => super::key::REMINDERS_REPEAT_ONCE,
        Repeat::Day => super::key::REMINDERS_REPEAT_DAY,
        Repeat::Week { .. } => super::key::REMINDERS_REPEAT_WEEK,
        Repeat::Month { .. } => super::key::REMINDERS_REPEAT_MONTH,
        Repeat::Year { .. } => super::key::REMINDERS_REPEAT_YEAR,
    }
}

impl Reminders {
    /// The reminders kept in `dir` (`None`: nowhere, which is what a test
    /// hands in), and what plays the chime.
    fn at(dir: Option<PathBuf>, zone: Arc<dyn Offset>, chime: Chime) -> Self {
        let store = ronoctua_store::Store::<Book>::at(dir, FILE);
        // `parsed` AND NOT `read`: this module writes the whole file back,
        // and a file read as empty because it did not parse would be
        // written over with nothing (AGENTS.md, "Regras de código").
        let (book, readable) = match store.parsed() {
            Some(book) => (book, true),
            None => {
                tracing::error!(
                    path = ?store.path(),
                    "the reminders' file will not parse; nothing is changed until it does"
                );
                (Book::default(), false)
            }
        };
        Self {
            store,
            zone,
            grammar: Grammar::compiled(),
            chime,
            state: Mutex::new(State {
                book,
                readable,
                running: None,
                generation: 1,
                chimes: BTreeMap::new(),
            }),
        }
    }

    /// Writes the book down and moves the page's generation.
    fn keep(&self, st: &mut State) {
        st.generation += 1;
        if !st.readable {
            return;
        }
        if let Err(e) = self.store.write(&st.book) {
            // LOUD: what is lost is a reminder somebody asked for.
            tracing::error!(error = %e, "could not write the reminders down");
        }
    }

    /// What time has done since the last look: reminders that ring, ring
    /// again or were missed, and the chimes that are due. Written down and
    /// said, and the chime played, OUTSIDE the lock.
    fn pass(&self, now: Moment, ctx: &Ctx) {
        let (happened, chime) = {
            let mut st = held(&self.state);
            // A FILE THAT BROKE does not silence what was read before it: that
            // rings from memory, and is only not written down.
            let running = *st.running.get_or_insert(now);
            let happened = st.book.tick(now, running, &*self.zone);
            let mut chime = false;
            for h in &happened {
                if let Happened::Rang {
                    id, sound: true, ..
                } = h
                {
                    st.chimes.insert(
                        *id,
                        Chimed {
                            times: 1,
                            last: now,
                        },
                    );
                    chime = true;
                }
            }
            // AND AGAIN, while it rings with nobody answering.
            let ringing: Vec<u64> = st
                .book
                .ringing()
                .filter(|r| r.sound)
                .map(|r| r.id)
                .collect();
            st.chimes.retain(|id, _| ringing.contains(id));
            for c in st.chimes.values_mut() {
                if c.times < CHIMES_MOST && now >= c.last + CHIME_EVERY {
                    c.times += 1;
                    c.last = now;
                    chime = true;
                }
            }
            if !happened.is_empty() {
                self.keep(&mut st);
            }
            (happened, chime)
        };
        for h in &happened {
            ctx.record(match h {
                Happened::Rang { text, .. } => {
                    Noted::of(super::key::REMINDERS_NOTED_RANG).with("text", text_arg(text))
                }
                Happened::Missed { text, due } => Noted::of(super::key::REMINDERS_NOTED_MISSED)
                    .with("text", text_arg(text))
                    .with("at", Arg::Moment(*due)),
            });
        }
        if chime && let Err(e) = (self.chime)() {
            // THE ALARM STILL RINGS, on the line: a machine with no sound is
            // told about in the log and not by silence on the screen.
            tracing::warn!(error = %e, "the reminder's chime could not play");
        }
        if !happened.is_empty() {
            ctx.changed();
        }
    }

    /// The file changed on disk: taken when somebody else wrote it, let by
    /// when it is this module's own write.
    fn reread(&self, ctx: &Ctx) {
        let read = self.store.parsed();
        {
            let mut st = held(&self.state);
            match read {
                None => {
                    if st.readable {
                        tracing::error!(
                            "the reminders' file stopped parsing; nothing is changed until it does"
                        );
                        st.readable = false;
                        st.generation += 1;
                    } else {
                        return;
                    }
                }
                // ITS OWN WRITE, heard back: nothing to take.
                Some(book) if st.readable && book == st.book => return,
                Some(book) => {
                    st.book = book;
                    st.readable = true;
                    st.generation += 1;
                }
            }
        }
        self.pass(ctx.now(), ctx);
        ctx.changed();
    }

    /// A change the person asked for, done under the lock and written down.
    fn change(&self, what: impl FnOnce(&mut Book) -> Result<(), &'static str>) -> Acted {
        if !held(&self.state).readable {
            return Acted::Refused(super::why::UNREADABLE);
        }
        self.answer(what)
    }

    /// An answer to what is ringing: taken even while the file cannot be read
    /// — an alarm that rings from memory must be able to stop — and written
    /// down only when it can.
    fn answer(&self, what: impl FnOnce(&mut Book) -> Result<(), &'static str>) -> Acted {
        let mut st = held(&self.state);
        match what(&mut st.book) {
            Ok(()) => {
                self.keep(&mut st);
                Acted::Done
            }
            Err(why) => Acted::Refused(why),
        }
    }

    /// `new`: a reminder from an answer.
    fn create(&self, answer: &str, ctx: &Ctx) -> Acted {
        let now = ctx.now();
        let heard = match self.grammar.understand(answer, now, &*self.zone) {
            Ok(h) => h,
            Err(w) => return Acted::Refused(w.word()),
        };
        let next = heard.plan.next;
        let text = heard.text.clone().unwrap_or_default();
        let acted = self.change(|book| {
            book.add(heard.plan, heard.text, heard.sound.unwrap_or(false), now)
                .map(|_| ())
                .map_err(|_| super::why::FULL)
        });
        if acted == Acted::Done {
            ctx.record(
                Noted::of(super::key::REMINDERS_NOTED_SET)
                    .with("text", text_arg(&text))
                    .with("at", Arg::Moment(next)),
            );
        }
        acted
    }

    /// `when`: a new "when" for a reminder. Its text stays unless the answer
    /// says a new one: "amanhã 9:00" moves it, "amanhã 9:00 ligar para o pai"
    /// also renames it.
    fn rewhen(&self, id: u64, answer: &str, ctx: &Ctx) -> Acted {
        let heard = match self.grammar.understand(answer, ctx.now(), &*self.zone) {
            Ok(h) => h,
            Err(w) => return Acted::Refused(w.word()),
        };
        self.change(|book| {
            book.change(id, heard.plan, heard.text, heard.sound)
                .then_some(())
                .ok_or(super::why::GONE)
        })
    }

    /// Every ringing reminder, as the page's rows: its text (which dismisses
    /// it) and the snoozes the notice offers.
    fn ringing_rows(book: &Book) -> Vec<Row> {
        let mut rows = Vec::new();
        for r in book.ringing() {
            rows.push(Row {
                id: row_id(r.id, "dismiss"),
                label: text_cell(&r.text),
                value: Cell::Moment(r.ringing.map_or(r.next, |g| g.due)),
                note: key(super::key::REMINDERS_ROW_DISMISS),
                tone: Tone::Danger,
                act: Some(super::action::DISMISS.id.into()),
                also: None,
            });
            for (n, minutes) in super::notice::DUE.snooze.iter().enumerate() {
                rows.push(Row {
                    id: row_id(r.id, &format!("snooze-{minutes}")),
                    // ONE LABEL for the three, as the weekdays: the same words
                    // on three rows would be three labels saying one thing.
                    label: if n == 0 {
                        key(super::key::REMINDERS_ROW_SNOOZE)
                    } else {
                        Cell::Empty
                    },
                    value: Cell::Count(u64::from(*minutes)),
                    act: Some(super::action::SNOOZE.id.into()),
                    ..Row::default()
                });
            }
        }
        rows
    }

    /// One reminder, as a section of its own: titled by how it repeats, then
    /// what it says and when it next rings (which changes it), the days it
    /// falls on, and the rows that repeat it another way, turn its chime on
    /// or off, and forget it.
    ///
    /// A SECTION EACH, because five rows of one reminder after five of another
    /// read as one list of ten: the title and the air before it are where one
    /// ends and the next begins.
    fn reminder_section(r: &Reminder) -> Section {
        let mut rows = vec![Row {
            id: row_id(r.id, "when"),
            label: text_cell(&r.text),
            value: Cell::Moment(r.coming()),
            act: Some(super::action::WHEN.id.into()),
            ..Row::default()
        }];
        match &r.repeat {
            Repeat::Week { days } => {
                for (n, d) in days.iter().enumerate() {
                    rows.push(Row {
                        id: row_id(r.id, &format!("day-{n}")),
                        label: if n == 0 {
                            key(super::key::REMINDERS_ROW_DAYS)
                        } else {
                            Cell::Empty
                        },
                        value: key(weekday_key(*d)),
                        ..Row::default()
                    });
                }
            }
            Repeat::Month { day } => rows.push(Row {
                id: row_id(r.id, "day"),
                label: key(super::key::REMINDERS_ROW_DAY),
                value: Cell::Count(u64::from(*day)),
                ..Row::default()
            }),
            Repeat::Once | Repeat::Day | Repeat::Year { .. } => {}
        }
        rows.push(Row {
            id: row_id(r.id, "repeat"),
            label: key(super::key::REMINDERS_ROW_REPEAT),
            act: Some(super::action::REPEAT.id.into()),
            ..Row::default()
        });
        rows.push(Row {
            id: row_id(r.id, "sound"),
            label: key(super::key::REMINDERS_ROW_SOUND),
            value: key(if r.sound {
                super::key::REMINDERS_SOUND_ON
            } else {
                super::key::REMINDERS_SOUND_OFF
            }),
            act: Some(super::action::SOUND.id.into()),
            ..Row::default()
        });
        rows.push(Row {
            id: row_id(r.id, "forget"),
            label: key(super::key::REMINDERS_ROW_FORGET),
            act: Some(super::action::FORGET.id.into()),
            ..Row::default()
        });
        Section {
            heading: repeat_key(&r.repeat).into(),
            rows,
        }
    }
}

impl Module for Reminders {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }

    fn face(&self, _look: &Look<'_>) -> Face {
        let st = held(&self.state);
        let mut face = Face {
            screen: st.generation,
            ..Face::default()
        };
        let ringing: Vec<&Reminder> = st.book.ringing().collect();
        if let Some(newest) = ringing.last() {
            let since = newest.ringing.map_or(newest.next, |g| g.since);
            face.notices.push(
                Notice::of(&super::notice::DUE, super::key::REMINDERS_SAYS_DUE, since)
                    .with("text", text_arg(&newest.text))
                    .with("more", Arg::Number(ringing.len() as i64 - 1)),
            );
            let count = u32::try_from(ringing.len()).unwrap_or(u32::MAX);
            face.marks.push(Mark {
                id: super::mark::RINGING,
                tone: Tone::Accent,
                // A COUNT ONLY WHEN THERE IS MORE THAN ONE: a "1" beside the
                // glyph says nothing the glyph does not.
                count: if count > 1 { count } else { 0 },
                act: None,
            });
        }
        if let Some(last) = st.book.missed.last() {
            face.notices.push(
                Notice::of(
                    &super::notice::MISSED,
                    super::key::REMINDERS_SAYS_MISSED,
                    last.found,
                )
                .with("text", text_arg(&last.text))
                .with("at", Arg::Moment(last.due))
                .with("more", Arg::Number(st.book.missed.len() as i64 - 1)),
            );
        }
        // A FILE THAT WILL NOT PARSE is said on the strip, not only on the
        // page: nothing new can be kept, and what it held beyond what was
        // read before it broke does not ring.
        if !st.readable {
            face.notices.push(
                Notice::of(
                    &super::notice::UNREADABLE,
                    super::key::REMINDERS_SAYS_UNREADABLE,
                    st.running.unwrap_or_default(),
                )
                .with(
                    "held",
                    Arg::Number(i64::try_from(st.book.reminders.len()).unwrap_or(i64::MAX)),
                ),
            );
        }
        // THE ONCE-ONLY ONES are one-shot items: they ring, and leave.
        for r in st
            .book
            .reminders
            .iter()
            .filter(|r| r.once() && r.ringing.is_none())
        {
            face.items.push(Item {
                // THE ROW'S OWN ID, so a press runs `forget` on it as the
                // page's row would.
                id: row_id(r.id, "forget"),
                label: text_cell(&r.text),
                at: r.coming(),
                act: Some(super::action::FORGET.id.into()),
            });
        }
        face
    }

    fn screen(&self, _look: &Look<'_>) -> Screen {
        let st = held(&self.state);
        let mut sections = Vec::new();
        // WHAT RINGS FIRST, even from a file that broke: it rings from memory,
        // and its rows are how it stops.
        let ringing = Self::ringing_rows(&st.book);
        if !ringing.is_empty() {
            sections.push(Section {
                heading: super::key::REMINDERS_HEADING_RINGING.into(),
                rows: ringing,
            });
        }
        if !st.readable {
            sections.push(Section {
                heading: super::key::REMINDERS_HEADING_LIST.into(),
                // THE SENTENCE IN THE VALUE, whose column is the wide one: in
                // the label's it would be cut.
                rows: vec![Row {
                    id: "unreadable".to_owned(),
                    label: key(super::key::REMINDERS_ROW_FILE),
                    value: key(super::key::REMINDERS_ROW_UNREADABLE),
                    tone: Tone::Danger,
                    ..Row::default()
                }],
            });
            return Screen { sections };
        }
        if !st.book.missed.is_empty() {
            sections.push(Section {
                heading: super::key::REMINDERS_HEADING_MISSED.into(),
                rows: st
                    .book
                    .missed
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(n, m)| Row {
                        id: format!("missed-{n}"),
                        label: text_cell(&m.text),
                        value: Cell::Moment(m.due),
                        tone: Tone::Warning,
                        ..Row::default()
                    })
                    .collect(),
            });
        }
        let mut coming: Vec<&Reminder> = st
            .book
            .reminders
            .iter()
            .filter(|r| r.ringing.is_none() || !r.once())
            .collect();
        coming.sort_by_key(|r| (r.coming(), r.id));
        if coming.is_empty() {
            sections.push(Section {
                heading: super::key::REMINDERS_HEADING_LIST.into(),
                rows: vec![Row {
                    id: "none".to_owned(),
                    label: key(super::key::REMINDERS_ROW_NONE),
                    ..Row::default()
                }],
            });
        }
        sections.extend(coming.iter().map(|r| Self::reminder_section(r)));
        Screen { sections }
    }

    fn act(&self, act: &Act, ctx: &Ctx) -> Acted {
        let answer = act.answer.as_deref().unwrap_or("");
        if act.action == super::action::NEW.id {
            return self.create(answer, ctx);
        }
        let Some((id, part)) = row_of(act.row.as_deref()) else {
            return Acted::Refused(super::why::GONE);
        };
        let now = ctx.now();
        let zone = &*self.zone;
        let gone = super::why::GONE;
        match act.action.as_str() {
            a if a == super::action::WHEN.id => self.rewhen(id, answer, ctx),
            a if a == super::action::REPEAT.id => {
                self.change(|b| b.cycle_repeat(id, now, zone).then_some(()).ok_or(gone))
            }
            a if a == super::action::SOUND.id => {
                self.change(|b| b.toggle_sound(id).map(|_| ()).ok_or(gone))
            }
            a if a == super::action::FORGET.id => {
                self.change(|b| b.forget(id).then_some(()).ok_or(gone))
            }
            a if a == super::action::DISMISS.id => self.answer(|b| {
                if b.get(id).is_none() {
                    return Err(gone);
                }
                b.dismiss(id).map(|_| ()).ok_or(super::why::NOT_RINGING)
            }),
            a if a == super::action::SNOOZE.id => {
                // ONLY THE SNOOZES THE NOTICE DECLARES: the row's minutes are
                // read back from its id, and an id somebody made up is not a
                // snooze of any length.
                let Some(minutes) = part
                    .strip_prefix("snooze-")
                    .and_then(|m| m.parse::<u16>().ok())
                    .filter(|m| super::notice::DUE.snooze.contains(m))
                else {
                    return Acted::Refused(gone);
                };
                self.answer(|b| {
                    if b.get(id).is_none() {
                        return Err(gone);
                    }
                    b.snooze(id, now + i64::from(minutes) * 60)
                        .then_some(())
                        .ok_or(super::why::NOT_RINGING)
                })
            }
            _ => Acted::Refused("unknown"),
        }
    }

    fn dismissed(&self, notice: &str, _ctx: &Ctx) {
        // ANSWERED IN MEMORY when the file cannot be read (`keep` does not
        // write then): an alarm that rings must be able to stop.
        let mut st = held(&self.state);
        let changed = if notice == super::notice::DUE.id {
            st.book.dismiss_all() > 0
        } else if notice == super::notice::MISSED.id {
            st.book.clear_missed()
        } else {
            false
        };
        if changed {
            self.keep(&mut st);
        }
    }

    fn snoozed(&self, notice: &str, until: Moment, _ctx: &Ctx) {
        let mut st = held(&self.state);
        if notice != super::notice::DUE.id {
            return;
        }
        if st.book.snooze_all(until) > 0 {
            self.keep(&mut st);
        }
    }
}

impl Watcher for Reminders {
    fn sources(&self) -> Vec<Source> {
        self.store.path().map(Source::Path).into_iter().collect()
    }

    fn started(&self, now: Moment, ctx: &Ctx) {
        // WHAT CAME DUE WHILE THE MACHINE WAS OFF, or while this module was
        // switched off: rung when it is only a little late, missed when not.
        // From here on the brain is running, and what comes due rings however
        // late it is seen — a suspend is not the machine being off.
        held(&self.state).running = Some(now);
        self.pass(now, ctx);
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        match heard {
            Heard::Wake => self.pass(ctx.now(), ctx),
            Heard::Path { .. } => self.reread(ctx),
            Heard::Lost { why, .. } => {
                tracing::warn!(
                    why,
                    "the reminders' file cannot be watched; an edit by hand will not be heard"
                );
            }
            Heard::Journal { .. } | Heard::Line { .. } | Heard::Back { .. } => {}
        }
    }

    fn next_wake(&self, _now: Moment) -> Option<Moment> {
        let st = held(&self.state);
        let chime = st
            .book
            .ringing()
            .filter_map(|r| st.chimes.get(&r.id))
            .filter(|c| c.times < CHIMES_MOST)
            .map(|c| c.last + CHIME_EVERY)
            .min();
        match (st.book.next_due(), chime) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::Hand;
    use crate::hub::Core;
    use crate::mind::Shared;
    use crate::module::Level;
    use crate::prefs::{Prefs, Where};
    use crate::pulse::Pulse;
    use crate::registry::Registry;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ZONE: crate::moment::Fixed = crate::moment::Fixed(0);
    const HOUR: Moment = 3600;

    /// Wednesday the 30th of September 2026, 08:00 UTC.
    fn eight() -> Moment {
        calendar::at_local(
            calendar::day_of(2026, 9, 30).expect("a real date"),
            8 * 60,
            &ZONE,
        )
    }

    /// A module that keeps nothing on disk and counts its chimes.
    fn bare() -> (Reminders, Arc<AtomicUsize>) {
        rooted(None)
    }

    fn rooted(dir: Option<PathBuf>) -> (Reminders, Arc<AtomicUsize>) {
        let chimes = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&chimes);
        let m = Reminders::at(
            dir,
            Arc::new(ZONE),
            Arc::new(move || {
                count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }),
        );
        (m, chimes)
    }

    /// A folder of this test's own under the temporary directory.
    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-brain-reminders-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a test folder");
        dir
    }

    fn ask(m: &Reminders, action: &str, row: Option<String>, answer: &str, now: Moment) -> Acted {
        let (ctx, _) = Ctx::kept(super::super::ID, now);
        m.act(
            &Act {
                action: action.to_owned(),
                row,
                answer: Some(answer.to_owned()),
            },
            &ctx,
        )
    }

    fn look(m: &Reminders, now: Moment) -> Face {
        let pulse = Pulse::default();
        let mine = super::super::MANIFEST.defaults();
        m.face(&Look {
            now,
            pulse: &pulse,
            mine: &mine,
            last: None,
        })
    }

    fn wake(m: &Reminders, now: Moment) -> Arc<crate::module::Kept> {
        let (ctx, kept) = Ctx::kept(super::super::ID, now);
        m.heard(Heard::Wake, &ctx);
        kept
    }

    #[test]
    fn a_reminder_asked_for_rings_at_its_moment_and_only_then() {
        let (m, chimes) = bare();
        assert_eq!(
            m.next_wake(eight()),
            None,
            "a clock armed with nothing to ring"
        );
        assert_eq!(
            ask(&m, "new", None, "in 1 hour with a sound tea", eight()),
            Acted::Done
        );
        // THE ONE CLOCK is armed for it, and for nothing else.
        assert_eq!(m.next_wake(eight()), Some(eight() + HOUR));
        let face = look(&m, eight());
        assert!(face.notices.is_empty() && face.marks.is_empty());
        assert_eq!(face.items.len(), 1, "a once-only reminder is not an item");
        assert_eq!(face.items[0].label, Cell::Data("tea".to_owned()));

        assert!(wake(&m, eight() + HOUR - 1).records().is_empty());
        let kept = wake(&m, eight() + HOUR);
        let face = look(&m, eight() + HOUR);
        assert_eq!(face.notices.len(), 1);
        let n = &face.notices[0];
        assert_eq!(n.level, Level::Alarm);
        assert_eq!(n.says, super::super::key::REMINDERS_SAYS_DUE);
        assert!(
            n.args
                .contains(&("text".into(), Arg::Text("tea".to_owned())))
        );
        assert!(n.dismiss);
        assert_eq!(&*n.snooze, &[5, 10, 60]);
        assert_eq!(face.marks.len(), 1);
        assert!(face.items.is_empty(), "a ringing reminder still waits");
        assert_eq!(chimes.load(Ordering::Relaxed), 1, "it asked for a sound");
        assert_eq!(kept.records().len(), 1, "the ring was not written down");
        assert_eq!(kept.changes().len(), 1);
    }

    #[test]
    fn an_alarm_leaves_only_when_answered_through_the_core() {
        let (m, _) = bare();
        let clock = Hand::at(eight());
        let core = Arc::new(Core::new(
            Arc::new(Shared::of(Prefs::default())),
            Registry::of(vec![Box::new(Borrowed(Arc::new(m)))]).expect("the registry"),
            Where::default(),
            Box::new(clock.clone()),
        ));
        core.act(
            super::super::ID,
            "new",
            None,
            Some("tomorrow 9:00 dentist".to_owned()),
        )
        .expect("taken");
        let due = eight() + 25 * HOUR;
        let alarms = |core: &Core| {
            core.shared
                .snapshot()
                .0
                .notices
                .iter()
                .filter(|n| n.what.level == Level::Alarm)
                .count()
        };
        let module = || core.modules().find(super::super::ID).expect("registered");
        let watcher = || module().watcher().expect("a watch");
        let tick = |at: Moment| {
            clock.set(at);
            watcher().heard(Heard::Wake, &core.ctx(super::super::ID, at, false));
            core.publish(at, Pulse::default());
        };
        tick(due);
        assert_eq!(alarms(&core), 1, "the alarm did not ring");
        // IT STAYS: an hour, a day later, nothing takes it away but a person.
        tick(due + HOUR);
        tick(due + 24 * HOUR);
        assert_eq!(alarms(&core), 1, "the alarm left by itself");
        // PUT OFF for ten minutes: gone, and back when they are up.
        core.snooze(super::super::ID, "due", 10).expect("snoozed");
        core.publish(due + 24 * HOUR, Pulse::default());
        assert_eq!(alarms(&core), 0);
        assert_eq!(
            watcher().next_wake(due + 24 * HOUR),
            Some(due + 24 * HOUR + 600)
        );
        tick(due + 24 * HOUR + 600);
        assert_eq!(alarms(&core), 1, "the snooze never ended");
        // AND DISMISSED: gone for good, and a once-only reminder with it.
        core.dismiss(super::super::ID, "due").expect("dismissed");
        core.publish(due + 24 * HOUR + 601, Pulse::default());
        assert_eq!(alarms(&core), 0);
        assert!(
            core.shared.snapshot().0.items.is_empty(),
            "an answered once-only reminder stayed"
        );
        assert_eq!(watcher().next_wake(due + 25 * HOUR), None);
        // EACH RING WAS WRITTEN DOWN, and the reminder being set.
        let says: Vec<String> = core
            .journal()
            .all()
            .iter()
            .filter(|r| r.task == super::super::ID)
            .map(|r| r.says.to_string())
            .collect();
        assert!(
            says.iter()
                .any(|s| s == super::super::key::REMINDERS_NOTED_SET)
        );
        assert_eq!(
            says.iter()
                .filter(|s| *s == super::super::key::REMINDERS_NOTED_RANG)
                .count(),
            2,
            "{says:?}"
        );
    }

    /// The registry keeps modules by value; the test keeps a handle on it.
    struct Borrowed(Arc<Reminders>);

    impl Module for Borrowed {
        fn manifest(&self) -> &'static Manifest {
            self.0.manifest()
        }
        fn watcher(&self) -> Option<&dyn Watcher> {
            self.0.watcher()
        }
        fn face(&self, look: &Look<'_>) -> Face {
            self.0.face(look)
        }
        fn screen(&self, look: &Look<'_>) -> Screen {
            self.0.screen(look)
        }
        fn act(&self, act: &Act, ctx: &Ctx) -> Acted {
            self.0.act(act, ctx)
        }
        fn dismissed(&self, notice: &str, ctx: &Ctx) {
            self.0.dismissed(notice, ctx);
        }
        fn snoozed(&self, notice: &str, until: Moment, ctx: &Ctx) {
            self.0.snoozed(notice, until, ctx);
        }
    }

    #[test]
    fn what_came_due_while_the_machine_was_off_is_missed_when_it_starts() {
        let dir = folder("missed");
        {
            let (m, _) = rooted(Some(dir.clone()));
            assert_eq!(
                ask(
                    &m,
                    "new",
                    None,
                    "tomorrow 9:00 with a sound pay the bill",
                    eight()
                ),
                Acted::Done
            );
            assert_eq!(
                ask(&m, "new", None, "every day 7:00 pills", eight()),
                Acted::Done
            );
        }
        // THE MACHINE WAS OFF until Saturday noon: a new brain, the same file.
        let back = eight() + 3 * 24 * HOUR + 4 * HOUR;
        let (m, chimes) = rooted(Some(dir.clone()));
        let (ctx, kept) = Ctx::kept(super::super::ID, back);
        m.started(back, &ctx);
        let face = look(&m, back);
        assert!(
            face.notices.iter().all(|n| n.level != Level::Alarm),
            "a reminder three days late rang"
        );
        let missed = face
            .notices
            .iter()
            .find(|n| n.id == super::super::notice::MISSED.id)
            .expect("no missed notice");
        assert_eq!(missed.level, Level::Warn);
        assert!(missed.args.contains(&("more".into(), Arg::Number(1))));
        assert_eq!(
            chimes.load(Ordering::Relaxed),
            0,
            "a missed reminder made a sound"
        );
        assert_eq!(kept.records().len(), 2);
        // THE ONCE-ONLY ONE LEFT, the daily one waits for tomorrow's 07:00.
        let st = held(&m.state);
        assert_eq!(st.book.reminders.len(), 1);
        drop(st);
        let (ctx, _) = Ctx::kept(super::super::ID, back);
        m.dismissed(super::super::notice::MISSED.id, &ctx);
        assert!(look(&m, back).notices.is_empty());
        // AND ALL OF IT WAS WRITTEN DOWN: a third brain reads the same.
        let (again, _) = rooted(Some(dir.clone()));
        assert!(held(&again.state).book.missed.is_empty());
        assert_eq!(held(&again.state).book.reminders.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_ringing_alarm_chimes_again_a_few_times_and_then_only_rings() {
        let (m, chimes) = bare();
        assert_eq!(
            ask(&m, "new", None, "in 10 min with a sound oven", eight()),
            Acted::Done
        );
        let due = eight() + 600;
        wake(&m, due);
        let mut at = due;
        while let Some(next) = m.next_wake(at) {
            assert!(next > at, "a wake that does not move on");
            at = next;
            wake(&m, at);
        }
        assert_eq!(chimes.load(Ordering::Relaxed), usize::from(CHIMES_MOST));
        assert_eq!(at, due + CHIME_EVERY * Moment::from(CHIMES_MOST - 1));
        assert_eq!(
            look(&m, at + HOUR).notices.len(),
            1,
            "it stopped ringing with the chime"
        );
    }

    #[test]
    fn the_page_lists_what_rings_and_what_is_coming_and_its_rows_act() {
        let (m, _) = bare();
        assert_eq!(
            ask(&m, "new", None, "every monday 8:30 standup", eight()),
            Acted::Done
        );
        assert_eq!(
            ask(&m, "new", None, "in 5 min kettle", eight()),
            Acted::Done
        );
        wake(&m, eight() + 300);
        let screen = m.screen(&Look {
            now: eight(),
            pulse: &Pulse::default(),
            mine: &super::super::MANIFEST.defaults(),
            last: None,
        });
        let headings: Vec<&str> = screen.sections.iter().map(|s| &*s.heading).collect();
        // WHAT RINGS FIRST, then each reminder under how it repeats — the
        // ringing kettle is not listed twice.
        assert_eq!(
            headings,
            [
                super::super::key::REMINDERS_HEADING_RINGING,
                super::super::key::REMINDERS_REPEAT_WEEK
            ]
        );
        let ringing = &screen.sections[0].rows;
        assert_eq!(ringing[0].label, Cell::Data("kettle".to_owned()));
        let snoozes: Vec<&Cell> = ringing[1..].iter().map(|r| &r.value).collect();
        assert_eq!(
            snoozes,
            [&Cell::Count(5), &Cell::Count(10), &Cell::Count(60)]
        );
        // ONE LABEL FOR THE THREE: the same words thrice would be three
        // labels of one view saying one thing.
        let labels: Vec<&Cell> = ringing[1..].iter().map(|r| &r.label).collect();
        assert_eq!(
            labels,
            [
                &key(super::super::key::REMINDERS_ROW_SNOOZE),
                &Cell::Empty,
                &Cell::Empty
            ]
        );
        // PUT OFF FOR AN HOUR from its row: a length the strip does not offer.
        let hour_row = ringing[3].id.clone();
        assert_eq!(
            ask(&m, "snooze", Some(hour_row), "", eight() + 300),
            Acted::Done
        );
        assert_eq!(m.next_wake(eight() + 300), Some(eight() + 300 + HOUR));
        // AN ID NOBODY DREW is no snooze at all.
        assert_eq!(
            ask(&m, "snooze", Some(row_id(2, "snooze-7")), "", eight()),
            Acted::Refused(super::super::why::GONE)
        );
        // THE WEEKLY ONE: its rows say Monday, and they change it.
        let list: Vec<Row> = m
            .screen(&Look {
                now: eight(),
                pulse: &Pulse::default(),
                mine: &super::super::MANIFEST.defaults(),
                last: None,
            })
            .sections
            .into_iter()
            .flat_map(|s| s.rows)
            .collect();
        let standup = list
            .iter()
            .find(|r| r.label == Cell::Data("standup".to_owned()))
            .expect("the weekly one is listed");
        let id = row_of(Some(&standup.id)).map(|(id, _)| id).expect("an id");
        assert!(
            list.iter()
                .any(|r| r.value == key(super::super::key::REMINDERS_WEEKDAY_MONDAY))
        );
        assert_eq!(
            ask(&m, "sound", Some(row_id(id, "sound")), "", eight()),
            Acted::Done
        );
        assert!(held(&m.state).book.get(id).is_some_and(|r| r.sound));
        assert_eq!(
            ask(&m, "repeat", Some(row_id(id, "repeat")), "", eight()),
            Acted::Done
        );
        assert!(matches!(
            held(&m.state).book.get(id).map(|r| r.repeat.clone()),
            Some(Repeat::Month { day: 5 })
        ));
        assert_eq!(
            ask(
                &m,
                "when",
                Some(row_id(id, "when")),
                "every friday 17:00",
                eight()
            ),
            Acted::Done
        );
        let r = held(&m.state).book.get(id).cloned().expect("kept");
        assert_eq!(r.text, "standup", "a new when took the text away");
        assert_eq!(
            r.repeat,
            Repeat::Week {
                days: vec![Weekday::Friday]
            }
        );
        // THE QUESTION'S OWN EXAMPLES, in Portuguese: a "when" and nothing
        // else, so the text stays and the rule is the one said.
        for (answer, repeat) in [
            (
                "toda segunda 8:30",
                Repeat::Week {
                    days: vec![Weekday::Monday],
                },
            ),
            ("amanhã 9:00", Repeat::Once),
            (
                "every monday 8:30",
                Repeat::Week {
                    days: vec![Weekday::Monday],
                },
            ),
        ] {
            assert_eq!(
                ask(&m, "when", Some(row_id(id, "when")), answer, eight()),
                Acted::Done,
                "{answer}"
            );
            let r = held(&m.state).book.get(id).cloned().expect("kept");
            assert_eq!(r.text, "standup", "{answer} renamed the reminder");
            assert_eq!(r.repeat, repeat, "{answer}");
        }
        // AND A NEW TEXT WITH THE WHEN renames it.
        assert_eq!(
            ask(
                &m,
                "when",
                Some(row_id(id, "when")),
                "every monday 8:30 team standup",
                eight()
            ),
            Acted::Done
        );
        assert_eq!(
            held(&m.state).book.get(id).map(|r| r.text.clone()),
            Some("team standup".to_owned())
        );
        assert_eq!(
            ask(&m, "when", Some(row_id(id, "when")), "someday", eight()),
            Acted::Refused(super::super::why::NOT_UNDERSTOOD)
        );
        assert_eq!(
            ask(&m, "forget", Some(row_id(id, "forget")), "", eight()),
            Acted::Done
        );
        assert_eq!(
            ask(&m, "forget", Some(row_id(id, "forget")), "", eight()),
            Acted::Refused(super::super::why::GONE)
        );
        assert_eq!(
            ask(&m, "dismiss", Some(row_id(99, "dismiss")), "", eight()),
            Acted::Refused(super::super::why::GONE)
        );
    }

    #[test]
    fn a_once_only_item_is_pressed_with_its_own_id_and_act() {
        let (m, _) = bare();
        assert_eq!(
            ask(&m, "new", None, "tomorrow 9:00 dentist", eight()),
            Acted::Done
        );
        let item = look(&m, eight()).items.remove(0);
        let act = item.act.as_deref().expect("an item that acts").to_owned();
        assert_eq!(ask(&m, &act, Some(item.id), "", eight()), Acted::Done);
        assert!(
            held(&m.state).book.reminders.is_empty(),
            "the press did nothing"
        );
    }

    #[test]
    fn what_comes_due_while_the_machine_sleeps_rings_when_it_wakes() {
        // THE BRAIN STARTED AT EIGHT, the lid closed, and it opens two days
        // later: an alarm, not a missed one.
        let (m, chimes) = bare();
        let (ctx, _) = Ctx::kept(super::super::ID, eight());
        m.started(eight(), &ctx);
        assert_eq!(
            ask(&m, "new", None, "in 1 hour with a sound tea", eight()),
            Acted::Done
        );
        let kept = wake(&m, eight() + 48 * HOUR);
        let face = look(&m, eight() + 48 * HOUR);
        assert_eq!(
            face.notices.iter().map(|n| n.level).collect::<Vec<_>>(),
            [Level::Alarm],
            "a reminder the machine slept through did not ring"
        );
        assert_eq!(chimes.load(Ordering::Relaxed), 1);
        assert_eq!(kept.records().len(), 1);
    }

    #[test]
    fn a_file_that_breaks_is_said_on_the_strip_and_what_was_read_still_rings() {
        let dir = folder("broke");
        let (m, _) = rooted(Some(dir.clone()));
        let (ctx, _) = Ctx::kept(super::super::ID, eight());
        m.started(eight(), &ctx);
        assert_eq!(
            ask(&m, "new", None, "in 5 min kettle", eight()),
            Acted::Done
        );
        std::fs::write(dir.join(FILE), b"{ broken").expect("break");
        let (ctx, _) = Ctx::kept(super::super::ID, eight());
        m.heard(
            Heard::Path {
                source: 0,
                path: &dir.join(FILE),
            },
            &ctx,
        );
        // SAID ON THE STRIP, with how many still ring from memory.
        let face = look(&m, eight());
        let broken = face
            .notices
            .iter()
            .find(|n| n.id == super::super::notice::UNREADABLE.id)
            .expect("a broken file said nowhere but the page");
        assert_eq!(broken.says, super::super::key::REMINDERS_SAYS_UNREADABLE);
        assert!(broken.args.contains(&("held".into(), Arg::Number(1))));
        assert!(!broken.dismiss, "a broken file dismissed is still broken");
        // THE KETTLE STILL RINGS, and stops when answered — in memory.
        wake(&m, eight() + 300);
        assert!(
            look(&m, eight() + 300)
                .notices
                .iter()
                .any(|n| n.level == Level::Alarm),
            "a broken file silenced the alarm"
        );
        let row = m
            .screen(&Look {
                now: eight() + 300,
                pulse: &Pulse::default(),
                mine: &super::super::MANIFEST.defaults(),
                last: None,
            })
            .sections[0]
            .rows[0]
            .id
            .clone();
        assert_eq!(
            ask(&m, "dismiss", Some(row), "", eight() + 300),
            Acted::Done
        );
        assert!(
            look(&m, eight() + 300)
                .notices
                .iter()
                .all(|n| n.level != Level::Alarm),
            "the alarm did not stop"
        );
        assert_eq!(
            std::fs::read(dir.join(FILE)).expect("still there"),
            b"{ broken",
            "the broken file was written over"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn every_refusal_is_a_declared_why() {
        let (m, _) = bare();
        for (answer, why) in [
            ("banana", super::super::why::NOT_UNDERSTOOD),
            ("today 7:00 late", super::super::why::PASSED),
            ("31/04 9:00 no such day", super::super::why::NO_SUCH_DAY),
        ] {
            assert_eq!(
                ask(&m, "new", None, answer, eight()),
                Acted::Refused(why),
                "{answer}"
            );
            assert!(super::super::MANIFEST.why_key(why).is_some());
        }
        for why in [
            super::super::why::GONE,
            super::super::why::NOT_RINGING,
            super::super::why::UNREADABLE,
            super::super::why::FULL,
        ] {
            assert!(super::super::MANIFEST.why_key(why).is_some(), "{why}");
        }
    }

    #[test]
    fn a_file_that_will_not_parse_is_never_written_over() {
        let dir = folder("unreadable");
        std::fs::write(dir.join(FILE), b"{ not json").expect("plant");
        let (m, _) = rooted(Some(dir.clone()));
        assert_eq!(
            ask(&m, "new", None, "tomorrow 9:00 anything", eight()),
            Acted::Refused(super::super::why::UNREADABLE)
        );
        assert_eq!(
            std::fs::read(dir.join(FILE)).expect("still there"),
            b"{ not json",
            "the broken file was written over"
        );
        let screen = m.screen(&Look {
            now: eight(),
            pulse: &Pulse::default(),
            mine: &super::super::MANIFEST.defaults(),
            last: None,
        });
        assert_eq!(
            screen.sections[0].rows[0].value,
            key(super::super::key::REMINDERS_ROW_UNREADABLE)
        );
        // FIXED BY HAND: heard, taken, and the module writes again.
        std::fs::write(dir.join(FILE), br#"{"reminders":[]}"#).expect("fix");
        let (ctx, kept) = Ctx::kept(super::super::ID, eight());
        m.heard(
            Heard::Path {
                source: 0,
                path: &dir.join(FILE),
            },
            &ctx,
        );
        assert!(!kept.changes().is_empty());
        assert_eq!(
            ask(&m, "new", None, "tomorrow 9:00 anything", eight()),
            Acted::Done
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_edit_by_hand_is_heard_and_its_own_write_is_let_by() {
        let dir = folder("edited");
        let (m, _) = rooted(Some(dir.clone()));
        assert_eq!(m.sources(), vec![Source::Path(dir.join(FILE))]);
        assert_eq!(
            ask(&m, "new", None, "tomorrow 9:00 one", eight()),
            Acted::Done
        );
        let generation = held(&m.state).generation;
        let heard = |m: &Reminders| {
            let (ctx, kept) = Ctx::kept(super::super::ID, eight());
            m.heard(
                Heard::Path {
                    source: 0,
                    path: &dir.join(FILE),
                },
                &ctx,
            );
            kept
        };
        // ITS OWN WRITE, heard back: nothing moves.
        assert!(heard(&m).changes().is_empty());
        assert_eq!(held(&m.state).generation, generation);
        // A REMINDER PLANTED BY HAND, already due: taken, and it rings.
        let planted = format!(
            r#"{{"reminders":[{{"id":40,"text":"planted","repeat":{{"every":"once"}},"minute":480,"next":{}}}]}}"#,
            eight()
        );
        std::fs::write(dir.join(FILE), planted).expect("plant");
        let kept = heard(&m);
        assert!(!kept.changes().is_empty());
        assert_eq!(
            held(&m.state).book.ringing().count(),
            1,
            "the planted reminder did not ring"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
