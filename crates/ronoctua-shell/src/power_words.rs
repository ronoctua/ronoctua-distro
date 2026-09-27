//! The sentences the two power doors share — a refusal, and which command a
//! lock is: one place, so the footer and the window cannot say it two ways.

use ronoctua_power::{Keep, Order, Refusal};

/// The command that takes or drops one of our locks — bare, what the leader
/// runs, and it flips the lock; with [`keep_word`], what the window sends,
/// and it says which way.
#[must_use]
pub const fn keep_command(keep: Keep) -> &'static str {
    match keep {
        Keep::Sleep => "power-keep-awake",
        Keep::Lid => "power-keep-lid",
    }
}

/// The lock WANTED, as the argument of [`keep_command`].
///
/// THE STATE AND NOT A FLIP. The window decides from its own reading, and a
/// flip decided again from the footer's could come out the other way when the
/// two readings are a moment apart — the key would do the opposite of what
/// the row under it said.
#[must_use]
pub const fn keep_word(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// [`keep_word`], read back. `None` for any other word, which is refused,
/// never taken for a flip.
#[must_use]
pub fn keep_wanted(word: &str) -> Option<bool> {
    match word.trim() {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

/// What the message line says when a daemon refused an order.
#[must_use]
pub fn refused(order: &Order, why: Refusal, words: &str, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    let what = texts.t(match order {
        Order::SetProfile(_) | Order::BatteryAware(_) => key::POWER_WHAT_PROFILE,
        Order::KeepAwake(..) => key::POWER_WHAT_LOCK,
        Order::Thresholds { .. } => key::POWER_WHAT_THRESHOLDS,
        Order::Do(_) => key::POWER_WHAT_ACTION,
    });
    let reason = texts.t(match why {
        Refusal::NotAuthorized => key::POWER_REFUSED_AUTHORITY,
        Refusal::NotAvailable => key::POWER_REFUSED_ABSENT,
        Refusal::Invalid => key::POWER_REFUSED_INVALID,
        Refusal::Other => key::POWER_REFUSED_OTHER,
    });
    texts.t_with(
        key::ECHO_POWER_REFUSED,
        &[("what", &what), ("why", &reason), ("words", words)],
    )
}

/// A daemon's word for an action or a thing it guards — `suspend`,
/// `hybrid-sleep`, `handle-lid-switch` — in the person's language.
///
/// logind speaks kebab-case (`hybrid-sleep`, `poweroff`) and UPower CamelCase
/// (`HybridSleep`, `PowerOff`); both land on the same key, because the match
/// is on the letters alone. The vocabulary is CLOSED and each key is a
/// constant, so a missing text is a compile error; a word outside it is shown
/// as it came — still the truth, raw until it is added here.
#[must_use]
pub fn daemon_word(raw: &str, texts: &ronoctua_i18n::Texts) -> String {
    spoken(WORDS, raw, texts)
}

/// What the kernel's `charge_behaviour` is doing to the battery right now.
#[must_use]
pub fn charge_behaviour(raw: &str, texts: &ronoctua_i18n::Texts) -> String {
    spoken(BEHAVIOURS, raw, texts)
}

/// The kernel's `charge_type` — how it is putting the charge in.
#[must_use]
pub fn charge_type(raw: &str, texts: &ronoctua_i18n::Texts) -> String {
    spoken(CHARGE_TYPES, raw, texts)
}

/// One closed vocabulary, looked up by its letters; the raw word is the
/// fallback, and it is the truth until somebody adds the key.
fn spoken(table: &[(&str, &str)], raw: &str, texts: &ronoctua_i18n::Texts) -> String {
    let letters = letters_of(raw);
    match table.iter().find(|(w, _)| *w == letters) {
        Some((_, key)) => texts.t(key),
        None => raw.to_owned(),
    }
}

/// logind's colon-separated list — `sleep:idle`, the `What` of an inhibitor
/// and the `BlockInhibited` property — as NOUNS, comma-separated: what is
/// held is a thing ("the sleep"), not what a key does ("suspend"), and in
/// Portuguese the two are different words.
#[must_use]
pub fn daemon_words(list: &str, texts: &ronoctua_i18n::Texts) -> String {
    list.split(':')
        .filter(|w| !w.is_empty())
        .map(|w| spoken(WHATS, w, texts))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The letters alone, lower-case: `hybrid-sleep` and `HybridSleep` agree.
fn letters_of(raw: &str) -> String {
    raw.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// What an inhibitor holds — logind's `What` values — as nouns.
const WHATS: &[(&str, &str)] = {
    use ronoctua_i18n::key;
    &[
        ("sleep", key::POWER_WHAT_SLEEP),
        ("idle", key::POWER_WHAT_IDLE),
        ("shutdown", key::POWER_WHAT_SHUTDOWN),
        ("handlepowerkey", key::POWER_WHAT_HANDLE_POWER_KEY),
        ("handlesuspendkey", key::POWER_WHAT_HANDLE_SUSPEND_KEY),
        ("handlehibernatekey", key::POWER_WHAT_HANDLE_HIBERNATE_KEY),
        ("handlelidswitch", key::POWER_WHAT_HANDLE_LID_SWITCH),
        ("handlerebootkey", key::POWER_WHAT_HANDLE_REBOOT_KEY),
    ]
};

/// The letters of a daemon's word, and the key that speaks it.
const WORDS: &[(&str, &str)] = {
    use ronoctua_i18n::key;
    &[
        // What closing the lid, the power key, idling and the critical level do.
        ("ignore", key::POWER_WORD_IGNORE),
        ("poweroff", key::POWER_WORD_POWEROFF),
        ("reboot", key::POWER_WORD_REBOOT),
        ("halt", key::POWER_WORD_HALT),
        ("kexec", key::POWER_WORD_KEXEC),
        ("suspend", key::POWER_WORD_SUSPEND),
        ("hibernate", key::POWER_WORD_HIBERNATE),
        ("hybridsleep", key::POWER_WORD_HYBRID_SLEEP),
        (
            "suspendthenhibernate",
            key::POWER_WORD_SUSPEND_THEN_HIBERNATE,
        ),
        ("lock", key::POWER_WORD_LOCK),
        ("factoryreset", key::POWER_WORD_FACTORY_RESET),
        ("softreboot", key::POWER_WORD_SOFT_REBOOT),
        // What UPower answers for `GetCriticalAction` on the owner's laptop,
        // and one of logind's words for the lid.
        ("sleep", key::POWER_WORD_SLEEP),
    ]
};

/// `charge_behaviour`, from the kernel's `power_supply` class.
const BEHAVIOURS: &[(&str, &str)] = {
    use ronoctua_i18n::key;
    &[
        ("auto", key::POWER_BEHAVIOUR_AUTO),
        ("inhibitcharge", key::POWER_BEHAVIOUR_INHIBIT_CHARGE),
        (
            "inhibitchargeawake",
            key::POWER_BEHAVIOUR_INHIBIT_CHARGE_AWAKE,
        ),
        ("forcedischarge", key::POWER_BEHAVIOUR_FORCE_DISCHARGE),
    ]
};

/// `charge_type`, from the same class. `N/A` and `Long_Life` lose their
/// punctuation to [`letters_of`], like every other word here.
const CHARGE_TYPES: &[(&str, &str)] = {
    use ronoctua_i18n::key;
    &[
        ("unknown", key::POWER_CHARGE_TYPE_UNKNOWN),
        ("na", key::POWER_CHARGE_TYPE_NA),
        ("trickle", key::POWER_CHARGE_TYPE_TRICKLE),
        ("fast", key::POWER_CHARGE_TYPE_FAST),
        ("standard", key::POWER_CHARGE_TYPE_STANDARD),
        ("adaptive", key::POWER_CHARGE_TYPE_ADAPTIVE),
        ("custom", key::POWER_CHARGE_TYPE_CUSTOM),
        ("longlife", key::POWER_CHARGE_TYPE_LONG_LIFE),
        ("bypass", key::POWER_CHARGE_TYPE_BYPASS),
    ]
};

#[cfg(test)]
mod tests {
    use super::*;

    /// THE WINDOW SAYS WHICH WAY, and the footer reads it back; a word it
    /// does not know is no flip.
    #[test]
    fn a_lock_wanted_is_said_and_read_back() {
        for on in [true, false] {
            assert_eq!(keep_wanted(keep_word(on)), Some(on));
        }
        for wrong in ["", "yes", "toggle", "On"] {
            assert_eq!(keep_wanted(wrong), None, "{wrong}");
        }
    }

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    #[test]
    fn both_daemons_land_on_the_same_word() {
        let t = texts();
        assert_eq!(daemon_word("Hibernate", &t), daemon_word("hibernate", &t));
        assert_eq!(
            daemon_word("HybridSleep", &t),
            daemon_word("hybrid-sleep", &t)
        );
        assert_eq!(daemon_word("PowerOff", &t), daemon_word("poweroff", &t));
        assert!(
            !daemon_word("Hibernate", &t).starts_with('['),
            "a key, not a raw word"
        );
    }

    #[test]
    fn a_word_outside_the_vocabulary_is_shown_as_it_came() {
        let t = texts();
        assert_eq!(daemon_word("frobnicate", &t), "frobnicate");
        assert_eq!(charge_behaviour("frobnicate", &t), "frobnicate");
        assert_eq!(charge_type("frobnicate", &t), "frobnicate");
    }

    #[test]
    fn the_kernels_two_charge_vocabularies_are_spoken() {
        let t = texts();
        // Punctuation and case are the kernel's, not the key's.
        for raw in [
            "auto",
            "inhibit-charge",
            "inhibit-charge-awake",
            "force-discharge",
        ] {
            assert_ne!(charge_behaviour(raw, &t), raw, "{raw} has no text");
        }
        for raw in [
            "Unknown",
            "N/A",
            "Trickle",
            "Fast",
            "Standard",
            "Adaptive",
            "Custom",
            "Long_Life",
            "Bypass",
        ] {
            assert_ne!(charge_type(raw, &t), raw, "{raw} has no text");
        }
        assert!(!charge_type("Long_Life", &t).contains('_'));
    }

    #[test]
    fn a_colon_list_becomes_nouns() {
        let t = texts();
        let words = daemon_words("sleep:idle", &t);
        assert_eq!(words.matches(", ").count(), 1);
        assert!(!words.contains(':'));
        assert_eq!(daemon_words("", &t), "");
        // A noun, not the verb the lid's action uses.
        let pt = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
        assert_ne!(daemon_words("sleep", &pt), daemon_word("sleep", &pt));
        assert_eq!(daemon_words("frobnicate", &pt), "frobnicate");
    }
}
