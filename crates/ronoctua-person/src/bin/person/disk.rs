//! Opening, reading and writing without ever following a link.
//!
//! Every entry below a folder this program opened is reached RELATIVE to that
//! folder's descriptor (`openat`, `statat`, `mkdirat`, `unlinkat`,
//! `renameat`) and with `O_NOFOLLOW`, so a name the person controls can never
//! become a path somewhere else between being looked at and being used. A
//! whole path is resolved once, for the folder the unit was handed, and that
//! folder must be a folder and not a link.

use std::ffi::CString;
use std::io::{Read as _, Write as _};
use std::os::fd::{AsFd, OwnedFd};

use ronoctua_person::tree::{Refused, TreeWhy};
use rustix::fs::{AtFlags, CWD, FileType, Mode, OFlags, Stat};
use rustix::io::Errno;

/// What a folder this program opened must be: a folder, owned by `owner`,
/// and writable by nobody else.
#[derive(Debug, Clone, Copy)]
pub struct Whose {
    /// The uid every entry must belong to.
    pub owner: u32,
}

/// The flags of every folder opened here.
fn dir_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

/// The flags of every file opened for reading here: no link, no blocking on a
/// pipe, no terminal taken over.
fn read_flags() -> OFlags {
    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY | OFlags::CLOEXEC
}

/// The refusal an `open` failure stands for: a link is a link, a file where a
/// folder was asked for is misplaced, anything else could not be read.
pub fn refusal_of(errno: Errno, at: &str) -> Refused {
    match errno {
        Errno::LOOP => Refused::new(TreeWhy::Link, at, "a symbolic link"),
        Errno::NOTDIR => Refused::new(TreeWhy::Misplaced, at, "not a folder"),
        e => Refused::new(TreeWhy::Unreadable, at, e.to_string()),
    }
}

/// The checks every entry passes once open: the kind it was when listed (the
/// same device and inode — nothing was swapped in between), its owner, and
/// that nobody else may write it.
pub fn same_and_theirs(was: &Stat, now: &Stat, whose: Whose, at: &str) -> Result<(), Refused> {
    if was.st_dev != now.st_dev || was.st_ino != now.st_ino {
        return Err(Refused::new(
            TreeWhy::Unreadable,
            at,
            "it changed while it was being read",
        ));
    }
    if now.st_uid != whose.owner {
        return Err(Refused::new(
            TreeWhy::NotOwned,
            at,
            format!("owned by uid {}, not {}", now.st_uid, whose.owner),
        ));
    }
    if now.st_mode & 0o022 != 0 {
        return Err(Refused::new(
            TreeWhy::Writable,
            at,
            format!(
                "mode {:o}: somebody else may write it",
                now.st_mode & 0o7777
            ),
        ));
    }
    Ok(())
}

/// The folder at `path`, which must be a folder and not a link, and `whose`.
pub fn open_root(path: &str, whose: Whose) -> Result<OwnedFd, Refused> {
    let before = rustix::fs::statat(CWD, path, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|e| refusal_of(e, path))?;
    match FileType::from_raw_mode(before.st_mode) {
        FileType::Directory => {}
        FileType::Symlink => return Err(Refused::new(TreeWhy::Link, path, "a symbolic link")),
        _ => return Err(Refused::new(TreeWhy::Misplaced, path, "not a folder")),
    }
    let fd = rustix::fs::openat(CWD, path, dir_flags(), Mode::empty())
        .map_err(|e| refusal_of(e, path))?;
    let now = rustix::fs::fstat(&fd).map_err(|e| refusal_of(e, path))?;
    same_and_theirs(&before, &now, whose, path)?;
    Ok(fd)
}

/// A name as the kernel takes it.
pub fn c_name(name: &str) -> Result<CString, Refused> {
    CString::new(name).map_err(|_| Refused::new(TreeWhy::BadName, name, "a NUL in a name"))
}

/// The names in a folder, sorted, without `.` and `..`.
pub fn names(dir: &OwnedFd, at: &str) -> Result<Vec<Vec<u8>>, Refused> {
    let listing = rustix::fs::Dir::read_from(dir).map_err(|e| refusal_of(e, at))?;
    let mut out = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|e| refusal_of(e, at))?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            out.push(name.to_vec());
        }
    }
    out.sort();
    Ok(out)
}

/// The entry `name` of `dir`, not followed.
pub fn stat_at(dir: &OwnedFd, name: &CString, at: &str) -> Result<Stat, Refused> {
    rustix::fs::statat(dir, name.as_c_str(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|e| refusal_of(e, at))
}

/// The folder `name` of `dir`, which was `was` when listed.
pub fn open_dir_at(
    dir: &OwnedFd,
    name: &CString,
    was: &Stat,
    whose: Whose,
    at: &str,
) -> Result<OwnedFd, Refused> {
    let fd = rustix::fs::openat(dir, name.as_c_str(), dir_flags(), Mode::empty())
        .map_err(|e| refusal_of(e, at))?;
    let now = rustix::fs::fstat(&fd).map_err(|e| refusal_of(e, at))?;
    same_and_theirs(was, &now, whose, at)?;
    Ok(fd)
}

/// The bytes of the file `name` of `dir`, which was `was` when listed: one
/// name only (a hard link is refused), at most `most_bytes`.
pub fn read_at(
    dir: &OwnedFd,
    name: &CString,
    was: &Stat,
    whose: Whose,
    most_bytes: u64,
    at: &str,
) -> Result<Vec<u8>, Refused> {
    let fd = rustix::fs::openat(dir, name.as_c_str(), read_flags(), Mode::empty())
        .map_err(|e| refusal_of(e, at))?;
    let now = rustix::fs::fstat(&fd).map_err(|e| refusal_of(e, at))?;
    if FileType::from_raw_mode(now.st_mode) != FileType::RegularFile {
        return Err(Refused::new(TreeWhy::Special, at, "not a regular file"));
    }
    same_and_theirs(was, &now, whose, at)?;
    if now.st_nlink != 1 {
        return Err(Refused::new(
            TreeWhy::HardLink,
            at,
            format!("{} names for one file", now.st_nlink),
        ));
    }
    let mut bytes = Vec::new();
    std::fs::File::from(fd)
        .take(most_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Refused::new(TreeWhy::Unreadable, at, e.to_string()))?;
    if bytes.len() as u64 > most_bytes {
        return Err(Refused::new(
            TreeWhy::TooLarge,
            at,
            format!("over {most_bytes} bytes"),
        ));
    }
    Ok(bytes)
}

/// The refusal of a write.
pub fn unwritable(at: &str, e: impl std::fmt::Display) -> Refused {
    Refused::new(TreeWhy::Unwritable, at, e.to_string())
}

/// Writes `bytes` as `name` in `dir`, whole: a draft beside it (a dot name)
/// created with `mode` already on, synced, and renamed over the old one —
/// then the folder synced, so the rename survives a power cut.
pub fn write_whole(dir: &OwnedFd, name: &str, bytes: &[u8], mode: u32) -> Result<(), Refused> {
    let draft = format!(".{name}.{}", std::process::id());
    match rustix::fs::unlinkat(dir, draft.as_str(), AtFlags::empty()) {
        Ok(()) | Err(Errno::NOENT) => {}
        Err(e) => return Err(unwritable(&draft, e)),
    }
    let written = (|| {
        let fd = rustix::fs::openat(
            dir,
            draft.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(mode),
        )?;
        rustix::fs::fchmod(&fd, Mode::from_raw_mode(mode))?;
        let mut file = std::fs::File::from(fd);
        file.write_all(bytes)
            .map_err(|e| e.raw_os_error().map_or(Errno::IO, Errno::from_raw_os_error))?;
        rustix::fs::fsync(file.as_fd())?;
        rustix::fs::renameat(dir, draft.as_str(), dir, name)?;
        rustix::fs::fsync(dir)
    })();
    if let Err(e) = written {
        let _ = rustix::fs::unlinkat(dir, draft.as_str(), AtFlags::empty());
        return Err(unwritable(name, e));
    }
    Ok(())
}

/// The folder `name` of `dir`, made (with `mode`) if it is not there; a link
/// or a file there is refused.
pub fn dir_in(dir: &OwnedFd, name: &str, mode: u32) -> Result<OwnedFd, Refused> {
    match rustix::fs::mkdirat(dir, name, Mode::from_raw_mode(mode)) {
        Ok(()) | Err(Errno::EXIST) => {}
        Err(e) => return Err(unwritable(name, e)),
    }
    let fd = rustix::fs::openat(dir, name, dir_flags(), Mode::empty())
        .map_err(|e| refusal_of(e, name))?;
    Ok(fd)
}

/// Removes the entry `name` of `dir` and, if it is a folder, everything in
/// it — never through a link: a link is removed, not followed.
pub fn remove_tree(dir: &OwnedFd, name: &str) -> Result<(), Refused> {
    let st = match rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(st) => st,
        Err(Errno::NOENT) => return Ok(()),
        Err(e) => return Err(unwritable(name, e)),
    };
    if FileType::from_raw_mode(st.st_mode) == FileType::Directory {
        let inner = rustix::fs::openat(dir, name, dir_flags(), Mode::empty())
            .map_err(|e| unwritable(name, e))?;
        for child in names(&inner, name)? {
            let child = String::from_utf8(child)
                .map_err(|_| unwritable(name, "a name that is not text"))?;
            remove_tree(&inner, &child)?;
        }
        rustix::fs::unlinkat(dir, name, AtFlags::REMOVEDIR).map_err(|e| unwritable(name, e))
    } else {
        rustix::fs::unlinkat(dir, name, AtFlags::empty()).map_err(|e| unwritable(name, e))
    }
}

/// The uid named by `owner`: a number, or a name in a passwd text.
pub fn uid_of(owner: &str, passwd: &str) -> Option<u32> {
    if !owner.is_empty() && owner.bytes().all(|b| b.is_ascii_digit()) {
        return owner.parse().ok();
    }
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let _password = fields.next()?;
        let uid = fields.next()?.parse().ok()?;
        (name == owner).then_some(uid)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_owner_is_a_number_or_a_name_the_passwd_file_knows() {
        let passwd = "root:x:0:0::/root:/bin/sh\nsomeone:x:1000:100::/home/someone:/bin/zsh\n";
        assert_eq!(uid_of("1000", passwd), Some(1000));
        assert_eq!(uid_of("someone", passwd), Some(1000));
        assert_eq!(uid_of("root", passwd), Some(0));
        assert_eq!(uid_of("nobody-here", passwd), None);
        assert_eq!(uid_of("", passwd), None);
    }
}
