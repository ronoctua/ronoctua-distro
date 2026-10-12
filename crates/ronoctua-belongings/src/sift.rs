//! What the walk never keeps because of what a file IS, wherever it lies.
//!
//! The table ([`crate::kept`]) decides by PATH. Two things cannot be told by
//! a path, and both must never reach the repository:
//!
//! - **A link into the system's store** (`/nix/store/…`), reason
//!   `system-managed`. Home Manager puts such links in the home, and a
//!   link is kept as its target's text: on another machine — or on this one
//!   after a garbage collection — that target is not there, and the link
//!   comes back DANGLING over whatever the system would have put in its
//!   place. The system makes them again; they are not the person's.
//! - **The access key under another name**, reason `access`. The deploy key,
//!   its public half and the token live under `config/belongings-keys`, which
//!   is never kept, whoever asks — but a hard link of the deploy key inside
//!   `~/.ssh` (the person's own keys, which the belongings do carry) is the
//!   same file under a name the table cannot see. So a file is compared with
//!   the access files by device and inode (the same file) and by content (a
//!   copy): "`~/.ssh` minus the deploy key" holds under any name.
//!
//! The walk sets each one aside in [`crate::walk::Found::never`] with its
//! reason, so the page that lists what is kept can say why.

use std::os::unix::fs::MetadataExt as _;
use std::path::{Component, Path, PathBuf};

use crate::places::Places;

/// The system's store, where the links the system makes point.
pub const STORE: &str = "/nix/store";

/// A link into [`STORE`] (`belongings-reason-system-managed`).
pub const SYSTEM_MANAGED: &str = "system-managed";

/// The access to the repository under another name
/// (`belongings-reason-access`, the table's own reason for the access files).
pub const ACCESS: &str = "access";

/// Every reason this module gives — each a sentence in every language.
pub const REASONS: [&str; 2] = [SYSTEM_MANAGED, ACCESS];

/// Whether a link whose text is `target`, sitting at `at`, points into the
/// system's store — an absolute target, or a relative one resolved against
/// the link's own folder, lexically (nothing is followed). Pure.
#[must_use]
pub fn into_the_store(at: &Path, target: &Path) -> bool {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        at.parent().unwrap_or_else(|| Path::new("/")).join(target)
    };
    let mut normal = PathBuf::from("/");
    for part in joined.components() {
        match part {
            Component::ParentDir => {
                normal.pop();
            }
            Component::Normal(p) => normal.push(p),
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    normal.starts_with(STORE)
}

/// The access files of this machine, as the walk compares against them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Access {
    /// Device and inode of each that is there.
    same: Vec<(u64, u64)>,
    /// The SHA-256 of each that could be read (feature `crypt`: the walk
    /// hashes every file it keeps, so a copy is caught by its hash).
    sums: Vec<String>,
}

impl Access {
    /// The deploy key, its public half and the token, as they are now.
    #[must_use]
    pub fn of(places: &Places) -> Self {
        let mut out = Self::default();
        for path in [places.deploy_key(), places.deploy_public(), places.token()] {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            out.same.push((meta.dev(), meta.ino()));
            // AN EMPTY ACCESS FILE holds no secret, and every empty file
            // would be its copy (the listing says the same, `inventory`).
            #[cfg(feature = "crypt")]
            if meta.len() > 0
                && let Ok(mut file) = std::fs::File::open(&path)
                && let Ok(sum) = crate::walk::sha256_of(&mut file)
            {
                out.sums.push(sum);
            }
        }
        out
    }

    /// Whether `meta` is one of the access files (a hard link of one).
    #[must_use]
    pub fn is_the_same_file(&self, meta: &std::fs::Metadata) -> bool {
        self.same.contains(&(meta.dev(), meta.ino()))
    }

    /// Whether content of this SHA-256 is one of the access files' (a copy).
    #[must_use]
    pub fn has_the_content(&self, sha256: &str) -> bool {
        self.sums.iter().any(|s| s == sha256)
    }
}

/// Why the file at `at` is never kept, by what it is — `None` for an
/// ordinary file. One `lstat`; a link's text is read, never followed.
#[must_use]
pub fn reason(at: &Path, access: &Access) -> Option<&'static str> {
    let meta = std::fs::symlink_metadata(at).ok()?;
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(at).ok()?;
        return into_the_store(at, &target).then_some(SYSTEM_MANAGED);
    }
    (meta.is_file() && access.is_the_same_file(&meta)).then_some(ACCESS)
}

/// Moves out of `found.files` every path that [`reason`] (or a copy of an
/// access file, by its hash) says is never kept, into `found.never`.
#[cfg(feature = "crypt")]
pub fn sift(places: &Places, found: &mut crate::walk::Found) {
    let access = Access::of(places);
    let refused: Vec<(String, &'static str)> = found
        .files
        .iter()
        .filter_map(|(logical, side)| {
            let at = places.resolve(logical)?;
            let why = reason(&at, &access).or_else(|| {
                (side.kind == crate::plan::Kind::File && access.has_the_content(&side.sha256))
                    .then_some(ACCESS)
            })?;
            Some((logical.clone(), why))
        })
        .collect();
    for (logical, why) in refused {
        found.files.remove(&logical);
        found.never.insert(logical, why);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_into_the_store_is_found_however_it_is_written() {
        let at = Path::new("/home/me/.config/app/settings");
        assert!(into_the_store(
            at,
            Path::new("/nix/store/abc-home-manager-files/.config/app/settings")
        ));
        assert!(into_the_store(
            at,
            Path::new("../../../../nix/store/abc-x/settings")
        ));
        assert!(!into_the_store(at, Path::new("settings.real")));
        assert!(!into_the_store(at, Path::new("/nix/storefront/x")));
        assert!(!into_the_store(at, Path::new("/home/me/notes")));
    }

    #[cfg(feature = "crypt")]
    mod walked {
        use std::collections::BTreeSet;

        use super::super::*;
        use crate::kept::{self, Verdict};
        use crate::prefs::Belongings;
        use crate::walk::walk;

        fn scratch(name: &str) -> PathBuf {
            let d = std::env::temp_dir().join(format!(
                "ronoctua-belongings-sift-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            d
        }

        /// FAILS IF: a draft a store leaves behind when it dies mid-write —
        /// `write_atomically`'s `<file>.<pid>.new`, or the older
        /// `.<file>.<pid>.tmp` — is walked as a kept file and would travel.
        #[test]
        fn belongings_drafts_are_never_kept() {
            let root = scratch("drafts");
            let p = Places::under(&root);
            std::fs::create_dir_all(p.config.join("studio")).unwrap();
            std::fs::write(p.config.join("theme.json"), b"{}").unwrap();
            std::fs::write(p.config.join("theme.json.4242.new"), b"{\"theme\"").unwrap();
            std::fs::write(p.config.join(".theme.json.9.tmp"), b"{").unwrap();
            std::fs::write(p.config.join("studio/scenes.json.77.new"), b"[").unwrap();
            let none = BTreeSet::new();
            let found = walk(
                &crate::scope::Scope::for_sync(&Belongings::default(), &none, &none),
                &p,
            );
            let kept: Vec<&str> = found.files.keys().map(String::as_str).collect();
            assert_eq!(kept, ["config/theme.json"], "a draft was kept");
            for draft in [
                "config/theme.json.4242.new",
                "config/.theme.json.9.tmp",
                "config/studio/scenes.json.77.new",
            ] {
                assert!(
                    matches!(kept::by_default(draft), Verdict::Never(n) if n.reason == "draft"),
                    "{draft}: {:?}",
                    kept::by_default(draft)
                );
            }
            let _ = std::fs::remove_dir_all(root);
        }

        /// One blob of the repository's index, pushed by another machine.
        fn theirs(path: &str, kind: crate::plan::Kind) -> crate::merge::Blob {
            crate::merge::Blob {
                path: path.to_owned(),
                side: crate::plan::Side {
                    size_bytes: 3,
                    modified_s: 0,
                    sha256: "0".repeat(64),
                    kind,
                    mode: 0o644,
                },
                machine: "other".to_owned(),
                added: false,
                ..crate::merge::Blob::default()
            }
        }

        /// FAILS IF: a link into the system's store (what Home Manager puts in
        /// a home) is kept — it would come back dangling — or the merge pulls
        /// the repository's copy over it, or through it when it is a link to
        /// a FOLDER, or an ordinary link stops being kept.
        #[test]
        fn belongings_store_links_are_not_kept() {
            let root = scratch("store-links");
            let p = Places::under(&root);
            std::fs::create_dir_all(&p.config).unwrap();
            std::fs::create_dir_all(p.home.join("notes")).unwrap();
            std::os::unix::fs::symlink(
                "/nix/store/0000000000000000000000000000000-home-manager-files/app.json",
                p.config.join("app.json"),
            )
            .unwrap();
            // A LINK TO A FOLDER of the store: Home Manager's `~/.config/<app>`.
            std::os::unix::fs::symlink(
                "/nix/store/0000000000000000000000000000000-home-manager-files/tool",
                p.config.join("tool"),
            )
            .unwrap();
            std::fs::write(p.home.join("notes/a.md"), b"# a").unwrap();
            std::os::unix::fs::symlink("a.md", p.home.join("notes/b.md")).unwrap();
            let mut prefs = Belongings::default();
            prefs.extra.push("home/notes".to_owned());
            let none = BTreeSet::new();
            let found = walk(&crate::scope::Scope::for_sync(&prefs, &none, &none), &p);
            assert!(
                !found.files.contains_key("config/app.json"),
                "a store link was kept"
            );
            assert_eq!(found.never.get("config/app.json"), Some(&SYSTEM_MANAGED));
            assert_eq!(found.never.get("config/tool"), Some(&SYSTEM_MANAGED));
            assert!(
                found.files.contains_key("home/notes/b.md"),
                "an ordinary link was dropped"
            );

            // THE REPOSITORY'S COPY — pushed by a machine before this rule —
            // is neither pulled over the system's link nor deleted for the
            // others: left alone.
            let mut remote = crate::merge::Index::fresh();
            remote
                .entries
                .push(theirs("config/app.json", crate::plan::Kind::Link));
            // A FILE ANOTHER MACHINE KEEPS where this one has the system's
            // folder link: pulled here, it would be written THROUGH the link.
            remote
                .entries
                .push(theirs("config/tool/settings.json", crate::plan::Kind::File));
            let plan = crate::merge::plan(None, &found, &remote, &prefs);
            for path in ["config/app.json", "config/tool/settings.json"] {
                assert!(
                    !plan.entries.iter().any(|e| e.path == path),
                    "the merge touched {path}, the system's link or under it: {:?}",
                    plan.entries
                );
                assert!(plan.conflicts.iter().all(|c| c.path != path), "{path}");
            }
            let _ = std::fs::remove_dir_all(root);
        }

        /// FAILS IF: the deploy key travels under another name — a hard link
        /// of it inside `~/.ssh`, or a copy — when the person keeps `~/.ssh`;
        /// or the person's own keys there stop travelling.
        #[test]
        fn belongings_access_never_travels() {
            let root = scratch("access");
            let p = Places::under(&root);
            std::fs::create_dir_all(p.keys()).unwrap();
            std::fs::create_dir_all(p.home.join(".ssh")).unwrap();
            std::fs::write(
                p.deploy_key(),
                // In pieces, or the repository's secret scan takes it for a key.
                concat!("-----BEGIN OPENSSH ", "PRIVATE", " KEY----- deploy"),
            )
            .unwrap();
            std::fs::write(p.deploy_public(), b"ssh-ed25519 AAAA deploy").unwrap();
            std::fs::write(p.home.join(".ssh/id_ed25519"), b"-----BEGIN mine").unwrap();
            std::fs::hard_link(p.deploy_key(), p.home.join(".ssh/belongings")).unwrap();
            std::fs::copy(p.deploy_public(), p.home.join(".ssh/belongings.pub")).unwrap();
            let mut prefs = Belongings::default();
            prefs.extra.push("home/.ssh".to_owned());
            let none = BTreeSet::new();
            let found = walk(&crate::scope::Scope::for_sync(&prefs, &none, &none), &p);
            assert!(
                found.files.contains_key("home/.ssh/id_ed25519"),
                "the person's own key stopped travelling"
            );
            for name in ["belongings", "belongings.pub"] {
                let logical = format!("home/.ssh/{name}");
                assert!(!found.files.contains_key(&logical), "{logical} travels");
                assert_eq!(found.never.get(&logical), Some(&ACCESS), "{logical}");
            }
            assert!(
                !found.files.keys().any(|k| k.contains("belongings-keys")),
                "the access folder itself was walked"
            );

            // THE KEY IN THE REPOSITORY ALREADY — pushed before this rule —
            // leaves it from the machine that sees what it is: left there,
            // every other machine would keep pulling it into its `~/.ssh`.
            let mut remote = crate::merge::Index::fresh();
            remote
                .entries
                .push(theirs("home/.ssh/belongings", crate::plan::Kind::File));
            let plan = crate::merge::plan(None, &found, &remote, &prefs);
            let step = |path: &str| plan.entries.iter().find(|e| e.path == path).map(|e| e.step);
            assert_eq!(
                step("home/.ssh/belongings"),
                Some(crate::plan::Step::Unkeep),
                "the deploy key stays in the repository: {:?}",
                plan.entries
            );
            assert_eq!(
                step("home/.ssh/belongings.pub"),
                None,
                "a key that never travelled is pushed or unkept"
            );
            assert_eq!(
                step("home/.ssh/id_ed25519"),
                Some(crate::plan::Step::Push),
                "the person's own key stopped travelling"
            );
            let _ = std::fs::remove_dir_all(root);
        }
    }
}
