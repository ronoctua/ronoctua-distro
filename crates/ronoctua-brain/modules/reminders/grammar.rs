//! "tomorrow 9:00 call mum", "daqui a 2 h", "every monday 8:30", "todo 12/03":
//! what a person types, turned into when a reminder rings and what it says.
//!
//! # The words are the LANGUAGES', not this file's
//!
//! Every word the grammar knows — "tomorrow", "amanhã", "every", "todo", the
//! days, the months, "with a sound" — is a key of this module's own texts
//! (`reminders-grammar-*`, one line per word class, the alternatives separated
//! by commas), turned into a table by `tables.nix`. So a third language is a
//! third file and nothing here, and the Portuguese words live where Portuguese
//! may (invariant 18). This file knows only the SHAPES: a number, a colon, a
//! slash, and which class of word may follow which.
//!
//! Each language reads the answer ON ITS OWN — "a" is "one" in English and
//! "at" in Portuguese, and a merged vocabulary would have to pick one — and
//! the reading whose "when" TAKES THE MOST of what was typed wins. "amanhã
//! 9:00" is a whole "when" in Portuguese and only "9:00" in English, which
//! would leave "amanhã" as the text; the first language that read anything
//! would have taken it. On a tie the language declared first wins.
//!
//! # Where the "when" is
//!
//! At the start ("tomorrow 9:00 call mum"), at the end ("call mum tomorrow
//! 9:00"), or both around the text ("9:00 call mum tomorrow"). What is left is
//! the reminder's text — DATA, exactly as it was typed, never a key
//! (invariant 8). "With a sound" may stand at either edge of the text or
//! inside the "when".
//!
//! # A connector belongs to the text at an edge
//!
//! "tomorrow 9:00 at the office" is at nine, and "at the office" is what it
//! says: a word like "at", "the", "on" or "and" at the edge of the "when" is
//! left to the text, and only a connector BETWEEN two parts of the "when" is
//! read as one — or "at" right before an hour, which is the hour's ("call mum
//! at 10 tomorrow").
//!
//! # A number beside the "when" is not guessed
//!
//! "tomorrow 8 dentist" has an 8 no rule places — an hour? a count? — and
//! "every 2 hours" is not a repetition this grammar has. An answer whose
//! "when" says no hour and leaves a bare number (or a unit right after its
//! own number) beside it is refused as not understood, never rung at nine
//! with the number in its text.

use super::calendar::{
    Date, Repeat, Weekday, at_local, date_of, day_of, days_in, local_day, minute_of, months_later,
    next_after,
};
use crate::moment::{DAY, Moment, Offset};

/// The minute a date with no hour rings at: nine in the morning.
pub const UNSAID_MINUTE: u16 = 9 * 60;

/// The longest text kept, in bytes — what a person types into a question, not
/// a document.
pub const TEXT_MOST: usize = 500;

/// How far ahead "in …" may reach, in seconds: ten years.
const FARTHEST: i64 = 10 * 366 * DAY;

/// Why an answer was not taken — each one of the manifest's `whys`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrong {
    /// No "when" could be read in it.
    NotUnderstood,
    /// It reads as a moment that has already passed.
    Passed,
    /// It names a day the calendar does not have (the 31st of April).
    NoSuchDay,
}

impl Wrong {
    /// The word the manifest declares for it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::NotUnderstood => super::super::why::NOT_UNDERSTOOD,
            Self::Passed => super::super::why::PASSED,
            Self::NoSuchDay => super::super::why::NO_SUCH_DAY,
        }
    }
}

/// When a reminder rings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// How it comes round.
    pub repeat: Repeat,
    /// The local minute of the day it rings at.
    pub minute: u16,
    /// The first instant it rings.
    pub next: Moment,
}

/// What an answer said: when, what, and whether it asked for a sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heard {
    /// When.
    pub plan: Plan,
    /// What it says, as typed; `None` when the answer was only a "when".
    pub text: Option<String>,
    /// `Some(true)` for "with a sound", `Some(false)` for "without", `None`
    /// when it said neither.
    pub sound: Option<bool>,
}

// ---------------------------------------------------------------------------
// The word classes.
// ---------------------------------------------------------------------------

/// A unit of a duration or of a repetition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// What a word (or a run of words) means to the grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    In,
    Unit(Unit),
    One,
    HalfHour,
    Today,
    Tomorrow,
    AfterTomorrow,
    Every,
    Daily,
    Weekly,
    Monthly,
    Yearly,
    At,
    And,
    Filler,
    Am,
    Pm,
    Noon,
    Midnight,
    Ordinal,
    Weekday(Weekday),
    Month(u8),
    SoundOn,
    SoundOff,
}

/// Every class, by the word its key ends in (`reminders-grammar-<word>`).
const CLASSES: [(&str, Class); 46] = [
    ("in", Class::In),
    ("minute", Class::Unit(Unit::Minute)),
    ("hour", Class::Unit(Unit::Hour)),
    ("day", Class::Unit(Unit::Day)),
    ("week", Class::Unit(Unit::Week)),
    ("month", Class::Unit(Unit::Month)),
    ("year", Class::Unit(Unit::Year)),
    ("one", Class::One),
    ("half-hour", Class::HalfHour),
    ("today", Class::Today),
    ("tomorrow", Class::Tomorrow),
    ("after-tomorrow", Class::AfterTomorrow),
    ("every", Class::Every),
    ("daily", Class::Daily),
    ("weekly", Class::Weekly),
    ("monthly", Class::Monthly),
    ("yearly", Class::Yearly),
    ("at", Class::At),
    ("and", Class::And),
    ("filler", Class::Filler),
    ("am", Class::Am),
    ("pm", Class::Pm),
    ("noon", Class::Noon),
    ("midnight", Class::Midnight),
    ("ordinal", Class::Ordinal),
    ("monday", Class::Weekday(Weekday::Monday)),
    ("tuesday", Class::Weekday(Weekday::Tuesday)),
    ("wednesday", Class::Weekday(Weekday::Wednesday)),
    ("thursday", Class::Weekday(Weekday::Thursday)),
    ("friday", Class::Weekday(Weekday::Friday)),
    ("saturday", Class::Weekday(Weekday::Saturday)),
    ("sunday", Class::Weekday(Weekday::Sunday)),
    ("january", Class::Month(1)),
    ("february", Class::Month(2)),
    ("march", Class::Month(3)),
    ("april", Class::Month(4)),
    ("may", Class::Month(5)),
    ("june", Class::Month(6)),
    ("july", Class::Month(7)),
    ("august", Class::Month(8)),
    ("september", Class::Month(9)),
    ("october", Class::Month(10)),
    ("november", Class::Month(11)),
    ("december", Class::Month(12)),
    ("sound-on", Class::SoundOn),
    ("sound-off", Class::SoundOff),
];

/// Every class, in one iterator.
fn classes() -> impl Iterator<Item = (&'static str, Class)> {
    CLASSES.iter().copied()
}

fn class_named(word: &str) -> Option<Class> {
    classes().find(|(w, _)| *w == word).map(|(_, c)| c)
}

// ---------------------------------------------------------------------------
// Tokens.
// ---------------------------------------------------------------------------

/// One piece of what was typed, folded (lower case, no accents).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Word(String),
    Num { value: u32, digits: u8 },
    Mark(char),
}

/// A piece, where it was, and whether it touched the one before it.
#[derive(Debug, Clone)]
struct Token {
    piece: Piece,
    start: usize,
    end: usize,
    glued: bool,
}

/// Lower case, and the accents a Latin alphabet puts on a letter taken off:
/// "Amanhã" and "amanha" are the same word to somebody typing fast.
fn fold(c: char) -> impl Iterator<Item = char> {
    c.to_lowercase().map(|l| match l {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    })
}

/// A word goes on through a hyphen or an apostrophe with a letter after it:
/// "segunda-feira" is one word, "12-03" is not.
fn joins(text: &str, at: usize) -> bool {
    text[at..].chars().nth(1).is_some_and(char::is_alphabetic)
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut last_end = None;
    while let Some((start, c)) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        let glued = last_end == Some(start);
        let mut end = start + c.len_utf8();
        let piece = if c.is_ascii_digit() {
            let mut digits = String::from(c);
            while let Some(&(at, d)) = chars.peek() {
                if !d.is_ascii_digit() {
                    break;
                }
                digits.push(d);
                end = at + d.len_utf8();
                chars.next();
            }
            Piece::Num {
                // A number longer than nine digits is no hour, day or year,
                // and is refused by every rule that reads one.
                value: if digits.len() > 9 {
                    u32::MAX
                } else {
                    digits.parse().unwrap_or(u32::MAX)
                },
                digits: u8::try_from(digits.len()).unwrap_or(u8::MAX),
            }
        } else if c.is_alphabetic() {
            let mut word: String = fold(c).collect();
            while let Some(&(at, d)) = chars.peek() {
                let inside = (d == '-' || d == '\'' || d == '’') && joins(text, at);
                if !d.is_alphabetic() && !inside {
                    break;
                }
                word.extend(fold(d));
                end = at + d.len_utf8();
                chars.next();
            }
            Piece::Word(word)
        } else {
            Piece::Mark(c)
        };
        last_end = Some(end);
        out.push(Token {
            piece,
            start,
            end,
            glued,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// The vocabulary.
// ---------------------------------------------------------------------------

/// One language's words, the longest phrases first.
#[derive(Debug, Clone)]
struct Vocabulary {
    language: &'static str,
    phrases: Vec<(Vec<Piece>, Class)>,
}

/// Every declared language's words. Built once, from the generated table.
#[derive(Debug, Clone)]
pub struct Grammar {
    languages: Vec<Vocabulary>,
}

impl Grammar {
    /// The grammar the build generated from this module's texts.
    #[must_use]
    pub fn compiled() -> Self {
        Self::of(super::super::tables::words::PHRASES)
    }

    /// A grammar out of `(language, class word, phrases)` rows. A row whose
    /// class this file does not know is left out and said in the log — the
    /// module's test refuses one before it can ship.
    fn of(rows: &[(&'static str, &str, &[&str])]) -> Self {
        let mut languages: Vec<Vocabulary> = Vec::new();
        for (language, word, phrases) in rows {
            let Some(class) = class_named(word) else {
                tracing::error!(
                    language,
                    word,
                    "a word class the reminders' grammar does not know"
                );
                continue;
            };
            let at = match languages.iter().position(|v| v.language == *language) {
                Some(at) => at,
                None => {
                    languages.push(Vocabulary {
                        language,
                        phrases: Vec::new(),
                    });
                    languages.len() - 1
                }
            };
            for phrase in *phrases {
                let pieces: Vec<Piece> = tokens(phrase).into_iter().map(|t| t.piece).collect();
                if !pieces.is_empty() {
                    languages[at].phrases.push((pieces, class));
                }
            }
        }
        for v in &mut languages {
            v.phrases.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
        }
        Self { languages }
    }

    /// Reads an answer: when it rings, what it says, and whether it asked for
    /// a sound.
    ///
    /// # Errors
    ///
    /// No language reads a "when" in it, or the one it reads has passed or
    /// names a day that does not exist.
    pub fn understand(&self, answer: &str, now: Moment, zone: &dyn Offset) -> Result<Heard, Wrong> {
        let toks = tokens(answer);
        if toks.is_empty() {
            return Err(Wrong::NotUnderstood);
        }
        let placed: Vec<Vec<Placed>> = self.languages.iter().map(|v| classify(&toks, v)).collect();
        // EVERY LANGUAGE READS, and the "when" that takes the most wins
        // (this file's header); a tie keeps the one found first.
        let mut best: Option<(usize, Reading)> = None;
        for (language, p) in placed.iter().enumerate() {
            for r in readings(p) {
                if best.as_ref().is_none_or(|(_, b)| r.covers > b.covers) {
                    best = Some((language, r));
                }
            }
        }
        let (language, r) = best.ok_or(Wrong::NotUnderstood)?;
        if r.stray {
            return Err(Wrong::NotUnderstood);
        }
        finish(
            &r.spec,
            r.sound,
            answer,
            &placed[language][r.rest],
            r.before,
            now,
            zone,
        )
    }
}

/// One way of reading an answer in one language: the "when", and where the
/// text is.
#[derive(Debug)]
struct Reading {
    spec: Spec,
    sound: Option<bool>,
    /// The text, as a range of the placed items.
    rest: std::ops::Range<usize>,
    /// Whether the text comes before a "when" (at the end, or around it).
    before: bool,
    /// How many of the typed pieces the "when" took.
    covers: usize,
    /// Whether it left a number, or a unit, beside itself that no rule
    /// placed (this file's header).
    stray: bool,
}

/// The readings of one language: the longest start that reads as a "when",
/// the longest end, and — when there are both and they do not touch — the two
/// read as one "when" around the text.
fn readings(p: &[Placed]) -> Vec<Reading> {
    let n = p.len();
    let start = (1..=n).rev().find_map(|j| read(&p[..j]).map(|w| (j, w)));
    let end = (1..n).find_map(|i| read(&p[i..]).map(|w| (i, w)));
    let mut out = Vec::new();
    if let Some((j, (spec, sound))) = &start {
        out.push(reading(p, spec.clone(), *sound, *j..n, false));
    }
    if let Some((i, (spec, sound))) = &end {
        out.push(reading(p, spec.clone(), *sound, 0..*i, true));
    }
    if let (Some((j, _)), Some((i, _))) = (&start, &end)
        && j < i
    {
        let around: Vec<Placed> = p[..*j].iter().chain(&p[*i..]).copied().collect();
        if let Some((spec, sound)) = read(&around) {
            out.push(reading(p, spec, sound, *j..*i, true));
        }
    }
    out
}

fn reading(
    p: &[Placed],
    spec: Spec,
    sound: Option<bool>,
    rest: std::ops::Range<usize>,
    before: bool,
) -> Reading {
    let all: usize = p.iter().map(|x| x.pieces).sum();
    let text: usize = p[rest.clone()].iter().map(|x| x.pieces).sum();
    let stray = strays(&spec, p, &rest);
    Reading {
        spec,
        sound,
        rest,
        before,
        covers: all - text,
        stray,
    }
}

/// Whether a "when" that says no hour has left a bare number beside itself
/// ("tomorrow 8 dentist"), or a unit right after its own last number ("every 2
/// hours").
fn strays(spec: &Spec, p: &[Placed], rest: &std::ops::Range<usize>) -> bool {
    if places_hour(spec) {
        return false;
    }
    let text = &p[rest.clone()];
    // WHAT GOES BETWEEN a "when" and its text (a comma, "at", "with a sound")
    // does not keep a number from being beside it.
    let quiet = |x: &&Placed| {
        is_connector(x.item) || matches!(x.item, Item::Class(Class::SoundOn | Class::SoundOff))
    };
    let loose = |beside: Option<&Placed>, when: Option<&Placed>| match beside.map(|x| x.item) {
        Some(Item::Num { .. }) => true,
        Some(Item::Class(Class::Unit(_))) => {
            matches!(when.map(|x| x.item), Some(Item::Num { .. }))
        }
        _ => false,
    };
    let after_a_when = rest.start > 0
        && loose(
            text.iter().find(|x| !quiet(x)),
            rest.start.checked_sub(1).and_then(|i| p.get(i)),
        );
    let before_a_when =
        rest.end < p.len() && loose(text.iter().rev().find(|x| !quiet(x)), p.get(rest.end));
    after_a_when || before_a_when
}

/// Whether a "when" says the hour it rings at, so a number beside it can be
/// the text's ("every day 7:00 2 pills").
const fn places_hour(spec: &Spec) -> bool {
    match spec {
        Spec::At { .. } => true,
        Spec::After { minutes, at, .. } => *minutes > 0 || at.is_some(),
        Spec::On { at, .. } | Spec::Every { at, .. } => at.is_some(),
    }
}

/// A piece of the answer as the grammar sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Num { value: u32, digits: u8 },
    Mark(char),
    Class(Class),
    Unknown,
}

/// An item, where it was in the answer, how many typed pieces it took, and
/// whether it touched the one before it.
#[derive(Debug, Clone, Copy)]
struct Placed {
    item: Item,
    start: usize,
    end: usize,
    pieces: usize,
    glued: bool,
}

fn classify(toks: &[Token], vocabulary: &Vocabulary) -> Vec<Placed> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let hit = vocabulary.phrases.iter().find(|(p, _)| {
            p.len() <= toks.len() - i && p.iter().zip(&toks[i..]).all(|(a, t)| *a == t.piece)
        });
        let (item, len) = match (hit, &toks[i].piece) {
            (Some((p, class)), _) => (Item::Class(*class), p.len()),
            (None, Piece::Num { value, digits }) => (
                Item::Num {
                    value: *value,
                    digits: *digits,
                },
                1,
            ),
            (None, Piece::Mark(c)) => (Item::Mark(*c), 1),
            (None, Piece::Word(_)) => (Item::Unknown, 1),
        };
        out.push(Placed {
            item,
            start: toks[i].start,
            end: toks[i + len - 1].end,
            pieces: len,
            glued: toks[i].glued,
        });
        i += len;
    }
    out
}

// ---------------------------------------------------------------------------
// Reading a "when".
// ---------------------------------------------------------------------------

/// A day, as said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DayRef {
    Today,
    Tomorrow,
    AfterTomorrow,
    Weekday(Weekday),
    Date {
        day: u32,
        month: u32,
        year: Option<i64>,
    },
}

/// A repetition, as said — a part left unsaid is taken from today.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pattern {
    Day,
    Week(Vec<Weekday>),
    Month(Option<u8>),
    Year(Option<(u8, u8)>),
}

/// A "when", as said.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Spec {
    After {
        minutes: i64,
        days: i64,
        months: i64,
        at: Option<u16>,
    },
    On {
        day: DayRef,
        at: Option<u16>,
    },
    At {
        at: u16,
    },
    Every {
        pattern: Pattern,
        at: Option<u16>,
    },
}

const fn is_connector(item: Item) -> bool {
    matches!(
        item,
        Item::Class(Class::Filler | Class::And | Class::At) | Item::Mark(_)
    )
}

/// A range of the answer, read as a whole "when" — or `None`.
fn read(range: &[Placed]) -> Option<(Spec, Option<bool>)> {
    let mut sound = None;
    let mut kept: Vec<Placed> = Vec::with_capacity(range.len());
    for p in range {
        match p.item {
            Item::Unknown => return None,
            Item::Class(Class::SoundOn) => sound = Some(true),
            Item::Class(Class::SoundOff) => sound = Some(false),
            _ => kept.push(*p),
        }
    }
    // A CONNECTOR AT AN EDGE IS THE TEXT'S (this file's header) — but "at"
    // right before an hour is the hour's.
    let leads = kept.first().is_some_and(|p| {
        !is_connector(p.item)
            || (p.item == Item::Class(Class::At)
                && matches!(
                    kept.get(1).map(|n| n.item),
                    Some(Item::Num { .. } | Item::Class(Class::Noon | Class::Midnight))
                ))
    });
    if !leads || kept.last().is_some_and(|p| is_connector(p.item)) {
        return None;
    }
    // INSIDE, a filler says nothing, and "at" says something only before an
    // hour ("às 8"); before anything else it is an article ("todas as
    // segundas").
    let mut read: Vec<Placed> = Vec::with_capacity(kept.len());
    for (at, p) in kept.iter().enumerate() {
        match p.item {
            Item::Class(Class::Filler) => {}
            Item::Class(Class::At)
                if !matches!(
                    kept.get(at + 1).map(|n| n.item),
                    Some(Item::Num { .. } | Item::Class(Class::Noon | Class::Midnight))
                ) => {}
            _ => read.push(*p),
        }
    }
    let spec = Reader { s: &read, i: 0 }.when()?;
    Some((spec, sound))
}

/// What a range is read into, before the "when" around it is known.
enum Body {
    After {
        minutes: i64,
        days: i64,
        months: i64,
    },
    On(DayRef),
    Every(Pattern),
}

struct Reader<'a> {
    s: &'a [Placed],
    i: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<Item> {
        self.s.get(self.i).map(|p| p.item)
    }

    fn peek_at(&self, ahead: usize) -> Option<Item> {
        self.s.get(self.i + ahead).map(|p| p.item)
    }

    fn glued_at(&self, ahead: usize) -> bool {
        self.s.get(self.i + ahead).is_some_and(|p| p.glued)
    }

    /// Takes one class, if it is next.
    fn take(&mut self, class: Class) -> bool {
        if self.peek() == Some(Item::Class(class)) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn when(&mut self) -> Option<Spec> {
        let lead = self.time();
        let body = self.body();
        let trail = if lead.is_none() { self.time() } else { None };
        if self.i != self.s.len() {
            return None;
        }
        let at = lead.or(trail);
        Some(match body {
            None => Spec::At { at: at? },
            Some(Body::After {
                minutes,
                days,
                months,
            }) => Spec::After {
                minutes,
                days,
                months,
                at,
            },
            Some(Body::On(day)) => Spec::On { day, at },
            Some(Body::Every(pattern)) => Spec::Every { pattern, at },
        })
    }

    fn body(&mut self) -> Option<Body> {
        let save = self.i;
        if self.take(Class::In) {
            if let Some(b) = self.durations() {
                return Some(b);
            }
            // "em 12/03": "in" before a day is "on".
            if let Some(d) = self.day_ref() {
                return Some(Body::On(d));
            }
            self.i = save;
            return None;
        }
        if let Some(p) = self.every() {
            return Some(Body::Every(p));
        }
        self.i = save;
        self.day_ref().map(Body::On)
    }

    /// "2 hours", "1 h 30 min", "2h30", "half an hour", "a day".
    fn durations(&mut self) -> Option<Body> {
        let (mut minutes, mut days, mut months) = (0_i64, 0_i64, 0_i64);
        let mut any = false;
        loop {
            let save = self.i;
            if any {
                self.take(Class::And);
            }
            if self.take(Class::HalfHour) {
                minutes += 30;
                any = true;
                continue;
            }
            let n = match self.peek() {
                Some(Item::Num { value, .. }) => i64::from(value),
                Some(Item::Class(Class::One)) => 1,
                _ => {
                    self.i = save;
                    break;
                }
            };
            let Some(Item::Class(Class::Unit(unit))) = self.peek_at(1) else {
                self.i = save;
                break;
            };
            self.i += 2;
            match unit {
                Unit::Minute => minutes += n,
                Unit::Hour => {
                    minutes += n * 60;
                    // "2h30": a bare number right after the hours is minutes.
                    if let Some(Item::Num { value, digits }) = self.peek()
                        && value < 60
                        && digits <= 2
                        && !matches!(
                            self.peek_at(1),
                            Some(Item::Class(Class::Unit(_)) | Item::Mark(_))
                        )
                    {
                        minutes += i64::from(value);
                        self.i += 1;
                    }
                }
                Unit::Day => days += n,
                Unit::Week => days += 7 * n,
                Unit::Month => months += n,
                Unit::Year => months += 12 * n,
            }
            any = true;
        }
        any.then_some(Body::After {
            minutes,
            days,
            months,
        })
    }

    /// Whether what comes next is an hour ("9:00", "9h", "9 pm", "at 9",
    /// "noon").
    fn time_here(&self) -> bool {
        match self.peek() {
            Some(Item::Class(Class::At | Class::Noon | Class::Midnight)) => true,
            Some(Item::Num { .. }) => matches!(
                self.peek_at(1),
                Some(
                    Item::Mark(':' | '.')
                        | Item::Class(Class::Unit(Unit::Hour) | Class::Am | Class::Pm)
                )
            ),
            _ => false,
        }
    }

    fn every(&mut self) -> Option<Pattern> {
        let save = self.i;
        let found = match self.peek()? {
            Item::Class(Class::Daily) => {
                self.i += 1;
                Some(Pattern::Day)
            }
            Item::Class(Class::Weekly) => {
                self.i += 1;
                Some(Pattern::Week(self.weekdays().unwrap_or_default()))
            }
            Item::Class(Class::Monthly) => {
                self.i += 1;
                self.take(Class::Unit(Unit::Day));
                Some(Pattern::Month(self.day_number()))
            }
            Item::Class(Class::Yearly) => {
                self.i += 1;
                Some(Pattern::Year(self.date_without_year()))
            }
            Item::Class(Class::Every) => {
                self.i += 1;
                self.every_what()
            }
            _ => None,
        };
        if found.is_none() {
            self.i = save;
        }
        found
    }

    /// What comes after "every".
    fn every_what(&mut self) -> Option<Pattern> {
        match self.peek()? {
            Item::Class(Class::Unit(Unit::Day)) => {
                self.i += 1;
                // "todo dia 7:00" is every day; "todo dia 5" the 5th of every
                // month; "todo dia 12/03" a date of every year.
                if self.time_here() {
                    return Some(Pattern::Day);
                }
                if let Some(md) = self.date_without_year() {
                    return Some(Pattern::Year(Some(md)));
                }
                Some(
                    self.day_number()
                        .map_or(Pattern::Day, |d| Pattern::Month(Some(d))),
                )
            }
            Item::Class(Class::Unit(Unit::Week)) => {
                self.i += 1;
                Some(Pattern::Week(self.weekdays().unwrap_or_default()))
            }
            Item::Class(Class::Unit(Unit::Month)) => {
                self.i += 1;
                self.take(Class::Unit(Unit::Day));
                Some(Pattern::Month(self.day_number()))
            }
            Item::Class(Class::Unit(Unit::Year)) => {
                self.i += 1;
                Some(Pattern::Year(self.date_without_year()))
            }
            Item::Class(Class::Weekday(_)) => self.weekdays().map(Pattern::Week),
            Item::Num { .. } => {
                if let Some(md) = self.date_without_year() {
                    return Some(Pattern::Year(Some(md)));
                }
                self.day_number().map(|d| Pattern::Month(Some(d)))
            }
            Item::Class(Class::Month(_)) => {
                self.date_without_year().map(|md| Pattern::Year(Some(md)))
            }
            _ => None,
        }
    }

    /// "monday", "monday and wednesday", "mon, wed, fri".
    fn weekdays(&mut self) -> Option<Vec<Weekday>> {
        let mut days = Vec::new();
        loop {
            let save = self.i;
            if !days.is_empty() && !self.take(Class::And) && self.peek() == Some(Item::Mark(',')) {
                self.i += 1;
            }
            match self.peek() {
                Some(Item::Class(Class::Weekday(w))) => {
                    self.i += 1;
                    if !days.contains(&w) {
                        days.push(w);
                    }
                }
                _ => {
                    self.i = save;
                    break;
                }
            }
        }
        days.sort_unstable();
        (!days.is_empty()).then_some(days)
    }

    /// A day of the month: "5", "5th", "5º" — and never an hour.
    fn day_number(&mut self) -> Option<u8> {
        if self.time_here() {
            return None;
        }
        let Some(Item::Num { value, digits }) = self.peek() else {
            return None;
        };
        if !(1..=31).contains(&value) || digits > 2 {
            return None;
        }
        self.i += 1;
        if self.peek() == Some(Item::Class(Class::Ordinal)) && self.glued_at(0) {
            self.i += 1;
        }
        u8::try_from(value).ok()
    }

    /// A date: "12/03", "12/03/2027", "12 March", "12 de março", "March 12".
    fn date(&mut self) -> Option<(u32, u32, Option<i64>)> {
        let save = self.i;
        let found = self.date_inner();
        if found.is_none() {
            self.i = save;
        }
        found
    }

    fn date_inner(&mut self) -> Option<(u32, u32, Option<i64>)> {
        match self.peek()? {
            Item::Num { value: day, digits } if digits <= 2 => {
                self.i += 1;
                if self.peek() == Some(Item::Mark('/')) {
                    let Some(Item::Num {
                        value: month,
                        digits: 1..=2,
                    }) = self.peek_at(1)
                    else {
                        return None;
                    };
                    self.i += 2;
                    let year = if self.peek() == Some(Item::Mark('/')) {
                        let Some(Item::Num { value, digits }) = self.peek_at(1) else {
                            return None;
                        };
                        self.i += 2;
                        Some(year_of(value, digits)?)
                    } else {
                        None
                    };
                    return valid(day, month, year);
                }
                if self.peek() == Some(Item::Class(Class::Ordinal)) && self.glued_at(0) {
                    self.i += 1;
                }
                let Some(Item::Class(Class::Month(month))) = self.peek() else {
                    return None;
                };
                self.i += 1;
                let year = self.four_digit_year();
                valid(day, u32::from(month), year)
            }
            Item::Class(Class::Month(month)) => {
                self.i += 1;
                let Some(Item::Num { value: day, digits }) = self.peek() else {
                    return None;
                };
                if digits > 2 || self.time_here() {
                    return None;
                }
                self.i += 1;
                if self.peek() == Some(Item::Class(Class::Ordinal)) && self.glued_at(0) {
                    self.i += 1;
                }
                let year = self.four_digit_year();
                valid(day, u32::from(month), year)
            }
            _ => None,
        }
    }

    fn four_digit_year(&mut self) -> Option<i64> {
        match self.peek() {
            Some(Item::Num { value, digits: 4 }) => {
                self.i += 1;
                Some(i64::from(value))
            }
            _ => None,
        }
    }

    /// A date with no year: a birthday.
    fn date_without_year(&mut self) -> Option<(u8, u8)> {
        let save = self.i;
        match self.date() {
            Some((day, month, None)) => Some((u8::try_from(month).ok()?, u8::try_from(day).ok()?)),
            _ => {
                self.i = save;
                None
            }
        }
    }

    fn day_ref(&mut self) -> Option<DayRef> {
        let found = match self.peek()? {
            Item::Class(Class::Today) => DayRef::Today,
            Item::Class(Class::Tomorrow) => DayRef::Tomorrow,
            Item::Class(Class::AfterTomorrow) => DayRef::AfterTomorrow,
            Item::Class(Class::Weekday(w)) => DayRef::Weekday(w),
            _ => {
                let (day, month, year) = self.date()?;
                return Some(DayRef::Date { day, month, year });
            }
        };
        self.i += 1;
        Some(found)
    }

    /// An hour: "9:00", "9.30", "9h", "9h30", "9 am", "at 9", "noon".
    fn time(&mut self) -> Option<u16> {
        let save = self.i;
        let found = self.time_inner();
        if found.is_none() {
            self.i = save;
        }
        found
    }

    fn time_inner(&mut self) -> Option<u16> {
        let at = self.take(Class::At);
        if self.take(Class::Noon) {
            return Some(12 * 60);
        }
        if self.take(Class::Midnight) {
            return Some(0);
        }
        let Some(Item::Num {
            value: hour,
            digits,
        }) = self.peek()
        else {
            return None;
        };
        if digits > 2 {
            return None;
        }
        self.i += 1;
        let minute = match (self.peek(), self.peek_at(1)) {
            (Some(Item::Mark(':' | '.')), Some(Item::Num { value, digits: 2 })) => {
                self.i += 2;
                value
            }
            (Some(Item::Class(Class::Unit(Unit::Hour))), _) => {
                self.i += 1;
                match self.peek() {
                    Some(Item::Num {
                        value,
                        digits: 1..=2,
                    }) if !matches!(
                        self.peek_at(1),
                        Some(Item::Class(Class::Unit(_)) | Item::Mark(':'))
                    ) =>
                    {
                        self.i += 1;
                        value
                    }
                    _ => 0,
                }
            }
            (Some(Item::Class(Class::Am | Class::Pm)), _) => 0,
            // A BARE NUMBER is an hour only after "at": "tomorrow 9" is not
            // read, "tomorrow at 9" is.
            _ if at => 0,
            _ => return None,
        };
        let hour = match self.peek() {
            Some(Item::Class(Class::Am)) => {
                self.i += 1;
                if !(1..=12).contains(&hour) {
                    return None;
                }
                hour % 12
            }
            Some(Item::Class(Class::Pm)) => {
                self.i += 1;
                if !(1..=12).contains(&hour) {
                    return None;
                }
                hour % 12 + 12
            }
            _ => hour,
        };
        (hour < 24 && minute < 60)
            .then(|| u16::try_from(hour * 60 + minute).ok())
            .flatten()
    }
}

/// A year as typed: "27" is 2027, "2027" is 2027.
fn year_of(value: u32, digits: u8) -> Option<i64> {
    match digits {
        2 => Some(2000 + i64::from(value)),
        4 => Some(i64::from(value)),
        _ => None,
    }
}

/// A day and a month that could be a date in some year (the 29th of February
/// is one); the year's own calendar is asked when it is known.
fn valid(day: u32, month: u32, year: Option<i64>) -> Option<(u32, u32, Option<i64>)> {
    ((1..=12).contains(&month) && (1..=31).contains(&day)).then_some((day, month, year))
}

// ---------------------------------------------------------------------------
// From what was said to when it rings.
// ---------------------------------------------------------------------------

/// The text, the sound, and the "when" resolved against now.
fn finish(
    spec: &Spec,
    mut sound: Option<bool>,
    answer: &str,
    rest: &[Placed],
    before: bool,
    now: Moment,
    zone: &dyn Offset,
) -> Result<Heard, Wrong> {
    let plan = resolve(spec, now, zone)?;
    let mut rest = rest;
    // "WITH A SOUND" AT EITHER EDGE OF THE TEXT is not the text.
    while let Some((first, tail)) = rest.split_first() {
        match first.item {
            Item::Class(Class::SoundOn) => sound = Some(true),
            Item::Class(Class::SoundOff) => sound = Some(false),
            _ => break,
        }
        rest = tail;
    }
    while let Some((last, head)) = rest.split_last() {
        match last.item {
            Item::Class(Class::SoundOn) => sound = Some(true),
            Item::Class(Class::SoundOff) => sound = Some(false),
            // AND A CONNECTOR BEFORE A "WHEN" AT THE END is the when's: "call
            // mum on monday" says "call mum".
            item if before && is_connector(item) => {}
            _ => break,
        }
        rest = head;
    }
    let text = match (rest.first(), rest.last()) {
        (Some(first), Some(last)) => tidy(&answer[first.start..last.end]),
        _ => None,
    };
    Ok(Heard { plan, text, sound })
}

/// The text as typed, without the separators around it ("tomorrow 9:00, call
/// mum"), and no longer than [`TEXT_MOST`].
fn tidy(text: &str) -> Option<String> {
    let separators = |c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':' | '-' | '–' | '—');
    let trimmed = text.trim_matches(separators);
    if trimmed.is_empty() {
        return None;
    }
    let mut end = trimmed.len().min(TEXT_MOST);
    while !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    Some(trimmed[..end].to_owned())
}

fn resolve(spec: &Spec, now: Moment, zone: &dyn Offset) -> Result<Plan, Wrong> {
    let today = local_day(now, zone);
    let once = |next: Moment| Plan {
        repeat: Repeat::Once,
        minute: minute_of(next, zone),
        next,
    };
    match spec {
        Spec::After {
            minutes,
            days,
            months,
            at,
        } => {
            let seconds = minutes.saturating_mul(60);
            if *days == 0 && *months == 0 {
                if at.is_some() || seconds <= 0 || seconds > FARTHEST {
                    return Err(Wrong::NotUnderstood);
                }
                return Ok(once(now + seconds));
            }
            if seconds > 0 {
                // "in 1 day and 2 hours" is a length of time, and a month has
                // none.
                if at.is_some() || *months > 0 || days.saturating_mul(DAY) > FARTHEST {
                    return Err(Wrong::NotUnderstood);
                }
                return Ok(once(now + days * DAY + seconds));
            }
            if days.saturating_mul(DAY) + months.saturating_mul(31 * DAY) > FARTHEST {
                return Err(Wrong::NotUnderstood);
            }
            let date = months_later(date_of(today + days), *months);
            let day = day_of(date.year, date.month, date.day).ok_or(Wrong::NoSuchDay)?;
            let next = at_local(day, at.unwrap_or_else(|| minute_of(now, zone)), zone);
            if next <= now {
                return Err(Wrong::Passed);
            }
            Ok(once(next))
        }
        Spec::On { day, at } => {
            let minute = at.unwrap_or(UNSAID_MINUTE);
            let on = |d: i64| at_local(d, minute, zone);
            let next = match *day {
                DayRef::Today => on(today),
                DayRef::Tomorrow => on(today + 1),
                DayRef::AfterTomorrow => on(today + 2),
                DayRef::Weekday(w) => (today..=today + 7)
                    .filter(|d| Weekday::of_day(*d) == w)
                    .map(on)
                    .find(|t| *t > now)
                    .ok_or(Wrong::NotUnderstood)?,
                DayRef::Date {
                    day,
                    month,
                    year: Some(year),
                } => on(day_of(year, month, day).ok_or(Wrong::NoSuchDay)?),
                DayRef::Date {
                    day,
                    month,
                    year: None,
                } => {
                    // A DATE WITH NO YEAR is its next one — this year's, or
                    // next year's once this year's has passed. The 29th of
                    // February waits for a year that has one.
                    if day > days_in(2000, month) {
                        return Err(Wrong::NoSuchDay);
                    }
                    let this = date_of(today).year;
                    (this..=this + 8)
                        .filter_map(|y| day_of(y, month, day))
                        .map(on)
                        .find(|t| *t > now)
                        .ok_or(Wrong::NoSuchDay)?
                }
            };
            if next <= now {
                return Err(Wrong::Passed);
            }
            Ok(Plan {
                repeat: Repeat::Once,
                minute,
                next,
            })
        }
        Spec::At { at } => {
            let first = at_local(today, *at, zone);
            let next = if first > now {
                first
            } else {
                at_local(today + 1, *at, zone)
            };
            Ok(Plan {
                repeat: Repeat::Once,
                minute: *at,
                next,
            })
        }
        Spec::Every { pattern, at } => {
            let minute = at.unwrap_or(UNSAID_MINUTE);
            let date: Date = date_of(today);
            let repeat = match pattern {
                Pattern::Day => Repeat::Day,
                Pattern::Week(days) if days.is_empty() => Repeat::Week {
                    days: vec![Weekday::of_day(today)],
                },
                Pattern::Week(days) => Repeat::Week { days: days.clone() },
                Pattern::Month(day) => Repeat::Month {
                    day: day.unwrap_or_else(|| u8::try_from(date.day).unwrap_or(1)),
                },
                Pattern::Year(md) => {
                    let (month, day) = md.unwrap_or((
                        u8::try_from(date.month).unwrap_or(1),
                        u8::try_from(date.day).unwrap_or(1),
                    ));
                    if u32::from(day) > days_in(2000, u32::from(month)) {
                        return Err(Wrong::NoSuchDay);
                    }
                    Repeat::Year { month, day }
                }
            };
            let next = next_after(&repeat, minute, now, zone).ok_or(Wrong::NotUnderstood)?;
            Ok(Plan {
                repeat,
                minute,
                next,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moment::Fixed;

    /// Wednesday the 30th of September 2026, 10:00, three hours west of UTC
    /// all year round.
    const ZONE: Fixed = Fixed(-3 * 3600);

    fn day(year: i64, month: u32, d: u32) -> i64 {
        day_of(year, month, d).expect("a real date")
    }

    fn now() -> Moment {
        at_local(day(2026, 9, 30), 10 * 60, &ZONE)
    }

    fn heard(answer: &str) -> Heard {
        Grammar::compiled()
            .understand(answer, now(), &ZONE)
            .unwrap_or_else(|w| panic!("`{answer}` was refused: {w:?}"))
    }

    fn refused(answer: &str) -> Wrong {
        match Grammar::compiled().understand(answer, now(), &ZONE) {
            Ok(h) => panic!("`{answer}` was taken: {h:?}"),
            Err(w) => w,
        }
    }

    /// When a plan rings, as a local date and minute.
    fn rings(h: &Heard) -> (Date, u16) {
        (
            date_of(local_day(h.plan.next, &ZONE)),
            minute_of(h.plan.next, &ZONE),
        )
    }

    fn date(year: i64, month: u32, day: u32) -> Date {
        Date { year, month, day }
    }

    #[test]
    fn every_class_has_words_in_every_language_and_no_word_means_two_things() {
        // THE TABLE IS THE TEXTS: a class a language forgot would make that
        // language's answers fail in silence, and a word in two classes would
        // be read as whichever the sort put first.
        let g = Grammar::compiled();
        let languages = languages_of_the_table();
        assert_eq!(
            g.languages.iter().map(|v| v.language).collect::<Vec<_>>(),
            languages,
            "the grammar's languages are not the declared ones"
        );
        for (language, word, _) in super::super::super::tables::words::PHRASES {
            assert!(
                class_named(word).is_some(),
                "{language}: `{word}` is not a class the grammar knows"
            );
        }
        for v in &g.languages {
            for (word, class) in classes() {
                assert!(
                    v.phrases.iter().any(|(_, c)| *c == class),
                    "{}: no words for `{word}`",
                    v.language
                );
            }
            for (at, (p, c)) in v.phrases.iter().enumerate() {
                for (q, d) in &v.phrases[at + 1..] {
                    assert!(
                        p != q || c == d,
                        "{}: {p:?} means both {c:?} and {d:?}",
                        v.language
                    );
                }
            }
        }
    }

    /// The declared languages, as the generated table lists them — read from
    /// the table itself, in the order it was generated.
    fn languages_of_the_table() -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for (language, _, _) in super::super::super::tables::words::PHRASES {
            if !out.contains(language) {
                out.push(language);
            }
        }
        assert!(out.len() >= 2, "fewer than two languages in the grammar");
        out
    }

    #[test]
    fn once_in_a_while_from_now() {
        for (answer, minutes, text) in [
            ("in 2 hours take the pizza out", 120, "take the pizza out"),
            ("in 2h30 call back", 150, "call back"),
            ("in 1 hour and 30 minutes stretch", 90, "stretch"),
            ("in half an hour tea", 30, "tea"),
            ("in an hour", 60, ""),
            ("daqui a 2 h tirar a pizza", 120, "tirar a pizza"),
            ("daqui a meia hora chá", 30, "chá"),
            ("daqui a uma hora", 60, ""),
            ("em 10 minutos, desligar o forno", 10, "desligar o forno"),
            ("daqui 1 h 15 min alongar", 75, "alongar"),
        ] {
            let h = heard(answer);
            assert_eq!(h.plan.repeat, Repeat::Once, "{answer}");
            assert_eq!(h.plan.next, now() + minutes * 60, "{answer}");
            assert_eq!(h.text.as_deref().unwrap_or(""), text, "{answer}");
            assert_eq!(h.sound, None, "{answer}");
        }
    }

    #[test]
    fn a_sound_is_asked_for_anywhere_around_the_when() {
        for (answer, text) in [
            (
                "in 2 hours with a sound take the pizza out",
                "take the pizza out",
            ),
            (
                "with a sound in 2 hours take the pizza out",
                "take the pizza out",
            ),
            (
                "take the pizza out in 2 hours with a sound",
                "take the pizza out",
            ),
            ("daqui a 2 h com som tirar a pizza", "tirar a pizza"),
            ("tirar a pizza daqui a 2 h com som", "tirar a pizza"),
        ] {
            let h = heard(answer);
            assert_eq!(h.sound, Some(true), "{answer}");
            assert_eq!(h.text.as_deref(), Some(text), "{answer}");
            assert_eq!(h.plan.next, now() + 2 * 3600, "{answer}");
        }
        let silent = heard("amanhã 9:00 sem som reunião");
        assert_eq!(silent.sound, Some(false));
    }

    #[test]
    fn a_day_and_an_hour() {
        let tomorrow = date(2026, 10, 1);
        for (answer, on, minute, text) in [
            ("tomorrow 9:00 call mum", tomorrow, 9 * 60, "call mum"),
            ("tomorrow at 9 call mum", tomorrow, 9 * 60, "call mum"),
            ("call mum tomorrow at 9", tomorrow, 9 * 60, "call mum"),
            ("call mum tomorrow 9am", tomorrow, 9 * 60, "call mum"),
            ("tomorrow 7.30pm dinner", tomorrow, 19 * 60 + 30, "dinner"),
            (
                "amanhã 9:00 ligar para a mãe",
                tomorrow,
                9 * 60,
                "ligar para a mãe",
            ),
            ("amanhã às 9h30 dentista", tomorrow, 9 * 60 + 30, "dentista"),
            ("amanha 21h reunião", tomorrow, 21 * 60, "reunião"),
            (
                "ligar para a mãe amanhã às 9",
                tomorrow,
                9 * 60,
                "ligar para a mãe",
            ),
            ("tomorrow", tomorrow, UNSAID_MINUTE, ""),
            (
                "today 18:00 buy bread",
                date(2026, 9, 30),
                18 * 60,
                "buy bread",
            ),
            (
                "hoje 18h comprar pão",
                date(2026, 9, 30),
                18 * 60,
                "comprar pão",
            ),
            (
                "depois de amanhã meio-dia almoço",
                date(2026, 10, 2),
                12 * 60,
                "almoço",
            ),
            (
                "the day after tomorrow at noon lunch",
                date(2026, 10, 2),
                12 * 60,
                "lunch",
            ),
            ("friday 9pm film", date(2026, 10, 2), 21 * 60, "film"),
            ("sexta 21h filme", date(2026, 10, 2), 21 * 60, "filme"),
            // WEDNESDAY IS TODAY, and 08:00 has passed: next Wednesday.
            ("wednesday 8:00 bins", date(2026, 10, 7), 8 * 60, "bins"),
            ("quarta 11:00 lixo", date(2026, 9, 30), 11 * 60, "lixo"),
            ("12/03 10:00 dentist", date(2027, 3, 12), 10 * 60, "dentist"),
            (
                "12 march at 10am dentist",
                date(2027, 3, 12),
                10 * 60,
                "dentist",
            ),
            (
                "12 de março às 10h dentista",
                date(2027, 3, 12),
                10 * 60,
                "dentista",
            ),
            (
                "25/12/2026 08:00 presentes",
                date(2026, 12, 25),
                8 * 60,
                "presentes",
            ),
            (
                "em 12/10 revisão do carro",
                date(2026, 10, 12),
                UNSAID_MINUTE,
                "revisão do carro",
            ),
            ("18:00 buy bread", date(2026, 9, 30), 18 * 60, "buy bread"),
            // 08:00 HAS PASSED: an hour alone is the next one.
            ("8:00 bins", date(2026, 10, 1), 8 * 60, "bins"),
            ("in 3 days at 9 renew", date(2026, 10, 3), 9 * 60, "renew"),
            (
                "tomorrow 9:00 at the office",
                tomorrow,
                9 * 60,
                "at the office",
            ),
        ] {
            let h = heard(answer);
            assert_eq!(h.plan.repeat, Repeat::Once, "{answer}");
            assert_eq!(rings(&h), (on, minute), "{answer}");
            assert_eq!(h.text.as_deref().unwrap_or(""), text, "{answer}");
        }
    }

    #[test]
    fn every_day_week_month_and_year() {
        let week = |days: &[Weekday]| Repeat::Week {
            days: days.to_vec(),
        };
        use Weekday::{Friday, Monday, Wednesday};
        for (answer, repeat, first, minute, text) in [
            (
                "every day 7:00 pills",
                Repeat::Day,
                date(2026, 10, 1),
                7 * 60,
                "pills",
            ),
            (
                "daily at 7am pills",
                Repeat::Day,
                date(2026, 10, 1),
                7 * 60,
                "pills",
            ),
            (
                "todo dia 7:00 remédio",
                Repeat::Day,
                date(2026, 10, 1),
                7 * 60,
                "remédio",
            ),
            (
                "todos os dias às 22h dormir",
                Repeat::Day,
                date(2026, 9, 30),
                22 * 60,
                "dormir",
            ),
            (
                "every monday 8:30 standup",
                week(&[Monday]),
                date(2026, 10, 5),
                8 * 60 + 30,
                "standup",
            ),
            (
                "every mon, wed and fri at 7 gym",
                week(&[Monday, Wednesday, Friday]),
                date(2026, 10, 2),
                7 * 60,
                "gym",
            ),
            (
                "toda segunda e quarta às 8 academia",
                week(&[Monday, Wednesday]),
                date(2026, 10, 5),
                8 * 60,
                "academia",
            ),
            (
                "todas as segundas 8:30 reunião",
                week(&[Monday]),
                date(2026, 10, 5),
                8 * 60 + 30,
                "reunião",
            ),
            (
                "every month on the 5th pay rent",
                Repeat::Month { day: 5 },
                date(2026, 10, 5),
                UNSAID_MINUTE,
                "pay rent",
            ),
            (
                "every 5th at 9am pay rent",
                Repeat::Month { day: 5 },
                date(2026, 10, 5),
                9 * 60,
                "pay rent",
            ),
            (
                "todo dia 5 pagar aluguel",
                Repeat::Month { day: 5 },
                date(2026, 10, 5),
                UNSAID_MINUTE,
                "pagar aluguel",
            ),
            (
                "todo mês dia 10 às 8h boleto",
                Repeat::Month { day: 10 },
                date(2026, 10, 10),
                8 * 60,
                "boleto",
            ),
            (
                "todo 12/03 aniversário da Ana",
                Repeat::Year { month: 3, day: 12 },
                date(2027, 3, 12),
                UNSAID_MINUTE,
                "aniversário da Ana",
            ),
            (
                "every 12/03 Ana's birthday",
                Repeat::Year { month: 3, day: 12 },
                date(2027, 3, 12),
                UNSAID_MINUTE,
                "Ana's birthday",
            ),
            (
                "every year on 2 October at 8:00 anniversary",
                Repeat::Year { month: 10, day: 2 },
                date(2026, 10, 2),
                8 * 60,
                "anniversary",
            ),
            (
                "todo ano 29/02 aniversário",
                Repeat::Year { month: 2, day: 29 },
                date(2027, 2, 28),
                UNSAID_MINUTE,
                "aniversário",
            ),
        ] {
            let h = heard(answer);
            assert_eq!(h.plan.repeat, repeat, "{answer}");
            assert_eq!(rings(&h), (first, minute), "{answer}");
            assert_eq!(h.plan.minute, minute, "{answer}");
            assert_eq!(h.text.as_deref().unwrap_or(""), text, "{answer}");
        }
    }

    #[test]
    fn what_cannot_be_read_is_refused_with_its_reason() {
        for answer in [
            "",
            "banana",
            "call mum",
            "at",
            "in 2",
            "every",
            "todo",
            "25:00 bins",
        ] {
            assert_eq!(refused(answer), Wrong::NotUnderstood, "{answer}");
        }
        assert_eq!(refused("today 8:00 bins"), Wrong::Passed);
        assert_eq!(refused("hoje 9h lixo"), Wrong::Passed);
        assert_eq!(refused("30/09/2026 08:00 bins"), Wrong::Passed);
        assert_eq!(refused("31/04 9:00 nothing"), Wrong::NoSuchDay);
        assert_eq!(refused("29/02/2027 9:00 nothing"), Wrong::NoSuchDay);
        assert_eq!(refused("every 30/02 nothing"), Wrong::NoSuchDay);
        // THE WORD FOR EACH is the manifest's.
        for w in [Wrong::NotUnderstood, Wrong::Passed, Wrong::NoSuchDay] {
            assert!(
                super::super::super::MANIFEST.why_key(w.word()).is_some(),
                "{w:?} has no why"
            );
        }
    }

    #[test]
    fn a_bare_when_is_read_in_its_own_language_and_leaves_no_text() {
        // EVERY LANGUAGE READS, and the one whose "when" takes the most wins:
        // English would read "amanhã 9:00" as "9:00" with "amanhã" for the
        // text, and "daqui a 2 h" as 02:00 with "daqui a".
        let week = |d: Weekday| Repeat::Week { days: vec![d] };
        for (answer, repeat, on, minute) in [
            ("daqui a 2 h", Repeat::Once, date(2026, 9, 30), 12 * 60),
            ("amanhã 9:00", Repeat::Once, date(2026, 10, 1), 9 * 60),
            ("amanha 9h", Repeat::Once, date(2026, 10, 1), 9 * 60),
            ("hoje 11h", Repeat::Once, date(2026, 9, 30), 11 * 60),
            ("sexta 21h", Repeat::Once, date(2026, 10, 2), 21 * 60),
            (
                "todo 12/03",
                Repeat::Year { month: 3, day: 12 },
                date(2027, 3, 12),
                UNSAID_MINUTE,
            ),
            (
                "toda segunda 8:30",
                week(Weekday::Monday),
                date(2026, 10, 5),
                8 * 60 + 30,
            ),
            ("todo dia 7:00", Repeat::Day, date(2026, 10, 1), 7 * 60),
            ("tomorrow 9:00", Repeat::Once, date(2026, 10, 1), 9 * 60),
            (
                "every monday 8:30",
                week(Weekday::Monday),
                date(2026, 10, 5),
                8 * 60 + 30,
            ),
            ("in 2 h", Repeat::Once, date(2026, 9, 30), 12 * 60),
        ] {
            let h = heard(answer);
            assert_eq!(h.plan.repeat, repeat, "{answer}");
            assert_eq!(rings(&h), (on, minute), "{answer}");
            assert_eq!(h.text, None, "{answer}: a word of the when became the text");
        }
    }

    #[test]
    fn at_before_an_hour_is_the_hours_and_a_when_may_stand_around_the_text() {
        let tomorrow = date(2026, 10, 1);
        for (answer, on, minute, text) in [
            ("call mum at 10 tomorrow", tomorrow, 10 * 60, "call mum"),
            // LITERALLY THREE: a 24-hour clock, and no guess at the afternoon.
            ("meeting at 3 tomorrow", tomorrow, 3 * 60, "meeting"),
            (
                "ligar para a mãe às 10 amanhã",
                tomorrow,
                10 * 60,
                "ligar para a mãe",
            ),
            // TODAY'S 09:00 has passed at ten: the next one.
            ("at 9 call mum", tomorrow, 9 * 60, "call mum"),
            (
                "às 9 ligar para a mãe",
                tomorrow,
                9 * 60,
                "ligar para a mãe",
            ),
            ("9:00 call mum tomorrow", tomorrow, 9 * 60, "call mum"),
            (
                "9:00 ligar para a mãe amanhã",
                tomorrow,
                9 * 60,
                "ligar para a mãe",
            ),
            // A NUMBER IS THE TEXT'S when the "when" says its hour.
            ("tomorrow 9:00 2 tickets", tomorrow, 9 * 60, "2 tickets"),
        ] {
            let h = heard(answer);
            assert_eq!(rings(&h), (on, minute), "{answer}");
            assert_eq!(h.text.as_deref(), Some(text), "{answer}");
        }
        let pills = heard("every day 7:00 2 pills");
        assert_eq!(
            (pills.plan.repeat, pills.text.as_deref()),
            (Repeat::Day, Some("2 pills"))
        );
    }

    #[test]
    fn a_number_no_rule_places_is_refused_and_never_rung_at_nine() {
        for answer in [
            "tomorrow 8 dentist",
            "amanhã 8 dentista",
            "amanhã 9",
            "tomorrow 9",
            "dentist tomorrow 8",
            "call mum 10 tomorrow",
            "every 2 hours drink water",
            "todo 2 horas beber água",
            "tomorrow, 8 dentist",
            "in 3 days 9 renew",
        ] {
            assert_eq!(refused(answer), Wrong::NotUnderstood, "{answer}");
        }
    }

    #[test]
    fn the_text_is_kept_as_typed() {
        // CASE, ACCENTS AND PUNCTUATION: what was typed is data (invariant 8),
        // and only the separator between it and the "when" goes.
        let (typed, kept) = ("Amanhã 9:00 — Ligar para a Mãe!", "Ligar para a Mãe!");
        assert_eq!(heard(typed).text.as_deref(), Some(kept));
        let long = format!("tomorrow 9:00 {}", "é".repeat(400));
        let text = heard(&long).text.expect("a text");
        assert!(text.len() <= TEXT_MOST && text.chars().all(|c| c == 'é'));
    }

    #[test]
    fn a_word_is_one_token_through_its_hyphen_and_a_number_is_not() {
        let t = tokens("segunda-feira 12-03 9h30 5th");
        let pieces: Vec<Piece> = t.iter().map(|t| t.piece.clone()).collect();
        assert_eq!(
            pieces,
            vec![
                Piece::Word("segunda-feira".to_owned()),
                Piece::Num {
                    value: 12,
                    digits: 2
                },
                Piece::Mark('-'),
                Piece::Num {
                    value: 3,
                    digits: 2
                },
                Piece::Num {
                    value: 9,
                    digits: 1
                },
                Piece::Word("h".to_owned()),
                Piece::Num {
                    value: 30,
                    digits: 2
                },
                Piece::Num {
                    value: 5,
                    digits: 1
                },
                Piece::Word("th".to_owned()),
            ]
        );
        assert!(t[5].glued && t[6].glued && t[8].glued && !t[4].glued);
    }
}
