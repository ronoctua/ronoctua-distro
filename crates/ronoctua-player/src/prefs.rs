//! What a person chose about the player: `$XDG_CONFIG_HOME/ronoctua/player.json`.
//!
//! Every door writes it and the daemon rereads it on `Ask::Reload` — the file
//! is the broadcast, the rule every preference in this system follows.
//!
//! # Both network switches are born OFF
//!
//! Lyrics from the internet send what is playing to `lrclib.net`, and skipping
//! sponsored segments sends which video is playing to `sponsor.ajay.app`.
//! Neither is a thing a machine should start doing on its own; the view names
//! the host beside each switch (the glance's sources have the same rule).

use serde::{Deserialize, Serialize};

/// The file's name.
pub const FILE: &str = "player.json";

/// The host lyrics are asked of.
pub const LRCLIB_HOST: &str = "lrclib.net";

/// The host sponsored segments are asked of.
pub const SPONSORBLOCK_HOST: &str = "sponsor.ajay.app";

/// The segments skipped when nobody chose: the ones that are not the video.
pub const SKIPPED_BY_DEFAULT: &[&str] = &["sponsor", "selfpromo", "interaction"];

/// The preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Ask `lrclib.net` for lyrics when a song has none of its own.
    pub lyrics_online: bool,
    /// Skip what SponsorBlock marks inside a YouTube video.
    pub sponsorblock: bool,
    /// Which of its categories are skipped.
    pub skipped: Vec<String>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            lyrics_online: false,
            sponsorblock: false,
            skipped: SKIPPED_BY_DEFAULT.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
}

/// The store.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_reaches_the_network_until_somebody_says_so() {
        let p = Preferences::default();
        assert!(!p.lyrics_online && !p.sponsorblock);
        let parsed: Preferences = serde_json::from_str("{}").expect("an empty file parses");
        assert_eq!(parsed, p);
    }
}
