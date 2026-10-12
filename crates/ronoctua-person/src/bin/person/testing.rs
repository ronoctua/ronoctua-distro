//! Folders for the tests, each its own, removed when dropped.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A fresh, empty folder under the temporary directory, `0755`.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(what: &str) -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("ronoctua-person-{what}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let me = Self(dir);
        me.mode("", 0o755);
        me
    }

    /// Sets the mode of `rel` (under the folder).
    pub fn mode(&self, rel: &str, mode: u32) {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(self.0.join(rel), std::fs::Permissions::from_mode(mode)).unwrap();
    }

    /// A folder (and its parents), `0755` each.
    pub fn dir(&self, rel: &str) {
        let mut at = String::new();
        for part in rel.split('/') {
            at = if at.is_empty() {
                part.to_owned()
            } else {
                format!("{at}/{part}")
            };
            let _ = std::fs::create_dir(self.0.join(&at));
            self.mode(&at, 0o755);
        }
    }

    /// A file with `bytes` (its folders made), `0644`.
    pub fn file(&self, rel: &str, bytes: &[u8]) {
        if let Some((parent, _)) = rel.rsplit_once('/') {
            self.dir(parent);
        }
        let _ = std::fs::remove_file(self.0.join(rel));
        std::fs::write(self.0.join(rel), bytes).unwrap();
        self.mode(rel, 0o644);
    }

    /// A program here, written with `text` and made executable.
    pub fn program(&self, name: &str, text: &str) -> PathBuf {
        self.file(name, text.as_bytes());
        self.mode(name, 0o755);
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
