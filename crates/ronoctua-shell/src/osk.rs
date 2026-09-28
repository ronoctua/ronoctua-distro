//! The on-screen keyboard (`LEADER m k`) — where its keys are, what they
//! say, and what pressing them sends.
//!
//! # Where it is drawn
//!
//! At the BOTTOM of the footer, below the expansion and below the brain's
//! line: the footer is the one surface that grows (invariant 14), and a
//! keyboard is the thing a finger reaches for at the bottom of a screen. It
//! is counted up from the surface's bottom edge and never from the region
//! above it — `the-footer-does-not-ask-itself` is about exactly that sum.
//!
//! # What its legends are
//!
//! NOT A TABLE OF LETTERS. A key is named by its KERNEL code, and what it
//! types — and so what it says — is read from the xkb layout the person
//! chose on the keyboard itself: `ç` sits on `AC10` in `br` because `br`
//! puts it there, `AB11` exists only where a layout gives it a symbol, and
//! a dead accent is a dead accent the receiving window composes. A new
//! language is a layout name, not a drawing.
//!
//! # Why nothing here repeats a key
//!
//! Because the WINDOW does. A key held down is a key held down: the press
//! goes out, the release goes out when the finger lifts, and the receiving
//! window repeats it at the rate the compositor gave it — exactly as it does
//! for a physical key. A timer here would be a second, different repeat.
//!
//! # Why a modifier is three states
//!
//! One finger cannot hold Ctrl and press C on a phone-sized key. So a tap
//! LATCHES a modifier for the next key, a second tap LOCKS it until a third,
//! and a modifier held down while another finger types is simply held —
//! [`Mods`], which is a pure machine with tests.
//!
//! Everything in this file is arithmetic and xkb; the footer (`surface.rs`)
//! draws what [`Board::plan`] places, hit-tests the SAME plan, and hands what
//! [`Board::press`]/[`Board::release`] answer to `crate::virtual_keys`.

use ronoctua_tokens as tokens;
use serde::{Deserialize, Serialize};
use xkbcommon::xkb;

/// The person's settings file, in the configuration directory.
pub const FILE: &str = "osk.json";

/// The languages the keyboard offers, after the machine's own.
///
/// xkb layout names. The two the owner asked for are here (`br`, `us`), and
/// the rest are the other languages this distro is likely to be typed in.
/// Any xkb layout works the same way; this is only the list the settings face
/// draws a key for.
pub const OFFERED: &[&str] = &["us", "gb", "br", "pt", "es", "fr", "de", "it"];

/// The languages on when nobody has chosen, after the machine's own.
const ON_BY_DEFAULT: &[&str] = &["br", "us"];

/// A modifier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    /// Shift.
    Shift,
    /// Control.
    Ctrl,
    /// Alt, the left one.
    Alt,
    /// The right Alt, which in most layouts is the third level.
    AltGr,
    /// The logo key.
    Super,
}

impl Modifier {
    /// Every modifier, in the order [`Mods`] keeps them.
    pub const ALL: [Self; 5] = [Self::Shift, Self::Ctrl, Self::Alt, Self::AltGr, Self::Super];

    /// The kernel code it goes out as.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Shift => 42,
            Self::Ctrl => 29,
            Self::Alt => 56,
            Self::AltGr => 100,
            Self::Super => 125,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Shift => 0,
            Self::Ctrl => 1,
            Self::Alt => 2,
            Self::AltGr => 3,
            Self::Super => 4,
        }
    }
}

/// A key whose legend is ours rather than the layout's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    /// Escape.
    Esc,
    /// Tab.
    Tab,
    /// Caps Lock.
    Caps,
    /// Backspace.
    Backspace,
    /// Return.
    Return,
    /// The space bar.
    Space,
    /// The left arrow.
    Left,
    /// The right arrow.
    Right,
    /// The up arrow.
    Up,
    /// The down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Insert.
    Insert,
    /// Delete.
    Delete,
    /// F1 to F12.
    F(u8),
}

impl Named {
    /// The kernel code it goes out as.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Esc => 1,
            Self::Tab => 15,
            Self::Caps => 58,
            Self::Backspace => 14,
            Self::Return => 28,
            Self::Space => 57,
            Self::Left => 105,
            Self::Right => 106,
            Self::Up => 103,
            Self::Down => 108,
            Self::Home => 102,
            Self::End => 107,
            Self::PageUp => 104,
            Self::PageDown => 109,
            Self::Insert => 110,
            Self::Delete => 111,
            // F1..F10 are 59..68, and F11 and F12 were added later as 87, 88.
            Self::F(n) => match n {
                11 => 87,
                12 => 88,
                n => 58 + n as u16,
            },
        }
    }
}

/// How wide the keyboard is, a ring on the settings face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Width {
    /// `space.osk.cellsSmall`.
    Small,
    /// `space.osk.cellsMedium`.
    #[default]
    Medium,
    /// The whole screen.
    Whole,
}

impl Width {
    /// Every step, in the order the face draws them.
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Whole];

    /// How many cells it is on a screen `columns` wide.
    #[must_use]
    pub fn cells(self, columns: u16) -> u16 {
        match self {
            Self::Small => tokens::space::OSK_CELLSSMALL.min(columns),
            Self::Medium => tokens::space::OSK_CELLSMEDIUM.min(columns),
            Self::Whole => columns,
        }
    }

    /// The words on its key.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Self::Small => ronoctua_i18n::key::OSK_WIDTH_SMALL,
            Self::Medium => ronoctua_i18n::key::OSK_WIDTH_MEDIUM,
            Self::Whole => ronoctua_i18n::key::OSK_WIDTH_WHOLE,
        }
    }
}

/// How tall a key is — a ring on the settings face.
///
/// In QUARTERS OF A LINE, from `space.osk`, because the tallest step is two
/// lines and a half: three whole lines made the keyboard take a third of a
/// laptop's screen, and the owner asked for it a little shorter. Four rows of
/// keys always come to a whole number of lines (`4 × q / 4 = q`), so the
/// footer's height stays on the grid; it is only the rows inside it that may
/// start half a line in, and their legends are drawn there
/// (`ronoctua_render::Size::Lowered`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyHeight {
    /// One line.
    Low,
    /// Two lines — about a fingertip at the body size.
    #[default]
    Medium,
    /// Two and a half.
    Tall,
}

impl KeyHeight {
    /// Every step, in the order the face draws them.
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::Tall];

    /// How tall, in quarters of a line.
    #[must_use]
    pub const fn quarters(self) -> u16 {
        match self {
            Self::Low => tokens::space::OSK_LOWQUARTERS,
            Self::Medium => tokens::space::OSK_MEDIUMQUARTERS,
            Self::Tall => tokens::space::OSK_TALLQUARTERS,
        }
    }

    /// The words on its key.
    #[must_use]
    pub const fn words(self) -> &'static str {
        match self {
            Self::Low => ronoctua_i18n::key::OSK_HEIGHT_LOW,
            Self::Medium => ronoctua_i18n::key::OSK_HEIGHT_MEDIUM,
            Self::Tall => ronoctua_i18n::key::OSK_HEIGHT_TALL,
        }
    }
}

/// What a place on the keyboard is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A key that types what the layout puts on it, by kernel code.
    Types(u16),
    /// A key with a legend of ours.
    Named(Named),
    /// A modifier.
    Mod(Modifier),
    /// The function layer: F-keys on the number row, pages on the arrows.
    Function,
    /// The leader, opened in this process — see `Footer::activate_leader`.
    Leader,
    /// The next language.
    Language,
    /// The settings face, and back.
    Settings,
    /// Puts the keyboard away.
    Hide,
    /// Settings face: offer the `n`th language of [`Board::offered`], or stop.
    Offer(usize),
    /// Settings face: the key height.
    Height(KeyHeight),
    /// Settings face: the width.
    Width(Width),
    /// Settings face: the words that name a row. Drawn, and not a key.
    Label(&'static str),
}

impl Key {
    /// Whether it is a key at all — a label is words beside the keys.
    #[must_use]
    pub const fn is_a_key(self) -> bool {
        !matches!(self, Self::Label(_))
    }
}

/// One key of a row, with its width in QUARTERS of a standard key.
#[derive(Debug, Clone, Copy)]
struct Def {
    key: Key,
    quarters: u16,
}

const fn t(code: u16) -> Def {
    Def {
        key: Key::Types(code),
        quarters: 4,
    }
}

const fn n(named: Named, quarters: u16) -> Def {
    Def {
        key: Key::Named(named),
        quarters,
    }
}

const fn m(modifier: Modifier, quarters: u16) -> Def {
    Def {
        key: Key::Mod(modifier),
        quarters,
    }
}

const fn f(n: u8) -> Def {
    Def {
        key: Key::Named(Named::F(n)),
        quarters: 4,
    }
}

// THE THREE BLOCKS, side by side and each four rows tall: the keyboard's own
// controls in a column on the left, the letters in the middle, the digits on
// the right in the shape of a keypad. The owner's arrangement — a strip of
// controls on top and a row of digits under it made the keyboard two rows
// taller than the four the letters need.
//
// The letter rows are 62 quarters each (15½ standard keys), so a letter sits
// under the same width on every row. A key a layout gives no symbol is dropped
// and its width goes to the modifier beside it — see `absorbed`.

/// The column of the keyboard's own keys, top to bottom.
const CONTROLS: [Key; 4] = [Key::Hide, Key::Settings, Key::Language, Key::Leader];

const UPPER: &[Def] = &[
    n(Named::Tab, 6),
    t(16),
    t(17),
    t(18),
    t(19),
    t(20),
    t(21),
    t(22),
    t(23),
    t(24),
    t(25),
    t(26),
    t(27),
    n(Named::Backspace, 8),
];

const MIDDLE: &[Def] = &[
    n(Named::Caps, 7),
    t(30),
    t(31),
    t(32),
    t(33),
    t(34),
    t(35),
    t(36),
    t(37),
    t(38),
    t(39),
    t(40),
    n(Named::Return, 11),
];

const LOWER: &[Def] = &[
    m(Modifier::Shift, 5),
    // `LSGT`, the ISO key left of Z: `\|` in gb and in br, absent in us.
    t(86),
    t(44),
    t(45),
    t(46),
    t(47),
    t(48),
    t(49),
    t(50),
    t(51),
    t(52),
    t(53),
    // `AB11`, ABNT2's `/?` right of `;:` — only a Brazilian layout has one.
    t(89),
    m(Modifier::Shift, 9),
];

const BOTTOM: &[Def] = &[
    n(Named::Esc, 4),
    Def {
        key: Key::Function,
        quarters: 4,
    },
    m(Modifier::Ctrl, 5),
    m(Modifier::Super, 4),
    m(Modifier::Alt, 4),
    n(Named::Space, 21),
    m(Modifier::AltGr, 4),
    n(Named::Left, 4),
    n(Named::Down, 4),
    n(Named::Up, 4),
    n(Named::Right, 4),
];

// THE DIGITS, as a keypad: 7 8 9 on top and 0 wide at the bottom, with the
// four keys the number row carried beside them (` - = \). Kernel codes: `1` is
// 2 and `0` is 11 — the number row's own keys, so each types what the layout
// puts there, `!` and `@` on Shift included.
const DIGITS: [&[Def]; 4] = [
    &[t(41), t(8), t(9), t(10)],
    &[t(12), t(5), t(6), t(7)],
    &[t(13), t(2), t(3), t(4)],
    &[
        t(43),
        Def {
            key: Key::Types(11),
            quarters: 12,
        },
    ],
];

// AND THE SAME BLOCK ON THE FUNCTION LAYER: the twelve F-keys, Insert and
// Delete.
const FUNCTIONS: [&[Def]; 4] = [
    &[f(1), f(2), f(3), f(4)],
    &[f(5), f(6), f(7), f(8)],
    &[f(9), f(10), f(11), f(12)],
    &[n(Named::Insert, 4), n(Named::Delete, 12)],
];

/// What the arrows become on the function layer.
const fn on_the_function_layer(named: Named) -> Named {
    match named {
        Named::Left => Named::Home,
        Named::Down => Named::PageDown,
        Named::Up => Named::PageUp,
        Named::Right => Named::End,
        other => other,
    }
}

/// The widths of the blocks across, in quarters: the controls, air, the
/// letters, air, the digits.
const CONTROLS_QUARTERS: u16 = 6;
const AIR_QUARTERS: u16 = 2;
const LETTERS_QUARTERS: u16 = 62;
const DIGITS_QUARTERS: u16 = 16;
const ACROSS_QUARTERS: u16 =
    CONTROLS_QUARTERS + AIR_QUARTERS + LETTERS_QUARTERS + AIR_QUARTERS + DIGITS_QUARTERS;

/// How many rows of keys, in every block and on both faces.
pub const ROWS: u16 = 4;

/// Which face the keys area shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Face {
    /// The keys.
    #[default]
    Keys,
    /// The keyboard's own settings, drawn as keys too.
    Settings,
}

/// One key, placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cap {
    /// What it is.
    pub key: Key,
    /// Its first column, counted from the surface's left edge.
    pub column: u16,
    /// How many columns wide.
    pub cells: u16,
    /// Its top, in QUARTERS OF A LINE from the keyboard's top.
    pub top: u16,
    /// How tall, in quarters of a line.
    pub tall: u16,
}

impl Cap {
    /// Whether the cell `column`, at `quarter` quarters of a line below the
    /// keyboard's top, is on this key.
    #[must_use]
    pub const fn covers(&self, column: u16, quarter: u16) -> bool {
        column >= self.column
            && column < self.column + self.cells
            && quarter >= self.top
            && quarter < self.top + self.tall
    }
}

/// How many lines the keyboard takes with keys `height` tall: four rows of
/// them, which is a whole number of lines by construction.
#[must_use]
pub const fn lines_of(height: KeyHeight) -> u16 {
    ROWS * height.quarters() / 4
}

/// Lays one row out across `width` cells from `left`, in proportion.
///
/// THE SAME FLOOR ON BOTH EDGES of every key — `left + width·sum/total` —
/// so the keys tile the row exactly, with nothing left over and nothing
/// overlapping, whatever the width.
fn lay_row(caps: &mut Vec<Cap>, row: &[(Key, u16)], left: u16, width: u16, top: u16, tall: u16) {
    let total: u32 = row.iter().map(|(_, q)| u32::from(*q)).sum();
    if total == 0 {
        return;
    }
    let mut sum = 0_u32;
    for (key, q) in row {
        let from = u32::from(width) * sum / total;
        sum += u32::from(*q);
        let to = u32::from(width) * sum / total;
        let cells = u16::try_from(to - from).unwrap_or(0);
        if cells == 0 {
            continue;
        }
        caps.push(Cap {
            key: *key,
            column: left + u16::try_from(from).unwrap_or(0),
            cells,
            top,
            tall,
        });
    }
}

/// A row with the keys a layout gives no symbol taken out, their width given
/// to the MODIFIER next to them (the Shift beside `LSGT` and `AB11`), so the
/// letters keep their width and only the long key grows.
fn absorbed(defs: &[Def], shows: &dyn Fn(Key) -> bool) -> Vec<(Key, u16)> {
    let mut row: Vec<(Key, u16)> = defs.iter().map(|d| (d.key, d.quarters)).collect();
    let mut at = 0;
    while at < row.len() {
        if shows(row[at].0) {
            at += 1;
            continue;
        }
        let (_, q) = row.remove(at);
        let long = |k: Key| matches!(k, Key::Mod(_) | Key::Named(_) | Key::Function);
        if at > 0 && long(row[at - 1].0) {
            row[at - 1].1 += q;
        } else if at < row.len() && long(row[at].0) {
            row[at].1 += q;
        } else if at > 0 {
            row[at - 1].1 += q;
        }
    }
    row
}

/// The whole keyboard laid out on a screen `columns` wide.
///
/// ONE FUNCTION, READ TWICE: the footer paints these caps and asks the SAME
/// caps which one a point is on. A control is taken where it is drawn.
#[must_use]
pub fn place(
    columns: u16,
    width: Width,
    height: KeyHeight,
    face: Face,
    function: bool,
    offered: usize,
    shows: &dyn Fn(Key) -> bool,
) -> Vec<Cap> {
    let width_cells = width.cells(columns);
    let left = (columns - width_cells) / 2;
    let tall = height.quarters();
    // WHERE EACH BLOCK STARTS AND ENDS, by the same floor the keys use.
    let edge = |q: u16| {
        left + u16::try_from(u32::from(width_cells) * u32::from(q) / u32::from(ACROSS_QUARTERS))
            .unwrap_or(0)
    };
    let controls = (edge(0), edge(CONTROLS_QUARTERS));
    let letters = (
        edge(CONTROLS_QUARTERS + AIR_QUARTERS),
        edge(CONTROLS_QUARTERS + AIR_QUARTERS + LETTERS_QUARTERS),
    );
    let digits = (
        edge(ACROSS_QUARTERS - DIGITS_QUARTERS),
        edge(ACROSS_QUARTERS),
    );
    let mut caps = Vec::new();
    for (r, key) in (0_u16..).zip(CONTROLS) {
        caps.push(Cap {
            key,
            column: controls.0,
            cells: controls.1 - controls.0,
            top: r * tall,
            tall,
        });
    }
    match face {
        Face::Keys => {
            let bottom: Vec<Def> = BOTTOM
                .iter()
                .map(|d| match d.key {
                    Key::Named(nm) if function => Def {
                        key: Key::Named(on_the_function_layer(nm)),
                        quarters: d.quarters,
                    },
                    _ => *d,
                })
                .collect();
            for (r, defs) in (0_u16..).zip([UPPER, MIDDLE, LOWER, bottom.as_slice()]) {
                lay_row(
                    &mut caps,
                    &absorbed(defs, shows),
                    letters.0,
                    letters.1 - letters.0,
                    r * tall,
                    tall,
                );
            }
            let block = if function { FUNCTIONS } else { DIGITS };
            for (r, defs) in (0_u16..).zip(block) {
                lay_row(
                    &mut caps,
                    &absorbed(defs, shows),
                    digits.0,
                    digits.1 - digits.0,
                    r * tall,
                    tall,
                );
            }
        }
        Face::Settings => {
            // THE SETTINGS TAKE THE LETTERS' AND THE DIGITS' ROOM, and the
            // column of controls stays where it is — its settings key, lit, is
            // the way back.
            let from = letters.0;
            let across = digits.1 - letters.0;
            let label = tokens::space::OSK_LABEL.min(across / 4);
            let rows: [(&'static str, Vec<Key>); 3] = [
                (
                    ronoctua_i18n::key::OSK_SETTING_LANGUAGES,
                    (0..offered).map(Key::Offer).collect(),
                ),
                (
                    ronoctua_i18n::key::OSK_SETTING_HEIGHT,
                    KeyHeight::ALL.iter().map(|h| Key::Height(*h)).collect(),
                ),
                (
                    ronoctua_i18n::key::OSK_SETTING_WIDTH,
                    Width::ALL.iter().map(|w| Key::Width(*w)).collect(),
                ),
            ];
            for (r, (words, keys)) in (0_u16..).zip(rows) {
                caps.push(Cap {
                    key: Key::Label(words),
                    column: from,
                    cells: label,
                    top: r * tall,
                    tall,
                });
                let row: Vec<(Key, u16)> = keys.into_iter().map(|k| (k, 4)).collect();
                lay_row(
                    &mut caps,
                    &row,
                    from + label,
                    across - label,
                    r * tall,
                    tall,
                );
            }
        }
    }
    caps
}

/// Where a modifier stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hold {
    /// Not in effect.
    #[default]
    Off,
    /// In effect for the next key, then off.
    Latched,
    /// In effect until tapped again.
    Locked,
}

/// The five modifiers.
#[derive(Debug, Clone, Default)]
pub struct Mods {
    holds: [Hold; 5],
    /// Held down by a finger or a button right now.
    down: [bool; 5],
    /// Whether a key was typed while it was held down — which makes its
    /// release the end of a HOLD rather than a tap.
    used: [bool; 5],
    /// Where it stood when it was pressed.
    before: [Hold; 5],
}

impl Mods {
    /// Whether `m` is in effect.
    #[must_use]
    pub const fn active(&self, m: Modifier) -> bool {
        let i = m.index();
        self.down[i] || !matches!(self.holds[i], Hold::Off)
    }

    /// Where `m` stands, for its colour.
    #[must_use]
    pub const fn hold(&self, m: Modifier) -> Hold {
        self.holds[m.index()]
    }

    /// `m` went down.
    pub const fn press(&mut self, m: Modifier) {
        let i = m.index();
        self.before[i] = self.holds[i];
        self.down[i] = true;
        self.used[i] = false;
    }

    /// `m` came up: a TAP walks the ring, a HOLD ends.
    pub const fn release(&mut self, m: Modifier) {
        let i = m.index();
        if !self.down[i] {
            return;
        }
        self.down[i] = false;
        self.holds[i] = if self.used[i] {
            match self.before[i] {
                Hold::Locked => Hold::Locked,
                _ => Hold::Off,
            }
        } else {
            match self.before[i] {
                Hold::Off => Hold::Latched,
                Hold::Latched => Hold::Locked,
                Hold::Locked => Hold::Off,
            }
        };
    }

    /// A typing key went down while these stand.
    pub const fn typing(&mut self) {
        let mut i = 0;
        while i < 5 {
            if self.down[i] {
                self.used[i] = true;
            }
            i += 1;
        }
    }

    /// A typing key came up: a latch has been spent.
    pub const fn typed(&mut self) {
        let mut i = 0;
        while i < 5 {
            if !self.down[i] && matches!(self.holds[i], Hold::Latched) {
                self.holds[i] = Hold::Off;
            }
            i += 1;
        }
    }

    /// Everything off — the keyboard is being put away.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn active_set(&self) -> [bool; 5] {
        Modifier::ALL.map(|m| self.active(m))
    }
}

/// The person's choices, in `osk.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// The languages turned on, as xkb names (`br`, `us(intl)`). `None` is
    /// "never chosen", which is the machine's own plus [`ON_BY_DEFAULT`].
    pub languages: Option<Vec<String>>,
    /// The one in use.
    pub language: Option<String>,
    /// How tall a key is.
    pub height: KeyHeight,
    /// How wide the keyboard is.
    pub width: Width,
}

/// Where the settings file is.
#[must_use]
pub fn home() -> Option<std::path::PathBuf> {
    ronoctua_store::config_dir()
}

/// The store the settings live in.
#[must_use]
pub fn store() -> ronoctua_store::Store<Prefs> {
    ronoctua_store::Store::at(home(), FILE)
}

/// The machine's own language: its FIRST layout, when it declares several —
/// a keyboard of our own cycles its own list.
#[must_use]
pub fn machine_language() -> Language {
    let machine = ronoctua_pointers::keyboards::Machine::from_env();
    let first = |s: &str| s.split(',').next().unwrap_or("").trim().to_owned();
    let layout = first(&machine.layout);
    Language {
        layout: if layout.is_empty() {
            "us".to_owned()
        } else {
            layout
        },
        variant: first(&machine.variant),
    }
}

/// The keymap text of `language`, as the virtual keyboard is given it —
/// for keys sent with the keyboard put away (the gestures' `Esc` and arrows).
#[must_use]
pub fn keymap_of(language: &Language) -> Option<String> {
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    xkb::Keymap::new_from_names(
        &context,
        "",
        "",
        &language.layout,
        &language.variant,
        None,
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .map(|k| k.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1))
}

/// A language: an xkb layout and its variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    /// The layout, `br`.
    pub layout: String,
    /// The variant, often empty.
    pub variant: String,
}

impl Language {
    /// From `br` or `us(intl)`.
    #[must_use]
    pub fn parse(id: &str) -> Self {
        match id.split_once('(') {
            Some((layout, rest)) => Self {
                layout: layout.to_owned(),
                variant: rest.trim_end_matches(')').to_owned(),
            },
            None => Self {
                layout: id.to_owned(),
                variant: String::new(),
            },
        }
    }

    /// Back to `br` or `us(intl)`.
    #[must_use]
    pub fn id(&self) -> String {
        if self.variant.is_empty() {
            self.layout.clone()
        } else {
            format!("{}({})", self.layout, self.variant)
        }
    }

    /// What its key says: the layout's name in capitals. Data, not prose —
    /// it is the name xkb gives it, and the one every keyboard prints.
    #[must_use]
    pub fn code(&self) -> String {
        self.layout.to_uppercase()
    }
}

/// What the footer must do after a press or a release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    /// A key down (`true`) or up, to the window, by kernel code.
    Key(u16, bool),
    /// Open the leader.
    Leader,
    /// Put the keyboard away.
    Hide,
    /// The settings changed and must be written.
    Keep,
    /// A sentence for the message line, by key.
    Say(&'static str),
}

/// Who pressed a key: a finger, by its touch id, or a pointer's button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finger {
    /// A touch point.
    Touch(i32),
    /// A pointer, by its object's protocol id.
    Pointer(u32),
}

/// A layout compiled once.
struct Compiled {
    language: Language,
    keymap: xkb::Keymap,
    text: String,
}

impl std::fmt::Debug for Compiled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compiled")
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

/// How a key is shaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A key that types a character.
    Cap,
    /// Any other key.
    Action,
    /// Held down, latched, or the setting in effect.
    On,
    /// A locked modifier.
    Locked,
    /// Words, no ground.
    Bare,
}

/// What a key says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Legend {
    /// Characters, from the layout.
    Text(String),
    /// A glyph token.
    Glyph(char),
    /// Words, by text key.
    Words(&'static str),
    /// A glyph and a short code beside it.
    GlyphAnd(char, String),
    /// `F` and a number.
    Function(u8),
}

/// The keyboard: its settings, what is held, and the compiled layouts.
#[derive(Debug)]
pub struct Board {
    /// The settings.
    pub prefs: Prefs,
    /// Which face is up.
    pub face: Face,
    /// Whether the function layer is up.
    pub function: bool,
    mods: Mods,
    caps: bool,
    down: Vec<(Finger, Key)>,
    compiled: Vec<Compiled>,
    machine: Language,
    context: Context,
}

/// xkb's context, which is not `Debug`.
struct Context(xkb::Context);

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Context")
    }
}

impl Board {
    /// A keyboard with these settings, on a machine whose own layout is
    /// `machine`.
    #[must_use]
    pub fn new(prefs: Prefs, machine: Language) -> Self {
        Self {
            prefs,
            face: Face::Keys,
            function: false,
            mods: Mods::default(),
            caps: false,
            down: Vec::new(),
            compiled: Vec::new(),
            machine,
            context: Context(xkb::Context::new(xkb::CONTEXT_NO_FLAGS)),
        }
    }

    /// Every language the settings face offers: the machine's first.
    #[must_use]
    pub fn offered(&self) -> Vec<Language> {
        let mut all = vec![self.machine.clone()];
        for id in OFFERED {
            let l = Language::parse(id);
            if !all.contains(&l) {
                all.push(l);
            }
        }
        // AND WHATEVER THE PERSON TURNED ON BY HAND in the file, which the
        // face must show to let them turn it off again.
        for l in self.enabled() {
            if !all.contains(&l) {
                all.push(l);
            }
        }
        all
    }

    /// The languages turned on, in the order they cycle.
    #[must_use]
    pub fn enabled(&self) -> Vec<Language> {
        let mut on: Vec<Language> = match &self.prefs.languages {
            Some(ids) => ids.iter().map(|i| Language::parse(i)).collect(),
            None => {
                let mut v = vec![self.machine.clone()];
                v.extend(ON_BY_DEFAULT.iter().map(|i| Language::parse(i)));
                v
            }
        };
        let mut seen = Vec::new();
        on.retain(|l| {
            let fresh = !seen.contains(l);
            seen.push(l.clone());
            fresh
        });
        if on.is_empty() {
            on.push(self.machine.clone());
        }
        on
    }

    /// The language in use.
    #[must_use]
    pub fn language(&self) -> Language {
        let on = self.enabled();
        self.prefs
            .language
            .as_deref()
            .map(Language::parse)
            .filter(|l| on.contains(l))
            .unwrap_or_else(|| on[0].clone())
    }

    /// How many lines the keyboard takes.
    #[must_use]
    pub fn lines(&self) -> u16 {
        lines_of(self.prefs.height)
    }

    fn compiled(&mut self) -> Option<&Compiled> {
        let language = self.language();
        if let Some(at) = self.compiled.iter().position(|c| c.language == language) {
            return self.compiled.get(at);
        }
        let keymap = xkb::Keymap::new_from_names(
            &self.context.0,
            "",
            "",
            &language.layout,
            &language.variant,
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        );
        let Some(keymap) = keymap else {
            tracing::warn!(layout = %language.id(), "xkb does not know this layout");
            return None;
        };
        let text = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
        self.compiled.push(Compiled {
            language,
            keymap,
            text,
        });
        self.compiled.last()
    }

    /// The keymap of the language in use, as the virtual keyboard is given it.
    pub fn keymap_text(&mut self) -> Option<String> {
        self.compiled().map(|c| c.text.clone())
    }

    /// The caps on a screen `columns` wide.
    pub fn plan(&mut self, columns: u16) -> Vec<Cap> {
        let offered = self.offered().len();
        let (width, height, face, function) = (
            self.prefs.width,
            self.prefs.height,
            self.face,
            self.function,
        );
        let compiled = self.compiled();
        let shows = |k: Key| match k {
            Key::Types(code) => compiled.is_some_and(|c| {
                !c.keymap
                    .key_get_syms_by_level(keycode(code), 0, 0)
                    .is_empty()
            }),
            _ => true,
        };
        place(columns, width, height, face, function, offered, &shows)
    }

    /// The legend of `key`, and the smaller second one a tall key carries
    /// (what Shift and AltGr would type on it).
    pub fn legend(&mut self, key: Key) -> (Legend, Option<String>) {
        use tokens::icon;
        match key {
            Key::Types(code) => self.typed_legend(code),
            Key::Named(nm) => (
                match nm {
                    Named::Esc => Legend::Words(ronoctua_i18n::key::OSK_KEY_ESC),
                    Named::Tab => Legend::Glyph(icon::OSK_TAB),
                    Named::Caps => Legend::Glyph(icon::OSK_CAPS),
                    Named::Backspace => Legend::Glyph(icon::OSK_BACKSPACE),
                    Named::Return => Legend::Glyph(icon::OSK_RETURN),
                    // THE SPACE BAR SAYS THE LANGUAGE, as every phone's does:
                    // it is the one place the eye always crosses.
                    Named::Space => Legend::Text(self.language().code()),
                    Named::Left => Legend::Glyph(tokens::mark::ARROW_LEFT),
                    Named::Right => Legend::Glyph(tokens::mark::ARROW_RIGHT),
                    Named::Up => Legend::Glyph(tokens::mark::ARROW_UP),
                    Named::Down => Legend::Glyph(tokens::mark::ARROW_DOWN),
                    Named::Home => Legend::Words(ronoctua_i18n::key::OSK_KEY_HOME),
                    Named::End => Legend::Words(ronoctua_i18n::key::OSK_KEY_END),
                    Named::PageUp => Legend::Words(ronoctua_i18n::key::OSK_KEY_PAGE_UP),
                    Named::PageDown => Legend::Words(ronoctua_i18n::key::OSK_KEY_PAGE_DOWN),
                    Named::Insert => Legend::Words(ronoctua_i18n::key::OSK_KEY_INSERT),
                    Named::Delete => Legend::Words(ronoctua_i18n::key::OSK_KEY_DELETE),
                    Named::F(n) => Legend::Function(n),
                },
                None,
            ),
            Key::Mod(md) => (
                match md {
                    Modifier::Shift => Legend::Glyph(icon::OSK_SHIFT),
                    Modifier::Ctrl => Legend::Words(ronoctua_i18n::key::OSK_KEY_CTRL),
                    Modifier::Alt => Legend::Words(ronoctua_i18n::key::OSK_KEY_ALT),
                    Modifier::AltGr => Legend::Words(ronoctua_i18n::key::OSK_KEY_ALTGR),
                    Modifier::Super => Legend::Words(ronoctua_i18n::key::OSK_KEY_SUPER),
                },
                None,
            ),
            Key::Function => (Legend::Words(ronoctua_i18n::key::OSK_KEY_FN), None),
            Key::Leader => (Legend::Words(ronoctua_i18n::key::OSK_KEY_LEADER), None),
            Key::Language => (
                Legend::GlyphAnd(icon::OSK_LANGUAGE, self.language().code()),
                None,
            ),
            Key::Settings => (Legend::Glyph(icon::OSK_SETTINGS), None),
            Key::Hide => (Legend::Glyph(icon::OSK_HIDE), None),
            Key::Offer(i) => (
                Legend::Text(
                    self.offered()
                        .get(i)
                        .map(Language::code)
                        .unwrap_or_default(),
                ),
                None,
            ),
            Key::Height(h) => (Legend::Words(h.words()), None),
            Key::Width(w) => (Legend::Words(w.words()), None),
            Key::Label(words) => (Legend::Words(words), None),
        }
    }

    fn typed_legend(&mut self, code: u16) -> (Legend, Option<String>) {
        let active: Vec<Modifier> = Modifier::ALL
            .into_iter()
            .filter(|md| self.mods.active(*md))
            .collect();
        let caps = self.caps;
        let Some(c) = self.compiled() else {
            return (Legend::Text(String::new()), None);
        };
        let kc = keycode(code);
        let mut state = xkb::State::new(&c.keymap);
        for md in active {
            state.update_key(keycode(md.code()), xkb::KeyDirection::Down);
        }
        if caps {
            let lock = keycode(Named::Caps.code());
            state.update_key(lock, xkb::KeyDirection::Down);
            state.update_key(lock, xkb::KeyDirection::Up);
        }
        let big = shown(state.key_get_one_sym(kc));
        let level = |l: u32| {
            c.keymap
                .key_get_syms_by_level(kc, 0, l)
                .first()
                .map(|s| shown(*s))
                .unwrap_or_default()
        };
        let (base, shifted, third) = (level(0), level(1), level(2));
        let mut small = Vec::new();
        for other in [&base, &shifted, &third] {
            let a_case = other.to_uppercase() == big.to_uppercase();
            if !other.is_empty() && *other != big && !a_case && !small.contains(other) {
                small.push(other.clone());
            }
        }
        let small = (!small.is_empty()).then(|| small.join(" "));
        (Legend::Text(big), small)
    }

    /// How `key` is shaded right now.
    #[must_use]
    pub fn tone(&self, key: Key) -> Tone {
        let pressed = self.down.iter().any(|(_, k)| *k == key);
        match key {
            Key::Label(_) => Tone::Bare,
            Key::Mod(md) => match self.mods.hold(md) {
                Hold::Locked => Tone::Locked,
                _ if self.mods.active(md) => Tone::On,
                _ => Tone::Action,
            },
            Key::Named(Named::Caps) if self.caps => Tone::Locked,
            Key::Function if self.function => Tone::On,
            Key::Settings if self.face == Face::Settings => Tone::On,
            Key::Offer(i) => {
                let on = self.enabled();
                if self.offered().get(i).is_some_and(|l| on.contains(l)) {
                    Tone::On
                } else {
                    Tone::Cap
                }
            }
            Key::Height(h) if self.prefs.height == h => Tone::On,
            Key::Width(w) if self.prefs.width == w => Tone::On,
            _ if pressed => Tone::On,
            Key::Types(_) | Key::Named(Named::Space) => Tone::Cap,
            Key::Height(_) | Key::Width(_) => Tone::Cap,
            _ => Tone::Action,
        }
    }

    /// The sentence the message line gives for `key` under the pointer, by
    /// key and with its argument — `None` for a key whose legend says it all.
    #[must_use]
    pub fn about(&self, key: Key) -> Option<(&'static str, Vec<(&'static str, String)>)> {
        use ronoctua_i18n::key as k;
        Some(match key {
            Key::Language => (
                k::ABOUT_OSK_LANGUAGE,
                vec![(
                    "languages",
                    self.enabled()
                        .iter()
                        .map(Language::code)
                        .collect::<Vec<_>>()
                        .join(" · "),
                )],
            ),
            Key::Settings => (k::ABOUT_OSK_SETTINGS, vec![]),
            Key::Hide => (k::ABOUT_OSK_HIDE, vec![]),
            Key::Leader => (k::ABOUT_OSK_LEADER, vec![]),
            Key::Function => (k::ABOUT_OSK_FN, vec![]),
            Key::Mod(_) => (k::ABOUT_OSK_MODIFIER, vec![]),
            Key::Offer(i) => (
                k::ABOUT_OSK_CHOICE,
                vec![(
                    "name",
                    self.offered().get(i).map(Language::id).unwrap_or_default(),
                )],
            ),
            Key::Height(_) => (k::ABOUT_OSK_HEIGHT, vec![]),
            Key::Width(_) => (k::ABOUT_OSK_WIDTH, vec![]),
            _ => return None,
        })
    }

    /// Whether anything is held down or in effect — the keyboard answers
    /// the pointer leaving by letting go of what it pressed.
    #[must_use]
    pub fn is_pressed_by(&self, finger: Finger) -> bool {
        self.down.iter().any(|(f, _)| *f == finger)
    }

    /// `finger` went down on `key`.
    pub fn press(&mut self, finger: Finger, key: Key) -> Vec<Out> {
        if !key.is_a_key() || self.is_pressed_by(finger) {
            return Vec::new();
        }
        let before = self.mods.active_set();
        let mut out = Vec::new();
        match key {
            Key::Types(code) => {
                self.mods.typing();
                out.push(Out::Key(code, true));
            }
            Key::Named(nm) => {
                self.mods.typing();
                if nm == Named::Caps {
                    self.caps = !self.caps;
                }
                out.push(Out::Key(nm.code(), true));
            }
            Key::Mod(md) => self.mods.press(md),
            _ => {}
        }
        self.down.push((finger, key));
        let mut changed = self.changed_mods(before);
        // A MODIFIER GOES DOWN BEFORE THE KEY IT MODIFIES — it never does
        // here, since typing does not move a modifier, but the order is the
        // one a physical keyboard gives and the one a window expects.
        changed.extend(out);
        changed
    }

    /// `finger` came up.
    pub fn release(&mut self, finger: Finger) -> Vec<Out> {
        let Some(at) = self.down.iter().position(|(f, _)| *f == finger) else {
            return Vec::new();
        };
        let (_, key) = self.down.remove(at);
        let before = self.mods.active_set();
        let mut out = Vec::new();
        match key {
            Key::Types(code) => {
                out.push(Out::Key(code, false));
                self.mods.typed();
            }
            Key::Named(nm) => {
                out.push(Out::Key(nm.code(), false));
                self.mods.typed();
            }
            Key::Mod(md) => self.mods.release(md),
            Key::Function => self.function = !self.function,
            Key::Leader => out.push(Out::Leader),
            Key::Hide => out.push(Out::Hide),
            Key::Settings => {
                self.face = match self.face {
                    Face::Keys => Face::Settings,
                    Face::Settings => Face::Keys,
                };
            }
            Key::Language => {
                let on = self.enabled();
                let now = self.language();
                let at = on.iter().position(|l| *l == now).unwrap_or(0);
                let next = on[(at + 1) % on.len()].clone();
                self.prefs.language = Some(next.id());
                out.push(Out::Keep);
            }
            Key::Offer(i) => {
                if let Some(l) = self.offered().get(i).cloned() {
                    let mut on = self.enabled();
                    if on.contains(&l) {
                        if on.len() == 1 {
                            out.push(Out::Say(ronoctua_i18n::key::ECHO_OSK_LAST_LANGUAGE));
                        } else {
                            on.retain(|x| *x != l);
                        }
                    } else {
                        on.push(l);
                    }
                    self.prefs.languages = Some(on.iter().map(Language::id).collect());
                    out.push(Out::Keep);
                }
            }
            Key::Height(h) => {
                self.prefs.height = h;
                out.push(Out::Keep);
            }
            Key::Width(w) => {
                self.prefs.width = w;
                out.push(Out::Keep);
            }
            Key::Label(_) => {}
        }
        out.extend(self.changed_mods(before));
        out
    }

    /// Everything up, every modifier off — the keyboard is going away, or
    /// the finger was lost.
    pub fn let_go(&mut self) -> Vec<Out> {
        let before = self.mods.active_set();
        let mut out: Vec<Out> = self
            .down
            .drain(..)
            .filter_map(|(_, k)| match k {
                Key::Types(code) => Some(Out::Key(code, false)),
                Key::Named(nm) => Some(Out::Key(nm.code(), false)),
                _ => None,
            })
            .collect();
        self.mods.clear();
        out.extend(self.changed_mods(before));
        out
    }

    /// An `Alt` or a `Ctrl` TAPPED ALONE — latched, with no typing key since
    /// and nothing else in effect — let go at once: the modifier, and the
    /// key-up that goes out for it.
    ///
    /// # Why, and only when the footer asks
    ///
    /// At the leader's root a tap of `Alt` or `Ctrl` chooses the leader's set
    /// (`crate::leader::Tap`), and this keyboard cannot tap: a tap LATCHES,
    /// the key-up waits for the next key typed, and by then that key has made
    /// it a chord — `LEADER M-s is not bound`. The footer asks this where the
    /// tap means a set, takes the set itself, and sends the key-up. Everywhere
    /// else the latch is what it always was: `Ctrl` for the next key.
    pub fn spend_a_lone_tap(&mut self) -> Option<(Modifier, Vec<Out>)> {
        // NO FINGER ON ANY KEY: a key still down is a chord under way.
        if !self.down.is_empty() {
            return None;
        }
        let lone = [Modifier::Alt, Modifier::Ctrl]
            .into_iter()
            .find(|m| self.mods.hold(*m) == Hold::Latched)?;
        // AND NOTHING ELSE IN EFFECT: two modifiers are a chord.
        if Modifier::ALL
            .into_iter()
            .any(|m| m != lone && self.mods.active(m))
        {
            return None;
        }
        let before = self.mods.active_set();
        self.mods.holds[lone.index()] = Hold::Off;
        Some((lone, self.changed_mods(before)))
    }

    fn changed_mods(&self, before: [bool; 5]) -> Vec<Out> {
        Modifier::ALL
            .into_iter()
            .zip(before)
            .filter(|(md, was)| self.mods.active(*md) != *was)
            .map(|(md, _)| Out::Key(md.code(), self.mods.active(md)))
            .collect()
    }
}

/// The xkb keycode of a kernel code: eight more, which is what xkb counts in.
fn keycode(code: u16) -> xkb::Keycode {
    xkb::Keycode::new(u32::from(code) + 8)
}

/// What a keysym is drawn as on its key.
///
/// The character it types, when it types one; and for a DEAD key, the accent
/// it will put on the next letter — a dead key types nothing by itself, and a
/// key with nothing on it would be a key nobody finds.
fn shown(sym: xkb::Keysym) -> String {
    let text = xkb::keysym_to_utf8(sym);
    if !text.is_empty() && !text.chars().any(char::is_control) {
        return text;
    }
    let accent = match xkb::keysym_get_name(sym).as_str() {
        "dead_grave" => '\u{60}',
        "dead_acute" => '\u{B4}',
        "dead_circumflex" => '\u{5E}',
        "dead_tilde" => '\u{7E}',
        "dead_diaeresis" => '\u{A8}',
        "dead_cedilla" => '\u{B8}',
        "dead_macron" => '\u{AF}',
        "dead_breve" => '\u{2D8}',
        "dead_abovedot" => '\u{2D9}',
        "dead_abovering" => '\u{2DA}',
        "dead_doubleacute" => '\u{2DD}',
        "dead_caron" => '\u{2C7}',
        "dead_ogonek" => '\u{2DB}',
        _ => return String::new(),
    };
    accent.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(machine: &str) -> Board {
        Board::new(Prefs::default(), Language::parse(machine))
    }

    const M: KeyHeight = KeyHeight::Medium;

    #[test]
    fn every_row_of_every_block_tiles_its_block() {
        for width in [60_u16, 116, 160, 211] {
            let caps = place(width, Width::Whole, M, Face::Keys, false, 0, &|_| true);
            for r in 0..ROWS {
                let mut row: Vec<&Cap> =
                    caps.iter().filter(|c| c.top == r * M.quarters()).collect();
                row.sort_by_key(|c| c.column);
                // WITHIN a block the keys touch; BETWEEN blocks there is air.
                let touching = row
                    .windows(2)
                    .filter(|w| w[0].column + w[0].cells == w[1].column)
                    .count();
                let apart = row
                    .windows(2)
                    .filter(|w| w[0].column + w[0].cells < w[1].column)
                    .count();
                assert!(
                    row.windows(2)
                        .all(|w| w[0].column + w[0].cells <= w[1].column),
                    "an overlap"
                );
                assert_eq!(
                    apart, 2,
                    "three blocks, two gaps between them, width {width} row {r}"
                );
                assert_eq!(touching + apart + 1, row.len());
                assert_eq!(row.first().unwrap().column, 0);
                let last = row.last().unwrap();
                assert_eq!(last.column + last.cells, width);
            }
        }
    }

    #[test]
    fn four_rows_and_a_whole_number_of_lines() {
        for h in KeyHeight::ALL {
            let caps = place(160, Width::Whole, h, Face::Keys, false, 0, &|_| true);
            let bottom = caps.iter().map(|c| c.top + c.tall).max().unwrap();
            assert_eq!(bottom, ROWS * h.quarters());
            assert_eq!(bottom % 4, 0, "{h:?} is not a whole number of lines");
            assert_eq!(lines_of(h), bottom / 4);
        }
        assert!(
            lines_of(KeyHeight::Tall) < 11,
            "the tallest is shorter than before"
        );
    }

    #[test]
    fn the_controls_are_a_column_and_the_digits_a_block_beside() {
        let caps = place(160, Width::Whole, M, Face::Keys, false, 0, &|_| true);
        let at = |k: Key| caps.iter().find(|c| c.key == k).copied().unwrap();
        let (hide, leader) = (at(Key::Hide), at(Key::Leader));
        assert_eq!(hide.column, leader.column);
        assert!(leader.top > hide.top);
        let (q, seven, zero) = (at(Key::Types(16)), at(Key::Types(8)), at(Key::Types(11)));
        assert!(hide.column < q.column && q.column < seven.column);
        assert_eq!(seven.top, 0);
        assert_eq!(zero.top, 3 * M.quarters());
    }

    #[test]
    fn the_keyboard_is_centred_and_capped() {
        let caps = place(300, Width::Medium, M, Face::Keys, false, 0, &|_| true);
        let left = caps.iter().map(|c| c.column).min().unwrap();
        let right = caps.iter().map(|c| c.column + c.cells).max().unwrap();
        assert_eq!(right - left, tokens::space::OSK_CELLSMEDIUM);
        assert_eq!(left, (300 - tokens::space::OSK_CELLSMEDIUM) / 2);
    }

    #[test]
    fn a_point_is_on_at_most_one_key() {
        let caps = place(
            160,
            Width::Whole,
            KeyHeight::Tall,
            Face::Keys,
            false,
            0,
            &|_| true,
        );
        for quarter in 0..ROWS * KeyHeight::Tall.quarters() {
            for column in 0..160 {
                assert!(caps.iter().filter(|c| c.covers(column, quarter)).count() <= 1);
            }
        }
    }

    #[test]
    fn a_missing_key_gives_its_width_to_the_shift_beside_it() {
        let all = place(160, Width::Whole, M, Face::Keys, false, 0, &|_| true);
        let without = place(160, Width::Whole, M, Face::Keys, false, 0, &|k| {
            k != Key::Types(86)
        });
        let shift = |caps: &[Cap]| {
            caps.iter()
                .find(|c| c.key == Key::Mod(Modifier::Shift))
                .unwrap()
                .cells
        };
        let z = |caps: &[Cap]| caps.iter().find(|c| c.key == Key::Types(44)).unwrap().cells;
        assert!(shift(&without) > shift(&all));
        assert_eq!(z(&without), z(&all));
    }

    #[test]
    fn a_tap_latches_twice_locks_and_a_third_turns_off() {
        let mut m = Mods::default();
        m.press(Modifier::Ctrl);
        m.release(Modifier::Ctrl);
        assert_eq!(m.hold(Modifier::Ctrl), Hold::Latched);
        m.press(Modifier::Ctrl);
        m.release(Modifier::Ctrl);
        assert_eq!(m.hold(Modifier::Ctrl), Hold::Locked);
        m.typing();
        m.typed();
        assert!(m.active(Modifier::Ctrl), "a lock survives a key");
        m.press(Modifier::Ctrl);
        m.release(Modifier::Ctrl);
        assert!(!m.active(Modifier::Ctrl));
    }

    #[test]
    fn a_latch_is_spent_by_one_key() {
        let mut m = Mods::default();
        m.press(Modifier::Shift);
        m.release(Modifier::Shift);
        m.typing();
        assert!(m.active(Modifier::Shift));
        m.typed();
        assert!(!m.active(Modifier::Shift));
    }

    #[test]
    fn a_modifier_held_while_typing_ends_when_it_comes_up() {
        let mut m = Mods::default();
        m.press(Modifier::Ctrl);
        m.typing();
        m.typed();
        assert!(m.active(Modifier::Ctrl), "still held by the finger");
        m.release(Modifier::Ctrl);
        assert!(!m.active(Modifier::Ctrl), "a hold is not a tap");
    }

    #[test]
    fn ctrl_c_sends_ctrl_around_the_c() {
        let mut b = board("us");
        let mut out = b.press(Finger::Touch(1), Key::Mod(Modifier::Ctrl));
        out.extend(b.release(Finger::Touch(1)));
        out.extend(b.press(Finger::Touch(2), Key::Types(46)));
        out.extend(b.release(Finger::Touch(2)));
        assert_eq!(
            out,
            vec![
                Out::Key(29, true),
                Out::Key(46, true),
                Out::Key(46, false),
                Out::Key(29, false),
            ]
        );
    }

    /// FAILS IF: an `Alt` or a `Ctrl` tapped alone is not given up at once
    /// with its key-up when the footer asks — the leader's set could not be
    /// chosen from this keyboard — or anything else is: a latch already
    /// spent on a key, a lock, `Shift`, two modifiers, or a finger still on
    /// the modifier.
    #[test]
    fn a_lone_alt_or_ctrl_is_given_up_with_its_key_up() {
        for (m, code) in [(Modifier::Alt, 56), (Modifier::Ctrl, 29)] {
            let mut b = board("us");
            let mut out = b.press(Finger::Touch(1), Key::Mod(m));
            out.extend(b.release(Finger::Touch(1)));
            assert_eq!(out, vec![Out::Key(code, true)], "latched, still down");
            assert_eq!(
                b.spend_a_lone_tap(),
                Some((m, vec![Out::Key(code, false)])),
                "{m:?}"
            );
            assert!(!b.mods.active(m), "{m:?} is off afterwards");
            assert_eq!(b.spend_a_lone_tap(), None, "and there is none left");
        }
        // STILL UNDER THE FINGER: not a tap yet.
        let mut b = board("us");
        b.press(Finger::Touch(1), Key::Mod(Modifier::Alt));
        assert_eq!(b.spend_a_lone_tap(), None);
        // A LATCH SPENT ON A KEY is gone already.
        let mut b = board("us");
        b.press(Finger::Touch(1), Key::Mod(Modifier::Ctrl));
        b.release(Finger::Touch(1));
        b.press(Finger::Touch(2), Key::Types(31));
        b.release(Finger::Touch(2));
        assert_eq!(b.spend_a_lone_tap(), None);
        // A LOCK, two taps, is the person's on purpose.
        let mut b = board("us");
        for _ in 0..2 {
            b.press(Finger::Touch(1), Key::Mod(Modifier::Alt));
            b.release(Finger::Touch(1));
        }
        assert_eq!(b.mods.hold(Modifier::Alt), Hold::Locked);
        assert_eq!(b.spend_a_lone_tap(), None);
        // `SHIFT` CHOOSES NO SET, and two modifiers are a chord.
        let mut b = board("us");
        b.press(Finger::Touch(1), Key::Mod(Modifier::Shift));
        b.release(Finger::Touch(1));
        assert_eq!(b.spend_a_lone_tap(), None);
        b.press(Finger::Touch(1), Key::Mod(Modifier::Alt));
        b.release(Finger::Touch(1));
        assert_eq!(b.spend_a_lone_tap(), None, "Shift and Alt latched");
    }

    #[test]
    fn putting_it_away_lets_go_of_everything() {
        let mut b = board("us");
        b.press(Finger::Touch(1), Key::Mod(Modifier::Alt));
        b.press(Finger::Touch(2), Key::Types(30));
        let out = b.let_go();
        assert!(out.contains(&Out::Key(30, false)));
        assert!(out.contains(&Out::Key(56, false)));
    }

    #[test]
    fn a_brazilian_layout_puts_c_cedilla_on_ac10_and_has_ab11() {
        let mut b = board("br");
        assert_eq!(b.legend(Key::Types(39)).0, Legend::Text("ç".to_owned()));
        let caps = b.plan(124);
        assert!(caps.iter().any(|c| c.key == Key::Types(89)));
    }

    #[test]
    fn an_american_layout_has_no_ab11() {
        let mut b = board("us");
        let caps = b.plan(124);
        assert!(!caps.iter().any(|c| c.key == Key::Types(89)));
        assert!(caps.iter().any(|c| c.key == Key::Types(30)));
    }

    #[test]
    fn a_dead_key_says_its_accent() {
        let mut b = board("br");
        // `AD11` in br is the dead acute.
        assert_eq!(
            b.legend(Key::Types(26)).0,
            Legend::Text("\u{B4}".to_owned())
        );
    }

    #[test]
    fn shift_turns_the_legends_to_capitals() {
        let mut b = board("us");
        assert_eq!(b.legend(Key::Types(30)).0, Legend::Text("a".to_owned()));
        b.press(Finger::Touch(1), Key::Mod(Modifier::Shift));
        b.release(Finger::Touch(1));
        assert_eq!(b.legend(Key::Types(30)).0, Legend::Text("A".to_owned()));
        // and the digit shows its shifted sign small
        let (_, small) = b.legend(Key::Types(2));
        assert_eq!(small.as_deref(), Some("1"));
    }

    #[test]
    fn the_keymap_is_a_whole_xkb_keymap() {
        let mut b = board("br");
        let text = b.keymap_text().unwrap();
        assert!(text.starts_with("xkb_keymap"));
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        assert!(
            xkb::Keymap::new_from_string(
                &ctx,
                text,
                xkb::KEYMAP_FORMAT_TEXT_V1,
                xkb::KEYMAP_COMPILE_NO_FLAGS
            )
            .is_some()
        );
    }

    #[test]
    fn the_language_key_cycles_what_is_on() {
        let mut b = board("gb");
        assert_eq!(b.language().layout, "gb");
        b.press(Finger::Touch(1), Key::Language);
        assert_eq!(b.release(Finger::Touch(1)), vec![Out::Keep]);
        assert_eq!(b.language().layout, "br");
    }

    #[test]
    fn the_last_language_cannot_be_turned_off() {
        let mut b = Board::new(
            Prefs {
                languages: Some(vec!["us".into()]),
                ..Prefs::default()
            },
            Language::parse("us"),
        );
        b.press(Finger::Touch(1), Key::Offer(0));
        let out = b.release(Finger::Touch(1));
        assert!(out.contains(&Out::Say(ronoctua_i18n::key::ECHO_OSK_LAST_LANGUAGE)));
        assert_eq!(b.enabled().len(), 1);
    }

    #[test]
    fn the_settings_face_keeps_the_height_and_the_controls() {
        let keys = place(160, Width::Whole, M, Face::Keys, false, 9, &|_| true);
        let settings = place(160, Width::Whole, M, Face::Settings, false, 9, &|_| true);
        let bottom = |c: &[Cap]| c.iter().map(|c| c.top + c.tall).max().unwrap();
        assert!(bottom(&settings) <= bottom(&keys));
        let hide = |c: &[Cap]| c.iter().find(|c| c.key == Key::Hide).copied();
        assert_eq!(hide(&keys), hide(&settings));
    }

    #[test]
    fn f11_and_f12_are_not_after_f10() {
        assert_eq!(Named::F(1).code(), 59);
        assert_eq!(Named::F(10).code(), 68);
        assert_eq!(Named::F(11).code(), 87);
        assert_eq!(Named::F(12).code(), 88);
    }
}
