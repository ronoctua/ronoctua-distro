//! The boundary between buffer and host — sections 7.8 and 7.9.
//!
//! A native buffer is a WASM component. It **does not draw**: it produces a
//! [`DisplayList`], which the host rasterises. That separation is what makes a
//! buffer testable without a GPU, without Wayland and without a compositor — run the component,
//! compare the list.
//!
//! # Invariant 11, in the form of types
//!
//! "WASM buffers do not touch Wayland, do not receive token values, do not
//! translate." All three become properties of this crate's types:
//!
//! - There is nothing of Wayland here, and the buffer has no other way out.
//! - [`Color`] carries the token's **name**, never `0xRRGGBB`. What resolves it is the
//!   host. A buffer cannot invent a colour because there is no
//!   constructor that accepts a value.
//! - [`Text`] distinguishes **data** from **label**: data comes out literal (a glyph's
//!   name, a file's content), a label comes out as a key and the host
//!   translates. A buffer cannot write a UI sentence in Portuguese.
//!
//! # Coordinates in cells
//!
//! Everything in cells, never in pixels. The cell is exact (0.600 at, section 4.1),
//! so the buffer positions by integer arithmetic and does not need to measure
//! text — nor could it, without the font.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

/// A colour token, **by name**.
///
/// Never a value. Invariant 9 puts the colours in the tokens and 11 forbids the
/// buffer from receiving the values; the two together mean that what
/// crosses the boundary is the name, and the host resolves it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color(pub Cow<'static, str>);

impl Color {
    /// A colour token name in WIT's format, like `text-primary`.
    ///
    /// `kebab-case` and not `text.primary`: it is the name of the generated enum's case,
    /// and it is the same on both sides of the boundary.
    #[must_use]
    pub const fn name(n: &'static str) -> Self {
        Self(Cow::Borrowed(n))
    }

    /// The name, for the host to resolve.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What the buffer ASKS to happen, returned by `event`.
///
/// `key` used to return `bool`: the buffer said "I changed" and the host guessed
/// the rest. With effects it asks — copy, fire a named command,
/// close — and the host does not have to assume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// The internal state changed; redraw.
    Redraw,
    /// Put this on the clipboard.
    Copy(String),
    /// Run the named command. **The registry's id**, never a function: section
    /// 13 requires everything to be addressable by name.
    Command(String),
    /// Close this buffer.
    Close,
    /// Write this text to the file the buffer was opened with.
    ///
    /// # Why not `Copy`
    ///
    /// `Copy` goes to the clipboard; saving goes to disk. Reusing it would put
    /// the file's whole content on the clipboard as a side effect of saving —
    /// a surprise nobody would connect to the key they pressed.
    ///
    /// The buffer still has NO filesystem (invariant 11): it hands the text
    /// over and the host writes, exactly as [`Input::Paste`] is how text gets
    /// in. That is what lets a file editor exist without a new capability.
    Save(String),
}

/// A capability the buffer ASKS for in the descriptor.
///
/// The host refuses whoever uses one without asking. The list is short on purpose:
/// each is surface that every future buffer gets to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// The PTY and the VT state machine, which live in the host.
    Terminal,
    /// An offscreen texture, for whoever needs pixels. "Rare and auditable".
    RawSurface,
    /// Emitting [`Command::Svg`] — a vector drawing rasterised by the host.
    ///
    /// Less than [`Capability::RawSurface`]: there is no texture and no pixel
    /// coming from the guest. It is the difference between "the buffer sends pixels" and "the
    /// buffer sends a figure".
    Vector,
}

/// A key binding declared by the buffer, as DATA (section 7.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The sequence, like `m f s`.
    pub keys: String,
    /// The command's id.
    pub command: String,
}

/// The prefix under which a buffer MAY bind (section 7.9).
///
/// "Buffers only bind under `SPC m` (Doom's local leader). An attempt to
/// bind under a global prefix is **rejected at load with an error**." Without that, a
/// third-party component hijacks `SPC f` and the whole system changes
/// behaviour because of one buffer.
pub const LOCAL_PREFIX: &str = "m";

/// What the buffer says about itself in `init`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Descriptor {
    /// The buffer's name.
    pub name: String,
    /// The mode, in the sense of Emacs's major mode.
    pub mode: String,
    /// The keymap declared as data.
    pub keymap: Vec<Binding>,
    /// The command ids it registers.
    pub commands: Vec<String>,
    /// Segments it contributes to the modeline.
    pub segments: Vec<(String, Text)>,
    /// Capabilities asked for.
    pub capabilities: Vec<Capability>,
}

/// The size of a text RUN, by name and never in pixels.
///
/// Invariant 11 is explicit: a buffer does not receive a token value. It says
/// `Large` and the host decides how many pixels that is worth — the same contract colour
/// already has.
///
/// Per run and not per buffer: it used to be a descriptor field, and that is why the glyph
/// browser's header grew along with the icons. The **cell grid
/// is still the normal size's** — a large run occupies more than one
/// cell, and what spaces it is whatever draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scale {
    /// Smaller than the body. For whatever **accompanies** the content without competing
    /// with it — the focused glyph's tags and the results counter.
    Small,
    /// The SYSTEM's size: the same as the footer's and the minibuffer's.
    ///
    /// For whatever is **interface** inside a buffer, and not its content —
    /// today the browser's search field, which is the same thing as the
    /// minibuffer and had no reason to have a different size.
    Base,
    /// The buffers' body text.
    #[default]
    Normal,
    /// Larger, for content that **is** the shape — the glyph in the browser.
    Large,
    /// The subject of the whole screen — the glyph on the sheet.
    Huge,
}

impl Descriptor {
    /// Refuses a descriptor that binds outside `SPC m`.
    ///
    /// # Errors
    ///
    /// Some binding leaves the local prefix. The message says which.
    pub fn validate(&self) -> Result<(), String> {
        for l in &self.keymap {
            let first = l.keys.split_whitespace().next().unwrap_or("");
            if first != LOCAL_PREFIX {
                return Err(format!(
                    "o buffer `{}` tenta bindar `{}`, fora de `{LOCAL_PREFIX}`.\n\
                     Buffers so bindam sob a particao local (secao 7.9); um \
                     prefixo global sequestraria a tecla para todo o sistema.",
                    self.name, l.keys
                ));
            }
        }
        Ok(())
    }
}

/// Text that crosses the boundary.
///
/// The distinction exists because invariants 8 and 11 meet here: **the
/// buffer does not translate**, but it also cannot send everything as a key — a glyph's
/// name or a file's content is not UI text and has no
/// translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Text {
    /// The buffer's data. It goes out as it is.
    Data(String),
    /// An interface label. The host resolves it through `ronoctua-i18n`.
    Key(String),
    /// An icon, by TOKEN name. The host resolves the codepoint.
    ///
    /// Without this, a buffer that wanted a magnifying glass would write the PUA
    /// character by hand — a codepoint that changes between Nerd Fonts versions,
    /// embedded in a component that has no way of knowing that (invariants 9,
    /// 10 and 11).
    Icon(String),
}

impl Text {
    /// Literal data.
    #[must_use]
    pub fn datum(s: impl Into<String>) -> Self {
        Self::Data(s.into())
    }

    /// A text key.
    #[must_use]
    pub fn key(s: impl Into<String>) -> Self {
        Self::Key(s.into())
    }

    /// An icon token's name.
    #[must_use]
    pub fn icon(s: impl Into<String>) -> Self {
        Self::Icon(s.into())
    }
}

/// A drawing command.
///
/// The list is deliberately SHORT. There is no filled rectangle, border,
/// shadow or radius: invariants 14 and 15 forbid cards and elevation, and
/// separation is by whitespace and a one-line rule. A rectangle
/// command here would be the way in for all of that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Text at a position, in cells.
    Text {
        /// The row, from the buffer's top.
        line: u16,
        /// The column, from the left.
        column: u16,
        /// What to write.
        text: Text,
        /// The colour token.
        color: Color,
        /// The size, by name. See [`Scale`].
        scale: Scale,
        /// How many cells the run may occupy. `0` = to the edge.
        ///
        /// The buffer declares the **budget**; what cuts is the host, which is what
        /// has the translated string.
        max_width: u16,
    },
    /// A ONE-line horizontal rule — the only separator that exists
    /// besides whitespace (section 5.1).
    Rule {
        /// On which row.
        line: u16,
        /// The colour token.
        color: Color,
    },
    /// A terminal grid — a FIRST-CLASS node (section 7.10).
    ///
    /// Not a `raw-surface`, on purpose: `raw-surface` would mean a
    /// second text rasteriser in the system, with its own font resolution and
    /// metrics, in the most-used buffer. Here the content arrives as
    /// already-formed lines and the host draws with the SAME font as everything else.
    Grid {
        /// The starting row, in cells.
        line: u16,
        /// The starting column, in cells.
        column: u16,
        /// One string per grid row, in order.
        lines: Vec<String>,
        /// The colour, by token name.
        color: Color,
    },
    /// The background of a list's focused row.
    ///
    /// **A change of colour, not a box and not an outline** (invariant 15). That is why
    /// there is no width and no radius: it paints the whole row, and that is all.
    Selection {
        /// On which row.
        line: u16,
        /// The colour token.
        color: Color,
    },
    /// A vector drawing, in the given rectangle of cells.
    ///
    /// **The first pixel primitive in this list.** It exists because Lucide's
    /// customisation changes the stroke's width, cap and colour, and a
    /// glyph honours none of that — the width is baked into the drawing.
    ///
    /// It requires [`Capability::Vector`]: it is the only primitive that produces arbitrary
    /// pixels, and the descriptor has to ask for it. See [`Descriptor::validate`].
    Svg {
        /// The starting row, in cells.
        line: u16,
        /// The starting column, in cells.
        column: u16,
        /// The rectangle's height, in cells.
        lines: u16,
        /// The rectangle's width, in cells.
        columns: u16,
        /// The document, as the copy would deliver it.
        svg: String,
        /// The token that replaces `currentColor`.
        ///
        /// The colour does **not** cross as a value (invariant 11), and without this
        /// Lucide's default `currentColor` would come out black over the buffer's
        /// background — an invisible drawing, indistinguishable from "not implemented".
        color: Color,
    },
}

impl Command {
    /// On which row the command draws. Used by the host for clipping.
    #[must_use]
    pub const fn line(&self) -> u16 {
        match self {
            Self::Text { line, .. }
            | Self::Rule { line, .. }
            | Self::Grid { line, .. }
            | Self::Selection { line, .. }
            | Self::Svg { line, .. } => *line,
        }
    }

    /// The capability this command requires, when it requires one.
    ///
    /// # Why the list belongs to the COMMAND, and not to the host
    ///
    /// Because the host has to refuse **at load**, before the buffer draws
    /// anything, and for that it needs to know what each primitive
    /// costs without consulting a second table. `Descriptor::validate` reads
    /// from here; a new command that needs a gate declares it here and gets the
    /// refusal for free.
    #[must_use]
    pub const fn requires(&self) -> Option<Capability> {
        match self {
            Self::Svg { .. } => Some(Capability::Vector),
            _ => None,
        }
    }
}

/// What a buffer produces per frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisplayList {
    /// In drawing order: the first goes underneath.
    pub commands: Vec<Command>,
}

impl DisplayList {
    /// An empty list.
    #[must_use]
    pub fn fresh_one() -> Self {
        Self::default()
    }

    /// Adds a command.
    pub fn push(&mut self, c: Command) {
        self.commands.push(c);
    }

    /// Refuses a list that uses a primitive without the declared capability.
    ///
    /// # Why the refusal is here, and not in the descriptor
    ///
    /// Because the descriptor does not say what the buffer is going to **draw** — it says what
    /// the buffer asks for. WIT's promise (*"the host refuses whoever uses without having
    /// asked"*) is only kept by confronting the two, and the first `render`
    /// happens **before the window exists**: a buffer that emits
    /// [`Command::Svg`] without asking for [`Capability::Vector`] dies at startup, with
    /// the message saying what and why.
    ///
    /// Until now that line of the WIT was a promise: `validate` only checked the
    /// keymap's prefix, and no capability was required of anyone.
    ///
    /// # Errors
    ///
    /// Some command requires a capability the descriptor did not declare.
    pub fn validate(&self, capabilities: &[Capability]) -> Result<(), String> {
        for c in &self.commands {
            let Some(exigida) = c.requires() else {
                continue;
            };
            if !capabilities.contains(&exigida) {
                return Err(format!(
                    "o buffer desenha {c:?} sem declarar `{exigida:?}`.\n\
                     Cada primitiva com trava e superficie que todo buffer \
                     futuro passa a poder usar (secao 7.6); quem a usa a pede \
                     no descritor, e o host confere antes de desenhar."
                ));
            }
        }
        Ok(())
    }

    /// How many rows the list occupies.
    ///
    /// Zero when empty — not 1. The host uses this to size things, and a
    /// empty buffer must reserve no rows at all.
    #[must_use]
    pub fn height(&self) -> u16 {
        self.commands
            .iter()
            .map(|c| c.line().saturating_add(1))
            .max()
            .unwrap_or(0)
    }

    /// Every colour token named, in stable order and without repetition.
    ///
    /// It is what the host has to resolve before drawing, and what a check
    /// uses to prove that a buffer does not name a non-existent token.
    #[must_use]
    pub fn colors(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .commands
            .iter()
            .map(|c| match c {
                Command::Text { color, .. }
                | Command::Rule { color, .. }
                | Command::Grid { color, .. }
                | Command::Selection { color, .. }
                | Command::Svg { color, .. } => color.as_str(),
            })
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// What the host sends to the buffer.
///
/// Deliberately poor: size and input, nothing else. A buffer that
/// needed to know the time, the language or the theme would be doing something the
/// host should be doing for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// Usable width, in cells.
    pub columns: u16,
    /// Usable height, in cells.
    pub lines: u16,
    /// How many cells of height each scale occupies.
    pub heights: Heights,
}

/// How many **cells** of height each scale occupies.
///
/// This is not a token value (invariant 11): it is a cell count, the same
/// unit the buffer already counts rows and columns in. What computes it is the host,
/// which knows the font size and the line height.
///
/// Without this the buffer had to GUESS how many rows a large run
/// occupies — and it guessed wrong: the sheet's `huge` glyph came out clipped at the
/// bottom, touching the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heights {
    /// Cells of a `small` run.
    pub small: u16,
    /// Cells of a `base` run.
    pub base: u16,
    /// Cells of a `normal` run. It is the cell, so it is 1.
    pub normal: u16,
    /// Cells of a `large` run.
    pub large: u16,
    /// Cells of an `huge` run.
    pub huge: u16,
}

impl Heights {
    /// How many cells a run of `size` px occupies.
    ///
    /// Pure arithmetic, with no token: whoever has the numbers is the host, and what
    /// converts them into cells is this calculation. **Rounding up**, always — half a cell
    /// short is the glyph clipped at the bottom, which was the defect that gave rise to
    /// this field.
    #[must_use]
    pub fn cells(size: f32, line_height: f32, altura_da_celula: f32) -> u16 {
        if altura_da_celula <= 0.0 {
            return 1;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (size * line_height / altura_da_celula).ceil() as u32;
        u16::try_from(n.max(1)).unwrap_or(1)
    }

    /// The production heights, given the five sizes and the line height.
    #[must_use]
    pub fn from_sizes(
        small: f32,
        base: f32,
        normal: f32,
        large: f32,
        huge: f32,
        line_height: f32,
    ) -> Self {
        // THE CELL IS `normal`'s: it is what defines the grid the buffer
        // counts rows and columns in. See `Context`.
        let cell = normal * line_height;
        Self {
            small: Self::cells(small, line_height, cell),
            base: Self::cells(base, line_height, cell),
            normal: Self::cells(normal, line_height, cell),
            large: Self::cells(large, line_height, cell),
            huge: Self::cells(huge, line_height, cell),
        }
    }
}

/// A key, already normalised by the host.
///
/// The buffer does NOT see compositor modifiers nor the leader sequence: those
/// are resolved beforehand (section 7.4). What arrives is what is left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A NAMED command, dispatched by the host when the buffer keymap's key
    /// sequence completes. It is the other side of [`Binding`]: the buffer
    /// declares, the host resolves the key, and the command comes back through here — the buffer
    /// never captures a key (section 7.9).
    Command(String),
    /// A character typed.
    Char(char),
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow. A grid buffer needs all four directions.
    Left,
    /// Right arrow.
    Right,
    /// Confirm.
    Confirm,
    /// Cancel.
    Cancel,
    /// Delete the previous character.
    Delete,
    /// `Tab`: toggle whatever is toggleable on screen.
    ///
    /// An intent and not a key, like the rest — and a case of its own because, without
    /// it, `Tab` arrives as `Char('\t')` and becomes an invisible tab
    /// inside the search field.
    Toggle,
    /// The result of an [`Effect::Copy`] asked for earlier.
    ///
    /// What knows whether the copy worked is the host — it is what talks to the
    /// clipboard. Without this way back, the buffer could only show an
    /// optimistic "copied!", which lies exactly when it matters.
    Copied(bool),
    /// Text coming from the clipboard (`Ctrl+V`), already resolved by the
    /// host: the buffer does not read the clipboard, just as it does not read a file (invariant 11).
    Paste(String),
    /// A left-button click on a cell of the buffer's grid.
    ///
    /// In **cells**, and never in pixels: a pixel is a Wayland and screen-scale
    /// thing, and the buffer knows neither of the two (invariant 11). What
    /// converts, and what subtracts the window's margin, is the host — in the same way
    /// it already converts a key into [`Input::Char`].
    ///
    /// What resolves WHICH item is there is the buffer: only it knows its own
    /// layout. The host sending "item number N" would be the host reimplementing the
    /// buffer's layout.
    Click {
        /// The column, from the buffer's (0,0) cell.
        column: u16,
        /// The row, from the buffer's (0,0) cell.
        line: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::{Binding, Capability, Color, Command, Descriptor, DisplayList};

    fn with_svg() -> DisplayList {
        let mut l = DisplayList::fresh_one();
        l.push(Command::Svg {
            line: 0,
            column: 0,
            lines: 4,
            columns: 4,
            svg: "<svg/>".into(),
            color: Color("text-primary".into()),
        });
        l
    }

    /// THE DEFECT: the gate that never had teeth.
    ///
    /// The WIT has always promised that *"the host refuses whoever uses without having
    /// asked"*, and until now `validate` checked the keymap's prefix and nothing
    /// else — **no capability was required of anyone**. A pixel
    /// primitive coming in without that gate would make "the list is short on purpose"
    /// a sentence with no consequence.
    #[test]
    fn a_gated_primitive_requires_the_capability() {
        assert!(
            with_svg().validate(&[]).is_err(),
            "desenhou vetor sem declarar `Vetor`"
        );
        assert!(
            with_svg().validate(&[Capability::Terminal]).is_err(),
            "a capacidade ERRADA nao pode abrir a porta"
        );
        with_svg()
            .validate(&[Capability::Vector])
            .expect("quem pediu tem de passar");
    }

    /// And whatever has no gate passes without asking for anything — otherwise every existing
    /// buffer would have to declare a capability in order to draw text.
    #[test]
    fn what_has_no_gate_requires_nothing() {
        let mut l = DisplayList::fresh_one();
        l.push(Command::Rule {
            line: 0,
            color: Color("rule-default".into()),
        });
        assert_eq!(l.commands[0].requires(), None);
        l.validate(&[]).expect("regua nao exige capacidade");
    }

    fn with_keys(t: &str) -> Descriptor {
        Descriptor {
            name: "x".into(),
            keymap: vec![Binding {
                keys: t.into(),
                command: "x.y".into(),
            }],
            ..Descriptor::default()
        }
    }

    #[test]
    fn a_buffer_only_binds_under_the_local_prefix() {
        // Section 7.9: "buffers only bind under `SPC m`. An attempt to bind
        // under a global prefix is REJECTED at load with an error."
        //
        // Without this, a third-party component hijacks `SPC f` and the whole
        // system changes behaviour because of one buffer.
        assert!(with_keys("m f s").validate().is_ok());
        assert!(with_keys("m").validate().is_ok());

        for global in ["f s", "b b", "g", "q q"] {
            let e = with_keys(global).validate().unwrap_err();
            assert!(e.contains(global), "a mensagem nao diz qual: {e}");
        }
    }

    #[test]
    fn the_refusal_message_names_the_buffer() {
        // Refusing without saying WHO forces a hunt among the loaded
        // components — and the load happens far from whoever wrote the buffer.
        let mut d = with_keys("f s");
        d.name = "navegador".into();
        assert!(d.validate().unwrap_err().contains("navegador"));
    }

    #[test]
    fn a_capability_is_asked_for_and_not_presumed() {
        // A descriptor with no capability at all is the common case: neither PTY nor
        // pixels. The host refuses whoever USES without having asked.
        assert!(Descriptor::default().capabilities.is_empty());
        let d = Descriptor {
            capabilities: vec![Capability::Terminal],
            ..Descriptor::default()
        };
        assert!(d.capabilities.contains(&Capability::Terminal));
        assert!(!d.capabilities.contains(&Capability::RawSurface));
    }

    use super::*;

    #[test]
    fn the_height_of_an_empty_list_is_zero() {
        // Zero, not 1: an empty buffer reserves no rows.
        assert_eq!(DisplayList::fresh_one().height(), 0);
    }

    #[test]
    fn the_height_counts_the_tallest_line_plus_one() {
        let mut d = DisplayList::fresh_one();
        d.push(Command::Text {
            line: 4,
            column: 0,
            text: Text::datum("x"),
            color: Color::name("text-primary"),
            scale: Scale::Normal,
            max_width: 0,
        });
        d.push(Command::Rule {
            line: 1,
            color: Color::name("rule-default"),
        });
        assert_eq!(d.height(), 5);
    }

    #[test]
    fn the_colours_come_out_sorted_and_with_no_repeats() {
        // The host resolves token by token; a repeat would be wasted work, and
        // an unstable order would make a check compare different lists of the same
        // buffer.
        let mut d = DisplayList::fresh_one();
        for name in ["text.primary", "rule.default", "text.primary"] {
            d.push(Command::Rule {
                line: 0,
                color: Color(name.into()),
            });
        }
        assert_eq!(d.colors(), ["rule.default", "text.primary"]);
    }

    #[test]
    fn data_and_key_are_different_kinds() {
        // Invariant 11 lives in this distinction: the buffer does not translate, but
        // it also cannot send everything as a key — a glyph's name has no
        // translation.
        assert_ne!(Text::datum("nf-fa-folder"), Text::key("nf-fa-folder"));
    }
}
