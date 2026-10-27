//! The journal's errors, grouped by kind, and a mark until they are seen.
//!
//! The whole story is in `why.md` beside this file; what is here is its shape.
//!
//! # A watch on the journal the core already follows
//!
//! One match ([`Watcher::sources`]): every entry at `err` or worse, of the
//! system's journal and the person's (the core's follower reads both). On the
//! FIRST start — no cursor yet — the core's one pass over the past hands it
//! this boot's last [`rules::BACKFILL_MOST`]; on every later start the cursor
//! decides, and what happened while the brain was off is heard then.
//!
//! # The book
//!
//! [`rules::Book`]: the kinds in the list, each with its count, its last
//! message and moment, its boot and how many times since it was seen; and the
//! kinds ignored. The list is state (`errors.json`, under its state
//! directory, with how often each ignored kind was heard since); what is
//! ignored is the person's choice (`ignored.json`, under its config
//! directory, through `ronoctua-store`). A burst of errors is one write of the
//! list, a few seconds later ([`Watcher::next_wake`] arms a clock only while
//! something waits to be written); a person's act is written at once.
//!
//! # The person's file is written only when they act
//!
//! And read with `parsed` first: a file that does not parse is left as it is
//! — the kinds in it are not heard as ignored meanwhile, the log says why,
//! and ignoring or hearing a kind again is refused with a sentence, never
//! written over the file. What is written is the file as it is NOW with the
//! one kind added or taken away: a kind the person put in or took out by
//! hand stays as they left it.
//!
//! # The mark
//!
//! While a kind is not seen yet, one mark on the modeline in the colour of a
//! watch, with how many kinds — and none at rest (invariant 14). It stays
//! until everything is marked seen.

mod rules;

#[cfg(test)]
mod fixtures;

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use ronoctua_store::Store;

use crate::module::{
    Act, Acted, Backfill, Birth, Cell, Ctx, Face, Heard, Look, Manifest, Mark, Match, Module, Row,
    Screen, Section, Source, Tone, Watcher, Word,
};
use crate::moment::Moment;

use rules::{Book, Filter, Group, Ignored, Kind, Origin};

/// The list, under its state directory.
const LIST_FILE: &str = "errors.json";

/// What is ignored, under its config directory.
const IGNORED_FILE: &str = "ignored.json";

/// What is filtered, under its config directory.
const FILTERS_FILE: &str = "filters.json";

/// How long a change of the list waits before it is written, in seconds — so
/// a burst of a thousand lines is one write.
const WRITE_AFTER_S: Moment = 5;

/// The entry: its book, read from where it was left, and this boot's id.
#[must_use]
pub fn new(birth: &Birth) -> impl Module + use<> {
    let boot = ronoctua_memory::machine::boot_id(&ronoctua_memory::Places::default())
        .map(|b| rules::journal_boot(&b))
        .unwrap_or_default();
    JournalErrors::keeping(
        birth.state_dir(super::ID),
        birth.config_dir(super::ID),
        boot,
    )
}

/// The watch, and its book.
struct JournalErrors {
    /// Where the list is kept; `None` keeps it nowhere (a test).
    state_dir: Option<PathBuf>,
    /// What is ignored: the person's file.
    ignored: Store<IgnoredFile>,
    /// What is filtered: the person's file (their trechos and which shipped
    /// ones they turned off).
    filters: Store<FiltersFile>,
    /// This boot, in the journal's spelling; empty when it could not be read,
    /// and then every kind is this boot's.
    boot: String,
    state: Mutex<State>,
}

/// What it holds.
#[derive(Debug, Default)]
struct State {
    book: Book,
    /// Why the journal is not followed, while it is not: a core word
    /// (`brain-journal-why-<word>`).
    lost: Option<String>,
    /// Its page's generation.
    generation: u64,
    /// Since when the list has changed without being written; `None` when it
    /// is written.
    unwritten: Option<Moment>,
}

/// The list as it is kept — state, the brain's own.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct ListFile {
    groups: Vec<Group>,
    /// How many times each ignored kind was heard since it was ignored.
    ignored_since: Vec<Since>,
    /// How many lines each filter has caught, and the last one, keyed by
    /// trecho — STATE, kept with the list, never in the person's file.
    filtered_since: Vec<Caught>,
}

/// How many times an ignored kind was heard since.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Since {
    kind: Kind,
    since: u64,
}

/// How many lines a filter has caught, and the last one.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Caught {
    text: String,
    since: u64,
    message: String,
}

/// What is ignored as it is kept — the person's choice, and nothing that
/// moves on its own.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct IgnoredFile {
    ignored: Vec<Ignored>,
}

/// What is filtered as it is kept — the person's choice, and nothing that
/// moves on its own. A shipped trecho turned off is here with `off`; one they
/// typed is here; a shipped trecho left as it is is NOT here, and on by being
/// shipped.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct FiltersFile {
    filters: Vec<Choice>,
}

/// One trecho in the person's file.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Choice {
    /// The trecho.
    text: String,
    /// Off: a shipped trecho the person turned off (never written for one of
    /// their own, which simply leaves the file).
    off: bool,
}

impl FiltersFile {
    /// Its trechos as `(text, off)` pairs, for the book to rebuild from.
    fn choices(&self) -> Vec<(String, bool)> {
        self.filters
            .iter()
            .map(|c| (c.text.clone(), c.off))
            .collect()
    }

    /// The file for these filters: the shipped ones turned off, and the
    /// person's own — never a shipped trecho left as it is.
    fn of(filters: &[Filter]) -> Self {
        Self {
            filters: filters
                .iter()
                .filter_map(|f| match f.origin {
                    Origin::Default => f.off.then(|| Choice {
                        text: f.text.clone(),
                        off: true,
                    }),
                    Origin::Custom => Some(Choice {
                        text: f.text.clone(),
                        off: f.off,
                    }),
                })
                .collect(),
        }
    }
}

/// The brain's own state file, or its default when there is none or it does
/// not parse — a list lost is a list that starts again, never a brain that
/// cannot start. NEVER the person's file: that one is a [`Store`], read with
/// `parsed` by whoever writes it.
fn read<T: Default + serde::de::DeserializeOwned>(dir: Option<&std::path::Path>, file: &str) -> T {
    dir.and_then(|d| std::fs::read_to_string(d.join(file)).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Writes one file, atomically; said in the log when it cannot.
fn write<T: Serialize>(dir: Option<&std::path::Path>, file: &str, value: &T) {
    let Some(dir) = dir else { return };
    let written = serde_json::to_string(value)
        .map_err(std::io::Error::other)
        .and_then(|text| ronoctua_xdg::write_atomically(dir, file, text.as_bytes()));
    if let Err(e) = written {
        tracing::warn!(error = %e, file, "the journal's errors could not be kept");
    }
}

impl JournalErrors {
    /// One keeping its list and its choices in these two directories.
    fn keeping(state_dir: Option<PathBuf>, config_dir: Option<PathBuf>, boot: String) -> Self {
        let list: ListFile = read(state_dir.as_deref(), LIST_FILE);
        let ignored_store = Store::at(config_dir.clone(), IGNORED_FILE);
        let filters_store = Store::at(config_dir, FILTERS_FILE);
        // A FILE THAT DOES NOT PARSE is read as nothing — `read` says so in the
        // log, loud — and is never written over (see `act`).
        let ignored: IgnoredFile = ignored_store
            .parsed()
            .unwrap_or_else(|| ignored_store.read());
        let filters: FiltersFile = filters_store
            .parsed()
            .unwrap_or_else(|| filters_store.read());
        let mut book = Book {
            groups: list
                .groups
                .into_iter()
                .map(|g| (g.kind.clone(), g))
                .collect(),
            ignored: ignored
                .ignored
                .into_iter()
                .map(|i| (i.kind.clone(), i))
                .collect(),
            filters: Vec::new(),
        };
        for Since { kind, since } in list.ignored_since {
            if let Some(i) = book.ignored.get_mut(&kind) {
                i.since = since;
            }
        }
        // THE FILTERS, from the shipped defaults and the person's file, with
        // the counts the brain kept — then a sweep of the kept list for a kind
        // an active filter now holds (a default added since it was written).
        book.set_filters(&filters.choices());
        for Caught {
            text,
            since,
            message,
        } in list.filtered_since
        {
            let folded = text.to_lowercase();
            if let Some(f) = book.filters.iter_mut().find(|f| f.folded == folded) {
                f.since = since;
                f.message = message;
            }
        }
        book.apply_filters();
        // WHAT WAS SEEN IN ANOTHER BOOT is over: read, and done.
        book.forget_seen_before(&boot);
        Self {
            state_dir,
            ignored: ignored_store,
            filters: filters_store,
            boot,
            state: Mutex::new(State {
                book,
                ..State::default()
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Writes the list now, with how often each ignored kind was heard —
    /// the brain's state, and never the person's file.
    fn write_list(&self, state: &mut State) {
        state.unwritten = None;
        write(
            self.state_dir.as_deref(),
            LIST_FILE,
            &ListFile {
                groups: state.book.groups.values().cloned().collect(),
                ignored_since: state
                    .book
                    .ignored
                    .values()
                    .map(|i| Since {
                        kind: i.kind.clone(),
                        since: i.since,
                    })
                    .collect(),
                filtered_since: state
                    .book
                    .filters()
                    .iter()
                    .filter(|f| f.since > 0)
                    .map(|f| Caught {
                        text: f.text.clone(),
                        since: f.since,
                        message: f.message.clone(),
                    })
                    .collect(),
            },
        );
    }

    /// Ignores a kind, or hears it again, in the book AND in the person's
    /// file. The file is read first, with `parsed`: one that does not parse
    /// is refused and left as it is; one that does is the base, so what the
    /// person changed in it by hand stays, and only this kind moves.
    fn choose(&self, state: &mut State, row: &str, ignore: bool) -> Acted {
        let Some(file) = self.ignored.parsed() else {
            tracing::error!(
                path = ?self.ignored.path(),
                "the file of ignored kinds does not parse; it is left as it is"
            );
            return Acted::Refused(super::why::UNREADABLE);
        };
        let kind = if ignore {
            state.book.group(row).cloned()
        } else {
            state.book.ignored_kind(row).cloned()
        };
        let Some(kind) = kind else {
            return Acted::Refused(super::why::GONE);
        };
        // THE FILE AS IT IS NOW is what is ignored, with the counts the brain
        // kept — then this one kind moves.
        let mut counts = std::mem::take(&mut state.book.ignored);
        for i in file.ignored {
            let since = counts.remove(&i.kind).map_or(0, |kept| kept.since);
            state
                .book
                .ignored
                .insert(i.kind.clone(), Ignored { since, ..i });
        }
        let done = if ignore {
            state.book.ignore(row)
        } else {
            state.book.unignore(&kind.id())
        };
        if !done && ignore {
            return Acted::Refused(super::why::GONE);
        }
        let written = self.ignored.write(&IgnoredFile {
            ignored: state.book.ignored.values().cloned().collect(),
        });
        if let Err(e) = written {
            tracing::warn!(error = %e, "the ignored kinds could not be kept");
        }
        Acted::Done
    }

    /// Changes the person's filters: reads their file first with `parsed`
    /// (one that does not parse is refused and left as it is), rebuilds the
    /// book's filters from it — so what they edited by hand stays — makes the
    /// one change, and writes the file as it is now with that change.
    fn refile(&self, state: &mut State, change: impl FnOnce(&mut Book) -> Acted) -> Acted {
        let Some(file) = self.filters.parsed() else {
            tracing::error!(
                path = ?self.filters.path(),
                "the file of filters does not parse; it is left as it is"
            );
            return Acted::Refused(super::why::UNREADABLE);
        };
        state.book.set_filters(&file.choices());
        let acted = change(&mut state.book);
        if acted != Acted::Done {
            return acted;
        }
        let written = self.filters.write(&FiltersFile::of(state.book.filters()));
        if let Err(e) = written {
            tracing::warn!(error = %e, "the filters could not be kept");
        }
        Acted::Done
    }

    /// The page moved.
    fn moved(state: &mut State, ctx: &Ctx) {
        state.generation += 1;
        ctx.changed();
    }
}

/// A text key as a cell.
fn key(k: &'static str) -> Cell {
    Cell::Key(k.into())
}

/// One kind of the list as a row: when it last happened, who said what, and
/// how many times — the one thing a press on it does, and the other one its
/// own key does (`also`).
fn row_of(g: &Group, act: &'static str, also: Option<&'static str>, tone: Tone) -> Row {
    Row {
        id: g.kind.id(),
        label: Cell::Moment(g.last),
        // THE JOURNAL'S OWN LINE, `<unit>: <message>` as `journalctl` prints
        // it — DATA, like the brain's log line (invariant 8's exception).
        value: Cell::Data(if g.kind.who.is_empty() {
            g.message.clone()
        } else {
            format!("{}: {}", g.kind.who, g.message)
        }),
        note: Cell::Count(g.count),
        tone,
        act: Some(Word::Borrowed(act)),
        also: also.map(Word::Borrowed),
    }
}

impl Module for JournalErrors {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }

    fn face(&self, _look: &Look<'_>) -> Face {
        let state = self.state();
        let unseen = state.book.unseen();
        let mut face = Face {
            screen: state.generation,
            ..Face::default()
        };
        // A MARK ONLY WHILE SOMETHING IS NOT SEEN YET (invariant 14).
        if unseen > 0 {
            face.marks.push(Mark {
                id: super::mark::UNSEEN,
                tone: Tone::Watching,
                count: u32::try_from(unseen).unwrap_or(u32::MAX),
                act: None,
            });
        }
        face
    }

    fn screen(&self, _look: &Look<'_>) -> Screen {
        let state = self.state();
        let sorted = state.book.sorted(&self.boot);
        let mut sections = Vec::new();
        // A JOURNAL THAT IS NOT FOLLOWED IS SAID FIRST, in the core's words:
        // an empty list under it would read as a machine with no errors.
        if let Some(why) = &state.lost {
            sections.push(Section {
                heading: super::key::JOURNAL_ERRORS_HEADING_JOURNAL.into(),
                rows: vec![Row {
                    id: "journal".to_owned(),
                    label: Cell::Key(format!("brain-journal-why-{why}").into()),
                    ..Row::default()
                }],
            });
        }
        let seen = super::action::SEEN.id;
        let ignore = super::action::IGNORE.id;
        let rows = |groups: &[&Group], act, also, tone| {
            groups
                .iter()
                .map(|g| row_of(g, act, also, tone))
                .collect::<Vec<_>>()
        };
        // AN ERROR NOT YET SEEN is marked seen by its press, and its type
        // ignored by its other key — one step, not seen first and ignored
        // after in the section below.
        sections.push(Section {
            heading: super::key::JOURNAL_ERRORS_HEADING_UNSEEN.into(),
            rows: rows(&sorted.unseen, seen, Some(ignore), Tone::Watching),
        });
        sections.push(Section {
            heading: super::key::JOURNAL_ERRORS_HEADING_EARLIER.into(),
            rows: rows(&sorted.earlier, seen, Some(ignore), Tone::Watching),
        });
        sections.push(Section {
            heading: super::key::JOURNAL_ERRORS_HEADING_SEEN.into(),
            rows: rows(&sorted.seen, ignore, None, Tone::Quiet),
        });
        sections.push(Section {
            heading: super::key::JOURNAL_ERRORS_HEADING_IGNORED.into(),
            rows: sorted
                .ignored
                .iter()
                .map(|i| Row {
                    id: i.kind.id(),
                    label: Cell::Data(i.kind.who.clone()),
                    value: Cell::Data(i.message.clone()),
                    note: Cell::Count(i.since),
                    tone: Tone::Quiet,
                    act: Some(Word::Borrowed(super::action::UNIGNORE.id)),
                    also: None,
                })
                .collect(),
        });
        // NOTHING AT ALL is a sentence, not a page of headings over nothing
        // (the page leaves out a section with no rows).
        if sorted.unseen.is_empty()
            && sorted.earlier.is_empty()
            && sorted.seen.is_empty()
            && sorted.ignored.is_empty()
        {
            sections.push(Section {
                heading: super::key::JOURNAL_ERRORS_HEADING_UNSEEN.into(),
                rows: vec![Row {
                    id: "none".to_owned(),
                    label: key(super::key::JOURNAL_ERRORS_ROW_NONE),
                    ..Row::default()
                }],
            });
        }
        // THE FILTERS, at the foot of the page: each trecho, the last line it
        // caught and how many — one that is on to turn off, one that is off to
        // turn on. A filtered line is counted here and never on the mark.
        let filters = state.book.filters();
        if !filters.is_empty() {
            let mut shown: Vec<&Filter> = filters.iter().collect();
            shown.sort_by(|a, b| a.off.cmp(&b.off).then_with(|| a.folded.cmp(&b.folded)));
            sections.push(Section {
                heading: super::key::JOURNAL_ERRORS_HEADING_FILTERED.into(),
                rows: shown
                    .iter()
                    .map(|f| Row {
                        id: f.id(),
                        label: Cell::Data(f.text.clone()),
                        value: Cell::Data(f.message.clone()),
                        note: Cell::Count(f.since),
                        tone: Tone::Quiet,
                        act: Some(Word::Borrowed(if f.off {
                            super::action::REFILTER.id
                        } else {
                            super::action::UNFILTER.id
                        })),
                        also: None,
                    })
                    .collect(),
            });
        }
        Screen { sections }
    }

    fn act(&self, act: &Act, ctx: &Ctx) -> Acted {
        let mut state = self.state();
        let action = act.action.as_str();
        // THE PAGE'S OWN, needing no row: mark everything seen, or add a filter
        // from what the person typed.
        if action == super::action::SEEN_ALL.id {
            state.book.see_all();
        } else if action == super::action::FILTER.id {
            let answer = act.answer.as_deref().unwrap_or("");
            let acted = self.refile(&mut state, |book| {
                let text = answer.trim();
                if text.is_empty() {
                    Acted::Refused(super::why::EMPTY)
                } else {
                    book.filter(text);
                    Acted::Done
                }
            });
            if acted != Acted::Done {
                return acted;
            }
        } else {
            // A ROW'S ACTION ASKED BY NAME, with no row: said so, and not taken
            // for a row that went away.
            let Some(row) = act.row.as_deref() else {
                return Acted::Refused(super::why::NO_ROW);
            };
            let acted = match action {
                a if a == super::action::SEEN.id => {
                    if state.book.see(row) {
                        Acted::Done
                    } else {
                        Acted::Refused(super::why::GONE)
                    }
                }
                a if a == super::action::IGNORE.id => self.choose(&mut state, row, true),
                a if a == super::action::UNIGNORE.id => self.choose(&mut state, row, false),
                a if a == super::action::UNFILTER.id => self.refile(&mut state, |book| {
                    if book.unfilter(row) {
                        Acted::Done
                    } else {
                        Acted::Refused(super::why::GONE)
                    }
                }),
                a if a == super::action::REFILTER.id => self.refile(&mut state, |book| {
                    if book.refilter(row) {
                        Acted::Done
                    } else {
                        Acted::Refused(super::why::GONE)
                    }
                }),
                _ => return Acted::Refused("unknown"),
            };
            if acted != Acted::Done {
                return acted;
            }
        }
        // A PERSON'S ACT IS WRITTEN AT ONCE: a brain stopped a second later
        // must not bring the mark back.
        self.write_list(&mut state);
        Self::moved(&mut state, ctx);
        Acted::Done
    }
}

impl Watcher for JournalErrors {
    fn sources(&self) -> Vec<Source> {
        vec![Source::Journal(Match {
            max_priority: Some(ronoctua_journal::ERR),
            backfill: Backfill::ThisBoot {
                most: rules::BACKFILL_MOST,
            },
            ..Match::default()
        })]
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        let now = ctx.now();
        let mut state = self.state();
        match heard {
            Heard::Journal { entry, .. } => {
                let at = i64::try_from(entry.realtime_us / 1_000_000).unwrap_or(now);
                state.book.hear(entry, at);
                state.unwritten.get_or_insert(now);
                Self::moved(&mut state, ctx);
            }
            Heard::Wake => {
                if state
                    .unwritten
                    .is_some_and(|since| since + WRITE_AFTER_S <= now)
                {
                    self.write_list(&mut state);
                }
            }
            Heard::Lost { why, .. } => {
                if state.lost.as_deref() != Some(why) {
                    state.lost = Some(why.to_owned());
                    Self::moved(&mut state, ctx);
                }
            }
            Heard::Back { .. } => {
                if state.lost.take().is_some() {
                    Self::moved(&mut state, ctx);
                }
            }
            Heard::Path { .. } | Heard::Line { .. } => {}
        }
    }

    fn next_wake(&self, _now: Moment) -> Option<Moment> {
        // A CLOCK ONLY WHILE THE LIST WAITS TO BE WRITTEN.
        self.state().unwritten.map(|since| since + WRITE_AFTER_S)
    }
}

/// The kind of a row, for a test.
#[cfg(test)]
fn kind_of(line: &str) -> rules::Kind {
    rules::Kind::of(
        &crate::module::Entry::parse(line).expect("a fixture that is not a journal line"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Entry;

    const BOOT: &str = "0123456789abcdef0123456789abcdef";

    fn errors() -> JournalErrors {
        JournalErrors::keeping(None, None, BOOT.to_owned())
    }

    fn look() -> Look<'static> {
        static PULSE: std::sync::OnceLock<crate::pulse::Pulse> = std::sync::OnceLock::new();
        static MINE: std::sync::OnceLock<crate::prefs::TaskPrefs> = std::sync::OnceLock::new();
        Look {
            now: 0,
            pulse: PULSE.get_or_init(crate::pulse::Pulse::default),
            mine: MINE.get_or_init(|| super::super::MANIFEST.defaults()),
            last: None,
        }
    }

    /// Hears a line through the module's own match, as the core would.
    fn hear(m: &JournalErrors, ctx: &Ctx, line: &str) -> bool {
        let entry = Entry::parse(line).expect("a fixture that is not a journal line");
        let Some(Source::Journal(wanted)) = m.sources().into_iter().next() else {
            panic!("no journal source");
        };
        if !wanted.accepts(&entry) {
            return false;
        }
        m.heard(
            Heard::Journal {
                source: 0,
                entry: &entry,
            },
            ctx,
        );
        true
    }

    fn marks(m: &JournalErrors) -> Vec<Mark> {
        m.face(&look()).marks
    }

    #[test]
    fn errors_of_one_kind_are_one_row_and_the_mark_counts_kinds() {
        let m = errors();
        let (ctx, kept) = Ctx::kept(super::super::ID, 100);
        assert!(marks(&m).is_empty(), "a mark at rest");
        for line in [
            fixtures::NGINX_BIND_80,
            fixtures::NGINX_BIND_443,
            fixtures::USER_SYNC,
            fixtures::KERNEL_IO,
        ] {
            assert!(hear(&m, &ctx, line), "an error was not accepted: {line}");
        }
        // A WARNING AND AN INFO LINE ARE NOT ERRORS.
        assert!(!hear(&m, &ctx, fixtures::WARNING));
        assert!(!hear(&m, &ctx, fixtures::INFO_NO_PRIORITY));
        let got = marks(&m);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].id, super::super::mark::UNSEEN);
        assert_eq!(
            got[0].tone,
            Tone::Watching,
            "the mark is not a watch's colour"
        );
        assert_eq!(
            got[0].count, 3,
            "two ports of one bind failure are one kind"
        );
        assert!(got[0].act.is_none(), "a click opens its page");
        assert_eq!(got[0].id.glyph, "journal-errors-unseen");
        assert!(
            !kept.changes().is_empty(),
            "the face moved and nobody was told"
        );
        // AND THE PAGE: this boot's kinds, newest first, the nginx one twice.
        let screen = m.screen(&look());
        let unseen = &screen.sections[0];
        assert_eq!(
            unseen.heading,
            super::super::key::JOURNAL_ERRORS_HEADING_UNSEEN
        );
        let nginx = unseen
            .rows
            .iter()
            .find(|r| matches!(&r.value, Cell::Data(d) if d.starts_with("nginx.service: ")))
            .expect("the nginx row");
        assert_eq!(nginx.note, Cell::Count(2));
        assert_eq!(
            nginx.value,
            Cell::Data(
                "nginx.service: bind() to 0.0.0.0:443 failed (98: Address in use)".to_owned()
            ),
            "the row does not show the last message"
        );
        assert_eq!(nginx.label, Cell::Moment(fixtures::AT + 60));
        assert_eq!(nginx.act.as_deref(), Some(super::super::action::SEEN.id));
        // ITS TYPE IS IGNORED FROM HERE, by the row's other key — not seen
        // first and ignored after, two steps for one wish.
        assert_eq!(nginx.also.as_deref(), Some(super::super::action::IGNORE.id));
        assert_eq!(nginx.tone, Tone::Watching);
        // THE KERNEL HAS NO UNIT, and is named `kernel`; the person's service
        // by its own unit, from their journal.
        let values: Vec<&Cell> = unseen.rows.iter().map(|r| &r.value).collect();
        assert!(
            values
                .iter()
                .any(|v| matches!(v, Cell::Data(d) if d.starts_with("kernel: ")))
        );
        assert!(
            values
                .iter()
                .any(|v| matches!(v, Cell::Data(d) if d.starts_with("syncthing.service: ")))
        );
    }

    #[test]
    fn marking_everything_seen_takes_the_mark_away_and_a_new_error_brings_it_back() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        hear(&m, &ctx, fixtures::KERNEL_IO);
        let nginx = kind_of(fixtures::NGINX_BIND_80).id();
        let act = |action: &str, row: Option<&str>| Act {
            action: action.to_owned(),
            row: row.map(ToOwned::to_owned),
            answer: None,
        };
        assert_eq!(m.act(&act("seen", Some(&nginx)), &ctx), Acted::Done);
        assert_eq!(marks(&m)[0].count, 1, "the mark did not count down");
        assert_eq!(m.act(&act("seen-all", None), &ctx), Acted::Done);
        assert!(
            marks(&m).is_empty(),
            "the mark stayed after everything was seen"
        );
        // THE ROWS STAY, as seen, and a press on one now ignores its kind.
        let screen = m.screen(&look());
        let seen = screen
            .sections
            .iter()
            .find(|s| s.heading == super::super::key::JOURNAL_ERRORS_HEADING_SEEN)
            .expect("the seen section");
        assert_eq!(seen.rows.len(), 2);
        assert!(
            seen.rows
                .iter()
                .all(|r| r.act.as_deref() == Some(super::super::action::IGNORE.id))
        );
        // AGAIN AFTER SEEN is news again.
        hear(&m, &ctx, fixtures::NGINX_BIND_443);
        assert_eq!(marks(&m)[0].count, 1);
        // A ROW THAT IS GONE is refused with the module's own word.
        assert_eq!(
            m.act(&act("seen", Some("ffffffffffffffff")), &ctx),
            Acted::Refused(super::super::why::GONE)
        );
        assert_eq!(
            super::super::MANIFEST.why_key(super::super::why::GONE),
            Some(super::super::key::JOURNAL_ERRORS_WHY_GONE)
        );
    }

    #[test]
    fn an_ignored_kind_leaves_the_list_and_is_kept_across_a_restart() {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-journal-errors-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let m = JournalErrors::keeping(
            Some(state_dir.clone()),
            Some(config_dir.clone()),
            BOOT.to_owned(),
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        hear(&m, &ctx, fixtures::USER_SYNC);
        let nginx = kind_of(fixtures::NGINX_BIND_80).id();
        let act = |action: &str| Act {
            action: action.to_owned(),
            row: Some(nginx.clone()),
            answer: None,
        };
        assert_eq!(m.act(&act("ignore"), &ctx), Acted::Done);
        hear(&m, &ctx, fixtures::NGINX_BIND_443);
        assert_eq!(
            marks(&m)[0].count,
            1,
            "an ignored kind came back to the mark"
        );
        // THE LIST IS WRITTEN LATER, a burst in one write: a clock is armed.
        let wake = m.next_wake(100).expect("no clock for an unwritten list");
        m.heard(Heard::Wake, &Ctx::kept(super::super::ID, wake).0);
        assert_eq!(
            m.next_wake(wake),
            None,
            "the clock stayed armed after the write"
        );
        // A NEW BRAIN, the same directories: the ignored kind is still ignored,
        // and the list is where it was.
        let again = JournalErrors::keeping(Some(state_dir), Some(config_dir), BOOT.to_owned());
        assert_eq!(marks(&again)[0].count, 1);
        let screen = again.screen(&look());
        let ignored = screen
            .sections
            .iter()
            .find(|s| s.heading == super::super::key::JOURNAL_ERRORS_HEADING_IGNORED)
            .expect("the ignored section");
        assert_eq!(ignored.rows.len(), 1);
        assert_eq!(ignored.rows[0].id, nginx);
        assert_eq!(ignored.rows[0].note, Cell::Count(1), "heard once since");
        assert_eq!(
            ignored.rows[0].act.as_deref(),
            Some(super::super::action::UNIGNORE.id)
        );
        // AND HEARD AGAIN once it is not ignored.
        assert_eq!(again.act(&act("unignore"), &ctx), Acted::Done);
        hear(&again, &ctx, fixtures::NGINX_BIND_80);
        assert_eq!(marks(&again)[0].count, 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn scratch(line: u32) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-journal-errors-{}-{line}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn act_on(action: &str, row: Option<&str>) -> Act {
        Act {
            action: action.to_owned(),
            row: row.map(ToOwned::to_owned),
            answer: None,
        }
    }

    #[test]
    fn a_file_of_ignored_kinds_that_does_not_parse_is_never_written_over() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        std::fs::create_dir_all(&config_dir).expect("mkdir");
        let broken = b"{\"ignored\": [ {\"kind\": {\"who\": \"nginx.service\"";
        std::fs::write(config_dir.join(IGNORED_FILE), broken).expect("write");
        let m = JournalErrors::keeping(Some(state_dir), Some(config_dir.clone()), BOOT.to_owned());
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        let nginx = kind_of(fixtures::NGINX_BIND_80).id();
        // SEEN IS THE BRAIN'S, and is taken; IGNORING IS THE PERSON'S FILE,
        // and is refused with a sentence.
        assert_eq!(m.act(&act_on("seen", Some(&nginx)), &ctx), Acted::Done);
        assert_eq!(
            m.act(&act_on("ignore", Some(&nginx)), &ctx),
            Acted::Refused(super::super::why::UNREADABLE)
        );
        assert_eq!(
            super::super::MANIFEST.why_key(super::super::why::UNREADABLE),
            Some(super::super::key::JOURNAL_ERRORS_WHY_UNREADABLE)
        );
        hear(&m, &ctx, fixtures::USER_SYNC);
        let wake = m.next_wake(100).expect("no clock for an unwritten list");
        m.heard(Heard::Wake, &Ctx::kept(super::super::ID, wake).0);
        assert_eq!(
            std::fs::read(config_dir.join(IGNORED_FILE)).expect("the file went away"),
            broken,
            "the person's file was written over"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_persons_file_moves_only_when_they_act_and_keeps_what_they_wrote() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let m = JournalErrors::keeping(
            Some(state_dir.clone()),
            Some(config_dir.clone()),
            BOOT.to_owned(),
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        hear(&m, &ctx, fixtures::USER_SYNC);
        // THE PERSON, BY HAND, ignores the kernel's kind in their own file.
        let kernel = kind_of(fixtures::KERNEL_IO);
        let by_hand = IgnoredFile {
            ignored: vec![Ignored {
                kind: kernel.clone(),
                message: "written by hand".to_owned(),
                since: 0,
            }],
        };
        std::fs::create_dir_all(&config_dir).expect("mkdir");
        std::fs::write(
            config_dir.join(IGNORED_FILE),
            serde_json::to_vec(&by_hand).expect("json"),
        )
        .expect("write");
        let nginx = kind_of(fixtures::NGINX_BIND_80).id();
        assert_eq!(m.act(&act_on("ignore", Some(&nginx)), &ctx), Acted::Done);
        let file: IgnoredFile =
            serde_json::from_slice(&std::fs::read(config_dir.join(IGNORED_FILE)).expect("read"))
                .expect("the file does not parse");
        let kinds: Vec<String> = file.ignored.iter().map(|i| i.kind.id()).collect();
        assert!(kinds.contains(&nginx), "the act was not kept");
        assert!(
            kinds.contains(&kernel.id()),
            "what the person wrote was lost"
        );
        // AND FROM NOW ON, hearing the ignored kinds and a burst of errors
        // moves the list and never their file.
        let before = std::fs::read(config_dir.join(IGNORED_FILE)).expect("read");
        hear(&m, &ctx, fixtures::NGINX_BIND_443);
        hear(&m, &ctx, fixtures::KERNEL_IO);
        let wake = m.next_wake(100).expect("no clock for an unwritten list");
        m.heard(Heard::Wake, &Ctx::kept(super::super::ID, wake).0);
        assert_eq!(
            std::fs::read(config_dir.join(IGNORED_FILE)).expect("read"),
            before,
            "the person's file moved without them"
        );
        // THE COUNTS ARE THE LIST'S, and come back with it.
        let again = JournalErrors::keeping(Some(state_dir), Some(config_dir), BOOT.to_owned());
        let since: Vec<u64> = again
            .state()
            .book
            .ignored
            .values()
            .map(|i| i.since)
            .collect();
        assert_eq!(since, [1, 1], "the counts since were not kept");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_rows_action_asked_by_name_says_to_choose_a_row() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        for action in ["seen", "ignore", "unignore"] {
            assert_eq!(
                m.act(&act_on(action, None), &ctx),
                Acted::Refused(super::super::why::NO_ROW),
                "{action}"
            );
        }
        assert_eq!(
            super::super::MANIFEST.why_key(super::super::why::NO_ROW),
            Some(super::super::key::JOURNAL_ERRORS_WHY_NO_ROW)
        );
        // THE PAGE'S OWN, by name, needs none.
        assert_eq!(m.act(&act_on("seen-all", None), &ctx), Acted::Done);
    }

    #[test]
    fn an_earlier_boot_is_its_own_section_and_what_was_seen_there_is_forgotten() {
        let m = JournalErrors::keeping(None, None, "ffffffffffffffffffffffffffffffff".to_owned());
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        let screen = m.screen(&look());
        let earlier = screen
            .sections
            .iter()
            .find(|s| s.heading == super::super::key::JOURNAL_ERRORS_HEADING_EARLIER)
            .expect("the earlier section");
        assert_eq!(
            earlier.rows.len(),
            1,
            "an error of another boot read as this one's"
        );
        assert!(
            screen
                .sections
                .iter()
                .find(|s| s.heading == super::super::key::JOURNAL_ERRORS_HEADING_UNSEEN)
                .is_some_and(|s| s.rows.is_empty())
        );
    }

    #[test]
    fn nothing_heard_is_a_sentence_and_a_journal_not_followed_is_said() {
        let m = errors();
        let screen = m.screen(&look());
        let rows: Vec<&Row> = screen.sections.iter().flat_map(|s| &s.rows).collect();
        // THE NONE ROW first, and the one shipped filter at the foot.
        assert_eq!(
            rows[0].label,
            key(super::super::key::JOURNAL_ERRORS_ROW_NONE)
        );
        assert!(rows[0].act.is_none());
        let filtered = screen
            .sections
            .iter()
            .find(|s| s.heading == super::super::key::JOURNAL_ERRORS_HEADING_FILTERED)
            .expect("the filtered section");
        assert_eq!(filtered.rows.len(), 1, "the shipped default is not shown");
        assert_eq!(
            filtered.rows[0].act.as_deref(),
            Some(super::super::action::UNFILTER.id)
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 1);
        m.heard(
            Heard::Lost {
                source: 0,
                why: "no-program",
            },
            &ctx,
        );
        let screen = m.screen(&look());
        assert_eq!(
            screen.sections[0].rows[0].label,
            Cell::Key("brain-journal-why-no-program".into())
        );
        m.heard(Heard::Back { source: 0 }, &ctx);
        assert_ne!(
            m.screen(&look()).sections[0].heading,
            super::super::key::JOURNAL_ERRORS_HEADING_JOURNAL
        );
    }

    #[test]
    fn it_asks_the_core_for_errors_and_this_boots_last_few() {
        let m = errors();
        let sources = m.sources();
        assert_eq!(sources.len(), 1);
        let Source::Journal(wanted) = &sources[0] else {
            panic!("not a journal source");
        };
        assert_eq!(wanted.max_priority, Some(ronoctua_journal::ERR));
        assert_eq!(
            wanted.backfill,
            Backfill::ThisBoot {
                most: rules::BACKFILL_MOST
            }
        );
        assert!(wanted.fields.is_empty() && wanted.message_contains.is_none());
    }

    #[test]
    fn a_shipped_filter_catches_the_noise_and_the_person_may_add_and_turn_off() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let m = JournalErrors::keeping(
            Some(state_dir.clone()),
            Some(config_dir.clone()),
            BOOT.to_owned(),
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        // THE SHIPPED DEFAULT holds the dbus noise, both buses: heard, counted
        // on the filter, never on the mark.
        assert!(hear(&m, &ctx, fixtures::DBUS_SYSTEM));
        assert!(hear(&m, &ctx, fixtures::DBUS_USER));
        assert!(marks(&m).is_empty(), "the dbus noise reached the mark");
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        assert_eq!(marks(&m)[0].count, 1, "only the real error counts");

        // THE PERSON ADDS A FILTER by typing it (the `filter` action's answer),
        // and it sweeps the nginx error off the mark at once.
        let typed = |answer: &str| Act {
            action: super::super::action::FILTER.id.to_owned(),
            row: None,
            answer: Some(answer.to_owned()),
        };
        assert_eq!(m.act(&typed("bind() to"), &ctx), Acted::Done);
        assert!(marks(&m).is_empty(), "a new filter did not sweep the list");
        // AN EMPTY ANSWER is refused with the module's word.
        assert_eq!(
            m.act(&typed("   "), &ctx),
            Acted::Refused(super::super::why::EMPTY)
        );

        // THE SHIPPED DEFAULT is turned off from its row, and the dbus noise
        // counts again from here (off is not retroactive).
        let default_id = filter_row(&m, "Ignoring duplicate name").expect("the default row");
        let on_row = |action: &str, id: &str| Act {
            action: action.to_owned(),
            row: Some(id.to_owned()),
            answer: None,
        };
        assert_eq!(
            m.act(
                &on_row(super::super::action::UNFILTER.id, &default_id),
                &ctx
            ),
            Acted::Done
        );
        hear(&m, &ctx, fixtures::DBUS_SYSTEM);
        assert_eq!(
            marks(&m)[0].count,
            1,
            "the default off let no dbus error through"
        );

        // A NEW BRAIN, the same directories: the person's own filter and their
        // choice to turn the default off both come back.
        let again = JournalErrors::keeping(Some(state_dir), Some(config_dir), BOOT.to_owned());
        let screen = again.screen(&look());
        let filtered = screen
            .sections
            .iter()
            .find(|s| s.heading == super::super::key::JOURNAL_ERRORS_HEADING_FILTERED)
            .expect("the filtered section");
        assert!(
            filtered.rows.iter().any(
                |r| matches!(&r.label, Cell::Data(d) if d == "Ignoring duplicate name")
                    && r.act.as_deref() == Some(super::super::action::REFILTER.id)
            ),
            "the default did not come back off"
        );
        assert!(
            filtered
                .rows
                .iter()
                .any(|r| matches!(&r.label, Cell::Data(d) if d == "bind() to")
                    && r.act.as_deref() == Some(super::super::action::UNFILTER.id)),
            "the person's own filter was lost"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The id of the filtered row whose trecho is `text`, for a test.
    fn filter_row(m: &JournalErrors, text: &str) -> Option<String> {
        m.screen(&look())
            .sections
            .iter()
            .find(|s| s.heading == super::super::key::JOURNAL_ERRORS_HEADING_FILTERED)?
            .rows
            .iter()
            .find(|r| matches!(&r.label, Cell::Data(d) if d == text))
            .map(|r| r.id.clone())
    }
}
