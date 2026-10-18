//! The thread that turns a shape and hands back one picture at a time.
//!
//! # Why this is `motion.rs` again, and not a fourth spelling of it
//!
//! Because the shape of the problem is the same one the film already solved,
//! and the reasons written there hold word for word here: **the frame arriving
//! IS the clock** (neither door has a timer and neither gained one), only the
//! CURRENT frame is kept rather than the file, and the manager decides what
//! SHOULD be on screen while the difference from what IS on screen is the
//! order. What differs is only where a frame comes from — an ffmpeg pipe there,
//! [`ronoctua_inspect::solid::draw`] here — so the parts that differ are the
//! parts that are written out, and the rest is deliberately the same shape so
//! that reading one teaches the other.
//!
//! # Why the body lives HERE and not in the answer
//!
//! Because it is large and the answer is small. The 3DBenchy's 225 154
//! triangles are eight megabytes of positions, and `inspection::Looked` is `Box`ed,
//! cloned and compared by the door — putting a mesh through it would move eight
//! megabytes down a channel to be compared field by field. So this thread reads
//! the file once, keeps what it read for as long as the cursor stays on it, and
//! a turn is then only a redraw: measured at 41 ms for that file and 14 ms for
//! an ordinary one, which is what makes dragging it feel like dragging a thing
//! rather than like waiting.
//!
//! Keeping it also means the ONE thing a pose change must not do is re-read the
//! file, and `Turn::same_file` is where that is decided.
//!
//! What is held is a [`Body`] and not a mesh: two of the eleven formats hold
//! no triangle at all — a STEP and an IGES are boundary representations and
//! come back as edges — and the difference lives inside the rasteriser, not
//! here.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ronoctua_inspect::mesh::Body;
use ronoctua_inspect::solid::{Paint, Pose};

/// One drawn view of a shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Which file this belongs to, as `Stamp::key` spells it.
    ///
    /// It travels because the cursor moves while a thread draws, and a picture
    /// of one file under another file's name is the one failure a preview must
    /// not have — the same reason `motion::Frame` carries one.
    pub key: String,
    /// The pixels, straight alpha, with nothing behind the shape.
    pub rgba: ronoctua_render::image::Rgba,
}

/// What comes back from the thread.
#[derive(Debug, Clone, PartialEq)]
pub enum Beat {
    /// A view of the shape.
    Drawn(Box<Frame>),
    /// The file could not be read as a shape, and this is why.
    ///
    /// A refusal is a VALUE here for the reason the whole preview says so: an
    /// empty panel and a refused panel look identical, and only one of them
    /// has a sentence.
    Trouble {
        /// Which file.
        key: String,
        /// What went wrong.
        why: ronoctua_inspect::Refusal,
    },
}

/// What the manager wants on screen.
///
/// A value, compared as one: the door builds what it wants and the difference
/// from what it last asked for IS the order. `motion::Reel` is the same idea
/// and for the same reason.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    /// The file.
    pub path: std::path::PathBuf,
    /// Its type, already through `settled`.
    pub mime: &'static str,
    /// Its key, which comes back on the frame.
    pub key: String,
    /// How wide the picture should be.
    pub width: u32,
    /// How tall.
    pub height: u32,
    /// From where.
    pub pose: Pose,
    /// What it is drawn with.
    pub paint: Paint,
}

impl Turn {
    /// Whether the two are about the same file, so a mesh already read can be
    /// drawn again instead of read again.
    ///
    /// The KEY and not the path: the key carries the modification time and the
    /// size, so a file rewritten under the cursor is correctly a different
    /// shape rather than the old one drawn at a new angle.
    #[must_use]
    pub fn same_file(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

/// What the door tells the thread.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// Draw this.
    Show(Box<Turn>),
    /// Nothing is being looked at; let the mesh go.
    Stop,
}

/// The door's handle on the thread.
pub struct Stage {
    asks: std::sync::mpsc::Sender<Ask>,
    /// Which request is current.
    ///
    /// Read INSIDE the drawing loop, so a pose the person has already dragged
    /// past is abandoned part-drawn rather than finished and thrown away. On a
    /// heavy shape that is the difference between a drag that keeps up and one
    /// that runs a frame behind for as long as the hand is moving.
    era: Arc<AtomicU64>,
    /// The last era handed out.
    latest: u64,
}

impl Stage {
    /// Passes one order on.
    pub fn tell(&mut self, ask: Ask) {
        self.latest = self.latest.wrapping_add(1);
        self.era.store(self.latest, Ordering::SeqCst);
        let _ = self.asks.send(ask);
    }
}

/// Starts the thread.
///
/// `out` is a channel of its own and not the door's `Report` one, for the
/// reason the film's frames already have theirs: a drag emits a frame every
/// few tens of milliseconds, and putting those through the channel that also
/// carries "the directory changed" would wake everything that reads a
/// directory, dozens of times a second, for a picture.
///
/// `None` when the thread could not be created — a machine in trouble, and the
/// panel then simply stays empty for a mesh.
#[must_use]
pub fn stage(out: impl Into<crate::wake::Out<Beat>>) -> Option<Stage> {
    let out = out.into();
    let (asks, orders) = std::sync::mpsc::channel::<Ask>();
    let era = Arc::new(AtomicU64::new(0));
    let mine = Arc::clone(&era);
    let born = std::thread::Builder::new()
        .name("ronoctua-stage".to_owned())
        .spawn(move || {
            // WHAT WAS LAST READ, kept for as long as the cursor stays on it.
            let mut held: Option<(Turn, Body)> = None;
            while let Ok(order) = orders.recv() {
                // ONLY THE NEWEST. A drag queues an order per pointer event and
                // every one behind the last is an angle nobody is looking at
                // any more — the same drain the player and the inspector do.
                let mut order = order;
                while let Ok(newer) = orders.try_recv() {
                    order = newer;
                }
                let Ask::Show(turn) = order else {
                    held = None;
                    continue;
                };
                let now = mine.load(Ordering::SeqCst);

                // READ ONLY WHEN IT IS A DIFFERENT FILE. This is the line that
                // makes turning cheap.
                if !held.as_ref().is_some_and(|(t, _)| t.same_file(&turn)) {
                    match ronoctua_inspect::mesh::read(&turn.path, turn.mime) {
                        Ok(read) => held = Some(((*turn).clone(), read.body)),
                        Err(why) => {
                            held = None;
                            let _ = out.send(Beat::Trouble {
                                key: turn.key.clone(),
                                why,
                            });
                            continue;
                        }
                    }
                }
                let Some((_, body)) = held.as_ref() else {
                    continue;
                };
                // ASKED AGAIN AFTER THE READ. A big file takes a tenth of a
                // second to read, which is long enough for the cursor to have
                // moved on, and drawing it then would cost a frame nobody
                // wants and put it on screen under the wrong name.
                if mine.load(Ordering::SeqCst) != now {
                    continue;
                }
                let rgba = ronoctua_inspect::solid::draw(
                    body,
                    turn.pose,
                    turn.width,
                    turn.height,
                    turn.paint,
                );
                if mine.load(Ordering::SeqCst) != now {
                    continue;
                }
                if out
                    .send(Beat::Drawn(Box::new(Frame {
                        key: turn.key.clone(),
                        rgba: ronoctua_render::image::Rgba {
                            width: rgba.width,
                            height: rgba.height,
                            pixels: rgba.pixels,
                        },
                    })))
                    .is_err()
                {
                    // The door left.
                    return;
                }
            }
        });
    match born {
        Ok(_) => Some(Stage {
            asks,
            era,
            latest: 0,
        }),
        Err(e) => {
            tracing::error!(error = %e, "could not create the stage thread");
            None
        }
    }
}
