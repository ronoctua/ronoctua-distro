//! The Ghostty engine as the differential runs it.
//!
//! The product engine itself, `ronoctua_term::ghostty::Vt`, set up the way a
//! corpus run needs and read through its safe API: the cells by
//! `Vt::written`, the answers by its effects. No `unsafe` and no binding
//! here — the binding is `ronoctua-term`'s one file that may write it.

use ronoctua_term::ghostty::{AskedColour, CellWidth, Config, Refused, Underline, Vt, Written};

use crate::{CellBox, Colour, Engine, Grid, Style, Width};

/// The pixels of one cell in a corpus run: no window exists to measure one,
/// and the goldens were recorded at this size.
const CELL_PX: u32 = 1;

/// The Ghostty terminal as the differential builds it: the reference colours
/// both engines are handed ([`crate::reference_colours`]), one pixel a
/// cell, and no sixel claim — a corpus run has no inbound filter to turn
/// sixel into images. XTVERSION names the distro's version `differential`,
/// which the goldens record.
///
/// Not the product's terminal: a holder is built with the window's colours.
/// The every-cut verdict snapshots this one, so the holder it stands for is
/// the same terminal the cross-engine verdict reads.
///
/// # Errors
///
/// The library refused to create or set up the terminal.
pub fn terminal(columns: u16, rows: u16) -> Result<Vt, Refused> {
    let config = Config {
        sixel: false,
        ..Config::for_version("differential")
    };
    let mut vt = Vt::new(columns, rows, &config)?;
    vt.resize(columns, rows, CELL_PX, CELL_PX)?;
    vt.set_colors(&crate::reference_colours().for_ghostty())?;
    // The resize and the colours answer nothing the differential asked.
    let _ = vt.take_effects();
    Ok(vt)
}

/// The Ghostty engine behind the differential's [`Engine`].
pub struct Ghostty {
    vt: Vt,
    /// What the terminal owes the PTY, gathered feed by feed until
    /// [`Engine::replies`] takes it.
    replies: Vec<u8>,
}

impl Ghostty {
    /// A fresh terminal ([`terminal`]).
    ///
    /// # Errors
    ///
    /// The library refused to create or set up the terminal.
    pub fn fresh(columns: u16, rows: u16) -> Result<Self, Refused> {
        Ok(Self {
            vt: terminal(columns, rows)?,
            replies: Vec::new(),
        })
    }
}

/// A cell of the product engine in the canonical grid's terms: the style bits
/// both engines keep (any underline is an underline; blink and overline
/// alacritty has not), the colours as the program named them, and a spacer
/// whichever side of a wide character it is on.
fn canonical(cell: Written) -> CellBox {
    let s = cell.style;
    CellBox {
        text: cell.text,
        style: Style {
            bold: s.bold,
            italic: s.italic,
            faint: s.faint,
            inverse: s.inverse,
            invisible: s.invisible,
            underline: s.underline != Underline::None,
            strikethrough: s.strikethrough,
            fg: colour(cell.fg),
            bg: colour(cell.bg),
        },
        width: match cell.width {
            CellWidth::Narrow => Width::Narrow,
            CellWidth::Wide => Width::Wide,
            CellWidth::SpacerTail | CellWidth::SpacerHead => Width::Spacer,
        },
    }
}

/// A colour as the canonical grid spells it.
const fn colour(c: AskedColour) -> Colour {
    match c {
        AskedColour::Default => Colour::Default,
        AskedColour::Palette(i) => Colour::Palette(i),
        AskedColour::Rgb(rgb) => Colour::Rgb(rgb.r, rgb.g, rgb.b),
    }
}

impl Engine for Ghostty {
    fn name(&self) -> &'static str {
        "ghostty"
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.vt.feed(bytes);
        // The answers are kept and the other effects dropped: the bell, the
        // title, a notification are the holder's business, and unread they
        // would only pile up in a terminal that may run a whole corpus.
        let effects = self.vt.take_effects();
        self.replies.extend_from_slice(&effects.to_pty);
    }

    fn resize(&mut self, columns: u16, lines: u16) {
        // A refusal keeps the old size, and the grid's size is the first
        // thing the comparison checks: it is reported there, by name, rather
        // than in a log nobody reads in a corpus run.
        let _ = self.vt.resize(columns, lines, CELL_PX, CELL_PX);
    }

    fn grid(&self) -> Grid {
        let (columns, rows) = self.vt.size();
        let mut grid = Grid::fresh_one(columns, rows);
        for row in 0..rows {
            for column in 0..columns {
                if let Some(cell) = self.vt.written(column, row) {
                    grid.put_at(row, column, canonical(cell));
                }
            }
        }
        let cursor = self.vt.cursor();
        grid.cursor = (cursor.y, cursor.x);
        grid
    }

    fn replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Reply, comparable_replies, reference_colours};

    fn fed(bytes: &[u8]) -> Ghostty {
        let mut engine = Ghostty::fresh(20, 4).expect("a terminal");
        engine.feed(bytes);
        engine
    }

    #[test]
    fn it_writes_two_letters_and_reads_them_back() {
        // FAILS IF the product engine's cells do not reach the canonical
        // grid where the program wrote them, or the cursor's row and column
        // are crossed on the way: every case would then diverge at once.
        let g = fed(b"hi").grid();
        assert_eq!((g.cell(0, 0).first(), g.cell(0, 1).first()), ('h', 'i'));
        assert_eq!(g.cursor, (0, 2));
        assert!(g.canonical_f().starts_with("hi\n"), "{}", g.canonical_f());
    }

    #[test]
    fn styles_reach_the_grid_in_the_terms_both_engines_keep() {
        // FAILS IF a style bit is lost on the way, a curly underline is not
        // an underline, a colour named by its palette entry arrives resolved,
        // or the two halves of a wide character are not "wide" and "spacer" —
        // the cross-engine style comparison would compare nothing, or flag
        // what alacritty cannot keep.
        let g = fed("\x1b[1;4:3;31;48;2;1;2;3mA\x1b[0;5;53m\u{65e5}".as_bytes()).grid();
        let a = g.cell(0, 0).style;
        assert!(a.bold && a.underline);
        assert_eq!((a.fg, a.bg), (Colour::Palette(1), Colour::Rgb(1, 2, 3)));
        let wide = g.cell(0, 1);
        assert_eq!(wide.width, Width::Wide);
        assert_eq!(
            wide.style,
            Style::default(),
            "blink and overline are not compared"
        );
        assert_eq!(g.cell(0, 2).width, Width::Spacer);
    }

    #[test]
    fn replies_are_kept_in_order_and_taken_once() {
        // FAILS IF an answer is dropped between feeds, the effects that are
        // no answer (the title here) reach the replies, or a reply taken
        // once is given again.
        let mut engine = fed(b"ab\x1b[6n");
        engine.feed(b"\x1b]2;a title\x07\x1b[5n");
        assert_eq!(engine.replies(), b"\x1b[1;3R\x1b[0n");
        assert!(
            engine.replies().is_empty(),
            "replies are taken, not repeated"
        );
    }

    #[test]
    fn colour_queries_are_answered_from_the_reference_table() {
        // FAILS IF the Ghostty engine is handed colours other than the
        // table's — alacritty answers from the table, and every colour query
        // of the corpus would diverge — or a slot is crossed on the way in.
        let mut engine = fed(b"\x1b]10;?\x07\x1b]11;?\x07\x1b]12;?\x07\x1b]4;196;?\x07");
        let table = reference_colours();
        let colour = |slot: &str, rgb| Reply::Colour {
            slot: slot.to_owned(),
            rgb,
        };
        assert_eq!(
            comparable_replies(&engine.replies()),
            [
                colour("10", table.foreground),
                colour("11", table.background),
                colour("12", table.cursor),
                colour("4;196", table.palette[196]),
            ]
        );
    }

    #[test]
    fn a_resize_reaches_the_grid() {
        // FAILS IF the engine's resize is not the terminal's: the grid would
        // keep its old size and every wrap after a resize would be compared
        // against a terminal of another width.
        let mut engine = fed(b"x");
        engine.resize(8, 2);
        let g = engine.grid();
        assert_eq!((g.columns, g.lines), (8, 2));
    }
}
