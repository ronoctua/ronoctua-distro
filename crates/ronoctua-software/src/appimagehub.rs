//! appimage.github.io's catalogue (`feed.json`), cached for a day.
//!
//! # What the feed is (measured 2026-09-26)
//!
//! A JSON Feed version 1 of 837 KB and 1,450 items. An item has a `name`, an
//! HTML `description` (absent from about one in nine), `categories`, and then
//! four fields that are each `null` often enough to matter: `authors` (313),
//! `license` (829), `links` (the `GitHub` link is `owner/repo`, never a URL)
//! and `icons` (paths relative to [`DATABASE`]). There are NO versions, NO
//! file addresses and NO hashes: what to download is answered by the item's
//! GitHub releases ([`crate::github`]), and an item without a GitHub link can
//! only be pointed at, never installed.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::found::{Found, Picture};
use crate::github::Fetch;
use crate::{AppKey, Failure, Refusal, Roots};

/// The catalogue's address.
pub const FEED: &str = "https://appimage.github.io/feed.json";

/// The hosts this catalogue asks — its feed and database, and GitHub's API
/// for the releases it points at — as the person's consent row lets them.
pub const HOSTS: &[&str] = &["appimage.github.io", "api.github.com"];
/// What an item's relative icon path is relative to.
pub const DATABASE: &str = "https://appimage.github.io/database/";
/// The most the feed may be: 4 MiB, five times what it is.
pub const MOST: u64 = 4 << 20;
/// How long a fetched feed is used without asking again: a day.
pub const FRESH: u64 = 24 * 3600;
/// The format of what is kept, in the file's name.
pub const FORMAT: u32 = 1;

/// One program of the catalogue, reduced to what a row and an install read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Its name (data).
    pub name: String,
    /// The key an install gives it ([`crate::slug_of`] of the name).
    pub key: String,
    /// The description as plain text, paragraphs joined by a blank line.
    pub description: String,
    /// Its categories.
    pub categories: Vec<String>,
    /// Who made it (the first author's name).
    pub publisher: Option<String>,
    /// Its licence.
    pub license: Option<String>,
    /// `owner/repo`, when the item names its GitHub repository — the only way
    /// it can be installed.
    pub github: Option<String>,
    /// Its pages (`Download` and `Install` links), for "open the page".
    pub pages: Vec<String>,
    /// Its icon's absolute address.
    pub icon: Option<String>,
}

impl Item {
    /// The one line a row shows: the first sentence of the description.
    #[must_use]
    pub fn summary(&self) -> String {
        let first = self.description.split("\n\n").next().unwrap_or_default();
        let sentence = first.split_inclusive(". ").next().unwrap_or(first).trim();
        cut(sentence, 200)
    }

    /// The row a search shows.
    #[must_use]
    pub fn found(&self, by_description: bool) -> Found {
        Found {
            key: AppKey::AppImage {
                key: self.key.clone(),
            },
            name: self.name.clone(),
            summary: self.summary(),
            version: String::new(),
            badges: Vec::new(),
            download_bytes: None,
            icon: self.icon.clone().map(|url| Picture::Url { url }),
            homepage: self
                .github
                .as_ref()
                .map(|r| format!("https://github.com/{r}"))
                .or_else(|| self.pages.first().cloned()),
            license: self.license.clone(),
            publisher: self.publisher.clone(),
            program: None,
            by_description,
        }
    }
}

fn cut(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(most - 1).collect();
    out.push('…');
    out
}

/// The feed, one item at a time: an item is read on its own, so ONE odd item
/// is one item missing and not the whole catalogue. Measured: ten items of
/// the live feed carry `"categories": [null]`, and a strict reading of the
/// list refused all 1,450.
#[derive(Deserialize)]
struct RawFeed {
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

/// Every list may hold a `null`; every field may be `null` or absent.
#[derive(Deserialize)]
struct RawItem {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    categories: Option<Vec<Option<String>>>,
    #[serde(default)]
    authors: Option<Vec<Option<RawAuthor>>>,
    #[serde(default)]
    license: Option<serde_json::Value>,
    #[serde(default)]
    links: Option<Vec<Option<RawLink>>>,
    #[serde(default)]
    icons: Option<Vec<Option<String>>>,
}

#[derive(Deserialize)]
struct RawAuthor {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct RawLink {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

/// The catalogue out of the feed's bytes. An item whose name folds to no key
/// (all of it outside ASCII) is left out: it could not be named on disk.
///
/// # Errors
///
/// `ToolFailed` for bytes that are not a feed.
pub fn parse(bytes: &[u8]) -> Result<Vec<Item>, Refusal> {
    let feed: RawFeed = serde_json::from_slice(bytes).map_err(|_| Refusal::ToolFailed)?;
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::with_capacity(feed.items.len());
    for value in feed.items {
        let Ok(raw) = serde_json::from_value::<RawItem>(value) else {
            continue;
        };
        let key = crate::slug_of(&raw.name);
        // TWO ITEMS, ONE KEY: the first keeps it, so a key always means the
        // same program between a search and the install it leads to.
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }
        let links: Vec<(String, String)> = raw
            .links
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .filter_map(|l| Some((l.kind?, l.url?)))
            .collect();
        let github = links
            .iter()
            .find(|(kind, url)| kind == "GitHub" && crate::appimage::is_repo(url))
            .map(|(_, url)| url.clone());
        let pages = links
            .iter()
            .filter(|(kind, url)| {
                matches!(kind.as_str(), "Download" | "Install") && url.starts_with("https://")
            })
            .map(|(_, url)| url.clone())
            .collect();
        let icon = raw
            .icons
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .find_map(|i| icon_url(&i));
        out.push(Item {
            description: plain(raw.description.as_deref().unwrap_or_default()),
            categories: raw
                .categories
                .unwrap_or_default()
                .into_iter()
                .flatten()
                .collect(),
            publisher: raw
                .authors
                .unwrap_or_default()
                .into_iter()
                .flatten()
                .find_map(|a| a.name),
            license: raw
                .license
                .and_then(|l| l.as_str().map(str::to_owned))
                .filter(|l| !l.trim().is_empty()),
            name: raw.name,
            key,
            github,
            pages,
            icon,
        });
    }
    Ok(out)
}

/// An icon path of the feed as an address, or `None` for one that would
/// leave the database (`..`) or is not a picture a launcher reads.
fn icon_url(path: &str) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    if path.starts_with("https://") {
        return Some(path.to_owned());
    }
    let fine = !path.starts_with('/')
        && !path.split('/').any(|c| c.is_empty() || c == "..")
        && (lower.ends_with(".png") || lower.ends_with(".svg"));
    fine.then(|| format!("{DATABASE}{path}"))
}

/// HTML reduced to plain paragraphs: tags out, the five entities and numeric
/// ones decoded, whitespace folded, a block element ends a paragraph.
#[must_use]
pub fn plain(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        text.push_str(&rest[..at]);
        let Some(end) = rest[at..].find('>') else {
            rest = "";
            break;
        };
        let tag = rest[at + 1..at + end]
            .trim_start_matches('/')
            .to_ascii_lowercase();
        let name = tag
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default();
        if matches!(
            name,
            "p" | "br" | "li" | "ul" | "ol" | "h1" | "h2" | "h3" | "h4" | "div"
        ) {
            text.push('\u{1}');
        }
        rest = &rest[at + end + 1..];
    }
    text.push_str(rest);
    let decoded = entities(&text);
    decoded
        .split('\u{1}')
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let end = tail[..tail.len().min(12)].find(';');
        let decoded = end.and_then(|e| {
            let name = &tail[1..e];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                if !c.is_control() {
                    out.push(c);
                }
                rest = &tail[e + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Search a catalogue: names first (exact, then prefix, then anywhere, all
/// folded), then descriptions, at most `limit` rows.
#[must_use]
pub fn search_in(items: &[Item], query: &str, limit: usize) -> Vec<Found> {
    let q = ronoctua_apps::fold::fold(query.trim());
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, String, &Item)> = items
        .iter()
        .filter_map(|i| {
            let name = ronoctua_apps::fold::fold(&i.name);
            let tier = if name == q {
                0
            } else if name.starts_with(&q) {
                1
            } else if name.contains(&q) || i.key.contains(&q) {
                2
            } else if ronoctua_apps::fold::fold(&i.description).contains(&q) {
                3
            } else {
                return None;
            };
            Some((tier, name, i))
        })
        .collect();
    hits.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    hits.into_iter()
        .take(limit)
        .map(|(tier, _, i)| i.found(tier == 3))
        .collect()
}

/// Where the fetched feed is kept.
#[must_use]
pub fn kept_at(roots: &Roots) -> PathBuf {
    roots
        .ours_cache()
        .join("appimagehub")
        .join(format!("feed-v{FORMAT}.json"))
}

fn age(path: &Path, now: u64) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let at = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(now.saturating_sub(at))
}

/// Fetch the feed again and keep it, whatever the age of the kept one.
/// Reaches the network: call it on a thread.
///
/// # Errors
///
/// The network's refusal, or `ToolFailed` for an answer that is not a feed —
/// in which case what was kept stays.
pub fn refresh_with(net: &dyn Fetch, roots: &Roots) -> Result<Vec<Item>, Failure> {
    let got = net.get(FEED, &[("Accept", "application/json")], MOST)?;
    if !got.reply.went_through() {
        return Err(Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("appimage.github.io answered {}", got.reply.status),
        });
    }
    // PARSED BEFORE KEPT: a broken answer must never replace a good file.
    let items = parse(&got.reply.body).map_err(Failure::refused)?;
    let path = kept_at(roots);
    let dir = path.parent().unwrap_or(&path);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    ronoctua_xdg::write_atomically(dir, name, &got.reply.body)
        .map_err(|e| Failure::trouble(format!("could not keep the feed: {e}")))?;
    Ok(items)
}

/// The catalogue: the kept feed while it is younger than [`FRESH`], a fetched
/// one otherwise (when `online`), and the kept one of any age when fetching
/// fails. Reaches the network: call it on a thread.
///
/// # Errors
///
/// `Offline` with nothing kept (or not `online`), `ToolFailed` for a feed that
/// does not parse.
pub fn catalogue_with(
    net: &dyn Fetch,
    roots: &Roots,
    online: bool,
    now: u64,
) -> Result<Vec<Item>, Failure> {
    let path = kept_at(roots);
    let kept = || std::fs::read(&path).ok().and_then(|b| parse(&b).ok());
    if !online || age(&path, now).is_some_and(|a| a < FRESH) {
        return kept().ok_or_else(|| Failure::refused(Refusal::Offline));
    }
    match refresh_with(net, roots) {
        Ok(items) => Ok(items),
        Err(f) => kept().ok_or(f),
    }
}

/// Search the catalogue (fetching it when the cache is older than a day and
/// the network is allowed).
///
/// # Errors
///
/// `Offline` with no cache; `ToolFailed` for a feed that does not parse.
pub fn search(
    roots: &Roots,
    query: &str,
    online: bool,
    limit: usize,
) -> Result<Vec<Found>, crate::Refusal> {
    let items = catalogue_with(&crate::github::Net, roots, online, crate::appimage::now())
        .map_err(|f| f.refusal.unwrap_or(Refusal::ToolFailed))?;
    Ok(search_in(&items, query, limit))
}

/// One program of the KEPT catalogue by its key, without asking anybody.
#[must_use]
pub fn item(roots: &Roots, key: &str) -> Option<Item> {
    let bytes = std::fs::read(kept_at(roots)).ok()?;
    parse(&bytes).ok()?.into_iter().find(|i| i.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::canned::Canned;

    fn feed() -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/appimage/feed-cut.json"),
        )
        .unwrap()
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-hub-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_feed_reads_with_its_nulls() {
        let items = parse(&feed()).unwrap();
        assert_eq!(
            items.len(),
            7,
            "the item whose name is a number is the only one left out"
        );
        let odd = items.iter().find(|i| i.key == "openaudible").unwrap();
        assert!(
            odd.categories.is_empty(),
            "a null inside a list is a missing entry, not a broken feed"
        );
        let obsidian = items.iter().find(|i| i.key == "obsidian").unwrap();
        assert_eq!(
            obsidian.github.as_deref(),
            Some("obsidianmd/obsidian-releases")
        );
        assert_eq!(
            obsidian.icon.as_deref(),
            Some("https://appimage.github.io/database/Obsidian/icons/128x128/obsidian.png")
        );
        assert_eq!(obsidian.license, None, "a null licence is none");
        let inkscape = items.iter().find(|i| i.key == "inkscape").unwrap();
        assert_eq!(
            inkscape.github, None,
            "null links: it can be pointed at, not installed"
        );
        assert!(
            inkscape
                .description
                .starts_with("A Free and open source vector graphics editor.")
        );
        assert!(!inkscape.description.contains('<'));
        let krita = items.iter().find(|i| i.key == "krita").unwrap();
        assert_eq!(
            krita.icon, None,
            "an .svgz is not a picture a launcher reads"
        );
        assert_eq!(
            items
                .iter()
                .find(|i| i.key == "freecad2")
                .unwrap()
                .publisher
                .as_deref(),
            Some("FreeCAD")
        );
    }

    #[test]
    fn html_becomes_paragraphs_and_entities_characters() {
        assert_eq!(
            plain("<p>One &amp; two</p>\n<p>Three&#33; <b>four</b></p>"),
            "One & two\n\nThree! four"
        );
        assert_eq!(plain("<ul><li>a</li><li>b</li></ul>"), "a\n\nb");
        assert_eq!(plain("fish &chips; &x"), "fish &chips; &x");
        assert_eq!(plain("no closing <b"), "no closing");
    }

    #[test]
    fn names_come_before_descriptions() {
        let items = parse(&feed()).unwrap();
        let rows = search_in(&items, "audac", 10);
        assert_eq!(rows[0].name, "Audacity");
        assert!(!rows[0].by_description);
        let rows = search_in(&items, "vector graphics", 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Inkscape");
        assert!(rows[0].by_description);
        assert_eq!(rows[0].homepage, None);
        assert!(search_in(&items, "  ", 10).is_empty());
        assert_eq!(search_in(&items, "a", 2).len(), 2, "the limit holds");
    }

    #[test]
    fn a_fetched_feed_is_kept_and_a_fresh_one_is_not_asked_again() {
        let dir = scratch("keep");
        let roots = Roots::under(&dir);
        let net = Canned::new(vec![(FEED, 200, vec![], feed())]);
        let now = crate::appimage::now();
        assert_eq!(catalogue_with(&net, &roots, true, now).unwrap().len(), 7);
        let none = Canned::new(Vec::new());
        assert_eq!(catalogue_with(&none, &roots, true, now).unwrap().len(), 7);
        assert!(none.asked.borrow().is_empty(), "a day has not passed");
        // A day later the network is asked, and its failure falls back.
        assert_eq!(
            catalogue_with(&none, &roots, true, now + FRESH + 5)
                .unwrap()
                .len(),
            7
        );
        assert_eq!(none.asked.borrow().len(), 1);
        assert_eq!(
            item(&roots, "upscayl").unwrap().github.as_deref(),
            Some("upscayl/upscayl")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_answer_never_replaces_a_good_feed() {
        let dir = scratch("broken");
        let roots = Roots::under(&dir);
        refresh_with(&Canned::new(vec![(FEED, 200, vec![], feed())]), &roots).unwrap();
        let bad = Canned::new(vec![(FEED, 200, vec![], b"<html>".to_vec())]);
        assert_eq!(
            refresh_with(&bad, &roots).unwrap_err().refusal,
            Some(Refusal::ToolFailed)
        );
        assert!(item(&roots, "obsidian").is_some());
        let off = Canned::new(Vec::new());
        let fresh = scratch("none");
        let f = catalogue_with(&off, &Roots::under(&fresh), false, 0).unwrap_err();
        assert_eq!(f.refusal, Some(Refusal::Offline));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&fresh);
    }
}
