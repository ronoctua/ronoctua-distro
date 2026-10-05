//! What yt-dlp LISTS without downloading anything: the entries of a playlist,
//! and a search for what is live right now.
//!
//! Both are `--flat-playlist --dump-single-json`: one JSON document with an
//! `entries` array, each entry a few fields and no stream address. The run is
//! [`crate::run::json`] (a deadline, a ceiling, kill on drop); the reading is
//! pure and takes bytes, so every rule here is tested on hand-written JSON.
//!
//! # Read loosely, one field at a time
//!
//! The document is read as a `serde_json::Value` and each field is taken out
//! by hand. A struct would fail the WHOLE listing on one entry whose `title`
//! came back as a number; here that entry loses its title and the other
//! nine hundred stay.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::address::{escape, is_address, listing_address};
use crate::{Trouble, run};

/// How long one listing may take, whole.
///
/// Twenty-five seconds: a search is one page, and a list of a thousand is ten
/// pages of a hundred; past this the person has stopped waiting, and the page
/// says so instead of spinning.
pub const DEADLINE: Duration = Duration::from_secs(25);

/// How long yt-dlp waits on one socket before giving up on it.
const SOCKET_SECONDS: u32 = 10;

/// The most bytes of JSON a listing may answer. An entry of a flat list is
/// about a kilobyte and a half with its thumbnails.
pub const MOST_BYTES: usize = 16 << 20;

/// How many entries of a playlist are listed, unless the caller says fewer.
///
/// A thousand: the queue holds ten thousand leaves in all, and one list that
/// took them would leave nothing for the rest.
pub const FLAT_MOST: usize = 1000;

/// The most lives one search asks for.
pub const LIVES_MOST: usize = 50;

/// The longest search, in characters; what is past it is left out.
pub const WORDS_MOST: usize = 200;

/// The Live filter of YouTube's search (`sp=EgJAAQ==`), escaped once, as
/// the results page writes it.
pub const LIVE_FILTER: &str = "EgJAAQ%3D%3D";

/// A playlist, listed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flat {
    /// What the list is called; EMPTY when it said nothing, and the caller
    /// names it by its address then.
    pub title: String,
    /// Its entries, in order, at most the number asked.
    pub entries: Vec<FlatEntry>,
}

/// One entry of a listed playlist.
///
/// `Default`, so an entry written by hand names only what it says
/// (`..FlatEntry::default()`) and a field added later breaks none of them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FlatEntry {
    /// The address that plays it.
    pub url: String,
    /// Its title; EMPTY is allowed (measured: some entries have none).
    pub title: String,
    /// How long it is, when the list said.
    pub duration_seconds: Option<f64>,
    /// The address of its picture, when the list gave one: a poster for the
    /// player's catalogue. The widest no wider than [`THUMBNAIL_MOST_WIDTH`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    /// Who published it (the channel, else the uploader), when said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

/// The widest picture of an entry a listing keeps, in pixels: a poster in a
/// grid is a few hundred wide, and YouTube lists the same frame at up to
/// 1280 — the size a poster is fetched at is the size it costs.
pub const THUMBNAIL_MOST_WIDTH: u64 = 640;

/// A broadcast that is live now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Live {
    /// The video id.
    pub id: String,
    /// The address that plays it.
    pub url: String,
    /// Its title; EMPTY is allowed.
    pub title: String,
    /// Who broadcasts it, when said.
    pub channel: Option<String>,
    /// How many are watching, when said.
    pub watching: Option<u64>,
}

/// The arguments that list `url` flat, at most `most` entries.
///
/// # Errors
///
/// [`Trouble::NotAnAddress`] for anything but a plain web address.
pub fn flat_arguments(url: &str, most: usize) -> Result<Vec<String>, Trouble> {
    if !is_address(url) {
        return Err(Trouble::NotAnAddress);
    }
    Ok(listing_arguments(
        &listing_address(url),
        most.clamp(1, FLAT_MOST),
    ))
}

/// The arguments of a search for lives.
///
/// # Errors
///
/// [`Trouble::NothingToAsk`] when there are no words.
pub fn lives_arguments(words: &str, most: usize) -> Result<Vec<String>, Trouble> {
    Ok(listing_arguments(
        &lives_address(words)?,
        most.clamp(1, LIVES_MOST),
    ))
}

/// The host the lives are asked of — the one a consent row names for them
/// (`ronoctua_player::find::Directory::host`). Named here, beside the
/// address that asks it: this crate is the one that talks to YouTube.
pub const LIVES_HOST: &str = "youtube.com";

/// The results page that searches `words` for what is live. Pure.
///
/// # Errors
///
/// [`Trouble::NothingToAsk`] when there are no words.
pub fn lives_address(words: &str) -> Result<String, Trouble> {
    let words: String = words.trim().chars().take(WORDS_MOST).collect();
    let words = words.trim();
    if words.is_empty() {
        return Err(Trouble::NothingToAsk);
    }
    Ok(format!(
        "https://www.youtube.com/results?search_query={}&sp={LIVE_FILTER}",
        escape(words)
    ))
}

fn listing_arguments(url: &str, most: usize) -> Vec<String> {
    [
        // NOT THE PERSON'S CONFIG: a `--format` or an `--output` somebody put
        // in `~/.config/yt-dlp` for their own downloads changes what a
        // listing answers.
        "--ignore-config",
        "--no-warnings",
        "--flat-playlist",
        // A mix's watch address is kept (see `listing_address`), and without
        // this yt-dlp answers its one video.
        "--yes-playlist",
        "--dump-single-json",
        "--playlist-end",
        &most.to_string(),
        "--socket-timeout",
        &SOCKET_SECONDS.to_string(),
        // THE ADDRESS AFTER `--`, so nothing in it is ever read as a flag.
        "--",
        url,
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// Lists the playlist at `url`: its title and at most [`FLAT_MOST`] entries.
///
/// A YouTube watch page with a list is asked as the list
/// ([`listing_address`]). Blocks for up to [`DEADLINE`]: call it on a thread.
///
/// # Errors
///
/// [`Trouble::Absent`] where this installation has no yt-dlp, and every
/// trouble of a run and of the reading ([`parse_flat`]).
pub fn flat(url: &str) -> Result<Flat, Trouble> {
    flat_up_to(url, FLAT_MOST)
}

/// [`flat`], at most `most` entries.
///
/// # Errors
///
/// As [`flat`].
pub fn flat_up_to(url: &str, most: usize) -> Result<Flat, Trouble> {
    let args = flat_arguments(url, most)?;
    if !crate::installed() {
        return Err(Trouble::Absent);
    }
    let bytes = run::json(&crate::program(), &args, DEADLINE, MOST_BYTES)?;
    parse_flat(&bytes, most)
}

/// Searches YouTube for `words` with the Live filter, and keeps only what is
/// live now. Blocks for up to [`DEADLINE`]: call it on a thread.
///
/// # Errors
///
/// [`Trouble::NothingToAsk`], [`Trouble::Absent`], and every trouble of a
/// run and of the reading ([`parse_lives`]).
pub fn search_lives(words: &str, most: usize) -> Result<Vec<Live>, Trouble> {
    let args = lives_arguments(words, most)?;
    if !crate::installed() {
        return Err(Trouble::Absent);
    }
    let bytes = run::json(&crate::program(), &args, DEADLINE, MOST_BYTES)?;
    parse_lives(&bytes, most.clamp(1, LIVES_MOST))
}

/// The document, which must be an object with an `entries` array.
pub(crate) fn entries_of(bytes: &[u8]) -> Result<(Value, Vec<Value>), Trouble> {
    let mut top: Value = serde_json::from_slice(bytes).map_err(|_| Trouble::Garbled)?;
    if !top.is_object() {
        return Err(Trouble::Garbled);
    }
    match top.get_mut("entries").map(Value::take) {
        Some(Value::Array(entries)) => Ok((top, entries)),
        // ONE VIDEO, and not a list: the address was not what the caller
        // took it for, and saying so beats a list of one.
        None | Some(Value::Null) => Err(Trouble::NotAList),
        Some(_) => Err(Trouble::Garbled),
    }
}

pub(crate) fn text(v: &Value, name: &str) -> Option<String> {
    v.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// A YouTube video id: eleven of letters, digits, `-` and `_`.
pub(crate) fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Where an entry plays: its own address, or — for a YouTube entry that
/// gave none — the watch page of its id.
pub(crate) fn address_of(entry: &Value) -> Option<String> {
    if let Some(url) = text(entry, "url").filter(|u| is_address(u)) {
        return Some(url);
    }
    let youtube = text(entry, "ie_key").is_some_and(|k| k.eq_ignore_ascii_case("youtube"));
    let id = text(entry, "id").filter(|id| is_video_id(id))?;
    youtube.then(|| format!("https://www.youtube.com/watch?v={id}"))
}

/// An entry's picture: its `thumbnail`, else the widest of its `thumbnails`
/// no wider than [`THUMBNAIL_MOST_WIDTH`] (the narrowest, when every one is
/// wider). Only a web address is a picture.
pub(crate) fn thumbnail_of(entry: &Value) -> Option<String> {
    if let Some(one) = text(entry, "thumbnail").filter(|u| is_address(u)) {
        return Some(one);
    }
    let all: Vec<(u64, String)> = entry
        .get("thumbnails")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|t| {
            let url = text(t, "url").filter(|u| is_address(u))?;
            Some((t.get("width").and_then(Value::as_u64).unwrap_or(0), url))
        })
        .collect();
    let fits = all
        .iter()
        .filter(|(w, _)| *w <= THUMBNAIL_MOST_WIDTH)
        .max_by_key(|(w, _)| *w);
    fits.or_else(|| all.iter().min_by_key(|(w, _)| *w))
        .map(|(_, url)| url.clone())
}

/// The titles YouTube gives an entry that cannot be played by anybody.
const GONE: [&str; 2] = ["[Private video]", "[Deleted video]"];

/// A flat listing, read. Pure.
///
/// Entries that cannot play — no address, or one YouTube marks private or
/// deleted — are left out; an empty title is kept as empty.
///
/// # Errors
///
/// [`Trouble::Garbled`] for what is not a JSON object, [`Trouble::NotAList`]
/// for one video instead of a list.
pub fn parse_flat(bytes: &[u8], most: usize) -> Result<Flat, Trouble> {
    let (top, entries) = entries_of(bytes)?;
    let entries = entries
        .iter()
        .filter(|e| e.is_object())
        .filter_map(|e| {
            let title = text(e, "title").unwrap_or_default();
            if GONE.contains(&title.as_str()) {
                return None;
            }
            Some(FlatEntry {
                url: address_of(e)?,
                title,
                duration_seconds: e
                    .get("duration")
                    .and_then(Value::as_f64)
                    .filter(|d| d.is_finite() && *d >= 0.0),
                thumbnail: thumbnail_of(e),
                channel: text(e, "channel").or_else(|| text(e, "uploader")),
            })
        })
        .take(most)
        .collect();
    Ok(Flat {
        title: text(&top, "title").unwrap_or_default(),
        entries,
    })
}

/// A search's results, keeping only what is live now. Pure.
///
/// `live_status` comes from the LIVE badge on the results page; an entry that
/// does not say `is_live` — a recording, a premiere, something upcoming — is
/// not a live, whatever the filter asked.
///
/// # Errors
///
/// As [`parse_flat`].
pub fn parse_lives(bytes: &[u8], most: usize) -> Result<Vec<Live>, Trouble> {
    let (_, entries) = entries_of(bytes)?;
    Ok(entries
        .iter()
        .filter(|e| text(e, "live_status").as_deref() == Some("is_live"))
        .filter_map(|e| {
            let id = text(e, "id").filter(|id| is_video_id(id))?;
            Some(Live {
                url: address_of(e)?,
                title: text(e, "title").unwrap_or_default(),
                channel: text(e, "channel").or_else(|| text(e, "uploader")),
                watching: e
                    .get("concurrent_view_count")
                    .and_then(Value::as_u64)
                    .or_else(|| e.get("view_count").and_then(Value::as_u64)),
                id,
            })
        })
        .take(most)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = r#"{
        "_type": "playlist", "id": "PLq", "title": "  Evening radio  ",
        "entries": [
            {"_type": "url", "ie_key": "Youtube", "id": "aaaaaaaaaaa",
             "url": "https://www.youtube.com/watch?v=aaaaaaaaaaa",
             "title": "First", "duration": 181.0},
            {"_type": "url", "ie_key": "Youtube", "id": "bbbbbbbbbbb",
             "title": "", "duration": null},
            {"_type": "url", "ie_key": "Youtube", "id": "ccccccccccc",
             "url": "https://www.youtube.com/watch?v=ccccccccccc",
             "title": "[Private video]"},
            null,
            {"_type": "url", "ie_key": "Generic", "id": "x", "title": "no address"},
            {"_type": "url", "ie_key": "Youtube", "id": "ddddddddddd",
             "url": "https://www.youtube.com/watch?v=ddddddddddd",
             "title": 7, "duration": "long"}
        ]
    }"#;

    #[test]
    fn a_list_keeps_its_title_and_every_entry_that_plays() {
        let flat = parse_flat(LIST.as_bytes(), FLAT_MOST).unwrap();
        assert_eq!(flat.title, "Evening radio");
        let urls: Vec<&str> = flat.entries.iter().map(|e| e.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://www.youtube.com/watch?v=aaaaaaaaaaa",
                "https://www.youtube.com/watch?v=bbbbbbbbbbb",
                "https://www.youtube.com/watch?v=ddddddddddd",
            ]
        );
        assert_eq!(flat.entries[0].duration_seconds, Some(181.0));
        // AN EMPTY TITLE IS ALLOWED, and so is one that came back as a number.
        assert_eq!(flat.entries[1].title, "");
        assert_eq!(flat.entries[2].title, "");
        assert_eq!(flat.entries[2].duration_seconds, None);
    }

    /// FAILS IF: an entry's picture or its channel is lost, or a picture too
    /// wide for a poster is the one kept.
    #[test]
    fn an_entry_keeps_a_poster_sized_picture_and_its_channel() {
        let list = r#"{"entries": [
            {"ie_key": "Youtube", "id": "aaaaaaaaaaa", "title": "One", "channel": "Somebody",
             "uploader": "Somebody else",
             "thumbnails": [
                {"url": "https://i.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg?sqp=a", "width": 168, "height": 94},
                {"url": "https://i.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg?sqp=b", "width": 336, "height": 188},
                {"url": "https://i.ytimg.com/vi/aaaaaaaaaaa/maxresdefault.jpg", "width": 1280, "height": 720},
                {"url": "not an address", "width": 480}
             ]},
            {"ie_key": "Youtube", "id": "bbbbbbbbbbb", "title": "Two", "uploader": "Uploader",
             "thumbnail": "https://i.ytimg.com/vi/bbbbbbbbbbb/mqdefault.jpg"},
            {"ie_key": "Youtube", "id": "ccccccccccc", "title": "Three",
             "thumbnails": [{"url": "https://i.ytimg.com/vi/ccccccccccc/maxres.jpg", "width": 1920}]},
            {"ie_key": "Youtube", "id": "ddddddddddd", "title": "Four"}
        ]}"#;
        let flat = parse_flat(list.as_bytes(), FLAT_MOST).unwrap();
        let e = &flat.entries;
        assert_eq!(
            e[0].thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg?sqp=b"),
            "the widest that fits a poster"
        );
        assert_eq!(
            e[0].channel.as_deref(),
            Some("Somebody"),
            "the channel first"
        );
        assert_eq!(
            e[1].thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/bbbbbbbbbbb/mqdefault.jpg")
        );
        assert_eq!(
            e[1].channel.as_deref(),
            Some("Uploader"),
            "else the uploader"
        );
        assert_eq!(
            e[2].thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/ccccccccccc/maxres.jpg"),
            "only wide ones: the narrowest"
        );
        assert_eq!(
            (e[3].thumbnail.as_deref(), e[3].channel.as_deref()),
            (None, None)
        );
    }

    #[test]
    fn a_list_is_cut_at_the_number_asked() {
        assert_eq!(parse_flat(LIST.as_bytes(), 1).unwrap().entries.len(), 1);
    }

    #[test]
    fn one_video_is_not_a_list_and_garbage_is_not_json() {
        let one = r#"{"_type": "video", "id": "aaaaaaaaaaa", "title": "x"}"#;
        assert_eq!(parse_flat(one.as_bytes(), 9), Err(Trouble::NotAList));
        assert_eq!(parse_flat(b"[]", 9), Err(Trouble::Garbled));
        assert_eq!(parse_flat(b"<html>", 9), Err(Trouble::Garbled));
        assert_eq!(parse_flat(br#"{"entries": 3}"#, 9), Err(Trouble::Garbled));
        let empty = parse_flat(br#"{"entries": []}"#, 9).unwrap();
        assert!(empty.entries.is_empty() && empty.title.is_empty());
    }

    #[test]
    fn a_live_search_keeps_only_lives() {
        let found = r#"{"_type": "playlist", "entries": [
            {"id": "live0000001", "url": "https://www.youtube.com/watch?v=live0000001",
             "title": "Lofi around the clock", "channel": "Somebody",
             "live_status": "is_live", "concurrent_view_count": 1234},
            {"id": "record00001", "url": "https://www.youtube.com/watch?v=record00001",
             "title": "Yesterday's stream", "live_status": "was_live"},
            {"id": "later000001", "title": "Soon", "live_status": "is_upcoming"},
            {"id": "plain000001", "title": "A video"},
            {"id": "live0000002", "ie_key": "Youtube", "title": "",
             "uploader": "Other", "live_status": "is_live", "view_count": 7},
            {"id": "not-an-id", "url": "https://www.youtube.com/watch?v=x",
             "live_status": "is_live"}
        ]}"#;
        let lives = parse_lives(found.as_bytes(), LIVES_MOST).unwrap();
        assert_eq!(
            lives,
            [
                Live {
                    id: "live0000001".to_owned(),
                    url: "https://www.youtube.com/watch?v=live0000001".to_owned(),
                    title: "Lofi around the clock".to_owned(),
                    channel: Some("Somebody".to_owned()),
                    watching: Some(1234),
                },
                Live {
                    id: "live0000002".to_owned(),
                    url: "https://www.youtube.com/watch?v=live0000002".to_owned(),
                    title: String::new(),
                    channel: Some("Other".to_owned()),
                    watching: Some(7),
                },
            ]
        );
        assert_eq!(parse_lives(found.as_bytes(), 1).unwrap().len(), 1);
    }

    #[test]
    fn a_live_search_asks_the_results_page_with_the_live_filter() {
        assert_eq!(
            lives_address("  news 24/7 ").unwrap(),
            "https://www.youtube.com/results?search_query=news%2024%2F7&sp=EgJAAQ%3D%3D"
        );
        assert_eq!(lives_address("   "), Err(Trouble::NothingToAsk));
        let long = "a".repeat(WORDS_MOST * 2);
        assert!(
            lives_address(&long)
                .unwrap()
                .contains(&"a".repeat(WORDS_MOST))
        );
        assert!(
            !lives_address(&long)
                .unwrap()
                .contains(&"a".repeat(WORDS_MOST + 1))
        );
    }

    #[test]
    fn the_arguments_ignore_the_persons_config_and_end_with_the_address() {
        let args = flat_arguments("https://www.youtube.com/watch?v=abc&list=PLq", 5000).unwrap();
        assert_eq!(args.first().map(String::as_str), Some("--ignore-config"));
        assert!(
            args.windows(2)
                .any(|w| w[0] == "--playlist-end" && w[1] == "1000")
        );
        assert!(args.contains(&"--flat-playlist".to_owned()));
        assert!(args.contains(&"--dump-single-json".to_owned()));
        assert_eq!(
            args[args.len() - 2..],
            ["--", "https://www.youtube.com/playlist?list=PLq"]
        );
        assert_eq!(flat_arguments("-o /tmp/x", 5), Err(Trouble::NotAnAddress));
        let lives = lives_arguments("jazz", 500).unwrap();
        assert!(
            lives
                .windows(2)
                .any(|w| w[0] == "--playlist-end" && w[1] == "50")
        );
        assert!(
            lives
                .last()
                .is_some_and(|u| u.ends_with("&sp=EgJAAQ%3D%3D"))
        );
    }
}
