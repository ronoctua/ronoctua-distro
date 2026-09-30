//! iptv-org: the world's TV channels, kept as one compact index.
//!
//! # What is downloaded, and how much
//!
//! Four files make the index — channels, streams, categories and the
//! blocklist — and three more place a channel on the map: the feeds (whose
//! `broadcast_area` names subdivisions `s/…` and cities `ct/…`), and the names
//! of those. Measured on 2026-09-28 the four are 11.7 MB plain and 1.6 MB
//! gzipped, and GitHub Pages serves gzip, so every file is asked gzipped
//! ([`crate::Request::get`]). The place files are OPTIONAL: when one does not
//! come, the index is built without areas and says so
//! ([`Index::areas_known`]).
//!
//! # What never reaches the index
//!
//! A channel that is `is_nsfw`, that has a `closed` date, that the blocklist
//! names, that is in an excluded category (`xxx`, from `assets/channels.nix`),
//! or that has no stream that plays. A stream with no channel (1,944 of them)
//! has no country and no category — nothing says it is not adult — so it is
//! left out too. What was dropped is counted ([`Dropped`]), never shown.
//!
//! # And how a channel's streams are ordered
//!
//! The best picture first, by the height its `quality` says (`1080p`, `576i`);
//! a stream labelled `Geo-blocked` goes LAST whatever its quality, because it
//! fails for most people; `Not 24/7` is kept as a flag the row can say.
//!
//! # UK is GB
//!
//! 688 channels say `UK` and none says `GB`; every code goes through
//! [`country::normal`], and a code that names no country is `None` (the row
//! "no place on the map"), never a guess.
//!
//! # Headers with commas
//!
//! 741 `user_agent` values have a comma in them, which ends an item of mpv's
//! option lists. They are kept exactly as given; quoting them (`%len%…`) is
//! the engine's, where the option list is written.

use std::collections::{HashMap, HashSet};

use ronoctua_playlist::own::{ALTERNATES_MOST, Origin, Stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ask::{Net, Request, answered};
use crate::country::{self, Code};
use crate::json::{flag, is_web_address, text, texts};
use crate::{Refusal, Source, Why};

/// Where iptv-org publishes its index.
pub const HOST: &str = "iptv-org.github.io";

/// The most bytes one file may be, inflated. The feeds, the largest, are
/// 8 MB today.
pub const FILE_MOST_BYTES: u64 = 32 << 20;

/// The layout of a kept index. A kept index of another layout is not read,
/// and the index is asked for again.
pub const FORMAT: u32 = 1;

/// One file of iptv-org's index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum File {
    /// Every channel: name, country, categories, flags.
    Channels,
    /// Every stream: address, headers, quality, labels.
    Streams,
    /// The categories iptv-org has today.
    Categories,
    /// Channels taken down (DMCA) or adult.
    Blocklist,
    /// Each channel's feeds, with where they are broadcast.
    Feeds,
    /// The names of the cities feeds name.
    Cities,
    /// The names of the subdivisions feeds name.
    Subdivisions,
}

impl File {
    /// Every file, in the order a refresh asks for them.
    pub const ALL: [Self; 7] = [
        Self::Categories,
        Self::Blocklist,
        Self::Channels,
        Self::Streams,
        Self::Feeds,
        Self::Cities,
        Self::Subdivisions,
    ];

    /// Its name, without `.json`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Channels => "channels",
            Self::Streams => "streams",
            Self::Categories => "categories",
            Self::Blocklist => "blocklist",
            Self::Feeds => "feeds",
            Self::Cities => "cities",
            Self::Subdivisions => "subdivisions",
        }
    }

    /// Where it is.
    #[must_use]
    pub fn address(self) -> String {
        format!("https://{HOST}/api/{}.json", self.name())
    }
}

/// Where a channel is broadcast, finer than its country.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "code", rename_all = "kebab-case")]
pub enum Area {
    /// An ISO 3166-2 subdivision (`BR-SP`), from `s/…`.
    Subdivision(String),
    /// A UN/LOCODE city (`BRSAO`), from `ct/…`.
    City(String),
}

/// The name of an area a channel names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceName {
    /// Which area.
    pub area: Area,
    /// What it is called (data, like any name a directory gives).
    pub name: String,
    /// Its country, when the code names one.
    pub country: Option<Code>,
}

/// One stream of a channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TvStream {
    /// The address.
    pub url: String,
    /// What this stream calls itself, when said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The `Referer` it must be asked with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,
    /// The `User-Agent` it must be asked with (commas and all).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// The height of its picture, from its `quality`, when said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<u16>,
    /// It is labelled `Geo-blocked`.
    #[serde(default)]
    pub geo_blocked: bool,
    /// It is labelled `Not 24/7`.
    #[serde(default)]
    pub not_always_on: bool,
}

/// One channel, with the streams that play it, best first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    /// iptv-org's id (`BBCNews.uk`).
    pub id: String,
    /// What it is called.
    pub name: String,
    /// Its other names (often in another script).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<String>,
    /// Its country, when the code names one.
    pub country: Option<Code>,
    /// iptv-org's categories.
    pub categories: Vec<String>,
    /// Its streams, best first, the geo-blocked last. Never empty.
    pub streams: Vec<TvStream>,
    /// Where its feeds are broadcast, finer than the country.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<Area>,
}

impl Channel {
    /// Whether every stream of it is labelled `Not 24/7`.
    #[must_use]
    pub fn sometimes_off(&self) -> bool {
        self.streams.iter().all(|s| s.not_always_on)
    }

    /// The stream the queue holds: the best address first, and as the ones
    /// tried next, the others asked with the SAME headers (a `Stream` carries
    /// one referrer and one user agent). Live, and where it was found.
    #[must_use]
    pub fn stream(&self) -> Option<Stream> {
        let first = self.streams.first()?;
        let alternates = self
            .streams
            .iter()
            .skip(1)
            .filter(|s| s.referrer == first.referrer && s.user_agent == first.user_agent)
            .map(|s| s.url.clone())
            .filter(|u| *u != first.url)
            .take(ALTERNATES_MOST)
            .collect();
        Some(Stream {
            url: first.url.clone(),
            title: Some(self.name.clone()),
            referrer: first.referrer.clone(),
            user_agent: first.user_agent.clone(),
            alternates,
            origin: Origin::Iptv {
                channel: self.id.clone(),
            },
            live: true,
        })
    }
}

/// What was left out of an index, counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dropped {
    /// Channels flagged `is_nsfw`.
    pub nsfw: u32,
    /// Channels with a `closed` date.
    pub closed: u32,
    /// Channels the blocklist names.
    pub blocked: u32,
    /// Channels in an excluded category.
    pub excluded: u32,
    /// Channels with no stream that plays.
    pub streamless: u32,
    /// Streams that name no channel.
    pub unowned: u32,
}

/// The index every TV question is answered from, with no network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Index {
    /// Its layout, [`FORMAT`].
    pub format: u32,
    /// When it was built, in seconds since the epoch (the caller's clock).
    pub fetched_unix_seconds: i64,
    /// Every channel kept, by name.
    pub channels: Vec<Channel>,
    /// The categories iptv-org had when it was built.
    pub categories: Vec<String>,
    /// The names of the areas the channels name.
    pub places: Vec<PlaceName>,
    /// Whether the feeds came, so a channel with no area has none — rather
    /// than none known.
    pub areas_known: bool,
    /// What was left out.
    pub dropped: Dropped,
}

/// The height a `quality` says: `1080p` and `576i` are 1080 and 576. Pure.
#[must_use]
pub fn quality_lines(quality: &str) -> Option<u16> {
    let q = quality.trim();
    let digits = q.strip_suffix(['p', 'i', 'P', 'I'])?;
    digits.parse::<u16>().ok().filter(|n| *n > 0)
}

/// Walks a JSON array one element at a time — each element is a `Value` only
/// while it is looked at, so a file of thirty thousand channels never
/// exists as thirty thousand `Value`s at once.
fn each_item(bytes: &[u8], mut see: impl FnMut(&Value)) -> Result<(), Why> {
    struct Walk<'f, F: FnMut(&Value)>(&'f mut F);
    impl<'de, F: FnMut(&Value)> serde::de::Visitor<'de> for Walk<'_, F> {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a JSON array")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
            while let Some(item) = seq.next_element::<Value>()? {
                (self.0)(&item);
            }
            Ok(())
        }
    }
    let mut de = serde_json::Deserializer::from_slice(bytes);
    serde::Deserializer::deserialize_seq(&mut de, Walk(&mut see)).map_err(|_| Why::Malformed)?;
    de.end().map_err(|_| Why::Malformed)
}

/// A channel while the index is being made: its streams and the feeds they
/// name, before the feeds are read.
struct Draft {
    channel: Channel,
    feeds: HashSet<String>,
}

/// An index being made, one file at a time — in the order of [`File::ALL`],
/// which is the order each needs the one before (a stream's channel must be
/// known, a feed's streams must be).
#[derive(Default)]
pub struct Builder {
    excluded: Vec<String>,
    categories: Vec<String>,
    blocked: HashSet<String>,
    drafts: HashMap<String, Draft>,
    dropped: Dropped,
    areas_known: bool,
    cities: HashMap<String, (String, Option<Code>)>,
    subdivisions: HashMap<String, (String, Option<Code>)>,
}

impl Builder {
    /// A builder that leaves out the categories `excluded`.
    #[must_use]
    pub fn new(excluded: &[&str]) -> Self {
        Self {
            excluded: excluded.iter().map(|e| (*e).to_owned()).collect(),
            ..Self::default()
        }
    }

    /// `categories.json`.
    ///
    /// # Errors
    ///
    /// [`Why::Malformed`] for what is not a list.
    pub fn categories(&mut self, bytes: &[u8]) -> Result<(), Why> {
        each_item(bytes, |v| {
            if let Some(id) = text(v, "id")
                && !self.categories.contains(&id)
            {
                self.categories.push(id);
            }
        })
    }

    /// `blocklist.json`: read BEFORE the channels, which it removes.
    ///
    /// # Errors
    ///
    /// [`Why::Malformed`] for what is not a list.
    pub fn blocklist(&mut self, bytes: &[u8]) -> Result<(), Why> {
        each_item(bytes, |v| {
            if let Some(id) = text(v, "channel") {
                self.blocked.insert(id);
            }
        })
    }

    /// `channels.json`.
    ///
    /// # Errors
    ///
    /// [`Why::Malformed`] for what is not a list.
    pub fn channels(&mut self, bytes: &[u8]) -> Result<(), Why> {
        each_item(bytes, |v| {
            let (Some(id), Some(name)) = (text(v, "id"), text(v, "name")) else {
                return;
            };
            let categories = texts(v, "categories");
            // ADULT FIRST, whatever else is true of the channel: it is the
            // one reason that must never be outweighed.
            if flag(v, "is_nsfw").unwrap_or(false) {
                self.dropped.nsfw += 1;
            } else if categories.iter().any(|c| self.excluded.contains(c)) {
                self.dropped.excluded += 1;
            } else if self.blocked.contains(&id) {
                self.dropped.blocked += 1;
            } else if text(v, "closed").is_some() {
                self.dropped.closed += 1;
            } else {
                let channel = Channel {
                    id: id.clone(),
                    name,
                    also: texts(v, "alt_names"),
                    country: text(v, "country").and_then(|c| country::normal(&c).ok()),
                    categories,
                    streams: Vec::new(),
                    areas: Vec::new(),
                };
                self.drafts.insert(
                    id,
                    Draft {
                        channel,
                        feeds: HashSet::new(),
                    },
                );
            }
        })
    }

    /// `streams.json`: after the channels.
    ///
    /// # Errors
    ///
    /// [`Why::Malformed`] for what is not a list.
    pub fn streams(&mut self, bytes: &[u8]) -> Result<(), Why> {
        each_item(bytes, |v| {
            let Some(owner) = text(v, "channel") else {
                self.dropped.unowned += 1;
                return;
            };
            // A STREAM OF A CHANNEL LEFT OUT is left out with it: adult,
            // blocked or closed, the reason was the channel's.
            let Some(draft) = self.drafts.get_mut(&owner) else {
                return;
            };
            let Some(url) = text(v, "url").filter(|u| is_web_address(u)) else {
                return;
            };
            if draft.channel.streams.iter().any(|s| s.url == url) {
                return;
            }
            let labels = texts(v, "labels");
            let said = |label: &str| labels.iter().any(|l| l.eq_ignore_ascii_case(label));
            if let Some(feed) = text(v, "feed") {
                draft.feeds.insert(feed);
            }
            draft.channel.streams.push(TvStream {
                url,
                title: text(v, "title"),
                referrer: text(v, "referrer"),
                user_agent: text(v, "user_agent"),
                lines: text(v, "quality").and_then(|q| quality_lines(&q)),
                geo_blocked: said("Geo-blocked"),
                not_always_on: said("Not 24/7"),
            });
        })
    }

    /// `feeds.json`: after the streams, which say which feeds they carry.
    ///
    /// A channel's areas are those of the feeds its streams name; a channel
    /// whose streams name none takes its MAIN feed's.
    ///
    /// # Errors
    ///
    /// [`Why::Malformed`] for what is not a list.
    pub fn feeds(&mut self, bytes: &[u8]) -> Result<(), Why> {
        each_item(bytes, |v| {
            let (Some(owner), Some(id)) = (text(v, "channel"), text(v, "id")) else {
                return;
            };
            let Some(draft) = self.drafts.get_mut(&owner) else {
                return;
            };
            let carried = if draft.feeds.is_empty() {
                flag(v, "is_main").unwrap_or(false)
            } else {
                draft.feeds.contains(&id)
            };
            if !carried {
                return;
            }
            for code in texts(v, "broadcast_area") {
                let area = if let Some(s) = code.strip_prefix("s/") {
                    Area::Subdivision(s.to_owned())
                } else if let Some(c) = code.strip_prefix("ct/") {
                    Area::City(c.to_owned())
                } else {
                    continue;
                };
                if !draft.channel.areas.contains(&area) {
                    draft.channel.areas.push(area);
                }
            }
        })?;
        self.areas_known = true;
        Ok(())
    }

    fn names(bytes: &[u8]) -> Result<HashMap<String, (String, Option<Code>)>, Why> {
        let mut out = HashMap::new();
        each_item(bytes, |v| {
            if let (Some(code), Some(name)) = (text(v, "code"), text(v, "name")) {
                let country = text(v, "country").and_then(|c| country::normal(&c).ok());
                out.insert(code, (name, country));
            }
        })?;
        Ok(out)
    }

    /// `cities.json`.
    ///
    /// # Errors
    ///
    /// [`Why::Malformed`] for what is not a list.
    pub fn cities(&mut self, bytes: &[u8]) -> Result<(), Why> {
        self.cities = Self::names(bytes)?;
        Ok(())
    }

    /// `subdivisions.json`.
    ///
    /// # Errors
    ///
    /// [`Why::Malformed`] for what is not a list.
    pub fn subdivisions(&mut self, bytes: &[u8]) -> Result<(), Why> {
        self.subdivisions = Self::names(bytes)?;
        Ok(())
    }

    /// The index: every channel with a stream, by name, its streams best
    /// first; the names of the areas they name.
    #[must_use]
    pub fn finish(self, fetched_unix_seconds: i64) -> Index {
        let mut dropped = self.dropped;
        let mut channels: Vec<Channel> = Vec::with_capacity(self.drafts.len());
        for (_, draft) in self.drafts {
            let mut channel = draft.channel;
            if channel.streams.is_empty() {
                dropped.streamless += 1;
                continue;
            }
            // STABLE: two streams of one height keep iptv-org's order.
            channel
                .streams
                .sort_by_key(|s| (s.geo_blocked, std::cmp::Reverse(s.lines.unwrap_or(0))));
            channel.areas.sort();
            channels.push(channel);
        }
        channels.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut wanted: Vec<&Area> = channels.iter().flat_map(|c| c.areas.iter()).collect();
        wanted.sort();
        wanted.dedup();
        let places = wanted
            .into_iter()
            .filter_map(|area| {
                let (name, country) = match area {
                    Area::City(code) => self.cities.get(code)?,
                    Area::Subdivision(code) => self.subdivisions.get(code)?,
                };
                Some(PlaceName {
                    area: area.clone(),
                    name: name.clone(),
                    country: *country,
                })
            })
            .collect();
        Index {
            format: FORMAT,
            fetched_unix_seconds,
            channels,
            categories: self.categories,
            places,
            areas_known: self.areas_known,
            dropped,
        }
    }
}

/// The files of one index, as downloaded; the place files may be missing.
#[derive(Debug, Clone, Copy)]
pub struct Raw<'a> {
    /// `channels.json`.
    pub channels: &'a [u8],
    /// `streams.json`.
    pub streams: &'a [u8],
    /// `categories.json`.
    pub categories: &'a [u8],
    /// `blocklist.json`.
    pub blocklist: &'a [u8],
    /// `feeds.json`, when it came.
    pub feeds: Option<&'a [u8]>,
    /// `cities.json`, when it came.
    pub cities: Option<&'a [u8]>,
    /// `subdivisions.json`, when it came.
    pub subdivisions: Option<&'a [u8]>,
}

/// An index out of its files, leaving out the categories `excluded`. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] when one of the four needed files is not a list. A
/// place file that is not one is left out, as if it had not come.
pub fn build(raw: &Raw<'_>, excluded: &[&str], fetched_unix_seconds: i64) -> Result<Index, Why> {
    let mut b = Builder::new(excluded);
    b.categories(raw.categories)?;
    b.blocklist(raw.blocklist)?;
    b.channels(raw.channels)?;
    b.streams(raw.streams)?;
    if let Some(feeds) = raw.feeds
        && b.feeds(feeds).is_ok()
    {
        if let Some(cities) = raw.cities {
            let _ = b.cities(cities);
        }
        if let Some(subdivisions) = raw.subdivisions {
            let _ = b.subdivisions(subdivisions);
        }
    }
    Ok(b.finish(fetched_unix_seconds))
}

/// Downloads iptv-org's files and makes the index, leaving out the categories
/// `excluded`. One file at a time, each dropped once read.
///
/// Blocks for as long as the downloads take: call it on a thread.
///
/// # Errors
///
/// The refusal of one of the four needed files. A place file that does not
/// come leaves the index without areas ([`Index::areas_known`]).
pub fn refresh(
    net: &dyn Net,
    excluded: &[&str],
    fetched_unix_seconds: i64,
) -> Result<Index, Refusal> {
    let get = |file: File| {
        answered(
            net,
            &Request::get(Source::Iptv, file.address()).most(FILE_MOST_BYTES),
        )
    };
    let bad = |why: Why| Refusal::new(Source::Iptv, why);
    let mut b = Builder::new(excluded);
    b.categories(&get(File::Categories)?).map_err(bad)?;
    b.blocklist(&get(File::Blocklist)?).map_err(bad)?;
    b.channels(&get(File::Channels)?).map_err(bad)?;
    b.streams(&get(File::Streams)?).map_err(bad)?;
    if get(File::Feeds)
        .ok()
        .is_some_and(|feeds| b.feeds(&feeds).is_ok())
    {
        if let Ok(cities) = get(File::Cities) {
            let _ = b.cities(&cities);
        }
        if let Ok(subdivisions) = get(File::Subdivisions) {
            let _ = b.subdivisions(&subdivisions);
        }
    }
    Ok(b.finish(fetched_unix_seconds))
}

impl Index {
    /// The index as it is kept.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// A kept index, when it is one of this [`FORMAT`].
    ///
    /// # Errors
    ///
    /// [`Why::Malformed`] for anything else — the caller asks again.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Why> {
        let index: Self = serde_json::from_slice(bytes).map_err(|_| Why::Malformed)?;
        if index.format == FORMAT {
            Ok(index)
        } else {
            Err(Why::Malformed)
        }
    }

    /// The channel with this id.
    #[must_use]
    pub fn channel(&self, id: &str) -> Option<&Channel> {
        self.channels.iter().find(|c| c.id == id)
    }

    /// The channels of a country, by name.
    #[must_use]
    pub fn in_country(&self, code: Code) -> Vec<&Channel> {
        self.channels
            .iter()
            .filter(|c| c.country == Some(code))
            .collect()
    }

    /// The channels in any of these categories, by name.
    #[must_use]
    pub fn in_categories(&self, ids: &[&str]) -> Vec<&Channel> {
        self.channels
            .iter()
            .filter(|c| c.categories.iter().any(|k| ids.contains(&k.as_str())))
            .collect()
    }

    /// The channels whose name — or another name of theirs — has `words`, in
    /// any case, at most `most`.
    #[must_use]
    pub fn search(&self, words: &str, most: usize) -> Vec<&Channel> {
        let Some(wanted) = crate::query::words(words, crate::radio::WORDS_MOST) else {
            return Vec::new();
        };
        let wanted = wanted.to_lowercase();
        self.channels
            .iter()
            .filter(|c| {
                std::iter::once(&c.name)
                    .chain(c.also.iter())
                    .any(|n| n.to_lowercase().contains(&wanted))
            })
            .take(most)
            .collect()
    }

    /// Every country with channels, and how many — the most first. A channel
    /// whose country names none is counted by [`Index::placeless`].
    #[must_use]
    pub fn countries(&self) -> Vec<(Code, u32)> {
        let mut counts: HashMap<Code, u32> = HashMap::new();
        for c in &self.channels {
            if let Some(code) = c.country {
                *counts.entry(code).or_default() += 1;
            }
        }
        let mut out: Vec<(Code, u32)> = counts.into_iter().collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }

    /// How many channels have no country the map knows.
    #[must_use]
    pub fn placeless(&self) -> usize {
        self.channels.iter().filter(|c| c.country.is_none()).count()
    }

    /// The name of an area, when it came.
    #[must_use]
    pub fn place(&self, area: &Area) -> Option<&PlaceName> {
        self.places.iter().find(|p| p.area == *area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    const CATEGORIES: &str = r#"[{"id":"news","name":"News"},{"id":"movies","name":"Movies"},
        {"id":"xxx","name":"XXX"},{"id":"news"}]"#;
    const BLOCKLIST: &str = r#"[{"channel":"Taken.us","reason":"dmca","ref":"x"}]"#;
    const CHANNELS: &str = r#"[
        {"id":"News24.uk","name":"News 24","alt_names":["N24"],"country":"UK",
         "categories":["news"],"is_nsfw":false,"closed":null},
        {"id":"Adult.us","name":"Adult","country":"US","categories":["movies"],"is_nsfw":true},
        {"id":"Hidden.us","name":"Hidden","country":"US","categories":["xxx","movies"],"is_nsfw":false},
        {"id":"Taken.us","name":"Taken","country":"US","categories":["movies"]},
        {"id":"Gone.fr","name":"Gone","country":"FR","categories":["news"],"closed":"2020-05-31"},
        {"id":"Quiet.br","name":"Quiet","country":"BR","categories":["movies"]},
        {"id":"Films.br","name":"films Brazil","alt_names":["Filmes"],"country":"BR",
         "categories":["movies"]},
        {"id":"Nowhere.zz","name":"Nowhere TV","country":"ZZ","categories":[]},
        null, {"name":"no id"}
    ]"#;
    const STREAMS: &str = r#"[
        {"channel":"News24.uk","feed":"SD","url":"https://a.example/sd.m3u8","quality":"576i",
         "labels":[]},
        {"channel":"News24.uk","feed":"HD","url":"https://a.example/hd.m3u8","quality":"1080p",
         "labels":["Geo-blocked"],"user_agent":"Mozilla/5.0 (X11; Linux x86_64, like Gecko)"},
        {"channel":"News24.uk","feed":"HD","url":"https://a.example/720.m3u8","quality":"720p",
         "labels":["Not 24/7"]},
        {"channel":"News24.uk","url":"https://a.example/unknown.m3u8","quality":null},
        {"channel":"News24.uk","url":"https://a.example/sd.m3u8","quality":"576i"},
        {"channel":"Adult.us","url":"https://b.example/x.m3u8"},
        {"channel":"Hidden.us","url":"https://b.example/y.m3u8"},
        {"channel":"Taken.us","url":"https://b.example/z.m3u8"},
        {"channel":"Films.br","url":"https://c.example/f.m3u8","labels":["Not 24/7"]},
        {"channel":"Quiet.br","url":"rtmp://c.example/q"},
        {"channel":"Nowhere.zz","url":"https://d.example/n.m3u8"},
        {"channel":null,"url":"https://e.example/orphan.m3u8"}
    ]"#;
    const FEEDS: &str = r#"[
        {"channel":"News24.uk","id":"HD","is_main":false,"broadcast_area":["c/UK","s/GB-ENG","ct/GBLON"]},
        {"channel":"News24.uk","id":"Other","is_main":true,"broadcast_area":["ct/GBMAN"]},
        {"channel":"Films.br","id":"Main","is_main":true,"broadcast_area":["r/LATAM","ct/BRSAO"]}
    ]"#;
    const CITIES: &str = r#"[{"country":"UK","subdivision":"GB-ENG","name":"London","code":"GBLON"},
        {"country":"BR","name":"Sao Paulo","code":"BRSAO"},{"country":"UK","name":"Manchester","code":"GBMAN"}]"#;
    const SUBDIVISIONS: &str = r#"[{"country":"UK","name":"England","code":"GB-ENG"}]"#;

    fn raw() -> Raw<'static> {
        Raw {
            channels: CHANNELS.as_bytes(),
            streams: STREAMS.as_bytes(),
            categories: CATEGORIES.as_bytes(),
            blocklist: BLOCKLIST.as_bytes(),
            feeds: Some(FEEDS.as_bytes()),
            cities: Some(CITIES.as_bytes()),
            subdivisions: Some(SUBDIVISIONS.as_bytes()),
        }
    }

    fn index() -> Index {
        build(&raw(), &["xxx"], 1_790_000_000).expect("an index")
    }

    #[test]
    fn a_closed_nsfw_or_blocked_channel_never_reaches_the_index() {
        let ix = index();
        let ids: Vec<&str> = ix.channels.iter().map(|c| c.id.as_str()).collect();
        // BY NAME, in any case: "films Brazil" sorts before "News 24".
        assert_eq!(ids, ["Films.br", "News24.uk", "Nowhere.zz"]);
        for gone in ["Adult.us", "Hidden.us", "Taken.us", "Gone.fr", "Quiet.br"] {
            assert!(ix.channel(gone).is_none(), "{gone} reached the index");
        }
        assert_eq!(
            ix.dropped,
            Dropped {
                nsfw: 1,
                closed: 1,
                blocked: 1,
                excluded: 1,
                streamless: 1,
                unowned: 1,
            }
        );
        assert_eq!(ix.categories, ["news", "movies", "xxx"]);
        // AND NO STREAM OF THEM survives anywhere.
        let every: Vec<&str> = ix
            .channels
            .iter()
            .flat_map(|c| c.streams.iter().map(|s| s.url.as_str()))
            .collect();
        assert!(!every.iter().any(|u| u.starts_with("https://b.example")));
        assert!(!every.contains(&"https://e.example/orphan.m3u8"));
    }

    #[test]
    fn nsfw_is_dropped_even_when_nothing_else_is_said() {
        let only = r#"[{"id":"A.us","name":"A","country":"US","categories":["news"],"is_nsfw":1}]"#;
        let streams = r#"[{"channel":"A.us","url":"https://x.example/a"}]"#;
        let ix = build(
            &Raw {
                channels: only.as_bytes(),
                streams: streams.as_bytes(),
                categories: b"[]",
                blocklist: b"[]",
                feeds: None,
                cities: None,
                subdivisions: None,
            },
            &[],
            0,
        )
        .unwrap();
        assert!(ix.channels.is_empty());
        assert_eq!(ix.dropped.nsfw, 1);
    }

    #[test]
    fn a_geo_blocked_stream_is_tried_last() {
        let ix = index();
        let news = ix.channel("News24.uk").unwrap();
        let order: Vec<(&str, Option<u16>, bool)> = news
            .streams
            .iter()
            .map(|s| (s.url.as_str(), s.lines, s.geo_blocked))
            .collect();
        assert_eq!(
            order,
            [
                ("https://a.example/720.m3u8", Some(720), false),
                ("https://a.example/sd.m3u8", Some(576), false),
                ("https://a.example/unknown.m3u8", None, false),
                ("https://a.example/hd.m3u8", Some(1080), true),
            ]
        );
        assert!(news.streams[0].not_always_on);
        assert!(!news.sometimes_off(), "one stream is always on");
        assert!(ix.channel("Films.br").unwrap().sometimes_off());
    }

    #[test]
    fn a_channels_stream_keeps_its_headers_and_tries_its_siblings() {
        let ix = index();
        let s = ix.channel("News24.uk").unwrap().stream().unwrap();
        assert_eq!(s.url, "https://a.example/720.m3u8");
        assert!(s.live);
        assert_eq!(
            s.origin,
            Origin::Iptv {
                channel: "News24.uk".to_owned()
            }
        );
        // THE GEO-BLOCKED ONE carries another user agent, and a `Stream`
        // carries one: it is not an alternate of these.
        assert_eq!(
            s.alternates,
            [
                "https://a.example/sd.m3u8",
                "https://a.example/unknown.m3u8"
            ]
        );
        let hd = &ix.channel("News24.uk").unwrap().streams[3];
        assert_eq!(
            hd.user_agent.as_deref(),
            Some("Mozilla/5.0 (X11; Linux x86_64, like Gecko)"),
            "a comma is kept as it is"
        );
    }

    #[test]
    fn the_united_kingdom_is_gb_and_an_unknown_country_is_placeless() {
        let ix = index();
        assert_eq!(ix.channel("News24.uk").unwrap().country, Some(Code(*b"GB")));
        assert_eq!(ix.channel("Nowhere.zz").unwrap().country, None);
        assert_eq!(ix.placeless(), 1);
        let countries = ix.countries();
        assert_eq!(countries, [(Code(*b"BR"), 1), (Code(*b"GB"), 1)]);
        assert_eq!(ix.in_country(Code(*b"GB")).len(), 1);
    }

    #[test]
    fn a_feeds_cities_and_subdivisions_are_kept_for_places() {
        let ix = index();
        assert!(ix.areas_known);
        let news = ix.channel("News24.uk").unwrap();
        // THE FEED ITS STREAMS CARRY (HD), not the main one it never streams.
        assert_eq!(
            news.areas,
            [
                Area::Subdivision("GB-ENG".to_owned()),
                Area::City("GBLON".to_owned())
            ]
        );
        // A CHANNEL WHOSE STREAMS NAME NO FEED takes the main feed's.
        assert_eq!(
            ix.channel("Films.br").unwrap().areas,
            [Area::City("BRSAO".to_owned())]
        );
        let london = ix.place(&Area::City("GBLON".to_owned())).unwrap();
        assert_eq!(london.name, "London");
        assert_eq!(london.country, Some(Code(*b"GB")));
        assert!(
            ix.place(&Area::City("GBMAN".to_owned())).is_none(),
            "a name nobody uses is not kept"
        );

        let without = build(
            &Raw {
                feeds: None,
                ..raw()
            },
            &["xxx"],
            0,
        )
        .unwrap();
        assert!(!without.areas_known);
        assert!(without.channels.iter().all(|c| c.areas.is_empty()));
        assert!(without.places.is_empty());
    }

    #[test]
    fn a_search_and_a_category_are_answered_from_the_index() {
        let ix = index();
        let found: Vec<&str> = ix
            .search("FILMES", 10)
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(found, ["Films.br"], "another name counts, in any case");
        assert!(ix.search("   ", 10).is_empty());
        let news: Vec<&str> = ix
            .in_categories(&["news"])
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(news, ["News24.uk"]);
    }

    #[test]
    fn an_index_is_kept_and_read_back_and_another_layout_is_not() {
        let ix = index();
        let bytes = ix.to_bytes();
        assert_eq!(Index::from_bytes(&bytes).unwrap(), ix);
        let mut other = ix;
        other.format = FORMAT + 1;
        assert_eq!(Index::from_bytes(&other.to_bytes()), Err(Why::Malformed));
        assert_eq!(Index::from_bytes(b"[]"), Err(Why::Malformed));
    }

    #[test]
    fn a_quality_is_a_height() {
        assert_eq!(quality_lines("1080p"), Some(1080));
        assert_eq!(quality_lines("576i"), Some(576));
        assert_eq!(quality_lines("HD"), None);
        assert_eq!(quality_lines("0p"), None);
        assert_eq!(quality_lines(""), None);
    }

    #[test]
    fn a_refresh_asks_gzipped_and_carries_on_without_the_places() {
        let net = Canned::new(&[
            (&File::Categories.address(), 200, CATEGORIES.as_bytes()),
            (&File::Blocklist.address(), 200, BLOCKLIST.as_bytes()),
            (&File::Channels.address(), 200, CHANNELS.as_bytes()),
            (&File::Streams.address(), 200, STREAMS.as_bytes()),
            (&File::Feeds.address(), 503, b""),
        ]);
        let ix = refresh(&net, &["xxx"], 5).unwrap();
        assert!(!ix.areas_known);
        assert_eq!(ix.channels.len(), 3);
        assert_eq!(ix.fetched_unix_seconds, 5);
        let asked = net.asked();
        assert!(asked.iter().all(|r| r.gzip && r.source == Source::Iptv));
        assert_eq!(
            asked[0].url,
            "https://iptv-org.github.io/api/categories.json"
        );

        let broken = Canned::new(&[
            (&File::Categories.address(), 200, CATEGORIES.as_bytes()),
            (&File::Blocklist.address(), 200, BLOCKLIST.as_bytes()),
            (&File::Channels.address(), 200, b"<html>"),
        ]);
        assert_eq!(
            refresh(&broken, &["xxx"], 5).unwrap_err(),
            Refusal::new(Source::Iptv, Why::Malformed)
        );
    }
}
