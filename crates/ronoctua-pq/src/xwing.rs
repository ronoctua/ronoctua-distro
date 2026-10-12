//! X-Wing — ML-KEM-768 and X25519 in one KEM — as the HPKE KEM `0x647a`
//! (`MLKEM768-X25519`), which is what age's `mlkem768x25519` recipient seals
//! the file key with.
//!
//! # Where every constant comes from
//!
//! The C2SP age spec names the KEM "MLKEM768-X25519 from draft-ietf-hpke-pq-03
//! / filippo.io/hpke-pq", and draft-irtf-cfrg-concrete-hybrid-kems-02 §4.2
//! says that KEM "is identical to the X-Wing construction"
//! (draft-connolly-cfrg-xwing-kem-11). Written from those texts:
//!
//! ```text
//! expand(seed)      SHAKE256(seed, 96) = d ‖ z ‖ sk_X
//!                   (ek_M, dk_M) = ML-KEM-768.KeyGen_internal(d, z)
//!                   pk_X = X25519(sk_X, 9)            pk = ek_M ‖ pk_X  (1216)
//! encaps(pk, r)     r = m ‖ sk_E (64): (ss_M, ct_M) = ML-KEM-768.Encaps_internal(ek_M, m)
//!                   ct_X = X25519(sk_E, 9), ss_X = X25519(sk_E, pk_X)
//!                   ct = ct_M ‖ ct_X  (1120)
//! decaps(seed, ct)  ss_M = ML-KEM-768.Decaps(dk_M, ct_M), ss_X = X25519(sk_X, ct_X)
//! combiner          ss = SHA3-256(ss_M ‖ ss_X ‖ ct_X ‖ pk_X ‖ "\.//^\")
//! DeriveKeyPair     seed = SHAKE256.LabeledDerive(ikm, "DeriveKeyPair", "", 32)
//! ```
//!
//! and judged by the vectors in [`crate::vectors`] and by the Go `age` CLI
//! (`pq-age-interop`).
//!
//! # What is refused, as Go refuses it
//!
//! - An ML-KEM encapsulation key that fails FIPS 203's modulus check (a
//!   coefficient ≥ q): Go's `mlkem.NewEncapsulationKey768` refuses to parse it.
//! - An X25519 share of low order (the Diffie–Hellman comes out all zero):
//!   Go's `crypto/ecdh` refuses the exchange.
//!
//! # Secrets
//!
//! The private key is the 32-byte seed and nothing else, wiped on drop. Every
//! use expands it again into ML-KEM and X25519 keys that wipe themselves on
//! drop (ml-kem's `zeroize`, x25519-dalek's default `zeroize`); the expansion
//! costs tens of microseconds, and a header holds a handful of stanzas.

use hpke::generic_array::GenericArray;
use hpke::generic_array::typenum::{self, U32, U96, U192, U1024};
use hpke::kem::SharedSecret;
use hpke::rand_core::{CryptoRng, RngCore};
use hpke::{Deserializable, HpkeError, Serializable};
use ml_kem::kem::Decapsulate as _;
use ml_kem::{B32, EncapsulateDeterministic as _, EncodedSizeUser as _, KemCore, MlKem768};
use sha3::digest::{ExtendableOutput as _, Update as _, XofReader as _};
use sha3::{Digest as _, Sha3_256, Shake256};
use subtle::ConstantTimeEq as _;
use x25519_dalek::{PublicKey as XPublic, StaticSecret};
use zeroize::Zeroize as _;

/// The HPKE identifier of `MLKEM768-X25519` (draft-ietf-hpke-pq-03, table 2).
pub(crate) const KEM_ID: u16 = 0x647a;

/// The private key: a seed (`Nsk`).
pub(crate) const SEED_BYTES: usize = 32;

/// The public key, `ek_M ‖ pk_X` (`Npk`).
pub(crate) const PUBLIC_BYTES: usize = 1216;

/// The encapsulated key, `ct_M ‖ ct_X` (`Nenc`).
pub(crate) const ENC_BYTES: usize = 1120;

/// What one encapsulation draws: ML-KEM's `m`, then the X25519 ephemeral
/// (`Nrandom`), in that order — the order `EncapsDerand` reads them in.
pub(crate) const RANDOM_BYTES: usize = 64;

/// The shared secret (`Nsecret`).
pub(crate) const SECRET_BYTES: usize = 32;

/// ML-KEM-768's encapsulation key and ciphertext.
const ML_KEM_EK_BYTES: usize = 1184;
const ML_KEM_CT_BYTES: usize = 1088;

/// The part of ML-KEM-768's encapsulation key that holds 12-bit coefficients
/// (`384·k`, k = 3); the 32 bytes after it are the seed ρ.
const ML_KEM_T_BYTES: usize = 1152;

/// ML-KEM's modulus.
const ML_KEM_Q: u16 = 3329;

/// X-Wing's label, `\.//^\` (hex `5c2e2f2f5e5c`), LAST in the combiner.
const LABEL: &[u8; 6] = b"\\.//^\\";

/// A seed expanded into the keys it stands for.
pub(crate) struct Expanded {
    /// ML-KEM's decapsulation key (wiped on drop: ml-kem's `zeroize`).
    dk_m: <MlKem768 as KemCore>::DecapsulationKey,
    /// X25519's secret (wiped on drop: x25519-dalek's `zeroize`).
    sk_x: StaticSecret,
    /// `ek_M ‖ pk_X`.
    pub(crate) public: [u8; PUBLIC_BYTES],
}

/// `expandKey(seed)`: SHAKE256 to 96 bytes, ML-KEM's `KeyGen_internal(d, z)`
/// on the first 64, X25519's scalar is the last 32.
pub(crate) fn expand(seed: &[u8; SEED_BYTES]) -> Expanded {
    let mut wide = [0_u8; 96];
    let mut xof = Shake256::default();
    xof.update(seed);
    xof.finalize_xof().read(&mut wide);

    let mut d = B32::try_from(&wide[0..32]).expect("32 bytes");
    let mut z = B32::try_from(&wide[32..64]).expect("32 bytes");
    let (dk_m, ek_m) = MlKem768::generate_deterministic(&d, &z);
    let mut scalar = [0_u8; 32];
    scalar.copy_from_slice(&wide[64..96]);
    let sk_x = StaticSecret::from(scalar);
    scalar.zeroize();
    d.zeroize();
    z.zeroize();
    wide.zeroize();

    let mut public = [0_u8; PUBLIC_BYTES];
    public[..ML_KEM_EK_BYTES].copy_from_slice(&ek_m.as_bytes());
    public[ML_KEM_EK_BYTES..].copy_from_slice(XPublic::from(&sk_x).as_bytes());
    Expanded { dk_m, sk_x, public }
}

/// `SHA3-256(ss_M ‖ ss_X ‖ ct_X ‖ pk_X ‖ label)`.
fn combine(ss_m: &[u8], ss_x: &[u8], ct_x: &[u8], pk_x: &[u8]) -> [u8; SECRET_BYTES] {
    let mut h = Sha3_256::new();
    sha3::Digest::update(&mut h, ss_m);
    sha3::Digest::update(&mut h, ss_x);
    sha3::Digest::update(&mut h, ct_x);
    sha3::Digest::update(&mut h, pk_x);
    sha3::Digest::update(&mut h, LABEL);
    h.finalize().into()
}

/// The X25519 half of a public key or an encapsulation, as a point.
fn x_point(bytes: &[u8]) -> XPublic {
    let mut point = [0_u8; 32];
    point.copy_from_slice(&bytes[..32]);
    XPublic::from(point)
}

/// Whether a public key is one Go's `age` would also seal to: ML-KEM's
/// encapsulation key passes FIPS 203's modulus check (every 12-bit
/// coefficient below q) and the X25519 share is not of low order.
pub(crate) fn public_is_valid(public: &[u8; PUBLIC_BYTES]) -> bool {
    let canonical = public[..ML_KEM_T_BYTES].chunks_exact(3).all(|c| {
        let low = u16::from(c[0]) | (u16::from(c[1] & 0x0f) << 8);
        let high = (u16::from(c[1]) >> 4) | (u16::from(c[2]) << 4);
        low < ML_KEM_Q && high < ML_KEM_Q
    });
    // A clamped scalar is a multiple of eight, so every point of small order
    // — on the curve or on its twist — comes out all zero.
    let probe = StaticSecret::from([0x5a_u8; 32]);
    canonical
        && probe
            .diffie_hellman(&x_point(&public[ML_KEM_EK_BYTES..]))
            .was_contributory()
}

/// `EncapsDerand(pk, randomness)`: `None` for a public key that fails
/// [`public_is_valid`]'s X25519 half (an all-zero exchange).
pub(crate) fn encaps_derand(
    public: &[u8; PUBLIC_BYTES],
    randomness: &[u8; RANDOM_BYTES],
) -> Option<([u8; SECRET_BYTES], [u8; ENC_BYTES])> {
    let encoded = ml_kem::Encoded::<<MlKem768 as KemCore>::EncapsulationKey>::try_from(
        &public[..ML_KEM_EK_BYTES],
    )
    .ok()?;
    let ek_m = <MlKem768 as KemCore>::EncapsulationKey::from_bytes(&encoded);
    let mut m = B32::try_from(&randomness[..32]).ok()?;
    let encapsulated = ek_m.encapsulate_deterministic(&m);
    m.zeroize();
    let (ct_m, mut ss_m) = encapsulated.ok()?;

    let mut ephemeral = [0_u8; 32];
    ephemeral.copy_from_slice(&randomness[32..]);
    let sk_e = StaticSecret::from(ephemeral);
    ephemeral.zeroize();
    let ct_x = XPublic::from(&sk_e);
    let pk_x = x_point(&public[ML_KEM_EK_BYTES..]);
    let ss_x = sk_e.diffie_hellman(&pk_x);
    if !ss_x.was_contributory() {
        ss_m.zeroize();
        return None;
    }
    let ss = combine(&ss_m, ss_x.as_bytes(), ct_x.as_bytes(), pk_x.as_bytes());
    ss_m.zeroize();

    let mut ct = [0_u8; ENC_BYTES];
    ct[..ML_KEM_CT_BYTES].copy_from_slice(&ct_m);
    ct[ML_KEM_CT_BYTES..].copy_from_slice(ct_x.as_bytes());
    Some((ss, ct))
}

/// `Decaps(seed, ct)`: `None` for an X25519 share of low order. ML-KEM
/// itself never fails (implicit rejection): a stranger's ciphertext gives a
/// stranger's secret, and the AEAD after it says no.
pub(crate) fn decaps(seed: &[u8; SEED_BYTES], ct: &[u8; ENC_BYTES]) -> Option<[u8; SECRET_BYTES]> {
    let keys = expand(seed);
    let ct_m = ml_kem::Ciphertext::<MlKem768>::try_from(&ct[..ML_KEM_CT_BYTES]).ok()?;
    let mut ss_m = keys.dk_m.decapsulate(&ct_m).ok()?;
    let ct_x = x_point(&ct[ML_KEM_CT_BYTES..]);
    let ss_x = keys.sk_x.diffie_hellman(&ct_x);
    if !ss_x.was_contributory() {
        ss_m.zeroize();
        return None;
    }
    let ss = combine(
        &ss_m,
        ss_x.as_bytes(),
        ct_x.as_bytes(),
        &keys.public[ML_KEM_EK_BYTES..],
    );
    ss_m.zeroize();
    Some(ss)
}

/// `SHAKE256.LabeledDerive(ikm, label, context, 32)` with this KEM's suite.
fn labeled_derive(ikm: &[u8], label: &[u8], context: &[u8]) -> [u8; SEED_BYTES] {
    let mut out = [0_u8; SEED_BYTES];
    let mut xof = Shake256::default();
    xof.update(ikm);
    xof.update(b"HPKE-v1");
    xof.update(b"KEM");
    xof.update(&KEM_ID.to_be_bytes());
    xof.update(
        &u16::try_from(label.len())
            .expect("a short label")
            .to_be_bytes(),
    );
    xof.update(label);
    xof.update(
        &u16::try_from(SEED_BYTES)
            .expect("a short output")
            .to_be_bytes(),
    );
    xof.update(context);
    xof.finalize_xof_into(&mut out);
    out
}

/// The private key: the seed, compared in constant time, wiped on drop.
#[derive(Clone)]
pub(crate) struct PrivateKey(pub(crate) [u8; SEED_BYTES]);

impl Drop for PrivateKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl PartialEq for PrivateKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.ct_eq(&other.0).into()
    }
}

impl Eq for PrivateKey {}

impl Serializable for PrivateKey {
    type OutputSize = U32;

    fn write_exact(&self, buf: &mut [u8]) {
        buf.copy_from_slice(&self.0);
    }
}

impl Deserializable for PrivateKey {
    fn from_bytes(encoded: &[u8]) -> Result<Self, HpkeError> {
        <[u8; SEED_BYTES]>::try_from(encoded)
            .map(Self)
            .map_err(|_| HpkeError::IncorrectInputLength(SEED_BYTES, encoded.len()))
    }
}

/// The public key, `ek_M ‖ pk_X`, valid by construction
/// ([`public_is_valid`] is asked when it is read).
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct PublicKey(pub(crate) Box<[u8; PUBLIC_BYTES]>);

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PublicKey(..)")
    }
}

impl Serializable for PublicKey {
    type OutputSize = typenum::Sum<U1024, U192>;

    fn write_exact(&self, buf: &mut [u8]) {
        buf.copy_from_slice(&self.0[..]);
    }
}

impl Deserializable for PublicKey {
    fn from_bytes(encoded: &[u8]) -> Result<Self, HpkeError> {
        let bytes = <[u8; PUBLIC_BYTES]>::try_from(encoded)
            .map_err(|_| HpkeError::IncorrectInputLength(PUBLIC_BYTES, encoded.len()))?;
        if public_is_valid(&bytes) {
            Ok(Self(Box::new(bytes)))
        } else {
            Err(HpkeError::ValidationError)
        }
    }
}

/// The encapsulated key, `ct_M ‖ ct_X`.
#[derive(Clone)]
pub(crate) struct EncappedKey(pub(crate) Box<[u8; ENC_BYTES]>);

impl Serializable for EncappedKey {
    type OutputSize = typenum::Sum<U1024, U96>;

    fn write_exact(&self, buf: &mut [u8]) {
        buf.copy_from_slice(&self.0[..]);
    }
}

impl Deserializable for EncappedKey {
    fn from_bytes(encoded: &[u8]) -> Result<Self, HpkeError> {
        <[u8; ENC_BYTES]>::try_from(encoded)
            .map(|bytes| Self(Box::new(bytes)))
            .map_err(|_| HpkeError::IncorrectInputLength(ENC_BYTES, encoded.len()))
    }
}

/// `MLKEM768-X25519` for the `hpke` crate: base mode only, as age uses it.
pub(crate) struct XWing;

impl hpke::Kem for XWing {
    type PublicKey = PublicKey;
    type PrivateKey = PrivateKey;
    type EncappedKey = EncappedKey;
    type NSecret = U32;
    const KEM_ID: u16 = KEM_ID;

    fn sk_to_pk(sk: &PrivateKey) -> PublicKey {
        PublicKey(Box::new(expand(&sk.0).public))
    }

    fn derive_keypair(ikm: &[u8]) -> (PrivateKey, PublicKey) {
        let sk = PrivateKey(labeled_derive(ikm, b"DeriveKeyPair", b""));
        let pk = Self::sk_to_pk(&sk);
        (sk, pk)
    }

    fn encap<R: CryptoRng + RngCore>(
        pk_recip: &PublicKey,
        sender_id_keypair: Option<(&PrivateKey, &PublicKey)>,
        csprng: &mut R,
    ) -> Result<(SharedSecret<Self>, EncappedKey), HpkeError> {
        // NO AUTHENTICATED MODE: a KEM, not a Diffie–Hellman, has none.
        if sender_id_keypair.is_some() {
            return Err(HpkeError::EncapError);
        }
        let mut randomness = [0_u8; RANDOM_BYTES];
        let drawn = csprng.try_fill_bytes(&mut randomness);
        let sealed = drawn
            .ok()
            .and_then(|()| encaps_derand(&pk_recip.0, &randomness));
        randomness.zeroize();
        let (mut ss, ct) = sealed.ok_or(HpkeError::EncapError)?;
        let shared = SharedSecret(GenericArray::clone_from_slice(&ss));
        ss.zeroize();
        Ok((shared, EncappedKey(Box::new(ct))))
    }

    fn decap(
        sk_recip: &PrivateKey,
        pk_sender_id: Option<&PublicKey>,
        encapped_key: &EncappedKey,
    ) -> Result<SharedSecret<Self>, HpkeError> {
        if pk_sender_id.is_some() {
            return Err(HpkeError::DecapError);
        }
        let mut ss = decaps(&sk_recip.0, &encapped_key.0).ok_or(HpkeError::DecapError)?;
        let shared = SharedSecret(GenericArray::clone_from_slice(&ss));
        ss.zeroize();
        Ok(shared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_encapsulation_opens_with_its_seed_and_not_another() {
        let seed = [7_u8; SEED_BYTES];
        let keys = expand(&seed);
        assert!(public_is_valid(&keys.public));
        let (ss, ct) = encaps_derand(&keys.public, &[3_u8; RANDOM_BYTES]).unwrap();
        assert_eq!(decaps(&seed, &ct), Some(ss));
        // Implicit rejection: another seed gets ANOTHER secret, not an error.
        let other = decaps(&[8_u8; SEED_BYTES], &ct).unwrap();
        assert_ne!(other, ss);
    }

    #[test]
    fn what_go_refuses_is_refused() {
        let mut public = expand(&[1_u8; SEED_BYTES]).public;
        // A coefficient of q (0xd01) in the first slot: not canonical.
        public[0] = 0x01;
        public[1] = (public[1] & 0xf0) | 0x0d;
        assert!(!public_is_valid(&public));

        // The X25519 half at a point of small order: u = 0 (order two) and
        // u = 1 (order four)…
        let mut public = expand(&[1_u8; SEED_BYTES]).public;
        public[ML_KEM_EK_BYTES..].fill(0);
        assert!(!public_is_valid(&public));
        assert_eq!(encaps_derand(&public, &[3_u8; RANDOM_BYTES]), None);
        public[ML_KEM_EK_BYTES] = 1;
        assert!(!public_is_valid(&public));

        // …and an encapsulation whose X25519 share is of low order.
        let seed = [2_u8; SEED_BYTES];
        let (_, mut ct) = encaps_derand(&expand(&seed).public, &[4_u8; RANDOM_BYTES]).unwrap();
        ct[ML_KEM_CT_BYTES..].fill(0);
        assert_eq!(decaps(&seed, &ct), None);
    }

    #[test]
    fn the_label_is_the_six_bytes_of_the_draft() {
        assert_eq!(LABEL, &[0x5c, 0x2e, 0x2f, 0x2f, 0x5e, 0x5c]);
        assert_eq!(<PublicKey as Serializable>::size(), PUBLIC_BYTES);
        assert_eq!(<EncappedKey as Serializable>::size(), ENC_BYTES);
    }
}
