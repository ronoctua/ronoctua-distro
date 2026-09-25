//! The hands in the camera, for whoever reads them ([`crate::wire::Ask::Hands`]).
//!
//! # Seen here, understood elsewhere
//!
//! This thread answers WHERE the hands are — 21 points each, and which hand —
//! and nothing about what they mean. What a pose means (a click, a scroll, a
//! command) is `ronoctua-gestures`, run by the footer, which is the process
//! that can act on it: this daemon owns the camera and no Wayland pointer.
//!
//! # The newest frame, at a rate of its own
//!
//! The mould of the segmentation thread: a frame read while the models ran is
//! never queued, the next read takes the newest. [`RATE`] caps how often the
//! models run at all — a hand moves smoothly at twelve readings a second, and
//! every reading above that is CPU a person working beside it pays for.
//!
//! # And slower with nobody's hands in the picture
//!
//! With no hand held the only work is the palm search, and nobody is making a
//! gesture: [`IDLE_RATE`] is enough to see a hand come in. Measured on the
//! machine it was written on, reading two hands costs about a core at fifteen
//! a second, and an empty picture a tenth of that.

use std::{
    io::Write as _,
    sync::Arc,
    time::{Duration, Instant},
};

use ronoctua_segment::Hands;

use crate::{
    engine::Shared,
    wire::{Say, Seen, SeenHand},
};

/// Readings a second, at most, while a hand is in the picture.
pub const RATE: u32 = 12;
/// Readings a second while none is.
pub const IDLE_RATE: u32 = 5;

/// The directory both models live in (`nix/hand-model.nix`).
const MODELS: &str = "RONOCTUA_HAND_MODELS";

/// The two models' bytes — the palm detector and the landmark model — or
/// `None` when they are not installed, which is a refusal and not an error.
pub fn models() -> Option<Arc<(Vec<u8>, Vec<u8>)>> {
    let dir = std::path::PathBuf::from(std::env::var_os(MODELS)?);
    let detector = std::fs::read(dir.join("hand_detector.tflite")).ok()?;
    let landmarks = std::fs::read(dir.join("hand_landmarks_detector.tflite")).ok()?;
    Some(Arc::new((detector, landmarks)))
}

/// Starts the thread; it ends by itself when the last reader goes.
pub fn start(shared: Shared, models: Arc<(Vec<u8>, Vec<u8>)>) {
    let spawned = std::thread::Builder::new()
        .name("studio-hands".into())
        .spawn(move || run(&shared, &models));
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "the hands thread did not start");
    }
}

fn run(shared: &Shared, models: &(Vec<u8>, Vec<u8>)) {
    let (lock, bell) = &**shared;
    let mut hands = match Hands::new(&models.0, &models.1) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(error = %e, "the hand models were refused");
            if let Ok(mut e) = lock.lock() {
                e.hand_readers.clear();
                e.hands_running = false;
            }
            return;
        }
    };
    let began = Instant::now();
    let mut last: Option<Arc<ronoctua_effects::Frame>> = None;
    loop {
        let started = Instant::now();
        let got = {
            let Ok(mut e) = lock.lock() else { return };
            if e.hand_readers.is_empty() {
                e.hands_running = false;
                drop(e);
                // The loop closes the camera.
                bell.notify_all();
                return;
            }
            e.hands_frame()
        };
        let Some((frame, mirrored)) =
            got.filter(|(f, _)| !last.as_ref().is_some_and(|l| Arc::ptr_eq(l, f)))
        else {
            // No frame yet, or the same one: the camera is slower than us.
            if last.is_none() {
                hands.forget();
            }
            std::thread::sleep(Duration::from_millis(10));
            continue;
        };
        let seen = hands.see(&frame.rgba, frame.width, frame.height, mirrored);
        let pace = Duration::from_secs(1) / if seen.is_empty() { IDLE_RATE } else { RATE };
        let say = Say::Hands(Seen {
            aspect: frame.width as f32 / frame.height.max(1) as f32,
            at: began.elapsed().as_millis() as u64,
            hands: seen
                .into_iter()
                .map(|h| SeenHand {
                    points: h.points.to_vec(),
                    right: h.right,
                })
                .collect(),
        });
        last = Some(frame);
        let mut line = serde_json::to_vec(&say).unwrap_or_default();
        line.push(b'\n');
        {
            let Ok(mut e) = lock.lock() else { return };
            let before = e.hand_readers.len();
            e.hand_readers.retain_mut(|r| r.write_all(&line).is_ok());
            if e.hand_readers.len() != before {
                bell.notify_all();
            }
        }
        let spent = started.elapsed();
        if spent < pace {
            std::thread::sleep(pace - spent);
        }
    }
}
