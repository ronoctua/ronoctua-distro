//! The guest half: the palette that resolves a role to the generated enum, and
//! the conversion from the frame this crate builds to the one a component
//! returns.
//!
//! # Why there is a conversion at all
//!
//! Because the two sides spell a colour differently and neither spelling can be
//! translated into the other at runtime. On the host a colour is a token's
//! NAME; inside the component it is the enum generated from
//! `assets/tokens/colors.nix`, and WIT produces no reverse table — a name comes
//! in and nothing turns it back into a case. Writing that table by hand would
//! be a second copy of the tokens, which invariant 9 exists to prevent.
//!
//! So the frame is built out of ROLES, and the role is resolved once, here, at
//! the end. Seven lines, and the compiler checks them for completeness.

use alloc::string::String;
use alloc::vec::Vec;

use crate::bindings::ronoctua::buffer::types as wit;
use crate::screen::{Frame, Ink, Screen};
use crate::{Palette, Role};

/// The component's palette: a role becomes a case of the generated enum.
///
/// The twin of [`Tokens`](crate::Tokens), and it has to name the same token for
/// every role — the two tables sit six lines apart for exactly that reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wit;

impl Palette for Wit {
    type Color = wit::Color;
    type Icon = wit::Icon;

    fn color(role: Role) -> Self::Color {
        match role {
            Role::Title => wit::Color::TextPrimary,
            Role::Body => wit::Color::TextSecondary,
            // TWO TOKENS SINCE 2026-09-20; the argument is in `Tokens::color`,
            // and this table is the one that has to agree with it.
            Role::Quiet => wit::Color::TextSecondary,
            Role::Dim => wit::Color::TextDim,
            Role::Accent => wit::Color::StateAccent,
            Role::Rule => wit::Color::RuleDefault,
            Role::RuleRaised => wit::Color::RuleRaised,
            Role::Band => wit::Color::SurfaceSelection,
            Role::Shelf => wit::Color::SurfaceContainer,
            Role::Header => wit::Color::SurfaceHeader,
            Role::Danger => wit::Color::StateError,
            Role::Once => wit::Color::TextOnce,
            Role::Hover => wit::Color::SurfaceHover,
            Role::HoverOf(0) => wit::Color::CursorHoverOne,
            Role::HoverOf(1) => wit::Color::CursorHoverTwo,
            Role::HoverOf(2) => wit::Color::CursorHoverThree,
            Role::HoverOf(3) => wit::Color::CursorHoverFour,
            Role::HoverOf(4) => wit::Color::CursorHoverFive,
            Role::HoverOf(5) => wit::Color::CursorHoverSix,
            Role::HoverOf(6) => wit::Color::CursorHoverSeven,
            Role::HoverOf(_) => wit::Color::CursorHoverEight,
            Role::SelectionOf(0) => wit::Color::CursorSelectionOne,
            Role::SelectionOf(1) => wit::Color::CursorSelectionTwo,
            Role::SelectionOf(2) => wit::Color::CursorSelectionThree,
            Role::SelectionOf(3) => wit::Color::CursorSelectionFour,
            Role::SelectionOf(4) => wit::Color::CursorSelectionFive,
            Role::SelectionOf(5) => wit::Color::CursorSelectionSix,
            Role::SelectionOf(6) => wit::Color::CursorSelectionSeven,
            Role::SelectionOf(_) => wit::Color::CursorSelectionEight,
            Role::Key => wit::Color::TextSubtleStrong,
            Role::Prose => wit::Color::TextSubtleDefault,
            Role::Faint => wit::Color::TextSubtleFaint,
            Role::Explain => wit::Color::TextSubtleExplain,
        }
    }

    #[cfg(feature = "syntax")]
    fn syntax(role: ronoctua_syntax::Role) -> Self::Color {
        use ronoctua_syntax::Role as R;

        // THE SAME TWELVE TOKENS AS `Tokens::syntax`, and the two tables are
        // one file apart for exactly that reason: a role that named a
        // different colour on the two sides would draw one thing in the
        // renderer's own tests and another inside a component.
        match role {
            R::Comment => wit::Color::SyntaxComment,
            R::String => wit::Color::SyntaxString,
            R::Escape => wit::Color::SyntaxEscape,
            R::Number | R::Constant => wit::Color::SyntaxNumber,
            R::Keyword => wit::Color::SyntaxKeyword,
            R::Type => wit::Color::SyntaxType,
            R::Function => wit::Color::SyntaxFunction,
            R::Attribute | R::Heading => wit::Color::SyntaxAttribute,
            R::Property => wit::Color::SyntaxProperty,
            R::Tag => wit::Color::SyntaxTag,
            R::Added => wit::Color::SyntaxAdded,
            R::Removed => wit::Color::SyntaxRemoved,
        }
    }

    fn glyph_name(icon: Self::Icon) -> String {
        // DIAGNOSTIC ONLY, and the doc on the trait says so: the enum has no
        // name at runtime, so this is the variant's `Debug` spelling. What
        // reads it is `Screen::as_protocol`, whose answer to the host's two
        // questions does not depend on the string — `columns_of` charges an
        // icon `heights.of(scale)` cells whatever it says.
        alloc::format!("{icon:?}")
    }
}

/// The context, as the arithmetic here speaks it.
///
/// A conversion and not a second `Context`: the record that crosses the WIT and
/// [`ronoctua_protocol::Context`] are the same three numbers, and the buffer
/// should be handing the one it received straight to [`Screen::new`].
impl From<&wit::Context> for ronoctua_protocol::Context {
    fn from(ctx: &wit::Context) -> Self {
        Self {
            columns: ctx.columns,
            lines: ctx.lines,
            heights: ronoctua_protocol::Heights {
                small: ctx.heights.small,
                base: ctx.heights.base,
                normal: ctx.heights.normal,
                large: ctx.heights.large,
                huge: ctx.heights.huge,
            },
        }
    }
}

/// The size, as the WIT spells it.
///
/// A `match` and not a cast: the two enums are generated from the same source
/// and happen to be in the same order today, and "happen to be" is what a cast
/// would depend on.
const fn scale(scale: ronoctua_protocol::Scale) -> wit::Scale {
    match scale {
        ronoctua_protocol::Scale::Small => wit::Scale::Small,
        ronoctua_protocol::Scale::Base => wit::Scale::Base,
        ronoctua_protocol::Scale::Normal => wit::Scale::Normal,
        ronoctua_protocol::Scale::Large => wit::Scale::Large,
        ronoctua_protocol::Scale::Huge => wit::Scale::Huge,
    }
}

impl<P: Palette<Color = wit::Color, Icon = wit::Icon>> Screen<P> {
    /// A fresh frame for the context the host just sent.
    ///
    /// The first line of a buffer's `render`, and the reason a component never
    /// has to name [`ronoctua_protocol::Context`]: the two records are the same
    /// three numbers and this is where the one becomes the other.
    #[must_use]
    pub fn of(ctx: &wit::Context) -> Self {
        Self::new(&ctx.into())
    }

    /// The frame, as the component returns it.
    ///
    /// This is the last line of a buffer's `render`. Everything before it is
    /// the arithmetic in this crate, which is the same arithmetic `cargo test`
    /// already asked the host's two questions of.
    #[must_use]
    pub fn finish_wit(&self) -> Vec<wit::Command> {
        // THE ANCHOR FIRST — a fact about the frame, read before anything is
        // placed in it. See `ronoctua_protocol::Command::Anchor`.
        let anchor = self
            .foot_from()
            .map(|foot_from| wit::Command::Anchor(wit::AnchorCommand { foot_from }));
        anchor
            .into_iter()
            .chain(self.parts().map(|part| match part {
                Frame::Ran(line, run) => wit::Command::Text(wit::TextCommand {
                    line,
                    column: run.column,
                    content: match &run.ink {
                        Ink::Datum(t) => wit::Text::Data(t.clone()),
                        Ink::Label(k) => wit::Text::Key(k.clone()),
                        Ink::Glyph(i) => wit::Text::Icon(*i),
                    },
                    color: P::color(run.role),
                    scale: scale(run.scale),
                    max_width: run.budget,
                    edge: run.edge,
                }),
                #[cfg(feature = "syntax")]
                Frame::Coded(line, column, text, role) => wit::Command::Text(wit::TextCommand {
                    line,
                    column,
                    content: wit::Text::Data(text.into()),
                    color: role.map_or_else(|| P::color(Role::Body), P::syntax),
                    scale: wit::Scale::Normal,
                    max_width: 0,
                    edge: false,
                }),
                // THE ROLE TRAVELS HERE TOO, and for the same reason it does
                // on the ground: a rule is a signal drawn ON something, and
                // which rule reads depends on what that something is.
                Frame::Ruled(line, column, columns, role) => wit::Command::Rule(wit::BandCommand {
                    line,
                    column,
                    columns,
                    color: P::color(role),
                }),
                // THE ROLE TRAVELS, for the reason the host's own conversion
                // gives: a container's ground and a focused row's are one
                // primitive in two registers.
                Frame::Banded(line, column, columns, role) => {
                    wit::Command::Selection(wit::BandCommand {
                        line,
                        column,
                        columns,
                        color: P::color(role),
                    })
                }
                // AND A PARAGRAPH CROSSES AS A KEY AND A RECTANGLE, for the
                // reason `Screen::paragraph` gives: wrapping needs the
                // translated string, which this side does not have.
                Frame::Wrapped(line, lines, from, run) => {
                    wit::Command::Paragraph(wit::ParagraphCommand {
                        line,
                        column: run.column,
                        lines,
                        columns: run.budget,
                        content: match &run.ink {
                            Ink::Datum(t) => wit::Text::Data(t.clone()),
                            Ink::Label(k) => wit::Text::Key(k.clone()),
                            Ink::Glyph(i) => wit::Text::Icon(*i),
                        },
                        color: P::color(run.role),
                        scale: scale(run.scale),
                        first_row: from,
                    })
                }
                // THE BAR CROSSES AS NUMBERS. What it becomes is a thumb in
                // pixels, and only the host can resolve that — the track's
                // residue is a fact about the surface, which is a thing this
                // side has never been told (invariant 11).
                Frame::Barred(line, lines, column, scroll, role) => {
                    wit::Command::Bar(wit::BarCommand {
                        line,
                        lines,
                        column,
                        total: scroll.total as u32,
                        shown: scroll.lines as u32,
                        offset: scroll.offset as u32,
                        color: P::color(role),
                    })
                }
            }))
            .collect()
    }
}
