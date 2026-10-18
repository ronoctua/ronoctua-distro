//! The file manager's engine — the only crate that touches the filesystem on
//! its behalf.
//!
//! # Why a crate of its own
//!
//! The same reason `ronoctua-scroll` is the only one that speaks the compositor's IPC,
//! `ronoctua-network` the only one that speaks NetworkManager and
//! `ronoctua-memory` the only one that reads `/proc`: one border, one crate.
//! Nothing here knows Wayland, wgpu or the display list, which is what lets a
//! directory listing, a permission explanation and the trash round-trip be
//! tested against a fake tree instead of a booted machine.
//!
//! # Every root is a parameter
//!
//! [`who::Roster::read_from`] takes the path of `passwd` and `group`;
//! [`trash::Bin::at`] takes the trash directory; [`when::Zone::read_from`] takes
//! the path of `localtime`. None of them reaches for a well-known location on
//! its own. It is the arrangement `ronoctua_memory::Places` already uses, and
//! for the same reason.
//!
//! # What it does NOT do
//!
//! It does not draw, does not translate and does not run a privileged
//! operation. A label comes out as a text KEY plus arguments, and whoever has
//! the bundle turns it into a sentence (invariant 8). A change that needs root
//! comes out as a [`act::Staged`] request, and whoever installs it is a systemd
//! unit that validates first — this crate never runs as root and never sees a
//! password.

#![forbid(unsafe_code)]

use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub mod act;
pub mod archive;
pub mod key;
pub mod listing;
pub mod menu;
pub mod mime;
pub mod mode;
pub mod notice;
pub mod sidebar;
pub mod sidebar_menu;
pub mod size;
pub mod trash;
pub mod watch;
pub mod when;
pub mod who;

pub use listing::{Order, read_dir_at};

/// What an entry IS, as the inode says — never as the name suggests.
///
/// A symlink stays [`Kind::Symlink`] here even when it points at a directory.
/// Collapsing the two would make "delete this" ambiguous in the one case where
/// the difference is the whole question: removing a link and removing what it
/// points at are different acts, and a listing that hides which one it is
/// invites the wrong one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A directory: a place, not a thing. Listings put these first.
    Directory,
    /// A regular file.
    File,
    /// A symbolic link. Where it points is in [`Entry::target`].
    Symlink,
    /// A named pipe.
    Fifo,
    /// A unix socket.
    Socket,
    /// A block device.
    Block,
    /// A character device.
    Char,
}

impl Kind {
    /// From the `st_mode` bits.
    ///
    /// An unrecognised type becomes [`Kind::File`] rather than an error: a file
    /// nobody can name is still a file one can see the size of, and dropping the
    /// row would be the listing lying about what the directory holds.
    #[must_use]
    pub const fn of_mode(mode: u32) -> Self {
        match mode & 0o170_000 {
            0o040_000 => Self::Directory,
            0o120_000 => Self::Symlink,
            0o010_000 => Self::Fifo,
            0o140_000 => Self::Socket,
            0o060_000 => Self::Block,
            0o020_000 => Self::Char,
            _ => Self::File,
        }
    }

    /// The glyph for this kind, before the file's type is consulted.
    ///
    /// Only the kinds that are not "a file with content" answer here; a regular
    /// file returns `None` so the MIME table decides, which is the one place
    /// that can tell a photograph from a spreadsheet.
    ///
    /// A `char` out of `ronoctua_tokens::icon`, never a literal: the codepoint
    /// is resolved at build time against the real font's `cmap` (invariants 9
    /// and 10).
    #[must_use]
    pub const fn icon(self) -> Option<char> {
        use ronoctua_tokens::icon;
        match self {
            Self::Directory => Some(icon::FOLDER),
            Self::Symlink => Some(icon::LINK),
            Self::Fifo => Some(icon::PIPE),
            Self::Socket => Some(icon::SOCKET),
            Self::Block | Self::Char => Some(icon::DISK),
            Self::File => None,
        }
    }
}

/// One row of a directory, with everything the screen shows.
///
/// The fields are what `lstat` gave, unconverted: the formatting of a size, of
/// a date and of a permission belongs to whoever knows the language and the
/// width, and doing it here would freeze both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The name inside its directory, never the whole path.
    pub name: String,
    /// What the inode says it is.
    pub kind: Kind,
    /// `st_mode`, whole. [`mode`] takes it apart.
    pub mode: u32,
    /// How many names this inode has.
    pub links: u64,
    /// The owner's numeric id. [`who::Roster`] turns it into a name.
    pub uid: u32,
    /// The group's numeric id.
    pub gid: u32,
    /// The size in bytes. For a directory this is the directory's own size,
    /// which is not what a person means by "how big is this folder" — that one
    /// costs a walk and is asked for deliberately.
    pub bytes: u64,
    /// Last modification, in seconds since the epoch. [`when`] formats it.
    pub modified: i64,
    /// Where a symlink points, verbatim — relative links stay relative.
    pub target: Option<String>,
    /// What the symlink resolves to, or `None` when it resolves to nothing.
    ///
    /// A broken link is the one state a listing must not hide: it looks exactly
    /// like a working one in every other column.
    pub aims_at: Option<Kind>,
}

impl Entry {
    /// The name of the row that goes back up, and the only name this crate
    /// reserves.
    pub const PARENT: &'static str = "..";

    /// The name a listing DRAWS: the name itself — or, for an entry the
    /// merged trash brought in from a volume's own bin, whose name is its
    /// whole path (so that `folder.join(name)` still lands on it), the last
    /// component of that path.
    ///
    /// A path and not a second field: every place that turns an entry into a
    /// path already writes `folder.join(&entry.name)`, and `join` with an
    /// absolute path IS that path. Only what is drawn and what is sorted by
    /// needs to know, and they ask here.
    #[must_use]
    pub fn shown(&self) -> &str {
        if self.name.starts_with('/') {
            Path::new(&self.name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&self.name)
        } else {
            &self.name
        }
    }

    /// Is this the row that goes back up a level?
    ///
    /// # Why it is a row and not a key
    ///
    /// It is BOTH — `C-b` and the breadcrumbs have always gone up. The owner
    /// asked for the row as well (*"crie a pasta `..` para retornar no path"*),
    /// and the row is the one of the three a person finds without being told.
    ///
    /// It is also the one entry in a listing that nothing may be DONE to:
    /// renaming, deleting, copying or measuring `..` would act on the
    /// directory one is standing in, under a name that says otherwise. The
    /// shell refuses it at the four places that could act — `View::row_of`
    /// gives the whole row one target, `menu_subject` hands the menu the
    /// FOLDER instead, and marking and measuring say no.
    #[must_use]
    pub fn is_parent(&self) -> bool {
        self.name == Self::PARENT
    }

    /// Executable by somebody — any of the three `x` bits.
    #[must_use]
    pub const fn executable(&self) -> bool {
        self.mode & 0o111 != 0
    }

    /// A symlink that points at nothing.
    #[must_use]
    pub const fn broken(&self) -> bool {
        matches!(self.kind, Kind::Symlink) && self.aims_at.is_none()
    }

    /// What this entry behaves as when opened: the target's kind for a symlink,
    /// its own otherwise.
    ///
    /// Opening is the one operation that SHOULD follow the link — that is what
    /// a link is for. Everything else (delete, chmod, chown) acts on the link
    /// itself.
    #[must_use]
    pub const fn opens_as(&self) -> Kind {
        match self.aims_at {
            Some(k) if matches!(self.kind, Kind::Symlink) => k,
            _ => self.kind,
        }
    }

    /// Built from a path's own metadata — `lstat`, never `stat`.
    ///
    /// `symlink_metadata` and not `metadata` is the same rule
    /// `ronoctua_memory::scan::subtree` follows: following a link makes a walk
    /// count the target twice and, worse, makes a delete act somewhere else.
    /// `pub` because the shell needs one for a directory that has no row: an
    /// empty listing still has a menu, and what it is about is the directory
    /// itself. See `files::View::menu_subject`.
    #[must_use]
    pub fn of_path(name: String, path: &Path) -> Option<Self> {
        let meta = std::fs::symlink_metadata(path).ok()?;
        let mode = meta.mode();
        let kind = Kind::of_mode(mode);
        let (target, aims_at) = if matches!(kind, Kind::Symlink) {
            let target = std::fs::read_link(path)
                .ok()
                .map(|p| p.to_string_lossy().into_owned());
            // The SECOND stat, and only for links: this one follows, and it is
            // how a broken link is told from a working one.
            let aims_at = std::fs::metadata(path)
                .ok()
                .map(|m| Kind::of_mode(m.mode()));
            (target, aims_at)
        } else {
            (None, None)
        };
        Some(Self {
            name,
            kind,
            mode,
            links: meta.nlink(),
            uid: meta.uid(),
            gid: meta.gid(),
            bytes: meta.size(),
            modified: meta.mtime(),
            target,
            aims_at,
        })
    }
}
