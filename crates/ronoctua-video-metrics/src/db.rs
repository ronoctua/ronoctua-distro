//! The catalogue's file: opening it read-only or read-write, its schema and
//! the migrations that bring an older one up, the pragmas, the generation,
//! the settings — and how SQLite's errors become refusals.
//!
//! # WAL, and who waits
//!
//! The file is in WAL mode (set by the first writer, and kept by the file):
//! a reader sees the last commit while the scanner writes, and a writer waits
//! for another up to [`BUSY`] (`busy_timeout`) before [`crate::Why::Busy`].
//! `synchronous=NORMAL` is WAL's own recommendation: a commit survives the
//! process, and a power cut loses at most the last few, never the file.
//!
//! # The schema is a number
//!
//! `meta.schema` says which [`MIGRATIONS`] have run. A writer brings an older
//! file up in one transaction; a reader that finds one asks a short writer to
//! do it first ([`open_ro`]); a NEWER file is refused
//! ([`crate::Why::OtherSchema`]) and never touched — it belongs to a build
//! that knows more.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, ErrorCode, OpenFlags, Transaction, TransactionBehavior, params};

use crate::{Refusal, Rule, RuleKind, Settings, Weights, Why};

/// How long a connection waits for another's lock, `busy_timeout`.
pub(crate) const BUSY: Duration = Duration::from_millis(5000);

/// The schema this build writes and reads.
pub(crate) const SCHEMA: i64 = 1;

/// The migrations, the first bringing an empty file to schema 1. Each runs
/// in the same transaction as the `meta.schema` it raises; a new schema is a
/// new entry, never an edit of one that shipped.
const MIGRATIONS: [&str; SCHEMA as usize] = [V1];

const V1: &str = "
CREATE TABLE meta (
  one INTEGER PRIMARY KEY CHECK (one = 1),
  schema INTEGER NOT NULL,
  generation INTEGER NOT NULL
);
INSERT INTO meta (one, schema, generation) VALUES (1, 0, 0);

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE channels (
  id TEXT PRIMARY KEY,
  platform TEXT NOT NULL DEFAULT 'youtube',
  handle TEXT,
  title TEXT NOT NULL,
  url TEXT NOT NULL,
  avatar_url TEXT,
  score INTEGER NOT NULL CHECK (score BETWEEN 0 AND 10000),
  added_at INTEGER NOT NULL,
  reference_at INTEGER,
  last_try_at INTEGER,
  last_error TEXT,
  first_done INTEGER NOT NULL DEFAULT 0,
  block_shorts INTEGER,
  recency_points INTEGER NOT NULL DEFAULT 0,
  recency_days INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;

CREATE TABLE videos (
  num INTEGER PRIMARY KEY,
  id TEXT NOT NULL UNIQUE,
  platform TEXT NOT NULL DEFAULT 'youtube',
  channel TEXT NOT NULL,
  url TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT '',
  duration_s INTEGER,
  published_at INTEGER,
  first_seen_at INTEGER NOT NULL,
  is_short INTEGER NOT NULL DEFAULT 0,
  short_evidence TEXT,
  live_status TEXT,
  width INTEGER,
  height INTEGER,
  language TEXT,
  audio_language TEXT,
  age_limit INTEGER NOT NULL DEFAULT 0,
  availability TEXT,
  chapters TEXT NOT NULL DEFAULT '[]',
  captions TEXT NOT NULL DEFAULT '[]',
  categories TEXT NOT NULL DEFAULT '[]',
  thumb_url TEXT,
  thumb_file TEXT,
  details_state TEXT NOT NULL DEFAULT 'pending',
  details_tries INTEGER NOT NULL DEFAULT 0,
  hidden INTEGER NOT NULL DEFAULT 0,
  hidden_at INTEGER,
  hidden_reason TEXT,
  rating INTEGER,
  second_description TEXT NOT NULL DEFAULT '',
  static_points INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL,
  generation INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX videos_by_channel ON videos (channel, published_at);
CREATE INDEX videos_by_generation ON videos (generation);
CREATE INDEX videos_by_details ON videos (details_state, published_at);

CREATE TABLE video_tags (
  video TEXT NOT NULL,
  own INTEGER NOT NULL,
  tag TEXT NOT NULL,
  UNIQUE (video, own, tag)
);

CREATE TABLE views (
  video TEXT NOT NULL,
  at INTEGER NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (video, at)
) WITHOUT ROWID;

CREATE TABLE rules (
  id INTEGER PRIMARY KEY,
  channel TEXT,
  kind TEXT NOT NULL,
  fields INTEGER NOT NULL DEFAULT 0,
  pattern TEXT NOT NULL DEFAULT '',
  how TEXT NOT NULL DEFAULT 'word',
  points INTEGER NOT NULL,
  min_s INTEGER,
  max_s INTEGER,
  days INTEGER,
  enabled INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX rules_by_channel ON rules (channel);

CREATE TABLE skipped (
  video TEXT PRIMARY KEY,
  channel TEXT,
  reason TEXT NOT NULL,
  at INTEGER NOT NULL,
  generation INTEGER NOT NULL DEFAULT 0
) WITHOUT ROWID;
CREATE INDEX skipped_by_generation ON skipped (generation);

CREATE TABLE scans (
  id INTEGER PRIMARY KEY,
  started_at INTEGER NOT NULL,
  ended_at INTEGER,
  channels INTEGER NOT NULL DEFAULT 0,
  channels_failed INTEGER NOT NULL DEFAULT 0,
  added INTEGER NOT NULL DEFAULT 0,
  skipped INTEGER NOT NULL DEFAULT 0,
  detailed INTEGER NOT NULL DEFAULT 0,
  pending INTEGER NOT NULL DEFAULT 0,
  thumbs INTEGER NOT NULL DEFAULT 0,
  stopped TEXT,
  said TEXT
);

CREATE VIRTUAL TABLE videos_fts USING fts5 (
  title, description, tags, second_description, second_tags,
  content = '', contentless_delete = 1,
  tokenize = 'unicode61 remove_diacritics 2'
);
";

/// The rules a new catalogue starts with, editable like any other: recency
/// worth 2000 points today, fading over 30 days, and a Short costing 1000.
/// Against a channel's 0–10000 and the ratings' ±500/±2000, a video of this
/// week from a middling channel sits above an old one from the same channel,
/// and a very positive rating outweighs a month.
pub(crate) fn starting_rules() -> [Rule; 2] {
    [
        Rule {
            id: None,
            channel: None,
            kind: RuleKind::Recency { days: 30 },
            points: 2000,
            enabled: true,
        },
        Rule {
            id: None,
            channel: None,
            kind: RuleKind::Short,
            points: -1000,
            enabled: true,
        },
    ]
}

/// A refusal for what SQLite said.
pub(crate) fn refused(e: &rusqlite::Error) -> Refusal {
    Refusal::of(match e.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Why::Busy,
        Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) => Why::Unreadable,
        Some(ErrorCode::CannotOpen | ErrorCode::PermissionDenied | ErrorCode::ReadOnly) => {
            Why::Unreadable
        }
        _ => Why::Failed,
    })
}

/// `map_err` for a SQLite result.
pub(crate) fn sql(e: rusqlite::Error) -> Refusal {
    refused(&e)
}

/// Seconds since the epoch, now.
#[must_use]
pub(crate) fn now_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The catalogue at `path`, read-write, created with the schema when it is
/// not there (and its folder with it), migrated when it is older.
///
/// # Errors
///
/// [`Why::Unreadable`] (not a database, no permission, a folder that cannot
/// be made), [`Why::OtherSchema`], [`Why::Busy`], [`Why::Failed`].
pub(crate) fn open_rw(path: &Path) -> Result<Connection, Refusal> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|_| Refusal::of(Why::Unreadable))?;
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(sql)?;
    conn.busy_timeout(BUSY).map_err(sql)?;
    // WAL IS A PROPERTY OF THE FILE, set once and kept; asking again is a
    // read. It cannot change inside a transaction, so it comes first.
    let mode: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))
        .map_err(sql)?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Refusal::of(Why::Unreadable));
    }
    conn.execute_batch("PRAGMA synchronous = NORMAL;")
        .map_err(sql)?;
    migrate(&conn)?;
    Ok(conn)
}

/// The catalogue at `path`, read-only (`query_only` on top of the open
/// flags). An older schema is migrated first by a short writer; a missing
/// file is [`Why::NotThere`] — nothing was ever scanned.
///
/// # Errors
///
/// [`Why::NotThere`], [`Why::Unreadable`], [`Why::OtherSchema`],
/// [`Why::Busy`], [`Why::Failed`].
pub(crate) fn open_ro(path: &Path) -> Result<Connection, Refusal> {
    if !path.is_file() {
        return Err(Refusal::of(Why::NotThere));
    }
    let open = || -> Result<Connection, Refusal> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(sql)?;
        conn.busy_timeout(BUSY).map_err(sql)?;
        conn.execute_batch("PRAGMA query_only = ON;").map_err(sql)?;
        Ok(conn)
    };
    let conn = open()?;
    match schema_of(&conn)? {
        SCHEMA => Ok(conn),
        newer if newer > SCHEMA => Err(Refusal::of(Why::OtherSchema)),
        _ => {
            drop(conn);
            drop(open_rw(path)?);
            open()
        }
    }
}

/// Which schema the file is at; 0 for a file without our tables.
pub(crate) fn schema_of(conn: &Connection) -> Result<i64, Refusal> {
    let ours: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'meta'",
            [],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if ours == 0 {
        return Ok(0);
    }
    conn.query_row("SELECT schema FROM meta WHERE one = 1", [], |r| r.get(0))
        .map_err(sql)
}

/// Brings the file up to [`SCHEMA`], in one immediate transaction.
fn migrate(conn: &Connection) -> Result<(), Refusal> {
    if schema_of(conn)? == SCHEMA {
        return Ok(());
    }
    let tx = immediate(conn)?;
    // ASKED AGAIN UNDER THE LOCK: another writer may have migrated between.
    let at = schema_of(&tx)?;
    if at > SCHEMA {
        return Err(Refusal::of(Why::OtherSchema));
    }
    for step in at..SCHEMA {
        let index = usize::try_from(step).map_err(|_| Refusal::of(Why::Failed))?;
        tx.execute_batch(MIGRATIONS[index]).map_err(sql)?;
        if step == 0 {
            seed(&tx)?;
        }
    }
    tx.execute("UPDATE meta SET schema = ?1 WHERE one = 1", params![SCHEMA])
        .map_err(sql)?;
    tx.commit().map_err(sql)
}

/// What a new catalogue holds before anything is scanned: the settings and
/// weights of their `Default`, and the starting rules.
fn seed(tx: &Transaction<'_>) -> Result<(), Refusal> {
    put_settings(tx, &Settings::default())?;
    put_weights(tx, &Weights::default())?;
    for rule in starting_rules() {
        crate::edit::insert_rule(tx, &rule)?;
    }
    Ok(())
}

/// A write transaction that takes the write lock at once (`BEGIN
/// IMMEDIATE`), so it waits for another writer at its start rather than
/// failing at its first write.
pub(crate) fn immediate(conn: &Connection) -> Result<Transaction<'_>, Refusal> {
    Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(sql)
}

/// The catalogue's generation: it moves with every write that changes a
/// video (what `Catalog::all_since` hands the index).
pub(crate) fn generation(conn: &Connection) -> Result<i64, Refusal> {
    conn.query_row("SELECT generation FROM meta WHERE one = 1", [], |r| {
        r.get(0)
    })
    .map_err(sql)
}

/// Moves the generation on, inside a write, and answers the new one.
pub(crate) fn bump(tx: &Transaction<'_>) -> Result<i64, Refusal> {
    tx.query_row(
        "UPDATE meta SET generation = generation + 1 WHERE one = 1 RETURNING generation",
        [],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// One setting, as kept.
fn setting(conn: &Connection, key: &str) -> Result<Option<String>, Refusal> {
    let mut statement = conn
        .prepare_cached("SELECT value FROM settings WHERE key = ?1")
        .map_err(sql)?;
    let mut rows = statement.query(params![key]).map_err(sql)?;
    match rows.next().map_err(sql)? {
        Some(row) => Ok(Some(row.get(0).map_err(sql)?)),
        None => Ok(None),
    }
}

/// A number setting; the default when it is not kept (a catalogue from
/// before the setting existed), a refusal when it is kept and is not a
/// number — never a plausible default over the person's value.
fn number<T: std::str::FromStr>(conn: &Connection, key: &str, default: T) -> Result<T, Refusal> {
    setting(conn, key)?.map_or(Ok(default), |v| {
        v.parse().map_err(|_| Refusal::of(Why::Unreadable))
    })
}

fn put(tx: &Transaction<'_>, key: &str, value: &str) -> Result<(), Refusal> {
    tx.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map(|_| ())
    .map_err(sql)
}

/// The catalogue's switches.
pub(crate) fn settings(conn: &Connection) -> Result<Settings, Refusal> {
    let defaults = Settings::default();
    Ok(Settings {
        block_shorts: number::<u8>(conn, "block_shorts", u8::from(defaults.block_shorts))? != 0,
        first_scan_days: number(conn, "first_scan_days", defaults.first_scan_days)?,
        first_scan_most: number(conn, "first_scan_most", defaults.first_scan_most)?,
        views_snapshots: number::<u8>(conn, "views_snapshots", u8::from(defaults.views_snapshots))?
            != 0,
    })
}

/// Keeps the switches.
pub(crate) fn put_settings(tx: &Transaction<'_>, settings: &Settings) -> Result<(), Refusal> {
    put(
        tx,
        "block_shorts",
        &u8::from(settings.block_shorts).to_string(),
    )?;
    put(tx, "first_scan_days", &settings.first_scan_days.to_string())?;
    put(tx, "first_scan_most", &settings.first_scan_most.to_string())?;
    put(
        tx,
        "views_snapshots",
        &u8::from(settings.views_snapshots).to_string(),
    )
}

/// The rating weights.
pub(crate) fn weights(conn: &Connection) -> Result<Weights, Refusal> {
    let defaults = Weights::default();
    Ok(Weights {
        very_negative: number(conn, "weight_very_negative", defaults.very_negative)?,
        negative: number(conn, "weight_negative", defaults.negative)?,
        positive: number(conn, "weight_positive", defaults.positive)?,
        very_positive: number(conn, "weight_very_positive", defaults.very_positive)?,
    })
}

/// Keeps the rating weights.
pub(crate) fn put_weights(tx: &Transaction<'_>, weights: &Weights) -> Result<(), Refusal> {
    put(
        tx,
        "weight_very_negative",
        &weights.very_negative.to_string(),
    )?;
    put(tx, "weight_negative", &weights.negative.to_string())?;
    put(tx, "weight_positive", &weights.positive.to_string())?;
    put(
        tx,
        "weight_very_positive",
        &weights.very_positive.to_string(),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A scratch folder of this process, removed when dropped.
    pub(crate) struct Scratch(pub(crate) std::path::PathBuf);

    impl Scratch {
        pub(crate) fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "ronoctua-video-metrics-{}-{tag}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        pub(crate) fn catalogue(&self) -> std::path::PathBuf {
            self.0.join("video-metrics").join("catalog.sqlite")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_new_catalogue_is_wal_at_the_schema_with_its_defaults() {
        let scratch = Scratch::new("new");
        let path = scratch.catalogue();
        let conn = open_rw(&path).unwrap();
        assert_eq!(schema_of(&conn).unwrap(), SCHEMA);
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
        assert_eq!(settings(&conn).unwrap(), Settings::default());
        assert_eq!(weights(&conn).unwrap(), Weights::default());
        let rules: i64 = conn
            .query_row("SELECT count(*) FROM rules", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rules, 2);
        drop(conn);
        // Opened again: nothing runs twice.
        let conn = open_rw(&path).unwrap();
        let rules: i64 = conn
            .query_row("SELECT count(*) FROM rules", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rules, 2);
        // FTS5 with diacritics removed is the system SQLite's.
        // pt-BR: the accents are the subject, folded by the tokenizer.
        conn.execute(
            "INSERT INTO videos_fts (rowid, title) VALUES (1, 'Configurações')",
            [],
        )
        .unwrap();
        let found: i64 = conn
            .query_row(
                "SELECT count(*) FROM videos_fts WHERE videos_fts MATCH '\"configuracoes\"'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 1);
    }

    #[test]
    fn a_reader_is_read_only_and_refuses_what_is_not_there() {
        let scratch = Scratch::new("ro");
        let path = scratch.catalogue();
        assert_eq!(open_ro(&path).unwrap_err().why, Why::NotThere);
        drop(open_rw(&path).unwrap());
        let reader = open_ro(&path).unwrap();
        assert_eq!(schema_of(&reader).unwrap(), SCHEMA);
        assert!(
            reader
                .execute("UPDATE meta SET generation = 7", [])
                .is_err(),
            "a reader cannot write"
        );
        // A file that is not a database is said so.
        let junk = scratch.0.join("junk.sqlite");
        std::fs::write(&junk, b"this is not sqlite, not even close to it at all").unwrap();
        assert_eq!(open_ro(&junk).unwrap_err().why, Why::Unreadable);
    }

    #[test]
    fn a_newer_catalogue_is_refused_and_left_alone() {
        let scratch = Scratch::new("newer");
        let path = scratch.catalogue();
        let conn = open_rw(&path).unwrap();
        conn.execute("UPDATE meta SET schema = ?1", params![SCHEMA + 1])
            .unwrap();
        drop(conn);
        assert_eq!(open_rw(&path).unwrap_err().why, Why::OtherSchema);
        assert_eq!(open_ro(&path).unwrap_err().why, Why::OtherSchema);
        let conn = Connection::open(&path).unwrap();
        assert_eq!(schema_of(&conn).unwrap(), SCHEMA + 1, "untouched");
    }

    #[test]
    fn a_setting_that_is_not_a_number_is_refused_not_defaulted() {
        let scratch = Scratch::new("setting");
        let conn = open_rw(&scratch.catalogue()).unwrap();
        conn.execute(
            "UPDATE settings SET value = 'many' WHERE key = 'first_scan_most'",
            [],
        )
        .unwrap();
        assert_eq!(settings(&conn).unwrap_err().why, Why::Unreadable);
        conn.execute("DELETE FROM settings WHERE key = 'first_scan_most'", [])
            .unwrap();
        assert_eq!(
            settings(&conn).unwrap().first_scan_most,
            10,
            "absent is the default"
        );
    }

    #[test]
    fn the_generation_moves_inside_a_write() {
        let scratch = Scratch::new("generation");
        let conn = open_rw(&scratch.catalogue()).unwrap();
        let before = generation(&conn).unwrap();
        let tx = immediate(&conn).unwrap();
        assert_eq!(bump(&tx).unwrap(), before + 1);
        tx.commit().unwrap();
        assert_eq!(generation(&conn).unwrap(), before + 1);
    }
}
