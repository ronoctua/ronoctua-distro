//! Compose sequences — `Multi_key` and the keys after it, turned into the one
//! character they name (`'` then `e` into `é`).
//!
//! # Why the shell composes, and the compositor does not
//!
//! xkb gives a key ONE keysym; a SEQUENCE of keysyms is a thing above the
//! keymap, held in an `xkb_compose_state` the client keeps and feeds a key at
//! a time. A wlroots compositor (`scroll`) ships the keymap and nothing more —
//! it does not compose for its clients — so composing is the client's to do.
//! Without it the reader took each keysym's own character
//! (`keys::char_of`): Caps Lock, which `compose:caps` makes the `Multi_key`,
//! had none, and `'` then `e` arrived as themselves. The person who chose a
//! Compose key pressed it and nothing happened.
//!
//! # Where it is fed
//!
//! Every typed key, at the ONE sink each reader has — `Door::typed`
//! (`door/mod.rs`) for a window, and the footer's own `typed`
//! (`ronoctua-shell-footer`). A key mid-sequence types nothing
//! ([`Composed::Swallow`]); the key that finishes one types the result
//! ([`Composed::Text`]); everything else is the key as it came
//! ([`Composed::Pass`]), so a binding, an arrow and a plain letter are
//! untouched unless a sequence is actually open.

use std::ffi::OsString;

use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym};
use xkbcommon::xkb;

/// The session's compose table and the state fed one key at a time.
pub struct Compose {
    state: xkb::compose::State,
}

/// What a key becomes once the compose state has seen it.
#[derive(Debug)]
pub enum Composed {
    /// Not part of a sequence: the key, unchanged — a binding, an arrow, a
    /// plain letter.
    Pass,
    /// Mid-sequence (`Multi_key` just pressed, or a step after it): it types
    /// nothing on its own, and a cancelled sequence types nothing either.
    Swallow,
    /// A sequence finished: the character it names, as the key that would have
    /// typed it — its text and a keysym a reader can still read.
    Text(KeyEvent),
}

impl Compose {
    /// The compose table for the session's locale, or `None` when the locale
    /// names none — then the shell composes nothing and every key is itself,
    /// exactly as before this existed.
    ///
    /// The locale is read as `libxkbcommon` reads it: `LC_ALL`, else
    /// `LC_CTYPE`, else `LANG`, else `C`.
    #[must_use]
    pub fn from_locale() -> Option<Self> {
        let locale = std::env::var_os("LC_ALL")
            .or_else(|| std::env::var_os("LC_CTYPE"))
            .or_else(|| std::env::var_os("LANG"))
            .unwrap_or_else(|| OsString::from("C"));
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let table =
            xkb::compose::Table::new_from_locale(&context, &locale, xkb::compose::COMPILE_NO_FLAGS)
                .ok()?;
        Some(Self {
            state: xkb::compose::State::new(&table, xkb::compose::STATE_NO_FLAGS),
        })
    }

    /// `event` fed to the compose state: the one place a key's keysym becomes a
    /// composed character.
    ///
    /// A modifier keysym the state IGNORES — it leaves a sequence in progress
    /// untouched, so Shift may be held to compose — and that key, and any key
    /// while nothing is open, passes through unchanged. Only a key the state
    /// takes can swallow or finish.
    pub fn feed(&mut self, event: &KeyEvent) -> Composed {
        use xkb::compose::{FeedResult, Status};
        if self.state.feed(event.keysym) == FeedResult::Ignored {
            return Composed::Pass;
        }
        match self.state.status() {
            Status::Composing => Composed::Swallow,
            Status::Composed => {
                let text = self.state.utf8().filter(|s| !s.is_empty());
                let keysym = self.state.keysym();
                // After a finished (or cancelled) sequence the next feed starts
                // fresh; reset now so a read of the state cannot see the last
                // one.
                self.state.reset();
                match text {
                    Some(text) => Composed::Text(KeyEvent {
                        time: event.time,
                        raw_code: event.raw_code,
                        // The composed keysym for a reader that reads keysyms
                        // (`keys::key_of`); the first character of the text when
                        // the sequence names no single keysym.
                        keysym: keysym
                            .filter(|k| *k != Keysym::NoSymbol)
                            .or_else(|| text.chars().next().map(Keysym::from_char))
                            .unwrap_or(event.keysym),
                        utf8: Some(text),
                    }),
                    None => Composed::Swallow,
                }
            }
            Status::Cancelled => {
                self.state.reset();
                Composed::Swallow
            }
            Status::Nothing => Composed::Pass,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pressed(keysym: Keysym) -> KeyEvent {
        KeyEvent {
            time: 0,
            raw_code: 0,
            keysym,
            utf8: None,
        }
    }

    /// A compose state over ONE sequence, built in memory so the test needs no
    /// locale or system Compose file.
    fn one_rule() -> Compose {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let buffer = "<Multi_key> <apostrophe> <e> : \"é\" eacute\n";
        let table = xkb::compose::Table::new_from_buffer(
            &context,
            buffer.as_bytes(),
            "C",
            xkb::compose::FORMAT_TEXT_V1,
            xkb::compose::COMPILE_NO_FLAGS,
        )
        .expect("the one-rule compose table compiles");
        Compose {
            state: xkb::compose::State::new(&table, xkb::compose::STATE_NO_FLAGS),
        }
    }

    /// THE DEFECT, END TO END: `Multi_key` then `'` then `e` is `é`, the first
    /// two keys type nothing, and a plain key is itself.
    #[test]
    fn multi_key_apostrophe_e_composes_e_acute() {
        let mut compose = one_rule();
        assert!(
            matches!(compose.feed(&pressed(Keysym::Multi_key)), Composed::Swallow),
            "the Compose key types nothing on its own"
        );
        assert!(
            matches!(
                compose.feed(&pressed(Keysym::apostrophe)),
                Composed::Swallow
            ),
            "a key mid-sequence types nothing"
        );
        match compose.feed(&pressed(Keysym::e)) {
            Composed::Text(made) => assert_eq!(made.utf8.as_deref(), Some("é")),
            other => panic!("expected the composed é, got {other:?}"),
        }
        // The sequence is over: a plain key is itself again.
        assert!(matches!(compose.feed(&pressed(Keysym::a)), Composed::Pass));
    }

    /// A SEQUENCE THAT CANNOT FINISH is abandoned, and the key that broke it
    /// types nothing.
    #[test]
    fn a_broken_sequence_types_nothing() {
        let mut compose = one_rule();
        assert!(matches!(
            compose.feed(&pressed(Keysym::Multi_key)),
            Composed::Swallow
        ));
        assert!(
            matches!(compose.feed(&pressed(Keysym::z)), Composed::Swallow),
            "`Multi_key` then an unknown key is cancelled, and types nothing"
        );
        assert!(
            matches!(compose.feed(&pressed(Keysym::a)), Composed::Pass),
            "and the keyboard types again"
        );
    }
}
