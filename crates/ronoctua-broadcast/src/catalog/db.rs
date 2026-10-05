//! The two files themselves: the schema, the one writer and the readers.
//!
//! Every statement the catalogue runs is in this file (`one-sqlite-boundary`
//! keeps `rusqlite` inside this crate and `ronoctua-video-metrics`).
//!
//! # Every listing is walked on an index
//!
//! A door asks a page of a listing — a source's items, a category's, a
//! theme's — in one of four orders, at any page, `<end>` included, filtered
//! by a letter, a country, a language, a height or words. An `ORDER BY` that
//! no index serves sorts the whole listing to hand back fifty rows
//! (measured: 100–160 ms over fifty thousand items), so every order has an
//! index for every scope: over the items (all of them, and one source's),
//! over a category's members (`item_categories`) and over a theme's
//! (`item_themes`). Each index holds the order's keys, the item's id right
//! after them (the last tie), and then what a filter asks — the letter, the
//! country, the language, the height — so a walk, its count and a facet
//! read one index and nothing else, and only the page's rows are joined to
//! their items ([`walk`]).
//!
//! Words are searched in `items_fts`, where each item also carries a token
//! for every list it is in — its source's, each of its categories', each of
//! its themes' ([`scope_token`]). A search in a scope is then one question
//! to the text index (`words AND scopes : c…`): counted without a row looked
//! up, and walked against a set no larger than the scope.
//!
//! A page nearer the end of its listing is walked FROM the end, in the
//! reverse order, so the last page costs what the first does; and the count
//! and the page are read in one snapshot, so a commit between them never
//! gives a page of another total.
//!
//! The dates an order compares are one number each, written with the item:
//! `newest` (published, else the start of its year; the undated last) and
//! `oldest` (its year, else published; the undated last). A theme is a list
//! of its own — one per source and one across every source — written at
//! ingest from the categories' themes, so a theme page is a list and not a
//! union computed per question.

use std::collections::{BTreeSet, HashMap};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, OptionalExtension as _, TransactionBehavior, params};

use super::{
    CategoryId, CategoryRow, Harvest, Item, ItemId, ItemKind, ListPage, ListQuery, NewItem,
    NewSource, PER_PAGE_MOST, Row, SCHEMA, Scope, Sort, SourceRow, Sources, Store, Trouble,
    cut_description, fts_query, letter_of, sort_key, theme_list,
};
use crate::sources::Kind as SourceKind;
use ronoctua_playlist::own::{ALTERNATES_MOST, Stream};

/// How long the writer waits for a reader's lock before it gives up.
const WRITER_BUSY: Duration = Duration::from_secs(5);

/// How long a reader waits for the writer's checkpoint.
const READER_BUSY: Duration = Duration::from_secs(2);

/// The most bytes a `-wal` file keeps after a checkpoint (SQLite truncates it
/// to this): an ingest of fifty thousand rows grows it to tens of megabytes,
/// and nothing needs them once they are in the database.
const WAL_KEPT_BYTES: i64 = 4 << 20;

/// The writer's page cache while it writes, in KiB. SQLite's default is 2 MiB,
/// and an ingest keeps a dozen indexes of random keys: measured on a replica
/// of the fifty-thousand-item catalogue the tests time, 225 s with the
/// default and 24 s with this. Given back after each write
/// (`shrink_memory`).
const WRITER_CACHE_KIB: i64 = 64 * 1024;

/// What an undated item sorts by among the newest: after every date.
const UNDATED_NEWEST: i64 = i64::MIN;

/// What an undated item sorts by among the oldest: after every date.
const UNDATED_OLDEST: i64 = i64::MAX;

/// The layout. `auto_vacuum` is set before any table exists (it cannot be
/// changed after without a `VACUUM`), so what a replaced source frees is
/// given back to the disk by `incremental_vacuum`.
///
/// A change here is a new layout: raise [`SCHEMA`] (the test
/// `the_layout_has_the_number_it_says` pins the text to the number).
pub(super) const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS meta(
    key TEXT PRIMARY KEY,
    value INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS sources(
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    shown TEXT NOT NULL,
    adult INTEGER NOT NULL,
    posters_on INTEGER NOT NULL,
    last_ingest INTEGER,
    last_error TEXT,
    items INTEGER NOT NULL DEFAULT 0,
    cut INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS categories(
    id INTEGER PRIMARY KEY,
    source TEXT NOT NULL,
    key TEXT NOT NULL,
    title TEXT NOT NULL,
    theme TEXT,
    adult INTEGER NOT NULL,
    items INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS categories_source ON categories(source);
CREATE TABLE IF NOT EXISTS theme_lists(
    id INTEGER PRIMARY KEY,
    theme TEXT NOT NULL,
    source TEXT,
    items INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS theme_lists_source ON theme_lists(source);
CREATE TABLE IF NOT EXISTS items(
    id INTEGER PRIMARY KEY,
    source TEXT NOT NULL,
    ext_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    title_folded TEXT NOT NULL,
    letter TEXT NOT NULL,
    country TEXT,
    language TEXT,
    quality INTEGER,
    duration_s INTEGER,
    year INTEGER,
    licence TEXT,
    popularity INTEGER NOT NULL DEFAULT 0,
    published INTEGER,
    newest INTEGER NOT NULL,
    oldest INTEGER NOT NULL,
    poster_url TEXT,
    stream_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS items_title ON items(title_folded, id, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS items_popularity ON items(popularity DESC, title_folded, id, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS items_newest ON items(newest DESC, title_folded, id, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS items_oldest ON items(oldest, title_folded, id, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS items_source_title ON items(source, title_folded, id, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS items_source_popularity ON items(source, popularity DESC, title_folded, id, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS items_source_newest ON items(source, newest DESC, title_folded, id, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS items_source_oldest ON items(source, oldest, title_folded, id, letter, country, language, quality);
CREATE TABLE IF NOT EXISTS item_categories(
    category INTEGER NOT NULL,
    item INTEGER NOT NULL,
    title_folded TEXT NOT NULL,
    popularity INTEGER NOT NULL,
    newest INTEGER NOT NULL,
    oldest INTEGER NOT NULL,
    letter TEXT NOT NULL,
    country TEXT,
    language TEXT,
    quality INTEGER,
    PRIMARY KEY(category, item)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS item_categories_title ON item_categories(category, title_folded, item, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS item_categories_popularity ON item_categories(category, popularity DESC, title_folded, item, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS item_categories_newest ON item_categories(category, newest DESC, title_folded, item, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS item_categories_oldest ON item_categories(category, oldest, title_folded, item, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS item_categories_item ON item_categories(item);
CREATE TABLE IF NOT EXISTS item_themes(
    theme_list INTEGER NOT NULL,
    item INTEGER NOT NULL,
    title_folded TEXT NOT NULL,
    popularity INTEGER NOT NULL,
    newest INTEGER NOT NULL,
    oldest INTEGER NOT NULL,
    letter TEXT NOT NULL,
    country TEXT,
    language TEXT,
    quality INTEGER,
    PRIMARY KEY(theme_list, item)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS item_themes_title ON item_themes(theme_list, title_folded, item, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS item_themes_popularity ON item_themes(theme_list, popularity DESC, title_folded, item, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS item_themes_newest ON item_themes(theme_list, newest DESC, title_folded, item, letter, country, language, quality);
CREATE INDEX IF NOT EXISTS item_themes_oldest ON item_themes(theme_list, oldest, title_folded, item, letter, country, language, quality);
CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
    title, also, description, scopes,
    tokenize = 'unicode61 remove_diacritics 2'
);
";

// ---------------------------------------------------------------------------
// TROUBLE: the only place a `rusqlite::Error` is read.
// ---------------------------------------------------------------------------

/// A value this build never writes: the row is another build's.
#[derive(Debug)]
struct Foreign;

impl std::fmt::Display for Foreign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a value this build does not write")
    }
}

impl std::error::Error for Foreign {}

/// The error a column read with [`Foreign`] in it.
fn foreign(column: usize) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        Box::new(Foreign),
    )
}

/// Whether an error says the file is not a catalogue at all — not a
/// database, or a damaged one — rather than that it could not be reached.
fn is_not_a_catalogue(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
    )
}

/// A reader's trouble: a value this build does not write, or a file that is
/// not a catalogue, is [`Trouble::OtherSchema`] (the writer makes the file
/// again); anything else is [`Trouble::Unreadable`].
fn read_trouble(e: rusqlite::Error) -> Trouble {
    let another = is_not_a_catalogue(&e)
        || matches!(
            e,
            rusqlite::Error::FromSqlConversionFailure(..)
                | rusqlite::Error::IntegralValueOutOfRange(..)
                | rusqlite::Error::InvalidColumnType(..)
        );
    if another {
        Trouble::OtherSchema
    } else {
        Trouble::Unreadable
    }
}

fn unwritable(e: rusqlite::Error) -> Trouble {
    match e.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DiskFull) => Trouble::TooLarge,
        _ => Trouble::Unwritable,
    }
}

/// A number the database keeps as a signed 64-bit integer.
fn signed(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A count or a popularity read back: never negative in a file this build
/// wrote.
fn count_at(r: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<u64> {
    let n: i64 = r.get(column)?;
    u64::try_from(n).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(column, n))
}

/// An item's kind read back: a word this build does not know is another
/// build's row, never a guess.
fn kind_at(r: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<ItemKind> {
    let word: String = r.get(column)?;
    ItemKind::of(&word).ok_or_else(|| foreign(column))
}

/// A source's kind read back, the same way.
fn source_kind_at(r: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<SourceKind> {
    let word: String = r.get(column)?;
    SourceKind::of(&word).ok_or_else(|| foreign(column))
}

/// A letter read back: one character, as [`letter_of`] writes it.
fn letter_at(r: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<char> {
    let text: String = r.get(column)?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(one), None) => Ok(one),
        _ => Err(foreign(column)),
    }
}

// ---------------------------------------------------------------------------
// A FILE, FOUND AND MADE
// ---------------------------------------------------------------------------

/// Creates the file `0600` before SQLite first opens it — its `-wal` and
/// `-shm` are created with the database's mode — or tightens one that was
/// there.
fn private_file(path: &Path) -> Result<(), Trouble> {
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| Trouble::Unwritable)
        }
        Err(_) => Err(Trouble::Unwritable),
    }
}

/// What a file holds, asked before anything is written to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Found {
    /// Nothing yet: a file with no table in it.
    Nothing,
    /// This build's layout.
    Ours,
    /// Anything else: another layout, or tables with no `meta` saying which.
    Other,
}

/// What `conn`'s file holds. Reads only: `sqlite_master`, and `meta` when it
/// has the two columns a `meta` of any build of ours has.
///
/// # Errors
///
/// What SQLite says — a file that is not a database is
/// [`is_not_a_catalogue`].
fn found(conn: &Connection) -> rusqlite::Result<Found> {
    let objects: i64 = conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))?;
    if objects == 0 {
        return Ok(Found::Nothing);
    }
    let columns: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('meta') WHERE name IN ('key', 'value')",
        [],
        |r| r.get(0),
    )?;
    if columns != 2 {
        return Ok(Found::Other);
    }
    let schema: Option<Value> = conn
        .query_row("SELECT value FROM meta WHERE key = 'schema'", [], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(if schema == Some(Value::Integer(SCHEMA)) {
        Found::Ours
    } else {
        Found::Other
    })
}

/// Opens (creating it `0600`) one store's file for writing, touching nothing
/// in it.
fn open_file(path: &Path) -> Result<Connection, Trouble> {
    private_file(path)?;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| Trouble::Unwritable)?;
    conn.busy_timeout(WRITER_BUSY).map_err(unwritable)?;
    Ok(conn)
}

/// Gives a file that holds nothing or this build's layout the layout (a
/// no-op on one that has it), in WAL, in one transaction: a reader sees no
/// table or all of them.
fn laid_out(mut conn: Connection, now_s: i64) -> Result<Connection, Trouble> {
    conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL;")
        .map_err(unwritable)?;
    let mode: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))
        .map_err(unwritable)?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Trouble::Unwritable);
    }
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(unwritable)?;
    conn.pragma_update(None, "journal_size_limit", WAL_KEPT_BYTES)
        .map_err(unwritable)?;
    conn.pragma_update(None, "cache_size", -WRITER_CACHE_KIB)
        .map_err(unwritable)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unwritable)?;
    tx.execute_batch(SCHEMA_SQL).map_err(unwritable)?;
    tx.execute(
        "INSERT OR IGNORE INTO meta(key, value) VALUES ('schema', ?1)",
        [SCHEMA],
    )
    .map_err(unwritable)?;
    // THE GENERATION STARTS AT THE CLOCK, so a file made again after a
    // rebuild never answers a number a door already saw.
    tx.execute(
        "INSERT OR IGNORE INTO meta(key, value) VALUES ('generation', ?1)",
        [now_s.max(1)],
    )
    .map_err(unwritable)?;
    tx.commit().map_err(unwritable)?;
    Ok(conn)
}

/// Removes a store's file and its two companions.
fn forget_files(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let _ = std::fs::remove_file(PathBuf::from(name));
    }
}

/// Opens one store for writing, making it again when it is not this build's:
/// another layout, tables with no `meta`, a file that is not a database or is
/// damaged. What it holds is asked BEFORE anything is written, so nothing of
/// this layout is run against another's tables.
fn store_for_writing(dir: &Path, store: Store, now_s: i64) -> Result<Connection, Trouble> {
    let path = dir.join(store.file());
    let conn = open_file(&path)?;
    let again = match found(&conn) {
        Ok(Found::Nothing | Found::Ours) => false,
        Ok(Found::Other) => true,
        Err(e) if is_not_a_catalogue(&e) => true,
        Err(_) => return Err(Trouble::Unwritable),
    };
    if !again {
        return laid_out(conn, now_s);
    }
    // ANOTHER LAYOUT, OR NONE: a cache is made again, never migrated.
    drop(conn);
    forget_files(&path);
    laid_out(open_file(&path)?, now_s)
}

/// What the file holds, in bytes: its pages that are in use (inside a
/// transaction, with what it has written so far).
fn used_bytes(conn: &Connection) -> Result<u64, Trouble> {
    let pages: i64 = conn
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .map_err(unwritable)?;
    let free: i64 = conn
        .query_row("PRAGMA freelist_count", [], |r| r.get(0))
        .map_err(unwritable)?;
    let size: i64 = conn
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .map_err(unwritable)?;
    Ok(u64::try_from((pages - free).max(0).saturating_mul(size)).unwrap_or(0))
}

// ---------------------------------------------------------------------------
// THE WRITER
// ---------------------------------------------------------------------------

/// The catalogue's one writer: the daemon's.
///
/// Not `Sync`: the daemon's ingest threads share it behind a lock, and one
/// source is written at a time.
pub struct Writer {
    dir: PathBuf,
    main: Connection,
    adult: Connection,
    most_bytes: u64,
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer")
            .field("dir", &self.dir)
            .field("most_bytes", &self.most_bytes)
            .finish_non_exhaustive()
    }
}

/// How many items a replace kept, in each store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Replaced {
    /// In the main file.
    pub main: u64,
    /// In the adult file.
    pub adult: u64,
}

/// An item of the harvest, merged with the ones that share its id.
struct Merged<'a> {
    first: &'a NewItem,
    categories: Vec<&'a str>,
    stream: Stream,
}

/// The harvest's items, one per id: the first keeps its fields, the others
/// add their categories and, when their address differs, an alternate.
fn merged(items: &[NewItem]) -> Vec<Merged<'_>> {
    let mut at: HashMap<&str, usize> = HashMap::with_capacity(items.len());
    let mut out: Vec<Merged<'_>> = Vec::with_capacity(items.len());
    for item in items {
        match at.get(item.ext_id.as_str()) {
            Some(&n) => {
                let m = &mut out[n];
                for c in &item.categories {
                    if !m.categories.contains(&c.as_str()) {
                        m.categories.push(c);
                    }
                }
                let s = &item.stream;
                if s.url != m.stream.url
                    && !m.stream.alternates.contains(&s.url)
                    && m.stream.alternates.len() < ALTERNATES_MOST
                    && s.referrer == m.stream.referrer
                    && s.user_agent == m.stream.user_agent
                {
                    m.stream.alternates.push(s.url.clone());
                }
            }
            None => {
                at.insert(&item.ext_id, out.len());
                out.push(Merged {
                    first: item,
                    categories: item.categories.iter().map(String::as_str).collect(),
                    stream: item.stream.clone(),
                });
            }
        }
    }
    out
}

/// The two numbers an item's dates sort by: `(newest, oldest)`.
///
/// Among the newest, its publication, else the start of its year; among the
/// oldest, the start of its year, else its publication; the undated last in
/// both.
fn dated(item: &NewItem) -> (i64, i64) {
    let year = item.year.and_then(crate::date::year_start);
    (
        item.published_s.or(year).unwrap_or(UNDATED_NEWEST),
        year.or(item.published_s).unwrap_or(UNDATED_OLDEST),
    )
}

impl Writer {
    /// The writer of the catalogue in `dir`, kept under `most_bytes` (the two
    /// files together). Makes the folder `0700` and each file `0600`; a file
    /// that is not this build's — another layout, tables with no `meta`, not
    /// a database, damaged — is made again, and a catalogue found over its
    /// ceiling is emptied. `now_s` is the clock, in seconds since the epoch.
    ///
    /// A door's [`Reader`] of a file made again keeps the old one open:
    /// [`Reader::replaced`] tells it to open a new one.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unwritable`] when the folder or a file cannot be made.
    pub fn open(dir: &Path, most_bytes: u64, now_s: i64) -> Result<Self, Trouble> {
        ronoctua_consent::keys::Dir::at(dir.to_owned())
            .make()
            .map_err(|_| Trouble::Unwritable)?;
        let mut writer = Self {
            dir: dir.to_owned(),
            main: store_for_writing(dir, Store::Main, now_s)?,
            adult: store_for_writing(dir, Store::Adult, now_s)?,
            most_bytes,
        };
        if writer.bytes()? > most_bytes {
            writer.forget_all(now_s)?;
        }
        Ok(writer)
    }

    /// This instance's catalogue ([`super::folder`]).
    ///
    /// # Errors
    ///
    /// [`Trouble::NoHome`], and as [`Writer::open`].
    pub fn player(most_bytes: u64, now_s: i64) -> Result<Self, Trouble> {
        let dir = super::folder().ok_or(Trouble::NoHome)?;
        Self::open(&dir, most_bytes, now_s)
    }

    /// Where it is.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// What the two files hold, in bytes.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unwritable`] when a file cannot be asked.
    pub fn bytes(&self) -> Result<u64, Trouble> {
        Ok(used_bytes(&self.main)?.saturating_add(used_bytes(&self.adult)?))
    }

    fn conn(&mut self, store: Store) -> &mut Connection {
        match store {
            Store::Main => &mut self.main,
            Store::Adult => &mut self.adult,
        }
    }

    /// Empties both files by making them again.
    fn forget_all(&mut self, now_s: i64) -> Result<(), Trouble> {
        for store in Store::ALL {
            let path = self.dir.join(store.file());
            let placeholder = Connection::open_in_memory().map_err(unwritable)?;
            drop(std::mem::replace(self.conn(store), placeholder));
            forget_files(&path);
            *self.conn(store) = laid_out(open_file(&path)?, now_s)?;
        }
        Ok(())
    }

    /// Replaces every row of `source` with what `harvest` found: one
    /// transaction per file, both written before either is committed, and
    /// committed together only when the two fit under the ceiling — a reader
    /// sees the old rows or the new ones, in both. An adult category — or
    /// every category of a source marked adult — and its items go to the
    /// adult file; an item in any adult category is adult.
    ///
    /// The adult file commits first, so the one way the two can disagree — a
    /// failure between the two commits (the disk, a crash) — leaves nothing
    /// adult in the main file; the next ingest of the source makes them agree.
    ///
    /// # Errors
    ///
    /// [`Trouble::TooLarge`] when the two files would pass the ceiling (both
    /// are rolled back, and the source keeps its old rows in each),
    /// [`Trouble::Unwritable`].
    pub fn replace(
        &mut self,
        source: &NewSource,
        harvest: &Harvest,
        now_s: i64,
    ) -> Result<Replaced, Trouble> {
        let adult_key: HashMap<&str, bool> = harvest
            .categories
            .iter()
            .map(|c| (c.key.as_str(), source.adult || c.adult))
            .collect();
        let items = merged(&harvest.items);
        let is_adult = |m: &Merged<'_>| {
            source.adult
                || m.categories
                    .iter()
                    .any(|c| adult_key.get(c).copied().unwrap_or(false))
        };
        let half = |store: Store| {
            let here = store == Store::Adult;
            let categories: Vec<&super::NewCategory> = harvest
                .categories
                .iter()
                .filter(|c| (source.adult || c.adult) == here)
                .collect();
            let chosen: Vec<&Merged<'_>> = items.iter().filter(|m| is_adult(m) == here).collect();
            // A SOURCE'S ROW is in the main file (unless it is adult), and in
            // the adult file only while it has adult rows there.
            let keep_row = if here {
                source.adult || !categories.is_empty() || !chosen.is_empty()
            } else {
                !source.adult
            };
            Half {
                categories,
                items: chosen,
                keep_row,
            }
        };
        let (adult_half, main_half) = (half(Store::Adult), half(Store::Main));
        let most = self.most_bytes;
        let adult_tx = self
            .adult
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unwritable)?;
        let adult = write_source(&adult_tx, source, &adult_half, harvest.cut, now_s)?;
        let main_tx = self
            .main
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unwritable)?;
        let main = write_source(&main_tx, source, &main_half, harvest.cut, now_s)?;
        // THE CEILING, measured inside both transactions: what the two files
        // would hold with these rows in them. Over it, both are dropped —
        // rolled back — and nothing changed.
        if used_bytes(&adult_tx)?.saturating_add(used_bytes(&main_tx)?) > most {
            return Err(Trouble::TooLarge);
        }
        adult_tx.commit().map_err(unwritable)?;
        main_tx.commit().map_err(unwritable)?;
        // WHAT THE OLD ROWS HELD goes back to the disk, and the cache's
        // memory to the daemon.
        for store in Store::ALL {
            let _ = self
                .conn(store)
                .execute_batch("PRAGMA incremental_vacuum; PRAGMA shrink_memory;");
        }
        Ok(Replaced { main, adult })
    }

    /// Records that an ingest of `source` failed, for `why` (a refusal's
    /// word): its rows stay as they were, and a door says the source is stale
    /// and why.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unwritable`].
    pub fn failed(&mut self, source: &NewSource, why: &str) -> Result<(), Trouble> {
        let store = if source.adult {
            Store::Adult
        } else {
            Store::Main
        };
        let conn = self.conn(store);
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unwritable)?;
        tx.execute(
            "INSERT INTO sources(id, kind, title, shown, adult, posters_on, last_ingest, last_error, items, cut)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, 0, 0)
             ON CONFLICT(id) DO UPDATE SET last_error = excluded.last_error, shown = excluded.shown",
            params![
                source.id,
                source.kind.id(),
                source.title,
                source.shown,
                source.adult,
                source.posters_on,
                why
            ],
        )
        .map_err(unwritable)?;
        bump(&tx)?;
        tx.commit().map_err(unwritable)
    }

    /// Removes `source` and every row of it, from both files.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unwritable`].
    pub fn remove(&mut self, source: &str) -> Result<(), Trouble> {
        for store in Store::ALL {
            let conn = self.conn(store);
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(unwritable)?;
            delete_rows(&tx, source)?;
            recount_themes(&tx, source)?;
            tx.execute("DELETE FROM sources WHERE id = ?1", [source])
                .map_err(unwritable)?;
            bump(&tx)?;
            tx.commit().map_err(unwritable)?;
            let _ = conn.execute_batch("PRAGMA incremental_vacuum;");
        }
        Ok(())
    }

    /// Records whether `source`'s posters are shown.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unwritable`].
    pub fn set_posters(&mut self, source: &str, on: bool) -> Result<(), Trouble> {
        for store in Store::ALL {
            let conn = self.conn(store);
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(unwritable)?;
            tx.execute(
                "UPDATE sources SET posters_on = ?2 WHERE id = ?1",
                params![source, on],
            )
            .map_err(unwritable)?;
            bump(&tx)?;
            tx.commit().map_err(unwritable)?;
        }
        Ok(())
    }
}

/// What one file of a replace holds.
struct Half<'h, 'a> {
    categories: Vec<&'h super::NewCategory>,
    items: Vec<&'h Merged<'a>>,
    keep_row: bool,
}

/// Raises the generation: the one number a door compares.
fn bump(tx: &rusqlite::Transaction<'_>) -> Result<(), Trouble> {
    tx.execute(
        "UPDATE meta SET value = value + 1 WHERE key = 'generation'",
        [],
    )
    .map_err(unwritable)
    .map(|_| ())
}

/// Deletes every row of `source` but its own.
fn delete_rows(tx: &rusqlite::Transaction<'_>, source: &str) -> Result<(), Trouble> {
    for sql in [
        "DELETE FROM items_fts WHERE rowid IN (SELECT id FROM items WHERE source = ?1)",
        "DELETE FROM item_categories WHERE category IN (SELECT id FROM categories WHERE source = ?1)",
        // ITS THEME LISTS whole, and its items out of the lists across every
        // source of its themes — asked before its lists and items go.
        "DELETE FROM item_themes WHERE theme_list IN (
             SELECT id FROM theme_lists WHERE source = ?1
             OR (source IS NULL AND theme IN (SELECT theme FROM theme_lists WHERE source = ?1)))
         AND item IN (SELECT id FROM items WHERE source = ?1)",
        "DELETE FROM items WHERE source = ?1",
        "DELETE FROM categories WHERE source = ?1",
        "DELETE FROM theme_lists WHERE source = ?1",
    ] {
        tx.execute(sql, [source]).map_err(unwritable)?;
    }
    Ok(())
}

/// Counts again the theme lists `source`'s rows are in — its own, and those
/// across every source — and drops the empty ones.
fn recount_themes(tx: &rusqlite::Transaction<'_>, source: &str) -> Result<(), Trouble> {
    tx.execute(
        "UPDATE theme_lists SET items = (SELECT count(*) FROM item_themes WHERE theme_list = theme_lists.id)
         WHERE source = ?1 OR source IS NULL",
        [source],
    )
    .map_err(unwritable)?;
    tx.execute("DELETE FROM theme_lists WHERE items = 0", [])
        .map_err(unwritable)?;
    Ok(())
}

/// One file's half of a replace, inside `tx`: the source's old rows out, the
/// new in. Gives back how many items the file holds for it.
fn write_source(
    tx: &rusqlite::Transaction<'_>,
    source: &NewSource,
    half: &Half<'_, '_>,
    cut: bool,
    now_s: i64,
) -> Result<u64, Trouble> {
    delete_rows(tx, &source.id)?;
    // THE THEMES of this file's categories, by key, and the keys themselves.
    let theme_of: HashMap<&str, &str> = half
        .categories
        .iter()
        .filter_map(|c| Some((c.key.as_str(), c.theme.as_deref()?)))
        .collect();
    let keys_here: BTreeSet<&str> = half.categories.iter().map(|c| c.key.as_str()).collect();
    {
        let mut add_category = tx
            .prepare_cached(
                "INSERT OR IGNORE INTO categories(id, source, key, title, theme, adult, items)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            )
            .map_err(unwritable)?;
        let mut add_theme = tx
            .prepare_cached(
                "INSERT OR IGNORE INTO theme_lists(id, theme, source, items) VALUES (?1, ?2, ?3, 0)",
            )
            .map_err(unwritable)?;
        for c in &half.categories {
            add_category
                .execute(params![
                    CategoryId::of(&source.id, &c.key).0,
                    source.id,
                    c.key,
                    c.title,
                    c.theme,
                    source.adult || c.adult,
                ])
                .map_err(unwritable)?;
            if let Some(theme) = c.theme.as_deref() {
                add_theme
                    .execute(params![
                        theme_list(Some(&source.id), theme),
                        theme,
                        source.id
                    ])
                    .map_err(unwritable)?;
                add_theme
                    .execute(params![
                        theme_list(None, theme),
                        theme,
                        Option::<&str>::None
                    ])
                    .map_err(unwritable)?;
            }
        }
        let mut add_item = tx
            .prepare_cached(
                "INSERT OR IGNORE INTO items(id, source, ext_id, kind, title, title_folded, letter,
                     country, language, quality, duration_s, year, licence, popularity, published,
                     newest, oldest, poster_url, stream_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                     ?17, ?18, ?19)",
            )
            .map_err(unwritable)?;
        let mut add_text = tx
            .prepare_cached(
                "INSERT INTO items_fts(rowid, title, also, description, scopes)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(unwritable)?;
        let mut add_member = tx
            .prepare_cached(
                "INSERT OR IGNORE INTO item_categories(category, item, title_folded, popularity,
                     newest, oldest, letter, country, language, quality)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
                 WHERE EXISTS (SELECT 1 FROM categories WHERE id = ?1)",
            )
            .map_err(unwritable)?;
        let mut add_to_theme = tx
            .prepare_cached(
                "INSERT OR IGNORE INTO item_themes(theme_list, item, title_folded, popularity,
                     newest, oldest, letter, country, language, quality)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )
            .map_err(unwritable)?;
        for m in &half.items {
            let item = m.first;
            let id = ItemId::of(&source.id, &item.ext_id).0;
            let folded = sort_key(&item.title);
            let letter = letter_of(&item.title).to_string();
            let popularity = signed(item.popularity);
            let (newest, oldest) = dated(item);
            let stream = serde_json::to_string(&m.stream).map_err(|_| Trouble::Unwritable)?;
            let added = add_item
                .execute(params![
                    id,
                    source.id,
                    item.ext_id,
                    item.kind.word(),
                    item.title,
                    folded,
                    letter,
                    item.country,
                    item.language,
                    item.height,
                    item.duration_s,
                    item.year,
                    item.licence,
                    popularity,
                    item.published_s,
                    newest,
                    oldest,
                    item.poster_url,
                    stream,
                ])
                .map_err(unwritable)?;
            if added == 0 {
                // TWO IDS ONE HASH: two in 2⁶³, and the second is left out
                // rather than written over the first.
                continue;
            }
            // ITS LISTS, as words of the text index: its source, the
            // categories it is in here, its themes' lists.
            let themes: BTreeSet<&str> = m
                .categories
                .iter()
                .filter_map(|k| theme_of.get(k).copied())
                .collect();
            let mut scopes = vec![scope_token(Scoped::Source(&source.id))];
            scopes.extend(
                m.categories
                    .iter()
                    .filter(|k| keys_here.contains(*k))
                    .map(|k| scope_token(Scoped::List(CategoryId::of(&source.id, k).0))),
            );
            for theme in &themes {
                for list in [theme_list(Some(&source.id), theme), theme_list(None, theme)] {
                    scopes.push(scope_token(Scoped::List(list)));
                }
            }
            add_text
                .execute(params![
                    id,
                    item.title,
                    item.also.join("\n"),
                    item.description.as_deref().map(cut_description),
                    scopes.join(" "),
                ])
                .map_err(unwritable)?;
            for key in &m.categories {
                add_member
                    .execute(params![
                        CategoryId::of(&source.id, key).0,
                        id,
                        folded,
                        popularity,
                        newest,
                        oldest,
                        letter,
                        item.country,
                        item.language,
                        item.height,
                    ])
                    .map_err(unwritable)?;
            }
            // ITS THEMES, once each whatever number of its categories share
            // one: in this source's list of it and in the one across sources.
            for theme in themes {
                for list in [theme_list(Some(&source.id), theme), theme_list(None, theme)] {
                    add_to_theme
                        .execute(params![
                            list,
                            id,
                            folded,
                            popularity,
                            newest,
                            oldest,
                            letter,
                            item.country,
                            item.language,
                            item.height,
                        ])
                        .map_err(unwritable)?;
                }
            }
        }
    }
    tx.execute(
        "UPDATE categories SET items = (SELECT count(*) FROM item_categories WHERE category = categories.id)
         WHERE source = ?1",
        [&source.id],
    )
    .map_err(unwritable)?;
    recount_themes(tx, &source.id)?;
    let count: i64 = tx
        .query_row(
            "SELECT count(*) FROM items WHERE source = ?1",
            [&source.id],
            |r| r.get(0),
        )
        .map_err(unwritable)?;
    if half.keep_row {
        tx.execute(
            "INSERT INTO sources(id, kind, title, shown, adult, posters_on, last_ingest, last_error, items, cut)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, title = excluded.title,
                 shown = excluded.shown, adult = excluded.adult, posters_on = excluded.posters_on,
                 last_ingest = excluded.last_ingest, last_error = NULL, items = excluded.items,
                 cut = excluded.cut",
            params![
                source.id,
                source.kind.id(),
                source.title,
                source.shown,
                source.adult,
                source.posters_on,
                now_s,
                count,
                cut
            ],
        )
        .map_err(unwritable)?;
    } else {
        tx.execute("DELETE FROM sources WHERE id = ?1", [&source.id])
            .map_err(unwritable)?;
    }
    bump(tx)?;
    Ok(u64::try_from(count).unwrap_or(0))
}

// ---------------------------------------------------------------------------
// A LISTING, BUILT
// ---------------------------------------------------------------------------

/// A list of members a listing may walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum List {
    /// One category's members.
    Category,
    /// One theme list's members (a source's theme, or a theme across all).
    Theme,
}

impl List {
    fn table(self) -> &'static str {
        match self {
            Self::Category => "item_categories",
            Self::Theme => "item_themes",
        }
    }
}

/// A term of a `WHERE`, and what it binds.
struct Term {
    sql: String,
    args: Vec<Value>,
}

fn term(sql: impl Into<String>, args: impl IntoIterator<Item = Value>) -> Term {
    Term {
        sql: sql.into(),
        args: args.into_iter().collect(),
    }
}

/// A listing, taken apart: what it walks — a list of members (alias `l`: one
/// category's, one theme's) or the items themselves (alias `i`) — the term
/// that says which (a list's id, a source), the filters (every one on what
/// is walked: the members carry what a filter asks), and the search in the
/// text index, when there are words: they, and the scope's token.
struct Built {
    list: Option<List>,
    scope: Option<Term>,
    filters: Vec<Term>,
    search: Option<String>,
}

impl Built {
    /// The alias the listing is walked on, and its item id there.
    fn walked(&self) -> (&'static str, &'static str) {
        if self.list.is_some() {
            ("l", "l.item")
        } else {
            ("i", "i.id")
        }
    }

    /// What is walked, with its alias.
    fn table(&self) -> String {
        match self.list {
            Some(list) => format!("{} l", list.table()),
            None => "items i".to_owned(),
        }
    }

    /// `FROM … WHERE …`: what is walked and every term, the words as a set
    /// each row is checked against — never the index it is walked on (the
    /// unary `+`), or a word most items have would be looked up item by item
    /// and sorted whole.
    fn selection(&self) -> (String, Vec<Value>) {
        let (_, id) = self.walked();
        let mut sql = format!("FROM {} WHERE 1", self.table());
        let mut args = Vec::new();
        for t in self.scope.iter().chain(&self.filters) {
            sql.push_str(" AND ");
            sql.push_str(&t.sql);
            args.extend(t.args.iter().cloned());
        }
        if let Some(search) = &self.search {
            sql.push_str(&format!(
                " AND +{id} IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?)"
            ));
            args.push(Value::Text(search.clone()));
        }
        (sql, args)
    }
}

/// What a scope token names.
enum Scoped<'a> {
    /// A source's items, by its id.
    Source(&'a str),
    /// A list's members: a category's or a theme list's, by its id.
    List(i64),
}

/// The word of the text index an item carries for a list it is in: `s…` a
/// source, `l…` a category or a theme list, in hexadecimal — a word the
/// tokenizer keeps whole, and one no title writes in the column it is in.
fn scope_token(scoped: Scoped<'_>) -> String {
    match scoped {
        Scoped::Source(id) => format!("s{:x}", super::fnv(&["source", id])),
        Scoped::List(id) => format!("l{id:x}"),
    }
}

/// Which of a listing's filters a question leaves out.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leave {
    Nothing,
    Letter,
    Country,
    Language,
}

/// A listing's walk and terms. A category of one source asked under another
/// is answered before this (`Reader::asks_nothing`): the ids say whose a
/// category is.
fn build(q: &ListQuery, leave: Leave) -> Built {
    let (list, scope, token) = match &q.scope {
        Scope::Category(c) => (
            Some(List::Category),
            Some(term("l.category = ?", [Value::Integer(c.0)])),
            Some(scope_token(Scoped::List(c.0))),
        ),
        Scope::Theme(theme) => {
            let one = match &q.sources {
                Sources::One(s) => Some(s.as_str()),
                Sources::All => None,
            };
            let id = theme_list(one, theme);
            (
                Some(List::Theme),
                Some(term("l.theme_list = ?", [Value::Integer(id)])),
                Some(scope_token(Scoped::List(id))),
            )
        }
        Scope::AllCategories => match &q.sources {
            Sources::One(s) => (
                None,
                Some(term("i.source = ?", [Value::Text(s.clone())])),
                Some(scope_token(Scoped::Source(s))),
            ),
            Sources::All => (None, None, None),
        },
    };
    let mut terms = Vec::new();
    let w = if list.is_some() { "l" } else { "i" };
    if leave != Leave::Letter
        && let Some(l) = q.letter
    {
        terms.push(term(
            format!("{w}.letter = ?"),
            [Value::Text(l.to_string())],
        ));
    }
    if leave != Leave::Country
        && let Some(c) = &q.country
    {
        terms.push(term(format!("{w}.country = ?"), [Value::Text(c.clone())]));
    }
    if leave != Leave::Language
        && let Some(l) = &q.language
    {
        terms.push(term(format!("{w}.language = ?"), [Value::Text(l.clone())]));
    }
    if let Some(h) = q.min_height {
        terms.push(term(
            format!("{w}.quality >= ?"),
            [Value::Integer(i64::from(h))],
        ));
    }
    // THE SEARCH: the words in the three text columns, and the scope's
    // token in its own.
    let search = q.words.as_deref().and_then(fts_query).map(|words| {
        let mut search = format!("{{title also description}} : ({words})");
        if let Some(token) = &token {
            search.push_str(&format!(" AND scopes : \"{token}\""));
        }
        search
    });
    Built {
        list,
        scope,
        filters: terms,
        search,
    }
}

/// The columns an order compares, in order, and whether each is descending;
/// the item's id breaks the last tie.
const fn keys_of(sort: Sort) -> &'static [(&'static str, bool)] {
    match sort {
        Sort::Title => &[("title_folded", false)],
        Sort::Popular => &[("popularity", true), ("title_folded", false)],
        Sort::Newest => &[("newest", true), ("title_folded", false)],
        Sort::Oldest => &[("oldest", false), ("title_folded", false)],
    }
}

/// `ORDER BY` the keys of `sort`, read from `alias` (as `k0`, `k1`… when
/// `named`), the id `id` last — or all of it the other way round, when
/// `backwards`.
fn order_by(sort: Sort, alias: &str, named: bool, id: &str, backwards: bool) -> String {
    let way = |desc: bool| if desc != backwards { " DESC" } else { "" };
    let mut parts: Vec<String> = keys_of(sort)
        .iter()
        .enumerate()
        .map(|(n, (column, desc))| {
            if named {
                format!("{alias}.k{n}{}", way(*desc))
            } else {
                format!("{alias}.{column}{}", way(*desc))
            }
        })
        .collect();
    parts.push(format!("{id}{}", way(false)));
    format!("ORDER BY {}", parts.join(", "))
}

/// The query that CHOOSES a page — its ids and what they sort by — walked on
/// the covering index of the listing's order, from its start or, when
/// `from_end`, from its end: the rows an `OFFSET` skips are never looked up.
/// Binds `LIMIT` and `OFFSET` last.
fn walk(q: &ListQuery, from_end: bool) -> (String, Vec<Value>) {
    let built = build(q, Leave::Nothing);
    let (walked, id) = built.walked();
    let (from_where, args) = built.selection();
    let keys: Vec<String> = keys_of(q.sort)
        .iter()
        .enumerate()
        .map(|(n, (column, _))| format!("{walked}.{column} AS k{n}"))
        .collect();
    let sql = format!(
        "SELECT {id} AS id, {} {from_where} {} LIMIT ? OFFSET ?",
        keys.join(", "),
        order_by(q.sort, walked, false, id, from_end)
    );
    (sql, args)
}

/// A page's rows: the ids [`walk`] chose, joined to their items, in the
/// listing's order whichever way they were walked.
fn page_query(q: &ListQuery, from_end: bool) -> (String, Vec<Value>) {
    let (walk, args) = walk(q, from_end);
    let sql = format!(
        "SELECT {ROW_COLUMNS} FROM ({walk}) AS page JOIN items i ON i.id = page.id {}",
        order_by(q.sort, "page", true, "page.id", false)
    );
    (sql, args)
}

/// How many items a listing has.
///
/// With words and nothing else, the text index alone answers: the words and
/// the scope's token are one question to it. With a filter as well, the
/// items the search finds drive, each looked up by its key in what is walked
/// — never every row of the scope checked against the set. A `CROSS JOIN`,
/// SQLite's word for "keep the left outside": left to choose, it put the
/// scope outside and ran the search once per row of it (measured: minutes).
fn count_query(q: &ListQuery) -> (String, Vec<Value>) {
    let built = build(q, Leave::Nothing);
    let Some(search) = &built.search else {
        let (from_where, args) = built.selection();
        return (format!("SELECT count(*) {from_where}"), args);
    };
    if built.filters.is_empty() {
        return (
            "SELECT count(*) FROM items_fts WHERE items_fts MATCH ?".to_owned(),
            vec![Value::Text(search.clone())],
        );
    }
    let (_, id) = built.walked();
    let mut sql = format!(
        "SELECT count(*) FROM items_fts f CROSS JOIN {} ON {id} = f.rowid WHERE items_fts MATCH ?",
        built.table()
    );
    let mut args = vec![Value::Text(search.clone())];
    for t in built.scope.iter().chain(&built.filters) {
        sql.push_str(" AND ");
        sql.push_str(&t.sql);
        args.extend(t.args.iter().cloned());
    }
    (sql, args)
}

/// Where a page begins, from the start of its listing or from its end: the
/// nearer. `(from_end, limit, offset)` for a listing of `total` rows, its
/// page `page` (from 1, at most its last) of `per_page`.
fn placed(total: u64, page: u32, per_page: u32) -> (bool, u64, u64) {
    let offset = u64::from(page.max(1) - 1) * u64::from(per_page);
    let on_page = total.saturating_sub(offset).min(u64::from(per_page));
    let after = total.saturating_sub(offset).saturating_sub(on_page);
    if after < offset {
        (true, on_page, after)
    } else {
        (false, on_page, offset)
    }
}

const ROW_COLUMNS: &str = "i.id, i.source, i.kind, i.title, i.letter, i.country, i.language, \
     i.quality, i.duration_s, i.year, i.popularity, i.published, i.poster_url";

/// A row, read strictly: a value this build does not write (a kind it does
/// not know, a letter that is not one character, a number out of its type)
/// is an error ([`read_trouble`] makes it [`Trouble::OtherSchema`]), never a
/// plausible guess.
fn row_of(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    Ok(Row {
        id: ItemId(r.get(0)?),
        source: r.get(1)?,
        kind: kind_at(r, 2)?,
        title: r.get(3)?,
        letter: letter_at(r, 4)?,
        country: r.get(5)?,
        language: r.get(6)?,
        height: r.get(7)?,
        duration_s: r.get(8)?,
        year: r.get(9)?,
        popularity: count_at(r, 10)?,
        published_s: r.get(11)?,
        poster_url: r.get(12)?,
    })
}

// ---------------------------------------------------------------------------
// THE READERS
// ---------------------------------------------------------------------------

/// A reader of one of the two files: what a door holds.
pub struct Reader {
    conn: Connection,
    store: Store,
    path: PathBuf,
    /// The device and inode of the file it opened.
    file: (u64, u64),
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl Reader {
    /// A reader of `store` in `dir`, read-only.
    ///
    /// # Errors
    ///
    /// [`Trouble::Absent`] before the first ingest, [`Trouble::OtherSchema`]
    /// for a file that is not this build's — another layout, or not a
    /// catalogue at all (the writer makes it again) — [`Trouble::Unreadable`].
    pub fn open(dir: &Path, store: Store) -> Result<Self, Trouble> {
        let path = dir.join(store.file());
        // THE FILE IS NAMED BEFORE IT IS OPENED: made again in between, the
        // reader holds the new one and calls it replaced — a new reader too
        // many, never a stale one kept.
        let meta = match std::fs::metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(Trouble::Absent),
            Err(_) => return Err(Trouble::Unreadable),
        };
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(read_trouble)?;
        conn.busy_timeout(READER_BUSY).map_err(read_trouble)?;
        match found(&conn).map_err(read_trouble)? {
            Found::Ours => {}
            Found::Nothing => return Err(Trouble::Absent),
            Found::Other => return Err(Trouble::OtherSchema),
        }
        conn.pragma_update(None, "query_only", true)
            .map_err(read_trouble)?;
        Ok(Self {
            conn,
            store,
            path,
            file: (meta.dev(), meta.ino()),
        })
    }

    /// A reader of this instance's catalogue ([`super::folder`]).
    ///
    /// # Errors
    ///
    /// [`Trouble::NoHome`], and as [`Reader::open`].
    pub fn player(store: Store) -> Result<Self, Trouble> {
        let dir = super::folder().ok_or(Trouble::NoHome)?;
        Self::open(&dir, store)
    }

    /// Which file it reads.
    #[must_use]
    pub const fn store(&self) -> Store {
        self.store
    }

    /// Whether the file at its path is no longer the one it opened: the
    /// writer made it again (another layout, a ceiling lowered) or it is
    /// gone. This reader keeps reading the old one; a door that sees it opens
    /// a new reader.
    #[must_use]
    pub fn replaced(&self) -> bool {
        std::fs::metadata(&self.path).map_or(true, |m| (m.dev(), m.ino()) != self.file)
    }

    /// The generation: a new number after every commit of the writer.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unreadable`].
    pub fn generation(&self) -> Result<u64, Trouble> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = 'generation'", [], |r| {
                count_at(r, 0)
            })
            .map_err(read_trouble)
    }

    /// Every source this file has rows or a status for, by id.
    ///
    /// # Errors
    ///
    /// [`Trouble::OtherSchema`] for a kind this build does not know (a newer
    /// build's row: the layout's number was not raised),
    /// [`Trouble::Unreadable`].
    pub fn sources(&self) -> Result<Vec<SourceRow>, Trouble> {
        let mut st = self
            .conn
            .prepare_cached(
                "SELECT id, kind, title, shown, adult, posters_on, last_ingest, last_error, items, cut
                 FROM sources ORDER BY id",
            )
            .map_err(read_trouble)?;
        let rows = st
            .query_map([], |r| {
                Ok(SourceRow {
                    id: r.get(0)?,
                    kind: source_kind_at(r, 1)?,
                    title: r.get(2)?,
                    shown: r.get(3)?,
                    adult: r.get(4)?,
                    posters_on: r.get(5)?,
                    last_ingest_s: r.get(6)?,
                    last_error: r.get(7)?,
                    items: count_at(r, 8)?,
                    cut: r.get(9)?,
                })
            })
            .map_err(read_trouble)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read_trouble)
    }

    /// The categories of one source or of all, by source and title.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unreadable`].
    pub fn categories(&self, sources: &Sources) -> Result<Vec<CategoryRow>, Trouble> {
        let (sql, args): (&str, Vec<Value>) = match sources {
            Sources::One(s) => (
                "SELECT id, source, key, title, theme, items FROM categories WHERE source = ?1
                 ORDER BY source, title, key",
                vec![Value::Text(s.clone())],
            ),
            Sources::All => (
                "SELECT id, source, key, title, theme, items FROM categories
                 ORDER BY source, title, key",
                Vec::new(),
            ),
        };
        let mut st = self.conn.prepare_cached(sql).map_err(read_trouble)?;
        let rows = st
            .query_map(rusqlite::params_from_iter(args), |r| {
                Ok(CategoryRow {
                    id: CategoryId(r.get(0)?),
                    source: r.get(1)?,
                    key: r.get(2)?,
                    title: r.get(3)?,
                    theme: r.get(4)?,
                    items: count_at(r, 5)?,
                })
            })
            .map_err(read_trouble)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read_trouble)
    }

    /// Every theme with items, and how many (each item once, whatever number
    /// of its categories the theme holds), by theme.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unreadable`].
    pub fn themes(&self, sources: &Sources) -> Result<Vec<(String, u64)>, Trouble> {
        let source = match sources {
            Sources::One(s) => Value::Text(s.clone()),
            Sources::All => Value::Null,
        };
        let mut st = self
            .conn
            .prepare_cached(
                "SELECT theme, items FROM theme_lists WHERE source IS ?1 AND items > 0
                 ORDER BY theme",
            )
            .map_err(read_trouble)?;
        let rows = st
            .query_map([source], |r| Ok((r.get::<_, String>(0)?, count_at(r, 1)?)))
            .map_err(read_trouble)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read_trouble)
    }

    /// Whether `q` asks a category under a source it is not of: a category
    /// is one source's, and holds nothing of another.
    fn asks_nothing(&self, q: &ListQuery) -> Result<bool, Trouble> {
        let (Scope::Category(c), Sources::One(s)) = (&q.scope, &q.sources) else {
            return Ok(false);
        };
        let whose: Option<String> = self
            .conn
            .prepare_cached("SELECT source FROM categories WHERE id = ?1")
            .and_then(|mut st| st.query_row([c.0], |r| r.get(0)).optional())
            .map_err(read_trouble)?;
        Ok(whose.as_deref() != Some(s.as_str()))
    }

    /// One page of a listing, and how many there are.
    ///
    /// A page past the last is the last; a page of nothing is page 1 of 1.
    /// The count and the page are read in one snapshot, and a page nearer the
    /// end is walked from the end.
    ///
    /// # Errors
    ///
    /// [`Trouble::OtherSchema`] for a row this build did not write,
    /// [`Trouble::Unreadable`].
    pub fn list(&self, q: &ListQuery) -> Result<ListPage, Trouble> {
        let snapshot = self.conn.unchecked_transaction().map_err(read_trouble)?;
        let total = if self.asks_nothing(q)? {
            0
        } else {
            let (count, args) = count_query(q);
            let n: i64 = self
                .conn
                .prepare_cached(&count)
                .and_then(|mut st| st.query_row(rusqlite::params_from_iter(args), |r| r.get(0)))
                .map_err(read_trouble)?;
            u64::try_from(n).unwrap_or(0)
        };
        let per_page = q.per_page.clamp(1, PER_PAGE_MOST);
        let pages = u32::try_from(total.div_ceil(u64::from(per_page)))
            .unwrap_or(u32::MAX)
            .max(1);
        let page = q.page.clamp(1, pages);
        let (from_end, limit, offset) = placed(total, page, per_page);
        let rows = if limit == 0 {
            Vec::new()
        } else {
            let (sql, mut args) = page_query(q, from_end);
            args.push(Value::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
            args.push(Value::Integer(i64::try_from(offset).unwrap_or(i64::MAX)));
            let mut st = self.conn.prepare_cached(&sql).map_err(read_trouble)?;
            st.query_map(rusqlite::params_from_iter(args), row_of)
                .map_err(read_trouble)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(read_trouble)?
        };
        snapshot.commit().map_err(read_trouble)?;
        Ok(ListPage {
            rows,
            total,
            page,
            pages,
        })
    }

    /// The letters a listing's titles are filed under — the listing as asked
    /// but for its letter — `#` first, then as they fold (`Ç` after `C`).
    ///
    /// # Errors
    ///
    /// [`Trouble::Unreadable`].
    pub fn letters(&self, q: &ListQuery) -> Result<Vec<char>, Trouble> {
        if self.asks_nothing(q)? {
            return Ok(Vec::new());
        }
        let built = build(q, Leave::Letter);
        let (walked, _) = built.walked();
        let (from_where, args) = built.selection();
        let sql = format!("SELECT DISTINCT {walked}.letter {from_where}");
        let mut st = self.conn.prepare_cached(&sql).map_err(read_trouble)?;
        let mut out: Vec<char> = st
            .query_map(rusqlite::params_from_iter(args), |r| letter_at(r, 0))
            .map_err(read_trouble)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read_trouble)?;
        out.sort_by(|a, b| {
            let fa = super::fold(&a.to_string());
            let fb = super::fold(&b.to_string());
            (*a != '#', fa, *a).cmp(&(*b != '#', fb, *b))
        });
        out.dedup();
        Ok(out)
    }

    /// The countries a listing's items are from — the listing as asked but
    /// for its country — the most items first.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unreadable`].
    pub fn countries(&self, q: &ListQuery) -> Result<Vec<(String, u64)>, Trouble> {
        self.facet(q, Leave::Country, "country")
    }

    /// The languages a listing's items speak — the listing as asked but for
    /// its language — the most items first.
    ///
    /// # Errors
    ///
    /// [`Trouble::Unreadable`].
    pub fn languages(&self, q: &ListQuery) -> Result<Vec<(String, u64)>, Trouble> {
        self.facet(q, Leave::Language, "language")
    }

    fn facet(
        &self,
        q: &ListQuery,
        leave: Leave,
        column: &str,
    ) -> Result<Vec<(String, u64)>, Trouble> {
        if self.asks_nothing(q)? {
            return Ok(Vec::new());
        }
        let built = build(q, leave);
        let (walked, _) = built.walked();
        let (from_where, args) = built.selection();
        let sql = format!(
            "SELECT {walked}.{column}, count(*) {from_where} AND {walked}.{column} IS NOT NULL
             GROUP BY 1 ORDER BY 2 DESC, 1"
        );
        let mut st = self.conn.prepare_cached(&sql).map_err(read_trouble)?;
        let rows = st
            .query_map(rusqlite::params_from_iter(args), |r| {
                Ok((r.get::<_, String>(0)?, count_at(r, 1)?))
            })
            .map_err(read_trouble)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read_trouble)
    }

    /// One item, whole; `None` when it is not (or no longer) there.
    ///
    /// # Errors
    ///
    /// [`Trouble::OtherSchema`] for a row this build did not write,
    /// [`Trouble::Unreadable`].
    pub fn item(&self, id: ItemId) -> Result<Option<Item>, Trouble> {
        let sql = format!(
            "SELECT {ROW_COLUMNS}, i.licence, i.stream_json, f.also, f.description
             FROM items i LEFT JOIN items_fts f ON f.rowid = i.id WHERE i.id = ?1"
        );
        let found = self
            .conn
            .prepare_cached(&sql)
            .and_then(|mut st| {
                st.query_row([id.0], |r| {
                    Ok((
                        row_of(r)?,
                        r.get::<_, Option<String>>(13)?,
                        r.get::<_, String>(14)?,
                        r.get::<_, Option<String>>(15)?,
                        r.get::<_, Option<String>>(16)?,
                    ))
                })
                .optional()
            })
            .map_err(read_trouble)?;
        let Some((row, licence, stream, also, description)) = found else {
            return Ok(None);
        };
        let stream: Stream = serde_json::from_str(&stream).map_err(|_| Trouble::OtherSchema)?;
        let mut st = self
            .conn
            .prepare_cached(
                "SELECT category FROM item_categories WHERE item = ?1 ORDER BY category",
            )
            .map_err(read_trouble)?;
        let categories = st
            .query_map([id.0], |r| Ok(CategoryId(r.get(0)?)))
            .map_err(read_trouble)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(read_trouble)?;
        Ok(Some(Item {
            row,
            also: also.unwrap_or_default(),
            description: description.filter(|d| !d.is_empty()),
            licence,
            stream,
            categories,
        }))
    }
}

#[cfg(test)]
mod tests;
