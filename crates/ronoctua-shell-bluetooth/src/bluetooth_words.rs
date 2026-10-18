//! The view's key table, read as presses — the one reader both bluetooth
//! doors call, so the footer's region and the window cannot answer a letter
//! two ways.
//!
//! What the doors SAY of the radio — a refusal of BlueZ in a sentence, the
//! pairing question, the modeline's marks — is the radio's, under this view
//! and the footer both (`bluez_words`, `ronoctua-shell-bluetooth-words`).

/// What a key means in the view — THE ONE READER, which both doors call:
/// the footer with the key it translated, the window with
/// `door::keys::key_of`.
///
/// BY THE WHOLE NOTATION, modifiers and all, looked up in
/// `bluetooth::OF_THE_VIEW`. The footer built the notation with shift alone,
/// so `C-f` arrived as `f` and FORGOT the device under the cursor, while the
/// window matched the text a key typed — a control character under Ctrl — and
/// the same chord did nothing there. A chord no row names is a key this view
/// does not answer, in either door.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<crate::bluetooth::Press> {
    use crate::bluetooth::Press;
    use ronoctua_keymap::Sym;
    Some(match key.sym {
        Sym::Esc => Press::Escape,
        Sym::Up => Press::Up,
        Sym::Down => Press::Down,
        Sym::Enter => Press::Enter,
        _ => {
            let notation = key.notation();
            let id = crate::bluetooth::OF_THE_VIEW
                .iter()
                .find(|(taught, _, _)| *taught == notation)
                .map(|(_, id, _)| *id)?;
            press_of(id)?
        }
    })
}

/// The view's press behind one of its table's ids.
///
/// ONE TABLE, TWO READERS: the key branch reads a notation out of
/// `bluetooth::OF_THE_VIEW` and lands here, and `run_command` reaches the
/// same presses by name. A letter added to the table with no press here does
/// nothing, and the unit test beside the view is what says so.
#[must_use]
pub fn press_of(id: &str) -> Option<crate::bluetooth::Press> {
    use crate::bluetooth::Press;
    Some(match id {
        "bluetooth-down" => Press::Down,
        "bluetooth-up" => Press::Up,
        "bluetooth-device" => Press::Enter,
        "bluetooth-more" => Press::More,
        "bluetooth-less" => Press::Less,
        "bluetooth-settings" => Press::Cross,
        "bluetooth-connect" => Press::Connect,
        "bluetooth-pair" => Press::Pair,
        "bluetooth-forget" => Press::Forget,
        "bluetooth-trust" => Press::Trust,
        "bluetooth-block" => Press::Block,
        "bluetooth-rename" => Press::Rename,
        "bluetooth-scan" => Press::Scan,
        "bluetooth-power" => Press::Power,
        "bluetooth-discoverable" => Press::Discoverable,
        "bluetooth-unnamed" => Press::Unnamed,
        "bluetooth-accept" => Press::Accept,
        "bluetooth-refuse" => Press::Refuse,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_in_the_table_does_something() {
        for (notation, id, _) in crate::bluetooth::OF_THE_VIEW {
            assert!(
                press_of(id).is_some(),
                "`{notation}` teaches `{id}`, and no press answers to it"
            );
        }
    }

    /// FAILS IF: a chord reaches a bare letter's deed — `C-f` forgetting the
    /// device, `C-b` blocking it — or the shift stops telling the arrows'
    /// two meanings apart.
    #[test]
    fn a_chord_is_not_its_letter() {
        use crate::bluetooth::Press;
        let read =
            |notation: &str| press_of_key(ronoctua_keymap::Key::parse(notation).expect(notation));
        for (notation, id, _) in crate::bluetooth::OF_THE_VIEW {
            assert_eq!(read(notation), press_of(id), "`{notation}`");
        }
        for chord in ["C-f", "C-b", "C-p", "C-c", "M-f", "C-y", "C-n"] {
            assert_eq!(read(chord), None, "`{chord}` did a deed");
        }
        assert_eq!(read("<right>"), Some(Press::Cross));
        assert_eq!(read("S-<right>"), Some(Press::More));
        assert_eq!(read("S-<left>"), Some(Press::Less));
        assert_eq!(read("S-TAB"), Some(Press::Cross));
    }
}
