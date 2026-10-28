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
//!
//! # And back: ONE reader for every window
//!
//! [`key_of`] is the other direction — what the keyboard sent, as the keymap
//! writes it — and [`as_read`] is what every window's key goes through first
//! (`Door::typed`). Each window used to carry its own copy of the reader, and
//! the copies had the same three holes: `Delete` typed U+007F, Caps Lock
//! turned every `C-<letter>` into a `C-<LETTER>` nothing binds, and a view's
//! letter keys stopped answering with the Lock on — while the footer, which
//! undoes the Lock, went on answering. Fixed here, they are fixed in every
//! door.

use ronoctua_keymap::{Key, Mods, Sym};
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym, Modifiers};

/// A `KeyEvent` as the keymap writes it — the notation a view's table and
/// the header's clicks ([`event_of`]) both speak.
///
/// The same mapping `Footer::translate` makes, minus the leader's inheritance:
/// that exists because the footer receives the keyboard with the leader still
/// held down, and a window is given focus like any other.
///
/// SHIFT ENTERS FROM THE KEYSYM for a character, which xkb has already
/// shifted — counting both would turn `M-S` into `M-S-S` and never match —
/// and FROM THE MODIFIER for every other key, where xkb has nothing to apply
/// it to: `ISO_Left_Tab` is Shift+Tab already applied, and `S-<left>` read as
/// a bare `<left>` walked the list instead of turning a shape.
#[must_use]
pub fn key_of(event: &KeyEvent, mods: Modifiers) -> Option<Key> {
    let sym = match event.keysym {
        Keysym::Escape => Sym::Esc,
        Keysym::Return | Keysym::KP_Enter => Sym::Enter,
        Keysym::Tab | Keysym::ISO_Left_Tab => Sym::Tab,
        Keysym::BackSpace => Sym::Backspace,
        // THE DELETE KEY IS A DELETE KEY: the notation has one symbol for
        // "remove" (`DEL` = `Sym::Backspace`), and the hand that reaches for
        // `Delete` means the same as the one that reaches for `Backspace`. xkb
        // gives `Delete` the text U+007F, which `char_of` throws away, so
        // without this line it fell through to a character nobody can see and
        // the key did nothing at all.
        Keysym::Delete | Keysym::KP_Delete => Sym::Backspace,
        Keysym::space => Sym::Space,
        Keysym::Left => Sym::Left,
        Keysym::Right => Sym::Right,
        Keysym::Up => Sym::Up,
        Keysym::Down => Sym::Down,
        Keysym::Page_Up | Keysym::KP_Page_Up => Sym::PageUp,
        Keysym::Page_Down | Keysym::KP_Page_Down => Sym::PageDown,
        Keysym::Home | Keysym::KP_Home => Sym::Home,
        Keysym::End | Keysym::KP_End => Sym::End,
        otherwise => Sym::Char(char_of(otherwise)?),
    };
    Some(Key {
        mods: Mods {
            ctrl: mods.ctrl,
            alt: mods.alt,
            shift: event.keysym == Keysym::ISO_Left_Tab
                || (mods.shift && !matches!(sym, Sym::Char(_))),
            logo: mods.logo,
        },
        sym,
    })
}

/// The character a key TYPES, or `None` for a key that types none.
///
/// A CONTROL CHARACTER TYPES NOTHING: `key_char` gives U+007F for `Delete`,
/// and a view that took it as a letter put a character nobody can see into
/// its search — and, held, one more per repeat.
#[must_use]
pub fn char_of(keysym: Keysym) -> Option<char> {
    keysym.key_char().filter(|c| !c.is_control())
}

/// Undoes the Caps Lock over the character xkb produced.
///
/// # Why the Lock must not decide a key
///
/// The keymap tells case apart on purpose — `w h` focuses the window on the
/// left, `w H` MOVES it there. That distinction is **Shift**: one more
/// gesture, deliberate, at the moment. The Caps Lock is a state that stays
/// on, that gets switched on by accident, and that the person forgot they
/// switched on.
///
/// With it on and without this function, xkb delivers `I` where `i` was
/// typed, **every** lowercase binding dies at once, and the only thing on
/// screen is `CTRL ALT SPC I is not bound` — which shows the consequence and
/// hides the cause. Worse still on the ones that exist in both cases: `w h`
/// would become `w H`, and the window would MOVE when the request was to
/// focus. A key that does something else is worse than one that does nothing.
///
/// Inverting the case is exactly undoing the Lock: with it on xkb has already
/// applied the inversion, and the Shift along with it undid it. Only letters
/// change. The footer's reader (`Footer::translate`) and every window's
/// ([`as_read`]) call this one function.
#[must_use]
pub fn without_caps_lock(c: char, caps: bool) -> char {
    if !caps {
        return c;
    }
    // `next()` and not `collect()`: they are 1:1 mappings in the ASCII and
    // Latin-1 the keymap uses; German's `ß`/`SS` is not a shortcut key, and
    // falling on the first character is better than allocating.
    if c.is_uppercase() {
        c.to_lowercase().next().unwrap_or(c)
    } else if c.is_lowercase() {
        c.to_uppercase().next().unwrap_or(c)
    } else {
        c
    }
}

/// Whether Caps Lock is UNDONE on a key — on a chord always, and on a plain
/// key unless the door types text where the Lock means capitals. The one
/// answer the windows ([`as_read`]) and the footer (`Footer::translate`)
/// read, so a field types the same capitals in every door.
#[must_use]
pub const fn lock_undone(mods: Modifiers, types_capitals: bool) -> bool {
    let chord = mods.ctrl || mods.alt || mods.logo;
    mods.caps_lock && (chord || !types_capitals)
}

/// A key as EVERY window reads it, before the window sees it.
///
/// - **Caps Lock is undone**, as the footer undoes it: on a chord always —
///   `C-v` with the Lock on is still `C-v` — and on a plain key unless the
///   window types TEXT where the Lock means capitals (`types_capitals`: a
///   terminal, a buffer's editor). The keysym and the text are turned
///   together, so a window that reads either reads the same key.
/// - **`Delete` carries no text**: its text is U+007F, a character nobody can
///   see, and a window that fell back on the text typed it.
#[must_use]
pub fn as_read(event: &KeyEvent, mods: Modifiers, types_capitals: bool) -> (KeyEvent, Modifiers) {
    let mut read = event.clone();
    let mut held = mods;
    if matches!(read.keysym, Keysym::Delete | Keysym::KP_Delete) {
        read.utf8 = None;
    }
    if lock_undone(mods, types_capitals) {
        held.caps_lock = false;
        if let Some(c) = char_of(read.keysym) {
            let undone = without_caps_lock(c, true);
            if undone != c {
                read.keysym = Keysym::from_char(undone);
                if read.utf8.as_deref() == Some(c.encode_utf8(&mut [0; 4])) {
                    read.utf8 = Some(undone.to_string());
                }
            }
        }
    }
    (read, held)
}

/// The key, as a `KeyEvent` and the modifiers held with it.
///
/// Shift enters the way [`key_of`] reads it back: from the KEYSYM
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
        Sym::PageUp => (Keysym::Page_Up, None),
        Sym::PageDown => (Keysym::Page_Down, None),
        Sym::Home => (Keysym::Home, None),
        Sym::End => (Keysym::End, None),
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
    fn pressed(keysym: Keysym, utf8: Option<&str>) -> KeyEvent {
        KeyEvent {
            time: 0,
            raw_code: 0,
            keysym,
            utf8: utf8.map(str::to_owned),
        }
    }

    /// `Delete` IS THE DELETE KEY, and types NO CHARACTER. The notation has one
    /// "remove" symbol (`Sym::Backspace`), so `Delete` reads as that — it
    /// forgets and erases wherever `Backspace` does. And the old defect stays
    /// fixed: it still types nothing (xkb's U+007F is a control character
    /// `char_of` throws away, and a window's text reader strips it), rather than
    /// putting a character nobody can see into a search.
    #[test]
    fn delete_is_the_delete_key_and_types_no_character() {
        let e = pressed(Keysym::Delete, Some("\u{7f}"));
        assert_eq!(
            key_of(&e, Modifiers::default()).map(|k| k.sym),
            Some(Sym::Backspace),
            "Delete is the remove key, the same one Backspace is"
        );
        let (read, _) = as_read(&e, Modifiers::default(), false);
        assert_eq!(read.utf8, None, "Delete types no character");
        assert_eq!(
            read.keysym,
            Keysym::Delete,
            "the key itself is still Delete"
        );
        assert_eq!(char_of(Keysym::Delete), None);
        assert_eq!(char_of(Keysym::a), Some('a'));
    }

    /// WITH CAPS LOCK ON, `C-v` is still `C-v` and a view's `f` is still `f`
    /// — in every window, as in the footer.
    #[test]
    fn caps_lock_decides_no_key_of_a_window() {
        let caps = Modifiers {
            caps_lock: true,
            ..Modifiers::default()
        };
        let ctrl_caps = Modifiers { ctrl: true, ..caps };
        // xkb's answer with the Lock on: the capital.
        let (read, held) = as_read(&pressed(Keysym::V, Some("\u{16}")), ctrl_caps, false);
        assert_eq!(read.keysym, Keysym::v);
        assert!(!held.caps_lock);
        assert_eq!(
            key_of(&read, held).map(|k| k.notation()),
            Some("C-v".to_owned())
        );
        // A PLAIN LETTER of a view: its keysym and its text, both.
        let (read, held) = as_read(&pressed(Keysym::F, Some("F")), caps, false);
        assert_eq!(read.keysym, Keysym::f);
        assert_eq!(read.utf8.as_deref(), Some("f"));
        assert_eq!(
            key_of(&read, held).map(|k| k.notation()),
            Some("f".to_owned())
        );
        // AND SHIFT WITH THE LOCK is the capital, as the footer reads it.
        let (read, _) = as_read(&pressed(Keysym::h, Some("h")), caps, false);
        assert_eq!(read.keysym, Keysym::H);
        // A digit has no case.
        let (read, _) = as_read(&pressed(Keysym::_3, Some("3")), caps, false);
        assert_eq!(read.keysym, Keysym::_3);
    }

    /// A WINDOW THAT TYPES CAPITALS keeps them — but not on a chord.
    #[test]
    fn a_window_that_types_capitals_keeps_the_lock_but_not_on_a_chord() {
        let caps = Modifiers {
            caps_lock: true,
            ..Modifiers::default()
        };
        let (read, held) = as_read(&pressed(Keysym::A, Some("A")), caps, true);
        assert_eq!(read.keysym, Keysym::A);
        assert!(held.caps_lock);
        let (read, _) = as_read(
            &pressed(Keysym::R, Some("\u{12}")),
            Modifiers { ctrl: true, ..caps },
            true,
        );
        assert_eq!(read.keysym, Keysym::r);
    }

    /// THE READER AND THE CLICK agree: every key a hint can name is read back
    /// as itself, `S-TAB` and `S-<left>` with their shift and `S` without one.
    #[test]
    fn a_key_is_read_back_as_the_notation_it_was_pressed_as() {
        for notation in [
            "RET", "ESC", "TAB", "S-TAB", "C-w", "C-S", "d", "S", "<up>", "S-<left>", "DEL", "SPC",
            "C-RET", "M-x", "<prior>", "<next>", "<home>", "<end>",
        ] {
            let (event, mods) = event_of(Key::parse(notation).expect(notation));
            let read = key_of(&event, mods).expect(notation);
            assert_eq!(read.notation(), notation);
        }
    }
}
