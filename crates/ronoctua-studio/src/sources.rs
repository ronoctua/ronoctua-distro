//! Where frames come from: one thread per source, the latest frame shared.
//!
//! # A source is a value, and dropping it closes the device
//!
//! [`Live`] owns the thread; its `Drop` asks the thread to stop and joins it,
//! and the thread's own values (the camera's [`Capture`], the media's
//! ffmpeg) close on the way out. The engine keeps exactly the sources the
//! scene on air needs, and only while somebody consumes the program — a
//! studio nobody is watching has no source, and so no camera light.
//!
//! # Effects run HERE, per source
//!
//! Each source thread applies its chain before publishing, so the effects of
//! two cameras run on two cores while the compositor places the last frames.
//! The chain is the one the engine sets on the source (`Shared::chain`): the
//! first visible layer that shows the source decides it. Two layers showing
//! the SAME camera with DIFFERENT effects therefore show the first's — a
//! camera can be opened once.
//!
//! # And a SECOND chain, for the virtual camera
//!
//! What a call sees is not always what the preview shows: a person may want
//! the background blurred for the call and the plain picture on screen. A
//! camera source runs a second chain (`Shared::own`) on a copy of the same
//! captured frame and publishes it beside the first (`Shared::own_frame`) —
//! one capture, two pictures, and the second costs nothing unless the virtual
//! camera sends the camera with an effect of its own
//! (`prefs::VirtualEffect`).

use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use ronoctua_camera::{capture::Capture, frames::Filled};
use ronoctua_effects::{Chain, Context, Frame, Refusal, Scratch};
use ronoctua_segment::{Mask, Segmenter, Smoother};

use crate::{
    compose,
    scene::Source,
    screen::{self, Target},
    wire::Trouble,
};

/// What a source thread publishes and what the engine sets on it.
#[derive(Default)]
pub struct Shared {
    /// The newest frame.
    pub frame: Mutex<Option<Arc<Frame>>>,
    /// Why there is none, when there is none.
    pub trouble: Mutex<Option<Trouble>>,
    /// The effect chain to run.
    pub chain: Mutex<Chain>,
    /// Bumped whenever the chain changes, so a still picture re-renders.
    pub chain_era: AtomicU64,
    /// Frames published, for the rate the state reports.
    pub published: AtomicU64,
    /// The camera is shown as a mirror — the hands read it as the person sees
    /// it either way.
    pub mirrored: AtomicBool,
    /// The virtual camera's own chain, when it has one: run on a copy of the
    /// same captured frame. `None` is the first chain's picture.
    pub own: Mutex<Option<Chain>>,
    /// The newest frame through that chain.
    pub own_frame: Mutex<Option<Arc<Frame>>>,
    stop: AtomicBool,
}

impl Shared {
    fn publish(&self, frame: Frame) {
        if let Ok(mut slot) = self.frame.lock() {
            *slot = Some(Arc::new(frame));
        }
        self.published.fetch_add(1, Ordering::Relaxed);
        self.say(None);
    }

    fn say(&self, trouble: Option<Trouble>) {
        if let Ok(mut t) = self.trouble.lock() {
            *t = trouble;
        }
    }

    fn going(&self) -> bool {
        !self.stop.load(Ordering::SeqCst)
    }

    /// Sleeps in ticks, waking early on stop.
    fn rest(&self, total: Duration) {
        let step = Duration::from_millis(50);
        let mut left = total;
        while self.going() && !left.is_zero() {
            let now = left.min(step);
            std::thread::sleep(now);
            left -= now;
        }
    }

    /// Sets the virtual camera's own chain, or takes it away.
    pub fn set_own(&self, chain: Option<Chain>) {
        if let Ok(mut c) = self.own.lock()
            && *c != chain
        {
            if chain.is_none()
                && let Ok(mut f) = self.own_frame.lock()
            {
                *f = None;
            }
            *c = chain;
        }
    }

    /// The chain the virtual camera has of its own, when it has one.
    fn own_chain(&self) -> Option<Chain> {
        self.own.lock().ok().and_then(|c| c.clone())
    }

    /// Publishes a frame through the virtual camera's own chain.
    fn publish_own(&self, frame: Frame) {
        if let Ok(mut slot) = self.own_frame.lock() {
            *slot = Some(Arc::new(frame));
        }
    }

    /// Sets the chain; a no-op when it is the same one.
    pub fn set_chain(&self, chain: Chain) {
        if let Ok(mut c) = self.chain.lock()
            && *c != chain
        {
            *c = chain;
            self.chain_era.fetch_add(1, Ordering::SeqCst);
        }
    }
}

/// A running source.
pub struct Live {
    /// What it publishes.
    pub shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Live {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// What every source needs to know about the program.
#[derive(Clone)]
pub struct Setup {
    /// The program's size: no source publishes larger.
    pub program: (u32, u32),
    /// The program's rate.
    pub fps: u32,
    /// Draw the pointer in screen captures.
    pub cursor: bool,
    /// The replaced background's picture.
    pub backdrop: Option<PathBuf>,
    /// The segmentation model's bytes, when installed.
    pub model: Option<Arc<Vec<u8>>>,
}

/// Starts a source.
pub fn start(source: Source, setup: Setup) -> Live {
    let shared = Arc::new(Shared::default());
    let mine = shared.clone();
    let thread = std::thread::Builder::new()
        .name("studio-source".into())
        .spawn(move || match source {
            Source::Camera { identity } => camera(&mine, identity.as_deref(), &setup),
            Source::Screen { output } => screen_source(&mine, &Target::Output(output), &setup),
            Source::Window { matching } => screen_source(&mine, &Target::Window(matching), &setup),
            Source::Image { path } => still(&mine, &setup, || picture(&path, setup.program)),
            Source::Colour { rgba } => still(&mine, &setup, || colour(&rgba)),
            Source::Text { text, rgba } => words(&mine, &setup, &text, &rgba),
            Source::Media { path } => media(&mine, &path, &setup),
        })
        .ok();
    Live { shared, thread }
}

/// Runs the chain on a frame, with a mask and a backdrop when it needs them,
/// and turns the refusals into the one trouble worth saying.
struct Effects {
    scratch: Scratch,
    alpha: Vec<u8>,
    backdrop: Option<(PathBuf, Frame)>,
    segment: Option<Segmentation>,
}

impl Effects {
    fn new() -> Self {
        Self {
            scratch: Scratch::default(),
            alpha: Vec::new(),
            backdrop: None,
            segment: None,
        }
    }

    fn run(&mut self, frame: &mut Frame, shared: &Shared, setup: &Setup) -> Option<Trouble> {
        let chain = shared.chain.lock().map(|c| c.clone()).unwrap_or_default();
        self.run_chain(frame, &chain, setup)
    }

    /// The same, for a chain the caller holds — the virtual camera's own.
    fn run_chain(&mut self, frame: &mut Frame, chain: &Chain, setup: &Setup) -> Option<Trouble> {
        let chain = chain.clone();
        if chain.0.is_empty() {
            self.segment = None;
            return None;
        }
        let mut masked = false;
        if chain.needs_mask() {
            let Some(model) = setup.model.clone() else {
                // The rest of the chain still runs; the masked steps refuse.
                let _ = ronoctua_effects::apply_chain(
                    &chain,
                    frame,
                    &Context::default(),
                    &mut self.scratch,
                );
                return Some(Trouble::NoModel);
            };
            let seg = self
                .segment
                .get_or_insert_with(|| Segmentation::start(model));
            seg.offer(frame, chain.steadiness());
            if let Some(m) = seg.latest() {
                m.alpha(frame.width, frame.height, &mut self.alpha);
                masked = true;
            }
        } else {
            self.segment = None;
        }
        if chain.0.iter().any(|s| s.id == "background-replace") {
            self.load_backdrop(frame.width, frame.height, setup);
        }
        let backdrop = self
            .backdrop
            .as_ref()
            .map(|(_, f)| f)
            .filter(|f| f.width == frame.width && f.height == frame.height);
        let mask = masked.then_some(self.alpha.as_slice());
        let ctx = Context { mask, backdrop };
        let refused = ronoctua_effects::apply_chain(&chain, frame, &ctx, &mut self.scratch);
        refused.iter().find_map(|(_, r)| match r {
            Refusal::NoBackdrop => Some(Trouble::NoBackdrop),
            _ => None,
        })
    }

    fn load_backdrop(&mut self, w: usize, h: usize, setup: &Setup) {
        let Some(path) = setup.backdrop.clone() else {
            self.backdrop = None;
            return;
        };
        let stale = self
            .backdrop
            .as_ref()
            .is_none_or(|(p, f)| *p != path || f.width != w || f.height != h);
        if stale {
            let Some(img) = image::open(&path).ok().map(|i| i.to_rgba8()) else {
                self.backdrop = None;
                return;
            };
            let src = Frame {
                width: img.width() as usize,
                height: img.height() as usize,
                rgba: img.into_raw(),
            };
            // Fill, cropped: a backdrop has no bars.
            let mut out = Frame::default();
            compose::compose(
                &mut out,
                w as u32,
                h as u32,
                &[compose::Placed {
                    frame: &src,
                    rect: crate::scene::Rect::WHOLE,
                    fit: crate::scene::Fit::Fill,
                    opacity: 100,
                    crop: [0.0; 4],
                }],
            );
            self.backdrop = Some((path, out));
        }
    }
}

/// The segmentation thread's input: the newest frame and its steadiness,
/// and the bell that wakes the thread.
type Offered = Arc<(Mutex<Option<(Frame, f32)>>, Condvar)>;

/// The model on a thread of its own, fed the newest frame and read for the
/// newest mask — a mask a frame or two old is invisible, and a camera that
/// waited 25 ms for every mask would drop to 20 frames a second.
struct Segmentation {
    input: Offered,
    output: Arc<Mutex<Option<Arc<Mask>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Segmentation {
    fn start(model: Arc<Vec<u8>>) -> Self {
        let input: Offered = Arc::default();
        let output: Arc<Mutex<Option<Arc<Mask>>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (i, o, s) = (input.clone(), output.clone(), stop.clone());
        let thread = std::thread::Builder::new()
            .name("studio-segment".into())
            .spawn(move || {
                let Ok(mut seg) = Segmenter::new(&model) else {
                    tracing::warn!("the segmentation model will not load");
                    return;
                };
                let mut smooth = Smoother::default();
                while !s.load(Ordering::SeqCst) {
                    let job = {
                        let (lock, cv) = &*i;
                        let Ok(guard) = lock.lock() else { return };
                        let Ok((mut guard, _)) = cv.wait_timeout(guard, Duration::from_millis(200))
                        else {
                            return;
                        };
                        guard.take()
                    };
                    let Some((frame, keep)) = job else { continue };
                    let fresh = seg.mask(&frame.rgba, frame.width, frame.height);
                    let mask = smooth.take(fresh, keep).clone();
                    if let Ok(mut out) = o.lock() {
                        *out = Some(Arc::new(mask));
                    }
                }
            })
            .ok();
        Self {
            input,
            output,
            stop,
            thread,
        }
    }

    fn offer(&self, frame: &Frame, keep: f32) {
        let (lock, cv) = &*self.input;
        if let Ok(mut slot) = lock.lock() {
            // A small copy: the model sees 256×256, so a quarter-size frame
            // loses nothing it would have used.
            let (w, h) = compose::fit_within(frame.width as u32, frame.height as u32, 512, 512);
            *slot = Some((compose::resize(frame, w, h), keep));
            cv.notify_one();
        }
    }

    fn latest(&self) -> Option<Arc<Mask>> {
        self.output.lock().ok()?.clone()
    }
}

impl Drop for Segmentation {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.input.1.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Who holds a camera node, in words — the busy sentence names the program.
fn holder_of(node: &std::path::Path) -> Option<String> {
    let node = node.to_string_lossy();
    ronoctua_memory::camera::holders("/dev/video")
        .into_iter()
        .find(|h| h.node == node && h.pid != std::process::id())
        .map(|h| h.name)
}

/// The camera reader's hand-over: the newest frame, whether it is unread, and
/// how the reading ended once it has.
#[derive(Default)]
struct Slot {
    frame: Vec<u8>,
    fresh: bool,
    end: Option<Filled>,
}

type Newest = Arc<(Mutex<Slot>, Condvar)>;

fn camera(shared: &Shared, identity: Option<&str>, setup: &Setup) {
    let mut effects = Effects::new();
    // THE SECOND CHAIN'S OWN SCRATCH AND SEGMENTER: its mask and its backdrop
    // are its own, and sharing them would hand one chain the other's.
    let mut own_effects = Effects::new();
    let mut buf = Vec::new();
    while shared.going() {
        let cameras = ronoctua_camera::scan();
        let prefs = ronoctua_camera::prefs::store().read();
        let chosen = identity
            .map(|i| ronoctua_camera::Identity(i.to_owned()))
            .or_else(|| prefs.default.clone());
        let Some(cam) = identity
            .and_then(|i| cameras.iter().find(|c| c.identity.0 == i && c.streamable))
            .or_else(|| ronoctua_camera::default_camera(&cameras, chosen.as_ref()))
            .cloned()
        else {
            shared.say(Some(Trouble::NoCamera));
            shared.rest(Duration::from_secs(2));
            continue;
        };
        let per = prefs.of(&cam.identity);
        let Some(mode) = ronoctua_camera::choose_mode(&cam, per.mode.as_ref()) else {
            shared.say(Some(Trouble::NoCamera));
            shared.rest(Duration::from_secs(2));
            continue;
        };
        let refused = ronoctua_camera::restore(&cam, &per.controls);
        if !refused.is_empty() {
            tracing::info!(?refused, "controls the camera would not take");
        }
        let (w, h) = compose::fit_within(mode.width, mode.height, setup.program.0, setup.program.1);
        shared.mirrored.store(per.mirror, Ordering::Relaxed);
        let mut capture = match Capture::open(&cam, &mode, w, h, per.mirror) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "could not open the camera");
                shared.say(Some(Trouble::NoFfmpeg));
                shared.rest(Duration::from_secs(3));
                continue;
            }
        };
        // THE CAMERA IS READ AT ITS OWN PACE, on a thread of its own, and only
        // the NEWEST frame is kept. Read here, between two effects, a slow
        // effect left the unread frames queued in ffmpeg, and the picture fell
        // one frame further behind for every frame the effect was late: a
        // background blur at 46 ms against a camera's 33 was seconds behind
        // within a minute. Taken from the slot, a slow effect costs frames
        // and never time.
        let ended = std::thread::scope(|scope| {
            let slot: Newest = Arc::default();
            let reader = slot.clone();
            let capture = &mut capture;
            scope.spawn(move || {
                let mut buf = Vec::new();
                let end = loop {
                    let got = capture.next(&mut buf, || shared.going());
                    if got != Filled::Whole {
                        break got;
                    }
                    let (lock, bell) = &*reader;
                    let Ok(mut s) = lock.lock() else { return };
                    // SWAPPED, not copied: the reader gets back the buffer the
                    // last frame was in, and nothing is allocated per frame.
                    std::mem::swap(&mut s.frame, &mut buf);
                    s.fresh = true;
                    bell.notify_one();
                };
                let (lock, bell) = &*reader;
                if let Ok(mut s) = lock.lock() {
                    s.end = Some(end);
                }
                bell.notify_one();
            });
            let mut first = true;
            loop {
                let got = {
                    let (lock, bell) = &*slot;
                    let Ok(mut s) = lock.lock() else {
                        return Filled::Left;
                    };
                    while !s.fresh && s.end.is_none() {
                        let Ok((next, _)) = bell.wait_timeout(s, Duration::from_millis(200)) else {
                            return Filled::Left;
                        };
                        s = next;
                    }
                    if s.fresh {
                        s.fresh = false;
                        std::mem::swap(&mut s.frame, &mut buf);
                        None
                    } else {
                        s.end
                    }
                };
                match got {
                    None => {
                        first = false;
                        let mut frame = Frame {
                            width: w as usize,
                            height: h as usize,
                            rgba: buf.clone(),
                        };
                        let trouble =
                            through_both(&mut frame, shared, setup, &mut effects, &mut own_effects);
                        shared.publish(frame);
                        if trouble.is_some() {
                            shared.say(trouble);
                        }
                    }
                    Some(Filled::Left) => return Filled::Left,
                    Some(end) => {
                        // A camera that gave nothing at all was taken by
                        // somebody; one that stopped mid-stream went quiet
                        // (unplugged, or the shutter's driver stopped it).
                        let why = if first {
                            holder_of(&cam.node)
                                .map_or(Trouble::CameraSilent, |by| Trouble::CameraBusy { by })
                        } else {
                            Trouble::CameraSilent
                        };
                        shared.say(Some(why));
                        if let Ok(mut f) = shared.frame.lock() {
                            *f = None;
                        }
                        return end;
                    }
                }
            }
        });
        if ended == Filled::Left {
            return;
        }
        drop(capture);
        shared.rest(Duration::from_secs(2));
    }
}

/// One captured frame through both chains: the virtual camera's own first,
/// on a copy of the capture before the first chain touches it, published
/// beside; then the first, in place.
///
/// Answers the FIRST trouble of either: the preview's chain, else the
/// virtual camera's own. A call that gets the plain picture because its
/// blur has no model is said in the camera's doors like the preview's —
/// thrown away, the settings would name an effect nobody was getting.
fn through_both(
    frame: &mut Frame,
    shared: &Shared,
    setup: &Setup,
    effects: &mut Effects,
    own_effects: &mut Effects,
) -> Option<Trouble> {
    let own_trouble = shared.own_chain().and_then(|own| {
        let mut copy = frame.clone();
        let trouble = own_effects.run_chain(&mut copy, &own, setup);
        shared.publish_own(copy);
        trouble
    });
    effects.run(frame, shared, setup).or(own_trouble)
}

fn screen_source(shared: &Shared, target: &Target, setup: &Setup) {
    let mut effects = Effects::new();
    while shared.going() {
        let mut publish = |mut frame: Frame| {
            let trouble = effects.run(&mut frame, shared, setup);
            shared.publish(frame);
            if trouble.is_some() {
                shared.say(trouble);
            }
        };
        let answer = screen::run(
            target,
            setup.program,
            setup.cursor,
            setup.fps,
            &|| shared.going(),
            &mut publish,
        );
        match answer {
            Ok(()) => {}
            Err(screen::Refused::NoScreen) => shared.say(Some(Trouble::NoScreen)),
            Err(screen::Refused::NoWindow) => shared.say(Some(Trouble::NoWindow)),
        }
        shared.rest(Duration::from_secs(2));
    }
}

/// A picture that does not move: made once, and again only when the chain
/// changes.
fn still(shared: &Shared, setup: &Setup, make: impl Fn() -> Option<Frame>) {
    let mut effects = Effects::new();
    let mut era = u64::MAX;
    let mut base: Option<Frame> = None;
    while shared.going() {
        if base.is_none() {
            base = make();
            if base.is_none() {
                shared.say(Some(Trouble::Write {
                    why: "unreadable".into(),
                }));
                shared.rest(Duration::from_secs(5));
                continue;
            }
            era = u64::MAX;
        }
        let now = shared.chain_era.load(Ordering::SeqCst);
        if now != era {
            era = now;
            if let Some(b) = &base {
                let mut frame = b.clone();
                let trouble = effects.run(&mut frame, shared, setup);
                shared.publish(frame);
                if trouble.is_some() {
                    shared.say(trouble);
                }
            }
        }
        shared.rest(Duration::from_millis(200));
    }
}

/// A text source: the words drawn once, and again when the chain changes —
/// or the trouble that says there is no face to draw them with.
fn words(shared: &Shared, setup: &Setup, text: &str, rgba: &str) {
    let colour = crate::words::rgba_of(rgba).unwrap_or([255, 255, 255, 255]);
    if !crate::words::has_a_face() {
        while shared.going() {
            shared.say(Some(Trouble::NoFace));
            shared.rest(Duration::from_secs(5));
        }
        return;
    }
    still(shared, setup, || {
        crate::words::drawn(text, colour, setup.program.1)
    });
}

fn picture(path: &str, program: (u32, u32)) -> Option<Frame> {
    let img = image::open(path).ok()?.to_rgba8();
    let src = Frame {
        width: img.width() as usize,
        height: img.height() as usize,
        rgba: img.into_raw(),
    };
    let (w, h) = compose::fit_within(src.width as u32, src.height as u32, program.0, program.1);
    Some(compose::resize(&src, w, h))
}

/// `RRGGBBAA` (or `RRGGBB`) to a small frame of that colour.
pub fn colour(hex: &str) -> Option<Frame> {
    let hex = hex.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    let rgba = [
        byte(0)?,
        byte(2)?,
        byte(4)?,
        if hex.len() >= 8 { byte(6)? } else { 255 },
    ];
    Some(Frame::filled(16, 16, rgba))
}

fn media(shared: &Shared, path: &str, setup: &Setup) {
    let mut effects = Effects::new();
    let (w, h) = setup.program;
    let mut buf = Vec::new();
    while shared.going() {
        let Ok(ffmpeg) = std::env::var(ronoctua_camera::capture::FFMPEG) else {
            shared.say(Some(Trouble::NoFfmpeg));
            shared.rest(Duration::from_secs(5));
            continue;
        };
        // Real time (`-re`), looped, and PADDED to the program's size with
        // transparency — so the frame's size is ours and no probe is needed.
        let filter = format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease,format=rgba,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black@0,fps={}",
            setup.fps
        );
        let child = std::process::Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-re",
                "-stream_loop",
                "-1",
                "-i",
                path,
                "-an",
                "-vf",
                &filter,
                "-pix_fmt",
                "rgba",
                "-f",
                "rawvideo",
                "-",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            shared.say(Some(Trouble::NoFfmpeg));
            shared.rest(Duration::from_secs(5));
            continue;
        };
        let Some(mut out) = child.stdout.take() else {
            continue;
        };
        let _guard = ronoctua_camera::frames::Reaped(child);
        buf.resize(w as usize * h as usize * 4, 0);
        loop {
            match ronoctua_camera::frames::fill(&mut out, &mut buf, Duration::from_secs(10), || {
                shared.going()
            }) {
                Filled::Whole => {
                    let mut frame = Frame {
                        width: w as usize,
                        height: h as usize,
                        rgba: buf.clone(),
                    };
                    let trouble = effects.run(&mut frame, shared, setup);
                    shared.publish(frame);
                    if trouble.is_some() {
                        shared.say(trouble);
                    }
                }
                Filled::Left => return,
                _ => break,
            }
        }
        shared.rest(Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_capture_goes_through_two_chains() {
        // THE PREVIEW PLAIN AND THE CALL NEGATIVE, out of one frame: the
        // virtual camera's chain runs on a copy, and the first chain's frame
        // is not touched by it.
        let shared = Shared::default();
        let setup = Setup {
            program: (2, 1),
            fps: 30,
            cursor: false,
            backdrop: None,
            model: None,
        };
        shared.set_own(Some(Chain(vec![
            ronoctua_effects::Step::new("negative").expect("declared"),
        ])));
        let mut frame = Frame {
            width: 2,
            height: 1,
            rgba: vec![10, 20, 30, 255, 200, 100, 0, 255],
        };
        let (mut a, mut b) = (Effects::new(), Effects::new());
        let _ = through_both(&mut frame, &shared, &setup, &mut a, &mut b);
        assert_eq!(
            frame.rgba,
            vec![10, 20, 30, 255, 200, 100, 0, 255],
            "the preview's frame changed"
        );
        let own = shared
            .own_frame
            .lock()
            .unwrap()
            .clone()
            .expect("the own frame");
        assert_ne!(
            own.rgba, frame.rgba,
            "the virtual camera's chain did not run"
        );
        // AND WITH NO OWN CHAIN there is no second picture.
        shared.set_own(None);
        assert!(shared.own_frame.lock().unwrap().is_none());
    }

    #[test]
    fn the_virtual_cameras_own_trouble_is_said() {
        // A BLUR FOR THE CALL WITH NO MODEL: the preview's chain is plain and
        // fine, and the trouble that comes back is the call's.
        let shared = Shared::default();
        let setup = Setup {
            program: (2, 1),
            fps: 30,
            cursor: false,
            backdrop: None,
            model: None,
        };
        shared.set_own(Some(Chain(vec![
            ronoctua_effects::Step::new("background-blur").expect("declared"),
        ])));
        let mut frame = Frame {
            width: 2,
            height: 1,
            rgba: vec![10, 20, 30, 255, 200, 100, 0, 255],
        };
        let (mut a, mut b) = (Effects::new(), Effects::new());
        assert_eq!(
            through_both(&mut frame, &shared, &setup, &mut a, &mut b),
            Some(Trouble::NoModel)
        );
        // AND WITH NO OWN CHAIN, nothing to say.
        shared.set_own(None);
        assert_eq!(
            through_both(&mut frame, &shared, &setup, &mut a, &mut b),
            None
        );
    }

    #[test]
    fn the_virtual_effect_is_the_previews_none_or_its_own() {
        use crate::prefs::VirtualEffect;
        assert_eq!(VirtualEffect::Preview.chain(), None);
        assert_eq!(VirtualEffect::None.chain(), Some(Chain(Vec::new())));
        let own = VirtualEffect::Own("negative".to_owned())
            .chain()
            .expect("a chain");
        assert_eq!(own.0.len(), 1);
        let ring = VirtualEffect::ring();
        assert_eq!(ring[0], VirtualEffect::Preview);
        assert_eq!(ring[1], VirtualEffect::None);
        assert!(ring.len() > 2);
    }

    #[test]
    fn a_colour_reads_with_and_without_alpha() {
        assert_eq!(
            &colour("ff000080").expect("parses").rgba[..4],
            &[255, 0, 0, 128]
        );
        assert_eq!(
            &colour("#00ff00").expect("parses").rgba[..4],
            &[0, 255, 0, 255]
        );
        assert!(colour("zz").is_none());
    }

    #[test]
    fn a_still_source_publishes_and_stops_when_dropped() {
        let setup = Setup {
            program: (64, 36),
            fps: 30,
            cursor: false,
            backdrop: None,
            model: None,
        };
        let live = start(
            Source::Colour {
                rgba: "0000ffff".into(),
            },
            setup,
        );
        let mut waited = 0;
        while live.shared.frame.lock().expect("lock").is_none() && waited < 100 {
            std::thread::sleep(Duration::from_millis(10));
            waited += 1;
        }
        assert!(live.shared.frame.lock().expect("lock").is_some());
        live.shared.set_chain(Chain(vec![
            ronoctua_effects::Step::new("negative").expect("declared"),
        ]));
        std::thread::sleep(Duration::from_millis(400));
        let f = live
            .shared
            .frame
            .lock()
            .expect("lock")
            .clone()
            .expect("a frame");
        assert_eq!(
            &f.rgba[..4],
            &[255, 255, 0, 255],
            "the chain re-rendered the still"
        );
        drop(live);
    }
}
