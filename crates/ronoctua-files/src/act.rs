//! Doing things to files — and the one narrow door to what needs root.
//!
//! # Every program is a store path
//!
//! `option_env!`, filled by `nix/rust.nix`, exactly as `ronoctua-rescue` does.
//! The shell runs inside a `systemd-run --scope` where `PATH` is minimal, and a
//! bare name fails far from its cause — the trap `CLAUDE.md` already records.
//! A missing variable comes back as [`Missing`](Trouble::Missing) naming the
//! VARIABLE, because that is a defect in the Nix and not on the machine.
//!
//! # What this module will not do
//!
//! It never runs as root and never asks for a password. The four operations
//! that need one are written into a directory the person owns and handed to a
//! systemd unit, which validates before acting — see [`Staged`]. Deleting and
//! moving are deliberately NOT among them: a root delete belongs in the root
//! terminal, where it is visibly what it is.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// `bsdtar`, which reads and writes more formats than the rest together.
const BSDTAR: Option<&str> = option_env!("RONOCTUA_BSDTAR");
/// `fstrim`, for the discard step of an overwriting delete.
const FSTRIM: Option<&str> = option_env!("RONOCTUA_FSTRIM");

/// Something that stopped an operation, with enough to say so on screen.
#[derive(Debug)]
pub enum Trouble {
    /// A program this module needs was not wired up in `nix/rust.nix`.
    Missing(&'static str),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(v) => write!(f, "{v} was not set in nix/rust.nix"),
        }
    }
}

impl std::error::Error for Trouble {}

/// Renames inside the same directory, or moves within one filesystem.
///
/// # Errors
///
/// When the destination exists, or the rename fails. The existence check is
/// not a race-free guarantee and is not meant to be: it is the difference
/// between a rename that usually refuses and one that always destroys.
pub fn rename(from: &Path, to: &Path) -> Result<()> {
    if to.symlink_metadata().is_ok() {
        bail!("there is already something at {}", to.display());
    }
    std::fs::rename(from, to)
        .with_context(|| format!("renomear {} para {}", from.display(), to.display()))
}

/// Copies a file, a directory tree or a symlink into a directory.
///
/// Returns where it landed.
///
/// # Errors
///
/// When the source cannot be read, the destination cannot be written, or a
/// name is already taken.
pub fn copy_into(source: &Path, directory: &Path) -> Result<PathBuf> {
    crate::watch::copy_into(source, directory, &mut crate::watch::Watch::none())
}

/// Moves into a directory, falling back to copy-then-delete across filesystems.
///
/// Unlike the trash, crossing a filesystem here is EXPECTED — moving a file to
/// another disk is a thing people mean to do — so it copies and then removes,
/// and only removes after the copy succeeded.
///
/// # Errors
///
/// When either half fails. A failure after the copy leaves both, which is the
/// safe side of that choice.
pub fn move_into(source: &Path, directory: &Path) -> Result<PathBuf> {
    crate::watch::move_into(source, directory, &mut crate::watch::Watch::none())
}

/// Deletes, whatever it is.
///
/// A directory goes recursively; a symlink goes as the LINK and never as what
/// it points at, which is the difference between deleting a shortcut and
/// deleting somebody's home directory.
///
/// # Errors
///
/// When the removal fails.
pub fn remove(path: &Path) -> Result<()> {
    crate::watch::remove(path, &mut crate::watch::Watch::none())
}

/// How many times an overwriting delete writes over the content.
///
/// Three, and the number is not folklore: one pass of a fixed pattern, one of
/// its complement, one of a varying pattern. More passes were an answer to
/// magnetic media that has not been sold in fifteen years, and on the media
/// this actually runs on the passes are not what decides the outcome — see
/// [`shred`].
pub const PASSES: u32 = 3;

/// Overwrites a file's content, then removes it.
///
/// # What this genuinely does, and what it cannot
///
/// It writes over the bytes the file currently occupies, syncs, truncates,
/// renames and unlinks. On a filesystem that writes in place and on a disk that
/// exposes its real blocks, that is destruction.
///
/// This machine has neither. **btrfs is copy-on-write**: an overwrite allocates
/// NEW extents and leaves the old ones intact until they are reclaimed, and a
/// snapshot may hold them forever. **An SSD levels wear**: the controller
/// remaps writes and the previous copy stays in a block the operating system
/// cannot name. So this is best effort, and the confirmation says so in those
/// words rather than promising an erasure it cannot perform.
///
/// What actually guarantees it is the disk encryption this system already has:
/// blocks nobody can decrypt are blocks nobody can recover. The discard pass
/// ([`trim`]) is the one step that talks to the controller, and it is offered
/// alongside.
///
/// # Errors
///
/// When the file cannot be opened for writing, or the removal fails. A
/// directory is refused: walking a tree overwriting things is a different
/// operation with a different confirmation, and doing it behind this name
/// would be a surprise.
pub fn shred(path: &Path, passes: u32) -> Result<()> {
    crate::watch::shred(path, passes, &mut crate::watch::Watch::none())
}

/// Asks the filesystem to tell the device which blocks are free.
///
/// This is the only step of an overwriting delete that reaches the controller,
/// and it needs root — so it goes through [`Staged`] like the other three.
#[must_use]
pub fn trim_request(mount: &Path) -> Staged {
    Staged {
        verb: Verb::Trim,
        path: mount.to_path_buf(),
        value: String::new(),
    }
}

/// Whether `fstrim` was wired up at all.
#[must_use]
pub const fn can_trim() -> bool {
    FSTRIM.is_some()
}

/// Changes the permission bits of a path this user owns.
///
/// # Errors
///
/// When the change is refused — which for a file somebody else owns is the
/// normal outcome, and the caller offers the privileged route instead.
pub fn chmod(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // A symlink has no permissions of its own on Linux, and `set_permissions`
    // follows: without this, "make this link read-only" would silently change
    // the target.
    if std::fs::symlink_metadata(path)
        .map(|m| m.is_symlink())
        .unwrap_or(false)
    {
        bail!("a symbolic link has no permission of its own; change the target's");
    }
    std::fs::set_permissions(path, PermissionsExt::from_mode(mode & 0o7777))
        .with_context(|| format!("mudar a permissao de {}", path.display()))
}

/// Hands a path to another group — allowed without privilege when this user
/// owns the file and belongs to the group.
///
/// # Errors
///
/// When the kernel refuses, which is what happens for a group one is not in.
pub fn chgrp(path: &Path, gid: u32) -> Result<()> {
    rustix::fs::chownat(
        rustix::fs::CWD,
        path,
        None,
        Some(rustix::fs::Gid::from_raw(gid)),
        // NOFOLLOW, always: the whole point of acting on the entry the listing
        // drew is that it is the entry the listing drew.
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )
    .with_context(|| format!("change the group of {}", path.display()))
}

/// What the privileged unit knows how to do. Four verbs, and no more.
///
/// Deleting and moving are NOT here on purpose: they are the two that cannot be
/// undone, and doing them as root behind a one-key gesture is how a file
/// manager erases somebody's system. Whoever needs that opens the root terminal,
/// where what is happening is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// Change the owner.
    Chown,
    /// Change the group, when one does not belong to it.
    Chgrp,
    /// Change the permission bits of a file one does not own.
    Chmod,
    /// Discard the free blocks of a mount point.
    Trim,
}

impl Verb {
    /// The four, for the check that compares this list against the unit's.
    pub const ALL: [Self; 4] = [Self::Chown, Self::Chgrp, Self::Chmod, Self::Trim];

    /// The word written into the staging directory, and the one the unit reads.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Chown => "chown",
            Self::Chgrp => "chgrp",
            Self::Chmod => "chmod",
            Self::Trim => "trim",
        }
    }

    /// Back from the word. An unknown one is `None` — the unit refuses rather
    /// than guessing, which is the whole reason this is an enum.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.word() == word)
    }
}

/// A request for the privileged unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    /// What to do.
    pub verb: Verb,
    /// To what.
    pub path: PathBuf,
    /// The new value: a uid, a gid, or an octal mode. Empty for `trim`.
    pub value: String,
}

/// The unit that installs a staged request. A FIXED name, so the polkit rule
/// can name it.
///
/// A templated unit taking the path as its instance would be the obvious shape
/// and it is the wrong one: `%i` is a free string, so the rule could not
/// enumerate what is allowed — it would have to match a prefix, and a prefix
/// match is a door of unknown width.
pub const UNIT: &str = "ronoctua-files-root.service";

/// Where a request is written. `RuntimeDirectory`, owned by the person.
pub const STAGING: &str = "/run/ronoctua-files/staged";

impl Staged {
    /// Writes the request out, one field per file.
    ///
    /// Three files and not one line: a path may contain a newline, a colon and
    /// a space, and any single-line encoding is a parser waiting to disagree
    /// with its writer. One field per file has nothing to parse.
    ///
    /// # Errors
    ///
    /// When the staging directory cannot be written — which on a running
    /// system means the unit's `RuntimeDirectory` was never created.
    pub fn write_into(&self, staging: &Path) -> Result<()> {
        use std::os::unix::ffi::OsStrExt;
        std::fs::create_dir_all(staging)
            .with_context(|| format!("create {}", staging.display()))?;
        std::fs::write(staging.join("verb"), self.verb.word())?;
        std::fs::write(staging.join("path"), self.path.as_os_str().as_bytes())?;
        std::fs::write(staging.join("value"), &self.value)?;
        Ok(())
    }

    /// Reads one back — what the privileged helper does.
    ///
    /// # Errors
    ///
    /// When a field is missing or the verb is not one of the four.
    pub fn read_from(staging: &Path) -> Result<Self> {
        use std::os::unix::ffi::OsStringExt;
        let word = std::fs::read_to_string(staging.join("verb")).context("ler o verbo")?;
        let verb = Verb::of_word(word.trim())
            .with_context(|| format!("verbo desconhecido: {}", word.trim()))?;
        let raw = std::fs::read(staging.join("path")).context("read the path")?;
        let value = std::fs::read_to_string(staging.join("value")).unwrap_or_default();
        Ok(Self {
            verb,
            path: PathBuf::from(std::ffi::OsString::from_vec(raw)),
            value: value.trim().to_owned(),
        })
    }
}

/// The archive formats offered, as `(extension, text key)`.
///
/// Three, and each is here for a reason somebody would give out loud: `.tar.zst`
/// is what one keeps, `.tar.gz` is what one sends to a stranger, `.zip` is what
/// one sends to somebody on Windows. A menu of eleven formats is a menu nobody
/// reads.
pub const FORMATS: [(&str, &str); 3] = [
    ("tar.zst", ronoctua_i18n::key::FILES_FORMAT_ZSTD),
    ("tar.gz", ronoctua_i18n::key::FILES_FORMAT_GZIP),
    ("zip", ronoctua_i18n::key::FILES_FORMAT_ZIP),
];

/// The command that packs `names` (relative to `directory`) into `archive`.
///
/// Returns the command rather than running it: what runs a child process and
/// waits for it is the shell, on a thread, for the reason
/// `systemd-run --scope` already taught this project — blocking the loop that
/// draws is how the footer stops responding.
///
/// # Errors
///
/// When `bsdtar` was not wired up.
pub fn packing(
    directory: &Path,
    names: &[String],
    archive: &Path,
) -> Result<std::process::Command> {
    let bsdtar = BSDTAR.ok_or(Trouble::Missing("RONOCTUA_BSDTAR"))?;
    let mut cmd = std::process::Command::new(bsdtar);
    // `-a` picks the compression from the NAME, which is what makes one code
    // path serve all three formats.
    cmd.arg("-a").arg("-c").arg("-f").arg(archive);
    // `--` and then the names, so a file called `-C` is a file and not a flag.
    cmd.arg("-C").arg(directory).arg("--");
    for n in names {
        cmd.arg(n);
    }
    Ok(cmd)
}

/// The command that unpacks `archive` into `directory`.
///
/// # Errors
///
/// When `bsdtar` was not wired up.
pub fn unpacking(archive: &Path, directory: &Path) -> Result<std::process::Command> {
    let bsdtar = BSDTAR.ok_or(Trouble::Missing("RONOCTUA_BSDTAR"))?;
    let mut cmd = std::process::Command::new(bsdtar);
    cmd.arg("-x")
        .arg("-f")
        .arg(archive)
        .arg("-C")
        .arg(directory);
    // An archive that contains `../..` writes outside the directory it was
    // opened in. `bsdtar` strips those by default and says so; asking for it
    // explicitly is what keeps a future flag change from opening it silently.
    cmd.arg("--no-absolute-paths");
    Ok(cmd)
}

/// How much a directory tree occupies.
///
/// `symlink_metadata` throughout, so a link into a large tree is twelve bytes
/// and not the tree — the rule `ronoctua_memory::scan::subtree` already
/// follows, and for the same reason: following links makes a walk count things
/// twice and, on a loop, never finish.
///
/// It reports what the FILES claim, not what the disk spends: a sparse file and
/// a compressed one both come out at their apparent size. That is the number a
/// person means by "how big is this folder"; the other one is the filesystem's,
/// and `ronoctua-memory` is where it lives.
#[must_use]
pub fn weigh(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| weigh(&e.path()))
        .fold(0u64, u64::saturating_add)
}

/// A name nothing is using, by appending a number before the extension.
pub(crate) fn free_name(wanted: &Path) -> PathBuf {
    if wanted.symlink_metadata().is_err() {
        return wanted.to_path_buf();
    }
    let stem = wanted
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = wanted
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for n in 1..10_000u32 {
        // A NAME on the person's disk, in the same English the rest of what
        // this module writes to disk is in.
        let candidate = wanted.with_file_name(format!("{stem} (copy {n}){extension}"));
        if candidate.symlink_metadata().is_err() {
            return candidate;
        }
    }
    wanted.with_file_name(format!("{stem} ({}){extension}", std::process::id()))
}

/// A short, stable hash of a path, for the blind name a shredded file wears.
pub(crate) fn fnv(path: &Path) -> u64 {
    use std::os::unix::ffi::OsStrExt;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.as_os_str().as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-act-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    #[test]
    fn renaming_over_something_is_refused() {
        let root = scratch("rename");
        std::fs::write(root.join("a"), b"a").unwrap();
        std::fs::write(root.join("b"), b"b").unwrap();
        let e = rename(&root.join("a"), &root.join("b")).unwrap_err();
        assert!(e.to_string().contains("already something at"), "{e}");
        assert_eq!(std::fs::read(root.join("b")).unwrap(), b"b");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn copying_a_link_copies_the_link() {
        // THE DEFECT THIS CATCHES: following the symlink and copying the
        // target. A twelve-byte shortcut into a gigabyte, behind a gesture
        // that promised to copy what the row showed.
        let root = scratch("link");
        std::fs::write(root.join("target"), b"large content").unwrap();
        std::os::unix::fs::symlink("target", root.join("shortcut")).unwrap();
        std::fs::create_dir(root.join("destination")).unwrap();

        let landed = copy_into(&root.join("shortcut"), &root.join("destination")).unwrap();
        let meta = std::fs::symlink_metadata(&landed).unwrap();
        assert!(
            meta.is_symlink(),
            "it copied the target instead of the link"
        );
        assert_eq!(
            std::fs::read_link(&landed).unwrap(),
            PathBuf::from("target")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_copy_into_the_same_directory_gets_a_name_of_its_own() {
        let root = scratch("same");
        std::fs::write(root.join("notes.txt"), b"x").unwrap();
        let landed = copy_into(&root.join("notes.txt"), &root).unwrap();
        assert_ne!(landed, root.join("notes.txt"));
        assert!(landed.to_string_lossy().contains("copy"));
        // And the extension survives, so the type is still recognisable.
        assert_eq!(landed.extension().unwrap(), "txt");
        assert!(root.join("notes.txt").exists(), "the original went away");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_tree_is_copied_whole() {
        let root = scratch("tree");
        std::fs::create_dir_all(root.join("source/inside")).unwrap();
        std::fs::write(root.join("source/inside/f.txt"), b"deep").unwrap();
        std::fs::create_dir(root.join("destination")).unwrap();
        copy_into(&root.join("source"), &root.join("destination")).unwrap();
        assert_eq!(
            std::fs::read(root.join("destination/source/inside/f.txt")).unwrap(),
            b"deep"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn shredding_leaves_nothing_and_takes_the_name_with_it() {
        let root = scratch("shred");
        let file = root.join("bank-passwords.txt");
        std::fs::write(&file, b"secret".repeat(5000)).unwrap();
        shred(&file, PASSES).unwrap();
        assert!(!file.exists());
        // And no directory entry is left carrying the old name.
        let left: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(left.is_empty(), "{left:?} was left over");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn shredding_a_directory_is_refused_and_a_link_is_only_unlinked() {
        let root = scratch("shred-refuse");
        std::fs::create_dir(root.join("folder")).unwrap();
        let e = shred(&root.join("folder"), 1).unwrap_err();
        assert!(e.to_string().contains("folder"), "{e}");

        // THE DEFECT THIS CATCHES: overwriting THROUGH a link, which destroys
        // the target while the person is looking at the shortcut.
        std::fs::write(root.join("target"), b"it cannot go away").unwrap();
        std::os::unix::fs::symlink("target", root.join("shortcut")).unwrap();
        shred(&root.join("shortcut"), 1).unwrap();
        assert_eq!(
            std::fs::read(root.join("target")).unwrap(),
            b"it cannot go away"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_staged_request_survives_a_path_with_a_newline_in_it() {
        // THE DEFECT THIS CATCHES: any single-line encoding. A file name may
        // contain a newline, a colon and a space, and a parser that disagrees
        // with its writer about where a field ends is a privileged operation
        // acting on the wrong path.
        let root = scratch("staged");
        let awkward = PathBuf::from("/tmp/one\nfile: with a space");
        let s = Staged {
            verb: Verb::Chown,
            path: awkward.clone(),
            value: "1000".to_owned(),
        };
        s.write_into(&root).unwrap();
        let back = Staged::read_from(&root).unwrap();
        assert_eq!(back.path, awkward);
        assert_eq!(back.verb, Verb::Chown);
        assert_eq!(back.value, "1000");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unknown_verb_is_refused_and_not_guessed() {
        let root = scratch("verb");
        std::fs::write(root.join("verb"), "rm -rf").unwrap();
        std::fs::write(root.join("path"), "/").unwrap();
        std::fs::write(root.join("value"), "").unwrap();
        let e = Staged::read_from(&root).unwrap_err();
        assert!(e.to_string().contains("desconhecido"), "{e}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_privileged_verbs_cannot_delete_or_move() {
        // The list is short BY DESIGN, and a fourth verb slipped in later is
        // exactly what nobody would notice reviewing a diff.
        for v in Verb::ALL {
            assert!(
                !matches!(v.word(), "rm" | "remove" | "mv" | "move" | "delete"),
                "verbo destrutivo na porta privilegiada: {}",
                v.word()
            );
        }
        assert_eq!(Verb::ALL.len(), 4);
    }

    #[test]
    fn every_format_has_a_key_and_an_extension() {
        for (extension, label) in FORMATS {
            assert!(!extension.is_empty());
            assert!(!label.is_empty());
        }
    }
}
