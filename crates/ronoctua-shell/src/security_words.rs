//! The security centre's words — one place for every closed set that becomes
//! a sentence, so the footer's door and the window's cannot say one thing two
//! ways, and a variant with no words is a compile error (every key is a
//! constant of `ronoctua_i18n::key`).

use ronoctua_i18n::key;
use ronoctua_security::Why;
use ronoctua_security::firewall::{Action, LogLevel, Profile, Protocol};
use ronoctua_security::fprint::{Enrolling, Finger};
use ronoctua_security::platform::{SecureBoot, Tpm};

/// A refusal of the centre (the door, the helper, a reading), as a sentence.
#[must_use]
pub const fn refusal(why: Why) -> &'static str {
    match why {
        Why::NoHelper => key::SECURITY_REFUSAL_NO_HELPER,
        Why::WrongPassword => key::SECURITY_REFUSAL_WRONG_PASSWORD,
        Why::NeedsPassword => key::SECURITY_REFUSAL_NEEDS_PASSWORD,
        Why::NotAllowed => key::SECURITY_REFUSAL_NOT_ALLOWED,
        Why::Unstartable => key::SECURITY_REFUSAL_UNSTARTABLE,
        Why::Failed => key::SECURITY_REFUSAL_FAILED,
        Why::Invalid => key::SECURITY_REFUSAL_INVALID,
        Why::NoReader => key::SECURITY_REFUSAL_NO_READER,
        Why::Unreadable => key::SECURITY_REFUSAL_UNREADABLE,
        Why::Busy => key::SECURITY_REFUSAL_BUSY,
        Why::TimedOut => key::SECURITY_REFUSAL_TIMED_OUT,
    }
}

/// Why an online check gave nothing, as a sentence that names the service.
#[must_use]
pub fn lookup_refusal(r: ronoctua_lookup::Refusal, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_lookup::Why as W;
    let k = match r.why {
        W::NoKey => key::SECURITY_ONLINE_REFUSAL_NO_KEY,
        W::KeyRefused | W::WrongKey => key::SECURITY_ONLINE_REFUSAL_KEY_REFUSED,
        W::Quota | W::TooManyAsks => key::SECURITY_ONLINE_REFUSAL_TOO_MANY_ASKS,
        W::NothingFound => key::SECURITY_ONLINE_REFUSAL_NOTHING_FOUND,
        W::NothingToAsk => key::SECURITY_ONLINE_REFUSAL_NOTHING_TO_ASK,
        W::Unreachable | W::Dormant => key::SECURITY_ONLINE_REFUSAL_UNREACHABLE,
        W::Malformed | W::Declined => key::SECURITY_ONLINE_REFUSAL_MALFORMED,
        W::TooLarge => key::SECURITY_ONLINE_REFUSAL_TOO_LARGE,
        W::Unreadable | W::TooSmall => key::SECURITY_ONLINE_REFUSAL_UNREADABLE,
        W::NotLet => key::SECURITY_ONLINE_REFUSAL_NOT_LET,
    };
    texts.t_with(k, &[("service", &texts.t(service_key(r.service)))])
}

/// A lookup service's name — the security centre's three; the player's are
/// the player's (`player::service_key`), and never drawn here.
#[must_use]
pub const fn service_key(service: ronoctua_lookup::Service) -> &'static str {
    use ronoctua_lookup::Service as S;
    match service {
        S::XposedOrNot => key::SECURITY_SERVICE_XPOSEDORNOT,
        S::PwnedPasswords => key::SECURITY_SERVICE_PWNEDPASSWORDS,
        // VIRUSTOTAL, AND ANY SERVICE THAT IS NOT THE CENTRE'S: a refusal
        // names the service that refused, and the centre asks only its own.
        _ => key::SECURITY_SERVICE_VIRUSTOTAL,
    }
}

/// A finger's name.
#[must_use]
pub const fn finger(f: Finger) -> &'static str {
    match f {
        Finger::LeftThumb => key::SECURITY_FINGER_LEFT_THUMB,
        Finger::LeftIndex => key::SECURITY_FINGER_LEFT_INDEX_FINGER,
        Finger::LeftMiddle => key::SECURITY_FINGER_LEFT_MIDDLE_FINGER,
        Finger::LeftRing => key::SECURITY_FINGER_LEFT_RING_FINGER,
        Finger::LeftLittle => key::SECURITY_FINGER_LEFT_LITTLE_FINGER,
        Finger::RightThumb => key::SECURITY_FINGER_RIGHT_THUMB,
        Finger::RightIndex => key::SECURITY_FINGER_RIGHT_INDEX_FINGER,
        Finger::RightMiddle => key::SECURITY_FINGER_RIGHT_MIDDLE_FINGER,
        Finger::RightRing => key::SECURITY_FINGER_RIGHT_RING_FINGER,
        Finger::RightLittle => key::SECURITY_FINGER_RIGHT_LITTLE_FINGER,
    }
}

/// What an enrolment step asks of the person, or says.
#[must_use]
pub const fn enrolling(e: Enrolling) -> &'static str {
    match e {
        Enrolling::Completed => key::SECURITY_FINGER_STEP_COMPLETED,
        Enrolling::StagePassed => key::SECURITY_FINGER_STEP_STAGE_PASSED,
        Enrolling::RetryScan => key::SECURITY_FINGER_STEP_RETRY_SCAN,
        Enrolling::SwipeTooShort => key::SECURITY_FINGER_STEP_SWIPE_TOO_SHORT,
        Enrolling::NotCentred => key::SECURITY_FINGER_STEP_NOT_CENTRED,
        Enrolling::RemoveAndRetry => key::SECURITY_FINGER_STEP_REMOVE_AND_RETRY,
        Enrolling::TooFast => key::SECURITY_FINGER_STEP_TOO_FAST,
        Enrolling::Duplicate => key::SECURITY_FINGER_STEP_DUPLICATE,
        Enrolling::DataFull => key::SECURITY_FINGER_STEP_DATA_FULL,
        Enrolling::Failed => key::SECURITY_FINGER_STEP_FAILED,
        Enrolling::Disconnected => key::SECURITY_FINGER_STEP_DISCONNECTED,
        Enrolling::Unknown => key::SECURITY_FINGER_STEP_UNKNOWN,
    }
}

/// Secure Boot's state.
#[must_use]
pub const fn secure_boot(s: SecureBoot) -> &'static str {
    match s {
        SecureBoot::Enabled => key::SECURITY_DEVICE_SECURE_BOOT_ENABLED,
        SecureBoot::Disabled => key::SECURITY_DEVICE_SECURE_BOOT_DISABLED,
        SecureBoot::SetupMode => key::SECURITY_DEVICE_SECURE_BOOT_SETUP_MODE,
        SecureBoot::Unsupported => key::SECURITY_DEVICE_SECURE_BOOT_UNSUPPORTED,
    }
}

/// The TPM, as a sentence with its version.
#[must_use]
pub fn tpm(t: Tpm, texts: &ronoctua_i18n::Texts) -> String {
    match t {
        Tpm::Absent => texts.t(key::SECURITY_DEVICE_TPM_ABSENT),
        Tpm::Present { version } => texts.t_args(
            key::SECURITY_DEVICE_TPM_PRESENT,
            &[("version", ronoctua_i18n::Arg::Number(i64::from(version)))],
        ),
    }
}

/// A network's profile.
#[must_use]
pub const fn profile(p: Profile) -> &'static str {
    match p {
        Profile::Home => key::SECURITY_FW_PROFILE_HOME,
        Profile::Public => key::SECURITY_FW_PROFILE_PUBLIC,
    }
}

/// How much the refusals are logged.
#[must_use]
pub const fn log_level(l: LogLevel) -> &'static str {
    match l {
        LogLevel::Off => key::SECURITY_FW_LOG_OFF,
        LogLevel::Quiet => key::SECURITY_FW_LOG_QUIET,
        LogLevel::Chatty => key::SECURITY_FW_LOG_CHATTY,
    }
}

/// What a rule does — also the word a typed rule begins with.
#[must_use]
pub const fn action(a: Action) -> &'static str {
    match a {
        Action::Allow => key::SECURITY_RULE_WORD_ALLOW,
        Action::Deny => key::SECURITY_RULE_WORD_DENY,
    }
}

/// A protocol, as it is written in a rule (the same in every language).
#[must_use]
pub const fn protocol(p: Protocol) -> &'static str {
    match p {
        Protocol::Tcp => "tcp",
        Protocol::Udp => "udp",
    }
}

/// How the hardware address shows on a connection.
#[must_use]
pub const fn mac(m: ronoctua_network::privacy::Mac) -> &'static str {
    use ronoctua_network::privacy::Mac;
    match m {
        Mac::Permanent => key::SECURITY_PRIVACY_MAC_PERMANENT,
        Mac::Preserve => key::SECURITY_PRIVACY_MAC_PRESERVE,
        Mac::Random => key::SECURITY_PRIVACY_MAC_RANDOM,
        Mac::Stable => key::SECURITY_PRIVACY_MAC_STABLE,
        Mac::Explicit => key::SECURITY_PRIVACY_MAC_EXPLICIT,
        Mac::Default => key::SECURITY_PRIVACY_MAC_DEFAULT,
    }
}

/// Whether a connection's DNS is encrypted.
#[must_use]
pub const fn dns(t: ronoctua_network::privacy::Tls) -> &'static str {
    use ronoctua_network::privacy::Tls;
    match t {
        Tls::Yes => key::SECURITY_PRIVACY_DNS_YES,
        Tls::Opportunistic => key::SECURITY_PRIVACY_DNS_OPPORTUNISTIC,
        Tls::No => key::SECURITY_PRIVACY_DNS_NO,
        Tls::Default => key::SECURITY_PRIVACY_DNS_DEFAULT,
    }
}

/// A VirusTotal verdict, as a sentence with the engines' counts.
#[must_use]
pub fn verdict(stats: ronoctua_lookup::virustotal::Stats, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::Arg;
    use ronoctua_lookup::virustotal::Verdict;
    let k = match stats.verdict() {
        Verdict::Clean => key::SECURITY_VT_VERDICT_CLEAN,
        Verdict::Suspicious => key::SECURITY_VT_VERDICT_SUSPICIOUS,
        Verdict::Malicious => key::SECURITY_VT_VERDICT_MALICIOUS,
    };
    texts.t_args(
        k,
        &[
            (
                "found",
                Arg::Number(i64::from(stats.malicious + stats.suspicious)),
            ),
            ("engines", Arg::Number(i64::from(stats.engines()))),
        ],
    )
}

/// Where one file's check is, as a sentence.
#[must_use]
pub fn scan_state(
    state: &ronoctua_lookup::virustotal::State,
    texts: &ronoctua_i18n::Texts,
) -> String {
    use ronoctua_i18n::Arg;
    use ronoctua_lookup::virustotal::State;
    match state {
        State::Asking => texts.t(key::SECURITY_VT_STATE_ASKING),
        State::Unknown => texts.t(key::SECURITY_VT_STATE_UNKNOWN),
        State::Sending { sent, of } => texts.t_args(
            key::SECURITY_VT_STATE_SENDING,
            &[(
                "percent",
                Arg::Number(i64::try_from(sent.saturating_mul(100) / (*of).max(1)).unwrap_or(0)),
            )],
        ),
        State::Pending => texts.t(key::SECURITY_VT_STATE_PENDING),
        State::Checked { report } => verdict(report.stats, texts),
        State::Failed { why } => lookup_refusal(
            ronoctua_lookup::Refusal::new(ronoctua_lookup::Service::VirusTotal, *why),
            texts,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    /// FAILS IF: a variant of a closed set has no words in a language — one
    /// frame per variant of every enum that becomes a key.
    #[test]
    fn every_variant_has_words_in_every_language() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut said: Vec<String> = Vec::new();
            said.extend(Why::ALL.map(|w| t.t(refusal(w))));
            said.extend(Finger::ALL.map(|f| t.t(finger(f))));
            said.extend(Enrolling::ALL.map(|e| t.t(enrolling(e))));
            said.extend(
                [
                    SecureBoot::Enabled,
                    SecureBoot::Disabled,
                    SecureBoot::SetupMode,
                    SecureBoot::Unsupported,
                ]
                .map(|s| t.t(secure_boot(s))),
            );
            said.push(tpm(Tpm::Absent, &t));
            said.push(tpm(Tpm::Present { version: 2 }, &t));
            said.extend([Profile::Home, Profile::Public].map(|p| t.t(profile(p))));
            said.extend(
                [LogLevel::Off, LogLevel::Quiet, LogLevel::Chatty].map(|l| t.t(log_level(l))),
            );
            said.extend([Action::Allow, Action::Deny].map(|a| t.t(action(a))));
            said.extend(ronoctua_network::privacy::Mac::ALL.map(|m| t.t(mac(m))));
            said.extend(ronoctua_network::privacy::Tls::ALL.map(|x| t.t(dns(x))));
            for why in ronoctua_lookup::Why::ALL {
                for service in ronoctua_lookup::Service::ALL
                    .into_iter()
                    .filter(|s| s.scope() == ronoctua_lookup::Scope::Security)
                {
                    said.push(lookup_refusal(
                        ronoctua_lookup::Refusal::new(service, why),
                        &t,
                    ));
                }
            }
            use ronoctua_lookup::virustotal::{Report, State, Stats};
            for state in [
                State::Asking,
                State::Unknown,
                State::Sending { sent: 5, of: 10 },
                State::Pending,
                State::Checked {
                    report: Report {
                        stats: Stats {
                            malicious: 2,
                            suspicious: 0,
                            undetected: 60,
                            harmless: 0,
                        },
                        analysed_s: None,
                    },
                },
                State::Failed {
                    why: ronoctua_lookup::Why::TooLarge,
                },
            ] {
                said.push(scan_state(&state, &t));
            }
            for s in &said {
                assert!(
                    !s.is_empty() && !s.starts_with('['),
                    "{language}: a missing key: {s}"
                );
            }
        }
    }
}
