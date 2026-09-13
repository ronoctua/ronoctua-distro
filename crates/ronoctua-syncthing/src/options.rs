//! What Syncthing lets a person change, and what this system says about it.
//!
//! The TABLE is generated — `assets/syncthing-options.nix` through
//! `nix/scripts/gen-syncthing-options.py`. What is written here is only what
//! the table cannot say: where each scope lives on the wire, how a value is
//! read out of a configuration object and put back into one, and how a ring
//! turns.
//!
//! # Why a hundred and twenty-one rows come out of a declaration
//!
//! Because the alternative is a hundred and twenty-one decisions in a file
//! nobody can audit. Declared, the same source produces the table, the text
//! keys both languages owe, and the check that compares the declaration with a
//! real `/rest/config` in BOTH directions — so an option Syncthing adds and an
//! option we invented each fail the build instead of quietly not being there.

/// Which of Syncthing's configuration objects an option lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// One folder. Needs the folder's id.
    Folder,
    /// One device. Needs the device's id.
    Device,
    /// The service as a whole.
    Global,
    /// The web interface.
    Interface,
    /// The template every new folder starts from.
    FolderDefaults,
    /// The template every new device starts from.
    DeviceDefaults,
}

impl Scope {
    /// Where this object is on the wire.
    ///
    /// `at` is the folder's or device's id, and is ignored by the scopes that
    /// do not have one.
    ///
    /// `rest.rs` IS THE ONLY MODULE THAT SPELLS ONE, and this delegates
    /// rather than repeating six. The header of that module explains what the
    /// promise is; `one-syncthing-boundary` is what holds it.
    #[must_use]
    pub fn path(self, at: Option<&str>) -> String {
        crate::rest::object_of(self, at)
    }

    /// Whether this scope needs to be told WHICH one.
    #[must_use]
    pub const fn needs_a_name(self) -> bool {
        matches!(self, Self::Folder | Self::Device)
    }

    /// The id, which is the stem of the section's text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Device => "device",
            Self::Global => "global",
            Self::Interface => "interface",
            Self::FolderDefaults => "folder-defaults",
            Self::DeviceDefaults => "device-defaults",
        }
    }
}

/// What kind of thing an option is, and therefore what a key does to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// On or off.
    Switch,
    /// A whole number inside a range.
    Number {
        /// The smallest Syncthing accepts.
        least: i64,
        /// The largest worth offering.
        most: i64,
        /// What the number is counted in, as a text key.
        ///
        /// `None` for the handful that are genuinely bare — a port number, a
        /// priority, a traffic class. Everything else gets one, because a row
        /// reading `look again every  3600` says nothing at all: that came off
        /// a printed screen, and no assertion had a way to see it.
        unit: Option<&'static str>,
    },
    /// Free text.
    Text,
    /// One of a fixed set of words.
    Ring(&'static [&'static str]),
    /// A list of strings, written on one line and separated by commas.
    List,
    /// A number with a unit beside it — `{ "value": 1, "unit": "%" }`.
    Amount,
}

impl Kind {
    /// Whether `S-<arrows>` mean anything on this row.
    ///
    /// Everything but text and a list: those two are typed, and typing happens
    /// in the footer's minibuffer.
    #[must_use]
    pub const fn turns(self) -> bool {
        !matches!(self, Self::Text | Self::List)
    }

    /// Whether this row is answered by typing.
    #[must_use]
    pub const fn is_typed(self) -> bool {
        matches!(self, Self::Text | Self::List)
    }
}

/// One option.
#[derive(Debug, Clone, Copy)]
pub struct Setting {
    /// Kebab-case, unique across every scope. What a check names.
    pub id: &'static str,
    /// The text key, which is `syncthing-opt-<id>`.
    ///
    /// A field rather than a function so that nothing allocates to draw a row
    /// and so that a check can find every one of them by reading the table.
    pub label: &'static str,
    /// The JSON key, exactly as Syncthing spells it.
    pub key: &'static str,
    /// The nested object it sits inside, when it does.
    pub within: Option<&'static str>,
    /// Which configuration object.
    pub scope: Scope,
    /// What kind of thing it is.
    pub kind: Kind,
    /// The flake writes this one, so the row refuses. Invariant 6.
    pub fixed: bool,
    /// It lives in the drawer, closed.
    pub advanced: bool,
}

impl Setting {
    /// The value out of a configuration object, honouring `within`.
    #[must_use]
    pub fn read<'a>(&self, from: &'a serde_json::Value) -> Option<&'a serde_json::Value> {
        match self.within {
            Some(nest) => from.get(nest)?.get(self.key),
            None => from.get(self.key),
        }
    }

    /// The body of the PATCH that writes `value`, `within` and all.
    ///
    /// A nested object deep-merges, because Go's unmarshal leaves the fields
    /// the JSON does not mention alone. An ARRAY does not — a slice is
    /// replaced whole — which is why a [`Kind::List`] is written as the entire
    /// list and never as one entry.
    #[must_use]
    pub fn written(&self, value: serde_json::Value) -> serde_json::Value {
        match self.within {
            Some(nest) => serde_json::json!({ nest: { self.key: value } }),
            None => serde_json::json!({ self.key: value }),
        }
    }

    /// The next value on this row, one step either way.
    ///
    /// `None` when the row is not one that turns — text and lists are typed.
    #[must_use]
    pub fn turned(
        &self,
        now: Option<&serde_json::Value>,
        forward: bool,
    ) -> Option<serde_json::Value> {
        match self.kind {
            Kind::Switch => {
                let was = now.and_then(serde_json::Value::as_bool).unwrap_or(false);
                Some(serde_json::json!(!was))
            }
            Kind::Number { least, most, .. } => {
                let was = now.and_then(serde_json::Value::as_i64).unwrap_or(least);
                let step = step_of(least, most);
                let next = if forward {
                    was.saturating_add(step)
                } else {
                    was.saturating_sub(step)
                };
                // AND IT STOPS AT THE ENDS INSTEAD OF WRAPPING.
                //
                // A ring of words wraps because every word is as good as the
                // next. A range does not: holding the arrow to raise a limit
                // and having it drop to the floor at the top is a number
                // nobody meant to write, and Syncthing would take it.
                Some(serde_json::json!(next.clamp(least, most)))
            }
            Kind::Ring(words) => {
                let was = now.and_then(serde_json::Value::as_str).unwrap_or("");
                let at = words.iter().position(|w| *w == was).unwrap_or(0);
                let step = if forward { 1 } else { words.len() - 1 };
                Some(serde_json::json!(words[(at + step) % words.len()]))
            }
            Kind::Amount => {
                // THE ARROWS MOVE THE NUMBER AND LEAVE THE UNIT ALONE.
                //
                // An amount is one JSON object with two halves, and two halves
                // want two rows — which is a shape no other option in the
                // table has. What a person almost always means by "keep more
                // free" is a bigger number in the unit they already chose, and
                // `docs/PENDING.md` records the other half as deliberately not
                // done rather than half-done here.
                let was = now
                    .and_then(|a| a.get("value"))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let unit = now
                    .and_then(|a| a.get("unit"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("%");
                let next = if forward { was + 1.0 } else { was - 1.0 };
                Some(serde_json::json!({ "value": next.max(0.0), "unit": unit }))
            }
            Kind::Text | Kind::List => None,
        }
    }

    /// The value, written the way a person reads it.
    ///
    /// Numbers and words come back as they are; this is about the shapes that
    /// are not one value — a list is its entries with commas between, an
    /// amount is its number and its unit.
    #[must_use]
    pub fn readable(&self, now: Option<&serde_json::Value>) -> String {
        match self.kind {
            Kind::List => now
                .and_then(serde_json::Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(serde_json::Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default(),
            Kind::Amount => {
                let value = now
                    .and_then(|a| a.get("value"))
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                let unit = now
                    .and_then(|a| a.get("unit"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("%");
                format!("{value:.0} {unit}")
            }
            Kind::Text => now
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            Kind::Ring(_) => now
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            Kind::Switch => now
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                .to_string(),
            Kind::Number { .. } => now
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0)
                .to_string(),
        }
    }

    /// What a typed line becomes.
    ///
    /// `None` when the row is not one that is typed.
    #[must_use]
    pub fn typed(&self, said: &str) -> Option<serde_json::Value> {
        match self.kind {
            Kind::Text => Some(serde_json::json!(said)),
            Kind::List => Some(serde_json::json!(
                said.split(',')
                    .map(str::trim)
                    .filter(|piece| !piece.is_empty())
                    .collect::<Vec<_>>()
            )),
            _ => None,
        }
    }
}

/// How far one press moves a number.
///
/// One would be right for a count of connections and useless for a bandwidth
/// limit in kilobits, where the range is a million wide: holding the arrow for
/// a minute would move a thousandth of it. So the step is a thousandth of the
/// range, rounded to something a person would have typed, and never less than
/// one.
#[must_use]
pub fn step_of(least: i64, most: i64) -> i64 {
    let span = most.saturating_sub(least);
    let rough = span / 1000;
    if rough <= 1 {
        return 1;
    }
    // The next round number at or below a thousandth: 1, 10, 100, 1000…
    let mut round = 1;
    while round * 10 <= rough {
        round *= 10;
    }
    round
}

/// The text key for one word of a ring.
///
/// Syncthing spells them three ways — `sendreceive`, `smallestFirst`,
/// `copy_file_range` — and an empty string means "none", which is a word on
/// the screen like any other. The rule is the generator's, written twice
/// because one side is Python and the other Rust; `syncthing-words-agree`
/// holds them together.
#[must_use]
pub fn word_key(value: &str) -> String {
    if value.is_empty() {
        return "syncthing-word-none".to_owned();
    }
    let mut out = String::with_capacity(value.len() + 4);
    for (n, c) in value.chars().enumerate() {
        if c == '_' {
            out.push('-');
        } else if c.is_ascii_uppercase() {
            if n > 0 {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    format!("syncthing-word-{out}")
}

/// Every option in one scope, in the order the screen draws them.
pub fn of_scope(scope: Scope) -> impl Iterator<Item = &'static Setting> {
    SETTINGS.iter().filter(move |s| s.scope == scope)
}

/// The option this id names.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.id == id)
}

include!(concat!(env!("OUT_DIR"), "/options.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_id_is_its_own_and_so_is_every_field() {
        // The generator refuses both, and this is the same promise held from
        // the other side: a duplicate id makes two rows share one label, and a
        // duplicate field makes two rows write one setting, of which only the
        // one under the cursor is telling the truth.
        let mut ids = std::collections::HashSet::new();
        let mut fields = std::collections::HashSet::new();
        for s in SETTINGS {
            assert!(ids.insert(s.id), "`{}` is declared twice", s.id);
            assert!(
                fields.insert((s.scope, s.within, s.key)),
                "`{}` writes a field another row also writes",
                s.id
            );
            assert_eq!(s.label, format!("syncthing-opt-{}", s.id));
        }
    }

    #[test]
    fn the_word_key_agrees_with_the_generators_rule() {
        // Written twice because one side is Python and the other Rust. A
        // disagreement would be a key the `.ftl` does not carry, which draws
        // as `[syncthing-word-…]` on the screen.
        assert_eq!(word_key(""), "syncthing-word-none");
        assert_eq!(word_key("sendreceive"), "syncthing-word-sendreceive");
        assert_eq!(word_key("smallestFirst"), "syncthing-word-smallest-first");
        assert_eq!(
            word_key("copy_file_range"),
            "syncthing-word-copy-file-range"
        );
        assert_eq!(
            word_key("duplicate_extents"),
            "syncthing-word-duplicate-extents"
        );
    }

    #[test]
    fn a_number_stops_at_the_ends_and_a_ring_wraps() {
        // Different on purpose. Holding the arrow to raise a limit and having
        // it drop to the floor at the top is a number nobody meant to write,
        // and Syncthing would take it. Every word of a ring is as good as the
        // next, so it wraps.
        let number = Setting {
            id: "x",
            label: "syncthing-opt-x",
            key: "x",
            within: None,
            scope: Scope::Global,
            kind: Kind::Number {
                least: 0,
                most: 8,
                unit: None,
            },
            fixed: false,
            advanced: false,
        };
        assert_eq!(
            number.turned(Some(&serde_json::json!(8)), true),
            Some(serde_json::json!(8))
        );
        assert_eq!(
            number.turned(Some(&serde_json::json!(0)), false),
            Some(serde_json::json!(0))
        );
        assert_eq!(
            number.turned(Some(&serde_json::json!(3)), true),
            Some(serde_json::json!(4))
        );

        let ring = Setting {
            kind: Kind::Ring(&["a", "b", "c"]),
            ..number
        };
        assert_eq!(
            ring.turned(Some(&serde_json::json!("c")), true),
            Some(serde_json::json!("a"))
        );
        assert_eq!(
            ring.turned(Some(&serde_json::json!("a")), false),
            Some(serde_json::json!("c"))
        );
        // AND A WORD SYNCTHING INVENTED LANDS ON THE FIRST, never nowhere.
        assert_eq!(
            ring.turned(Some(&serde_json::json!("new")), true),
            Some(serde_json::json!("b"))
        );
    }

    #[test]
    fn the_step_follows_the_range() {
        // One is right for a count of connections and useless for a limit in
        // kilobits: a range a million wide would take a thousand presses to
        // cross a thousandth of it.
        assert_eq!(step_of(0, 8), 1);
        assert_eq!(step_of(0, 100), 1);
        assert_eq!(step_of(0, 1_000_000), 1000);
        assert_eq!(step_of(0, 31_536_000), 10000);
        assert_eq!(step_of(-1, 3), 1);
    }

    #[test]
    fn a_nested_option_writes_a_nested_body() {
        let versioning = by_id("versioning").expect("the table declares it");
        assert_eq!(versioning.within, Some("versioning"));
        assert_eq!(
            versioning.written(serde_json::json!("simple")),
            serde_json::json!({ "versioning": { "type": "simple" } })
        );
        // AND IT READS BACK OUT OF THE SAME PLACE.
        let folder = serde_json::json!({ "versioning": { "type": "trashcan" } });
        assert_eq!(
            versioning.read(&folder),
            Some(&serde_json::json!("trashcan"))
        );
    }

    #[test]
    fn a_list_is_one_line_in_and_a_whole_array_out() {
        let listen = by_id("listen-addresses").expect("the table declares it");
        assert_eq!(
            listen.readable(Some(&serde_json::json!([
                "tcp://0.0.0.0:22000",
                "quic://0.0.0.0:22000"
            ]))),
            "tcp://0.0.0.0:22000, quic://0.0.0.0:22000"
        );
        assert_eq!(
            listen.typed(" default , dynamic ,, "),
            Some(serde_json::json!(["default", "dynamic"]))
        );
        // A LIST NEVER TURNS: it is typed, in the footer's minibuffer.
        assert_eq!(listen.turned(None, true), None);
        assert!(listen.kind.is_typed());
    }

    #[test]
    fn the_fourteen_the_flake_owns_are_the_fourteen_the_module_writes() {
        // If this list and `nix/modules/syncthing.nix` ever disagree, a row
        // either refuses something the person could really change, or offers
        // a key whose effect the next `nixos-rebuild` undoes in silence.
        // `syncthing-flake-owned-rows-refuse` is the check that reads the
        // module; this is the same promise from the Rust side.
        let owned: Vec<&str> = SETTINGS
            .iter()
            .filter(|s| s.fixed && s.scope == Scope::Global)
            .map(|s| s.key)
            .collect();
        let mut sorted = owned.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            [
                "autoUpgradeIntervalH",
                "crashReportingEnabled",
                "globalAnnounceEnabled",
                "limitBandwidthInLan",
                "localAnnounceEnabled",
                // TWO OF THESE ARE NIXPKGS' OWN DEFAULTS and not our module's,
                // and they are the flake's all the same: the module PUTs the
                // whole of `settings.options` at every activation.
                "localAnnouncePort",
                "maxFolderConcurrency",
                "maxRecvKbps",
                "maxSendKbps",
                "natEnabled",
                "relaysEnabled",
                "setLowPriority",
                "startBrowser",
                "urAccepted",
            ]
        );
    }

    #[test]
    fn a_scope_that_needs_a_name_puts_it_in_the_path() {
        assert!(Scope::Folder.needs_a_name());
        assert!(!Scope::Global.needs_a_name());
        assert_eq!(
            Scope::Folder.path(Some("a b")),
            "/rest/config/folders/a%20b",
            "a folder id with a space in it would otherwise cut the path in half"
        );
        assert_eq!(Scope::Global.path(None), "/rest/config/options");
    }

    #[test]
    fn there_are_options_in_every_scope_and_a_drawer_in_most() {
        for scope in [
            Scope::Folder,
            Scope::Device,
            Scope::Global,
            Scope::Interface,
        ] {
            assert!(
                of_scope(scope).count() > 0,
                "{} has no options at all",
                scope.id()
            );
        }
        assert!(
            SETTINGS.iter().any(|s| s.advanced),
            "nothing is in the drawer, so the drawer draws empty"
        );
        assert!(
            SETTINGS.iter().any(|s| s.fixed),
            "nothing is the flake's, so no row ever explains invariant 6"
        );
    }
}
