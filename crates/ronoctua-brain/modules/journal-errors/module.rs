//! The journal's errors, grouped by kind, and a mark until they are seen.
//!
//! The whole story is in `why.md` beside this file; what is here is its shape.
//!
//! # A watch on the journal the core already follows
//!
//! Two matches ([`Watcher::sources`]): every entry at `err` or worse, and the
//! warnings alone, each with a ring of its own, of the system's journal and the
//! person's (the core's follower reads both). On the brain's FIRST start — no
//! cursor yet — the core's one pass over the past hands this boot's last
//! [`rules::BACKFILL_MOST`] errors and [`rules::BACKFILL_WARNINGS`] warnings to
//! the modules on THEN; on every later start the cursor decides, and what
//! happened while the brain was off is heard then.
//!
//! # What it missed while it was off
//!
//! The cursor is everybody's, so it moves on while this is switched off. So
//! this keeps a place of its own, the last line it heard ([`State::left_at`]),
//! and the core reads for it what came after ([`Watcher::pass`]): by itself
//! when it comes back on, and on the page's `run` while it stays off. Only a
//! line moves the place: a pass that read nothing or could not read says
//! nothing of where the journal is, and forgetting the place over it would
//! read the whole boot again. A pass asked for is OWED until it ends, in the
//! file too ([`State::owed`]): a brain stopped while one read reads it again
//! as it starts ([`Watcher::owed`]), up to where the follower took over.
//!
//! # The book
//!
//! [`rules::Book`]: the kinds in the list, each with its count, its last
//! message and moment, its boot and how many times since it was seen; the plain
//! warnings apart from it; and the kinds ignored. The list is state
//! (`errors.json`, under its state directory, with how often each ignored kind
//! was heard since); what is ignored is the person's choice (`ignored.json`,
//! under its config directory, through `ronoctua-store`). A burst of errors is
//! one write of the list, a few seconds later ([`Watcher::next_wake`] arms a
//! clock only while something waits to be written), and the page hears of a
//! line that moved no count on the mark only every half minute — the numbers
//! on its row catch up then — because most lines are those. An error reaches
//! the page at once. The file does not wait for that clock: it goes ahead of
//! the journal's cursor ([`Watcher::keep`]), at the cursor's pace. A person's
//! act is written at once.
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
//! watch, with how many kinds, and in the colour of a crisis while one of them
//! is critical — and none at rest (invariant 14). It stays until everything is
//! marked seen. The warnings are never on it.

mod classify;
mod noise;
mod rules;

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use ronoctua_store::Store;

use crate::module::{
    Act, Acted, Arg, Backfill, Birth, Cell, Ctx, Cursor, Entry, Event, Face, Fact, Fold, Heard,
    Look, Manifest, Mark, Match, Module, Pass, Passed, Row, Screen, Section, Source, Swatch, Tone,
    Watcher, Word,
};
use crate::moment::Moment;

use rules::{Book, Choice, Filter, Group, Ignored, Kind, Origin, Severity};

/// The list, under its state directory.
const LIST_FILE: &str = "errors.json";

/// What is ignored, under its config directory.
const IGNORED_FILE: &str = "ignored.json";

/// What is filtered, under its config directory.
const FILTERS_FILE: &str = "filters.json";

/// The person's two switches, beside them ([`OptionsFile`]).
const OPTIONS_FILE: &str = "options.json";

/// A BURST is over once the journal has said no error for this long — then
/// its errors go out in one notification.
const BURST_QUIET_S: Moment = 10;

/// And no notification follows the last sooner than this: a machine that
/// errs without pause is one notification every few minutes, with what it
/// said since, and not a stream.
const BURST_GAP_S: Moment = 300;

/// The most lines one notification lists — the last kinds heard; the rest is
/// a count.
const BURST_MOST: usize = 8;

/// How long a change of the list waits before it is written, in seconds — so
/// a burst of a thousand lines is one write.
const WRITE_AFTER_S: Moment = 5;

/// How long the page waits to hear of a line that moved no count on the mark,
/// in seconds: a warning, or one a filter or an ignored kind took. Those are
/// most of what the journal says, and the mark has no news of them, so at most
/// one publication goes to them in this long, and the counts on their rows
/// catch up then. The file is not held to it, because a cursor is never written
/// ahead of the list ([`Watcher::keep`]).
const QUIET_AFTER_S: Moment = 30;

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
    /// What is filtered: the person's file (their snippets and which shipped
    /// ones they turned off).
    filters: Store<FiltersFile>,
    /// The person's two switches: their file.
    options: Store<OptionsFile>,
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
    /// When what waits is next done; `None` when nothing waits.
    write_at: Option<Moment>,
    /// What waits moved no count on the mark, so nothing has said the page
    /// changed: the wake says it.
    quietly: bool,
    /// A line was heard since the file was written, which a cursor about to
    /// be written needs in it.
    behind: bool,
    /// Where it stopped: the last line it read, and what a pass starts after.
    /// `None` when it never read one, and a pass reads this boot.
    left_at: Option<Cursor>,
    /// When: the moment of that line, or of being switched off.
    left_moment: Option<Moment>,
    /// The core says it is listening: between [`Watcher::started`] and
    /// [`Watcher::stopped`].
    on: bool,
    /// A pass is on its way, in this process.
    reading: bool,
    /// A pass is owed: asked for and not answered yet — kept in the file, so a
    /// brain stopped while one read reads it again ([`Watcher::owed`]). While
    /// one is, a line the follower brings does not move [`State::left_at`],
    /// which is where that pass reads from.
    owed: bool,
    /// Where the follower is while a pass is owed: the last line it brought,
    /// and its moment — the place, once a pass that ended behind it is done.
    followed: Option<(Cursor, Moment)>,
    /// The first line the follower brought while a pass was owed: where that
    /// pass, read again after a stop, ends — what came from there on was
    /// heard, or is the follower's to bring again.
    followed_from: Option<Cursor>,
    /// What the last pass found.
    last_run: Option<LastRun>,
    /// The person's switches, as their file said at start or they last set.
    options: OptionsFile,
    /// Since when the core has been bringing what happens as it happens —
    /// what a burst is made of. A line written before it (the read back to
    /// this boot's start, a pass) is never in a notification.
    live_since: Option<Moment>,
    /// The errors of the burst still to be told, one a kind — its latest
    /// line, and how many times it came — the last kinds heard, at most
    /// [`BURST_MOST`].
    burst: Vec<(Kind, String, u64)>,
    /// How many more kinds the burst heard than it lists.
    burst_more: u64,
    /// When the burst's last error came.
    burst_last: Option<Moment>,
    /// When the last burst was told.
    told_at: Option<Moment>,
}

/// THE PERSON'S TWO SWITCHES (2026-11-08): a notification for each burst of
/// errors — off, for a person who opts in, as the watch itself is — and the
/// mark on the modeline, on, for one who wants only the notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct OptionsFile {
    notify: bool,
    mark: bool,
}

impl Default for OptionsFile {
    fn default() -> Self {
        Self {
            notify: false,
            mark: true,
        }
    }
}

/// What the last pass found, for the page to say.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct LastRun {
    /// When it ended.
    at: Moment,
    /// How many lines reached a list.
    listed: u64,
    /// How many were past its ring, and went.
    lost: u64,
    /// Why it stopped short: a core word (`brain-journal-why-<word>`).
    why: Option<String>,
}

/// The list as it is kept — state, the brain's own.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct ListFile {
    /// The boot it was written in: what a filter caught counts only for it.
    boot: String,
    groups: Vec<Group>,
    /// The plain warnings: only those of this boot survive a start.
    warnings: Vec<Group>,
    /// How many times each ignored kind was heard since it was ignored.
    ignored_since: Vec<Since>,
    /// How many lines each filter caught in [`ListFile::boot`], and the last
    /// one, keyed by snippet — STATE, kept with the list, never in the person's
    /// file.
    filtered_since: Vec<Caught>,
    /// See [`State::left_at`].
    left_at: Option<Cursor>,
    /// See [`State::left_moment`].
    left_moment: Option<Moment>,
    /// See [`State::last_run`].
    last_run: Option<LastRun>,
    /// See [`State::owed`].
    owed: bool,
    /// See [`State::followed`]: the line.
    followed: Option<Cursor>,
    /// See [`State::followed`]: its moment.
    followed_moment: Option<Moment>,
    /// See [`State::followed_from`].
    followed_from: Option<Cursor>,
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

impl IgnoredFile {
    /// What it holds, by kind as today's folding writes it, so what the person
    /// ignored before a folding changed is still what they ignored — each with
    /// the kind the file wrote, which the list's counts were kept by.
    fn refolded(self) -> impl Iterator<Item = (Kind, Ignored)> {
        self.ignored.into_iter().map(|i| {
            let kept = i.kind.clone();
            let kind = i.kind.refolded_with(&i.message);
            (kept, Ignored { kind, ..i })
        })
    }
}

/// What is filtered as it is kept — the person's choice, and nothing that
/// moves on its own. A shipped snippet turned off, or with a reason of theirs,
/// is here; one they typed is here; a shipped snippet left as it is is NOT
/// here, and on by being shipped.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct FiltersFile {
    filters: Vec<Choice>,
}

impl FiltersFile {
    /// The file for these filters: the shipped ones the person changed, and
    /// their own — never a shipped snippet left as it is.
    fn of(filters: &[Filter]) -> Self {
        Self {
            filters: filters
                .iter()
                .filter(|f| f.origin == Origin::Custom || f.off || !f.reason.is_empty())
                .map(|f| Choice {
                    text: f.text.clone(),
                    off: f.off,
                    reason: f.reason.clone(),
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
        let filters_store = Store::at(config_dir.clone(), FILTERS_FILE);
        let options_store: Store<OptionsFile> = Store::at(config_dir, OPTIONS_FILE);
        let options = options_store.read();
        // A FILE THAT DOES NOT PARSE is read as nothing — `read` says so in the
        // log, loud — and is never written over (see `act`).
        let ignored: IgnoredFile = ignored_store
            .parsed()
            .unwrap_or_else(|| ignored_store.read());
        let filters: FiltersFile = filters_store
            .parsed()
            .unwrap_or_else(|| filters_store.read());
        // THE COUNTS WERE KEPT by the kind the person's file wrote — until they
        // act, that file is never written again — or by today's.
        let mut was: BTreeMap<Kind, Kind> = BTreeMap::new();
        let mut book = Book {
            groups: rules::by_kind(list.groups),
            warnings: rules::by_kind(list.warnings),
            ignored: ignored
                .refolded()
                .map(|(kept, i)| {
                    was.insert(kept, i.kind.clone());
                    (i.kind.clone(), i)
                })
                .collect(),
            filters: Vec::new(),
            boot: boot.clone(),
        };
        for Since { kind, since } in list.ignored_since {
            let kind = was.remove(&kind).unwrap_or_else(|| kind.refolded());
            if let Some(i) = book.ignored.get_mut(&kind) {
                i.since = since;
            }
        }
        // THE FILTERS, from the shipped ones and the person's file, with the
        // counts the brain kept — this boot's, and no other: that is the whole
        // of what ended a number that only grew — then a sweep of the kept list
        // for a kind an active filter now holds (one added since it was written).
        book.set_filters(&filters.filters);
        if list.boot == boot {
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
        }
        book.apply_filters();
        // WHAT WAS SEEN IN ANOTHER BOOT is over: read, and done.
        book.forget_seen_before(&boot);
        Self {
            state_dir,
            ignored: ignored_store,
            filters: filters_store,
            options: options_store,
            boot,
            state: Mutex::new(State {
                book,
                left_at: list.left_at,
                left_moment: list.left_moment,
                last_run: list.last_run,
                owed: list.owed,
                followed: list.followed.zip(list.followed_moment),
                followed_from: list.followed_from,
                options,
                ..State::default()
            }),
        }
    }

    /// Changes the person's switches with `change`, over what their file
    /// says now — one that does not parse is left as it is and refused.
    fn switch(&self, state: &mut State, change: impl FnOnce(&mut OptionsFile)) -> Acted {
        let Some(mut options) = self.options.parsed() else {
            return Acted::Refused(super::why::UNREADABLE);
        };
        change(&mut options);
        if let Err(e) = self.options.write(&options) {
            tracing::warn!(error = %e, "the journal's errors' options could not be kept");
            return Acted::Refused(super::why::UNREADABLE);
        }
        state.options = options;
        Acted::Done
    }

    /// One error of now, into the burst: a kind already in it counts once
    /// more and says its latest line, a new one goes last — and past
    /// [`BURST_MOST`], the oldest makes room and is counted among the rest.
    fn into_the_burst(state: &mut State, kind: Kind, line: String, now: Moment) {
        if let Some((_, said, n)) = state.burst.iter_mut().find(|(k, _, _)| *k == kind) {
            *said = line;
            *n += 1;
        } else {
            if state.burst.len() >= BURST_MOST {
                state.burst.remove(0);
                state.burst_more += 1;
            }
            state.burst.push((kind, line, 1));
        }
        state.burst_last = Some(now);
    }

    /// When the burst is to be told: the journal quiet for
    /// [`BURST_QUIET_S`], and [`BURST_GAP_S`] since the last one. `None`
    /// with nothing in it.
    fn burst_due(state: &State) -> Option<Moment> {
        let last = state.burst_last.filter(|_| !state.burst.is_empty())?;
        let quiet = last + BURST_QUIET_S;
        Some(state.told_at.map_or(quiet, |t| quiet.max(t + BURST_GAP_S)))
    }

    /// ONE NOTIFICATION for the burst: how many errors, and each kind's line
    /// — the last ones detected — with a count of the kinds past the list.
    fn tell_the_burst(state: &mut State, ctx: &Ctx, now: Moment) {
        let count: u64 = state.burst.iter().map(|(_, _, n)| n).sum::<u64>() + state.burst_more;
        let lines = state
            .burst
            .iter()
            .map(|(_, l, _)| l.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        tracing::info!(
            count,
            kinds = state.burst.len(),
            "a burst of the journal's errors, told"
        );
        ctx.tell(
            Event::of(&super::event::BURST)
                .with(
                    "count",
                    Arg::Number(i64::try_from(count).unwrap_or(i64::MAX)),
                )
                .with("lines", Arg::Text(lines))
                .with(
                    "more",
                    Arg::Number(i64::try_from(state.burst_more).unwrap_or(i64::MAX)),
                ),
        );
        Self::forget_the_burst(state);
        state.told_at = Some(now);
    }

    /// The burst, emptied.
    fn forget_the_burst(state: &mut State) {
        state.burst.clear();
        state.burst_more = 0;
        state.burst_last = None;
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Writes the list now and leaves nothing waiting: whoever calls this says
    /// the page moved.
    fn write_list(&self, state: &mut State) {
        state.write_at = None;
        state.quietly = false;
        self.write_file(state);
    }

    /// Writes the file, with how often each ignored kind was heard — the
    /// brain's state, and never the person's file.
    fn write_file(&self, state: &mut State) {
        state.behind = false;
        write(
            self.state_dir.as_deref(),
            LIST_FILE,
            &ListFile {
                boot: self.boot.clone(),
                groups: state.book.groups.values().cloned().collect(),
                warnings: state.book.warnings.values().cloned().collect(),
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
                left_at: state.left_at.clone(),
                left_moment: state.left_moment,
                last_run: state.last_run.clone(),
                owed: state.owed,
                followed: state.followed.as_ref().map(|(at, _)| at.clone()),
                followed_moment: state.followed.as_ref().map(|(_, moment)| *moment),
                followed_from: state.followed_from.clone(),
            },
        );
    }

    /// Does what waits: the file if a cursor has not taken it there, and the
    /// page if nothing has told it yet.
    fn write_what_waits(&self, state: &mut State, ctx: &Ctx) {
        let quietly = state.quietly;
        state.write_at = None;
        state.quietly = false;
        if state.behind {
            self.write_file(state);
        }
        if quietly {
            Self::moved(state, ctx);
        }
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
        for (_, i) in file.refolded() {
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
        state.book.set_filters(&file.filters);
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

/// The second an entry was written, by the journal's clock.
fn second_of(entry: &Entry, now: Moment) -> Moment {
    i64::try_from(entry.realtime_us / 1_000_000).unwrap_or(now)
}

/// The page's options: the two switches, each a row whose press turns it.
fn options_rows(options: OptionsFile) -> Vec<Row> {
    let on = |yes: bool| {
        key(if yes {
            super::key::JOURNAL_ERRORS_ON
        } else {
            super::key::JOURNAL_ERRORS_OFF
        })
    };
    vec![
        Row {
            id: "notify".to_owned(),
            label: key(super::key::JOURNAL_ERRORS_OPTION_NOTIFY),
            value: on(options.notify),
            act: Some(Word::Borrowed(super::action::NOTIFY.id)),
            ..Row::default()
        },
        Row {
            id: "mark".to_owned(),
            label: key(super::key::JOURNAL_ERRORS_OPTION_MARK),
            value: on(options.mark),
            act: Some(Word::Borrowed(super::action::MARK.id)),
            ..Row::default()
        },
    ]
}

/// The colour of how grave a kind is: the legend's, and one meaning in every
/// section.
const fn tone_of(severity: Severity) -> Tone {
    match severity {
        Severity::Critical => Tone::Critical,
        Severity::Error => Tone::Danger,
        Severity::Notable => Tone::Warning,
        Severity::Warning => Tone::Minor,
    }
}

/// The word for how grave a kind is, which the legend also says.
const fn word_of(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => super::key::JOURNAL_ERRORS_LEGEND_CRITICAL,
        Severity::Error => super::key::JOURNAL_ERRORS_LEGEND_ERROR,
        Severity::Notable => super::key::JOURNAL_ERRORS_LEGEND_NOTABLE,
        Severity::Warning => super::key::JOURNAL_ERRORS_LEGEND_WARNING,
    }
}

/// One colour of the legend.
fn swatch(severity: Severity, about: &'static str) -> Swatch {
    Swatch {
        tone: tone_of(severity),
        says: word_of(severity).into(),
        about: about.into(),
    }
}

/// The legend: one line, and no number, because the titles have those.
fn legend() -> Vec<Swatch> {
    vec![
        swatch(
            Severity::Critical,
            super::key::JOURNAL_ERRORS_LEGEND_CRITICAL_ABOUT,
        ),
        swatch(
            Severity::Error,
            super::key::JOURNAL_ERRORS_LEGEND_ERROR_ABOUT,
        ),
        swatch(
            Severity::Notable,
            super::key::JOURNAL_ERRORS_LEGEND_NOTABLE_ABOUT,
        ),
        swatch(
            Severity::Warning,
            super::key::JOURNAL_ERRORS_LEGEND_WARNING_ABOUT,
        ),
    ]
}

/// What the page says of a kind under the cursor: how grave in a word, since
/// colour alone is lost on some eyes, since when, how often this boot and
/// the message whole, which takes the room left.
fn foot_of(g: &Group, boot: &str) -> Vec<Fact> {
    let this_boot = if boot.is_empty() || g.boot == boot {
        g.boot_count
    } else {
        0
    };
    vec![
        Fact {
            label: super::key::JOURNAL_ERRORS_FACT_SEVERITY.into(),
            value: key(word_of(g.severity())),
            tone: tone_of(g.severity()),
        },
        Fact {
            label: super::key::JOURNAL_ERRORS_FACT_FIRST.into(),
            value: Cell::Moment(g.first),
            tone: Tone::Quiet,
        },
        Fact {
            label: super::key::JOURNAL_ERRORS_FACT_THIS_BOOT.into(),
            value: Cell::Count(this_boot),
            tone: Tone::Working,
        },
        Fact {
            label: super::key::JOURNAL_ERRORS_FACT_WHOLE.into(),
            value: Cell::Data(g.message.clone()),
            tone: Tone::Quiet,
        },
    ]
}

/// One kind of the list as a row: when it last happened, who said what, and
/// how many times — the one thing a press on it does, and the other one its
/// own key does (`also`). The line a filter by it starts from is the row's
/// own, for the person to cut down to what should go.
fn row_of(g: &Group, boot: &str, act: &'static str, also: &'static str) -> Row {
    // THE JOURNAL'S OWN LINE, `<unit>: <message>` as `journalctl` prints it —
    // DATA, like the brain's log line (invariant 8's exception).
    let shown = rules::line(&g.kind.who, rules::headline(&g.message));
    Row {
        id: g.kind.id(),
        label: Cell::Moment(g.last),
        starts: if also == super::action::FILTER_LINE.id {
            shown.clone()
        } else {
            String::new()
        },
        value: Cell::Data(shown),
        note: Cell::Count(g.count),
        tone: tone_of(g.severity()),
        act: Some(Word::Borrowed(act)),
        also: Some(Word::Borrowed(also)),
        foot: foot_of(g, boot),
        ..Row::default()
    }
}

/// One filter as a row: its snippet, what it last caught and how many this
/// boot, why it exists under it, a press to turn it off or on and its other
/// key to say why.
fn filter_row(f: &Filter) -> Row {
    let mut foot = vec![Fact {
        label: super::key::JOURNAL_ERRORS_FACT_THIS_BOOT.into(),
        value: Cell::Count(f.since),
        tone: Tone::Working,
    }];
    if !f.message.is_empty() {
        foot.push(Fact {
            label: super::key::JOURNAL_ERRORS_FACT_LAST_CAUGHT.into(),
            value: Cell::Data(f.message.clone()),
            tone: Tone::Quiet,
        });
    }
    // WHAT THE PERSON WROTE WINS, and what a filter ships with is the
    // language's: the reason a question starts from is only theirs to edit.
    let below = match f.shipped {
        _ if !f.reason.is_empty() => Cell::Data(f.reason.clone()),
        Some(sentence) => key(sentence),
        None => Cell::Empty,
    };
    Row {
        id: f.id(),
        label: Cell::Data(f.text.clone()),
        value: Cell::Data(f.message.clone()),
        note: Cell::Count(f.since),
        // MINOR AND NOT QUIET: a quiet value takes a setting's colour, which
        // reads as the legend's notable warning. What a filter held is no
        // severity at all.
        tone: Tone::Minor,
        act: Some(Word::Borrowed(if f.off {
            super::action::REFILTER.id
        } else {
            super::action::UNFILTER.id
        })),
        also: Some(Word::Borrowed(super::action::REASON.id)),
        below,
        foot,
        starts: f.reason.clone(),
    }
}

/// The rows of a module that is off: where it stopped, and what the last read
/// found.
fn run_rows(state: &State) -> Vec<Row> {
    let stopped = match (state.reading, &state.left_at, state.left_moment) {
        (true, ..) => key(super::key::JOURNAL_ERRORS_RUN_READING),
        (false, Some(_), Some(at)) => Cell::Moment(at),
        (false, ..) => key(super::key::JOURNAL_ERRORS_RUN_FROM_BOOT),
    };
    let mut rows = vec![Row {
        id: "stopped".to_owned(),
        label: key(super::key::JOURNAL_ERRORS_ROW_STOPPED),
        value: stopped,
        ..Row::default()
    }];
    if let Some(run) = &state.last_run {
        // WHY IT STOPPED SHORT before anything else it could say: a read that
        // failed and a read that found nothing are not the same news.
        let below = match &run.why {
            Some(why) => Cell::Key(format!("brain-journal-why-{why}").into()),
            None if run.lost > 0 => key(super::key::JOURNAL_ERRORS_RUN_CUT),
            None if run.listed == 0 => key(super::key::JOURNAL_ERRORS_RUN_NOTHING),
            None => Cell::Empty,
        };
        rows.push(Row {
            id: "last-run".to_owned(),
            label: key(super::key::JOURNAL_ERRORS_ROW_LAST_RUN),
            value: Cell::Moment(run.at),
            note: Cell::Count(run.listed),
            below,
            ..Row::default()
        });
    }
    rows
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
        // A MARK ONLY WHILE SOMETHING IS NOT SEEN YET (invariant 14) — and
        // while the person keeps it: one who wants only the notifications
        // turns it off, and the page still counts.
        if unseen > 0 && state.options.mark {
            face.marks.push(Mark {
                id: super::mark::UNSEEN,
                tone: if state.book.worst_unseen() == Some(Severity::Critical) {
                    Tone::Critical
                } else {
                    Tone::Watching
                },
                count: u32::try_from(unseen).unwrap_or(u32::MAX),
                act: None,
            });
        }
        face
    }

    fn screen(&self, look: &Look<'_>) -> Screen {
        let state = self.state();
        let sorted = state.book.sorted(&self.boot);
        let mut sections = Vec::new();
        // A JOURNAL THAT IS NOT FOLLOWED IS SAID FIRST, in the core's words:
        // an empty list under it would read as a machine with no errors.
        if let Some(why) = &state.lost {
            sections.push(Section {
                id: "journal".into(),
                heading: super::key::JOURNAL_ERRORS_HEADING_JOURNAL.into(),
                about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_JOURNAL.into(),
                rows: vec![Row {
                    id: "journal".to_owned(),
                    label: Cell::Key(format!("brain-journal-why-{why}").into()),
                    ..Row::default()
                }],
                ..Section::default()
            });
        }
        // SWITCHED OFF, IT SAYS WHERE IT STOPPED: what it holds is from then,
        // and the read that brings it up to now is the person's to ask for.
        if !look.mine.enabled {
            sections.push(Section {
                id: "run".into(),
                heading: super::key::JOURNAL_ERRORS_HEADING_RUN.into(),
                about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_RUN.into(),
                rows: run_rows(&state),
                ..Section::default()
            });
        }
        let seen = super::action::SEEN.id;
        let ignore = super::action::IGNORE.id;
        let filter_line = super::action::FILTER_LINE.id;
        let rows = |groups: &[&Group], act, also| {
            groups
                .iter()
                .map(|g| row_of(g, &self.boot, act, also))
                .collect::<Vec<_>>()
        };
        // NOTHING AT ALL is a sentence, not a page of headings over nothing
        // (the page leaves out a section with no rows), and it is a title
        // with no count: the one row under it is not a kind.
        let nothing = sorted.unseen.is_empty()
            && sorted.earlier.is_empty()
            && sorted.seen.is_empty()
            && sorted.ignored.is_empty();
        let unseen = if nothing {
            vec![Row {
                id: "none".to_owned(),
                label: key(if look.mine.enabled {
                    super::key::JOURNAL_ERRORS_ROW_NONE
                } else {
                    super::key::JOURNAL_ERRORS_ROW_OFF
                }),
                ..Row::default()
            }]
        } else {
            rows(&sorted.unseen, seen, ignore)
        };
        // AN ERROR NOT YET SEEN is marked seen by its press, and its type
        // ignored by its other key — one step, not seen first and ignored
        // after in the section below.
        sections.push(Section {
            id: "unseen".into(),
            heading: super::key::JOURNAL_ERRORS_HEADING_UNSEEN.into(),
            about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_UNSEEN.into(),
            rows: unseen,
            fold: if nothing { Fold::Never } else { Fold::Open },
            ..Section::default()
        });
        sections.push(Section {
            id: "earlier".into(),
            heading: super::key::JOURNAL_ERRORS_HEADING_EARLIER.into(),
            about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_EARLIER.into(),
            rows: rows(&sorted.earlier, seen, ignore),
            fold: Fold::Open,
            ..Section::default()
        });
        // A WARNING IS NOT ON THE MARK, so there is nothing to see in it: its
        // press ignores the kind, and its other key filters by the line.
        sections.push(Section {
            id: "warnings".into(),
            heading: super::key::JOURNAL_ERRORS_HEADING_WARNINGS.into(),
            about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_WARNINGS.into(),
            rows: rows(&sorted.warnings, ignore, filter_line),
            fold: Fold::Shut,
            ..Section::default()
        });
        sections.push(Section {
            id: "seen".into(),
            heading: super::key::JOURNAL_ERRORS_HEADING_SEEN.into(),
            about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_SEEN.into(),
            rows: rows(&sorted.seen, ignore, filter_line),
            fold: Fold::Shut,
            ..Section::default()
        });
        sections.push(Section {
            id: "ignored".into(),
            heading: super::key::JOURNAL_ERRORS_HEADING_IGNORED.into(),
            about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_IGNORED.into(),
            rows: sorted
                .ignored
                .iter()
                .map(|i| Row {
                    id: i.kind.id(),
                    label: Cell::Data(i.kind.who.clone()),
                    value: Cell::Data(rules::headline(&i.message).to_owned()),
                    note: Cell::Count(i.since),
                    // Minor, for the reason a filter row is (`filter_row`).
                    tone: Tone::Minor,
                    act: Some(Word::Borrowed(super::action::UNIGNORE.id)),
                    also: None,
                    ..Row::default()
                })
                .collect(),
            fold: Fold::Shut,
            ..Section::default()
        });
        // THE FILTERS, at the foot of the page: each snippet, the last line it
        // caught and how many this boot — one that is on to turn off, one that
        // is off to turn on. A filtered line is counted here and never on the
        // mark. What ships and caught nothing is a long list that says nothing
        // yet, in a section of its own; the person's own stays in the first,
        // where they will look for it.
        let (idle, mut caught): (Vec<&Filter>, Vec<&Filter>) = state
            .book
            .filters()
            .iter()
            .partition(|f| f.origin == Origin::Default && !f.off && f.since == 0);
        caught.sort_by_key(|f| (f.off, Reverse(f.since), f.folded.clone()));
        sections.push(Section {
            id: "filtered".into(),
            heading: super::key::JOURNAL_ERRORS_HEADING_FILTERED.into(),
            about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_FILTERED.into(),
            tally: Cell::Count(caught.iter().map(|f| f.since).sum()),
            rows: caught.iter().map(|f| filter_row(f)).collect(),
            fold: Fold::Shut,
        });
        sections.push(Section {
            id: "filtered-idle".into(),
            heading: super::key::JOURNAL_ERRORS_HEADING_FILTERED_IDLE.into(),
            about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_FILTERED_IDLE.into(),
            rows: idle.iter().map(|f| filter_row(f)).collect(),
            fold: Fold::Shut,
            ..Section::default()
        });
        sections.push(Section {
            id: "options".into(),
            heading: super::key::JOURNAL_ERRORS_HEADING_OPTIONS.into(),
            about: super::key::JOURNAL_ERRORS_ABOUT_SECTION_OPTIONS.into(),
            rows: options_rows(state.options),
            fold: Fold::Open,
            ..Section::default()
        });
        Screen {
            sections,
            legend: legend(),
        }
    }

    fn act(&self, act: &Act, ctx: &Ctx) -> Acted {
        let mut state = self.state();
        let action = act.action.as_str();
        // A READ BY HAND IS THE CORE'S TO MAKE: it refuses one while this
        // hears everything as it happens, and here is only the one on its way.
        if action == super::action::RUN.id {
            return if state.reading {
                Acted::Refused(super::why::READING)
            } else {
                Acted::Pass
            };
        }
        // THE PAGE'S OWN, needing no row: mark everything seen, or add a filter
        // from what the person typed — from nothing, or from a line of a row
        // they cut down, which is the same filter.
        if action == super::action::NOTIFY.id || action == super::action::MARK.id {
            let notify = action == super::action::NOTIFY.id;
            let acted = self.switch(&mut state, |o| {
                if notify {
                    o.notify = !o.notify;
                } else {
                    o.mark = !o.mark;
                }
            });
            if acted != Acted::Done {
                return acted;
            }
            // A BURST WAITING to be told goes with the notifications turned
            // off: switched on again, only what comes from then on is told.
            if !state.options.notify {
                Self::forget_the_burst(&mut state);
            }
        } else if action == super::action::SEEN_ALL.id {
            state.book.see_all();
        } else if action == super::action::FILTER.id || action == super::action::FILTER_LINE.id {
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
                a if a == super::action::REASON.id => {
                    let answer = act.answer.as_deref().unwrap_or("");
                    self.refile(&mut state, |book| {
                        if book.give_reason(row, answer) {
                            Acted::Done
                        } else {
                            Acted::Refused(super::why::GONE)
                        }
                    })
                }
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
        vec![
            Source::Journal(Match {
                max_priority: Some(ronoctua_journal::ERR),
                backfill: Backfill::ThisBoot {
                    most: rules::BACKFILL_MOST,
                },
                ..Match::default()
            }),
            // THE WARNINGS ALONE, with a ring of their own, so a burst of them
            // reaches back no further than itself and pushes out no error.
            Source::Journal(Match {
                max_priority: Some(ronoctua_journal::WARNING),
                min_priority: Some(ronoctua_journal::WARNING),
                backfill: Backfill::ThisBoot {
                    most: rules::BACKFILL_WARNINGS,
                },
                ..Match::default()
            }),
        ]
    }

    fn started(&self, now: Moment, ctx: &Ctx) {
        let mut state = self.state();
        state.on = true;
        state.live_since = Some(now);
        Self::moved(&mut state, ctx);
    }

    fn stopped(&self, now: Moment, ctx: &Ctx) {
        let mut state = self.state();
        state.on = false;
        state.live_since = None;
        Self::forget_the_burst(&mut state);
        state.left_moment = Some(now);
        // NOW, because off is the one state with no wake: the core tells a
        // module on, and what waited would wait until it came back.
        self.write_list(&mut state);
        Self::moved(&mut state, ctx);
    }

    fn keep(&self, _now: Moment, _ctx: &Ctx) {
        let mut state = self.state();
        if !state.behind {
            return;
        }
        self.write_file(&mut state);
        // THE FILE, AND ONLY THE FILE: the cursor is due every few seconds and
        // at once after a silence, and the page of a quiet line stays on its
        // half minute. A line that moved the mark's count has told the page
        // already.
        if !state.quietly {
            state.write_at = None;
        }
    }

    fn pass(&self, _now: Moment, ctx: &Ctx) -> Option<Pass> {
        let mut state = self.state();
        // OWED SINCE A BRAIN THAT STOPPED while it read: read again up to the
        // first line the follower brought then, and no further — those were
        // heard. A pass asked for afresh starts with the follower nowhere.
        let until = if state.owed {
            state.followed_from.clone()
        } else {
            state.followed = None;
            state.followed_from = None;
            None
        };
        state.reading = true;
        state.owed = true;
        // AT ONCE: a brain stopped before the next cursor would otherwise
        // forget that the stretch this reads was never heard.
        self.write_list(&mut state);
        Self::moved(&mut state, ctx);
        Some(Pass {
            after: state.left_at.clone(),
            most: rules::PASS_MOST,
            until,
        })
    }

    fn owed(&self) -> bool {
        self.state().owed
    }

    fn passed(&self, passed: Passed<'_>, ctx: &Ctx) {
        let now = ctx.now();
        let mut state = self.state();
        // EACH SOURCE HANDS ITS OWN, oldest first; heard in the order they
        // were written, a row ends on the message that came last.
        let mut entries: Vec<&Entry> = passed.shares.iter().flat_map(|(_, e)| e).collect();
        entries.sort_by_key(|e| e.realtime_us);
        let mut listed = 0;
        for entry in &entries {
            let heard = state.book.hear(entry, second_of(entry, now));
            if !matches!(heard, rules::Heard::Ignored | rules::Heard::Filtered) {
                listed += 1;
            }
        }
        // THE PLACE MOVES ONLY WITH A LINE. A pass that read nothing or could
        // not read hands no `last`, and the place it started from still
        // stands: wiping it would have the next read count the whole boot
        // again. And never behind the follower, which brought lines while
        // this read and may be past where this pass ended, on or off since —
        // unless the pass read every line the follower brought, and is past
        // it: the core skips what the follower brings up to there, so nothing
        // else would move the place over lines already counted.
        let followed = state.followed.take();
        state.followed_from = None;
        state.owed = false;
        match passed.last {
            Some(last) if passed.ahead || followed.is_none() => {
                state.left_at = Some(last.clone());
                state.left_moment = Some(entries.last().map_or(now, |e| second_of(e, now)));
            }
            _ => {
                if let Some((at, moment)) = followed {
                    state.left_at = Some(at);
                    state.left_moment = Some(moment);
                }
            }
        }
        state.last_run = Some(LastRun {
            at: now,
            listed,
            lost: passed.lost,
            why: passed.why.map(|why| why.id().to_owned()),
        });
        state.reading = false;
        // AT ONCE, moved or not: off, nothing else will write it.
        self.write_list(&mut state);
        Self::moved(&mut state, ctx);
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        let now = ctx.now();
        let mut state = self.state();
        match heard {
            Heard::Journal { entry, .. } => {
                let at = second_of(entry, now);
                // WHILE A PASS IS OWED the place is where it reads from, and
                // the follower's is kept beside it until it ends.
                if state.owed {
                    state
                        .followed_from
                        .get_or_insert_with(|| entry.cursor.clone());
                    state.followed = Some((entry.cursor.clone(), at));
                } else {
                    state.left_at = Some(entry.cursor.clone());
                    state.left_moment = Some(at);
                }
                state.behind = true;
                match state.book.hear(entry, at) {
                    rules::Heard::New | rules::Heard::Again => {
                        // INTO THE BURST, when the person asked to be told and
                        // the line is of now and not read back from before.
                        if state.options.notify && state.live_since.is_some_and(|s| at >= s) {
                            Self::into_the_burst(
                                &mut state,
                                Kind::of(entry),
                                rules::shown_of(entry),
                                now,
                            );
                        }
                        let soon = now + WRITE_AFTER_S;
                        state.write_at = Some(state.write_at.map_or(soon, |at| at.min(soon)));
                        Self::moved(&mut state, ctx);
                    }
                    // NO COUNT ON THE MARK MOVED: nothing for the mark, and
                    // most of what the journal says — the counts on their rows
                    // reach the page with the wake.
                    rules::Heard::Quiet | rules::Heard::Ignored | rules::Heard::Filtered => {
                        state.write_at.get_or_insert(now + QUIET_AFTER_S);
                        state.quietly = true;
                    }
                }
            }
            Heard::Wake => {
                if state.write_at.is_some_and(|at| at <= now) {
                    self.write_what_waits(&mut state, ctx);
                }
                if Self::burst_due(&state).is_some_and(|at| at <= now) {
                    Self::tell_the_burst(&mut state, ctx, now);
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
        // A CLOCK ONLY WHILE SOMETHING WAITS: THE FILE, THE PAGE, OR A BURST.
        let state = self.state();
        match (state.write_at, Self::burst_due(&state)) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
mod fixtures;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Refusal;

    const BOOT: &str = "0123456789abcdef0123456789abcdef";

    /// The kind of a row.
    fn kind_of(line: &str) -> rules::Kind {
        rules::Kind::of(&Entry::parse(line).expect("a fixture that is not a journal line"))
    }

    fn errors() -> JournalErrors {
        JournalErrors::keeping(None, None, BOOT.to_owned())
    }

    /// What it may look at while it is on, the state most of these are about:
    /// it ships off, and off its page opens with where it stopped.
    fn look() -> Look<'static> {
        static PULSE: std::sync::OnceLock<crate::pulse::Pulse> = std::sync::OnceLock::new();
        static MINE: std::sync::OnceLock<crate::prefs::TaskPrefs> = std::sync::OnceLock::new();
        Look {
            now: 0,
            pulse: PULSE.get_or_init(crate::pulse::Pulse::default),
            mine: MINE.get_or_init(|| crate::prefs::TaskPrefs {
                enabled: true,
                ..super::super::MANIFEST.defaults()
            }),
            last: None,
        }
    }

    /// The same while it is off, as it ships.
    fn look_off() -> Look<'static> {
        static MINE: std::sync::OnceLock<crate::prefs::TaskPrefs> = std::sync::OnceLock::new();
        Look {
            mine: MINE.get_or_init(|| super::super::MANIFEST.defaults()),
            ..look()
        }
    }

    /// Hears a line through the module's own matches, as the core would: each
    /// source that accepts it. How many did.
    fn heard_by(m: &JournalErrors, ctx: &Ctx, line: &str) -> usize {
        let entry = Entry::parse(line).expect("a fixture that is not a journal line");
        let mut accepted = 0;
        for (source, wanted) in m.sources().into_iter().enumerate() {
            let Source::Journal(wanted) = wanted else {
                panic!("not a journal source");
            };
            if wanted.accepts(&entry) {
                accepted += 1;
                m.heard(
                    Heard::Journal {
                        source,
                        entry: &entry,
                    },
                    ctx,
                );
            }
        }
        accepted
    }

    fn hear(m: &JournalErrors, ctx: &Ctx, line: &str) -> bool {
        heard_by(m, ctx, line) > 0
    }

    fn marks(m: &JournalErrors) -> Vec<Mark> {
        m.face(&look()).marks
    }

    /// One line of a made-up journal, the `n`th: its cursor says so, and it is
    /// written `n` seconds after [`fixtures::AT`].
    fn written(n: u64, priority: u8, unit: &str, message: &str) -> String {
        let at = fixtures::AT.unsigned_abs() + n;
        serde_json::json!({
            "__CURSOR": format!("cursor-{n}"),
            "__REALTIME_TIMESTAMP": format!("{at}000000"),
            "_BOOT_ID": BOOT,
            "PRIORITY": priority.to_string(),
            "_SYSTEMD_UNIT": unit,
            "MESSAGE": message,
        })
        .to_string()
    }

    /// A pass over `journal`, as the core makes one: from where the module
    /// says, each source its own lines, and the place of the last line read.
    fn read(m: &JournalErrors, ctx: &Ctx, journal: &[String]) {
        let pass = m.pass(ctx.now(), ctx).expect("it wants no pass");
        let entries: Vec<Entry> = journal
            .iter()
            .map(|line| Entry::parse(line).expect("a line of no journal"))
            .collect();
        let from = pass.after.as_ref().map_or(0, |after| {
            entries
                .iter()
                .position(|e| e.cursor == *after)
                .map_or(0, |at| at + 1)
        });
        let shares: Vec<(usize, Vec<Entry>)> = m
            .sources()
            .into_iter()
            .enumerate()
            .map(|(source, wanted)| {
                let Source::Journal(wanted) = wanted else {
                    panic!("not a journal source");
                };
                let taken = entries[from..]
                    .iter()
                    .filter(|e| wanted.accepts(e))
                    .cloned()
                    .collect();
                (source, taken)
            })
            .collect();
        let last = entries[from..].last().map(|e| e.cursor.clone());
        m.passed(
            Passed {
                shares: &shares,
                lost: 0,
                last: last.as_ref(),
                why: None,
                ahead: false,
            },
            ctx,
        );
    }

    /// The section of this id, of a page.
    fn section<'a>(screen: &'a Screen, id: &str) -> Option<&'a Section> {
        screen.sections.iter().find(|s| s.id == id)
    }

    /// How many times the kind that says `what` was counted, in the list or
    /// among the warnings.
    fn count_of(m: &JournalErrors, what: &str) -> u64 {
        let state = m.state();
        state
            .book
            .groups
            .values()
            .chain(state.book.warnings.values())
            .filter(|g| g.message.contains(what))
            .map(|g| g.count)
            .sum()
    }

    /// The events a wake at `now` tells, after the lines heard.
    fn wake_at(m: &JournalErrors, now: Moment) -> Vec<crate::module::Event> {
        let (ctx, kept) = Ctx::kept(super::super::ID, now);
        m.heard(Heard::Wake, &ctx);
        kept.events().into_iter().map(|(_, e)| e).collect()
    }

    /// FAILS IF: a burst of errors is told before the journal is quiet, or in
    /// more than one notification, or without its lines — the last ones
    /// detected — and its count; a second burst comes sooner than the gap; or
    /// anything is told while the person did not ask for it.
    #[test]
    fn a_burst_of_errors_is_one_notification_once_the_journal_is_quiet() {
        let m = errors();
        let start = fixtures::AT;
        let (ctx, _) = Ctx::kept(super::super::ID, start);
        m.started(start, &ctx);
        // NOT ASKED FOR: heard, listed, and told to nobody.
        assert!(hear(
            &m,
            &ctx,
            &written(1, 3, "a.service", "the disk is gone")
        ));
        assert!(
            wake_at(&m, start + 60).is_empty(),
            "told without being asked"
        );
        assert_eq!(m.state().options, OptionsFile::default());
        m.state().options.notify = true;
        let (ctx, _) = Ctx::kept(super::super::ID, start + 100);
        for (n, unit, said) in [
            (101, "a.service", "the disk 7 is gone"),
            (102, "b.service", "the disk 7 is gone"),
            (103, "a.service", "the disk 9 is gone"),
        ] {
            assert!(hear(&m, &ctx, &written(n, 3, unit, said)));
        }
        assert_eq!(
            JournalErrors::burst_due(&m.state()),
            Some(start + 100 + BURST_QUIET_S)
        );
        assert!(
            wake_at(&m, start + 105).is_empty(),
            "told while it still came"
        );
        let told = wake_at(&m, start + 100 + BURST_QUIET_S);
        assert_eq!(told.len(), 1, "{told:?}");
        assert_eq!(told[0].tag, super::super::event::BURST.tag);
        let arg = |name: &str| {
            told[0]
                .args
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, a)| a.clone())
        };
        assert_eq!(arg("count"), Some(Arg::Number(3)));
        let Some(Arg::Text(lines)) = arg("lines") else {
            panic!("no lines: {told:?}");
        };
        assert_eq!(lines.lines().count(), 2, "one line a kind: {lines}");
        assert!(lines.contains("a.service") && lines.contains("b.service"));
        assert!(
            lines.contains("disk 9"),
            "a kind says its latest line: {lines}"
        );
        assert_eq!(
            JournalErrors::burst_due(&m.state()),
            None,
            "a clock with nothing to tell"
        );
        // THE NEXT BURST WAITS FOR THE GAP, and then tells what came since.
        let (ctx, _) = Ctx::kept(super::super::ID, start + 130);
        assert!(hear(
            &m,
            &ctx,
            &written(130, 3, "c.service", "out of memory")
        ));
        assert!(wake_at(&m, start + 150).is_empty(), "sooner than the gap");
        let told = wake_at(&m, start + 100 + BURST_QUIET_S + BURST_GAP_S);
        assert_eq!(told.len(), 1);
    }

    /// FAILS IF: what is read back from before the watch began — the boot's
    /// start, a pass — is told as if it happened now, or the mark stays when
    /// the person turned it off (the page still counting).
    #[test]
    fn what_came_before_is_never_told_and_the_mark_can_go() {
        let m = errors();
        m.state().options.notify = true;
        let (ctx, _) = Ctx::kept(super::super::ID, fixtures::AT + 1000);
        m.started(fixtures::AT + 1000, &ctx);
        assert!(hear(&m, &ctx, &written(1, 3, "a.service", "boom")));
        assert!(
            wake_at(&m, fixtures::AT + 2000).is_empty(),
            "the past was told"
        );
        assert_eq!(marks(&m).len(), 1);
        m.state().options.mark = false;
        assert!(marks(&m).is_empty(), "the mark the person turned off");
        assert_eq!(m.state().book.unseen(), 1, "and the page stopped counting");
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
        // A WARNING IS HEARD AND NOT COUNTED, and an info line is not heard.
        assert_eq!(heard_by(&m, &ctx, fixtures::WARNING), 1);
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
        // AND THE PAGE: this boot's kinds, the nginx one twice.
        let screen = m.screen(&look());
        let unseen = &screen.sections[0];
        assert_eq!(unseen.id, "unseen");
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
        assert_eq!(
            nginx.tone,
            Tone::Danger,
            "an error is not an error's colour"
        );
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
        let seen = section(&screen, "seen").expect("the seen section");
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
        let ignored = section(&screen, "ignored").expect("the ignored section");
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
        let earlier = section(&screen, "earlier").expect("the earlier section");
        assert_eq!(
            earlier.rows.len(),
            1,
            "an error of another boot read as this one's"
        );
        assert!(section(&screen, "unseen").is_some_and(|s| s.rows.is_empty()));
    }

    #[test]
    fn nothing_heard_is_a_sentence_and_a_journal_not_followed_is_said() {
        let m = errors();
        let screen = m.screen(&look());
        let rows: Vec<&Row> = screen.sections.iter().flat_map(|s| &s.rows).collect();
        // THE NONE ROW first, under a title that counts nothing, and what
        // ships at the foot: nothing caught yet, so all of it idle.
        assert_eq!(
            rows[0].label,
            key(super::super::key::JOURNAL_ERRORS_ROW_NONE)
        );
        assert!(rows[0].act.is_none());
        assert_eq!(screen.sections[0].fold, Fold::Never);
        let idle = section(&screen, "filtered-idle").expect("the idle filters");
        assert_eq!(
            idle.rows.len(),
            noise::SHIPPED.len(),
            "what ships is not shown"
        );
        assert_eq!(
            idle.rows[0].act.as_deref(),
            Some(super::super::action::UNFILTER.id)
        );
        assert!(section(&screen, "filtered").is_some_and(|s| s.rows.is_empty()));
        // OFF, IT SAYS SO: an empty list then proves nothing about the machine.
        let screen = m.screen(&look_off());
        let none = section(&screen, "unseen").expect("the section of the sentence");
        assert_eq!(
            none.rows[0].label,
            key(super::super::key::JOURNAL_ERRORS_ROW_OFF)
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
        assert_eq!(sources.len(), 2);
        let (Source::Journal(errors), Source::Journal(warnings)) = (&sources[0], &sources[1])
        else {
            panic!("not two journal sources");
        };
        assert_eq!(errors.max_priority, Some(ronoctua_journal::ERR));
        assert_eq!(errors.min_priority, None);
        assert_eq!(
            errors.backfill,
            Backfill::ThisBoot {
                most: rules::BACKFILL_MOST
            }
        );
        // THE WARNINGS ALONE: an error is not heard twice, and a ring of their
        // own keeps them from pushing the errors out of the past.
        assert_eq!(warnings.max_priority, Some(ronoctua_journal::WARNING));
        assert_eq!(warnings.min_priority, Some(ronoctua_journal::WARNING));
        assert_eq!(
            warnings.backfill,
            Backfill::ThisBoot {
                most: rules::BACKFILL_WARNINGS
            }
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        assert_eq!(heard_by(&m, &ctx, fixtures::NGINX_BIND_80), 1);
        assert_eq!(heard_by(&m, &ctx, fixtures::WARNING), 1);
        assert!(errors.fields.is_empty() && errors.message_contains.is_none());
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
        let filtered = section(&screen, "filtered").expect("the filtered section");
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

    /// The id of the filter row whose snippet is `text`, for a test.
    fn filter_row(m: &JournalErrors, text: &str) -> Option<String> {
        m.screen(&look())
            .sections
            .iter()
            .filter(|s| s.id == "filtered" || s.id == "filtered-idle")
            .flat_map(|s| &s.rows)
            .find(|r| matches!(&r.label, Cell::Data(d) if d == text))
            .map(|r| r.id.clone())
    }

    #[test]
    fn a_page_is_its_ten_sections_each_saying_what_it_is() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        // ONE ROW IN EACH, so none is the empty one the page would leave out.
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        hear(&m, &ctx, &written(1, 4, "w.service", "a plain warning"));
        hear(&m, &ctx, fixtures::KERNEL_IO);
        hear(&m, &ctx, fixtures::USER_SYNC);
        let seen = kind_of(fixtures::KERNEL_IO).id();
        assert_eq!(m.act(&act_on("seen", Some(&seen)), &ctx), Acted::Done);
        let ignored = kind_of(fixtures::USER_SYNC).id();
        assert_eq!(m.act(&act_on("ignore", Some(&ignored)), &ctx), Acted::Done);
        hear(&m, &ctx, fixtures::DBUS_SYSTEM);
        m.heard(
            Heard::Lost {
                source: 0,
                why: "unreadable",
            },
            &ctx,
        );
        let screen = m.screen(&look_off());
        let shape: Vec<(&str, Fold)> = screen
            .sections
            .iter()
            .map(|s| (s.id.as_ref(), s.fold))
            .collect();
        assert_eq!(
            shape,
            [
                ("journal", Fold::Never),
                ("run", Fold::Never),
                ("unseen", Fold::Open),
                ("earlier", Fold::Open),
                ("warnings", Fold::Shut),
                ("seen", Fold::Shut),
                ("ignored", Fold::Shut),
                ("filtered", Fold::Shut),
                ("filtered-idle", Fold::Shut),
                ("options", Fold::Open),
            ]
        );
        for s in &screen.sections {
            assert!(!s.heading.is_empty(), "`{}` has no title", s.id);
            assert_eq!(
                s.about,
                format!("journal-errors-about-section-{}", s.id),
                "the section says nothing of itself"
            );
        }
        // THE COUNT OF THE FILTERS IS LINES, the others' their rows.
        assert_eq!(
            section(&screen, "filtered").map(|s| s.tally.clone()),
            Some(Cell::Count(1))
        );
        assert_eq!(
            section(&screen, "warnings").map(|s| s.tally.clone()),
            Some(Cell::Empty)
        );
        // THE LEGEND IS ONE COLOUR A DEGREE, the words in the rows' own colour.
        let legend: Vec<(Tone, &str)> = screen
            .legend
            .iter()
            .map(|s| (s.tone, s.says.as_ref()))
            .collect();
        assert_eq!(
            legend,
            [
                (
                    Tone::Critical,
                    super::super::key::JOURNAL_ERRORS_LEGEND_CRITICAL
                ),
                (Tone::Danger, super::super::key::JOURNAL_ERRORS_LEGEND_ERROR),
                (
                    Tone::Warning,
                    super::super::key::JOURNAL_ERRORS_LEGEND_NOTABLE
                ),
                (
                    Tone::Minor,
                    super::super::key::JOURNAL_ERRORS_LEGEND_WARNING
                ),
            ]
        );
        assert!(screen.legend.iter().all(|s| !s.about.is_empty()));
    }

    #[test]
    fn a_row_says_how_grave_in_words_and_the_rest_under_the_cursor() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        let long = "the first line\nand the rest of it";
        hear(&m, &ctx, &written(1, 2, "a.service", long));
        hear(&m, &ctx, &written(2, 2, "a.service", long));
        let screen = m.screen(&look());
        let row = &section(&screen, "unseen").expect("the list").rows[0];
        assert_eq!(row.tone, Tone::Critical);
        assert_eq!(
            row.value,
            Cell::Data("a.service: the first line".to_owned()),
            "a row shows the first line"
        );
        let labels: Vec<&str> = row.foot.iter().map(|f| f.label.as_ref()).collect();
        assert_eq!(
            labels,
            [
                super::super::key::JOURNAL_ERRORS_FACT_SEVERITY,
                super::super::key::JOURNAL_ERRORS_FACT_FIRST,
                super::super::key::JOURNAL_ERRORS_FACT_THIS_BOOT,
                super::super::key::JOURNAL_ERRORS_FACT_WHOLE,
            ]
        );
        assert_eq!(
            row.foot[0].value,
            key(super::super::key::JOURNAL_ERRORS_LEGEND_CRITICAL),
            "colour was the only thing that said how grave"
        );
        assert_eq!(row.foot[0].tone, Tone::Critical);
        assert_eq!(row.foot[1].value, Cell::Moment(fixtures::AT + 1));
        assert_eq!(row.foot[2].value, Cell::Count(2));
        assert_eq!(row.foot[3].value, Cell::Data(long.to_owned()));
        // A ROW THAT ASKS NOTHING starts from nothing.
        assert!(row.starts.is_empty());
    }

    #[test]
    fn a_warning_row_starts_its_filter_from_the_line_and_stays_off_the_mark() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::WARNING);
        assert!(marks(&m).is_empty(), "a warning reached the mark");
        let screen = m.screen(&look());
        let row = &section(&screen, "warnings").expect("the warnings").rows[0];
        assert_eq!(row.tone, Tone::Minor);
        assert_eq!(row.act.as_deref(), Some(super::super::action::IGNORE.id));
        assert_eq!(
            row.also.as_deref(),
            Some(super::super::action::FILTER_LINE.id)
        );
        assert_eq!(row.starts, "nginx.service: low on worker connections");
        // IGNORED FROM THE ROW, and it leaves.
        let id = row.id.clone();
        assert_eq!(m.act(&act_on("ignore", Some(&id)), &ctx), Acted::Done);
        assert!(
            m.screen(&look())
                .sections
                .iter()
                .all(|s| { s.id != "warnings" || s.rows.is_empty() })
        );
        // AND FILTERED FROM THE ROW, by the line the person cut down.
        let m = errors();
        hear(&m, &ctx, fixtures::WARNING);
        let cut = Act {
            action: super::super::action::FILTER_LINE.id.to_owned(),
            row: Some(id),
            answer: Some("low on worker".to_owned()),
        };
        assert_eq!(m.act(&cut, &ctx), Acted::Done);
        assert!(
            m.state().book.warnings.is_empty(),
            "the filter left it there"
        );
    }

    #[test]
    fn the_warnings_survive_a_restart_in_the_same_boot_and_not_in_a_new_one() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let keeping = |boot: &str| {
            JournalErrors::keeping(
                Some(state_dir.clone()),
                Some(config_dir.clone()),
                boot.to_owned(),
            )
        };
        let m = keeping(BOOT);
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::WARNING);
        m.keep(100, &ctx);
        let warnings = |m: &JournalErrors| m.state().book.warnings.len();
        assert_eq!(warnings(&keeping(BOOT)), 1, "a restart lost the warnings");
        assert_eq!(
            warnings(&keeping("ffffffffffffffffffffffffffffffff")),
            0,
            "another boot's warnings came back"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn what_a_filter_counted_survives_a_restart_and_is_nothing_in_a_new_boot() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let keeping = |boot: &str| {
            JournalErrors::keeping(
                Some(state_dir.clone()),
                Some(config_dir.clone()),
                boot.to_owned(),
            )
        };
        let m = keeping(BOOT);
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::DBUS_SYSTEM);
        hear(&m, &ctx, fixtures::DBUS_USER);
        m.keep(100, &ctx);
        let caught = |m: &JournalErrors| m.state().book.filters()[0].since;
        assert_eq!(
            caught(&keeping(BOOT)),
            2,
            "a restart lost this boot's count"
        );
        // A NEW BOOT STARTS FROM NOTHING, and that is what ended the number
        // that only grew.
        let next = keeping("ffffffffffffffffffffffffffffffff");
        assert_eq!(caught(&next), 0);
        assert!(next.state().book.filters()[0].message.is_empty());
        // AN OLDER FILE, which names no boot, is another boot's.
        std::fs::write(
            state_dir.join(LIST_FILE),
            r#"{"filtered-since":[{"text":"Ignoring duplicate name","since":3536}]}"#,
        )
        .expect("write");
        assert_eq!(caught(&keeping(BOOT)), 0, "3536 came back");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_filter_row_says_why_and_how_many_and_the_last_line() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::DBUS_SYSTEM);
        let screen = m.screen(&look());
        let filtered = section(&screen, "filtered").expect("the filtered section");
        assert_eq!(filtered.tally, Cell::Count(1));
        let row = &filtered.rows[0];
        assert_eq!(
            row.below,
            key(super::super::key::JOURNAL_ERRORS_REASON_DUPLICATE_NAME)
        );
        assert_eq!(row.also.as_deref(), Some(super::super::action::REASON.id));
        assert_eq!(row.foot.len(), 2);
        assert_eq!(row.foot[0].value, Cell::Count(1));
        assert_eq!(row.foot[0].tone, Tone::Working);
        assert_eq!(
            row.foot[0].label,
            super::super::key::JOURNAL_ERRORS_FACT_THIS_BOOT
        );
        assert_eq!(row.foot[1].tone, Tone::Quiet);
        assert!(
            matches!(&row.foot[1].value, Cell::Data(d) if d.contains("Ignoring duplicate name"))
        );
        // ONE THAT CAUGHT NOTHING has no last line to say.
        let idle = section(&screen, "filtered-idle").expect("the idle filters");
        assert_eq!(idle.rows[0].foot.len(), 1);
    }

    #[test]
    fn a_reason_on_a_shipped_filter_is_written_and_blank_clears_it() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let m = JournalErrors::keeping(
            Some(state_dir.clone()),
            Some(config_dir.clone()),
            BOOT.to_owned(),
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        let id = filter_row(&m, "Ignoring duplicate name").expect("the row");
        let reason = |answer: &str| Act {
            action: super::super::action::REASON.id.to_owned(),
            row: Some(id.clone()),
            answer: Some(answer.to_owned()),
        };
        // ON, AND WITH A REASON: the file holds it though it is off nowhere.
        assert_eq!(m.act(&reason("my own words"), &ctx), Acted::Done);
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(config_dir.join(FILTERS_FILE)).expect("read"))
                .expect("json");
        assert_eq!(file["filters"][0]["reason"], "my own words");
        assert_eq!(file["filters"][0]["off"], false);
        let again =
            JournalErrors::keeping(Some(state_dir), Some(config_dir.clone()), BOOT.to_owned());
        let screen = again.screen(&look());
        let row = section(&screen, "filtered-idle")
            .and_then(|s| s.rows.iter().find(|r| r.id == id))
            .expect("the row");
        assert_eq!(row.below, Cell::Data("my own words".to_owned()));
        assert_eq!(
            row.starts, "my own words",
            "the question starts from nothing"
        );
        // BLANK GIVES THE SENTENCE IT SHIPS WITH BACK, and the file empties.
        assert_eq!(m.act(&reason(""), &ctx), Acted::Done);
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(config_dir.join(FILTERS_FILE)).expect("read"))
                .expect("json");
        assert_eq!(file["filters"].as_array().map(Vec::len), Some(0));
        let screen = m.screen(&look());
        let row = section(&screen, "filtered-idle")
            .and_then(|s| s.rows.iter().find(|r| r.id == id))
            .expect("the row");
        assert_eq!(
            row.below,
            key(super::super::key::JOURNAL_ERRORS_REASON_DUPLICATE_NAME)
        );
        // A REASON FOR A ROW THAT WENT is refused.
        let gone = Act {
            row: Some("ffffffffffffffff".to_owned()),
            ..reason("x")
        };
        assert_eq!(m.act(&gone, &ctx), Acted::Refused(super::super::why::GONE));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_shipped_filter_the_person_turned_off_in_a_file_stays_off() {
        // THE FILE AS IT IS ON A MACHINE TODAY, written before there were
        // reasons or more than one shipped snippet.
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        std::fs::create_dir_all(&config_dir).expect("mkdir");
        std::fs::write(
            config_dir.join(FILTERS_FILE),
            r#"{"filters":[{"text":"Ignoring duplicate name","off":true}]}"#,
        )
        .expect("write");
        let m = JournalErrors::keeping(Some(state_dir), Some(config_dir.clone()), BOOT.to_owned());
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        assert!(m.state().book.filters()[0].off, "the person's off was lost");
        assert!(hear(&m, &ctx, fixtures::DBUS_SYSTEM));
        assert_eq!(marks(&m)[0].count, 1, "a filter the person turned off held");
        let screen = m.screen(&look());
        let row = section(&screen, "filtered")
            .map(|s| &s.rows[0])
            .expect("a filter that is off is with those that are on");
        assert_eq!(row.act.as_deref(), Some(super::super::action::REFILTER.id));
        // AND A CHANGE TO ANOTHER FILTER keeps it.
        let other = filter_row(&m, "hrtimer: interrupt took").expect("the row");
        assert_eq!(
            m.act(
                &act_on(super::super::action::UNFILTER.id, Some(&other)),
                &ctx
            ),
            Acted::Done
        );
        let file = std::fs::read_to_string(config_dir.join(FILTERS_FILE)).expect("read");
        assert!(file.contains("Ignoring duplicate name"), "{file}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_old_failure_of_a_unit_stays_ignored_and_its_two_old_rows_are_one() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        std::fs::create_dir_all(&state_dir).expect("mkdir");
        std::fs::create_dir_all(&config_dir).expect("mkdir");
        // THE PERSON IGNORED A FAILURE, when each of its lines was a kind.
        let ignored = IgnoredFile {
            ignored: vec![Ignored {
                kind: rules::Kind {
                    who: "made-up.service".to_owned(),
                    template: "made-up.service: Failed with result 'exit-code'.".to_owned(),
                },
                message: "made-up.service: Failed with result 'exit-code'.".to_owned(),
                since: 0,
            }],
        };
        std::fs::write(
            config_dir.join(IGNORED_FILE),
            serde_json::to_vec(&ignored).expect("json"),
        )
        .expect("write");
        // AND ANOTHER ONE IS TWO ROWS OF THE OLD LIST.
        let row = |template: &str| {
            serde_json::json!({
                "kind": {"who": "other.service", "template": template},
                "message": template,
                "first": 1, "last": 2, "count": 1, "unseen": 1, "boot": BOOT,
            })
        };
        let list = serde_json::json!({"groups": [
            row("Failed to start other.service."),
            row("other.service: Failed with result 'exit-code'."),
        ]});
        std::fs::write(state_dir.join(LIST_FILE), list.to_string()).expect("write");
        let m = JournalErrors::keeping(Some(state_dir), Some(config_dir), BOOT.to_owned());
        assert_eq!(m.state().book.groups.len(), 1, "one failure stayed two");
        assert_eq!(marks(&m)[0].count, 1, "one failure counted twice");
        // BOTH ITS LINES, HEARD NOW, are what the person ignored.
        for line in [fixtures::UNIT_RESULT, fixtures::UNIT_START_FAILED] {
            let entry = Entry::parse(line).expect("a fixture that is not a journal line");
            assert_eq!(
                m.state().book.hear(&entry, 3),
                rules::Heard::Ignored,
                "an ignored failure came back"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_old_list_and_an_old_ignored_file_hold_under_todays_folding() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        std::fs::create_dir_all(&state_dir).expect("mkdir");
        std::fs::create_dir_all(&config_dir).expect("mkdir");
        let crash = |pid: u32| rules::Kind {
            who: format!("systemd-coredump@3-{pid}-0.service"),
            template: "Process # (a) of user # dumped core.\n\nStack trace of thread #:".to_owned(),
        };
        // THE PERSON IGNORED ONE CRASH, when each crash was a kind of its own.
        let ignored = IgnoredFile {
            ignored: vec![Ignored {
                kind: crash(1),
                message: "Process 1 (a) of user 1000 dumped core.\n\nStack".to_owned(),
                since: 0,
            }],
        };
        std::fs::write(
            config_dir.join(IGNORED_FILE),
            serde_json::to_vec(&ignored).expect("json"),
        )
        .expect("write");
        // AND TWO MORE OF THE SAME ARE IN THE OLD LIST, TWO ROWS.
        let row = |pid: u32| {
            serde_json::json!({
                "kind": {"who": crash(pid).who, "template": crash(pid).template},
                "message": "Process (b)",
                "first": 1, "last": 2, "count": 1, "unseen": 1, "boot": BOOT,
            })
        };
        let list = serde_json::json!({"groups": [row(2), row(3)]});
        std::fs::write(state_dir.join(LIST_FILE), list.to_string()).expect("write");
        let before = std::fs::read(config_dir.join(IGNORED_FILE)).expect("read");
        let m = JournalErrors::keeping(Some(state_dir), Some(config_dir.clone()), BOOT.to_owned());
        assert_eq!(m.state().book.groups.len(), 1, "two rows of one crash");
        // A CRASH HEARD NOW is what the person ignored.
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        let now = written(
            1,
            2,
            "systemd-coredump@7-9000-0.service",
            "Process 9000 (a) of user 1000 dumped core.\n\nStack trace of thread 9000:",
        );
        hear(&m, &ctx, &now);
        assert_eq!(
            m.state().book.ignored.values().next().map(|i| i.since),
            Some(1),
            "an ignored kind of the old file is not ignored any more"
        );
        // THE FILE IS WRITTEN ONLY WHEN THE PERSON ACTS.
        assert_eq!(
            std::fs::read(config_dir.join(IGNORED_FILE)).expect("read"),
            before
        );
        let kind = m.state().book.ignored.keys().next().cloned().expect("kind");
        assert_eq!(
            m.act(&act_on("unignore", Some(&kind.id())), &ctx),
            Acted::Done,
            "the act could not find what the file holds"
        );
        assert!(m.state().book.ignored.is_empty(), "the old kind stayed in");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a kind kept under the folding before this one, with a word
    /// it cut into pieces — a Nix hash, a short rev, a name `mktemp` made —
    /// stops matching its own lines: ignored, they come back on the list and
    /// the mark, and an old row stands beside today's of the same kind.
    #[test]
    fn a_kind_kept_with_a_word_the_old_folding_cut_holds_under_todays() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        std::fs::create_dir_all(&state_dir).expect("mkdir");
        std::fs::create_dir_all(&config_dir).expect("mkdir");
        // AS THE FOLDING BEFORE WROTE THEM: each run of digits of the word
        // folded alone. (message then, template then, a line of it now)
        let cases = [
            (
                "Cannot open /nix/store/62yhnxbs4zbclpansd8vcpnwz5js40zs-system-path/share",
                "Cannot open /nix/store/#yhnxbs#zbclpansd#vcpnwz#js#zs-system-path/share",
                "Cannot open /nix/store/0aw8bdz1jnc0ihxa7kd3mf6lzr2qpg9x-system-path/share",
            ),
            (
                "Generation 26.11.20261027.31ecbac is broken",
                "Generation #.#.#.#ecbac is broken",
                "Generation 26.11.20261103.9d2f0aa is broken",
            ),
            (
                "Cannot write /tmp/nix-shell.6BABLQ/stdout",
                "Cannot write /tmp/nix-shell.#BABLQ/stdout",
                "Cannot write /tmp/nix-shell.Xk3PqZ/stdout",
            ),
        ];
        let kept = |template: &str| rules::Kind {
            who: "made-up.service".to_owned(),
            template: template.to_owned(),
        };
        // THE PERSON IGNORED THE FIRST TWO, with a count kept by the old kind.
        let ignored = IgnoredFile {
            ignored: cases[..2]
                .iter()
                .map(|(message, template, _)| Ignored {
                    kind: kept(template),
                    message: (*message).to_owned(),
                    since: 0,
                })
                .collect(),
        };
        std::fs::write(
            config_dir.join(IGNORED_FILE),
            serde_json::to_vec(&ignored).expect("json"),
        )
        .expect("write");
        // AND THE THIRD IS A ROW OF THE OLD LIST.
        let (message, template, _) = cases[2];
        let list = serde_json::json!({
            "groups": [{
                "kind": kept(template),
                "message": message,
                "first": 1, "last": 2, "count": 1, "unseen": 1, "boot": BOOT,
            }],
            "ignored-since": [{"kind": kept(cases[0].1), "since": 4}],
        });
        std::fs::write(state_dir.join(LIST_FILE), list.to_string()).expect("write");
        let m = JournalErrors::keeping(Some(state_dir), Some(config_dir), BOOT.to_owned());
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        for (n, (_, _, now)) in (1..).zip(cases) {
            hear(&m, &ctx, &written(n, 3, "made-up.service", now));
        }
        let state = m.state();
        let since: Vec<u64> = state.book.ignored.values().map(|i| i.since).collect();
        assert_eq!(state.book.ignored.len(), 2);
        assert!(
            since.contains(&5) && since.contains(&1),
            "an ignored kind stopped matching its lines, or lost its count: {since:?}"
        );
        assert_eq!(state.book.groups.len(), 1, "an old row beside today's");
        let row = state.book.groups.values().next().expect("the row");
        assert_eq!(row.count, 2);
        assert_eq!(row.kind.template, "Cannot write /tmp/nix-shell.#/stdout");
        drop(state);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_run_while_off_is_the_cores_and_a_second_one_says_it_is_reading() {
        let m = errors();
        let (ctx, kept) = Ctx::kept(super::super::ID, 100);
        // BY NAME, WITH NO ROW, as the page sends it.
        let run = act_on(super::super::action::RUN.id, None);
        assert_eq!(m.act(&run, &ctx), Acted::Pass);
        const { assert!(super::super::action::RUN.while_off) };
        let pass = m.pass(100, &ctx).expect("it wants the pass");
        assert_eq!(pass.after, None, "a place it never had");
        assert_eq!(pass.most, rules::PASS_MOST);
        let screen = m.screen(&look_off());
        assert_eq!(
            section(&screen, "run").map(|s| s.rows[0].value.clone()),
            Some(key(super::super::key::JOURNAL_ERRORS_RUN_READING)),
            "nothing says it is reading"
        );
        assert_eq!(
            m.act(&run, &ctx),
            Acted::Refused(super::super::why::READING)
        );
        assert_eq!(
            super::super::MANIFEST.why_key(super::super::why::READING),
            Some(super::super::key::JOURNAL_ERRORS_WHY_READING)
        );
        assert!(!kept.changes().is_empty(), "the page did not hear of it");
    }

    #[test]
    fn the_run_section_is_there_only_while_off_and_says_where_it_stopped() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        assert!(section(&m.screen(&look()), "run").is_none());
        // NEVER RAN: from the boot's start.
        let screen = m.screen(&look_off());
        let run = section(&screen, "run").expect("the section");
        assert_eq!(run.rows.len(), 1);
        assert_eq!(
            run.rows[0].value,
            key(super::super::key::JOURNAL_ERRORS_RUN_FROM_BOOT)
        );
        // RAN, AND STOPPED: since when, whatever the last line it heard.
        m.started(100, &ctx);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        let (off, _) = Ctx::kept(super::super::ID, 500);
        m.stopped(500, &off);
        let screen = m.screen(&look_off());
        assert_eq!(
            section(&screen, "run").map(|s| s.rows[0].value.clone()),
            Some(Cell::Moment(500))
        );
    }

    #[test]
    fn the_last_read_says_what_it_found() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 700);
        let last_read = |m: &JournalErrors| {
            m.screen(&look_off())
                .sections
                .iter()
                .find(|s| s.id == "run")
                .and_then(|s| s.rows.get(1).cloned())
                .expect("the row of the last read")
        };
        let journal = [
            written(1, 3, "a.service", "broken"),
            written(2, 4, "b.service", "slow"),
            written(3, 3, "dbus.service", "Ignoring duplicate name 'x'"),
        ];
        read(&m, &ctx, &journal);
        let found = last_read(&m);
        assert_eq!(found.value, Cell::Moment(700));
        assert_eq!(
            found.note,
            Cell::Count(2),
            "a filtered line is not on a list"
        );
        assert_eq!(found.below, Cell::Empty);
        // NOTHING NEW is said, and the row is not left blank.
        read(&m, &ctx, &journal);
        assert_eq!(
            last_read(&m).below,
            key(super::super::key::JOURNAL_ERRORS_RUN_NOTHING)
        );
        // MORE THAN THE RING HELD.
        m.pass(700, &ctx);
        m.passed(
            Passed {
                shares: &[],
                lost: 12,
                last: None,
                why: None,
                ahead: false,
            },
            &ctx,
        );
        assert_eq!(
            last_read(&m).below,
            key(super::super::key::JOURNAL_ERRORS_RUN_CUT)
        );
        // AND A READ THAT FAILED says that, in the core's words.
        m.pass(700, &ctx);
        m.passed(
            Passed {
                shares: &[],
                lost: 12,
                last: None,
                why: Some(Refusal::Unstartable),
                ahead: false,
            },
            &ctx,
        );
        assert_eq!(
            last_read(&m).below,
            Cell::Key("brain-journal-why-unstartable".into())
        );
    }

    #[test]
    fn a_line_read_by_a_pass_is_not_read_again() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        let mut journal = vec![
            written(1, 3, "a.service", "broken"),
            written(2, 4, "b.service", "slow"),
        ];
        read(&m, &ctx, &journal);
        assert_eq!(count_of(&m, "broken"), 1);
        assert_eq!(count_of(&m, "slow"), 1);
        read(&m, &ctx, &journal);
        assert_eq!(count_of(&m, "broken"), 1, "the second read read it again");
        assert_eq!(count_of(&m, "slow"), 1);
        // WHAT CAME AFTER is read, and only that.
        journal.push(written(3, 3, "a.service", "broken"));
        read(&m, &ctx, &journal);
        assert_eq!(count_of(&m, "broken"), 2);
    }

    #[test]
    fn nothing_repeats_when_it_comes_back_on() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        m.started(100, &ctx);
        let mut journal = vec![written(1, 3, "a.service", "broken")];
        hear(&m, &ctx, &journal[0]);
        m.stopped(101, &ctx);
        // OFF: the journal goes on.
        journal.push(written(2, 3, "a.service", "broken"));
        journal.push(written(3, 3, "a.service", "broken"));
        // BACK ON, with the pass the core makes for it and a line the follower
        // brings while it runs. The core hands each line once (its cursor is
        // how it knows), and what is asked of the module is where it goes on.
        m.started(200, &ctx);
        read(&m, &ctx, &journal);
        assert_eq!(count_of(&m, "broken"), 3, "what it missed was not read");
        // OFF AT ONCE, nothing heard since: the pass was the last thing read,
        // and a run from before it would read the same lines twice.
        m.stopped(201, &ctx);
        read(&m, &ctx, &journal);
        assert_eq!(count_of(&m, "broken"), 3, "a line came twice");
        // AND BACK ON WITH A FOLLOWER AHEAD OF THE PASS: its place wins.
        m.started(300, &ctx);
        let ahead = written(9, 3, "a.service", "broken");
        let pass = m.pass(300, &ctx).expect("a pass");
        assert_eq!(pass.after, Some(Cursor("cursor-3".to_owned())));
        hear(&m, &ctx, &ahead);
        let behind = Cursor("cursor-5".to_owned());
        m.passed(
            Passed {
                shares: &[],
                lost: 0,
                last: Some(&behind),
                why: None,
                ahead: false,
            },
            &ctx,
        );
        assert_eq!(
            m.state().left_at,
            Some(Cursor("cursor-9".to_owned())),
            "a pass moved the place back behind the follower"
        );
    }

    /// FAILS IF: a brain stopped while a pass read loses what the pass was
    /// reading — nothing in the file says it is owed, and the follower's lines
    /// moved the place over it — or the pass read again counts a line the
    /// follower brought before the stop.
    #[test]
    fn a_pass_the_brain_stopped_in_is_owed_and_read_again_up_to_the_follower() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let keeping = || {
            JournalErrors::keeping(
                Some(state_dir.clone()),
                Some(config_dir.clone()),
                BOOT.to_owned(),
            )
        };
        let journal: Vec<String> = (1..=6)
            .map(|n| written(n, 3, "a.service", "broken"))
            .collect();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        let m = keeping();
        read(&m, &ctx, &journal[..1]);
        assert!(!m.owed(), "a pass that ended is owed");
        // SWITCHED ON: the pass reads from the first line, and the follower
        // brings the fifth and the sixth meanwhile — then the brain stops.
        m.started(100, &ctx);
        let pass = m.pass(100, &ctx).expect("a pass");
        assert_eq!(pass.until, None, "a pass asked afresh stops short");
        hear(&m, &ctx, &journal[4]);
        hear(&m, &ctx, &journal[5]);
        m.keep(100, &ctx);
        drop(m);
        // STARTED AGAIN: the pass is owed, from where it was to read, up to
        // the first line the follower brought.
        let m = keeping();
        assert!(m.owed(), "a pass the brain stopped in is not owed");
        m.started(200, &ctx);
        let pass = m.pass(200, &ctx).expect("a pass");
        assert_eq!(pass.after, Some(Cursor("cursor-1".to_owned())));
        assert_eq!(pass.until, Some(Cursor("cursor-5".to_owned())));
        let shares: Vec<Entry> = journal[1..4]
            .iter()
            .map(|line| Entry::parse(line).expect("a line"))
            .collect();
        let last = shares[2].cursor.clone();
        m.passed(
            Passed {
                shares: &[(0, shares)],
                lost: 0,
                last: Some(&last),
                why: None,
                ahead: false,
            },
            &ctx,
        );
        assert!(!m.owed());
        assert_eq!(count_of(&m, "broken"), 6, "a line lost, or counted twice");
        assert_eq!(
            m.state().left_at,
            Some(Cursor("cursor-6".to_owned())),
            "the place is not the follower's"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a pass that read past every line the follower brought while
    /// it ran leaves the place at the follower's line. The core skips what
    /// the follower brings up to where the pass ended, so nothing else moves
    /// it, and a read after the module goes off counts those lines again.
    #[test]
    fn a_pass_past_the_follower_takes_the_place() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        m.started(100, &ctx);
        let journal: Vec<String> = (1..=3)
            .map(|n| written(n, 3, "a.service", "broken"))
            .collect();
        let pass = m.pass(100, &ctx).expect("a pass");
        assert_eq!(pass.after, None);
        // THE FOLLOWER BRINGS THE FIRST LINE while the pass reads all three;
        // the core hands the pass's share without it.
        hear(&m, &ctx, &journal[0]);
        let rest: Vec<Entry> = journal[1..]
            .iter()
            .map(|line| Entry::parse(line).expect("a line"))
            .collect();
        let last = rest[1].cursor.clone();
        m.passed(
            Passed {
                shares: &[(0, rest)],
                lost: 0,
                last: Some(&last),
                why: None,
                ahead: true,
            },
            &ctx,
        );
        assert_eq!(m.state().left_at, Some(last), "the place stayed behind");
        // OFF, AND READ AGAIN: nothing it holds is counted twice.
        m.stopped(101, &ctx);
        read(&m, &ctx, &journal);
        assert_eq!(count_of(&m, "broken"), 3, "a line came twice");
    }

    #[test]
    fn an_empty_pass_and_a_failed_one_keep_the_place_and_a_third_read_brings_nothing() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let m = JournalErrors::keeping(
            Some(state_dir.clone()),
            Some(config_dir.clone()),
            BOOT.to_owned(),
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        let journal = vec![
            written(1, 3, "a.service", "broken"),
            written(2, 3, "a.service", "broken"),
        ];
        read(&m, &ctx, &journal);
        let place = Some(Cursor("cursor-2".to_owned()));
        assert_eq!(m.state().left_at, place);
        // NOTHING NEW: `last` is `None`.
        read(&m, &ctx, &journal);
        assert_eq!(m.state().left_at, place, "an empty pass wiped the place");
        // AND A PASS THAT FAILED, which read nothing either.
        m.pass(100, &ctx);
        m.passed(
            Passed {
                shares: &[],
                lost: 0,
                last: None,
                why: Some(Refusal::Unreadable),
                ahead: false,
            },
            &ctx,
        );
        assert_eq!(m.state().left_at, place, "a failed pass wiped the place");
        // THE THIRD READ brings nothing: wiped, it would count the boot again.
        read(&m, &ctx, &journal);
        assert_eq!(count_of(&m, "broken"), 2);
        // THE PLACE IS IN THE FILE, for a brain that starts again.
        let again = JournalErrors::keeping(Some(state_dir), Some(config_dir), BOOT.to_owned());
        assert_eq!(again.state().left_at, place);
        read(&again, &ctx, &journal);
        assert_eq!(count_of(&again, "broken"), 2, "a restart read it all again");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_pass_cut_short_keeps_what_it_read_and_goes_on_from_there() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        let journal: Vec<String> = (1..=3)
            .map(|n| written(n, 3, "a.service", "broken"))
            .collect();
        let got = Entry::parse(&journal[0]).expect("a line");
        let last = got.cursor.clone();
        m.pass(100, &ctx);
        m.passed(
            Passed {
                shares: &[(0, vec![got])],
                lost: 0,
                last: Some(&last),
                why: Some(Refusal::Unreadable),
                ahead: false,
            },
            &ctx,
        );
        assert_eq!(m.state().left_at, Some(last), "what it read is read");
        read(&m, &ctx, &journal);
        assert_eq!(count_of(&m, "broken"), 3, "a line came twice, or none");
    }

    #[test]
    fn stopping_writes_what_waited_because_off_has_no_wake() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let m = JournalErrors::keeping(
            Some(state_dir.clone()),
            Some(config_dir.clone()),
            BOOT.to_owned(),
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        m.started(100, &ctx);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        assert!(m.next_wake(100).is_some());
        m.stopped(101, &ctx);
        assert_eq!(m.next_wake(101), None, "the clock stayed armed off");
        let again = JournalErrors::keeping(Some(state_dir), Some(config_dir), BOOT.to_owned());
        assert_eq!(marks(&again)[0].count, 1, "what waited was lost");
        let place = again.state().left_at.clone();
        assert_eq!(place, Some(kept_cursor(fixtures::NGINX_BIND_80)));
        let _ = std::fs::remove_dir_all(&root);
    }

    fn kept_cursor(line: &str) -> Cursor {
        Entry::parse(line).expect("a line").cursor
    }

    #[test]
    fn the_journals_cursor_never_gets_ahead_of_the_list() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let m = JournalErrors::keeping(
            Some(state_dir.clone()),
            Some(config_dir.clone()),
            BOOT.to_owned(),
        );
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        // THE CORE IS ABOUT TO WRITE THE CURSOR, long before the wake.
        m.keep(101, &ctx);
        assert_eq!(m.next_wake(101), None, "nothing is left to write");
        let again = JournalErrors::keeping(Some(state_dir), Some(config_dir), BOOT.to_owned());
        assert_eq!(
            marks(&again)[0].count,
            1,
            "the cursor passed a line the list lost"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_cursor_takes_a_quiet_line_to_the_file_and_the_page_keeps_its_clock() {
        let root = scratch(line!());
        let (state_dir, config_dir) = (root.join("state"), root.join("config"));
        let file = state_dir.join(LIST_FILE);
        let m = JournalErrors::keeping(
            Some(state_dir.clone()),
            Some(config_dir.clone()),
            BOOT.to_owned(),
        );
        let (ctx, kept) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::WARNING);
        // THE CORE IS ABOUT TO WRITE THE CURSOR, long before the wake.
        m.keep(101, &ctx);
        assert!(kept.changes().is_empty(), "a cursor published the mind");
        assert_eq!(
            m.next_wake(101),
            Some(100 + QUIET_AFTER_S),
            "the page lost its clock"
        );
        let again = JournalErrors::keeping(Some(state_dir), Some(config_dir), BOOT.to_owned());
        assert_eq!(
            again.state().book.warnings.len(),
            1,
            "the cursor passed a line the list lost"
        );
        // ANOTHER MODULE'S LINE MOVES THE CURSOR TOO, and nothing here moved.
        std::fs::remove_file(&file).expect("remove");
        m.keep(102, &ctx);
        assert!(!file.exists(), "a cursor wrote a list that had not moved");
        // THE WAKE TELLS THE PAGE, and the file is already there.
        let (wake, kept) = Ctx::kept(super::super::ID, 100 + QUIET_AFTER_S);
        m.heard(Heard::Wake, &wake);
        assert_eq!(kept.changes().len(), 1, "the page never heard of the line");
        assert!(!file.exists(), "the wake wrote a list that had not moved");
        assert_eq!(m.next_wake(200), None);
        // A LINE AFTER THE CURSOR is the wake's to write.
        hear(&m, &wake, fixtures::WARNING);
        let (wake, _) = Ctx::kept(super::super::ID, 200 + QUIET_AFTER_S);
        m.heard(Heard::Wake, &wake);
        assert!(file.exists(), "the wake lost a line no cursor took");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_line_that_moves_no_count_waits_half_a_minute_and_publishes_once() {
        let m = errors();
        let (ctx, kept) = Ctx::kept(super::super::ID, 100);
        for n in 0..50 {
            hear(&m, &ctx, &written(n, 4, "a.service", "slow"));
            hear(
                &m,
                &ctx,
                &written(n, 3, "b.service", "Ignoring duplicate name 'x'"),
            );
        }
        assert_eq!(
            m.next_wake(100),
            Some(100 + QUIET_AFTER_S),
            "no five-second clock"
        );
        assert!(kept.changes().is_empty(), "each line published the mind");
        // THE WAKE WRITES, AND SAYS SO ONCE, for the page.
        m.heard(
            Heard::Wake,
            &Ctx::kept(super::super::ID, 100 + QUIET_AFTER_S).0,
        );
        assert_eq!(m.next_wake(200), None);
        // AND AN ERROR HURRIES WHAT WAITS.
        let (ctx, kept) = Ctx::kept(super::super::ID, 300);
        hear(&m, &ctx, &written(60, 4, "a.service", "slow"));
        assert_eq!(m.next_wake(300), Some(300 + QUIET_AFTER_S));
        hear(&m, &ctx, &written(61, 3, "c.service", "broken"));
        assert_eq!(m.next_wake(300), Some(300 + WRITE_AFTER_S));
        assert_eq!(kept.changes().len(), 1, "the error did not publish at once");
    }

    #[test]
    fn a_quiet_wake_tells_the_page() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::WARNING);
        let generation = m.face(&look()).screen;
        let (wake, kept) = Ctx::kept(super::super::ID, 100 + QUIET_AFTER_S);
        m.heard(Heard::Wake, &wake);
        assert_eq!(
            kept.changes().len(),
            1,
            "the page never heard of the warning"
        );
        assert!(m.face(&look()).screen > generation);
        // A WAKE WITH NOTHING TO DO SAYS NOTHING.
        m.heard(Heard::Wake, &wake);
        assert_eq!(kept.changes().len(), 1);
    }

    #[test]
    fn a_critical_not_seen_turns_the_mark_and_seen_it_goes_back() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::NGINX_BIND_80);
        assert_eq!(marks(&m)[0].tone, Tone::Watching);
        hear(&m, &ctx, fixtures::COREDUMP_NO_MESSAGE);
        assert_eq!(marks(&m)[0].tone, Tone::Critical);
        assert_eq!(marks(&m)[0].count, 2);
        let crash = kind_of(fixtures::COREDUMP_NO_MESSAGE).id();
        assert_eq!(m.act(&act_on("seen", Some(&crash)), &ctx), Acted::Done);
        assert_eq!(marks(&m)[0].tone, Tone::Watching, "a critical already seen");
        assert_eq!(marks(&m)[0].count, 1);
    }

    #[test]
    fn a_worth_a_look_warning_counts_on_the_mark_and_a_plain_one_does_not() {
        let m = errors();
        let (ctx, _) = Ctx::kept(super::super::ID, 100);
        hear(&m, &ctx, fixtures::UNIT_RESULT);
        assert_eq!(marks(&m)[0].count, 1);
        let screen = m.screen(&look());
        let row = &section(&screen, "unseen").expect("the list").rows[0];
        assert_eq!(row.tone, Tone::Warning);
        hear(&m, &ctx, fixtures::WARNING);
        assert_eq!(marks(&m)[0].count, 1, "a plain warning reached the mark");
    }
}
