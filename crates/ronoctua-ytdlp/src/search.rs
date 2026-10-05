//! A SEARCH OF YOUTUBE, for fast-access's `/y`: `ytsearchN:<words>` listed
//! flat (`--flat-playlist --dump-single-json`), nothing downloaded.
//!
//! # Asked with the person's pass, stopped with the next key
//!
//! yt-dlp asks [`SEARCH_HOST`], so a search takes a
//! [`ronoctua_consent::Pass`] for it — the proof the person let that host
//! (`ronoctua-consent`). And it takes a [`Stop`]: fast-access hands each
//! keystroke's search one and stops it at the next key, which kills the child
//! within [`crate::run::STOP_LOOK`] instead of letting it talk to YouTube for
//! the rest of its deadline behind a search box that moved on.
//!
//! # Shorts
//!
//! A result is a Short when its address is a `/shorts/` one, or when it is
//! known to last a minute or less ([`Found::short`]). Whether they are shown
//! is the door's switch, filtered by default ([`Shorts`]); the search asks
//! for more than it shows so a page of Shorts does not come back empty.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::listing::{address_of, entries_of, is_video_id, text, thumbnail_of};
use crate::{Trouble, run};

/// The host yt-dlp asks for a search — what the consent row names and the
/// pass must be for. YouTube's own pages, the host the video catalogue's
/// scans ask too.
pub const SEARCH_HOST: &str = "www.youtube.com";

/// How long one search may take, whole: one page of results.
pub const SEARCH_DEADLINE: Duration = Duration::from_secs(20);

/// The most results one search asks yt-dlp for.
pub const SEARCH_MOST: usize = 60;

/// The longest a Short lasts, by its duration alone, in seconds — a result
/// whose address does not say so but lasts this or less.
pub const SHORT_MOST_SECONDS: f64 = 60.0;

/// The longest search, in grapheme clusters (what a person reads as
/// characters); what is past it is left out.
pub const SEARCH_WORDS_MOST: usize = 200;

/// A search's stop: shared with the thread that searches, set by whoever no
/// longer wants the answer.
#[derive(Debug, Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    /// A stop nobody pulled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stops every search holding this handle.
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether [`Stop::stop`] was called.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Shorts in a search's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Shorts {
    /// Left out — the default.
    #[default]
    Without,
    /// Kept.
    With,
}

/// One result of a search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Found {
    /// The video's id.
    pub id: String,
    /// The address that plays it.
    pub url: String,
    /// Its title; EMPTY is allowed.
    pub title: String,
    /// Who published it, when said.
    pub channel: Option<String>,
    /// How long it is, in seconds, when said.
    pub duration_s: Option<f64>,
    /// Its picture's address (on `i.ytimg.com`), when given.
    pub thumbnail: Option<String>,
    /// How many have watched it, when said.
    pub views: Option<u64>,
}

impl Found {
    /// Whether it is a Short: a `/shorts/` address, or a minute or less.
    #[must_use]
    pub fn short(&self) -> bool {
        self.url.contains("/shorts/")
            || self
                .duration_s
                .is_some_and(|d| d > 0.0 && d <= SHORT_MOST_SECONDS)
    }
}

/// The arguments of a search for `words`, at most `most` results.
///
/// # Errors
///
/// [`Trouble::NothingToAsk`] when there are no words.
pub fn search_arguments(words: &str, most: usize) -> Result<Vec<String>, Trouble> {
    // CUT BETWEEN CLUSTERS, never inside one (`ink-is-not-a-count`).
    let words: String = ronoctua_protocol::cells_of(words.trim())
        .take(SEARCH_WORDS_MOST)
        .collect();
    let words = words.trim();
    if words.is_empty() {
        return Err(Trouble::NothingToAsk);
    }
    let most = most.clamp(1, SEARCH_MOST);
    Ok([
        // NOT THE PERSON'S CONFIG, as every listing: a `--format` somebody
        // keeps for their downloads changes what a search answers.
        "--ignore-config".to_owned(),
        "--no-warnings".to_owned(),
        "--flat-playlist".to_owned(),
        "--dump-single-json".to_owned(),
        "--socket-timeout".to_owned(),
        "10".to_owned(),
        // THE SEARCH AFTER `--`: words that begin with `-` are words.
        "--".to_owned(),
        format!("ytsearch{most}:{words}"),
    ]
    .into())
}

/// A search's answer, read: every entry with a video id and an address,
/// Shorts left out unless `shorts` keeps them, at most `most`. Pure.
///
/// # Errors
///
/// [`Trouble::Garbled`] for what is not yt-dlp's JSON.
pub fn parse_search(bytes: &[u8], most: usize, shorts: Shorts) -> Result<Vec<Found>, Trouble> {
    let (_, entries) = entries_of(bytes).map_err(|t| match t {
        // A SEARCH ALWAYS ANSWERS A LIST; one video is not what was asked.
        Trouble::NotAList => Trouble::Garbled,
        t => t,
    })?;
    Ok(entries
        .iter()
        .filter(|e| e.is_object())
        .filter_map(|e| {
            let id = text(e, "id").filter(|id| is_video_id(id))?;
            Some(Found {
                url: address_of(e)?,
                title: text(e, "title").unwrap_or_default(),
                channel: text(e, "channel").or_else(|| text(e, "uploader")),
                duration_s: e
                    .get("duration")
                    .and_then(Value::as_f64)
                    .filter(|d| d.is_finite() && *d >= 0.0),
                thumbnail: thumbnail_of(e),
                views: e.get("view_count").and_then(Value::as_u64),
                id,
            })
        })
        .filter(|f| shorts == Shorts::With || !f.short())
        .take(most)
        .collect())
}

/// Searches YouTube for `words`: at most `most` results, Shorts as `shorts`
/// says. Blocks for up to [`SEARCH_DEADLINE`]: call it on a thread.
///
/// `None` when `stop` was pulled before it answered — nobody waits for the
/// answer, and none is made.
///
/// # Errors
///
/// In the `Some`: [`Trouble::NothingToAsk`], [`Trouble::Absent`] where this
/// installation has no yt-dlp, [`Trouble::Failed`] when the pass is for
/// another host (nothing is started), and every trouble of a run and of the
/// reading.
#[must_use]
pub fn search(
    words: &str,
    most: usize,
    shorts: Shorts,
    pass: &ronoctua_consent::Pass,
    stop: &Stop,
) -> Option<Result<Vec<Found>, Trouble>> {
    if !pass.covers(SEARCH_HOST) {
        return Some(Err(Trouble::Failed));
    }
    // ASKED FOR MORE when the Shorts are left out: a page of them would
    // otherwise come back with nothing to show.
    let asked = match shorts {
        Shorts::With => most,
        Shorts::Without => most.saturating_mul(2),
    };
    let args = match search_arguments(words, asked) {
        Ok(a) => a,
        Err(t) => return Some(Err(t)),
    };
    if !crate::installed() {
        return Some(Err(Trouble::Absent));
    }
    let bytes = run::json_until(
        &crate::program(),
        &args,
        SEARCH_DEADLINE,
        crate::MOST_BYTES,
        &stop.0,
    )?;
    Some(bytes.and_then(|b| parse_search(&b, most, shorts)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER: &str = r#"{"_type":"playlist","entries":[
        {"ie_key":"Youtube","id":"aaaaaaaaaaa","url":"https://www.youtube.com/watch?v=aaaaaaaaaaa",
         "title":"A long one","channel":"Chan","duration":600.0,"view_count":1234,
         "thumbnails":[{"url":"https://i.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg","width":480}]},
        {"ie_key":"Youtube","id":"bbbbbbbbbbb","url":"https://www.youtube.com/shorts/bbbbbbbbbbb",
         "title":"A Short","duration":null},
        {"ie_key":"Youtube","id":"ccccccccccc","url":"https://www.youtube.com/watch?v=ccccccccccc",
         "title":"Under a minute","duration":45.0},
        {"ie_key":"Youtube","id":"short","title":"not an id"},
        null
    ]}"#;

    /// FAILS IF: Shorts stop being left out by default (by address or by
    /// duration), or kept when the switch keeps them.
    #[test]
    fn shorts_are_left_out_by_default() {
        let found = parse_search(ANSWER.as_bytes(), 10, Shorts::Without).unwrap();
        let ids: Vec<&str> = found.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["aaaaaaaaaaa"]);
        assert_eq!(found[0].channel.as_deref(), Some("Chan"));
        assert_eq!(found[0].views, Some(1234));
        assert_eq!(
            found[0].thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg")
        );
        let all = parse_search(ANSWER.as_bytes(), 10, Shorts::With).unwrap();
        assert_eq!(all.len(), 3);
        assert!(all[1].short() && all[2].short() && !all[0].short());
        assert_eq!(
            parse_search(ANSWER.as_bytes(), 1, Shorts::With)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            parse_search(b"{\"id\":\"x\"}", 5, Shorts::With),
            Err(Trouble::Garbled)
        );
    }

    #[test]
    fn the_search_is_one_ytsearch_after_the_double_dash() {
        let args = search_arguments("  -rust lang ", 500).unwrap();
        assert_eq!(args.last().unwrap(), "ytsearch60:-rust lang");
        assert_eq!(args[args.len() - 2], "--");
        assert!(args.contains(&"--flat-playlist".to_owned()));
        assert_eq!(search_arguments("   ", 5), Err(Trouble::NothingToAsk));
    }

    /// FAILS IF: a search starts anything under a pass for another host, or
    /// a stopped search answers.
    #[test]
    fn a_search_asks_only_the_host_it_was_let_and_a_stop_answers_nothing() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-ytdlp-search-{}", std::process::id()));
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        ronoctua_consent::let_hosts_in(&store, &["other.example", SEARCH_HOST], true).unwrap();
        let other = ronoctua_consent::pass_in(&store, "other.example").unwrap();
        let ours = ronoctua_consent::pass_in(&store, SEARCH_HOST).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        let go = Stop::new();
        assert_eq!(
            search("rust", 5, Shorts::Without, &other, &go),
            Some(Err(Trouble::Failed))
        );
        let pulled = Stop::new();
        pulled.stop();
        // STOPPED BEFORE IT RAN: whatever this machine has, nothing answers —
        // unless there is no yt-dlp at all, which is said first.
        let said = search("rust", 5, Shorts::Without, &ours, &pulled);
        assert!(
            said.is_none() || said == Some(Err(Trouble::Absent)),
            "{said:?}"
        );
    }

    #[test]
    fn a_stop_is_shared_by_its_clones() {
        let stop = Stop::new();
        let other = stop.clone();
        assert!(!other.is_stopped());
        stop.stop();
        assert!(other.is_stopped());
    }
}
