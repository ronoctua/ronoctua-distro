//! `pin`: the person's signing key, pinned in the machine directory
//! (`local/signer.json`) — and root's reading of that pin, which every
//! `apply` holds a staging to.
//!
//! # Where a key comes from
//!
//! Never from the person's staging, and never from the repository's copy of
//! the public key: trusting the first key seen would hand the machine to
//! whoever wrote the repository first. A key reaches this program as a
//! FILE of root's (or, in the installer, of the user it names) that is
//! consumed as it is read:
//!
//! - the sudo door's helper writes the key the door was given
//!   (`ronoctua_person::root::SIGNER_PIN`) — the key the person's passphrase
//!   opened from `key.age`, on their screen — and starts
//!   `ronoctua-person-pin.service`;
//! - the installer writes the key `ronoctua-belongings` opened from
//!   `key.age` with the passphrase the person typed, and runs this itself.
//!
//! # What a pin keeps
//!
//! The key, its fingerprint, when it was pinned — and, as applies go, the
//! highest counter of the person's that one took (a lower one is an older
//! tree played again), the last refusal of a staging for its signature, and
//! what the last apply kept back. Pinning the key already pinned keeps all
//! of that; a new key starts from nothing.

use std::os::fd::OwnedFd;
use std::str::FromStr as _;

use ronoctua_person::local::Signer;
use ronoctua_person::tree::{Refused, TreeWhy};
use ronoctua_person::{Stamp, names};
use ronoctua_pq::sign::PublicKey;

use crate::verify;

/// The most a key's file may weigh: one public key and a line end.
const KEY_MOST_BYTES: u64 = 4096;

/// What a `pin` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pinned {
    /// The key is pinned now, its fingerprint.
    New(String),
    /// It already was: nothing was written.
    Again(String),
}

/// The pin of the machine directory `machine`, and the key it holds;
/// `None` when no key is pinned.
///
/// # Errors
///
/// [`TreeWhy::BadKey`] for a pin that does not read, whose key is not one,
/// or whose fingerprint is not its key's — trusting nothing is the only
/// safe reading of a pin root cannot read; [`TreeWhy::Unreadable`] when the
/// file could not be read.
pub fn read(machine: &OwnedFd) -> Result<Option<(Signer, PublicKey)>, Refused> {
    let bytes = verify::read_local(machine, names::SIGNER)
        .map_err(|e| Refused::new(TreeWhy::Unreadable, names::SIGNER, e))?;
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let signer: Signer = ronoctua_person::parse(&bytes)
        .map_err(|r| Refused::new(TreeWhy::BadKey, names::SIGNER, r.to_string()))?;
    let key = PublicKey::from_str(&signer.key)
        .map_err(|r| Refused::new(TreeWhy::BadKey, names::SIGNER, format!("the key: {r}")))?;
    if key.fingerprint() != signer.fingerprint {
        return Err(Refused::new(
            TreeWhy::BadKey,
            names::SIGNER,
            "the fingerprint is not the key's",
        ));
    }
    Ok(Some((signer, key)))
}

/// Writes `signer` as the pin of `machine`, whole.
///
/// # Errors
///
/// [`TreeWhy::Unwritable`].
pub fn write(machine: &OwnedFd, signer: &Signer) -> Result<(), Refused> {
    verify::write_local(
        machine,
        names::SIGNER,
        ronoctua_person::to_json(signer).as_bytes(),
    )
    .map_err(|e| Refused::new(TreeWhy::Unwritable, names::SIGNER, e))
}

/// The key the file at `path` holds — the file removed whatever comes of
/// it, so a key is pinned once. It must be a regular file of `owner`'s, not
/// a link, readable by nobody else, and small.
///
/// # Errors
///
/// [`TreeWhy::Unreadable`] when there is none ("the sudo door pinned
/// nothing"), [`TreeWhy::NotOwned`] for a file that is not `owner`'s
/// private one, [`TreeWhy::BadKey`] for text that is not a public key.
pub fn take_key(path: &str, owner: u32) -> Result<PublicKey, Refused> {
    use std::io::Read as _;
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
    let opened = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(i32::try_from(flags.bits()).unwrap_or(0))
        .open(path);
    let _ = std::fs::remove_file(path);
    let file = match opened {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Refused::new(
                TreeWhy::Unreadable,
                path,
                "no key was given to pin",
            ));
        }
        Err(e) => return Err(Refused::new(TreeWhy::Unreadable, path, e.to_string())),
    };
    let meta = file
        .metadata()
        .map_err(|e| Refused::new(TreeWhy::Unreadable, path, e.to_string()))?;
    // ONE NAME AT MOST — the one just removed: a second name is a file
    // somebody else may still reach.
    if !meta.is_file() || meta.uid() != owner || meta.mode() & 0o077 != 0 || meta.nlink() > 1 {
        return Err(Refused::new(
            TreeWhy::NotOwned,
            path,
            "not a private file of the owner's",
        ));
    }
    if meta.len() > KEY_MOST_BYTES {
        return Err(Refused::new(TreeWhy::TooLarge, path, "not one key"));
    }
    let mut text = String::new();
    file.take(KEY_MOST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| Refused::new(TreeWhy::Unreadable, path, e.to_string()))?;
    PublicKey::from_str(&text).map_err(|r| Refused::new(TreeWhy::BadKey, path, r.to_string()))
}

/// Pins `key` in `machine` at `at`: the pin as it was when it is that key
/// already, else a new one.
///
/// # Errors
///
/// What opening the machine directory and writing the pin refuse; a pin
/// there that does not read is replaced (the person's gesture says which key
/// is theirs).
pub fn pin(machine: &str, key: &PublicKey, at: Stamp) -> Result<Pinned, Refused> {
    let machine_fd = crate::apply::open_machine(machine)?;
    let fingerprint = key.fingerprint();
    if let Ok(Some((was, _))) = read(&machine_fd)
        && was.fingerprint == fingerprint
    {
        return Ok(Pinned::Again(fingerprint));
    }
    write(
        &machine_fd,
        &Signer::pinned(key.to_string(), fingerprint.clone(), at),
    )?;
    Ok(Pinned::New(fingerprint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Scratch;

    fn me() -> u32 {
        rustix::process::geteuid().as_raw()
    }

    fn private(s: &Scratch, rel: &str, text: &str) {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let path = s.0.join(rel);
        let _ = std::fs::remove_file(&path);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    }

    /// FAILS IF: a key is pinned from a file that is not the owner's private
    /// one, from text that is not a key, or twice from one file; or a key
    /// pinned does not read back, or pinning it again resets what applies
    /// recorded under it.
    #[test]
    fn a_key_is_pinned_once_from_its_owners_file() {
        let s = Scratch::new("pin");
        s.dir("machine");
        let key = ronoctua_pq::sign::SigningKey::generate().unwrap();
        let text = key.public().to_string();
        let at = Stamp::parse("2026-10-11T12:00:00Z").unwrap();
        let file = s.0.join("key").display().to_string();
        private(&s, "key", &text);
        let taken = take_key(&file, me()).unwrap();
        assert_eq!(&taken, key.public());
        assert!(!s.0.join("key").exists(), "the key's file was not consumed");
        assert_eq!(take_key(&file, me()).unwrap_err().why, TreeWhy::Unreadable);

        private(&s, "key", &text);
        assert_eq!(
            take_key(&file, me() + 1).unwrap_err().why,
            TreeWhy::NotOwned
        );
        s.file("key", text.as_bytes());
        std::fs::set_permissions(
            s.0.join("key"),
            std::os::unix::fs::PermissionsExt::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(take_key(&file, me()).unwrap_err().why, TreeWhy::NotOwned);
        private(&s, "key", "ronoctua-mldsa65-1c2hvcnQ");
        assert_eq!(take_key(&file, me()).unwrap_err().why, TreeWhy::BadKey);
        std::os::unix::fs::symlink(s.0.join("elsewhere"), s.0.join("key")).unwrap();
        assert_ne!(take_key(&file, me()).unwrap_err().why, TreeWhy::BadKey);

        let machine = s.0.join("machine").display().to_string();
        let fingerprint = key.public().fingerprint();
        assert_eq!(
            pin(&machine, key.public(), at.clone()).unwrap(),
            Pinned::New(fingerprint.clone())
        );
        let fd = crate::apply::open_machine(&machine).unwrap();
        let (mut signer, read_key) = read(&fd).unwrap().unwrap();
        assert_eq!(&read_key, key.public());
        assert_eq!(signer.counter, 0);
        signer.counter = 9;
        write(&fd, &signer).unwrap();
        assert_eq!(
            pin(&machine, key.public(), at.clone()).unwrap(),
            Pinned::Again(fingerprint)
        );
        assert_eq!(
            read(&fd).unwrap().unwrap().0.counter,
            9,
            "the counter was reset"
        );
        let other = ronoctua_pq::sign::SigningKey::generate().unwrap();
        assert!(matches!(
            pin(&machine, other.public(), at).unwrap(),
            Pinned::New(_)
        ));
        assert_eq!(
            read(&fd).unwrap().unwrap().0.counter,
            0,
            "a new key starts over"
        );
    }

    /// FAILS IF: a pin that does not read, or whose fingerprint is not its
    /// key's, is trusted as if no key were pinned — or as a key at all.
    #[test]
    fn a_pin_root_cannot_read_trusts_nothing() {
        let s = Scratch::new("pin-broken");
        s.file("machine/local/signer.json", b"{ not json");
        let fd = crate::apply::open_machine(s.0.join("machine").to_str().unwrap()).unwrap();
        assert_eq!(read(&fd).unwrap_err().why, TreeWhy::BadKey);
        let key = ronoctua_pq::sign::SigningKey::generate().unwrap();
        let lying = Signer::pinned(
            key.public().to_string(),
            "a".repeat(64),
            Stamp::parse("2026-10-11T12:00:00Z").unwrap(),
        );
        s.file(
            "machine/local/signer.json",
            ronoctua_person::to_json(&lying).as_bytes(),
        );
        assert_eq!(read(&fd).unwrap_err().why, TreeWhy::BadKey);
        std::fs::remove_file(s.0.join("machine/local/signer.json")).unwrap();
        assert_eq!(read(&fd).unwrap(), None);
    }

    /// FAILS IF: the pin's key prefix and the signature crate's drift apart
    /// — the sudo door would refuse every real key, or pass a shape the
    /// binary cannot read.
    #[test]
    fn the_pins_shape_is_the_keys() {
        assert_eq!(
            ronoctua_person::local::KEY_PREFIX,
            ronoctua_pq::sign::PUBLIC_KEY_PREFIX
        );
        let key = ronoctua_pq::sign::SigningKey::generate().unwrap();
        assert!(ronoctua_person::local::key_is_plain(
            &key.public().to_string()
        ));
    }
}
