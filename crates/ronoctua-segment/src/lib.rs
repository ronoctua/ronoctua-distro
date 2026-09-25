//! Which pixels of a camera frame are a person.
//!
//! # Whose it is
//!
//! The studio's (`ronoctua-studio`): three effects — blur the background,
//! replace it, remove it — need to know where the person ends, and this crate
//! is the one answer. It has no thread, no clock and no file: [`Segmenter`] is
//! built from the MODEL'S BYTES and answers a frame with a [`Mask`]. Whoever
//! owns the camera decides how often to ask; the mask is cheap to sample at any
//! resolution, so it is asked at the model's size and read at the frame's.
//!
//! # The model
//!
//! MediaPipe's `selfie_segmenter` (Apache-2.0), float16, version 1 — pinned by
//! hash in `nix/segment-model.nix`. Its input is 256×256 RGB in `[0, 1]`, its
//! output one channel of person-probability at the same size. The frame is
//! squeezed to that square and the mask stretched back: the model was trained
//! on exactly that squeeze, which is what MediaPipe itself feeds it.
//!
//! # And why the mask is SMOOTHED
//!
//! Frame to frame, the raw answer flickers at the edge of the hair — a mask
//! taken as is makes the background blur BOIL around the head. [`Smoother`]
//! blends each new mask into the last one (an exponential average, the weight
//! a parameter of the effect), which is the cheapest thing that stops that and
//! the reason the struct keeps state and the function does not.

#![forbid(unsafe_code)]

mod flat;
mod hands;
mod net;

pub use hands::{Hand, Hands, Palm, Square};
pub use net::{Net, Refusal, Scratch};

/// A person-probability per cell, `0.0` background to `1.0` person.
#[derive(Debug, Clone, PartialEq)]
pub struct Mask {
    /// Cells across.
    pub width: usize,
    /// Cells down.
    pub height: usize,
    /// Row-major, `width * height` of them.
    pub cells: Vec<f32>,
}

impl Mask {
    /// A mask saying "all person" — what an effect gets while the first real
    /// mask is still being computed, so nothing is blurred by mistake.
    pub fn whole(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![1.0; width * height],
        }
    }

    /// The probability at a point given in `[0, 1]` of the frame, bilinear.
    pub fn at(&self, u: f32, v: f32) -> f32 {
        if self.cells.is_empty() {
            return 1.0;
        }
        let x = (u * self.width as f32 - 0.5).clamp(0.0, (self.width - 1) as f32);
        let y = (v * self.height as f32 - 0.5).clamp(0.0, (self.height - 1) as f32);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let c = |x: usize, y: usize| self.cells[y * self.width + x];
        (c(x0, y0) * (1.0 - fx) + c(x1, y0) * fx) * (1.0 - fy)
            + (c(x0, y1) * (1.0 - fx) + c(x1, y1) * fx) * fy
    }

    /// The mask resampled to a frame's size, one `u8` alpha per pixel.
    ///
    /// Bilinear per pixel; a 720p frame is under a million of them, and the
    /// caller asks once per MASK, not once per frame.
    pub fn alpha(&self, width: usize, height: usize, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(width * height);
        for y in 0..height {
            let v = (y as f32 + 0.5) / height as f32;
            for x in 0..width {
                let u = (x as f32 + 0.5) / width as f32;
                out.push((self.at(u, v) * 255.0).round().clamp(0.0, 255.0) as u8);
            }
        }
    }
}

/// The model, loaded, and the buffers one run needs.
pub struct Segmenter {
    net: Net,
    scratch: Scratch,
    input: Vec<f32>,
}

impl Segmenter {
    /// Reads the model's bytes.
    pub fn new(model: &[u8]) -> Result<Self, Refusal> {
        let net = Net::read(model)?;
        let [_, _, c] = net.input_shape();
        if c != 3 {
            return Err(Refusal::Unsupported(format!("{c} input channels")));
        }
        Ok(Self {
            net,
            scratch: Scratch::default(),
            input: Vec::new(),
        })
    }

    /// The side the model wants its input at.
    pub fn input_size(&self) -> (usize, usize) {
        let [h, w, _] = self.net.input_shape();
        (w, h)
    }

    /// The mask of an RGBA frame, at the model's resolution.
    ///
    /// The frame is squeezed to the model's input with a box filter (every
    /// source pixel counts once — a point sample of a 1080p frame at 256 would
    /// alias hair into noise), and each channel is mapped to `[0, 1]`.
    pub fn mask(&mut self, rgba: &[u8], width: usize, height: usize) -> Mask {
        let (mw, mh) = self.input_size();
        squeeze(rgba, width, height, mw, mh, &mut self.input);
        let out = self.net.run(&self.input, &mut self.scratch);
        let [oh, ow, _] = self.net.output_shape();
        Mask {
            width: ow,
            height: oh,
            cells: out.iter().map(|v| v.clamp(0.0, 1.0)).collect(),
        }
    }
}

/// An RGBA frame to an RGB `f32` tensor in `[0, 1]`, by area average.
pub fn squeeze(
    rgba: &[u8],
    width: usize,
    height: usize,
    to_w: usize,
    to_h: usize,
    out: &mut Vec<f32>,
) {
    out.clear();
    out.resize(to_w * to_h * 3, 0.0);
    if width == 0 || height == 0 {
        return;
    }
    for ty in 0..to_h {
        let y0 = ty * height / to_h;
        let y1 = ((ty + 1) * height / to_h).max(y0 + 1).min(height);
        for tx in 0..to_w {
            let x0 = tx * width / to_w;
            let x1 = ((tx + 1) * width / to_w).max(x0 + 1).min(width);
            let mut sum = [0u32; 3];
            for y in y0..y1 {
                let row = &rgba[(y * width + x0) * 4..(y * width + x1) * 4];
                for px in row.as_chunks::<4>().0 {
                    sum[0] += u32::from(px[0]);
                    sum[1] += u32::from(px[1]);
                    sum[2] += u32::from(px[2]);
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as f32 * 255.0;
            let dst = &mut out[(ty * to_w + tx) * 3..][..3];
            dst[0] = sum[0] as f32 / n;
            dst[1] = sum[1] as f32 / n;
            dst[2] = sum[2] as f32 / n;
        }
    }
}

/// The temporal filter that stops the edge from boiling.
#[derive(Debug, Default)]
pub struct Smoother {
    last: Option<Mask>,
}

impl Smoother {
    /// Blends `fresh` into the last answer. `keep` is how much of the old mask
    /// survives, `0.0` (none: the raw model) to `0.9` (a slow, steady edge).
    pub fn take(&mut self, fresh: Mask, keep: f32) -> &Mask {
        let keep = keep.clamp(0.0, 0.95);
        match &mut self.last {
            Some(old) if old.width == fresh.width && old.height == fresh.height => {
                for (o, n) in old.cells.iter_mut().zip(&fresh.cells) {
                    *o = *o * keep + n * (1.0 - keep);
                }
            }
            slot => *slot = Some(fresh),
        }
        self.last.as_ref().expect("just set")
    }

    /// Forgets the last mask — a new camera is a new scene.
    pub fn reset(&mut self) {
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The model this distro pins, when the environment carries it. The
    /// build's tests do (`nix/rust.nix`); a bare `cargo test` without the
    /// devshell skips the two model tests and SAYS so on stderr, rather than
    /// passing in silence.
    fn model() -> Option<Vec<u8>> {
        let path = std::env::var("RONOCTUA_SEGMENT_MODEL").ok()?;
        std::fs::read(path).ok()
    }

    /// The synthetic frame the reference was taken on: the same arithmetic
    /// as the Python that produced MediaPipe's answer, so nothing personal —
    /// no face, no photograph — is in this repository.
    fn synthetic() -> Vec<u8> {
        let mut out = Vec::with_capacity(256 * 256 * 4);
        for y in 0..256i64 {
            for x in 0..256i64 {
                let r = (x * 7 + y * 3) % 256;
                let g = (x * x / 50 + y) % 256;
                let inside = (x - 128).pow(2) + (y - 100).pow(2) < 60 * 60;
                let b = if inside { 230 } else { 30 };
                out.extend_from_slice(&[r as u8, g as u8, b as u8, 255]);
            }
        }
        out
    }

    /// MediaPipe 1.0.1's confidence mask for [`synthetic`], sampled on a
    /// 16×16 grid at the centre of every 16th cell.
    ///
    /// Taken on 2026-09-23 with the `mediapipe` wheel in a throwaway venv:
    /// `vision.ImageSegmenter` over the pinned model with
    /// `output_confidence_masks=True`, fed the frame [`synthetic`] builds, and
    /// `mask[8::16, 8::16]` printed to five decimals. Over the whole 256×256 the
    /// two implementations disagreed by at most 5.8e-5 (mean 1.3e-7); a kernel
    /// flipped in the transposed convolution moves that to 0.31, which is what
    /// the bound below is for.
    const REFERENCE: &str = include_str!("reference.txt");

    #[test]
    fn the_mask_is_mediapipes_own() {
        let Some(bytes) = model() else {
            eprintln!("RONOCTUA_SEGMENT_MODEL is not set: the model test did not run");
            return;
        };
        let mut s = Segmenter::new(&bytes).expect("the pinned model reads");
        assert_eq!(s.input_size(), (256, 256));
        let mask = s.mask(&synthetic(), 256, 256);
        let reference: Vec<f32> = REFERENCE
            .split_whitespace()
            .filter_map(|w| w.parse().ok())
            .collect();
        assert_eq!(reference.len(), 256, "sixteen by sixteen");
        let mut worst = 0.0f32;
        for (i, want) in reference.iter().enumerate() {
            let (gy, gx) = (i / 16, i % 16);
            let got = mask.cells[(gy * 16 + 8) * 256 + gx * 16 + 8];
            worst = worst.max((got - want).abs());
        }
        // float16 weights through float32 arithmetic in a different order:
        // the two agree to the third decimal, not bit for bit.
        assert!(worst < 0.01, "worst disagreement with MediaPipe: {worst}");
    }

    #[test]
    fn the_smoother_keeps_what_it_is_told_to() {
        let mut s = Smoother::default();
        s.take(
            Mask {
                width: 1,
                height: 1,
                cells: vec![0.0],
            },
            0.5,
        );
        let m = s.take(
            Mask {
                width: 1,
                height: 1,
                cells: vec![1.0],
            },
            0.5,
        );
        assert!((m.cells[0] - 0.5).abs() < 1e-6);
        let m = s.take(
            Mask {
                width: 2,
                height: 1,
                cells: vec![1.0, 1.0],
            },
            0.5,
        );
        assert_eq!(m.cells, vec![1.0, 1.0], "a new size starts over");
    }

    #[test]
    fn the_squeeze_counts_every_pixel_once() {
        // A 4×2 frame, left half black, right half white → 2×1: 0 and 1.
        let mut frame = Vec::new();
        for _ in 0..2 {
            for x in 0..4 {
                let v = if x < 2 { 0 } else { 255 };
                frame.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let mut out = Vec::new();
        squeeze(&frame, 4, 2, 2, 1, &mut out);
        assert_eq!(out, vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn a_mask_samples_its_corners_exactly() {
        let m = Mask {
            width: 2,
            height: 2,
            cells: vec![0.0, 1.0, 1.0, 0.0],
        };
        assert_eq!(m.at(0.0, 0.0), 0.0);
        assert_eq!(m.at(1.0, 0.0), 1.0);
        let mut a = Vec::new();
        m.alpha(2, 2, &mut a);
        assert_eq!(a, vec![0, 255, 255, 0]);
    }
}
