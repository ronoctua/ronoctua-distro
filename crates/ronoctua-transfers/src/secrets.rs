//! A password on its way to a job: a `0600` file, created new, in a folder of
//! the queue's own — and deleted by the job.
//!
//! # Why a file, and why its PATH is what crosses the socket
//!
//! Because a password must never be an argument (`ps` reads every argv on the
//! machine) and must never sit in the ledger, which every watcher receives
//! whole. The door that asked writes it here, `create_new` with the mode ON
//! the creation — a file made and then chmodded is readable between the two
//! calls — and the work carries only where it is. The job reads it when a
//! program asks (`ronoctua-archive` hands it on stdin) and removes it when it
//! ends, however it ends.
//!
//! # Where
//!
//! `$XDG_RUNTIME_DIR/<name>-transfers/secrets`, per instance like the socket
//! (`ronoctua_xdg::runtime_file`): a tmpfs that dies with the session, owned
//! by the person, `0700`. Nothing outside it is ever removed by [`forget`] —
//! a client names the path, and a path the queue did not hand out is not the
//! queue's to delete.

use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};

/// What every secret's file name starts with: a file in the folder without it
/// is not one of ours.
const PREFIX: &str = "job-";

/// The longest password kept: somebody typed it. The same bound the archive
/// crate reads with.
const MOST_BYTES: usize = 4096;

/// The folder the secrets live in, or `None` without a runtime directory —
/// there is no fallback to `/tmp`, where a secret would be one `ls` away from
/// anybody.
#[must_use]
pub fn dir() -> Option<PathBuf> {
    ronoctua_xdg::runtime_file("transfers").map(|d| d.join("secrets"))
}

/// Keeps `text` for a job, in [`dir`], and answers where.
///
/// # Errors
///
/// No runtime directory, a password that is empty, too long or more than one
/// line, or a file that could not be made.
pub fn keep(text: &str) -> Result<PathBuf> {
    keep_in(
        &dir().context("XDG_RUNTIME_DIR is not set; a password has nowhere private to wait")?,
        text,
    )
}

/// [`keep`], in a folder given — what a test hands.
///
/// # Errors
///
/// As [`keep`].
pub fn keep_in(dir: &Path, text: &str) -> Result<PathBuf> {
    if text.is_empty() || text.len() > MOST_BYTES || text.contains(['\n', '\r', '\0']) {
        bail!("a password is one line of at most {MOST_BYTES} bytes");
    }
    make(dir)?;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    for _ in 0..16 {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("{PREFIX}{}-{nanos}-{n}", std::process::id()));
        // `create_new` AND THE MODE ON THE CREATION: private from its first
        // byte, and never written into a file somebody else made first.
        let opened = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path);
        let mut file = match opened {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e).with_context(|| format!("make {}", path.display())),
        };
        let written = file
            .write_all(text.as_bytes())
            .and_then(|()| file.sync_all());
        if let Err(e) = written {
            let _ = std::fs::remove_file(&path);
            return Err(e).with_context(|| format!("write {}", path.display()));
        }
        return Ok(path);
    }
    bail!("no free name for a secret in {}", dir.display())
}

/// Makes the folder `0700`, or tightens one that was there.
fn make(dir: &Path) -> Result<()> {
    if let Some(parent) = dir.parent() {
        match std::fs::DirBuilder::new().mode(0o700).create(parent) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e).with_context(|| format!("make {}", parent.display())),
        }
    }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e).with_context(|| format!("make {}", dir.display())),
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("make {} private", dir.display()))
}

/// Whether `path` is a secret of ours in `dir`: directly inside it, and named
/// the way [`keep_in`] names one.
#[must_use]
pub fn is_ours(path: &Path, dir: &Path) -> bool {
    path.parent() == Some(dir)
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(PREFIX) && !n.contains('/'))
}

/// Removes the secret at `path`, when it is one of ours — the job's last act.
/// Anything else is left where it is and said in the log: a client that named
/// a path the queue never handed out does not get it deleted.
pub fn forget(path: &Path) {
    let Some(dir) = dir() else {
        return;
    };
    forget_in(path, &dir);
}

/// [`forget`], in a folder given.
pub fn forget_in(path: &Path, dir: &Path) {
    if !is_ours(path, dir) {
        tracing::warn!(path = %path.display(), "a secret outside the queue's folder is not removed");
        return;
    }
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(error = %e, "a secret could not be removed"),
    }
}

/// Removes every secret left in [`dir`] — at the daemon's start, when no job
/// is running: a secret outlives no queue.
pub fn sweep() {
    if let Some(dir) = dir() {
        sweep_in(&dir);
    }
}

/// [`sweep`], in a folder given.
pub fn sweep_in(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_ours(&path, dir) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt as _;

    fn place(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ronoctua-transfers-secrets-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn a_secret_is_private_from_its_first_byte_and_goes_with_the_job() {
        let root = place("keep");
        let dir = root.join("secrets");
        let path = keep_in(&dir, "right horse").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "right horse");
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
        assert!(is_ours(&path, &dir));
        let second = keep_in(&dir, "another").unwrap();
        assert_ne!(path, second, "two secrets never share a file");
        forget_in(&path, &dir);
        assert!(!path.exists());
        // FORGETTING TWICE is not a failure: the job's end and a sweep may
        // both reach one.
        forget_in(&path, &dir);
        sweep_in(&dir);
        assert!(!second.exists(), "the sweep left a secret behind");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nothing_outside_the_folder_is_removed_and_a_bad_password_is_refused() {
        let root = place("outside");
        let dir = root.join("secrets");
        std::fs::create_dir_all(&dir).unwrap();
        let theirs = root.join("job-precious");
        std::fs::write(&theirs, "keep me").unwrap();
        forget_in(&theirs, &dir);
        assert!(
            theirs.exists(),
            "a path the queue never handed out was removed"
        );
        let unnamed = dir.join("notes.txt");
        std::fs::write(&unnamed, "keep me too").unwrap();
        forget_in(&unnamed, &dir);
        sweep_in(&dir);
        assert!(unnamed.exists(), "a file not named as a secret was removed");
        for bad in ["", "two\nlines", "nul\0byte"] {
            assert!(keep_in(&dir, bad).is_err(), "{bad:?} was kept");
        }
        assert!(keep_in(&dir, &"x".repeat(MOST_BYTES + 1)).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_folder_somebody_left_open_is_tightened() {
        let root = place("tighten");
        let dir = root.join("secrets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = keep_in(&dir, "pw").unwrap();
        assert_eq!(std::fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
        forget_in(&path, &dir);
        let _ = std::fs::remove_dir_all(root);
    }
}
