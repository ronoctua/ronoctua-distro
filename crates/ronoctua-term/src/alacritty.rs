//! The `alacritty_terminal` engine — the implementation milestone 7 closes with.
//!
//! Section 7.10's `libghostty-vt` comes in later, behind the same
//! [`crate::Engine`]. See risk R1: the trait is what makes the order
//! irrelevant.

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::{Config, Term, test::TermSize};
use alacritty_terminal::vte::ansi::Processor;

use crate::{CellBox, Engine, Grid};

/// alacritty's `Term` requires an event listener. Ours discards everything:
/// events are things like "the title changed" and "the bell rang", and the engine
/// exists here only to interpret the grid.
struct Deaf;

impl EventListener for Deaf {
    fn send_event(&self, _: Event) {}
}

/// An engine based on `alacritty_terminal`.
pub struct Alacritty {
    term: Term<Deaf>,
    parser: Processor,
}

impl Alacritty {
    /// A terminal of the given size.
    #[must_use]
    pub fn fresh(columns: u16, lines: u16) -> Self {
        let size = TermSize::new(usize::from(columns), usize::from(lines));
        Self {
            term: Term::new(Config::default(), &size, Deaf),
            parser: Processor::new(),
        }
    }
}

impl Engine for Alacritty {
    fn name(&self) -> &'static str {
        "alacritty"
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.term, bytes);
    }

    fn grid(&self) -> Grid {
        let g = self.term.grid();
        let columns = u16::try_from(g.columns()).unwrap_or(u16::MAX);
        let lines = u16::try_from(g.screen_lines()).unwrap_or(u16::MAX);
        let mut out = Grid::fresh_one(columns, lines);

        for l in 0..lines {
            for c in 0..columns {
                let p = Point::new(Line(i32::from(l)), Column(usize::from(c)));
                out.put_at(l, c, CellBox { c: g[p].c });
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
}

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
}
