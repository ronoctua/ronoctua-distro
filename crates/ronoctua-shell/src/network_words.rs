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
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_in_the_table_does_something() {
        for (notation, id) in crate::network::OF_THE_VIEW {
            assert!(
                press_of(id).is_some(),
                "`{notation}` teaches `{id}`, and no press answers to it"
            );
        }
    }
}
