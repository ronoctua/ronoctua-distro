//! The files: the reconciling walk, one path at a time, and the ceiling
//! (feature `write`).
//!
//! # Reconcile, then follow
//!
//! At start the unit walks every root and asks the index, per path, what it
//! holds (a term lookup by id, [`Writer::stamp`]): a file whose time and size
//! match is left alone, anything else is (re)read. Every path walked is noted
//! (a 64-bit hash each — a million paths are eight megabytes, not a hundred),
//! and the ids under a root the walk did not see are deleted
//! ([`Crawler::prune`]). After that the unit follows inotify, one path or one
//! tree at a time ([`Crawler::recheck`], [`Crawler::recheck_tree`]), each
//! resolved against the disk as it is when the batch is applied — so the
//! order the events came in never matters.
//!
//! # What is never walked
//!
//! `searchSkip` (`assets/cache.nix`: `.git`, `target`, `node_modules`,
//! `.cache`, `.qlot`, `/nix`) plus the person's `skip` — a bare word is a NAME
//! anywhere, a word starting with `/` a PATH and everything under it (anything
//! else is refused where it is read: `crate::settings::skip_entry`) — and,
//! unless the person asked, hidden entries (a name that starts with `.`).
//! Neither a symbolic link to a folder (it is indexed as a file, never
//! followed) nor a mount under a root (a network share, a stick: the kernel
//! says where one begins, `STATX_ATTR_MOUNT_ROOT`) is entered; a root of its
//! own is how one is indexed — and a root IS followed, so `~/Dropbox`, a link
//! to another disk, is walked as the folder it names.
//!
//! Nor is the index's own folder ([`Rules::without`]), whatever the roots: a
//! walk that reached it would index the commit it just made, and every commit
//! would be the next batch.
//!
//! # The ceiling
//!
//! The walk commits every [`BATCH`] changes and measures the index after
//! every commit: past `searchIndexMiB` the oldest files lose their contents
//! first ([`Writer::shed`]), down to [`ROOM_PERCENT`] below it; if names alone
//! are past it, nothing new is added ([`crate::Why::Full`]) until deletions
//! make room.

use std::collections::{BTreeSet, HashSet};
use std::hash::{Hash as _, Hasher as _};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use crate::write::{Document, Writer};
use crate::{Kind, Refusal, Settings, Why};

/// Changes between two commits of a walk: the gate's number, "about ten
/// thousand documents".
pub const BATCH: usize = 10_000;

/// What a shed leaves free below the ceiling, in percent of it.
///
/// A shed rewrites the segments that held the bodies it took out — in a home
/// that sits at its ceiling, most of the index. Shedding only down TO the
/// ceiling left the next saved document over it again, and every edit after
/// that rewrote the whole index (measured: 9 to 16 MB written for a 35 KB
/// file on a 2 MiB index — gigabytes a save at `searchIndexMiB`). With this
/// room a shed pays for the edits that fill it.
pub const ROOM_PERCENT: u64 = 15;

/// Where a shed stops, for a ceiling of `ceiling_bytes`.
#[must_use]
pub const fn shed_target(ceiling_bytes: u64) -> u64 {
    ceiling_bytes - ceiling_bytes / 100 * ROOM_PERCENT
}

/// The distro's limits (`assets/cache.nix`), as the unit is started with them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// `searchIndexMiB`, in bytes: the folder's ceiling.
    pub ceiling_bytes: u64,
    /// `searchContentFileMiB`, in bytes: a larger file is indexed by name only.
    pub content_file_bytes: u64,
    /// `searchContentKiB`, in bytes: the most text kept of one file.
    pub content_bytes: usize,
    /// `searchSkip`: names, and absolute paths (a leading `/`).
    pub skip: Vec<String>,
}

/// What a walk leaves out, and whether it reads contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    names: BTreeSet<String>,
    paths: Vec<PathBuf>,
    /// The index's own folder, as the kernel knows it (device, inode): no
    /// path that names it — a link, a bind mount — is walked.
    own: Option<(u64, u64)>,
    hidden: bool,
    /// Whether contents are read.
    pub contents: bool,
    /// The largest file whose contents are read, in bytes.
    pub content_file_bytes: u64,
    /// The most text kept of one file, in bytes.
    pub content_bytes: usize,
}

impl Rules {
    /// The distro's limits and the person's settings, together.
    ///
    /// Every entry was checked where it was read (`index.json` does not
    /// parse with a bad one, and the unit does not start with a bad
    /// `--skip`); one that is neither a name nor an absolute path reaching
    /// here anyway is a caller's bug, and it is left out loudly.
    #[must_use]
    pub fn new(limits: &Limits, settings: &Settings) -> Self {
        let mut names = BTreeSet::new();
        let mut paths = Vec::new();
        for skip in limits.skip.iter().chain(&settings.skip) {
            if let Err(why) = crate::settings::skip_entry(skip) {
                tracing::error!(why, "a skip entry is left out");
            } else if skip.starts_with('/') {
                paths.push(PathBuf::from(skip));
            } else {
                names.insert(skip.clone());
            }
        }
        Self {
            names,
            paths,
            own: None,
            hidden: settings.hidden,
            contents: settings.contents,
            content_file_bytes: limits.content_file_bytes,
            content_bytes: limits.content_bytes,
        }
    }

    /// These rules, with `dir` — the index's own folder — never walked: by
    /// its path as given, as the kernel resolves it, and by its identity.
    /// `dir` exists (the writer made it).
    #[must_use]
    pub fn without(mut self, dir: &Path) -> Self {
        self.paths.push(dir.to_path_buf());
        if let Ok(real) = std::fs::canonicalize(dir)
            && real != dir
        {
            self.paths.push(real);
        }
        self.own = std::fs::metadata(dir).ok().map(|m| (m.dev(), m.ino()));
        self
    }

    /// Whether this folder is the index's own, however it is reached.
    fn is_own(&self, meta: &std::fs::Metadata) -> bool {
        meta.is_dir() && self.own == Some((meta.dev(), meta.ino()))
    }

    /// Whether `path` is left out (by its own name, or by a path above it).
    #[must_use]
    pub fn skips(&self, path: &Path) -> bool {
        if self.paths.iter().any(|p| path.starts_with(p)) {
            return true;
        }
        path.file_name().is_some_and(|n| {
            let name = n.to_string_lossy();
            (!self.hidden && name.starts_with('.')) || self.names.contains(name.as_ref())
        })
    }

    /// Whether `path` is left out by itself or by any folder above it up to
    /// `root` (for one path that arrived by inotify, not by a walk).
    #[must_use]
    pub fn skips_under(&self, root: &Path, path: &Path) -> bool {
        let mut at = Some(path);
        while let Some(p) = at {
            if p == root || !p.starts_with(root) {
                break;
            }
            if self.skips(p) {
                return true;
            }
            at = p.parent();
        }
        false
    }
}

/// The hash a walk notes a path by.
#[must_use]
pub fn noted(path: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    h.finish()
}

/// Whether `path` is where a mount begins (`None`: the kernel cannot say).
fn mount_root(path: &Path) -> Option<bool> {
    use rustix::fs::{AtFlags, StatxAttributes, StatxFlags};
    let got = rustix::fs::statx(
        rustix::fs::CWD,
        path,
        AtFlags::NO_AUTOMOUNT | AtFlags::SYMLINK_NOFOLLOW,
        StatxFlags::empty(),
    )
    .ok()?;
    got.stx_attributes_mask
        .contains(StatxAttributes::MOUNT_ROOT)
        .then(|| got.stx_attributes.contains(StatxAttributes::MOUNT_ROOT))
}

/// What a walk did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Walked {
    /// Entries looked at.
    pub entries: u64,
    /// Documents added or replaced.
    pub indexed: u64,
    /// Documents deleted.
    pub deleted: u64,
}

/// What holding the ceiling has cost so far (the unit logs it; a test holds
/// it to a number).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Held {
    /// Commits that found the index past its ceiling and shed.
    pub sheds: u64,
    /// Bytes of segments those sheds rewrote.
    pub rewritten_bytes: u64,
}

/// The writer, the rules, and the ceiling.
#[derive(Debug)]
pub struct Crawler {
    /// The index.
    pub writer: Writer,
    /// What is walked and read.
    pub rules: Rules,
    ceiling_bytes: u64,
    full: bool,
    held: Held,
}

impl Crawler {
    /// A crawler over `writer`.
    #[must_use]
    pub fn new(writer: Writer, rules: Rules, ceiling_bytes: u64) -> Self {
        Self {
            writer,
            rules,
            ceiling_bytes,
            full: false,
            held: Held::default(),
        }
    }

    /// Whether names alone reached the ceiling (nothing new is added).
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.full
    }

    /// What holding the ceiling has cost since this crawler began.
    #[must_use]
    pub const fn held(&self) -> Held {
        self.held
    }

    /// Commits, then holds the ceiling: past it, sheds contents down to
    /// [`shed_target`] — room for the edits that come next — and stops adding
    /// when names alone are past it.
    ///
    /// # Errors
    ///
    /// As [`Writer::commit`].
    pub fn commit(&mut self) -> Result<(), Refusal> {
        self.writer.commit()?;
        let size = self.writer.committed_bytes();
        if size <= self.ceiling_bytes {
            self.full = false;
            return Ok(());
        }
        let shed = self.writer.shed(shed_target(self.ceiling_bytes))?;
        self.full = shed.size_bytes > self.ceiling_bytes;
        self.held.sheds += 1;
        self.held.rewritten_bytes += shed.rewritten_bytes;
        tracing::info!(
            emptied = shed.emptied,
            rounds = shed.rounds,
            rewritten_bytes = shed.rewritten_bytes,
            size_bytes = shed.size_bytes,
            ceiling_bytes = self.ceiling_bytes,
            full = self.full,
            "the index reached its ceiling"
        );
        Ok(())
    }

    /// Commits when a batch is due.
    fn maybe_commit(&mut self) -> Result<(), Refusal> {
        if self.writer.pending() >= BATCH {
            self.commit()?;
        }
        Ok(())
    }

    /// Indexes one entry whose metadata is `meta` (not followed), unless the
    /// index already holds it as it is. `force` reads it again anyway.
    fn entry(
        &mut self,
        path: &Path,
        meta: &std::fs::Metadata,
        force: bool,
    ) -> Result<bool, Refusal> {
        let kind = if meta.is_dir() {
            Kind::Folder
        } else {
            Kind::File
        };
        let id = path.to_string_lossy();
        let mtime_s = meta.mtime();
        let size_bytes = if meta.is_dir() { 0 } else { meta.len() };
        if !force
            && let Some(stamp) = self.writer.stamp(&id)
            && stamp.kind == kind
            && stamp.mtime_s == mtime_s
            && stamp.size_bytes == size_bytes
        {
            return Ok(false);
        }
        if self.full {
            return Err(Refusal { why: Why::Full });
        }
        let files_kind = if meta.is_dir() {
            ronoctua_files::Kind::Directory
        } else if meta.file_type().is_symlink() {
            ronoctua_files::Kind::Symlink
        } else {
            ronoctua_files::Kind::File
        };
        let reads = self.rules.contents && files_kind == ronoctua_files::Kind::File;
        let mime = ronoctua_files::mime::of_path(path, files_kind, reads);
        let mut document = Document::of_path(path, kind, Some(mime), mtime_s, size_bytes);
        if reads && size_bytes > 0 && size_bytes <= self.rules.content_file_bytes {
            document.body = match ronoctua_inspect::plain::of(path, mime, self.rules.content_bytes)
            {
                Some(Ok(plain)) => Some(plain.text),
                // A file that cannot be read as text (not text after all,
                // unreadable, a program missing for `.xls`) is still found
                // by its name.
                Some(Err(_)) | None => None,
            };
        }
        self.writer.replace(&document)?;
        self.maybe_commit()?;
        Ok(true)
    }

    /// Walks `root` (reconcile): every entry indexed if it changed, its hash
    /// put in `seen`, `on_folder` called for each folder entered (the watches).
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] from the writer; a folder that cannot be read is
    /// skipped, not an error.
    pub fn walk(
        &mut self,
        root: &Path,
        seen: &mut HashSet<u64>,
        force: bool,
        on_folder: &mut dyn FnMut(&Path),
    ) -> Result<Walked, Refusal> {
        let mut walked = Walked::default();
        // The root is FOLLOWED (a root may be a link to the folder it names);
        // what is under it is not.
        let Ok(meta) = std::fs::metadata(root) else {
            return Ok(walked);
        };
        if self.rules.is_own(&meta) {
            return Ok(walked);
        }
        seen.insert(noted(&root.to_string_lossy()));
        walked.entries += 1;
        if self.entry_counted(root, &meta, force)? {
            walked.indexed += 1;
        }
        if !meta.is_dir() {
            return Ok(walked);
        }
        let mut stack = vec![root.to_path_buf()];
        while let Some(folder) = stack.pop() {
            on_folder(&folder);
            let Ok(entries) = std::fs::read_dir(&folder) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if self.rules.skips(&path) {
                    continue;
                }
                let Ok(meta) = entry.metadata() else {
                    continue;
                };
                let ft = meta.file_type();
                if !(ft.is_dir() || ft.is_file() || ft.is_symlink()) {
                    // Sockets, pipes, devices: nothing a person opens.
                    continue;
                }
                if ft.is_dir() && (self.rules.is_own(&meta) || mount_root(&path) == Some(true)) {
                    continue;
                }
                seen.insert(noted(&path.to_string_lossy()));
                walked.entries += 1;
                if self.entry_counted(&path, &meta, force)? {
                    walked.indexed += 1;
                }
                if ft.is_dir() {
                    stack.push(path);
                }
            }
        }
        Ok(walked)
    }

    /// [`Crawler::entry`], with a full index answering "not indexed" rather
    /// than failing the walk (the walk goes on: what is there is noted, so
    /// the prune keeps it).
    fn entry_counted(
        &mut self,
        path: &Path,
        meta: &std::fs::Metadata,
        force: bool,
    ) -> Result<bool, Refusal> {
        match self.entry(path, meta, force) {
            Err(Refusal { why: Why::Full }) => Ok(false),
            other => other,
        }
    }

    /// Deletes every path id under one of `roots` that `seen` does not hold,
    /// and every path id under no root at all (a root the person removed).
    ///
    /// # Errors
    ///
    /// As [`Writer::delete`].
    pub fn prune(&mut self, roots: &[PathBuf], seen: &HashSet<u64>) -> Result<u64, Refusal> {
        let mut gone = Vec::new();
        self.writer.each_id("/", |id| {
            let under_a_root = roots.iter().any(|r| Path::new(id).starts_with(r));
            if !under_a_root || !seen.contains(&noted(id)) {
                gone.push(id.to_owned());
            }
        });
        for id in &gone {
            self.writer.delete(id)?;
        }
        self.maybe_commit()?;
        Ok(gone.len() as u64)
    }

    /// One path, as the disk has it now: indexed when it is there, deleted
    /// when it is not — and left alone when the rules leave it out (it was
    /// never indexed; a change of the rules walks everything again). For a
    /// file event.
    ///
    /// # Errors
    ///
    /// As [`Writer::replace`].
    pub fn recheck(&mut self, root: &Path, path: &Path) -> Result<(), Refusal> {
        if self.rules.skips_under(root, path) {
            return Ok(());
        }
        match std::fs::symlink_metadata(path) {
            Ok(meta) => {
                let ft = meta.file_type();
                if ft.is_dir() || ft.is_file() || ft.is_symlink() {
                    self.entry_counted(path, &meta, false)?;
                }
                Ok(())
            }
            Err(_) => self.writer.delete(&path.to_string_lossy()),
        }
    }

    /// One folder, as the disk has it now: walked (and what it no longer holds
    /// deleted) when it is there, deleted with everything under it when it is
    /// not. For a folder created, moved in or out, or an overflowed watch.
    ///
    /// # Errors
    ///
    /// As [`Crawler::walk`].
    pub fn recheck_tree(
        &mut self,
        root: &Path,
        path: &Path,
        on_folder: &mut dyn FnMut(&Path),
    ) -> Result<Walked, Refusal> {
        // A root is followed, as the walk follows it.
        let meta = if path == root {
            std::fs::metadata(path)
        } else {
            std::fs::symlink_metadata(path)
        };
        let there = meta.is_ok_and(|m| m.is_dir());
        if !there || self.rules.skips_under(root, path) {
            let deleted = self.remove_tree(path)?;
            return Ok(Walked {
                deleted,
                ..Walked::default()
            });
        }
        let mut seen = HashSet::new();
        let mut walked = self.walk(path, &mut seen, false, on_folder)?;
        let mut gone = Vec::new();
        let prefix = format!("{}/", path.to_string_lossy().trim_end_matches('/'));
        self.writer.each_id(&prefix, |id| {
            if !seen.contains(&noted(id)) {
                gone.push(id.to_owned());
            }
        });
        for id in &gone {
            self.writer.delete(id)?;
        }
        walked.deleted = gone.len() as u64;
        Ok(walked)
    }

    /// Deletes `path` and every id under it.
    ///
    /// # Errors
    ///
    /// As [`Writer::delete`].
    pub fn remove_tree(&mut self, path: &Path) -> Result<u64, Refusal> {
        let id = path.to_string_lossy().into_owned();
        let prefix = format!("{}/", id.trim_end_matches('/'));
        let mut gone = vec![id];
        self.writer.each_id(&prefix, |id| gone.push(id.to_owned()));
        for id in &gone {
            self.writer.delete(id)?;
        }
        Ok(gone.len() as u64)
    }
}
