//! `belongings.json`: where the belongings go, how they get there, how often,
//! and what the person added or took out (feature `prefs` — the installer
//! writes it into the person's home, a door writes it through a `Store`).

use serde::{Deserialize, Serialize};

/// The file's name, under the config root.
pub const FILE: &str = "belongings.json";

/// The file's format.
pub const PREFS_FORMAT: u32 = 1;

/// How the repository is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Auth {
    /// An SSH deploy key under `belongings-keys/` (generated, `0600`).
    #[default]
    Ssh,
    /// A token kept `0600`; only its path travels (`GIT_ASKPASS`).
    Token,
}

/// The periods a person picks from, in days; `None` is never. The brain
/// reminds once the last sync is older than the one chosen.
pub const PERIODS: [Option<u16>; 6] = [Some(1), Some(3), Some(7), Some(14), Some(30), None];

/// The person's belongings settings.
///
/// `default` on the whole: a file a hand wrote with only a repository in it
/// still parses, with the rest as a fresh machine has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Belongings {
    /// The file's format ([`PREFS_FORMAT`]).
    pub format: u32,
    /// The repository: `git@host:path`, `ssh://…` or `https://…` — never with
    /// credentials in it ([`repository_is_plain`]). Empty: none yet.
    pub repository: String,
    /// How it is reached.
    pub auth: Auth,
    /// The user name an HTTPS host asks for, when it asks.
    pub user: Option<String>,
    /// Days between syncs the brain reminds of; `None` is never.
    pub period_days: Option<u16>,
    /// LOGICAL paths the person added (`home/notes`, `home/.bashrc`) —
    /// folders walked whole, links kept as links.
    pub extra: Vec<String>,
    /// Logical paths the person took out of what is kept.
    pub excluded: Vec<String>,
}

impl Belongings {
    /// Whether a repository is set.
    #[must_use]
    pub fn has_repository(&self) -> bool {
        !self.repository.trim().is_empty()
    }

    /// A fresh machine's settings with `repository` set — what the installer
    /// writes into the person's home.
    #[must_use]
    pub fn with_repository(repository: &str) -> Self {
        Self {
            repository: repository.trim().to_owned(),
            ..Self::default()
        }
    }

    /// The settings a file's text holds; `None` when it does not parse.
    #[must_use]
    pub fn from_file(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }

    /// The file's text: JSON, one field per line, a newline at the end — the
    /// shape a person reads and edits by hand.
    #[must_use]
    pub fn to_file(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    /// The period after `days`, round the list — what `RET` on the row does.
    #[must_use]
    pub fn next_period(days: Option<u16>) -> Option<u16> {
        let at = PERIODS.iter().position(|p| *p == days).unwrap_or(0);
        PERIODS[(at + 1) % PERIODS.len()]
    }

    /// Keeps `logical` (the file manager's "keep in belongings", a "taken
    /// out" row): out of what was taken out, and into the person's additions
    /// only when that alone does not keep it — a path the table keeps is never
    /// turned into "added by you", which would make it travel as an addition
    /// and let a conflict keep both sides of `~/.config/ronoctua`. Says whether
    /// anything changed.
    pub fn keep(&mut self, logical: &str) -> bool {
        let before = (self.extra.len(), self.excluded.len());
        self.excluded.retain(|e| e != logical);
        if !crate::kept::kept_here(logical, self, false) && !self.extra.iter().any(|e| e == logical)
        {
            self.extra.push(logical.to_owned());
            self.extra.sort();
        }
        (self.extra.len(), self.excluded.len()) != before
    }

    /// Stops keeping `logical`: out of the person's additions, and INTO what
    /// is taken out, always. A path this machine added and synced is in the
    /// repository as an addition every machine follows; only "taken out"
    /// makes the next sync take it out of the repository and the other
    /// machines forget it (`crate::merge`) — leaving the additions alone would
    /// keep syncing it while the page says it is not kept. Says whether
    /// anything changed.
    pub fn stop_keeping(&mut self, logical: &str) -> bool {
        let before = (self.extra.len(), self.excluded.len());
        self.extra.retain(|e| e != logical);
        if !self.excluded.iter().any(|e| e == logical) {
            self.excluded.push(logical.to_owned());
            self.excluded.sort();
        }
        (self.extra.len(), self.excluded.len()) != before
    }
}

impl Default for Belongings {
    fn default() -> Self {
        Self {
            format: PREFS_FORMAT,
            repository: String::new(),
            auth: Auth::Ssh,
            user: None,
            period_days: Some(7),
            extra: Vec::new(),
            excluded: Vec::new(),
        }
    }
}

/// Why a repository address is refused, as a closed set
/// (`inst-belongings-invalid-<id>` and the view's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Invalid {
    /// Nothing was typed.
    Empty,
    /// It carries a user and password (`user:pass@`): a credential would sit
    /// in a plain file and in git's config.
    CredentialsInAddress,
    /// `http://`: the content would cross the network in the clear (it is
    /// encrypted, but the access is not).
    PlainHttp,
    /// `file://` or a bare path: not a remote.
    NotRemote,
    /// Some other shape.
    Unknown,
}

/// Whether `address` is a repository the belongings may use: `git@host:path`,
/// `ssh://…` or `https://…`, with no credentials in it. Pure.
///
/// # Errors
///
/// An [`Invalid`].
pub fn repository_is_plain(address: &str) -> Result<(), Invalid> {
    let address = address.trim();
    if address.is_empty() {
        return Err(Invalid::Empty);
    }
    if address.chars().any(char::is_whitespace) {
        return Err(Invalid::Unknown);
    }
    let rest = if let Some(rest) = address.strip_prefix("https://") {
        rest
    } else if let Some(rest) = address.strip_prefix("ssh://") {
        rest
    } else if address.starts_with("http://") {
        return Err(Invalid::PlainHttp);
    } else if address.starts_with("file://") || address.starts_with('/') || address.starts_with('.')
    {
        return Err(Invalid::NotRemote);
    } else if let Some((user, rest)) = address.split_once('@') {
        // `git@host:path`, scp-like: a user name, never a password.
        if user.contains(':') {
            return Err(Invalid::CredentialsInAddress);
        }
        let fits = !user.is_empty()
            && !user.contains('/')
            && rest.split_once(':').is_some_and(|(host, path)| {
                !host.is_empty() && !host.contains('/') && !path.is_empty()
            });
        return if fits { Ok(()) } else { Err(Invalid::Unknown) };
    } else {
        return Err(Invalid::Unknown);
    };
    let authority = rest.split('/').next().unwrap_or_default();
    if let Some((userinfo, _)) = authority.rsplit_once('@')
        && (userinfo.contains(':') || address.starts_with("https://"))
    {
        return Err(Invalid::CredentialsInAddress);
    }
    if authority.is_empty() || rest.len() == authority.len() {
        return Err(Invalid::Unknown);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_plain_remote_is_a_repository() {
        for good in [
            "git@example.org:someone/belongings.git",
            "ssh://git@example.org/someone/belongings.git",
            "https://example.org/someone/belongings.git",
        ] {
            assert_eq!(repository_is_plain(good), Ok(()), "{good}");
        }
        assert_eq!(repository_is_plain(" "), Err(Invalid::Empty));
        assert_eq!(
            repository_is_plain("https://someone:hunter2@example.org/b.git"),
            Err(Invalid::CredentialsInAddress)
        );
        assert_eq!(
            repository_is_plain("https://token@example.org/b.git"),
            Err(Invalid::CredentialsInAddress),
            "a token as the user is a credential too"
        );
        assert_eq!(
            repository_is_plain("ssh://git:pw@example.org/b.git"),
            Err(Invalid::CredentialsInAddress)
        );
        assert_eq!(
            repository_is_plain("git:pw@example.org:b.git"),
            Err(Invalid::CredentialsInAddress)
        );
        assert_eq!(
            repository_is_plain("http://example.org/b.git"),
            Err(Invalid::PlainHttp)
        );
        assert_eq!(
            repository_is_plain("file:///srv/b.git"),
            Err(Invalid::NotRemote)
        );
        assert_eq!(repository_is_plain("/srv/b.git"), Err(Invalid::NotRemote));
        assert_eq!(
            repository_is_plain("https://example.org"),
            Err(Invalid::Unknown)
        );
        let kept: Belongings = serde_json::from_str(
            r#"{"format":1,"repository":"","auth":"token","user":null,"period_days":null,"extra":[],"excluded":[]}"#,
        )
        .unwrap();
        assert_eq!(kept.auth, Auth::Token);
        assert_eq!(kept.period_days, None, "never");
        let partial: Belongings =
            serde_json::from_str(r#"{"repository":"git@example.org:a/b.git"}"#).unwrap();
        assert_eq!(
            partial.period_days,
            Some(7),
            "a partial file takes the defaults"
        );
        assert!(partial.has_repository());
    }

    #[test]
    fn the_period_walks_round_and_keeping_is_undone_by_its_opposite() {
        let mut seen = vec![Some(7)];
        let mut p = Some(7);
        for _ in 0..PERIODS.len() {
            p = Belongings::next_period(p);
            seen.push(p);
        }
        assert_eq!(seen.first(), seen.last(), "the period does not come round");
        assert!(seen.contains(&None), "never is not on the ring");

        let mut b = Belongings::default();
        assert!(b.keep("home/notes"));
        assert!(!b.keep("home/notes"), "kept twice");
        // STOPPING an addition takes it out — what makes the next sync take it
        // out of the repository — and keeping it again puts it back.
        assert!(b.stop_keeping("home/notes"));
        assert!(b.extra.is_empty(), "{b:?}");
        assert_eq!(b.excluded, ["home/notes"]);
        assert!(!b.stop_keeping("home/notes"), "stopped twice");
        assert!(b.keep("home/notes"));
        assert_eq!(
            (b.extra.as_slice(), b.excluded.len()),
            (&["home/notes".to_owned()][..], 0)
        );
        // A DEFAULT path is taken out by excluding it, and put back by keeping
        // it — never as an addition of the person's.
        let mut b = Belongings::default();
        assert!(b.stop_keeping("config/gestures.json"));
        assert_eq!(b.excluded, ["config/gestures.json"]);
        assert!(b.keep("config/gestures.json"));
        assert!(b.excluded.is_empty() && b.extra.is_empty(), "{b:?}");
        assert!(!b.keep("config/theme.json"), "the table keeps it already");
        assert!(b.extra.is_empty(), "a default path became an addition");
        // UNDER A FOLDER TAKEN OUT, keeping one file adds that file alone.
        assert!(b.stop_keeping("config/studio"));
        assert!(b.keep("config/studio/scenes.json"));
        assert_eq!(b.extra, ["config/studio/scenes.json"]);
        assert!(crate::kept::kept_here(
            "config/studio/scenes.json",
            &b,
            false
        ));
        assert!(!crate::kept::kept_here(
            "config/studio/other.json",
            &b,
            false
        ));
    }
}
