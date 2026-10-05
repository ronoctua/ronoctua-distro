//! This machine's side (feature `crypt`): every path it keeps, read.
//!
//! The walk starts at every root of what is kept — the trees, the files, and
//! the folders and files a person added on any of their machines (the
//! index's roots) — and asks [`crate::kept::kept_here`] about each path it
//! meets. Links are read as links (their target is what is kept), never
//! followed; a socket or a pipe is not a file of anybody's. A file — or a
//! FOLDER — that cannot be read is UNREADABLE and not missing: the merge
//! leaves it, and everything under it, where it is on both sides, instead of
//! deleting it from the repository.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use crate::kept;
use crate::places::{Places, Root, covers};
use crate::plan::{Kind, Side};
use crate::prefs::Belongings;

/// What the walk found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Every path kept here that could be read, and its side.
    pub files: BTreeMap<String, Side>,
    /// The paths kept here that could not be read.
    pub unreadable: BTreeSet<String>,
}

/// The SHA-256 of what `from` gives, hexadecimal — streamed.
///
/// # Errors
///
/// A read failed.
pub fn sha256_of(from: &mut dyn std::io::Read) -> std::io::Result<String> {
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = from.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        context.update(&buffer[..n]);
    }
    Ok(hex(context.finish().as_ref()))
}

/// Bytes as lowercase hexadecimal.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// One path's side, read: a file hashed, a link's target hashed. `None` for
/// what is neither (a folder, a socket, a pipe).
///
/// # Errors
///
/// It could not be read.
pub fn side_of(path: &Path) -> std::io::Result<Option<Side>> {
    let meta = std::fs::symlink_metadata(path)?;
    let modified_s = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    let kind = meta.file_type();
    if kind.is_symlink() {
        let target = std::fs::read_link(path)?;
        let bytes = target.as_os_str().as_bytes();
        return Ok(Some(Side {
            size_bytes: bytes.len() as u64,
            modified_s,
            sha256: sha256_of(&mut &bytes[..])?,
            kind: Kind::Link,
            mode: 0o777,
        }));
    }
    if !kind.is_file() {
        return Ok(None);
    }
    let mut file = std::fs::File::open(path)?;
    let sha256 = sha256_of(&mut file)?;
    Ok(Some(Side {
        size_bytes: meta.len(),
        modified_s,
        sha256,
        kind: Kind::File,
        mode: meta.permissions().mode() & 0o7777,
    }))
}

/// Walks every path this machine keeps. `added_elsewhere` are the roots a
/// person added on any machine (`crate::merge::roots_after`); `extra_roots`
/// the paths the walk must also look at (the base's, so a deletion here is
/// seen).
#[must_use]
pub fn walk(
    places: &Places,
    prefs: &Belongings,
    added_elsewhere: &BTreeSet<String>,
    extra_roots: &BTreeSet<String>,
) -> Found {
    let mut roots: BTreeSet<String> = kept::TREES
        .iter()
        .map(|t| t.path.to_owned())
        .chain(kept::FILES.iter().map(|f| f.path.to_owned()))
        .chain(prefs.extra.iter().cloned())
        .chain(added_elsewhere.iter().cloned())
        .chain(extra_roots.iter().cloned())
        .collect();
    // A root under another is walked with it.
    let all: Vec<String> = roots.iter().cloned().collect();
    roots.retain(|r| !all.iter().any(|o| o != r && covers(o, r)));
    let mut found = Found::default();
    for root in &roots {
        let Some(at) = places.resolve(root) else {
            continue;
        };
        visit(places, prefs, added_elsewhere, root, &at, &mut found, 0);
    }
    found
}

/// How deep a walk goes: a loop of bind mounts must end somewhere.
const DEEPEST: usize = 64;

fn visit(
    places: &Places,
    prefs: &Belongings,
    added_elsewhere: &BTreeSet<String>,
    logical: &str,
    at: &Path,
    found: &mut Found,
    depth: usize,
) {
    // A PATH INSIDE ANOTHER ROOT is that root's: `home/.config/ronoctua` is
    // `config`, walked as itself.
    if places.logical_of(at).as_deref() != Some(logical) {
        return;
    }
    let Ok(meta) = std::fs::symlink_metadata(at) else {
        return;
    };
    let elsewhere = added_elsewhere.iter().any(|a| covers(a, logical));
    if meta.is_dir() {
        if depth >= DEEPEST {
            return;
        }
        let Ok(entries) = std::fs::read_dir(at) else {
            // A FOLDER THAT CANNOT BE READ hides what is in it: the merge
            // takes it for every path under it.
            if kept::kept_here(logical, prefs, elsewhere) {
                found.unreadable.insert(logical.to_owned());
            }
            return;
        };
        let mut names: Vec<_> = entries.flatten().map(|e| e.file_name()).collect();
        names.sort();
        for name in names {
            let Some(name) = name.to_str() else {
                continue;
            };
            let child = format!("{logical}/{name}");
            if !crate::places::is_logical(&child) {
                continue;
            }
            visit(
                places,
                prefs,
                added_elsewhere,
                &child,
                &at.join(name),
                found,
                depth + 1,
            );
        }
        return;
    }
    if !kept::kept_here(logical, prefs, elsewhere) {
        return;
    }
    match side_of(at) {
        Ok(Some(side)) => {
            found.files.insert(logical.to_owned(), side);
        }
        Ok(None) => {}
        Err(_) => {
            found.unreadable.insert(logical.to_owned());
        }
    }
}

/// Reads the first `most` bytes of a file, for the text test.
#[must_use]
pub fn head(path: &Path, most: u64) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(most)
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// Whether bytes read as text a diff can show: UTF-8 with no NUL. Pure.
#[must_use]
pub fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

/// The roots of every logical path, for a check that a root exists here.
#[must_use]
pub fn root_of(logical: &str) -> Option<Root> {
    Root::split(logical).map(|(r, _)| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ronoctua-belongings-walk-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_walk_keeps_what_the_rules_keep_and_reads_links_as_links() {
        let root = scratch("rules");
        let p = Places::under(&root);
        std::fs::create_dir_all(p.config.join("player-keys")).unwrap();
        std::fs::create_dir_all(p.keys()).unwrap();
        std::fs::create_dir_all(p.home.join("notes")).unwrap();
        std::fs::write(p.config.join("theme.json"), b"{}").unwrap();
        std::fs::write(p.config.join("player-keys/tmdb"), b"k").unwrap();
        std::fs::write(p.config.join("column-player.pid"), b"1").unwrap();
        std::fs::write(p.config.join(".theme.json.9.tmp"), b"{").unwrap();
        std::fs::write(p.keys().join("deploy"), b"secret").unwrap();
        std::fs::write(p.home.join("notes/a.md"), b"# a").unwrap();
        std::os::unix::fs::symlink("a.md", p.home.join("notes/b.md")).unwrap();

        let mut prefs = Belongings::default();
        let none = BTreeSet::new();
        let found = walk(&p, &prefs, &none, &none);
        let paths: Vec<&str> = found.files.keys().map(String::as_str).collect();
        assert_eq!(paths, ["config/player-keys/tmdb", "config/theme.json"]);

        prefs.extra.push("home/notes".to_owned());
        let found = walk(&p, &prefs, &none, &none);
        let link = &found.files["home/notes/b.md"];
        assert_eq!(link.kind, Kind::Link);
        assert_eq!(link.size_bytes, 4, "the target's length, never the file's");
        assert!(found.files.contains_key("home/notes/a.md"));
        assert!(
            !found.files.keys().any(|k| k.contains("belongings-keys")),
            "the deploy key was walked"
        );
        // ADDING THE HOME ITSELF does not walk the configuration twice.
        prefs.extra.push("home".to_owned());
        let found = walk(&p, &prefs, &none, &none);
        assert!(!found.files.keys().any(|k| k.starts_with("home/.config")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_unreadable_and_not_missing() {
        let root = scratch("unreadable");
        let p = Places::under(&root);
        std::fs::create_dir_all(&p.config).unwrap();
        let locked = p.config.join("locked.json");
        std::fs::write(&locked, b"{}").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let none = BTreeSet::new();
        let found = walk(&p, &Belongings::default(), &none, &none);
        // ROOT READS EVERYTHING: the test says so instead of passing blind.
        if std::fs::File::open(&locked).is_err() {
            assert!(found.unreadable.contains("config/locked.json"));
            assert!(!found.files.contains_key("config/locked.json"));
        } else {
            assert!(found.files.contains_key("config/locked.json"));
        }
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o600)).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a folder that cannot be read is not said as unreadable —
    /// the merge would take every file in it for deleted.
    #[test]
    fn a_folder_that_cannot_be_read_is_unreadable_and_not_empty() {
        let root = scratch("unreadable-folder");
        let p = Places::under(&root);
        let studio = p.config.join("studio");
        std::fs::create_dir_all(&studio).unwrap();
        std::fs::write(studio.join("scenes.json"), b"{}").unwrap();
        std::fs::set_permissions(&studio, std::fs::Permissions::from_mode(0o000)).unwrap();
        let none = BTreeSet::new();
        let found = walk(&p, &Belongings::default(), &none, &none);
        if std::fs::read_dir(&studio).is_err() {
            assert!(found.unreadable.contains("config/studio"), "{found:?}");
        } else {
            assert!(found.files.contains_key("config/studio/scenes.json"));
        }
        std::fs::set_permissions(&studio, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn text_is_utf8_without_nul() {
        assert!(is_text(b"{\"a\": 1}\n"));
        assert!(is_text("ação".as_bytes()));
        assert!(!is_text(b"\x00\x01"));
        assert!(!is_text(&[0xff, 0xfe]));
    }
}
