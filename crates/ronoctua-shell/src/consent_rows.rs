//! THE ROW THAT ASKS FOR A HOST — "Ask {hosts}", and pressing it lets them —
//! drawn the same way in every app.
//!
//! # Why one row
//!
//! Nothing in this distro talks to the network before the person let the
//! host it would ask (`ronoctua-consent`). Every app that asks a host has the
//! same moment to draw: the answer needs a host nobody let yet. The player
//! drew it one way (`player-consent-ask`), the calculator another
//! (`calc-rates-ask`), and the search box would have been a third — three
//! sentences for one question, and three ways of saying that the press could
//! not be kept. So the question is ONE row, here: its words
//! ([`label`], [`note`]), its tones, its key ([`OF_THE_CONSENT`], `C-a`, the
//! row's own press from the keyboard), and what pressing it does
//! ([`allow`]) — the merge write into `consent.json` through
//! `ronoctua_consent::let_hosts`, and a sentence when it could not be kept
//! ([`unkept`], `consent-unkept-<id>`).
//!
//! # What a host app still says
//!
//! WHAT FOR: "converting currencies needs today's exchange rates" is the
//! calculator's, "a search of the web" the search box's. The app hands the
//! key of that purpose ([`Ask::what`]); the row says it under the hosts, and
//! adds the one thing every row says the same way — pressing allows it.

use ronoctua_tokens as tokens;

/// The row's own key, from the keyboard: the press a click on it makes.
/// Taught while a row is drawn and only then — a key that does nothing is not
/// taught.
pub const OF_THE_CONSENT: &[(&str, &str)] = &[("C-a", ALLOW_ID)];

/// The id of the row's key — its words (`consent-allow`) and its sentence
/// (`about-consent-allow`).
pub const ALLOW_ID: &str = "consent-allow";

/// What a host app asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ask<'a> {
    /// The hosts it would ask, as `ronoctua_consent::normal` writes them.
    pub hosts: &'a [&'a str],
    /// The key of the sentence that says what for — the app's own.
    pub what: &'static str,
}

/// The hosts, as a reader reads a list of them.
#[must_use]
pub fn hosts_text(hosts: &[&str], texts: &ronoctua_i18n::Texts) -> String {
    match hosts {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [first @ .., last] => texts.t_with(
            ronoctua_i18n::key::CONSENT_HOSTS_AND,
            &[("hosts", &first.join(", ")), ("last", last)],
        ),
    }
}

/// The row's label: "Ask {hosts}" — what the press does, in the register of
/// a row that acts.
#[must_use]
pub fn label(hosts: &[&str], texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        ronoctua_i18n::key::CONSENT_ASK,
        &[("hosts", &hosts_text(hosts, texts))],
    )
}

/// The row's note: what for, and that pressing allows it.
#[must_use]
pub fn note(what: &str, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        ronoctua_i18n::key::CONSENT_NOTE,
        &[("what", &texts.t(what))],
    )
}

/// The row as lines `room` cells wide: the label first, in the tones of a
/// row that acts (focused or not), then its note wrapped — never cut
/// (`a-note-is-wrapped-and-not-cut`) — in a fact's quiet tone. Each line is
/// the press: the whole row is one control.
#[must_use]
pub fn lines(
    ask: Ask<'_>,
    room: u16,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
) -> Vec<(String, tokens::Color)> {
    let (acts, _) = crate::chrome::Kind::ListedSetting.tones(focused);
    let mut out: Vec<(String, tokens::Color)> =
        crate::chrome::note_lines(&label(ask.hosts, texts), room)
            .into_iter()
            .map(|l| (l, acts))
            .collect();
    out.extend(
        crate::chrome::note_lines(&note(ask.what, texts), room)
            .into_iter()
            .map(|l| (l, tokens::color::TEXT_SECONDARY)),
    );
    out
}

/// The row was pressed: `hosts` are let, merged into what `consent.json`
/// already says. A refusal is a SENTENCE for the door's line, never silence.
///
/// # Errors
///
/// What could not be kept, in the reader's words ([`unkept`]).
pub fn allow(hosts: &[&str], texts: &ronoctua_i18n::Texts) -> Result<(), String> {
    ronoctua_consent::let_hosts(hosts, true).map_err(|why| {
        tracing::warn!(why = why.id(), ?hosts, "a consent could not be kept");
        unkept(why, hosts, texts)
    })
}

/// Why a consent could not be kept, as the reader reads it.
#[must_use]
pub fn unkept(
    why: ronoctua_consent::Unkept,
    hosts: &[&str],
    texts: &ronoctua_i18n::Texts,
) -> String {
    texts.t_with(
        &format!("consent-unkept-{}", why.id()),
        &[("hosts", &hosts_text(hosts, texts))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("the language")
    }

    /// FAILS IF: a language lacks one of the row's words, a reason a consent
    /// was not kept, or the sentence of its key.
    #[test]
    fn every_word_of_the_row_is_in_every_language() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            for why in [
                ronoctua_consent::Unkept::NoHome,
                ronoctua_consent::Unkept::Unparseable,
                ronoctua_consent::Unkept::Unwritable,
            ] {
                let key = format!("consent-unkept-{}", why.id());
                assert!(t.has(&key), "{language}: {key}");
                assert!(unkept(why, &["a.example"], &t).contains("a.example"));
            }
            assert!(t.has(ALLOW_ID), "{language}");
            assert!(t.has(&format!("about-{ALLOW_ID}")), "{language}");
            assert!(label(&["www.ecb.europa.eu"], &t).contains("www.ecb.europa.eu"));
        }
    }

    /// FAILS IF: the hosts are not all named, or the row's note is cut
    /// instead of wrapped.
    #[test]
    fn the_row_names_every_host_and_wraps_its_note() {
        let t = texts("en-GB");
        let two = label(&["a.example", "b.example"], &t);
        assert!(
            two.contains("a.example") && two.contains("b.example"),
            "{two}"
        );
        let three = hosts_text(&["a.example", "b.example", "c.example"], &t);
        assert!(three.contains("a.example, b.example"), "{three}");
        let drawn = lines(
            Ask {
                hosts: &["www.ecb.europa.eu"],
                what: "calc-rates-for",
            },
            20,
            false,
            &t,
        );
        assert!(drawn.len() > 2, "{drawn:?}");
        assert!(
            drawn
                .iter()
                .all(|(l, _)| ronoctua_render::layout::width(l) <= 20)
        );
        let all: String = drawn
            .iter()
            .map(|(l, _)| l.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!all.contains('…'), "a note is wrapped and not cut: {all}");
    }
}
