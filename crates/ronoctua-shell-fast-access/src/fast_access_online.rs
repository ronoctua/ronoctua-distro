//! FAST-ACCESS'S NETWORK — the web, YouTube, the dictionary, the synonyms, a
//! translation and the exchange rates: what is asked after the pause, one
//! thread per source, each with a stop the next key pulls.
//!
//! # Nothing before the person lets the host
//!
//! Every source mints its pass from `consent.json` at the moment of asking
//! (`ronoctua_consent::pass`). A host nobody let is never asked: its section
//! gets the consent row instead (`consent_rows`, a hit of its own), pressing
//! which lets the host and asks again. The offline English dictionary needs
//! no host at all.
//!
//! # Everything, without a prefix
//!
//! What is typed with no prefix asks the web and YouTube too, after the same
//! pause — but ONLY the hosts the person has already let, and it never offers
//! a consent row: the person asked for everything, not for the web, and a
//! row asking to let three hosts under every word typed would be the network
//! asking for itself. The web's answers join the kiosk row in its section,
//! YouTube's join the catalogue's videos (Shorts left out unless the switch
//! of `/y` lets them in), and a source that refuses is left out in silence:
//! the section keeps what the others found.
//!
//! # One section, several sources
//!
//! The web has two keyless sources and two the person adds (their SearXNG,
//! Brave with their key). Each answers on its own thread into one gatherer,
//! which hands the SECTION back whole each time a source answers — in the
//! sources' order, an address given twice kept once — so the section fills
//! as its answers come, and the page does not move for them.

use std::sync::Arc;
use std::sync::mpsc;

use crate::fast_access::{Answer, Card, Filled, Hit, Said, Scope, SectionId};
use crate::fast_access_search::{Ask, Back, Sink, Stops};

/// Asks every source of `ask`'s scope, each on a thread of its own, its stop
/// kept in `stops`.
pub fn ask(ask: &Ask, stops: &Stops, sink: &Sink) {
    match ask.scope {
        Scope::All => {
            web(ask, stops, sink, true);
            crate::fast_access_videos::online_in_everything(ask, stops, sink);
        }
        Scope::Web => web(ask, stops, sink, false),
        Scope::YouTube => crate::fast_access_videos::online(ask, stops, sink),
        Scope::Dictionary | Scope::Synonyms => words(ask, stops, sink),
        Scope::Translate => translate(ask, stops, sink),
        Scope::Currency => fetch_rates(ask.generation, stops, sink),
        _ => {}
    }
}

/// A section's answer, for `ask`'s generation — dropped when the stops were
/// pulled (a key came).
fn answer(ask: &Ask, stops: &Stops, sink: &Sink, section: SectionId, filled: Filled) {
    if stops.pulled() {
        return;
    }
    sink(Back::Answer(Answer {
        generation: ask.generation,
        section,
        filled,
    }));
}

/// A thread of a search, named.
fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(e) = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(work)
    {
        tracing::warn!(error = %e, "fast-access: a search's thread could not start");
    }
}

/// The web's sources for the reader: Wikipedia in their language and
/// DuckDuckGo, then the person's SearXNG and Brave when they have them.
#[must_use]
pub fn web_sources(ask: &Ask) -> Vec<ronoctua_websearch::Source> {
    let tongue = if ask.subtag.is_empty() {
        "en".to_owned()
    } else {
        ask.subtag.clone()
    };
    let mut out = vec![
        ronoctua_websearch::Source::Wikipedia { tongue },
        ronoctua_websearch::Source::DuckDuckGo,
    ];
    if let Some(url) = ask.searxng.clone() {
        out.push(ronoctua_websearch::Source::SearXNG { url });
    }
    if ronoctua_websearch::keys::has_brave() {
        out.push(ronoctua_websearch::Source::Brave);
    }
    out
}

/// The web: every source let is asked on a thread; the hosts nobody let are
/// one consent row at the top of the section — or, asked for `everything`
/// (no prefix), are not asked and not offered.
fn web(ask: &Ask, stops: &Stops, sink: &Sink, everything: bool) {
    let sources = web_sources(ask);
    let mut unlet: Vec<String> = Vec::new();
    let (tell, heard) = mpsc::channel::<(
        usize,
        Result<Vec<ronoctua_websearch::WebHit>, ronoctua_websearch::Why>,
    )>();
    let mut asked = 0usize;
    for (n, source) in sources.iter().enumerate() {
        let Some(host) = source.host() else {
            continue;
        };
        let Ok(pass) = ronoctua_consent::pass(&host) else {
            unlet.push(host);
            continue;
        };
        asked += 1;
        let net = ronoctua_websearch::internet::Internet::abortable(stops.web());
        let tell = tell.clone();
        let source = source.clone();
        let words = ask.words.clone();
        spawn("fast-access-web", move || {
            let keys = ronoctua_websearch::keys::dir();
            let got = ronoctua_websearch::search_with(
                &net,
                keys.as_ref(),
                &source,
                &words,
                ronoctua_websearch::MOST_HITS,
                &pass,
            )
            .map_err(|r| r.why);
            let _ = tell.send((n, got));
        });
    }
    drop(tell);
    let consent: Vec<Hit> = (!unlet.is_empty() && !everything)
        .then(|| {
            let hosts: Vec<&str> = unlet.iter().map(String::as_str).collect();
            crate::fast_access::consent_hit(&hosts, "fast-access-web-for")
        })
        .into_iter()
        .collect();
    if asked == 0 {
        // EVERYTHING WITH NO HOST LET keeps its kiosk row, and says nothing.
        if !everything {
            answer(ask, stops, sink, SectionId::Web, Filled::Hits(consent));
        }
        return;
    }
    // THE GATHERER: each source's answer as it comes, the section handed
    // back whole every time.
    let (ask, stops, sink) = (ask.clone(), stops.clone(), Arc::clone(sink));
    spawn("fast-access-web-gather", move || {
        let mut by: Vec<Option<Result<Vec<ronoctua_websearch::WebHit>, ronoctua_websearch::Why>>> =
            vec![None; sources.len()];
        while let Ok((n, got)) = heard.recv() {
            if stops.pulled() {
                return;
            }
            by[n] = Some(got);
            answer(
                &ask,
                &stops,
                &sink,
                SectionId::Web,
                gathered(&consent, &by, everything),
            );
        }
    });
}

/// The web's section from what the sources answered so far: the consent row
/// first, then every result in the sources' order, an address once. Every
/// source refused: the first refusal, said — except in `everything`, where a
/// refusal would take the kiosk row's place: there the section is what was
/// found, nothing when nothing was.
fn gathered(
    consent: &[Hit],
    by: &[Option<Result<Vec<ronoctua_websearch::WebHit>, ronoctua_websearch::Why>>],
    everything: bool,
) -> Filled {
    let mut hits: Vec<Hit> = consent.to_vec();
    let mut seen = std::collections::BTreeSet::new();
    let mut refused: Option<ronoctua_websearch::Why> = None;
    let mut any = false;
    for got in by.iter().flatten() {
        match got {
            Ok(found) => {
                any = true;
                for h in crate::fast_access_search::web_hits(found.clone()) {
                    if seen.insert(h.id.clone()) {
                        hits.push(h);
                    }
                }
            }
            Err(why) => {
                refused.get_or_insert(*why);
            }
        }
    }
    let answered = by.iter().all(Option::is_some);
    match (any, refused) {
        (false, Some(why)) if answered && consent.is_empty() && !everything => {
            Filled::Refused(Said::key(format!("fast-access-web-why-{}", why.id())))
        }
        _ => Filled::Hits(hits),
    }
}

/// The language a word is looked up in: the reader's, English when the
/// reader's subtag is none.
fn tongue_of(ask: &Ask) -> Option<ronoctua_words::Tongue> {
    ronoctua_words::Tongue::new(&ask.subtag).or_else(|| ronoctua_words::Tongue::new("en"))
}

/// What a word means, or other words for it: offline for English, Wiktionary
/// with the person's pass otherwise.
fn words(ask: &Ask, stops: &Stops, sink: &Sink) {
    let Some(tongue) = tongue_of(ask) else {
        return;
    };
    let offline = tongue.is_english() && ronoctua_words::offline_dir().is_some();
    let pass = ronoctua_consent::pass(ronoctua_words::WIKTIONARY_HOST).ok();
    if !offline && pass.is_none() {
        answer(
            ask,
            stops,
            sink,
            SectionId::Words,
            Filled::Hits(vec![crate::fast_access::consent_hit(
                &[ronoctua_words::WIKTIONARY_HOST],
                "fast-access-words-for",
            )]),
        );
        return;
    }
    let net = ronoctua_words::internet::Internet::abortable(stops.web());
    let (ask, stops, sink) = (ask.clone(), stops.clone(), Arc::clone(sink));
    spawn("fast-access-words", move || {
        let reach = ronoctua_words::Reach::new(&net, ronoctua_words::cache::shared());
        let word = ask.words.trim().to_owned();
        let filled = if ask.scope == Scope::Synonyms {
            match reach.synonyms(&word, &tongue, pass.as_ref()) {
                Ok(found) => Filled::Hits(vec![crate::fast_access_search::card_hit(
                    Card::Synonyms(found),
                    &word,
                )]),
                Err(r) => Filled::Refused(Said::key(format!("words-why-{}", r.why.id()))),
            }
        } else {
            match reach.define(&word, &tongue, pass.as_ref()) {
                Ok(senses) => Filled::Hits(vec![crate::fast_access_search::card_hit(
                    Card::Senses(senses),
                    &word,
                )]),
                Err(r) => Filled::Refused(Said::key(format!("words-why-{}", r.why.id()))),
            }
        };
        answer(&ask, &stops, &sink, SectionId::Words, filled);
    });
}

/// A translation, by MyMemory, with the person's pass.
fn translate(ask: &Ask, stops: &Stops, sink: &Sink) {
    let to = ask
        .tongue
        .as_deref()
        .and_then(ronoctua_words::Tongue::new)
        .or_else(|| tongue_of(ask));
    let Some(to) = to else {
        return;
    };
    let provider = ronoctua_words::Provider::MyMemory;
    let host = ronoctua_words::MYMEMORY_HOST;
    let Ok(pass) = ronoctua_consent::pass(host) else {
        answer(
            ask,
            stops,
            sink,
            SectionId::Words,
            Filled::Hits(vec![crate::fast_access::consent_hit(
                &[host],
                "fast-access-translate-for",
            )]),
        );
        return;
    };
    let net = ronoctua_words::internet::Internet::abortable(stops.web());
    let (ask, stops, sink) = (ask.clone(), stops.clone(), Arc::clone(sink));
    spawn("fast-access-translate", move || {
        let reach = ronoctua_words::Reach::new(&net, ronoctua_words::cache::shared());
        let filled = match reach.translate(ask.words.trim(), None, &to, &provider, &pass) {
            Ok(t) => Filled::Hits(vec![crate::fast_access_search::card_hit(
                Card::Translation(t),
                ask.words.trim(),
            )]),
            Err(r) => Filled::Refused(Said::key(format!("words-why-{}", r.why.id()))),
        };
        answer(&ask, &stops, &sink, SectionId::Words, filled);
    });
}

/// The exchange rates for generation `generation`: fresh on disk at once;
/// else fetched with the person's pass on a thread, its socket shut by the
/// generation's stops — the next key, the view closing. A fetch that fails is
/// SAID (`Back::RatesFailed`), with the rates on disk when there are any,
/// however old: a currency answered at an older day's rate, dated, is better
/// than "searching" for ever. A host nobody let asks nothing — the section
/// draws the consent row (`fast_access::currency_section`).
pub fn fetch_rates(generation: u64, stops: &Stops, sink: &Sink) {
    let now = ronoctua_index::now_s();
    let cached = ronoctua_calc::rates::cached();
    if let Some(c) = &cached
        && !c.stale(now)
    {
        sink(Back::Rates(c.rates.clone()));
        return;
    }
    let Ok(pass) = ronoctua_consent::pass(ronoctua_calc::rates::HOST) else {
        return;
    };
    let abort = stops.web();
    let (stops, sink) = (stops.clone(), Arc::clone(sink));
    spawn("fast-access-rates", move || {
        let got = ronoctua_calc::rates::fetch_stoppable(&pass, Some(&abort));
        match got {
            // RATES ARE NO GENERATION'S: fetched, they serve the next key too.
            Ok(r) => sink(Back::Rates(r)),
            Err(_) if stops.pulled() => {}
            Err(why) => {
                tracing::info!(
                    why = why.id(),
                    "fast-access: the exchange rates did not come"
                );
                sink(Back::RatesFailed {
                    generation,
                    why,
                    stale: cached.map(|c| c.rates),
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(url: &str) -> ronoctua_websearch::WebHit {
        ronoctua_websearch::WebHit {
            title: url.to_owned(),
            url: url.to_owned(),
            snippet: None,
            source: ronoctua_websearch::Source::DuckDuckGo,
        }
    }

    /// FAILS IF: the web's section stops keeping its consent row first, an
    /// address given by two sources shows twice, or every source refusing is
    /// drawn as an empty section instead of a sentence.
    #[test]
    fn the_web_gathers_its_sources_in_order_and_says_a_refusal() {
        let by = vec![
            Some(Ok(vec![
                found("https://a.example"),
                found("https://b.example"),
            ])),
            None,
        ];
        let ids = crate::fast_access_search::ids_of(&gathered(&[], &by, false));
        assert_eq!(ids, ["web:https://a.example", "web:https://b.example"]);
        let by = vec![
            Some(Ok(vec![found("https://a.example")])),
            Some(Ok(vec![
                found("https://a.example"),
                found("https://c.example"),
            ])),
        ];
        let consent = vec![crate::fast_access::consent_hit(
            &["brave.example"],
            "fast-access-web-for",
        )];
        let ids = crate::fast_access_search::ids_of(&gathered(&consent, &by, false));
        assert_eq!(
            ids,
            [
                "consent:brave.example",
                "web:https://a.example",
                "web:https://c.example"
            ]
        );
        let by = vec![
            Some(Err(ronoctua_websearch::Why::Unreachable)),
            Some(Err(ronoctua_websearch::Why::Failed)),
        ];
        assert_eq!(
            gathered(&[], &by, false),
            Filled::Refused(Said::key("fast-access-web-why-unreachable".to_owned()))
        );
    }

    /// FAILS IF: a search with no prefix stops asking the web, or asks the
    /// hosts the person never let, or a refusal of every source takes the
    /// place of what "everything" already shows (the kiosk row).
    #[test]
    fn everything_asks_the_web_it_was_let_and_keeps_quiet_otherwise() {
        let mut ask = Ask::nothing(1);
        ask.words = "rust".to_owned();
        assert!(ask.wants_online(), "no prefix no longer goes online");
        ask.words = "  ".to_owned();
        assert!(!ask.wants_online(), "nothing typed went online");
        let by = vec![
            Some(Err(ronoctua_websearch::Why::Unreachable)),
            Some(Err(ronoctua_websearch::Why::Failed)),
        ];
        assert_eq!(gathered(&[], &by, true), Filled::Hits(Vec::new()));
        let by = vec![Some(Ok(vec![found("https://a.example")])), None];
        let ids = crate::fast_access_search::ids_of(&gathered(&[], &by, true));
        assert_eq!(ids, ["web:https://a.example"]);
    }

    #[test]
    fn the_web_asks_wikipedia_in_the_readers_language() {
        let mut ask = Ask::nothing(1);
        ask.subtag = "pt".to_owned();
        ask.searxng = Some("https://sx.example".to_owned());
        let sources = web_sources(&ask);
        assert_eq!(sources[0].host().as_deref(), Some("pt.wikipedia.org"));
        assert_eq!(sources[1], ronoctua_websearch::Source::DuckDuckGo);
        assert!(
            sources
                .iter()
                .any(|s| s.host().as_deref() == Some("sx.example"))
        );
    }

    #[test]
    fn a_word_is_looked_up_in_the_readers_language() {
        let mut ask = Ask::nothing(1);
        ask.subtag = "pt".to_owned();
        assert_eq!(tongue_of(&ask).unwrap().code(), "pt");
        ask.subtag = String::new();
        assert_eq!(tongue_of(&ask).unwrap().code(), "en");
    }
}
