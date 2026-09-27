//! The camera's settings — one view, two doors (`LEADER s c c`, `LEADER s c w`).
//!
//! # What is here
//!
//! Which rows exist, what a key does on each, and what the door must do about
//! it. Nothing here opens a camera or a socket: the cameras come from
//! `ronoctua-camera` (the one V4L2 boundary) and the rest from
//! `ronoctua-studio` (the daemon's socket and its two files). It is the shape
//! `pointers.rs` has, down to the rings.
//!
//! # Every value is a ring
//!
//! `←`/`→` turn the value on the row and `RET` turns it forward, because
//! `RET` is what a click becomes (invariant 19). A control a camera does not
//! have is not drawn: the list of controls is the driver's own answer to
//! `VIDIOC_QUERYCTRL`, so a screen for this camera shows this camera's knobs —
//! on any camera, not the one it was written on.
//!
//! # What is typed, is typed in either door
//!
//! A stream address, its key, a scene's new name and a script: the row's key
//! is a [`Question`] (`Errand::Ask`), and the door asks it where the view is
//! open — the footer on its minibuffer, a window on its own line
//! (`door::Ui::ask`) — with the view still on screen and the cursor on its
//! row. Both answer into [`answered`], so a name, a script or a key typed in
//! one door does exactly what it does in the other. THE KEY IS A SECRET:
//! masked on the line, left out of the log, and written by the process that
//! typed it to its `0600` file — it never goes down a socket or into an
//! `argv`.

use std::path::PathBuf;

use crate::files::{Field, Hit as FilesHit, Ink};
use ronoctua_camera::{Camera, ControlKind, Kind, Mode};
use ronoctua_studio::{
    prefs::{self, Container, Encoder, Mark, Preferences, Sends},
    wire::{Ask, State},
};
use ronoctua_tokens as tokens;

/// The keys this view teaches, and where each one applies.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "camera-settings-down", Where::Anywhere),
    ("<up>", "camera-settings-up", Where::Anywhere),
    ("<left>", "camera-settings-less", Where::OnAValue),
    ("<right>", "camera-settings-more", Where::OnAValue),
    ("RET", "camera-settings-enter", Where::OnAValue),
    ("d", "camera-settings-remove", Where::OnRemovable),
];

/// Where a key applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row that turns or opens.
    OnAValue,
    /// On a scene or a script.
    OnRemovable,
}

impl Where {
    fn shown(self, target: &Target) -> bool {
        match self {
            Self::Anywhere => true,
            Self::OnAValue => target.turns(),
            Self::OnRemovable => matches!(target, Target::Scene(_) | Target::Script(_)),
        }
    }
}

/// A key, read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up a row.
    Up,
    /// Down a row.
    Down,
    /// The previous value.
    Less,
    /// The next value.
    More,
    /// A click: the next value, or the row's door.
    Enter,
    /// Remove a scene or a script.
    Remove,
    /// Out.
    Escape,
}

/// What a row is.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// Nothing.
    Nothing,
    /// The default camera.
    DefaultCamera,
    /// A camera's mode (`None`: chosen for it).
    Mode(String),
    /// A camera's mirror.
    Mirror(String),
    /// A camera's control, by key.
    Control(String, String),
    /// Every control of a camera back to the factory.
    Factory(String),
    /// The camera layer's effect.
    Effect,
    /// One of its parameters (`true`: of the background step).
    Param(String, bool),
    /// The background mode.
    Background,
    /// The replaced background's picture.
    Backdrop,
    /// The virtual camera now.
    Virtual,
    /// At every start.
    VirtualAtStart,
    /// What it sends.
    VirtualSends,
    /// Program size.
    Size,
    /// Program rate.
    Rate,
    /// The container.
    Container,
    /// The encoder.
    Encoder,
    /// Quality.
    Quality,
    /// A photo's countdown.
    Countdown,
    /// The pointer in screen captures.
    Cursor,
    /// Microphone on.
    Microphone,
    /// Its gain.
    MicrophoneGain,
    /// Desktop sound on.
    Desktop,
    /// Its gain.
    DesktopGain,
    /// A scene: on air on `RET`.
    Scene(String),
    /// A new scene: asks its name.
    SceneAdd,
    /// Rename the scene on air: asks the name.
    SceneRename,
    /// A script: run or stop on `RET`.
    Script(String),
    /// A new script: asks it.
    ScriptAdd,
    /// The stream's address: asks it.
    StreamUrl,
    /// The stream's key: asks it, never shows it.
    StreamKey,
    /// The stream on or off.
    Stream,
    /// When the modeline shows the camera mark.
    Mark,
    /// The door to the camera app.
    OpenApp,
    /// Every setting of this screen back as the distribution ships it —
    /// pressed twice.
    Defaults,
}

impl Target {
    const fn turns(&self) -> bool {
        !matches!(self, Self::Nothing)
    }
}

/// One drawn line.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Placed and cut.
    pub fields: Vec<Field>,
    /// The cursor is on it.
    pub focused: bool,
    /// The cursor skips it.
    pub heading: bool,
    /// What a key does here.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: String,
}

/// A question the view asks: the command that asks it — whose `ask-<id>` is
/// the sentence, and under whose id the answer comes back — and whether what
/// is typed is hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Question {
    /// The command, and the id the answer comes back under.
    pub id: &'static str,
    /// Masked on the line and left out of the log: the stream's key.
    pub secret: bool,
}

/// A new scene's name.
pub const SCENE_ADD: &str = "camera-scene-add";
/// The scene on air's new name.
pub const SCENE_RENAME: &str = "camera-scene-rename";
/// A new script, as it is typed (`studio::scene::parse_script`).
pub const SCRIPT_ADD: &str = "camera-script-add";
/// The stream's address.
pub const STREAM_URL: &str = "camera-stream-url";
/// The stream's key — a secret.
pub const STREAM_KEY: &str = "camera-stream-key";

/// Every question this view asks — the commands `M-x` reaches them by, too.
pub const QUESTIONS: [Question; 5] = [
    Question {
        id: SCENE_ADD,
        secret: false,
    },
    Question {
        id: SCENE_RENAME,
        secret: false,
    },
    Question {
        id: SCRIPT_ADD,
        secret: false,
    },
    Question {
        id: STREAM_URL,
        secret: false,
    },
    Question {
        id: STREAM_KEY,
        secret: true,
    },
];

/// The question a command asks, when it is one of this view's.
#[must_use]
pub fn question(id: &str) -> Option<Question> {
    QUESTIONS.into_iter().find(|q| q.id == id)
}

/// The stream's key, as typed: its `Debug` never prints it, so no log line
/// and no test failure carries it.
#[derive(Clone, PartialEq, Eq)]
pub struct StreamKey(String);

impl std::fmt::Debug for StreamKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StreamKey(•••)")
    }
}

/// What the door has to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Draw again.
    Nothing,
    /// Put this on the clipboard.
    Copy(String),
    /// Shut the view.
    Close,
    /// Run a command of the registry — the camera app's door.
    Run(&'static str),
    /// Ask this, where the view is open, and keep the view on screen: the
    /// answer comes back into [`answered`].
    Ask(Question),
    /// Write the stream's address, and tell the studio to reread it.
    StreamUrl(String),
    /// Write the stream's key to its `0600` file.
    StreamKey(StreamKey),
    /// Add the script as it was typed — read against the scene book when it
    /// is carried out, and refused with a sentence when it will not parse.
    Script(String),
    /// Tell the studio.
    Tell(Ask),
    /// Write the studio's settings, and tell it to reread them.
    Studio(Box<Preferences>),
    /// Write the cameras' file, and tell the studio to reread it.
    Cameras(ronoctua_camera::prefs::Preferences),
    /// Every setting back as it came (see [`Target::Defaults`]).
    Defaults,
    /// Set a camera's control now, and remember it.
    Control {
        /// The camera.
        identity: String,
        /// The control's key.
        key: String,
        /// Its new value.
        value: i32,
    },
}

/// Everything a row reads.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Every camera, as the watch last read them.
    pub cameras: Vec<Camera>,
    /// `camera.json`.
    pub camera_prefs: ronoctua_camera::prefs::Preferences,
    /// `studio.json`.
    pub prefs: Preferences,
    /// The daemon's state, when it answered.
    pub state: Option<State>,
    /// Whether a stream key is set.
    pub key_set: bool,
    /// The pictures a background can be replaced with.
    pub backdrops: Vec<PathBuf>,
}

/// How many pictures the backdrop ring offers.
const BACKDROPS_MOST: usize = 60;

impl Snapshot {
    /// Reads the two files and the pictures directory; the cameras and the
    /// state come from the threads.
    #[must_use]
    pub fn read(cameras: Vec<Camera>, state: Option<State>) -> Self {
        let prefs = prefs::store().read();
        let dir = prefs.pictures_dir();
        let mut backdrops: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|d| {
                d.flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension().is_some_and(|x| {
                            matches!(
                                x.to_string_lossy().to_lowercase().as_str(),
                                "png" | "jpg" | "jpeg" | "webp"
                            )
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        backdrops.sort();
        backdrops.truncate(BACKDROPS_MOST);
        Self {
            cameras,
            camera_prefs: ronoctua_camera::prefs::store().read(),
            prefs,
            state,
            key_set: prefs::has_stream_key(),
            backdrops,
        }
    }

    fn camera(&self, identity: &str) -> Option<&Camera> {
        self.cameras.iter().find(|c| c.identity.0 == identity)
    }
}

/// Where the rows start and how wide they are.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    crate::power::extent(columns)
}

/// The view.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    moved: bool,
    /// The reset row was pressed once: the next press there carries it out,
    /// and any other press, or the cursor leaving, takes it back.
    armed: bool,
}

/// The rings, in the order `→` walks them.
const QUALITIES: [u8; 5] = [18, 20, 23, 26, 28];
const COUNTDOWNS: [u8; 4] = [0, 3, 5, 10];
const GAINS: [u8; 9] = [25, 50, 75, 100, 125, 150, 175, 200, 0];

fn ring<T: PartialEq + Clone>(all: &[T], now: &T, forward: bool) -> T {
    let n = all.len();
    let at = all.iter().position(|x| x == now).unwrap_or(0);
    all[if forward {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    }]
    .clone()
}

/// The modes a camera offers, and "chosen for it" first.
fn modes_of(c: &Camera) -> Vec<Option<Mode>> {
    let mut out = vec![None];
    for f in c
        .formats
        .iter()
        .filter(|f| ronoctua_camera::input_format(&f.fourcc).is_some())
    {
        for s in &f.sizes {
            for i in &s.intervals {
                out.push(Some(Mode {
                    fourcc: f.fourcc.clone(),
                    width: s.width,
                    height: s.height,
                    interval: *i,
                }));
            }
        }
    }
    out
}

impl View {
    /// A view with the cursor on the first row that does something.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            cursor: 1,
            moved: false,
            armed: false,
        }
    }

    /// Which row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The hint line.
    #[must_use]
    pub fn hint(
        &self,
        snap: &Snapshot,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let rows = self.rows(snap, columns, texts);
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.clone());
        let here: Vec<(&str, &str)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(&target))
            .map(|(k, id, _)| (*k, *id))
            .collect();
        crate::chrome::hint_of(&here, texts)
    }

    /// A click; `true` when it was the second on the same row.
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        if rows.get(n).is_some_and(|r| !r.heading) {
            let again = self.cursor == n;
            if !again {
                self.armed = false;
            }
            self.moved = true;
            self.cursor = n;
            return again;
        }
        false
    }

    /// The cursor back on a real row after the rows changed.
    pub fn settle_in(&mut self, snap: &Snapshot, columns: u16, texts: &ronoctua_i18n::Texts) {
        let rows = self.rows(snap, columns, texts);
        self.clamp(&rows);
        if !self.moved
            && rows.get(self.cursor).is_some_and(|r| !r.target.turns())
            && let Some(i) = rows.iter().position(|r| !r.heading && r.target.turns())
        {
            self.cursor = i;
        }
    }

    /// The cursor stays on its row by identity.
    pub fn follow(
        &mut self,
        spot: &str,
        snap: &Snapshot,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(snap, columns, texts);
        if let Some(i) = rows.iter().position(|r| r.spot == spot) {
            self.cursor = i;
        }
        self.clamp(&rows);
    }

    fn clamp(&mut self, rows: &[Row]) {
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        self.cursor = self.cursor.min(rows.len() - 1);
        if rows[self.cursor].heading {
            let after = (self.cursor..rows.len()).find(|&i| !rows[i].heading);
            let before = (0..=self.cursor).rev().find(|&i| !rows[i].heading);
            if let Some(i) = after.or(before) {
                self.cursor = i;
            }
        }
    }

    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        loop {
            if down {
                if at + 1 >= rows.len() {
                    return;
                }
                at += 1;
            } else {
                if at == 0 {
                    return;
                }
                at -= 1;
            }
            if !rows[at].heading {
                self.cursor = at;
                return;
            }
        }
    }

    /// A key.
    pub fn key(
        &mut self,
        press: Press,
        snap: &Snapshot,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.settle_in(snap, columns, texts);
        let rows = self.rows(snap, columns, texts);
        if rows.is_empty() {
            return if press == Press::Escape {
                Errand::Close
            } else {
                Errand::Nothing
            };
        }
        let target = rows[self.cursor].target.clone();
        // THE RESET ASKS TWICE. It throws away every choice on this screen,
        // and the first press is one a click or a stray `RET` can make; it only
        // ARMS the row, which then says so. Anything else takes it back.
        let was_armed = std::mem::take(&mut self.armed);
        if target == Target::Defaults && press == Press::Enter {
            if was_armed {
                return Errand::Defaults;
            }
            self.armed = true;
            return Errand::Nothing;
        }
        match press {
            Press::Escape => return Errand::Close,
            Press::Up | Press::Down => {
                self.moved = true;
                self.walk(&rows, press == Press::Down);
                return Errand::Nothing;
            }
            _ => {}
        }
        if press == Press::Remove {
            return match target {
                Target::Scene(name) => Errand::Tell(Ask::SceneRemove { name }),
                Target::Script(name) => Errand::Tell(Ask::ScriptRemove { name }),
                _ => Errand::Nothing,
            };
        }
        let forward = press != Press::Less;
        let ask = |id: &str| question(id).map_or(Errand::Nothing, Errand::Ask);
        let mut p = snap.prefs.clone();
        let studio = |p: Preferences| Errand::Studio(Box::new(p));
        let state = snap.state.as_ref();
        match target {
            Target::Nothing => rows
                .get(self.cursor)
                .and_then(|r| crate::chrome::information_of(&r.fields))
                .map_or(Errand::Nothing, Errand::Copy),
            Target::DefaultCamera => {
                let ids: Vec<Option<ronoctua_camera::Identity>> = std::iter::once(None)
                    .chain(
                        snap.cameras
                            .iter()
                            .filter(|c| c.streamable && c.kind != Kind::Virtual)
                            .map(|c| Some(c.identity.clone())),
                    )
                    .collect();
                let mut cp = snap.camera_prefs.clone();
                cp.default = ring(&ids, &cp.default, forward);
                Errand::Cameras(cp)
            }
            Target::Mode(id) => {
                let Some(c) = snap.camera(&id) else {
                    return Errand::Nothing;
                };
                let ident = c.identity.clone();
                let mut cp = snap.camera_prefs.clone();
                let mut per = cp.of(&ident);
                per.mode = ring(&modes_of(c), &per.mode, forward);
                cp.set(&ident, per);
                Errand::Cameras(cp)
            }
            Target::Mirror(id) => {
                let ident = ronoctua_camera::Identity(id);
                let mut cp = snap.camera_prefs.clone();
                let mut per = cp.of(&ident);
                per.mirror = !per.mirror;
                cp.set(&ident, per);
                Errand::Cameras(cp)
            }
            Target::Control(id, key) => {
                let Some(c) = snap
                    .camera(&id)
                    .and_then(|c| c.controls.iter().find(|k| k.key == key))
                else {
                    return Errand::Nothing;
                };
                let value = match c.kind {
                    ControlKind::Boolean => i32::from(c.value == 0),
                    ControlKind::Menu | ControlKind::IntegerMenu => {
                        let values: Vec<i32> = c.menu.iter().map(|(v, _)| *v).collect();
                        if values.is_empty() {
                            c.value
                        } else {
                            ring(&values, &c.value, forward)
                        }
                    }
                    ControlKind::Integer => {
                        // A twentieth of the range per press, in the driver's own steps:
                        // nobody walks a 0..255 brightness one unit at a time.
                        let stride = ((c.max - c.min) / 20).max(c.step).max(1);
                        let stride = stride - stride % c.step.max(1);
                        (c.value + if forward { stride } else { -stride }).clamp(c.min, c.max)
                    }
                };
                Errand::Control {
                    identity: id,
                    key,
                    value,
                }
            }
            Target::Factory(id) => {
                let ident = ronoctua_camera::Identity(id);
                let mut cp = snap.camera_prefs.clone();
                let mut per = cp.of(&ident);
                per.controls.clear();
                // Every control to its default NOW, too: the file only says what
                // to put back on the next plug.
                if let Some(c) = snap.cameras.iter().find(|c| c.identity == ident) {
                    for k in c
                        .controls
                        .iter()
                        .filter(|k| !k.read_only && k.value != k.default)
                    {
                        let _ = ronoctua_camera::set_control(c, &k.key, k.default);
                    }
                }
                cp.set(&ident, per);
                Errand::Cameras(cp)
            }
            Target::Effect => Errand::Tell(Ask::EffectTurn {
                by: if forward { 1 } else { -1 },
            }),
            Target::Param(name, background) => Errand::Tell(Ask::EffectParam {
                name,
                by: if forward { 1 } else { -1 },
                background,
            }),
            Target::Background => Errand::Tell(Ask::BackgroundTurn {
                by: if forward { 1 } else { -1 },
            }),
            Target::Backdrop => {
                let all: Vec<Option<PathBuf>> = std::iter::once(None)
                    .chain(snap.backdrops.iter().cloned().map(Some))
                    .collect();
                p.backdrop = ring(&all, &p.backdrop, forward);
                studio(p)
            }
            Target::Virtual => Errand::Tell(Ask::Virtual {
                on: Some(state.is_none_or(|s| s.virtual_node.is_none())),
            }),
            Target::VirtualAtStart => {
                p.virtual_at_start = !p.virtual_at_start;
                studio(p)
            }
            Target::VirtualSends => {
                p.virtual_sends = ring(&[Sends::Program, Sends::Camera], &p.virtual_sends, forward);
                studio(p)
            }
            Target::Size => {
                let (w, h) = ring(&prefs::SIZES, &p.size(), forward);
                p.width = w;
                p.height = h;
                studio(p)
            }
            Target::Rate => {
                p.fps = ring(&prefs::RATES, &p.rate(), forward);
                studio(p)
            }
            Target::Container => {
                p.container = ring(&[Container::Mp4, Container::Mkv], &p.container, forward);
                studio(p)
            }
            Target::Encoder => {
                p.encoder = ring(
                    &[Encoder::Auto, Encoder::Hardware, Encoder::Software],
                    &p.encoder,
                    forward,
                );
                studio(p)
            }
            Target::Quality => {
                p.quality = ring(&QUALITIES, &p.quality, forward);
                studio(p)
            }
            Target::Countdown => {
                p.countdown = ring(&COUNTDOWNS, &p.countdown, forward);
                studio(p)
            }
            Target::Cursor => {
                p.cursor = !p.cursor;
                studio(p)
            }
            Target::Microphone => Errand::Tell(Ask::Sound {
                which: ronoctua_studio::wire::Sound::Microphone,
                gain: None,
                muted: Some(p.microphone),
            }),
            Target::Desktop => Errand::Tell(Ask::Sound {
                which: ronoctua_studio::wire::Sound::Desktop,
                gain: None,
                muted: Some(p.desktop),
            }),
            Target::MicrophoneGain => Errand::Tell(Ask::Sound {
                which: ronoctua_studio::wire::Sound::Microphone,
                gain: Some(ring(&GAINS, &p.microphone_gain, forward)),
                muted: None,
            }),
            Target::DesktopGain => Errand::Tell(Ask::Sound {
                which: ronoctua_studio::wire::Sound::Desktop,
                gain: Some(ring(&GAINS, &p.desktop_gain, forward)),
                muted: None,
            }),
            Target::Scene(name) => Errand::Tell(Ask::Scene { name }),
            Target::SceneAdd => ask(SCENE_ADD),
            Target::SceneRename => ask(SCENE_RENAME),
            Target::Script(name) => {
                let running = state
                    .and_then(|s| s.running.as_ref())
                    .is_some_and(|r| r.script == name);
                Errand::Tell(Ask::Script {
                    name: (!running).then_some(name),
                })
            }
            Target::ScriptAdd => ask(SCRIPT_ADD),
            Target::StreamUrl => ask(STREAM_URL),
            Target::StreamKey => ask(STREAM_KEY),
            Target::Stream => Errand::Tell(Ask::Stream {
                on: Some(state.is_none_or(|s| s.streaming.is_none())),
            }),
            Target::Mark => {
                p.mark = ring(&[Mark::WhenInUse, Mark::Never], &p.mark, forward);
                studio(p)
            }
            Target::OpenApp => Errand::Run("camera-open"),
            // `←`/`→` on it: nothing to turn.
            Target::Defaults => Errand::Nothing,
        }
    }

    /// Every row, in reading order.
    #[must_use]
    pub fn rows(&self, snap: &Snapshot, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        use ronoctua_i18n::key;
        let (left, room) = extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
        };
        cameras(&mut b, snap);
        effects(&mut b, snap);
        // THE VIRTUAL CAMERA.
        let p = &snap.prefs;
        let state = snap.state.as_ref();
        b.heading(key::CAMERA_HEADING_VIRTUAL);
        b.note(key::CAMERA_ABOUT_VIRTUAL);
        let node = state.and_then(|s| s.virtual_node.clone());
        b.pair(
            b.t(key::CAMERA_VIRTUAL_NOW),
            node.unwrap_or_else(|| on_off(false, texts)),
            Target::Virtual,
            "virtual",
        );
        b.pair(
            b.t(key::CAMERA_VIRTUAL_AT_START),
            on_off(p.virtual_at_start, texts),
            Target::VirtualAtStart,
            "virtual-start",
        );
        b.pair(
            b.t(key::CAMERA_VIRTUAL_SENDS),
            b.t(match p.virtual_sends {
                Sends::Program => key::CAMERA_SENDS_PROGRAM,
                Sends::Camera => key::CAMERA_SENDS_CAMERA,
            }),
            Target::VirtualSends,
            "virtual-sends",
        );
        // CAPTURE.
        b.heading(key::CAMERA_HEADING_CAPTURE);
        b.note(key::CAMERA_ABOUT_CAPTURE);
        let (w, h) = p.size();
        b.pair(
            b.t(key::CAMERA_SIZE),
            format!("{w} × {h}"),
            Target::Size,
            "size",
        );
        b.pair(
            b.t(key::CAMERA_RATE),
            texts.t_with(key::CAMERA_FPS, &[("fps", &p.rate().to_string())]),
            Target::Rate,
            "rate",
        );
        b.pair(
            b.t(key::CAMERA_CONTAINER),
            b.t(match p.container {
                Container::Mp4 => key::CAMERA_CONTAINER_MP4,
                Container::Mkv => key::CAMERA_CONTAINER_MKV,
            }),
            Target::Container,
            "container",
        );
        b.pair(
            b.t(key::CAMERA_ENCODER),
            b.t(match p.encoder {
                Encoder::Auto => key::CAMERA_ENCODER_AUTO,
                Encoder::Hardware => key::CAMERA_ENCODER_HARDWARE,
                Encoder::Software => key::CAMERA_ENCODER_SOFTWARE,
            }),
            Target::Encoder,
            "encoder",
        );
        b.pair(
            b.t(key::CAMERA_QUALITY),
            p.quality.to_string(),
            Target::Quality,
            "quality",
        );
        b.pair(
            b.t(key::CAMERA_COUNTDOWN),
            if p.countdown == 0 {
                b.t(key::CAMERA_AT_ONCE)
            } else {
                texts.t_with(
                    key::CAMERA_SECONDS,
                    &[("seconds", &p.countdown.to_string())],
                )
            },
            Target::Countdown,
            "countdown",
        );
        b.pair(
            b.t(key::CAMERA_CURSOR),
            on_off(p.cursor, texts),
            Target::Cursor,
            "cursor",
        );
        b.pair(
            b.t(key::CAMERA_PICTURES),
            p.pictures_dir().to_string_lossy().into_owned(),
            Target::Nothing,
            "pictures",
        );
        b.pair(
            b.t(key::CAMERA_VIDEOS),
            p.videos_dir().to_string_lossy().into_owned(),
            Target::Nothing,
            "videos",
        );
        // SOUND.
        b.heading(key::CAMERA_HEADING_SOUND);
        b.note(key::CAMERA_ABOUT_SOUND);
        b.pair(
            b.t(key::CAMERA_MICROPHONE),
            on_off(p.microphone, texts),
            Target::Microphone,
            "microphone",
        );
        b.pair(
            b.t(key::CAMERA_MICROPHONE_GAIN),
            format!("{}%", p.microphone_gain),
            Target::MicrophoneGain,
            "microphone-gain",
        );
        b.pair(
            b.t(key::CAMERA_DESKTOP),
            on_off(p.desktop, texts),
            Target::Desktop,
            "desktop",
        );
        b.pair(
            b.t(key::CAMERA_DESKTOP_GAIN),
            format!("{}%", p.desktop_gain),
            Target::DesktopGain,
            "desktop-gain",
        );
        scenes(&mut b, snap);
        // STREAMING.
        b.heading(key::CAMERA_HEADING_STREAM);
        b.note(key::CAMERA_ABOUT_STREAM);
        b.pair(
            b.t(key::CAMERA_STREAM_URL),
            p.stream_url
                .clone()
                .unwrap_or_else(|| b.t(key::CAMERA_NOT_SET)),
            Target::StreamUrl,
            "stream-url",
        );
        b.pair(
            b.t(key::CAMERA_STREAM_KEY),
            b.t(if snap.key_set {
                key::CAMERA_SET
            } else {
                key::CAMERA_NOT_SET
            }),
            Target::StreamKey,
            "stream-key",
        );
        b.pair(
            b.t(key::CAMERA_STREAM_NOW),
            on_off(state.is_some_and(|s| s.streaming.is_some()), texts),
            Target::Stream,
            "stream",
        );
        // PRIVACY.
        b.heading(key::CAMERA_HEADING_PRIVACY);
        b.note(key::CAMERA_ABOUT_PRIVACY);
        b.pair(
            b.t(key::CAMERA_MARK),
            b.t(match p.mark {
                Mark::WhenInUse => key::CAMERA_MARK_WHEN_IN_USE,
                Mark::Never => key::CAMERA_MARK_NEVER,
            }),
            Target::Mark,
            "mark",
        );
        // DEFAULTS: one row that puts everything above back.
        b.heading(key::CAMERA_HEADING_DEFAULTS);
        b.note(key::CAMERA_ABOUT_DEFAULTS);
        b.pair(
            b.t(key::CAMERA_DEFAULTS),
            if self.armed {
                b.t(key::CAMERA_DEFAULTS_CONFIRM)
            } else {
                String::new()
            },
            Target::Defaults,
            "defaults",
        );
        // THE APP, last: the door this screen would otherwise never mention.
        b.heading(key::CAMERA_HEADING_APP);
        b.pair(
            b.t(key::CAMERA_OPEN_APP),
            String::new(),
            Target::OpenApp,
            "open-app",
        );
        b.out
    }
}

/// What was typed for one of this view's questions, as the errand both doors
/// carry out — the footer's minibuffer and a window's line answer here.
///
/// NOTHING TYPED IS NEVER MIND: an empty answer changes nothing, in either
/// door. It used to ask again in the footer, which made a question that could
/// only be left with `Esc`. An id that is not one of [`QUESTIONS`] is nothing
/// too: an answer is never taken for a question nobody asked.
#[must_use]
pub fn answered(id: &str, said: &str, state: Option<&State>) -> Errand {
    let said = said.trim();
    if said.is_empty() {
        return Errand::Nothing;
    }
    match id {
        SCENE_ADD => Errand::Tell(Ask::SceneAdd {
            name: said.to_owned(),
        }),
        // THE SCENE ON AIR NOW, which is the one the row names.
        SCENE_RENAME => Errand::Tell(Ask::SceneRename {
            from: state.map(|s| s.scene.clone()).unwrap_or_default(),
            to: said.to_owned(),
        }),
        SCRIPT_ADD => Errand::Script(said.to_owned()),
        STREAM_URL => Errand::StreamUrl(said.to_owned()),
        STREAM_KEY => Errand::StreamKey(StreamKey(said.to_owned())),
        _ => Errand::Nothing,
    }
}

/// Carries out the errands that are the same through both doors — the studio,
/// the two files, a camera's control, the clipboard, what was typed — and
/// gives back what the door should say, if anything. `Run`, `Ask` and `Close`
/// are the door's own, and come back untouched as `Err`.
///
/// # Errors
///
/// The errand is one the DOOR carries out: a command to run, a question to
/// ask, or closing.
pub fn carry_out(
    errand: Errand,
    snap: &Snapshot,
    texts: &ronoctua_i18n::Texts,
) -> Result<Option<String>, Errand> {
    use ronoctua_i18n::key;
    let reload = || {
        let _ = ronoctua_studio::client::tell(&Ask::Reload);
    };
    match errand {
        Errand::Nothing => Ok(None),
        Errand::Copy(text) => Ok(Some(crate::chrome::copied(&text, texts))),
        Errand::Run(_) | Errand::Ask(_) | Errand::Close => Err(errand),
        // THE ADDRESS IS WRITTEN OVER WHAT THE FILE SAYS NOW, not over this
        // door's copy: the other door may have turned a ring since.
        Errand::StreamUrl(url) => {
            let store = prefs::store();
            let mut p = store.read();
            p.stream_url = Some(url);
            Ok(match prefs::keep(&store, &p) {
                Ok(()) => {
                    reload();
                    None
                }
                Err(e) => Some(texts.t_with(key::STUDIO_TROUBLE_WRITE, &[("why", &e.to_string())])),
            })
        }
        // THE KEY, BY THIS PROCESS, to its `0600` file — never down a socket,
        // never into an `argv` (`stream-key-is-never-an-argument`).
        Errand::StreamKey(StreamKey(secret)) => Ok(prefs::set_stream_key(&secret)
            .err()
            .map(|e| texts.t_with(key::STUDIO_TROUBLE_WRITE, &[("why", &e.to_string())]))),
        Errand::Script(text) => {
            let book = prefs::book().read();
            match ronoctua_studio::scene::parse_script(&text, &book) {
                Some(script) => carry_out(Errand::Tell(Ask::ScriptAdd { script }), snap, texts),
                None => Ok(Some(texts.t(key::CAMERA_SCRIPT_BAD))),
            }
        }
        Errand::Tell(ask) => Ok(match ronoctua_studio::client::ask(&ask) {
            Ok(ronoctua_studio::wire::Say::Refused(t)) => {
                Some(crate::camera::trouble_sentence(&t, texts))
            }
            Ok(_) => None,
            Err(_) => Some(texts.t(key::STUDIO_NOT_RUNNING)),
        }),
        Errand::Studio(p) => Ok(match prefs::keep(&prefs::store(), &p) {
            Ok(()) => {
                reload();
                None
            }
            Err(e) => Some(texts.t_with(key::STUDIO_TROUBLE_WRITE, &[("why", &e.to_string())])),
        }),
        Errand::Cameras(p) => Ok(
            match ronoctua_camera::prefs::keep(&ronoctua_camera::prefs::store(), &p) {
                Ok(()) => {
                    reload();
                    None
                }
                Err(e) => Some(texts.t_with(key::STUDIO_TROUBLE_WRITE, &[("why", &e.to_string())])),
            },
        ),
        Errand::Defaults => Ok(Some(match defaults(snap) {
            Ok(()) => {
                reload();
                texts.t(key::CAMERA_DEFAULTS_DONE)
            }
            Err(e) => texts.t_with(key::STUDIO_TROUBLE_WRITE, &[("why", &e.to_string())]),
        })),
        Errand::Control {
            identity,
            key: control,
            value,
        } => {
            let Some(camera) = snap.cameras.iter().find(|c| c.identity.0 == identity) else {
                return Ok(None);
            };
            if let Err(e) = ronoctua_camera::set_control(camera, &control, value) {
                return Ok(Some(
                    texts.t_with(key::STUDIO_TROUBLE_WRITE, &[("why", &e.to_string())]),
                ));
            }
            // REMEMBERED, so the next plug puts it back — a UVC camera forgets
            // its controls when it loses power.
            let store = ronoctua_camera::prefs::store();
            let mut p = snap.camera_prefs.clone();
            let mut per = p.of(&camera.identity);
            per.controls.insert(control, value);
            p.set(&camera.identity, per);
            Ok(ronoctua_camera::prefs::keep(&store, &p)
                .err()
                .map(|e| texts.t_with(key::STUDIO_TROUBLE_WRITE, &[("why", &e.to_string())])))
        }
    }
}

/// Everything back as it came: `studio.json` but for the stream's address,
/// `camera.json` whole, every camera's controls at their factory values NOW
/// (the file only says what to put back on the next plug), and the camera
/// layer's effect and background off.
///
/// What stays is what a person TYPED rather than chose: the scenes, the
/// stream's address and its key. The note on the screen says so.
fn defaults(snap: &Snapshot) -> anyhow::Result<()> {
    let p = Preferences {
        stream_url: snap.prefs.stream_url.clone(),
        ..Preferences::default()
    };
    prefs::keep(&prefs::store(), &p)?;
    for c in &snap.cameras {
        for k in c
            .controls
            .iter()
            .filter(|k| !k.read_only && k.value != k.default)
        {
            let _ = ronoctua_camera::set_control(c, &k.key, k.default);
        }
    }
    ronoctua_camera::prefs::keep(
        &ronoctua_camera::prefs::store(),
        &ronoctua_camera::prefs::Preferences::default(),
    )?;
    let _ = ronoctua_studio::client::ask(&Ask::Effect { name: None });
    let _ = ronoctua_studio::client::ask(&Ask::Background { name: None });
    Ok(())
}

fn cameras(b: &mut Builder<'_>, snap: &Snapshot) {
    use ronoctua_i18n::key;
    b.heading(key::CAMERA_HEADING_CAMERAS);
    b.note(key::CAMERA_ABOUT_CAMERAS);
    let shown: Vec<&Camera> = snap
        .cameras
        .iter()
        .filter(|c| c.kind != Kind::Virtual)
        .collect();
    if shown.is_empty() {
        b.note(key::STUDIO_TROUBLE_NO_CAMERA);
        return;
    }
    let chosen = snap
        .camera_prefs
        .default
        .as_ref()
        .and_then(|id| shown.iter().find(|c| &c.identity == id))
        .map(|c| c.card.clone());
    b.pair(
        b.t(key::CAMERA_DEFAULT),
        chosen.unwrap_or_else(|| b.t(key::CAMERA_DEFAULT_FIRST)),
        Target::DefaultCamera,
        "default",
    );
    for c in shown {
        let id = c.identity.0.clone();
        let aside = match (c.kind, c.streamable) {
            (_, false) => Some(b.t(key::CAMERA_CANNOT_STREAM)),
            (Kind::Infrared, _) => Some(b.t(key::CAMERA_INFRARED)),
            _ => None,
        };
        b.under(tokens::icon::CAMERA, c.card.clone(), aside);
        if !c.streamable {
            b.note(key::CAMERA_ABOUT_CANNOT_STREAM);
            continue;
        }
        let per = snap.camera_prefs.of(&c.identity);
        let mode = per.mode.as_ref().map_or_else(
            || {
                ronoctua_camera::choose_mode(c, None).map_or_else(
                    || b.t(key::CAMERA_MODE_CHOSEN),
                    |m| {
                        b.texts
                            .t_with(key::CAMERA_MODE_CHOSEN_IS, &[("mode", &mode_text(&m))])
                    },
                )
            },
            mode_text,
        );
        b.pair(
            b.t(key::CAMERA_MODE),
            mode,
            Target::Mode(id.clone()),
            &format!("mode-{id}"),
        );
        b.pair(
            b.t(key::CAMERA_MIRRORED),
            on_off(per.mirror, b.texts),
            Target::Mirror(id.clone()),
            &format!("mirror-{id}"),
        );
        if let Some(true) = c.shuttered() {
            b.note(key::CAMERA_SHUTTERED);
        }
        for k in c.controls.iter().filter(|k| k.key != "privacy") {
            let value = match k.kind {
                ControlKind::Boolean => on_off(k.value != 0, b.texts),
                ControlKind::Menu | ControlKind::IntegerMenu => k
                    .menu
                    .iter()
                    .find(|(v, _)| *v == k.value)
                    .map_or_else(|| k.value.to_string(), |(_, n)| n.clone()),
                ControlKind::Integer => k.value.to_string(),
            };
            // A control another one decides (manual exposure while exposure is
            // automatic) is DRAWN and REFUSES — with nothing to turn, the value
            // reads as information.
            let target = if k.inactive || k.read_only {
                Target::Nothing
            } else {
                Target::Control(id.clone(), k.key.clone())
            };
            b.pair(
                k.name.clone(),
                value,
                target,
                &format!("control-{id}-{}", k.key),
            );
        }
        if c.controls.iter().any(|k| !k.read_only) {
            b.pair(
                b.t(key::CAMERA_FACTORY),
                String::new(),
                Target::Factory(id.clone()),
                &format!("factory-{id}"),
            );
        }
    }
}

fn mode_text(m: &Mode) -> String {
    format!(
        "{} {}×{} {}",
        m.fourcc.trim(),
        m.width,
        m.height,
        (m.fps().round() as u32)
    )
}

fn effects(b: &mut Builder<'_>, snap: &Snapshot) {
    use ronoctua_i18n::key;
    b.heading(key::CAMERA_HEADING_EFFECTS);
    b.note(key::CAMERA_ABOUT_EFFECTS);
    let state = snap.state.as_ref();
    let effect = state.and_then(|s| s.effect.clone());
    b.pair(
        b.t(key::CAMERA_EFFECT),
        crate::camera::effect_name(effect.as_deref(), b.texts),
        Target::Effect,
        "effect",
    );
    if let Some(d) = effect.as_deref().and_then(ronoctua_effects::declared) {
        let values = state.map(|s| s.effect_values.clone()).unwrap_or_default();
        for (i, p) in d.params.iter().enumerate() {
            let v = values.get(i).copied().unwrap_or(p.default);
            b.pair(
                b.t(p.key),
                v.to_string(),
                Target::Param(p.name.to_owned(), false),
                &format!("param-{}", p.name),
            );
        }
    }
    let background = state.and_then(|s| s.background.clone());
    b.pair(
        b.t(key::CAMERA_BACKGROUND),
        crate::camera::effect_name(background.as_deref(), b.texts),
        Target::Background,
        "background",
    );
    if let Some(d) = background.as_deref().and_then(ronoctua_effects::declared) {
        let values = state
            .map(|s| s.background_values.clone())
            .unwrap_or_default();
        for (i, p) in d.params.iter().enumerate() {
            let v = values.get(i).copied().unwrap_or(p.default);
            b.pair(
                b.t(p.key),
                v.to_string(),
                Target::Param(p.name.to_owned(), true),
                &format!("background-param-{}", p.name),
            );
        }
        if d.id == "background-replace" {
            let pic = snap
                .prefs
                .backdrop
                .as_ref()
                .and_then(|p| p.file_name())
                .map_or_else(
                    || b.t(key::CAMERA_NOT_SET),
                    |n| n.to_string_lossy().into_owned(),
                );
            b.pair(b.t(key::CAMERA_BACKDROP), pic, Target::Backdrop, "backdrop");
        }
    }
    if state.is_some_and(|s| {
        s.troubles
            .contains(&ronoctua_studio::wire::Trouble::NoModel)
    }) {
        b.note(key::STUDIO_TROUBLE_NO_MODEL);
    }
}

fn scenes(b: &mut Builder<'_>, snap: &Snapshot) {
    use ronoctua_i18n::key;
    let Some(state) = snap.state.as_ref() else {
        b.heading(key::CAMERA_HEADING_SCENES);
        b.note(key::CAMERA_NO_STUDIO);
        return;
    };
    b.heading(key::CAMERA_HEADING_SCENES);
    b.note(key::CAMERA_ABOUT_SCENES);
    for name in &state.scenes {
        let value = if *name == state.scene {
            b.t(key::CAMERA_ON_AIR)
        } else {
            String::new()
        };
        b.pair(
            crate::camera::scene_name(name, b.texts),
            value,
            Target::Scene(name.clone()),
            &format!("scene-{name}"),
        );
    }
    b.pair(
        b.t(key::CAMERA_SCENE_ADD),
        String::new(),
        Target::SceneAdd,
        "scene-add",
    );
    b.pair(
        b.t(key::CAMERA_SCENE_RENAME),
        String::new(),
        Target::SceneRename,
        "scene-rename",
    );
    b.heading(key::CAMERA_HEADING_SCRIPTS);
    b.note(key::CAMERA_ABOUT_SCRIPTS);
    for name in &state.scripts {
        let running = state.running.as_ref().is_some_and(|r| r.script == *name);
        b.pair(
            name.clone(),
            if running {
                b.t(key::CAMERA_RUNNING)
            } else {
                String::new()
            },
            Target::Script(name.clone()),
            &format!("script-{name}"),
        );
    }
    b.pair(
        b.t(key::CAMERA_SCRIPT_ADD),
        String::new(),
        Target::ScriptAdd,
        "script-add",
    );
}

fn on_off(on: bool, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(if on {
        ronoctua_i18n::key::POINTERS_ON
    } else {
        ronoctua_i18n::key::POINTERS_OFF
    })
}

/// Which press a command id is — the one map both doors reach.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "camera-settings-up" => Press::Up,
        "camera-settings-down" => Press::Down,
        "camera-settings-less" => Press::Less,
        "camera-settings-more" => Press::More,
        "camera-settings-enter" => Press::Enter,
        "camera-settings-remove" => Press::Remove,
        _ => return None,
    })
}

/// A key, as the view's press — the ONE reader both doors call: the window
/// with `door::keys::key_of`, the footer with the key its own reader built.
///
/// `Esc` is every level's way back, and the arrows and `RET` are read by their
/// symbol whatever is held, as both doors always read them. A LETTER is read
/// by its whole notation: `C-d` is not `d`. The footer used to
/// drop every modifier (`Key::simple`) and answer a letter held with `Ctrl` or
/// `Alt` as the bare letter, while the window read xkb's control character
/// and answered nothing — one key, two answers.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    use ronoctua_keymap::{Key, Sym};
    let notation = match key.sym {
        Sym::Esc => return Some(Press::Escape),
        Sym::Char(_) => key.notation(),
        other => Key::simple(other).notation(),
    };
    pressed(&notation)
}

/// Which press a notation is, through [`OF_THE_VIEW`].
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    OF_THE_VIEW
        .iter()
        .find(|(k, _, _)| *k == notation)
        .and_then(|(_, id, _)| press_of(id))
}

struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: usize,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    fn t(&self, key: &str) -> String {
        self.texts.t(key)
    }

    fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    fn inert(&mut self, fields: Vec<Field>) {
        self.out.push(Row {
            fields,
            focused: false,
            heading: true,
            target: Target::Nothing,
            spot: String::new(),
        });
    }

    fn heading(&mut self, key: &str) {
        if !self.out.is_empty() {
            self.inert(crate::chrome::AIR);
        }
        let text = self.texts.t(key);
        self.inert(vec![Self::piece(
            self.left,
            text,
            tokens::color::TEXT_HEADING,
        )]);
    }

    fn under(&mut self, glyph: char, text: String, aside: Option<String>) {
        let at = self.left + tokens::space::VIEW_NESTCELLS;
        let mut fields = vec![
            Self::piece(at, glyph.to_string(), tokens::color::TEXT_SECONDARY),
            Self::piece(at + 2, text.clone(), tokens::color::TEXT_SECONDARY),
        ];
        if let Some(aside) = aside {
            let after = at + 2 + ronoctua_protocol::cells(&text) + 1;
            fields.push(Self::piece(
                after,
                format!("({aside})"),
                tokens::color::TEXT_DIM,
            ));
        }
        self.inert(fields);
    }

    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        for line in crate::chrome::note_lines(&text, self.room) {
            self.inert(vec![Self::piece(self.left, line, tokens::color::TEXT_DIM)]);
        }
    }

    fn pair(&mut self, label: String, value: String, target: Target, spot: &str) {
        let focused = self.cursor == self.out.len();
        let acts = target.turns();
        let kind = crate::chrome::Kind::of(acts);
        self.out.push(Row {
            fields: crate::chrome::field_pair(self.left, self.room, &label, &value, kind, focused),
            focused,
            heading: false,
            target,
            spot: spot.to_owned(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_camera::{Control, Format, Identity, Interval, Size};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn a_camera() -> Camera {
        Camera {
            identity: Identity::of("Cam", "usb-1", "1-1:1.0"),
            node: "/dev/video0".into(),
            card: "Cam".into(),
            driver: "uvcvideo".into(),
            bus: "usb-1".into(),
            kind: Kind::Colour,
            formats: vec![Format {
                fourcc: "MJPG".into(),
                description: String::new(),
                compressed: true,
                sizes: vec![Size {
                    width: 1280,
                    height: 720,
                    intervals: vec![Interval { num: 1, den: 30 }],
                }],
            }],
            controls: vec![
                Control {
                    id: 1,
                    name: "Brightness".into(),
                    key: "brightness".into(),
                    kind: ControlKind::Integer,
                    min: 0,
                    max: 255,
                    step: 1,
                    default: 128,
                    value: 128,
                    inactive: false,
                    read_only: false,
                    menu: Vec::new(),
                },
                Control {
                    id: 2,
                    name: "Exposure".into(),
                    key: "exposure".into(),
                    kind: ControlKind::Integer,
                    min: 0,
                    max: 100,
                    step: 1,
                    default: 50,
                    value: 50,
                    inactive: true,
                    read_only: false,
                    menu: Vec::new(),
                },
            ],
            streamable: true,
        }
    }

    fn snap() -> Snapshot {
        Snapshot {
            cameras: vec![a_camera()],
            state: Some(State {
                scenes: vec!["camera".into(), "screen".into()],
                scene: "camera".into(),
                ..State::default()
            }),
            ..Snapshot::default()
        }
    }

    #[test]
    fn every_control_the_camera_has_is_a_row_and_an_inactive_one_refuses() {
        let tx = texts();
        let rows = View::fresh().rows(&snap(), 100, &tx);
        let b = rows
            .iter()
            .find(|r| r.spot.ends_with("-brightness"))
            .expect("brightness");
        assert!(matches!(b.target, Target::Control(_, _)));
        let e = rows
            .iter()
            .find(|r| r.spot.ends_with("-exposure"))
            .expect("exposure");
        assert_eq!(
            e.target,
            Target::Nothing,
            "an inactive control is drawn and refuses"
        );
    }

    #[test]
    fn a_brightness_moves_a_twentieth_of_its_range() {
        let tx = texts();
        let s = snap();
        let mut v = View::fresh();
        let rows = v.rows(&s, 100, &tx);
        let at = rows
            .iter()
            .position(|r| r.spot.ends_with("-brightness"))
            .expect("brightness");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::More, &s, 100, &tx),
            Errand::Control {
                identity: s.cameras[0].identity.0.clone(),
                key: "brightness".into(),
                value: 140
            }
        );
    }

    #[test]
    fn the_second_click_on_a_row_is_its_return() {
        let tx = texts();
        let s = snap();
        let mut v = View::fresh();
        let rows = v.rows(&s, 100, &tx);
        let at = rows
            .iter()
            .position(|r| r.spot == "scene-screen")
            .expect("a scene row");
        assert!(!v.clicked(at, &rows));
        assert!(v.clicked(at, &rows));
        assert_eq!(
            v.key(Press::Enter, &s, 100, &tx),
            Errand::Tell(Ask::Scene {
                name: "screen".into()
            })
        );
    }

    #[test]
    fn a_scene_is_removed_with_d_and_nothing_else_is() {
        let tx = texts();
        let s = snap();
        let mut v = View::fresh();
        let rows = v.rows(&s, 100, &tx);
        let at = rows
            .iter()
            .position(|r| r.spot == "scene-screen")
            .expect("a scene row");
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Remove, &s, 100, &tx),
            Errand::Tell(Ask::SceneRemove {
                name: "screen".into()
            })
        );
        let at = rows.iter().position(|r| r.spot == "size").expect("size");
        v.clicked(at, &rows);
        assert_eq!(v.key(Press::Remove, &s, 100, &tx), Errand::Nothing);
    }

    #[test]
    fn the_program_size_turns_round_the_ring() {
        let tx = texts();
        let s = snap();
        let mut v = View::fresh();
        let rows = v.rows(&s, 100, &tx);
        let at = rows.iter().position(|r| r.spot == "size").expect("size");
        v.clicked(at, &rows);
        let Errand::Studio(p) = v.key(Press::More, &s, 100, &tx) else {
            panic!("a studio change")
        };
        assert_eq!(p.size(), (1920, 1080));
    }

    #[test]
    fn putting_everything_back_asks_twice_and_leaving_the_row_takes_it_back() {
        let tx = texts();
        let s = snap();
        let mut v = View::fresh();
        let rows = v.rows(&s, 100, &tx);
        let at = rows
            .iter()
            .position(|r| r.spot == "defaults")
            .expect("the reset row");
        v.clicked(at, &rows);
        // The first press only arms it, and the row says so.
        assert_eq!(v.key(Press::Enter, &s, 100, &tx), Errand::Nothing);
        let armed = v.rows(&s, 100, &tx);
        let said: String = armed[at].fields.iter().map(|f| f.text.clone()).collect();
        assert!(
            said.contains(&tx.t(ronoctua_i18n::key::CAMERA_DEFAULTS_CONFIRM)),
            "{said}"
        );
        // Walking away and back disarms it: the next press arms again.
        v.key(Press::Up, &s, 100, &tx);
        v.key(Press::Down, &s, 100, &tx);
        assert_eq!(v.cursor(), at);
        assert_eq!(v.key(Press::Enter, &s, 100, &tx), Errand::Nothing);
        assert_eq!(v.key(Press::Enter, &s, 100, &tx), Errand::Defaults);
    }

    #[test]
    fn the_door_to_the_app_is_the_last_row() {
        let tx = texts();
        let s = snap();
        let mut v = View::fresh();
        let rows = v.rows(&s, 100, &tx);
        assert_eq!(rows.last().map(|r| r.spot.as_str()), Some("open-app"));
        let at = rows.len() - 1;
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, &s, 100, &tx),
            Errand::Run("camera-open")
        );
    }

    /// WHAT IS TYPED IS THE SAME ERRAND FROM EITHER DOOR: the footer's
    /// minibuffer and a window's line both answer into `answered`, and
    /// nothing typed — or an answer to a question nobody asked — is nothing.
    #[test]
    fn what_is_typed_is_one_errand_whichever_door_typed_it() {
        let s = snap();
        let state = s.state.as_ref();
        assert_eq!(
            answered(SCENE_ADD, "  interview ", state),
            Errand::Tell(Ask::SceneAdd {
                name: "interview".into()
            })
        );
        assert_eq!(
            answered(SCENE_RENAME, "desk", state),
            Errand::Tell(Ask::SceneRename {
                from: "camera".into(),
                to: "desk".into()
            }),
            "the scene renamed is the one on air"
        );
        assert_eq!(
            answered(SCRIPT_ADD, "talk: camera 30, screen", state),
            Errand::Script("talk: camera 30, screen".into())
        );
        assert_eq!(
            answered(STREAM_URL, "rtmp://example.org/live\n", state),
            Errand::StreamUrl("rtmp://example.org/live".into())
        );
        assert!(matches!(
            answered(STREAM_KEY, "k", state),
            Errand::StreamKey(_)
        ));
        for q in QUESTIONS {
            assert_eq!(answered(q.id, "   ", state), Errand::Nothing, "{}", q.id);
        }
        assert_eq!(answered("camera-open", "x", state), Errand::Nothing);
    }

    /// EVERY ROW THAT TYPES ASKS — and keeps the view: the errand is a
    /// question, never a command the door would run somewhere else. Only the
    /// stream's key is a secret.
    #[test]
    fn every_row_that_types_asks_its_own_question() {
        let tx = texts();
        let s = snap();
        let mut v = View::fresh();
        let rows = v.rows(&s, 100, &tx);
        for (spot, id) in [
            ("scene-add", SCENE_ADD),
            ("scene-rename", SCENE_RENAME),
            ("script-add", SCRIPT_ADD),
            ("stream-url", STREAM_URL),
            ("stream-key", STREAM_KEY),
        ] {
            let at = rows.iter().position(|r| r.spot == spot).expect(spot);
            v.clicked(at, &rows);
            assert_eq!(
                v.key(Press::Enter, &s, 100, &tx),
                Errand::Ask(Question {
                    id,
                    secret: id == STREAM_KEY
                }),
                "{spot}"
            );
            assert_eq!(v.cursor(), at, "{spot}: asking moved the cursor");
        }
        // AND THE DOOR'S OWN ERRANDS COME BACK TO IT untouched.
        let ask = Errand::Ask(QUESTIONS[0]);
        assert_eq!(carry_out(ask.clone(), &s, &tx), Err(ask));
    }

    #[test]
    fn every_hint_key_reaches_a_press() {
        for (k, id, _) in OF_THE_VIEW {
            assert!(pressed(k).is_some(), "{k} ({id})");
        }
    }

    #[test]
    fn a_stepwise_mode_ring_starts_with_the_choice_left_to_the_system() {
        let modes = modes_of(&a_camera());
        assert_eq!(modes[0], None);
        assert_eq!(modes.len(), 2);
    }
}
