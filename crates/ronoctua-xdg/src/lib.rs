//! The two directories this system keeps a person's files under — and the one
//! way it writes into them.
//!
//! # Why a crate for sixteen lines
//!
//! Because they were written three times. `ronoctua-notify`, `ronoctua-brain`
//! and `ronoctua-files` each carried their own `config_dir()` and
//! `state_dir()`, and each said why: the twin lived in a crate that would have
//! dragged zbus, or an SVG rasteriser, or a file manager into a daemon whose
//! job was to decide what time it is. The brain's copy ended with the rule —
//! *"if a third caller appears, the answer is a crate of its own for the XDG
//! roots — not a dependency on whichever one happened to have them first"* —
//! and the theme was the fourth caller. This is that crate.
//!
//! # Two lifetimes, two directories
//!
//! CONFIGURATION is what the person chose — filters, the sidebar, the brain's
//! preferences, the theme. Hand-editable, small, theirs. It lives under
//! `$XDG_CONFIG_HOME/ronoctua`.
//!
//! STATE is what happened — the notification history, the brain's journal.
//! Machine-written, growing, worthless to hand-edit. It lives under
//! `$XDG_STATE_HOME/ronoctua`.
//!
//! Both are in `nix/modules/persist.nix`, so an impermanent root does not
//! take them.
//!
//! # The roots are PARAMETERS
//!
//! Every reader and writer in this system takes the directory as an argument
//! (each store keeps its own `Where` pair of `Option<PathBuf>`s, so a test
//! can hand it `None`); `config_dir()` and `state_dir()` are what an
//! APPLICATION calls once, at the top, before handing the answer down. The reason is written where it was
//! paid for (`ronoctua_files::sidebar::store`): functions that found
//! `$XDG_CONFIG_HOME` on their own let unit tests write into the real
//! configuration of whoever ran `cargo test`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::path::{Path, PathBuf};

/// The subdirectory under both roots.
pub const NAME: &str = "ronoctua";

/// `$XDG_CONFIG_HOME/ronoctua`, or `~/.config/ronoctua`; `None` with neither
/// variable set, which is not a home this system knows how to write into.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join(NAME))
}

/// `$XDG_STATE_HOME/ronoctua`, or `~/.local/state/ronoctua`. See [`config_dir`].
#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state"))
        })?;
    Some(base.join(NAME))
}

/// Writes `bytes` to `dir/file`, whole and renamed over.
///
/// Written to `<file>.new` and renamed, so a full disk or a kill mid-write
/// leaves the previous file rather than half of this one — and so a watcher on
/// the directory sees ONE event (`MOVED_TO`) with the finished content, never
/// a file that is still being written. The directory is created if it is not
/// there.
///
/// # Errors
///
/// The directory cannot be created, or the file cannot be written or renamed.
pub fn write_atomically(dir: &Path, file: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let scratch = dir.join(format!("{file}.new"));
    std::fs::write(&scratch, bytes)?;
    std::fs::rename(&scratch, dir.join(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-xdg-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn an_atomic_write_leaves_the_file_and_no_scratch() {
        let dir = scratch("atomic");
        write_atomically(&dir, "theme.json", b"{\"theme\":\"owl\"}").unwrap();
        assert_eq!(
            std::fs::read(dir.join("theme.json")).unwrap(),
            b"{\"theme\":\"owl\"}"
        );
        assert!(!dir.join("theme.json.new").exists());
        write_atomically(&dir, "theme.json", b"{\"theme\":\"forest\"}").unwrap();
        assert_eq!(
            std::fs::read(dir.join("theme.json")).unwrap(),
            b"{\"theme\":\"forest\"}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_roots_end_in_the_systems_name() {
        // Whatever the environment, the tail is ours; the head is the person's.
        for dir in [config_dir(), state_dir()].into_iter().flatten() {
            assert_eq!(dir.file_name().unwrap(), NAME);
        }
    }
}
