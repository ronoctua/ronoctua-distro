//! The player's local catalogue: every source's categories and items, in two
//! SQLite files a door reads directly.
//!
//! # Why a database, and why here
//!
//! The desks asked a directory once per page, one page per machine: two doors
//! overwrote each other, a filter was a round trip, and a list of fifty
//! thousand IPTV channels could not be filtered by letter at all. The
//! catalogue turns that around. The daemon INGESTS a source whole — its
//! categories, its items — and the doors read the result on their own, with
//! every filter answered by an index in a few milliseconds.
//!
//! It is SQLite through `rusqlite`, linked to the system's `libsqlite3`
//! (FTS5 comes with it; nothing is compiled in C of our own). This crate and
//! `ronoctua-video-metrics` are the only two that name `rusqlite`
//! (`one-sqlite-boundary`): a door asks [`Reader`], and no SQL leaves this
//! file.
//!
//! # Two files
//!
//! `$XDG_CACHE_HOME/ronoctua/player-catalog/catalog.sqlite`, and beside it
//! `catalog-adult.sqlite`, where every group an adult word names goes
//! ([`crate::sources::is_adult`]) and every source the person marked adult. A
//! door opens the second only after the person's password (a lock on the
//! doors, not encryption — the page says so).
//!
//! Both are a CACHE: rebuildable from the sources, so a schema that changed is
//! a catalogue emptied and asked again, never migrated — and so is a file
//! that is not this build's at all (no `meta`, not a database, damaged): the
//! writer asks what a file holds before it writes a thing to it. Both live in
//! a `0700` folder and are created `0600` before SQLite first opens them (its
//! `-wal` and `-shm` take the database's mode): an IPTV provider's stream
//! addresses carry the person's account in them, and so do the items made of
//! them — whose `Debug` masks them ([`NewItem`], [`Item`]).
//!
//! # One writer, many readers
//!
//! The daemon's ingest threads write through one [`Writer`] (WAL, a busy
//! timeout, each source replaced in one transaction per file, the two
//! committed together, so a reader sees the old rows or the new ones and
//! never half). The doors open [`Reader`]s read-only (`SQLITE_OPEN_READ_ONLY`,
//! the flag `mode=ro` sets, and `query_only` on top). What moved is told by
//! `meta.generation`, raised by every commit; a file made again is told by
//! [`Reader::replaced`].
//!
//! # Every listing on an index
//!
//! Every scope (every item, a source's, a category's, a theme's) has an index
//! for every order ([`Sort`]), and a page is chosen on it before its rows are
//! looked up: the last page of fifty thousand costs what the first does
//! (measured in `a_listing_over_fifty_thousand_items_is_walked_on_an_index`,
//! in steps of SQLite's machine, which also refuses a plan that sorts a
//! whole listing).
//!
//! # Bounded
//!
//! `playerCatalogMib` (`assets/cache.nix`) is the most the two files may hold
//! together, and the writer enforces it: an ingest that would pass it is
//! rolled back and refused ([`Trouble::TooLarge`]), the source keeping its old
//! rows; a catalogue found over it (the ceiling was lowered) is emptied.
//!
//! # Stable ids
//!
//! An item's id is a hash of its source and its own id there, and a
//! category's of its source and key: re-ingesting a source gives the same ids,
//! so a door's cursor (which keeps an id, never a row number) stays on the
//! same channel.

#[cfg(feature = "catalog")]
use std::borrow::Cow;

use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use ronoctua_playlist::own::Stream;
use serde::{Deserialize, Serialize};

use crate::sources::Kind as SourceKind;

/// The layout of the two files. A file of another layout is emptied by the
/// writer and refused by a reader until it is.
///
/// A change to the layout — a column, an index, a word a column holds (an
/// item's kind, a source's kind) — raises it: a reader of an older build
/// would otherwise take a newer file for its own. The test
/// `the_layout_has_the_number_it_says` pins the layout's text and words to
/// this number.
pub const SCHEMA: i64 = 1;

/// The folder, under the cache root.
pub const FOLDER: &str = "player-catalog";

/// The most characters of a description kept for the search.
pub const DESCRIPTION_MOST: usize = 2000;

/// The most rows one page may hold, whatever the door asks.
pub const PER_PAGE_MOST: u32 = 500;

/// Which of the two files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Store {
    /// Everything but the adult groups.
    Main,
    /// The adult groups and the sources marked adult.
    Adult,
}

impl Store {
    /// Both, in the order a writer commits them (the adult first: a failure
    /// of the second leaves nothing adult in the main file).
    pub const ALL: [Self; 2] = [Self::Adult, Self::Main];

    /// Its file's name in the folder.
    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Self::Main => "catalog.sqlite",
            Self::Adult => "catalog-adult.sqlite",
        }
    }
}

/// The posters' folder, inside the catalogue's: one file per poster
/// ([`poster_file`]), bounded by `playerPostersMib` (`assets/cache.nix`).
pub const POSTERS: &str = "posters";

/// The file a poster is kept in, in [`POSTERS`]: the digest of its address
/// — the daemon writes it and a door reads it by the same name, and an
/// address (which may carry the person's account) is never a file's name.
#[must_use]
pub fn poster_file(url: &str) -> String {
    format!("{}.img", digest(url.trim()))
}

/// This instance's folder: `player-catalog` under the cache root.
#[must_use]
pub fn folder() -> Option<std::path::PathBuf> {
    ronoctua_xdg::cache_dir().map(|d| d.join(FOLDER))
}

/// What an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    /// A broadcast with no end: a TV channel, a radio, a live.
    Live,
    /// A recording with a picture.
    Video,
    /// A recording with sound only.
    Audio,
}

impl ItemKind {
    /// Every kind.
    pub const ALL: [Self; 3] = [Self::Live, Self::Video, Self::Audio];

    /// Its word, as the database keeps it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Video => "video",
            Self::Audio => "audio",
        }
    }

    /// The kind a word names.
    #[must_use]
    pub fn of(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.word() == word)
    }
}

/// An item's id in the catalogue: stable across ingests of its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ItemId(pub i64);

/// A category's id in the catalogue: stable across ingests of its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CategoryId(pub i64);

/// FNV-1a, 64 bits: the one hash an id is made of. Written here because it
/// must never change between two builds (`std`'s hasher may), and an id that
/// changed would move every cursor.
fn fnv(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (n, part) in parts.iter().enumerate() {
        if n > 0 {
            // A separator no text holds, so ("ab", "c") and ("a", "bc") differ.
            h ^= 0xff;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        for b in part.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// A positive id out of a hash (SQLite's rowid is signed; 0 is avoided).
fn positive(h: u64) -> i64 {
    i64::try_from(h >> 1).unwrap_or(i64::MAX).max(1)
}

impl ItemId {
    /// The id of `source`'s item `ext_id`.
    #[must_use]
    pub fn of(source: &str, ext_id: &str) -> Self {
        Self(positive(fnv(&["item", source, ext_id])))
    }
}

impl CategoryId {
    /// The id of `source`'s category `key`.
    #[must_use]
    pub fn of(source: &str, key: &str) -> Self {
        Self(positive(fnv(&["category", source, key])))
    }
}

/// The id of a theme's list of items: one source's (`Some`), or every
/// source's (`None`).
#[cfg_attr(not(feature = "catalog"), allow(dead_code))]
fn theme_list(source: Option<&str>, theme: &str) -> i64 {
    match source {
        Some(source) => positive(fnv(&["theme", source, theme])),
        None => positive(fnv(&["themes", theme])),
    }
}

/// A short, stable, printable hash of a text — for an id made of something
/// that must not be kept as it is (a stream's address, which may carry the
/// person's account).
#[must_use]
pub fn digest(text: &str) -> String {
    format!("{:016x}", fnv(&[text]))
}

// ---------------------------------------------------------------------------
// FOLDING: the one spelling a title sorts and is filed by.
// ---------------------------------------------------------------------------

/// Whether `c` is a combining mark that folding takes off: the accents of
/// Latin, Greek and Cyrillic after decomposition, and the rarer blocks of
/// the same kind.
const fn is_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}'
            | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}'
            | '\u{20D0}'..='\u{20FF}'
            | '\u{FE20}'..='\u{FE2F}'
    )
}

/// `text` with its accents off, in lower case, its spaces collapsed.
///
/// Decomposed (NFD) and stripped of the combining marks, then lower-cased:
/// `Ação` is `acao`, `Ñandú` is `nandu`, `Ёлка` is `елка`. Not the file
/// manager's 1:1 fold (`ronoctua_apps::fold`, which keeps a character count
/// for its highlight): here nothing is highlighted, and the whole of Unicode
/// is folded rather than two languages.
#[must_use]
pub fn fold(text: &str) -> String {
    const NFD: DecomposingNormalizerBorrowed<'static> = DecomposingNormalizerBorrowed::new_nfd();
    let decomposed = NFD.normalize(text);
    let mut out = String::with_capacity(decomposed.len());
    let mut space = false;
    for c in decomposed.chars().filter(|c| !is_mark(*c)) {
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// The key a title sorts by: [`fold`], without what comes before its first
/// letter or digit — `"Casablanca"` and `¡Hola!` sort under C and H.
#[must_use]
pub fn sort_key(title: &str) -> String {
    let folded = fold(title);
    match folded.find(char::is_alphanumeric) {
        Some(at) => folded[at..].to_owned(),
        None => folded,
    }
}

/// The letter a title is filed under.
///
/// Its first letter or digit, in capitals: a digit (or nothing to file by) is
/// `#`. A Latin or Greek accent is taken off (`É` is `E`, `Ά` is `Α`) except
/// on `Ñ` and `Ç`, which the languages that write them treat as letters of
/// their own; Cyrillic keeps `Й` and `Ё`; any other script is its own first
/// character (a CJK title is filed under its first ideograph). A heuristic,
/// and the one the design named: the letters a list shows are the ones its
/// titles start with.
#[must_use]
pub fn letter_of(title: &str) -> char {
    const NFC: ComposingNormalizerBorrowed<'static> = ComposingNormalizerBorrowed::new_nfc();
    const NFD: DecomposingNormalizerBorrowed<'static> = DecomposingNormalizerBorrowed::new_nfd();
    let composed = NFC.normalize(title);
    let Some(first) = composed.chars().find(|c| c.is_alphanumeric()) else {
        return '#';
    };
    if first.is_numeric() {
        return '#';
    }
    let kept = matches!(first, 'Ñ' | 'ñ' | 'Ç' | 'ç') || ('\u{0400}'..='\u{04FF}').contains(&first);
    let base = if kept {
        first
    } else {
        let mut buffer = [0u8; 4];
        NFD.normalize(first.encode_utf8(&mut buffer))
            .chars()
            .next()
            .unwrap_or(first)
    };
    let mut upper = base.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(one), None) => one,
        // `ß` is `SS`: two letters are not one; it stays itself.
        _ => base,
    }
}

/// The words of `text`, folded: split at every character that is neither a
/// letter, a digit nor `+` (so `18+` is a word).
#[must_use]
pub fn words_of(text: &str) -> Vec<String> {
    fold(text)
        .split(|c: char| !(c.is_alphanumeric() || c == '+'))
        .filter(|w| !w.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// Whether the words of `needle` appear, in order and next to each other, in
/// `haystack` (both already [`words_of`]).
#[must_use]
pub fn has_phrase(haystack: &[String], needle: &[String]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------------------------------------------------------------------------
// WHAT AN INGEST HANDS THE WRITER
// ---------------------------------------------------------------------------

/// A source, as the writer records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSource {
    /// Its id: a default source's (`nasa`) or a person's (`p…`).
    pub id: String,
    /// What kind of source it is.
    pub kind: SourceKind,
    /// What it is called: the list's, the feed's or the channel's own name
    /// (data), or empty for a default source (the door names it by its key).
    pub title: String,
    /// Its address as a door may show it: MASKED (`host/…`,
    /// [`crate::sources::Address::shown`]). The address itself is never in
    /// the catalogue — it lives in `sources.json`, `0600`.
    pub shown: String,
    /// The person marked it adult: all of it goes to the adult store.
    pub adult: bool,
    /// Its posters are shown.
    pub posters_on: bool,
}

/// One category an ingest found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCategory {
    /// Its key, unique in the source: a group's name, a collection, an id.
    pub key: String,
    /// What the source calls it (data); EMPTY for the entries a list filed
    /// under no group, which the door names.
    pub title: String,
    /// The theme it is filed under, when one is known.
    pub theme: Option<String>,
    /// An adult word names it: it goes to the adult store.
    pub adult: bool,
}

/// One item an ingest found.
///
/// Its `Debug` masks its stream's addresses ([`crate::sources::shown`]): a
/// provider's list writes the person's account into every one.
#[derive(Clone, PartialEq)]
pub struct NewItem {
    /// Its id in the source: an iptv-org or programme-guide id, an
    /// archive.org identifier, a video id, a hash of its address. Two items
    /// with one id are ONE item, in both their categories.
    pub ext_id: String,
    /// What it is.
    pub kind: ItemKind,
    /// What it is called (data).
    pub title: String,
    /// Its other names, and words it is found by (a channel's name in another
    /// script, a video's channel, an archive item's subjects).
    pub also: Vec<String>,
    /// What it says about itself, for the search (cut to
    /// [`DESCRIPTION_MOST`]).
    pub description: Option<String>,
    /// Its country, ISO 3166-1 alpha-2, when known.
    pub country: Option<String>,
    /// Its language, as the source writes it, when known.
    pub language: Option<String>,
    /// The height of its picture, in lines, when known.
    pub height: Option<u16>,
    /// How long it runs, in seconds, when known.
    pub duration_s: Option<u32>,
    /// Its year, when known.
    pub year: Option<i32>,
    /// What the source says about its rights (a licence's name or address).
    pub licence: Option<String>,
    /// How popular the source says it is (downloads, views); 0 unknown.
    pub popularity: u64,
    /// When it was published, in seconds since the epoch, when known.
    pub published_s: Option<i64>,
    /// The address of its poster, when it has one.
    pub poster_url: Option<String>,
    /// What the queue holds when it is played.
    pub stream: Stream,
    /// The keys of its categories in this ingest.
    pub categories: Vec<String>,
}

impl std::fmt::Debug for NewItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewItem")
            .field("ext_id", &self.ext_id)
            .field("kind", &self.kind)
            .field("title", &self.title)
            .field("also", &self.also)
            .field("description", &self.description)
            .field("country", &self.country)
            .field("language", &self.language)
            .field("height", &self.height)
            .field("duration_s", &self.duration_s)
            .field("year", &self.year)
            .field("licence", &self.licence)
            .field("popularity", &self.popularity)
            .field("published_s", &self.published_s)
            .field("poster_url", &self.poster_url)
            .field("stream", &Masked(&self.stream))
            .field("categories", &self.categories)
            .finish()
    }
}

/// A stream as a `Debug` prints it: its address, its alternates and its
/// referrer masked (`host/…`), since a provider writes the person's account
/// into them.
struct Masked<'a>(&'a Stream);

impl std::fmt::Debug for Masked<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.0;
        f.debug_struct("Stream")
            .field("url", &crate::sources::shown(&s.url))
            .field("title", &s.title)
            .field(
                "referrer",
                &s.referrer.as_deref().map(crate::sources::shown),
            )
            .field("user_agent", &s.user_agent)
            .field(
                "alternates",
                &s.alternates
                    .iter()
                    .map(|a| crate::sources::shown(a))
                    .collect::<Vec<_>>(),
            )
            .field("origin", &s.origin)
            .field("live", &s.live)
            .finish()
    }
}

/// Everything one ingest of one source found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Harvest {
    /// Its categories.
    pub categories: Vec<NewCategory>,
    /// Its items.
    pub items: Vec<NewItem>,
    /// There was more than the bounds let in.
    pub cut: bool,
}

impl Harvest {
    /// The category `key`, added once: the first title and theme stay.
    pub fn category(&mut self, key: &str, title: &str, theme: Option<&str>, adult: bool) {
        if !self.categories.iter().any(|c| c.key == key) {
            self.categories.push(NewCategory {
                key: key.to_owned(),
                title: title.to_owned(),
                theme: theme.map(ToOwned::to_owned),
                adult,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// WHAT A DOOR ASKS AND GETS
// ---------------------------------------------------------------------------

/// Which items a listing takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// One category's.
    Category(CategoryId),
    /// Every category's filed under this theme.
    Theme(String),
    /// Every item.
    AllCategories,
}

/// From which sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sources {
    /// One: its id.
    One(String),
    /// Every one.
    All,
}

/// In which order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sort {
    /// By title, accents and case folded.
    Title,
    /// The most downloaded or watched first.
    Popular,
    /// The newest first (published, else year); the undated last.
    Newest,
    /// The oldest first (year, else published); the undated last.
    Oldest,
}

impl Sort {
    /// Every order, in the ring's order.
    pub const ALL: [Self; 4] = [Self::Title, Self::Popular, Self::Newest, Self::Oldest];

    /// Its kebab-case name, the suffix of its text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Popular => "popular",
            Self::Newest => "newest",
            Self::Oldest => "oldest",
        }
    }
}

/// One listing asked of a [`Reader`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListQuery {
    /// Which items.
    pub scope: Scope,
    /// From which sources.
    pub sources: Sources,
    /// Only titles filed under this letter ([`letter_of`]; `#` for digits).
    pub letter: Option<char>,
    /// Only items these words find (title, other names, description; every
    /// word, each a prefix, accents and case folded).
    pub words: Option<String>,
    /// Only this country (ISO alpha-2).
    pub country: Option<String>,
    /// Only this language, as the sources write it.
    pub language: Option<String>,
    /// Only pictures at least this tall, in lines.
    pub min_height: Option<u16>,
    /// The order.
    pub sort: Sort,
    /// Which page, from 1; past the last, the last.
    pub page: u32,
    /// How many rows a page holds (at most [`PER_PAGE_MOST`]).
    pub per_page: u32,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            scope: Scope::AllCategories,
            sources: Sources::All,
            letter: None,
            words: None,
            country: None,
            language: None,
            min_height: None,
            sort: Sort::Title,
            page: 1,
            per_page: 50,
        }
    }
}

/// One row of a listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// Its id.
    pub id: ItemId,
    /// Its source's id.
    pub source: String,
    /// What it is.
    pub kind: ItemKind,
    /// What it is called (data).
    pub title: String,
    /// The letter it is filed under.
    pub letter: char,
    /// Its country, when known.
    pub country: Option<String>,
    /// Its language, when known.
    pub language: Option<String>,
    /// Its picture's height, in lines, when known.
    pub height: Option<u16>,
    /// How long it runs, in seconds, when known.
    pub duration_s: Option<u32>,
    /// Its year, when known.
    pub year: Option<i32>,
    /// How popular its source says it is.
    pub popularity: u64,
    /// When it was published, in seconds since the epoch, when known.
    pub published_s: Option<i64>,
    /// Its poster's address, when it has one.
    pub poster_url: Option<String>,
}

/// One page of a listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListPage {
    /// Its rows, in order.
    pub rows: Vec<Row>,
    /// How many items the listing has in all.
    pub total: u64,
    /// Which page this is, from 1.
    pub page: u32,
    /// How many pages there are (at least 1).
    pub pages: u32,
}

/// An item, whole: what its page shows and what plays.
///
/// Not `Serialize`, and its `Debug` masks its stream's addresses: it carries
/// a provider's address with the person's account in it, and a door reads it
/// from the file, never from a wire.
#[derive(Clone, PartialEq)]
pub struct Item {
    /// Its row.
    pub row: Row,
    /// Its other names and words.
    pub also: String,
    /// What it says about itself, when it says.
    pub description: Option<String>,
    /// Its rights, as its source says them.
    pub licence: Option<String>,
    /// What the queue holds when it is played.
    pub stream: Stream,
    /// The categories it is in.
    pub categories: Vec<CategoryId>,
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Item")
            .field("row", &self.row)
            .field("also", &self.also)
            .field("description", &self.description)
            .field("licence", &self.licence)
            .field("stream", &Masked(&self.stream))
            .field("categories", &self.categories)
            .finish()
    }
}

/// A source, as the catalogue last saw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRow {
    /// Its id.
    pub id: String,
    /// What kind it is.
    pub kind: SourceKind,
    /// Its own name (data), or empty.
    pub title: String,
    /// Its address, masked.
    pub shown: String,
    /// Marked adult.
    pub adult: bool,
    /// Its posters are shown.
    pub posters_on: bool,
    /// When it was last ingested, in seconds since the epoch.
    pub last_ingest_s: Option<i64>,
    /// Why the last ingest failed, as a word (`Refused::id`), when it did.
    pub last_error: Option<String>,
    /// How many items it has in this store.
    pub items: u64,
    /// Its last ingest found more than the bounds let in.
    pub cut: bool,
}

/// A category, as a door lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryRow {
    /// Its id.
    pub id: CategoryId,
    /// Its source's id.
    pub source: String,
    /// Its key in the source.
    pub key: String,
    /// Its source's name for it (data), or empty.
    pub title: String,
    /// Its theme, when filed under one.
    pub theme: Option<String>,
    /// How many items it has.
    pub items: u64,
}

/// Why the catalogue did not do what was asked, as a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trouble {
    /// There is no cache root to keep it in.
    NoHome,
    /// There is no catalogue yet: nothing was ingested.
    Absent,
    /// The file is not this build's — another layout, a value this build
    /// does not write, not a catalogue at all — and waits to be made again.
    OtherSchema,
    /// It could not be opened or read.
    Unreadable,
    /// It could not be written.
    Unwritable,
    /// The ingest would pass `playerCatalogMib`; nothing changed.
    TooLarge,
}

impl Trouble {
    /// Every trouble, for a test that walks the words.
    pub const ALL: [Self; 6] = [
        Self::NoHome,
        Self::Absent,
        Self::OtherSchema,
        Self::Unreadable,
        Self::Unwritable,
        Self::TooLarge,
    ];

    /// Its kebab-case name, the suffix of the key a door says it with.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoHome => "no-home",
            Self::Absent => "absent",
            Self::OtherSchema => "other-schema",
            Self::Unreadable => "unreadable",
            Self::Unwritable => "unwritable",
            Self::TooLarge => "too-large",
        }
    }
}

/// The words a search asks FTS5 for: each word of the person's, quoted (so
/// nothing in it is syntax) and a prefix. `None` when there are none.
#[must_use]
pub fn fts_query(words: &str) -> Option<String> {
    let parts: Vec<String> = words
        .split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .take(16)
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// A description kept for the search: at most [`DESCRIPTION_MOST`]
/// characters.
#[cfg(feature = "catalog")]
fn cut_description(text: &str) -> Cow<'_, str> {
    match text.char_indices().nth(DESCRIPTION_MOST) {
        Some((at, _)) => Cow::Owned(text[..at].to_owned()),
        None => Cow::Borrowed(text),
    }
}

#[cfg(feature = "catalog")]
mod db;

#[cfg(feature = "catalog")]
pub use db::{Reader, Replaced, Writer};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_takes_accents_and_case_off_in_every_script() {
        // pt-BR: the accent IS the subject — this is what folding folds.
        assert_eq!(fold("  Ação   Popular "), "acao popular");
        assert_eq!(fold("Ñandú"), "nandu");
        assert_eq!(fold("Ёлка"), "елка");
        assert_eq!(fold("Ἀθῆναι"), "αθηναι");
        assert_eq!(fold("Tiếng Việt"), "tieng viet");
        assert_eq!(fold("東京"), "東京", "nothing to take off");
        // A title already decomposed folds the same.
        assert_eq!(fold("Ac\u{0327}a\u{0303}o"), "acao");
    }

    #[test]
    fn a_title_sorts_by_its_first_letter_or_digit() {
        assert_eq!(sort_key("\"Casablanca\""), "casablanca\"");
        assert_eq!(sort_key("¡Hola!"), "hola!");
        assert_eq!(sort_key("…"), "…");
        assert!(sort_key("Émile") < sort_key("Zorro"), "É sorts with E");
    }

    /// FAILS IF: a letter a list shows stops being the one its titles start
    /// with — Ç and Ñ lost, É kept apart from E, a digit not `#`.
    #[test]
    fn a_title_is_filed_under_its_letter() {
        assert_eq!(letter_of("Élan"), 'E');
        assert_eq!(letter_of("ação"), 'A');
        assert_eq!(letter_of("Ñu"), 'Ñ');
        assert_eq!(letter_of("çava"), 'Ç');
        assert_eq!(letter_of("N\u{0303}u"), 'Ñ', "decomposed, still Ñ");
        assert_eq!(letter_of("йогурт"), 'Й');
        assert_eq!(letter_of("Ёж"), 'Ё');
        assert_eq!(letter_of("Άλφα"), 'Α');
        assert_eq!(letter_of("東京テレビ"), '東');
        assert_eq!(letter_of("24 Horas"), '#');
        assert_eq!(letter_of("  ...  "), '#');
        assert_eq!(letter_of("\"The Kid\""), 'T');
        assert_eq!(letter_of("ßeta"), 'ß', "two capitals are not one letter");
    }

    #[test]
    fn words_are_folded_and_a_plus_is_part_of_one() {
        assert_eq!(words_of("Adultos 18+ | XXX"), ["adultos", "18+", "xxx"]);
        assert_eq!(words_of("Sci-Fi & Horror"), ["sci", "fi", "horror"]);
        // pt-BR: a group's name as a person's list writes it, accents and all.
        let hay = words_of("Filmes de Ficção Científica");
        assert!(has_phrase(&hay, &words_of("ficcao cientifica")));
        // pt-BR: the same two words, out of order.
        assert!(!has_phrase(&hay, &words_of("cientifica ficcao")));
        assert!(!has_phrase(&hay, &[]));
    }

    #[test]
    fn ids_are_stable_positive_and_apart() {
        let a = ItemId::of("iptv", "BBCNews.uk");
        assert_eq!(a, ItemId::of("iptv", "BBCNews.uk"), "stable");
        assert!(a.0 > 0);
        assert_ne!(a, ItemId::of("iptv2", "BBCNews.uk"));
        assert_ne!(ItemId::of("ab", "c"), ItemId::of("a", "bc"));
        assert_ne!(CategoryId::of("s", "k").0, ItemId::of("s", "k").0);
        // THE NUMBER ITSELF IS PINNED: a changed hash would move every
        // cursor a door keeps, and nothing else would fail.
        assert_eq!(digest("x"), "af63f54c86021707", "FNV-1a of `x`");
    }

    #[test]
    fn a_search_is_words_and_never_syntax() {
        assert_eq!(
            fts_query("  música  \"ao vivo\" "),
            Some("\"música\"* \"ao\"* \"vivo\"*".to_owned())
        );
        assert_eq!(
            fts_query("NEAR( OR *"),
            Some("\"NEAR(\"* \"OR\"*".to_owned())
        );
        assert_eq!(fts_query("  \" - "), None);
    }

    #[test]
    fn every_name_is_its_serde_name() {
        for t in Trouble::ALL {
            assert_eq!(
                serde_json::to_string(&t).unwrap(),
                format!("\"{}\"", t.id())
            );
        }
        for s in Sort::ALL {
            assert_eq!(
                serde_json::to_string(&s).unwrap(),
                format!("\"{}\"", s.id())
            );
        }
        for k in ItemKind::ALL {
            assert_eq!(ItemKind::of(k.word()), Some(k));
        }
    }

    /// FAILS IF: an item printed by `Debug` — in a log, a panic, a test's
    /// failure — shows the person's account a provider wrote into its
    /// stream's addresses.
    #[test]
    fn an_item_prints_its_stream_masked() {
        let mut stream = Stream::typed("http://iptv.example:8080/movie/ana/hunter2/123.mp4");
        stream.alternates = vec!["http://backup.example/live/ana/hunter2/9.ts".to_owned()];
        stream.referrer = Some("http://iptv.example/panel?password=hunter2".to_owned());
        let item = NewItem {
            ext_id: "123".to_owned(),
            kind: ItemKind::Video,
            title: "A film".to_owned(),
            also: Vec::new(),
            description: None,
            country: None,
            language: None,
            height: None,
            duration_s: None,
            year: None,
            licence: None,
            popularity: 0,
            published_s: None,
            poster_url: None,
            stream: stream.clone(),
            categories: Vec::new(),
        };
        let harvest = Harvest {
            categories: Vec::new(),
            items: vec![item],
            cut: false,
        };
        let whole = Item {
            row: Row {
                id: ItemId(1),
                source: "p1".to_owned(),
                kind: ItemKind::Video,
                title: "A film".to_owned(),
                letter: 'A',
                country: None,
                language: None,
                height: None,
                duration_s: None,
                year: None,
                popularity: 0,
                published_s: None,
                poster_url: None,
            },
            also: String::new(),
            description: None,
            licence: None,
            stream,
            categories: Vec::new(),
        };
        for printed in [format!("{harvest:?}"), format!("{whole:?}")] {
            assert!(!printed.contains("hunter2"), "{printed}");
            assert!(!printed.contains("ana"), "{printed}");
            assert!(printed.contains("\"iptv.example/…\""), "{printed}");
            assert!(printed.contains("\"backup.example/…\""), "{printed}");
            assert!(printed.contains("A film"), "the rest is printed: {printed}");
        }
    }
}
