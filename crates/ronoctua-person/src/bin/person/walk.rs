//! The walk of a tree root reads: every entry where [`staged_place`] (or the
//! machine's own rule) says to look, reached relative to its folder and never
//! through a link, and nothing anywhere else — and the two narrower reads
//! that are taken apart from it: the icons the declarations name, by name,
//! the staging's nonce, and what the person signed (the manifest and its
//! signature).
//!
//! [`staged_place`]: ronoctua_person::tree::staged_place

use std::os::fd::OwnedFd;

use ronoctua_person::tree::{
    self, Kind, MANIFEST, NONCE, Place, Refused, SIGNATURE, Staged, TreeWhy, caps,
};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, Stat};
use rustix::io::Errno;

use crate::disk::{self, Whose};

/// What a path is to this walk.
pub type Rule<'a> = &'a dyn Fn(&str, Kind) -> Result<Place, Refused>;

/// What one entry of a listed folder came to.
enum Met {
    /// The rule ignores it: named, never opened.
    Left,
    /// A narrower read takes it: not named, not opened here.
    Apart,
    /// A folder to walk, open.
    Folder(OwnedFd),
    /// A file, read.
    File(Vec<u8>),
}

/// The flags of a folder this walk opens again from its own descriptor.
fn here_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC
}

/// One entry, `raw` in `dir`, at `at`: looked at only where the rule looks,
/// and refused if it is a link, a special file, misplaced, not the owner's,
/// writable by others or over its cap.
fn meet(dir: &OwnedFd, raw: &[u8], at: &str, rule: Rule<'_>, whose: Whose) -> Result<Met, Refused> {
    // A place the rule ignores in BOTH shapes is never looked at: not opened,
    // not even asked what it is.
    let as_dir = rule(at, Kind::Dir);
    let as_file = rule(at, Kind::File);
    let both = |place: Place| {
        matches!(&as_dir, Ok(p) if *p == place) && matches!(&as_file, Ok(p) if *p == place)
    };
    if both(Place::Apart) {
        return Ok(Met::Apart);
    }
    if both(Place::Ignore) {
        return Ok(Met::Left);
    }
    let name = std::str::from_utf8(raw)
        .map_err(|_| Refused::new(TreeWhy::BadName, at, "a name that is not text"))?;
    let c_name = disk::c_name(name)?;
    let was = disk::stat_at(dir, &c_name, at)?;
    match FileType::from_raw_mode(was.st_mode) {
        FileType::Symlink => Err(Refused::new(TreeWhy::Link, at, "a symbolic link")),
        FileType::Directory => match as_dir? {
            Place::Walk => Ok(Met::Folder(disk::open_dir_at(
                dir, &c_name, &was, whose, at,
            )?)),
            Place::Ignore => Ok(Met::Left),
            Place::Apart => Ok(Met::Apart),
            Place::Read { .. } => Err(Refused::new(
                TreeWhy::Misplaced,
                at,
                "a folder where a file belongs",
            )),
        },
        FileType::RegularFile => match as_file? {
            Place::Read { most_bytes } => Ok(Met::File(disk::read_at(
                dir, &c_name, &was, whose, most_bytes, at,
            )?)),
            Place::Ignore => Ok(Met::Left),
            Place::Apart => Ok(Met::Apart),
            Place::Walk => Err(Refused::new(
                TreeWhy::Misplaced,
                at,
                "a file where a folder belongs",
            )),
        },
        _ => Err(Refused::new(
            TreeWhy::Special,
            at,
            "a pipe, a socket or a device",
        )),
    }
}

/// Walks the folder `root` (already opened and checked) under `rule`: every
/// folder and file owned by `whose`, writable by nobody else, a file with one
/// name only and under its cap; a link or a special file where the walk looks
/// is refused, and a place the rule ignores is listed and never opened.
///
/// A refusal INSIDE one brain module (`brain-modules/<id>/…`, the folder
/// itself included) leaves that module out — what was read of it is dropped
/// and nothing more of it is opened — and the walk goes on. Anything else
/// refuses the tree, and so do the caps on the whole: the entries met, the
/// bytes read.
pub fn walk(root: &OwnedFd, rule: Rule<'_>, whose: Whose) -> Result<Staged, Refused> {
    let mut staged = Staged::default();
    let mut met = 0usize;
    let mut read = 0u64;
    let first = rustix::fs::openat(root, ".", here_flags(), Mode::empty())
        .map_err(|e| disk::refusal_of(e, "."))?;
    let mut pending: Vec<(OwnedFd, String)> = vec![(first, String::new())];
    while let Some((dir, here)) = pending.pop() {
        if tree::module_of(&here).is_some_and(|id| staged.is_left_out(id)) {
            continue;
        }
        let listing = match disk::names(&dir, &here) {
            Ok(listing) => listing,
            Err(r) => match tree::module_of(&here) {
                Some(id) => {
                    staged.leave_out(id, r);
                    continue;
                }
                None => return Err(r),
            },
        };
        for raw in listing {
            met += 1;
            if met > caps::ENTRIES_MOST {
                return Err(Refused::new(
                    TreeWhy::TooMany,
                    here,
                    format!("more than {} entries", caps::ENTRIES_MOST),
                ));
            }
            let name = String::from_utf8_lossy(&raw).into_owned();
            let at = if here.is_empty() {
                name
            } else {
                format!("{here}/{name}")
            };
            match meet(&dir, &raw, &at, rule, whose) {
                Ok(Met::Apart) => {}
                Ok(Met::Left) => staged.ignored.push(at),
                Ok(Met::Folder(inner)) => {
                    staged.dirs.insert(at.clone());
                    pending.push((inner, at));
                }
                Ok(Met::File(bytes)) => {
                    read += bytes.len() as u64;
                    if read > caps::TREE_MOST_BYTES {
                        return Err(Refused::new(
                            TreeWhy::TooLarge,
                            at,
                            format!("over {} bytes in all", caps::TREE_MOST_BYTES),
                        ));
                    }
                    staged.files.insert(at, bytes);
                }
                Err(r) => match tree::module_of(&at) {
                    Some(id) => {
                        let inside = tree::module_of(&here) == Some(id);
                        staged.leave_out(id, r);
                        // The rest of this folder is the same module's.
                        if inside {
                            break;
                        }
                    }
                    None => return Err(r),
                },
            }
        }
    }
    Ok(staged)
}

/// The rule of a tree this program wrote itself (the machine's `person/`):
/// every folder walked and every file read, up to the total.
pub fn everything(_: &str, kind: Kind) -> Result<Place, Refused> {
    Ok(match kind {
        Kind::Dir => Place::Walk,
        Kind::File => Place::Read {
            most_bytes: caps::TREE_MOST_BYTES,
        },
    })
}

/// The entry `name` of `dir` as it is now, not followed; `None` when it is
/// not there.
fn present(dir: &OwnedFd, name: &str, at: &str) -> Result<Option<Stat>, Refused> {
    match rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(st) => Ok(Some(st)),
        Err(Errno::NOENT) => Ok(None),
        Err(e) => Err(disk::refusal_of(e, at)),
    }
}

/// The refusal of an entry that is not the kind `want`, or `Ok` when it is.
fn of_kind(st: &Stat, want: FileType, at: &str) -> Result<(), Refused> {
    match FileType::from_raw_mode(st.st_mode) {
        kind if kind == want => Ok(()),
        FileType::Symlink => Err(Refused::new(TreeWhy::Link, at, "a symbolic link")),
        FileType::Directory => Err(Refused::new(
            TreeWhy::Misplaced,
            at,
            "a folder where a file belongs",
        )),
        FileType::RegularFile => Err(Refused::new(
            TreeWhy::Misplaced,
            at,
            "a file where a folder belongs",
        )),
        _ => Err(Refused::new(
            TreeWhy::Special,
            at,
            "a pipe, a socket or a device",
        )),
    }
}

/// Reads into `staged` the icons `named` (`<sha256>.png`, from
/// [`tree::named_icons`]) out of the staged `person/software/icons/` — BY
/// NAME, never by listing the folder, so an icon no entry names is never
/// opened and can neither cost a byte nor refuse the tree. Each passes the
/// walk's checks. One that is not there is left for
/// [`tree::assemble`] to refuse, with the entry that names it.
pub fn read_icons(
    root: &OwnedFd,
    named: &[String],
    whose: Whose,
    staged: &mut Staged,
) -> Result<(), Refused> {
    if named.is_empty() {
        return Ok(());
    }
    let mut dir = rustix::fs::openat(root, ".", here_flags(), Mode::empty())
        .map_err(|e| disk::refusal_of(e, "."))?;
    let mut here = String::new();
    for part in [tree::PERSON, "software", "icons"] {
        let at = if here.is_empty() {
            part.to_owned()
        } else {
            format!("{here}/{part}")
        };
        let Some(was) = present(&dir, part, &at)? else {
            return Ok(());
        };
        of_kind(&was, FileType::Directory, &at)?;
        dir = disk::open_dir_at(&dir, &disk::c_name(part)?, &was, whose, &at)?;
        here = at;
    }
    let mut total: u64 = staged.files.values().map(|b| b.len() as u64).sum();
    for name in named {
        let at = format!("{here}/{name}");
        let Some(was) = present(&dir, name, &at)? else {
            continue;
        };
        of_kind(&was, FileType::RegularFile, &at)?;
        let bytes = disk::read_at(
            &dir,
            &disk::c_name(name)?,
            &was,
            whose,
            caps::ICON_MOST_BYTES,
            &at,
        )?;
        total += bytes.len() as u64;
        if total > caps::TREE_MOST_BYTES {
            return Err(Refused::new(
                TreeWhy::TooLarge,
                at,
                format!("over {} bytes in all", caps::TREE_MOST_BYTES),
            ));
        }
        staged.files.insert(at, bytes);
    }
    Ok(())
}

/// A staging's signed pair: the manifest's bytes and its signature's.
pub type SignedPair = (Vec<u8>, Vec<u8>);

/// The manifest the person signed and its signature, as the staging carries
/// them ([`MANIFEST`], [`SIGNATURE`]) — each passing the walk's checks; `None`
/// when either is not there (a staging nobody signed: the caller decides what
/// that is worth, for root's pin).
pub fn read_signed(root: &OwnedFd, whose: Whose) -> Result<Option<SignedPair>, Refused> {
    let one = |name: &str, most_bytes: u64| -> Result<Option<Vec<u8>>, Refused> {
        let Some(was) = present(root, name, name)? else {
            return Ok(None);
        };
        of_kind(&was, FileType::RegularFile, name)?;
        disk::read_at(root, &disk::c_name(name)?, &was, whose, most_bytes, name).map(Some)
    };
    let manifest = one(MANIFEST, caps::MANIFEST_MOST_BYTES)?;
    let signature = one(SIGNATURE, caps::SIGNATURE_MOST_BYTES)?;
    Ok(manifest.zip(signature))
}

/// The staging's [`NONCE`], when it has one: a file of the owner's, one
/// name only, under its cap, holding a nonce.
pub fn read_nonce(root: &OwnedFd, whose: Whose) -> Result<Option<String>, Refused> {
    let Some(was) = present(root, NONCE, NONCE)? else {
        return Ok(None);
    };
    of_kind(&was, FileType::RegularFile, NONCE)?;
    let bytes = disk::read_at(
        root,
        &disk::c_name(NONCE)?,
        &was,
        whose,
        caps::NONCE_MOST_BYTES,
        NONCE,
    )?;
    tree::nonce_of(&bytes).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Scratch;
    use ronoctua_person::tree::staged_place;

    fn whose() -> Whose {
        Whose {
            owner: rustix::process::geteuid().as_raw(),
        }
    }

    fn walk_of(dir: &std::path::Path) -> Result<Staged, Refused> {
        let root = disk::open_root(dir.to_str().unwrap(), whose())?;
        walk(&root, &staged_place, whose())
    }

    /// FAILS IF: a link, a pipe or a second name is read instead of refused
    /// — the ways a person could make root read what is not theirs.
    #[test]
    fn links_and_special_files_are_refused_where_the_walk_looks() {
        let s = Scratch::new("walk-links");
        s.file("person/software/declared.json", b"{}");
        s.file("elsewhere.json", b"{}");
        assert!(walk_of(&s.0).is_ok());

        let link = Scratch::new("walk-link");
        link.dir("person/software");
        std::os::unix::fs::symlink("/etc/passwd", link.0.join("person/software/declared.json"))
            .unwrap();
        assert_eq!(walk_of(&link.0).unwrap_err().why, TreeWhy::Link);

        let folder = Scratch::new("walk-folder-link");
        folder.dir("person");
        std::os::unix::fs::symlink("/etc", folder.0.join("person/software")).unwrap();
        assert_eq!(walk_of(&folder.0).unwrap_err().why, TreeWhy::Link);

        let hard = Scratch::new("walk-hard");
        hard.file("person/options.json", b"{}");
        std::fs::hard_link(hard.0.join("person/options.json"), hard.0.join("spare")).unwrap();
        assert_eq!(walk_of(&hard.0).unwrap_err().why, TreeWhy::HardLink);

        let pipe = Scratch::new("walk-pipe");
        pipe.dir("person");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            pipe.0.join("person/options.json").as_path(),
            FileType::Fifo,
            rustix::fs::Mode::from_raw_mode(0o600),
            0,
        )
        .unwrap();
        assert_eq!(walk_of(&pipe.0).unwrap_err().why, TreeWhy::Special);

        let root_link = Scratch::new("walk-root-link");
        root_link.dir("real/person");
        std::os::unix::fs::symlink(root_link.0.join("real"), root_link.0.join("staged")).unwrap();
        assert_eq!(
            walk_of(&root_link.0.join("staged")).unwrap_err().why,
            TreeWhy::Link,
            "the staged folder itself"
        );
    }

    /// FAILS IF: a link where the walk does NOT look stops the tree — or is
    /// opened at all; or the icons folder is walked instead of read by name.
    #[test]
    fn what_the_walk_ignores_is_never_opened() {
        let s = Scratch::new("walk-ignored");
        s.dir("person/machines");
        s.dir("person/software/icons");
        std::os::unix::fs::symlink("/etc/shadow", s.0.join("person/machines/x.json")).unwrap();
        std::os::unix::fs::symlink("/etc", s.0.join("person/notes")).unwrap();
        std::os::unix::fs::symlink("/etc/shadow", s.0.join("person/software/icons/x.png")).unwrap();
        let staged = walk_of(&s.0).unwrap();
        assert_eq!(staged.ignored, ["person/machines", "person/notes"]);
        assert!(staged.files.is_empty());
        assert!(!staged.dirs.contains("person/software/icons"));
    }

    /// FAILS IF: something wrong inside ONE brain module refuses the whole
    /// tree, an editor's dropping is opened, or what was read of the module
    /// before the problem still arrives.
    #[test]
    fn a_problem_inside_a_module_leaves_that_module_out() {
        let s = Scratch::new("walk-module");
        s.file("person/software/declared.json", b"{}");
        s.file("brain-modules/my-good/module.rs", b"fn good() {}");
        s.file("brain-modules/my-bad/a.rs", b"read before the link");
        std::os::unix::fs::symlink("/etc/passwd", s.0.join("brain-modules/my-bad/b.rs")).unwrap();
        s.file("brain-modules/my-bad/c.rs", b"after it");
        std::os::unix::fs::symlink("/etc/shadow", s.0.join("brain-modules/my-good/.#module.rs"))
            .unwrap();
        s.file("brain-modules/my-good/.module.rs.swp", b"vim");
        s.file("brain-modules/my-good/module.rs~", b"backup");
        s.dir("brain-modules/not-mine");
        let staged = walk_of(&s.0).unwrap();
        let out: Vec<_> = staged
            .left_out
            .iter()
            .map(|l| (l.module.as_str(), l.why))
            .collect();
        assert_eq!(
            out,
            [("not-mine", TreeWhy::NotAModule), ("my-bad", TreeWhy::Link)]
        );
        assert!(!staged.files.keys().any(|f| f.contains("my-bad")));
        assert!(!staged.dirs.iter().any(|d| d.contains("my-bad")));
        assert_eq!(
            staged.files["brain-modules/my-good/module.rs"],
            b"fn good() {}"
        );
        assert!(staged.files.contains_key("person/software/declared.json"));
        let mut ignored = staged.ignored.clone();
        ignored.sort();
        assert_eq!(
            ignored,
            [
                "brain-modules/my-good/.#module.rs",
                "brain-modules/my-good/.module.rs.swp",
                "brain-modules/my-good/module.rs~",
            ]
        );
    }

    #[test]
    fn a_file_over_its_cap_and_a_folder_others_may_write_are_refused() {
        let big = Scratch::new("walk-big");
        big.file(
            "person/options.json",
            &vec![b' '; caps::OPTIONS_MOST_BYTES as usize + 1],
        );
        assert_eq!(walk_of(&big.0).unwrap_err().why, TreeWhy::TooLarge);

        let open = Scratch::new("walk-open");
        open.dir("person/software");
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(
            open.0.join("person/software"),
            std::fs::Permissions::from_mode(0o777),
        )
        .unwrap();
        assert_eq!(walk_of(&open.0).unwrap_err().why, TreeWhy::Writable);
    }

    /// FAILS IF: an icon is opened that was not named, a named one that is a
    /// link is read, or a missing one refuses here instead of in `assemble`.
    #[test]
    fn icons_are_read_by_name_and_only_those() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let s = Scratch::new("walk-icons");
        s.file(
            &format!("person/software/icons/{a}.png"),
            b"\x89PNG\r\n\x1a\na",
        );
        s.file(
            &format!("person/software/icons/{b}.png"),
            &vec![0; caps::ICON_MOST_BYTES as usize + 1],
        );
        std::os::unix::fs::symlink(
            "/etc/shadow",
            s.0.join(format!("person/software/icons/{}.png", "c".repeat(64))),
        )
        .unwrap();
        let root = disk::open_root(s.0.to_str().unwrap(), whose()).unwrap();
        let mut staged = Staged::default();
        let named = [format!("{a}.png"), format!("{}.png", "d".repeat(64))];
        read_icons(&root, &named, whose(), &mut staged).unwrap();
        assert_eq!(
            staged.files.keys().collect::<Vec<_>>(),
            [&format!("person/software/icons/{a}.png")],
            "the named one that is there, and nothing else"
        );
        let linked = [format!("{}.png", "c".repeat(64))];
        assert_eq!(
            read_icons(&root, &linked, whose(), &mut Staged::default())
                .unwrap_err()
                .why,
            TreeWhy::Link
        );
        let none = Scratch::new("walk-no-icons");
        none.dir("person");
        let root = disk::open_root(none.0.to_str().unwrap(), whose()).unwrap();
        assert!(read_icons(&root, &named, whose(), &mut Staged::default()).is_ok());
    }

    /// FAILS IF: the signed pair is read through a link, past its cap, or
    /// as half a pair.
    #[test]
    fn the_signed_pair_is_read_whole_or_not_at_all() {
        let s = Scratch::new("walk-signed");
        let root = disk::open_root(s.0.to_str().unwrap(), whose()).unwrap();
        assert_eq!(read_signed(&root, whose()).unwrap(), None);
        s.file(MANIFEST, b"{}");
        assert_eq!(read_signed(&root, whose()).unwrap(), None, "half a pair");
        s.file(SIGNATURE, b"c2ln\n");
        assert_eq!(
            read_signed(&root, whose()).unwrap(),
            Some((b"{}".to_vec(), b"c2ln\n".to_vec()))
        );
        s.file(
            SIGNATURE,
            &vec![b'A'; usize::try_from(caps::SIGNATURE_MOST_BYTES).unwrap() + 1],
        );
        assert_eq!(
            read_signed(&root, whose()).unwrap_err().why,
            TreeWhy::TooLarge
        );
        std::fs::remove_file(s.0.join(SIGNATURE)).unwrap();
        std::os::unix::fs::symlink("/etc/hostname", s.0.join(SIGNATURE)).unwrap();
        assert_eq!(read_signed(&root, whose()).unwrap_err().why, TreeWhy::Link);
    }

    #[test]
    fn a_nonce_is_read_when_there_and_refused_when_not_one() {
        let s = Scratch::new("walk-nonce");
        let root = disk::open_root(s.0.to_str().unwrap(), whose()).unwrap();
        assert_eq!(read_nonce(&root, whose()).unwrap(), None);
        s.file("nonce", b"0123456789abcdef\n");
        assert_eq!(
            read_nonce(&root, whose()).unwrap().as_deref(),
            Some("0123456789abcdef")
        );
        s.file("nonce", b"not one");
        assert_eq!(
            read_nonce(&root, whose()).unwrap_err().why,
            TreeWhy::NotANonce
        );
        std::fs::remove_file(s.0.join("nonce")).unwrap();
        std::os::unix::fs::symlink("/etc/hostname", s.0.join("nonce")).unwrap();
        assert_eq!(read_nonce(&root, whose()).unwrap_err().why, TreeWhy::Link);
    }
}
