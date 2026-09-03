//! The words the host resolved for a buffer, and how a buffer reads them back.
//!
//! # Why a buffer holds translated text at all
//!
//! Because `Text::Key` only reaches the PERSON. It is resolved when the host
//! draws, and the buffer never holds what came out — which covers everything a
//! buffer only shows, and does not cover text the buffer also HOLDS and READS
//! BACK.
//!
//! The three browsers' search field is that text: the buffer keeps the string,
//! parses it, and the person edits it. Written as data it opened
//! `familia:emoji` in an en-GB session; written as a key the buffer could not
//! have re-read what it drew.
//!
//! So the buffer NAMES the keys in its descriptor and the host answers with the
//! text, exactly as it answers a paste. The rule the invariant protects is
//! untouched: what is here was written by the host, never by the component.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

/// The keys a buffer asked for, with the text that came back.
///
/// A list and not a map: it is ten entries at the most, a lookup happens once
/// per keystroke, and a map would cost the component a hasher it has no other
/// use for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Phrases(Vec<(String, String)>);

impl Phrases {
    /// What the host sent, in the order it sent it.
    #[must_use]
    pub fn of(said: Vec<(String, String)>) -> Self {
        Self(said)
    }

    /// The text for a key, or the FALLBACK the buffer compiled in.
    ///
    /// Never an `Option`, and that is the point: a buffer that had to decide
    /// what to do without a phrase would be a buffer with two behaviours, and
    /// the one nobody exercises is the one that breaks. The fallback is the
    /// English word, which is what invariant 18 asks for where a single
    /// language fits — so a host that sends nothing (a driver, an old build)
    /// gets the same screen the tree had before this existed.
    #[must_use]
    pub fn or(&self, key: &str, fallback: &'static str) -> String {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map_or_else(|| fallback.to_owned(), |(_, t)| t.clone())
    }

    /// Is there anything in here?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn a_phrase_that_arrived_wins_over_the_fallback() {
        let p = Phrases::of(vec![(
            "unicode-prefix-family".to_owned(),
            "familia".to_owned(),
        )]);
        assert_eq!(p.or("unicode-prefix-family", "family"), "familia");
    }

    /// THE EMPTY CASE IS THE ONE THAT SHIPS TO A DRIVER, and it must not be a
    /// second behaviour: with nothing sent, the buffer writes exactly what it
    /// wrote before phrases existed.
    #[test]
    fn nothing_sent_leaves_the_english_word() {
        let p = Phrases::default();
        assert!(p.is_empty());
        assert_eq!(p.or("unicode-prefix-family", "family"), "family");
        assert_eq!(p.or("whatever", "block"), "block");
    }
}
