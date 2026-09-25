//! Hands drawn by hand: an upright skeleton whose fingers are put out or
//! folded, turned, and moved — every pose of the table, in the geometry
//! MediaPipe's points have.

use super::*;

const ASPECT: f32 = 16.0 / 9.0;

/// How a finger is held.
#[derive(Clone, Copy, PartialEq)]
enum F {
    Out,
    In,
}

/// A hand, in HEIGHT units, wrist at `at`, turned by `turn` radians
/// (0 = fingers up, positive = clockwise as seen).
#[derive(Clone, Copy)]
struct Draw {
    at: [f32; 2],
    turn: f32,
    thumb: F,
    fingers: [F; 4],
    /// The thumb's tip goes to this point instead (a touch).
    thumb_to: Option<usize>,
    /// Fingers fanned apart.
    spread: bool,
    right: bool,
}

impl Draw {
    fn new(fingers: [F; 4], thumb: F) -> Self {
        Self {
            at: [0.9, 0.8],
            turn: 0.0,
            thumb,
            fingers,
            thumb_to: None,
            spread: false,
            right: true,
        }
    }

    fn hand(&self) -> Hand {
        // Palm: wrist to the middle knuckle is 0.16 of the frame's height.
        let knuckles = [
            [-0.05, -0.15],
            [-0.015, -0.16],
            [0.02, -0.15],
            [0.05, -0.135],
        ];
        let fans = if self.spread {
            [-0.4f32, 0.0, 0.2, 0.4]
        } else {
            [-0.05f32, 0.0, 0.03, 0.06]
        };
        let mut p = [[0.0f32; 3]; 21];
        // Thumb: base, knuckle, joint, tip.
        let thumb_out = [
            [-0.04, -0.03],
            [-0.075, -0.055],
            [-0.11, -0.08],
            [-0.14, -0.1],
        ];
        let thumb_in = [
            [-0.035, -0.03],
            [-0.055, -0.06],
            [-0.05, -0.09],
            [-0.03, -0.1],
        ];
        let thumb = if self.thumb == F::Out {
            thumb_out
        } else {
            thumb_in
        };
        for (k, t) in thumb.iter().enumerate() {
            p[1 + k] = [t[0], t[1], 0.0];
        }
        for (f, k) in knuckles.iter().enumerate() {
            let base = 5 + 4 * f;
            p[base] = [k[0], k[1], 0.0];
            let (s, c) = fans[f].sin_cos();
            let dir = [s, -c];
            match self.fingers[f] {
                F::Out => {
                    for (j, len) in [0.05f32, 0.085, 0.11].iter().enumerate() {
                        p[base + 1 + j] = [k[0] + dir[0] * len, k[1] + dir[1] * len, 0.0];
                    }
                }
                F::In => {
                    p[base + 1] = [k[0], k[1] - 0.035, -0.02];
                    p[base + 2] = [k[0], k[1] - 0.01, -0.045];
                    p[base + 3] = [k[0], k[1] + 0.02, -0.03];
                }
            }
        }
        if let Some(to) = self.thumb_to {
            p[4] = p[to];
            p[3] = [
                (p[2][0] + p[to][0]) / 2.0,
                (p[2][1] + p[to][1]) / 2.0,
                (p[2][2] + p[to][2]) / 2.0,
            ];
        }
        let (s, c) = self.turn.sin_cos();
        let mut points = [[0.0f32; 3]; 21];
        for (o, q) in points.iter_mut().zip(p.iter()) {
            let x = q[0] * c - q[1] * s + self.at[0];
            let y = q[0] * s + q[1] * c + self.at[1];
            *o = [x / ASPECT, y, q[2] / ASPECT];
        }
        Hand {
            points,
            right: self.right,
        }
    }
}

/// The POINTER: index and middle out and apart, the thumb folded.
fn pointer() -> Draw {
    let mut d = Draw::new([F::Out, F::Out, F::In, F::In], F::In);
    d.spread = true;
    d
}

/// The index alone.
fn index() -> Draw {
    Draw::new([F::Out, F::In, F::In, F::In], F::In)
}

fn three() -> Draw {
    Draw::new([F::Out, F::Out, F::In, F::In], F::Out)
}

fn hang_loose() -> Draw {
    Draw::new([F::In, F::In, F::In, F::Out], F::Out)
}

fn fist() -> Draw {
    Draw::new([F::In; 4], F::In)
}

fn open() -> Draw {
    let mut d = Draw::new([F::Out; 4], F::Out);
    d.spread = true;
    d
}

fn reading(at: u64, hands: &[Draw]) -> Reading {
    Reading {
        at,
        aspect: ASPECT,
        hands: hands.iter().map(Draw::hand).collect(),
    }
}

/// Feeds the same hands every 66 ms for `ms`, and gathers every event.
fn hold(r: &mut Reader, from: u64, ms: u64, hands: &[Draw]) -> Vec<Event> {
    let mut out = Vec::new();
    let mut t = from;
    while t <= from + ms {
        out.extend(r.read(&reading(t, hands)));
        t += 66;
    }
    out
}

fn commands(events: &[Event]) -> Vec<Event> {
    events
        .iter()
        .copied()
        .filter(|e| !matches!(e, Event::Point { .. } | Event::Scroll(_)))
        .collect()
}

#[test]
fn the_drawn_poses_are_the_poses() {
    let pose = |d: Draw| Shape::of(&d.hand(), ASPECT).pose();
    assert_eq!(pose(pointer()), Pose::Pointer);
    assert_eq!(pose(three()), Pose::Three);
    assert_eq!(pose(index()), Pose::Point);
    assert_eq!(pose(open()), Pose::Open);
    assert_eq!(pose(hang_loose()), Pose::HangLoose);
    assert_eq!(
        pose(Draw::new([F::In, F::Out, F::In, F::In], F::In)),
        Pose::Middle
    );
    assert_eq!(
        pose(Draw::new([F::Out, F::In, F::In, F::Out], F::In)),
        Pose::Rock
    );
    assert_eq!(pose(fist()), Pose::Other);
}

#[test]
fn two_fingers_together_do_not_start_the_pointer() {
    // A hand at rest falls into the pose with the fingers close together;
    // pointing on purpose opens them.
    let mut r = Reader::new();
    let close = Draw::new([F::Out, F::Out, F::In, F::In], F::In);
    assert!(hold(&mut r, 0, 1000, &[close]).is_empty());
}

fn last_x(e: &[Event]) -> f32 {
    match e.iter().rev().find(|e| matches!(e, Event::Point { .. })) {
        Some(Event::Point { x, .. }) => *x,
        _ => panic!("no pointer: {e:?}"),
    }
}

#[test]
fn index_and_middle_move_the_pointer_across_the_area() {
    let mut r = Reader::new();
    let mut left = pointer();
    left.at = [0.5, 0.9];
    let mut right = pointer();
    right.at = [1.25, 0.9];
    let a = hold(&mut r, 0, 1000, &[left]);
    let b = hold(&mut r, 2000, 1000, &[right]);
    assert!(last_x(&a) < 0.3, "{}", last_x(&a));
    assert!(last_x(&b) > 0.7, "{}", last_x(&b));
    assert!(commands(&a).is_empty() && commands(&b).is_empty());
}

#[test]
fn the_index_alone_moves_nothing() {
    let mut r = Reader::new();
    let e = hold(&mut r, 0, 1000, &[index()]);
    assert!(e.is_empty(), "{e:?}");
}

/// The pointer on the right, and the other hand on the left.
fn with_other(other: Draw) -> [Draw; 2] {
    let mut p = pointer();
    p.at = [1.1, 0.8];
    let mut o = other;
    o.right = false;
    o.at = [0.6, 0.8];
    [o, p]
}

#[test]
fn the_other_hand_open_holds_the_left_button_down() {
    let mut r = Reader::new();
    let mut e = hold(&mut r, 0, 300, &with_other(fist()));
    e.extend(hold(&mut r, 400, 1200, &with_other(open())));
    assert_eq!(
        commands(&e),
        [Event::Press(Button::Left)],
        "held while open"
    );
    e.extend(hold(&mut r, 1700, 300, &with_other(fist())));
    assert_eq!(
        commands(&e),
        [Event::Press(Button::Left), Event::Release(Button::Left)]
    );
}

#[test]
fn the_pointer_keeps_moving_while_the_button_is_down() {
    let mut r = Reader::new();
    hold(&mut r, 0, 600, &with_other(open()));
    let [o, mut p] = with_other(open());
    p.at = [1.4, 0.8];
    let e = hold(&mut r, 700, 600, &[o, p]);
    assert!(e.iter().any(|e| matches!(e, Event::Point { .. })), "a drag");
    assert!(
        !e.iter().any(|e| matches!(e, Event::Focus(_))),
        "the open hand is not a swipe"
    );
}

#[test]
fn one_reading_of_an_open_hand_is_not_a_click() {
    let mut r = Reader::new();
    let mut e = hold(&mut r, 0, 300, &with_other(fist()));
    e.extend(r.read(&reading(400, &with_other(open()))));
    e.extend(hold(&mut r, 466, 600, &with_other(fist())));
    assert!(commands(&e).is_empty(), "{e:?}");
}

#[test]
fn a_lost_pointer_lets_go_of_its_button() {
    let mut r = Reader::new();
    let mut e = hold(&mut r, 0, 400, &with_other(open()));
    e.extend(hold(&mut r, 500, 600, &[]));
    assert_eq!(
        commands(&e),
        [Event::Press(Button::Left), Event::Release(Button::Left)]
    );
}

#[test]
fn the_other_hand_hanging_loose_is_one_right_click() {
    let mut r = Reader::new();
    let e = hold(&mut r, 0, 1000, &with_other(hang_loose()));
    assert_eq!(
        commands(&e),
        [Event::Press(Button::Right), Event::Release(Button::Right)]
    );
}

#[test]
fn a_hang_loose_without_a_pointer_is_nothing() {
    let mut r = Reader::new();
    assert!(commands(&hold(&mut r, 0, 1000, &[hang_loose()])).is_empty());
}

#[test]
fn three_fingers_moving_down_scroll_down() {
    let mut r = Reader::new();
    let mut total = 0.0;
    for k in 0..10u64 {
        let mut d = three();
        d.at[1] = 0.6 + 0.03 * k as f32;
        for e in r.read(&reading(k * 66, &[d])) {
            if let Event::Scroll(l) = e {
                total += l;
            }
        }
    }
    assert!(total > 3.0, "{total}");
    let e = hold(&mut r, 2000, 600, &[three()]);
    assert!(
        !e.iter().any(|e| matches!(e, Event::Scroll(_))),
        "still, nothing"
    );
}

fn two(mut a: Draw, mut b: Draw, apart: [f32; 2]) -> [Draw; 2] {
    a.right = false;
    b.right = true;
    a.at = [0.9 - apart[0], 0.6 - apart[1]];
    b.at = [0.9 + apart[0], 0.6 + apart[1]];
    [a, b]
}

#[test]
fn both_index_fingers_the_same_way_are_that_arrow() {
    for (turn, want) in [
        (0.0, Direction::Up),
        (std::f32::consts::FRAC_PI_2, Direction::Right),
        (std::f32::consts::PI, Direction::Down),
        (-std::f32::consts::FRAC_PI_2, Direction::Left),
    ] {
        let mut r = Reader::new();
        let mut i = index();
        i.turn = turn;
        let e = hold(&mut r, 0, 1500, &two(i, i, [0.25, 0.0]));
        assert_eq!(commands(&e), [Event::Arrow(want)], "turned {turn}");
    }
}

#[test]
fn the_index_fingers_crossed_are_escape() {
    let mut r = Reader::new();
    let mut a = index();
    a.turn = 0.7;
    let mut b = index();
    b.turn = -0.7;
    let e = hold(&mut r, 0, 1500, &two(a, b, [0.14, 0.0]));
    assert_eq!(commands(&e), [Event::Escape]);
}

#[test]
fn the_rock_sign_opens_the_player() {
    let mut r = Reader::new();
    let e = hold(
        &mut r,
        0,
        1000,
        &[Draw::new([F::Out, F::In, F::In, F::Out], F::In)],
    );
    assert_eq!(commands(&e), [Event::Player]);
}

#[test]
fn an_open_hand_swept_moves_the_focus() {
    for (dx, dy, want) in [
        (0.3f32, 0.0f32, Direction::Right),
        (-0.3, 0.0, Direction::Left),
        (0.0, -0.3, Direction::Up),
        (0.0, 0.3, Direction::Down),
    ] {
        let mut r = Reader::new();
        let mut e = Vec::new();
        for k in 0..6u64 {
            let mut d = open();
            d.at = [0.9 + dx * k as f32 / 5.0, 0.6 + dy * k as f32 / 5.0];
            e.extend(r.read(&reading(k * 66, &[d])));
        }
        assert_eq!(commands(&e), [Event::Focus(want)]);
        let mut still = open();
        still.at = [0.9 + dx, 0.6 + dy];
        assert!(commands(&hold(&mut r, 1000, 1000, &[still])).is_empty());
    }
}

#[test]
fn two_middle_fingers_close_the_window() {
    let mut r = Reader::new();
    let m = Draw::new([F::In, F::Out, F::In, F::In], F::In);
    let e = hold(&mut r, 0, 1500, &two(m, m, [0.35, 0.0]));
    assert_eq!(commands(&e), [Event::Close]);
}

#[test]
fn a_single_middle_finger_is_nothing() {
    let mut r = Reader::new();
    let m = Draw::new([F::In, F::Out, F::In, F::In], F::In);
    assert!(commands(&hold(&mut r, 0, 1500, &[m])).is_empty());
}

#[test]
fn both_hands_three_fingers_change_the_workspace() {
    let mut r = Reader::new();
    let e = hold(&mut r, 0, 1000, &two(three(), three(), [0.35, 0.0]));
    assert_eq!(commands(&e), [Event::Workspace(Direction::Up)]);
    let mut down = three();
    down.turn = std::f32::consts::PI;
    let mut r = Reader::new();
    let e = hold(&mut r, 0, 1000, &two(down, down, [0.35, 0.0]));
    assert_eq!(commands(&e), [Event::Workspace(Direction::Down)]);
}

#[test]
fn ten_fingers_side_by_side_are_the_overview() {
    let mut r = Reader::new();
    let e = hold(&mut r, 0, 1000, &two(open(), open(), [0.4, 0.0]));
    assert_eq!(commands(&e), [Event::Overview]);
}

#[test]
fn two_open_hands_stacked_are_the_keyboard() {
    let mut r = Reader::new();
    let e = hold(&mut r, 0, 1000, &two(open(), open(), [0.0, 0.3]));
    assert_eq!(commands(&e), [Event::Keyboard]);
}

#[test]
fn crossed_arms_turn_gestures_off() {
    let mut r = Reader::new();
    // Seen from the camera, the right hand on the LEFT.
    let [mut left, mut right] = two(fist(), fist(), [0.3, 0.0]);
    std::mem::swap(&mut left.at, &mut right.at);
    let e = hold(&mut r, 0, 1500, &[left, right]);
    assert_eq!(commands(&e), [Event::Off]);
    let mut r = Reader::new();
    assert!(commands(&hold(&mut r, 0, 1500, &two(fist(), fist(), [0.3, 0.0]))).is_empty());
}

#[test]
fn a_command_waits_out_the_cooldown_of_the_last() {
    let mut r = Reader::new();
    let rock = Draw::new([F::Out, F::In, F::In, F::Out], F::In);
    let mut e = hold(&mut r, 0, 400, &[rock]);
    let m = Draw::new([F::In, F::Out, F::In, F::In], F::In);
    // The close's dwell ends inside the player's cooldown; it fires after.
    e.extend(hold(&mut r, 420, 1500, &two(m, m, [0.35, 0.0])));
    assert_eq!(commands(&e), [Event::Player, Event::Close]);
}

#[test]
fn calibration_measures_the_area_from_its_corners() {
    let a = Area::around(&[[0.3, 0.2], [0.7, 0.2], [0.7, 0.6], [0.3, 0.6]]).unwrap();
    let m = a.map([0.5, 0.4]);
    assert!(
        (m[0] - 0.5).abs() < 1e-5 && (m[1] - 0.5).abs() < 1e-5,
        "{m:?}"
    );
    assert_eq!(a.map([0.0, 1.0]), [0.0, 1.0], "clamped");
    assert!(
        Area::around(&[[0.3, 0.2], [0.31, 0.21]]).is_none(),
        "a dot is no area"
    );
}

// WHAT THE OWNER'S OWN HANDS TAUGHT, one pose each: a way the first reader
// failed on a recording of a real hand, drawn here as a skeleton so the
// numbers stay out of the tree.

#[test]
fn the_hand_coming_back_moves_nothing() {
    let mut r = Reader::new();
    let mut e = Vec::new();
    let mut t = 0;
    for (from, to) in [(0.7f32, 1.1f32), (1.1, 0.7)] {
        for k in 0..6 {
            let mut d = open();
            d.at = [from + (to - from) * k as f32 / 5.0, 0.8];
            e.extend(r.read(&reading(t, &[d])));
            t += 66;
        }
        let mut still = open();
        still.at = [to, 0.8];
        e.extend(hold(&mut r, t, 700, &[still]));
        t += 766;
    }
    assert_eq!(commands(&e), [Event::Focus(Direction::Right)]);
}

#[test]
fn a_hand_lying_on_the_desk_does_not_swipe() {
    let mut r = Reader::new();
    let mut e = Vec::new();
    for k in 0..6u64 {
        let mut d = open();
        d.turn = std::f32::consts::PI;
        d.at = [0.7 + 0.08 * k as f32, 0.6];
        e.extend(r.read(&reading(k * 66, &[d])));
    }
    assert!(commands(&e).is_empty(), "{e:?}");
}

#[test]
fn the_table_the_help_reads_is_every_gesture_and_nothing_else() {
    let every = [
        Event::Point { x: 0.0, y: 0.0 },
        Event::Press(Button::Left),
        Event::Release(Button::Left),
        Event::Press(Button::Right),
        Event::Release(Button::Right),
        Event::Scroll(1.0),
        Event::Arrow(Direction::Up),
        Event::Escape,
        Event::Focus(Direction::Up),
        Event::Workspace(Direction::Up),
        Event::Overview,
        Event::Keyboard,
        Event::Player,
        Event::Close,
        Event::Off,
    ];
    for e in every {
        assert!(GESTURES.contains(&gesture_of(e)), "{e:?} has no row");
    }
    for g in GESTURES {
        assert!(
            every.iter().any(|e| gesture_of(*e) == *g),
            "{g} comes from no event"
        );
    }
}

// WHAT A LAP TAUGHT: hands resting, not gesturing, had opened the overview
// and turned gestures off.

#[test]
fn open_hands_lying_flat_are_not_the_overview() {
    let mut r = Reader::new();
    let mut a = open();
    a.turn = std::f32::consts::FRAC_PI_2;
    let mut b = open();
    b.turn = std::f32::consts::FRAC_PI_2;
    assert!(commands(&hold(&mut r, 0, 1500, &two(a, b, [0.4, 0.0]))).is_empty());
}

#[test]
fn hands_crossed_low_in_the_picture_do_not_turn_gestures_off() {
    let mut r = Reader::new();
    let [mut left, mut right] = two(fist(), fist(), [0.3, 0.0]);
    std::mem::swap(&mut left.at, &mut right.at);
    left.at[1] = 0.85;
    right.at[1] = 0.85;
    assert!(commands(&hold(&mut r, 0, 1500, &[left, right])).is_empty());
}

#[test]
fn a_hand_cut_by_the_edge_is_not_read() {
    let mut r = Reader::new();
    let mut p = pointer();
    p.at = [0.9, 1.05];
    assert!(hold(&mut r, 0, 1000, &[p]).is_empty());
}
