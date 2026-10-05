//! A person's feed — a podcast's RSS, an Atom feed, a channel's video feed,
//! RSS 1.0 — read into categories and items.
//!
//! # What plays
//!
//! An item plays when it carries something to play, in this order: its
//! `enclosure` (RSS) or `link rel="enclosure"` (Atom) of a sound or a
//! picture; a `media:content` of one; a YouTube video it names (`yt:videoId`,
//! or a link to a watch page), which mpv plays through yt-dlp. An item that
//! is only text is left out.
//!
//! # What else is read
//!
//! Its title, its id (`guid`, Atom's `id`), when it was published (RFC 822 in
//! RSS, RFC 3339 in Atom and in `dc:date`), how long it runs
//! (`itunes:duration`, `media:content`'s `duration`), its picture
//! (`itunes:image`, `media:thumbnail`, else the feed's own), its description
//! (for the search, markup taken off) and its categories.
//!
//! # Its categories
//!
//! The feed itself is one category (key [`FEED`], titled with the feed's
//! title) holding every item, filed under the theme its own categories name
//! (`itunes:category`, `category`). Each category an item names is one more,
//! filed by its own words.
//!
//! Through `ronoctua-xml`'s depth guard, as every XML from outside is: a feed
//! is a text anyone could have written.

use ronoctua_playlist::own::{Origin, Stream};
use ronoctua_xml::roxmltree::{Document, Node, ParsingOptions};

use super::{Refused, is_adult, theme_of};
use crate::catalog::{Harvest, ItemKind, NewItem, digest};
use crate::date::{clock_seconds, rfc822, rfc3339, year_of};
use crate::json::is_web_address;

/// The key of the category every item of a feed is in.
pub const FEED: &str = "feed";

/// The namespaces a feed's extensions are written in.
const ITUNES: &str = "http://www.itunes.com/dtds/podcast-1.0.dtd";
const MEDIA: &str = "http://search.yahoo.com/mrss/";
const YOUTUBE: &str = "http://www.youtube.com/xml/schemas/2015";
const DUBLIN_CORE: &str = "http://purl.org/dc/elements/1.1/";

/// A feed, read.
#[derive(Debug, Clone, PartialEq)]
pub struct Fed {
    /// What the feed calls itself (data); empty when it says nothing.
    pub title: String,
    /// Its categories and items.
    pub harvest: Harvest,
}

/// Whether `n` is the element `name` in namespace `ns` (`None`: no namespace,
/// or the feed's own — RSS has none, Atom's is its default).
fn is(n: &Node<'_, '_>, name: &str, ns: Option<&str>) -> bool {
    n.is_element()
        && n.tag_name().name() == name
        && match ns {
            Some(ns) => n.tag_name().namespace() == Some(ns),
            None => !matches!(
                n.tag_name().namespace(),
                Some(ITUNES | MEDIA | YOUTUBE | DUBLIN_CORE)
            ),
        }
}

/// The first child `name` in `ns`.
fn child<'a, 'i>(n: &Node<'a, 'i>, name: &str, ns: Option<&str>) -> Option<Node<'a, 'i>> {
    n.children().find(|c| is(c, name, ns))
}

/// The trimmed text of the first child `name` in `ns`, when it has any.
fn text_of(n: &Node<'_, '_>, name: &str, ns: Option<&str>) -> Option<String> {
    let c = child(n, name, ns)?;
    let t: String = c
        .descendants()
        .filter(Node::is_text)
        .filter_map(|t| t.text())
        .collect();
    let t = t.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

/// Markup taken off a description, and the few entities feeds double-escape.
#[must_use]
pub fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut inside = false;
    for c in html.chars() {
        match c {
            '<' => {
                inside = true;
                out.push(' ');
            }
            '>' if inside => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    let joined = out.split_whitespace().collect::<Vec<_>>().join(" ");
    // A TAG BEFORE A MARK left a space before it (`<i>playing</i>.`).
    let mut tidy = String::with_capacity(joined.len());
    for c in joined.chars() {
        if matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')') && tidy.ends_with(' ') {
            tidy.pop();
        }
        tidy.push(c);
    }
    tidy
}

/// A YouTube video's id in an address that plays one.
fn watch_id(url: &str) -> Option<String> {
    let host = crate::ask::host_of(url)?;
    let id = if host == "youtu.be" {
        super::path_of(url).trim_start_matches('/').to_owned()
    } else if host.ends_with("youtube.com") {
        super::query_param(url, "v")?.to_owned()
    } else {
        return None;
    };
    (id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
    .then_some(id)
}

/// What an item plays, and what it is.
struct Playing {
    url: String,
    kind: ItemKind,
    origin: Origin,
    duration_s: Option<u32>,
}

/// A media type's word for what it is.
fn kind_of_type(mime: &str, medium: Option<&str>) -> Option<ItemKind> {
    let m = mime.to_ascii_lowercase();
    if m.starts_with("audio/") || medium == Some("audio") {
        Some(ItemKind::Audio)
    } else if m.starts_with("video/") || medium == Some("video") {
        Some(ItemKind::Video)
    } else {
        None
    }
}

/// What an item plays, when it plays anything.
fn playing(item: &Node<'_, '_>) -> Option<Playing> {
    // AN ENCLOSURE: RSS's element, or Atom's link.
    for c in item.children().filter(Node::is_element) {
        let enclosure = if is(&c, "enclosure", None) {
            Some((c.attribute("url"), c.attribute("type")))
        } else if is(&c, "link", None) && c.attribute("rel") == Some("enclosure") {
            Some((c.attribute("href"), c.attribute("type")))
        } else {
            None
        };
        if let Some((Some(url), mime)) = enclosure {
            let url = url.trim();
            if is_web_address(url) {
                // A PODCAST'S ENCLOSURE with no type is a sound more often
                // than not; one of a picture says so.
                let kind = kind_of_type(mime.unwrap_or(""), None).unwrap_or(ItemKind::Audio);
                return Some(Playing {
                    url: url.to_owned(),
                    kind,
                    origin: Origin::Typed,
                    duration_s: None,
                });
            }
        }
    }
    // A MEDIA RSS CONTENT, on the item or in its group.
    let groups =
        std::iter::once(*item).chain(item.children().filter(|c| is(c, "group", Some(MEDIA))));
    for g in groups {
        for c in g.children().filter(|c| is(c, "content", Some(MEDIA))) {
            let (Some(url), kind) = (
                c.attribute("url"),
                kind_of_type(c.attribute("type").unwrap_or(""), c.attribute("medium")),
            ) else {
                continue;
            };
            if let (true, Some(kind)) = (is_web_address(url.trim()), kind) {
                return Some(Playing {
                    url: url.trim().to_owned(),
                    kind,
                    origin: Origin::Typed,
                    duration_s: c.attribute("duration").and_then(clock_seconds),
                });
            }
        }
    }
    // A YOUTUBE VIDEO, by its id or a link to it.
    let id = text_of(item, "videoId", Some(YOUTUBE)).or_else(|| {
        item.children()
            .filter(|c| is(c, "link", None))
            .filter_map(|c| {
                c.attribute("href")
                    .map(ToOwned::to_owned)
                    .or_else(|| c.text().map(|t| t.trim().to_owned()))
            })
            .find_map(|u| watch_id(&u))
    })?;
    Some(Playing {
        url: format!("https://www.youtube.com/watch?v={id}"),
        kind: ItemKind::Video,
        origin: Origin::YouTube { id },
        duration_s: None,
    })
}

/// A picture an element names: `itunes:image href`, `media:thumbnail url`,
/// RSS's `image/url`, Atom's `logo`.
fn picture(n: &Node<'_, '_>) -> Option<String> {
    let candidates = [
        child(n, "image", Some(ITUNES)).and_then(|c| c.attribute("href").map(ToOwned::to_owned)),
        child(n, "thumbnail", Some(MEDIA)).and_then(|c| c.attribute("url").map(ToOwned::to_owned)),
        child(n, "group", Some(MEDIA))
            .and_then(|g| child(&g, "thumbnail", Some(MEDIA)))
            .and_then(|c| c.attribute("url").map(ToOwned::to_owned)),
        child(n, "image", None).and_then(|i| text_of(&i, "url", None)),
        text_of(n, "logo", None),
    ];
    candidates
        .into_iter()
        .flatten()
        .map(|u| u.trim().to_owned())
        .find(|u| is_web_address(u))
}

/// The categories an element names: RSS's `category` text, Atom's `term`
/// (or `label`), `itunes:category`'s `text` (nested ones too).
fn categories(n: &Node<'_, '_>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in n.children().filter(Node::is_element) {
        let found: Vec<String> = if is(&c, "category", None) {
            c.attribute("label")
                .or_else(|| c.attribute("term"))
                .map(ToOwned::to_owned)
                .or_else(|| c.text().map(ToOwned::to_owned))
                .into_iter()
                .collect()
        } else if is(&c, "category", Some(ITUNES)) {
            c.descendants()
                .filter(|d| is(d, "category", Some(ITUNES)))
                .filter_map(|d| d.attribute("text").map(ToOwned::to_owned))
                .collect()
        } else {
            Vec::new()
        };
        for f in found {
            let f = f.trim().to_owned();
            if !f.is_empty() && !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out
}

/// When an item was published: `pubDate`, `published`, `updated`, `dc:date`.
fn published(item: &Node<'_, '_>) -> Option<i64> {
    text_of(item, "pubDate", None)
        .and_then(|d| rfc822(&d))
        .or_else(|| text_of(item, "published", None).and_then(|d| rfc3339(&d)))
        .or_else(|| text_of(item, "updated", None).and_then(|d| rfc3339(&d)))
        .or_else(|| text_of(item, "date", Some(DUBLIN_CORE)).and_then(|d| rfc3339(&d)))
}

/// A feed's text, read into its categories and items (at most `most`). Pure.
///
/// # Errors
///
/// [`Refused::Malformed`] for text that is not XML (or nests past the
/// guard), [`Refused::Unknown`] for XML that is not a feed,
/// [`Refused::NothingPlayable`] for a feed of text only.
pub fn feed(xml: &str, most: usize) -> Result<Fed, Refused> {
    let doc: Document<'_> =
        ronoctua_xml::parse(xml, ParsingOptions::default()).map_err(|_| Refused::Malformed)?;
    let root = doc.root_element();
    let local = root.tag_name().name();
    // THE CHANNEL: RSS's `channel`, Atom's `feed` itself, RSS 1.0's `channel`
    // beside its items.
    let (channel, items): (Node<'_, '_>, Vec<Node<'_, '_>>) = match local {
        "rss" => {
            let channel = child(&root, "channel", None).ok_or(Refused::Unknown)?;
            let items = channel.children().filter(|c| is(c, "item", None)).collect();
            (channel, items)
        }
        "feed" => (
            root,
            root.children().filter(|c| is(c, "entry", None)).collect(),
        ),
        "RDF" => {
            let channel = child(&root, "channel", None).ok_or(Refused::Unknown)?;
            (
                channel,
                root.children().filter(|c| is(c, "item", None)).collect(),
            )
        }
        _ => return Err(Refused::Unknown),
    };
    let title = text_of(&channel, "title", None).unwrap_or_default();
    let feed_picture = picture(&channel);
    let feed_categories = categories(&channel);
    let theme = feed_categories
        .iter()
        .find_map(|c| theme_of(c))
        .or_else(|| theme_of(&title));
    let adult = is_adult(&title) || feed_categories.iter().any(|c| is_adult(c));
    let mut h = Harvest::default();
    h.category(FEED, &title, theme, adult);

    for item in items {
        if h.items.len() >= most {
            h.cut = true;
            break;
        }
        let Some(play) = playing(&item) else {
            continue;
        };
        let item_title = text_of(&item, "title", None)
            .map(|t| plain(&t))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| crate::sources::shown(&play.url));
        let ext_id = text_of(&item, "guid", None)
            .or_else(|| text_of(&item, "id", None))
            .unwrap_or_else(|| format!("u{}", digest(&play.url)));
        let description = text_of(&item, "summary", Some(ITUNES))
            .or_else(|| text_of(&item, "description", None))
            .or_else(|| text_of(&item, "summary", None))
            .or_else(|| text_of(&item, "content", None))
            .or_else(|| {
                child(&item, "group", Some(MEDIA))
                    .and_then(|g| text_of(&g, "description", Some(MEDIA)))
            })
            .map(|d| plain(&d))
            .filter(|d| !d.is_empty());
        let when = published(&item);
        let duration_s = text_of(&item, "duration", Some(ITUNES))
            .and_then(|d| clock_seconds(&d))
            .or(play.duration_s);
        let mut keys = vec![FEED.to_owned()];
        for c in categories(&item) {
            let key = format!("c:{c}");
            h.category(&key, &c, theme_of(&c), is_adult(&c));
            keys.push(key);
        }
        let also: Vec<String> = [
            text_of(&item, "author", Some(ITUNES)),
            child(&item, "author", None).and_then(|a| text_of(&a, "name", None)),
        ]
        .into_iter()
        .flatten()
        .collect();
        h.items.push(NewItem {
            ext_id,
            kind: play.kind,
            also,
            description,
            country: None,
            language: text_of(&channel, "language", None),
            height: None,
            duration_s,
            year: when.map(year_of),
            licence: None,
            popularity: 0,
            published_s: when,
            poster_url: picture(&item).or_else(|| feed_picture.clone()),
            stream: Stream {
                url: play.url,
                title: Some(item_title.clone()),
                referrer: None,
                user_agent: None,
                alternates: Vec::new(),
                origin: play.origin,
                live: false,
            },
            categories: keys,
            title: item_title,
        });
    }
    if h.items.is_empty() {
        return Err(Refused::NothingPlayable);
    }
    Ok(Fed { title, harvest: h })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A HAND-WRITTEN podcast in the shape Apple's podcast specification and
    /// RSS 2.0 document: `itunes:*`, enclosures, a `guid`, a nested
    /// `itunes:category`, an item that is only text.
    const PODCAST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"
     xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel>
    <title>Ciência Hoje</title>
    <language>pt-BR</language>
    <itunes:image href="https://cdn.example/show.jpg"/>
    <itunes:category text="Science"><itunes:category text="Astronomy"/></itunes:category>
    <item>
      <title>Episódio 2: &lt;b&gt;Marte&lt;/b&gt;</title>
      <guid isPermaLink="false">ep-2</guid>
      <pubDate>Tue, 10 Jun 2003 04:00:00 GMT</pubDate>
      <description>&lt;p&gt;Sobre &amp;amp; Marte&lt;/p&gt;</description>
      <enclosure url="https://cdn.example/ep2.mp3" length="123" type="audio/mpeg"/>
      <itunes:duration>48:22</itunes:duration>
      <itunes:image href="https://cdn.example/ep2.jpg"/>
      <category>Space</category>
    </item>
    <item>
      <title>Only words</title>
      <guid>ep-text</guid>
    </item>
    <item>
      <title>Episódio 1</title>
      <enclosure url="https://cdn.example/ep1.mp4" type="video/mp4"/>
      <itunes:duration>3723</itunes:duration>
    </item>
  </channel>
</rss>"#;

    /// A HAND-WRITTEN YouTube channel feed in the shape YouTube publishes at
    /// `feeds/videos.xml` (Atom with `yt:` and `media:group`).
    const YOUTUBE_FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015"
      xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">
  <title>Blender</title>
  <entry>
    <id>yt:video:aaaaaaaaaaa</id>
    <yt:videoId>aaaaaaaaaaa</yt:videoId>
    <title>Blender 5.0 Reel</title>
    <link rel="alternate" href="https://www.youtube.com/watch?v=aaaaaaaaaaa"/>
    <author><name>Blender</name></author>
    <published>2026-09-27T00:40:21+00:00</published>
    <media:group>
      <media:title>Blender 5.0 Reel</media:title>
      <media:content url="https://www.youtube.com/v/aaaaaaaaaaa?version=3" type="application/x-shockwave-flash" width="640" height="390"/>
      <media:thumbnail url="https://i.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg" width="480" height="360"/>
      <media:description>What is new.</media:description>
    </media:group>
  </entry>
  <entry>
    <id>tag:example,2026:1</id>
    <title>An Atom enclosure</title>
    <link rel="enclosure" href="https://files.example/talk.webm" type="video/webm"/>
    <updated>2026-01-02T03:04:05Z</updated>
    <category term="education" label="Education"/>
  </entry>
</feed>"#;

    #[test]
    fn a_podcast_plays_its_enclosures_under_its_theme() {
        let fed = feed(PODCAST, 100).unwrap();
        assert_eq!(fed.title, "Ciência Hoje");
        let h = &fed.harvest;
        let whole = &h.categories[0];
        assert_eq!(
            (whole.key.as_str(), whole.title.as_str()),
            (FEED, "Ciência Hoje")
        );
        assert_eq!(
            whole.theme.as_deref(),
            Some("science"),
            "by its itunes:category"
        );
        assert_eq!(h.items.len(), 2, "the item of words only is left out");
        let two = &h.items[0];
        assert_eq!(two.title, "Episódio 2: Marte", "markup off the title");
        assert_eq!(two.ext_id, "ep-2");
        assert_eq!(two.kind, ItemKind::Audio);
        assert_eq!(two.stream.url, "https://cdn.example/ep2.mp3");
        assert_eq!(two.duration_s, Some(2902));
        assert_eq!(two.published_s, Some(1_055_217_600));
        assert_eq!(two.year, Some(2003));
        assert_eq!(
            two.poster_url.as_deref(),
            Some("https://cdn.example/ep2.jpg")
        );
        assert_eq!(two.description.as_deref(), Some("Sobre & Marte"));
        assert_eq!(two.language.as_deref(), Some("pt-BR"));
        assert_eq!(two.categories, [FEED, "c:Space"]);
        let space = h.categories.iter().find(|c| c.key == "c:Space").unwrap();
        assert_eq!(space.theme.as_deref(), Some("space"));
        let one = &h.items[1];
        assert_eq!(one.kind, ItemKind::Video, "its type says a picture");
        assert_eq!(one.duration_s, Some(3723));
        assert_eq!(
            one.poster_url.as_deref(),
            Some("https://cdn.example/show.jpg"),
            "the show's picture"
        );
        assert!(one.ext_id.starts_with('u'), "no guid: a digest");
    }

    /// FAILS IF: a YouTube channel's own feed, pasted at the address YouTube
    /// publishes it at, is refused for its host before its bytes are read.
    #[test]
    fn a_youtube_feed_plays_its_videos_and_an_atom_enclosure_plays_too() {
        let at = "https://www.youtube.com/feeds/videos.xml?channel_id=UCSMOQeBJ2RAnuFungnQOxLg";
        assert_eq!(
            crate::sources::sniff(at, &YOUTUBE_FEED.as_bytes()[..200]),
            Ok(crate::sources::Sniffed::Feed)
        );
        let fed = feed(YOUTUBE_FEED, 100).unwrap();
        assert_eq!(fed.title, "Blender");
        let h = &fed.harvest;
        assert_eq!(h.items.len(), 2);
        let reel = &h.items[0];
        assert_eq!(
            reel.stream.url,
            "https://www.youtube.com/watch?v=aaaaaaaaaaa"
        );
        assert_eq!(
            reel.stream.origin,
            Origin::YouTube {
                id: "aaaaaaaaaaa".to_owned()
            }
        );
        assert_eq!(
            reel.poster_url.as_deref(),
            Some("https://i.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg")
        );
        assert_eq!(reel.description.as_deref(), Some("What is new."));
        assert_eq!(reel.also, ["Blender"]);
        assert_eq!(reel.published_s, Some(1_790_469_621));
        let talk = &h.items[1];
        assert_eq!(talk.stream.url, "https://files.example/talk.webm");
        assert_eq!(talk.kind, ItemKind::Video);
        let education = h
            .categories
            .iter()
            .find(|c| c.key == "c:Education")
            .unwrap();
        assert_eq!(education.theme.as_deref(), Some("education"));
    }

    #[test]
    fn what_is_not_a_feed_is_refused_and_a_feed_is_bounded() {
        assert_eq!(feed("not xml", 10).unwrap_err(), Refused::Malformed);
        assert_eq!(
            feed("<html><body/></html>", 10).unwrap_err(),
            Refused::Unknown
        );
        let words = "<rss><channel><title>t</title><item><title>x</title></item></channel></rss>";
        assert_eq!(feed(words, 10).unwrap_err(), Refused::NothingPlayable);
        let deep = format!("<rss>{}{}</rss>", "<a>".repeat(200), "</a>".repeat(200));
        assert_eq!(
            feed(&deep, 10).unwrap_err(),
            Refused::Malformed,
            "the guard"
        );
        let one = feed(PODCAST, 1).unwrap();
        assert_eq!(one.harvest.items.len(), 1);
        assert!(one.harvest.cut);
    }

    #[test]
    fn markup_is_taken_off_a_description() {
        assert_eq!(
            plain("<p>One&nbsp;<b>two</b></p>\n three &amp; four"),
            "One two three & four"
        );
        assert_eq!(plain("no markup"), "no markup");
    }
}
