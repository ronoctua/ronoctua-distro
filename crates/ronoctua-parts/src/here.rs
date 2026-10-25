//! Whether a part is here — the one question every reader of a part asks.
//!
//! A part's path ([`crate::names::profile`]) is stable and may not exist yet:
//! the part is arriving, waits for a network, or was declined. A reader that
//! used the path without asking would fail far from the cause, so it asks
//! here, on EVERY use and never once at start — a part arrives while the
//! session runs, and a reader that asked at start would keep saying it is
//! missing (`part-paths-are-read-as-arriving`).

use std::path::{Path, PathBuf};

/// A part that is not here, and which: the reader's sentence names it and
/// opens the parts' page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Absent {
    /// The part's id, the segment after the parts' root.
    pub part: Option<String>,
    /// The path that was asked after.
    pub path: PathBuf,
}

/// `path` when it exists, else what is absent.
///
/// `path` is a part's profile or anything under it (`…/v1/bin/yt-dlp`): the
/// part is named from the segment after [`crate::names::PARTS_ROOT`].
///
/// # Errors
///
/// [`Absent`] when nothing is at `path` — a dangling profile link included,
/// since `Path::exists` follows it.
pub fn here(path: &Path) -> Result<PathBuf, Absent> {
    if path.exists() {
        return Ok(path.to_path_buf());
    }
    let part = path
        .strip_prefix(crate::names::PARTS_ROOT)
        .ok()
        .and_then(|rest| rest.components().next())
        .and_then(|c| c.as_os_str().to_str())
        .map(str::to_owned);
    Err(Absent {
        part,
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a missing part is not named, so the screen could not say
    /// WHICH part is arriving — or a path that exists is taken for absent.
    #[test]
    fn a_missing_part_is_named() {
        let missing = Path::new("/nix/var/nix/profiles/ronoctua/parts/yt-dlp/v1/bin/yt-dlp");
        let absent = here(missing).expect_err("nothing is there in a test");
        assert_eq!(absent.part.as_deref(), Some("yt-dlp"));
        assert_eq!(absent.path, missing);

        let elsewhere = here(Path::new("/nonexistent/x")).expect_err("absent");
        assert_eq!(elsewhere.part, None);

        let there = std::env::temp_dir();
        assert_eq!(here(&there), Ok(there.clone()));
    }
}
