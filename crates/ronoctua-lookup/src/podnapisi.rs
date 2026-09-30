//! Subtitles from Podnapisi, keyless — ASLEEP.
//!
//! `www.podnapisi.net` has answered NXDOMAIN since at least June 2026 (checked
//! through DNS over HTTPS on 2026-09-28), so no keyless subtitle source is
//! known to work today. The reader stays: when the domain comes back, waking
//! it is [`AWAKE`] and a check, not a new module. Until then every ask is
//! [`Why::Dormant`] and nothing is sent.
//!
//! The site never documented its JSON: the shape below is what its advanced
//! search answered with `Accept: application/json` while it was up, written
//! from memory and UNVERIFIED — the first thing to confirm on waking it.

use serde::Deserialize;

use crate::ask::{Net, Request, answered, object, present};
use crate::found::{Found, Take, tongue};
use crate::query::{escape, language_tag};
use crate::subdl::{Member, subtitles_in};
use crate::{Host, Refusal, Service, Why};

/// Whether the reader is awake. `false` while the domain does not resolve.
pub const AWAKE: bool = false;

/// The most a zip may be.
pub const ZIP_MOST_BYTES: u64 = 16 << 20;

/// What a search asks with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Search {
    /// The title.
    pub keywords: String,
    /// The year.
    pub year: Option<u16>,
    /// The season.
    pub season: Option<u16>,
    /// The episode.
    pub episode: Option<u16>,
    /// The languages (`en`, `pt-br`).
    pub languages: Vec<String>,
}

/// The search's address.
///
/// # Errors
///
/// [`Why::NothingToAsk`] with no words.
pub fn search_url(search: &Search) -> Result<String, Why> {
    let words = present(Some(&search.keywords)).ok_or(Why::NothingToAsk)?;
    let mut url = format!(
        "{}/subtitles/search/advanced?keywords={}",
        Host::Podnapisi.origin(),
        escape(&words)
    );
    if let Some(y) = search.year {
        url.push_str(&format!("&year={y}"));
    }
    if let Some(s) = search.season {
        url.push_str(&format!("&seasons={s}"));
    }
    if let Some(e) = search.episode {
        url.push_str(&format!("&episodes={e}"));
    }
    for l in search.languages.iter().filter_map(|l| language_tag(l)) {
        url.push_str(&format!("&language={l}"));
    }
    Ok(url)
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawMovie {
    title: Option<String>,
    year: Option<u16>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSubtitle {
    id: String,
    language: Option<String>,
    release: Option<String>,
    downloads: Option<u64>,
    flags: Vec<String>,
    fps: Option<f64>,
    season: Option<u16>,
    episode: Option<u16>,
    movie: RawMovie,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSearch {
    data: Vec<RawSubtitle>,
}

/// Whether `id` could be one of its ids: letters and digits.
fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A search answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_search(body: &[u8]) -> Result<Vec<Found>, Refusal> {
    let raw: RawSearch = object(Service::Podnapisi, body)?;
    Ok(raw
        .data
        .into_iter()
        .filter(|s| is_id(&s.id))
        .map(|s| Found {
            source: Service::Podnapisi,
            language: tongue(s.language.as_deref().unwrap_or_default()),
            release: present(s.release.as_deref()).unwrap_or_default(),
            file_name: None,
            downloads: s.downloads,
            hearing_impaired: s.flags.iter().any(|f| f == "hearing_impaired"),
            fps: s.fps.filter(|f| f.is_finite() && *f > 0.0),
            hash_match: false,
            machine_translated: s.flags.iter().any(|f| f == "machine_translated"),
            title: present(s.movie.title.as_deref()),
            year: s.movie.year,
            season: s.season,
            episode: s.episode,
            take: Take::Podnapisi { id: s.id },
        })
        .collect())
}

/// What a search finds.
///
/// # Errors
///
/// [`Why::Dormant`] while asleep (nothing is sent); then
/// [`Why::NothingToAsk`] and what the network answers.
pub fn search(net: &dyn Net, search: &Search) -> Result<Vec<Found>, Refusal> {
    let refused = |why| Refusal::new(Service::Podnapisi, why);
    if Service::Podnapisi.is_dormant() {
        return Err(refused(Why::Dormant));
    }
    let url = search_url(search).map_err(refused)?;
    let mut found = parse_search(&answered(net, &Request::get(Host::Podnapisi, url))?)?;
    crate::found::rank(&mut found);
    Ok(found)
}

/// The subtitles of one result: its zip, fetched and opened.
///
/// # Errors
///
/// [`Why::Dormant`] while asleep (nothing is sent), [`Why::Malformed`] for an
/// id that is not one, and as [`crate::subdl::fetch`].
pub fn fetch(net: &dyn Net, id: &str) -> Result<Vec<Member>, Refusal> {
    let refused = |why| Refusal::new(Service::Podnapisi, why);
    if Service::Podnapisi.is_dormant() {
        return Err(refused(Why::Dormant));
    }
    if !is_id(id) {
        return Err(refused(Why::Malformed));
    }
    let url = format!("{}/subtitles/{id}/download", Host::Podnapisi.origin());
    let r = Request::get(Host::Podnapisi, url)
        .accepting("application/zip")
        .identity()
        .most(ZIP_MOST_BYTES);
    subtitles_in(&answered(net, &r)?).map_err(refused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    #[test]
    fn asleep_it_sends_nothing_and_says_so() {
        let net = Canned::new(&[]);
        let s = Search {
            keywords: "matrix".into(),
            ..Search::default()
        };
        assert_eq!(
            search(&net, &s).unwrap_err(),
            Refusal::new(Service::Podnapisi, Why::Dormant)
        );
        assert_eq!(fetch(&net, "abc").unwrap_err().why, Why::Dormant);
        assert!(net.asked().is_empty());
    }

    #[test]
    fn its_reader_is_ready_for_the_day_it_wakes() {
        let url = search_url(&Search {
            keywords: "The Matrix".into(),
            year: Some(1999),
            languages: vec!["en".into(), "pt-BR".into()],
            ..Search::default()
        })
        .unwrap();
        assert_eq!(
            url,
            "https://www.podnapisi.net/subtitles/search/advanced?keywords=The%20Matrix\
             &year=1999&language=en&language=pt-br"
        );
        let body = br#"{"data":[
            {"id":"aB3d","language":"en","release":"The.Matrix.1999.1080p","downloads":42,
             "flags":["hearing_impaired"],"fps":23.976,"movie":{"title":"The Matrix","year":1999}},
            {"id":"../x","language":"en"}]}"#;
        let found = parse_search(body).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].take, Take::Podnapisi { id: "aB3d".into() });
        assert!(found[0].hearing_impaired);
        assert_eq!(found[0].year, Some(1999));
    }
}
