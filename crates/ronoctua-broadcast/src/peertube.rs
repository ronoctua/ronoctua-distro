//! PeerTube, through SepiaSearch: free-licensed video from the federation's
//! instances, `video.blender.org` among them, a category at a time.
//!
//! # What is asked
//!
//! SepiaSearch (`sepiasearch.org`) indexes the public instances and answers
//! PeerTube's own search API. Every question carries three filters, and they
//! are what makes this a default source:
//!
//! - `licenceOneOf` 1 to 7: the six Creative Commons licences and the public
//!   domain dedication — a video whose licence is not said is never asked;
//! - `nsfw=false`: nothing an instance marked sensitive;
//! - `categoryOneOf`: the one PeerTube category the native names.
//!
//! The most watched first. The answer is read defensively all the same: an
//! item marked `nsfw`, or with a licence outside the seven, is left out.
//!
//! # What plays
//!
//! A video's watch address on its own instance (`…/videos/watch/<uuid>` or
//! `/w/<id>`), which mpv plays through yt-dlp; a live is kept as a live.
//! Its picture is on that instance too, so the source's posters wait for the
//! person's switch.
//!
//! The format read is the one PeerTube's API documents for
//! `/api/v1/search/videos` (`total`, `data[]`); the fixtures are hand-written
//! from it.

use ronoctua_playlist::own::{Origin, Stream};
use serde_json::Value;

use crate::catalog::{Harvest, ItemKind, NewItem};
use crate::date::{rfc3339, year_of};
use crate::json::{flag, is_web_address, text, whole};
use crate::sources::Refused;
use crate::sources::feed::plain;
use crate::table::Native;

/// Where the index answers.
pub const HOST: &str = "sepiasearch.org";

/// PeerTube's licences a video may have to be asked: the Creative Commons
/// ones (1 to 6) and the public domain dedication (7).
pub const LICENCES: std::ops::RangeInclusive<u16> = 1..=7;

/// PeerTube's categories (its API documents 1 to 18).
pub const CATEGORIES: std::ops::RangeInclusive<u16> = 1..=18;

/// The most videos one page asks for (the index's own limit is 100).
pub const PAGE_MOST: u32 = 100;

/// The address of one page of a category, the most watched first. Pure.
///
/// # Errors
///
/// [`Refused::Unknown`] for a category PeerTube does not document.
pub fn search_address(category: u16, start: u32, count: u32) -> Result<String, Refused> {
    if !CATEGORIES.contains(&category) {
        return Err(Refused::Unknown);
    }
    let licences: String = LICENCES.map(|l| format!("&licenceOneOf={l}")).collect();
    Ok(format!(
        "https://{HOST}/api/v1/search/videos?start={start}&count={}&sort=-views&nsfw=false&categoryOneOf={category}{licences}",
        count.clamp(1, PAGE_MOST)
    ))
}

/// One video the index found.
#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    /// Its uuid.
    pub uuid: String,
    /// Its name.
    pub name: String,
    /// Its watch address on its instance.
    pub url: String,
    /// What it says about itself, markup off.
    pub description: Option<String>,
    /// How long it runs, in seconds (none for a live).
    pub duration_s: Option<u32>,
    /// When it was published, in seconds since the epoch.
    pub published_s: Option<i64>,
    /// How many times it was watched.
    pub views: u64,
    /// Its licence's name.
    pub licence: Option<String>,
    /// Its language's code.
    pub language: Option<String>,
    /// Its picture's address.
    pub thumbnail: Option<String>,
    /// It is a live.
    pub live: bool,
    /// Its channel's name.
    pub channel: Option<String>,
    /// Its instance.
    pub instance: Option<String>,
}

/// One page of a category.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// Its videos.
    pub videos: Vec<Video>,
    /// How many there are in all.
    pub total: u64,
}

/// A page of the index, read. Pure.
///
/// # Errors
///
/// [`Refused::Malformed`] for what is not the index's answer.
pub fn parse_search(bytes: &[u8]) -> Result<Page, Refused> {
    let top: Value = serde_json::from_slice(bytes).map_err(|_| Refused::Malformed)?;
    let data = top
        .get("data")
        .and_then(Value::as_array)
        .ok_or(Refused::Malformed)?;
    let videos = data
        .iter()
        .filter_map(|v| {
            // READ DEFENSIVELY: what was asked away is left out if it came.
            if flag(v, "nsfw").unwrap_or(true) {
                return None;
            }
            let licence = v.get("licence")?;
            let licence_id = whole(licence, "id").and_then(|l| u16::try_from(l).ok())?;
            if !LICENCES.contains(&licence_id) {
                return None;
            }
            let url = text(v, "url").filter(|u| is_web_address(u))?;
            let uuid = text(v, "uuid")?;
            let live = flag(v, "isLive").unwrap_or(false);
            let channel = v.get("channel");
            Some(Video {
                name: text(v, "name").unwrap_or_else(|| uuid.clone()),
                description: text(v, "description")
                    .or_else(|| text(v, "truncatedDescription"))
                    .map(|d| plain(&d))
                    .filter(|d| !d.is_empty()),
                duration_s: whole(v, "duration")
                    .filter(|d| *d > 0 && !live)
                    .and_then(|d| u32::try_from(d).ok()),
                published_s: text(v, "publishedAt").and_then(|d| rfc3339(&d)),
                views: whole(v, "views").unwrap_or(0),
                licence: text(licence, "label"),
                language: v
                    .get("language")
                    .and_then(|l| text(l, "id"))
                    .filter(|l| l != "zxx"),
                thumbnail: text(v, "thumbnailUrl").filter(|u| is_web_address(u)),
                live,
                channel: channel.and_then(|c| text(c, "displayName").or_else(|| text(c, "name"))),
                instance: channel.and_then(|c| text(c, "host")),
                url,
                uuid,
            })
        })
        .collect();
    Ok(Page {
        videos,
        total: whole(&top, "total").unwrap_or(0),
    })
}

/// A page's videos, as items of `native`'s category.
pub fn harvest(h: &mut Harvest, native: &Native, page: &Page) {
    h.category(native.key, native.ask, Some(native.theme), false);
    for v in &page.videos {
        let kind = if v.live {
            ItemKind::Live
        } else {
            ItemKind::Video
        };
        h.items.push(NewItem {
            ext_id: v.uuid.clone(),
            kind,
            title: v.name.clone(),
            also: v.channel.iter().chain(v.instance.iter()).cloned().collect(),
            description: v.description.clone(),
            country: None,
            language: v.language.clone(),
            height: None,
            duration_s: v.duration_s,
            year: v.published_s.map(year_of),
            licence: v.licence.clone(),
            popularity: v.views,
            published_s: v.published_s,
            poster_url: v.thumbnail.clone(),
            stream: Stream {
                url: v.url.clone(),
                title: Some(v.name.clone()),
                referrer: None,
                user_agent: None,
                alternates: Vec::new(),
                origin: Origin::Typed,
                live: v.live,
            },
            categories: vec![native.key.to_owned()],
        });
    }
}

/// Asks one page of a category, on the person's consent for [`HOST`].
///
/// # Errors
///
/// [`Refused::NotLet`], [`Refused::Unknown`] (a category PeerTube does not
/// have; nothing is sent), the network's refusals, and [`parse_search`]'s.
#[cfg(feature = "net")]
pub fn search(
    web: &dyn crate::ask::Web,
    pass: &ronoctua_consent::Pass,
    category: u16,
    start: u32,
) -> Result<Page, Refused> {
    let url = search_address(category, start, PAGE_MOST)?;
    parse_search(&crate::ask::fetched(
        web,
        pass,
        &crate::ask::Fetch::json(url),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HAND-WRITTEN from PeerTube's documented search answer (and the shape
    /// SepiaSearch answered on 2026-09-30, looked at, not kept): a video, a
    /// live, one marked nsfw, one with no licence, one under a licence
    /// outside the seven.
    pub(crate) const SEARCH: &str = r#"{"total": 37843, "data": [
      {"uuid": "c6c70a03-e711-4eed-9835-1d02f5a5ec58", "shortUUID": "qxEodF8nJbTTzkS2mfkrxW",
       "name": "Sprite Fright", "description": "<p>An <b>open</b> movie.</p>",
       "duration": 629, "publishedAt": "2021-10-29T10:00:00.000Z", "views": 193987,
       "category": {"id": 2, "label": "Films"}, "licence": {"id": 1, "label": "CC BY 4.0"},
       "language": {"id": "en", "label": "English"}, "isLive": false, "nsfw": false,
       "thumbnailUrl": "https://video.blender.org/lazy-static/thumbnails/a.jpg",
       "url": "https://video.blender.org/videos/watch/c6c70a03-e711-4eed-9835-1d02f5a5ec58",
       "channel": {"name": "blender", "displayName": "Blender", "host": "video.blender.org"}},
      {"uuid": "11111111-2222-3333-4444-555555555555", "name": "Etna", "duration": 0,
       "publishedAt": "2026-09-27T00:40:21.698Z", "views": 7,
       "licence": {"id": 7, "label": "Public Domain Dedication"},
       "language": {"id": "zxx", "label": "No linguistic content"}, "isLive": true, "nsfw": false,
       "url": "https://garr.tv/videos/watch/11111111-2222-3333-4444-555555555555",
       "channel": {"name": "ingv", "host": "garr.tv"}},
      {"uuid": "a", "name": "Marked", "licence": {"id": 1}, "nsfw": true,
       "url": "https://x.example/videos/watch/a"},
      {"uuid": "b", "name": "No licence", "licence": {"id": null}, "nsfw": false,
       "url": "https://x.example/videos/watch/b"},
      {"uuid": "c", "name": "Other licence", "licence": {"id": 9}, "nsfw": false,
       "url": "https://x.example/videos/watch/c"}
    ]}"#;

    #[test]
    fn a_category_is_asked_free_licensed_and_safe() {
        assert_eq!(
            search_address(2, 100, 500).unwrap(),
            "https://sepiasearch.org/api/v1/search/videos?start=100&count=100&sort=-views&nsfw=false&categoryOneOf=2\
             &licenceOneOf=1&licenceOneOf=2&licenceOneOf=3&licenceOneOf=4&licenceOneOf=5&licenceOneOf=6&licenceOneOf=7"
        );
        assert_eq!(search_address(19, 0, 10), Err(Refused::Unknown));
        assert_eq!(search_address(0, 0, 10), Err(Refused::Unknown));
    }

    #[test]
    fn only_free_safe_videos_are_read() {
        let page = parse_search(SEARCH.as_bytes()).unwrap();
        assert_eq!(page.total, 37843);
        let names: Vec<&str> = page.videos.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, ["Sprite Fright", "Etna"]);
        let film = &page.videos[0];
        assert_eq!(film.duration_s, Some(629));
        assert_eq!(film.licence.as_deref(), Some("CC BY 4.0"));
        assert_eq!(film.language.as_deref(), Some("en"));
        assert_eq!(film.description.as_deref(), Some("An open movie."));
        assert_eq!(film.channel.as_deref(), Some("Blender"));
        assert_eq!(film.instance.as_deref(), Some("video.blender.org"));
        let live = &page.videos[1];
        assert!(live.live);
        assert_eq!(live.duration_s, None);
        assert_eq!(live.language, None, "no linguistic content is no language");
        assert_eq!(parse_search(b"{}"), Err(Refused::Malformed));
    }

    #[test]
    fn a_page_is_items_of_its_native_category() {
        let source = crate::table::DEFAULT_SOURCES
            .iter()
            .find(|s| s.id == "peertube")
            .unwrap();
        let films = source
            .natives
            .iter()
            .find(|n| n.ask == "2")
            .expect("declared");
        let mut h = Harvest::default();
        harvest(&mut h, films, &parse_search(SEARCH.as_bytes()).unwrap());
        assert_eq!(h.categories[0].theme.as_deref(), Some("films"));
        assert_eq!(h.items[0].popularity, 193_987);
        assert_eq!(h.items[0].year, Some(2021));
        assert_eq!(h.items[1].kind, ItemKind::Live);
        assert!(h.items[1].stream.live);
    }
}
