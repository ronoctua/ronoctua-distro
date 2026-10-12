//! `ronoctua-test-pq` — the known answers, and this crate's half of the Go
//! cross-test.
//!
//! ```text
//! ronoctua-test-pq vectors                       every known answer; exit 1 when one is wrong
//! ronoctua-test-pq identity <file>               a fresh identity into <file> (new, 0600),
//!                                                written as age-keygen writes one; its
//!                                                recipient on stdout
//! ronoctua-test-pq debug <file>                  the `Debug` of the identity in <file>, and
//!                                                of its secret text
//! ronoctua-test-pq mixed <pq> <x25519> <file>    a file sealed to a PQ and a classical
//!                                                recipient at once (new, 0600)
//! ronoctua-test-pq signing-key <file>            a fresh ML-DSA-65 signing key into <file>
//!                                                (new, 0600); its public key on stdout
//! ronoctua-test-pq debug-signing <file>          the `Debug` of the signing key in <file>,
//!                                                of its public key and of a refusal to
//!                                                open a manifest it signed, tampered
//! ronoctua-test-pq sign-tree <key> <dir> <n>     a staged tree signed as the person signs
//!                                                one: `<dir>/signed.json` (every file under
//!                                                `person/` and `brain-modules/` but the
//!                                                machines' records, counter <n>) and
//!                                                `<dir>/signed.sig`, with the key in <key>
//! ```
//!
//! `mixed` writes what no honest sealer writes — age refuses the two labels
//! together, so the PQ stanza's label is dropped on the way — because a
//! refusal to OPEN such a file can only be shown with one in hand.

use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr as _;

use age::secrecy::{ExposeSecret as _, SecretString};
use age_core::format::{FileKey, Stanza};
use ronoctua_pq::{IDENTITY_PREFIX, Identity, Recipient};

const USAGE: &str = "usage: ronoctua-test-pq vectors | identity <file> | debug <file> | mixed <pq-recipient> <x25519-recipient> <file> | signing-key <file> | debug-signing <file> | sign-tree <key-file> <dir> <counter>";

/// A new file nobody else can read, refusing to follow a link or to replace
/// anything.
fn new_private_file(path: &Path) -> std::io::Result<std::fs::File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits()).unwrap_or(0))
        .open(path)
}

fn vectors() -> ExitCode {
    let verdicts = ronoctua_pq::vectors::verify();
    let mut wrong = 0;
    for verdict in &verdicts {
        match &verdict.failure {
            None => println!("ok      {}", verdict.name),
            Some(why) => {
                wrong += 1;
                println!("FAILED  {}: {why}", verdict.name);
            }
        }
    }
    if wrong == 0 && !verdicts.is_empty() {
        println!("OK: {} known answers", verdicts.len());
        ExitCode::SUCCESS
    } else {
        println!("FAILED: {wrong} of {} known answers", verdicts.len());
        ExitCode::FAILURE
    }
}

fn identity(path: &Path) -> Result<(), String> {
    let identity = Identity::generate();
    let recipient = identity.recipient().to_string();
    let mut file = new_private_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = SecretString::from(format!(
        "# public key: {recipient}\n{}\n",
        identity.to_secret().expose_secret()
    ));
    file.write_all(text.expose_secret().as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{recipient}");
    Ok(())
}

fn debug(path: &Path) -> Result<(), String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if meta.mode() & 0o077 != 0 {
        return Err(format!("{}: readable by others", path.display()));
    }
    let mut text = String::new();
    file.read_to_string(&mut text).map_err(|e| e.to_string())?;
    let text = SecretString::from(text);
    let line = text
        .expose_secret()
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with(IDENTITY_PREFIX))
        .ok_or_else(|| format!("{}: no identity", path.display()))?;
    let identity = Identity::from_str(line).map_err(|r| r.to_string())?;
    println!("{identity:?}");
    println!("{:?}", identity.to_secret());
    Ok(())
}

/// A fresh signing key into a new `0600` file; its public key on stdout.
fn signing_key(path: &Path) -> Result<(), String> {
    let key = ronoctua_pq::sign::SigningKey::generate().map_err(|r| r.to_string())?;
    let mut file = new_private_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut text = key.to_secret();
    text.push('\n');
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{}", key.public());
    Ok(())
}

/// The `Debug` of the signing key in `path`, of its public key, and of the
/// refusal a manifest it signed meets once one byte of it changed — every
/// answer a log could carry with the key in hand.
fn debug_signing(path: &Path) -> Result<(), String> {
    use ronoctua_pq::sign;
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if meta.mode() & 0o077 != 0 {
        return Err(format!("{}: readable by others", path.display()));
    }
    let mut text = zeroize::Zeroizing::new(String::new());
    file.read_to_string(&mut text).map_err(|e| e.to_string())?;
    let key = sign::SigningKey::from_secret(&text).map_err(|r| r.to_string())?;
    println!("{key:?}");
    println!("{:?}", key.public());
    let manifest = sign::Manifest {
        format: sign::MANIFEST_FORMAT,
        counter: 1,
        signed_at_s: 0,
        signer: String::new(),
        files: std::collections::BTreeMap::from([(
            "person/options.json".to_owned(),
            sign::sha256_hex(b"{}"),
        )]),
    };
    let (mut json, signature) = sign::sign_manifest(&key, &manifest).map_err(|r| r.to_string())?;
    json.push(b' ');
    println!("{:?}", sign::open_manifest(&json, &signature, key.public()));
    Ok(())
}

/// Every regular file under `dir`, by its path relative to `root`; links
/// and anything else are not followed.
fn files_under(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if kind.is_dir() {
            files_under(root, &path, out)?;
        } else if kind.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .into_owned();
            out.push((rel, std::fs::read(&path).map_err(|e| e.to_string())?));
        }
    }
    Ok(())
}

/// Signs the staged tree `dir` with the key in `key_file` at `counter`.
fn sign_tree(key_file: &Path, dir: &Path, counter: &str) -> Result<(), String> {
    use ronoctua_pq::sign;
    let counter: u64 = counter
        .parse()
        .map_err(|_| format!("{counter}: not a counter"))?;
    let text = zeroize::Zeroizing::new(
        std::fs::read_to_string(key_file).map_err(|e| format!("{}: {e}", key_file.display()))?,
    );
    let key = sign::SigningKey::from_secret(&text).map_err(|r| r.to_string())?;
    let mut all = Vec::new();
    files_under(dir, dir, &mut all)?;
    let covered = |rel: &str| {
        (rel.starts_with("person/") || rel.starts_with("brain-modules/"))
            && !rel.starts_with("person/machines/")
    };
    let files = all
        .into_iter()
        .filter(|(rel, _)| covered(rel))
        .map(|(rel, bytes)| (rel, sign::sha256_hex(&bytes)))
        .collect();
    let manifest = sign::Manifest {
        format: sign::MANIFEST_FORMAT,
        counter,
        signed_at_s: 0,
        signer: String::new(),
        files,
    };
    let (json, signature) = sign::sign_manifest(&key, &manifest).map_err(|r| r.to_string())?;
    std::fs::write(dir.join("signed.json"), json).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("signed.sig"), signature).map_err(|e| e.to_string())
}

/// A recipient whose labels are dropped, so age lets it travel with any.
struct Unlabelled(Box<dyn age::Recipient>);

impl age::Recipient for Unlabelled {
    fn wrap_file_key(
        &self,
        file_key: &FileKey,
    ) -> Result<(Vec<Stanza>, HashSet<String>), age::EncryptError> {
        self.0
            .wrap_file_key(file_key)
            .map(|(stanzas, _)| (stanzas, HashSet::new()))
    }
}

fn mixed(pq: &str, classical: &str, path: &Path) -> Result<(), String> {
    let pq = Recipient::from_str(pq).map_err(|r| format!("the PQ recipient: {r}"))?;
    let classical = age::x25519::Recipient::from_str(classical)
        .map_err(|e| format!("the classical recipient: {e}"))?;
    let recipients = [Unlabelled(Box::new(pq)), Unlabelled(Box::new(classical))];
    let encryptor =
        age::Encryptor::with_recipients(recipients.iter().map(|r| r as &dyn age::Recipient))
            .map_err(|e| e.to_string())?;
    let file = new_private_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut writer = encryptor.wrap_output(file).map_err(|e| e.to_string())?;
    writer
        .write_all(b"sealed to two kinds at once\n")
        .map_err(|e| e.to_string())?;
    writer
        .finish()
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let done = match args.as_slice() {
        ["vectors"] => return vectors(),
        ["identity", file] => identity(Path::new(file)),
        ["debug", file] => debug(Path::new(file)),
        ["mixed", pq, classical, file] => mixed(pq, classical, Path::new(file)),
        ["signing-key", file] => signing_key(Path::new(file)),
        ["debug-signing", file] => debug_signing(Path::new(file)),
        ["sign-tree", key, dir, counter] => sign_tree(Path::new(key), Path::new(dir), counter),
        _ => Err(USAGE.to_owned()),
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("ronoctua-test-pq: {why}");
            ExitCode::FAILURE
        }
    }
}
