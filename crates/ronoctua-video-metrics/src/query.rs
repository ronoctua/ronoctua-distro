//! Every read of the catalogue: a page of videos by score, how many a filter
//! keeps, one video whole, the channels, what changed since a generation,
//! and the last scan.
//!
//! # The order and the page are SQL's
//!
//! The total is computed in the query ([`TOTAL`]) — the channel's score, the
//! video's static points and its recency — and `ORDER BY`/`LIMIT`/`OFFSET`
//! take the page there: a door never holds fifty thousand rows to show
//! twenty. Recency is `floor(R · max(0, D − age) / D)` with the age in whole
//! days from `unixepoch('now')`, so a list read tomorrow is tomorrow's; R is
//! never negative (the recompute clamps it), so SQLite's integer division,
//! which truncates, IS the floor [`crate::score::recency`] takes.

use std::collections::HashMap;
use std::path::PathBuf;

use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension as _, Row, ToSql, params};

use crate::db::{self, sql};
use crate::read::{Channel, Chapter, Filter, Hidden, Page, RankedVideo, ScanRecord, Shorts, Video};
use crate::{ChannelId, Rating, Refusal, Skip, Stop, VideoId, Why};

/// The most rows one page may ask for.
pub(crate) const PAGE_MOST: u32 = 1000;

/// A video's total score, with `:now` the moment it is read at (NULL: now).
pub(crate) const TOTAL: &str = "(c.score + v.static_points + CASE
    WHEN c.recency_days > 0 AND v.published_at IS NOT NULL THEN
      (c.recency_points * max(0, c.recency_days
        - max(0, (coalesce(:now, unixepoch('now')) - v.published_at) / 86400)))
      / c.recency_days
    ELSE 0 END)";

/// The columns of a ranked row, in [`ranked_of`]'s order.
const RANKED: &str = "v.id, v.url, v.title, c.title, v.channel, TOTAL, v.thumb_file,
    v.duration_s, v.published_at, v.is_short, v.hidden";

/// The words of a full-text search as an FTS5 query: each word quoted (a
/// quote inside doubled), all of them required, the last one a prefix — so
/// what is typed finds as it is typed. `None` when there are no words.
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let words = crate::score::words_of(text);
    let last = words.len().checked_sub(1)?;
    Some(
        words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let quoted = format!("\"{}\"", w.replace('"', "\"\""));
                if i == last { quoted + "*" } else { quoted }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// The `WHERE` of a filter, and its named parameters.
fn where_of(filter: &Filter) -> (String, Vec<(&'static str, Value)>) {
    let mut clauses = vec!["1 = 1".to_owned()];
    let mut values = Vec::new();
    if let Some(channel) = &filter.channel {
        clauses.push("v.channel = :channel".to_owned());
        values.push((":channel", Value::Text(channel.0.clone())));
    }
    match filter.shorts {
        Shorts::Any => {}
        Shorts::Only => clauses.push("v.is_short = 1".to_owned()),
        Shorts::Without => clauses.push("v.is_short = 0".to_owned()),
    }
    if let Some(rating) = filter.rating {
        clauses.push("v.rating = :rating".to_owned());
        values.push((":rating", Value::Integer(rating.number())));
    }
    match filter.hidden {
        Hidden::Any => {}
        Hidden::Only => clauses.push("v.hidden = 1".to_owned()),
        Hidden::Without => clauses.push("v.hidden = 0".to_owned()),
    }
    if let Some(query) = filter.text.as_deref().and_then(fts_query) {
        clauses.push(
            "v.num IN (SELECT rowid FROM videos_fts WHERE videos_fts MATCH :text)".to_owned(),
        );
        values.push((":text", Value::Text(query)));
    }
    if let Some(after) = filter.published_after_s {
        clauses.push("v.published_at >= :after".to_owned());
        values.push((":after", Value::Integer(after)));
    }
    if let Some(before) = filter.published_before_s {
        clauses.push("v.published_at < :before".to_owned());
        values.push((":before", Value::Integer(before)));
    }
    (clauses.join(" AND "), values)
}

fn named<'a>(values: &'a [(&'static str, Value)]) -> Vec<(&'static str, &'a dyn ToSql)> {
    values.iter().map(|(n, v)| (*n, v as &dyn ToSql)).collect()
}

/// A page of videos by score, best first, as of `now_s` (`None`: now).
pub(crate) fn ranked(
    conn: &Connection,
    filter: &Filter,
    page: Page,
    now_s: Option<i64>,
) -> Result<Vec<RankedVideo>, Refusal> {
    let (where_, mut values) = where_of(filter);
    let per_page = page.per_page.clamp(1, PAGE_MOST);
    values.push((":now", now_s.map_or(Value::Null, Value::Integer)));
    values.push((":limit", Value::Integer(i64::from(per_page))));
    values.push((
        ":offset",
        Value::Integer(i64::from(page.number) * i64::from(per_page)),
    ));
    let text = format!(
        "SELECT {} FROM videos v JOIN channels c ON c.id = v.channel WHERE {where_}
         ORDER BY 6 DESC, v.published_at DESC NULLS LAST, v.id LIMIT :limit OFFSET :offset",
        RANKED.replace("TOTAL", TOTAL)
    );
    let mut statement = conn.prepare_cached(&text).map_err(sql)?;
    let rows = statement
        .query_map(named(&values).as_slice(), ranked_of)
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

/// A ranked row, from [`RANKED`]'s columns.
fn ranked_of(r: &Row<'_>) -> rusqlite::Result<RankedVideo> {
    Ok(RankedVideo {
        id: VideoId(r.get(0)?),
        url: r.get(1)?,
        title: r.get(2)?,
        channel: r.get(3)?,
        channel_id: ChannelId(r.get(4)?),
        score: r.get(5)?,
        thumb_file: r.get::<_, Option<String>>(6)?.map(PathBuf::from),
        duration_s: r.get(7)?,
        published_at_s: r.get(8)?,
        is_short: r.get(9)?,
        hidden: r.get(10)?,
    })
}

/// How many videos `filter` keeps.
pub(crate) fn count(conn: &Connection, filter: &Filter) -> Result<u64, Refusal> {
    let (where_, values) = where_of(filter);
    let text =
        format!("SELECT count(*) FROM videos v JOIN channels c ON c.id = v.channel WHERE {where_}");
    let mut statement = conn.prepare_cached(&text).map_err(sql)?;
    let n: i64 = statement
        .query_row(named(&values).as_slice(), |r| r.get(0))
        .map_err(sql)?;
    Ok(u64::try_from(n).unwrap_or(0))
}

/// The columns of a whole video: the ranked ones, then the rest, in
/// [`video_of`]'s order.
const WHOLE: &str = "v.description, v.first_seen_at, v.live_status, v.width, v.height,
    v.language, v.audio_language, v.age_limit, v.chapters, v.captions, v.categories,
    v.second_description, v.rating, v.static_points";

/// Every field of the videos a `WHERE` names — their tags and latest view
/// counts in one query each, not one a video (the index's first crawl reads
/// the whole catalogue).
fn videos_where(
    conn: &Connection,
    where_: &str,
    values: &[(&'static str, Value)],
) -> Result<Vec<Video>, Refusal> {
    let from = format!("FROM videos v JOIN channels c ON c.id = v.channel WHERE {where_}");
    let mut all = Vec::new();
    {
        let mut with_now = values.to_vec();
        with_now.push((":now", Value::Null));
        let text = format!(
            "SELECT {}, {WHOLE} {from} ORDER BY v.num",
            RANKED.replace("TOTAL", TOTAL)
        );
        let mut statement = conn.prepare(&text).map_err(sql)?;
        let mut rows = statement.query(named(&with_now).as_slice()).map_err(sql)?;
        while let Some(row) = rows.next().map_err(sql)? {
            all.push(video_of(row)?);
        }
    }
    let mut tags: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    {
        let text = format!(
            "SELECT t.video, t.own, t.tag FROM video_tags t JOIN videos v ON v.id = t.video
               JOIN channels c ON c.id = v.channel WHERE {where_} ORDER BY t.own, t.rowid"
        );
        let mut statement = conn.prepare(&text).map_err(sql)?;
        let mut rows = statement.query(named(values).as_slice()).map_err(sql)?;
        while let Some(row) = rows.next().map_err(sql)? {
            let video: String = row.get(0).map_err(sql)?;
            let own: bool = row.get(1).map_err(sql)?;
            let tag: String = row.get(2).map_err(sql)?;
            let entry = tags.entry(video).or_default();
            if own {
                entry.1.push(tag)
            } else {
                entry.0.push(tag)
            }
        }
    }
    let mut views: HashMap<String, i64> = HashMap::new();
    {
        // OLDEST FIRST, so the last one kept for a video is its latest.
        let text = format!(
            "SELECT w.video, w.count FROM views w JOIN videos v ON v.id = w.video
               JOIN channels c ON c.id = v.channel WHERE {where_} ORDER BY w.at"
        );
        let mut statement = conn.prepare(&text).map_err(sql)?;
        let mut rows = statement.query(named(values).as_slice()).map_err(sql)?;
        while let Some(row) = rows.next().map_err(sql)? {
            views.insert(row.get(0).map_err(sql)?, row.get(1).map_err(sql)?);
        }
    }
    for video in &mut all {
        if let Some((theirs, own)) = tags.remove(&video.row.id.0) {
            video.tags = theirs;
            video.second_tags = own;
        }
        video.views = views
            .get(&video.row.id.0)
            .and_then(|v| u64::try_from(*v).ok());
    }
    Ok(all)
}

/// A video's row read whole (tags and views are filled after).
fn video_of(r: &Row<'_>) -> Result<Video, Refusal> {
    let row = ranked_of(r).map_err(sql)?;
    let at = |i: usize| i + 11;
    let json = |i: usize| -> Result<String, Refusal> { r.get(at(i)).map_err(sql) };
    let unreadable = || Refusal::of(Why::Unreadable);
    let chapters: Vec<Chapter> = serde_json::from_str(&json(8)?).map_err(|_| unreadable())?;
    let captions: Vec<String> = serde_json::from_str(&json(9)?).map_err(|_| unreadable())?;
    let categories: Vec<String> = serde_json::from_str(&json(10)?).map_err(|_| unreadable())?;
    let width: Option<u32> = r.get(at(3)).map_err(sql)?;
    let height: Option<u32> = r.get(at(4)).map_err(sql)?;
    let rating = match r.get::<_, Option<i64>>(at(12)).map_err(sql)? {
        None => None,
        Some(n) => Some(Rating::of_number(n).ok_or_else(unreadable)?),
    };
    let live: Option<String> = r.get(at(2)).map_err(sql)?;
    Ok(Video {
        row,
        description: r.get(at(0)).map_err(sql)?,
        first_seen_at_s: r.get(at(1)).map_err(sql)?,
        live: crate::score::is_live(live.as_deref()),
        size_px: width.zip(height),
        languages: (r.get(at(5)).map_err(sql)?, r.get(at(6)).map_err(sql)?),
        age_limit: r.get(at(7)).map_err(sql)?,
        chapters,
        captions,
        categories,
        tags: Vec::new(),
        second_tags: Vec::new(),
        second_description: r.get(at(11)).map_err(sql)?,
        rating,
        static_points: r.get(at(13)).map_err(sql)?,
        views: None,
    })
}

/// Every field of one video; `None` when there is no such video.
pub(crate) fn video(conn: &Connection, id: &VideoId) -> Result<Option<Video>, Refusal> {
    Ok(
        videos_where(conn, "v.id = :id", &[(":id", Value::Text(id.0.clone()))])?
            .into_iter()
            .next(),
    )
}

/// The videos changed since `generation`, and the generation now.
pub(crate) fn all_since(conn: &Connection, generation: u64) -> Result<(Vec<Video>, u64), Refusal> {
    let since = i64::try_from(generation).unwrap_or(i64::MAX);
    let now = db::generation(conn)?;
    let videos = videos_where(
        conn,
        "v.generation > :since",
        &[(":since", Value::Integer(since))],
    )?;
    Ok((videos, u64::try_from(now).unwrap_or(0)))
}

/// The ids taken out of the catalogue (deleted, unavailable) since
/// `generation`.
pub(crate) fn gone_since(conn: &Connection, generation: u64) -> Result<Vec<VideoId>, Refusal> {
    let since = i64::try_from(generation).unwrap_or(i64::MAX);
    let mut statement = conn
        .prepare_cached(
            "SELECT video FROM skipped WHERE generation > ?1 AND reason IN (?2, ?3)
             ORDER BY video",
        )
        .map_err(sql)?;
    let rows = statement
        .query_map(
            params![
                since,
                crate::Skip::Deleted.id(),
                crate::Skip::Unavailable.id()
            ],
            |r| Ok(VideoId(r.get(0)?)),
        )
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

/// How many ids lie under a tombstone, by reason — every reason, in
/// [`Skip::ALL`]'s order, none left out for being zero. A reason this build
/// does not know is refused, never counted as another.
pub(crate) fn skipped(conn: &Connection) -> Result<Vec<(Skip, u64)>, Refusal> {
    let mut statement = conn
        .prepare_cached("SELECT reason, count(*) FROM skipped GROUP BY reason")
        .map_err(sql)?;
    let rows = statement
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(sql)?;
    let mut counts: Vec<(Skip, u64)> = Skip::ALL.iter().map(|s| (*s, 0)).collect();
    for row in rows {
        let (reason, n) = row.map_err(sql)?;
        let slot = counts
            .iter_mut()
            .find(|(s, _)| s.id() == reason)
            .ok_or(Refusal::of(Why::Unreadable))?;
        slot.1 = u64::try_from(n).unwrap_or(0);
    }
    Ok(counts)
}

/// Every channel, by score, best first.
pub(crate) fn channels(conn: &Connection) -> Result<Vec<Channel>, Refusal> {
    let mut statement = conn
        .prepare_cached(
            "SELECT c.id, c.handle, c.title, c.url, c.avatar_url, c.score, c.added_at,
               c.reference_at, c.last_try_at, c.last_error, c.block_shorts,
               (SELECT count(*) FROM videos v WHERE v.channel = c.id)
             FROM channels c ORDER BY c.score DESC, c.title, c.id",
        )
        .map_err(sql)?;
    let rows = statement
        .query_map([], |r| {
            Ok(Channel {
                id: ChannelId(r.get(0)?),
                handle: r.get(1)?,
                title: r.get(2)?,
                url: r.get(3)?,
                avatar_url: r.get(4)?,
                score: r.get(5)?,
                added_at_s: r.get(6)?,
                reference_at_s: r.get(7)?,
                last_try_at_s: r.get(8)?,
                last_error: r.get(9)?,
                block_shorts: r.get(10)?,
                videos: u64::try_from(r.get::<_, i64>(11)?).unwrap_or(0),
            })
        })
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

/// The last scan's record, when there was one.
pub(crate) fn last_scan(conn: &Connection) -> Result<Option<ScanRecord>, Refusal> {
    let row = conn
        .query_row(
            "SELECT started_at, ended_at, channels, channels_failed, added, skipped, detailed,
               pending, thumbs, stopped, said
             FROM scans ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                let count = |i: usize| -> rusqlite::Result<u32> {
                    Ok(u32::try_from(r.get::<_, i64>(i)?).unwrap_or(u32::MAX))
                };
                Ok((
                    ScanRecord {
                        started_at_s: r.get(0)?,
                        ended_at_s: r.get(1)?,
                        channels: count(2)?,
                        channels_failed: count(3)?,
                        added: count(4)?,
                        skipped: count(5)?,
                        detailed: count(6)?,
                        pending: count(7)?,
                        thumbs: count(8)?,
                        stopped: None,
                        said: r.get(10)?,
                    },
                    r.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .optional()
        .map_err(sql)?;
    let Some((mut record, stopped)) = row else {
        return Ok(None);
    };
    record.stopped = match stopped {
        None => None,
        // A STOP THIS BUILD DOES NOT KNOW is refused, never shown as none.
        Some(id) => Some(Stop::of(&id).ok_or(Refusal::of(Why::Unreadable))?),
    };
    Ok(Some(record))
}
