//! What the daemon reads: every unit's remaps and leader key.
//!
//! # Why a file of its own, and not `keyboards.json`
//!
//! The daemon is root and the preferences are the person's, under their home.
//! A root daemon reading a home directory is a daemon that follows whatever a
//! user left there; so the shell RENDERS what the daemon needs into a
//! directory the machine gives it (`/var/lib/ronoctua/keys`, the keyd
//! arrangement: made by `systemd-tmpfiles`, owned by the session's user), and
//! the daemon reads only that.
//!
//! A unit the file does not name — plugged in after the last render — still
//! works: it takes its MODEL's remaps and the distro's leader key, which is
//! what Together mode would have given it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where the rendered file lives.
pub const DIRECTORY: &str = "/var/lib/ronoctua/keys";

/// Its name.
pub const FILE: &str = "keys.json";

/// One unit's rules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    /// The key whose tap is the leader.
    pub leader: String,
    /// keyd's key name, to keyd's action.
    pub keys: BTreeMap<String, String>,
}

/// Everything the daemon reads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// By [`ronoctua_keyboards::Unit::identity`].
    pub units: BTreeMap<String, Rules>,
    /// By `vendor:product` — the Together remaps, for a unit not named above.
    pub models: BTreeMap<String, BTreeMap<String, String>>,
}

impl Config {
    /// The rules for a unit.
    #[must_use]
    pub fn rules_of(&self, unit: &ronoctua_keyboards::Unit) -> Rules {
        self.units
            .get(&unit.identity())
            .cloned()
            .unwrap_or_else(|| Rules {
                leader: ronoctua_keyboards::LEADER.0.to_owned(),
                keys: self.models.get(&unit.id).cloned().unwrap_or_default(),
            })
    }
}

/// The file for these preferences and these units.
#[must_use]
pub fn render(
    prefs: &ronoctua_keyboards::Preferences,
    units: &[ronoctua_keyboards::Unit],
) -> Config {
    let units = units
        .iter()
        .map(|u| {
            let identity = u.identity();
            let rules = Rules {
                leader: prefs.unit_leader(&identity),
                keys: prefs.unit_keys(u),
            };
            (identity, rules)
        })
        .collect();
    let models = prefs
        .boards
        .iter()
        .map(|(id, b)| (id.clone(), b.keys.clone()))
        .collect();
    Config { units, models }
}

/// The file as a [`ronoctua_store::Store`] in `directory`.
///
/// Not under the person's roots (the machine gives the daemon its own
/// directory), so it keeps no repair ledger: a repair is said in the journal.
fn store_in(directory: &str) -> ronoctua_store::Store<Config> {
    ronoctua_store::Store::at(Some(std::path::PathBuf::from(directory)), FILE)
}

/// Reads the file; a missing one is every unit with the distro's leader and no
/// remaps, which is a keyboard that works — and a BROKEN one keeps every unit
/// that still reads: one unit's rules that will not parse cost that unit.
#[must_use]
pub fn read(directory: &str) -> Config {
    store_in(directory).read()
}

/// Writes the file, atomically, keeping what it holds that this build does
/// not know.
///
/// # Errors
///
/// The directory is not there or not ours.
pub fn write(directory: &str, config: &Config) -> anyhow::Result<()> {
    store_in(directory)
        .write(config)
        .map_err(|e| anyhow::anyhow!("writing {directory}/{FILE}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: &str, phys: &str) -> ronoctua_keyboards::Unit {
        ronoctua_keyboards::Unit {
            id: id.to_owned(),
            name: "Keyboard".to_owned(),
            built_in: false,
            phys: phys.to_owned(),
            uniq: String::new(),
            event: "event9".to_owned(),
        }
    }

    #[test]
    fn a_unit_nobody_named_takes_its_models_remaps() {
        let mut prefs = ronoctua_keyboards::Preferences::default();
        prefs.set("046d:c31c", "capslock", "esc");
        let known = unit("046d:c31c", "usb-1");
        let config = render(&prefs, std::slice::from_ref(&known));
        let later = unit("046d:c31c", "usb-2");
        let rules = config.rules_of(&later);
        assert_eq!(rules.keys.get("capslock").map(String::as_str), Some("esc"));
        assert_eq!(rules.leader, ronoctua_keyboards::LEADER.0);
    }

    #[test]
    fn a_unit_with_its_own_rules_keeps_them() {
        let mut prefs = ronoctua_keyboards::Preferences::default();
        prefs.set("046d:c31c", "capslock", "esc");
        let u = unit("046d:c31c", "usb-1");
        prefs.units.insert(
            u.identity(),
            ronoctua_keyboards::UnitPrefs {
                keys: Some(BTreeMap::new()),
                leader: Some("rightalt".to_owned()),
                ..Default::default()
            },
        );
        let rules = render(&prefs, std::slice::from_ref(&u)).rules_of(&u);
        assert!(rules.keys.is_empty(), "its own, empty, beats the model's");
        assert_eq!(rules.leader, "rightalt");
    }
}
