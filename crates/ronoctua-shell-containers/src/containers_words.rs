//! The sentences the two containers' doors share — a refusal, a state, a
//! kind, a tab, what the queue was given — and the firewall's verdict on a
//! published port: one place, so the footer and the window cannot say it
//! two ways.
//!
//! What is podman's own — a status line (`Up 3 hours`), an image's name, a
//! log line — is DATA and is drawn as it came (invariant 8, the brain's log
//! line's exception). What is ours is a key.

use crate::key;
use ronoctua_containers::{Job, Port, Refusal, State, Why};
use ronoctua_security::firewall::{Action as RuleAction, Profile, Protocol, Settings, Status};

use crate::containers::{Kind, Tab};

/// A kind's name, in the person's language.
#[must_use]
pub const fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Containers => key::CONTAINERS_KIND_CONTAINERS,
        Kind::Pods => key::CONTAINERS_KIND_PODS,
        Kind::Compose => key::CONTAINERS_KIND_COMPOSE,
        Kind::Images => key::CONTAINERS_KIND_IMAGES,
        Kind::Volumes => key::CONTAINERS_KIND_VOLUMES,
        Kind::Networks => key::CONTAINERS_KIND_NETWORKS,
    }
}

/// What a kind's list says when it is empty.
#[must_use]
pub const fn empty_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Containers => key::CONTAINERS_EMPTY_CONTAINERS,
        Kind::Pods => key::CONTAINERS_EMPTY_PODS,
        Kind::Compose => key::CONTAINERS_EMPTY_COMPOSE,
        Kind::Images => key::CONTAINERS_EMPTY_IMAGES,
        Kind::Volumes => key::CONTAINERS_EMPTY_VOLUMES,
        Kind::Networks => key::CONTAINERS_EMPTY_NETWORKS,
    }
}

/// A kind's glyph token.
#[must_use]
pub const fn kind_glyph(kind: Kind) -> char {
    use ronoctua_tokens::icon;
    match kind {
        Kind::Containers => icon::CONTAINERS_KIND_CONTAINERS,
        Kind::Pods => icon::CONTAINERS_KIND_PODS,
        Kind::Compose => icon::CONTAINERS_KIND_COMPOSE,
        Kind::Images => icon::CONTAINERS_KIND_IMAGES,
        Kind::Volumes => icon::CONTAINERS_KIND_VOLUMES,
        Kind::Networks => icon::CONTAINERS_KIND_NETWORKS,
    }
}

/// A tab's name.
#[must_use]
pub const fn tab_key(tab: Tab) -> &'static str {
    match tab {
        Tab::Overview => key::CONTAINERS_TAB_OVERVIEW,
        Tab::Logs => key::CONTAINERS_TAB_LOGS,
        Tab::Stats => key::CONTAINERS_TAB_STATS,
        Tab::Inspect => key::CONTAINERS_TAB_INSPECT,
        Tab::Ports => key::CONTAINERS_TAB_PORTS,
        Tab::Mounts => key::CONTAINERS_TAB_MOUNTS,
        Tab::Env => key::CONTAINERS_TAB_ENV,
    }
}

/// A container's state, as a word.
#[must_use]
pub const fn state_key(state: State) -> &'static str {
    match state {
        State::Created => key::CONTAINERS_STATE_CREATED,
        State::Running => key::CONTAINERS_STATE_RUNNING,
        State::Paused => key::CONTAINERS_STATE_PAUSED,
        State::Exited => key::CONTAINERS_STATE_EXITED,
        State::Stopping => key::CONTAINERS_STATE_STOPPING,
        State::Unknown => key::CONTAINERS_STATE_UNKNOWN,
    }
}

/// Why podman answered nothing, as a sentence.
#[must_use]
pub const fn why_key(why: Why) -> &'static str {
    match why {
        Why::NoPodman => key::CONTAINERS_WHY_NO_PODMAN,
        Why::Unstartable => key::CONTAINERS_WHY_UNSTARTABLE,
        Why::Failed => key::CONTAINERS_WHY_FAILED,
        Why::Garbled => key::CONTAINERS_WHY_GARBLED,
        Why::NotFound => key::CONTAINERS_WHY_NOT_FOUND,
        Why::Refused => key::CONTAINERS_WHY_REFUSED,
        Why::TooLarge => key::CONTAINERS_WHY_TOO_LARGE,
    }
}

/// A refusal of podman's, said.
#[must_use]
pub fn refused(refusal: Refusal, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(why_key(refusal.why))
}

/// What a job the queue was given is, in words: the queue's own stem for
/// it (`transfers-work-container-…`), so the view and the queue's panel say
/// it the same way.
#[must_use]
pub fn job_said(job: &Job, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(&format!("transfers-work-container-{}", job.word()))
}

/// What the message line says once a job went to the queue; `keys` are
/// [`queue_keys`].
#[must_use]
pub fn queued_said(job: &Job, label: &str, keys: &str, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        key::CONTAINERS_ECHO_QUEUED,
        &[
            ("what", &job_said(job, texts)),
            ("name", label),
            ("keys", keys),
        ],
    )
}

/// What the firewall does with a published port, for someone on another
/// machine.
///
/// # Read from the person's own table
///
/// From `firewall.json` (the person's rules and switches) and the helper's
/// status (which network each interface is on), both readable by the
/// person — `ronoctua_security::firewall`. A port rootless podman publishes
/// is a socket of this machine's (the rootless network helper listens on it), so the firewall
/// answers for it as for any other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Bound to this machine's loopback: nothing from outside reaches it.
    Local,
    /// The person turned the firewall off: it answers to everyone.
    Open,
    /// A rule of the person's lets it in on every network the machine is on.
    Allowed,
    /// Only on the home networks: on a public one the firewall refuses it.
    HomeOnly,
    /// A rule of the person's lets in only the addresses, or the connection,
    /// it names: someone elsewhere may reach it, and not everyone.
    Narrowed,
    /// Shields are up: every new connection is dropped.
    Shields,
    /// No rule of the person's lets it in: the firewall refuses it.
    Refused,
    /// The firewall's settings could not be read.
    Unknown,
}

impl Verdict {
    /// Every verdict, for the census of texts.
    #[cfg(test)]
    pub const ALL: [Self; 8] = [
        Self::Local,
        Self::Open,
        Self::Allowed,
        Self::HomeOnly,
        Self::Narrowed,
        Self::Shields,
        Self::Refused,
        Self::Unknown,
    ];

    /// Its sentence.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Local => key::CONTAINERS_PORT_LOCAL,
            Self::Open => key::CONTAINERS_PORT_OPEN,
            Self::Allowed => key::CONTAINERS_PORT_ALLOWED,
            Self::HomeOnly => key::CONTAINERS_PORT_HOME_ONLY,
            Self::Narrowed => key::CONTAINERS_PORT_NARROWED,
            Self::Shields => key::CONTAINERS_PORT_SHIELDS,
            Self::Refused => key::CONTAINERS_PORT_REFUSED,
            Self::Unknown => key::CONTAINERS_PORT_UNKNOWN,
        }
    }

    /// The sentence under the ports that says what it MEANS and what helps —
    /// said once for every port it is true of. Each is its own: under
    /// shields no rule lets anything in, and lowering them is what helps.
    #[must_use]
    pub const fn explained(self) -> Option<&'static str> {
        match self {
            Self::Local | Self::Allowed => None,
            Self::HomeOnly | Self::Refused => Some(key::CONTAINERS_PORTS_BLOCKED),
            Self::Narrowed => Some(key::CONTAINERS_PORTS_NARROWED),
            Self::Shields => Some(key::CONTAINERS_PORTS_SHIELDS),
            Self::Open => Some(key::CONTAINERS_PORTS_OPEN),
            Self::Unknown => Some(key::CONTAINERS_PORTS_UNKNOWN),
        }
    }

    /// Whether someone elsewhere cannot reach it, or not everyone can — the
    /// rows tinted as a warning.
    #[must_use]
    pub const fn blocked(self) -> bool {
        matches!(
            self,
            Self::HomeOnly | Self::Narrowed | Self::Shields | Self::Refused
        )
    }

    /// Whether the firewall's rules are where something is done about it:
    /// a port held out, or a firewall switched off.
    #[must_use]
    pub const fn leads_to_the_firewall(self) -> bool {
        self.blocked() || matches!(self, Self::Open)
    }
}

/// What the firewall read says of a published port. Pure.
///
/// `settings` is `firewall.json` (`None`: never written, the defaults —
/// on, no rules); `status` which profile each interface is on (`None`: the
/// helper never ran, so both profiles are possible); `unreadable` when the
/// settings exist and could not be read.
#[must_use]
pub fn verdict(
    port: &Port,
    settings: Option<&Settings>,
    status: Option<&Status>,
    unreadable: bool,
) -> Verdict {
    if port.host_ip.as_deref().is_some_and(|ip| {
        ip.parse::<std::net::IpAddr>()
            .is_ok_and(|a| a.is_loopback())
    }) {
        return Verdict::Local;
    }
    if unreadable {
        return Verdict::Unknown;
    }
    let fallback = Settings::default();
    let settings = settings.unwrap_or(&fallback);
    if settings.shields {
        return Verdict::Shields;
    }
    if !settings.enabled {
        return Verdict::Open;
    }
    let protocol = match port.protocol.as_str() {
        "tcp" => Protocol::Tcp,
        "udp" => Protocol::Udp,
        // SCTP has no rule of the person's: the declared firewall decides,
        // and it opens nothing.
        _ => return Verdict::Refused,
    };
    // THE PROFILES IN USE: the helper's word on each interface, or both when
    // it never said.
    let mut profiles: Vec<Profile> = status
        .map(|s| s.profiles.values().copied().collect())
        .unwrap_or_else(|| vec![Profile::Home, Profile::Public]);
    profiles.sort();
    profiles.dedup();
    if profiles.is_empty() {
        profiles = vec![Profile::Home, Profile::Public];
    }
    // EACH PROFILE IN USE, asked as the table asks it: the first rule that
    // matches decides for everyone — but a rule narrowed to a source or an
    // interface decides only for whom it names, so it is passed over, and
    // remembered when it lets those in.
    let reach = |profile: Profile| {
        let mut some = false;
        for rule in settings.rules.iter().filter(|r| {
            r.protocol == protocol
                && r.profiles.contains(&profile)
                && (r.ports.is_empty()
                    || r.ports
                        .iter()
                        .any(|&(a, b)| (a..=b).contains(&port.host_port)))
        }) {
            if rule.source.is_some() || rule.interface.is_some() {
                some |= rule.action == RuleAction::Allow;
                continue;
            }
            return if rule.action == RuleAction::Allow {
                Reach::Everyone
            } else if some {
                Reach::Some
            } else {
                Reach::Nobody
            };
        }
        if some { Reach::Some } else { Reach::Nobody }
    };
    let home = profiles
        .contains(&Profile::Home)
        .then(|| reach(Profile::Home));
    let public = profiles
        .contains(&Profile::Public)
        .then(|| reach(Profile::Public));
    let all = [home, public];
    let in_use = all.iter().flatten();
    if in_use.clone().all(|r| *r == Reach::Everyone) {
        Verdict::Allowed
    } else if home == Some(Reach::Everyone) {
        Verdict::HomeOnly
    } else if in_use.clone().any(|r| *r != Reach::Nobody) {
        Verdict::Narrowed
    } else {
        Verdict::Refused
    }
}

/// Who a profile's rules let in on a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// Everyone on a network of that profile.
    Everyone,
    /// Only the addresses or the connection a rule names.
    Some,
    /// Nobody.
    Nobody,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_security::firewall::Rule;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn port(host_ip: Option<&str>, host_port: u16, protocol: &str) -> Port {
        Port {
            host_ip: host_ip.map(str::to_owned),
            host_port,
            container_port: 80,
            protocol: protocol.to_owned(),
        }
    }

    fn allow(profiles: &[Profile], from: u16, to: u16) -> Rule {
        Rule {
            id: "r-web".to_owned(),
            action: RuleAction::Allow,
            protocol: Protocol::Tcp,
            ports: vec![(from, to)],
            source: None,
            interface: None,
            profiles: profiles.to_vec(),
            note: String::new(),
        }
    }

    fn status(profiles: &[Profile]) -> Status {
        let links: ronoctua_security::firewall::Links = profiles
            .iter()
            .enumerate()
            .map(|(n, _)| (format!("eth{n}"), format!("c{n}")))
            .collect();
        let mut settings = Settings::default();
        for (n, p) in profiles.iter().enumerate() {
            settings.networks.insert(format!("c{n}"), *p);
        }
        Status::of(&settings, &links, Ok(()))
    }

    #[test]
    fn a_published_port_is_judged_as_the_table_would() {
        let web = port(None, 8080, "tcp");
        // THE DEFAULTS: on, no rule — the declared firewall refuses it.
        assert_eq!(verdict(&web, None, None, false), Verdict::Refused);
        // LOOPBACK is nobody else's business.
        assert_eq!(
            verdict(&port(Some("127.0.0.1"), 8080, "tcp"), None, None, false),
            Verdict::Local
        );
        let mut settings = Settings::default();
        settings
            .rules
            .push(allow(&[Profile::Home, Profile::Public], 8000, 8099));
        assert_eq!(
            verdict(&web, Some(&settings), None, false),
            Verdict::Allowed
        );
        // A RULE FOR HOME ONLY, on a machine that may be on either.
        settings.rules = vec![allow(&[Profile::Home], 8080, 8080)];
        assert_eq!(
            verdict(&web, Some(&settings), None, false),
            Verdict::HomeOnly
        );
        // …and on a machine that is on a home network only, that is all.
        assert_eq!(
            verdict(
                &web,
                Some(&settings),
                Some(&status(&[Profile::Home])),
                false
            ),
            Verdict::Allowed
        );
        assert_eq!(
            verdict(
                &web,
                Some(&settings),
                Some(&status(&[Profile::Public])),
                false
            ),
            Verdict::Refused
        );
        // UDP is not TCP.
        assert_eq!(
            verdict(&port(None, 8080, "udp"), Some(&settings), None, false),
            Verdict::Refused
        );
        // THE FIRST RULE DECIDES: a deny before the allow wins.
        let mut deny = allow(&[Profile::Home, Profile::Public], 8080, 8080);
        deny.action = RuleAction::Deny;
        settings.rules = vec![deny, allow(&[Profile::Home, Profile::Public], 8080, 8080)];
        assert_eq!(
            verdict(&web, Some(&settings), None, false),
            Verdict::Refused
        );
        // SWITCHES: off answers everyone, shields drop everything.
        let off = Settings {
            enabled: false,
            ..Settings::default()
        };
        assert_eq!(verdict(&web, Some(&off), None, false), Verdict::Open);
        let shields = Settings {
            shields: true,
            ..Settings::default()
        };
        assert_eq!(verdict(&web, Some(&shields), None, false), Verdict::Shields);
        assert_eq!(verdict(&web, None, None, true), Verdict::Unknown);
    }

    #[test]
    fn a_rule_for_some_addresses_lets_some_in() {
        let web = port(None, 8080, "tcp");
        let mut lan = allow(&[Profile::Home, Profile::Public], 8080, 8080);
        lan.source = Some("192.168.1.0/24".to_owned());
        let mut settings = Settings {
            rules: vec![lan.clone()],
            ..Settings::default()
        };
        assert_eq!(
            verdict(&web, Some(&settings), None, false),
            Verdict::Narrowed
        );
        // …AND A BROADER RULE AFTER IT still lets everyone in.
        settings
            .rules
            .push(allow(&[Profile::Home, Profile::Public], 8000, 8099));
        assert_eq!(
            verdict(&web, Some(&settings), None, false),
            Verdict::Allowed
        );
        // A NARROWED DENY decides nothing for everyone else.
        let mut deny = lan;
        deny.action = RuleAction::Deny;
        settings.rules = vec![deny, allow(&[Profile::Home, Profile::Public], 8080, 8080)];
        assert_eq!(
            verdict(&web, Some(&settings), None, false),
            Verdict::Allowed
        );
        // A RULE FOR AN INTERFACE is narrowed too.
        let mut wired = allow(&[Profile::Home, Profile::Public], 8080, 8080);
        wired.interface = Some("eth0".to_owned());
        settings.rules = vec![wired];
        assert_eq!(
            verdict(&web, Some(&settings), None, false),
            Verdict::Narrowed
        );
    }

    #[test]
    fn every_verdict_says_what_helps_in_its_own_words() {
        let t = texts();
        // SHIELDS ARE NOT A MISSING RULE: their sentence is their own.
        assert_ne!(Verdict::Shields.explained(), Verdict::Refused.explained());
        assert_ne!(Verdict::Narrowed.explained(), Verdict::Refused.explained());
        for v in Verdict::ALL {
            if let Some(k) = v.explained() {
                assert!(!t.t(k).starts_with('['), "no text for {k}");
            }
        }
    }

    #[test]
    fn every_verdict_state_kind_tab_and_refusal_has_its_words() {
        let t = texts();
        let said = |k: &str| {
            let s = t.t(k);
            assert!(!s.starts_with('['), "no text for {k}");
            s
        };
        for v in Verdict::ALL {
            said(v.key());
        }
        for w in Why::ALL {
            said(why_key(w));
        }
        for k in Kind::ALL {
            said(kind_key(k));
            said(empty_key(k));
        }
        for tab in Tab::ALL {
            said(tab_key(tab));
        }
        for s in [
            State::Created,
            State::Running,
            State::Paused,
            State::Exited,
            State::Stopping,
            State::Unknown,
        ] {
            said(state_key(s));
        }
        for word in Job::WORDS {
            said(&format!("transfers-work-container-{word}"));
        }
    }
}
