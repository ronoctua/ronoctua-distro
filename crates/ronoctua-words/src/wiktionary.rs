//! Wiktionary, online and keyless (`en.wiktionary.org`) — for every tongue
//! the offline dictionary does not hold, and for English where it is absent.
//!
//! # Two questions to one host
//!
//! Definitions come from the REST API
//! (`/api/rest_v1/page/definition/<word>`), which answers every language's
//! entries of the word keyed by the language's Wiktionary code (`pt`, `en`),
//! each definition an HTML fragment. It carries no synonyms, so those come
//! from the page's WIKITEXT (`/w/api.php?action=parse&prop=wikitext`): the
//! `{{syn|<code>|…}}` lines under a sense and the `{{l|<code>|…}}` items of a
//! `Synonyms` section. Both carry the language's code, so the section's
//! English name is never needed. One host, so one consent covers both.
//!
//! Wiktionary's codes are the primary subtags this crate's tongues have:
//! `pt-BR` asks for `pt`.

use serde::Deserialize;

use crate::ask::{Request, escape};
use crate::html::{before_a_list, text_of};
use crate::{Part, Sense, Source, Tongue, WIKTIONARY_HOST, Why};

/// The most senses kept of one word: a common word has dozens across its
/// etymologies, and a card shows a handful.
pub const MOST_SENSES: usize = 32;

/// The page of a word, as Wiktionary titles it: spaces as underscores, and
/// the case the person typed (Wiktionary's titles are case-sensitive).
fn title_of(word: &str) -> String {
    word.split_whitespace().collect::<Vec<_>>().join("_")
}

/// The question for the definitions of `word`. Pure.
#[must_use]
pub fn definition_request(word: &str) -> Request {
    Request::get(
        WIKTIONARY_HOST,
        format!(
            "https://{WIKTIONARY_HOST}/api/rest_v1/page/definition/{}",
            escape(&title_of(word))
        ),
    )
}

/// The question for the page's wikitext, where the synonyms are. Pure.
#[must_use]
pub fn wikitext_request(word: &str) -> Request {
    Request::get(
        WIKTIONARY_HOST,
        format!(
            "https://{WIKTIONARY_HOST}/w/api.php?action=parse&page={}&prop=wikitext&format=json&formatversion=2&redirects=1",
            escape(&title_of(word))
        ),
    )
}

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "partOfSpeech", default)]
    part: String,
    #[serde(default)]
    definitions: Vec<Definition>,
}

#[derive(Deserialize)]
struct Definition {
    #[serde(default)]
    definition: String,
    #[serde(default)]
    examples: Vec<String>,
}

/// The senses of the word in `tongue`, out of the REST answer. Pure.
///
/// # Errors
///
/// [`Why::NotFound`] when the page has no entry in that tongue (or none with
/// a definition), [`Why::Garbled`] for what is not its JSON.
pub fn read_definitions(body: &[u8], tongue: &Tongue) -> Result<Vec<Sense>, Why> {
    let all: std::collections::BTreeMap<String, serde_json::Value> =
        serde_json::from_slice(body).map_err(|_| Why::Garbled)?;
    let Some(entries) = all.get(tongue.primary()) else {
        return Err(Why::NotFound);
    };
    let entries: Vec<Entry> = serde_json::from_value(entries.clone()).map_err(|_| Why::Garbled)?;
    let mut out = Vec::new();
    for entry in entries {
        let part = part_of(&entry.part);
        for d in entry.definitions {
            let gloss = text_of(before_a_list(&d.definition));
            if gloss.is_empty() {
                continue;
            }
            out.push(Sense {
                part: Some(part),
                gloss,
                examples: d
                    .examples
                    .iter()
                    .map(|e| text_of(e))
                    .filter(|e| !e.is_empty())
                    .collect(),
                synonyms: Vec::new(),
                source: Source::Wiktionary,
            });
            if out.len() == MOST_SENSES {
                return Ok(out);
            }
        }
    }
    if out.is_empty() {
        return Err(Why::NotFound);
    }
    Ok(out)
}

/// Wiktionary's heading for a part of speech. Pure.
#[must_use]
pub fn part_of(heading: &str) -> Part {
    match heading.trim().to_ascii_lowercase().as_str() {
        "noun" => Part::Noun,
        "proper noun" => Part::ProperNoun,
        "verb" => Part::Verb,
        "adjective" | "participle" => Part::Adjective,
        "adverb" => Part::Adverb,
        "pronoun" => Part::Pronoun,
        "preposition" | "postposition" | "contraction" => Part::Preposition,
        "conjunction" => Part::Conjunction,
        "interjection" => Part::Interjection,
        "numeral" | "number" => Part::Numeral,
        "determiner" | "article" => Part::Determiner,
        "phrase" | "idiom" | "proverb" | "prepositional phrase" => Part::Phrase,
        "prefix" | "suffix" | "infix" | "affix" | "circumfix" | "interfix" => Part::Affix,
        _ => Part::Other,
    }
}

#[derive(Deserialize)]
struct Parsed {
    parse: Option<Page>,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    wikitext: String,
}

/// The page's wikitext, out of the parse answer. Pure.
///
/// # Errors
///
/// [`Why::NotFound`] for a page that does not exist (the API says so in an
/// `error`), [`Why::Garbled`] for what is not its JSON.
pub fn read_wikitext(body: &[u8]) -> Result<String, Why> {
    let parsed: Parsed = serde_json::from_slice(body).map_err(|_| Why::Garbled)?;
    if parsed.error.is_some() {
        return Err(Why::NotFound);
    }
    parsed.parse.map(|p| p.wikitext).ok_or(Why::Garbled)
}

/// The synonyms of `word` in `tongue` the wikitext names, each once, the word
/// itself left out: every `{{syn|<code>|…}}` (and `{{synonyms|…}}`), and the
/// `{{l|<code>|…}}` items of a `Synonyms` section. Pure.
#[must_use]
pub fn synonyms_in(wikitext: &str, word: &str, tongue: &Tongue) -> Vec<String> {
    let code = tongue.primary();
    let mut out: Vec<String> = Vec::new();
    let mut add = |s: &str| {
        let s = s.trim();
        if !s.is_empty() && s != word.trim() && !out.iter().any(|o| o == s) {
            out.push(s.to_owned());
        }
    };
    let mut in_synonyms = false;
    for line in wikitext.lines() {
        let heading = line.trim();
        if heading.starts_with("==") && heading.ends_with("==") {
            in_synonyms = heading
                .trim_matches('=')
                .trim()
                .eq_ignore_ascii_case("synonyms");
            continue;
        }
        for (name, params) in templates(line) {
            let listed = matches!(name.as_str(), "syn" | "synonyms" | "syn-lite");
            let linked = in_synonyms && matches!(name.as_str(), "l" | "link" | "l-self");
            if !(listed || linked) || params.first().map(String::as_str) != Some(code) {
                continue;
            }
            let terms = if listed {
                &params[1..]
            } else {
                &params[1..params.len().min(2)]
            };
            for term in terms {
                add(&clean_term(term));
            }
        }
    }
    out
}

/// The templates of a line: each `{{name|a|b|…}}` as its name and its
/// positional parameters (a named one, `q=…`, is left out). Nested templates
/// inside a parameter are skipped with it. Pure.
fn templates(line: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find("{{") {
        let body_start = open + 2;
        let mut depth = 1;
        let mut k = body_start;
        let bytes = rest.as_bytes();
        while k + 1 < bytes.len() && depth > 0 {
            if bytes[k] == b'{' && bytes[k + 1] == b'{' {
                depth += 1;
                k += 2;
            } else if bytes[k] == b'}' && bytes[k + 1] == b'}' {
                depth -= 1;
                k += 2;
            } else {
                k += 1;
            }
        }
        if depth > 0 {
            break;
        }
        let inner = &rest[body_start..k - 2];
        let mut parts = split_top(inner).into_iter();
        let name = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        let params = parts
            .filter(|p| !p.contains('=') && !p.contains("{{"))
            .map(|p| p.trim().to_owned())
            .collect();
        out.push((name, params));
        rest = &rest[k..];
    }
    out
}

/// `inner` split at the `|` that are not inside a nested template or a link.
fn split_top(inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '[' if chars.peek() == Some(&c) => {
                depth += 1;
                current.push(c);
                current.push(chars.next().unwrap_or(c));
            }
            '}' | ']' if chars.peek() == Some(&c) => {
                depth -= 1;
                current.push(c);
                current.push(chars.next().unwrap_or(c));
            }
            '|' if depth == 0 => out.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    out.push(current);
    out
}

/// A term as a template writes it, as a person reads it: without its inline
/// modifiers (`lar<q:colloquial>`), a thesaurus prefix, or link brackets.
fn clean_term(term: &str) -> String {
    let term = term.split('<').next().unwrap_or_default();
    let term = term.strip_prefix("Thesaurus:").unwrap_or(term);
    term.replace("[[", "").replace("]]", "").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of the REST answer for `casa`, cut down: every language's
    /// entries keyed by code, definitions as HTML with the sub-senses folded
    /// into the first.
    const CASA: &str = r#"{
      "en": [{"partOfSpeech":"Noun","language":"English","definitions":[
        {"definition":"<span class=\"usage-label-sense\"></span> <a rel=\"mw:WikiLink\" href=\"/wiki/house\" title=\"house\">house</a>","examples":["Get out of my <b>casa</b><span typeof=\"mw:Entity\">!</span>"]}]}],
      "pt": [{"partOfSpeech":"Noun","language":"Portuguese","definitions":[
        {"definition":""},
        {"definition":"<a rel=\"mw:WikiLink\" href=\"/wiki/house\" title=\"house\">house</a>\n<ol><li>structure serving as an abode of human beings","examples":["Aquela <b>casa</b> é grande."]},
        {"definition":"structure serving as an abode of human beings","examples":["Aquela <b>casa</b> é grande."]},
        {"definition":"<a rel=\"mw:WikiLink\" href=\"/wiki/home\" title=\"home\">home</a> <span class=\"mention-gloss-paren\">(</span><span class=\"mention-gloss\">one’s own dwelling place</span><span class=\"mention-gloss-paren\">)</span>"}]},
        {"partOfSpeech":"Verb","language":"Portuguese","definitions":[
        {"definition":"<a href=\"/wiki/marry\">to marry</a>"}]}]
    }"#;

    #[test]
    fn the_definitions_of_a_tongue_are_its_entries_read_as_text() {
        let pt = Tongue::new("pt-BR").unwrap();
        let senses = read_definitions(CASA.as_bytes(), &pt).unwrap();
        let glosses: Vec<&str> = senses.iter().map(|s| s.gloss.as_str()).collect();
        assert_eq!(
            glosses,
            [
                "house",
                "structure serving as an abode of human beings",
                "home (one’s own dwelling place)",
                "to marry"
            ]
        );
        assert_eq!(senses[0].examples, ["Aquela casa é grande."]);
        assert_eq!(senses[3].part, Some(Part::Verb));
        assert!(senses.iter().all(|s| s.source == Source::Wiktionary));
        let en = read_definitions(CASA.as_bytes(), &Tongue::new("en").unwrap()).unwrap();
        assert_eq!(en[0].examples, ["Get out of my casa!"]);
        assert_eq!(
            read_definitions(CASA.as_bytes(), &Tongue::new("fr").unwrap()),
            Err(Why::NotFound)
        );
        assert_eq!(read_definitions(b"<html>", &pt), Err(Why::Garbled));
    }

    #[test]
    fn the_addresses_carry_the_title_escaped() {
        assert_eq!(
            definition_request("olá mundo").url,
            "https://en.wiktionary.org/api/rest_v1/page/definition/ol%C3%A1_mundo"
        );
        let r = wikitext_request("casa");
        assert!(
            r.url
                .starts_with("https://en.wiktionary.org/w/api.php?action=parse&page=casa&")
        );
        assert_eq!(r.host, WIKTIONARY_HOST);
        assert!(r.headers.is_empty(), "keyless");
    }

    /// The shape of `casa`'s wikitext: synonyms under a sense, in another
    /// language's section, and in a `Synonyms` section of their own.
    const WIKITEXT: &str = "==Portuguese==\n\
        ===Noun===\n\
        # [[house]]\n\
        #: {{syn|pt|lar<q:formal>|moradia|Thesaurus:habitação}}\n\
        # [[noble]] family\n\
        ##: {{syn|pt|dinastia}}\n\
        ====Synonyms====\n\
        * {{l|pt|residência}}, {{l|pt|[[domicílio]]}}\n\
        * {{l|pt|lar}}\n\
        ====Derived terms====\n\
        * {{l|pt|casinha}}\n\
        ==Spanish==\n\
        # {{syn|es|hogar|q=common}}\n";

    /// What `WIKITEXT` holds for Portuguese, in its order: data, like the
    /// wikitext itself.
    const PT_SYNONYMS: [&str; 6] = [
        "lar",
        "moradia",
        "habitação",
        "dinastia",
        "residência",
        "domicílio",
    ];

    #[test]
    fn synonyms_are_read_out_of_the_wikitext_for_one_tongue() {
        let pt = Tongue::new("pt-BR").unwrap();
        assert_eq!(synonyms_in(WIKITEXT, "casa", &pt), PT_SYNONYMS);
        assert_eq!(
            synonyms_in(WIKITEXT, "casa", &Tongue::new("es").unwrap()),
            ["hogar"]
        );
        assert!(synonyms_in(WIKITEXT, "casa", &Tongue::new("en").unwrap()).is_empty());
    }

    #[test]
    fn a_missing_page_is_not_found() {
        let missing =
            br#"{"error":{"code":"missingtitle","info":"The page you specified doesn't exist."}}"#;
        assert_eq!(read_wikitext(missing), Err(Why::NotFound));
        assert_eq!(
            read_wikitext(br#"{"parse":{"title":"casa","wikitext":"==Portuguese=="}}"#).unwrap(),
            "==Portuguese=="
        );
        assert_eq!(read_wikitext(b"x"), Err(Why::Garbled));
    }
}
