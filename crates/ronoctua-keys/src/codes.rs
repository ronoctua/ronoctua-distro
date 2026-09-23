//! keyd's words for keys, as the kernel's key codes.
//!
//! The vocabulary is `assets/keyboards.nix`, written in keyd's words because
//! Together mode writes it into keyd's files. Independent mode reads the SAME
//! words, and needs the kernel's number behind each one — `input-event-codes.h`.
//!
//! The table is the words the vocabulary uses and nothing more. A word the
//! vocabulary gains and this table lacks is a test that fails
//! (`every_word_of_the_vocabulary_is_a_key`), never a key that quietly does
//! nothing.

/// `KEY_LEFTMETA` — what `meta` means, the modifier by the space bar.
pub const LEFTMETA: u16 = 125;
/// `KEY_F13` — the key keyd emits for the leader; here it is the leader.
pub const F13: u16 = 183;
/// The highest key code the kernel defines (`KEY_MAX`).
pub const MAX: u16 = 0x2ff;

/// Whether a code is a BUTTON and not a key — `BTN_MISC` to `BTN_GEAR_UP`
/// (mice, joysticks, pens, `0x100..=0x15f`) and `BTN_TRIGGER_HAPPY`
/// (`0x2c0..=0x2e7`). A twin declares none of them: a device that can press
/// `BTN_LEFT` is a pointer to libinput.
#[must_use]
pub const fn is_a_button(code: u16) -> bool {
    matches!(code, 0x100..=0x15f | 0x2c0..=0x2e7)
}

/// The kernel's code for one of keyd's words.
#[must_use]
pub fn of(word: &str) -> Option<u16> {
    Some(match word {
        "esc" => 1,
        "backspace" => 14,
        "leftcontrol" | "control" => 29,
        "leftalt" | "alt" => 56,
        "capslock" => 58,
        "rightalt" => 100,
        "insert" => 110,
        "leftmeta" | "meta" => LEFTMETA,
        "rightmeta" => 126,
        "menu" => 139,
        "f13" => F13,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_twin_declares_keys_and_no_button() {
        assert!(is_a_button(0x110), "BTN_LEFT");
        assert!(is_a_button(0x2c0), "BTN_TRIGGER_HAPPY");
        assert!(!is_a_button(30), "KEY_A");
        assert!(
            !is_a_button(0x160),
            "KEY_OK, the first key after the buttons"
        );
        assert!(!is_a_button(F13));
    }

    #[test]
    fn every_word_of_the_vocabulary_is_a_key() {
        for (key, _) in ronoctua_keyboards::KEYS {
            assert!(of(key).is_some(), "no code for the key `{key}`");
        }
        for (word, _, _) in ronoctua_keyboards::CHOICES {
            if word.is_empty() || *word == "noop" {
                continue;
            }
            assert!(
                crate::engine::Action::parse(word).is_some(),
                "the choice `{word}` does not parse"
            );
        }
        assert!(of(ronoctua_keyboards::LEADER.0).is_some());
        assert!(crate::engine::Action::parse(ronoctua_keyboards::LEADER.1).is_some());
    }
}
