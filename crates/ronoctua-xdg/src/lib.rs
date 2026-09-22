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
//! # Four lifetimes, four directories
//!
//! CONFIGURATION is what the person chose — filters, the sidebar, the brain's
//! preferences, the theme. Hand-editable, small, theirs. It lives under
//! `$XDG_CONFIG_HOME/ronoctua`.
//!
//! STATE is what happened — the notification history, the brain's journal.
//! Machine-written, growing, worthless to hand-edit. It lives under
//! `$XDG_STATE_HOME/ronoctua`.
//!
//! DATA is what the person DOWNLOADED — a region's streets. Big, slow to get
//! back, and not the machine's to throw away. `$XDG_DATA_HOME/ronoctua`.
//!
//! A CACHE is what can be deleted and made again from something that is still
//! there: a decoded thumbnail of a file that is still on disk.
//! `$XDG_CACHE_HOME/ronoctua`, and it is the one root a cleaning tool may
//! empty without asking.
//!
//! The first three are in `nix/modules/persist.nix`, so an impermanent root
//! does not take them. The fourth deliberately is NOT: a cache that survived a
//! reboot of a machine whose whole root does not is a cache that has outlived
//! its own argument.
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

/// `$XDG_DATA_HOME/ronoctua`, or `~/.local/share/ronoctua`. See [`config_dir`].
///
/// # Why a third root, and what belongs in it
///
/// State is what this system writes about itself and can lose without anybody
/// minding — a history, a cache of the weather, the last notification. The
/// third root is for what a person DOWNLOADED: a region's streets is half a
/// gigabyte that took an hour on somebody's connection, and a tool that swept
/// it away with the state would be a tool that made that hour worthless. The
/// XDG specification draws the line in the same place, which is why this is
/// its own variable and not a subdirectory of the other.
#[must_use]
pub fn data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
        })?;
    Some(base.join(NAME))
}

/// `$XDG_CACHE_HOME/ronoctua`, or `~/.cache/ronoctua`. See [`config_dir`].
///
/// # Why a fourth root, and what belongs in it
///
/// Nothing in this tree had one until a thumbnail wanted one, and the reason is
/// the definition: a cache is what can be DELETED and rebuilt from something
/// else that still exists. State can be lost without anybody minding, but it
/// cannot be rebuilt — the last notification is gone once it is gone. A
/// decoded picture of a file can always be decoded again from the file.
///
/// The practical difference is who may sweep it. Everything under here is fair
/// game for a disk-cleaning tool, for a full disk, and for a person with
/// `rm -rf`; nothing under here may be the only copy of anything.
#[must_use]
pub fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join(NAME))
}

/// `$XDG_CACHE_HOME`, or `~/.cache` — the root itself, with no name of ours on
/// it.
///
/// # Why this one is not under `ronoctua/`
///
/// Because it is where SHARED layouts live, and a shared layout is not ours to
/// namespace: the freedesktop Thumbnail Managing Standard says
/// `$XDG_CACHE_HOME/thumbnails/`, and every file manager on the machine reads
/// and writes exactly that. Putting it under a directory of our own would be a
/// cache only we can use, which is the whole thing this is for.
///
/// [`cache_dir`] is still the right answer for anything of ours.
#[must_use]
pub fn cache_root() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
}

/// Writes `bytes` to `dir/file`, whole and renamed over.
///
/// Written to a scratch file and renamed, so a full disk or a kill mid-write
/// leaves the previous file rather than half of this one — and so a watcher on
/// the directory sees ONE event (`MOVED_TO`) with the finished content, never
/// a file that is still being written. The directory is created if it is not
/// there.
///
/// # The scratch name carries the PROCESS, and that is not decoration
///
/// It was `<file>.new`, one name for everybody. That held while each of these
/// files had a single writer, and the screens ended it: two doors — two
/// PROCESSES — write `display.json` and `scroll-display.conf` through here. Two
/// of them at once open the same scratch path with truncate, so one write can
/// interleave with the other and the rename then installs a mixed file; the
/// other outcome is the loser's `rename` answering `ENOENT` because the winner
/// already moved the path away.
///
/// A mixed file is the worse of the two, because `display.json` is JSON: half
/// of one write and half of another does not parse, and what a store does with
/// a file that will not parse is fall back to the default. One interleaved
/// write would therefore read as "nothing was ever chosen".
///
/// The pid makes the scratch private to the writer, so the two can only race
/// at the `rename`, which is atomic and is the one place this function is
/// allowed to have a winner.
///
/// # Errors
///
/// The directory cannot be created, or the file cannot be written or renamed.
pub fn write_atomically(dir: &Path, file: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let scratch = dir.join(format!("{file}.{}.new", std::process::id()));
    std::fs::write(&scratch, bytes)?;
    // AND THE SCRATCH DOES NOT SURVIVE A FAILED RENAME. With one shared name
    // the next writer overwrote whatever was left; a per-process name would
    // otherwise accumulate one file per crashed pid, in a directory a person
    // opens.
    match std::fs::rename(&scratch, dir.join(file)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&scratch);
            Err(e)
        }
    }
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
        // NO SCRATCH LEFT, whatever it was called. Globbing rather than
        // naming it: the scratch carries the pid now, and a test that knows
        // the exact spelling would pass over a rename that never happened.
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".new"))
            .collect();
        assert!(left.is_empty(), "a scratch file was left behind: {left:?}");
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

    #[test]
    fn one_writer_never_touches_another_writers_scratch() {
        // TWO DOORS ARE TWO PROCESSES, and both write `display.json` through
        // here. With one shared `<file>.new` they open the same path with
        // truncate: one write interleaves with the other and the rename
        // installs a mixed file — which for JSON means a file that will not
        // parse, and a store that will not parse falls back to the default.
        // One interleaved write would read as "nothing was ever chosen".
        //
        // A test cannot fork a second process here, but it can assert the
        // property that makes the race impossible: the scratch belongs to ONE
        // writer, so another's is not the same path and is left alone.
        let dir = scratch("two-writers");
        std::fs::create_dir_all(&dir).unwrap();
        // A scratch that some other process is in the middle of writing.
        let theirs = dir.join("display.json.999999.new");
        std::fs::write(&theirs, b"half of somebody else's write").unwrap();

        write_atomically(&dir, "display.json", b"{\"screens\":{}}").unwrap();

        assert_eq!(
            std::fs::read(&theirs).unwrap(),
            b"half of somebody else's write",
            "this write reached into another writer's scratch",
        );
        assert_eq!(
            std::fs::read(dir.join("display.json")).unwrap(),
            b"{\"screens\":{}}",
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
