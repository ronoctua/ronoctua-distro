//! What the questions ask the world, and what comes back.
//!
//! # Why the questions do not ask it themselves
//!
//! Three answers are checked against the network when there is one — the
//! way out, the access, the passphrase — and each takes seconds. The TUI's
//! [`State`](crate::tui::State) only says WHAT it wants checked (a [`Job`]),
//! and takes what came back ([`Done`]) when it arrives; a thread runs the job
//! (`crate::reach`). So the state stays a pure function of keys and answers,
//! the screen keeps drawing while the radio and the remote take their time,
//! and the tests answer the jobs themselves, with no network at all. The
//! terminal-less mode runs the same jobs, one after the other.
//!
//! With no network the same questions ask only what is local: the scratch
//! home and the deploy key ([`Job::Prepare`] with `online` off), and the
//! token kept where the access is read ([`Job::Keep`]). Nothing else.
//!
//! A job carries the secret it needs (the Wi-Fi password, a token, the
//! passphrase) in memory only; [`Job`]'s `Debug` never prints one.

use ronoctua_belongings::Why;

use crate::access::Probed;
use crate::gate::Gate;

/// What a question wants checked.
#[derive(Clone, PartialEq, Eq)]
pub enum Job {
    /// Join the chosen network (none: a cable, or no network at all) and
    /// read the way out.
    Network {
        /// The network chosen; empty for a cable, or none.
        ssid: String,
        /// Its password.
        password: String,
    },
    /// Make the scratch home the repository is asked from — its settings,
    /// the consent for its host — and, for an SSH address, the deploy key.
    Prepare {
        /// The address.
        address: String,
        /// Whether the repository's server may be asked: an HTTPS address's
        /// transport is probed only online, and with no network the
        /// address is taken as typed.
        online: bool,
    },
    /// Probe the access: the deploy key, or this token (HTTPS).
    Probe {
        /// The address.
        address: String,
        /// The token typed, for an HTTPS address.
        token: Option<String>,
        /// Seconds to wait first: a key that was not added yet is asked
        /// again after a while, never in a loop that spins.
        wait_s: u64,
    },
    /// Keep the token typed where the access is read, without asking the
    /// repository: with no network, the first boot tries it.
    Keep {
        /// The address.
        address: String,
        /// The token typed.
        token: String,
    },
    /// Open the repository's `key.age` with this passphrase.
    Verify {
        /// The address.
        address: String,
        /// The passphrase typed.
        passphrase: String,
    },
}

impl std::fmt::Debug for Job {
    /// The kind of job and its address — never a secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network { ssid, .. } => write!(f, "Network({ssid:?})"),
            Self::Prepare { address, online } => {
                write!(f, "Prepare({address:?}, online: {online})")
            }
            Self::Probe {
                address,
                token,
                wait_s,
            } => write!(
                f,
                "Probe({address:?}, token: {}, after {wait_s} s)",
                token.is_some()
            ),
            Self::Keep { address, .. } => write!(f, "Keep({address:?})"),
            Self::Verify { address, .. } => write!(f, "Verify({address:?})"),
        }
    }
}

/// What came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    /// The way out.
    Network(Gate),
    /// The scratch home is made; the deploy key's public half when one was
    /// made.
    Prepared(Result<Option<String>, Why>),
    /// What the access reached.
    Probed(Result<Probed, Why>),
    /// Whether the token was kept.
    Kept(Result<(), Why>),
    /// Whether the passphrase opened the repository.
    Verified(Result<(), Why>),
}

/// A job, numbered: a reply to a job the screen already left behind (the
/// person went back) is dropped by its number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Its number.
    pub serial: u64,
    /// The job.
    pub job: Job,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_never_prints_its_secret() {
        for job in [
            Job::Network {
                ssid: "home".into(),
                password: "Zq7-radio-secret".into(),
            },
            Job::Probe {
                address: "https://example.org/b.git".into(),
                token: Some("Zq7-token-secret".into()),
                wait_s: 0,
            },
            Job::Keep {
                address: "https://example.org/b.git".into(),
                token: "Zq7-token-secret".into(),
            },
            Job::Verify {
                address: "git@example.org:b.git".into(),
                passphrase: "Zq7-belongings-secret".into(),
            },
        ] {
            let shown = format!("{job:?}");
            assert!(!shown.contains("Zq7"), "{shown}");
        }
    }
}
