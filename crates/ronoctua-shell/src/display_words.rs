//! The sentences the screens' doors share.
//!
//! One place, so the footer and the window cannot say a refusal two ways —
//! `power_words.rs`'s arrangement, and its reason. Not every sentence started
//! here: `went_away` was formatted by each door separately until a reviewer
//! read the two, which is what the header used to claim never happened.

use ronoctua_display::{Refusal, Refused};

/// The key a letter of `display::OF_THE_VIEW` stands for.
///
/// THE TABLE IS THE TRUTH and this is the only translation of it. A letter
/// the table teaches and this does not answer is a key the hint line promises
/// and the keyboard swallows — which is worse than a key that was never
/// taught, and the test below is what refuses it.
#[must_use]
pub fn press_of(id: &str) -> Option<crate::display::Press> {
    use crate::display::Press;
    Some(match id {
        "display-up" => Press::Up,
        "display-down" => Press::Down,
        "display-less" => Press::Less,
        "display-more" => Press::More,
        "display-enter" => Press::Enter,
        "display-automatic" => Press::Automatic,
        "display-forget" => Press::Forget,
        "display-arrange" => Press::Arrange,
        _ => return None,
    })
}

/// What a door says when the screen a question was about is not there.
///
/// HERE, because the two doors were formatting it themselves — the one
/// sentence about a question dying, said in two places, in a module whose
/// whole point is that they cannot say a refusal two ways.
///
/// # And there are TWO sentences, because there are two endings
///
/// `applied` is whether the question had changed anything — the same flag the
/// doors read to decide whether to send `Order::Undo`. It gained that flag and
/// the sentence did not, so the door that correctly sent nothing said "the
/// others were put back" anyway: a report of an action taken, on the one path
/// whose whole point is that it must not be taken.
///
/// AND NEITHER SENTENCE NAMES THE OTHER SCREENS ANY MORE. `Undo` re-says the
/// file for every screen with a preview standing, and the vanished one is
/// skipped inside that loop — so on the ordinary path, where it is the only
/// one previewed, nothing is sent at all. What is true either way is what the
/// person can see: the monitor is gone, and so is whatever was being tried
/// on it.
#[must_use]
pub fn went_away(at: &str, applied: bool, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        if applied {
            ronoctua_i18n::key::DISPLAY_SCREEN_GONE
        } else {
            ronoctua_i18n::key::DISPLAY_SCREEN_GONE_ALONE
        },
        &[("screen", at)],
    )
}

/// What the message line says when the screens refused an order.
///
/// # A NAMED refusal says its name and stops there
///
/// `Refused` carries two things: a reason with a key in every language, and
/// the WORDS the crate wrote beside it. Those words are English — they are a
/// `bail!` or a `.context`, which invariant 18 requires them to be — so
/// putting them on the screen next to the translated reason gave a pt-BR
/// machine "o brilho desta tela está nos botões dela (this screen's
/// brightness is on its own buttons)": the same sentence twice, once in the
/// wrong language. That is invariant 8, and the five named reasons are the
/// ones that do not need them: their key already says the whole fact.
///
/// `Other` keeps them, and has to: it is the refusal with no name, and the
/// words are the only thing that says what went wrong. They are still
/// English, which is this view inheriting a shape `power`, `network` and the
/// radio all have — recorded in `docs/PENDING.md` rather than redesigned
/// here, because it is four views' plumbing and one of them is not this one.
#[must_use]
pub fn refused(why: &Refused, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    let reason = texts.t(match why.why {
        Refusal::NoCompositor => key::DISPLAY_REFUSED_NO_COMPOSITOR,
        Refusal::NoSuchScreen => key::DISPLAY_REFUSED_NO_SCREEN,
        Refusal::NoLight => key::DISPLAY_REFUSED_NO_LIGHT,
        Refusal::NotWritable => key::DISPLAY_REFUSED_NOT_WRITABLE,
        Refusal::NoPlaceToWrite => key::DISPLAY_REFUSED_NOWHERE,
        Refusal::Other => key::DISPLAY_REFUSED_OTHER,
    });
    if why.why == Refusal::Other {
        return texts.t_with(
            key::ECHO_DISPLAY_REFUSED,
            &[("why", &reason), ("words", &why.words)],
        );
    }
    texts.t_with(key::ECHO_DISPLAY_REFUSED_PLAIN, &[("why", &reason)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vanished_screen_says_which_of_the_two_things_happened() {
        // THE MODULE'S OTHER REASON FOR EXISTING, and it had no test. The flag
        // was added so a door that correctly sends no `Undo` does not say "the
        // others were put back" — and nothing asserted the branch. Two keys
        // that had collapsed onto one would have compiled and passed.
        let t = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let applied = went_away("DP-1", true, &t);
        let untouched = went_away("DP-1", false, &t);
        assert_ne!(
            applied, untouched,
            "the two endings say the same thing: {applied}",
        );
        for said in [&applied, &untouched] {
            assert!(said.contains("DP-1"), "{said}");
        }
        // AND EACH IS THE KEY IT IS MEANT TO BE. Against the FORMATTED key
        // rather than a phrase typed here: a literal in a test is the same
        // string in two places, and the one in the `.ftl` is the one a person
        // reads.
        let of = |k| t.t_with(k, &[("screen", "DP-1")]);
        assert_eq!(applied, of(ronoctua_i18n::key::DISPLAY_SCREEN_GONE));
        assert_eq!(untouched, of(ronoctua_i18n::key::DISPLAY_SCREEN_GONE_ALONE));
    }

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
            said.contains(&t.t(ronoctua_i18n::key::DISPLAY_REFUSED_NO_LIGHT)),
            "{said}"
        );

        // AND THE UNNAMED ONE KEEPS THEM, because they are the only thing that
        // says what went wrong.
        let other = Refused {
            why: Refusal::Other,
            words: "the socket went away".to_owned(),
        };
        let said = refused(&other, &t);
        assert!(said.contains("the socket went away"), "{said}");
    }

    #[test]
    fn every_key_in_the_table_does_something() {
        // A LETTER THE HINT LINE TEACHES AND THE KEYBOARD SWALLOWS is worse
        // than a key nobody was told about: the screen says it works. The
        // table is written once and read by both, so this is the only place
        // the two can disagree.
        for (notation, id, _) in crate::display::OF_THE_VIEW {
            let Some(press) = press_of(id) else {
                panic!("`{notation}` teaches `{id}`, and nothing answers to it");
            };
            // AND IT ANSWERS WITH THE RIGHT ONE. `is_some()` alone was the
            // whole assertion, so `"display-forget" => Press::Arrange` passed
            // it: a letter that is taught, is answered, and does somebody
            // else's job — which is worse than a swallowed key, because the
            // screen changes and it is not what the line promised.
            //
            // The two names are one word apart by construction: an id is
            // `display-<verb>` and the variant is that verb. Comparing them is
            // what makes this a mapping test rather than a presence test.
            let verb = id
                .strip_prefix("display-")
                .expect("every id in the table is `display-<verb>`");
            assert!(
                format!("{press:?}").eq_ignore_ascii_case(&verb.replace('-', "")),
                "`{notation}` teaches `{id}` and it does `{press:?}`"
            );
        }
    }

    #[test]
    fn no_two_rows_of_the_table_share_a_key() {
        // Two rows with one notation is a key whose meaning depends on which
        // was written first, and the hint line would teach only one of them.
        let mut seen: Vec<&str> = crate::display::OF_THE_VIEW
            .iter()
            .map(|(notation, _, _)| *notation)
            .collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), before, "two rows of the table share a key");
    }
}
