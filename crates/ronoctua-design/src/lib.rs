//! The layout vocabulary every buffer draws with — **one** implementation of
//! each decision the screen makes.
//!
//! # The defect this crate exists to close
//!
//! From a photograph of the screen: buffer titles with no proportional air, a
//! rule that left a gap on the left and spilled on the right, every buffer with
//! a margin of its own, and one view spaced differently in the footer and in
//! its own window. Four symptoms, one cause — there were seven answers to
//! "what closes a heading" and three copies of the grid's arithmetic, each with
//! its own constants:
//!
//! - `buffers/format` drew the rule and left a blank cell under it;
//! - `buffers/transfers` drew the rule and no blank;
//! - `buffers/memory` left a blank and drew no rule;
//! - `buffers/example` built the pair by hand, at a fourth column.
//!
//! A check that scolds seven copies is still seven copies. What ends it is
//! there being one, and this is it: [`Screen::heading`] is the heading, and a
//! buffer that wants a different one has to change this file, where the reason
//! is written down beside the number.
//!
//! # Invariant 12, and the reason this is a crate and not a module
//!
//! Invariant 12 names `ronoctua-design` as a `wasm32-wasip2` component. It is
//! the same crate on both sides: with `host` (the default) the arithmetic is
//! compiled for the machine and tested by `cargo test`; with `guest` it is
//! compiled into the component, alongside the WIT bindings and the conversion
//! to the display list the component returns.
//!
//! # `no_std`, and it is not about bytes
//!
//! `buffers/example` already argues correctly that saving a few KiB is not
//! worth ceremony. The reason here is different: `no_std` makes **invariant 11
//! a property the compiler checks**, in the one crate every buffer now links.
//! No filesystem, no clock, no threads, no `std::env` — not "we agreed not to",
//! but *the names do not exist*. `alloc` is taken because a display list is a
//! `Vec` of commands with `String`s in it, which is what [`ronoctua_protocol`]
//! already decided.
//!
//! # What this crate does NOT know
//!
//! A colour's value, an icon's codepoint, a translated sentence, a pixel and a
//! token (invariants 9 and 11). It knows **cells**, and it knows which *role* a
//! run plays. [`Palette`] is the one wall where the two sides differ, and it is
//! seven names wide.
//!
//! # Where the buffer's margin comes from, and why there is no gutter here
//!
//! It is the host's. `paint::with_margin` shifts every run by
//! `space.buffer.marginX`, and `buffer.rs`'s `context_of` takes that margin off
//! **both** sides before it reports [`Context::columns`] — so cell zero is
//! already two cells inside the window and `columns` already stops two cells
//! short of its right edge. A buffer that adds a gutter of its own pays the
//! margin twice, which is exactly the "every buffer with its own margin" in the
//! report. Nothing in this crate starts anywhere but at cell zero, and nothing
//! reserves air at the right edge.
//!
//! [`Context::columns`]: ronoctua_protocol::Context::columns

#![no_std]
#![warn(missing_docs)]
#![warn(unsafe_code)]

extern crate alloc;

// The test harness is a binary and links `std`; the crate under test does not.
// Without this line `cargo test` cannot build a `no_std` library, and with it
// the library itself is unchanged — `#[cfg(test)]` code never ships.
#[cfg(test)]
extern crate std;

/// The WIT bindings, generated ONCE and shared by every component.
///
/// # Why a library owns the generation
///
/// Every buffer used to call `wit_bindgen::generate!` for itself, which meant
/// every buffer had its own copy of the guest types — and a helper that took
/// one of them could not be shared, because `format`'s `Command` and
/// `memory`'s `Command` are different Rust types with the same shape. That is
/// what kept the layout vocabulary from being a crate at all.
///
/// The four options below are what make one copy work. `pub_export_macro`
/// with `export_macro_name` re-exports the `export!` a component needs; a
/// buffer then writes
///
/// ```ignore
/// ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);
/// ```
///
/// and `default_bindings_module` is what tells the generated macro where those
/// types live. The result is a real component: the produced file starts
/// `00 61 73 6d 0d 00 01 00`, the component model's magic, byte for byte the
/// same as a buffer that generates its own.
///
/// The `path` is a LITERAL — there is no way to pass it through an environment
/// variable, which is why `nix/buffers.nix` rebuilds the repository's layout
/// inside the derivation instead of rewriting the string.
#[cfg(feature = "guest")]
// THE EXEMPTION IS ON THE MODULE, and not on the crate.
//
// `wit_bindgen::generate!` emits FFI glue — `export_name`, `unsafe fn`, modules
// with no docs — and none of it is ours or optional: it is the target's shape.
// `buffers/example` puts the same `allow` at the top of its `lib.rs` because a
// buffer is nothing but bindings. Here the generated code is a tenth of the
// crate, so the exemption stops at the module wall and the arithmetic below
// stays under the rule.
#[allow(unsafe_code, missing_docs)]
pub mod bindings {
    wit_bindgen::generate!({
        world: "buffer",
        path: "../ronoctua-protocol/wit",
        // `generate_all` because the world uses a dependency PACKAGE
        // (`ronoctua:tokens`, the enum generated from the tokens). Without it
        // `wit-bindgen` refuses with "missing `with` mapping", and the next
        // error points at the `use` rather than at the cause.
        generate_all,
        pub_export_macro: true,
        export_macro_name: "export",
        default_bindings_module: "ronoctua_design::bindings",
    });
}

pub mod budget;
pub mod grid;
pub mod phrases;
// THE PIXEL RAIL IS THE HOST'S. A component counts in cells and lines and has
// no surface; see the module's own head for why that is not an omission.
#[cfg(feature = "guest")]
pub mod guest;
#[cfg(feature = "host")]
pub mod rails;
pub mod region;
pub mod screen;
pub mod scroll;
pub mod sheet;
pub mod table;

pub use budget::{budget, claimed, room, tall, wide};
pub use grid::Grid;
pub use phrases::Phrases;
#[cfg(feature = "host")]
pub use rails::Frame;
pub use region::{Region, Slice, split};
pub use screen::{Ink, Run, Screen};
pub use scroll::Scroll;
pub use sheet::{SHEET_LABEL, Sheet, sheet_split};
pub use table::{Column, Field, Layout, Row, Table, Width};

/// The types a buffer draws with, in one place.
///
/// A buffer writes `use ronoctua_design::types::*` and never
/// `wit_bindgen::generate!`. Under `guest` it gets the generated half — the
/// `Guest` trait its component implements, and every record and enum that
/// crosses the WIT — plus this crate's own vocabulary. Under `host` it gets the
/// [`ronoctua_protocol`] half instead.
///
/// # Why the two halves are never exported together
///
/// Because `Context`, `Command`, `Text` and `Scale` exist on both sides with
/// the same names and different shapes, and a module that offered both would
/// make `Context` mean whichever one the glob happened to win. The generated
/// half is what a component's own signatures need; the protocol half is what
/// the arithmetic here counts in; and [`Screen::of`](screen::Screen) bridges
/// them so a buffer never has to name the second.
pub mod types {
    pub use crate::screen::{Ink, Run, Screen};
    pub use crate::scroll::Scroll;
    pub use crate::sheet::{SHEET_LABEL, Sheet};
    pub use crate::table::{Column, Field, Layout, Row, Table, Width};
    pub use crate::{AIR, GAP, Grid, ICON, Palette, Role};

    /// The `Guest` trait a component implements.
    #[cfg(feature = "guest")]
    pub use crate::bindings::exports::ronoctua::buffer::view::Guest;
    /// Everything that crosses the WIT.
    #[cfg(feature = "guest")]
    pub use crate::bindings::ronoctua::buffer::types::*;

    #[cfg(not(feature = "guest"))]
    pub use ronoctua_protocol::{Context, DisplayList, Heights, Scale};
}

/// THE FRAME AS THE HOST RECEIVES IT.
///
/// [`screen::Screen::as_protocol`] answers in these types, and a buffer's own
/// TESTS are the only callers — production hands the WIT's shapes over instead
/// (`Screen::finish_wit`). It is re-exported here because a buffer must not
/// name the protocol crate itself: this crate is its one door, and a buffer
/// that reached past it would be a second place the wall is described.
pub use ronoctua_protocol as frame;

// ---------------------------------------------------------------------------
// The two numbers a buffer may not be told, and the reason for each.
// ---------------------------------------------------------------------------

/// The unit of vertical air: **one cell**.
///
/// Not a token value crossing the wall (invariant 11) — a *definition*. The
/// cell is `font.size.base × font.lineHeight`, which is what
/// [`Heights::cells`](ronoctua_protocol::Heights::cells) already computes from
/// the other side; "the air is one line height" and "the air is one cell" are
/// the same sentence. What a token could still change is the pixel height of
/// that cell, and the air follows it without this file being touched.
pub const AIR: u16 = 1;

/// How many cells a UI icon takes across: **two**.
///
/// `ceil(font.size.icon / font.size.base) = ceil(22 / 18) = 2`. The icon is
/// drawn on the body's cell grid at 22 px, and its advance — `0.600 × 22 = 13.2`
/// px — is already wider than the 10.8 px cell, so one column would clip it and
/// three would leave a hole. The three browsers each derived this
/// independently and got 2, 4 and 2.
///
/// It was `ceil(22 / 20)` while the body was 20 px, and the answer was two
/// then as well: the body came down to the system's one size and this
/// definition did not move.
pub const ICON: u16 = 2;

/// The horizontal air beside anything: **one cell**.
///
/// The vertical direction gets its air free — the font's box is `1.25 em`
/// against a `1.45` line height, so every run already carries `0.10 em` above
/// and below. Horizontally there is no box at all: a Nerd glyph runs from 0 to
/// 600 units in an advance of 600, with no side bearing whatsoever. Between a
/// glyph and the word after it, or between two columns of a table, the cell has
/// to be put in by hand.
///
/// It is also the whole of the separation: no rule between columns, no box
/// around them (invariants 14 and 15).
pub const GAP: u16 = 1;

/// How many rows of content a heading has to leave behind to be worth its air.
///
/// One. A heading over nothing is a heading about nothing — and below this the
/// block gives its air up rather than taking the last row the content had. See
/// [`Screen::heading`].
pub const CONTENT_FLOOR: u16 = 1;

// ---------------------------------------------------------------------------
// The wall between the two sides.
// ---------------------------------------------------------------------------

/// What a run is FOR — never which colour it is.
///
/// # Why a role and not a colour
///
/// Because the arithmetic in this crate never looks at a colour, and the two
/// sides spell one differently: on the host a colour is
/// [`ronoctua_protocol::Color`], a token's name; inside the component it is the
/// enum generated from `assets/tokens/colors.nix`, where a name that does not
/// exist does not compile. A layout function that took either of them would
/// only compile on one side.
///
/// Invariant 9 is untouched: the names are still tokens, and this only fixes
/// **which token plays which part**, once. Seven parts is the whole vocabulary
/// — a buffer that needs an eighth is proposing a new one, which is a decision
/// and not an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// The subject of the block. What a heading's words are drawn in.
    Title,
    /// The buffer's content: what one came to read.
    Body,
    /// Content that accompanies without competing — a count, a unit.
    Quiet,
    /// A label naming a value, and the value is the point.
    Dim,
    /// Focus, selection, the glyph in front of a heading. The one colour the
    /// system reserves for "this one".
    Accent,
    /// The rule, and only the rule.
    Rule,
    /// The same rule, on a ground that was RAISED.
    ///
    /// # Why a rule needs to know what it is drawn on
    ///
    /// Because a rule is a signal and a ground is where the theme lives, and
    /// the two climb the same ladder. `rule.default` is the step that reads
    /// against the chrome; on a raised ground it does not. Measured across the
    /// nine themes on 2026-09-20: `rule.default` on `surface.header` is ΔE
    /// 0,025 and on `surface.container` 0,047 — the first is a line nobody can
    /// see, and the owner photographed exactly that. `rule.raised` measures
    /// 0,098 and 0,123 on the same two.
    ///
    /// # Why no buffer ever names it
    ///
    /// Because choosing it is not a decision anyone should be making twice.
    /// [`Pen::fill`] remembers the role the block's ground was filled with and
    /// [`Pen::rule`] resolves this one from it, so a rule inside a container
    /// is the raised one by construction. The check
    /// `a-rule-follows-its-ground` is what keeps that true.
    RuleRaised,
    /// The band under a list's focused row. A change of colour, and no radius.
    Band,
    /// The ground under a CONTAINER — a block of rows that belong together.
    ///
    /// # Why it is not [`Role::Band`]
    ///
    /// Because a band means "this one", and a container means "these". Drawn in
    /// the band's colour, five rows of history would read as five selected
    /// rows, which is the one thing the band is for. The owner asked for the
    /// result to sit on a background of its own, and a change of ground is how
    /// this system says it: a card is allowed, the preference is for the
    /// quieter answer, and shadow and elevation are what stay out.
    ///
    /// It was `surface.shelf` and it drew NOTHING, which is worth writing
    /// down: a shelf is `neutral.window` and a buffer's own ground is
    /// `surface.buffer`, which is `neutral.window` too. The quad was emitted,
    /// painted, and identical to the page. Measured off a photograph of the
    /// brain — three history rows that each asked for a ground, and three rows
    /// whose right edge read exactly the page colour. `surface.container` is
    /// the next rung that exists, and it is a rung a BUFFER can see.
    Shelf,
    /// The ground under a buffer's header line — the band that carries its
    /// glyph, its name and every shortcut it answers to.
    ///
    /// Louder than [`Role::Shelf`] on purpose: a container is meant to be
    /// quiet and a header is meant to be chrome. One rung apart, which is all
    /// the difference a page needs when the separation is done with ground
    /// rather than with a border.
    Header,
    /// Something is wrong, or about to be, and the screen says so in colour.
    ///
    /// The one role that is allowed to alarm. A buffer may not name a token
    /// (invariant 11), so without this there is no way for one to draw the
    /// difference between "here is a fact" and "nothing will run until you do
    /// something" — and the brain's whole reason for having a screen is the
    /// second kind of sentence.
    Danger,
    /// A thing that happens ONCE and is then gone, against the things that
    /// repeat.
    ///
    /// The brain's list has two kinds of row and they are not told apart by
    /// their words: both are a name, a moment and a standing. A glyph says
    /// which, and this says it again in colour, because a glyph two cells wide
    /// is a small thing to hang a distinction on.
    Once,
    /// The ground under the row the POINTER is on.
    ///
    /// Not [`Role::Band`], which is where the KEYBOARD is. The two are
    /// different questions and every window in the shell already answers both
    /// — a buffer could answer neither, because no role reached
    /// `surface.hover` and the wall carried no pointer event at all.
    Hover,
    /// The notation of a shortcut — `RET`, `m r`, `Tab`.
    ///
    /// # Why the hint family had to be reachable
    ///
    /// Because the minibuffer's shortcut line is drawn in `text.subtle.*`, a
    /// hue family deliberately OFF the warm content ramp so the line reads as
    /// "not content", and [`Palette::color`] was a closed table of ten tokens
    /// with none of that family in it. A buffer teaching its keys had no
    /// choice but to reach for `state.accent` and `text.dim` — the content
    /// ramp — and the owner read the result on screen: the brain's header did
    /// not look like the file manager's minibuffer, because it was not wearing
    /// the same colours.
    Key,
    /// What a shortcut DOES, beside its notation.
    Prose,
    /// Punctuation between two shortcuts, and the only thing below the prose.
    ///
    /// A third tone, so a separator is not read as a word — the same argument
    /// `Register::Separator` makes on the host's side of the wall.
    Faint,
}

/// The one thing that differs between the host and the component.
///
/// Seven roles to seven token names, and an icon type. Everything else in this
/// crate is integer arithmetic over cells and is compiled, unchanged, on both
/// sides.
///
/// # Why one method and not seven
///
/// Seven functions cannot be checked for completeness; a `match` on [`Role`]
/// can. Adding a role breaks every implementation at compile time, which is the
/// property that makes the wall worth having.
pub trait Palette {
    /// A colour, as this side spells one.
    ///
    /// `Clone` and not `Copy`: on the host a colour is a token's NAME, which is
    /// a `Cow<'static, str>` — it copies in practice and does not in the type
    /// system, and demanding `Copy` would rule out the very type
    /// [`ronoctua_protocol`] chose for it.
    type Color: Clone;
    /// An icon, as this side spells one.
    type Icon: Copy;

    /// The token that plays this part.
    fn color(role: Role) -> Self::Color;

    /// The token that colours a piece of CODE.
    ///
    /// # Why this is a second method and not seven more roles
    ///
    /// Because [`Role`] is the vocabulary of LAYOUT — what part of the screen
    /// a run plays — and its own documentation says an eighth part is a
    /// decision rather than an argument. A comment and a keyword are not parts
    /// of a screen; they are what a CHARACTER is, which is a second axis. One
    /// enum for where a thing sits and one for what it is keeps both closed.
    ///
    /// Fourteen roles reach twelve tokens: `Constant` shares `number` and
    /// `Heading` shares `attribute`, for the reason [`Tokens::color`] gives
    /// about `Quiet` and `Dim` a few lines down.
    #[cfg(feature = "syntax")]
    fn syntax(role: ronoctua_syntax::Role) -> Self::Color;

    /// The glyph's name, **for a message somebody has to act on**.
    ///
    /// On the host it is the token's own name and
    /// [`Screen::as_protocol`](screen::Screen::as_protocol) is the real frame.
    /// Inside the component the generated enum has no name at runtime — WIT
    /// produces no reverse table, and writing one by hand would be a second
    /// copy of `assets/icons` (invariant 9) — so it is the variant's `Debug`
    /// spelling, and `as_protocol` is a geometry shadow. Nothing is lost by
    /// that: `DisplayList::columns_of` charges an icon `heights.of(scale)`
    /// cells whatever the string says, so the two questions the host asks
    /// answer the same either way.
    fn glyph_name(icon: Self::Icon) -> alloc::string::String;
}

/// The host's palette: a role becomes the token's NAME.
///
/// It is what `cargo test` draws with, and what a host-side caller would use.
/// The names are `assets/tokens/colors.nix`'s own, in WIT spelling — the same
/// strings `ronoctua_tokens::color_by_wit_name` resolves, so a typo here is a
/// protocol violation the host refuses rather than a colour nobody notices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tokens;

impl Palette for Tokens {
    type Color = ronoctua_protocol::Color;
    type Icon = &'static str;

    fn color(role: Role) -> Self::Color {
        ronoctua_protocol::Color::name(match role {
            Role::Title => "text-primary",
            Role::Body => "text-secondary",
            // TWO ROLES, TWO TOKENS, AND THE DAY IS TODAY.
            //
            // They were both `text-dim`, and the comment here said the day the
            // theme separated them the change would be in this table. The
            // owner found the day: the brain's log draws a run's NAME in
            // `Quiet` and the machine's error beside it in `Dim`, and he read
            // them on screen as one colour — *"a mensagem de erro na mesma cor
            // do título, e não mais discreta como eu havia pedido"*.
            //
            // `Quiet` is content that accompanies — a count, a unit, a name
            // beside a date — so it takes the middle of the content ramp, the
            // step that is present and does not pull. `Dim` is a label naming
            // a value, and the value is the point: it stays at the bottom.
            // Between them the row now has a hierarchy instead of a tone.
            Role::Quiet => "text-secondary",
            Role::Dim => "text-dim",
            Role::Accent => "state-accent",
            Role::Rule => "rule-default",
            Role::RuleRaised => "rule-raised",
            Role::Band => "surface-selection",
            // `surface-container` AND NOT `surface-shelf`, and the enum's own
            // note carries the measurement: the shelf is the buffer's page
            // colour exactly, so it painted nothing at all.
            Role::Shelf => "surface-container",
            Role::Header => "surface-header",
            Role::Danger => "state-error",
            Role::Once => "text-once",
            Role::Hover => "surface-hover",
            // THE HINT FAMILY, the same three the minibuffer's own line uses:
            // `Register::Key`, `Register::Prose` and `Register::Separator` in
            // `chrome.rs` resolve to exactly these.
            Role::Key => "text-subtle-strong",
            Role::Prose => "text-subtle-default",
            Role::Faint => "text-subtle-faint",
        })
    }

    #[cfg(feature = "syntax")]
    fn syntax(role: ronoctua_syntax::Role) -> Self::Color {
        use ronoctua_syntax::Role as R;

        ronoctua_protocol::Color::name(match role {
            R::Comment => "syntax-comment",
            R::String => "syntax-string",
            R::Escape => "syntax-escape",
            // TWO ROLES ON ONE TOKEN, and they stay two: a literal `null` and
            // a literal `42` are the same kind of thing today, and the day the
            // theme parts them the change is this line and no other.
            R::Number | R::Constant => "syntax-number",
            R::Keyword => "syntax-keyword",
            R::Type => "syntax-type",
            R::Function => "syntax-function",
            // The same again: both are a marker ABOUT a block rather than the
            // block — an attribute over a struct, a heading over a paragraph.
            R::Attribute | R::Heading => "syntax-attribute",
            R::Property => "syntax-property",
            R::Tag => "syntax-tag",
            R::Added => "syntax-added",
            R::Removed => "syntax-removed",
        })
    }

    fn glyph_name(icon: Self::Icon) -> alloc::string::String {
        alloc::string::String::from(icon)
    }
}
