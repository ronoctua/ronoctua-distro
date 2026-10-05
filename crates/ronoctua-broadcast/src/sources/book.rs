//! `sources.json`: the person's own sources, with the addresses that carry
//! their accounts.
//!
//! # Private from its first byte
//!
//! The file is written into a draft opened `0600` (`create_new` with the mode
//! on the creation — never a `chmod` after the bytes are in), synced and
//! renamed over the old one: [`ronoctua_consent::keys::write_private_file`],
//! the same writer every key of this system goes through. Its folder is
//! `0700`. The serialised bytes are wiped once written.
//!
//! # One writer, and never over a file it could not read
//!
//! The daemon writes it; doors read it only to know it is there (they read
//! the catalogue, which keeps the MASKED address). [`Book::read`] refuses a
//! file that does not parse ([`Unread::Unparseable`]) rather than answer
//! "none" — the daemon would then write an empty list over the person's
//! sources.
//!
//! # Never logged
//!
//! [`PersonSource`]'s `Debug` prints the address masked; nothing here
//! logs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ronoctua_consent::keys::{Dir, Secret, write_private_file};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize as _;

use super::{Address, Kind};

/// The folder, under the config root.
pub const FOLDER: &str = "player-sources";

/// The file, in the folder.
pub const FILE: &str = "sources.json";

/// The layout of the file.
pub const VERSION: u32 = 1;

/// The largest file read, in bytes: a thousand sources with long addresses.
pub const MOST_BYTES: u64 = 1 << 20;

/// One of the person's own sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonSource {
    /// Its id in the catalogue (`p…`, [`Book::next_id`]).
    pub id: String,
    /// What kind of source it is.
    pub kind: Kind,
    /// Its address: secret.
    pub address: Address,
    /// What it is called, as it last said (data), or empty.
    pub title: String,
    /// The person marked it adult: all of it goes to the adult store.
    pub adult: bool,
    /// Its posters are shown.
    pub posters: bool,
    /// When it was added, in seconds since the epoch.
    pub added_s: i64,
}

/// Everything the book keeps: the person's sources, and the switches the
/// person turned on the DEFAULT sources (whose addresses are the table's,
/// and never here).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Kept {
    /// The person's sources, in the order they were added.
    pub sources: Vec<PersonSource>,
    /// A default source's posters switch, by its id, where the person turned
    /// it away from the table's (`DefaultSource::posters_on`).
    pub default_posters: BTreeMap<String, bool>,
}

/// The file's shape. Private: the one place the address is a plain string,
/// and its bytes are wiped when it is dropped.
#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    #[serde(default)]
    sources: Vec<StoredSource>,
    /// The default sources' switches — added in the same version: a file
    /// without them has none turned.
    #[serde(default)]
    default_posters: BTreeMap<String, bool>,
}

#[derive(Serialize, Deserialize)]
struct StoredSource {
    id: String,
    kind: Kind,
    url: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    adult: bool,
    #[serde(default)]
    posters: bool,
    #[serde(default)]
    added_s: i64,
}

impl Drop for StoredSource {
    fn drop(&mut self) {
        self.url.zeroize();
    }
}

/// Why [`Book::read`] gave nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unread {
    /// The file is there and could not be read.
    Unreadable,
    /// The file is there and is not the book (or is of another version, or
    /// larger than [`MOST_BYTES`]); it is kept as it is.
    Unparseable,
}

/// Why [`Book::write`] kept nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unkept {
    /// There is no config root.
    NoHome,
    /// The folder or the file could not be written.
    Unwritable,
}

/// Where the person's sources are kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    dir: PathBuf,
}

impl Book {
    /// The book in `dir` (a test's, or a process handed its root).
    #[must_use]
    pub const fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// This instance's: `player-sources` under the config root.
    ///
    /// # Errors
    ///
    /// [`Unkept::NoHome`].
    pub fn player() -> Result<Self, Unkept> {
        ronoctua_xdg::config_dir()
            .map(|d| Self::at(d.join(FOLDER)))
            .ok_or(Unkept::NoHome)
    }

    /// The file's path.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.dir.join(FILE)
    }

    /// Its folder.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// What the book keeps; nothing when there is no file yet.
    ///
    /// # Errors
    ///
    /// [`Unread::Unreadable`], [`Unread::Unparseable`] — never "none" for a
    /// file that is there.
    pub fn read(&self) -> Result<Kept, Unread> {
        use std::io::Read as _;
        let file = match std::fs::File::open(self.path()) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Kept::default()),
            Err(_) => return Err(Unread::Unreadable),
        };
        let mut bytes = Vec::new();
        let read = file.take(MOST_BYTES + 1).read_to_end(&mut bytes);
        let parsed = match read {
            Ok(n) if n as u64 <= MOST_BYTES => serde_json::from_slice::<Stored>(&bytes).ok(),
            Ok(_) => None,
            Err(_) => {
                bytes.zeroize();
                return Err(Unread::Unreadable);
            }
        };
        bytes.zeroize();
        let Some(mut stored) = parsed.filter(|s| s.version == VERSION) else {
            return Err(Unread::Unparseable);
        };
        let default_posters = std::mem::take(&mut stored.default_posters);
        let sources = stored
            .sources
            .iter_mut()
            .map(|s| {
                let url = std::mem::take(&mut s.url);
                let address = Address(Secret::new(url));
                PersonSource {
                    id: s.id.clone(),
                    kind: s.kind,
                    address,
                    title: s.title.clone(),
                    adult: s.adult,
                    posters: s.posters,
                    added_s: s.added_s,
                }
            })
            .collect();
        Ok(Kept {
            sources,
            default_posters,
        })
    }

    /// Keeps `kept` as the whole book, replacing the file: private from its
    /// first byte, whole or not at all.
    ///
    /// # Errors
    ///
    /// [`Unkept::Unwritable`].
    pub fn write(&self, kept: &Kept) -> Result<(), Unkept> {
        Dir::at(self.dir.clone())
            .make()
            .map_err(|_| Unkept::Unwritable)?;
        let stored = Stored {
            version: VERSION,
            sources: kept
                .sources
                .iter()
                .map(|s| StoredSource {
                    id: s.id.clone(),
                    kind: s.kind,
                    url: s.address.expose().to_owned(),
                    title: s.title.clone(),
                    adult: s.adult,
                    posters: s.posters,
                    added_s: s.added_s,
                })
                .collect(),
            default_posters: kept.default_posters.clone(),
        };
        let mut bytes = serde_json::to_vec_pretty(&stored).map_err(|_| Unkept::Unwritable)?;
        drop(stored);
        let written = write_private_file(&self.path(), &bytes);
        bytes.zeroize();
        written.map_err(|_| Unkept::Unwritable)
    }

    /// An id for a source added at `now_s` that no source in `sources` has.
    #[must_use]
    pub fn next_id(sources: &[PersonSource], now_s: i64) -> String {
        let base = format!("p{}", now_s.max(0));
        let taken = |id: &str| sources.iter().any(|s| s.id == id);
        if !taken(&base) {
            return base;
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|id| !taken(id))
            .unwrap_or(base)
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt as _;

    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-broadcast-book-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn iptv() -> PersonSource {
        PersonSource {
            id: "p1790469621".to_owned(),
            kind: Kind::Playlist,
            address: Address::new(
                "https://iptv.example:8080/get.php?username=ana&password=hunter2&type=m3u_plus",
            ),
            title: "My provider".to_owned(),
            adult: false,
            posters: false,
            added_s: 1_790_469_621,
        }
    }

    /// FAILS IF: the book is ever readable by anybody else, even for an
    /// instant, or its address reaches a `Debug`.
    #[test]
    fn the_book_is_private_from_its_first_byte_and_never_prints_an_address() {
        let root = scratch("private");
        let book = Book::at(root.join(FOLDER));
        assert_eq!(book.read(), Ok(Kept::default()), "no file is no source");
        let kept = Kept {
            sources: vec![iptv()],
            default_posters: BTreeMap::from([("lives".to_owned(), true)]),
        };
        book.write(&kept).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().mode() & 0o777;
        assert_eq!(mode(book.dir()), 0o700, "the folder");
        assert_eq!(mode(&book.path()), 0o600, "the file");
        // A FILE LEFT READABLE BY HAND is replaced by one that never was.
        std::fs::set_permissions(
            book.path(),
            std::os::unix::fs::PermissionsExt::from_mode(0o644),
        )
        .unwrap();
        book.write(&kept).unwrap();
        assert_eq!(mode(&book.path()), 0o600);

        let back = book.read().unwrap();
        assert_eq!(back, kept, "the default sources' switches come back too");
        let back = back.sources;
        assert_eq!(
            back[0].address.expose(),
            "https://iptv.example:8080/get.php?username=ana&password=hunter2&type=m3u_plus"
        );
        let printed = format!("{back:?}");
        assert!(!printed.contains("hunter2"), "{printed}");
        assert!(!printed.contains("ana"), "{printed}");
        assert!(printed.contains("iptv.example/…"), "{printed}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_book_that_will_not_parse_is_refused_and_kept() {
        let root = scratch("broken");
        let book = Book::at(root.clone());
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(book.path(), b"{ not json").unwrap();
        assert_eq!(book.read(), Err(Unread::Unparseable));
        std::fs::write(book.path(), br#"{"version": 99, "sources": []}"#).unwrap();
        assert_eq!(book.read(), Err(Unread::Unparseable), "another version");
        assert_eq!(
            std::fs::read(book.path()).unwrap(),
            br#"{"version": 99, "sources": []}"#
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_new_id_is_one_nobody_has() {
        let one = iptv();
        assert_eq!(Book::next_id(&[], 5), "p5");
        let taken = PersonSource {
            id: "p5".to_owned(),
            ..one.clone()
        };
        let taken2 = PersonSource {
            id: "p5-2".to_owned(),
            ..one
        };
        assert_eq!(Book::next_id(&[taken, taken2], 5), "p5-3");
    }
}
