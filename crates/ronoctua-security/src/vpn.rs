//! The person's WireGuard key and locations, staged for root.
//!
//! # One key, one gesture
//!
//! The importer (`nix/scripts/vpn-import.py`, run AS THE PERSON) reads a
//! folder of WireGuard `.conf` files, asks the private key once, and writes
//! two files into a runtime folder of the person's own
//! ([`STAGING_DIR`], `0700`, made by `nix/modules/vpn.nix`'s tmpfiles): the
//! key ([`KEY_FILE`], `0600`, the bare base64) and the locations
//! ([`LOCATIONS_FILE`], the JSON the sync service reads). Then it starts
//! `ronoctua-vpn-install.service` through the one narrow polkit rule — the
//! same shape tor and the parts realizers use, never the sudo door, which is
//! the shell's alone.
//!
//! # Root's half
//!
//! This is what that unit runs ([`install`], a [`crate::verb::UnitVerb`], so
//! the door never calls it). It takes the two files the way the binary cache's
//! netrc is taken (`ronoctua_cachix::netrc::take_staged`): the folder opened
//! as a folder and never through a link, and the person's; each file
//! `O_NOFOLLOW`, a regular file of the person's, `0600`, one link, size-capped.
//! Whatever the key file holds is checked as a WireGuard key. Then it seals the
//! key in the TPM ([`ronoctua_network::secret::seal_wireguard`]) and writes the
//! locations where the sync service reads them ([`LOCATIONS`]). The key is
//! never a word of any verb, and never touches `argv`, the store or the repo.

use std::io::Read as _;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

use rustix::fs::{AtFlags, FileType, Mode, OFlags};

use crate::{Refusal, Why, refused};

/// Where the person stages the two files — a folder of their own.
pub const STAGING_DIR: &str = "/run/ronoctua-vpn";

/// The staged key's name: the bare base64, `0600`.
pub const KEY_FILE: &str = "private.key";

/// The staged locations' name: the JSON the importer wrote, `0600`.
pub const LOCATIONS_FILE: &str = "locations.json";

/// Where root keeps the locations for the sync service.
pub const INSTALLED_DIR: &str = "/var/lib/ronoctua-vpn";

/// The locations file the sync service reads (`nix/modules/vpn.nix`).
pub const LOCATIONS: &str = "/var/lib/ronoctua-vpn/locations.json";

/// The most bytes read from the staged key: a WireGuard key is 44 base64
/// characters, and anything near this is not one.
const KEY_MOST: usize = 128;

/// The most bytes read from the staged locations: every Surfshark server is a
/// few tens of kilobytes; a megabyte is far past any real list and caps a file
/// swapped for a huge one.
const LOCATIONS_MOST: usize = 1 << 20;

/// Whether `text` is a WireGuard private key: Curve25519 is 32 bytes, which is
/// 44 base64 characters ending in `=`. A space, a quote, a line break —
/// anything a keyfile would read as more than the key — is refused. Pure.
#[must_use]
pub fn is_wireguard_key(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 44
        && b.ends_with(b"=")
        && b[..43]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/'))
}

/// Takes the key and the locations the person `uid` staged, seals the key in
/// the TPM and writes the locations for the sync service. The staged copies
/// are spent (unlinked) whatever becomes of them.
///
/// # Errors
///
/// [`Why::NotAllowed`] when the folder or a file is not the person's own
/// private file; [`Why::Invalid`] when nothing is staged, the key is not one,
/// or the locations are not the JSON this distro reads; [`Why::Failed`] when
/// the TPM refused or the locations could not be written.
pub fn install(uid: u32) -> Result<(), Refusal> {
    let (key, locations) = take_staged(Path::new(STAGING_DIR), uid)?;

    // SEALED FIRST: if the TPM refuses, no locations are written, so no profile
    // is generated that the agent could never key. The reverse is harmless —
    // a key sealed with no locations is a key nothing asks for.
    ronoctua_network::secret::seal_wireguard(&key).map_err(|e| {
        eprintln!("<3>the WireGuard key could not be sealed: {e:#}");
        refused(Why::Failed)
    })?;

    write_locations(&locations)
}

/// Opens the staging folder and takes both files from it. See the module
/// header for the checks; they are the binary cache's, file for file.
fn take_staged(dir: &Path, uid: u32) -> Result<(String, Vec<u8>), Refusal> {
    let folder = rustix::fs::open(
        dir,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| {
        if e == rustix::io::Errno::NOENT {
            eprintln!("<3>nothing staged at {STAGING_DIR}");
            refused(Why::Invalid)
        } else {
            eprintln!("<3>{STAGING_DIR} is not a folder to take from");
            refused(Why::NotAllowed)
        }
    })?;
    let at = rustix::fs::fstat(&folder).map_err(|_| refused(Why::NotAllowed))?;
    if at.st_uid != uid {
        eprintln!("<3>{STAGING_DIR} is not the person's folder");
        return Err(refused(Why::NotAllowed));
    }

    let key_bytes = take_file(&folder, KEY_FILE, uid, KEY_MOST)?;
    let key = String::from_utf8(key_bytes).map_err(|_| refused(Why::Invalid))?;
    let key = key.trim().to_owned();
    if !is_wireguard_key(&key) {
        eprintln!("<3>the staged key is not a WireGuard key");
        return Err(refused(Why::Invalid));
    }

    let locations = take_file(&folder, LOCATIONS_FILE, uid, LOCATIONS_MOST)?;
    if !locations_are_read(&locations) {
        eprintln!("<3>the staged locations are not the JSON this distro reads");
        return Err(refused(Why::Invalid));
    }
    Ok((key, locations))
}

/// Opens `name` inside `folder` with `O_NOFOLLOW`, checks it is a regular
/// file of `uid`'s with no bits for anybody else and one link, spends it
/// (unlinks it), and reads up to `most` bytes.
fn take_file(
    folder: &std::os::fd::OwnedFd,
    name: &str,
    uid: u32,
    most: usize,
) -> Result<Vec<u8>, Refusal> {
    let file = match rustix::fs::openat(
        folder,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(f) => f,
        Err(rustix::io::Errno::NOENT) => {
            eprintln!("<3>{name} is not staged");
            return Err(refused(Why::Invalid));
        }
        Err(_) => {
            eprintln!("<3>{name} is not a file to take");
            return Err(refused(Why::NotAllowed));
        }
    };
    let stat = rustix::fs::fstat(&file).map_err(|_| refused(Why::NotAllowed))?;
    let size = usize::try_from(stat.st_size).unwrap_or(usize::MAX);
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != uid
        || stat.st_mode & 0o077 != 0
        || stat.st_nlink != 1
        || size > most
    {
        eprintln!("<3>{name} is not the person's own private file");
        return Err(refused(Why::NotAllowed));
    }
    // SPENT: whatever it holds, the copy goes now.
    let _ = rustix::fs::unlinkat(folder, name, AtFlags::empty());
    let mut bytes = Vec::with_capacity(size.min(most));
    std::fs::File::from(file)
        .take(u64::try_from(most + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| refused(Why::NotAllowed))?;
    Ok(bytes)
}

/// Whether `bytes` is a locations file this distro reads: valid JSON, format
/// 1, a list of locations. The sync service reads the fields; here it is only
/// that the file is the shape, so a truncated or alien file is refused before
/// it reaches `/var/lib`. Pure.
#[must_use]
fn locations_are_read(bytes: &[u8]) -> bool {
    let Ok(doc) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return false;
    };
    doc.get("format").and_then(serde_json::Value::as_u64) == Some(1)
        && doc
            .get("locations")
            .is_some_and(serde_json::Value::is_array)
}

/// Writes the locations at [`LOCATIONS`], `0644` (the sync service reads them;
/// they name no secret), the old one replaced. The directory is root's own
/// (`nix/modules/vpn.nix`'s tmpfiles), made here too for a first run.
fn write_locations(bytes: &[u8]) -> Result<(), Refusal> {
    std::fs::create_dir_all(INSTALLED_DIR).map_err(|e| {
        eprintln!("<3>{INSTALLED_DIR} could not be made: {e}");
        refused(Why::Failed)
    })?;
    let path: PathBuf = Path::new(INSTALLED_DIR).join("locations.json.new");
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .open(&path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| {
            eprintln!("<3>{} could not be written: {e}", path.display());
            refused(Why::Failed)
        })?;
    // RENAMED INTO PLACE, so the sync service's path unit never reads a
    // half-written file.
    std::fs::rename(&path, LOCATIONS).map_err(|e| {
        eprintln!("<3>{LOCATIONS} could not be put in place: {e}");
        refused(Why::Failed)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WireGuard key's shape, with nothing real in it (invariant 13): 44
    /// base64 characters ending in `=`.
    const KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA0=";

    #[test]
    fn a_key_is_44_base64_characters_and_nothing_else() {
        assert_eq!(KEY.len(), 44);
        assert!(is_wireguard_key(KEY));
        for bad in [
            "",
            "short=",
            &"A".repeat(43),             // no `=`
            &format!("{}B", &KEY[..43]), // 44 but no `=`
            &format!("{}\n", KEY),       // a line break
            "AAAA AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==", // 45
        ] {
            assert!(!is_wireguard_key(bad), "{bad:?}");
        }
    }

    #[test]
    fn the_locations_must_be_the_shape_before_they_reach_var_lib() {
        assert!(locations_are_read(br#"{"format":1,"locations":[]}"#));
        assert!(locations_are_read(
            br#"{"format":1,"provider":"surfshark","locations":[{"id":"us-nyc"}]}"#
        ));
        for bad in [
            &b"not json"[..],
            br#"{"format":2,"locations":[]}"#,
            br#"{"locations":[]}"#,
            br#"{"format":1}"#,
            br#"{"format":1,"locations":{}}"#,
        ] {
            assert!(!locations_are_read(bad), "{:?}", std::str::from_utf8(bad));
        }
    }
}
