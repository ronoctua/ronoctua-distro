//! This machine's side (feature `crypt`): every path it keeps, read.
//!
//! The walk starts at every root of what is kept — the trees, the files, the
//! targets, and the folders and files a person added on any of their
//! machines (the index's roots), all read by [`Scope`] alone — and asks the
//! scope ([`Scope::kept_here`]) about each path it meets. Links are read as links (their target is what is kept), never
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
use crate::places::{Places, Root};
use crate::plan::{Kind, Side};
use crate::scope::{Met, Scope};

/// What the walk found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Every path kept here that could be read, and its side.
    pub files: BTreeMap<String, Side>,
    /// The paths kept here that could not be read.
    pub unreadable: BTreeSet<String>,
    // ── anchor: L07 ──
    /// The paths the rules keep that are NEVER kept for what they are, with
    /// the reason's id ([`crate::sift`]): a link into the system's store, the
    /// access key under another name. Left alone on both sides by the merge.
    pub never: BTreeMap<String, &'static str>,
    // ── anchor: L09 ──
    // ── anchor: L10 ──
    // ── anchor: L21 ──
}

/// The SHA-256 of what `from` gives, hexadecimal — streamed.
///
/// # Errors
///
/// A read failed.
pub fn sha256_of(from: &mut dyn std::io::Read) -> std::io::Result<String> {
    let mut context = aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA256);
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

/// Walks every path this machine keeps, from the [`Scope`]'s roots — the
/// walk the listing of what goes to the repository is made of too
/// (`crate::inventory`): both meet the same paths ([`Scope::meet`]), and
/// this one reads (hashes) what the scope keeps.
#[must_use]
pub fn walk(scope: &Scope, places: &Places) -> Found {
    let mut found = Found::default();
    scope.meet(places, &mut |logical, at, met| match met {
        // A FOLDER THAT CANNOT BE READ hides what is in it: the merge takes
        // it for every path under it.
        Met::Hidden => {
            if scope.kept_here(logical) {
                found.unreadable.insert(logical.to_owned());
            }
        }
        Met::Entry(_) => {
            if !scope.kept_here(logical) {
                return;
            }
            match side_of(at) {
                Ok(Some(mut side)) => {
                    // A PRIVATE TARGET'S FILE is read with its owner's bits
                    // alone: group and other never travel
                    // (`kept::kept_mode`).
                    if side.kind == Kind::File {
                        side.mode = kept::kept_mode(logical, side.mode);
                    }
                    found.files.insert(logical.to_owned(), side);
                }
                Ok(None) => {}
                Err(_) => {
                    found.unreadable.insert(logical.to_owned());
                }
            }
        }
    });
    crate::sift::sift(places, &mut found);
    found
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
    use crate::prefs::Belongings;

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
        let found = walk(&crate::scope::Scope::for_sync(&prefs, &none, &none), &p);
        let paths: Vec<&str> = found.files.keys().map(String::as_str).collect();
        assert_eq!(paths, ["config/player-keys/tmdb", "config/theme.json"]);

        prefs.extra.push("home/notes".to_owned());
        let found = walk(&crate::scope::Scope::for_sync(&prefs, &none, &none), &p);
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
        let found = walk(&crate::scope::Scope::for_sync(&prefs, &none, &none), &p);
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
        let found = walk(
            &crate::scope::Scope::for_sync(&Belongings::default(), &none, &none),
            &p,
        );
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
        let found = walk(
            &crate::scope::Scope::for_sync(&Belongings::default(), &none, &none),
            &p,
        );
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
