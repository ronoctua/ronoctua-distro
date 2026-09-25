//! What the hands mean.
//!
//! [`Reader::read`] takes one camera reading — up to two hands of MediaPipe's
//! 21 points, as the person sees them (mirrored) — and answers what the
//! person did: move the pointer, press or let go of a button, scroll, an
//! arrow, or one of the commands. It never acts; the footer does.
//!
//! # Three kinds of gesture, three rules
//!
//! - **Continuous** (the pointer, the scroll): answered on every reading
//!   while the pose lasts.
//! - **Held** (the left button): down while the OTHER hand stays open beside
//!   a pointing one, up when it closes — each after [`MODIFIER_READINGS`]
//!   readings in a row, or one frame the model guessed wrong would be a click.
//! - **Commands**: fired once, after the pose has been held for its DWELL,
//!   and never again until the pose is let go. A command that closes a window
//!   or turns gestures off dwells longer ([`DWELL_DESTRUCTIVE`]), because a
//!   hand passing through the pose on its way to another is the misfire that
//!   matters. After any command a [`COOLDOWN`] passes.
//!
//! # Two hands first, and most commands ARE two hands
//!
//! A pose of both hands is looked for before any pose of one. The owner moved
//! the commands there to cut false positives: a hand doing something else
//! makes one of these poses by chance far more often than two do together.
//! Only the pointer, the scroll, the swipe and the player are one hand, and
//! the pointer's buttons are its OTHER hand.
//!
//! # Units
//!
//! Every distance is measured in PALMS — wrist to the middle finger's knuckle
//! — so a hand near the camera and one far from it are the same hand. The
//! points arrive in `0..1` of their own side of the frame, and [`Reading`]
//! carries the frame's aspect to put `x` and `y` in one unit first.

pub mod art;

use std::collections::VecDeque;

/// A thumb this far from the two raised fingers is held OUT: with it, the
/// pair scrolls; without it, the pair is the pointer. Measured on the owner's
/// hand: a thumb held out beside the fingers sits at 0.45–0.6.
pub const SCROLL_THUMB: f32 = 0.4;
/// Milliseconds a swipe survives the hand leaving the pose: a thumb that
/// folds for a frame while the hand sweeps is the same sweep.
pub const SWIPE_GAP: u64 = 250;
/// Readings in a row the OTHER hand has to be open before the button goes
/// down, and closed before it comes up: one reading is one frame of a model
/// that sometimes guesses — measured, a click in the middle of pointing.
pub const MODIFIER_READINGS: u8 = 2;
/// Milliseconds after a swipe in which a swipe the OTHER way is the hand
/// coming back, and moves nothing — measured, every sweep of the owner's was
/// followed by its return 1.3 to 2.0 seconds later, told to come back slowly.
/// The price is that a deliberate swipe back waits this long.
pub const SWIPE_RETURN: u64 = 2500;
/// Index tip to middle tip, in palms, above which two raised fingers START
/// the pointer. Measured on the owner's hands: pointing on purpose, 0.41 to
/// 0.95; a hand at rest that happened into the pose, 0.26 to 0.44. Once the
/// pointer is on the distance no longer matters — a drag closes the fingers.
pub const POINTER_GAP: f32 = 0.45;
/// Degrees a hand may lean from upright (wrist to the middle knuckle) and
/// still be RAISED — what the ten fingers, the two middle fingers and the
/// rock sign ask. Measured: the owner made them at 16° at most; hands resting
/// in a lap lie near 90°, and they had opened the overview five times.
pub const UPRIGHT: f32 = 40.0;
/// Crossed arms: each hand leans at most this much, and its wrist is in the
/// top [`CROSS_HIGH`] of the picture. Made on purpose, 24–65° and 0.34–0.53;
/// two hands resting over each other in a lap had turned gestures off.
pub const CROSS_LEAN: f32 = 80.0;
/// See [`CROSS_LEAN`].
pub const CROSS_HIGH: f32 = 0.7;
/// A hand with a point this close to the picture's edge is cut off, and the
/// part the model cannot see it GUESSES: it is not read at all.
pub const EDGE: f32 = 0.01;
/// Milliseconds a command's pose is held before it fires.
pub const DWELL: u64 = 350;
/// The same, for a command that throws something away (closing a window,
/// turning gestures off).
pub const DWELL_DESTRUCTIVE: u64 = 600;
/// A right click's dwell: a click is quick, but a hand passing through the
/// pose on its way to another is not a click.
pub const DWELL_CLICK: u64 = 150;
/// An arrow's: two index fingers are deliberate, and an arrow is cheap.
pub const DWELL_ARROW: u64 = 250;
/// Milliseconds after a command in which no other fires.
pub const COOLDOWN: u64 = 700;
/// How far an open hand travels, in palms, to move the focus.
pub const SWIPE: f32 = 1.2;
/// A swipe that took longer than this is a hand drifting, not a swipe.
pub const SWIPE_WITHIN: u64 = 600;
/// Lines of scroll per palm the hand travels.
pub const SCROLL_PER_PALM: f32 = 6.0;
/// Hand speed, in palms a second, below which a scroll hand is still.
pub const SCROLL_STILL: f32 = 0.6;
/// Milliseconds the pointer holds still after a button changes, so the jolt
/// of the other hand opening does not drag what it clicks.
pub const CLICK_STILL: u64 = 120;
/// Milliseconds without the pointer's hand before its button is let go.
pub const LOST_AFTER: u64 = 300;

/// One hand, as the studio sends it.
#[derive(Debug, Clone, PartialEq)]
pub struct Hand {
    /// MediaPipe's 21 points, `[x, y, z]`, `x` and `y` in `0..1` of the frame
    /// as the person sees it.
    pub points: [[f32; 3]; 21],
    /// The person's right hand.
    pub right: bool,
}

/// One camera reading.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// Milliseconds, from any clock that only goes forward.
    pub at: u64,
    /// The frame's width over its height.
    pub aspect: f32,
    /// Up to two hands.
    pub hands: Vec<Hand>,
}

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// The primary.
    Left,
    /// The secondary.
    Right,
}

/// A direction, as the person sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Up.
    Up,
    /// Toward the person's right.
    Right,
    /// Down.
    Down,
    /// Toward the person's left.
    Left,
}

/// What the person did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// The pointer goes here: `0..1` across and down the screen.
    Point {
        /// Across.
        x: f32,
        /// Down.
        y: f32,
    },
    /// A button goes down.
    Press(Button),
    /// A button comes up.
    Release(Button),
    /// Lines of scroll, positive downward.
    Scroll(f32),
    /// An arrow key.
    Arrow(Direction),
    /// `Esc`.
    Escape,
    /// Move the focus to the window that way.
    Focus(Direction),
    /// The workspace above (`Up`) or below (`Down`).
    Workspace(Direction),
    /// Every workspace at once.
    Overview,
    /// The on-screen keyboard.
    Keyboard,
    /// The player.
    Player,
    /// Close the window in focus.
    Close,
    /// Turn gestures off.
    Off,
}

/// Every gesture the reader answers, in the order a manual lists them.
///
/// THE HELP IS GENERATED FROM THIS (`LEADER h g`, the shell's `manual.rs`),
/// and [`gesture_of`] is an exhaustive `match`: an event added without a row
/// here does not compile, and a row no event comes from fails a test. A
/// manual written by hand is the second copy of the table, and it lies the
/// day a gesture changes.
pub const GESTURES: &[&str] = &[
    "point",
    "click",
    "right-click",
    "scroll",
    "arrows",
    "escape",
    "focus",
    "workspace",
    "overview",
    "keyboard",
    "player",
    "close",
    "off",
];

/// The gesture an event comes from — one of [`GESTURES`].
#[must_use]
pub const fn gesture_of(e: Event) -> &'static str {
    match e {
        Event::Point { .. } => "point",
        Event::Press(Button::Left) | Event::Release(Button::Left) => "click",
        Event::Press(Button::Right) | Event::Release(Button::Right) => "right-click",
        Event::Scroll(_) => "scroll",
        Event::Arrow(_) => "arrows",
        Event::Escape => "escape",
        Event::Focus(_) => "focus",
        Event::Workspace(_) => "workspace",
        Event::Overview => "overview",
        Event::Keyboard => "keyboard",
        Event::Player => "player",
        Event::Close => "close",
        Event::Off => "off",
    }
}

/// The part of the camera's picture the screen is mapped onto, in `0..1` of
/// the frame as the person sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// Left edge.
    pub left: f32,
    /// Top edge.
    pub top: f32,
    /// Right edge.
    pub right: f32,
    /// Bottom edge.
    pub bottom: f32,
}

impl Default for Area {
    /// The middle of the picture, a little high: where a hand pointing at a
    /// screen under the camera sits, with room around it to reach the edges.
    fn default() -> Self {
        Self {
            left: 0.2,
            top: 0.15,
            right: 0.8,
            bottom: 0.7,
        }
    }
}

impl Area {
    /// The smallest area that holds four corners — what calibration found.
    #[must_use]
    pub fn around(corners: &[[f32; 2]]) -> Option<Self> {
        if corners.len() < 2 {
            return None;
        }
        let (mut l, mut t, mut r, mut b) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for c in corners {
            l = l.min(c[0]);
            r = r.max(c[0]);
            t = t.min(c[1]);
            b = b.max(c[1]);
        }
        (r - l > 0.05 && b - t > 0.05).then_some(Self {
            left: l,
            top: t,
            right: r,
            bottom: b,
        })
    }

    fn map(&self, p: [f32; 2]) -> [f32; 2] {
        [
            ((p[0] - self.left) / (self.right - self.left).max(1e-3)).clamp(0.0, 1.0),
            ((p[1] - self.top) / (self.bottom - self.top).max(1e-3)).clamp(0.0, 1.0),
        ]
    }
}

/// A finger's state; `Unsure` is neither, and matches no pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Out,
    In,
    Unsure,
}

/// What one hand shows, measured once per reading.
#[derive(Debug, Clone, Copy)]
struct Shape {
    thumb: State,
    /// Index, middle, ring, little.
    fingers: [State; 4],
    /// Wrist to the middle knuckle: the unit.
    palm: f32,
    /// The palm's centre.
    centre: [f32; 2],
    /// Thumb tip to the nearest joint of the two raised fingers, in palms.
    to_pair: f32,
    /// Where the index finger points (knuckle to tip).
    index_points: [f32; 2],
    /// Index tip to middle tip, in palms.
    pair_gap: f32,
    /// Degrees from upright, wrist to the middle knuckle, either way.
    lean: f32,
    /// The wrist's height in the picture, `0` the top.
    wrist_y: f32,
    /// The index knuckle, in the frame's height unit like every other point
    /// here — the segment from it along `index_points` is the finger.
    index_base: [f32; 2],
    /// Where the index and middle point together.
    pair_points: [f32; 2],
    /// Where the middle finger points (knuckle to tip): up is an open hand
    /// RAISED, and not one lying on a desk.
    middle_points: [f32; 2],
    /// The narrowest gap between two neighbouring fingertips, in palms.
    spread: f32,
    /// The index tip.
    tip: [f32; 2],
    right: bool,
}

/// A pose of one hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pose {
    /// Index and middle out, the thumb folded: the POINTER.
    Pointer,
    /// Index and middle out and the thumb out: scroll, and — both hands —
    /// the workspace.
    Three,
    /// The index alone: an arrow, with both hands.
    Point,
    /// The middle finger alone: closing a window, with both hands.
    Middle,
    /// Index and little.
    Rock,
    /// Thumb and little, the "hang loose": the right button, beside a pointer.
    HangLoose,
    /// All five out.
    Open,
    Other,
}

/// A command candidate the dwell machine holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Candidate {
    Escape,
    Close,
    Off,
    Workspace(Direction),
    Arrow(Direction),
    Overview,
    Keyboard,
    Player,
    RightClick,
}

impl Candidate {
    fn dwell(self) -> u64 {
        match self {
            Self::Close | Self::Off => DWELL_DESTRUCTIVE,
            Self::RightClick => DWELL_CLICK,
            Self::Arrow(_) => DWELL_ARROW,
            _ => DWELL,
        }
    }

    fn events(self) -> Vec<Event> {
        match self {
            Self::Escape => vec![Event::Escape],
            Self::Close => vec![Event::Close],
            Self::Off => vec![Event::Off],
            Self::Workspace(d) => vec![Event::Workspace(d)],
            Self::Arrow(d) => vec![Event::Arrow(d)],
            Self::Overview => vec![Event::Overview],
            Self::Keyboard => vec![Event::Keyboard],
            Self::Player => vec![Event::Player],
            Self::RightClick => vec![Event::Press(Button::Right), Event::Release(Button::Right)],
        }
    }
}

/// The One-Euro filter (Casiez, Roussel and Vogel, 2012): smooth when the
/// hand is still, quick when it moves — the trade a fixed average cannot make.
#[derive(Debug, Clone, Default)]
struct OneEuro {
    last: Option<(u64, f32, f32)>,
}

impl OneEuro {
    const MIN_CUTOFF: f32 = 1.0;
    const BETA: f32 = 6.0;
    const D_CUTOFF: f32 = 1.0;

    fn alpha(cutoff: f32, dt: f32) -> f32 {
        let tau = 1.0 / (std::f32::consts::TAU * cutoff);
        1.0 / (1.0 + tau / dt)
    }

    fn take(&mut self, at: u64, x: f32) -> f32 {
        let Some((then, prev, dprev)) = self.last else {
            self.last = Some((at, x, 0.0));
            return x;
        };
        let dt = ((at.saturating_sub(then)) as f32 / 1000.0).max(1e-3);
        let dx = (x - prev) / dt;
        let d = dprev + Self::alpha(Self::D_CUTOFF, dt) * (dx - dprev);
        let cutoff = Self::MIN_CUTOFF + Self::BETA * d.abs();
        let out = prev + Self::alpha(cutoff, dt) * (x - prev);
        self.last = Some((at, out, d));
        out
    }

    fn reset(&mut self) {
        self.last = None;
    }
}

/// The reader: every piece of state a gesture needs between two readings.
#[derive(Debug, Clone, Default)]
pub struct Reader {
    area: Area,
    /// When a raised open hand was last seen, for [`SWIPE_GAP`].
    swipe_seen: u64,
    /// The last swipe, and when, for [`SWIPE_RETURN`].
    swiped: Option<(Direction, u64)>,
    /// Readings in a row the other hand has been open, and closed.
    opened: u8,
    closed: u8,
    /// The command pose held now, since when, and whether it already fired.
    holding: Option<(Candidate, u64, bool)>,
    quiet_until: u64,
    /// The left button is down.
    pressed: bool,
    /// A pointer is on, and which hand it is (`true` the right).
    pointing: Option<bool>,
    /// Readings in a row a pointer pose has been wide enough to start.
    starting: u8,
    still_until: u64,
    pointer_seen: u64,
    raw: Option<[f32; 2]>,
    filter: (OneEuro, OneEuro),
    scroll_from: Option<(u64, f32)>,
    swipe: VecDeque<(u64, [f32; 2])>,
    swipe_palm: f32,
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn dist3(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cos3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
        * (b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt();
    if n <= 0.0 { 0.0 } else { dot / n }
}

fn opposite(d: Direction) -> Direction {
    match d {
        Direction::Up => Direction::Down,
        Direction::Down => Direction::Up,
        Direction::Left => Direction::Right,
        Direction::Right => Direction::Left,
    }
}

/// Where a vector points, by its larger component.
fn direction_of(v: [f32; 2]) -> Direction {
    if v[0].abs() > v[1].abs() {
        if v[0] > 0.0 {
            Direction::Right
        } else {
            Direction::Left
        }
    } else if v[1] > 0.0 {
        Direction::Down
    } else {
        Direction::Up
    }
}

impl Shape {
    fn of(hand: &Hand, aspect: f32) -> Self {
        // One unit: the frame's HEIGHT. `z` comes in the width's unit.
        let p: Vec<[f32; 3]> = hand
            .points
            .iter()
            .map(|q| [q[0] * aspect, q[1], q[2] * aspect])
            .collect();
        let flat = |i: usize| [p[i][0], p[i][1]];
        let palm = dist3(p[0], p[9]).max(1e-4);
        let finger = |mcp: usize| {
            let (pip, tip) = (mcp + 1, mcp + 3);
            let straight = cos3(sub3(p[pip], p[mcp]), sub3(p[tip], p[pip]));
            let reach = dist3(p[0], p[tip]) / dist3(p[0], p[pip]).max(1e-4);
            if straight > 0.5 && reach > 1.1 {
                State::Out
            } else if reach < 1.0 || straight < 0.0 {
                State::In
            } else {
                State::Unsure
            }
        };
        let thumb = {
            let away = dist3(p[4], p[9]) / dist3(p[3], p[9]).max(1e-4);
            let wide = dist3(p[4], p[5]) / palm;
            if away > 1.05 && wide > 0.55 {
                State::Out
            } else if wide < 0.45 {
                State::In
            } else {
                State::Unsure
            }
        };
        let centre = {
            let k = [0, 5, 9, 13, 17];
            let n = k.len() as f32;
            [
                k.iter().map(|i| p[*i][0]).sum::<f32>() / n,
                k.iter().map(|i| p[*i][1]).sum::<f32>() / n,
            ]
        };
        let tips = [8, 12, 16, 20];
        let spread = tips
            .windows(2)
            .map(|w| dist(flat(w[0]), flat(w[1])) / palm)
            .fold(f32::MAX, f32::min);
        let to_pair = [6, 7, 8, 10, 11, 12]
            .iter()
            .map(|i| dist3(p[4], p[*i]) / palm)
            .fold(f32::MAX, f32::min);
        let mid = |a: usize, b: usize| [(p[a][0] + p[b][0]) / 2.0, (p[a][1] + p[b][1]) / 2.0];
        let base = mid(5, 9);
        let ends = mid(8, 12);
        Self {
            thumb,
            fingers: [finger(5), finger(9), finger(13), finger(17)],
            palm,
            centre,
            to_pair,
            index_points: [p[8][0] - p[5][0], p[8][1] - p[5][1]],
            pair_gap: dist3(p[8], p[12]) / palm,
            lean: (p[9][0] - p[0][0])
                .atan2(-(p[9][1] - p[0][1]))
                .to_degrees()
                .abs(),
            wrist_y: hand.points[0][1],
            index_base: flat(5),
            pair_points: [ends[0] - base[0], ends[1] - base[1]],
            middle_points: [p[12][0] - p[9][0], p[12][1] - p[9][1]],
            spread,
            // Back to the frame's own units for the pointer.
            tip: [hand.points[8][0], hand.points[8][1]],
            right: hand.right,
        }
    }

    fn pose(&self) -> Pose {
        use State::{In, Out};
        let f = self.fingers;
        let folded = |s: State| s != Out;
        match f {
            [Out, Out, Out, Out] if self.thumb == Out => Pose::Open,
            [Out, In, In, Out] => Pose::Rock,
            [a, b, c, Out] if folded(a) && folded(b) && folded(c) && self.thumb == Out => {
                Pose::HangLoose
            }
            [In, Out, In, In] => Pose::Middle,
            // Folded, or not sure: a hand pointing DOWN reads its two folded
            // fingers as unsure, measured, and the pose was lost. The LITTLE
            // finger any way at all: a hand pointing to the side reads it
            // out, measured, and no other pose has this shape.
            [Out, Out, a, _] if folded(a) => {
                if self.thumb == Out && self.to_pair > SCROLL_THUMB {
                    Pose::Three
                } else {
                    Pose::Pointer
                }
            }
            [Out, b, c, d] if b == In && c == In && folded(d) => Pose::Point,
            _ => Pose::Other,
        }
    }

    /// Four fingers out and apart, the thumb however it falls — it folds for a
    /// frame while a hand moves, measured.
    fn upright(&self) -> bool {
        self.lean <= UPRIGHT
    }

    fn open(&self) -> bool {
        self.fingers.iter().all(|f| *f == State::Out) && self.spread > 0.2
    }
}

impl Reader {
    /// A reader mapping the default [`Area`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Maps the screen onto `area` of the picture from now on.
    pub fn set_area(&mut self, area: Area) {
        self.area = area;
    }

    /// Where the pointing finger is in the PICTURE (not the screen), for
    /// calibration: `None` while no hand points.
    #[must_use]
    pub fn pointing_at(&self) -> Option<[f32; 2]> {
        self.raw
    }

    /// Forgets every pose in progress; what was held comes back as releases.
    pub fn let_go(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        if self.pressed {
            out.push(Event::Release(Button::Left));
        }
        *self = Self {
            area: self.area,
            ..Self::default()
        };
        out
    }

    /// One reading, and what it meant.
    pub fn read(&mut self, reading: &Reading) -> Vec<Event> {
        let now = reading.at;
        let shapes: Vec<Shape> = reading
            .hands
            .iter()
            .filter(|h| whole(h))
            .map(|h| Shape::of(h, reading.aspect))
            .collect();
        let mut out = Vec::new();

        // Both hands first.
        let both = if let [a, b] = shapes.as_slice() {
            both_hands(a, b)
        } else {
            None
        };
        if let Some(candidate) = both {
            // A pose of two hands owns them: whatever one hand held goes.
            out.extend(self.release_pointer());
            self.scroll_from = None;
            self.swipe.clear();
            self.dwell(Some(candidate), now, &mut out);
            return out;
        }

        // THE POINTER: index and middle out, the thumb folded — the right hand
        // when both are pointers. The OTHER hand is the pointer's buttons:
        // open, the left one is down for as long as it stays open; the hang
        // loose, one right click.
        // Once pointing, a thumb that slips out for a reading (the pose reads
        // as a scroll) is the same pointer.
        let pointing = self.pointing;
        let at = shapes
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                s.pose() == Pose::Pointer || (pointing == Some(s.right) && s.pose() == Pose::Three)
            })
            .max_by_key(|(_, s)| s.right)
            .map(|(i, _)| i);
        // AND IT STARTS ONLY WIDE, twice in a row (see `POINTER_GAP`).
        let at = match (at, pointing) {
            (Some(i), Some(_)) => Some(i),
            (Some(i), None) => {
                if shapes[i].pair_gap >= POINTER_GAP {
                    self.starting = self.starting.saturating_add(1);
                } else {
                    self.starting = 0;
                }
                (self.starting >= MODIFIER_READINGS).then_some(i)
            }
            (None, _) => {
                self.starting = 0;
                if now.saturating_sub(self.pointer_seen) > LOST_AFTER {
                    self.pointing = None;
                }
                None
            }
        };
        if let Some(i) = at {
            self.pointing = Some(shapes[i].right);
        }
        let mut candidate = None;
        match at {
            Some(i) => {
                self.pointer_seen = now;
                let other = shapes
                    .iter()
                    .enumerate()
                    .find(|(j, _)| *j != i)
                    .map(|(_, s)| s);
                self.buttons(other, now, &mut out);
                if other.is_some_and(|o| o.pose() == Pose::HangLoose) {
                    candidate = Some(Candidate::RightClick);
                }
                self.point(&shapes[i], now, &mut out);
            }
            None => {
                self.raw = None;
                if self.pressed && now.saturating_sub(self.pointer_seen) > LOST_AFTER {
                    out.extend(self.release_pointer());
                }
            }
        }

        // One-hand commands, the first hand that shows one.
        if candidate.is_none() {
            candidate = shapes
                .iter()
                .find(|s| s.pose() == Pose::Rock && s.upright())
                .map(|_| Candidate::Player);
        }
        self.dwell(candidate, now, &mut out);

        // Scroll, and the swipe — neither while a hand points: the other hand
        // is then its buttons, and an open one moving is not a swipe.
        let up = |v: [f32; 2]| v[1] < -v[0].abs();
        let three = shapes
            .iter()
            .find(|s| at.is_none() && s.pose() == Pose::Three && up(s.pair_points));
        match three {
            Some(s) => self.scroll(s, now, &mut out),
            None => self.scroll_from = None,
        }
        // A RAISED open hand: pointing up, not lying on a desk.
        let open = shapes
            .iter()
            .find(|s| at.is_none() && s.open() && up(s.middle_points));
        match open {
            Some(s) => {
                self.swipe_seen = now;
                self.swipe(s, now, &mut out);
            }
            None if now.saturating_sub(self.swipe_seen) > SWIPE_GAP => self.swipe.clear(),
            None => {}
        }
        out
    }

    fn release_pointer(&mut self) -> Vec<Event> {
        self.raw = None;
        self.filter.0.reset();
        self.filter.1.reset();
        self.opened = 0;
        self.closed = 0;
        if std::mem::take(&mut self.pressed) {
            vec![Event::Release(Button::Left)]
        } else {
            Vec::new()
        }
    }

    /// The left button, from the other hand: down after it has been open
    /// [`MODIFIER_READINGS`] readings, up after it has been anything else as
    /// many.
    fn buttons(&mut self, other: Option<&Shape>, now: u64, out: &mut Vec<Event>) {
        if other.is_some_and(Shape::open) {
            self.opened = self.opened.saturating_add(1);
            self.closed = 0;
        } else {
            self.closed = self.closed.saturating_add(1);
            self.opened = 0;
        }
        if !self.pressed && self.opened >= MODIFIER_READINGS {
            self.pressed = true;
            self.still_until = now + CLICK_STILL;
            out.push(Event::Press(Button::Left));
        } else if self.pressed && self.closed >= MODIFIER_READINGS {
            self.pressed = false;
            self.still_until = now + CLICK_STILL;
            out.push(Event::Release(Button::Left));
        }
    }

    fn point(&mut self, s: &Shape, now: u64, out: &mut Vec<Event>) {
        let raw = [
            self.filter.0.take(now, s.tip[0]),
            self.filter.1.take(now, s.tip[1]),
        ];
        self.raw = Some(raw);
        if now < self.still_until {
            return;
        }
        let [x, y] = self.area.map(raw);
        out.push(Event::Point { x, y });
    }

    fn dwell(&mut self, candidate: Option<Candidate>, now: u64, out: &mut Vec<Event>) {
        let Some(c) = candidate else {
            self.holding = None;
            return;
        };
        match &mut self.holding {
            Some((held, since, fired)) if *held == c => {
                if !*fired && now.saturating_sub(*since) >= c.dwell() && now >= self.quiet_until {
                    *fired = true;
                    self.quiet_until = now + COOLDOWN;
                    out.extend(c.events());
                }
            }
            _ => self.holding = Some((c, now, false)),
        }
    }

    fn scroll(&mut self, s: &Shape, now: u64, out: &mut Vec<Event>) {
        let y = s.centre[1];
        let Some((then, from)) = self.scroll_from else {
            self.scroll_from = Some((now, y));
            return;
        };
        let dt = (now.saturating_sub(then) as f32 / 1000.0).max(1e-3);
        let palms = (y - from) / s.palm;
        self.scroll_from = Some((now, y));
        if (palms / dt).abs() < SCROLL_STILL {
            return;
        }
        out.push(Event::Scroll(palms * SCROLL_PER_PALM));
    }

    fn swipe(&mut self, s: &Shape, now: u64, out: &mut Vec<Event>) {
        self.swipe.push_back((now, s.centre));
        self.swipe_palm = s.palm;
        while self
            .swipe
            .front()
            .is_some_and(|(t, _)| now.saturating_sub(*t) > SWIPE_WITHIN)
        {
            self.swipe.pop_front();
        }
        let Some(&(_, from)) = self.swipe.front() else {
            return;
        };
        let moved = [s.centre[0] - from[0], s.centre[1] - from[1]];
        if dist(moved, [0.0, 0.0]) / self.swipe_palm >= SWIPE {
            let way = direction_of(moved);
            self.swipe.clear();
            let coming_back = self.swiped.is_some_and(|(last, at)| {
                opposite(last) == way && now.saturating_sub(at) <= SWIPE_RETURN
            });
            if coming_back {
                // Spent: the NEXT swipe the same way is a swipe again.
                self.swiped = None;
                return;
            }
            if now >= self.quiet_until {
                self.quiet_until = now + COOLDOWN;
                self.swiped = Some((way, now));
                out.push(Event::Focus(way));
            }
        }
    }
}

/// Whether every point of a hand is inside the picture (see [`EDGE`]).
fn whole(h: &Hand) -> bool {
    h.points
        .iter()
        .all(|p| (EDGE..=1.0 - EDGE).contains(&p[0]) && (EDGE..=1.0 - EDGE).contains(&p[1]))
}

/// Whether the two index fingers cross: the segments knuckle→tip intersect.
fn fingers_cross(a: &Shape, b: &Shape) -> bool {
    let (p, r) = (a.index_base, a.index_points);
    let (q, t) = (b.index_base, b.index_points);
    let cross = |u: [f32; 2], v: [f32; 2]| u[0] * v[1] - u[1] * v[0];
    let d = cross(r, t);
    if d.abs() < 1e-6 {
        return false;
    }
    let qp = [q[0] - p[0], q[1] - p[1]];
    let along_a = cross(qp, t) / d;
    let along_b = cross(qp, r) / d;
    (0.0..=1.0).contains(&along_a) && (0.0..=1.0).contains(&along_b)
}

/// A pose of both hands, if they show one.
fn both_hands(a: &Shape, b: &Shape) -> Option<Candidate> {
    let (right, left) = if a.right { (a, b) } else { (b, a) };
    let (pa, pb) = (a.pose(), b.pose());
    // THE TWO INDEX FINGERS CROSSED IN AN X ARE `Esc` — asked before the
    // crossed arms, because two hands crossing their fingers can be close
    // enough to read as crossed, and before the arrows, which are the same two
    // fingers apart.
    if pa == Pose::Point && pb == Pose::Point && fingers_cross(a, b) {
        return Some(Candidate::Escape);
    }
    // CROSSED ARMS TURN GESTURES OFF, and nothing else does: the right hand
    // is seen LEFT of the left one. The model reads which hand it is from the
    // hand's own shape, so crossing the arms does not swap the answer — it
    // swaps the places.
    let raised = |s: &Shape| s.lean <= CROSS_LEAN && s.wrist_y <= CROSS_HIGH;
    if right.right != left.right
        && right.centre[0] + 0.5 * right.palm < left.centre[0]
        && raised(right)
        && raised(left)
    {
        return Some(Candidate::Off);
    }
    // Both middle fingers close the window.
    if pa == Pose::Middle && pb == Pose::Middle && a.upright() && b.upright() {
        return Some(Candidate::Close);
    }
    let up = |v: [f32; 2]| v[1] < -v[0].abs();
    let down = |v: [f32; 2]| v[1] > v[0].abs();
    // Both hands' thumb, index and middle, up or down: the workspace.
    if pa == Pose::Three && pb == Pose::Three {
        if up(a.pair_points) && up(b.pair_points) {
            return Some(Candidate::Workspace(Direction::Up));
        }
        if down(a.pair_points) && down(b.pair_points) {
            return Some(Candidate::Workspace(Direction::Down));
        }
    }
    // Both index fingers, the same way: that arrow.
    if pa == Pose::Point && pb == Pose::Point {
        let (da, db) = (direction_of(a.index_points), direction_of(b.index_points));
        if da == db {
            return Some(Candidate::Arrow(da));
        }
    }
    if pa == Pose::Open && pb == Pose::Open {
        let dx = (a.centre[0] - b.centre[0]).abs();
        let dy = (a.centre[1] - b.centre[1]).abs();
        if dx > dy && a.spread > 0.2 && b.spread > 0.2 && a.upright() && b.upright() {
            return Some(Candidate::Overview);
        }
        if dy > dx {
            return Some(Candidate::Keyboard);
        }
    }
    None
}

#[cfg(test)]
mod tests;
