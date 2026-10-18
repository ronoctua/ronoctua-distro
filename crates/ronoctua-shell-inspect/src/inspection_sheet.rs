//! WHAT IS KNOWN ABOUT ONE FILE, as rows already in words — the sheet the
//! info panel draws.
//!
//! # Why it is a module of its own
//!
//! Because the sheet is the half of the panel that has nothing to do with
//! drawing: a list of [`Fact`]s, each a label, a value and what acting on it
//! reaches. The manager's panel beside the listing, its strip along the
//! bottom and the modal sheet the menu's *information* row opens all read it,
//! and so will every other door that shows a file without a listing — the
//! search's, the picker's. One builder, because a second builder is a second
//! answer to "what does this file say".
//!
//! # Two halves, in the reading order
//!
//! [`fs_facts`] is what the FILESYSTEM knows — the path, the type, the mode,
//! the owner, the size, the date — and [`content_facts`] is what the CONTENT
//! says, out of the inspector's answer (`ronoctua_inspect::facts`). One
//! answers "which file is this", the other "what is inside it", and
//! [`sheet_of`] puts the second after the first only when the answer is about
//! the same file.

use ronoctua_files::{Kind, mode::Explanation, when::Zone};

use crate::fields::Doable;
use crate::inspection::{Looked, Subject};

/// One row of the info panel: what it says, and what it offers.
///
/// The label and the value are already WORDS by the time they are here —
/// [`sheet_of`] resolves them through the panel's own bundle — because a `Choice`
/// and a filename follow the same rule: text on screen is text, and a key that
/// reached the screen is the defect invariant 8 exists to refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    /// What it is called.
    pub label: String,
    /// What it says.
    pub value: String,
    /// What acting on it reaches — and [`Doable::Nothing`] for the facts one
    /// only reads.
    pub does: Doable,
    /// WHAT THIS ROW SAYS UNDER THE POINTER, when `label: value` is not the
    /// honest answer.
    ///
    /// A date is the case that made it exist: the column has room for
    /// `2026-09-03 12:45` and the question somebody asks by pointing at it is
    /// *which second, and in whose time*. Every other row leaves it `None` and
    /// gets the generic sentence, which carries both halves un-elided and is
    /// the right answer for a path or a size.
    ///
    /// It is NOT a `Doable`: that enum answers "what can be DONE here", and a
    /// date is not something one edits — a row that lit up and then refused
    /// would be worse than one that never promised.
    pub says: Option<Explanation>,
}

/// WHAT A REMEMBERED SHEET WAS BUILT FOR.
///
/// Everything here can change what the sheet says; nothing here changes without
/// the sheet needing to be built again. See `inspection::Panel::remembered`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SheetFor {
    /// The entry the sheet is about.
    pub(crate) name: String,
    /// Its mtime, because a file rewritten in place keeps its name.
    pub(crate) modified: i64,
    /// What a directory measured to, when one has been.
    pub(crate) measured: Option<u64>,
    /// Which inspection has come back for it.
    pub(crate) looked: Option<String>,
    /// Whether that inspection's SECOND answer — what can be changed — has
    /// come back too: it arrives under the same `looked` key, and a sheet
    /// remembered from before it would go on drawing the facts without their
    /// editable rows.
    pub(crate) changeable: bool,
    /// Which language it was written in, by the identity of the bundle.
    pub(crate) said: usize,
}

/// A fact's value, in words.
///
/// TWO SHAPES, and the split is not tidiness. A camera's model is `Pixel 8 Pro`
/// in every language and translating it would be wrong — the same rule a
/// `Choice`'s label already follows, because a filename is text and never a
/// key. A frame rate is not: where the unit goes and what two channels are
/// called are the LANGUAGE's business, and that belongs in the `.ftl`
/// (invariant 8).
///
/// # Why an argument called `n` crosses as a NUMBER
///
/// Because `files-value-channels` and `files-value-pages` are Fluent
/// SELECTORS, and `facts::Value::Phrase` carries its arguments as `String`.
/// Given the string `"1"`, Fluent looks for a variant literally named `1` and
/// falls through to `*[other]`: the screen says **"1 páginas"** with every
/// check green. It is verbatim the defect `ronoctua_i18n::Arg::Number`'s own
/// docstring exists for.
///
/// The rule is the argument's NAME and not a list of the two keys, so a third
/// selector arriving later is right by construction — and it is `n` and not
/// "anything that parses as an integer" on purpose: Fluent groups a number by
/// locale, so `iso` would become `1,600` and a photograph's dimensions
/// `3,024 × 4,032`.
pub(crate) fn fact_said(
    value: &ronoctua_inspect::facts::Value,
    texts: &ronoctua_i18n::Texts,
) -> String {
    use ronoctua_i18n::Arg;
    use ronoctua_inspect::facts::Value;
    match value {
        Value::Data(text) => text.clone(),
        Value::Phrase { key, args } => {
            let crossing: Vec<(&str, Arg<'_>)> = args
                .iter()
                .map(|(k, v)| {
                    // WHICH NAMES CARRY NUMBERS, written down. `n` is the
                    // count a plural selects on; `w`, `d` and `h` are the
                    // three sides of a shape's box. Everything else is data —
                    // a camera model, a date, a licence — and handing THOSE to
                    // a number formatter would group a year into `2.025`.
                    if !matches!(*k, "n" | "w" | "d" | "h") {
                        return (*k, Arg::Text(v.as_str()));
                    }
                    if let Ok(n) = v.parse::<i64>() {
                        (*k, Arg::Number(n))
                    } else if let Ok(n) = v.parse::<f64>() {
                        (*k, Arg::Fraction(n))
                    } else {
                        (*k, Arg::Text(v.as_str()))
                    }
                })
                .collect();
            texts.t_args(key, &crossing)
        }
    }
}

/// What the FILESYSTEM knows about one subject, as [`Fact`]s already in
/// words: the path, the type, the mode, the owner, the size, the date, how
/// many names it has — and where a link goes.
///
/// **A fact with no answer is ABSENT.** Never a row saying "unknown": a panel
/// of unknowns is a panel nobody reads twice.
#[must_use]
pub fn fs_facts(subject: &Subject, zone: &Zone, texts: &ronoctua_i18n::Texts) -> Vec<Fact> {
    let entry = &subject.entry;
    let path = &subject.path;
    let (named, sniffed) = ronoctua_files::mime::examined(path, entry.kind);
    let owner = &subject.owner;
    let group = &subject.group;
    let measured = subject
        .measured
        .map(|b| crate::sizes::size_text(b, texts.decimal_mark()));

    // WHAT EACH ROW OFFERS IS DECIDED HERE, once, beside the value it is
    // about — and not in the drawing, which would have to work out what
    // kind of fact it was holding from the text of it.
    let mut lines: Vec<(&'static str, String, Doable, Option<Explanation>)> = vec![
        // THE NAME, which is the one row a person most often wants to change —
        // and the way out (`..`) is not a name anything may be done to.
        (
            crate::key::FILES_DETAIL_NAME,
            entry.shown().to_owned(),
            if entry.is_parent() {
                Doable::Nothing
            } else {
                Doable::Rename
            },
            None,
        ),
        (
            crate::key::FILES_DETAIL_PATH,
            path.to_string_lossy().into_owned(),
            Doable::Nothing,
            None,
        ),
        (
            crate::key::FILES_DETAIL_TYPE,
            match sniffed {
                Some(found) if found != named => format!("{named} / {found}"),
                _ => named.to_owned(),
            },
            Doable::Nothing,
            None,
        ),
        (
            crate::key::FILES_DETAIL_MODE,
            format!(
                "{} ({})",
                ronoctua_files::mode::shown(entry.mode),
                ronoctua_files::mode::octal(entry.mode)
            ),
            Doable::Mode,
            None,
        ),
        (
            crate::key::FILES_DETAIL_OWNER,
            format!("{owner}:{group}"),
            Doable::Ownership,
            None,
        ),
        (
            crate::key::FILES_DETAIL_SIZE,
            measured.unwrap_or_else(|| crate::sizes::size_text(entry.bytes, texts.decimal_mark())),
            // A DIRECTORY'S SIZE IS A WALK and a file's is already known:
            // one of the two is a thing to ask for and the other is a
            // thing to read. See `measure_field`.
            if matches!(entry.kind, Kind::Directory) {
                Doable::Measure
            } else {
                Doable::Nothing
            },
            None,
        ),
        (
            crate::key::FILES_DETAIL_MODIFIED,
            crate::chrome::when_text(entry.modified, zone, false, texts),
            // A DATE ONE CAN CORRECT: a photograph copied off a phone arrives
            // with today's, and the question offers the full date, seconds and
            // all, in the one shape it reads back.
            if entry.is_parent() {
                Doable::Nothing
            } else {
                Doable::Modified
            },
            // THE SAME TWO KEYS THE LISTING'S COLUMN USES, so the column
            // and the panel cannot come to disagree about a date — which
            // is the argument `chrome::when_text` opens with, one panel
            // along.
            Some(Explanation {
                key: if zone.is_utc() {
                    crate::key::FILES_EXPLAIN_WHEN_UTC
                } else {
                    crate::key::FILES_EXPLAIN_WHEN
                },
                args: vec![(
                    "when",
                    crate::chrome::when_long(entry.modified, zone, texts),
                )],
            }),
        ),
        (
            crate::key::FILES_DETAIL_LINKS,
            entry.links.to_string(),
            Doable::Nothing,
            None,
        ),
    ];
    // A LINK SAYS WHERE IT GOES, and whether it goes anywhere at all.
    if let Some(target) = &entry.target {
        lines.push((
            if entry.broken() {
                crate::key::FILES_DETAIL_BROKEN
            } else {
                crate::key::FILES_DETAIL_TARGET
            },
            target.clone(),
            Doable::Nothing,
            None,
        ));
    }
    lines
        .into_iter()
        .map(|(k, value, does, says)| Fact {
            label: texts.t(k),
            value,
            does,
            says,
        })
        .collect()
}

/// What the TOOLS said is inside the file — the inspector's answer, as
/// [`Fact`]s already in words — then what can be CHANGED in it (a song's
/// tags, a picture's details: `inspection_edit::rows`), and where its
/// coordinate is, in words too.
///
/// A photograph's ISO and a video's frame rate are what the file IS, and
/// nothing a panel does could change one: those stay facts. A read-only row
/// that says what an editable row says gives its place to that row
/// (`inspection_edit::shadowed`) — two rows about one title, one that can be
/// pressed and one that cannot, would be two answers to "can I change this".
/// And the PLACE can be taken out, so its two rows offer that.
#[must_use]
pub fn content_facts(looked: &Looked, texts: &ronoctua_i18n::Texts) -> Vec<Fact> {
    // WHAT CAN BE CHANGED COMES FIRST, right after what the filesystem knows:
    // a photograph's title and author are what a person opens this panel to
    // correct, and a camera answers with two hundred tags of its own that
    // would push them off the bottom.
    let mut out: Vec<Fact> = crate::inspection_edit::rows(&looked.amendable, texts);
    out.extend(
        looked
            .facts
            .iter()
            .filter(|f| !crate::inspection_edit::shadowed(&looked.amendable, f))
            .map(|f| Fact {
                // A KEY IS TRANSLATED AND A NAME IS NOT, which is the whole
                // of `Label`: a row this system decided to show has a sentence
                // in both languages, and a tag the FILE chose to write has the
                // name the file wrote.
                label: match &f.label {
                    ronoctua_inspect::facts::Label::Key(k) => texts.t(k),
                    ronoctua_inspect::facts::Label::Named(name) => name.clone(),
                },
                value: fact_said(&f.value, texts),
                does: if f.label
                    == ronoctua_inspect::facts::Label::Key(ronoctua_inspect::key::FACT_PLACE)
                {
                    Doable::RemoveLocation
                } else {
                    Doable::Nothing
                },
                // THE GENERIC SENTENCE, which carries both halves un-elided.
                // A tool's fact is a label and a value and nothing more; the
                // one row that needs its own sentence is a date, and the dates
                // this panel shows are assembled in `fs_facts`.
                says: None,
            }),
    );

    // AND WHERE THAT COORDINATE IS, in words.
    //
    // A row of its own and not a longer coordinate: the numbers say
    // where on the Earth, and this says where among PEOPLE — which is
    // the question anybody actually has when they look at a
    // photograph. It is offline arithmetic over a table this machine
    // carries; the glance's refusal of a geocoder was about the
    // NETWORK, and this one opens no socket.
    //
    // Absent on a machine with no world data, WITH A SENTENCE: the
    // installer and the rescue console have none, and a row that
    // simply vanished would read as a file that carries no place.
    if let Some(at) = looked.at {
        let value = crate::globe::address_of(at, texts).map_or_else(
            || {
                if crate::globe::present() {
                    texts.t(crate::key::GLOBE_ADDRESS_NOWHERE)
                } else {
                    texts.t(crate::key::ATLAS_ABSENT)
                }
            },
            |found| crate::globe::said(&found, texts),
        );
        out.push(Fact {
            label: texts.t(crate::key::FILES_FACT_WHERE),
            value,
            does: Doable::RemoveLocation,
            says: None,
        });
    }
    out
}

/// Everything known about one subject, as [`Fact`]s already in words.
///
/// # Why ONE builder for THREE homes
///
/// Because the right-hand column, the bottom strip and the modal sheet the
/// menu's *information* row opens are three places showing the same list,
/// and a second builder is a second answer to "what does this file say".
/// It is the same argument that keeps the two doors on one `View`.
///
/// What the FILESYSTEM knows comes first — the path, the type, the mode,
/// the owner, the size, the date — and what the CONTENT says follows, from
/// `ronoctua_inspect::facts`. The order is the reading order: one answers
/// "which file is this", the other "what is inside it".
///
/// **A fact with no answer is ABSENT.** Never a row saying "unknown": a
/// panel of unknowns is a panel nobody reads twice, and the crate that
/// produces them already refuses to emit one.
///
/// # And what the tools said, only when they were asked about THIS file
///
/// The path is compared and not only the name: the cursor moves while a child
/// process runs, and facts about the previous file under the current file's
/// name is the one failure this panel must not have.
#[must_use]
pub fn sheet_of(
    subject: &Subject,
    looked: Option<&Looked>,
    zone: &Zone,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Fact> {
    let mut out = fs_facts(subject, zone, texts);
    if let Some(l) = looked.filter(|l| l.path == subject.path) {
        out.extend(content_facts(l, texts));
    }
    out
}
