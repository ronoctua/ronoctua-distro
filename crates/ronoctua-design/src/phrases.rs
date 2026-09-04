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

    /// The reserved key by which the host tells the buffer WHICH language the
    /// reader is in.
    ///
    /// # Why a `-` in front, and why that makes it safe
    ///
    /// In Fluent a name beginning with `-` is a TERM, and `gen-i18n.py`'s key
    /// pattern (`^[a-z][a-z0-9-]*\s*=`) excludes terms on purpose — they are
    /// not addressable messages. So `-language` cannot collide with a text key
    /// BY CONSTRUCTION, rather than by nobody having used the name yet, and
    /// `hand_phrases` — which is what refuses a declared key the `.ftl` lacks,
    /// and would otherwise refuse this one — walks straight past it without an
    /// exception written by hand.
    ///
    /// # What it does NOT license
    ///
    /// It is not the buffer learning to translate (invariant 11). The value is
    /// a tag, never a sentence, and the only thing a buffer may do with it is
    /// INDEX data it was already holding — pick which of its own tag lists to
    /// show first. A buffer that used it to choose between two strings it wrote
    /// itself would be writing UI text in a language, which is the thing the
    /// invariant forbids and which no amount of tagging makes acceptable.
    pub const LANGUAGE: &'static str = "-language";

    /// Which of `spoken` the reader's language is, or `0`.
    ///
    /// `0` and not `None`: the first is the SOURCE language, which every record
    /// has, so an unseeded buffer behaves exactly as it did before — which is
    /// the same bargain [`Self::or`] makes with its fallback.
    ///
    /// The match is on the SUBTAG (`pt-BR` finds `pt`), because the tag lists
    /// are keyed on the language and not on the region.
    #[must_use]
    pub fn mine(&self, spoken: &str) -> usize {
        let said = self.or(Self::LANGUAGE, "");
        let Some(want) = said.split('-').next().map(str::to_lowercase) else {
            return 0;
        };
        spoken
            .split(',')
            .position(|s| s.eq_ignore_ascii_case(&want))
            .unwrap_or(0)
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
