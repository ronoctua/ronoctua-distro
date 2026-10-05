//! Subtitles from OpenSubtitles (`api.opensubtitles.com/api/v1`), with the
//! person's key and, when kept, their account.
//!
//! # The questions
//!
//! - `GET /subtitles` by the file's fingerprint ([`moviehash`]) first — a
//!   match is timed for exactly this file — and by words, season, episode
//!   and languages. The parameters go sorted and lowercased: the service
//!   REDIRECTS any other spelling to that one, which is a request more.
//! - `POST /login {username, password}` answers a token (and, for some
//!   accounts, another API host) that raises the day's downloads above what
//!   the key alone gets. The token lives in the daemon's memory, never on
//!   disk.
//! - `POST /download {file_id}` answers a LINK on another host and how many
//!   downloads are left today ([`Download::remaining`], shown by the door);
//!   the file itself is a plain GET of the link, which carries no key.
//!
//! The key goes as `Api-Key`, the token as `Authorization: Bearer`; both are
//! [`Secret`]s and neither is ever in an address.

use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ask::{Net, Request, Value, answered, object, present};
use crate::found::{Found, Take, tongue};
use crate::keys::{Account, Secret};
use crate::query::{escape, language_tag};
use crate::{Host, Refusal, Service, Why};

/// The API's path, after the host.
pub const API: &str = "/api/v1";

/// How many bytes of each end of a file its fingerprint reads.
pub const CHUNK: u64 = 65_536;

/// The most a subtitle file may be: a film's subtitles are tens of kilobytes,
/// and a few megabytes is a bound, not a guess.
pub const FILE_MOST_BYTES: u64 = 4 << 20;

/// A file's fingerprint: its size plus the little-endian 64-bit sums of its
/// first and last 64 KiB (OpenSubtitles' `moviehash`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Moviehash {
    /// The hash.
    pub hash: u64,
    /// The file's size.
    pub size_bytes: u64,
}

impl Moviehash {
    /// Sixteen lowercase hex digits, as the service takes it.
    #[must_use]
    pub fn hex(&self) -> String {
        format!("{:016x}", self.hash)
    }
}

/// The fingerprint of a file of `size` bytes whose first and last [`CHUNK`]
/// bytes are `head` and `tail`: the size plus every little-endian 64-bit word
/// of both, wrapping at 64 bits (the reference's mask). Pure.
#[must_use]
pub fn moviehash_of(size: u64, head: &[u8], tail: &[u8]) -> u64 {
    let sum = |bytes: &[u8]| {
        bytes.chunks_exact(8).fold(0u64, |acc, word| {
            let mut w = [0u8; 8];
            w.copy_from_slice(word);
            acc.wrapping_add(u64::from_le_bytes(w))
        })
    };
    size.wrapping_add(sum(head)).wrapping_add(sum(tail))
}

/// A file's fingerprint. Reads 128 KiB of it: call it on a thread.
///
/// # Errors
///
/// [`Why::Unreadable`] for a file that cannot be read, [`Why::TooSmall`] for
/// one under 128 KiB (the reference implementation refuses those too: its
/// two ends would overlap).
pub fn moviehash(path: &Path) -> Result<Moviehash, Why> {
    let mut file = std::fs::File::open(path).map_err(|_| Why::Unreadable)?;
    let size = file.metadata().map_err(|_| Why::Unreadable)?.len();
    if size < CHUNK * 2 {
        return Err(Why::TooSmall);
    }
    let mut head = vec![0u8; usize::try_from(CHUNK).unwrap_or(65_536)];
    let mut tail = head.clone();
    file.read_exact(&mut head).map_err(|_| Why::Unreadable)?;
    file.seek(SeekFrom::Start(size - CHUNK))
        .map_err(|_| Why::Unreadable)?;
    file.read_exact(&mut tail).map_err(|_| Why::Unreadable)?;
    Ok(Moviehash {
        hash: moviehash_of(size, &head, &tail),
        size_bytes: size,
    })
}

/// A login: the token, and the host to ask with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// The token (`Authorization: Bearer`).
    pub token: Secret,
    /// The API host the service named for this account — always a host of
    /// `opensubtitles.com` ([`api_host`]); the key is never sent anywhere
    /// else.
    pub host: String,
    /// How many downloads the account allows a day.
    pub allowed_downloads: Option<u32>,
}

/// What a search asks with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Search {
    /// The file's fingerprint.
    pub moviehash: Option<Moviehash>,
    /// Words: the title as the file's name or tags say.
    pub query: Option<String>,
    /// The year.
    pub year: Option<u16>,
    /// The season, for an episode.
    pub season: Option<u16>,
    /// The episode.
    pub episode: Option<u16>,
    /// The IMDb id (the number, without `tt`).
    pub imdb: Option<u64>,
    /// The languages (`en`, `pt-br`), in the order the person put them.
    pub languages: Vec<String>,
}

/// A download: where the file is, and how many are left today.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Download {
    /// The link (on another host; it carries no key).
    pub link: String,
    /// The file's name.
    pub file_name: Option<String>,
    /// How many downloads are left today, as the service counts them.
    pub remaining: Option<i64>,
    /// When the count starts again, as the service writes it.
    pub reset: Option<String>,
}

/// The API host a login named, when it is a host of `opensubtitles.com`;
/// otherwise the default one. The key and the token are never sent to a host
/// a reply named unless it is theirs. Pure.
#[must_use]
pub fn api_host(named: Option<&str>) -> String {
    let fine = |h: &str| {
        (h == "opensubtitles.com" || h.ends_with(".opensubtitles.com"))
            && h.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
    };
    named
        .map(|h| {
            h.trim()
                .trim_start_matches("https://")
                .trim_end_matches('/')
        })
        .filter(|h| fine(h))
        .map_or_else(|| Host::OpenSubtitles.name().to_owned(), str::to_owned)
}

/// The search's path, parameters sorted by name and values lowercased (the
/// service redirects any other spelling).
///
/// # Errors
///
/// [`Why::NothingToAsk`] with no fingerprint, no words and no IMDb id.
pub fn search_path(search: &Search) -> Result<String, Why> {
    let query = present(search.query.as_deref()).map(|q| q.to_lowercase());
    if search.moviehash.is_none() && query.is_none() && search.imdb.is_none() {
        return Err(Why::NothingToAsk);
    }
    let languages: Vec<String> = search
        .languages
        .iter()
        .filter_map(|l| language_tag(l))
        .collect();
    let mut pairs: Vec<(&str, String)> = Vec::new();
    if let Some(e) = search.episode {
        pairs.push(("episode_number", e.to_string()));
    }
    if let Some(i) = search.imdb {
        pairs.push(("imdb_id", i.to_string()));
    }
    if !languages.is_empty() {
        let mut sorted = languages;
        sorted.sort();
        sorted.dedup();
        pairs.push(("languages", sorted.join(",")));
    }
    if let Some(h) = &search.moviehash {
        pairs.push(("moviehash", h.hex()));
    }
    if let Some(q) = query {
        pairs.push(("query", q));
    }
    if let Some(s) = search.season {
        pairs.push(("season_number", s.to_string()));
    }
    if let Some(y) = search.year {
        pairs.push(("year", y.to_string()));
    }
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    let query: Vec<String> = pairs
        .iter()
        .map(|(name, value)| {
            // `,` between languages stays a comma, as the service writes it.
            format!("{name}={}", escape(value).replace("%2C", ","))
        })
        .collect();
    Ok(format!("{API}/subtitles?{}", query.join("&")))
}

/// A request to the API with the key, and the token when there is one.
fn request(key: &Secret, session: Option<&Session>, path: &str) -> Request {
    let host = session.map_or_else(|| Host::OpenSubtitles.name().to_owned(), |s| s.host.clone());
    let mut r = Request::get(Host::OpenSubtitles, format!("https://{host}{path}"))
        .secret_header("Api-Key", key.clone());
    if let Some(s) = session {
        r = r.secret_header(
            "Authorization",
            Secret::new(format!("Bearer {}", s.token.expose())),
        );
    }
    r
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawFile {
    file_id: u64,
    file_name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawFeature {
    title: Option<String>,
    movie_name: Option<String>,
    parent_title: Option<String>,
    year: Option<u16>,
    season_number: Option<u16>,
    episode_number: Option<u16>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawAttributes {
    language: Option<String>,
    download_count: Option<u64>,
    hearing_impaired: bool,
    fps: Option<f64>,
    release: Option<String>,
    moviehash_match: bool,
    ai_translated: bool,
    machine_translated: bool,
    feature_details: RawFeature,
    files: Vec<RawFile>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawDatum {
    attributes: RawAttributes,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSearch {
    data: Vec<RawDatum>,
}

/// A search answer: one [`Found`] per FILE (a two-CD subtitle is two). Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_search(body: &[u8]) -> Result<Vec<Found>, Refusal> {
    let raw: RawSearch = object(Service::OpenSubtitles, body)?;
    let mut out = Vec::new();
    for datum in raw.data {
        let a = datum.attributes;
        let f = &a.feature_details;
        for file in a.files.iter().filter(|f| f.file_id > 0) {
            out.push(Found {
                source: Service::OpenSubtitles,
                take: Take::OpenSubtitles {
                    file_id: file.file_id,
                },
                language: tongue(a.language.as_deref().unwrap_or_default()),
                release: present(a.release.as_deref()).unwrap_or_default(),
                file_name: present(file.file_name.as_deref()),
                downloads: a.download_count,
                hearing_impaired: a.hearing_impaired,
                fps: a.fps.filter(|f| f.is_finite() && *f > 0.0),
                hash_match: a.moviehash_match,
                machine_translated: a.ai_translated || a.machine_translated,
                title: present(
                    f.parent_title
                        .as_deref()
                        .or(f.movie_name.as_deref())
                        .or(f.title.as_deref()),
                ),
                year: f.year,
                season: f.season_number,
                episode: f.episode_number,
            });
        }
    }
    Ok(out)
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawUser {
    allowed_downloads: Option<u32>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawLogin {
    token: Option<String>,
    base_url: Option<String>,
    user: RawUser,
}

/// A login answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] with no token (or one that could not be a header).
pub fn parse_login(body: &[u8]) -> Result<Session, Refusal> {
    let raw: RawLogin = object(Service::OpenSubtitles, body)?;
    let token = present(raw.token.as_deref())
        .filter(|t| !t.chars().any(char::is_control))
        .ok_or(Refusal::new(Service::OpenSubtitles, Why::Malformed))?;
    Ok(Session {
        token: Secret::new(token),
        host: api_host(raw.base_url.as_deref()),
        allowed_downloads: raw.user.allowed_downloads,
    })
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawDownload {
    link: Option<String>,
    file_name: Option<String>,
    remaining: Option<i64>,
    reset_time_utc: Option<String>,
}

/// A download answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] with no `https` link.
pub fn parse_download(body: &[u8]) -> Result<Download, Refusal> {
    let raw: RawDownload = object(Service::OpenSubtitles, body)?;
    let link = present(raw.link.as_deref())
        .filter(|l| l.starts_with("https://"))
        .ok_or(Refusal::new(Service::OpenSubtitles, Why::Malformed))?;
    Ok(Download {
        link,
        file_name: present(raw.file_name.as_deref()),
        remaining: raw.remaining,
        reset: present(raw.reset_time_utc.as_deref()),
    })
}

/// What the account allows today, as `/infos/user` says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Allowance {
    /// Downloads a day.
    pub allowed_downloads: Option<u32>,
    /// Downloads left today.
    pub remaining_downloads: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawUserInfo {
    allowed_downloads: Option<u32>,
    remaining_downloads: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawUserAnswer {
    data: Option<RawUserInfo>,
}

/// A `/infos/user` answer. Pure.
///
/// The shape is the API's documented one (`{"data":{"allowed_downloads",
/// "remaining_downloads", …}}`), written from its documentation and not
/// measured: it needs an account's token.
///
/// # Errors
///
/// [`Why::Malformed`] with no `data`.
pub fn parse_user(body: &[u8]) -> Result<Allowance, Refusal> {
    let raw: RawUserAnswer = object(Service::OpenSubtitles, body)?;
    let data = raw
        .data
        .ok_or(Refusal::new(Service::OpenSubtitles, Why::Malformed))?;
    Ok(Allowance {
        allowed_downloads: data.allowed_downloads,
        remaining_downloads: data.remaining_downloads,
    })
}

/// A login's body: a [`Secret`], since the password is in it.
#[must_use]
pub fn login_body(account: &Account) -> Secret {
    Secret::new(
        serde_json::json!({
            "username": account.user,
            "password": account.password.expose(),
        })
        .to_string(),
    )
}

/// Whether the key opens the API (`GET /infos/formats`, which needs nothing
/// else), asked when the person pastes it.
///
/// # Errors
///
/// [`Why::KeyRefused`], and what the network answers.
pub fn check(net: &dyn Net, key: &Secret) -> Result<(), Refusal> {
    answered(net, &request(key, None, &format!("{API}/infos/formats"))).map(drop)
}

/// Log in with the person's account.
///
/// # Errors
///
/// [`Why::KeyRefused`] (a wrong key, user or password), and what the network
/// answers.
pub fn login(net: &dyn Net, key: &Secret, account: &Account) -> Result<Session, Refusal> {
    let r = request(key, None, &format!("{API}/login"))
        .posting(Value::Secret(login_body(account)))
        .identity();
    parse_login(&answered(net, &r)?)
}

/// What the logged-in account allows today (`GET /infos/user`, with the
/// token): asked after a login, so the door says how many downloads are left
/// before the person spends one.
///
/// # Errors
///
/// [`Why::KeyRefused`] (a token the service no longer takes), and what the
/// network answers.
pub fn user(net: &dyn Net, key: &Secret, session: &Session) -> Result<Allowance, Refusal> {
    parse_user(&answered(
        net,
        &request(key, Some(session), &format!("{API}/infos/user")),
    )?)
}

/// What a search finds, in [`crate::found::rank`]'s order; an empty list is
/// an answer.
///
/// # Errors
///
/// [`Why::NothingToAsk`] (nothing is sent), and what the network answers.
pub fn search(
    net: &dyn Net,
    key: &Secret,
    session: Option<&Session>,
    search: &Search,
) -> Result<Vec<Found>, Refusal> {
    let path = search_path(search).map_err(|why| Refusal::new(Service::OpenSubtitles, why))?;
    let mut found = parse_search(&answered(net, &request(key, session, &path))?)?;
    crate::found::rank(&mut found);
    Ok(found)
}

/// Ask for a file's link — one of the day's downloads.
///
/// # Errors
///
/// [`Why::Quota`] when the day's downloads are spent (the service's 406, or
/// an answer with none left and no link), and what the network answers.
pub fn download(
    net: &dyn Net,
    key: &Secret,
    session: Option<&Session>,
    file_id: u64,
) -> Result<Download, Refusal> {
    let body = serde_json::json!({ "file_id": file_id }).to_string();
    let r = request(key, session, &format!("{API}/download"))
        .posting(Value::Plain(body))
        .identity();
    let answer = answered(net, &r)?;
    match parse_download(&answer) {
        Ok(d) => Ok(d),
        Err(_)
            if object::<RawDownload>(Service::OpenSubtitles, &answer)
                .is_ok_and(|d| d.remaining.is_some_and(|n| n <= 0)) =>
        {
            Err(Refusal::new(Service::OpenSubtitles, Why::Quota))
        }
        Err(e) => Err(e),
    }
}

/// The file a download's link points at. The link carries no key, and none
/// is sent with it.
///
/// # Errors
///
/// [`Why::TooLarge`] past [`FILE_MOST_BYTES`], and what the network answers.
pub fn fetch(net: &dyn Net, download: &Download) -> Result<Vec<u8>, Refusal> {
    let r = Request::get(Host::OpenSubtitles, download.link.clone())
        .accepting("*/*")
        .most(FILE_MOST_BYTES);
    answered(net, &r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ask::Method;
    use crate::canned::{Canned, header};

    fn key() -> Secret {
        Secret::new("0123456789abcdefABCDEF0123456789")
    }

    #[test]
    fn the_fingerprint_is_the_size_plus_both_ends_little_endian() {
        // The definition, by hand: every word of both ends, wrapping.
        let chunk = usize::try_from(CHUNK).unwrap();
        let zeros = vec![0u8; chunk];
        assert_eq!(moviehash_of(131_072, &zeros, &zeros), 131_072);
        let mut head = zeros.clone();
        head[0] = 1; // the first word is 1 (little-endian)
        let mut tail = zeros.clone();
        tail[8] = 2; // the second word of the tail is 2
        assert_eq!(moviehash_of(131_072, &head, &tail), 131_072 + 3);
        // A word's high byte is its LAST byte.
        let mut high = zeros.clone();
        high[7] = 0x80;
        assert_eq!(moviehash_of(0, &high, &zeros), 0x8000_0000_0000_0000);
        // And the sum wraps at 64 bits, as the reference's mask does.
        let ones = vec![0xFFu8; chunk];
        let words = u64::try_from(chunk / 8).unwrap();
        let expected = 7u64
            .wrapping_add(u64::MAX.wrapping_mul(words))
            .wrapping_add(u64::MAX.wrapping_mul(words));
        assert_eq!(moviehash_of(7, &ones, &ones), expected);
        assert_eq!(
            Moviehash {
                hash: 0x20000,
                size_bytes: 0
            }
            .hex(),
            "0000000000020000"
        );
    }

    /// One of OpenSubtitles' published vectors: the file `variable` names
    /// hashes to `hex`. A missing variable FAILS it; it never passes by
    /// having nothing to read.
    fn published(variable: &str, size: u64, hex: &str) {
        let path = std::env::var_os(variable)
            .unwrap_or_else(|| panic!("{variable} names no file: set it to run this"));
        let h = moviehash(Path::new(&path)).unwrap();
        assert_eq!(h.size_bytes, size, "{variable} is not the published file");
        assert_eq!(h.hex(), hex, "{variable}");
    }

    /// OpenSubtitles' published `breakdance.avi` (12 909 756 bytes), which
    /// is not in this tree: ignored here, and run by
    /// `moviehash-published-vectors` over a pinned copy
    /// (`nix/test-vectors.nix`) — or by hand with
    /// `RONOCTUA_OSHASH_BREAKDANCE=<file> cargo test -p ronoctua-lookup --
    /// --ignored`.
    #[test]
    #[ignore = "needs OpenSubtitles' published breakdance.avi"]
    fn the_breakdance_vector_holds() {
        published("RONOCTUA_OSHASH_BREAKDANCE", 12_909_756, "8e245d9679d31e12");
    }

    /// The other published vector, the `dummy.bin` inside `dummy.rar`
    /// (4 295 033 000 bytes): a file past 4 GiB, which no check downloads.
    /// By hand: `RONOCTUA_OSHASH_DUMMY=<file> cargo test -p ronoctua-lookup
    /// -- --ignored`.
    #[test]
    #[ignore = "needs OpenSubtitles' published dummy.bin, 4 GiB"]
    fn the_dummy_vector_holds() {
        published("RONOCTUA_OSHASH_DUMMY", 4_295_033_000, "61f7751fc2a72bfb");
    }

    #[test]
    fn a_file_is_fingerprinted_by_its_two_ends() {
        let dir = std::env::temp_dir().join(format!("ronoctua-lookup-hash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("film.bin");
        // 200 000 bytes: byte i is i % 251, so the two ends differ and overlap
        // nothing.
        let bytes: Vec<u8> = (0..200_000u32)
            .map(|i| u8::try_from(i % 251).unwrap())
            .collect();
        std::fs::write(&path, &bytes).unwrap();
        let chunk = usize::try_from(CHUNK).unwrap();
        let h = moviehash(&path).unwrap();
        assert_eq!(h.size_bytes, 200_000);
        assert_eq!(
            h.hash,
            moviehash_of(200_000, &bytes[..chunk], &bytes[bytes.len() - chunk..])
        );
        std::fs::write(&path, vec![0u8; 131_071]).unwrap();
        assert_eq!(moviehash(&path), Err(Why::TooSmall));
        assert_eq!(moviehash(&dir.join("nothing")), Err(Why::Unreadable));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_search_is_sorted_lowercase_and_carries_the_key_in_a_header() {
        let search = Search {
            moviehash: Some(Moviehash {
                hash: 0x8e24_5d96_79d3_1e12,
                size_bytes: 1,
            }),
            query: Some("Breaking Bad".into()),
            season: Some(1),
            episode: Some(2),
            languages: vec!["pt-BR".into(), "en".into(), "en".into()],
            ..Search::default()
        };
        let path = search_path(&search).unwrap();
        assert_eq!(
            path,
            "/api/v1/subtitles?episode_number=2&languages=en,pt-br&moviehash=8e245d9679d31e12\
             &query=breaking%20bad&season_number=1"
        );
        let url = format!("https://api.opensubtitles.com{path}");
        let body = br#"{"total_count":2,"data":[
          {"id":"1","type":"subtitle","attributes":{"language":"en","download_count":10,
            "hearing_impaired":false,"fps":23.976,"release":"Breaking.Bad.S01E02.720p",
            "moviehash_match":false,"ai_translated":false,"machine_translated":false,
            "feature_details":{"title":"Cat's in the Bag...","parent_title":"Breaking Bad",
              "year":2008,"season_number":1,"episode_number":2},
            "files":[{"file_id":111,"file_name":"a.srt"}]}},
          {"id":"2","type":"subtitle","attributes":{"language":"en","download_count":3,
            "hearing_impaired":true,"moviehash_match":true,"release":"x",
            "files":[{"file_id":222,"cd_number":1,"file_name":"b1.srt"},{"file_id":0}]}}
        ]}"#;
        let net = Canned::new(&[(url.as_str(), 200, body)]);
        let found = super::search(&net, &key(), None, &search).unwrap();
        assert_eq!(found.len(), 2, "a file with no id is dropped");
        assert_eq!(
            found[0].take,
            Take::OpenSubtitles { file_id: 222 },
            "the fingerprint match first"
        );
        assert!(found[0].hash_match && found[0].hearing_impaired);
        assert_eq!(found[1].title.as_deref(), Some("Breaking Bad"));
        assert_eq!(found[1].fps, Some(23.976));
        let asked = net.only();
        assert_eq!(header(&asked, "Api-Key"), Some(key().expose()));
        assert!(!asked.exposed_url().contains(key().expose()));
        assert_eq!(
            search_path(&Search::default()),
            Err(Why::NothingToAsk),
            "no fingerprint, no words, no id"
        );
    }

    #[test]
    fn a_login_is_a_secret_post_and_its_token_rides_after_it() {
        let account = Account {
            user: "someone".into(),
            password: Secret::new("hunter2"),
        };
        let login_url = "https://api.opensubtitles.com/api/v1/login";
        let answer = br#"{"user":{"allowed_downloads":100,"level":"Sub leecher"},
            "base_url":"vip-api.opensubtitles.com","token":"eyJtoken","status":200}"#;
        let net = Canned::new(&[(login_url, 200, answer)]);
        let session = login(&net, &key(), &account).unwrap();
        assert_eq!(session.host, "vip-api.opensubtitles.com");
        assert_eq!(session.allowed_downloads, Some(100));
        let asked = net.only();
        assert_eq!(asked.method, Method::Post);
        assert!(
            matches!(asked.body, Some(Value::Secret(_))),
            "the password is a secret"
        );
        assert!(!format!("{asked:?}").contains("hunter2"));
        let sent: serde_json::Value =
            serde_json::from_str(asked.body.as_ref().unwrap().expose()).unwrap();
        assert_eq!(sent["username"], "someone");
        assert_eq!(sent["password"], "hunter2");

        // THE HOST A REPLY NAMES gets the key only when it is theirs.
        assert_eq!(api_host(Some("evil.example")), "api.opensubtitles.com");
        assert_eq!(
            api_host(Some("opensubtitles.com.evil.example")),
            "api.opensubtitles.com"
        );
        assert_eq!(api_host(None), "api.opensubtitles.com");

        // The token rides on the next request, to the account's host.
        let url = "https://vip-api.opensubtitles.com/api/v1/download";
        let net = Canned::new(&[(
            url,
            200,
            br#"{"link":"https://www.opensubtitles.com/download/abc/x.srt","file_name":"x.srt",
                "requests":3,"remaining":97,"reset_time_utc":"2026-09-29T00:00:00.000Z"}"#,
        )]);
        let d = download(&net, &key(), Some(&session), 111).unwrap();
        assert_eq!(d.remaining, Some(97));
        let asked = net.only();
        assert_eq!(header(&asked, "Authorization"), Some("Bearer eyJtoken"));
        assert_eq!(
            asked.body.as_ref().map(Value::expose),
            Some(r#"{"file_id":111}"#)
        );
    }

    /// FAILS IF: the day's allowance stops being asked with the token, to
    /// the account's own host — or an answer without it passes for one.
    #[test]
    fn the_days_allowance_is_asked_with_the_token() {
        let session = Session {
            token: Secret::new("eyJtoken"),
            host: "vip-api.opensubtitles.com".into(),
            allowed_downloads: Some(20),
        };
        let url = "https://vip-api.opensubtitles.com/api/v1/infos/user";
        let net = Canned::new(&[(
            url,
            200,
            br#"{"data":{"allowed_downloads":20,"level":"Sub leecher","user_id":1,
                "ext_installed":false,"vip":false,"downloads_count":3,"remaining_downloads":17}}"#,
        )]);
        let a = user(&net, &key(), &session).unwrap();
        assert_eq!(
            a,
            Allowance {
                allowed_downloads: Some(20),
                remaining_downloads: Some(17)
            }
        );
        let asked = net.only();
        assert_eq!(header(&asked, "Authorization"), Some("Bearer eyJtoken"));
        assert_eq!(header(&asked, "Api-Key"), Some(key().expose()));
        assert_eq!(parse_user(b"{}").unwrap_err().why, Why::Malformed);
        assert_eq!(parse_user(b"[]").unwrap_err().why, Why::Malformed);
    }

    #[test]
    fn a_spent_day_is_the_quota_and_the_file_goes_without_the_key() {
        let url = "https://api.opensubtitles.com/api/v1/download";
        let net = Canned::new(&[(
            url,
            406,
            br#"{"message":"You have downloaded your allowed 5 subtitles"}"#,
        )]);
        assert_eq!(
            download(&net, &key(), None, 1).unwrap_err(),
            Refusal::new(Service::OpenSubtitles, Why::Quota)
        );
        let net = Canned::new(&[(url, 200, br#"{"remaining":0,"message":"no more"}"#)]);
        assert_eq!(download(&net, &key(), None, 1).unwrap_err().why, Why::Quota);

        let link = "https://www.opensubtitles.com/download/abc/x.srt";
        let net = Canned::new(&[(link, 200, b"1\n00:00:01,000 --> 00:00:02,000\nhi\n")]);
        let d = Download {
            link: link.into(),
            file_name: None,
            remaining: None,
            reset: None,
        };
        assert!(fetch(&net, &d).unwrap().starts_with(b"1\n"));
        let asked = net.only();
        assert!(
            asked.headers.is_empty(),
            "no key goes to the link: {asked:?}"
        );
        assert_eq!(asked.most_bytes, FILE_MOST_BYTES);
        assert_eq!(
            parse_download(br#"{"link":"http://plain.example/x"}"#)
                .unwrap_err()
                .why,
            Why::Malformed
        );
    }
}
