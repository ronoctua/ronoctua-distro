//! Where every file of the belongings lives — and what a LOGICAL path is.
//!
//! # Logical paths
//!
//! A path kept is written `<root>/<rest>`: `config/theme.json`,
//! `state/software/history.jsonl`, `home/notes/todo.md`. The root is resolved
//! on each machine against its own directories ([`Places`]), so a machine
//! whose `$XDG_CONFIG_HOME` is elsewhere — or the `.#dev` loop, whose roots are
//! `ronoctua-dev` — restores into ITS roots, and the repository never carries
//! an absolute path of anybody's.
//!
//! A file under the configuration root has two names (`config/theme.json` and
//! `home/.config/ronoctua/theme.json`); [`Places::logical_of`] always gives the
//! first, and a walk of a folder the person added skips the roots inside it —
//! they are walked as themselves.
//!
//! # The belongings' own files
//!
//! ```text
//! config/belongings.json              what the person chose (repository, access, period, extras)
//! config/belongings-keys/deploy       the SSH deploy key (0600), and deploy.pub
//! config/belongings-keys/token        an HTTPS token (0600)
//! config/belongings-keys/known_hosts  our own known hosts
//! state/belongings/last.json          what the last sync left: the base of the next merge
//! state/belongings/status.json        how the last run ended — what the door and the brain read
//! state/belongings/run.lock           held by the one run of this machine (`crate::running`)
//! state/belongings/repo/              the clone git transports through
//! state/belongings/replaced/<stamp>/  what a sync overwrote here (the last three)
//! $XDG_RUNTIME_DIR/ronoctua-belongings/passphrase       0600, written by a door, deleted after
//! $XDG_RUNTIME_DIR/ronoctua-belongings/new-passphrase   the same, for a change of passphrase
//! $XDG_RUNTIME_DIR/ronoctua-belongings/decisions.json   0600, what the person decided, by a door
//! $XDG_RUNTIME_DIR/ronoctua-belongings/session/         0700: plan.json, theirs/
//! ```
//!
//! # Why the decisions sit beside the session and not in it
//!
//! Because two doors write them and each redraws what the other wrote, and a
//! door hears a folder's own names only (inotify is not recursive): the
//! runtime folder is one both doors watch, from its birth to the end of the
//! session. The CLI deletes the file with the session.

use std::path::{Path, PathBuf};

/// A root a logical path is written against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Root {
    /// `$XDG_CONFIG_HOME/ronoctua`.
    Config,
    /// `$XDG_STATE_HOME/ronoctua`.
    State,
    /// The home.
    Home,
}

impl Root {
    /// The three, the most specific first: a path under the configuration
    /// root is named by it, never by the home.
    pub const ALL: [Self; 3] = [Self::Config, Self::State, Self::Home];

    /// Its word, the first component of a logical path.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::State => "state",
            Self::Home => "home",
        }
    }

    /// The root a logical path begins with, and the rest.
    #[must_use]
    pub fn split(logical: &str) -> Option<(Self, &str)> {
        let (word, rest) = logical.split_once('/').unwrap_or((logical, ""));
        let root = Self::ALL.into_iter().find(|r| r.word() == word)?;
        Some((root, rest))
    }
}

/// Whether `logical` is a logical path: a root, then components that neither
/// climb nor are empty. Pure.
#[must_use]
pub fn is_logical(logical: &str) -> bool {
    let Some((_, rest)) = Root::split(logical) else {
        return false;
    };
    // `config` alone is a root; `config/` names nothing under it.
    if rest.is_empty() {
        return !logical.contains('/');
    }
    rest.split('/')
        .all(|c| !c.is_empty() && c != "." && c != ".." && !c.contains('\0'))
}

/// Whether `path` is `root` or lies under it — by component, never by prefix
/// (`config/theme` does not cover `config/themes.json`). Pure.
#[must_use]
pub fn covers(root: &str, path: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The leaf under the runtime directory the belongings' secrets and session
/// live in (`$XDG_RUNTIME_DIR/ronoctua[-dev]-belongings`).
pub const RUNTIME_LEAF: &str = "belongings";
/// The folder under the state root.
pub const OWN: &str = "belongings";
/// The folder of the access files under the configuration root.
pub const KEYS: &str = "belongings-keys";
/// What the last sync left (the base of the next merge).
pub const LAST_FILE: &str = "last.json";
/// How the last run ended.
pub const STATUS_FILE: &str = "status.json";
/// The clone.
pub const REPO_DIR: &str = "repo";
/// The lock one run at a time holds (`crate::running`).
pub const LOCK_FILE: &str = "run.lock";
/// What a sync overwrote here.
pub const REPLACED_DIR: &str = "replaced";
/// The passphrase a door wrote.
pub const PASSPHRASE_FILE: &str = "passphrase";
/// The new passphrase, for a change of passphrase.
pub const NEW_PASSPHRASE_FILE: &str = "new-passphrase";
/// The session: the plan, the decisions and the repository's side of each
/// conflicting text.
pub const SESSION_DIR: &str = "session";
/// Inside the session: the repository's side of each conflicting text.
pub const THEIRS_DIR: &str = "theirs";
/// The SSH deploy key, inside [`KEYS`].
pub const DEPLOY_KEY: &str = "deploy";
/// Our own known hosts, inside [`KEYS`].
pub const KNOWN_HOSTS: &str = "known_hosts";
/// The HTTPS token, inside [`KEYS`].
pub const TOKEN_FILE: &str = "token";
/// The merge bases kept sealed, under the belongings' own folder
/// (`state/belongings/base/<sha256>.age`).
pub const BASE_DIR: &str = "base";
/// What a door asks of the next sync, under the belongings' own folder.
pub const REQUESTS_FILE: &str = "requests.json";
/// What the index says of the person's machines (which runs the newest
/// distro), under the belongings' own folder.
pub const FLEET_FILE: &str = "fleet.json";
/// The marker a door reads while the identity is kept for the session,
/// under the runtime folder.
pub const UNLOCKED_FILE: &str = "unlocked";
/// The lenient stores' repair ledger, under the state root
/// (`state/repairs/<escaped logical path>.json`) — the store's own name for
/// it, so the two can never disagree.
pub const REPAIRS_DIR: &str = ronoctua_store::repair::DIR;
// ── anchor: L07 ──
// ── anchor: L08 ──
/// The person's post-quantum recipient (`age1pq1…`, public) the last sync
/// left, under the belongings' own folder (`crate::fleet::recipient`).
pub const RECIPIENT_FILE: &str = "recipient";
// ── anchor: L09 ──
/// Under the runtime folder: where a merge's three sides are written for
/// `git merge-file` (`0700`, emptied after each merge).
pub const MERGE_DIR: &str = "merge";
/// Under the runtime folder: the scratch repository a probe of an address
/// pushes nothing from (`push --dry-run`).
pub const PROBE_DIR: &str = "probe";
// ── anchor: L10 ──

/// Where everything is, on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// The home.
    pub home: PathBuf,
    /// `$XDG_CONFIG_HOME/ronoctua`.
    pub config: PathBuf,
    /// `$XDG_STATE_HOME/ronoctua`.
    pub state: PathBuf,
    /// `$XDG_RUNTIME_DIR/ronoctua-belongings`.
    pub runtime: PathBuf,
}

impl Places {
    /// The places of this process, from its environment; `None` without a
    /// home, a configuration root or a runtime directory.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        Some(Self {
            home: std::env::var_os("HOME")
                .filter(|h| !h.is_empty())
                .map(PathBuf::from)?,
            config: ronoctua_xdg::config_dir()?,
            state: ronoctua_xdg::state_dir()?,
            runtime: ronoctua_xdg::runtime_file(RUNTIME_LEAF)?,
        })
    }

    /// The places under one folder — what a test hands in.
    #[must_use]
    pub fn under(root: &Path) -> Self {
        Self {
            home: root.join("home"),
            config: root.join("home/.config/ronoctua"),
            state: root.join("home/.local/state/ronoctua"),
            runtime: root.join("run/ronoctua-belongings"),
        }
    }

    /// The directory a root is.
    #[must_use]
    pub fn root(&self, root: Root) -> &Path {
        match root {
            Root::Config => &self.config,
            Root::State => &self.state,
            Root::Home => &self.home,
        }
    }

    /// Where a logical path is on this machine; `None` for one that is not.
    #[must_use]
    pub fn resolve(&self, logical: &str) -> Option<PathBuf> {
        if !is_logical(logical) {
            return None;
        }
        let (root, rest) = Root::split(logical)?;
        let base = self.root(root);
        Some(if rest.is_empty() {
            base.to_path_buf()
        } else {
            base.join(rest)
        })
    }

    /// The logical name of an absolute path: under the configuration or the
    /// state root first, then the home. `None` outside all three, or for a
    /// name that is not text.
    #[must_use]
    pub fn logical_of(&self, path: &Path) -> Option<String> {
        for root in Root::ALL {
            if let Ok(rest) = path.strip_prefix(self.root(root)) {
                let rest = rest.to_str()?;
                let logical = if rest.is_empty() {
                    root.word().to_owned()
                } else {
                    format!("{}/{rest}", root.word())
                };
                return is_logical(&logical).then_some(logical);
            }
        }
        None
    }

    /// `config/belongings.json`.
    #[must_use]
    pub fn prefs(&self) -> PathBuf {
        self.config.join(crate::prefs::FILE)
    }

    /// The access files' folder.
    #[must_use]
    pub fn keys(&self) -> PathBuf {
        self.config.join(KEYS)
    }

    /// The SSH deploy key.
    #[must_use]
    pub fn deploy_key(&self) -> PathBuf {
        self.keys().join(DEPLOY_KEY)
    }

    /// Its public half.
    #[must_use]
    pub fn deploy_public(&self) -> PathBuf {
        self.keys().join(format!("{DEPLOY_KEY}.pub"))
    }

    /// Our known hosts.
    #[must_use]
    pub fn known_hosts(&self) -> PathBuf {
        self.keys().join(KNOWN_HOSTS)
    }

    /// The HTTPS token.
    #[must_use]
    pub fn token(&self) -> PathBuf {
        self.keys().join(TOKEN_FILE)
    }

    /// The belongings' own folder under the state root.
    #[must_use]
    pub fn own(&self) -> PathBuf {
        self.state.join(OWN)
    }

    /// `last.json`.
    #[must_use]
    pub fn last(&self) -> PathBuf {
        self.own().join(LAST_FILE)
    }

    /// `status.json`.
    #[must_use]
    pub fn status(&self) -> PathBuf {
        self.own().join(STATUS_FILE)
    }

    /// The clone.
    #[must_use]
    pub fn repo(&self) -> PathBuf {
        self.own().join(REPO_DIR)
    }

    /// The lock one run at a time holds.
    #[must_use]
    pub fn lock(&self) -> PathBuf {
        self.own().join(LOCK_FILE)
    }

    /// What a sync overwrote here.
    #[must_use]
    pub fn replaced(&self) -> PathBuf {
        self.own().join(REPLACED_DIR)
    }

    /// The passphrase a door wrote.
    #[must_use]
    pub fn passphrase(&self) -> PathBuf {
        self.runtime.join(PASSPHRASE_FILE)
    }

    /// The new passphrase, for a change of passphrase.
    #[must_use]
    pub fn new_passphrase(&self) -> PathBuf {
        self.runtime.join(NEW_PASSPHRASE_FILE)
    }

    /// The session folder.
    #[must_use]
    pub fn session(&self) -> PathBuf {
        self.runtime.join(SESSION_DIR)
    }

    /// The plan.
    #[must_use]
    pub fn plan(&self) -> PathBuf {
        self.session().join(crate::plan::PLAN_FILE)
    }

    /// The decisions a door wrote — beside the session, where both doors
    /// hear them (see this module's header).
    #[must_use]
    pub fn decisions(&self) -> PathBuf {
        self.runtime.join(crate::plan::DECISIONS_FILE)
    }

    /// The repository's side of the conflicting text number `n`.
    #[must_use]
    pub fn theirs(&self, name: &str) -> PathBuf {
        self.session().join(THEIRS_DIR).join(name)
    }

    /// The sealed merge bases.
    #[must_use]
    pub fn base_cache(&self) -> PathBuf {
        self.own().join(BASE_DIR)
    }

    /// What a door asks of the next sync.
    #[must_use]
    pub fn requests(&self) -> PathBuf {
        self.own().join(REQUESTS_FILE)
    }

    /// What the index says of the person's machines.
    #[must_use]
    pub fn fleet(&self) -> PathBuf {
        self.own().join(FLEET_FILE)
    }

    /// The marker of an identity kept for the session.
    #[must_use]
    pub fn unlocked(&self) -> PathBuf {
        self.runtime.join(UNLOCKED_FILE)
    }

    /// The lenient stores' repair ledger.
    #[must_use]
    pub fn repairs(&self) -> PathBuf {
        self.state.join(REPAIRS_DIR)
    }

    // ── anchor: L07 ──
    /// The person's files the repair ledger lists and nobody answered yet,
    /// by logical path — those whose file is still the one the ledger
    /// judged (a file that moved since is read again before it is listed).
    #[must_use]
    pub fn repaired(&self) -> Vec<String> {
        ronoctua_store::repair::pending(&self.repairs())
            .into_iter()
            .filter(|l| {
                self.resolve(&l.path)
                    .and_then(|p| std::fs::read(p).ok())
                    .is_some_and(|b| ronoctua_store::repair::sha256_hex(&b) == l.file_sha256)
            })
            .map(|l| l.path)
            .collect()
    }

    // ── anchor: L08 ──
    /// The person's post-quantum recipient, public.
    #[must_use]
    pub fn recipient(&self) -> PathBuf {
        self.own().join(RECIPIENT_FILE)
    }

    // ── anchor: L09 ──
    /// The manifest the person signed of this machine's plain part and its
    /// signature (`state/person-signed/`, beside the mirror and never in
    /// it): what a staging carries to root, and what a push carries when it
    /// covers what goes ([`crate::plain::SIGNED_DIR`]).
    #[must_use]
    pub fn signed(&self) -> PathBuf {
        self.state.join(ronoctua_person::names::SIGNED)
    }

    /// The identity and the signing key, encrypted to the passphrase, in
    /// the repository (`engine::KEY_FILE`).
    pub const KEY_AGE: &'static str = "key.age";

    /// The clone's `key.age` ([`Self::KEY_AGE`]): there once this machine
    /// synced with its repository — what a gesture that signs asks before it
    /// asks for the passphrase, since with no clone there is nothing to open.
    #[must_use]
    pub fn key_age(&self) -> PathBuf {
        self.repo().join(Self::KEY_AGE)
    }

    /// The places of a home that is not this process's — the installer's
    /// restore into `/mnt/home/<user>` before anybody logged in there: its
    /// roots as a session of this instance would name them, and `runtime`
    /// (a `0700` folder the installer made) for the passphrase and the
    /// session.
    #[must_use]
    pub fn for_home(home: &Path, runtime: &Path) -> Self {
        let name = ronoctua_xdg::name();
        Self {
            home: home.to_path_buf(),
            config: home.join(".config").join(&name),
            state: home.join(".local").join("state").join(&name),
            runtime: runtime.to_path_buf(),
        }
    }

    /// Where a merge's three sides are written.
    #[must_use]
    pub fn merging(&self) -> PathBuf {
        self.runtime.join(MERGE_DIR)
    }

    /// The scratch repository of a probe.
    #[must_use]
    pub fn probing(&self) -> PathBuf {
        self.runtime.join(PROBE_DIR)
    }

    // ── anchor: L10 ──
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_logical_path_names_one_root_and_never_climbs() {
        for good in [
            "config",
            "config/theme.json",
            "state/software/history.jsonl",
            "home/notes/todo.md",
            "home/.bashrc",
        ] {
            assert!(is_logical(good), "{good}");
        }
        for bad in [
            "",
            "etc/shadow",
            "/config/x",
            "config//x",
            "config/../x",
            "home/./x",
            "config/",
        ] {
            assert!(!is_logical(bad), "{bad}");
        }
        assert!(covers("config", "config/theme.json"));
        assert!(covers("config/theme.json", "config/theme.json"));
        assert!(!covers("config/theme", "config/themes.json"));
        assert!(!covers("config/theme.json", "config"));
    }

    #[test]
    fn a_path_is_named_by_its_most_specific_root_and_resolves_back() {
        let p = Places::under(Path::new("/x"));
        assert_eq!(
            p.logical_of(Path::new("/x/home/.config/ronoctua/theme.json"))
                .as_deref(),
            Some("config/theme.json"),
            "the configuration root is named as itself, never as home/.config"
        );
        assert_eq!(
            p.logical_of(Path::new("/x/home/notes/a.md")).as_deref(),
            Some("home/notes/a.md")
        );
        assert_eq!(p.logical_of(Path::new("/etc/passwd")), None);
        for logical in ["config/theme.json", "state/a/b", "home/notes/a.md"] {
            let there = p.resolve(logical).unwrap();
            assert_eq!(p.logical_of(&there).as_deref(), Some(logical));
        }
        assert_eq!(p.resolve("home/../../etc/shadow"), None);
    }

    /// FAILS IF: the installer's restore writes another home's files where a
    /// session of the new home would not look for them.
    #[test]
    fn a_home_that_is_not_ours_has_the_roots_a_session_there_reads() {
        let p = Places::for_home(Path::new("/mnt/home/someone"), Path::new("/run/x"));
        let name = ronoctua_xdg::name();
        assert_eq!(p.config, Path::new("/mnt/home/someone/.config").join(&name));
        assert_eq!(
            p.state,
            Path::new("/mnt/home/someone/.local/state").join(&name)
        );
        assert_eq!(p.passphrase(), Path::new("/run/x/passphrase"));
        assert_eq!(
            p.resolve("state/person/options.json").unwrap(),
            p.state.join("person/options.json")
        );
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Mode {
        #[default]
        Calm,
        Busy,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Prefs {
        mode: Mode,
    }

    /// FAILS IF: the files a sync says are waiting for "write the fix" or
    /// "leave it" include one the person already answered ("leave it"), one
    /// whose file changed since its ledger was written (stale: a read makes
    /// a fresh one), or one whose file is gone — or leave out one still
    /// waiting, or name it by anything but its logical path.
    #[test]
    fn only_the_ledgers_still_waiting_and_still_true_are_repaired() {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-belongings-places-repaired-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let p = Places::under(&root);
        std::fs::create_dir_all(&p.config).unwrap();
        let store = |file: &'static str| {
            ronoctua_store::Store::<Prefs>::at(Some(p.config.clone()), file)
                .with_roots(Some(p.config.clone()), Some(p.state.clone()))
        };
        for file in ["waiting.json", "left.json", "moved.json", "gone.json"] {
            std::fs::write(p.config.join(file), br#"{"mode": "frantic"}"#).unwrap();
            let _ = store(file).read();
        }
        assert_eq!(
            ronoctua_store::repair::all(&p.repairs()).len(),
            4,
            "the stores wrote no ledgers where the places look"
        );
        // "LEAVE IT", about these bytes.
        let left = store("left.json").ledger().unwrap();
        ronoctua_store::repair::dismiss(&p.repairs(), &p.config.join("left.json"), &left).unwrap();
        // CHANGED BY HAND since its ledger, and not read again.
        std::fs::write(p.config.join("moved.json"), br#"{"mode": "later"}"#).unwrap();
        // GONE.
        std::fs::remove_file(p.config.join("gone.json")).unwrap();
        assert_eq!(p.repaired(), ["config/waiting.json"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
