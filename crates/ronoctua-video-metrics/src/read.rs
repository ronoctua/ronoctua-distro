//! The read side: what the player's ranking and fast-access open.
//!
//! [`Catalog::open_ro`] opens the catalogue `mode=ro` (WAL: a reader sees the
//! last commit while the scanner writes). The page and the order are SQL's
//! (`ORDER BY` the total, `LIMIT`/`OFFSET`), and recency is computed at read
//! time from `unixepoch('now')`, so a list read tomorrow is tomorrow's.
//! [`Catalog::hide`] and [`Catalog::delete`] are the two writes a reader may
//! make, through a short read-write connection of their own.
//!
//! # Watching it
//!
//! A door watches `<catalogue>-wal` (the writers' commits land there) and,
//! when it changes, asks [`Catalog::data_version`]: it moves exactly when
//! another connection committed. Never a clock.
//!
//! Without the `db` feature a [`Catalog`] cannot exist: [`Catalog::open_ro`]
//! refuses with [`crate::Why::NoDatabase`].

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{ChannelId, Rating, Refusal, Skip, Stop, VideoId};

/// Shorts in a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Shorts {
    /// Shorts and long videos.
    #[default]
    Any,
    /// Only Shorts.
    Only,
    /// No Shorts.
    Without,
}

/// Hidden videos in a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Hidden {
    /// Not the hidden ones (the default).
    #[default]
    Without,
    /// Only the hidden ones.
    Only,
    /// Both.
    Any,
}

/// What a list shows.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Filter {
    /// Only this channel's.
    pub channel: Option<ChannelId>,
    /// Shorts or not.
    pub shorts: Shorts,
    /// Only videos the person rated so.
    pub rating: Option<Rating>,
    /// Hidden or not.
    pub hidden: Hidden,
    /// Words the full-text index must find (title, description, tags and the
    /// second fields), accents folded.
    pub text: Option<String>,
    /// Published at or after, seconds since the epoch.
    pub published_after_s: Option<i64>,
    /// Published before, seconds since the epoch.
    pub published_before_s: Option<i64>,
}

/// Which page of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Page {
    /// The page, from 0.
    pub number: u32,
    /// Rows per page.
    pub per_page: u32,
}

/// One row of a ranked list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedVideo {
    /// The video.
    pub id: VideoId,
    /// Its address (`https://www.youtube.com/watch?v=…`, or `/shorts/…`).
    pub url: String,
    /// Its title — data.
    pub title: String,
    /// Its channel's title — data.
    pub channel: String,
    /// Its channel.
    pub channel_id: ChannelId,
    /// Its total score, recency included.
    pub score: i64,
    /// Its thumbnail on disk, when fetched.
    pub thumb_file: Option<PathBuf>,
    /// Its duration, in seconds, when known.
    pub duration_s: Option<u32>,
    /// When it was published, seconds since the epoch.
    pub published_at_s: Option<i64>,
    /// Whether it is a Short.
    pub is_short: bool,
    /// Whether the person hid it.
    pub hidden: bool,
}

/// A chapter of a video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chapter {
    /// Where it starts, in seconds.
    pub start_s: u32,
    /// Its title — data.
    pub title: String,
}

/// Every field of a video — the video page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Video {
    /// Its ranked row (id, title, channel, score…).
    pub row: RankedVideo,
    /// YouTube's description — data.
    pub description: String,
    /// When the scanner first saw it, seconds since the epoch.
    pub first_seen_at_s: i64,
    /// Whether it is (or was) live or a premiere.
    pub live: bool,
    /// Its width and height, in pixels, when known.
    pub size_px: Option<(u32, u32)>,
    /// Its language and its audio's, as YouTube tags them.
    pub languages: (Option<String>, Option<String>),
    /// Its age limit, in years (0: none).
    pub age_limit: u8,
    /// Its chapters.
    pub chapters: Vec<Chapter>,
    /// The languages of its captions.
    pub captions: Vec<String>,
    /// YouTube's categories.
    pub categories: Vec<String>,
    /// YouTube's tags.
    pub tags: Vec<String>,
    /// The person's second tags.
    pub second_tags: Vec<String>,
    /// The person's second description.
    pub second_description: String,
    /// The person's rating.
    pub rating: Option<Rating>,
    /// Its static points (everything but the channel and recency).
    pub static_points: i64,
    /// Its latest view count, when snapshots are kept.
    pub views: Option<u64>,
}

/// A channel of the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// Its id.
    pub id: ChannelId,
    /// Its handle (`@name`), when known.
    pub handle: Option<String>,
    /// Its title — data.
    pub title: String,
    /// Its address.
    pub url: String,
    /// Its picture's address, when known.
    pub avatar_url: Option<String>,
    /// Its score, 0–10000.
    pub score: i32,
    /// When the person added it, seconds since the epoch.
    pub added_at_s: i64,
    /// Where the scans stand: only what is newer (minus a day) is taken.
    pub reference_at_s: Option<i64>,
    /// When the last scan tried it, seconds since the epoch.
    pub last_try_at_s: Option<i64>,
    /// Why the last scan of it failed, when it did (the tool's words — data).
    pub last_error: Option<String>,
    /// Its own "block Shorts" switch; `None` follows the global one.
    pub block_shorts: Option<bool>,
    /// How many of its videos are catalogued.
    pub videos: u64,
}

/// What the last scan did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanRecord {
    /// When it started, seconds since the epoch.
    pub started_at_s: i64,
    /// When it ended; `None` while it runs — or when its process was killed
    /// before it ended, which [`Catalog::scanning`] tells apart.
    pub ended_at_s: Option<i64>,
    /// Channels whose feeds were read.
    pub channels: u32,
    /// Channels whose feeds failed (all of them is the brain's danger).
    pub channels_failed: u32,
    /// Videos catalogued.
    pub added: u32,
    /// Videos tombstoned.
    pub skipped: u32,
    /// Details fetched.
    pub detailed: u32,
    /// Details left for the next run.
    pub pending: u32,
    /// Thumbnails fetched.
    pub thumbs: u32,
    /// Why it stopped early, when it did.
    pub stopped: Option<Stop>,
    /// What the tool said when it stopped — data, for a log.
    pub said: Option<String>,
}

/// The catalogue, read-only.
#[derive(Debug)]
pub struct Catalog {
    path: PathBuf,
    #[cfg(feature = "db")]
    conn: rusqlite::Connection,
    #[cfg(not(feature = "db"))]
    never: std::convert::Infallible,
}

impl Catalog {
    /// Opens the catalogue at `path` read-only. A catalogue of an older
    /// schema is brought up first, by a short writer of this crate.
    ///
    /// # Errors
    ///
    /// [`crate::Why::NoDatabase`] without `db`; [`crate::Why::NotThere`],
    /// [`crate::Why::Unreadable`], [`crate::Why::OtherSchema`],
    /// [`crate::Why::Busy`].
    pub fn open_ro(path: &Path) -> Result<Self, Refusal> {
        #[cfg(feature = "db")]
        return Ok(Self {
            path: path.to_owned(),
            conn: crate::db::open_ro(path)?,
        });
        #[cfg(not(feature = "db"))]
        return {
            let _ = path;
            Err(Refusal::no_database())
        };
    }

    /// Where it is (the door watches `<path>-wal` beside it).
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether a scan holds the catalogue NOW. The last scan's record has no
    /// end both while it runs and when its process was killed; this tells
    /// the two apart — by the scan's lock, which goes with its process —
    /// and writes nothing.
    #[must_use]
    pub fn scanning(&self) -> bool {
        #[cfg(feature = "db")]
        return crate::scan::running(&self.path);
        #[cfg(not(feature = "db"))]
        return false;
    }

    /// SQLite's `data_version`: it moves when another connection commits.
    ///
    /// # Errors
    ///
    /// [`crate::Why::Busy`], [`crate::Why::Failed`].
    pub fn data_version(&self) -> Result<u64, Refusal> {
        #[cfg(feature = "db")]
        return self
            .conn
            .query_row("PRAGMA data_version", [], |r| r.get::<_, i64>(0))
            .map(|v| u64::try_from(v).unwrap_or(0))
            .map_err(crate::db::sql);
        #[cfg(not(feature = "db"))]
        return self.never(());
    }

    /// A page of videos by score, best first.
    ///
    /// # Errors
    ///
    /// [`crate::Why::Busy`], [`crate::Why::Unreadable`],
    /// [`crate::Why::Failed`].
    pub fn ranked(&self, filter: &Filter, page: Page) -> Result<Vec<RankedVideo>, Refusal> {
        self.ranked_at(filter, page, None)
    }

    /// [`Catalog::ranked`] as of `now_s` (`None`: now) — a test's clock.
    ///
    /// # Errors
    ///
    /// As [`Catalog::ranked`].
    #[doc(hidden)]
    pub fn ranked_at(
        &self,
        filter: &Filter,
        page: Page,
        now_s: Option<i64>,
    ) -> Result<Vec<RankedVideo>, Refusal> {
        #[cfg(feature = "db")]
        return crate::query::ranked(&self.conn, filter, page, now_s);
        #[cfg(not(feature = "db"))]
        return self.never((filter, page, now_s));
    }

    /// How many videos `filter` keeps — "page n of N".
    ///
    /// # Errors
    ///
    /// As [`Catalog::ranked`].
    pub fn count(&self, filter: &Filter) -> Result<u64, Refusal> {
        #[cfg(feature = "db")]
        return crate::query::count(&self.conn, filter);
        #[cfg(not(feature = "db"))]
        return self.never(filter);
    }

    /// Every field of one video; `None` when there is no such video.
    ///
    /// # Errors
    ///
    /// As [`Catalog::ranked`].
    pub fn video(&self, id: &VideoId) -> Result<Option<Video>, Refusal> {
        #[cfg(feature = "db")]
        return crate::query::video(&self.conn, id);
        #[cfg(not(feature = "db"))]
        return self.never(id);
    }

    /// Every channel, by score.
    ///
    /// # Errors
    ///
    /// As [`Catalog::ranked`].
    pub fn channels(&self) -> Result<Vec<Channel>, Refusal> {
        #[cfg(feature = "db")]
        return crate::query::channels(&self.conn);
        #[cfg(not(feature = "db"))]
        return self.never(());
    }

    /// The videos changed since the catalogue's generation `generation` —
    /// what the index's crawler takes — and the generation now.
    ///
    /// # Errors
    ///
    /// As [`Catalog::ranked`].
    pub fn all_since(&self, generation: u64) -> Result<(Vec<Video>, u64), Refusal> {
        #[cfg(feature = "db")]
        return crate::query::all_since(&self.conn, generation);
        #[cfg(not(feature = "db"))]
        return self.never(generation);
    }

    /// The ids taken out since `generation` (deleted, unavailable) — what
    /// the index's crawler removes.
    ///
    /// # Errors
    ///
    /// As [`Catalog::ranked`].
    pub fn gone_since(&self, generation: u64) -> Result<Vec<VideoId>, Refusal> {
        #[cfg(feature = "db")]
        return crate::query::gone_since(&self.conn, generation);
        #[cfg(not(feature = "db"))]
        return self.never(generation);
    }

    /// What the last scan did, when there was one.
    ///
    /// # Errors
    ///
    /// As [`Catalog::ranked`].
    pub fn last_scan(&self) -> Result<Option<ScanRecord>, Refusal> {
        #[cfg(feature = "db")]
        return crate::query::last_scan(&self.conn);
        #[cfg(not(feature = "db"))]
        return self.never(());
    }

    /// How many ids lie under a tombstone, by reason — every reason, zero
    /// included: what was never catalogued, or was taken out, and why.
    ///
    /// # Errors
    ///
    /// As [`Catalog::ranked`].
    pub fn skipped(&self) -> Result<Vec<(Skip, u64)>, Refusal> {
        #[cfg(feature = "db")]
        return crate::query::skipped(&self.conn);
        #[cfg(not(feature = "db"))]
        return self.never(());
    }

    /// Hides a video (it stays catalogued; a hidden list shows it).
    ///
    /// # Errors
    ///
    /// [`crate::Why::NotFound`], [`crate::Why::Busy`],
    /// [`crate::Why::Failed`].
    pub fn hide(&self, id: &VideoId) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::set_hidden(
            &crate::db::open_rw(&self.path)?,
            id,
            true,
            crate::db::now_s(),
        );
        #[cfg(not(feature = "db"))]
        return self.never(id);
    }

    /// Deletes a video and leaves its tombstone: it never comes back. Its
    /// thumbnail goes with it.
    ///
    /// # Errors
    ///
    /// As [`Catalog::hide`].
    pub fn delete(&self, id: &VideoId) -> Result<(), Refusal> {
        #[cfg(feature = "db")]
        return crate::edit::delete(&crate::db::open_rw(&self.path)?, id, crate::db::now_s())
            .map(|thumb| crate::thumbs::forget(thumb.as_deref()));
        #[cfg(not(feature = "db"))]
        return self.never(id);
    }

    /// What a method of a catalogue that cannot exist answers: a build
    /// without `db` never makes one ([`Catalog::open_ro`] refuses).
    #[cfg(not(feature = "db"))]
    fn never<T, A>(&self, _arguments: A) -> T {
        match self.never {}
    }
}
