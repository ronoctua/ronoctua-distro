//! The Internet Archive: a category's films, and the files of one of them
//! that mpv plays.
//!
//! # The search
//!
//! `advancedsearch.php` over a category's collections, most downloaded first.
//! `AND mediatype:(movies)` is not optional: without it `collection:(…)`
//! answers the collection's SUB-collections first (measured: `feature_films`
//! gave `SciFi_Horror` and `silent_films` before any film).
//!
//! # An item, reduced to what plays
//!
//! An item is an original and the copies the archive derived from it — an
//! MPEG-2 and its h.264, its Ogg, its thumbnails, a torrent, three metadata
//! files. [`playable`] keeps, for each original, the ONE copy mpv plays best:
//!
//! 1. h.264 or MPEG-4 at most 1080 lines high, the tallest first;
//! 2. then Ogg video, WebM or Matroska;
//! 3. then h.264 or MPEG-4 taller than 1080 (heavy on a machine that decodes
//!    in software);
//! 4. and for an item with no picture at all, VBR MP3, then Ogg Vorbis, then
//!    FLAC.
//!
//! It never keeps a thumbnail (`_thumb`), a GIF, a `_meta` file or a torrent.
//! An item where nothing is left is refused with [`Why::NothingPlayable`]
//! (measured: `HisNewJobCharlesChaplin-1915` has only metadata).
//!
//! # The address
//!
//! `https://archive.org/download/<identifier>/<file>`, which answers with a
//! redirect to the storage host holding it today, which answers ranges. The
//! storage host (`d1` in the metadata) moves; the `download` address does
//! not, and it is what a saved queue keeps.
//!
//! The item's `licenseurl` is carried, so the page can say what the archive
//! says about the rights — the collection's claim, not a promise.

use ronoctua_playlist::own::{Origin, Stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ask::{Net, Request, answered};
use crate::json::{number, text, value, whole};
use crate::query::{escape, words};
use crate::{Refusal, Source, Why};

/// Where the archive answers.
pub const HOST: &str = "archive.org";

/// The most rows one page may hold, whatever the caller asks.
pub const ROWS_CEILING: u32 = 100;

/// The longest words of a search, in characters.
pub const WORDS_MOST: usize = 100;

fn refused(why: Why) -> Refusal {
    Refusal::new(Source::Archive, why)
}

/// An item's identifier, as the archive writes one: letters, digits, `.`,
/// `_` and `-`, not starting with `.`.
#[must_use]
pub fn is_identifier(id: &str) -> bool {
    (1..=100).contains(&id.len())
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// One search of the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Search<'a> {
    /// The collections asked (at least one).
    pub collections: &'a [&'a str],
    /// Subjects that narrow them; all of them when empty.
    pub subjects: &'a [&'a str],
    /// Words in the title, when the person typed some.
    pub words: Option<&'a str>,
    /// Which page, from 1.
    pub page: u32,
    /// How many rows a page holds.
    pub rows: u32,
}

/// Lucene's special characters, escaped so a typed word is a word and not
/// syntax.
fn lucene(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(
            c,
            '+' | '-'
                | '&'
                | '|'
                | '!'
                | '('
                | ')'
                | '{'
                | '}'
                | '['
                | ']'
                | '^'
                | '"'
                | '~'
                | '*'
                | '?'
                | ':'
                | '\\'
                | '/'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The query of a search: its collections, films only, its subjects and its
/// words. Pure.
///
/// # Errors
///
/// [`Why::NothingToAsk`] for a search with no collection, or a collection
/// that is not shaped like an identifier.
pub fn query_of(search: &Search<'_>) -> Result<String, Why> {
    if search.collections.is_empty() || !search.collections.iter().all(|c| is_identifier(c)) {
        return Err(Why::NothingToAsk);
    }
    let mut q = format!(
        "collection:({}) AND mediatype:(movies)",
        search.collections.join(" OR ")
    );
    let subjects: Vec<String> = search
        .subjects
        .iter()
        .filter_map(|s| words(s, WORDS_MOST))
        .map(|s| format!("\"{}\"", lucene(&s)))
        .collect();
    if !subjects.is_empty() {
        q.push_str(&format!(" AND subject:({})", subjects.join(" OR ")));
    }
    if let Some(w) = search.words.and_then(|w| words(w, WORDS_MOST)) {
        q.push_str(&format!(" AND title:({})", lucene(&w)));
    }
    Ok(q)
}

/// The address of a search, the most downloaded first. Pure.
///
/// # Errors
///
/// As [`query_of`].
pub fn search_address(search: &Search<'_>) -> Result<String, Why> {
    let q = query_of(search)?;
    let fields = ["identifier", "title", "year", "downloads", "licenseurl"]
        .iter()
        .map(|f| format!("&fl%5B%5D={f}"))
        .collect::<String>();
    Ok(format!(
        "https://{HOST}/advancedsearch.php?q={}{fields}&sort%5B%5D=downloads%20desc&rows={}&page={}&output=json",
        escape(&q),
        search.rows.clamp(1, ROWS_CEILING),
        search.page.max(1)
    ))
}

/// One item a search found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Its identifier: what opens it.
    pub identifier: String,
    /// Its title; the identifier when it has none.
    pub title: String,
    /// Its year, when said.
    pub year: Option<i32>,
    /// How many times it was downloaded.
    pub downloads: u64,
    /// What the archive says about its rights, when it says.
    pub license_url: Option<String>,
}

/// One page of a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Found {
    /// The items of this page, in the archive's order.
    pub items: Vec<Item>,
    /// How many there are in all.
    pub total: u64,
    /// Which page this is, from 1.
    pub page: u32,
    /// Whether there is a page after it.
    pub more: bool,
}

/// A year out of the archive's `year` (a number, or a string such as
/// `1915` or `1915-08-02`).
fn year_of(v: &Value) -> Option<i32> {
    let digits: String = text(v, "year")
        .or_else(|| whole(v, "year").map(|n| n.to_string()))?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok().filter(|y| (1800..=2200).contains(y))
}

/// A page of a search, read. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for what is not the archive's answer.
pub fn parse_search(bytes: &[u8], page: u32, rows: u32) -> Result<Found, Refusal> {
    let top = value(Source::Archive, bytes)?;
    let response = top
        .get("response")
        .filter(|r| r.is_object())
        .ok_or(refused(Why::Malformed))?;
    let Some(docs) = response.get("docs").and_then(Value::as_array) else {
        return Err(refused(Why::Malformed));
    };
    let items: Vec<Item> = docs
        .iter()
        .filter_map(|d| {
            let identifier = text(d, "identifier").filter(|i| is_identifier(i))?;
            Some(Item {
                title: text(d, "title").unwrap_or_else(|| identifier.clone()),
                year: year_of(d),
                downloads: whole(d, "downloads").unwrap_or(0),
                license_url: text(d, "licenseurl"),
                identifier,
            })
        })
        .collect();
    let total = whole(response, "numFound").unwrap_or(items.len() as u64);
    let page = page.max(1);
    let seen = u64::from(page) * u64::from(rows.clamp(1, ROWS_CEILING));
    Ok(Found {
        items,
        total,
        page,
        more: seen < total,
    })
}

/// Searches the archive.
///
/// # Errors
///
/// [`Why::NothingToAsk`], the archive's refusal, or [`Why::Malformed`].
pub fn search(net: &dyn Net, search: &Search<'_>) -> Result<Found, Refusal> {
    let url = search_address(search).map_err(refused)?;
    parse_search(
        &answered(net, &Request::get(Source::Archive, url))?,
        search.page,
        search.rows,
    )
}

/// What a playable file shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A picture, with its sound.
    Video,
    /// Sound only.
    Audio,
}

/// One file of an item that mpv plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playable {
    /// Its name inside the item.
    pub name: String,
    /// Its address.
    pub url: String,
    /// A picture or a sound.
    pub kind: Kind,
    /// The archive's name for its format (`h.264`, `VBR MP3`).
    pub format: String,
    /// Its height, when said.
    pub lines: Option<u16>,
    /// Its length, when said.
    pub length_seconds: Option<f64>,
    /// Its title, when the file has one of its own.
    pub title: Option<String>,
}

/// An item, opened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Opened {
    /// Its identifier.
    pub identifier: String,
    /// Its title; the identifier when it has none.
    pub title: String,
    /// What the archive says about its rights, when it says.
    pub license_url: Option<String>,
    /// What plays, one per original, in the order of their names. Never
    /// empty.
    pub files: Vec<Playable>,
}

impl Opened {
    /// The streams the queue holds: one per file, titled by the file's own
    /// title, else the item's (with the file's name when there are several).
    #[must_use]
    pub fn streams(&self) -> Vec<Stream> {
        let many = self.files.len() > 1;
        self.files
            .iter()
            .map(|f| Stream {
                url: f.url.clone(),
                title: Some(f.title.clone().unwrap_or_else(|| {
                    if many {
                        format!("{} — {}", self.title, f.name)
                    } else {
                        self.title.clone()
                    }
                })),
                referrer: None,
                user_agent: None,
                alternates: Vec::new(),
                origin: Origin::Archive {
                    identifier: self.identifier.clone(),
                },
                live: false,
            })
            .collect()
    }
}

/// The formats a picture is kept in, by rank (lower is better), for a height
/// within 1080 lines.
fn video_rank(format: &str, lines: Option<u16>) -> Option<u8> {
    let f = format.to_ascii_lowercase();
    let h264 = f.starts_with("h.264") || f.ends_with("mpeg4");
    if h264 {
        return Some(if lines.is_some_and(|h| h > 1080) {
            2
        } else {
            0
        });
    }
    matches!(f.as_str(), "ogg video" | "webm" | "matroska").then_some(1)
}

/// The formats a sound is kept in, by rank.
fn audio_rank(format: &str) -> Option<u8> {
    match format.to_ascii_lowercase().as_str() {
        "vbr mp3" => Some(0),
        "ogg vorbis" => Some(1),
        "flac" | "24bit flac" => Some(2),
        _ => None,
    }
}

/// Whether a file is never played, whatever its format says.
fn never(name: &str, format: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let f = format.to_ascii_lowercase();
    n.contains("_thumb")
        || n.ends_with(".gif")
        || n.contains("_meta")
        || n.ends_with(".torrent")
        || f.contains("thumbnail")
        || f.contains("gif")
        || f == "metadata"
        || f.contains("bittorrent")
}

/// A length out of the archive's `length`: seconds (`1233.45`) or a clock
/// (`20:33`, `1:02:03`).
fn length_of(v: &Value) -> Option<f64> {
    if let Some(n) = number(v, "length") {
        return (n > 0.0).then_some(n);
    }
    let clock = text(v, "length")?;
    let mut seconds = 0.0;
    for part in clock.split(':') {
        seconds = seconds * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    (seconds > 0.0 && seconds.is_finite()).then_some(seconds)
}

/// The address of a file of an item, each segment of its name escaped.
fn download_address(identifier: &str, name: &str) -> String {
    let path: Vec<String> = name.split('/').map(escape).collect();
    format!("https://{HOST}/download/{identifier}/{}", path.join("/"))
}

/// What of an item's `files` mpv plays: per original, the one copy it plays
/// best (see the module's header), pictures before sounds — an item with any
/// picture plays only pictures. Pure.
#[must_use]
pub fn playable(identifier: &str, files: &[Value]) -> Vec<Playable> {
    // (group, video?, rank, taller first, name) → the file.
    let mut best: Vec<(String, bool, u8, u16, Playable)> = Vec::new();
    for f in files {
        let Some(name) = text(f, "name") else {
            continue;
        };
        let format = text(f, "format").unwrap_or_default();
        if never(&name, &format) || name.contains("..") {
            continue;
        }
        let lines = whole(f, "height")
            .and_then(|h| u16::try_from(h).ok())
            .filter(|h| *h > 0);
        let (video, rank) = match (video_rank(&format, lines), audio_rank(&format)) {
            (Some(r), _) => (true, r),
            (None, Some(r)) => (false, r),
            (None, None) => continue,
        };
        // A DERIVATIVE BELONGS TO ITS ORIGINAL: the h.264 and the Ogg made
        // from one MPEG-2 are one film, played once.
        let group = match text(f, "source").as_deref() {
            Some("derivative") => text(f, "original").unwrap_or_else(|| name.clone()),
            _ => name.clone(),
        };
        let candidate = Playable {
            url: download_address(identifier, &name),
            kind: if video { Kind::Video } else { Kind::Audio },
            format,
            lines,
            length_seconds: length_of(f),
            title: text(f, "title"),
            name,
        };
        let taller = lines.unwrap_or(0);
        match best.iter_mut().find(|b| b.0 == group && b.1 == video) {
            Some(b) => {
                let better = (rank, std::cmp::Reverse(taller), &candidate.name)
                    < (b.2, std::cmp::Reverse(b.3), &b.4.name);
                if better {
                    *b = (group, video, rank, taller, candidate);
                }
            }
            None => best.push((group, video, rank, taller, candidate)),
        }
    }
    let any_picture = best.iter().any(|b| b.1);
    let mut out: Vec<(String, Playable)> = best
        .into_iter()
        .filter(|b| b.1 == any_picture)
        .map(|b| (b.0, b.4))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|(_, p)| p).collect()
}

// ---------------------------------------------------------------------------
// A WHOLE COLLECTION, for the catalogue
// ---------------------------------------------------------------------------

/// What of a collection is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Media {
    /// Films: `mediatype:(movies)`.
    Movies,
    /// Recordings of sound: `mediatype:(audio)` (LibriVox).
    Audio,
    /// Either — a person's collection, whose kind is not declared.
    Either,
}

/// The most rows one page of a collection may hold.
pub const COLLECTION_ROWS_CEILING: u32 = 1000;

/// One page of a collection, asked for the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Collection<'a> {
    /// Its identifier.
    pub identifier: &'a str,
    /// What of it.
    pub media: Media,
    /// Subjects that narrow it; all of it when empty.
    pub subjects: &'a [&'a str],
    /// Which page, from 1.
    pub page: u32,
    /// How many rows a page holds (at most [`COLLECTION_ROWS_CEILING`]).
    pub rows: u32,
}

/// The query of a collection's page: its items of that media — never its
/// sub-collections — narrowed by its subjects. Pure.
///
/// # Errors
///
/// [`Why::NotAName`] for an identifier that is not shaped like one.
pub fn collection_query(c: &Collection<'_>) -> Result<String, Why> {
    if !is_identifier(c.identifier) {
        return Err(Why::NotAName);
    }
    let media = match c.media {
        Media::Movies => "mediatype:(movies)",
        Media::Audio => "mediatype:(audio)",
        Media::Either => "mediatype:(movies OR audio)",
    };
    let mut q = format!("collection:({}) AND {media}", c.identifier);
    let subjects: Vec<String> = c
        .subjects
        .iter()
        .filter_map(|s| words(s, WORDS_MOST))
        .map(|s| format!("\"{}\"", lucene(&s)))
        .collect();
    if !subjects.is_empty() {
        q.push_str(&format!(" AND subject:({})", subjects.join(" OR ")));
    }
    Ok(q)
}

/// The address of a collection's page, the most downloaded first. Pure.
///
/// # Errors
///
/// As [`collection_query`].
pub fn collection_address(c: &Collection<'_>) -> Result<String, Why> {
    let q = collection_query(c)?;
    let fields = [
        "identifier",
        "title",
        "year",
        "downloads",
        "licenseurl",
        "description",
        "subject",
        "language",
        "mediatype",
    ]
    .iter()
    .map(|f| format!("&fl%5B%5D={f}"))
    .collect::<String>();
    Ok(format!(
        "https://{HOST}/advancedsearch.php?q={}{fields}&sort%5B%5D=downloads%20desc&rows={}&page={}&output=json",
        escape(&q),
        c.rows.clamp(1, COLLECTION_ROWS_CEILING),
        c.page.max(1)
    ))
}

/// The address of an item's picture, which the archive serves for every
/// item from its own host.
#[must_use]
pub fn poster_address(identifier: &str) -> Option<String> {
    is_identifier(identifier).then(|| format!("https://{HOST}/services/img/{identifier}"))
}

/// One item of a collection, as the catalogue keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holding {
    /// The item, as a search finds it.
    pub item: Item,
    /// What it says about itself, markup off, when it says.
    pub description: Option<String>,
    /// Its subjects.
    pub subjects: Vec<String>,
    /// Its language, as the archive writes it.
    pub language: Option<String>,
    /// It is sound only (`mediatype: audio`).
    pub audio: bool,
}

/// One page of a collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holdings {
    /// Its items, the most downloaded first.
    pub items: Vec<Holding>,
    /// How many the collection has in all.
    pub total: u64,
    /// Whether there is a page after this one.
    pub more: bool,
}

/// A page of a collection, read. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for what is not the archive's answer.
pub fn parse_collection(bytes: &[u8], page: u32, rows: u32) -> Result<Holdings, Refusal> {
    let top = value(Source::Archive, bytes)?;
    let response = top
        .get("response")
        .filter(|r| r.is_object())
        .ok_or(refused(Why::Malformed))?;
    let Some(docs) = response.get("docs").and_then(Value::as_array) else {
        return Err(refused(Why::Malformed));
    };
    let items: Vec<Holding> = docs
        .iter()
        .filter_map(|d| {
            let identifier = text(d, "identifier").filter(|i| is_identifier(i))?;
            Some(Holding {
                item: Item {
                    title: text(d, "title").unwrap_or_else(|| identifier.clone()),
                    year: year_of(d),
                    downloads: whole(d, "downloads").unwrap_or(0),
                    license_url: text(d, "licenseurl"),
                    identifier,
                },
                description: text(d, "description")
                    .map(|h| crate::sources::feed::plain(&h))
                    .filter(|h| !h.is_empty()),
                subjects: crate::json::texts(d, "subject"),
                language: text(d, "language"),
                audio: text(d, "mediatype").as_deref() == Some("audio"),
            })
        })
        .collect();
    let total = whole(response, "numFound").unwrap_or(items.len() as u64);
    let seen = u64::from(page.max(1)) * u64::from(rows.clamp(1, COLLECTION_ROWS_CEILING));
    Ok(Holdings {
        items,
        total,
        more: seen < total,
    })
}

/// Asks one page of a collection, on the person's consent for the archive.
///
/// # Errors
///
/// [`Refused::NotLet`], [`Refused::NotAnAddress`] for an identifier that is
/// not one (nothing is sent), the network's refusals, [`Refused::Malformed`].
#[cfg(feature = "net")]
pub fn collection(
    web: &dyn crate::ask::Web,
    pass: &ronoctua_consent::Pass,
    c: &Collection<'_>,
) -> Result<Holdings, crate::sources::Refused> {
    let url = collection_address(c).map_err(crate::sources::Refused::from)?;
    let body = crate::ask::fetched(web, pass, &crate::ask::Fetch::json(url).most(16 << 20))?;
    parse_collection(&body, c.page, c.rows).map_err(|r| crate::sources::Refused::from(r.why))
}

/// The address of an item's own page.
///
/// # Errors
///
/// [`Why::NotAName`] for an identifier that is not shaped like one.
pub fn details_address(identifier: &str) -> Result<String, Why> {
    if !is_identifier(identifier) {
        return Err(Why::NotAName);
    }
    Ok(format!("https://{HOST}/details/{identifier}"))
}

impl Item {
    /// The WHOLE item as a stream a door puts in the queue: its page's
    /// address, found in the archive. The daemon opens it when it is put —
    /// what plays is its files ([`open`]), one entry or a playlist of them —
    /// because which files an item has is only known by asking
    /// ([`whole_item`] tells it from a file's stream). `None` for an item
    /// whose identifier is not shaped like one.
    #[must_use]
    pub fn stream(&self) -> Option<Stream> {
        Some(Stream {
            url: details_address(&self.identifier).ok()?,
            title: Some(self.title.clone()),
            referrer: None,
            user_agent: None,
            alternates: Vec::new(),
            origin: Origin::Archive {
                identifier: self.identifier.clone(),
            },
            live: false,
        })
    }
}

/// The identifier of the item `stream` stands for WHOLE — its page, as
/// [`Item::stream`] makes it — and `None` for any other stream: one of the
/// item's files ([`Opened::streams`]) plays as it is.
#[must_use]
pub fn whole_item(stream: &Stream) -> Option<&str> {
    match &stream.origin {
        Origin::Archive { identifier }
            if details_address(identifier).is_ok_and(|a| a == stream.url) =>
        {
            Some(identifier)
        }
        _ => None,
    }
}

/// The address of an item's metadata.
///
/// # Errors
///
/// [`Why::NotAName`] for an identifier that is not shaped like one.
pub fn metadata_address(identifier: &str) -> Result<String, Why> {
    if !is_identifier(identifier) {
        return Err(Why::NotAName);
    }
    Ok(format!("https://{HOST}/metadata/{identifier}"))
}

/// An item's metadata, read and reduced to what plays. Pure.
///
/// # Errors
///
/// [`Why::NothingFound`] for an item that does not exist (the archive answers
/// `{}`), [`Why::Declined`] for a dark one, [`Why::NothingPlayable`] when
/// nothing in it plays, [`Why::Malformed`] for what is not an answer.
pub fn parse_metadata(identifier: &str, bytes: &[u8]) -> Result<Opened, Refusal> {
    let top = value(Source::Archive, bytes)?;
    let Some(object) = top.as_object() else {
        return Err(refused(Why::Malformed));
    };
    if object.is_empty() {
        return Err(refused(Why::NothingFound));
    }
    if top.get("is_dark").and_then(Value::as_bool) == Some(true) {
        return Err(refused(Why::Declined));
    }
    let files = top
        .get("files")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let meta = top.get("metadata").cloned().unwrap_or(Value::Null);
    let files = playable(identifier, files);
    if files.is_empty() {
        return Err(refused(Why::NothingPlayable));
    }
    Ok(Opened {
        identifier: identifier.to_owned(),
        title: text(&meta, "title").unwrap_or_else(|| identifier.to_owned()),
        license_url: text(&meta, "licenseurl"),
        files,
    })
}

/// Opens an item: its title, its rights and what of it plays.
///
/// # Errors
///
/// [`Why::NotAName`] (nothing is sent), the archive's refusal, and those of
/// [`parse_metadata`].
pub fn open(net: &dyn Net, identifier: &str) -> Result<Opened, Refusal> {
    let url = metadata_address(identifier).map_err(refused)?;
    let body = answered(net, &Request::get(Source::Archive, url).most(8 << 20))?;
    parse_metadata(identifier, &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;
    use serde_json::json;

    #[test]
    fn a_search_asks_for_films_in_the_collections_and_escapes_the_words() {
        let s = Search {
            collections: &["feature_films", "SciFi_Horror"],
            subjects: &["action"],
            words: Some("  night: of (the) \"dead\" "),
            page: 2,
            rows: 30,
        };
        assert_eq!(
            query_of(&s).unwrap(),
            "collection:(feature_films OR SciFi_Horror) AND mediatype:(movies) \
             AND subject:(\"action\") AND title:(night\\: of \\(the\\) \\\"dead\\\")"
        );
        let url = search_address(&s).unwrap();
        assert!(url.starts_with("https://archive.org/advancedsearch.php?q=collection%3A%28feature_films%20OR%20SciFi_Horror%29%20AND%20mediatype%3A%28movies%29"));
        assert!(url.ends_with("&sort%5B%5D=downloads%20desc&rows=30&page=2&output=json"));
        assert!(url.contains("&fl%5B%5D=licenseurl"));
        let none = Search {
            collections: &[],
            subjects: &[],
            words: None,
            page: 1,
            rows: 30,
        };
        assert_eq!(query_of(&none), Err(Why::NothingToAsk));
        let odd = Search {
            collections: &["a) OR (b"],
            ..none
        };
        assert_eq!(query_of(&odd), Err(Why::NothingToAsk));
    }

    #[test]
    fn a_page_of_results_is_read_whatever_shape_its_fields_came_in() {
        let body = br#"{"responseHeader":{"status":0},"response":{"numFound":65,"start":30,"docs":[
            {"identifier":"NightOfTheLivingDead","title":"Night of the Living Dead","year":"1968",
             "downloads":123456,"licenseurl":"http://creativecommons.org/publicdomain/mark/1.0/"},
            {"identifier":"Untitled_1","year":1915,"downloads":"12"},
            {"identifier":"../etc","title":"no"},
            {"title":"no identifier"}
        ]}}"#;
        let found = parse_search(body, 2, 30).unwrap();
        assert_eq!(found.total, 65);
        assert!(found.more, "sixty seen of sixty-five");
        assert_eq!(found.items.len(), 2);
        assert_eq!(found.items[0].year, Some(1968));
        assert_eq!(found.items[1].title, "Untitled_1");
        assert_eq!(found.items[1].downloads, 12);
        assert!(!parse_search(body, 3, 30).unwrap().more);
        assert_eq!(parse_search(b"[]", 1, 30).unwrap_err().why, Why::Malformed);
        assert_eq!(
            parse_search(br#"{"response":{}}"#, 1, 30).unwrap_err().why,
            Why::Malformed
        );
    }

    fn file(
        name: &str,
        format: &str,
        source: &str,
        original: Option<&str>,
        height: Option<&str>,
    ) -> Value {
        let mut f = json!({"name": name, "format": format, "source": source});
        if let Some(o) = original {
            f["original"] = json!(o);
        }
        if let Some(h) = height {
            f["height"] = json!(h);
        }
        f
    }

    #[test]
    fn the_archive_picks_what_mpv_plays_and_never_a_thumbnail() {
        let files = vec![
            file("film.mpeg", "MPEG2", "original", None, Some("480")),
            file(
                "film.ogv",
                "Ogg Video",
                "derivative",
                Some("film.mpeg"),
                Some("480"),
            ),
            file(
                "film_512kb.mp4",
                "512Kb MPEG4",
                "derivative",
                Some("film.mpeg"),
                Some("240"),
            ),
            file(
                "film.mp4",
                "h.264",
                "derivative",
                Some("film.mpeg"),
                Some("480"),
            ),
            // A THUMBNAIL IN A PLAYABLE FORMAT, and taller: it would win on
            // format and height, and it is still never played.
            file(
                "film_thumb.mp4",
                "h.264",
                "derivative",
                Some("film.mpeg"),
                Some("720"),
            ),
            file(
                "film.thumbs/film_000001.jpg",
                "Thumbnail",
                "derivative",
                Some("film.mpeg"),
                None,
            ),
            file(
                "film.gif",
                "Animated GIF",
                "derivative",
                Some("film.mpeg"),
                None,
            ),
            file("__ia_thumb.jpg", "Item Tile", "original", None, None),
            file("item_meta.xml", "Metadata", "original", None, None),
            file(
                "item_archive.torrent",
                "Archive BitTorrent",
                "metadata",
                None,
                None,
            ),
            file("big.mkv", "Matroska", "original", None, Some("2160")),
            file(
                "big.mp4",
                "h.264 HD",
                "derivative",
                Some("big.mkv"),
                Some("2160"),
            ),
            file(
                "track.mp3",
                "VBR MP3",
                "derivative",
                Some("track.flac"),
                None,
            ),
        ];
        let got = playable("SomeFilm", &files);
        let picked: Vec<(&str, &str)> = got
            .iter()
            .map(|p| (p.name.as_str(), p.format.as_str()))
            .collect();
        // ONE PER ORIGINAL: the tallest h.264 of the MPEG-2; for a 4K item,
        // Matroska before an h.264 above 1080 lines. The sound is left out,
        // because the item has pictures.
        assert_eq!(picked, [("big.mkv", "Matroska"), ("film.mp4", "h.264")]);
        assert_eq!(got[1].url, "https://archive.org/download/SomeFilm/film.mp4");
        assert!(got.iter().all(|p| p.kind == Kind::Video));
    }

    #[test]
    fn a_recording_plays_its_sound_in_the_order_asked() {
        let files = vec![
            file("side-a.flac", "Flac", "original", None, None),
            file(
                "side-a.ogg",
                "Ogg Vorbis",
                "derivative",
                Some("side-a.flac"),
                None,
            ),
            file(
                "side-a.mp3",
                "VBR MP3",
                "derivative",
                Some("side-a.flac"),
                None,
            ),
            file("side-b.flac", "Flac", "original", None, None),
            file("cover.png", "PNG", "original", None, None),
        ];
        let got = playable("Record", &files);
        let picked: Vec<&str> = got.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(picked, ["side-a.mp3", "side-b.flac"]);
        assert!(got.iter().all(|p| p.kind == Kind::Audio));
    }

    #[test]
    fn an_item_with_nothing_playable_is_refused() {
        let chaplin = br#"{"files":[
            {"name":"HisNewJob_archive.torrent","source":"metadata","format":"Archive BitTorrent"},
            {"name":"HisNewJob_meta.xml","source":"original","format":"Metadata"},
            {"name":"__ia_thumb.jpg","source":"original","format":"Item Tile"}],
            "metadata":{"title":"His New Job"}}"#;
        assert_eq!(
            parse_metadata("HisNewJob", chaplin).unwrap_err().why,
            Why::NothingPlayable
        );
        assert_eq!(
            parse_metadata("x", b"{}").unwrap_err().why,
            Why::NothingFound
        );
        assert_eq!(
            parse_metadata("x", br#"{"is_dark":true}"#).unwrap_err().why,
            Why::Declined
        );
    }

    #[test]
    fn an_opened_item_carries_its_title_rights_and_streams() {
        let body = br#"{"d1":"dn800309.us.archive.org","dir":"/0/items/Night",
            "files":[{"name":"Night of the Living Dead.mp4","format":"h.264","source":"original",
                      "height":"480","length":"5760.5"},
                     {"name":"Part 2.mp4","format":"h.264","source":"original","length":"20:33",
                      "title":"The second part"}],
            "metadata":{"title":["Night of the Living Dead"],
                        "licenseurl":"http://creativecommons.org/publicdomain/mark/1.0/"}}"#;
        let opened = parse_metadata("Night", body).unwrap();
        assert_eq!(opened.title, "Night of the Living Dead");
        assert_eq!(
            opened.license_url.as_deref(),
            Some("http://creativecommons.org/publicdomain/mark/1.0/")
        );
        assert_eq!(opened.files[0].length_seconds, Some(5760.5));
        assert_eq!(opened.files[1].length_seconds, Some(1233.0));
        assert_eq!(
            opened.files[0].url,
            "https://archive.org/download/Night/Night%20of%20the%20Living%20Dead.mp4"
        );
        let streams = opened.streams();
        assert_eq!(streams.len(), 2);
        assert!(streams.iter().all(|s| !s.live));
        assert_eq!(
            streams[0].origin,
            Origin::Archive {
                identifier: "Night".to_owned()
            }
        );
        assert_eq!(
            streams[0].title.as_deref(),
            Some("Night of the Living Dead — Night of the Living Dead.mp4")
        );
        assert_eq!(streams[1].title.as_deref(), Some("The second part"));
    }

    #[test]
    fn opening_asks_the_metadata_and_never_sends_a_bad_identifier() {
        let net = Canned::new(&[(
            "https://archive.org/metadata/Night",
            200,
            br#"{"files":[{"name":"a.mp4","format":"h.264","source":"original"}],"metadata":{}}"#,
        )]);
        let opened = open(&net, "Night").unwrap();
        assert_eq!(opened.title, "Night", "no title: the identifier");
        let nothing = Canned::new(&[]);
        assert_eq!(open(&nothing, "../x").unwrap_err().why, Why::NotAName);
        assert!(nothing.asked().is_empty());
    }

    /// FAILS IF: a collection's page asks for its sub-collections, or its
    /// items lose their description, subjects or kind.
    ///
    /// The answer is HAND-WRITTEN in the shape `advancedsearch.php` answers
    /// (`response.numFound`, `docs[]` whose fields come as strings or lists).
    #[test]
    fn a_collection_asks_its_items_and_reads_them_whole() {
        let c = Collection {
            identifier: "librivoxaudio",
            media: Media::Audio,
            subjects: &[],
            page: 2,
            rows: 5000,
        };
        assert_eq!(
            collection_query(&c).unwrap(),
            "collection:(librivoxaudio) AND mediatype:(audio)"
        );
        let url = collection_address(&c).unwrap();
        assert!(url.contains("&rows=1000&page=2"), "{url}");
        assert!(url.contains("&fl%5B%5D=description&fl%5B%5D=subject"));
        let narrowed = Collection {
            identifier: "feature_films",
            media: Media::Movies,
            subjects: &["action"],
            ..c
        };
        assert_eq!(
            collection_query(&narrowed).unwrap(),
            "collection:(feature_films) AND mediatype:(movies) AND subject:(\"action\")"
        );
        let bad = Collection {
            identifier: "a) OR (b",
            ..c
        };
        assert_eq!(collection_query(&bad), Err(Why::NotAName));

        let body = br#"{"response":{"numFound":3,"docs":[
            {"identifier":"pride_prejudice_librivox","title":"Pride and Prejudice","year":"2006",
             "downloads":1234,"description":"<p>A <b>novel</b></p>",
             "subject":["fiction","romance"],"language":"English","mediatype":"audio"},
            {"identifier":"Duck_and_Cover","title":["Duck and Cover"],"mediatype":"movies",
             "subject":"civil defense; cold war"},
            {"identifier":"../x"}
        ]}}"#;
        let found = parse_collection(body, 1, 2).unwrap();
        assert_eq!(found.items.len(), 2);
        assert!(found.more);
        let book = &found.items[0];
        assert!(book.audio);
        assert_eq!(book.description.as_deref(), Some("A novel"));
        assert_eq!(book.subjects, ["fiction", "romance"]);
        assert_eq!(book.item.year, Some(2006));
        assert!(!found.items[1].audio);
        assert_eq!(
            poster_address("Duck_and_Cover").as_deref(),
            Some("https://archive.org/services/img/Duck_and_Cover")
        );
        assert_eq!(poster_address("../x"), None);
    }

    /// FAILS IF: a whole item stops being told from one of its files — the
    /// daemon would open a file's address as an item, or play an item's page
    /// as if it were a film.
    #[test]
    fn a_whole_item_is_its_page_and_a_file_is_not_one() {
        let item = Item {
            identifier: "Night".to_owned(),
            title: "Night of the Living Dead".to_owned(),
            year: Some(1968),
            downloads: 5,
            license_url: None,
        };
        let whole = item.stream().expect("a well-shaped identifier");
        assert_eq!(whole.url, "https://archive.org/details/Night");
        assert_eq!(whole.title.as_deref(), Some("Night of the Living Dead"));
        assert!(!whole.live);
        assert_eq!(whole_item(&whole), Some("Night"));
        let file = Stream {
            url: "https://archive.org/download/Night/a.mp4".to_owned(),
            ..whole
        };
        assert_eq!(whole_item(&file), None, "a file plays as it is");
        let typed = Stream::typed("https://archive.org/details/Night");
        assert_eq!(whole_item(&typed), None, "only what the directory made");
        let bad = Item {
            identifier: "../x".to_owned(),
            ..item
        };
        assert_eq!(bad.stream(), None);
    }
}
