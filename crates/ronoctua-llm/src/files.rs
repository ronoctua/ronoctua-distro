//! Reading and writing the agents' files without surprising the person.
//!
//! Three rules, each from a way this goes wrong:
//!
//! - **Every read is bounded.** A transcript line can be megabytes (a tool's
//!   whole output), and a file that is not what it should be can be anything.
//! - **A secret is read into memory that is wiped** ([`read_secret`]), sized
//!   before the read so it is not copied into a larger buffer on the way.
//! - **A file of the person's keeps its mode and its link**
//!   ([`write_keeping`]): `settings.json` can hold an API key in its `env`
//!   (so it may be `0600`), and it can be a link into a dotfiles folder — an
//!   atomic rename over the LINK would replace it with a file.

use std::io::{BufRead, Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

#[cfg(feature = "fetch")]
use zeroize::Zeroizing;

/// A whole file, refused past `most` bytes (`InvalidData`).
pub(crate) fn read_bounded(path: &Path, most: u64) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(most + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > most {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} is larger than {most} bytes", path.display()),
        ));
    }
    Ok(bytes)
}

/// A whole file of secrets, in memory that is zeroed when it is dropped.
///
/// The buffer is sized from the file's length first, so reading it does not
/// grow it — a grown `Vec` leaves the old, unwiped copy behind in the heap.
#[cfg(feature = "fetch")]
pub(crate) fn read_secret(path: &Path, most: u64) -> std::io::Result<Zeroizing<Vec<u8>>> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    if len > most {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} is larger than {most} bytes", path.display()),
        ));
    }
    let wanted = usize::try_from(len).unwrap_or(usize::MAX);
    let mut bytes = Zeroizing::new(vec![0_u8; wanted]);
    let mut filled = 0;
    while filled < wanted {
        match file.read(&mut bytes[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    // A file that shrank between the length and the read: the zeros past it
    // are not JSON, so cut them (truncating never reallocates).
    bytes.truncate(filled);
    Ok(bytes)
}

/// Where a write to `path` must land: the file a link points at, so the link
/// stays a link; the path itself otherwise.
fn landing(path: &Path) -> std::io::Result<PathBuf> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::canonicalize(path),
        Ok(_) => Ok(path.to_path_buf()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(e) => Err(e),
    }
}

/// Writes `bytes` as `path`, atomically, keeping the mode the file had (or
/// `fresh_mode` for a new one) and writing THROUGH a link.
///
/// The draft is created with the mode already on it (`create_new` + `mode`),
/// synced, and renamed over the file, so a reader sees the old file or the
/// new one, and a `0600` file is never readable in between.
pub(crate) fn write_keeping(path: &Path, bytes: &[u8], fresh_mode: u32) -> std::io::Result<()> {
    let target = landing(path)?;
    let mode = match std::fs::metadata(&target) {
        Ok(meta) => meta.mode() & 0o7777,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => fresh_mode,
        Err(e) => return Err(e),
    };
    let dir = target.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "a file with no folder")
    })?;
    std::fs::create_dir_all(dir)?;
    let name = target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"))?;
    let draft = dir.join(format!(".{name}.{}.new", std::process::id()));
    let _ = std::fs::remove_file(&draft);
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&draft)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&draft, &target)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    written
}

/// One line of a log, as [`next_line`] found it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Line {
    /// A whole line, its `\n` included in the count, not in the buffer.
    Whole(usize),
    /// A line longer than the bound, skipped whole; its length in bytes.
    TooLong(usize),
    /// Bytes after the last `\n`: a line still being written. The caller
    /// does not count them, so the next read starts at the line again.
    Partial,
    /// Nothing left.
    End,
}

/// Reads the next line into `buf` (cleared first), at most `most` bytes of
/// it; a longer one is skipped to its `\n` without being kept.
///
/// A line is only ever handed over WHOLE: a log is read while it is written,
/// and half a JSON object counted now would be the other half's parse
/// failure next time.
pub(crate) fn next_line(
    reader: &mut impl BufRead,
    buf: &mut Vec<u8>,
    most: usize,
) -> std::io::Result<Line> {
    buf.clear();
    let mut seen = 0_usize;
    let mut kept = true;
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            return Ok(if seen == 0 { Line::End } else { Line::Partial });
        }
        let (part, done) = match chunk.iter().position(|b| *b == b'\n') {
            Some(at) => (&chunk[..at], Some(at + 1)),
            None => (chunk, None),
        };
        if kept && buf.len() + part.len() <= most {
            buf.extend_from_slice(part);
        } else {
            kept = false;
            buf.clear();
        }
        let used = done.unwrap_or(chunk.len());
        seen += used;
        reader.consume(used);
        if done.is_some() {
            return Ok(if kept {
                Line::Whole(seen)
            } else {
                Line::TooLong(seen)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-llm-files-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_write_keeps_the_mode_and_goes_through_the_link() {
        let dir = scratch("keep");
        let real = dir.join("dotfiles").join("settings.json");
        std::fs::create_dir_all(real.parent().unwrap()).unwrap();
        std::fs::write(&real, b"{}").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.join("settings.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write_keeping(&link, b"{\"a\":1}", 0o644).unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the link is still a link"
        );
        assert_eq!(std::fs::read(&real).unwrap(), b"{\"a\":1}");
        assert_eq!(std::fs::metadata(&real).unwrap().mode() & 0o777, 0o600);

        let fresh = dir.join("fresh.json");
        write_keeping(&fresh, b"[]", 0o640).unwrap();
        assert_eq!(std::fs::metadata(&fresh).unwrap().mode() & 0o777, 0o640);
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".new"))
            .collect();
        assert!(left.is_empty(), "no draft left: {left:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reads_are_bounded() {
        let dir = scratch("bounded");
        let file = dir.join("big");
        std::fs::write(&file, vec![b'x'; 100]).unwrap();
        assert_eq!(read_bounded(&file, 100).unwrap().len(), 100);
        assert!(read_bounded(&file, 99).is_err());
        #[cfg(feature = "fetch")]
        {
            assert_eq!(read_secret(&file, 100).unwrap().len(), 100);
            assert!(read_secret(&file, 99).is_err());
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn only_whole_lines_are_handed_over() {
        let text = b"one\nabcdefghij\ntwo\nhalf";
        let mut reader = std::io::BufReader::with_capacity(4, &text[..]);
        let mut buf = Vec::new();
        assert_eq!(next_line(&mut reader, &mut buf, 5).unwrap(), Line::Whole(4));
        assert_eq!(buf, b"one");
        assert_eq!(
            next_line(&mut reader, &mut buf, 5).unwrap(),
            Line::TooLong(11),
            "a line past the bound is skipped whole"
        );
        assert_eq!(next_line(&mut reader, &mut buf, 5).unwrap(), Line::Whole(4));
        assert_eq!(buf, b"two");
        assert_eq!(next_line(&mut reader, &mut buf, 5).unwrap(), Line::Partial);
        let mut empty = std::io::BufReader::new(&b""[..]);
        assert_eq!(next_line(&mut empty, &mut buf, 5).unwrap(), Line::End);
    }
}
