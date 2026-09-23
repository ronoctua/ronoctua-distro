//! What the program shows: scenes, their layers, and the scripts that switch
//! between them.
//!
//! # The file is the broadcast
//!
//! `$XDG_CONFIG_HOME/ronoctua/studio/scenes.json`. The camera app edits a
//! layer's place with the pointer, the settings view renames a scene, a person
//! edits the file by hand — and the daemon rereads it on every change. A scene
//! changed DURING a recording is only the compositor reading another state:
//! the encoder never restarts, which is the whole point of compositing in the
//! daemon rather than in ffmpeg.
//!
//! # Geometry is in FRACTIONS of the program
//!
//! A layer's place is `[x, y, w, h]` in `0..=1`, so a scene written for 720p
//! is the same scene at 1080p. The one conversion to pixels is [`Rect::pixels`],
//! in integers, and it is the only place a program size meets a layer.

use serde::{Deserialize, Serialize};

/// Where a layer's pictures come from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Source {
    /// A camera; `None` is "the default one", whatever that is right now.
    Camera {
        /// A `ronoctua_camera::Identity`'s text.
        #[serde(default)]
        identity: Option<String>,
    },
    /// A monitor; `None` is the first one.
    Screen {
        /// The output's name (`eDP-1`, `DP-2`).
        #[serde(default)]
        output: Option<String>,
    },
    /// One window, matched by its application id or, failing that, a part of
    /// its title — the words a person can read on it.
    Window {
        /// What to match.
        matching: String,
    },
    /// A picture file.
    Image {
        /// Its path.
        path: String,
    },
    /// A video file, looped.
    Media {
        /// Its path.
        path: String,
    },
    /// A flat colour, `RRGGBBAA` — CONTENT, like a picture, and never a token:
    /// it is what a person picked for their stream's background.
    Colour {
        /// The colour.
        rgba: String,
    },
}

impl Source {
    /// A key that is the same for two layers showing the same thing, so one
    /// camera shown twice is opened once.
    pub fn key(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// How a picture fills its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit {
    /// All of it, bars where the shapes differ.
    Fit,
    /// All of the place, the picture cropped.
    #[default]
    Fill,
    /// Stretched to the place.
    Stretch,
}

/// A place in the program, in fractions of it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// The whole program.
    pub const WHOLE: Self = Self {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };

    /// In pixels of a `width × height` program: left, top, width, height.
    /// Never empty and never outside.
    pub fn pixels(&self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let clamp = |v: f32| v.clamp(0.0, 1.0);
        let x0 = (clamp(self.x) * width as f32).round() as u32;
        let y0 = (clamp(self.y) * height as f32).round() as u32;
        let x1 = (clamp(self.x + self.w) * width as f32).round() as u32;
        let y1 = (clamp(self.y + self.h) * height as f32).round() as u32;
        let x0 = x0.min(width.saturating_sub(1));
        let y0 = y0.min(height.saturating_sub(1));
        (
            x0,
            y0,
            x1.max(x0 + 1).min(width) - x0,
            y1.max(y0 + 1).min(height) - y0,
        )
    }

    /// Moved by a fraction, kept inside the program.
    #[must_use]
    pub fn moved(self, dx: f32, dy: f32) -> Self {
        Self {
            x: (self.x + dx).clamp(0.0, 1.0 - self.w.min(1.0)),
            y: (self.y + dy).clamp(0.0, 1.0 - self.h.min(1.0)),
            ..self
        }
    }

    /// Grown or shrunk about its centre by a factor, never below a twentieth
    /// of the program and never past the whole of it.
    #[must_use]
    pub fn scaled(self, by: f32) -> Self {
        let w = (self.w * by).clamp(0.05, 1.0);
        let h = (self.h * by).clamp(0.05, 1.0);
        let cx = self.x + self.w / 2.0;
        let cy = self.y + self.h / 2.0;
        Self {
            x: (cx - w / 2.0).clamp(0.0, 1.0 - w),
            y: (cy - h / 2.0).clamp(0.0, 1.0 - h),
            w,
            h,
        }
    }

    /// Whether a point in fractions is inside.
    pub fn holds(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// One effect in a layer's chain, as the file stores it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectStep {
    /// The declared id.
    pub id: String,
    /// Its values, in declared order.
    #[serde(default)]
    pub values: Vec<i32>,
}

/// One layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    /// What it shows.
    pub source: Source,
    /// Where.
    #[serde(default = "whole")]
    pub rect: Rect,
    /// How it fills that.
    #[serde(default)]
    pub fit: Fit,
    /// Opacity, 0 to 100.
    #[serde(default = "hundred")]
    pub opacity: u8,
    /// Crop from each edge, in fractions of the source: left, top, right,
    /// bottom.
    #[serde(default)]
    pub crop: [f32; 4],
    /// The effect chain, in order.
    #[serde(default)]
    pub effects: Vec<EffectStep>,
    /// Hidden without being removed.
    #[serde(default = "yes")]
    pub visible: bool,
}

fn whole() -> Rect {
    Rect::WHOLE
}
fn hundred() -> u8 {
    100
}
fn yes() -> bool {
    true
}

impl Layer {
    /// A layer showing `source` over the whole program.
    pub fn of(source: Source) -> Self {
        Self {
            source,
            rect: Rect::WHOLE,
            fit: Fit::Fill,
            opacity: 100,
            crop: [0.0; 4],
            effects: Vec::new(),
            visible: true,
        }
    }

    /// The chain as the effects crate runs it.
    pub fn chain(&self) -> ronoctua_effects::Chain {
        ronoctua_effects::Chain(
            self.effects
                .iter()
                .map(|e| ronoctua_effects::Step {
                    id: e.id.clone(),
                    values: e.values.clone(),
                })
                .collect(),
        )
    }

    /// The step of the chain that is not a background one — the one `e`
    /// turns. There is at most one of those by construction of the ring.
    pub fn effect(&self) -> Option<&str> {
        self.effects
            .iter()
            .find(|e| ronoctua_effects::declared(&e.id).is_some_and(|d| d.family != "background"))
            .map(|e| e.id.as_str())
    }

    /// The background step, if any — the one `b` turns.
    pub fn background(&self) -> Option<&str> {
        self.effects
            .iter()
            .find(|e| ronoctua_effects::declared(&e.id).is_some_and(|d| d.family == "background"))
            .map(|e| e.id.as_str())
    }

    /// Replaces the non-background effect with `id` (or removes it). The
    /// background step stays FIRST: the person is cut out of the camera
    /// picture, and then the colour effect runs on what is left.
    pub fn set_effect(&mut self, id: Option<&str>) {
        self.effects.retain(|e| {
            ronoctua_effects::declared(&e.id).is_some_and(|d| d.family == "background")
        });
        if let Some(step) = id.and_then(ronoctua_effects::Step::new) {
            self.effects.push(EffectStep {
                id: step.id,
                values: step.values,
            });
        }
    }

    /// Replaces the background step with `id` (or removes it).
    pub fn set_background(&mut self, id: Option<&str>) {
        self.effects.retain(|e| {
            !ronoctua_effects::declared(&e.id).is_some_and(|d| d.family == "background")
        });
        if let Some(step) = id.and_then(ronoctua_effects::Step::new) {
            self.effects.insert(
                0,
                EffectStep {
                    id: step.id,
                    values: step.values,
                },
            );
        }
    }
}

/// How one scene gives way to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transition {
    /// At once.
    Cut,
    /// A cross-fade, in milliseconds.
    Fade {
        /// How long.
        ms: u32,
    },
    /// The default: a short fade, which reads as a switch and not a glitch.
    #[default]
    Quick,
}

impl Transition {
    /// Its length in milliseconds.
    pub fn millis(&self) -> u32 {
        match self {
            Self::Cut => 0,
            Self::Fade { ms } => *ms,
            Self::Quick => 300,
        }
    }
}

/// One scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    /// Its name, which is what a person reads and what `studio-scene` takes.
    pub name: String,
    /// Bottom first.
    pub layers: Vec<Layer>,
    /// How it arrives.
    #[serde(default)]
    pub transition: Transition,
}

/// One step of a script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cue {
    /// The scene to show.
    pub scene: String,
    /// For how long, in seconds; `None` waits for the next `studio-next`.
    #[serde(default)]
    pub seconds: Option<u32>,
}

/// Reads a script as a person types it in the minibuffer:
/// `name: scene 30, other scene 60, last` — each cue a scene name and,
/// optionally, the seconds it stays (none waits for `studio-next`); a
/// trailing `loop` repeats it. `None` for anything else, and for a scene the
/// book does not have.
pub fn parse_script(text: &str, book: &Book) -> Option<Script> {
    let (name, rest) = text.split_once(':')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let mut cues = Vec::new();
    let mut looped = false;
    for part in rest.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        if part.eq_ignore_ascii_case("loop") {
            looped = true;
            continue;
        }
        let (scene, seconds) = match part.rsplit_once(' ') {
            Some((s, n)) if n.parse::<u32>().is_ok() => (s.trim(), n.parse().ok()),
            _ => (part, None),
        };
        if !book.scenes.iter().any(|s| s.name == scene) {
            return None;
        }
        cues.push(Cue {
            scene: scene.to_owned(),
            seconds,
        });
    }
    (!cues.is_empty()).then(|| Script {
        name: name.to_owned(),
        cues,
        looped,
    })
}

/// A script: scenes in order, on a clock or on command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Script {
    /// Its name.
    pub name: String,
    /// The cues.
    pub cues: Vec<Cue>,
    /// Starts over after the last.
    #[serde(default)]
    pub looped: bool,
}

/// The whole file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Book {
    /// Every scene.
    pub scenes: Vec<Scene>,
    /// Every script.
    pub scripts: Vec<Script>,
    /// The scene on air when nothing else says.
    pub active: String,
}

/// The names of the three scenes a fresh machine has — IDS, not words.
///
/// A scene's name is DATA: what a person types is shown as they typed it.
/// These three were not typed by anybody, and drawing them as English would be
/// a string literal on screen (invariant 8), so the views draw
/// `studio-scene-<name>` when the language has that key and the name itself
/// otherwise. Renamed, a scene is the person's and shows their words.
pub const CAMERA: &str = "camera";
/// See [`CAMERA`].
pub const SCREEN: &str = "screen";
/// See [`CAMERA`].
pub const SCREEN_AND_CAMERA: &str = "screen-and-camera";

/// The three ids above, for the check that both languages carry their keys.
pub const BUILT_IN: [&str; 3] = [CAMERA, SCREEN, SCREEN_AND_CAMERA];

impl Default for Book {
    fn default() -> Self {
        let camera = Source::Camera { identity: None };
        let screen = Source::Screen { output: None };
        let mut corner = Layer::of(camera.clone());
        // The bottom-right quarter, with the air a quarter of a margin gives.
        corner.rect = Rect {
            x: 0.72,
            y: 0.70,
            w: 0.25,
            h: 0.25,
        };
        Self {
            scenes: vec![
                Scene {
                    name: CAMERA.into(),
                    layers: vec![Layer::of(camera)],
                    transition: Transition::Quick,
                },
                Scene {
                    name: SCREEN.into(),
                    layers: vec![Layer {
                        fit: Fit::Fit,
                        ..Layer::of(screen.clone())
                    }],
                    transition: Transition::Quick,
                },
                Scene {
                    name: SCREEN_AND_CAMERA.into(),
                    layers: vec![
                        Layer {
                            fit: Fit::Fit,
                            ..Layer::of(screen)
                        },
                        corner,
                    ],
                    transition: Transition::Quick,
                },
            ],
            scripts: Vec::new(),
            active: CAMERA.into(),
        }
    }
}

impl Book {
    /// The scene named `name`, or the first.
    pub fn scene(&self, name: &str) -> Option<&Scene> {
        self.scenes
            .iter()
            .find(|s| s.name == name)
            .or_else(|| self.scenes.first())
    }

    /// The same, to change.
    pub fn scene_mut(&mut self, name: &str) -> Option<&mut Scene> {
        let at = self
            .scenes
            .iter()
            .position(|s| s.name == name)
            .or(if self.scenes.is_empty() {
                None
            } else {
                Some(0)
            })?;
        self.scenes.get_mut(at)
    }

    /// The scene after (`+1`) or before (`-1`) `name`, going round.
    pub fn turned(&self, name: &str, by: i32) -> Option<&str> {
        let n = self.scenes.len() as i32;
        if n == 0 {
            return None;
        }
        let at = self.scenes.iter().position(|s| s.name == name).unwrap_or(0) as i32;
        Some(&self.scenes[(at + by).rem_euclid(n) as usize].name)
    }

    /// Every source the scene named `name` needs.
    pub fn sources_of(&self, name: &str) -> Vec<Source> {
        let mut out: Vec<Source> = Vec::new();
        for l in self
            .scene(name)
            .map(|s| s.layers.as_slice())
            .unwrap_or_default()
        {
            if l.visible && !out.contains(&l.source) {
                out.push(l.source.clone());
            }
        }
        out
    }

    /// The layer the app's keys act on: the top camera layer, and in a scene
    /// with no camera the BOTTOM layer — a screen or a film is the picture a
    /// person puts an effect on there, and a key that did nothing would read
    /// as a broken key.
    pub fn camera_layer(&self, name: &str) -> Option<usize> {
        let layers = &self.scene(name)?.layers;
        layers
            .iter()
            .rposition(|l| matches!(l.source, Source::Camera { .. }))
            .or((!layers.is_empty()).then_some(0))
    }
}

/// Where a script is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Running {
    /// The script's name.
    pub script: String,
    /// The cue on air.
    pub cue: usize,
    /// Milliseconds left on it, `None` waiting for a command.
    pub left_ms: Option<u64>,
}

impl Running {
    /// Advances by `elapsed` milliseconds; `Some(scene)` when a new cue went
    /// on air, and the script's end is `Err(())`.
    #[allow(clippy::result_unit_err)]
    pub fn tick(&mut self, script: &Script, elapsed: u64) -> Result<Option<String>, ()> {
        let Some(left) = self.left_ms else {
            return Ok(None);
        };
        if elapsed < left {
            self.left_ms = Some(left - elapsed);
            return Ok(None);
        }
        self.advance(script).map(Some)
    }

    /// The next cue, by command or by clock.
    #[allow(clippy::result_unit_err)]
    pub fn advance(&mut self, script: &Script) -> Result<String, ()> {
        let next = self.cue + 1;
        let at = if next < script.cues.len() {
            next
        } else if script.looped && !script.cues.is_empty() {
            0
        } else {
            return Err(());
        };
        self.cue = at;
        self.left_ms = script.cues[at].seconds.map(|s| u64::from(s) * 1000);
        Ok(script.cues[at].scene.clone())
    }

    /// Starts a script at its first cue.
    pub fn start(script: &Script) -> Option<(Self, String)> {
        let first = script.cues.first()?;
        Some((
            Self {
                script: script.name.clone(),
                cue: 0,
                left_ms: first.seconds.map(|s| u64::from(s) * 1000),
            },
            first.scene.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rect_never_leaves_the_program_nor_vanishes() {
        assert_eq!(Rect::WHOLE.pixels(1280, 720), (0, 0, 1280, 720));
        let r = Rect {
            x: 0.9,
            y: 0.9,
            w: 0.5,
            h: 0.5,
        };
        let (x, y, w, h) = r.pixels(100, 100);
        assert!(x + w <= 100 && y + h <= 100 && w > 0 && h > 0);
        let tiny = Rect {
            x: 0.5,
            y: 0.5,
            w: 0.0,
            h: 0.0,
        };
        assert_eq!(tiny.pixels(100, 100).2, 1);
    }

    #[test]
    fn a_moved_rect_stays_inside_and_a_scaled_one_keeps_its_centre() {
        let r = Rect {
            x: 0.7,
            y: 0.7,
            w: 0.25,
            h: 0.25,
        };
        let m = r.moved(0.5, 0.5);
        assert!((m.x - 0.75).abs() < 1e-6 && (m.y - 0.75).abs() < 1e-6);
        let s = Rect {
            x: 0.4,
            y: 0.4,
            w: 0.2,
            h: 0.2,
        }
        .scaled(2.0);
        assert!((s.x + s.w / 2.0 - 0.5).abs() < 1e-6);
        assert!((s.w - 0.4).abs() < 1e-6);
    }

    #[test]
    fn a_fresh_book_opens_on_the_camera() {
        let b = Book::default();
        assert_eq!(b.active, CAMERA);
        assert_eq!(
            b.sources_of(CAMERA),
            vec![Source::Camera { identity: None }]
        );
        assert_eq!(b.sources_of(SCREEN_AND_CAMERA).len(), 2);
        assert_eq!(b.turned(CAMERA, 1), Some(SCREEN));
        assert_eq!(b.turned(CAMERA, -1), Some(SCREEN_AND_CAMERA));
    }

    #[test]
    fn the_file_keeps_what_it_does_not_name() {
        // A hand-written layer with nothing but a source: every other field
        // takes its default rather than refusing the file.
        let json = r#"{"scenes":[{"name":"x","layers":[{"source":{"kind":"colour","rgba":"000000ff"}}]}],"active":"x"}"#;
        let b: Book = serde_json::from_str(json).expect("parses");
        let l = &b.scenes[0].layers[0];
        assert_eq!(l.rect, Rect::WHOLE);
        assert_eq!(l.opacity, 100);
        assert!(l.visible);
    }

    #[test]
    fn the_background_step_stays_first() {
        let mut l = Layer::of(Source::Camera { identity: None });
        l.set_effect(Some("grey"));
        l.set_background(Some("background-blur"));
        assert_eq!(l.effects[0].id, "background-blur");
        assert_eq!(l.effect(), Some("grey"));
        l.set_effect(Some("sepia"));
        assert_eq!(l.effects.len(), 2);
        assert_eq!(l.background(), Some("background-blur"));
        l.set_background(None);
        assert_eq!(l.effects.len(), 1);
    }

    #[test]
    fn a_typed_script_reads_back() {
        let b = Book::default();
        let s =
            parse_script("talk: camera 30, screen-and-camera, screen 5, loop", &b).expect("parses");
        assert_eq!(s.name, "talk");
        assert!(s.looped);
        assert_eq!(
            s.cues[0],
            Cue {
                scene: "camera".into(),
                seconds: Some(30)
            }
        );
        assert_eq!(s.cues[1].seconds, None);
        assert!(
            parse_script("x: nowhere 3", &b).is_none(),
            "a scene the book lacks"
        );
        assert!(parse_script("no colon", &b).is_none());
    }

    #[test]
    fn a_script_walks_its_cues_on_the_clock_and_on_command() {
        let s = Script {
            name: "talk".into(),
            cues: vec![
                Cue {
                    scene: "a".into(),
                    seconds: Some(2),
                },
                Cue {
                    scene: "b".into(),
                    seconds: None,
                },
                Cue {
                    scene: "c".into(),
                    seconds: Some(1),
                },
            ],
            looped: false,
        };
        let (mut r, first) = Running::start(&s).expect("starts");
        assert_eq!(first, "a");
        assert_eq!(r.tick(&s, 1500), Ok(None));
        assert_eq!(r.tick(&s, 600), Ok(Some("b".into())));
        assert_eq!(r.tick(&s, 99_999), Ok(None), "b waits for a command");
        assert_eq!(r.advance(&s), Ok("c".into()));
        assert_eq!(
            r.tick(&s, 1000),
            Err(()),
            "the end of a script that does not loop"
        );
    }
}
