//! An archive, walked into like a folder: the listing as a tree, the
//! listings a process keeps, and where a member goes when it is opened.
//!
//! # What this is, and what it is not
//!
//! It is the file manager's half of `Place::Archive`: given what
//! `ronoctua-archive` LISTED (every member, by its path inside), it answers
//! what a folder inside the archive holds, as the same [`Entry`] rows a real
//! directory gives — so the listing, the sort, the filter, the marks and the
//! cursor are the ones the manager already has. It runs no program: the
//! listing and every extraction are the archive crate's, on a thread of the
//! door that asked.
//!
//! # Where the view stands
//!
//! A place inside an archive is written as the archive's own path with the
//! member's path after it — `/home/a/photos.zip/2026/june` — which is what the
//! breadcrumbs draw and what going up walks. [`Place::of`] takes that path
//! apart again, against the archive the view entered: a path that is not
//! under it is a folder on the disk.
//!
//! # Bounded
//!
//! A process keeps at most [`policy::ARCHIVE_LISTINGS`] listings, each keyed
//! by the archive's path, size and modification time — an archive rewritten in
//! place is listed again. What an opened member is extracted into lives under
//! `$XDG_CACHE_HOME/ronoctua/archive`, trimmed to
//! [`policy::ARCHIVE_OPENED_MIB`] after every extraction (oldest first), and
//! swept by the cache's age like everything else there.

use std::collections::{BTreeMap, VecDeque};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub use ronoctua_archive::{Format, Member, Why};

use crate::{Entry, Kind};

/// The cache policy, generated from `assets/cache.nix` by `nix/cache.nix`.
#[allow(dead_code)]
pub mod policy {
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// Whether a file of this name is ENTERED rather than opened: an archive this
/// system reads. By the name, as [`Format::of_path`] decides — the same answer
/// the program that lists it is chosen by.
#[must_use]
pub fn enters(name: &str) -> bool {
    Format::of_path(Path::new(name)).readable()
}

/// Whether members can be added to or removed from an archive of this name
/// in place (zip and 7z).
#[must_use]
pub fn editable(name: &str) -> bool {
    Format::of_path(Path::new(name)).editable()
}

/// An archive's name without the format's own ending: `photos` for
/// `photos.tar.gz`, `notes` for `notes.7z` — the folder an extraction goes
/// into and the name a conversion starts from.
#[must_use]
pub fn stem_of(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for tail in [
        ".tar.gz", ".tar.xz", ".tar.zst", ".tar.bz2", ".tar.lz4", ".tgz", ".txz", ".tzst", ".tbz2",
        ".tbz", ".tar", ".zip", ".7z", ".rar", ".iso",
    ] {
        if lower.ends_with(tail) && lower.len() > tail.len() {
            return name[..name.len() - tail.len()].to_owned();
        }
    }
    Path::new(name)
        .file_stem()
        .map_or_else(|| name.to_owned(), |s| s.to_string_lossy().into_owned())
}

/// What an archive file IS right now, for the cache's key: its size and its
/// modification time, to the nanosecond. A file rewritten in place changes
/// one of the two, and its listing is read again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stamp {
    /// Its size, in bytes.
    pub bytes: u64,
    /// Its modification time, in nanoseconds since the epoch.
    pub modified_ns: i128,
}

impl Stamp {
    /// The stamp of the file at `path` (following a link: an archive reached
    /// through one is the file it points at), or `None` when it is not there.
    #[must_use]
    pub fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        Some(Self {
            bytes: meta.len(),
            modified_ns: i128::from(meta.mtime()) * 1_000_000_000 + i128::from(meta.mtime_nsec()),
        })
    }
}

/// Where the view stands: a folder on the disk, or a folder inside an
/// archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// A folder on the disk.
    Folder(PathBuf),
    /// A folder inside an archive: the archive, and the path inside it (`""`
    /// is its top).
    Archive {
        /// The archive file.
        archive: PathBuf,
        /// The folder inside, without a leading or trailing `/`.
        inner: String,
    },
}

impl Place {
    /// Where `path` is, when the view has entered `archive`: inside it when
    /// `path` is the archive or under it, a folder otherwise.
    #[must_use]
    pub fn of(path: &Path, archive: Option<&Path>) -> Self {
        if let Some(archive) = archive
            && let Ok(rest) = path.strip_prefix(archive)
        {
            let inner = rest
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            return Self::Archive {
                archive: archive.to_path_buf(),
                inner,
            };
        }
        Self::Folder(path.to_path_buf())
    }

    /// The path the view writes for this place.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        match self {
            Self::Folder(p) => p.clone(),
            Self::Archive { archive, inner } if inner.is_empty() => archive.clone(),
            Self::Archive { archive, inner } => archive.join(inner),
        }
    }
}

/// A member's path inside the archive: the parts that name somewhere, joined
/// with `/`. A path with a `..` in it, or nothing left, names nowhere — it is
/// not drawn (the archive crate refuses to extract it past the target too).
fn normal(path: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            p => parts.push(p),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// One node of the tree: a member, or a folder only its members imply.
#[derive(Debug, Clone, Default)]
struct Node {
    /// The member, when the archive has one at this path.
    member: Option<Member>,
    /// Whether something is under it: a folder, whatever the listing said.
    parent: bool,
}

/// An archive's listing, as a tree one walks.
#[derive(Debug, Clone, Default)]
pub struct Listing {
    /// Every path, member or implied folder, in order.
    nodes: BTreeMap<String, Node>,
    /// Whether any member is encrypted.
    encrypted: bool,
    /// Whether the names themselves were behind the password.
    hidden_names: bool,
}

impl Listing {
    /// The tree of `members`. `hidden_names` is whether it took a password to
    /// list them at all — a 7z with encrypted headers.
    #[must_use]
    pub fn of(members: Vec<Member>, hidden_names: bool) -> Self {
        let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
        let mut encrypted = false;
        for m in members {
            let Some(path) = normal(&m.path) else {
                continue;
            };
            encrypted |= m.encrypted;
            // EVERY FOLDER ABOVE IT, whether the archive names it or not: a
            // zip made by some programs carries `a/b/c.txt` and no `a/`.
            let mut above = String::new();
            let mut parts = path.split('/').peekable();
            while let Some(part) = parts.next() {
                if parts.peek().is_none() {
                    break;
                }
                if !above.is_empty() {
                    above.push('/');
                }
                above.push_str(part);
                nodes.entry(above.clone()).or_default().parent = true;
            }
            let node = nodes.entry(path.clone()).or_default();
            // A MEMBER NAMED TWICE keeps the later one, which is what
            // extracting it would leave on disk.
            node.member = Some(Member { path, ..m });
        }
        Self {
            nodes,
            encrypted,
            hidden_names,
        }
    }

    /// How many paths it holds, members and implied folders.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether it holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Whether any member is encrypted — the listing a door keeps a password
    /// with.
    #[must_use]
    pub const fn encrypted(&self) -> bool {
        self.encrypted
    }

    /// Whether the names were behind the password.
    #[must_use]
    pub const fn hidden_names(&self) -> bool {
        self.hidden_names
    }

    /// Whether `inner` is a folder of this archive — its top (`""`) always.
    #[must_use]
    pub fn is_dir(&self, inner: &str) -> bool {
        inner.is_empty()
            || self
                .nodes
                .get(inner)
                .is_some_and(|n| n.parent || n.member.as_ref().is_some_and(|m| m.is_dir))
    }

    /// The member at `path`, when the archive has one there.
    #[must_use]
    pub fn member(&self, path: &str) -> Option<&Member> {
        self.nodes.get(path).and_then(|n| n.member.as_ref())
    }

    /// What the folder `inner` holds, as rows: one per name directly under
    /// it, a folder for anything with members under it. Owned by `uid` and
    /// `gid` — what they will be once extracted — and dated `fallback_s` when
    /// the archive gives no date.
    #[must_use]
    pub fn entries_at(&self, inner: &str, uid: u32, gid: u32, fallback_s: i64) -> Vec<Entry> {
        let prefix = if inner.is_empty() {
            String::new()
        } else {
            format!("{inner}/")
        };
        self.nodes
            .range(prefix.clone()..)
            .take_while(|(path, _)| path.starts_with(&prefix))
            .filter(|(path, _)| {
                let rest = &path[prefix.len()..];
                !rest.is_empty() && !rest.contains('/')
            })
            .map(|(path, node)| {
                let name = path[prefix.len()..].to_owned();
                let dir = node.parent || node.member.as_ref().is_some_and(|m| m.is_dir);
                let member = node.member.as_ref();
                Entry {
                    name,
                    kind: if dir { Kind::Directory } else { Kind::File },
                    mode: if dir { 0o040_755 } else { 0o100_644 },
                    links: 1,
                    uid,
                    gid,
                    bytes: if dir {
                        0
                    } else {
                        member.and_then(|m| m.size_bytes).unwrap_or(0)
                    },
                    modified: member.and_then(|m| m.modified_s).unwrap_or(fallback_s),
                    target: None,
                    aims_at: None,
                }
            })
            .collect()
    }

    /// Every member at or under `path` — what taking a folder out of an
    /// archive means, member by member.
    #[must_use]
    pub fn at_and_under(&self, path: &str) -> Vec<String> {
        let prefix = format!("{path}/");
        self.nodes
            .iter()
            .filter(|(p, n)| n.member.is_some() && (p.as_str() == path || p.starts_with(&prefix)))
            .map(|(p, _)| p.clone())
            .collect()
    }
}

/// The listings this process keeps, newest last.
type Kept = VecDeque<(PathBuf, Stamp, Arc<Listing>)>;

fn kept() -> &'static Mutex<Kept> {
    static KEPT: Mutex<Kept> = Mutex::new(VecDeque::new());
    &KEPT
}

/// The listing kept for the archive at `path` — `None` when there is none,
/// or when the file changed since it was listed.
#[must_use]
pub fn cached(path: &Path) -> Option<Arc<Listing>> {
    let stamp = Stamp::of(path)?;
    let kept = kept()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    kept.iter()
        .rev()
        .find(|(p, s, _)| p == path && *s == stamp)
        .map(|(_, _, l)| Arc::clone(l))
}

/// Keeps `listing` for the archive at `path` as it was at `stamp`, letting
/// the oldest go past [`policy::ARCHIVE_LISTINGS`].
pub fn keep(path: &Path, stamp: Stamp, listing: Arc<Listing>) {
    let mut kept = kept()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    kept.retain(|(p, _, _)| p != path);
    kept.push_back((path.to_path_buf(), stamp, listing));
    while kept.len() > policy::ARCHIVE_LISTINGS {
        kept.pop_front();
    }
}

/// Where members opened from archives are extracted:
/// `$XDG_CACHE_HOME/ronoctua/archive` (per instance).
#[must_use]
pub fn opened_root() -> Option<PathBuf> {
    ronoctua_xdg::cache_dir().map(|d| d.join("archive"))
}

/// The folder ONE archive's opened members go into, under `root`: named by a
/// hash of the archive's path and stamp, so the same member of the same
/// archive is extracted once, and an archive rewritten gets a folder of its
/// own.
#[must_use]
pub fn opened_dir(root: &Path, archive: &Path, stamp: Stamp) -> PathBuf {
    use std::os::unix::ffi::OsStrExt as _;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
    };
    eat(archive.as_os_str().as_bytes());
    eat(&stamp.bytes.to_le_bytes());
    eat(&stamp.modified_ns.to_le_bytes());
    root.join(format!("{h:016x}"))
}

/// Where the member `path` of an archive lands inside its `opened` folder —
/// `None` for a path that names nowhere.
#[must_use]
pub fn opened_member(opened: &Path, path: &str) -> Option<PathBuf> {
    normal(path).map(|p| opened.join(p))
}

/// A fresh, empty folder inside `opened` for ONE extraction to land in —
/// hidden, and named so two threads and two processes never share one.
///
/// # Why a member is not extracted into `opened` itself
///
/// The archive crate never replaces what is already where it extracts: a
/// top-level entry that is there already makes what came out take a free
/// name beside it (`2026 (copy 1)`). So the SECOND member taken out of the
/// same top-level folder of an archive landed under the copy's name, and was
/// not where the door went to open it. Extracted into a folder of its own and
/// then moved into place ([`settle_opened`]), every member lands at its path.
#[must_use]
pub fn taking_dir(opened: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    opened.join(format!(".taking-{}-{n}", std::process::id()))
}

/// Moves the member `path`, extracted into `taken` (a [`taking_dir`]), to its
/// place in `opened`, making the folders above it — and answers where it is.
/// `None` for a path that names nowhere or a member that did not come out.
///
/// What is already at that place is replaced: it is the same member of the
/// same archive as it is now (the folder is named by its stamp), taken out by
/// a second thread a moment before.
#[must_use]
pub fn settle_opened(taken: &Path, opened: &Path, path: &str) -> Option<PathBuf> {
    let member = normal(path)?;
    let from = taken.join(&member);
    let to = opened.join(&member);
    from.symlink_metadata().ok()?;
    std::fs::create_dir_all(to.parent()?).ok()?;
    std::fs::rename(&from, &to).ok()?;
    Some(to)
}

/// Trims `root` to `most_bytes`, taking whole archive folders, oldest first,
/// and never `keep` — the one just written. Answers how many went.
pub fn trim_opened(root: &Path, most_bytes: u64, keep: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    let mut folders: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| {
            let path = e.path();
            let when = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (when, crate::act::weigh(&path), path)
        })
        .collect();
    let mut total: u64 = folders.iter().map(|(_, b, _)| *b).sum();
    folders.sort_by_key(|(when, _, _)| *when);
    let mut gone = 0;
    for (_, bytes, path) in folders {
        if total <= most_bytes {
            break;
        }
        if path == keep {
            continue;
        }
        if std::fs::remove_dir_all(&path).is_ok() {
            total = total.saturating_sub(bytes);
            gone += 1;
        }
    }
    gone
}

/// The opened members' bound, in bytes ([`policy::ARCHIVE_OPENED_MIB`]).
#[must_use]
pub const fn opened_most_bytes() -> u64 {
    policy::ARCHIVE_OPENED_MIB as u64 * 1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(path: &str, size: u64, dir: bool) -> Member {
        Member {
            path: path.to_owned(),
            size_bytes: Some(size),
            packed_bytes: None,
            modified_s: Some(1_767_225_600),
            is_dir: dir,
            encrypted: false,
        }
    }

    fn names(entries: &[Entry]) -> Vec<(String, Kind)> {
        entries.iter().map(|e| (e.name.clone(), e.kind)).collect()
    }

    #[test]
    fn a_folder_inside_is_its_members_and_the_folders_they_imply() {
        // A ZIP WITH NO FOLDER ENTRIES: `a/b/c.txt` and nothing naming `a/`,
        // which is what several programs write. The folders still come out.
        let l = Listing::of(
            vec![
                member("a/b/c.txt", 5, false),
                member("./top.txt", 3, false),
                member("empty/", 0, true),
                member("a/d.txt", 7, false),
                member("../../etc/passwd", 9, false),
                member("/", 0, true),
            ],
            false,
        );
        let top = l.entries_at("", 1000, 100, 0);
        assert_eq!(
            names(&top),
            [
                ("a".to_owned(), Kind::Directory),
                ("empty".to_owned(), Kind::Directory),
                ("top.txt".to_owned(), Kind::File)
            ],
            "a path that climbs out of the archive is not drawn"
        );
        assert_eq!(
            names(&l.entries_at("a", 1000, 100, 0)),
            [
                ("b".to_owned(), Kind::Directory),
                ("d.txt".to_owned(), Kind::File)
            ]
        );
        let c = &l.entries_at("a/b", 1000, 100, 0)[0];
        assert_eq!((c.bytes, c.modified, c.uid), (5, 1_767_225_600, 1000));
        assert!(l.is_dir("a") && l.is_dir("a/b") && l.is_dir("") && l.is_dir("empty"));
        assert!(!l.is_dir("top.txt") && !l.is_dir("nowhere"));
        assert_eq!(l.at_and_under("a"), ["a/b/c.txt", "a/d.txt"]);
        assert_eq!(l.member("top.txt").unwrap().size_bytes, Some(3));
        // A NAME THAT ONLY STARTS LIKE ANOTHER is not under it.
        let l = Listing::of(
            vec![member("ab/x", 1, false), member("a/y", 1, false)],
            false,
        );
        assert_eq!(
            names(&l.entries_at("a", 0, 0, 0)),
            [("y".to_owned(), Kind::File)]
        );
    }

    #[test]
    fn a_place_is_inside_the_archive_only_under_it() {
        let zip = Path::new("/home/a/photos.zip");
        assert_eq!(
            Place::of(Path::new("/home/a/photos.zip/2026/june"), Some(zip)),
            Place::Archive {
                archive: zip.to_path_buf(),
                inner: "2026/june".to_owned()
            }
        );
        assert_eq!(
            Place::of(zip, Some(zip)),
            Place::Archive {
                archive: zip.to_path_buf(),
                inner: String::new()
            }
        );
        // GOING UP OUT OF IT is the folder the archive is in — a disk folder.
        assert_eq!(
            Place::of(Path::new("/home/a"), Some(zip)),
            Place::Folder(PathBuf::from("/home/a"))
        );
        // AND A SIBLING WHOSE NAME STARTS THE SAME is not inside.
        assert_eq!(
            Place::of(Path::new("/home/a/photos.zip.old"), Some(zip)),
            Place::Folder(PathBuf::from("/home/a/photos.zip.old"))
        );
        let inside = Place::of(Path::new("/home/a/photos.zip/x"), Some(zip));
        assert_eq!(inside.path(), PathBuf::from("/home/a/photos.zip/x"));
    }

    #[test]
    fn a_name_says_whether_it_is_entered_and_what_its_folder_is_called() {
        assert!(enters("photos.ZIP") && enters("b.tar.zst") && enters("c.rar") && enters("d.iso"));
        assert!(!enters("notes.txt") && !enters("photo.jpg"));
        assert!(editable("a.7z") && !editable("a.tar.xz"));
        assert_eq!(stem_of("photos.tar.gz"), "photos");
        assert_eq!(stem_of("Notes.7Z"), "Notes");
        // A NAME THAT IS ONLY THE ENDING keeps it: a folder called nothing is
        // not a folder one can make.
        assert_eq!(stem_of(".zip"), ".zip");
        assert_eq!(stem_of("plain"), "plain");
    }

    #[test]
    fn a_listing_is_kept_per_stamp_and_the_oldest_goes_past_the_bound() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-files-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let first = dir.join("0.zip");
        std::fs::write(&first, b"one").unwrap();
        let listing = Arc::new(Listing::of(vec![member("x", 1, false)], false));
        keep(&first, Stamp::of(&first).unwrap(), Arc::clone(&listing));
        assert!(cached(&first).is_some());
        // REWRITTEN IN PLACE: another size, and the kept listing is not it.
        std::fs::write(&first, b"one, and longer").unwrap();
        assert!(
            cached(&first).is_none(),
            "a listing of a file that changed was kept"
        );
        keep(&first, Stamp::of(&first).unwrap(), Arc::clone(&listing));
        for n in 1..=policy::ARCHIVE_LISTINGS {
            let p = dir.join(format!("{n}.zip"));
            std::fs::write(&p, b"x").unwrap();
            keep(&p, Stamp::of(&p).unwrap(), Arc::clone(&listing));
        }
        assert!(cached(&first).is_none(), "the bound let the oldest stay");
        assert!(cached(&dir.join("1.zip")).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_members_of_one_folder_each_land_at_their_own_path() {
        // THE DEFECT: the second member of `2026/` came out as
        // `2026 (copy 1)/june/a.jpg`, because the archive crate never
        // replaces what is already where it extracts. Each extraction lands
        // in a folder of its own and is moved into place.
        let opened =
            std::env::temp_dir().join(format!("ronoctua-files-settle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&opened);
        std::fs::create_dir_all(&opened).unwrap();
        for (member, bytes) in [("2026/b.jpg", b"b"), ("2026/june/a.jpg", b"a")] {
            let taken = taking_dir(&opened);
            assert_ne!(taken, taking_dir(&opened), "two extractions share a folder");
            let at = taken.join(member);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(&at, bytes).unwrap();
            let landed = settle_opened(&taken, &opened, member).expect("settled");
            assert_eq!(landed, opened.join(member));
            assert_eq!(std::fs::read(&landed).unwrap(), bytes);
            let _ = std::fs::remove_dir_all(&taken);
        }
        assert!(opened.join("2026/b.jpg").exists() && opened.join("2026/june/a.jpg").exists());
        let names: Vec<_> = std::fs::read_dir(&opened)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names, ["2026"], "a copy's name, or a folder left behind");
        // A MEMBER THAT DID NOT COME OUT, or a path that climbs, lands nowhere.
        let taken = taking_dir(&opened);
        std::fs::create_dir_all(&taken).unwrap();
        assert_eq!(settle_opened(&taken, &opened, "missing.txt"), None);
        assert_eq!(settle_opened(&taken, &opened, "../x"), None);
        let _ = std::fs::remove_dir_all(&opened);
    }

    #[test]
    fn opened_members_have_a_folder_per_archive_and_the_oldest_go_first() {
        let root =
            std::env::temp_dir().join(format!("ronoctua-files-opened-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let a = Stamp {
            bytes: 1,
            modified_ns: 1,
        };
        let b = Stamp {
            bytes: 1,
            modified_ns: 2,
        };
        let one = opened_dir(&root, Path::new("/x.zip"), a);
        assert_eq!(one, opened_dir(&root, Path::new("/x.zip"), a));
        assert_ne!(
            one,
            opened_dir(&root, Path::new("/x.zip"), b),
            "a rewritten archive has its own"
        );
        assert_eq!(opened_member(&one, "../../x"), None);
        assert_eq!(opened_member(&one, "a/b.txt"), Some(one.join("a/b.txt")));
        let old = root.join("old");
        let new = root.join("new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("f"), vec![0u8; 3000]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(new.join("f"), vec![0u8; 3000]).unwrap();
        assert_eq!(trim_opened(&root, 4000, &new), 1);
        assert!(!old.exists() && new.exists(), "the oldest goes first");
        // THE ONE JUST WRITTEN STAYS, even alone over the bound.
        assert_eq!(trim_opened(&root, 10, &new), 0);
        assert!(new.exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
