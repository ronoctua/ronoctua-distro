//! Words as a source of the scene — a title, a name under a face, a line
//! saying "back in five minutes".
//!
//! # What is drawn, and with what
//!
//! The words are CONTENT, as a picture or a flat colour is: what a person
//! typed into their stream, in the colour they picked (`RRGGBBAA`, never a
//! token — the shape [`crate::scene::Source::Colour`] already has). The face
//! is the distro's own text face, the file the renderer embeds
//! (`RONOCTUA_FONT_REGULAR`, a store path baked at build), rasterised here by
//! `cosmic-text` and `swash` — the same shaping the screen uses, so a word
//! with an accent or a ligature looks in the program as it looks on screen.
//!
//! # The size is the layer's
//!
//! The picture is drawn at a line height of an eighth of the program's
//! height — 90 pixels at 720p — on a transparent ground exactly as wide as
//! the words, and the layer's place scales it where it goes (`Fit::Fit` keeps
//! its shape). A longer text is a wider picture, never a smaller line.

use ronoctua_effects::Frame;

/// The text face, by store path, baked at build.
const FACE: Option<&str> = option_env!("RONOCTUA_FONT_REGULAR");

/// How many characters a text source draws: a title, not a document — and a
/// frame as wide as a novel would be a picture nobody can place.
pub const MOST: usize = 200;

/// The colour of a text, `RRGGBB` or `RRGGBBAA` with or without `#` — what
/// the person picked.
#[must_use]
pub fn rgba_of(hex: &str) -> Option<[u8; 4]> {
    let hex = hex.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some([
        byte(0)?,
        byte(2)?,
        byte(4)?,
        if hex.len() >= 8 { byte(6)? } else { 255 },
    ])
}

/// The line height the words are drawn at, for a program `height` tall.
#[must_use]
pub const fn line_height(height: u32) -> u32 {
    let h = height / 8;
    if h < 8 { 8 } else { h }
}

/// Whether the build named a face, and it can be read.
#[must_use]
pub fn has_a_face() -> bool {
    FACE.is_some_and(|f| std::path::Path::new(f).is_file())
}

/// The words, drawn: a transparent frame as wide as they are, a line per
/// line of text — and blank words a single transparent pixel, which places
/// nothing. `None` with no face to draw with.
#[must_use]
pub fn drawn(text: &str, rgba: [u8; 4], program_height: u32) -> Option<Frame> {
    let face = std::fs::read(FACE?).ok()?;
    drawn_with(&face, text, rgba, program_height)
}

/// [`drawn`], with the face's bytes given — what the tests hand it.
#[must_use]
pub fn drawn_with(face: &[u8], text: &str, rgba: [u8; 4], program_height: u32) -> Option<Frame> {
    use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache};
    let text: String = text.chars().take(MOST).collect();
    if text.trim().is_empty() {
        return Some(Frame::filled(1, 1, [0, 0, 0, 0]));
    }
    let mut db = cosmic_text::fontdb::Database::new();
    db.load_font_data(face.to_vec());
    let mut system = FontSystem::new_with_locale_and_db("en-GB".to_owned(), db);
    let line = line_height(program_height) as f32;
    let mut buffer = Buffer::new(&mut system, Metrics::new(line * 0.8, line));
    buffer.set_text(&text, &Attrs::new(), Shaping::Advanced, None);
    buffer.shape_until_scroll(&mut system, false);
    let (mut wide, mut lines) = (0.0f32, 0usize);
    for run in buffer.layout_runs() {
        wide = wide.max(run.line_w);
        lines += 1;
    }
    // AIR ON BOTH SIDES of the ink: a glyph's overhang is not measured by the
    // advance, and an italic or a `j` would be cut at the edge.
    let pad = (line / 8.0).ceil();
    let width = (wide + 2.0 * pad).ceil().max(1.0) as usize;
    let height = ((lines.max(1) as f32) * line).ceil() as usize;
    let mut frame = Frame {
        width,
        height,
        rgba: vec![0u8; width * height * 4],
    };
    let mut cache = SwashCache::new();
    let colour = Color::rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
    buffer.draw(&mut system, &mut cache, colour, |x, y, w, h, c| {
        let alpha = u32::from(c.a());
        if alpha == 0 {
            return;
        }
        for dy in 0..h {
            for dx in 0..w {
                let px = x + pad as i32 + dx as i32;
                let py = y + dy as i32;
                let (Ok(px), Ok(py)) = (usize::try_from(px), usize::try_from(py)) else {
                    continue;
                };
                if px >= width || py >= height {
                    continue;
                }
                let at = (py * width + px) * 4;
                // OVER, with the colour's own alpha: glyph coverage arrives
                // as the alpha of `c`, and two passes over one pixel (a
                // ligature's parts) add up instead of the second winning.
                let under = u32::from(frame.rgba[at + 3]);
                let out = alpha + under * (255 - alpha) / 255;
                if out == 0 {
                    continue;
                }
                for (i, channel) in [c.r(), c.g(), c.b()].into_iter().enumerate() {
                    let top = u32::from(channel) * alpha;
                    let bottom = u32::from(frame.rgba[at + i]) * under * (255 - alpha) / 255;
                    frame.rgba[at + i] = ((top + bottom) / out).min(255) as u8;
                }
                frame.rgba[at + 3] = out.min(255) as u8;
            }
        }
    });
    Some(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_reads_with_and_without_alpha() {
        assert_eq!(rgba_of("ff000080"), Some([255, 0, 0, 128]));
        assert_eq!(rgba_of("#00ff00"), Some([0, 255, 0, 255]));
        assert_eq!(rgba_of("zz"), None);
    }

    #[test]
    fn the_line_is_an_eighth_of_the_program_and_never_nothing() {
        assert_eq!(line_height(720), 90);
        assert_eq!(line_height(1080), 135);
        assert_eq!(line_height(10), 8);
    }

    /// The real face, when the build named one: words come out as ink of the
    /// picked colour on a transparent ground, wider for more words.
    #[test]
    fn words_are_ink_on_nothing_and_grow_with_the_text() {
        let Some(path) = FACE else {
            // A build with no face has no text source at all, and says so
            // on screen; there is nothing here to draw with.
            assert!(drawn("x", [255; 4], 720).is_none());
            return;
        };
        let face = std::fs::read(path).expect("the face the build named");
        let short = drawn_with(&face, "Hi", [255, 0, 0, 255], 720).expect("drawn");
        let long = drawn_with(&face, "Hi there, everybody", [255, 0, 0, 255], 720).expect("drawn");
        assert!(long.width > short.width);
        assert_eq!(short.height, 90);
        let inked = short.rgba.chunks(4).filter(|p| p[3] > 0).count();
        assert!(inked > 20, "no ink: {inked}");
        assert!(
            short.rgba.chunks(4).any(|p| p[3] == 0),
            "no transparent ground"
        );
        assert!(
            short
                .rgba
                .chunks(4)
                .filter(|p| p[3] == 255)
                .all(|p| p[0] > 200 && p[1] < 60 && p[2] < 60)
        );
        // AND NOTHING TO DRAW places nothing: one transparent pixel.
        let blank = drawn_with(&face, "   ", [255; 4], 720).expect("a frame");
        assert_eq!((blank.width, blank.height, blank.rgba[3]), (1, 1, 0));
    }
}
