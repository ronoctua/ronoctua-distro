//! Layers into one program frame, on the CPU.
//!
//! # Why the CPU, measured
//!
//! Every source already arrives at (or near) the size its layer uses — ffmpeg
//! scales the camera and the media inside its own SIMD code — so composing is
//! placing and blending rectangles: 3.7 MB a frame at 720p. The GPU path the
//! plan named (`ronoctua-render` offscreen) would pay an upload and a
//! read-back of the same 3.7 MB per frame to save work that is this cheap, and
//! it would need a device in a unit with no screen. The measurement is in the
//! test below, which fails if a 720p frame of two layers stops fitting a
//! thirtieth of a second.
//!
//! # The one sampling rule
//!
//! Bilinear, from the CROPPED source rectangle to the layer's place, with
//! straight alpha blended over what is below (`src·a + dst·(1−a)`). The canvas
//! itself stays opaque: what leaves this module is a video frame, and a video
//! frame has no transparency to give a viewer.

use ronoctua_effects::Frame;

use crate::scene::{Fit, Rect};

/// One layer, ready to draw.
pub struct Placed<'a> {
    /// The picture.
    pub frame: &'a Frame,
    /// Where, in fractions of the program.
    pub rect: Rect,
    /// How it fills that.
    pub fit: Fit,
    /// 0 to 100.
    pub opacity: u8,
    /// Left, top, right, bottom, in fractions of the picture.
    pub crop: [f32; 4],
}

/// Left, top, width and height in program pixels.
type Place = (u32, u32, u32, u32);

/// The source rectangle (in source pixels, as `f32`) and the destination
/// rectangle (in program pixels) one layer maps between.
fn mapping(layer: &Placed<'_>, width: u32, height: u32) -> Option<([f32; 4], Place)> {
    let f = layer.frame;
    if f.width == 0 || f.height == 0 || !f.is_whole() {
        return None;
    }
    let [cl, ct, cr, cb] = layer.crop.map(|c| c.clamp(0.0, 0.45));
    let sx = cl * f.width as f32;
    let sy = ct * f.height as f32;
    let sw = (1.0 - cl - cr) * f.width as f32;
    let sh = (1.0 - ct - cb) * f.height as f32;
    let (dx, dy, dw, dh) = layer.rect.pixels(width, height);
    let (dw_f, dh_f) = (dw as f32, dh as f32);
    let source_ratio = sw / sh;
    let place_ratio = dw_f / dh_f;
    match layer.fit {
        Fit::Stretch => Some(([sx, sy, sw, sh], (dx, dy, dw, dh))),
        Fit::Fill => {
            // Crop the source further to the place's shape.
            if source_ratio > place_ratio {
                let keep = sh * place_ratio;
                Some(([sx + (sw - keep) / 2.0, sy, keep, sh], (dx, dy, dw, dh)))
            } else {
                let keep = sw / place_ratio;
                Some(([sx, sy + (sh - keep) / 2.0, sw, keep], (dx, dy, dw, dh)))
            }
        }
        Fit::Fit => {
            // Shrink the place to the source's shape, centred.
            if source_ratio > place_ratio {
                let h = (dw_f / source_ratio).round().max(1.0) as u32;
                Some((
                    [sx, sy, sw, sh],
                    (dx, dy + (dh - h.min(dh)) / 2, dw, h.min(dh)),
                ))
            } else {
                let w = (dh_f * source_ratio).round().max(1.0) as u32;
                Some((
                    [sx, sy, sw, sh],
                    (dx + (dw - w.min(dw)) / 2, dy, w.min(dw), dh),
                ))
            }
        }
    }
}

/// Draws the layers, bottom first, over an opaque black canvas.
///
/// Rows are split across the machine's threads with `std::thread::scope`:
/// each row of the canvas is written by exactly one thread, so the split is
/// safe without a lock.
pub fn compose(canvas: &mut Frame, width: u32, height: u32, layers: &[Placed<'_>]) {
    canvas.width = width as usize;
    canvas.height = height as usize;
    canvas.rgba.clear();
    canvas.rgba.resize(width as usize * height as usize * 4, 0);
    for px in canvas.rgba.as_chunks_mut::<4>().0 {
        px[3] = 255;
    }
    let maps: Vec<_> = layers
        .iter()
        .filter_map(|l| mapping(l, width, height).map(|m| (l, m)))
        .collect();
    if maps.is_empty() {
        return;
    }
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, 8);
    let row_bytes = width as usize * 4;
    let rows_each = (height as usize).div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (chunk, band) in canvas.rgba.chunks_mut(rows_each * row_bytes).enumerate() {
            let maps = &maps;
            scope.spawn(move || {
                let first_row = chunk * rows_each;
                for (l, (src, (dx, dy, dw, dh))) in maps {
                    let alpha = f32::from(l.opacity.min(100)) / 100.0;
                    if alpha <= 0.0 {
                        continue;
                    }
                    let f = l.frame;
                    let rows_here = band.len() / row_bytes;
                    let y0 = (*dy as usize).max(first_row);
                    let y1 = ((dy + dh) as usize).min(first_row + rows_here);
                    for y in y0..y1 {
                        let v =
                            src[1] + ((y as f32 - *dy as f32 + 0.5) / *dh as f32) * src[3] - 0.5;
                        let row = &mut band[(y - first_row) * row_bytes..][..row_bytes];
                        for x in *dx as usize..(dx + dw) as usize {
                            let u = src[0] + ((x as f32 - *dx as f32 + 0.5) / *dw as f32) * src[2]
                                - 0.5;
                            let s = sample(f, u, v);
                            let a = f32::from(s[3]) / 255.0 * alpha;
                            if a <= 0.0 {
                                continue;
                            }
                            let d = &mut row[x * 4..x * 4 + 4];
                            for c in 0..3 {
                                d[c] = (f32::from(s[c]) * a + f32::from(d[c]) * (1.0 - a)).round()
                                    as u8;
                            }
                        }
                    }
                }
            });
        }
    });
}

/// Bilinear sample at a pixel-space point, edges extended.
fn sample(f: &Frame, u: f32, v: f32) -> [u8; 4] {
    let (w, h) = (f.width as isize, f.height as isize);
    let x = u.clamp(0.0, (w - 1) as f32);
    let y = v.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as isize, y.floor() as isize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |x: isize, y: isize| &f.rgba[((y * w + x) * 4) as usize..][..4];
    let (p00, p10, p01, p11) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
    let mut out = [0u8; 4];
    for c in 0..4 {
        let top = f32::from(p00[c]) * (1.0 - fx) + f32::from(p10[c]) * fx;
        let bottom = f32::from(p01[c]) * (1.0 - fx) + f32::from(p11[c]) * fx;
        out[c] = (top * (1.0 - fy) + bottom * fy).round() as u8;
    }
    out
}

/// The largest size of `w × h`'s shape that fits `max_w × max_h`, never
/// larger than `w × h` itself — enlarging invents pixels — and always even.
pub fn fit_within(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (2, 2);
    }
    let scale = (f64::from(max_w) / f64::from(w))
        .min(f64::from(max_h) / f64::from(h))
        .min(1.0);
    let even = |v: f64| (((v.round() as u32) / 2) * 2).max(2);
    (even(f64::from(w) * scale), even(f64::from(h) * scale))
}

/// A frame resampled to `w × h`: the AREA average when shrinking by two or
/// more (a point sample of a 4K screen at 720p would shimmer every edge of
/// text), bilinear otherwise.
pub fn resize(src: &Frame, w: u32, h: u32) -> Frame {
    let (w, h) = (w as usize, h as usize);
    if src.width == w && src.height == h {
        return src.clone();
    }
    let mut out = Frame {
        width: w,
        height: h,
        rgba: vec![0; w * h * 4],
    };
    if src.width == 0 || src.height == 0 || !src.is_whole() {
        return out;
    }
    let box_filter = src.width >= w * 2 && src.height >= h * 2;
    for y in 0..h {
        for x in 0..w {
            let px = if box_filter {
                let (x0, x1) = (
                    x * src.width / w,
                    ((x + 1) * src.width / w).max(x * src.width / w + 1),
                );
                let (y0, y1) = (
                    y * src.height / h,
                    ((y + 1) * src.height / h).max(y * src.height / h + 1),
                );
                let mut sum = [0u32; 4];
                for sy in y0..y1.min(src.height) {
                    for sx in x0..x1.min(src.width) {
                        let p = &src.rgba[(sy * src.width + sx) * 4..][..4];
                        for c in 0..4 {
                            sum[c] += u32::from(p[c]);
                        }
                    }
                }
                let n = ((y1.min(src.height) - y0) * (x1.min(src.width) - x0)).max(1) as u32;
                sum.map(|v| (v / n) as u8)
            } else {
                let u = (x as f32 + 0.5) * src.width as f32 / w as f32 - 0.5;
                let v = (y as f32 + 0.5) * src.height as f32 / h as f32 - 0.5;
                sample(src, u, v)
            };
            out.rgba[(y * w + x) * 4..][..4].copy_from_slice(&px);
        }
    }
    out
}

/// A cross-fade: `into = from·(1−t) + to·t`, in place over `into`, which holds
/// `to` on entry. Both frames are the program's size.
pub fn fade(into: &mut Frame, from: &Frame, t: f32) {
    if from.rgba.len() != into.rgba.len() {
        return;
    }
    let t = t.clamp(0.0, 1.0);
    for (d, s) in into.rgba.iter_mut().zip(&from.rgba) {
        *d = (f32::from(*s) * (1.0 - t) + f32::from(*d) * t).round() as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(frame: &Frame, rect: Rect, fit: Fit) -> Placed<'_> {
        Placed {
            frame,
            rect,
            fit,
            opacity: 100,
            crop: [0.0; 4],
        }
    }

    #[test]
    fn a_whole_layer_fills_the_program_with_its_colour() {
        let red = Frame::filled(4, 4, [255, 0, 0, 255]);
        let mut canvas = Frame::default();
        compose(
            &mut canvas,
            16,
            9,
            &[placed(&red, Rect::WHOLE, Fit::Stretch)],
        );
        assert!(
            canvas
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [255, 0, 0, 255])
        );
    }

    #[test]
    fn fit_leaves_bars_and_fill_does_not() {
        let tall = Frame::filled(10, 20, [0, 255, 0, 255]);
        let mut canvas = Frame::default();
        compose(&mut canvas, 40, 20, &[placed(&tall, Rect::WHOLE, Fit::Fit)]);
        assert_eq!(&canvas.rgba[..4], &[0, 0, 0, 255], "the left edge is a bar");
        let mid = (10 * 40 + 20) * 4;
        assert_eq!(&canvas.rgba[mid..mid + 4], &[0, 255, 0, 255]);
        compose(
            &mut canvas,
            40,
            20,
            &[placed(&tall, Rect::WHOLE, Fit::Fill)],
        );
        assert_eq!(&canvas.rgba[..4], &[0, 255, 0, 255], "fill has no bars");
    }

    #[test]
    fn a_corner_layer_lands_in_its_corner_over_the_one_below() {
        let blue = Frame::filled(8, 8, [0, 0, 255, 255]);
        let white = Frame::filled(8, 8, [255, 255, 255, 255]);
        let mut canvas = Frame::default();
        let corner = Rect {
            x: 0.5,
            y: 0.5,
            w: 0.5,
            h: 0.5,
        };
        compose(
            &mut canvas,
            20,
            20,
            &[
                placed(&blue, Rect::WHOLE, Fit::Stretch),
                placed(&white, corner, Fit::Stretch),
            ],
        );
        assert_eq!(&canvas.rgba[..4], &[0, 0, 255, 255]);
        let last = canvas.rgba.len() - 4;
        assert_eq!(&canvas.rgba[last..], &[255, 255, 255, 255]);
    }

    #[test]
    fn a_transparent_pixel_shows_what_is_below() {
        let blue = Frame::filled(2, 2, [0, 0, 255, 255]);
        let clear = Frame::filled(2, 2, [255, 0, 0, 0]);
        let mut canvas = Frame::default();
        compose(
            &mut canvas,
            4,
            4,
            &[
                placed(&blue, Rect::WHOLE, Fit::Stretch),
                placed(&clear, Rect::WHOLE, Fit::Stretch),
            ],
        );
        assert!(
            canvas
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [0, 0, 255, 255])
        );
    }

    #[test]
    fn half_opacity_is_half_the_colour() {
        let white = Frame::filled(2, 2, [255, 255, 255, 255]);
        let mut canvas = Frame::default();
        let mut l = placed(&white, Rect::WHOLE, Fit::Stretch);
        l.opacity = 50;
        compose(&mut canvas, 2, 2, &[l]);
        assert_eq!(canvas.rgba[0], 128);
    }

    #[test]
    fn a_fade_runs_from_one_frame_to_the_other() {
        let from = Frame::filled(2, 1, [0, 0, 0, 255]);
        let mut into = Frame::filled(2, 1, [200, 200, 200, 255]);
        fade(&mut into, &from, 0.25);
        assert_eq!(into.rgba[0], 50);
    }

    #[test]
    fn a_size_fits_and_is_never_enlarged() {
        assert_eq!(fit_within(3840, 2160, 1280, 720), (1280, 720));
        assert_eq!(fit_within(640, 480, 1280, 720), (640, 480));
        assert_eq!(fit_within(1080, 1920, 1280, 720), (404, 720));
    }

    #[test]
    fn shrinking_averages_every_pixel() {
        let mut src = Frame::filled(4, 2, [0, 0, 0, 255]);
        for y in 0..2 {
            for x in 2..4 {
                src.rgba[(y * 4 + x) * 4] = 255;
            }
        }
        let small = resize(&src, 2, 1);
        assert_eq!(small.rgba, vec![0, 0, 0, 255, 255, 0, 0, 255]);
    }

    #[test]
    fn two_layers_at_720p_fit_a_frame() {
        // THE MEASUREMENT behind "the CPU is enough": a full-frame camera and
        // a corner screen, 1280×720, must compose inside a thirtieth of a
        // second. Only in an optimised build — a debug build measures the
        // compiler, not the arithmetic.
        if cfg!(debug_assertions) {
            return;
        }
        let camera = Frame::filled(1280, 720, [10, 20, 30, 255]);
        let screen = Frame::filled(1920, 1080, [40, 50, 60, 255]);
        let mut canvas = Frame::default();
        let layers = [
            placed(&camera, Rect::WHOLE, Fit::Fill),
            placed(
                &screen,
                Rect {
                    x: 0.7,
                    y: 0.7,
                    w: 0.25,
                    h: 0.25,
                },
                Fit::Fit,
            ),
        ];
        let began = std::time::Instant::now();
        for _ in 0..10 {
            compose(&mut canvas, 1280, 720, &layers);
        }
        let each = began.elapsed() / 10;
        assert!(
            each < std::time::Duration::from_millis(33),
            "{each:?} a frame"
        );
    }
}
