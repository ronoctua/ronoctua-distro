//! Where the hands in a camera frame are: 21 points each, as MediaPipe's
//! hand landmarker finds them, run by this crate's own interpreter.
//!
//! # Two models, and the second is what runs most frames
//!
//! `hand_detector.tflite` (192²) finds PALMS anywhere in the frame; it is the
//! expensive search. `hand_landmarks_detector.tflite` (224²) reads one hand in
//! a rotated square around it and answers its 21 points, whether a hand is
//! there at all, and which hand it is. As MediaPipe does, the square for the
//! next frame is derived from this frame's points, so the palm search only
//! runs while fewer than two hands are held — and even then only every
//! [`SEARCH_EVERY`] frames, because a hand that just entered can wait a tenth
//! of a second and the search costs more than both hands together.
//!
//! # The geometry is MediaPipe's, number for number
//!
//! The anchors (`SsdAnchorsCalculator`: four layers, strides 8/16/16/16, one
//! aspect ratio plus the interpolated one, fixed size), the decode (x, y, w, h
//! divided by 192), the WEIGHTED non-maximum suppression at 0.3, the palm
//! rectangle turned so the wrist→middle-knuckle line points up and grown 2.6×
//! with a half-height shift, and the hand rectangle grown 2.0× with a tenth's
//! shift. Every angle is measured in PIXELS, never in normalised units: on a
//! 16:9 frame the two disagree, and the square would be tilted.
//!
//! # Mirrored, as the models assume
//!
//! Handedness is trained on SELFIE images. A caller that feeds an unmirrored
//! frame says so, and the answer is flipped here — both the points and the
//! hand — so everything downstream reads the person's own left and right.

use crate::{Net, Refusal, Scratch};

/// The detector's side.
const DETECTOR: usize = 192;
/// The landmark model's side.
const LANDMARKS: usize = 224;
/// A palm is kept above this score (after the sigmoid).
const PALM_SCORE: f32 = 0.5;
/// Overlap above which two palms are one.
const SUPPRESSION: f32 = 0.3;
/// A hand is still there above this presence.
const PRESENCE: f32 = 0.5;
/// How many hands are followed at once.
pub const MOST: usize = 2;
/// Frames between two palm searches while fewer than [`MOST`] hands are held.
pub const SEARCH_EVERY: u32 = 3;
/// Threads each convolution may take. ONE, measured: two cut a hand from
/// 30 ms to 19 but cost a quarter more CPU in spawning and waiting, and
/// gestures run while somebody works — lightness is what was asked for, and
/// the pointer's filter hides a reading that is a few milliseconds later.
pub const THREADS: usize = 1;
/// The points the next square is measured from: the palm and the base of
/// every finger, never a fingertip, which moves too much to anchor anything.
const ANCHORS: [usize; 12] = [0, 1, 2, 3, 5, 6, 9, 10, 13, 14, 17, 18];

/// One hand.
#[derive(Debug, Clone, PartialEq)]
pub struct Hand {
    /// The 21 points, `x` and `y` in `0..1` of the frame (as the person sees
    /// it: mirrored), `z` in the same unit as `x`, smaller nearer the camera.
    pub points: [[f32; 3]; 21],
    /// How sure the model is there is a hand.
    pub presence: f32,
    /// The person's RIGHT hand.
    pub right: bool,
}

/// A rotated square in PIXELS of the frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Square {
    /// Centre across.
    pub cx: f32,
    /// Centre down.
    pub cy: f32,
    /// The side.
    pub side: f32,
    /// Radians, the turn that makes the hand point up.
    pub turn: f32,
}

#[derive(Debug, Clone, Copy)]
struct Anchor {
    x: f32,
    y: f32,
}

/// A palm the detector found, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palm {
    /// After the sigmoid.
    pub score: f32,
    /// Centre, width and height.
    pub box_: [f32; 4],
    /// The seven palm keypoints; 0 is the wrist, 2 the middle knuckle.
    pub keys: [[f32; 2]; 7],
}

/// Both models, their buffers, and the hands held from the last frame.
#[derive(Debug)]
pub struct Hands {
    detector: Net,
    landmarks: Net,
    scratch: Scratch,
    anchors: Vec<Anchor>,
    held: Vec<Square>,
    since_search: u32,
    input: Vec<f32>,
}

impl Hands {
    /// Reads both models' bytes.
    pub fn new(detector: &[u8], landmarks: &[u8]) -> Result<Self, Refusal> {
        let mut detector = Net::read(detector)?;
        let mut landmarks = Net::read(landmarks)?;
        detector.set_threads(THREADS);
        landmarks.set_threads(THREADS);
        if detector.input_shape() != [DETECTOR, DETECTOR, 3]
            || landmarks.input_shape() != [LANDMARKS, LANDMARKS, 3]
            || detector.outputs() < 2
            || landmarks.outputs() < 3
        {
            return Err(Refusal::Malformed("not the hand landmarker's two models"));
        }
        Ok(Self {
            detector,
            landmarks,
            scratch: Scratch::default(),
            anchors: anchors(),
            held: Vec::new(),
            since_search: SEARCH_EVERY,
            input: Vec::new(),
        })
    }

    /// Forgets the hands held, so the next frame searches the whole of it.
    pub fn forget(&mut self) {
        self.held.clear();
        self.since_search = SEARCH_EVERY;
    }

    /// The hands in one RGBA frame. `mirrored` says whether the frame is
    /// already as the person sees it.
    pub fn see(&mut self, rgba: &[u8], width: usize, height: usize, mirrored: bool) -> Vec<Hand> {
        if width == 0 || height == 0 || rgba.len() < width * height * 4 {
            return Vec::new();
        }
        self.since_search += 1;
        if self.held.len() < MOST && self.since_search >= SEARCH_EVERY {
            self.since_search = 0;
            for palm in self.palms(rgba, width, height) {
                let square = palm_square(&palm);
                let known = self
                    .held
                    .iter()
                    .any(|h| overlap_of_squares(h, &square) > 0.5);
                if !known && self.held.len() < MOST {
                    self.held.push(square);
                }
            }
        }
        let mut found = Vec::new();
        let mut next = Vec::new();
        for square in std::mem::take(&mut self.held) {
            let Some((hand, pixels)) = self.read_hand(rgba, width, height, square) else {
                continue;
            };
            let follow = hand_square(&pixels);
            if next
                .iter()
                .any(|n: &Square| overlap_of_squares(n, &follow) > 0.5)
            {
                continue;
            }
            next.push(follow);
            found.push(hand);
        }
        self.held = next;
        let (w, h) = (width as f32, height as f32);
        for hand in &mut found {
            for p in &mut hand.points {
                p[0] /= w;
                p[1] /= h;
                p[2] /= w;
                if !mirrored {
                    p[0] = 1.0 - p[0];
                }
            }
            if !mirrored {
                hand.right = !hand.right;
            }
        }
        found
    }

    /// Palms anywhere in the frame, in pixels, best first.
    pub fn palms(&mut self, rgba: &[u8], width: usize, height: usize) -> Vec<Palm> {
        // Letterbox: the frame fitted inside the square, centred.
        let scale = DETECTOR as f32 / width.max(height) as f32;
        let (fw, fh) = (width as f32 * scale, height as f32 * scale);
        let (ox, oy) = ((DETECTOR as f32 - fw) / 2.0, (DETECTOR as f32 - fh) / 2.0);
        self.input.clear();
        self.input.resize(DETECTOR * DETECTOR * 3, 0.0);
        for v in 0..DETECTOR {
            for u in 0..DETECTOR {
                let x = (u as f32 + 0.5 - ox) / scale - 0.5;
                let y = (v as f32 + 0.5 - oy) / scale - 0.5;
                if x < -0.5 || y < -0.5 || x > width as f32 - 0.5 || y > height as f32 - 0.5 {
                    continue;
                }
                let px = sample(rgba, width, height, x, y);
                self.input[(v * DETECTOR + u) * 3..][..3].copy_from_slice(&px);
            }
        }
        self.detector.run_all(&self.input, &mut self.scratch);
        let boxes = self.detector.output(&self.scratch, 0);
        let scores = self.detector.output(&self.scratch, 1);
        let back = |x: f32, y: f32| {
            [
                (x * DETECTOR as f32 - ox) / scale,
                (y * DETECTOR as f32 - oy) / scale,
            ]
        };
        let mut palms: Vec<Palm> = decode(boxes, scores, &self.anchors)
            .into_iter()
            .map(|p| {
                let c = back(p.box_[0], p.box_[1]);
                let mut keys = [[0.0; 2]; 7];
                for (k, key) in keys.iter_mut().enumerate() {
                    *key = back(p.keys[k][0], p.keys[k][1]);
                }
                Palm {
                    score: p.score,
                    box_: [
                        c[0],
                        c[1],
                        p.box_[2] * DETECTOR as f32 / scale,
                        p.box_[3] * DETECTOR as f32 / scale,
                    ],
                    keys,
                }
            })
            .collect();
        palms = weighted_suppression(palms);
        palms.truncate(MOST);
        palms
    }

    /// One hand inside `square`: the hand, and its points in pixels.
    fn read_hand(
        &mut self,
        rgba: &[u8],
        width: usize,
        height: usize,
        square: Square,
    ) -> Option<(Hand, [[f32; 3]; 21])> {
        let (sin, cos) = square.turn.sin_cos();
        let step = square.side / LANDMARKS as f32;
        self.input.clear();
        self.input.resize(LANDMARKS * LANDMARKS * 3, 0.0);
        for v in 0..LANDMARKS {
            for u in 0..LANDMARKS {
                let lx = (u as f32 + 0.5) * step - square.side / 2.0;
                let ly = (v as f32 + 0.5) * step - square.side / 2.0;
                let x = square.cx + lx * cos - ly * sin - 0.5;
                let y = square.cy + lx * sin + ly * cos - 0.5;
                if x < -0.5 || y < -0.5 || x > width as f32 - 0.5 || y > height as f32 - 0.5 {
                    continue;
                }
                let px = sample(rgba, width, height, x, y);
                self.input[(v * LANDMARKS + u) * 3..][..3].copy_from_slice(&px);
            }
        }
        self.landmarks.run_all(&self.input, &mut self.scratch);
        let presence = self.landmarks.output(&self.scratch, 1)[0];
        if presence < PRESENCE {
            return None;
        }
        // BELOW one half is the RIGHT hand, on the mirrored picture the model
        // expects: measured with the owner raising one hand at a time, the
        // right one alone read 0.71 and sat on the right of the mirrored
        // frame. MediaPipe's label file says the opposite of what the score's
        // name suggests, and nothing in the model's metadata tells which.
        let right = self.landmarks.output(&self.scratch, 2)[0] < 0.5;
        let raw = self.landmarks.output(&self.scratch, 0);
        let mut pixels = [[0.0f32; 3]; 21];
        for (k, p) in pixels.iter_mut().enumerate() {
            let lx = raw[3 * k] / LANDMARKS as f32 * square.side - square.side / 2.0;
            let ly = raw[3 * k + 1] / LANDMARKS as f32 * square.side - square.side / 2.0;
            *p = [
                square.cx + lx * cos - ly * sin,
                square.cy + lx * sin + ly * cos,
                raw[3 * k + 2] / LANDMARKS as f32 * square.side,
            ];
        }
        Some((
            Hand {
                points: pixels,
                presence,
                right,
            },
            pixels,
        ))
    }
}

/// A pixel of an RGBA frame, bilinear, in `0..1`.
fn sample(rgba: &[u8], width: usize, height: usize, x: f32, y: f32) -> [f32; 3] {
    let x = x.clamp(0.0, (width - 1) as f32);
    let y = y.clamp(0.0, (height - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |xx: usize, yy: usize, c: usize| f32::from(rgba[(yy * width + xx) * 4 + c]);
    let mut out = [0.0; 3];
    for (c, o) in out.iter_mut().enumerate() {
        let top = at(x0, y0, c) * (1.0 - fx) + at(x1, y0, c) * fx;
        let bottom = at(x0, y1, c) * (1.0 - fx) + at(x1, y1, c) * fx;
        *o = (top * (1.0 - fy) + bottom * fy) / 255.0;
    }
    out
}

/// `SsdAnchorsCalculator` with the palm detector's options, in its order:
/// row, column, then the anchors of that cell.
fn anchors() -> Vec<Anchor> {
    // Stride 8 is one layer (two anchors a cell); the three stride-16 layers
    // share one feature map (six a cell).
    let mut out = Vec::with_capacity(2016);
    for (stride, per_cell) in [(8usize, 2usize), (16, 6)] {
        let cells = DETECTOR.div_ceil(stride);
        for y in 0..cells {
            for x in 0..cells {
                for _ in 0..per_cell {
                    out.push(Anchor {
                        x: (x as f32 + 0.5) / cells as f32,
                        y: (y as f32 + 0.5) / cells as f32,
                    });
                }
            }
        }
    }
    out
}

/// The detector's raw answer as palms in `0..1` of the letterboxed square.
fn decode(boxes: &[f32], scores: &[f32], anchors: &[Anchor]) -> Vec<Palm> {
    let d = DETECTOR as f32;
    let mut out = Vec::new();
    for (n, anchor) in anchors.iter().enumerate() {
        let Some(&logit) = scores.get(n) else { break };
        let score = 1.0 / (1.0 + (-logit.clamp(-100.0, 100.0)).exp());
        if score < PALM_SCORE {
            continue;
        }
        let r = &boxes[n * 18..][..18];
        let mut keys = [[0.0; 2]; 7];
        for (k, key) in keys.iter_mut().enumerate() {
            *key = [r[4 + 2 * k] / d + anchor.x, r[5 + 2 * k] / d + anchor.y];
        }
        out.push(Palm {
            score,
            box_: [r[0] / d + anchor.x, r[1] / d + anchor.y, r[2] / d, r[3] / d],
            keys,
        });
    }
    out
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let (ax0, ay0, ax1, ay1) = (
        a[0] - a[2] / 2.0,
        a[1] - a[3] / 2.0,
        a[0] + a[2] / 2.0,
        a[1] + a[3] / 2.0,
    );
    let (bx0, by0, bx1, by1) = (
        b[0] - b[2] / 2.0,
        b[1] - b[3] / 2.0,
        b[0] + b[2] / 2.0,
        b[1] + b[3] / 2.0,
    );
    let w = (ax1.min(bx1) - ax0.max(bx0)).max(0.0);
    let h = (ay1.min(by1) - ay0.max(by0)).max(0.0);
    let inter = w * h;
    let union = a[2] * a[3] + b[2] * b[3] - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

/// `NonMaxSuppressionCalculator`, WEIGHTED: every palm that overlaps the best
/// one is averaged into it by score, instead of thrown away.
fn weighted_suppression(mut palms: Vec<Palm>) -> Vec<Palm> {
    palms.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut out = Vec::new();
    while let Some(best) = palms.first().copied() {
        let (group, rest): (Vec<Palm>, Vec<Palm>) = palms
            .into_iter()
            .partition(|p| iou(&best.box_, &p.box_) > SUPPRESSION);
        palms = rest;
        let total: f32 = group.iter().map(|p| p.score).sum();
        let mut merged = best;
        merged.box_ = [0.0; 4];
        merged.keys = [[0.0; 2]; 7];
        for p in &group {
            let w = p.score / total;
            for i in 0..4 {
                merged.box_[i] += p.box_[i] * w;
            }
            for k in 0..7 {
                merged.keys[k][0] += p.keys[k][0] * w;
                merged.keys[k][1] += p.keys[k][1] * w;
            }
        }
        out.push(merged);
    }
    out
}

/// The turn that makes `from → to` point straight up, in `-π..π`.
fn turn_between(from: [f32; 2], to: [f32; 2]) -> f32 {
    let r = std::f32::consts::FRAC_PI_2 - (-(to[1] - from[1])).atan2(to[0] - from[0]);
    r - std::f32::consts::TAU * ((r + std::f32::consts::PI) / std::f32::consts::TAU).floor()
}

/// `RectTransformationCalculator`: shift along the turned axes, square on the
/// long side, grow.
fn transform(cx: f32, cy: f32, w: f32, h: f32, turn: f32, shift_y: f32, grow: f32) -> Square {
    let (sin, cos) = turn.sin_cos();
    Square {
        cx: cx - h * shift_y * sin,
        cy: cy + h * shift_y * cos,
        side: w.max(h) * grow,
        turn,
    }
}

/// The square the landmark model reads a newly found palm in.
pub fn palm_square(palm: &Palm) -> Square {
    let turn = turn_between(palm.keys[0], palm.keys[2]);
    transform(
        palm.box_[0],
        palm.box_[1],
        palm.box_[2],
        palm.box_[3],
        turn,
        -0.5,
        2.6,
    )
}

/// The square next frame reads this hand in, from its points in pixels.
pub fn hand_square(points: &[[f32; 3]; 21]) -> Square {
    let turn = turn_between([points[0][0], points[0][1]], [points[9][0], points[9][1]]);
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &k in &ANCHORS {
        x0 = x0.min(points[k][0]);
        x1 = x1.max(points[k][0]);
        y0 = y0.min(points[k][1]);
        y1 = y1.max(points[k][1]);
    }
    let (ax, ay) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    // The same box, measured in the turned frame around the upright centre.
    let (sin, cos) = (-turn).sin_cos();
    let (mut rx0, mut ry0, mut rx1, mut ry1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &k in &ANCHORS {
        let (dx, dy) = (points[k][0] - ax, points[k][1] - ay);
        let (rx, ry) = (dx * cos - dy * sin, dx * sin + dy * cos);
        rx0 = rx0.min(rx);
        rx1 = rx1.max(rx);
        ry0 = ry0.min(ry);
        ry1 = ry1.max(ry);
    }
    let (mx, my) = ((rx0 + rx1) / 2.0, (ry0 + ry1) / 2.0);
    let (sin, cos) = turn.sin_cos();
    let cx = ax + mx * cos - my * sin;
    let cy = ay + mx * sin + my * cos;
    transform(cx, cy, rx1 - rx0, ry1 - ry0, turn, -0.1, 2.0)
}

/// How much two squares overlap, as upright boxes — enough to tell one hand
/// from two.
fn overlap_of_squares(a: &Square, b: &Square) -> f32 {
    iou(&[a.cx, a.cy, a.side, a.side], &[b.cx, b.cy, b.side, b.side])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_anchors_are_the_detectors_2016() {
        let a = anchors();
        assert_eq!(a.len(), 2016);
        assert!((a[0].x - 0.5 / 24.0).abs() < 1e-6);
        assert!(
            (a[1152].x - 0.5 / 12.0).abs() < 1e-6,
            "the stride-16 map starts at 1152"
        );
    }

    #[test]
    fn a_hand_pointing_up_is_not_turned() {
        assert!(turn_between([100.0, 200.0], [100.0, 100.0]).abs() < 1e-6);
        // Pointing right: a quarter turn.
        let r = turn_between([100.0, 100.0], [200.0, 100.0]);
        assert!((r - std::f32::consts::FRAC_PI_2).abs() < 1e-6, "{r}");
    }

    #[test]
    fn the_palm_square_grows_and_moves_toward_the_fingers() {
        let palm = Palm {
            score: 0.9,
            box_: [100.0, 100.0, 40.0, 40.0],
            keys: [
                [100.0, 120.0],
                [0.0; 2],
                [100.0, 80.0],
                [0.0; 2],
                [0.0; 2],
                [0.0; 2],
                [0.0; 2],
            ],
        };
        let s = palm_square(&palm);
        assert!((s.side - 104.0).abs() < 1e-3);
        // Up the hand is up the picture: the centre rises by half the height.
        assert!((s.cy - 80.0).abs() < 1e-3, "{s:?}");
        assert!((s.cx - 100.0).abs() < 1e-3);
    }

    #[test]
    fn overlapping_palms_are_one() {
        let p = |x: f32, s: f32| Palm {
            score: s,
            box_: [x, 0.5, 0.2, 0.2],
            keys: [[x, 0.5]; 7],
        };
        let out = weighted_suppression(vec![p(0.5, 0.9), p(0.52, 0.6), p(0.1, 0.8)]);
        assert_eq!(out.len(), 2);
        assert!(out[0].box_[0] > 0.5 && out[0].box_[0] < 0.52);
        assert_eq!(out[0].score, 0.9);
    }

    fn models() -> Option<(Vec<u8>, Vec<u8>)> {
        let dir = std::env::var_os("RONOCTUA_HAND_MODELS")?;
        let dir = std::path::Path::new(&dir);
        Some((
            std::fs::read(dir.join("hand_detector.tflite")).ok()?,
            std::fs::read(dir.join("hand_landmarks_detector.tflite")).ok()?,
        ))
    }

    /// The same synthetic frame `reference-hands.txt` was made from.
    fn synthetic(side: usize) -> Vec<f32> {
        let mut v = Vec::with_capacity(side * side * 3);
        for y in 0..side {
            for x in 0..side {
                for c in 0..3 {
                    v.push(((x * 7 + y * 13 + c * 29) % 97) as f32 / 96.0);
                }
            }
        }
        v
    }

    /// Both models' raw outputs against the TFLite runtime's
    /// (`ai-edge-litert` 2.1.6 on 2026-09-25, `tflite.Interpreter` with its
    /// default XNNPACK delegate): every `stride`-th value of every output.
    #[test]
    fn the_outputs_are_tflites_own() {
        let Some((detector, landmarks)) = models() else {
            eprintln!("RONOCTUA_HAND_MODELS unset: skipped");
            return;
        };
        let reference = include_str!("reference-hands.txt");
        let mut scratch = Scratch::default();
        let mut worst = 0.0f32;
        for (name, bytes, side) in [
            ("hand_detector", &detector, DETECTOR),
            ("hand_landmarks_detector", &landmarks, LANDMARKS),
        ] {
            let mut net = Net::read(bytes).expect("the model reads");
            net.set_threads(THREADS);
            let input = synthetic(side);
            let mut times = Vec::new();
            for _ in 0..20 {
                let t0 = std::time::Instant::now();
                net.run_all(&input, &mut scratch);
                times.push(t0.elapsed());
            }
            times.sort();
            eprintln!("{name}: fastest {:?}, median {:?}", times[0], times[10]);
            for line in reference
                .lines()
                .filter(|l| l.starts_with(&format!("{name} ")))
            {
                let mut words = line.split_whitespace().skip(1);
                let n: usize = words.next().unwrap().parse().unwrap();
                let stride: usize = words.next().unwrap().parse().unwrap();
                let ours = net.output(&scratch, n);
                for (k, want) in words.enumerate() {
                    let want: f32 = want.parse().unwrap();
                    let got = ours[k * stride];
                    let off = (got - want).abs() / want.abs().max(1.0);
                    worst = worst.max(off);
                    assert!(
                        off < 0.01,
                        "{name} output {n}[{}]: {got} against {want}",
                        k * stride
                    );
                }
            }
        }
        eprintln!("worst relative difference: {worst}");
    }

    #[test]
    fn no_hand_in_a_flat_frame() {
        let Some((detector, landmarks)) = models() else {
            return;
        };
        let mut hands = Hands::new(&detector, &landmarks).expect("both read");
        let frame = vec![128u8; 320 * 180 * 4];
        let t0 = std::time::Instant::now();
        assert!(hands.see(&frame, 320, 180, true).is_empty());
        eprintln!("one search: {:?}", t0.elapsed());
    }
}
