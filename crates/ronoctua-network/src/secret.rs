//! The Wi-Fi password sealed in the TPM2.
//!
//! # What this solves, and what it does not
//!
//! Everyone's way is `psk=<password>` inside NetworkManager's keyfile,
//! mode `0600`. In this distro that already lives on a LUKS volume,
//! so it is not quite "a plaintext password on disk" — what is left of exposure
//! is narrow and concrete: **a copy that leaves the encrypted volume**.
//! `/etc/NetworkManager/system-connections` is on `persist.nix`'s list,
//! and any backup of `/persist` would carry the passwords with it.
//!
//! It is against that that this module exists. It does **not** protect against root on a
//! running machine: the password reaches `wpa_supplicant` in RAM either way,
//! and no design avoids that.
//!
//! # Why NO PCR policy
//!
//! `systemd-creds` can bind the secret to boot measurements. Binding to
//! PCR 11 — which measures the UKI — would mean **every kernel update
//! silently breaking Wi-Fi**, and would buy almost nothing: the blob already lives
//! inside a volume the same PCRs protect. With no policy, the blob
//! stays useless on any other machine, which is the target, and survives an
//! update.
//!
//! It is also what lets the installer seal **from inside the ISO**: the TPM is the
//! same, and no PCR of the final system needs to exist yet.
//!
//! # `--with-key=tpm2`, and not `auto`
//!
//! `auto` mixes in the host key (`/var/lib/systemd/credential.secret`),
//! which on the target does not exist yet when the installer seals. The blob would open on
//! the machine that created it and on no other — including on the same machine once
//! installed.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Where the blobs live.
///
/// `/persist` because `nix/modules/persist.nix` routes it out of the root
/// wipe. A blob in `/etc` would survive until the first boot and vanish on the
/// second — and the symptom would be "the Wi-Fi forgot the password on its own".
pub const DIRECTORY: &str = "/persist/etc/ronoctua/wifi";

/// The binary, by store path.
///
/// Never by name: inside a systemd unit `PATH` is minimal, and the failure
/// would show up far from the cause — a trap already recorded in AGENTS.md.
const SYSTEMD_CREDS_PATH: &str = match option_env!("RONOCTUA_SYSTEMD_CREDS") {
    Some(c) => c,
    None => "systemd-creds",
};

/// The SSID in hexadecimal, which is the blob's name.
///
/// # Why hex, and not the raw SSID nor a digest
///
/// The standard allows **any byte** in an SSID: a slash, a dot-dot, a byte that
/// is not UTF-8. Using the raw name would give path traversal and a file
/// impossible to create.
///
/// And hex instead of sha256 for two reasons: it brings no new dependency, and it is
/// **reversible** — one can list what is stored and know which network it belongs to.
/// The SSID is not a secret: it goes over the air in the clear on every beacon.
#[must_use]
pub fn name(ssid: &str) -> String {
    use std::fmt::Write as _;
    ssid.as_bytes().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The path of a network's blob, in any given directory.
///
/// The directory is a parameter because of the **installer**: it seals for the
/// target system, which is mounted at `/mnt`. Hard-coding `DIRECTORY` would make the
/// sealing write into the ISO — which is volatile — and the installed system would be born
/// with no password at all, with nothing to flag it.
#[must_use]
pub fn path_at(directory: &str, ssid: &str) -> PathBuf {
    Path::new(directory).join(format!("{}.cred", name(ssid)))
}

/// The path of a network's blob, in the default place.
#[must_use]
pub fn path(ssid: &str) -> PathBuf {
    path_at(DIRECTORY, ssid)
}

/// Is there already a stored password for this network?
#[must_use]
pub fn ha(ssid: &str) -> bool {
    path(ssid).is_file()
}

/// Seals the password into the TPM and writes the blob.
///
/// The password comes in through **stdin**, never through `argv`: `/proc/<pid>/cmdline` is
/// readable by any process on the machine, and an argument would be exposed
/// for as long as the command ran.
///
/// # Errors
///
/// There is no TPM, `systemd-creds` refused, or the directory could not be created.
pub fn seal(ssid: &str, password: &str) -> Result<()> {
    seal_at(DIRECTORY, ssid, password)
}

/// Seals into a given directory. See [`path_at`] for why the parameter.
///
/// # Errors
///
/// There is no TPM, `systemd-creds` refused, or the directory could not be created.
pub fn seal_at(directory: &str, ssid: &str, password: &str) -> Result<()> {
    let destination = path_at(directory, ssid);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    let mut child = Command::new(SYSTEMD_CREDS_PATH)
        .arg("encrypt")
        .arg("--with-key=tpm2")
        // THE NAME IS AUTHENTICATED along with the content: a blob sealed with one
        // name only opens when the same name is asked for. It is what prevents swapping one
        // network's file for another's and having the machine connect to the wrong one.
        .arg(format!("--name={}", name(ssid)))
        .arg("-")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("run {SYSTEMD_CREDS_PATH}"))?;

    child
        .stdin
        .take()
        .context("no stdin for systemd-creds")?
        .write_all(password.as_bytes())
        .context("send the password to systemd-creds")?;

    let output = child.wait_with_output().context("wait for systemd-creds")?;
    if !output.status.success() {
        // `systemd-creds`'s stderr says whether the TPM is absent, busy or
        // refusing. Passing it along is the difference between "it did not work" and knowing why.
        tracing::warn!(
            said = %String::from_utf8_lossy(&output.stderr).trim(),
            "the TPM refused to seal"
        );
        return Err(crate::refusal::Named(crate::refusal::Refusal::Vault).into());
    }

    write_restricted(&destination, &output.stdout)
        .with_context(|| format!("write {}", destination.display()))
}

/// Opens the blob and returns the password.
///
/// # Errors
///
/// There is no blob, the TPM is not the same one, or `systemd-creds` refused.
pub fn open(ssid: &str) -> Result<String> {
    open_at(DIRECTORY, ssid)
}

/// Opens a blob of a given directory (see [`seal_at`]): the first boot's
/// sealed passphrase lives beside its record, not with the networks.
///
/// # Errors
///
/// There is no blob, the TPM is not the same one, or `systemd-creds` refused.
pub fn open_at(directory: &str, ssid: &str) -> Result<String> {
    let origin = path_at(directory, ssid);
    let blob = std::fs::read(&origin).with_context(|| format!("read {}", origin.display()))?;

    let mut child = Command::new(SYSTEMD_CREDS_PATH)
        .arg("decrypt")
        .arg(format!("--name={}", name(ssid)))
        .arg("-")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("run {SYSTEMD_CREDS_PATH}"))?;

    child
        .stdin
        .take()
        .context("no stdin for systemd-creds")?
        .write_all(&blob)
        .context("send the blob to systemd-creds")?;

    let output = child.wait_with_output().context("wait for systemd-creds")?;
    if !output.status.success() {
        tracing::warn!(
            said = %String::from_utf8_lossy(&output.stderr).trim(),
            "the TPM refused to unseal"
        );
        return Err(crate::refusal::Named(crate::refusal::Refusal::Vault).into());
    }

    // The password may contain any byte; `from_utf8_lossy` would swap an invalid
    // byte for `U+FFFD` and the network would refuse the connection without saying why.
    String::from_utf8(output.stdout).context("the stored password is not valid text")
}

/// Deletes a network's blob. Silent when there was none.
///
/// # Errors
///
/// The file exists and could not be removed.
pub fn erase(ssid: &str) -> Result<()> {
    let target = path(ssid);
    match std::fs::remove_file(&target) {
        Ok(()) => Ok(()),
        // Forgetting a network that was never remembered is not an error: it is the result
        // that was asked for.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("delete {}", target.display())),
    }
}

/// Writes with `0600` **from creation**.
///
/// `write` and then `set_permissions` would leave a window in which the file
/// exists readable per the umask — short, and still a window. Here the mode
/// goes into the `open`.
fn write_restricted(destination: &Path, data_bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(destination)?;
    f.write_all(data_bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_is_the_ssid_in_hex() {
        assert_eq!(name("a house"), "6120686f757365");
        assert_eq!(name(""), "");
    }

    #[test]
    fn an_ssid_with_a_slash_does_not_escape_the_directory() {
        // The case that motivated not using the raw name: the standard allows any
        // byte in an SSID, and `../../etc/passwd` as a network name would write
        // outside the directory. In hex there is neither slash nor dot.
        let c = path("../../etc/passwd");
        assert!(c.starts_with(DIRECTORY), "it escaped: {}", c.display());
        let file = c.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!file.contains('/'), "a slash was left over: {file}");
        assert!(!file.contains(".."), "a dot-dot was left over: {file}");
    }

    #[test]
    fn an_ssid_that_is_not_valid_utf8_still_becomes_a_name() {
        // A router with a malformed SSID exists. The crate's `ssid_name` already
        // uses `from_utf8_lossy`, so what arrives here may contain `U+FFFD` —
        // and it still has to become a creatable file name.
        let foreign = String::from_utf8_lossy(&[0xff, 0xfe, b'a']).into_owned();
        let n = name(&foreign);
        assert!(!n.is_empty());
        assert!(n.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn two_different_networks_do_not_share_the_same_blob() {
        assert_ne!(name("home"), name("home "));
        assert_ne!(path("home"), path("Home"));
    }
}
