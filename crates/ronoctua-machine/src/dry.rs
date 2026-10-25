//! What `nix build --dry-run` says it would do, read off its standard error.
//!
//! Nix prints two headings and nothing else of the kind:
//!
//! ```text
//! these 3 derivations will be built:          (or "this derivation will be built:")
//! these 12 paths will be fetched (34.56 MiB download, 120.00 MiB unpacked):
//! ```
//!
//! — the size in the unit Nix chose (`KiB`, `MiB`, `GiB`…) — each followed by
//! its store paths, one a line, two spaces in (a third, "don't know how to
//! build these paths:", lists what nothing can make). A heading that is
//! absent is nothing to do: no build, no download.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ronoctua_person::frontier::{self, Download};

use crate::{Rehearsal, argv, run};

/// The derivations the dry run says will be built: the `.drv`s listed under
/// its "will be built:" heading. That is Nix's own plan — a derivation whose
/// consumer the store already holds is not in it, though it is in the
/// `.drv` closure. `None` for words this program does not read: a heading
/// whose count is not the paths under it, a path there that is not a
/// `.drv`, or a store path under no heading it knows. Never a set smaller
/// than the one Nix said.
#[must_use]
pub fn to_build(text: &str) -> Option<BTreeSet<String>> {
    /// The heading the lines below belong to.
    enum Under {
        /// None, or a line that is no heading (a warning).
        Nothing,
        /// What will be built: the count said and the paths listed.
        Built { said: u64, listed: u64 },
        /// What will be fetched, or what nothing can build.
        Other,
    }
    let complete = |under: &Under| match under {
        Under::Built { said, listed } => said == listed,
        Under::Nothing | Under::Other => true,
    };
    let mut built = BTreeSet::new();
    let mut under = Under::Nothing;
    for line in text.lines() {
        let item = line.trim();
        if line.starts_with(char::is_whitespace) && item.starts_with("/nix/store/") {
            match &mut under {
                Under::Built { listed, .. }
                    if item.ends_with(".drv") && !item.contains(char::is_whitespace) =>
                {
                    *listed += 1;
                    built.insert(item.to_owned());
                }
                Under::Other => {}
                Under::Built { .. } | Under::Nothing => return None,
            }
            continue;
        }
        if !complete(&under) {
            return None;
        }
        under = if item.ends_with("will be built:") {
            Under::Built {
                said: count(item),
                listed: 0,
            }
        } else if item.contains("will be fetched") || item.contains("know how to build") {
            Under::Other
        } else {
            Under::Nothing
        };
    }
    complete(&under).then_some(built)
}

/// The downloads building `drv` still needs: the fixed-output derivations
/// among what a dry run of it says it would build, each with the hash its
/// output is pinned to and where it comes from ([`frontier::downloads`]).
/// Asked of Nix's own plan after a build failed — a part's
/// (`ronoctua-parts realize`) or the system's ([`crate::build`]) —, never of
/// its log; an answer this cannot read is no list, and so is a program that
/// does not start. `nix-store` is the one beside `nix`.
#[must_use]
pub fn missing(nix: &Path, drv: &str) -> Vec<Download> {
    let Ok(dry) = run::capture(nix, &argv::dry_run_drvs(&[drv.to_owned()])) else {
        return Vec::new();
    };
    let Some(will) = to_build(&dry.stderr).filter(|w| !w.is_empty()) else {
        return Vec::new();
    };
    let will_list: Vec<String> = will.iter().cloned().collect();
    let shown = run::capture(nix, &argv::show(&will_list))
        .ok()
        .filter(run::Ran::ok)
        .and_then(|ran| serde_json::from_str::<serde_json::Value>(&ran.stdout).ok());
    let Some(shown) = shown else {
        return Vec::new();
    };
    // THE OUTPUT OF EACH DOWNLOAD ONLY: one question per derivation, and a
    // system's plan lists hundreds that are no download.
    let nix_store = nix.with_file_name("nix-store");
    let outputs: BTreeMap<String, String> = frontier::fixed(&shown)
        .into_iter()
        .filter(|d| will.contains(d))
        .filter_map(|d| {
            let ran = run::capture(&nix_store, &argv::outputs(&d)).ok()?;
            let out = ran.stdout.lines().next()?.trim().to_owned();
            (ran.ok() && !out.is_empty()).then_some((d, out))
        })
        .collect();
    frontier::downloads(&shown, &will, &outputs)
}

/// The rehearsal `text` describes.
#[must_use]
pub fn rehearsal(text: &str) -> Rehearsal {
    let mut r = Rehearsal {
        download_bytes: 0,
        builds: 0,
    };
    for line in text.lines().map(str::trim) {
        if line.ends_with("will be built:") {
            r.builds += count(line);
        } else if line.contains("will be fetched (") {
            r.download_bytes += download(line).unwrap_or(0);
        }
    }
    r
}

/// The N of "these N …"; 1 for "this …".
fn count(line: &str) -> u64 {
    let mut words = line.split_whitespace();
    match words.next() {
        Some("these") => words.next().and_then(|n| n.parse().ok()).unwrap_or(0),
        Some("this") => 1,
        _ => 0,
    }
}

/// The bytes of "(<size> <unit> download, …)".
fn download(line: &str) -> Option<u64> {
    let inside = line.split_once('(')?.1;
    let before = inside.split_once(" download")?.0;
    let (number, unit) = before.trim().rsplit_once(' ')?;
    let number: f64 = number.trim().parse().ok()?;
    let scale: f64 = match unit {
        "B" | "bytes" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    let bytes = (number * scale).round();
    if bytes.is_finite() && bytes >= 0.0 {
        // In range by the guard: a dry run never fetches 2^64 bytes.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        Some(bytes as u64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_headings_are_read_in_every_unit() {
        let text = "warning: Git tree is dirty\n\
                    these 3 derivations will be built:\n  /nix/store/a.drv\n  /nix/store/b.drv\n  /nix/store/c.drv\n\
                    these 12 paths will be fetched (34.50 MiB download, 120.00 MiB unpacked):\n  /nix/store/x\n";
        let r = rehearsal(text);
        assert_eq!(r.builds, 3);
        assert_eq!(r.download_bytes, 36_175_872);
        let one = rehearsal(
            "this derivation will be built:\n  /nix/store/a.drv\nthis path will be fetched (1.5 KiB download, 3.0 KiB unpacked):\n",
        );
        assert_eq!((one.builds, one.download_bytes), (1, 1536));
        let big = rehearsal("these 2 paths will be fetched (1.25 GiB download, 4 GiB unpacked):");
        assert_eq!(big.download_bytes, 1_342_177_280);
    }

    /// FAILS IF: what Nix says it will build is read as anything but the
    /// `.drv`s it listed — a list of fetched paths taken for builds, a
    /// listed derivation left out, or a plan in words this does not read
    /// taken for "nothing to build".
    #[test]
    fn what_will_be_built_is_what_nix_listed() {
        // Nix 2.35.1's own words (the names shortened), for a derivation and
        // its one input.
        let two = "these 2 derivations will be built:\n  /nix/store/04fbsbvycsh2x8rnak9jvi5jcasf6gjc-n4-probe-dep.drv\n  /nix/store/ij2y37vfjigsxrh3hm36wlv2cxx459vg-n4-probe-top.drv\n";
        assert_eq!(
            to_build(two).unwrap().into_iter().collect::<Vec<_>>(),
            [
                "/nix/store/04fbsbvycsh2x8rnak9jvi5jcasf6gjc-n4-probe-dep.drv",
                "/nix/store/ij2y37vfjigsxrh3hm36wlv2cxx459vg-n4-probe-top.drv"
            ]
        );
        let mixed = "warning: Git tree '/etc/nixos' is dirty\n\
                     this derivation will be built:\n  /nix/store/a-nixos-system.drv\n\
                     these 2 paths will be fetched (1.00 MiB download, 2.00 MiB unpacked):\n  /nix/store/x-glibc\n  /nix/store/y-ffmpeg\n";
        assert_eq!(
            to_build(mixed).unwrap().into_iter().collect::<Vec<_>>(),
            ["/nix/store/a-nixos-system.drv"]
        );
        assert_eq!(
            to_build("").unwrap().len(),
            0,
            "a consumer the store holds: nothing below it is built"
        );
        assert_eq!(
            to_build("these 2 paths will be fetched (1 KiB download, 2 KiB unpacked):\n  /nix/store/x\n  /nix/store/y\n").unwrap().len(),
            0
        );
        for unread in [
            "these 3 derivations will be built:\n  /nix/store/a.drv\n  /nix/store/b.drv\n",
            "this derivation will be built:\n  /nix/store/a.drv\n  /nix/store/b.drv\n",
            "these many derivations will be built:\n  /nix/store/a.drv\n",
            "this derivation will be built:\n  /nix/store/a-not-a-derivation\n",
            "these derivations are on their way:\n  /nix/store/a.drv\n",
            "  /nix/store/a.drv\n",
        ] {
            assert_eq!(to_build(unread), None, "{unread:?}");
        }
    }

    #[test]
    fn nothing_said_is_nothing_to_do() {
        let r = rehearsal("");
        assert_eq!((r.builds, r.download_bytes), (0, 0));
        let odd = rehearsal("these many paths will be fetched (lots download):");
        assert_eq!(
            odd.download_bytes, 0,
            "a size it cannot read is not invented"
        );
    }
}
