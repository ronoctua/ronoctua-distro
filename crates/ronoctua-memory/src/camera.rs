//! Who has a camera open — the question behind the modeline's camera mark and
//! the camera app's "busy" sentence.
//!
//! # Why `/proc` and not the PipeWire graph
//!
//! A browser opens `/dev/videoN` DIRECTLY; the graph never hears of it. The
//! one answer that covers every program is the kernel's: a process whose
//! descriptor table points at a video node. This crate is the tree's one
//! boundary with `/proc`, so the walk lives here.
//!
//! # What it costs
//!
//! Every descriptor of every process this user can read: measured on
//! 2026-09-23, 1 399 of them in 7.9 ms (in Python; this is Rust). Asked every
//! two seconds by the footer, which is 0.4 % of one core at the worst.
//! Another user's processes are unreadable and are skipped — a camera held by
//! another account is invisible here, and the device's own light remains the
//! answer for it.

use std::path::Path;

/// One process holding a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    /// Its pid.
    pub pid: u32,
    /// Its short name (`/proc/<pid>/comm`).
    pub name: String,
    /// The node it holds, e.g. `/dev/video0`.
    pub node: String,
    /// The short names of the processes above it — its parent first, then
    /// the parent's, up to [`LINEAGE`] of them (`/proc/<pid>/stat`).
    pub parents: Vec<String>,
}

/// How far up a holder's parents are read.
///
/// A few: what this answers is "did one of OURS start it", and ours start
/// their device-holding children directly — the studio's `ffmpeg` is its own
/// child, `setpriv` having EXECED into it. Walking to the session's root
/// would find every program's launcher and prove nothing.
pub const LINEAGE: usize = 4;

impl Holder {
    /// Whether it is, or runs under, a process named one of `names` — which
    /// is how a program's own child is told from a stranger's with the same
    /// name: the studio's camera is held by an `ffmpeg`, and so may be
    /// anybody's.
    #[must_use]
    pub fn under(&self, names: &[&str]) -> bool {
        std::iter::once(&self.name)
            .chain(&self.parents)
            .any(|n| names.contains(&n.as_str()))
    }
}

/// Every process with a descriptor whose target starts with `prefix`
/// (`/dev/video` for cameras).
pub fn holders(prefix: &str) -> Vec<Holder> {
    holders_in(Path::new("/proc"), prefix)
}

/// The same walk over another root, for the test.
pub fn holders_in(proc_root: &Path, prefix: &str) -> Vec<Holder> {
    let Ok(entries) = std::fs::read_dir(proc_root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(target) = std::fs::read_link(fd.path()) else {
                continue;
            };
            let target = target.to_string_lossy();
            if target.starts_with(prefix) {
                let name = std::fs::read_to_string(entry.path().join("comm"))
                    .map(|s| s.trim().to_owned())
                    .unwrap_or_default();
                let node = target.into_owned();
                if !out.iter().any(|h: &Holder| h.pid == pid && h.node == node) {
                    let parents = parents_in(proc_root, pid);
                    out.push(Holder {
                        pid,
                        name,
                        node,
                        parents,
                    });
                }
            }
        }
    }
    out.sort_by_key(|h| h.pid);
    out
}

/// The short names of `pid`'s parents, nearest first, up to [`LINEAGE`].
///
/// Read from `stat`, whose fourth field is the parent's pid — after the
/// name in parentheses, which may itself hold spaces and parentheses, so the
/// fields are counted from the LAST `)`. A process that is gone, or one this
/// user cannot read, ends the walk: what was read so far is the answer.
fn parents_in(proc_root: &Path, pid: u32) -> Vec<String> {
    let parent_of = |pid: u32| -> Option<u32> {
        let stat = std::fs::read_to_string(proc_root.join(pid.to_string()).join("stat")).ok()?;
        let after = &stat[stat.rfind(')')? + 1..];
        // THE STATE, THEN THE PARENT.
        after.split_whitespace().nth(1)?.parse().ok()
    };
    let mut out = Vec::new();
    let mut at = pid;
    while out.len() < LINEAGE {
        // 0 IS NOBODY, and 1 is the root every process ends at: it names no
        // program anybody started.
        let Some(parent) = parent_of(at).filter(|p| *p > 1 && *p != at) else {
            break;
        };
        let Ok(name) = std::fs::read_to_string(proc_root.join(parent.to_string()).join("comm"))
        else {
            break;
        };
        out.push(name.trim().to_owned());
        at = parent;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_planted_descriptor_is_found_and_others_are_not() {
        // A fake `/proc`: one process holding a "camera" (a link to a file
        // whose name starts with the prefix), one holding something else.
        let root = std::env::temp_dir().join(format!("ronoctua-holders-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dev = root.join("dev");
        std::fs::create_dir_all(&dev).expect("dev");
        std::fs::write(dev.join("video7"), b"").expect("node");
        std::fs::write(dev.join("null"), b"").expect("null");
        for (pid, name, target) in [("41", "browser", "video7"), ("42", "editor", "null")] {
            let fd = root.join(pid).join("fd");
            std::fs::create_dir_all(&fd).expect("fd dir");
            std::os::unix::fs::symlink(dev.join(target), fd.join("3")).expect("link");
            std::fs::write(root.join(pid).join("comm"), format!("{name}\n")).expect("comm");
        }
        let prefix = dev.join("video").to_string_lossy().into_owned();
        let found = holders_in(&root, &prefix);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].pid, found[0].name.as_str()), (41, "browser"));
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: the studio's own `ffmpeg` — the process that really holds
    /// the camera during a preview or a recording — is not found to run
    /// under the studio, or a stranger's `ffmpeg` with the same name is; or a
    /// name with spaces and parentheses in it breaks the reading of `stat`.
    #[test]
    fn a_holder_knows_whose_child_it_is() {
        let root = std::env::temp_dir().join(format!("ronoctua-lineage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dev = root.join("dev");
        std::fs::create_dir_all(&dev).expect("dev");
        std::fs::write(dev.join("video4"), b"").expect("node");
        // pid, comm, parent, holds the camera
        let tree: [(u32, &str, u32, bool); 5] = [
            (50, "ronoctua-studio", 1, false),
            (51, "ffmpeg", 50, true),
            (60, "a (weird) name", 1, false),
            (61, "sh", 60, false),
            (62, "ffmpeg", 61, true),
        ];
        for (pid, name, parent, holds) in tree {
            let at = root.join(pid.to_string());
            std::fs::create_dir_all(at.join("fd")).expect("fd dir");
            std::fs::write(at.join("comm"), format!("{name}\n")).expect("comm");
            std::fs::write(
                at.join("stat"),
                format!("{pid} ({name}) S {parent} {pid} {pid} 0 -1 4194304 0 0\n"),
            )
            .expect("stat");
            if holds {
                std::os::unix::fs::symlink(dev.join("video4"), at.join("fd").join("9"))
                    .expect("link");
            }
        }
        let prefix = dev.join("video").to_string_lossy().into_owned();
        let found = holders_in(&root, &prefix);
        let pids: Vec<u32> = found.iter().map(|h| h.pid).collect();
        assert_eq!(pids, vec![51, 62]);
        assert_eq!(found[0].parents, vec!["ronoctua-studio".to_owned()]);
        assert_eq!(
            found[1].parents,
            vec!["sh".to_owned(), "a (weird) name".to_owned()]
        );
        let ours = ["ronoctua-studio"];
        assert!(found[0].under(&ours), "the studio's own ffmpeg");
        assert!(!found[1].under(&ours), "a stranger's ffmpeg");
        let _ = std::fs::remove_dir_all(root);
    }
}
