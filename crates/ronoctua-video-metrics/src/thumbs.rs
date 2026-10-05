//! The thumbnails: `mqdefault` pictures in the cache, under a ceiling.
//!
//! `$XDG_CACHE_HOME/ronoctua[-dev]/video-metrics/thumbs/<id>.jpg`, written
//! by the scanner alone ([`Thumbs::keep`]: a scratch named with the pid,
//! renamed over the name) and kept under `videoMetricsThumbsMib`
//! (`assets/cache.nix`): past it, the file written longest ago goes first
//! ([`Thumbs::trim`]), and the catalogue forgets it — the next scan fetches
//! it again when the video is among the best two hundred.
//!
//! # Its own ceiling
//!
//! The sweep of the cache takes whole entries of its root, and
//! `~/.cache/ronoctua` is touched every day: nothing under it is ever swept.
//! A folder that grew by fifteen pictures a channel a scan would grow for
//! ever, so this one keeps itself.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::VideoId;

/// The folder under the cache root.
pub const DIR: &str = "video-metrics/thumbs";

/// The most bytes one thumbnail may have: `mqdefault` is 320×180, ten to
/// thirty kilobytes; a megabyte is not a thumbnail.
pub const MOST_BYTES: u64 = 1 << 20;

#[cfg(feature = "net")]
mod policy {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// One mebibyte.
#[cfg(feature = "net")]
const MIB: u64 = 1024 * 1024;

/// The thumbnails' folder and its ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbs {
    dir: PathBuf,
    most_bytes: u64,
}

impl Thumbs {
    /// A folder `dir` kept under `most_bytes`.
    #[must_use]
    pub const fn at(dir: PathBuf, most_bytes: u64) -> Self {
        Self { dir, most_bytes }
    }

    /// This instance's: [`DIR`] under the cache root, under
    /// `videoMetricsThumbsMib`. `None` with no home.
    #[cfg(feature = "net")]
    #[must_use]
    pub fn of_this_instance() -> Option<Self> {
        let root = ronoctua_xdg::cache_dir()?;
        Some(Self::at(
            root.join(DIR),
            u64::from(policy::VIDEO_METRICS_THUMBS_MIB) * MIB,
        ))
    }

    /// Where they are.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Its ceiling, in bytes.
    #[must_use]
    pub const fn most_bytes(&self) -> u64 {
        self.most_bytes
    }

    /// Where a video's thumbnail is kept; `None` for an id that is not a
    /// video id (a name with a `/` in it would be a path).
    #[must_use]
    pub fn path_of(&self, id: &VideoId) -> Option<PathBuf> {
        let fits = id.0.len() == 11
            && id
                .0
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        fits.then(|| self.dir.join(format!("{}.jpg", id.0)))
    }

    /// Keeps a thumbnail: a JPEG (its first bytes say so) of at most
    /// [`MOST_BYTES`], written whole or not at all. Answers where.
    ///
    /// # Errors
    ///
    /// `InvalidData` for what is not a JPEG or too large or an id that is
    /// not one; the folder's and the file's own errors.
    pub fn keep(&self, id: &VideoId, bytes: &[u8]) -> std::io::Result<PathBuf> {
        let invalid = || std::io::Error::from(std::io::ErrorKind::InvalidData);
        let path = self.path_of(id).ok_or_else(invalid)?;
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if !bytes.starts_with(&[0xff, 0xd8, 0xff]) || size > MOST_BYTES {
            return Err(invalid());
        }
        std::fs::create_dir_all(&self.dir)?;
        let scratch = self
            .dir
            .join(format!(".{}.{}.new", id.0, std::process::id()));
        std::fs::write(&scratch, bytes)?;
        if let Err(e) = std::fs::rename(&scratch, &path) {
            let _ = std::fs::remove_file(&scratch);
            return Err(e);
        }
        Ok(path)
    }

    /// Removes what was written longest ago until the folder is under its
    /// ceiling (scratches of writers that died go first); answers the files
    /// removed, for the catalogue to forget.
    ///
    /// # Errors
    ///
    /// The folder cannot be listed.
    pub fn trim(&self) -> std::io::Result<Vec<PathBuf>> {
        let listing = match std::fs::read_dir(&self.dir) {
            Ok(listing) => listing,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut files: Vec<(bool, SystemTime, u64, PathBuf)> = Vec::new();
        for entry in listing.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let scratch = entry.file_name().to_string_lossy().starts_with('.');
            let written = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            files.push((!scratch, written, meta.len(), entry.path()));
        }
        let mut total: u64 = files.iter().map(|f| f.2).sum();
        // Scratches first (`false` sorts first), then the oldest.
        files.sort_by(|a, b| (a.0, a.1, &a.3).cmp(&(b.0, b.1, &b.3)));
        let mut removed = Vec::new();
        for (kept, _, size, path) in files {
            if kept && total <= self.most_bytes {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(size);
                if kept {
                    removed.push(path);
                }
            }
        }
        Ok(removed)
    }
}

/// Removes a deleted video's thumbnail, if it had one — only a file inside
/// a `thumbs` folder of ours, never a path the catalogue happened to hold.
pub(crate) fn forget(thumb: Option<&str>) {
    let Some(thumb) = thumb.map(Path::new) else {
        return;
    };
    let ours = thumb.parent().is_some_and(|dir| dir.ends_with(DIR))
        && thumb.extension().is_some_and(|e| e == "jpg");
    if ours {
        let _ = std::fs::remove_file(thumb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::Scratch;

    const JPEG: [u8; 6] = [0xff, 0xd8, 0xff, 0xe0, 0, 0];

    #[test]
    fn a_thumbnail_is_a_jpeg_kept_whole_under_its_id() {
        let scratch = Scratch::new("thumbs-keep");
        let thumbs = Thumbs::at(scratch.0.join(DIR), 1024);
        let id = VideoId("aaaaaaaaaaa".to_owned());
        let path = thumbs.keep(&id, &JPEG).unwrap();
        assert_eq!(path, scratch.0.join(DIR).join("aaaaaaaaaaa.jpg"));
        assert_eq!(std::fs::read(&path).unwrap(), JPEG);
        assert!(thumbs.keep(&id, b"<html>").is_err(), "not a JPEG");
        assert!(
            thumbs
                .keep(&VideoId("../../etc/x".to_owned()), &JPEG)
                .is_err(),
            "not an id"
        );
        let names: Vec<_> = std::fs::read_dir(thumbs.dir())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names.len(), 1, "no scratch is left behind");
    }

    #[test]
    fn the_oldest_goes_first_until_under_the_ceiling() {
        let scratch = Scratch::new("thumbs-trim");
        let thumbs = Thumbs::at(scratch.0.join(DIR), 2500);
        let mut picture = JPEG.to_vec();
        picture.resize(1000, 7);
        let ids = ["aaaaaaaaaa1", "aaaaaaaaaa2", "aaaaaaaaaa3", "aaaaaaaaaa4"];
        for (i, id) in ids.iter().enumerate() {
            let path = thumbs.keep(&VideoId((*id).to_owned()), &picture).unwrap();
            let at = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000 + i as u64);
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(at)
                .unwrap();
        }
        std::fs::write(thumbs.dir().join(".dead.1.new"), b"x").unwrap();
        let removed = thumbs.trim().unwrap();
        let names: Vec<String> = removed
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["aaaaaaaaaa1.jpg", "aaaaaaaaaa2.jpg"]);
        assert!(
            !thumbs.dir().join(".dead.1.new").exists(),
            "a dead scratch goes"
        );
        assert!(thumbs.dir().join("aaaaaaaaaa4.jpg").exists());
        assert!(
            Thumbs::at(scratch.0.join("absent"), 1)
                .trim()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn only_our_own_thumbnail_is_forgotten() {
        let scratch = Scratch::new("thumbs-forget");
        let ours = scratch.0.join(DIR).join("aaaaaaaaaaa.jpg");
        std::fs::create_dir_all(ours.parent().unwrap()).unwrap();
        std::fs::write(&ours, JPEG).unwrap();
        let elsewhere = scratch.0.join("keep.jpg");
        std::fs::write(&elsewhere, JPEG).unwrap();
        forget(elsewhere.to_str());
        forget(ours.to_str());
        forget(None);
        assert!(elsewhere.exists() && !ours.exists());
    }
}
