//! What the person chose for the index: `index.json`.
//!
//! Written by fast-access's settings (through [`ronoctua_store::Store`]) and
//! watched by the unit, which walks again when the roots or the skips change.
//! The distro's own limits — the ceiling, the content sizes, the names never
//! walked — are `assets/cache.nix`'s and arrive as the unit's arguments; this
//! file only adds to them.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};

/// The file's name, under `$XDG_CONFIG_HOME/ronoctua[-dev]/`.
pub const FILE: &str = "index.json";

/// The person's settings.
///
/// A value that means nothing is REFUSED, not dropped: a relative root, or a
/// skip that is neither a name nor an absolute path, makes the file not parse
/// — the unit logs why and keeps walking by what it had — instead of a folder
/// the person asked to leave out being indexed without a word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// The folders walked; empty is the home. Each is absolute.
    #[serde(deserialize_with = "absolute_roots")]
    pub roots: Vec<PathBuf>,
    /// Whether what is written in a file is read (names always are).
    pub contents: bool,
    /// Whether hidden files and folders (a name that starts with `.`) are
    /// walked.
    pub hidden: bool,
    /// Names (`build`) or absolute paths (`/home/ana/Videos/raw`) never
    /// walked, on top of the distro's `searchSkip`.
    #[serde(deserialize_with = "skip_entries")]
    pub skip: Vec<String>,
}

/// One skip entry, checked: a NAME (`build`, no `/`), matched anywhere, or an
/// absolute PATH (`/home/ana/Videos/raw`), with everything under it. Anything
/// else matches nothing — `Videos/raw` is neither, and a name is never `.`,
/// `..` or empty — so it is refused with the reason.
///
/// # Errors
///
/// Why the entry means nothing.
pub fn skip_entry(entry: &str) -> Result<(), String> {
    if entry.is_empty() || entry == "." || entry == ".." {
        return Err(format!("the skip entry {entry:?} names nothing"));
    }
    if entry.contains('/') && !entry.starts_with('/') {
        return Err(format!(
            "the skip entry {entry:?} is neither a name (no `/`) nor an absolute path"
        ));
    }
    Ok(())
}

fn skip_entries<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let entries = Vec::<String>::deserialize(d)?;
    for entry in &entries {
        skip_entry(entry).map_err(serde::de::Error::custom)?;
    }
    Ok(entries)
}

fn absolute_roots<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<PathBuf>, D::Error> {
    let roots = Vec::<PathBuf>::deserialize(d)?;
    if let Some(relative) = roots.iter().find(|r| !r.is_absolute()) {
        return Err(serde::de::Error::custom(format!(
            "the root {:?} is not an absolute path",
            relative.display().to_string()
        )));
    }
    Ok(roots)
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            contents: true,
            hidden: false,
            skip: Vec::new(),
        }
    }
}

impl Settings {
    /// Where they are kept: `index.json` in the configuration root.
    #[must_use]
    pub fn store() -> ronoctua_store::Store<Self> {
        ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
    }

    /// The folders to walk: the chosen ones, or `home` when none was chosen.
    /// A relative root is left out (it would be relative to wherever the unit
    /// happened to start — `index.json` with one does not even parse; this
    /// holds the rule for settings built in code), and so is one given twice.
    #[must_use]
    pub fn roots_or(&self, home: &Path) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        let chosen: Vec<&PathBuf> = self.roots.iter().filter(|r| r.is_absolute()).collect();
        if chosen.is_empty() {
            out.push(home.to_path_buf());
        } else {
            for root in chosen {
                if !out.contains(root) {
                    out.push(root.clone());
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_walks_the_home_with_contents_and_no_hidden_files() {
        let s = Settings::default();
        assert!(s.contents);
        assert!(!s.hidden);
        assert_eq!(
            s.roots_or(Path::new("/home/ana")),
            [PathBuf::from("/home/ana")]
        );
    }

    #[test]
    fn a_relative_or_repeated_root_is_not_walked() {
        let s = Settings {
            roots: vec![
                "docs".into(),
                "/srv/a".into(),
                "/srv/a".into(),
                "/srv/b".into(),
            ],
            ..Settings::default()
        };
        assert_eq!(
            s.roots_or(Path::new("/home/ana")),
            [PathBuf::from("/srv/a"), PathBuf::from("/srv/b")]
        );
    }

    #[test]
    fn a_skip_or_root_that_means_nothing_makes_the_file_refused() {
        for good in [
            r#"{ "skip": ["build", "/home/ana/Videos/raw", "/"] }"#,
            r#"{ "roots": ["/srv/a"] }"#,
        ] {
            assert!(serde_json::from_str::<Settings>(good).is_ok(), "{good}");
        }
        for (bad, why) in [
            (r#"{ "skip": ["Videos/raw"] }"#, "neither a name"),
            (r#"{ "skip": ["build", ""] }"#, "names nothing"),
            (r#"{ "skip": [".."] }"#, "names nothing"),
            (r#"{ "roots": ["docs"] }"#, "not an absolute path"),
        ] {
            let err = serde_json::from_str::<Settings>(bad)
                .unwrap_err()
                .to_string();
            assert!(err.contains(why), "{bad}: {err}");
        }
    }

    #[test]
    fn a_partial_file_keeps_the_defaults_and_an_unknown_field_is_refused() {
        let s: Settings = serde_json::from_str(r#"{ "contents": false }"#).unwrap();
        assert!(!s.contents);
        assert!(s.roots.is_empty());
        assert!(serde_json::from_str::<Settings>(r#"{ "rots": [] }"#).is_err());
    }
}
