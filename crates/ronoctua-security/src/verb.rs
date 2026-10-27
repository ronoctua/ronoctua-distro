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
///
/// The timed ones close at their end (sudo's own `NOTAFTER` in the line,
/// and a timer that takes the line away); [`RelaxSpan::NoLimit`] has no
/// end and closes only when the person closes it. Every one survives a
/// reboot (`crate::relax`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelaxSpan {
    /// Five minutes.
    Five,
    /// Fifteen minutes.
    Fifteen,
    /// Thirty minutes.
    Thirty,
    /// An hour.
    Sixty,
    /// A day.
    Day,
    /// Two days.
    TwoDays,
    /// Until the person closes it.
    NoLimit,
}

impl RelaxSpan {
    /// Every one, shortest first — the order a door offers them in.
    pub const ALL: [Self; 7] = [
        Self::Five,
        Self::Fifteen,
        Self::Thirty,
        Self::Sixty,
        Self::Day,
        Self::TwoDays,
        Self::NoLimit,
    ];

    /// How many minutes; `None` for [`RelaxSpan::NoLimit`].
    #[must_use]
    pub const fn minutes(self) -> Option<u32> {
        match self {
            Self::Five => Some(5),
            Self::Fifteen => Some(15),
            Self::Thirty => Some(30),
            Self::Sixty => Some(60),
            Self::Day => Some(24 * 60),
            Self::TwoDays => Some(48 * 60),
            Self::NoLimit => None,
        }
    }

    /// The helper's word for it: the minutes in digits, or `no-limit`.
    #[must_use]
    pub fn word(self) -> String {
        self.minutes()
            .map_or_else(|| NO_LIMIT.to_owned(), |minutes| minutes.to_string())
    }

    /// The span of a helper's word; `None` for any other word (a number of
    /// minutes that is not one of these included).
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|span| span.word() == word)
    }
}

/// The helper's word for [`RelaxSpan::NoLimit`].
const NO_LIMIT: &str = "no-limit";

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
///
/// Not `Copy`: two verbs carry words (the module and the hash an approval
/// was shown, the person's security keys), each checked by [`RootVerb::parse`]
/// before the helper acts on it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "kebab-case")]
pub enum RootVerb {
    /// Open the passwordless window for this long — or open it again for
    /// the same span, which is "start the count again" (`sudo -n` lets it
    /// through while the window is open).
    Relax {
        /// How long.
        span: RelaxSpan,
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
    /// Start the `/private` vault's root unit, which reads the request the
    /// person staged (the private-volume package).
    PrivateRoot,
    /// Start the stick writer's root unit, which waits for the staged `ready`
    /// (the stick package).
    StickWrite,
    /// Approve a person brain module's root part at the hash the person was
    /// shown: the helper pins the two in root's folder
    /// (`ronoctua_person::root::APPROVE_PIN`) and starts the approval's unit,
    /// which holds the staged request to them (the person-apply package).
    PersonModuleApprove {
        /// The module (`my-…`), as `ronoctua_person` reads one.
        id: String,
        /// The hash `nix/person-hash.nix` gave it, as shown.
        hash: String,
    },
    /// Let these security keys answer for the invoking person's password
    /// (`pam_u2f`), or none when empty: root's own list
    /// ([`crate::u2f::AUTHFILE`]), never the person's `u2f_keys`, which any
    /// of their processes may write.
    U2fKeys {
        /// The credentials, as `pamu2fcfg` writes them after the user name
        /// (`[crate::u2f::credential_is_plain]`), at most
        /// [`crate::u2f::MOST_KEYS`].
        credentials: Vec<String>,
    },
    /// Pin the person's signing key on this machine: the helper writes the
    /// key the door was given in root's folder
    /// (`ronoctua_person::root::SIGNER_PIN`) and starts the pin's unit,
    /// which writes `local/signer.json` (the plain-signature package). The
    /// key is the one the person's passphrase opened from `key.age`, as the
    /// door showed it — never the repository's copy.
    PersonSignerPin {
        /// The public key, as `ronoctua_person::local::key_is_plain` reads
        /// one.
        key: String,
    },
    // ── anchor: L06 ──
    // ── anchor: L12 ──
    // ── anchor: L17 ──
    // THE 2026-10-11 WAVE: the person's PRIVATE binary cache, which the
    // machine's Nix daemon reads through a netrc of root's
    // (`ronoctua_cachix::NETRC`). The token is never a word of this verb: the
    // gesture stages it in the person's runtime folder, and the helper takes
    // that copy (`ronoctua_cachix::netrc::take_staged`, `O_NOFOLLOW`, the
    // person's own file, `0600`) and deletes it.
    /// Write root's netrc for the person's private cache `name`.
    BinaryCacheNetrc {
        /// The cache, as `ronoctua_person::options::is_cache_name` reads one.
        name: String,
    },
    /// Remove root's netrc: the token went, or the cache is no longer
    /// private.
    BinaryCacheNetrcRemove,
}

impl RootVerb {
    /// Whether the verb acts for a person, so the helper needs the one
    /// `SUDO_UID` names. `FirewallApply` is also what a unit runs (the
    /// boot's table); the window's units have verbs of their own
    /// ([`UnitVerb::RelaxExpire`], [`UnitVerb::RelaxBoot`]).
    #[must_use]
    pub const fn needs_a_person(&self) -> bool {
        !matches!(self, Self::FirewallApply)
    }

    /// The helper's arguments for it. Pure.
    #[must_use]
    pub fn arguments(&self) -> Vec<String> {
        let words: Vec<&str> = match self {
            Self::Relax { span } => return vec!["relax".to_owned(), span.word()],
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
            Self::PrivateRoot => vec!["private-root"],
            Self::StickWrite => vec!["stick-write"],
            Self::PersonModuleApprove { id, hash } => {
                return vec!["person-module-approve".to_owned(), id.clone(), hash.clone()];
            }
            Self::U2fKeys { credentials } => {
                return std::iter::once("u2f-keys".to_owned())
                    .chain(credentials.iter().cloned())
                    .collect();
            }
            Self::BinaryCacheNetrc { name } => {
                return vec!["binary-cache-netrc".to_owned(), name.clone()];
            }
            Self::BinaryCacheNetrcRemove => vec!["binary-cache-netrc-remove"],
            Self::PersonSignerPin { key } => {
                return vec!["person-signer-pin".to_owned(), key.clone()];
            }
        };
        words.into_iter().map(str::to_owned).collect()
    }

    /// The verb of the helper's arguments; `None` for anything that is not
    /// exactly one. Pure.
    #[must_use]
    pub fn parse(args: &[String]) -> Option<Self> {
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        let verb = match words.as_slice() {
            ["relax", span] => Self::Relax {
                span: RelaxSpan::of_word(span)?,
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
            ["private-root"] => Self::PrivateRoot,
            ["stick-write"] => Self::StickWrite,
            ["person-module-approve", id, hash] => {
                // THE ONE GRAMMAR of an approval's two words: the person
                // crate's, which the unit reads the pin with.
                ronoctua_person::approve::of_pin(&ronoctua_person::approve::pin_text(id, hash))?;
                Self::PersonModuleApprove {
                    id: (*id).to_owned(),
                    hash: (*hash).to_owned(),
                }
            }
            // A KEY'S ONE WORD, in the person crate's one shape.
            ["person-signer-pin", key] if ronoctua_person::local::key_is_plain(key) => {
                Self::PersonSignerPin {
                    key: (*key).to_owned(),
                }
            }
            ["u2f-keys", credentials @ ..] => {
                if credentials.len() > crate::u2f::MOST_KEYS
                    || !credentials
                        .iter()
                        .all(|c| crate::u2f::credential_is_plain(c))
                {
                    return None;
                }
                Self::U2fKeys {
                    credentials: credentials.iter().map(|&c| c.to_owned()).collect(),
                }
            }
            // A CACHE'S NAME, by the one grammar the person's options are
            // judged with — a host label, never a path or a second word.
            ["binary-cache-netrc", name] if ronoctua_person::options::is_cache_name(name) => {
                Self::BinaryCacheNetrc {
                    name: (*name).to_owned(),
                }
            }
            ["binary-cache-netrc-remove"] => Self::BinaryCacheNetrcRemove,
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
    /// The window's own timer: close the window that ends at this instant
    /// (`relax-expire <until>`) — and only that one, so a timer that fires
    /// as the person opens a new window never closes the new one.
    RelaxExpire {
        /// The end the timer was armed for, in seconds since the epoch.
        until_unix_s: i64,
    },
    /// The boot's look at the window (`relax-boot`, from
    /// `ronoctua-sudo-window`): an expired or orphaned line goes, a timed
    /// one gets its timer back (a transient timer does not survive a
    /// reboot), one with no limit stays.
    RelaxBoot,
    /// Install the person's WireGuard key and locations
    /// (`ronoctua-vpn-install.service`, which the person starts through the
    /// one narrow polkit rule — NEVER the sudo door). The key is not a word
    /// of the verb: the gesture staged it in the person's runtime folder, and
    /// the unit takes it (`crate::vpn::install`). The person is named, not
    /// `SUDO_UID`'d, because a unit has no caller — the name is baked into the
    /// unit by `nix/modules/vpn.nix`, and the uid read from `/etc/passwd`.
    VpnInstall {
        /// The person whose staged key and locations to take, [`crate::users::plain`].
        user: String,
    },
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
            Self::RelaxExpire { until_unix_s } => {
                vec!["relax-expire".to_owned(), until_unix_s.to_string()]
            }
            Self::RelaxBoot => vec!["relax-boot".to_owned()],
            Self::VpnInstall { user } => vec!["vpn-install".to_owned(), user.clone()],
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
            // DIGITS ONLY: what `i64` parses besides them (a sign) is no end
            // a timer was armed for.
            ["relax-expire", until]
                if !until.is_empty() && until.bytes().all(|b| b.is_ascii_digit()) =>
            {
                until
                    .parse()
                    .ok()
                    .map(|until_unix_s| Self::RelaxExpire { until_unix_s })
            }
            ["relax-boot"] => Some(Self::RelaxBoot),
            // ONE PLAIN NAME, the person the unit was baked for, and nothing
            // that would let it say more.
            ["vpn-install", user] if crate::users::plain(user) => Some(Self::VpnInstall {
                user: (*user).to_owned(),
            }),
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

    /// A hash as `nix/person-hash.nix` writes one.
    const HASH: &str = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";

    /// A credential as `pamu2fcfg` writes one after the user name.
    const CREDENTIAL: &str = "owBYtXj1Pq+Zb5c1Kd/AbQ==,04d6f1aa,es256,+presence";

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
            RootVerb::PrivateRoot,
            RootVerb::StickWrite,
            RootVerb::PersonModuleApprove {
                id: "my-tea".to_owned(),
                hash: HASH.to_owned(),
            },
            RootVerb::U2fKeys {
                credentials: Vec::new(),
            },
            RootVerb::U2fKeys {
                credentials: vec![CREDENTIAL.to_owned(), CREDENTIAL.to_owned()],
            },
            RootVerb::BinaryCacheNetrc {
                name: "someone-2".to_owned(),
            },
            RootVerb::BinaryCacheNetrcRemove,
            RootVerb::PersonSignerPin {
                key: format!(
                    "{}{}",
                    ronoctua_person::local::KEY_PREFIX,
                    "A".repeat(ronoctua_person::local::KEY_BASE64_CHARS)
                ),
            },
        ];
        verbs.extend(RelaxSpan::ALL.map(|span| RootVerb::Relax { span }));
        verbs.extend(Finger::ALL.map(|finger| RootVerb::FingerEnroll { finger }));
        verbs.extend(Finger::ALL.map(|finger| RootVerb::FingerDelete {
            which: Fingers::One(finger),
        }));
        for verb in verbs {
            assert_eq!(RootVerb::parse(&verb.arguments()), Some(verb.clone()));
            assert_eq!(UnitVerb::parse(&verb.arguments()), None, "{verb:?}");
        }
        assert_eq!(
            RootVerb::parse(&words(&["relax", "7"])),
            None,
            "a closed set of minutes"
        );
        for wrong in ["-5", "05", "+15", "1440 ", "no limit", "No-Limit", "0", ""] {
            assert_eq!(
                RootVerb::parse(&words(&["relax", wrong])),
                None,
                "{wrong:?}"
            );
        }
        assert_eq!(
            RootVerb::parse(&words(&["relax", "no-limit"])),
            Some(RootVerb::Relax {
                span: RelaxSpan::NoLimit
            })
        );
        assert_eq!(
            RootVerb::parse(&words(&["relax", "2880"])),
            Some(RootVerb::Relax {
                span: RelaxSpan::TwoDays
            })
        );
        assert_eq!(RootVerb::parse(&words(&["relax-end", "now"])), None);
        assert_eq!(RootVerb::parse(&words(&["sh", "-c", "id"])), None);
        assert_eq!(RootVerb::parse(&words(&["finger-enroll", "nose"])), None);
        assert_eq!(RootVerb::parse(&words(&[])), None);
        assert!(
            RootVerb::RelaxEnd.needs_a_person(),
            "no unit closes the window through the door's verb"
        );
        assert!(!RootVerb::FirewallApply.needs_a_person());
        assert!(RootVerb::FilesRoot.needs_a_person());
        assert!(RootVerb::PrivateRoot.needs_a_person());
        assert!(RootVerb::StickWrite.needs_a_person());
        assert!(
            RootVerb::PersonModuleApprove {
                id: "my-tea".to_owned(),
                hash: HASH.to_owned()
            }
            .needs_a_person()
        );
        assert!(
            RootVerb::U2fKeys {
                credentials: Vec::new()
            }
            .needs_a_person()
        );
        // AN APPROVAL'S WORDS are an id and a hash, nothing else.
        for wrong in [
            &["person-module-approve"][..],
            &["person-module-approve", "my-tea"],
            &["person-module-approve", "tea", HASH],
            &["person-module-approve", "my-tea", "a\"b"],
            &["person-module-approve", "my-tea", HASH, "more"],
            &["person-module-approve", "my-tea\nmy-evil", HASH],
        ] {
            assert_eq!(RootVerb::parse(&words(wrong)), None, "{wrong:?}");
        }
        // A KEY'S WORDS are a credential's characters, and few.
        for wrong in [
            &["u2f-keys", "user:abc"][..],
            &["u2f-keys", "a b"],
            &["u2f-keys", ""],
        ] {
            assert_eq!(RootVerb::parse(&words(wrong)), None, "{wrong:?}");
        }
        let many: Vec<String> = std::iter::once("u2f-keys".to_owned())
            .chain(std::iter::repeat_n(
                CREDENTIAL.to_owned(),
                crate::u2f::MOST_KEYS + 1,
            ))
            .collect();
        assert_eq!(RootVerb::parse(&many), None);
        assert_eq!(
            RootVerb::parse(&words(&["private-root", "/etc/shadow"])),
            None,
            "a fixed unit takes no argument"
        );
        // A CACHE'S WORD is a cache's name, one, and never the token.
        for wrong in [
            &["binary-cache-netrc"][..],
            &["binary-cache-netrc", "Some_One"],
            &["binary-cache-netrc", "../../etc/shadow"],
            &[
                "binary-cache-netrc",
                "someone",
                "eyJhbGciOiJIUzI1NiJ9.token",
            ],
            &["binary-cache-netrc", "someone\nmachine evil.org"],
            &["binary-cache-netrc-remove", "someone"],
        ] {
            assert_eq!(RootVerb::parse(&words(wrong)), None, "{wrong:?}");
        }
        assert!(RootVerb::BinaryCacheNetrcRemove.needs_a_person());
    }

    /// FAILS IF: a span is said to the helper in a word it reads back as
    /// another, the day and the two days are not there, or no limit has a
    /// number of minutes.
    #[test]
    fn every_span_has_one_word() {
        let words: Vec<String> = RelaxSpan::ALL.iter().map(|s| s.word()).collect();
        assert_eq!(words, ["5", "15", "30", "60", "1440", "2880", "no-limit"]);
        for span in RelaxSpan::ALL {
            assert_eq!(RelaxSpan::of_word(&span.word()), Some(span));
        }
        assert_eq!(RelaxSpan::NoLimit.minutes(), None);
        assert_eq!(RelaxSpan::TwoDays.minutes(), Some(2880));
        // A PIN'S WORD is one key of the shape, nothing else.
        let key = format!(
            "{}{}",
            ronoctua_person::local::KEY_PREFIX,
            "A".repeat(ronoctua_person::local::KEY_BASE64_CHARS)
        );
        for wrong in [
            vec!["person-signer-pin".to_owned()],
            vec![
                "person-signer-pin".to_owned(),
                key[..key.len() - 1].to_owned(),
            ],
            vec!["person-signer-pin".to_owned(), format!("{key}\nmore")],
            vec!["person-signer-pin".to_owned(), key.clone(), key.clone()],
            vec!["person-signer-pin".to_owned(), "/etc/shadow".to_owned()],
        ] {
            assert_eq!(RootVerb::parse(&wrong), None, "{:.40?}", wrong);
        }
        assert!(RootVerb::PersonSignerPin { key }.needs_a_person());
    }

    #[test]
    fn unit_verbs_cross_too_and_refuse_what_is_not_plain() {
        let link = |up| UnitVerb::Link {
            interface: "wlp2s0".to_owned(),
            connection: "3f2a9c1e-0b7d-4e55-9a51-2c8d7e6f1a0b".to_owned(),
            up,
        };
        for verb in [
            link(true),
            link(false),
            UnitVerb::ShieldsUp,
            UnitVerb::RelaxBoot,
            UnitVerb::RelaxExpire {
                until_unix_s: 1_790_000_000,
            },
            UnitVerb::RelaxExpire { until_unix_s: 0 },
            UnitVerb::VpnInstall {
                user: "someone".to_owned(),
            },
        ] {
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
            words(&["relax-expire"]),
            words(&["relax-expire", "-1"]),
            words(&["relax-expire", "+1790000000"]),
            words(&["relax-expire", "1e9"]),
            words(&["relax-expire", "99999999999999999999"]),
            words(&["relax-expire", "1", "2"]),
            words(&["relax-boot", "now"]),
            // A VPN INSTALL'S WORD is one plain name, nothing else.
            words(&["vpn-install"]),
            words(&["vpn-install", "Odd Name"]),
            words(&["vpn-install", "a\nb"]),
            words(&["vpn-install", "someone", "more"]),
        ] {
            assert_eq!(UnitVerb::parse(&bad), None, "{bad:?}");
        }
        // AND IT IS A UNIT'S VERB, never the door's.
        assert_eq!(RootVerb::parse(&words(&["vpn-install", "someone"])), None);
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
