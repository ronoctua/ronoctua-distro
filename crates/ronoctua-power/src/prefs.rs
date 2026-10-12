//! `power.json` — the profile the person chose, and the floor under it.
//!
//! The same three-step resolution `theme.rs` gives the theme, and for the same
//! reasons: the file wins because it is the person's own word, the
//! environment (`RONOCTUA_POWER_PROFILE`, set by `nix/modules/power.nix`) is the
//! distro's floor, and `performance` is the ground — the owner's decision for
//! this distro, written once in the module's option and once here.
//!
//! PURE where it can be: `resolve_in` takes the directory and the variable as
//! parameters, so a test never reads the real home of whoever runs it.

use std::path::Path;

use crate::Profile;

/// The file, under `$XDG_CONFIG_HOME/ronoctua`.
pub const FILE: &str = "power.json";

/// The variable the module sets — the distro's floor.
pub const ENV: &str = "RONOCTUA_POWER_PROFILE";

/// Where the answer came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `power.json` named it.
    File,
    /// `RONOCTUA_POWER_PROFILE` named it.
    Env,
    /// Neither did: `performance`, the distro's ground.
    Default,
}

/// The profile the sources name, where it came from, and what was wrong with
/// the sources that were passed over.
#[must_use]
pub fn resolve_in(dir: Option<&Path>, env: Option<&str>) -> (Profile, Source, Vec<String>) {
    let mut complaints = Vec::new();
    if let Some(dir) = dir {
        match read_choice(dir) {
            Ok(None) => {}
            Ok(Some(p)) => return (p, Source::File, complaints),
            Err(e) => complaints.push(format!("{FILE}: {e}")),
        }
    }
    if let Some(id) = env {
        match Profile::by_id(id) {
            Some(p) => return (p, Source::Env, complaints),
            None => complaints.push(format!("{ENV} names an unknown profile `{id}`")),
        }
    }
    (Profile::Performance, Source::Default, complaints)
}

/// `power.json` as it is written: `{ "profile": "balanced" }`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Written {
    /// The profile's id; absent is "nobody chose".
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<String>,
}

/// The file as a [`ronoctua_store::Store`] in `dir`, judged by [`refused`]
/// on every read and write.
fn store_at(dir: &Path) -> ronoctua_store::Store<Written> {
    ronoctua_store::Store::at(Some(dir.to_path_buf()), FILE).checked(refused)
}

/// The profile ids this system knows — an id it does not is REPAIRED (out of
/// the value read, into the ledger), never taken for a profile.
fn refused(w: &Written) -> Vec<ronoctua_store::Refused> {
    match &w.profile {
        Some(id) if Profile::by_id(id).is_none() => vec![ronoctua_store::Refused {
            pointer: "/profile".to_owned(),
            why: ronoctua_store::repair::Why::UnknownVariant,
            kept: None,
        }],
        _ => Vec::new(),
    }
}

/// The file's one field, looked up — `power.json` in `dir`.
///
/// # Errors
///
/// The file is not JSON, has no `profile`, or names a profile that does not
/// exist (or one that is not a word). A MISSING file is `Ok(None)`: nobody
/// chose yet.
pub fn read_choice(dir: &Path) -> Result<Option<Profile>, String> {
    let reading = store_at(dir).reading();
    if !reading.found {
        return Ok(None);
    }
    if let Some(trouble) = reading.trouble {
        return Err(trouble.to_string());
    }
    if let Some(repair) = reading.repairs.iter().find(|r| r.pointer == "/profile") {
        let said = match &repair.found {
            ronoctua_store::Node::String(s) => s.clone(),
            other => other.to_string(),
        };
        return Err(format!("unknown profile `{said}`"));
    }
    let id = reading
        .value
        .profile
        .ok_or_else(|| "no `profile` field".to_owned())?;
    Profile::by_id(&id)
        .map(Some)
        .ok_or_else(|| format!("unknown profile `{id}`"))
}

/// Writes the choice, atomically — keeping what the file holds that this
/// machine does not know.
///
/// # Errors
///
/// The directory cannot be written.
pub fn persist(dir: &Path, profile: Profile) -> std::io::Result<()> {
    store_at(dir).write(&Written {
        profile: Some(profile.id().to_owned()),
    })
}

/// The directory the file lives in — `$XDG_CONFIG_HOME/ronoctua`.
#[must_use]
pub fn home() -> Option<std::path::PathBuf> {
    ronoctua_xdg::config_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-power-prefs-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_file_wins_then_the_environment_then_performance() {
        let dir = scratch("order");
        assert_eq!(
            resolve_in(Some(&dir), None),
            (Profile::Performance, Source::Default, vec![])
        );
        assert_eq!(
            resolve_in(Some(&dir), Some("balanced")).0,
            Profile::Balanced
        );
        persist(&dir, Profile::PowerSaver).unwrap();
        let (p, from, complaints) = resolve_in(Some(&dir), Some("balanced"));
        assert_eq!((p, from), (Profile::PowerSaver, Source::File));
        assert!(complaints.is_empty());
    }

    #[test]
    fn a_wrong_file_is_named_and_passed_over() {
        let dir = scratch("wrong");
        std::fs::write(dir.join(FILE), "{\"profile\": \"turbo\"}").unwrap();
        let (p, from, complaints) = resolve_in(Some(&dir), Some("nonsense"));
        assert_eq!((p, from), (Profile::Performance, Source::Default));
        assert_eq!(complaints.len(), 2, "{complaints:?}");
        assert!(complaints[0].contains("turbo"));
        assert!(complaints[1].contains("nonsense"));
    }
}
