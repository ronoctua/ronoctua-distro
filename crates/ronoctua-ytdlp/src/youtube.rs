//! What video-metrics asks yt-dlp about YouTube: who a channel is
//! ([`channel_of`]), what one of its tabs lists ([`tab`]), and every field of
//! one video ([`details`]).
//!
//! The uploads FEEDS are plain HTTP and are not here (`ronoctua-video-metrics`
//! reads them through `ronoctua-http`); what needs YouTube's own pages — a
//! handle turned into a channel id, a backlog longer than a feed's fifteen,
//! the duration, chapters and captions of a video — goes through yt-dlp, and
//! so through this crate, the one place its command lines live.
//!
//! # Why a failure says what yt-dlp said
//!
//! A scanner that asks sixty videos in a row must STOP at the first bot check
//! or HTTP 429 — every further request makes it worse — and must tombstone a
//! private or removed video rather than ask it again tomorrow. The exit status
//! says none of that; the last `ERROR:` line does. So these three read it
//! ([`crate::run::json_telling`]) and sort it ([`Said::of`]); the line itself
//! is kept for a log (data, like a journal line), never shown as a sentence.
//!
//! [`details`] reads the WARNINGS of a run that succeeded too: it asks the
//! fields without the streams (`--ignore-no-formats-error`), and with that
//! flag the pinned yt-dlp says why a video cannot play — a bot check, a rate
//! limit, a private or removed video — as a warning and exits 0 with an
//! answer ([`Said::of_warnings`]).
//!
//! # Read loosely
//!
//! As [`crate::flat`]: the document is a `serde_json::Value` and each field is
//! taken by hand, so one field of an unexpected type costs that field and not
//! the answer.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::address::is_address;
use crate::{DEADLINE, MOST_BYTES, Trouble, run};

/// How long one video's details may take, whole: longer than a listing's
/// [`DEADLINE`], because minting a PO token on demand runs a script first.
pub const DETAILS_DEADLINE: Duration = Duration::from_secs(60);

/// The most entries one tab listing asks for.
pub const TAB_MOST: usize = 200;

/// How long yt-dlp waits on one socket before giving up on it.
const SOCKET_SECONDS: u32 = 10;

/// What yt-dlp's last error line says, as far as a caller must act on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Said {
    /// "Sign in to confirm you're not a bot", a captcha, "your IP is likely
    /// being blocked": stop asking, now.
    BotCheck,
    /// HTTP 429, or "try again later … rate-limited": stop asking, now.
    TooManyRequests,
    /// Private, removed, terminated, members-only, not available here: it
    /// will not answer tomorrow either.
    Gone,
    /// Anything else, or nothing said.
    Other,
}

impl Said {
    /// Sorts one of yt-dlp's lines (its last `ERROR:` line, or a warning).
    /// Pure.
    ///
    /// A STOP IS READ BEFORE A TOMBSTONE. The pinned yt-dlp words YouTube's
    /// rate limit as "Video unavailable. This content isn't available, try
    /// again later. The current session has been rate-limited…" — both at
    /// once — and a tombstone is forever while a stop costs one run: what
    /// reads as both is a stop.
    #[must_use]
    pub fn of(line: Option<&str>) -> Self {
        let Some(line) = line else {
            return Self::Other;
        };
        let line = line.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| line.contains(w));
        if (line.contains("confirm you") && line.contains("bot"))
            || has(&["captcha", "likely being blocked"])
        {
            Self::BotCheck
        } else if has(&[
            "http error 429",
            "too many requests",
            "try again later",
            "rate-limited",
            "rate limited",
        ]) {
            Self::TooManyRequests
        } else if has(&[
            "private video",
            "video unavailable",
            "video is unavailable",
            "has been removed",
            "no longer available",
            "available anymore",
            "has been terminated",
            "members-only",
            "members only",
            "this video is not available",
            "does not exist",
        ]) {
            Self::Gone
        } else {
            Self::Other
        }
    }

    /// What the warnings of a run about ONE video that SUCCEEDED say, when
    /// they say something a caller must act on: the worst of them — a bot
    /// check, then a 429, then gone — with its line. Pure.
    ///
    /// A stop is read from ANY warning: a 429 on one of the pages yt-dlp
    /// fetched is a 429 whoever says it, and a stop is never wrong for long.
    /// Gone only from the VIDEO EXTRACTOR's own warning ([`VIDEO_WARNING`]):
    /// that is where the pinned yt-dlp says why the video cannot play — as
    /// `WARNING: [youtube] This video is unavailable`, WITHOUT the video's id
    /// (measured) — and a plugin's or yt-dlp's own words about something else
    /// never tombstone a video.
    #[must_use]
    pub fn of_warnings(warnings: &[String]) -> Option<(Self, String)> {
        warnings
            .iter()
            .filter_map(|warning| {
                let said = Self::of(Some(warning));
                let counts = match said {
                    Self::BotCheck | Self::TooManyRequests => true,
                    Self::Gone => warning.starts_with(VIDEO_WARNING),
                    Self::Other => false,
                };
                counts.then(|| (said, warning.clone()))
            })
            .min_by_key(|(said, _)| match said {
                Self::BotCheck => 0,
                Self::TooManyRequests => 1,
                Self::Gone | Self::Other => 2,
            })
    }
}

/// How the video extractor's warnings begin: its name (`youtube`, not
/// `youtube:tab`, a plugin's `pot:…`, or none for yt-dlp's own) right after
/// the mark.
pub const VIDEO_WARNING: &str = "WARNING: [youtube] ";

/// A run that did not answer: the trouble, how its words sort, and the words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    /// Which of the closed set.
    pub trouble: Trouble,
    /// What the error line means to a caller.
    pub said: Said,
    /// yt-dlp's last `ERROR:` line, or the warning that decided
    /// ([`Said::of_warnings`]) — data, for a log.
    pub line: Option<String>,
}

impl From<Trouble> for Failed {
    fn from(trouble: Trouble) -> Self {
        Self {
            trouble,
            said: Said::Other,
            line: None,
        }
    }
}

impl From<(Trouble, Option<String>)> for Failed {
    fn from((trouble, line): (Trouble, Option<String>)) -> Self {
        Self {
            trouble,
            said: Said::of(line.as_deref()),
            line,
        }
    }
}

/// A YouTube channel, as yt-dlp names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// Its id (`UC…`).
    pub id: String,
    /// Its handle (`@name`), when it has one.
    pub handle: Option<String>,
    /// Its name — data.
    pub title: String,
    /// Its address, by id.
    pub url: String,
    /// Its picture, when listed.
    pub avatar_url: Option<String>,
}

/// Which tab of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tab {
    /// `/videos`.
    Videos,
    /// `/shorts`.
    Shorts,
}

impl Tab {
    /// The tab's path segment.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Videos => "videos",
            Self::Shorts => "shorts",
        }
    }
}

/// One entry of a listed tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabEntry {
    /// The video id.
    pub id: String,
    /// Its address.
    pub url: String,
    /// Its title; EMPTY is allowed.
    pub title: String,
    /// How long it is, when listed.
    pub duration_s: Option<u32>,
    /// When it was published, seconds since the epoch — APPROXIMATE (the tab
    /// says "3 days ago").
    pub published_at_s: Option<i64>,
    /// yt-dlp's `live_status`, when listed.
    pub live_status: Option<String>,
    /// Its view count, when listed.
    pub views: Option<u64>,
}

/// A chapter of a video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chapter {
    /// Where it starts, in seconds.
    pub start_s: u32,
    /// Its title — data.
    pub title: String,
}

/// Every field of one video that yt-dlp answers without downloading it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Details {
    /// The video id.
    pub id: String,
    /// Its title — data.
    pub title: String,
    /// Its description — data.
    pub description: String,
    /// Its duration, when known.
    pub duration_s: Option<u32>,
    /// When it was published (or, for a premiere, is released), seconds
    /// since the epoch.
    pub published_at_s: Option<i64>,
    /// Its channel's id.
    pub channel_id: Option<String>,
    /// Its channel's name.
    pub channel: Option<String>,
    /// Its picture's width and height, when a format said.
    pub size_px: Option<(u32, u32)>,
    /// Its language, as YouTube tags it.
    pub language: Option<String>,
    /// Its default audio track's language.
    pub audio_language: Option<String>,
    /// `not_live`, `is_live`, `was_live`, `is_upcoming`, `post_live`.
    pub live_status: Option<String>,
    /// Its age limit, in years (0: none).
    pub age_limit: u8,
    /// `public`, `unlisted`, `needs_auth`, `subscriber_only`…
    pub availability: Option<String>,
    /// Its chapters.
    pub chapters: Vec<Chapter>,
    /// The languages of its captions (the uploader's, not the automatic
    /// ones).
    pub captions: Vec<String>,
    /// YouTube's categories.
    pub categories: Vec<String>,
    /// YouTube's tags.
    pub tags: Vec<String>,
    /// Its view count.
    pub views: Option<u64>,
    /// Whether YouTube itself calls it a Short (`media_type`), when it says.
    pub short: Option<bool>,
}

/// Whether `id` is a channel id: `UC` and 22 of letters, digits, `-`, `_`.
#[must_use]
pub fn is_channel_id(id: &str) -> bool {
    id.len() == 24
        && id.starts_with("UC")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A YouTube video id: eleven of letters, digits, `-` and `_`.
fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The hosts a channel address may name.
const HOSTS: [&str; 4] = [
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
];

/// The host of an `http(s)` address, lowercased.
fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    (!authority.contains('@')).then(|| {
        authority
            .split(':')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    })
}

/// The address a channel is asked by, from what a person pasted: `@handle`,
/// a bare channel id, or a YouTube address (a channel's page or one of its
/// videos). Pure.
///
/// # Errors
///
/// [`Trouble::NotAnAddress`] for anything else — another site's address, a
/// handle with characters YouTube does not allow, text.
pub fn channel_address(input: &str) -> Result<String, Trouble> {
    let input = input.trim();
    if let Some(handle) = input.strip_prefix('@') {
        let fits = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '·');
        if (3..=100).contains(&handle.chars().count()) && handle.chars().all(fits) {
            return Ok(format!(
                "https://www.youtube.com/@{}",
                crate::address::escape(handle)
            ));
        }
        return Err(Trouble::NotAnAddress);
    }
    if is_channel_id(input) {
        return Ok(format!("https://www.youtube.com/channel/{input}"));
    }
    let host = host_of(input).ok_or(Trouble::NotAnAddress)?;
    if is_address(input) && (HOSTS.contains(&host.as_str()) || host == "youtu.be") {
        return Ok(input.to_owned());
    }
    Err(Trouble::NotAnAddress)
}

/// The arguments that ask who the channel behind `input` is.
///
/// # Errors
///
/// As [`channel_address`].
pub fn channel_arguments(input: &str) -> Result<Vec<String>, Trouble> {
    let url = channel_address(input)?;
    Ok(strings(&[
        "--ignore-config",
        "--no-warnings",
        "--flat-playlist",
        "--dump-single-json",
        // ONE entry is enough to name the channel; the list is not wanted.
        "--playlist-end",
        "1",
        "--socket-timeout",
        &SOCKET_SECONDS.to_string(),
        "--",
        &url,
    ]))
}

/// Who the channel behind `input` is (`@handle`, an id, a channel's page or
/// one of its videos). Blocks for up to [`DEADLINE`]: call it on a thread.
///
/// # Errors
///
/// [`Trouble::NotAnAddress`], [`Trouble::Absent`], and every trouble of a
/// run and of the reading ([`parse_channel`]), with what yt-dlp said.
pub fn channel_of(input: &str) -> Result<Channel, Failed> {
    let args = channel_arguments(input)?;
    if !crate::installed() {
        return Err(Trouble::Absent.into());
    }
    let (bytes, _) = run::json_telling(&crate::program(), &args, DEADLINE, MOST_BYTES)?;
    Ok(parse_channel(&bytes)?)
}

/// A channel, read out of yt-dlp's answer (a channel's listing or one video).
/// Pure.
///
/// # Errors
///
/// [`Trouble::Garbled`] for what is not a JSON object or names no channel id.
pub fn parse_channel(bytes: &[u8]) -> Result<Channel, Trouble> {
    let top: Value = serde_json::from_slice(bytes).map_err(|_| Trouble::Garbled)?;
    if !top.is_object() {
        return Err(Trouble::Garbled);
    }
    let id = text(&top, "channel_id")
        .or_else(|| text(&top, "id"))
        .filter(|id| is_channel_id(id))
        .ok_or(Trouble::Garbled)?;
    let handle = text(&top, "uploader_id")
        .or_else(|| text(&top, "channel_handle"))
        .filter(|h| h.starts_with('@'));
    let title = text(&top, "channel")
        .or_else(|| text(&top, "uploader"))
        .or_else(|| text(&top, "title"))
        .unwrap_or_default();
    let avatar_url = top
        .get("thumbnails")
        .and_then(Value::as_array)
        .and_then(|all| {
            all.iter()
                .filter(|t| text(t, "id").is_some_and(|i| i.contains("avatar")))
                .max_by_key(|t| t.get("width").and_then(Value::as_u64).unwrap_or(0))
                .and_then(|t| text(t, "url"))
        })
        .filter(|u| is_address(u));
    Ok(Channel {
        url: format!("https://www.youtube.com/channel/{id}"),
        id,
        handle,
        title,
        avatar_url,
    })
}

/// The page of one tab of a channel. Pure.
///
/// # Errors
///
/// [`Trouble::NotAnAddress`] when `channel` is not a channel id.
pub fn tab_address(channel: &str, tab: Tab) -> Result<String, Trouble> {
    if !is_channel_id(channel) {
        return Err(Trouble::NotAnAddress);
    }
    Ok(format!(
        "https://www.youtube.com/channel/{channel}/{}",
        tab.path()
    ))
}

/// The arguments that list a tab flat, newest first, at most `most`.
///
/// # Errors
///
/// As [`tab_address`].
pub fn tab_arguments(channel: &str, tab: Tab, most: usize) -> Result<Vec<String>, Trouble> {
    let url = tab_address(channel, tab)?;
    Ok(strings(&[
        "--ignore-config",
        "--no-warnings",
        "--flat-playlist",
        "--dump-single-json",
        "--playlist-end",
        &most.clamp(1, TAB_MOST).to_string(),
        // "3 DAYS AGO" TURNED INTO A DATE: without it a flat tab has none, and
        // the safeguard cannot tell a backlog from the channel's whole past.
        "--extractor-args",
        "youtubetab:approximate_date",
        "--socket-timeout",
        &SOCKET_SECONDS.to_string(),
        "--",
        &url,
    ]))
}

/// Lists one tab of `channel` flat, newest first, at most `most` entries.
/// Blocks for up to [`DEADLINE`]: call it on a thread.
///
/// # Errors
///
/// [`Trouble::NotAnAddress`], [`Trouble::Absent`], and every trouble of a
/// run and of the reading ([`parse_tab`]), with what yt-dlp said.
pub fn tab(channel: &str, tab: Tab, most: usize) -> Result<Vec<TabEntry>, Failed> {
    let args = tab_arguments(channel, tab, most)?;
    if !crate::installed() {
        return Err(Trouble::Absent.into());
    }
    let (bytes, _) = run::json_telling(&crate::program(), &args, DEADLINE, MOST_BYTES)?;
    Ok(parse_tab(&bytes, most.clamp(1, TAB_MOST))?)
}

/// A tab's listing, read. Pure. Entries without a video id are left out.
///
/// # Errors
///
/// [`Trouble::Garbled`] for what is not a JSON object with an `entries`
/// array.
pub fn parse_tab(bytes: &[u8], most: usize) -> Result<Vec<TabEntry>, Trouble> {
    let top: Value = serde_json::from_slice(bytes).map_err(|_| Trouble::Garbled)?;
    let entries = top
        .get("entries")
        .and_then(Value::as_array)
        .ok_or(Trouble::Garbled)?;
    Ok(entries
        .iter()
        .filter_map(|e| {
            let id = text(e, "id").filter(|id| is_video_id(id))?;
            let url = text(e, "url")
                .filter(|u| is_address(u))
                .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={id}"));
            Some(TabEntry {
                title: text(e, "title").unwrap_or_default(),
                duration_s: seconds(e, "duration"),
                published_at_s: published_of(e),
                live_status: text(e, "live_status"),
                views: e.get("view_count").and_then(Value::as_u64),
                url,
                id,
            })
        })
        .take(most)
        .collect())
}

/// The arguments that ask every field of one video without downloading it.
///
/// `--ignore-no-formats-error`: the fields are wanted, not the streams — a
/// video whose formats need a PO token this installation cannot mint, or a
/// premiere not yet out, still has a title and chapters. With that flag the
/// pinned yt-dlp says why a video cannot play as a WARNING and exits 0, so
/// `--no-warnings` is NOT passed here: [`details`] reads them
/// ([`Said::of_warnings`]). The PO token flags go along
/// ([`crate::pot_flags`]) — this is the one request here that reads a player
/// response.
///
/// # Errors
///
/// [`Trouble::NotAnAddress`] for anything but a plain web address.
pub fn details_arguments(url: &str) -> Result<Vec<String>, Trouble> {
    details_arguments_with(url, &crate::pot_flags())
}

fn details_arguments_with(url: &str, pot: &[String]) -> Result<Vec<String>, Trouble> {
    if !is_address(url) {
        return Err(Trouble::NotAnAddress);
    }
    let mut args = strings(&[
        "--ignore-config",
        "--skip-download",
        "--dump-json",
        "--no-playlist",
        "--ignore-no-formats-error",
        "--socket-timeout",
        &SOCKET_SECONDS.to_string(),
    ]);
    args.extend(pot.iter().cloned());
    args.extend(strings(&["--", url]));
    Ok(args)
}

/// Every field of the video at `url`. Blocks for up to [`DETAILS_DEADLINE`]:
/// call it on a thread.
///
/// # Errors
///
/// [`Trouble::NotAnAddress`], [`Trouble::Absent`], and every trouble of a
/// run and of the reading ([`parse_details`]), with what yt-dlp said — a
/// private or removed video is [`Said::Gone`]. An answer whose warnings say
/// a bot check, a 429 or (about this video) gone is [`Trouble::Failed`] with
/// that warning, whatever fields came with it.
pub fn details(url: &str) -> Result<Details, Failed> {
    let args = details_arguments(url)?;
    if !crate::installed() {
        return Err(Trouble::Absent.into());
    }
    details_by(&crate::program(), &args)
}

/// [`details`] by `program` with `args` — the seam a test drives with a
/// stand-in yt-dlp.
fn details_by(program: &std::path::Path, args: &[String]) -> Result<Details, Failed> {
    let (bytes, warnings) = run::json_telling(program, args, DETAILS_DEADLINE, MOST_BYTES)?;
    if let Some((said, line)) = Said::of_warnings(&warnings) {
        return Err(Failed {
            trouble: Trouble::Failed,
            said,
            line: Some(line),
        });
    }
    Ok(parse_details(&bytes)?)
}

/// One video's fields, read. Pure.
///
/// # Errors
///
/// [`Trouble::Garbled`] for what is not a JSON object with a video id,
/// [`Trouble::NotAList`] never — one video is what this reads.
pub fn parse_details(bytes: &[u8]) -> Result<Details, Trouble> {
    let top: Value = serde_json::from_slice(bytes).map_err(|_| Trouble::Garbled)?;
    let id = text(&top, "id")
        .filter(|id| is_video_id(id))
        .ok_or(Trouble::Garbled)?;
    let formats = top.get("formats").and_then(Value::as_array);
    let pictures = || {
        formats.into_iter().flatten().filter(|f| {
            text(f, "vcodec").is_some_and(|c| c != "none")
                && f.get("height").and_then(Value::as_u64).is_some()
        })
    };
    let size_px = pixels(&top).or_else(|| {
        pictures()
            .max_by_key(|f| f.get("height").and_then(Value::as_u64).unwrap_or(0))
            .and_then(pixels)
    });
    let audio_language = formats
        .into_iter()
        .flatten()
        .filter(|f| text(f, "vcodec").as_deref() == Some("none"))
        .filter_map(|f| {
            let language = text(f, "language")?;
            let preference = f
                .get("language_preference")
                .and_then(Value::as_i64)
                .unwrap_or(-1);
            Some((preference, language))
        })
        .max_by_key(|(preference, _)| *preference)
        .map(|(_, language)| language);
    let chapters = top
        .get("chapters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            Some(Chapter {
                start_s: seconds(c, "start_time")?,
                title: text(c, "title").unwrap_or_default(),
            })
        })
        .collect();
    let captions = top
        .get("subtitles")
        .and_then(Value::as_object)
        .map(|all| {
            all.keys()
                .filter(|k| k.as_str() != "live_chat")
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let short = text(&top, "media_type").map(|m| m == "short").or_else(|| {
        text(&top, "webpage_url")
            .or_else(|| text(&top, "original_url"))
            .filter(|u| u.contains("/shorts/"))
            .map(|_| true)
    });
    Ok(Details {
        title: title_of(&top, &id),
        description: text(&top, "description").unwrap_or_default(),
        duration_s: seconds(&top, "duration"),
        published_at_s: published_of(&top),
        channel_id: text(&top, "channel_id").filter(|c| is_channel_id(c)),
        channel: text(&top, "channel").or_else(|| text(&top, "uploader")),
        size_px,
        language: text(&top, "language"),
        audio_language,
        live_status: text(&top, "live_status"),
        age_limit: top
            .get("age_limit")
            .and_then(Value::as_u64)
            .and_then(|a| u8::try_from(a).ok())
            .unwrap_or(0),
        availability: text(&top, "availability"),
        chapters,
        captions,
        categories: words(&top, "categories"),
        tags: words(&top, "tags"),
        views: top.get("view_count").and_then(Value::as_u64),
        short,
        id,
    })
}

/// A video's own title, or EMPTY when it has none.
///
/// `fulltitle` first: yt-dlp keeps there what the extractor found, BEFORE it
/// dates a live stream's `title` or, finding none, makes one up
/// (`youtube video #<id>`, with a warning). Without `fulltitle` (an answer
/// that does not carry it) the `title`, unless it is that made-up one.
fn title_of(top: &Value, id: &str) -> String {
    if let Some(full) = top.get("fulltitle") {
        return full.as_str().map(str::trim).unwrap_or_default().to_owned();
    }
    let extractor = text(top, "extractor").unwrap_or_else(|| "youtube".to_owned());
    let made_up = format!("{} video #{id}", extractor.replace(':', "-"));
    text(top, "title")
        .filter(|t| *t != made_up)
        .unwrap_or_default()
}

/// A field as trimmed, non-empty text.
fn text(v: &Value, name: &str) -> Option<String> {
    v.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// A list of words, each trimmed and non-empty.
fn words(v: &Value, name: &str) -> Vec<String> {
    v.get(name)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A non-negative number of seconds, rounded down.
fn seconds(v: &Value, name: &str) -> Option<u32> {
    let f = v.get(name).and_then(Value::as_f64)?;
    if !(f.is_finite() && (0.0..4.0e9).contains(&f)) {
        // A negative or absurd value is no duration at all.
        return None;
    }
    // Truncation is the intent: whole seconds, inside `u32` by the test above.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = f as u32;
    Some(whole)
}

/// A picture's width and height, when both are there.
fn pixels(v: &Value) -> Option<(u32, u32)> {
    let w = v.get("width").and_then(Value::as_u64)?;
    let h = v.get("height").and_then(Value::as_u64)?;
    Some((u32::try_from(w).ok()?, u32::try_from(h).ok()?))
}

/// When it was published: the release time of a premiere, the timestamp, or
/// the upload day at midnight UTC.
fn published_of(v: &Value) -> Option<i64> {
    v.get("release_timestamp")
        .and_then(Value::as_i64)
        .or_else(|| v.get("timestamp").and_then(Value::as_i64))
        .or_else(|| text(v, "upload_date").and_then(|d| day_of(&d)))
}

/// `YYYYMMDD` as seconds since the epoch at midnight UTC. Pure.
#[must_use]
pub fn day_of(date: &str) -> Option<i64> {
    if date.len() != 8 || !date.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: i64 = date[..4].parse().ok()?;
    let month: i64 = date[4..6].parse().ok()?;
    let day: i64 = date[6..].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`). Pure.
#[must_use]
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn strings(all: &[&str]) -> Vec<String> {
    all.iter().map(|s| (*s).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_yt_dlp_said_is_sorted() {
        assert_eq!(
            Said::of(Some(
                "ERROR: [youtube] abc: Sign in to confirm you’re not a bot. Use --cookies…"
            )),
            Said::BotCheck
        );
        assert_eq!(
            Said::of(Some(
                "ERROR: [youtube] abc: HTTP Error 429: Too Many Requests"
            )),
            Said::TooManyRequests
        );
        for gone in [
            "ERROR: [youtube] abc: Private video. Sign in if you've been granted access",
            "ERROR: [youtube] abc: Video unavailable. This video has been removed by the uploader",
            "ERROR: [youtube] abc: This video is no longer available because the YouTube account associated with this video has been terminated.",
            "ERROR: [youtube] abc: Join this channel to get access to members-only content",
        ] {
            assert_eq!(Said::of(Some(gone)), Said::Gone, "{gone}");
        }
        assert_eq!(
            Said::of(Some("ERROR: [youtube] abc: Sign in to confirm your age")),
            Said::Other,
            "an age gate is not a bot check"
        );
        assert_eq!(Said::of(None), Said::Other);
        let failed = Failed::from((Trouble::Failed, Some("ERROR: HTTP Error 429".to_owned())));
        assert_eq!(failed.said, Said::TooManyRequests);
        // THE PINNED RATE LIMIT, which says "video unavailable" too: a stop,
        // never a tombstone — as an error line and as a warning.
        let rate_limited = RATE_LIMITED.replacen("WARNING:", "ERROR:", 1);
        assert_eq!(Said::of(Some(&rate_limited)), Said::TooManyRequests);
        assert_eq!(Said::of(Some(RATE_LIMITED)), Said::TooManyRequests);
        for stop in [
            "ERROR: [youtube] abc: Video unavailable. YouTube is requiring a captcha challenge before playback",
            "ERROR: [youtube] abc: All player responses are invalid. Your IP is likely being blocked by Youtube",
        ] {
            assert_eq!(Said::of(Some(stop)), Said::BotCheck, "{stop}");
        }
        assert_eq!(
            Said::of(Some(
                "ERROR: [youtube] abc: This video isn't available anymore"
            )),
            Said::Gone
        );
    }

    /// The pinned yt-dlp's words (`yt_dlp/extractor/youtube/_video.py`, the
    /// no-formats reason, with `common.py`'s cookies hint), as the warning
    /// `--ignore-no-formats-error` makes of them: `raise_no_formats` is called
    /// without the video's id, so none follows the extractor's name.
    const BOT_CHECK: &str = concat!(
        "WARNING: [youtube] Sign in to confirm you\u{2019}re not a bot. ",
        "Use --cookies-from-browser or --cookies for the authentication. ",
        "See  https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp  ",
        "for how to manually pass cookies. Also see  ",
        "https://github.com/yt-dlp/yt-dlp/wiki/Extractors#exporting-youtube-cookies  ",
        "for tips on effectively exporting YouTube cookies"
    );
    const RATE_LIMITED: &str = concat!(
        "WARNING: [youtube] Video unavailable. This content isn't available, ",
        "try again later. The current session has been rate-limited by YouTube for up to an ",
        "hour. It is recommended to use `-t sleep` to add a delay between video requests to ",
        "avoid exceeding the rate limit. For more information, refer to  ",
        "https://github.com/yt-dlp/yt-dlp/wiki/Extractors#this-content-isnt-available-try-again-later"
    );
    const PRIVATE: &str = concat!(
        "WARNING: [youtube] Private video. Sign in if you've been granted access ",
        "to this video. Use --cookies-from-browser or --cookies for the authentication."
    );
    /// MEASURED: what the pinned yt-dlp said, with the details flags, of a
    /// video id YouTube does not have (2026-09-30).
    const UNAVAILABLE: &str = "WARNING: [youtube] This video is unavailable";

    fn lines(all: &[&str]) -> Vec<String> {
        all.iter().map(|l| (*l).to_owned()).collect()
    }

    #[test]
    fn the_warnings_of_a_success_are_sorted_worst_first() {
        assert_eq!(
            Said::of_warnings(&lines(&[PRIVATE, RATE_LIMITED, BOT_CHECK])).map(|(s, _)| s),
            Some(Said::BotCheck)
        );
        assert_eq!(
            Said::of_warnings(&lines(&[PRIVATE, RATE_LIMITED])),
            Some((Said::TooManyRequests, RATE_LIMITED.to_owned()))
        );
        for gone in [PRIVATE, UNAVAILABLE] {
            assert_eq!(
                Said::of_warnings(&lines(&[gone])),
                Some((Said::Gone, gone.to_owned()))
            );
        }
        // GONE ONLY FROM THE VIDEO EXTRACTOR; a stop from any line.
        for elsewhere in [
            "WARNING: [youtube:tab] This video is unavailable",
            "WARNING: [pot:bgutil:script] Private video",
            "WARNING: This video is unavailable",
        ] {
            assert_eq!(Said::of_warnings(&lines(&[elsewhere])), None, "{elsewhere}");
        }
        assert_eq!(
            Said::of_warnings(&lines(&[
                "WARNING: [youtube] Unable to download API page: HTTP Error 429: Too Many Requests"
            ]))
            .map(|(s, _)| s),
            Some(Said::TooManyRequests)
        );
        // What a video that plays (or will) says on the way is nothing to act
        // on.
        assert_eq!(
            Said::of_warnings(&lines(&[
                "WARNING: [youtube] aaaaaaaaaaa: Some web client https formats have been skipped as they are missing a url",
                "WARNING: [youtube] Sign in to confirm your age. This video may be inappropriate for some users.",
                "WARNING: [youtube] Premieres in 5 hours",
                "WARNING: Extractor failed to obtain \"title\". Creating a generic title instead",
                "WARNING: No video formats found!",
                "WARNING: Requested format is not available",
            ])),
            None
        );
    }

    /// `/bin/sh -c script` standing in for yt-dlp, with `arguments` as `$1…`.
    fn stand_in(script: &str, arguments: &[&str]) -> (std::path::PathBuf, Vec<String>) {
        let mut args = vec!["-c".to_owned(), script.to_owned(), "yt-dlp".to_owned()];
        args.extend(arguments.iter().map(|a| (*a).to_owned()));
        (std::path::PathBuf::from("/bin/sh"), args)
    }

    /// What the pinned yt-dlp did with a video it cannot play, under the
    /// details flags (MEASURED, with `$1` in place of why): three warnings,
    /// an answer with a made-up title and no duration, and exit 0.
    const UNPLAYABLE: &str = r##"printf '%s\n' "$1" 'WARNING: No video formats found!' 'WARNING: Requested format is not available' >&2
printf '%s' '{"id": "aaaaaaaaaaa", "title": "youtube video #aaaaaaaaaaa", "fulltitle": "", "duration": null, "formats": [], "extractor": "youtube"}'"##;

    #[test]
    fn a_success_that_says_it_could_not_play_is_no_answer() {
        for (warning, said) in [
            (BOT_CHECK, Said::BotCheck),
            (RATE_LIMITED, Said::TooManyRequests),
            (PRIVATE, Said::Gone),
            (UNAVAILABLE, Said::Gone),
        ] {
            let (program, args) = stand_in(UNPLAYABLE, &[warning]);
            assert_eq!(
                details_by(&program, &args),
                Err(Failed {
                    trouble: Trouble::Failed,
                    said,
                    // As kept: cut at the runner's ceiling.
                    line: Some(warning.chars().take(crate::run::SAID_MOST).collect()),
                }),
                "{warning}"
            );
        }
        // A premiere cannot play YET, and its fields are the answer.
        let (program, args) = stand_in(
            r##"printf '%s\n' 'WARNING: [youtube] Premieres in 5 hours' >&2
printf '%s' '{"id": "aaaaaaaaaaa", "title": "Soon", "fulltitle": "Soon", "live_status": "is_upcoming", "release_timestamp": 1800000000}'"##,
            &[],
        );
        let d = details_by(&program, &args).unwrap();
        assert_eq!(
            (d.title.as_str(), d.live_status.as_deref(), d.published_at_s),
            ("Soon", Some("is_upcoming"), Some(1_800_000_000))
        );
        // A failure is still read from its error line.
        let (program, args) = stand_in(
            r"printf '%s\n' 'ERROR: [youtube] aaaaaaaaaaa: Video unavailable. This video has been removed by the uploader' >&2; exit 1",
            &[],
        );
        assert_eq!(details_by(&program, &args).unwrap_err().said, Said::Gone);
    }

    #[test]
    fn a_channel_is_asked_by_what_was_pasted() {
        assert_eq!(
            channel_address(" @SomeChannel ").unwrap(),
            "https://www.youtube.com/@SomeChannel"
        );
        assert_eq!(
            channel_address("@canal_é").unwrap(),
            "https://www.youtube.com/@canal_%C3%A9"
        );
        assert_eq!(
            channel_address("UCabcdefghijklmnopqrstuv").unwrap(),
            "https://www.youtube.com/channel/UCabcdefghijklmnopqrstuv"
        );
        let page = "https://www.youtube.com/@SomeChannel/videos";
        assert_eq!(channel_address(page).unwrap(), page);
        let video = "https://youtu.be/aaaaaaaaaaa";
        assert_eq!(channel_address(video).unwrap(), video);
        for refused in [
            "https://example.com/@x",
            "https://www.youtube.com@example.com/x",
            "@a b",
            "@x",
            "just words",
            "--exec=rm",
        ] {
            assert_eq!(
                channel_address(refused),
                Err(Trouble::NotAnAddress),
                "{refused}"
            );
        }
        let args = channel_arguments("@SomeChannel").unwrap();
        assert_eq!(args.first().map(String::as_str), Some("--ignore-config"));
        assert_eq!(
            args[args.len() - 2..],
            ["--", "https://www.youtube.com/@SomeChannel"]
        );
    }

    #[test]
    fn a_channel_is_read_from_its_listing() {
        let listing = r#"{
            "_type": "playlist", "id": "UCabcdefghijklmnopqrstuv",
            "title": "Some Channel - Videos", "channel": "Some Channel",
            "channel_id": "UCabcdefghijklmnopqrstuv", "uploader_id": "@SomeChannel",
            "thumbnails": [
                {"url": "https://yt3.googleusercontent.com/banner", "id": "banner_uncropped"},
                {"url": "https://yt3.googleusercontent.com/small", "id": "7", "width": 88},
                {"url": "https://yt3.googleusercontent.com/avatar", "id": "avatar_uncropped", "width": 900}
            ],
            "entries": []
        }"#;
        assert_eq!(
            parse_channel(listing.as_bytes()).unwrap(),
            Channel {
                id: "UCabcdefghijklmnopqrstuv".to_owned(),
                handle: Some("@SomeChannel".to_owned()),
                title: "Some Channel".to_owned(),
                url: "https://www.youtube.com/channel/UCabcdefghijklmnopqrstuv".to_owned(),
                avatar_url: Some("https://yt3.googleusercontent.com/avatar".to_owned()),
            }
        );
        // One video names its channel too.
        let video = r#"{"id": "aaaaaaaaaaa", "channel_id": "UCabcdefghijklmnopqrstuv",
                        "uploader": "Other Name", "uploader_id": "UCnot-a-handle"}"#;
        let channel = parse_channel(video.as_bytes()).unwrap();
        assert_eq!(channel.title, "Other Name");
        assert_eq!(channel.handle, None);
        assert_eq!(
            parse_channel(br#"{"id": "PLsomething"}"#),
            Err(Trouble::Garbled)
        );
        assert_eq!(parse_channel(b"[]"), Err(Trouble::Garbled));
    }

    #[test]
    fn a_tab_is_listed_with_approximate_dates() {
        let args = tab_arguments("UCabcdefghijklmnopqrstuv", Tab::Shorts, 50).unwrap();
        assert!(
            args.windows(2)
                .any(|w| w[0] == "--playlist-end" && w[1] == "50")
        );
        assert!(
            args.windows(2)
                .any(|w| w[0] == "--extractor-args" && w[1] == "youtubetab:approximate_date")
        );
        assert_eq!(
            args.last().unwrap(),
            "https://www.youtube.com/channel/UCabcdefghijklmnopqrstuv/shorts"
        );
        assert_eq!(
            tab_arguments("@handle", Tab::Videos, 5),
            Err(Trouble::NotAnAddress)
        );
        let listing = r#"{"entries": [
            {"id": "aaaaaaaaaaa", "url": "https://www.youtube.com/shorts/aaaaaaaaaaa",
             "title": "A short", "duration": 31.5, "timestamp": 1790000000, "view_count": 12},
            {"id": "bbbbbbbbbbb", "title": "Dated by day", "upload_date": "20260102"},
            {"id": "not-an-id", "title": "dropped"},
            {"id": "ccccccccccc", "title": 5, "duration": -3}
        ]}"#;
        let entries = parse_tab(listing.as_bytes(), 50).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].duration_s, Some(31));
        assert_eq!(entries[0].published_at_s, Some(1_790_000_000));
        assert_eq!(entries[0].views, Some(12));
        assert_eq!(
            entries[1].url,
            "https://www.youtube.com/watch?v=bbbbbbbbbbb"
        );
        assert_eq!(entries[1].published_at_s, Some(1_767_312_000));
        assert_eq!(
            (entries[2].title.as_str(), entries[2].duration_s),
            ("", None)
        );
        assert_eq!(parse_tab(listing.as_bytes(), 1).unwrap().len(), 1);
        assert_eq!(parse_tab(br#"{"id": "x"}"#, 5), Err(Trouble::Garbled));
    }

    #[test]
    fn details_are_read_loosely() {
        let answer = r#"{
            "id": "aaaaaaaaaaa", "title": " A title ", "description": "Line one\nLine two",
            "duration": 212.7, "timestamp": 1790000000, "upload_date": "20260101",
            "channel_id": "UCabcdefghijklmnopqrstuv", "channel": "Some Channel",
            "language": "pt", "live_status": "not_live", "age_limit": 18,
            "availability": "public",
            "chapters": [{"start_time": 0.0, "title": "Start"}, {"start_time": 61.9, "title": "Middle"}, {"title": "no start"}],
            "subtitles": {"en": [], "pt-BR": [], "live_chat": []},
            "automatic_captions": {"fr": []},
            "categories": ["Education"], "tags": ["a", " b ", "", 3],
            "view_count": 4567, "media_type": "video",
            "formats": [
                {"format_id": "140", "vcodec": "none", "acodec": "mp4a", "language": "pt", "language_preference": 10},
                {"format_id": "139", "vcodec": "none", "acodec": "mp4a", "language": "en", "language_preference": -1},
                {"format_id": "137", "vcodec": "avc1", "width": 1920, "height": 1080},
                {"format_id": "136", "vcodec": "avc1", "width": 1280, "height": 720}
            ]
        }"#;
        let d = parse_details(answer.as_bytes()).unwrap();
        assert_eq!(d.id, "aaaaaaaaaaa");
        assert_eq!(d.title, "A title");
        assert_eq!(d.duration_s, Some(212));
        assert_eq!(
            d.published_at_s,
            Some(1_790_000_000),
            "the timestamp wins over the day"
        );
        assert_eq!(d.size_px, Some((1920, 1080)), "the tallest picture");
        assert_eq!(
            d.audio_language.as_deref(),
            Some("pt"),
            "the preferred track"
        );
        assert_eq!(d.age_limit, 18);
        assert_eq!(
            d.chapters,
            [
                Chapter {
                    start_s: 0,
                    title: "Start".to_owned()
                },
                Chapter {
                    start_s: 61,
                    title: "Middle".to_owned()
                }
            ]
        );
        assert_eq!(
            d.captions,
            ["en", "pt-BR"],
            "not the live chat, not the automatic ones"
        );
        assert_eq!(d.tags, ["a", "b"]);
        assert_eq!(d.views, Some(4567));
        assert_eq!(d.short, Some(false));

        // A premiere is dated by its release; a Short says so by its address.
        let short = r#"{"id": "bbbbbbbbbbb", "release_timestamp": 1800000000, "timestamp": 1,
                        "webpage_url": "https://www.youtube.com/shorts/bbbbbbbbbbb",
                        "width": 1080, "height": 1920}"#;
        let d = parse_details(short.as_bytes()).unwrap();
        assert_eq!(d.published_at_s, Some(1_800_000_000));
        assert_eq!(d.short, Some(true));
        assert_eq!(d.size_px, Some((1080, 1920)));
        assert_eq!(d.duration_s, None);
        assert_eq!(
            parse_details(br#"{"title": "no id"}"#),
            Err(Trouble::Garbled)
        );
        assert_eq!(parse_details(b"<html>"), Err(Trouble::Garbled));

        // A TITLE yt-dlp MADE UP is none; a live stream's is the one it had
        // before yt-dlp dated it.
        let made_up =
            r#"{"id": "ccccccccccc", "title": "youtube video #ccccccccccc", "fulltitle": null}"#;
        assert_eq!(parse_details(made_up.as_bytes()).unwrap().title, "");
        let without_full = r#"{"id": "ccccccccccc", "title": "youtube video #ccccccccccc", "extractor": "youtube"}"#;
        assert_eq!(parse_details(without_full.as_bytes()).unwrap().title, "");
        let live =
            r#"{"id": "ccccccccccc", "title": "Radio 2026-09-30 10:00", "fulltitle": "Radio"}"#;
        assert_eq!(parse_details(live.as_bytes()).unwrap().title, "Radio");
    }

    #[test]
    fn details_ask_the_fields_and_not_the_streams() {
        let pot = ["--js-runtimes".to_owned(), "deno:/d".to_owned()];
        let args =
            details_arguments_with("https://www.youtube.com/watch?v=aaaaaaaaaaa", &pot).unwrap();
        for flag in [
            "--skip-download",
            "--dump-json",
            "--no-playlist",
            "--ignore-no-formats-error",
        ] {
            assert!(args.iter().any(|a| a == flag), "{flag}");
        }
        // WITH THAT FLAG why a video cannot play is a warning: silencing the
        // warnings would turn a bot check, a 429 and a private video into
        // answers.
        assert!(!args.iter().any(|a| a == "--no-warnings"));
        assert!(
            args.windows(2).any(|w| w == pot),
            "the PO token flags go along"
        );
        assert_eq!(
            args[args.len() - 2..],
            ["--", "https://www.youtube.com/watch?v=aaaaaaaaaaa"]
        );
        assert_eq!(details_arguments("-o /tmp/x"), Err(Trouble::NotAnAddress));
    }

    #[test]
    fn a_day_is_its_midnight() {
        assert_eq!(day_of("19700101"), Some(0));
        assert_eq!(day_of("20000301"), Some(951_868_800));
        assert_eq!(day_of("20261301"), None);
        assert_eq!(day_of("2026-01-01"), None);
        assert_eq!(days_from_civil(2026, 9, 30), 20_726);
    }
}
