//! The encryption (feature `crypt`): `age`, as a person could run it by hand.
//!
//! # One passphrase, one identity — post-quantum
//!
//! `key.age` is the person's identity (`AGE-SECRET-KEY-PQ-1…`, one line, the
//! hybrid `mlkem768x25519` of `ronoctua-pq`) encrypted to the PASSPHRASE with
//! scrypt; everything else — the index and every chunk — is sealed to that
//! identity's recipient (`age1pq1…`). So a sync pays scrypt once, changing the
//! passphrase re-encrypts one small file, and what the repository holds stays
//! sealed against "harvest now, decrypt later". Recovery without the distro is
//! Go `age` ≥ 1.3 (`RECOVER.md`, written into the repository).
//!
//! [`Keyring::open`] and [`Keyring::from_secret`] take a post-quantum
//! identity alone: any other text in `key.age` is [`Why::Corrupt`].
//!
//! # And the signing key, beside it
//!
//! The plain part is signed (`crate::plain`): ML-DSA-65, a key born with the
//! repository and written in `key.age` on the line after the identity, as a
//! comment — `# RONOCTUA-SIGNING-KEY-MLDSA65-1…` — so the passphrase that
//! opens the identity opens it, and Go `age` reading the same file as an
//! identity file (`RECOVER.md`) passes over it. One without the other is
//! [`Why::Corrupt`]: no repository of this format was ever made without it.
//!
//! # Chunk ids are keyed
//!
//! A chunk is named by HMAC-SHA256 of its plaintext under a key derived from
//! the identity ([`Keyring::chunk_id`]): equal content has one name (an
//! unchanged file never changes in git), and a name says nothing about the
//! content to whoever lacks the identity — never the content's own SHA-256,
//! which anybody holding a copy of a file could compute and look for. The
//! chunker's gear table is seeded from the same key ([`Keyring::seed`]), so
//! where a known file is cut — visible as the chunks' sizes — differs from
//! one person to the next.
//!
//! # Streams
//!
//! A chunk is sealed from memory (at most `chunk.maxKiB`) and the index and
//! `key.age` are small; a file is never held whole.

use std::io::{Read, Write};
use std::str::FromStr as _;

use age::secrecy::{ExposeSecret as _, SecretString};
use zeroize::Zeroizing;

use crate::{Refusal, Why};

/// The salt the chunk key is derived with (HKDF-SHA256, the identity as the
/// input key material).
pub const CHUNK_SALT: &[u8] = b"ronoctua-belongings chunk v2";
/// The HKDF info of the chunk ids' HMAC key.
pub const CHUNK_ID_INFO: &[u8] = b"id";
/// The HKDF info of the chunker's gear seed.
pub const CHUNK_SEED_INFO: &[u8] = b"seed";

/// The identity every chunk and the index are sealed to, the keys the
/// chunks are named and cut with, and the key the plain part is signed with.
pub struct Keyring {
    /// The person's post-quantum identity.
    identity: ronoctua_pq::Identity,
    /// Its recipient, public.
    recipient: ronoctua_pq::Recipient,
    /// The person's signing key (ML-DSA-65).
    signing: ronoctua_pq::sign::SigningKey,
    /// The HMAC key chunk ids are made with.
    ids: aws_lc_rs::hmac::Key,
    /// The chunker's gear seed.
    seed: u64,
}

impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // THE PUBLIC HALF ONLY: the identity never reaches a log.
        f.debug_struct("Keyring")
            .field("recipient", &self.recipient.to_string())
            .field("signing", &self.signing)
            .finish_non_exhaustive()
    }
}

/// A passphrase as `age` takes it, from what a door wrote.
#[must_use]
pub fn passphrase(text: String) -> SecretString {
    SecretString::from(text)
}

/// The length HKDF fills, for the seed.
struct Len(usize);

impl aws_lc_rs::hkdf::KeyType for Len {
    fn len(&self) -> usize {
        self.0
    }
}

/// The chunk-id key and the gear seed, derived from the identity as written.
///
/// # Errors
///
/// [`Why::Failed`] when HKDF refuses — only for an output longer than 255
/// hashes, which neither is; never a key made of nothing in its place.
fn derived(secret: &SecretString) -> Result<(aws_lc_rs::hmac::Key, u64), Refusal> {
    let failed = |_| Refusal::of(Why::Failed);
    let salt = aws_lc_rs::hkdf::Salt::new(aws_lc_rs::hkdf::HKDF_SHA256, CHUNK_SALT);
    let prk = salt.extract(secret.expose_secret().as_bytes());
    let ids = aws_lc_rs::hmac::Key::from(
        prk.expand(&[CHUNK_ID_INFO], aws_lc_rs::hmac::HMAC_SHA256)
            .map_err(failed)?,
    );
    let mut seed = [0u8; 8];
    prk.expand(&[CHUNK_SEED_INFO], Len(seed.len()))
        .and_then(|okm| okm.fill(&mut seed))
        .map_err(failed)?;
    Ok((ids, u64::from_le_bytes(seed)))
}

/// What `ronoctua-pq` refused, as the belongings say it when OPENING: what
/// does not open with this identity is not the person's, or is damaged.
const fn opened(refusal: ronoctua_pq::Refusal) -> Refusal {
    match refusal.why {
        ronoctua_pq::Why::WrongPassphrase => Refusal::of(Why::WrongPassphrase),
        ronoctua_pq::Why::WrongKey
        | ronoctua_pq::Why::Corrupt
        | ronoctua_pq::Why::Mixed
        | ronoctua_pq::Why::Failed => Refusal::of(Why::Corrupt),
    }
}

/// How the signing key is written in `key.age`: a comment line, for Go
/// `age` reading the file as an identity file.
const SIGNING_LINE: &str = "# ";

/// The identity and the signing key `text` writes (`key.age`'s plaintext,
/// or what the session kept): one line of each.
///
/// # Errors
///
/// [`Why::Corrupt`] when either is missing or is not one.
fn keys_of(text: &str) -> Result<(ronoctua_pq::Identity, ronoctua_pq::sign::SigningKey), Refusal> {
    let corrupt = Refusal::of(Why::Corrupt);
    let mut identity = None;
    let mut signing = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with(ronoctua_pq::IDENTITY_PREFIX) {
            identity = Some(ronoctua_pq::Identity::from_str(line).map_err(|_| corrupt)?);
        } else if let Some(key) = line
            .strip_prefix('#')
            .map(str::trim_start)
            .filter(|k| k.starts_with(ronoctua_pq::sign::SIGNING_KEY_PREFIX))
        {
            signing = Some(ronoctua_pq::sign::SigningKey::from_secret(key).map_err(|_| corrupt)?);
        }
    }
    identity.zip(signing).ok_or(corrupt)
}

impl Keyring {
    /// Holds `identity` and `signing`, with the keys derived from the
    /// identity.
    fn holding(
        identity: ronoctua_pq::Identity,
        signing: ronoctua_pq::sign::SigningKey,
    ) -> Result<Self, Refusal> {
        let (ids, seed) = derived(&identity.to_secret())?;
        let recipient = identity.recipient();
        Ok(Self {
            identity,
            recipient,
            signing,
            ids,
            seed,
        })
    }

    /// A new post-quantum identity and signing key, and their `key.age`
    /// encrypted to `passphrase`. `work` is scrypt's log₂ N: `None` lets
    /// `age` pick about a second on this machine; a test hands a small one.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when `age` could not encrypt, or there is no
    /// randomness for the signing key.
    pub fn new(passphrase: &SecretString, work: Option<u8>) -> Result<(Self, Vec<u8>), Refusal> {
        let signing =
            ronoctua_pq::sign::SigningKey::generate().map_err(|_| Refusal::of(Why::Failed))?;
        let ring = Self::holding(ronoctua_pq::Identity::generate(), signing)?;
        let sealed = ring.rewrap(passphrase, work)?;
        Ok((ring, sealed))
    }

    /// The post-quantum identity `key_age` holds, opened with `passphrase`.
    ///
    /// # Errors
    ///
    /// [`Why::WrongPassphrase`] when it does not open it, [`Why::Corrupt`]
    /// when what opened is not an identity or the file is not `age`.
    pub fn open(key_age: &[u8], passphrase: &SecretString) -> Result<Self, Refusal> {
        let mut text = Zeroizing::new(Vec::new());
        ronoctua_pq::open_passphrase(passphrase, &mut &key_age[..], &mut *text, &mut |_| {})
            .map_err(opened)?;
        let text = std::str::from_utf8(&text).map_err(|_| Refusal::of(Why::Corrupt))?;
        let (identity, signing) = keys_of(text)?;
        Self::holding(identity, signing)
    }

    /// The keyring its keys AS WRITTEN make ([`Keyring::secret`]: the
    /// identity, `AGE-SECRET-KEY-PQ-1…`, and the signing key) — what the
    /// session keeps in the kernel's keyring instead of the passphrase.
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] for text that is not a post-quantum identity and a
    /// signing key.
    pub fn from_secret(secret: &SecretString) -> Result<Self, Refusal> {
        let (identity, signing) = keys_of(secret.expose_secret())?;
        Self::holding(identity, signing)
    }

    /// The keys as written — the identity's line, then the signing key's,
    /// as a comment — for the places that keep them: `key.age`, a `0600`
    /// file or the kernel's keyring. Never a log, never an argument.
    #[must_use]
    pub fn secret(&self) -> SecretString {
        let identity = self.identity.to_secret();
        let signing = self.signing.to_secret();
        let mut text = Zeroizing::new(String::with_capacity(
            identity.expose_secret().len() + signing.len() + 8,
        ));
        text.push_str(identity.expose_secret());
        text.push('\n');
        text.push_str(SIGNING_LINE);
        text.push_str(&signing);
        text.push('\n');
        SecretString::from(text.to_string())
    }

    /// The person's signing key: what signs the plain part.
    #[must_use]
    pub const fn signing(&self) -> &ronoctua_pq::sign::SigningKey {
        &self.signing
    }

    /// The post-quantum recipient (`age1pq1…`), public: what "to myself"
    /// seals to.
    #[must_use]
    pub const fn recipient(&self) -> &ronoctua_pq::Recipient {
        &self.recipient
    }

    /// The identity, as `age` opens with it — for a file sealed to the person
    /// elsewhere (encrypt-anything's "to myself").
    #[must_use]
    pub fn identity(&self) -> &dyn age::Identity {
        &self.identity
    }

    /// Every identity this keyring opens with, as `age` takes a list — the
    /// shape encrypt-anything's "open a file sealed to me" hands `age`.
    #[must_use]
    pub fn identities(&self) -> Vec<&dyn age::Identity> {
        vec![self.identity()]
    }

    /// `key.age` again: the same identity encrypted to `passphrase`.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when `age` could not encrypt.
    pub fn rewrap(&self, passphrase: &SecretString, work: Option<u8>) -> Result<Vec<u8>, Refusal> {
        let secret = self.secret();
        let lines = Zeroizing::new(secret.expose_secret().as_bytes().to_vec());
        let mut out = Vec::new();
        ronoctua_pq::seal_passphrase(passphrase, work, &mut &lines[..], &mut out, &mut |_| {})
            .map_err(|_| Refusal::of(Why::Failed))?;
        Ok(out)
    }

    /// Seals everything `from` gives into `to`, to this identity's recipient.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when a read or a write failed.
    pub fn seal(&self, from: &mut dyn Read, to: &mut dyn Write) -> Result<(), Refusal> {
        let recipient: &dyn age::Recipient = &self.recipient;
        ronoctua_pq::seal(&[recipient], from, to, &mut |_| {})
            .map(|_| ())
            .map_err(|_| Refusal::of(Why::Failed))
    }

    /// The same, for bytes in hand.
    ///
    /// # Errors
    ///
    /// As [`Keyring::seal`].
    pub fn seal_bytes(&self, plain: &[u8]) -> Result<Vec<u8>, Refusal> {
        let mut out = Vec::with_capacity(plain.len() + 2048);
        self.seal(&mut &plain[..], &mut out)?;
        Ok(out)
    }

    /// Opens everything `from` gives into `to`.
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] when it is not `age` for this identity, or a read or
    /// a write failed half way.
    pub fn open_into(&self, from: &mut dyn Read, to: &mut dyn Write) -> Result<(), Refusal> {
        ronoctua_pq::open(&[self.identity()], from, to, &mut |_| {})
            .map(|_| ())
            .map_err(opened)
    }

    /// The same, into bytes in hand.
    ///
    /// # Errors
    ///
    /// As [`Keyring::open_into`].
    pub fn open_bytes(&self, sealed: &[u8]) -> Result<Vec<u8>, Refusal> {
        let mut out = Vec::new();
        self.open_into(&mut &sealed[..], &mut out)?;
        Ok(out)
    }

    /// A chunk's name: HMAC-SHA256 of its plaintext under the key derived
    /// from this identity, 64 lowercase hexadecimal characters.
    #[must_use]
    pub fn chunk_id(&self, plain: &[u8]) -> String {
        crate::walk::hex(aws_lc_rs::hmac::sign(&self.ids, plain).as_ref())
    }

    /// The seed of the chunker's gear table, derived from this identity.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small scrypt, so the tests do not pay a second each.
    const WORK: Option<u8> = Some(10);

    #[test]
    fn the_key_opens_only_with_its_passphrase_and_the_chunks_only_with_the_key() {
        let right = passphrase("correct horse battery".to_owned());
        let (ring, key_age) = Keyring::new(&right, WORK).unwrap();
        assert_eq!(ronoctua_pq::sniff(&key_age), ronoctua_pq::Kind::Passphrase);
        let reopened = Keyring::open(&key_age, &right).unwrap();
        let sealed = ring.seal_bytes(b"{\"theme\":\"owl\"}").unwrap();
        assert_eq!(
            ronoctua_pq::sniff(&sealed),
            ronoctua_pq::Kind::Pq,
            "a chunk was not sealed post-quantum"
        );
        assert!(
            !sealed.windows(5).any(|w| w == b"theme"),
            "the plain text is in the chunk"
        );
        assert_eq!(
            reopened.open_bytes(&sealed).unwrap(),
            b"{\"theme\":\"owl\"}"
        );

        let wrong = passphrase("incorrect horse".to_owned());
        assert_eq!(
            Keyring::open(&key_age, &wrong).unwrap_err().why,
            Why::WrongPassphrase
        );
        assert_eq!(
            Keyring::open(b"not age at all", &right).unwrap_err().why,
            Why::Corrupt
        );
        // ANOTHER identity cannot read the chunk.
        let (other, _) = Keyring::new(&right, WORK).unwrap();
        assert_eq!(other.open_bytes(&sealed).unwrap_err().why, Why::Corrupt);

        // A NEW PASSPHRASE opens the same identity; the old one no longer does.
        let new = passphrase("a new passphrase".to_owned());
        let rewrapped = reopened.rewrap(&new, WORK).unwrap();
        let again = Keyring::open(&rewrapped, &new).unwrap();
        assert_eq!(again.open_bytes(&sealed).unwrap(), b"{\"theme\":\"owl\"}");
        assert_eq!(
            Keyring::open(&rewrapped, &right).unwrap_err().why,
            Why::WrongPassphrase
        );
        // THE SAME IDENTITY, THE SAME NAMES: the chunk ids and the seed come
        // from the identity, not from the passphrase or the moment.
        assert_eq!(again.chunk_id(b"x"), ring.chunk_id(b"x"));
        assert_eq!(again.seed(), ring.seed());
        // AND THE IDENTITY NEVER REACHES A LOG.
        assert!(!format!("{again:?}").contains("AGE-SECRET-KEY"));
        // FROM ITS SECRET, the same keyring (the session's keyring path).
        let kept = Keyring::from_secret(&again.secret()).unwrap();
        assert_eq!(kept.recipient(), ring.recipient());
        assert!(kept.recipient().to_string().starts_with("age1pq1"));
        assert_eq!(kept.chunk_id(b"x"), ring.chunk_id(b"x"));
    }

    /// FAILS IF: a `key.age` or a kept secret that holds anything but a
    /// post-quantum identity — a classical X25519 one included — opens as a
    /// keyring: nothing would ever be sealed to it again, and a sync under it
    /// would seal classically.
    #[test]
    fn only_a_post_quantum_identity_opens() {
        let right = passphrase("the passphrase".to_owned());
        let classical = age::x25519::Identity::generate();
        let line = format!("{}\n", classical.to_string().expose_secret());
        let mut key_age = Vec::new();
        ronoctua_pq::seal_passphrase(
            &right,
            WORK,
            &mut line.as_bytes(),
            &mut key_age,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            Keyring::open(&key_age, &right).unwrap_err().why,
            Why::Corrupt
        );
        assert_eq!(
            Keyring::from_secret(&classical.to_string())
                .unwrap_err()
                .why,
            Why::Corrupt
        );
        assert_eq!(
            Keyring::from_secret(&passphrase("AGE-SECRET-KEY-PQ-1nonsense".to_owned()))
                .unwrap_err()
                .why,
            Why::Corrupt
        );
    }

    /// FAILS IF: the signing key does not travel in `key.age` beside the
    /// identity (as a comment Go `age` passes over), does not come back the
    /// same from the passphrase, a new passphrase or what the session keeps
    /// — or a `key.age` holding the identity alone opens as a keyring.
    #[test]
    fn the_signing_key_travels_in_key_age_beside_the_identity() {
        let right = passphrase("the passphrase".to_owned());
        let (ring, key_age) = Keyring::new(&right, WORK).unwrap();
        let mut text = Vec::new();
        ronoctua_pq::open_passphrase(&right, &mut &key_age[..], &mut text, &mut |_| {}).unwrap();
        let text = String::from_utf8(text).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{} lines", lines.len());
        assert!(lines[0].starts_with(ronoctua_pq::IDENTITY_PREFIX));
        assert!(
            lines[1].starts_with(&format!("# {}", ronoctua_pq::sign::SIGNING_KEY_PREFIX)),
            "the signing key is not a comment line"
        );
        let public = ring.signing().public().clone();
        assert_eq!(
            Keyring::open(&key_age, &right).unwrap().signing().public(),
            &public
        );
        let new = passphrase("another".to_owned());
        let rewrapped = ring.rewrap(&new, WORK).unwrap();
        assert_eq!(
            Keyring::open(&rewrapped, &new).unwrap().signing().public(),
            &public
        );
        assert_eq!(
            Keyring::from_secret(&ring.secret())
                .unwrap()
                .signing()
                .public(),
            &public
        );
        // THE IDENTITY ALONE is not a keyring of this format.
        let mut alone = Vec::new();
        let line = format!("{}\n", lines[0]);
        ronoctua_pq::seal_passphrase(&right, WORK, &mut line.as_bytes(), &mut alone, &mut |_| {})
            .unwrap();
        assert_eq!(Keyring::open(&alone, &right).unwrap_err().why, Why::Corrupt);
        // AND NEVER PRINTED.
        let printed = format!("{ring:?}");
        let seed = ring.signing().to_secret();
        assert!(!printed.contains(&seed[seed.len() - 20..]), "{printed}");
        assert!(printed.contains(&public.fingerprint()), "{printed}");
    }

    /// FAILS IF: a chunk's id is the content's own SHA-256, or two people's
    /// ids for the same content meet — a name would say what a file is to
    /// anybody holding a copy of it.
    #[test]
    fn chunk_ids_are_keyed() {
        let p = passphrase("p".to_owned());
        let (a, _) = Keyring::new(&p, WORK).unwrap();
        let (b, _) = Keyring::new(&p, WORK).unwrap();
        let content = b"the same content on two machines";
        let id = a.chunk_id(content);
        assert_eq!(id.len(), 64);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
        assert_eq!(id, a.chunk_id(content), "one content, one name");
        assert_ne!(id, b.chunk_id(content), "two identities, one name");
        let mut sha = aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA256);
        sha.update(content);
        assert_ne!(id, crate::walk::hex(sha.finish().as_ref()));
        assert_ne!(a.seed(), b.seed());
    }
}
