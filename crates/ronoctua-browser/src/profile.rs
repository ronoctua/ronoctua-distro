//! A Chrome profile's files that no extension API reveals: the pinned
//! extensions (`Preferences`) and each extension's popup page (its manifest).
//!
//! # Why the host reads files at all
//!
//! `chrome.management` tells an extension every other extension's name,
//! version and options page — and not which ones the person PINNED to the
//! toolbar, nor which page its toolbar button opens. The modeline draws the
//! pinned ones one by one and opens their popup in a tab, so the host (which
//! runs as the person, beside the profile) reads the two answers where Chrome
//! keeps them.
//!
//! # Read only, bounded, and never fatal
//!
//! Nothing here writes into a profile: Chrome owns those files and rewrites
//! them whole. Every read has a ceiling, and every failure — a file missing, a
//! file too large, JSON that does not parse — fills nothing and says so in the
//! log: an extension drawn without its popup is a smaller kiosk, never a
//! broken one.

use crate::wire::State;
use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

/// The largest `Preferences` read. A profile's is tens to hundreds of
/// kibibytes; past this it is not a file Chrome wrote.
const PREFERENCES_CEILING: u64 = 16 * 1024 * 1024;

/// The largest extension manifest read.
const MANIFEST_CEILING: u64 = 1024 * 1024;

/// The two files that may hold the pinned list, in the order they are asked:
/// `Preferences` is where Chrome keeps it; `Secure Preferences` is where the
/// protected preferences go, and a Chrome that moved the list there is still
/// read.
pub const PREFERENCE_FILES: [&str; 2] = ["Preferences", "Secure Preferences"];

/// The profile's directory, `<config>/google-chrome/<profile>`, or `None`
/// for a name that is not a profile directory (`launch::is_profile_dir`) —
/// a name from the wire is never joined onto a path unchecked.
#[must_use]
pub fn profile_dir(config_home: &Path, profile: &str) -> Option<PathBuf> {
    crate::launch::is_profile_dir(profile).then(|| config_home.join("google-chrome").join(profile))
}

/// The ids pinned to the toolbar, in the toolbar's order, from a
/// `Preferences` text: `extensions.pinned_extensions`. `None` when the text
/// has no such list (asked of the next file), an empty list when it has an
/// empty one.
#[must_use]
pub fn pinned(preferences_json: &str) -> Option<Vec<String>> {
    let value: serde_json::Value = serde_json::from_str(preferences_json).ok()?;
    let list = value
        .get("extensions")?
        .get("pinned_extensions")?
        .as_array()?;
    Some(
        list.iter()
            .filter_map(|id| id.as_str())
            .filter(|id| crate::url::is_extension_id(id))
            .map(str::to_owned)
            .collect(),
    )
}

/// An extension's toolbar popup, a path inside the extension, from its
/// manifest: `action` (MV3), `browser_action` or `page_action` (MV2). A
/// leading slash is dropped, and a path that climbs out (`..`) is refused.
#[must_use]
pub fn popup(manifest_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(manifest_json).ok()?;
    ["action", "browser_action", "page_action"]
        .iter()
        .find_map(|key| value.get(key)?.get("default_popup")?.as_str())
        .map(|p| p.trim().trim_start_matches('/'))
        .filter(|p| !p.is_empty() && !p.split('/').any(|part| part == ".."))
        .map(str::to_owned)
}

/// Where an extension's manifest is: `<profile>/Extensions/<id>/<dir>/
/// manifest.json`, whose `<dir>` is `<version>_<n>`. The one for the version
/// the state reports, or — Chrome keeps the old one until it restarts — the
/// highest there is.
#[must_use]
pub fn manifest_path(profile_dir: &Path, id: &str, version: &str) -> Option<PathBuf> {
    if !crate::url::is_extension_id(id) {
        return None;
    }
    let root = profile_dir.join("Extensions").join(id);
    let mut dirs: Vec<String> = std::fs::read_dir(&root)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    let wanted = format!("{version}_");
    let exact = dirs
        .iter()
        .find(|d| !version.is_empty() && d.starts_with(&wanted))
        .cloned();
    let pick = exact.or_else(|| {
        dirs.sort_by_key(|d| version_key(d));
        dirs.pop()
    })?;
    Some(root.join(pick).join("manifest.json"))
}

/// A directory name `1.10.2_0` as numbers, so `1.10` sorts after `1.9`.
fn version_key(dir: &str) -> Vec<u64> {
    dir.split(['.', '_'])
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// Reads a file of at most `ceiling` bytes as text; `None` (and a line in the
/// log) for anything else.
fn bounded(path: &Path, ceiling: u64) -> Option<String> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            tracing::debug!(path = %path.display(), error = %e, "a profile file that cannot be opened");
            return None;
        }
    };
    let mut text = String::new();
    match file.take(ceiling + 1).read_to_string(&mut text) {
        Ok(n) if u64::try_from(n).is_ok_and(|n| n <= ceiling) => Some(text),
        Ok(_) => {
            tracing::warn!(path = %path.display(), ceiling, "a profile file past its ceiling");
            None
        }
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "a profile file that cannot be read");
            None
        }
    }
}

/// The pinned ids of a profile: the first of [`PREFERENCE_FILES`] that has
/// the list.
#[must_use]
pub fn pinned_in(profile_dir: &Path) -> Vec<String> {
    PREFERENCE_FILES
        .iter()
        .find_map(|name| pinned(&bounded(&profile_dir.join(name), PREFERENCES_CEILING)?))
        .unwrap_or_default()
}

/// Fills what the host adds to a state, remembering each manifest's answer:
/// a state arrives at every tab change, and an extension's popup changes only
/// with its version.
#[derive(Debug, Default)]
pub struct Reader {
    popups: HashMap<(String, String), Option<String>>,
}

impl Reader {
    /// Fills [`crate::wire::Extension::popup`] and `pinned_at` for every
    /// extension of the state's profile, from the files under `config_home`
    /// (`$XDG_CONFIG_HOME`). A state with no profile, or a profile that is not
    /// one, is left as it came.
    pub fn enrich(&mut self, state: &mut State, config_home: &Path) {
        let Some(dir) = state
            .profile
            .as_deref()
            .and_then(|p| profile_dir(config_home, p))
        else {
            return;
        };
        let pinned = pinned_in(&dir);
        for extension in &mut state.extensions {
            extension.pinned_at = pinned
                .iter()
                .position(|id| *id == extension.id)
                .and_then(|i| u32::try_from(i).ok());
            let key = (extension.id.clone(), extension.version.clone());
            let popup = self
                .popups
                .entry(key)
                .or_insert_with(|| {
                    manifest_path(&dir, &extension.id, &extension.version)
                        .and_then(|p| bounded(&p, MANIFEST_CEILING))
                        .and_then(|text| popup(&text))
                })
                .clone();
            extension.popup = popup;
        }
    }
}

/// [`Reader::enrich`] with nothing remembered.
pub fn enrich(state: &mut State, config_home: &Path) {
    Reader::default().enrich(state, config_home);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::Extension;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccccccccccccccccccc";

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-profile-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        dir
    }

    #[test]
    fn the_pinned_list_is_read_in_order_and_only_ids_are_kept() {
        let text = format!(
            r#"{{"extensions":{{"pinned_extensions":["{B}","not an id",7,"{A}"]}},"other":1}}"#
        );
        assert_eq!(pinned(&text), Some(vec![B.to_owned(), A.to_owned()]));
        assert_eq!(
            pinned(r#"{"extensions":{}}"#),
            None,
            "no list is asked of the next file"
        );
        assert_eq!(
            pinned(r#"{"extensions":{"pinned_extensions":[]}}"#),
            Some(vec![])
        );
        assert_eq!(pinned("not json"), None);
    }

    #[test]
    fn a_popup_is_found_in_mv3_and_mv2_and_never_climbs_out() {
        assert_eq!(
            popup(r#"{"action":{"default_popup":"popup.html"}}"#).as_deref(),
            Some("popup.html")
        );
        assert_eq!(
            popup(r#"{"browser_action":{"default_popup":"/ui/p.html"}}"#).as_deref(),
            Some("ui/p.html")
        );
        assert_eq!(
            popup(r#"{"page_action":{"default_popup":"p.html"}}"#).as_deref(),
            Some("p.html")
        );
        assert_eq!(popup(r#"{"action":{"default_title":"x"}}"#), None);
        assert_eq!(popup(r#"{"action":{"default_popup":"../../etc/x"}}"#), None);
        assert_eq!(popup(r#"{"action":{"default_popup":""}}"#), None);
    }

    #[test]
    fn a_name_that_is_not_a_profile_is_never_a_path() {
        let home = Path::new("/home/p/.config");
        assert_eq!(
            profile_dir(home, "Profile 2"),
            Some(PathBuf::from("/home/p/.config/google-chrome/Profile 2"))
        );
        assert_eq!(profile_dir(home, "../../etc"), None);
        assert_eq!(profile_dir(home, "Default/../x"), None);
    }

    #[test]
    fn the_manifest_of_the_reported_version_or_the_highest() {
        let dir = scratch("manifest");
        for v in ["1.9.0_0", "1.10.0_0", "1.2_0"] {
            std::fs::create_dir_all(dir.join("Extensions").join(A).join(v)).expect("dir");
        }
        let root = dir.join("Extensions").join(A);
        assert_eq!(
            manifest_path(&dir, A, "1.2"),
            Some(root.join("1.2_0/manifest.json"))
        );
        assert_eq!(
            manifest_path(&dir, A, "3.0"),
            Some(root.join("1.10.0_0/manifest.json"))
        );
        assert_eq!(
            manifest_path(&dir, A, ""),
            Some(root.join("1.10.0_0/manifest.json"))
        );
        assert_eq!(manifest_path(&dir, B, "1"), None);
        assert_eq!(manifest_path(&dir, "../x", "1"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn extension(id: &str, version: &str) -> Extension {
        Extension {
            id: id.to_owned(),
            name: id.to_owned(),
            version: version.to_owned(),
            enabled: true,
            ..Extension::default()
        }
    }

    #[test]
    fn a_state_is_enriched_from_the_profiles_files() {
        let config = scratch("enrich");
        let profile = config.join("google-chrome").join("Default");
        let manifest = profile.join("Extensions").join(A).join("2.0_0");
        std::fs::create_dir_all(&manifest).expect("dir");
        std::fs::write(
            manifest.join("manifest.json"),
            r#"{"action":{"default_popup":"pop.html"}}"#,
        )
        .expect("write");
        // Not in `Preferences`: the list is found in the second file.
        std::fs::write(profile.join("Preferences"), r#"{"extensions":{}}"#).expect("write");
        std::fs::write(
            profile.join("Secure Preferences"),
            format!(r#"{{"extensions":{{"pinned_extensions":["{C}","{A}"]}}}}"#),
        )
        .expect("write");
        let mut state = State {
            v: crate::wire::VERSION,
            profile: Some("Default".into()),
            extensions: vec![
                extension(A, "2.0"),
                extension(B, "1.0"),
                extension(C, "1.0"),
            ],
            ..State::default()
        };
        let mut reader = Reader::default();
        reader.enrich(&mut state, &config);
        assert_eq!(state.extensions[0].popup.as_deref(), Some("pop.html"));
        assert_eq!(state.extensions[0].pinned_at, Some(1));
        assert_eq!(state.extensions[1].popup, None, "no manifest, no popup");
        assert_eq!(state.extensions[1].pinned_at, None);
        assert_eq!(state.extensions[2].pinned_at, Some(0));
        // A profile that is not one fills nothing.
        let mut odd = State {
            profile: Some("../..".into()),
            extensions: vec![extension(A, "2.0")],
            ..State::default()
        };
        reader.enrich(&mut odd, &config);
        assert_eq!(odd.extensions[0].popup, None);
        let _ = std::fs::remove_dir_all(&config);
    }

    #[test]
    fn a_file_past_its_ceiling_is_not_read() {
        let dir = scratch("ceiling");
        let file = dir.join("big");
        std::fs::write(&file, vec![b' '; 2048]).expect("write");
        assert!(bounded(&file, 1024).is_none());
        assert_eq!(bounded(&file, 4096).map(|t| t.len()), Some(2048));
        assert!(bounded(&dir.join("missing"), 1024).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
