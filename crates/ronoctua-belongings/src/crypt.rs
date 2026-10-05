//! The encryption (feature `crypt`): `age`, as a person could run it by hand.
//!
//! # One passphrase, one identity
//!
//! `key.age` is an X25519 identity (`AGE-SECRET-KEY-1…`, one line) encrypted
//! to the PASSPHRASE with scrypt; everything else — the index and every blob —
//! is encrypted to that identity's recipient. So a sync pays scrypt once, and
//! changing the passphrase re-encrypts one small file. Recovery without the
//! distro is the `age` CLI:
//!
//! ```text
//! age -d key.age > identity        (asks the passphrase)
//! age -d -i identity index.age     (the paths, and each blob's id)
//! age -d -i identity blobs/<id>.age > the-file
//! ```
//!
//! # Streams
//!
//! A blob is encrypted from the file and decrypted into a draft beside its
//! place, never held whole: a folder the person added may hold a video.

use std::io::{Read, Write};
use std::str::FromStr as _;

use age::secrecy::{ExposeSecret as _, SecretString};

use crate::{Refusal, Why};

/// The identity every blob and the index are encrypted to.
pub struct Keyring {
    identity: age::x25519::Identity,
    recipient: age::x25519::Recipient,
}

impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // THE PUBLIC HALF ONLY: the identity never reaches a log.
        f.debug_struct("Keyring")
            .field("recipient", &self.recipient.to_string())
            .finish_non_exhaustive()
    }
}

/// A passphrase as `age` takes it, from what a door wrote.
#[must_use]
pub fn passphrase(text: String) -> SecretString {
    SecretString::from(text)
}

impl Keyring {
    /// A new identity, and its `key.age` encrypted to `passphrase`. `work` is
    /// scrypt's log₂ N: `None` lets `age` pick about a second on this machine;
    /// a test hands a small one.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when `age` could not encrypt.
    pub fn new(passphrase: &SecretString, work: Option<u8>) -> Result<(Self, Vec<u8>), Refusal> {
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public();
        let sealed = seal_key(&identity, passphrase, work)?;
        Ok((
            Self {
                identity,
                recipient,
            },
            sealed,
        ))
    }

    /// The identity `key_age` holds, opened with `passphrase`.
    ///
    /// # Errors
    ///
    /// [`Why::WrongPassphrase`] when it does not open it, [`Why::Corrupt`]
    /// when what opened is not an identity or the file is not `age`.
    pub fn open(key_age: &[u8], passphrase: &SecretString) -> Result<Self, Refusal> {
        let decryptor =
            age::Decryptor::new_buffered(key_age).map_err(|_| Refusal::of(Why::Corrupt))?;
        let scrypt = age::scrypt::Identity::new(passphrase.clone());
        let mut reader = decryptor
            .decrypt(std::iter::once(&scrypt as &dyn age::Identity))
            .map_err(|e| match e {
                age::DecryptError::DecryptionFailed
                | age::DecryptError::KeyDecryptionFailed
                | age::DecryptError::NoMatchingKeys => Refusal::of(Why::WrongPassphrase),
                _ => Refusal::of(Why::Corrupt),
            })?;
        let mut text = String::new();
        reader
            .read_to_string(&mut text)
            .map_err(|_| Refusal::of(Why::Corrupt))?;
        let line = text
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("AGE-SECRET-KEY-"))
            .ok_or(Refusal::of(Why::Corrupt))?;
        let identity =
            age::x25519::Identity::from_str(line).map_err(|_| Refusal::of(Why::Corrupt))?;
        wipe(&mut text);
        let recipient = identity.to_public();
        Ok(Self {
            identity,
            recipient,
        })
    }

    /// `key.age` again, the same identity encrypted to a new passphrase.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when `age` could not encrypt.
    pub fn rewrap(&self, passphrase: &SecretString, work: Option<u8>) -> Result<Vec<u8>, Refusal> {
        seal_key(&self.identity, passphrase, work)
    }

    /// Encrypts everything `from` gives into `to`.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when a read or a write failed.
    pub fn seal(&self, from: &mut dyn Read, to: &mut dyn Write) -> Result<(), Refusal> {
        let encryptor = age::Encryptor::with_recipients(std::iter::once(
            &self.recipient as &dyn age::Recipient,
        ))
        .map_err(|_| Refusal::of(Why::Failed))?;
        let mut writer = encryptor
            .wrap_output(to)
            .map_err(|_| Refusal::of(Why::Failed))?;
        std::io::copy(from, &mut writer).map_err(|_| Refusal::of(Why::Failed))?;
        writer.finish().map_err(|_| Refusal::of(Why::Failed))?;
        Ok(())
    }

    /// The same, for bytes in hand.
    ///
    /// # Errors
    ///
    /// As [`Keyring::seal`].
    pub fn seal_bytes(&self, plain: &[u8]) -> Result<Vec<u8>, Refusal> {
        let mut out = Vec::with_capacity(plain.len() + 256);
        self.seal(&mut &plain[..], &mut out)?;
        Ok(out)
    }

    /// Decrypts everything `from` gives into `to`.
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] when it is not `age` for this identity, or a read or
    /// a write failed half way.
    pub fn open_into(&self, from: &mut dyn Read, to: &mut dyn Write) -> Result<(), Refusal> {
        let decryptor = age::Decryptor::new(from).map_err(|_| Refusal::of(Why::Corrupt))?;
        let mut reader = decryptor
            .decrypt(std::iter::once(&self.identity as &dyn age::Identity))
            .map_err(|_| Refusal::of(Why::Corrupt))?;
        std::io::copy(&mut reader, to).map_err(|_| Refusal::of(Why::Corrupt))?;
        Ok(())
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
}

/// An identity encrypted to a passphrase: `key.age`.
fn seal_key(
    identity: &age::x25519::Identity,
    passphrase: &SecretString,
    work: Option<u8>,
) -> Result<Vec<u8>, Refusal> {
    let mut recipient = age::scrypt::Recipient::new(passphrase.clone());
    if let Some(log_n) = work {
        recipient.set_work_factor(log_n);
    }
    let encryptor =
        age::Encryptor::with_recipients(std::iter::once(&recipient as &dyn age::Recipient))
            .map_err(|_| Refusal::of(Why::Failed))?;
    let mut out = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut out)
        .map_err(|_| Refusal::of(Why::Failed))?;
    let secret = identity.to_string();
    writer
        .write_all(secret.expose_secret().as_bytes())
        .and_then(|()| writer.write_all(b"\n"))
        .map_err(|_| Refusal::of(Why::Failed))?;
    writer.finish().map_err(|_| Refusal::of(Why::Failed))?;
    Ok(out)
}

/// Overwrites a string's bytes before it is freed.
fn wipe(text: &mut String) {
    let len = text.len();
    text.clear();
    text.extend(std::iter::repeat_n('\0', len));
    text.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small scrypt, so the tests do not pay a second each.
    const WORK: Option<u8> = Some(10);

    #[test]
    fn the_key_opens_only_with_its_passphrase_and_the_blobs_only_with_the_key() {
        let right = passphrase("correct horse battery".to_owned());
        let (ring, key_age) = Keyring::new(&right, WORK).unwrap();
        let reopened = Keyring::open(&key_age, &right).unwrap();
        let sealed = ring.seal_bytes(b"{\"theme\":\"owl\"}").unwrap();
        assert!(
            !sealed.windows(5).any(|w| w == b"theme"),
            "the plain text is in the blob"
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
        // ANOTHER identity cannot read the blob.
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
        // AND THE IDENTITY NEVER REACHES A LOG.
        assert!(!format!("{again:?}").contains("AGE-SECRET-KEY"));
    }
}
