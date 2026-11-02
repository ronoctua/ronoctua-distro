//! Who signs the person's commits: their security key, a key held by this
//! machine's TPM, or a key kept in a plain file.
//!
//! # A key in the TPM, behind a switch only root moves
//!
//! A security key asks for a touch, and a person away from the machine — a
//! session over SSH, an agent working for them — cannot touch it. A key made
//! INSIDE the TPM signs with no touch, and never leaves the machine (the
//! private half is wrapped by the TPM; the file beside it is useless on any
//! other one). That is also its danger: on this machine, whatever reaches it
//! signs as the person. So it reaches nobody unless the person said so with
//! the administrator's password:
//!
//! - the TPM is root's and its agent's alone (`/dev/tpmrm0`, group `tss`; the
//!   person is not in it), so no process of the person's can use the key by
//!   itself;
//! - the key is made by a declared unit ([`KEYGEN_UNIT`]) that the sudo door
//!   starts, into root's folder ([`SEALED_KEY`], [`PUBLIC_KEY`]);
//! - the agent that signs with it ([`AGENT_UNIT`], `ssh-tpm-agent`) listens
//!   on a socket only the person may open ([`SOCKET`]), and exists only
//!   while [`ALLOWED`] does — the switch, written and taken away by the sudo
//!   door ([`set_allowed`]); the units carry the same file as a condition, so
//!   a boot keeps the choice;
//! - while it is allowed, git's signer (`nix/modules/ssh.nix`) signs with the
//!   TPM's key instead of the one git names — every commit, here or from
//!   afar (owner, 2026-11-02); while it is not, only the security key signs.
//!
//! # A key in a plain file
//!
//! A private key on the disk signs for whoever can read the file, with no
//! touch and from any machine it is copied to — the danger the security
//! centre marks ([`file_keys`]). A security key's handle and a TPM-wrapped
//! key are files too, and they are not that: each is read by what it holds
//! ([`holder_of`], [`gnupg_holder_of`]), never by its name.

use std::path::{Path, PathBuf};

use crate::tools::{self, Tools};
use crate::{Refusal, Why, refused};

/// Root's folder for the TPM's key: the agent's, readable by all
/// (`nix/modules/tpm-signing.nix`).
pub const KEY_DIR: &str = "/var/lib/ronoctua-security/tpm-key";

/// The TPM-wrapped private half, `ssh-tpm-keygen`'s `.tpm` file — usable
/// only through this machine's TPM.
pub const SEALED_KEY: &str = "/var/lib/ronoctua-security/tpm-key/id_ecdsa.tpm";

/// Its public half, one OpenSSH line, readable by all: what the person
/// registers where their commits are checked, and what git's signer names.
pub const PUBLIC_KEY: &str = "/var/lib/ronoctua-security/tpm-key/id_ecdsa.pub";

/// THE SWITCH: present, the TPM's key signs; absent, only the security key
/// does. Root's, in the folder the security watch already follows.
pub const ALLOWED: &str = "/var/lib/ronoctua-security/tpm-signing";

/// Where the TPM's agent listens — the person's alone (`SocketMode=0600`).
pub const SOCKET: &str = "/run/ronoctua-tpm-agent/agent.sock";

/// The agent's socket unit, started and stopped with the switch.
pub const AGENT_SOCKET_UNIT: &str = "ronoctua-tpm-agent.socket";

/// The agent the socket starts.
pub const AGENT_UNIT: &str = "ronoctua-tpm-agent.service";

/// The unit that makes the TPM's key, once.
pub const KEYGEN_UNIT: &str = "ronoctua-tpm-keygen.service";

/// The largest file read to tell what holds a key: a private key is a few
/// kilobytes, and a folder may hold anything.
const MOST_READ: u64 = 64 * 1024;

/// The first line of an OpenSSH private key, assembled in pieces: written
/// whole, this file would read as a private key to the repository's own
/// secret scans (`secrets-out-of-repo`).
const OPENSSH_ARMOUR: &str = concat!("-----BEGIN OPENSSH ", "PRIVATE KEY-----");

/// What holds a private key's secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Holder {
    /// A security key (FIDO2 `sk-…`, or an OpenPGP card's stub): the file is
    /// a handle, and nothing signs without the key and a touch.
    SecurityKey,
    /// This machine's TPM: the file is wrapped by it.
    Tpm,
    /// The file itself: whoever reads it holds the key.
    File,
}

/// What holds the secret of the private key in `bytes` — an SSH key in any of
/// the formats `ssh` reads, a TPM-wrapped one, a PuTTY one — or `None` for
/// bytes that are no private key (a public key, `known_hosts`, a config).
/// Pure.
#[must_use]
pub fn holder_of(bytes: &[u8]) -> Option<Holder> {
    let text = std::str::from_utf8(bytes).ok()?.trim_start();
    if let Some(rest) = text.strip_prefix(OPENSSH_ARMOUR) {
        let body = rest.split("-----END").next().unwrap_or_default();
        // A BODY THAT DOES NOT READ is still a private key's armour: a file
        // that says it is one is taken at its word, and the loud side.
        return Some(match openssh_key_type(body) {
            Some(kind) if kind.starts_with("sk-") => Holder::SecurityKey,
            _ => Holder::File,
        });
    }
    if text.starts_with("-----BEGIN TSS2 PRIVATE KEY-----") {
        return Some(Holder::Tpm);
    }
    // PEM (`RSA`, `DSA`, `EC`, PKCS#8 plain or encrypted) and PuTTY's own:
    // the secret is in the file.
    let pem = text
        .lines()
        .next()
        .is_some_and(|l| l.starts_with("-----BEGIN ") && l.ends_with("PRIVATE KEY-----"));
    (pem || text.starts_with("PuTTY-User-Key-File-")).then_some(Holder::File)
}

/// What holds the secret of a GnuPG key file (`private-keys-v1.d/*.key`): a
/// card's stub is `shadowed`, and every other private key is in the file,
/// protected by a passphrase or not. `None` for a file that is no key. Pure.
#[must_use]
pub fn gnupg_holder_of(bytes: &[u8]) -> Option<Holder> {
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
    if has(b"shadowed-private-key") {
        Some(Holder::SecurityKey)
    } else if has(b"private-key") {
        Some(Holder::File)
    } else {
        None
    }
}

/// The key type an `openssh-key-v1` body names in its public part — which is
/// never encrypted, so a key with a passphrase says it too. Pure.
fn openssh_key_type(body: &str) -> Option<String> {
    use base64::Engine as _;
    let packed: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    let raw = base64::engine::general_purpose::STANDARD
        .decode(packed)
        .ok()?;
    let rest = raw.strip_prefix(b"openssh-key-v1\0")?;
    // The cipher, the KDF and its options, then the count of keys.
    let (_, rest) = ssh_string(rest)?;
    let (_, rest) = ssh_string(rest)?;
    let (_, rest) = ssh_string(rest)?;
    let (_, rest) = rest.split_first_chunk::<4>()?;
    let (public, _) = ssh_string(rest)?;
    let (kind, _) = ssh_string(public)?;
    std::str::from_utf8(kind).ok().map(str::to_owned)
}

/// One SSH wire string — a big-endian length, then that many bytes — and
/// what follows it. Pure.
fn ssh_string(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let (len, tail) = bytes.split_first_chunk::<4>()?;
    tail.split_at_checked(usize::try_from(u32::from_be_bytes(*len)).ok()?)
}

/// The private keys under `home` kept in plain files: in `~/.ssh` (every
/// file, by what it holds) and in GnuPG's `private-keys-v1.d`. Sorted; a
/// folder that is not there has none.
#[must_use]
pub fn file_keys(home: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = files_in(&home.join(".ssh"))
        .filter(|p| read_small(p).as_deref().and_then(holder_of) == Some(Holder::File))
        .collect();
    found.extend(
        files_in(&home.join(".gnupg").join("private-keys-v1.d"))
            .filter(|p| p.extension().is_some_and(|e| e == "key"))
            .filter(|p| read_small(p).as_deref().and_then(gnupg_holder_of) == Some(Holder::File)),
    );
    found.sort();
    found
}

/// The regular files directly in `dir` — a link is followed to see what it
/// is, a folder below is not walked.
fn files_in(dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
}

/// The file's bytes, when it is small enough to be a key.
fn read_small(path: &Path) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > MOST_READ {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MOST_READ).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// The TPM's key, as its public line says it.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TpmKey {
    /// The key type (`ecdsa-sha2-nistp256`).
    pub kind: String,
    /// The comment the unit gave it (data).
    pub comment: String,
    /// The whole line, as it is registered.
    pub line: String,
}

/// The TPM's key of the public line in `text`; `None` for text that is no
/// one-line OpenSSH public key. Pure.
#[must_use]
pub fn tpm_key_of(text: &str) -> Option<TpmKey> {
    let line = text.lines().next()?.trim();
    let mut words = line.splitn(3, ' ');
    let kind = words.next()?;
    let blob = words.next()?;
    let comment = words.next().unwrap_or_default().trim();
    let plain_kind = !kind.is_empty()
        && kind
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'@' | b'.'));
    let plain_blob = !blob.is_empty()
        && blob
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='));
    (plain_kind && plain_blob).then(|| TpmKey {
        kind: kind.to_owned(),
        comment: comment.chars().filter(|c| !c.is_control()).collect(),
        line: line.to_owned(),
    })
}

/// Where the switch and the key are — handed in, so a test has its own.
#[derive(Debug, Clone)]
pub struct Places {
    /// [`ALLOWED`].
    pub allowed: PathBuf,
    /// [`SEALED_KEY`].
    pub sealed: PathBuf,
    /// [`PUBLIC_KEY`].
    pub public: PathBuf,
}

impl Places {
    /// The machine's.
    #[must_use]
    pub fn system() -> Self {
        Self {
            allowed: PathBuf::from(ALLOWED),
            sealed: PathBuf::from(SEALED_KEY),
            public: PathBuf::from(PUBLIC_KEY),
        }
    }

    /// The TPM's key, when one was made: its wrapped half is there and its
    /// public line reads.
    #[must_use]
    pub fn key(&self) -> Option<TpmKey> {
        if !self.sealed.is_file() {
            return None;
        }
        tpm_key_of(&String::from_utf8(read_small(&self.public)?).ok()?)
    }

    /// Whether the TPM's key may sign — the switch is there.
    #[must_use]
    pub fn allowed(&self) -> bool {
        self.allowed.symlink_metadata().is_ok()
    }
}

/// The machine's TPM key, when one was made.
#[must_use]
pub fn tpm_key() -> Option<TpmKey> {
    Places::system().key()
}

/// Whether the machine's TPM key may sign the person's commits.
#[must_use]
pub fn allowed() -> bool {
    Places::system().allowed()
}

/// Makes the TPM's key, once (root, through the door): the declared unit
/// runs `ssh-tpm-keygen` as the agent, and this waits for it.
///
/// # Errors
///
/// [`Why::Invalid`] when there is a key already — a second one would orphan
/// the first, which may be registered somewhere; [`Why::Failed`] when the
/// unit failed or left no key; the program's own refusals.
pub fn create_key(places: &Places, tools: &Tools) -> Result<(), Refusal> {
    if places.sealed.symlink_metadata().is_ok() {
        eprintln!("<3>the TPM already has a signing key here");
        return Err(refused(Why::Invalid));
    }
    let systemctl = tools::given(tools.systemctl.as_ref())?;
    tools::run(systemctl, &["start", KEYGEN_UNIT], None)?;
    places.key().map(|_| ()).ok_or_else(|| {
        eprintln!("<3>{KEYGEN_UNIT} ended and left no key");
        refused(Why::Failed)
    })
}

/// Lets the TPM's key sign, or stops it (root, through the door): the
/// switch written or taken away FIRST, then the agent's socket started — or
/// the agent stopped, so nothing that already holds the socket signs again.
///
/// # Errors
///
/// [`Why::Invalid`] to allow with no key made; [`Why::Failed`] when the
/// switch cannot be written; the program's own refusals.
pub fn set_allowed(places: &Places, tools: &Tools, on: bool) -> Result<(), Refusal> {
    let systemctl = tools::given(tools.systemctl.as_ref())?;
    if on {
        if places.key().is_none() {
            eprintln!("<3>no TPM key to allow");
            return Err(refused(Why::Invalid));
        }
        crate::write_whole(&places.allowed, b"", 0o644).map_err(|e| {
            eprintln!("<3>{} could not be written: {e}", places.allowed.display());
            refused(Why::Failed)
        })?;
        tools::run(systemctl, &["start", AGENT_SOCKET_UNIT], None).map(|_| ())
    } else {
        match std::fs::remove_file(&places.allowed) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                eprintln!("<3>{} could not be removed: {e}", places.allowed.display());
                return Err(refused(Why::Failed));
            }
        }
        tools::run(systemctl, &["stop", AGENT_SOCKET_UNIT, AGENT_UNIT], None).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    /// An `openssh-key-v1` body naming `kind` in its public part, armoured
    /// the way `ssh-keygen` writes one (the private part left empty: only
    /// the public one is read).
    fn openssh(kind: &str) -> String {
        use base64::Engine as _;
        let string = |s: &[u8]| {
            let mut v = u32::try_from(s.len()).unwrap().to_be_bytes().to_vec();
            v.extend_from_slice(s);
            v
        };
        let mut public = string(kind.as_bytes());
        public.extend(string(b"key-bytes"));
        let mut raw = b"openssh-key-v1\0".to_vec();
        raw.extend(string(b"aes256-ctr"));
        raw.extend(string(b"bcrypt"));
        raw.extend(string(b"salt"));
        raw.extend(1_u32.to_be_bytes());
        raw.extend(string(&public));
        raw.extend(string(b""));
        let body = base64::engine::general_purpose::STANDARD.encode(raw);
        let lines: Vec<&str> = body
            .as_bytes()
            .chunks(70)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect();
        format!(
            "{OPENSSH_ARMOUR}\n{}\n-----END OPENSSH PRIVATE KEY-----\n",
            lines.join("\n")
        )
    }

    /// FAILS IF: a key is told by its name rather than by what holds it — a
    /// security key's handle taken for a file key, a plain one (with a
    /// passphrase or not, in any armour) let through, or a public key, a
    /// config or `known_hosts` taken for a key.
    #[test]
    fn a_key_is_told_by_what_holds_it() {
        let sk = openssh("sk-ssh-ed25519@openssh.com");
        assert_eq!(holder_of(sk.as_bytes()), Some(Holder::SecurityKey));
        let ecdsa_sk = openssh("sk-ecdsa-sha2-nistp256@openssh.com");
        assert_eq!(holder_of(ecdsa_sk.as_bytes()), Some(Holder::SecurityKey));
        for plain in ["ssh-ed25519", "ssh-rsa", "ecdsa-sha2-nistp256"] {
            assert_eq!(
                holder_of(openssh(plain).as_bytes()),
                Some(Holder::File),
                "{plain}"
            );
        }
        for armour in [
            "RSA PRIVATE KEY",
            "EC PRIVATE KEY",
            "DSA PRIVATE KEY",
            "PRIVATE KEY",
            "ENCRYPTED PRIVATE KEY",
        ] {
            let pem = format!("-----BEGIN {armour}-----\nAAAA\n-----END {armour}-----\n");
            assert_eq!(holder_of(pem.as_bytes()), Some(Holder::File), "{armour}");
        }
        assert_eq!(
            holder_of(b"PuTTY-User-Key-File-3: ssh-ed25519\n"),
            Some(Holder::File)
        );
        let tpm = "-----BEGIN TSS2 PRIVATE KEY-----\nMIIB\n-----END TSS2 PRIVATE KEY-----\n";
        assert_eq!(holder_of(tpm.as_bytes()), Some(Holder::Tpm));
        // AN ARMOUR WHOSE BODY DOES NOT READ is still a private key's.
        let broken = format!("{OPENSSH_ARMOUR}\n!!!\n-----END OPENSSH PRIVATE KEY-----\n");
        assert_eq!(holder_of(broken.as_bytes()), Some(Holder::File));
        for not_a_key in [
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI comment\n",
            "Host *\n  IdentityFile ~/.ssh/id\n",
            "github.com ssh-ed25519 AAAA\n",
            "",
        ] {
            assert_eq!(holder_of(not_a_key.as_bytes()), None, "{not_a_key:?}");
        }
        assert_eq!(holder_of(&[0xff, 0xfe, 0x00]), None);
    }

    /// FAILS IF: an OpenPGP card's stub is taken for a key on the disk, or a
    /// key on the disk (protected or not) for a card's.
    #[test]
    fn a_gnupg_key_is_on_a_card_or_in_its_file() {
        assert_eq!(
            gnupg_holder_of(b"Key: (shadowed-private-key (rsa (n #00#)))"),
            Some(Holder::SecurityKey)
        );
        assert_eq!(
            gnupg_holder_of(b"(20:shadowed-private-key(3:rsa))"),
            Some(Holder::SecurityKey)
        );
        assert_eq!(
            gnupg_holder_of(b"Key: (protected-private-key (ecc (curve Ed25519)))"),
            Some(Holder::File)
        );
        assert_eq!(
            gnupg_holder_of(b"(11:private-key(3:rsa))"),
            Some(Holder::File)
        );
        assert_eq!(gnupg_holder_of(b"Created: 20260101T000000"), None);
    }

    /// FAILS IF: a file key in either folder is missed, or anything else —
    /// a security key's handle, a TPM key, a public key, a GnuPG stub — is
    /// reported as one.
    #[test]
    fn the_file_keys_of_a_home_are_found_and_nothing_else() {
        let home = Scratch::new("signing-home");
        let ssh = home.0.join(".ssh");
        let gnupg = home.0.join(".gnupg").join("private-keys-v1.d");
        std::fs::create_dir_all(&ssh).unwrap();
        std::fs::create_dir_all(&gnupg).unwrap();
        assert_eq!(file_keys(&home.0), Vec::<PathBuf>::new());
        std::fs::write(
            ssh.join("id_yubikey"),
            openssh("sk-ssh-ed25519@openssh.com"),
        )
        .unwrap();
        std::fs::write(
            ssh.join("id_yubikey.pub"),
            "sk-ssh-ed25519@openssh.com AAAA x\n",
        )
        .unwrap();
        std::fs::write(
            ssh.join("id_ecdsa.tpm"),
            "-----BEGIN TSS2 PRIVATE KEY-----\nMII\n-----END TSS2 PRIVATE KEY-----\n",
        )
        .unwrap();
        std::fs::write(ssh.join("known_hosts"), "github.com ssh-ed25519 AAAA\n").unwrap();
        std::fs::write(gnupg.join("AB.key"), "Key: (shadowed-private-key (rsa))").unwrap();
        assert_eq!(
            file_keys(&home.0),
            Vec::<PathBuf>::new(),
            "nothing kept in a file"
        );
        std::fs::write(ssh.join("deploy"), openssh("ssh-ed25519")).unwrap();
        std::fs::write(gnupg.join("CD.key"), "Key: (protected-private-key (rsa))").unwrap();
        // A FOLDER BELOW is not walked, and a big file is no key.
        std::fs::create_dir(ssh.join("old")).unwrap();
        std::fs::write(ssh.join("old").join("id_rsa"), openssh("ssh-rsa")).unwrap();
        std::fs::write(ssh.join("huge"), "x".repeat(70 * 1024)).unwrap();
        assert_eq!(
            file_keys(&home.0),
            vec![gnupg.join("CD.key"), ssh.join("deploy")]
        );
    }

    /// FAILS IF: a public line is misread, or text that is not one is taken
    /// for the TPM's key.
    #[test]
    fn the_tpm_key_is_its_public_line() {
        let key = tpm_key_of("ecdsa-sha2-nistp256 AAAAE2VjZHNh+/= noctua@host-tpm\n").unwrap();
        assert_eq!(key.kind, "ecdsa-sha2-nistp256");
        assert_eq!(key.comment, "noctua@host-tpm");
        assert_eq!(
            key.line,
            "ecdsa-sha2-nistp256 AAAAE2VjZHNh+/= noctua@host-tpm"
        );
        assert_eq!(tpm_key_of("ecdsa-sha2-nistp256 AAAA").unwrap().comment, "");
        for not_one in ["", "ecdsa", "ecdsa AAAA$$", "ec dsa\tAAAA x", "\n"] {
            assert_eq!(tpm_key_of(not_one), None, "{not_one:?}");
        }
    }

    fn places(dir: &Scratch) -> Places {
        Places {
            allowed: dir.0.join("tpm-signing"),
            sealed: dir.0.join("id_ecdsa.tpm"),
            public: dir.0.join("id_ecdsa.pub"),
        }
    }

    /// FAILS IF: the switch is allowed with no key, the agent is started
    /// before the switch is written or stopped before it is taken away, or a
    /// second key is made over the first.
    #[test]
    fn the_switch_moves_the_file_then_the_agent() {
        let dir = Scratch::new("signing-switch");
        let places = places(&dir);
        let said = dir.0.join("said");
        let systemctl = dir.program(
            "systemctl",
            &format!(
                "#!/bin/sh\nif [ -e {allowed} ]; then f=with; else f=without; fi\necho \"$* $f\" >> {said}\n",
                allowed = places.allowed.display(),
                said = said.display()
            ),
        );
        let tools = Tools {
            systemctl: Some(systemctl),
            ..Tools::default()
        };
        assert_eq!(
            set_allowed(&places, &tools, true).unwrap_err().why,
            Why::Invalid
        );
        assert!(!places.allowed(), "no key, no switch");
        std::fs::write(&places.sealed, "-----BEGIN TSS2 PRIVATE KEY-----\n").unwrap();
        std::fs::write(&places.public, "ecdsa-sha2-nistp256 AAAA noctua@host-tpm\n").unwrap();
        set_allowed(&places, &tools, true).unwrap();
        assert!(places.allowed());
        set_allowed(&places, &tools, false).unwrap();
        assert!(!places.allowed());
        // OFF TWICE is still off.
        set_allowed(&places, &tools, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(&said).unwrap(),
            format!(
                "start {AGENT_SOCKET_UNIT} with\nstop {AGENT_SOCKET_UNIT} {AGENT_UNIT} without\nstop {AGENT_SOCKET_UNIT} {AGENT_UNIT} without\n"
            )
        );
        assert_eq!(create_key(&places, &tools).unwrap_err().why, Why::Invalid);
    }

    /// FAILS IF: a key is said made when the unit left none, or the unit is
    /// not what makes it.
    #[test]
    fn a_key_is_made_by_its_unit() {
        let dir = Scratch::new("signing-keygen");
        let places = places(&dir);
        let silent = dir.program("silent", "#!/bin/sh\nexit 0\n");
        let tools = Tools {
            systemctl: Some(silent),
            ..Tools::default()
        };
        assert_eq!(create_key(&places, &tools).unwrap_err().why, Why::Failed);
        let makes = dir.program(
            "makes",
            &format!(
                "#!/bin/sh\n[ \"$*\" = \"start {KEYGEN_UNIT}\" ] || exit 9\necho sealed > {sealed}\necho 'ecdsa-sha2-nistp256 AAAA x' > {public}\n",
                sealed = places.sealed.display(),
                public = places.public.display()
            ),
        );
        let tools = Tools {
            systemctl: Some(makes),
            ..Tools::default()
        };
        create_key(&places, &tools).unwrap();
        assert_eq!(places.key().unwrap().kind, "ecdsa-sha2-nistp256");
    }
}
