//! The two lookups that ask on their own, each only when the person switched
//! it on — lyrics for a song that has none (`lyrics_online`) and the
//! sponsored stretches of a YouTube video (`sponsorblock`) — both through
//! `ronoctua-lookup`, the one crate that talks to their hosts.
//!
//! A request is queued under the lock (`Engine::fetches`) and made OUTSIDE
//! it, on a thread of its own: a host on the internet is given seconds, and
//! the engine is not. The answer is kept only when the item it was for is
//! still the one playing.
//!
//! And what a stream of a directory is owed once it STARTS
//! (`directory.rs`, `Directories::started`): radio-browser's click, asked
//! only while the radio is let, and its place among the recently played —
//! a file of this machine's, written here for the same reason: nothing that
//! waits is done under the lock.

use ronoctua_broadcast::{
    picks::{Change, Pick, Shelf},
    radio::Radio,
    table,
};
use ronoctua_lookup::{internet::Internet, lrclib, sponsorblock};
use ronoctua_player::prefs;

use super::Shared;

/// A request to a host on the internet, and the item it is for.
#[derive(Debug, Clone)]
pub(crate) enum Fetch {
    /// The song's lyrics, from LRCLIB.
    Lyrics {
        item: String,
        artist: String,
        title: String,
        album: Option<String>,
        duration_seconds: Option<f64>,
    },
    /// The video's sponsored stretches, from SponsorBlock.
    Segments {
        item: String,
        id: String,
        categories: Vec<String>,
    },
    /// radio-browser told a station played — once its sound started.
    Click { server: Radio, uuid: String },
    /// A stream of a directory, kept at the front of the recently played.
    Recent { pick: Box<Pick> },
}

/// Makes one request on a thread of its own, and hands the answer to the
/// engine if the item it was for is still the one playing.
pub(crate) fn spawn_fetch(fetch: Fetch, shared: Shared) {
    let _ = std::thread::Builder::new()
        .name("player-fetch".into())
        .spawn(move || match fetch {
            Fetch::Lyrics {
                item,
                artist,
                title,
                album,
                duration_seconds,
            } => {
                let found = lrclib::one(
                    &Internet,
                    &artist,
                    &title,
                    album.as_deref(),
                    duration_seconds,
                );
                let record = match found {
                    Ok(r) => r,
                    Err(r) => {
                        tracing::info!(refusal = %r, "no lyrics came back");
                        return;
                    }
                };
                let Some((_, text)) = record.best() else {
                    return;
                };
                let lines = ronoctua_player::lyrics::parse(text);
                let (lock, bell) = &*shared;
                let Ok(mut e) = lock.lock() else { return };
                if e.playing().as_deref() != Some(item.as_str()) || !e.state.lyrics.is_empty() {
                    return;
                }
                e.state.lyrics = lines;
                e.state.lyrics_from = Some(prefs::LRCLIB_HOST.to_owned());
                e.broadcast();
                drop(e);
                bell.notify_all();
            }
            Fetch::Segments {
                item,
                id,
                categories,
            } => {
                let segments = match sponsorblock::fetch(&Internet, &id, &categories) {
                    Ok(s) => s,
                    Err(r) => {
                        tracing::info!(refusal = %r, "no segments came back");
                        return;
                    }
                };
                let (lock, bell) = &*shared;
                let Ok(mut e) = lock.lock() else { return };
                if e.playing().as_deref() != Some(item.as_str()) {
                    return;
                }
                e.segments = segments;
                e.jumped.clear();
                tracing::info!(segments = e.segments.len(), "sponsored segments to skip");
                drop(e);
                bell.notify_all();
            }
            Fetch::Click { server, uuid } => {
                match server.click(&ronoctua_broadcast::internet::Internet, &uuid) {
                    Ok(()) => tracing::debug!(station = %uuid, "radio-browser was told it played"),
                    Err(r) => {
                        tracing::info!(station = %uuid, refusal = ?r, "the click was not taken")
                    }
                }
            }
            Fetch::Recent { pick } => {
                if let Err(why) = Shelf::recent(table::RECENT_MOST).change(&Change::Add(*pick)) {
                    tracing::info!(?why, "the recently played could not be written");
                }
            }
        });
}
