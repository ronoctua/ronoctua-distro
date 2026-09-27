//! GitHub's release pages, for AppImages: the latest release's x86_64
//! `.AppImage` asset, its size and digest. Cached with `If-None-Match` (a 304
//! does not count against the 60 requests an hour).
//!
//! # What was measured (2026-09-26, the live API)
//!
//! * The asset's digest is the field `digest`, written `sha256:<hex>`, and it
//!   is `null` on some releases (Upscayl's) — so "verified" is a property of
//!   the RELEASE, never of GitHub as a whole, and the absence is the person's
//!   question (`Consent::unverified`).
//! * `releases/latest` is the newest non-draft, non-prerelease release, and it
//!   may carry NO AppImage: Obsidian's latest is a mobile release with a
//!   single `.apk`. So when the latest has no asset for us, the list of recent
//!   releases is asked once more, and the newest one that has one wins.
//! * The `ETag` of the same address alternates between two values (two
//!   backends), so a conditional request answers 304 only some of the time.
//!   That is why an answer is also simply KEPT for [`FRESH`] seconds: the
//!   cheapest request is the one never made.
//!
//! # Rate limit
//!
//! Sixty an hour per address, unauthenticated. A 403 or 429 with
//! `X-RateLimit-Remaining: 0` is [`Refusal::RateLimited`], the reset time is
//! remembered, and until then nothing is asked — a kept answer, however old,
//! is used instead when there is one.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Failure, Refusal};

pub use crate::appimage::is_repo;
// THE NETWORK IS `crate::net`'s: one request, one download, one redirect rule.
// Named here too because the modules that fetch releases, feeds and indices
// (`appimage`, `appimagehub`, `android`, `fdroid`) take it by this path.
#[cfg(test)]
pub(crate) use crate::net::canned;
pub use crate::net::{Fetch, Net};

/// One asset to download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    /// The release tag (the version).
    pub tag: String,
    /// The file name.
    pub name: String,
    /// Its download URL.
    pub url: String,
    /// Its size.
    pub size: u64,
    /// `sha256:<hex>` when GitHub gives a digest.
    pub digest: Option<String>,
}

impl Asset {
    /// The digest's hex, lower case, when it is a SHA-256 — the only kind
    /// this package can check.
    #[must_use]
    pub fn sha256(&self) -> Option<String> {
        sha256_of_digest(self.digest.as_deref()?)
    }
}

/// The hex of a `sha256:<64 hex>` digest, lower case; `None` for anything else.
#[must_use]
pub fn sha256_of_digest(digest: &str) -> Option<String> {
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

/// How long a kept answer is used without asking again: six hours.
pub const FRESH: u64 = 6 * 3600;

/// The most an API answer may be. A release with a hundred assets is about
/// 100 KiB; the list of ten releases, a megabyte at worst.
pub const MOST: u64 = 2 << 20;

/// How many recent releases the fallback reads.
pub const RECENT: usize = 10;

/// One release, reduced to what choosing an asset reads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    /// Its tag.
    pub tag: String,
    /// A draft (never offered).
    #[serde(default)]
    pub draft: bool,
    /// A pre-release (never offered).
    #[serde(default)]
    pub prerelease: bool,
    /// Its files.
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

impl RawRelease {
    fn reduced(self) -> Release {
        let tag = self.tag_name;
        Release {
            assets: self
                .assets
                .into_iter()
                .map(|a| Asset {
                    tag: tag.clone(),
                    name: a.name,
                    url: a.browser_download_url,
                    size: a.size,
                    digest: a.digest,
                })
                .collect(),
            tag,
            draft: self.draft,
            prerelease: self.prerelease,
        }
    }
}

/// One release out of `releases/latest`.
///
/// # Errors
///
/// `ToolFailed` for an answer that is not a release.
pub fn parse_release(json: &[u8]) -> Result<Release, Refusal> {
    serde_json::from_slice::<RawRelease>(json)
        .map(RawRelease::reduced)
        .map_err(|_| Refusal::ToolFailed)
}

/// The releases out of `releases?per_page=N`, newest first as GitHub lists them.
///
/// # Errors
///
/// `ToolFailed` for an answer that is not a list of releases.
pub fn parse_releases(json: &[u8]) -> Result<Vec<Release>, Refusal> {
    serde_json::from_slice::<Vec<RawRelease>>(json)
        .map(|all| all.into_iter().map(RawRelease::reduced).collect())
        .map_err(|_| Refusal::ToolFailed)
}

/// Markers that say a file is for ANOTHER machine wherever they appear.
const OTHER_MACHINES: [&str; 11] = [
    "aarch64", "arm64", "armhf", "armv7", "armv6", "armel", "i386", "i686", "ppc64", "riscv64",
    "s390x",
];

/// Which machine a file name says it is for: `Some(true)` this one,
/// `Some(false)` another, `None` when it does not say.
///
/// Substrings for the long markers, which nothing else spells, and whole
/// WORDS (split on `-`, `_`, `.` and `+`) for the short ones: `arm` and `x86`
/// inside a longer word (`charm`, the `x86` of `x86_64`) say nothing.
#[must_use]
pub fn machine_of(name: &str) -> Option<bool> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("x86_64") || lower.contains("amd64") {
        return Some(true);
    }
    if OTHER_MACHINES.iter().any(|m| lower.contains(m)) {
        return Some(false);
    }
    let mut verdict = None;
    for w in lower.split(['-', '_', '.', '+']) {
        match w {
            "x64" => verdict = Some(true),
            "arm" | "x86" | "i586" => return Some(false),
            _ => {}
        }
    }
    verdict
}

/// The asset of a release to download: the name ends `.AppImage` (any case),
/// no other machine's architecture is in it, and one that names x86_64 wins
/// over one that names none. With a `pattern` (the `*`-glob out of an
/// AppImage's own update information) only names that match are considered.
#[must_use]
pub fn choose(release: &Release, pattern: Option<&str>) -> Option<Asset> {
    if release.draft || release.prerelease {
        return None;
    }
    let mut best: Option<(u8, &Asset)> = None;
    for a in &release.assets {
        let machine = machine_of(&a.name);
        if !a.name.to_ascii_lowercase().ends_with(".appimage") || machine == Some(false) {
            continue;
        }
        if pattern.is_some_and(|p| !glob(p, &a.name)) {
            continue;
        }
        // The address must be one this crate would fetch at all.
        if crate::net::transport_of(&a.url).is_err() {
            continue;
        }
        let rank = u8::from(machine.is_none());
        if best.is_none_or(|(r, _)| rank < r) {
            best = Some((rank, a));
        }
    }
    best.map(|(_, a)| a.clone())
}

/// `*`-only glob, the shape AppImage update information uses
/// (`Obsidian-*.AppImage.zsync`).
#[must_use]
pub fn glob(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !name.starts_with(first) || name.len() < first.len() + last.len() || !name.ends_with(last) {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for middle in &parts[1..parts.len() - 1] {
        match rest.find(middle) {
            Some(at) => rest = &rest[at + middle.len()..],
            None => return false,
        }
    }
    true
}

/// What is kept of one repository's answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    /// The `ETag` of `releases/latest`, for the next conditional request.
    #[serde(default)]
    pub etag: Option<String>,
    /// When it was asked (seconds since the epoch).
    pub when: u64,
    /// The release that HAS an asset for us (the latest, or the newest of the
    /// recent ones when the latest has none); `None` when no recent release
    /// has one.
    #[serde(default)]
    pub release: Option<Release>,
}

/// The format of a kept answer, in its file name: a new shape never reads an
/// old file as if it were its own.
pub const FORMAT: u32 = 1;

fn kept_path(cache_dir: &Path, owner_repo: &str) -> std::path::PathBuf {
    cache_dir.join(format!("v{FORMAT}-{}.json", owner_repo.replace('/', "__")))
}

/// The kept answer for a repository, fresh or not, without asking anybody.
#[must_use]
pub fn kept(cache_dir: &Path, owner_repo: &str) -> Option<Kept> {
    if !is_repo(owner_repo) {
        return None;
    }
    std::fs::read(kept_path(cache_dir, owner_repo))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

fn keep(cache_dir: &Path, owner_repo: &str, kept: &Kept) {
    let path = kept_path(cache_dir, owner_repo);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    if let Err(e) = ronoctua_xdg::write_atomically(
        cache_dir,
        &name,
        &serde_json::to_vec(kept).unwrap_or_default(),
    ) {
        tracing::debug!(error = %e, "the release answer could not be kept");
    }
}

/// Until when GitHub said to stop asking, remembered across processes.
const LIMIT_FILE: &str = "rate-limit";

fn limited_until(cache_dir: &Path) -> u64 {
    std::fs::read_to_string(cache_dir.join(LIMIT_FILE))
        .ok()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0)
}

/// When a reply says the hourly allowance is spent, the moment it comes back
/// (seconds since the epoch); `None` for any other reply.
#[must_use]
pub fn spent_until(reply: &ronoctua_http::Reply, now: u64) -> Option<u64> {
    let spent = reply
        .header("x-ratelimit-remaining")
        .is_some_and(|v| v.trim() == "0");
    if !(reply.status == 429 || (reply.status == 403 && spent)) {
        return None;
    }
    let reset = reply
        .header("x-ratelimit-reset")
        .and_then(|v| v.trim().parse::<u64>().ok());
    let after = reply
        .header("retry-after")
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|s| now + s);
    Some(reset.or(after).unwrap_or(now + 3600).max(now + 1))
}

/// The latest release's asset for this machine. Reaches the network: call it
/// on a thread.
///
/// # Errors
///
/// `UnknownItem` for a repository that is not `owner/repo` or does not exist,
/// `RateLimited` with nothing kept, `Offline` with nothing kept,
/// `NoUpdateInfo` when no recent release carries an AppImage for this
/// machine, `ToolFailed` for an answer that does not parse.
pub fn latest(owner_repo: &str, cache_dir: &Path) -> Result<Asset, Refusal> {
    latest_with(&Net, owner_repo, cache_dir, None, crate::appimage::now())
        .map_err(|f| f.refusal.unwrap_or(Refusal::ToolFailed))
}

/// [`latest`] over any network, with an asset-name `pattern` and the clock as
/// parameters.
///
/// # Errors
///
/// As [`latest`], with the host's words as trouble.
pub fn latest_with(
    net: &dyn Fetch,
    owner_repo: &str,
    cache_dir: &Path,
    pattern: Option<&str>,
    now: u64,
) -> Result<Asset, Failure> {
    if !is_repo(owner_repo) {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    let before = kept(cache_dir, owner_repo);
    let answer = |k: &Kept| -> Result<Asset, Failure> {
        k.release
            .as_ref()
            .and_then(|r| choose(r, pattern))
            .ok_or_else(|| Failure::refused(Refusal::NoUpdateInfo))
    };
    if let Some(k) = before
        .as_ref()
        .filter(|k| k.when <= now && now - k.when < FRESH)
    {
        return answer(k);
    }
    // SPENT: ask nobody until the reset; an old answer is better than none.
    if limited_until(cache_dir) > now {
        return before
            .as_ref()
            .map_or_else(|| Err(Failure::refused(Refusal::RateLimited)), answer);
    }
    let api = format!("https://api.github.com/repos/{owner_repo}");
    let etag = before.as_ref().and_then(|k| k.etag.clone());
    let mut headers = vec![
        ("Accept", "application/vnd.github+json"),
        ("X-GitHub-Api-Version", "2022-11-28"),
    ];
    if let Some(tag) = etag.as_deref() {
        headers.push(("If-None-Match", tag));
    }
    let got = match net.get(&format!("{api}/releases/latest"), &headers, MOST) {
        Ok(g) => g,
        Err(f) if f.refusal == Some(Refusal::Offline) => {
            return before.as_ref().map_or(Err(f), answer);
        }
        Err(f) => return Err(f),
    };
    let reply = &got.reply;
    if let Some(until) = spent_until(reply, now) {
        let _ = std::fs::create_dir_all(cache_dir);
        let _ = ronoctua_xdg::write_atomically(cache_dir, LIMIT_FILE, until.to_string().as_bytes());
        return before
            .as_ref()
            .map_or_else(|| Err(Failure::refused(Refusal::RateLimited)), answer);
    }
    if reply.status == 304
        && let Some(mut k) = before
    {
        k.when = now;
        keep(cache_dir, owner_repo, &k);
        return answer(&k);
    }
    if reply.status == 404 {
        return Err(Failure {
            refusal: Some(Refusal::UnknownItem),
            trouble: format!("{owner_repo} has no release"),
        });
    }
    if !reply.went_through() {
        return Err(Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("api.github.com answered {} for {owner_repo}", reply.status),
        });
    }
    let latest = parse_release(&reply.body).map_err(Failure::refused)?;
    let etag = reply.header("etag").map(str::to_owned);
    let release = if choose(&latest, pattern).is_some() {
        Some(latest)
    } else {
        // THE LATEST MAY CARRY NOTHING FOR US (a mobile-only release); the
        // newest recent one that does is the answer.
        let list = net.get(
            &format!("{api}/releases?per_page={RECENT}"),
            &headers[..2],
            MOST,
        )?;
        if let Some(until) = spent_until(&list.reply, now) {
            let _ =
                ronoctua_xdg::write_atomically(cache_dir, LIMIT_FILE, until.to_string().as_bytes());
            return before
                .as_ref()
                .map_or_else(|| Err(Failure::refused(Refusal::RateLimited)), answer);
        }
        if list.reply.went_through() {
            parse_releases(&list.reply.body)
                .map_err(Failure::refused)?
                .into_iter()
                .find(|r| choose(r, pattern).is_some())
        } else {
            None
        }
    };
    let k = Kept {
        etag,
        when: now,
        release,
    };
    keep(cache_dir, owner_repo, &k);
    answer(&k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/appimage")
                .join(name),
        )
        .unwrap()
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-github-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    use crate::github::canned::Canned;

    const LATEST: &str = "https://api.github.com/repos/audacity/audacity/releases/latest";

    #[test]
    fn the_digest_field_is_sha256_and_may_be_absent() {
        let audacity = parse_release(&fixture("github-audacity-latest.json")).unwrap();
        let a = choose(&audacity, None).unwrap();
        assert_eq!(
            a.name, "audacity-linux-4.0.0-x86_64.AppImage",
            "never the aarch64 one"
        );
        assert_eq!(
            a.sha256().as_deref(),
            Some("772663b0b407be44232193b8402cde4da4665c7f6e81edb5b70e3b14e8b9b5b4")
        );
        assert_eq!(a.size, 73_415_160);
        let upscayl = parse_release(&fixture("github-upscayl-latest.json")).unwrap();
        let u = choose(&upscayl, None).unwrap();
        assert_eq!(
            u.name, "upscayl-2.15.0-linux.AppImage",
            "no architecture in the name is ours too"
        );
        assert_eq!(u.sha256(), None);
    }

    #[test]
    fn a_latest_release_without_an_appimage_falls_back_to_the_recent_ones() {
        let latest = parse_release(&fixture("github-obsidian-latest.json")).unwrap();
        assert_eq!(
            choose(&latest, None),
            None,
            "the mobile release has an .apk only"
        );
        let recent = parse_releases(&fixture("github-obsidian-releases.json")).unwrap();
        let a = recent.iter().find_map(|r| choose(r, None)).unwrap();
        assert_eq!(a.name, "Obsidian-1.13.7.AppImage");
        assert_eq!(a.tag, "v1.13.7");
        let patterned = recent
            .iter()
            .find_map(|r| choose(r, Some("Obsidian-*-arm64.AppImage")));
        assert_eq!(
            patterned, None,
            "a pattern never lets another machine's file in"
        );
    }

    #[test]
    fn a_glob_is_stars_and_nothing_else() {
        assert!(glob("Obsidian-*.AppImage", "Obsidian-1.13.7.AppImage"));
        assert!(glob("a*b*c", "a-x-b-y-c"));
        assert!(!glob("a*b*c", "a-x-c"));
        assert!(!glob("x.AppImage", "y.AppImage"));
        assert!(glob("*", "anything"));
    }

    #[test]
    fn a_repository_is_two_plain_names() {
        assert!(is_repo("obsidianmd/obsidian-releases"));
        assert!(!is_repo("../etc"));
        assert!(!is_repo("a/b/c"));
        assert!(!is_repo("a"));
        assert!(!is_repo("a/.."));
    }

    #[test]
    fn an_answer_is_kept_and_a_304_costs_nothing() {
        let dir = scratch("keep");
        let body = fixture("github-audacity-latest.json");
        let net = Canned::new(vec![(LATEST, 200, vec![("ETag", "W/\"one\"")], body)]);
        let a = latest_with(&net, "audacity/audacity", &dir, None, 1_000).unwrap();
        assert_eq!(a.tag, "Audacity-4.0.0");
        // FRESH: nobody is asked again.
        latest_with(&net, "audacity/audacity", &dir, None, 1_000 + 60).unwrap();
        assert_eq!(net.asked.borrow().len(), 1);
        // STALE: asked again, conditionally, and a 304 keeps the answer.
        let net = Canned::new(vec![(LATEST, 304, vec![], Vec::new())]);
        let b = latest_with(&net, "audacity/audacity", &dir, None, 1_000 + FRESH + 1).unwrap();
        assert_eq!(b, a);
        let asked = net.asked.borrow();
        assert!(
            asked[0]
                .1
                .iter()
                .any(|(n, v)| n == "If-None-Match" && v == "W/\"one\"")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_spent_allowance_is_a_refusal_and_is_remembered() {
        let dir = scratch("limit");
        let net = Canned::new(vec![(
            LATEST,
            403,
            vec![
                ("X-RateLimit-Remaining", "0"),
                ("X-RateLimit-Reset", "5000"),
            ],
            b"{}".to_vec(),
        )]);
        let f = latest_with(&net, "audacity/audacity", &dir, None, 1_000).unwrap_err();
        assert_eq!(f.refusal, Some(Refusal::RateLimited));
        // Until the reset, nobody is asked at all.
        let net = Canned::new(Vec::new());
        let f = latest_with(&net, "audacity/audacity", &dir, None, 2_000).unwrap_err();
        assert_eq!(f.refusal, Some(Refusal::RateLimited));
        assert!(net.asked.borrow().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_mobile_only_latest_asks_for_the_recent_list_once() {
        let dir = scratch("fallback");
        let net = Canned::new(vec![
            (
                "https://api.github.com/repos/obsidianmd/obsidian-releases/releases/latest",
                200,
                vec![],
                fixture("github-obsidian-latest.json"),
            ),
            (
                "https://api.github.com/repos/obsidianmd/obsidian-releases/releases?per_page=10",
                200,
                vec![],
                fixture("github-obsidian-releases.json"),
            ),
        ]);
        let a = latest_with(&net, "obsidianmd/obsidian-releases", &dir, None, 10).unwrap();
        assert_eq!(a.name, "Obsidian-1.13.7.AppImage");
        assert_eq!(net.asked.borrow().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The live chain, end to end, through the real network: GitHub's API and
    /// a redirected asset download, the AppImageHub feed, F-Droid's signed
    /// entry, and one F-Droid icon checked against the hash the signed chain
    /// vouches for. Ignored: a test reaches no host. Run it with
    /// `RONOCTUA_LIVE=1 cargo test … -- --ignored --nocapture live`.
    #[test]
    #[ignore = "reaches the network"]
    fn the_live_chain_holds() {
        if std::env::var_os("RONOCTUA_LIVE").is_none() {
            return;
        }
        let dir = scratch("live");
        let roots = crate::Roots::under(&dir);
        let a = latest("audacity/audacity", &dir.join("github")).unwrap();
        println!(
            "github: {} {} {} bytes {:?}",
            a.tag, a.name, a.size, a.digest
        );
        assert!(a.name.contains("x86_64"));
        // A small asset of a real release: the address answers 302 to another
        // host, which is what the redirect code exists for.
        let release = parse_release(
            &Net.get(
                "https://api.github.com/repos/FreeCAD/FreeCAD/releases/latest",
                &[("Accept", "application/vnd.github+json")],
                MOST,
            )
            .unwrap()
            .reply
            .body,
        )
        .unwrap();
        let small = release
            .assets
            .iter()
            .find(|x| x.name.ends_with("-SHA256.txt"))
            .unwrap();
        let to = dir.join("small.txt");
        Net.download(&small.url, 1 << 20, &to, &mut |_, _| true)
            .unwrap();
        assert_eq!(
            Some(crate::run::sha256_of(&to).unwrap()),
            small.sha256(),
            "{}",
            small.name
        );
        println!("redirected download: {} verified", small.name);
        let items = crate::appimagehub::refresh_with(&Net, &roots).unwrap();
        println!("appimagehub: {} items", items.len());
        assert!(items.len() > 1000);
        let jar = Net
            .get(crate::fdroid::ENTRY, &[], crate::fdroid::ENTRY_MOST)
            .unwrap();
        let entry = crate::fdroid::verify_jar(&jar.reply.body).unwrap();
        println!(
            "fdroid: index {} bytes, sha256 {}",
            entry.index.size, entry.index.sha256
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn offline_with_an_old_answer_uses_it() {
        let dir = scratch("offline");
        let net = Canned::new(vec![(
            LATEST,
            200,
            vec![],
            fixture("github-audacity-latest.json"),
        )]);
        latest_with(&net, "audacity/audacity", &dir, None, 1).unwrap();
        let none = Canned::new(Vec::new());
        assert!(latest_with(&none, "audacity/audacity", &dir, None, 1 + FRESH * 10).is_ok());
        let f = latest_with(&none, "audacity/audacity", &scratch("empty"), None, 1).unwrap_err();
        assert_eq!(f.refusal, Some(Refusal::Offline));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
