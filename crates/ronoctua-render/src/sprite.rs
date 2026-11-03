//! GLYPHS THAT ARE DRAWN, NOT READ: box drawing, blocks, braille, the legacy
//! computing mosaics, powerline's separators, and every line drawn under,
//! through and over a cell.
//!
//! # Why a font's glyph is not good enough
//!
//! Because a font draws its box-drawing lines in ITS box, and a grid of cells
//! draws in its own. The face's box is 1.25 em tall; a cell is the face's
//! ascent, descent and gap rounded to a whole pixel, and the two only agree by
//! accident. A `│` from the font stops short of the cell's edges, and a column
//! of them is a dashed line nobody asked for. Drawn here, at the cell's exact
//! size in physical pixels, a line touches both edges at every size and every
//! scale, and the next cell's line continues it without a seam.
//!
//! # Coverage, and nothing else
//!
//! A sprite is one byte per pixel: how much of that pixel the shape covers.
//! The colour is the cell's, applied where it is drawn (`crate::cells`), so
//! one raster serves every colour a program asks for — which is also why the
//! atlas key of a sprite carries no palette.
//!
//! # No rasteriser crate
//!
//! The renderer depends on none, and a sprite does not need one. Straight
//! strokes are whole pixels — a rectangle has no edge to smooth, and a
//! smoothed one would blur the joint with the next cell. Everything slanted
//! or curved is SAMPLED: each pixel asks [`SAMPLES`]² points inside it whether
//! they are inside the shape, which is the whole of what an anti-aliased fill
//! is, written in thirty lines that a test can hold to the cell's edges.
//!
//! # What is covered
//!
//! All of Box Drawing (U+2500–257F), Block Elements (U+2580–259F) and Braille
//! (U+2800–28FF); powerline's U+E0B0–E0BF; and of Symbols for Legacy Computing
//! (U+1FB00–1FBFF) the sextants, the smooth mosaics, the triangular blocks,
//! the eighth blocks, the shaded halves, the checkerboards and the diagonal
//! box lines — U+1FB00–1FB97, U+1FB9A–1FB9F and U+1FBA0–1FBAF, but U+1FB93,
//! which Unicode left unassigned. The hatched fills (U+1FB98–1FB99), the
//! arrows, the pointer, the segmented digits and the rest past U+1FBAF are the
//! font's: they are pictures, not geometry that has to meet a neighbour.

use crate::cells::Underline;

/// How many points per axis a sampled pixel asks — sixteen in all, which is
/// sixteen shades of an edge. More would cost a sprite's first draw and buy an
/// edge nobody can tell apart at a cell's size.
const SAMPLES: u32 = 4;

/// LIGHT SHADE (`░`): a quarter of the ink, by the characters' own order —
/// light, medium and dark, between none and the full block.
const LIGHT_SHADE: u8 = 0x40;

/// MEDIUM SHADE (`▒`): half of it.
const MEDIUM_SHADE: u8 = 0x80;

/// DARK SHADE (`▓`): three quarters.
const DARK_SHADE: u8 = 0xC0;

/// All of the pixel.
const FULL: u8 = 0xFF;

/// One sprite's raster: `width × height` bytes, row after row, each how much
/// of its pixel the shape covers — 0 none of it, 255 all of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width × height` bytes.
    pub data: Vec<u8>,
}

/// A stroke across a cell, in physical pixels: where its top is, counted down
/// from the cell's top, and how thick it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Stroke {
    /// The stroke's top, from the cell's top. Negative is above the cell.
    pub top: i32,
    /// Its thickness, at least one pixel.
    pub thickness: u32,
}

/// The box a sprite is drawn into, and the font's own strokes it is drawn
/// with — every number in physical pixels, and every one of them in the atlas
/// key, because each changes the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frame {
    /// ONE cell's width — the period of everything that repeats along a line,
    /// so a curl or a dash under a wide cell meets the one under its
    /// neighbour.
    pub cell: u32,
    /// The width drawn into: one cell, or two for a wide one.
    pub width: u32,
    /// The cell's height.
    pub height: u32,
    /// A light line's thickness — the font's underline, which is what a
    /// box-drawing line is drawn as beside the text it frames.
    pub stroke: u32,
    /// Where the underline is.
    pub underline: Stroke,
    /// Where the strike-through is.
    pub strike: Stroke,
    /// Where the overline is.
    pub overline: Stroke,
}

/// A line drawn under, through or over a cell's glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Line {
    /// Under it, in one of its kinds.
    Under(Underline),
    /// Through it.
    Strike,
    /// Over it.
    Over,
}

impl Coverage {
    /// A raster with nothing on it.
    fn blank(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width as usize) * (height as usize)],
        }
    }

    /// Whether nothing is drawn on it.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.data.iter().all(|&v| v == 0)
    }

    /// How much of pixel `(x, y)` is covered — for a test, and for whoever
    /// reads a raster back.
    #[must_use]
    pub fn at(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.data[(y * self.width + x) as usize]
    }

    /// Covers the WHOLE pixels of `[x0, x1) × [y0, y1)` by `shade`, clipped to
    /// the raster. What is already covered more stays so.
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, shade: u8) {
        let (w, h) = (to_i32(self.width), to_i32(self.height));
        let (x0, x1) = (x0.clamp(0, w), x1.clamp(0, w));
        let (y0, y1) = (y0.clamp(0, h), y1.clamp(0, h));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * w + x) as usize;
                self.data[i] = self.data[i].max(shade);
            }
        }
    }

    /// Covers what `inside` says, each pixel asked at [`SAMPLES`]² points, by
    /// `shade` at most — the fill of everything slanted or curved.
    fn sample(&mut self, shade: u8, inside: impl Fn(f32, f32) -> bool) {
        let step = 1.0 / SAMPLES as f32;
        let all = SAMPLES * SAMPLES;
        for y in 0..self.height {
            for x in 0..self.width {
                let mut hits = 0;
                for j in 0..SAMPLES {
                    for i in 0..SAMPLES {
                        let px = (i as f32 + 0.5).mul_add(step, x as f32);
                        let py = (j as f32 + 0.5).mul_add(step, y as f32);
                        if inside(px, py) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    let v = u8::try_from(u32::from(shade) * hits / all).unwrap_or(FULL);
                    let at = (y * self.width + x) as usize;
                    self.data[at] = self.data[at].max(v);
                }
            }
        }
    }
}

/// A count of pixels as a coordinate. A cell is never two billion pixels
/// wide, and saturating says so rather than wrapping if one ever were.
fn to_i32(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

/// A fraction of a length, to the nearest whole pixel — the one rounding
/// every block uses, so the lower half and the upper half of a cell meet on
/// the same row and never overlap or part.
fn part(length: u32, of: u32, parts: u32) -> i32 {
    to_i32(((length * of) as f32 / parts as f32).round() as u32)
}

/// Whether the codepoint is one this module draws, so the font is not asked.
///
/// THE SAME ANSWER AS [`glyph`], range for range: a test holds the two to
/// each other, because a codepoint said covered and then drawn as nothing is a
/// blank cell where the font had a glyph.
#[must_use]
pub fn covers(c: char) -> bool {
    matches!(
        u32::from(c),
        0x2500..=0x259F
            | 0x2800..=0x28FF
            | 0xE0B0..=0xE0BF
            | 0x1FB00..=0x1FB92
            | 0x1FB94..=0x1FB97
            | 0x1FB9A..=0x1FBAF
    )
}

/// The raster of `c` in `frame`, or `None` when it is not one this module
/// draws — and then the font draws it.
#[must_use]
pub fn glyph(c: char, frame: &Frame) -> Option<Coverage> {
    if !covers(c) || frame.width == 0 || frame.height == 0 {
        return None;
    }
    let mut out = Coverage::blank(frame.width, frame.height);
    let cp = u32::from(c);
    match cp {
        0x2500..=0x257F => box_drawing(&mut out, frame, cp),
        0x2580..=0x259F => block(&mut out, cp),
        0x2800..=0x28FF => braille(&mut out, cp),
        0xE0B0..=0xE0BF => powerline(&mut out, frame, cp),
        _ => legacy(&mut out, frame, cp),
    }
    Some(out)
}

/// The raster of a line under, through or over a cell, the size of `frame`.
/// Nothing for [`Underline::None`].
#[must_use]
pub fn line(line: Line, frame: &Frame) -> Coverage {
    let mut out = Coverage::blank(frame.width, frame.height);
    match line {
        Line::Under(Underline::None) => {}
        Line::Under(Underline::Single) => across(&mut out, frame.underline),
        Line::Under(Underline::Double) => double(&mut out, frame.underline),
        Line::Under(Underline::Curly) => curly(&mut out, frame),
        Line::Under(Underline::Dotted) => dotted(&mut out, frame),
        Line::Under(Underline::Dashed) => dashed(&mut out, frame),
        Line::Strike => across(&mut out, frame.strike),
        Line::Over => across(&mut out, frame.overline),
    }
    out
}

/// A stroke the whole width, kept inside the cell: a font whose underline
/// sits below its own descent would otherwise draw one nobody sees.
fn across(out: &mut Coverage, s: Stroke) {
    let t = to_i32(s.thickness.max(1));
    let top = s.top.clamp(0, (to_i32(out.height) - t).max(0));
    out.rect(0, top, to_i32(out.width), top + t, FULL);
}

/// Two strokes, a stroke apart — moved up together when the second would
/// leave the cell.
fn double(out: &mut Coverage, s: Stroke) {
    let t = to_i32(s.thickness.max(1));
    let top = s.top.min(to_i32(out.height) - 3 * t).max(0);
    let w = to_i32(out.width);
    out.rect(0, top, w, top + t, FULL);
    out.rect(0, top + 2 * t, w, top + 3 * t, FULL);
}

/// One wave per cell, low at the cell's edges and high in its middle, so the
/// curl under one cell carries on under the next.
///
/// The height of the wave is a cell's width over π — the curve whose
/// steepest slope is one, the steepest a stroke of the underline's thickness
/// keeps that thickness on.
fn curly(out: &mut Coverage, f: &Frame) {
    let t = f.underline.thickness.max(1) as f32;
    let period = f.cell.max(1) as f32;
    let amplitude = period / std::f32::consts::PI;
    let room = (out.height as f32 - amplitude - t).max(0.0);
    let top = (f.underline.top as f32).min(room).max(0.0);
    let tau = std::f32::consts::TAU;
    out.sample(FULL, |x, y| {
        let phase = tau * x / period;
        let middle = top + t / 2.0 + amplitude / 2.0 * (1.0 + phase.cos());
        let slope = amplitude / 2.0 * tau / period * phase.sin();
        (y - middle).abs() <= t / 2.0 * slope.mul_add(slope, 1.0).sqrt()
    });
}

/// Square dots a stroke wide, evenly spaced inside each cell.
fn dotted(out: &mut Coverage, f: &Frame) {
    let t = f.underline.thickness.max(1);
    let cell = f.cell.max(1);
    let per_cell = (cell / (2 * t)).max(1);
    let spacing = cell as f32 / per_cell as f32;
    let top = f
        .underline
        .top
        .clamp(0, (to_i32(out.height) - to_i32(t)).max(0));
    let cells = out.width.div_ceil(cell);
    for k in 0..cells * per_cell {
        let middle = (k as f32 + 0.5) * spacing;
        let left = (middle - t as f32 / 2.0).round() as i32;
        out.rect(left, top, left + to_i32(t), top + to_i32(t), FULL);
    }
}

/// A dash over the middle half of each cell, so a run of cells is dash and
/// gap in equal measure.
fn dashed(out: &mut Coverage, f: &Frame) {
    let t = to_i32(f.underline.thickness.max(1));
    let cell = f.cell.max(1);
    let top = f.underline.top.clamp(0, (to_i32(out.height) - t).max(0));
    for k in 0..out.width.div_ceil(cell) {
        let start = (k * cell) as f32;
        let x0 = (cell as f32).mul_add(0.25, start).round() as i32;
        let x1 = (cell as f32).mul_add(0.75, start).round() as i32;
        out.rect(x0, top, x1, top + t, FULL);
    }
}

/// How a box-drawing character's line leaves the middle towards one edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arm {
    /// It does not.
    No,
    /// One light stroke.
    Light,
    /// One stroke twice as thick.
    Heavy,
    /// Two light strokes, a light stroke apart.
    Double,
}

/// The four arms of a box-drawing character.
#[derive(Debug, Clone, Copy)]
struct Arms {
    up: Arm,
    right: Arm,
    down: Arm,
    left: Arm,
}

/// THE ARMS OF U+2500–257F, read off the characters' Unicode names, four
/// letters each in the order UP, RIGHT, DOWN, LEFT: `.` none, `l` light, `h`
/// heavy, `d` double. `-` is a character drawn some other way — a dash, an
/// arc or a diagonal. (Checked against Ghostty 1.4's own table, entry for
/// entry, when it was written.)
const BOX: [&str; 128] = [
    // 2500 ─ ━ │ ┃, then the eight dashes 2504–250B
    ".l.l", ".h.h", "l.l.", "h.h.", "-", "-", "-", "-", "-", "-", "-", "-",
    // 250C ┌ ┍ ┎ ┏ ┐ ┑ ┒ ┓
    ".ll.", ".hl.", ".lh.", ".hh.", "..ll", "..lh", "..hl", "..hh",
    // 2514 └ ┕ ┖ ┗ ┘ ┙ ┚ ┛
    "ll..", "lh..", "hl..", "hh..", "l..l", "l..h", "h..l", "h..h",
    // 251C ├ ┝ ┞ ┟ ┠ ┡ ┢ ┣
    "lll.", "lhl.", "hll.", "llh.", "hlh.", "hhl.", "lhh.", "hhh.",
    // 2524 ┤ ┥ ┦ ┧ ┨ ┩ ┪ ┫
    "l.ll", "l.lh", "h.ll", "l.hl", "h.hl", "h.lh", "l.hh", "h.hh",
    // 252C ┬ ┭ ┮ ┯ ┰ ┱ ┲ ┳
    ".lll", ".llh", ".hll", ".hlh", ".lhl", ".lhh", ".hhl", ".hhh",
    // 2534 ┴ ┵ ┶ ┷ ┸ ┹ ┺ ┻
    "ll.l", "ll.h", "lh.l", "lh.h", "hl.l", "hl.h", "hh.l", "hh.h",
    // 253C ┼ ┽ ┾ ┿ ╀ ╁ ╂ ╃
    "llll", "lllh", "lhll", "lhlh", "hlll", "llhl", "hlhl", "hllh",
    // 2544 ╄ ╅ ╆ ╇ ╈ ╉ ╊ ╋
    "hhll", "llhh", "lhhl", "hhlh", "lhhh", "hlhh", "hhhl", "hhhh",
    // 254C ╌ ╍ ╎ ╏ — dashes
    "-", "-", "-", "-", // 2550 ═ ║ ╒ ╓ ╔ ╕ ╖ ╗
    ".d.d", "d.d.", ".dl.", ".ld.", ".dd.", "..ld", "..dl", "..dd",
    // 2558 ╘ ╙ ╚ ╛ ╜ ╝ ╞ ╟
    "ld..", "dl..", "dd..", "l..d", "d..l", "d..d", "ldl.", "dld.",
    // 2560 ╠ ╡ ╢ ╣ ╤ ╥ ╦ ╧
    "ddd.", "l.ld", "d.dl", "d.dd", ".dld", ".ldl", ".ddd", "ld.d",
    // 2568 ╨ ╩ ╪ ╫ ╬
    "dl.l", "dd.d", "ldld", "dldl", "dddd",
    // 256D ╭ ╮ ╯ ╰ — arcs; 2571 ╱ ╲ ╳ — diagonals
    "-", "-", "-", "-", "-", "-", "-", // 2574 ╴ ╵ ╶ ╷ ╸ ╹ ╺ ╻
    "...l", "l...", ".l..", "..l.", "...h", "h...", ".h..", "..h.",
    // 257C ╼ ╽ ╾ ╿
    ".h.l", "l.h.", ".l.h", "h.l.",
];

/// The arms the table gives a codepoint of U+2500–257F, or `None` for one
/// drawn some other way.
fn arms_of(cp: u32) -> Option<Arms> {
    let entry = BOX.get(cp.checked_sub(0x2500)? as usize)?.as_bytes();
    let arm = |b: u8| match b {
        b'l' => Arm::Light,
        b'h' => Arm::Heavy,
        b'd' => Arm::Double,
        _ => Arm::No,
    };
    match entry {
        [up, right, down, left] => Some(Arms {
            up: arm(*up),
            right: arm(*right),
            down: arm(*down),
            left: arm(*left),
        }),
        _ => None,
    }
}

/// One of U+2500–257F.
fn box_drawing(out: &mut Coverage, f: &Frame, cp: u32) {
    if let Some(arms) = arms_of(cp) {
        lines(out, f, arms);
        return;
    }
    let light = f.stroke.max(1);
    let gap = light.max(4);
    match cp {
        0x2504 => dashes(out, 3, light, gap, true),
        0x2505 => dashes(out, 3, light * 2, gap, true),
        0x2506 => dashes(out, 3, light, gap, false),
        0x2507 => dashes(out, 3, light * 2, gap, false),
        0x2508 => dashes(out, 4, light, gap, true),
        0x2509 => dashes(out, 4, light * 2, gap, true),
        0x250A => dashes(out, 4, light, gap, false),
        0x250B => dashes(out, 4, light * 2, gap, false),
        0x254C => dashes(out, 2, light, gap, true),
        0x254D => dashes(out, 2, light * 2, gap, true),
        0x254E => dashes(out, 2, light, gap, false),
        0x254F => dashes(out, 2, light * 2, gap, false),
        0x256D..=0x2570 => arc(out, f, cp),
        0x2571 => diagonal(out, light, false),
        0x2572 => diagonal(out, light, true),
        0x2573 => {
            diagonal(out, light, false);
            diagonal(out, light, true);
        }
        _ => {}
    }
}

/// The four arms of a line character, each from its edge to past the middle
/// by exactly what closes the joint with the arms across it: a light arm
/// stops at the far side of a heavy crossing, a double one at the near side
/// of its partner's inner stroke — the junctions of `╋`, `╬` and `╪` that a
/// font only gets right at the size it was drawn for.
fn lines(out: &mut Coverage, f: &Frame, a: Arms) {
    use Arm::{Double, Heavy, Light, No};
    let (w, h) = (out.width, out.height);
    let light = f.stroke.max(1);
    let heavy = light * 2;

    // THE HORIZONTAL STROKES' ROWS, centred: light, heavy, and the two of a
    // double, which sit a light stroke above and below the light one.
    let h_light_top = h.saturating_sub(light) / 2;
    let h_light_bottom = h_light_top + light;
    let h_heavy_top = h.saturating_sub(heavy) / 2;
    let h_heavy_bottom = h_heavy_top + heavy;
    let h_double_top = h_light_top.saturating_sub(light);
    let h_double_bottom = h_light_bottom + light;
    // AND THE VERTICAL STROKES' COLUMNS, the same way.
    let v_light_left = w.saturating_sub(light) / 2;
    let v_light_right = v_light_left + light;
    let v_heavy_left = w.saturating_sub(heavy) / 2;
    let v_heavy_right = v_heavy_left + heavy;
    let v_double_left = v_light_left.saturating_sub(light);
    let v_double_right = v_light_right + light;

    // WHERE A VERTICAL ARM STOPS, past the middle: at the far edge of the
    // thickest stroke across it, or at its own light stroke's far side when
    // it is the only line through the middle (`│`, `╵`).
    let up_bottom = if a.left == Heavy || a.right == Heavy {
        h_heavy_bottom
    } else if a.left != a.right || a.down == a.up {
        if a.left == Double || a.right == Double {
            h_double_bottom
        } else {
            h_light_bottom
        }
    } else if a.left == No && a.right == No {
        h_light_bottom
    } else {
        h_light_top
    };
    let down_top = if a.left == Heavy || a.right == Heavy {
        h_heavy_top
    } else if a.left != a.right || a.up == a.down {
        if a.left == Double || a.right == Double {
            h_double_top
        } else {
            h_light_top
        }
    } else if a.left == No && a.right == No {
        h_light_top
    } else {
        h_light_bottom
    };
    let left_right = if a.up == Heavy || a.down == Heavy {
        v_heavy_right
    } else if a.up != a.down || a.left == a.right {
        if a.up == Double || a.down == Double {
            v_double_right
        } else {
            v_light_right
        }
    } else if a.up == No && a.down == No {
        v_light_right
    } else {
        v_light_left
    };
    let right_left = if a.up == Heavy || a.down == Heavy {
        v_heavy_left
    } else if a.up != a.down || a.right == a.left {
        if a.up == Double || a.down == Double {
            v_double_left
        } else {
            v_light_left
        }
    } else if a.up == No && a.down == No {
        v_light_left
    } else {
        v_light_right
    };

    let r = |out: &mut Coverage, x0: u32, y0: u32, x1: u32, y1: u32| {
        out.rect(to_i32(x0), to_i32(y0), to_i32(x1), to_i32(y1), FULL);
    };
    match a.up {
        No => {}
        Light => r(out, v_light_left, 0, v_light_right, up_bottom),
        Heavy => r(out, v_heavy_left, 0, v_heavy_right, up_bottom),
        Double => {
            let left_bottom = if a.left == Double {
                h_light_top
            } else {
                up_bottom
            };
            let right_bottom = if a.right == Double {
                h_light_top
            } else {
                up_bottom
            };
            r(out, v_double_left, 0, v_light_left, left_bottom);
            r(out, v_light_right, 0, v_double_right, right_bottom);
        }
    }
    match a.right {
        No => {}
        Light => r(out, right_left, h_light_top, w, h_light_bottom),
        Heavy => r(out, right_left, h_heavy_top, w, h_heavy_bottom),
        Double => {
            let top_left = if a.up == Double {
                v_light_right
            } else {
                right_left
            };
            let bottom_left = if a.down == Double {
                v_light_right
            } else {
                right_left
            };
            r(out, top_left, h_double_top, w, h_light_top);
            r(out, bottom_left, h_light_bottom, w, h_double_bottom);
        }
    }
    match a.down {
        No => {}
        Light => r(out, v_light_left, down_top, v_light_right, h),
        Heavy => r(out, v_heavy_left, down_top, v_heavy_right, h),
        Double => {
            let left_top = if a.left == Double {
                h_light_bottom
            } else {
                down_top
            };
            let right_top = if a.right == Double {
                h_light_bottom
            } else {
                down_top
            };
            r(out, v_double_left, left_top, v_light_left, h);
            r(out, v_light_right, right_top, v_double_right, h);
        }
    }
    match a.left {
        No => {}
        Light => r(out, 0, h_light_top, left_right, h_light_bottom),
        Heavy => r(out, 0, h_heavy_top, left_right, h_heavy_bottom),
        Double => {
            let top_right = if a.up == Double {
                v_light_left
            } else {
                left_right
            };
            let bottom_right = if a.down == Double {
                v_light_left
            } else {
                left_right
            };
            r(out, 0, h_double_top, top_right, h_light_top);
            r(out, 0, h_light_bottom, bottom_right, h_double_bottom);
        }
    }
}

/// `count` dashes across (or down) the cell, `thick` pixels thick, with gaps
/// of `gap` at most — half a gap at each end across, a whole one at the
/// bottom down, so a row or a column of them tiles into one even dashed line.
/// The pixels left over go to the dashes, where an extra one is not seen.
fn dashes(out: &mut Coverage, count: u32, thick: u32, gap: u32, across: bool) {
    let (length, breadth) = if across {
        (out.width, out.height)
    } else {
        (out.height, out.width)
    };
    let stroke_at = to_i32(breadth.saturating_sub(thick) / 2);
    let mut paint = |from: i32, to: i32| {
        if across {
            out.rect(from, stroke_at, to, stroke_at + to_i32(thick), FULL);
        } else {
            out.rect(stroke_at, from, stroke_at + to_i32(thick), to, FULL);
        }
    };
    // TOO SMALL TO DASH: a solid line, which is what a dashed line at that
    // size reads as anyway.
    if length < 2 * count {
        paint(0, to_i32(length));
        return;
    }
    let gap = gap.min(length / (2 * count));
    let dash = (length - count * gap) / count;
    let mut extra = (length - count * gap) % count;
    let mut at = if across { gap / 2 } else { 0 };
    for _ in 0..count {
        let mut end = at + dash;
        if extra > 0 {
            extra -= 1;
            end += 1;
        }
        paint(to_i32(at), to_i32(end));
        at = end + gap;
    }
}

/// `╭ ╮ ╯ ╰`: a quarter of a circle between the two arms, its radius half the
/// cell's narrower side, its stroke on the light lines' own middle — so the
/// arc's ends sit exactly on the straight lines of the cells beside it.
fn arc(out: &mut Coverage, f: &Frame, cp: u32) {
    let light = f.stroke.max(1);
    let (w, h) = (out.width, out.height);
    let t = light as f32;
    let cx = (w.saturating_sub(light) / 2) as f32 + t / 2.0;
    let cy = (h.saturating_sub(light) / 2) as f32 + t / 2.0;
    let r = w.min(h) as f32 / 2.0;
    // WHICH WAY THE ARMS GO: to the right (`1`) or the left (`-1`), and down
    // (`1`) or up (`-1`).
    let (sx, sy): (f32, f32) = match cp {
        0x256D => (1.0, 1.0),
        0x256E => (-1.0, 1.0),
        0x256F => (-1.0, -1.0),
        _ => (1.0, -1.0),
    };
    let (ox, oy) = (sx.mul_add(r, cx), sy.mul_add(r, cy));
    out.sample(FULL, |x, y| {
        // THE QUARTER FACING THE MIDDLE OF THE CELL from the circle's centre.
        let facing = (x - ox) * sx <= 0.0 && (y - oy) * sy <= 0.0;
        facing && ((x - ox).hypot(y - oy) - r).abs() <= t / 2.0
    });
    // AND THE STRAIGHT REST OF EACH ARM, from where the arc ends to the edge.
    let (v_left, v_right) = (cx - t / 2.0, cx + t / 2.0);
    let (h_top, h_bottom) = (cy - t / 2.0, cy + t / 2.0);
    let oy_px = oy.round() as i32;
    let ox_px = ox.round() as i32;
    let (vx0, vx1) = (v_left.round() as i32, v_right.round() as i32);
    let (hy0, hy1) = (h_top.round() as i32, h_bottom.round() as i32);
    if sy > 0.0 {
        out.rect(vx0, oy_px, vx1, to_i32(h), FULL);
    } else {
        out.rect(vx0, 0, vx1, oy_px, FULL);
    }
    if sx > 0.0 {
        out.rect(ox_px, hy0, to_i32(w), hy1, FULL);
    } else {
        out.rect(0, hy0, ox_px, hy1, FULL);
    }
}

/// `╲` (`falling`) or `╱`, corner to corner, carried half a stroke past each
/// corner so the diagonal of the next cell continues it with no notch.
fn diagonal(out: &mut Coverage, thick: u32, falling: bool) {
    let (w, h) = (out.width as f32, out.height as f32);
    let (a, b) = if falling {
        ((0.0, 0.0), (w, h))
    } else {
        ((w, 0.0), (0.0, h))
    };
    let half = thick as f32 / 2.0;
    out.sample(FULL, |x, y| near_segment((x, y), a, b, half, half));
}

/// Whether `p` is within `half` of the segment `a b`, the segment carried
/// `cap` past each end.
fn near_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32), half: f32, cap: f32) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return false;
    }
    let (ux, uy) = (dx / length, dy / length);
    let (px, py) = (p.0 - a.0, p.1 - a.1);
    let along = px.mul_add(ux, py * uy);
    let off = px.mul_add(uy, -(py * ux)).abs();
    along >= -cap && along <= length + cap && off <= half
}

/// Which side of the line `a b` the point `p` is on: the sign of the cross
/// product, zero on it.
fn side(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    (b.0 - a.0).mul_add(p.1 - a.1, -((b.1 - a.1) * (p.0 - a.0)))
}

/// Whether `p` is on the side of the line `a b` that `keep` (a [`side`])
/// names — a point ON the line going to the positive side only, so a shape
/// and its complement split the cell exactly and never both cover a sample.
fn on_the_side(p: (f32, f32), a: (f32, f32), b: (f32, f32), keep: f32) -> bool {
    let s = side(p, a, b);
    s * keep > 0.0 || (s == 0.0 && keep > 0.0)
}

/// Whether `p` is inside the triangle `a b c`, whichever way it winds.
fn in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let (d1, d2, d3) = (side(p, a, b), side(p, b, c), side(p, c, a));
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

/// One of U+2580–259F.
fn block(out: &mut Coverage, cp: u32) {
    let (w, h) = (out.width, out.height);
    let (wi, hi) = (to_i32(w), to_i32(h));
    let x8 = |n: u32| part(w, n, 8);
    let y8 = |n: u32| part(h, n, 8);
    let (xm, ym) = (part(w, 1, 2), part(h, 1, 2));
    match cp {
        // ▀ the upper half.
        0x2580 => out.rect(0, 0, wi, ym, FULL),
        // ▁▂▃▄▅▆▇█ the lower eighths, one to eight.
        0x2581..=0x2588 => out.rect(0, y8(8 - (cp - 0x2580)), wi, hi, FULL),
        // ▉▊▋▌▍▎▏ the left eighths, seven down to one.
        0x2589..=0x258F => out.rect(0, 0, x8(8 - (cp - 0x2588)), hi, FULL),
        // ▐ the right half.
        0x2590 => out.rect(xm, 0, wi, hi, FULL),
        0x2591 => out.rect(0, 0, wi, hi, LIGHT_SHADE),
        0x2592 => out.rect(0, 0, wi, hi, MEDIUM_SHADE),
        0x2593 => out.rect(0, 0, wi, hi, DARK_SHADE),
        // ▔ the upper eighth; ▕ the right eighth.
        0x2594 => out.rect(0, 0, wi, y8(1), FULL),
        0x2595 => out.rect(x8(7), 0, wi, hi, FULL),
        // ▖▗▘▙▚▛▜▝▞▟ the quadrants.
        0x2596..=0x259F => {
            // UPPER LEFT, UPPER RIGHT, LOWER LEFT, LOWER RIGHT, by name.
            let [ul, ur, ll, lr] = match cp {
                0x2596 => [false, false, true, false],
                0x2597 => [false, false, false, true],
                0x2598 => [true, false, false, false],
                0x2599 => [true, false, true, true],
                0x259A => [true, false, false, true],
                0x259B => [true, true, true, false],
                0x259C => [true, true, false, true],
                0x259D => [false, true, false, false],
                0x259E => [false, true, true, false],
                _ => [false, true, true, true],
            };
            if ul {
                out.rect(0, 0, xm, ym, FULL);
            }
            if ur {
                out.rect(xm, 0, wi, ym, FULL);
            }
            if ll {
                out.rect(0, ym, xm, hi, FULL);
            }
            if lr {
                out.rect(xm, ym, wi, hi, FULL);
            }
        }
        _ => {}
    }
}

/// One of U+2800–28FF: two columns of four dots, square, the codepoint's
/// eight low bits naming them — dots 1, 2, 3 down the left, 4, 5, 6 down the
/// right, then 7 and 8 under them.
fn braille(out: &mut Coverage, cp: u32) {
    let (w, h) = (out.width, out.height);
    let dot = (w / 4).min(h / 8).max(1);
    let half = dot as f32 / 2.0;
    let column = |c: u32| (w as f32 * (2 * c + 1) as f32 / 4.0 - half).round() as i32;
    let row = |r: u32| (h as f32 * (2 * r + 1) as f32 / 8.0 - half).round() as i32;
    // (bit, column, row), in Unicode's numbering of the dots.
    let dots = [
        (0, 0, 0),
        (1, 0, 1),
        (2, 0, 2),
        (3, 1, 0),
        (4, 1, 1),
        (5, 1, 2),
        (6, 0, 3),
        (7, 1, 3),
    ];
    let d = to_i32(dot);
    for (bit, c, r) in dots {
        if cp & (1 << bit) != 0 {
            let (x, y) = (column(c), row(r));
            out.rect(x, y, x + d, y + d, FULL);
        }
    }
}

/// One of powerline's U+E0B0–E0BF: the solid and the hollow arrows, the
/// half discs, and the corner triangles with their slanted lines.
fn powerline(out: &mut Coverage, f: &Frame, cp: u32) {
    let (w, h) = (out.width as f32, out.height as f32);
    let light = f.stroke.max(1);
    let t = light as f32;
    let triangle = |out: &mut Coverage, a, b, c| {
        out.sample(FULL, |x, y| in_triangle((x, y), a, b, c));
    };
    let chevron = |out: &mut Coverage, edge: f32, tip: f32| {
        out.sample(FULL, |x, y| {
            near_segment((x, y), (edge, 0.0), (tip, h / 2.0), t / 2.0, 0.0)
                || near_segment((x, y), (tip, h / 2.0), (edge, h), t / 2.0, 0.0)
        });
    };
    // A HALF DISC on one edge, filled or as its outline: an ellipse whose
    // centre is the middle of that edge, as wide as the cell and as tall.
    let half_disc = |out: &mut Coverage, centre: f32, filled: bool| {
        let (rx, ry) = (w, h / 2.0);
        out.sample(FULL, |x, y| {
            let at = |grow: f32| {
                let (a, b) = (rx + grow, ry + grow);
                if a <= 0.0 || b <= 0.0 {
                    return false;
                }
                let (u, v) = ((x - centre) / a, (y - h / 2.0) / b);
                u.mul_add(u, v * v) <= 1.0
            };
            if filled {
                at(0.0)
            } else {
                at(t / 2.0) && !at(-t / 2.0)
            }
        });
    };
    match cp {
        0xE0B0 => triangle(out, (0.0, 0.0), (w, h / 2.0), (0.0, h)),
        0xE0B1 => chevron(out, 0.0, w),
        0xE0B2 => triangle(out, (w, 0.0), (0.0, h / 2.0), (w, h)),
        0xE0B3 => chevron(out, w, 0.0),
        0xE0B4 => half_disc(out, 0.0, true),
        0xE0B5 => half_disc(out, 0.0, false),
        0xE0B6 => half_disc(out, w, true),
        0xE0B7 => half_disc(out, w, false),
        0xE0B8 => triangle(out, (0.0, 0.0), (w, h), (0.0, h)),
        0xE0BA => triangle(out, (w, 0.0), (w, h), (0.0, h)),
        0xE0BC => triangle(out, (0.0, 0.0), (w, 0.0), (0.0, h)),
        0xE0BE => triangle(out, (0.0, 0.0), (w, 0.0), (w, h)),
        0xE0B9 | 0xE0BF => diagonal(out, light, true),
        _ => diagonal(out, light, false),
    }
}

/// THE SMOOTH MOSAICS U+1FB3C–1FB51, by their names: a diagonal from one
/// point of the sextant grid to another, and the corner whose side of it is
/// filled. U+1FB52–1FB67 are the same 22 diagonals with the OPPOSITE corner
/// filled, which is how Unicode laid them out. A point is `(x, y)` in halves
/// across and thirds down: `x` 0, 1 or 2; `y` 0 to 3.
const MOSAICS: [Mosaic; 22] = [
    ((0, 2), (1, 3), Corner::LowerLeft),
    ((0, 2), (2, 3), Corner::LowerLeft),
    ((0, 1), (1, 3), Corner::LowerLeft),
    ((0, 1), (2, 3), Corner::LowerLeft),
    ((0, 0), (1, 3), Corner::LowerLeft),
    ((0, 1), (1, 0), Corner::LowerRight),
    ((0, 1), (2, 0), Corner::LowerRight),
    ((0, 2), (1, 0), Corner::LowerRight),
    ((0, 2), (2, 0), Corner::LowerRight),
    ((0, 3), (1, 0), Corner::LowerRight),
    ((0, 2), (2, 1), Corner::LowerRight),
    ((1, 3), (2, 2), Corner::LowerRight),
    ((0, 3), (2, 2), Corner::LowerRight),
    ((1, 3), (2, 1), Corner::LowerRight),
    ((0, 3), (2, 1), Corner::LowerRight),
    ((1, 3), (2, 0), Corner::LowerRight),
    ((1, 0), (2, 1), Corner::LowerLeft),
    ((0, 0), (2, 1), Corner::LowerLeft),
    ((1, 0), (2, 2), Corner::LowerLeft),
    ((0, 0), (2, 2), Corner::LowerLeft),
    ((1, 0), (2, 3), Corner::LowerLeft),
    ((0, 1), (2, 2), Corner::LowerLeft),
];

/// A point of the sextant grid: halves across, thirds down.
type Point = (u8, u8);

/// A smooth mosaic: its diagonal, from one point to another, and the corner
/// whose side of it is filled.
type Mosaic = (Point, Point, Corner);

/// A corner of the cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Corner {
    UpperLeft,
    UpperRight,
    LowerLeft,
    LowerRight,
}

impl Corner {
    /// The corner across from this one.
    const fn opposite(self) -> Self {
        match self {
            Self::UpperLeft => Self::LowerRight,
            Self::UpperRight => Self::LowerLeft,
            Self::LowerLeft => Self::UpperRight,
            Self::LowerRight => Self::UpperLeft,
        }
    }

    /// Where it is, in pixels of a `w × h` cell.
    fn at(self, w: f32, h: f32) -> (f32, f32) {
        match self {
            Self::UpperLeft => (0.0, 0.0),
            Self::UpperRight => (w, 0.0),
            Self::LowerLeft => (0.0, h),
            Self::LowerRight => (w, h),
        }
    }
}

/// One of the legacy computing symbols this module draws (see the module's
/// list); nothing for the rest of the range, which [`covers`] does not claim.
fn legacy(out: &mut Coverage, f: &Frame, cp: u32) {
    let (w, h) = (out.width, out.height);
    let (wi, hi) = (to_i32(w), to_i32(h));
    let (wf, hf) = (w as f32, h as f32);
    let x8 = |n: u32| part(w, n, 8);
    let y8 = |n: u32| part(h, n, 8);
    let (xm, ym) = (part(w, 1, 2), part(h, 1, 2));
    match cp {
        // THE SEXTANTS: a 2×3 grid, the codepoint counting the patterns from
        // one, past the three a block already has (the left half, the right
        // half and the full block).
        0x1FB00..=0x1FB3B => {
            let mut pattern = cp - 0x1FB00 + 1;
            if pattern >= 21 {
                pattern += 1;
            }
            if pattern >= 42 {
                pattern += 1;
            }
            let ys = [0, part(h, 1, 3), part(h, 2, 3), hi];
            for i in 0..6_u32 {
                if pattern & (1 << i) != 0 {
                    let (column, row) = (i % 2, (i / 2) as usize);
                    let (x0, x1) = if column == 0 { (0, xm) } else { (xm, wi) };
                    out.rect(x0, ys[row], x1, ys[row + 1], FULL);
                }
            }
        }
        0x1FB3C..=0x1FB67 => {
            let i = (cp - 0x1FB3C) as usize;
            let (a, b, corner) = MOSAICS[i % 22];
            let corner = if i < 22 { corner } else { corner.opposite() };
            let point = |(x, y): Point| (wf * f32::from(x) / 2.0, hf * f32::from(y) / 3.0);
            let (a, b) = (point(a), point(b));
            let keep = side(corner.at(wf, hf), a, b);
            out.sample(FULL, |x, y| on_the_side((x, y), a, b, keep));
        }
        // THE TRIANGULAR QUARTERS, by the cell's two diagonals: one quarter
        // (U+1FB6C–1FB6F, left, upper, right, lower), or the three others
        // (U+1FB68–1FB6B, which name the three they keep).
        0x1FB68..=0x1FB6F => {
            let quarter = [Edge::Left, Edge::Top, Edge::Right, Edge::Bottom][(cp & 3) as usize];
            let alone = cp >= 0x1FB6C;
            out.sample(FULL, |x, y| (Edge::of(x / wf, y / hf) == quarter) == alone);
        }
        // THE VERTICAL AND HORIZONTAL ONE EIGHTH BLOCKS 2 to 7.
        0x1FB70..=0x1FB75 => {
            let n = cp - 0x1FB70 + 1;
            out.rect(x8(n), 0, x8(n + 1), hi, FULL);
        }
        0x1FB76..=0x1FB7B => {
            let n = cp - 0x1FB76 + 1;
            out.rect(0, y8(n), wi, y8(n + 1), FULL);
        }
        // THE CORNERS OF EIGHTHS, and the two rows.
        0x1FB7C..=0x1FB80 => {
            let (left, right) = (
                cp == 0x1FB7C || cp == 0x1FB7D,
                cp == 0x1FB7E || cp == 0x1FB7F,
            );
            let upper = matches!(cp, 0x1FB7D | 0x1FB7E | 0x1FB80);
            let lower = matches!(cp, 0x1FB7C | 0x1FB7F | 0x1FB80);
            if left {
                out.rect(0, 0, x8(1), hi, FULL);
            }
            if right {
                out.rect(x8(7), 0, wi, hi, FULL);
            }
            if upper {
                out.rect(0, 0, wi, y8(1), FULL);
            }
            if lower {
                out.rect(0, y8(7), wi, hi, FULL);
            }
        }
        // HORIZONTAL ONE EIGHTH BLOCK-1358.
        0x1FB81 => {
            for n in [0, 2, 4, 7] {
                out.rect(0, y8(n), wi, y8(n + 1), FULL);
            }
        }
        // UPPER ONE QUARTER, THREE EIGHTHS, FIVE EIGHTHS, THREE QUARTERS AND
        // SEVEN EIGHTHS; then the same from the right.
        0x1FB82..=0x1FB86 => {
            let n = [2, 3, 5, 6, 7][(cp - 0x1FB82) as usize];
            out.rect(0, 0, wi, y8(n), FULL);
        }
        0x1FB87..=0x1FB8B => {
            let n = [2, 3, 5, 6, 7][(cp - 0x1FB87) as usize];
            out.rect(x8(8 - n), 0, wi, hi, FULL);
        }
        // THE SHADED HALVES: left, right, upper, lower.
        0x1FB8C => out.rect(0, 0, xm, hi, MEDIUM_SHADE),
        0x1FB8D => out.rect(xm, 0, wi, hi, MEDIUM_SHADE),
        0x1FB8E => out.rect(0, 0, wi, ym, MEDIUM_SHADE),
        0x1FB8F => out.rect(0, ym, wi, hi, MEDIUM_SHADE),
        // THE INVERSE MEDIUM SHADE, alone and beside a solid half: a shade is
        // a coverage here, and the inverse of half is half.
        0x1FB90 => out.rect(0, 0, wi, hi, MEDIUM_SHADE),
        0x1FB91 => {
            out.rect(0, 0, wi, hi, MEDIUM_SHADE);
            out.rect(0, 0, wi, ym, FULL);
        }
        0x1FB92 => {
            out.rect(0, 0, wi, hi, MEDIUM_SHADE);
            out.rect(0, ym, wi, hi, FULL);
        }
        0x1FB94 => {
            out.rect(0, 0, wi, hi, MEDIUM_SHADE);
            out.rect(xm, 0, wi, hi, FULL);
        }
        // THE CHECKERBOARDS, a quarter cell a square, and its inverse.
        0x1FB95 | 0x1FB96 => {
            let xs = [0, part(w, 1, 4), xm, part(w, 3, 4), wi];
            let ys = [0, part(h, 1, 4), ym, part(h, 3, 4), hi];
            let first = usize::from(cp == 0x1FB96);
            for row in 0..4 {
                for column in 0..4 {
                    if (row + column) % 2 == first {
                        out.rect(xs[column], ys[row], xs[column + 1], ys[row + 1], FULL);
                    }
                }
            }
        }
        // HEAVY HORIZONTAL FILL: the second and the fourth quarter rows.
        0x1FB97 => {
            out.rect(0, part(h, 1, 4), wi, ym, FULL);
            out.rect(0, part(h, 3, 4), wi, hi, FULL);
        }
        // THE TRIANGULAR HALVES — upper and lower, left and right — and the
        // four shaded corner triangles.
        0x1FB9A | 0x1FB9B => {
            let pair = if cp == 0x1FB9A {
                [Edge::Top, Edge::Bottom]
            } else {
                [Edge::Left, Edge::Right]
            };
            out.sample(FULL, |x, y| pair.contains(&Edge::of(x / wf, y / hf)));
        }
        0x1FB9C..=0x1FB9F => {
            let corner = [
                Corner::UpperLeft,
                Corner::UpperRight,
                Corner::LowerRight,
                Corner::LowerLeft,
            ][(cp - 0x1FB9C) as usize];
            let (a, b) = match corner {
                Corner::UpperLeft | Corner::LowerRight => ((wf, 0.0), (0.0, hf)),
                Corner::UpperRight | Corner::LowerLeft => ((0.0, 0.0), (wf, hf)),
            };
            let keep = side(corner.at(wf, hf), a, b);
            out.sample(MEDIUM_SHADE, |x, y| on_the_side((x, y), a, b, keep));
        }
        // THE DIAGONAL BOX LINES between the middles of the edges, by which
        // corners they cut: upper left, upper right, lower left, lower right.
        0x1FBA0..=0x1FBAE => {
            let [ul, ur, ll, lr] = match cp {
                0x1FBA0 => [true, false, false, false],
                0x1FBA1 => [false, true, false, false],
                0x1FBA2 => [false, false, true, false],
                0x1FBA3 => [false, false, false, true],
                0x1FBA4 => [true, false, true, false],
                0x1FBA5 => [false, true, false, true],
                0x1FBA6 => [false, false, true, true],
                0x1FBA7 => [true, true, false, false],
                0x1FBA8 => [true, false, false, true],
                0x1FBA9 => [false, true, true, false],
                0x1FBAA => [false, true, true, true],
                0x1FBAB => [true, false, true, true],
                0x1FBAC => [true, true, false, true],
                0x1FBAD => [true, true, true, false],
                _ => [true, true, true, true],
            };
            let half = f.stroke.max(1) as f32 / 2.0;
            let (top, bottom) = ((wf / 2.0, 0.0), (wf / 2.0, hf));
            let (left, right) = ((0.0, hf / 2.0), (wf, hf / 2.0));
            out.sample(FULL, |x, y| {
                let p = (x, y);
                (ul && near_segment(p, top, left, half, half))
                    || (ur && near_segment(p, top, right, half, half))
                    || (ll && near_segment(p, bottom, left, half, half))
                    || (lr && near_segment(p, bottom, right, half, half))
            });
        }
        // BOX DRAWINGS LIGHT HORIZONTAL WITH VERTICAL STROKE.
        0x1FBAF => lines(
            out,
            f,
            Arms {
                up: Arm::Heavy,
                right: Arm::Light,
                down: Arm::Heavy,
                left: Arm::Light,
            },
        ),
        _ => {}
    }
}

/// Which of the four triangles the cell's two diagonals cut it into a point
/// is in, the point given as fractions of the cell across and down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Left,
    Top,
    Right,
    Bottom,
}

impl Edge {
    /// The triangle `(u, v)` is in, ties going to the top and the bottom so
    /// every point is in exactly one.
    fn of(u: f32, v: f32) -> Self {
        let below_falling = v >= u;
        let below_rising = v >= 1.0 - u;
        match (below_falling, below_rising) {
            (true, true) => Self::Bottom,
            (false, false) => Self::Top,
            (true, false) => Self::Left,
            (false, true) => Self::Right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of `w × h`, its strokes where a font of that size would put
    /// them.
    fn frame(w: u32, h: u32, stroke: u32) -> Frame {
        let under = Stroke {
            top: to_i32(h) - to_i32(h / 6),
            thickness: stroke,
        };
        Frame {
            cell: w,
            width: w,
            height: h,
            stroke,
            underline: under,
            strike: Stroke {
                top: to_i32(h / 2),
                thickness: stroke,
            },
            overline: Stroke {
                top: 0,
                thickness: stroke,
            },
        }
    }

    /// Every size a cell of this system takes at any scale, and some it never
    /// does — odd and even, narrow and tall — each with every stroke a face
    /// could give a cell that size: one that leaves room for a double line's
    /// two strokes and the gap between them, which a font's underline always
    /// does (it is a twentieth of the em, and the cell is six tenths of it).
    fn sizes() -> impl Iterator<Item = (u32, u32, u32)> {
        (4..=44)
            .step_by(3)
            .flat_map(|w| (8..=96).step_by(5).map(move |h| (w, h)))
            .flat_map(|(w, h)| (1..=4).map(move |s| (w, h, s)))
            .filter(|&(w, h, s)| 3 * s + 2 <= w && 3 * s + 2 <= h)
    }

    /// FAILS IF: a box-drawing line stops short of an edge it runs to, at
    /// any size tried — a column of `│` would be a dashed line, a row of `─`
    /// a dotted one, and a frame of `┼` a grid with holes.
    #[test]
    fn a_box_drawing_line_touches_both_edges_of_the_cell() {
        for (w, h, s) in sizes() {
            let f = frame(w, h, s);
            for c in ['─', '━', '═', '┼', '╋', '╬', '╪', '╫'] {
                let r = glyph(c, &f).expect("a sprite");
                assert!(
                    (0..h).any(|y| r.at(0, y) == FULL && r.at(w - 1, y) == FULL),
                    "{c} at {w}x{h}/{s} does not cross from the left edge to the right one"
                );
            }
            for c in ['│', '┃', '║', '┼', '╋', '╬', '╪', '╫'] {
                let r = glyph(c, &f).expect("a sprite");
                assert!(
                    (0..w).any(|x| r.at(x, 0) == FULL && r.at(x, h - 1) == FULL),
                    "{c} at {w}x{h}/{s} does not run from the top edge to the bottom one"
                );
            }
        }
    }

    /// FAILS IF: an arm of a corner or a tee is not joined to the arm across
    /// it — `┌` drawn as two strokes with a notch at their meeting, which is
    /// the defect a font's glyph shows when its box is not the cell's.
    #[test]
    fn the_arms_of_a_corner_meet() {
        for (w, h, s) in sizes() {
            // A STROKE AS WIDE AS THE CELL has no "left of the middle" to
            // leave empty.
            if w < s + 2 || h < s + 2 {
                continue;
            }
            let f = frame(w, h, s);
            let r = glyph('┌', &f).expect("a sprite");
            let (cx, cy) = ((w.saturating_sub(s)) / 2, (h.saturating_sub(s)) / 2);
            // THE JOINT: every pixel of the stroke's square at the middle.
            for y in cy..cy + s {
                for x in cx..cx + s {
                    assert_eq!(r.at(x, y), FULL, "┌ at {w}x{h}/{s} has a hole at {x},{y}");
                }
            }
            // AND NOTHING ABOVE THE MIDDLE OR LEFT OF IT.
            assert_eq!(r.at(cx, 0), 0, "┌ at {w}x{h}/{s} has an arm up");
            assert_eq!(r.at(0, cy), 0, "┌ at {w}x{h}/{s} has an arm left");
            assert_eq!(r.at(w - 1, cy), FULL, "┌ at {w}x{h}/{s} has no arm right");
            assert_eq!(r.at(cx, h - 1), FULL, "┌ at {w}x{h}/{s} has no arm down");
        }
    }

    /// FAILS IF: the full block leaves a pixel of its cell uncovered — a
    /// column of `█` would show the ground through a seam.
    #[test]
    fn a_full_block_fills_the_cell() {
        for (w, h, s) in sizes() {
            let r = glyph('█', &frame(w, h, s)).expect("a sprite");
            assert!(r.data.iter().all(|&v| v == FULL), "█ at {w}x{h} has a gap");
        }
    }

    /// FAILS IF: the two halves of a cell overlap or part — `▀` over `▄`
    /// must be the full block exactly, and `▌` beside `▐` too, at odd sizes
    /// as well as even ones.
    #[test]
    fn two_halves_make_the_whole_block() {
        for (w, h, s) in sizes() {
            let f = frame(w, h, s);
            for (a, b) in [('▀', '▄'), ('▌', '▐'), ('▘', '▟'), ('▝', '▙')] {
                let (ra, rb) = (glyph(a, &f).expect("a"), glyph(b, &f).expect("b"));
                for (i, (x, y)) in ra.data.iter().zip(&rb.data).enumerate() {
                    assert!(
                        (*x == FULL) != (*y == FULL),
                        "{a}{b} at {w}x{h}: pixel {i} is in both or neither"
                    );
                }
            }
        }
    }

    /// FAILS IF: a codepoint claimed by `covers` draws nothing, or one
    /// `glyph` would draw is not claimed — the font would be asked for a
    /// blank, or never asked for a glyph it has.
    #[test]
    fn what_is_covered_is_what_is_drawn() {
        let f = frame(11, 26, 1);
        for cp in (0x2500..0x2600)
            .chain(0x2800..0x2900)
            .chain(0xE0A0..0xE0D0)
            .chain(0x1FB00..0x1FC00)
        {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let drawn = glyph(c, &f);
            assert_eq!(covers(c), drawn.is_some(), "U+{cp:04X}");
            // THE BRAILLE BLANK is the one codepoint drawn as nothing, by its
            // own definition.
            if let Some(r) = drawn
                && cp != 0x2800
            {
                assert!(!r.is_blank(), "U+{cp:04X} is claimed and draws nothing");
            }
        }
    }

    /// FAILS IF: a mosaic's diagonal is read from the wrong corner — 🬼 (the
    /// small lower-left wedge) and 🭒 (its complement) must share the
    /// diagonal and cover the cell between them exactly once, as the names
    /// say.
    #[test]
    fn a_mosaic_and_its_complement_make_the_whole_cell() {
        let f = frame(12, 27, 1);
        for i in 0..22 {
            let (a, b) = (0x1FB3C + i, 0x1FB52 + i);
            let ra = glyph(char::from_u32(a).expect("a"), &f).expect("a sprite");
            let rb = glyph(char::from_u32(b).expect("b"), &f).expect("a sprite");
            for (k, (x, y)) in ra.data.iter().zip(&rb.data).enumerate() {
                let sum = u32::from(*x) + u32::from(*y);
                // TO A SHADE'S ROUNDING: each side floors its sixteenth.
                assert!(
                    sum + 1 >= u32::from(FULL) && sum <= u32::from(FULL),
                    "U+{a:04X} and U+{b:04X} cover pixel {k} {sum}/255 between them"
                );
            }
        }
        // AND THE FIRST IS THE SMALL ONE, in the lower left.
        let wedge = glyph('\u{1FB3C}', &f).expect("a sprite");
        assert_eq!(wedge.at(0, 26), FULL);
        assert_eq!(wedge.at(11, 0), 0);
    }

    /// FAILS IF: an underline leaves its cell, or a curl or a dash does not
    /// carry on from one cell into the next — a wavy line under a word would
    /// break at every letter.
    #[test]
    fn an_underline_stays_in_its_cell_and_tiles() {
        for (w, h, s) in sizes() {
            let f = frame(w, h, s);
            for kind in [
                Underline::Single,
                Underline::Double,
                Underline::Curly,
                Underline::Dotted,
                Underline::Dashed,
            ] {
                let r = line(Line::Under(kind), &f);
                assert_eq!((r.width, r.height), (w, h));
                assert!(!r.is_blank(), "{kind:?} at {w}x{h}/{s} draws nothing");
            }
            // A CURL is low at both edges of its cell, so the next cell's
            // begins where this one ends.
            let curl = line(Line::Under(Underline::Curly), &f);
            let lowest = |x: u32| (0..h).rev().find(|&y| curl.at(x, y) > 0);
            assert_eq!(
                lowest(0),
                lowest(w - 1),
                "the curl at {w}x{h}/{s} does not tile"
            );
        }
        assert!(line(Line::Under(Underline::None), &frame(11, 26, 1)).is_blank());
    }
}
