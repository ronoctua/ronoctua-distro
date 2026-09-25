//! A key of the keymap's notation, as the keyboard would have sent it.
//!
//! # Why a click is turned into a key and not into a command
//!
//! Because a shortcut on a header is a KEY of the window that has it, and what
//! the key does depends on the state the window is in — `RET` opens a folder
//! on one row and plays a film on the next, `S-<left>` goes up a directory in
//! the listing and rewinds in the preview. A click that ran a command of its
//! own would be a second path to the same behaviour, and the two would drift.
//! Pressed through the path the keyboard takes, the click does exactly what the
//! key does, in every state, by construction.

use ronoctua_keymap::{Key, Sym};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym, Modifiers};

/// The key, as a `KeyEvent` and the modifiers held with it.
///
/// Shift enters the way `files_window::key_of` reads it back: from the KEYSYM
/// for a character (xkb has already applied it — `S` is shifted `s`) and for
/// `S-TAB` (`ISO_Left_Tab`), and from the modifier for every other key.
#[must_use]
pub fn event_of(key: Key) -> (KeyEvent, Modifiers) {
    let (keysym, utf8) = match key.sym {
        Sym::Char(c) => (Keysym::from_char(c), Some(c.to_string())),
        Sym::Space => (Keysym::space, Some(" ".to_owned())),
        Sym::Enter => (Keysym::Return, Some("\r".to_owned())),
        Sym::Tab if key.mods.shift => (Keysym::ISO_Left_Tab, None),
        Sym::Tab => (Keysym::Tab, Some("\t".to_owned())),
        Sym::Esc => (Keysym::Escape, None),
        Sym::Backspace => (Keysym::BackSpace, None),
        Sym::Left => (Keysym::Left, None),
        Sym::Right => (Keysym::Right, None),
        Sym::Up => (Keysym::Up, None),
        Sym::Down => (Keysym::Down, None),
    };
    let mods = Modifiers {
        ctrl: key.mods.ctrl,
        alt: key.mods.alt,
        shift: key.mods.shift,
        logo: key.mods.logo,
        ..Modifiers::default()
    };
    (
        KeyEvent {
            time: 0,
            raw_code: 0,
            keysym,
            utf8,
        },
        mods,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EVERY KEY A HINT CAN NAME comes back as the keysym the keyboard sends
    /// for it — the shell's own reading turns it back into the same key.
    #[test]
    fn a_click_is_the_key_it_names() {
        for notation in [
            "RET", "ESC", "TAB", "S-TAB", "C-w", "C-S", "d", "<up>", "S-<left>", "DEL", "SPC",
        ] {
            let key = Key::parse(notation).expect(notation);
            let (event, mods) = event_of(key);
            assert_ne!(event.keysym, Keysym::NoSymbol, "{notation}");
            assert_eq!(mods.ctrl, key.mods.ctrl, "{notation}");
        }
        let (e, _) = event_of(Key::parse("S-TAB").expect("S-TAB"));
        assert_eq!(e.keysym, Keysym::ISO_Left_Tab);
        let (e, _) = event_of(Key::parse("RET").expect("RET"));
        assert_eq!(e.keysym, Keysym::Return);
    }
}
