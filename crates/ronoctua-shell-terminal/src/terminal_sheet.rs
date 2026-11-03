//! What a terminal's frame becomes on the window: the colours of the page,
//! resolved, and the grid of cells the door draws (`ronoctua_render::cells`).
//!
//! # Roles and data
//!
//! The terminal's OWN colours are the theme's: the sixteen ANSI ones, the
//! default foreground and background, the cursor and the selection are the
//! `terminal.*` roles of the page the window wears, resolved at every frame
//! ([`Palette::of_the_page`]) so a theme changed is a terminal recoloured.
//! The window hands them to the holder ([`Palette::wire`]), which gives them
//! to its engine, and they come back resolved into every cell. What a
//! PROGRAM asks for — SGR's 256 colours and 24-bit ones, what OSC 4/10/11
//! changed — is content, like an emoji's colours (the owner's exception to
//! invariant 9), and reaches the sheet as the data it is. Colours 16 to 255,
//! which no role names, are the engine's own table ([`engine_palette`]).
//!
//! # Only the rows that moved
//!
//! The engine's render state says which rows changed since the last frame;
//! only those are written into the sheet, and the sheet's generations make
//! the door rebuild only those on the GPU. A change that touches every row —
//! the size, the default colours, the page's own — writes them all.

use std::sync::OnceLock;

use ronoctua_render::cells::{self, Cell, Defaults, Grapheme, Rgba, Sheet, Style, Width};
use ronoctua_term::ghostty::{self, CellStyle, CellWidth, Dirty, Frame, Rgb};
use ronoctua_terminals::wire;
use ronoctua_tokens as tokens;

/// THE TERMINAL'S COLOURS ON THE PAGE THE WINDOW WEARS — roles, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// `terminal.foreground`.
    pub foreground: Rgba,
    /// `terminal.background`: the window's ground on the chrome's page, the
    /// desk on a desk's (the theme solves it so).
    pub background: Rgba,
    /// `terminal.cursor`.
    pub cursor: Rgba,
    /// `terminal.cursor-text`: the glyph under a block cursor.
    pub cursor_text: Rgba,
    /// `terminal.selection-background`.
    pub selection_background: Rgba,
    /// `terminal.selection-foreground`.
    pub selection_foreground: Rgba,
    /// `terminal.ansi.*`, in the order a program numbers them: black, red,
    /// green, yellow, blue, magenta, cyan, white, then their bright forms.
    pub ansi: [Rgba; 16],
    /// The theme is a light one — what a program asking (mode 2031) is told.
    pub light: bool,
}

/// The sixteen ANSI roles, in their numbers' order.
const ANSI: [tokens::Color; 16] = [
    tokens::color::TERMINAL_ANSI_BLACK,
    tokens::color::TERMINAL_ANSI_RED,
    tokens::color::TERMINAL_ANSI_GREEN,
    tokens::color::TERMINAL_ANSI_YELLOW,
    tokens::color::TERMINAL_ANSI_BLUE,
    tokens::color::TERMINAL_ANSI_MAGENTA,
    tokens::color::TERMINAL_ANSI_CYAN,
    tokens::color::TERMINAL_ANSI_WHITE,
    tokens::color::TERMINAL_ANSI_BRIGHT_BLACK,
    tokens::color::TERMINAL_ANSI_BRIGHT_RED,
    tokens::color::TERMINAL_ANSI_BRIGHT_GREEN,
    tokens::color::TERMINAL_ANSI_BRIGHT_YELLOW,
    tokens::color::TERMINAL_ANSI_BRIGHT_BLUE,
    tokens::color::TERMINAL_ANSI_BRIGHT_MAGENTA,
    tokens::color::TERMINAL_ANSI_BRIGHT_CYAN,
    tokens::color::TERMINAL_ANSI_BRIGHT_WHITE,
];

impl Palette {
    /// The roles on the page the process wears now — the door wears the
    /// window's before every frame.
    #[must_use]
    pub fn of_the_page() -> Self {
        Self {
            foreground: Rgba::of(tokens::color::TERMINAL_FOREGROUND),
            background: Rgba::of(tokens::color::TERMINAL_BACKGROUND),
            cursor: Rgba::of(tokens::color::TERMINAL_CURSOR),
            cursor_text: Rgba::of(tokens::color::TERMINAL_CURSOR_TEXT),
            selection_background: Rgba::of(tokens::color::TERMINAL_SELECTION_BACKGROUND),
            selection_foreground: Rgba::of(tokens::color::TERMINAL_SELECTION_FOREGROUND),
            ansi: ANSI.map(Rgba::of),
            light: tokens::theme::is_light(),
        }
    }

    /// The colours as the holder's engine takes them: these sixteen, and the
    /// engine's own table from 16 on.
    #[must_use]
    pub fn wire(&self) -> wire::Colors {
        let rgb = |c: Rgba| wire::Rgb([c.r, c.g, c.b]);
        let engine = engine_palette();
        let palette = (0..256)
            .map(|i| {
                self.ansi.get(i).map_or_else(
                    || wire::Rgb([engine[i].r, engine[i].g, engine[i].b]),
                    |c| rgb(*c),
                )
            })
            .collect();
        wire::Colors {
            foreground: rgb(self.foreground),
            background: rgb(self.background),
            cursor: Some(rgb(self.cursor)),
            palette,
            scheme: if self.light {
                wire::Scheme::Light
            } else {
                wire::Scheme::Dark
            },
        }
    }
}

/// THE ENGINE'S OWN 256 COLOURS — the table a terminal starts with, read off
/// an engine that was never fed: the 6×6×6 cube and the greys a program
/// means by 16 to 255, as the library defines them, and not a copy of them
/// written here (a table of colours by hand is what invariant 9 forbids).
///
/// Once per process; `None` only if the library could not make a terminal
/// at all, and then every entry is the foreground's place-holder black.
#[must_use]
pub fn engine_palette() -> &'static [Rgb; 256] {
    static TABLE: OnceLock<Box<[Rgb; 256]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let read = || -> Option<Box<[Rgb; 256]>> {
            let vt = ghostty::Vt::new(1, 1, &ghostty::Config::for_version(VERSION)).ok()?;
            let state = ghostty::RenderState::new().ok()?;
            state.update(&vt).ok()?;
            let palette = state.frame().colors().palette;
            Some(palette)
        };
        read().unwrap_or_else(|| {
            tracing::warn!("the engine gave no colour table; colours past 15 draw black");
            Box::new([Rgb { r: 0, g: 0, b: 0 }; 256])
        })
    })
}

/// The distro's version, which the in-process engine answers XTVERSION with.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// An engine colour as a sheet's.
#[must_use]
pub const fn rgba(c: Rgb) -> Rgba {
    Rgba::opaque(c.r, c.g, c.b)
}

/// The engine's colours for the wire's — what a mirror is given when the
/// holder says its colours changed. `None` for a palette that is not 256.
#[must_use]
pub fn engine_colors(c: &wire::Colors) -> Option<ghostty::Colors> {
    let palette = c.palette_of_256()?;
    let rgb = |w: wire::Rgb| Rgb {
        r: w.0[0],
        g: w.0[1],
        b: w.0[2],
    };
    Some(ghostty::Colors {
        foreground: rgb(c.foreground),
        background: rgb(c.background),
        cursor: c.cursor.map(rgb),
        palette: Box::new(palette.map(rgb)),
        scheme: match c.scheme {
            wire::Scheme::Light => ghostty::Scheme::Light,
            wire::Scheme::Dark => ghostty::Scheme::Dark,
        },
    })
}

/// What a frame said, past its cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Filled {
    /// The ground the cells are drawn on — the window's clear colour: the
    /// page's `terminal.background` unless a program changed it (OSC 11).
    pub background: Rgba,
    /// The cursor is on screen and blinks: the blink's clock is wanted.
    pub blinks: bool,
    /// How many rows were written.
    pub rows: usize,
}

/// How a frame is to be drawn on the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Showing {
    /// The window has the keyboard: the cursor is drawn as the program
    /// asked; without it, hollow.
    pub focused: bool,
    /// The blink is in its dark half.
    pub blink_off: bool,
    /// Every row is to be written, whatever the frame says moved — the page
    /// changed, or something was drawn over a row.
    pub all: bool,
    /// The faint ink's alpha.
    pub faint: u8,
}

/// Writes `frame` into `sheet`: the rows that moved (or all of them), the
/// default colours, and the cursor.
pub fn fill(
    sheet: &mut Sheet,
    frame: &mut Frame<'_>,
    palette: &Palette,
    showing: Showing,
) -> Filled {
    let (columns, rows) = frame.size();
    let mut all = showing.all;
    if (columns, rows) != (sheet.columns(), sheet.lines()) {
        sheet.resize(columns, rows);
        all = true;
    }
    let colors = frame.colors();
    let defaults = Defaults {
        foreground: rgba(colors.foreground),
        background: rgba(colors.background),
        faint: showing.faint,
    };
    if *sheet.defaults() != defaults {
        sheet.set_defaults(defaults);
        all = true;
    }
    sheet.set_blink_off(showing.blink_off);
    let every = all || frame.dirty() == Dirty::Full;
    let mut written = 0;
    if every || frame.dirty() != Dirty::Clean {
        frame.rows(|row| {
            if !every && !row.dirty {
                return;
            }
            let Ok(line) = u16::try_from(row.y) else {
                return;
            };
            let cells = sheet.row_mut(line);
            for (place, cell) in cells.iter_mut().zip(row.cells) {
                *place = cell_of(row.text(cell), cell, &defaults, palette);
            }
            written += 1;
        });
    }
    let cursor = frame.cursor();
    sheet.cursor = cursor
        .at
        .filter(|_| cursor.visible)
        .map(|(x, y)| cells::Cursor {
            line: y,
            // ON THE RIGHT HALF OF A WIDE CHARACTER the cursor is drawn on the
            // character, which the left half begins.
            column: if cursor.wide_tail {
                x.saturating_sub(1)
            } else {
                x
            },
            shape: if showing.focused {
                match cursor.shape {
                    ghostty::CursorShape::Bar => cells::CursorShape::Bar,
                    ghostty::CursorShape::Underline => cells::CursorShape::Underline,
                    ghostty::CursorShape::Hollow => cells::CursorShape::Hollow,
                    ghostty::CursorShape::Block => cells::CursorShape::Block,
                }
            } else {
                // A WINDOW WITHOUT THE KEYBOARD says so in its cursor, Ghostty's
                // way: the block's outline, wherever the program put it.
                cells::CursorShape::Hollow
            },
            visible: !(showing.focused && cursor.blinking && showing.blink_off),
            colour: colors.cursor.map_or(palette.cursor, rgba),
            text: palette.cursor_text,
        });
    frame.clean();
    Filled {
        background: defaults.background,
        blinks: showing.focused && cursor.visible && cursor.blinking && cursor.at.is_some(),
        rows: written,
    }
}

/// One engine cell as a sheet's: its grapheme, its colours resolved (the
/// default where it names none, swapped for inverse, the selection's where
/// it is selected), its style and its width.
#[must_use]
pub fn cell_of(text: &str, cell: &ghostty::Cell, defaults: &Defaults, palette: &Palette) -> Cell {
    let mut fg = cell.fg.map_or(defaults.foreground, rgba);
    let mut bg = cell.bg.map(rgba);
    if cell.style.inverse {
        // THE DEFAULTS SWAP TOO: a cell that named no ground is drawn on the
        // foreground, and its ink is the window's own ground.
        let ground = bg.unwrap_or(defaults.background);
        bg = Some(fg);
        fg = ground;
    }
    if cell.selected {
        fg = palette.selection_foreground;
        bg = Some(palette.selection_background);
    }
    let width = match cell.width {
        CellWidth::Narrow => Width::Narrow,
        CellWidth::Wide => Width::Wide,
        CellWidth::SpacerTail => Width::Spacer,
        // THE END OF A LINE A WIDE CHARACTER DID NOT FIT ON draws nothing
        // of its own: a blank in its ground.
        CellWidth::SpacerHead => Width::Narrow,
    };
    let grapheme = if cell.width == CellWidth::SpacerHead {
        Grapheme::default()
    } else {
        Grapheme::of(text)
    };
    Cell {
        grapheme,
        fg,
        bg,
        underline_colour: cell.underline_colour.map(rgba),
        style: style_of(cell.style),
        width,
    }
}

/// The engine's style as a sheet's — inverse resolved by [`cell_of`].
#[must_use]
pub const fn style_of(s: CellStyle) -> Style {
    Style {
        bold: s.bold,
        italic: s.italic,
        faint: s.faint,
        blink: s.blink,
        invisible: s.invisible,
        strike: s.strikethrough,
        overline: s.overline,
        underline: match s.underline {
            ghostty::Underline::None => cells::Underline::None,
            ghostty::Underline::Single => cells::Underline::Single,
            ghostty::Underline::Double => cells::Underline::Double,
            ghostty::Underline::Curly => cells::Underline::Curly,
            ghostty::Underline::Dotted => cells::Underline::Dotted,
            ghostty::Underline::Dashed => cells::Underline::Dashed,
        },
    }
}

/// The text of a sheet's line, its graphemes in a row — what a test reads
/// of what the window would draw.
#[must_use]
pub fn line_text(sheet: &Sheet, line: u16) -> String {
    sheet
        .row(line)
        .iter()
        .filter(|c| c.width != Width::Spacer)
        .map(|c| match &c.grapheme {
            Grapheme::One(ch) => ch.to_string(),
            Grapheme::Many(s) => s.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Defaults {
        Defaults {
            foreground: Rgba::opaque(200, 200, 200),
            background: Rgba::opaque(10, 10, 10),
            faint: 128,
        }
    }

    fn palette() -> Palette {
        Palette {
            foreground: Rgba::opaque(200, 200, 200),
            background: Rgba::opaque(10, 10, 10),
            cursor: Rgba::opaque(250, 250, 250),
            cursor_text: Rgba::opaque(0, 0, 0),
            selection_background: Rgba::opaque(60, 60, 120),
            selection_foreground: Rgba::opaque(255, 255, 255),
            ansi: [Rgba::opaque(1, 2, 3); 16],
            light: false,
        }
    }

    #[test]
    fn a_cell_with_no_colour_wears_the_defaults_and_inverse_swaps_them() {
        // FAILS IF a cell that named no colour is drawn in something other
        // than the terminal's foreground on its own ground, or inverse leaves
        // a default unswapped (white-on-black text that stays white on black).
        let plain = ghostty::Cell::default();
        let c = cell_of("a", &plain, &defaults(), &palette());
        assert_eq!(c.fg, defaults().foreground);
        assert_eq!(c.bg, None, "the default ground is the clear colour");
        let mut inverse = ghostty::Cell::default();
        inverse.style.inverse = true;
        let c = cell_of("a", &inverse, &defaults(), &palette());
        assert_eq!(c.fg, defaults().background);
        assert_eq!(c.bg, Some(defaults().foreground));
    }

    #[test]
    fn a_programs_colour_is_data_and_a_selection_is_the_pages() {
        // FAILS IF the colour a program asked for is not the one drawn (a
        // 24-bit SGR turned into a role), or a selected cell keeps the
        // program's colours instead of the page's selection.
        let red = Rgb { r: 255, g: 0, b: 0 };
        let blue = Rgb { r: 0, g: 0, b: 255 };
        let mut coloured = ghostty::Cell::default();
        coloured.fg = Some(red);
        coloured.bg = Some(blue);
        let c = cell_of("x", &coloured, &defaults(), &palette());
        assert_eq!((c.fg, c.bg), (rgba(red), Some(rgba(blue))));
        let mut selected = coloured;
        selected.selected = true;
        let c = cell_of("x", &selected, &defaults(), &palette());
        assert_eq!(c.fg, palette().selection_foreground);
        assert_eq!(c.bg, Some(palette().selection_background));
    }

    #[test]
    fn a_wide_cell_is_two_and_its_tail_draws_nothing() {
        // FAILS IF a wide character's second half is drawn as a cell of its
        // own, or the head spacer at a line's end shows a character.
        let mut wide = ghostty::Cell::default();
        wide.width = CellWidth::Wide;
        assert_eq!(
            cell_of("日", &wide, &defaults(), &palette()).width,
            Width::Wide
        );
        let mut tail = ghostty::Cell::default();
        tail.width = CellWidth::SpacerTail;
        assert_eq!(
            cell_of("", &tail, &defaults(), &palette()).width,
            Width::Spacer
        );
        let mut head = ghostty::Cell::default();
        head.width = CellWidth::SpacerHead;
        let c = cell_of("?", &head, &defaults(), &palette());
        assert_eq!((c.width, c.grapheme), (Width::Narrow, Grapheme::default()));
    }

    #[test]
    fn the_wire_carries_the_sixteen_roles_and_the_engines_table_after() {
        // FAILS IF the holder is handed a palette of another length (it
        // refuses one that is not 256), the sixteen are not the roles', or
        // the rest is not the engine's own table — a 256-colour program would
        // draw in colours nobody chose.
        let p = palette();
        let w = p.wire();
        assert_eq!(w.palette.len(), 256);
        assert!(w.palette_of_256().is_some());
        assert_eq!(w.palette[3], wire::Rgb([1, 2, 3]));
        let engine = engine_palette();
        for i in [16usize, 100, 231, 232, 255] {
            assert_eq!(
                w.palette[i],
                wire::Rgb([engine[i].r, engine[i].g, engine[i].b]),
                "{i}"
            );
        }
        // The cube is not one colour: a table that came back empty would be.
        assert_ne!(engine[16], engine[231]);
        assert_eq!(w.foreground, wire::Rgb([200, 200, 200]));
        assert_eq!(w.scheme, wire::Scheme::Dark);
        // AND IT CROSSES BACK whole.
        let back = engine_colors(&w).expect("256 entries");
        assert_eq!(back.palette[3], Rgb { r: 1, g: 2, b: 3 });
    }

    #[test]
    fn every_page_has_its_own_terminal_colours() {
        // FAILS IF the window's colours stop following the page it wears —
        // a terminal on the desk drawn on the chrome's ground — or a role is
        // not what the palette says. THE ONLY TEST HERE THAT MOVES THE GROUND
        // (`theme::set_ground` is the process's): the others never read the
        // page.
        let mut seen = Vec::new();
        for ground in tokens::theme::Ground::ALL {
            tokens::theme::set_ground(ground);
            let p = Palette::of_the_page();
            assert_eq!(p.background, Rgba::of(tokens::color::TERMINAL_BACKGROUND));
            assert_eq!(p.ansi[1], Rgba::of(tokens::color::TERMINAL_ANSI_RED));
            seen.push(p.background);
        }
        tokens::theme::set_ground(tokens::theme::Ground::Chrome);
        seen.dedup();
        assert!(seen.len() > 1, "every page drew the same ground: {seen:?}");
    }
}
