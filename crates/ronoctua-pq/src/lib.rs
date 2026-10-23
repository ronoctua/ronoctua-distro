//! Post-quantum age — the one crate that holds the person's PQ identities.
//!
//! # What it is
//!
//! age's hybrid recipient `mlkem768x25519` (C2SP age; the KEM is X-Wing:
//! ML-KEM-768 and X25519 in one), so a file sealed today stays sealed against
//! "harvest now, decrypt later" — and opens with the Go `age` ≥ 1.3, with no
//! distro at all:
//!
//! ```text
//! identity   AGE-SECRET-KEY-PQ-1…    (the person's; never printed, never in argv)
//! recipient  age1pq1…                 (public; ~1960 characters of Bech32 without its 90 limit)
//! stanza     -> mlkem768x25519 <enc>   label "postquantum": age refuses to mix it with classical ones
//! ```
//!
//! # Why it is written here
//!
//! The `age` crate in the lock (0.12) speaks the tagged PQ recipient of
//! hardware keys but not this one. Its pieces are all there — `hpke`,
//! `ml-kem`, `x25519-dalek`, `sha3`, and age-core's long Bech32 — so the
//! recipient is assembled from them (`xwing.rs`, `crypt.rs`) instead of a
//! second, unaudited X-Wing crate; the known answers of the three texts that
//! define it (`vectors.rs`) and the Go `age` CLI, both ways, decide whether it
//! is right.
//!
//! # Layers
//!
//! Without features: the wire (the queue's [`Job`], [`Line`], [`Why`],
//! [`Kind`], [`sniff`]) — what the transfer queue links. Feature `crypt`: age.
//! Feature `sign`: the person's ML-DSA-65 signatures and the signed manifest
//! of a tree (`sign`), without `age`. Feature `cli`: the `ronoctua-seal`
//! child the queue runs.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[cfg(feature = "crypt")]
mod crypt;
#[cfg(feature = "crypt")]
pub use crypt::{
    Identity, PASSPHRASE_WORK, PASSPHRASE_WORK_MOST, Recipient, open, open_passphrase, seal,
    seal_passphrase,
};
#[cfg(feature = "crypt")]
#[doc(hidden)]
pub mod vectors;
#[cfg(feature = "crypt")]
mod xwing;
// THE PERSON'S SIGNATURES: ML-DSA-65 from `aws-lc-rs`, and the signed
// manifest of a tree — what the belongings' plain part carries and root's
// applier holds the person's staging to. Without `age`: root's applier
// links only this.
#[cfg(feature = "sign")]
pub mod sign;

/// The stanza's tag.
pub const STANZA: &str = "mlkem768x25519";

/// age's other post-quantum stanza: the tagged hybrid of hardware keys
/// (ML-KEM-768 and P-256). Not one this crate opens, but PQ all the same — it
/// carries [`LABEL`], so age seals it beside [`STANZA`] and never beside a
/// classical one.
const TAGGED_PQ_STANZA: &str = "mlkem768p256tag";

/// The label every PQ stanza carries: age seals only when every recipient
/// says the same labels, so a PQ recipient never travels with a classical one.
pub const LABEL: &str = "postquantum";

/// The identity's Bech32 prefix (upper case, as written).
pub const IDENTITY_PREFIX: &str = "AGE-SECRET-KEY-PQ-1";

/// The recipient's Bech32 prefix.
pub const RECIPIENT_PREFIX: &str = "age1pq1";

/// Why a seal or an open did not happen, as a closed set
/// (`seal-refused-<id>` on screen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// No identity given opens it.
    WrongKey,
    /// The passphrase does not open it.
    WrongPassphrase,
    /// It is not age, or it is damaged.
    Corrupt,
    /// PQ and classical recipients together: age refuses the mix, and so do we.
    Mixed,
    /// A read or a write failed, or what was handed over is not usable.
    Failed,
}

impl Why {
    /// Every one.
    pub const ALL: [Self; 5] = [
        Self::WrongKey,
        Self::WrongPassphrase,
        Self::Corrupt,
        Self::Mixed,
        Self::Failed,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::WrongKey => "wrong-key",
            Self::WrongPassphrase => "wrong-passphrase",
            Self::Corrupt => "corrupt",
            Self::Mixed => "mixed",
            Self::Failed => "failed",
        }
    }

    /// The one named `id`.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }
}

/// A refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

impl Refusal {
    /// A refusal for `why`.
    #[must_use]
    pub const fn of(why: Why) -> Self {
        Self { why }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.why.id())
    }
}

impl std::error::Error for Refusal {}

/// What an age file's header says it is sealed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Only post-quantum recipients ([`STANZA`], or age's tagged hybrid).
    Pq,
    /// Only classical recipients (X25519, SSH, plugins).
    Classical,
    /// One passphrase (scrypt).
    Passphrase,
    /// PQ and classical stanzas together — refused.
    Mixed,
    /// Not an age file (or armoured, which this crate does not read).
    NotAge,
}

/// What the header at the start of `bytes` says. Pure: it reads the header's
/// text and nothing else, so a door can ask it of the first few kilobytes of
/// a file before choosing which question to ask.
#[must_use]
pub fn sniff(bytes: &[u8]) -> Kind {
    let mut lines = bytes.split(|&b| b == b'\n');
    if lines.next() != Some(b"age-encryption.org/v1".as_slice()) {
        return Kind::NotAge;
    }
    let (mut pq, mut scrypt, mut classical) = (0_usize, 0_usize, 0_usize);
    let mut ended = false;
    for line in lines {
        if line.starts_with(b"---") {
            ended = true;
            break;
        }
        let Some(rest) = line.strip_prefix(b"-> ") else {
            continue;
        };
        let tag = rest.split(|&b| b == b' ').next().unwrap_or_default();
        if tag.ends_with(b"-grease") {
            continue;
        }
        match tag {
            t if t == STANZA.as_bytes() || t == TAGGED_PQ_STANZA.as_bytes() => pq += 1,
            b"scrypt" => scrypt += 1,
            _ => classical += 1,
        }
    }
    match (ended || pq + scrypt + classical > 0, pq, scrypt, classical) {
        (false, ..) => Kind::NotAge,
        (true, 0, 1, 0) => Kind::Passphrase,
        (true, p, 0, 0) if p > 0 => Kind::Pq,
        (true, 0, 0, c) if c > 0 => Kind::Classical,
        (true, 0, 0, 0) => Kind::NotAge,
        _ => Kind::Mixed,
    }
}

/// The bytes of a recipient's key: ML-KEM-768's encapsulation key (1184) and
/// X25519's public key (32), X-Wing's order.
const RECIPIENT_KEY_BYTES: usize = 1184 + 32;

/// Whether `text` is an `age1pq1…` recipient: its prefix, Bech32's alphabet,
/// a key of the right length and a checksum that holds. Pure, and without
/// `age` — what a door asks of a key the person pasted BEFORE it queues a
/// seal, so a key cut short or mistyped is said at once, and never found
/// out by the queue after the person left. Upper or lower case, never both
/// (Bech32's rule); spaces around it are the paste's, and ignored.
#[must_use]
pub fn is_recipient(text: &str) -> bool {
    const ALPHABET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
    const GENERATOR: [u32; 5] = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ];
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    if text != lower && text != text.to_ascii_uppercase() {
        return false;
    }
    let Some(data) = lower.strip_prefix(RECIPIENT_PREFIX) else {
        return false;
    };
    // The key in five-bit groups, padded, and the six of the checksum.
    let groups = (RECIPIENT_KEY_BYTES * 8).div_ceil(5);
    if data.len() != groups + 6 {
        return false;
    }
    let polymod = |values: &mut dyn Iterator<Item = u8>| {
        values.fold(1_u32, |chk, v| {
            let top = chk >> 25;
            let mut chk = ((chk & 0x01ff_ffff) << 5) ^ u32::from(v);
            for (i, g) in GENERATOR.iter().enumerate() {
                if (top >> i) & 1 == 1 {
                    chk ^= g;
                }
            }
            chk
        })
    };
    let mut values = Vec::with_capacity(data.len());
    for b in data.bytes() {
        match ALPHABET.iter().position(|&a| a == b) {
            Some(v) => values.push(u8::try_from(v).unwrap_or(0)),
            None => return false,
        }
    }
    // THE HUMAN-READABLE PART, expanded as Bech32 hashes it: its high bits, a
    // zero, its low bits — then the data and the checksum. Bech32 (not m):
    // the remainder is one.
    let hrp = RECIPIENT_PREFIX.trim_end_matches('1').as_bytes();
    let mut all = hrp
        .iter()
        .map(|c| c >> 5)
        .chain(std::iter::once(0))
        .chain(hrp.iter().map(|c| c & 31))
        .chain(values.iter().copied());
    polymod(&mut all) == 1
}

/// What the queue hands `ronoctua-seal` on its standard input, whole.
///
/// Secrets never cross it: a passphrase or an identity is the PATH of a file
/// created `0600` (`create_new` + mode) in the runtime directory, which the
/// queue removes when the job ends ([`Job::secrets`]). The output is created
/// new and never overwrites; the input is never deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Job {
    /// What to do.
    pub action: Action,
    /// What is read.
    pub from: PathBuf,
    /// What is written — a name that must not exist yet.
    pub into: PathBuf,
}

/// Seal or open, and with what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "kebab-case",
    rename_all_fields = "kebab-case",
    tag = "do",
    deny_unknown_fields
)]
pub enum Action {
    /// To these recipients (`age1pq1…`; the person's own among them for "to
    /// myself").
    SealTo {
        /// The recipients, as written.
        recipients: Vec<String>,
    },
    /// To a passphrase (scrypt).
    SealPassphrase {
        /// The `0600` file holding it.
        passphrase_file: PathBuf,
        /// scrypt's log₂ N; `None` lets age pick about a second.
        work: Option<u8>,
    },
    /// With the post-quantum identities of a `0600` file, one per line.
    Open {
        /// The `0600` file holding them.
        identities_file: PathBuf,
    },
    /// With a passphrase.
    OpenPassphrase {
        /// The `0600` file holding it.
        passphrase_file: PathBuf,
    },
    /// With the identities an age file sealed to a passphrase holds — the
    /// belongings' `key.age`, for a file sealed "to myself". The identity is
    /// opened in memory and never written anywhere.
    OpenWithKey {
        /// The sealed identities: read, never removed — it is not the job's.
        key_file: PathBuf,
        /// The `0600` file holding the passphrase that opens it.
        passphrase_file: PathBuf,
    },
}

impl Job {
    /// The secret files this job carries — what the queue removes when it
    /// ends, however it ends.
    #[must_use]
    pub fn secrets(&self) -> Vec<&Path> {
        match &self.action {
            Action::SealTo { .. } => Vec::new(),
            Action::SealPassphrase {
                passphrase_file, ..
            }
            | Action::OpenPassphrase { passphrase_file }
            // THE PASSPHRASE ONLY: the key file is the belongings', and a
            // job that removed it would take the person's identity with it.
            | Action::OpenWithKey {
                passphrase_file, ..
            } => vec![passphrase_file.as_path()],
            Action::Open { identities_file } => vec![identities_file.as_path()],
        }
    }

    /// Whether it seals (else it opens).
    #[must_use]
    pub const fn seals(&self) -> bool {
        matches!(
            self.action,
            Action::SealTo { .. } | Action::SealPassphrase { .. }
        )
    }
}

/// One line `ronoctua-seal` prints on its standard output: progress, then
/// the verdict last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Line {
    /// `bytes <done>/<total>` (`-` for a total nobody knows).
    Bytes {
        /// Read so far.
        done: u64,
        /// Of how many, when known.
        total: Option<u64>,
    },
    /// `done <bytes>`: it worked, and this many were read.
    Done {
        /// Bytes read.
        bytes: u64,
    },
    /// `refused <why>`.
    Refused(Why),
}

impl Line {
    /// The line, without its break.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Bytes { done, total } => match total {
                Some(t) => format!("bytes {done}/{t}"),
                None => format!("bytes {done}/-"),
            },
            Self::Done { bytes } => format!("done {bytes}"),
            Self::Refused(why) => format!("refused {}", why.id()),
        }
    }

    /// What a line said; `None` for anything else.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\r', '\n']);
        let (word, rest) = line.split_once(' ')?;
        match word {
            "bytes" => {
                let (done, total) = rest.split_once('/')?;
                let total = match total {
                    "-" => None,
                    t => Some(t.parse().ok()?),
                };
                Some(Self::Bytes {
                    done: done.parse().ok()?,
                    total,
                })
            }
            "done" => Some(Self::Done {
                bytes: rest.parse().ok()?,
            }),
            "refused" => Why::of_id(rest).map(Self::Refused),
            _ => None,
        }
    }

    /// Whether it is the verdict.
    #[must_use]
    pub const fn is_verdict(&self) -> bool {
        !matches!(self, Self::Bytes { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_says_what_it_is_sealed_to() {
        let header = |stanzas: &[&str]| {
            let mut h = String::from("age-encryption.org/v1\n");
            for s in stanzas {
                h.push_str(&format!("-> {s}\nYm9keQ\n"));
            }
            h.push_str("--- bWFj\n");
            h.into_bytes()
        };
        assert_eq!(sniff(&header(&["mlkem768x25519 ZW5j"])), Kind::Pq);
        assert_eq!(
            sniff(&header(&["mlkem768x25519 YQ", "mlkem768x25519 Yg"])),
            Kind::Pq
        );
        assert_eq!(sniff(&header(&["X25519 ZXBr"])), Kind::Classical);
        // age's tagged PQ recipient is PQ; its classical sibling is not.
        assert_eq!(
            sniff(&header(&["mlkem768x25519 YQ", "mlkem768p256tag dGFn ZW5j"])),
            Kind::Pq
        );
        assert_eq!(
            sniff(&header(&["mlkem768x25519 YQ", "p256tag dGFn ZW5j"])),
            Kind::Mixed
        );
        assert_eq!(sniff(&header(&["scrypt c2FsdA 18"])), Kind::Passphrase);
        assert_eq!(
            sniff(&header(&["mlkem768x25519 YQ", "X25519 ZXBr"])),
            Kind::Mixed
        );
        assert_eq!(
            sniff(&header(&["X25519 ZXBr", "a?b-grease x"])),
            Kind::Classical
        );
        assert_eq!(sniff(b"-----BEGIN AGE ENCRYPTED FILE-----\n"), Kind::NotAge);
        assert_eq!(sniff(b"hello"), Kind::NotAge);
    }

    #[test]
    fn the_lines_cross_and_come_back() {
        for line in [
            Line::Bytes {
                done: 5,
                total: Some(9),
            },
            Line::Bytes {
                done: 5,
                total: None,
            },
            Line::Done { bytes: 9 },
            Line::Refused(Why::Mixed),
        ] {
            assert_eq!(Line::parse(&line.text()), Some(line));
        }
        for noise in [
            "",
            "bytes",
            "bytes x/y",
            "done",
            "refused nonsense",
            "hello world",
        ] {
            assert_eq!(Line::parse(noise), None, "{noise:?}");
        }
    }

    #[test]
    fn a_job_names_its_secrets_and_refuses_unknown_fields() {
        let job = Job {
            action: Action::OpenPassphrase {
                passphrase_file: "/run/user/1000/ronoctua/seal/p".into(),
            },
            from: "/home/p/a.age".into(),
            into: "/home/p/a".into(),
        };
        let text = serde_json::to_string(&job).unwrap();
        assert_eq!(serde_json::from_str::<Job>(&text).unwrap(), job);
        assert_eq!(job.secrets(), [Path::new("/run/user/1000/ronoctua/seal/p")]);
        assert!(!job.seals());
        let extra = text.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<Job>(&extra).is_err());
        for why in Why::ALL {
            assert_eq!(Why::of_id(why.id()), Some(why));
        }
    }

    /// FAILS IF: the shape a door checks a pasted key with lets a key that
    /// age would refuse through — one character changed, one cut off, the
    /// identity pasted instead, a classical key — or refuses a real one.
    #[cfg(feature = "crypt")]
    #[test]
    fn a_pasted_key_is_checked_before_anything_is_queued() {
        use age::secrecy::ExposeSecret as _;
        let me = Identity::generate();
        let real = me.recipient().to_string();
        assert!(is_recipient(&real), "a real recipient was refused");
        assert!(is_recipient(&format!("  {real}\n")), "a paste's spaces");
        assert!(is_recipient(&real.to_ascii_uppercase()), "upper case");
        let mut mixed = real.clone();
        mixed.replace_range(10..11, &mixed[10..11].to_ascii_uppercase());
        let at = real.len() / 2;
        let swapped = if &real[at..=at] == "q" { "p" } else { "q" };
        let mut changed = real.clone();
        changed.replace_range(at..=at, swapped);
        let classical = age::x25519::Identity::generate().to_public().to_string();
        for wrong in [
            changed,
            real[..real.len() - 1].to_owned(),
            format!("{real}q"),
            me.to_secret().expose_secret().to_owned(),
            classical,
            String::new(),
            "age1pq1".to_owned(),
        ] {
            assert!(!is_recipient(&wrong), "taken: {wrong:.40}");
        }
        if mixed != real {
            assert!(!is_recipient(&mixed), "mixed case");
        }
    }

    /// FAILS IF: a job opened with the belongings' key names that key among
    /// its secrets — the queue removes every secret when a job ends, and it
    /// would remove the person's identity.
    #[test]
    fn opening_with_a_key_names_only_the_passphrase_as_a_secret() {
        let job = Job {
            action: Action::OpenWithKey {
                key_file: "/home/p/.local/state/ronoctua/belongings/repo/key.age".into(),
                passphrase_file: "/run/user/1000/ronoctua-transfers/secrets/job-1".into(),
            },
            from: "/home/p/a.age".into(),
            into: "/home/p/a".into(),
        };
        assert_eq!(
            job.secrets(),
            [Path::new("/run/user/1000/ronoctua-transfers/secrets/job-1")]
        );
        assert!(!job.seals());
        let text = serde_json::to_string(&job).unwrap();
        assert!(text.contains(r#""do":"open-with-key""#), "{text}");
        assert!(text.contains(r#""key-file":"#), "{text}");
        assert_eq!(serde_json::from_str::<Job>(&text).unwrap(), job);
    }
}
