//! The camera app (`LEADER m c`): what its keys do and what its controls are.
//!
//! # A window that is the picture
//!
//! The program — the scene on air, effects and all — fills the whole window,
//! and the controls are drawn OVER it only while somebody is using them: the
//! pointer moved inside the window, or a key of the app was pressed, in the
//! last [`SHOWN_FOR`]; or they were pinned (`h`). A face on a video call is
//! what a person opened this for, and a toolbar over it permanently is chrome
//! that was not asked for (invariant 14 is about the desktop, and the same
//! reasoning holds inside a window whose whole content is one picture).
//!
//! # Every control is a key and a click
//!
//! [`OF_THE_VIEW`] is the one table: the hint line is generated from it, the
//! bottom row's buttons are the same entries drawn as glyphs, and a click on a
//! button runs exactly the key's press (invariant 19). Nothing here talks to
//! the daemon — [`Errand`] says what the door must do, and the door does it.
//!
//! # And a layer is dragged where it is drawn
//!
//! A scene's layers are in fractions of the program ([`ronoctua_studio::scene::Rect`]);
//! the door maps the pointer through the SAME picture rectangle it drew the
//! program into ([`picture_in`]), so a layer is picked up under the hand and
//! not a letterbox away from it.

use ronoctua_studio::{
    scene::Rect,
    wire::{Ask, Sound, State, Trouble},
};
use ronoctua_tokens as tokens;

/// How long the controls stay after the last sign of a person.
pub const SHOWN_FOR: std::time::Duration = std::time::Duration::from_secs(3);

/// The window's `app_id` — the compositor floats it by this (`terminal::floating_rules`).
pub const APP_ID: &str = "br.ronoctua.camera";

/// The keys of the app: notation, command id, and the glyph its button wears
/// (`None`: a key with no button — it lives on the hint line only).
pub const OF_THE_VIEW: &[(&str, &str, Option<&str>)] = &[
    ("SPC", "camera-photo", Some("studio-photo")),
    ("r", "camera-record", Some("studio-record")),
    ("e", "camera-effect-next", Some("studio-effect")),
    ("E", "camera-effect-previous", None),
    ("b", "camera-background-next", Some("studio-background")),
    ("-", "camera-param-less", None),
    ("=", "camera-param-more", None),
    ("c", "camera-camera-next", Some("studio-camera-turn")),
    ("m", "camera-mirror", Some("studio-mirror")),
    ("s", "camera-scene-next", Some("studio-scene")),
    ("S", "camera-scene-previous", None),
    ("p", "camera-script", Some("studio-script")),
    ("n", "camera-next-cue", None),
    ("v", "camera-virtual", Some("studio-virtual")),
    ("t", "camera-stream", Some("studio-stream")),
    ("a", "camera-mixer", Some("studio-mixer")),
    ("f", "camera-fit", Some("studio-fit")),
    ("h", "camera-pin", Some("studio-pin")),
    (",", "camera-settings-window", Some("studio-settings")),
    // THE MENUS, from the keyboard: the same four a click on the button
    // opens. What the mouse reaches, the keys reach.
    ("C-e", "camera-menu-effect", None),
    ("C-b", "camera-menu-background", None),
    ("C-c", "camera-menu-camera", None),
    ("C-s", "camera-menu-scene", None),
];

/// The keys that are the WINDOW's own: they change how this window draws
/// and mean nothing anywhere else, so they are not commands of the registry
/// and their sentences are `about-<id>`, not `cmd-<id>`.
pub const LOCAL: &[&str] = &[
    "camera-mixer",
    "camera-fit",
    "camera-pin",
    "camera-menu-effect",
    "camera-menu-background",
    "camera-menu-camera",
    "camera-menu-scene",
];

/// A key, once read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Take a photo.
    Photo,
    /// Start or stop recording.
    Record,
    /// The effect ring, forward or back.
    Effect(i32),
    /// The background ring.
    Background,
    /// The effect's first parameter, down or up a step.
    Param(i32),
    /// The next camera.
    Camera,
    /// Mirror on or off.
    Mirror,
    /// The scene ring.
    Scene(i32),
    /// Run the first script, or stop the one running.
    Script,
    /// The running script's next cue.
    NextCue,
    /// The virtual camera on or off.
    Virtual,
    /// The stream on or off.
    Stream,
    /// The sound drawer open or shut.
    Mixer,
    /// Fit or fill.
    Fit,
    /// Controls pinned or not.
    Pin,
    /// The settings, in a window.
    Settings,
    /// A menu opened, or shut when it is the one open.
    Menu(Menu),
    /// Close the window.
    Escape,
}

/// Which press a command id is.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "camera-photo" => Press::Photo,
        "camera-record" => Press::Record,
        "camera-effect-next" => Press::Effect(1),
        "camera-effect-previous" => Press::Effect(-1),
        "camera-background-next" => Press::Background,
        "camera-param-less" => Press::Param(-1),
        "camera-param-more" => Press::Param(1),
        "camera-camera-next" => Press::Camera,
        "camera-mirror" => Press::Mirror,
        "camera-scene-next" => Press::Scene(1),
        "camera-scene-previous" => Press::Scene(-1),
        "camera-script" => Press::Script,
        "camera-next-cue" => Press::NextCue,
        "camera-virtual" => Press::Virtual,
        "camera-stream" => Press::Stream,
        "camera-mixer" => Press::Mixer,
        "camera-fit" => Press::Fit,
        "camera-pin" => Press::Pin,
        "camera-settings-window" => Press::Settings,
        "camera-menu-effect" => Press::Menu(Menu::Effect),
        "camera-menu-background" => Press::Menu(Menu::Background),
        "camera-menu-camera" => Press::Menu(Menu::Camera),
        "camera-menu-scene" => Press::Menu(Menu::Scene),
        _ => return None,
    })
}

/// Which press a notation is, through [`OF_THE_VIEW`].
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    OF_THE_VIEW
        .iter()
        .find(|(k, _, _)| *k == notation)
        .and_then(|(_, id, _)| press_of(id))
}

/// What the door has to do about a press.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Only draw again.
    Nothing,
    /// Tell the studio.
    Tell(Ask),
    /// Run a command of the registry (the settings window).
    Run(&'static str),
    /// Close the window.
    Close,
}

/// The app's own state: what the door keeps between frames.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// The controls stay even without a person.
    pub pinned: bool,
    /// The picture fills the window (cropped) rather than fitting it (bars).
    pub fill: bool,
    /// The sound drawer is open.
    pub mixer: bool,
    /// When a person last did something here.
    pub last_sign: Option<std::time::Instant>,
    /// The menu open over the picture, and where its cursor is.
    pub menu: Option<Open>,
}

/// A control that holds ONE OF SEVERAL values, and opens a menu of them.
///
/// A click on its button opens the menu (the right button still turns the
/// ring, one value at a time); its key turns the ring, and `C-` with the
/// same letter opens the menu. Reaching the twelfth effect was eleven presses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// The camera layer's effect.
    Effect,
    /// Its background.
    Background,
    /// Which camera.
    Camera,
    /// The scene on air.
    Scene,
}

impl Menu {
    /// The button that opens it.
    #[must_use]
    pub const fn button(self) -> &'static str {
        match self {
            Self::Effect => "camera-effect-next",
            Self::Background => "camera-background-next",
            Self::Camera => "camera-camera-next",
            Self::Scene => "camera-scene-next",
        }
    }

    /// The menu a button opens, when it opens one.
    #[must_use]
    pub fn of_button(id: &str) -> Option<Self> {
        Some(match id {
            "camera-effect-next" => Self::Effect,
            "camera-background-next" => Self::Background,
            "camera-camera-next" => Self::Camera,
            "camera-scene-next" => Self::Scene,
            _ => return None,
        })
    }
}

/// An open menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Open {
    /// Which.
    pub menu: Menu,
    /// The row the keyboard is on.
    pub cursor: usize,
}

/// One row of a menu.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// What it reads, translated.
    pub label: String,
    /// What choosing it asks the studio.
    pub ask: Ask,
    /// It is the value now.
    pub current: bool,
}

/// What a menu's rows ASK, and which is current — everything but the words,
/// so the keyboard can walk and choose without a translation in hand.
#[must_use]
pub fn asks(menu: Menu, state: Option<&State>) -> Vec<(Ask, bool)> {
    let effects = |background: bool, now: Option<&str>| {
        let ask = |name: Option<String>| {
            if background {
                Ask::Background { name }
            } else {
                Ask::Effect { name }
            }
        };
        let mut out = vec![(ask(None), now.is_none())];
        out.extend(
            ronoctua_effects::DECLARED
                .iter()
                .filter(|d| (d.family == "background") == background)
                .map(|d| (ask(Some(d.id.to_owned())), now == Some(d.id))),
        );
        out
    };
    match menu {
        Menu::Effect => effects(false, state.and_then(|s| s.effect.as_deref())),
        Menu::Background => effects(true, state.and_then(|s| s.background.as_deref())),
        Menu::Camera => state.map_or_else(Vec::new, |s| {
            s.cameras
                .iter()
                .filter(|c| c.streamable)
                .map(|c| {
                    (
                        Ask::Camera {
                            identity: c.identity.clone(),
                        },
                        s.camera.as_deref() == Some(c.identity.as_str()),
                    )
                })
                .collect()
        }),
        Menu::Scene => state.map_or_else(Vec::new, |s| {
            s.scenes
                .iter()
                .map(|n| (Ask::Scene { name: n.clone() }, *n == s.scene))
                .collect()
        }),
    }
}

/// A menu's rows with their words: every value the ring walks, in the ring's
/// order, "none" first where the ring has one.
#[must_use]
pub fn choices(menu: Menu, state: Option<&State>, texts: &ronoctua_i18n::Texts) -> Vec<Choice> {
    asks(menu, state)
        .into_iter()
        .map(|(ask, current)| {
            let label = match &ask {
                Ask::Effect { name } | Ask::Background { name } => {
                    effect_name(name.as_deref(), texts)
                }
                Ask::Scene { name } => scene_name(name, texts),
                Ask::Camera { identity } => state
                    .and_then(|s| s.cameras.iter().find(|c| c.identity == *identity))
                    .map_or_else(
                        || identity.clone(),
                        |c| {
                            if c.infrared {
                                format!(
                                    "{} ({})",
                                    c.card,
                                    texts.t(ronoctua_i18n::key::CAMERA_INFRARED)
                                )
                            } else {
                                c.card.clone()
                            }
                        },
                    ),
                _ => String::new(),
            };
            Choice {
                label,
                ask,
                current,
            }
        })
        .collect()
}

/// Where a menu is drawn, in cells and lines — and so where it is hit: the
/// window paints from this and asks it where a click fell, and no second
/// sum exists to disagree with the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuBox {
    /// First column.
    pub left: u16,
    /// First line.
    pub top: u16,
    /// Rows per column.
    pub lines: u16,
    /// How many columns.
    pub columns: u16,
    /// Cells per column: air, the mark, a space, the label, air.
    pub width: u16,
    /// How many choices.
    pub count: usize,
}

/// Cells a menu row spends besides its label: air, the mark and its space,
/// and air.
pub const MENU_FRAME: u16 = 4;

impl MenuBox {
    /// Where row `i` is: its first column and its line.
    #[must_use]
    pub fn place(&self, i: usize) -> Option<(u16, u16)> {
        if i >= self.count || self.lines == 0 {
            return None;
        }
        let lines = usize::from(self.lines);
        let column = u16::try_from(i / lines).ok()?;
        let line = u16::try_from(i % lines).ok()?;
        Some((self.left + column * self.width, self.top + line))
    }

    /// Which row a cell is in, if any.
    #[must_use]
    pub fn at(&self, column: u16, line: u16) -> Option<usize> {
        if column < self.left || line < self.top || line >= self.top + self.lines || self.width == 0
        {
            return None;
        }
        let c = (column - self.left) / self.width;
        if c >= self.columns {
            return None;
        }
        let i = usize::from(c) * usize::from(self.lines) + usize::from(line - self.top);
        (i < self.count).then_some(i)
    }

    /// Cells a label may take.
    #[must_use]
    pub const fn label_cells(&self) -> u16 {
        self.width.saturating_sub(MENU_FRAME)
    }
}

/// A menu's box: sitting on line `end` (exclusive) and growing up as far as
/// `first`, starting under the button's column `anchor` and kept inside
/// `[left_edge, right)`. Too many rows for the height become COLUMNS, the way
/// the leader's list does, rather than a list that scrolls over the picture;
/// too many columns for the width share the width and cut their labels.
#[must_use]
pub fn menu_box(
    labels: &[String],
    anchor: u16,
    first: u16,
    end: u16,
    left_edge: u16,
    right: u16,
) -> Option<MenuBox> {
    let count = labels.len();
    let room_lines = usize::from(end.saturating_sub(first));
    let room_cells = right.saturating_sub(left_edge);
    if count == 0 || room_lines == 0 || room_cells <= MENU_FRAME {
        return None;
    }
    let widest = labels
        .iter()
        .map(|l| ronoctua_protocol::cells(l))
        .max()
        .unwrap_or(0);
    let mut width = (widest + MENU_FRAME).min(room_cells);
    let columns = count.div_ceil(room_lines);
    let columns = u16::try_from(columns).ok()?;
    if columns * width > room_cells {
        width = room_cells / columns;
        if width <= MENU_FRAME {
            return None;
        }
    }
    let lines = u16::try_from(count.div_ceil(usize::from(columns))).ok()?;
    let span = columns * width;
    let left = anchor.clamp(left_edge, right - span);
    Some(MenuBox {
        left,
        top: end - lines,
        lines,
        columns,
        width,
        count,
    })
}

/// A key inside an open menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    /// A row up.
    Up,
    /// A row down.
    Down,
    /// A column left.
    Left,
    /// A column right.
    Right,
    /// Take the row the cursor is on.
    Choose,
    /// Shut, choosing nothing.
    Close,
}

impl Default for View {
    fn default() -> Self {
        Self {
            pinned: false,
            fill: false,
            mixer: false,
            last_sign: Some(std::time::Instant::now()),
            menu: None,
        }
    }
}

impl View {
    /// A person did something: the controls come back.
    pub fn sign(&mut self) {
        self.last_sign = Some(std::time::Instant::now());
    }

    /// Whether the controls are drawn now.
    #[must_use]
    pub fn showing(&self) -> bool {
        self.pinned
            || self.mixer
            || self.menu.is_some()
            || self.last_sign.is_some_and(|t| t.elapsed() < SHOWN_FOR)
    }

    /// A menu opened on its current value, or shut when it is the one open.
    pub fn toggle_menu(&mut self, menu: Menu, state: Option<&State>) {
        self.sign();
        if self.menu.is_some_and(|o| o.menu == menu) {
            self.menu = None;
            return;
        }
        // One drawer over the picture at a time: the sound drawer sits where
        // the menu would.
        self.mixer = false;
        let cursor = asks(menu, state)
            .iter()
            .position(|(_, current)| *current)
            .unwrap_or(0);
        self.menu = Some(Open { menu, cursor });
    }

    /// A key inside the open menu. `lines` is the box's rows per column,
    /// which is how far a column away is.
    pub fn menu_key(&mut self, key: MenuKey, lines: u16, state: Option<&State>) -> Errand {
        self.sign();
        let Some(open) = self.menu else {
            return Errand::Nothing;
        };
        let rows = asks(open.menu, state);
        if rows.is_empty() {
            self.menu = None;
            return Errand::Nothing;
        }
        let last = rows.len() - 1;
        let lines = usize::from(lines.max(1));
        let cursor = open.cursor.min(last);
        let moved = match key {
            MenuKey::Up => cursor.saturating_sub(1),
            MenuKey::Down => (cursor + 1).min(last),
            MenuKey::Left => cursor.saturating_sub(lines),
            MenuKey::Right => (cursor + lines).min(last),
            MenuKey::Close => {
                self.menu = None;
                return Errand::Nothing;
            }
            MenuKey::Choose => {
                self.menu = None;
                return Errand::Tell(rows[cursor].0.clone());
            }
        };
        self.menu = Some(Open {
            menu: open.menu,
            cursor: moved,
        });
        Errand::Nothing
    }

    /// A click on row `i` of the open menu: the first click SELECTS, the
    /// second on the same row takes it — what `RET` does (invariant 19).
    pub fn menu_clicked(&mut self, i: usize, state: Option<&State>) -> Errand {
        let Some(open) = self.menu else {
            return Errand::Nothing;
        };
        if open.cursor == i {
            return self.menu_key(MenuKey::Choose, 1, state);
        }
        self.sign();
        self.menu = Some(Open {
            menu: open.menu,
            cursor: i,
        });
        Errand::Nothing
    }

    /// A press, against the state the studio last said.
    pub fn key(&mut self, press: Press, state: Option<&State>) -> Errand {
        self.sign();
        match press {
            Press::Photo => Errand::Tell(Ask::Photo),
            Press::Record => Errand::Tell(Ask::Record { on: None }),
            Press::Effect(by) => Errand::Tell(Ask::EffectTurn { by }),
            Press::Background => Errand::Tell(Ask::BackgroundTurn { by: 1 }),
            Press::Param(by) => {
                // The effect's first parameter; with no effect, the
                // background's.
                let pick = |id: Option<&str>| {
                    id.and_then(ronoctua_effects::declared)
                        .and_then(|d| d.params.first())
                        .map(|p| p.name.to_owned())
                };
                let effect = pick(state.and_then(|s| s.effect.as_deref()));
                let background = pick(state.and_then(|s| s.background.as_deref()));
                match (effect, background) {
                    (Some(name), _) => Errand::Tell(Ask::EffectParam {
                        name,
                        by,
                        background: false,
                    }),
                    (None, Some(name)) => Errand::Tell(Ask::EffectParam {
                        name,
                        by,
                        background: true,
                    }),
                    (None, None) => Errand::Nothing,
                }
            }
            Press::Camera => Errand::Tell(Ask::CameraTurn { by: 1 }),
            Press::Mirror => Errand::Tell(Ask::Mirror),
            Press::Scene(by) => Errand::Tell(Ask::SceneTurn { by }),
            Press::Script => match state {
                Some(s) if s.running.is_some() => Errand::Tell(Ask::Script { name: None }),
                Some(s) => s.scripts.first().map_or(Errand::Nothing, |n| {
                    Errand::Tell(Ask::Script {
                        name: Some(n.clone()),
                    })
                }),
                None => Errand::Nothing,
            },
            Press::NextCue => Errand::Tell(Ask::Next),
            Press::Virtual => Errand::Tell(Ask::Virtual { on: None }),
            Press::Stream => Errand::Tell(Ask::Stream { on: None }),
            Press::Mixer => {
                self.menu = None;
                self.mixer = !self.mixer;
                Errand::Nothing
            }
            Press::Fit => {
                self.fill = !self.fill;
                Errand::Nothing
            }
            Press::Pin => {
                self.pinned = !self.pinned;
                Errand::Nothing
            }
            Press::Settings => Errand::Run("camera-settings-window"),
            Press::Menu(menu) => {
                self.toggle_menu(menu, state);
                Errand::Nothing
            }
            Press::Escape => {
                if self.mixer {
                    self.mixer = false;
                    Errand::Nothing
                } else {
                    Errand::Close
                }
            }
        }
    }
}

/// One button of the bottom row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    /// The command id it presses.
    pub id: &'static str,
    /// The glyph, from the icon tokens.
    pub glyph: char,
    /// The words beside it, when the button carries a value (the effect's
    /// name, the scene's) — already translated.
    pub words: Option<String>,
    /// Lit: the thing it switches is on (recording, virtual camera…).
    pub on: bool,
}

/// The glyph a token names.
fn glyph(token: &str) -> char {
    match token {
        "studio-photo" => tokens::icon::STUDIO_PHOTO,
        "studio-record" => tokens::icon::STUDIO_RECORD,
        "studio-stop" => tokens::icon::STUDIO_STOP,
        "studio-effect" => tokens::icon::STUDIO_EFFECT,
        "studio-background" => tokens::icon::STUDIO_BACKGROUND,
        "studio-camera-turn" => tokens::icon::STUDIO_CAMERA_TURN,
        "studio-mirror" => tokens::icon::STUDIO_MIRROR,
        "studio-scene" => tokens::icon::STUDIO_SCENE,
        "studio-script" => tokens::icon::STUDIO_SCRIPT,
        "studio-virtual" => tokens::icon::STUDIO_VIRTUAL,
        "studio-stream" => tokens::icon::STUDIO_STREAM,
        "studio-mixer" => tokens::icon::STUDIO_MIXER,
        "studio-fit" => tokens::icon::STUDIO_FIT,
        "studio-fill" => tokens::icon::STUDIO_FILL,
        "studio-pin" => tokens::icon::STUDIO_PIN,
        "studio-unpin" => tokens::icon::STUDIO_UNPIN,
        _ => tokens::icon::STUDIO_SETTINGS,
    }
}

/// A scene's name as a person reads it: the built-in ids translated, a name a
/// person typed shown as they typed it (see `ronoctua_studio::scene::CAMERA`).
#[must_use]
pub fn scene_name(name: &str, texts: &ronoctua_i18n::Texts) -> String {
    let key = format!("studio-scene-{name}");
    let said = texts.t(&key);
    if said.starts_with('[') {
        name.to_owned()
    } else {
        said
    }
}

/// An effect's name, or "none".
#[must_use]
pub fn effect_name(id: Option<&str>, texts: &ronoctua_i18n::Texts) -> String {
    id.and_then(ronoctua_effects::declared).map_or_else(
        || texts.t(ronoctua_i18n::key::CAMERA_EFFECT_NONE),
        |d| texts.t(d.key),
    )
}

/// The bottom row, from the table and the studio's state.
#[must_use]
pub fn buttons(view: &View, state: Option<&State>, texts: &ronoctua_i18n::Texts) -> Vec<Button> {
    OF_THE_VIEW
        .iter()
        .filter_map(|(_, id, token)| {
            let token = (*token)?;
            let (glyph_of, words, on) = match *id {
                "camera-record" => {
                    let rolling = state.is_some_and(|s| s.recording.is_some());
                    (if rolling { "studio-stop" } else { token }, None, rolling)
                }
                "camera-effect-next" => (
                    token,
                    Some(effect_name(state.and_then(|s| s.effect.as_deref()), texts)),
                    state.is_some_and(|s| s.effect.is_some()),
                ),
                "camera-background-next" => (
                    token,
                    Some(effect_name(
                        state.and_then(|s| s.background.as_deref()),
                        texts,
                    )),
                    state.is_some_and(|s| s.background.is_some()),
                ),
                "camera-scene-next" => (token, state.map(|s| scene_name(&s.scene, texts)), false),
                "camera-script" => {
                    // A SCRIPT BUTTON ONLY WHERE THERE IS A SCRIPT: with none, it
                    // would be a button that does nothing.
                    if state.is_none_or(|s| s.scripts.is_empty()) {
                        return None;
                    }
                    (token, None, state.is_some_and(|s| s.running.is_some()))
                }
                "camera-mirror" => (token, None, state.is_some_and(|s| s.mirror)),
                "camera-virtual" => (token, None, state.is_some_and(|s| s.virtual_node.is_some())),
                "camera-stream" => (token, None, state.is_some_and(|s| s.streaming.is_some())),
                "camera-mixer" => (token, None, view.mixer),
                "camera-fit" => (
                    if view.fill { "studio-fill" } else { token },
                    None,
                    view.fill,
                ),
                "camera-pin" => (
                    if view.pinned { "studio-unpin" } else { token },
                    None,
                    view.pinned,
                ),
                _ => (token, None, false),
            };
            Some(Button {
                id,
                glyph: glyph(glyph_of),
                words,
                on,
            })
        })
        .collect()
}

/// The header while the pointer rests on a button: its key, what it does,
/// and — for a control with several values — what each mouse button does.
#[must_use]
pub fn about_button(id: &str, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
    let mut out = Vec::new();
    if let Some((key, _, _)) = OF_THE_VIEW.iter().find(|(_, i, _)| *i == id) {
        let piece = crate::chrome::Piece::key(*key, false);
        // THE KEY IS A CONTROL like every other one on a header: a click on
        // it presses it.
        out.push(match ronoctua_keymap::Key::parse(key) {
            Ok(k) => piece.pressing(k),
            Err(_) => piece,
        });
        out.push(crate::chrome::Piece::prose(" "));
    }
    if let Some(sentence) = crate::chrome::hint_sentence(id, texts) {
        out.push(crate::chrome::Piece::said(sentence));
    }
    if Menu::of_button(id).is_some() {
        out.push(crate::chrome::Piece::separator());
        out.push(crate::chrome::Piece::prose(
            texts.t(ronoctua_i18n::key::CAMERA_BUTTON_MENU),
        ));
    }
    out
}

/// The hint line: every key of the table.
#[must_use]
pub fn hint(texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
    let here: Vec<(&str, &str)> = OF_THE_VIEW.iter().map(|(k, id, _)| (*k, *id)).collect();
    crate::chrome::hint_of(&here, texts)
}

/// The sentence for a trouble — every one has a key in both languages.
#[must_use]
pub fn trouble_sentence(t: &Trouble, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    match t {
        Trouble::NoCamera => texts.t(key::STUDIO_TROUBLE_NO_CAMERA),
        Trouble::CameraBusy { by } => {
            texts.t_with(key::STUDIO_TROUBLE_CAMERA_BUSY, &[("program", by)])
        }
        Trouble::CameraSilent => texts.t(key::STUDIO_TROUBLE_CAMERA_SILENT),
        Trouble::NoModel => texts.t(key::STUDIO_TROUBLE_NO_MODEL),
        Trouble::NoHandModel => texts.t(key::STUDIO_TROUBLE_NO_HAND_MODEL),
        Trouble::NoBackdrop => texts.t(key::STUDIO_TROUBLE_NO_BACKDROP),
        Trouble::NoFfmpeg => texts.t(key::STUDIO_TROUBLE_NO_FFMPEG),
        Trouble::NoScreen => texts.t(key::STUDIO_TROUBLE_NO_SCREEN),
        Trouble::NoWindow => texts.t(key::STUDIO_TROUBLE_NO_WINDOW),
        Trouble::NoVirtual => texts.t(key::STUDIO_TROUBLE_NO_VIRTUAL),
        Trouble::VirtualBusy => texts.t(key::STUDIO_TROUBLE_VIRTUAL_BUSY),
        Trouble::Encoder { why } => texts.t_with(key::STUDIO_TROUBLE_ENCODER, &[("why", why)]),
        Trouble::Stream { why } => texts.t_with(key::STUDIO_TROUBLE_STREAM, &[("why", why)]),
        Trouble::NoStreamTarget => texts.t(key::STUDIO_TROUBLE_NO_STREAM_TARGET),
        Trouble::Name => texts.t(key::STUDIO_TROUBLE_NAME),
        Trouble::Write { why } => texts.t_with(key::STUDIO_TROUBLE_WRITE, &[("why", why)]),
    }
}

/// Where the program is drawn inside a window of `w × h` pixels: fitted
/// (bars) or filled (cropped, the rectangle larger than the window and
/// centred — the part outside is simply not on screen).
#[must_use]
pub fn picture_in(w: f32, h: f32, program: (u32, u32), fill: bool) -> ronoctua_render::image::Area {
    let (pw, ph) = (program.0.max(1) as f32, program.1.max(1) as f32);
    let scale = if fill {
        (w / pw).max(h / ph)
    } else {
        (w / pw).min(h / ph)
    };
    let (dw, dh) = (pw * scale, ph * scale);
    ronoctua_render::image::Area {
        x: (w - dw) / 2.0,
        y: (h - dh) / 2.0,
        width: dw,
        height: dh,
    }
}

/// A point of the window as a fraction of the program, when it is on it.
#[must_use]
pub fn on_program(area: ronoctua_render::image::Area, x: f32, y: f32) -> Option<(f32, f32)> {
    if area.width <= 0.0 || area.height <= 0.0 {
        return None;
    }
    let u = (x - area.x) / area.width;
    let v = (y - area.y) / area.height;
    ((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v)).then_some((u, v))
}

/// The topmost layer under a point that a hand may MOVE: visible and not the
/// whole program (a full-frame layer has nowhere to go).
#[must_use]
pub fn movable_at(state: &State, u: f32, v: f32) -> Option<(usize, Rect)> {
    state
        .layers
        .iter()
        .enumerate()
        .rev()
        .find(|(_, l)| l.visible && (l.rect.w < 0.999 || l.rect.h < 0.999) && l.rect.holds(u, v))
        .map(|(i, l)| (i, l.rect))
}

/// `mm:ss` since an epoch second.
#[must_use]
pub fn elapsed(since: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let s = now.saturating_sub(since);
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// The two sound rows the mixer drawer draws: which, the level and the gain.
#[must_use]
pub fn sounds(state: &State) -> [(Sound, u8, u8, bool); 2] {
    [
        (
            Sound::Microphone,
            state.levels[0],
            state.gains[0],
            state.muted[0],
        ),
        (
            Sound::Desktop,
            state.levels[1],
            state.gains[1],
            state.muted[1],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn labels(n: usize, cells: usize) -> Vec<String> {
        (0..n).map(|_| "x".repeat(cells)).collect()
    }

    #[test]
    fn a_menu_sits_on_the_status_line_under_its_button() {
        let b = menu_box(&labels(3, 6), 10, 1, 20, 2, 100).expect("fits");
        assert_eq!((b.left, b.top, b.lines, b.columns), (10, 17, 3, 1));
        assert_eq!(b.width, 6 + MENU_FRAME);
    }

    #[test]
    fn too_many_rows_become_columns_and_every_row_is_hit_where_it_is_drawn() {
        let b = menu_box(&labels(15, 8), 4, 5, 11, 2, 200).expect("fits");
        assert_eq!((b.lines, b.columns), (5, 3), "{b:?}");
        for i in 0..15 {
            let (c, l) = b.place(i).expect("placed");
            for dc in 0..b.width {
                assert_eq!(b.at(c + dc, l), Some(i), "cell {dc} of row {i}");
            }
        }
        assert_eq!(b.at(b.left + b.columns * b.width, b.top), None);
        assert_eq!(b.at(b.left, b.top + b.lines), None);
    }

    #[test]
    fn a_menu_near_the_right_edge_moves_left_to_stay_inside() {
        let b = menu_box(&labels(2, 10), 95, 1, 20, 2, 100).expect("fits");
        assert_eq!(b.left + b.columns * b.width, 100);
    }

    #[test]
    fn the_menu_keys_walk_and_choose_and_the_key_again_shuts_it() {
        let mut v = View::default();
        v.key(Press::Menu(Menu::Effect), None);
        assert_eq!(v.menu.map(|o| o.cursor), Some(0), "opens on none");
        v.menu_key(MenuKey::Down, 5, None);
        v.menu_key(MenuKey::Down, 5, None);
        let chosen = v.menu_key(MenuKey::Choose, 5, None);
        let second = ronoctua_effects::DECLARED
            .iter()
            .filter(|d| d.family != "background")
            .nth(1)
            .map(|d| d.id.to_owned());
        assert_eq!(chosen, Errand::Tell(Ask::Effect { name: second }));
        assert!(v.menu.is_none());
        v.key(Press::Menu(Menu::Scene), None);
        v.key(Press::Menu(Menu::Scene), None);
        assert!(v.menu.is_none(), "its own key shuts it");
    }

    #[test]
    fn a_click_selects_and_the_second_takes() {
        let mut v = View::default();
        v.toggle_menu(Menu::Background, None);
        assert_eq!(v.menu_clicked(1, None), Errand::Nothing);
        assert!(matches!(
            v.menu_clicked(1, None),
            Errand::Tell(Ask::Background { name: Some(_) })
        ));
    }

    #[test]
    fn every_key_of_the_table_is_a_press() {
        for (k, id, _) in OF_THE_VIEW {
            assert!(
                pressed(k).is_some(),
                "{k} ({id}) is taught and does nothing"
            );
        }
    }

    #[test]
    fn the_controls_leave_when_nobody_is_there_unless_pinned() {
        let mut v = View {
            last_sign: None,
            ..View::default()
        };
        assert!(!v.showing());
        v.sign();
        assert!(v.showing());
        v.last_sign = Some(std::time::Instant::now() - SHOWN_FOR * 2);
        assert!(!v.showing());
        v.key(Press::Pin, None);
        v.last_sign = None;
        assert!(v.showing(), "pinned stays");
    }

    #[test]
    fn a_fitted_picture_has_bars_and_a_filled_one_overflows() {
        let fit = picture_in(1000.0, 1000.0, (1280, 720), false);
        assert!((fit.width - 1000.0).abs() < 0.01 && fit.y > 0.0);
        let fill = picture_in(1000.0, 1000.0, (1280, 720), true);
        assert!((fill.height - 1000.0).abs() < 0.01 && fill.x < 0.0);
    }

    #[test]
    fn a_point_maps_to_the_layer_under_it() {
        let area = picture_in(1280.0, 720.0, (1280, 720), false);
        let state = State {
            layers: vec![
                ronoctua_studio::wire::LayerSeen {
                    rect: Rect::WHOLE,
                    visible: true,
                    camera: false,
                },
                ronoctua_studio::wire::LayerSeen {
                    rect: Rect {
                        x: 0.7,
                        y: 0.7,
                        w: 0.25,
                        h: 0.25,
                    },
                    visible: true,
                    camera: true,
                },
            ],
            ..State::default()
        };
        let (u, v) = on_program(area, 1100.0, 600.0).expect("on it");
        assert_eq!(movable_at(&state, u, v).map(|(i, _)| i), Some(1));
        let (u, v) = on_program(area, 100.0, 100.0).expect("on it");
        assert_eq!(
            movable_at(&state, u, v),
            None,
            "the full-frame layer does not move"
        );
    }

    #[test]
    fn the_record_button_becomes_stop_while_recording() {
        let tx = texts();
        let mut s = State::default();
        let find = |b: &[Button]| {
            b.iter()
                .find(|b| b.id == "camera-record")
                .cloned()
                .expect("a record button")
        };
        assert_eq!(
            find(&buttons(&View::default(), Some(&s), &tx)).glyph,
            tokens::icon::STUDIO_RECORD
        );
        s.recording = Some(ronoctua_studio::wire::Rolling {
            since: 0,
            target: String::new(),
        });
        let b = find(&buttons(&View::default(), Some(&s), &tx));
        assert_eq!(b.glyph, tokens::icon::STUDIO_STOP);
        assert!(b.on);
    }

    #[test]
    fn a_built_in_scene_is_translated_and_a_typed_one_is_not() {
        let tx = texts();
        assert_ne!(scene_name("camera", &tx), "camera");
        assert_eq!(scene_name("My talk", &tx), "My talk");
    }

    #[test]
    fn every_trouble_has_a_sentence() {
        let tx = texts();
        for t in [
            Trouble::NoCamera,
            Trouble::CameraBusy { by: "x".into() },
            Trouble::CameraSilent,
            Trouble::NoModel,
            Trouble::NoHandModel,
            Trouble::NoBackdrop,
            Trouble::NoFfmpeg,
            Trouble::NoScreen,
            Trouble::NoWindow,
            Trouble::NoVirtual,
            Trouble::VirtualBusy,
            Trouble::Encoder { why: "x".into() },
            Trouble::Stream { why: "x".into() },
            Trouble::NoStreamTarget,
            Trouble::Name,
            Trouble::Write { why: "x".into() },
        ] {
            assert!(
                !trouble_sentence(&t, &tx).starts_with('['),
                "{t:?} has no sentence"
            );
        }
    }
}
