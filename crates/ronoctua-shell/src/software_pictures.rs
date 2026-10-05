//! The software center's pictures: a program's own icon and a web app's icon
//! candidates, fetched, decoded and kept — for both doors.
//!
//! # Why a pool of its own and not the launcher's painter
//!
//! Because two of the three kinds come from the NETWORK: a Flathub result
//! carries an icon URL, and a web app's candidates are a site's manifest, a
//! Dashboard Icons drawing or a file somebody dropped. The launcher's painter
//! walks icon themes and never waits on a socket; a slow server here must not
//! hold up the icon of the next row. So this is `thumbs.rs`'s arrangement:
//! a few hands sharing one queue, answers by key, and `None` an ANSWER.
//!
//! # Every byte from outside is decoded by the software crate
//!
//! A URL's bytes and a candidate go through `ronoctua_software::icons` —
//! magic bytes decide the format, ceilings on every read, an SVG renderer that
//! refuses external references — and only its square PNG reaches the
//! launcher's rasteriser. The one path that does not is a program ALREADY
//! INSTALLED, whose icon is a file the launcher draws the same way already
//! (`ronoctua_notify::icon::rasterize_in`): drawing it differently here would
//! be the same program with two faces.
//!
//! # The key carries everything the pixels depend on
//!
//! The side in pixels (an `Rgba` is rasterised at a number of pixels, and the
//! renderer never drops a texture by key — `apps::pixmap_key`'s scar), where
//! the picture comes from, and [`FORMAT`], the version of how it is made. No
//! theme: nothing here is painted in a token's value, and a Dashboard Icons
//! drawing chosen for the theme's ground names its own file.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use ronoctua_software::found::Picture;
use ronoctua_software::order::IconChoice;
use ronoctua_tokens as tokens;

/// The version of how a picture is made. A change to the decoding that
/// changes the pixels bumps it, and every key with it.
pub const FORMAT: u32 = 1;

/// How many pictures are kept in memory. A result list is a few dozen rows
/// and an icon a few kibibytes at the body's size; past this the oldest go
/// and are asked for again if they come back on screen.
const MOST: usize = 512;

/// How many hands decode at once: a network answer takes a second, and one
/// hand would draw a list of twenty icons one second at a time.
const HANDS: usize = 3;

/// What a row shows as its picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Portrait {
    /// A program's own icon, wherever its source says it is.
    Program(Picture),
    /// A web app's icon candidate.
    Candidate(IconChoice),
}

/// The key one picture is filed under, at one side.
#[must_use]
pub fn pixmap_key(portrait: &Portrait, side: u32) -> String {
    // `\u{1}` BETWEEN THE PARTS, for `apps::pixmap_key`'s reason: a path or
    // a URL can hold any other byte, and two different pairs must never spell
    // the same key.
    let (kind, reference) = match portrait {
        Portrait::Program(Picture::Url { url }) => ("url", url.clone()),
        Portrait::Program(Picture::File { path }) => ("file", path.display().to_string()),
        Portrait::Program(Picture::Named { name }) => ("named", name.clone()),
        Portrait::Candidate(IconChoice::Site { url, .. }) => ("site", url.clone()),
        Portrait::Candidate(IconChoice::Dashboard { name, variant }) => {
            ("dashboard", format!("{name}\u{1}{variant}"))
        }
        Portrait::Candidate(IconChoice::Collection { name }) => ("collection", name.clone()),
        Portrait::Candidate(IconChoice::File { path }) => ("upload", path.display().to_string()),
    };
    format!("software\u{1}{kind}\u{1}{reference}\u{1}{side}\u{1}{FORMAT}")
}

/// Whether the theme is a DARK one: its text is lighter than its ground.
///
/// Measured on the theme and not written anywhere: a relation between two of
/// its colours, so a theme added tomorrow answers for itself. It picks which of
/// a Dashboard Icons drawing's variants is offered first — the light-coloured
/// drawing is the one made for a dark ground.
#[must_use]
pub fn dark_theme() -> bool {
    luminance(tokens::color::TEXT_PRIMARY) > luminance(tokens::color::SURFACE_BASE)
}

/// Relative luminance, from the linear components.
fn luminance(c: tokens::Color) -> f32 {
    let [r, g, b, _] = c.to_linear();
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// One ask for the hands.
struct Ask {
    key: String,
    portrait: Portrait,
    side: u32,
}

/// One answer.
struct Painted {
    key: String,
    rgba: Option<ronoctua_render::image::Rgba>,
}

/// What the hands need, read once when they start.
#[derive(Clone)]
struct Where {
    /// The chain the launcher draws installed programs' icons from.
    icon_roots: ronoctua_notify::icon::Roots,
    /// The distro's collection (the superpack), for a candidate from it.
    collection: Option<PathBuf>,
    /// Where a candidate's square PNG is written while it is only a preview.
    previews: PathBuf,
}

/// The pictures, kept and asked for.
pub struct Pictures {
    kept: HashMap<String, Option<ronoctua_render::image::Rgba>>,
    /// Insertion order, for letting the oldest go.
    order: VecDeque<String>,
    /// Asked and not answered yet.
    asked: HashSet<String>,
    queue: Option<mpsc::Sender<Ask>>,
    arrived: Arc<Mutex<Vec<Painted>>>,
}

impl std::fmt::Debug for Pictures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pictures")
            .field("kept", &self.kept.len())
            .field("asked", &self.asked.len())
            .finish_non_exhaustive()
    }
}

impl Pictures {
    /// The session's pictures. `wake` is called from a hand whenever an answer
    /// is ready; the door answers it with [`Pictures::take`] and a repaint.
    #[must_use]
    pub fn new(
        roots: &ronoctua_software::Roots,
        tools: &ronoctua_software::Tools,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let arrived = Arc::new(Mutex::new(Vec::new()));
        let place = Where {
            icon_roots: ronoctua_notify::icon::Roots::of_session(tools.icons.clone()),
            collection: tools.icons.clone(),
            previews: roots.ours_cache().join("previews"),
        };
        let queue = hands(place, Arc::clone(&arrived), Arc::new(wake));
        Self {
            kept: HashMap::new(),
            order: VecDeque::new(),
            asked: HashSet::new(),
            queue,
            arrived,
        }
    }

    /// Nothing kept, and no hands: every picture stays its glyph. For tests,
    /// and for a door whose threads could not start.
    #[cfg(test)]
    #[must_use]
    pub fn none() -> Self {
        Self {
            kept: HashMap::new(),
            order: VecDeque::new(),
            asked: HashSet::new(),
            queue: None,
            arrived: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// The picture at this side, when it is ready — and asked for when it is
    /// not. `None` while it is on its way AND when it has none: the row draws
    /// its glyph either way, and asks nothing twice.
    pub fn get(&mut self, portrait: &Portrait, side: u32) -> Option<&ronoctua_render::image::Rgba> {
        let key = pixmap_key(portrait, side);
        if !self.kept.contains_key(&key) && !self.asked.contains(&key) {
            if let Some(q) = &self.queue
                && q.send(Ask {
                    key: key.clone(),
                    portrait: portrait.clone(),
                    side,
                })
                .is_ok()
            {
                self.asked.insert(key.clone());
            } else {
                // NO HANDS IS AN ANSWER TOO: the glyph, for good.
                self.keep(key.clone(), None);
            }
        }
        self.kept.get(&key).and_then(Option::as_ref)
    }

    /// The KEY of this picture when its pixels are here — what a door hands
    /// its painter, which asks the GPU whether the texture is up and uploads
    /// it by [`Pictures::pixels`] when it is not. Asks for nothing: the asking
    /// is [`Pictures::get`]'s, made before the frame, where the door may write.
    #[must_use]
    pub fn ready(&self, portrait: &Portrait, side: u32) -> Option<String> {
        let key = pixmap_key(portrait, side);
        matches!(self.kept.get(&key), Some(Some(_))).then_some(key)
    }

    /// The pixels filed under a key [`Pictures::ready`] gave.
    #[must_use]
    pub fn pixels(&self, key: &str) -> Option<&ronoctua_render::image::Rgba> {
        self.kept.get(key).and_then(Option::as_ref)
    }

    /// Whether this picture has been answered, pixels or not.
    #[cfg(test)]
    #[must_use]
    pub fn answered(&self, portrait: &Portrait, side: u32) -> bool {
        self.kept.contains_key(&pixmap_key(portrait, side))
    }

    /// Everything the hands finished since the last call. Answers the keys
    /// that arrived WITH pixels, which the door uploads.
    pub fn take(&mut self) -> Vec<String> {
        let done: Vec<Painted> = self
            .arrived
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        let mut with_pixels = Vec::new();
        for p in done {
            self.asked.remove(&p.key);
            if p.rgba.is_some() {
                with_pixels.push(p.key.clone());
            }
            self.keep(p.key, p.rgba);
        }
        with_pixels
    }

    /// Keep one answer, letting the oldest go past [`MOST`].
    fn keep(&mut self, key: String, rgba: Option<ronoctua_render::image::Rgba>) {
        if self.kept.insert(key.clone(), rgba).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > MOST {
            if let Some(old) = self.order.pop_front() {
                self.kept.remove(&old);
            }
        }
    }
}

/// Starts the hands. `None` when not one of them could start.
fn hands(
    place: Where,
    arrived: Arc<Mutex<Vec<Painted>>>,
    wake: Arc<dyn Fn() + Send + Sync>,
) -> Option<mpsc::Sender<Ask>> {
    let (queue, asks) = mpsc::channel::<Ask>();
    let asks = Arc::new(Mutex::new(asks));
    let mut born = 0;
    for n in 0..HANDS {
        let asks = Arc::clone(&asks);
        let arrived = Arc::clone(&arrived);
        let wake = Arc::clone(&wake);
        let place = place.clone();
        let made = std::thread::Builder::new()
            .name(format!("ronoctua-software-pictures-{n}"))
            .spawn(move || {
                loop {
                    // THE LOCK IS HELD FOR THE RECEIVE AND NOT FOR THE WORK —
                    // `thumbs.rs`, and its reason.
                    let Some(ask) = asks.lock().ok().and_then(|q| q.recv().ok()) else {
                        return;
                    };
                    let rgba = painted(&ask.portrait, ask.side, &place);
                    if let Ok(mut q) = arrived.lock() {
                        q.push(Painted { key: ask.key, rgba });
                    }
                    wake();
                }
            });
        match made {
            Ok(_) => born += 1,
            Err(e) => tracing::warn!(error = %e, "a picture hand did not start"),
        }
    }
    (born > 0).then_some(queue)
}

/// One picture, made. Touches the disk and the network: a hand's work.
fn painted(portrait: &Portrait, side: u32, place: &Where) -> Option<ronoctua_render::image::Rgba> {
    let rgba = match portrait {
        // INSTALLED, drawn as the launcher draws it.
        Portrait::Program(Picture::Named { name }) => {
            ronoctua_notify::icon::rasterize_in(&place.icon_roots, name, side)
        }
        Portrait::Program(Picture::File { path }) => {
            ronoctua_notify::icon::rasterize_in(&place.icon_roots, &path.to_string_lossy(), side)
        }
        // FROM SOMEBODY ELSE'S SERVER, through the crate's decoder.
        Portrait::Program(Picture::Url { url }) => {
            let bytes = fetched(url)?;
            let png = ronoctua_software::icons::normalised(&bytes).ok()?;
            ronoctua_notify::icon::from_raster(&png, side)
        }
        Portrait::Candidate(choice) => {
            let file = ronoctua_software::icons::prepare(
                choice,
                place.collection.as_deref(),
                &place.previews,
            )
            .ok()?;
            let png = std::fs::read(file).ok()?;
            ronoctua_notify::icon::from_raster(&png, side)
        }
    }?;
    Some(ronoctua_render::image::Rgba {
        width: rgba.width,
        height: rgba.height,
        pixels: rgba.pixels,
    })
}

/// An icon's bytes from the network, under the crate's ceiling.
fn fetched(url: &str) -> Option<Vec<u8>> {
    match ronoctua_software::net::get(url, ronoctua_software::icons::ICON_MOST) {
        Ok(got) => Some(got.reply.body),
        Err(why) => {
            tracing::debug!(url, why = why.word(), "an icon could not be fetched");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_carries_the_side_the_source_and_the_format() {
        let a = Portrait::Program(Picture::Named {
            name: "gimp".into(),
        });
        assert_ne!(
            pixmap_key(&a, 18),
            pixmap_key(&a, 36),
            "two sides, one texture"
        );
        let b = Portrait::Program(Picture::File {
            path: "gimp".into(),
        });
        assert_ne!(
            pixmap_key(&a, 18),
            pixmap_key(&b, 18),
            "a name and a path spelt alike"
        );
        let light = Portrait::Candidate(IconChoice::Dashboard {
            name: "github".into(),
            variant: String::new(),
        });
        let dark = Portrait::Candidate(IconChoice::Dashboard {
            name: "github".into(),
            variant: "light".into(),
        });
        assert_ne!(pixmap_key(&light, 18), pixmap_key(&dark, 18));
        assert!(pixmap_key(&a, 18).ends_with(&format!("\u{1}{FORMAT}")));
        assert_eq!(pixmap_key(&a, 18), pixmap_key(&a, 18));
    }

    #[test]
    fn without_hands_every_picture_is_answered_with_nothing_once() {
        let mut p = Pictures::none();
        let a = Portrait::Program(Picture::Named {
            name: "gimp".into(),
        });
        assert!(p.get(&a, 18).is_none());
        // SOME(NONE) IS AN ANSWER: asked once, and never again.
        assert!(p.answered(&a, 18));
        assert!(p.asked.is_empty());
    }

    #[test]
    fn the_oldest_picture_goes_first() {
        let mut p = Pictures::none();
        for n in 0..=MOST {
            p.keep(format!("k{n}"), None);
        }
        assert_eq!(p.kept.len(), MOST);
        assert!(!p.kept.contains_key("k0"));
        assert!(p.kept.contains_key(&format!("k{MOST}")));
    }

    #[test]
    fn an_answer_is_kept_and_its_key_handed_back() {
        let mut p = Pictures::none();
        let a = Portrait::Program(Picture::Named {
            name: "gimp".into(),
        });
        let key = pixmap_key(&a, 18);
        p.asked.insert(key.clone());
        p.arrived.lock().unwrap().push(Painted {
            key: key.clone(),
            rgba: Some(ronoctua_render::image::Rgba {
                width: 1,
                height: 1,
                pixels: vec![0, 0, 0, 255],
            }),
        });
        assert_eq!(p.take(), vec![key]);
        assert!(p.get(&a, 18).is_some());
        assert!(p.asked.is_empty());
    }
}
