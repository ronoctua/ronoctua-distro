//! The languages of content: the ones a subtitle is searched in and a film's
//! streams are tagged with.
//!
//! Not the languages this system SPEAKS (two, `assets/i18n/languages.nix`):
//! these are declared in `assets/tongues.nix`, and `nix/tongues.nix` turns
//! them into [`TONGUES`] — refusing a row of the wrong shape, a code twice,
//! or a language neither subtitle service searches in.
//!
//! # One row, four spellings
//!
//! A [`Tongue`]'s `code` is what this system writes and matches (a subtitle
//! file's name, a door's search, the text key `tongue-<code>`); the other
//! three are how ISO and each service spell the same language. The door names
//! a language by its key, never by one of these.
//!
//! # The default search
//!
//! The interface's language, then English ([`default_for`]).

use serde::{Deserialize, Serialize};

/// One language of content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tongue {
    /// The tag this system writes and matches: `en`, `pt-BR`, `zh-TW`.
    pub code: &'static str,
    /// Its name's text key, `tongue-<code>` in lowercase.
    pub key: &'static str,
    /// ISO 639-1.
    pub two_letter: &'static str,
    /// ISO 639-2/B — what a Matroska stream and ffprobe say.
    pub three_letter: &'static str,
    /// The code OpenSubtitles' API takes; `None` when it has none.
    pub opensubtitles: Option<&'static str>,
    /// The code SubDL's API takes; `None` when it has none.
    pub subdl: Option<&'static str>,
}

include!(concat!(env!("OUT_DIR"), "/tongues.rs"));

/// The code of English, the second of every default search.
pub const ENGLISH: &str = "en";

/// Which subtitle service a code is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Spelling {
    /// OpenSubtitles' own codes.
    OpenSubtitles,
    /// SubDL's own codes.
    SubDl,
}

impl Tongue {
    /// Its code for `spelling`'s service, when the service has one.
    #[must_use]
    pub const fn for_service(&self, spelling: Spelling) -> Option<&'static str> {
        match spelling {
            Spelling::OpenSubtitles => self.opensubtitles,
            Spelling::SubDl => self.subdl,
        }
    }
}

/// The language a code names, however it is spelled, in this order: this
/// system's code (`pt-BR`, any case, `_` for `-`); a service's (`pt-br`,
/// `BR_PT`, and SubDL's `PT`, which is European Portuguese — a subtitle it
/// found is tagged so); ISO 639-2/B (`por`); ISO 639-1 alone, which, when two
/// rows share it, is the one `assets/tongues.nix` names first. A region this
/// table does not split (`en-GB`) is its language. `None` for a code no row
/// has.
#[must_use]
pub fn of(code: &str) -> Option<&'static Tongue> {
    let wanted = code.trim().replace('_', "-").to_ascii_lowercase();
    if wanted.is_empty() {
        return None;
    }
    let same = |a: &str| a.replace('_', "-").eq_ignore_ascii_case(&wanted);
    TONGUES
        .iter()
        .find(|t| same(t.code))
        .or_else(|| {
            TONGUES
                .iter()
                .find(|t| t.opensubtitles.is_some_and(same) || t.subdl.is_some_and(same))
        })
        .or_else(|| TONGUES.iter().find(|t| same(t.three_letter)))
        .or_else(|| {
            let primary = wanted.split('-').next().unwrap_or_default();
            TONGUES.iter().find(|t| t.two_letter == primary)
        })
}

/// The text key a code is NAMED by on screen: its row's — this system's
/// code, or a service's spelling ([`of`]) — except a bare language that
/// several rows share (`pt`, `por`, `zh`), which is named by that language's
/// own key with no region ([`SHARED`]): a film's stream tagged `por`, or a
/// radio's list saying `pt`, is Portuguese, and calling it Brazilian or
/// European would be a guess on screen. `None` for a code no row has.
#[must_use]
pub fn name_key(code: &str) -> Option<&'static str> {
    let wanted = code.trim().replace('_', "-").to_ascii_lowercase();
    if !wanted.contains('-') && !TONGUES.iter().any(|t| t.code.eq_ignore_ascii_case(&wanted)) {
        let mut rows = TONGUES
            .iter()
            .filter(|t| t.two_letter == wanted || t.three_letter == wanted);
        if let (Some(first), Some(_)) = (rows.next(), rows.next())
            && let Some((_, key)) = SHARED.iter().find(|(two, _)| *two == first.two_letter)
        {
            return Some(key);
        }
    }
    of(code).map(|t| t.key)
}

/// The languages a search asks in when the person added none: the
/// interface's (`pt-BR` → Brazilian Portuguese, `en-GB` → English), then
/// English, each once.
#[must_use]
pub fn default_for(interface: &str) -> Vec<&'static Tongue> {
    let mut out: Vec<&'static Tongue> = Vec::new();
    for t in [of(interface), of(ENGLISH)].into_iter().flatten() {
        if !out.iter().any(|o| o.code == t.code) {
            out.push(t);
        }
    }
    out
}

/// The languages `codes` name, each once, in the order given; a code no row
/// has is left out (and so is a repeat).
#[must_use]
pub fn all_of(codes: &[String]) -> Vec<&'static Tongue> {
    let mut out: Vec<&'static Tongue> = Vec::new();
    for t in codes.iter().filter_map(|c| of(c)) {
        if !out.iter().any(|o| o.code == t.code) {
            out.push(t);
        }
    }
    out
}

/// `tongues` as `spelling`'s service spells them, each once; a language the
/// service does not have is left out.
#[must_use]
pub fn spelled(tongues: &[&Tongue], spelling: Spelling) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for code in tongues.iter().filter_map(|t| t.for_service(spelling)) {
        if !out.iter().any(|o| o == code) {
            out.push(code.to_owned());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a language stops being found by one of the spellings a
    /// service answers with — a found subtitle tagged `pt-br` or `BR_PT`
    /// would be saved without its language.
    #[test]
    fn a_language_is_found_by_every_spelling() {
        let br = of("pt-BR").expect("Brazilian Portuguese");
        assert_eq!(br.code, "pt-BR");
        assert_eq!(of("pt_br"), Some(br));
        assert_eq!(of("PT-BR"), Some(br));
        assert_eq!(of("BR_PT"), Some(br), "SubDL's spelling");
        assert_eq!(of("por"), Some(br), "the first row of its language");
        // SUBDL'S `PT` IS EUROPEAN PORTUGUESE, and a subtitle it found in
        // `PT` is tagged `pt`: a service's spelling wins over ISO's.
        assert_eq!(of("pt").map(|t| t.code), Some("pt-PT"));
        assert_eq!(of("zh").map(|t| t.code), Some("zh-CN"));
        assert_eq!(of("en-GB").map(|t| t.code), Some("en"));
        assert_eq!(of("zh-tw").map(|t| t.code), Some("zh-TW"));
        assert_eq!(of("ZH_BG").map(|t| t.code), Some("zh-TW"));
        assert_eq!(of("xx"), None);
        assert_eq!(of("  "), None);
    }

    /// FAILS IF: a bare code two rows share is named as one of them — a
    /// stream tagged `por` called Brazilian — or a code that IS a row's
    /// stops being named by it.
    #[test]
    fn a_language_two_rows_share_is_named_without_a_region() {
        assert_eq!(name_key("por"), Some("tongue-pt"));
        assert_eq!(name_key("pt"), Some("tongue-pt"));
        assert_eq!(name_key("PT"), Some("tongue-pt"));
        assert_eq!(name_key("zh"), Some("tongue-zh"));
        assert_eq!(name_key("pt-BR"), Some("tongue-pt-br"));
        assert_eq!(
            name_key("pt-br"),
            Some("tongue-pt-br"),
            "OpenSubtitles' spelling"
        );
        assert_eq!(name_key("BR_PT"), Some("tongue-pt-br"), "SubDL's");
        assert_eq!(name_key("eng"), Some("tongue-en"));
        assert_eq!(name_key("en"), Some("tongue-en"));
        assert_eq!(name_key("xx"), None);
        for (two, key) in SHARED {
            assert!(TONGUES.iter().filter(|t| t.two_letter == *two).count() > 1);
            assert_eq!(*key, format!("tongue-{two}"));
        }
    }

    /// FAILS IF: the default search stops being the interface's language
    /// and English — or says English twice.
    #[test]
    fn the_default_search_is_the_interfaces_language_then_english() {
        let codes = |v: Vec<&Tongue>| v.iter().map(|t| t.code).collect::<Vec<_>>();
        assert_eq!(codes(default_for("pt-BR")), ["pt-BR", "en"]);
        assert_eq!(codes(default_for("en-GB")), ["en"]);
        assert_eq!(codes(default_for("")), ["en"]);
    }

    /// FAILS IF: a service is asked in a language it does not have, or twice
    /// in one.
    #[test]
    fn a_service_is_asked_only_in_its_own_spellings() {
        let asked = all_of(&["pt-BR".into(), "hy".into(), "pt-br".into(), "nope".into()]);
        assert_eq!(asked.len(), 2);
        assert_eq!(spelled(&asked, Spelling::OpenSubtitles), ["pt-br", "hy"]);
        assert_eq!(
            spelled(&asked, Spelling::SubDl),
            ["BR_PT"],
            "SubDL has no Armenian"
        );
    }

    /// FAILS IF: the generated table stops being what its generator
    /// promises — a key that is not the code's, a code twice.
    #[test]
    fn every_row_has_its_own_key_and_code() {
        assert!(TONGUES.len() >= 50, "{}", TONGUES.len());
        for (n, t) in TONGUES.iter().enumerate() {
            assert_eq!(t.key, format!("tongue-{}", t.code.to_ascii_lowercase()));
            assert!(
                t.opensubtitles.is_some() || t.subdl.is_some(),
                "{}: nobody searches in it",
                t.code
            );
            assert!(
                TONGUES[n + 1..].iter().all(|o| o.code != t.code),
                "{} twice",
                t.code
            );
        }
        assert!(of(ENGLISH).is_some());
    }
}
