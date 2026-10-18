//! How long a message stays — on the footer's minibuffer and on a window's
//! line 0.
//!
//! # Why classes, and not one number
//!
//! There was one number, three seconds, for everything the shell says. A
//! `Quit` after `Esc` held the line for as long as a refusal that has to be
//! read to the end, and the owner saw the short ones linger: a word that
//! repeats what the hand just did needs a glance, and three seconds of it is
//! a line taken from whatever comes next. So a message says what KIND it is,
//! and the seconds are tokens (`space.echo.*`), in tenths, beside every other
//! number the screen keeps.
//!
//! # And a clock only while there is a message
//!
//! The footer arms a one-shot timer for the deadline of the message it put up
//! (`Footer::echo_for`), and the timer finds the message still there or does
//! nothing — a newer message carries its own. Nothing ticks while the line is
//! empty.
//!
//! A window's line 0 takes its deadline from the door's own one-shot
//! (`ronoctua_shell_door::door::hang_the_echo`), and from THIS table: `Ui::say` is the
//! [`Echo::Normal`] class, and `Ui::say_for` names another. One table for
//! both doors — two would be two answers to "how long is a message on
//! screen".

use ronoctua_tokens as tokens;

/// The class of a message: how long it is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Echo {
    /// A word that confirms what the hand just did — `Quit`, the bars'
    /// new mode. A glance.
    Brief,
    /// A sentence: what happened, and what it means. Every message that
    /// names no class.
    #[default]
    Normal,
    /// A refusal that has to be read to the end, and acted on — a
    /// preference that could not be saved.
    Long,
}

impl Echo {
    /// How long a message of this class stays.
    #[must_use]
    pub fn lasts(self) -> std::time::Duration {
        let tenths = match self {
            Self::Brief => tokens::space::ECHO_BRIEFTENTHS,
            Self::Normal => tokens::space::ECHO_NORMALTENTHS,
            Self::Long => tokens::space::ECHO_LONGTENTHS,
        };
        std::time::Duration::from_millis(u64::from(tenths) * 100)
    }
}

/// How long a NORMAL message stays on screen — BOTH doors, in whole
/// seconds, for the windows that arm a one-shot of their own
/// (`FilesWindow::say`).
///
/// It is the `normal` class's token (`crate::echo`), and not a number of its
/// own: two mechanisms that disagreed about "how long is a message on
/// screen" would give that question two answers.
#[allow(clippy::cast_lossless)]
pub const ECHO_SECONDS: u64 = tokens::space::ECHO_NORMALTENTHS as u64 / 10;

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the classes stop being ordered — a brief message outlasting
    /// a normal one, or a refusal gone before a sentence — or the normal one
    /// stops being the number both doors agreed on (`ECHO_SECONDS`).
    #[test]
    fn the_classes_are_ordered_and_the_normal_one_is_the_old_number() {
        let [brief, normal, long] = [Echo::Brief, Echo::Normal, Echo::Long].map(Echo::lasts);
        assert!(
            brief < normal && normal < long,
            "{brief:?} {normal:?} {long:?}"
        );
        assert_eq!(
            normal,
            std::time::Duration::from_secs(crate::echo::ECHO_SECONDS)
        );
        assert_eq!(Echo::default(), Echo::Normal);
        // A GLANCE IS UNDER TWO SECONDS, and a refusal twice a sentence.
        assert!(brief < std::time::Duration::from_secs(2));
        assert!(long >= normal * 2);
    }
}
