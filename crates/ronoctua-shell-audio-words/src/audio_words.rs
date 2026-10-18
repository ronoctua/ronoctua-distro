//! PipeWire's words, in the person's language.
//!
//! # Why a refusal is a SENTENCE and not a log line
//!
//! Because from outside the machine, a key that was refused and a key that
//! did nothing look the same. `wpctl` says `Object '83' not found` when a
//! headset is pulled out between the key going down and the order arriving —
//! which is a thing that happens to anybody — and the row it was about is
//! already gone from the screen by the time the answer comes back.
//!
//! So the shape of the refusal becomes our sentence and the program's own
//! words travel beside it as an ARGUMENT, never as a key: a text key is
//! something both languages owe, and `wpctl`'s output is neither ours nor
//! translated.

use ronoctua_audio::Refusal;

/// What to say when an order was refused.
#[must_use]
pub fn refused(why: Refusal, words: &str, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    let reason = texts.t(match why {
        Refusal::Gone => key::AUDIO_REFUSED_GONE,
        Refusal::NotAvailable => key::AUDIO_REFUSED_ABSENT,
        Refusal::Invalid => key::AUDIO_REFUSED_INVALID,
        Refusal::Other => key::AUDIO_REFUSED_OTHER,
    });
    texts.t_with(
        key::ECHO_AUDIO_REFUSED,
        &[("why", &reason), ("words", words)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    #[test]
    fn every_refusal_has_words_of_its_own() {
        let t = texts();
        let said: Vec<String> = [
            Refusal::Gone,
            Refusal::NotAvailable,
            Refusal::Invalid,
            Refusal::Other,
        ]
        .into_iter()
        .map(|why| refused(why, "Object '83' not found", &t))
        .collect();
        for one in &said {
            assert!(!one.contains('['), "a missing key came through: {one}");
            assert!(
                one.contains("Object '83'"),
                "the daemon's words went: {one}"
            );
        }
        // FOUR SHAPES, FOUR SENTENCES. A refusal that read the same as another
        // would be a sentence that answers a question it was not asked.
        let mut unique = said.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), said.len(), "{said:?}");
    }
}
