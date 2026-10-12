//! The chunk store (feature `crypt`): every sealed file of format 2, cut
//! into content-defined chunks.
//!
//! # Why chunks
//!
//! GitHub refuses a file over 100 MB and a push over 2 GiB, and a blob per
//! file rewrote a whole video for one changed byte. A file is now cut by
//! FastCDC (v2020, `chunk.minKiB`/`avgKiB`/`maxKiB` of `assets/belongings.nix`,
//! [`crate::kept::CHUNK_SIZES`]): where it is cut depends on the content, so
//! an edit moves the chunks around it and leaves the rest as they were.
//!
//! # Named by a keyed hash, written once
//!
//! A chunk lives at `chunks/<aa>/<id>.age`, sealed to the person's identity;
//! `<id>` is [`Keyring::chunk_id`] — HMAC-SHA256 of the plaintext under a key
//! derived from the identity, never the content's own SHA-256. A chunk is
//! written only when its file is ABSENT ([`Store::put`]): age's encryption is
//! randomised, so sealing it again would change a file nothing changed, and
//! an unchanged file must never move in git. Two files with the same content
//! are one set of chunks.
//!
//! # Read back in order, and checked
//!
//! [`Store::get`] opens each chunk, recomputes its id from what opened, and
//! refuses one that does not match its name or its size — a chunk moved, or
//! swapped for another, is [`Why::Corrupt`]; the whole file's SHA-256 is then
//! checked by the caller as it always was.
//!
//! # What leaks
//!
//! How many chunks a file is, and each one's size (the gear table is seeded
//! from the identity, so a known file is not cut where a stranger would cut
//! it); and that two files, or two versions of one, share content — a chunk
//! named twice is one file.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::crypt::Keyring;
use crate::kept::ChunkSizes;
use crate::{Refusal, Why};

/// The folder of the chunks in the repository.
pub const CHUNKS_DIR: &str = "chunks";

/// How long an id is: 32 bytes of HMAC-SHA256, hexadecimal.
pub const ID_CHARS: usize = 64;

/// The most a sealed chunk adds to its plaintext: age's header (the
/// post-quantum stanza is ~1.6 KiB of base64), and 16 bytes per 64 KiB of
/// payload. A bound, for the check that no repository file reaches the
/// limit.
#[must_use]
pub const fn sealed_most_bytes(plain_bytes: u64) -> u64 {
    plain_bytes + 8 * 1024 + plain_bytes.div_ceil(64 * 1024) * 16
}

/// One chunk of a file, in the index.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChunkRef {
    /// Its name ([`Keyring::chunk_id`]).
    pub id: String,
    /// Its plaintext size, in bytes.
    pub size_bytes: u64,
}

/// Whether `id` is a chunk's name: 64 lowercase hexadecimal characters —
/// what an index says becomes a path only if it is one. Pure.
#[must_use]
pub fn is_id(id: &str) -> bool {
    id.len() == ID_CHARS
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The chunk's file, relative to the repository: `chunks/<aa>/<id>.age`.
/// `None` for what is not a chunk's name. Pure.
#[must_use]
pub fn relative(id: &str) -> Option<String> {
    is_id(id).then(|| format!("{CHUNKS_DIR}/{}/{id}.age", &id[..2]))
}

/// The id a repository path names, when it is a chunk's. Pure.
#[must_use]
pub fn id_of(relative_path: &str) -> Option<&str> {
    let rest = relative_path.strip_prefix(CHUNKS_DIR)?.strip_prefix('/')?;
    let (fan, file) = rest.split_once('/')?;
    let id = file.strip_suffix(".age")?;
    (is_id(id) && fan == &id[..2]).then_some(id)
}

/// What [`Store::put`] made of a stream.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Put {
    /// Its chunks, in order.
    pub chunks: Vec<ChunkRef>,
    /// The whole stream's SHA-256, hexadecimal.
    pub sha256: String,
    /// The whole stream's size, in bytes.
    pub size_bytes: u64,
}

/// The chunk files a run wrote, in the order it wrote them, with their
/// sealed sizes — what a push too large for one goes up in batches of.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Written {
    /// Each file, relative to the repository, and its sealed size in bytes.
    pub files: Vec<(String, u64)>,
}

impl Written {
    /// How many bytes they weigh.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|(_, b)| b).sum()
    }
}

/// The batches `written` goes up in when it weighs more than `most_bytes`:
/// each at most `most_bytes` (one chunk alone past it is its own batch), in
/// the order written. Empty when one push carries everything. Pure.
#[must_use]
pub fn batches(written: &Written, most_bytes: u64) -> Vec<Vec<String>> {
    if written.bytes() <= most_bytes {
        return Vec::new();
    }
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut weight = 0u64;
    for (file, bytes) in &written.files {
        if out.is_empty() || weight.saturating_add(*bytes) > most_bytes {
            out.push(Vec::new());
            weight = 0;
        }
        weight = weight.saturating_add(*bytes);
        if let Some(last) = out.last_mut() {
            last.push(file.clone());
        }
    }
    out
}

/// The chunks of one repository's work tree, sealed to one identity.
pub struct Store<'s> {
    /// The work tree's root.
    root: &'s Path,
    /// Where swept chunks wait for the pulls that still read them
    /// ([`Store::put_aside`]), when there is such a place.
    aside: Option<&'s Path>,
    /// The identity they are sealed to and named with.
    ring: &'s Keyring,
    /// Where the chunker cuts.
    sizes: ChunkSizes,
}

impl std::fmt::Debug for Store<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("root", &self.root)
            .field("sizes", &self.sizes)
            .finish_non_exhaustive()
    }
}

impl<'s> Store<'s> {
    /// The store of the work tree at `root`.
    #[must_use]
    pub const fn new(root: &'s Path, ring: &'s Keyring, sizes: ChunkSizes) -> Self {
        Self {
            root,
            aside: None,
            ring,
            sizes,
        }
    }

    /// The same store, reading too what was put aside in `dir`.
    #[must_use]
    pub const fn with_aside(mut self, dir: &'s Path) -> Self {
        self.aside = Some(dir);
        self
    }

    /// Where a chunk's file is; `None` for what is not a chunk's name.
    #[must_use]
    pub fn path(&self, id: &str) -> Option<PathBuf> {
        relative(id).map(|r| self.root.join(r))
    }

    /// Moves a swept chunk out of the work tree into the aside folder —
    /// out of the commit, still there for a pull that reads it after the
    /// push (the repository's side of a path both machines keep). Without an
    /// aside folder it is deleted.
    pub fn put_aside(&self, id: &str) {
        let (Some(from), Some(dir)) = (self.path(id), self.aside) else {
            self.remove(id);
            return;
        };
        let to = dir.join(format!("{id}.age"));
        if std::fs::create_dir_all(dir)
            .and_then(|()| std::fs::rename(&from, &to))
            .is_err()
        {
            self.remove(id);
        }
    }

    /// Whether the chunk is there.
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.path(id).is_some_and(|p| p.is_file())
    }

    /// Cuts everything `from` gives into chunks, writes those that are not
    /// there yet (adding them to `written`), and says what it was.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when a read or a write failed.
    pub fn put(&self, from: &mut dyn Read, written: &mut Written) -> Result<Put, Refusal> {
        let chunker = fastcdc::v2020::StreamCDC::with_level_and_seed(
            from,
            self.sizes.min_bytes,
            self.sizes.avg_bytes,
            self.sizes.max_bytes,
            fastcdc::v2020::Normalization::Level1,
            self.ring.seed(),
        );
        let mut whole = aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA256);
        let mut put = Put::default();
        for chunk in chunker {
            let chunk = chunk.map_err(|_| Refusal::of(Why::Failed))?;
            whole.update(&chunk.data);
            let size_bytes = chunk.data.len() as u64;
            put.size_bytes += size_bytes;
            let id = self.ring.chunk_id(&chunk.data);
            self.write(&id, &chunk.data, written)?;
            put.chunks.push(ChunkRef { id, size_bytes });
        }
        put.sha256 = crate::walk::hex(whole.finish().as_ref());
        Ok(put)
    }

    /// The same, for bytes in hand.
    ///
    /// # Errors
    ///
    /// As [`Store::put`].
    pub fn put_bytes(&self, bytes: &[u8], written: &mut Written) -> Result<Put, Refusal> {
        self.put(&mut &bytes[..], written)
    }

    /// Seals one chunk into its file when the file is not there — through a
    /// draft beside it, so a run that dies half way leaves no chunk whose
    /// name lies about it.
    fn write(&self, id: &str, plain: &[u8], written: &mut Written) -> Result<(), Refusal> {
        let failed = |_| Refusal::of(Why::Failed);
        let rel = relative(id).ok_or(Refusal::of(Why::Failed))?;
        let path = self.root.join(&rel);
        if path.is_file() {
            return Ok(());
        }
        if written.files.iter().any(|(f, _)| *f == rel) {
            return Ok(());
        }
        let parent = path.parent().ok_or(Refusal::of(Why::Failed))?;
        std::fs::create_dir_all(parent).map_err(failed)?;
        let sealed = self.ring.seal_bytes(plain)?;
        let draft = parent.join(format!(".{id}.{}.tmp", std::process::id()));
        let _ = std::fs::remove_file(&draft);
        let wrote = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&draft)
            .and_then(|mut f| f.write_all(&sealed).and_then(|()| f.sync_all()))
            .and_then(|()| std::fs::rename(&draft, &path));
        if wrote.is_err() {
            let _ = std::fs::remove_file(&draft);
        }
        wrote.map_err(failed)?;
        written.files.push((rel, sealed.len() as u64));
        Ok(())
    }

    /// Opens one chunk, checked: what opened must have the chunk's name and
    /// size.
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] when it is missing, does not open, or is not what its
    /// name says.
    pub fn open(&self, chunk: &ChunkRef) -> Result<Vec<u8>, Refusal> {
        let corrupt = Refusal::of(Why::Corrupt);
        let path = self.path(&chunk.id).ok_or(corrupt)?;
        let path = match self.aside {
            Some(dir) if !path.is_file() => dir.join(format!("{}.age", chunk.id)),
            _ => path,
        };
        let mut file = std::fs::File::open(&path).map_err(|_| corrupt)?;
        let mut plain = Vec::with_capacity(usize::try_from(chunk.size_bytes).unwrap_or(0));
        self.ring.open_into(&mut file, &mut plain)?;
        if plain.len() as u64 != chunk.size_bytes || self.ring.chunk_id(&plain) != chunk.id {
            return Err(corrupt);
        }
        Ok(plain)
    }

    /// Streams `chunks` in order into `to`, each checked ([`Store::open`]).
    /// Gives back how many bytes were written.
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] for a chunk that is not what the index says,
    /// [`Why::Failed`] when a write failed.
    pub fn get(&self, chunks: &[ChunkRef], to: &mut dyn Write) -> Result<u64, Refusal> {
        let mut done = 0u64;
        for chunk in chunks {
            let plain = self.open(chunk)?;
            to.write_all(&plain).map_err(|_| Refusal::of(Why::Failed))?;
            done += plain.len() as u64;
        }
        Ok(done)
    }

    /// The same, into bytes in hand.
    ///
    /// # Errors
    ///
    /// As [`Store::get`].
    pub fn get_bytes(&self, chunks: &[ChunkRef]) -> Result<Vec<u8>, Refusal> {
        let mut out = Vec::new();
        self.get(chunks, &mut out)?;
        Ok(out)
    }

    /// Deletes a chunk's file (a sweep). Missing is not an error.
    pub fn remove(&self, id: &str) {
        if let Some(path) = self.path(id) {
            let _ = std::fs::remove_file(path);
        }
    }

    /// Every chunk the work tree holds, by id.
    #[must_use]
    pub fn on_disk(&self) -> BTreeSet<String> {
        on_disk(self.root)
    }
}

/// Every chunk the work tree at `root` holds, by id.
#[must_use]
pub fn on_disk(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(fans) = std::fs::read_dir(root.join(CHUNKS_DIR)) else {
        return out;
    };
    for fan in fans.flatten() {
        let Ok(files) = std::fs::read_dir(fan.path()) else {
            continue;
        };
        let fan = fan.file_name().to_string_lossy().into_owned();
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let rel = format!("{CHUNKS_DIR}/{fan}/{name}");
            if let Some(id) = id_of(&rel) {
                out.insert(id.to_owned());
            }
        }
    }
    out
}

// ── anchor: L08 ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::passphrase;

    const WORK: Option<u8> = Some(10);

    /// Small sizes, so a test cuts many chunks out of little data.
    const SMALL: ChunkSizes = ChunkSizes {
        min_bytes: 1024,
        avg_bytes: 4096,
        max_bytes: 16 * 1024,
    };

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ronoctua-belongings-chunks-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Bytes no compressor and no chunker find a pattern in.
    fn noise(seed: u64, len: usize) -> Vec<u8> {
        let mut x = seed | 1;
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            out.extend_from_slice(&x.to_le_bytes());
        }
        out.truncate(len);
        out
    }

    #[test]
    fn a_chunk_path_is_made_only_of_a_name() {
        let id = "ab".repeat(32);
        assert_eq!(
            relative(&id).as_deref(),
            Some(format!("chunks/ab/{id}.age").as_str())
        );
        assert_eq!(id_of(&relative(&id).unwrap()), Some(id.as_str()));
        for bad in ["", "../../etc/passwd", &"AB".repeat(32), &"ab".repeat(31)] {
            assert_eq!(relative(bad), None, "{bad}");
        }
        assert_eq!(id_of(&format!("chunks/cd/{id}.age")), None, "a wrong fan");
        assert_eq!(id_of("blobs/0123.age"), None);
    }

    /// FAILS IF: a stream does not come back byte for byte, a chunk is
    /// written twice, one past its size limit is made, or a chunk swapped for
    /// another opens as if it were the one named.
    #[test]
    fn a_stream_comes_back_whole_and_a_chunk_is_written_once() {
        let root = scratch("whole");
        let p = passphrase("p".to_owned());
        let (ring, _) = Keyring::new(&p, WORK).unwrap();
        let store = Store::new(&root, &ring, SMALL);
        let data = noise(7, 300 * 1024);
        let mut written = Written::default();
        let put = store.put_bytes(&data, &mut written).unwrap();
        assert!(put.chunks.len() > 10, "{} chunks", put.chunks.len());
        assert_eq!(put.size_bytes, data.len() as u64);
        assert!(
            put.chunks
                .iter()
                .all(|c| c.size_bytes <= u64::from(SMALL.max_bytes))
        );
        assert_eq!(written.files.len(), put.chunks.len());
        assert_eq!(store.get_bytes(&put.chunks).unwrap(), data);

        // THE SAME CONTENT AGAIN writes nothing.
        let mut again = Written::default();
        let second = store.put_bytes(&data, &mut again).unwrap();
        assert_eq!(second, put);
        assert!(again.files.is_empty(), "{:?}", again.files);
        assert_eq!(store.on_disk().len(), put.chunks.len());

        // A CHUNK SWAPPED FOR ANOTHER is refused, never opened as the one named.
        let (a, b) = (&put.chunks[0], &put.chunks[1]);
        std::fs::copy(store.path(&b.id).unwrap(), store.path(&a.id).unwrap()).unwrap();
        assert_eq!(store.get_bytes(&put.chunks).unwrap_err().why, Why::Corrupt);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_large_push_goes_in_batches_under_the_limit() {
        let written = Written {
            files: (0..10).map(|n| (format!("c{n}"), 40)).collect(),
        };
        assert!(batches(&written, 400).is_empty(), "one push carries 400");
        let cut = batches(&written, 100);
        assert_eq!(cut.len(), 5);
        assert!(cut.iter().all(|b| b.len() == 2));
        assert_eq!(
            cut.iter().flatten().count(),
            10,
            "a chunk was lost between batches"
        );
        // ONE CHUNK PAST THE LIMIT is a batch of its own.
        let big = Written {
            files: vec![
                ("a".to_owned(), 10),
                ("b".to_owned(), 500),
                ("c".to_owned(), 10),
            ],
        };
        assert_eq!(batches(&big, 100), [vec!["a"], vec!["b"], vec!["c"]]);
    }

    /// FAILS IF: one byte changed in the middle of a file moves more than the
    /// chunks around it.
    #[test]
    fn one_byte_changed_moves_few_chunks() {
        let root = scratch("edit");
        let p = passphrase("p".to_owned());
        let (ring, _) = Keyring::new(&p, WORK).unwrap();
        let store = Store::new(&root, &ring, SMALL);
        let mut data = noise(11, 512 * 1024);
        let before = store.put_bytes(&data, &mut Written::default()).unwrap();
        data[256 * 1024] ^= 0xff;
        let mut written = Written::default();
        let after = store.put_bytes(&data, &mut written).unwrap();
        assert!(
            written.files.len() <= 2,
            "{} new chunks",
            written.files.len()
        );
        let old: BTreeSet<&str> = before.chunks.iter().map(|c| c.id.as_str()).collect();
        let kept = after
            .chunks
            .iter()
            .filter(|c| old.contains(c.id.as_str()))
            .count();
        assert!(kept + 2 >= after.chunks.len());
        let _ = std::fs::remove_dir_all(root);
    }
}
