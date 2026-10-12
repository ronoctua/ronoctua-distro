//! The person's tree as root copies it: what a staged tree may hold, and the
//! machine's `person/` that is made of it.
//!
//! # The two shapes
//!
//! ```text
//! staged (the person's, 0700)             the machine's person/ (root's)
//!   person/software/declared.json    →      software/declared.json   rewritten canonical
//!   person/software/declined.json    →      software/declined.json   rewritten canonical
//!   person/software/icons/<h>.png    →      software/icons/<h>.png   only the ones an entry names
//!   person/options.json              →      options.json             rewritten canonical
//!   person/machines/…                       —  (the machines' records: Nix reads none)
//!   brain-modules/<my-id>/…          →      brain/<my-id>/…          byte for byte
//!   signed.json, signed.sig                 —  (what the person signed: held to the pin)
//!   nonce                                   —  (which staging this is: said in the verdict)
//! ```
//!
//! The staged shape is the repository's `plain/` — so the installer hands its
//! clone of it to the same applier the machine runs, and the person's own
//! processes copy their mirror (`$XDG_STATE_HOME/ronoctua/person/`, and
//! `$XDG_CONFIG_HOME/ronoctua/brain-modules/`) into it.
//!
//! # Data, and the one kind of code
//!
//! The documents are data: each is parsed, validated and written back in the
//! one canonical form, so root writes only what it understood. A brain
//! module is the person's CODE and is copied byte for byte; its Nix part
//! enters a build only when its hash is in `local/approved.json`, which only
//! the sudo door writes ([`crate::approve`]).
//!
//! # What refuses the tree, and what refuses one module
//!
//! A document, the icons the entries name and the caps on the whole are the
//! declaration: any of them wrong refuses the tree, and the machine keeps the
//! copy it had. A brain module is the person's working folder, where an
//! editor leaves its droppings: a hidden name or a backup (`.x.swp`, `x~`,
//! `#x#`, an Emacs lock link `.#x`) is LEFT OUT and never opened, and
//! anything else wrong inside one module — a link, a strange name, a file
//! over its cap — leaves THAT module out ([`LeftOut`]): the machine keeps its
//! previous copy of it, and every other module and every declaration still
//! arrive.
//!
//! # What the person signed, held to root's pin
//!
//! A machine that pinned the person's signing key (`local/signer.json`,
//! [`crate::local::Signer`]) takes from a staging ONLY what the person
//! signed: the staging carries the manifest of the person's plain part and
//! its ML-DSA-65 signature ([`MANIFEST`], [`SIGNATURE`]), the binary opens
//! it with the pinned key and refuses the whole tree when the signature does
//! not hold, is missing, or names a counter lower than the last one this
//! machine took ([`TreeWhy::NotSigned`], [`TreeWhy::BadSignature`],
//! [`TreeWhy::RolledBack`]). A signature that holds still vouches file by
//! file ([`hold_to_signature`]): a document whose bytes are not the ones
//! signed is KEPT BACK — the machine keeps its own copy — and a brain module
//! with one file not as signed is left out, as a module that cannot be read
//! is ([`TreeWhy::NotAsSigned`]). So a change of this machine's the person
//! has not signed yet (a pin the update moved while their identity was
//! locked) waits, and everything signed still arrives.
//!
//! A machine with NO key pinned takes the declarations as the person staged
//! them — a person without a repository has nothing to sign with — but never
//! their options nor a brain module ([`TreeWhy::NoKeyPinned`]): what changes
//! root's own configuration, and the code the brain compiles, need the
//! person's signature, always.
//!
//! # Which staging this is
//!
//! `systemctl start` of a running oneshot joins the job already running, so
//! two stagers could be answered by one run. A stager that cares writes a
//! [`NONCE`] last (and removes the old one first); the applier reads it
//! before and after the walk — a different one means the tree was staged
//! again while it was read ([`TreeWhy::Restaged`]) — and the verdict carries
//! it. A verdict whose nonce is not yours is not your answer.
//!
//! # Pure
//!
//! [`staged_place`] says, from a path alone, whether the walker reads it,
//! walks it or leaves it; [`named_icons`] says which icons to read once the
//! declarations are; [`assemble`] turns what was read into the machine's
//! tree or refuses. Who opens the files (with `O_NOFOLLOW`, as root, under
//! [`caps`]) is the binary's business.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::declared::{Declared, Entry};
use crate::declined::Declined;
use crate::options::Options;
use crate::{Document, Refusal, Stamp, check, names};

/// The staged folder of the person's data (the repository's `plain/person`).
pub const PERSON: &str = "person";
/// The staged folder of the person's brain modules (`plain/brain-modules`).
pub const BRAIN_MODULES: &str = "brain-modules";
/// Where the brain modules land under the machine's `person/`.
pub const BRAIN: &str = "brain";
/// The staged file that names this staging: one [`nonce_of`] token.
pub const NONCE: &str = "nonce";
/// The staged manifest the person signed of their plain part
/// (`ronoctua_pq::sign::Manifest`, the repository's `plain/signed.json`):
/// every file by its path under the staged root, with its SHA-256.
pub const MANIFEST: &str = "signed.json";
/// The staged detached signature of [`MANIFEST`] (`plain/signed.sig`).
pub const SIGNATURE: &str = "signed.sig";

/// How much root reads, and how deep. Each cap refuses rather than
/// truncating: a declaration cut in half is a different declaration.
pub mod caps {
    /// `software/declared.json`.
    pub const DECLARED_MOST_BYTES: u64 = 1 << 20;
    /// `software/declined.json`.
    pub const DECLINED_MOST_BYTES: u64 = 256 << 10;
    /// `options.json`.
    pub const OPTIONS_MOST_BYTES: u64 = 16 << 10;
    /// One icon. The machine's flake is copied whole into the store at every
    /// evaluation (`path:`), so its icons stay small.
    pub const ICON_MOST_BYTES: u64 = 256 << 10;
    /// The icons the entries name.
    pub const ICONS_MOST: usize = 512;
    /// The person's brain modules.
    pub const MODULES_MOST: usize = 64;
    /// The files of one module.
    pub const MODULE_FILES_MOST: usize = 256;
    /// One file of a module.
    pub const MODULE_FILE_MOST_BYTES: u64 = 1 << 20;
    /// Path components under the staged root (`brain-modules/<id>/a/b/c/f`).
    pub const DEPTH_MOST: usize = 6;
    /// Every entry a walk may meet, ignored ones included.
    pub const ENTRIES_MOST: usize = 4096;
    /// Everything read, together.
    pub const TREE_MOST_BYTES: u64 = 16 << 20;
    /// One name, in bytes.
    pub const NAME_MOST: usize = 128;
    /// The nonce file.
    pub const NONCE_MOST_BYTES: u64 = 128;
    /// The manifest: [`ENTRIES_MOST`] paths with their hashes fit well.
    pub const MANIFEST_MOST_BYTES: u64 = 2 << 20;
    /// The signature, in base64: 3309 bytes are 4412 characters.
    pub const SIGNATURE_MOST_BYTES: u64 = 8 << 10;
}

/// Why a tree (or one module of it) was refused, as a closed set. The five
/// of a document ([`crate::Why`]) keep their own names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum TreeWhy {
    /// Nothing was staged: no `person/` folder. An empty staging folder is
    /// what every boot leaves, and applying it would take every declaration
    /// away — so it is refused, never applied.
    NothingStaged,
    /// A symbolic link where the walk looked.
    Link,
    /// A file with a second name (a hard link).
    HardLink,
    /// A pipe, a socket or a device.
    Special,
    /// A file where a folder belongs, or a folder where a file does.
    Misplaced,
    /// Something the person does not own.
    NotOwned,
    /// A folder or file that somebody else may write.
    Writable,
    /// A name that is not plain (in a brain module), or not text.
    BadName,
    /// A folder under `brain-modules/` whose name is not `my-<slug>`.
    NotAModule,
    /// A file over its cap, or the tree over its total.
    TooLarge,
    /// More entries, icons, modules or files than their cap.
    TooMany,
    /// Deeper than [`caps::DEPTH_MOST`].
    TooDeep,
    /// An icon that is not a PNG.
    NotPng,
    /// An entry names an icon the tree does not carry.
    MissingIcon,
    /// Something could not be read.
    Unreadable,
    /// The machine directory could not be written, or is not root's alone.
    Unwritable,
    /// Another of root's programs (the update, an approval) held the lock
    /// longer than [`crate::root::LOCK_WAIT_S`].
    Busy,
    /// The tree was staged again while it was being read: its [`NONCE`]
    /// changed, appeared or went away.
    Restaged,
    /// The [`NONCE`] file does not hold a nonce.
    NotANonce,
    /// A key is pinned and the staging carries no signature ([`MANIFEST`] or
    /// [`SIGNATURE`] missing).
    NotSigned,
    /// The signature is not the pinned key's over the manifest, or the
    /// manifest does not read.
    BadSignature,
    /// The manifest's counter is lower than the last one this machine took:
    /// an older signed tree, played again.
    RolledBack,
    /// A file is not what the person signed — its bytes differ, the manifest
    /// does not name it, or it names one the staging does not carry. The
    /// machine keeps its own copy (a document kept back, a module left out).
    NotAsSigned,
    /// No key is pinned on this machine: the person's options and brain
    /// modules wait for one (the machine keeps its own copies).
    NoKeyPinned,
    /// The pinned key, or a key offered to be pinned, does not read as one.
    BadKey,
    /// A document was refused, for its own reason.
    Document(crate::Why),
}

impl TreeWhy {
    /// Every one, in the order the names are written.
    pub const ALL: [Self; 30] = [
        Self::NothingStaged,
        Self::Link,
        Self::HardLink,
        Self::Special,
        Self::Misplaced,
        Self::NotOwned,
        Self::Writable,
        Self::BadName,
        Self::NotAModule,
        Self::TooLarge,
        Self::TooMany,
        Self::TooDeep,
        Self::NotPng,
        Self::MissingIcon,
        Self::Unreadable,
        Self::Unwritable,
        Self::Busy,
        Self::Restaged,
        Self::NotANonce,
        Self::NotSigned,
        Self::BadSignature,
        Self::RolledBack,
        Self::NotAsSigned,
        Self::NoKeyPinned,
        Self::BadKey,
        Self::Document(crate::Why::NotJson),
        Self::Document(crate::Why::Newer),
        Self::Document(crate::Why::Shape),
        Self::Document(crate::Why::Invalid),
        Self::Document(crate::Why::Duplicate),
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NothingStaged => "nothing-staged",
            Self::Link => "link",
            Self::HardLink => "hard-link",
            Self::Special => "special",
            Self::Misplaced => "misplaced",
            Self::NotOwned => "not-owned",
            Self::Writable => "writable",
            Self::BadName => "bad-name",
            Self::NotAModule => "not-a-module",
            Self::TooLarge => "too-large",
            Self::TooMany => "too-many",
            Self::TooDeep => "too-deep",
            Self::NotPng => "not-png",
            Self::MissingIcon => "missing-icon",
            Self::Unreadable => "unreadable",
            Self::Unwritable => "unwritable",
            Self::Busy => "busy",
            Self::Restaged => "restaged",
            Self::NotANonce => "not-a-nonce",
            Self::NotSigned => "not-signed",
            Self::BadSignature => "bad-signature",
            Self::RolledBack => "rolled-back",
            Self::NotAsSigned => "not-as-signed",
            Self::NoKeyPinned => "no-key-pinned",
            Self::BadKey => "bad-key",
            Self::Document(why) => why.id(),
        }
    }

    /// The reason of this name; `None` for any other.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }

    /// Whether it refuses a staging for its signature as a whole — what
    /// root records beside its pin, and the machine's report says.
    #[must_use]
    pub const fn refuses_the_signature(self) -> bool {
        matches!(
            self,
            Self::NotSigned | Self::BadSignature | Self::RolledBack
        )
    }

    /// Whether it keeps one file back (the machine keeps its own copy) while
    /// the rest of the staging arrives.
    #[must_use]
    pub const fn keeps_back(self) -> bool {
        matches!(self, Self::NotAsSigned | Self::NoKeyPinned)
    }
}

impl From<TreeWhy> for String {
    fn from(why: TreeWhy) -> Self {
        why.id().to_owned()
    }
}

impl TryFrom<String> for TreeWhy {
    type Error = String;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        Self::of_id(&id).ok_or_else(|| format!("not a reason: {id:?}"))
    }
}

/// A refused tree: why, where (a path under the staged root, and a JSON
/// pointer after `#` for a document) and what was found — English, for the
/// journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// Why.
    pub why: TreeWhy,
    /// Where.
    pub at: String,
    /// What was found.
    pub detail: String,
}

impl Refused {
    /// A refusal for `why` at `at`.
    #[must_use]
    pub fn new(why: TreeWhy, at: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            why,
            at: at.into(),
            detail: detail.into(),
        }
    }

    /// A document's refusal, placed in its file.
    #[must_use]
    pub fn of_document(file: &str, refusal: Refusal) -> Self {
        Self {
            why: TreeWhy::Document(refusal.why),
            at: format!("{file}#{}", refusal.at),
            detail: refusal.detail,
        }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at '{}': {}", self.why.id(), self.at, self.detail)
    }
}

impl std::error::Error for Refused {}

/// What an entry of a walked folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A folder.
    Dir,
    /// A regular file.
    File,
}

/// What the walker does with a staged entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// A folder it opens and lists.
    Walk,
    /// A file it reads, up to this many bytes.
    Read {
        /// The cap.
        most_bytes: u64,
    },
    /// Neither: it is named in the verdict and never opened.
    Ignore,
    /// Neither, and not named either: a narrower read takes it apart from
    /// the walk — the icons, by the names the declarations give, and the
    /// [`NONCE`], before the walk and after it.
    Apart,
}

/// A name a module's file or folder may have: letters, digits, `.`, `_`,
/// `+` and `-`, not starting with `.`, at most [`caps::NAME_MOST`] bytes —
/// a subset of what `nix/person-hash.nix` hashes, so a machine's copy never
/// holds a name the hash throws on.
#[must_use]
pub fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= caps::NAME_MOST
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'+' | b'-'))
}

/// What an editor or a version-control tool leaves in a working folder and
/// nobody means as part of a module: a hidden name (`.x.swp`, `.git`, an
/// Emacs lock `.#x`), an auto-save `#x#`, a backup `x~`.
#[must_use]
pub fn dropping(name: &str) -> bool {
    name.starts_with('.') || name.starts_with('#') || name.ends_with('~')
}

/// An icon's file name: `<sha256 hex>.png`.
fn icon_name(name: &str) -> bool {
    name.strip_suffix(".png").is_some_and(check::sha256_hex)
}

/// The brain module a staged path is inside (`brain-modules/<id>/…`, the
/// folder itself included), whatever its name.
#[must_use]
pub fn module_of(rel: &str) -> Option<&str> {
    let rest = rel.strip_prefix(BRAIN_MODULES)?.strip_prefix('/')?;
    let id = rest.split('/').next().unwrap_or(rest);
    (!id.is_empty()).then_some(id)
}

/// What the walker does with the entry at `rel` (`/`-joined, under the staged
/// root, never empty), which is a `kind`.
///
/// # Errors
///
/// [`TreeWhy::Misplaced`] for a file where a folder belongs or the other way
/// round; [`TreeWhy::NotAModule`], [`TreeWhy::BadName`] and
/// [`TreeWhy::TooDeep`] under `brain-modules/` — which leave that module
/// out, not the tree.
pub fn staged_place(rel: &str, kind: Kind) -> Result<Place, Refused> {
    let parts: Vec<&str> = rel.split('/').collect();
    let misplaced = || {
        Err(Refused::new(
            TreeWhy::Misplaced,
            rel,
            match kind {
                Kind::Dir => "a folder where a file belongs",
                Kind::File => "a file where a folder belongs",
            },
        ))
    };
    let folder = |want: Kind, place: Place| {
        if kind == want { Ok(place) } else { misplaced() }
    };
    match parts.as_slice() {
        [PERSON] | [BRAIN_MODULES] | [PERSON, "software"] => folder(Kind::Dir, Place::Walk),
        [NONCE] | [MANIFEST] | [SIGNATURE] | [PERSON, "software", "icons", ..] => Ok(Place::Apart),
        [PERSON, "software", "declared.json"] => folder(
            Kind::File,
            Place::Read {
                most_bytes: caps::DECLARED_MOST_BYTES,
            },
        ),
        [PERSON, "software", "declined.json"] => folder(
            Kind::File,
            Place::Read {
                most_bytes: caps::DECLINED_MOST_BYTES,
            },
        ),
        [PERSON, "options.json"] => folder(
            Kind::File,
            Place::Read {
                most_bytes: caps::OPTIONS_MOST_BYTES,
            },
        ),
        [BRAIN_MODULES, id] if kind == Kind::File || dropping(id) => Ok(Place::Ignore),
        [BRAIN_MODULES, id] => {
            if check::person_module(id) {
                Ok(Place::Walk)
            } else {
                Err(Refused::new(
                    TreeWhy::NotAModule,
                    rel,
                    "a person's brain module is named my-<letters, digits and ->",
                ))
            }
        }
        [BRAIN_MODULES, _, .., last] if dropping(last) => Ok(Place::Ignore),
        [BRAIN_MODULES, _, rest @ ..] => {
            if parts.len() > caps::DEPTH_MOST {
                return Err(Refused::new(
                    TreeWhy::TooDeep,
                    rel,
                    format!("deeper than {} levels", caps::DEPTH_MOST),
                ));
            }
            if let Some(bad) = rest.iter().find(|n| !plain_name(n)) {
                return Err(Refused::new(TreeWhy::BadName, rel, format!("{bad:?}")));
            }
            Ok(match kind {
                Kind::Dir => Place::Walk,
                Kind::File => Place::Read {
                    most_bytes: caps::MODULE_FILE_MOST_BYTES,
                },
            })
        }
        _ => Ok(Place::Ignore),
    }
}

/// Whether the person's signature covers the staged path `rel`: everything
/// under [`PERSON`] and [`BRAIN_MODULES`] but the machines' records
/// (`person/machines/…`), which each machine publishes by itself, without
/// the person's identity — and which nothing reads as configuration. The
/// repository's `plain/` has the same shape, so the belongings sign by this
/// rule too. Pure.
#[must_use]
pub fn signed(rel: &str) -> bool {
    let under = |folder: &str| rel.strip_prefix(folder).is_some_and(|r| r.starts_with('/'));
    let machines = format!("{PERSON}/{}", names::MACHINES);
    (under(PERSON) || under(BRAIN_MODULES)) && rel != machines && !under(&machines)
}

/// The nonce a [`NONCE`] file holds: 8 to 64 letters, digits, `_` or `-`,
/// and at most one line end after it.
///
/// # Errors
///
/// [`TreeWhy::NotANonce`] for anything else.
pub fn nonce_of(bytes: &[u8]) -> Result<String, Refused> {
    let text = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let ok = (8..=64).contains(&text.len())
        && text
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'));
    if !ok {
        return Err(Refused::new(
            TreeWhy::NotANonce,
            NONCE,
            "a nonce is 8 to 64 letters, digits, _ or -",
        ));
    }
    Ok(String::from_utf8_lossy(text).into_owned())
}

/// Whether the staging read is one staging: the nonce before the walk and
/// after it are the same (both absent included — a stager that does not
/// care races with nobody).
///
/// # Errors
///
/// [`TreeWhy::Restaged`] when they differ.
pub fn same_staging(before: Option<&str>, after: Option<&str>) -> Result<(), Refused> {
    if before == after {
        Ok(())
    } else {
        Err(Refused::new(
            TreeWhy::Restaged,
            NONCE,
            format!("staged again while it was read ({before:?}, then {after:?})"),
        ))
    }
}

/// A brain module left out of an apply: the rest of the tree arrived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LeftOut {
    /// The module's folder name under `brain-modules/` (not always a
    /// `my-…`: a folder misnamed is left out for it).
    pub module: String,
    /// Why.
    pub why: TreeWhy,
    /// Where, under the staged root.
    pub at: String,
    /// What was found, English.
    pub detail: String,
    /// Whether the machine keeps the copy of it it had (an apply never takes
    /// away a module only because it could not read the new one).
    pub kept: bool,
}

impl LeftOut {
    /// The module `module` left out for `refused`, not (yet) kept.
    #[must_use]
    pub fn of(module: &str, refused: Refused) -> Self {
        Self {
            module: module.to_owned(),
            why: refused.why,
            at: refused.at,
            detail: refused.detail,
            kept: false,
        }
    }
}

/// A file an apply did not take because the person's signature did not
/// vouch for it — the machine kept its own copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct KeptBack {
    /// Where, under the staged root (`person/options.json`).
    pub at: String,
    /// [`TreeWhy::NotAsSigned`] or [`TreeWhy::NoKeyPinned`].
    pub why: TreeWhy,
}

/// What the walker read of a staged tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Staged {
    /// The folders it walked (`/`-joined, under the staged root).
    pub dirs: BTreeSet<String>,
    /// The files it read, by path.
    pub files: BTreeMap<String, Vec<u8>>,
    /// What it left alone ([`Place::Ignore`]).
    pub ignored: Vec<String>,
    /// The modules it left out, and why.
    pub left_out: Vec<LeftOut>,
}

impl Staged {
    /// Whether the module `id` was left out already.
    #[must_use]
    pub fn is_left_out(&self, id: &str) -> bool {
        self.left_out.iter().any(|l| l.module == id)
    }

    /// Leaves the module `id` out for `refused`: everything read of it is
    /// dropped, and nothing more of it will be.
    pub fn leave_out(&mut self, id: &str, refused: Refused) {
        let folder = format!("{BRAIN_MODULES}/{id}");
        let inside = |p: &String| *p == folder || p.starts_with(&format!("{folder}/"));
        self.dirs.retain(|d| !inside(d));
        self.files.retain(|f, _| !inside(f));
        self.ignored.retain(|i| !inside(i));
        if !self.is_left_out(id) {
            self.left_out.push(LeftOut::of(id, refused));
        }
    }
}

/// A machine's `person/`: folders and files, by path under it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tree {
    /// Every folder, parents included.
    pub dirs: BTreeSet<String>,
    /// Every file.
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Tree {
    /// Puts a file, and every folder above it.
    pub fn put(&mut self, rel: &str, bytes: Vec<u8>) {
        let mut at = rel;
        while let Some((parent, _)) = at.rsplit_once('/') {
            self.dirs.insert(parent.to_owned());
            at = parent;
        }
        self.files.insert(rel.to_owned(), bytes);
    }

    /// Every byte of every file.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.files.values().map(|b| b.len() as u64).sum()
    }
}

/// The machine's tree, and what was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assembled {
    /// The machine's `person/`.
    pub tree: Tree,
    /// What the staged tree held and the machine does not take, sorted.
    pub ignored: Vec<String>,
    /// The modules left out, and why.
    pub left_out: Vec<LeftOut>,
    /// The documents the signature did not vouch for, kept as the machine
    /// had them ([`hold_to_signature`]).
    pub kept_back: Vec<KeptBack>,
}

/// The icon an entry names, if it names one.
fn icon_of(entry: &Entry) -> Option<&str> {
    match entry {
        Entry::AppImage(a) => a.icon.as_deref(),
        Entry::Web(a) => a.icon.as_deref(),
        Entry::Nix(_) | Entry::Flatpak(_) | Entry::Android(_) => None,
    }
}

/// Whether `bytes` start as a PNG does.
fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
}

/// One document of the staged tree, parsed, when it is there.
fn document<T: Document>(staged: &Staged, at: &str) -> Result<Option<T>, Refused> {
    staged
        .files
        .get(at)
        .map(|bytes| crate::parse::<T>(bytes).map_err(|r| Refused::of_document(at, r)))
        .transpose()
}

/// Where the staged icons are, under the staged root.
fn icons_at() -> String {
    format!("{PERSON}/{}", names::ICONS)
}

/// The icons the staged declarations name (`<sha256>.png`, sorted, once
/// each) — the only icons root opens: one an entry forgot is never read, so
/// it can neither cost a byte nor refuse the tree.
///
/// # Errors
///
/// The declarations' own refusal; [`TreeWhy::TooMany`] over
/// [`caps::ICONS_MOST`].
pub fn named_icons(staged: &Staged) -> Result<Vec<String>, Refused> {
    let declared: Option<Declared> = document(staged, &format!("{PERSON}/{}", names::DECLARED))?;
    let named: BTreeSet<String> = declared
        .iter()
        .flat_map(|d| d.apps.iter())
        .filter_map(icon_of)
        .map(|icon| format!("{icon}.png"))
        .filter(|name| icon_name(name))
        .collect();
    if named.len() > caps::ICONS_MOST {
        return Err(Refused::new(
            TreeWhy::TooMany,
            icons_at(),
            format!("{} icons, at most {}", named.len(), caps::ICONS_MOST),
        ));
    }
    Ok(named.into_iter().collect())
}

/// The machine's `person/` made of a staged tree.
///
/// # Errors
///
/// [`TreeWhy::NothingStaged`] without a `person/` folder; a document's own
/// refusal (a NEWER format among them, so an older machine never rewrites
/// what it does not understand); [`TreeWhy::MissingIcon`],
/// [`TreeWhy::NotPng`]; [`TreeWhy::TooMany`] and [`TreeWhy::TooLarge`] over
/// the caps of the whole. A module over its own cap is left out, not
/// refused.
pub fn assemble(staged: &Staged) -> Result<Assembled, Refused> {
    if !staged.dirs.contains(PERSON) {
        return Err(Refused::new(
            TreeWhy::NothingStaged,
            PERSON,
            "no person/ folder was staged",
        ));
    }
    let mut tree = Tree::default();
    let mut ignored = staged.ignored.clone();
    let mut left_out = staged.left_out.clone();

    let declared_at = format!("{PERSON}/{}", names::DECLARED);
    let declined_at = format!("{PERSON}/{}", names::DECLINED);
    let options_at = format!("{PERSON}/{}", names::OPTIONS);
    let declared: Option<Declared> = document(staged, &declared_at)?;
    let declined: Option<Declined> = document(staged, &declined_at)?;
    let options: Option<Options> = document(staged, &options_at)?;
    if let Some(d) = &declared {
        tree.put(names::DECLARED, crate::to_json(d).into_bytes());
    }
    if let Some(d) = &declined {
        tree.put(names::DECLINED, crate::to_json(d).into_bytes());
    }
    if let Some(o) = &options {
        tree.put(names::OPTIONS, crate::to_json(o).into_bytes());
    }

    // THE ICONS THE ENTRIES NAME, and no other: an icon of a forgotten entry
    // would be copied into every evaluation's store copy for nothing.
    let icons_at = format!("{}/", icons_at());
    let mut named = BTreeSet::new();
    for (i, entry) in declared.iter().flat_map(|d| d.apps.iter()).enumerate() {
        let Some(icon) = icon_of(entry) else {
            continue;
        };
        let from = format!("{icons_at}{icon}.png");
        let bytes = staged.files.get(&from).ok_or_else(|| {
            Refused::new(
                TreeWhy::MissingIcon,
                format!("{declared_at}#/apps/{i}/icon"),
                format!("{from} was not staged"),
            )
        })?;
        if !is_png(bytes) {
            return Err(Refused::new(TreeWhy::NotPng, from, "not a PNG"));
        }
        if named.insert(from.clone()) {
            tree.put(&format!("{}/{icon}.png", names::ICONS), bytes.clone());
        }
    }
    if named.len() > caps::ICONS_MOST {
        return Err(Refused::new(
            TreeWhy::TooMany,
            icons_at,
            format!("{} icons, at most {}", named.len(), caps::ICONS_MOST),
        ));
    }
    ignored.extend(
        staged
            .files
            .keys()
            .filter(|p| p.starts_with(&icons_at) && !named.contains(*p))
            .cloned(),
    );

    // THE BRAIN MODULES, byte for byte, folders included — the approval's
    // hash reads the folders too. One over its own cap of files is left out.
    let modules_at = format!("{BRAIN_MODULES}/");
    let mut per_module: BTreeMap<&str, usize> = BTreeMap::new();
    for dir in &staged.dirs {
        if let Some(id) = module_of(dir) {
            per_module.entry(id).or_default();
        }
    }
    for path in staged.files.keys() {
        if let Some(id) = module_of(path) {
            *per_module.entry(id).or_default() += 1;
        }
    }
    if per_module.len() > caps::MODULES_MOST {
        return Err(Refused::new(
            TreeWhy::TooMany,
            BRAIN_MODULES,
            format!(
                "{} modules, at most {}",
                per_module.len(),
                caps::MODULES_MOST
            ),
        ));
    }
    let crowded: BTreeSet<&str> = per_module
        .iter()
        .filter(|(_, n)| **n > caps::MODULE_FILES_MOST)
        .map(|(id, _)| *id)
        .collect();
    for id in &crowded {
        left_out.push(LeftOut::of(
            id,
            Refused::new(
                TreeWhy::TooMany,
                format!("{modules_at}{id}"),
                format!(
                    "{} files, at most {}",
                    per_module[id],
                    caps::MODULE_FILES_MOST
                ),
            ),
        ));
    }
    let taken = |p: &str| module_of(p).is_some_and(|id| !crowded.contains(id));
    for dir in staged.dirs.iter().filter(|d| taken(d)) {
        if let Some(rest) = dir.strip_prefix(&modules_at) {
            tree.dirs.insert(format!("{BRAIN}/{rest}"));
            tree.dirs.insert(BRAIN.to_owned());
        }
    }
    for (path, bytes) in staged.files.iter().filter(|(p, _)| taken(p)) {
        if let Some(rest) = path.strip_prefix(&modules_at) {
            tree.put(&format!("{BRAIN}/{rest}"), bytes.clone());
        }
    }

    if tree.bytes() > caps::TREE_MOST_BYTES {
        return Err(Refused::new(
            TreeWhy::TooLarge,
            "",
            format!(
                "{} bytes in all, at most {}",
                tree.bytes(),
                caps::TREE_MOST_BYTES
            ),
        ));
    }
    ignored.sort();
    ignored.dedup();
    left_out.sort_by(|a, b| a.module.cmp(&b.module));
    Ok(Assembled {
        tree,
        ignored,
        left_out,
        kept_back: Vec::new(),
    })
}

/// Puts back, into the new `tree`, the machine's `current` copy of every
/// module `left_out` — an apply never takes a module away only because it
/// could not read the new one — and marks each that had a copy as kept.
pub fn carry_modules(tree: &mut Tree, current: &Tree, left_out: &mut [LeftOut]) {
    for out in left_out {
        let folder = format!("{BRAIN}/{}", out.module);
        if !current.dirs.contains(&folder) {
            continue;
        }
        let inside = |p: &str| p == folder || p.starts_with(&format!("{folder}/"));
        // Whatever the new tree had of it goes first: the copy is the old one,
        // whole.
        tree.dirs.retain(|d| !inside(d));
        tree.files.retain(|f, _| !inside(f));
        tree.dirs.insert(BRAIN.to_owned());
        tree.dirs
            .extend(current.dirs.iter().filter(|d| inside(d)).cloned());
        tree.files.extend(
            current
                .files
                .iter()
                .filter(|(f, _)| inside(f))
                .map(|(f, b)| (f.clone(), b.clone())),
        );
        out.kept = true;
    }
}

/// What root's pin makes of a staging's signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust<'a> {
    /// No key is pinned on this machine.
    Unpinned,
    /// The pinned key signed this manifest — opened, and not rolled back:
    /// every file it vouches for, by its path under the staged root, with
    /// its SHA-256.
    Signed(&'a BTreeMap<String, String>),
}

/// The documents of the person's data, by their path under the staged root
/// and under the machine's `person/`.
fn documents() -> [(String, &'static str); 3] {
    [
        (format!("{PERSON}/{}", names::DECLARED), names::DECLARED),
        (format!("{PERSON}/{}", names::DECLINED), names::DECLINED),
        (format!("{PERSON}/{}", names::OPTIONS), names::OPTIONS),
    ]
}

/// Puts back, in `tree`, the machine's `current` copy of the document at
/// `rel` (or none, when it had none).
fn keep_current(tree: &mut Tree, current: &Tree, rel: &str) {
    tree.files.remove(rel);
    if let Some(bytes) = current.files.get(rel) {
        tree.put(rel, bytes.clone());
    }
}

/// Puts back, in `tree`, the machine's `current` icons — the ones its own
/// `declared.json` named, which is the one kept.
fn keep_current_icons(tree: &mut Tree, current: &Tree) {
    let icons = format!("{}/", names::ICONS);
    tree.files.retain(|f, _| !f.starts_with(&icons));
    tree.dirs.retain(|d| d != names::ICONS);
    for (f, bytes) in current.files.iter().filter(|(f, _)| f.starts_with(&icons)) {
        tree.put(f, bytes.clone());
    }
}

/// Holds the assembled tree to what the person signed, as root's pin says
/// ([`Trust`]) — after [`assemble`], before [`carry_modules`] (which puts
/// back the machine's copy of every module left out here).
///
/// SIGNED: a document whose staged bytes are not the signed ones — another
/// content, one the manifest does not name, or one it names that the
/// staging lacks — is kept as the machine has it ([`KeptBack`]); so is
/// `declared.json` when an icon it names is not as signed, and then the
/// machine's icons with it. A brain module with one file root reads that is
/// not as signed, or that the manifest names and the staging lacks, is left
/// out. UNPINNED: the declarations are taken; the options and every brain
/// module are not ([`TreeWhy::NoKeyPinned`]).
///
/// `sha256` is the hash a manifest names files by (hexadecimal): the binary
/// hands `ronoctua_pq::sign::sha256_hex`, a test its own — this crate stays
/// pure.
pub fn hold_to_signature(
    assembled: &mut Assembled,
    staged: &Staged,
    current: &Tree,
    trust: Trust<'_>,
    sha256: &dyn Fn(&[u8]) -> String,
) {
    let modules_at = format!("{BRAIN_MODULES}/");
    let mut out: BTreeMap<String, Refused> = BTreeMap::new();
    match trust {
        Trust::Unpinned => {
            let options = format!("{PERSON}/{}", names::OPTIONS);
            if staged.files.contains_key(&options) {
                keep_current(&mut assembled.tree, current, names::OPTIONS);
                assembled.kept_back.push(KeptBack {
                    at: options,
                    why: TreeWhy::NoKeyPinned,
                });
            }
            for dir in assembled.tree.dirs.iter() {
                if let Some(id) = dir
                    .strip_prefix(&format!("{BRAIN}/"))
                    .map(|r| r.split('/').next().unwrap_or(r).to_owned())
                {
                    out.entry(id.clone()).or_insert_with(|| {
                        Refused::new(
                            TreeWhy::NoKeyPinned,
                            format!("{modules_at}{id}"),
                            "no signing key is pinned on this machine",
                        )
                    });
                }
            }
        }
        Trust::Signed(files) => {
            let as_signed = |rel: &str| match (staged.files.get(rel), files.get(rel)) {
                (Some(bytes), Some(sha)) => sha256(bytes) == *sha,
                (None, None) => true,
                _ => false,
            };
            // THE ICONS `declared.json` NAMES go with it: a declaration
            // whose icon is not as signed is not as signed.
            let icons_at = format!("{PERSON}/{}/", names::ICONS);
            let icons_hold = staged
                .files
                .keys()
                .filter(|f| f.starts_with(&icons_at))
                .filter(|f| {
                    let name = &f[icons_at.len()..];
                    assembled
                        .tree
                        .files
                        .contains_key(&format!("{}/{name}", names::ICONS))
                })
                .all(|f| as_signed(f));
            for (rel, machine) in documents() {
                let holds = as_signed(&rel) && (machine != names::DECLARED || icons_hold);
                if holds {
                    continue;
                }
                keep_current(&mut assembled.tree, current, machine);
                if machine == names::DECLARED {
                    keep_current_icons(&mut assembled.tree, current);
                }
                assembled.kept_back.push(KeptBack {
                    at: rel,
                    why: TreeWhy::NotAsSigned,
                });
            }
            // EVERY MODULE the staging carries, and every one the manifest
            // names: each file root reads of it must be the signed one, and
            // each file the manifest names (that root would read) must be
            // there.
            let mut ids: BTreeSet<String> = BTreeSet::new();
            for path in staged
                .files
                .keys()
                .chain(staged.dirs.iter())
                .chain(files.keys())
            {
                if let Some(id) = module_of(path).filter(|id| check::person_module(id)) {
                    ids.insert(id.to_owned());
                }
            }
            for id in ids {
                if staged.is_left_out(&id) {
                    continue;
                }
                let inside = format!("{modules_at}{id}/");
                let staged_ok = staged
                    .files
                    .iter()
                    .filter(|(p, _)| p.starts_with(&inside))
                    .all(|(p, bytes)| files.get(p).is_some_and(|sha| sha256(bytes) == *sha));
                let signed_ok = files
                    .keys()
                    .filter(|p| p.starts_with(&inside))
                    .filter(|p| matches!(staged_place(p, Kind::File), Ok(Place::Read { .. })))
                    .all(|p| staged.files.contains_key(p));
                let named = staged.files.keys().any(|p| p.starts_with(&inside))
                    || files.keys().any(|p| p.starts_with(&inside));
                if named && !(staged_ok && signed_ok) {
                    out.insert(
                        id.clone(),
                        Refused::new(
                            TreeWhy::NotAsSigned,
                            format!("{modules_at}{id}"),
                            "a file of the module is not what the person signed",
                        ),
                    );
                }
            }
        }
    }
    for (id, refused) in out {
        let folder = format!("{BRAIN}/{id}");
        let inside = |p: &str| p == folder || p.starts_with(&format!("{folder}/"));
        assembled.tree.dirs.retain(|d| !inside(d));
        assembled.tree.files.retain(|f, _| !inside(f));
        if !assembled
            .tree
            .dirs
            .iter()
            .any(|d| d.starts_with(&format!("{BRAIN}/")))
        {
            assembled.tree.dirs.remove(BRAIN);
        }
        if !assembled.left_out.iter().any(|l| l.module == id) {
            assembled.left_out.push(LeftOut::of(&id, refused));
        }
    }
    assembled.left_out.sort_by(|a, b| a.module.cmp(&b.module));
    assembled.kept_back.sort_by(|a, b| a.at.cmp(&b.at));
}

/// What an apply came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The staged tree is the machine's now.
    Applied,
    /// It already was: nothing was written.
    Unchanged,
    /// It was refused, and the machine's data is what it was.
    Refused,
}

/// What an apply that was not refused did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Done {
    /// [`Outcome::Applied`] or [`Outcome::Unchanged`].
    pub outcome: Outcome,
    /// What the staged tree held and the machine did not take.
    pub ignored: Vec<String>,
    /// The modules left out.
    pub left_out: Vec<LeftOut>,
    /// The documents the signature did not vouch for, kept as the machine
    /// had them.
    pub kept_back: Vec<KeptBack>,
}

/// Why, where and what, of a refused apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct VerdictRefusal {
    /// Why.
    pub why: TreeWhy,
    /// Where (under the staged root; `#` and a pointer for a document).
    pub at: String,
    /// What was found, English.
    pub detail: String,
}

/// What the applier says it did — `/run/ronoctua-person/verdict.json`,
/// written on every way out while the lock is still held, and read by
/// whoever started it once `systemctl start` returns.
///
/// The `nonce` is how a starter knows the answer is to IT: a start that
/// joined a run already going is answered by that run, which read somebody
/// else's staging (or an earlier one of the same stager's). A starter whose
/// nonce is not the verdict's starts again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Verdict {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// When.
    pub at: Stamp,
    /// The [`NONCE`] of the staging read; `None` when it had none, or the
    /// run ended before reading it (busy, the owner unknown).
    pub nonce: Option<String>,
    /// What it came to.
    pub outcome: Outcome,
    /// Why it was refused, when it was.
    pub refused: Option<VerdictRefusal>,
    /// What the staged tree held and the machine did not take.
    pub ignored: Vec<String>,
    /// The brain modules left out, and whether the machine kept its copy.
    pub left_out: Vec<LeftOut>,
    /// The documents the person's signature did not vouch for, kept as the
    /// machine had them. A verdict of an applier before the signature has
    /// none.
    #[serde(default)]
    pub kept_back: Vec<KeptBack>,
}

impl Verdict {
    /// The verdict of an apply of the staging `nonce` that came to `done`
    /// (or was refused).
    #[must_use]
    pub fn of(at: Stamp, nonce: Option<String>, done: Result<Done, Refused>) -> Self {
        match done {
            Ok(done) => Self {
                format: crate::FORMAT,
                at,
                nonce,
                outcome: done.outcome,
                refused: None,
                ignored: done.ignored,
                left_out: done.left_out,
                kept_back: done.kept_back,
            },
            Err(r) => Self {
                format: crate::FORMAT,
                at,
                nonce,
                outcome: Outcome::Refused,
                refused: Some(VerdictRefusal {
                    why: r.why,
                    at: r.at,
                    detail: r.detail,
                }),
                ignored: Vec::new(),
                left_out: Vec::new(),
                kept_back: Vec::new(),
            },
        }
    }
}

impl Document for Verdict {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if (self.outcome == Outcome::Refused) != self.refused.is_some() {
            return Err(Refusal::new(
                crate::Why::Invalid,
                "/refused",
                "a refusal goes with the outcome refused, and only with it",
            ));
        }
        if self.outcome == Outcome::Refused && !self.left_out.is_empty() {
            return Err(Refusal::new(
                crate::Why::Invalid,
                "/left-out",
                "a refused tree leaves nothing out: nothing of it arrived",
            ));
        }
        if self.outcome == Outcome::Refused && !self.kept_back.is_empty() {
            return Err(Refusal::new(
                crate::Why::Invalid,
                "/kept-back",
                "a refused tree keeps nothing back: nothing of it arrived",
            ));
        }
        if let Some(i) = self.kept_back.iter().position(|k| !k.why.keeps_back()) {
            return Err(Refusal::new(
                crate::Why::Invalid,
                format!("/kept-back/{i}/why"),
                self.kept_back[i].why.id(),
            ));
        }
        if let Some(nonce) = &self.nonce
            && nonce_of(nonce.as_bytes()).is_err()
        {
            return Err(Refusal::new(crate::Why::Invalid, "/nonce", nonce.as_str()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-the-rest";
    const ICON: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn good() -> Staged {
        let mut s = Staged::default();
        for d in [PERSON, "person/software", "person/software/icons"] {
            s.dirs.insert(d.to_owned());
        }
        s.files.insert(
            "person/software/declared.json".to_owned(),
            include_bytes!("../fixtures/good/declared.json").to_vec(),
        );
        s.files.insert(
            "person/software/declined.json".to_owned(),
            include_bytes!("../fixtures/good/declined.json").to_vec(),
        );
        s.files.insert(
            "person/options.json".to_owned(),
            include_bytes!("../fixtures/good/options.json").to_vec(),
        );
        s.files
            .insert(format!("person/software/icons/{ICON}.png"), PNG.to_vec());
        s
    }

    #[test]
    fn every_reason_has_one_name() {
        for why in TreeWhy::ALL {
            assert_eq!(TreeWhy::of_id(why.id()), Some(why));
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
        let ids: BTreeSet<_> = TreeWhy::ALL.iter().map(|w| w.id()).collect();
        assert_eq!(ids.len(), TreeWhy::ALL.len(), "two reasons share a name");
        for why in crate::Why::ALL {
            assert_eq!(TreeWhy::of_id(why.id()), Some(TreeWhy::Document(why)));
        }
    }

    /// FAILS IF: the walker is sent into a place the machine never takes, the
    /// icons are walked instead of read by name, an editor's dropping in a
    /// module is opened, or a module named without `my-`, a module file with
    /// a strange name or a tree deeper than the cap is let through.
    #[test]
    fn a_place_is_decided_by_its_path_alone() {
        use Kind::{Dir, File};
        assert_eq!(staged_place("person", Dir), Ok(Place::Walk));
        assert_eq!(
            staged_place("person/software/declared.json", File),
            Ok(Place::Read {
                most_bytes: caps::DECLARED_MOST_BYTES
            })
        );
        assert_eq!(
            staged_place("person/software/icons", Dir),
            Ok(Place::Apart),
            "the icons are read by the names the entries give, never walked"
        );
        assert_eq!(
            staged_place(&format!("person/software/icons/{ICON}.png"), File),
            Ok(Place::Apart)
        );
        assert_eq!(staged_place("nonce", File), Ok(Place::Apart));
        assert_eq!(
            staged_place("person/machines", Dir),
            Ok(Place::Ignore),
            "the machines' records are never walked"
        );
        assert_eq!(
            staged_place("brain-modules/README", File),
            Ok(Place::Ignore)
        );
        assert_eq!(staged_place("brain-modules/my-mod", Dir), Ok(Place::Walk));
        assert_eq!(
            staged_place("brain-modules/my-mod/i18n/en-GB.ftl", File),
            Ok(Place::Read {
                most_bytes: caps::MODULE_FILE_MOST_BYTES
            })
        );
        assert_eq!(
            staged_place("brain-modules/my-mod/c++.rs", File),
            Ok(Place::Read {
                most_bytes: caps::MODULE_FILE_MOST_BYTES
            }),
            "a name the hash takes"
        );
        for dropping in [
            "brain-modules/my-mod/.module.rs.swp",
            "brain-modules/my-mod/module.rs~",
            "brain-modules/my-mod/#module.rs#",
            "brain-modules/my-mod/.#module.rs",
            "brain-modules/my-mod/a/b/c/d/.deep.swp",
        ] {
            assert_eq!(
                staged_place(dropping, File),
                Ok(Place::Ignore),
                "{dropping}"
            );
        }
        assert_eq!(
            staged_place("brain-modules/my-mod/.git", Dir),
            Ok(Place::Ignore)
        );
        assert_eq!(staged_place("brain-modules/.git", Dir), Ok(Place::Ignore));
        let refused = |rel: &str, kind| staged_place(rel, kind).unwrap_err().why;
        assert_eq!(
            refused("person/software/declared.json", Dir),
            TreeWhy::Misplaced
        );
        assert_eq!(refused("person/software", File), TreeWhy::Misplaced);
        assert_eq!(refused("brain-modules/reminders", Dir), TreeWhy::NotAModule);
        assert_eq!(
            refused("brain-modules/my-mod/a b.rs", File),
            TreeWhy::BadName
        );
        assert_eq!(
            refused("brain-modules/my-mod/a/b/c/d/e.rs", File),
            TreeWhy::TooDeep
        );
        assert_eq!(staged_place("strange", File), Ok(Place::Ignore));
        assert_eq!(module_of("brain-modules/my-mod/a/b"), Some("my-mod"));
        assert_eq!(module_of("brain-modules/my-mod"), Some("my-mod"));
        assert_eq!(module_of("brain-modules"), None);
        assert_eq!(module_of("person/software"), None);
    }

    /// FAILS IF: the documents are not rewritten in their canonical form, an
    /// icon nobody names travels, or a module does not land under `brain/`.
    #[test]
    fn a_good_tree_becomes_the_machines() {
        let mut staged = good();
        staged.files.insert(
            format!("person/software/icons/{}.png", "a".repeat(64)),
            PNG.to_vec(),
        );
        staged.dirs.insert("brain-modules".to_owned());
        staged.dirs.insert("brain-modules/my-mod".to_owned());
        staged.dirs.insert("brain-modules/my-mod/i18n".to_owned());
        staged.files.insert(
            "brain-modules/my-mod/module.nix".to_owned(),
            b"{ }\n".to_vec(),
        );
        staged.ignored.push("person/machines".to_owned());
        let a = assemble(&staged).unwrap();
        assert_eq!(
            a.tree.files["software/declared.json"],
            include_bytes!("../fixtures/good/declared.json")
        );
        assert!(
            a.tree
                .files
                .contains_key(&format!("software/icons/{ICON}.png"))
        );
        assert_eq!(a.tree.files["brain/my-mod/module.nix"], b"{ }\n");
        assert!(
            a.tree.dirs.contains("brain/my-mod/i18n"),
            "an empty folder too"
        );
        assert!(a.tree.dirs.contains("software/icons") && a.tree.dirs.contains("brain"));
        assert_eq!(
            a.ignored,
            vec![
                "person/machines".to_owned(),
                format!("person/software/icons/{}.png", "a".repeat(64)),
            ]
        );
        assert!(a.left_out.is_empty());
    }

    /// FAILS IF: an icon is read that no entry names, or one an entry names
    /// is not asked for.
    #[test]
    fn only_the_named_icons_are_asked_for() {
        assert_eq!(named_icons(&good()).unwrap(), [format!("{ICON}.png")]);
        let mut none = good();
        none.files.remove("person/software/declared.json");
        assert!(named_icons(&none).unwrap().is_empty());
        let mut newer = good();
        newer.files.insert(
            "person/software/declared.json".to_owned(),
            include_bytes!("../fixtures/bad/declared-newer.json").to_vec(),
        );
        assert_eq!(
            named_icons(&newer).unwrap_err().why,
            TreeWhy::Document(crate::Why::Newer)
        );
    }

    /// FAILS IF: an empty staging folder — what every boot leaves — would
    /// take every declaration away, or a newer file is taken in.
    #[test]
    fn nothing_staged_and_a_newer_file_are_refused() {
        assert_eq!(
            assemble(&Staged::default()).unwrap_err().why,
            TreeWhy::NothingStaged
        );
        let mut newer = good();
        newer.files.insert(
            "person/software/declared.json".to_owned(),
            include_bytes!("../fixtures/bad/declared-newer.json").to_vec(),
        );
        let r = assemble(&newer).unwrap_err();
        assert_eq!(r.why, TreeWhy::Document(crate::Why::Newer));
        assert_eq!(r.at, "person/software/declared.json#/format");
        // An empty person/ is a person who declared nothing — taken.
        let mut empty = Staged::default();
        empty.dirs.insert(PERSON.to_owned());
        assert_eq!(assemble(&empty).unwrap().tree, Tree::default());
    }

    #[test]
    fn a_named_icon_must_be_there_and_be_a_png() {
        let mut missing = good();
        missing.files.retain(|p, _| !p.contains("/icons/"));
        let r = assemble(&missing).unwrap_err();
        assert_eq!(r.why, TreeWhy::MissingIcon);
        assert_eq!(r.at, "person/software/declared.json#/apps/2/icon");
        let mut not_png = good();
        not_png.files.insert(
            format!("person/software/icons/{ICON}.png"),
            b"GIF89a".to_vec(),
        );
        assert_eq!(assemble(&not_png).unwrap_err().why, TreeWhy::NotPng);
    }

    /// FAILS IF: the caps of the whole truncate instead of refusing, or a
    /// module over its own cap refuses the tree instead of staying out.
    #[test]
    fn the_caps_refuse_rather_than_truncate() {
        let mut many = good();
        for i in 0..=caps::MODULES_MOST {
            many.dirs.insert(format!("brain-modules/my-m{i}"));
        }
        assert_eq!(assemble(&many).unwrap_err().why, TreeWhy::TooMany);
        let mut big = good();
        for i in 0..17 {
            big.files.insert(
                format!("brain-modules/my-big/f{i}"),
                vec![0; caps::MODULE_FILE_MOST_BYTES as usize],
            );
        }
        assert_eq!(assemble(&big).unwrap_err().why, TreeWhy::TooLarge);
        let mut crowded = good();
        for i in 0..=caps::MODULE_FILES_MOST {
            crowded
                .files
                .insert(format!("brain-modules/my-crowd/f{i}"), Vec::new());
        }
        crowded
            .files
            .insert("brain-modules/my-calm/module.rs".to_owned(), Vec::new());
        let a = assemble(&crowded).unwrap();
        assert_eq!(a.left_out.len(), 1);
        assert_eq!(a.left_out[0].module, "my-crowd");
        assert_eq!(a.left_out[0].why, TreeWhy::TooMany);
        assert!(!a.tree.dirs.contains("brain/my-crowd"));
        assert!(a.tree.files.contains_key("brain/my-calm/module.rs"));
        assert!(a.tree.files.contains_key("software/declared.json"));
    }

    /// FAILS IF: a module left out takes the machine's copy of it away, or
    /// its half-read new files arrive instead of the old ones.
    #[test]
    fn a_module_left_out_keeps_the_machines_copy() {
        let mut current = Tree::default();
        current.put("brain/my-x/module.rs", b"old".to_vec());
        current.put("brain/my-x/i18n/en-GB.ftl", b"old ftl".to_vec());
        current.put("software/declared.json", b"old declared".to_vec());
        let mut new = Tree::default();
        new.put("software/declared.json", b"new declared".to_vec());
        new.put("brain/my-x/half.rs", b"half".to_vec());
        let mut out = vec![
            LeftOut::of(
                "my-x",
                Refused::new(TreeWhy::Link, "brain-modules/my-x/l", ""),
            ),
            LeftOut::of(
                "my-new",
                Refused::new(TreeWhy::Link, "brain-modules/my-new/l", ""),
            ),
        ];
        carry_modules(&mut new, &current, &mut out);
        assert_eq!(new.files["brain/my-x/module.rs"], b"old");
        assert_eq!(new.files["brain/my-x/i18n/en-GB.ftl"], b"old ftl");
        assert!(!new.files.contains_key("brain/my-x/half.rs"));
        assert_eq!(new.files["software/declared.json"], b"new declared");
        assert!(out[0].kept, "the machine had it");
        assert!(!out[1].kept, "nothing to keep");
        assert!(!new.dirs.iter().any(|d| d.contains("my-new")));
    }

    /// FAILS IF: a staging read across a restaging is taken, or a nonce that
    /// is not one is.
    #[test]
    fn a_staging_is_one_staging() {
        assert_eq!(nonce_of(b"0123456789abcdef\n").unwrap(), "0123456789abcdef");
        for bad in [
            &b"short"[..],
            b"has space in it",
            b"a\"b-cdefgh",
            b"x\n\n12345678",
        ] {
            assert_eq!(
                nonce_of(bad).unwrap_err().why,
                TreeWhy::NotANonce,
                "{bad:?}"
            );
        }
        assert!(same_staging(None, None).is_ok());
        assert!(same_staging(Some("a1b2c3d4"), Some("a1b2c3d4")).is_ok());
        for (before, after) in [
            (Some("a1b2c3d4"), Some("e5f6a7b8")),
            (None, Some("e5f6a7b8")),
            (Some("a1b2c3d4"), None),
        ] {
            assert_eq!(
                same_staging(before, after).unwrap_err().why,
                TreeWhy::Restaged
            );
        }
    }

    /// The "hash" these tests sign by: the content itself, so a manifest is
    /// written by hand and a change of one byte is a change of "hash".
    fn content(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    /// The manifest of everything `staged` read, as the person signed it.
    fn manifest_of(staged: &Staged) -> BTreeMap<String, String> {
        staged
            .files
            .iter()
            .filter(|(p, _)| signed(p))
            .map(|(p, b)| (p.clone(), content(b)))
            .collect()
    }

    /// `good()` with a brain module of two files.
    fn good_with_a_module() -> Staged {
        let mut s = good();
        for d in [
            BRAIN_MODULES,
            "brain-modules/my-tea",
            "brain-modules/my-tea/i18n",
        ] {
            s.dirs.insert(d.to_owned());
        }
        s.files.insert(
            "brain-modules/my-tea/module.rs".to_owned(),
            b"pub fn tea() {}".to_vec(),
        );
        s.files.insert(
            "brain-modules/my-tea/i18n/en-GB.ftl".to_owned(),
            b"tea = Tea".to_vec(),
        );
        s
    }

    /// The machine's copy before the apply: older documents, an older module.
    fn machine_had() -> Tree {
        let mut t = Tree::default();
        t.put(names::DECLARED, b"the machine's declared".to_vec());
        t.put(names::OPTIONS, b"the machine's options".to_vec());
        t.put(
            &format!("{}/{}.png", names::ICONS, "c".repeat(64)),
            b"old icon".to_vec(),
        );
        t.put("brain/my-tea/module.rs", b"pub fn old_tea() {}".to_vec());
        t
    }

    /// FAILS IF: the signature covers a machine's record, or leaves a
    /// declaration, an icon or a module file out — or covers something that
    /// is not the person's data (the manifest itself, the nonce).
    #[test]
    fn the_signature_covers_the_persons_data_but_the_machines_records() {
        for yes in [
            "person/options.json",
            "person/software/declared.json",
            "person/software/icons/x.png",
            "brain-modules/my-tea/module.rs",
        ] {
            assert!(signed(yes), "{yes}");
        }
        for no in [
            "person/machines/laptop.json",
            "person/machines",
            "person",
            "brain-modules",
            MANIFEST,
            SIGNATURE,
            NONCE,
            "personal/x",
            "brain-modulesx/a",
        ] {
            assert!(!signed(no), "{no}");
        }
        assert_eq!(staged_place(MANIFEST, Kind::File), Ok(Place::Apart));
        assert_eq!(staged_place(SIGNATURE, Kind::File), Ok(Place::Apart));
    }

    /// FAILS IF: a staging exactly as signed loses a file to the check.
    #[test]
    fn a_staging_as_signed_arrives_whole() {
        let staged = good_with_a_module();
        let files = manifest_of(&staged);
        let mut a = assemble(&staged).unwrap();
        let before = a.tree.clone();
        hold_to_signature(
            &mut a,
            &staged,
            &machine_had(),
            Trust::Signed(&files),
            &content,
        );
        assert_eq!(a.tree, before);
        assert!(a.kept_back.is_empty(), "{:?}", a.kept_back);
        assert!(a.left_out.is_empty(), "{:?}", a.left_out);
    }

    /// FAILS IF: a document that is not what the person signed — tampered,
    /// unsigned, or signed and missing — is taken, or takes the rest of the
    /// staging down with it.
    #[test]
    fn a_document_not_as_signed_is_kept_back_and_the_rest_arrives() {
        let signed_staging = good_with_a_module();
        let files = manifest_of(&signed_staging);
        let current = machine_had();

        // THE OPTIONS CHANGED after the signature (or by somebody else).
        let mut tampered = signed_staging.clone();
        tampered.files.insert(
            "person/options.json".to_owned(),
            br#"{"format": 1}"#.to_vec(),
        );
        let mut a = assemble(&signed_staging).unwrap();
        hold_to_signature(&mut a, &tampered, &current, Trust::Signed(&files), &content);
        assert_eq!(a.tree.files[names::OPTIONS], b"the machine's options");
        assert_eq!(
            a.kept_back,
            [KeptBack {
                at: "person/options.json".to_owned(),
                why: TreeWhy::NotAsSigned
            }]
        );
        assert_eq!(
            a.tree.files[names::DECLARED],
            include_bytes!("../fixtures/good/declared.json"),
            "the declarations as signed did not arrive"
        );
        assert!(a.tree.files.contains_key("brain/my-tea/module.rs"));

        // A DOCUMENT SIGNED AND NOT STAGED: the machine keeps its copy (here,
        // none of `declined.json`).
        let mut missing = signed_staging.clone();
        missing.files.remove("person/software/declined.json");
        let mut a = assemble(&missing).unwrap();
        hold_to_signature(&mut a, &missing, &current, Trust::Signed(&files), &content);
        assert!(!a.tree.files.contains_key(names::DECLINED));
        assert_eq!(a.kept_back.len(), 1);
        assert_eq!(a.kept_back[0].at, "person/software/declined.json");

        // AN ICON NOT AS SIGNED takes its declaration back, and the machine's
        // icons with it.
        let mut icon = signed_staging;
        icon.files.insert(
            format!("person/software/icons/{ICON}.png"),
            b"\x89PNG\r\n\x1a\nanother".to_vec(),
        );
        let mut a = assemble(&icon).unwrap();
        hold_to_signature(&mut a, &icon, &current, Trust::Signed(&files), &content);
        assert_eq!(a.tree.files[names::DECLARED], b"the machine's declared");
        assert!(
            a.tree
                .files
                .contains_key(&format!("{}/{}.png", names::ICONS, "c".repeat(64)))
        );
        assert!(
            !a.tree
                .files
                .contains_key(&format!("{}/{ICON}.png", names::ICONS))
        );
        assert_eq!(a.kept_back[0].at, "person/software/declared.json");
    }

    /// FAILS IF: a brain module the person did not sign — one file changed,
    /// one file added, one signed file gone, or a module the manifest never
    /// named — reaches the machine's copy, which the brain is compiled from.
    #[test]
    fn an_unsigned_module_never_reaches_the_machine() {
        let signed_staging = good_with_a_module();
        let files = manifest_of(&signed_staging);
        let current = machine_had();
        type Tamper = Box<dyn Fn(&mut Staged)>;
        let cases: [(&str, Tamper); 4] = [
            (
                "a file changed",
                Box::new(|s: &mut Staged| {
                    s.files.insert(
                        "brain-modules/my-tea/module.rs".to_owned(),
                        b"pub fn tea() { evil() }".to_vec(),
                    );
                }),
            ),
            (
                "a file added",
                Box::new(|s: &mut Staged| {
                    s.files.insert(
                        "brain-modules/my-tea/build.rs".to_owned(),
                        b"fn main() {}".to_vec(),
                    );
                }),
            ),
            (
                "a signed file gone",
                Box::new(|s: &mut Staged| {
                    s.files.remove("brain-modules/my-tea/i18n/en-GB.ftl");
                }),
            ),
            (
                "a module nobody signed",
                Box::new(|s: &mut Staged| {
                    s.dirs.insert("brain-modules/my-new".to_owned());
                    s.files.insert(
                        "brain-modules/my-new/module.rs".to_owned(),
                        b"pub fn new() {}".to_vec(),
                    );
                }),
            ),
        ];
        for (name, change) in cases {
            let mut staging = signed_staging.clone();
            change(&mut staging);
            let mut a = assemble(&staging).unwrap();
            hold_to_signature(&mut a, &staging, &current, Trust::Signed(&files), &content);
            let out: Vec<&str> = a.left_out.iter().map(|l| l.module.as_str()).collect();
            let module = if name == "a module nobody signed" {
                "my-new"
            } else {
                "my-tea"
            };
            assert_eq!(out, [module], "{name}");
            assert_eq!(a.left_out[0].why, TreeWhy::NotAsSigned, "{name}");
            assert!(
                !a.tree
                    .files
                    .keys()
                    .any(|f| f.starts_with(&format!("brain/{module}/"))),
                "{name}: an unsigned module is in the machine's tree"
            );
            // THE MACHINE'S COPY comes back, whole and old.
            carry_modules(&mut a.tree, &current, &mut a.left_out);
            if module == "my-tea" {
                assert_eq!(
                    a.tree.files["brain/my-tea/module.rs"],
                    b"pub fn old_tea() {}"
                );
                assert!(a.left_out[0].kept, "{name}");
            } else {
                assert!(!a.left_out[0].kept, "{name}");
            }
            assert!(a.kept_back.is_empty(), "{name}: {:?}", a.kept_back);
        }
    }

    /// FAILS IF: a machine with no key pinned takes the person's options or a
    /// brain module, or stops taking their declarations.
    #[test]
    fn with_no_key_pinned_only_the_declarations_arrive() {
        let staged = good_with_a_module();
        let current = machine_had();
        let mut a = assemble(&staged).unwrap();
        hold_to_signature(&mut a, &staged, &current, Trust::Unpinned, &content);
        assert_eq!(
            a.tree.files[names::DECLARED],
            include_bytes!("../fixtures/good/declared.json")
        );
        assert_eq!(a.tree.files[names::OPTIONS], b"the machine's options");
        assert_eq!(
            a.kept_back,
            [KeptBack {
                at: "person/options.json".to_owned(),
                why: TreeWhy::NoKeyPinned
            }]
        );
        assert_eq!(a.left_out.len(), 1);
        assert_eq!(a.left_out[0].why, TreeWhy::NoKeyPinned);
        assert!(!a.tree.files.keys().any(|f| f.starts_with("brain/")));
        let mut none = Tree::default();
        none.put(names::DECLARED, b"x".to_vec());
        let mut a = assemble(&staged).unwrap();
        hold_to_signature(&mut a, &staged, &none, Trust::Unpinned, &content);
        assert!(
            !a.tree.files.contains_key(names::OPTIONS),
            "options from nowhere"
        );
        assert!(!a.tree.dirs.contains(BRAIN));
    }

    #[test]
    fn a_verdict_reads_back_and_says_its_refusal() {
        let at = Stamp::parse("2026-10-05T12:00:00Z").unwrap();
        let left = LeftOut::of(
            "my-x",
            Refused::new(TreeWhy::Link, "brain-modules/my-x/l", "a symbolic link"),
        );
        for v in [
            Verdict::of(
                at.clone(),
                Some("n0nce-0001".to_owned()),
                Ok(Done {
                    outcome: Outcome::Applied,
                    ignored: vec!["person/machines".to_owned()],
                    left_out: vec![left],
                    kept_back: vec![KeptBack {
                        at: "person/options.json".to_owned(),
                        why: TreeWhy::NotAsSigned,
                    }],
                }),
            ),
            Verdict::of(
                at.clone(),
                None,
                Ok(Done {
                    outcome: Outcome::Unchanged,
                    ignored: Vec::new(),
                    left_out: Vec::new(),
                    kept_back: Vec::new(),
                }),
            ),
            Verdict::of(
                at,
                None,
                Err(Refused::new(TreeWhy::Busy, "", "the update holds the lock")),
            ),
        ] {
            let text = crate::to_json(&v);
            assert_eq!(crate::parse::<Verdict>(text.as_bytes()).unwrap(), v);
        }
        let lying = r#"{"format":1,"at":"2026-10-05T12:00:00Z","nonce":null,"outcome":"applied","refused":{"why":"link","at":"x","detail":"y"},"ignored":[],"left-out":[],"kept-back":[]}"#;
        assert_eq!(
            crate::parse::<Verdict>(lying.as_bytes()).unwrap_err().why,
            crate::Why::Invalid
        );
        let bad_nonce = r#"{"format":1,"at":"2026-10-05T12:00:00Z","nonce":"x y","outcome":"unchanged","refused":null,"ignored":[],"left-out":[],"kept-back":[]}"#;
        assert_eq!(
            crate::parse::<Verdict>(bad_nonce.as_bytes())
                .unwrap_err()
                .why,
            crate::Why::Invalid
        );
        // A REFUSED TREE KEEPS NOTHING BACK, and only a signature's reasons
        // keep a file back.
        for lying in [
            r#"{"format":1,"at":"2026-10-05T12:00:00Z","nonce":null,"outcome":"refused","refused":{"why":"bad-signature","at":"","detail":""},"ignored":[],"left-out":[],"kept-back":[{"at":"person/options.json","why":"not-as-signed"}]}"#,
            r#"{"format":1,"at":"2026-10-05T12:00:00Z","nonce":null,"outcome":"applied","refused":null,"ignored":[],"left-out":[],"kept-back":[{"at":"person/options.json","why":"link"}]}"#,
        ] {
            assert_eq!(
                crate::parse::<Verdict>(lying.as_bytes()).unwrap_err().why,
                crate::Why::Invalid,
                "{lying}"
            );
        }
        for why in TreeWhy::ALL {
            assert!(
                !(why.keeps_back() && why.refuses_the_signature()),
                "{why:?}"
            );
        }
    }
}
