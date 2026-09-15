//! A name too long for its column, walking sideways.
//!
//! # What it is for
//!
//! A listing is a grid, and a grid cuts. The owner asked for the rest of a cut
//! name to be reachable three ways, and this module is the third: *"tanto pela
//! seleção do teclado, como pelo foco/hover do mouse, o nome do arquivo que
//! seja maior do que o espaço disponível, deve se movimentar automaticamente
//! para a esquerda (assim mostrando o restante do nome), em uma 'animação de
//! movimento', de tempo em tempo"*. The other two are the minibuffer — under
//! the pointer, and for three seconds when the cursor arrives.
//!
//! # The shape of the walk, and whose choice it is
//!
//! Asked, the owner chose *"passo a passo, com pausa nas pontas"* over a
//! smooth glide: it rests a second at the start, steps one CELL at a time to
//! the end, rests a second there, and jumps back. A cell at a time is what
//! keeps it a grid — sub-cell motion would need the name drawn at a pixel
//! offset, which is a second way of placing text on a surface where everything
//! else is placed in columns.
//!
//! # Why it is a pure function of a duration
//!
//! Because then the clock is the DOOR's and the animation is arithmetic. Two
//! doors draw this listing and neither may have an idea of its own about where
//! the name has got to; the view records how long the current name has been on
//! screen and both ask this. It is also the whole of what a test can reach:
//! `offset_at` walks a cycle in this file's own tests, with no surface, no
//! timer and no frame.

use std::time::Duration;

/// How long the name rests at each end before moving on.
///
/// A second, and the same second at both ends: the pause at the start is there
/// to be READ — a name that began walking the instant the cursor landed would
/// never be still long enough to take in — and the pause at the end is the
/// same courtesy for the other half of it.
pub const DWELL: Duration = Duration::from_millis(1000);

/// How long one cell takes.
///
/// Five cells a second. Slower reads as broken; faster is a smear, and the
/// owner asked for step by step.
pub const STEP: Duration = Duration::from_millis(200);

/// How far along a name has walked, in cells.
///
/// `elapsed` is how long it has been under the cursor or the pointer; `over`
/// is how many cells of it do not fit. `0` means the beginning of the name is
/// showing, `over` means the end is.
///
/// The cycle is `DWELL + over × STEP + DWELL`, and then it starts again from
/// the beginning — a jump back rather than a walk back, which is the owner's
/// choice and also the honest one: walking back would spend as long showing
/// the middle a second time as it spent getting there.
#[must_use]
pub fn offset_at(elapsed: Duration, over: u16) -> u16 {
    if over == 0 {
        return 0;
    }
    let walk = STEP * u32::from(over);
    let cycle = DWELL + walk + DWELL;
    // BY THE REMAINDER, so the walk repeats for as long as the name is there.
    // In milliseconds, which is exact for every duration this module names and
    // wide enough for a name left under the pointer for a fortnight.
    let at =
        Duration::from_millis(u64::try_from(elapsed.as_millis() % cycle.as_millis()).unwrap_or(0));
    if at < DWELL {
        return 0;
    }
    let at = at - DWELL;
    if at >= walk {
        return over;
    }
    // WHOLE CELLS ONLY, which is what makes it a step and not a glide.
    u16::try_from(at.as_millis() / STEP.as_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{DWELL, STEP, offset_at};
    use std::time::Duration;

    #[test]
    fn a_name_that_fits_never_moves() {
        for ms in [0, 1, 999, 1000, 5_000, 60_000] {
            assert_eq!(offset_at(Duration::from_millis(ms), 0), 0);
        }
    }

    /// THE SHAPE THE OWNER CHOSE, walked end to end: a second still, a cell
    /// every two hundred milliseconds, a second still at the far end, and back
    /// to the beginning.
    #[test]
    fn it_rests_at_both_ends_and_steps_between_them() {
        let over = 5_u16;
        let walk = STEP * u32::from(over);
        let cycle = DWELL + walk + DWELL;

        // THE FIRST SECOND IS STILL, and the last instant of it too.
        assert_eq!(offset_at(Duration::ZERO, over), 0);
        assert_eq!(offset_at(DWELL - Duration::from_millis(1), over), 0);
        // THEN ONE CELL PER STEP.
        for n in 0..over {
            let at = DWELL + STEP * u32::from(n);
            assert_eq!(offset_at(at, over), n, "at {at:?}");
            assert_eq!(
                offset_at(at + STEP - Duration::from_millis(1), over),
                n,
                "a cell moved inside its own step"
            );
        }
        // THE FAR END IS HELD for the whole second, and not for an instant.
        assert_eq!(offset_at(DWELL + walk, over), over);
        assert_eq!(
            offset_at(cycle - Duration::from_millis(1), over),
            over,
            "the rest at the far end ended early"
        );
        // AND THEN BACK TO THE BEGINNING, all at once.
        assert_eq!(offset_at(cycle, over), 0);
        assert_eq!(offset_at(cycle + DWELL, over), 0);
        assert_eq!(offset_at(cycle * 7 + DWELL + STEP * 2, over), 2);
    }

    /// AND IT NEVER WALKS PAST THE END. The drawing windows the name at this
    /// offset, and one cell more would show a blank column where the last
    /// letter should be.
    #[test]
    fn nothing_walks_past_the_end_of_the_name() {
        for over in 1..40_u16 {
            for ms in (0..20_000).step_by(37) {
                let at = offset_at(Duration::from_millis(ms), over);
                assert!(at <= over, "over {over}, at {ms} ms: {at}");
            }
        }
    }
}
