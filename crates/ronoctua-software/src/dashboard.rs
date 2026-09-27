//! Dashboard Icons (dashboardicons.com, homarr-labs/dashboard-icons): the
//! metadata, searched by name and alias. Pure over the metadata bytes.
//!
//! # The metadata, and not the tree
//!
//! The repository also publishes a `tree.json`, and it is out of date: 972
//! names of the metadata are missing from it while their files exist (measured
//! 2026-09-26). The metadata is one JSON object keyed by name — 3 327 entries,
//! 1.2 MB — whose `base` says which format is the ORIGINAL (`svg` for 2 613,
//! `png` for 714: those have no SVG at all), whose `aliases` are what makes
//! `code repository` find `github`, and whose `colors` names the drawings made
//! for one background.
//!
//! # What `colors` means
//!
//! `colors.light` names the LIGHT-COLOURED drawing — white strokes, the one
//! for a DARK background — and `colors.dark` the dark-coloured one. Measured on
//! `spotify-light.svg`, whose fill is `#fff`. The names are not always
//! `<name>-<key>`: in 415 of 416 exceptions one of the two is the base drawing
//! itself (`github`'s `dark` is `github`), so the file name is read from the
//! metadata and never built.

use serde::Deserialize;

/// One icon of the collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    /// Its name (`whatsapp`).
    pub name: String,
    /// Its aliases.
    pub aliases: Vec<String>,
    /// `svg` or `png`: which format is the original.
    pub base: String,
    /// The variant for dark backgrounds, when there is one.
    pub for_dark: Option<String>,
    /// The variant for light backgrounds, when there is one.
    pub for_light: Option<String>,
    /// Its categories, as the collection writes them (`Email-Providers`).
    pub categories: Vec<String>,
}

impl Icon {
    /// Every drawing of it, as `(file name, variant word)`: the base first
    /// (variant `""`), then the one for a dark background (`"light"`, the
    /// metadata's key: a light-coloured drawing), then the one for a light
    /// background (`"dark"`). A variant whose file IS the base is not listed
    /// twice.
    #[must_use]
    pub fn drawings(&self) -> Vec<(&str, &'static str)> {
        let mut out = vec![(self.name.as_str(), "")];
        for (file, word) in [(&self.for_dark, "light"), (&self.for_light, "dark")] {
            if let Some(f) = file.as_deref()
                && !out.iter().any(|(seen, _)| *seen == f)
            {
                out.push((f, word));
            }
        }
        out
    }
}

/// The metadata's URL (jsDelivr).
pub const METADATA: &str = "https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/metadata.json";

/// How much the metadata may weigh before it is refused: four times what it
/// weighed when this was written (1 181 244 bytes).
pub const METADATA_MOST: u64 = 4 << 20;

/// One entry as the metadata writes it.
#[derive(Deserialize)]
struct Entry {
    #[serde(default)]
    base: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    colors: Colors,
}

/// The `colors` object.
#[derive(Deserialize, Default)]
struct Colors {
    light: Option<String>,
    dark: Option<String>,
}

/// Parse the metadata. An entry that does not read is skipped, not fatal: one
/// malformed icon upstream should not take the other three thousand with it.
/// Sorted by name.
#[must_use]
pub fn parse(metadata: &[u8]) -> Vec<Icon> {
    let Ok(map) = serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(metadata)
    else {
        return Vec::new();
    };
    let mut out: Vec<Icon> = map
        .into_iter()
        .filter(|(name, _)| is_file_name(name))
        .filter_map(|(name, value)| {
            let e: Entry = serde_json::from_value(value).ok()?;
            let base = if e.base == "png" { "png" } else { "svg" }.to_owned();
            let file = |f: Option<String>| f.filter(|f| is_file_name(f));
            Some(Icon {
                name,
                aliases: e.aliases,
                base,
                for_dark: file(e.colors.light),
                for_light: file(e.colors.dark),
                categories: e.categories,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Whether a name can be put in a URL path as it is: the collection's are
/// kebab-case, and one that is not is not fetched.
pub(crate) fn is_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && !name.starts_with('.')
}

/// A query or a name reduced to what compares: folded, lower case, and letters
/// and digits only — so `Proton Mail`, `proton-mail` and `protonmail` are one.
fn squashed(s: &str) -> String {
    ronoctua_apps::fold::fold(s)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// Search by name and alias, best first.
///
/// The tiers, in order: the name IS the query, the name starts with it, the
/// name contains it, an alias starts with it, an alias contains it. Inside a
/// tier the shorter name wins (`whatsapp` before `whatsapp-business`), then
/// the alphabet.
#[must_use]
pub fn search<'a>(all: &'a [Icon], query: &str, limit: usize) -> Vec<&'a Icon> {
    let q = squashed(query);
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, &Icon)> = all
        .iter()
        .filter_map(|icon| {
            let n = squashed(&icon.name);
            let tier = if n == q {
                0
            } else if n.starts_with(&q) {
                1
            } else if n.contains(&q) {
                2
            } else {
                let aliases: Vec<String> = icon.aliases.iter().map(|a| squashed(a)).collect();
                if aliases.iter().any(|a| a.starts_with(&q)) {
                    3
                } else if aliases.iter().any(|a| a.contains(&q)) {
                    4
                } else {
                    return None;
                }
            };
            Some((tier, icon))
        })
        .collect();
    hits.sort_by(|(ta, a), (tb, b)| {
        ta.cmp(tb)
            .then(a.name.len().cmp(&b.name.len()))
            .then(a.name.cmp(&b.name))
    });
    hits.into_iter().take(limit).map(|(_, i)| i).collect()
}

/// A query from a web address: the name a site is known by, which is the
/// label before its public suffix — `web.whatsapp.com` → `whatsapp`,
/// `mail.proton.me` → `proton`, `bbc.co.uk` → `bbc`. A heuristic and not the
/// public-suffix list: the two-letter country codes with a generic second
/// level (`co.uk`, `com.br`) are the one case worth handling, and a miss only
/// costs a worse first guess in a list the person can still type into. Pure.
#[must_use]
pub fn query_of_host(host: &str) -> String {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    let generic = ["co", "com", "org", "net", "gov", "ac", "edu"];
    let site = match labels.as_slice() {
        [] => "",
        [only] => only,
        [.., before, second, tld] if tld.len() == 2 && generic.contains(second) => before,
        [.., site, _tld] => site,
    };
    site.to_owned()
}

/// The URL of one icon in one format (`png` at 512 px, or `svg`).
#[must_use]
pub fn url_of(name: &str, format: &str) -> String {
    format!("https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/{format}/{name}.{format}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slice of the real metadata (Apache-2.0), fetched 2026-09-26 from
    /// [`METADATA`], with the `update` field — who changed an entry and when —
    /// left out.
    fn all() -> Vec<Icon> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/web/dashboard-metadata.json");
        parse(&std::fs::read(path).unwrap())
    }

    fn names(hits: &[&Icon]) -> Vec<String> {
        hits.iter().map(|i| i.name.clone()).collect()
    }

    #[test]
    fn the_metadata_reads_with_its_variants_the_right_way_round() {
        let all = all();
        assert!(all.len() >= 30, "{}", all.len());
        let spotify = all.iter().find(|i| i.name == "spotify").unwrap();
        // `spotify-light` is WHITE: the drawing for a dark background.
        assert_eq!(spotify.for_dark.as_deref(), Some("spotify-light"));
        assert_eq!(spotify.for_light.as_deref(), Some("spotify-dark"));
        let github = all.iter().find(|i| i.name == "github").unwrap();
        assert_eq!(
            github.drawings(),
            [("github", ""), ("github-light", "light")],
            "the base is not listed twice"
        );
        let ntopng = all.iter().find(|i| i.name == "ntopng").unwrap();
        assert_eq!(ntopng.base, "png");
        let gmail = all.iter().find(|i| i.name == "gmail").unwrap();
        assert!(gmail.aliases.contains(&"google-mail".to_owned()));
        assert!(
            all.windows(2).all(|w| w[0].name < w[1].name),
            "sorted by name"
        );
    }

    #[test]
    fn the_search_ranks_the_name_before_the_alias() {
        let all = all();
        assert_eq!(names(&search(&all, "WhatsApp", 3)), ["whatsapp"]);
        // Separators and case do not matter.
        assert_eq!(names(&search(&all, "Proton Mail", 1)), ["proton-mail"]);
        // A prefix: the shorter name first.
        assert_eq!(
            &names(&search(&all, "proton", 3))[..],
            ["proton-mail", "proton-drive", "proton-calendar"]
        );
        let youtube = names(&search(&all, "youtube", 5));
        assert_eq!(
            youtube[..2],
            ["youtube".to_owned(), "youtube-music".to_owned()]
        );
        // An alias reaches what the name does not.
        assert_eq!(names(&search(&all, "code repository", 5)), ["github"]);
        assert!(names(&search(&all, "google mail", 5)).contains(&"gmail".to_owned()));
        assert!(search(&all, "", 5).is_empty());
        assert!(search(&all, "--", 5).is_empty());
        assert_eq!(search(&all, "a", 2).len(), 2, "the limit holds");
    }

    #[test]
    fn a_host_becomes_the_name_a_site_is_known_by() {
        assert_eq!(query_of_host("web.whatsapp.com"), "whatsapp");
        assert_eq!(query_of_host("mail.proton.me"), "proton");
        assert_eq!(query_of_host("github.com"), "github");
        assert_eq!(query_of_host("www.bbc.co.uk"), "bbc");
        assert_eq!(query_of_host("globo.com.br"), "globo");
        assert_eq!(query_of_host("localhost"), "localhost");
        assert_eq!(query_of_host("Example.ORG."), "example");
        assert_eq!(query_of_host(""), "");
    }

    #[test]
    fn a_name_that_is_not_a_file_name_is_not_kept() {
        let all = parse(br#"{"ok":{"base":"svg"},"../x":{"base":"svg"},"a b":{},"v":{"colors":{"light":"../../etc"}}}"#);
        assert_eq!(
            all.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
            ["ok", "v"]
        );
        assert_eq!(all[1].for_dark, None);
        assert!(parse(b"[1,2]").is_empty());
    }

    #[test]
    fn a_url_names_the_format_twice() {
        assert_eq!(
            url_of("whatsapp", "png"),
            "https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/whatsapp.png"
        );
        assert_eq!(
            url_of("github-light", "svg"),
            "https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/svg/github-light.svg"
        );
    }
}
