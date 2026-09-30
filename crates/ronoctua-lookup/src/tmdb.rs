//! A film's or an episode's synopsis and poster from TMDB, with the
//! person's token.
//!
//! # The token goes in a header, never in the address
//!
//! TMDB takes a v3 `api_key` ONLY in the query string, where it would be in
//! every log and every error that names the address; and a v4 "API read
//! access token" as `Authorization: Bearer`, which works on the same v3
//! endpoints. So this takes the token and nothing else: a v3 key pasted where
//! the token goes is [`Why::WrongKey`] ([`shape`]), said at once, and no
//! address built here has `api_key` in it.
//!
//! # What it answers
//!
//! A search (`search/movie`, `search/tv`), a film with its credits
//! (`movie/<id>?append_to_response=credits`), a series (`tv/<id>`) and an
//! episode (`tv/<id>/season/<s>/episode/<e>`, whose own `crew` names its
//! director). The poster is `image.tmdb.org/t/p/w<width><path>`.

use ronoctua_tags::Field;
use serde::{Deserialize, Serialize};

use crate::ask::{Net, Request, answered, object, present};
use crate::keys::Secret;
use crate::query::{escape, language_tag};
use crate::suggestion::{Cover, Suggestion, joined};
use crate::{Host, Refusal, Service, Why};

/// The poster widths TMDB serves.
pub const POSTER_WIDTHS: [u32; 6] = [92, 154, 185, 342, 500, 780];

/// Film or series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A film.
    Movie,
    /// A series.
    Tv,
}

impl Kind {
    const fn path(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Tv => "tv",
        }
    }
}

/// One result of a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    /// Its id.
    pub id: u64,
    /// Film or series.
    pub kind: Kind,
    /// Its title in the language asked.
    pub title: String,
    /// Its original title.
    pub original_title: Option<String>,
    /// Its release (or first air) date.
    pub date: Option<String>,
    /// Its synopsis.
    pub overview: Option<String>,
    /// Its poster's path (`/abc.jpg`).
    pub poster: Option<String>,
}

/// A film's, a series' or an episode's facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Details {
    /// The film's, the series' or the episode's id.
    pub id: u64,
    /// Film or series (an episode is `Tv`).
    pub kind: Kind,
    /// The film's title, or the episode's.
    pub title: String,
    /// The series, for an episode.
    pub show: Option<String>,
    /// The season, for an episode.
    pub season: Option<u16>,
    /// The episode's number.
    pub episode: Option<u16>,
    /// Its date.
    pub date: Option<String>,
    /// Its synopsis.
    pub overview: Option<String>,
    /// Its genres.
    pub genres: Vec<String>,
    /// Its directors.
    pub directors: Vec<String>,
    /// Its original language (ISO 639-1).
    pub language: Option<String>,
    /// Its poster's path (an episode's still when it has no poster).
    pub poster: Option<String>,
}

/// Whether `token` has the shape of a read access token, and not a v3 key.
///
/// A v3 key is thirty-two hex digits; a read token is a JWT, three dotted
/// parts. Pure.
///
/// # Errors
///
/// [`Why::WrongKey`] for a v3 key, [`Why::NoKey`] for nothing.
pub fn shape(token: &Secret) -> Result<(), Why> {
    let t = token.expose().trim();
    if t.is_empty() {
        return Err(Why::NoKey);
    }
    let v3 = t.len() == 32 && t.bytes().all(|b| b.is_ascii_hexdigit());
    if v3 || t.split('.').count() != 3 {
        return Err(Why::WrongKey);
    }
    Ok(())
}

/// A GET of `path` with the token as `Bearer`.
fn request(token: &Secret, path: &str) -> Request {
    Request::get(Host::Tmdb, format!("{}/3{path}", Host::Tmdb.origin())).secret_header(
        "Authorization",
        Secret::new(format!("Bearer {}", token.expose().trim())),
    )
}

/// `&language=<tag>`, or nothing for a language that is not a tag.
fn language_part(language: &str) -> String {
    language_tag(language).map_or_else(String::new, |l| format!("&language={l}"))
}

/// A search's path (after `/3`).
///
/// # Errors
///
/// [`Why::NothingToAsk`] for no words.
pub fn search_path(
    kind: Kind,
    words: &str,
    year: Option<u16>,
    language: &str,
) -> Result<String, Why> {
    let words = present(Some(words)).ok_or(Why::NothingToAsk)?;
    let mut out = format!(
        "/search/{}?query={}&include_adult=false{}",
        kind.path(),
        escape(&words),
        language_part(language)
    );
    if let Some(y) = year {
        let name = match kind {
            Kind::Movie => "year",
            Kind::Tv => "first_air_date_year",
        };
        out.push_str(&format!("&{name}={y}"));
    }
    Ok(out)
}

/// A film's path, its credits appended.
#[must_use]
pub fn movie_path(id: u64, language: &str) -> String {
    format!(
        "/movie/{id}?append_to_response=credits{}",
        language_part(language)
    )
}

/// `?language=<tag>`, or nothing.
fn language_query(language: &str) -> String {
    language_tag(language).map_or_else(String::new, |l| format!("?language={l}"))
}

/// A series' path.
#[must_use]
pub fn tv_path(id: u64, language: &str) -> String {
    format!("/tv/{id}{}", language_query(language))
}

/// An episode's path.
#[must_use]
pub fn episode_path(tv: u64, season: u16, episode: u16, language: &str) -> String {
    format!(
        "/tv/{tv}/season/{season}/episode/{episode}{}",
        language_query(language)
    )
}

/// A poster's address at the served width nearest above `width`; `None` for
/// a path that is not one (`/` and a file name). Pure.
#[must_use]
pub fn poster_url(path: &str, width: u32) -> Option<String> {
    let name = path.strip_prefix('/')?;
    let fine = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !name.contains("..");
    if !fine {
        return None;
    }
    let w = POSTER_WIDTHS
        .iter()
        .copied()
        .find(|w| *w >= width)
        .unwrap_or(POSTER_WIDTHS[POSTER_WIDTHS.len() - 1]);
    Some(format!("{}/t/p/w{w}/{name}", Host::TmdbImages.origin()))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawHit {
    id: u64,
    title: Option<String>,
    name: Option<String>,
    original_title: Option<String>,
    original_name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSearch {
    results: Vec<RawHit>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawNamed {
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawCrew {
    job: String,
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawCredits {
    crew: Vec<RawCrew>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawDetails {
    id: u64,
    title: Option<String>,
    name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    air_date: Option<String>,
    overview: Option<String>,
    genres: Vec<RawNamed>,
    original_language: Option<String>,
    poster_path: Option<String>,
    still_path: Option<String>,
    season_number: Option<u16>,
    episode_number: Option<u16>,
    crew: Vec<RawCrew>,
    credits: Option<RawCredits>,
}

/// A search answer. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_search(kind: Kind, body: &[u8]) -> Result<Vec<Hit>, Refusal> {
    let raw: RawSearch = object(Service::Tmdb, body)?;
    Ok(raw
        .results
        .into_iter()
        .filter(|h| h.id > 0)
        .map(|h| Hit {
            id: h.id,
            kind,
            title: present(h.title.as_deref().or(h.name.as_deref())).unwrap_or_default(),
            original_title: present(h.original_title.as_deref().or(h.original_name.as_deref())),
            date: present(h.release_date.as_deref().or(h.first_air_date.as_deref())),
            overview: present(h.overview.as_deref()),
            poster: present(h.poster_path.as_deref()),
        })
        .collect())
}

/// A film's, a series' or an episode's answer. For an episode, `show` is the
/// series' name (the episode's answer does not carry it). Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_details(kind: Kind, body: &[u8], show: Option<&str>) -> Result<Details, Refusal> {
    let raw: RawDetails = object(Service::Tmdb, body)?;
    if raw.id == 0 {
        return Err(Refusal::new(Service::Tmdb, Why::Malformed));
    }
    let mut directors: Vec<String> = Vec::new();
    let credited = raw.credits.as_ref().map_or(&[][..], |c| &c.crew[..]);
    for c in raw.crew.iter().chain(credited) {
        if c.job == "Director" && !c.name.trim().is_empty() && !directors.contains(&c.name) {
            directors.push(c.name.clone());
        }
    }
    Ok(Details {
        id: raw.id,
        kind,
        title: present(raw.title.as_deref().or(raw.name.as_deref())).unwrap_or_default(),
        show: present(show),
        season: raw.season_number,
        episode: raw.episode_number,
        date: present(
            raw.release_date
                .as_deref()
                .or(raw.air_date.as_deref())
                .or(raw.first_air_date.as_deref()),
        ),
        overview: present(raw.overview.as_deref()),
        genres: raw
            .genres
            .into_iter()
            .map(|g| g.name)
            .filter(|n| !n.trim().is_empty())
            .collect(),
        directors,
        language: present(raw.original_language.as_deref()),
        poster: present(raw.poster_path.as_deref().or(raw.still_path.as_deref())),
    })
}

/// The candidate a film or an episode makes, with its poster at
/// `poster_width` pixels.
#[must_use]
pub fn suggestion(details: &Details, poster_width: u32) -> Suggestion {
    let mut s = Suggestion::new(Service::Tmdb, details.id.to_string());
    s.put(Field::Title, Some(details.title.clone()));
    s.put(Field::Show, details.show.clone());
    s.put(Field::Season, details.season.map(|n| n.to_string()));
    s.put(Field::Episode, details.episode.map(|n| n.to_string()));
    s.put(Field::Date, details.date.clone());
    s.put(Field::Genre, joined(&details.genres));
    s.put(Field::Director, joined(&details.directors));
    s.put(Field::Description, details.overview.clone());
    s.cover = details
        .poster
        .as_deref()
        .and_then(|p| poster_url(p, poster_width))
        .map(|url| Cover::Address {
            host: Host::TmdbImages,
            url,
        });
    s
}

/// Whether the token opens TMDB (`/3/authentication`), asked when the person
/// pastes it.
///
/// # Errors
///
/// [`Why::WrongKey`] / [`Why::NoKey`] for its shape (nothing is sent),
/// [`Why::KeyRefused`], and what the network answers.
pub fn check(net: &dyn Net, token: &Secret) -> Result<(), Refusal> {
    shape(token).map_err(|why| Refusal::new(Service::Tmdb, why))?;
    answered(net, &request(token, "/authentication")).map(drop)
}

/// What a search finds; an empty list is an answer.
///
/// # Errors
///
/// As [`check`], [`Why::NothingToAsk`], and what the network answers.
pub fn search(
    net: &dyn Net,
    token: &Secret,
    kind: Kind,
    words: &str,
    year: Option<u16>,
    language: &str,
) -> Result<Vec<Hit>, Refusal> {
    let refused = |why| Refusal::new(Service::Tmdb, why);
    shape(token).map_err(refused)?;
    let path = search_path(kind, words, year, language).map_err(refused)?;
    parse_search(kind, &answered(net, &request(token, &path))?)
}

/// A film, with its directors.
///
/// # Errors
///
/// As [`check`], and what the network answers.
pub fn movie(net: &dyn Net, token: &Secret, id: u64, language: &str) -> Result<Details, Refusal> {
    shape(token).map_err(|why| Refusal::new(Service::Tmdb, why))?;
    let body = answered(net, &request(token, &movie_path(id, language)))?;
    parse_details(Kind::Movie, &body, None)
}

/// An episode, with its series' name.
///
/// # Errors
///
/// As [`check`], and what the network answers.
pub fn episode(
    net: &dyn Net,
    token: &Secret,
    tv: u64,
    season: u16,
    number: u16,
    language: &str,
) -> Result<Details, Refusal> {
    shape(token).map_err(|why| Refusal::new(Service::Tmdb, why))?;
    let series = parse_details(
        Kind::Tv,
        &answered(net, &request(token, &tv_path(tv, language)))?,
        None,
    )?;
    let body = answered(
        net,
        &request(token, &episode_path(tv, season, number, language)),
    )?;
    let mut details = parse_details(Kind::Tv, &body, Some(&series.title))?;
    if details.genres.is_empty() {
        details.genres = series.genres;
    }
    if details.poster.is_none() {
        details.poster = series.poster;
    }
    Ok(details)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::{Canned, header};

    fn token() -> Secret {
        Secret::new("eyJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJ4In0.c2lnbmF0dXJl")
    }

    #[test]
    fn the_token_is_a_bearer_header_and_never_in_the_address() {
        let url = "https://api.themoviedb.org/3/authentication";
        let net = Canned::new(&[(url, 200, br#"{"success":true}"#)]);
        check(&net, &token()).unwrap();
        let asked = net.only();
        assert_eq!(
            header(&asked, "Authorization"),
            Some("Bearer eyJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJ4In0.c2lnbmF0dXJl")
        );
        assert!(!asked.url.contains("api_key"), "{}", asked.url);
        assert!(
            !asked.exposed_url().contains("eyJ"),
            "{}",
            asked.exposed_url()
        );
        assert!(
            !format!("{asked:?}").contains("eyJhbGci"),
            "a log line never shows it"
        );
    }

    #[test]
    fn a_v3_key_is_the_wrong_key_and_nothing_is_sent() {
        let net = Canned::new(&[]);
        let v3 = Secret::new("0123456789abcdef0123456789abcdef");
        assert_eq!(
            check(&net, &v3).unwrap_err(),
            Refusal::new(Service::Tmdb, Why::WrongKey)
        );
        assert_eq!(shape(&Secret::new(" ")), Err(Why::NoKey));
        assert!(net.asked().is_empty());
        let url = "https://api.themoviedb.org/3/authentication";
        let net = Canned::new(&[(url, 401, br#"{"status_code":7}"#)]);
        assert_eq!(check(&net, &token()).unwrap_err().why, Why::KeyRefused);
    }

    #[test]
    fn a_search_answer_gives_films_with_their_poster() {
        let path = search_path(Kind::Movie, "the matrix", Some(1999), "pt-BR").unwrap();
        assert_eq!(
            path,
            "/search/movie?query=the%20matrix&include_adult=false&language=pt-br&year=1999"
        );
        let url = format!("https://api.themoviedb.org/3{path}");
        let body = br#"{"page":1,"results":[
            {"id":603,"title":"Matrix","original_title":"The Matrix","release_date":"1999-03-30",
             "overview":"Um hacker...","poster_path":"/p.jpg"},
            {"id":0,"title":"broken"}]}"#;
        let net = Canned::new(&[(url.as_str(), 200, body)]);
        let hits = search(
            &net,
            &token(),
            Kind::Movie,
            "the matrix",
            Some(1999),
            "pt-BR",
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].original_title.as_deref(), Some("The Matrix"));
        assert_eq!(
            poster_url(hits[0].poster.as_deref().unwrap(), 400).as_deref(),
            Some("https://image.tmdb.org/t/p/w500/p.jpg")
        );
        assert_eq!(poster_url("/../x", 500), None);
        assert_eq!(poster_url("p.jpg", 500), None);
        assert_eq!(tv_path(1, ""), "/tv/1", "no language, no query");
        assert_eq!(
            search_path(Kind::Tv, "x", Some(2019), "en").unwrap(),
            "/search/tv?query=x&include_adult=false&language=en&first_air_date_year=2019"
        );
    }

    #[test]
    fn a_film_names_its_directors_and_suggests_its_synopsis() {
        let body = br#"{"id":603,"title":"The Matrix","release_date":"1999-03-30",
            "overview":"A hacker learns...","genres":[{"id":28,"name":"Action"},{"id":878,"name":"Science Fiction"}],
            "original_language":"en","poster_path":"/p.jpg",
            "credits":{"cast":[],"crew":[{"job":"Director","name":"Lana Wachowski"},
              {"job":"Director","name":"Lilly Wachowski"},{"job":"Producer","name":"Joel Silver"}]}}"#;
        let d = parse_details(Kind::Movie, body, None).unwrap();
        assert_eq!(d.directors, ["Lana Wachowski", "Lilly Wachowski"]);
        let s = suggestion(&d, 500);
        assert_eq!(s.value(Field::Description), Some("A hacker learns..."));
        assert_eq!(s.value(Field::Genre), Some("Action; Science Fiction"));
        assert_eq!(s.value(Field::Date), Some("1999-03-30"));
        assert_eq!(
            s.cover,
            Some(Cover::Address {
                host: Host::TmdbImages,
                url: "https://image.tmdb.org/t/p/w500/p.jpg".into()
            })
        );
    }

    #[test]
    fn an_episode_carries_its_series_name_and_numbers() {
        let tv = "https://api.themoviedb.org/3/tv/1399?language=en";
        let ep = "https://api.themoviedb.org/3/tv/1399/season/3/episode/9?language=en";
        let net = Canned::new(&[
            (
                tv,
                200,
                br#"{"id":1399,"name":"Game of Thrones","genres":[{"name":"Drama"}],"poster_path":"/s.jpg"}"#,
            ),
            (
                ep,
                200,
                br#"{"id":63099,"name":"The Rains of Castamere","air_date":"2013-06-02",
                    "season_number":3,"episode_number":9,"overview":"Robb...",
                    "crew":[{"job":"Director","name":"David Nutter"}],"still_path":null}"#,
            ),
        ]);
        let d = episode(&net, &token(), 1399, 3, 9, "en").unwrap();
        let s = suggestion(&d, 342);
        assert_eq!(s.value(Field::Show), Some("Game of Thrones"));
        assert_eq!(s.value(Field::Title), Some("The Rains of Castamere"));
        assert_eq!(s.value(Field::Season), Some("3"));
        assert_eq!(s.value(Field::Episode), Some("9"));
        assert_eq!(s.value(Field::Director), Some("David Nutter"));
        assert_eq!(s.value(Field::Genre), Some("Drama"), "the series' genres");
        assert!(s.cover.is_some(), "the series' poster");
        assert!(
            net.asked()
                .iter()
                .all(|r| header(r, "Authorization").is_some())
        );
    }
}
