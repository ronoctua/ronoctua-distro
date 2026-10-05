//! A person's M3U list — an IPTV provider's, iptv-org's own, anybody's —
//! read into categories and items.
//!
//! Each `group-title` is a category (a `;` in one separates several, as
//! iptv-org writes `Animation;Kids`), filed under a theme by the table's
//! aliases and sent to the adult store when an adult word names it. What the
//! list says of an entry becomes the item: its guide id is its id (two lines
//! with one id are one channel, the second address its alternate), its logo
//! its poster, its country and language filters, and a height its title
//! states (`(1080p)`, `HD`) its quality.

use ronoctua_playlist::own::{Origin, Stream};
use ronoctua_playlist::{Base, Bounds, Kind as ListKind, Where};

use super::{Refused, is_adult, theme_of};
use crate::catalog::{Harvest, ItemKind, NewItem, digest};
use crate::json::is_web_address;

/// The key of the category a list's ungrouped entries are in; its title is
/// empty, and the door names it.
pub const UNGROUPED: &str = "";

/// The extensions of a recording with a picture.
const VIDEO: [&str; 8] = ["mp4", "mkv", "avi", "webm", "mov", "m4v", "mpg", "mpeg"];

/// The extensions of a recording with sound only.
const AUDIO: [&str; 7] = ["mp3", "aac", "ogg", "opus", "flac", "m4a", "wav"];

/// What an address's extension says it is: a recording of a picture or of a
/// sound, or — anything else, an `.m3u8`, a `.ts`, no extension — a live
/// stream.
fn kind_of(url: &str) -> ItemKind {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let last = path.rsplit('/').next().unwrap_or("");
    let extension = last
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if VIDEO.contains(&extension.as_str()) {
        ItemKind::Video
    } else if AUDIO.contains(&extension.as_str()) {
        ItemKind::Audio
    } else {
        ItemKind::Live
    }
}

/// The height a title states: `(1080p)`, `720i`, `4K`/`UHD`, `FHD`, `HD`,
/// `SD`. The list's word, not a measurement.
#[must_use]
pub fn height_in(title: &str) -> Option<u16> {
    for word in title.split(|c: char| !c.is_alphanumeric()) {
        let lower = word.to_ascii_lowercase();
        if let Some(digits) = lower.strip_suffix(['p', 'i'])
            && (3..=4).contains(&digits.len())
            && let Ok(n) = digits.parse::<u16>()
            && (144..=4320).contains(&n)
        {
            return Some(n);
        }
        match lower.as_str() {
            "4k" | "uhd" => return Some(2160),
            "fhd" => return Some(1080),
            "hd" => return Some(720),
            "sd" => return Some(480),
            _ => {}
        }
    }
    None
}

/// The first of a list of values written `a;b` or `a,b`, trimmed.
fn first_of(text: &str) -> Option<&str> {
    text.split([';', ','])
        .map(str::trim)
        .find(|t| !t.is_empty())
}

/// The last piece of an address's path, for an entry with no title.
fn name_of(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty() && !s.contains(':'))
        .map_or_else(|| crate::sources::shown(url), ToOwned::to_owned)
}

/// A list's text, read into categories and items. `at` is the address it
/// was fetched from (a relative entry is beside it); `most_bytes` and
/// `most_items` bound it. Pure.
///
/// # Errors
///
/// [`Refused::NothingPlayable`] for a list with no web address in it,
/// [`Refused::Unknown`] for text that names nothing.
pub fn playlist(
    text: &str,
    at: &str,
    most_bytes: usize,
    most_items: usize,
) -> Result<Harvest, Refused> {
    let bounds = Bounds {
        bytes: most_bytes,
        entries: most_items,
    };
    let list = ronoctua_playlist::parse_text_within(text, ListKind::M3u, Base::Address(at), bounds)
        .map_err(|_| Refused::Unknown)?;
    let mut h = Harvest {
        cut: list.cut || text.len() > most_bytes,
        ..Harvest::default()
    };
    for entry in &list.entries {
        let Where::Remote(url) = &entry.at else {
            continue;
        };
        if !is_web_address(url) {
            continue;
        }
        let said = entry.attributes.as_deref();
        let mut keys: Vec<String> = Vec::new();
        if let Some(groups) = said.and_then(|a| a.group.as_deref()) {
            for group in groups.split(';').map(str::trim).filter(|g| !g.is_empty()) {
                h.category(group, group, theme_of(group), is_adult(group));
                if !keys.iter().any(|k| k == group) {
                    keys.push(group.to_owned());
                }
            }
        }
        if keys.is_empty() {
            h.category(UNGROUPED, "", None, false);
            keys.push(UNGROUPED.to_owned());
        }
        let title = entry.title.clone().unwrap_or_else(|| name_of(url));
        let kind = kind_of(url);
        // THE ID: the guide's when the list gives one (so two lines of one
        // channel are one item), else a digest of the address — never the
        // address itself, which may carry the person's account.
        let ext_id = said
            .and_then(|a| a.guide_id.clone())
            .unwrap_or_else(|| format!("u{}", digest(url)));
        let country = said
            .and_then(|a| a.country.as_deref())
            .and_then(first_of)
            .and_then(|c| crate::country::normal(c).ok())
            .map(|c| c.to_string());
        let language = said
            .and_then(|a| a.language.as_deref())
            .and_then(first_of)
            .map(ToOwned::to_owned);
        let poster = said
            .and_then(|a| a.logo.clone())
            .filter(|l| is_web_address(l));
        h.items.push(NewItem {
            ext_id,
            kind,
            also: Vec::new(),
            description: None,
            country,
            language,
            height: height_in(&title),
            duration_s: entry
                .seconds
                .filter(|s| *s >= 1.0 && *s < f64::from(u32::MAX))
                .map(|s| s as u32),
            year: None,
            licence: None,
            popularity: 0,
            published_s: None,
            poster_url: poster,
            stream: Stream {
                url: url.clone(),
                title: Some(title.clone()),
                referrer: said.and_then(|a| a.referrer.clone()),
                user_agent: said.and_then(|a| a.user_agent.clone()),
                alternates: Vec::new(),
                origin: Origin::Typed,
                live: kind == ItemKind::Live,
            },
            categories: keys,
            title,
        });
    }
    if h.items.is_empty() {
        return Err(Refused::NothingPlayable);
    }
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A HAND-WRITTEN list in the shape IPTV providers publish (`m3u_plus`,
    /// with `tvg-*` attributes and groups) and iptv-org's own
    /// (`group-title="Animation;Kids"`, `#EXTVLCOPT` headers).
    const LIST: &str = "#EXTM3U\n\
        #EXTINF:-1 tvg-id=\"BBCNews.uk\" tvg-country=\"UK\" tvg-language=\"English\" tvg-logo=\"https://logo.example/bbc.png\" group-title=\"News\",BBC News (1080p)\n\
        https://stream.example/bbc/index.m3u8\n\
        #EXTINF:-1 tvg-id=\"BBCNews.uk\" group-title=\"UK\",BBC News backup\n\
        https://backup.example/bbc.m3u8\n\
        #EXTINF:-1 tvg-id=\"Toons.us\" tvg-country=\"US;CA\" group-title=\"Animation;Kids\",Toons HD\n\
        #EXTVLCOPT:http-referrer=https://toons.example/\n\
        https://stream.example/toons.m3u8\n\
        #EXTINF:-1 group-title=\"Filmes | Ação\",Um Filme\n\
        http://vod.example/movie/ana/secret/123.mp4\n\
        #EXTINF:-1 group-title=\"Adultos 18+\",Late\n\
        https://stream.example/late.m3u8\n\
        #EXTINF:-1,No group at all\n\
        relative/stream.m3u8\n\
        #EXTINF:-1 group-title=\"News\",Not a web address\n\
        rtmp://stream.example/live\n";

    #[test]
    fn a_list_s_groups_are_categories_under_themes_and_the_adult_store() {
        let h = playlist(LIST, "https://lists.example/get.php?u=a", 1 << 20, 1000).unwrap();
        let cat = |key: &str| h.categories.iter().find(|c| c.key == key).unwrap();
        assert_eq!(cat("News").theme.as_deref(), Some("news"));
        assert_eq!(cat("Animation").theme.as_deref(), Some("cartoons"));
        assert_eq!(cat("Kids").theme.as_deref(), Some("kids"));
        assert_eq!(
            cat("Filmes | Ação").theme.as_deref(),
            Some("action"),
            "accents folded"
        );
        assert!(cat("Adultos 18+").adult);
        assert!(!cat("News").adult);
        assert_eq!(cat("UK").theme, None, "no alias says it");
        assert_eq!(cat(UNGROUPED).title, "", "the door names it");

        // THE RTMP LINE is not a web address and is left out.
        assert_eq!(h.items.len(), 6);
        let bbc = &h.items[0];
        assert_eq!(bbc.ext_id, "BBCNews.uk");
        assert_eq!(bbc.country.as_deref(), Some("GB"), "UK is GB");
        assert_eq!(bbc.language.as_deref(), Some("English"));
        assert_eq!(bbc.height, Some(1080));
        assert_eq!(
            bbc.poster_url.as_deref(),
            Some("https://logo.example/bbc.png")
        );
        assert_eq!(bbc.kind, ItemKind::Live);
        assert!(bbc.stream.live);
        // THE SECOND LINE OF ONE CHANNEL keeps its id: the writer merges them.
        assert_eq!(h.items[1].ext_id, "BBCNews.uk");

        let toons = &h.items[2];
        assert_eq!(toons.categories, ["Animation", "Kids"]);
        assert_eq!(toons.country.as_deref(), Some("US"), "the first of two");
        assert_eq!(toons.height, Some(720), "HD");
        assert_eq!(
            toons.stream.referrer.as_deref(),
            Some("https://toons.example/")
        );

        let film = &h.items[3];
        assert_eq!(film.kind, ItemKind::Video, "an .mp4 is a recording");
        assert!(!film.stream.live);
        assert!(film.ext_id.starts_with('u'));
        assert!(
            !film.ext_id.contains("secret"),
            "an id never carries the address"
        );

        let loose = &h.items[5];
        assert_eq!(loose.categories, [UNGROUPED]);
        assert_eq!(
            loose.stream.url, "https://lists.example/relative/stream.m3u8",
            "beside the list's address"
        );
    }

    #[test]
    fn a_list_past_its_bounds_says_so_and_one_with_nothing_is_refused() {
        let h = playlist(LIST, "https://lists.example/a.m3u", 1 << 20, 2).unwrap();
        assert_eq!(h.items.len(), 2);
        assert!(h.cut);
        assert_eq!(
            playlist(
                "#EXTM3U\n#EXTINF:-1,x\nrtmp://a/b\n",
                "https://l.example/",
                1 << 20,
                10
            ),
            Err(Refused::NothingPlayable)
        );
        assert_eq!(
            playlist("#EXTM3U\n", "https://l.example/", 1 << 20, 10),
            Err(Refused::Unknown)
        );
    }

    #[test]
    fn a_title_states_a_height() {
        assert_eq!(height_in("Canal (720p) [Geo-blocked]"), Some(720));
        assert_eq!(height_in("Movies 4K"), Some(2160));
        assert_eq!(height_in("Sports FHD"), Some(1080));
        assert_eq!(height_in("News"), None);
        assert_eq!(height_in("Channel 1234"), None, "a number is not a height");
        assert_eq!(height_in("TV 99p"), None);
    }
}
