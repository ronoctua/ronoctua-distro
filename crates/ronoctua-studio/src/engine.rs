//! The studio's state, and the loop that turns it into frames.
//!
//! # One lock, never held while composing
//!
//! [`Engine`] is behind one mutex. The loop takes it to read what the scene
//! on air needs (the layers and their newest frames, as `Arc`s), lets it go
//! to COMPOSE — the only expensive step — and takes it again to hand the frame
//! to the outputs. An ask from a client waits at most for one of those two
//! short holds, never for a frame.
//!
//! # Nothing runs for nobody
//!
//! Sources exist only while somebody consumes the program: a preview window,
//! a recording, a stream, or the virtual camera ([`Engine::wanted`]). With
//! none of them the loop drops every source — the camera's light goes out —
//! and sleeps on a condition variable until an ask wakes it.

use std::{
    collections::HashMap,
    io::Write as _,
    os::unix::net::UnixStream,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant, SystemTime},
};

use ronoctua_effects::Frame;

use crate::{
    audio::{self, Sound},
    compose::{self, Placed},
    mix::Knobs,
    outputs::{self, Recording, Streaming, Virtual},
    prefs::{self, Preferences, Sends},
    preview::Ring,
    scene::{Book, Running, Source},
    sources::{self, Live, Setup},
    wire::{self, Ask, CameraSeen, LayerSeen, Rolling, Say, Sound as Which, State, Trouble},
};

/// Everything the daemon holds.
pub struct Engine {
    prefs: Preferences,
    book: Book,
    stamps: (Option<SystemTime>, Option<SystemTime>),
    book_dirty: Option<Instant>,
    cameras: Vec<ronoctua_camera::Camera>,
    sources: HashMap<String, (Source, Live)>,
    scene: String,
    fading: Option<(Arc<Frame>, Instant, u32)>,
    running: Option<(Running, Instant)>,
    recording: Option<Recording>,
    streaming: Option<Streaming>,
    virtual_cam: Option<Virtual>,
    sound: Option<Sound>,
    ring: Option<Ring>,
    viewers: Vec<UnixStream>,
    watchers: Vec<UnixStream>,
    countdown: Option<Instant>,
    photo_due: bool,
    last_photo: Option<String>,
    last_recording: Option<String>,
    lasting: Vec<Trouble>,
    program: Option<Arc<Frame>>,
    frames: u32,
    measured: u32,
    fps_clock: Instant,
    model: Option<Arc<Vec<u8>>>,
    dirty: bool,
    last_broadcast: Instant,
}

/// The engine and the bell that wakes its loop.
pub type Shared = Arc<(Mutex<Engine>, Condvar)>;

fn stamp(path: Option<std::path::PathBuf>) -> Option<SystemTime> {
    std::fs::metadata(path?).and_then(|m| m.modified()).ok()
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Reads the settings, the book and the model.
    pub fn new() -> Self {
        let prefs = prefs::store().read();
        let book = prefs::book().read();
        let model = std::env::var_os("RONOCTUA_SEGMENT_MODEL")
            .and_then(|p| std::fs::read(p).ok())
            .map(Arc::new);
        if model.is_none() {
            tracing::info!("no segmentation model: the background effects will refuse");
        }
        let scene = book
            .scene(&book.active)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        Self {
            stamps: (stamp(prefs::store().path()), stamp(prefs::book().path())),
            prefs,
            book,
            book_dirty: None,
            cameras: Vec::new(),
            sources: HashMap::new(),
            scene,
            fading: None,
            running: None,
            recording: None,
            streaming: None,
            virtual_cam: None,
            sound: None,
            ring: None,
            viewers: Vec::new(),
            watchers: Vec::new(),
            countdown: None,
            photo_due: false,
            last_photo: None,
            last_recording: None,
            lasting: Vec::new(),
            program: None,
            frames: 0,
            measured: 0,
            fps_clock: Instant::now(),
            model,
            dirty: true,
            last_broadcast: Instant::now(),
        }
    }

    /// Whether anything consumes the program.
    pub fn wanted(&self) -> bool {
        !self.viewers.is_empty()
            || self.recording.is_some()
            || self.streaming.is_some()
            || self.virtual_cam.is_some()
            || self.photo_due
            || self.countdown.is_some()
    }

    fn setup(&self) -> Setup {
        Setup {
            program: self.prefs.size(),
            fps: self.prefs.rate(),
            cursor: self.prefs.cursor,
            backdrop: self.prefs.backdrop.clone(),
            model: self.model.clone(),
        }
    }

    /// Starts the sources the scene on air (and the one fading out) needs,
    /// stops the rest, and hands each its chain.
    fn sync_sources(&mut self) {
        if !self.wanted() {
            self.sources.clear();
            return;
        }
        let mut need: Vec<Source> = self.book.sources_of(&self.scene);
        let mut chains: HashMap<String, ronoctua_effects::Chain> = HashMap::new();
        if let Some(scene) = self.book.scene(&self.scene) {
            for l in scene.layers.iter().filter(|l| l.visible) {
                chains.entry(l.source.key()).or_insert_with(|| l.chain());
            }
        }
        if self.prefs.virtual_sends == Sends::Camera && self.virtual_cam.is_some() {
            let cam = Source::Camera { identity: None };
            if !need.contains(&cam) {
                need.push(cam);
            }
        }
        let keys: Vec<String> = need.iter().map(Source::key).collect();
        self.sources.retain(|k, _| keys.contains(k));
        let setup = self.setup();
        for s in need {
            let key = s.key();
            let (_, live) = self
                .sources
                .entry(key.clone())
                .or_insert_with(|| (s.clone(), sources::start(s, setup.clone())));
            live.shared
                .set_chain(chains.remove(&key).unwrap_or_default());
        }
    }

    /// The layers of the scene on air, ready to compose.
    fn layers(&self) -> Vec<(Arc<Frame>, crate::scene::Layer)> {
        let Some(scene) = self.book.scene(&self.scene) else {
            return Vec::new();
        };
        scene
            .layers
            .iter()
            .filter(|l| l.visible)
            .filter_map(|l| {
                let (_, live) = self.sources.get(&l.source.key())?;
                let frame = live.shared.frame.lock().ok()?.clone()?;
                Some((frame, l.clone()))
            })
            .collect()
    }

    fn camera_frame(&self) -> Option<Arc<Frame>> {
        let key = self
            .book
            .camera_layer(&self.scene)
            .and_then(|i| {
                self.book
                    .scene(&self.scene)
                    .map(|s| s.layers[i].source.key())
            })
            .unwrap_or_else(|| Source::Camera { identity: None }.key());
        self.sources.get(&key)?.1.shared.frame.lock().ok()?.clone()
    }

    /// Every trouble the sources and outputs have now.
    fn troubles(&self) -> Vec<Trouble> {
        let mut out = self.lasting.clone();
        for (_, live) in self.sources.values() {
            if let Ok(t) = live.shared.trouble.lock()
                && let Some(t) = t.clone()
                && !out.contains(&t)
            {
                out.push(t);
            }
        }
        out
    }

    /// The whole state, as a client sees it.
    pub fn state(&self) -> State {
        let (width, height) = self.prefs.size();
        let scene = self.book.scene(&self.scene);
        let cam_layer = self
            .book
            .camera_layer(&self.scene)
            .and_then(|i| scene.map(|s| &s.layers[i]));
        let camera_id = cam_layer.and_then(|l| match &l.source {
            Source::Camera { identity } => identity.clone(),
            _ => None,
        });
        let camprefs = ronoctua_camera::prefs::store().read();
        let showing = camera_id.map(ronoctua_camera::Identity).or_else(|| {
            ronoctua_camera::default_camera(&self.cameras, camprefs.default.as_ref())
                .map(|c| c.identity.clone())
        });
        let effect = cam_layer.and_then(|l| l.effect().map(str::to_owned));
        let values_of = |id: Option<&str>| -> Vec<i32> {
            cam_layer
                .and_then(|l| l.effects.iter().find(|e| Some(e.id.as_str()) == id))
                .map(|e| {
                    let step = ronoctua_effects::Step {
                        id: e.id.clone(),
                        values: e.values.clone(),
                    };
                    ronoctua_effects::declared(&e.id)
                        .map(|d| d.params.iter().map(|p| step.value(p.name)).collect())
                        .unwrap_or_default()
                })
                .unwrap_or_default()
        };
        let effect_values = values_of(effect.as_deref());
        let background = cam_layer.and_then(|l| l.background().map(str::to_owned));
        let background_values = values_of(background.as_deref());
        let board = self
            .sound
            .as_ref()
            .and_then(|s| s.board.lock().ok().map(|b| b.levels));
        State {
            cameras: self
                .cameras
                .iter()
                .filter(|c| c.kind != ronoctua_camera::Kind::Virtual)
                .map(|c| CameraSeen {
                    identity: c.identity.0.clone(),
                    card: c.card.clone(),
                    infrared: c.kind == ronoctua_camera::Kind::Infrared,
                    streamable: c.streamable,
                    shuttered: c.shuttered() == Some(true),
                })
                .collect(),
            camera: showing.as_ref().map(|i| i.0.clone()),
            mirror: showing.as_ref().is_some_and(|i| camprefs.of(i).mirror),
            scene: self.scene.clone(),
            scenes: self.book.scenes.iter().map(|s| s.name.clone()).collect(),
            scripts: self.book.scripts.iter().map(|s| s.name.clone()).collect(),
            running: self.running.as_ref().map(|(r, _)| r.clone()),
            layers: scene
                .map(|s| {
                    s.layers
                        .iter()
                        .map(|l| LayerSeen {
                            rect: l.rect,
                            visible: l.visible,
                            camera: matches!(l.source, Source::Camera { .. }),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            effect,
            effect_values,
            background,
            background_values,
            recording: self.recording.as_ref().map(|r| Rolling {
                since: r.since,
                target: r.finished.to_string_lossy().into_owned(),
            }),
            streaming: self.streaming.as_ref().map(|s| Rolling {
                since: s.since,
                target: s.url.clone(),
            }),
            virtual_node: self
                .virtual_cam
                .as_ref()
                .map(|v| v.node.to_string_lossy().into_owned()),
            countdown: self.countdown.map(|d| {
                d.saturating_duration_since(Instant::now())
                    .as_secs()
                    .saturating_add(1)
                    .min(255) as u8
            }),
            last_photo: self.last_photo.clone(),
            last_recording: self.last_recording.clone(),
            levels: board.unwrap_or([0, 0]),
            gains: [self.prefs.microphone_gain, self.prefs.desktop_gain],
            muted: [!self.prefs.microphone, !self.prefs.desktop],
            width,
            height,
            fps: self.prefs.rate(),
            measured_fps: self.measured,
            troubles: self.troubles(),
        }
    }

    /// Sends the state to every watcher; a watcher that went away is dropped.
    pub fn broadcast(&mut self) {
        self.dirty = false;
        self.last_broadcast = Instant::now();
        if self.watchers.is_empty() {
            return;
        }
        let mut line = serde_json::to_vec(&Say::State(Box::new(self.state()))).unwrap_or_default();
        line.push(b'\n');
        self.watchers.retain_mut(|w| w.write_all(&line).is_ok());
    }

    /// Adds a watcher, and tells it the state at once.
    pub fn add_watcher(&mut self, stream: UnixStream) {
        let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
        self.watchers.push(stream);
        self.dirty = true;
        self.broadcast();
    }

    /// Adds a preview viewer.
    pub fn add_viewer(&mut self, stream: UnixStream) -> anyhow::Result<()> {
        let size = self.prefs.size();
        if self.ring.as_ref().is_none_or(|r| r.size() != size) {
            self.ring = Some(Ring::new(size.0, size.1)?);
            // A new ring is a new size: the old viewers hold the old one.
            self.viewers.clear();
        }
        if let Some(ring) = &self.ring {
            ring.hand_over(&stream)?;
        }
        stream.set_nonblocking(true)?;
        self.viewers.push(stream);
        self.dirty = true;
        Ok(())
    }

    /// Replaces the camera list.
    pub fn cameras_changed(&mut self, cameras: Vec<ronoctua_camera::Camera>) {
        self.cameras = cameras;
        self.dirty = true;
    }

    fn save_book_soon(&mut self) {
        self.book_dirty.get_or_insert_with(Instant::now);
        self.dirty = true;
    }

    fn write_book(&mut self) {
        self.book.active = self.scene.clone();
        if let Err(e) = prefs::keep(&prefs::book(), &self.book) {
            tracing::warn!(error = %e, "the scene book was not written");
        }
        self.stamps.1 = stamp(prefs::book().path());
        self.book_dirty = None;
    }

    fn write_prefs(&mut self) {
        if let Err(e) = prefs::keep(&prefs::store(), &self.prefs) {
            tracing::warn!(error = %e, "the studio settings were not written");
        }
        self.stamps.0 = stamp(prefs::store().path());
    }

    fn camera_layer_mut(&mut self) -> Option<&mut crate::scene::Layer> {
        let i = self.book.camera_layer(&self.scene)?;
        let scene = self.scene.clone();
        self.book.scene_mut(&scene)?.layers.get_mut(i)
    }

    fn put_on_air(&mut self, name: &str) {
        if name == self.scene || self.book.scenes.iter().all(|s| s.name != name) {
            return;
        }
        let ms = self.book.scene(name).map_or(0, |s| s.transition.millis());
        if ms > 0
            && let Some(p) = &self.program
        {
            self.fading = Some((p.clone(), Instant::now(), ms));
        }
        self.scene = name.to_owned();
        self.save_book_soon();
    }

    fn sound_wanted(&self) -> bool {
        (self.prefs.microphone || self.prefs.desktop)
            && (self.recording.is_some() || self.streaming.is_some())
    }

    fn knobs(&self) -> [Knobs; 2] {
        [
            Knobs {
                gain: f32::from(self.prefs.microphone_gain) / 100.0,
                muted: !self.prefs.microphone,
            },
            Knobs {
                gain: f32::from(self.prefs.desktop_gain) / 100.0,
                muted: !self.prefs.desktop,
            },
        ]
    }

    fn ensure_sound(&mut self) {
        if self.sound.is_none() && (self.prefs.microphone || self.prefs.desktop) {
            self.sound = Some(audio::start(
                self.prefs.microphone,
                self.prefs.desktop,
                self.knobs(),
            ));
        }
    }

    fn settle_sound(&mut self) {
        if !self.sound_wanted() {
            self.sound = None;
        }
    }

    /// Handles one ask. Stopping a recording is slow (a remux), so it is
    /// handed to a thread with the shared handle.
    pub fn ask(&mut self, ask: Ask, shared: &Shared) -> Say {
        self.dirty = true;
        match ask {
            Ask::Watch | Ask::Look | Ask::Preview => {}
            Ask::Photo => {
                if self.prefs.countdown > 0 {
                    self.countdown =
                        Some(Instant::now() + Duration::from_secs(u64::from(self.prefs.countdown)));
                } else {
                    self.photo_due = true;
                }
            }
            Ask::Record { on } => {
                let on = on.unwrap_or(self.recording.is_none());
                if on && self.recording.is_none() {
                    let sound = self.prefs.microphone || self.prefs.desktop;
                    match Recording::start(&self.prefs, sound) {
                        Ok(r) => {
                            self.lasting
                                .retain(|t| !matches!(t, Trouble::Encoder { .. }));
                            self.ensure_sound();
                            if let (Some(s), Some(f)) = (&self.sound, &r.fifo) {
                                s.attach(f.clone());
                            }
                            self.recording = Some(r);
                        }
                        Err(e) => {
                            let t = Trouble::Encoder { why: e.to_string() };
                            self.lasting.push(t.clone());
                            return Say::Refused(t);
                        }
                    }
                } else if !on && let Some(r) = self.recording.take() {
                    if let (Some(s), Some(f)) = (&self.sound, &r.fifo) {
                        s.detach(f);
                    }
                    self.settle_sound();
                    let shared = shared.clone();
                    let _ = std::thread::Builder::new()
                        .name("studio-finish".into())
                        .spawn(move || {
                            let path = r.stop();
                            let (lock, bell) = &*shared;
                            if let Ok(mut e) = lock.lock() {
                                e.last_recording = Some(path.to_string_lossy().into_owned());
                                e.broadcast();
                            }
                            bell.notify_all();
                        });
                }
            }
            Ask::Stream { on } => {
                let on = on.unwrap_or(self.streaming.is_none());
                if on && self.streaming.is_none() {
                    if self.prefs.stream_url.is_none() || !prefs::has_stream_key() {
                        return Say::Refused(Trouble::NoStreamTarget);
                    }
                    let sound = self.prefs.microphone || self.prefs.desktop;
                    match Streaming::start(&self.prefs, sound) {
                        Ok(s) => {
                            self.lasting
                                .retain(|t| !matches!(t, Trouble::Stream { .. }));
                            self.ensure_sound();
                            if let (Some(snd), Some(f)) = (&self.sound, &s.fifo) {
                                snd.attach(f.clone());
                            }
                            self.streaming = Some(s);
                        }
                        Err(e) => return Say::Refused(Trouble::Stream { why: e.to_string() }),
                    }
                } else if !on && let Some(s) = self.streaming.take() {
                    if let (Some(snd), Some(f)) = (&self.sound, &s.fifo) {
                        snd.detach(f);
                    }
                    drop(s);
                    self.settle_sound();
                }
            }
            Ask::Virtual { on } => {
                let on = on.unwrap_or(self.virtual_cam.is_none());
                if on && self.virtual_cam.is_none() {
                    match Virtual::start(&self.prefs) {
                        Ok(v) => self.virtual_cam = Some(v),
                        Err(t) => return Say::Refused(t),
                    }
                } else if !on {
                    self.virtual_cam = None;
                }
            }
            Ask::Scene { name } => self.put_on_air(&name),
            Ask::SceneTurn { by } => {
                if let Some(next) = self.book.turned(&self.scene, by).map(str::to_owned) {
                    self.put_on_air(&next);
                }
            }
            Ask::Script { name } => {
                match name.and_then(|n| self.book.scripts.iter().find(|s| s.name == n).cloned()) {
                    Some(script) => {
                        if let Some((running, first)) = Running::start(&script) {
                            self.running = Some((running, Instant::now()));
                            self.put_on_air(&first);
                        }
                    }
                    None => self.running = None,
                }
            }
            Ask::Next => {
                if let Some((r, clock)) = &mut self.running
                    && let Some(script) = self
                        .book
                        .scripts
                        .iter()
                        .find(|s| s.name == r.script)
                        .cloned()
                {
                    *clock = Instant::now();
                    match r.advance(&script) {
                        Ok(scene) => self.put_on_air(&scene),
                        Err(()) => self.running = None,
                    }
                }
            }
            // CHOSEN, from a menu: only a declared id of the right family is
            // taken, so a menu built from a stale list cannot put a
            // background step in the effect's place.
            Ask::Effect { name } => {
                if chosen_in(name.as_deref(), false)
                    && let Some(layer) = self.camera_layer_mut()
                {
                    layer.set_effect(name.as_deref());
                    self.save_book_soon();
                }
            }
            Ask::Background { name } => {
                if chosen_in(name.as_deref(), true)
                    && let Some(layer) = self.camera_layer_mut()
                {
                    layer.set_background(name.as_deref());
                    self.save_book_soon();
                }
            }
            Ask::Camera { identity } => {
                let known = self.cameras.iter().any(|c| {
                    c.identity.0 == identity
                        && c.streamable
                        && c.kind != ronoctua_camera::Kind::Virtual
                });
                // The same rule as the ring: only a CAMERA layer changes camera.
                if known
                    && let Some(layer) = self.camera_layer_mut()
                    && matches!(layer.source, Source::Camera { .. })
                {
                    layer.source = Source::Camera {
                        identity: Some(identity),
                    };
                    self.save_book_soon();
                }
            }
            Ask::EffectTurn { by } => {
                if let Some(layer) = self.camera_layer_mut() {
                    let ring: Vec<&str> = ronoctua_effects::DECLARED
                        .iter()
                        .filter(|d| d.family != "background")
                        .map(|d| d.id)
                        .collect();
                    let next = turn_ring(&ring, layer.effect(), by);
                    layer.set_effect(next);
                    self.save_book_soon();
                }
            }
            Ask::BackgroundTurn { by } => {
                if let Some(layer) = self.camera_layer_mut() {
                    let ring: Vec<&str> = ronoctua_effects::DECLARED
                        .iter()
                        .filter(|d| d.family == "background")
                        .map(|d| d.id)
                        .collect();
                    let next = turn_ring(&ring, layer.background(), by);
                    layer.set_background(next);
                    self.save_book_soon();
                }
            }
            Ask::EffectParam {
                name,
                by,
                background,
            } => {
                if let Some(layer) = self.camera_layer_mut() {
                    let target = if background {
                        layer.background()
                    } else {
                        layer.effect()
                    }
                    .map(str::to_owned);
                    if let Some(step) = layer
                        .effects
                        .iter_mut()
                        .find(|e| Some(&e.id) == target.as_ref())
                    {
                        let mut s = ronoctua_effects::Step {
                            id: step.id.clone(),
                            values: step.values.clone(),
                        };
                        s.turn(&name, by);
                        step.values = s.values;
                        self.save_book_soon();
                    }
                }
            }
            Ask::CameraTurn { by } => {
                let ids: Vec<String> = self
                    .cameras
                    .iter()
                    .filter(|c| c.streamable && c.kind != ronoctua_camera::Kind::Virtual)
                    .map(|c| c.identity.0.clone())
                    .collect();
                let current = self.state().camera;
                // ONLY A CAMERA LAYER changes camera: in a scene with none, the
                // layer the keys act on is a screen or a film, and turning it
                // into a camera would throw away what the person put there.
                if let Some(layer) = self.camera_layer_mut()
                    && matches!(layer.source, Source::Camera { .. })
                {
                    let at = current
                        .and_then(|c| ids.iter().position(|i| *i == c))
                        .unwrap_or(0) as i32;
                    if !ids.is_empty() {
                        let next = ids[(at + by).rem_euclid(ids.len() as i32) as usize].clone();
                        layer.source = Source::Camera {
                            identity: Some(next),
                        };
                        self.save_book_soon();
                    }
                }
            }
            Ask::Mirror => {
                if let Some(id) = self.state().camera.map(ronoctua_camera::Identity) {
                    let store = ronoctua_camera::prefs::store();
                    let mut p = store.read();
                    let mut per = p.of(&id);
                    per.mirror = !per.mirror;
                    p.set(&id, per);
                    if ronoctua_camera::prefs::keep(&store, &p).is_ok() {
                        // The mirror is ffmpeg's `hflip`: the capture reopens.
                        self.sources
                            .retain(|_, (s, _)| !matches!(s, Source::Camera { .. }));
                    }
                }
            }
            Ask::Place { layer, rect } => {
                let scene = self.scene.clone();
                if let Some(l) = self
                    .book
                    .scene_mut(&scene)
                    .and_then(|s| s.layers.get_mut(layer))
                {
                    l.rect = rect;
                    self.save_book_soon();
                }
            }
            Ask::Show { layer, on } => {
                let scene = self.scene.clone();
                if let Some(l) = self
                    .book
                    .scene_mut(&scene)
                    .and_then(|s| s.layers.get_mut(layer))
                {
                    l.visible = on;
                    self.save_book_soon();
                }
            }
            Ask::Sound { which, gain, muted } => {
                let (g, m) = match which {
                    Which::Microphone => {
                        (&mut self.prefs.microphone_gain, &mut self.prefs.microphone)
                    }
                    Which::Desktop => (&mut self.prefs.desktop_gain, &mut self.prefs.desktop),
                };
                if let Some(gain) = gain {
                    *g = gain.min(200);
                }
                if let Some(muted) = muted {
                    *m = !muted;
                }
                self.write_prefs();
                if let Some(s) = &self.sound
                    && let Ok(mut b) = s.board.lock()
                {
                    b.knobs = self.knobs();
                }
            }
            Ask::SceneAdd { name } => {
                let name = name.trim().to_owned();
                if name.is_empty() || self.book.scenes.iter().any(|s| s.name == name) {
                    return Say::Refused(Trouble::Name);
                }
                let mut copy = self
                    .book
                    .scene(&self.scene)
                    .cloned()
                    .unwrap_or_else(|| crate::scene::Book::default().scenes[0].clone());
                copy.name.clone_from(&name);
                self.book.scenes.push(copy);
                self.put_on_air(&name);
                self.save_book_soon();
            }
            Ask::SceneRename { from, to } => {
                let to = to.trim().to_owned();
                if to.is_empty() || self.book.scenes.iter().any(|s| s.name == to) {
                    return Say::Refused(Trouble::Name);
                }
                if let Some(s) = self.book.scenes.iter_mut().find(|s| s.name == from) {
                    s.name.clone_from(&to);
                    for script in &mut self.book.scripts {
                        for cue in &mut script.cues {
                            if cue.scene == from {
                                cue.scene.clone_from(&to);
                            }
                        }
                    }
                    if self.scene == from {
                        self.scene = to;
                    }
                    self.save_book_soon();
                }
            }
            Ask::SceneRemove { name } => {
                if self.book.scenes.len() <= 1 {
                    return Say::Refused(Trouble::Name);
                }
                self.book.scenes.retain(|s| s.name != name);
                if self.scene == name {
                    self.scene = self.book.scenes[0].name.clone();
                }
                self.save_book_soon();
            }
            Ask::ScriptAdd { script } => {
                self.book.scripts.retain(|s| s.name != script.name);
                self.book.scripts.push(script);
                self.save_book_soon();
            }
            Ask::ScriptRemove { name } => {
                self.book.scripts.retain(|s| s.name != name);
                if self.running.as_ref().is_some_and(|(r, _)| r.script == name) {
                    self.running = None;
                }
                self.save_book_soon();
            }
            Ask::Reload => self.reload(),
        }
        Say::Done
    }

    fn reload(&mut self) {
        let before = self.prefs.clone();
        self.prefs = prefs::store().read();
        self.book = prefs::book().read();
        self.stamps = (stamp(prefs::store().path()), stamp(prefs::book().path()));
        if self.book.scenes.iter().all(|s| s.name != self.scene) {
            self.scene = self
                .book
                .scene(&self.book.active)
                .map(|s| s.name.clone())
                .unwrap_or_default();
        }
        // A new size, rate, cursor or backdrop reaches the sources only through
        // a restart.
        if before.size() != self.prefs.size()
            || before.rate() != self.prefs.rate()
            || before.cursor != self.prefs.cursor
            || before.backdrop != self.prefs.backdrop
        {
            self.sources.clear();
        }
        // AND EVERY CAMERA REOPENS: its mode and its mirror are read when it
        // opens (`camera.json`), and a reload is how the settings view says
        // one of them changed.
        self.sources
            .retain(|_, (s, _)| !matches!(s, Source::Camera { .. }));
        self.dirty = true;
    }

    /// Rereads the two files when somebody else changed them.
    fn follow_files(&mut self) {
        let now = (stamp(prefs::store().path()), stamp(prefs::book().path()));
        if now != self.stamps && self.book_dirty.is_none() {
            self.reload();
        }
    }
}

/// The ring of a family: `None` (no effect), then every declared id, then
/// `None` again.
/// Whether `name` may be put in the effect's place (`background` false) or
/// the background's: nothing always may, and an id only when it is declared in
/// that family.
fn chosen_in(name: Option<&str>, background: bool) -> bool {
    name.is_none_or(|n| {
        ronoctua_effects::DECLARED
            .iter()
            .any(|d| d.id == n && (d.family == "background") == background)
    })
}

fn turn_ring<'a>(ring: &[&'a str], now: Option<&str>, by: i32) -> Option<&'a str> {
    let n = ring.len() as i32 + 1;
    let at = now
        .and_then(|id| ring.iter().position(|r| *r == id))
        .map_or(0, |i| i as i32 + 1);
    let next = (at + by).rem_euclid(n);
    (next > 0).then(|| ring[(next - 1) as usize])
}

/// The loop, on a thread of its own, for the life of the daemon.
pub fn run(shared: &Shared) {
    let mut canvas = Frame::default();
    let mut alone = Frame::default();
    let mut next_second = Instant::now();
    loop {
        let (lock, bell) = &**shared;
        // --- the first hold: what to draw ---
        let (layers, size, fps, fading, camera_only) = {
            let Ok(mut e) = lock.lock() else { return };
            if !e.wanted() {
                e.sources.clear();
                if e.dirty {
                    e.broadcast();
                }
                let _ = bell.wait_timeout(e, Duration::from_millis(500));
                continue;
            }
            e.sync_sources();
            let camera_only = (e.prefs.virtual_sends == Sends::Camera && e.virtual_cam.is_some())
                .then(|| e.camera_frame())
                .flatten();
            (
                e.layers(),
                e.prefs.size(),
                e.prefs.rate(),
                e.fading.clone(),
                camera_only,
            )
        };
        let began = Instant::now();
        // --- compose, with no lock held ---
        let placed: Vec<Placed<'_>> = layers
            .iter()
            .map(|(f, l)| Placed {
                frame: f,
                rect: l.rect,
                fit: l.fit,
                opacity: l.opacity,
                crop: l.crop,
            })
            .collect();
        compose::compose(&mut canvas, size.0, size.1, &placed);
        let mut faded = false;
        if let Some((from, since, ms)) = &fading {
            let t = since.elapsed().as_millis() as f32 / (*ms).max(1) as f32;
            if t < 1.0 {
                compose::fade(&mut canvas, from, t);
                faded = true;
            }
        }
        let program = Arc::new(canvas.clone());
        let camera_program = camera_only.map(|f| {
            compose::compose(
                &mut alone,
                size.0,
                size.1,
                &[Placed {
                    frame: &f,
                    rect: crate::scene::Rect::WHOLE,
                    fit: crate::scene::Fit::Fill,
                    opacity: 100,
                    crop: [0.0; 4],
                }],
            );
            Arc::new(alone.clone())
        });
        // --- the second hold: hand it over ---
        {
            let Ok(mut e) = lock.lock() else { return };
            if !faded {
                e.fading = None;
            }
            e.program = Some(program.clone());
            let viewing = !e.viewers.is_empty();
            let slot = match &mut e.ring {
                Some(ring) if viewing => ring.publish(&program.rgba).ok(),
                _ => None,
            };
            if let Some(slot) = slot {
                e.viewers.retain_mut(|v| match v.write(&[slot]) {
                    Ok(_) => true,
                    Err(err) => err.kind() == std::io::ErrorKind::WouldBlock,
                });
            }
            if let Some(r) = &mut e.recording {
                r.offer(&program);
            }
            if let Some(s) = &mut e.streaming {
                s.offer(&program);
            }
            let virtual_frame = camera_program.unwrap_or_else(|| program.clone());
            if let Some(v) = &mut e.virtual_cam {
                v.offer(&virtual_frame);
            }
            // A photo, when one is due.
            if let Some(due) = e.countdown {
                if Instant::now() >= due {
                    e.countdown = None;
                    e.photo_due = true;
                } else {
                    e.dirty = true;
                }
            }
            if e.photo_due {
                e.photo_due = false;
                match outputs::photo(&program, &e.prefs) {
                    Ok(p) => e.last_photo = Some(p.to_string_lossy().into_owned()),
                    Err(err) => e.lasting.push(Trouble::Write {
                        why: err.to_string(),
                    }),
                }
                e.dirty = true;
            }
            // Outputs that died say so and are let go.
            let mut fell = Vec::new();
            if let Some(r) = &mut e.recording
                && let Err(why) = r.check()
            {
                fell.push(Trouble::Encoder { why });
            }
            if !fell.is_empty()
                && let Some(r) = e.recording.take()
            {
                if let (Some(s), Some(f)) = (&e.sound, &r.fifo) {
                    s.detach(f);
                }
                e.settle_sound();
                // Finishing is slow (a remux): never under the lock.
                let shared = shared.clone();
                let _ = std::thread::Builder::new()
                    .name("studio-finish".into())
                    .spawn(move || {
                        let path = r.stop();
                        let (lock, _) = &*shared;
                        if let Ok(mut e) = lock.lock() {
                            e.last_recording = Some(path.to_string_lossy().into_owned());
                            e.broadcast();
                        }
                    });
            }
            if let Some(s) = &mut e.streaming
                && let Err(why) = s.check()
            {
                fell.push(Trouble::Stream { why });
                e.streaming = None;
                e.settle_sound();
            }
            if e.virtual_cam.as_mut().is_some_and(|v| !v.alive()) {
                e.virtual_cam = None;
                fell.push(Trouble::VirtualBusy);
            }
            for t in fell {
                if !e.lasting.contains(&t) {
                    e.lasting.push(t);
                }
                e.dirty = true;
            }
            // The script's clock.
            let script = e
                .running
                .as_ref()
                .and_then(|(r, _)| e.book.scripts.iter().find(|s| s.name == r.script).cloned());
            if let Some((r, clock)) = &mut e.running {
                let elapsed = clock.elapsed().as_millis() as u64;
                *clock = Instant::now();
                let step = script.map(|s| r.tick(&s, elapsed));
                match step {
                    Some(Ok(Some(scene))) => e.put_on_air(&scene),
                    Some(Err(())) | None => e.running = None,
                    Some(Ok(None)) => {}
                }
            }
            // The rate, the files, and the book's delayed write, once a second.
            e.frames += 1;
            if Instant::now() >= next_second {
                next_second = Instant::now() + Duration::from_secs(1);
                let secs = e.fps_clock.elapsed().as_secs_f32().max(0.001);
                e.measured = (e.frames as f32 / secs).round() as u32;
                e.frames = 0;
                e.fps_clock = Instant::now();
                e.follow_files();
                e.dirty = true;
            }
            if e.book_dirty
                .is_some_and(|since| since.elapsed() > Duration::from_millis(400))
            {
                e.write_book();
            }
            // Levels move while sound is captured: four states a second.
            if e.dirty
                || (e.sound.is_some() && e.last_broadcast.elapsed() > Duration::from_millis(250))
            {
                e.broadcast();
            }
        }
        let pace = Duration::from_secs(1) / fps.max(1);
        let spent = began.elapsed();
        if spent < pace {
            std::thread::sleep(pace - spent);
        }
    }
}

/// Serves one connection: one ask, and for a watcher or a viewer, the
/// connection is kept.
pub fn serve_one(stream: UnixStream, shared: &Shared) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut input = std::io::BufReader::new(read_half);
    let ask: Ask = match wire::read_line(&mut input) {
        Ok(Some(a)) => a,
        _ => return,
    };
    let (lock, bell) = &**shared;
    let Ok(mut e) = lock.lock() else { return };
    match ask {
        Ask::Watch => e.add_watcher(stream),
        Ask::Preview => {
            if let Err(err) = e.add_viewer(stream) {
                tracing::warn!(error = %err, "a preview was refused");
            }
        }
        Ask::Look => {
            let mut out = stream;
            let _ = wire::write_line(&mut out, &Say::State(Box::new(e.state())));
        }
        other => {
            let say = e.ask(other, shared);
            e.broadcast();
            let mut out = stream;
            let _ = wire::write_line(&mut out, &say);
        }
    }
    drop(e);
    bell.notify_all();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chosen_effect_must_be_of_its_family() {
        assert!(chosen_in(None, false));
        assert!(chosen_in(Some("grey"), false));
        assert!(!chosen_in(Some("grey"), true));
        assert!(chosen_in(Some("background-blur"), true));
        assert!(!chosen_in(Some("background-blur"), false));
        assert!(!chosen_in(Some("no-such-effect"), false));
    }

    #[test]
    fn the_effect_ring_passes_through_none() {
        let ring = ["a", "b"];
        assert_eq!(turn_ring(&ring, None, 1), Some("a"));
        assert_eq!(turn_ring(&ring, Some("b"), 1), None);
        assert_eq!(turn_ring(&ring, None, -1), Some("b"));
        assert_eq!(turn_ring(&ring, Some("a"), -1), None);
    }
}
