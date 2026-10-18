//! The sentences the screens' doors share.
//!
//! One place, so the footer and the window cannot say a refusal two ways —
//! `power_words.rs`'s arrangement, and its reason. Not every sentence started
//! here: `went_away` was formatted by each door separately until a reviewer
//! read the two, which is what the header used to claim never happened.

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
        "display-rise" => Press::Rise,
        "display-sink" => Press::Sink,
        _ => return None,
    })
}

/// A key as the view's press — the ONE reader both doors call, from the
/// keyboard or from a click on the hint.
///
/// The arrows, `RET` and `Esc` are read whatever is held, as both doors
/// always read them. A LETTER is read by its whole notation out of
/// `display::OF_THE_VIEW`, and that is where the two doors used to part: the
/// footer dropped the modifiers and took `C-a` for `a`, while the window read
/// the character xkb typed and took `C-a` for nothing. A chord that is not in
/// the table is now nothing in both.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<crate::display::Press> {
    use crate::display::Press;
    use ronoctua_keymap::Sym;
    Some(match key.sym {
        Sym::Esc => Press::Escape,
        // THE SHIFTED PAIR IS THE OTHER AXIS on a place row (`OF_THE_VIEW`),
        // read before the bare arrows that are the cursor.
        Sym::Up if key.mods.shift => Press::Rise,
        Sym::Down if key.mods.shift => Press::Sink,
        Sym::Up => Press::Up,
        Sym::Down => Press::Down,
        Sym::Left => Press::Less,
        Sym::Right => Press::More,
        Sym::Enter => Press::Enter,
        _ => {
            let notation = key.notation();
            let id = crate::display::OF_THE_VIEW
                .iter()
                .find(|(taught, _, _)| *taught == notation)
                .map(|(_, id, _)| *id)?;
            press_of(id)?
        }
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
            crate::key::DISPLAY_SCREEN_GONE
        } else {
            crate::key::DISPLAY_SCREEN_GONE_ALONE
        },
        &[("screen", at)],
    )
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
        assert_eq!(applied, of(crate::key::DISPLAY_SCREEN_GONE));
        assert_eq!(untouched, of(crate::key::DISPLAY_SCREEN_GONE_ALONE));
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

    /// THE TWO DOORS READ A KEY ONE WAY: every row of the table is its own
    /// press, the arrows and `RET` are read whatever is held, and a chord is
    /// not the letter under it.
    #[test]
    fn both_doors_read_a_key_one_way() {
        use crate::display::Press;
        let read = |n: &str| press_of_key(ronoctua_keymap::Key::parse(n).expect("a key"));
        for (taught, id, _) in crate::display::OF_THE_VIEW {
            assert_eq!(read(taught), press_of(id), "`{taught}`");
        }
        assert_eq!(read("S-<left>"), Some(Press::Less));
        assert_eq!(read("S-<up>"), Some(Press::Rise));
        assert_eq!(read("S-<down>"), Some(Press::Sink));
        assert_eq!(read("ESC"), Some(Press::Escape));
        assert_eq!(read("C-a"), None, "a chord was taken for its letter");
        assert_eq!(read("M-d"), None, "a chord was taken for its letter");
        assert_eq!(read("x"), None);
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
