//! Wikimedia Commons: free-licensed video, a category at a time.
//!
//! # One question per page
//!
//! The MediaWiki API, `generator=categorymembers` over the category's FILES
//! with `prop=imageinfo`: every member comes back with its address, its media
//! type, its size and length, a thumbnail and the licence and description
//! Commons keeps for it. Only the files directly in the category are asked —
//! not its sub-categories, which is why `assets/channels.nix` names
//! categories that were checked to hold videos themselves. A page ends with a
//! `continue` token for the next ([`Page::next`]).
//!
//! # What plays
//!
//! A member whose media type is `VIDEO` (or `AUDIO`), whose MIME type says so
//! (a GIF is `BITMAP`, and never one); it plays from `upload.wikimedia.org`,
//! the original file (WebM or Ogg, which mpv plays). Every file on Commons is
//! free-licensed or in the public domain — that is what Commons admits — and
//! each item carries the licence its page names.
//!
//! The formats read are the API's documented ones
//! (<https://www.mediawiki.org/wiki/API:Imageinfo>, `formatversion=2`); the
//! fixtures are hand-written from them.

use ronoctua_playlist::own::{Origin, Stream};
use serde_json::Value;

use crate::catalog::{Harvest, ItemKind, NewItem};
use crate::date::{rfc3339, year_of};
use crate::json::{is_web_address, number, text, whole};
use crate::query::escape;
use crate::sources::Refused;
use crate::sources::feed::plain;
use crate::table::Native;

/// Where the API answers.
pub const HOST: &str = "commons.wikimedia.org";

/// Where the files and their thumbnails are.
pub const MEDIA_HOST: &str = "upload.wikimedia.org";

/// The most members one page asks for (the API's limit for a client without
/// a bot flag is 500; the imageinfo of each is the heavy half).
pub const PAGE_MOST: u32 = 100;

/// The width a thumbnail is asked at, in pixels: a poster in a grid.
pub const THUMBNAIL_WIDTH: u32 = 320;

/// Whether `name` can be a category's name: printable, none of the characters
/// a title cannot hold, no namespace.
#[must_use]
pub fn is_category(name: &str) -> bool {
    let name = name.trim();
    (1..=200).contains(&name.len())
        && !name.contains(['#', '<', '>', '[', ']', '|', '{', '}', ':'])
        && !name.chars().any(char::is_control)
}

/// The address of one page of a category's files. Pure.
///
/// # Errors
///
/// [`Refused::Unknown`] for a name that is not a category's, or a
/// continuation token with a control character.
pub fn members_address(
    category: &str,
    continuation: Option<&str>,
    rows: u32,
) -> Result<String, Refused> {
    if !is_category(category) {
        return Err(Refused::Unknown);
    }
    let title = format!("Category:{}", category.trim().replace(' ', "_"));
    let mut url = format!(
        "https://{HOST}/w/api.php?action=query&format=json&formatversion=2\
         &generator=categorymembers&gcmtitle={}&gcmtype=file&gcmlimit={}\
         &prop=imageinfo&iiprop={}&iiurlwidth={THUMBNAIL_WIDTH}&iiextmetadatafilter={}",
        escape(&title),
        rows.clamp(1, PAGE_MOST),
        escape("url|mime|size|mediatype|extmetadata|timestamp"),
        escape("LicenseShortName|ImageDescription"),
    );
    if let Some(token) = continuation {
        if token.chars().any(char::is_control) {
            return Err(Refused::Unknown);
        }
        url.push_str(&format!(
            "&continue={}&gcmcontinue={}",
            escape("gcmcontinue||"),
            escape(token)
        ));
    }
    Ok(url)
}

/// One file of a category.
#[derive(Debug, Clone, PartialEq)]
pub struct File {
    /// Its page's id: what it is known by.
    pub page_id: u64,
    /// Its name, without `File:` and its extension, `_` as spaces.
    pub title: String,
    /// The file's own address.
    pub url: String,
    /// A picture or a sound.
    pub kind: ItemKind,
    /// Its picture's height, when it has one.
    pub height: Option<u16>,
    /// How long it runs, in seconds.
    pub duration_s: Option<u32>,
    /// A thumbnail's address.
    pub thumbnail: Option<String>,
    /// The licence its page names (`CC BY-SA 4.0`, `Public domain`).
    pub licence: Option<String>,
    /// Its description, markup off.
    pub description: Option<String>,
    /// When it was uploaded, in seconds since the epoch.
    pub uploaded_s: Option<i64>,
}

/// One page of a category.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// Its files that play.
    pub files: Vec<File>,
    /// The token that asks for the next page; `None` on the last.
    pub next: Option<String>,
}

/// The name of a file's page, as a title: `File:Old_film.webm` is
/// `Old film`.
fn title_of(page_title: &str) -> String {
    let name = page_title.strip_prefix("File:").unwrap_or(page_title);
    let name = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    name.replace('_', " ").trim().to_owned()
}

/// A value of the page's `extmetadata`.
fn meta(info: &Value, name: &str) -> Option<String> {
    let v = info.get("extmetadata")?.get(name)?;
    text(v, "value")
}

/// A page of a category, read. Pure.
///
/// # Errors
///
/// [`Refused::Malformed`] for what is not the API's answer.
pub fn parse_members(bytes: &[u8]) -> Result<Page, Refused> {
    let top: Value = serde_json::from_slice(bytes).map_err(|_| Refused::Malformed)?;
    if top.get("error").is_some() {
        return Err(Refused::Declined);
    }
    // AN EMPTY CATEGORY answers no `query` at all.
    let pages = match top.get("query") {
        None => Vec::new(),
        Some(q) => q
            .get("pages")
            .and_then(Value::as_array)
            .cloned()
            .ok_or(Refused::Malformed)?,
    };
    let files = pages
        .iter()
        .filter_map(|p| {
            let info = p.get("imageinfo")?.as_array()?.first()?;
            let kind = match text(info, "mediatype").as_deref() {
                Some("VIDEO") => ItemKind::Video,
                Some("AUDIO") => ItemKind::Audio,
                _ => return None,
            };
            let mime = text(info, "mime").unwrap_or_default();
            if !(mime.starts_with("video/") || mime.starts_with("audio/")) {
                return None;
            }
            let url = text(info, "url").filter(|u| is_web_address(u))?;
            if crate::ask::host_of(&url).as_deref() != Some(MEDIA_HOST) {
                return None;
            }
            Some(File {
                page_id: whole(p, "pageid")?,
                title: title_of(&text(p, "title")?),
                url,
                kind,
                height: whole(info, "height")
                    .and_then(|h| u16::try_from(h).ok())
                    .filter(|h| *h > 0),
                duration_s: number(info, "duration")
                    .filter(|d| *d >= 1.0 && *d < f64::from(u32::MAX))
                    .map(|d| d as u32),
                thumbnail: text(info, "thumburl").filter(|u| is_web_address(u)),
                licence: meta(info, "LicenseShortName").map(|l| plain(&l)),
                description: meta(info, "ImageDescription")
                    .map(|d| plain(&d))
                    .filter(|d| !d.is_empty()),
                uploaded_s: text(info, "timestamp").and_then(|t| rfc3339(&t)),
            })
        })
        .collect();
    let next = top.get("continue").and_then(|c| text(c, "gcmcontinue"));
    Ok(Page { files, next })
}

/// A page's files, as items of `native`'s category.
pub fn harvest(h: &mut Harvest, native: &Native, page: &Page) {
    h.category(native.key, native.ask, Some(native.theme), false);
    for f in &page.files {
        h.items.push(NewItem {
            ext_id: f.page_id.to_string(),
            kind: f.kind,
            title: f.title.clone(),
            also: Vec::new(),
            description: f.description.clone(),
            country: None,
            language: None,
            height: f.height,
            duration_s: f.duration_s,
            year: f.uploaded_s.map(year_of),
            licence: f.licence.clone(),
            popularity: 0,
            published_s: f.uploaded_s,
            poster_url: f.thumbnail.clone(),
            stream: Stream {
                url: f.url.clone(),
                title: Some(f.title.clone()),
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

/// Asks one page of a category, on the person's consent for [`HOST`].
///
/// # Errors
///
/// [`Refused::NotLet`], [`Refused::Unknown`] (not a category; nothing is
/// sent), the network's refusals, and [`parse_members`]'s.
#[cfg(feature = "net")]
pub fn members(
    web: &dyn crate::ask::Web,
    pass: &ronoctua_consent::Pass,
    category: &str,
    continuation: Option<&str>,
) -> Result<Page, Refused> {
    let url = members_address(category, continuation, PAGE_MOST)?;
    parse_members(&crate::ask::fetched(
        web,
        pass,
        &crate::ask::Fetch::json(url),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HAND-WRITTEN from the API's documented `formatversion=2` answer: a
    /// video, a sound, a GIF (never played), a file on another host, and the
    /// `continue` token.
    pub(crate) const MEMBERS: &str = r#"{
      "batchcomplete": true,
      "continue": {"gcmcontinue": "file|4f4c44|123", "continue": "gcmcontinue||"},
      "query": {"pages": [
        {"pageid": 1001, "ns": 6, "title": "File:Cat_playing_piano.webm",
         "imagerepository": "local",
         "imageinfo": [{"timestamp": "2014-05-06T07:08:09Z", "size": 5242880,
            "width": 1280, "height": 720, "duration": 94.8,
            "thumburl": "https://upload.wikimedia.org/wikipedia/commons/thumb/a/ab/Cat_playing_piano.webm/320px--Cat_playing_piano.webm.jpg",
            "url": "https://upload.wikimedia.org/wikipedia/commons/a/ab/Cat_playing_piano.webm",
            "mime": "video/webm", "mediatype": "VIDEO",
            "extmetadata": {
              "LicenseShortName": {"value": "CC BY-SA 4.0", "source": "commons-desc-page"},
              "ImageDescription": {"value": "<div class=\"description en\">A cat, <i>playing</i>.</div>"}
            }}]},
        {"pageid": 1002, "ns": 6, "title": "File:Purr.ogg",
         "imageinfo": [{"url": "https://upload.wikimedia.org/wikipedia/commons/b/bc/Purr.ogg",
            "mime": "audio/ogg", "mediatype": "AUDIO", "duration": 12,
            "extmetadata": {"LicenseShortName": {"value": "Public domain"}}}]},
        {"pageid": 1003, "ns": 6, "title": "File:Cat.gif",
         "imageinfo": [{"url": "https://upload.wikimedia.org/wikipedia/commons/c/cd/Cat.gif",
            "mime": "image/gif", "mediatype": "BITMAP"}]},
        {"pageid": 1004, "ns": 6, "title": "File:Elsewhere.webm",
         "imageinfo": [{"url": "https://mirror.example/Elsewhere.webm",
            "mime": "video/webm", "mediatype": "VIDEO"}]}
      ]}
    }"#;

    #[test]
    fn a_category_s_files_are_asked_with_their_licence_and_length() {
        let url = members_address("Videos of cats", None, 1000).unwrap();
        assert!(
            url.starts_with("https://commons.wikimedia.org/w/api.php?action=query&format=json&formatversion=2&generator=categorymembers&gcmtitle=Category%3AVideos_of_cats&gcmtype=file&gcmlimit=100"),
            "{url}"
        );
        assert!(url.contains("&iiprop=url%7Cmime%7Csize%7Cmediatype%7Cextmetadata%7Ctimestamp"));
        let next = members_address("Videos of cats", Some("file|4f4c44|123"), 10).unwrap();
        assert!(next.ends_with("&continue=gcmcontinue%7C%7C&gcmcontinue=file%7C4f4c44%7C123"));
        assert_eq!(
            members_address("Category:Cats", None, 10),
            Err(Refused::Unknown)
        );
        assert_eq!(members_address("a|b", None, 10), Err(Refused::Unknown));
    }

    #[test]
    fn only_what_plays_from_commons_own_host_is_kept() {
        let page = parse_members(MEMBERS.as_bytes()).unwrap();
        assert_eq!(page.next.as_deref(), Some("file|4f4c44|123"));
        assert_eq!(
            page.files.len(),
            2,
            "the GIF and the other host are left out"
        );
        let cat = &page.files[0];
        assert_eq!(cat.title, "Cat playing piano");
        assert_eq!(cat.kind, ItemKind::Video);
        assert_eq!(cat.height, Some(720));
        assert_eq!(cat.duration_s, Some(94));
        assert_eq!(cat.licence.as_deref(), Some("CC BY-SA 4.0"));
        assert_eq!(cat.description.as_deref(), Some("A cat, playing."));
        assert_eq!(cat.uploaded_s, Some(1_399_360_089));
        let purr = &page.files[1];
        assert_eq!(purr.kind, ItemKind::Audio);
        assert_eq!(purr.licence.as_deref(), Some("Public domain"));
        let empty = parse_members(br#"{"batchcomplete": true}"#).unwrap();
        assert!(empty.files.is_empty() && empty.next.is_none());
        assert_eq!(
            parse_members(br#"{"error": {"code": "badvalue"}}"#),
            Err(Refused::Declined)
        );
        assert_eq!(parse_members(b"<html>"), Err(Refused::Malformed));
    }

    #[test]
    fn a_page_is_items_of_its_native_category() {
        let source = crate::table::DEFAULT_SOURCES
            .iter()
            .find(|s| s.id == "commons")
            .unwrap();
        let cats = source
            .natives
            .iter()
            .find(|n| n.ask == "Videos of cats")
            .expect("declared");
        let mut h = Harvest::default();
        harvest(&mut h, cats, &parse_members(MEMBERS.as_bytes()).unwrap());
        assert_eq!(h.categories[0].theme.as_deref(), Some("nature"));
        assert_eq!(h.items[0].ext_id, "1001");
        assert_eq!(h.items[0].year, Some(2014));
        assert!(
            h.items[0]
                .poster_url
                .as_deref()
                .unwrap()
                .starts_with("https://upload.wikimedia.org/")
        );
    }
}
