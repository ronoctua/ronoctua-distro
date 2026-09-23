//! The arithmetic of each effect.
//!
//! Every function takes the frame in place and its parameters already clamped
//! to their declared range (`Step::value`). None of them allocates on a frame
//! after the first: what needs a copy borrows [`Scratch`].

use crate::{Frame, Scratch};

/// Rec. 709 luma, on the stored values. A camera frame is sRGB and this is
/// the weighting a greyscale conversion is expected to look like; the
/// linear-light version reads darker than anyone asked for.
fn luma(p: &[u8]) -> f32 {
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}

fn byte(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn mix(a: u8, b: f32, t: f32) -> u8 {
    byte(f32::from(a) * (1.0 - t) + b * t)
}

pub(crate) fn grey(f: &mut Frame, amount: i32) {
    let t = amount as f32 / 100.0;
    for p in f.rgba.as_chunks_mut::<4>().0 {
        let l = luma(p);
        p[0] = mix(p[0], l, t);
        p[1] = mix(p[1], l, t);
        p[2] = mix(p[2], l, t);
    }
}

pub(crate) fn sepia(f: &mut Frame, amount: i32) {
    let t = amount as f32 / 100.0;
    for p in f.rgba.as_chunks_mut::<4>().0 {
        let (r, g, b) = (f32::from(p[0]), f32::from(p[1]), f32::from(p[2]));
        // The matrix every photo editor uses for "sepia" (Microsoft's, 2006).
        let sr = 0.393 * r + 0.769 * g + 0.189 * b;
        let sg = 0.349 * r + 0.686 * g + 0.168 * b;
        let sb = 0.272 * r + 0.534 * g + 0.131 * b;
        p[0] = mix(p[0], sr, t);
        p[1] = mix(p[1], sg, t);
        p[2] = mix(p[2], sb, t);
    }
}

pub(crate) fn negative(f: &mut Frame) {
    for p in f.rgba.as_chunks_mut::<4>().0 {
        p[0] = 255 - p[0];
        p[1] = 255 - p[1];
        p[2] = 255 - p[2];
    }
}

pub(crate) fn posterize(f: &mut Frame, levels: i32) {
    let n = (levels.max(2) - 1) as f32;
    let q = |v: u8| byte((f32::from(v) * n / 255.0).round() * 255.0 / n);
    for p in f.rgba.as_chunks_mut::<4>().0 {
        p[0] = q(p[0]);
        p[1] = q(p[1]);
        p[2] = q(p[2]);
    }
}

/// A hue in degrees at full saturation and value, to RGB in `[0, 1]`.
fn hue_rgb(h: f32) -> [f32; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    match h as u32 {
        0 => [1.0, x, 0.0],
        1 => [x, 1.0, 0.0],
        2 => [0.0, 1.0, x],
        3 => [0.0, x, 1.0],
        4 => [x, 0.0, 1.0],
        _ => [1.0, 0.0, x],
    }
}

/// Luma to a hue sweep from blue (cold) through red to yellow, and the last
/// tenth to white — the ramp every thermal camera draws, COMPUTED rather than
/// listed, which is what keeps a hex value out of this file.
pub(crate) fn thermal(f: &mut Frame) {
    for p in f.rgba.as_chunks_mut::<4>().0 {
        let t = luma(p) / 255.0;
        // 240° (blue) down to -60° (= 300°, magenta) through red to yellow at 60°:
        // one continuous sweep of 300 degrees going the short way round.
        let hue = 240.0 - t.min(0.9) / 0.9 * 180.0;
        let mut c = hue_rgb(hue);
        let v = 0.25 + 0.75 * (t / 0.9).min(1.0);
        for ch in &mut c {
            *ch *= v;
        }
        let white = ((t - 0.9) / 0.1).clamp(0.0, 1.0);
        p[0] = byte((c[0] * (1.0 - white) + white) * 255.0);
        p[1] = byte((c[1] * (1.0 - white) + white) * 255.0);
        p[2] = byte((c[2] * (1.0 - white) + white) * 255.0);
    }
}

/// An image intensifier's phosphor: luma amplified, drawn in the green channel
/// with a trace of the others so white highlights stay white-ish.
pub(crate) fn night(f: &mut Frame, gain: i32) {
    let g = gain as f32;
    for p in f.rgba.as_chunks_mut::<4>().0 {
        let l = (luma(p) * g).min(255.0);
        p[0] = byte(l * 0.15);
        p[1] = byte(l);
        p[2] = byte(l * 0.15);
    }
}

/// Four sliders, all zero at rest: brightness adds, contrast scales around the
/// middle grey, saturation scales away from luma, temperature trades red for
/// blue.
pub(crate) fn tone(f: &mut Frame, [brightness, contrast, saturation, temperature]: [i32; 4]) {
    if brightness == 0 && contrast == 0 && saturation == 0 && temperature == 0 {
        return;
    }
    let add = brightness as f32 * 1.28;
    let scale = if contrast >= 0 {
        1.0 + contrast as f32 / 50.0
    } else {
        1.0 + contrast as f32 / 100.0
    };
    let sat = 1.0 + saturation as f32 / 100.0;
    let warm = temperature as f32 * 0.4;
    for p in f.rgba.as_chunks_mut::<4>().0 {
        let mut c = [f32::from(p[0]), f32::from(p[1]), f32::from(p[2])];
        let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        for ch in &mut c {
            *ch = l + (*ch - l) * sat;
            *ch = (*ch - 128.0) * scale + 128.0 + add;
        }
        c[0] += warm;
        c[2] -= warm;
        p[0] = byte(c[0]);
        p[1] = byte(c[1]);
        p[2] = byte(c[2]);
    }
}

pub(crate) fn pixelate(f: &mut Frame, size: i32) {
    let s = size.max(1) as usize;
    let (w, h) = (f.width, f.height);
    for by in (0..h).step_by(s) {
        for bx in (0..w).step_by(s) {
            let (x1, y1) = ((bx + s).min(w), (by + s).min(h));
            let mut sum = [0u32; 4];
            for y in by..y1 {
                for x in bx..x1 {
                    let p = &f.rgba[(y * w + x) * 4..][..4];
                    for (acc, v) in sum.iter_mut().zip(p) {
                        *acc += u32::from(*v);
                    }
                }
            }
            let n = ((x1 - bx) * (y1 - by)) as u32;
            let avg = sum.map(|v| (v / n) as u8);
            for y in by..y1 {
                for x in bx..x1 {
                    f.rgba[(y * w + x) * 4..][..4].copy_from_slice(&avg);
                }
            }
        }
    }
}

/// One horizontal and one vertical running-sum box pass, three times: the
/// cheapest blur that looks Gaussian, and it costs the same at any radius.
pub(crate) fn blur(f: &mut Frame, radius: i32, scratch: &mut Scratch) {
    let r = radius.max(1) as usize;
    for _ in 0..3 {
        box_pass(&mut f.rgba, f.width, f.height, r, true, scratch);
        box_pass(&mut f.rgba, f.width, f.height, r, false, scratch);
    }
}

fn box_pass(px: &mut [u8], w: usize, h: usize, r: usize, across: bool, scratch: &mut Scratch) {
    let (lines, len) = if across { (h, w) } else { (w, h) };
    if len == 0 {
        return;
    }
    let at = |line: usize, i: usize| {
        if across {
            (line * w + i) * 4
        } else {
            (i * w + line) * 4
        }
    };
    scratch.line.clear();
    scratch.line.resize(len * 4, 0);
    let window = (2 * r + 1) as u32;
    for line in 0..lines {
        for i in 0..len {
            let o = at(line, i);
            for c in 0..4 {
                scratch.line[i * 4 + c] = u32::from(px[o + c]);
            }
        }
        // The edge is EXTENDED, not faded to black: a blurred frame must not
        // grow a dark border it did not have.
        let clamp = |i: isize| i.clamp(0, len as isize - 1) as usize;
        let mut sum = [0u32; 4];
        for k in -(r as isize)..=(r as isize) {
            let j = clamp(k);
            for (s, v) in sum.iter_mut().zip(&scratch.line[j * 4..j * 4 + 4]) {
                *s += v;
            }
        }
        for i in 0..len {
            let o = at(line, i);
            for (d, s) in px[o..o + 4].iter_mut().zip(&sum) {
                *d = (s / window) as u8;
            }
            let out = clamp(i as isize - r as isize);
            let inn = clamp(i as isize + r as isize + 1);
            let (gain, loss) = (
                &scratch.line[inn * 4..inn * 4 + 4],
                &scratch.line[out * 4..out * 4 + 4],
            );
            for ((s, g), l) in sum.iter_mut().zip(gain).zip(loss) {
                *s = *s + g - l;
            }
        }
    }
}

/// Edges in the luma (Sobel), drawn dark on white: a pencil sketch.
pub(crate) fn sketch(f: &mut Frame, strength: i32, scratch: &mut Scratch) {
    let (w, h) = (f.width, f.height);
    scratch.copy.clear();
    scratch
        .copy
        .extend(f.rgba.as_chunks::<4>().0.iter().map(|p| byte(luma(p))));
    let gain = strength as f32 / 25.0;
    let l = |x: isize, y: isize| {
        let x = x.clamp(0, w as isize - 1) as usize;
        let y = y.clamp(0, h as isize - 1) as usize;
        f32::from(scratch.copy[y * w + x])
    };
    for y in 0..h as isize {
        for x in 0..w as isize {
            let gx = -l(x - 1, y - 1) - 2.0 * l(x - 1, y) - l(x - 1, y + 1)
                + l(x + 1, y - 1)
                + 2.0 * l(x + 1, y)
                + l(x + 1, y + 1);
            let gy = -l(x - 1, y - 1) - 2.0 * l(x, y - 1) - l(x + 1, y - 1)
                + l(x - 1, y + 1)
                + 2.0 * l(x, y + 1)
                + l(x + 1, y + 1);
            let edge = ((gx * gx + gy * gy).sqrt() * gain / 4.0).min(255.0);
            let v = byte(255.0 - edge);
            let o = (y as usize * w + x as usize) * 4;
            f.rgba[o] = v;
            f.rgba[o + 1] = v;
            f.rgba[o + 2] = v;
        }
    }
}

pub(crate) fn vignette(f: &mut Frame, strength: i32) {
    let (w, h) = (f.width as f32, f.height as f32);
    let k = strength as f32 / 100.0;
    let (cx, cy) = (w / 2.0, h / 2.0);
    let reach = (cx * cx + cy * cy).sqrt().max(1.0);
    for (i, p) in f.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let x = (i % f.width) as f32 + 0.5 - cx;
        let y = (i / f.width) as f32 + 0.5 - cy;
        let d = (x * x + y * y).sqrt() / reach;
        // Flat in the middle, falling off past half the way to a corner.
        let fall = ((d - 0.5) / 0.5).clamp(0.0, 1.0);
        let keep = 1.0 - k * fall * fall;
        for c in &mut p[..3] {
            *c = byte(f32::from(*c) * keep);
        }
    }
}

/// The left half mirrored onto the right.
pub(crate) fn symmetry(f: &mut Frame) {
    let w = f.width;
    for y in 0..f.height {
        for x in w / 2..w {
            let from = (y * w + (w - 1 - x)) * 4;
            let to = (y * w + x) * 4;
            f.rgba.copy_within(from..from + 4, to);
        }
    }
}

/// The frame folded into `segments` mirrored wedges around the centre.
pub(crate) fn kaleidoscope(f: &mut Frame, segments: i32, scratch: &mut Scratch) {
    let (w, h) = (f.width, f.height);
    scratch.copy.clear();
    scratch.copy.extend_from_slice(&f.rgba);
    let wedge = std::f32::consts::TAU / segments.max(2) as f32;
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let r = (dx * dx + dy * dy).sqrt();
            let mut a = dy.atan2(dx).rem_euclid(wedge * 2.0);
            if a > wedge {
                a = wedge * 2.0 - a;
            }
            let sx = (cx + r * a.cos()).clamp(0.0, w as f32 - 1.0) as usize;
            let sy = (cy + r * a.sin()).clamp(0.0, h as f32 - 1.0) as usize;
            let from = (sy * w + sx) * 4;
            f.rgba[(y * w + x) * 4..][..4].copy_from_slice(&scratch.copy[from..from + 4]);
        }
    }
}

/// Hue, saturation and value of a pixel: degrees and two fractions.
fn hsv(p: &[u8]) -> (f32, f32, f32) {
    let (r, g, b) = (
        f32::from(p[0]) / 255.0,
        f32::from(p[1]) / 255.0,
        f32::from(p[2]) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

/// A green (or any hue) screen: pixels near the key hue, and saturated enough
/// to be a screen and not a grey shirt, become transparent — softly at the
/// edge of the tolerance, so hair does not cut out as a staircase.
pub(crate) fn chroma_key(f: &mut Frame, hue: i32, tolerance: i32) {
    let reach = tolerance as f32 * 1.8;
    let soft = (reach * 0.25).max(1.0);
    for p in f.rgba.as_chunks_mut::<4>().0 {
        let (h, s, v) = hsv(p);
        if s < 0.25 || v < 0.15 {
            continue;
        }
        let d = (h - hue as f32).rem_euclid(360.0);
        let d = d.min(360.0 - d);
        let keep = ((d - reach) / soft).clamp(0.0, 1.0);
        p[3] = byte(f32::from(p[3]) * keep);
    }
}

/// The background blurred, the person kept.
///
/// # At a fraction of the size, and why that is not a shortcut
///
/// The whole-frame blur is six passes over every pixel, and at 1280×720 it
/// measured 46 ms a frame — more than a 30 fps camera's 33, so every frame
/// fell further behind and the picture lagged by as much as ffmpeg would
/// queue. What is blurred here is a BACKGROUND, whose detail the effect exists
/// to remove: it is averaged down by `shrink` in each direction, blurred there
/// with the radius shrunk to match, and brought back up bilinearly. Detail
/// lost to the shrink is detail the blur was going to lose anyway.
///
/// Where the mask is the person (`255`) nothing of this is read: the original
/// pixel stays, byte for byte.
pub(crate) fn background_blur(f: &mut Frame, radius: i32, mask: &[u8], scratch: &mut Scratch) {
    let (w, h) = (f.width, f.height);
    if w == 0 || h == 0 || mask.len() < w * h {
        return;
    }
    let r = radius.max(1) as usize;
    let shrink = blur_shrink(r);
    let (sw, sh) = (w.div_ceil(shrink), h.div_ceil(shrink));
    let mut small = Frame {
        width: sw,
        height: sh,
        rgba: std::mem::take(&mut scratch.small),
    };
    small.rgba.clear();
    small.rgba.resize(sw * sh * 4, 0);
    // DOWN: the average of each `shrink × shrink` block (the last row and
    // column average what is there).
    for sy in 0..sh {
        let (y0, y1) = (sy * shrink, ((sy + 1) * shrink).min(h));
        for sx in 0..sw {
            let (x0, x1) = (sx * shrink, ((sx + 1) * shrink).min(w));
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                for p in f.rgba[(y * w + x0) * 4..(y * w + x1) * 4]
                    .as_chunks::<4>()
                    .0
                {
                    for (acc, v) in sum.iter_mut().zip(p) {
                        *acc += u32::from(*v);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let o = (sy * sw + sx) * 4;
            for (d, s) in small.rgba[o..o + 4].iter_mut().zip(sum) {
                *d = ((s + n / 2) / n) as u8;
            }
        }
    }
    blur(&mut small, (r / shrink).max(1) as i32, scratch);
    // UP, blended by the mask, in fixed point (weights out of 256). The
    // column's two neighbours and weight are the same on every row, so they
    // are worked out once.
    let place = |i: usize, n: usize| {
        let at = ((i as f32 + 0.5) / shrink as f32 - 0.5).max(0.0);
        let lo = (at as usize).min(n - 1);
        let hi = (lo + 1).min(n - 1);
        let t = ((at - lo as f32) * 256.0) as u32;
        (lo, hi, t.min(256))
    };
    let columns: Vec<(usize, usize, u32)> = (0..w).map(|x| place(x, sw)).collect();
    for y in 0..h {
        let (ya, yb, ty) = place(y, sh);
        for (x, &(xa, xb, tx)) in columns.iter().enumerate() {
            let i = y * w + x;
            let m = u32::from(mask[i]);
            if m == 255 {
                continue;
            }
            let s = |yy: usize, xx: usize, c: usize| u32::from(small.rgba[(yy * sw + xx) * 4 + c]);
            let o = i * 4;
            for c in 0..4 {
                let top = s(ya, xa, c) * (256 - tx) + s(ya, xb, c) * tx;
                let bottom = s(yb, xa, c) * (256 - tx) + s(yb, xb, c) * tx;
                let blurred = (top * (256 - ty) + bottom * ty + (1 << 15)) >> 16;
                let kept = u32::from(f.rgba[o + c]);
                f.rgba[o + c] = ((kept * m + blurred * (255 - m) + 127) / 255) as u8;
            }
        }
    }
    scratch.small = small.rgba;
}

/// How much smaller the background is blurred, for a blur `radius`: a fourth
/// of the radius, so the blur that is left is never under four pixels (below
/// that the box's steps would show), and at most eight.
fn blur_shrink(radius: usize) -> usize {
    (radius / 4).clamp(1, 8)
}

pub(crate) fn background_replace(f: &mut Frame, back: &Frame, mask: &[u8]) {
    for (i, p) in f.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let m = f32::from(mask[i]) / 255.0;
        let b = &back.rgba[i * 4..][..4];
        for c in 0..4 {
            p[c] = byte(f32::from(p[c]) * m + f32::from(b[c]) * (1.0 - m));
        }
    }
}

pub(crate) fn background_remove(f: &mut Frame, mask: &[u8]) {
    for (i, p) in f.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        p[3] = byte(f32::from(p[3]) * f32::from(mask[i]) / 255.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_person_is_kept_byte_for_byte_and_a_flat_background_stays_flat() {
        // Wider than the blur reaches: three box passes of radius 20 spread
        // the person some sixty pixels.
        let (w, h) = (240, 23);
        let mut rgba = Vec::new();
        for i in 0..w * h {
            // A person on the left half, noisy; a flat background elsewhere.
            if i % w < w / 2 {
                rgba.extend_from_slice(&[(i * 7 % 251) as u8, (i * 13 % 251) as u8, 9, 255]);
            } else {
                rgba.extend_from_slice(&[80, 120, 200, 255]);
            }
        }
        let mask: Vec<u8> = (0..w * h)
            .map(|i| if i % w < w / 2 { 255 } else { 0 })
            .collect();
        let before = rgba.clone();
        let mut f = Frame {
            width: w,
            height: h,
            rgba,
        };
        background_blur(&mut f, 20, &mask, &mut Scratch::default());
        for i in 0..w * h {
            let (px, was) = (&f.rgba[i * 4..i * 4 + 4], &before[i * 4..i * 4 + 4]);
            if mask[i] == 255 {
                assert_eq!(px, was, "the person changed at {i}");
            } else if i % w > w / 2 + 80 {
                // Far from the person the blur has only the flat colour to
                // average: no border, no step from the shrink.
                for c in 0..4 {
                    assert!(px[c].abs_diff(was[c]) <= 1, "{px:?} at {i}");
                }
            }
        }
    }

    #[test]
    fn the_blur_is_shrunk_by_a_fourth_of_its_radius_and_never_below_four_pixels() {
        assert_eq!(blur_shrink(1), 1);
        assert_eq!(blur_shrink(20), 5);
        assert_eq!(blur_shrink(60), 8);
        for r in 1..=60 {
            assert!(r / blur_shrink(r) >= 1);
            assert!(r < 4 || r / blur_shrink(r) >= 4, "radius {r}");
        }
    }

    #[test]
    fn grey_makes_the_three_channels_equal() {
        let mut f = Frame {
            width: 1,
            height: 1,
            rgba: vec![200, 50, 10, 255],
        };
        grey(&mut f, 100);
        assert_eq!(f.rgba[0], f.rgba[1]);
        assert_eq!(f.rgba[1], f.rgba[2]);
    }

    #[test]
    fn a_blur_of_a_flat_frame_is_the_same_frame() {
        // The edge is extended, not faded: a flat frame has no edge to lose.
        let mut f = Frame::filled(9, 7, [40, 90, 200, 255]);
        let before = f.clone();
        blur(&mut f, 3, &mut Scratch::default());
        assert_eq!(f, before);
    }

    #[test]
    fn a_blur_spreads_a_single_point() {
        let mut f = Frame::filled(9, 9, [0, 0, 0, 255]);
        f.rgba[(4 * 9 + 4) * 4] = 255;
        blur(&mut f, 1, &mut Scratch::default());
        assert!(f.rgba[(4 * 9 + 5) * 4] > 0, "the neighbour got some");
        assert!(f.rgba[(4 * 9 + 4) * 4] < 255, "the point lost some");
    }

    #[test]
    fn posterize_to_two_levels_is_black_or_white() {
        let mut f = Frame {
            width: 3,
            height: 1,
            rgba: vec![10, 10, 10, 255, 128, 128, 128, 255, 250, 250, 250, 255],
        };
        posterize(&mut f, 2);
        for v in f.rgba.chunks(4).flat_map(|p| p[..3].to_vec()) {
            assert!(v == 0 || v == 255);
        }
    }

    #[test]
    fn the_thermal_ramp_runs_cold_to_hot() {
        let mut dark = Frame::filled(1, 1, [10, 10, 10, 255]);
        let mut bright = Frame::filled(1, 1, [250, 250, 250, 255]);
        thermal(&mut dark);
        thermal(&mut bright);
        assert!(dark.rgba[2] > dark.rgba[0], "dark is blue-ish");
        assert!(
            bright.rgba[0] > 200 && bright.rgba[1] > 200,
            "bright is yellow to white"
        );
    }

    #[test]
    fn a_green_screen_goes_and_a_face_stays() {
        let mut f = Frame {
            width: 2,
            height: 1,
            rgba: vec![20, 220, 30, 255, 220, 170, 140, 255],
        };
        chroma_key(&mut f, 120, 30);
        assert_eq!(f.rgba[3], 0, "green is keyed out");
        assert_eq!(f.rgba[7], 255, "skin stays");
    }

    #[test]
    fn symmetry_mirrors_the_left_onto_the_right() {
        let mut f = Frame {
            width: 4,
            height: 1,
            rgba: vec![1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255],
        };
        symmetry(&mut f);
        let reds: Vec<u8> = f.rgba.chunks(4).map(|p| p[0]).collect();
        assert_eq!(reds, [1, 2, 2, 1]);
    }

    #[test]
    fn the_vignette_leaves_the_centre_and_darkens_the_corner() {
        let mut f = Frame::filled(21, 21, [200, 200, 200, 255]);
        vignette(&mut f, 100);
        assert_eq!(f.rgba[(10 * 21 + 10) * 4], 200);
        assert!(f.rgba[0] < 100);
    }
}
