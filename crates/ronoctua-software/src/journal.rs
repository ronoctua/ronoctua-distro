//! What was done, for the history the advanced level shows.
//!
//! Append-only and BOUNDED: one line per finished order in
//! `$XDG_STATE_HOME/ronoctua[-dev]/software/history.jsonl`, rewritten to the
//! last [`KEEP`] lines when it passes [`MOST`]. It is a record for a person
//! to read, not a source of truth — [`crate::state`] reads reality.

use std::io::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{AppKey, Refusal};

/// The file's name under our state directory.
pub const FILE: &str = "history.jsonl";
/// Past this many lines, the file is cut.
pub const MOST: usize = 500;
/// What the cut keeps.
pub const KEEP: usize = 400;

/// One finished order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Entry {
    /// When it ended, seconds since the epoch.
    pub when: u64,
    /// The order's word (`install`, `remove`, …).
    pub order: String,
    /// The program, when there was one.
    #[serde(default)]
    pub key: Option<AppKey>,
    /// The name to show (data).
    #[serde(default)]
    pub name: String,
    /// `true` when it worked.
    pub ok: bool,
    /// Why not, when it was refused.
    #[serde(default)]
    pub refusal: Option<Refusal>,
}

/// Add one entry, cutting the file when it grew past [`MOST`].
///
/// # Errors
///
/// When the directory or the file cannot be written.
pub fn add(dir: &Path, entry: &Entry) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(FILE);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{}", serde_json::to_string(entry).unwrap_or_default())?;
    drop(file);
    let lines = read_lines(&path);
    if lines.len() > MOST {
        let kept = lines[lines.len() - KEEP..].join("\n") + "\n";
        ronoctua_xdg::write_atomically(dir, FILE, kept.as_bytes())?;
    }
    Ok(())
}

/// Every entry, newest LAST; a line that does not parse is skipped.
#[must_use]
pub fn read(dir: &Path) -> Vec<Entry> {
    read_lines(&dir.join(FILE))
        .iter()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|t| {
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_history_is_bounded_and_keeps_the_newest() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-software-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for i in 0..(MOST as u64 + 1) {
            add(
                &dir,
                &Entry {
                    when: i,
                    order: "install".into(),
                    key: None,
                    name: String::new(),
                    ok: true,
                    refusal: None,
                },
            )
            .unwrap();
        }
        let all = read(&dir);
        assert_eq!(all.len(), KEEP);
        assert_eq!(all.last().unwrap().when, MOST as u64);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
