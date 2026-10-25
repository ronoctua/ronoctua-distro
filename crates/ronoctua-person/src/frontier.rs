//! The downloads a build still needs — a part's or the system's — and the
//! ways in for each.
//!
//! After a build fails, its owner (root's realizer of a part,
//! `ronoctua-machine apply` of the system) asks Nix's own plan (`nix build
//! --dry-run`, read by `ronoctua_machine::dry`) what building it would still
//! build, and keeps the FIXED-OUTPUT derivations of that list: the downloads
//! from their origins. Nothing here reads a log — a log says what one tool
//! printed, and the plan says what is missing.
//!
//! A download is pinned by its hash, so it can come in any way that gives
//! the same bytes ([`Means`]): the person's binary cache or the public one
//! (Nix's own substitution, tried first by every build), a mirror the
//! person named (a flat file only, asked by its hash: `ronoctua_machine::
//! mirror`), a file the person hands in (anything
//! with an address, which is a file or an archive), or a stick from another
//! machine (any download: its path is its content).
//!
//! It lives here, beside the machine's record, because both records carry
//! it: a part's (`ronoctua_parts::record`) and the machine's last apply
//! ([`crate::state::LastApply`]).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// One download a build still needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Download {
    /// Its derivation.
    pub drv: String,
    /// Its output: the store path the bytes become, which only the right
    /// bytes reach.
    pub out: String,
    /// The derivation's name, which a file handed in is added under.
    pub name: String,
    /// The hash the flake pins, in SRI form (`sha256-…`).
    pub hash: String,
    /// What the hash is of.
    pub mode: Mode,
    /// Where it is fetched from, as the derivation says; empty for a
    /// download no single address gives (an npm cache, a vendored crate set).
    pub urls: Vec<String>,
}

/// What a download's hash is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// The file itself (`fetchurl`).
    Flat,
    /// A tree (`fetchzip`, an npm cache): the archive unpacked, or a folder
    /// built from many.
    Recursive,
}

/// A way a download can come in besides its origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Means {
    /// A file the person downloaded elsewhere, checked by the hash.
    HandIn,
    /// A mirror the person named: flat files only, as `fetchurl` asks them.
    Mirror,
    /// A stick from another machine that has it.
    Import,
}

impl Means {
    /// Every one.
    pub const ALL: [Self; 3] = [Self::HandIn, Self::Mirror, Self::Import];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::HandIn => "hand-in",
            Self::Mirror => "mirror",
            Self::Import => "import",
        }
    }
}

impl Download {
    /// The ways this one can come in: a stick always; a file and a mirror
    /// only where an address says what the file is — an npm cache is
    /// assembled from hundreds of addresses and is no file anybody holds.
    #[must_use]
    pub fn means(&self) -> Vec<Means> {
        let mut out = Vec::new();
        if !self.urls.is_empty() {
            out.push(Means::HandIn);
            if self.mode == Mode::Flat {
                out.push(Means::Mirror);
            }
        }
        out.push(Means::Import);
        out
    }

    /// Where the person's `mirrors` may hold it, in the order to ask: for
    /// each mirror, by its hash (`<mirror>/<algo>/<hex>`, the layout of
    /// `tarballs.nixos.org`), then by its name (`<mirror>/<name>`). Only a
    /// flat file: a mirror holds files, and a tree's hash names no file.
    #[must_use]
    pub fn mirror_urls(&self, mirrors: &[String]) -> Vec<String> {
        if self.mode != Mode::Flat {
            return Vec::new();
        }
        let by_hash = self
            .hash
            .split_once('-')
            .and_then(|(algo, b64)| Some((algo, hex(&from_base64(b64)?))));
        let mut out = Vec::new();
        for mirror in mirrors {
            let mirror = mirror.trim_end_matches('/');
            if let Some((algo, hex)) = &by_hash {
                out.push(format!("{mirror}/{algo}/{hex}"));
            }
            out.push(format!("{mirror}/{}", self.name));
        }
        out
    }

    /// The hosts its addresses name, for the page: `github.com`,
    /// `registry.npmjs.org`.
    #[must_use]
    pub fn hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> = self
            .urls
            .iter()
            .filter_map(|u| u.split_once("://").map(|(_, rest)| rest))
            .filter_map(|rest| rest.split('/').next())
            .filter(|h| !h.is_empty())
            .map(str::to_owned)
            .collect();
        hosts.sort();
        hosts.dedup();
        hosts
    }
}

/// The downloads among `will` (the derivations a dry run says it would
/// build), read from `shown` — `nix derivation show` of them, in either of
/// the two shapes Nix writes (keyed by store path, or by name under
/// `derivations`) — and `outputs`, each fixed-output derivation's output
/// path as `nix-store --query --outputs` says it.
#[must_use]
pub fn downloads(
    shown: &serde_json::Value,
    will: &BTreeSet<String>,
    outputs: &std::collections::BTreeMap<String, String>,
) -> Vec<Download> {
    let table = shown.get("derivations").unwrap_or(shown);
    let Some(table) = table.as_object() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (key, drv) in table {
        let path = store_path(key);
        if !will.contains(&path) {
            continue;
        }
        let Some(out) = drv.pointer("/outputs/out") else {
            continue;
        };
        let Some((hash, mode)) = fixed_hash(out) else {
            continue;
        };
        let Some(out_path) = outputs.get(&path) else {
            continue;
        };
        let env = |name: &str| {
            drv.pointer(&format!("/env/{name}"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let urls = env("urls")
            .or_else(|| env("url"))
            .map(|u| u.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default();
        found.push(Download {
            drv: path,
            out: out_path.clone(),
            name: drv
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map_or_else(|| env("name").unwrap_or_default(), str::to_owned),
            hash,
            mode,
            urls,
        });
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// The derivations of `shown` (as [`downloads`] reads it) whose output is
/// fixed — the downloads among them —, as store paths.
#[must_use]
pub fn fixed(shown: &serde_json::Value) -> Vec<String> {
    let table = shown.get("derivations").unwrap_or(shown);
    let Some(table) = table.as_object() else {
        return Vec::new();
    };
    table
        .iter()
        .filter(|(_, drv)| drv.pointer("/outputs/out").and_then(fixed_hash).is_some())
        .map(|(key, _)| store_path(key))
        .collect()
}

/// `key` of `nix derivation show` as a store path: the newer shape names a
/// derivation by its base name.
fn store_path(key: &str) -> String {
    if key.starts_with("/nix/store/") {
        key.to_owned()
    } else {
        format!("/nix/store/{key}")
    }
}

/// The pinned hash of a fixed output, in SRI form, and what it is of.
fn fixed_hash(out: &serde_json::Value) -> Option<(String, Mode)> {
    let text = |k: &str| out.get(k).and_then(serde_json::Value::as_str);
    // NEWER NIX: `{ "hash": "sha256-…", "method": "flat" | "nar" }`.
    if let (Some(hash), Some(method)) = (text("hash"), text("method")) {
        let mode = if method == "flat" {
            Mode::Flat
        } else {
            Mode::Recursive
        };
        return hash.contains('-').then(|| (hash.to_owned(), mode));
    }
    // OLDER NIX: `{ "hash": "<hex>", "hashAlgo": "r:sha256" | "sha256" }`.
    let (hex, algo) = (text("hash")?, text("hashAlgo")?);
    let (mode, algo) = match algo.strip_prefix("r:") {
        Some(rest) => (Mode::Recursive, rest),
        None => (Mode::Flat, algo),
    };
    Some((format!("{algo}-{}", base64(&from_hex(hex)?)), mode))
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// Standard base64, padded — what an SRI hash carries. `None` for anything
/// else.
fn from_base64(text: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if text.is_empty() || !text.len().is_multiple_of(4) {
        return None;
    }
    let body = text.trim_end_matches('=');
    if text.len() - body.len() > 2 {
        return None;
    }
    let mut bits: u32 = 0;
    let mut held = 0;
    let mut out = Vec::new();
    for c in body.bytes() {
        let v = u32::try_from(TABLE.iter().position(|t| *t == c)?).ok()?;
        bits = (bits << 6) | v;
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push(u8::try_from((bits >> held) & 0xff).ok()?);
        }
    }
    Some(out)
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = match chunk {
            [a, b, c] => (u32::from(*a) << 16) | (u32::from(*b) << 8) | u32::from(*c),
            [a, b] => (u32::from(*a) << 16) | (u32::from(*b) << 8),
            [a] => u32::from(*a) << 16,
            _ => 0,
        };
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(char::from(TABLE[((n >> (18 - 6 * k)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// FAILS IF: a mirror is asked at another place than the hash's or the
    /// name's, the hash is not the SRI's in hex, or a tree is asked of a
    /// mirror at all.
    #[test]
    fn a_mirror_is_asked_by_the_hash_then_by_the_name() {
        let bytes: Vec<u8> = (0u8..32).collect();
        let file = Download {
            drv: String::new(),
            out: String::new(),
            name: "x-1.tar.gz".to_owned(),
            hash: format!("sha256-{}", base64(&bytes)),
            mode: Mode::Flat,
            urls: vec!["https://example.org/x-1.tar.gz".to_owned()],
        };
        assert_eq!(from_base64(&base64(&bytes)), Some(bytes.clone()));
        assert_eq!(from_base64("abc"), None);
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            file.mirror_urls(&["https://m.org/".to_owned(), "http://lan:8080".to_owned()]),
            [
                format!("https://m.org/sha256/{hex}"),
                "https://m.org/x-1.tar.gz".to_owned(),
                format!("http://lan:8080/sha256/{hex}"),
                "http://lan:8080/x-1.tar.gz".to_owned(),
            ]
        );
        let tree = Download {
            mode: Mode::Recursive,
            ..file
        };
        assert!(tree.mirror_urls(&["https://m.org".to_owned()]).is_empty());
    }

    const FOD: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-x-1.AppImage.drv";
    const NPM: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyy-pot-npm-deps.drv";
    const PART: &str = "/nix/store/0123456789abcdfghijklmnpqrsvwxyx-x-1.drv";
    const OUT: &str = "/nix/store/zyxwvsrqpnmlkjihgfdcba9876543210-x-1.AppImage";
    const NPM_OUT: &str = "/nix/store/zyxwvsrqpnmlkjihgfdcba9876543211-pot-npm-deps";

    fn will() -> BTreeSet<String> {
        [FOD, NPM, PART].iter().map(|s| (*s).to_owned()).collect()
    }

    fn outputs() -> BTreeMap<String, String> {
        [(FOD, OUT), (NPM, NPM_OUT)]
            .iter()
            .map(|(d, o)| ((*d).to_owned(), (*o).to_owned()))
            .collect()
    }

    /// FAILS IF: a download the plan would build is missed in either shape
    /// Nix writes, a derivation that is no download is taken for one, or an
    /// npm cache — no file anybody holds — is offered to be handed in.
    #[test]
    fn the_downloads_are_the_fixed_outputs_of_the_plan() {
        let shown = serde_json::json!({
            "version": 4,
            "derivations": {
                FOD.trim_start_matches("/nix/store/"): {
                    "name": "x-1.AppImage",
                    "outputs": { "out": { "hash": "sha256-AAAA", "method": "flat" } },
                    "env": { "urls": "https://github.com/o/x/releases/download/v1/x.AppImage" }
                },
                NPM.trim_start_matches("/nix/store/"): {
                    "name": "pot-npm-deps",
                    "outputs": { "out": { "hash": "sha256-BBBB", "method": "nar" } },
                    "env": {}
                },
                PART.trim_start_matches("/nix/store/"): {
                    "name": "x-1",
                    "outputs": { "out": { "path": "zyx-x-1" } },
                    "env": {}
                }
            }
        });
        let found = downloads(&shown, &will(), &outputs());
        assert_eq!(found.len(), 2);
        let app = found
            .iter()
            .find(|d| d.name == "x-1.AppImage")
            .expect("the AppImage");
        assert_eq!((app.out.as_str(), app.mode), (OUT, Mode::Flat));
        assert_eq!(app.means(), [Means::HandIn, Means::Mirror, Means::Import]);
        assert_eq!(app.hosts(), ["github.com"]);
        let npm = found
            .iter()
            .find(|d| d.name == "pot-npm-deps")
            .expect("the npm cache");
        assert_eq!(npm.means(), [Means::Import]);
        let mut fixed = fixed(&shown);
        fixed.sort();
        assert_eq!(
            fixed,
            [NPM, FOD],
            "only the downloads are asked their output"
        );
    }

    /// FAILS IF: the older shape — a hex hash and `r:` for a tree — is read
    /// as anything but the same SRI hash and mode.
    #[test]
    fn the_older_shape_reads_the_same() {
        let shown = serde_json::json!({
            FOD: {
                "name": "x-1.AppImage",
                "outputs": { "out": {
                    "path": OUT,
                    "hash": "00010203",
                    "hashAlgo": "r:sha256"
                } },
                "env": { "url": "https://example.org/x.tar.gz" }
            }
        });
        let found = downloads(&shown, &will(), &outputs());
        assert_eq!(found[0].hash, "sha256-AAECAw==");
        assert_eq!(found[0].mode, Mode::Recursive);
        assert_eq!(found[0].means(), [Means::HandIn, Means::Import]);
    }
}
