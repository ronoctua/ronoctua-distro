//! ClipCascade's cryptography, and the vector that proves it matches.
//!
//! # What the upstream client does, measured rather than guessed
//!
//! From `utils/cipher_manager.py` of the 3.2.0 this system pins:
//!
//! ```text
//! key = pbkdf2_hmac("sha256",
//!                   password,
//!                   salt = username + password + salt,
//!                   iterations = hash_rounds,   # 664937 by default
//!                   dklen = 32)
//! AES.new(key, AES.MODE_GCM)  ->  {nonce, ciphertext, tag}, each base64
//! ```
//!
//! The salt is the username, the password and the server's salt CONCATENATED,
//! in that order. It is unusual and it is not ours to change: a client that
//! derived a different key would simply fail to decrypt everything, and the
//! only symptom would be a clipboard that stays empty.
//!
//! # The nonce is sixteen bytes, and it is why the provider is not the cipher here
//!
//! `AES.new(key, AES.MODE_GCM)` with no nonce takes pycryptodome's default,
//! which is 16 bytes — measured on the installed 3.2.0, five times, all 16.
//! `aws_lc_rs::aead::NONCE_LEN` is 12 (as `ring`'s was) and there is no other
//! constructor.
//!
//! GCM with a nonce that is not 96 bits does not simply pad: it derives its
//! first counter block through GHASH instead of by concatenation. So the
//! provider would compute a different J0, every message would fail to authenticate, and
//! nothing on either side would say why. `aes-gcm` is generic over the length
//! and does the derivation, which is the whole reason it is in the manifest.
//!
//! # What is NOT here
//!
//! The password, ever, in any form that outlives the call. What is kept is the
//! derived key, in `$XDG_STATE_HOME` at `0600` — see `crate::prefs`. Nothing
//! about either is in this repository (invariant 13).

use aes_gcm::aead::{Aead, KeyInit, Payload};
use anyhow::{Context as _, Result, bail};
use base64::Engine as _;

/// AES-256-GCM with a sixteen-byte nonce — ClipCascade's shape.
type Cipher = aes_gcm::AesGcm<aes_gcm::aes::Aes256, aes_gcm::aead::consts::U16>;

/// How long the derived key is. AES-256.
pub const KEY_LEN: usize = 32;

/// How long a nonce is, here.
pub const NONCE_LEN: usize = 16;

/// The default number of PBKDF2 rounds, when the server has not said.
///
/// Upstream's own default, and it travels in the configuration because the
/// SERVER decides it: a client that assumed this number against a server that
/// had changed it would derive the wrong key and decrypt nothing.
pub const ROUNDS: u32 = 664_937;

/// The derived key. Never the password.
#[derive(Clone, PartialEq, Eq)]
pub struct Key([u8; KEY_LEN]);

impl std::fmt::Debug for Key {
    /// It prints as a shape and never as bytes.
    ///
    /// A key in a log is a key on disk, and `tracing` writes to the journal.
    /// This type exists partly so that no `{:?}` anywhere can spill it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(<32 bytes>)")
    }
}

impl Key {
    /// The key, from what the person typed.
    ///
    /// **ALL FOUR ARE THE PERSON'S**, and the server is asked for none of
    /// them: measured against the upstream (3.2.0), `salt` and `rounds` are
    /// typed at login — `core/config.py` defaults them to `""` and
    /// [`ROUNDS`] — and every device on one installation has to be given the
    /// same two by hand. There is no endpoint that serves them; `/max-size`
    /// answers `{"maxsize": n}` and nothing else.
    ///
    /// The PBKDF2 salt is `username + password + salt`, in that order — see
    /// this module's header for why it is not ours to tidy.
    #[must_use]
    pub fn derive(username: &str, password: &str, salt: &str, rounds: u32) -> Self {
        let mut out = [0u8; KEY_LEN];
        let rounds = std::num::NonZeroU32::new(rounds)
            .unwrap_or_else(|| std::num::NonZeroU32::new(ROUNDS).expect("a positive default"));
        let material = format!("{username}{password}{salt}");
        aws_lc_rs::pbkdf2::derive(
            aws_lc_rs::pbkdf2::PBKDF2_HMAC_SHA256,
            rounds,
            material.as_bytes(),
            password.as_bytes(),
            &mut out,
        );
        Self(out)
    }

    /// A key that was already derived and stored.
    #[must_use]
    pub const fn of_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// The bytes, for writing to the `0600` file that keeps them.
    ///
    /// `pub(crate)` on purpose: the one caller is `crate::prefs`, and a public
    /// reader is one careless row away from the screen. The Syncthing crate
    /// has a check that says the same thing about its API key.
    #[must_use]
    pub(crate) const fn bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// Encrypts, with a fresh nonce.
    ///
    /// # Errors
    ///
    /// The system refused to give random bytes, or the cipher refused.
    pub fn seal(&self, plaintext: &str) -> Result<Sealed> {
        let mut nonce = [0u8; NONCE_LEN];
        aws_lc_rs::rand::SecureRandom::fill(&aws_lc_rs::rand::SystemRandom::new(), &mut nonce)
            .map_err(|_| anyhow::anyhow!("the system refused to give random bytes"))?;
        let cipher = Cipher::new_from_slice(&self.0)
            // `InvalidLength` IS NOT `std::error::Error`, so `context` cannot see
            // it. The sentence is ours anyway: a key of the wrong length here is
            // a programming error, not something a person can act on.
            .map_err(|_| anyhow::anyhow!("the key is not 32 bytes"))?;
        // A FRESH NONCE PER MESSAGE, and never a counter: two messages under
        // one nonce is how GCM stops being secret, and a counter that reset
        // with the process would do it at every restart.
        let joined = cipher
            .encrypt(
                nonce.as_slice().into(),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: &[],
                },
            )
            .map_err(|_| anyhow::anyhow!("the clipboard could not be encrypted"))?;
        // `aes-gcm` HANDS BACK CIPHERTEXT AND TAG AS ONE RUN, and ClipCascade
        // wants them as two fields. The tag is the last sixteen bytes; a split
        // done the other way round would send a message that never verifies.
        let at = joined
            .len()
            .checked_sub(16)
            .context("the cipher produced nothing to split")?;
        Ok(Sealed {
            nonce: nonce.to_vec(),
            ciphertext: joined[..at].to_vec(),
            tag: joined[at..].to_vec(),
        })
    }

    /// Opens what the server sent.
    ///
    /// # Errors
    ///
    /// The nonce or the tag is the wrong length, the message does not
    /// authenticate — which is what a wrong password looks like — or what came
    /// out is not text.
    pub fn open(&self, sealed: &Sealed) -> Result<String> {
        if sealed.nonce.is_empty() {
            bail!("the message carries no nonce");
        }
        if sealed.tag.len() != 16 {
            bail!("the message's tag is {} bytes, not 16", sealed.tag.len());
        }
        let cipher = Cipher::new_from_slice(&self.0)
            // `InvalidLength` IS NOT `std::error::Error`, so `context` cannot see
            // it. The sentence is ours anyway: a key of the wrong length here is
            // a programming error, not something a person can act on.
            .map_err(|_| anyhow::anyhow!("the key is not 32 bytes"))?;
        let mut joined = sealed.ciphertext.clone();
        joined.extend_from_slice(&sealed.tag);
        let plain = cipher
            .decrypt(
                sealed.nonce.as_slice().into(),
                Payload {
                    msg: &joined,
                    aad: &[],
                },
            )
            // THE SENTENCE MATTERS. A message that will not authenticate is
            // almost always the wrong password, and "decryption failed" sends
            // a person looking at the network.
            .map_err(|_| anyhow::anyhow!("this message will not open with that password"))?;
        String::from_utf8(plain).context("what came out of the message is not text")
    }
}

/// One encrypted message, in the three fields ClipCascade puts on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    /// The nonce — sixteen bytes here, see the header.
    pub nonce: Vec<u8>,
    /// The ciphertext, without the tag.
    pub ciphertext: Vec<u8>,
    /// GCM's tag, sixteen bytes.
    pub tag: Vec<u8>,
}

impl Sealed {
    /// The JSON ClipCascade sends: three base64 strings.
    #[must_use]
    pub fn to_json(&self) -> String {
        let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
        serde_json::json!({
            "nonce": b64(&self.nonce),
            "ciphertext": b64(&self.ciphertext),
            "tag": b64(&self.tag),
        })
        .to_string()
    }

    /// The same, read back.
    ///
    /// # Errors
    ///
    /// It is not JSON, a field is missing, or a field is not base64.
    pub fn of_json(text: &str) -> Result<Self> {
        let value: serde_json::Value =
            serde_json::from_str(text).context("the message is not JSON")?;
        let field = |name: &str| -> Result<Vec<u8>> {
            let said = value
                .get(name)
                .and_then(serde_json::Value::as_str)
                .with_context(|| format!("the message has no `{name}`"))?;
            base64::engine::general_purpose::STANDARD
                .decode(said)
                .with_context(|| format!("the message's `{name}` is not base64"))
        };
        Ok(Self {
            nonce: field("nonce")?,
            ciphertext: field("ciphertext")?,
            tag: field("tag")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from the upstream Python, on the 3.2.0 this system pins, with
    /// every input fixed so it is reproducible.
    ///
    /// THE USERNAME, PASSWORD AND SALT ARE INVENTED. Nothing here came off the
    /// owner's install: the account, the server and the real salt are personal
    /// data and do not enter this repository (invariant 13). What is proved is
    /// the ALGORITHM, which is the part that has to agree.
    mod vector {
        pub const USERNAME: &str = "tester";
        pub const PASSWORD: &str = "correct horse battery staple";
        pub const SALT: &str = "a-declared-salt";
        pub const ROUNDS: u32 = 664_937;
        pub const KEY: &str = "M8SnadkG/Z+V3WJJDO6D+m08DhspI6XklvfcjVbU2hU=";
        pub const PLAINTEXT: &str = "the clipboard crosses the network";
        pub const NONCE: &str = "AAECAwQFBgcICQoLDA0ODw==";
        pub const CIPHERTEXT: &str = "2kWI3/cdgaH/ShT10HNGzC3I5qoverc6u42G7vem8DYm";
        pub const TAG: &str = "Y94/DjKxTFqqxaN6c1RTzQ==";
    }

    fn b64(s: &str) -> Vec<u8> {
        base64::engine::general_purpose::STANDARD.decode(s).unwrap()
    }

    #[test]
    fn the_key_is_the_one_the_python_derives() {
        // THE SALT IS `username + password + salt`, in that order. It is
        // unusual, and a client that tidied it would derive a different key —
        // the only symptom being a clipboard that never fills.
        let key = Key::derive(
            vector::USERNAME,
            vector::PASSWORD,
            vector::SALT,
            vector::ROUNDS,
        );
        assert_eq!(key.bytes().as_slice(), b64(vector::KEY).as_slice());
    }

    #[test]
    fn what_the_python_sealed_opens_here() {
        // THE PROOF THAT THE TWO AGREE, and the only one worth having: a
        // round trip with ourselves would pass with any algorithm at all.
        let key = Key::of_bytes(b64(vector::KEY).try_into().unwrap());
        let sealed = Sealed {
            nonce: b64(vector::NONCE),
            ciphertext: b64(vector::CIPHERTEXT),
            tag: b64(vector::TAG),
        };
        assert_eq!(key.open(&sealed).unwrap(), vector::PLAINTEXT);
    }

    #[test]
    fn the_nonce_is_sixteen_bytes_and_that_is_not_negotiable() {
        // `aws_lc_rs::aead::NONCE_LEN` IS 12. GCM with a nonce that is not 96 bits
        // derives J0 through GHASH rather than by concatenation, so a
        // twelve-byte implementation does not "almost" work — it authenticates
        // nothing, silently. The vector's own nonce is what holds this down.
        assert_eq!(NONCE_LEN, 16);
        assert_eq!(b64(vector::NONCE).len(), NONCE_LEN);
        let key = Key::of_bytes(b64(vector::KEY).try_into().unwrap());
        assert_eq!(key.seal("x").unwrap().nonce.len(), NONCE_LEN);
    }

    #[test]
    fn a_fresh_nonce_every_message() {
        // TWO MESSAGES UNDER ONE NONCE is how GCM stops being secret.
        let key = Key::of_bytes(b64(vector::KEY).try_into().unwrap());
        let one = key.seal("same text").unwrap();
        let two = key.seal("same text").unwrap();
        assert_ne!(one.nonce, two.nonce, "the nonce repeated");
        assert_ne!(one.ciphertext, two.ciphertext);
    }

    #[test]
    fn the_wrong_password_says_so_rather_than_saying_nothing() {
        let right = Key::of_bytes(b64(vector::KEY).try_into().unwrap());
        let wrong = Key::derive("tester", "not the password", "a-declared-salt", 1000);
        let sealed = right.seal("a secret").unwrap();
        let said = wrong.open(&sealed).unwrap_err().to_string();
        // "decryption failed" sends a person looking at the network.
        assert!(said.contains("password"), "{said}");
    }

    #[test]
    fn the_wire_shape_survives_a_round_trip_and_is_three_base64_fields() {
        let key = Key::of_bytes(b64(vector::KEY).try_into().unwrap());
        let sealed = key.seal("something copied").unwrap();
        let json = sealed.to_json();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        for field in ["nonce", "ciphertext", "tag"] {
            assert!(value.get(field).is_some_and(|v| v.is_string()), "{json}");
        }
        assert_eq!(Sealed::of_json(&json).unwrap(), sealed);
        assert_eq!(
            key.open(&Sealed::of_json(&json).unwrap()).unwrap(),
            "something copied"
        );
    }

    #[test]
    fn a_message_that_is_not_a_message_is_an_error_and_not_a_panic() {
        assert!(Sealed::of_json("").is_err());
        assert!(Sealed::of_json("{}").is_err());
        assert!(Sealed::of_json(r#"{"nonce":"!!","ciphertext":"","tag":""}"#).is_err());
        let key = Key::of_bytes([0u8; KEY_LEN]);
        assert!(
            key.open(&Sealed {
                nonce: Vec::new(),
                ciphertext: Vec::new(),
                tag: vec![0; 16],
            })
            .is_err()
        );
    }

    #[test]
    fn a_key_never_prints_itself() {
        // A key in a log is a key on disk, and `tracing` writes to the
        // journal. This type exists partly so no `{:?}` anywhere can spill it.
        let key = Key::of_bytes(b64(vector::KEY).try_into().unwrap());
        let said = format!("{key:?}");
        assert_eq!(said, "Key(<32 bytes>)");
        assert!(!said.contains("M8Sn"), "{said}");
    }
}
