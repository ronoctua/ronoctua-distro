//! Where each account's last agent was opened — the folder the next one is
//! offered in (owner, 2026-11-09: a new agent is opened in a folder the
//! person chooses, and the choice starts where the last one of that account
//! was).
//!
//! `$XDG_STATE_HOME/ronoctua/llm-workspaces.json`: by the account's id, the
//! folder chosen the last time a session or a login of it was opened from
//! the accounts' view. STATE and not a preference: a folder of this machine,
//! so it never travels with the belongings, and the list of accounts
//! (`llm-accounts.json`, which does) keeps its format.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Refusal, Roots, Why};

/// The folders, by account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Workspaces {
    /// Always 1.
    pub format: u32,
    /// The last folder an agent of each account was opened in, by its id.
    pub last: BTreeMap<String, PathBuf>,
}

impl Default for Workspaces {
    fn default() -> Self {
        Self {
            format: 1,
            last: BTreeMap::new(),
        }
    }
}

impl Workspaces {
    /// Its file, in the state folder.
    pub const FILE: &'static str = "llm-workspaces.json";

    /// The store under a state root.
    #[must_use]
    pub fn store_in(state: Option<PathBuf>) -> ronoctua_store::Store<Self> {
        ronoctua_store::Store::at(state, Self::FILE)
    }

    /// The folder the last agent of account `id` was opened in — while it
    /// is still a folder: one moved or deleted since offers nothing, and the
    /// choice starts where the picker would.
    #[must_use]
    pub fn last_in(roots: &Roots, id: &str) -> Option<PathBuf> {
        Self::store_in(roots.state.clone())
            .read()
            .last
            .remove(id)
            .filter(|d| d.is_absolute() && d.is_dir())
    }

    /// Remembers `dir` as account `id`'s last, laid over what the file says
    /// NOW — the other door may have remembered another account since.
    ///
    /// # Errors
    ///
    /// The file does not read (nothing is written over it), or it could not
    /// be written.
    pub fn remember_in(roots: &Roots, id: &str, dir: &Path) -> Result<(), Refusal> {
        if !crate::fine_id(id) || !dir.is_absolute() {
            return Err(Refusal::of(Why::Invalid));
        }
        Self::keep_in(roots, |w| {
            w.last.insert(id.to_owned(), dir.to_path_buf());
        })
    }

    /// Forgets the accounts `gone` — removed from the list, so an account
    /// given the same id later starts where the picker would.
    ///
    /// # Errors
    ///
    /// As [`Self::remember_in`].
    pub fn forget_in(roots: &Roots, gone: &[&str]) -> Result<(), Refusal> {
        let store = Self::store_in(roots.state.clone());
        if !gone.iter().any(|id| store.read().last.contains_key(*id)) {
            return Ok(());
        }
        Self::keep_in(roots, |w| {
            w.last.retain(|id, _| !gone.contains(&id.as_str()))
        })
    }

    /// Applies `edit` to what the file says now, and writes it back whole.
    fn keep_in(roots: &Roots, edit: impl FnOnce(&mut Self)) -> Result<(), Refusal> {
        let store = Self::store_in(roots.state.clone());
        if store.path().is_none() {
            return Err(Refusal::of(Why::Failed));
        }
        // `parsed` and never `read`: this writes the whole file back.
        let mut now = store.parsed().ok_or(Refusal::of(Why::Unreadable))?;
        edit(&mut now);
        store.write(&now).map_err(|_| Refusal::of(Why::Failed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(tag: &str) -> (Roots, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-llm-workspaces-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("work")).unwrap();
        (
            Roots {
                state: Some(dir.join("state")),
                ..Roots::default()
            },
            dir,
        )
    }

    /// FAILS IF: a folder remembered for one account is not offered for it,
    /// is offered for another, survives the account's removal, or one that is
    /// not a folder any more is offered at all.
    #[test]
    fn the_last_folder_is_an_accounts_own_while_it_is_a_folder() {
        let (roots, dir) = roots("own");
        let work = dir.join("work");
        assert_eq!(Workspaces::last_in(&roots, "claude-1"), None);
        Workspaces::remember_in(&roots, "claude-1", &work).unwrap();
        assert_eq!(Workspaces::last_in(&roots, "claude-1"), Some(work));
        assert_eq!(Workspaces::last_in(&roots, "codex-1"), None);
        Workspaces::remember_in(&roots, "codex-1", &dir).unwrap();
        Workspaces::forget_in(&roots, &["claude-1"]).unwrap();
        assert_eq!(Workspaces::last_in(&roots, "claude-1"), None);
        assert_eq!(Workspaces::last_in(&roots, "codex-1"), Some(dir.clone()));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(Workspaces::last_in(&roots, "codex-1"), None);
    }

    /// FAILS IF: a relative folder or an id that is no account's is kept.
    #[test]
    fn only_a_whole_folder_of_an_account_is_kept() {
        let (roots, dir) = roots("refused");
        assert!(Workspaces::remember_in(&roots, "claude-1", Path::new("work")).is_err());
        assert!(Workspaces::remember_in(&roots, "../x", &dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
