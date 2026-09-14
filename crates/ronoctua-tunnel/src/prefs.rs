//! The one Tor decision that is OURS rather than Tor's.
//!
//! # Why autostart is a preference and not `systemctl enable`
//!
//! Because `enable` is not in the polkit rule and must not be. The rule this
//! system installs is `manage-units` with the verbs `start` and `stop`, one
//! unit, one user — and `tor-polkit-is-narrow` refuses a third verb, naming
//! `enable` in particular, because a unit enabled by a key press survives a
//! reboot. That is a different kind of power from starting something now, and
//! widening the door to get it would be paying for ever for a convenience.
//!
//! So the footer reads this file when the session comes up and sends
//! `Order::Tor(true)` itself. It is the same shape `ronoctua.power.profile`
//! already has: the flake declares a floor, the file is the person's, and
//! nothing needs a rebuild to change its mind.
//!
//! # Why it is not in the torrc fragment
//!
//! Because it is not a torrc option. Tor has no idea whether it was asked for
//! at login, and a file the daemon reads is the wrong place for a fact about
//! the session.

use serde::{Deserialize, Serialize};

/// What the file is called.
pub const FILE: &str = "tor.json";

/// The preferences.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Preferences {
    /// Start Tor when the session comes up.
    ///
    /// OFF BY DEFAULT, which is the owner's decision written in
    /// `nix/modules/tor.nix`: a person who has not asked for Tor is not using
    /// Tor, and a daemon building circuits nobody wants is battery spent on
    /// nothing.
    pub autostart: bool,
}

/// Where the file lives.
///
/// `$XDG_CONFIG_HOME/ronoctua`, because it is a CHOICE and not something that
/// happened — the same root the theme and the locale use.
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
/// The file could not be written — a read-only home, a full disk.
pub fn write(prefs: &Preferences) -> std::io::Result<()> {
    // `parsed`, NOT `read`. The default is the right answer for a reader and
    // destruction for a writer: a `tor.json` that will not parse would be
    // overwritten here with the default, and whatever was in it would be gone.
    // `ronoctua_store::Store` says the same thing in its own doc, and a
    // `display.json` is what taught it.
    let current = store();
    if current.path().is_some() && current.parsed().is_none() && read_exists(&current) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{FILE} does not parse; it was left alone"),
        ));
    }
    current.write(prefs)
}

/// Is there a file there at all?
///
/// A store with nothing in it parses as nothing and is the ordinary state of a
/// fresh machine — which must not be mistaken for a file that is corrupt.
fn read_exists(store: &ronoctua_store::Store<Preferences>) -> bool {
    store.path().is_some_and(|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tor_does_not_come_up_by_itself() {
        // THE OWNER'S DECISION, and the module says it in words: a person who
        // has not asked for Tor is not using Tor. A default that started it
        // would be a daemon building circuits from every boot.
        assert!(!Preferences::default().autostart);
    }

    #[test]
    fn the_file_is_named_once() {
        assert_eq!(FILE, "tor.json");
    }
}
