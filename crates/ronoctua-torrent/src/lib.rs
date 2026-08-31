//! BitTorrent, v1 and v2 — the safe half of the border with libtorrent.
//!
//! # Why libtorrent and not a client of our own
//!
//! The owner asked for BitTorrent **v2**. Measured on the pinned nixpkgs, not
//! assumed: the `librqbit` of `rqbit` 8.1.1 has no `meta version`, no
//! `file tree` and no `piece layers` — its only trace of v2 is a type alias
//! with a comment. Writing v2 means metainfo v2, Merkle verification with
//! SHA-256, hybrid torrents and the v2 half of the peer protocol; it is
//! specialist work on an uncertain schedule.
//!
//! `libtorrent-rasterbar` 2.0 speaks it today, and it is not the heavy option:
//! its closure is **62,7 MiB** against `rqbit`'s **69,8 MiB** and
//! `qbittorrent-nox`'s **886,7 MiB** — what makes those daemons big is Qt and
//! Python, not the engine. It brings no `tokio` either.
//!
//! What is given up is authorship of the engine. The app is still ours: the
//! queue, the panel, the keyboard, the texts and the `.desktop` that catches a
//! magnet. The engine is not, and that trade was made deliberately — v2
//! working is worth more than having written the swarm logic.
//!
//! # `deny`, and not `forbid`
//!
//! `docs/ARCHITECTURE.md` says the FFI is isolated. With `forbid` that promise
//! cannot be relaxed anywhere, not even by the module that means it. With
//! `deny`, unsafe is refused everywhere EXCEPT where a module says otherwise in
//! writing — and today that is exactly one file, [`ffi`]. The isolation stops
//! being a hope and becomes a grep for `allow(unsafe_code)`. It is the same
//! arrangement `crates/ronoctua-term` uses, word for word.
//!
//! # The session never moves
//!
//! A libtorrent session owns threads, sockets and a disk cache, and its handles
//! are not something to pass around casually. [`Session`] is therefore built
//! INSIDE the thread that uses it and never leaves — the same discipline
//! `ronoctua-term`'s `Terminal` uses for a `!Send` engine, and the reason
//! neither needed a lock.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod ffi;

use anyhow::{Result, bail};

/// Where a torrent is, in this project's terms rather than libtorrent's.
///
/// Six stages for a copy, a download and a torrent alike: the queue has one
/// vocabulary, and a screen that used a different one per engine would make
/// the reader learn three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Reading what is already on disk.
    Checking,
    /// It has the magnet and is looking for the metadata.
    ///
    /// A stage of its own because it is the one where a bar cannot exist: with
    /// no metadata there is no total, and a magnet can sit here for a while.
    Metadata,
    /// Moving.
    Downloading,
    /// It has everything it wanted.
    Finished,
    /// It has everything and is giving it back.
    Seeding,
    /// It stopped, and `trouble` says why.
    Errored,
}

impl State {
    const fn of(raw: i32) -> Self {
        match raw {
            0 => Self::Checking,
            1 => Self::Metadata,
            3 => Self::Finished,
            4 => Self::Seeding,
            5 => Self::Errored,
            // Two and anything unknown: a state this side has not learned to
            // read is far better shown as "moving" than as a panic.
            _ => Self::Downloading,
        }
    }
}

/// One torrent, as the session last reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Torrent {
    /// The v2 info hash when there is one, the v1 hash otherwise.
    ///
    /// Never libtorrent's `get_best()`, which TRUNCATES a SHA-256 to twenty
    /// bytes so v1 code paths can carry it: truncated, two different v2
    /// torrents can collide, and what this names is what a cancel deletes.
    pub hash: String,
    /// Its own name, as the torrent gives it.
    pub name: String,
    /// A failure, or empty.
    pub trouble: String,
    /// Bytes of the WANTED files that are done.
    pub done: u64,
    /// Bytes of the wanted files in total. Zero while the metadata is missing.
    pub total: u64,
    /// Bytes per second, down and up.
    pub down: u64,
    /// Bytes per second going back out.
    pub up: u64,
    /// Where it is.
    pub state: State,
    /// Whether it is held.
    pub paused: bool,
    /// Whether it has a v1 info hash.
    pub v1: bool,
    /// Whether it has a v2 info hash — a BEP 52 torrent, hybrid or pure.
    pub v2: bool,
    /// How many peers are connected.
    pub peers: i32,
    /// How many of them have the whole thing.
    pub seeds: i32,
}

/// How many rows one poll may bring back.
///
/// A fixed buffer, so a poll allocates nothing and the other side never has to
/// hand back memory this side would have to free. Two hundred and fifty-six
/// torrents at once is far past what a person watches, and the queue shows
/// what fits anyway.
const ROOM: usize = 256;

/// A live BitTorrent session.
///
/// Built inside the thread that uses it, and never sent anywhere: see the
/// crate documentation.
pub struct Session {
    #[cfg(torrent)]
    raw: ffi::Raw,
    #[cfg(torrent)]
    rows: Vec<ffi::Status>,
}

/// Whether this build has the engine at all.
///
/// Outside Nix — a bare `cargo check` — the shim is absent and the crate
/// compiles without it. The queue asks this before taking a torrent job, so an
/// engine that is not there says so on the row instead of showing a job that
/// never moves.
#[must_use]
pub const fn available() -> bool {
    cfg!(torrent)
}

/// What libtorrent version is linked, for the line in the log that says what
/// is actually running rather than what was meant to be.
#[must_use]
pub fn version() -> String {
    #[cfg(torrent)]
    {
        ffi::version()
    }
    #[cfg(not(torrent))]
    {
        String::new()
    }
}

/// Which BitTorrent the torrent being made speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavour {
    /// Both, in one file — what most clients still produce.
    Hybrid,
    /// v1 alone.
    One,
    /// v2 alone (BEP 52): `meta version 2`, a file tree, piece layers, and no
    /// v1 info dictionary at all.
    Two,
}

/// Writes a `.torrent` describing what is at `content`.
///
/// A real feature — a torrent client that cannot make one is half a client —
/// and also what makes "this build speaks BEP 52" checkable OFFLINE rather
/// than a claim resting on a changelog.
///
/// # Errors
///
/// When the shim is missing, the content is not there, or libtorrent refuses.
pub fn make_torrent(
    content: &std::path::Path,
    out_file: &std::path::Path,
    flavour: Flavour,
) -> Result<()> {
    #[cfg(torrent)]
    {
        let (Some(content), Some(out)) = (ffi::c_path(content), ffi::c_path(out_file)) else {
            bail!("the path has a NUL in it");
        };
        let flavour = match flavour {
            Flavour::Hybrid => 0,
            Flavour::One => 1,
            Flavour::Two => 2,
        };
        if ffi::make_torrent(&content, &out, flavour) {
            return Ok(());
        }
        bail!("libtorrent refused to create the torrent")
    }
    #[cfg(not(torrent))]
    {
        let _ = (content, out_file, flavour);
        bail!("this build does not have libtorrent's shim")
    }
}

impl Session {
    /// Opens a session, restoring whatever the state directory holds.
    ///
    /// # Errors
    ///
    /// When the shim is not in this build, or libtorrent refuses to listen.
    pub fn open(state_dir: &std::path::Path, ports: (u16, u16)) -> Result<Self> {
        #[cfg(torrent)]
        {
            let Some(dir) = ffi::c_path(state_dir) else {
                bail!("the state's path has a NUL in it");
            };
            let Some(raw) = ffi::Raw::open(&dir, ports.0, ports.1) else {
                bail!(
                    "libtorrent refused to bring the session up on ports {}-{}",
                    ports.0,
                    ports.1
                );
            };
            let mut rows = Vec::new();
            rows.resize_with(ROOM, ffi::Status::empty);
            tracing::info!(libtorrent = %ffi::version(), "sessao de torrent no ar");
            Ok(Self { raw, rows })
        }
        // WITHOUT THE SHIM THE ENGINE SAYS SO, and the row shows it. The
        // alternative — a job that is accepted and never moves — is the one
        // failure a queue must never produce.
        #[cfg(not(torrent))]
        {
            let _ = (state_dir, ports);
            bail!("this build does not have libtorrent's shim")
        }
    }

    /// Adds a magnet, and gives back the hash it is known by.
    ///
    /// # Errors
    ///
    /// When the URI is not a magnet, or the session refuses it.
    pub fn add_magnet(&mut self, uri: &str, into: &std::path::Path) -> Result<String> {
        #[cfg(torrent)]
        {
            let (Ok(uri), Some(into)) = (std::ffi::CString::new(uri), ffi::c_path(into)) else {
                bail!("the address or the destination has a NUL in it");
            };
            self.raw
                .add_magnet(&uri, &into)
                .ok_or_else(|| anyhow::anyhow!("libtorrent refused the magnet"))
        }
        #[cfg(not(torrent))]
        {
            let _ = (uri, into);
            bail!("this build does not have libtorrent's shim")
        }
    }

    /// The same, from a `.torrent` on disk.
    ///
    /// # Errors
    ///
    /// When the file is not a torrent, or the session refuses it.
    pub fn add_file(&mut self, path: &std::path::Path, into: &std::path::Path) -> Result<String> {
        #[cfg(torrent)]
        {
            let (Some(path), Some(into)) = (ffi::c_path(path), ffi::c_path(into)) else {
                bail!("the path or the destination has a NUL in it");
            };
            self.raw
                .add_file(&path, &into)
                .ok_or_else(|| anyhow::anyhow!("libtorrent refused the file"))
        }
        #[cfg(not(torrent))]
        {
            let _ = (path, into);
            bail!("this build does not have libtorrent's shim")
        }
    }

    /// Holds it. `false` when the hash names nothing.
    pub fn pause(&mut self, hash: &str) -> bool {
        #[cfg(torrent)]
        {
            std::ffi::CString::new(hash).is_ok_and(|h| self.raw.pause(&h))
        }
        #[cfg(not(torrent))]
        {
            let _ = hash;
            false
        }
    }

    /// Lets it go again.
    pub fn resume(&mut self, hash: &str) -> bool {
        #[cfg(torrent)]
        {
            std::ffi::CString::new(hash).is_ok_and(|h| self.raw.resume(&h))
        }
        #[cfg(not(torrent))]
        {
            let _ = hash;
            false
        }
    }

    /// Gives up on it, taking what was written with it when asked.
    pub fn remove(&mut self, hash: &str, delete_files: bool) -> bool {
        #[cfg(torrent)]
        {
            std::ffi::CString::new(hash).is_ok_and(|h| self.raw.remove(&h, delete_files))
        }
        #[cfg(not(torrent))]
        {
            let _ = (hash, delete_files);
            false
        }
    }

    /// Where everything is, right now.
    #[must_use]
    pub fn poll(&mut self) -> Vec<Torrent> {
        #[cfg(torrent)]
        {
            let n = self.raw.poll(&mut self.rows);
            self.rows
                .iter()
                .take(n)
                .map(|r| Torrent {
                    hash: ffi::field(&r.hash),
                    name: ffi::field(&r.name),
                    trouble: ffi::field(&r.trouble),
                    done: r.done,
                    total: r.total,
                    down: r.down_rate,
                    up: r.up_rate,
                    state: State::of(r.state),
                    paused: r.flags & ffi::FLAG_PAUSED != 0,
                    v1: r.flags & ffi::FLAG_HAS_V1 != 0,
                    v2: r.flags & ffi::FLAG_HAS_V2 != 0,
                    peers: r.peers,
                    seeds: r.seeds,
                })
                .collect()
        }
        #[cfg(not(torrent))]
        {
            Vec::new()
        }
    }

    /// Writes resume data for everything, so a reboot does not re-check.
    pub fn save(&mut self) {
        #[cfg(torrent)]
        self.raw.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_state_reads_as_moving_and_never_panics() {
        // A state this side has not learned about is far better shown as
        // "moving" than as a crash inside a queue that is copying elsewhere.
        assert_eq!(State::of(99), State::Downloading);
        assert_eq!(State::of(-1), State::Downloading);
        assert_eq!(State::of(1), State::Metadata);
        assert_eq!(State::of(5), State::Errored);
    }

    #[test]
    fn opening_without_the_shim_fails_instead_of_pretending() {
        // A queue that ACCEPTED a torrent job with no engine behind it would
        // show a row that never moves, which is the one failure a queue must
        // never produce. Without the shim, `open` says so at the door.
        let out = Session::open(std::path::Path::new("/does/not/exist"), (6881, 6889));
        assert_eq!(out.is_err(), !available());
    }
}
