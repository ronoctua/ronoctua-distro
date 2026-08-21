//! What a piece of long work IS, while it is happening.
//!
//! # Why the model is one type and not one per engine
//!
//! Because the screen is one list. A copy of forty gibibytes, a torrent and a
//! `yt-dlp` are three completely different mechanisms and exactly the same
//! question to whoever is looking: *what is it, how far along, and can I stop
//! it*. A row per engine would be three drawings, three sets of keys and three
//! ways to be paused — and the person would have to learn which list their
//! thing is in before they could stop it.
//!
//! So the engine is a FIELD. What varies between them is what fills the fields
//! in and which of the switches they honour, and both of those are answered by
//! data rather than by a second code path.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::wire::Paths;

/// Who is doing the moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Engine {
    /// This machine's own disk, through [`ronoctua_files`].
    Local,
    /// A plain link, through aria2's RPC.
    Fetch,
    /// A site's media, through yt-dlp.
    Media,
    /// BitTorrent, through the session this unit owns.
    Torrent,
}

impl Engine {
    /// Every one of them, for the check that demands a text and an icon per
    /// engine. A `match` would compile with one missing; a list is what can be
    /// compared against the `.ftl`.
    pub const ALL: [Self; 4] = [Self::Local, Self::Fetch, Self::Media, Self::Torrent];

    /// The stem of its text key and of its icon token.
    ///
    /// One word feeding both, so an engine cannot end up with a name on screen
    /// and no glyph — which is what happens when the two tables are written
    /// separately.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Fetch => "fetch",
            Self::Media => "media",
            Self::Torrent => "torrent",
        }
    }
}

/// Where a job is.
///
/// [`Stage::Weighing`] is a stage of its own and not a slow start: measuring a
/// folder of a hundred thousand files takes long enough that a bar sitting at
/// zero would read as stuck, and it is the one moment when stopping costs
/// nothing at all — not a byte has moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    /// Finding out how much there is.
    Weighing,
    /// Moving.
    Running,
    /// Held, keeping everything it has done.
    Paused,
    /// Over, and it worked.
    Done,
    /// Over, and it did not.
    Failed,
    /// Over, because somebody said so.
    Cancelled,
}

impl Stage {
    /// Every one of them — see [`Engine::ALL`] for why this is a list.
    pub const ALL: [Self; 6] = [
        Self::Weighing,
        Self::Running,
        Self::Paused,
        Self::Done,
        Self::Failed,
        Self::Cancelled,
    ];

    /// The stem of its text key and its icon token.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Weighing => "weighing",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Is there still something happening?
    ///
    /// It is what decides whether the modeline shows a segment at all, and
    /// whether a finished job may be swept away.
    #[must_use]
    pub const fn afoot(self) -> bool {
        matches!(self, Self::Weighing | Self::Running | Self::Paused)
    }
}

/// What the counter counts.
///
/// Bytes for a copy, entries for a delete — and the difference is not
/// cosmetic. A directory of two hundred thousand empty files weighs nothing and
/// takes minutes; a bar over its bytes would sit at a hundred per cent for the
/// whole wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Measure {
    /// Bytes.
    Bytes,
    /// Things: files, directories, links.
    Items,
}

/// What was asked for — the whole request, so the daemon needs nothing else.
///
/// Every variant carries its ROOTS as parameters, exactly as `ronoctua-files`
/// does: the daemon never reaches for a well-known location on its own, and a
/// test can hand it a fake tree instead of a booted machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "work")]
pub enum Work {
    /// Copy — or, when `moves`, move — these into that directory.
    Paste {
        /// What to carry.
        #[serde(with = "crate::wire::paths")]
        paths: Vec<PathBuf>,
        /// Where to.
        #[serde(with = "crate::wire::path")]
        into: PathBuf,
        /// `true` when the originals go away.
        moves: bool,
    },
    /// Remove these outright.
    Delete {
        /// What to remove.
        #[serde(with = "crate::wire::paths")]
        paths: Vec<PathBuf>,
    },
    /// Overwrite and then remove these.
    Shred {
        /// What to overwrite.
        #[serde(with = "crate::wire::paths")]
        paths: Vec<PathBuf>,
    },
    /// Move these to the trash whose root is given.
    Trash {
        /// The bin's root, so the daemon does not ask the environment a second
        /// time and possibly get a different answer.
        #[serde(with = "crate::wire::path")]
        root: PathBuf,
        /// What to put in it.
        #[serde(with = "crate::wire::paths")]
        paths: Vec<PathBuf>,
    },
    /// Put these back where they came from, by their name in the trash.
    Restore {
        /// The bin's root.
        #[serde(with = "crate::wire::path")]
        root: PathBuf,
        /// The names inside it.
        names: Vec<String>,
    },
    /// Pack these names, relative to that directory, into that archive.
    Compress {
        /// Where the names are.
        #[serde(with = "crate::wire::path")]
        directory: PathBuf,
        /// What to pack.
        names: Vec<String>,
        /// Where it lands.
        #[serde(with = "crate::wire::path")]
        archive: PathBuf,
    },
    /// Unpack that archive into that directory.
    Extract {
        /// What to unpack.
        #[serde(with = "crate::wire::path")]
        archive: PathBuf,
        /// Where to.
        #[serde(with = "crate::wire::path")]
        into: PathBuf,
    },
    /// Walk a directory and report how big it is.
    ///
    /// It is in the queue for the same reason as the rest: it is unbounded, and
    /// until now it was the one operation somebody watches the screen during
    /// with nothing to watch.
    Measure {
        /// What to weigh.
        #[serde(with = "crate::wire::path")]
        path: PathBuf,
    },

    // ---- AND THE ONES THAT COME OVER THE NETWORK ----
    //
    // They carry a `String` and not a `PathBuf`, and the difference is not
    // cosmetic: a URL is text with a grammar, and putting it through the
    // path-or-bytes encoding would let a byte sequence that is not a URL
    // through as if it were one.
    /// Download a plain link, through aria2.
    Fetch {
        /// Where to get it.
        url: String,
        /// Which directory it lands in.
        #[serde(with = "crate::wire::path")]
        into: PathBuf,
    },
    /// Download a site's media, through yt-dlp.
    Media {
        /// The PAGE's address, not the media's: yt-dlp is what resolves one
        /// into the other, and it is the only thing here that knows how.
        url: String,
        /// Which directory it lands in.
        #[serde(with = "crate::wire::path")]
        into: PathBuf,
        /// Which of the three the menu asked for.
        want: Want,
    },
    /// A torrent: a magnet URI, or the path of a `.torrent` as text.
    Torrent {
        /// The magnet, or the file.
        source: String,
        /// Which directory it lands in.
        #[serde(with = "crate::wire::path")]
        into: PathBuf,
    },
}

/// Which part of a page's media was asked for.
///
/// A field of [`Work::Media`] and not three variants, for the same reason
/// `moves` is a field of [`Work::Paste`]: it is one operation with a switch,
/// and three variants would be three copies of the same argument building.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Want {
    /// The video, at the best quality on offer.
    Video,
    /// The audio alone.
    Audio,
    /// The subtitles alone, with no media at all.
    Subtitles,
}

impl Work {
    /// The stem of its text key.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Paste { moves: false, .. } => "copy",
            Self::Paste { moves: true, .. } => "move",
            Self::Delete { .. } => "delete",
            Self::Shred { .. } => "shred",
            Self::Trash { .. } => "trash",
            Self::Restore { .. } => "restore",
            Self::Compress { .. } => "compress",
            Self::Extract { .. } => "extract",
            Self::Measure { .. } => "measure",
            Self::Fetch { .. } => "fetch",
            Self::Media {
                want: Want::Video, ..
            } => "media",
            Self::Media {
                want: Want::Audio, ..
            } => "audio",
            Self::Media {
                want: Want::Subtitles,
                ..
            } => "subtitles",
            Self::Torrent { .. } => "torrent",
        }
    }

    /// Every stem, for the check that demands a text per kind of work.
    pub const WORDS: [&'static str; 14] = [
        "copy",
        "move",
        "delete",
        "shred",
        "trash",
        "restore",
        "compress",
        "extract",
        "measure",
        "fetch",
        "media",
        "audio",
        "subtitles",
        "torrent",
    ];

    /// Who does it.
    ///
    /// It comes OUT of the work and is not asked for alongside it: with an
    /// engine field on the request, a caller could ask for a torrent to be
    /// pasted, and the daemon would have to decide which half of the request to
    /// believe. Here there is nothing to disagree with.
    #[must_use]
    pub const fn engine(&self) -> Engine {
        match self {
            Self::Fetch { .. } => Engine::Fetch,
            Self::Media { .. } => Engine::Media,
            Self::Torrent { .. } => Engine::Torrent,
            Self::Paste { .. }
            | Self::Delete { .. }
            | Self::Shred { .. }
            | Self::Trash { .. }
            | Self::Restore { .. }
            | Self::Compress { .. }
            | Self::Extract { .. }
            | Self::Measure { .. } => Engine::Local,
        }
    }

    /// What the counter will count.
    ///
    /// Bytes wherever bytes are what takes the time, entries wherever they are
    /// not. Compressing and extracting are neither: what decides them is the
    /// compressor, and the honest answer is that we do not know — see
    /// [`Job::total`].
    #[must_use]
    pub const fn measure(&self) -> Measure {
        match self {
            Self::Paste { .. }
            | Self::Shred { .. }
            | Self::Fetch { .. }
            | Self::Media { .. }
            | Self::Torrent { .. } => Measure::Bytes,
            Self::Measure { .. }
            | Self::Delete { .. }
            | Self::Trash { .. }
            | Self::Restore { .. }
            | Self::Compress { .. }
            | Self::Extract { .. } => Measure::Items,
        }
    }

    /// Where the work lands, when it lands anywhere.
    ///
    /// It is the second half of the row's sentence — *twelve files into
    /// `~/backup`* — and the reason a copy and a move do not need two rows to
    /// say where they went.
    ///
    /// `landing` and not `into`: `into` is `Into::into`, and Rust resolves the
    /// trait first. The call compiled into a conversion and the errors landed
    /// two files away, on the `Path` methods that were being called on a
    /// `Work`.
    #[must_use]
    pub fn landing(&self) -> Option<&std::path::Path> {
        match self {
            Self::Paste { into, .. }
            | Self::Extract { into, .. }
            | Self::Fetch { into, .. }
            | Self::Media { into, .. }
            | Self::Torrent { into, .. } => Some(into),
            Self::Trash { root, .. } | Self::Restore { root, .. } => Some(root),
            Self::Compress { archive, .. } => Some(archive),
            Self::Delete { .. } | Self::Shred { .. } | Self::Measure { .. } => None,
        }
    }

    /// What it acts on, for the row's first half.
    #[must_use]
    pub fn subjects(&self) -> Paths<'_> {
        match self {
            Self::Paste { paths, .. } | Self::Delete { paths } | Self::Shred { paths } => {
                Paths::Some(paths)
            }
            Self::Trash { paths, .. } => Paths::Some(paths),
            Self::Restore { names, .. } | Self::Compress { names, .. } => Paths::Named(names),
            Self::Extract { archive, .. } => Paths::One(archive),
            Self::Measure { path } => Paths::One(path),
            Self::Fetch { url, .. } | Self::Media { url, .. } => Paths::Text(url),
            Self::Torrent { source, .. } => Paths::Text(source),
        }
    }

    /// How many things it acts on. `1` for the ones that act on a single path.
    #[must_use]
    pub fn how_many(&self) -> usize {
        match self.subjects() {
            Paths::Some(p) => p.len(),
            Paths::Named(n) => n.len(),
            Paths::One(_) | Paths::Text(_) => 1,
        }
    }
}

/// One piece of long work, as everybody sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Job {
    /// Its number, unique for the life of the daemon. It is what `pause 7`
    /// names.
    pub id: u64,
    /// Who is doing it.
    pub engine: Engine,
    /// What was asked for.
    pub work: Work,
    /// Where it is.
    pub stage: Stage,
    /// What the two numbers below count.
    pub measure: Measure,
    /// How much is behind it.
    pub done: u64,
    /// How much there is in total. **Zero means not known** — either it is
    /// still weighing, or the operation is one whose end nobody can predict
    /// (a compressor's output, a stream with no length). A bar over an unknown
    /// total draws no bar and says the stage instead, which is the truth.
    pub total: u64,
    /// The name it is on right now. Data, not a text key: it is somebody's
    /// filename and it does not get translated.
    pub now: String,
    /// What went wrong, in the daemon's own words. `None` while it has not.
    pub trouble: Option<String>,
    /// The number the work PRODUCED, for the one kind that produces one.
    ///
    /// [`Work::Measure`] exists to answer "how big is this folder", and that
    /// answer is not progress: the progress is entries walked, and the size is
    /// what comes out at the end. Writing it into `total` would make a bar out
    /// of a result — a folder that turned out to be 40 GiB would show up as a
    /// job that is 0% of the way through 40 GiB, which is not a thing that was
    /// ever happening.
    pub answer: Option<u64>,
}

impl Job {
    /// A job as it is born: weighed by nobody, moved by nobody.
    #[must_use]
    pub fn fresh_one(id: u64, engine: Engine, work: Work) -> Self {
        Self {
            id,
            engine,
            measure: work.measure(),
            work,
            stage: Stage::Weighing,
            done: 0,
            total: 0,
            now: String::new(),
            trouble: None,
            answer: None,
        }
    }

    /// How far along, in PER MILLE — `12.3%` is `123`.
    ///
    /// Integer, and per mille rather than per cent, for the reason
    /// `ronoctua_protocol::Pressure` already gives: a rounding that happens in
    /// one place is a rounding that can be pointed at, and per mille is one
    /// digit more than the screen shows, which is what keeps `99%` from
    /// appearing while a gibibyte is still moving.
    ///
    /// `None` when the total is not known — see [`Job::total`].
    #[must_use]
    pub const fn along(&self) -> Option<u16> {
        if self.total == 0 {
            return None;
        }
        // Done can pass the total: a file that grew between the weighing and
        // the copy is not a bug in either. Clamping is what keeps `104%` off
        // the screen.
        let per = self.done.saturating_mul(1000) / self.total;
        Some(if per > 1000 { 1000 } else { per as u16 })
    }

    /// Is it still happening?
    #[must_use]
    pub const fn afoot(&self) -> bool {
        self.stage.afoot()
    }
}
