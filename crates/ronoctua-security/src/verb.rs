//! The closed sets of what the root helper does, and what it says back.
//!
//! The shell hands a [`RootVerb`] to the sudo door; the door passes
//! [`RootVerb::arguments`] after the helper's store path; the helper, as
//! root, reads them back with [`RootVerb::parse`] and refuses anything else.
//! A unit or NetworkManager's dispatcher passes a [`UnitVerb`] instead, and
//! the helper refuses one of those that arrives through sudo. Pure on both
//! sides, so the contract between them is a test.
//!
//! The helper answers in [`Said`] lines on its standard output: progress
//! first (an enrolment's touches), the verdict last. A failure with no
//! verdict line is sudo's own, which is how the door tells a wrong password
//! from a helper that refused.

use serde::{Deserialize, Serialize};

use crate::Why;
use crate::fprint::Finger;

/// How long the passwordless window stays open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelaxMinutes {
    /// Five minutes.
    Five,
    /// Fifteen minutes.
    Fifteen,
    /// Thirty minutes.
    Thirty,
    /// An hour.
    Sixty,
}

impl RelaxMinutes {
    /// The four, in the ring's order.
    pub const ALL: [Self; 4] = [Self::Five, Self::Fifteen, Self::Thirty, Self::Sixty];

    /// The minutes.
    #[must_use]
    pub const fn minutes(self) -> u32 {
        match self {
            Self::Five => 5,
            Self::Fifteen => 15,
            Self::Thirty => 30,
            Self::Sixty => 60,
        }
    }

    /// The window of these minutes; `None` for any other number.
    #[must_use]
    pub fn of_minutes(minutes: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.minutes() == minutes)
    }
}

/// Which fingers a deletion removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fingers {
    /// One finger.
    One(Finger),
    /// Every enrolled finger.
    All,
}

/// What the root helper does for a person — nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "kebab-case")]
pub enum RootVerb {
    /// Open the passwordless window for these minutes.
    Relax {
        /// How long.
        minutes: RelaxMinutes,
    },
    /// Close it now.
    RelaxEnd,
    /// Validate the staged wish, write `firewall.json`, apply the table.
    /// From a unit (no `SUDO_UID`): apply `firewall.json` as it is.
    FirewallApply,
    /// Enrol a finger for the invoking user (fprintd, as root: adding a
    /// credential must require the existing one).
    FingerEnroll {
        /// Which finger.
        finger: Finger,
    },
    /// Delete enrolled fingers of the invoking user.
    FingerDelete {
        /// Which.
        which: Fingers,
    },
    /// Let (or stop letting) fingerprints answer sudo in terminals.
    FingerSudo {
        /// On or off.
        on: bool,
    },
    /// Start the file manager's root helper unit (chown, chmod, trim), keeping
    /// that unit's hardening.
    FilesRoot,
}

impl RootVerb {
    /// Whether the verb acts for a person, so the helper needs the one
    /// `SUDO_UID` names. `RelaxEnd` and `FirewallApply` are also what units
    /// run (the expiry timer, the sleep hook, the boot's table).
    #[must_use]
    pub const fn needs_a_person(&self) -> bool {
        !matches!(self, Self::RelaxEnd | Self::FirewallApply)
    }

    /// The helper's arguments for it. Pure.
    #[must_use]
    pub fn arguments(&self) -> Vec<String> {
        let words: Vec<&str> = match self {
            Self::Relax { minutes } => {
                return vec!["relax".to_owned(), minutes.minutes().to_string()];
            }
            Self::RelaxEnd => vec!["relax-end"],
            Self::FirewallApply => vec!["firewall-apply"],
            Self::FingerEnroll { finger } => vec!["finger-enroll", finger.id()],
            Self::FingerDelete {
                which: Fingers::One(finger),
            } => vec!["finger-delete", finger.id()],
            Self::FingerDelete {
                which: Fingers::All,
            } => vec!["finger-delete", "all"],
            Self::FingerSudo { on } => vec!["finger-sudo", if *on { "on" } else { "off" }],
            Self::FilesRoot => vec!["files-root"],
        };
        words.into_iter().map(str::to_owned).collect()
    }

    /// The verb of the helper's arguments; `None` for anything that is not
    /// exactly one. Pure.
    #[must_use]
    pub fn parse(args: &[String]) -> Option<Self> {
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        let verb = match words.as_slice() {
            ["relax", minutes] => Self::Relax {
                minutes: RelaxMinutes::of_minutes(minutes.parse().ok()?)?,
            },
            ["relax-end"] => Self::RelaxEnd,
            ["firewall-apply"] => Self::FirewallApply,
            ["finger-enroll", finger] => Self::FingerEnroll {
                finger: Finger::of_id(finger)?,
            },
            ["finger-delete", "all"] => Self::FingerDelete {
                which: Fingers::All,
            },
            ["finger-delete", finger] => Self::FingerDelete {
                which: Fingers::One(Finger::of_id(finger)?),
            },
            ["finger-sudo", "on"] => Self::FingerSudo { on: true },
            ["finger-sudo", "off"] => Self::FingerSudo { on: false },
            ["files-root"] => Self::FilesRoot,
            _ => return None,
        };
        Some(verb)
    }
}

/// What a unit or NetworkManager asks the helper — never through the door.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnitVerb {
    /// A network came up on an interface, or went away from it: the profiles
    /// follow each network by event (`firewall-apply --link IFACE UUID
    /// up|down`, from the dispatcher).
    Link {
        /// The interface, checked ([`crate::firewall::interface_is_plain`]).
        interface: String,
        /// NetworkManager's connection UUID, checked
        /// ([`crate::firewall::connection_is_plain`]).
        connection: String,
        /// Up or down.
        up: bool,
    },
    /// Raise the shields: the one loosening-free change a person may make
    /// with no password (`shields-up`, from `ronoctua-firewall-shields`).
    ShieldsUp,
}

impl UnitVerb {
    /// The helper's arguments for it. Pure.
    #[must_use]
    pub fn arguments(&self) -> Vec<String> {
        match self {
            Self::Link {
                interface,
                connection,
                up,
            } => vec![
                "firewall-apply".to_owned(),
                "--link".to_owned(),
                interface.clone(),
                connection.clone(),
                if *up { "up" } else { "down" }.to_owned(),
            ],
            Self::ShieldsUp => vec!["shields-up".to_owned()],
        }
    }

    /// The verb of the helper's arguments; `None` for anything that is not
    /// exactly one, or names an interface or a connection that is not plain.
    /// Pure.
    #[must_use]
    pub fn parse(args: &[String]) -> Option<Self> {
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["firewall-apply", "--link", interface, connection, state] => {
                let up = match *state {
                    "up" => true,
                    "down" => false,
                    _ => return None,
                };
                (crate::firewall::interface_is_plain(interface)
                    && crate::firewall::connection_is_plain(connection))
                .then(|| Self::Link {
                    interface: (*interface).to_owned(),
                    connection: (*connection).to_owned(),
                    up,
                })
            }
            ["shields-up"] => Some(Self::ShieldsUp),
            _ => None,
        }
    }
}

/// One line the helper says on its standard output.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Said {
    /// Progress: a word of what is happening (fprintd's enrolment result, as
    /// `enroll-stage-passed`). Several may come.
    Step(String),
    /// The verdict: it was done.
    Done,
    /// The verdict: it was refused, and why.
    Refused(Why),
}

impl Said {
    /// The line (no line break). Pure.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Self::Step(word) => format!("step {word}"),
            Self::Done => "done".to_owned(),
            Self::Refused(why) => format!("refused {}", why.id()),
        }
    }

    /// What a line said; `None` for a line that is not one of these. A step's
    /// word is a single token of `a-z`, digits and `-`. Pure.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\r', '\n']);
        match line.split_once(' ') {
            None if line == "done" => Some(Self::Done),
            Some(("refused", id)) => Why::of_id(id).map(Self::Refused),
            Some(("step", word))
                if !word.is_empty()
                    && word.len() <= 64
                    && word
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') =>
            {
                Some(Self::Step(word.to_owned()))
            }
            _ => None,
        }
    }

    /// Whether it is the verdict (the last line).
    #[must_use]
    pub const fn is_verdict(&self) -> bool {
        !matches!(self, Self::Step(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(w: &[&str]) -> Vec<String> {
        w.iter().map(|&s| s.to_owned()).collect()
    }

    #[test]
    fn every_verb_crosses_to_the_helper_and_back() {
        let mut verbs = vec![
            RootVerb::RelaxEnd,
            RootVerb::FirewallApply,
            RootVerb::FingerDelete {
                which: Fingers::All,
            },
            RootVerb::FingerSudo { on: true },
            RootVerb::FingerSudo { on: false },
            RootVerb::FilesRoot,
        ];
        verbs.extend(RelaxMinutes::ALL.map(|minutes| RootVerb::Relax { minutes }));
        verbs.extend(Finger::ALL.map(|finger| RootVerb::FingerEnroll { finger }));
        verbs.extend(Finger::ALL.map(|finger| RootVerb::FingerDelete {
            which: Fingers::One(finger),
        }));
        for verb in verbs {
            assert_eq!(RootVerb::parse(&verb.arguments()), Some(verb));
            assert_eq!(UnitVerb::parse(&verb.arguments()), None, "{verb:?}");
        }
        assert_eq!(
            RootVerb::parse(&words(&["relax", "7"])),
            None,
            "a closed set of minutes"
        );
        assert_eq!(RootVerb::parse(&words(&["relax", "-5"])), None);
        assert_eq!(RootVerb::parse(&words(&["relax-end", "now"])), None);
        assert_eq!(RootVerb::parse(&words(&["sh", "-c", "id"])), None);
        assert_eq!(RootVerb::parse(&words(&["finger-enroll", "nose"])), None);
        assert_eq!(RootVerb::parse(&words(&[])), None);
        assert!(!RootVerb::RelaxEnd.needs_a_person());
        assert!(!RootVerb::FirewallApply.needs_a_person());
        assert!(RootVerb::FilesRoot.needs_a_person());
    }

    #[test]
    fn unit_verbs_cross_too_and_refuse_what_is_not_plain() {
        let link = |up| UnitVerb::Link {
            interface: "wlp2s0".to_owned(),
            connection: "3f2a9c1e-0b7d-4e55-9a51-2c8d7e6f1a0b".to_owned(),
            up,
        };
        for verb in [link(true), link(false), UnitVerb::ShieldsUp] {
            assert_eq!(UnitVerb::parse(&verb.arguments()), Some(verb.clone()));
            assert_eq!(RootVerb::parse(&verb.arguments()), None);
        }
        let uuid = "3f2a9c1e-0b7d-4e55-9a51-2c8d7e6f1a0b";
        for bad in [
            words(&["firewall-apply", "--link", "wl p", uuid, "up"]),
            words(&["firewall-apply", "--link", "wlan0", "../../x", "up"]),
            words(&["firewall-apply", "--link", "wlan0", uuid, "sideways"]),
            words(&["firewall-apply", "--link", "", uuid, "up"]),
            words(&["firewall-apply", "--link", "wlan0", uuid]),
            words(&["shields-up", "now"]),
            words(&["shields-down"]),
        ] {
            assert_eq!(UnitVerb::parse(&bad), None, "{bad:?}");
        }
    }

    #[test]
    fn what_the_helper_says_is_read_back() {
        for said in [
            Said::Done,
            Said::Step("enroll-stage-passed".to_owned()),
            Said::Refused(Why::Invalid),
            Said::Refused(Why::TimedOut),
        ] {
            assert_eq!(Said::parse(&said.line()), Some(said.clone()));
            assert_eq!(Said::parse(&format!("{}\n", said.line())), Some(said));
        }
        for noise in [
            "",
            "sudo: a password is required",
            "done now",
            "refused nonsense",
            "step",
            "step two words",
            "step UPPER",
        ] {
            assert_eq!(Said::parse(noise), None, "{noise:?}");
        }
        assert!(Said::Done.is_verdict());
        assert!(!Said::Step("x".to_owned()).is_verdict());
    }
}
