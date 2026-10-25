//! `accept`: what the person handed in, taken only when it IS the download.
//!
//! The person puts a file they downloaded elsewhere — a browser, another
//! computer, a phone — at `<HANDIN>/<part>/<the download's name>`, the name
//! the parts' page says, and starts `ronoctua-parts-accept.service`. For each
//! file there, root:
//!
//! 1. refuses a link or anything that is not a plain file, at either level —
//!    the folder is the person's, and root follows nothing it put there;
//! 2. takes it only for a download the part's record says is missing;
//! 3. copies it into a folder of root's (`0600`), so what Nix reads is what
//!    was checked, never a file the person can still change;
//! 4. adds it with `nix store prefetch-file --expected-hash`, under the
//!    download's own name: Nix checks the hash the flake pins, and the path
//!    that comes out must be the download's output — the same bytes, so the
//!    same path, or nothing;
//! 5. removes what was handed in, and starts the part's realizer.
//!
//! A refusal is said in the part's record (`hand-in-refused`), never a
//! silence.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use ronoctua_parts::exit;
use ronoctua_parts::frontier::{Download, Mode};
use ronoctua_parts::names;

use crate::disk::{self, LOCK_WAIT, Lock};
use crate::{Usage, program, say};

pub fn run(args: &[String]) -> Result<u8, Usage> {
    if !args.is_empty() {
        return Err(Usage);
    }
    let Ok(parts) = fs::read_dir(names::HANDIN) else {
        println!("accepted 0");
        return Ok(exit::DONE);
    };
    let mut accepted = 0;
    let mut touched = Vec::new();
    for part in parts.filter_map(Result::ok) {
        let Some(id) = part.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !ronoctua_parts::id::is_id(&id) || !is_plain_dir(&part.path()) {
            say(
                4,
                &format!("{}: not a part's folder, left alone", part.path().display()),
            );
            continue;
        }
        let Ok(files) = fs::read_dir(part.path()) else {
            continue;
        };
        for file in files.filter_map(Result::ok) {
            let path = file.path();
            let name = file.file_name().to_string_lossy().into_owned();
            match take(&id, &name, &path) {
                Taken::Accepted => {
                    accepted += 1;
                    touched.push(id.clone());
                }
                Taken::Refused(why) => {
                    say(4, &format!("{id}: {name} was not taken: {why}"));
                    let _ = say_in_record(&id, &format!("{name}: {why}"));
                }
            }
            let _ = fs::remove_file(&path);
        }
    }
    touched.sort();
    touched.dedup();
    let systemctl = program("RONOCTUA_SYSTEMCTL");
    for id in &touched {
        let unit = names::realizer_unit(id);
        if !disk::run_quiet(&[
            systemctl.clone(),
            "start".to_owned(),
            "--no-block".to_owned(),
            unit,
        ]) {
            say(4, &format!("{id}: its realizer did not start"));
        }
    }
    println!("accepted {accepted}");
    Ok(exit::DONE)
}

enum Taken {
    Accepted,
    Refused(String),
}

fn take(id: &str, name: &str, path: &Path) -> Taken {
    if !is_plain_file(path) {
        return Taken::Refused("it is a link or not a plain file".to_owned());
    }
    let Ok(record) = disk::read_record(id) else {
        return Taken::Refused("the part has no record".to_owned());
    };
    let Some(download) = record.missing.iter().find(|d| d.name == name).cloned() else {
        return Taken::Refused("no download of the part is missing under that name".to_owned());
    };
    if download.urls.is_empty() {
        return Taken::Refused("this download is no single file: it comes by a stick".to_owned());
    }
    let copy = match copy_in(path, name) {
        Ok(copy) => copy,
        Err(e) => return Taken::Refused(format!("it could not be read: {e}")),
    };
    let added = add(&download, &copy);
    let _ = fs::remove_file(&copy);
    match added {
        Ok(out) if out == download.out => Taken::Accepted,
        Ok(out) => Taken::Refused(format!("it became {out}, not {}", download.out)),
        Err(e) => Taken::Refused(e),
    }
}

/// Whether `path` is a folder and not a link to one.
fn is_plain_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

/// Whether `path` is a plain file and not a link to one.
fn is_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file())
}

/// The handed file copied into root's own folder, `0600`, opened with
/// `O_NOFOLLOW`: what Nix reads is what root read.
fn copy_in(path: &Path, name: &str) -> std::io::Result<std::path::PathBuf> {
    let folder = Path::new(names::ACCEPTING);
    fs::create_dir_all(folder)?;
    fs::set_permissions(folder, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    let nofollow = i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits()).unwrap_or(0);
    let mut from = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nofollow)
        .open(path)?;
    let copy = folder.join(name);
    let _ = fs::remove_file(&copy);
    let mut to = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nofollow)
        .open(&copy)?;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = from.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        to.write_all(&buffer[..n])?;
    }
    to.sync_all()?;
    Ok(copy)
}

/// The copy added under the download's name and pinned hash; the store path
/// it became.
fn add(download: &Download, copy: &Path) -> Result<String, String> {
    let said = disk::run(&ronoctua_parts::argv::prefetch(
        &program("RONOCTUA_NIX"),
        &download.name,
        &download.hash,
        download.mode == Mode::Recursive,
        &format!("file://{}", copy.display()),
    ));
    if !said.ok {
        return Err("its hash is not the one the flake pins".to_owned());
    }
    store_path_of(&said.out)
}

/// The store path `nix store prefetch-file --json` named.
pub fn store_path_of(json: &str) -> Result<String, String> {
    let json: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "nix answered nothing it could read".to_owned())?;
    json.get("storePath")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "nix named no store path".to_owned())
}

/// A refusal said in the part's record.
fn say_in_record(id: &str, detail: &str) -> Result<(), String> {
    let _lock = Lock::take(LOCK_WAIT)?;
    let record = disk::read_record(id)?;
    disk::write_record(&record.refused_hand_in(detail, disk::now()))
}
