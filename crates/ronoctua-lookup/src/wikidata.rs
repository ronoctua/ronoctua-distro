//! A film's or a series' facts from Wikidata, keyless.
//!
//! Two questions. `wbsearchentities` on `www.wikidata.org` turns words into
//! items (`Q83495`). One SPARQL query on `query.wikidata.org` then asks every
//! item at once for what a sheet holds: its title in the person's language,
//! its first release date (P577, with its precision), its directors (P57),
//! genres (P136), original language (P364), IMDb id (P345), TMDB id (P4947
//! for a film, P4983 for a series), and its poster (P3383) when Commons has
//! one — rarely, since most posters are not free.
//!
//! An item with several release dates, directors or genres would be one row
//! per combination; the query GROUPS by item and aggregates (`MIN` of the
//! dates, `GROUP_CONCAT` of the names, `SAMPLE` of the rest), so one item is
//! one row.
//!
//! No synopsis: Wikidata has none. The Candidate page says a synopsis and a
//! poster come with the person's TMDB token.
//!
//! Nothing a person typed goes into SPARQL as text: the items are checked
//! (`Q` and digits) and the languages are tags (letters and `-`).

use ronoctua_tags::Field;
use serde::{Deserialize, Serialize};

use crate::ask::{Net, Request, answered, object, present};
use crate::query::{escape, is_qid, language_tag};
use crate::suggestion::{Cover, Suggestion, joined};
use crate::{Host, Refusal, Service, Why};

/// How many items a search asks for, and a query asks about.
pub const LIMIT: usize = 10;

/// What joins several names inside one SPARQL cell: the unit separator, a
/// character no name has.
const SEPARATOR: char = '\u{1f}';

/// An item a search found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    /// The item (`Q83495`).
    pub id: String,
    /// Its label in the language asked.
    pub label: String,
    /// Its one-line description (`1999 film by the Wachowskis`).
    pub description: Option<String>,
}

/// What an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A film (an instance of film, Q11424, or of a subclass).
    Film,
    /// A television series (Q5398426, or a subclass).
    Series,
}

/// A film's or a series' facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Film {
    /// The item.
    pub id: String,
    /// Film or series.
    pub kind: Kind,
    /// Its title in the person's language (or the first language that has
    /// one).
    pub title: String,
    /// Its first release, as precise as Wikidata knows it: `YYYY`,
    /// `YYYY-MM` or `YYYY-MM-DD`.
    pub date: Option<String>,
    /// Its directors.
    pub directors: Vec<String>,
    /// Its genres.
    pub genres: Vec<String>,
    /// Its original language, by name.
    pub language: Option<String>,
    /// Its IMDb id (`tt0133093`).
    pub imdb: Option<String>,
    /// Its TMDB id.
    pub tmdb: Option<u64>,
    /// Its poster's address on Commons, when there is one.
    pub poster: Option<String>,
}

/// The search's address.
///
/// # Errors
///
/// [`Why::NothingToAsk`] for no words, [`Why::Malformed`] for a language
/// that is not a tag.
pub fn search_url(words: &str, language: &str) -> Result<String, Why> {
    let words = present(Some(words)).ok_or(Why::NothingToAsk)?;
    let language = language_tag(language).ok_or(Why::Malformed)?;
    Ok(format!(
        "{}/w/api.php?action=wbsearchentities&search={}&language={language}&uselang={language}&type=item&limit={LIMIT}&format=json",
        Host::Wikidata.origin(),
        escape(&words)
    ))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawHit {
    id: String,
    label: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSearch {
    search: Vec<RawHit>,
}

/// The items of a search answer, at most [`LIMIT`]. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_search(body: &[u8]) -> Result<Vec<Hit>, Refusal> {
    let raw: RawSearch = object(Service::Wikidata, body)?;
    Ok(raw
        .search
        .into_iter()
        .filter(|h| is_qid(&h.id))
        .take(LIMIT)
        .map(|h| Hit {
            label: present(h.label.as_deref()).unwrap_or_else(|| h.id.clone()),
            description: present(h.description.as_deref()),
            id: h.id,
        })
        .collect())
}

/// The SPARQL query for `ids`, labels in `languages` (the first that has one
/// wins; English is always the last).
///
/// # Errors
///
/// [`Why::NothingToAsk`] for no items, [`Why::Malformed`] for an item that
/// is not one or a language that is not a tag.
pub fn sparql(ids: &[String], languages: &[String]) -> Result<String, Why> {
    if ids.is_empty() {
        return Err(Why::NothingToAsk);
    }
    if !ids.iter().all(|id| is_qid(id)) || ids.len() > LIMIT {
        return Err(Why::Malformed);
    }
    let mut tags: Vec<String> = Vec::new();
    for l in languages {
        let tag = language_tag(l).ok_or(Why::Malformed)?;
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    if !tags.iter().any(|t| t == "en") {
        tags.push("en".to_owned());
    }
    let values: Vec<String> = ids.iter().map(|id| format!("wd:{id}")).collect();
    Ok(format!(
        r#"SELECT ?item ?title (SAMPLE(?kindOf) AS ?kind)
  (MIN(CONCAT(STR(?dateValue), "|", STR(?precision))) AS ?date)
  (GROUP_CONCAT(DISTINCT ?directorName; separator="\u001F") AS ?directors)
  (GROUP_CONCAT(DISTINCT ?genreName; separator="\u001F") AS ?genres)
  (SAMPLE(?languageName) AS ?language) (SAMPLE(?imdbId) AS ?imdb)
  (SAMPLE(COALESCE(?tmdbFilm, ?tmdbSeries)) AS ?tmdb) (SAMPLE(?posterFile) AS ?poster)
WHERE {{
  VALUES ?item {{ {values} }}
  BIND(IF(EXISTS {{ ?item wdt:P31/wdt:P279* wd:Q5398426 }}, "series",
       IF(EXISTS {{ ?item wdt:P31/wdt:P279* wd:Q11424 }}, "film", "other")) AS ?kindOf)
  OPTIONAL {{ ?item p:P577/psv:P577 ?dateNode .
             ?dateNode wikibase:timeValue ?dateValue ; wikibase:timePrecision ?precision . }}
  OPTIONAL {{ ?item wdt:P57 ?director . }}
  OPTIONAL {{ ?item wdt:P136 ?genre . }}
  OPTIONAL {{ ?item wdt:P364 ?originalLanguage . }}
  OPTIONAL {{ ?item wdt:P345 ?imdbId . }}
  OPTIONAL {{ ?item wdt:P4947 ?tmdbFilm . }}
  OPTIONAL {{ ?item wdt:P4983 ?tmdbSeries . }}
  OPTIONAL {{ ?item wdt:P3383 ?posterFile . }}
  SERVICE wikibase:label {{
    bd:serviceParam wikibase:language "{languages}" .
    ?item rdfs:label ?title .
    ?director rdfs:label ?directorName .
    ?genre rdfs:label ?genreName .
    ?originalLanguage rdfs:label ?languageName .
  }}
}}
GROUP BY ?item ?title"#,
        values = values.join(" "),
        languages = tags.join(","),
    ))
}

/// The query's address.
///
/// # Errors
///
/// As [`sparql`].
pub fn films_url(ids: &[String], languages: &[String]) -> Result<String, Why> {
    Ok(format!(
        "{}/sparql?format=json&query={}",
        Host::WikidataQuery.origin(),
        escape(&sparql(ids, languages)?)
    ))
}

/// One SPARQL cell.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Cell {
    value: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawResults {
    bindings: Vec<std::collections::HashMap<String, Cell>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSparql {
    results: RawResults,
}

/// `1999-03-31T00:00:00Z|11` as far as its precision goes: 11 is the day, 10
/// the month, 9 the year. A precision coarser than a year (a decade) is no
/// date for a tag. Pure.
#[must_use]
pub fn date_of(cell: &str) -> Option<String> {
    let (value, precision) = cell.split_once('|')?;
    // A year before 1000 or with a sign is not a film's.
    let date = value.strip_prefix('+').unwrap_or(value);
    let take = match precision.trim().parse::<u8>().ok()? {
        11.. => 10,
        10 => 7,
        9 => 4,
        _ => return None,
    };
    let cut = date.get(..take)?;
    let shaped = cut.bytes().enumerate().all(|(i, b)| {
        if i == 4 || i == 7 {
            b == b'-'
        } else {
            b.is_ascii_digit()
        }
    });
    shaped.then(|| cut.to_owned())
}

/// The films and series of a query answer, in the order of `ids`; an item
/// that is neither is dropped. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when it is not one.
pub fn parse_films(body: &[u8], ids: &[String]) -> Result<Vec<Film>, Refusal> {
    let raw: RawSparql = object(Service::Wikidata, body)?;
    let mut films: Vec<Film> = Vec::new();
    for row in raw.results.bindings {
        let cell = |name: &str| row.get(name).map(|c| c.value.as_str());
        let Some(id) = cell("item").and_then(|u| u.rsplit('/').next()) else {
            continue;
        };
        if !is_qid(id) || films.iter().any(|f| f.id == id) {
            continue;
        }
        let kind = match cell("kind") {
            Some("film") => Kind::Film,
            Some("series") => Kind::Series,
            _ => continue,
        };
        let names = |name: &str| -> Vec<String> {
            cell(name)
                .unwrap_or_default()
                .split(SEPARATOR)
                .map(str::trim)
                // An unlabelled item comes back as its own address.
                .filter(|n| !n.is_empty() && !n.starts_with("http"))
                .map(str::to_owned)
                .collect()
        };
        films.push(Film {
            id: id.to_owned(),
            kind,
            title: present(cell("title")).unwrap_or_else(|| id.to_owned()),
            date: cell("date").and_then(date_of),
            directors: names("directors"),
            genres: names("genres"),
            language: present(cell("language")).filter(|l| !l.starts_with("http")),
            imdb: present(cell("imdb")).filter(|i| {
                i.starts_with("tt") && i.len() > 2 && i[2..].bytes().all(|b| b.is_ascii_digit())
            }),
            tmdb: cell("tmdb").and_then(|t| t.trim().parse().ok()),
            poster: present(cell("poster"))
                .filter(|p| crate::picture::address_on(Host::Commons, p).is_some()),
        });
    }
    films.sort_by_key(|f| ids.iter().position(|i| *i == f.id).unwrap_or(usize::MAX));
    Ok(films)
}

/// A poster's address at `width` pixels: Commons' `Special:FilePath` scales
/// on the server, so a 20 MB scan never crosses the network. `None` for an
/// address that is not on Commons. Pure.
#[must_use]
pub fn poster_url(poster: &str, width: u32) -> Option<String> {
    let https = crate::picture::address_on(Host::Commons, poster)?;
    let join = if https.contains('?') { '&' } else { '?' };
    Some(format!("{https}{join}width={width}"))
}

/// The candidate a film makes: the fields a sheet would write, and its
/// poster when Commons has one (at `poster_width` pixels).
#[must_use]
pub fn suggestion(film: &Film, poster_width: u32) -> Suggestion {
    let mut s = Suggestion::new(Service::Wikidata, film.id.clone());
    s.put(
        if film.kind == Kind::Series {
            Field::Show
        } else {
            Field::Title
        },
        Some(film.title.clone()),
    );
    s.put(Field::Date, film.date.clone());
    s.put(Field::Director, joined(&film.directors));
    s.put(Field::Genre, joined(&film.genres));
    s.cover = film
        .poster
        .as_deref()
        .and_then(|p| poster_url(p, poster_width))
        .map(|url| Cover::Address {
            host: Host::Commons,
            url,
        });
    s
}

/// The items a search finds; an empty list is an answer.
///
/// # Errors
///
/// [`Why::NothingToAsk`] / [`Why::Malformed`] (nothing is sent), and what the
/// network answers.
pub fn search(net: &dyn Net, words: &str, language: &str) -> Result<Vec<Hit>, Refusal> {
    let url = search_url(words, language).map_err(|why| Refusal::new(Service::Wikidata, why))?;
    parse_search(&answered(net, &Request::get(Host::Wikidata, url))?)
}

/// The films and series among `ids`.
///
/// # Errors
///
/// As [`sparql`] (nothing is sent), and what the network answers.
pub fn films(net: &dyn Net, ids: &[String], languages: &[String]) -> Result<Vec<Film>, Refusal> {
    let url = films_url(ids, languages).map_err(|why| Refusal::new(Service::Wikidata, why))?;
    let request =
        Request::get(Host::WikidataQuery, url).accepting("application/sparql-results+json");
    parse_films(&answered(net, &request)?, ids)
}

/// Words to films: the search, then one query about what it found. An empty
/// search is an empty answer, and the query is not sent.
///
/// # Errors
///
/// As [`search`] and [`films`].
pub fn find(net: &dyn Net, words: &str, languages: &[String]) -> Result<Vec<Film>, Refusal> {
    let first = languages.first().map_or("en", String::as_str);
    let hits = search(net, words, first)?;
    if hits.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<String> = hits.into_iter().map(|h| h.id).collect();
    films(net, &ids, languages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_search_names_its_language_and_escapes_its_words() {
        assert_eq!(
            search_url("the matrix", "pt-BR").unwrap(),
            "https://www.wikidata.org/w/api.php?action=wbsearchentities&search=the%20matrix\
             &language=pt-br&uselang=pt-br&type=item&limit=10&format=json"
        );
        assert_eq!(search_url(" ", "en"), Err(Why::NothingToAsk));
        assert_eq!(search_url("x", "en\"x"), Err(Why::Malformed));
    }

    #[test]
    fn a_search_answer_gives_items() {
        let body = br#"{"search":[
            {"id":"Q83495","label":"The Matrix","description":"1999 film"},
            {"id":"L1","label":"a lexeme"},
            {"id":"Q2","description":"no label"}]}"#;
        let hits = parse_search(body).unwrap();
        assert_eq!(
            hits,
            [
                Hit {
                    id: "Q83495".into(),
                    label: "The Matrix".into(),
                    description: Some("1999 film".into())
                },
                Hit {
                    id: "Q2".into(),
                    label: "Q2".into(),
                    description: Some("no label".into())
                },
            ]
        );
    }

    #[test]
    fn the_query_aggregates_and_takes_no_text() {
        let q = sparql(&ids(&["Q83495", "Q1"]), &ids(&["pt-BR", "en"])).unwrap();
        assert!(q.contains("VALUES ?item { wd:Q83495 wd:Q1 }"), "{q}");
        assert!(q.contains(r#"wikibase:language "pt-br,en""#), "{q}");
        assert!(q.contains("GROUP_CONCAT(DISTINCT ?directorName"), "{q}");
        assert!(q.contains("SAMPLE(?posterFile)"), "{q}");
        assert!(q.contains("MIN(CONCAT(STR(?dateValue)"), "{q}");
        assert!(q.contains("wdt:P3383"), "the poster");
        assert!(q.contains("GROUP BY ?item ?title"), "{q}");
        // English is always the last fallback.
        assert!(
            sparql(&ids(&["Q1"]), &ids(&["de"]))
                .unwrap()
                .contains(r#""de,en""#)
        );
        assert_eq!(sparql(&ids(&["Q1 } UNION {"]), &[]), Err(Why::Malformed));
        assert_eq!(
            sparql(&ids(&["Q1"]), &ids(&["en\" ."])),
            Err(Why::Malformed)
        );
        assert_eq!(sparql(&[], &[]), Err(Why::NothingToAsk));
    }

    #[test]
    fn a_date_is_as_precise_as_wikidata_knows_it() {
        assert_eq!(
            date_of("1999-03-31T00:00:00Z|11").as_deref(),
            Some("1999-03-31")
        );
        assert_eq!(
            date_of("1999-03-01T00:00:00Z|10").as_deref(),
            Some("1999-03")
        );
        assert_eq!(date_of("1999-01-01T00:00:00Z|9").as_deref(), Some("1999"));
        assert_eq!(
            date_of("1990-01-01T00:00:00Z|8"),
            None,
            "a decade is no date"
        );
        assert_eq!(date_of("garbage|11"), None);
        assert_eq!(date_of("1999-03-31T00:00:00Z"), None);
    }

    #[test]
    fn a_query_answer_is_one_film_per_item_in_the_searchs_order() {
        let body = "{\"head\":{\"vars\":[]},\"results\":{\"bindings\":[
            {\"item\":{\"type\":\"uri\",\"value\":\"http://www.wikidata.org/entity/Q2\"},
             \"title\":{\"value\":\"A Show\"},\"kind\":{\"value\":\"series\"},
             \"tmdb\":{\"value\":\"1399\"}},
            {\"item\":{\"value\":\"http://www.wikidata.org/entity/Q83495\"},
             \"title\":{\"value\":\"Matrix\"},\"kind\":{\"value\":\"film\"},
             \"date\":{\"value\":\"1999-03-24T00:00:00Z|11\"},
             \"directors\":{\"value\":\"Lana Wachowski\\u001fLilly Wachowski\"},
             \"genres\":{\"value\":\"science fiction film\\u001fhttp://www.wikidata.org/entity/Q9\"},
             \"language\":{\"value\":\"English\"},
             \"imdb\":{\"value\":\"tt0133093\"},\"tmdb\":{\"value\":\"603\"},
             \"poster\":{\"value\":\"http://commons.wikimedia.org/wiki/Special:FilePath/Matrix.jpg\"}},
            {\"item\":{\"value\":\"http://www.wikidata.org/entity/Q5\"},
             \"title\":{\"value\":\"A painting\"},\"kind\":{\"value\":\"other\"}}
        ]}}";
        let films = parse_films(body.as_bytes(), &ids(&["Q83495", "Q2", "Q5"])).unwrap();
        assert_eq!(films.len(), 2, "what is neither film nor series is dropped");
        let m = &films[0];
        assert_eq!(m.id, "Q83495", "the search's order");
        assert_eq!(m.date.as_deref(), Some("1999-03-24"));
        assert_eq!(m.directors, ["Lana Wachowski", "Lilly Wachowski"]);
        assert_eq!(
            m.genres,
            ["science fiction film"],
            "an unlabelled item is dropped"
        );
        assert_eq!(m.imdb.as_deref(), Some("tt0133093"));
        assert_eq!(m.tmdb, Some(603));
        assert_eq!(films[1].kind, Kind::Series);

        let s = suggestion(m, 1200);
        assert_eq!(s.value(Field::Title), Some("Matrix"));
        assert_eq!(
            s.value(Field::Director),
            Some("Lana Wachowski; Lilly Wachowski")
        );
        assert_eq!(s.value(Field::Date), Some("1999-03-24"));
        assert_eq!(
            s.cover,
            Some(Cover::Address {
                host: Host::Commons,
                url: "https://commons.wikimedia.org/wiki/Special:FilePath/Matrix.jpg?width=1200"
                    .into()
            }),
            "the poster, over https and scaled on the server"
        );
        assert_eq!(
            suggestion(&films[1], 500).value(Field::Show),
            Some("A Show")
        );
    }

    #[test]
    fn words_become_films_in_two_questions_and_nothing_found_asks_once() {
        let search = search_url("matrix", "en").unwrap();
        let films_at = films_url(&ids(&["Q83495"]), &ids(&["en"])).unwrap();
        let hits = br#"{"search":[{"id":"Q83495","label":"The Matrix"}]}"#;
        let rows =
            br#"{"results":{"bindings":[{"item":{"value":"http://www.wikidata.org/entity/Q83495"},
            "title":{"value":"The Matrix"},"kind":{"value":"film"}}]}}"#;
        let net = Canned::new(&[(search.as_str(), 200, hits), (films_at.as_str(), 200, rows)]);
        let found = find(&net, "matrix", &ids(&["en"])).unwrap();
        assert_eq!(found.len(), 1);
        let asked = net.asked();
        assert_eq!(asked[0].host, Host::Wikidata);
        assert_eq!(asked[1].host, Host::WikidataQuery);
        assert_eq!(asked[1].accept, "application/sparql-results+json");

        let net = Canned::new(&[(search.as_str(), 200, br#"{"search":[]}"#)]);
        assert_eq!(find(&net, "matrix", &ids(&["en"])), Ok(Vec::new()));
        assert_eq!(net.asked().len(), 1, "no query about nothing");
    }
}
