//! The predefined targets, carried (feature `crypt`): what a door asked
//! (`requests.json`) applied to the index, the folders of a private target
//! remembered and restored, and what the files a sync brought offer
//! ([`crate::kept::Target`], [`crate::requests`], [`crate::offers`]).
//!
//! # "Not on any machine"
//!
//! The sync that reads a `DropEverywhere(id)` writes `id` into the index's
//! `dropped` (who, when); its plan then takes every copy of the target out
//! of the repository (`Step::Unkeep`), and every other machine that syncs
//! after it finds the target dropped and FORGETS what it had of it
//! (`Step::Forget`): the files stay where they are, the base no longer names
//! them, and nothing of them is pushed again. `KeepEverywhere(id)` takes
//! `id` out of `dropped`, and every machine keeps the target again. A
//! request leaves the file once the index that settles it is pushed.
//!
//! # Private folders
//!
//! A private target's files travel with their owner's bits alone
//! (`kept::kept_mode`). Its folders do not travel at all — so the index
//! remembers their modes (`dirs`), as the machine that pushed last had
//! them, and a sync that brings a file into one sets the folders above it,
//! up to the target's own, to the owner's bits of what the index says —
//! `0700` when it says nothing. `~/.ssh` and `~/.gnupg` come back `0700` even
//! where something made them `0755` first.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt as _;

use super::Engine;
use crate::kept::{self, PRIVATE_DIR_MODE};
use crate::merge::{Dropped, Index};
use crate::requests::{Request, Requests};
use crate::status::Offer;

impl Engine<'_> {
    /// What a door asked of this sync.
    pub(super) fn requested(&self) -> Requests {
        Requests::read(&self.places.requests())
    }

    /// Applies `requests` to `index`: a target dropped by this machine now,
    /// or kept everywhere again.
    pub(super) fn apply_requests(&self, index: &mut Index, requests: &Requests) {
        for request in &requests.requests {
            match request {
                Request::DropEverywhere { target } if kept::target_by_id(target).is_some() => {
                    index
                        .dropped
                        .entry(target.clone())
                        .or_insert_with(|| Dropped {
                            by: self.machine.clone(),
                            at_s: self.now_s,
                        });
                }
                Request::KeepEverywhere { target } => {
                    index.dropped.remove(target);
                }
                _ => {}
            }
        }
    }

    /// Takes out of `requests.json` what the pushed `index` settled. Best
    /// effort: a request left there is applied again by the next sync, and
    /// applying it twice changes nothing.
    pub(super) fn settle_requests(&self, index: &Index) {
        let path = self.places.requests();
        let asked = Requests::read(&path);
        let dropped: BTreeSet<String> = index.dropped.keys().cloned().collect();
        let left = asked.unsettled(&dropped);
        if left != asked {
            let _ = left.write(&path);
        }
    }

    /// The folders of every private target `index` keeps a file of, with
    /// their modes: as they are HERE when they are here, as the index said
    /// before (`previous`) when they are not.
    pub(super) fn private_dirs(
        &self,
        index: &Index,
        previous: &BTreeMap<String, u32>,
    ) -> BTreeMap<String, u32> {
        let mut dirs = BTreeMap::new();
        for blob in &index.entries {
            for dir in private_folders_of(&blob.path) {
                if dirs.contains_key(&dir) {
                    continue;
                }
                let here = self
                    .places
                    .resolve(&dir)
                    .and_then(|p| std::fs::symlink_metadata(p).ok())
                    .filter(std::fs::Metadata::is_dir)
                    .map(|m| m.permissions().mode() & 0o7777);
                if let Some(mode) = here.or_else(|| previous.get(&dir).copied()) {
                    dirs.insert(dir, mode);
                }
            }
        }
        dirs
    }

    /// Sets the folders above every file of a private target in `landed`,
    /// up to the target's own, to their owner's bits of what `dirs` says —
    /// [`PRIVATE_DIR_MODE`] when it says nothing (or nothing the owner may
    /// enter). A link is never followed. Best effort.
    pub(super) fn restore_dirs(&self, landed: &[String], dirs: &BTreeMap<String, u32>) {
        let mut done = BTreeSet::new();
        for path in landed {
            for dir in private_folders_of(path) {
                if !done.insert(dir.clone()) {
                    continue;
                }
                let mode = dirs
                    .get(&dir)
                    .map(|m| m & 0o700)
                    .filter(|m| m & 0o500 == 0o500)
                    .unwrap_or(PRIVATE_DIR_MODE);
                let Some(at) = self.places.resolve(&dir) else {
                    continue;
                };
                let is_dir = std::fs::symlink_metadata(&at).is_ok_and(|m| m.is_dir());
                if is_dir {
                    let _ = std::fs::set_permissions(&at, std::fs::Permissions::from_mode(mode));
                }
            }
        }
    }

    /// What the files a sync brought (`landed`) offer.
    pub(super) fn offers_after(&self, landed: &[String]) -> Vec<Offer> {
        // GIT IS ASKED only when SSH signing could be offered at all.
        let signing = crate::offers::may_offer_signing(self.places, landed)
            .then(|| crate::offers::Signing::of(&self.tools, &self.places.home))
            .flatten();
        crate::offers::offered(self.places, landed, signing.as_ref())
    }
}

/// The folders of a private target above `path`, from the target's own
/// down — none for a path of no private target, or a target that is a
/// file. Pure.
fn private_folders_of(path: &str) -> Vec<String> {
    let Some(target) = kept::target_of(path).filter(|t| t.private) else {
        return Vec::new();
    };
    let Some(root) = target
        .paths
        .iter()
        .find(|p| crate::places::covers(p, path) && **p != path)
    else {
        return Vec::new();
    };
    let below = path.get(root.len()..).unwrap_or_default();
    let parts: Vec<&str> = below.split('/').filter(|c| !c.is_empty()).collect();
    let mut out = vec![(*root).to_owned()];
    let mut at = (*root).to_owned();
    // EVERY FOLDER BUT THE FILE ITSELF (the last part).
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        at = format!("{at}/{part}");
        out.push(at.clone());
    }
    out
}

// ── anchor: L10 ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folders_of_a_private_target_are_the_ones_above_the_file() {
        assert_eq!(
            private_folders_of("home/.gnupg/private-keys-v1.d/A1.key"),
            ["home/.gnupg", "home/.gnupg/private-keys-v1.d"]
        );
        assert_eq!(private_folders_of("home/.ssh/config"), ["home/.ssh"]);
        assert!(
            private_folders_of("home/.gitconfig").is_empty(),
            "not private"
        );
        assert!(private_folders_of("config/theme.json").is_empty());
    }
}
