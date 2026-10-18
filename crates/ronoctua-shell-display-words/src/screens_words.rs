//! THE SCREENS AS THE SHELL SAYS THEM — what the footer, which keeps the
//! screens' reader for the session (the brightness keys order it with no
//! view open), and the screens' view (`ronoctua-shell-display`) both say: a
//! refusal of the reader in a sentence, and the step one key moves the
//! brightness by.
//!
//! A leaf under both, on no other shell crate: the footer names no feature,
//! and a line of the view recompiles the view and the binary alone.

use ronoctua_display::{Refusal, Refused};

/// What the message line says when the screens refused an order.
///
/// # Every refusal says its NAME, and only its name
///
/// `Refused` carries two things: a reason with a key in every language, and
/// the WORDS the crate wrote beside it. Those words are English — a `bail!` or
/// a `.context`, which invariant 18 requires them to be — so on a pt-BR
/// machine they put a sentence on the message line in a language the person
/// did not choose (invariant 8). Every way a refusal reaches here has a name
/// now (`ronoctua_display::Refusal`); the one left, `Other`, is the error
/// nobody classified, and its words go to the LOG — where whoever reads a log
/// reads English — while the line says the refusal in the person's language.
#[must_use]
pub fn refused(why: &Refused, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    let reason = texts.t(match why.why {
        Refusal::NoCompositor => key::DISPLAY_REFUSED_NO_COMPOSITOR,
        Refusal::NoSuchScreen => key::DISPLAY_REFUSED_NO_SCREEN,
        Refusal::NoLight => key::DISPLAY_REFUSED_NO_LIGHT,
        Refusal::NotWritable => key::DISPLAY_REFUSED_NOT_WRITABLE,
        Refusal::NoPlaceToWrite => key::DISPLAY_REFUSED_NOWHERE,
        Refusal::CompositorRefused => key::DISPLAY_REFUSED_COMPOSITOR,
        Refusal::UnreadableFile => key::DISPLAY_REFUSED_UNREADABLE,
        Refusal::PanelRefused => key::DISPLAY_REFUSED_PANEL,
        Refusal::MonitorSilent => key::DISPLAY_REFUSED_MONITOR,
        Refusal::Other => key::DISPLAY_REFUSED_OTHER,
    });
    tracing::info!(why = ?why.why, words = %why.words, "the screens refused");
    texts.t_with(key::ECHO_DISPLAY_REFUSED_PLAIN, &[("why", &reason)])
}

/// How far one arrow moves the brightness, in percent.
///
/// Five, which is `power`'s step for the charge thresholds. The keyboard's own
/// brightness keys are a later deliverable and will read this constant rather
/// than carry one, so that holding the key and holding the arrow arrive at the
/// same numbers — written here because the number is what makes that possible,
/// not because those keys exist yet.
pub const STEP: i16 = 5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_a_sentence_of_its_own() {
        // SIX VARIANTS ONTO SIX KEYS, and two of them collapsed onto one would
        // have compiled and passed every other test in the file: a person
        // would be told the wrong reason, which is worse than a generic one.
        let t = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let of = |why| {
            refused(
                &Refused {
                    why,
                    words: "the words".to_owned(),
                },
                &t,
            )
        };
        let all = [
            of(Refusal::NoCompositor),
            of(Refusal::NoSuchScreen),
            of(Refusal::NoLight),
            of(Refusal::NotWritable),
            of(Refusal::NoPlaceToWrite),
            of(Refusal::CompositorRefused),
            of(Refusal::UnreadableFile),
            of(Refusal::PanelRefused),
            of(Refusal::MonitorSilent),
            of(Refusal::Other),
        ];
        let mut seen: Vec<&String> = all.iter().collect();
        seen.sort();
        seen.dedup();
        assert_eq!(
            seen.len(),
            all.len(),
            "two refusals say the same thing: {all:?}"
        );
    }

    #[test]
    fn a_named_refusal_does_not_say_itself_twice() {
        // THE WORDS BESIDE A REASON ARE ENGLISH, because invariant 18 requires
        // every `bail!` and `.context` to be — so drawing them next to the
        // translated reason put the same sentence on the glass twice, once in
        // a language the person did not choose. Invariant 8.
        let t = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
        let named = Refused {
            why: Refusal::NoLight,
            words: "this screen's brightness is on its own buttons".to_owned(),
        };
        let said = refused(&named, &t);
        assert!(
            !said.contains("this screen"),
            "the English words were drawn beside the translated reason: {said}",
        );
        assert!(
            said.contains(&t.t(crate::key::DISPLAY_REFUSED_NO_LIGHT)),
            "{said}"
        );

        // AND THE UNNAMED ONE DOES NOT CARRY THEM EITHER: no sentence on the
        // message line has a half in a language the person did not choose.
        let other = Refused {
            why: Refusal::Other,
            words: "the socket went away".to_owned(),
        };
        let said = refused(&other, &t);
        assert!(!said.contains("the socket went away"), "{said}");
        assert!(
            said.contains(&t.t(crate::key::DISPLAY_REFUSED_OTHER)),
            "{said}"
        );
    }
}
