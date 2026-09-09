//! Where what was fetched is kept.
//!
//! STATE, not configuration: `$XDG_STATE_HOME/ronoctua/world.json` holds what
//! happened — a temperature, a rate and when they arrived. What the person
//! CHOSE (which places, which of the four things each shows) is configuration
//! and lives under `$XDG_CONFIG_HOME`, in the file `LEADER s l` writes.
//!
//! The split is the one `ronoctua-xdg` states: *"CONFIGURATION is what the
//! person chose; STATE is what happened."* Getting it backwards would put a
//! cached temperature in the directory that is meant to survive a reinstall.

use crate::Cache;

/// The file, under `$XDG_STATE_HOME/ronoctua`.
///
/// `world-cache.json` and NOT `world.json`, which is what it was called: the
/// person's own places live in `world.json` under `$XDG_CONFIG_HOME`, and two
/// files of one name in two directories is a trap that had already sprung —
/// the footer watched this one believing it was watching that one, so every
/// switch flipped in the window wrote a file nobody was listening to. The
/// name says which is which now.
pub const FILE: &str = "world-cache.json";

/// The store this lives in. `None` when there is no state directory at all.
#[must_use]
pub fn store() -> ronoctua_store::Store<Cache> {
    ronoctua_store::Store::at(ronoctua_store::state_dir(), FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// It is STATE and not configuration, and the two directories differ.
    ///
    /// THE DEFECT THIS CATCHES: a cached temperature written where the
    /// person's own choices live — a directory meant to be backed up and
    /// carried to another machine, filling with weather.
    #[test]
    fn the_cache_lives_in_the_state_directory() {
        let state = ronoctua_store::state_dir();
        let config = ronoctua_store::config_dir();
        if let (Some(state), Some(config)) = (state, config) {
            assert_ne!(state, config, "state and configuration are the same path");
            let path = store().path().expect("a path when there is a directory");
            assert!(path.starts_with(&state), "{path:?} is not under {state:?}");
        }
    }
}
