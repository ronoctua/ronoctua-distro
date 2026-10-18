//! The power daemon's words and marks, which the footer and the power view
//! both say — a refusal, which command a lock is, how long the battery has,
//! the profile's glyph and the modeline's battery segment: one place, so the
//! footer and the view's two doors cannot say any of it two ways.
//!
//! # Why in a crate of their own
//!
//! The footer says them with no view open — a refusal of the orders its own
//! thread carries, the alarms' daemon word, the modeline's mark and the
//! pointer's sentence over it — and the power view says them in both of its
//! doors. The footer names no feature, so what both speak lives below both;
//! and not in the kit, which every shell crate stands on, so a line of the
//! power crate recompiles these words, the view and the binary, and nothing
//! of the foundation.

use ronoctua_power::{Keep, Order, Profile, Refusal, State};
use ronoctua_tokens as tokens;

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
    use crate::key;
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
        Refusal::TimedOut => key::POWER_REFUSED_TIMED_OUT,
        Refusal::Failed => key::POWER_REFUSED_FAILED,
        Refusal::NoBus => key::POWER_REFUSED_NO_BUS,
        Refusal::Other => key::POWER_REFUSED_OTHER,
    });
    // THE DAEMON'S OWN WORDS GO TO THE LOG: they are English, and the line
    // is in the person's language. Every refusal that reaches the line has
    // a name now; the words are for whoever reads the journal.
    tracing::info!(?why, words, "a power daemon refused");
    texts.t_with(
        key::ECHO_POWER_REFUSED,
        &[("what", &what), ("why", &reason)],
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
    use crate::key;
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
    use crate::key;
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
    use crate::key;
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
    use crate::key;
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

/// How long the battery has, in words — or nothing, when there is nothing to
/// say.
///
/// # Why it is one function
///
/// Because it was two, and they disagreed. The view matched `State::Charging`
/// exactly; the pointer's sentence on the minibuffer matched
/// `state.is_charging()`, which also covers `PendingCharge` — the state a
/// charge threshold or a firmware pause produces. So a battery in
/// `PendingCharge` with a `TimeToFull` read "full in 1 h 20" under the
/// pointer and "unknown" in the view, two lines apart on the same screen.
///
/// `is_charging()` is the right one of the two: `PendingCharge` is charging
/// that has not started moving, and the daemon's own estimate is the best
/// answer there is. `FullyCharged` says nothing at all, because the state
/// word already says it.
#[must_use]
pub fn time_left(battery: &ronoctua_power::Battery, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    if battery.state == State::FullyCharged {
        return String::new();
    }
    match (
        battery.state.is_charging(),
        battery.time_to_empty,
        battery.time_to_full,
    ) {
        (false, Some(t), _) => {
            texts.t_with(key::POWER_TIME_EMPTY, &[("time", &duration_text(t, texts))])
        }
        (true, _, Some(t)) => {
            texts.t_with(key::POWER_TIME_FULL, &[("time", &duration_text(t, texts))])
        }
        _ => texts.t(key::POWER_TIME_UNKNOWN),
    }
}

/// `1 h 23 min`, in the person's language.
#[must_use]
pub fn duration_text(d: std::time::Duration, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    use ronoctua_i18n::Arg;
    let minutes = d.as_secs() / 60;
    let (h, m) = (minutes / 60, minutes % 60);
    let h = i64::try_from(h).unwrap_or(i64::MAX);
    let m = i64::try_from(m).unwrap_or(0);
    if h == 0 {
        texts.t_args(key::POWER_DURATION_MINUTES, &[("m", Arg::Number(m))])
    } else if m == 0 {
        texts.t_args(key::POWER_DURATION_HOURS, &[("h", Arg::Number(h))])
    } else {
        texts.t_args(
            key::POWER_DURATION,
            &[("h", Arg::Number(h)), ("m", Arg::Number(m))],
        )
    }
}

/// The modeline's battery segment: the glyph, and the number at
/// [`ronoctua_power::PERCENT_UNTIL`] and below — one cell between them, the
/// bell's measured reason (`main.rs`).
///
/// One place for the ladder-to-token map: the rung and the charging flag pick
/// the drawing, and the danger colour is the paint's to add (`segment_color`).
#[must_use]
pub fn battery_segment(m: &ronoctua_power::Mark) -> String {
    use ronoctua_power::Rung;
    // DANGER IS A COLOUR, NOT A DRAWING — and this used to be both.
    //
    // The owner asked for the colour to change at 15% and below; the glyph
    // was ALSO swapped for `battery-danger`, which is the same picture
    // whether the machine is on the charger or not. Nine per cent draining
    // and nine per cent charging drew the identical mark, at the one moment
    // when "is it charging?" is the question. Two rungs of the ladder
    // (`battery-empty`, `battery-charging-empty`) were unreachable as well,
    // because `Rung::Empty` is 0-10 and danger is 0-15.
    //
    // The exclamation mark is kept for the one case it says something the
    // ladder cannot: low AND unplugged, which is the state nobody wants to
    // discover late. Charging at the same percentage draws the charging
    // rung, in the danger colour.
    let glyph = if m.danger && !m.charging {
        tokens::icon::BATTERY_DANGER
    } else {
        match (m.rung, m.charging) {
            // `Rung::Empty` CANNOT ARRIVE HERE UNPLUGGED, and that is the
            // owner's rule and not an oversight: the rung is 0-10 and danger
            // is 0-15, so the branch above has already taken it. It used to
            // have an arm of its own drawing `battery-empty`, a token nothing
            // could reach — with a comment and a TEST that both said it
            // could, the test by building a `Mark` that `mark()` cannot
            // produce (`rung: Empty, danger: false`). The two rungs share the
            // one drawing that IS reachable.
            (Rung::Empty | Rung::Low, false) => tokens::icon::BATTERY_LOW,
            (Rung::Half, false) => tokens::icon::BATTERY_HALF,
            (Rung::High, false) => tokens::icon::BATTERY_HIGH,
            (Rung::Full, false) => tokens::icon::BATTERY_FULL,
            (Rung::Empty, true) => tokens::icon::BATTERY_CHARGING_EMPTY,
            (Rung::Low, true) => tokens::icon::BATTERY_CHARGING_LOW,
            (Rung::Half, true) => tokens::icon::BATTERY_CHARGING_HALF,
            (Rung::High, true) => tokens::icon::BATTERY_CHARGING_HIGH,
            (Rung::Full, true) => tokens::icon::BATTERY_CHARGING_FULL,
        }
    };
    match m.percent {
        Some(p) => format!("{glyph} {p}%"),
        None => glyph.to_string(),
    }
}

/// The profile's glyph — the leaf, the middle gauge, the full gauge.
#[must_use]
pub const fn profile_glyph(p: Profile) -> char {
    match p {
        Profile::PowerSaver => tokens::icon::PROFILE_POWER_SAVER,
        Profile::Balanced => tokens::icon::PROFILE_BALANCED,
        Profile::Performance => tokens::icon::PROFILE_PERFORMANCE,
    }
}

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

    #[test]
    fn durations_read_as_hours_and_minutes() {
        let t = texts();
        assert_eq!(
            duration_text(std::time::Duration::from_secs(4_980), &t),
            "1 h 23 min"
        );
        assert_eq!(
            duration_text(std::time::Duration::from_secs(600), &t),
            "10 min"
        );
    }
}

#[cfg(test)]
mod segment_tests {
    use super::*;
    use ronoctua_power::{Mark, Rung};
    use ronoctua_render::layout;

    #[test]
    fn the_segment_is_one_glyph_and_the_number_only_when_low() {
        let quiet = Mark {
            rung: Rung::Half,
            charging: false,
            percent: None,
            danger: false,
        };
        assert_eq!(layout::width(&battery_segment(&quiet)), 1);
        let low = Mark {
            rung: Rung::Low,
            charging: true,
            percent: Some(22),
            danger: false,
        };
        let text = battery_segment(&low);
        assert!(text.ends_with(" 22%"), "{text}");
        assert!(
            text.starts_with(tokens::icon::BATTERY_CHARGING_LOW),
            "the bolt"
        );
        let danger = Mark {
            rung: Rung::Empty,
            charging: false,
            percent: Some(9),
            danger: true,
        };
        assert!(battery_segment(&danger).starts_with(tokens::icon::BATTERY_DANGER));
    }

    #[test]
    fn the_glyph_still_says_whether_it_is_charging_at_the_danger_mark() {
        // NINE PER CENT ON THE CHARGER AND NINE PER CENT DRAINING drew the
        // same picture, because the danger branch came first and
        // `battery-danger` is one drawing. That is the one moment when "is it
        // charging?" is the whole question. The COLOUR is what danger
        // changes — `segment_color` — and the drawing is the ladder's.
        let charging = Mark {
            rung: Rung::Empty,
            charging: true,
            percent: Some(9),
            danger: true,
        };
        assert!(
            battery_segment(&charging).starts_with(tokens::icon::BATTERY_CHARGING_EMPTY),
            "the danger mark hid the charging"
        );
        let draining = Mark {
            charging: false,
            ..charging
        };
        assert_ne!(
            battery_segment(&draining),
            battery_segment(&charging),
            "the two states draw the same thing"
        );
        // AND THE BOTTOM RUNG IS NOT DRAWN UNPLUGGED, which is the rule and
        // not an accident: `Rung::Empty` is 0-10 and danger is 0-15, so
        // `mark()` cannot produce an empty-and-not-in-danger battery. The
        // assertion that used to be here built exactly that impossible
        // `Mark` and called the result proof.
        for percent in 0..=15_u8 {
            let m = Mark {
                rung: Rung::of(percent),
                charging: false,
                percent: Some(percent),
                danger: true,
            };
            assert!(
                battery_segment(&m).starts_with(tokens::icon::BATTERY_DANGER),
                "{percent}% unplugged should be the alert glyph"
            );
        }
    }
}
