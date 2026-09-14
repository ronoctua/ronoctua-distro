//! Every Tor setting this system offers, and what a key does to one.
//!
//! # Where the table comes from
//!
//! `assets/tor-options.nix`, through `nix/scripts/gen-options.py`. Nothing
//! here is hand-written: the generator refuses a duplicate id, a number with
//! no range and a ring that cannot turn, and it writes the list of text keys
//! both languages owe alongside. See the asset's own header.
//!
//! # What a VALUE is here
//!
//! A torrc line's text. Tor's configuration is not typed — `UseBridges 1` and
//! `ExitNodes {de},{nl}` are both a name and a string — so the arithmetic of
//! turning a ring or nudging a number comes from [`ronoctua_options`], which
//! the other declared tables share, and the marshalling is a `to_string` here.
//!
//! # `fixed` IS INVARIANT 6 ON THE SCREEN
//!
//! The generated torrc reads `%include` FIRST, so anything
//! `nix/modules/tor.nix` declares overrides what the fragment says. A row for
//! one of those that could be edited would be a key whose effect is undone
//! before it is pressed. They are drawn, reachable, and they refuse — the
//! shape the format buffer's disabled XFS row already has.

use ronoctua_options::{Kind, Spelling};

use crate::tor::fragment::Lines;

/// Which part of the sheet a setting belongs to.
///
/// The order is the order the screen draws in, and it is declared in
/// `flake.nix` rather than read off the table: `builtins.toJSON` sorts an
/// attribute set alphabetically, and a sheet's shape is an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// What the flake owns: the socket, the cookie, the SOCKS port.
    Service,
    /// Where traffic comes out — entry and exit nodes, and how strictly.
    Path,
    /// Bridges and pluggable transports.
    Bridges,
    /// What this machine offers back.
    Onion,
    /// Real settings, rarely touched. Drawn inside the drawer.
    Advanced,
}

/// One declared setting.
///
/// Eight fields and not seven: `label` is the text key and `id` is what a
/// check names, and they are different on purpose — `type` would collide
/// across scopes and reads as nothing on a screen.
#[derive(Debug, Clone, Copy)]
pub struct Setting {
    /// Kebab-case, and the stem of [`Setting::label`].
    pub id: &'static str,
    /// The text key for the row's name.
    pub label: &'static str,
    /// The torrc option, spelled as Tor spells it.
    pub key: &'static str,
    /// Unused here — torrc has no nesting. Kept because the generator writes
    /// one table shape for four domains, and Syncthing's `/rest/config` does.
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

/// How torrc spells a boolean.
pub const TRUTH: Spelling = Spelling::DIGITS;

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
    /// What the fragment holds for this setting, if anything.
    #[must_use]
    pub fn read<'a>(&self, lines: &'a Lines) -> Option<&'a Vec<String>> {
        lines.get(self.key)
    }

    /// The one value, for the kinds that have exactly one.
    #[must_use]
    pub fn one<'a>(&self, lines: &'a Lines) -> Option<&'a str> {
        self.read(lines).and_then(|v| v.first()).map(String::as_str)
    }

    /// Whether a switch is on.
    ///
    /// ABSENT IS OFF, and that is a decision rather than a default: torrc's own
    /// defaults differ per option, and a screen that guessed them would say
    /// "on" about something this system never wrote. What the fragment does
    /// not say, this system did not ask for.
    #[must_use]
    pub fn is_on(&self, lines: &Lines) -> bool {
        self.one(lines).is_some_and(|v| TRUTH.reads_yes(v))
    }

    /// The value a key press would leave behind, or `None` when the kind is
    /// typed rather than turned.
    ///
    /// `forwards` is which arrow. For a switch the direction does not matter —
    /// there are two states and both arrows reach the other one — which is
    /// why `RET` can use this too.
    #[must_use]
    pub fn turned(&self, lines: &Lines, forwards: bool) -> Option<String> {
        match self.kind {
            Kind::Switch => Some(TRUTH.of(!self.is_on(lines)).to_owned()),
            Kind::Ring(ring) => {
                let now = self.one(lines).unwrap_or("");
                Some(ronoctua_options::turn(ring, now, forwards).to_owned())
            }
            Kind::Number { least, most, .. } => {
                let now = self
                    .one(lines)
                    .and_then(|v| v.trim().parse::<i64>().ok())
                    .unwrap_or(least);
                Some(ronoctua_options::nudge(now, least, most, forwards).to_string())
            }
            Kind::Text | Kind::List | Kind::Amount => None,
        }
    }

    /// Writes a value in, or takes the setting out when it is empty.
    ///
    /// AN EMPTY VALUE REMOVES THE LINE rather than writing a bare key. Tor
    /// refuses a torrc with `ExitNodes` and nothing after it, and the view
    /// produces exactly that whenever somebody clears a field — so the empty
    /// case is the ordinary one, not an edge.
    pub fn put(&self, lines: &mut Lines, values: Vec<String>) {
        let kept: Vec<String> = values
            .into_iter()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
            .collect();
        if kept.is_empty() {
            lines.remove(self.key);
        } else {
            lines.insert(self.key.to_owned(), kept);
        }
    }

    /// The text key for a ring's current word, for the screen.
    ///
    /// `None` for every other kind: a switch says on or off in the view's own
    /// words, and a number says itself.
    #[must_use]
    pub fn word(&self, lines: &Lines) -> Option<String> {
        match self.kind {
            Kind::Ring(_) => Some(format!(
                "tor-word-{}",
                ronoctua_options::word_key(self.one(lines).unwrap_or(""))
            )),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_arrived_and_covers_every_scope() {
        assert!(
            SETTINGS.len() > 20,
            "the generated table is too small to be the real one"
        );
        for scope in [
            Scope::Service,
            Scope::Path,
            Scope::Bridges,
            Scope::Onion,
            Scope::Advanced,
        ] {
            assert!(
                of_scope(scope).next().is_some(),
                "{scope:?} has no rows, so a section of the sheet would be a heading over nothing"
            );
        }
    }

    #[test]
    fn every_id_is_unique_and_so_is_every_torrc_key() {
        // The generator refuses both, and this holds the promise on the Rust
        // side too — the table is `include!`d, so a generator that stopped
        // refusing would arrive here silently.
        let ids: std::collections::BTreeSet<_> = SETTINGS.iter().map(|s| s.id).collect();
        assert_eq!(ids.len(), SETTINGS.len(), "two settings share an id");
        let keys: std::collections::BTreeSet<_> = SETTINGS.iter().map(|s| s.key).collect();
        assert_eq!(
            keys.len(),
            SETTINGS.len(),
            "two settings write one torrc key"
        );
    }

    #[test]
    fn what_the_flake_owns_is_marked_and_is_what_the_module_declares() {
        let fixed: Vec<&str> = SETTINGS.iter().filter(|s| s.fixed).map(|s| s.key).collect();
        // THE FOUR THE MODULE WRITES. `tor-flake-owned-rows-refuse` compares
        // this against `nix/modules/tor.nix` itself; here it is held so that a
        // table that quietly dropped one is caught by `cargo test` too.
        assert!(fixed.contains(&"SOCKSPort"), "{fixed:?}");
        assert!(fixed.contains(&"CookieAuthentication"), "{fixed:?}");
        assert!(fixed.contains(&"CookieAuthFile"), "{fixed:?}");
    }

    #[test]
    fn a_switch_that_is_not_written_is_off() {
        // ABSENT IS OFF, and it is a decision: torrc's own defaults differ per
        // option, and a screen that guessed them would say "on" about
        // something this system never wrote.
        let bridges = by_id("use-bridges").expect("use-bridges");
        let mut lines = Lines::new();
        assert!(!bridges.is_on(&lines));
        let on = bridges.turned(&lines, true).unwrap();
        bridges.put(&mut lines, vec![on]);
        assert!(bridges.is_on(&lines));
        assert_eq!(lines.get("UseBridges").unwrap(), &vec!["1".to_owned()]);
        // AND TURNING IT BACK TAKES THE LINE OUT rather than writing `0`: what
        // the fragment does not say, this system did not ask for.
        let off = bridges.turned(&lines, true).unwrap();
        assert_eq!(off, "0");
        bridges.put(&mut lines, vec![String::new()]);
        assert!(!lines.contains_key("UseBridges"));
    }

    #[test]
    fn clearing_a_field_removes_the_line_instead_of_writing_a_bare_key() {
        // Tor REFUSES a torrc with `ExitNodes` and nothing after it, and the
        // view produces exactly that whenever somebody clears a field.
        let exits = by_id("exit-nodes").expect("exit-nodes");
        let mut lines = Lines::new();
        exits.put(&mut lines, vec!["{de}".to_owned()]);
        assert!(lines.contains_key("ExitNodes"));
        exits.put(&mut lines, vec!["   ".to_owned()]);
        assert!(
            !lines.contains_key("ExitNodes"),
            "a cleared field left a bare key, which Tor will not start with"
        );
    }

    #[test]
    fn a_number_lands_inside_its_range_from_anything_the_file_said() {
        let life = by_id("circuit-lifetime").expect("circuit-lifetime");
        let Kind::Number { least, most, .. } = life.kind else {
            panic!("circuit-lifetime stopped being a number");
        };
        let mut lines = Lines::new();
        // A FILE EDITED BY HAND can hold anything, including a word.
        life.put(&mut lines, vec!["nonsense".to_owned()]);
        let moved: i64 = life.turned(&lines, true).unwrap().parse().unwrap();
        assert!((least..=most).contains(&moved), "{moved}");
        // AND THE ENDS CLAMP.
        life.put(&mut lines, vec![most.to_string()]);
        assert_eq!(life.turned(&lines, true).unwrap(), most.to_string());
        life.put(&mut lines, vec![least.to_string()]);
        assert_eq!(life.turned(&lines, false).unwrap(), least.to_string());
    }

    #[test]
    fn a_rings_word_is_a_text_key_the_generator_also_wrote() {
        let transport = by_id("bridge-transport").expect("bridge-transport");
        let mut lines = Lines::new();
        // NOTHING WRITTEN lands on the ring's first word, which is `none`.
        assert_eq!(transport.word(&lines).unwrap(), "tor-word-none");
        transport.put(&mut lines, vec!["meek_lite".to_owned()]);
        // THE UNDERSCORE BECOMES A HYPHEN, the same way the generator did it.
        // A key with an underscore in it is one `gen-i18n.py` never matches.
        assert_eq!(transport.word(&lines).unwrap(), "tor-word-meek-lite");
    }

    #[test]
    fn a_typed_kind_has_nothing_to_turn() {
        // The one place these would be confused is the place a person presses
        // a key and watches nothing happen.
        let bridges = by_id("bridge-lines").expect("bridge-lines");
        assert!(bridges.kind.is_typed());
        assert!(bridges.turned(&Lines::new(), true).is_none());
    }
}
