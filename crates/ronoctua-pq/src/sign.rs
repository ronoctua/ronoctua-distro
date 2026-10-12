//! The person's signatures (feature `sign`): ML-DSA-65 (FIPS 204), and the
//! signed manifest of a tree that the belongings' plain part carries.
//!
//! # Why this primitive, from this library
//!
//! What the plain part holds is read by every machine of the person's before
//! anybody types a passphrase — their declarations, their options, their
//! brain modules — so it is signed, and the signature has to outlive a
//! quantum computer as the sealed part does. ML-DSA-65 is the lattice
//! signature of FIPS 204 at the strength of ML-KEM-768 (the X-Wing half of
//! the identity), and it comes from `aws-lc-rs`, the one implementation of
//! the primitives our own code calls (`one-crypto-implementation`) — its
//! stable `signature` module (the `unstable` one only aliases it now, and
//! warns).
//!
//! # The key
//!
//! The secret is ML-DSA's 32-byte seed (FIPS 204's ξ): the whole key pair is
//! derived from it ([`SigningKey::from_secret`]). Written, it is
//! [`SIGNING_KEY_PREFIX`] and 64 upper-case hexadecimal digits — a line the
//! belongings keep inside `key.age`, under the passphrase, beside the X-Wing
//! identity. It exists only there, in a `0600` file, or in memory that is
//! wiped when dropped; its `Debug` shows the public key's fingerprint and
//! nothing else (`pq-identity-never-prints`).
//!
//! # The public key
//!
//! [`PUBLIC_KEY_PREFIX`] and the 1952 bytes of the raw key in unpadded,
//! canonical base64 — what a machine pins. Its fingerprint is the SHA-256
//! of those bytes, hexadecimal ([`PublicKey::fingerprint`]).
//!
//! # The manifest
//!
//! [`Manifest`]: the files of a tree by path, each with its SHA-256, a
//! counter that only grows, when it was signed and by whom (the
//! fingerprint). Its bytes AS WRITTEN are what is signed, after a fixed
//! context line ([`CONTEXT`]) so a signature made for it can never be taken
//! for one of something else; the signature travels beside it, detached, in
//! base64 ([`sign_manifest`], [`open_manifest`]). Which paths a tree covers,
//! and what a reader does with a file the manifest does not vouch for, is
//! the reader's: `ronoctua-person`'s tree for root, the belongings' plain
//! part for the person.

use std::collections::BTreeMap;
use std::str::FromStr;

use aws_lc_rs::signature::{
    KeyPair as _, ML_DSA_65, ML_DSA_65_SIGNING, PqdsaKeyPair, UnparsedPublicKey,
};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{Refusal, Why};

/// How a signing key is written: this, then the seed in 64 upper-case
/// hexadecimal digits.
pub const SIGNING_KEY_PREFIX: &str = "RONOCTUA-SIGNING-KEY-MLDSA65-1";

/// How a public key is written: this, then the raw key in unpadded base64.
pub const PUBLIC_KEY_PREFIX: &str = "ronoctua-mldsa65-1";

/// The bytes of ML-DSA's seed.
pub const SEED_BYTES: usize = 32;

/// The bytes of an ML-DSA-65 public key.
pub const PUBLIC_KEY_BYTES: usize = 1952;

/// The bytes of an ML-DSA-65 signature.
pub const SIGNATURE_BYTES: usize = 3309;

/// The one format of a manifest this build writes and reads.
pub const MANIFEST_FORMAT: u32 = 1;

/// What every signed manifest is prefixed with before it is signed: a
/// signature over a manifest is never one over anything else.
pub const CONTEXT: &[u8] = b"ronoctua signed manifest v1\n";

/// The SHA-256 of `bytes`, hexadecimal, lower case. Pure.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, bytes).as_ref())
}

/// Lower-case hexadecimal of `bytes`.
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    out
}

/// Whether `text` is a SHA-256 as [`sha256_hex`] writes one. Pure.
#[must_use]
pub fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The person's signing key: ML-DSA-65, from its seed.
pub struct SigningKey {
    seed: Zeroizing<[u8; SEED_BYTES]>,
    pair: PqdsaKeyPair,
    public: PublicKey,
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // THE FINGERPRINT ONLY: the seed never reaches a log.
        f.debug_struct("SigningKey")
            .field("fingerprint", &self.public.fingerprint())
            .finish_non_exhaustive()
    }
}

impl SigningKey {
    /// A new key, from the operating system's randomness.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when there is no randomness or the library refuses.
    pub fn generate() -> Result<Self, Refusal> {
        let mut seed = Zeroizing::new([0_u8; SEED_BYTES]);
        aws_lc_rs::rand::fill(&mut seed[..]).map_err(|_| Refusal::of(Why::Failed))?;
        Self::of_seed(seed)
    }

    fn of_seed(seed: Zeroizing<[u8; SEED_BYTES]>) -> Result<Self, Refusal> {
        let pair = PqdsaKeyPair::from_seed(&ML_DSA_65_SIGNING, &seed[..])
            .map_err(|_| Refusal::of(Why::Failed))?;
        let public = PublicKey::of_raw(pair.public_key().as_ref())?;
        Ok(Self { seed, pair, public })
    }

    /// The key as written ([`SIGNING_KEY_PREFIX`] and the seed): for
    /// `key.age` and a `0600` file, never for a log or an argument.
    #[must_use]
    pub fn to_secret(&self) -> Zeroizing<String> {
        const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
        let mut text = Zeroizing::new(String::with_capacity(
            SIGNING_KEY_PREFIX.len() + SEED_BYTES * 2,
        ));
        text.push_str(SIGNING_KEY_PREFIX);
        for b in self.seed.iter() {
            text.push(char::from(DIGITS[usize::from(b >> 4)]));
            text.push(char::from(DIGITS[usize::from(b & 0x0f)]));
        }
        text
    }

    /// The key a text writes ([`SigningKey::to_secret`]'s shape; spaces
    /// around it are a paste's).
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] for anything else.
    pub fn from_secret(text: &str) -> Result<Self, Refusal> {
        let corrupt = Refusal::of(Why::Corrupt);
        let digits = text
            .trim()
            .strip_prefix(SIGNING_KEY_PREFIX)
            .ok_or(corrupt)?
            .as_bytes();
        if digits.len() != SEED_BYTES * 2 {
            return Err(corrupt);
        }
        let value = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        };
        let mut seed = Zeroizing::new([0_u8; SEED_BYTES]);
        for (i, pair) in digits.chunks_exact(2).enumerate() {
            let (Some(high), Some(low)) = (value(pair[0]), value(pair[1])) else {
                return Err(corrupt);
            };
            seed[i] = (high << 4) | low;
        }
        Self::of_seed(seed)
    }

    /// Its public half.
    #[must_use]
    pub const fn public(&self) -> &PublicKey {
        &self.public
    }

    /// The signature of `message`.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when the library refuses.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, Refusal> {
        let mut signature = vec![0_u8; ML_DSA_65_SIGNING.signature_len()];
        let n = self
            .pair
            .sign(message, &mut signature)
            .map_err(|_| Refusal::of(Why::Failed))?;
        signature.truncate(n);
        Ok(signature)
    }
}

/// A signing key's public half — what a machine pins.
#[derive(Clone, PartialEq, Eq)]
pub struct PublicKey {
    raw: Box<[u8]>,
}

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicKey")
            .field("fingerprint", &self.fingerprint())
            .finish()
    }
}

impl PublicKey {
    /// The key of these raw bytes.
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] for bytes of the wrong length.
    pub fn of_raw(raw: &[u8]) -> Result<Self, Refusal> {
        if raw.len() != PUBLIC_KEY_BYTES {
            return Err(Refusal::of(Why::Corrupt));
        }
        Ok(Self { raw: raw.into() })
    }

    /// Its raw bytes.
    #[must_use]
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// The SHA-256 of its raw bytes, hexadecimal: what a manifest names its
    /// signer by, and what a screen shows.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        sha256_hex(&self.raw)
    }

    /// Whether `signature` is this key's over `message`.
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] for a signature of the wrong length, [`Why::WrongKey`]
    /// for one this key did not make over these bytes.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), Refusal> {
        if signature.len() != SIGNATURE_BYTES {
            return Err(Refusal::of(Why::Corrupt));
        }
        UnparsedPublicKey::new(&ML_DSA_65, &self.raw[..])
            .verify(message, signature)
            .map_err(|_| Refusal::of(Why::WrongKey))
    }
}

impl std::fmt::Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(PUBLIC_KEY_PREFIX)?;
        f.write_str(&STANDARD_NO_PAD.encode(&self.raw))
    }
}

impl FromStr for PublicKey {
    type Err = Refusal;

    /// The key [`PublicKey`]'s `Display` writes; spaces around it are a
    /// paste's.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let corrupt = Refusal::of(Why::Corrupt);
        let body = text.trim().strip_prefix(PUBLIC_KEY_PREFIX).ok_or(corrupt)?;
        // `STANDARD_NO_PAD` refuses `=` and non-zero trailing bits: one key,
        // one spelling.
        let raw = STANDARD_NO_PAD.decode(body).map_err(|_| corrupt)?;
        Self::of_raw(&raw)
    }
}

/// What a tree holds, signed: every file by path with its SHA-256, the
/// counter, when and by whom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Manifest {
    /// [`MANIFEST_FORMAT`].
    pub format: u32,
    /// Grows with every new signature of the same tree: a reader that saw a
    /// higher one refuses a lower one (an older tree replayed).
    pub counter: u64,
    /// When it was signed, seconds since the epoch.
    pub signed_at_s: i64,
    /// The signer's fingerprint ([`PublicKey::fingerprint`]).
    pub signer: String,
    /// Every file, by its path in the tree, with its SHA-256.
    pub files: BTreeMap<String, String>,
}

impl Manifest {
    /// The bytes a signature is made over: the JSON as written, in one
    /// canonical form (the fields in their order, the files sorted).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(self).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }

    /// Whether every path is a relative one with no step back and every
    /// hash is a SHA-256. Pure.
    #[must_use]
    pub fn is_whole(&self) -> bool {
        self.format == MANIFEST_FORMAT
            && is_sha256_hex(&self.signer)
            && self.files.iter().all(|(path, sha)| {
                is_sha256_hex(sha)
                    && !path.is_empty()
                    && path
                        .split('/')
                        .all(|part| !part.is_empty() && part != "." && part != "..")
            })
    }
}

/// The message a manifest's signature is over: [`CONTEXT`], then its bytes.
fn message(json: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(CONTEXT.len() + json.len());
    message.extend_from_slice(CONTEXT);
    message.extend_from_slice(json);
    message
}

/// `manifest`, signed by `key`: its bytes, and the detached signature in
/// unpadded base64 with a line end — the two files a tree carries. The
/// signer is `key`'s, whatever the manifest said.
///
/// # Errors
///
/// [`Why::Failed`] when the library refuses; [`Why::Corrupt`] for a manifest
/// that is not whole ([`Manifest::is_whole`]).
pub fn sign_manifest(key: &SigningKey, manifest: &Manifest) -> Result<(Vec<u8>, Vec<u8>), Refusal> {
    let manifest = Manifest {
        signer: key.public().fingerprint(),
        ..manifest.clone()
    };
    if !manifest.is_whole() {
        return Err(Refusal::of(Why::Corrupt));
    }
    let json = manifest.to_bytes();
    let signature = key.sign(&message(&json))?;
    let mut text = STANDARD_NO_PAD.encode(signature).into_bytes();
    text.push(b'\n');
    Ok((json, text))
}

/// The manifest `json` is, when `signature_text` is `key`'s signature over
/// it — and only then.
///
/// # Errors
///
/// [`Why::Corrupt`] for a signature that is not one or a manifest that does
/// not read whole; [`Why::WrongKey`] for a signature `key` did not make over
/// these bytes, or a manifest that names another signer.
pub fn open_manifest(
    json: &[u8],
    signature_text: &[u8],
    key: &PublicKey,
) -> Result<Manifest, Refusal> {
    let corrupt = Refusal::of(Why::Corrupt);
    let text = std::str::from_utf8(signature_text).map_err(|_| corrupt)?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    let signature = STANDARD_NO_PAD.decode(text).map_err(|_| corrupt)?;
    // THE SIGNATURE FIRST, over the bytes as they are: nothing of the JSON is
    // read before the key vouched for it.
    key.verify(&message(json), &signature)?;
    let manifest: Manifest = serde_json::from_slice(json).map_err(|_| corrupt)?;
    if !manifest.is_whole() {
        return Err(corrupt);
    }
    if manifest.signer != key.fingerprint() {
        return Err(Refusal::of(Why::WrongKey));
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(counter: u64) -> Manifest {
        Manifest {
            format: MANIFEST_FORMAT,
            counter,
            signed_at_s: 1_760_000_000,
            signer: String::new(),
            files: BTreeMap::from([
                ("person/options.json".to_owned(), sha256_hex(b"{}")),
                (
                    "brain-modules/my-tea/module.rs".to_owned(),
                    sha256_hex(b"pub fn tea() {}"),
                ),
            ]),
        }
    }

    /// FAILS IF: a key does not come back from its own text, two keys share
    /// a text, or the text of a key is anything but the prefix and 64
    /// upper-case digits.
    #[test]
    fn a_key_is_its_seed_and_comes_back_from_it() {
        let key = SigningKey::generate().unwrap();
        let text = key.to_secret();
        assert!(text.starts_with(SIGNING_KEY_PREFIX));
        assert_eq!(text.len(), SIGNING_KEY_PREFIX.len() + 64);
        let back = SigningKey::from_secret(&format!("  {}\n", text.as_str())).unwrap();
        assert_eq!(
            back.public(),
            key.public(),
            "the seed did not make the same key"
        );
        let other = SigningKey::generate().unwrap();
        assert_ne!(other.public(), key.public());
        for bad in [
            String::new(),
            SIGNING_KEY_PREFIX.to_owned(),
            text.to_lowercase(),
            format!("{}0", text.as_str()),
            text[..text.len() - 1].to_owned(),
            text.replacen(SIGNING_KEY_PREFIX, "RONOCTUA-SIGNING-KEY-MLDSA44-1", 1),
        ] {
            assert_eq!(
                SigningKey::from_secret(&bad).unwrap_err().why,
                Why::Corrupt,
                "{bad:.40}"
            );
        }
    }

    /// FAILS IF: a signing key's or a public key's `Debug` carries the seed,
    /// or does not show the fingerprint (an empty answer would pass).
    #[test]
    fn the_signing_key_never_prints() {
        let key = SigningKey::generate().unwrap();
        let secret = key.to_secret();
        let digits = &secret[SIGNING_KEY_PREFIX.len()..];
        for printed in [format!("{key:?}"), format!("{:?}", key.public())] {
            assert!(printed.contains(&key.public().fingerprint()), "{printed}");
            assert!(!printed.contains(SIGNING_KEY_PREFIX), "{printed}");
            assert!(!printed.contains(&digits[..16]), "{printed}");
            assert!(!printed.to_uppercase().contains(&digits[48..]), "{printed}");
        }
    }

    /// FAILS IF: a public key does not come back from its text, or a text
    /// cut short, padded, or of another prefix is taken for one.
    #[test]
    fn a_public_key_is_written_one_way() {
        let key = SigningKey::generate().unwrap();
        let text = key.public().to_string();
        assert!(text.starts_with(PUBLIC_KEY_PREFIX));
        assert_eq!(
            &PublicKey::from_str(&format!(" {text}\n")).unwrap(),
            key.public()
        );
        assert_eq!(key.public().fingerprint().len(), 64);
        for bad in [
            text[..text.len() - 4].to_owned(),
            format!("{text}=="),
            text.replacen(PUBLIC_KEY_PREFIX, "ronoctua-mldsa87-1", 1),
            String::new(),
        ] {
            assert!(PublicKey::from_str(&bad).is_err(), "{bad:.40}");
        }
    }

    /// FAILS IF: a manifest the key signed does not open, or one whose bytes
    /// changed by one, signed by another key, carrying another signer or a
    /// signature cut short opens.
    #[test]
    fn a_manifest_opens_only_as_its_key_signed_it() {
        let key = SigningKey::generate().unwrap();
        let (json, signature) = sign_manifest(&key, &manifest(7)).unwrap();
        let opened = open_manifest(&json, &signature, key.public()).unwrap();
        assert_eq!(opened.counter, 7);
        assert_eq!(opened.signer, key.public().fingerprint());
        assert_eq!(opened.files.len(), 2);

        // ONE BYTE OF THE JSON CHANGED: the counter rolled back by hand.
        let tampered = String::from_utf8(json.clone())
            .unwrap()
            .replace("\"counter\": 7", "\"counter\": 9")
            .into_bytes();
        assert_ne!(tampered, json);
        assert_eq!(
            open_manifest(&tampered, &signature, key.public())
                .unwrap_err()
                .why,
            Why::WrongKey
        );
        // ANOTHER KEY's signature, and a valid manifest of another signer.
        let other = SigningKey::generate().unwrap();
        let (their_json, their_signature) = sign_manifest(&other, &manifest(7)).unwrap();
        assert_eq!(
            open_manifest(&their_json, &their_signature, key.public())
                .unwrap_err()
                .why,
            Why::WrongKey
        );
        // A SIGNATURE CUT SHORT, or not base64.
        assert_eq!(
            open_manifest(&json, &signature[..100], key.public())
                .unwrap_err()
                .why,
            Why::Corrupt
        );
        assert_eq!(
            open_manifest(&json, b"not base64 at all!", key.public())
                .unwrap_err()
                .why,
            Why::Corrupt
        );
        // THE CONTEXT IS PART OF WHAT WAS SIGNED: a signature over the bare
        // bytes is not one over the manifest.
        let bare = key.sign(&json).unwrap();
        let mut bare_text = STANDARD_NO_PAD.encode(bare).into_bytes();
        bare_text.push(b'\n');
        assert_eq!(
            open_manifest(&json, &bare_text, key.public())
                .unwrap_err()
                .why,
            Why::WrongKey
        );
    }

    /// FAILS IF: a manifest that climbs out of its tree, names an empty
    /// path, or carries a hash that is not a SHA-256 is signed or opened.
    #[test]
    fn a_manifest_that_is_not_whole_is_neither_signed_nor_opened() {
        let key = SigningKey::generate().unwrap();
        for bad in ["../etc/shadow", "person//x", "", "./x", "person/.."] {
            let mut m = manifest(1);
            m.files.insert(bad.to_owned(), sha256_hex(b"x"));
            assert_eq!(
                sign_manifest(&key, &m).unwrap_err().why,
                Why::Corrupt,
                "{bad:?}"
            );
        }
        let mut m = manifest(1);
        m.files.insert("person/x".to_owned(), "ABC".to_owned());
        assert_eq!(sign_manifest(&key, &m).unwrap_err().why, Why::Corrupt);
        let mut newer = manifest(1);
        newer.format = MANIFEST_FORMAT + 1;
        newer.signer = key.public().fingerprint();
        let json = newer.to_bytes();
        let mut text = STANDARD_NO_PAD
            .encode(key.sign(&message(&json)).unwrap())
            .into_bytes();
        text.push(b'\n');
        assert_eq!(
            open_manifest(&json, &text, key.public()).unwrap_err().why,
            Why::Corrupt
        );
    }

    #[test]
    fn a_sha256_is_written_lower_case() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert!(is_sha256_hex(&sha256_hex(b"x")));
        assert!(!is_sha256_hex(&sha256_hex(b"x").to_uppercase()));
    }
}
