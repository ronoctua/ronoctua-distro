//! A person's YouTube channel or playlist, listed through yt-dlp and read
//! into categories and items.
//!
//! A playlist is one category. A channel is its videos (one category) and
//! each of its playlists (one category each, at most
//! [`crate::table::YOUTUBE_PLAYLISTS_MOST`]); a video in two of them is one
//! item in both.
//!
//! The listing is `ronoctua_ytdlp::flat_up_to` — a child process with its
//! deadline and ceiling — handed in as a [`Lister`], so every rule here is
//! tested on hand-written listings without yt-dlp or a network.

use ronoctua_playlist::own::{Origin, Stream};
use ronoctua_ytdlp::{Flat, FlatEntry, Trouble};

use super::{Refused, YouTubeList, is_adult, theme_of};
use crate::catalog::{Harvest, ItemKind, NewItem, digest};

/// What lists an address flat: `ronoctua_ytdlp::flat_up_to` in the daemon.
pub type Lister<'a> = &'a dyn Fn(&str, usize) -> Result<Flat, Trouble>;

/// The key of a channel's own videos.
pub const VIDEOS: &str = "videos";

/// The key of a playlist's one category.
pub const LIST: &str = "list";

/// A video's id in its watch address.
fn video_id(url: &str) -> Option<String> {
    let id = super::query_param(url, "v")
        .map(ToOwned::to_owned)
        .or_else(|| {
            (crate::ask::host_of(url).as_deref() == Some("youtu.be"))
                .then(|| super::path_of(url).trim_start_matches('/').to_owned())
        })?;
    (id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
    .then_some(id)
}

/// A channel's own address: `https://www.youtube.com/@handle`, or
/// `/channel/UC…`, `/c/name`, `/user/name` — without a tab after it.
#[must_use]
pub fn channel_root(url: &str) -> Option<String> {
    let segments: Vec<&str> = super::path_of(url)
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let path = match segments.as_slice() {
        [handle, ..] if handle.starts_with('@') && handle.len() > 1 => (*handle).to_owned(),
        [kind @ ("channel" | "c" | "user"), name, ..] => format!("{kind}/{name}"),
        _ => return None,
    };
    Some(format!("https://www.youtube.com/{path}"))
}

/// One entry of a listing, as an item in `category`.
fn item_of(entry: &FlatEntry, category: &str) -> NewItem {
    let id = video_id(&entry.url);
    let title = if entry.title.is_empty() {
        crate::sources::shown(&entry.url)
    } else {
        entry.title.clone()
    };
    NewItem {
        ext_id: id
            .clone()
            .unwrap_or_else(|| format!("u{}", digest(&entry.url))),
        kind: ItemKind::Video,
        title: title.clone(),
        also: entry.channel.iter().cloned().collect(),
        description: None,
        country: None,
        language: None,
        height: None,
        duration_s: entry
            .duration_seconds
            .filter(|d| *d >= 1.0 && *d < f64::from(u32::MAX))
            .map(|d| d as u32),
        year: None,
        licence: None,
        popularity: 0,
        published_s: None,
        poster_url: entry.thumbnail.clone(),
        stream: Stream {
            url: entry.url.clone(),
            title: Some(title),
            referrer: None,
            user_agent: None,
            alternates: Vec::new(),
            origin: id.map_or(Origin::Typed, |id| Origin::YouTube { id }),
            live: false,
        },
        categories: vec![category.to_owned()],
    }
}

/// Adds a listing's entries to `h` under `key`, titled `title`.
fn add(h: &mut Harvest, key: &str, title: &str, flat: &Flat) {
    h.category(key, title, theme_of(title), is_adult(title));
    h.items.extend(flat.entries.iter().map(|e| item_of(e, key)));
}

/// A person's YouTube `list` at `url`, listed by `lister`: its title (the
/// channel's or the playlist's, as YouTube writes it) and its harvest. At
/// most `playlists_most` of a channel's playlists, and `list_most` videos of
/// each list.
///
/// A channel's playlist that fails to list is left out (and the harvest says
/// it was cut); its videos failing is the channel failing.
///
/// # Errors
///
/// [`Refused::Unknown`] for a channel address with no channel in it, and
/// yt-dlp's troubles ([`Refused::NoYtDlp`], [`Refused::ListingFailed`]…).
pub fn youtube(
    list: YouTubeList,
    url: &str,
    lister: Lister<'_>,
    playlists_most: usize,
    list_most: usize,
) -> Result<(String, Harvest), Refused> {
    let mut h = Harvest::default();
    match list {
        YouTubeList::Playlist => {
            let flat = lister(url, list_most)?;
            h.cut = flat.entries.len() >= list_most;
            add(&mut h, LIST, &flat.title, &flat);
            if h.items.is_empty() {
                return Err(Refused::NothingPlayable);
            }
            Ok((flat.title, h))
        }
        YouTubeList::Channel => {
            let root = channel_root(url).ok_or(Refused::Unknown)?;
            let videos = lister(&format!("{root}/videos"), list_most)?;
            h.cut = videos.entries.len() >= list_most;
            add(&mut h, VIDEOS, &videos.title, &videos);
            // THE PLAYLISTS TAB lists playlists, each an entry whose address
            // is the list's; failing to list it leaves the videos.
            if let Ok(playlists) = lister(&format!("{root}/playlists"), playlists_most) {
                h.cut |= playlists.entries.len() >= playlists_most;
                for entry in &playlists.entries {
                    let Some(id) = super::query_param(&entry.url, "list") else {
                        continue;
                    };
                    match lister(&entry.url, list_most) {
                        Ok(flat) => {
                            let title = if entry.title.is_empty() {
                                flat.title.clone()
                            } else {
                                entry.title.clone()
                            };
                            add(&mut h, &format!("list:{id}"), &title, &flat);
                        }
                        Err(_) => h.cut = true,
                    }
                }
            } else {
                h.cut = true;
            }
            if h.items.is_empty() {
                return Err(Refused::NothingPlayable);
            }
            Ok((videos.title, h))
        }
    }
}

/// [`youtube`], listed by yt-dlp (`ronoctua_ytdlp::flat_up_to`) within the
/// table's bounds. Blocks for as long as the listings take: call it on a
/// thread.
///
/// # Errors
///
/// As [`youtube`].
#[cfg(feature = "net")]
pub fn listed(list: YouTubeList, url: &str) -> Result<(String, Harvest), Refused> {
    youtube(
        list,
        url,
        &ronoctua_ytdlp::flat_up_to,
        crate::table::YOUTUBE_PLAYLISTS_MOST,
        crate::table::YOUTUBE_LIST_MOST,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, title: &str) -> FlatEntry {
        FlatEntry {
            url: format!("https://www.youtube.com/watch?v={id}"),
            title: title.to_owned(),
            duration_seconds: Some(61.5),
            thumbnail: Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")),
            channel: Some("Blender".to_owned()),
        }
    }

    /// A HAND-WRITTEN channel, in the shape `yt-dlp --flat-playlist
    /// --dump-single-json` answers for its tabs (see `ronoctua-ytdlp`'s own
    /// fixtures): its videos, its playlists, and a playlist that fails.
    fn channel(url: &str, most: usize) -> Result<Flat, Trouble> {
        let flat = |title: &str, entries: Vec<FlatEntry>| Flat {
            title: title.to_owned(),
            entries: entries.into_iter().take(most).collect(),
        };
        match url {
            "https://www.youtube.com/@BlenderOfficial/videos" => Ok(flat(
                "Blender - Videos",
                vec![
                    entry("aaaaaaaaaaa", "Reel"),
                    entry("bbbbbbbbbbb", "Sprite Fright"),
                ],
            )),
            "https://www.youtube.com/@BlenderOfficial/playlists" => Ok(flat(
                "Blender - Playlists",
                vec![
                    FlatEntry {
                        url: "https://www.youtube.com/playlist?list=PLfilms".to_owned(),
                        title: "Open Movies".to_owned(),
                        duration_seconds: None,
                        thumbnail: None,
                        channel: None,
                    },
                    FlatEntry {
                        url: "https://www.youtube.com/playlist?list=PLbroken".to_owned(),
                        title: "Broken".to_owned(),
                        duration_seconds: None,
                        thumbnail: None,
                        channel: None,
                    },
                ],
            )),
            "https://www.youtube.com/playlist?list=PLfilms" => Ok(flat(
                "Open Movies",
                vec![
                    entry("bbbbbbbbbbb", "Sprite Fright"),
                    entry("ccccccccccc", "Spring"),
                ],
            )),
            _ => Err(Trouble::Failed),
        }
    }

    #[test]
    fn a_channel_is_its_videos_and_each_playlist_a_category() {
        let (title, h) = youtube(
            YouTubeList::Channel,
            "https://www.youtube.com/@BlenderOfficial/featured",
            &channel,
            50,
            500,
        )
        .unwrap();
        assert_eq!(title, "Blender - Videos");
        let keys: Vec<&str> = h.categories.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, [VIDEOS, "list:PLfilms"]);
        assert_eq!(h.categories[1].title, "Open Movies");
        assert_eq!(
            h.categories[1].theme.as_deref(),
            Some("films"),
            "by its words"
        );
        assert!(h.cut, "the broken playlist is left out, and said");
        assert_eq!(h.items.len(), 4, "the writer merges the one in both");
        let reel = &h.items[0];
        assert_eq!(reel.ext_id, "aaaaaaaaaaa");
        assert_eq!(reel.also, ["Blender"]);
        assert_eq!(reel.duration_s, Some(61));
        assert_eq!(
            reel.poster_url.as_deref(),
            Some("https://i.ytimg.com/vi/aaaaaaaaaaa/hqdefault.jpg")
        );
        assert_eq!(
            reel.stream.origin,
            Origin::YouTube {
                id: "aaaaaaaaaaa".to_owned()
            }
        );
    }

    #[test]
    fn a_playlist_is_one_category_and_troubles_are_refusals() {
        let (title, h) = youtube(
            YouTubeList::Playlist,
            "https://www.youtube.com/playlist?list=PLfilms",
            &channel,
            50,
            1,
        )
        .unwrap();
        assert_eq!(title, "Open Movies");
        assert_eq!(h.items.len(), 1);
        assert!(h.cut, "one asked, one listed: there may be more");
        let absent = |_: &str, _: usize| -> Result<Flat, Trouble> { Err(Trouble::Absent) };
        assert_eq!(
            youtube(
                YouTubeList::Playlist,
                "https://www.youtube.com/playlist?list=x",
                &absent,
                5,
                5
            )
            .unwrap_err(),
            Refused::NoYtDlp
        );
        assert_eq!(
            youtube(
                YouTubeList::Channel,
                "https://www.youtube.com/watch?v=x",
                &channel,
                5,
                5
            )
            .unwrap_err(),
            Refused::Unknown
        );
    }

    #[test]
    fn a_channel_s_own_address_has_no_tab() {
        assert_eq!(
            channel_root("https://m.youtube.com/@Blender/videos?view=0").as_deref(),
            Some("https://www.youtube.com/@Blender")
        );
        assert_eq!(
            channel_root("https://www.youtube.com/channel/UCabc/playlists").as_deref(),
            Some("https://www.youtube.com/channel/UCabc")
        );
        assert_eq!(channel_root("https://www.youtube.com/watch?v=x"), None);
    }
}
