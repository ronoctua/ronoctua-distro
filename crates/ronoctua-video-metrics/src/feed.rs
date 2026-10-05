//! A channel's uploads feed — YouTube's `feeds/videos.xml`, Atom — read.
//!
//! # Which feed
//!
//! A channel `UC<rest>` has uploads playlists YouTube keeps without being
//! asked: `UULF<rest>` (its videos, without Shorts or lives) and `UUSH<rest>`
//! (its Shorts). Each has a feed of its fifteen newest ([`FEED_LEN`]), with
//! no key and no quota; `channel_id=UC…` is the whole channel's, the
//! fallback when a playlist's feed is not there. [`Which::path`] writes them.
//!
//! # Read loosely, through the guard
//!
//! Parsed by `ronoctua-xml` (the nesting measured first, no DTD), and each
//! entry taken field by field: an entry without a video id is left out, a
//! missing date or statistic is `None`, and the rest of the feed stands.

use std::borrow::Cow;

use ronoctua_xml::roxmltree::{Node, ParsingOptions};

use crate::{ChannelId, VideoId};

/// How many entries a feed carries: a feed this full, all of them new, may
/// be hiding more (the scanner then lists the tab).
pub const FEED_LEN: usize = 15;

/// The most bytes of one feed: fifteen entries with their descriptions are
/// tens of kilobytes; a megabyte is an answer that is not a feed.
pub const MOST_BYTES: u64 = 1 << 20;

const ATOM: &str = "http://www.w3.org/2005/Atom";
const YT: &str = "http://www.youtube.com/xml/schemas/2015";
const MEDIA: &str = "http://search.yahoo.com/mrss/";

/// Which feed of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Which {
    /// `playlist_id=UULF…`: its videos.
    Videos,
    /// `playlist_id=UUSH…`: its Shorts.
    Shorts,
    /// `channel_id=UC…`: everything, the fallback.
    Channel,
}

impl Which {
    /// The feed's path and query on the feeds' host; `None` when `channel`
    /// is not a channel id. Pure.
    #[must_use]
    pub fn path(self, channel: &ChannelId) -> Option<String> {
        let rest = channel.0.strip_prefix("UC")?;
        if !ronoctua_ytdlp::youtube::is_channel_id(&channel.0) {
            return None;
        }
        Some(match self {
            Self::Videos => format!("/feeds/videos.xml?playlist_id=UULF{rest}"),
            Self::Shorts => format!("/feeds/videos.xml?playlist_id=UUSH{rest}"),
            Self::Channel => format!("/feeds/videos.xml?channel_id={}", channel.0),
        })
    }
}

/// One entry of a feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The video.
    pub id: VideoId,
    /// Its channel, as the entry says.
    pub channel: Option<ChannelId>,
    /// Its title — data.
    pub title: String,
    /// Its address (`/watch?v=…`, or `/shorts/…` for a Short).
    pub url: String,
    /// When it was published, seconds since the epoch.
    pub published_at_s: Option<i64>,
    /// Its description — data.
    pub description: String,
    /// The feed's thumbnail address.
    pub thumb_url: Option<String>,
    /// Its view count, as the feed says.
    pub views: Option<u64>,
}

impl Entry {
    /// Whether its address is a Short's (`/shorts/…`).
    #[must_use]
    pub fn links_a_short(&self) -> bool {
        self.url.contains("/shorts/")
    }
}

/// A feed, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feed {
    /// The channel the feed is of.
    pub channel: Option<ChannelId>,
    /// Its title — data.
    pub title: String,
    /// Its entries, newest first as YouTube lists them.
    pub entries: Vec<Entry>,
}

/// Why a feed was not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unread {
    /// Not text, or not XML that parses (or nests too deep).
    NotXml,
    /// XML, and not an Atom feed.
    NotAFeed,
}

/// A feed, read out of its bytes. Pure.
///
/// # Errors
///
/// [`Unread::NotXml`], [`Unread::NotAFeed`].
pub fn parse(bytes: &[u8]) -> Result<Feed, Unread> {
    let text = std::str::from_utf8(bytes).map_err(|_| Unread::NotXml)?;
    let options = ParsingOptions {
        allow_dtd: false,
        ..ParsingOptions::default()
    };
    let document = ronoctua_xml::parse(text, options).map_err(|_| Unread::NotXml)?;
    let root = document.root_element();
    if !root.has_tag_name((ATOM, "feed")) {
        return Err(Unread::NotAFeed);
    }
    let entries = root
        .children()
        .filter(|n| n.has_tag_name((ATOM, "entry")))
        .filter_map(|entry| entry_of(&entry))
        .collect();
    Ok(Feed {
        channel: text_of(&root, YT, "channelId").and_then(channel_id),
        title: text_of(&root, ATOM, "title").unwrap_or_default(),
        entries,
    })
}

fn entry_of(entry: &Node<'_, '_>) -> Option<Entry> {
    let id = text_of(entry, YT, "videoId").filter(|id| is_video_id(id))?;
    let url = entry
        .children()
        .filter(|n| n.has_tag_name((ATOM, "link")))
        .find(|n| n.attribute("rel").unwrap_or("alternate") == "alternate")
        .and_then(|n| n.attribute("href"))
        .filter(|h| h.starts_with("https://"))
        .map_or_else(
            || format!("https://www.youtube.com/watch?v={id}"),
            str::to_owned,
        );
    let group = child(entry, MEDIA, "group");
    let thumb_url = group
        .as_ref()
        .and_then(|g| child(g, MEDIA, "thumbnail"))
        .and_then(|t| t.attribute("url").map(str::to_owned))
        .filter(|u| u.starts_with("https://"));
    let views = group
        .as_ref()
        .and_then(|g| child(g, MEDIA, "community"))
        .and_then(|c| child(&c, MEDIA, "statistics"))
        .and_then(|s| s.attribute("views"))
        .and_then(|v| v.parse().ok());
    Some(Entry {
        channel: text_of(entry, YT, "channelId").and_then(channel_id),
        title: text_of(entry, ATOM, "title").unwrap_or_default(),
        published_at_s: text_of(entry, ATOM, "published").and_then(|t| rfc3339_s(&t)),
        description: group
            .as_ref()
            .and_then(|g| text_of(g, MEDIA, "description"))
            .unwrap_or_default(),
        thumb_url,
        views,
        url,
        id: VideoId(id),
    })
}

fn child<'a, 'input>(node: &Node<'a, 'input>, ns: &str, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|n| n.has_tag_name((ns, name)))
}

/// A child's text, trimmed; `None` when absent or empty.
fn text_of(node: &Node<'_, '_>, ns: &str, name: &str) -> Option<String> {
    let text: Cow<'_, str> = child(node, ns, name)?
        .children()
        .filter_map(|n| n.text())
        .collect::<String>()
        .into();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn channel_id(text: String) -> Option<ChannelId> {
    ronoctua_ytdlp::youtube::is_channel_id(&text).then_some(ChannelId(text))
}

fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// An RFC 3339 time (`2026-09-29T15:00:07+00:00`, `…Z`, with or without a
/// fraction) as seconds since the epoch. Pure.
#[must_use]
pub fn rfc3339_s(text: &str) -> Option<i64> {
    let text = text.trim();
    let digits = |s: &str| -> Option<i64> {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .then(|| s.parse().ok())
            .flatten()
    };
    let (date, rest) = text.split_once(['T', 't', ' '])?;
    let mut day = date.splitn(3, '-');
    let (year, month, dom) = (
        digits(day.next()?)?,
        digits(day.next()?)?,
        digits(day.next()?)?,
    );
    if !(1..=12).contains(&month) || !(1..=31).contains(&dom) {
        return None;
    }
    // `get` and not `split_at`: the text is the network's, and byte 8 may fall
    // inside a character — that is no time, never a panic.
    let (clock, zone) = (rest.get(..8)?, rest.get(8..)?);
    let mut hms = clock.splitn(3, ':');
    let (hour, minute, second) = (
        digits(hms.next()?)?,
        digits(hms.next()?)?,
        digits(hms.next()?)?,
    );
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    // A FRACTION of a second is dropped; the offset follows it.
    let zone = zone
        .strip_prefix('.')
        .map_or(zone, |f| f.trim_start_matches(|c: char| c.is_ascii_digit()));
    let offset = match zone {
        "Z" | "z" => 0,
        _ => {
            let sign = match zone.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let (h, m) = zone[1..].split_once(':')?;
            sign * (digits(h)? * 3600 + digits(m)? * 60)
        }
    };
    let days = ronoctua_ytdlp::youtube::days_from_civil(year, month, dom);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A feed as YouTube writes one (trimmed to what is read), with a Short
    /// link, an entry without an id, and a description with markup escaped.
    pub(crate) const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">
 <link rel="self" href="http://www.youtube.com/feeds/videos.xml?playlist_id=UULFabcdefghijklmnopqrstuv"/>
 <id>yt:playlist:UULFabcdefghijklmnopqrstuv</id>
 <yt:playlistId>UULFabcdefghijklmnopqrstuv</yt:playlistId>
 <yt:channelId>UCabcdefghijklmnopqrstuv</yt:channelId>
 <title>Some Channel</title>
 <entry>
  <id>yt:video:aaaaaaaaaaa</id>
  <yt:videoId>aaaaaaaaaaa</yt:videoId>
  <yt:channelId>UCabcdefghijklmnopqrstuv</yt:channelId>
  <title>First &amp; best</title>
  <link rel="alternate" href="https://www.youtube.com/watch?v=aaaaaaaaaaa"/>
  <author><name>Some Channel</name></author>
  <published>2026-09-29T15:00:07+00:00</published>
  <updated>2026-09-30T01:02:03+00:00</updated>
  <media:group>
   <media:title>First &amp; best</media:title>
   <media:content url="https://www.youtube.com/v/aaaaaaaaaaa?version=3" type="application/x-shockwave-flash" width="640" height="390"/>
   <media:thumbnail url="https://i2.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg" width="480" height="360"/>
   <media:description>Line one
&lt;b&gt;not markup&lt;/b&gt;</media:description>
   <media:community>
    <media:starRating count="12" average="5.00" min="1" max="5"/>
    <media:statistics views="4567"/>
   </media:community>
  </media:group>
 </entry>
 <entry>
  <yt:videoId>bbbbbbbbbbb</yt:videoId>
  <title>A short one</title>
  <link rel="alternate" href="https://www.youtube.com/shorts/bbbbbbbbbbb"/>
  <published>2026-09-28T10:00:00.123Z</published>
 </entry>
 <entry>
  <title>No id: left out</title>
 </entry>
</feed>"#;

    #[test]
    fn a_feed_is_read_entry_by_entry() {
        let feed = parse(FEED.as_bytes()).unwrap();
        assert_eq!(
            feed.channel,
            Some(ChannelId("UCabcdefghijklmnopqrstuv".to_owned()))
        );
        assert_eq!(feed.title, "Some Channel");
        assert_eq!(feed.entries.len(), 2);
        let first = &feed.entries[0];
        assert_eq!(first.id, VideoId("aaaaaaaaaaa".to_owned()));
        assert_eq!(first.title, "First & best");
        assert_eq!(first.published_at_s, Some(1_790_694_007));
        assert_eq!(first.description, "Line one\n<b>not markup</b>");
        assert_eq!(
            first.thumb_url.as_deref(),
            Some("https://i2.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg")
        );
        assert_eq!(first.views, Some(4567));
        assert!(!first.links_a_short());
        let second = &feed.entries[1];
        assert!(second.links_a_short());
        assert_eq!(second.published_at_s, Some(1_790_589_600));
        assert_eq!((second.views, second.channel.as_ref()), (None, None));
    }

    #[test]
    fn what_is_not_a_feed_is_said() {
        assert_eq!(
            parse(b"<html><body>Sorry</body></html>"),
            Err(Unread::NotAFeed)
        );
        assert_eq!(parse(b"not xml at all"), Err(Unread::NotXml));
        assert_eq!(parse(&[0xff, 0xfe, 0x00]), Err(Unread::NotXml));
        let deep = format!("{}{}", "<a>".repeat(200), "</a>".repeat(200));
        assert_eq!(
            parse(deep.as_bytes()),
            Err(Unread::NotXml),
            "the guard holds"
        );
        let empty = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>x</title></feed>"#;
        assert!(parse(empty.as_bytes()).unwrap().entries.is_empty());
    }

    #[test]
    fn each_feed_has_its_address() {
        let channel = ChannelId("UCabcdefghijklmnopqrstuv".to_owned());
        assert_eq!(
            Which::Videos.path(&channel).unwrap(),
            "/feeds/videos.xml?playlist_id=UULFabcdefghijklmnopqrstuv"
        );
        assert_eq!(
            Which::Shorts.path(&channel).unwrap(),
            "/feeds/videos.xml?playlist_id=UUSHabcdefghijklmnopqrstuv"
        );
        assert_eq!(
            Which::Channel.path(&channel).unwrap(),
            "/feeds/videos.xml?channel_id=UCabcdefghijklmnopqrstuv"
        );
        assert_eq!(Which::Videos.path(&ChannelId("@handle".to_owned())), None);
    }

    #[test]
    fn times_are_read_with_their_offset() {
        assert_eq!(rfc3339_s("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339_s("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(rfc3339_s("1969-12-31T21:00:00.5-03:00"), Some(0));
        assert_eq!(rfc3339_s("2026-09-30T00:00:00+00:00"), Some(1_790_726_400));
        assert_eq!(rfc3339_s("2026-13-01T00:00:00Z"), None);
        assert_eq!(rfc3339_s("yesterday"), None);
        assert_eq!(rfc3339_s("2026-09-30T00:00:00"), None, "no zone, no time");
        // Byte 8 inside a character: no time, and no panic.
        assert_eq!(rfc3339_s("2026-09-30T\u{e9}\u{e9}\u{e9}\u{20ac}"), None);
        assert_eq!(rfc3339_s("2026-09-30T00:00:0\u{e9}Z"), None);
        assert_eq!(rfc3339_s("2026-09-30T00:00:00+0\u{e9}:00"), None);
    }
}
