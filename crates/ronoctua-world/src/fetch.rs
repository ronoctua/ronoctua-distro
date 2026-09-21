//! Asking the hosts, and what to make of what they say.
//!
//! # Neither of them is asked from the drawing loop
//!
//! [`once`] blocks — on a name, on a socket, on somebody else's server — and
//! the loop that calls it is [`watch`]'s thread. The footer reads the FILE.
//! This project has paid twice for a frame that waited on something outside
//! it (`systemd-run --scope`, and a child a buffer waited on), and the rule
//! that came out of it is written in `ronoctua_brain::client`: whoever draws
//! gets microseconds or nothing.
//!
//! # A source is a LIST, and the reading is chosen by id
//!
//! `assets/world.nix` declares who is asked and in what order; the PATH and
//! the shape of the answer are Rust, one arm per id. That split is not
//! decoration: a fallback is not a host swap — met.no's answer is a forecast
//! with a hundred timesteps in it and Open-Meteo's is one number — so a
//! declaration can only hold who and how often.
//!
//! An id with no reader is refused, and a test walks the declared table to
//! prove none is left without one.

use anyhow::{Context as _, Result, bail};

use crate::prefs::{Preferences, Wanted};
use crate::{Attempt, Cache, Known, Provider, Source};

/// How long a source waits after a pass that answered nothing.
///
/// # Why a failure does not simply wait out the cadence
///
/// Because the cadence is half an hour and the likeliest failure is the first
/// pass of all: the thread starts with the footer, and the footer starts
/// before the network does. One missed name lookup cost the glance its
/// temperature for thirty minutes, with nothing on screen saying why — which
/// is exactly what "the weather does not work" looks like from the outside.
const RETRY_MINUTES: u64 = 2;

/// And how many of those in a row before it goes back to waiting properly.
///
/// A machine that is simply offline must not ask every two minutes for ever:
/// after this many quick tries the source waits out its declared cadence like
/// any other, and the next ordinary pass tries again.
const RETRIES: u32 = 3;

/// The current temperature at a coordinate, in Celsius, from one host.
///
/// # Errors
///
/// The id has no reader, the host could not be reached, or it answered
/// something without the field.
pub fn temperature(who: &Provider, place: &Wanted) -> Result<f64> {
    // FIVE DECIMALS is about a metre, and the coordinates in the table have
    // two. Formatting them short keeps the request from carrying a precision
    // that says more about where somebody is than the feature needs.
    match who.id {
        "open-meteo" => {
            let path = format!(
                "/v1/forecast?latitude={:.2}&longitude={:.2}&current=temperature_2m",
                place.latitude, place.longitude
            );
            let parsed = json(who, &path)?;
            parsed
                .get("current")
                .and_then(|c| c.get("temperature_2m"))
                .and_then(serde_json::Value::as_f64)
                .context("the answer has no current temperature")
        }
        // MET.NO ANSWERS A FORECAST, not a reading: a hundred timesteps of
        // which the FIRST is now. Taking `timeseries[0]` is the reading the
        // institute's own documentation describes, and it is why this is an
        // arm of a match rather than a second host name in a table.
        "met-no" => {
            let path = format!(
                "/weatherapi/locationforecast/2.0/compact?lat={:.2}&lon={:.2}",
                place.latitude, place.longitude
            );
            let parsed = json(who, &path)?;
            parsed
                .get("properties")
                .and_then(|p| p.get("timeseries"))
                .and_then(|s| s.get(0))
                .and_then(|s| s.get("data"))
                .and_then(|d| d.get("instant"))
                .and_then(|i| i.get("details"))
                .and_then(|d| d.get("air_temperature"))
                .and_then(serde_json::Value::as_f64)
                .context("the answer has no air temperature")
        }
        other => bail!("there is no reader for the weather source `{other}`"),
    }
}

/// What one unit of `place.currency` is worth in `place.rate_in`, from one
/// host.
///
/// # Errors
///
/// The id has no reader, the host could not be reached, or it answered
/// without the pair asked for.
pub fn rate(who: &Provider, place: &Wanted) -> Result<f64> {
    let path = match who.id {
        "frankfurter" => format!(
            "/v1/latest?base={}&symbols={}",
            place.currency, place.rate_in
        ),
        // THE WHOLE TABLE, because this host has no way to ask for one pair.
        // Three kilobytes against the transport's megabyte ceiling.
        "er-api" => format!("/v6/latest/{}", place.currency),
        other => bail!("there is no reader for the rate source `{other}`"),
    };
    let parsed = json(who, &path)?;
    parsed
        .get("rates")
        .and_then(|r| r.get(&place.rate_in))
        .and_then(serde_json::Value::as_f64)
        .with_context(|| format!("the answer has no {}", place.rate_in))
}

/// `GET` and parse, so that every reader above is only about the SHAPE.
fn json(who: &Provider, path: &str) -> Result<serde_json::Value> {
    let raw = crate::http::get(who.host, path)?;
    serde_json::from_slice(&raw).context("the host did not answer JSON")
}

/// What a pass at one source found, and from whom.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Answer {
    /// What each place was told, in the order the places were given. `None`
    /// where nobody answered for it.
    pub values: Vec<Option<f64>>,
    /// How it went, ready to be written down.
    pub attempt: Attempt,
}

/// One pass at ONE source, over the places that want it.
///
/// The hosts are asked IN ORDER and only until one answers, so the second
/// costs nothing on a machine where the first is up. A host that answered for
/// the first place is the one asked for the rest of them: falling back per
/// place would mix two hosts' numbers in one line.
///
/// # Why the failure is returned rather than logged
///
/// Because there is nobody to return it TO at the call site — the thread runs
/// unattended — and because a miss that only reaches the log is a miss the
/// screen cannot say anything about. See [`crate::Attempt`].
#[must_use]
pub fn ask(source: Source, places: &[&Wanted], now: i64) -> Answer {
    let read: fn(&Provider, &Wanted) -> Result<f64> = match source {
        Source::Weather => temperature,
        Source::Rate => rate,
    };
    ask_from(source.from(), places, now, read)
}

/// The same, with the hosts and the reader HANDED IN.
///
/// # Why the seam exists
///
/// Because the fallback is the rule worth being sure of, and a test of it
/// through [`ask`] would need two hosts on the internet — one of them down.
/// In a sandbox, where nothing resolves, such a test passes for the wrong
/// reason and would go on passing after somebody made the first failure
/// abandon the pass. It is the argument [`keep`] already makes about itself,
/// one function along.
#[must_use]
pub fn ask_from(
    who_all: &[Provider],
    places: &[&Wanted],
    now: i64,
    read: fn(&Provider, &Wanted) -> Result<f64>,
) -> Answer {
    let mut trouble = String::new();
    for who in who_all {
        let mut values = Vec::with_capacity(places.len());
        let mut answered = false;
        let mut said = String::new();
        for place in places {
            match read(who, place) {
                Ok(value) => {
                    answered = true;
                    values.push(Some(value));
                }
                Err(e) => {
                    said = format!("{}: {e}", who.host);
                    tracing::debug!(zone = place.zone, host = who.host, error = %e, "no answer");
                    values.push(None);
                }
            }
        }
        if answered {
            return Answer {
                values,
                attempt: Attempt {
                    host: who.host.to_owned(),
                    at: Some(now),
                    // A PASS THAT ANSWERED IS NOT A PASS WITH A PROBLEM, even
                    // when one place of five was refused: what the screen says
                    // about a source is whether the source is answering.
                    trouble: String::new(),
                },
            };
        }
        trouble = said;
    }
    Answer {
        values: vec![None; places.len()],
        attempt: Attempt {
            // THE LAST HOST TRIED, so the screen names somebody rather than
            // saying that something went wrong somewhere.
            host: who_all
                .last()
                .map_or_else(String::new, |p| p.host.to_owned()),
            // NOTHING ARRIVED, so nothing is stamped: `at` is when an answer
            // last came, and a failure is not one.
            at: None,
            trouble,
        },
    }
}

/// One pass over one source, laid into a cache.
///
/// # Why a failure does not clear what was there
///
/// Because a fetch that fails must leave the field alone: the glance would
/// blink between a temperature and nothing every time the network hiccuped,
/// which reads as the feature being broken rather than as one request being
/// lost.
#[must_use]
pub fn once(source: Source, prefs: &Preferences, mut cache: Cache, now: i64) -> Cache {
    // THE PLACE AND THE SWITCH BOTH HAVE TO SAY YES. The place asks for a
    // temperature; the switch says whether this machine asks anybody at all —
    // which is a decision about privacy, not about a city.
    let places = prefs.places();
    let wanted: Vec<&Wanted> = if source.asked(prefs) {
        places.iter().filter(|p| source.wanted_by(p)).collect()
    } else {
        Vec::new()
    };
    // EVERY PLACE GETS AN ENTRY, asked for or not, so that a lookup never has
    // to tell "no answer yet" from "never asked".
    for place in &places {
        cache.places.entry(place.zone.clone()).or_default();
    }
    if wanted.is_empty() {
        return cache;
    }
    let answer = ask(source, &wanted, now);
    for (place, value) in wanted.iter().zip(&answer.values) {
        let known = cache.places.entry(place.zone.clone()).or_default();
        keep(known, source, *value);
    }
    // AND WHAT HAPPENED, whether or not it went well. A record that were only
    // written on failure could never say when the last answer arrived.
    let kept = cache.sources.entry(source.id().to_owned()).or_default();
    lay(kept, answer.attempt);
    cache
}

/// Lays a pass's record over the one before it.
///
/// A pass that ANSWERED replaces the record whole. A pass that did not keeps
/// the timestamp of the last answer — that is the field's whole meaning, "when
/// something last arrived" — and puts its own words in `trouble`.
pub fn lay(kept: &mut Attempt, fresh: Attempt) {
    if fresh.at.is_some() {
        *kept = fresh;
        return;
    }
    kept.host = fresh.host;
    kept.trouble = fresh.trouble;
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
pub fn keep(known: &mut Known, source: Source, value: Option<f64>) {
    let Some(value) = value else { return };
    match source {
        Source::Weather => known.temperature = Some(value),
        Source::Rate => known.rate = Some(value),
    }
}

/// How long a source waits before it is asked again.
///
/// A pure function, so the rule can be read and tested without a clock: a pass
/// that answered waits out the declared cadence; one that did not tries again
/// in [`RETRY_MINUTES`], up to [`RETRIES`] times in a row, and then waits like
/// any other.
#[must_use]
pub const fn again_in(source: Source, misses: u32) -> std::time::Duration {
    if misses > 0 && misses <= RETRIES {
        return std::time::Duration::from_secs(60 * RETRY_MINUTES);
    }
    std::time::Duration::from_secs(60 * source.every_minutes())
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
    std::thread::Builder::new()
        .name("world".to_owned())
        .spawn(move || {
            let store = crate::store::store();
            // EACH SOURCE KEEPS ITS OWN NEXT MOMENT, which is what makes the
            // declared cadence mean anything: one sleep of the smaller of the
            // two asked the exchange host twelve times as often as
            // `assets/world.nix` says to.
            let mut due = [std::time::Instant::now(); Source::ALL.len()];
            let mut misses = [0u32; Source::ALL.len()];
            loop {
                // BOTH FILES ARE RE-READ EACH PASS. The cache because another
                // process may have written it — starting from what is on disk
                // is what keeps two writers from undoing each other — and the
                // PREFERENCES because a place added or a host switched off has
                // to take effect without a restart, which is the whole point
                // of it being a file rather than a rebuild.
                let prefs = crate::prefs::store().read();
                let before = store.read();
                let mut after = before.clone();
                let start = std::time::Instant::now();
                let now = epoch_seconds();
                for (n, source) in Source::ALL.into_iter().enumerate() {
                    if due[n] > start {
                        continue;
                    }
                    after = once(source, &prefs, after, now);
                    let answered = after.about(source).trouble.is_empty();
                    misses[n] = if answered { 0 } else { misses[n] + 1 };
                    due[n] = start + again_in(source, misses[n]);
                }
                if after != before
                    && let Err(e) = store.write(&after)
                {
                    tracing::warn!(error = %e, "the glance's cache could not be written");
                }
                // UNTIL THE FIRST OF THEM IS DUE, and never longer.
                let sleep = due
                    .iter()
                    .map(|at| at.saturating_duration_since(std::time::Instant::now()))
                    .min()
                    .unwrap_or_else(|| std::time::Duration::from_secs(60));
                std::thread::sleep(sleep.max(std::time::Duration::from_secs(1)));
            }
        })
        .map_err(|e| tracing::warn!(error = %e, "the glance's thread did not start"))
        .ok()
}

/// The wall clock, in seconds since the epoch. `0` when the clock is before
/// it, which is a machine whose time has not been set rather than an error.
fn epoch_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EVERY DECLARED SOURCE HAS A READER.
    ///
    /// THE DEFECT THIS CATCHES: a host added to `assets/world.nix` with no arm
    /// in the `match` above. It would build, it would be drawn in `LEADER s l`
    /// as a party this machine asks, and it would never be able to answer —
    /// the failure arriving as "there is no reader", from inside the fallback
    /// chain, where nobody is watching.
    ///
    /// It reaches no network: an unknown id is refused before a socket is
    /// opened, so the ids that ARE known fail on the name lookup instead, and
    /// the two messages are what this tells apart.
    #[test]
    fn every_declared_source_has_a_reader() {
        let nowhere = Wanted {
            zone: "Europe/London".to_owned(),
            latitude: 51.51,
            longitude: -0.13,
            currency: "GBP".to_owned(),
            rate_in: "BRL".to_owned(),
            ..Wanted::default()
        };
        let mut seen = 0;
        for source in Source::ALL {
            assert!(!source.from().is_empty(), "{} asks nobody", source.id());
            for who in source.from() {
                let said = match source {
                    Source::Weather => temperature(who, &nowhere),
                    Source::Rate => rate(who, &nowhere),
                }
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default();
                assert!(
                    !said.contains("no reader"),
                    "`{}` is declared and has no reader",
                    who.id
                );
                seen += 1;
            }
        }
        assert!(seen >= 4, "only {seen} providers were walked");
    }

    /// An id nobody wrote a reader for is refused BY NAME, and that refusal is
    /// what the test above leans on.
    #[test]
    fn an_undeclared_id_is_refused_rather_than_guessed() {
        let who = Provider {
            id: "not-a-source",
            host: "example.invalid",
        };
        let place = Wanted::default();
        for said in [
            temperature(&who, &place).unwrap_err().to_string(),
            rate(&who, &place).unwrap_err().to_string(),
        ] {
            assert!(said.contains("no reader"), "{said}");
        }
    }

    /// THE SECOND HOST IS ASKED, AND ONLY WHEN THE FIRST DOES NOT ANSWER.
    ///
    /// THE DEFECT THIS CATCHES, and it is the owner's own request: a source
    /// with one host down had nothing to show and nothing to say — the miss
    /// was a `debug!` line and the temperature simply was not on the bar,
    /// which from the outside is a feature that does not work.
    ///
    /// It reaches no network: the readers are planted, which is the whole
    /// reason `ask_from` takes one. A test of this through the real hosts
    /// would need one of them to be down, and in a sandbox it would pass by
    /// resolving nothing at all.
    #[test]
    fn the_second_host_answers_when_the_first_does_not() {
        const WHO: &[Provider] = &[
            Provider {
                id: "first",
                host: "first.invalid",
            },
            Provider {
                id: "second",
                host: "second.invalid",
            },
        ];
        let place = Wanted::default();
        let places = [&place];

        // BOTH UP: the first answers and the second is never reached.
        let said = ask_from(WHO, &places, 7, |_, _| Ok(1.0));
        assert_eq!(said.values, vec![Some(1.0)]);
        assert_eq!(said.attempt.host, "first.invalid");
        assert_eq!(said.attempt.at, Some(7));
        assert!(said.attempt.trouble.is_empty());

        // THE FIRST DOWN: the second answers, and the screen names IT.
        let said = ask_from(WHO, &places, 7, |who, _| {
            if who.id == "first" {
                anyhow::bail!("answered 503")
            } else {
                Ok(2.0)
            }
        });
        assert_eq!(said.values, vec![Some(2.0)], "the fallback was not asked");
        assert_eq!(said.attempt.host, "second.invalid");
        assert!(
            said.attempt.trouble.is_empty(),
            "a pass that answered was written down as a failure"
        );

        // BOTH DOWN: nothing arrives, the last host tried is named, and what
        // it said is kept — which is what turns "the weather does not work"
        // into a sentence on `LEADER s l`.
        let said = ask_from(WHO, &places, 7, |_, _| anyhow::bail!("answered 503"));
        assert_eq!(said.values, vec![None]);
        assert_eq!(said.attempt.host, "second.invalid");
        assert_eq!(said.attempt.at, None, "a failure stamped a time of arrival");
        assert!(said.attempt.trouble.contains("503"), "{:?}", said.attempt);
        assert!(
            said.attempt.trouble.contains("second.invalid"),
            "the trouble does not name who said it: {:?}",
            said.attempt
        );
    }

    /// ONE HOST ANSWERS FOR EVERY PLACE OF A PASS, and the fallback is not
    /// taken per place.
    ///
    /// THE DEFECT THIS CATCHES: two hosts' numbers in one line. Falling back
    /// per place would put Open-Meteo's temperature beside met.no's on the
    /// same bar, and nothing on screen could say which was which.
    #[test]
    fn one_host_answers_for_the_whole_pass() {
        const WHO: &[Provider] = &[
            Provider {
                id: "first",
                host: "first.invalid",
            },
            Provider {
                id: "second",
                host: "second.invalid",
            },
        ];
        let london = Wanted {
            zone: "Europe/London".to_owned(),
            ..Wanted::default()
        };
        let lisbon = Wanted {
            zone: "Europe/Lisbon".to_owned(),
            ..Wanted::default()
        };
        let places = [&london, &lisbon];
        // THE FIRST HOST ANSWERS FOR ONE OF THE TWO, which is a pass that
        // answered: the second place keeps what it had and the host is not
        // abandoned for it.
        let said = ask_from(WHO, &places, 7, |who, place| {
            if who.id == "first" && place.zone.ends_with("London") {
                Ok(1.0)
            } else {
                anyhow::bail!("no such place")
            }
        });
        assert_eq!(said.values, vec![Some(1.0), None]);
        assert_eq!(said.attempt.host, "first.invalid");
    }

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
        keep(&mut known, Source::Weather, None);
        keep(&mut known, Source::Rate, None);
        assert_eq!(known.temperature, Some(8.0));
        assert_eq!(known.rate, Some(7.42));

        // And what DOES arrive lands, one field at a time.
        keep(&mut known, Source::Weather, Some(15.3));
        assert_eq!(known.temperature, Some(15.3));
        assert_eq!(
            known.rate,
            Some(7.42),
            "the rate was cleared by a weather answer"
        );

        keep(&mut known, Source::Rate, Some(7.5));
        assert_eq!(known.temperature, Some(15.3));
        assert_eq!(known.rate, Some(7.5));
    }

    /// A pass that answered nothing keeps the stamp of the last one that did.
    ///
    /// THE DEFECT THIS CATCHES: `at` is "when something last arrived", and a
    /// failure overwriting it with `None` would make the screen say a source
    /// has never answered the first time it misses one pass.
    #[test]
    fn a_failure_does_not_erase_when_the_last_answer_came() {
        let mut kept = Attempt {
            host: "api.open-meteo.com".to_owned(),
            at: Some(1_788_000_000),
            trouble: String::new(),
        };
        lay(
            &mut kept,
            Attempt {
                host: "api.met.no".to_owned(),
                at: None,
                trouble: "api.met.no: answered 503".to_owned(),
            },
        );
        assert_eq!(
            kept.at,
            Some(1_788_000_000),
            "the last answer was forgotten"
        );
        assert_eq!(kept.host, "api.met.no");
        assert!(kept.trouble.contains("503"));

        // And a pass that answers clears the trouble whole.
        lay(
            &mut kept,
            Attempt {
                host: "api.open-meteo.com".to_owned(),
                at: Some(1_788_000_900),
                trouble: String::new(),
            },
        );
        assert_eq!(kept.at, Some(1_788_000_900));
        assert!(kept.trouble.is_empty());
    }

    /// A pass that failed is retried in MINUTES, and not in half an hour —
    /// but not for ever.
    ///
    /// THE DEFECT THIS CATCHES: the likeliest failure of all is the first pass,
    /// because the thread starts with the footer and the footer starts before
    /// the network. Waiting out the cadence cost the glance its temperature for
    /// thirty minutes with nothing on screen saying why.
    #[test]
    fn a_miss_is_tried_again_soon_and_then_left_alone() {
        let every = std::time::Duration::from_secs(60 * Source::Weather.every_minutes());
        assert_eq!(again_in(Source::Weather, 0), every);
        let quick = std::time::Duration::from_secs(60 * RETRY_MINUTES);
        assert!(quick < every, "the retry is not quicker than the cadence");
        for misses in 1..=RETRIES {
            assert_eq!(again_in(Source::Weather, misses), quick, "{misses} misses");
        }
        assert_eq!(
            again_in(Source::Weather, RETRIES + 1),
            every,
            "an offline machine would ask every two minutes for ever"
        );
    }

    /// EACH SOURCE KEEPS ITS OWN CADENCE, and they differ.
    ///
    /// THE DEFECT THIS CATCHES: one sleep of `min(weather, rate)` for both,
    /// which asked the exchange host every thirty minutes where
    /// `assets/world.nix` declares three hundred and sixty.
    #[test]
    fn the_two_sources_are_not_asked_at_one_cadence() {
        assert_ne!(
            Source::Weather.every_minutes(),
            Source::Rate.every_minutes(),
            "the two cadences are the same, so this proves nothing"
        );
        assert_ne!(again_in(Source::Weather, 0), again_in(Source::Rate, 0));
    }

    /// A place that has never been heard from is an empty record, not a
    /// missing key — so a lookup never has to tell "no answer yet" from
    /// "never asked".
    #[test]
    fn nothing_known_is_a_value_rather_than_an_absence() {
        let mut known = Known::default();
        keep(&mut known, Source::Weather, None);
        assert_eq!(known, Known::default());
        assert_eq!(Cache::default().about(Source::Rate), Attempt::default());
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
        let mut after = Cache::default();
        for source in Source::ALL {
            after = once(source, &prefs, after, 0);
        }
        for place in prefs.places() {
            assert!(after.places.contains_key(&place.zone), "{}", place.zone);
            assert_eq!(after.of(&place.zone), Known::default());
        }
        assert!(
            after.sources.is_empty(),
            "a source nobody asked was written down as having been tried"
        );
    }
}
