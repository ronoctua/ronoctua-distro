//! The network view's key table, read as presses — one place, so the two
//! doors cannot answer differently to the same letter.
//!
//! The shape `bluetooth_words` already has, and for its reason: the table in
//! `network::OF_THE_VIEW` is what the hint line is generated from and what
//! the manual names, and a letter taught there that no press answers to is a
//! key that does nothing.

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
        // TOR'S SHEET. `Esc` has no press of its own — the doors map it
        // before they reach this table, the way they already do for the list —
        // so what is here is the two that change a value.
        // `Esc` IS ALREADY HANDLED BY BOTH DOORS before they reach this
        // table — it is not a letter — but it is in `OF_THE_VIEW` because the
        // sheet has to TEACH it, and every id that table teaches owes a press.
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
}
