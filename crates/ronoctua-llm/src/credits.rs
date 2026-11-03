//! An API key's credits — the one word anybody can have on them.
//!
//! # Why so little
//!
//! A Claude Code account on an API key ([`Kind::ClaudeApi`]) is paid from the
//! Console's prepaid credits, and Anthropic has NO interface that says the
//! balance — not to the key, not to Claude Code. What is known is the moment
//! Claude Code says they ran out: an answer that fails with `billing_error`.
//! So that is all this keeps: since when the account is out of credits, or
//! that it is not ([`out_since`]). The tokens it spends are counted from its
//! transcripts, as a subscription's are ([`crate::tokens`]).
//!
//! # Kept whole, by whoever heard it
//!
//! `$XDG_STATE_HOME/ronoctua/llm/<id>/credits.json`, `{"format": 1,
//! "out-since-s": <seconds since the epoch> | null}`, written whole and
//! atomically by [`mark_out`] and [`clear`] — what Claude Code's managed hook
//! calls when an answer fails for want of credits, and when one goes through
//! again, holding the sessions' lock so two sessions of one account never
//! lose a word between a read and a write ([`crate::hook`]) — and read by the
//! accounts' view and the brain's marks, which watch it through
//! [`crate::watched`]. A subscription has windows instead, and is
//! refused here ([`Why::Invalid`]): nothing may say "out of credits" of an
//! account whose limits say something else.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Account, Kind, Refusal, Why};

/// The file's name, in the account's state folder.
const KEPT_FILE: &str = "credits.json";

/// The format written, and the only one read.
const FORMAT: u32 = 1;

/// The most the file may weigh: two numbers.
const MOST_BYTES: u64 = 4 * 1024;

/// What is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Kept {
    /// Always [`FORMAT`].
    format: u32,
    /// Since when Claude Code has said the credits are out, in seconds since
    /// the epoch; `None` when it has not, or an answer went through since.
    out_since_s: Option<i64>,
}

/// Where an account's word on its credits is kept, under a state root — an
/// API key's account's only.
pub(crate) fn kept_path(state: &Path, account: &Account) -> Option<PathBuf> {
    (account.kind == Kind::ClaudeApi)
        .then(|| crate::id_folder(state, &account.id))
        .flatten()
        .map(|d| d.join(KEPT_FILE))
}

/// Since when `account` is out of credits, as Claude Code last said
/// (seconds since the epoch) — `None` when it is not, or nothing was ever
/// said. Always `None` for an account that has no credits to run out of
/// (a subscription's, Codex's).
///
/// # Errors
///
/// [`Why::Unreadable`] when the kept file does not read: nothing is guessed,
/// least of all that the credits are fine.
pub fn out_since(account: &Account) -> Result<Option<i64>, Refusal> {
    match ronoctua_store::state_dir() {
        Some(state) => out_since_in(&state, account),
        None => Ok(None),
    }
}

/// [`out_since`], under a given state root.
pub(crate) fn out_since_in(state: &Path, account: &Account) -> Result<Option<i64>, Refusal> {
    let Some(path) = kept_path(state, account) else {
        return Ok(None);
    };
    match crate::files::read_bounded(&path, MOST_BYTES) {
        Ok(bytes) => serde_json::from_slice::<Kept>(&bytes)
            .ok()
            .filter(|k| k.format == FORMAT)
            .map(|k| k.out_since_s)
            .ok_or(Refusal::of(Why::Unreadable)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Refusal::of(Why::Unreadable)),
    }
}

/// Keeps that `account` is out of credits, from `now_s` — or from the
/// moment it was first said, when it already was: "out since" is when they
/// ran out, and every answer that fails after it says the same thing again.
///
/// Only under the sessions' lock (`held`), as every word on the credits is
/// told: this reads the file and writes it back, and two writers between
/// would lose one word ([`crate::hook`]).
///
/// # Errors
///
/// [`Why::Invalid`] for an account that is not an API key's;
/// [`Why::Failed`] when there is no state folder or it could not be written.
pub fn mark_out(
    _held: &crate::sessions::Lock,
    account: &Account,
    now_s: i64,
) -> Result<(), Refusal> {
    let state = ronoctua_store::state_dir().ok_or(Refusal::of(Why::Failed))?;
    mark_out_in(&state, account, now_s)
}

/// [`mark_out`], under a given state root.
pub(crate) fn mark_out_in(state: &Path, account: &Account, now_s: i64) -> Result<(), Refusal> {
    // A FILE THAT DOES NOT READ is ours, and replaced: what it said is lost
    // either way, and this is what is true now.
    let since = out_since_in(state, account).ok().flatten().unwrap_or(now_s);
    keep_in(state, account, Some(since))
}

/// Keeps that `account` is not out of credits (an answer went through).
/// Nothing is written when that is already what the file says — a watcher
/// would wake for nothing after every answer. Under the sessions' lock, as
/// [`mark_out`].
///
/// # Errors
///
/// As [`mark_out`].
pub fn clear(_held: &crate::sessions::Lock, account: &Account) -> Result<(), Refusal> {
    let state = ronoctua_store::state_dir().ok_or(Refusal::of(Why::Failed))?;
    clear_in(&state, account)
}

/// [`clear`], under a given state root.
pub(crate) fn clear_in(state: &Path, account: &Account) -> Result<(), Refusal> {
    if kept_path(state, account).is_some() && out_since_in(state, account) == Ok(None) {
        return Ok(());
    }
    keep_in(state, account, None)
}

/// Writes the file whole and atomically.
fn keep_in(state: &Path, account: &Account, out_since_s: Option<i64>) -> Result<(), Refusal> {
    if account.kind != Kind::ClaudeApi {
        return Err(Refusal::of(Why::Invalid));
    }
    let path = kept_path(state, account).ok_or(Refusal::of(Why::Unknown))?;
    let kept = Kept {
        format: FORMAT,
        out_since_s,
    };
    let bytes = serde_json::to_vec_pretty(&kept).map_err(|_| Refusal::of(Why::Failed))?;
    crate::files::write_keeping(&path, &bytes, 0o644).map_err(|_| Refusal::of(Why::Failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-llm-credits-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn account(kind: Kind) -> Account {
        Account {
            id: format!("{}-1", kind.id()),
            kind,
            name: "Work".to_owned(),
            dir: "/home/p/.claude-api".into(),
            default: true,
        }
    }

    /// FAILS IF: running out is not kept, "since" moves with every failed
    /// answer after the first, an answer that goes through does not clear
    /// it, or the file is not the one `watched` names — the view and the
    /// marks would wake for nothing, or never.
    #[test]
    fn running_out_is_kept_from_the_first_moment_and_cleared() {
        let dir = scratch("round");
        let api = account(Kind::ClaudeApi);
        assert_eq!(out_since_in(&dir, &api), Ok(None), "nothing said yet");
        mark_out_in(&dir, &api, 1_000).unwrap();
        mark_out_in(&dir, &api, 2_000).unwrap();
        assert_eq!(out_since_in(&dir, &api), Ok(Some(1_000)));
        let path = kept_path(&dir, &api).unwrap();
        assert!(path.ends_with("llm/claude-api-1/credits.json"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "{\n  \"format\": 1,\n  \"out-since-s\": 1000\n}"
        );
        clear_in(&dir, &api).unwrap();
        assert_eq!(out_since_in(&dir, &api), Ok(None));
        // Cleared again: the file is not written — every write is a new
        // file renamed over it — and a watcher hears nothing.
        let inode = |p: &Path| std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(p).unwrap());
        let before = inode(&path);
        clear_in(&dir, &api).unwrap();
        assert_eq!(inode(&path), before, "written again for nothing");
        assert_eq!(
            crate::watched_in(
                &crate::Roots {
                    state: Some(dir.clone()),
                    ..crate::Roots::default()
                },
                None,
                &api
            )
            .files,
            [path]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a subscription's or Codex's account can be said to be out
    /// of credits, a file that does not read is taken for credits that are
    /// fine, or a field the format does not know is read past.
    #[test]
    fn only_an_api_key_runs_out_and_a_strange_file_is_not_fine() {
        let dir = scratch("refused");
        for kind in [Kind::Claude, Kind::Codex] {
            let other = account(kind);
            assert_eq!(mark_out_in(&dir, &other, 1).unwrap_err().why, Why::Invalid);
            assert_eq!(clear_in(&dir, &other).unwrap_err().why, Why::Invalid);
            assert_eq!(out_since_in(&dir, &other), Ok(None));
        }
        let api = account(Kind::ClaudeApi);
        let path = kept_path(&dir, &api).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for strange in [
            "{ broken",
            r#"{"format":2,"out-since-s":null}"#,
            r#"{"format":1,"out-since-s":null,"balance":3}"#,
        ] {
            std::fs::write(&path, strange).unwrap();
            assert_eq!(
                out_since_in(&dir, &api).unwrap_err().why,
                Why::Unreadable,
                "{strange}"
            );
        }
        // Running out is said over it all the same: it is what is true now.
        mark_out_in(&dir, &api, 5).unwrap();
        assert_eq!(out_since_in(&dir, &api), Ok(Some(5)));
        let _ = std::fs::remove_dir_all(dir);
    }
}
