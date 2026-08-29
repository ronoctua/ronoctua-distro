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
            // The same one token, and still two roles: see `Tokens::color`.
            Role::Quiet | Role::Dim => wit::Color::TextDim,
            Role::Accent => wit::Color::StateAccent,
            Role::Rule => wit::Color::RuleDefault,
            Role::Band => wit::Color::SurfaceSelection,
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
        self.parts()
            .map(|part| match part {
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
                }),
                Frame::Ruled(line) => wit::Command::Rule(wit::LineCommand {
                    line,
                    color: P::color(Role::Rule),
                }),
                Frame::Banded(line) => wit::Command::Selection(wit::LineCommand {
                    line,
                    color: P::color(Role::Band),
                }),
            })
            .collect()
    }
}
