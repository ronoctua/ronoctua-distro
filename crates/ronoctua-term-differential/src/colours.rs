//! The colours both engines are handed — the one table of colours written by
//! hand in the terminal's code, and the reason it lives in this crate.

use ronoctua_term::ghostty::{Colors, Rgb, Scheme};

/// An 8-bit colour as the reference table spells it.
pub type Rgb8 = (u8, u8, u8);

/// The colours the differential gives both engines.
///
/// The xterm table: sixteen named colours, the 6x6x6 cube and the 24 greys.
/// What matters is that both engines are handed the SAME colours, so an OSC
/// query answers the same number in both; it is data for a test, never a
/// colour anything draws (the window's colours are roles, resolved per theme).
///
/// HAND-WRITTEN COLOURS, and that is why only the differential has them: a
/// product that linked a colour table would have one to reach for instead of
/// the page's roles (invariant 9). This crate's program is a driver, and
/// nothing a machine runs links it — not even under a feature, as when the
/// table sat in `ronoctua-term`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceColours {
    /// The 256-colour palette.
    pub palette: [Rgb8; 256],
    /// The default foreground.
    pub foreground: Rgb8,
    /// The default background.
    pub background: Rgb8,
    /// The cursor.
    pub cursor: Rgb8,
}

/// The reference colours ([`ReferenceColours`]).
#[must_use]
pub fn reference_colours() -> ReferenceColours {
    const NAMED: [Rgb8; 16] = [
        (0x00, 0x00, 0x00),
        (0xcd, 0x00, 0x00),
        (0x00, 0xcd, 0x00),
        (0xcd, 0xcd, 0x00),
        (0x00, 0x00, 0xee),
        (0xcd, 0x00, 0xcd),
        (0x00, 0xcd, 0xcd),
        (0xe5, 0xe5, 0xe5),
        (0x7f, 0x7f, 0x7f),
        (0xff, 0x00, 0x00),
        (0x00, 0xff, 0x00),
        (0xff, 0xff, 0x00),
        (0x5c, 0x5c, 0xff),
        (0xff, 0x00, 0xff),
        (0x00, 0xff, 0xff),
        (0xff, 0xff, 0xff),
    ];
    let mut palette = [(0, 0, 0); 256];
    palette[..16].copy_from_slice(&NAMED);
    let level = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
    for i in 0..216u8 {
        palette[16 + usize::from(i)] = (level(i / 36), level((i / 6) % 6), level(i % 6));
    }
    for i in 0..24u8 {
        let grey = 8 + i * 10;
        palette[232 + usize::from(i)] = (grey, grey, grey);
    }
    ReferenceColours {
        palette,
        foreground: (0xe5, 0xe5, 0xe5),
        background: (0x10, 0x10, 0x10),
        cursor: (0xff, 0xff, 0xff),
    }
}

impl ReferenceColours {
    /// The same colours as the Ghostty engine takes them, dark: the scheme a
    /// program asks with `CSI ? 996 n`, answered alike in every case.
    #[must_use]
    pub fn for_ghostty(&self) -> Colors {
        let rgb = |(r, g, b): Rgb8| Rgb { r, g, b };
        Colors {
            foreground: rgb(self.foreground),
            background: rgb(self.background),
            cursor: Some(rgb(self.cursor)),
            palette: Box::new(self.palette.map(rgb)),
            scheme: Scheme::Dark,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_palette_is_the_xterm_table() {
        // FAILS IF the cube or the greys are computed off by one: the colour
        // answers compared across engines would still agree (both get the same
        // table) while the table is not the one its comment promises.
        let palette = reference_colours().palette;
        assert_eq!(palette[16], (0, 0, 0));
        assert_eq!(palette[21], (0, 0, 0xff));
        assert_eq!(palette[196], (0xff, 0, 0));
        assert_eq!(palette[231], (0xff, 0xff, 0xff));
        assert_eq!(palette[232], (8, 8, 8));
        assert_eq!(palette[255], (0xee, 0xee, 0xee));
    }
}
