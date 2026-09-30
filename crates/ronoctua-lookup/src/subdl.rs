//! Subtitles from SubDL (`api.subdl.com/api/v1`), with the person's free key.
//!
//! # The key is in the query string, and only there
//!
//! SubDL documents one way to send the key: `api_key=` in the address. So it
//! is the one key this crate puts in a query — as the request's
//! [`secret_query`](crate::ask::Request::secret_query), appended only by the
//! transport, so the address a log line or an error shows never has it. It
//! still reaches SubDL's own logs; that is the service's design, and the
//! reason the person is told the key is SubDL's alone.
//!
//! # A zip
//!
//! A result is a zip on `dl.subdl.com` (a season pack is one zip of many
//! episodes). It is read with [`ronoctua_zip`], bounded on the way out, and
//! only the members that are subtitles come back ([`subtitles_in`]).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ask::{Net, Request, answered, object, present};
use crate::found::{Found, Take, tongue};
use crate::keys::Secret;
use crate::query::{escape, language_tag};
use crate::{Host, Refusal, Service, Why};

/// The most a zip may be.
pub const ZIP_MOST_BYTES: u64 = 16 << 20;

/// The most one subtitle inside it may inflate to.
pub const MEMBER_MOST_BYTES: u64 = 4 << 20;

/// The most subtitles taken out of one zip (a season pack).
pub const MEMBERS_MOST: usize = 64;

/// How many results a search asks for (the service's most).
pub const PER_PAGE: u8 = 30;

/// The extensions that are subtitles, lowercase.
pub const EXTENSIONS: [&str; 14] = [
    "srt", "ass", "ssa", "vtt", "sub", "idx", "smi", "sami", "ttml", "dfxp", "sbv", "mpl", "txt",
    "sup",
];

/// Film or series, as SubDL asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A film.
    Movie,
    /// A series.
    Tv,
}

/// What a search asks with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Search {
    /// The title.
    pub name: Option<String>,
    /// The IMDb id (the number).
    pub imdb: Option<u64>,
    /// The TMDB id.
    pub tmdb: Option<u64>,
    /// Film or series.
    pub kind: Option<Kind>,
    /// The year.
    pub year: Option<u16>,
    /// The season.
    pub season: Option<u16>,
    /// The episode.
    pub episode: Option<u16>,
    /// The languages (`en`, `pt-br`).
    pub languages: Vec<String>,
}

/// SubDL's code for a language: a code SubDL spells itself (`EN`, `BR_PT`,
/// `ZH_BG`) as it is — what the player's table of languages hands over —
/// else a tag made uppercase, with Brazilian Portuguese its own `BR_PT`.
/// Pure.
#[must_use]
pub fn language_code(tag: &str) -> Option<String> {
    let given = tag.trim();
    let own = given.len() <= 6
        && given.split('_').all(|part| {
            (2..=3).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_uppercase())
        })
        && given.split('_').count() <= 2;
    if own {
        return Some(given.to_owned());
    }
    language_tag_code(given)
}

/// [`language_code`] of a tag.
fn language_tag_code(tag: &str) -> Option<String> {
    let tag = language_tag(tag)?;
    Some(match tag.as_str() {
        "pt-br" => "BR_PT".to_owned(),
        other => other
            .split('-')
            .next()
            .unwrap_or(other)
            .to_ascii_uppercase(),
    })
}

/// A search's request: the address without the key, and the key as its
/// secret query.
///
/// # Errors
///
/// [`Why::NothingToAsk`] with no title and no id.
pub fn search_request(key: &Secret, search: &Search) -> Result<Request, Why> {
    let name = present(search.name.as_deref());
    if name.is_none() && search.imdb.is_none() && search.tmdb.is_none() {
        return Err(Why::NothingToAsk);
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(n) = name {
        parts.push(format!("film_name={}", escape(&n)));
    }
    if let Some(i) = search.imdb {
        parts.push(format!("imdb_id=tt{i:07}"));
    }
    if let Some(t) = search.tmdb {
        parts.push(format!("tmdb_id={t}"));
    }
    if let Some(k) = search.kind {
        parts.push(format!(
            "type={}",
            match k {
                Kind::Movie => "movie",
                Kind::Tv => "tv",
            }
        ));
    }
    if let Some(y) = search.year {
        parts.push(format!("year={y}"));
    }
    if let Some(s) = search.season {
        parts.push(format!("season_number={s}"));
    }
    if let Some(e) = search.episode {
        parts.push(format!("episode_number={e}"));
    }
    let mut codes: Vec<String> = search
        .languages
        .iter()
        .filter_map(|l| language_code(l))
        .collect();
    codes.dedup();
    if !codes.is_empty() {
        parts.push(format!("languages={}", codes.join(",")));
    }
    parts.push(format!("subs_per_page={PER_PAGE}"));
    Ok(Request::get(
        Host::SubDl,
        format!(
            "{}/api/v1/subtitles?{}",
            Host::SubDl.origin(),
            parts.join("&")
        ),
    )
    .secret_query("api_key", key.clone()))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSubtitle {
    release_name: Option<String>,
    name: Option<String>,
    language: Option<String>,
    url: Option<String>,
    season: Option<u16>,
    episode: Option<u16>,
    hi: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawResult {
    name: Option<String>,
    year: Option<u16>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSearch {
    status: bool,
    error: Option<String>,
    results: Vec<RawResult>,
    subtitles: Vec<RawSubtitle>,
}

/// Whether `path` is a zip on `dl.subdl.com`: `/subtitle/…​.zip`, plain
/// characters. Pure.
#[must_use]
pub fn is_file_path(path: &str) -> bool {
    path.starts_with("/subtitle/")
        && path.ends_with(".zip")
        && !path.contains("..")
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
}

/// Which refusal SubDL's own error words are. It answers `200` with
/// `status: false` and a sentence, so the sentence is all there is. Pure.
#[must_use]
pub fn why_of_error(said: &str) -> Why {
    let said = said.to_ascii_lowercase();
    if said.contains("key") {
        Why::KeyRefused
    } else if said.contains("not found") || said.contains("can't find") || said.contains("no sub") {
        Why::NothingFound
    } else if said.contains("limit") || said.contains("too many") {
        Why::TooManyAsks
    } else {
        Why::Declined
    }
}

/// A search answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one, and the refusal its error words
/// are ([`why_of_error`]).
pub fn parse_search(body: &[u8]) -> Result<Vec<Found>, Refusal> {
    let raw: RawSearch = object(Service::SubDl, body)?;
    if !raw.status {
        let why = raw.error.as_deref().map_or(Why::Declined, why_of_error);
        return Err(Refusal::new(Service::SubDl, why));
    }
    let title = raw.results.first().and_then(|r| present(r.name.as_deref()));
    let year = raw.results.first().and_then(|r| r.year);
    Ok(raw
        .subtitles
        .into_iter()
        .filter_map(|s| {
            let path = s.url.filter(|p| is_file_path(p))?;
            Some(Found {
                source: Service::SubDl,
                take: Take::SubDl { path },
                language: tongue(s.language.as_deref().unwrap_or_default()),
                release: present(s.release_name.as_deref()).unwrap_or_default(),
                file_name: present(s.name.as_deref()),
                downloads: None,
                hearing_impaired: s.hi,
                fps: None,
                hash_match: false,
                machine_translated: false,
                title: title.clone(),
                year,
                season: s.season.filter(|n| *n > 0),
                episode: s.episode,
            })
        })
        .collect())
}

/// A subtitle out of a zip: its name (the last part of its path) and bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct Member {
    /// Its file name, without the folders inside the zip.
    pub name: String,
    /// Its bytes, as they are (the charset is the reader's to find).
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for Member {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Member")
            .field("name", &self.name)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// The subtitles in a zip, by extension ([`EXTENSIONS`]), at most
/// [`MEMBERS_MOST`], each bounded by [`MEMBER_MOST_BYTES`] on the way out.
/// A member whose name has no file part (a folder) or a `..` is skipped.
/// Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for a zip this cannot walk, [`Why::TooLarge`] for a
/// member past its bound, [`Why::NothingFound`] for a zip with no subtitle.
pub fn subtitles_in(zip: &[u8]) -> Result<Vec<Member>, Why> {
    let members = ronoctua_zip::members(zip).map_err(|_| Why::Malformed)?;
    let mut out = Vec::new();
    for m in &members {
        let full = m.name().replace('\\', "/");
        let Some(name) = full.rsplit('/').next().filter(|n| !n.is_empty()) else {
            continue;
        };
        let subtitle = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
        if !subtitle || full.split('/').any(|part| part == "..") {
            continue;
        }
        let bytes = ronoctua_zip::member_up_to(zip, m, MEMBER_MOST_BYTES).map_err(|t| match t {
            ronoctua_zip::Trouble::TooBig(_) => Why::TooLarge,
            ronoctua_zip::Trouble::Malformed => Why::Malformed,
        })?;
        out.push(Member {
            name: name.to_owned(),
            bytes,
        });
        if out.len() == MEMBERS_MOST {
            break;
        }
    }
    if out.is_empty() {
        return Err(Why::NothingFound);
    }
    Ok(out)
}

/// What a search finds; an empty list is an answer.
///
/// # Errors
///
/// [`Why::NothingToAsk`] (nothing is sent), the refusal SubDL's words are,
/// and what the network answers.
pub fn search(net: &dyn Net, key: &Secret, search: &Search) -> Result<Vec<Found>, Refusal> {
    let request = search_request(key, search).map_err(|why| Refusal::new(Service::SubDl, why))?;
    let mut found = parse_search(&answered(net, &request)?)?;
    crate::found::rank(&mut found);
    Ok(found)
}

/// Whether the key opens SubDL, asked when the person pastes it: a search
/// for one well-known film, one result, since SubDL has no other way to ask.
///
/// # Errors
///
/// [`Why::KeyRefused`], and what the network answers.
pub fn check(net: &dyn Net, key: &Secret) -> Result<(), Refusal> {
    let mut r = search_request(
        key,
        &Search {
            imdb: Some(133_093),
            ..Search::default()
        },
    )
    .map_err(|why| Refusal::new(Service::SubDl, why))?;
    r.url = r
        .url
        .replace(&format!("subs_per_page={PER_PAGE}"), "subs_per_page=1");
    match parse_search(&answered(net, &r)?) {
        Ok(_) => Ok(()),
        Err(e) if e.why == Why::NothingFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The subtitles of one result: its zip, fetched and opened.
///
/// # Errors
///
/// [`Why::Malformed`] for a path that is not a SubDL zip (nothing is sent)
/// or a zip this cannot walk, [`Why::NothingFound`] for a zip with no
/// subtitle, and what the network answers.
pub fn fetch(net: &dyn Net, path: &str) -> Result<Vec<Member>, Refusal> {
    let refused = |why| Refusal::new(Service::SubDl, why);
    if !is_file_path(path) {
        return Err(refused(Why::Malformed));
    }
    let r = Request::get(
        Host::SubDlFiles,
        format!("{}{path}", Host::SubDlFiles.origin()),
    )
    .accepting("application/zip")
    .identity()
    .most(ZIP_MOST_BYTES);
    subtitles_in(&answered(net, &r)?).map_err(refused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    /// A zip of stored members, built by hand: what a test hands the reader.
    fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, bytes) in files {
            let at = u32::try_from(out.len()).unwrap();
            let size = u32::try_from(bytes.len()).unwrap();
            let name_len = u16::try_from(name.len()).unwrap();
            out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            out.extend_from_slice(&0u32.to_le_bytes()); // crc (unchecked)
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&name_len.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(bytes);
            central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0]);
            central.extend_from_slice(&[0, 0, 0, 0]); // time, date
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&name_len.to_le_bytes());
            central.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend_from_slice(&at.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let dir_at = u32::try_from(out.len()).unwrap();
        let dir_len = u32::try_from(central.len()).unwrap();
        let count = u16::try_from(files.len()).unwrap();
        out.extend_from_slice(&central);
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&dir_len.to_le_bytes());
        out.extend_from_slice(&dir_at.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    fn key() -> Secret {
        Secret::new("subdl-key-123")
    }

    #[test]
    fn the_key_rides_only_on_the_wire_and_the_rest_is_escaped() {
        let r = search_request(
            &key(),
            &Search {
                name: Some("Amélie".into()),
                imdb: Some(211_915),
                kind: Some(Kind::Movie),
                year: Some(2001),
                languages: vec!["pt-BR".into(), "en".into()],
                ..Search::default()
            },
        )
        .unwrap();
        assert_eq!(
            r.url,
            "https://api.subdl.com/api/v1/subtitles?film_name=Am%C3%A9lie&imdb_id=tt0211915\
             &type=movie&year=2001&languages=BR_PT,EN&subs_per_page=30"
        );
        assert!(!r.url.contains("subdl-key-123"));
        assert!(!format!("{r:?}").contains("subdl-key-123"));
        assert!(r.exposed_url().ends_with("&api_key=subdl-key-123"));
        assert_eq!(
            search_request(&key(), &Search::default()),
            Err(Why::NothingToAsk)
        );
    }

    #[test]
    fn a_search_answer_gives_zips_and_its_error_words_are_refusals() {
        let body =
            br#"{"status":true,"results":[{"sd_id":1,"type":"movie","name":"Amelie","year":2001}],
          "subtitles":[
            {"release_name":"Amelie.2001.1080p","name":"SUBDL::amelie.zip","lang":"english",
             "language":"EN","url":"/subtitle/123-456.zip","hi":false,"season":0,"episode":null},
            {"release_name":"bad","language":"BR_PT","url":"https://evil.example/x.zip"},
            {"release_name":"Amelie.pt","language":"BR_PT","url":"/subtitle/7-8.zip","hi":true}
          ]}"#;
        let found = parse_search(body).unwrap();
        assert_eq!(
            found.len(),
            2,
            "an address that is not SubDL's zip is dropped"
        );
        assert_eq!(
            found[0].take,
            Take::SubDl {
                path: "/subtitle/123-456.zip".into()
            }
        );
        assert_eq!(found[0].title.as_deref(), Some("Amelie"));
        assert_eq!(found[0].season, None, "season 0 is no season");
        assert_eq!(found[1].language, "pt-br");
        assert!(found[1].hearing_impaired);

        for (said, why) in [
            ("Invalid API key", Why::KeyRefused),
            ("can't find movie or tv", Why::NothingFound),
            ("rate limit exceeded", Why::TooManyAsks),
            ("something else", Why::Declined),
        ] {
            let body = format!(r#"{{"status":false,"error":"{said}"}}"#);
            assert_eq!(
                parse_search(body.as_bytes()).unwrap_err().why,
                why,
                "{said}"
            );
        }
    }

    #[test]
    fn a_zip_gives_its_subtitles_and_nothing_else() {
        let z = zip(&[
            (
                "Season 1/Show.S01E01.srt",
                b"1\n00:00:01,000 --> 00:00:02,000\nhi\n",
            ),
            ("readme.nfo", b"release notes"),
            ("../../etc/evil.srt", b"x"),
            ("Show.S01E02.ASS", b"[Script Info]"),
        ]);
        let subs = subtitles_in(&z).unwrap();
        let names: Vec<&str> = subs.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Show.S01E01.srt", "Show.S01E02.ASS"]);
        assert!(subs[0].bytes.starts_with(b"1\n"));
        assert_eq!(
            subtitles_in(&zip(&[("a.nfo", b"x")])),
            Err(Why::NothingFound)
        );
        assert_eq!(subtitles_in(b"not a zip"), Err(Why::Malformed));
    }

    #[test]
    fn a_result_is_fetched_from_the_files_host_as_it_is() {
        let z = zip(&[("a.srt", b"1\n")]);
        let url = "https://dl.subdl.com/subtitle/123-456.zip";
        let net = Canned::new(&[(url, 200, &z)]);
        let subs = fetch(&net, "/subtitle/123-456.zip").unwrap();
        assert_eq!(subs.len(), 1);
        let asked = net.only();
        assert_eq!(asked.host, Host::SubDlFiles);
        assert!(!asked.gzip);
        assert!(asked.secret_query.is_none(), "the file needs no key");
        let net = Canned::new(&[]);
        assert_eq!(fetch(&net, "/../x.zip").unwrap_err().why, Why::Malformed);
        assert!(net.asked().is_empty());
    }

    #[test]
    fn a_language_is_subdls_code() {
        assert_eq!(language_code("pt-BR").as_deref(), Some("BR_PT"));
        assert_eq!(language_code("en").as_deref(), Some("EN"));
        assert_eq!(language_code("zh-cn").as_deref(), Some("ZH"));
        assert_eq!(language_code("x"), None);
        // SUBDL'S OWN SPELLING goes as it is: the player's table of
        // languages hands it over, traditional Chinese included.
        assert_eq!(language_code("ZH_BG").as_deref(), Some("ZH_BG"));
        assert_eq!(language_code("BR_PT").as_deref(), Some("BR_PT"));
        assert_eq!(language_code("PT").as_deref(), Some("PT"));
        assert_eq!(
            language_code("EN,X").as_deref(),
            None,
            "never a second code"
        );
    }
}
