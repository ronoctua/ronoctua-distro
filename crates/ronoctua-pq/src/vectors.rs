//! Known answers, from the texts that define what this crate writes — copied
//! VERBATIM by a script out of the downloaded documents, never typed:
//!
//! - X-Wing's own vectors (draft-connolly-cfrg-xwing-kem-11, Appendix C): key
//!   generation, encapsulation and decapsulation, the KEM alone;
//! - HPKE's (hpkewg/hpke-pq `test-vectors.json`, commit 11b5b95), the one of
//!   age's suite — KEM 0x647a, HKDF-SHA256, ChaCha20Poly1305: `DeriveKeyPair`,
//!   the encapsulation, then RFC 9180's key schedule through ten sealed
//!   messages and five exports, both ways;
//! - the C2SP age spec's own sample: its identity, the recipient it names, and
//!   its `mlkem768x25519` stanza, which that identity must open.
//!
//! What `ronoctua-test-pq vectors` runs, and `pq-xwing-vectors` asks of it.
//! The Go `age` CLI judges the rest (`pq-age-interop`).

use std::str::FromStr as _;

use age::secrecy::ExposeSecret as _;
use age_core::format::Stanza;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use hpke::aead::ChaCha20Poly1305;
use hpke::kdf::HkdfSha256;
use hpke::rand_core::{CryptoRng, RngCore};
use hpke::{Kem as _, OpModeR, OpModeS, Serializable as _};

use crate::xwing::{self, ENC_BYTES, PUBLIC_BYTES, RANDOM_BYTES, SEED_BYTES, XWing};
use crate::{Identity, Recipient, STANZA};

/// One known answer, and what this crate gave instead when it did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// Which answer (`xwing-11 #1 keygen`, `hpke-pq seal 3`, …).
    pub name: String,
    /// `None` when it agrees; otherwise what differs, in English.
    pub failure: Option<String>,
}

/// Every known answer, asked in order.
#[must_use]
pub fn verify() -> Vec<Verdict> {
    let mut verdicts = Vec::new();
    let mut say = |name: String, failure: Option<String>| verdicts.push(Verdict { name, failure });
    for (n, v) in XWING.iter().enumerate() {
        for (step, failure) in xwing_answers(v) {
            say(format!("xwing-11 #{} {step}", n + 1), failure);
        }
    }
    for (name, failure) in hpke_answers(&HPKE) {
        say(format!("hpke-pq {name}"), failure);
    }
    for (name, failure) in spec_answers(&AGE_SPEC) {
        say(format!("age-spec {name}"), failure);
    }
    verdicts
}

/// The bytes of a hex text; whitespace is layout.
fn unhex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .map(|b| match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => panic!("a vector holds a non-hex character: {}", b as char),
        })
        .collect();
    assert!(
        digits.len().is_multiple_of(2),
        "a vector holds an odd number of hex digits"
    );
    digits.chunks_exact(2).map(|p| p[0] << 4 | p[1]).collect()
}

/// A text without its layout.
fn joined(text: &str) -> String {
    text.split_whitespace().collect()
}

fn array<const N: usize>(text: &str) -> [u8; N] {
    let bytes = unhex(text);
    <[u8; N]>::try_from(bytes.as_slice())
        .unwrap_or_else(|_| panic!("a vector of {} bytes where {N} were due", bytes.len()))
}

/// `None` when equal; otherwise where the two first differ.
fn same(what: &str, got: &[u8], want: &[u8]) -> Option<String> {
    if got == want {
        return None;
    }
    let at = got.iter().zip(want).position(|(g, w)| g != w);
    Some(match at {
        Some(at) => format!("{what} differs at byte {at} of {}", want.len()),
        None => format!("{what} is {} bytes, {} due", got.len(), want.len()),
    })
}

/// Hands out exactly the bytes a vector says the encapsulation drew.
struct Replay<'a>(&'a [u8]);

impl RngCore for Replay<'_> {
    fn next_u32(&mut self) -> u32 {
        let mut word = [0_u8; 4];
        self.fill_bytes(&mut word);
        u32::from_le_bytes(word)
    }

    fn next_u64(&mut self) -> u64 {
        let mut word = [0_u8; 8];
        self.fill_bytes(&mut word);
        u64::from_le_bytes(word)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.try_fill_bytes(dest)
            .expect("the encapsulation drew more than the vector holds");
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), hpke::rand_core::Error> {
        if dest.len() > self.0.len() {
            let code =
                std::num::NonZeroU32::new(hpke::rand_core::Error::CUSTOM_START).expect("not zero");
            return Err(code.into());
        }
        let (taken, rest) = self.0.split_at(dest.len());
        dest.copy_from_slice(taken);
        self.0 = rest;
        Ok(())
    }
}

impl CryptoRng for Replay<'_> {}

fn xwing_answers(v: &Kem) -> Vec<(&'static str, Option<String>)> {
    let seed: [u8; SEED_BYTES] = array(v.seed);
    let pk: [u8; PUBLIC_BYTES] = array(v.pk);
    let eseed: [u8; RANDOM_BYTES] = array(v.eseed);
    let ct: [u8; ENC_BYTES] = array(v.ct);
    let ss = unhex(v.ss);
    let keygen = same("the public key", &xwing::expand(&seed).public, &pk);
    let encaps = match xwing::encaps_derand(&pk, &eseed) {
        Some((got_ss, got_ct)) => {
            same("the ciphertext", &got_ct, &ct).or_else(|| same("the secret", &got_ss, &ss))
        }
        None => Some("the encapsulation was refused".into()),
    };
    let decaps = match xwing::decaps(&seed, &ct) {
        Some(got) => same("the secret", &got, &ss),
        None => Some("the decapsulation was refused".into()),
    };
    vec![("keygen", keygen), ("encaps", encaps), ("decaps", decaps)]
}

fn hpke_answers(v: &Hpke) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let info = unhex(v.info);
    let pt = unhex(v.pt);
    let ikm_e = unhex(v.ikm_e);
    let enc = unhex(v.enc);

    let (sk, pk) = XWing::derive_keypair(&unhex(v.ikm_r));
    out.push((
        "derive".to_owned(),
        same("the private key", &sk.to_bytes(), &unhex(v.sk_r))
            .or_else(|| same("the public key", &pk.to_bytes(), &unhex(v.pk_r))),
    ));

    let encap = match XWing::encap(&pk, None, &mut Replay(&ikm_e)) {
        Ok((secret, got)) => same("enc", &got.to_bytes(), &enc)
            .or_else(|| same("the shared secret", &secret.0, &unhex(v.shared_secret))),
        Err(e) => Some(format!("the encapsulation was refused: {e}")),
    };
    out.push(("encap".to_owned(), encap));

    match hpke::setup_sender::<ChaCha20Poly1305, HkdfSha256, XWing, _>(
        &OpModeS::Base,
        &pk,
        &info,
        &mut Replay(&ikm_e),
    ) {
        Ok((got, mut ctx)) => {
            out.push(("sender enc".to_owned(), same("enc", &got.to_bytes(), &enc)));
            for (seq, (aad, ct)) in v.sealed.iter().enumerate() {
                let failure = match ctx.seal(&pt, &unhex(aad)) {
                    Ok(sealed) => same("the ciphertext", &sealed, &unhex(ct)),
                    Err(e) => Some(format!("sealing was refused: {e}")),
                };
                out.push((format!("seal {seq}"), failure));
            }
            for (n, (context, len, value)) in v.exports.iter().enumerate() {
                let mut got = vec![0_u8; *len];
                let failure = match ctx.export(&unhex(context), &mut got) {
                    Ok(()) => same("the export", &got, &unhex(value)),
                    Err(e) => Some(format!("the export was refused: {e}")),
                };
                out.push((format!("sender export {n}"), failure));
            }
        }
        Err(e) => out.push((
            "sender".to_owned(),
            Some(format!("the setup was refused: {e}")),
        )),
    }

    let encapped = <xwing::EncappedKey as hpke::Deserializable>::from_bytes(&enc);
    match encapped.and_then(|e| {
        hpke::setup_receiver::<ChaCha20Poly1305, HkdfSha256, XWing>(&OpModeR::Base, &sk, &e, &info)
    }) {
        Ok(mut ctx) => {
            for (seq, (aad, ct)) in v.sealed.iter().enumerate() {
                let failure = match ctx.open(&unhex(ct), &unhex(aad)) {
                    Ok(opened) => same("the plaintext", &opened, &pt),
                    Err(e) => Some(format!("opening was refused: {e}")),
                };
                out.push((format!("open {seq}"), failure));
            }
            for (n, (context, len, value)) in v.exports.iter().enumerate() {
                let mut got = vec![0_u8; *len];
                let failure = match ctx.export(&unhex(context), &mut got) {
                    Ok(()) => same("the export", &got, &unhex(value)),
                    Err(e) => Some(format!("the export was refused: {e}")),
                };
                out.push((format!("receiver export {n}"), failure));
            }
        }
        Err(e) => out.push((
            "receiver".to_owned(),
            Some(format!("the setup was refused: {e}")),
        )),
    }
    out
}

fn spec_answers(v: &Spec) -> Vec<(&'static str, Option<String>)> {
    let recipient = joined(v.recipient);
    let Ok(identity) = Identity::from_str(v.identity) else {
        return vec![("identity", Some("the identity was refused".into()))];
    };
    let names = same(
        "the recipient",
        identity.recipient().to_string().as_bytes(),
        recipient.as_bytes(),
    );
    let written = same(
        "the identity",
        identity.to_secret().expose_secret().as_bytes(),
        v.identity.as_bytes(),
    );
    let reads = match Recipient::from_str(&recipient) {
        Ok(r) => same(
            "the recipient",
            r.to_string().as_bytes(),
            recipient.as_bytes(),
        ),
        Err(_) => Some("the recipient was refused".into()),
    };
    let body = STANDARD_NO_PAD
        .decode(v.body)
        .expect("the spec's body is base64");
    let stanza = |body: Vec<u8>| Stanza {
        tag: STANZA.to_owned(),
        args: vec![joined(v.enc)],
        body,
    };
    let opens = match age::Identity::unwrap_stanza(&identity, &stanza(body.clone())) {
        Some(Ok(_)) => None,
        Some(Err(e)) => Some(format!("the stanza was refused: {e}")),
        None => Some("the identity did not recognise the stanza".into()),
    };
    // AND NOT VACUOUSLY: one bit of the body changed, the AEAD says no.
    let mut flipped = body;
    flipped[0] ^= 1;
    let tag_holds = age::Identity::unwrap_stanza(&identity, &stanza(flipped))
        .map(|_| "a damaged stanza was not passed over".to_owned());
    vec![
        ("identity names its recipient", names),
        ("identity is written back", written),
        ("recipient reads back", reads),
        ("stanza opens", opens),
        ("damaged stanza does not", tag_holds),
    ]
}

/// One KEM vector (hex).
pub(crate) struct Kem {
    seed: &'static str,
    pk: &'static str,
    eseed: &'static str,
    ct: &'static str,
    ss: &'static str,
}

/// One HPKE vector (hex): `sealed` is (aad, ciphertext) at sequence 0, 1, …;
/// `exports` is (context, length, value).
pub(crate) struct Hpke {
    info: &'static str,
    ikm_r: &'static str,
    ikm_e: &'static str,
    sk_r: &'static str,
    pk_r: &'static str,
    enc: &'static str,
    shared_secret: &'static str,
    pt: &'static str,
    sealed: &'static [(&'static str, &'static str)],
    exports: &'static [(&'static str, usize, &'static str)],
}

/// The age spec's sample, as written there (Bech32, base64).
pub(crate) struct Spec {
    identity: &'static str,
    recipient: &'static str,
    enc: &'static str,
    body: &'static str,
}

// ─────────────────────────────────────────────────────────────────────────
// THE DATA, copied by a script out of the documents named at the top, and
// changed only the same way: re-copied, never typed.
// ─────────────────────────────────────────────────────────────────────────

/// draft-connolly-cfrg-xwing-kem-11, Appendix C, the three vectors.
pub(crate) const XWING: [Kem; 3] = [
    Kem {
        seed: "7f9c2ba4e88f827d616045507605853ed73b8093f6efbc88eb1a6eacfa66ef26",
        pk: "
            e2236b35a8c24b39b10aa1323a96a919a2ced88400633a7b07131713fc14b2b5
            b19cfc3da5fa1a92c49f25513e0fd30d6b1611c9ab9635d7086727a4b7d21d34
            244e66969cf15b3b2a785329f61b096b277ea037383479a6b556de7231fe4b7f
            a9c9ac24c0699a0018a5253401bacfa905ca816573e56a2d2e067e9b7287533b
            a13a937dedb31fa44baced40769923610034ae31e619a170245199b3c5c39864
            859fe1b4c9717a07c30495bdfb98a0a002ccf56c1286cef5041dede3c44cf16b
            f562c7448518026b3d8b9940680abd38a1575fd27b58da063bfac32c39c30869
            374c05c1aeb1898b6b303cc68be455346ee0af699636224a148ca2aea1046311
            1c709f69b69c70ce8538746698c4c60a9aef0030c7924ceec42a5d36816f545e
            ae13293460b3acb37ea0e13d70e4aa78686da398a8397c08eaf96882113fe4f7
            bad4da40b0501e1c753efe73053c87014e8661c33099afe8bede414a5b1aa27d
            8392b3e131e9a70c1055878240cad0f40d5fe3cdf85236ead97e2a97448363b2
            808caafd516cd25052c5c362543c2517e4acd0e60ec07163009b6425fc32277a
            cee71c24bab53ed9f29e74c66a0a3564955998d76b96a9a8b50d1635a4d7a67e
            b42df5644d330457293a8042f53cc7a69288f17ed55827e82b28e82665a86a14
            fbd96645eca8172c044f83bc0d8c0b4c8626985631ca87af829068f1358963cb
            333664ca482763ba3b3bb208577f9ba6ac62c25f76592743b64be519317714cb
            4102cb7b2f9a25b2b4f0615de31decd9ca55026d6da0b65111b16fe52feed8a4
            87e144462a6dba93728f500b6ffc49e515569ef25fed17aff520507368253525
            860f58be3be61c964604a6ac814e6935596402a520a4670b3d284318866593d1
            5a4bb01c35e3e587ee0c67d2880d6f2407fb7a70712b838deb96c5d7bf2b44bc
            f6038ccbe33fbcf51a54a584fe90083c91c7a6d43d4fb15f48c60c2fd66e0a8a
            ad4ad64e5c42bb8877c0ebec2b5e387c8a988fdc23beb9e16c8757781e0a1499
            c61e138c21f216c29d076979871caa6942bafc090544bee99b54b16cb9a9a364
            d6246d9f42cce53c66b59c45c8f9ae9299a75d15180c3c952151a91b7a107724
            29dc4cbae6fcc622fa8018c63439f890630b9928db6bb7f9438ae4065ed34d73
            d486f3f52f90f0807dc88dfdd8c728e954f1ac35c06c000ce41a0582580e3bb5
            7b672972890ac5e7988e7850657116f1b57d0809aaedec0bede1ae148148311c
            6f7e317346e5189fb8cd635b986f8c0bdd27641c584b778b3a911a80be1c9692
            ab8e1bbb12839573cce19df183b45835bbb55052f9fc66a1678ef2a36dea7841
            1e6c8d60501b4e60592d13698a943b509185db912e2ea10be06171236b327c71
            716094c964a68b03377f513a05bcd99c1f346583bb052977a10a12adfc758034
            e5617da4c1276585e5774e1f3b9978b09d0e9c44d3bc86151c43aad185712717
            340223ac381d21150a04294e97bb13bbda21b5a182b6da969e19a7fd072737fa
            8e880a53c2428e3d049b7d2197405296ddb361912a7bcf4827ced611d0c7a7da
            104dde4322095339f64a61d5bb108ff0bf4d780cae509fb22c256914193ff734
            9042581237d522828824ee3bdfd07fb03f1f942d2ea179fe722f06cc03de5b69
            859edb06eff389b27dce59844570216223593d4ba32d9abac8cd049040ef6534
        ",
        eseed: "
            3cb1eea988004b93103cfb0aeefd2a686e01fa4a58e8a3639ca8a1e3f9ae57e2
            35b8cc873c23dc62b8d260169afa2f75ab916a58d974918835d25e6a435085b2
        ",
        ct: "
            b83aa828d4d62b9a83ceffe1d3d3bb1ef31264643c070c5798927e41fb07914a
            273f8f96e7826cd5375a283d7da885304c5de0516a0f0654243dc5b97f8bfeb8
            31f68251219aabdd723bc6512041acbaef8af44265524942b902e68ffd23221c
            da70b1b55d776a92d1143ea3a0c475f63ee6890157c7116dae3f62bf72f60acd
            2bb8cc31ce2ba0de364f52b8ed38c79d719715963a5dd3842d8e8b43ab704e47
            59b5327bf027c63c8fa857c4908d5a8a7b88ac7f2be394d93c3706ddd4e698cc
            6ce370101f4d0213254238b4a2e8821b6e414a1cf20f6c1244b699046f5a01ca
            a0a1a55516300b40d2048c77cc73afba79afeea9d2c0118bdf2adb8870dc328c
            5516cc45b1a2058141039e2c90a110a9e16b318dfb53bd49a126d6b73f215787
            517b8917cc01cabd107d06859854ee8b4f9861c226d3764c87339ab16c3667d2
            f49384e55456dd40414b70a6af841585f4c90c68725d57704ee8ee7ce6e2f9be
            582dbee985e038ffc346ebfb4e22158b6c84374a9ab4a44e1f91de5aac5197f8
            9bc5e5442f51f9a5937b102ba3beaebf6e1c58380a4a5fedce4a4e5026f88f52
            8f59ffd2db41752b3a3d90efabe463899b7d40870c530c8841e8712b733668ed
            033adbfafb2d49d37a44d4064e5863eb0af0a08d47b3cc888373bc05f7a33b84
            1bc2587c57eb69554e8a3767b7506917b6b70498727f16eac1a36ec8d8cfaf75
            1549f2277db277e8a55a9a5106b23a0206b4721fa9b3048552c5bd5b594d6e24
            7f38c18c591aea7f56249c72ce7b117afcc3a8621582f9cf71787e183dee0936
            7976e98409ad9217a497df888042384d7707a6b78f5f7fb8409e3b5351753734
            61b776002d799cbad62860be70573ecbe13b246e0da7e93a52168e0fb6a9756b
            895ef7f0147a0dc81bfa644b088a9228160c0f9acf1379a2941cd28c06ebc80e
            44e17aa2f8177010afd78a97ce0868d1629ebb294c5151812c583daeb8868522
            0f4da9118112e07041fcc24d5564a99fdbde28869fe0722387d7a9a4d16e1cc8
            555917e09944aa5ebaaaec2cf62693afad42a3f518fce67d273cc6c9fb5472b3
            80e8573ec7de06a3ba2fd5f931d725b493026cb0acbd3fe62d00e4c790d965d7
            a03a3c0b4222ba8c2a9a16e2ac658f572ae0e746eafc4feba023576f08942278
            a041fb82a70a595d5bacbf297ce2029898a71e5c3b0d1c6228b485b1ade509b3
            5fbca7eca97b2132e7cb6bc465375146b7dceac969308ac0c2ac89e7863eb894
            3015b24314cafb9c7c0e85fe543d56658c213632599efabfc1ec49dd8c88547b
            b2cc40c9d38cbd3099b4547840560531d0188cd1e9c23a0ebee0a03d5577d66b
            1d2bcb4baaf21cc7fef1e03806ca96299df0dfbc56e1b2b43e4fc20c37f834c4
            af62127e7dae86c3c25a2f696ac8b589dec71d595bfbe94b5ed4bc07d800b330
            796fda89edb77be0294136139354eb8cd37591578f9c600dd9be8ec6219fdd50
            7adf3397ed4d68707b8d13b24ce4cd8fb22851bfe9d632407f31ed6f7cb1600d
            e56f17576740ce2a32fc5145030145cfb97e63e0e41d354274a079d3e6fb2e15
        ",
        ss: "d2df0522128f09dd8e2c92b1e905c793d8f57a54c3da25861f10bf4ca613e384",
    },
    Kem {
        seed: "badfd6dfaac359a5efbb7bcc4b59d538df9a04302e10c8bc1cbf1a0b3a5120ea",
        pk: "
            0333285fa253661508c9fb444852caa4061636cb060e69943b431400134ae1fb
            c02287247cb38068bbb89e6714af10a3fcda6613acc4b5e4b0d6eb960c302a02
            53b1f507b596f0884d351da89b01c35543214c8e542390b2bc497967961ef102
            86879c34316e6483b644fc27e8019d73024ba1d1cc83650bb068a5431b33d122
            1b3d122dc1239010a55cb13782140893f30aca7c09380255a0c621602ffbb6a9
            db064c1406d12723ab3bbe2950a21fe521b160b30b16724cc359754b4c883426
            51333ea9412d5137791cf75558ebc5c54c520dd6c622a059f6b332ccebb9f241
            03e59a297cd69e4a48a3bfe53a5958559e840db5c023f66c10ce23081c2c8261
            d744799ba078285cfa71ac51f44708d0a6212c3993340724b3ac38f63e82a889
            a4fc581f6b8353cc6233ac8f5394b6cca292f892360570a3031c90c4da3f02a8
            95677390e60c24684a405f69ccf1a7b95312a47c844a4f9c2c4a37696dc10072
            a87bf41a2717d45b2a99ce09a4898d5a3f6b67085f9a626646bcf369982d4839
            72b9cd7d244c4f49970f766a22507925eca7df99a491d80c27723e84c7b49b63
            3a46b46785a16a41e02c538251622117364615d9c2cdaa1687a860c18bfc9ce8
            690efb2a524cb97cdfd1a4ea661fa7d08817998af838679b07c9db8455e2167a
            67c14d6a347522e89e8971270bec858364b1c1023b82c483cf8a8b76f040fe41
            c24dec2d49f6376170660605b80383391c4abad1136d874a77ef73b440758b6e
            7059add20873192e6e372e069c22c5425188e5c240cb3a6e29197ad17e87ec41
            a813af68531f262a6db25bbdb8a15d2ed9c9f35b9f2063890bd26ef09426f225
            aa1e6008d31600a29bcdf3b10d0bc72788d35e25f4976b3ca6ac7cbf0b442ae3
            99b225d9714d0638a864bda7018d3b7c793bd2ace6ac68f4284d10977cc029cf
            203c5698f15a06b162d6c8b4fd40c6af40824f9c6101bb94e9327869ab7efd83
            5dfc805367160d6c8571e3643ac70cbad5b96a1ad99352793f5af71705f95126
            cb4787392e94d808491a2245064ba5a7a30c066301392a6c315336e10dbc9c21
            77c7af382765b6c88eeab51588d01d6a95747f3652dc5b5c401a23863c7a0343
            737c737c99287a40a90896d4594730b552b910d23244684206f0eb842fb9aa31
            6ab182282a75fb72b6806cea4774b822169c386a58773c3edc8229d85905abb8
            7ac228f0f7a2ce9a497bb5325e17a6a82777a997c036c3b862d29c14682ad325
            a9600872f3913029a1588648ba590a7157809ff740b5138380015c40e9fb90f0
            311107946f28e5962e21666ad65092a3a60480cd16e61ff7fb5b44b70cf12201
            878428ef8067fceb1e1dcb49d66c773d312c7e53238cb620e126187009472d41
            036b702032411dc96cb750631df9d99452e495deb4300df660c8d35f32b424e9
            8c7ed14b12d8ab11a289ac63c50a24d52925950e49ba6bf4c2c38953c92d60b6
            cd034e575c711ac41bfa66951f62b9392828d7b45aed377ac69c35f1c6b80f38
            8f34e0bb9ce8167eb2bc630382825c396a407e905108081b444ac8a07c250737
            6a750d18248ee0a81c4318d9a38fc44c3b41e8681f87c34138442659512c4127
            6e1cc8fc4eb66e12727bcb5a9e0e405cdea21538d6ea885ab169050e6b91e1b6
            9f7ed34bcbb48fd4c562a576549f85b528c953926d96ea8a160b8843f1c89c62
        ",
        eseed: "
            17cda7cfad765f5623474d368ccca8af0007cd9f5e4c849f167a580b14aabdef
            aee7eef47cb0fca9767be1fda69419dfb927e9df07348b196691abaeb580b32d
        ",
        ct: "
            c93beb22326705699bbc3d1d0aa6339be7a405debe61a7c337e1a91453c097a6
            f77c130639d1aaeb193175f1a987aa1fd789a63c9cd487ebd6965f5d8389c8d7
            c8cfacbba4b44d2fbe0ae84de9e96fb11215d9b76acd51887b752329c1a3e046
            8ccc49392c1e0f1aad61a73c10831e60a9798cb2e7ec07596b5803db3e243ecb
            b94166feade0c9197378700f8eb65a43502bbac4605992e2de2b906ab30ba401
            d7e1ff3c98f42cfc4b30b974d3316f331461ac05f43e0db7b41d3da702a4f567
            b6ee7295199c7be92f6b4a47e7307d34278e03c872fb48647c446a64a3937dcc
            d7c6d8de4d34b9dea45a0b065ef15b9e94d1b6df6dca7174d9bc9d14c6225e3a
            78a58785c3fe4e2fe6a0706f3365389e4258fbb61ecf1a1957715982b3f18444
            24e03acd83da7eee50573f6cd3ff396841e9a00ad679da92274129da277833d0
            524674feea09a98d25b888616f338412d8e65e151e65736c8c6fb448c9260fa2
            0e7b2712148bcd3a0853865f50c1fc9e4f201aee3757120e034fd509d954b7a7
            49ff776561382c4cb64cebcbb6aa82d04cd5c2b40395ecaf231bde8334ecfd95
            5d09efa8c6e7935b1cb0298fb8b6740be4593360eed5f129d59d98822a6cea37
            c57674e919e84d6b90f695fca58e7d29092bd70f7c97c6dfb021b9f87216a627
            1d8b144a364d03b6bf084f972dc59800b14a2c008bbd0992b5b82801020978f2
            bdddb3ca3367d876cffb3548dab695a29882cae2eb5ba7c847c3c71bd0150fa9
            c33aac8e6240e0c269b8e295ddb7b77e9c17bd310be65e28c0802136d086777b
            e5652d6f1ac879d3263e9c712d1af736eac048fe848a577d6afaea1428dc71db
            8c430edd7b584ae6e6aeaf7257aff0fd8fe25c30840e30ccfa1d95118ef0f665
            7367e9070f3d97a2e9a7bae19957bd707b00e31b6b0ebb9d7df4bd22e44c0608
            30a194b5b8288353255b52954ff5905ab2b126d9aa049e44599368c27d6cb033
            eae5182c2e1504ee4e3745f51488997b8f958f0209064f6f44a7e4de5226d559
            4d1ad9b42ac59a2d100a2f190df873a2e141552f33c923b4c927e8747c6f830c
            441a8bd3c5b371f6b3ab8103ebcfb18543aefc1beb6f776bbfd5344779f4aa23
            daaf395f69ec31dc046b491f0e5cc9c651dfc306bd8f2105be7bc7a4f4e21957
            f87278c771528a8740a92e2daefa76a3525f1fae17ec4362a2700988001d8600
            11d6ca3a95f79a0205bcf634cef373a8ea273ff0f4250eb8617d0fb92102a6aa
            09cf0c3ee2cad1ad96438c8e4dfd6ee0fcc85833c3103dd6c1600cd305bc2df4
            cda89b55ca237a3f9c3f82390074ff30825fc750130ebaf13d0cf7556d2c52a9
            8a4bad39ca5d44aaadeaef775c695e64d06e966acfcd552a14e2df6c63ae541f
            0fa88fc48263089685704506a21a03856ce65d4f06d54f3157eeabd62491cb4a
            c7bf029e79f9fbd4c77e2a3588790c710e611da8b2040c76a61507a8020758dc
            c30894ad018fef98e401cc54106e20d94bd544a8f0e1fd0500342d123f618aa8
            c91bdf6e0e03200693c9651e469aee6f91c98bea4127ae66312f4ae3ea155b67
        ",
        ss: "f2e86241c64d60f6649fbc6c5b7d17180b780a3f34355e64a85749949c45f150",
    },
    Kem {
        seed: "ef58538b8d23f87732ea63b02b4fa0f4873360e2841928cd60dd4cee8cc0d4c9",
        pk: "
            36244278824f77c621c660892c1c3886a9560caa52a97c461fd3958a598e749b
            bc8c7798ac8870bac7318ac2b863000ca3b0bdcbbc1ccfcb1a30875df9a76976
            763247083e646ccb2499a4e4f0c9f4125378ba3da1999538b86f99f2328332c1
            77d1192b849413e65510128973f679d23253850bb6c347ba7ca81b5e6ac4c574
            565c731740b3cd8c9756caac39fba7ac422acc60c6c1a645b94e3b6d21485eba
            d9c4fe5bb4ea0853670c5246652bff65ce8381cb473c40c1a0cd06b54dcec118
            72b351397c0eaf995bebdb6573000cbe2496600ba76c8cb023ec260f0571e3ec
            12a9c82d9db3c57b3a99e8701f78db4fabc1cc58b1bae02745073a81fc804543
            9ba3b885581a283a1ba64e103610aabb4ddfe9959e7241011b2638b56ba6a982
            ef610c514a57212555db9a98fb6bcf0e91660ec15dfa66a67408596e9ccb9748
            9a09a073ffd1a0a7ebbe71aa5ff793cb91964160703b4b6c9c5390842c2c905d
            4a9f88111fed57874ba9b03cf611e70486edf539767c7485189d5f1b08e32a27
            4dc24a39c918fd2a4dfa946a8c897486f2c974031b2804aabc81749db430b853
            11372a3b8478868200b40e043f7bf4a1c3a08b0771b431e342ee277410bca034
            a0c77086c8f702b3aed2b4108bbd3af471633373a1ac74b128b148d1b9412aa6
            6948cac6dc6614681fda02ca86675d2a756003c49c50f06e13c63ce4bc9f321c
            860b202ee931834930011f485c9af86b9f642f0c353ad305c66996b9a136b753
            973929495f0d8048db75529edcb4935904797ac66605490f66329c3bb36b8573
            a3e00f817b3082162ff106674d11b261baae0506cde7e69fdce93c6c7b59b9d4
            c759758acf287c2e4c4bfab5170a9236daf21bdb6005e92464ee8863f845cf37
            978ef19969264a516fe992c93b5f7ae7cb6718ac69257d630379e4aac6029cb9
            06f98d91c92d118c36a6d16115d4c8f16066078badd161a65ba51e0252bc358c
            67cd2c4beab2537e42956e08a39cfccf0cd875b5499ee952c83a162c68084f6d
            35cf92f71ec66baec74ab87e2243160b64df54afb5a07f78ec0f5c5759e5a432
            2bca2643425748a1a97c62108510c44fd9089c5a7c14e57b1b77532800013027
            cff91922d7c935b4202bb507aa47598a6a5a030117210d4c49c174700550ad6f
            82ad40e965598b86bc575448eb19d70380d465c1f870824c026d74a2522a799b
            7b122d06c83aa64c0974635897261433914fdfb14106c230425a83dc8467ad82
            34f086c72a47418be9cfb582b1dcfa3d9aa45299b79fff265356d8286a1ca2f3
            c2184b2a70d15289e5b202d03b64c735a867b1154c55533ff61d6c2962770118
            48143bc85a4b823040ae025a29293ab77747d85310078682e0ba0ac236548d90
            5a79494324574d417c7a3457bd5fb5253c4876679034ae844d0d05010fec722d
            b5621e3a67a2d58e2ff33b432269169b51f9dcc095b8406dc1864cf0aeb6a213
            2661a38d641877594b3c51892b9364d25c63d637140a2018d10931b0daa5a2f2
            a405017688c991e586b522f94b1132bc7e87a63246475816c8be9c62b731691a
            b912eb656ce2619225663364701a014b7d0337212caa2ecc731f34438289e0ca
            4590a276802d980056b5d0d316cae2ecfea6d86696a9f161aa90ad47eaad8cad
            d31ae3cbc1c013747dfee80fb35b5299f555dcc2b787ea4f6f16ffdf66952461
        ",
        eseed: "
            22a96188d032675c8ac850933c7aff1533b94c834adbb69c6115bad4692d8619
            f90b0cdf8a7b9c264029ac185b70b83f2801f2f4b3f70c593ea3aeeb613a7f1b
        ",
        ct: "
            0d2e38cbf17a2e2e4e0c87a94ca1e7701ae1552e02509b3b00f9c82c39e3fd43
            5b05b91275f47abc9f1021429a26a346598cd6cd9efdc8adc1dbc35036d0290b
            f89733c835309202232f9bf652ea82f3d49280d6e8a3bd3135fb883445ab5b07
            4d949c5350c7c7d6ac59905bdbfce6639da8a9d4b390ecc1dd05522d2956f2d3
            7a05593996e5cb3fd8d5a9eb52417732e1ebf545588713b4760227115aab7ada
            178dadbca583b26cfedba2888a0c95b950bf07f750d7aa8103798aa3470a042c
            0105c6a037de2f9ebc396021b2ba2c16aba696fbac3454dc8e053b8fa55edd45
            215eeb57a1eab9106fb426b375a9b9e5c3419efc7610977e72640f9fd1b2ec33
            7de33c35e5a7581b2aae4d8ee86d2e0ebf82a1350714de50d2d788687878a196
            44ae4e3175e8d59dc90171b3badeff65aeaf600e5e5483a3595fdeb40cbafcbd
            040c29a2f6900533ae999d24f54dfcef748c30313ca447cdddfa57ad78eaa890
            e90f3f7bf8d116968a5713cc75fd0408f36364fa265c5617039304eaeac4cbee
            6fc49b9fe2276768cdbec2d73a507b543cc028dc1b154b7c2b0412254c466a94
            a8d6ea3a47e1743469bd45c08f54cf965884be3696e961741ede16e3b1bc4feb
            93faaef31d911dc0cb3fa90bcda991959a9d2cbc817a5564c5c01177a59e9577
            589ea344d60cf5b0aa39f31863febd54603ca87ad2363c766642a3f52557bcd9
            e4c05a87665842ba336b83156a677030f0bad531a8387a1486a599caa748fcea
            7bdc1eb63f3cdb97173551ab7c1c36b69acbbdb2ff7a1e7bc70439632ddc67b9
            7f3da1f59b3c1588515957cb8a2f86ab635ce0a78b7cdf24eac3445e8fc8b79b
            a04da9e903f49a7d912c197a84b4cfabc779b97d24788419bcf58035db99717e
            db9fd1c1df8c4005f700eabba528ddfcbaeda6dd30754f795948a34c9319ab65
            3524b19931c7900c4167988af52292fe902e746b524d20ceffb4339e8f5535f4
            1cf35f0f8ea8b4a7b949c5d2381116b146e9b913a83a3fa1c65ff9468c835fe4
            114554a6c66a80e1c9a6bb064b380be3c95e5595ec979bf1c85aa938938e3f10
            e72b0c87811969e8ab0d83de0b0604c4016ac3a015e19514089271bdc6ebf2ec
            56fab6018e44de749b4c36cc235e370da8466dbdc253542a2d704eb3316fd70d
            5d238cb7eaaf05966d973f62c7ef43b9a806f4ed213ac8099ea15d61a9024441
            60883f6bf441a3e1469945c9b79489ea18390f1ebc83caca10bdb8f2429877b5
            2bd44c94a228ef91c392ef5398c5c83982701318ccedab92f7a279c4fddebaa7
            fe5e986c48b7d8135b3fe4cd15be2004ce73ff86b1e55f8ecd6ba5b8114315f8
            e716ef3ab0a64564a4644651166ebd68b1f783e2e443dbccadfe189368647629
            f1a12215840b7f1d026de2f665c2eb023ff51a6df160912811ee03444ae4227f
            b941dc9ec4f31b445006fd384de5e60e0a5061b50cb1202f863090fc05eb814e
            2d42a03586c0b56f533847ac7b8184ce9690bc8dece32a88ca934f541d4cc520
            fa64de6b6e1c3c8e03db5971a445992227c825590688d203523f527161137334
        ",
        ss: "953f7f4e8c5b5049bdc771d1dffada0dd961477d1a2ae0988baa7ea6898d893f",
    },
];

/// hpkewg/hpke-pq `test-vectors.json` (commit 11b5b95), the one vector of
/// age's suite: mode 0, KEM 0x647a, KDF 1 (HKDF-SHA256), AEAD 3
/// (ChaCha20Poly1305). Its encryptions are sequence 0, 1, 2… in order.
pub(crate) const HPKE: Hpke = Hpke {
    info: "
        3466363436353230366636653230363132303437373236353633363936313665
        3230353537323665
    ",
    ikm_r: "c8575d137deab99ac98fb0873048c83c3a1f47ef5b409f609c0ca652f58c83e0",
    ikm_e: "
        c82228383c9fb887f7d8b332c28262024eda5b6b0ecd2325fe662daffc0594fa
        d4990e7c8d13812137d06ba7017453de675ab0388d418853617f3ca58cc5daf3
    ",
    sk_r: "b6bfa0299b955e85224df2e468f29eeab377ff3b96d4462b39447a22d32b91be",
    pk_r: "
        d3d102410970b8bab2984008669914490c95dea2c2ae331ca229aaf3609a6d5a
        cbd60a9ecdd71cd462c08300c474e318b554675679b6d4e1bb1a76269a7a87e6
        8335ba54593239290f35b65e5aa0d3352933339336e83260125e10ac21c97b40
        cff8385a18b2b7d62436377a13b00c27d22fea4176ff76ab80970ff0e26101c2
        45b7c33dcaa420da8ab4df41b88985c6f3845e5a5364e7d7a74ae764c175bd46
        2017cbbc5f684a7e4494391d050b83ba9ba66347774c707658c5cfe481a8c20e
        fc670314d7569a4793625abfd6579435b31886f80924c88c9a4011a0c58b6fe8
        5c5330ca27aa80ffcc58d3ac6a759897b44893bf8b520802b4d7d56bc24976ba
        32c070f0cc7fd88c19e958de0b8fe93a7669b5bb8cf80a88e6aa827924cb1659
        fe6177023611ddd30eb913ad8b2c08fc8209bb9719c61450518881ca5b12b8c7
        3ba3c27824773b5ec6571df41b277555d9aa8f566444614a8473d3b1a5e997af
        066ae1f355338302952281c82ba94f50a22e9ba55a56193d1a3f4b5365f7eb2c
        a81cb4573c33d1d9861cac8d4cf327c0393ab0c0b02572bbc3821bedb45d4025
        7e1f7643796786fcf50126eb136b35737794168a15643973639f6b6f3dd945d3
        ac6a5ab96e5d4b8fdcc54809053828610ee3a56bf4c84447147a9ca5745d5c05
        e3d9cfa659bb4c4c8f2e006826c27633a120c32151deaa2a98bc9578232b5186
        04a5186fc7ac6a1e594b09106418eb1aa1ec6b44fabeae5105d75531b9279b07
        f6277ab297a5ea77d2383154bcb9db8a08fbb10522987d63c99958a774160333
        f5f208567a311464b59ebb8cc8375a1f02470de68bad55c8e5184c8c4a0dcd45
        bfb59995f4c48ecf947896126830818aca48c0749c057b6591d381922d8bb593
        e09d4bb7b6b046179c16a1d6b4149fb1cee31b6b9a015cca526cfac8bf060c69
        9e751c9456c26d832d58926032bb78ffa44d0d024f7a417e1b452f1f779d49c8
        6e0784ab97686acb025c90f9893cc13bfc9186f0e9946ba498eb15a6684828bc
        0ca91482ce91dc4c12d0ce4b936e2556aebdc48cede6adcc066e78895086b493
        16725d9a0987ca75a7e00b0bbe909397389fead7a8ff73bf94bb4272b27b9354
        bdbf3777a978024d074a45077ed0435b8b845e4481139b39ad1276973fd1cd8b
        1cc6e27b16998a7d6295c98347998d6669b15270dba7c098f16136015bd3b1c5
        897b1312f230e49bc07476cf4deb5bf0c971d5ac37f980283940c4e52999c549
        aed09681a7f350f5a1bb1dd5a3b8837a22d506eb2029146817ec955d9dbc2939
        514ecf468c4eeb85198230effac612fb73348b42bcb9b27d9816a16596fb3618
        e71bcc321a4523a66ca248ac032c06b9e24cb084c59bc489868639ae7533f8f0
        3a4eeb56f8a3568f687f38f83fbe163673eb76275b79d7f32a9109c30eaa96a0
        f622503c0337801fd9b2007603ce73c0378b250c35b634b6c8ac2cec1fbb2456
        1a7a7812e77b4844bf9c6a299b409303c85199ca5dc6eac4e4f85ff10a464d96
        c71382760af67fdf006c4eb6b627861d5bf1978588c02825ac70206274742af0
        5a0dd136c2d465ac90b4293d05381d5911dcf48a878a6cf8d971d62c14700867
        e91dd050f85d68c32e227e5be68d3de029608179f6f83b95e5deb6263fc01c80
        641763ebe08e7add5686b0ed1d1e7053982aa616130939d0a09462df7dc74d05
    ",
    enc: "
        ab354dd589f74ee0eab7718a630cbec5df1d09058e177cd6dd141d883450ddd7
        0c050d88bed3d07cce23415cab411108cc30906482a71adcb134a56e978a6152
        a8e063b24acd1534f264f10458152a9ed4f1f32b3d480c4f2453b7fdea772014
        6b3ee92cf8a13a4840076f68c911c65fa3db5053fb0aabf79e64cd5e7aa71b2b
        9641e713ec7df552e17d5020f8721ee449b42c888e2a3f87cfd96e3a98c3e7c4
        cd8f647f899570f596bf17d2b6fa2cad19706d9cc3cf09493e1c7ffa0eb2a455
        9ae1d940fdbef97bed383e6ccfdb448d9f1a81805166b32c2af2e16878c6dc46
        ab43323ed9c136b925239782e3c329c31a5cf2a80faf025a80766e244605c27a
        fe4b624d9d8ca99b6ef5439ed1ad044b518c434385acd49f1369ded6624a2832
        a571ccdd70d08b3c04cb1cd3136166f9a485f536f69ec66f0293e840025ccaac
        42f8e5f7c9cb818076c272797047f5e50c1e9f1dab81cfb48fe4c4998b2427f0
        09702b145f34ad8dbc3e7ad4e4023057ba31cd02c4c0545ebf71eb02533e8eaa
        2b2f2690ee1407bf1f66dc5f4d836c45b82f10b720df72d237488a9af1b6dfb4
        741fd613379c2e211e77f7fae6b3734ad81de2d452005334857c4a3cbc82afc7
        428fe510495969b296d24e1a7431f557d48578cf92ae86c0392f0ba73755a9e5
        465c8e3495e4cd2a82d463244341e39414e26c9b242f31d2cf0e46b2aeb11dd5
        e56ec44834350d151344229e410faff2b2ace5c9b3fa12571db1d28da2c71334
        92781dac41b7a7e2bac2260fd12f56939033587824c9dfb17d41b3bceea53763
        193abe0c7c184d5de161ef5312f31fab42478c9a193b868e4d29b2b7624f3ebe
        740f393d03d843cd5327286a579fd2a6e37aca5b64f9316115d612c7781e704e
        a7d182701c5019975cad14fbf4ab3904d4a35acaf0be32d716a1ef5d7188fc41
        8ae9e60744325a3e8001655b756df94c24031c3ce32bd90c0ecdac52ca140fda
        d7f44d04bd0a7e2a726c54cf9793f8784a23296f65da3fd1cbc18300d503c5b2
        7be99b9b0e32d20b3614dc8a999f30c2779dd7886cfd486dc1c93ebcf517b521
        0a4359d9fa1805381f0f2261ff47de01de555d98bc1a30dda557a83007b61636
        abaf9041f96890f0f565eefc45859fbcd32d91b203215541227a4fcc3d95be2d
        db0702878caa20f2da62c4ff9fe33af591ba1ec241fbe2208e0480f8b1cca167
        9c096f8f5a02a33e9df445b3274ac112b43d51510135cd3f532a3379e90bb7f0
        cb43717e90555bb1a80924cc69577455687cceb9b1610c05839541e87ad83d79
        ef3ff24ace1934cfbe989691959d93ac48c716b672b370dd4c144ca1e3250870
        7a6ef8aa29b55759b3d054c56bee1baa6f41b84b9fc3fd681a1a1528eac57814
        1529836a29dda1501a49ba2455367256d2fe6f74ebb74ef9a49a94a4c6cd1dd0
        9810f0e9bffa69dd8c94d226d0b2977b11a35382888961004a44c60fd602e9ff
        4271287e9240ba96146515b9db9da60375aeeafeac1eeb764faebacd197df278
        17c35fe4c5c802e43349d7bc95c8b40c001449d3251c1d92ff6d5c3b08c4b27c
    ",
    shared_secret: "e059d39125d1f09a7232413a13ec5cb18a37417675442c962700d59da46d105a",
    pt: "
        3432363536313735373437393230363937333230373437323735373436383263
        3230373437323735373436383230363236353631373537343739
    ",
    sealed: &[
        (
            "436f756e742d30",
            "
            a4ab74475a498ed725f685421f67c09a4783fe76f67bd251e1e73db8eb1452df
            ad4df3c6453f7edecc7bb055dde561e2efd54d73a3d4f1f2f02eac90ba1e9b84
            ded66d43aee6393524db
        ",
        ),
        (
            "436f756e742d31",
            "
            9a38169c711afe09f0e470779b137ab236a2559faf61d55e417eaf06d48fe623
            64643e95ca68382a281c04c67bd88f311939955a14f5f8c64344e4c103e1d94f
            3084f122671fc7c0cd7e
        ",
        ),
        (
            "436f756e742d32",
            "
            0f2a23afb6d94feccf5e151cc5cac46e686325dbf34def1394d157448c420376
            cee6d3c8aa4f3862edcc16fb4c3a9ef8a7ca1054d13fd9b1cfc4a5a47198ffb3
            b97372295c894dded976
        ",
        ),
        (
            "436f756e742d33",
            "
            e5bad2306fccb51b100ce45fba5f88947d7853d66478099011de84bd2a5b9de3
            37efdf9c459da7edee76cd07db9b7cddbc9a512fa45ee8f121531660861818dd
            da7d9d2b53d3f257d36a
        ",
        ),
        (
            "436f756e742d34",
            "
            923c933cbdc5955d34bf6134f1ad5e3c36f2acc9d9fb41dd8b4e0eba2101a05a
            d5fe7a895e683176381b9baba7ced24498e87766a14fec8c256c7df2802371a5
            d29d2faf03d9b4bbcaa9
        ",
        ),
        (
            "436f756e742d35",
            "
            0dd329bc293f83ac756c3fd052c2e52a8082a0a41069764736319127fb222a64
            4f6c28155bfab1bd7763d34ab09907ead4f497c9b14d729c75c8bb6f4acfdd59
            6f39c2e9872557802821
        ",
        ),
        (
            "436f756e742d36",
            "
            ae092db4f524c459e04a2453c92b59edef2ed07d08138f3582e7cfcb735d2377
            2aa223baad29cc2dfe2e6d2935d7c593828413eaabd7eb6cb2b20d6e9fb82ab5
            21d25f80fb93190260cc
        ",
        ),
        (
            "436f756e742d37",
            "
            299434544769a352047b82f0f74cd7f832dbb47b8b3d8500476f95e07d13502e
            4b748296f19f51c4b35f4c8f80607d087f6e0632e68bad401e4153c95ceb380c
            9b6e1a50477069d68345
        ",
        ),
        (
            "436f756e742d38",
            "
            6f09ad8f28a51f7f3e34eb99ac25e9b1e4a09ca063babb7c462a7ac7f65310aa
            2d4a1eb3b629fba0606793c10a469af0b2407aac02e6073fa8b070d9aab532a8
            349f884fd97cc5041b2b
        ",
        ),
        (
            "436f756e742d39",
            "
            90eb1a4fc6e9f6b5cb142d60d905ad221135aab544c8a20484c249d5417bd2eb
            97c655ba8f15feda2adaab1a6bb5c06d0d7b17a6a6d298c9df95681d5ed131d7
            c703918397e302620c6a
        ",
        ),
    ],
    exports: &[
        (
            "70736575646f72616e646f6d30",
            32,
            "14b64c194571a8e6fc16cb4d22754c79391081174b6001cd8050142f928defbb",
        ),
        (
            "70736575646f72616e646f6d31",
            32,
            "7152f82449d4cd8137afb9b514fab6791e0938ba7734cb62228b8385003f32a6",
        ),
        (
            "70736575646f72616e646f6d32",
            32,
            "e3759bd609d81501bdac23863795a37437e804dd3274f0ed3964f82e8a6f97e5",
        ),
        (
            "70736575646f72616e646f6d33",
            32,
            "3e9e46c25af47fe02c149de2d3d8300881bb59fdb5b07c6b46d001918945cffa",
        ),
        (
            "70736575646f72616e646f6d34",
            32,
            "0dc9bd0216b18caa75d0188cab85d7fdef4b405c340da75b50e052b889fb2f78",
        ),
    ],
};

/// The C2SP age spec (`age.md`, "The MLKEM768-X25519 (i.e. X-Wing) hybrid
/// post-quantum recipient type"): its identity, the recipient it names and
/// the stanza it shows, whose file key that identity opens.
pub(crate) const AGE_SPEC: Spec = Spec {
    identity: "AGE-SECRET-KEY-PQ-1XX76JRALNLXDMEW0CRK45QMCCH4X06SE84UN3VPM33W6HWDX0H3SK3ZQFR",
    recipient: "
        age1pq1x34nzsvr0rxjsgdn8zgyhfe8j7ceq5r9rdelkjuh3y235jzxshfg87pzf
        5zrqtzdxz95paef6caq5aapdmwjjqpjfdyxnzr2zampc3uxy0dg4z2n2gm9su72p
        0pc3u0jvev55l694v78snxg3yzvcl7yda0eyytqj6a0ec477lnhcy5hzpz4zq3px
        anve4cn62gqj3pjy5lqj9c6kyj4v2z8alktn8zh99970x79gjkv7522hv9kfz35z
        snxhsx8wwtmu9cy3ftzjgwcp4sshn3llnylnpdsyz5jm72vefv4x5vfwytrefxg4
        wq3mv42wcrvkj742479zrxzpvp2p3e9fed9f0739vcu80r7ma28qfhnvlv4gfzel
        9q654dj3zmuvvz893azhxdvs9fxd0r7jzchzcfcs5mkyyjxhw0n2z6dvp9yn9qfd
        p29h0azxqyjw6v7fhyuzj7zel0uq6j9rd7wgrpz7mf5dnj43jwsgvrc8qcnhy7tu
        6dkdujuxzkp9xj43xe8h92ktre2a3u3s8mm5mrp9nr9pwkgtz4mdlq9hgn4fps4k
        57ff6wddn2fy23t47sm20r8km8sd2pcyyafnet8f0dajsrlyjeah4n3mssr6asee
        vuuskdvq5lzguyvpgwpta742c6698vgutzqgny8usfg0w2he7kq5vyxjd0f9hqg8
        xk26y9e4th0gezq92q4cpp5p2y9hf5f2cje5l0c3sa3a2qxmm38pxxvhxh99yzmf
        z0zk7r2s64nnwjhkfgfr3gf8xnmppcgmaykvh5sh6g7vk9790rf8ws0axmr2t7z8
        aae5fq2029uvcn2ghgt4fu4wgwdc0k0cz52qkvwmuzj8p8k5jgf3xzk5zmrkavje
        kjrpeq408xz3zxazwkc6tyfmhayrkfpjhwtz5mp8j8guqe43k2q6m2kte03vrw27
        y3wmqyu5etmt9dnkwcnnpmu9gz9dekfhdevf42ucshphnrk38ra6hx8w5f8q5ru0
        xdhrjxmwqf6cused7zc5xvq43r0zscjglpwlptpwydhqw64xz7ptjdyeyzpq2zkx
        tmzg29gzjpvzva4d3l0cenn9xs297wf4y4ukwrunf57xj6pm7nvrkwvtrt8hwcmg
        v8x7ajw7258ugf9wvkmk4052ekg87tw5vnx8nq2swyzv77v8yqlwsenvamr0zssk
        nwts8rrhfuwj7ykysnq9jxy0uv3kuyt22djszjdtvpz6d0s0kwh8ryynddzud92e
        meyvvyqktd0jtj7rvvg5gch25v8smlvny3kvn5gagyz475ze2y6q466xqmz2n3hs
        77lddeqyta2nch5k2u5yacuk9ywnwfdzvyejnucz724hj77hrrmakm7pr3kxsrxq
        22ejexlud9fy2kdqmkg5yncz7jm5wv2qjk5w5kvcpqsry2yqffh2la52dxfjkjq5
        rzhjzeyn6dupn0qwtyv7s4lwg3xdarsdlwe2y3tujy480y7z39q259fzx6jhd2j0
        f5hagqpcpees7hzc2yrk5cy788uk3s7qvp5cpepx24gvws3m2g433exgwppnkjsc
        ec8qu4y9z9r7vccexjcjaen42245lmgmxmuavg9alej92322gvvyy2t6267v09ch
        64y0m53jff0vjj96s0ypk60hr3jw4myd6m5hpn3xjstx7tl2szhpr5qe8jj08ydj
        c4wy2rch2fhuy3pdfjax5awe9j99ly5hkntzz9fe5zatgjvzdd0kgtxs25njnajy
        f6ssekp7gelxquusn4pt25czh3scj68kq79wdn5tgm6yvm9nzavrg043x3msnygf
        8dweknw5jmqd0uvny6ttsn09508k0c55zfnegrm9efhxpfqdkmhh6gjtqmwze9py
        yzk3tlhl53k2ykx3qheyty7saeq0d3fzv49zc0k
    ",
    enc: "
        U7E10Aon27j1oDrH7fP3B++SPROSnDVvQpBemzOM+ZcsojxJifUbeeYWejMYKNB+
        H3rlXNvgtDKfSCkt78oqncr3faY1JtMj0GQPBZ2g6t8o1cMzZgyLMepeVuTQKxLq
        nM6L48n8dJL2y0N3i7WSOV6csB79r9tcIs88Al9iCwVLpt74OzDXAIwD1QlCAvj7
        5ZEnwtgP6Xr6s+wHg9hLD0OSSQOaTdCqzR5shQWyrjVs7GCqd/5WbNGYNaolPQJB
        u9wTNhHjP9XGCTv8iKXkYwXDYujInkF5hOjpCHE8Vu+9UeMI+l0GJAEervT51bwa
        f288GiqgapP5g4HF8V4P0+03B/SwXhvxxZpKjTU2OcFS8I9chtEZ7Ucw5bvPkC1l
        yjNnqQXZyc4JnAqbFQTlPGnwbEsm0UGE+v0FOxTBznIivHoPHV++5PuYpombiV/l
        fYy8BFx0Hk1igNI3iLNjlIg4MLXNWjN3VROlXIiSsWNXjr/Frh/OE87PXoaKhsDP
        Wfv6akd9PxSDqJMQ4j8k11kCSOnnU8Rw4TSVnTpt7gfYzBULrMr7qDgyH/TYvp0o
        7mAq4IaraFJmTqm+LjfAJT4PCQOpnMNRHLyXZn6mtMUujNv7+zR/I9Vqi+bJdQBw
        NWdJcUQVarqx2zxV2rM7l+j0gsmxjxp5yAPyALzla7XzVvbboLMnHeM+CzN4GoB2
        PC4neVDSw7Dfs8vbm7f9CeDGvhjWrzcSCOsUCalvH1WkbpE0b9cE/TSJqwXFXEc8
        npTSQWH58RXDALNF3qCbW5LaboD4wCloClEakgljT0s0mTdqIwzj4xtdlX0S2dCt
        aV5MHQJiQHGwPvBSCiIsh8ZWJPwZ7Ron7xoWgNI0lXRqhKm7fvFNCzF+Ray2kOyK
        WHbsMy3a5Zx3yZxCGFwexAxgYfAH9HJ1raHi1XYGpNpBlDXg47U1y4uU7RBeBb4e
        wETv/B/p8sGW88tIY87dHM2xHjnntGXLvkCIkmjXGNULAVYHxDPJTygxSF7uTVPY
        CBUk5KeLnDVcgWF8p/wxigza47ptMtpVtm9ZrSEu9vzOnaTN3KO75RkkLuZszhXf
        mMZ46NQrdS7Y6duCiN501BnR5BUC4dX4iMxchix6aLN3pKv6UOEwwcx2aSF4Ib96
        3MAtTPEjhqe+dt0VcTar/08uLWv1kQQueFaEGet4KQ975ZqaoU3PmZH9gqTOumX1
        EKf0Pnl0F3NMW+lEOlnCEce99zHBwBf7LV0pLNLDONBxpQySrZUQsNfhAq2Ktf5n
        G2hcGAMFMWIsEOuOyzEe+l2aHW4/ojOjU1Oqy4TLzVVRlWmPxQde7qKbvBN7CQhD
        qJHJA2nphQT/4DOdLqkwRiauMmnfx9FI3+hYnX25NpnyvM/Pt94vpsRkhcuttu4/
        G4xOYTNQd2YWjGwhypZ0oPSiriuTbhj6y2qm33jiwxvegeDz8CmAIhOEY2JoVpjj
        X9XzuNR2f5KTLmLzEkOcJQ
    ",
    body: "c/k+TjhBA6Je1DF8rxZhe2Jm8CN0zbkV9rre6nA8g98",
};

#[cfg(test)]
mod tests {
    #[test]
    fn every_known_answer_is_given() {
        let verdicts = super::verify();
        assert_eq!(verdicts.len(), 3 * 3 + 2 + 1 + 10 + 5 + 10 + 5 + 5);
        let failed: Vec<_> = verdicts.iter().filter(|v| v.failure.is_some()).collect();
        assert!(failed.is_empty(), "{failed:#?}");
    }
}
