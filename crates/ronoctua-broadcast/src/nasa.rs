//! The NASA Image and Video Library: public-domain video, asked by words, no
//! key.
//!
//! # Two hosts
//!
//! The search is `images-api.nasa.gov`; an item's files are on
//! `images-assets.nasa.gov`, listed in its `collection.json`. Both are named in
//! the source's consent row (`assets/channels.nix`), and each request carries
//! the [`Pass`](ronoctua_consent::Pass) of its own host.
//!
//! # What plays
//!
//! A search item names its `collection.json`, not a file: which renditions an
//! item has (`~orig.mp4`, `~large.mp4`, `~medium.mp4`, `~small.mp4`,
//! `~mobile.mp4`, `~preview.mp4`) is only known by asking. So the catalogue
//! keeps the item WHOLE — its stream is the `collection.json` address
//! ([`whole_item`] tells it apart) — and the daemon opens it when it is
//! played ([`open`]), the way an archive.org item is opened. [`playable`]
//! picks the rendition: the large one, then the medium, then the original
//! (which may be far heavier), then the smaller ones.
//!
//! The asset list writes `http://` and leaves spaces in names
//! (`http://images-assets.nasa.gov/video/Apollo 11 Overview/…`); an address
//! kept here is `https`, each segment escaped.
//!
//! The formats read are the ones the library documents
//! (<https://images.nasa.gov/docs/images.nasa.gov_api_docs.pdf>): the search
//! answers `collection.items[]`, each with `href` (its asset list), `data[]`
//! (`nasa_id`, `title`, `description`, `date_created`, `keywords`, `center`)
//! and `links[]` (`rel: preview` is its picture).

use ronoctua_playlist::own::{Origin, Stream};
use serde_json::Value;

use crate::catalog::{Harvest, ItemKind, NewItem};
use crate::date::{rfc3339, year_of};
use crate::json::{is_web_address, text, texts, whole};
use crate::query::{escape, words};
use crate::sources::Refused;
use crate::table::Native;

/// Where the library is searched.
pub const HOST: &str = "images-api.nasa.gov";

/// Where its files are.
pub const ASSETS_HOST: &str = "images-assets.nasa.gov";

/// The most rows one page asks for (the library's own limit is 100).
pub const PAGE_MOST: u32 = 100;

/// The renditions of a video, the one played first.
const RENDITIONS: [&str; 6] = [
    "~large.mp4",
    "~medium.mp4",
    "~orig.mp4",
    "~small.mp4",
    "~mobile.mp4",
    "~preview.mp4",
];

/// A NASA id as the library writes one: printable, no `/`, at most 200.
#[must_use]
pub fn is_nasa_id(id: &str) -> bool {
    (1..=200).contains(&id.len())
        && !id.contains('/')
        && !id.contains("..")
        && !id.chars().any(char::is_control)
}

/// The address of a search for videos. Pure.
///
/// # Errors
///
/// [`Refused::Unknown`] when there are no words.
pub fn search_address(words_asked: &str, page: u32, rows: u32) -> Result<String, Refused> {
    let w = words(words_asked, 200).ok_or(Refused::Unknown)?;
    Ok(format!(
        "https://{HOST}/search?q={}&media_type=video&page={}&page_size={}",
        escape(&w),
        page.max(1),
        rows.clamp(1, PAGE_MOST)
    ))
}

/// The address of an item's asset list. `None` for an id that is not one.
#[must_use]
pub fn assets_address(nasa_id: &str) -> Option<String> {
    is_nasa_id(nasa_id).then(|| {
        format!(
            "https://{ASSETS_HOST}/video/{}/collection.json",
            escape(nasa_id)
        )
    })
}

/// The NASA id of the item `stream` stands for whole — its asset list, as
/// [`harvest`] makes it — and `None` for any other stream.
#[must_use]
pub fn whole_item(stream: &Stream) -> Option<String> {
    let rest = stream
        .url
        .strip_prefix(&format!("https://{ASSETS_HOST}/video/"))?
        .strip_suffix("/collection.json")?;
    let id = unescaped(rest);
    (assets_address(&id).as_deref() == Some(stream.url.as_str())).then_some(id)
}

/// `%XX` escapes decoded (the inverse of [`escape`] for an id).
fn unescaped(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut n = 0;
    while n < bytes.len() {
        if bytes[n] == b'%'
            && let Some(hex) = text.get(n + 1..n + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            n += 3;
            continue;
        }
        out.push(bytes[n]);
        n += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One video the library found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    /// Its id.
    pub nasa_id: String,
    /// Its title.
    pub title: String,
    /// What it says about itself.
    pub description: Option<String>,
    /// Its keywords.
    pub keywords: Vec<String>,
    /// The NASA centre that made it.
    pub center: Option<String>,
    /// When it was made, in seconds since the epoch.
    pub created_s: Option<i64>,
    /// Its picture.
    pub preview: Option<String>,
}

/// One page of a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Its videos.
    pub videos: Vec<Video>,
    /// How many there are in all.
    pub total: u64,
    /// Whether the library names a next page.
    pub more: bool,
}

/// A page of a search, read. Pure.
///
/// # Errors
///
/// [`Refused::Malformed`] for what is not the library's answer.
pub fn parse_search(bytes: &[u8]) -> Result<Page, Refused> {
    let top: Value = serde_json::from_slice(bytes).map_err(|_| Refused::Malformed)?;
    let collection = top.get("collection").ok_or(Refused::Malformed)?;
    let items = collection
        .get("items")
        .and_then(Value::as_array)
        .ok_or(Refused::Malformed)?;
    let videos = items
        .iter()
        .filter_map(|item| {
            let data = item.get("data")?.as_array()?.first()?;
            if text(data, "media_type").as_deref() != Some("video") {
                return None;
            }
            let nasa_id = text(data, "nasa_id").filter(|id| is_nasa_id(id))?;
            let preview = item
                .get("links")
                .and_then(Value::as_array)
                .and_then(|links| {
                    links
                        .iter()
                        .find(|l| text(l, "rel").as_deref() == Some("preview"))
                })
                .and_then(|l| text(l, "href"))
                .map(|h| https(&h))
                .filter(|h| is_web_address(h));
            Some(Video {
                title: text(data, "title").unwrap_or_else(|| nasa_id.clone()),
                description: text(data, "description")
                    .map(|d| crate::sources::feed::plain(&d))
                    .filter(|d| !d.is_empty()),
                keywords: texts(data, "keywords"),
                center: text(data, "center"),
                created_s: text(data, "date_created").and_then(|d| rfc3339(&d)),
                preview,
                nasa_id,
            })
        })
        .collect();
    let total = collection
        .get("metadata")
        .and_then(|m| whole(m, "total_hits"))
        .unwrap_or(0);
    let more = collection
        .get("links")
        .and_then(Value::as_array)
        .is_some_and(|l| l.iter().any(|l| text(l, "rel").as_deref() == Some("next")));
    Ok(Page {
        videos,
        total,
        more,
    })
}

/// An asset address, as kept: `https`, every space escaped.
fn https(url: &str) -> String {
    let url = url.trim();
    let url = url
        .strip_prefix("http://")
        .map_or_else(|| url.to_owned(), |rest| format!("https://{rest}"));
    url.replace(' ', "%20")
}

/// The rendition an item's asset list offers to play: `https`, escaped.
/// Pure.
///
/// # Errors
///
/// [`Refused::Malformed`] for what is not a list of addresses,
/// [`Refused::NothingPlayable`] for a list with no video in it.
pub fn playable(bytes: &[u8]) -> Result<String, Refused> {
    let list: Vec<String> = serde_json::from_slice(bytes).map_err(|_| Refused::Malformed)?;
    RENDITIONS
        .iter()
        .find_map(|r| list.iter().find(|a| a.ends_with(r)))
        .map(|a| https(a))
        .filter(|a| crate::ask::host_of(a).as_deref() == Some(ASSETS_HOST))
        .ok_or(Refused::NothingPlayable)
}

/// A page's videos, as items of `native`'s category.
pub fn harvest(h: &mut Harvest, native: &Native, page: &Page) {
    h.category(native.key, native.ask, Some(native.theme), false);
    for v in &page.videos {
        let Some(url) = assets_address(&v.nasa_id) else {
            continue;
        };
        let mut also = v.keywords.clone();
        also.extend(v.center.iter().cloned());
        h.items.push(NewItem {
            ext_id: v.nasa_id.clone(),
            kind: ItemKind::Video,
            title: v.title.clone(),
            also,
            description: v.description.clone(),
            country: Some("US".to_owned()),
            language: None,
            height: None,
            duration_s: None,
            year: v.created_s.map(year_of),
            // WORKS OF THE UNITED STATES GOVERNMENT are in the public domain;
            // the library says so of all it holds, logos aside.
            licence: Some("public-domain".to_owned()),
            popularity: 0,
            published_s: v.created_s,
            poster_url: v.preview.clone(),
            stream: Stream {
                url,
                title: Some(v.title.clone()),
                referrer: None,
                user_agent: None,
                alternates: Vec::new(),
                origin: Origin::Typed,
                live: false,
            },
            categories: vec![native.key.to_owned()],
        });
    }
}

/// Searches the library, on the person's consent for [`HOST`].
///
/// # Errors
///
/// [`Refused::NotLet`], [`Refused::Unknown`] (no words; nothing is sent),
/// the network's refusals, and [`parse_search`]'s.
#[cfg(feature = "net")]
pub fn search(
    web: &dyn crate::ask::Web,
    pass: &ronoctua_consent::Pass,
    words_asked: &str,
    page: u32,
) -> Result<Page, Refused> {
    let url = search_address(words_asked, page, PAGE_MOST)?;
    parse_search(&crate::ask::fetched(
        web,
        pass,
        &crate::ask::Fetch::json(url),
    )?)
}

/// What an item plays, asked of its asset list, on the person's consent for
/// [`ASSETS_HOST`].
///
/// # Errors
///
/// [`Refused::NotLet`], [`Refused::NotAnAddress`] for an id that is not one
/// (nothing is sent), the network's refusals, and [`playable`]'s.
#[cfg(feature = "net")]
pub fn open(
    web: &dyn crate::ask::Web,
    pass: &ronoctua_consent::Pass,
    nasa_id: &str,
) -> Result<String, Refused> {
    let url = assets_address(nasa_id).ok_or(Refused::NotAnAddress)?;
    playable(&crate::ask::fetched(
        web,
        pass,
        &crate::ask::Fetch::json(url),
    )?)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// HAND-WRITTEN from the library's documented format (and the shape it
    /// answered on 2026-09-30, looked at, not kept): `collection.items[]`
    /// with `href`, `data[]` and `links[]`, `metadata.total_hits`, and a
    /// `next` link.
    pub(crate) const SEARCH: &str = r#"{"collection": {
        "version": "1.1",
        "href": "http://images-api.nasa.gov/search?q=apollo&media_type=video",
        "items": [
          {"href": "https://images-assets.nasa.gov/video/Apollo 11 Overview/collection.json",
           "data": [{"center": "HQ", "date_created": "2009-07-15T00:00:00Z",
                     "description": "<p>An overview of the <b>Apollo 11</b> mission.</p>",
                     "keywords": ["Apollo 11", "Moon"], "media_type": "video",
                     "nasa_id": "Apollo 11 Overview", "title": "Apollo 11 Overview"}],
           "links": [{"href": "https://images-assets.nasa.gov/video/Apollo 11 Overview/Apollo 11 Overview~thumb.jpg",
                      "rel": "preview", "render": "image"},
                     {"href": "https://images-assets.nasa.gov/video/Apollo 11 Overview/Apollo 11 Overview.srt",
                      "rel": "captions"}]},
          {"href": "https://images-assets.nasa.gov/image/as11-40-5903/collection.json",
           "data": [{"media_type": "image", "nasa_id": "as11-40-5903", "title": "A picture"}]},
          {"href": "x", "data": [{"media_type": "video", "nasa_id": "../escape", "title": "no"}]}
        ],
        "metadata": {"total_hits": 71},
        "links": [{"rel": "next", "prompt": "Next",
                   "href": "http://images-api.nasa.gov/search?q=apollo&media_type=video&page=2"}]
    }}"#;

    #[test]
    fn a_search_asks_for_videos_and_reads_them() {
        assert_eq!(
            search_address("  apollo 11 ", 2, 500).unwrap(),
            "https://images-api.nasa.gov/search?q=apollo%2011&media_type=video&page=2&page_size=100"
        );
        assert_eq!(search_address("  ", 1, 10), Err(Refused::Unknown));
        let page = parse_search(SEARCH.as_bytes()).unwrap();
        assert_eq!(page.total, 71);
        assert!(page.more);
        assert_eq!(page.videos.len(), 1, "a picture and a bad id are left out");
        let v = &page.videos[0];
        assert_eq!(v.nasa_id, "Apollo 11 Overview");
        assert_eq!(
            v.description.as_deref(),
            Some("An overview of the Apollo 11 mission.")
        );
        assert_eq!(v.keywords, ["Apollo 11", "Moon"]);
        assert_eq!(v.created_s, Some(1_247_616_000));
        assert_eq!(
            v.preview.as_deref(),
            Some(
                "https://images-assets.nasa.gov/video/Apollo%2011%20Overview/Apollo%2011%20Overview~thumb.jpg"
            )
        );
        assert_eq!(parse_search(b"[]"), Err(Refused::Malformed));
    }

    /// FAILS IF: the rendition played is not the one the order names, or an
    /// address is kept `http` or with a space in it.
    #[test]
    fn an_item_plays_its_best_rendition_over_https() {
        let assets = br#"[
            "http://images-assets.nasa.gov/video/Apollo 11 Overview/Apollo 11 Overview.srt",
            "http://images-assets.nasa.gov/video/Apollo 11 Overview/Apollo 11 Overview~mobile.mp4",
            "http://images-assets.nasa.gov/video/Apollo 11 Overview/Apollo 11 Overview~orig.mp4",
            "http://images-assets.nasa.gov/video/Apollo 11 Overview/Apollo 11 Overview~medium.mp4",
            "http://images-assets.nasa.gov/video/Apollo 11 Overview/metadata.json"
        ]"#;
        assert_eq!(
            playable(assets).unwrap(),
            "https://images-assets.nasa.gov/video/Apollo%2011%20Overview/Apollo%2011%20Overview~medium.mp4"
        );
        assert_eq!(
            playable(br#"["https://x/a.srt"]"#),
            Err(Refused::NothingPlayable)
        );
        assert_eq!(
            playable(br#"["https://elsewhere.example/a~large.mp4"]"#),
            Err(Refused::NothingPlayable),
            "only the library's own host"
        );
        assert_eq!(playable(b"{}"), Err(Refused::Malformed));
    }

    #[test]
    fn an_item_is_kept_whole_and_told_apart() {
        let page = parse_search(SEARCH.as_bytes()).unwrap();
        let source = crate::table::DEFAULT_SOURCES
            .iter()
            .find(|s| s.id == "nasa")
            .unwrap();
        let native = &source.natives[0];
        let mut h = Harvest::default();
        harvest(&mut h, native, &page);
        let item = &h.items[0];
        assert_eq!(
            item.stream.url,
            "https://images-assets.nasa.gov/video/Apollo%2011%20Overview/collection.json"
        );
        assert_eq!(
            whole_item(&item.stream).as_deref(),
            Some("Apollo 11 Overview")
        );
        assert_eq!(
            whole_item(&Stream::typed("https://images-assets.nasa.gov/x.mp4")),
            None
        );
        assert_eq!(item.year, Some(2009));
        assert_eq!(item.also, ["Apollo 11", "Moon", "HQ"]);
        assert_eq!(h.categories[0].theme.as_deref(), Some(native.theme));
        assert_eq!(assets_address("../x"), None);
    }
}
