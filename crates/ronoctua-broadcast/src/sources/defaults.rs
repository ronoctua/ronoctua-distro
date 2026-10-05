//! The default sources' answers, read into the catalogue: iptv-org's index,
//! the archive's collections and the live searches. (NASA's, Commons' and
//! PeerTube's pages are read by their own modules: [`crate::nasa`],
//! [`crate::commons`], [`crate::peertube`].)
//!
//! Each native category of a default source ([`crate::table::Native`]) is one
//! category of the catalogue, under its theme; its title is what the source
//! calls it, or what was asked of it.

use ronoctua_playlist::own::{Origin, Stream};
use ronoctua_ytdlp::Live;

use crate::archive::Holdings;
use crate::catalog::{Harvest, ItemKind, NewItem};
use crate::iptv::Index;
use crate::table::{DefaultSource, Native};

/// iptv-org's index, whole: each of the source's natives (an iptv-org
/// category) one category, and every channel an item in the ones it names.
/// A channel whose categories are none of them is in the category of the
/// uncategorised ([`crate::sources::playlist::UNGROUPED`]).
#[must_use]
pub fn iptv_org(index: &Index, source: &DefaultSource) -> Harvest {
    let mut h = Harvest::default();
    for n in source.natives {
        h.category(n.key, n.ask, Some(n.theme), false);
    }
    for channel in &index.channels {
        let Some(stream) = channel.stream() else {
            continue;
        };
        let mut keys: Vec<String> = channel
            .categories
            .iter()
            .filter(|c| source.natives.iter().any(|n| n.key == c.as_str()))
            .cloned()
            .collect();
        if keys.is_empty() {
            h.category(super::playlist::UNGROUPED, "", None, false);
            keys.push(super::playlist::UNGROUPED.to_owned());
        }
        h.items.push(NewItem {
            ext_id: channel.id.clone(),
            kind: ItemKind::Live,
            title: channel.name.clone(),
            also: channel.also.clone(),
            description: None,
            country: channel.country.map(|c| c.to_string()),
            language: None,
            height: channel.streams.first().and_then(|s| s.lines),
            duration_s: None,
            year: None,
            licence: None,
            popularity: 0,
            published_s: None,
            poster_url: None,
            stream,
            categories: keys,
        });
    }
    h
}

/// What one live search found, as items of `native`'s category.
pub fn lives(h: &mut Harvest, native: &Native, found: &[Live]) {
    h.category(native.key, native.ask, Some(native.theme), false);
    for live in found {
        h.items.push(NewItem {
            ext_id: live.id.clone(),
            kind: ItemKind::Live,
            title: live.title.clone(),
            also: live.channel.iter().cloned().collect(),
            description: None,
            country: None,
            language: None,
            height: None,
            duration_s: None,
            year: None,
            licence: None,
            popularity: live.watching.unwrap_or(0),
            published_s: None,
            // YOUTUBE SERVES EVERY VIDEO'S PICTURE at one address of its id.
            poster_url: Some(format!("https://i.ytimg.com/vi/{}/mqdefault.jpg", live.id)),
            stream: Stream {
                url: live.url.clone(),
                title: Some(live.title.clone()),
                referrer: None,
                user_agent: None,
                alternates: Vec::new(),
                origin: Origin::YouTube {
                    id: live.id.clone(),
                },
                live: true,
            },
            categories: vec![native.key.to_owned()],
        });
    }
}

/// A page of an archive collection, as items of the category `key`
/// (created with `title`, `theme` and `adult` when it is not yet).
pub fn holdings(
    h: &mut Harvest,
    key: &str,
    title: &str,
    theme: Option<&str>,
    adult: bool,
    found: &Holdings,
) {
    h.category(key, title, theme, adult);
    for held in &found.items {
        let Some(stream) = held.item.stream() else {
            continue;
        };
        h.items.push(NewItem {
            ext_id: held.item.identifier.clone(),
            kind: if held.audio {
                ItemKind::Audio
            } else {
                ItemKind::Video
            },
            title: held.item.title.clone(),
            also: held.subjects.clone(),
            description: held.description.clone(),
            country: None,
            language: held.language.clone(),
            height: None,
            duration_s: None,
            year: held.item.year,
            licence: held.item.license_url.clone(),
            popularity: held.item.downloads,
            published_s: None,
            poster_url: crate::archive::poster_address(&held.item.identifier),
            stream,
            categories: vec![key.to_owned()],
        });
    }
}

/// [`holdings`], for one of the archive source's natives.
pub fn archive_native(h: &mut Harvest, native: &Native, found: &Holdings) {
    holdings(h, native.key, native.ask, Some(native.theme), false, found);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::DEFAULT_SOURCES;

    fn source(id: &str) -> &'static DefaultSource {
        DEFAULT_SOURCES.iter().find(|s| s.id == id).unwrap()
    }

    #[test]
    fn iptv_org_s_categories_are_the_themes_and_a_channel_is_in_each_it_names() {
        let raw = crate::iptv::Raw {
            categories: br#"[{"id":"news","name":"News"},{"id":"movies","name":"Movies"},{"id":"xxx","name":"XXX"}]"#,
            blocklist: b"[]",
            channels: br#"[
                {"id":"A.us","name":"Alpha News","alt_names":["Alfa"],"country":"US","categories":["news","movies"],"is_nsfw":false},
                {"id":"B.br","name":"Beta","country":"BR","categories":[],"is_nsfw":false}
            ]"#,
            streams: br#"[
                {"channel":"A.us","url":"https://a.example/a.m3u8","quality":"720p"},
                {"channel":"B.br","url":"https://b.example/b.m3u8"}
            ]"#,
            feeds: None,
            cities: None,
            subdivisions: None,
        };
        let index = crate::iptv::build(&raw, crate::table::EXCLUDED, 0).unwrap();
        let h = iptv_org(&index, source("iptv"));
        let news = h.categories.iter().find(|c| c.key == "news").unwrap();
        assert_eq!(news.theme.as_deref(), Some("news"));
        let alpha = h.items.iter().find(|i| i.ext_id == "A.us").unwrap();
        assert_eq!(alpha.categories, ["news", "movies"]);
        assert_eq!(alpha.country.as_deref(), Some("US"));
        assert_eq!(alpha.height, Some(720));
        assert_eq!(alpha.also, ["Alfa"]);
        assert!(alpha.stream.live);
        let beta = h.items.iter().find(|i| i.ext_id == "B.br").unwrap();
        assert_eq!(beta.categories, [crate::sources::playlist::UNGROUPED]);
    }

    #[test]
    fn a_live_is_an_item_of_its_search_and_an_archive_item_of_its_collection() {
        let lives_source = source("lives");
        let native = &lives_source.natives[0];
        let mut h = Harvest::default();
        lives(
            &mut h,
            native,
            &[Live {
                id: "live0000001".to_owned(),
                url: "https://www.youtube.com/watch?v=live0000001".to_owned(),
                title: "Around the clock".to_owned(),
                channel: Some("Somebody".to_owned()),
                watching: Some(12),
            }],
        );
        assert_eq!(h.categories[0].theme.as_deref(), Some(native.theme));
        assert_eq!(h.items[0].popularity, 12);
        assert_eq!(
            h.items[0].poster_url.as_deref(),
            Some("https://i.ytimg.com/vi/live0000001/mqdefault.jpg")
        );

        let archive = source("archive");
        let prelinger = archive
            .natives
            .iter()
            .find(|n| n.ask == "prelinger")
            .expect("a public-domain collection");
        let found = crate::archive::parse_collection(
            br#"{"response":{"numFound":1,"docs":[{"identifier":"Duck_and_Cover",
                "title":"Duck and Cover","year":"1951","downloads":99,"mediatype":"movies",
                "licenseurl":"http://creativecommons.org/publicdomain/mark/1.0/"}]}}"#,
            1,
            100,
        )
        .unwrap();
        let mut h = Harvest::default();
        archive_native(&mut h, prelinger, &found);
        assert_eq!(h.categories[0].theme.as_deref(), Some("history"));
        let duck = &h.items[0];
        assert_eq!(duck.kind, ItemKind::Video);
        assert_eq!(duck.popularity, 99);
        assert_eq!(
            duck.stream.url,
            "https://archive.org/details/Duck_and_Cover"
        );
        assert_eq!(
            duck.poster_url.as_deref(),
            Some("https://archive.org/services/img/Duck_and_Cover")
        );
    }
}
