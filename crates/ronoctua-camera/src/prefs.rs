//! What a person chose about their cameras: `camera.json`.
//!
//! Per camera, by [`Identity`](crate::Identity) — never by node — the mode,
//! the mirror and every control a person moved. Put back on every plug
//! (`crate::restore`), because a UVC camera forgets its controls when it loses
//! power: this is the only memory it has.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Identity, Mode};

/// The file's name.
pub const FILE: &str = "camera.json";

/// One camera's choices.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerCamera {
    /// The mode it opens in; `None` is "choose for me" (`crate::choose_mode`).
    pub mode: Option<Mode>,
    /// Shown as a mirror — what a person expects of their own face. A capture
    /// and the virtual camera follow the same choice, so what is seen is what
    /// is sent.
    pub mirror: bool,
    /// Every control moved away from the factory, by key.
    pub controls: BTreeMap<String, i32>,
}

/// The whole file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// The camera chosen as the default; `None` is "the first colour one".
    pub default: Option<Identity>,
    /// Per camera, by identity.
    pub cameras: BTreeMap<String, PerCamera>,
}

impl Preferences {
    /// One camera's choices, or the empty default.
    pub fn of(&self, id: &Identity) -> PerCamera {
        self.cameras
            .get(&id.0)
            .cloned()
            .unwrap_or_else(|| PerCamera {
                mirror: true,
                ..PerCamera::default()
            })
    }

    /// Replaces one camera's choices; an entry that says nothing is REMOVED,
    /// so the file does not grow a line for every camera ever plugged in.
    pub fn set(&mut self, id: &Identity, per: PerCamera) {
        if per
            == (PerCamera {
                mirror: true,
                ..PerCamera::default()
            })
        {
            self.cameras.remove(&id.0);
        } else {
            self.cameras.insert(id.0.clone(), per);
        }
    }
}

/// The directory the file lives in.
#[must_use]
pub fn home() -> Option<std::path::PathBuf> {
    ronoctua_store::config_dir()
}

/// The store.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(home(), FILE)
}

/// Writes the record back, and REFUSES over a file that will not parse (the
/// rule `ronoctua_audio::prefs::keep` states: a reader degrades, a writer
/// must refuse).
pub fn keep(store: &ronoctua_store::Store<Preferences>, prefs: &Preferences) -> anyhow::Result<()> {
    if store.parsed().is_none() {
        anyhow::bail!("{FILE} will not parse, and writing would replace it");
    }
    store.write(prefs)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_camera_nobody_touched_mirrors_and_is_not_written() {
        let mut p = Preferences::default();
        let id = Identity::of("c", "b", "i");
        assert!(p.of(&id).mirror, "a person expects their own face mirrored");
        p.set(&id, p.of(&id));
        assert!(p.cameras.is_empty());
        let mut per = p.of(&id);
        per.controls.insert("brightness".into(), 140);
        p.set(&id, per);
        assert_eq!(p.cameras.len(), 1);
    }

    #[test]
    fn a_writer_refuses_over_a_file_that_will_not_parse() {
        let dir = std::env::temp_dir().join(format!("ronoctua-camera-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join(FILE), "{ not json").expect("write");
        let store = ronoctua_store::Store::<Preferences>::at(Some(dir.clone()), FILE);
        assert!(keep(&store, &Preferences::default()).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.join(FILE)).expect("read"),
            "{ not json"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
