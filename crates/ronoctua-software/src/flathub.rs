//! flathub.org's search and details API.
//!
//! Three endpoints, all public and read-only (`https://flathub.org/api/v2`,
//! measured 2026-09-26):
//!
//! * `POST /search?locale=<l>` with `{"query","filters","hits_per_page","page"}`
//!   — ranked by installs, only `desktop-application` and
//!   `console-application`, and only apps with an icon (the server filters).
//! * `GET /appstream/<id>?locale=<l>` — name, summary, the description as
//!   appstream's HTML-ish markup, urls, licence, verification.
//! * `GET /summary/<id>` — download and installed size, the runtime and ITS
//!   installed size, and the permissions from the app's metadata. For a
//!   runtime id it lists every branch with its own sizes.
//!
//! Every network call goes through [`crate::net`] (the one framer, our
//! User-Agent, a ceiling of [`MOST`]); everything that reads an answer is a
//! PURE function over bytes, so the tests run on captured answers and never
//! on the network. No rate limit is published and none is sent in headers;
//! the screen debounces, and a 429 or 503 is [`Refusal::RateLimited`].

use std::path::PathBuf;

use serde_json::Value;

use crate::found::{Badge, Found, Picture};
use crate::{AppKey, Refusal};

/// The API's root.
pub const API: &str = "https://flathub.org/api/v2";

/// The hosts this catalogue asks — its API and its pictures — as the
/// person's consent row lets them (`consent.json`): nothing is asked before.
pub const HOSTS: &[&str] = &["flathub.org", "dl.flathub.org"];

/// The largest answer read: a search of [`MOST_HITS`] carries full
/// descriptions and every keyword (37 KB for 21 hits, measured), an appstream
/// answer every screenshot size.
pub const MOST: u64 = 2 * 1024 * 1024;

/// The most hits asked for in one search.
pub const MOST_HITS: usize = 60;

/// The locale as the API wants it: a BCP 47 tag with a DASH and the region in
/// capitals — `pt-BR`, never `pt_BR`.
///
/// Measured: `locale=pt_BR` answers in ENGLISH (the backend's translation keys
/// are `language.replace("_", "-")`, and the fallback is the part before a
/// dash, which `pt_BR` does not have), `locale=pt-br` answers in European
/// Portuguese (it falls back to `pt`), and only `pt-BR` answers "Calculadora —
/// Efetue cálculos…". The backend maps `zh-CN`/`zh-TW` to `zh-Hans`/`zh-Hant`,
/// so a tag it would not find is mapped the same way here. Anything that is
/// not a tag (`C`, empty, a path) is `en`.
#[must_use]
pub fn api_locale(language: &str) -> String {
    let raw = language
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-");
    let parts: Vec<String> = raw
        .split('-')
        .enumerate()
        .map(|(i, p)| match (i, p.len()) {
            (0, _) => p.to_ascii_lowercase(),
            (_, 2) => p.to_ascii_uppercase(),
            (_, 4) => {
                let mut s = p.to_ascii_lowercase();
                s[..1].make_ascii_uppercase();
                s
            }
            _ => p.to_owned(),
        })
        .collect();
    let tag = parts.join("-");
    let tag = match tag.as_str() {
        "zh-CN" | "zh-SG" => "zh-Hans".to_owned(),
        "zh-TW" | "zh-HK" | "zh-MO" => "zh-Hant".to_owned(),
        _ => tag,
    };
    let valid = parts
        .first()
        .is_some_and(|l| (2..=3).contains(&l.len()) && l.chars().all(|c| c.is_ascii_alphabetic()))
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 8 && p.chars().all(|c| c.is_ascii_alphanumeric()));
    if valid { tag } else { "en".to_owned() }
}

/// The body of a search.
#[must_use]
pub fn search_body(query: &str, limit: usize) -> String {
    serde_json::json!({
        "query": query,
        "filters": [],
        "hits_per_page": limit.clamp(1, MOST_HITS),
        "page": 1,
    })
    .to_string()
}

fn text_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn owned_at(v: &Value, key: &str) -> Option<String> {
    text_at(v, key).map(str::to_owned)
}

fn texts_at<'a>(v: &'a Value, key: &str) -> Vec<&'a str> {
    v.get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

fn flag_at(v: &Value, key: &str) -> Option<bool> {
    match v.get(key)? {
        Value::Bool(b) => Some(*b),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn number_at(v: &Value, key: &str) -> Option<u64> {
    match v.get(key)? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Every search hit as a row, in the API's order. A hit whose `app_id` is not
/// a valid id is dropped (it would never reach a command line). `query` decides
/// [`Found::by_description`]: a hit whose name, id and keywords do not contain
/// it was found by what it does.
///
/// # Errors
///
/// [`Refusal::ToolFailed`] when the answer is not a search answer.
pub fn parse_search(bytes: &[u8], query: &str) -> Result<Vec<Found>, Refusal> {
    let answer: Value = serde_json::from_slice(bytes).map_err(|_| Refusal::ToolFailed)?;
    let hits = answer
        .get("hits")
        .and_then(Value::as_array)
        .ok_or(Refusal::ToolFailed)?;
    Ok(hits
        .iter()
        .filter_map(|h| found_of_hit(h, query, crate::flatpak::arch()))
        .collect())
}

/// One hit as a row, on a machine of `arch`.
fn found_of_hit(hit: &Value, query: &str, arch: &str) -> Option<Found> {
    let id = text_at(hit, "app_id").filter(|id| crate::flatpak::valid_id(id))?;
    let license = owned_at(hit, "project_license");
    let mut badges = vec![Badge::Sandboxed];
    if flag_at(hit, "verification_verified") == Some(true) {
        badges.push(Badge::Verified);
    }
    let free = flag_at(hit, "is_free_license").unwrap_or_else(|| {
        !license
            .as_deref()
            .is_some_and(crate::appstream::is_proprietary)
    });
    if !free {
        badges.push(Badge::Unfree);
    }
    if text_at(hit, "type") == Some("console-application") {
        badges.push(Badge::CommandLine);
    }
    let arches = texts_at(hit, "arches");
    if !arches.is_empty()
        && !arches.contains(&arch)
        && arches
            .iter()
            .all(|a| a.starts_with("aarch64") || a.starts_with("arm"))
    {
        badges.push(Badge::ArmOnly);
    }
    let fold = ronoctua_apps::fold::fold;
    let q = fold(query.trim());
    let by_name = q.is_empty()
        || [text_at(hit, "name").unwrap_or_default(), id]
            .iter()
            .any(|s| fold(s).contains(&q))
        || texts_at(hit, "keywords")
            .iter()
            .chain(&texts_at(hit, "localized_keywords"))
            .any(|k| fold(k).contains(&q));
    Some(Found {
        key: AppKey::Flatpak { id: id.to_owned() },
        name: text_at(hit, "name").unwrap_or(id).to_owned(),
        summary: owned_at(hit, "summary").unwrap_or_default(),
        version: String::new(),
        badges,
        download_bytes: None,
        icon: owned_at(hit, "icon").map(|url| Picture::Url { url }),
        homepage: None,
        license,
        publisher: owned_at(hit, "developer_name"),
        program: None,
        by_description: !by_name,
    })
}

/// Appstream's HTML-ish description as plain paragraphs: `<p>`, `<li>` and the
/// other blocks each end one, inline tags (`<em>`, `<code>`) are dropped, the
/// five XML entities and numeric ones are decoded, and whitespace collapses.
///
/// A list item is a paragraph of its own. The mark that shows it came from a
/// list is the screen's to draw, from a token: a bullet written here would be a
/// codepoint baked into data (invariant 9).
#[must_use]
pub fn paragraphs_of(html: &str) -> Vec<String> {
    const BLOCKS: [&str; 13] = [
        "p",
        "li",
        "ul",
        "ol",
        "br",
        "div",
        "h1",
        "h2",
        "h3",
        "h4",
        "dd",
        "dt",
        "blockquote",
    ];
    let mut out = Vec::new();
    let mut current = String::new();
    let flush = |current: &mut String, out: &mut Vec<String>| {
        let squashed = current.split_whitespace().collect::<Vec<_>>().join(" ");
        if !squashed.is_empty() {
            out.push(squashed);
        }
        current.clear();
    };
    let mut rest = html;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            let Some(end) = rest.find('>') else {
                break;
            };
            let tag = rest[1..end]
                .trim_start_matches('/')
                .split(|c: char| c.is_whitespace() || c == '/')
                .next();
            if tag.is_some_and(|t| BLOCKS.contains(&t.to_ascii_lowercase().as_str())) {
                flush(&mut current, &mut out);
            }
            rest = &rest[end + 1..];
        } else if c == '&' {
            let entity = rest[1..]
                .find(';')
                .filter(|n| *n <= 10)
                .map(|n| &rest[1..=n]);
            let decoded = entity.and_then(|e| match e {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => e.strip_prefix('#').and_then(|n| {
                    n.strip_prefix(['x', 'X'])
                        .map_or_else(|| n.parse().ok(), |h| u32::from_str_radix(h, 16).ok())
                        .and_then(char::from_u32)
                }),
            });
            match (entity, decoded) {
                (Some(e), Some(ch)) => {
                    current.push(ch);
                    rest = &rest[e.len() + 2..];
                }
                _ => {
                    current.push('&');
                    rest = &rest[1..];
                }
            }
        } else {
            current.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    flush(&mut current, &mut out);
    out
}

/// What `/appstream/<id>` says about an app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Appstream {
    /// Its name, in the locale asked for.
    pub name: String,
    /// One line on what it does.
    pub summary: String,
    /// Plain paragraphs.
    pub description: Vec<String>,
    /// Its homepage.
    pub homepage: Option<String>,
    /// The licence expression.
    pub license: Option<String>,
    /// Who publishes it.
    pub developer: Option<String>,
    /// Flathub verified the publisher.
    pub verified: bool,
    /// The licence is free (`None` when the answer did not say).
    pub free: Option<bool>,
    /// Its icon's address.
    pub icon: Option<String>,
    /// The runtime it needs, from its bundle.
    pub runtime: Option<String>,
    /// The newest stable release's version.
    pub version: String,
    /// The app itself reached end of life.
    pub eol: bool,
}

/// Read an `/appstream/<id>` answer.
///
/// # Errors
///
/// [`Refusal::ToolFailed`] when it is not an appstream answer.
pub fn parse_appstream(bytes: &[u8]) -> Result<Appstream, Refusal> {
    let v: Value = serde_json::from_slice(bytes).map_err(|_| Refusal::ToolFailed)?;
    let name = owned_at(&v, "name").ok_or(Refusal::ToolFailed)?;
    let metadata = v.get("metadata").cloned().unwrap_or(Value::Null);
    let version = v
        .get("releases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|r| text_at(r, "type") != Some("development"))
        .filter_map(|r| {
            Some((
                number_at(r, "timestamp").unwrap_or(0),
                text_at(r, "version")?,
            ))
        })
        .max_by_key(|(when, _)| *when)
        .map(|(_, version)| version.to_owned())
        .unwrap_or_default();
    Ok(Appstream {
        name,
        summary: owned_at(&v, "summary").unwrap_or_default(),
        description: text_at(&v, "description")
            .map(paragraphs_of)
            .unwrap_or_default(),
        homepage: v.get("urls").and_then(|u| owned_at(u, "homepage")),
        license: owned_at(&v, "project_license"),
        developer: owned_at(&v, "developer_name"),
        verified: flag_at(&metadata, "flathub::verification::verified") == Some(true),
        free: flag_at(&v, "is_free_license"),
        icon: owned_at(&v, "icon"),
        runtime: v.get("bundle").and_then(|b| owned_at(b, "runtime")),
        version,
        eol: flag_at(&v, "is_eol") == Some(true),
    })
}

/// What `/summary/<id>` says about an app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// Bytes the app itself downloads.
    pub download_bytes: Option<u64>,
    /// Bytes the app itself takes once installed.
    pub installed_bytes: Option<u64>,
    /// The runtime it needs (`org.gnome.Platform/x86_64/51`).
    pub runtime: Option<String>,
    /// The runtime's own name (`GNOME Application Platform version 51`).
    pub runtime_name: Option<String>,
    /// Bytes the runtime takes once installed.
    pub runtime_installed_bytes: Option<u64>,
    /// The runtime reached end of life.
    pub runtime_eol: bool,
    /// What the app may reach, as [`crate::flatpak::permission_line`] words.
    pub permissions: Vec<String>,
}

/// Read a `/summary/<id>` answer.
///
/// # Errors
///
/// [`Refusal::ToolFailed`] when it is not a summary answer.
pub fn parse_summary(bytes: &[u8]) -> Result<Summary, Refusal> {
    let v: Value = serde_json::from_slice(bytes).map_err(|_| Refusal::ToolFailed)?;
    let metadata = v
        .get("metadata")
        .filter(|m| m.is_object())
        .ok_or(Refusal::ToolFailed)?;
    Ok(Summary {
        download_bytes: number_at(&v, "download_size"),
        installed_bytes: number_at(&v, "installed_size"),
        runtime: owned_at(metadata, "runtime"),
        runtime_name: owned_at(metadata, "runtimeName"),
        runtime_installed_bytes: number_at(metadata, "runtimeInstalledSize"),
        runtime_eol: flag_at(metadata, "runtimeIsEol") == Some(true),
        permissions: metadata
            .get("permissions")
            .map(permission_lines_of)
            .unwrap_or_default(),
    })
}

/// The permissions object of a summary as lines, in the same words as an
/// installed app's ([`crate::flatpak::permission_lines`]), so the sheet says
/// one thing the same way before and after installing.
#[must_use]
pub fn permission_lines_of(permissions: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(groups) = permissions.as_object() else {
        return out;
    };
    for (key, value) in groups {
        match (key.as_str(), value) {
            ("session-bus" | "system-bus", Value::Object(policies)) => {
                for (policy, names) in policies {
                    for name in names
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                    {
                        out.push(crate::flatpak::bus_line(key == "system-bus", name, policy));
                    }
                }
            }
            ("environment", Value::Object(vars)) => {
                out.extend(
                    vars.iter()
                        .filter_map(|(k, v)| Some(format!("env={k}={}", v.as_str()?))),
                );
            }
            (_, Value::Array(values)) => {
                out.extend(
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|v| crate::flatpak::permission_line(key, v)),
                );
            }
            _ => {}
        }
    }
    out
}

/// A runtime's sizes on one branch, from `/summary/<runtime id>`:
/// (download, installed).
#[must_use]
pub fn runtime_sizes(bytes: &[u8], branch: &str) -> Option<(u64, u64)> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    let b = v.get("branches")?.get(branch)?;
    Some((
        number_at(b, "download_size")?,
        number_at(b, "installed_size")?,
    ))
}

/// A runtime ref's three parts, `name/arch/branch`, each safe to put in a
/// path and a URL.
#[must_use]
pub fn runtime_parts(runtime: &str) -> Option<(&str, &str, &str)> {
    let mut parts = runtime.split('/');
    let (name, arch, branch) = (parts.next()?, parts.next()?, parts.next()?);
    let plain = |s: &str| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    };
    (parts.next().is_none() && crate::flatpak::valid_id(name) && plain(arch) && plain(branch))
        .then_some((name, arch, branch))
}

/// Whether a runtime is installed in any of these installations (their roots:
/// `~/.local/share/flatpak`, `/var/lib/flatpak`). A `--user` app runs on a
/// runtime from the SYSTEM installation too, so both count.
#[must_use]
pub fn runtime_present(installations: &[PathBuf], runtime: &str) -> bool {
    runtime_parts(runtime).is_some_and(|(name, arch, branch)| {
        installations.iter().any(|root| {
            root.join("runtime")
                .join(name)
                .join(arch)
                .join(branch)
                .join("active")
                .exists()
        })
    })
}

/// What a sheet shows about one app: description, sizes (download and
/// installed, with the runtime when it is missing), permissions, licence,
/// publisher, verification.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Details {
    /// Plain paragraphs (HTML reduced).
    pub description: Vec<String>,
    /// Bytes to download: the app, plus its runtime when [`Details::runtime_missing`]
    /// and the runtime's size could be had.
    pub download_bytes: Option<u64>,
    /// Bytes once installed, counted the same way.
    pub installed_bytes: Option<u64>,
    /// The runtime it needs.
    pub runtime: Option<String>,
    /// What it may reach, one line each (`share=network`, `filesystem=home`, …).
    pub permissions: Vec<String>,
    /// Its name, in the locale asked for (data).
    pub name: String,
    /// One line on what it does.
    pub summary: String,
    /// Its homepage.
    pub homepage: Option<String>,
    /// The licence expression.
    pub license: Option<String>,
    /// Who publishes it.
    pub publisher: Option<String>,
    /// Flathub verified the publisher.
    pub verified: bool,
    /// The licence is not free.
    pub unfree: bool,
    /// Its icon's address.
    pub icon: Option<String>,
    /// The newest stable version.
    pub version: String,
    /// The runtime's own name.
    pub runtime_name: Option<String>,
    /// The runtime, or the app, reached end of life.
    pub end_of_life: bool,
    /// Bytes the runtime downloads (known only when it was asked, because it
    /// is missing).
    pub runtime_download_bytes: Option<u64>,
    /// Bytes the runtime takes once installed.
    pub runtime_installed_bytes: Option<u64>,
    /// Whether the runtime still has to be installed: `None` when nobody
    /// looked ([`details_with`] always does).
    pub runtime_missing: Option<bool>,
}

/// The details from their parts. Pure: [`details_with`] fetches them.
///
/// The runtime is ADDED to both sizes only when it is missing — the first
/// GNOME app costs a gigabyte, the second a few megabytes — and its download
/// is added only when it was known. What flatpak also pulls beside a runtime
/// (its locale and the GL extension) is not in any answer, so the sum is a
/// floor: the job's own table (`< 386.1 MB`) is the exact word.
#[must_use]
pub fn combine(
    app: Appstream,
    summary: Summary,
    runtime_download: Option<u64>,
    runtime_missing: Option<bool>,
) -> Details {
    let missing = runtime_missing == Some(true);
    let add = |app: Option<u64>, runtime: Option<u64>| match (app, runtime, missing) {
        (Some(a), Some(r), true) => Some(a + r),
        (a, _, _) => a,
    };
    Details {
        download_bytes: add(summary.download_bytes, runtime_download),
        installed_bytes: add(summary.installed_bytes, summary.runtime_installed_bytes),
        runtime: summary.runtime.or(app.runtime),
        permissions: summary.permissions,
        description: app.description,
        name: app.name,
        summary: app.summary,
        homepage: app.homepage,
        unfree: app.free == Some(false)
            || app
                .license
                .as_deref()
                .is_some_and(crate::appstream::is_proprietary),
        license: app.license,
        publisher: app.developer,
        verified: app.verified,
        icon: app.icon,
        version: app.version,
        runtime_name: summary.runtime_name,
        end_of_life: app.eol || summary.runtime_eol,
        runtime_download_bytes: runtime_download,
        runtime_installed_bytes: summary.runtime_installed_bytes,
        runtime_missing,
    }
}

/// Flathub's search, through `net`: rows in the API's order (by installs),
/// names and summaries in `language`, verified publishers marked
/// ([`parse_search`]).
///
/// # Errors
///
/// The network's refusal, the status's ([`crate::net::answered`]), or
/// `ToolFailed` for an answer that is not a search.
pub fn search_with(
    net: &dyn crate::net::Fetch,
    query: &str,
    language: &str,
    limit: usize,
) -> Result<Vec<Found>, Refusal> {
    let url = format!("{}/search?locale={}", API, api_locale(language));
    let got = net
        .post_json(&url, &search_body(query, limit), MOST)
        .and_then(crate::net::answered)
        .map_err(|f| f.refusal.unwrap_or(Refusal::ToolFailed))?;
    let mut found = parse_search(&got.reply.body, query)?;
    found.truncate(limit);
    Ok(found)
}

/// Flathub's details through `net`: the app's appstream (in `language`) and
/// summary, and — when its runtime is in none of `installations` — the
/// runtime's summary too, so both sizes are what THIS person would pay
/// ([`combine`]).
///
/// # Errors
///
/// [`Refusal::UnknownItem`] for an id that is not one or that Flathub does
/// not have, the network's refusal, and `ToolFailed` for an answer that does
/// not parse. The runtime's own answer failing is not an error: the sizes then
/// count no download for it.
pub fn details_with(
    net: &dyn crate::net::Fetch,
    installations: &[std::path::PathBuf],
    id: &str,
    language: &str,
) -> Result<Details, Refusal> {
    if !crate::flatpak::valid_id(id) {
        return Err(Refusal::UnknownItem);
    }
    let get = |url: &str| {
        net.get(url, &[], MOST)
            .and_then(crate::net::answered)
            .map(|g| g.reply.body)
            .map_err(|f| f.refusal.unwrap_or(Refusal::ToolFailed))
    };
    let locale = api_locale(language);
    let app = parse_appstream(&get(&format!("{API}/appstream/{id}?locale={locale}"))?)?;
    let summary = parse_summary(&get(&format!("{API}/summary/{id}"))?)?;
    let Some(runtime) = summary.runtime.clone().or_else(|| app.runtime.clone()) else {
        return Ok(combine(app, summary, None, None));
    };
    let missing = !runtime_present(installations, &runtime);
    let download = match runtime_parts(&runtime) {
        Some((name, _, branch)) if missing => get(&format!("{API}/summary/{name}"))
            .ok()
            .and_then(|b| runtime_sizes(&b, branch))
            .map(|(download, _)| download),
        _ => None,
    };
    Ok(combine(app, summary, download, Some(missing)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH: &[u8] =
        include_bytes!("../tests/fixtures/flatpak/flathub-search-calculator.json");
    const SPOTIFY: &[u8] = include_bytes!("../tests/fixtures/flatpak/flathub-search-spotify.json");
    const APPSTREAM: &[u8] =
        include_bytes!("../tests/fixtures/flatpak/flathub-appstream-calculator.json");
    const SUMMARY: &[u8] =
        include_bytes!("../tests/fixtures/flatpak/flathub-summary-calculator.json");
    const FIREFOX: &[u8] = include_bytes!("../tests/fixtures/flatpak/flathub-summary-firefox.json");
    const PLATFORM: &[u8] =
        include_bytes!("../tests/fixtures/flatpak/flathub-summary-platform.json");

    #[test]
    fn the_locale_is_spelled_the_way_the_api_finds_it() {
        assert_eq!(api_locale("pt-BR"), "pt-BR");
        assert_eq!(api_locale("pt_BR"), "pt-BR");
        assert_eq!(api_locale("pt_BR.UTF-8"), "pt-BR");
        assert_eq!(api_locale("pt-br"), "pt-BR");
        assert_eq!(api_locale("en-GB"), "en-GB");
        assert_eq!(api_locale("de"), "de");
        assert_eq!(api_locale("zh_CN"), "zh-Hans");
        assert_eq!(api_locale("zh-hant"), "zh-Hant");
        assert_eq!(api_locale("sr@latin"), "sr");
        for nonsense in ["", "C", "POSIX", "../x", "pt BR", "a"] {
            assert_eq!(api_locale(nonsense), "en", "{nonsense:?}");
        }
    }

    #[test]
    fn the_search_body_is_the_api_s_shape() {
        let body: Value = serde_json::from_str(&search_body("calc \"x\"", 500)).unwrap();
        assert_eq!(body["query"], "calc \"x\"");
        assert_eq!(body["hits_per_page"], MOST_HITS);
        assert_eq!(body["page"], 1);
        assert_eq!(body["filters"], serde_json::json!([]));
    }

    #[test]
    fn a_search_answer_becomes_rows_in_the_api_s_order() {
        let rows = parse_search(SEARCH, "calculator").unwrap();
        assert_eq!(
            rows.iter().map(|r| r.key.id()).collect::<Vec<_>>(),
            [
                "org.gnome.Calculator",
                "org.kde.kalk",
                "dev.edfloreshz.Calculator"
            ]
        );
        let calc = &rows[0];
        assert_eq!(calc.name, "Calculator");
        assert_eq!(
            calc.summary,
            "Perform arithmetic, scientific or financial calculations"
        );
        assert_eq!(calc.badges, [Badge::Sandboxed, Badge::Verified]);
        assert_eq!(calc.license.as_deref(), Some("GPL-3.0-or-later"));
        assert_eq!(calc.publisher.as_deref(), Some("The GNOME Project"));
        assert!(
            matches!(&calc.icon, Some(Picture::Url { url }) if url.starts_with("https://dl.flathub.org/"))
        );
        assert!(!calc.by_description);
        assert!(
            !parse_search(SEARCH, "arithmetic").unwrap()[0].by_description,
            "a keyword is a name match"
        );
        assert!(
            parse_search(SEARCH, "equation")
                .unwrap()
                .iter()
                .any(|r| r.by_description)
        );
    }

    #[test]
    fn a_proprietary_app_says_so() {
        let rows = parse_search(SPOTIFY, "spotify").unwrap();
        assert_eq!(
            rows[0].key,
            AppKey::Flatpak {
                id: "com.spotify.Client".into()
            }
        );
        assert!(rows[0].badges.contains(&Badge::Unfree));
        assert!(!rows[0].badges.contains(&Badge::Verified));
    }

    #[test]
    fn an_app_built_only_for_arm_says_so_on_a_pc() {
        let hit = serde_json::json!({"app_id": "org.x.Y", "name": "Y", "arches": ["aarch64"], "is_free_license": true});
        assert!(
            found_of_hit(&hit, "y", "x86_64")
                .unwrap()
                .badges
                .contains(&Badge::ArmOnly)
        );
        assert!(
            !found_of_hit(&hit, "y", "aarch64")
                .unwrap()
                .badges
                .contains(&Badge::ArmOnly)
        );
        let both = serde_json::json!({"app_id": "org.x.Y", "arches": ["aarch64", "x86_64"]});
        assert!(
            !found_of_hit(&both, "y", "x86_64")
                .unwrap()
                .badges
                .contains(&Badge::ArmOnly)
        );
        assert_eq!(
            found_of_hit(&serde_json::json!({"app_id": "-rf"}), "", "x86_64"),
            None
        );
    }

    #[test]
    fn garbage_is_a_refusal_and_not_an_empty_list() {
        assert_eq!(parse_search(b"<html>", "x"), Err(Refusal::ToolFailed));
        assert_eq!(
            parse_search(b"{\"detail\":\"x\"}", "x"),
            Err(Refusal::ToolFailed)
        );
        assert_eq!(parse_appstream(b"{}"), Err(Refusal::ToolFailed));
        assert_eq!(
            parse_summary(b"{\"detail\":\"App not found\"}"),
            Err(Refusal::ToolFailed)
        );
    }

    #[test]
    fn the_description_is_plain_paragraphs() {
        assert_eq!(
            paragraphs_of(
                "<p>A &amp; B&#33;</p>\n<ul>\n <li>One</li><li>Two <em>x</em>&nbsp;y</li></ul><p>AT&T &bogus; 5 < 6</p>"
            ),
            ["A & B!", "One", "Two x y", "AT&T &bogus; 5"],
        );
        assert!(paragraphs_of("").is_empty());
    }

    #[test]
    fn an_appstream_answer_becomes_what_a_sheet_shows() {
        let a = parse_appstream(APPSTREAM).unwrap();
        assert_eq!(a.name, "Calculator");
        assert_eq!(
            a.homepage.as_deref(),
            Some("https://apps.gnome.org/Calculator")
        );
        assert_eq!(a.license.as_deref(), Some("GPL-3.0-or-later"));
        assert_eq!(a.developer.as_deref(), Some("The GNOME Project"));
        assert!(a.verified);
        assert_eq!(a.free, Some(true));
        assert_eq!(
            a.version, "51.0",
            "the development release is not the version"
        );
        assert_eq!(a.runtime.as_deref(), Some("org.gnome.Platform/x86_64/51"));
        assert!(
            a.description[0]
                .starts_with("Calculator is an application that solves mathematical equations.")
        );
        assert!(
            a.description
                .iter()
                .all(|p| !p.contains('<') && !p.contains("  "))
        );
    }

    #[test]
    fn a_summary_gives_sizes_the_runtime_and_permissions() {
        let s = parse_summary(SUMMARY).unwrap();
        assert_eq!(s.download_bytes, Some(1_921_122));
        assert_eq!(s.installed_bytes, Some(4_975_616));
        assert_eq!(s.runtime.as_deref(), Some("org.gnome.Platform/x86_64/51"));
        assert_eq!(s.runtime_installed_bytes, Some(1_125_792_768));
        assert!(!s.runtime_eol);
        for want in [
            "share=ipc",
            "share=network",
            "socket=wayland",
            "socket=fallback-x11",
            "device=dri",
        ] {
            assert!(s.permissions.iter().any(|p| p == want), "{want}");
        }
        let f = parse_summary(FIREFOX).unwrap();
        for want in [
            "own-name=org.mozilla.firefox.*",
            "talk-name=org.a11y.Bus",
            "filesystem=xdg-download",
            "device=all",
        ] {
            assert!(
                f.permissions.iter().any(|p| p == want),
                "{want} in {:?}",
                f.permissions
            );
        }
    }

    #[test]
    fn a_runtime_s_branch_has_its_own_sizes() {
        let (download, installed) = runtime_sizes(PLATFORM, "51").unwrap();
        assert!(download > 100_000_000 && installed > download);
        assert_eq!(runtime_sizes(PLATFORM, "99"), None);
    }

    #[test]
    fn a_missing_runtime_is_added_to_both_sizes() {
        let app = parse_appstream(APPSTREAM).unwrap();
        let summary = parse_summary(SUMMARY).unwrap();
        let present = combine(app.clone(), summary.clone(), None, Some(false));
        assert_eq!(
            (present.download_bytes, present.installed_bytes),
            (Some(1_921_122), Some(4_975_616))
        );
        let missing = combine(app.clone(), summary.clone(), Some(419_660_190), Some(true));
        assert_eq!(missing.download_bytes, Some(1_921_122 + 419_660_190));
        assert_eq!(missing.installed_bytes, Some(4_975_616 + 1_125_792_768));
        // Missing, and the runtime's download could not be had: the installed
        // size still counts it, the download stays the app's.
        let unknown = combine(app, summary, None, Some(true));
        assert_eq!(unknown.download_bytes, Some(1_921_122));
        assert_eq!(unknown.installed_bytes, Some(4_975_616 + 1_125_792_768));
        assert!(!unknown.unfree && unknown.verified);
    }

    #[test]
    fn a_runtime_is_present_in_either_installation() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-software-flathub-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (user, system) = (dir.join("user"), dir.join("system"));
        std::fs::create_dir_all(system.join("runtime/org.gnome.Platform/x86_64/51/active"))
            .unwrap();
        let both = [user, system];
        assert!(runtime_present(&both, "org.gnome.Platform/x86_64/51"));
        assert!(!runtime_present(&both, "org.gnome.Platform/x86_64/50"));
        assert!(!runtime_present(
            &both,
            "org.gnome.Platform/x86_64/../../51"
        ));
        assert_eq!(
            runtime_parts("org.kde.Platform/x86_64/6.9"),
            Some(("org.kde.Platform", "x86_64", "6.9"))
        );
        assert_eq!(runtime_parts("org.kde.Platform/x86_64/.."), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
