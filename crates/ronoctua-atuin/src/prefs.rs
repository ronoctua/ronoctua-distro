//! The one thing about Atuin that is OURS rather than Atuin's.
//!
//! Everything else it keeps itself, in `config.toml` — and this crate edits
//! that file rather than shadowing it, because a second copy of a setting is a
//! second answer the day one of them changes. What is here is the modeline's
//! mark, which Atuin has no opinion about.

use serde::{Deserialize, Serialize};

/// What the file is called.
pub const FILE: &str = "atuin.json";

/// What is remembered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Preferences {
    /// Which modeline mark to draw, if any.
    pub mark: Mark,
}

/// When the modeline says something about the shell's history.
///
/// The same ring the Syncthing view carries, and the same default: `WhenOff`
/// keeps the modeline silent while everything is working, which is the rule of
/// the house.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mark {
    /// Never draw one.
    Never,
    /// Draw one when the history is NOT syncing.
    #[default]
    WhenOff,
    /// Draw one while it IS.
    WhenOn,
}

impl Mark {
    /// The id, which is the stem of the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::WhenOff => "when-off",
            Self::WhenOn => "when-on",
        }
    }

    /// The ring, in the order it turns.
    pub const ALL: [Self; 3] = [Self::Never, Self::WhenOff, Self::WhenOn];

    /// Should a mark be drawn?
    ///
    /// A MACHINE THAT NEVER LOGGED IN LIGHTS NOTHING, whatever the ring says.
    /// It is the Syncthing mark's own rule, in its words: a mark that is lit on
    /// the first boot does not mean anything by the second day. `WhenOff` is
    /// about a sync that STOPPED, not about one that was never asked for.
    #[must_use]
    pub const fn shows(self, configured: bool, syncing: bool) -> bool {
        if !configured {
            return false;
        }
        match self {
            Self::Never => false,
            Self::WhenOff => !syncing,
            Self::WhenOn => syncing,
        }
    }
}

/// Where it lives.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

/// What is on disk, or the default.
#[must_use]
pub fn read() -> Preferences {
    store().read()
}

/// Writes them back.
///
/// # Errors
///
/// The file could not be written, or the one on disk will not parse — in which
/// case it is LEFT ALONE. The default is the right answer for a reader and
/// destruction for a writer.
pub fn write(prefs: &Preferences) -> std::io::Result<()> {
    let current = store();
    if current.path().is_some_and(|p| p.exists()) && current.parsed().is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{FILE} does not parse; it was left alone"),
        ));
    }
    current.write(prefs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modeline_is_silent_while_the_history_is_syncing() {
        assert_eq!(Mark::default(), Mark::WhenOff);
        assert!(!Mark::default().shows(true, true));
        assert!(Mark::default().shows(true, false));
        for mark in Mark::ALL {
            assert!(
                !mark.shows(false, false),
                "{mark:?} lit with nothing set up"
            );
        }
        let ids: std::collections::BTreeSet<_> = Mark::ALL.iter().map(|m| m.id()).collect();
        assert_eq!(ids.len(), Mark::ALL.len(), "two values share one word");
    }

    #[test]
    fn atuins_own_settings_are_not_shadowed_here() {
        // A SECOND COPY OF A SETTING IS A SECOND ANSWER the day one of them
        // changes. `config.toml` is Atuin's, and this crate edits it in place.
        let fresh = Preferences::default();
        assert_eq!(
            fresh,
            Preferences {
                mark: Mark::WhenOff
            }
        );
    }
}
