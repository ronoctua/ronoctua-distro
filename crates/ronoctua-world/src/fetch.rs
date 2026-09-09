//! Asking the two hosts, and what to make of what they say.
//!
//! # Neither of them is asked from the drawing loop
//!
//! [`once`] blocks — on a name, on a socket, on somebody else's server — and
//! the loop that calls it is [`watch`]'s thread. The footer reads the FILE.
//! This project has paid twice for a frame that waited on something outside
//! it (`systemd-run --scope`, and a child a buffer waited on), and the rule
//! that came out of it is written in `ronoctua_brain::client`: whoever draws
//! gets microseconds or nothing.

use anyhow::{Context as _, Result};

use crate::prefs::{Preferences, Wanted};
use crate::{Cache, Known};

/// The current temperature at a coordinate, in Celsius.
///
/// # Errors
///
/// The host could not be reached, or answered something without the field.
pub fn temperature(place: &Wanted) -> Result<f64> {
    // FIVE DECIMALS is about a metre, and the coordinates in the table have
    // two. Formatting them short keeps the request from carrying a precision
    // that says more about where somebody is than the feature needs.
    let path = format!(
        "/v1/forecast?latitude={:.2}&longitude={:.2}&current=temperature_2m",
        place.latitude, place.longitude
    );
    let raw = crate::http::get(crate::WEATHER_HOST, &path)?;
    let parsed: serde_json::Value =
        serde_json::from_slice(&raw).context("the weather host did not answer JSON")?;
    parsed
        .get("current")
        .and_then(|c| c.get("temperature_2m"))
        .and_then(serde_json::Value::as_f64)
        .context("the weather answer has no current temperature")
}

/// What one unit of `place.currency` is worth in `place.rate_in`.
///
/// # Errors
///
/// The host could not be reached, or answered without the pair asked for.
pub fn rate(place: &Wanted) -> Result<f64> {
    let path = format!(
        "/v1/latest?base={}&symbols={}",
        place.currency, place.rate_in
    );
    let raw = crate::http::get(crate::RATE_HOST, &path)?;
    let parsed: serde_json::Value =
        serde_json::from_slice(&raw).context("the rate host did not answer JSON")?;
    parsed
        .get("rates")
        .and_then(|r| r.get(&place.rate_in))
        .and_then(serde_json::Value::as_f64)
        .with_context(|| format!("the rate answer has no {}", place.rate_in))
}

/// One pass over every place that asked for something, into a cache.
///
/// # Why a failure is not returned
///
/// Because there is nobody to return it to, and because one host being down
/// must not cost the other's answer. Each miss is a `debug!` and a field that
/// stays as it was — the glance then draws what it has, which is the whole
/// arrangement: an absent temperature is a piece the line does not have, not
/// a state to render.
#[must_use]
pub fn once(prefs: &Preferences, mut cache: Cache) -> Cache {
    for place in prefs.places() {
        // THE PLACE AND THE SWITCH BOTH HAVE TO SAY YES. The place asks for a
        // temperature; the switch says whether this machine asks anybody at
        // all — which is a decision about privacy, not about a city.
        let wants_weather = place.weather && prefs.ask_weather;
        let wants_rate = place.rate && prefs.ask_rate;
        let temperature = wants_weather.then(|| answer(&place, "temperature", temperature));
        let rate = wants_rate.then(|| answer(&place, "rate", rate));
        let known = cache.places.entry(place.zone.clone()).or_default();
        keep(known, temperature.flatten(), rate.flatten());
    }
    cache
}

/// One fetch, with the failure turned into `None` and a line in the log.
fn answer(place: &Wanted, what: &str, ask: fn(&Wanted) -> Result<f64>) -> Option<f64> {
    match ask(place) {
        Ok(value) => Some(value),
        Err(e) => {
            tracing::debug!(zone = place.zone, what, error = %e, "no answer");
            None
        }
    }
}

/// Lays what arrived over what was there, and `None` means NOTHING ARRIVED.
///
/// # Why this is a function with a name
///
/// Because it is the rule, and the rule is the part worth being sure of: a
/// fetch that fails must leave the field alone. Written inline it could only
/// be tested by reaching the network — and in a sandbox, where nothing
/// resolves, such a test passes for the wrong reason and would go on passing
/// after somebody changed `Err` to clear the field.
pub fn keep(known: &mut Known, temperature: Option<f64>, rate: Option<f64>) {
    if let Some(t) = temperature {
        known.temperature = Some(t);
    }
    if let Some(r) = rate {
        known.rate = Some(r);
    }
}

/// The thread that keeps the cache fresh, and the file that carries it.
///
/// # Why a thread and not the footer's second timer
///
/// Because a fetch BLOCKS — on a name, on a socket, on somebody else's
/// server — and the footer's one-second clock runs in the drawing loop. This
/// project has paid twice for a frame that waited on something outside it, and
/// the rule that came out is in `ronoctua_brain::client`: whoever draws gets
/// microseconds or nothing.
///
/// # And why it writes a file rather than sending a message
///
/// The file IS the broadcast — the theme's arrangement, the notifications',
/// and now this one. The footer reads it; so would a second door, if there
/// ever is one; and a footer that was not running when the answer arrived
/// picks it up at its next start instead of losing it.
///
/// Returns `None`, having started nothing, when no place asks for anything
/// over the network: a machine whose glance is clocks must not open a socket
/// or read a certificate bundle.
pub fn watch() -> Option<std::thread::JoinHandle<()>> {
    if !crate::prefs::store().read().anything_to_ask() {
        tracing::debug!("no place asks for weather or a rate; nothing will be fetched");
        return None;
    }
    let every = std::time::Duration::from_secs(
        60 * crate::WEATHER_EVERY_MINUTES.min(crate::RATE_EVERY_MINUTES),
    );
    std::thread::Builder::new()
        .name("world".to_owned())
        .spawn(move || {
            let store = crate::store::store();
            loop {
                // BOTH FILES ARE RE-READ EACH PASS. The cache because another
                // process may have written it — starting from what is on disk
                // is what keeps two writers from undoing each other — and the
                // PREFERENCES because a place added or a host switched off has
                // to take effect without a restart, which is the whole point
                // of it being a file rather than a rebuild.
                let prefs = crate::prefs::store().read();
                let before = store.read();
                let after = once(&prefs, before.clone());
                if after != before
                    && let Err(e) = store.write(&after)
                {
                    tracing::warn!(error = %e, "the glance's cache could not be written");
                }
                std::thread::sleep(every);
            }
        })
        .map_err(|e| tracing::warn!(error = %e, "the glance's thread did not start"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A miss leaves what was already known ALONE.
    ///
    /// THE DEFECT THIS CATCHES: a failed fetch clearing the field. The glance
    /// would blink between a temperature and nothing every time the network
    /// hiccuped, which reads as the feature being broken rather than as one
    /// request being lost.
    ///
    /// It tests [`keep`] and NOT `once`, deliberately. The first version of
    /// this test called `once` and asserted the old values survived, on the
    /// premise that a test machine has no network — and this one does: the
    /// fetch succeeded, London came back at 15,3 °C, and the assertion failed
    /// for the one reason that was good news. In a sandbox it would have
    /// passed while proving nothing.
    #[test]
    fn a_place_keeps_what_it_had_when_an_answer_does_not_come() {
        let mut known = Known {
            temperature: Some(8.0),
            rate: Some(7.42),
        };
        keep(&mut known, None, None);
        assert_eq!(known.temperature, Some(8.0));
        assert_eq!(known.rate, Some(7.42));

        // And what DOES arrive lands, one field at a time.
        keep(&mut known, Some(15.3), None);
        assert_eq!(known.temperature, Some(15.3));
        assert_eq!(
            known.rate,
            Some(7.42),
            "the rate was cleared by a weather answer"
        );

        keep(&mut known, None, Some(7.5));
        assert_eq!(known.temperature, Some(15.3));
        assert_eq!(known.rate, Some(7.5));
    }

    /// A place that has never been heard from is an empty record, not a
    /// missing key — so a lookup never has to tell "no answer yet" from
    /// "never asked".
    #[test]
    fn nothing_known_is_a_value_rather_than_an_absence() {
        let mut known = Known::default();
        keep(&mut known, None, None);
        assert_eq!(known, Known::default());
    }

    /// A pass with both hosts switched off asks nobody anything, and still
    /// gives every place an entry — so a lookup never has to tell "no answer
    /// yet" from "never asked".
    ///
    /// It reaches no network even on a machine that has one, which is what
    /// makes it a test rather than a measurement.
    #[test]
    fn a_pass_with_both_hosts_off_asks_nobody() {
        let prefs = Preferences {
            ask_weather: false,
            ask_rate: false,
            ..Preferences::default()
        };
        let after = once(&prefs, Cache::default());
        for place in prefs.places() {
            assert!(after.places.contains_key(&place.zone), "{}", place.zone);
            assert_eq!(after.of(&place.zone), Known::default());
        }
    }
}
