//! THE WHEEL — an axis event turned into lines, and the lines a door has
//! earned out of it.
//!
//! # Why a module of its own
//!
//! It was the footer's, and every window that scrolls borrowed it from the
//! footer: the one arithmetic of a continuous gesture (what is left of a
//! line stays) has to be the same in every door, and a window that names
//! the footer to scroll a list can never be built without it.

use ronoctua_tokens as tokens;

/// How many lines a wheel detent steps where there is scrolling.
///
/// Three, which is what every text viewer does. One line per detent would
/// turn reading a manual into a wrist exercise; one page per detent is the
/// other mode's behavior, and mixing the two would make the same wheel mean
/// different things with nothing on screen saying which.
pub const LINES_PER_DETENT: f64 = 3.0;

/// How many "detents" an axis event is worth.
///
/// # Why three sources, and not one
///
/// Wayland has three ways of saying the same thing, and a client that reads
/// only one of them works on half the devices:
///
/// - `value120` is today's, in 120ths of a detent — it is what gives a
///   high-resolution wheel without losing the step;
/// - `discrete` is the old one, an integer per detent, and it is still what
///   arrives from an older compositor;
/// - `absolute` is the continuous one, in **finger pixels**, and it is the
///   only one the touchpad sends.
///
/// The touchpad is converted by the LINE HEIGHT, and not by a number of
/// pixels picked by hand: that way the finger moves the content in the same
/// proportion as it moves on screen, and the arithmetic recalibrates itself
/// if the font changes. Without that it would be one more visual value
/// outside the tokens (invariant 9).
pub fn axis_detents(v: &smithay_client_toolkit::seat::pointer::AxisScroll, line_px: f64) -> f64 {
    axis_lines(v, line_px) / LINES_PER_DETENT
}

/// The same event, in LINES.
///
/// # Why this is the primitive and the detent is derived from it
///
/// Because a detent is a MOUSE's unit and a line is the list's. A wheel says
/// "one notch" and the answer is three lines; a touchpad says "so many pixels"
/// and the answer is that many lines, which is what makes the list travel with
/// the finger. Rounding the finger's pixels into notches first and multiplying
/// back afterwards quantises it to three: the list stands still, stands still,
/// and jumps — which is what the owner read off the screen.
///
/// The footer keeps counting DETENTS because one of its two modes turns a
/// PAGE per detent, and a page is not a number of lines. The two are one
/// arithmetic with a division between them.
pub fn axis_lines(v: &smithay_client_toolkit::seat::pointer::AxisScroll, line_px: f64) -> f64 {
    if v.value120 != 0 {
        return f64::from(v.value120) / 120.0 * LINES_PER_DETENT;
    }
    if v.discrete != 0 {
        return f64::from(v.discrete) * LINES_PER_DETENT;
    }
    if line_px <= 0.0 {
        return 0.0;
    }
    v.absolute / line_px
}

/// One axis event, added to a door's accumulator of LINES.
///
/// # Why every list window needs one of these
///
/// Because the five of them worked the wheel out for themselves and all five
/// reached the same answer — `(pixels / line) as i64 * 3` — which is wrong in
/// both directions at once. Integer division throws away a fraction of a line,
/// so a slow touchpad gesture moved NOTHING until it crossed a whole line and
/// then moved THREE at once; and a mouse detent arrives as fifteen pixels,
/// which is nought lines, so the wheel did nothing at all until two of them
/// landed in the same frame. The owner read the result off the screen: a page
/// that jumps.
///
/// The footer never had it. It keeps what is left over and spends only whole
/// steps, which is the whole of what makes a continuous gesture continuous —
/// and this is that arithmetic, in the one place the five doors can share.
///
/// THE HORIZONTAL AXIS COUNTS TOO, for the reason the footer gives: sideways
/// is the natural touchpad gesture for "move on", and none of these lists
/// scrolls horizontally, so it competes with nothing.
pub fn wheel_arrived(
    left: &mut f64,
    vertical: &smithay_client_toolkit::seat::pointer::AxisScroll,
    horizontal: &smithay_client_toolkit::seat::pointer::AxisScroll,
) {
    // The `stop` of a continuous gesture zeroes what was left: without it,
    // half a line from the last gesture adds itself to the next and the first
    // scroll comes out bigger than the finger asked for.
    if vertical.stop || horizontal.stop {
        *left = 0.0;
    }
    let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
    *left += axis_lines(vertical, line_px) + axis_lines(horizontal, line_px);
}

/// How many whole LINES the accumulator has earned, taken out of it.
///
/// What is left STAYS, which is the half that makes the gesture add up.
#[allow(clippy::cast_possible_truncation)]
pub fn wheel_lines(left: &mut f64) -> i64 {
    let whole = left.trunc();
    *left -= whole;
    whole as i64
}
