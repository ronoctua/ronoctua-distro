//! The identities, the recipients, and sealing and opening streams
//! (feature `crypt`).
//!
//! # The stanza (C2SP age, "mlkem768x25519 recipient stanza")
//!
//! The 16-byte file key is sealed with HPKE `SealBase` (RFC 9180 §6.1):
//! KEM `MLKEM768-X25519` (X-Wing, [`crate::xwing`]), KDF HKDF-SHA256, AEAD
//! ChaCha20-Poly1305, `pkR` = the recipient, `info` =
//! `age-encryption.org/mlkem768x25519`, `aad` empty. It travels as
//!
//! ```text
//! -> mlkem768x25519 <base64 enc: 1120 bytes, 1494 characters, one line>
//! <base64 ciphertext: 32 bytes>
//! ```
//!
//! and an identity ignores every stanza with another tag, and refuses one
//! with another number of arguments, a non-canonical or wrongly sized `enc`,
//! or a body that is not exactly 32 bytes — before it decrypts anything.
//!
//! # The names
//!
//! An identity is 32 random bytes, Bech32 under `AGE-SECRET-KEY-PQ-`; its
//! recipient is X-Wing's 1216-byte public key, Bech32 under `age1pq` — about
//! 1960 characters, past BIP-173's 90, so the checksum is age's Bech32 without
//! the length limit (`age_core::primitives::bech32_*`).

use std::collections::HashSet;
use std::io::{Read, Write};
use std::str::FromStr;

use age::secrecy::{ExposeSecret as _, SecretString};
use age_core::format::{FILE_KEY_BYTES, FileKey, Stanza};
use age_core::primitives::{bech32_decode, bech32_encode_to_fmt};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use bech32::{Bech32, Hrp};
use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::{Deserializable as _, OpModeR, OpModeS, Serializable as _};
use rand::RngCore as _;
use rand::rngs::OsRng;
use zeroize::Zeroize as _;

use crate::xwing::{self, ENC_BYTES, SEED_BYTES, XWing};
use crate::{Kind, LABEL, Refusal, STANZA, Why, sniff};

/// The identity's human-readable part (written upper case).
const IDENTITY_HRP: Hrp = Hrp::parse_unchecked("age-secret-key-pq-");

/// The recipient's human-readable part.
const RECIPIENT_HRP: Hrp = Hrp::parse_unchecked("age1pq");

/// HPKE's `info` for the stanza.
const INFO: &[u8] = b"age-encryption.org/mlkem768x25519";

/// The stanza's body: the file key and ChaCha20-Poly1305's tag.
const BODY_BYTES: usize = FILE_KEY_BYTES + 16;

/// The longest header [`open`] reads before deciding whether it is mixed.
///
/// A PQ stanza is some 1.56 KB (its 1494-character `enc` and its body), so
/// this is about 670 recipients. Go `age` sets no bound at all, and the old
/// 64 KiB one called a valid file of 43 recipients damaged. Past it the header
/// is not read, and the job [`Why::Failed`]: nothing says such a file is
/// damaged — a damaged one is refused long before, at its first byte that no
/// header has (see [`header`]).
const HEADER_MOST_BYTES: usize = 1024 * 1024;

/// A header's first line, newline included.
const INTRO: &[u8] = b"age-encryption.org/v1\n";

/// How often [`seal`] and [`open`] report progress, in bytes.
const BEAT_BYTES: u64 = 1024 * 1024;

/// A person's identity: what opens what was sealed to its [`Recipient`].
///
/// Wiped from memory when dropped; its `Debug` shows the public half only.
pub struct Identity {
    key: xwing::PrivateKey,
    recipient: Recipient,
}

impl Identity {
    /// A new identity, from the operating system's randomness.
    #[must_use]
    pub fn generate() -> Self {
        let mut seed = [0_u8; SEED_BYTES];
        OsRng.fill_bytes(&mut seed);
        let identity = Self::of_seed(&seed);
        seed.zeroize();
        identity
    }

    fn of_seed(seed: &[u8; SEED_BYTES]) -> Self {
        let recipient = Recipient {
            key: xwing::PublicKey(Box::new(xwing::expand(seed).public)),
        };
        Self {
            key: xwing::PrivateKey(*seed),
            recipient,
        }
    }

    /// The identity as written (`AGE-SECRET-KEY-PQ-1…`): for `key.age` and a
    /// `0600` file, never for a log or an argument.
    ///
    /// # Panics
    ///
    /// Never: 32 bytes under this prefix are 77 characters of Bech32.
    #[must_use]
    pub fn to_secret(&self) -> SecretString {
        let text = bech32::encode_upper::<Bech32>(IDENTITY_HRP, &self.key.0)
            .expect("32 bytes fit Bech32's 90 characters");
        SecretString::from(text)
    }

    /// Its recipient.
    #[must_use]
    pub fn recipient(&self) -> Recipient {
        self.recipient.clone()
    }
}

impl FromStr for Identity {
    type Err = Refusal;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let corrupt = Refusal::of(Why::Corrupt);
        let mut seed = bech32_decode(
            text.trim(),
            |_| corrupt,
            |hrp| {
                if hrp == IDENTITY_HRP {
                    Ok(())
                } else {
                    Err(corrupt)
                }
            },
            |_, bytes| {
                let mut seed = [0_u8; SEED_BYTES];
                let mut n = 0;
                for byte in bytes {
                    if n == SEED_BYTES {
                        seed.zeroize();
                        return Err(corrupt);
                    }
                    seed[n] = byte;
                    n += 1;
                }
                if n == SEED_BYTES {
                    Ok(seed)
                } else {
                    seed.zeroize();
                    Err(corrupt)
                }
            },
        )?;
        let identity = Self::of_seed(&seed);
        seed.zeroize();
        Ok(identity)
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // THE PUBLIC HALF ONLY: the identity never reaches a log.
        f.debug_struct("Identity")
            .field("recipient", &self.recipient.to_string())
            .finish_non_exhaustive()
    }
}

/// The 1120 bytes of a stanza's one argument, when it is canonical base64
/// of exactly that many.
fn enc_of(stanza: &Stanza) -> Option<xwing::EncappedKey> {
    let [arg] = &stanza.args[..] else {
        return None;
    };
    // `STANDARD_NO_PAD` refuses `=` and non-zero trailing bits: canonical.
    let bytes = STANDARD_NO_PAD.decode(arg).ok()?;
    if bytes.len() != ENC_BYTES {
        return None;
    }
    xwing::EncappedKey::from_bytes(&bytes).ok()
}

impl age::Identity for Identity {
    fn unwrap_stanza(&self, stanza: &Stanza) -> Option<Result<FileKey, age::DecryptError>> {
        if stanza.tag != STANZA {
            return None;
        }
        // THE SHAPE FIRST, before any decryption (the spec's MUSTs): one
        // argument, canonical, 1120 bytes; a body of exactly 32.
        let Some(enc) = enc_of(stanza) else {
            return Some(Err(age::DecryptError::InvalidHeader));
        };
        if stanza.body.len() != BODY_BYTES {
            return Some(Err(age::DecryptError::InvalidHeader));
        }
        match hpke::single_shot_open::<ChaCha20Poly1305, HkdfSha256, XWing>(
            &OpModeR::Base,
            &self.key,
            &enc,
            INFO,
            &stanza.body,
            &[],
        ) {
            Ok(mut plain) => {
                let key = (plain.len() == FILE_KEY_BYTES)
                    .then(|| FileKey::init_with_mut(|file_key| file_key.copy_from_slice(&plain)));
                plain.zeroize();
                Some(key.ok_or(age::DecryptError::InvalidHeader))
            }
            // A low-order X25519 share: malformed, as age's own X25519 says.
            Err(hpke::HpkeError::DecapError) => Some(Err(age::DecryptError::InvalidHeader)),
            // Not ours: another identity may still open it.
            Err(_) => None,
        }
    }
}

/// Where a file is sealed to: public, written `age1pq1…`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Recipient {
    key: xwing::PublicKey,
}

impl FromStr for Recipient {
    type Err = Refusal;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let corrupt = Refusal::of(Why::Corrupt);
        bech32_decode(
            text.trim(),
            |_| corrupt,
            |hrp| {
                if hrp == RECIPIENT_HRP {
                    Ok(())
                } else {
                    Err(corrupt)
                }
            },
            |_, bytes| {
                // The public key's own reading refuses what Go refuses (a
                // non-canonical ML-KEM key, an X25519 share of low order).
                xwing::PublicKey::from_bytes(&bytes.collect::<Vec<u8>>())
                    .map(|key| Self { key })
                    .map_err(|_| corrupt)
            },
        )
    }
}

impl std::fmt::Display for Recipient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        bech32_encode_to_fmt(f, RECIPIENT_HRP, &self.key.0[..])
    }
}

impl std::fmt::Debug for Recipient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recipient({self})")
    }
}

impl serde::Serialize for Recipient {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Recipient {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::from_str(&text).map_err(serde::de::Error::custom)
    }
}

impl age::Recipient for Recipient {
    fn wrap_file_key(
        &self,
        file_key: &FileKey,
    ) -> Result<(Vec<Stanza>, HashSet<String>), age::EncryptError> {
        let (enc, body) = hpke::single_shot_seal::<ChaCha20Poly1305, HkdfSha256, XWing, _>(
            &OpModeS::Base,
            &self.key,
            INFO,
            file_key.expose_secret(),
            &[],
            &mut OsRng,
        )
        .map_err(|_| {
            age::EncryptError::Io(std::io::Error::other(
                "mlkem768x25519: the encapsulation failed",
            ))
        })?;
        Ok((
            vec![Stanza {
                tag: STANZA.to_owned(),
                args: vec![STANDARD_NO_PAD.encode(enc.to_bytes())],
                body,
            }],
            std::iter::once(LABEL.to_owned()).collect(),
        ))
    }
}

/// Copies `from` into `to` and counts, saying where it is every
/// [`BEAT_BYTES`].
fn pump(
    from: &mut dyn Read,
    to: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
    failed: Why,
) -> Result<u64, Refusal> {
    // What passes through is plaintext on one side: wiped on every way out.
    let mut buffer = zeroize::Zeroizing::new(vec![0_u8; 64 * 1024]);
    let (mut done, mut said) = (0_u64, 0_u64);
    loop {
        let n = match from.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Refusal::of(failed)),
        };
        to.write_all(&buffer[..n])
            .map_err(|_| Refusal::of(Why::Failed))?;
        done += n as u64;
        if done - said >= BEAT_BYTES {
            said = done;
            progress(done);
        }
    }
    progress(done);
    Ok(done)
}

fn encrypt_error(e: &age::EncryptError) -> Refusal {
    match e {
        age::EncryptError::IncompatibleRecipients { .. }
        | age::EncryptError::InvalidRecipientLabels(_)
        | age::EncryptError::MixedRecipientAndPassphrase => Refusal::of(Why::Mixed),
        _ => Refusal::of(Why::Failed),
    }
}

fn seal_with(
    encryptor: age::Encryptor,
    from: &mut dyn Read,
    to: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, Refusal> {
    let mut writer = encryptor
        .wrap_output(to)
        .map_err(|_| Refusal::of(Why::Failed))?;
    let done = pump(from, &mut writer, progress, Why::Failed)?;
    writer.finish().map_err(|_| Refusal::of(Why::Failed))?;
    Ok(done)
}

/// Seals everything `from` gives into `to`, for these recipients; `progress`
/// hears the bytes read so far. Gives back how many were read.
///
/// # Errors
///
/// [`Why::Mixed`] when the recipients' labels differ (a PQ one with a
/// classical one), [`Why::Failed`] for none or for a read or write that failed.
pub fn seal(
    recipients: &[&dyn age::Recipient],
    from: &mut dyn Read,
    to: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, Refusal> {
    if recipients.is_empty() {
        return Err(Refusal::of(Why::Failed));
    }
    let encryptor = age::Encryptor::with_recipients(recipients.iter().copied())
        .map_err(|e| encrypt_error(&e))?;
    seal_with(encryptor, from, to, progress)
}

/// scrypt's log₂ N for every passphrase sealed here: 18, what the reference
/// `age` (Go) writes — 256 MiB (`2^18 × 8 × 128` bytes), a second or so on a
/// slow laptop.
///
/// # Never the machine's measure
///
/// `age` alone times scrypt where it runs and picks about a second: 20 on a
/// fast idle machine, a gibibyte, which a 1 GiB VM could not hold (the
/// belongings' `prepare` died there allocating it, 2026-10-23), and a weaker
/// factor on a busy one. It also opens only up to its measure plus four, so a
/// `key.age` sealed on a fast desktop could be refused on a slow laptop, or on
/// the same desktop while it was busy. Sealed at a constant and opened under
/// one, a file is the same and opens the same on every machine.
pub const PASSPHRASE_WORK: u8 = 18;

/// The most an opening pays: 22, the reference `age`'s ceiling (4 GiB) — over
/// every factor `age` measured for a `key.age` sealed before
/// [`PASSPHRASE_WORK`], and what a file from the Go `age` may carry. A file
/// past it is refused before any scrypt runs ([`Why::Failed`]).
pub const PASSPHRASE_WORK_MOST: u8 = 22;

/// Seals to a passphrase (scrypt). `work` is scrypt's log₂ N: `None` is
/// [`PASSPHRASE_WORK`]; a test hands a small one.
///
/// # Errors
///
/// As [`seal`].
pub fn seal_passphrase(
    passphrase: &SecretString,
    work: Option<u8>,
    from: &mut dyn Read,
    to: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, Refusal> {
    let mut recipient = age::scrypt::Recipient::new(passphrase.clone());
    recipient.set_work_factor(work.unwrap_or(PASSPHRASE_WORK));
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|e| encrypt_error(&e))?;
    seal_with(encryptor, from, to, progress)
}

/// Reads the header (up to `---`) off `from`, refusing what is not age or
/// is mixed, and gives back the bytes it read.
///
/// What is not a header is refused at its first byte that says so: a first
/// line that is not [`INTRO`], or a byte that is not printable ASCII — every
/// header line is (stanza arguments, base64 bodies, the MAC), so such a byte
/// is the binary payload of a file whose `---` line is gone, and reading on
/// would only reach [`HEADER_MOST_BYTES`] to say less.
fn header(from: &mut dyn Read) -> Result<(Vec<u8>, Kind), Refusal> {
    let corrupt = Refusal::of(Why::Corrupt);
    let mut head = Vec::with_capacity(4096);
    let mut byte = [0_u8; 1];
    let mut line_start = 0;
    loop {
        match from.read(&mut byte) {
            Ok(0) => return Err(corrupt),
            Ok(_) => head.push(byte[0]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Refusal::of(Why::Failed)),
        }
        if head.len() <= INTRO.len() {
            if head[..] != INTRO[..head.len()] {
                return Err(corrupt);
            }
            if head.len() == INTRO.len() {
                line_start = head.len();
            }
            continue;
        }
        match byte[0] {
            b'\n' => {
                if head[line_start..].starts_with(b"---") {
                    break;
                }
                line_start = head.len();
            }
            b' '..=b'~' => {}
            _ => return Err(corrupt),
        }
        if head.len() > HEADER_MOST_BYTES {
            return Err(Refusal::of(Why::Failed));
        }
    }
    match sniff(&head) {
        Kind::NotAge => Err(Refusal::of(Why::Corrupt)),
        Kind::Mixed => Err(Refusal::of(Why::Mixed)),
        kind => Ok((head, kind)),
    }
}

fn open_with(
    identities: &[&dyn age::Identity],
    from: &mut dyn Read,
    to: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
    wrong: Why,
) -> Result<u64, Refusal> {
    let (head, _) = header(from)?;
    let whole = std::io::Cursor::new(head).chain(from);
    let decryptor = age::Decryptor::new(whole).map_err(|_| Refusal::of(Why::Corrupt))?;
    let mut reader = decryptor
        .decrypt(identities.iter().copied())
        .map_err(|e| match e {
            age::DecryptError::NoMatchingKeys
            | age::DecryptError::DecryptionFailed
            | age::DecryptError::KeyDecryptionFailed => Refusal::of(wrong),
            age::DecryptError::ExcessiveWork { .. } | age::DecryptError::Io(_) => {
                Refusal::of(Why::Failed)
            }
            _ => Refusal::of(Why::Corrupt),
        })?;
    pump(&mut reader, to, progress, Why::Corrupt)
}

/// Opens everything `from` gives into `to` with any of these identities.
/// Gives back how many plaintext bytes were written.
///
/// # Errors
///
/// [`Why::WrongKey`] when none opens it, [`Why::Mixed`] for a header that
/// mixes PQ and classical stanzas, [`Why::Corrupt`] for what is not age or is
/// damaged, [`Why::Failed`] for a read or write that failed and for a header
/// past [`HEADER_MOST_BYTES`] (about 670 PQ recipients).
pub fn open(
    identities: &[&dyn age::Identity],
    from: &mut dyn Read,
    to: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, Refusal> {
    open_with(identities, from, to, progress, Why::WrongKey)
}

/// Opens what was sealed to a passphrase, at a work up to
/// [`PASSPHRASE_WORK_MOST`] — the same on every machine.
///
/// # Errors
///
/// [`Why::WrongPassphrase`] when it does not open it; otherwise as [`open`].
pub fn open_passphrase(
    passphrase: &SecretString,
    from: &mut dyn Read,
    to: &mut dyn Write,
    progress: &mut dyn FnMut(u64),
) -> Result<u64, Refusal> {
    let mut scrypt = age::scrypt::Identity::new(passphrase.clone());
    scrypt.set_max_work_factor(PASSPHRASE_WORK_MOST);
    open_with(
        &[&scrypt as &dyn age::Identity],
        from,
        to,
        progress,
        Why::WrongPassphrase,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Option<u8> = Some(10);

    fn sealed_to(recipients: &[&dyn age::Recipient], plain: &[u8]) -> Result<Vec<u8>, Refusal> {
        let mut out = Vec::new();
        seal(recipients, &mut &plain[..], &mut out, &mut |_| {})?;
        Ok(out)
    }

    fn opened(identities: &[&dyn age::Identity], sealed: &[u8]) -> Result<Vec<u8>, Refusal> {
        let mut plain = Vec::new();
        open(identities, &mut &sealed[..], &mut plain, &mut |_| {})?;
        Ok(plain)
    }

    /// The header's one PQ stanza, as text lines (`-> …`, body).
    fn stanza_lines(sealed: &[u8]) -> (String, String) {
        let text = String::from_utf8_lossy(sealed);
        let mut lines = text.lines();
        let arrow = lines
            .find(|l| l.starts_with("-> mlkem768x25519"))
            .unwrap()
            .to_owned();
        (arrow, lines.next().unwrap().to_owned())
    }

    #[test]
    fn an_identity_opens_what_its_recipient_sealed_and_nothing_else() {
        let me = Identity::generate();
        let sealed = sealed_to(&[&me.recipient()], b"a secret").unwrap();
        assert_eq!(sniff(&sealed), Kind::Pq);
        assert!(!sealed.windows(6).any(|w| w == b"secret"));
        assert_eq!(opened(&[&me], &sealed).unwrap(), b"a secret");

        let other = Identity::generate();
        assert_eq!(opened(&[&other], &sealed).unwrap_err().why, Why::WrongKey);
        // Either of two opens a file sealed to both, whichever comes first.
        let sealed = sealed_to(&[&other.recipient(), &me.recipient()], b"both").unwrap();
        assert_eq!(opened(&[&me], &sealed).unwrap(), b"both");
        assert_eq!(opened(&[&other, &me], &sealed).unwrap(), b"both");
        let refused = opened(&[&me], b"not age");
        assert_eq!(refused.unwrap_err().why, Why::Corrupt);
    }

    #[test]
    fn the_stanza_is_the_one_the_spec_writes() {
        let me = Identity::generate();
        let sealed = sealed_to(&[&me.recipient()], b"x").unwrap();
        let (arrow, body) = stanza_lines(&sealed);
        let args: Vec<&str> = arrow.split(' ').collect();
        assert_eq!(args.len(), 3, "-> tag enc");
        assert_eq!(args[2].len(), 1494, "1120 bytes, unpadded base64");
        assert_eq!(STANDARD_NO_PAD.decode(args[2]).unwrap().len(), ENC_BYTES);
        assert_eq!(STANDARD_NO_PAD.decode(&body).unwrap().len(), BODY_BYTES);
    }

    #[test]
    fn a_malformed_stanza_is_refused_before_decrypting() {
        let me = Identity::generate();
        let sealed = sealed_to(&[&me.recipient()], b"x").unwrap();
        let (arrow, body) = stanza_lines(&sealed);
        let enc = arrow.rsplit(' ').next().unwrap().to_owned();
        let stanza = |args: Vec<String>, body: Vec<u8>| Stanza {
            tag: STANZA.to_owned(),
            args,
            body,
        };
        let body = STANDARD_NO_PAD.decode(body).unwrap();
        let invalid = |s: &Stanza| {
            matches!(
                age::Identity::unwrap_stanza(&me, s),
                Some(Err(age::DecryptError::InvalidHeader))
            )
        };
        // The good one opens…
        let good = stanza(vec![enc.clone()], body.clone());
        assert!(matches!(
            age::Identity::unwrap_stanza(&me, &good),
            Some(Ok(_))
        ));
        // …and each of the spec's MUSTs refuses its own breakage.
        assert!(invalid(&stanza(vec![], body.clone())));
        assert!(invalid(&stanza(
            vec![enc.clone(), "x".into()],
            body.clone()
        )));
        assert!(invalid(&stanza(vec![format!("{enc}==")], body.clone())));
        assert!(invalid(&stanza(
            vec![enc[..enc.len() - 4].into()],
            body.clone()
        )));
        // 1120 = 3·373 + 1: the last character carries two bits and four
        // spare ones, which are zero; one more sets a spare bit.
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut last = enc.clone().into_bytes();
        let at = ALPHABET
            .iter()
            .position(|&c| Some(&c) == last.last())
            .unwrap();
        assert_eq!(at % 16, 0, "canonical: no spare bit set");
        *last.last_mut().unwrap() = ALPHABET[at + 1];
        let last = String::from_utf8(last).unwrap();
        assert!(STANDARD_NO_PAD.decode(&last).is_err());
        assert!(invalid(&stanza(vec![last], body.clone())));
        assert!(invalid(&stanza(vec![enc.clone()], body[..31].to_vec())));
        let mut longer = body.clone();
        longer.push(0);
        assert!(invalid(&stanza(vec![enc.clone()], longer)));
        // Another tag is not ours at all.
        let mut other = stanza(vec![enc], body);
        other.tag = "X25519".into();
        assert!(age::Identity::unwrap_stanza(&me, &other).is_none());
        // A stranger's identity passes over a well-formed stanza.
        assert!(age::Identity::unwrap_stanza(&Identity::generate(), &good).is_none());
    }

    #[test]
    fn identities_and_recipients_are_written_under_their_names() {
        let me = Identity::generate();
        let secret = me.to_secret();
        assert!(secret.expose_secret().starts_with(crate::IDENTITY_PREFIX));
        assert_eq!(secret.expose_secret().len(), 77);
        let again = Identity::from_str(secret.expose_secret()).unwrap();
        assert_eq!(again.recipient(), me.recipient());
        // Bech32 has one case at a time; the lower one is the same key.
        let lower = Identity::from_str(&secret.expose_secret().to_lowercase()).unwrap();
        assert_eq!(lower.recipient(), me.recipient());

        let text = me.recipient().to_string();
        assert!(text.starts_with(crate::RECIPIENT_PREFIX), "{text}");
        assert!(
            text.len() > 1900,
            "past Bech32's 90 characters: {}",
            text.len()
        );
        assert_eq!(Recipient::from_str(&text).unwrap(), me.recipient());
        let json = serde_json::to_string(&me.recipient()).unwrap();
        assert_eq!(
            serde_json::from_str::<Recipient>(&json).unwrap(),
            me.recipient()
        );

        // A changed character breaks the checksum.
        let mut broken = text.clone().into_bytes();
        let at = broken.len() / 2;
        broken[at] = if broken[at] == b'q' { b'p' } else { b'q' };
        assert!(Recipient::from_str(std::str::from_utf8(&broken).unwrap()).is_err());

        // AND THE IDENTITY NEVER REACHES A LOG.
        let debug = format!("{me:?}");
        assert!(!debug.contains("SECRET"), "{debug}");
        let data = &secret.expose_secret()[crate::IDENTITY_PREFIX.len()..];
        assert!(!debug.to_uppercase().contains(&data[..20]), "{debug}");
        assert!(debug.contains(&text));
        assert!(!format!("{secret:?}").contains(&data[..20]));

        // A classical age key is not a PQ one, either way round.
        let classical = age::x25519::Identity::generate();
        assert!(Identity::from_str(classical.to_string().expose_secret()).is_err());
        assert!(Recipient::from_str(&classical.to_public().to_string()).is_err());
        assert!(Recipient::from_str(secret.expose_secret()).is_err());
        assert!(Identity::from_str(&text).is_err());
    }

    #[test]
    fn pq_and_classical_never_travel_together() {
        let me = Identity::generate();
        let classical = age::x25519::Identity::generate().to_public();
        let refused = sealed_to(&[&me.recipient(), &classical], b"x");
        assert_eq!(refused.unwrap_err().why, Why::Mixed);
        let refused = sealed_to(&[&classical, &me.recipient()], b"x");
        assert_eq!(refused.unwrap_err().why, Why::Mixed);
    }

    #[test]
    fn a_passphrase_opens_only_what_it_sealed() {
        let right = SecretString::from("correct horse battery".to_owned());
        let mut sealed = Vec::new();
        let mut heard = Vec::new();
        seal_passphrase(&right, WORK, &mut &b"note"[..], &mut sealed, &mut |n| {
            heard.push(n);
        })
        .unwrap();
        assert_eq!(heard.last(), Some(&4));
        assert_eq!(sniff(&sealed), Kind::Passphrase);
        let mut plain = Vec::new();
        open_passphrase(&right, &mut &sealed[..], &mut plain, &mut |_| {}).unwrap();
        assert_eq!(plain, b"note");
        let wrong = SecretString::from("incorrect".to_owned());
        let refused = open_passphrase(&wrong, &mut &sealed[..], &mut Vec::new(), &mut |_| {});
        assert_eq!(refused.unwrap_err().why, Why::WrongPassphrase);
    }

    /// The `-> scrypt <salt> <log₂ N>` line of a passphrase's header.
    fn scrypt_line(sealed: &[u8]) -> String {
        String::from_utf8_lossy(sealed)
            .lines()
            .find(|l| l.starts_with("-> scrypt "))
            .unwrap()
            .to_owned()
    }

    /// FAILS IF: a passphrase is sealed at a work this machine measured
    /// instead of [`PASSPHRASE_WORK`] — a gibibyte on a fast idle machine,
    /// which a small one cannot hold, and a weaker factor on a busy one.
    #[test]
    fn a_passphrase_is_sealed_at_one_work_on_every_machine() {
        let right = SecretString::from("correct horse battery".to_owned());
        let mut sealed = Vec::new();
        seal_passphrase(&right, None, &mut &b"note"[..], &mut sealed, &mut |_| {}).unwrap();
        let line = scrypt_line(&sealed);
        assert_eq!(
            line.rsplit_once(' ').map(|(_, work)| work),
            Some(PASSPHRASE_WORK.to_string().as_str()),
            "{line}"
        );
    }

    /// FAILS IF: what an opening pays is bounded by this machine's measure
    /// instead of [`PASSPHRASE_WORK_MOST`]: a fast machine would run a
    /// file's scrypt past the ceiling (8 GiB at 23) and die allocating it,
    /// and a slow one would refuse a `key.age` that a fast one sealed.
    #[test]
    fn an_opening_pays_at_most_the_ceiling_on_every_machine() {
        let right = SecretString::from("correct horse battery".to_owned());
        let mut sealed = Vec::new();
        seal_passphrase(&right, WORK, &mut &b"note"[..], &mut sealed, &mut |_| {}).unwrap();
        let line = scrypt_line(&sealed);
        let at = sealed
            .windows(line.len())
            .position(|w| w == line.as_bytes())
            .unwrap();
        let (head, _) = line.rsplit_once(' ').unwrap();
        let mut heavier = sealed[..at].to_vec();
        heavier.extend_from_slice(format!("{head} {}", PASSPHRASE_WORK_MOST + 1).as_bytes());
        heavier.extend_from_slice(&sealed[at + line.len()..]);
        let refused = open_passphrase(&right, &mut &heavier[..], &mut Vec::new(), &mut |_| {});
        assert_eq!(refused.unwrap_err().why, Why::Failed);
    }

    #[test]
    fn a_header_of_many_pq_recipients_opens() {
        // 45 stanzas are some 70 KB of header: more than the old 64 KiB bound,
        // which called such a file (Go seals it gladly) damaged.
        let all: Vec<Identity> = (0..45).map(|_| Identity::generate()).collect();
        let recipients: Vec<Recipient> = all.iter().map(Identity::recipient).collect();
        let dyns: Vec<&dyn age::Recipient> = recipients
            .iter()
            .map(|r| r as &dyn age::Recipient)
            .collect();
        let sealed = sealed_to(&dyns, b"to many").unwrap();
        let end = sealed.windows(4).position(|w| w == b"\n---").unwrap();
        assert!(end > 64 * 1024, "the header is {end} bytes");
        assert_eq!(sniff(&sealed), Kind::Pq);
        // The last recipient's identity: every stanza before its own is tried.
        assert_eq!(opened(&[&all[44]], &sealed).unwrap(), b"to many");
        assert_eq!(
            opened(&[&Identity::generate()], &sealed).unwrap_err().why,
            Why::WrongKey
        );
    }

    /// A reader that counts what was taken from it.
    struct Counted<'a> {
        bytes: &'a [u8],
        taken: usize,
    }

    impl Read for Counted<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.bytes.read(buf)?;
            self.taken += n;
            Ok(n)
        }
    }

    fn refused_after(bytes: &[u8]) -> (Why, usize) {
        let me = Identity::generate();
        let mut from = Counted { bytes, taken: 0 };
        let why = open(&[&me], &mut from, &mut Vec::new(), &mut |_| {})
            .unwrap_err()
            .why;
        (why, from.taken)
    }

    #[test]
    fn what_no_header_has_is_refused_at_once_and_a_long_one_is_not_called_damaged() {
        let me = Identity::generate();
        let sealed = sealed_to(&[&me.recipient()], &[0_u8; 4096]).unwrap();
        let end = sealed.windows(4).position(|w| w == b"\n---").unwrap() + 1;

        // A file whose `---` line was damaged: refused at the payload's first
        // byte that no header has, not a mebibyte later.
        let mut damaged = sealed;
        damaged[end] = b'x';
        let (why, taken) = refused_after(&damaged);
        assert_eq!(why, Why::Corrupt);
        assert!(
            taken < end + 200,
            "read {taken} bytes of a {end}-byte header"
        );

        // Not age: refused at the first byte of the first line.
        assert_eq!(refused_after(b"not age at all"), (Why::Corrupt, 1));
        let (why, taken) = refused_after(b"age-encryption.org/v2\n-> X25519 a\n");
        assert_eq!((why, taken), (Why::Corrupt, 21));
        assert_eq!(refused_after(b"").0, Why::Corrupt);
        assert_eq!(refused_after(INTRO).0, Why::Corrupt);

        // Past the bound, a header of well-formed lines is not read, and it is
        // not called damaged: nothing says it is.
        let mut long = INTRO.to_vec();
        let line = format!("-> {STANZA} {}\n", "A".repeat(1494));
        while long.len() <= HEADER_MOST_BYTES {
            long.extend_from_slice(line.as_bytes());
        }
        long.extend_from_slice(b"--- AAAA\n");
        let (why, taken) = refused_after(&long);
        assert_eq!(why, Why::Failed);
        assert_eq!(taken, HEADER_MOST_BYTES + 1);
    }

    #[test]
    fn progress_beats_every_mebibyte_and_ends_on_the_total() {
        let me = Identity::generate();
        let plain = vec![0x61_u8; 3 * 1024 * 1024 + 5];
        let mut sealed = Vec::new();
        let mut heard = Vec::new();
        let read = seal(&[&me.recipient()], &mut &plain[..], &mut sealed, &mut |n| {
            heard.push(n);
        })
        .unwrap();
        assert_eq!(read, plain.len() as u64);
        assert!(heard.len() >= 4, "{heard:?}");
        assert_eq!(heard.last(), Some(&read));
        let mut back = Vec::new();
        let written = open(&[&me], &mut &sealed[..], &mut back, &mut |_| {}).unwrap();
        assert_eq!((written, back == plain), (read, true));
    }
}
