//! The network view's key table, read as presses — one place, so the two
//! doors cannot answer differently to the same letter.
//!
//! The shape `bluetooth_words` already has, and for its reason: the table in
//! `network::OF_THE_VIEW` is what the hint line is generated from and what
//! the manual names, and a letter taught there that no press answers to is a
//! key that does nothing.

/// The view's press behind one KEY — what both doors call, the footer on the
/// key its `translate` made and the window on `door::keys::key_of`.
///
/// # By the whole notation, modifiers included
///
/// The footer used to look the table up by `Key::simple(sym)`, which drops
/// the modifiers, and the window by the key's TEXT, which an arrow has none
/// of: so `S-<right>` and `S-<left>` — taught on Tor's sheet — did nothing in
/// either door, from the keyboard or from a click on the hint. Read by its
/// notation, a key is the table's row or no row, and a chord that is not in
/// the table (`C-s`) is not taken for the letter under it. Caps Lock is
/// already undone by both readers (`door::keys::without_caps_lock`).
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<crate::network::Press> {
    let notation = key.notation();
    crate::network::OF_THE_VIEW
        .iter()
        .find(|(taught, _, _)| *taught == notation)
        .and_then(|(_, id, _)| press_of(id))
}

/// The view's press behind one of its table's ids.
#[must_use]
pub fn press_of(id: &str) -> Option<crate::network::Press> {
    use crate::network::Press;
    Some(match id {
        "network-down" => Press::Down,
        "network-up" => Press::Up,
        "network-connect" => Press::Enter,
        "network-scan" => Press::Scan,
        "network-wifi-toggle" => Press::Radio,
        "network-disconnect" => Press::Disconnect,
        "network-forget" => Press::Forget,
        "network-airplane" => Press::Airplane,
        "network-link" => Press::Facts,
        "network-lan" => Press::Discover,
        "network-import" => Press::Import,
        "network-section-next" => Press::Section,
        "network-section-previous" => Press::SectionBack,
        // TOR'S SHEET. `ESC` is in `OF_THE_VIEW` because the sheet has to
        // TEACH it, and this row is also how both doors read the key on every
        // screen (`press_of_key`): one level back — out of the sheet, out of
        // a file's reading, `no` to "keep it?" — and then the view closes.
        "network-tor-back" => Press::Escape,
        "network-tor-more" => Press::More,
        "network-tor-less" => Press::Less,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_in_the_table_does_something() {
        for (notation, id, _) in crate::network::OF_THE_VIEW {
            assert!(
                press_of(id).is_some(),
                "`{notation}` teaches `{id}`, and no press answers to it"
            );
        }
    }

    /// EVERY KEY THE TABLE TEACHES IS READ AS ITSELF, by the reader both doors
    /// call — `S-<right>` and `S-<left>` above all, which the two doors used to
    /// read as a bare arrow or as nothing.
    #[test]
    fn every_taught_key_is_read_back_as_its_own_press() {
        for (notation, id, _) in crate::network::OF_THE_VIEW {
            let key = ronoctua_keymap::Key::parse(notation).expect(notation);
            assert_eq!(press_of_key(key), press_of(id), "`{notation}`");
        }
        let key = |n: &str| ronoctua_keymap::Key::parse(n).expect(n);
        assert_eq!(
            press_of_key(key("S-<right>")),
            Some(crate::network::Press::More)
        );
        assert_eq!(
            press_of_key(key("S-<left>")),
            Some(crate::network::Press::Less)
        );
        // THE BARE ARROWS ARE NOT THE SHIFTED ONES: they are not in the table,
        // and walking is `<up>`/`<down>`.
        assert_eq!(press_of_key(key("<right>")), None);
        // A CHORD IS NOT THE LETTER UNDER IT.
        assert_eq!(press_of_key(key("C-s")), None);
        assert_eq!(
            press_of_key(key("S-TAB")),
            Some(crate::network::Press::SectionBack)
        );
    }

    /// THE WINDOW'S KEYBOARD, as the door reads it: `Shift+Right` is `More`,
    /// and with Caps Lock on the view's letters are still its letters.
    #[test]
    fn the_windows_keyboard_reaches_the_same_presses() {
        use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym, Modifiers};
        let pressed = |keysym: Keysym, utf8: Option<&str>, mods: Modifiers| {
            let event = KeyEvent {
                time: 0,
                raw_code: 0,
                keysym,
                utf8: utf8.map(str::to_owned),
            };
            let (event, mods) = crate::door::keys::as_read(&event, mods, false);
            crate::door::keys::key_of(&event, mods).and_then(press_of_key)
        };
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        assert_eq!(
            pressed(Keysym::Right, None, shift),
            Some(crate::network::Press::More)
        );
        assert_eq!(
            pressed(Keysym::Left, None, shift),
            Some(crate::network::Press::Less)
        );
        let caps = Modifiers {
            caps_lock: true,
            ..Modifiers::default()
        };
        assert_eq!(
            pressed(Keysym::S, Some("S"), caps),
            Some(crate::network::Press::Scan)
        );
        assert_eq!(
            pressed(Keysym::KP_Enter, Some("\r"), Modifiers::default()),
            Some(crate::network::Press::Enter)
        );
        assert_eq!(
            pressed(Keysym::ISO_Left_Tab, None, shift),
            Some(crate::network::Press::SectionBack)
        );
    }
}
