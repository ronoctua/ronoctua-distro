//! The thumbnails: the workers that make them, and what has already been made.
//!
//! # Why a POOL, and not the one thread the preview has
//!
//! Because they want opposite things. `files::inspector` is one thread that
//! drains its queue to the NEWEST and throws the rest away, and that is right
//! for a panel about the file under the cursor: holding an arrow key down over
//! a folder of videos is sixty asks a second and fifty-nine of them are about
//! files the cursor has already left.
//!
//! A grid asks about everything on screen at once, and it WANTS every answer.
//! Put through the inspector's shape, a folder of sixty-four files would draw
//! one thumbnail. So this is `apps::painter`'s arrangement instead — requests
//! that are all wanted, answers by key — with the one thing that one does not
//! have: several hands, because the cost here is not a lookup but a child
//! process.
//!
//! # And the number of hands was measured
//!
//! Against the owner's own folder — sixty-four files, thirty-six extensions.
//! One thumbnail costs about 110 ms whatever the file is: a 22 KB JPEG took
//! 109 ms and a 355 MB video 167, because what is being paid for is the
//! PROCESS and not the picture. The whole folder came to 8 129 ms in one hand,
//! 2 229 in four, 1 681 in six and 1 293 in twelve — so past half the machine
//! the returns fall away and what is left is a file manager that has taken the
//! machine to draw itself.
//!
//! Half the cores, floored at two and capped, is what that says.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};

use smithay_client_toolkit::reexports::calloop;

/// One thumbnail to make.
///
/// It carries its own KEY rather than being identified by its path: the key
/// carries the size and the theme as well, and two of those change without the
/// file changing. See `ronoctua_inspect::Stamp`.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    /// The file, its modification time, its size and the step it is decoded at.
    ///
    /// The whole stamp and not a path, because the DISK cache is keyed on all
    /// four of them: a file edited in place, one that grew, and a window
    /// resized past a step boundary are three different pictures of one name.
    pub stamp: ronoctua_inspect::Stamp,
    /// The type to act on — already through `ronoctua_inspect::settled`.
    pub mime: &'static str,
    /// The cache key this answer will carry back.
    pub key: String,
    /// The waveform's ink, for a sound with no artwork.
    ///
    /// It travels because it is a TOKEN and `ronoctua-inspect` does not receive
    /// token values — the same reason the inspector's own ask carries one.
    pub wave: String,
    /// A film that might belong to this picture, from the SAME folder.
    ///
    /// An iPhone's Live Photo is two files — `IMG_1.HEIC` and `IMG_1.MOV` — and
    /// only the VIEW knows what else is in the directory. The pair is confirmed
    /// by what is inside them and never by the name; this is the candidate.
    pub beside: Option<PathBuf>,
    /// How a SHAPE is lit and painted.
    ///
    /// It travels for the reason `wave` does: every number in it comes out of a
    /// token, and `ronoctua-inspect` receives no token values.
    pub paint: ronoctua_inspect::solid::Paint,
    /// WHAT MAKES THIS PICTURE DIFFERENT beyond the file and the size.
    ///
    /// The theme's position, as a string. The memory cache does not need it —
    /// its key carries the theme already — but the DISK cache is filed under
    /// the stamp, and a stamp has no theme in it. A waveform and a shape are
    /// both painted in token VALUES, so a picture made in one theme and read
    /// back in another would be the one thing in this cache that comes out
    /// wrong rather than missing.
    pub tint: String,
}

/// One thumbnail, made or refused.
#[derive(Debug)]
pub struct Made {
    /// Which ask this answers.
    pub key: String,
    /// The pixels, when the file had any.
    ///
    /// `None` is an ANSWER — *this file has no picture* — and not an absence.
    /// Without somewhere to write that down, a document would be asked about
    /// again on every frame, and a folder of them would be a fork per frame per
    /// cell. It is the same distinction `surface`'s icon cache already makes
    /// with its `Some(None)`.
    pub rgba: Option<ronoctua_render::image::Rgba>,
    /// How long the film runs, for a file that is one.
    ///
    /// Answered by the worker for the reason `moves` is: it costs an `ffprobe`
    /// and the worker is already off the event loop. The pointer's clips need
    /// it to know WHERE to seek — a film is walked at a few evenly spaced
    /// points, and evenly spaced over what is the question this answers.
    pub seconds: Option<f64>,
    /// Where the moving part of a PHOTOGRAPH lives, when it has one.
    ///
    /// `Some` is the definition of a motion photo. It is answered here for the
    /// reason `seconds` is: it costs a read of the file's own bytes and the
    /// worker is off the event loop — and without it the pointer resting on one
    /// would find a picture and nothing to play, which the owner reported.
    pub inside: Option<ronoctua_inspect::motion::Inside>,
    /// Whether this file MOVES, and can be moved in process.
    ///
    /// Answered by the worker because it has the file open anyway, and the test
    /// is two frames — see `ronoctua_inspect::motion::animates`. Asked in the
    /// view it would be an open and a decode per cell per frame.
    pub moves: bool,
}

/// How many workers to run.
///
/// Half the machine, so the other half is still drawing. Two at least — one
/// hand is the 8-second folder — and never more than eight, because past that
/// the measurement above says the wall clock stops falling and only the fan
/// notices.
fn hands() -> usize {
    std::thread::available_parallelism()
        .map_or(2, std::num::NonZeroUsize::get)
        .div_ceil(2)
        .clamp(2, 8)
}

/// Starts the workers. `None` when not one of them could be spawned.
///
/// Two channels, as every worker in this shell has: an `mpsc` in, because only
/// it has a blocking `recv` and a hand with nothing to do should sleep, and the
/// loop's own `calloop` channel out.
///
/// # And it is a channel of its OWN, not the reports'
///
/// A grid answers dozens of times while a folder opens, and putting those
/// through the channel that also carries "the directory changed" would make
/// every one of them wake the parts of the door that read a directory. The
/// doors already keep four channels apart for exactly this; this is the fifth.
/// `keep` is where thumbnails are kept between sessions — the door calls
/// `ronoctua_xdg::cache_dir()` once and hands the answer down, which is the
/// rule that crate's own header states: a worker that found the root itself
/// would write into the real cache of whoever ran the tests.
/// `shared` is `$XDG_CACHE_HOME` itself, where the freedesktop standard's
/// `thumbnails/` directory lives — see `ronoctua_inspect::cache::shared`.
pub fn pool(
    out: calloop::channel::Sender<Made>,
    keep: Option<PathBuf>,
    shared: Option<PathBuf>,
) -> Option<mpsc::Sender<Ask>> {
    let (queue, asks) = mpsc::channel::<Ask>();
    let asks = Arc::new(Mutex::new(asks));
    let mut born = 0;
    for n in 0..hands() {
        let asks = Arc::clone(&asks);
        let out = out.clone();
        let keep = keep.clone();
        let shared = shared.clone();
        let made = std::thread::Builder::new()
            .name(format!("ronoctua-thumbs-{n}"))
            .spawn(move || {
                loop {
                    // THE LOCK IS HELD FOR THE RECEIVE AND NOT FOR THE WORK.
                    //
                    // Held across the decode it would be one hand with extra
                    // steps: every other worker would be waiting on the mutex
                    // rather than on a file. The `else` covers both a poisoned
                    // lock and a closed queue, which mean the same thing here —
                    // there is nothing more to do.
                    let Some(ask) = asks.lock().ok().and_then(|q| q.recv().ok()) else {
                        return;
                    };
                    // THE DISK FIRST. Measured: making one costs 109 ms at
                    // best because the floor is a child process, and reading
                    // one back is an open and a read.
                    // WHICH CACHE THIS PICTURE BELONGS IN.
                    //
                    // What the FILE looks like goes where every other manager
                    // on the machine keeps theirs; what WE painted stays in
                    // ours. A waveform is drawn in a token's value and a mesh
                    // is lit by our own lamp: neither is a thumbnail of the
                    // file in a sense another program would recognise, both
                    // change with the theme, and the standard has nowhere to
                    // say so.
                    //
                    // It is answered from the TYPE and not from what came back,
                    // because it has to be answered before anything is read.
                    let painted =
                        ronoctua_inspect::pixels::turns(ask.mime) || ask.mime.starts_with("audio/");
                    let kept = if painted {
                        keep.as_deref()
                            .and_then(|k| ronoctua_inspect::cache::read(k, &ask.stamp, &ask.tint))
                    } else {
                        shared.as_deref().and_then(|k| {
                            ronoctua_inspect::cache::shared::read(
                                k,
                                &ask.stamp.path,
                                ask.stamp.mtime,
                                ask.stamp.target,
                            )
                        })
                    };
                    let made = kept.or_else(|| {
                        // A SHAPE IS DRAWN, NOT DECODED, and that is why it gets
                        // a thumbnail here when the panel's preview leaves it to
                        // the turning stage: the stage exists to turn ONE piece
                        // under the hand and keeps its body in memory to do it,
                        // and sixty bodies would be the file manager's memory.
                        // One still needs no body afterwards.
                        //
                        // Measured on the owner's own files: 18 ms for a small
                        // piece and 44 for the 225 000-triangle Benchy, which is
                        // a third of what one ffmpeg costs.
                        let made = if ronoctua_inspect::pixels::turns(ask.mime) {
                            let read =
                                ronoctua_inspect::mesh::read(&ask.stamp.path, ask.mime).ok()?;
                            ronoctua_inspect::solid::draw(
                                &read.body,
                                ronoctua_inspect::solid::Pose::default(),
                                ask.stamp.target,
                                ask.stamp.target,
                                ask.paint,
                            )
                        } else {
                            ronoctua_inspect::pixels::preview(
                                &ask.stamp.path,
                                ask.mime,
                                ask.stamp.target,
                                &ask.wave,
                            )
                            .rgba?
                        };
                        // AND WHAT WAS MADE IS KEPT. Only a picture: a refusal
                        // is cheap to reach again (the decoder answers from the
                        // type in most cases) and writing one down would mean a
                        // file that has to be swept when a reader is installed.
                        if painted {
                            if let Some(k) = keep.as_deref() {
                                ronoctua_inspect::cache::write(k, &ask.stamp, &ask.tint, &made);
                            }
                        } else if let Some(k) = shared.as_deref() {
                            ronoctua_inspect::cache::shared::write(
                                k,
                                &ask.stamp.path,
                                ask.stamp.mtime,
                                ask.stamp.target,
                                &made,
                            );
                        }
                        Some(made)
                    });
                    let rgba = made.map(|r| ronoctua_render::image::Rgba {
                        width: r.width,
                        height: r.height,
                        pixels: r.pixels,
                    });
                    // AND WHETHER IT MOVES, while the file is already in hand.
                    // Only for something that HAS a picture: a document that
                    // moves is not a thing.
                    let moves = rgba.is_some()
                        && ronoctua_inspect::motion::animates(&ask.stamp.path, ask.mime);
                    // AND HOW LONG IT RUNS, for a film. One `ffprobe`, once,
                    // on a thread that is already paying for a child process —
                    // and never for anything that is not a film, because the
                    // answer is only ever used to walk one.
                    let seconds = ask
                        .mime
                        .starts_with("video/")
                        .then(|| ronoctua_inspect::facts::duration_of(&ask.stamp.path))
                        .flatten();
                    // AND WHETHER A PHOTOGRAPH HAS A FILM IN IT.
                    //
                    // Asked of pictures only, and with no probe: the motion
                    // photo branch of `moving` is the FIRST thing it tries and
                    // reads the file's own bytes — a 128 KiB XMP window and a
                    // tail scan — so the empty probe costs nothing and no
                    // `ffprobe` is spawned for it.
                    let carried = ask.mime.starts_with("image/").then(|| {
                        ronoctua_inspect::motion::moving(
                            &ask.stamp.path,
                            ask.mime,
                            "",
                            ask.beside.as_deref(),
                        )
                    });
                    let carried = carried.flatten().filter(|m| m.inside.is_some());
                    let seconds = seconds.or_else(|| carried.as_ref()?.shape.seconds);
                    let inside = carried.and_then(|m| m.inside);
                    if out
                        .send(Made {
                            key: ask.key,
                            rgba,
                            seconds,
                            inside,
                            moves,
                        })
                        .is_err()
                    {
                        // A closed channel = the door left.
                        return;
                    }
                }
            });
        match made {
            Ok(_) => born += 1,
            Err(e) => tracing::warn!(error = %e, "a thumbnail worker did not start"),
        }
    }
    (born > 0).then_some(queue)
}

/// THE KEY ONE CELL'S PICTURE IS FILED UNDER.
///
/// # Why the size and the theme are IN it
///
/// Because an `Rgba` is rasterised at a number of PIXELS, and a cache that
/// cannot tell two sizes apart keeps the small one — which `Area::centre` then
/// draws at its own size inside a doubled box. `apps::pixmap_key` and
/// `buffer::svg_key` carry the same scar, and the owner watched the second of
/// them: a launcher opened before a change to 200% drew 18 px textures over
/// 36 px boxes for the life of the footer.
///
/// `Stamp::key()` already carries the file, its modification time, its size and
/// the decode STEP; what it has no room for is the theme, and a waveform is
/// painted in a token's VALUE. So the tint goes on the end.
///
/// AND IT LIVES HERE, beside the map it keys. It was built in `files.rs` for a
/// round, which is a key function a long way from its cache — and
/// `one-text-size` said so, in those terms.
#[must_use]
pub fn thumb_key(stamp: &ronoctua_inspect::Stamp, tint: &str) -> String {
    format!("{}:{tint}", stamp.key())
}

/// ONE CELL THAT MOVES BY ITSELF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moving {
    /// The key its frames arrive under — the same key its still picture had, so
    /// the texture is replaced rather than a second one made.
    pub key: String,
    /// The file, absolute.
    pub path: std::path::PathBuf,
    /// Which of the three in-process formats it is.
    pub mime: &'static str,
    /// The longest side its frames are scaled to.
    pub side: u32,
}

/// What the animator is told: THESE, and only these, from now on.
///
/// The whole set and not a delta, for the reason `stage.rs` gives about a pose:
/// what the animator has to know is what is on screen, and enumerating the ways
/// a cell can arrive or leave — scrolled, filtered, resized, the mode turned
/// over — is how one gets missed. One question, one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wind(pub Vec<Moving>);

/// HOW OFTEN ONE CELL MAY BE REDRAWN, in milliseconds.
///
/// A hundred, so ten frames a second. Measured: compositing and scaling one
/// frame of a four-hundred-pixel GIF costs 2,35 ms on this machine, so twelve
/// cells at this rate is 28% of one core — and the rate is what that number is
/// linear in.
///
/// AND A FASTER FILE IS SKIPPED, NOT SLOWED. A GIF authored at twenty frames a
/// second is advanced two frames and shown one, so it takes exactly as long to
/// loop as its author said. Slowing it instead would be a thumbnail that runs
/// at half speed, which is a different picture of the file rather than a
/// cheaper one.
const SLOWEST_MS: u64 = 100;

/// A GIF MAY DECLARE NOUGHT for a frame's delay, and browsers have clamped that
/// for twenty years; `ronoctua_inspect::motion` makes the same argument about
/// the preview's player.
const QUICKEST_MS: u64 = 20;

/// Starts the animator. `None` when the thread could not be raised.
///
/// # Why it is a thread of its own and not one of the pool's hands
///
/// Because a hand is BUSY for a hundred milliseconds at a time — that is the
/// cost of a child process — and an animation is a frame every hundred
/// milliseconds for as long as the folder is open. One would take a hand out of
/// the pool for the life of the view; all of them would mean no thumbnails get
/// made at all while anything is moving.
///
/// Its frames come back on the SAME channel as the still pictures, under the
/// same keys: a frame is a thumbnail that changed, and the door already knows
/// what to do with one.
pub fn animator(out: calloop::channel::Sender<Made>) -> Option<mpsc::Sender<Wind>> {
    let (queue, orders) = mpsc::channel::<Wind>();
    std::thread::Builder::new()
        .name("ronoctua-thumbs-motion".to_owned())
        .spawn(move || {
            let mut slots: Vec<Slot> = Vec::new();
            loop {
                // HOW LONG UNTIL THE EARLIEST CELL IS DUE. With nothing to
                // animate there is no deadline at all and the thread SLEEPS on
                // the channel — which is what makes a folder of documents cost
                // nothing.
                let now = std::time::Instant::now();
                let wait = slots
                    .iter()
                    .map(|s| s.due.saturating_duration_since(now))
                    .min();
                let order = match wait {
                    Some(w) => match orders.recv_timeout(w) {
                        Ok(o) => Some(o),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    },
                    None => match orders.recv() {
                        Ok(o) => Some(o),
                        Err(_) => return,
                    },
                };
                if let Some(mut wind) = order {
                    // ONLY THE NEWEST. Scrolling emits one of these per frame,
                    // and every one but the last is about a screen that has
                    // already gone.
                    while let Ok(newer) = orders.try_recv() {
                        wind = newer;
                    }
                    retune(&mut slots, wind.0);
                    continue;
                }
                let now = std::time::Instant::now();
                for slot in &mut slots {
                    if slot.due > now {
                        continue;
                    }
                    match slot.next_frame() {
                        Some(made) => {
                            if out.send(made).is_err() {
                                return;
                            }
                        }
                        // IT COULD NOT BE READ AT ALL: the file went away, or
                        // was never what its name said. The slot stops rather
                        // than spinning on a file that will not open.
                        None => slot.due = now + std::time::Duration::from_secs(3600),
                    }
                }
            }
        })
        .ok()?;
    Some(queue)
}

/// One animated cell, mid-loop.
struct Slot {
    what: Moving,
    frames: image::Frames<'static>,
    due: std::time::Instant,
}

impl Slot {
    /// Opens a file's frames. `None` when it is not one of the three.
    fn open(what: &Moving) -> Option<image::Frames<'static>> {
        use image::AnimationDecoder;

        let file = std::io::BufReader::new(std::fs::File::open(&what.path).ok()?);
        Some(match what.mime {
            "image/gif" => image::codecs::gif::GifDecoder::new(file)
                .ok()?
                .into_frames(),
            "image/webp" => image::codecs::webp::WebPDecoder::new(file)
                .ok()?
                .into_frames(),
            _ => image::codecs::png::PngDecoder::new(file)
                .ok()?
                .apng()
                .ok()?
                .into_frames(),
        })
    }

    /// The next frame worth drawing, and when the one after it is due.
    ///
    /// It walks frames until their delays add up to [`SLOWEST_MS`], so a file
    /// faster than the cell's rate is SKIPPED rather than slowed — see that
    /// constant. Running off the end re-opens: a loop is a re-read, which is
    /// what keeps one frame in memory instead of the film.
    fn next_frame(&mut self) -> Option<Made> {
        let mut waited = 0u64;
        let mut last = None;
        let mut reopened = false;
        while waited < SLOWEST_MS {
            let Some(frame) = self.frames.next() else {
                if reopened {
                    // A FILE OF ONE FRAME AFTER ALL. Re-opening again would
                    // spin; the worker's `animates` said two, so something
                    // changed underneath.
                    return None;
                }
                self.frames = Self::open(&self.what)?;
                reopened = true;
                continue;
            };
            let frame = frame.ok()?;
            let (num, den) = frame.delay().numer_denom_ms();
            waited += u64::from(num / den.max(1)).max(QUICKEST_MS);
            last = Some(frame.into_buffer());
        }
        let picture = scaled(last?, self.what.side)?;
        self.due = std::time::Instant::now() + std::time::Duration::from_millis(waited);
        Some(Made {
            key: self.what.key.clone(),
            rgba: Some(picture),
            seconds: None,
            inside: None,
            moves: true,
        })
    }
}

/// Keeps the slots that are still wanted, opens the ones that are new, and
/// stops at the ceiling.
fn retune(slots: &mut Vec<Slot>, want: Vec<Moving>) {
    // WHAT IS STILL THERE KEEPS ITS PLACE IN ITS OWN LOOP. Re-opening a cell
    // that never left would restart every animation on screen each time the
    // list scrolled by one.
    slots.retain(|s| want.contains(&s.what));
    let ceiling = usize::from(ronoctua_tokens::space::FILES_THUMBANIMATORS);
    for what in want {
        if slots.len() >= ceiling {
            break;
        }
        if slots.iter().any(|s| s.what == what) {
            continue;
        }
        if let Some(frames) = Slot::open(&what) {
            slots.push(Slot {
                what,
                frames,
                due: std::time::Instant::now(),
            });
        }
    }
}

/// A frame at the thumbnail's size, never enlarged.
///
/// No padding to a fixed canvas, unlike the preview's player: there the frame
/// has to keep the box's shape because the box is what was measured, and here
/// the box is worked out FROM the picture — `View::grid_pictures` centres
/// whatever arrives.
fn scaled(picture: image::RgbaImage, side: u32) -> Option<ronoctua_render::image::Rgba> {
    let (w, h) = (picture.width(), picture.height());
    if w == 0 || h == 0 || side == 0 {
        return None;
    }
    let by = (f64::from(side) / f64::from(w.max(h))).min(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (to_w, to_h) = (
        ((f64::from(w) * by).round() as u32).max(1),
        ((f64::from(h) * by).round() as u32).max(1),
    );
    let out = if (to_w, to_h) == (w, h) {
        picture
    } else {
        // THE SAME FILTER THE STILL PICTURE USES, so a file does not change
        // character when it starts moving.
        image::imageops::resize(&picture, to_w, to_h, image::imageops::FilterType::Lanczos3)
    };
    ronoctua_render::image::Rgba::new(to_w, to_h, out.into_raw())
}

/// The thumbnails already made, and the ones already asked about.
///
/// # Why `Some(None)` is not the same as absent
///
/// `made` holds an `Option` inside an `Option`: the outer says whether this
/// file has been answered about, the inner whether the answer was a picture. A
/// document has no picture and never will, and without somewhere to write that
/// down the grid would ask again on every frame — which is a child process per
/// frame per cell.
#[derive(Debug, Default)]
pub struct Kept {
    made: HashMap<String, Option<ronoctua_render::image::Rgba>>,
    asked: HashSet<String>,
    /// How long each film runs, by key.
    films: HashMap<String, f64>,
    /// Where a photograph's film lives, for the ones that carry one.
    carried: HashMap<String, ronoctua_inspect::motion::Inside>,
    /// The keys whose file moves by itself.
    ///
    /// Kept apart from `made` because it survives a frame replacing a picture:
    /// an animator sends frame after frame under one key, and each of them is a
    /// `Made` whose `moves` says the same thing.
    moving: HashSet<String>,
    bytes: usize,
}

impl Kept {
    /// Whether this key has been answered or asked about already.
    #[must_use]
    pub fn knows(&self, key: &str) -> bool {
        self.made.contains_key(key) || self.asked.contains(key)
    }

    /// The picture for a key, when there is one.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&ronoctua_render::image::Rgba> {
        self.made.get(key)?.as_ref()
    }

    /// Writes down that this key has been asked about.
    pub fn asking(&mut self, key: String) {
        self.asked.insert(key);
    }

    /// Takes an answer.
    ///
    /// A key nobody asked about is still kept: the ask may have been forgotten
    /// by a sweep between the question and the answer, and throwing the picture
    /// away would mean asking for it again.
    pub fn took(&mut self, made: Made) {
        self.asked.remove(&made.key);
        if made.moves {
            self.moving.insert(made.key.clone());
        }
        if let Some(seconds) = made.seconds {
            self.films.insert(made.key.clone(), seconds);
        }
        if let Some(inside) = made.inside {
            self.carried.insert(made.key.clone(), inside);
        }
        self.bytes += made.rgba.as_ref().map_or(0, |r| r.pixels.len());
        if let Some(old) = self.made.insert(made.key, made.rgba) {
            self.bytes = self
                .bytes
                .saturating_sub(old.as_ref().map_or(0, |r| r.pixels.len()));
        }
    }

    /// How long this key's film runs, when it is one.
    #[must_use]
    pub fn film(&self, key: &str) -> Option<f64> {
        self.films.get(key).copied()
    }

    /// Where this key's photograph keeps its film, when it is a motion photo.
    #[must_use]
    pub fn carries(&self, key: &str) -> Option<&ronoctua_inspect::motion::Inside> {
        self.carried.get(key)
    }

    /// Whether this key's file moves by itself.
    ///
    /// What the door asks before uploading: a still picture goes up once, and a
    /// moving one replaces its texture under the SAME key every frame — which is
    /// what keeps an animation to one slot of the renderer's ceiling instead of
    /// filling the cache with one file's film.
    #[must_use]
    pub fn moves(&self, key: &str) -> bool {
        self.moving.contains(key)
    }

    /// Every key this cache holds a picture for.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        self.made
            .iter()
            .filter(|(_, v)| v.is_some())
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// How many bytes of decoded picture are being held.
    ///
    /// Only the tests read it, and they have to: the ceiling is the one promise
    /// here that nothing on screen would show being broken — the same reason
    /// `files::View::header_shown` exists.
    #[cfg(test)]
    #[must_use]
    pub const fn weight(&self) -> usize {
        self.bytes
    }

    /// Lets go of everything that is not on screen, but only over the ceiling.
    ///
    /// # Why the policy is this coarse
    ///
    /// It is `ronoctua_render::image::Images::dump`'s, deliberately, and that
    /// one's own comment makes the argument: a true LRU needs a clock and a
    /// sort per frame to save a transfer that is already cheap. Here it is
    /// cheaper still — what a dropped thumbnail costs is a re-read of a file
    /// the disk cache has already made, not a decode.
    ///
    /// And it acts only ABOVE the ceiling, so scrolling inside a folder that
    /// fits in memory never drops anything at all.
    pub fn keep_only(&mut self, alive: &HashSet<String>) {
        let ceiling = usize::from(ronoctua_tokens::space::FILES_THUMBMEMORY) * 1024 * 1024;
        if self.bytes <= ceiling {
            return;
        }
        self.made.retain(|k, _| alive.contains(k));
        self.moving.retain(|k| alive.contains(k));
        self.films.retain(|k, _| alive.contains(k));
        self.carried.retain(|k, _| alive.contains(k));
        self.bytes = self
            .made
            .values()
            .map(|r| r.as_ref().map_or(0, |r| r.pixels.len()))
            .sum();
    }

    // AND THERE IS NO `clear`, which is the decision rather than an omission: a
    // key carries the whole path, so another directory's thumbnails are other
    // keys and cost nothing to hold — and going up and coming back is the
    // commonest gesture there is. What bounds the memory is [`Kept::keep_only`],
    // which acts on the ceiling and not on a gesture.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_picture(side: u32) -> ronoctua_render::image::Rgba {
        ronoctua_render::image::Rgba {
            width: side,
            height: side,
            pixels: vec![0; (side * side * 4) as usize],
        }
    }

    /// A REFUSAL IS AN ANSWER. Without this the grid asks again every frame,
    /// which for a folder of documents is a child process per cell per frame.
    #[test]
    fn a_file_with_no_picture_is_answered_once() {
        let mut kept = Kept::default();
        assert!(!kept.knows("doc"));
        kept.asking("doc".to_owned());
        assert!(kept.knows("doc"), "an ask in flight is not a second ask");
        kept.took(Made {
            key: "doc".to_owned(),
            rgba: None,
            seconds: None,
            inside: None,
            moves: false,
        });
        assert!(kept.knows("doc"), "the refusal was not written down");
        assert!(kept.get("doc").is_none());
    }

    /// AND THE WEIGHT IS WHAT IS HELD, not what has ever been held.
    #[test]
    fn replacing_a_picture_does_not_count_it_twice() {
        let mut kept = Kept::default();
        kept.took(Made {
            key: "a".to_owned(),
            rgba: Some(a_picture(64)),
            seconds: None,
            inside: None,
            moves: false,
        });
        let once = kept.weight();
        assert!(once > 0);
        kept.took(Made {
            key: "a".to_owned(),
            rgba: Some(a_picture(64)),
            seconds: None,
            inside: None,
            moves: false,
        });
        assert_eq!(kept.weight(), once, "the same key was counted twice");
    }

    /// AND NOTHING IS LET GO OF UNDER THE CEILING, which is what makes
    /// scrolling inside an ordinary folder free.
    #[test]
    fn what_fits_is_never_dropped() {
        let mut kept = Kept::default();
        kept.took(Made {
            key: "a".to_owned(),
            rgba: Some(a_picture(64)),
            seconds: None,
            inside: None,
            moves: false,
        });
        kept.keep_only(&HashSet::new());
        assert!(kept.get("a").is_some(), "a picture that fits was dropped");
    }

    /// OVER THE CEILING, WHAT IS ON SCREEN STAYS AND THE REST GOES.
    #[test]
    fn over_the_ceiling_only_what_is_drawn_is_kept() {
        let mut kept = Kept::default();
        let ceiling = usize::from(ronoctua_tokens::space::FILES_THUMBMEMORY) * 1024 * 1024;
        // A picture big enough that a handful of them pass the ceiling, so the
        // test does not depend on the token's value.
        let side = 1024;
        let each = (side * side * 4) as usize;
        let many = ceiling / each + 2;
        for n in 0..many {
            kept.took(Made {
                key: format!("k{n}"),
                rgba: Some(a_picture(side)),
                seconds: None,
                inside: None,
                moves: false,
            });
        }
        assert!(kept.weight() > ceiling, "the ceiling was never passed");
        let alive: HashSet<String> = ["k0".to_owned()].into_iter().collect();
        kept.keep_only(&alive);
        assert!(kept.get("k0").is_some(), "what is on screen was dropped");
        assert!(kept.get("k1").is_none(), "what is off screen was kept");
        assert_eq!(kept.weight(), each, "the weight did not follow the drop");
    }

    /// TWO SIZES ARE TWO KEYS, AND TWO THEMES ARE TWO KEYS.
    ///
    /// The other half of `one-text-size`'s rule, and its own comment says why a
    /// check cannot be it: *"a check cannot tell a key that carries a number
    /// from one that names a variable, and a test can"*. What it stops is the
    /// defect the owner watched in the launcher — a texture rasterised at 18 px
    /// drawn into a 36 px box for the life of the door, because the cache could
    /// not tell the two apart.
    #[test]
    fn two_sizes_and_two_themes_are_four_keys() {
        let at = |target| ronoctua_inspect::Stamp {
            path: std::path::PathBuf::from("/tmp/a.jpg"),
            mtime: 1,
            bytes: 2,
            target,
        };
        let keys = [
            thumb_key(&at(128), "0"),
            thumb_key(&at(256), "0"),
            thumb_key(&at(128), "1"),
            thumb_key(&at(256), "1"),
        ];
        let mut seen: HashSet<&str> = HashSet::new();
        for k in &keys {
            assert!(seen.insert(k), "two of these are one key: {keys:?}");
        }
        // AND THE SAME FILE AT THE SAME SIZE IN THE SAME THEME IS ONE KEY,
        // which is what makes the cache a cache.
        assert_eq!(thumb_key(&at(128), "0"), keys[0]);
    }

    /// AND THERE IS ALWAYS AT LEAST ONE HAND, and never the whole machine.
    #[test]
    fn the_pool_is_half_the_machine_and_bounded() {
        let n = hands();
        assert!((2..=8).contains(&n), "{n} hands");
    }
}
