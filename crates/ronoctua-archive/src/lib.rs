//! A person's archives — the one crate that runs `7zz`, `bsdtar` and `unrar`.
//!
//! # Who does what
//!
//! | Operation | Program |
//! |---|---|
//! | create zip / 7z (levels, AES-256, hidden names in 7z, split volumes) | 7zz |
//! | create tar.{gz,xz,zst,bz2,lz4} | bsdtar |
//! | list / extract / test the tar family | bsdtar |
//! | list / extract / test the rest | 7zz |
//! | RAR | unrar (the free 7zz has no RAR codec) |
//! | add / remove members | 7zz, zip and 7z only (a tar is converted) |
//! | convert | extract to a temporary folder, then create |
//!
//! Each program by its store path ([`Tools`]); a missing one is a refusal
//! naming it, never a bare name on `PATH`.
//!
//! # A password never reaches argv
//!
//! MEASURED with `7zz` 25.01 and `unrar` 7.2.3 first, and held to the pinned
//! 26.02 and 7.2.6 by the tests at the end, which run them: with no terminal
//! to ask on, both read the password from
//! stdin. So the child starts under `setsid` (`RONOCTUA_SETSID`: no
//! controlling terminal, so no `/dev/tty` to open) and this crate writes the
//! password to its stdin, read from a `0600` file whose PATH is all it is
//! handed (`secret`). On the transfer queue that file is the job's own,
//! deleted with the job.
//!
//! Two things the measurement taught, and the code follows:
//!
//! - `-p` with no value asks only when CREATING (and adding). On `x`, `t`
//!   and `l` it means an EMPTY password; there the switch is left out and
//!   7zz asks by itself when it meets an encrypted member or header.
//! - Without a terminal 7zz asks once, with no "verify" question; the
//!   password is written on more lines than any question needs, and the
//!   ones nobody reads are dropped with the pipe.
//!
//! A wrong password is refused ([`Why::WrongPassword`]); an encrypted archive
//! opened without one is [`Why::NeedsPassword`] — stdin is empty, the
//! question meets its end, and the program stops.
//!
//! # Nothing is overwritten
//!
//! Extraction writes into a fresh hidden folder inside the target and moves
//! what came out into place afterwards, taking a free name beside anything
//! already there. A wrong password leaves nothing behind (7zz would have
//! left an empty file per member), and neither does a damaged archive.
//! Creation refuses a target that exists ([`Why::Exists`]) and removes what
//! it wrote when it fails.
//!
//! # Blocks, and is held or stopped from outside
//!
//! Every operation runs a program to its end: on the queue, or a thread. A
//! caller that must be able to hold or give up a run hands a [`Switch`]
//! ([`Tools::switch`]); while the program runs it is asked every tenth of a
//! second, and a hold is `SIGSTOP` to the program's process group (the one
//! `setsid` made), a resume `SIGCONT`, a stop `SIGKILL` — and the run ends
//! [`Why::Stopped`], having removed what it wrote like any other refusal.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};

mod parse;
mod run;

use run::{Call, Password};

/// The variable naming 7zz's store path.
pub const SEVENZZ_VARIABLE: &str = "RONOCTUA_7ZZ";
/// The variable naming bsdtar's store path.
pub const BSDTAR_VARIABLE: &str = "RONOCTUA_BSDTAR";
/// The variable naming unrar's store path.
pub const UNRAR_VARIABLE: &str = "RONOCTUA_UNRAR";
/// The variable naming setsid's store path (util-linux).
pub const SETSID_VARIABLE: &str = "RONOCTUA_SETSID";
/// An archive's format.
///
/// On the transfer queue's wire it is its [`Format::id`], the same word its
/// text key ends in — one name, never two spellings of one format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Format {
    /// `.zip` (and the formats that are zips: `.jar`, `.epub`… read as zip).
    #[serde(rename = "zip")]
    Zip,
    /// `.7z`.
    #[serde(rename = "7z")]
    SevenZ,
    /// A plain `.tar`.
    #[serde(rename = "tar")]
    Tar,
    /// `.tar.gz`, `.tgz`.
    #[serde(rename = "tar-gz")]
    TarGz,
    /// `.tar.xz`, `.txz`.
    #[serde(rename = "tar-xz")]
    TarXz,
    /// `.tar.zst`, `.tzst`.
    #[serde(rename = "tar-zst")]
    TarZst,
    /// `.tar.bz2`, `.tbz2`.
    #[serde(rename = "tar-bz2")]
    TarBz2,
    /// `.tar.lz4`.
    #[serde(rename = "tar-lz4")]
    TarLz4,
    /// `.rar`.
    #[serde(rename = "rar")]
    Rar,
    /// `.iso`.
    #[serde(rename = "iso")]
    Iso,
    /// Something else 7zz may still read (`.cab`, `.dmg`, `.deb`…).
    #[serde(rename = "other")]
    Other,
}

/// The program an operation runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    /// 7zz.
    SevenZz,
    /// bsdtar.
    Bsdtar,
    /// unrar.
    Unrar,
}

impl Format {
    /// Every format, in the "Compress…" sheet's order (the ones it can make
    /// first).
    pub const ALL: [Self; 11] = [
        Self::Zip,
        Self::SevenZ,
        Self::TarZst,
        Self::TarXz,
        Self::TarGz,
        Self::TarBz2,
        Self::TarLz4,
        Self::Tar,
        Self::Rar,
        Self::Iso,
        Self::Other,
    ];

    /// Its kebab-case name, the suffix of its key (`files-archive-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZ => "7z",
            Self::Tar => "tar",
            Self::TarGz => "tar-gz",
            Self::TarXz => "tar-xz",
            Self::TarZst => "tar-zst",
            Self::TarBz2 => "tar-bz2",
            Self::TarLz4 => "tar-lz4",
            Self::Rar => "rar",
            Self::Iso => "iso",
            Self::Other => "other",
        }
    }

    /// The format a file's name says. Pure: the name, not the bytes.
    #[must_use]
    pub fn of_path(path: &Path) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let ends = |tails: &[&str]| tails.iter().any(|t| name.ends_with(t));
        if ends(&[".tar.gz", ".tgz"]) {
            Self::TarGz
        } else if ends(&[".tar.xz", ".txz"]) {
            Self::TarXz
        } else if ends(&[".tar.zst", ".tzst"]) {
            Self::TarZst
        } else if ends(&[".tar.bz2", ".tbz2", ".tbz"]) {
            Self::TarBz2
        } else if ends(&[".tar.lz4"]) {
            Self::TarLz4
        } else if ends(&[".tar"]) {
            Self::Tar
        } else if ends(&[".zip"]) {
            Self::Zip
        } else if ends(&[".7z"]) {
            Self::SevenZ
        } else if ends(&[".rar"]) {
            Self::Rar
        } else if ends(&[".iso"]) {
            Self::Iso
        } else {
            Self::Other
        }
    }

    /// The extension a created archive gets.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZ => "7z",
            Self::Tar => "tar",
            Self::TarGz => "tar.gz",
            Self::TarXz => "tar.xz",
            Self::TarZst => "tar.zst",
            Self::TarBz2 => "tar.bz2",
            Self::TarLz4 => "tar.lz4",
            Self::Rar => "rar",
            Self::Iso => "iso",
            Self::Other => "",
        }
    }

    /// Whether this crate can create one.
    #[must_use]
    pub const fn creatable(self) -> bool {
        !matches!(self, Self::Rar | Self::Iso | Self::Other)
    }

    /// Whether members can be added and removed in place (else: convert).
    #[must_use]
    pub const fn editable(self) -> bool {
        matches!(self, Self::Zip | Self::SevenZ)
    }

    /// Whether it can carry a password.
    #[must_use]
    pub const fn encryptable(self) -> bool {
        matches!(self, Self::Zip | Self::SevenZ)
    }

    /// Whether it can hide the members' NAMES behind the password (7z only:
    /// a zip's central directory is always in clear).
    #[must_use]
    pub const fn hides_names(self) -> bool {
        matches!(self, Self::SevenZ)
    }

    /// Whether it can be split into volumes (7zz writes them; bsdtar does
    /// not).
    #[must_use]
    pub const fn splits(self) -> bool {
        matches!(self, Self::Zip | Self::SevenZ)
    }

    /// Whether this crate can READ it — list it, extract it, test it. Only
    /// [`Format::Other`] is not promised (7zz may still read it).
    #[must_use]
    pub const fn readable(self) -> bool {
        !matches!(self, Self::Other)
    }

    /// The formats this crate CAN make, in the sheet's order.
    #[must_use]
    pub fn creatable_ones() -> Vec<Self> {
        Self::ALL.into_iter().filter(|f| f.creatable()).collect()
    }

    /// The format of this id ([`Format::id`]); `None` for a word that names
    /// none — refused, never guessed.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }

    /// The program that reads it.
    #[must_use]
    pub const fn reader(self) -> Tool {
        match self {
            Self::Tar | Self::TarGz | Self::TarXz | Self::TarZst | Self::TarBz2 | Self::TarLz4 => {
                Tool::Bsdtar
            }
            Self::Rar => Tool::Unrar,
            Self::Zip | Self::SevenZ | Self::Iso | Self::Other => Tool::SevenZz,
        }
    }
}

/// How hard to compress. On the wire, its [`Level::id`].
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// No compression.
    Store,
    /// The fastest.
    Fastest,
    /// The program's default.
    #[default]
    Normal,
    /// Smaller, slower.
    Maximum,
    /// The smallest.
    Ultra,
}

impl Level {
    /// The five, in the ring's order.
    pub const ALL: [Self; 5] = [
        Self::Store,
        Self::Fastest,
        Self::Normal,
        Self::Maximum,
        Self::Ultra,
    ];

    /// 7zz's `-mx` number (0, 1, 5, 7, 9); bsdtar's filters map it.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Store => 0,
            Self::Fastest => 1,
            Self::Normal => 5,
            Self::Maximum => 7,
            Self::Ultra => 9,
        }
    }

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Store => "store",
            Self::Fastest => "fastest",
            Self::Normal => "normal",
            Self::Maximum => "maximum",
            Self::Ultra => "ultra",
        }
    }
}

/// How to create an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The format.
    pub format: Format,
    /// The level.
    pub level: Level,
    /// Split into volumes of this many bytes (7zz only).
    pub volume_bytes: Option<u64>,
    /// The `0600` file holding the password, when it is to be encrypted.
    pub secret: Option<PathBuf>,
    /// Encrypt the names too (7z only: `-mhe=on`).
    pub hide_names: bool,
}

/// One member of an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Its path inside the archive — data.
    pub path: String,
    /// Its size, in bytes, when the listing says.
    pub size_bytes: Option<u64>,
    /// Its packed size, in bytes, when the listing says.
    pub packed_bytes: Option<u64>,
    /// Its modification time, seconds since the epoch.
    pub modified_s: Option<i64>,
    /// Whether it is a folder.
    pub is_dir: bool,
    /// Whether it is encrypted.
    pub encrypted: bool,
}

/// Where an operation is, for the queue's row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Progress {
    /// How far, when the program says.
    pub percent: Option<u8>,
    /// The member being written — data.
    pub member: Option<String>,
}

impl Format {
    /// bsdtar's filter for a compressed tar: its switch and the name its
    /// `compression-level` option is given under. `None` for a plain tar and
    /// for what bsdtar does not write.
    #[must_use]
    pub const fn filter(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::TarGz => Some(("--gzip", "gzip")),
            Self::TarXz => Some(("--xz", "xz")),
            Self::TarZst => Some(("--zstd", "zstd")),
            Self::TarBz2 => Some(("--bzip2", "bzip2")),
            Self::TarLz4 => Some(("--lz4", "lz4")),
            _ => None,
        }
    }
}

impl Level {
    /// The `compression-level` a filter is given for this level; `None`
    /// leaves the filter's default. Each filter counts differently: gzip and
    /// xz 0–9 (0 stores), bzip2 1–9, zstd 1–22, lz4 1–9 (1 is its default and
    /// its fastest, 3 and up its high-compression mode). bzip2, zstd and lz4
    /// cannot store, so their store is their fastest: MEASURED (3.8.8), lz4
    /// answers 0 with "compression-level invalid". The lz4 levels need the
    /// bsdtar of `nix/archive-tools.nix`, with liblz4 linked in. Pure.
    #[must_use]
    pub const fn for_filter(self, filter: &str) -> Option<u8> {
        let table: [u8; 5] = match filter.as_bytes() {
            b"gzip" => [0, 1, 6, 8, 9],
            b"xz" => [0, 1, 6, 7, 9],
            b"bzip2" => [1, 1, 9, 9, 9],
            b"zstd" => [1, 1, 3, 12, 19],
            b"lz4" => [1, 1, 1, 6, 9],
            _ => return None,
        };
        Some(table[self as usize])
    }
}

/// Why an operation did not happen, as a closed set (`files-archive-why-<id>`).
///
/// On the wire, its [`Why::id`]: the transfer queue carries a refusal whole,
/// and the screen turns it into the sentence of that key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// This process was not given the program the operation needs (or the
    /// `setsid` it must run under).
    NoTool,
    /// The format cannot do this (a tar has no in-place add or remove, a
    /// tar no password, a zip no hidden names), or a password the programs
    /// cannot be given on one line.
    Unsupported,
    /// The archive is encrypted and no password was given.
    NeedsPassword,
    /// The password is wrong.
    WrongPassword,
    /// The archive is damaged (or is not an archive).
    Corrupt,
    /// The target already exists.
    Exists,
    /// The program could not be started.
    Unstartable,
    /// The program ended with a failure.
    Failed,
    /// The caller's [`Switch`] said stop, and the program was killed — what
    /// it had written is removed, as for any other refusal.
    Stopped,
}

impl Why {
    /// Every refusal, for the keys' census.
    pub const ALL: [Self; 9] = [
        Self::NoTool,
        Self::Unsupported,
        Self::NeedsPassword,
        Self::WrongPassword,
        Self::Corrupt,
        Self::Exists,
        Self::Unstartable,
        Self::Failed,
        Self::Stopped,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoTool => "no-tool",
            Self::Unsupported => "unsupported",
            Self::NeedsPassword => "needs-password",
            Self::WrongPassword => "wrong-password",
            Self::Corrupt => "corrupt",
            Self::Exists => "exists",
            Self::Unstartable => "unstartable",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }

    /// Whether a password would change the answer — the refusal the screen
    /// answers with a question and not with a sentence.
    #[must_use]
    pub const fn wants_a_password(self) -> bool {
        matches!(self, Self::NeedsPassword | Self::WrongPassword)
    }
}

/// What a running program is told, when its [`Switch`] is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// Go on (or go on again, after a pause).
    Go,
    /// Hold where it is: `SIGSTOP` to its process group.
    Pause,
    /// Give it up: `SIGKILL`, and the run ends [`Why::Stopped`].
    Stop,
}

/// The caller's hand on a run, asked every tenth of a second while a program
/// runs (see `run::watch`). A function and not a flag, so the transfer queue
/// hands its own switch in without this crate knowing its type.
#[derive(Clone)]
pub struct Switch(std::sync::Arc<dyn Fn() -> Hold + Send + Sync>);

impl Switch {
    /// A switch that answers what `now` says.
    #[must_use]
    pub fn new(now: impl Fn() -> Hold + Send + Sync + 'static) -> Self {
        Self(std::sync::Arc::new(now))
    }

    /// What it says right now.
    #[must_use]
    pub fn now(&self) -> Hold {
        (self.0)()
    }
}

impl std::fmt::Debug for Switch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Switch({:?})", self.now())
    }
}

/// Two switches are the same when they are the SAME hand, not when they
/// happen to say the same thing.
impl PartialEq for Switch {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Switch {}

/// A refusal, and the program it concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Refusal {
    /// Why.
    pub why: Why,
    /// The program, when one was (or would have been) run.
    pub tool: Option<Tool>,
}

const fn refused(why: Why, tool: Option<Tool>) -> Refusal {
    Refusal { why, tool }
}

/// A program's path: this process's environment first (a test's, a unit's),
/// then what `nix/rust.nix` compiled in.
fn program_path(variable: &str, built: Option<&'static str>) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| built.filter(|p| !p.is_empty()).map(PathBuf::from))
}

/// A name nothing in `folder` uses: `name`, or `stem (copy n).ext` — the
/// file manager's own shape.
fn free_name(folder: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let wanted = folder.join(name);
    if wanted.symlink_metadata().is_err() {
        return wanted;
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    (1..10_000u32)
        .map(|n| folder.join(format!("{stem} (copy {n}){extension}")))
        .find(|c| c.symlink_metadata().is_err())
        .unwrap_or_else(|| folder.join(format!("{stem} ({}){extension}", std::process::id())))
}

/// A hidden folder of our own beside `near`, named so two processes and two
/// calls never share one.
fn scratch(near: &Path, what: &str) -> Result<PathBuf, Refusal> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = near.join(format!(".ronoctua-{what}-{}-{n}", std::process::id()));
    std::fs::create_dir(&dir).map_err(|e| {
        refused(
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Why::Exists
            } else {
                Why::Failed
            },
            None,
        )
    })?;
    Ok(dir)
}

/// The folder every source is in, and their names in it — what a program
/// stores. Sources from two folders are refused: an archive of a selection
/// is of one folder's selection.
fn one_folder(sources: &[PathBuf]) -> Result<(PathBuf, Vec<OsString>), Refusal> {
    let mut folder: Option<PathBuf> = None;
    let mut names = Vec::with_capacity(sources.len());
    for source in sources {
        let absolute = if source.is_absolute() {
            source.clone()
        } else {
            std::env::current_dir()
                .map_err(|_| refused(Why::Failed, None))?
                .join(source)
        };
        let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) else {
            return Err(refused(Why::Unsupported, None));
        };
        match &folder {
            None => folder = Some(parent.to_path_buf()),
            Some(f) if f == parent => {}
            Some(_) => return Err(refused(Why::Unsupported, None)),
        }
        names.push(name.to_os_string());
    }
    folder
        .map(|f| (f, names))
        .ok_or(refused(Why::Unsupported, None))
}

/// `path` as an argument that cannot be read as a switch, nor as bsdtar's
/// `-` (stdin): absolute stays, relative gains `./`.
fn plain(path: &Path) -> OsString {
    if path.is_absolute() {
        path.as_os_str().to_os_string()
    } else {
        Path::new(".").join(path).into_os_string()
    }
}

/// A member name for bsdtar's patterns: `*`, `?`, `[` and `\` escaped, so a
/// name is matched as itself.
fn literal_pattern(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if matches!(c, '*' | '?' | '[' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A progress reporter that hands on only what changed.
struct Reporter<'a> {
    tool: Tool,
    last: Progress,
    out: &'a mut dyn FnMut(&Progress),
}

impl Reporter<'_> {
    fn piece(&mut self, piece: &str) {
        let Some(p) = parse::progress(self.tool, piece) else {
            return;
        };
        let next = Progress {
            percent: p.percent.or(self.last.percent),
            member: p.member.or_else(|| self.last.member.clone()),
        };
        if next != self.last {
            self.last = next;
            (self.out)(&self.last);
        }
    }
}

/// Where the programs are; each operation is a method, so a test hands its
/// own.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tools {
    /// 7zz.
    pub sevenzz: Option<PathBuf>,
    /// bsdtar.
    pub bsdtar: Option<PathBuf>,
    /// unrar.
    pub unrar: Option<PathBuf>,
    /// setsid (util-linux), which every program runs under: without it 7zz
    /// and unrar are refused, since a password question could reach a
    /// terminal nobody is looking at.
    pub setsid: Option<PathBuf>,
    /// The caller's hand on every run these tools make: hold, go on, stop.
    /// `None` runs every program to its end.
    pub switch: Option<Switch>,
}

impl Tools {
    /// The programs this process was given: each variable from the
    /// environment, else as `nix/rust.nix` compiled it in.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            sevenzz: program_path(SEVENZZ_VARIABLE, option_env!("RONOCTUA_7ZZ")),
            bsdtar: program_path(BSDTAR_VARIABLE, option_env!("RONOCTUA_BSDTAR")),
            unrar: program_path(UNRAR_VARIABLE, option_env!("RONOCTUA_UNRAR")),
            setsid: program_path(SETSID_VARIABLE, option_env!("RONOCTUA_SETSID")),
            switch: None,
        }
    }

    /// The same programs, unrar found at `found` when nothing handed it in —
    /// a machine's unrar is a part, which this crate does not know
    /// (`ronoctua_parts::file_of`, asked by the caller on every run).
    #[must_use]
    pub fn with_unrar(mut self, found: Option<PathBuf>) -> Self {
        if self.unrar.is_none() {
            self.unrar = found;
        }
        self
    }

    /// The same programs, run under `switch`.
    #[must_use]
    pub fn held_by(self, switch: Switch) -> Self {
        Self {
            switch: Some(switch),
            ..self
        }
    }

    /// Whether the caller's switch says stop — asked after a run, so a program
    /// killed by it is [`Why::Stopped`] and never the failure its death looks
    /// like.
    fn stopped(&self) -> bool {
        self.switch.as_ref().is_some_and(|s| s.now() == Hold::Stop)
    }

    /// The path of a program, or the refusal that names it.
    ///
    /// # Errors
    ///
    /// [`Why::NoTool`].
    pub fn program(&self, tool: Tool) -> Result<&Path, Refusal> {
        let path = match tool {
            Tool::SevenZz => &self.sevenzz,
            Tool::Bsdtar => &self.bsdtar,
            Tool::Unrar => &self.unrar,
        };
        path.as_deref().ok_or(refused(Why::NoTool, Some(tool)))
    }

    /// Runs `call`; `Ok` only when the program succeeded.
    fn go(&self, call: &Call<'_>, each: &mut dyn FnMut(&str)) -> Result<(), Refusal> {
        let program = self.program(call.tool)?;
        let setsid = self.setsid.as_deref();
        if setsid.is_none() && call.tool != Tool::Bsdtar {
            return Err(refused(Why::NoTool, Some(call.tool)));
        }
        if self.stopped() {
            return Err(refused(Why::Stopped, Some(call.tool)));
        }
        let ended = run::run(program, setsid, call, self.switch.as_ref(), each)?;
        if self.stopped() {
            return Err(refused(Why::Stopped, Some(call.tool)));
        }
        match run::verdict(call.tool, ended.code, &ended.tail, call.password.is_some()) {
            None => Ok(()),
            Some(why) => Err(refused(why, Some(call.tool))),
        }
    }

    /// The members of the archive at `path` (`secret`: the password's file,
    /// for one with hidden names).
    ///
    /// # Errors
    ///
    /// [`Why::NeedsPassword`] (hidden names and no password),
    /// [`Why::WrongPassword`], [`Why::Corrupt`], [`Why::NoTool`],
    /// [`Why::Failed`].
    pub fn list(&self, path: &Path, secret: Option<&Path>) -> Result<Vec<Member>, Refusal> {
        let tool = Format::of_path(path).reader();
        let password = secret.map(|s| Password::read(s, tool)).transpose()?;
        let password = password.as_ref().map(|p| (p, 4));
        match tool {
            Tool::SevenZz => {
                let mut listing = parse::SevenZzListing::default();
                let args = ["l", "-slt", "-ba", "-sccUTF-8", "--"]
                    .map(OsString::from)
                    .into_iter()
                    .chain([plain(path)])
                    .collect();
                self.go(
                    &Call {
                        tool,
                        args,
                        cwd: None,
                        password,
                        listing: None,
                    },
                    &mut |line| listing.line(line),
                )?;
                Ok(listing.finish())
            }
            Tool::Unrar => {
                let mut listing = parse::UnrarListing::default();
                let args = ["lt", "-idcdp", "--"]
                    .map(OsString::from)
                    .into_iter()
                    .chain([plain(path)])
                    .collect();
                self.go(
                    &Call {
                        tool,
                        args,
                        cwd: None,
                        password,
                        listing: None,
                    },
                    &mut |line| listing.line(line),
                )?;
                Ok(listing.finish())
            }
            Tool::Bsdtar => {
                // The archive written again as an mtree: one line per member,
                // every awkward byte of a name escaped (`parse`).
                let mut members = Vec::new();
                let mut at = OsString::from("@");
                at.push(plain(path));
                let args = [
                    "-c",
                    "-f",
                    "-",
                    "--format=mtree",
                    "--options=!all,type,size,time",
                ]
                .map(OsString::from)
                .into_iter()
                .chain([at])
                .collect();
                self.go(
                    &Call {
                        tool,
                        args,
                        cwd: None,
                        password: None,
                        listing: None,
                    },
                    &mut |line| members.extend(parse::mtree_member(line)),
                )?;
                Ok(members)
            }
        }
    }

    /// Extracts `members` (every member when empty) of the archive at `path`
    /// into the folder `into` (made when missing). Nothing already there is
    /// replaced: what came out takes a free name beside it.
    ///
    /// # Errors
    ///
    /// As [`Tools::list`]; on any refusal nothing is left in `into`.
    pub fn extract(
        &self,
        path: &Path,
        into: &Path,
        members: &[String],
        secret: Option<&Path>,
        progress: &mut dyn FnMut(&Progress),
    ) -> Result<(), Refusal> {
        let tool = Format::of_path(path).reader();
        let password = secret.map(|s| Password::read(s, tool)).transpose()?;
        // Refused before anything is made: the program is missing.
        self.program(tool)?;
        std::fs::create_dir_all(into).map_err(|_| refused(Why::Failed, Some(tool)))?;
        let staging = scratch(into, "extract")?;
        let archive = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|_| refused(Why::Failed, Some(tool)))?
                .join(path)
        };
        let mut args: Vec<OsString> = Vec::new();
        match tool {
            Tool::SevenZz => {
                let mut out = OsString::from("-o");
                out.push(&staging);
                args.extend(
                    [
                        "x",
                        "-aou",
                        "-y",
                        "-bsp1",
                        "-bso0",
                        "-bb0",
                        "-sccUTF-8",
                        "-spd",
                    ]
                    .map(OsString::from),
                );
                args.push(out);
                args.push("--".into());
                args.push(archive.into_os_string());
                args.extend(members.iter().map(OsString::from));
            }
            Tool::Unrar => {
                // EVERY MEMBER, and the selection kept afterwards: unrar has
                // no way to read a name as itself — `folder/n?te.txt` is a
                // wildcard that extracts `folder/note.txt` too (MEASURED,
                // 7.2.6), and a name that starts with `@` is a list file.
                args.extend(["x", "-or", "-idcd", "--"].map(OsString::from));
                args.push(archive.into_os_string());
                let mut to = staging.clone().into_os_string();
                to.push("/");
                args.push(to);
            }
            Tool::Bsdtar => {
                args.extend(["-x", "-v", "-f"].map(OsString::from));
                args.push(archive.into_os_string());
                args.push("-C".into());
                args.push(staging.clone().into_os_string());
                args.push("--".into());
                args.extend(members.iter().map(|m| OsString::from(literal_pattern(m))));
            }
        }
        let mut reporter = Reporter {
            tool,
            last: Progress::default(),
            out: progress,
        };
        let done = self.go(
            &Call {
                tool,
                args,
                cwd: None,
                password: password.as_ref().map(|p| (p, 4)),
                listing: None,
            },
            &mut |piece| reporter.piece(piece),
        );
        let kept = done.and_then(|()| {
            if tool == Tool::Unrar && !members.is_empty() {
                keep_only(&staging, members, tool).map(Some)
            } else {
                Ok(None)
            }
        });
        let moved = kept.as_ref().map_err(|r| *r).and_then(|kept| {
            let from = kept.as_deref().unwrap_or(&staging);
            let mut entries: Vec<_> = std::fs::read_dir(from)
                .map_err(|_| refused(Why::Failed, Some(tool)))?
                .flatten()
                .collect();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let to = free_name(into, &entry.file_name());
                std::fs::rename(entry.path(), to).map_err(|_| refused(Why::Failed, Some(tool)))?;
            }
            Ok(())
        });
        if let Ok(Some(kept)) = &kept {
            let _ = std::fs::remove_dir_all(kept);
        }
        let _ = std::fs::remove_dir_all(&staging);
        moved
    }

    /// Creates the archive `target` from `sources` (all in one folder).
    ///
    /// # Errors
    ///
    /// [`Why::Exists`], [`Why::Unsupported`] (a format this crate does not
    /// write, a password or split volumes on a tar, hidden names without a
    /// password or outside 7z, sources from two folders), and as
    /// [`Tools::list`].
    pub fn create(
        &self,
        sources: &[PathBuf],
        target: &Path,
        options: &Options,
        progress: &mut dyn FnMut(&Progress),
    ) -> Result<(), Refusal> {
        let format = options.format;
        let tool = if format.reader() == Tool::Bsdtar {
            Tool::Bsdtar
        } else {
            Tool::SevenZz
        };
        let unsupported = refused(Why::Unsupported, Some(tool));
        let fits = format.creatable()
            && (options.secret.is_none() || format.encryptable())
            && (!options.hide_names || (format == Format::SevenZ && options.secret.is_some()))
            && (options.volume_bytes.is_none() || tool == Tool::SevenZz)
            && options.volume_bytes != Some(0)
            && !sources.is_empty();
        if !fits {
            return Err(unsupported);
        }
        let (folder, names) = one_folder(sources)?;
        let target = if target.is_absolute() {
            target.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|_| refused(Why::Failed, Some(tool)))?
                .join(target)
        };
        // A split archive's first volume is `<name>.001`.
        let first = match options.volume_bytes {
            Some(_) => {
                let mut v = target.clone().into_os_string();
                v.push(".001");
                PathBuf::from(v)
            }
            None => target.clone(),
        };
        if first.symlink_metadata().is_ok() || target.symlink_metadata().is_ok() {
            return Err(refused(Why::Exists, Some(tool)));
        }
        let password = options
            .secret
            .as_deref()
            .map(|s| Password::read(s, tool))
            .transpose()?;
        let mut args: Vec<OsString> = Vec::new();
        let mut listing = None;
        match tool {
            Tool::SevenZz => {
                let kind = if format == Format::Zip {
                    "-tzip"
                } else {
                    "-t7z"
                };
                args.extend(
                    [
                        "a",
                        kind,
                        "-mmt=on",
                        "-bsp1",
                        "-bso0",
                        "-bb0",
                        "-sccUTF-8",
                        "-spd",
                        // The name exactly as given: 7zz would add `.7z` to
                        // one without an extension.
                        "-sae",
                    ]
                    .map(OsString::from),
                );
                args.push(format!("-mx{}", options.level.number()).into());
                if let Some(bytes) = options.volume_bytes {
                    args.push(format!("-v{bytes}b").into());
                }
                if password.is_some() {
                    // With no value: 7zz asks, and the answer comes on stdin.
                    args.push("-p".into());
                    if format == Format::Zip {
                        // AES-256, not ZipCrypto (7zz's zip default).
                        args.push("-mem=AES256".into());
                    }
                    if options.hide_names {
                        args.push("-mhe=on".into());
                    }
                }
                // After `--` a name that starts with `@` is a name, not a
                // list file to read names from (MEASURED, 26.02).
                args.push("--".into());
                args.push(target.clone().into_os_string());
                args.extend(names);
            }
            Tool::Bsdtar | Tool::Unrar => {
                args.extend(["-c", "-v", "-f"].map(OsString::from));
                args.push(target.clone().into_os_string());
                // A name on this machine does not travel in the archive: the
                // owner is written as numbers only.
                args.push("--numeric-owner".into());
                if let Some((switch, name)) = format.filter() {
                    args.push(switch.into());
                    if let Some(level) = options.level.for_filter(name) {
                        args.push(format!("--options={name}:compression-level={level}").into());
                    }
                }
                args.push("-C".into());
                args.push(folder.clone().into_os_string());
                // THE NAMES ON STDIN, NUL-ended, and never as arguments.
                // bsdtar reads a name argument that starts with `@` as an
                // ARCHIVE whose members it copies — after `--` too — and one
                // that starts with `-C` as a folder to change into. MEASURED
                // (3.8.8): the selection `@old.tar` beside an `old.tar` packed
                // the other archive's members, and `@types` failed. From
                // `--null -T -` a name is a name.
                args.extend(["--null", "-T", "-"].map(OsString::from));
                let mut list = Vec::new();
                for name in &names {
                    list.extend_from_slice(name.as_encoded_bytes());
                    list.push(0);
                }
                listing = Some(list);
            }
        }
        let mut reporter = Reporter {
            tool,
            last: Progress::default(),
            out: progress,
        };
        let done = self.go(
            &Call {
                tool,
                args,
                cwd: Some(&folder),
                password: password.as_ref().map(|p| (p, 2)),
                listing: listing.as_deref(),
            },
            &mut |piece| reporter.piece(piece),
        );
        if done.is_err() {
            // What this call wrote, and only that: the target did not exist.
            let _ = std::fs::remove_file(&target);
            if options.volume_bytes.is_some() {
                for n in 1..10_000u32 {
                    let mut v = target.clone().into_os_string();
                    v.push(format!(".{n:03}"));
                    if std::fs::remove_file(PathBuf::from(v)).is_err() {
                        break;
                    }
                }
            }
        }
        done
    }

    /// Tests every member of the archive at `path`.
    ///
    /// # Errors
    ///
    /// [`Why::Corrupt`], [`Why::WrongPassword`], and as [`Tools::list`].
    pub fn test(&self, path: &Path, secret: Option<&Path>) -> Result<(), Refusal> {
        self.test_members(path, &[], secret)
    }

    /// Tests `members` of the archive at `path` — every one when empty. A
    /// member is named as itself (`-spd`), never as a pattern; unrar and
    /// bsdtar test the whole archive whatever is named, which is only ever
    /// asked of them empty.
    fn test_members(
        &self,
        path: &Path,
        members: &[String],
        secret: Option<&Path>,
    ) -> Result<(), Refusal> {
        let tool = Format::of_path(path).reader();
        let password = secret.map(|s| Password::read(s, tool)).transpose()?;
        let head: &[&str] = match tool {
            Tool::SevenZz => &["t", "-bsp0", "-bso0", "-bb0", "-sccUTF-8", "-spd", "--"],
            Tool::Unrar => &["t", "-idcdp", "--"],
            Tool::Bsdtar => &["-t", "-f"],
        };
        let named = if tool == Tool::SevenZz { members } else { &[] };
        let args = head
            .iter()
            .map(OsString::from)
            .chain([plain(path)])
            .chain(named.iter().map(OsString::from))
            .collect();
        self.go(
            &Call {
                tool,
                args,
                cwd: None,
                password: password.as_ref().map(|p| (p, 4)),
                listing: None,
            },
            &mut |_| {},
        )
    }

    /// Adds `sources` (all in one folder) to the archive at `archive` (zip
    /// and 7z). With a password the new members are encrypted with it (and
    /// a 7z with hidden names keeps them hidden).
    ///
    /// # An encrypted archive takes nothing in the clear
    ///
    /// 7zz adds to an encrypted archive WITHOUT a password and says nothing:
    /// the new members go in readable by anybody, beside members that are not
    /// (MEASURED, 7zz 26.02: `7zz a enc.zip b.txt` on an AES zip exits 0 and
    /// lists `b.txt Encrypted = -`). And given a password it does not check it
    /// against what is already there: the new members would be sealed under
    /// a password nothing else in the archive opens. So an archive with an
    /// encrypted member is refused without one ([`Why::NeedsPassword`] — the
    /// word a door answers by asking), and the one given is proved first on
    /// its smallest encrypted member ([`Why::WrongPassword`]).
    ///
    /// # Errors
    ///
    /// [`Why::Unsupported`] for a tar, the two above, and as [`Tools::list`].
    pub fn add(
        &self,
        archive: &Path,
        sources: &[PathBuf],
        secret: Option<&Path>,
        progress: &mut dyn FnMut(&Progress),
    ) -> Result<(), Refusal> {
        let format = Format::of_path(archive);
        if !format.editable() || sources.is_empty() {
            return Err(refused(Why::Unsupported, Some(format.reader())));
        }
        let (folder, names) = one_folder(sources)?;
        self.opens_what_is_sealed(archive, secret)?;
        let password = secret
            .map(|s| Password::read(s, Tool::SevenZz))
            .transpose()?;
        let mut args: Vec<OsString> = ["a", "-bsp1", "-bso0", "-bb0", "-sccUTF-8", "-spd", "-sae"]
            .map(OsString::from)
            .to_vec();
        if password.is_some() {
            args.push("-p".into());
            if format == Format::Zip {
                args.push("-mem=AES256".into());
            }
        }
        args.push("--".into());
        args.push(archive_path(archive, Tool::SevenZz)?.into_os_string());
        args.extend(names);
        let mut reporter = Reporter {
            tool: Tool::SevenZz,
            last: Progress::default(),
            out: progress,
        };
        self.go(
            &Call {
                tool: Tool::SevenZz,
                args,
                cwd: Some(&folder),
                password: password.as_ref().map(|p| (p, 4)),
                listing: None,
            },
            &mut |piece| reporter.piece(piece),
        )
    }

    /// Whether `secret` opens what the archive at `archive` already keeps
    /// encrypted — see [`Tools::add`]. An archive with nothing encrypted
    /// needs none; one with hidden names is proved by being listed at all.
    fn opens_what_is_sealed(&self, archive: &Path, secret: Option<&Path>) -> Result<(), Refusal> {
        let members = self.list(archive, secret)?;
        let Some(smallest) = members
            .iter()
            .filter(|m| m.encrypted && !m.is_dir)
            .min_by_key(|m| m.size_bytes.unwrap_or(u64::MAX))
        else {
            return Ok(());
        };
        if secret.is_none() {
            return Err(refused(Why::NeedsPassword, Some(Tool::SevenZz)));
        }
        self.test_members(archive, std::slice::from_ref(&smallest.path), secret)
    }

    /// Removes `members` from the archive at `archive` (zip and 7z).
    ///
    /// # Errors
    ///
    /// As [`Tools::add`].
    pub fn remove(
        &self,
        archive: &Path,
        members: &[String],
        secret: Option<&Path>,
    ) -> Result<(), Refusal> {
        let format = Format::of_path(archive);
        if !format.editable() || members.is_empty() {
            return Err(refused(Why::Unsupported, Some(format.reader())));
        }
        let password = secret
            .map(|s| Password::read(s, Tool::SevenZz))
            .transpose()?;
        // No `-p`: on `d` it would mean an empty password. 7zz asks for the
        // one it needs to read hidden names, and stdin answers.
        let mut args: Vec<OsString> = [
            "d",
            "-bsp0",
            "-bso0",
            "-bb0",
            "-sccUTF-8",
            "-spd",
            "-sae",
            "--",
        ]
        .map(OsString::from)
        .to_vec();
        args.push(archive_path(archive, Tool::SevenZz)?.into_os_string());
        args.extend(members.iter().map(OsString::from));
        self.go(
            &Call {
                tool: Tool::SevenZz,
                args,
                cwd: None,
                password: password.as_ref().map(|p| (p, 4)),
                listing: None,
            },
            &mut |_| {},
        )
    }

    /// Writes the archive at `from` (its password's file `from_secret`) again
    /// as `target`, in `options`' format: extracted into a hidden folder
    /// beside `target`, packed, and the folder removed.
    ///
    /// # Errors
    ///
    /// As [`Tools::extract`] and [`Tools::create`].
    pub fn convert(
        &self,
        from: &Path,
        from_secret: Option<&Path>,
        target: &Path,
        options: &Options,
        progress: &mut dyn FnMut(&Progress),
    ) -> Result<(), Refusal> {
        if target.symlink_metadata().is_ok() {
            return Err(refused(Why::Exists, None));
        }
        let near = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let staging = scratch(near, "convert")?;
        let done = self
            .extract(from, &staging, &[], from_secret, progress)
            .and_then(|()| {
                let mut sources: Vec<PathBuf> = std::fs::read_dir(&staging)
                    .map_err(|_| refused(Why::Failed, None))?
                    .flatten()
                    .map(|e| e.path())
                    .collect();
                sources.sort();
                self.create(&sources, target, options, progress)
            });
        let _ = std::fs::remove_dir_all(&staging);
        done
    }

    /// A plain tar of `folder`, STREAMED: bsdtar started with the archive on
    /// its standard output, which the caller reads (`child.stdout`) and waits
    /// on — encrypt-anything's `name.tar.age`, sealed as it is written, so
    /// the folder in clear never lands on a disk as one file.
    ///
    /// The archive holds the folder under its own name, links as links and
    /// owners as numbers. The name crosses on stdin, NUL-ended
    /// ([`Tools::create`]'s reason: an argument that starts with `@` or `-C`
    /// is read as something else), and stdin is closed before this returns.
    /// bsdtar's complaints go where this process's go (its standard error);
    /// a member it could not read makes it exit with a failure at the end,
    /// which the caller's `wait` sees. The child is in the caller's process
    /// group, so whatever stops the caller's group stops it too.
    ///
    /// # Errors
    ///
    /// [`Why::NoTool`] without bsdtar, [`Why::Unsupported`] for a path that
    /// names no folder inside another (`/`, `..`), [`Why::Unstartable`] when
    /// the program does not start.
    pub fn tar_stream(&self, folder: &Path) -> Result<std::process::Child, Refusal> {
        let program = self.program(Tool::Bsdtar)?;
        let unsupported = refused(Why::Unsupported, Some(Tool::Bsdtar));
        let (parent, names) = one_folder(&[folder.to_path_buf()])?;
        if names.iter().any(|n| n == "." || n == "..") {
            return Err(unsupported);
        }
        let mut args: Vec<OsString> = ["-c", "-f", "-", "--numeric-owner", "-C"]
            .map(OsString::from)
            .to_vec();
        args.push(parent.clone().into_os_string());
        args.extend(["--null", "-T", "-"].map(OsString::from));
        let mut listing = Vec::new();
        for name in &names {
            listing.extend_from_slice(name.as_encoded_bytes());
            listing.push(0);
        }
        run::stream(
            program,
            &run::Call {
                tool: Tool::Bsdtar,
                args,
                cwd: Some(&parent),
                password: None,
                listing: Some(&listing),
            },
        )
    }
}

/// Moves the `members` out of `staging` (everything extracted) into a
/// hidden folder of their own beside it, each at its own path; a folder
/// brings what is under it, as 7zz and bsdtar do. A member that names no
/// path (`..`, absolute) or that did not come out is [`Why::Failed`]: the
/// person asked for it.
fn keep_only(staging: &Path, members: &[String], tool: Tool) -> Result<PathBuf, Refusal> {
    let near = staging.parent().unwrap_or(staging);
    let kept = scratch(near, "keep")?;
    let failed = refused(Why::Failed, Some(tool));
    // A folder before what is under it (a prefix sorts first), so the
    // folder moves whole and its members are then found already moved.
    let mut wanted: Vec<&str> = members.iter().map(|m| m.trim_end_matches('/')).collect();
    wanted.sort_unstable();
    wanted.dedup();
    let done = wanted.into_iter().try_for_each(|member| {
        let relative = Path::new(member);
        let plain = relative
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)));
        if !plain || relative.as_os_str().is_empty() {
            return Err(failed);
        }
        let from = staging.join(relative);
        let to = kept.join(relative);
        if from.symlink_metadata().is_err() {
            // Already moved with a folder asked for before it.
            return if to.symlink_metadata().is_ok() {
                Ok(())
            } else {
                Err(failed)
            };
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|_| failed)?;
        }
        std::fs::rename(&from, &to).map_err(|_| failed)
    });
    match done {
        Ok(()) => Ok(kept),
        Err(r) => {
            let _ = std::fs::remove_dir_all(&kept);
            Err(r)
        }
    }
}

/// An archive's path made absolute (7zz runs in the sources' folder).
fn archive_path(path: &Path, tool: Tool) -> Result<PathBuf, Refusal> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .map_err(|_| refused(Why::Failed, Some(tool)))?
            .join(path))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    /// ONE TEST AT A TIME STARTS A PROGRAM HERE. A stand-in is a script
    /// written and then run; a `fork` in another test's thread between the
    /// write and the `exec` inherits the file open for writing, and the
    /// `exec` fails with "text file busy". Serialised, it cannot.
    static FORKS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        FORKS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The real programs: the environment's (the dev shell, the tests
    /// derivation) or what `nix/rust.nix` compiled in. A test without them
    /// FAILS — a test that returns early proves nothing and passes.
    fn tools() -> Tools {
        let tools = Tools::from_env();
        for (name, path) in [
            (SEVENZZ_VARIABLE, &tools.sevenzz),
            (BSDTAR_VARIABLE, &tools.bsdtar),
            (UNRAR_VARIABLE, &tools.unrar),
            (SETSID_VARIABLE, &tools.setsid),
        ] {
            assert!(
                path.as_deref().is_some_and(Path::exists),
                "{name} is not set to a program: run inside `nix develop` or the tests derivation"
            );
        }
        tools
    }

    /// A folder of this test's own, empty.
    fn place(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-archive-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A password file the way the queue writes one: `0600`, one line.
    fn secret(dir: &Path, name: &str, password: &str) -> PathBuf {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let path = dir.join(name);
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        writeln!(f, "{password}").unwrap();
        path
    }

    /// Every regular file under `root`, relative, with its bytes.
    fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                let meta = entry.metadata().unwrap();
                if meta.is_dir() {
                    walk(root, &path, out);
                } else {
                    let rel = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    out.insert(rel, std::fs::read(&path).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        if root.exists() {
            walk(root, root, &mut out);
        }
        out
    }

    /// The tree every archive here is made of: an awkward name or three.
    fn tree(dir: &Path) -> PathBuf {
        let src = dir.join("src");
        std::fs::create_dir_all(src.join("sp ace/empty")).unwrap();
        std::fs::write(src.join("plain.txt"), "plain text\n").unwrap();
        std::fs::write(src.join("sp ace/we=ird#name\\x.txt"), "weird\n").unwrap();
        std::fs::write(src.join("-dash.txt"), "a name like a switch\n").unwrap();
        std::fs::write(src.join("star*.txt"), "a name like a pattern\n").unwrap();
        // Names a program reads as something else when they are arguments:
        // `@x` a list file (7zz) or an archive to copy (bsdtar), `-Cx` a
        // folder to change into (bsdtar).
        std::fs::create_dir_all(src.join("@types")).unwrap();
        std::fs::write(src.join("@types/index.d.ts"), "export {};\n").unwrap();
        std::fs::write(src.join("-Cx.txt"), "a name like -C\n").unwrap();
        src
    }

    fn sources(src: &Path) -> Vec<PathBuf> {
        let mut all: Vec<PathBuf> = std::fs::read_dir(src)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        all.sort();
        all
    }

    fn options(format: Format) -> Options {
        Options {
            format,
            level: Level::Normal,
            volume_bytes: None,
            secret: None,
            hide_names: false,
        }
    }

    fn quiet(_: &Progress) {}

    /// THE PROOF THE REST STANDS ON: the password reaches the program on
    /// stdin and never in its arguments. A stand-in "7zz" writes down its
    /// argv and its stdin; the real one is exercised below.
    #[test]
    fn a_password_is_written_to_stdin_and_never_to_argv() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("argv");
        let spy = dir.join("spy");
        std::fs::write(
            &spy,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{0}/argv'\ncat > '{0}/stdin'\n",
                dir.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&spy, std::fs::Permissions::from_mode(0o700)).unwrap();
        let src = tree(&dir);
        let pw = secret(&dir, "pw", "open sesame 42");
        let stand_in = Tools {
            sevenzz: Some(spy),
            ..tools
        };
        let mut o = options(Format::SevenZ);
        o.secret = Some(pw.clone());
        o.hide_names = true;
        stand_in
            .create(&sources(&src), &dir.join("out.7z"), &o, &mut quiet)
            .unwrap();
        let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
        let stdin = std::fs::read_to_string(dir.join("stdin")).unwrap();
        assert!(!argv.contains("sesame"), "the password is in argv: {argv}");
        assert!(
            argv.lines().any(|a| a == "-p"),
            "7zz is told to ask: {argv}"
        );
        assert!(argv.lines().any(|a| a == "-mhe=on"));
        assert_eq!(stdin, "open sesame 42\nopen sesame 42\n");
        stand_in
            .extract(
                &dir.join("x.7z"),
                &dir.join("into"),
                &[],
                Some(&pw),
                &mut quiet,
            )
            .unwrap();
        let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
        assert!(!argv.contains("sesame"));
        assert!(
            !argv.lines().any(|a| a.starts_with("-p")),
            "extracting, `-p` would mean an EMPTY password: {argv}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn seven_zz_takes_the_password_on_stdin_and_refuses_a_wrong_one() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("7z-password");
        let src = tree(&dir);
        let right = secret(&dir, "right", "right horse");
        let wrong = secret(&dir, "wrong", "wrong horse");
        let archive = dir.join("hidden.7z");
        let mut o = options(Format::SevenZ);
        o.secret = Some(right.clone());
        o.hide_names = true;
        tools
            .create(&sources(&src), &archive, &o, &mut quiet)
            .unwrap();

        // Hidden names: even the listing needs it.
        assert_eq!(
            tools.list(&archive, None).unwrap_err().why,
            Why::NeedsPassword
        );
        assert_eq!(
            tools.list(&archive, Some(&wrong)).unwrap_err().why,
            Why::WrongPassword
        );
        let members = tools.list(&archive, Some(&right)).unwrap();
        let names: Vec<&str> = members.iter().map(|m| m.path.as_str()).collect();
        for want in [
            "plain.txt",
            "sp ace/we=ird#name\\x.txt",
            "-dash.txt",
            "star*.txt",
        ] {
            assert!(names.contains(&want), "{want} missing from {names:?}");
        }
        let plain = members.iter().find(|m| m.path == "plain.txt").unwrap();
        assert!(plain.encrypted);
        assert_eq!(plain.size_bytes, Some(11));
        assert!(members.iter().any(|m| m.path == "sp ace/empty" && m.is_dir));

        let into = dir.join("into");
        assert_eq!(
            tools
                .extract(&archive, &into, &[], Some(&wrong), &mut quiet)
                .unwrap_err()
                .why,
            Why::WrongPassword
        );
        assert!(files(&into).is_empty(), "a wrong password leaves nothing");
        assert_eq!(
            std::fs::read_dir(&into).unwrap().count(),
            0,
            "not even the staging folder"
        );
        assert_eq!(
            tools
                .extract(&archive, &into, &[], None, &mut quiet)
                .unwrap_err()
                .why,
            Why::NeedsPassword
        );
        let mut seen = Vec::new();
        tools
            .extract(&archive, &into, &[], Some(&right), &mut |p| {
                seen.push(p.clone());
            })
            .unwrap();
        assert_eq!(files(&into), files(&src));
        assert_eq!(
            tools.test(&archive, Some(&wrong)).unwrap_err().why,
            Why::WrongPassword
        );
        tools.test(&archive, Some(&right)).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_zip_is_aes_and_its_names_stay_readable() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("zip-password");
        let src = tree(&dir);
        let right = secret(&dir, "right", "zip pass");
        let wrong = secret(&dir, "wrong", "nope");
        let archive = dir.join("out.zip");
        let mut o = options(Format::Zip);
        o.secret = Some(right.clone());
        tools
            .create(&sources(&src), &archive, &o, &mut quiet)
            .unwrap();
        let members = tools.list(&archive, None).unwrap();
        assert!(members.iter().filter(|m| !m.is_dir).all(|m| m.encrypted));
        // AES-256, not ZipCrypto: the method is 7zz's to say.
        let said = std::process::Command::new(tools.sevenzz.as_ref().unwrap())
            .args(["l", "-slt", "-ba", "--"])
            .arg(&archive)
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&said.stdout);
        assert!(said.contains("Method = AES-256"), "{said}");
        let into = dir.join("into");
        assert_eq!(
            tools
                .extract(&archive, &into, &[], Some(&wrong), &mut quiet)
                .unwrap_err()
                .why,
            Why::WrongPassword
        );
        assert!(files(&into).is_empty(), "no empty file per member is left");
        assert_eq!(
            tools
                .extract(&archive, &into, &[], None, &mut quiet)
                .unwrap_err()
                .why,
            Why::NeedsPassword
        );
        tools
            .extract(&archive, &into, &[], Some(&right), &mut quiet)
            .unwrap();
        assert_eq!(files(&into), files(&src));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unrar_takes_the_password_on_stdin_and_refuses_a_wrong_one() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("rar");
        let right = secret(&dir, "right", fixtures::PASSWORD);
        let wrong = secret(&dir, "wrong", "wrong horse");
        let write = |name: &str, bytes: &[u8]| {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        let plain = write("plain.rar", fixtures::PLAIN);
        let data = write("data.rar", fixtures::DATA);
        let hidden = write("hidden.rar", fixtures::HIDDEN);
        let note = |into: &Path| std::fs::read_to_string(into.join("folder/note.txt")).unwrap();

        let members = tools.list(&plain, None).unwrap();
        assert_eq!(members.len(), 2, "{members:?}");
        let n = members
            .iter()
            .find(|m| m.path == "folder/note.txt")
            .unwrap();
        assert_eq!(n.size_bytes, Some(16));
        assert_eq!(n.modified_s, Some(1_767_225_600), "2026-01-01, read as UTC");
        assert!(!n.encrypted);
        assert!(members.iter().any(|m| m.path == "folder" && m.is_dir));
        let into = dir.join("plain");
        tools.extract(&plain, &into, &[], None, &mut quiet).unwrap();
        assert_eq!(note(&into), fixtures::NOTE);
        // A member is itself: unrar would read `n?te` as a wildcard and
        // extract `note.txt` (MEASURED, 7.2.6).
        let into = dir.join("pattern");
        assert_eq!(
            tools
                .extract(
                    &plain,
                    &into,
                    &["folder/n?te.txt".to_owned()],
                    None,
                    &mut quiet
                )
                .unwrap_err()
                .why,
            Why::Failed,
            "no member has that name"
        );
        assert!(files(&into).is_empty());
        assert_eq!(
            std::fs::read_dir(&into).unwrap().count(),
            0,
            "no staging left"
        );
        tools
            .extract(
                &plain,
                &into,
                &["folder/note.txt".to_owned()],
                None,
                &mut quiet,
            )
            .unwrap();
        assert_eq!(
            files(&into).into_keys().collect::<Vec<_>>(),
            ["folder/note.txt"]
        );
        let into = dir.join("folder-and-member");
        tools
            .extract(
                &plain,
                &into,
                &["folder/note.txt".to_owned(), "folder/".to_owned()],
                None,
                &mut quiet,
            )
            .unwrap();
        assert_eq!(
            note(&into),
            fixtures::NOTE,
            "a folder brings what is under it"
        );

        // The data encrypted: the names are readable without it.
        let members = tools.list(&data, None).unwrap();
        assert!(
            members
                .iter()
                .find(|m| m.path == "folder/note.txt")
                .unwrap()
                .encrypted
        );
        let into = dir.join("data");
        assert_eq!(
            tools
                .extract(&data, &into, &[], None, &mut quiet)
                .unwrap_err()
                .why,
            Why::NeedsPassword
        );
        assert_eq!(
            tools
                .extract(&data, &into, &[], Some(&wrong), &mut quiet)
                .unwrap_err()
                .why,
            Why::WrongPassword
        );
        assert!(files(&into).is_empty());
        tools
            .extract(&data, &into, &[], Some(&right), &mut quiet)
            .unwrap();
        assert_eq!(note(&into), fixtures::NOTE);

        // The names encrypted: the listing needs it too.
        assert_eq!(
            tools.list(&hidden, None).unwrap_err().why,
            Why::NeedsPassword
        );
        assert_eq!(
            tools.list(&hidden, Some(&wrong)).unwrap_err().why,
            Why::WrongPassword
        );
        assert_eq!(tools.list(&hidden, Some(&right)).unwrap().len(), 2);
        let into = dir.join("hidden");
        assert_eq!(
            tools
                .extract(&hidden, &into, &[], Some(&wrong), &mut quiet)
                .unwrap_err()
                .why,
            Why::WrongPassword
        );
        tools
            .extract(&hidden, &into, &[], Some(&right), &mut quiet)
            .unwrap();
        assert_eq!(note(&into), fixtures::NOTE);
        assert_eq!(
            tools.test(&hidden, Some(&wrong)).unwrap_err().why,
            Why::WrongPassword
        );
        tools.test(&hidden, Some(&right)).unwrap();

        // Nothing makes a RAR.
        let mut o = options(Format::Rar);
        o.level = Level::Store;
        assert_eq!(
            tools
                .create(&[plain], &dir.join("new.rar"), &o, &mut quiet)
                .unwrap_err()
                .why,
            Why::Unsupported
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn every_tar_and_every_level_goes_there_and_back() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("tar");
        let src = tree(&dir);
        // A line end in a name: the mtree listing still reads it as one.
        std::fs::write(src.join("new\nline"), "n").unwrap();
        // `@old.tar` beside an `old.tar`: as an argument, bsdtar packed the
        // members of `old.tar` in its place (MEASURED, 3.8.8).
        let inner = dir.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join("inner.txt"), "the other archive's\n").unwrap();
        tools
            .create(
                &[inner.join("inner.txt")],
                &src.join("old.tar"),
                &options(Format::Tar),
                &mut quiet,
            )
            .unwrap();
        std::fs::write(src.join("@old.tar"), "a file, not an archive\n").unwrap();
        for format in [
            Format::Tar,
            Format::TarGz,
            Format::TarXz,
            Format::TarZst,
            Format::TarBz2,
            Format::TarLz4,
        ] {
            for level in Level::ALL {
                let archive = dir.join(format!(
                    "{}-{}.{}",
                    format.id(),
                    level.id(),
                    format.extension()
                ));
                let mut o = options(format);
                o.level = level;
                let mut named = Vec::new();
                tools
                    .create(&sources(&src), &archive, &o, &mut |p| {
                        named.extend(p.member.clone());
                    })
                    .unwrap_or_else(|r| panic!("{} at {}: {r:?}", format.id(), level.id()));
                assert!(
                    !named.is_empty(),
                    "no progress while creating {}",
                    format.id()
                );
                assert_eq!(Format::of_path(&archive), format);
                let members = tools.list(&archive, None).unwrap();
                let names: Vec<&str> = members.iter().map(|m| m.path.as_str()).collect();
                for want in [
                    "plain.txt",
                    "sp ace/we=ird#name\\x.txt",
                    "new\nline",
                    "-dash.txt",
                    "star*.txt",
                    "@old.tar",
                    "@types/index.d.ts",
                    "-Cx.txt",
                ] {
                    assert!(names.contains(&want), "{want:?} missing from {names:?}");
                }
                assert!(!names.contains(&"inner.txt"), "{names:?}");
                assert!(
                    members
                        .iter()
                        .any(|m| m.path.trim_end_matches('/') == "sp ace/empty" && m.is_dir)
                );
                let plain = members.iter().find(|m| m.path == "plain.txt").unwrap();
                assert_eq!(plain.size_bytes, Some(11));
                assert!(plain.modified_s.is_some_and(|s| s > 1_700_000_000));
                tools.test(&archive, None).unwrap();
                let into = dir.join(format!("into-{}-{}", format.id(), level.id()));
                tools
                    .extract(&archive, &into, &[], None, &mut quiet)
                    .unwrap();
                assert_eq!(files(&into), files(&src), "{}", format.id());
            }
        }
        // One member alone, its `*` matched as itself.
        let archive = dir.join(format!("{}-normal.tar.zst", Format::TarZst.id()));
        let into = dir.join("one");
        tools
            .extract(&archive, &into, &["star*.txt".to_owned()], None, &mut quiet)
            .unwrap();
        assert_eq!(files(&into).into_keys().collect::<Vec<_>>(), ["star*.txt"]);

        // What a tar cannot do.
        let pw = secret(&dir, "pw", "x");
        let mut o = options(Format::TarXz);
        o.secret = Some(pw);
        assert_eq!(
            tools
                .create(&sources(&src), &dir.join("no.tar.xz"), &o, &mut quiet)
                .unwrap_err()
                .why,
            Why::Unsupported
        );
        assert_eq!(
            tools
                .add(&archive, &[src.join("plain.txt")], None, &mut quiet)
                .unwrap_err()
                .why,
            Why::Unsupported
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn zip_and_seven_z_at_every_level_and_in_volumes() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("seven");
        let src = tree(&dir);
        for format in [Format::Zip, Format::SevenZ] {
            for level in Level::ALL {
                let archive = dir.join(format!("{}.{}", level.id(), format.extension()));
                let mut o = options(format);
                o.level = level;
                tools
                    .create(&sources(&src), &archive, &o, &mut quiet)
                    .unwrap();
                let into = dir.join(format!("into-{}-{}", format.id(), level.id()));
                tools
                    .extract(&archive, &into, &[], None, &mut quiet)
                    .unwrap();
                assert_eq!(files(&into), files(&src));
            }
        }
        // Volumes: incompressible bytes, so the split is certain.
        let big = dir.join("big");
        std::fs::create_dir_all(&big).unwrap();
        let noise: Vec<u8> = (0u32..300_000)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        std::fs::write(big.join("noise.bin"), &noise).unwrap();
        let archive = dir.join("split.7z");
        let mut o = options(Format::SevenZ);
        o.level = Level::Store;
        o.volume_bytes = Some(100_000);
        let mut percents = Vec::new();
        tools
            .create(&[big.join("noise.bin")], &archive, &o, &mut |p| {
                percents.extend(p.percent);
            })
            .unwrap();
        assert!(dir.join("split.7z.001").exists() && dir.join("split.7z.004").exists());
        assert!(!archive.exists());
        assert_eq!(
            tools
                .create(&[big.join("noise.bin")], &archive, &o, &mut quiet)
                .unwrap_err()
                .why,
            Why::Exists,
            "the first volume counts as the archive"
        );
        let into = dir.join("from-volumes");
        tools
            .extract(&dir.join("split.7z.001"), &into, &[], None, &mut quiet)
            .unwrap();
        assert_eq!(std::fs::read(into.join("noise.bin")).unwrap(), noise);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn members_are_added_and_removed_in_place() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("edit");
        let src = tree(&dir);
        let more = dir.join("more");
        std::fs::create_dir_all(&more).unwrap();
        std::fs::write(more.join("late.txt"), "late\n").unwrap();
        let pw = secret(&dir, "pw", "edit me");

        for format in [Format::Zip, Format::SevenZ] {
            let archive = dir.join(format!("edit.{}", format.extension()));
            let mut o = options(format);
            o.secret = Some(pw.clone());
            o.hide_names = format == Format::SevenZ;
            tools
                .create(&sources(&src), &archive, &o, &mut quiet)
                .unwrap();
            tools
                .add(&archive, &[more.join("late.txt")], Some(&pw), &mut quiet)
                .unwrap();
            tools
                .remove(&archive, &["star*.txt".to_owned()], Some(&pw))
                .unwrap();
            let members = tools.list(&archive, Some(&pw)).unwrap();
            let names: Vec<&str> = members.iter().map(|m| m.path.as_str()).collect();
            assert!(names.contains(&"late.txt"), "{names:?}");
            assert!(!names.contains(&"star*.txt"), "{names:?}");
            assert!(
                members.iter().filter(|m| !m.is_dir).all(|m| m.encrypted),
                "an added member is encrypted with the rest"
            );
            if format == Format::SevenZ {
                assert_eq!(
                    tools.list(&archive, None).unwrap_err().why,
                    Why::NeedsPassword,
                    "the names stay hidden after an edit"
                );
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// THE DEFECT: 7zz adds to an encrypted archive without a password, and
    /// the new member goes in readable by anybody — and given a wrong one, it
    /// seals the new member under a password nothing else opens. Both are
    /// refused, by the words a door answers by asking.
    #[test]
    fn nothing_goes_into_an_encrypted_archive_in_the_clear_or_under_another_password() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("sealed-add");
        let src = tree(&dir);
        let more = dir.join("more");
        std::fs::create_dir_all(&more).unwrap();
        std::fs::write(more.join("late.txt"), "late\n").unwrap();
        let pw = secret(&dir, "pw", "the right one");
        let other = secret(&dir, "other", "another one");
        for format in [Format::Zip, Format::SevenZ] {
            let archive = dir.join(format!("sealed.{}", format.extension()));
            let mut o = options(format);
            o.secret = Some(pw.clone());
            tools
                .create(&sources(&src), &archive, &o, &mut quiet)
                .unwrap();
            let before = std::fs::read(&archive).unwrap();
            assert_eq!(
                tools
                    .add(&archive, &[more.join("late.txt")], None, &mut quiet)
                    .unwrap_err()
                    .why,
                Why::NeedsPassword,
                "{format:?}: added in the clear"
            );
            assert_eq!(
                tools
                    .add(&archive, &[more.join("late.txt")], Some(&other), &mut quiet)
                    .unwrap_err()
                    .why,
                Why::WrongPassword,
                "{format:?}: added under a password nothing else opens"
            );
            assert_eq!(
                std::fs::read(&archive).unwrap(),
                before,
                "{format:?}: a refused add touched the archive"
            );
            tools
                .add(&archive, &[more.join("late.txt")], Some(&pw), &mut quiet)
                .unwrap();
            let late = tools
                .list(&archive, Some(&pw))
                .unwrap()
                .into_iter()
                .find(|m| m.path == "late.txt")
                .expect("added");
            assert!(
                late.encrypted,
                "{format:?}: the added member is in the clear"
            );
        }
        // AND AN ARCHIVE WITH NOTHING ENCRYPTED needs no password at all.
        let plain_zip = dir.join("plain.zip");
        tools
            .create(
                &sources(&src),
                &plain_zip,
                &options(Format::Zip),
                &mut quiet,
            )
            .unwrap();
        tools
            .add(&plain_zip, &[more.join("late.txt")], None, &mut quiet)
            .unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_archive_is_converted_through_a_folder_that_goes_away() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("convert");
        let src = tree(&dir);
        let out = dir.join("out");
        std::fs::create_dir_all(&out).unwrap();
        let tar = out.join("from.tar.zst");
        tools
            .create(&sources(&src), &tar, &options(Format::TarZst), &mut quiet)
            .unwrap();
        let pw = secret(&dir, "pw", "converted");
        let mut o = options(Format::Zip);
        o.secret = Some(pw.clone());
        let zip = out.join("to.zip");
        tools.convert(&tar, None, &zip, &o, &mut quiet).unwrap();
        let into = dir.join("into");
        tools
            .extract(&zip, &into, &[], Some(&pw), &mut quiet)
            .unwrap();
        assert_eq!(files(&into), files(&src));
        let left: Vec<_> = std::fs::read_dir(&out)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left.len(), 2, "only the two archives: {left:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn nothing_is_overwritten_and_nothing_damaged_is_trusted() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("safe");
        let src = tree(&dir);
        let archive = dir.join("a.zip");
        tools
            .create(&sources(&src), &archive, &options(Format::Zip), &mut quiet)
            .unwrap();
        assert_eq!(
            tools
                .create(&sources(&src), &archive, &options(Format::Zip), &mut quiet)
                .unwrap_err()
                .why,
            Why::Exists
        );
        let into = dir.join("into");
        std::fs::create_dir_all(&into).unwrap();
        std::fs::write(into.join("plain.txt"), "the person's own").unwrap();
        tools
            .extract(&archive, &into, &[], None, &mut quiet)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(into.join("plain.txt")).unwrap(),
            "the person's own"
        );
        assert_eq!(
            std::fs::read_to_string(into.join("plain (copy 1).txt")).unwrap(),
            "plain text\n"
        );

        let junk: Vec<u8> = (0u16..4000).map(|i| (i * 7 + 3) as u8).collect();
        for name in ["junk.zip", "junk.7z", "junk.tar.gz", "junk.rar"] {
            let path = dir.join(name);
            std::fs::write(&path, &junk).unwrap();
            let why = tools.list(&path, None).unwrap_err().why;
            assert_eq!(why, Why::Corrupt, "{name}");
            let into = dir.join(format!("into-{name}"));
            assert_eq!(
                tools
                    .extract(&path, &into, &[], None, &mut quiet)
                    .unwrap_err()
                    .why,
                Why::Corrupt,
                "{name}"
            );
            assert!(files(&into).is_empty());
        }
        let whole = std::fs::read(&archive).unwrap();
        let cut = dir.join("cut.zip");
        std::fs::write(&cut, &whole[..whole.len() / 2]).unwrap();
        assert_eq!(tools.test(&cut, None).unwrap_err().why, Why::Corrupt);

        // Sources from two folders are refused, not flattened.
        assert_eq!(
            tools
                .create(
                    &[src.join("plain.txt"), src.join("sp ace/empty")],
                    &dir.join("two.zip"),
                    &options(Format::Zip),
                    &mut quiet
                )
                .unwrap_err()
                .why,
            Why::Unsupported
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_name_says_its_format_and_its_program() {
        let of = |name: &str| Format::of_path(Path::new(name));
        assert_eq!(of("/x/Backup.TAR.ZST"), Format::TarZst);
        assert_eq!(of("photos.tgz"), Format::TarGz);
        assert_eq!(of("a.7z"), Format::SevenZ);
        assert_eq!(of("film.rar").reader(), Tool::Unrar);
        assert_eq!(of("old.tar").reader(), Tool::Bsdtar);
        assert_eq!(of("disk.iso").reader(), Tool::SevenZz);
        assert_eq!(of("README"), Format::Other);
        assert!(Format::Zip.editable() && !Format::TarXz.editable());
        assert!(!Format::Rar.creatable());
        assert_eq!(Level::Ultra.number(), 9);
        assert_eq!(Level::Normal.for_filter("zstd"), Some(3));
        assert_eq!(Level::Store.for_filter("gzip"), Some(0));
        assert_eq!(Level::Store.for_filter("lz4"), Some(1));
        assert_eq!(Level::Ultra.for_filter("lz4"), Some(9));
        assert_eq!(Format::TarXz.filter(), Some(("--xz", "xz")));
        let ids: std::collections::BTreeSet<_> = Why::ALL.iter().map(|w| w.id()).collect();
        assert_eq!(ids.len(), Why::ALL.len());

        let nothing = Tools::default();
        let r = nothing.list(Path::new("a.zip"), None).unwrap_err();
        assert_eq!((r.why, r.tool), (Why::NoTool, Some(Tool::SevenZz)));
        assert_eq!(nothing.program(Tool::Unrar).unwrap_err().why, Why::NoTool);
        // 7zz without setsid is refused: a question could reach a terminal.
        let bare = Tools {
            sevenzz: Some(PathBuf::from("/nix/store/x-7zz/bin/7zz")),
            ..Tools::default()
        };
        assert_eq!(
            bare.list(Path::new("a.zip"), None).unwrap_err().why,
            Why::NoTool
        );
        assert_eq!(literal_pattern("a*b?[c]\\"), "a\\*b\\?\\[c]\\\\");
    }

    #[test]
    fn a_format_a_level_and_a_refusal_cross_the_wire_by_their_ids() {
        for f in Format::ALL {
            let said = serde_json::to_string(&f).unwrap();
            assert_eq!(said, format!("\"{}\"", f.id()), "{f:?}");
            assert_eq!(serde_json::from_str::<Format>(&said).unwrap(), f);
            assert_eq!(Format::of_id(f.id()), Some(f));
        }
        for l in Level::ALL {
            assert_eq!(
                serde_json::to_string(&l).unwrap(),
                format!("\"{}\"", l.id())
            );
        }
        for w in Why::ALL {
            assert_eq!(
                serde_json::to_string(&w).unwrap(),
                format!("\"{}\"", w.id())
            );
        }
        assert!(
            serde_json::from_str::<Format>("\"zipx\"").is_err(),
            "an unknown word is refused"
        );
        assert_eq!(Format::of_id("tar.gz"), None, "the id, not the extension");
        assert_eq!(
            Format::creatable_ones(),
            [
                Format::Zip,
                Format::SevenZ,
                Format::TarZst,
                Format::TarXz,
                Format::TarGz,
                Format::TarBz2,
                Format::TarLz4,
                Format::Tar
            ]
        );
        assert!(Why::NeedsPassword.wants_a_password() && !Why::Corrupt.wants_a_password());
    }

    /// A stand-in "7zz" that takes `seconds` and says nothing.
    fn slow(dir: &Path, seconds: &str) -> PathBuf {
        let spy = dir.join("slow");
        std::fs::write(&spy, format!("#!/bin/sh\nsleep {seconds}\n")).unwrap();
        std::fs::set_permissions(&spy, std::fs::Permissions::from_mode(0o700)).unwrap();
        spy
    }

    #[test]
    fn a_stop_kills_the_program_and_the_run_is_stopped() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("stop");
        let src = tree(&dir);
        let asked = std::time::Instant::now();
        let stand_in = Tools {
            sevenzz: Some(slow(&dir, "30")),
            ..tools
        }
        .held_by(Switch::new(move || {
            if asked.elapsed() > std::time::Duration::from_millis(300) {
                Hold::Stop
            } else {
                Hold::Go
            }
        }));
        let target = dir.join("out.zip");
        let r = stand_in
            .create(&sources(&src), &target, &options(Format::Zip), &mut quiet)
            .unwrap_err();
        assert_eq!(r.why, Why::Stopped);
        assert!(
            asked.elapsed() < std::time::Duration::from_secs(10),
            "the program was waited for, not killed: {:?}",
            asked.elapsed()
        );
        assert!(!target.exists(), "what a stopped run wrote is removed");
        // AND A SWITCH THAT ALREADY SAYS STOP STARTS NOTHING.
        let never = Tools {
            sevenzz: Some(slow(&dir, "30")),
            ..Tools::from_env()
        }
        .held_by(Switch::new(|| Hold::Stop));
        let started = std::time::Instant::now();
        assert_eq!(
            never
                .create(&sources(&src), &target, &options(Format::Zip), &mut quiet)
                .unwrap_err()
                .why,
            Why::Stopped
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_hold_holds_the_program_and_going_on_lets_it_finish() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("hold");
        let src = tree(&dir);
        // HELD FOR A SECOND AND A HALF, a program that needs a third of one:
        // it ends after the hold and not before, so it was stopped — and it
        // ENDS, so it was let go.
        let asked = std::time::Instant::now();
        let stand_in = Tools {
            sevenzz: Some(slow(&dir, "0.3")),
            ..tools
        }
        .held_by(Switch::new(move || {
            if asked.elapsed() < std::time::Duration::from_millis(1500) {
                Hold::Pause
            } else {
                Hold::Go
            }
        }));
        stand_in
            .create(
                &sources(&src),
                &dir.join("held.zip"),
                &options(Format::Zip),
                &mut quiet,
            )
            .unwrap();
        assert!(
            asked.elapsed() >= std::time::Duration::from_millis(1500),
            "the program finished while it was held: {:?}",
            asked.elapsed()
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a folder streamed as a tar comes back other than it went —
    /// a name read as a switch or an archive (`-Cx.txt`, `@types`), a file
    /// lost — or under another name than the folder's own; or if a name that
    /// is no folder inside another is streamed at all.
    #[test]
    fn a_folder_streams_as_a_tar_that_extracts_to_itself() {
        let _one = one_at_a_time();
        let tools = tools();
        let dir = place("stream");
        let src = tree(&dir);
        let mut child = tools.tar_stream(&src).unwrap();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut child.stdout.take().unwrap(), &mut bytes).unwrap();
        assert!(child.wait().unwrap().success());
        let tar = dir.join("streamed.tar");
        std::fs::write(&tar, &bytes).unwrap();
        let out = dir.join("out");
        tools.extract(&tar, &out, &[], None, &mut quiet).unwrap();
        assert_eq!(files(&out.join("src")), files(&src));
        assert!(
            out.join("src/sp ace/empty").is_dir(),
            "an empty folder was lost"
        );
        for nowhere in [Path::new("/"), Path::new("/home/..")] {
            assert_eq!(
                tools.tar_stream(nowhere).unwrap_err().why,
                Why::Unsupported,
                "{}",
                nowhere.display()
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}

// Declared LAST, after the tests: the checks cut a file at its first
// `#[cfg(test)] mod`, and everything after it is not the product.
#[cfg(test)]
mod fixtures;
