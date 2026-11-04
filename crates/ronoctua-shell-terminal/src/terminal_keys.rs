//! A key, as the terminal hears it: the few the window keeps, and every
//! other one as the frame the holder encodes.
//!
//! # The window does not encode a key
//!
//! It says what it SAW — the physical key, the modifiers, the text the
//! layout produced — and the holder encodes it against the modes the program
//! actually set (`ronoctua_term::holder`): kitty's flags, modifyOtherKeys,
//! the cursor keys' mode, Alt as Escape. A window whose mirror is a frame
//! behind the holder's modes would otherwise encode a key the old way, and
//! there is no frame of raw bytes for it to try with (`ronoctua_terminals::wire`).
//!
//! # What the door does not say, and how it is made up for
//!
//! The kitty protocol wants the key's UNSHIFTED character and the modifiers
//! the layout CONSUMED producing the text; xkb knows both, and the door keeps
//! no xkb state a window can ask. Until it does, both are read off what the
//! door does give — exactly for every letter of every Latin layout (a letter's
//! unshifted form is its lower case), and left unsaid for a shifted symbol,
//! where the holder's encoder falls back on the text.

use ronoctua_terminals::wire;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Keysym, Modifiers};

/// THE PHYSICAL KEYS, by the code the compositor sends (Linux's evdev
/// codes, `input-event-codes.h`) and the W3C UI Events `code` the holder
/// encodes by — the key in that POSITION, whatever the layout prints on it.
///
/// Sorted by code, each code and each name once (the table's own test), and
/// every name one the engine knows (`ronoctua_term::ghostty::Key::from_w3c`).
pub const PHYSICAL: &[(u32, &str)] = &[
    (1, "Escape"),
    (2, "Digit1"),
    (3, "Digit2"),
    (4, "Digit3"),
    (5, "Digit4"),
    (6, "Digit5"),
    (7, "Digit6"),
    (8, "Digit7"),
    (9, "Digit8"),
    (10, "Digit9"),
    (11, "Digit0"),
    (12, "Minus"),
    (13, "Equal"),
    (14, "Backspace"),
    (15, "Tab"),
    (16, "KeyQ"),
    (17, "KeyW"),
    (18, "KeyE"),
    (19, "KeyR"),
    (20, "KeyT"),
    (21, "KeyY"),
    (22, "KeyU"),
    (23, "KeyI"),
    (24, "KeyO"),
    (25, "KeyP"),
    (26, "BracketLeft"),
    (27, "BracketRight"),
    (28, "Enter"),
    (29, "ControlLeft"),
    (30, "KeyA"),
    (31, "KeyS"),
    (32, "KeyD"),
    (33, "KeyF"),
    (34, "KeyG"),
    (35, "KeyH"),
    (36, "KeyJ"),
    (37, "KeyK"),
    (38, "KeyL"),
    (39, "Semicolon"),
    (40, "Quote"),
    (41, "Backquote"),
    (42, "ShiftLeft"),
    (43, "Backslash"),
    (44, "KeyZ"),
    (45, "KeyX"),
    (46, "KeyC"),
    (47, "KeyV"),
    (48, "KeyB"),
    (49, "KeyN"),
    (50, "KeyM"),
    (51, "Comma"),
    (52, "Period"),
    (53, "Slash"),
    (54, "ShiftRight"),
    (55, "NumpadMultiply"),
    (56, "AltLeft"),
    (57, "Space"),
    (58, "CapsLock"),
    (59, "F1"),
    (60, "F2"),
    (61, "F3"),
    (62, "F4"),
    (63, "F5"),
    (64, "F6"),
    (65, "F7"),
    (66, "F8"),
    (67, "F9"),
    (68, "F10"),
    (69, "NumLock"),
    (70, "ScrollLock"),
    (71, "Numpad7"),
    (72, "Numpad8"),
    (73, "Numpad9"),
    (74, "NumpadSubtract"),
    (75, "Numpad4"),
    (76, "Numpad5"),
    (77, "Numpad6"),
    (78, "NumpadAdd"),
    (79, "Numpad1"),
    (80, "Numpad2"),
    (81, "Numpad3"),
    (82, "Numpad0"),
    (83, "NumpadDecimal"),
    (86, "IntlBackslash"),
    (87, "F11"),
    (88, "F12"),
    (89, "IntlRo"),
    (92, "Convert"),
    (93, "KanaMode"),
    (94, "NonConvert"),
    (96, "NumpadEnter"),
    (97, "ControlRight"),
    (98, "NumpadDivide"),
    (99, "PrintScreen"),
    (100, "AltRight"),
    (102, "Home"),
    (103, "ArrowUp"),
    (104, "PageUp"),
    (105, "ArrowLeft"),
    (106, "ArrowRight"),
    (107, "End"),
    (108, "ArrowDown"),
    (109, "PageDown"),
    (110, "Insert"),
    (111, "Delete"),
    (113, "AudioVolumeMute"),
    (114, "AudioVolumeDown"),
    (115, "AudioVolumeUp"),
    (116, "Power"),
    (117, "NumpadEqual"),
    (119, "Pause"),
    (121, "NumpadComma"),
    (124, "IntlYen"),
    (125, "MetaLeft"),
    (126, "MetaRight"),
    (127, "ContextMenu"),
    (133, "Copy"),
    (135, "Paste"),
    (137, "Cut"),
    (138, "Help"),
    (163, "MediaTrackNext"),
    (164, "MediaPlayPause"),
    (165, "MediaTrackPrevious"),
    (166, "MediaStop"),
    (183, "F13"),
    (184, "F14"),
    (185, "F15"),
    (186, "F16"),
    (187, "F17"),
    (188, "F18"),
    (189, "F19"),
    (190, "F20"),
    (191, "F21"),
    (192, "F22"),
    (193, "F23"),
    (194, "F24"),
];

/// The W3C name of the physical key `code` — `Unidentified` for one the
/// table does not hold, which the engine encodes from its text alone.
#[must_use]
pub fn physical(code: u32) -> &'static str {
    PHYSICAL
        .binary_search_by_key(&code, |&(c, _)| c)
        .map_or("Unidentified", |i| PHYSICAL[i].1)
}

/// Which way a kept key moves the view of the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaf {
    /// A page up.
    PageUp,
    /// A page down.
    PageDown,
    /// To the oldest line kept.
    Top,
    /// Back to the live screen.
    Bottom,
}

/// What a key of the GROUP does — the terminals one window holds
/// (`crate::terminal_group`). Ghostty's own keys for its tabs, so a hand that
/// knows one terminal knows this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grouped {
    /// `C-T` (Ctrl+Shift+t): a new terminal, right after the one in front,
    /// in its folder.
    New,
    /// `C-TAB`: the next terminal, in front.
    Next,
    /// `C-S-TAB`: the previous one.
    Previous,
    /// `C-S-RET`: the one in front, in a window of its own.
    Detach,
}

/// THE GROUP'S KEYS AND COMMANDS, ONE TABLE: the key as the keymap writes it
/// (`door::keys::key_of` reads a press back into the same notation), the
/// registry's command (`crate::KEYS`, `cmd-<id>`), and what it does
/// — what `Content::shortcuts` lists, what `Content::run_shortcut` runs and
/// what [`kept`] takes, so the three never teach different keys.
///
/// `C-T` and not `C-S-t`: xkb has already shifted the letter, and a chord
/// is written as its letter reads (`C-S` is Ctrl and a capital s). The Tab
/// is no letter, so Shift stays in the notation (`ISO_Left_Tab` is read back
/// as `S-TAB`).
pub const GROUP: &[(&str, &str, Grouped)] = &[
    ("C-T", NEW, Grouped::New),
    ("C-TAB", NEXT, Grouped::Next),
    ("C-S-TAB", PREVIOUS, Grouped::Previous),
    ("C-S-RET", DETACH, Grouped::Detach),
];

/// THE GROUP'S KEYS AFTER `LEADER Alt` — one key each
/// (`chrome::leader_alt_key`), where the window's own are chords because every
/// bare key belongs to the program: `t` a new terminal, `n` and `p` the next
/// and the previous, `w` the one in front in a window of its own. The chords
/// of [`GROUP`] stay what the window answers to.
pub const LEADER_ALT_KEYS: &[(&str, &str)] = &[
    ("C-T", "t"),
    ("C-TAB", "n"),
    ("C-S-TAB", "p"),
    ("C-S-RET", "w"),
];

/// `terminal-new`: a new terminal in this window's group.
pub const NEW: &str = "terminal-new";
/// `terminal-next`: the next terminal of the group, in front.
pub const NEXT: &str = "terminal-next";
/// `terminal-previous`: the previous one.
pub const PREVIOUS: &str = "terminal-previous";
/// `terminal-detach`: the one in front, in a window of its own.
pub const DETACH: &str = "terminal-detach";

/// What the command `id` does in a terminal's window, when it is one of the
/// group's.
#[must_use]
pub fn grouped(id: &str) -> Option<Grouped> {
    GROUP.iter().find(|(_, c, _)| *c == id).map(|&(_, _, g)| g)
}

/// Which way a zoom goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zoom {
    /// Larger.
    In,
    /// Smaller.
    Out,
    /// Back to the person's own size.
    Reset,
}

/// A key the TERMINAL keeps, and never hands the program — Ghostty's own
/// family (`ctrl+shift+…`, `shift+…` for the history), so a hand that knows
/// one terminal knows this one, and `C-c`, `C-v`, `Esc` and every other key
/// still reach whatever runs in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    /// `C-S-c`: the selection, onto the clipboard.
    Copy,
    /// `C-S-v`: the clipboard, pasted.
    Paste,
    /// `S-Insert`: the primary selection, pasted.
    PastePrimary,
    /// `S-PgUp`, `S-PgDn`, `S-Home`, `S-End`: the history.
    Leaf(Leaf),
    /// `C-S-Up` / `C-S-Down`: to the previous or the next prompt (OSC 133);
    /// `true` is back.
    Prompt(bool),
    /// `C-S-a`: everything, selected.
    SelectAll,
    /// `C-=` / `C-+`, `C--`, `C-0`: this window's size of text.
    Zoom(Zoom),
    /// `C-S-f`: the search line (`crate::terminal_search`).
    Search,
    /// One of the group's keys ([`GROUP`]). Taken ALWAYS, before the
    /// encoder, as Ghostty takes its tabs' — whether the group has two
    /// terminals to move between or not: a key that reached the program
    /// only while the window held one terminal would mean two things.
    Group(Grouped),
}

/// The key `sym` held with `mods`, when the terminal keeps it.
///
/// THE MODIFIERS ARE EXACT: `C-S-c` is Ctrl and Shift and nothing else, so
/// `C-M-S-c` — a chord a program may bind — still reaches the program.
/// Shift is in the keysym already for a letter (xkb gives `C` for Shift+c),
/// so both cases of a letter are matched.
#[must_use]
pub fn kept(sym: Keysym, mods: Modifiers) -> Option<Kept> {
    let (ctrl, shift) = (mods.ctrl, mods.shift);
    if mods.alt || mods.logo {
        return None;
    }
    match (ctrl, shift) {
        (true, true) => match sym {
            Keysym::C | Keysym::c => Some(Kept::Copy),
            Keysym::V | Keysym::v => Some(Kept::Paste),
            Keysym::A | Keysym::a => Some(Kept::SelectAll),
            Keysym::F | Keysym::f => Some(Kept::Search),
            Keysym::T | Keysym::t => Some(Kept::Group(Grouped::New)),
            // SHIFT+TAB ARRIVES AS `ISO_Left_Tab` from xkb (the layout's
            // shifted Tab), and as `Tab` with Shift held from a layout that
            // has none: both are the previous terminal.
            Keysym::ISO_Left_Tab | Keysym::Tab => Some(Kept::Group(Grouped::Previous)),
            Keysym::Return | Keysym::KP_Enter => Some(Kept::Group(Grouped::Detach)),
            Keysym::Up | Keysym::KP_Up => Some(Kept::Prompt(true)),
            Keysym::Down | Keysym::KP_Down => Some(Kept::Prompt(false)),
            // `C-+` on a layout whose plus is Shift and equals.
            Keysym::plus => Some(Kept::Zoom(Zoom::In)),
            _ => None,
        },
        (false, true) => match sym {
            Keysym::Insert | Keysym::KP_Insert => Some(Kept::PastePrimary),
            Keysym::Page_Up | Keysym::KP_Page_Up => Some(Kept::Leaf(Leaf::PageUp)),
            Keysym::Page_Down | Keysym::KP_Page_Down => Some(Kept::Leaf(Leaf::PageDown)),
            Keysym::Home | Keysym::KP_Home => Some(Kept::Leaf(Leaf::Top)),
            Keysym::End | Keysym::KP_End => Some(Kept::Leaf(Leaf::Bottom)),
            _ => None,
        },
        (true, false) => match sym {
            Keysym::Tab => Some(Kept::Group(Grouped::Next)),
            // THE CAPITAL WITH NO SHIFT HELD is the shortcut's own press
            // (`door::keys::event_of` of `C-T`, a click on it): the keyboard
            // never sends it — Caps Lock is undone on a chord.
            Keysym::T => Some(Kept::Group(Grouped::New)),
            Keysym::F => Some(Kept::Search),
            // A layout whose Shift+Tab is `ISO_Left_Tab` with the Shift
            // already spent on it.
            Keysym::ISO_Left_Tab => Some(Kept::Group(Grouped::Previous)),
            Keysym::equal | Keysym::plus | Keysym::KP_Add => Some(Kept::Zoom(Zoom::In)),
            Keysym::minus | Keysym::KP_Subtract => Some(Kept::Zoom(Zoom::Out)),
            Keysym::_0 | Keysym::KP_0 => Some(Kept::Zoom(Zoom::Reset)),
            _ => None,
        },
        (false, false) => None,
    }
}

/// The wire's modifiers for the door's.
#[must_use]
pub const fn mods_of(m: Modifiers) -> wire::Mods {
    wire::Mods {
        shift: m.shift,
        ctrl: m.ctrl,
        alt: m.alt,
        super_key: m.logo,
        caps_lock: m.caps_lock,
        num_lock: m.num_lock,
    }
}

/// The text a key TYPES, before Ctrl or Alt changed it — what the encoder
/// wants (`KeyEvent::utf8` on the wire): xkb's own text unless it is a
/// control character (`C-c` comes as U+0003), and then the keysym's, which
/// Ctrl does not touch. A key that types nothing (an arrow, Enter) has none.
fn typed_text(event: &KeyEvent) -> String {
    match event.utf8.as_deref() {
        Some(t) if !t.is_empty() && !t.chars().any(char::is_control) => t.to_owned(),
        _ => event
            .keysym
            .key_char()
            .filter(|c| !c.is_control())
            .map(String::from)
            .unwrap_or_default(),
    }
}

/// A key as the frame the holder encodes: the physical key, the modifiers,
/// the text, and the unshifted character and the consumed modifiers as far
/// as they can be read (see the head of the module).
#[must_use]
pub fn frame_of(event: &KeyEvent, mods: Modifiers, action: wire::KeyAction) -> wire::KeyEvent {
    let utf8 = typed_text(event);
    let mut chars = utf8.chars();
    let sole = match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    };
    let unshifted = sole.and_then(|c| {
        if c.is_alphabetic() {
            c.to_lowercase().next()
        } else if mods.shift {
            None
        } else {
            Some(c)
        }
    });
    let consumed = wire::Mods {
        // Shift made the character, whenever it is held over a key that types
        // one: a letter's capital, a digit's symbol.
        shift: mods.shift && sole.is_some(),
        // And the Lock made a letter's capital without it.
        caps_lock: mods.caps_lock && !mods.shift && sole.is_some_and(char::is_uppercase),
        ..wire::Mods::default()
    };
    wire::KeyEvent {
        code: event.raw_code,
        key: physical(event.raw_code).to_owned(),
        action,
        mods: mods_of(mods),
        consumed,
        unshifted,
        utf8,
        composing: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(ctrl: bool, shift: bool) -> Modifiers {
        Modifiers {
            ctrl,
            shift,
            ..Modifiers::default()
        }
    }

    fn pressed(raw_code: u32, keysym: Keysym, utf8: Option<&str>) -> KeyEvent {
        KeyEvent {
            time: 0,
            raw_code,
            keysym,
            utf8: utf8.map(str::to_owned),
        }
    }

    #[test]
    fn the_physical_table_is_sorted_unique_and_the_engines() {
        // FAILS IF a code or a name is in the table twice (the lookup is a
        // binary search, and a second entry would never be found), or a name
        // is one the holder's engine cannot map — the key would reach the
        // program as `Unidentified`, encoded from its text alone.
        for pair in PHYSICAL.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{pair:?} out of order");
        }
        let mut names: Vec<&str> = PHYSICAL.iter().map(|&(_, n)| n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), PHYSICAL.len(), "a name is in the table twice");
        for &(code, name) in PHYSICAL {
            assert!(
                ronoctua_term::ghostty::Key::from_w3c(name).is_some(),
                "{code} {name} is no key of the engine"
            );
        }
    }

    #[test]
    fn a_physical_key_is_found_by_its_code_and_an_unknown_one_is_unidentified() {
        // FAILS IF the lookup answers another key's name (Enter for A), or
        // something other than the engine's own word for a key it lacks.
        assert_eq!(physical(28), "Enter");
        assert_eq!(physical(30), "KeyA");
        assert_eq!(physical(103), "ArrowUp");
        assert_eq!(physical(1), "Escape");
        assert_eq!(physical(194), "F24");
        assert_eq!(physical(0), "Unidentified");
        assert_eq!(physical(9999), "Unidentified");
    }

    #[test]
    fn the_terminal_keeps_its_own_keys_and_nothing_else() {
        // FAILS IF one of Ghostty's own keys reaches the program instead of
        // the terminal (a copy that sends `^C`), or a key the program is owed
        // is kept: `C-c`, `C-v`, `Esc`, `C-y`, a plain `Insert`.
        let cs = held(true, true);
        // BOTH CASES of the letter: xkb shifts it (`C`), a click on a hint
        // and Caps Lock undone on a chord give it lower (`c`).
        for (upper, lower, wanted) in [
            (Keysym::C, Keysym::c, Kept::Copy),
            (Keysym::V, Keysym::v, Kept::Paste),
            (Keysym::A, Keysym::a, Kept::SelectAll),
        ] {
            assert_eq!(kept(upper, cs), Some(wanted));
            assert_eq!(kept(lower, cs), Some(wanted));
        }
        assert_eq!(kept(Keysym::Up, cs), Some(Kept::Prompt(true)));
        assert_eq!(kept(Keysym::Down, cs), Some(Kept::Prompt(false)));
        let s = held(false, true);
        assert_eq!(kept(Keysym::Insert, s), Some(Kept::PastePrimary));
        assert_eq!(kept(Keysym::Page_Up, s), Some(Kept::Leaf(Leaf::PageUp)));
        assert_eq!(kept(Keysym::Page_Down, s), Some(Kept::Leaf(Leaf::PageDown)));
        assert_eq!(kept(Keysym::Home, s), Some(Kept::Leaf(Leaf::Top)));
        assert_eq!(kept(Keysym::End, s), Some(Kept::Leaf(Leaf::Bottom)));
        let c = held(true, false);
        assert_eq!(kept(Keysym::equal, c), Some(Kept::Zoom(Zoom::In)));
        assert_eq!(kept(Keysym::minus, c), Some(Kept::Zoom(Zoom::Out)));
        assert_eq!(kept(Keysym::_0, c), Some(Kept::Zoom(Zoom::Reset)));

        // AND WHAT THE PROGRAM IS OWED.
        for (sym, mods) in [
            (Keysym::c, c),
            (Keysym::v, c),
            (Keysym::y, c),
            (Keysym::Escape, Modifiers::default()),
            (Keysym::Insert, Modifiers::default()),
            (Keysym::Page_Up, Modifiers::default()),
            (Keysym::Up, c),
        ] {
            assert_eq!(kept(sym, mods), None, "{sym:?} {mods:?} is the program's");
        }
        // A chord with one modifier more is the program's too.
        let more = Modifiers { alt: true, ..cs };
        assert_eq!(kept(Keysym::C, more), None);
    }

    #[test]
    fn the_groups_keys_are_kept_and_shift_tab_arrives_as_iso_left_tab() {
        // FAILS IF a key of the group reaches the program (a `C-S-t` typed
        // as a capital T), Shift+Tab is only heard as one of its two
        // spellings, or the table the shortcuts list teaches a key that is
        // not the one this window takes — read back by the door's own
        // reader, as a click on the shortcut would press it.
        let cs = held(true, true);
        assert_eq!(kept(Keysym::T, cs), Some(Kept::Group(Grouped::New)));
        assert_eq!(kept(Keysym::t, cs), Some(Kept::Group(Grouped::New)));
        assert_eq!(
            kept(Keysym::ISO_Left_Tab, cs),
            Some(Kept::Group(Grouped::Previous))
        );
        assert_eq!(kept(Keysym::Tab, cs), Some(Kept::Group(Grouped::Previous)));
        assert_eq!(
            kept(Keysym::ISO_Left_Tab, held(true, false)),
            Some(Kept::Group(Grouped::Previous))
        );
        assert_eq!(
            kept(Keysym::Tab, held(true, false)),
            Some(Kept::Group(Grouped::Next))
        );
        assert_eq!(kept(Keysym::Return, cs), Some(Kept::Group(Grouped::Detach)));
        assert_eq!(kept(Keysym::F, cs), Some(Kept::Search));
        // A PLAIN TAB AND A PLAIN RETURN ARE THE PROGRAM'S.
        assert_eq!(kept(Keysym::Tab, Modifiers::default()), None);
        assert_eq!(kept(Keysym::Return, held(true, false)), None);
        for &(notation, id, wanted) in GROUP {
            let key = ronoctua_keymap::Key::parse(notation).expect(notation);
            let (event, mods) = ronoctua_shell_door::door::keys::event_of(key);
            assert_eq!(
                kept(event.keysym, mods),
                Some(Kept::Group(wanted)),
                "{notation}"
            );
            assert_eq!(
                ronoctua_shell_door::door::keys::key_of(&event, mods).map(|k| k.notation()),
                Some(notation.to_owned()),
                "{notation} is not how the door reads its own key"
            );
            assert_eq!(grouped(id), Some(wanted));
            assert!(id.starts_with("terminal-"), "{id}");
        }
        assert_eq!(grouped("terminal-elsewhere"), None);
    }

    #[test]
    fn ctrl_c_reaches_the_program_as_its_key_and_letter() {
        // FAILS IF the frame carries xkb's control character as the text (the
        // encoder wants the letter and the key, and makes `^C` itself), or
        // the physical key is lost.
        let k = frame_of(
            &pressed(46, Keysym::c, Some("\u{3}")),
            held(true, false),
            wire::KeyAction::Press,
        );
        assert_eq!(k.key, "KeyC");
        assert_eq!(k.code, 46);
        assert_eq!(k.utf8, "c");
        assert!(k.mods.ctrl && !k.mods.shift);
        assert_eq!(k.unshifted, Some('c'));
    }

    #[test]
    fn a_shifted_letter_says_its_unshifted_form_and_the_shift_it_spent() {
        // FAILS IF kitty's alternate key is lost for a capital, or Shift is
        // not said to have made it (the encoder would add a second shift).
        let k = frame_of(
            &pressed(30, Keysym::A, Some("A")),
            held(false, true),
            wire::KeyAction::Press,
        );
        assert_eq!(k.utf8, "A");
        assert_eq!(k.unshifted, Some('a'));
        assert!(k.consumed.shift);
        // A symbol Shift made: the character it would be without is not
        // known here, and not invented.
        let k = frame_of(
            &pressed(2, Keysym::exclam, Some("!")),
            held(false, true),
            wire::KeyAction::Press,
        );
        assert_eq!(k.unshifted, None);
        assert!(k.consumed.shift);
    }

    #[test]
    fn a_key_that_types_nothing_carries_no_text() {
        // FAILS IF Enter's `\r` or an arrow's nothing is handed on as text:
        // the encoder would type it instead of encoding the key.
        let k = frame_of(
            &pressed(28, Keysym::Return, Some("\r")),
            Modifiers::default(),
            wire::KeyAction::Press,
        );
        assert_eq!(k.key, "Enter");
        assert!(k.utf8.is_empty());
        assert_eq!(k.unshifted, None);
        let k = frame_of(
            &pressed(103, Keysym::Up, None),
            Modifiers::default(),
            wire::KeyAction::Repeat,
        );
        assert_eq!(k.key, "ArrowUp");
        assert_eq!(k.action, wire::KeyAction::Repeat);
        assert!(k.utf8.is_empty() && !k.consumed.shift);
    }
}
