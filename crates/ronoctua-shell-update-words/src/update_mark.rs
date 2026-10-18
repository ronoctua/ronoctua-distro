//! THE UPDATE'S MARK AS THE SHELL DRAWS IT — what the footer's modeline
//! draws while the machine brings its software forward, and the door a click
//! on it opens: the footer names no feature, so the mark lives under it and
//! under the update's view (`ronoctua-shell-update`), on no other shell crate.

use ronoctua_brain::compiled::system_update as module;
use ronoctua_tokens as tokens;

/// The two frames of the mark: the same circle, filled and hollow.
///
/// # Why an arrow in a circle, and why it breathes
///
/// Because it was a clock face and the owner threw it out on sight — a clock
/// says what time it is, and nothing on that line is about the time. An arrow
/// in a circle is what an update is drawn as everywhere one has been drawn.
///
/// It still moves, because a still glyph and a hung update look identical and
/// this is the one thing here that legitimately takes twenty minutes. Filled
/// against hollow is the same shape at the same size, so it breathes rather
/// than twitches — which is what `chrome-icons-match-the-text` is measuring.
///
/// # Why not the pie
///
/// `assets/tokens/icons.nix` argues it where the tokens live, and the
/// distinction is the whole point: `countdown-*` is how much time is LEFT and
/// empties towards a deadline; this is something running whose end nobody
/// knows. Same subject, opposite question, so they are different drawings —
/// and reusing the pie here would promise a finish line the machine cannot see.
pub const FRAMES: [char; 2] = [tokens::icon::WORKING_ONE, tokens::icon::WORKING_TWO];

/// Which frame a tick of the footer's clock draws.
///
/// The tick is the footer's own one-second counter — see the timer in
/// the footer's `surface`, which says why a second clock for the same second is refused.
/// It wraps at twelve, so the hand goes round once a minute.
#[must_use]
pub fn frame_of(tick: u64) -> char {
    // `as usize` on a remainder that is already below twelve: the cast cannot
    // truncate, and the modulo is what guarantees it.
    #[allow(clippy::cast_possible_truncation)]
    let n = (tick % FRAMES.len() as u64) as usize;
    FRAMES[n]
}

/// Whether the machine has an update in flight, as the brain reports it.
///
/// # Why the QUEUE counts, and not only what is running
///
/// Because a queued task is one the brain has already decided to run: nobody
/// is going to be asked again, and what it is waiting for is the machine — RAM,
/// processor, mains — which is exactly what `brain-hold-*` exists to say out
/// loud. Taking the mark off screen until the run starts would hide the wait
/// rather than the work, and "a scheduler that silently does nothing is a
/// scheduler nobody trusts" is this crate's own sentence about it.
///
/// The animation is what tells the two apart at a glance: it moves while the
/// unit is producing and it moves while the gate holds, and the window says
/// which. What it never does is stay on screen with nothing pending.
#[must_use]
pub fn running(mind: Option<&ronoctua_brain::Mind>) -> bool {
    let Some(m) = mind else {
        return false;
    };
    m.running == module::ID || m.queue.iter().any(|q| q == module::ID)
}

/// The modeline's mark, or `None` when there is nothing to say.
///
/// One cell and one glyph. There is no number beside it on purpose: a
/// percentage of a rebuild is a number nobody can compute honestly, and a step
/// count would put the sequence's ids on the line that has least room for them.
/// What the count is for is the window.
#[must_use]
pub fn mark(mind: Option<&ronoctua_brain::Mind>, tick: u64) -> Option<String> {
    running(mind).then(|| frame_of(tick).to_string())
}

/// The command that opens the view in a window — the door the modeline's
/// mark opens.
pub const WATCH: &str = "system-update-watch";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_frame_per_tick_and_it_breathes() {
        // TWO FRAMES: the same circle, filled and hollow. It was twelve
        // positions of a clock hand until the owner saw it — a clock says what
        // time it is, and nothing on that line is about the time.
        assert_eq!(FRAMES.len(), 2, "the pulse has two frames");
        for tick in 0..8u64 {
            assert_eq!(
                frame_of(tick),
                FRAMES[usize::try_from(tick % 2).unwrap()],
                "the frame of tick {tick} left the lap"
            );
        }
        // ONE PER TICK, and never two the same: the footer beats once a second,
        // and a mark that drew the same frame twice would stop breathing.
        for tick in 0..5u64 {
            assert_ne!(
                frame_of(tick),
                frame_of(tick + 1),
                "two ticks in a row drew the same frame"
            );
        }
        assert_eq!(frame_of(0), frame_of(2), "the lap did not close in two");
        // AND THEY ARE NOT THE PIE. The two answer opposite questions and
        // `icons.nix` says so; the day somebody reaches for the wrong family
        // this is what refuses.
        for frame in FRAMES {
            assert!(
                !matches!(
                    frame,
                    tokens::icon::COUNTDOWN_ONE
                        | tokens::icon::COUNTDOWN_TWO
                        | tokens::icon::COUNTDOWN_THREE
                        | tokens::icon::COUNTDOWN_FOUR
                        | tokens::icon::COUNTDOWN_FIVE
                        | tokens::icon::COUNTDOWN_SIX
                        | tokens::icon::COUNTDOWN_SEVEN
                        | tokens::icon::COUNTDOWN_EIGHT
                ),
                "the mark took a frame from the pie"
            );
        }
    }
}
