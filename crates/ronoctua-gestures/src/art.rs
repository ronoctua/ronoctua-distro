//! The gestures DRAWN — one small picture per row of [`crate::GESTURES`],
//! for the help (`LEADER h g`).
//!
//! # Drawn from one hand, not from thirteen files
//!
//! Every picture is the same hand — a palm and four fingers, each out or
//! folded, and a thumb out or tucked in — placed, turned and mirrored. A
//! gesture whose pose changes changes one line in [`scene`], and the picture
//! follows; thirteen hand-drawn files would be thirteen things to forget to
//! redraw. The poses are the ones [`crate::Reader`] reads, in the same words.
//!
//! # One ink
//!
//! Everything is `currentColor`, which the shell resolves to a token of the
//! theme (invariant 9): a picture has no colour of its own. The palm is the
//! same ink at a lower opacity, so the fingers read in front of it.
//!
//! # As the person sees it
//!
//! Mirrored, as a mirror shows the person: the RIGHT hand is on the right,
//! with its thumb toward the middle — which is how the camera's picture is
//! read too.

use std::fmt::Write as _;

/// The picture's size, in its own units: twice as wide as tall, which fits
/// two hands side by side.
pub const WIDTH: f32 = 200.0;
/// See [`WIDTH`].
pub const HEIGHT: f32 = 100.0;

/// A finger's width, and the arrows' — in the picture's units.
const STROKE: f32 = 7.0;

/// How a thumb is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Thumb {
    Out,
    In,
}

/// One hand of a picture.
#[derive(Debug, Clone, Copy)]
struct Hand {
    /// The wrist, in the picture's units.
    at: [f32; 2],
    /// Degrees, clockwise; `0` is fingers up.
    turn: f32,
    /// How big: `1` is a hand whose fingertips are 75 units above its wrist.
    scale: f32,
    /// The person's right hand (thumb toward the middle of the picture).
    right: bool,
    /// Index, middle, ring, little: out or folded.
    fingers: [bool; 4],
    thumb: Thumb,
    /// The fingers that are out fanned apart.
    spread: bool,
}

impl Hand {
    const fn new(at: [f32; 2], right: bool, fingers: [bool; 4], thumb: Thumb) -> Self {
        Self {
            at,
            turn: 0.0,
            scale: 1.0,
            right,
            fingers,
            thumb,
            spread: false,
        }
    }

    const fn spread(mut self) -> Self {
        self.spread = true;
        self
    }

    const fn turned(mut self, degrees: f32) -> Self {
        self.turn = degrees;
        self
    }

    const fn scaled(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// A point of the hand's own frame (a RIGHT hand, wrist at the origin,
    /// fingers up) placed in the picture.
    fn place(&self, p: [f32; 2]) -> [f32; 2] {
        let x = if self.right { p[0] } else { -p[0] };
        let (s, c) = self.turn.to_radians().sin_cos();
        [
            (x * c - p[1] * s).mul_add(self.scale, self.at[0]),
            (x * s + p[1] * c).mul_add(self.scale, self.at[1]),
        ]
    }

    fn draw(&self, out: &mut String) {
        // The knuckles, and each finger's length when out.
        let knuckles = [[-12.0, -38.0], [-4.0, -41.0], [4.0, -39.0], [11.0, -35.0]];
        let lengths = [30.0, 34.0, 31.0, 24.0];
        let fans: [f32; 4] = if self.spread {
            [-20.0, -6.0, 8.0, 22.0]
        } else {
            [-6.0, 0.0, 5.0, 10.0]
        };
        // THE PALM, a rounded shape filled at a lower opacity.
        let palm = [
            [-11.0, 0.0],
            [11.0, 0.0],
            [14.0, -33.0],
            [4.0, -40.0],
            [-14.0, -38.0],
            [-16.0, -14.0],
        ]
        .map(|p| self.place(p));
        let _ = write!(out, r#"<path d="M{:.1} {:.1}"#, palm[0][0], palm[0][1]);
        for p in &palm[1..] {
            let _ = write!(out, " L{:.1} {:.1}", p[0], p[1]);
        }
        let _ = write!(
            out,
            r#" Z" fill="currentColor" fill-opacity="0.45" stroke="currentColor" stroke-width="{:.1}" stroke-linejoin="round"/>"#,
            STROKE * 0.6 * self.scale
        );
        // THE FINGERS: out is a long stroke along its fan; folded, a short
        // one — the knuckle, which is what a fist shows.
        for f in 0..4 {
            let k = knuckles[f];
            let (s, c) = fans[f].to_radians().sin_cos();
            let len = if self.fingers[f] { lengths[f] } else { 7.0 };
            let tip = [k[0] + s * len, k[1] - c * len];
            self.stroke(out, k, tip);
        }
        // THE THUMB: out to the side, or tucked across the palm.
        let base = [-13.0, -12.0];
        let tip = match self.thumb {
            Thumb::Out => [-32.0, -30.0],
            Thumb::In => [-2.0, -24.0],
        };
        self.stroke(out, base, tip);
    }

    fn stroke(&self, out: &mut String, from: [f32; 2], to: [f32; 2]) {
        let (a, b) = (self.place(from), self.place(to));
        let _ = write!(
            out,
            r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="currentColor" stroke-width="{:.1}" stroke-linecap="round"/>"#,
            a[0],
            a[1],
            b[0],
            b[1],
            STROKE * self.scale
        );
    }
}

/// An arrow with a head at one end, or both.
fn arrow(out: &mut String, from: [f32; 2], to: [f32; 2], both: bool) {
    let _ = write!(
        out,
        r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="currentColor" stroke-width="{:.1}" stroke-linecap="round"/>"#,
        from[0],
        from[1],
        to[0],
        to[1],
        STROKE * 0.7
    );
    let head = |out: &mut String, tip: [f32; 2], back: [f32; 2]| {
        let (dx, dy) = (tip[0] - back[0], tip[1] - back[1]);
        let n = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (ux, uy) = (dx / n, dy / n);
        let size = 11.0;
        let left = [
            tip[0] - ux * size + uy * size * 0.7,
            tip[1] - uy * size - ux * size * 0.7,
        ];
        let right = [
            tip[0] - ux * size - uy * size * 0.7,
            tip[1] - uy * size + ux * size * 0.7,
        ];
        let _ = write!(
            out,
            r#"<path d="M{:.1} {:.1} L{:.1} {:.1} L{:.1} {:.1}" fill="none" stroke="currentColor" stroke-width="{:.1}" stroke-linecap="round" stroke-linejoin="round"/>"#,
            left[0],
            left[1],
            tip[0],
            tip[1],
            right[0],
            right[1],
            STROKE * 0.7
        );
    };
    head(out, to, from);
    if both {
        head(out, from, to);
    }
}

/// An arrow: from, to, and whether it has a head at both ends.
type Arrow = ([f32; 2], [f32; 2], bool);

/// What a picture holds: its hands, and its arrows.
fn scene(id: &str) -> Option<(Vec<Hand>, Vec<Arrow>)> {
    use Thumb::{In, Out};
    const POINTER: [bool; 4] = [true, true, false, false];
    const INDEX: [bool; 4] = [true, false, false, false];
    const OPEN: [bool; 4] = [true; 4];
    const MIDDLE: [bool; 4] = [false, true, false, false];
    const FIST: [bool; 4] = [false; 4];
    let y = 92.0;
    Some(match id {
        "point" => (
            vec![Hand::new([100.0, y], true, POINTER, In).spread()],
            vec![],
        ),
        "click" => (
            vec![
                Hand::new([58.0, y], false, OPEN, Out).spread(),
                Hand::new([142.0, y], true, POINTER, In).spread(),
            ],
            vec![],
        ),
        "right-click" => (
            vec![
                Hand::new([58.0, y], false, [false, false, false, true], Out),
                Hand::new([142.0, y], true, POINTER, In).spread(),
            ],
            vec![],
        ),
        "scroll" => (
            vec![Hand::new([85.0, y], true, POINTER, Out).spread()],
            vec![([150.0, 22.0], [150.0, 82.0], true)],
        ),
        "arrows" => (
            vec![
                Hand::new([68.0, y], false, INDEX, In),
                Hand::new([132.0, y], true, INDEX, In),
            ],
            vec![([100.0, 58.0], [100.0, 14.0], false)],
        ),
        "escape" => (
            vec![
                // Wide apart and leaning in, so the two index fingers meet
                // over the middle and the palms stay out of the X.
                Hand::new([68.0, 98.0], false, INDEX, In).turned(20.0),
                Hand::new([132.0, 98.0], true, INDEX, In).turned(-20.0),
            ],
            vec![],
        ),
        "focus" => (
            vec![Hand::new([75.0, y], true, OPEN, Out).spread()],
            vec![([128.0, 52.0], [188.0, 52.0], true)],
        ),
        "workspace" => (
            vec![
                Hand::new([58.0, y], false, POINTER, Out).spread(),
                Hand::new([142.0, y], true, POINTER, Out).spread(),
            ],
            vec![([100.0, 22.0], [100.0, 82.0], true)],
        ),
        "overview" => (
            vec![
                Hand::new([58.0, y], false, OPEN, Out).spread(),
                Hand::new([142.0, y], true, OPEN, Out).spread(),
            ],
            vec![],
        ),
        "keyboard" => (
            vec![
                Hand::new([75.0, 32.0], true, OPEN, Out)
                    .turned(90.0)
                    .scaled(0.55),
                Hand::new([75.0, 76.0], true, OPEN, Out)
                    .turned(90.0)
                    .scaled(0.55),
            ],
            vec![],
        ),
        "player" => (
            vec![Hand::new([100.0, y], true, [true, false, false, true], In).spread()],
            vec![],
        ),
        "close" => (
            vec![
                Hand::new([62.0, y], false, MIDDLE, In),
                Hand::new([138.0, y], true, MIDDLE, In),
            ],
            vec![],
        ),
        "off" => (
            vec![
                // The forearms are drawn as the hands' wrists reaching across:
                // each fist sits on the OTHER side.
                Hand::new([72.0, 52.0], true, FIST, In)
                    .turned(-35.0)
                    .scaled(0.65),
                Hand::new([128.0, 52.0], false, FIST, In)
                    .turned(35.0)
                    .scaled(0.65),
            ],
            vec![],
        ),
        _ => return None,
    })
}

/// The picture of one of [`crate::GESTURES`], as an SVG document in
/// `currentColor`; `None` for a name that is not one.
#[must_use]
pub fn picture(id: &str) -> Option<String> {
    let (hands, arrows) = scene(id)?;
    // THE DOCUMENT DECLARES FOUR TIMES ITS UNITS: a rasteriser that never
    // enlarges (the shell's `Fit::Natural`) can then draw it up to that size
    // before it would have to.
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {HEIGHT}" width="{}" height="{}">"#,
        WIDTH * 4.0,
        HEIGHT * 4.0
    );
    if id == "off" {
        // THE CROSSED FOREARMS, under the fists: each from a bottom corner to
        // the fist on the other side.
        for (from, to) in [
            ([40.0f32, 100.0f32], [132.0f32, 40.0f32]),
            ([160.0, 100.0], [68.0, 40.0]),
        ] {
            let _ = write!(
                out,
                r#"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="currentColor" stroke-opacity="0.6" stroke-width="{:.1}" stroke-linecap="round"/>"#,
                from[0],
                from[1],
                to[0],
                to[1],
                STROKE * 2.2
            );
        }
    }
    for h in &hands {
        h.draw(&mut out);
    }
    for (from, to, both) in arrows {
        arrow(&mut out, from, to, both);
    }
    out.push_str("</svg>");
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gesture_has_a_picture_in_the_theme_s_ink() {
        for g in crate::GESTURES {
            let svg = picture(g).unwrap_or_else(|| panic!("{g} has no picture"));
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
            assert!(svg.contains("currentColor"), "{g} has an ink of its own");
            assert!(!svg.contains('#'), "{g} names a colour");
        }
        assert!(picture("not-a-gesture").is_none());
    }

    #[test]
    fn a_hand_stays_inside_its_picture() {
        for g in crate::GESTURES {
            let (hands, _) = scene(g).unwrap();
            for h in hands {
                for p in [[0.0, 0.0], [-32.0, -30.0], [-4.0, -75.0], [14.0, -33.0]] {
                    let [x, y] = h.place(p);
                    assert!(
                        (-2.0..=WIDTH + 2.0).contains(&x) && (-2.0..=HEIGHT + 2.0).contains(&y),
                        "{g}: ({x}, {y}) is outside"
                    );
                }
            }
        }
    }
}
