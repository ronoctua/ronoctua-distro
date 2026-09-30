//! A song's tags from MusicBrainz (`musicbrainz.org/ws/2`), keyless.
//!
//! Three questions: a SEARCH for recordings by title, artist, album and
//! length (Lucene syntax), a RECORDING by id (its ISRCs, genres, and the
//! composers and lyricists of the work it performs), and a RELEASE by id (the
//! album's artist, label, date, and where the recording sits on it). When the
//! file already carries a MusicBrainz id, the search is skipped.
//!
//! MusicBrainz asks for one request a second per address and answers 503
//! above it: its [`crate::Host::MusicBrainz`] pace is one second, and its 503
//! is [`Why::TooManyAsks`] and not "the service is down".

use ronoctua_tags::Field;
use serde::{Deserialize, Serialize};

use crate::ask::{Net, Request, answered, object, present};
use crate::query::{escape, is_mbid};
use crate::suggestion::{Cover, Suggestion, joined};
use crate::{Host, Refusal, Service, Why};

/// How many recordings a search asks for.
pub const SEARCH_LIMIT: usize = 10;

/// How far a candidate's length may be from the file's, in the search:
/// two seconds either way (an encoder's padding, a fade).
pub const LENGTH_SLACK_MS: u64 = 2000;

/// What a song search asks with — usually the file's own tags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Song {
    /// The title.
    pub title: String,
    /// The artist.
    pub artist: Option<String>,
    /// The album.
    pub album: Option<String>,
    /// The file's length.
    pub duration_seconds: Option<f64>,
}

/// A phrase for a Lucene query: quoted, with `\` and `"` escaped, so a title
/// cannot close the phrase and add a clause of its own. Pure.
#[must_use]
pub fn phrase(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        if matches!(c, '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// The search's path.
///
/// # Errors
///
/// [`Why::NothingToAsk`] with no title.
pub fn search_path(song: &Song) -> Result<String, Why> {
    let title = present(Some(&song.title)).ok_or(Why::NothingToAsk)?;
    let mut clauses = vec![format!("recording:{}", phrase(&title))];
    if let Some(a) = present(song.artist.as_deref()) {
        clauses.push(format!("artist:{}", phrase(&a)));
    }
    if let Some(a) = present(song.album.as_deref()) {
        clauses.push(format!("release:{}", phrase(&a)));
    }
    if let Some(d) = song.duration_seconds.filter(|d| d.is_finite() && *d > 0.0) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ms = (d * 1000.0).round() as u64;
        clauses.push(format!(
            "dur:[{} TO {}]",
            ms.saturating_sub(LENGTH_SLACK_MS),
            ms + LENGTH_SLACK_MS
        ));
    }
    Ok(format!(
        "/ws/2/recording?query={}&fmt=json&limit={SEARCH_LIMIT}",
        escape(&clauses.join(" AND "))
    ))
}

/// A recording's path: its credits, releases, ISRCs, genres, and the work it
/// performs with that work's writers.
///
/// # Errors
///
/// [`Why::Malformed`] for an id that is not one.
pub fn recording_path(mbid: &str) -> Result<String, Why> {
    if !is_mbid(mbid) {
        return Err(Why::Malformed);
    }
    Ok(format!(
        "/ws/2/recording/{mbid}?inc=artists+releases+isrcs+genres+work-rels+work-level-rels+artist-rels&fmt=json"
    ))
}

/// A release's path: its credits, labels, media with their recordings, and
/// genres.
///
/// # Errors
///
/// [`Why::Malformed`] for an id that is not one.
pub fn release_path(mbid: &str) -> Result<String, Why> {
    if !is_mbid(mbid) {
        return Err(Why::Malformed);
    }
    Ok(format!(
        "/ws/2/release/{mbid}?inc=artists+labels+recordings+release-groups+genres&fmt=json"
    ))
}

/// A release a recording is on, as a search or a recording answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseRef {
    /// Its id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// When it came out (`YYYY`, `YYYY-MM` or `YYYY-MM-DD`).
    pub date: Option<String>,
    /// Where.
    pub country: Option<String>,
    /// `Official`, `Bootleg`…
    pub status: Option<String>,
}

/// A recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recording {
    /// Its id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Who performs it, as credited (`A feat. B`).
    pub artist: String,
    /// Its length.
    pub length_ms: Option<u64>,
    /// The search's confidence, 0–100 (a lookup by id has none).
    pub score: Option<u8>,
    /// When it was first released.
    pub first_release_date: Option<String>,
    /// The releases it is on.
    pub releases: Vec<ReleaseRef>,
    /// Its ISRCs.
    pub isrcs: Vec<String>,
    /// Its genres, most voted first.
    pub genres: Vec<String>,
    /// Who wrote the music of the work it performs.
    pub composers: Vec<String>,
    /// Who wrote the words.
    pub lyricists: Vec<String>,
}

/// Where a recording sits on a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    /// Its track number on its disc.
    pub track: u16,
    /// How many tracks the disc has.
    pub tracks: u16,
    /// Its disc.
    pub disc: u16,
    /// How many discs the release has.
    pub discs: u16,
}

/// One track of a release's medium.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackRef {
    /// Its position on the medium.
    pub position: u16,
    /// The recording it is.
    pub recording: String,
}

/// One medium (disc) of a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Medium {
    /// Its position in the release.
    pub position: u16,
    /// How many tracks it has.
    pub track_count: u16,
    /// Its tracks.
    pub tracks: Vec<TrackRef>,
}

/// A release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    /// Its id.
    pub id: String,
    /// Its title (the album).
    pub title: String,
    /// Who the album is by, as credited.
    pub artist: String,
    /// When it came out.
    pub date: Option<String>,
    /// Its label (the first).
    pub label: Option<String>,
    /// Its genres, most voted first.
    pub genres: Vec<String>,
    /// Whether the Cover Art Archive has a front image for it.
    pub front_cover: bool,
    /// Its media.
    pub media: Vec<Medium>,
}

impl Release {
    /// Where `recording` is on it.
    #[must_use]
    pub fn place_of(&self, recording: &str) -> Option<Place> {
        let discs = u16::try_from(self.media.len()).unwrap_or(u16::MAX);
        self.media.iter().find_map(|m| {
            m.tracks
                .iter()
                .find(|t| t.recording == recording)
                .map(|t| Place {
                    track: t.position,
                    tracks: m.track_count,
                    disc: m.position,
                    discs,
                })
        })
    }
}

// THE SERVICE'S SHAPES, read loosely: every field defaulted, so a field the
// service leaves out is absent and not a parse error.

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawCredit {
    name: String,
    joinphrase: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawNamed {
    name: String,
    count: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawArtist {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawWorkRelation {
    #[serde(rename = "type")]
    kind: String,
    artist: Option<RawArtist>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawWork {
    relations: Vec<RawWorkRelation>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawRelation {
    #[serde(rename = "type")]
    kind: String,
    work: Option<RawWork>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawRecordingRef {
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawTrack {
    position: u16,
    recording: Option<RawRecordingRef>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "kebab-case")]
struct RawMedium {
    position: u16,
    track_count: u16,
    #[serde(alias = "track")]
    tracks: Vec<RawTrack>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawLabel {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawLabelInfo {
    label: Option<RawLabel>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawCoverArt {
    front: bool,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "kebab-case")]
struct RawRelease {
    id: String,
    title: String,
    date: Option<String>,
    country: Option<String>,
    status: Option<String>,
    artist_credit: Vec<RawCredit>,
    label_info: Vec<RawLabelInfo>,
    genres: Vec<RawNamed>,
    media: Vec<RawMedium>,
    cover_art_archive: Option<RawCoverArt>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "kebab-case")]
struct RawRecording {
    id: String,
    title: String,
    length: Option<u64>,
    score: Option<u8>,
    first_release_date: Option<String>,
    artist_credit: Vec<RawCredit>,
    releases: Vec<RawRelease>,
    isrcs: Vec<String>,
    genres: Vec<RawNamed>,
    relations: Vec<RawRelation>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSearch {
    recordings: Vec<RawRecording>,
}

/// A credit as it is printed: every name and its join phrase.
fn credit(credits: &[RawCredit]) -> String {
    credits
        .iter()
        .map(|c| format!("{}{}", c.name, c.joinphrase))
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Genres, most voted first, at most three.
fn genres(named: &[RawNamed]) -> Vec<String> {
    let mut sorted: Vec<&RawNamed> = named.iter().filter(|g| !g.name.is_empty()).collect();
    sorted.sort_by(|a, b| b.count.cmp(&a.count).then(a.name.cmp(&b.name)));
    sorted.into_iter().take(3).map(|g| g.name.clone()).collect()
}

/// The writers of the works a recording performs, by the relation's type.
fn writers(relations: &[RawRelation], of: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for work in relations
        .iter()
        .filter(|r| r.kind == "performance")
        .filter_map(|r| r.work.as_ref())
    {
        for r in &work.relations {
            if of.contains(&r.kind.as_str())
                && let Some(a) = &r.artist
                && !a.name.is_empty()
                && !out.contains(&a.name)
            {
                out.push(a.name.clone());
            }
        }
    }
    out
}

impl From<RawRecording> for Recording {
    fn from(r: RawRecording) -> Self {
        Self {
            id: r.id,
            title: r.title,
            artist: credit(&r.artist_credit),
            length_ms: r.length,
            score: r.score,
            first_release_date: present(r.first_release_date.as_deref()),
            releases: r
                .releases
                .into_iter()
                .map(|x| ReleaseRef {
                    id: x.id,
                    title: x.title,
                    date: present(x.date.as_deref()),
                    country: present(x.country.as_deref()),
                    status: present(x.status.as_deref()),
                })
                .collect(),
            isrcs: r.isrcs,
            genres: genres(&r.genres),
            composers: writers(&r.relations, &["composer", "writer"]),
            lyricists: writers(&r.relations, &["lyricist", "librettist", "writer"]),
        }
    }
}

/// The recordings of a search answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_search(body: &[u8]) -> Result<Vec<Recording>, Refusal> {
    let raw: RawSearch = object(Service::MusicBrainz, body)?;
    Ok(raw
        .recordings
        .into_iter()
        .filter(|r| is_mbid(&r.id))
        .map(Recording::from)
        .collect())
}

/// A recording answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_recording(body: &[u8]) -> Result<Recording, Refusal> {
    let raw: RawRecording = object(Service::MusicBrainz, body)?;
    if !is_mbid(&raw.id) {
        return Err(Refusal::new(Service::MusicBrainz, Why::Malformed));
    }
    Ok(Recording::from(raw))
}

/// A release answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_release(body: &[u8]) -> Result<Release, Refusal> {
    let raw: RawRelease = object(Service::MusicBrainz, body)?;
    if !is_mbid(&raw.id) {
        return Err(Refusal::new(Service::MusicBrainz, Why::Malformed));
    }
    Ok(Release {
        id: raw.id,
        title: raw.title,
        artist: credit(&raw.artist_credit),
        date: present(raw.date.as_deref()),
        label: raw
            .label_info
            .iter()
            .filter_map(|l| l.label.as_ref())
            .map(|l| l.name.clone())
            .find(|n| !n.trim().is_empty()),
        genres: genres(&raw.genres),
        front_cover: raw.cover_art_archive.is_some_and(|c| c.front),
        media: raw
            .media
            .into_iter()
            .map(|m| Medium {
                position: m.position,
                track_count: m.track_count,
                tracks: m
                    .tracks
                    .into_iter()
                    .filter_map(|t| {
                        Some(TrackRef {
                            position: t.position,
                            recording: t.recording?.id,
                        })
                    })
                    .collect(),
            })
            .collect(),
    })
}

/// The candidate a recording (and the release chosen for it) makes: the
/// fields a sheet would write, and the release's front cover.
#[must_use]
pub fn suggestion(recording: &Recording, release: Option<&Release>) -> Suggestion {
    let mut s = Suggestion::new(Service::MusicBrainz, recording.id.clone());
    s.score = recording.score;
    s.put(Field::Title, Some(recording.title.clone()));
    s.put(Field::Artist, Some(recording.artist.clone()));
    s.put(Field::Isrc, recording.isrcs.first().cloned());
    s.put(Field::Composer, joined(&recording.composers));
    s.put(Field::Lyricist, joined(&recording.lyricists));
    if let Some(r) = release {
        s.put(Field::Album, Some(r.title.clone()));
        s.put(Field::AlbumArtist, Some(r.artist.clone()));
        s.put(Field::Date, r.date.clone());
        s.put(Field::Publisher, r.label.clone());
        if let Some(p) = r.place_of(&recording.id) {
            s.put(Field::Track, Some(format!("{}/{}", p.track, p.tracks)));
            s.put(Field::Disc, Some(format!("{}/{}", p.disc, p.discs)));
        }
        let genres = if recording.genres.is_empty() {
            &r.genres
        } else {
            &recording.genres
        };
        s.put(Field::Genre, joined(genres));
        if r.front_cover {
            s.cover = Some(Cover::CoverArt {
                release: r.id.clone(),
            });
        }
    } else {
        s.put(Field::Genre, joined(&recording.genres));
        s.put(
            Field::Album,
            recording.releases.first().map(|r| r.title.clone()),
        );
    }
    s.put(Field::Date, recording.first_release_date.clone());
    s
}

/// The recordings a search finds; an empty list is an answer.
///
/// # Errors
///
/// [`Why::NothingToAsk`] (nothing is sent), and what the network answers.
pub fn search(net: &dyn Net, song: &Song) -> Result<Vec<Recording>, Refusal> {
    let path = search_path(song).map_err(|why| Refusal::new(Service::MusicBrainz, why))?;
    let url = format!("{}{path}", Host::MusicBrainz.origin());
    parse_search(&answered(net, &Request::get(Host::MusicBrainz, url))?)
}

/// A recording by id.
///
/// # Errors
///
/// [`Why::Malformed`] for an id that is not one (nothing is sent), and what
/// the network answers.
pub fn recording(net: &dyn Net, mbid: &str) -> Result<Recording, Refusal> {
    let path = recording_path(mbid).map_err(|why| Refusal::new(Service::MusicBrainz, why))?;
    let url = format!("{}{path}", Host::MusicBrainz.origin());
    parse_recording(&answered(net, &Request::get(Host::MusicBrainz, url))?)
}

/// A release by id.
///
/// # Errors
///
/// As [`recording`].
pub fn release(net: &dyn Net, mbid: &str) -> Result<Release, Refusal> {
    let path = release_path(mbid).map_err(|why| Refusal::new(Service::MusicBrainz, why))?;
    let url = format!("{}{path}", Host::MusicBrainz.origin());
    parse_release(&answered(net, &Request::get(Host::MusicBrainz, url))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    const REC: &str = "11111111-1111-4111-8111-111111111111";
    const REL: &str = "22222222-2222-4222-8222-222222222222";

    fn song() -> Song {
        Song {
            title: "Hoppípolla".into(),
            artist: Some("Sigur Rós".into()),
            album: None,
            duration_seconds: Some(268.4),
        }
    }

    #[test]
    fn a_search_is_one_lucene_query_escaped_whole() {
        let p = search_path(&song()).unwrap();
        // recording:"Hoppípolla" AND artist:"Sigur Rós" AND dur:[266400 TO 270400]
        let q =
            escape("recording:\"Hoppípolla\" AND artist:\"Sigur Rós\" AND dur:[266400 TO 270400]");
        assert_eq!(p, format!("/ws/2/recording?query={q}&fmt=json&limit=10"));
        assert_eq!(
            search_path(&Song {
                title: " ".into(),
                ..song()
            }),
            Err(Why::NothingToAsk)
        );
    }

    #[test]
    fn a_title_cannot_close_its_phrase() {
        assert_eq!(phrase(r#"a" OR artist:"b"#), r#""a\" OR artist:\"b""#);
        assert_eq!(phrase(r"back\slash"), r#""back\\slash""#);
    }

    #[test]
    fn an_id_that_is_not_one_never_becomes_a_path() {
        assert_eq!(recording_path("../../x"), Err(Why::Malformed));
        assert_eq!(release_path("abc"), Err(Why::Malformed));
        let net = Canned::new(&[]);
        assert_eq!(recording(&net, "nope").unwrap_err().why, Why::Malformed);
        assert!(net.asked().is_empty());
        assert!(
            recording_path(REC)
                .unwrap()
                .starts_with(&format!("/ws/2/recording/{REC}?inc="))
        );
    }

    #[test]
    fn a_search_answer_gives_recordings_with_their_credit() {
        let body = format!(
            r#"{{"count":2,"recordings":[
              {{"id":"{REC}","score":100,"title":"Hoppípolla","length":268000,
                "artist-credit":[{{"name":"Sigur Rós","joinphrase":" feat. "}},{{"name":"X","joinphrase":""}}],
                "first-release-date":"2005-09-12",
                "releases":[{{"id":"{REL}","title":"Takk…","date":"2005-09-12","country":"GB","status":"Official",
                  "media":[{{"position":1,"track-count":11,"track":[{{"position":2}}]}}]}}]}},
              {{"id":"not-an-id","title":"junk"}}
            ]}}"#
        );
        let found = parse_search(body.as_bytes()).unwrap();
        assert_eq!(found.len(), 1, "an id that is not one is dropped");
        let r = &found[0];
        assert_eq!(r.artist, "Sigur Rós feat. X");
        assert_eq!(r.length_ms, Some(268_000));
        assert_eq!(r.score, Some(100));
        assert_eq!(r.releases[0].title, "Takk…");
        assert_eq!(r.releases[0].country.as_deref(), Some("GB"));
        assert_eq!(
            parse_search(b"[]").unwrap_err(),
            Refusal::new(Service::MusicBrainz, Why::Malformed)
        );
    }

    #[test]
    fn a_recording_answer_names_its_writers_through_its_work() {
        let body = format!(
            r#"{{"id":"{REC}","title":"Song","length":200000,
              "artist-credit":[{{"name":"Band","joinphrase":""}}],
              "isrcs":["GBAAA0500001"],
              "genres":[{{"name":"rock","count":1}},{{"name":"post-rock","count":5}}],
              "relations":[
                {{"type":"performance","target-type":"work","work":{{"id":"w","title":"Song","relations":[
                  {{"type":"composer","artist":{{"name":"A"}}}},
                  {{"type":"lyricist","artist":{{"name":"B"}}}},
                  {{"type":"writer","artist":{{"name":"C"}}}},
                  {{"type":"arranger","artist":{{"name":"D"}}}}]}}}},
                {{"type":"producer","artist":{{"name":"E"}}}}
              ]}}"#
        );
        let r = parse_recording(body.as_bytes()).unwrap();
        assert_eq!(r.composers, ["A", "C"]);
        assert_eq!(r.lyricists, ["B", "C"]);
        assert_eq!(r.genres, ["post-rock", "rock"], "most voted first");
        assert_eq!(r.isrcs, ["GBAAA0500001"]);
        assert_eq!(r.score, None);
    }

    #[test]
    fn a_release_places_the_recording_and_suggests_every_field() {
        let rec_body = format!(
            r#"{{"id":"{REC}","title":"Song","artist-credit":[{{"name":"Band","joinphrase":""}}],
               "isrcs":["X1"],"first-release-date":"2004"}}"#
        );
        let rel_body = format!(
            r#"{{"id":"{REL}","title":"Album","date":"2005-09-12",
               "artist-credit":[{{"name":"Various Artists","joinphrase":""}}],
               "label-info":[{{"catalog-number":"1","label":{{"name":"EMI"}}}}],
               "genres":[{{"name":"pop","count":2}}],
               "cover-art-archive":{{"front":true,"back":false,"count":1}},
               "media":[
                 {{"position":1,"track-count":10,"tracks":[{{"position":1,"recording":{{"id":"x"}}}}]}},
                 {{"position":2,"track-count":12,"tracks":[{{"position":7,"recording":{{"id":"{REC}"}}}}]}}]}}"#
        );
        let rec_url = format!("https://musicbrainz.org{}", recording_path(REC).unwrap());
        let rel_url = format!("https://musicbrainz.org{}", release_path(REL).unwrap());
        let net = Canned::new(&[
            (rec_url.as_str(), 200, rec_body.as_bytes()),
            (rel_url.as_str(), 200, rel_body.as_bytes()),
        ]);
        let r = recording(&net, REC).unwrap();
        let album = release(&net, REL).unwrap();
        assert_eq!(
            album.place_of(REC),
            Some(Place {
                track: 7,
                tracks: 12,
                disc: 2,
                discs: 2
            })
        );
        let s = suggestion(&r, Some(&album));
        assert_eq!(s.value(Field::Title), Some("Song"));
        assert_eq!(s.value(Field::Artist), Some("Band"));
        assert_eq!(s.value(Field::Album), Some("Album"));
        assert_eq!(s.value(Field::AlbumArtist), Some("Various Artists"));
        assert_eq!(s.value(Field::Track), Some("7/12"));
        assert_eq!(s.value(Field::Disc), Some("2/2"));
        assert_eq!(
            s.value(Field::Date),
            Some("2005-09-12"),
            "the release's date first"
        );
        assert_eq!(s.value(Field::Genre), Some("pop"));
        assert_eq!(s.value(Field::Publisher), Some("EMI"));
        assert_eq!(s.value(Field::Isrc), Some("X1"));
        assert_eq!(
            s.cover,
            Some(Cover::CoverArt {
                release: REL.into()
            })
        );
        assert!(
            net.asked().iter().all(|r| r.host == Host::MusicBrainz),
            "one pace for both"
        );
    }

    #[test]
    fn too_fast_is_said_as_too_fast() {
        let url = format!("https://musicbrainz.org{}", recording_path(REC).unwrap());
        let net = Canned::new(&[(url.as_str(), 503, b"")]);
        assert_eq!(recording(&net, REC).unwrap_err().why, Why::TooManyAsks);
    }
}
