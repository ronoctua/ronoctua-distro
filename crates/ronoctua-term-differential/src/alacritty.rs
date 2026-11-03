//! The `alacritty_terminal` engine — the differential's second opinion.
//!
//! It was the engine milestone 7 closed with, behind the same
//! [`crate::Engine`]. The product runs on `libghostty-vt` now (risk R1 is
//! closed), and this one stays for what it is good at: being a SECOND,
//! independently written reading of the same bytes, so a wrong reading of the
//! VT specification shows up as two engines disagreeing.

use std::sync::{Arc, Mutex, PoisonError};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, test::TermSize};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb};

use crate::{CellBox, Colour, Engine, Grid, Style, Width};

/// An answer alacritty asked to have written, in the order it asked.
///
/// A colour answer cannot be written where it is asked: alacritty hands the
/// listener a FORMATTER and expects the host to look the colour up, and the
/// colours live in the `Term` that is busy parsing at that moment. So it waits
/// here and is resolved after the parse, still in its place among the others.
enum Pending {
    /// Bytes to write as they are.
    Bytes(Vec<u8>),
    /// A colour query: the slot alacritty numbers (0–255 the palette, 256 the
    /// foreground, 257 the background, 258 the cursor) and how to spell it.
    Colour(usize, Arc<dyn Fn(Rgb) -> String + Sync + Send + 'static>),
}

/// alacritty's `Term` requires an event listener. Ours keeps what would have
/// gone back to the PTY — a query nobody answers is a program that times out
/// or degrades — and drops the rest: the title, the bell and the clipboard are
/// the product engine's business, not the differential's.
#[derive(Clone, Default)]
struct Listener {
    pending: Arc<Mutex<Vec<Pending>>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let pending = match event {
            Event::PtyWrite(text) => Pending::Bytes(text.into_bytes()),
            Event::ColorRequest(slot, format) => Pending::Colour(slot, format),
            // CSI 14 t asks for the text area in PIXELS, and a differential
            // run has no pixels to report: no window exists. Answering with
            // an invented size would be a reply both engines agree on by
            // accident.
            _ => return,
        };
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(pending);
    }
}

/// An engine based on `alacritty_terminal`.
pub struct Alacritty {
    term: Term<Listener>,
    parser: Processor,
    listener: Listener,
    /// The colours a query falls back to when no OSC set one: the same table
    /// the Ghostty engine is handed ([`crate::reference_colours`]).
    colours: crate::ReferenceColours,
    replies: Vec<u8>,
}

impl Alacritty {
    /// A terminal of the given size.
    #[must_use]
    pub fn fresh(columns: u16, lines: u16) -> Self {
        let size = TermSize::new(usize::from(columns), usize::from(lines));
        let listener = Listener::default();
        Self {
            term: Term::new(Config::default(), &size, listener.clone()),
            parser: Processor::new(),
            listener,
            colours: crate::reference_colours(),
            replies: Vec::new(),
        }
    }

    /// The colour alacritty would answer for a slot: what an OSC set, or the
    /// reference table.
    fn colour_of(&self, slot: usize) -> Rgb {
        if let Some(rgb) = self.term.colors()[slot] {
            return rgb;
        }
        let c = &self.colours;
        let (r, g, b) = match slot {
            0..=255 => c.palette[slot],
            256 => c.foreground,
            257 => c.background,
            258 => c.cursor,
            _ => (0, 0, 0),
        };
        Rgb { r, g, b }
    }
}

/// A cell's colour as the canonical grid spells it.
///
/// The named foreground and background (and the dim and bright variants
/// alacritty only ever uses while drawing) are the DEFAULT; the sixteen named
/// colours are palette entries, which is what `SGR 31` and `SGR 38;5;1` both
/// mean to Ghostty too.
fn colour(c: Color) -> Colour {
    match c {
        Color::Named(named) => {
            let index = named as usize;
            u8::try_from(index)
                .ok()
                .filter(|i| *i < 16)
                .map_or(Colour::Default, Colour::Palette)
        }
        Color::Indexed(i) => Colour::Palette(i),
        Color::Spec(Rgb { r, g, b }) => Colour::Rgb(r, g, b),
    }
}

impl Engine for Alacritty {
    fn name(&self) -> &'static str {
        "alacritty"
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
        let pending = std::mem::take(
            &mut *self
                .listener
                .pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        for p in pending {
            match p {
                Pending::Bytes(b) => self.replies.extend_from_slice(&b),
                Pending::Colour(slot, format) => {
                    let text = format(self.colour_of(slot));
                    self.replies.extend_from_slice(text.as_bytes());
                }
            }
        }
    }

    fn grid(&self) -> Grid {
        let g = self.term.grid();
        let columns = u16::try_from(g.columns()).unwrap_or(u16::MAX);
        let lines = u16::try_from(g.screen_lines()).unwrap_or(u16::MAX);
        let mut out = Grid::fresh_one(columns, lines);

        for l in 0..lines {
            for c in 0..columns {
                let p = Point::new(Line(i32::from(l)), Column(usize::from(c)));
                let cell = &g[p];
                let flags = cell.flags;
                let width = if flags.contains(Flags::WIDE_CHAR) {
                    Width::Wide
                } else if flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    Width::Spacer
                } else {
                    Width::Narrow
                };
                // A spacer holds no text — the character it belongs to is
                // drawn by its neighbour — and Ghostty's spacer has none.
                let text = if width == Width::Spacer {
                    String::new()
                } else {
                    std::iter::once(cell.c)
                        .chain(cell.zerowidth().unwrap_or_default().iter().copied())
                        .collect()
                };
                let style = Style {
                    bold: flags.contains(Flags::BOLD),
                    italic: flags.contains(Flags::ITALIC),
                    faint: flags.contains(Flags::DIM),
                    inverse: flags.contains(Flags::INVERSE),
                    invisible: flags.contains(Flags::HIDDEN),
                    underline: flags.intersects(Flags::ALL_UNDERLINES),
                    strikethrough: flags.contains(Flags::STRIKEOUT),
                    fg: colour(cell.fg),
                    bg: colour(cell.bg),
                };
                out.put_at(l, c, CellBox { text, style, width });
            }
        }

        let cursor = self.term.grid().cursor.point;
        out.cursor = (
            u16::try_from(cursor.line.0.max(0)).unwrap_or(0),
            u16::try_from(cursor.column.0).unwrap_or(0),
        );
        out
    }

    fn resize(&mut self, columns: u16, lines: u16) {
        self.term
            .resize(TermSize::new(usize::from(columns), usize::from(lines)));
    }

    fn replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }
}

// `NamedColor` is matched by its number above; this keeps the numbering the
// code relies on from moving under it without a test noticing.
const _: () = assert!(NamedColor::BrightWhite as usize == 15);
const _: () = assert!(NamedColor::Foreground as usize == 256);

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_for(bytes: &[u8]) -> String {
        let mut m = Alacritty::fresh(20, 4);
        m.feed(bytes);
        m.grid().canonical_f()
    }

    #[test]
    fn plain_text() {
        assert!(grid_for(b"ola").starts_with("ola\n"));
    }

    #[test]
    fn a_line_break_is_a_return() {
        // `\n` without `\r` does NOT return to column zero in a terminal: it is the
        // classic mistake of whoever writes a test corpus by hand.
        let g = grid_for(b"ab\r\ncd");
        assert!(g.starts_with("ab\ncd\n"), "{g}");
    }

    #[test]
    fn absolute_positioning() {
        // `ESC[3;5H` = row 3, column 5, counting from 1.
        let g = grid_for(b"\x1b[3;5Hx");
        let lines: Vec<&str> = g.lines().collect();
        assert_eq!(lines[2], "    x", "{g}");
    }

    #[test]
    fn clears_the_screen() {
        let g = grid_for(b"lixo\x1b[2J\x1b[Hok");
        assert!(g.starts_with("ok\n"), "{g}");
    }

    #[test]
    fn an_invalid_sequence_does_not_bring_it_down() {
        // A PTY can emit any byte. The engine has to survive: if it
        // panicked, a badly behaved program would take the shell down.
        let g = grid_for(b"\x1b[999999999999;9999999H\x1b[?x");
        assert!(!g.is_empty());
    }

    #[test]
    fn a_cursor_query_is_answered() {
        // FAILS IF `PtyWrite` is dropped again, as the old `Deaf` listener did:
        // a program asking where the cursor is would wait for an answer that
        // never comes.
        let mut m = Alacritty::fresh(20, 4);
        m.feed(b"ab\x1b[6n");
        assert_eq!(m.replies(), b"\x1b[1;3R");
        assert!(m.replies().is_empty(), "replies are taken, not repeated");
    }

    #[test]
    fn a_colour_query_is_answered_from_the_reference_table() {
        // FAILS IF the colour query is dropped, or answered from a table other
        // than the one the Ghostty engine is given: the differential compares
        // the two answers.
        let mut m = Alacritty::fresh(20, 4);
        m.feed(b"\x1b]4;1;?\x07");
        let replies = crate::comparable_replies(&m.replies());
        assert_eq!(
            replies,
            [crate::Reply::Colour {
                slot: "4;1".to_owned(),
                rgb: crate::reference_colours().palette[1]
            }]
        );
    }

    #[test]
    fn a_colour_set_by_the_program_is_the_one_answered() {
        // FAILS IF the query is answered from the reference table even after
        // the program changed the colour.
        let mut m = Alacritty::fresh(20, 4);
        m.feed(b"\x1b]11;rgb:12/34/56\x07\x1b]11;?\x07");
        assert_eq!(
            crate::comparable_replies(&m.replies()),
            [crate::Reply::Colour {
                slot: "11".to_owned(),
                rgb: (0x12, 0x34, 0x56)
            }]
        );
    }

    #[test]
    fn styles_reach_the_grid() {
        // FAILS IF a cell's SGR attributes are lost on the way to the grid —
        // the cross-engine style comparison would then compare nothing.
        let mut m = Alacritty::fresh(20, 2);
        m.feed(b"\x1b[1;31mA\x1b[0m\x1b[48;2;1;2;3mB");
        let g = m.grid();
        let a = g.cell(0, 0).style;
        assert!(a.bold);
        assert_eq!(a.fg, Colour::Palette(1));
        assert_eq!(g.cell(0, 1).style.bg, Colour::Rgb(1, 2, 3));
        assert_eq!(g.cell(0, 1).style.fg, Colour::Default);
    }
}
