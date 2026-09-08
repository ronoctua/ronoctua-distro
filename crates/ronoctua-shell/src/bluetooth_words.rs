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
        Refusal::Other => key::BLUETOOTH_REFUSED_OTHER,
    });
    texts.t_with(
        key::ECHO_BLUETOOTH_REFUSED,
        &[("what", &what), ("why", &reason), ("words", words)],
    )
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
        "bluetooth-more" => Press::Right,
        "bluetooth-less" => Press::Left,
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
            Refusal::Other,
        ];
        for ask in &asks {
            for why in whys {
                let said = refused(ask, why, "org.bluez.Error.Failed", &t);
                assert!(!said.contains('['), "a key reached the screen raw: {said}");
                assert!(
                    said.contains("org.bluez"),
                    "the daemon's words were dropped"
                );
            }
        }
    }
}
