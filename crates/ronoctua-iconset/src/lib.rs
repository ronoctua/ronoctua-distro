//! The icons: Nerd Fonts glyphs, Lucide figures and Unicode characters.
//!
//! # One library for the three collections
//!
//! The icons app (`icons.rs`, `icons_page.rs`) and fast-access's `/g` row read
//! the same three tables, search them with ONE [`search`] (it was three copies,
//! one per buffer), and copy with the same formats ([`copy`]). Lucide's
//! adjustments and SVG are [`lucide`].
//!
//! # Read at run time, held while shown
//!
//! `glyphs.json`, `lucide.json` and `unicode.json` — the three generators'
//! outputs, symlinked into one directory by `nix/iconset.nix` — are read by
//! path from [`VARIABLE`] (`atRunTime` in `nix/rust.nix`, never `generated`:
//! no `include_str!`, no `option_env!`). [`shared`] hands out one [`Set`]
//! behind a `Weak`, so the ~9 MB exist while a view holds them and die with
//! the last one: the footer lives for months.
//!
//! # Search terms are folded and indexed at load
//!
//! Every name, alias, set, category and tag is folded (lowercase, no
//! accents) and split into words ONCE, when the set is built; each word is an
//! id into one sorted vocabulary. A keystroke folds only what was typed, turns
//! each word into the range of vocabulary ids it prefixes, and asks each
//! record a few binary searches — no allocation per record, within the 2 ms a
//! fast-access strip has. Both sides folded: `atenção` finds the `atencao` a
//! table wrote, and the other way round.
//!
//! # The search language is interface
//!
//! `family:letter`, `bloco:arrows`, `md:` are what a person TYPES, so the
//! prefix words and the enumeration values come from the `.ftl` of every
//! language ([`Grammar::new`]) — never from a table written here. The set
//! prefixes (`md:`, `fa:`) come from the Nerd Fonts table itself, so a new set
//! of the next release needs no code.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use ronoctua_i18n::{LANGUAGES, Texts, key};
use serde_json::{Map, Value};

pub mod copy;
pub mod lucide;

/// The variable that names the directory of the three tables.
pub const VARIABLE: &str = "RONOCTUA_ICONSET";
/// The Nerd Fonts table's file name.
pub const NERD_FILE: &str = "glyphs.json";
/// Lucide's table's file name.
pub const LUCIDE_FILE: &str = "lucide.json";
/// The Unicode table's file name.
pub const UNICODE_FILE: &str = "unicode.json";

/// One of the three collections — the app's three screens, fast-access's
/// three strips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Collection {
    /// Nerd Fonts glyphs (the default screen).
    NerdFonts,
    /// Lucide figures.
    Lucide,
    /// Unicode characters, emoji included.
    Unicode,
}

impl Collection {
    /// The three, in the tabs' order.
    pub const ALL: [Self; 3] = [Self::NerdFonts, Self::Lucide, Self::Unicode];

    /// Its kebab-case name, the suffix of its key (`icons-screen-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NerdFonts => "nerd-fonts",
            Self::Lucide => "lucide",
            Self::Unicode => "unicode",
        }
    }
}

/// A Nerd Fonts glyph (`glyphs.json`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Nerd {
    /// Its codepoint.
    pub cp: u32,
    /// Its primary name (`md-home`), when it has one.
    pub name: Option<String>,
    /// Every name of the codepoint, the primary first.
    pub names: Vec<String>,
    /// The sets it belongs to (`md`, `fa`, `cod`…).
    pub sets: Vec<String>,
    /// Its tags per language subtag, accented as a screen shows them.
    pub tags: BTreeMap<String, Vec<String>>,
    /// Every tag folded (no accents, lowercase) — what a search compares.
    pub search: Vec<String>,
    /// The design-system icon token that uses it, when one does.
    pub token: Option<String>,
    /// Whether an app id of the tray is assigned to it.
    pub in_tray: bool,
}

/// A Lucide figure (`lucide.json`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lucide {
    /// Its name (`house`).
    pub name: String,
    /// The codepoint the screen draws: our remapped face, `U+F2000…`.
    pub cp: u32,
    /// The codepoint Lucide publishes (`lcp`) — what a person pasting into a
    /// page with Lucide's own font needs.
    pub lcp: u32,
    /// Its categories.
    pub categories: Vec<String>,
    /// Its drawing, encoded as the generator writes it (elements and
    /// attributes with the generator's separators); [`lucide::svg`] reads it.
    pub nodes: String,
    /// Its other names.
    pub aliases: Vec<String>,
    /// Its tags per language subtag.
    pub tags: BTreeMap<String, Vec<String>>,
    /// Every tag folded.
    pub search: Vec<String>,
}

/// A Unicode character or emoji sequence (`unicode.json`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Char {
    /// Its first codepoint.
    pub cp: u32,
    /// The whole sequence (one codepoint for most; several for an emoji).
    pub seq: Vec<u32>,
    /// What is drawn: a combining mark is shown on a dotted circle, and COPIED
    /// alone.
    pub shows: Vec<u32>,
    /// Its official name.
    pub name: String,
    /// Its other names.
    pub aliases: Vec<String>,
    /// Its general category (`Sm`, `Lu`).
    pub category: String,
    /// One of the seven families (`letter`, `symbol`, `emoji`…).
    pub family: String,
    /// Its block's name.
    pub block: String,
    /// Its canonical decomposition, hexadecimal; empty when none.
    pub decomposition: String,
    /// The emoji group, for an emoji.
    pub group: Option<String>,
    /// The emoji subgroup, for an emoji.
    pub subgroup: Option<String>,
    /// The skin tone, for an emoji that has one.
    pub tone: Option<String>,
    /// How many cells it takes: 0 (combining), 1 or 2.
    pub width: u8,
    /// Its tags per language subtag.
    pub tags: BTreeMap<String, Vec<String>>,
    /// Every tag folded.
    pub search: Vec<String>,
}

impl Char {
    /// The text a copy puts on the clipboard: the whole sequence, and never
    /// the dotted circle a combining mark is drawn on.
    #[must_use]
    pub fn text(&self) -> String {
        self.seq.iter().filter_map(|c| char::from_u32(*c)).collect()
    }

    /// The text the grid draws (a combining mark on its dotted circle).
    #[must_use]
    pub fn drawn(&self) -> String {
        self.shows
            .iter()
            .filter_map(|c| char::from_u32(*c))
            .collect()
    }

    /// How many cells of INK it takes at a scale worth `cells` cells: the
    /// width the generator measured from the font (at least one — a mark is
    /// drawn on its circle), times the scale.
    #[must_use]
    pub fn ink(&self, cells: u16) -> u16 {
        u16::from(self.width.max(1)).saturating_mul(cells)
    }
}

/// The three tables.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Set {
    /// The Nerd Fonts glyphs, by codepoint.
    pub nerd: Vec<Nerd>,
    /// The Lucide figures, by name.
    pub lucide: Vec<Lucide>,
    /// The Unicode characters, by codepoint.
    pub unicode: Vec<Char>,
    /// The search index, built once: by [`load`], or by the first search of
    /// a set built by hand. A table changed after that is not re-indexed —
    /// build a new set with [`Set::new`].
    index: OnceLock<Index>,
}

impl Set {
    /// A set of these tables, indexed for [`search`] now.
    #[must_use]
    pub fn new(nerd: Vec<Nerd>, lucide: Vec<Lucide>, unicode: Vec<Char>) -> Self {
        let set = Self {
            nerd,
            lucide,
            unicode,
            index: OnceLock::new(),
        };
        set.index();
        set
    }

    fn index(&self) -> &Index {
        self.index.get_or_init(|| Index::build(self))
    }

    /// How many records a collection has.
    #[must_use]
    pub const fn len(&self, collection: Collection) -> usize {
        match collection {
            Collection::NerdFonts => self.nerd.len(),
            Collection::Lucide => self.lucide.len(),
            Collection::Unicode => self.unicode.len(),
        }
    }

    /// Whether every table is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.nerd.is_empty() && self.lucide.is_empty() && self.unicode.is_empty()
    }

    /// The Nerd Fonts sets, the most populated first (ties by name) — the
    /// panel's order, the same in every language.
    #[must_use]
    pub fn sets(&self) -> Vec<String> {
        counted(self.nerd.iter().flat_map(|g| g.sets.first()))
    }

    /// Lucide's categories, the most populated first (ties by name).
    #[must_use]
    pub fn categories(&self) -> Vec<String> {
        counted(self.lucide.iter().flat_map(|f| &f.categories))
    }
}

/// Distinct words, the most frequent first, ties by the word.
fn counted<'a>(words: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for w in words {
        *count.entry(w.as_str()).or_default() += 1;
    }
    let mut out: Vec<(&str, usize)> = count.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    out.into_iter().map(|(w, _)| w.to_owned()).collect()
}

/// Why there is no set, as a closed set (`icons-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Why {
    /// [`VARIABLE`] is not set: this process was not given the tables.
    NoVariable,
    /// A table is missing or unreadable.
    Unreadable,
    /// A table does not parse as its generator writes it.
    Garbled,
}

impl Why {
    /// Every refusal, for a test that walks the words.
    pub const ALL: [Self; 3] = [Self::NoVariable, Self::Unreadable, Self::Garbled];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoVariable => "no-variable",
            Self::Unreadable => "unreadable",
            Self::Garbled => "garbled",
        }
    }
}

/// Why, and which table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Refusal {
    /// Why.
    pub why: Why,
    /// The file it concerns, when one.
    pub file: Option<&'static str>,
}

impl Refusal {
    /// A refusal that names no file.
    #[must_use]
    pub const fn new(why: Why) -> Self {
        Self { why, file: None }
    }

    /// A refusal about one table.
    #[must_use]
    pub const fn of(why: Why, file: &'static str) -> Self {
        Self {
            why,
            file: Some(file),
        }
    }
}

/// The three tables in `dir`.
///
/// # Errors
///
/// [`Why::Unreadable`] or [`Why::Garbled`], with the file.
pub fn load(dir: &Path) -> Result<Set, Refusal> {
    let nerd = records(dir, NERD_FILE, nerd_of)?;
    let lucide = records(dir, LUCIDE_FILE, lucide_of)?;
    let unicode = records(dir, UNICODE_FILE, char_of)?;
    Ok(Set::new(nerd, lucide, unicode))
}

/// One table: a JSON array of objects, each turned into its record AS IT IS
/// READ — only one object exists as a JSON tree at a time.
fn records<T>(
    dir: &Path,
    file: &'static str,
    of: fn(&Map<String, Value>) -> Option<T>,
) -> Result<Vec<T>, Refusal> {
    use serde::Deserializer as _;
    use serde::de::{Error as _, SeqAccess, Visitor};

    struct Each<T>(fn(&Map<String, Value>) -> Option<T>);

    impl<'de, T> Visitor<'de> for Each<T> {
        type Value = Vec<T>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an array of records")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
            while let Some(record) = seq.next_element::<Map<String, Value>>()? {
                out.push((self.0)(&record).ok_or_else(|| A::Error::custom("a garbled record"))?);
            }
            Ok(out)
        }
    }

    let garbled = |_| Refusal::of(Why::Garbled, file);
    let bytes = std::fs::read(dir.join(file)).map_err(|_| Refusal::of(Why::Unreadable, file))?;
    let mut json = serde_json::Deserializer::from_slice(&bytes);
    let out = json.deserialize_seq(Each(of)).map_err(garbled)?;
    json.end().map_err(garbled)?;
    Ok(out)
}

fn text(r: &Map<String, Value>, field: &str) -> Option<String> {
    r.get(field)?.as_str().map(str::to_owned)
}

/// A text field that may be absent, `null` or empty — all three are "none".
fn optional(r: &Map<String, Value>, field: &str) -> Option<String> {
    text(r, field).filter(|s| !s.is_empty())
}

fn number(r: &Map<String, Value>, field: &str) -> Option<u32> {
    u32::try_from(r.get(field)?.as_u64()?).ok()
}

/// A list of strings; absent is empty (the generators leave out what is
/// empty, to spend no bytes on it).
fn words(r: &Map<String, Value>, field: &str) -> Option<Vec<String>> {
    match r.get(field) {
        None | Some(Value::Null) => Some(Vec::new()),
        Some(Value::Array(a)) => a.iter().map(|v| v.as_str().map(str::to_owned)).collect(),
        Some(_) => None,
    }
}

fn numbers(r: &Map<String, Value>, field: &str) -> Option<Vec<u32>> {
    match r.get(field) {
        None | Some(Value::Null) => Some(Vec::new()),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| v.as_u64().and_then(|n| u32::try_from(n).ok()))
            .collect(),
        Some(_) => None,
    }
}

/// Every `tags_<subtag>` field, by subtag: the generators write one per
/// language of `assets/i18n/languages.nix`, and a third language needs no
/// code here.
fn tags(r: &Map<String, Value>) -> Option<BTreeMap<String, Vec<String>>> {
    r.keys()
        .filter_map(|k| k.strip_prefix("tags_"))
        .map(|subtag| Some((subtag.to_owned(), words(r, &format!("tags_{subtag}"))?)))
        .collect()
}

fn nerd_of(r: &Map<String, Value>) -> Option<Nerd> {
    Some(Nerd {
        cp: number(r, "cp")?,
        name: optional(r, "name"),
        names: words(r, "names")?,
        sets: words(r, "sets")?,
        tags: tags(r)?,
        search: words(r, "search")?,
        token: optional(r, "token"),
        in_tray: r.get("in_tray").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn lucide_of(r: &Map<String, Value>) -> Option<Lucide> {
    Some(Lucide {
        name: text(r, "name")?,
        cp: number(r, "cp")?,
        lcp: number(r, "lcp")?,
        categories: words(r, "cats")?,
        nodes: text(r, "nodes")?,
        aliases: words(r, "aliases")?,
        tags: tags(r)?,
        search: words(r, "search")?,
    })
}

fn char_of(r: &Map<String, Value>) -> Option<Char> {
    let cp = number(r, "cp")?;
    let name = text(r, "name")?;
    let aliases = words(r, "aliases")?;
    // ABSENT MEANS THE CODEPOINT ALONE, and absent width means one: the
    // generator writes both only when they are not the ordinary answer.
    let seq = Some(numbers(r, "seq")?)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| vec![cp]);
    let width = match r.get("width") {
        None => 1,
        Some(v) => u8::try_from(v.as_u64()?).ok()?,
    };
    Some(Char {
        cp,
        seq,
        shows: numbers(r, "shows")?,
        category: text(r, "cat").unwrap_or_default(),
        family: text(r, "family")?,
        block: text(r, "block").unwrap_or_default(),
        decomposition: text(r, "decomp").unwrap_or_default(),
        group: optional(r, "group"),
        subgroup: optional(r, "subgroup"),
        tone: optional(r, "tone"),
        width,
        tags: tags(r)?,
        search: words(r, "search")?,
        name,
        aliases,
    })
}

/// The one set of this process, loaded from [`VARIABLE`] the first time a
/// view asks, and dropped when the last `Arc` is.
static SHARED: Mutex<Weak<Set>> = Mutex::new(Weak::new());

/// The set this process shares: the one a view already holds, or a fresh
/// load when none does.
///
/// Blocks for the read (a few hundred milliseconds for ~9 MB): a door calls it
/// on a thread, never on its event loop.
///
/// # Errors
///
/// [`Why::NoVariable`], or what [`load`] refuses.
pub fn shared() -> Result<Arc<Set>, Refusal> {
    held(&SHARED, || {
        let dir = std::env::var_os(VARIABLE)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or(Refusal::new(Why::NoVariable))?;
        load(&dir)
    })
}

/// What `slot` still holds, or a fresh `load` it will hold while anyone does.
fn held(
    slot: &Mutex<Weak<Set>>,
    load: impl FnOnce() -> Result<Set, Refusal>,
) -> Result<Arc<Set>, Refusal> {
    let mut slot = slot.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(set) = slot.upgrade() {
        return Ok(set);
    }
    let set = Arc::new(load()?);
    *slot = Arc::downgrade(&set);
    Ok(set)
}

/// What a filter prefix narrows by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterKind {
    /// A Nerd Fonts set (`md:`); the prefix names the set itself.
    Set,
    /// A block: by hexadecimal prefix for glyphs, by name for Unicode.
    Block,
    /// A tag.
    Tag,
    /// Only what is a design-system token.
    Token,
    /// Only what an app id of the tray uses.
    App,
    /// Unicode's opening posture: symbols and numbers.
    Symbols,
    /// A Unicode family.
    Family,
    /// A category: Unicode's general category by prefix, or Lucide's.
    Category,
    /// The official name and aliases only.
    Name,
    /// An emoji group.
    Group,
    /// An emoji subgroup.
    Subgroup,
    /// A skin tone.
    Tone,
}

/// How much text a filter takes after its prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Takes {
    /// Nothing: the prefix is the whole filter (`token:`).
    Nothing,
    /// One word (an enumeration: `family:letter`), and the rest of the query
    /// stays free text — so a screen that opens on a filter can be typed in.
    Word,
    /// The rest of the query (free text: `name:black star`, `block:box
    /// drawing` — a value with a space inside it).
    Rest,
}

impl FilterKind {
    /// How much text it takes.
    #[must_use]
    pub const fn takes(self) -> Takes {
        match self {
            Self::Token | Self::App | Self::Symbols | Self::Set => Takes::Nothing,
            Self::Name | Self::Tag | Self::Block | Self::Subgroup => Takes::Rest,
            Self::Family | Self::Category | Self::Group | Self::Tone => Takes::Word,
        }
    }

    /// The kinds whose prefix is a word of the `.ftl` (`Set`'s prefixes are
    /// the table's set names), with the key each one's word lives under.
    pub const WORDED: [(Self, &'static str); 12] = [
        (Self::Block, key::ICONS_PREFIX_BLOCK),
        (Self::Tag, key::ICONS_PREFIX_TAG),
        (Self::Token, key::ICONS_PREFIX_TOKEN),
        (Self::App, key::ICONS_PREFIX_APP),
        (Self::Symbols, key::ICONS_PREFIX_SYMBOLS),
        (Self::Family, key::ICONS_PREFIX_FAMILY),
        (Self::Category, key::ICONS_PREFIX_CATEGORY),
        // `cat:` is the Unicode general category's own abbreviation, the same
        // three letters in every language — a key all the same, because it
        // is typed.
        (Self::Category, key::ICONS_PREFIX_CAT),
        (Self::Name, key::ICONS_PREFIX_NAME),
        (Self::Group, key::ICONS_PREFIX_GROUP),
        (Self::Subgroup, key::ICONS_PREFIX_SUBGROUP),
        (Self::Tone, key::ICONS_PREFIX_TONE),
    ];
}

/// The Unicode families, as the table writes them (the suffix of
/// `icons-family-<id>` and `icons-value-family-<id>`).
pub const FAMILIES: [&str; 7] = [
    "symbol",
    "letter",
    "number",
    "mark",
    "punctuation",
    "other",
    "emoji",
];

/// The emoji groups, as the table writes them.
pub const GROUPS: [&str; 9] = [
    "faces",
    "people",
    "nature",
    "food",
    "travel",
    "activities",
    "objects",
    "symbols",
    "flags",
];

/// The skin tones, as the table writes them.
pub const TONES: [&str; 5] = ["light", "medium-light", "medium", "medium-dark", "dark"];

/// One prefix a person may type, in some language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prefix {
    /// What is typed, colon included, folded (`familia:`, `family:`, `md:`).
    pub word: String,
    /// What it filters by.
    pub kind: FilterKind,
    /// The value the prefix itself names (the set of `md:`), when it does.
    pub implied: Option<String>,
}

/// A value word of an enumeration filter, in some language: `letra` is the
/// family `letter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    /// The filter it is a value of.
    pub kind: FilterKind,
    /// What is typed, folded.
    pub word: String,
    /// The table's id it stands for.
    pub id: String,
}

/// The prefixes a query understands: the words come from the `.ftl` of every
/// language (the search DSL is interface — ARCHITECTURE.md), the set names
/// from the table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Grammar {
    /// Every prefix, in every language.
    pub prefixes: Vec<Prefix>,
    /// Every value word of the enumeration filters, in every language. A value
    /// that is none of these is taken as it was typed — so the table's own id
    /// always works, and an unknown one finds nothing rather than something.
    pub values: Vec<Named>,
}

impl Grammar {
    /// The grammar of every declared language, for `set`.
    ///
    /// Builds the bundles of every language, which costs a few milliseconds:
    /// a door builds it once when the set arrives, never per keystroke.
    #[must_use]
    pub fn new(set: &Set) -> Self {
        let languages: Vec<Texts> = LANGUAGES.iter().filter_map(|l| Texts::fresh(l)).collect();
        let mut grammar = Self::default();
        for texts in &languages {
            for (kind, k) in FilterKind::WORDED {
                grammar.add_prefix(&texts.t(k), kind, None);
            }
            for (kind, ids, stem) in [
                (FilterKind::Family, &FAMILIES[..], "icons-value-family-"),
                (FilterKind::Group, &GROUPS[..], "icons-value-group-"),
                (FilterKind::Tone, &TONES[..], "icons-value-tone-"),
            ] {
                for id in ids {
                    let word = fold(&texts.t(&format!("{stem}{id}")));
                    let named = Named {
                        kind,
                        word,
                        id: (*id).to_owned(),
                    };
                    if !grammar.values.contains(&named) {
                        grammar.values.push(named);
                    }
                }
            }
        }
        for s in set.sets() {
            grammar.add_prefix(&s, FilterKind::Set, Some(s.clone()));
        }
        grammar
    }

    fn add_prefix(&mut self, word: &str, kind: FilterKind, implied: Option<String>) {
        let word = format!("{}:", fold(word.trim()));
        if word.len() > 1 && !self.prefixes.iter().any(|p| p.word == word) {
            self.prefixes.push(Prefix {
                word,
                kind,
                implied,
            });
        }
    }

    /// The id a typed value stands for: its word in some language, or itself.
    fn value(&self, kind: FilterKind, typed: &str) -> String {
        self.values
            .iter()
            .find(|n| n.kind == kind && n.word == typed)
            .map_or_else(|| typed.to_owned(), |n| n.id.clone())
    }
}

/// Lowercase without accents — the form a prefix and a value are compared in.
#[must_use]
pub fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(ronoctua_apps::fold::folded_char)
        .collect()
}

/// A filter of a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    /// What it narrows by.
    pub kind: FilterKind,
    /// Its value (folded), for a kind that takes one.
    pub value: Option<String>,
}

/// A parsed query: filters, and the words every hit must match.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    /// The words, folded (lowercase, no accents); each must prefix some
    /// term of a hit.
    pub words: Vec<String>,
    /// The filters, in the order typed. Every one must hold.
    pub filters: Vec<Filter>,
    /// The one character — or one emoji sequence, which is one character to
    /// the eye — that is the whole query: the gesture of pasting a character
    /// to learn which it is. The identity rule finds exactly it, first, and
    /// its words still find the rest.
    pub pasted: Option<String>,
}

impl Query {
    /// The query in `text`, by `grammar`'s prefixes.
    #[must_use]
    pub fn parse(text: &str, grammar: &Grammar) -> Self {
        let trimmed = text.trim();
        if one_character(trimmed) {
            return Self {
                words: split_words(&fold(trimmed)),
                filters: Vec::new(),
                pasted: Some(trimmed.to_owned()),
            };
        }
        let mut query = Self::default();
        let mut pieces = trimmed.split_whitespace().peekable();
        while let Some(piece) = pieces.next() {
            let folded = fold(piece);
            let Some(prefix) = grammar
                .prefixes
                .iter()
                .filter(|p| folded.starts_with(&p.word))
                .max_by_key(|p| p.word.len())
            else {
                query.words.extend(split_words(&folded));
                continue;
            };
            let glued = folded[prefix.word.len()..].to_owned();
            let value = match prefix.kind.takes() {
                Takes::Nothing => {
                    // A prefix that takes nothing leaves what is glued to it
                    // as free text: `md:home` is the set and a word.
                    query.words.extend(split_words(&glued));
                    prefix.implied.clone()
                }
                Takes::Word if glued.is_empty() => pieces.next().map(fold),
                Takes::Word => Some(glued),
                Takes::Rest => {
                    let rest: Vec<String> = std::iter::once(glued)
                        .chain(pieces.by_ref().map(fold))
                        .filter(|w| !w.is_empty())
                        .collect();
                    (!rest.is_empty()).then(|| rest.join(" "))
                }
            };
            let value = match prefix.kind.takes() {
                Takes::Word => value.map(|v| grammar.value(prefix.kind, &v)),
                _ => value,
            };
            query.filters.push(Filter {
                kind: prefix.kind,
                value,
            });
        }
        query
    }
}

/// Whether `text` is ONE character to the eye: one codepoint, or an emoji
/// sequence (a base with joiners, modifiers, selectors and the characters a
/// joiner glues on).
fn one_character(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first.is_whitespace() {
        return false;
    }
    let mut glued = false;
    let mut regional = u32::from(regional_indicator(first));
    for c in chars {
        let cp = c as u32;
        let joins = matches!(cp,
            0x200D | 0xFE0E | 0xFE0F | 0x20E3 | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F
        ) || (0x0300..=0x036F).contains(&cp);
        if joins {
            glued = cp == 0x200D;
            continue;
        }
        if glued && !c.is_whitespace() && !c.is_ascii() {
            glued = false;
            continue;
        }
        if regional_indicator(c) && regional == 1 {
            regional = 2;
            continue;
        }
        return false;
    }
    true
}

const fn regional_indicator(c: char) -> bool {
    matches!(c as u32, 0x1F1E6..=0x1F1FF)
}

/// The words of a query or of a value: split on whitespace AND on ASCII
/// punctuation — the same boundary [`words_of`] uses on the text, or
/// `seti-ui` would be one word that matches no tag. ASCII punctuation and not
/// "everything that is not alphanumeric": a pasted `←` is not alphanumeric,
/// and splitting on it would empty the query.
fn split_words(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The words of a text as a search compares them: folded, and split on
/// everything that is not a letter or a digit — a word, because a loose
/// substring put `PESETA SIGN` before every arrow for `seta`.
fn words_of(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

/// The list kinds of a record, in the order [`Records::bounds`] keeps them;
/// the tags follow, one list per subtag of [`Index::subtags`].
const NAME_WORDS: usize = 0;
/// The words of every alias (another name of the same record).
const ALIAS_WORDS: usize = 1;
/// The Nerd Fonts sets, each WHOLE, or the words of Lucide's categories.
const GROUP_WORDS: usize = 2;
/// The words of the record's `search` field.
const SEARCH: usize = 3;
/// The first tag list.
const TAGS: usize = 4;

/// What the records of one collection offer a search, built ONCE at load
/// ([`Set::index`]): every word is an id into the set's sorted vocabulary,
/// and every list is sorted. A keystroke allocates nothing per record — a
/// typed word is a RANGE of ids (the words it prefixes are contiguous in
/// sorted order), and "does this record have a word it prefixes" is one
/// binary search. The lists of all records live in ONE array, record after
/// record, so a search walks memory in order instead of chasing a pointer
/// per list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Records {
    /// Every record's whole name, folded, one after the other.
    names: String,
    /// Where each record's name ends in `names`.
    name_ends: Vec<u32>,
    /// Every list of every record.
    ids: Vec<u32>,
    /// `kinds` ends per record into `ids`, after a leading 0.
    bounds: Vec<u32>,
    /// How many lists a record has: [`TAGS`] plus one per subtag.
    kinds: usize,
    /// The Unicode blocks, lowered (`block:` compares by substring); empty
    /// for the other collections.
    blocks: Vec<Box<str>>,
}

/// One record's terms, borrowed from its [`Records`].
struct Terms<'a> {
    name: &'a str,
    ids: &'a [u32],
    /// The `kinds + 1` bounds of its lists in `ids`.
    cuts: &'a [u32],
    block: &'a str,
}

impl Records {
    fn len(&self) -> usize {
        self.name_ends.len()
    }

    fn terms(&self, i: usize) -> Terms<'_> {
        let start = if i == 0 { 0 } else { self.name_ends[i - 1] };
        Terms {
            name: &self.names[start as usize..self.name_ends[i] as usize],
            ids: &self.ids,
            cuts: &self.bounds[i * self.kinds..=(i + 1) * self.kinds],
            block: self.blocks.get(i).map_or("", |b| b),
        }
    }
}

/// The search index of a [`Set`]: the sorted vocabulary of every folded word
/// of the three tables, and each collection's [`Records`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Index {
    vocabulary: Box<[Box<str>]>,
    /// The tag languages the tables carry, sorted.
    subtags: Box<[String]>,
    nerd: Records,
    lucide: Records,
    unicode: Records,
}

/// The words of the tables as they are met, each once, with a provisional id.
#[derive(Default)]
struct Interner {
    ids: std::collections::HashMap<Box<str>, u32>,
    words: Vec<Box<str>>,
}

impl Interner {
    fn id(&mut self, word: &str) -> u32 {
        if let Some(id) = self.ids.get(word) {
            return *id;
        }
        // 2³² distinct words would be a table of hundreds of gigabytes.
        let id = u32::try_from(self.words.len()).unwrap_or(u32::MAX);
        self.words.push(word.into());
        self.ids.insert(word.into(), id);
        id
    }

    /// The ids of every word of `texts`.
    fn words<'a>(&mut self, texts: impl IntoIterator<Item = &'a String>) -> Vec<u32> {
        let mut out = Vec::new();
        for text in texts {
            let folded = fold(text);
            for w in words_of(&folded) {
                out.push(self.id(w));
            }
        }
        out
    }

    /// The ids of `texts`, each whole.
    fn wholes<'a>(&mut self, texts: impl IntoIterator<Item = &'a String>) -> Vec<u32> {
        texts.into_iter().map(|t| self.id(&fold(t))).collect()
    }
}

/// A record's terms with provisional ids, before the vocabulary is sorted.
struct Raw {
    name: String,
    name_words: Vec<u32>,
    alias_words: Vec<u32>,
    groups: Vec<u32>,
    tags: Vec<Vec<u32>>,
    search: Vec<u32>,
    /// A Unicode record's block, lowered; `None` in the other collections.
    block: Option<String>,
}

impl Raw {
    fn new(
        words: &mut Interner,
        subtags: &[String],
        name: &str,
        aliases: &[String],
        tags: &BTreeMap<String, Vec<String>>,
        search: &[String],
    ) -> Self {
        let name = fold(name);
        let name_words = words_of(&name).map(|w| words.id(w)).collect();
        Self {
            name_words,
            name,
            alias_words: words.words(aliases),
            groups: Vec::new(),
            tags: subtags
                .iter()
                .map(|s| words.words(tags.get(s).into_iter().flatten()))
                .collect(),
            search: words.words(search),
            block: None,
        }
    }

    /// Every record's lists renamed to the sorted ids, laid out one record
    /// after another.
    fn records(raws: Vec<Self>, renamed: &[u32], subtags: usize) -> Records {
        let kinds = TAGS + subtags;
        let mut out = Records {
            kinds,
            bounds: vec![0],
            ..Records::default()
        };
        let end = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        for raw in raws {
            out.names.push_str(&raw.name);
            out.name_ends.push(end(out.names.len()));
            if let Some(block) = raw.block {
                out.blocks.push(block.into());
            }
            let lists = [raw.name_words, raw.alias_words, raw.groups, raw.search]
                .into_iter()
                .chain(raw.tags);
            for list in lists {
                let from = out.ids.len();
                out.ids
                    .extend(list.into_iter().map(|i| renamed[i as usize]));
                out.ids[from..].sort_unstable();
                let mut kept = from;
                for k in from..out.ids.len() {
                    if kept == from || out.ids[k] != out.ids[kept - 1] {
                        out.ids[kept] = out.ids[k];
                        kept += 1;
                    }
                }
                out.ids.truncate(kept);
                out.bounds.push(end(kept));
            }
        }
        out
    }
}

impl Index {
    fn build(set: &Set) -> Self {
        let subtags: Vec<String> = set
            .nerd
            .iter()
            .flat_map(|g| g.tags.keys())
            .chain(set.lucide.iter().flat_map(|f| f.tags.keys()))
            .chain(set.unicode.iter().flat_map(|c| c.tags.keys()))
            .cloned()
            .collect::<std::collections::BTreeSet<String>>()
            .into_iter()
            .collect();
        let mut words = Interner::default();
        let nerd: Vec<Raw> = set
            .nerd
            .iter()
            .map(|g| {
                let name = g.name.as_deref().unwrap_or_default();
                let aliases = g.names.get(1..).unwrap_or_default();
                let mut raw = Raw::new(&mut words, &subtags, name, aliases, &g.tags, &g.search);
                raw.groups = words.wholes(&g.sets);
                raw
            })
            .collect();
        let lucide: Vec<Raw> = set
            .lucide
            .iter()
            .map(|f| {
                let mut raw = Raw::new(
                    &mut words, &subtags, &f.name, &f.aliases, &f.tags, &f.search,
                );
                raw.groups = words.words(&f.categories);
                raw
            })
            .collect();
        let unicode: Vec<Raw> = set
            .unicode
            .iter()
            .map(|c| {
                let mut raw = Raw::new(
                    &mut words, &subtags, &c.name, &c.aliases, &c.tags, &c.search,
                );
                raw.block = Some(c.block.to_lowercase());
                raw
            })
            .collect();
        // THE VOCABULARY SORTED, and every id renamed to its place in it.
        let mut order: Vec<usize> = (0..words.words.len()).collect();
        order.sort_unstable_by(|a, b| words.words[*a].cmp(&words.words[*b]));
        let mut renamed = vec![0u32; order.len()];
        for (new, old) in order.iter().enumerate() {
            renamed[*old] = u32::try_from(new).unwrap_or(u32::MAX);
        }
        let mut provisional = std::mem::take(&mut words.words);
        let vocabulary = order
            .iter()
            .map(|old| std::mem::take(&mut provisional[*old]))
            .collect();
        let kinds = subtags.len();
        let sorted = |raws: Vec<Raw>| Raw::records(raws, &renamed, kinds);
        Self {
            vocabulary,
            subtags: subtags.into(),
            nerd: sorted(nerd),
            lucide: sorted(lucide),
            unicode: sorted(unicode),
        }
    }

    /// A typed word: the range of the vocabulary it prefixes.
    fn word<'q>(&self, text: &'q str) -> Word<'q> {
        let lo = self.vocabulary.partition_point(|w| &**w < text);
        let hi = lo + self.vocabulary[lo..].partition_point(|w| w.starts_with(text));
        let exact = (lo < hi && &*self.vocabulary[lo] == text).then_some(lo);
        let id = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
        Word {
            text,
            lo: id(lo),
            hi: id(hi),
            exact: exact.map(id),
        }
    }

    const fn of(&self, collection: Collection) -> &Records {
        match collection {
            Collection::NerdFonts => &self.nerd,
            Collection::Lucide => &self.lucide,
            Collection::Unicode => &self.unicode,
        }
    }
}

/// A typed word, looked up once per search.
struct Word<'q> {
    /// Folded.
    text: &'q str,
    /// The ids of the vocabulary words it prefixes: `lo..hi`.
    lo: u32,
    hi: u32,
    /// Its own id, when it is a whole word of the vocabulary.
    exact: Option<u32>,
}

impl Word<'_> {
    /// Whether it prefixes one of `ids` (sorted).
    fn in_(&self, ids: &[u32]) -> bool {
        let i = ids.partition_point(|x| *x < self.lo);
        ids.get(i).is_some_and(|x| *x < self.hi)
    }
}

/// A search result: which record of which collection, and how well it
/// matched (higher is better; the score of the WORST word).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hit {
    /// The collection.
    pub collection: Collection,
    /// The index into that collection's table in the [`Set`].
    pub index: usize,
    /// The score.
    pub score: u32,
}

/// How a word matched, BEST first — one ladder for the three collections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    /// The pasted character itself.
    Identity,
    /// The whole name.
    Name,
    /// The start of the name.
    NameStart,
    /// The start of a word of the name.
    NameWord,
    /// The start of a word of an alias (another name of the same record).
    Alias,
    /// The Nerd Fonts set, whole, or a word of a Lucide category.
    Group,
    /// A tag of the reader's language, or a folded tag.
    Tag,
    /// A tag of another language — found, because a person who reads English
    /// still knows the Portuguese word, but after the reader's own.
    OtherTag,
    /// No word to match: everything that passes the filters, tied.
    Unranked,
}

impl Rank {
    const WORST: u32 = Self::Unranked as u32;

    const fn score(self) -> u32 {
        Self::WORST - self as u32
    }
}

impl Terms<'_> {
    fn list(&self, kind: usize) -> &[u32] {
        &self.ids[self.cuts[kind] as usize..self.cuts[kind + 1] as usize]
    }

    fn tags(&self) -> impl Iterator<Item = &[u32]> {
        (TAGS..self.cuts.len() - 1).map(|k| self.list(k))
    }

    /// How `word` matches this record; `own` is the reader's subtag's place
    /// in [`Index::subtags`], and `groups_whole` says a group matches only
    /// whole (a Nerd Fonts set).
    fn rank(&self, word: &Word<'_>, own: Option<usize>, groups_whole: bool) -> Option<Rank> {
        if self.name == word.text {
            Some(Rank::Name)
        } else if self.name.starts_with(word.text) {
            Some(Rank::NameStart)
        } else if word.in_(self.list(NAME_WORDS)) {
            Some(Rank::NameWord)
        } else if word.in_(self.list(ALIAS_WORDS)) {
            Some(Rank::Alias)
        } else if if groups_whole {
            word.exact
                .is_some_and(|id| self.list(GROUP_WORDS).binary_search(&id).is_ok())
        } else {
            word.in_(self.list(GROUP_WORDS))
        } {
            Some(Rank::Group)
        } else if own.is_some_and(|o| word.in_(self.list(TAGS + o))) || word.in_(self.list(SEARCH))
        {
            Some(Rank::Tag)
        } else if self
            .tags()
            .enumerate()
            .any(|(i, t)| Some(i) != own && word.in_(t))
        {
            Some(Rank::OtherTag)
        } else {
            None
        }
    }

    /// Whether every word prefixes a tag of some language, or of `search`.
    fn has_tag(&self, words: &[Word<'_>]) -> bool {
        words
            .iter()
            .all(|w| w.in_(self.list(SEARCH)) || self.tags().any(|t| w.in_(t)))
    }

    /// Whether every word prefixes a word of the name or of an alias.
    fn named(&self, words: &[Word<'_>]) -> bool {
        words
            .iter()
            .all(|w| w.in_(self.list(NAME_WORDS)) || w.in_(self.list(ALIAS_WORDS)))
    }
}

/// A filter as one search reads it: its value's words looked up once.
struct Reading<'f, 'q> {
    filter: &'f Filter,
    words: Vec<Word<'q>>,
}

impl Reading<'_, '_> {
    fn value(&self) -> &str {
        self.filter.value.as_deref().unwrap_or_default()
    }

    /// Whether the value is still being typed: a half-written query
    /// (`block:`) does not narrow to nothing — "unfinished" is not "none".
    fn open(&self) -> bool {
        self.filter.value.as_deref().is_none_or(str::is_empty)
    }
}

/// The ONE search: every word must prefix some term (name, alias, set or
/// category, a tag of any language), filters narrow, the best `most` come
/// first. A single pasted character finds itself first, whatever the filters.
///
/// `subtag` is the reader's language (`en`, `pt`): a word found in its tags
/// ranks above the same word found only in another language's.
///
/// Words are compared FOLDED on both sides (`atenção` finds `atencao`),
/// against the index built at load: no allocation per record, a few binary
/// searches per word.
#[must_use]
pub fn search(
    set: &Set,
    collection: Collection,
    query: &Query,
    subtag: &str,
    most: usize,
) -> Vec<Hit> {
    let index = set.index();
    let own = index.subtags.iter().position(|s| s == subtag);
    // FOLDED HERE TOO: `Query`'s fields are public, and a caller that built
    // one by hand should not find less.
    let typed: Vec<String> = query.words.iter().map(|w| fold(w)).collect();
    let words: Vec<Word<'_>> = typed.iter().map(|w| index.word(w)).collect();
    let values: Vec<Vec<String>> = query
        .filters
        .iter()
        .map(|f| f.value.as_deref().map(split_words).unwrap_or_default())
        .collect();
    let readings: Vec<Reading<'_, '_>> = query
        .filters
        .iter()
        .zip(&values)
        .map(|(filter, v)| Reading {
            filter,
            words: v.iter().map(|w| index.word(w)).collect(),
        })
        .collect();
    let pasted: Option<Vec<u32>> = query
        .pasted
        .as_deref()
        .map(|p| p.chars().map(u32::from).collect());
    let groups_whole = collection == Collection::NerdFonts;
    let mut scored: Vec<(Rank, usize)> = Vec::new();
    let records = index.of(collection);
    for i in 0..records.len() {
        let terms = &records.terms(i);
        let (identity, passes) = match collection {
            Collection::NerdFonts => {
                let g = &set.nerd[i];
                (
                    pasted.as_deref() == Some(&[g.cp]),
                    readings.iter().all(|r| nerd_passes(g, terms, r)),
                )
            }
            Collection::Lucide => {
                let f = &set.lucide[i];
                (
                    pasted
                        .as_deref()
                        .is_some_and(|p| p == [f.cp] || p == [f.lcp]),
                    readings.iter().all(|r| lucide_passes(f, terms, r)),
                )
            }
            Collection::Unicode => {
                let c = &set.unicode[i];
                (
                    pasted.as_deref() == Some(&c.seq[..]),
                    readings.iter().all(|r| unicode_passes(c, terms, r)),
                )
            }
        };
        if identity {
            scored.push((Rank::Identity, i));
            continue;
        }
        if !passes {
            continue;
        }
        let rank = if words.is_empty() {
            Some(Rank::Unranked)
        } else {
            words.iter().try_fold(Rank::Identity, |worst, w| {
                terms.rank(w, own, groups_whole).map(|r| worst.max(r))
            })
        };
        if let Some(rank) = rank {
            scored.push((rank, i));
        }
    }
    // A TIE IS BROKEN BY THE SHORTER NAME — every word matched it, so more
    // of it is what was typed (`long leftwards arrow` is LONG LEFTWARDS ARROW
    // before DOWNWARDS TRIANGLE-HEADED ARROW WITH LONG TIP LEFTWARDS) — and
    // then by the name, so the list does not dance between two similar
    // queries. With no word typed, by the name alone.
    let name = |index: usize| -> &str {
        match collection {
            Collection::NerdFonts => set.nerd[index].name.as_deref().unwrap_or_default(),
            Collection::Lucide => &set.lucide[index].name,
            Collection::Unicode => &set.unicode[index].name,
        }
    };
    let length = |index: usize| {
        if words.is_empty() {
            0
        } else {
            name(index).len()
        }
    };
    // AND A RECORD WITH NO NAME LAST among its ties: 126 glyphs of the font
    // are in no table of names, and with nothing typed they sorted first —
    // the grid opened on the ones nobody can look up.
    let order = |a: &(Rank, usize), b: &(Rank, usize)| {
        let (na, nb) = (name(a.1), name(b.1));
        (a.0, length(a.1), na.is_empty(), na, a.1).cmp(&(b.0, length(b.1), nb.is_empty(), nb, b.1))
    };
    // ONLY THE BEST `most` ARE SORTED: a strip shows a dozen of thousands.
    if most == 0 {
        return Vec::new();
    }
    if scored.len() > most {
        scored.select_nth_unstable_by(most - 1, order);
        scored.truncate(most);
    }
    scored.sort_unstable_by(order);
    scored
        .into_iter()
        .map(|(rank, index)| Hit {
            collection,
            index,
            score: rank.score(),
        })
        .collect()
}

/// Whether `cp`, written in hexadecimal without leading zeros, starts with
/// `hex` — `block:f0` is `U+F0xx` — without writing it.
fn hex_starts(cp: u32, hex: &str) -> bool {
    if hex.is_empty() {
        return true;
    }
    if hex.len() > 8 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    let Ok(value) = u32::from_str_radix(hex, 16) else {
        return false;
    };
    let digits = (32 - cp.leading_zeros()).div_ceil(4).max(1);
    let typed = u32::try_from(hex.len()).unwrap_or(u32::MAX);
    typed <= digits && cp >> (4 * (digits - typed)) == value
}

fn nerd_passes(g: &Nerd, terms: &Terms<'_>, r: &Reading<'_, '_>) -> bool {
    match r.filter.kind {
        FilterKind::Set => r.open() || g.sets.iter().any(|s| s == r.value()),
        // BY HEXADECIMAL PREFIX: `block:f0` is `U+F0xx`.
        FilterKind::Block => hex_starts(g.cp, r.value().trim_start_matches("u+")),
        FilterKind::Tag => r.open() || terms.has_tag(&r.words),
        FilterKind::Token => g.token.is_some(),
        FilterKind::App => g.in_tray,
        FilterKind::Name => terms.named(&r.words),
        // NOT THIS COLLECTION'S: a filter that means nothing here finds
        // nothing, rather than being silently ignored.
        FilterKind::Symbols
        | FilterKind::Family
        | FilterKind::Category
        | FilterKind::Group
        | FilterKind::Subgroup
        | FilterKind::Tone => false,
    }
}

fn lucide_passes(f: &Lucide, terms: &Terms<'_>, r: &Reading<'_, '_>) -> bool {
    match r.filter.kind {
        FilterKind::Category => r.open() || f.categories.iter().any(|c| c == r.value()),
        FilterKind::Name => terms.named(&r.words),
        FilterKind::Tag => r.open() || terms.has_tag(&r.words),
        FilterKind::Set
        | FilterKind::Block
        | FilterKind::Token
        | FilterKind::App
        | FilterKind::Symbols
        | FilterKind::Family
        | FilterKind::Group
        | FilterKind::Subgroup
        | FilterKind::Tone => false,
    }
}

fn unicode_passes(c: &Char, terms: &Terms<'_>, r: &Reading<'_, '_>) -> bool {
    let value = r.value();
    let is = |field: &Option<String>| r.open() || field.as_deref() == Some(value);
    match r.filter.kind {
        // A SYMBOL AND A NUMBER: `①` and `¼` are numbers by category and
        // symbols to the eye.
        FilterKind::Symbols => c.family == "symbol" || c.family == "number",
        // EXACT against the id: nine records carry `emoji` among their tags
        // without being of the family, and a filter that returns too much is
        // worse than one that returns too little.
        FilterKind::Family => r.open() || c.family == value,
        // BY PREFIX of the code: `cat:s` is the four kinds of symbol (the
        // codes are ASCII).
        FilterKind::Category => c
            .category
            .get(..value.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(value)),
        // BY NAME, not by range: Unicode's blocks do not end on round numbers.
        FilterKind::Block => terms.block.contains(value),
        FilterKind::Name => terms.named(&r.words),
        FilterKind::Tag => r.open() || terms.has_tag(&r.words),
        FilterKind::Group => is(&c.group),
        FilterKind::Subgroup => {
            r.open() || c.subgroup.as_deref().is_some_and(|s| s.contains(value))
        }
        FilterKind::Tone => is(&c.tone),
        FilterKind::Set | FilterKind::Token | FilterKind::App => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grammar() -> Grammar {
        Grammar {
            prefixes: vec![
                Prefix {
                    word: "family:".to_owned(),
                    kind: FilterKind::Family,
                    implied: None,
                },
                Prefix {
                    word: "familia:".to_owned(),
                    kind: FilterKind::Family,
                    implied: None,
                },
                Prefix {
                    word: "name:".to_owned(),
                    kind: FilterKind::Name,
                    implied: None,
                },
                Prefix {
                    word: "md:".to_owned(),
                    kind: FilterKind::Set,
                    implied: Some("md".to_owned()),
                },
            ],
            values: vec![Named {
                kind: FilterKind::Family,
                word: "letra".to_owned(),
                id: "letter".to_owned(),
            }],
        }
    }

    #[test]
    fn a_query_takes_its_filters_by_the_grammar() {
        let grammar = grammar();
        let query = Query::parse("md: Family:letter arrow name:black star", &grammar);
        assert_eq!(query.words, ["arrow"]);
        assert_eq!(
            query.filters,
            [
                Filter {
                    kind: FilterKind::Set,
                    value: Some("md".to_owned())
                },
                Filter {
                    kind: FilterKind::Family,
                    value: Some("letter".to_owned())
                },
                Filter {
                    kind: FilterKind::Name,
                    value: Some("black star".to_owned())
                },
            ]
        );
        assert_eq!(Query::parse(" ★ ", &grammar).pasted.as_deref(), Some("★"));
        assert_eq!(
            load(Path::new("/nonexistent")).unwrap_err(),
            Refusal::of(Why::Unreadable, NERD_FILE)
        );
    }

    /// The tables are read record by record: a good file loads and is
    /// searchable at once, and a bad one is refused by name, whether the
    /// JSON is broken, a record lacks a field, or something follows the array.
    #[test]
    fn a_table_loads_record_by_record_and_a_bad_one_is_named() {
        let dir = std::env::temp_dir().join(format!("ronoctua-iconset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |file: &str, text: &str| std::fs::write(dir.join(file), text).unwrap();
        write(
            NERD_FILE,
            r#"[{"cp": 61461, "name": "md-home", "sets": ["md"]}]"#,
        );
        write(
            LUCIDE_FILE,
            r#"[{"name": "house", "cp": 991232, "lcp": 57589, "nodes": "", "tags_pt": ["casa"]}]"#,
        );
        write(
            UNICODE_FILE,
            r#"[{"cp": 945, "name": "GREEK SMALL LETTER ALPHA", "family": "letter"}]"#,
        );
        let set = load(&dir).unwrap();
        assert_eq!(
            (set.nerd.len(), set.lucide.len(), set.unicode.len()),
            (1, 1, 1)
        );
        assert_eq!(
            set.unicode[0].seq,
            [945],
            "an absent sequence is the codepoint"
        );
        let q = Query::parse("casa", &Grammar::default());
        assert_eq!(search(&set, Collection::Lucide, &q, "en", 5).len(), 1);
        for (text, why) in [
            ("[{", Why::Garbled),
            (r#"[{"name": "house"}]"#, Why::Garbled),
            (r#"[] []"#, Why::Garbled),
            (r#"{"cp": 1}"#, Why::Garbled),
        ] {
            write(LUCIDE_FILE, text);
            assert_eq!(
                load(&dir).unwrap_err(),
                Refusal::of(why, LUCIDE_FILE),
                "{text}"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_value_is_typed_in_any_language_and_accents_do_not_matter() {
        let grammar = grammar();
        // pt-BR: the search language is interface, and this is its pt-BR half.
        let q = Query::parse("Família:letra alfa", &grammar);
        assert_eq!(q.filters[0].value.as_deref(), Some("letter"));
        assert_eq!(q.words, ["alfa"]);
        // An enumeration takes ONE word: the rest is still free text.
        let q = Query::parse("family: letter alpha", &grammar);
        assert_eq!(q.filters[0].value.as_deref(), Some("letter"));
        assert_eq!(q.words, ["alpha"]);
        // A set prefix takes nothing, and what is glued to it is a word.
        let q = Query::parse("md:home", &grammar);
        assert_eq!(q.filters[0].value.as_deref(), Some("md"));
        assert_eq!(q.words, ["home"]);
    }

    #[test]
    fn every_refusal_and_prefix_has_words_in_every_language() {
        for language in LANGUAGES {
            let texts = Texts::fresh(language).unwrap();
            for w in Why::ALL {
                assert!(
                    texts.has(&format!("icons-why-{}", w.id())),
                    "{language}: {}",
                    w.id()
                );
            }
            for (_, k) in FilterKind::WORDED {
                assert!(
                    !texts.t(k).contains(' '),
                    "{language}: a prefix is one word: {k}"
                );
            }
        }
    }

    #[test]
    fn one_character_is_one_to_the_eye() {
        assert!(one_character("a"));
        assert!(one_character("α"));
        assert!(one_character("\u{1F469}\u{200D}\u{1F4BB}"), "👩‍💻 is one");
        assert!(one_character("\u{1F44D}\u{1F3FF}"), "a toned thumb is one");
        assert!(one_character("\u{1F1E7}\u{1F1F7}"), "a flag is one");
        assert!(!one_character("ab"));
        assert!(!one_character("a b"));
        assert!(!one_character(""));
    }

    #[test]
    fn every_word_must_match_and_the_worst_decides() {
        let tag = |w: &[&str]| -> BTreeMap<String, Vec<String>> {
            [("en".to_owned(), w.iter().map(|s| (*s).to_owned()).collect())].into()
        };
        let set = Set {
            lucide: vec![
                Lucide {
                    name: "arrow-up".to_owned(),
                    cp: 0xF2000,
                    lcp: 0xE000,
                    tags: tag(&["direction"]),
                    ..Lucide::default()
                },
                Lucide {
                    name: "house".to_owned(),
                    cp: 0xF2001,
                    lcp: 0xE0F5,
                    aliases: vec!["home".to_owned()],
                    tags: tag(&["up"]),
                    ..Lucide::default()
                },
            ],
            ..Set::default()
        };
        let g = Grammar::default();
        let hits = |q: &str| -> Vec<usize> {
            search(&set, Collection::Lucide, &Query::parse(q, &g), "en", 10)
                .iter()
                .map(|h| h.index)
                .collect()
        };
        assert_eq!(hits("arrow up"), [0]);
        assert_eq!(hits("up"), [0, 1], "a name word beats a tag");
        assert_eq!(hits("home"), [1]);
        assert_eq!(hits("\u{E0F5}"), [1], "Lucide's published character");
        assert_eq!(hits("\u{F2000}"), [0], "the drawn character");
        assert_eq!(hits("up"), hits("up"), "deterministic");
        assert!(hits("nothing").is_empty());
        assert_eq!(
            search(&set, Collection::Lucide, &Query::default(), "en", 1).len(),
            1,
            "`most` is a ceiling"
        );
        assert!(search(&set, Collection::Lucide, &Query::default(), "en", 0).is_empty());
        // pt-BR: typed with its accents, found in a table that wrote none.
        let accented = Set::new(
            Vec::new(),
            vec![Lucide {
                name: "triangle-alert".to_owned(),
                tags: [("pt".to_owned(), vec!["atencao".to_owned()])].into(),
                ..Lucide::default()
            }],
            Vec::new(),
        );
        for typed in ["atenção", "ATENÇÃO", "atencao", "aten"] {
            let q = Query::parse(typed, &g);
            assert_eq!(
                search(&accented, Collection::Lucide, &q, "pt", 10).len(),
                1,
                "{typed}"
            );
        }
    }

    #[test]
    fn a_block_is_a_hexadecimal_prefix_written_without_zeros() {
        assert!(hex_starts(0xF015, "f0"));
        assert!(hex_starts(0xF015, "f015"));
        assert!(hex_starts(0xF015, ""));
        assert!(!hex_starts(0xF015, "f0150"));
        assert!(!hex_starts(0xF015, "0f"), "no leading zeros are written");
        assert!(!hex_starts(0xF015, "e"));
        assert!(!hex_starts(0xF015, "fz"));
        assert!(hex_starts(0xE0A0, "e0a"));
        assert!(hex_starts(0x0, "0"));
        assert!(hex_starts(0xF_FFFF, "fffff"));
    }
}

/// The tests on the REAL tables — the ones the three buffers' drivers ran
/// (`ronoctua-test-{browser,symbols,lucide}`), ported. They need
/// [`VARIABLE`]: the dev shell and `rust.tests` set it, and without it they
/// fail rather than pass having proved nothing.
#[cfg(test)]
mod on_the_tables {
    use super::*;
    use crate::copy::{LucideFormat, UnicodeFormat};
    use crate::lucide::Adjust;

    fn set() -> Arc<Set> {
        assert!(
            std::env::var_os(VARIABLE).is_some(),
            "{VARIABLE} is not set: these tests search the real tables (enter the dev shell)"
        );
        shared().expect("the three tables load")
    }

    /// The grammar of every language, built once for the whole run: it
    /// parses every `.ftl`.
    fn grammar(set: &Set) -> &'static Grammar {
        static GRAMMAR: std::sync::OnceLock<Grammar> = std::sync::OnceLock::new();
        GRAMMAR.get_or_init(|| Grammar::new(set))
    }

    fn found(set: &Set, c: Collection, text: &str) -> Vec<Hit> {
        search(set, c, &Query::parse(text, grammar(set)), "en", set.len(c))
    }

    fn count(set: &Set, c: Collection, text: &str) -> usize {
        found(set, c, text).len()
    }

    #[test]
    fn the_set_is_shared_while_held_and_dropped_after() {
        let slot = Mutex::new(Weak::new());
        let mut loads = 0;
        let mut load = || {
            loads += 1;
            Ok(Set::default())
        };
        let a = held(&slot, &mut load).unwrap();
        let b = held(&slot, &mut load).unwrap();
        assert!(Arc::ptr_eq(&a, &b), "a second view gets the same tables");
        drop((a, b));
        assert!(
            slot.lock().unwrap().upgrade().is_none(),
            "the last view let go"
        );
        let _c = held(&slot, &mut load).unwrap();
        assert_eq!(loads, 2, "and the next view loads them again");
        assert!(shared().is_ok_and(|s| !s.is_empty()), "the real ones load");
    }

    #[test]
    fn house_is_lucides_e0f5_and_every_figure_draws_in_the_moved_range() {
        let set = set();
        let hits = found(&set, Collection::Lucide, "house");
        let house = &set.lucide[hits[0].index];
        assert_eq!(house.name, "house", "`house` comes first for `house`");
        assert_eq!(house.lcp, 0xE0F5);
        assert_eq!(
            copy::lucide(house, &Adjust::lucide(), LucideFormat::Character),
            "\u{E0F5}"
        );
        assert_eq!(
            copy::lucide(house, &Adjust::lucide(), LucideFormat::Hex),
            "U+E0F5"
        );
        // THE DRAWN CODEPOINTS ARE OURS: Lucide's own range collides with 634
        // Nerd Fonts glyphs, and the face was moved to U+F2000…U+F2683.
        for f in &set.lucide {
            assert!(
                (0xF2000..=0xF2683).contains(&f.cp),
                "{} draws at U+{:X}",
                f.name,
                f.cp
            );
            assert_ne!(f.cp, f.lcp, "{}", f.name);
        }
        assert!(
            count(&set, Collection::Lucide, "home") > 0,
            "an alias finds it"
        );
        // pt-BR: a Portuguese tag.
        assert!(count(&set, Collection::Lucide, "casa") > 0);
        let up = found(&set, Collection::Lucide, "arrow up");
        let first = &set.lucide[up[0].index].name;
        assert!(first.contains("arrow") && first.contains("up"), "{first}");
    }

    #[test]
    fn the_copied_svg_is_the_previewed_svg_before_and_after_a_stroke_change() {
        let set = set();
        let house = set.lucide.iter().find(|f| f.name == "house").unwrap();
        let mut adjust = Adjust::lucide();
        let before = lucide::svg(house, &adjust, false);
        assert_eq!(copy::lucide(house, &adjust, LucideFormat::Svg), before);
        assert!(
            before.contains("M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"),
            "{before}"
        );
        adjust.stroke_tenths = 15;
        let after = lucide::svg(house, &adjust, false);
        assert_ne!(before, after);
        assert_eq!(copy::lucide(house, &adjust, LucideFormat::Svg), after);
        assert!(after.contains(r#"stroke-width="1.5""#));
        let every: Vec<String> = LucideFormat::ALL
            .iter()
            .map(|f| copy::lucide(house, &adjust, *f))
            .collect();
        assert!(
            every.iter().all(|s| !s.is_empty()),
            "all 19 formats write something"
        );
        assert!(
            every.last().unwrap().contains("lucide.svg#house"),
            "the sprite is last"
        );
    }

    #[test]
    fn a_combining_mark_is_drawn_on_its_circle_and_copied_alone() {
        let set = set();
        let marks = found(&set, Collection::Unicode, "family:mark");
        assert!(marks.len() > 100, "the combining marks are there");
        for hit in &marks {
            let mark = &set.unicode[hit.index];
            if mark.width != 0 {
                continue;
            }
            assert!(mark.drawn().starts_with('\u{25CC}'), "{:X}", mark.cp);
            let copied = copy::unicode(mark, UnicodeFormat::Character);
            assert_eq!(
                copied.chars().count(),
                1,
                "{:X} copies one character",
                mark.cp
            );
            assert!(!copied.starts_with('\u{25CC}'), "never the dotted circle");
            assert_eq!(
                copy::unicode(mark, UnicodeFormat::Hex),
                format!("U+{:04X}", mark.cp)
            );
        }
    }

    #[test]
    fn the_long_arrows_budget_two_cells() {
        let set = set();
        // THE TWENTY-THREE the symbol driver named are a floor: the font has
        // grown more wide characters since, and every one budgets two cells.
        let wide: Vec<&Char> = set
            .unicode
            .iter()
            .filter(|c| c.width == 2 && c.family != "emoji")
            .collect();
        assert!(
            wide.len() >= 23,
            "the long arrows and their kin: {}",
            wide.len()
        );
        for c in &set.unicode {
            assert_eq!(c.ink(3), u16::from(c.width.max(1)) * 3, "{:X}", c.cp);
        }
        let long: Vec<&Char> = set
            .unicode
            .iter()
            .filter(|c| c.block == "Supplemental Arrows-A" && c.name.starts_with("LONG "))
            .collect();
        assert!(!long.is_empty() && long.iter().all(|c| c.width == 2));
        let arrow = &set.unicode[found(&set, Collection::Unicode, "\u{27F5}")[0].index];
        assert_eq!(arrow.name, "LONG LEFTWARDS ARROW");
        assert_eq!(arrow.ink(2), 4, "two cells of advance at a two-cell scale");
        for hit in found(&set, Collection::Unicode, "block:box drawing") {
            assert_eq!(set.unicode[hit.index].ink(2), 2);
        }
    }

    #[test]
    fn a_pasted_character_is_found_first_whatever_it_is() {
        let set = set();
        let alpha = found(&set, Collection::Unicode, "α");
        assert_eq!(set.unicode[alpha[0].index].cp, 0x3B1);
        let coder = found(&set, Collection::Unicode, "\u{1F469}\u{200D}\u{1F4BB}");
        let first = &set.unicode[coder[0].index];
        assert_eq!(first.seq, [0x1F469, 0x200D, 0x1F4BB], "the whole sequence");
        assert_eq!(
            copy::unicode(first, UnicodeFormat::Character)
                .chars()
                .count(),
            3
        );
        let glyph = &set.nerd[0];
        let text = char::from_u32(glyph.cp).unwrap().to_string();
        assert_eq!(found(&set, Collection::NerdFonts, &text)[0].index, 0);
        // AND A TYPED LETTER IS NOT AN IDENTITY OF THE ARROW: `LONG
        // LEFTWARDS ARROW` once put the letter `a` first.
        let long = found(&set, Collection::Unicode, "LONG LEFTWARDS ARROW");
        let first = &set.unicode[long[0].index];
        assert!(
            first.name.starts_with("LONG LEFTWARDS"),
            "{} first",
            first.name
        );
    }

    #[test]
    fn words_narrow_by_prefix_and_the_order_is_the_searchers() {
        let set = set();
        // pt-BR: the searches below are the Portuguese half of the language.
        let phrase = count(&set, Collection::Unicode, "seta para cima");
        let arrow = found(&set, Collection::Unicode, "seta");
        assert!(
            phrase > 0 && phrase < arrow.len(),
            "{phrase} of {}",
            arrow.len()
        );
        assert_ne!(
            set.unicode[arrow[0].index].cp, 0x20A7,
            "not PESETA SIGN first"
        );
        // pt-BR: a flag's CLDR words.
        assert!(count(&set, Collection::Unicode, "bandeira brasil") > 0);
        assert!(
            count(&set, Collection::Unicode, "cat:s") > count(&set, Collection::Unicode, "cat:sm")
        );
        let total = set.unicode.len();
        for q in ["familia:letra", "cat:sm", "bloco:arrows", "grupo:carinhas"] {
            let n = count(&set, Collection::Unicode, q);
            assert!(n > 0 && n < total, "`{q}` narrowed to {n} of {total}");
        }
        let emoji = count(&set, Collection::Unicode, "family:emoji");
        let tones: Vec<usize> = TONES
            .iter()
            .map(|t| count(&set, Collection::Unicode, &format!("tone:{t}")))
            .collect();
        assert!(tones.iter().all(|n| *n > 0), "{tones:?}");
        assert!(tones.iter().sum::<usize>() < emoji);
        let dark = count(&set, Collection::Unicode, "tom:escuro");
        let hand = count(&set, Collection::Unicode, "tom:escuro mao");
        assert!(hand > 0 && hand < dark, "{hand} of {dark}");
        assert!(
            count(&set, Collection::Unicode, "symbols:") > 0,
            "the opening posture finds"
        );
        // The Nerd Fonts' own prefixes.
        for hit in found(&set, Collection::NerdFonts, "md:home") {
            assert!(set.nerd[hit.index].sets.iter().any(|s| s == "md"));
        }
        for hit in found(&set, Collection::NerdFonts, "block:f0") {
            assert!(format!("{:x}", set.nerd[hit.index].cp).starts_with("f0"));
        }
        let tokens = found(&set, Collection::NerdFonts, "token:");
        assert!(!tokens.is_empty() && tokens.iter().all(|h| set.nerd[h.index].token.is_some()));
        // A filter that means nothing here finds nothing, and says so by it.
        assert_eq!(count(&set, Collection::Lucide, "token:"), 0);
        let within = found(&set, Collection::Lucide, "categoria:arrows up");
        assert!(within.len() < count(&set, Collection::Lucide, "category:arrows"));
        assert!(set.lucide[within[0].index].name.contains("up"));
    }

    #[test]
    fn an_accent_typed_or_not_finds_the_same() {
        let set = set();
        for c in [
            Collection::NerdFonts,
            Collection::Lucide,
            Collection::Unicode,
        ] {
            // pt-BR: a word the tables write without its accent.
            let bare = count(&set, c, "atencao");
            // pt-BR: the same word typed with its accents.
            assert_eq!(count(&set, c, "atenção"), bare, "{c:?}");
            // pt-BR: and capitalised.
            assert_eq!(count(&set, c, "Atenção"), bare, "{c:?}");
        }
        // pt-BR: a Lucide tag.
        assert!(count(&set, Collection::Lucide, "atenção") > 0);
    }

    /// Fast-access searches the three collections on every keystroke, on its
    /// event loop, inside 2 ms for every in-process source together
    /// (components-apps.md F2). Measured in an optimized build — `rust.tests`
    /// runs the `loop` profile — as the FASTEST of several runs: the cost of
    /// the search itself, which a machine busy with the other tests (they run
    /// in parallel) only adds to. About 0.9 ms for the three when measured.
    #[test]
    fn three_strips_fit_a_keystroke() {
        let set = set();
        let g = grammar(&set);
        if cfg!(debug_assertions) {
            // An unoptimized build is an order of magnitude slower and
            // measures nothing; the searches still run, for their answers.
            assert!(!found(&set, Collection::Unicode, "arrow").is_empty());
            return;
        }
        for typed in ["a", "arrow", "seta para", "home", "", "family:emoji mao"] {
            let q = Query::parse(typed, g);
            let fastest = (0..15)
                .map(|_| {
                    let t = std::time::Instant::now();
                    for c in Collection::ALL {
                        std::hint::black_box(search(&set, c, &q, "pt", 24));
                    }
                    t.elapsed()
                })
                .min()
                .unwrap_or_default();
            assert!(
                fastest < std::time::Duration::from_millis(2),
                "`{typed}` took {fastest:?} for the three strips"
            );
        }
    }

    #[test]
    fn every_panel_label_finds_its_whole_category_in_every_language() {
        let set = set();
        let mut panels: Vec<(String, String, Collection)> = Vec::new();
        for id in FAMILIES {
            panels.push((
                format!("icons-family-{id}"),
                format!("family:{id}"),
                Collection::Unicode,
            ));
        }
        for id in GROUPS {
            panels.push((
                format!("icons-group-{id}"),
                format!("group:{id}"),
                Collection::Unicode,
            ));
        }
        for id in TONES {
            panels.push((
                format!("icons-tone-{id}"),
                format!("tone:{id}"),
                Collection::Unicode,
            ));
        }
        for c in set.categories() {
            panels.push((
                format!("icons-category-{c}"),
                format!("category:{c}"),
                Collection::Lucide,
            ));
        }
        for s in set.sets() {
            panels.push((
                format!("icons-set-{s}"),
                format!("{s}:"),
                Collection::NerdFonts,
            ));
        }
        for language in LANGUAGES {
            let texts = Texts::fresh(language).expect("a declared language");
            for (k, filter, c) in &panels {
                assert!(
                    texts.has(k),
                    "{language} has no `{k}`: the panel would draw [{k}]"
                );
                let whole = count(&set, *c, filter);
                assert!(whole > 0, "`{filter}` is empty");
                let label = texts.t(k).to_lowercase();
                let by_label = count(&set, *c, &label);
                assert!(
                    by_label >= whole,
                    "in {language} the panel says {label:?}, which finds {by_label}, and \
                     `{filter}` has {whole}: the screen teaches a word the search does not know"
                );
            }
        }
    }

    /// FAILS IF: with nothing typed, a glyph no table names comes before one
    /// that has a name — the grid would open on the ones nobody can look up.
    #[test]
    fn a_nameless_glyph_comes_last() {
        let nameless = Nerd {
            cp: 0xE000,
            ..Nerd::default()
        };
        let named = Nerd {
            cp: 0xF000,
            name: Some("md-home".to_owned()),
            names: vec!["md-home".to_owned()],
            ..Nerd::default()
        };
        let set = Set::new(vec![nameless, named], Vec::new(), Vec::new());
        let hits = search(&set, Collection::NerdFonts, &Query::default(), "en", 10);
        let order: Vec<usize> = hits.iter().map(|h| h.index).collect();
        assert_eq!(order, [1, 0]);
    }
}
