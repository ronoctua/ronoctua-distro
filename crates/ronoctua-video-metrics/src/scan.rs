//! The scanner: what `ronoctua-video-metrics scan` (the unit) runs.
//!
//! Without an API key: new ids from each channel's uploads feeds (`UULF…`
//! videos, `UUSH…` Shorts; the `channel_id` feed as a fallback, see
//! [`crate::feed`]); a backlog larger than a feed's fifteen through a flat tab
//! of `ronoctua-ytdlp`; details by `yt-dlp -j`, one every 4±2 s, at most
//! [`Options::details_most`] per run (the rest stays pending); thumbnails
//! (`mqdefault`) into the cache under `videoMetricsThumbsMib`. A bot check or
//! an HTTP 429 stops the whole run and is recorded.
//!
//! # The decisions are here, the network is not
//!
//! Everything a run DECIDES — which ids are new, what the first scan takes
//! ([`plan`]), when a feed is hiding a backlog, what a failure does to a
//! channel's reference — is in [`run_with`], over any [`Reach`]: the tests
//! hand it a fake one and read the catalogue after. The one [`Reach`] that
//! goes out is `net`'s ([`run`]).
//!
//! # The first scan's safeguard
//!
//! A channel with no reference yet takes only what was published in the last
//! `first_scan_days` (14), at most the newest `first_scan_most` (10); every
//! other id it sees becomes a tombstone (`first-scan`). On success its
//! reference is the moment the scan STARTED; later scans take only ids in no
//! table and published at or after `reference − 1 day`, and tombstone the
//! older ones they meet (`before-reference`). An entry with no date is left
//! for a scan that knows it: nothing is decided without one.
//!
//! # A channel that failed keeps its reference
//!
//! What was read is catalogued, and the reference does not move: the next
//! scan asks again from the same point, so a feed that failed for a day
//! loses nothing.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ronoctua_ytdlp::youtube::{Details, Tab, TabEntry};
use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};

use crate::db::{self, sql};
use crate::edit::{self, NewVideo, evidence};
use crate::feed::{self, FEED_LEN, Which};
use crate::read::{Filter, Page};
use crate::thumbs::Thumbs;
use crate::{ChannelId, Refusal, Settings, Skip, VideoId, Why};

pub use crate::Stop;

/// A day, in seconds.
pub const DAY_S: i64 = 86_400;

/// How many entries of a tab a backlog lists.
pub const BACKLOG_MOST: usize = 50;

/// How many of the best videos get their missing thumbnail fetched again.
pub const THUMBS_TOP: u32 = 200;

/// What a run may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Options {
    /// The most details fetched in one run.
    pub details_most: u32,
    /// Only walk the feeds; fetch no details and no thumbnails (a rehearsal).
    pub feeds_only: bool,
}

impl Default for Options {
    /// The design's run: 60 details at most.
    fn default() -> Self {
        Self {
            details_most: 60,
            feeds_only: false,
        }
    }
}

/// What a run did.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Report {
    /// Channels whose feeds were read.
    pub channels: u32,
    /// Channels whose feeds failed (all failing is the brain's danger mark).
    pub channels_failed: u32,
    /// Videos catalogued.
    pub added: u32,
    /// Videos tombstoned (first scan, before the reference, blocked Shorts,
    /// unavailable).
    pub skipped: u32,
    /// Details fetched.
    pub detailed: u32,
    /// Details still pending for the next run.
    pub pending: u32,
    /// Thumbnails fetched.
    pub thumbs: u32,
    /// Why it stopped early, when it did.
    pub stopped: Option<Stop>,
    /// What the tool said when it stopped — data, for a log.
    pub said: Option<String>,
}

/// Why something a run asked did not come back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missed {
    /// Stop the whole run (a bot check, a 429, no network, no consent), with
    /// what the tool said.
    Stop(Stop, Option<String>),
    /// There is no such feed (a channel without Shorts has no `UUSH…`).
    NotFound,
    /// The video is private, removed or not available: never again.
    Gone,
    /// This installation has no yt-dlp: no detail can be asked this run.
    NoTool,
    /// Anything else, in the tool's words (data, for the channel's error).
    Failed(String),
}

/// What reaches YouTube: the feeds, the tabs, a video's details, a
/// thumbnail — and the pause between two details.
pub trait Reach {
    /// One feed's bytes.
    ///
    /// # Errors
    ///
    /// [`Missed`].
    fn feed(&self, which: Which, channel: &ChannelId) -> Result<Vec<u8>, Missed>;

    /// One tab, flat, newest first.
    ///
    /// # Errors
    ///
    /// [`Missed`].
    fn tab(&self, channel: &ChannelId, tab: Tab, most: usize) -> Result<Vec<TabEntry>, Missed>;

    /// One video's details.
    ///
    /// # Errors
    ///
    /// [`Missed`].
    fn details(&self, url: &str) -> Result<Details, Missed>;

    /// One video's thumbnail's bytes.
    ///
    /// # Errors
    ///
    /// [`Missed`].
    fn thumb(&self, id: &VideoId) -> Result<Vec<u8>, Missed>;

    /// Waits between two details (4±2 s on the network; nothing in a test).
    fn pause(&self) {}
}

/// A video a feed or a tab named, before it is decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Its id.
    pub id: VideoId,
    /// Its address.
    pub url: String,
    /// Its title.
    pub title: String,
    /// Its description, when the feed had it.
    pub description: String,
    /// When it was published, when known.
    pub published_at_s: Option<i64>,
    /// Its duration, when the tab said.
    pub duration_s: Option<u32>,
    /// Why it is a Short, when something says so (the Shorts feed or tab, a
    /// `/shorts/` link).
    pub short: Option<&'static str>,
    /// yt-dlp's `live_status`, when the tab said.
    pub live_status: Option<String>,
    /// The feed's thumbnail address.
    pub thumb_url: Option<String>,
    /// Its view count, when said.
    pub views: Option<u64>,
}

impl Candidate {
    /// An entry of a feed; `shorts` for the Shorts feed.
    #[must_use]
    pub fn of_entry(entry: &feed::Entry, shorts: bool) -> Self {
        Self {
            id: entry.id.clone(),
            url: entry.url.clone(),
            title: entry.title.clone(),
            description: entry.description.clone(),
            published_at_s: entry.published_at_s,
            duration_s: None,
            short: if shorts {
                Some(evidence::FEED)
            } else if entry.links_a_short() {
                Some(evidence::LINK)
            } else {
                None
            },
            live_status: None,
            thumb_url: entry.thumb_url.clone(),
            views: entry.views,
        }
    }

    /// An entry of a tab.
    #[must_use]
    pub fn of_tab(entry: &TabEntry, tab: Tab) -> Self {
        Self {
            id: VideoId(entry.id.clone()),
            url: entry.url.clone(),
            title: entry.title.clone(),
            description: String::new(),
            published_at_s: entry.published_at_s,
            duration_s: entry.duration_s,
            short: match tab {
                Tab::Shorts => Some(evidence::TAB),
                Tab::Videos if entry.url.contains("/shorts/") => Some(evidence::LINK),
                Tab::Videos => None,
            },
            live_status: entry.live_status.clone(),
            thumb_url: None,
            views: entry.views,
        }
    }

    /// Two sightings of one video as one: what either knew, the stronger
    /// Short evidence.
    fn merged(self, other: Self) -> Self {
        let strength = |e: Option<&str>| match e {
            Some(evidence::FEED) => 3,
            Some(evidence::TAB) => 2,
            Some(evidence::LINK) => 1,
            _ => 0,
        };
        let short = if strength(other.short) > strength(self.short) {
            other.short
        } else {
            self.short
        };
        Self {
            title: if self.title.is_empty() {
                other.title
            } else {
                self.title
            },
            description: if self.description.is_empty() {
                other.description
            } else {
                self.description
            },
            published_at_s: self.published_at_s.or(other.published_at_s),
            duration_s: self.duration_s.or(other.duration_s),
            live_status: self.live_status.or(other.live_status),
            thumb_url: self.thumb_url.or(other.thumb_url),
            views: self.views.or(other.views),
            short,
            ..self
        }
    }
}

/// What a channel's scan decided.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// What is catalogued, newest first.
    pub take: Vec<Candidate>,
    /// What becomes a tombstone, and why.
    pub skip: Vec<(VideoId, Skip)>,
}

/// Decides what a channel's sightings become. Pure: `known` says whether an
/// id is in the catalogue or among its tombstones (those are never decided
/// again — a deleted video never comes back).
#[must_use]
pub fn plan(
    reference_at_s: Option<i64>,
    candidates: Vec<Candidate>,
    known: &dyn Fn(&VideoId) -> bool,
    settings: &Settings,
    block_shorts: bool,
    now_s: i64,
) -> Plan {
    // ONE SIGHTING PER ID, in the order first seen.
    let mut order = Vec::new();
    let mut by_id: HashMap<VideoId, Candidate> = HashMap::new();
    for candidate in candidates {
        match by_id.remove(&candidate.id) {
            Some(seen) => {
                by_id.insert(candidate.id.clone(), seen.merged(candidate));
            }
            None => {
                order.push(candidate.id.clone());
                by_id.insert(candidate.id.clone(), candidate);
            }
        }
    }
    let mut plan = Plan::default();
    let mut dated = Vec::new();
    for id in order {
        let Some(candidate) = by_id.remove(&id) else {
            continue;
        };
        if known(&candidate.id) {
            continue;
        }
        if block_shorts && candidate.short.is_some() {
            plan.skip.push((candidate.id, Skip::BlockedShort));
            continue;
        }
        // NO DATE, NO DECISION: a scan that knows the date decides.
        if candidate.published_at_s.is_some() {
            dated.push(candidate);
        }
    }
    dated.sort_by(|a, b| {
        b.published_at_s
            .cmp(&a.published_at_s)
            .then_with(|| a.id.cmp(&b.id))
    });
    match reference_at_s {
        None => {
            let window = now_s - i64::from(settings.first_scan_days) * DAY_S;
            let most = usize::try_from(settings.first_scan_most).unwrap_or(usize::MAX);
            for candidate in dated {
                let recent = candidate.published_at_s.is_some_and(|p| p >= window);
                if recent && plan.take.len() < most {
                    plan.take.push(candidate);
                } else {
                    plan.skip.push((candidate.id, Skip::FirstScan));
                }
            }
        }
        Some(reference) => {
            let floor = reference - DAY_S;
            for candidate in dated {
                if candidate.published_at_s.is_some_and(|p| p >= floor) {
                    plan.take.push(candidate);
                } else {
                    plan.skip.push((candidate.id, Skip::BeforeReference));
                }
            }
        }
    }
    plan
}

/// Why a channel's scan did not finish, beyond its own failure.
enum Halt {
    /// Stop the run.
    Stop(Stop, Option<String>),
    /// The catalogue refused.
    Refused(Refusal),
}

impl From<Refusal> for Halt {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

/// What one channel's scan did.
#[derive(Default)]
struct Done {
    added: Vec<VideoId>,
    skipped: u32,
    failed: bool,
}

/// One scan of every channel of the catalogue at `path`, reaching YouTube
/// through `reach`, at `now_s`, keeping thumbnails in `thumbs` (none:
/// no thumbnails). Blocks for minutes: the unit's process, never a door.
///
/// # Errors
///
/// The catalogue's refusals; [`Why::Busy`] while another scan runs.
pub fn run_with(
    path: &Path,
    reach: &dyn Reach,
    options: &Options,
    now_s: i64,
    thumbs: Option<&Thumbs>,
) -> Result<Report, Refusal> {
    run_held(path, reach, options, now_s, thumbs, &mut || {})
}

/// [`run_with`], calling `began` once the catalogue is HELD and the scan's
/// record is open — the moment a reader asking [`running`] is told yes. A
/// report written before it would say "running" for a scan no lock says is.
fn run_held(
    path: &Path,
    reach: &dyn Reach,
    options: &Options,
    now_s: i64,
    thumbs: Option<&Thumbs>,
    began: &mut dyn FnMut(),
) -> Result<Report, Refusal> {
    let conn = db::open_rw(path)?;
    let _held = hold(path)?;
    let settings = db::settings(&conn)?;
    let record: i64 = conn
        .query_row(
            "INSERT INTO scans (started_at) VALUES (?1) RETURNING id",
            params![now_s],
            |r| r.get(0),
        )
        .map_err(sql)?;
    began();
    let mut report = Report::default();
    let mut added = Vec::new();
    for (channel, reference) in channels_to_scan(&conn)? {
        report.channels += 1;
        match scan_channel(&conn, reach, &channel, reference, &settings, now_s) {
            Ok(done) => {
                report.skipped += done.skipped;
                report.added += u32::try_from(done.added.len()).unwrap_or(u32::MAX);
                report.channels_failed += u32::from(done.failed);
                added.extend(done.added);
            }
            Err(Halt::Stop(stop, said)) => {
                report.channels_failed += 1;
                report.stopped = Some(stop);
                report.said = said;
                break;
            }
            Err(Halt::Refused(refusal)) => return Err(refusal),
        }
    }
    if report.stopped.is_none() && !options.feeds_only {
        details(&conn, reach, options, &settings, now_s, &mut report)?;
    }
    if report.stopped.is_none()
        && !options.feeds_only
        && let Some(thumbs) = thumbs
    {
        thumbnails(&conn, reach, thumbs, &added, now_s, &mut report)?;
    }
    let pending: i64 = conn
        .query_row(
            "SELECT count(*) FROM videos WHERE details_state = 'pending'",
            [],
            |r| r.get(0),
        )
        .map_err(sql)?;
    report.pending = u32::try_from(pending).unwrap_or(u32::MAX);
    conn.execute(
        "UPDATE scans SET ended_at = ?1, channels = ?2, channels_failed = ?3, added = ?4,
           skipped = ?5, detailed = ?6, pending = ?7, thumbs = ?8, stopped = ?9, said = ?10
         WHERE id = ?11",
        params![
            db::now_s().max(now_s),
            report.channels,
            report.channels_failed,
            report.added,
            report.skipped,
            report.detailed,
            report.pending,
            report.thumbs,
            report.stopped.map(Stop::id),
            report.said,
            record,
        ],
    )
    .map_err(sql)?;
    Ok(report)
}

/// One scan of every channel, over the network. Blocks for minutes.
///
/// # Errors
///
/// As [`run_with`], and [`Why::Invalid`] for a feeds origin override that
/// is not a loopback `http` address.
#[cfg(feature = "net")]
pub fn run(path: &Path, options: &Options) -> Result<Report, Refusal> {
    run_announcing(path, options, &mut || {})
}

/// [`run`], calling `began` once the catalogue is held and the scan's record
/// is open — where the unit writes the brain's "running" report, so the
/// report and the lock ([`running`]) never disagree.
///
/// # Errors
///
/// As [`run`].
#[cfg(feature = "net")]
pub fn run_announcing(
    path: &Path,
    options: &Options,
    began: &mut dyn FnMut(),
) -> Result<Report, Refusal> {
    let reach = crate::reach::Internet::new()?;
    run_held(
        path,
        &reach,
        options,
        db::now_s(),
        Thumbs::of_this_instance().as_ref(),
        began,
    )
}

/// Whether a scan holds the catalogue at `path` NOW: the lock a scan takes
/// before it opens its record, and which goes with its process however that
/// ends — so a record a killed scan left open (`ended_at` empty) is told
/// apart from a scan running. READ-ONLY: the lock file is opened for reading
/// and only asked; no lock file is no scan.
#[must_use]
pub fn running(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(lock_of(path)) else {
        return false;
    };
    matches!(
        file.try_lock_shared(),
        Err(std::fs::TryLockError::WouldBlock)
    )
}

/// The lock file of the catalogue at `path`.
fn lock_of(path: &Path) -> PathBuf {
    path.with_extension("scan.lock")
}

/// The lock that keeps two scans of one catalogue apart (the unit and a
/// person's `scan` at once), held until dropped.
fn hold(path: &Path) -> Result<std::fs::File, Refusal> {
    let lock = lock_of(path);
    let file = std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock)
        .map_err(|_| Refusal::of(Why::Unreadable))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => Err(Refusal::of(Why::Busy)),
        Err(std::fs::TryLockError::Error(_)) => Err(Refusal::of(Why::Failed)),
    }
}

/// Every channel and its reference, the one tried longest ago first — so a
/// run a bot check cuts short leaves the others first in line next time.
fn channels_to_scan(conn: &Connection) -> Result<Vec<(ChannelId, Option<i64>)>, Refusal> {
    let mut statement = conn
        .prepare(
            "SELECT id, reference_at FROM channels
             ORDER BY last_try_at IS NOT NULL, last_try_at, added_at, id",
        )
        .map_err(sql)?;
    let rows = statement
        .query_map([], |r| Ok((ChannelId(r.get(0)?), r.get(1)?)))
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

/// A feed, read: `None` when there is no such feed.
fn feed_of(
    reach: &dyn Reach,
    which: Which,
    channel: &ChannelId,
) -> Result<Result<Option<feed::Feed>, String>, Halt> {
    match reach.feed(which, channel) {
        Ok(bytes) => Ok(feed::parse(&bytes)
            .map(Some)
            .map_err(|unread| format!("the {which:?} feed is not a feed ({unread:?})"))),
        Err(Missed::NotFound) => Ok(Ok(None)),
        Err(Missed::Stop(stop, said)) => Err(Halt::Stop(stop, said)),
        Err(Missed::Failed(words)) => Ok(Err(words)),
        Err(Missed::Gone | Missed::NoTool) => Ok(Err(format!("the {which:?} feed failed"))),
    }
}

/// Whether a feed this full, every entry new and recent, may be hiding more.
fn saturated(feed: &feed::Feed, known: &dyn Fn(&VideoId) -> bool, floor: i64) -> bool {
    feed.entries.len() >= FEED_LEN
        && feed
            .entries
            .iter()
            .all(|e| !known(&e.id) && e.published_at_s.is_some_and(|p| p >= floor))
}

/// Reads one channel's feeds (and its tabs when a feed is hiding a
/// backlog), decides, and writes what it decided in one transaction.
fn scan_channel(
    conn: &Connection,
    reach: &dyn Reach,
    channel: &ChannelId,
    reference: Option<i64>,
    settings: &Settings,
    now_s: i64,
) -> Result<Done, Halt> {
    let block = edit::blocks_shorts(conn, channel, settings)?;
    let known_ids = |id: &VideoId| edit::known(conn, id).unwrap_or(true);
    let floor = reference.map(|r| r - DAY_S);
    let mut candidates = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    let read = |which: Which,
                tab: Tab,
                candidates: &mut Vec<Candidate>,
                errors: &mut Vec<String>|
     -> Result<(), Halt> {
        let fed = match feed_of(reach, which, channel)? {
            Ok(Some(feed)) => Some(feed),
            Ok(None) if which == Which::Videos => {
                // NO VIDEOS PLAYLIST: the whole channel's feed instead, whose
                // Shorts say so by their `/shorts/` links.
                match feed_of(reach, Which::Channel, channel)? {
                    Ok(feed) => feed,
                    Err(words) => {
                        errors.push(words);
                        None
                    }
                }
            }
            Ok(None) => None,
            Err(words) => {
                errors.push(words);
                return Ok(());
            }
        };
        let Some(fed) = fed else {
            return Ok(());
        };
        let backlog = floor.is_some_and(|floor| saturated(&fed, &known_ids, floor));
        candidates.extend(
            fed.entries
                .iter()
                .map(|e| Candidate::of_entry(e, which == Which::Shorts)),
        );
        if backlog {
            match reach.tab(channel, tab, BACKLOG_MOST) {
                Ok(entries) => {
                    candidates.extend(entries.iter().map(|e| Candidate::of_tab(e, tab)));
                }
                Err(Missed::Stop(stop, said)) => return Err(Halt::Stop(stop, said)),
                Err(Missed::Failed(words)) => errors.push(words),
                Err(_) => errors.push(format!("the {tab:?} tab failed")),
            }
        }
        Ok(())
    };
    read(Which::Videos, Tab::Videos, &mut candidates, &mut errors)?;
    // A CHANNEL THAT BLOCKS SHORTS is not asked for them: what would be read
    // is only what would be refused.
    if !block {
        read(Which::Shorts, Tab::Shorts, &mut candidates, &mut errors)?;
    }

    let decided = plan(reference, candidates, &known_ids, settings, block, now_s);
    let tx = db::immediate(conn)?;
    let generation = db::bump(&tx)?;
    let mut done = Done::default();
    for candidate in &decided.take {
        let video = NewVideo {
            id: &candidate.id,
            channel,
            url: &candidate.url,
            title: &candidate.title,
            description: &candidate.description,
            published_at_s: candidate.published_at_s,
            duration_s: candidate.duration_s,
            short_evidence: candidate.short,
            live_status: candidate.live_status.as_deref(),
            thumb_url: candidate.thumb_url.as_deref(),
            views: candidate.views,
        };
        if edit::insert_video(&tx, &video, settings.views_snapshots, generation, now_s)? {
            done.added.push(candidate.id.clone());
        }
    }
    for (id, reason) in &decided.skip {
        edit::tombstone(&tx, id, Some(channel), *reason, generation, now_s)?;
        done.skipped += 1;
    }
    done.failed = !errors.is_empty();
    let error = (!errors.is_empty()).then(|| errors.join("; "));
    edit::channel_scanned(&tx, channel, error.as_deref(), now_s, now_s)?;
    edit::rescore(&tx, edit::Scope::Channel(channel), generation)?;
    tx.commit().map_err(sql)?;
    Ok(done)
}

/// The details phase: the pending videos, newest first ([`edit::PENDING`]: a
/// premiere waits for its time), at most `details_most`, a pause between two.
/// An answer that read nothing of the video (no title) is a failed try, never
/// a video marked done with nothing known.
fn details(
    conn: &Connection,
    reach: &dyn Reach,
    options: &Options,
    settings: &Settings,
    now_s: i64,
    report: &mut Report,
) -> Result<(), Refusal> {
    let pending: Vec<(VideoId, String)> = {
        let mut statement = conn.prepare(edit::PENDING).map_err(sql)?;
        let rows = statement
            .query_map(params![now_s, options.details_most], |r| {
                Ok((VideoId(r.get(0)?), r.get(1)?))
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)?
    };
    for (i, (id, url)) in pending.iter().enumerate() {
        if i > 0 {
            reach.pause();
        }
        let answer = reach.details(url);
        let tx = db::immediate(conn)?;
        let generation = db::bump(&tx)?;
        let mut forget = None;
        match answer {
            // AN ANSWER WITHOUT A TITLE is no answer: yt-dlp read nothing of
            // the video (it makes a title up when it finds none), and taking
            // it would mark the video done with nothing known.
            Ok(details) if details.title.is_empty() => edit::details_failed(&tx, id)?,
            Ok(details) => {
                edit::apply_details(
                    &tx,
                    id,
                    &details,
                    settings.views_snapshots,
                    generation,
                    now_s,
                )?;
                report.detailed += 1;
            }
            Err(Missed::Stop(stop, said)) => {
                report.stopped = Some(stop);
                report.said = said;
                return Ok(());
            }
            Err(Missed::Gone) => {
                forget = edit::take_out(&tx, id, Skip::Unavailable, now_s, generation)?;
                report.skipped += 1;
            }
            // NO yt-dlp HERE: every other video would fail the same way, and
            // counting that against them would mark them failed for a tool.
            Err(Missed::NoTool) => return Ok(()),
            Err(Missed::NotFound | Missed::Failed(_)) => edit::details_failed(&tx, id)?,
        }
        tx.commit().map_err(sql)?;
        crate::thumbs::forget(forget.as_deref());
    }
    Ok(())
}

/// The thumbnails phase: the videos this run added, then the best
/// [`THUMBS_TOP`] whose picture is missing; then the folder is trimmed and
/// the catalogue forgets what the trim removed.
fn thumbnails(
    conn: &Connection,
    reach: &dyn Reach,
    thumbs: &Thumbs,
    added: &[VideoId],
    now_s: i64,
    report: &mut Report,
) -> Result<(), Refusal> {
    let best = crate::query::ranked(
        conn,
        &Filter::default(),
        Page {
            number: 0,
            per_page: THUMBS_TOP,
        },
        Some(now_s),
    )?;
    let mut wanted: Vec<VideoId> = added.to_vec();
    wanted.extend(
        best.into_iter()
            .filter(|row| !row.thumb_file.as_ref().is_some_and(|f| f.is_file()))
            .map(|row| row.id),
    );
    let mut seen = HashSet::new();
    wanted.retain(|id| seen.insert(id.clone()));
    for id in wanted {
        let there: Option<Option<String>> = conn
            .query_row(
                "SELECT thumb_file FROM videos WHERE id = ?1",
                params![id.0],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?;
        match there {
            // Gone meanwhile, or a picture already on disk.
            None => continue,
            Some(Some(file)) if Path::new(&file).is_file() => continue,
            Some(_) => {}
        }
        match reach.thumb(&id) {
            Ok(bytes) => {
                if let Ok(path) = thumbs.keep(&id, &bytes) {
                    conn.execute(
                        "UPDATE videos SET thumb_file = ?1 WHERE id = ?2",
                        params![path.to_string_lossy(), id.0],
                    )
                    .map_err(sql)?;
                    report.thumbs += 1;
                }
            }
            Err(Missed::Stop(stop, said)) => {
                report.stopped = Some(stop);
                report.said = said;
                break;
            }
            Err(_) => {}
        }
    }
    if let Ok(removed) = thumbs.trim() {
        for path in removed {
            conn.execute(
                "UPDATE videos SET thumb_file = NULL WHERE thumb_file = ?1",
                params![path.to_string_lossy()],
            )
            .map_err(sql)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
