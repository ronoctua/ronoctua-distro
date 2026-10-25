//! The downloads carried by a stick between two machines.
//!
//! `export` (root, `ronoctua-parts-export.service`, started from the parts'
//! page) writes the downloads this machine keeps — for its parts, and those
//! its system was built from (`ronoctua-machine`'s kept downloads) — into
//! `<folder>/ronoctua-parts`, a binary cache Nix reads by `file://`, and gives
//! what it wrote to the folder's owner. `import` (root,
//! `ronoctua-parts-import.service`) copies in, from such a folder, the
//! downloads a part is missing, and those the machine's last apply could not
//! fetch — and ONLY downloads: a download's path is its
//! content, so Nix takes it with no signature, and anything built is refused
//! by Nix itself, since nothing here passes `--no-check-sigs`. A built program
//! from another machine would be that machine's code running here; the
//! person's own binary cache, with its key, is the way for those.
//!
//! The person names the folder in a file of theirs
//! (`/run/ronoctua-parts/{import/from,export/to}`); root goes only to a
//! folder on a removable medium, reached with no link on the way. How each
//! gesture ended is written for the page ([`ronoctua_parts::stick::Said`]).

use std::fs;
use std::path::{Path, PathBuf};

use ronoctua_parts::exit;
use ronoctua_parts::names;
use ronoctua_parts::stick::{Outcome, Refusal, Said, Verb};
use ronoctua_person::frontier::Download;

use crate::disk;
use crate::{Usage, program, say};

pub fn export(args: &[String]) -> Result<u8, Usage> {
    if !args.is_empty() {
        return Err(Usage);
    }
    let to = match named_folder(names::EXPORT_TO) {
        Ok(to) => to,
        Err(why) => return Ok(refused(Verb::Export, why)),
    };
    let folder = to.join(names::STICK_FOLDER);
    let mut paths: Vec<String> = kept_paths();
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Ok(done(Verb::Export, 0));
    }
    let mut argv = vec![
        program("RONOCTUA_NIX"),
        "copy".to_owned(),
        "--to".to_owned(),
        format!("file://{}", folder.display()),
    ];
    argv.extend(paths.iter().cloned());
    if !disk::run(&argv).ok {
        say(
            3,
            &format!("the downloads were not written to {}", folder.display()),
        );
        return Ok(refused(Verb::Export, Refusal::Unwritable));
    }
    // WHAT ROOT WROTE IS THE FOLDER OWNER'S: on a stick that keeps owners
    // (ext4), the person could not otherwise take it away again.
    if let Err(e) = give_to_the_owner(&to, &folder) {
        say(
            4,
            &format!("{}: not given to its owner: {e}", folder.display()),
        );
    }
    Ok(done(
        Verb::Export,
        u32::try_from(paths.len()).unwrap_or(u32::MAX),
    ))
}

/// Gives `tree` — what root wrote — to the owner of `of`, link by link
/// (`lchown`: a link is never followed).
fn give_to_the_owner(of: &Path, tree: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let owner = fs::symlink_metadata(of)?;
    let (uid, gid) = (Some(owner.uid()), Some(owner.gid()));
    let mut todo = vec![tree.to_path_buf()];
    while let Some(path) = todo.pop() {
        let meta = fs::symlink_metadata(&path)?;
        std::os::unix::fs::lchown(&path, uid, gid)?;
        if meta.file_type().is_dir() {
            for entry in fs::read_dir(&path)? {
                todo.push(entry?.path());
            }
        }
    }
    Ok(())
}

/// Every download this machine keeps: for a part that is here (the targets
/// of the links under [`names::KEPT`]), and the system's (under the
/// machine's record, one folder per generation of the set).
fn kept_paths() -> Vec<String> {
    let system =
        Path::new(ronoctua_machine::STATE_DIR).join(ronoctua_machine::build::DOWNLOADS_DIR);
    [Path::new(names::KEPT), system.as_path()]
        .into_iter()
        .filter_map(|folder| fs::read_dir(folder).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|kept| fs::read_dir(kept.path()).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|link| fs::read_link(link.path()).ok())
        .filter_map(|target| target.to_str().map(str::to_owned))
        .filter(|t| ronoctua_parts::plan::is_store_path(t))
        .collect()
}

pub fn import(args: &[String]) -> Result<u8, Usage> {
    if !args.is_empty() {
        return Err(Usage);
    }
    let from = match named_folder(names::IMPORT_FROM) {
        Ok(from) => from,
        Err(why) => return Ok(refused(Verb::Import, why)),
    };
    if !from
        .join(names::STICK_FOLDER)
        .join("nix-cache-info")
        .is_file()
    {
        return Ok(refused(Verb::Import, Refusal::NoDownloadsThere));
    }
    let cache = format!("file://{}", from.join(names::STICK_FOLDER).display());
    let nix = program("RONOCTUA_NIX");
    // One download brought in, when the stick has it: whether it came.
    let bring = |owner: &str, download: &Download| -> bool {
        let there = disk::run_quiet(&[
            nix.clone(),
            "path-info".to_owned(),
            "--store".to_owned(),
            cache.clone(),
            download.out.clone(),
        ]);
        if !there {
            return false;
        }
        // NO `--no-check-sigs`: a download is content-addressed and needs
        // none; anything else on the stick is refused by Nix.
        let ok = disk::run_quiet(&[
            nix.clone(),
            "copy".to_owned(),
            "--from".to_owned(),
            cache.clone(),
            download.out.clone(),
        ]);
        if !ok {
            say(
                4,
                &format!(
                    "{owner}: {} was on the stick and was refused",
                    download.name
                ),
            );
        }
        ok
    };
    let mut copied: u32 = 0;
    let mut touched = Vec::new();
    for record in disk::read_records() {
        for download in &record.missing {
            if bring(&record.id, download) {
                copied += 1;
                touched.push(record.id.clone());
            }
        }
    }
    // THE SYSTEM'S, which wait in the store for the next update.
    match disk::system_missing() {
        Ok(missing) => {
            for download in &missing {
                if bring(names::SYSTEM, download) {
                    copied += 1;
                }
            }
        }
        Err(e) => say(6, &format!("the system's downloads were not asked: {e}")),
    }
    touched.sort();
    touched.dedup();
    let systemctl = program("RONOCTUA_SYSTEMCTL");
    for id in &touched {
        let _ = disk::run_quiet(&[
            systemctl.clone(),
            "start".to_owned(),
            "--no-block".to_owned(),
            names::realizer_unit(id),
        ]);
    }
    Ok(done(Verb::Import, copied))
}

/// The folder the person named in `file`: an absolute path under a
/// removable medium's mount (`/run/media/<user>/…`), reached with no link on
/// the way — its canonical form is itself.
fn named_folder(file: &str) -> Result<PathBuf, Refusal> {
    let named = fs::read_to_string(file).map_err(|_| Refusal::NoFolderNamed)?;
    let named = PathBuf::from(named.trim());
    let canonical = fs::canonicalize(&named).map_err(|_| Refusal::FolderMissing)?;
    if canonical != named {
        return Err(Refusal::FolderThroughALink);
    }
    if !on_a_removable_medium(&canonical) || !canonical.is_dir() {
        return Err(Refusal::NotARemovableMedium);
    }
    Ok(canonical)
}

/// Whether `path` is a folder under `/run/media/<user>/<medium>`. Pure.
fn on_a_removable_medium(path: &Path) -> bool {
    let mut parts = path.components();
    let root = parts.next().map(|c| c.as_os_str().to_owned());
    let run = parts.next().map(|c| c.as_os_str().to_owned());
    let media = parts.next().map(|c| c.as_os_str().to_owned());
    let (user, medium) = (parts.next(), parts.next());
    root.as_deref() == Some(std::ffi::OsStr::new("/"))
        && run.as_deref() == Some(std::ffi::OsStr::new("run"))
        && media.as_deref() == Some(std::ffi::OsStr::new("media"))
        && user.is_some()
        && medium.is_some()
}

/// A gesture that moved `count` downloads: said for the page, and on
/// standard output.
fn done(verb: Verb, count: u32) -> u8 {
    tell(verb, Outcome::Done { count });
    println!(
        "{} {count}",
        match verb {
            Verb::Import => "imported",
            Verb::Export => "exported",
        }
    );
    exit::DONE
}

/// A gesture refused: said for the page, in the journal, and on standard
/// output.
fn refused(verb: Verb, why: Refusal) -> u8 {
    say(3, &format!("nothing was moved: {}", why.id()));
    tell(verb, Outcome::Refused { why });
    println!("refused {}", why.id());
    exit::FAILED
}

/// How the gesture ended, written whole for the page (`0644`).
fn tell(verb: Verb, outcome: Outcome) {
    let said = Said {
        format: ronoctua_person::FORMAT,
        verb,
        at: disk::now(),
        outcome,
    };
    let path = Path::new(names::STICK_SAID);
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return;
    };
    if let Err(e) = disk::write_whole(dir, name, ronoctua_person::to_json(&said).as_bytes(), 0o644)
    {
        say(4, &format!("how the gesture ended was not written: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: root would go to a folder that is not on a removable
    /// medium — the person's home, `/etc` — or refuses one that is.
    #[test]
    fn only_a_removable_medium_is_a_stick() {
        assert!(on_a_removable_medium(Path::new("/run/media/noctua/STICK")));
        assert!(on_a_removable_medium(Path::new(
            "/run/media/noctua/STICK/deep"
        )));
        assert!(!on_a_removable_medium(Path::new("/run/media/noctua")));
        assert!(!on_a_removable_medium(Path::new("/home/noctua")));
        assert!(!on_a_removable_medium(Path::new("/run/mediax/a/b")));
        assert!(!on_a_removable_medium(Path::new("/etc")));
    }
}
