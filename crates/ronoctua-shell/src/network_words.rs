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

/// What the message line says when NetworkManager — or this machine — refused.
///
/// BY NAME (`ronoctua_network::refusal`): the error's words are English, and
/// they go to the log; the line says the refusal in the person's language.
#[must_use]
pub fn refused(e: &anyhow::Error, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    texts.t_with(key::ECHO_NETWORK_REFUSED, &[("why", &reason(e, texts))])
}

/// The REASON alone, by name — what an errand's own sentence ("could not
/// connect to X: …", "could not share: …") puts after its colon.
///
/// The errand's sentence says WHAT was not done; this says WHY, in the
/// person's language. The error's words go to the log, never to the line.
///
/// A refusal the network crate names itself ([`ronoctua_network::Refusal`]:
/// no phone, no VPN, not a VPN) is said by its own sentence
/// (`network::network_why_key`); every other is classified by
/// `ronoctua_network::refusal`.
#[must_use]
pub fn reason(e: &anyhow::Error, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    use ronoctua_network::refusal::Refusal;
    if let Some(why) = e
        .chain()
        .find_map(|c| c.downcast_ref::<ronoctua_network::Refusal>())
    {
        tracing::info!(?why, words = %format!("{e:#}"), "the network refused");
        return texts.t(crate::network::network_why_key(*why));
    }
    let why = ronoctua_network::refusal::refusal(e);
    tracing::info!(?why, words = %format!("{e:#}"), "the network refused");
    texts.t(match why {
        Refusal::NotAuthorized => key::NETWORK_REFUSED_AUTHORITY,
        Refusal::NotAvailable => key::NETWORK_REFUSED_ABSENT,
        Refusal::Invalid => key::NETWORK_REFUSED_INVALID,
        Refusal::NoSecrets => key::NETWORK_REFUSED_SECRETS,
        Refusal::Failed => key::NETWORK_REFUSED_FAILED,
        Refusal::TimedOut => key::NETWORK_REFUSED_TIMED_OUT,
        Refusal::NoBus => key::NETWORK_REFUSED_NO_BUS,
        Refusal::NoRadio => key::NETWORK_REFUSED_NO_RADIO,
        Refusal::OutOfRange => key::NETWORK_REFUSED_OUT_OF_RANGE,
        Refusal::NoDevice => key::NETWORK_REFUSED_NO_DEVICE,
        Refusal::NoProfile => key::NETWORK_REFUSED_NO_PROFILE,
        Refusal::ImportRefused => key::NETWORK_REFUSED_IMPORT,
        Refusal::Vault => key::NETWORK_REFUSED_VAULT,
        Refusal::Other => key::NETWORK_REFUSED_OTHER,
    })
}

/// What the message line says when a tunnel's daemon refused an order —
/// by name, and with the line Tor quoted when it quoted one: that is the
/// person's own text given back, and the one thing that says which bridge was
/// pasted wrong.
#[must_use]
pub fn tunnel_refused(r: &ronoctua_tunnel::Refused, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    use ronoctua_tunnel::Refusal;
    tracing::info!(why = ?r.why, words = %r.words, "a tunnel refused");
    if let (Refusal::ConfigRefused, Some(line)) = (r.why, r.line.as_deref()) {
        return texts.t_with(key::ECHO_TUNNEL_REFUSED_LINE, &[("line", line)]);
    }
    let reason = texts.t(match r.why {
        Refusal::NoDaemon => key::TUNNEL_REFUSED_NO_DAEMON,
        Refusal::NotAuthorized => key::TUNNEL_REFUSED_AUTHORITY,
        Refusal::UnitFailed => key::TUNNEL_REFUSED_UNIT,
        Refusal::NotWritten => key::TUNNEL_REFUSED_NOT_WRITTEN,
        Refusal::ConfigRefused => key::TUNNEL_REFUSED_CONFIG,
        Refusal::DaemonRefused => key::TUNNEL_REFUSED_DAEMON,
        Refusal::Other => key::TUNNEL_REFUSED_OTHER,
    });
    texts.t_with(key::ECHO_NETWORK_REFUSED, &[("why", &reason)])
}

#[cfg(test)]
mod refusal_tests {
    use super::*;

    /// FAILS IF: an English word of a daemon reaches the line, or two
    /// refusals say the same thing.
    #[test]
    fn every_network_refusal_is_said_by_its_name() {
        use ronoctua_network::refusal::{Named, Refusal};
        let t = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        let all = [
            Refusal::NoRadio,
            Refusal::OutOfRange,
            Refusal::NoDevice,
            Refusal::NoProfile,
            Refusal::ImportRefused,
            Refusal::Vault,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for r in all {
            let said = refused(
                &anyhow::Error::from(Named(r)).context("the English context"),
                &t,
            );
            assert!(!said.contains("English"), "{said}");
            assert!(seen.insert(said), "{r:?} says what another says");
        }
        let other = refused(&anyhow::anyhow!("the socket went away"), &t);
        assert!(!other.contains("socket"), "{other}");
        // AND A TUNNEL: the line Tor quoted is given back, nothing else.
        let line = tunnel_refused(
            &ronoctua_tunnel::Refused {
                why: ronoctua_tunnel::Refusal::ConfigRefused,
                line: Some("Brige obfs4".to_owned()),
                words: "553 Unrecognized option".to_owned(),
            },
            &t,
        );
        assert!(
            line.contains("Brige obfs4") && !line.contains("Unrecognized"),
            "{line}"
        );
        let unit = tunnel_refused(
            &ronoctua_tunnel::Refused {
                why: ronoctua_tunnel::Refusal::UnitFailed,
                line: None,
                words: "Job failed".to_owned(),
            },
            &t,
        );
        assert!(!unit.contains("Job"), "{unit}");
    }

    /// FAILS IF: an errand's own sentence — connect, share, import, keep
    /// the password — carries the error's English after its colon.
    #[test]
    fn an_errands_sentence_carries_the_reason_by_name() {
        use ronoctua_i18n::key;
        use ronoctua_network::refusal::{Named, Refusal};
        let t = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR");
        let e = anyhow::Error::from(Named(Refusal::NoRadio)).context("asking NetworkManager");
        let why = reason(&e, &t);
        assert_eq!(why, t.t(key::NETWORK_REFUSED_NO_RADIO));
        for said in [
            t.t_with(
                key::ECHO_NETWORK_FAILED,
                &[("network", "casa"), ("reason", &why)],
            ),
            t.t_with(key::ECHO_NETWORK_HOTSPOT_FAILED, &[("reason", &why)]),
            t.t_with(
                key::ECHO_NETWORK_IMPORT_FAILED,
                &[("file", "vpn.ovpn"), ("reason", &why)],
            ),
            t.t_with(key::ECHO_NETWORK_STORE_FAILED, &[("reason", &why)]),
        ] {
            assert!(
                !said.contains("Wi-Fi radio") && !said.contains("asking"),
                "{said}"
            );
            assert!(said.contains(&why), "{said}");
        }
    }
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
