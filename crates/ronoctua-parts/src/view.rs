//! The session's view of the parts that are here.
//!
//! The session finds a part's programs and launchers through ONE folder,
//! [`crate::names::VIEW`], which `nix/modules/parts.nix` puts among the
//! session's profiles: its `bin/` reaches `PATH`, its `share/` reaches
//! `XDG_DATA_DIRS`. It holds only [`LINKED`], file by file, each a link to
//! the part's STABLE path (never a store path): a part's next generation is
//! seen through the same link, and the view changes only when the set of
//! files does.
//!
//! Why a view and not each part among the profiles: a profile brings every
//! variable NixOS derives from one — `QT_PLUGIN_PATH`, `GIO_EXTRA_MODULES`,
//! `GTK_PATH` — and a part's plugin would be loaded into the base's
//! programs. The view has no `lib/`, so it brings none.

use std::collections::BTreeMap;

/// The folders of a part the view links, relative to its profile. Nothing
/// else of a part reaches the session.
pub const LINKED: [&str; 5] = [
    "bin",
    "share/applications",
    "share/icons",
    "share/pixmaps",
    "share/man",
];

/// One link of the view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Where, relative to the view (`bin/yt-dlp`).
    pub at: String,
    /// What it points at: the part's stable path and the same relative path.
    pub to: String,
}

/// Two parts offering the same file: the first in id order is linked, and
/// the other is said (`parts-do-not-collide` refuses it at build time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    /// The file, relative to the view.
    pub at: String,
    /// The part that is linked.
    pub kept: String,
    /// The part that is not.
    pub left: String,
}

/// The view of `parts`: each id with its stable profile path and the files
/// found under it, relative to it.
///
/// A file outside [`LINKED`], or one whose relative path climbs (`..`) or is
/// absolute, is never linked: the walk that lists them is the binary's, and
/// this is what holds it to the closed set.
#[must_use]
pub fn links(parts: &BTreeMap<String, (String, Vec<String>)>) -> (Vec<Link>, Vec<Collision>) {
    let mut taken: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut collisions = Vec::new();
    for (id, (profile, files)) in parts {
        for file in files {
            if !is_linked(file) {
                continue;
            }
            if let Some((kept, _)) = taken.get(file) {
                collisions.push(Collision {
                    at: file.clone(),
                    kept: kept.clone(),
                    left: id.clone(),
                });
                continue;
            }
            taken.insert(file.clone(), (id.clone(), format!("{profile}/{file}")));
        }
    }
    let links = taken
        .into_iter()
        .map(|(at, (_, to))| Link { at, to })
        .collect();
    (links, collisions)
}

/// Whether `file` (relative to a part's profile) may be linked.
#[must_use]
pub fn is_linked(file: &str) -> bool {
    !file.starts_with('/')
        && !file
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
        && LINKED.iter().any(|dir| {
            file.strip_prefix(dir)
                .is_some_and(|rest| rest.starts_with('/'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(profile: &str, files: &[&str]) -> (String, Vec<String>) {
        (
            profile.to_owned(),
            files.iter().map(|f| (*f).to_owned()).collect(),
        )
    }

    /// FAILS IF: anything outside the closed set reaches the session — a
    /// part's `lib/qt-6/plugins` would load into the base's programs — or a
    /// link points at a store path, so a part's update would leave the
    /// session on the old generation.
    #[test]
    fn the_view_links_only_the_closed_set_by_stable_path() {
        let mut parts = BTreeMap::new();
        parts.insert(
            "yt-dlp".to_owned(),
            part(
                "/nix/var/nix/profiles/ronoctua/parts/yt-dlp/v1",
                &[
                    "bin/yt-dlp",
                    "share/man/man1/yt-dlp.1.gz",
                    "lib/qt-6/plugins/x.so",
                    "bin/../etc/passwd",
                    "/bin/sh",
                    "bin",
                ],
            ),
        );
        let (links, collisions) = links(&parts);
        let at: Vec<&str> = links.iter().map(|l| l.at.as_str()).collect();
        assert_eq!(at, ["bin/yt-dlp", "share/man/man1/yt-dlp.1.gz"]);
        assert_eq!(
            links[0].to,
            "/nix/var/nix/profiles/ronoctua/parts/yt-dlp/v1/bin/yt-dlp"
        );
        assert!(collisions.is_empty());
    }

    /// FAILS IF: two parts offering one file are both linked, or silently —
    /// the session would run whichever the filesystem listed last.
    #[test]
    fn a_collision_is_said_and_the_first_kept() {
        let mut parts = BTreeMap::new();
        parts.insert("a".to_owned(), part("/p/a/v1", &["bin/x"]));
        parts.insert("b".to_owned(), part("/p/b/v1", &["bin/x", "bin/y"]));
        let (links, collisions) = links(&parts);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].to, "/p/a/v1/bin/x");
        assert_eq!(
            collisions,
            [Collision {
                at: "bin/x".to_owned(),
                kept: "a".to_owned(),
                left: "b".to_owned()
            }]
        );
    }
}
