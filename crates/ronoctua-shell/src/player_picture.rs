//! One picture of the player, and the thread that brings its frames — shared
//! by every door that shows one: the window, the column and the footer.
//!
//! # The frames are the clock
//!
//! Each picture has a thread of its own reading the player's ring, and every
//! frame that arrives is what asks for the next paint — no timer. The newest
//! frame wins: one slot, so a door that was behind never plays a backlog.
//!
//! # The size is the door's to say
//!
//! The thread asks the daemon for frames at the size the door last said
//! ([`Picture::want`]), in PHYSICAL pixels, and asks again when it changes; at
//! zero it waits. A door that drew the picture in a band of its own and a door
//! that drew it in a panel beside its rows ask the same thing.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use smithay_client_toolkit::reexports::calloop::channel;

/// The newest frame of one picture, and nothing queued.
type Slot = Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>;

/// One picture: the size its thread should ask for, and what it received.
pub struct Picture {
    /// Which video: `None` the main one, `Some(i)` the i-th follower.
    pub of: Option<usize>,
    wanted: Arc<Mutex<(u32, u32)>>,
    slot: Slot,
    alive: Arc<AtomicBool>,
    /// The newest frame taken from the thread.
    pub frame: Option<ronoctua_render::image::Rgba>,
    /// Whether that frame is already on the GPU, under [`Picture::key`].
    pub uploaded: bool,
}

impl Picture {
    /// The texture's key: the picture, whose content moves every frame.
    #[must_use]
    pub fn key(&self) -> String {
        format!("player:picture:{}", self.of.map_or(0, |i| i + 1))
    }

    /// A picture and its thread, which pings `k` down `ping` on every frame.
    #[must_use]
    pub fn start(of: Option<usize>, k: usize, ping: channel::Sender<usize>) -> Self {
        let wanted: Arc<Mutex<(u32, u32)>> = Arc::default();
        let slot: Slot = Arc::default();
        let alive = Arc::new(AtomicBool::new(true));
        let (w2, s2, a2) = (wanted.clone(), slot.clone(), alive.clone());
        let _ = std::thread::Builder::new()
            .name("player-picture".into())
            .spawn(move || {
                let mut buf = Vec::new();
                while a2.load(Ordering::SeqCst) {
                    let size = w2.lock().map_or((0, 0), |g| *g);
                    if size.0 == 0 || size.1 == 0 {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        continue;
                    }
                    let mut feed = match ronoctua_player::client::preview(size.0, size.1, of) {
                        Ok(f) => f,
                        Err(e) => {
                            tracing::debug!(error = %e, "no preview yet");
                            std::thread::sleep(std::time::Duration::from_secs(1));
                            continue;
                        }
                    };
                    // PATIENCE, so a paused film still notices the door
                    // changed size: no frame comes, and the thread has to
                    // look up anyway.
                    let _ = feed.patience(Some(std::time::Duration::from_millis(250)));
                    tracing::debug!(of = ?of, width = feed.width, height = feed.height, "a preview");
                    loop {
                        if !a2.load(Ordering::SeqCst) {
                            return;
                        }
                        if w2.lock().map_or(size, |g| *g) != size {
                            break;
                        }
                        match feed.next(&mut buf) {
                            Ok(true) => {
                                if let Ok(mut s) = s2.lock() {
                                    *s = Some((feed.width, feed.height, buf.clone()));
                                }
                                if ping.send(k).is_err() {
                                    return;
                                }
                            }
                            Ok(false) => {}
                            Err(e) => {
                                tracing::debug!(error = %e, "the preview ended");
                                break;
                            }
                        }
                    }
                }
            });
        Self {
            of,
            wanted,
            slot,
            alive,
            frame: None,
            uploaded: false,
        }
    }

    /// The size the thread asks the daemon for, in physical pixels.
    pub fn want(&self, size: (u32, u32)) {
        if let Ok(mut w) = self.wanted.lock() {
            *w = size;
        }
    }

    /// Takes the newest frame the thread left, if there is one: `true` when
    /// the picture changed and has to go up to the GPU again.
    pub fn took_a_frame(&mut self) -> bool {
        let Some((w, h, px)) = self.slot.lock().ok().and_then(|mut s| s.take()) else {
            return false;
        };
        self.frame = ronoctua_render::image::Rgba::new(w, h, px);
        tracing::trace!(w, h, taken = self.frame.is_some(), "a frame");
        self.uploaded = false;
        true
    }
}

/// A picture that goes takes its thread with it.
impl Drop for Picture {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
    }
}
