//! The session's view rebuilt from the parts that are here
//! ([`ronoctua_parts::view`] decides what it holds; this writes it).
//!
//! Built whole beside the old one and swapped in with one
//! `renameat2(RENAME_EXCHANGE)`, so the session's `PATH` never meets a view
//! that is missing or half made.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ronoctua_parts::exit;
use ronoctua_parts::names;
use ronoctua_parts::plan::Kind;
use ronoctua_parts::record::Phase;
use ronoctua_parts::view::{LINKED, links};
use rustix::fs::{CWD, RenameFlags};

use crate::disk;
use crate::{Usage, say};

/// How deep a part's linked folders are walked: icons are
/// `share/icons/<theme>/<size>/<context>/<name>`, and nothing linked is
/// deeper; a cycle of links ends here.
const DEPTH: usize = 8;

pub fn run() -> Result<u8, Usage> {
    rebuild();
    println!("view rebuilt");
    Ok(exit::DONE)
}

/// The view rebuilt; what goes wrong is said, and the old view stays.
pub fn rebuild() {
    let kinds: BTreeMap<String, Kind> = disk::read_plan(Path::new(names::PLAN))
        .map(|plan| plan.parts.into_iter().map(|(id, w)| (id, w.kind)).collect())
        .unwrap_or_default();
    let mut parts = BTreeMap::new();
    for record in disk::read_records() {
        let Some(current) = &record.current else {
            continue;
        };
        if record.phase == Phase::Unwanted || kinds.get(&record.id) != Some(&Kind::Program) {
            continue;
        }
        let profile = names::profile(&record.id, current.interface);
        let mut files = Vec::new();
        for dir in LINKED {
            walk(&profile.join(dir), dir, DEPTH, &mut files);
        }
        parts.insert(
            record.id.clone(),
            (profile.to_string_lossy().into_owned(), files),
        );
    }
    let (links, collisions) = links(&parts);
    for c in &collisions {
        say(
            4,
            &format!(
                "{} is offered by {} and {}: {}'s is in the view",
                c.at, c.kept, c.left, c.kept
            ),
        );
    }
    if let Err(e) = write(&links) {
        say(3, &format!("the view was not rebuilt: {e}"));
    }
}

/// Every file under `path`, as `rel/…`, following links to folders.
fn walk(path: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if !meta.is_dir() {
        out.push(rel.to_owned());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .collect();
    names.sort();
    for name in names {
        walk(&path.join(&name), &format!("{rel}/{name}"), depth - 1, out);
    }
}

fn write(links: &[ronoctua_parts::view::Link]) -> Result<(), String> {
    let state = Path::new(names::STATE_DIR);
    disk::make_dir(state, 0o755)?;
    let view = PathBuf::from(names::VIEW);
    let next = state.join(format!(".view.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&next);
    let made = (|| -> std::io::Result<()> {
        std::fs::create_dir(&next)?;
        // `bin/` and `share/` always exist, empty or not: the session's
        // `PATH` names them from the first boot on.
        std::fs::create_dir_all(next.join("bin"))?;
        std::fs::create_dir_all(next.join("share"))?;
        for link in links {
            let at = next.join(&link.at);
            if let Some(parent) = at.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(&link.to, &at)?;
        }
        Ok(())
    })();
    if let Err(e) = made {
        let _ = std::fs::remove_dir_all(&next);
        return Err(format!("{}: {e}", next.display()));
    }
    let swapped = if view.exists() {
        rustix::fs::renameat_with(CWD, &next, CWD, &view, RenameFlags::EXCHANGE)
            .map_err(|e| format!("{}: {e}", view.display()))
    } else {
        std::fs::rename(&next, &view).map_err(|e| format!("{}: {e}", view.display()))
    };
    // After an exchange `next` holds the old view; after a rename, nothing.
    let _ = std::fs::remove_dir_all(&next);
    swapped
}
