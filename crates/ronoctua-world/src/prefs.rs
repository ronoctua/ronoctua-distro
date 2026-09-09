//! The places THIS person watches, over the ones the distro declares.
//!
//! # Two lists, and the file wins
//!
//! `assets/world.nix` declares London and Brasília — the distro's answer, and
//! the one a fresh machine shows. `$XDG_CONFIG_HOME/ronoctua/world.json` is
//! the person's, written by `LEADER s l`, and it wins whole rather than field
//! by field: a list of places merged entry-by-entry with a list somebody else
//! wrote is a list nobody can predict, and the question "which places do I
//! watch" has one answer.
//!
//! It is the shape `power.json` and `theme.json` already have — file over the
//! distro's floor — and the same reason invariant 6 makes it necessary: what
//! `assets/world.nix` says takes a rebuild to change, and choosing a city is
//! not a rebuild-sized decision.
//!
//! # Why the switches for the two hosts live here too
//!
//! Because turning a source off is a decision about privacy, not about a
//! place: every fetch tells a third party this machine's address and which
//! cities it watches. It belongs where the person's other choices are, and it
//! has to be reachable without editing Nix — which is the whole reason this
//! file exists.

use crate::Place;

/// The file, under `$XDG_CONFIG_HOME/ronoctua`.
pub const FILE: &str = "world.json";

/// One place, as the person keeps it.
///
/// The owned twin of [`Place`], which is `&'static` because it is generated.
/// One type for both would mean either allocating the generated table or
/// leaking the person's — so there are two, and [`Wanted::of`] is the one
/// crossing between them.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Wanted {
    /// The IANA zone. It is also the identity: the cache is keyed by it.
    pub zone: String,
    /// The text key its name is drawn from, when the distro declares it.
    ///
    /// Empty for a place the person added, and then the ZONE is drawn —
    /// `Europe/Lisbon` reads as a place, and a key that does not exist would
    /// read as `[world-place-europe-lisbon]`.
    pub key: String,
    /// Where it is, for the weather.
    pub latitude: f64,
    /// Where it is, for the weather.
    pub longitude: f64,
    /// What one of this is worth, in [`Wanted::rate_in`].
    pub currency: String,
    /// And in what.
    pub rate_in: String,
    /// Draw the clock?
    pub clock: bool,
    /// Draw the date?
    pub date: bool,
    /// Draw the temperature?
    pub weather: bool,
    /// Draw the rate?
    pub rate: bool,
}

impl Default for Wanted {
    /// A place that shows its clock and nothing else.
    ///
    /// By hand, and not derived: `bool::default()` is false four times, which
    /// is the one shape the generator refuses — a place that occupies a name's
    /// worth of the line and says nothing.
    fn default() -> Self {
        Self {
            zone: String::new(),
            key: String::new(),
            latitude: 0.0,
            longitude: 0.0,
            currency: String::new(),
            rate_in: String::new(),
            clock: true,
            date: false,
            weather: false,
            rate: false,
        }
    }
}

impl Wanted {
    /// The owned form of a declared place.
    #[must_use]
    pub fn of(place: &Place) -> Self {
        Self {
            zone: place.zone.to_owned(),
            key: place.key.to_owned(),
            latitude: place.latitude,
            longitude: place.longitude,
            currency: place.currency.to_owned(),
            rate_in: place.rate_in.to_owned(),
            clock: place.clock,
            date: place.date,
            weather: place.weather,
            rate: place.rate,
        }
    }

    /// Does it have somewhere on the planet?
    ///
    /// A place the distro declares carries a coordinate; a place somebody
    /// typed the name of has none, because there is no geocoder here and
    /// asking a name of one would be a third network service. Null island is
    /// the honest test — nobody watches the weather at 0,0.
    #[must_use]
    pub fn has_a_coordinate(&self) -> bool {
        self.latitude != 0.0 || self.longitude != 0.0
    }

    /// Does it show anything at all?
    ///
    /// A place with all four off is what the generator refuses in the distro's
    /// table; here it can HAPPEN, because somebody turned the last one off. It
    /// is not refused — refusing the fourth keystroke of four is a screen that
    /// fights back — it is simply not drawn, and the row says so.
    #[must_use]
    pub const fn shows_anything(&self) -> bool {
        self.clock || self.date || self.weather || self.rate
    }
}

/// What the person chose about the glance.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// The places, primary first. EMPTY means "the distro's", and that is not
    /// the same as "none": a person who turned every place off has an empty
    /// list of their own, which `chosen` distinguishes.
    pub places: Vec<Wanted>,
    /// Has the person ever chosen? It is what tells an empty list from an
    /// untouched one, and without it turning the last place off would put the
    /// distro's two back on the next start.
    pub chosen: bool,
    /// Ask the weather host at all.
    pub ask_weather: bool,
    /// Ask the rate host at all.
    pub ask_rate: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            places: Vec::new(),
            chosen: false,
            // BOTH ON, because the distro's places ask for them and a default
            // that contradicted the default table would be a feature that
            // ships switched off.
            ask_weather: true,
            ask_rate: true,
        }
    }
}

impl Preferences {
    /// The places in force: the person's when they have chosen, else the
    /// distro's.
    #[must_use]
    pub fn places(&self) -> Vec<Wanted> {
        if self.chosen {
            self.places.clone()
        } else {
            crate::PLACES.iter().map(Wanted::of).collect()
        }
    }

    /// Does anything need the network, once the switches are applied?
    ///
    /// A machine whose glance is clocks must not start a thread, open a socket
    /// or read a certificate bundle — and turning both hosts off is exactly
    /// how a person says so.
    #[must_use]
    pub fn anything_to_ask(&self) -> bool {
        self.places()
            .iter()
            .any(|p| (p.weather && self.ask_weather) || (p.rate && self.ask_rate))
    }
}

/// The store the choices live in.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

/// The places in force, reading the real file.
#[must_use]
pub fn places() -> Vec<Wanted> {
    store().read().places()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Untouched, the distro's places are what shows.
    #[test]
    fn a_machine_that_never_chose_shows_the_distros_places() {
        let prefs = Preferences::default();
        let places = prefs.places();
        assert_eq!(places.len(), crate::PLACES.len());
        assert_eq!(places[0].zone, crate::primary().zone);
    }

    /// AND AN EMPTY LIST OF ONE'S OWN IS NOT AN UNTOUCHED ONE.
    ///
    /// THE DEFECT THIS CATCHES: turning off the last place and finding the
    /// distro's two back at the next start, for ever, with no way to say no.
    /// `chosen` is what tells the two apart, and it is why the field exists
    /// rather than the emptiness being read as a signal.
    #[test]
    fn choosing_nothing_is_a_choice_and_survives() {
        let prefs = Preferences {
            places: Vec::new(),
            chosen: true,
            ..Preferences::default()
        };
        assert!(prefs.places().is_empty(), "the distro's places came back");
        assert!(!prefs.anything_to_ask());
    }

    /// Turning a host off stops the asking, whatever the places want.
    #[test]
    fn a_host_switched_off_is_not_asked() {
        let both_on = Preferences::default();
        assert!(
            both_on.anything_to_ask(),
            "the distro's table asks for both"
        );

        let off = Preferences {
            ask_weather: false,
            ask_rate: false,
            ..Preferences::default()
        };
        assert!(
            !off.anything_to_ask(),
            "a socket would be opened for a source nobody asked for"
        );
    }

    /// A place the person added, with no text key, is still identifiable —
    /// by its zone, which is what the cache and the line both use.
    #[test]
    fn a_place_of_ones_own_needs_no_text_key() {
        let mine = Wanted {
            zone: "Europe/Lisbon".to_owned(),
            ..Wanted::default()
        };
        assert!(mine.key.is_empty());
        assert!(mine.shows_anything(), "a new place shows its clock");
        assert!(!mine.date && !mine.weather && !mine.rate);
    }

    /// The round trip keeps every field, which is what a file is for.
    #[test]
    fn what_is_written_is_what_comes_back() {
        let mine = Preferences {
            places: crate::PLACES.iter().map(Wanted::of).collect(),
            chosen: true,
            ask_weather: false,
            ask_rate: true,
        };
        let text = serde_json::to_string(&mine).expect("serialises");
        let back: Preferences = serde_json::from_str(&text).expect("parses");
        assert_eq!(back, mine);
    }

    /// A file from before a field existed still parses, and the field takes
    /// its default — `#[serde(default)]` on the struct, which is what keeps a
    /// new switch from emptying somebody's places.
    #[test]
    fn an_older_file_still_parses() {
        let back: Preferences =
            serde_json::from_str(r#"{"places":[],"chosen":true}"#).expect("parses");
        assert!(back.chosen);
        assert!(back.ask_weather, "a missing switch defaulted to off");
    }
}
