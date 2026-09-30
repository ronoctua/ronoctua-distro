//! A subtitle a service found: the one shape every subtitle source answers,
//! so the door lists OpenSubtitles', SubDL's and Podnapisi's side by side.

use serde::{Deserialize, Serialize};

use crate::Service;

/// How a found subtitle is fetched — the service's own handle.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "from")]
pub enum Take {
    /// `POST /download {file_id}`, then the link it answers.
    #[serde(rename = "opensubtitles")]
    OpenSubtitles {
        /// The file's id.
        file_id: u64,
    },
    /// `https://dl.subdl.com<path>`, a zip.
    #[serde(rename = "subdl")]
    SubDl {
        /// `/subtitle/<id>.zip`.
        path: String,
    },
    /// `https://www.podnapisi.net/subtitles/<id>/download`, a zip.
    #[serde(rename = "podnapisi")]
    Podnapisi {
        /// Its id.
        id: String,
    },
}

/// One subtitle a service found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Found {
    /// Who found it.
    pub source: Service,
    /// How to fetch it.
    pub take: Take,
    /// Its language, lowercase (`en`, `pt-br`), as the service tags it.
    pub language: String,
    /// The release it was made for (`Movie.2010.1080p.BluRay-GROUP`).
    pub release: String,
    /// The file's name, when the service says.
    pub file_name: Option<String>,
    /// How many times it was downloaded, when the service says.
    pub downloads: Option<u64>,
    /// Whether it is for the hearing impaired (sounds described).
    pub hearing_impaired: bool,
    /// The frame rate it was timed for, when the service says.
    pub fps: Option<f64>,
    /// Whether it was matched by the file's own fingerprint (the moviehash),
    /// and so is timed for exactly this file.
    pub hash_match: bool,
    /// Whether a machine translated it.
    pub machine_translated: bool,
    /// The film's or the show's title, as the service knows it.
    pub title: Option<String>,
    /// Its year.
    pub year: Option<u16>,
    /// The season, for an episode.
    pub season: Option<u16>,
    /// The episode.
    pub episode: Option<u16>,
}

/// In the order a person wants them: by language, then those matched by the
/// fingerprint, then by a person's translation over a machine's, then the most
/// downloaded. Stable for equal ones.
pub fn rank(found: &mut [Found]) {
    found.sort_by(|a, b| {
        a.language
            .cmp(&b.language)
            .then(b.hash_match.cmp(&a.hash_match))
            .then(a.machine_translated.cmp(&b.machine_translated))
            .then(b.downloads.unwrap_or(0).cmp(&a.downloads.unwrap_or(0)))
    });
}

/// The lowercase language tag of a service's code: `EN` → `en`, `BR_PT` and
/// `pt-BR` → `pt-br`. Pure.
#[must_use]
pub fn tongue(code: &str) -> String {
    let lower = code.trim().replace('_', "-").to_ascii_lowercase();
    match lower.split_once('-') {
        // SubDL writes the region FIRST (`BR_PT`); a region is two letters and
        // a language two or three, so `br-pt` is read back as `pt-br`.
        Some((region, language)) if region == "br" && language == "pt" => "pt-br".to_owned(),
        _ => lower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(language: &str, hash: bool, downloads: u64) -> Found {
        Found {
            source: Service::OpenSubtitles,
            take: Take::OpenSubtitles { file_id: downloads },
            language: language.into(),
            release: String::new(),
            file_name: None,
            downloads: Some(downloads),
            hearing_impaired: false,
            fps: None,
            hash_match: hash,
            machine_translated: false,
            title: None,
            year: None,
            season: None,
            episode: None,
        }
    }

    #[test]
    fn a_fingerprint_match_comes_first_in_its_language() {
        let mut all = vec![
            one("pt-br", false, 900),
            one("en", false, 500),
            one("en", true, 3),
            one("en", false, 800),
        ];
        rank(&mut all);
        let order: Vec<(&str, bool, Option<u64>)> = all
            .iter()
            .map(|f| (f.language.as_str(), f.hash_match, f.downloads))
            .collect();
        assert_eq!(
            order,
            [
                ("en", true, Some(3)),
                ("en", false, Some(800)),
                ("en", false, Some(500)),
                ("pt-br", false, Some(900)),
            ]
        );
    }

    #[test]
    fn a_services_code_becomes_one_tag() {
        assert_eq!(tongue("EN"), "en");
        assert_eq!(tongue("BR_PT"), "pt-br");
        assert_eq!(tongue("pt-BR"), "pt-br");
        assert_eq!(tongue("zh-CN"), "zh-cn");
    }

    #[test]
    fn a_take_says_where_it_is_from() {
        let t = Take::SubDl {
            path: "/subtitle/1-2.zip".into(),
        };
        assert_eq!(
            serde_json::to_string(&t).unwrap(),
            r#"{"from":"subdl","path":"/subtitle/1-2.zip"}"#
        );
    }
}
