//! The screen capture's answer, between the overlay that chose and the
//! footer that started it.
//!
//! # Why the wire is here and not in the overlay
//!
//! The capture is another process (`ronoctua-shell capture`), and it says
//! how it ended on its standard output — one line, read by the footer once
//! the process is gone. Both ends speak the same three words, so they are
//! written once, here: the overlay prints them ([`SAVED`], [`FAILED`],
//! [`OUTSIDE_THE_ROOM`]) and the footer reads them back ([`outcome_of`]).
//! The overlay is a door of the binary and the footer names no door, so the
//! words live below both.

/// What starts the line that says where the file was written:
/// `saved <path>`.
pub const SAVED: &str = "saved ";

/// What starts the line that says why nothing was written, for the log:
/// `failed <why>`.
pub const FAILED: &str = "failed ";

/// The line the overlay prints when the rectangle chosen lay wholly under
/// the resizer's curtains — a refusal by NAME, which the footer says in the
/// person's language ([`Outcome::OutsideTheRoom`]).
pub const OUTSIDE_THE_ROOM: &str = "refused outside-the-room";

/// How the overlay ended, as the footer reads it off what it printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A file was written here.
    Saved(std::path::PathBuf),
    /// It failed, in these words — for the log.
    Failed(String),
    /// The rectangle lay wholly under the resizer's curtains.
    OutsideTheRoom,
    /// Given up: `Esc`, or nothing said.
    GivenUp,
}

/// What the overlay printed, read.
#[must_use]
pub fn outcome_of(said: &str) -> Outcome {
    said.lines()
        .find_map(|l| {
            if l == OUTSIDE_THE_ROOM {
                return Some(Outcome::OutsideTheRoom);
            }
            l.strip_prefix(SAVED)
                .map(|p| Outcome::Saved(std::path::PathBuf::from(p)))
                .or_else(|| {
                    l.strip_prefix(FAILED)
                        .map(|w| Outcome::Failed(w.to_owned()))
                })
        })
        .unwrap_or(Outcome::GivenUp)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the footer reads a choice the room emptied as a capture
    /// given up — silence — or as a failure in English words.
    #[test]
    fn every_ending_is_read_by_its_name() {
        assert_eq!(
            outcome_of("saved /home/x/a.png\n"),
            Outcome::Saved("/home/x/a.png".into())
        );
        assert_eq!(outcome_of(OUTSIDE_THE_ROOM), Outcome::OutsideTheRoom);
        assert_eq!(
            outcome_of("failed grim exited"),
            Outcome::Failed("grim exited".into())
        );
        assert_eq!(outcome_of(""), Outcome::GivenUp);
    }
}
