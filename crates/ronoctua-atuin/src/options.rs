//! Every Atuin setting this system offers, and what a key does to one.
//!
//! # Where the table comes from
//!
//! `assets/atuin-options.nix`, through `nix/scripts/gen-options.py` — the same
//! generator Tor's and Syncthing's tables come out of. Nothing here is
//! hand-written: it refuses a duplicate id, a number with no range and a ring
//! that cannot turn, and it writes the text keys both languages owe alongside.
//!
//! # A VALUE HERE CARRIES ITS QUOTES
//!
//! And that is the one thing this module differs from Tor's about. A torrc
//! line is `UseBridges 1` — a name and a string, with no types. TOML says
//! `search_mode = "fuzzy"` and `inline_height = 20`, and the quotes are part
//! of the syntax: writing `search_mode = fuzzy` makes the file unparseable and
//! Atuin then falls back to its defaults for EVERYTHING, silently.
//!
//! So [`Setting::written`] puts them on and [`Setting::plain`] takes them off,
//! and neither is guesswork — the declared `kind` says which a setting is.

use ronoctua_options::{Kind, Spelling};

pub use crate::config::at;

/// What the file says, by `section.key`.
pub type Lines = std::collections::BTreeMap<String, String>;

/// Which part of the sheet a setting belongs to.
///
/// The order is the order the screen draws in, and it is declared in
/// `flake.nix` rather than read off the table: `builtins.toJSON` sorts an
/// attribute set alphabetically, and a sheet's shape is an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// What leaves this machine — the section this feature turns on.
    Leaving,
    /// What `Ctrl+R` does.
    Search,
    /// How the search behaves under the fingers.
    Keys,
    /// Real settings, rarely touched. Drawn inside the drawer.
    Advanced,
}

/// One declared setting.
#[derive(Debug, Clone, Copy)]
pub struct Setting {
    /// Kebab-case, and the stem of [`Setting::label`].
    pub id: &'static str,
    /// The text key for the row's name.
    pub label: &'static str,
    /// The TOML key, spelled as Atuin spells it.
    pub key: &'static str,
    /// The section it lives under, when it lives under one.
    pub within: Option<&'static str>,
    /// Which part of the sheet it is drawn in.
    pub scope: Scope,
    /// What it is, which is what decides how a key turns it.
    pub kind: Kind,
    /// The flake declares this one. Drawn, reachable, refusing.
    pub fixed: bool,
    /// It lives in the drawer, closed.
    pub advanced: bool,
}

include!(concat!(env!("OUT_DIR"), "/options.rs"));

/// How TOML spells a boolean.
pub const TRUTH: Spelling = Spelling::WORDS;

/// Every setting of one scope, in declared order.
pub fn of_scope(scope: Scope) -> impl Iterator<Item = &'static Setting> {
    SETTINGS.iter().filter(move |s| s.scope == scope)
}

/// One setting by its id.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.id == id)
}

impl Setting {
    /// Where this setting sits in the file.
    #[must_use]
    pub fn place(&self) -> String {
        at(self.within, self.key)
    }

    /// What the file holds for it, exactly as written — quotes and all.
    #[must_use]
    pub fn raw<'a>(&self, lines: &'a Lines) -> Option<&'a str> {
        lines.get(&self.place()).map(String::as_str)
    }

    /// The same, with TOML's quotes taken off.
    ///
    /// A STRING IS THE ONLY KIND THAT HAS THEM, so this does not strip a quote
    /// somebody actually typed inside a number — and a `Text` whose value a
    /// person hand-wrote without quotes still reads back, because trimming a
    /// quote that is not there is not an error.
    #[must_use]
    pub fn plain<'a>(&self, lines: &'a Lines) -> Option<&'a str> {
        let raw = self.raw(lines)?.trim();
        Some(raw.trim_matches('"'))
    }

    /// Whether a switch is on.
    ///
    /// ABSENT MEANS ATUIN'S OWN DEFAULT, which this cannot know per setting —
    /// so it answers `false` and the VIEW says "not set" rather than "off".
    /// The one place it matters, `auto_sync`, is read in
    /// [`crate::look`] against Atuin's documented default instead.
    #[must_use]
    pub fn is_on(&self, lines: &Lines) -> bool {
        self.plain(lines).is_some_and(|v| TRUTH.reads_yes(v))
    }

    /// Is this setting written down at all?
    ///
    /// The view draws "not set" rather than a guess: Atuin's defaults differ
    /// per setting and are not in the file, so a screen that showed one would
    /// be stating something this system never wrote.
    #[must_use]
    pub fn written(&self, lines: &Lines) -> bool {
        self.raw(lines).is_some()
    }

    /// The value a key press would leave behind, in TOML's spelling.
    ///
    /// `None` for the kinds that are typed rather than turned.
    #[must_use]
    pub fn turned(&self, lines: &Lines, forwards: bool) -> Option<String> {
        match self.kind {
            Kind::Switch => Some(TRUTH.of(!self.is_on(lines)).to_owned()),
            // A RING'S WORD IS A STRING, and the quotes are not decoration:
            // `search_mode = fuzzy` makes the whole file unparseable, and
            // Atuin then falls back to its defaults for EVERYTHING, silently.
            Kind::Ring(ring) => {
                let now = self.plain(lines).unwrap_or("");
                Some(format!(
                    "\"{}\"",
                    ronoctua_options::turn(ring, now, forwards)
                ))
            }
            Kind::Number { least, most, .. } => {
                let now = self
                    .plain(lines)
                    .and_then(|v| v.trim().parse::<i64>().ok())
                    .unwrap_or(least);
                Some(ronoctua_options::nudge(now, least, most, forwards).to_string())
            }
            Kind::Text | Kind::List | Kind::Amount => None,
        }
    }

    /// One typed value, in TOML's spelling.
    ///
    /// A LIST STAYS AS TYPED. `history_filter` is `["^ ", "^secret"]`, and a
    /// person who pastes that has written TOML already — quoting it again
    /// would make the whole list one string, and the filter would then match
    /// nothing while appearing to be set.
    #[must_use]
    pub fn typed(&self, said: &str) -> String {
        let said = said.trim();
        if said.is_empty() {
            return String::new();
        }
        if said.starts_with('[') || said.starts_with('"') {
            return said.to_owned();
        }
        match self.kind {
            Kind::Number { .. } | Kind::Switch => said.to_owned(),
            _ => format!("\"{said}\""),
        }
    }

    /// The text key for a ring's current word.
    #[must_use]
    pub fn word(&self, lines: &Lines) -> Option<String> {
        match self.kind {
            Kind::Ring(_) => Some(format!(
                "atuin-word-{}",
                ronoctua_options::word_key(self.plain(lines).unwrap_or(""))
            )),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_of(pairs: &[(&str, &str)]) -> Lines {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn the_table_arrived_and_covers_every_scope() {
        assert!(SETTINGS.len() > 30, "the generated table is too small");
        for scope in [Scope::Leaving, Scope::Search, Scope::Keys, Scope::Advanced] {
            assert!(
                of_scope(scope).next().is_some(),
                "{scope:?} has no rows, so a section would be a heading over nothing"
            );
        }
    }

    #[test]
    fn every_id_is_unique_and_so_is_every_place_in_the_file() {
        let ids: std::collections::BTreeSet<_> = SETTINGS.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), SETTINGS.len(), "two settings share an id");
        // TWO ROWS WRITING ONE LINE is the defect the generator refuses on the
        // Nix side; this holds it on the Rust side too, because the table is
        // `include!`d and a generator that stopped refusing would arrive here
        // in silence. `sync_frequency` exists at the top level AND under
        // `[daemon]`, which is exactly why the section is part of the place.
        let places: std::collections::BTreeSet<_> = SETTINGS.iter().map(Setting::place).collect();
        assert_eq!(places.len(), SETTINGS.len(), "two settings write one line");
    }

    #[test]
    fn a_ring_keeps_its_quotes_and_a_number_has_none() {
        // THE ONE THAT BREAKS THE WHOLE FILE. `search_mode = fuzzy` is not
        // TOML, and Atuin then falls back to its defaults for EVERYTHING —
        // silently, so the symptom is a search that stopped behaving and a
        // file that looks fine.
        let mode = by_id("search-mode").expect("search-mode");
        let lines = lines_of(&[("search_mode", "\"fuzzy\"")]);
        let next = mode.turned(&lines, true).unwrap();
        assert!(next.starts_with('"') && next.ends_with('"'), "{next}");
        assert_ne!(next, "\"fuzzy\"");

        let height = by_id("inline-height").expect("inline-height");
        let lines = lines_of(&[("inline_height", "20")]);
        let next = height.turned(&lines, true).unwrap();
        assert!(!next.contains('"'), "{next}");
        assert!(next.parse::<i64>().is_ok(), "{next}");
    }

    #[test]
    fn a_switch_is_written_the_way_toml_spells_it() {
        // `1` AND `0` ARE TORRC'S. A TOML file with `auto_sync = 1` parses as
        // an integer where Atuin wants a boolean, and refuses the file.
        let sync = by_id("auto-sync").expect("auto-sync");
        let off = lines_of(&[("auto_sync", "false")]);
        assert!(!sync.is_on(&off));
        assert_eq!(sync.turned(&off, true).unwrap(), "true");
        let on = lines_of(&[("auto_sync", "true")]);
        assert!(sync.is_on(&on));
        assert_eq!(sync.turned(&on, true).unwrap(), "false");
    }

    #[test]
    fn a_setting_under_a_section_knows_which_one() {
        // `sync_frequency` EXISTS TWICE — at the top level and under
        // `[daemon]` — and they are different settings. The place is what
        // keeps them apart.
        let top = by_id("sync-frequency").expect("sync-frequency");
        let daemon = by_id("daemon-sync-frequency").expect("daemon-sync-frequency");
        assert_eq!(top.key, daemon.key);
        assert_ne!(top.place(), daemon.place());
        assert_eq!(daemon.place(), "daemon.sync_frequency");
    }

    #[test]
    fn a_pasted_list_is_left_as_typed() {
        // `history_filter` IS `["^ ", "^secret"]`, and somebody who pastes
        // that has written TOML already. Quoting it again would make the whole
        // list ONE STRING, and the filter would then match nothing while
        // appearing to be set — which is the worst of both.
        let filter = by_id("history-filter").expect("history-filter");
        assert_eq!(filter.typed(r#"["^ ", "^secret"]"#), r#"["^ ", "^secret"]"#);
        // AND A BARE WORD GETS ITS QUOTES.
        let frequency = by_id("sync-frequency").expect("sync-frequency");
        assert_eq!(frequency.typed("10m"), "\"10m\"");
        assert_eq!(frequency.typed("  "), "");
    }

    #[test]
    fn what_is_not_written_is_not_guessed_at() {
        // ATUIN'S DEFAULTS DIFFER PER SETTING and are not in the file, so a
        // screen that showed one would be stating something this system never
        // wrote. The view says "not set".
        let preview = by_id("show-preview").expect("show-preview");
        let empty = Lines::new();
        assert!(!preview.written(&empty));
        assert!(!preview.is_on(&empty));
    }

    #[test]
    fn the_one_the_flake_owns_is_marked() {
        let fixed: Vec<&str> = SETTINGS.iter().filter(|s| s.fixed).map(|s| s.key).collect();
        // THE VERSION IS PINNED, and a binary that phones home to offer one it
        // cannot install is a request nobody asked for.
        assert_eq!(fixed, vec!["update_check"], "{fixed:?}");
    }

    #[test]
    fn the_filters_that_decide_what_is_recorded_are_in_the_visible_section() {
        // THEY ARE THE ANSWER TO THE OBJECTION `nix/modules/home.nix` used to
        // state, and a safeguard in a closed drawer is not one. They belong in
        // `Leaving`, beside the switch they qualify.
        for id in ["secrets-filter", "history-filter", "auto-sync"] {
            let setting = by_id(id).unwrap_or_else(|| panic!("{id} is not declared"));
            assert_eq!(setting.scope, Scope::Leaving, "{id}");
            assert!(!setting.advanced, "{id} is hidden in the drawer");
        }
    }
}
