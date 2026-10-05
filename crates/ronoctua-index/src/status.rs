//! What the unit is doing, said in a file beside the index: `status.json`.
//!
//! # Why a file
//!
//! The brain starts the unit and draws one mark — only while the index is
//! built FROM SCRATCH, the first walk of a home, minutes of disk — and its
//! page says where the index stands: how many documents, when the last commit
//! landed. The unit is a process of systemd's with no socket of its own, and a
//! file it writes when its phase moves is what the brain's watch hears
//! (inotify on the folder, never a poll). A reader's watch is on `meta.json`
//! by name (`crate::on_commit`), so this file waking the folder costs a door
//! nothing.
//!
//! # Written whole, by rename
//!
//! [`write`] puts a draft beside it and renames it into place: a reader sees
//! the old status or the new, never half of one.
//!
//! # And whether it is still true
//!
//! A unit killed (the memory ceiling, a stop) leaves its last status behind,
//! "walking" for ever. So the file names its process, and [`Status::alive`]
//! asks `/proc` whether that process is still this program — a pid is
//! recycled, and a living pid of somebody else's is no unit running.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The file's name, in the index's folder.
pub const FILE: &str = "status.json";

/// What the unit is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Its first pass over every source, since it started.
    Walking,
    /// The pass ended; it follows the kernel's events.
    Following,
}

impl Phase {
    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Walking => "walking",
            Self::Following => "following",
        }
    }
}

/// The unit's status, as it last wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Status {
    /// The unit's process.
    pub pid: u32,
    /// What it is doing.
    pub phase: Phase,
    /// Whether this walk began with an EMPTY index — a first walk, or an
    /// index of another format wiped: what the brain's mark is for.
    pub scratch: bool,
    /// When the unit started, seconds since the epoch.
    pub since_s: i64,
    /// How many documents its last commit holds.
    pub documents: u64,
    /// When its last commit landed, seconds since the epoch; `None` before the
    /// first.
    pub committed_s: Option<i64>,
}

impl Status {
    /// Whether the process that wrote it is still this program: its pid is
    /// alive and its name is the unit's.
    #[must_use]
    pub fn alive(&self) -> bool {
        alive_at(Path::new("/proc"), self.pid)
    }

    /// Whether the index is being built from scratch NOW — what the brain
    /// marks.
    #[must_use]
    pub fn building_from_scratch(&self) -> bool {
        self.scratch && self.phase == Phase::Walking && self.alive()
    }
}

/// Whether `pid` under `proc` is a process of this program — its `comm`.
#[must_use]
pub fn alive_at(proc: &Path, pid: u32) -> bool {
    std::fs::read_to_string(proc.join(pid.to_string()).join("comm"))
        .is_ok_and(|comm| comm.trim() == PROGRAM_COMM)
}

/// What the kernel calls the unit's process: its executable's name, cut at
/// the fifteen bytes `comm` holds.
pub const PROGRAM_COMM: &str = "ronoctua-index";

/// The status in `dir`, when one is there and reads.
#[must_use]
pub fn read(dir: &Path) -> Option<Status> {
    let bytes = std::fs::read(dir.join(FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Writes `status` into `dir`, whole, by rename.
///
/// # Errors
///
/// The draft could not be written or renamed.
pub fn write(dir: &Path, status: &Status) -> std::io::Result<()> {
    let text = serde_json::to_vec(status).map_err(std::io::Error::other)?;
    let draft = dir.join(format!(".{FILE}.{}", std::process::id()));
    std::fs::write(&draft, text)?;
    std::fs::rename(&draft, dir.join(FILE)).inspect_err(|_| {
        let _ = std::fs::remove_file(&draft);
    })
}

/// How many bytes the files directly in `dir` weigh — the index's folder,
/// whose segments are files of its own. `0` when it cannot be read.
#[must_use]
pub fn weight_of(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|e| e.metadata().ok())
                .filter(std::fs::Metadata::is_file)
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-index-status-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// FAILS IF: a status does not come back as written, or a killed unit's
    /// "walking" is taken for a walk going on.
    #[test]
    fn a_status_is_read_back_and_a_dead_writer_builds_nothing() {
        let dir = scratch("roundtrip");
        let s = Status {
            pid: u32::MAX,
            phase: Phase::Walking,
            scratch: true,
            since_s: 10,
            documents: 0,
            committed_s: None,
        };
        write(&dir, &s).unwrap();
        assert_eq!(read(&dir), Some(s.clone()));
        assert!(!s.alive(), "no such process");
        assert!(!s.building_from_scratch());
        assert!(weight_of(&dir) > 0);
        assert_eq!(read(&dir.join("nowhere")), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_process_is_this_program_by_its_name() {
        let proc = scratch("proc");
        std::fs::create_dir_all(proc.join("42")).unwrap();
        std::fs::write(proc.join("42/comm"), "ronoctua-index\n").unwrap();
        std::fs::create_dir_all(proc.join("43")).unwrap();
        std::fs::write(proc.join("43/comm"), "bash\n").unwrap();
        assert!(alive_at(&proc, 42));
        assert!(!alive_at(&proc, 43), "a recycled pid");
        assert!(!alive_at(&proc, 44));
        let _ = std::fs::remove_dir_all(proc);
    }
}
