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
    /// A region's streets: a download, and then a baking.
    ///
    /// ONE ENGINE AND NOT TWO JOBS, which is `Work::Media`'s shape a subject
    /// along — that one downloads and then remuxes. Two chained jobs would
    /// need a second scheduler, and the half-way state (a zip on disk that is
    /// nobody's) would be a thing this queue promises not to leave.
    Atlas,
    /// The person's own programs: the software center's CLI, as a child.
    ///
    /// The same shape as [`Engine::Media`] — a program in its own process
    /// group, read line by line — and for the same reason the queue does not
    /// do the work itself: that program links a TLS stack and image decoders,
    /// and this daemon runs for as long as the machine is on.
    Software,
    /// A file sent to VirusTotal: `ronoctua-scan`, as a child — the software
    /// engine's shape and reason (a TLS stack this daemon does not carry).
    Scan,
    /// The person's containers: podman pulling, building and running
    /// compose, as a child (`crate::container`).
    Container,
    /// The person's belongings: their CLI, as a child — for the software
    /// engine's reason and a sharper one: it links `age`, which this daemon
    /// must never carry.
    Belongings,
}

impl Engine {
    /// Every one of them, for the check that demands a text and an icon per
    /// engine. A `match` would compile with one missing; a list is what can be
    /// compared against the `.ftl`.
    pub const ALL: [Self; 9] = [
        Self::Local,
        Self::Fetch,
        Self::Media,
        Self::Torrent,
        Self::Atlas,
        Self::Software,
        Self::Scan,
        Self::Container,
        Self::Belongings,
    ];

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
            Self::Atlas => "atlas",
            Self::Software => "software",
            Self::Scan => "scan",
            Self::Container => "container",
            Self::Belongings => "belongings",
        }
    }

    /// Whether it honours PAUSE. Cancel is honoured by every engine.
    ///
    /// Data and not a second code path, which is what this module's header
    /// promises: the socket asks it before flipping a row to held, and a screen
    /// asks it before offering the key.
    ///
    /// The software engine does not pause, and it cannot be made to honestly.
    /// What it runs is `nix`, `flatpak` and `waydroid` in the middle of a
    /// transaction and under a LANE lock (`ronoctua_software::lock`), and a
    /// `SIGSTOP` would freeze them holding it — so the update run, which takes
    /// the same lane, would wait behind a row somebody forgot was held. A key
    /// that says no is better than a held row quietly blocking the machine.
    ///
    /// Nor does the belongings': a sync held between its commit and its push
    /// would hold the passphrase in the runtime directory for as long as
    /// somebody forgot the row.
    #[must_use]
    ///
    /// Nor does the container engine, for the same reason in podman's words:
    /// a stopped pull holds the person's container storage locked, and every
    /// other podman — the app's own listing — would wait behind it.
    pub const fn pauses(self) -> bool {
        // AN UPLOAD HELD is a socket the host times out, and the file would
        // have to go again from its first byte: the same "no" as the software
        // engine's, for a different reason — and a container's job is podman's
        // own child, which a pause would leave half done, as the belongings'
        // CLI is.
        !matches!(
            self,
            Self::Software | Self::Scan | Self::Container | Self::Belongings
        )
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
    /// Pack these names, relative to that directory, into that archive —
    /// through `ronoctua-archive`, which runs the program.
    ///
    /// THE SHEET'S FOUR ANSWERS travel with it: the format, the level, the
    /// size of a volume and the password's FILE (never the password: see
    /// [`crate::secrets`]). Each has a default, so a request from before they
    /// existed still parses — as the format its name says, at the normal
    /// level, whole and in clear.
    Compress {
        /// Where the names are.
        #[serde(with = "crate::wire::path")]
        directory: PathBuf,
        /// What to pack.
        names: Vec<String>,
        /// Where it lands.
        #[serde(with = "crate::wire::path")]
        archive: PathBuf,
        /// The format; `None` is the one the archive's name says.
        #[serde(default)]
        format: Option<ronoctua_archive::Format>,
        /// How hard to compress.
        #[serde(default)]
        level: ronoctua_archive::Level,
        /// Split into volumes of this many bytes (zip and 7z).
        #[serde(default)]
        volume_bytes: Option<u64>,
        /// The names too, behind the password (7z).
        #[serde(default)]
        hide_names: bool,
        /// The password's `0600` file, which the job removes when it ends.
        #[serde(default, with = "crate::wire::maybe_path")]
        secret: Option<PathBuf>,
    },
    /// Unpack that archive — or only these members of it — into that
    /// directory. Nothing there is replaced: what comes out takes a free name.
    Extract {
        /// What to unpack.
        #[serde(with = "crate::wire::path")]
        archive: PathBuf,
        /// Where to.
        #[serde(with = "crate::wire::path")]
        into: PathBuf,
        /// The members, by their path inside; empty is every one.
        #[serde(default)]
        members: Vec<String>,
        /// The password's `0600` file, which the job removes when it ends.
        #[serde(default, with = "crate::wire::maybe_path")]
        secret: Option<PathBuf>,
    },
    /// Add these files to that archive, or take these members out of it — in
    /// place, zip and 7z only (a tar is converted instead).
    ArchiveEdit {
        /// Which archive.
        #[serde(with = "crate::wire::path")]
        archive: PathBuf,
        /// What goes in, all from one folder.
        #[serde(with = "crate::wire::paths")]
        add: Vec<PathBuf>,
        /// What comes out, by path inside.
        remove: Vec<String>,
        /// The password's `0600` file: what goes in is encrypted with it,
        /// and hidden names need it to be read at all.
        #[serde(default, with = "crate::wire::maybe_path")]
        secret: Option<PathBuf>,
    },
    /// Read every member of that archive and check it against its sums.
    ArchiveTest {
        /// Which archive.
        #[serde(with = "crate::wire::path")]
        archive: PathBuf,
        /// The password's `0600` file.
        #[serde(default, with = "crate::wire::maybe_path")]
        secret: Option<PathBuf>,
    },
    /// Write that archive again, in another format, beside it.
    ArchiveConvert {
        /// The archive as it is.
        #[serde(with = "crate::wire::path")]
        from: PathBuf,
        /// Its password's `0600` file, when it has one.
        #[serde(default, with = "crate::wire::maybe_path")]
        from_secret: Option<PathBuf>,
        /// The archive to make.
        #[serde(with = "crate::wire::path")]
        to: PathBuf,
        /// Its format; `None` is the one its name says.
        #[serde(default)]
        format: Option<ronoctua_archive::Format>,
        /// How hard to compress.
        #[serde(default)]
        level: ronoctua_archive::Level,
        /// Split into volumes of this many bytes (zip and 7z).
        #[serde(default)]
        volume_bytes: Option<u64>,
        /// The names too, behind the password (7z).
        #[serde(default)]
        hide_names: bool,
        /// The NEW archive's password's `0600` file.
        #[serde(default, with = "crate::wire::maybe_path")]
        secret: Option<PathBuf>,
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
        /// Whether it goes out as the person or as nobody.
        #[serde(default)]
        identity: Identity,
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
        /// Whether this one goes out as the person or as nobody.
        ///
        /// A field of the REQUEST and never a setting: what it costs is
        /// anonymity, and a switch somewhere else would make that cost
        /// invisible at the moment it is paid. `Default` so an older client's
        /// message still parses — as anonymous, which is the safe reading.
        #[serde(default)]
        identity: Identity,
    },
    /// A region's streets, from the catalogue baked into the world.
    ///
    /// The URL is the catalogue's, which is a `-latest-` one and a REDIRECT:
    /// what it points at carries a date, and that date is the pack's version.
    /// Following it is why this goes through the queue's engine and not
    /// through `ronoctua-http`, which says in its own header that it does not
    /// follow redirects.
    Atlas {
        /// Geofabrik's id — `portugal`, `brazil/sudeste`.
        region: String,
        /// What the catalogue calls it, carried so the pack can name itself
        /// without the daemon holding a copy of the catalogue.
        label: String,
        /// Where to get it.
        url: String,
        /// The atlas root — `$XDG_DATA_HOME/ronoctua/atlas`. An argument for
        /// the reason `Work::Trash` carries its root: the daemon asking the
        /// environment a second time could get a different answer.
        #[serde(with = "crate::wire::path")]
        into: PathBuf,
    },
    /// A torrent: a magnet URI, or the path of a `.torrent` as text.
    Torrent {
        /// The magnet, or the file.
        source: String,
        /// Which directory it lands in.
        #[serde(with = "crate::wire::path")]
        into: PathBuf,
    },
    /// Something asked of the person's programs: install, remove, upgrade,
    /// try, a web app — the software center's ONE order type, carried whole.
    ///
    /// The order is the crate's own enum and not a copy of it: the screen that
    /// builds it, this queue that carries it and the CLI that runs it are then
    /// one type, and a variant nobody handles does not compile.
    Software {
        /// What to do, as the CLI receives it on its standard input.
        ///
        /// BOXED: an order carries a web app's whole spec, and inline it made
        /// every `Work` — and every `Ask` and `Job` around one — four times the
        /// size of the largest other variant. The wire does not see the box.
        order: Box<ronoctua_software::Order>,
        /// The name to show — `GIMP`, not `org.gimp.GIMP`. Data, not a key:
        /// it is a program's name and it does not get translated.
        label: String,
    },
    /// One file sent to VirusTotal, after the person answered yes to the
    /// question that says what that shares — the security centre's.
    Scan {
        /// The file.
        #[serde(with = "crate::wire::path")]
        path: PathBuf,
        /// Its SHA-256 when the question was asked: a file that changed since
        /// is not sent.
        sha256: String,
    },
    /// Something asked of the person's containers: pull, build, compose —
    /// `ronoctua-containers`' own job type, carried whole, for the reason
    /// `Software` carries its order.
    Container {
        /// What to do.
        job: ronoctua_containers::Job,
        /// The name to show — the image, the folder, the project. Data.
        label: String,
    },
    /// Something asked of the person's belongings — a sync, its apply or its
    /// abandon, a deploy key, a new passphrase. The ORDER only: the passphrase
    /// is a `0600` runtime file a door wrote, and the plan and the decisions
    /// are files beside it (`ronoctua_belongings::places`), so nothing secret
    /// crosses this socket or sits in the ledger.
    Belongings {
        /// What to do.
        op: ronoctua_belongings::order::Op,
    },
}

/// Whose session a download is made with.
///
/// # Why this is not a preference
///
/// Because the whole of what it changes is whether the request carries who you
/// are. Age-restricted video, private video, a members' club and 1080p Premium
/// all need it; everything else is better without it. A setting kept somewhere
/// else would mean pressing a key and not knowing which of the two just
/// happened — and the one that cannot be taken back is the identified one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Identity {
    /// Nobody. No cookies, no account, no token.
    #[default]
    Anonymous,
    /// The person: their cookies, and the device proof if there is one.
    Mine,
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
            Self::ArchiveEdit { add, .. } if !add.is_empty() => "archive-add",
            Self::ArchiveEdit { .. } => "archive-remove",
            Self::ArchiveTest { .. } => "archive-test",
            Self::ArchiveConvert { .. } => "archive-convert",
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
            Self::Atlas { .. } => "region",
            Self::Software { .. } => "software",
            Self::Scan { .. } => "scan",
            Self::Container { job, .. } => match job {
                ronoctua_containers::Job::Pull { .. } => "container-pull",
                ronoctua_containers::Job::Build { .. } => "container-build",
                ronoctua_containers::Job::ComposeUp { .. } => "container-compose-up",
                ronoctua_containers::Job::ComposeDown { .. } => "container-compose-down",
                ronoctua_containers::Job::ComposePull { .. } => "container-compose-pull",
            },
            Self::Belongings { .. } => "belongings",
        }
    }

    /// Every stem, for the check that demands a text per kind of work.
    pub const WORDS: [&'static str; 27] = [
        "copy",
        "move",
        "delete",
        "shred",
        "trash",
        "restore",
        "compress",
        "extract",
        "archive-add",
        "archive-remove",
        "archive-test",
        "archive-convert",
        "measure",
        "fetch",
        "media",
        "audio",
        "subtitles",
        "torrent",
        "region",
        "software",
        "scan",
        "container-pull",
        "container-build",
        "container-compose-up",
        "container-compose-down",
        "container-compose-pull",
        "belongings",
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
            Self::Atlas { .. } => Engine::Atlas,
            Self::Software { .. } => Engine::Software,
            Self::Scan { .. } => Engine::Scan,
            Self::Container { .. } => Engine::Container,
            Self::Belongings { .. } => Engine::Belongings,
            Self::Paste { .. }
            | Self::Delete { .. }
            | Self::Shred { .. }
            | Self::Trash { .. }
            | Self::Restore { .. }
            | Self::Compress { .. }
            | Self::Extract { .. }
            | Self::ArchiveEdit { .. }
            | Self::ArchiveTest { .. }
            | Self::ArchiveConvert { .. }
            | Self::Measure { .. } => Engine::Local,
        }
    }

    /// The password files this work carries — what the job removes when it
    /// ends, however it ends ([`crate::secrets::forget`]).
    #[must_use]
    pub fn secrets(&self) -> Vec<&std::path::Path> {
        match self {
            Self::Compress { secret, .. }
            | Self::Extract { secret, .. }
            | Self::ArchiveEdit { secret, .. }
            | Self::ArchiveTest { secret, .. } => secret.iter().map(PathBuf::as_path).collect(),
            Self::ArchiveConvert {
                from_secret,
                secret,
                ..
            } => from_secret
                .iter()
                .chain(secret.iter())
                .map(PathBuf::as_path)
                .collect(),
            _ => Vec::new(),
        }
    }

    /// What the counter will count.
    ///
    /// Bytes wherever bytes are what takes the time, entries wherever they are
    /// not. An archive's work is BYTES — what is read, the sources or the
    /// archive — and its total is only written once the program says how far
    /// it is (7zz does, bsdtar does not): until then the total is unknown and
    /// the row says the stage, never a bar that would be a guess. See
    /// [`Job::total`].
    #[must_use]
    pub const fn measure(&self) -> Measure {
        match self {
            Self::Paste { .. }
            | Self::Shred { .. }
            | Self::Fetch { .. }
            | Self::Media { .. }
            | Self::Torrent { .. }
            | Self::Atlas { .. }
            | Self::Compress { .. }
            | Self::Extract { .. }
            | Self::ArchiveEdit { .. }
            | Self::ArchiveTest { .. }
            | Self::ArchiveConvert { .. }
            | Self::Scan { .. } => Measure::Bytes,
            Self::Measure { .. }
            | Self::Delete { .. }
            | Self::Trash { .. }
            | Self::Restore { .. } => Measure::Items,
            // WHERE IT STARTS, and not where it stays: the measure of a
            // software job follows its PHASE (`software::measure_of`) — bytes
            // while something downloads, things otherwise.
            Self::Software { .. } => Measure::Items,
            // A build's steps, a pull's layers: things, never bytes — podman
            // prints no byte count on a line a pipe can read.
            Self::Container { .. } => Measure::Items,
            // The CLI counts files: read, encrypted, written.
            Self::Belongings { .. } => Measure::Items,
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
            | Self::Torrent { into, .. }
            | Self::Atlas { into, .. } => Some(into),
            Self::Trash { root, .. } | Self::Restore { root, .. } => Some(root),
            Self::Compress { archive, .. } | Self::ArchiveEdit { archive, .. } => Some(archive),
            Self::ArchiveConvert { to, .. } => Some(to),
            // A program lands in the person's profile, in Flatpak's tree, in
            // `~/Applications` or inside Android — nowhere one path says.
            Self::Delete { .. }
            | Self::Shred { .. }
            | Self::Measure { .. }
            | Self::ArchiveTest { .. }
            | Self::Software { .. }
            | Self::Scan { .. }
            | Self::Container { .. }
            | Self::Belongings { .. } => None,
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
            Self::ArchiveEdit { add, remove, .. } => {
                if add.is_empty() {
                    Paths::Named(remove)
                } else {
                    Paths::Some(add)
                }
            }
            Self::Extract { archive, .. } | Self::ArchiveTest { archive, .. } => {
                Paths::One(archive)
            }
            Self::ArchiveConvert { from, .. } => Paths::One(from),
            Self::Measure { path } | Self::Scan { path, .. } => Paths::One(path),
            Self::Fetch { url, .. } | Self::Media { url, .. } => Paths::Text(url),
            Self::Torrent { source, .. } => Paths::Text(source),
            // THE LABEL AND NOT THE URL: the row says what a person chose, and
            // what they chose is a place with a name. The URL is in the
            // catalogue and nobody typed it.
            Self::Atlas { label, .. } => Paths::Text(label),
            // The same rule: the program's name, not the attribute or the id.
            Self::Software { label, .. } | Self::Container { label, .. } => Paths::Text(label),
            // THE ORDER'S WORD: what is synced is the person's configuration,
            // and the row names what it is doing to it.
            Self::Belongings { op } => Paths::Text(op.word()),
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
    /// What a software job is doing right now — downloading, building,
    /// installing — for the row's sentence (`software-phase-<word>`).
    ///
    /// A field of the JOB and not a new [`Stage`]: the stage is what every
    /// engine shares and what the switches act on, and a phase is one engine's
    /// account of where inside `Running` it is. `None` for every other engine,
    /// and cleared when the job ends. `default` so a message from before it
    /// existed still parses.
    #[serde(default)]
    pub phase: Option<ronoctua_software::progress::Phase>,
    /// What a finished software order left: the program, the launcher entries
    /// it made (so "open" can be offered), how much a tidy freed.
    #[serde(default)]
    pub outcome: Option<ronoctua_software::Outcome>,
    /// Why a software order was not done, as the closed set the screen has a
    /// SENTENCE for. [`Job::trouble`] keeps the program's own words beside it.
    #[serde(default)]
    pub refusal: Option<ronoctua_software::Refusal>,
    /// Why an archive's work was not done, as the closed set the screen has a
    /// sentence for (`files-archive-why-<id>`) — and the one that decides
    /// whether a door asks for a password and tries again. [`Job::trouble`]
    /// stays empty beside it: the archive crate answers with this word and
    /// not with the program's prose, and a word in `trouble` would be drawn
    /// as it is.
    #[serde(default)]
    pub archive: Option<ronoctua_archive::Why>,
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
            phase: None,
            outcome: None,
            refusal: None,
            archive: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_software::AppKey;
    use ronoctua_software::order::{Consent, Order};

    fn software() -> Work {
        Work::Software {
            order: Box::new(Order::Install {
                key: AppKey::Flatpak {
                    id: "org.gnome.Calculator".into(),
                },
                consent: Consent::default(),
            }),
            label: "Calculator".into(),
        }
    }

    #[test]
    fn a_software_order_crosses_the_wire_whole() {
        // The order travels INSIDE the work, with its own tag beside the
        // work's: the two are different fields and neither may eat the other.
        let work = software();
        let text = serde_json::to_string(&work).unwrap();
        assert!(
            text.starts_with(r#"{"work":"software","order":{"order":"install""#),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Work>(&text).unwrap(), work);
        assert_eq!(work.word(), "software");
        assert!(Work::WORDS.contains(&work.word()));
        assert_eq!(work.engine(), Engine::Software);
        // The row names the PROGRAM, not its id.
        assert_eq!(work.subjects().first(), "Calculator");
        assert_eq!(work.landing(), None);
    }

    #[test]
    fn an_archive_s_work_crosses_the_wire_with_its_sheet_and_no_password() {
        let work = Work::Compress {
            directory: PathBuf::from("/home/a"),
            names: vec!["one.txt".into(), "two".into()],
            archive: PathBuf::from("/home/a/one.7z"),
            format: Some(ronoctua_archive::Format::SevenZ),
            level: ronoctua_archive::Level::Ultra,
            volume_bytes: Some(1_000_000),
            hide_names: true,
            secret: Some(PathBuf::from(
                "/run/user/1000/ronoctua-transfers/secrets/job-1",
            )),
        };
        let text = serde_json::to_string(&work).unwrap();
        assert!(
            text.contains(r#""format":"7z""#) && text.contains(r#""level":"ultra""#),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Work>(&text).unwrap(), work);
        assert_eq!(work.secrets().len(), 1);
        assert_eq!(work.engine(), Engine::Local);
        assert_eq!(work.measure(), Measure::Bytes);
        // A REQUEST FROM BEFORE THE SHEET still parses, as the format its name
        // says, at the normal level, whole and in clear.
        let old = r#"{"work":"compress","directory":"/a","names":["x"],"archive":"/a/x.zip"}"#;
        let Work::Compress {
            format,
            level,
            volume_bytes,
            hide_names,
            secret,
            ..
        } = serde_json::from_str::<Work>(old).unwrap()
        else {
            panic!("not a compress");
        };
        assert_eq!(
            (format, level, volume_bytes, hide_names, secret),
            (None, ronoctua_archive::Level::Normal, None, false, None)
        );
        let old = r#"{"work":"extract","archive":"/a/x.zip","into":"/a"}"#;
        assert!(serde_json::from_str::<Work>(old).is_ok());
        // EVERY WORD HAS A PLACE IN THE CENSUS the texts are checked against.
        for w in [
            work.word(),
            Work::ArchiveTest {
                archive: PathBuf::from("/a.zip"),
                secret: None,
            }
            .word(),
            Work::ArchiveEdit {
                archive: PathBuf::from("/a.zip"),
                add: vec![PathBuf::from("/b")],
                remove: vec![],
                secret: None,
            }
            .word(),
            Work::ArchiveEdit {
                archive: PathBuf::from("/a.zip"),
                add: vec![],
                remove: vec!["b".into()],
                secret: None,
            }
            .word(),
        ] {
            assert!(Work::WORDS.contains(&w), "{w}");
        }
        let convert = Work::ArchiveConvert {
            from: PathBuf::from("/a.tar.gz"),
            from_secret: Some(PathBuf::from("/s/job-1")),
            to: PathBuf::from("/a.7z"),
            format: None,
            level: ronoctua_archive::Level::Normal,
            volume_bytes: None,
            hide_names: false,
            secret: Some(PathBuf::from("/s/job-2")),
        };
        assert_eq!(convert.secrets().len(), 2, "both passwords go with the job");
        assert_eq!(convert.landing(), Some(std::path::Path::new("/a.7z")));
    }

    #[test]
    fn only_the_child_engines_refuse_a_pause() {
        for engine in Engine::ALL {
            assert_eq!(
                engine.pauses(),
                !matches!(
                    engine,
                    Engine::Software | Engine::Scan | Engine::Container | Engine::Belongings
                ),
                "{engine:?}"
            );
        }
    }

    /// A CONTAINER'S JOB crosses the socket whole, names what the person
    /// chose, and has a word per kind — the five texts the rows need.
    #[test]
    fn a_container_job_crosses_the_socket_and_says_its_kind() {
        let work = Work::Container {
            job: ronoctua_containers::Job::Pull {
                image: "docker.io/library/nginx:latest".into(),
            },
            label: "nginx".into(),
        };
        let text = serde_json::to_string(&work).unwrap();
        assert!(
            text.starts_with(r#"{"work":"container","job":{"do":"pull""#),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Work>(&text).unwrap(), work);
        assert_eq!(work.word(), "container-pull");
        assert_eq!(work.engine(), Engine::Container);
        assert_eq!(work.subjects().first(), "nginx");
        assert_eq!(work.landing(), None);
        for word in ronoctua_containers::Job::WORDS {
            assert!(
                Work::WORDS.contains(&format!("container-{word}").as_str()),
                "no stem for {word}"
            );
        }
    }

    #[test]
    fn a_belongings_order_crosses_the_wire_with_no_secret() {
        let work = Work::Belongings {
            op: ronoctua_belongings::order::Op::Prepare,
        };
        let text = serde_json::to_string(&work).unwrap();
        assert_eq!(text, r#"{"work":"belongings","op":"prepare"}"#);
        assert_eq!(serde_json::from_str::<Work>(&text).unwrap(), work);
        assert_eq!(work.engine(), Engine::Belongings);
        assert!(Work::WORDS.contains(&work.word()));
        assert_eq!(work.subjects().first(), "prepare");
    }

    #[test]
    fn a_row_from_before_the_software_fields_still_parses() {
        // A client and a daemon of two builds meet every time the queue
        // outlives a rebuild; the new fields default rather than refuse.
        let old = r#"{"id":3,"engine":"local","work":{"work":"measure","path":"/x"},
            "stage":"done","measure":"items","done":1,"total":1,"now":"",
            "trouble":null,"answer":7}"#;
        let job: Job = serde_json::from_str(old).unwrap();
        assert_eq!((job.phase, job.outcome, job.refusal), (None, None, None));
        assert_eq!(job.answer, Some(7));
    }
}
