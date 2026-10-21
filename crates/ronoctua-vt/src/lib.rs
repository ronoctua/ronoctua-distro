//! The kernel's console (`TERM=linux`), where the installer (the ISO's) and
//! the rescue console (VT 8) run.
//!
//! # Why a terminal UI cannot hand it 24-bit colours
//!
//! The console takes a 24-bit colour and keeps about one bit of each channel:
//! every grey of a palette comes out white, and the dark tones a band is made
//! of — the cursor's, the pointer's — come out black, the ground's own colour.
//! On the ISO no screen said which option the cursor was on.
//!
//! What it does keep is sixteen slots whose colours a program may write
//! (`ESC ] P`), the first eight of them a background can wear: with a font of
//! more than 256 glyphs (the distro's has 512) the ninth bit of a glyph takes
//! a ground's fourth. So on the console a terminal UI writes the tokens it
//! paints into the slots ([`Table`], [`Console::palette`]), paints every frame
//! by slot ([`Console::paint`]), and gives the console its own palette back
//! when it leaves ([`Console::RESET`]) — the console then shows the palette's
//! own colours, exactly.

use ratatui::buffer::Buffer;
use ratatui::style::Modifier;
use ronoctua_tokens::Color;

/// How many slots the console has.
pub const SLOTS: usize = 16;

/// How many of them a background can be.
pub const GROUNDS: usize = 8;

/// The roles a terminal UI paints, each in the console's slot it takes: the
/// first [`GROUNDS`] are the ones a cell's ground may wear, and every role a
/// ground wears has to be among them.
///
/// Slots 0 and 7 are the console's own ground and text, so a table that puts
/// its ground and its text there leaves what the console prints after it in
/// the palette's tones too.
#[derive(Debug, Clone, Copy)]
pub struct Table(&'static [Color]);

impl Table {
    /// The table of `roles`, in slot order.
    ///
    /// # Panics
    ///
    /// With more roles than the console has slots — in a `const`, which is
    /// where a table is made, that is a build that fails.
    #[must_use]
    pub const fn new(roles: &'static [Color]) -> Self {
        assert!(roles.len() <= SLOTS, "the console has sixteen slots");
        Self(roles)
    }
}

/// Is the terminal the kernel's console? Its getty and systemd name it
/// `linux`, and no other terminal does.
#[must_use]
pub fn here() -> bool {
    std::env::var_os("TERM").is_some_and(|t| t == "linux")
}

/// The console's slots, resolved in the palette loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Console {
    /// Each slot's colour.
    slots: Vec<[u8; 3]>,
}

impl Console {
    /// The sequence that gives the console its own palette back.
    pub const RESET: &str = "\x1b]R";

    /// The console, when the terminal is the kernel's ([`here`]).
    #[must_use]
    pub fn of_term(table: Table) -> Option<Self> {
        here().then(|| Self::resolved(table))
    }

    /// The slots of `table`, in the palette loaded now.
    #[must_use]
    pub fn resolved(table: Table) -> Self {
        Self {
            slots: table
                .0
                .iter()
                .map(|c| {
                    let [r, g, b, _] = c.to_rgba8();
                    [r, g, b]
                })
                .collect(),
        }
    }

    /// The sequence that writes every slot: `ESC ] P`, the slot's hex digit
    /// and its colour's six — no terminator, the console counts them.
    #[must_use]
    pub fn palette(&self) -> String {
        self.slots
            .iter()
            .enumerate()
            .map(|(i, [r, g, b])| format!("\x1b]P{i:x}{r:02x}{g:02x}{b:02x}"))
            .collect()
    }

    /// The buffer painted by slot. AND NO BOLD: on the console bold is not a
    /// weight but the bright half of the slots — it turned one slot into
    /// another, and the token's colour is already the one its slot holds.
    pub fn paint(&self, buffer: &mut Buffer) {
        for cell in &mut buffer.content {
            cell.fg = self.slot(cell.fg, self.slots.len());
            cell.bg = self.slot(cell.bg, GROUNDS);
            cell.modifier.remove(Modifier::BOLD);
        }
    }

    /// What `buffer` paints that no slot holds exactly — a text colour off
    /// the table, or a ground outside its first eight —, one line each. The
    /// console would draw it in another token's colour; a terminal UI's
    /// tests ask for this to be empty on every screen.
    #[must_use]
    pub fn unslotted(&self, buffer: &Buffer) -> Vec<String> {
        let mut found = Vec::new();
        for (i, cell) in buffer.content.iter().enumerate() {
            for (c, of, what) in [
                (cell.fg, self.slots.len(), "text"),
                (cell.bg, GROUNDS, "ground"),
            ] {
                let held = match c {
                    ratatui::style::Color::Rgb(r, g, b) => {
                        self.slots.iter().take(of).any(|s| *s == [r, g, b])
                    }
                    _ => false,
                };
                if !held {
                    found.push(format!(
                        "cell {i} (`{}`): its {what} is {c:?}, which no slot holds",
                        cell.symbol()
                    ));
                }
            }
        }
        found
    }

    /// The slot among the first `of` that holds `c` — or, for a colour the
    /// table does not have, the nearest one.
    fn slot(&self, c: ratatui::style::Color, of: usize) -> ratatui::style::Color {
        let ratatui::style::Color::Rgb(r, g, b) = c else {
            return c;
        };
        let distance = |s: &[u8; 3]| -> u32 {
            [r, g, b]
                .iter()
                .zip(s)
                .map(|(a, b)| u32::from(a.abs_diff(*b)).pow(2))
                .sum()
        };
        self.slots
            .iter()
            .take(of)
            .enumerate()
            .min_by_key(|(_, s)| distance(s))
            .and_then(|(i, _)| u8::try_from(i).ok())
            .map_or(c, ratatui::style::Color::Indexed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;
    use ronoctua_tokens::color;

    /// Eight grounds and one more: the ninth slot is text only.
    const TABLE: Table = Table::new(&[
        color::SURFACE_BASE,
        color::SURFACE_SELECTION,
        color::SURFACE_HOVER,
        color::TEXT_PRIMARY,
        color::STATE_ERROR,
        color::STATE_SUCCESS,
        color::STATE_WARNING,
        color::STATE_ACCENT,
        color::TEXT_DIM,
    ]);

    fn rgb(c: Color) -> ratatui::style::Color {
        let [r, g, b, _] = c.to_rgba8();
        ratatui::style::Color::Rgb(r, g, b)
    }

    /// FAILS IF: a token is painted in another slot than its own, a ground
    /// is painted past the eight the console has, or bold survives — on the
    /// console it turns one slot into the one eight above it.
    #[test]
    fn a_frame_is_painted_by_slot() {
        let console = Console::resolved(TABLE);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 3, 1));
        let cells = &mut buffer.content;
        cells[0].fg = rgb(color::TEXT_PRIMARY);
        cells[0].bg = rgb(color::SURFACE_SELECTION);
        cells[0].modifier = Modifier::BOLD;
        cells[1].fg = rgb(color::TEXT_DIM);
        cells[1].bg = rgb(color::SURFACE_BASE);
        // A ground in the ninth slot, which no ground can wear: the nearest
        // of the eight.
        cells[2].fg = rgb(color::SURFACE_BASE);
        cells[2].bg = rgb(color::TEXT_DIM);
        assert_eq!(
            console.unslotted(&buffer).len(),
            1,
            "{:?}",
            console.unslotted(&buffer)
        );
        console.paint(&mut buffer);
        let cells = &buffer.content;
        use ratatui::style::Color::Indexed;
        assert_eq!((cells[0].fg, cells[0].bg), (Indexed(3), Indexed(1)));
        assert!(!cells[0].modifier.contains(Modifier::BOLD));
        assert_eq!((cells[1].fg, cells[1].bg), (Indexed(8), Indexed(0)));
        assert_eq!(cells[2].fg, Indexed(0));
        assert!(matches!(cells[2].bg, Indexed(n) if usize::from(n) < GROUNDS));
    }

    /// FAILS IF: the palette is not in the console's own grammar — `ESC ] P`,
    /// one hex digit, six — or skips a slot.
    #[test]
    fn the_palette_writes_every_slot() {
        let palette = Console::resolved(TABLE).palette();
        let writes: Vec<&str> = palette.split("\x1b]P").skip(1).collect();
        assert_eq!(writes.len(), 9, "{palette:?}");
        for (i, w) in writes.iter().enumerate() {
            assert_eq!(w.len(), 7, "{w:?}");
            assert!(w.chars().all(|c| c.is_ascii_hexdigit()), "{w:?}");
            assert_eq!(usize::from_str_radix(&w[..1], 16).unwrap(), i);
        }
        let [r, g, b, _] = color::TEXT_PRIMARY.to_rgba8();
        assert_eq!(writes[3], format!("3{r:02x}{g:02x}{b:02x}"));
    }
}
