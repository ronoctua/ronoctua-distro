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
//! a recording, a stream, or the virtual camera ([`Engine::wanted`]) — or
//! somebody reads the HANDS ([`crate::hands`]). With none of them the loop
//! drops every source — the camera's light goes out — and sleeps on a
//! condition variable until an ask wakes it.
//!
//! # Hands alone compose nothing
//!
//! Read by nobody else, the camera is opened SMALL ([`HANDS_SIZE`], which is
//! what the palm detector's 192² needs and a quarter of a 720p frame to
//! decode) and the loop draws no program: the only work left is the hands'
//! own thread. A consumer of the program arriving reopens the camera at the
//! program's size, and leaving closes it back.

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
    /// Every camera, as the watch last read them — and NONE while no camera
    /// is in use: reading one opens it, and its light blinks
    /// ([`Engine::follow_cameras`]).
    cameras: Vec<ronoctua_camera::Camera>,
    /// The watch that reads them, raised only while a camera is in use.
    camera_watch: Option<ronoctua_camera::watch::Handle>,
    /// Which raising of the watch is the current one: a reading of a watch
    /// already let go is dropped ([`Engine::cameras_read`]).
    camera_epoch: u64,
    /// The watch raised has answered: `cameras` is its reading, and not
    /// the nothing it starts from.
    cameras_heard: bool,
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
    /// Who reads the hands, and the two models' bytes when installed.
    pub(crate) hand_readers: Vec<UnixStream>,
    hand_models: Option<Arc<(Vec<u8>, Vec<u8>)>>,
    pub(crate) hands_running: bool,
    /// The sources now open were opened for the hands alone, small.
    light: bool,
    dirty: bool,
    last_broadcast: Instant,
    /// A SECOND COPY OF THE SYSTEM (`nix run .#dev`, `RONOCTUA_INSTANCE`)
    /// REHEARSES, as its brain does: a photo, a recording and a stream go
    /// through their motions on screen — the countdown, the rolling clock —
    /// and nothing is written to `Pictures` or `Videos`, nothing is sent and
    /// the virtual camera is left to the system's own studio.
    rehearsing: bool,
    /// A rehearsed recording: when it "started", seconds since the epoch.
    rehearsed_recording: Option<u64>,
    /// A rehearsed stream: when it "started", and where it would go.
    rehearsed_stream: Option<(u64, String)>,
}

/// Seconds since the epoch, now.
fn epoch_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Whether this studio is a second copy of the system's, which rehearses.
#[must_use]
pub fn rehearsing_here() -> bool {
    ronoctua_xdg::instance().is_some()
}

/// The camera's size when only the hands read it.
pub const HANDS_SIZE: (u32, u32) = (640, 360);

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
            camera_watch: None,
            camera_epoch: 0,
            cameras_heard: false,
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
            // THE REHEARSAL SAYS ITSELF, for as long as the studio lives: a
            // person pressing record in the development loop sees that
            // nothing is written, rather than a file that never appears.
            lasting: if rehearsing_here() {
                vec![Trouble::Rehearsal]
            } else {
                Vec::new()
            },
            program: None,
            frames: 0,
            measured: 0,
            fps_clock: Instant::now(),
            model,
            hand_readers: Vec::new(),
            hand_models: crate::hands::models(),
            hands_running: false,
            light: false,
            dirty: true,
            last_broadcast: Instant::now(),
            rehearsing: rehearsing_here(),
            rehearsed_recording: None,
            rehearsed_stream: None,
        }
    }

    /// Whether anything needs a source open.
    pub fn wanted(&self) -> bool {
        self.composing() || !self.hand_readers.is_empty()
    }

    /// Whether anything consumes the PROGRAM, which is what makes the loop
    /// compose.
    pub fn composing(&self) -> bool {
        !self.viewers.is_empty()
            || self.recording.is_some()
            || self.streaming.is_some()
            || self.virtual_cam.is_some()
            || self.photo_due
            || self.countdown.is_some()
    }

    /// The source the hands are read from: the scene's camera when the
    /// program is being composed (one device, one capture), the default
    /// camera otherwise.
    fn hands_source(&self) -> Source {
        if self.composing()
            && let Some(i) = self.book.camera_layer(&self.scene)
            && let Some(scene) = self.book.scene(&self.scene)
        {
            return scene.layers[i].source.clone();
        }
        Source::Camera { identity: None }
    }

    /// The newest frame of the hands' source, and whether it is mirrored.
    pub(crate) fn hands_frame(&self) -> Option<(Arc<Frame>, bool)> {
        let (_, live) = self.sources.get(&self.hands_source().key())?;
        let frame = live.shared.frame.lock().ok()?.clone()?;
        Some((
            frame,
            live.shared
                .mirrored
                .load(std::sync::atomic::Ordering::Relaxed),
        ))
    }

    /// Adds a reader of the hands, starting their thread; refused, with the
    /// reason on the connection, when the models are not installed.
    pub fn add_hand_reader(&mut self, stream: UnixStream, shared: &Shared) {
        let Some(models) = self.hand_models.clone() else {
            let mut out = stream;
            let _ = wire::write_line(&mut out, &Say::Refused(Trouble::NoHandModel));
            return;
        };
        let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
        self.hand_readers.push(stream);
        self.dirty = true;
        if !self.hands_running {
            self.hands_running = true;
            crate::hands::start(shared.clone(), models);
        }
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
        let light = !self.composing();
        if light != self.light {
            // Opened at the other size: reopened, which is the one moment the
            // camera's light blinks.
            self.sources.clear();
            self.light = light;
        }
        let mut need: Vec<Source> = if light {
            Vec::new()
        } else {
            self.book.sources_of(&self.scene)
        };
        if !self.hand_readers.is_empty() {
            let hands = self.hands_source();
            if !need.contains(&hands) {
                need.push(hands);
            }
        }
        let mut chains: HashMap<String, ronoctua_effects::Chain> = HashMap::new();
        if let Some(scene) = self.book.scene(&self.scene) {
            for l in scene.layers.iter().filter(|l| l.visible) {
                chains.entry(l.source.key()).or_insert_with(|| l.chain());
            }
        }
        if !light && self.prefs.virtual_sends == Sends::Camera && self.virtual_cam.is_some() {
            let cam = Source::Camera { identity: None };
            if !need.contains(&cam) {
                need.push(cam);
            }
        }
        let keys: Vec<String> = need.iter().map(Source::key).collect();
        self.sources.retain(|k, _| keys.contains(k));
        // THE VIRTUAL CAMERA'S OWN CHAIN goes on the camera it sends, and on
        // no other source — and only while it sends the camera alone.
        let own =
            (!light && self.prefs.virtual_sends == Sends::Camera && self.virtual_cam.is_some())
                .then(|| self.prefs.virtual_effect.chain())
                .flatten();
        let virtual_key = self.virtual_key();
        let mut setup = self.setup();
        if light {
            setup.program = HANDS_SIZE;
        }
        for s in need {
            let key = s.key();
            let (_, live) = self
                .sources
                .entry(key.clone())
                .or_insert_with(|| (s.clone(), sources::start(s, setup.clone())));
            live.shared
                .set_chain(chains.remove(&key).unwrap_or_default());
            live.shared.set_own(if key == virtual_key {
                own.clone()
            } else {
                None
            });
        }
    }

    /// The key of the camera the virtual camera sends when it sends the
    /// camera alone: the scene's camera layer, or the default camera.
    fn virtual_key(&self) -> String {
        self.book
            .camera_layer(&self.scene)
            .and_then(|i| {
                self.book
                    .scene(&self.scene)
                    .map(|s| s.layers[i].source.key())
            })
            .unwrap_or_else(|| Source::Camera { identity: None }.key())
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

    /// What the virtual camera sends when it sends the camera alone: the
    /// picture through its own chain when it has one, the preview's
    /// otherwise.
    fn camera_frame(&self) -> Option<Arc<Frame>> {
        let shared = &self.sources.get(&self.virtual_key())?.1.shared;
        if shared.own.lock().ok().is_some_and(|c| c.is_some()) {
            return shared.own_frame.lock().ok()?.clone();
        }
        shared.frame.lock().ok()?.clone()
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
            recording: self
                .recording
                .as_ref()
                .map(|r| Rolling {
                    since: r.since,
                    target: r.finished.to_string_lossy().into_owned(),
                })
                .or_else(|| {
                    self.rehearsed_recording.map(|since| Rolling {
                        since,
                        target: String::new(),
                    })
                }),
            streaming: self
                .streaming
                .as_ref()
                .map(|s| Rolling {
                    since: s.since,
                    target: s.url.clone(),
                })
                .or_else(|| {
                    self.rehearsed_stream.as_ref().map(|(since, url)| Rolling {
                        since: *since,
                        target: url.clone(),
                    })
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

    /// Whether the cameras are to be read now: while one is OPEN for a
    /// source — its light is on — or the camera app shows the program (a
    /// viewer: a person opened the camera). Nothing else asks for them: a
    /// door watching the state, the screen recorded alone, the virtual
    /// camera sending a scene with no camera, or a studio that only started.
    fn cameras_wanted(&self) -> bool {
        cameras_wanted(
            self.viewers.len(),
            self.sources.values().map(|(source, _)| source),
        )
    }

    /// Whether the cameras held are the raised watch's reading — raised, and
    /// answered. Before its first answer the list is empty, not read.
    #[must_use]
    pub fn knows_the_cameras(&self) -> bool {
        self.camera_watch.is_some() && self.cameras_heard
    }

    /// RAISES THE CAMERA WATCH while a camera is wanted, and lets go of it —
    /// and of the list — when none is: read after every change of the
    /// sources, by the loop.
    ///
    /// Kept for the studio's whole life, the watch read every camera again at
    /// every plug and every change of a node's permissions — a camera's light
    /// blinking with nobody having asked for one.
    fn follow_cameras(&mut self, shared: &Shared) {
        match (self.cameras_wanted(), self.camera_watch.is_some()) {
            (true, false) => {
                self.camera_epoch += 1;
                self.cameras_heard = false;
                let epoch = self.camera_epoch;
                let shared = Arc::clone(shared);
                self.camera_watch = Some(ronoctua_camera::watch::watch(move |cameras| {
                    let (lock, bell) = &*shared;
                    if let Ok(mut e) = lock.lock() {
                        e.cameras_read(epoch, cameras);
                    }
                    bell.notify_all();
                }));
            }
            (false, true) => {
                self.camera_watch = None;
                self.cameras_heard = false;
                self.cameras.clear();
                self.dirty = true;
            }
            _ => {}
        }
    }

    /// A reading of the watch raised as `epoch`, taken while that watch is
    /// still the one raised: one that finished after its watch was let go
    /// is about cameras nobody asks for any more.
    fn cameras_read(&mut self, epoch: u64, cameras: Vec<ronoctua_camera::Camera>) {
        if self.camera_watch.is_some() && epoch == self.camera_epoch {
            self.cameras = cameras;
            self.cameras_heard = true;
            self.dirty = true;
            self.broadcast();
        }
    }

    /// [`Engine::ask`], for a gesture about a camera ([`Ask::reads_the_cameras`])
    /// made while no watch's reading is held: the cameras it was given, read
    /// once for it, are the list it reads — and are let go after it, unless
    /// the watch answered meanwhile, so the state never keeps a camera nobody
    /// looks at.
    pub fn ask_having_read(
        &mut self,
        ask: Ask,
        read: Option<Vec<ronoctua_camera::Camera>>,
        shared: &Shared,
    ) -> Say {
        let once = match read {
            Some(cameras) if !self.knows_the_cameras() => {
                self.cameras = cameras;
                true
            }
            _ => false,
        };
        let say = self.ask(ask, shared);
        if once && !self.knows_the_cameras() {
            self.cameras.clear();
        }
        say
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
            Ask::Watch | Ask::Look | Ask::Preview | Ask::Hands => {}
            Ask::Photo => {
                if self.prefs.countdown > 0 {
                    self.countdown =
                        Some(Instant::now() + Duration::from_secs(u64::from(self.prefs.countdown)));
                } else {
                    self.photo_due = true;
                }
            }
            // REHEARSED: the clock rolls on screen, nothing is opened or
            // written.
            Ask::Record { on } if self.rehearsing => {
                let on = on.unwrap_or(self.rehearsed_recording.is_none());
                self.rehearsed_recording = on.then(epoch_now);
            }
            Ask::Stream { on } if self.rehearsing => {
                let on = on.unwrap_or(self.rehearsed_stream.is_none());
                self.rehearsed_stream = on.then(|| {
                    (
                        epoch_now(),
                        self.prefs.stream_url.clone().unwrap_or_default(),
                    )
                });
            }
            Ask::Virtual { .. } if self.rehearsing => return Say::Refused(Trouble::Rehearsal),
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

/// Whether the cameras are to be read, given how many viewers the program
/// has and the sources open: a viewer is the camera app, and a camera source
/// is a light on ([`Engine::follow_cameras`]).
fn cameras_wanted<'a>(viewers: usize, mut open: impl Iterator<Item = &'a Source>) -> bool {
    viewers > 0 || open.any(|s| matches!(s, Source::Camera { .. }))
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
                e.follow_cameras(shared);
                if e.dirty {
                    e.broadcast();
                }
                let _ = bell.wait_timeout(e, Duration::from_millis(500));
                continue;
            }
            e.sync_sources();
            e.follow_cameras(shared);
            if !e.composing() {
                // The hands alone: their thread reads the source, and there
                // is no program to draw.
                if e.dirty {
                    e.broadcast();
                }
                let _ = bell.wait_timeout(e, Duration::from_millis(500));
                continue;
            }
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
            if e.photo_due && e.rehearsing {
                // THE COUNTDOWN RAN, and no picture is written.
                e.photo_due = false;
                e.dirty = true;
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
    serve_one_reading(stream, shared, ronoctua_camera::scan);
}

/// [`serve_one`], with `read` the reading of the cameras a gesture at rest
/// takes — a test's, which opens nothing.
fn serve_one_reading(
    stream: UnixStream,
    shared: &Shared,
    read: impl FnOnce() -> Vec<ronoctua_camera::Camera>,
) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut input = std::io::BufReader::new(read_half);
    let ask: Ask = match wire::read_line(&mut input) {
        Ok(Some(a)) => a,
        _ => return,
    };
    let (lock, bell) = &**shared;
    // A GESTURE ABOUT A CAMERA with none in use reads them ONCE, for it —
    // off the lock: a reading opens every node and takes its time, and the
    // loop must not wait on it. With a camera in use, the watch holds them
    // (once it answered: until then this reads them too).
    //
    // AND IT IS ANSWERED BEFORE THE READING. The door that asked waits for
    // the answer on its own loop, and gives up after two seconds saying the
    // studio is not running — while a reading of every node can take
    // longer, the footer frozen meanwhile. These asks refuse nothing
    // (`Ask::reads_the_cameras`), so `done` is the answer the reading would
    // have given; what the gesture changes reaches every door as the state.
    if ask.reads_the_cameras() && lock.lock().is_ok_and(|e| !e.knows_the_cameras()) {
        let mut out = stream;
        let _ = wire::write_line(&mut out, &Say::Done);
        drop(out);
        let cameras = read();
        let Ok(mut e) = lock.lock() else { return };
        let said = e.ask_having_read(ask, Some(cameras), shared);
        if said != Say::Done {
            tracing::error!(
                ?said,
                "a gesture about a camera, answered done before its reading, said otherwise once read"
            );
        }
        e.broadcast();
        drop(e);
        bell.notify_all();
        return;
    }
    let Ok(mut e) = lock.lock() else { return };
    match ask {
        Ask::Watch => e.add_watcher(stream),
        Ask::Hands => e.add_hand_reader(stream, shared),
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
    fn a_rehearsing_studio_rolls_on_screen_and_opens_nothing() {
        // THE DEVELOPMENT LOOP'S STUDIO: record and stream roll their clocks
        // in the state, no encoder starts, and the virtual camera is refused
        // with the sentence that says why.
        let mut e = Engine {
            rehearsing: true,
            ..Engine::default()
        };
        let shared: Shared = Arc::new((Mutex::new(Engine::default()), Condvar::new()));
        let _ = e.ask(Ask::Record { on: Some(true) }, &shared);
        assert!(e.recording.is_none(), "an encoder started in a rehearsal");
        let rolling = e.state().recording.expect("the clock rolls");
        assert!(rolling.target.is_empty(), "a rehearsal names no file");
        let _ = e.ask(Ask::Record { on: Some(false) }, &shared);
        assert!(e.state().recording.is_none());
        assert_eq!(
            e.ask(Ask::Virtual { on: Some(true) }, &shared),
            Say::Refused(Trouble::Rehearsal)
        );
        assert!(e.virtual_cam.is_none());
    }

    /// A camera as a driver would report it, with nothing opened.
    fn planted(card: &str, interface: &str) -> ronoctua_camera::Camera {
        ronoctua_camera::Camera {
            identity: ronoctua_camera::Identity::of(card, "usb-1", interface),
            node: format!("/dev/video-{interface}").into(),
            card: card.into(),
            driver: "uvcvideo".into(),
            bus: "usb-1".into(),
            kind: ronoctua_camera::Kind::Colour,
            formats: Vec::new(),
            controls: Vec::new(),
            streamable: true,
        }
    }

    /// FAILS IF: anything but a camera in use or the camera app asks for the
    /// cameras to be read — a screen recorded alone, a door watching the
    /// state or a studio that only started would blink the light.
    #[test]
    fn only_a_camera_in_use_or_the_camera_app_reads_the_cameras() {
        let camera = Source::Camera { identity: None };
        let screen = Source::Screen { output: None };
        assert!(!cameras_wanted(0, std::iter::empty()));
        assert!(!cameras_wanted(0, [&screen].into_iter()));
        assert!(cameras_wanted(0, [&screen, &camera].into_iter()));
        assert!(cameras_wanted(1, [&screen].into_iter()), "the camera app");
        // A STUDIO THAT ONLY STARTED, followed by its loop: no watch, no
        // camera in its state.
        let shared: Shared = Arc::new((Mutex::new(Engine::default()), Condvar::new()));
        let mut e = Engine::default();
        e.follow_cameras(&shared);
        assert!(e.camera_watch.is_none(), "an idle studio raised the watch");
        assert!(e.state().cameras.is_empty());
    }

    /// FAILS IF: a gesture about a camera made with none in use does not
    /// act on the cameras read for it, or the state keeps them after it —
    /// a list nobody looks at, gone stale at the next unplug.
    #[test]
    fn a_camera_gesture_at_rest_reads_once_and_keeps_nothing() {
        let shared: Shared = Arc::new((Mutex::new(Engine::default()), Condvar::new()));
        let mut e = Engine {
            book: crate::scene::Book::default(),
            scene: crate::scene::CAMERA.into(),
            ..Engine::default()
        };
        let (first, second) = (planted("Front", "1.0"), planted("Back", "2.0"));
        let turned_to = second.identity.0.clone();
        let read = vec![first, second];
        assert!(Ask::CameraTurn { by: 1 }.reads_the_cameras());
        assert!(!Ask::Look.reads_the_cameras());
        assert_eq!(
            e.ask_having_read(Ask::CameraTurn { by: 1 }, Some(read), &shared),
            Say::Done
        );
        let layer = e.book.camera_layer(crate::scene::CAMERA).expect("a layer");
        assert_eq!(
            e.book
                .scene(crate::scene::CAMERA)
                .expect("the scene")
                .layers[layer]
                .source,
            Source::Camera {
                identity: Some(turned_to)
            },
            "the turn did not walk the cameras read for it"
        );
        assert!(e.cameras.is_empty(), "the cameras read once were kept");
        assert!(e.camera_watch.is_none());
    }

    /// FAILS IF: a gesture about a camera made with none in use is answered
    /// only once the cameras are read — the door that asked waits on its
    /// loop, the footer frozen, and past two seconds says the studio is not
    /// running though the gesture happened — or it is not carried out on the
    /// reading when that comes.
    #[test]
    fn a_camera_gesture_at_rest_is_answered_before_its_reading() {
        use std::io::BufReader;
        use std::sync::mpsc;
        let shared: Shared = Arc::new((
            Mutex::new(Engine {
                book: crate::scene::Book::default(),
                scene: crate::scene::CAMERA.into(),
                ..Engine::default()
            }),
            Condvar::new(),
        ));
        let (first, second) = (planted("Front", "1.0"), planted("Back", "2.0"));
        let turned_to = second.identity.0.clone();
        let (studio_end, asker) = UnixStream::pair().expect("a socket pair");
        asker
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a timeout");
        wire::write_line(&mut &asker, &Ask::CameraTurn { by: 1 }).expect("the ask");
        // THE READING HELD until the answer is in: a camera slow to answer.
        let (release, held) = mpsc::channel::<()>();
        let serving = Arc::clone(&shared);
        let served = std::thread::spawn(move || {
            serve_one_reading(studio_end, &serving, move || {
                let _ = held.recv_timeout(Duration::from_secs(10));
                vec![first, second]
            });
        });
        let said: Option<Say> =
            wire::read_line(&mut BufReader::new(&asker)).expect("an answer within the timeout");
        assert_eq!(said, Some(Say::Done));
        assert!(
            !served.is_finished(),
            "the reading was over before the answer: this proves nothing"
        );
        let _ = release.send(());
        served.join().expect("the serving thread");
        let (lock, _) = &*shared;
        let e = lock.lock().expect("the engine");
        let layer = e.book.camera_layer(crate::scene::CAMERA).expect("a layer");
        assert_eq!(
            e.book
                .scene(crate::scene::CAMERA)
                .expect("the scene")
                .layers[layer]
                .source,
            Source::Camera {
                identity: Some(turned_to)
            },
            "the turn was not carried out on the reading that came after"
        );
        assert!(e.cameras.is_empty(), "the cameras read once were kept");
    }

    /// FAILS IF: an ask that reads the cameras can say anything but `done` —
    /// the studio answers it before the reading (`serve_one`), and a refusal
    /// would never reach the door that asked.
    #[test]
    fn the_asks_that_read_the_cameras_refuse_nothing() {
        let shared: Shared = Arc::new((Mutex::new(Engine::default()), Condvar::new()));
        let asks = || {
            [
                Ask::CameraTurn { by: 1 },
                Ask::CameraTurn { by: -1 },
                Ask::Camera {
                    identity: "nobody|usb-9|9.0".into(),
                },
                // Turned with no camera known and none named: nothing is
                // written (`camera.json` is the person's).
                Ask::Mirror,
            ]
        };
        for scene in [crate::scene::CAMERA, "screen"] {
            for read in [None, Some(Vec::new())] {
                for ask in asks() {
                    assert!(ask.reads_the_cameras(), "{ask:?}");
                    let mut e = Engine {
                        book: crate::scene::Book::default(),
                        scene: scene.into(),
                        ..Engine::default()
                    };
                    assert_eq!(
                        e.ask_having_read(ask.clone(), read.clone(), &shared),
                        Say::Done,
                        "{ask:?} in `{scene}`"
                    );
                }
            }
        }
    }

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
