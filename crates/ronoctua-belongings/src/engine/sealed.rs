//! The sealed part (feature `crypt`): format 2 — the post-quantum keyring,
//! the index and every file as chunks ([`crate::chunks`]), the plain files
//! every repository carries (`FORMAT`, `RECOVER.md`, `.gitattributes`).

use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use age::secrecy::SecretString;

use super::{Engine, INDEX_FILE, KEY_FILE};
use crate::chunks::{Store, Written};
use crate::crypt::Keyring;
use crate::kept;
use crate::merge::{Blob, INDEX_VERSION, Index, IndexHead};
use crate::plan::Kind;
use crate::secret::{draft_of, write_private};
use crate::walk;
use crate::{FORMAT, Refusal, Why};

/// The repository's guide to its own recovery, in the clear.
pub const RECOVER_FILE: &str = "RECOVER.md";

/// git's attributes file, in the clear.
pub const ATTRIBUTES_FILE: &str = ".gitattributes";

/// Every file is bytes, never a text git would convert.
pub const ATTRIBUTES: &str = "* -text\n";

/// The folder, inside the clone's git folder, of the chunks a sync swept
/// while a pull of the same sync still reads them.
pub const ASIDE_DIR: &str = "ronoctua-aside";

/// `RECOVER.md`: how a person restores every file with Go `age` and `jq`,
/// without the distro — a script `belongings-recover-without-the-distro`
/// runs as written. English: one file, one language (invariant 18).
pub const RECOVER: &str = r#"# Recovering these belongings without the distro

This repository holds a person's configuration, kept by the ronoctua-distro
belongings (format 2). Everything but this file, `FORMAT`, `.gitattributes`
and the folder `plain/` is sealed with age to a post-quantum identity
(`mlkem768x25519`), and that identity is sealed with the passphrase in
`key.age`.

`plain/` is kept in clear, as it is: the person's declared programs and
options (`plain/person/`, which is `~/.local/state/ronoctua/person/`) and
their brain modules (`plain/brain-modules/`, which is
`~/.config/ronoctua/brain-modules/`). Copy back what you want from it. It is
signed with the person's ML-DSA-65 key (`plain/signed.json`, the files and
their SHA-256; `plain/signed.sig`, the signature; `plain/signer.pub`, the
public key) so a machine takes in by itself only what the person signed. The
signature is NOT needed to recover anything, and the script below never
reads it. The signing key itself is the second line of what `key.age` holds,
written as a comment, so `age` reads that file as an identity file and
passes over it.

You need Go `age` 1.3 or later (it reads `AGE-SECRET-KEY-PQ-1` identities),
`jq`, `sha256sum` and a POSIX shell. Run the script below inside a clone of
this repository. It asks the passphrase once and writes every file under
`restored/`, by its logical path:

- `config/...` is `~/.config/ronoctua/...`
- `state/...` is `~/.local/state/ronoctua/...`
- `home/...` is the home itself

Copy back what you want from there. The identity is written to
`identity.txt`, readable by you alone, and deleted when the script ends,
however it ends: whoever holds it reads every file here. A file whose chunks
do not open, or whose content does not match the index, is named in
`damaged.txt` and left as far as it came; the script goes on with the rest,
and then ends with a failure.

```sh
set -eu
umask 077
trap 'rm -f identity.txt index.json' EXIT
trap 'exit 1' HUP INT TERM
age -d -o identity.txt key.age
chunks() {
  while read -r id; do
    age -d -i identity.txt "chunks/$(printf '%.2s' "$id")/$id.age" || return 1
  done
}
age -d -i identity.txt index.age | jq -r '.chunks[].id' | chunks > index.json || :
if [ ! -s index.json ] || ! jq -e . index.json > /dev/null 2>&1; then
  echo "the index did not open" >&2
  exit 1
fi
: > damaged.txt
jq -c '.entries[]' index.json | while read -r entry; do
  path=$(printf '%s\n' "$entry" | jq -r '.path')
  out="restored/$path"
  mkdir -p "$(dirname "$out")"
  if printf '%s\n' "$entry" | jq -r '.chunks[].id' | chunks > "$out" &&
    [ "$(sha256sum < "$out" | cut -d ' ' -f 1)" = "$(printf '%s\n' "$entry" | jq -r '.sha256')" ]; then
    if [ "$(printf '%s\n' "$entry" | jq -r '.kind')" = link ]; then
      target=$(cat "$out")
      rm -f "$out"
      ln -s "$target" "$out"
    else
      chmod "$(printf '%o' "$(printf '%s\n' "$entry" | jq -r '.mode')")" "$out"
    fi
  else
    echo "$path" >> damaged.txt
  fi
done
if [ -s damaged.txt ]; then
  cat damaged.txt >&2
  exit 1
fi
rm -f damaged.txt
```

The index is JSON. `entries` lists every file: its `path`, its `kind`
(`file`, or `link`, whose content is its target), its `mode` (a decimal
number), `sha256`, `size_bytes`, and its `chunks` in order, each
`chunks/<first two characters>/<id>.age`. `index.age` names the index's own
chunks the same way. A chunk's id is a keyed hash: it says nothing of the
content to whoever lacks the identity.
"#;

impl Engine<'_> {
    /// The format the work tree says it is.
    ///
    /// # Errors
    ///
    /// [`Why::NotOurs`] when there is no `FORMAT` or it is not ours.
    pub(super) fn format(&self) -> Result<u32, Refusal> {
        let said = std::fs::read_to_string(self.places.repo().join(super::FORMAT_FILE))
            .map_err(|_| Refusal::of(Why::NotOurs))?;
        crate::format_of(&said).ok_or(Refusal::of(Why::NotOurs))
    }

    /// `index.age`, opened: where the index is. `None` when the repository
    /// has none yet.
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] when it does not open or parse, [`Why::Newer`] for
    /// an index of a newer format.
    pub(super) fn index_head(&self, ring: &Keyring) -> Result<Option<IndexHead>, Refusal> {
        let path = self.places.repo().join(INDEX_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let plain = ring.open_bytes(&self.read_repo(INDEX_FILE)?)?;
        let head: IndexHead =
            serde_json::from_slice(&plain).map_err(|_| Refusal::of(Why::Corrupt))?;
        if head.version > INDEX_VERSION {
            return Err(Refusal::of(Why::Newer));
        }
        Ok(Some(head))
    }

    /// The index, opened from its chunks and checked against its head.
    ///
    /// # Errors
    ///
    /// As [`Engine::index_head`], and [`Why::Corrupt`] for a chunk or a body
    /// that is not what the head says.
    pub(super) fn read_index(&self, ring: &Keyring) -> Result<Index, Refusal> {
        let Some(head) = self.index_head(ring)? else {
            return Err(Refusal::of(Why::Corrupt));
        };
        let repo = self.places.repo();
        let store = Store::new(&repo, ring, kept::CHUNK_SIZES);
        let body = store.get_bytes(&head.chunks)?;
        let mut sha = aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA256);
        sha.update(&body);
        if body.len() as u64 != head.size_bytes || walk::hex(sha.finish().as_ref()) != head.sha256 {
            return Err(Refusal::of(Why::Corrupt));
        }
        let index: Index = serde_json::from_slice(&body).map_err(|_| Refusal::of(Why::Corrupt))?;
        if index.version > INDEX_VERSION {
            return Err(Refusal::of(Why::Newer));
        }
        Ok(index)
    }

    /// Writes `index` as chunks and `index.age` as its sealed head. Gives
    /// back the head.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when it could not be written.
    pub(super) fn write_index(
        &self,
        store: &Store<'_>,
        ring: &Keyring,
        index: &Index,
        written: &mut Written,
    ) -> Result<IndexHead, Refusal> {
        let body = serde_json::to_vec(index).map_err(|_| Refusal::of(Why::Failed))?;
        let put = store.put_bytes(&body, written)?;
        let head = IndexHead {
            version: INDEX_VERSION,
            size_bytes: put.size_bytes,
            sha256: put.sha256,
            chunks: put.chunks,
        };
        let text = serde_json::to_vec(&head).map_err(|_| Refusal::of(Why::Failed))?;
        std::fs::write(self.places.repo().join(INDEX_FILE), ring.seal_bytes(&text)?)
            .map_err(|_| Refusal::of(Why::Failed))?;
        Ok(head)
    }

    /// The plain files every repository of format 2 carries, written as this
    /// build has them: `FORMAT`, `RECOVER.md`, `.gitattributes`.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when one could not be written.
    pub(super) fn write_plain_files(&self) -> Result<(), Refusal> {
        let repo = self.places.repo();
        for (name, text) in [
            (super::FORMAT_FILE, format!("{FORMAT}\n")),
            (RECOVER_FILE, RECOVER.to_owned()),
            (ATTRIBUTES_FILE, ATTRIBUTES.to_owned()),
        ] {
            let path = repo.join(name);
            if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                std::fs::write(&path, text).map_err(|_| Refusal::of(Why::Failed))?;
            }
        }
        Ok(())
    }

    /// Leaves the person's post-quantum recipient (public) where a door finds
    /// it without the passphrase (`crate::fleet::recipient`). Best effort:
    /// nothing in a sync depends on it.
    pub(super) fn remember_recipient(&self, ring: &Keyring) {
        let text = format!("{}\n", ring.recipient());
        let path = self.places.recipient();
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
            let _ = write_private(&path, text.as_bytes());
        }
    }

    /// The keyring and the index of a repository of this format — what a
    /// restore reads.
    ///
    /// # Errors
    ///
    /// [`Why::Newer`] and [`Why::NotOurs`] for a repository of another
    /// format ([`crate::format_check`]), and what opening refuses.
    pub fn open_repository(&self, passphrase: &SecretString) -> Result<(Keyring, Index), Refusal> {
        crate::format_check(self.format()?)?;
        let ring = Keyring::open(&self.read_repo(KEY_FILE)?, passphrase)?;
        let index = self.read_index(&ring)?;
        Ok((ring, index))
    }

    /// What the repository holds for `blob`, whole — a text of the session,
    /// at most [`super::TEXT_MOST_BYTES`].
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`] when it does not open.
    pub(super) fn read_blob(&self, ring: &Keyring, blob: &Blob) -> Result<Vec<u8>, Refusal> {
        let repo = self.places.repo();
        Store::new(&repo, ring, kept::CHUNK_SIZES).get_bytes(&blob.chunks)
    }

    /// Opens what the repository has for `blob` into `to`.
    fn open_blob(
        &self,
        ring: &Keyring,
        blob: &Blob,
        to: &mut dyn std::io::Write,
    ) -> Result<(), Refusal> {
        let repo = self.places.repo();
        let aside = self.aside();
        Store::new(&repo, ring, kept::CHUNK_SIZES)
            .with_aside(&aside)
            .get(&blob.chunks, to)
            .map(|_| ())
    }

    /// Where a sync puts the chunks it swept that a pull still reads: inside
    /// git's own folder, so no commit ever carries them; emptied when the
    /// sync is done.
    pub(super) fn aside(&self) -> PathBuf {
        self.places.repo().join(".git").join(ASIDE_DIR)
    }

    /// Writes what the repository has for `logical` at `dest`: what was there
    /// set aside first, the file through a draft born with its mode, its
    /// chunks streamed in order through the SHA-256 check, a link as a link.
    pub(super) fn pull(
        &self,
        ring: &Keyring,
        blob: &Blob,
        dest: &Path,
        stamp: &Path,
        logical: &str,
    ) -> Result<(), Refusal> {
        let failed = |_| Refusal::of(Why::Failed);
        // A PRIVATE TARGET'S FILE comes back with its owner's bits alone,
        // whatever an older index says (`kept::kept_mode`).
        let mode = kept::kept_mode(logical, blob.side.mode & 0o7777);
        let private = mode & 0o077 == 0;
        if let Some(parent) = dest.parent() {
            if private {
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)
                    .map_err(failed)?;
            } else {
                std::fs::create_dir_all(parent).map_err(failed)?;
            }
        }
        self.set_aside(dest, stamp, logical)?;
        if blob.side.kind == Kind::Link {
            let mut target = Vec::new();
            self.open_blob(ring, blob, &mut target)?;
            let target: std::ffi::OsString = std::os::unix::ffi::OsStringExt::from_vec(target);
            let target = PathBuf::from(target);
            let _ = std::fs::remove_file(dest);
            std::os::unix::fs::symlink(&target, dest).map_err(failed)?;
            return Ok(());
        }
        let draft = draft_of(dest);
        let _ = std::fs::remove_file(&draft);
        // BORN WITH ITS MODE: a private file is never readable, not even for
        // the time between a write and a `chmod`.
        let mut to = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode | 0o600)
            .open(&draft)
            .map_err(failed)?;
        let mut hashing = Hashing::new(&mut to);
        let opened = self.open_blob(ring, blob, &mut hashing);
        let sha = hashing.finish();
        if let Err(r) = opened.and_then(|()| {
            if sha == blob.side.sha256 {
                Ok(())
            } else {
                Err(Refusal::of(Why::Corrupt))
            }
        }) {
            let _ = std::fs::remove_file(&draft);
            return Err(r);
        }
        to.sync_all().map_err(failed)?;
        drop(to);
        // THE EXACT MODE: the umask took bits the file had elsewhere.
        std::fs::set_permissions(&draft, std::fs::Permissions::from_mode(mode)).map_err(failed)?;
        std::fs::rename(&draft, dest).map_err(failed)
    }

    /// Cuts this machine's `logical` into chunks (writing those the
    /// repository lacks); `None` when it is gone since the walk. Its size and
    /// SHA-256 are the ones of the bytes chunked — read once — and its
    /// "added" mark is the carrying's to set.
    pub(super) fn push(
        &self,
        store: &Store<'_>,
        logical: &str,
        source: &Path,
        written: &mut Written,
    ) -> Result<Option<Blob>, Refusal> {
        let failed = |_| Refusal::of(Why::Failed);
        let Some(mut side) = walk::side_of(source).map_err(failed)? else {
            return Ok(None);
        };
        // THE MODE THE WALK READ, a private target's owner's bits alone.
        if side.kind == Kind::File {
            side.mode = kept::kept_mode(logical, side.mode);
        }
        let put = if side.kind == Kind::Link {
            let target = std::fs::read_link(source).map_err(failed)?;
            let bytes = std::os::unix::ffi::OsStrExt::as_bytes(target.as_os_str()).to_vec();
            store.put_bytes(&bytes, written)?
        } else {
            let mut from = std::fs::File::open(source).map_err(failed)?;
            store.put(&mut from, written)?
        };
        // WHAT WAS CHUNKED is what the index says: a file that changed between
        // the walk and now is kept as it was chunked, and the next sync sees
        // the rest.
        side.size_bytes = put.size_bytes;
        side.sha256 = put.sha256;
        Ok(Some(Blob {
            path: logical.to_owned(),
            side,
            machine: self.machine.clone(),
            added: false,
            chunks: put.chunks,
        }))
    }
}

/// A writer that hashes what goes through it.
struct Hashing<'w> {
    to: &'w mut dyn std::io::Write,
    context: aws_lc_rs::digest::Context,
}

impl<'w> Hashing<'w> {
    fn new(to: &'w mut dyn std::io::Write) -> Self {
        Self {
            to,
            context: aws_lc_rs::digest::Context::new(&aws_lc_rs::digest::SHA256),
        }
    }

    fn finish(self) -> String {
        walk::hex(self.context.finish().as_ref())
    }
}

impl std::io::Write for Hashing<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.to.write(buf)?;
        self.context.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.to.flush()
    }
}

// ── anchor: L08 ──
