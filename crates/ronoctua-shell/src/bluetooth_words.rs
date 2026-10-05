//! The sentence the two bluetooth doors share — what was refused, and by
//! which door. One place, so the footer and the window cannot say it two ways.
//!
//! The shape `power_words` already has, and for its reason: BlueZ answers
//! with thirty error NAMES and no sentence worth showing a person, so the
//! name chooses the phrase and the daemon's own words travel in brackets.

use ronoctua_bluetooth::{Ask, Refusal};

/// What the message line says when the daemon refused an order.
#[must_use]
pub fn refused(ask: &Ask, why: Refusal, words: &str, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    let what = texts.t(match ask {
        Ask::Connect(_) | Ask::Disconnect(_) | Ask::Profile { .. } => key::BLUETOOTH_WHAT_CONNECT,
        Ask::Pair(_) => key::BLUETOOTH_WHAT_PAIR,
        Ask::Forget(_) => key::BLUETOOTH_WHAT_FORGET,
        Ask::Power(_)
        | Ask::Discoverable(_)
        | Ask::Pairable(_)
        | Ask::Scan(_)
        | Ask::AdapterAlias(_)
        | Ask::DiscoverableTimeout(_)
        | Ask::PairableTimeout(_) => key::BLUETOOTH_WHAT_RADIO,
        Ask::Trust(..) | Ask::Block(..) | Ask::Rename(..) | Ask::Wake(..) => {
            key::BLUETOOTH_WHAT_DEVICE
        }
    });
    let reason = texts.t(match why {
        Refusal::NotAuthorized => key::BLUETOOTH_REFUSED_AUTHORITY,
        Refusal::NotAvailable => key::BLUETOOTH_REFUSED_ABSENT,
        Refusal::Away => key::BLUETOOTH_REFUSED_AWAY,
        Refusal::Busy => key::BLUETOOTH_REFUSED_BUSY,
        Refusal::Rejected => key::BLUETOOTH_REFUSED_REJECTED,
        Refusal::TimedOut => key::BLUETOOTH_REFUSED_TIMED_OUT,
        Refusal::NoBus => key::BLUETOOTH_REFUSED_NO_BUS,
        Refusal::Other => key::BLUETOOTH_REFUSED_OTHER,
    });
    // BLUEZ'S OWN WORDS GO TO THE LOG: they are English, and the line is in
    // the person's language. The name already chose the phrase.
    tracing::info!(?why, words, "the Bluetooth daemon refused");
    texts.t_with(
        key::ECHO_BLUETOOTH_REFUSED,
        &[("what", &what), ("why", &reason)],
    )
}

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

    #[test]
    fn every_refusal_has_a_sentence() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let asks = [
            Ask::Connect("/d/1".to_owned()),
            Ask::Pair("/d/1".to_owned()),
            Ask::Forget("/d/1".to_owned()),
            Ask::Power(true),
            Ask::Trust("/d/1".to_owned(), true),
        ];
        let whys = [
            Refusal::NotAuthorized,
            Refusal::NotAvailable,
            Refusal::Away,
            Refusal::Busy,
            Refusal::Rejected,
            Refusal::TimedOut,
            Refusal::NoBus,
            Refusal::Other,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for ask in &asks {
            for why in whys {
                let said = refused(ask, why, "org.bluez.Error.Failed", &t);
                assert!(!said.contains('['), "a key reached the screen raw: {said}");
                // THE DAEMON'S WORDS NEVER REACH THE LINE: they are English.
                assert!(
                    !said.contains("org.bluez"),
                    "the daemon's words were drawn: {said}"
                );
                seen.insert(said);
            }
        }
        // AND EVERY REFUSAL IS ITS OWN SENTENCE.
        assert_eq!(seen.len(), asks.len() * whys.len());
    }
}
