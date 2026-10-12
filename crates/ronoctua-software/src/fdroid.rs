//! F-Droid's catalogue, verified from its signed entry point
//! (`entry.jar` → pinned signer → `entry.json` → `index-v2.json` sha256 → the
//! APK's sha256), compacted into our cache.
//!
//! # The chain, as measured on the live repository (2026-09-26)
//!
//! `entry.jar` is 2,582 bytes: `entry.json`, `META-INF/MANIFEST.MF`,
//! `META-INF/CIARANG.SF` and `META-INF/CIARANG.RSA`. The `.RSA` is a PKCS#7
//! SignedData whose one certificate hashes (SHA-256 of its DER) to
//! [`SIGNER`]; its one SignerInfo digests with SHA-256, signs with plain
//! `rsaEncryption` (a 2048-bit key), and carries NO signed attributes — so
//! the signature is over the `.SF` bytes themselves. The `.SF` holds
//! `SHA-256-Digest-Manifest` (base64 of the manifest's SHA-256), and the
//! manifest's `entry.json` section holds that file's. Each link is checked
//! here in Rust ([`verify_parts`]): a DER walker of a hundred lines and
//! `aws-lc-rs` for the RSA, and no new crate. Signed attributes are understood
//! too (the signature then covers their DER `SET`, and their
//! `messageDigest` must be the `.SF`'s hash), because `apksigner` and
//! `jarsigner` write them and a repository can change tools.
//!
//! `entry.json` then names `index-v2.json` with its SHA-256 and size (61 MB,
//! 4,458 packages). It is downloaded, checked, read ONCE as a stream into
//! one small record per package ([`compact_index`]) and deleted: the
//! compacted catalogue is what the search reads.
//!
//! # Which version is offered
//!
//! The newest STABLE version (no release channel) whose native code runs here
//! (none, or `x86_64`/`x86`) and whose `minSdkVersion` the Waydroid image
//! meets ([`ANDROID_SDK`]); failing that, the newest stable one that runs here;
//! failing that, the newest stable one, which is then ARM-only and refused
//! without consent. F-Droid's own client makes the same choice (its 2.0 betas
//! are newer than 1.23.2 and are not suggested).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::found::{Badge, Found, Picture};
use crate::{AppKey, Refusal, Roots};

/// The repository.
pub const REPO: &str = "https://f-droid.org/repo";

/// The host this catalogue asks — its index, its icons and its packages —
/// as the person's consent row lets it (`consent.json`).
pub const HOSTS: &[&str] = &["f-droid.org"];
/// Its signed entry point.
pub const ENTRY: &str = "https://f-droid.org/repo/entry.jar";
/// SHA-256 of the DER of the certificate that signs `entry.jar` — the one
/// thing this chain trusts without asking the network.
pub const SIGNER: &str = "43238d512c1e5eb2d6569f4a3afbf5523418b82e0a3ed1552770abb9a9c9ccab";
/// The most `entry.jar` may be (it is 2.5 KiB).
pub const ENTRY_MOST: u64 = 1 << 20;
/// The most `index-v2.json` may be (it is 61 MB).
pub const INDEX_MOST: u64 = 256 << 20;
/// The API level of Waydroid's LineageOS 20 image (Android 13).
pub const ANDROID_SDK: u32 = 33;
/// The format of the compacted catalogue, in its file's content.
pub const FORMAT: u32 = 1;
/// F-Droid's own default locale — a key of ITS data, not a language of ours.
pub const DEFAULT_LOCALE: &str = "en-US";

// ─── DER ────────────────────────────────────────────────────────────────────

/// One DER element: its tag, its content, and all of it.
#[derive(Debug, Clone, Copy)]
struct Der<'a> {
    tag: u8,
    content: &'a [u8],
    whole: &'a [u8],
}

/// The first element of `bytes` and what follows it. Definite lengths only:
/// DER has no other, and an indefinite one is BER that a verifier must not
/// guess at.
fn read_der(bytes: &[u8]) -> Option<(Der<'_>, &[u8])> {
    let tag = *bytes.first()?;
    if tag & 0x1f == 0x1f {
        return None;
    }
    let first = *bytes.get(1)?;
    let (length, head) = if first < 0x80 {
        (usize::from(first), 2)
    } else {
        let n = usize::from(first & 0x7f);
        if n == 0 || n > 4 {
            return None;
        }
        let mut length = 0usize;
        for i in 0..n {
            length = (length << 8) | usize::from(*bytes.get(2 + i)?);
        }
        (length, 2 + n)
    };
    let end = head.checked_add(length)?;
    let whole = bytes.get(..end)?;
    Some((
        Der {
            tag,
            content: &whole[head..],
            whole,
        },
        &bytes[end..],
    ))
}

/// Every element inside a constructed one.
fn items(content: &[u8]) -> Option<Vec<Der<'_>>> {
    let mut out = Vec::new();
    let mut rest = content;
    while !rest.is_empty() {
        let (d, r) = read_der(rest)?;
        out.push(d);
        rest = r;
    }
    Some(out)
}

/// The contents of the object identifiers this reads.
const OID_SIGNED_DATA: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02];
const OID_RSA: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01];
const OID_SHA256_RSA: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x0B];
const OID_SHA256: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];
const OID_MESSAGE_DIGEST: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x04];

/// The OID of an `AlgorithmIdentifier`.
fn algorithm(d: Der<'_>) -> Option<&[u8]> {
    let inner = items(d.content)?;
    let oid = inner.first().filter(|o| o.tag == 0x06)?;
    Some(oid.content)
}

/// What a PKCS#7 SignedData says, reduced to what verifying it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signed {
    /// Every certificate it carries, as DER.
    pub certificates: Vec<Vec<u8>>,
    /// The signed attributes re-tagged as the `SET` the signature covers.
    pub signed_attributes: Option<Vec<u8>>,
    /// The `messageDigest` among them.
    pub message_digest: Option<Vec<u8>>,
    /// The signature.
    pub signature: Vec<u8>,
}

/// Read a `.RSA` block: one SignedData, SHA-256 and RSA, first signer.
///
/// # Errors
///
/// `Integrity` for anything else — a verifier does not guess.
pub fn signed_data(block: &[u8]) -> Result<Signed, Refusal> {
    signed_data_of(block).ok_or(Refusal::Integrity)
}

fn signed_data_of(block: &[u8]) -> Option<Signed> {
    let (info, _) = read_der(block)?;
    let top = items(info.content).filter(|_| info.tag == 0x30)?;
    if top.first()?.tag != 0x06 || top[0].content != OID_SIGNED_DATA || top.get(1)?.tag != 0xA0 {
        return None;
    }
    let (data, _) = read_der(top[1].content)?;
    let parts = items(data.content).filter(|_| data.tag == 0x30)?;
    let certificates = parts
        .iter()
        .find(|p| p.tag == 0xA0)
        .and_then(|c| items(c.content))
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.tag == 0x30)
        .map(|c| c.whole.to_vec())
        .collect();
    let signers = parts.iter().rev().find(|p| p.tag == 0x31)?;
    let first = items(signers.content)?
        .into_iter()
        .next()
        .filter(|s| s.tag == 0x30)?;
    let fields = items(first.content)?;
    // version, sid, digestAlgorithm, [0] signedAttrs?, signatureAlgorithm, signature
    let digest = fields.get(2).copied()?;
    if algorithm(digest)? != OID_SHA256 {
        return None;
    }
    let mut at = 3;
    let mut signed_attributes = None;
    let mut message_digest = None;
    if fields.get(at)?.tag == 0xA0 {
        let attrs = fields[at];
        let mut set = attrs.whole.to_vec();
        set[0] = 0x31;
        for attr in items(attrs.content)? {
            let pair = items(attr.content)?;
            if pair.first()?.content == OID_MESSAGE_DIGEST {
                let values = items(pair.get(1)?.content)?;
                message_digest = values
                    .first()
                    .filter(|v| v.tag == 0x04)
                    .map(|v| v.content.to_vec());
            }
        }
        signed_attributes = Some(set);
        at += 1;
    }
    let scheme = algorithm(*fields.get(at)?)?;
    if scheme != OID_RSA && scheme != OID_SHA256_RSA {
        return None;
    }
    let signature = fields
        .get(at + 1)
        .filter(|s| s.tag == 0x04)?
        .content
        .to_vec();
    Some(Signed {
        certificates,
        signed_attributes,
        message_digest,
        signature,
    })
}

/// The RSA public key (`RSAPublicKey` DER, what `aws-lc-rs` takes) of a
/// certificate.
fn rsa_key_of(certificate: &[u8]) -> Option<Vec<u8>> {
    let (cert, _) = read_der(certificate)?;
    let (tbs, _) = read_der(cert.content)?;
    let fields = items(tbs.content)?;
    // [0] version is optional; then serial, signature, issuer, validity,
    // subject, subjectPublicKeyInfo.
    let skip = usize::from(fields.first()?.tag == 0xA0);
    let spki = fields.get(skip + 5).filter(|s| s.tag == 0x30)?;
    let parts = items(spki.content)?;
    if algorithm(*parts.first()?)? != OID_RSA {
        return None;
    }
    let bits = parts.get(1).filter(|b| b.tag == 0x03)?;
    (bits.content.first() == Some(&0)).then(|| bits.content[1..].to_vec())
}

/// Check that `sf` was signed by the certificate pinned as `pin` (the hex
/// SHA-256 of its DER).
///
/// # Errors
///
/// `Integrity`: no certificate with that hash, a signature that does not
/// verify, or signed attributes whose `messageDigest` is not the `.SF`'s.
pub fn verify_signature(sf: &[u8], block: &[u8], pin: &str) -> Result<(), Refusal> {
    let signed = signed_data(block)?;
    let certificate = signed
        .certificates
        .iter()
        .find(|c| crate::run::sha256(c).eq_ignore_ascii_case(pin))
        .ok_or(Refusal::Integrity)?;
    let key = rsa_key_of(certificate).ok_or(Refusal::Integrity)?;
    let message: &[u8] = match &signed.signed_attributes {
        Some(set) => {
            let want = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, sf);
            if signed.message_digest.as_deref() != Some(want.as_ref()) {
                return Err(Refusal::Integrity);
            }
            set
        }
        None => sf,
    };
    aws_lc_rs::signature::UnparsedPublicKey::new(
        &aws_lc_rs::signature::RSA_PKCS1_2048_8192_SHA256,
        key,
    )
    .verify(message, &signed.signature)
    .map_err(|_| Refusal::Integrity)
}

// ─── The JAR's manifests ────────────────────────────────────────────────────

/// A manifest's sections, each a list of `(name, value)`; a line that starts
/// with a space continues the one before it (the 72-byte rule).
#[must_use]
pub fn sections(text: &str) -> Vec<Vec<(String, String)>> {
    let mut out: Vec<Vec<(String, String)>> = vec![Vec::new()];
    for line in text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)) {
        if line.is_empty() {
            if out.last().is_some_and(|s| !s.is_empty()) {
                out.push(Vec::new());
            }
        } else if let Some(more) = line.strip_prefix(' ') {
            if let Some(last) = out.last_mut().and_then(|s| s.last_mut()) {
                last.1.push_str(more);
            }
        } else if let Some((name, value)) = line.split_once(": ")
            && let Some(section) = out.last_mut()
        {
            section.push((name.to_owned(), value.to_owned()));
        }
    }
    out.retain(|s| !s.is_empty());
    out
}

fn value<'a>(section: &'a [(String, String)], name: &str) -> Option<&'a str> {
    section
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

/// Standard base64, padded — the form a manifest writes its digests in.
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn sha256_base64(bytes: &[u8]) -> String {
    base64(aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, bytes).as_ref())
}

/// Check the four parts of `entry.jar` against each other and the pin: the
/// `.SF` signed by the pinned certificate, the manifest digested in the
/// `.SF`, `entry.json` digested in the manifest.
///
/// # Errors
///
/// `Integrity` at the first link that does not hold.
pub fn verify_parts(
    manifest: &[u8],
    sf: &[u8],
    block: &[u8],
    entry: &[u8],
    pin: &str,
) -> Result<(), Refusal> {
    verify_signature(sf, block, pin)?;
    let sf_text = std::str::from_utf8(sf).map_err(|_| Refusal::Integrity)?;
    let main = sections(sf_text).into_iter().next().unwrap_or_default();
    if value(&main, "SHA-256-Digest-Manifest") != Some(sha256_base64(manifest).as_str()) {
        return Err(Refusal::Integrity);
    }
    let manifest_text = std::str::from_utf8(manifest).map_err(|_| Refusal::Integrity)?;
    let named = sections(manifest_text)
        .into_iter()
        .find(|s| value(s, "Name") == Some("entry.json"))
        .ok_or(Refusal::Integrity)?;
    if value(&named, "SHA-256-Digest") != Some(sha256_base64(entry).as_str()) {
        return Err(Refusal::Integrity);
    }
    Ok(())
}

/// What `entry.json` says.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Entry {
    /// When the repository was published (milliseconds since the epoch).
    pub timestamp: u64,
    /// The index it points at.
    pub index: IndexRef,
}

/// The index `entry.json` points at.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IndexRef {
    /// Its path in the repository (`/index-v2.json`).
    pub name: String,
    /// Its SHA-256, hex.
    pub sha256: String,
    /// Its size.
    pub size: u64,
    /// How many packages it lists.
    #[serde(default, rename = "numPackages")]
    pub packages: u64,
}

/// A path in the repository that may be fetched: `/name.ext`, plain.
fn repo_path(name: &str) -> bool {
    name.strip_prefix('/').is_some_and(|rest| {
        !rest.is_empty()
            && rest.len() <= 300
            && rest
                .split('/')
                .all(|c| !c.is_empty() && c != "." && c != "..")
            && rest.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/' | b'=' | b'+')
            })
    })
}

fn is_sha256(hex: &str) -> bool {
    hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Verify `entry.jar` against [`SIGNER`] and read the entry it vouches for.
///
/// # Errors
///
/// `Integrity` for a JAR that does not verify or an entry that does not name
/// a plain index; `TooLarge` above the ceilings.
pub fn verify_jar(jar: &[u8]) -> Result<Entry, Refusal> {
    verify_jar_with(jar, SIGNER)
}

/// [`verify_jar`] against any pin (a test's).
///
/// # Errors
///
/// As [`verify_jar`].
pub fn verify_jar_with(jar: &[u8], pin: &str) -> Result<Entry, Refusal> {
    let members = ronoctua_zip::members(jar).map_err(|_| Refusal::Integrity)?;
    let read = |name: &str| -> Result<Vec<u8>, Refusal> {
        let m = ronoctua_zip::part(&members, name).ok_or(Refusal::Integrity)?;
        ronoctua_zip::member_up_to(jar, m, ENTRY_MOST).map_err(|e| match e {
            ronoctua_zip::Trouble::TooBig(_) => Refusal::TooLarge,
            ronoctua_zip::Trouble::Malformed => Refusal::Integrity,
        })
    };
    // ONE SIGNATURE FILE, and its block by the same stem: two would be two
    // answers to "who signed this".
    let signatures: Vec<&str> = members
        .iter()
        .map(ronoctua_zip::Member::name)
        .filter(|n| n.starts_with("META-INF/") && n.ends_with(".SF"))
        .collect();
    let [sf_name] = signatures.as_slice() else {
        return Err(Refusal::Integrity);
    };
    let block_name = format!("{}.RSA", sf_name.trim_end_matches(".SF"));
    let (manifest, sf, block, entry) = (
        read("META-INF/MANIFEST.MF")?,
        read(sf_name)?,
        read(&block_name)?,
        read("entry.json")?,
    );
    verify_parts(&manifest, &sf, &block, &entry, pin)?;
    let entry: Entry = serde_json::from_slice(&entry).map_err(|_| Refusal::Integrity)?;
    if !repo_path(&entry.index.name) || !is_sha256(&entry.index.sha256) {
        return Err(Refusal::Integrity);
    }
    if entry.index.size > INDEX_MOST {
        return Err(Refusal::TooLarge);
    }
    Ok(entry)
}

// ─── The compacted catalogue ────────────────────────────────────────────────

/// A file of the repository with its hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RepoFile {
    /// Its path (`/org.fdroid.fdroid/en-US/icon_….png`).
    pub name: String,
    /// Its SHA-256, hex.
    pub sha256: String,
}

/// One package, at the version offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct App {
    /// The package name.
    pub package: String,
    /// Its name, per locale (F-Droid's keys: `en-US`, `pt-BR`, …).
    pub name: BTreeMap<String, String>,
    /// Its summary, per locale.
    #[serde(default)]
    pub summary: BTreeMap<String, String>,
    /// Its icon.
    #[serde(default)]
    pub icon: Option<RepoFile>,
    /// The version offered.
    pub version_code: u64,
    /// Its name.
    #[serde(default)]
    pub version_name: String,
    /// The APK's path in the repository.
    pub apk: String,
    /// The APK's SHA-256.
    pub sha256: String,
    /// The APK's size.
    #[serde(default)]
    pub size: u64,
    /// The native code it carries (empty: runs anywhere).
    #[serde(default)]
    pub nativecode: Vec<String>,
    /// The lowest Android API level it runs on.
    #[serde(default)]
    pub min_sdk: Option<u32>,
    /// F-Droid's anti-features of that version, by their words.
    #[serde(default)]
    pub anti_features: Vec<String>,
    /// Its licence.
    #[serde(default)]
    pub license: Option<String>,
    /// Its site.
    #[serde(default)]
    pub web_site: Option<String>,
}

/// Native code this machine runs: none at all, or x86.
fn runs_here(nativecode: &[String]) -> bool {
    nativecode.is_empty() || nativecode.iter().any(|n| n == "x86_64" || n == "x86")
}

impl App {
    /// Built only for other machines: it will not start without a
    /// translation layer.
    #[must_use]
    pub fn arm_only(&self) -> bool {
        !runs_here(&self.nativecode)
    }

    /// Where the APK is.
    #[must_use]
    pub fn apk_url(&self) -> String {
        format!("{REPO}{}", self.apk)
    }

    /// Where the icon is.
    #[must_use]
    pub fn icon_url(&self) -> Option<String> {
        self.icon.as_ref().map(|i| format!("{REPO}{}", i.name))
    }

    /// Its name in `language` (F-Droid's key form, `pt-BR`), then the
    /// language alone, then F-Droid's default, then any.
    #[must_use]
    pub fn name_in(&self, language: &str) -> String {
        localized(&self.name, language)
            .unwrap_or(&self.package)
            .to_owned()
    }

    /// The row a search shows.
    #[must_use]
    pub fn found(&self, language: &str, by_description: bool) -> Found {
        let mut badges = Vec::new();
        if self.arm_only() {
            badges.push(Badge::ArmOnly);
        }
        if !self.anti_features.is_empty() {
            badges.push(Badge::AntiFeatures);
        }
        Found {
            key: AppKey::Android {
                package: self.package.clone(),
            },
            name: self.name_in(language),
            summary: localized(&self.summary, language)
                .unwrap_or_default()
                .to_owned(),
            version: self.version_name.clone(),
            badges,
            download_bytes: (self.size > 0).then_some(self.size),
            icon: self.icon_url().map(|url| Picture::Url { url }),
            homepage: self.web_site.clone(),
            license: self.license.clone(),
            publisher: None,
            program: None,
            by_description,
        }
    }
}

/// A localized value: the language, the language without its region, F-Droid's
/// default, the first there is.
#[must_use]
pub fn localized<'a>(map: &'a BTreeMap<String, String>, language: &str) -> Option<&'a str> {
    let short = language.split(['-', '_']).next().unwrap_or(language);
    let wanted = language.replace('_', "-");
    map.get(&wanted)
        .or_else(|| map.get(short))
        .or_else(|| {
            map.iter()
                .find(|(k, _)| k.split('-').next() == Some(short) && !short.is_empty())
                .map(|(_, v)| v)
        })
        .or_else(|| map.get(DEFAULT_LOCALE))
        .or_else(|| map.values().next())
        .map(String::as_str)
}

/// The compacted catalogue, as kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Catalogue {
    /// [`FORMAT`].
    pub format: u32,
    /// `entry.json`'s timestamp (milliseconds): what "catalogue of <date>" says.
    pub timestamp: u64,
    /// The SHA-256 of the index it was made from.
    pub index_sha256: String,
    /// Every package.
    pub apps: Vec<App>,
}

#[derive(Deserialize, Default)]
struct RawFile {
    #[serde(default)]
    name: String,
    #[serde(default)]
    sha256: String,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawMetadata {
    #[serde(default)]
    name: BTreeMap<String, String>,
    #[serde(default)]
    summary: BTreeMap<String, String>,
    #[serde(default)]
    icon: BTreeMap<String, RawFile>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    web_site: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawSdk {
    #[serde(default)]
    min_sdk_version: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawManifest {
    #[serde(default)]
    version_name: String,
    version_code: u64,
    #[serde(default)]
    uses_sdk: Option<RawSdk>,
    #[serde(default)]
    nativecode: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawVersion {
    file: RawFile,
    manifest: RawManifest,
    #[serde(default)]
    release_channels: Vec<String>,
    #[serde(default)]
    anti_features: BTreeMap<String, IgnoredAny>,
}

#[derive(Deserialize, Default)]
struct RawPackage {
    #[serde(default)]
    metadata: RawMetadata,
    #[serde(default)]
    versions: BTreeMap<String, RawVersion>,
}

/// A Java package name.
fn is_package(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}

/// One package reduced to the version offered, or `None` when nothing of it
/// can be offered safely (no version, a name or a path that is not plain).
fn compact(package: String, raw: RawPackage) -> Option<App> {
    if !is_package(&package) {
        return None;
    }
    let all: Vec<&RawVersion> = raw
        .versions
        .values()
        .filter(|v| repo_path(&v.file.name) && is_sha256(&v.file.sha256))
        .collect();
    let stable: Vec<&RawVersion> = all
        .iter()
        .copied()
        .filter(|v| v.release_channels.is_empty())
        .collect();
    let pool = if stable.is_empty() { &all } else { &stable };
    let newest = |keep: &dyn Fn(&RawVersion) -> bool| {
        pool.iter()
            .copied()
            .filter(|v| keep(v))
            .max_by_key(|v| v.manifest.version_code)
    };
    let min = |v: &RawVersion| v.manifest.uses_sdk.as_ref().and_then(|s| s.min_sdk_version);
    let chosen =
        newest(&|v| runs_here(&v.manifest.nativecode) && min(v).is_none_or(|m| m <= ANDROID_SDK))
            .or_else(|| newest(&|v| runs_here(&v.manifest.nativecode)))
            .or_else(|| newest(&|_| true))?;
    let meta = raw.metadata;
    let icon = meta
        .icon
        .get(DEFAULT_LOCALE)
        .or_else(|| meta.icon.values().next())
        .filter(|f| repo_path(&f.name) && is_sha256(&f.sha256))
        .map(|f| RepoFile {
            name: f.name.clone(),
            sha256: f.sha256.to_ascii_lowercase(),
        });
    Some(App {
        name: meta.name,
        summary: meta.summary,
        icon,
        version_code: chosen.manifest.version_code,
        version_name: chosen.manifest.version_name.clone(),
        apk: chosen.file.name.clone(),
        sha256: chosen.file.sha256.to_ascii_lowercase(),
        size: chosen.file.size,
        nativecode: chosen.manifest.nativecode.clone(),
        min_sdk: min(chosen),
        anti_features: chosen.anti_features.keys().cloned().collect(),
        license: meta.license,
        web_site: meta
            .web_site
            .filter(|w| w.starts_with("https://") || w.starts_with("http://")),
        package,
    })
}

/// The `packages` object, read one package at a time: each is reduced to its
/// [`App`] before the next is parsed, so memory holds one package's versions
/// and the small records — never the index.
struct Packages(Vec<App>);

impl<'de> Deserialize<'de> for Packages {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Each;
        impl<'de> Visitor<'de> for Each {
            type Value = Packages;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("F-Droid's packages")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Packages, A::Error> {
                let mut out = Vec::new();
                while let Some(package) = map.next_key::<String>()? {
                    let raw: RawPackage = map.next_value()?;
                    out.extend(compact(package, raw));
                }
                Ok(Packages(out))
            }
        }
        d.deserialize_map(Each)
    }
}

#[derive(Deserialize)]
struct RawIndex {
    packages: Packages,
}

/// Read an `index-v2.json` as a stream into one record per package, sorted
/// by package name.
///
/// # Errors
///
/// `ToolFailed` for something that is not an index.
pub fn compact_index(reader: impl std::io::Read) -> Result<Vec<App>, Refusal> {
    let raw: RawIndex = serde_json::from_reader(reader).map_err(|_| Refusal::ToolFailed)?;
    let mut apps = raw.packages.0;
    apps.sort_by(|a, b| a.package.cmp(&b.package));
    Ok(apps)
}

/// Our F-Droid directory in the cache.
#[must_use]
pub fn directory(roots: &Roots) -> PathBuf {
    roots.ours_cache().join("fdroid")
}

/// The compacted catalogue's file, when there is one (the newest, should a
/// crash have left two).
#[must_use]
pub fn current_file(roots: &Roots) -> Option<PathBuf> {
    let dir = std::fs::read_dir(directory(roots)).ok()?;
    dir.filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("compact-") && n.ends_with(".json"))
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max()
        .map(|(_, p)| p)
}

/// The compacted catalogue, read once per file and kept in the process: a
/// search per keystroke must not parse megabytes per keystroke.
#[must_use]
pub fn catalogue(roots: &Roots) -> Option<std::sync::Arc<Catalogue>> {
    use std::os::unix::fs::MetadataExt as _;
    // THE FILE'S IDENTITY, not its name: a catalogue is replaced by a rename,
    // which is a new inode even when the name and the second are the same.
    type Stamp = (u64, u64, i64, i64);
    type Held = Option<(PathBuf, Stamp, std::sync::Arc<Catalogue>)>;
    static HELD: std::sync::Mutex<Held> = std::sync::Mutex::new(None);
    let path = current_file(roots)?;
    let meta = std::fs::metadata(&path).ok()?;
    let stamp = (meta.ino(), meta.len(), meta.mtime(), meta.mtime_nsec());
    let mut held = HELD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((p, s, c)) = held.as_ref()
        && *p == path
        && *s == stamp
    {
        return Some(c.clone());
    }
    let read: Catalogue = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
    if read.format != FORMAT {
        return None;
    }
    let c = std::sync::Arc::new(read);
    *held = Some((path, stamp, c.clone()));
    Some(c)
}

/// Keep a catalogue, replacing whatever was kept.
///
/// # Errors
///
/// What the filesystem says.
pub fn keep(roots: &Roots, catalogue: &Catalogue) -> std::io::Result<PathBuf> {
    let dir = directory(roots);
    let name = format!(
        "compact-{}.json",
        catalogue.index_sha256.get(..8).unwrap_or("00000000")
    );
    let bytes = serde_json::to_vec(catalogue).map_err(std::io::Error::other)?;
    ronoctua_xdg::write_atomically(&dir, &name, &bytes)?;
    for e in std::fs::read_dir(&dir)?.flatten() {
        let other = e
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with("compact-") && n != name);
        if other {
            let _ = std::fs::remove_file(e.path());
        }
    }
    Ok(dir.join(name))
}

/// One package of the kept catalogue.
#[must_use]
pub fn app(roots: &Roots, package: &str) -> Option<App> {
    let c = catalogue(roots)?;
    let at = c
        .apps
        .binary_search_by(|a| a.package.as_str().cmp(package))
        .ok()?;
    Some(c.apps[at].clone())
}

/// Search a list of apps: names in any locale (exact, prefix, anywhere),
/// then the package name, then summaries.
#[must_use]
pub fn search_apps(apps: &[App], query: &str, language: &str, limit: usize) -> Vec<Found> {
    let q = ronoctua_apps::fold::fold(query.trim());
    if q.is_empty() {
        return Vec::new();
    }
    // PUNCTUATION IS NOT PART OF A NAME a person types: "fdroid" has to find
    // "F-Droid" first, not tenth behind nine apps with "fdroid" in the package.
    let bare = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    };
    let q_bare = bare(&q);
    let mut hits: Vec<(u8, String, &App)> = apps
        .iter()
        .filter_map(|a| {
            let names: Vec<String> = a
                .name
                .values()
                .map(|n| ronoctua_apps::fold::fold(n))
                .collect();
            let bares: Vec<String> = names.iter().map(|n| bare(n)).collect();
            let tier = if names.contains(&q) || (!q_bare.is_empty() && bares.contains(&q_bare)) {
                0
            } else if names.iter().any(|n| n.starts_with(&q))
                || (!q_bare.is_empty() && bares.iter().any(|n| n.starts_with(&q_bare)))
            {
                1
            } else if names.iter().any(|n| n.contains(&q)) {
                2
            } else if a.package.to_ascii_lowercase().contains(&q) {
                3
            } else if a
                .summary
                .values()
                .any(|s| ronoctua_apps::fold::fold(s).contains(&q))
            {
                4
            } else {
                return None;
            };
            Some((tier, ronoctua_apps::fold::fold(&a.name_in(language)), a))
        })
        .collect();
    hits.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    hits.into_iter()
        .take(limit)
        .map(|(tier, _, a)| a.found(language, tier == 4))
        .collect()
}

/// Search the compacted catalogue, names in F-Droid's default locale.
///
/// # Errors
///
/// `SourceUnprepared` when the catalogue was never fetched.
pub fn search(roots: &Roots, query: &str, limit: usize) -> Result<Vec<Found>, crate::Refusal> {
    search_for(roots, query, DEFAULT_LOCALE, limit)
}

/// [`search`] with the person's language (`pt-BR`), which names and
/// summaries are shown in when F-Droid has them.
///
/// # Errors
///
/// As [`search`].
pub fn search_for(
    roots: &Roots,
    query: &str,
    language: &str,
    limit: usize,
) -> Result<Vec<Found>, crate::Refusal> {
    let c = catalogue(roots).ok_or(Refusal::SourceUnprepared)?;
    Ok(search_apps(&c.apps, query, language, limit))
}

/// Fetch, verify and compact the catalogue again; answers how many packages
/// it holds. Nothing is downloaded when the signed entry names the index
/// already compacted. Reaches the network: call it on a thread.
///
/// # Errors
///
/// `Integrity` at any broken link (and for an entry older than the catalogue
/// already here, which is how a replayed old signature looks), `Offline`,
/// `TooLarge`, `ToolFailed`.
#[cfg(feature = "net")]
pub fn refresh_with(
    net: &dyn crate::github::Fetch,
    roots: &Roots,
    report: &mut dyn crate::progress::Report,
) -> Result<usize, crate::Failure> {
    use crate::Failure;
    use crate::progress::Phase;
    report.step(Phase::Resolving, 0, 0, "entry.jar");
    let got = net.get(ENTRY, &[], ENTRY_MOST)?;
    if !got.reply.went_through() {
        return Err(Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("f-droid.org answered {}", got.reply.status),
        });
    }
    report.step(Phase::Verifying, 0, 0, "entry.jar");
    let entry = verify_jar(&got.reply.body).map_err(Failure::refused)?;
    let index = &entry.index;
    if let Some(here) = catalogue(roots) {
        if here.index_sha256 == index.sha256.to_ascii_lowercase() {
            return Ok(here.apps.len());
        }
        if entry.timestamp < here.timestamp {
            return Err(Failure {
                refusal: Some(Refusal::Integrity),
                trouble: "the signed entry is older than the catalogue already here".to_owned(),
            });
        }
    }
    let dir = directory(roots);
    std::fs::create_dir_all(&dir)
        .map_err(|e| Failure::trouble(format!("could not create {}: {e}", dir.display())))?;
    let raw = dir.join(format!("index-{}.json.part", &index.sha256[..8]));
    // THE RAW INDEX NEVER STAYS: removed on every way out of this function.
    struct Gone(PathBuf);
    impl Drop for Gone {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let gone = Gone(raw.clone());
    let mut last = 0u64;
    net.download(
        &format!("{REPO}{}", index.name),
        index.size,
        &raw,
        &mut |done, total| {
            if done >= last + (1 << 20) || Some(done) == total {
                last = done;
                report.step(
                    Phase::Downloading,
                    done,
                    total.unwrap_or(index.size),
                    "index-v2.json",
                );
            }
            true
        },
    )?;
    report.step(Phase::Verifying, 0, 0, "index-v2.json");
    let size = std::fs::metadata(&raw).map(|m| m.len()).unwrap_or(0);
    let sha = crate::run::sha256_of(&raw).map_err(|e| Failure::trouble(e.to_string()))?;
    if size != index.size || !sha.eq_ignore_ascii_case(&index.sha256) {
        return Err(Failure {
            refusal: Some(Refusal::Integrity),
            trouble: format!(
                "index-v2.json: {size} bytes, sha256 {sha}; the signed entry says {} and {}",
                index.size, index.sha256
            ),
        });
    }
    report.step(Phase::Indexing, 0, 0, "index-v2.json");
    let file = std::fs::File::open(&raw).map_err(|e| Failure::trouble(e.to_string()))?;
    let apps = compact_index(std::io::BufReader::with_capacity(1 << 20, file))
        .map_err(Failure::refused)?;
    drop(gone);
    let count = apps.len();
    let c = Catalogue {
        format: FORMAT,
        timestamp: entry.timestamp,
        index_sha256: index.sha256.to_ascii_lowercase(),
        apps,
    };
    keep(roots, &c).map_err(|e| Failure::trouble(format!("could not keep the catalogue: {e}")))?;
    Ok(count)
}

/// What an APK must contain to be one: the manifest and the code.
///
/// # Errors
///
/// `NotAnApk`.
pub fn check_apk(bytes: &[u8]) -> Result<(), Refusal> {
    let members = ronoctua_zip::members(bytes).map_err(|_| Refusal::NotAnApk)?;
    let has = |name: &str| ronoctua_zip::part(&members, name).is_some();
    if bytes.starts_with(b"PK\x03\x04") && has("AndroidManifest.xml") && has("classes.dex") {
        Ok(())
    } else {
        Err(Refusal::NotAnApk)
    }
}

/// Whether a file on disk is an APK; `ceiling` bounds what is read.
///
/// # Errors
///
/// `NotAnApk`, or `TooLarge` above the ceiling.
pub fn check_apk_file(path: &Path, ceiling: u64) -> Result<(), Refusal> {
    let size = std::fs::metadata(path)
        .map_err(|_| Refusal::NotAnApk)?
        .len();
    if size > ceiling {
        return Err(Refusal::TooLarge);
    }
    check_apk(&std::fs::read(path).map_err(|_| Refusal::NotAnApk)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/android")
                .join(name),
        )
        .unwrap()
    }

    /// The four parts of the real `entry.jar`.
    fn parts() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let jar = fixture("entry.jar");
        let members = ronoctua_zip::members(&jar).unwrap();
        let read = |n: &str| {
            ronoctua_zip::member_bytes(&jar, ronoctua_zip::part(&members, n).unwrap()).unwrap()
        };
        (
            read("META-INF/MANIFEST.MF"),
            read("META-INF/CIARANG.SF"),
            read("META-INF/CIARANG.RSA"),
            read("entry.json"),
        )
    }

    #[test]
    fn the_real_entry_jar_verifies_against_the_pinned_signer() {
        let entry = verify_jar(&fixture("entry.jar")).unwrap();
        assert_eq!(entry.index.name, "/index-v2.json");
        assert_eq!(
            entry.index.sha256,
            "4ef32c0181b22ca2c083ddc913cd1ced690fbc70ed5d11e99d930e0b06064cca"
        );
        assert_eq!(entry.index.size, 61_253_664);
        let (.., block, _) = parts();
        let signed = signed_data(&block).unwrap();
        assert_eq!(signed.certificates.len(), 1);
        assert_eq!(crate::run::sha256(&signed.certificates[0]), SIGNER);
        assert_eq!(
            signed.signed_attributes, None,
            "measured: F-Droid signs the .SF itself"
        );
    }

    #[test]
    fn a_byte_changed_anywhere_breaks_the_chain() {
        let (manifest, sf, block, entry) = parts();
        assert_eq!(verify_parts(&manifest, &sf, &block, &entry, SIGNER), Ok(()));
        let flip = |b: &[u8], at: usize| {
            let mut c = b.to_vec();
            c[at] ^= 1;
            c
        };
        assert_eq!(
            verify_parts(&manifest, &sf, &block, &flip(&entry, 40), SIGNER),
            Err(Refusal::Integrity),
            "entry.json"
        );
        assert_eq!(
            verify_parts(&flip(&manifest, 30), &sf, &block, &entry, SIGNER),
            Err(Refusal::Integrity),
            "manifest"
        );
        assert_eq!(
            verify_parts(&manifest, &flip(&sf, 60), &block, &entry, SIGNER),
            Err(Refusal::Integrity),
            ".SF"
        );
        let last = block.len() - 5;
        assert_eq!(
            verify_parts(&manifest, &sf, &flip(&block, last), &entry, SIGNER),
            Err(Refusal::Integrity),
            "signature"
        );
        // A byte inside the certificate: its hash is no longer the pin.
        assert_eq!(
            verify_parts(&manifest, &sf, &flip(&block, 200), &entry, SIGNER),
            Err(Refusal::Integrity),
            "certificate"
        );
        let other = "0".repeat(64);
        assert_eq!(
            verify_parts(&manifest, &sf, &block, &entry, &other),
            Err(Refusal::Integrity),
            "another signer"
        );
    }

    #[test]
    fn a_jar_with_two_signatures_or_none_is_refused() {
        assert_eq!(verify_jar(b"not a zip"), Err(Refusal::Integrity));
    }

    #[test]
    fn base64_is_the_standard_one() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_manifest_line_continues_on_a_space() {
        let s = sections(
            "Manifest-Version: 1.0\r\n\r\nName: a-very-long\r\n -name.json\r\nSHA-256-Digest: x\r\n\r\n",
        );
        assert_eq!(s.len(), 2);
        assert_eq!(value(&s[1], "Name"), Some("a-very-long-name.json"));
    }

    #[test]
    fn the_index_is_compacted_to_the_version_that_runs_here() {
        let apps = compact_index(std::io::Cursor::new(fixture("index-v2-cut.json"))).unwrap();
        let by = |p: &str| apps.iter().find(|a| a.package == p).unwrap();
        let fdroid = by("org.fdroid.fdroid");
        assert_eq!(
            (fdroid.version_code, fdroid.version_name.as_str()),
            (1_023_052, "1.23.2"),
            "never a Beta"
        );
        assert_eq!(
            fdroid.apk_url(),
            "https://f-droid.org/repo/org.fdroid.fdroid_1023052.apk"
        );
        assert!(
            fdroid
                .icon_url()
                .unwrap()
                .starts_with("https://f-droid.org/repo/org.fdroid.fdroid/en-US/icon_")
        );
        let pipe = by("InfinityLoop1309.NewPipeEnhanced");
        assert_eq!(
            pipe.version_code, 110_803,
            "the x86_64 split, not the newer arm64 one"
        );
        assert!(!pipe.arm_only());
        assert_eq!(pipe.anti_features, ["NonFreeNet"]);
        assert!(by("chat.simplex.app").arm_only());
        let rebooter = by("com.github.girator.rebooter");
        assert!(
            rebooter.min_sdk.unwrap() <= ANDROID_SDK,
            "the newest that the image can run"
        );
        assert!(
            apps.windows(2).all(|w| w[0].package < w[1].package),
            "sorted, for the binary search"
        );
    }

    #[test]
    fn a_name_is_shown_in_the_persons_language_and_found_in_any() {
        let apps = compact_index(std::io::Cursor::new(fixture("index-v2-cut.json"))).unwrap();
        let rows = search_apps(&apps, "f-droid", "de", 5);
        assert_eq!(
            rows[0].key,
            AppKey::Android {
                package: "org.fdroid.fdroid".into()
            }
        );
        let rows = search_apps(&apps, "fdroid", "en-US", 5);
        assert_eq!(
            rows[0].key,
            AppKey::Android {
                package: "org.fdroid.fdroid".into()
            },
            "typed without the hyphen, the name still wins over the package"
        );
        assert!(!rows[0].by_description);
        let rows = search_apps(&apps, "chat", "en-US", 10);
        let simplex = rows
            .iter()
            .find(|r| {
                r.key
                    == AppKey::Android {
                        package: "chat.simplex.app".into(),
                    }
            })
            .unwrap();
        assert!(simplex.badges.contains(&Badge::ArmOnly));
        let aurora = search_apps(&apps, "aurora", "en-US", 1);
        assert!(aurora[0].badges.contains(&Badge::AntiFeatures));
        let mut map = BTreeMap::new();
        map.insert("en-US".to_owned(), "Hello".to_owned());
        map.insert("pt-BR".to_owned(), "Olá".to_owned());
        assert_eq!(localized(&map, "pt-BR"), Some("Olá"));
        assert_eq!(localized(&map, "pt_BR"), Some("Olá"));
        assert_eq!(localized(&map, "pt"), Some("Olá"));
        assert_eq!(localized(&map, "fr"), Some("Hello"));
    }

    #[test]
    fn a_kept_catalogue_replaces_the_one_before_and_is_found_by_package() {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-fdroid-{}-keep",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let roots = Roots::under(&dir);
        let apps = compact_index(std::io::Cursor::new(fixture("index-v2-cut.json"))).unwrap();
        let first = Catalogue {
            format: FORMAT,
            timestamp: 1,
            index_sha256: "a".repeat(64),
            apps,
        };
        keep(&roots, &first).unwrap();
        let second = Catalogue {
            index_sha256: "b".repeat(64),
            timestamp: 2,
            ..first
        };
        let path = keep(&roots, &second).unwrap();
        assert_eq!(current_file(&roots), Some(path));
        assert_eq!(std::fs::read_dir(directory(&roots)).unwrap().count(), 1);
        assert_eq!(
            app(&roots, "com.aurora.store").unwrap().version_name,
            "4.8.4"
        );
        assert!(app(&roots, "no.such.app").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_apk_is_a_zip_with_a_manifest_and_code() {
        assert_eq!(check_apk(&fixture("entry.jar")), Err(Refusal::NotAnApk));
        assert_eq!(check_apk(b"PK\x03\x04 not really"), Err(Refusal::NotAnApk));
    }

    #[cfg(feature = "net")]
    #[test]
    fn a_refresh_downloads_verifies_compacts_and_deletes_the_raw_index() {
        use crate::github::canned::Canned;
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-fdroid-{}-refresh",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let roots = Roots::under(&dir);
        // The real entry.jar vouches for the real index's hash, which the cut
        // does not have: served under it, the refresh must refuse it.
        let net = Canned::new(vec![(ENTRY, 200, vec![], fixture("entry.jar"))]).serving(
            "https://f-droid.org/repo/index-v2.json",
            fixture("index-v2-cut.json"),
        );
        let f = refresh_with(&net, &roots, &mut Vec::new()).unwrap_err();
        assert_eq!(f.refusal, Some(Refusal::Integrity));
        assert!(current_file(&roots).is_none());
        assert_eq!(
            std::fs::read_dir(directory(&roots)).unwrap().count(),
            0,
            "the raw index never stays"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The REAL index, 61 MB, compacted while measuring the peak memory of the
    /// process. Ignored: it needs the file. Run it with
    /// `RONOCTUA_FDROID_INDEX=<index-v2.json> cargo test … -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads a 61 MB file that is not in the repository"]
    fn the_real_index_compacts_in_little_memory() {
        let Some(path) = std::env::var_os("RONOCTUA_FDROID_INDEX") else {
            return;
        };
        let peak = || {
            std::fs::read_to_string("/proc/self/status")
                .unwrap_or_default()
                .lines()
                .find(|l| l.starts_with("VmHWM:"))
                .map(str::to_owned)
                .unwrap_or_default()
        };
        println!("before: {}", peak());
        let started = std::time::Instant::now();
        let file = std::fs::File::open(path).unwrap();
        let apps = compact_index(std::io::BufReader::with_capacity(1 << 20, file)).unwrap();
        println!(
            "{} apps in {:?}; after: {}",
            apps.len(),
            started.elapsed(),
            peak()
        );
        let arm = apps.iter().filter(|a| a.arm_only()).count();
        let bytes = serde_json::to_vec(&apps).unwrap().len();
        println!(
            "arm-only: {arm}; compacted: {bytes} bytes; with the JSON: {}",
            peak()
        );
        assert!(apps.len() > 4000);
    }
}
