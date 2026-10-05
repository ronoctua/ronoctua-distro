//! Every write to the catalogue: the person's edits (channels, rules,
//! weights, ratings, the second fields, hiding and deleting), the scanner's
//! (videos, details, tombstones, what a channel's scan ended in), and the
//! recompute that follows either.
//!
//! # One transaction per gesture
//!
//! Each edit is one immediate transaction, and whatever it changes in the
//! score is recomputed inside it: a reader never sees a rule that changed
//! with the points of the old one. A write that changes a video moves the
//! catalogue's generation and stamps the video with it
//! (`Catalog::all_since`).
//!
//! # The full-text index
//!
//! `videos_fts` is contentless (`content=''`, `contentless_delete=1`): it
//! keeps the words and not a second copy of every description. So it is
//! refreshed HERE, by the one function every text write ends in
//! ([`refresh_fts`]), keyed by the video's `num` — an `INTEGER PRIMARY KEY`,
//! which a `VACUUM` never renumbers.

use std::collections::HashMap;

use ronoctua_ytdlp::youtube::{Details, is_channel_id};
use rusqlite::{Connection, OptionalExtension as _, Transaction, params};

use crate::db::{self, sql};
use crate::person::NewChannel;
use crate::score::{self, Facts};
use crate::{
    CHANNEL_SCORE, CHANNEL_SCORE_MOST, ChannelId, Fields, Rating, Refusal, Rule, RuleId, RuleKind,
    Settings, Skip, TermMatch, VideoId, Why,
};

/// How a video came to be called a Short, strongest first. The feed of
/// Shorts and the Shorts tab are YouTube's own lists; a `/shorts/` link is
/// its address; `media_type` is what yt-dlp read; the shape (≤ 180 s and
/// taller than wide) is a guess, and the only one a later answer overrides.
pub(crate) mod evidence {
    /// In the channel's Shorts feed (`UUSH…`).
    pub(crate) const FEED: &str = "feed";
    /// In the channel's `/shorts` tab.
    pub(crate) const TAB: &str = "tab";
    /// Its address is `/shorts/…`.
    pub(crate) const LINK: &str = "link";
    /// yt-dlp's `media_type` said `short`.
    pub(crate) const YOUTUBE: &str = "youtube";
    /// At most 180 s and taller than wide.
    pub(crate) const SHAPE: &str = "shape";
}

/// The longest a Short can be, in seconds — the shape test's bound.
pub(crate) const SHORT_MOST_S: u32 = 180;

/// How many failed details make a video `failed` (it is asked no more).
pub(crate) const DETAILS_TRIES: i64 = 3;

// ------------------------------------------------------------------ rules

/// Every rule of the catalogue, in the order they were kept.
pub(crate) fn all_rules(conn: &Connection) -> Result<Vec<Rule>, Refusal> {
    let mut statement = conn
        .prepare_cached(
            "SELECT id, channel, kind, fields, pattern, how, points, min_s, max_s, days, enabled
             FROM rules ORDER BY id",
        )
        .map_err(sql)?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, Option<i64>>(9)?,
                r.get::<_, bool>(10)?,
            ))
        })
        .map_err(sql)?;
    let mut rules = Vec::new();
    for row in rows {
        let (id, channel, kind, fields, pattern, how, points, min_s, max_s, days, enabled) =
            row.map_err(sql)?;
        // A KIND THIS BUILD DOES NOT KNOW is refused, never read as a rule
        // it resembles: a score computed without it would be a wrong score.
        let unreadable = || Refusal::of(Why::Unreadable);
        let seconds = |v: Option<i64>| v.and_then(|v| u32::try_from(v).ok()).ok_or_else(unreadable);
        let kind = match kind.as_str() {
            "term" => RuleKind::Term {
                fields: Fields(u8::try_from(fields).map_err(|_| unreadable())?),
                pattern,
                how: TermMatch::of(&how).ok_or_else(unreadable)?,
            },
            "band" => RuleKind::Band {
                min_s: seconds(min_s)?,
                max_s: seconds(max_s)?,
            },
            "preferred" => RuleKind::Preferred {
                min_s: seconds(min_s)?,
                max_s: seconds(max_s)?,
            },
            "short" => RuleKind::Short,
            "recency" => RuleKind::Recency {
                days: seconds(days)?,
            },
            "live" => RuleKind::Live,
            _ => return Err(unreadable()),
        };
        rules.push(Rule {
            id: Some(RuleId(id)),
            channel: channel.map(ChannelId),
            kind,
            points: i32::try_from(points).map_err(|_| unreadable())?,
            enabled,
        });
    }
    Ok(rules)
}

/// Refuses a rule the score cannot use — never kept to be read as another.
///
/// Recency with a NEGATIVE R or no days would score as no recency at all (a
/// video older than none, points fading from below zero): the person who
/// typed it is told, and nothing is switched off behind their back. Zero
/// points is how recency is switched off, and says so.
fn valid(rule: &Rule) -> Result<(), Refusal> {
    let invalid = Err(Refusal::of(Why::Invalid));
    match &rule.kind {
        RuleKind::Term {
            pattern, fields, ..
        } if score::words_of(pattern).is_empty() || fields.0 == 0 => invalid,
        RuleKind::Band { min_s, max_s } | RuleKind::Preferred { min_s, max_s } if min_s > max_s => {
            invalid
        }
        RuleKind::Recency { days } if *days == 0 || rule.points < 0 => invalid,
        _ if rule.channel.as_ref().is_some_and(|c| !is_channel_id(&c.0)) => invalid,
        _ => Ok(()),
    }
}

/// Inserts a rule as it is, and answers its id.
pub(crate) fn insert_rule(tx: &Transaction<'_>, rule: &Rule) -> Result<RuleId, Refusal> {
    let (fields, pattern, how, min_s, max_s, days) = columns_of(&rule.kind);
    tx.execute(
        "INSERT INTO rules (channel, kind, fields, pattern, how, points, min_s, max_s, days, enabled)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            rule.channel.as_ref().map(|c| c.0.as_str()),
            rule.kind.id(),
            fields,
            pattern,
            how,
            rule.points,
            min_s,
            max_s,
            days,
            rule.enabled,
        ],
    )
    .map_err(sql)?;
    Ok(RuleId(tx.last_insert_rowid()))
}

type Columns<'a> = (
    i64,
    &'a str,
    &'static str,
    Option<u32>,
    Option<u32>,
    Option<u32>,
);

fn columns_of(kind: &RuleKind) -> Columns<'_> {
    match kind {
        RuleKind::Term {
            fields,
            pattern,
            how,
        } => (
            i64::from(fields.0),
            pattern.as_str(),
            how.id(),
            None,
            None,
            None,
        ),
        RuleKind::Band { min_s, max_s } | RuleKind::Preferred { min_s, max_s } => (
            0,
            "",
            TermMatch::Word.id(),
            Some(*min_s),
            Some(*max_s),
            None,
        ),
        RuleKind::Recency { days } => (0, "", TermMatch::Word.id(), None, None, Some(*days)),
        RuleKind::Short | RuleKind::Live => (0, "", TermMatch::Word.id(), None, None, None),
    }
}

/// Keeps a rule — a new one when its id is `None`, else in place — and
/// rescores what it reaches, in one transaction. A single rule (see
/// [`RuleKind::is_single`]) new in a scope that has one REPLACES it: one per
/// scope is what the score reads.
pub(crate) fn put_rule(conn: &Connection, rule: &Rule) -> Result<RuleId, Refusal> {
    valid(rule)?;
    let tx = db::immediate(conn)?;
    if let Some(channel) = &rule.channel {
        channel_exists(&tx, channel)?;
    }
    let existing = match rule.id {
        Some(id) => Some(id),
        None if rule.kind.is_single() => tx
            .query_row(
                "SELECT id FROM rules WHERE kind = ?1 AND channel IS ?2 ORDER BY id LIMIT 1",
                params![rule.kind.id(), rule.channel.as_ref().map(|c| c.0.as_str())],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?
            .map(RuleId),
        None => None,
    };
    // WHERE IT WAS, for a rule edited in place: moved from one scope to
    // another, it stops reaching where it was, and that is rescored too.
    let was: Option<Option<String>> = match existing {
        Some(id) => tx
            .query_row(
                "SELECT channel FROM rules WHERE id = ?1",
                params![id.0],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?,
        None => None,
    };
    let id = match existing {
        Some(id) => {
            let (fields, pattern, how, min_s, max_s, days) = columns_of(&rule.kind);
            let changed = tx
                .execute(
                    "UPDATE rules SET channel = ?1, kind = ?2, fields = ?3, pattern = ?4, how = ?5,
                       points = ?6, min_s = ?7, max_s = ?8, days = ?9, enabled = ?10
                     WHERE id = ?11",
                    params![
                        rule.channel.as_ref().map(|c| c.0.as_str()),
                        rule.kind.id(),
                        fields,
                        pattern,
                        how,
                        rule.points,
                        min_s,
                        max_s,
                        days,
                        rule.enabled,
                        id.0,
                    ],
                )
                .map_err(sql)?;
            if changed == 0 {
                return Err(Refusal::of(Why::NotFound));
            }
            id
        }
        None => insert_rule(&tx, rule)?,
    };
    let generation = db::bump(&tx)?;
    // A GLOBAL RULE reaches every channel that has no version of its own;
    // rescoring them all is the one answer that cannot miss one. A channel's
    // rule reaches its channel — and the one it was moved from.
    let now_at = rule.channel.as_ref().map(|c| c.0.clone());
    match (&now_at, was) {
        (Some(channel), None) => {
            rescore(&tx, Scope::Channel(&ChannelId(channel.clone())), generation)?;
        }
        (Some(channel), Some(Some(before))) if *channel == before => {
            rescore(&tx, Scope::Channel(&ChannelId(before)), generation)?;
        }
        (Some(channel), Some(Some(before))) => {
            rescore(&tx, Scope::Channel(&ChannelId(channel.clone())), generation)?;
            rescore(&tx, Scope::Channel(&ChannelId(before)), generation)?;
        }
        _ => {
            rescore(&tx, Scope::All, generation)?;
        }
    }
    tx.commit().map_err(sql)?;
    Ok(id)
}

/// Removes a rule and rescores what it reached.
pub(crate) fn remove_rule(conn: &Connection, id: RuleId) -> Result<(), Refusal> {
    let tx = db::immediate(conn)?;
    let channel: Option<Option<String>> = tx
        .query_row(
            "DELETE FROM rules WHERE id = ?1 RETURNING channel",
            params![id.0],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    let Some(channel) = channel else {
        return Err(Refusal::of(Why::NotFound));
    };
    let generation = db::bump(&tx)?;
    match channel.map(ChannelId) {
        Some(channel) => rescore(&tx, Scope::Channel(&channel), generation)?,
        None => rescore(&tx, Scope::All, generation)?,
    };
    tx.commit().map_err(sql)
}

/// The rules in force for `channel`: its own (on or off, so a door can switch
/// one back on) and every global rule it has no version of — a version of a
/// single kind switched off still stands in for the global one, because it
/// is that kind switched off for the channel (`score::in_force`), and no
/// band list counts until one of its bands is on; every global rule for
/// `None`.
pub(crate) fn rules_for(
    conn: &Connection,
    channel: Option<&ChannelId>,
) -> Result<Vec<Rule>, Refusal> {
    let all = all_rules(conn)?;
    let Some(channel) = channel else {
        return Ok(all.into_iter().filter(|r| r.channel.is_none()).collect());
    };
    let own: Vec<Rule> = all
        .iter()
        .filter(|r| r.channel.as_ref() == Some(channel))
        .cloned()
        .collect();
    let overrides = |global: &Rule| {
        own.iter().any(|r| match (&r.kind, &global.kind) {
            (RuleKind::Term { .. }, _) => false,
            (RuleKind::Band { .. }, RuleKind::Band { .. }) => r.enabled,
            (mine, theirs) => mine.is_single() && mine.id() == theirs.id(),
        })
    };
    let mut rules: Vec<Rule> = all
        .iter()
        .filter(|r| r.channel.is_none() && !overrides(r))
        .cloned()
        .collect();
    rules.extend(own);
    Ok(rules)
}

// --------------------------------------------------------------- channels

fn channel_exists(conn: &Connection, id: &ChannelId) -> Result<(), Refusal> {
    let there: Option<i64> = conn
        .query_row("SELECT 1 FROM channels WHERE id = ?1", params![id.0], |r| {
            r.get(0)
        })
        .optional()
        .map_err(sql)?;
    there.map(|_| ()).ok_or(Refusal::of(Why::NotFound))
}

/// Adds a channel at [`CHANNEL_SCORE`]; one already there keeps its score
/// and its reference, and takes the new name, handle and address.
pub(crate) fn add_channel(
    conn: &Connection,
    channel: &NewChannel,
    now: i64,
) -> Result<(), Refusal> {
    if !is_channel_id(&channel.id.0) {
        return Err(Refusal::of(Why::Invalid));
    }
    let tx = db::immediate(conn)?;
    tx.execute(
        "INSERT INTO channels (id, handle, title, url, avatar_url, score, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (id) DO UPDATE SET
           handle = excluded.handle, title = excluded.title, url = excluded.url,
           avatar_url = coalesce(excluded.avatar_url, avatar_url)",
        params![
            channel.id.0,
            channel.handle,
            channel.title,
            channel.url,
            channel.avatar_url,
            CHANNEL_SCORE,
            now,
        ],
    )
    .map_err(sql)?;
    let generation = db::bump(&tx)?;
    rescore(&tx, Scope::Channel(&channel.id), generation)?;
    tx.commit().map_err(sql)
}

/// Sets a channel's score, clamped to 0–[`CHANNEL_SCORE_MOST`].
pub(crate) fn set_channel_score(
    conn: &Connection,
    id: &ChannelId,
    score: i32,
) -> Result<(), Refusal> {
    let changed = conn
        .execute(
            "UPDATE channels SET score = ?1 WHERE id = ?2",
            params![score.clamp(0, CHANNEL_SCORE_MOST), id.0],
        )
        .map_err(sql)?;
    if changed == 0 {
        return Err(Refusal::of(Why::NotFound));
    }
    Ok(())
}

/// Sets whether a channel's Shorts are blocked (`None`: as the global
/// switch), and hides or brings back its Shorts to match.
pub(crate) fn set_channel_block_shorts(
    conn: &Connection,
    id: &ChannelId,
    block: Option<bool>,
    now: i64,
) -> Result<(), Refusal> {
    let tx = db::immediate(conn)?;
    let changed = tx
        .execute(
            "UPDATE channels SET block_shorts = ?1 WHERE id = ?2",
            params![block, id.0],
        )
        .map_err(sql)?;
    if changed == 0 {
        return Err(Refusal::of(Why::NotFound));
    }
    let generation = db::bump(&tx)?;
    apply_blocking(&tx, Some(id), generation, now)?;
    tx.commit().map_err(sql)
}

/// Removes a channel, its rules and its videos; every video id stays as a
/// tombstone, so adding the channel again starts from its first-scan window
/// and not from its whole past. Answers the videos' thumbnails, for the
/// caller to remove once this has committed ([`take_out`]).
pub(crate) fn remove_channel(
    conn: &Connection,
    id: &ChannelId,
    now: i64,
) -> Result<Vec<String>, Refusal> {
    let tx = db::immediate(conn)?;
    channel_exists(&tx, id)?;
    let generation = db::bump(&tx)?;
    let videos: Vec<String> = {
        let mut statement = tx
            .prepare("SELECT id FROM videos WHERE channel = ?1")
            .map_err(sql)?;
        let rows = statement
            .query_map(params![id.0], |r| r.get(0))
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)?
    };
    let mut thumbs = Vec::new();
    for video in &videos {
        thumbs.extend(take_out(
            &tx,
            &VideoId(video.clone()),
            Skip::Deleted,
            now,
            generation,
        )?);
    }
    tx.execute("DELETE FROM rules WHERE channel = ?1", params![id.0])
        .map_err(sql)?;
    tx.execute("DELETE FROM channels WHERE id = ?1", params![id.0])
        .map_err(sql)?;
    tx.commit().map_err(sql)?;
    Ok(thumbs)
}

/// What a channel's scan ended in: on success the reference moves to when
/// the scan STARTED (what was published while it ran is taken next time),
/// and the error is cleared; on failure only the try and its words are kept.
pub(crate) fn channel_scanned(
    tx: &Transaction<'_>,
    id: &ChannelId,
    error: Option<&str>,
    started_at: i64,
    now: i64,
) -> Result<(), Refusal> {
    match error {
        None => tx.execute(
            "UPDATE channels SET reference_at = ?1, first_done = 1, last_try_at = ?2,
               last_error = NULL WHERE id = ?3",
            params![started_at, now, id.0],
        ),
        Some(error) => tx.execute(
            "UPDATE channels SET last_try_at = ?1, last_error = ?2 WHERE id = ?3",
            params![now, error, id.0],
        ),
    }
    .map(|_| ())
    .map_err(sql)
}

/// Whether a channel's Shorts are blocked: its own switch, else the global.
pub(crate) fn blocks_shorts(
    conn: &Connection,
    channel: &ChannelId,
    settings: &Settings,
) -> Result<bool, Refusal> {
    let own: Option<Option<bool>> = conn
        .query_row(
            "SELECT block_shorts FROM channels WHERE id = ?1",
            params![channel.0],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    Ok(own.flatten().unwrap_or(settings.block_shorts))
}

/// Hides the Shorts of every channel (or one) that blocks them, with the
/// reason `blocked-short`, and brings back the ones hidden for that reason
/// where Shorts are no longer blocked. The person's own hiding is left alone.
pub(crate) fn apply_blocking(
    tx: &Transaction<'_>,
    channel: Option<&ChannelId>,
    generation: i64,
    now: i64,
) -> Result<(), Refusal> {
    let settings = db::settings(tx)?;
    let channels: Vec<String> = match channel {
        Some(c) => vec![c.0.clone()],
        None => {
            let mut statement = tx.prepare("SELECT id FROM channels").map_err(sql)?;
            let rows = statement.query_map([], |r| r.get(0)).map_err(sql)?;
            rows.collect::<Result<_, _>>().map_err(sql)?
        }
    };
    for id in channels {
        if blocks_shorts(tx, &ChannelId(id.clone()), &settings)? {
            tx.execute(
                "UPDATE videos SET hidden = 1, hidden_at = ?1, hidden_reason = ?2, generation = ?3
                 WHERE channel = ?4 AND is_short = 1 AND hidden = 0",
                params![now, Skip::BlockedShort.id(), generation, id],
            )
        } else {
            tx.execute(
                "UPDATE videos SET hidden = 0, hidden_at = NULL, hidden_reason = NULL,
                   generation = ?1
                 WHERE channel = ?2 AND hidden_reason = ?3",
                params![generation, id, Skip::BlockedShort.id()],
            )
        }
        .map_err(sql)?;
    }
    Ok(())
}

/// Keeps the switches; blocking Shorts hides the ones already in, and
/// unblocking brings back what blocking hid.
pub(crate) fn set_settings(
    conn: &Connection,
    settings: &Settings,
    now: i64,
) -> Result<(), Refusal> {
    let tx = db::immediate(conn)?;
    db::put_settings(&tx, settings)?;
    let generation = db::bump(&tx)?;
    apply_blocking(&tx, None, generation, now)?;
    tx.commit().map_err(sql)
}

/// Keeps the rating weights and rescores every video.
pub(crate) fn set_weights(conn: &Connection, weights: &crate::Weights) -> Result<(), Refusal> {
    let tx = db::immediate(conn)?;
    db::put_weights(&tx, weights)?;
    let generation = db::bump(&tx)?;
    rescore(&tx, Scope::All, generation)?;
    tx.commit().map_err(sql)
}

// ----------------------------------------------------------------- videos

/// A video the scanner found, before its details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewVideo<'a> {
    pub(crate) id: &'a VideoId,
    pub(crate) channel: &'a ChannelId,
    pub(crate) url: &'a str,
    pub(crate) title: &'a str,
    pub(crate) description: &'a str,
    pub(crate) published_at_s: Option<i64>,
    pub(crate) duration_s: Option<u32>,
    pub(crate) short_evidence: Option<&'static str>,
    pub(crate) live_status: Option<&'a str>,
    pub(crate) thumb_url: Option<&'a str>,
    pub(crate) views: Option<u64>,
}

/// Whether an id is in the catalogue or among its tombstones.
pub(crate) fn known(conn: &Connection, id: &VideoId) -> Result<bool, Refusal> {
    let seen: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM videos WHERE id = ?1 UNION ALL SELECT 1 FROM skipped WHERE video = ?1
             LIMIT 1",
            params![id.0],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    Ok(seen.is_some())
}

/// Catalogues a video (its details pending); `false` when it was already
/// there. Its points are the channel's rescore's, after the scan's inserts.
pub(crate) fn insert_video(
    tx: &Transaction<'_>,
    video: &NewVideo<'_>,
    views_snapshots: bool,
    generation: i64,
    now: i64,
) -> Result<bool, Refusal> {
    let added = tx
        .execute(
            "INSERT INTO videos (id, channel, url, title, description, duration_s, published_at,
               first_seen_at, is_short, short_evidence, live_status, thumb_url, updated_at,
               generation)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?8, ?13)
             ON CONFLICT (id) DO NOTHING",
            params![
                video.id.0,
                video.channel.0,
                video.url,
                video.title,
                video.description,
                video.duration_s,
                video.published_at_s,
                now,
                video.short_evidence.is_some(),
                video.short_evidence,
                video.live_status,
                video.thumb_url,
                generation,
            ],
        )
        .map_err(sql)?
        == 1;
    if added {
        if views_snapshots {
            keep_views(tx, video.id, video.views, now)?;
        }
        refresh_fts(tx, video.id)?;
    }
    Ok(added)
}

fn keep_views(
    tx: &Transaction<'_>,
    id: &VideoId,
    views: Option<u64>,
    now: i64,
) -> Result<(), Refusal> {
    let Some(views) = views.and_then(|v| i64::try_from(v).ok()) else {
        return Ok(());
    };
    tx.execute(
        "INSERT INTO views (video, at, count) VALUES (?1, ?2, ?3)
         ON CONFLICT (video, at) DO UPDATE SET count = excluded.count",
        params![id.0, now, views],
    )
    .map(|_| ())
    .map_err(sql)
}

/// Leaves a tombstone for an id that is not (or no longer) catalogued. The
/// first reason stays, except that a deletion or an unavailability
/// overwrites: they are what the id IS now.
pub(crate) fn tombstone(
    tx: &Transaction<'_>,
    id: &VideoId,
    channel: Option<&ChannelId>,
    reason: Skip,
    generation: i64,
    now: i64,
) -> Result<(), Refusal> {
    let overwrite = matches!(reason, Skip::Deleted | Skip::Unavailable);
    tx.execute(
        if overwrite {
            "INSERT INTO skipped (video, channel, reason, at, generation)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (video) DO UPDATE SET reason = excluded.reason, at = excluded.at,
               generation = excluded.generation"
        } else {
            "INSERT INTO skipped (video, channel, reason, at, generation)
             VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (video) DO NOTHING"
        },
        params![
            id.0,
            channel.map(|c| c.0.as_str()),
            reason.id(),
            now,
            generation
        ],
    )
    .map(|_| ())
    .map_err(sql)
}

/// Takes a video out of the catalogue — row, tags, views, words — leaving a
/// tombstone; answers its thumbnail's file, for the caller to remove once
/// the transaction is committed.
pub(crate) fn take_out(
    tx: &Transaction<'_>,
    id: &VideoId,
    reason: Skip,
    now: i64,
    generation: i64,
) -> Result<Option<String>, Refusal> {
    let row: Option<(i64, String, Option<String>)> = tx
        .query_row(
            "SELECT num, channel, thumb_file FROM videos WHERE id = ?1",
            params![id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((num, channel, thumb)) = row else {
        return Err(Refusal::of(Why::NotFound));
    };
    tx.execute("DELETE FROM videos_fts WHERE rowid = ?1", params![num])
        .map_err(sql)?;
    tx.execute("DELETE FROM video_tags WHERE video = ?1", params![id.0])
        .map_err(sql)?;
    tx.execute("DELETE FROM views WHERE video = ?1", params![id.0])
        .map_err(sql)?;
    tx.execute("DELETE FROM videos WHERE num = ?1", params![num])
        .map_err(sql)?;
    tombstone(tx, id, Some(&ChannelId(channel)), reason, generation, now)?;
    Ok(thumb)
}

/// Deletes a video for good: it never comes back.
pub(crate) fn delete(conn: &Connection, id: &VideoId, now: i64) -> Result<Option<String>, Refusal> {
    let tx = db::immediate(conn)?;
    let generation = db::bump(&tx)?;
    let thumb = take_out(&tx, id, Skip::Deleted, now, generation)?;
    tx.commit().map_err(sql)?;
    Ok(thumb)
}

/// Hides a video (`true`) or shows it again (`false`); the person's hiding,
/// whatever hid it before.
pub(crate) fn set_hidden(
    conn: &Connection,
    id: &VideoId,
    hidden: bool,
    now: i64,
) -> Result<(), Refusal> {
    let tx = db::immediate(conn)?;
    let generation = db::bump(&tx)?;
    let changed = if hidden {
        tx.execute(
            "UPDATE videos SET hidden = 1, hidden_at = ?1, hidden_reason = 'person',
               generation = ?2 WHERE id = ?3",
            params![now, generation, id.0],
        )
    } else {
        tx.execute(
            "UPDATE videos SET hidden = 0, hidden_at = NULL, hidden_reason = NULL,
               generation = ?1 WHERE id = ?2",
            params![generation, id.0],
        )
    }
    .map_err(sql)?;
    if changed == 0 {
        return Err(Refusal::of(Why::NotFound));
    }
    tx.commit().map_err(sql)
}

/// Rates a video (`None` clears it) and rescores it.
pub(crate) fn set_rating(
    conn: &Connection,
    id: &VideoId,
    rating: Option<Rating>,
    now: i64,
) -> Result<(), Refusal> {
    person_write(conn, id, |tx, generation| {
        tx.execute(
            "UPDATE videos SET rating = ?1, updated_at = ?2, generation = ?3 WHERE id = ?4",
            params![rating.map(Rating::number), now, generation, id.0],
        )
        .map_err(sql)
    })
}

/// Replaces a video's second description.
pub(crate) fn set_second_description(
    conn: &Connection,
    id: &VideoId,
    text: &str,
    now: i64,
) -> Result<(), Refusal> {
    person_write(conn, id, |tx, generation| {
        tx.execute(
            "UPDATE videos SET second_description = ?1, updated_at = ?2, generation = ?3
             WHERE id = ?4",
            params![text, now, generation, id.0],
        )
        .map_err(sql)
    })
}

/// Replaces a video's second tags (trimmed, empty ones and repeats dropped).
pub(crate) fn set_second_tags(
    conn: &Connection,
    id: &VideoId,
    tags: &[String],
    now: i64,
) -> Result<(), Refusal> {
    person_write(conn, id, |tx, generation| {
        let changed = tx
            .execute(
                "UPDATE videos SET updated_at = ?1, generation = ?2 WHERE id = ?3",
                params![now, generation, id.0],
            )
            .map_err(sql)?;
        if changed == 1 {
            put_tags(tx, id, true, tags)?;
        }
        Ok(changed)
    })
}

/// One edit of the person's on one video: the write, then its points and
/// its words, in one transaction.
fn person_write(
    conn: &Connection,
    id: &VideoId,
    write: impl FnOnce(&Transaction<'_>, i64) -> Result<usize, Refusal>,
) -> Result<(), Refusal> {
    let tx = db::immediate(conn)?;
    let generation = db::bump(&tx)?;
    if write(&tx, generation)? == 0 {
        return Err(Refusal::of(Why::NotFound));
    }
    rescore(&tx, Scope::Video(id), generation)?;
    refresh_fts(&tx, id)?;
    tx.commit().map_err(sql)
}

/// Replaces one set of a video's tags (YouTube's, or the person's with
/// `own`).
fn put_tags(tx: &Transaction<'_>, id: &VideoId, own: bool, tags: &[String]) -> Result<(), Refusal> {
    tx.execute(
        "DELETE FROM video_tags WHERE video = ?1 AND own = ?2",
        params![id.0, own],
    )
    .map_err(sql)?;
    for tag in tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        tx.execute(
            "INSERT INTO video_tags (video, own, tag) VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING",
            params![id.0, own, tag],
        )
        .map_err(sql)?;
    }
    Ok(())
}

/// A video's tags: YouTube's, then the person's.
pub(crate) fn tags_of(
    conn: &Connection,
    id: &VideoId,
) -> Result<(Vec<String>, Vec<String>), Refusal> {
    let mut statement = conn
        // IN THE ORDER THEY WERE GIVEN: the table keeps its rowid for that.
        .prepare_cached("SELECT own, tag FROM video_tags WHERE video = ?1 ORDER BY own, rowid")
        .map_err(sql)?;
    let rows = statement
        .query_map(params![id.0], |r| {
            Ok((r.get::<_, bool>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(sql)?;
    let (mut theirs, mut own) = (Vec::new(), Vec::new());
    for row in rows {
        let (mine, tag) = row.map_err(sql)?;
        if mine {
            own.push(tag)
        } else {
            theirs.push(tag)
        }
    }
    Ok((theirs, own))
}

/// Replaces a video's entry in the full-text index with its words now.
pub(crate) fn refresh_fts(tx: &Transaction<'_>, id: &VideoId) -> Result<(), Refusal> {
    let row: Option<(i64, String, String, String)> = tx
        .query_row(
            "SELECT num, title, description, second_description FROM videos WHERE id = ?1",
            params![id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((num, title, description, second)) = row else {
        return Ok(());
    };
    let (tags, own) = tags_of(tx, id)?;
    tx.execute("DELETE FROM videos_fts WHERE rowid = ?1", params![num])
        .map_err(sql)?;
    tx.execute(
        "INSERT INTO videos_fts (rowid, title, description, tags, second_description, second_tags)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            num,
            title,
            description,
            tags.join(" "),
            second,
            own.join(" ")
        ],
    )
    .map(|_| ())
    .map_err(sql)
}

/// yt-dlp's `live_status` of a premiere or a stream not yet on.
pub(crate) const UPCOMING: &str = "is_upcoming";

/// Fills a video's fields from its details, decides again whether it is a
/// Short, hides it when its channel blocks Shorts, and rescores it.
///
/// NO BLANK OVER WHAT IS KNOWN: an answer without a title, a description or a
/// duration leaves the one the feed or an earlier answer gave. And A VIDEO
/// NOT YET OUT (a premiere, a scheduled stream: [`UPCOMING`]) has no duration
/// to give: it stays `pending`, asked again once its time has come
/// ([`PENDING`]); still not out by then, each answer counts as a failed try.
pub(crate) fn apply_details(
    tx: &Transaction<'_>,
    id: &VideoId,
    details: &Details,
    views_snapshots: bool,
    generation: i64,
    now: i64,
) -> Result<(), Refusal> {
    let row: Option<(String, bool, Option<String>)> = tx
        .query_row(
            "SELECT channel, is_short, short_evidence FROM videos WHERE id = ?1",
            params![id.0],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((channel, was_short, was_evidence)) = row else {
        return Err(Refusal::of(Why::NotFound));
    };
    let (is_short, evidence) = short_after_details(was_short, was_evidence.as_deref(), details);
    let upcoming = details.live_status.as_deref() == Some(UPCOMING);
    let state = if upcoming { "pending" } else { "done" };
    tx.execute(
        "UPDATE videos SET
           title = CASE WHEN ?1 = '' THEN title ELSE ?1 END,
           description = CASE WHEN ?2 = '' THEN description ELSE ?2 END,
           duration_s = coalesce(?3, duration_s), published_at = coalesce(?4, published_at),
           width = ?5, height = ?6, language = ?7, audio_language = ?8, live_status = ?9,
           age_limit = ?10, availability = ?11, chapters = ?12, captions = ?13, categories = ?14,
           is_short = ?15, short_evidence = ?16, details_state = ?20, updated_at = ?17,
           generation = ?18
         WHERE id = ?19",
        params![
            details.title,
            details.description,
            details.duration_s,
            details.published_at_s,
            details.size_px.map(|(w, _)| w),
            details.size_px.map(|(_, h)| h),
            details.language,
            details.audio_language,
            details.live_status,
            details.age_limit,
            details.availability,
            json(&details.chapters),
            json(&details.captions),
            json(&details.categories),
            is_short,
            evidence,
            now,
            generation,
            id.0,
            state,
        ],
    )
    .map_err(sql)?;
    // ITS TIME HAS COME and it is still not out: a delayed premiere, or a
    // stream that never started — asked again, a bounded number of times.
    if upcoming && details.published_at_s.is_none_or(|at| at <= now) {
        details_failed(tx, id)?;
    }
    put_tags(tx, id, false, &details.tags)?;
    if views_snapshots {
        keep_views(tx, id, details.views, now)?;
    }
    let channel = ChannelId(channel);
    if is_short && blocks_shorts(tx, &channel, &db::settings(tx)?)? {
        tx.execute(
            "UPDATE videos SET hidden = 1, hidden_at = ?1, hidden_reason = ?2
             WHERE id = ?3 AND hidden = 0",
            params![now, Skip::BlockedShort.id(), id.0],
        )
        .map_err(sql)?;
    }
    rescore(tx, Scope::Video(id), generation)?;
    refresh_fts(tx, id)
}

/// JSON of what a column keeps as JSON; `[]` if it would not serialise.
fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_owned())
}

/// Whether a video is a Short once its details are known, and why. Pure.
///
/// YouTube's own lists and its `/shorts/` address stand; `media_type` decides
/// otherwise; the shape (≤ [`SHORT_MOST_S`] and taller than wide) decides
/// only what nothing else did.
pub(crate) fn short_after_details(
    was_short: bool,
    evidence: Option<&str>,
    details: &Details,
) -> (bool, Option<&'static str>) {
    let listed = match evidence {
        Some(evidence::FEED) => Some(evidence::FEED),
        Some(evidence::TAB) => Some(evidence::TAB),
        Some(evidence::LINK) => Some(evidence::LINK),
        _ => None,
    };
    if let Some(listed) = listed {
        return (was_short, Some(listed));
    }
    match details.short {
        Some(true) => return (true, Some(evidence::YOUTUBE)),
        Some(false) => return (false, None),
        None => {}
    }
    let shaped = details.duration_s.is_some_and(|d| d <= SHORT_MOST_S)
        && details.size_px.is_some_and(|(w, h)| h > w);
    if shaped {
        (true, Some(evidence::SHAPE))
    } else {
        (false, None)
    }
}

/// Which videos a details phase asks, newest first: every `pending` one but
/// a video not yet out whose time has not come (`?1` is now, `?2` the most).
/// The ones not yet out come last: a scheduled stream is asked after
/// everything that can answer.
pub(crate) const PENDING: &str = "SELECT id, url FROM videos
   WHERE details_state = 'pending'
     AND NOT (live_status IS 'is_upcoming' AND coalesce(published_at, 0) > ?1)
   ORDER BY live_status IS 'is_upcoming', published_at DESC NULLS LAST, num
   LIMIT ?2";

/// Counts a failed attempt at a video's details; at [`DETAILS_TRIES`] it is
/// `failed` and asked no more.
pub(crate) fn details_failed(tx: &Transaction<'_>, id: &VideoId) -> Result<(), Refusal> {
    tx.execute(
        "UPDATE videos SET details_tries = details_tries + 1,
           details_state = CASE WHEN details_tries + 1 >= ?1 THEN 'failed' ELSE details_state END
         WHERE id = ?2",
        params![DETAILS_TRIES, id.0],
    )
    .map(|_| ())
    .map_err(sql)
}

// ---------------------------------------------------------------- scoring

/// What a recompute reaches.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Scope<'a> {
    /// Every channel and video.
    All,
    /// One channel and its videos.
    Channel(&'a ChannelId),
    /// One video.
    Video(&'a VideoId),
}

/// One video's row, as the score reads it.
struct Scored {
    num: i64,
    id: String,
    channel: String,
    duration_s: Option<u32>,
    is_short: bool,
    live_status: Option<String>,
    title: String,
    description: String,
    second_description: String,
    rating: Option<i64>,
    points: i64,
}

/// Recomputes the static points of what `scope` reaches, and stores each
/// channel's resolved recency (R, D) on its row for the read-time SQL.
/// Answers how many videos changed. Inside the caller's transaction.
pub(crate) fn rescore(
    tx: &Transaction<'_>,
    scope: Scope<'_>,
    generation: i64,
) -> Result<u64, Refusal> {
    let rules = all_rules(tx)?;
    let weights = db::weights(tx)?;
    let (where_, key): (&str, Option<&str>) = match scope {
        // THE SAME ONE PARAMETER in every shape: NULL here, so `?1 IS NULL`
        // holds for every row.
        Scope::All => ("?1 IS NULL", None),
        Scope::Channel(c) => ("v.channel = ?1", Some(c.0.as_str())),
        Scope::Video(v) => ("v.id = ?1", Some(v.0.as_str())),
    };
    let videos: Vec<Scored> = {
        let mut statement = tx
            .prepare(&format!(
                "SELECT v.num, v.id, v.channel, v.duration_s, v.is_short, v.live_status, v.title,
                   v.description, v.second_description, v.rating, v.static_points
                 FROM videos v WHERE {where_}"
            ))
            .map_err(sql)?;
        let rows = statement
            .query_map(params![key], |r| {
                Ok(Scored {
                    num: r.get(0)?,
                    id: r.get(1)?,
                    channel: r.get(2)?,
                    duration_s: r.get(3)?,
                    is_short: r.get(4)?,
                    live_status: r.get(5)?,
                    title: r.get(6)?,
                    description: r.get(7)?,
                    second_description: r.get(8)?,
                    rating: r.get(9)?,
                    points: r.get(10)?,
                })
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)?
    };
    // EVERY TAG OF WHAT IS REACHED, in one pass rather than one query a video.
    let mut tags: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    {
        let mut statement = tx
            .prepare(&format!(
                "SELECT t.video, t.own, t.tag FROM video_tags t JOIN videos v ON v.id = t.video
                 WHERE {where_}"
            ))
            .map_err(sql)?;
        let rows = statement
            .query_map(params![key], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(sql)?;
        for row in rows {
            let (video, own, tag) = row.map_err(sql)?;
            let entry = tags.entry(video).or_default();
            if own {
                entry.1.push(tag)
            } else {
                entry.0.push(tag)
            }
        }
    }
    // THE CHANNELS REACHED, each resolved once — a channel with no videos
    // yet included, so its recency is on its row before the first arrives.
    let mut channels: Vec<String> = match scope {
        Scope::All => {
            let mut statement = tx.prepare("SELECT id FROM channels").map_err(sql)?;
            let rows = statement.query_map([], |r| r.get(0)).map_err(sql)?;
            rows.collect::<Result<_, _>>().map_err(sql)?
        }
        Scope::Channel(c) => vec![c.0.clone()],
        Scope::Video(_) => Vec::new(),
    };
    channels.extend(videos.iter().map(|v| v.channel.clone()));
    channels.sort();
    channels.dedup();
    let mut in_force = HashMap::new();
    for channel in channels {
        let resolved = score::in_force(&rules, Some(&ChannelId(channel.clone())));
        tx.execute(
            "UPDATE channels SET recency_points = ?1, recency_days = ?2 WHERE id = ?3",
            params![resolved.recency_points, resolved.recency_days, channel],
        )
        .map_err(sql)?;
        in_force.insert(channel, resolved);
    }
    let empty = (Vec::new(), Vec::new());
    let mut changed = 0_u64;
    let mut update = tx
        .prepare("UPDATE videos SET static_points = ?1, generation = ?2 WHERE num = ?3")
        .map_err(sql)?;
    for video in &videos {
        let Some(rules) = in_force.get(&video.channel) else {
            continue;
        };
        let rating = match video.rating {
            None => None,
            Some(n) => Some(Rating::of_number(n).ok_or(Refusal::of(Why::Unreadable))?),
        };
        let (theirs, own) = tags.get(&video.id).unwrap_or(&empty);
        let facts = Facts {
            duration_s: video.duration_s,
            is_short: video.is_short,
            live: score::is_live(video.live_status.as_deref()),
            title: &video.title,
            description: &video.description,
            tags: theirs,
            second_description: &video.second_description,
            second_tags: own,
            rating,
        };
        let points = score::static_points(rules, &weights, &facts);
        if points != video.points {
            update
                .execute(params![points, generation, video.num])
                .map_err(sql)?;
            changed += 1;
        }
    }
    Ok(changed)
}

/// Recomputes every video's static points in one transaction.
pub(crate) fn rescore_all(conn: &Connection) -> Result<u64, Refusal> {
    let tx = db::immediate(conn)?;
    let generation = db::bump(&tx)?;
    let changed = rescore(&tx, Scope::All, generation)?;
    tx.commit().map_err(sql)?;
    Ok(changed)
}
