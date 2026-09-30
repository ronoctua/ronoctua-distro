//! Lyrics from `lrclib.net` — the source after a `.lrc` beside the file and
//! the lyrics the file carries itself.
//!
//! Two questions. `GET /api/get` with the artist, the title, the album and the
//! length: the service matches on all four, and the length is what tells a
//! live version from the studio one (the player asks it on its own, once per
//! song, when the person allowed it). `GET /api/search`, by title and artist
//! or by any words, which answers up to twenty records for the person to
//! choose from. It answers synced lyrics (LRC) when it has them and plain text
//! when it does not; synced wins.
//!
//! LRCLIB has NO language field and no translations: a record is the words
//! somebody uploaded, in whatever language they are sung.
//!
//! Moved here from `ronoctua-player`: the answer is the RECORD, and turning
//! its text into lines is the player's `lyrics::parse`, which this crate does
//! not link.

use serde::{Deserialize, Serialize};

use crate::ask::{Net, Request, answered, json, object, present};
use crate::query::escape;
use crate::{Host, Refusal, Service, Why};

/// The most records a search answers (the service's own cap).
pub const SEARCH_MOST: usize = 20;

/// A search's answer can carry twenty songs' lyrics: 232 KB was measured.
pub const SEARCH_MOST_BYTES: u64 = 2 << 20;

/// The request's path, every value escaped.
#[must_use]
pub fn path(artist: &str, title: &str, album: Option<&str>, duration: Option<f64>) -> String {
    let mut out = format!(
        "/api/get?artist_name={}&track_name={}",
        escape(artist),
        escape(title)
    );
    if let Some(a) = album.filter(|a| !a.is_empty()) {
        out.push_str(&format!("&album_name={}", escape(a)));
    }
    if let Some(d) = duration {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let s = d.round().max(0.0) as u64;
        out.push_str(&format!("&duration={s}"));
    }
    out
}

/// What a search asks with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "by")]
pub enum Search {
    /// Any words: the service looks in titles, artists and albums.
    Words {
        /// The words.
        words: String,
    },
    /// A title, and maybe its artist and album.
    Track {
        /// The title.
        track: String,
        /// The artist.
        artist: Option<String>,
        /// The album.
        album: Option<String>,
    },
}

/// A search's path.
///
/// # Errors
///
/// [`Why::NothingToAsk`] for no words and no title: the service answers 503
/// to a search with neither, which would read as the service being down.
pub fn search_path(search: &Search) -> Result<String, Why> {
    match search {
        Search::Words { words } => {
            let words = present(Some(words)).ok_or(Why::NothingToAsk)?;
            Ok(format!("/api/search?q={}", escape(&words)))
        }
        Search::Track {
            track,
            artist,
            album,
        } => {
            let track = present(Some(track)).ok_or(Why::NothingToAsk)?;
            let mut out = format!("/api/search?track_name={}", escape(&track));
            if let Some(a) = present(artist.as_deref()) {
                out.push_str(&format!("&artist_name={}", escape(&a)));
            }
            if let Some(a) = present(album.as_deref()) {
                out.push_str(&format!("&album_name={}", escape(&a)));
            }
            Ok(out)
        }
    }
}

/// Which text a record's lyrics are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// LRC, a time on every line.
    Synced,
    /// Words, no times.
    Plain,
}

/// One record of LRCLIB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Its id.
    pub id: u64,
    /// The song's title.
    pub track: String,
    /// The artist.
    pub artist: String,
    /// The album.
    pub album: Option<String>,
    /// The song's length, as the record says.
    pub duration_seconds: Option<f64>,
    /// An instrumental: there are no words, and the record says so.
    pub instrumental: bool,
    /// The words, no times.
    pub plain: Option<String>,
    /// The words, in LRC.
    pub synced: Option<String>,
}

impl Record {
    /// The lyrics to use: synced when there are, plain otherwise; `None` for
    /// an instrumental or a record with neither.
    #[must_use]
    pub fn best(&self) -> Option<(Kind, &str)> {
        if self.instrumental {
            return None;
        }
        self.synced
            .as_deref()
            .map(|t| (Kind::Synced, t))
            .or_else(|| self.plain.as_deref().map(|t| (Kind::Plain, t)))
    }

    /// How far its length is from `duration`, in seconds, when both are
    /// known — what tells a live version from the studio one.
    #[must_use]
    pub fn off_by_seconds(&self, duration: Option<f64>) -> Option<f64> {
        Some((self.duration_seconds? - duration?).abs())
    }
}

/// A record as the service writes it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Raw {
    id: u64,
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    album_name: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    instrumental: Option<bool>,
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
}

impl From<Raw> for Record {
    fn from(r: Raw) -> Self {
        Self {
            id: r.id,
            track: present(r.track_name.as_deref().or(r.name.as_deref())).unwrap_or_default(),
            artist: present(r.artist_name.as_deref()).unwrap_or_default(),
            album: present(r.album_name.as_deref()),
            duration_seconds: r.duration.filter(|d| d.is_finite() && *d > 0.0),
            instrumental: r.instrumental.unwrap_or(false),
            plain: present(r.plain_lyrics.as_deref()),
            synced: present(r.synced_lyrics.as_deref()),
        }
    }
}

/// The record of a `/api/get` answer. Pure.
///
/// # Errors
///
/// [`Refusal`] with [`Why::Malformed`] when it is not one.
pub fn parse_one(body: &[u8]) -> Result<Record, Refusal> {
    object::<Raw>(Service::Lrclib, body).map(Record::from)
}

/// The records of a `/api/search` answer, at most [`SEARCH_MOST`]. Pure.
///
/// # Errors
///
/// [`Refusal`] with [`Why::Malformed`] when it is not a list of records.
pub fn parse_search(body: &[u8]) -> Result<Vec<Record>, Refusal> {
    let all: Vec<Raw> = json(Service::Lrclib, body)?;
    Ok(all
        .into_iter()
        .take(SEARCH_MOST)
        .map(Record::from)
        .collect())
}

/// The record LRCLIB has for this song.
///
/// # Errors
///
/// [`Why::NothingFound`] (its 404 for a song it does not know), and what the
/// network answers.
pub fn one(
    net: &dyn Net,
    artist: &str,
    title: &str,
    album: Option<&str>,
    duration: Option<f64>,
) -> Result<Record, Refusal> {
    let url = format!(
        "{}{}",
        Host::Lrclib.origin(),
        path(artist, title, album, duration)
    );
    parse_one(&answered(net, &Request::get(Host::Lrclib, url))?)
}

/// The records a search finds; an empty list is an answer.
///
/// # Errors
///
/// [`Why::NothingToAsk`] (nothing is sent), and what the network answers.
pub fn search(net: &dyn Net, search: &Search) -> Result<Vec<Record>, Refusal> {
    let path = search_path(search).map_err(|why| Refusal::new(Service::Lrclib, why))?;
    let url = format!("{}{path}", Host::Lrclib.origin());
    parse_search(&answered(
        net,
        &Request::get(Host::Lrclib, url).most(SEARCH_MOST_BYTES),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    #[test]
    fn the_request_names_all_four_and_escapes_them() {
        assert_eq!(
            path("Sigur Rós", "Hoppípolla", Some("Takk…"), Some(268.4)),
            "/api/get?artist_name=Sigur%20R%C3%B3s&track_name=Hopp%C3%ADpolla\
             &album_name=Takk%E2%80%A6&duration=268"
        );
        assert_eq!(
            path("a", "b", None, None),
            "/api/get?artist_name=a&track_name=b"
        );
    }

    #[test]
    fn synced_lyrics_win_and_an_instrumental_has_none() {
        let both =
            br#"{"id":1,"syncedLyrics":"[00:01.00]one\n[00:02.00]two","plainLyrics":"one\ntwo"}"#;
        let r = parse_one(both).unwrap();
        assert_eq!(
            r.best(),
            Some((Kind::Synced, "[00:01.00]one\n[00:02.00]two"))
        );
        let plain = br#"{"id":2,"syncedLyrics":null,"plainLyrics":"one\ntwo"}"#;
        assert_eq!(
            parse_one(plain).unwrap().best(),
            Some((Kind::Plain, "one\ntwo"))
        );
        let blank = br#"{"id":3,"syncedLyrics":"  ","plainLyrics":"x"}"#;
        assert_eq!(parse_one(blank).unwrap().best(), Some((Kind::Plain, "x")));
        let instrumental = br#"{"id":4,"instrumental":true,"plainLyrics":"x"}"#;
        assert_eq!(parse_one(instrumental).unwrap().best(), None);
        assert_eq!(
            parse_one(b"not json").unwrap_err(),
            Refusal::new(Service::Lrclib, Why::Malformed)
        );
    }

    #[test]
    fn a_search_is_by_words_or_by_title_and_never_by_nothing() {
        assert_eq!(
            search_path(&Search::Words {
                words: "hoppipolla sigur".into()
            })
            .unwrap(),
            "/api/search?q=hoppipolla%20sigur"
        );
        assert_eq!(
            search_path(&Search::Track {
                track: "Hoppípolla".into(),
                artist: Some("Sigur Rós".into()),
                album: Some(" ".into()),
            })
            .unwrap(),
            "/api/search?track_name=Hopp%C3%ADpolla&artist_name=Sigur%20R%C3%B3s"
        );
        let net = Canned::new(&[]);
        let nothing = Search::Track {
            track: "  ".into(),
            artist: Some("x".into()),
            album: None,
        };
        assert_eq!(search(&net, &nothing).unwrap_err().why, Why::NothingToAsk);
        assert!(net.asked().is_empty(), "nothing was sent");
    }

    #[test]
    fn a_search_answers_records_with_their_length() {
        let body = br#"[
          {"id":10,"trackName":"Song","artistName":"A","albumName":"B","duration":200.0,
           "instrumental":false,"plainLyrics":"la","syncedLyrics":null},
          {"id":11,"name":"Song (Live)","artistName":"A","duration":260.5,
           "instrumental":true,"plainLyrics":null,"syncedLyrics":null}
        ]"#;
        let url = "https://lrclib.net/api/search?q=song";
        let net = Canned::new(&[(url, 200, body)]);
        let found = search(
            &net,
            &Search::Words {
                words: "song".into(),
            },
        )
        .unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].album.as_deref(), Some("B"));
        assert_eq!(found[1].track, "Song (Live)");
        assert_eq!(found[1].best(), None);
        assert_eq!(found[0].off_by_seconds(Some(203.0)), Some(3.0));
        assert_eq!(found[0].off_by_seconds(None), None);
        assert_eq!(net.only().most_bytes, SEARCH_MOST_BYTES);
    }

    #[test]
    fn a_song_it_does_not_know_is_nothing_found() {
        let url = "https://lrclib.net/api/get?artist_name=a&track_name=b";
        let net = Canned::new(&[(url, 404, br#"{"code":404}"#)]);
        assert_eq!(
            one(&net, "a", "b", None, None).unwrap_err(),
            Refusal::new(Service::Lrclib, Why::NothingFound)
        );
    }
}
