//! FAST-ACCESS'S YOUTUBE (`/y`) — the person's catalogue first, YouTube's
//! own search after the pause, in one grid of pictures.
//!
//! # Two sources, one grid
//!
//! The video catalogue (`read::Catalog`, read-only) answers on the
//! index thread, at once: its videos ranked by the person's own scores, their
//! pictures already on disk. YouTube is asked after the pause through yt-dlp
//! (`ronoctua_ytdlp::search`, `ytsearchN:`, a stop the next key pulls), with
//! the person's pass for the host it asks. The view merges the two by the
//! video's id — what the catalogue has stays first, with its picture
//! (`fast_access::View::arrived`).
//!
//! # Shorts are left out by default
//!
//! A switch row at the top of the grid (and `C-s`) shows them; the setting
//! is kept in `fast-access.json`. Both sources honour it: the catalogue by
//! its filter, the search by `Found::short`.
//!
//! # The pictures
//!
//! The catalogue's are files of its own. YouTube's come from `i.ytimg.com`
//! — asked only with the person's pass for that host, through the video
//! catalogue's crate, the one boundary with YouTube over HTTP — and are kept
//! under `~/.cache/ronoctua/fast-access/thumbs`, `webThumbsMiB` at most, the
//! oldest going first. Every picture is decoded off the loop, at
//! `space.fastAccess.thumbPx`, under a key with its id, its size and the
//! format's version — the same video at the same size is the same texture.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ronoctua_tokens as tokens;

use crate::fast_access::{Filled, Hit, Said, SectionId, Video, What};
use crate::fast_access_search::{Ask, Back, Sink, Stops};

/// The version of how a picture is decoded: a change of it is a new key.
const PICTURE_FORMAT: u32 = 1;

/// The folder YouTube's pictures are kept in, under the cache root.
pub const THUMBS_DIR: &str = "fast-access/thumbs";

/// The key a video's picture is drawn under.
#[must_use]
pub fn picture_key(video: &str) -> String {
    format!(
        "fast-access-thumb:{video}:{}:{PICTURE_FORMAT}",
        tokens::space::FASTACCESS_THUMBPX
    )
}

/// The catalogue's videos the words find, Shorts as the switch says — the
/// switch's row first.
#[must_use]
pub fn local(words: &str, shorts: bool) -> Filled {
    let mut hits = vec![shorts_row(shorts)];
    let Some(path) = ronoctua_video_metrics::default_path() else {
        return Filled::Hits(hits);
    };
    // NO CATALOGUE (nothing scanned yet) is no local video, and the search of
    // YouTube still comes: not a refusal.
    let Ok(catalog) = ronoctua_video_metrics::read::Catalog::open_ro(&path) else {
        return Filled::Hits(hits);
    };
    let filter = ronoctua_video_metrics::read::Filter {
        text: Some(words.trim().to_owned()).filter(|w| !w.is_empty()),
        shorts: if shorts {
            ronoctua_video_metrics::read::Shorts::Any
        } else {
            ronoctua_video_metrics::read::Shorts::Without
        },
        ..ronoctua_video_metrics::read::Filter::default()
    };
    let page = ronoctua_video_metrics::read::Page {
        number: 0,
        per_page: u32::from(tokens::space::FASTACCESS_GRIDMOST),
    };
    match catalog.ranked(&filter, page) {
        Ok(rows) => hits.extend(rows.into_iter().map(|r| Hit {
            id: format!("yt:{}", r.id.0),
            title: r.title,
            note: Some(r.channel.clone()).filter(|c| !c.is_empty()),
            what: What::Video(Box::new(Video {
                id: r.id.0,
                url: r.url,
                channel: Some(r.channel).filter(|c| !c.is_empty()),
                duration_s: r.duration_s.map(f64::from),
                views: None,
                short: r.is_short,
                local: true,
                thumb_file: r.thumb_file,
                thumb: None,
            })),
        })),
        Err(r) => {
            tracing::info!(
                why = r.why.id(),
                "fast-access: the video catalogue did not answer"
            );
        }
    }
    Filled::Hits(hits)
}

/// The switch's row: Shorts shown or left out.
#[must_use]
pub fn shorts_row(shown: bool) -> Hit {
    Hit {
        id: "shorts".to_owned(),
        title: String::new(),
        note: None,
        what: What::Shorts(shown),
    }
}

/// YouTube's search, on a thread with its stop; then the pictures of what it
/// found, each on the same thread, asked only with the pass for their host.
pub fn online(ask: &Ask, stops: &Stops, sink: &Sink) {
    let host = ronoctua_ytdlp::SEARCH_HOST;
    let Ok(pass) = ronoctua_consent::pass(host) else {
        send(
            ask,
            stops,
            sink,
            Filled::Hits(vec![crate::fast_access::consent_hit(
                &[host],
                "fast-access-youtube-for",
            )]),
        );
        return;
    };
    let stop = stops.ytdlp();
    let abort = stops.web();
    let (ask, stops, sink) = (ask.clone(), stops.clone(), Arc::clone(sink));
    let spawned = std::thread::Builder::new()
        .name("fast-access-youtube".into())
        .spawn(move || {
            let shorts = if ask.shorts {
                ronoctua_ytdlp::Shorts::With
            } else {
                ronoctua_ytdlp::Shorts::Without
            };
            let most = usize::from(tokens::space::FASTACCESS_GRIDMOST);
            // `None`: the next key stopped it, and nobody waits.
            let Some(got) = ronoctua_ytdlp::search(&ask.words, most, shorts, &pass, &stop) else {
                return;
            };
            let found = match got {
                Ok(found) => found,
                Err(t) => {
                    send(
                        &ask,
                        &stops,
                        &sink,
                        Filled::Refused(Said::key(format!("fast-access-youtube-why-{}", t.id()))),
                    );
                    return;
                }
            };
            let thumbs_host = ronoctua_video_metrics::THUMBS_HOST;
            let thumbs_pass = ronoctua_consent::pass(thumbs_host).ok();
            let mut hits: Vec<Hit> = Vec::new();
            if thumbs_pass.is_none() && !found.is_empty() {
                hits.push(crate::fast_access::consent_hit(
                    &[thumbs_host],
                    "fast-access-thumbs-for",
                ));
            }
            hits.extend(found.iter().map(found_hit));
            send(&ask, &stops, &sink, Filled::Hits(hits));
            // THE PICTURES, one after the other, each from the cache when it
            // is there and from the host only with its pass.
            let kept = thumbs();
            for f in &found {
                if stops.pulled() {
                    return;
                }
                let file = kept
                    .as_ref()
                    .and_then(|k| k.path_of(&ronoctua_video_metrics::VideoId(f.id.clone())));
                let file = match file.filter(|p| p.is_file()) {
                    Some(p) => Some(p),
                    None => thumbs_pass.as_ref().and_then(|pass| {
                        let id = ronoctua_video_metrics::VideoId(f.id.clone());
                        let bytes = ronoctua_video_metrics::fetch_thumb(&id, pass, &abort)?;
                        let k = kept.as_ref()?;
                        let written = k.keep(&id, &bytes).ok();
                        let _ = k.trim();
                        written
                    }),
                };
                if let Some(file) = file
                    && let Some((key, rgba)) = decoded(&file)
                    && !stops.pulled()
                {
                    sink(Back::Picture {
                        video: f.id.clone(),
                        key,
                        rgba,
                    });
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "fast-access: YouTube's search could not start");
    }
}

/// YouTube's search for EVERYTHING — what is typed with no prefix: asked
/// only when the person already let the host (no consent row: they did not
/// ask for YouTube), Shorts left out unless `/y`'s switch lets them in, and
/// handed to the Videos section beside the catalogue's, which keeps its
/// place first ([`crate::fast_access::View::arrived`]). A refusal says
/// nothing — the section keeps what the catalogue found — and no picture is
/// fetched: the section is a list of names.
pub fn online_in_everything(ask: &Ask, stops: &Stops, sink: &Sink) {
    let Ok(pass) = ronoctua_consent::pass(ronoctua_ytdlp::SEARCH_HOST) else {
        return;
    };
    let stop = stops.ytdlp();
    let (ask, stops, sink) = (ask.clone(), stops.clone(), Arc::clone(sink));
    let spawned = std::thread::Builder::new()
        .name("fast-access-youtube-everything".into())
        .spawn(move || {
            let shorts = if ask.shorts {
                ronoctua_ytdlp::Shorts::With
            } else {
                ronoctua_ytdlp::Shorts::Without
            };
            let most = crate::fast_access::most_of(ask.scope, SectionId::Videos);
            // `None`: the next key stopped it; `Err`: refused, said nowhere.
            let Some(Ok(found)) = ronoctua_ytdlp::search(&ask.words, most, shorts, &pass, &stop)
            else {
                return;
            };
            if stops.pulled() || found.is_empty() {
                return;
            }
            sink(Back::Answer(crate::fast_access::Answer {
                generation: ask.generation,
                section: SectionId::Videos,
                filled: Filled::Hits(found.iter().map(found_hit).collect()),
            }));
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "fast-access: YouTube's search could not start");
    }
}

/// A video the search found, as a hit: not the catalogue's, no picture yet.
fn found_hit(f: &ronoctua_ytdlp::Found) -> Hit {
    Hit {
        id: format!("yt:{}", f.id),
        title: f.title.clone(),
        note: f.channel.clone(),
        what: What::Video(Box::new(Video {
            id: f.id.clone(),
            url: f.url.clone(),
            channel: f.channel.clone(),
            duration_s: f.duration_s,
            views: f.views,
            short: f.short(),
            local: false,
            thumb_file: None,
            thumb: None,
        })),
    }
}

/// The YouTube section's answer, unless the stops were pulled.
fn send(ask: &Ask, stops: &Stops, sink: &Sink, filled: Filled) {
    if stops.pulled() {
        return;
    }
    sink(Back::Answer(crate::fast_access::Answer {
        generation: ask.generation,
        section: SectionId::YouTube,
        filled,
    }));
}

/// The folder of YouTube's pictures, under its ceiling.
fn thumbs() -> Option<ronoctua_video_metrics::thumbs::Thumbs> {
    let root = ronoctua_xdg::cache_dir()?;
    Some(ronoctua_video_metrics::thumbs::Thumbs::at(
        root.join(THUMBS_DIR),
        u64::from(ronoctua_inspect::cache::policy::WEB_THUMBS_MIB) * 1024 * 1024,
    ))
}

/// A picture file decoded, no wider than `space.fastAccess.thumbPx`, and its
/// key. `None` for a file that is no picture.
#[must_use]
pub fn decoded(file: &Path) -> Option<(String, Arc<ronoctua_render::image::Rgba>)> {
    let picture = image::ImageReader::open(file)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .map_err(|e| tracing::info!(file = %file.display(), error = %e, "a video's picture that is no picture"))
        .ok()?;
    let most = u32::from(tokens::space::FASTACCESS_THUMBPX);
    let picture = if picture.width() > most {
        picture.thumbnail(most, most)
    } else {
        picture
    };
    let rgba = picture.to_rgba8();
    let (w, h) = rgba.dimensions();
    let video = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    ronoctua_render::image::Rgba::new(w, h, rgba.into_raw())
        .map(|rgba| (picture_key(&video), Arc::new(rgba)))
}

/// The pictures a section's videos want and have not asked for: their files
/// of the catalogue.
#[must_use]
pub fn wanted(hits: &[Hit], asked: &std::collections::BTreeSet<String>) -> Vec<(String, PathBuf)> {
    hits.iter()
        .filter_map(|h| match &h.what {
            What::Video(v) if v.thumb.is_none() && !asked.contains(&v.id) => {
                v.thumb_file.clone().map(|f| (v.id.clone(), f))
            }
            _ => None,
        })
        .collect()
}

/// A video's length, as a clock: `4:05`, `1:02:03`.
#[must_use]
pub fn length(seconds: f64) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let s = seconds.max(0.0).round() as u64;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_is_a_clock() {
        assert_eq!(length(245.0), "4:05");
        assert_eq!(length(3723.4), "1:02:03");
        assert_eq!(length(-3.0), "0:00");
    }

    /// FAILS IF: a video already pictured, or one asked for already, asks
    /// again — or one with a file does not ask.
    #[test]
    fn only_a_picture_not_asked_for_is_wanted() {
        let v = |id: &str, file: Option<&str>, thumb: Option<&str>| Hit {
            id: format!("yt:{id}"),
            title: String::new(),
            note: None,
            what: What::Video(Box::new(Video {
                id: id.to_owned(),
                url: String::new(),
                channel: None,
                duration_s: None,
                views: None,
                short: false,
                local: true,
                thumb_file: file.map(PathBuf::from),
                thumb: thumb.map(ToOwned::to_owned),
            })),
        };
        let hits = vec![
            v("aaaaaaaaaaa", Some("/t/a.jpg"), None),
            v("bbbbbbbbbbb", Some("/t/b.jpg"), Some("k")),
            v("ccccccccccc", None, None),
            v("ddddddddddd", Some("/t/d.jpg"), None),
        ];
        let asked: std::collections::BTreeSet<String> = ["ddddddddddd".to_owned()].into();
        let w = wanted(&hits, &asked);
        assert_eq!(
            w,
            vec![("aaaaaaaaaaa".to_owned(), PathBuf::from("/t/a.jpg"))]
        );
        assert!(picture_key("x").contains(&tokens::space::FASTACCESS_THUMBPX.to_string()));
    }
}
