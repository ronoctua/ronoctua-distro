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
                    out.push(Holder { pid, name, node });
                }
            }
        }
    }
    out.sort_by_key(|h| h.pid);
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
}
