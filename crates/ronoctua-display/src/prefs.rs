//! What somebody chose, kept between sessions — `display.json`.
//!
//! One file under `$XDG_CONFIG_HOME/ronoctua/`, written through
//! [`ronoctua_store::Store`], which makes it atomic and makes a file that
//! will not parse cost this one feature instead of the desktop.
//!
//! # Why this is not the same file the compositor reads
//!
//! There are two, and they hold different things.
//! [`crate::fragment`] writes `output` blocks for the compositor, and the
//! compositor is the only reader; this one is ours, and holds the same
//! decisions plus the ones the compositor knows nothing about — a monitor's own
//! brightness, for one. Writing only the fragment would mean parsing the
//! compositor's config grammar to read our own preferences back, which is that
//! grammar leaking into a crate that has no business knowing it.
//!
//! Keyed by [`ronoctua_compositor::Output::naming`] — the monitor's description
//! when that can only be this monitor, and the CONNECTOR when it cannot. A
//! monitor with a serial keeps its settings across a different port and
//! across a week in a drawer; one without a serial keeps them per port,
//! because every other unit of its model would answer to the same
//! description. Which of the two it is, is on the screen.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::fragment::Wanted;

/// The file's name, under `$XDG_CONFIG_HOME/ronoctua/`.
pub const FILE: &str = "display.json";

/// Everything somebody chose about the screens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// What each monitor was told, by the name it is filed under.
    pub screens: BTreeMap<String, Wanted>,
    /// How big the text is, in percent of the design's base size.
    ///
    /// It is HERE and not per monitor, and that is a decision rather than a
    /// simplification: the shell draws one size across every screen, because
    /// the footer's height is the same on all of them — a break that gave two
    /// lines on the laptop and one on the monitor would reflow every buffer
    /// when the focus moved between them, which `docs/ARCHITECTURE.md`
    /// records as already decided. A per-screen size would be a per-screen
    /// footer height.
    ///
    /// `ronoctua_tokens::text::allowed` holds it inside the declared range
    /// and snaps it to a step the arrows can reach, so a hand-edited number
    /// lands somewhere a key can leave.
    #[serde(default = "one_hundred")]
    pub text: u16,
}

/// What a machine that has never been told draws at.
fn one_hundred() -> u16 {
    ronoctua_tokens::text::NORMAL
}

// `Default` BY HAND, and a test is what asked for it. Deriving it gives
// `u16::default()` for the size — which is ZERO, and the whole shell drawing
// at nothing. The `#[serde(default)]` attribute only covers a field MISSING
// from a file that exists; `Store::read` hands back `T::default()` for a file
// that is not there at all, which is every machine on its first morning.
impl Default for Preferences {
    fn default() -> Self {
        Self {
            screens: BTreeMap::new(),
            text: one_hundred(),
        }
    }
}

// THERE IS NO BRIGHTNESS HERE, and there was: a map that was written on every
// change and read by nothing. Its comment claimed a machine waking from
// suspend had to put the value back, which is not true of either kind of
// screen — the kernel restores a panel's `brightness` across a resume, and a
// monitor keeps its own in its own non-volatile memory. A preference nothing
// restores is a file that grows and a promise that is not kept.

impl Preferences {
    /// What was chosen for one monitor — the default when nothing was.
    #[must_use]
    pub fn of(&self, filed_as: &str) -> Wanted {
        self.screens.get(filed_as).cloned().unwrap_or_default()
    }

    /// Records a choice, and forgets a monitor that was returned to defaults.
    ///
    /// Forgetting matters: a map that only grows turns into a file listing
    /// every monitor ever plugged into this machine, each saying nothing.
    pub fn set(&mut self, filed_as: &str, wanted: Wanted) {
        if wanted.says_nothing() {
            self.screens.remove(filed_as);
        } else {
            self.screens.insert(filed_as.to_owned(), wanted);
        }
    }
}

/// The store this lives in. `None` when there is no config directory at all.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::{Spot, Wanted};

    #[test]
    fn a_machine_with_no_file_at_all_draws_at_a_hundred_percent() {
        // `Store::read` answers `T::default()` for a file that is not there,
        // which is every machine on its first morning. A derived `Default`
        // gives `u16::default()` for the size — zero — and the shell draws at
        // nothing. The serde attribute does not cover this: it is only for a
        // field missing from a file that exists.
        assert_eq!(Preferences::default().text, ronoctua_tokens::text::NORMAL);
        assert!(Preferences::default().screens.is_empty());
    }

    #[test]
    fn a_monitor_returned_to_its_defaults_is_forgotten() {
        // A map that only grows becomes a file listing every monitor ever
        // plugged into the machine, each of them saying nothing.
        let mut p = Preferences::default();
        p.set(
            "Other Model-2 ABC123",
            Wanted {
                position: Some(Spot { x: 1920, y: 0 }),
                ..Wanted::default()
            },
        );
        assert_eq!(p.screens.len(), 1);
        p.set("Other Model-2 ABC123", Wanted::default());
        assert!(p.screens.is_empty());
    }

    #[test]
    fn an_unknown_monitor_answers_with_the_defaults() {
        let p = Preferences::default();
        assert!(p.of("nobody").says_nothing());
    }

    #[test]
    fn a_file_from_an_older_version_still_reads() {
        // `#[serde(default)]` on the struct AND on the fields: a preference
        // file written before a field existed has to read as "nothing was
        // chosen", not as a parse failure that loses every other setting.
        let old: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(old, Preferences::default());
        let half: Preferences =
            serde_json::from_str(r#"{"screens":{"A B C":{"vary":true}}}"#).unwrap();
        assert!(half.of("A B C").vary);
        // AND A FILE FROM A NEWER ONE. `display.json` used to carry a
        // `brightness` map; a machine that downgrades must not lose its
        // screens over a key this version does not know.
        let newer: Preferences =
            serde_json::from_str(r#"{"screens":{},"brightness":{"A B C":50}}"#).unwrap();
        assert_eq!(newer, Preferences::default());
        // AND THE SIZE OF THE TEXT DEFAULTS TO A HUNDRED, not to zero. A
        // `#[serde(default)]` on the struct would give `u16::default()`,
        // which is 0 — a file written before this field existed would make
        // the whole shell draw at nothing.
        assert_eq!(old.text, ronoctua_tokens::text::NORMAL);
        assert_eq!(half.text, ronoctua_tokens::text::NORMAL);
    }

    #[test]
    fn the_whole_thing_survives_the_disk() {
        let mut p = Preferences::default();
        p.set(
            "Maker Model-1 Unknown",
            Wanted {
                scale: Some(1.5),
                ..Wanted::default()
            },
        );
        let text = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Preferences>(&text).unwrap(), p);
    }
}
