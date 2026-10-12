//! The person's own copy of their choice — the mirror of `options.json`
//! under their state root (`$XDG_STATE_HOME/ronoctua/person/options.json`),
//! which the screen writes and the belongings carry, SIGNED, in the plain
//! part to every machine of theirs. The machine reads root's validated copy
//! of it at its next update (`nix/modules/binary-cache.nix`); nothing here
//! reaches the machine directory.
//!
//! Read with `ronoctua_person::parse` and written whole with
//! `ronoctua_person::to_json`, through a draft: a file of a newer format, or
//! one that does not read, is refused and never rewritten blind — the rest
//! of the person's options (their SSH signing, their security keys) are in
//! the same file.

use std::path::{Path, PathBuf};

use ronoctua_person::options::{BinaryCache, Options};
use ronoctua_person::{Document as _, Why as PersonWhy};

use crate::Why;

/// What the person's copy names.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Named {
    /// No cache — or no file yet.
    #[default]
    None,
    /// This cache.
    Cache(BinaryCache),
    /// The file is there and does not read.
    Unreadable,
    /// The file is of a newer format than this machine reads.
    Newer,
}

impl Named {
    /// The cache, when one is named.
    #[must_use]
    pub const fn cache(&self) -> Option<&BinaryCache> {
        match self {
            Self::Cache(c) => Some(c),
            _ => None,
        }
    }
}

/// This instance's copy — `None` with no home.
#[must_use]
pub fn mirror() -> Option<PathBuf> {
    ronoctua_xdg::state_dir().map(|d| {
        d.join(ronoctua_person::names::MIRROR)
            .join(ronoctua_person::names::OPTIONS)
    })
}

/// What the copy at `path` names.
#[must_use]
pub fn read(path: &Path) -> Named {
    match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Named::None,
        Err(_) => Named::Unreadable,
        Ok(bytes) => match ronoctua_person::parse::<Options>(&bytes) {
            Ok(o) => o.binary_cache.map_or(Named::None, Named::Cache),
            Err(r) if r.why == PersonWhy::Newer => Named::Newer,
            Err(_) => Named::Unreadable,
        },
    }
}

/// Changes the cache the copy at `path` names — `change` gets what it names
/// now and leaves what it should name — and writes the whole file back, its
/// other options as they were.
///
/// # Errors
///
/// [`Why::Newer`] and [`Why::Unreadable`] for a file that is not rewritten;
/// [`Why::NoCache`] when the cache `change` leaves is not one;
/// [`Why::Unwritable`].
pub fn change(path: &Path, change: impl FnOnce(&mut Option<BinaryCache>)) -> Result<(), Why> {
    let mut options = match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Options::default(),
        Err(_) => return Err(Why::Unreadable),
        Ok(bytes) => ronoctua_person::parse::<Options>(&bytes).map_err(|r| {
            if r.why == PersonWhy::Newer {
                Why::Newer
            } else {
                Why::Unreadable
            }
        })?,
    };
    change(&mut options.binary_cache);
    options.validate().map_err(|_| Why::NoCache)?;
    let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str())) else {
        return Err(Why::Unwritable);
    };
    std::fs::create_dir_all(dir).map_err(|_| Why::Unwritable)?;
    ronoctua_xdg::write_atomically(dir, file, ronoctua_person::to_json(&options).as_bytes())
        .map_err(|_| Why::Unwritable)
}

/// What root's netrc should be, after the person's copy and their token: the
/// cache to write it for — a PRIVATE cache named, and a token kept — or
/// `None`, and then no netrc at all. Pure.
#[must_use]
pub fn netrc_for(named: &Named, token_kept: bool) -> Option<String> {
    named
        .cache()
        .filter(|c| c.is_private() && token_kept)
        .map(|c| c.name.clone())
}

#[cfg(test)]
mod tests {
    use ronoctua_person::options::Visibility;

    use super::*;

    const KEY: &str = "someone.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ=";

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ronoctua-cachix-settings-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn cache(visibility: Visibility) -> BinaryCache {
        BinaryCache {
            name: "someone".to_owned(),
            public_key: KEY.to_owned(),
            visibility,
        }
    }

    /// FAILS IF: naming the cache loses the person's other options, writes a
    /// cache that is not one, or rewrites a file of a newer format or one
    /// that does not read.
    #[test]
    fn the_cache_is_written_beside_the_other_options_and_a_strange_file_is_left() {
        let root = scratch("change");
        let path = root.join("person").join("options.json");
        assert_eq!(read(&path), Named::None);
        change(&path, |c| *c = Some(cache(Visibility::Public))).unwrap();
        assert_eq!(read(&path), Named::Cache(cache(Visibility::Public)));
        // THE OTHER OPTIONS STAY.
        let mut text = std::fs::read_to_string(&path).unwrap();
        text = text.replace(
            "\"ssh-signing\": null",
            "\"ssh-signing\": {\"key\": \".ssh/id.pub\"}",
        );
        std::fs::write(&path, &text).unwrap();
        change(&path, |c| {
            if let Some(c) = c.as_mut() {
                c.visibility = Visibility::Private;
            }
        })
        .unwrap();
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains(".ssh/id.pub"), "{after}");
        assert_eq!(read(&path), Named::Cache(cache(Visibility::Private)));
        // NOT A CACHE: refused, and the file as it was.
        let bad = change(&path, |c| {
            if let Some(c) = c.as_mut() {
                c.public_key =
                    "another.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ=".to_owned();
            }
        });
        assert_eq!(bad, Err(Why::NoCache));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), after);
        // NEWER, and NOT JSON: never rewritten.
        std::fs::write(&path, "{\"format\": 9}").unwrap();
        assert_eq!(read(&path), Named::Newer);
        assert_eq!(change(&path, |c| *c = None), Err(Why::Newer));
        std::fs::write(&path, "not json").unwrap();
        assert_eq!(read(&path), Named::Unreadable);
        assert_eq!(change(&path, |c| *c = None), Err(Why::Unreadable));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: root's netrc is wanted for a public cache, without a
    /// token, or with no cache — or not wanted for a private one with one.
    #[test]
    fn a_netrc_is_wanted_only_for_a_private_cache_with_a_token() {
        let private = Named::Cache(cache(Visibility::Private));
        let public = Named::Cache(cache(Visibility::Public));
        assert_eq!(netrc_for(&private, true), Some("someone".to_owned()));
        assert_eq!(netrc_for(&private, false), None);
        assert_eq!(netrc_for(&public, true), None);
        assert_eq!(netrc_for(&Named::None, true), None);
        assert_eq!(netrc_for(&Named::Newer, true), None);
    }
}
