//! The boundary between buffer and host — sections 7.8 and 7.9.
//!
//! A native buffer is a WASM component. It **does not draw**: it produces a
//! [`DisplayList`], which the host rasterises. That separation is what makes a
//! buffer testable without a GPU, without Wayland and without a compositor — run the component,
//! compare the list.
//!
//! # Invariant 11, in the form of types
//!
//! "WASM buffers do not touch Wayland, do not receive token values, do not
//! translate." All three become properties of this crate's types:
//!
//! - There is nothing of Wayland here, and the buffer has no other way out.
//! - [`Color`] carries the token's **name**, never `0xRRGGBB`. What resolves it is the
//!   host. A buffer cannot invent a colour because there is no
//!   constructor that accepts a value.
//! - [`Text`] distinguishes **data** from **label**: data comes out literal (a glyph's
//!   name, a file's content), a label comes out as a key and the host
//!   translates. A buffer cannot write a UI sentence in Portuguese.
//!
//! # Coordinates in cells
//!
//! Everything in cells, never in pixels. The cell is exact (0.600 at, section 4.1),
//! so the buffer positions by integer arithmetic and does not need to measure
//! text — nor could it, without the font.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

/// A colour token, **by name**.
///
/// Never a value. Invariant 9 puts the colours in the tokens and 11 forbids the
/// buffer from receiving the values; the two together mean that what
/// crosses the boundary is the name, and the host resolves it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color(pub Cow<'static, str>);

impl Color {
    /// A colour token name in WIT's format, like `text-primary`.
    ///
    /// `kebab-case` and not `text.primary`: it is the name of the generated enum's case,
    /// and it is the same on both sides of the boundary.
    #[must_use]
    pub const fn name(n: &'static str) -> Self {
        Self(Cow::Borrowed(n))
    }

    /// The name, for the host to resolve.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What the buffer ASKS to happen, returned by `event`.
///
/// `key` used to return `bool`: the buffer said "I changed" and the host guessed
/// the rest. With effects it asks — copy, fire a named command,
/// close — and the host does not have to assume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// The internal state changed; redraw.
    Redraw,
    /// Put this on the clipboard.
    Copy(String),
    /// Run the named command. **The registry's id**, never a function: section
    /// 13 requires everything to be addressable by name.
    Command(String),
    /// Close this buffer.
    Close,
    /// Write this text to the file the buffer was opened with.
    ///
    /// # Why not `Copy`
    ///
    /// `Copy` goes to the clipboard; saving goes to disk. Reusing it would put
    /// the file's whole content on the clipboard as a side effect of saving —
    /// a surprise nobody would connect to the key they pressed.
    ///
    /// The buffer still has NO filesystem (invariant 11): it hands the text
    /// over and the host writes, exactly as [`Input::Paste`] is how text gets
    /// in. That is what lets a file editor exist without a new capability.
    Save(String),
    /// Ask the host to do something to the machine's memory or disk.
    ///
    /// Requires [`Capability::System`]. Not [`Effect::Command`]: that one is a
    /// registry id and carries no argument, and "close pid 4321" needs one.
    Ask(Demand),
    /// Write a filesystem. Needs [`Capability::Disks`].
    ///
    /// THE ONE EFFECT THAT DESTROYS SOMETHING NOBODY GETS BACK, and it is not
    /// folded into [`Effect::Ask`] for that reason: the reader should see it
    /// named, and the capability gate should have one arm to point at.
    Format(Plan),
    /// "I am in this state now", after changing on my own.
    ///
    /// Typing a letter into the icon browser enters its search, and nobody
    /// pressed a key that means "enter search" — so the host has no way to know.
    /// Without this the modeline would be truthful only about the changes the
    /// host itself caused, which is the same class of lie as a stale branch
    /// name and worse, because this one goes wrong every time somebody types.
    ///
    /// The id has to be one the descriptor declared; the host refuses any other
    /// rather than drawing a word no key can return from.
    StateChanged(String),
    /// Remember this preference. Needs [`Capability::Brain`].
    ///
    /// NOT an [`Effect::Ask`]: that one is a verb plus an index and carries no
    /// value, and "every seven days" is a value. See [`Setting`].
    ///
    /// The buffer still touches no disk (invariant 11): it says what it wants
    /// remembered and the HOST writes the file, the same way [`Effect::Save`]
    /// is how a file editor writes.
    Tune(Setting),
}

impl Effect {
    /// The capability this effect requires, when it requires one.
    ///
    /// The twin of [`Command::requires`], and for the same reason: the host has
    /// to refuse BEFORE acting, and a second table of what costs what is a
    /// second table to keep in step.
    ///
    /// # Exhaustive, and it was not
    ///
    /// This ended in `_ => None` until the brain arrived, which is the same
    /// defect [`Request::requires`] carried one shade louder — and quieter here
    /// is worse. A request added without an arm was silently granted the system
    /// capability; an EFFECT added without an arm was silently granted the
    /// right to need NO capability at all, which is the widest answer there is.
    ///
    /// Every variant is named. Adding one is a compile error until somebody
    /// decides whether it is gated, which is the decision this function exists
    /// to force.
    #[must_use]
    pub const fn requires(&self) -> Option<Capability> {
        match self {
            Self::Ask(d) => Some(d.what.requires()),
            // A PREFERENCE IS GATED LIKE A REQUEST, and behind the same
            // capability the requests it belongs with are: what is being
            // remembered here is when the machine rebuilds its own operating
            // system, and a buffer that asked for nothing must not get to
            // change that quietly.
            Self::Tune(_) => Some(Capability::Brain),
            // AND THESE COST NOTHING, each for a reason that is worth being
            // able to check rather than assume: redrawing, closing and saying
            // which state one is in are the buffer talking about ITSELF;
            // copying and saving are the host's own doors, already gated where
            // they act; and a command id is checked against the registry, which
            // is a narrower gate than a capability.
            Self::Redraw
            | Self::Copy(_)
            | Self::Command(_)
            | Self::Close
            | Self::Save(_)
            | Self::StateChanged(_) => None,
            // `Format` is gated by `Capability::Disks` at the point it acts —
            // see `Host::format`. Named here so the list is complete, not to
            // change where that gate lives.
            Self::Format(_) => Some(Capability::Disks),
        }
    }
}

/// A capability the buffer ASKS for in the descriptor.
///
/// The host refuses whoever uses one without asking. The list is short on purpose:
/// each is surface that every future buffer gets to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// The PTY and the VT state machine, which live in the host.
    Terminal,
    /// An offscreen texture, for whoever needs pixels. "Rare and auditable".
    RawSurface,
    /// Emitting [`Command::Svg`] — a vector drawing rasterised by the host.
    ///
    /// Less than [`Capability::RawSurface`]: there is no texture and no pixel
    /// coming from the guest. It is the difference between "the buffer sends pixels" and "the
    /// buffer sends a figure".
    Vector,
    /// Receiving [`Input::Survey`] and emitting [`Effect::Ask`].
    ///
    /// The buffer still reads nothing: the host samples `/proc`, the cgroups and
    /// systemd, and the component receives numbers that are already numbers. What
    /// the capability opens is the CHANNEL, not the access — invariant 11 stands.
    System,
    /// Receiving [`Input::Queue`] and asking for a job to be paused, resumed,
    /// cancelled or forgotten.
    ///
    /// Separate from [`Capability::System`] because they are different powers
    /// over different things: one closes processes and sweeps the store, the
    /// other touches a queue the host owns. A buffer that wants both asks for
    /// both.
    Transfers,
    /// Receiving [`Input::Rack`] and asking for a device to be formatted.
    ///
    /// The heaviest of the five, and the reason it is not folded into
    /// [`Capability::System`]: that one ends processes and sweeps the store,
    /// which is recoverable. This one writes a filesystem over a disk. A buffer
    /// asks for exactly the power it uses.
    Disks,
    /// Receiving [`Input::Mind`], and asking the brain to run something or to
    /// remember a preference.
    ///
    /// A fourth one and not [`Capability::System`], because `System` means "how
    /// much memory, who is eating it, do something about it" and this one ends
    /// with the machine REBUILDING ITS OWN OPERATING SYSTEM. Folding it in
    /// would hand that reach to the memory panel, which asked for neither — the
    /// same argument [`Capability::Transfers`] and [`Capability::Disks`] make,
    /// one step further along.
    ///
    /// It opens no filesystem, no socket, no clock and no parser: the whole
    /// mind arrives already read, already decided and already formatted —
    /// including the DATES, which the component cannot compute, having no zone.
    Brain,
}

// ---------------------------------------------------------------------------
// The system's memory — behind [`Capability::System`].
//
// THE FIELD ORDER OF EVERY STRUCT BELOW IS THE WIT'S ORDER, and it is not a
// matter of taste: `wasmtime` lowers a `Val::Record` by zipping it POSITIONALLY
// against the WIT's fields and only then compares the names. A field moved here
// without being moved there fails when the function is CALLED, with a message
// about a name — not at compile time. See `ronoctua-runtime`'s `val_of_survey`,
// which is where the two orders meet.
// ---------------------------------------------------------------------------

/// What a number measures. An enum, and never a free string.
///
/// The same lesson [`Color`] paid for: a typo in a string compiles and fails in
/// silence. And a buffer has no JSON parser — no field carries raw characters
/// for the guest to take apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gauge {
    /// `/proc/meminfo`: `MemTotal`.
    RamTotal,
    /// `MemAvailable` — what a new process could actually get, which is not
    /// `MemFree` and is the number that says whether the machine is in trouble.
    RamAvailable,
    /// `MemFree`.
    RamFree,
    /// `Cached`.
    RamCached,
    /// `Buffers`.
    RamBuffers,
    /// `Shmem` — tmpfs and shared memory, which look like cache and are not
    /// reclaimable.
    RamShmem,
    /// `SReclaimable`.
    RamSlabReclaimable,
    /// `Dirty` — what is on its way to disk, and the shape of a stall that is
    /// about to happen.
    RamDirty,
    /// `AnonPages`.
    RamAnonymous,
    /// `KernelStack` + `PageTables` + `VmallocUsed`.
    RamKernel,
    /// `/proc/swaps`, total of every device.
    SwapTotal,
    /// Total minus used.
    SwapFree,
    /// `/sys/block/zram0/mm_stat`: what was written, uncompressed.
    ZramOriginal,
    /// What it occupies after compression — the pair is what makes the ratio a
    /// measurement instead of the nominal number in the module.
    ZramCompressed,
    /// The ceiling zram may take from RAM.
    ZramLimit,
    /// The desktop's floor, EFFECTIVE — after every ancestor capped it.
    DesktopMin,
    /// `memory.current` of `session.slice`.
    DesktopCurrent,
    /// `memory.current` of `app.slice`.
    AppCurrent,
    /// `memory.high` of `app.slice`, resolved to bytes by systemd.
    AppCeiling,
    /// `memory.current` of `system.slice`.
    SystemCurrent,
    /// Graphics memory, when the driver reports it. On an APU it comes out of
    /// the same RAM, which is why it belongs here.
    VramTotal,
    /// How much of it is taken.
    VramUsed,
    /// The filesystem holding the store.
    DiskTotal,
    /// Free bytes on it.
    ///
    /// The store, the journal and the caches are NOT gauges: each needs a
    /// process spawned or a walk, so they arrive as [`Consumer`]s of a scan. A
    /// gauge nothing fills is a row that never appears.
    DiskFree,
}

/// One measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// What it measures.
    pub of: Gauge,
    /// How many bytes.
    pub bytes: u64,
}

/// Pressure (PSI) in PER MILLE: `12.3%` is `123`.
///
/// Integer and not float because per mille is exactly what the screen shows,
/// and because a rounding that happens in one place is one that can be pointed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pressure {
    /// `some`, ten-second window.
    pub some10: u16,
    /// `some`, one minute.
    pub some60: u16,
    /// `some`, five minutes.
    pub some300: u16,
    /// `full`, ten seconds — everyone stalled at once.
    pub full10: u16,
    /// `full`, one minute.
    pub full60: u16,
}

/// What kind of thing is eating memory or disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumerKind {
    /// A process, by pid.
    Process,
    /// One of our scopes — one per foreign buffer, which is what makes the
    /// answer "which BUFFER" and not only "which process" (section 7.5).
    Scope,
    /// A directory a scan walked.
    Directory,
    /// A mounted filesystem.
    Filesystem,
    /// A NixOS generation.
    Generation,
}

/// Someone a scan found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consumer {
    /// Process, scope, directory, filesystem or generation.
    pub kind: ConsumerKind,
    /// As the system spells it. Data, never a label.
    pub name: String,
    /// How many bytes.
    pub bytes: u64,
    /// The pid when there is one, `0` otherwise.
    pub hint: u64,
}

/// Which policy is in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuardMode {
    /// systemd answered something we do not recognise. It is a state to SHOW,
    /// not to hide behind a plausible default.
    #[default]
    Unknown,
    /// Nothing is armed.
    Off,
    /// The flake's numbers.
    Strict,
    /// The loosened ones.
    Tolerant,
}

/// The protection, as systemd reports it right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Guard {
    /// What is in force NOW — never what the flake asked for. The two diverge
    /// exactly when the interesting thing happened.
    pub mode: GuardMode,
    /// Per mille of a ten-second window spent stalled.
    pub pressure_limit: u16,
    /// For how long it has to stay there.
    pub pressure_seconds: u32,
    /// `MemoryHigh` on the applications' slice, in per mille of RAM.
    pub ceiling: u16,
    /// How many times it has acted since boot.
    pub acted: u32,
    /// Whether compressed swap is up.
    pub zram: bool,
    /// What the machine's own file says — what survives a reboot.
    pub persisted: GuardMode,
}

/// What the host samples and hands over.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Survey {
    /// A list, so a machine that has no zram simply does not report it.
    pub readings: Vec<Reading>,
    /// Stall, in per mille.
    pub pressure: Pressure,
    /// Empty until a scan is asked for.
    pub consumers: Vec<Consumer>,
    /// The protection's state.
    pub guard: Guard,
    /// A scan in flight.
    pub scanning: bool,
    /// How far it got, in per mille.
    pub progress: u16,
}

/// One disk the machine has, as the host last read it.
///
/// **THE FIELD ORDER IS THE WIT'S ORDER.** `wasmtime` zips a record
/// POSITIONALLY and only then compares names, so a field moved on one side
/// alone fails when the function is CALLED, with a message about a name. There
/// is a test that walks both shapes field by field.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Device {
    /// The id the host gave it — opaque here, a udisks object path there.
    ///
    /// Never drawn: that path embeds the drive's SERIAL NUMBER, and this
    /// repository is public (invariant 13).
    pub id: String,
    /// What to call it, already chosen by the host.
    pub name: String,
    /// `/dev/sda2`.
    pub node: String,
    /// `vfat`, `ntfs`, `btrfs` — or empty, which is a partition with nothing on
    /// it yet, and a REASON to format rather than a failure.
    pub kind: String,
    /// How big, in bytes. The buffer has no formatter.
    pub bytes: u64,
    /// Is a filesystem from it mounted right now?
    pub mounted: bool,
    /// Can the drive it sits on be pulled out?
    pub removable: bool,
    /// Why it must not be touched — the mount point or the word that decided,
    /// already resolved. Empty means it is spare.
    pub needed: String,
    /// Is this the whole disk rather than one partition of it?
    pub whole: bool,
}

/// Every disk the machine has.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rack {
    /// The devices, already filtered and already judged by the host.
    pub devices: Vec<Device>,
}

/// What became of the last [`Effect::Format`].
///
/// The same channel `Input::Copied` opened, and for the same reason: an effect
/// that acts on the machine has to answer, or the buffer can only show an
/// optimistic "done!" — which lies exactly when it matters.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    /// Did it work?
    pub done: bool,
    /// What went wrong, already a sentence — empty when [`Self::done`].
    pub trouble: String,
}

/// A filesystem this build can actually create.
///
/// Not "what exists": what the slimmed udisks carries a `mkfs` for. XFS is
/// deliberately absent, and the buffer says so rather than leaving a gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Recipe {
    /// vfat — opens on every computer there is.
    #[default]
    Universal,
    /// exfat — the same, without the four-gigabyte file limit.
    LargeFiles,
    /// ntfs — what Windows writes.
    Windows,
    /// ext4 — the ordinary Linux one.
    Linux,
    /// btrfs — snapshots and checksums.
    Modern,
}

impl Recipe {
    /// The name `mkfs` knows it by.
    ///
    /// The one place this enum meets a filesystem's real name, so the buffer
    /// never carries one — a component that knew the string `vfat` would be a
    /// component with an opinion about disks.
    #[must_use]
    pub const fn filesystem(self) -> &'static str {
        match self {
            Self::Universal => "vfat",
            Self::LargeFiles => "exfat",
            Self::Windows => "ntfs",
            Self::Linux => "ext4",
            Self::Modern => "btrfs",
        }
    }

    /// From the WIT's name. `None` for one this build does not know.
    #[must_use]
    pub fn of_wit_name(name: &str) -> Option<Self> {
        Some(match name {
            "universal" => Self::Universal,
            "large-files" => Self::LargeFiles,
            "windows" => Self::Windows,
            "linux" => Self::Linux,
            "modern" => Self::Modern,
            _ => return None,
        })
    }
}

/// What to write on a disk, and on which.
///
/// # Why an effect of its own and not another [`Request`]
///
/// Because a [`Demand`] is a verb plus an INDEX, and that fits every request
/// there is. Formatting is not that: it carries a filesystem, a name and
/// whether the partition table goes too. Bending `Demand` to hold them would
/// put three disk-shaped fields on the record every other buffer sends.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// The device, by the id the host gave in the last [`Rack`].
    ///
    /// Never one the buffer built: the host checks it against that list, and
    /// refuses one the running system needs whatever the buffer says.
    pub device: String,
    /// What to write.
    pub recipe: Recipe,
    /// What to call it. Empty means no name, which is ordinary.
    pub label: String,
    /// Wipe the partition table and lay down a single partition first.
    pub whole_disk: bool,
}

/// What the buffer asks the host to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Sample now instead of waiting for the tick.
    Refresh,
    /// Find who is eating RAM.
    ScanRam,
    /// Find who is eating disk.
    ScanDisk,
    /// Stop whichever scan is running.
    CancelScan,
    /// Loosen the policy for this session.
    GuardTolerant,
    /// Put it back.
    GuardDefault,
    /// Write the permanent option into the machine's own file.
    GuardPersist,
    /// Close what the target names.
    StopConsumer,
    /// Remove old system generations.
    SweepGenerations,
    /// Collect the store's garbage.
    SweepStore,
    /// Vacuum the journal.
    ///
    /// The user's cache is deliberately NOT a request: it is shown as a reading
    /// so it can be found, and emptying it in one key is a button whose damage
    /// varies by application.
    SweepJournal,

    /// Hold the job the target names.
    PauseJob,
    /// Let it run again.
    ResumeJob,
    /// Give it up. Irreversible: it deletes the piece already written at the
    /// destination.
    CancelJob,
    /// Drop a FINISHED job from the list. Touches no file.
    ForgetJob,
    /// Drop every finished job at once. The target is ignored.
    SweepJobs,

    /// Run the task the target names, for real.
    ///
    /// The target is a 1-BASED index into the last [`Mind`]'s task list and
    /// never a name the buffer built: the host checks it against the list it
    /// just sent, exactly as it does with a consumer and with a job.
    RunTask,
    /// The same, changing nothing — the `nix run .#install` idiom.
    RehearseTask,
    /// Stop what is running, or take it out of the queue.
    AbortTask,
    /// Do not wait the announcement out; start now. The target is ignored.
    Proceed,
    /// Not now — it comes back at its next moment. The target is ignored.
    Postpone,
}

impl Request {
    /// The capability this request needs.
    ///
    /// It is the request and NOT the effect that decides, because
    /// [`Effect::Ask`] carries every one of them and two capabilities now meet
    /// inside it. A `match` and not a table: a request added without an arm
    /// does not compile, where a table would let it through and the host would
    /// refuse it at runtime for the wrong reason.
    ///
    /// # And the arm that used to be here
    ///
    /// The paragraph above was written when this ended in `_ => System`, and
    /// the two contradicted each other in the same doc comment: with a
    /// catch-all, a request added without an arm compiles perfectly and is
    /// GRANTED the system capability — the widest one there is, and the one
    /// that reaches the machine's memory. The failure is silent, and it fails
    /// OPEN.
    ///
    /// Every variant is named now. Adding one is a compile error until
    /// somebody decides which capability it belongs behind, which is exactly
    /// the decision this function exists to force.
    #[must_use]
    pub const fn requires(self) -> Capability {
        match self {
            Self::PauseJob
            | Self::ResumeJob
            | Self::CancelJob
            | Self::ForgetJob
            | Self::SweepJobs => Capability::Transfers,
            Self::RunTask
            | Self::RehearseTask
            | Self::AbortTask
            | Self::Proceed
            | Self::Postpone => Capability::Brain,
            Self::Refresh
            | Self::ScanRam
            | Self::ScanDisk
            | Self::CancelScan
            | Self::GuardTolerant
            | Self::GuardDefault
            | Self::GuardPersist
            | Self::StopConsumer
            | Self::SweepGenerations
            | Self::SweepStore
            | Self::SweepJournal => Capability::System,
        }
    }
}

/// A request plus what it acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Demand {
    /// What to do.
    pub what: Request,
    /// Which consumer, 1-based into the last survey's list; `0` for none.
    ///
    /// An index and not a pid: a scope has no pid, and a scope — one per
    /// application — is what one actually wants to close.
    pub target: u64,
}

// ---------------------------------------------------------------------------
// The transfer queue — behind [`Capability::Transfers`].
//
// THE FIELD ORDER OF [`Job`] IS THE WIT'S ORDER, for the reason written above
// the survey's structs: `wasmtime` zips a record positionally and only then
// compares names.
// ---------------------------------------------------------------------------

/// Who is doing the moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// This machine's own disk.
    Local,
    /// A plain link, through aria2.
    Fetch,
    /// A site's media, through yt-dlp.
    Media,
    /// BitTorrent, through the session the queue owns.
    Torrent,
}

/// Where a job is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Finding out how much there is.
    Weighing,
    /// Moving.
    Running,
    /// Held.
    Paused,
    /// Finished, and it worked.
    Done,
    /// Finished, and it did not.
    Failed,
    /// Given up on.
    Cancelled,
}

/// What is being done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errand {
    /// Copy into a directory.
    Copy,
    /// Move into a directory.
    ///
    /// `MoveIt` and not `Move`: a bare `Move` reads as the verb in half the
    /// call sites and as a noun in the other half, and the WIT case has to
    /// avoid a word that several generators treat as a keyword.
    MoveIt,
    /// Remove, without the wastebasket.
    Delete,
    /// Overwrite before removing.
    Shred,
    /// Move to the wastebasket.
    Trash,
    /// Bring back from the wastebasket.
    Restore,
    /// Pack into an archive.
    Compress,
    /// Unpack an archive.
    Extract,
    /// Add up a directory's size.
    Measure,
    /// Download a plain link.
    Fetch,
    /// Download a site's video.
    Media,
    /// Download a site's audio only.
    Audio,
    /// Download a site's subtitles.
    Subtitles,
    /// Download a torrent.
    Torrent,
}

/// What `done` and `total` are counted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Amount {
    /// Bytes.
    Bytes,
    /// Whole items — files, entries.
    Items,
}

/// One line of the queue, as the buffer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// What the buffer names when it asks for something. Never a row number:
    /// the list reorders itself as jobs finish.
    pub id: u64,
    /// Who is moving it.
    pub engine: Engine,
    /// What is being done.
    pub errand: Errand,
    /// Where it is in its life.
    pub stage: Stage,
    /// What `done` and `total` count.
    pub amount: Amount,
    /// How much is done.
    pub done: u64,
    /// How much there is in total. **Zero means not known** — a download with
    /// no `Content-Length`, a torrent whose metadata has not arrived. The
    /// buffer shows the stage instead of a proportion.
    pub total: u64,
    /// What it is working on right now. Data, and already cut by the host: the
    /// buffer has no path type and no idea what a separator is.
    pub subject: String,
    /// A failure, already translated by the host. Empty when there is none.
    pub trouble: String,
    /// What a [`Errand::Measure`] answered, in bytes. Zero when there is none.
    pub answer: u64,
    /// Whether it went out as the PERSON rather than as nobody.
    ///
    /// The one property of a job that cannot be undone after the fact: an
    /// anonymous download can be repeated as oneself, and the reverse is not
    /// true — the request already carried who was asking.
    pub identified: bool,
}

impl Job {
    /// How far along, in **per mille**, or `None` when the total is not known.
    ///
    /// Per mille and not per cent for the same reason the queue's own model
    /// uses it: a bar twelve cells wide has more than a hundred distinguishable
    /// positions, and integer per cent makes it stutter.
    #[must_use]
    pub const fn along(&self) -> Option<u16> {
        if self.total == 0 {
            return None;
        }
        let done = if self.done > self.total {
            self.total
        } else {
            self.done
        };
        #[allow(clippy::cast_possible_truncation)]
        Some((done as u128 * 1000 / self.total as u128) as u16)
    }
}

/// Every line, as the host last read them.
///
/// The WHOLE list and never a delta: a buffer that had to apply deltas would
/// need to hold state that can go out of step with the host's, and the list is
/// short enough that sending it whole costs less than being wrong.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Queue {
    /// The lines, in the order they are to be drawn.
    pub jobs: Vec<Job>,
}

// ---------------------------------------------------------------------------
// THE BRAIN — what the machine does by itself, behind [`Capability::Brain`].
//
// THE FIELD ORDER OF EVERY STRUCT BELOW IS THE WIT'S ORDER, for the reason
// written above the survey's structs and repeated in the `.wit` itself:
// `wasmtime` zips a record POSITIONALLY against the WIT's fields and only then
// compares the names. A field moved here without being moved there fails when
// the function is CALLED, with a message about a name — never at compile time.
// See `ronoctua-runtime`'s `val_of_mind`, which is where the two orders meet.
// ---------------------------------------------------------------------------

/// What a reading of the [`Pulse`] measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vital {
    /// RAM in use, in PER CENT of the total.
    RamUsed,
    /// Stall waiting for memory, in per mille of a ten-second window.
    MemoryStall,
    /// Stall waiting for a processor, in the same unit.
    CpuStall,
    /// Running off the battery: `1` for yes, `0` for no.
    OnBattery,
}

/// One reading of the pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VitalSign {
    /// What it measures.
    pub of: Vital,
    /// How much, in the unit [`Vital`] names.
    pub amount: u16,
}

/// How busy the machine is, as the gate last read it.
///
/// A list and not a record with a field per reading, for the reason
/// [`Survey::readings`] is one: what the machine does not measure does not
/// report, and zero is a real reading. A kernel with no `/proc/pressure/cpu`
/// simply has no [`Vital::CpuStall`] here, and the screen draws a dash instead
/// of a number nobody took.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pulse {
    /// When it was taken, in epoch seconds.
    pub at: u64,
    /// What was taken.
    pub signs: Vec<VitalSign>,
}

/// Where a task is right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Standing {
    /// Not running and not waiting to.
    #[default]
    Idle,
    /// Waiting its turn.
    Queued,
    /// Running.
    Running,
}

/// What is in the way.
///
/// [`HoldKind::None`] is not a failure being hidden: it is what a row says when
/// nothing is holding it, the same shape [`GuardMode::Unknown`] has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoldKind {
    /// Nothing is holding it.
    #[default]
    None,
    /// Not enough memory.
    Ram,
    /// The processor is too busy.
    Cpu,
    /// On the battery.
    Battery,
    /// Something else of ours is running.
    Busy,
    /// No network.
    Network,
    /// The machine is asleep.
    Asleep,
}

/// How a run ended.
///
/// [`OutcomeKind::Skipped`] is FIRST, and deliberately the harmless one: a
/// record built from a file that lost a field must not read as "it worked".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutcomeKind {
    /// It did not run — and that is not a failure.
    #[default]
    Skipped,
    /// It ran and it worked.
    Done,
    /// It ran and it did not.
    Failed,
}

/// Which stretch of the person's day a task prefers.
///
/// The hours each one covers are the brain's, with the reason written beside
/// them — the buffer has no clock and no zone (invariant 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DayPart {
    /// Whenever it comes due.
    #[default]
    Any,
    /// The small hours.
    Night,
    /// The morning.
    Morning,
    /// The afternoon.
    Afternoon,
    /// The evening.
    Evening,
}

impl DayPart {
    /// From the WIT's name. `None` for one this build does not know.
    ///
    /// The twin of [`Recipe::of_wit_name`], and for the same reason: a case the
    /// host has not learned to read is REFUSED rather than rounded to a
    /// neighbour, because rounding here would move a rebuild from the small
    /// hours into the middle of somebody's afternoon.
    #[must_use]
    pub fn of_wit_name(name: &str) -> Option<Self> {
        Some(match name {
            "any" => Self::Any,
            "night" => Self::Night,
            "morning" => Self::Morning,
            "afternoon" => Self::Afternoon,
            "evening" => Self::Evening,
            _ => return None,
        })
    }
}

/// What became of one step of a run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StepOutcome {
    /// Which step, by the id the task declared.
    pub id: String,
    /// Did it work?
    pub ok: bool,
    /// How long it took.
    pub seconds: u32,
    /// What the machine said. Empty when it said nothing.
    pub trouble: String,
}

/// One run, as the journal keeps it.
///
/// `RunRecord` and NOT `Run`: the layout vocabulary every component now imports
/// (`ronoctua-design`) already has a `Run` — one line of a screen — and a buffer
/// that imported both would resolve one of them silently. The name is longer at
/// the one place it is written and unambiguous everywhere it is read, which is
/// the same argument [`BufferState`] makes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunRecord {
    /// When it started, in epoch seconds.
    pub began: u64,
    /// When it ended.
    pub ended: u64,
    /// When it ran, ALREADY FORMATTED by the host, in ISO.
    ///
    /// The component has no zone and no calendar (invariant 11), so the number
    /// is for the arithmetic this side does and the text is for the date it
    /// cannot compute.
    pub when_text: String,
    /// How it ended.
    pub outcome: OutcomeKind,
    /// The step it failed at, by id. Empty when there is none.
    pub at: String,
    /// A WORD of the brain's own — `runaway`, `aborted`, `off` — drawn through
    /// `brain-why-<word>`. Empty when the reason came from the machine, in
    /// which case [`RunRecord::trouble`] carries it.
    ///
    /// They are two fields because they are drawn differently: one through the
    /// bundle, the other exactly as it arrived.
    pub why: String,
    /// What the machine said, verbatim. Empty when it said nothing.
    pub trouble: String,
    /// What became of each step.
    pub steps: Vec<StepOutcome>,
}

/// One thing a task runs, as the screen draws it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskStep {
    /// English, kebab-case, like every other name here (invariant 18).
    pub id: String,
    /// What it really runs. DATA — a command line is not translated.
    pub command: String,
    /// Whether it needs to be root.
    pub needs_root: bool,
    /// Whether it is on.
    pub on: bool,
    /// Whether that differs from what the distro ships.
    pub changed: bool,
}

/// One task, as the screen draws it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskState {
    /// English, kebab-case. `brain-task-<id>` names it on screen.
    pub id: String,
    /// Where it is right now.
    pub standing: Standing,
    /// Whether it runs on its own at all.
    pub enabled: bool,
    /// How often, in days.
    pub interval_days: u16,
    /// Which stretch of the day it prefers.
    pub part_of_day: DayPart,
    /// The next moment it will be considered, in epoch seconds. `0` when there
    /// is none — it is switched off, or it only runs when asked.
    pub next: u64,
    /// The same moment, already formatted. Empty when there is none.
    pub next_text: String,
    /// What is in the way.
    pub hold: HoldKind,
    /// Why it is not due at all, as a word. Empty when it is.
    pub why_not: String,
    /// Whether [`TaskState::last`] means anything.
    ///
    /// A record cannot be absent across the boundary, and `began: 0` would be a
    /// moment in 1970 — which is a date the screen would draw.
    pub ran: bool,
    /// The last run. Meaningless unless [`TaskState::ran`].
    pub last: RunRecord,
    /// What it runs.
    pub steps: Vec<TaskStep>,
}

/// The warning that something is about to happen.
///
/// [`Announcement::task`] EMPTY means there is none — the same shape
/// [`Mind::running`] uses. What crosses is the DEADLINE and never a ticking
/// number: whoever draws already beats a one-second clock, and a daemon that
/// pushed a digit every second would wake three processes to redraw one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Announcement {
    /// Which task, by id. Empty when there is no announcement.
    pub task: String,
    /// How long the warning lasts in total, in seconds.
    pub total_seconds: u16,
    /// When it runs out, in epoch seconds.
    pub deadline: u64,
    /// Why it is happening, as a word.
    pub why: String,
}

/// How busy is too busy — the general gate, as the person set it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gate {
    /// The most RAM that may be in use, in per cent.
    pub ram_ceiling_pct: u16,
    /// The most processor stall that may be going on, in per mille.
    pub cpu_stall_per_mille: u16,
    /// Whether running off the battery stops it.
    pub abort_on_battery: bool,
}

/// Everything the brain knows, all at once.
///
/// Whole and never a delta, for the reason [`Queue`] is whole: a delta needs
/// both sides to agree on what they last saw, and what is being agreed about
/// here is when the machine will rebuild itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mind {
    /// The daemon's clock, in epoch seconds.
    ///
    /// It crosses because the component has none (invariant 11): a countdown is
    /// `deadline - now`, and without the second number the first is a moment
    /// this side cannot place.
    pub now: u64,
    /// How busy the machine is.
    pub pulse: Pulse,
    /// Every task, in the order they are to be drawn.
    pub tasks: Vec<TaskState>,
    /// What is waiting its turn, by id.
    pub queue: Vec<String>,
    /// What is running, by id. Empty when nothing is.
    pub running: String,
    /// The task whose next run is a rehearsal. Empty when none is.
    pub rehearsing: String,
    /// The warning in flight.
    pub announcement: Announcement,
    /// The general gate.
    pub gate: Gate,
    /// The cadences one key steps through, in days.
    ///
    /// The LADDER crosses instead of living in the component, because which
    /// steps are worth having is a policy with its reason written in
    /// `ronoctua-brain` — and a screen that stepped by one would take
    /// twenty-three presses to get from a month to a week.
    pub cadence: Vec<u16>,
}

/// One step, switched on or off by the id the last [`Mind`] gave.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StepSwitch {
    /// Which step.
    pub id: String,
    /// Whether it is to run.
    pub on: bool,
}

/// WHICH preference, and its VALUE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Knob {
    /// How often, in days.
    IntervalDays(u16),
    /// Which stretch of the day.
    PartOfDay(DayPart),
    /// Whether it runs on its own at all.
    Enabled(bool),
    /// One step, on or off.
    Step(StepSwitch),
    /// Every step back to what the distro ships.
    ///
    /// It carries nothing: there is one default and it is the flake's.
    ResetSteps,
}

/// A preference the person changed.
///
/// # Why it is not a [`Demand`]
///
/// Because a `Demand` is a verb plus an INDEX, and that shape fits every
/// request there is: pause job seven, close consumer three. A preference is not
/// that — it carries a VALUE the person chose. Bending `Demand` to hold one
/// would put brain-shaped fields on the record every other buffer sends, which
/// is the argument [`Plan`] already carries a few hundred lines above.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// Which task it belongs to, by the id the last [`Mind`] gave.
    pub task: String,
    /// Which preference, and its value.
    pub knob: Knob,
}

/// A key binding declared by the buffer, as DATA (section 7.9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The sequence, like `m f s`.
    pub keys: String,
    /// The command's id.
    pub command: String,
}

/// The prefix under which a buffer MAY bind (section 7.9).
///
/// "Buffers only bind under `SPC m` (Doom's local leader). An attempt to
/// bind under a global prefix is **rejected at load with an error**." Without that, a
/// third-party component hijacks `SPC f` and the whole system changes
/// behaviour because of one buffer.
pub const LOCAL_PREFIX: &str = "m";

/// The id every buffer's BASE state carries.
///
/// A constant and not a convention: `Esc` returns to it, the modeline draws it
/// quiet while it draws every other state in the alternative colour, and both
/// of those need to know which one it is without asking the buffer.
pub const BASE_STATE: &str = "normal";

/// The tone a state is drawn in — a NAME, never a colour value (invariant 11).
///
/// [`Tone::Standard`] is what every buffer uses, and it means "the host
/// decides": [`BASE_STATE`] comes out quiet and everything else comes out in the
/// one token the system reserves for an alternative state. That token having a
/// name of its own is the whole point — a theme changes one line and every
/// buffer follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    /// Let the host choose by the id. What every buffer should use.
    #[default]
    Standard,
    /// The accent — the colour the system otherwise reserves for focus.
    Accent,
    /// Something is going well or is under way.
    Success,
    /// Something is waiting on the person.
    Warning,
    /// Something went wrong.
    Error,
}

/// One state a buffer can be in.
///
/// `BufferState` and not `State`: every component that has one calls its own
/// whole memory `State`, and two things by that name in one file is how a reader
/// stops knowing which is which.
///
/// The major mode says what KIND of buffer this is; the state says what it is
/// doing right now. The second existed before this type — it just had no name,
/// so typing a letter into the icon browser silently became a search while the
/// modeline went on saying the same word it said at rest.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BufferState {
    /// English, kebab-case (invariant 18). The screen reads `state-<id>`.
    pub id: String,
    /// The key that selects it directly under `SPC b s`.
    pub letter: char,
    /// How it is drawn.
    pub tone: Tone,
}

/// What the buffer says about itself in `init`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Descriptor {
    /// The buffer's name.
    pub name: String,
    /// The mode, in the sense of Emacs's major mode.
    pub mode: String,
    /// The states it can be in. At least one, and the first is the base.
    pub states: Vec<BufferState>,
    /// The keymap declared as data.
    pub keymap: Vec<Binding>,
    /// The command ids it registers.
    pub commands: Vec<String>,
    /// Segments it contributes to the modeline.
    pub segments: Vec<(String, Text)>,
    /// Capabilities asked for.
    pub capabilities: Vec<Capability>,
}

/// The size of a text RUN, by name and never in pixels.
///
/// Invariant 11 is explicit: a buffer does not receive a token value. It says
/// `Large` and the host decides how many pixels that is worth — the same contract colour
/// already has.
///
/// Per run and not per buffer: it used to be a descriptor field, and that is why the glyph
/// browser's header grew along with the icons. The **cell grid
/// is still the normal size's** — a large run occupies more than one
/// cell, and what spaces it is whatever draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scale {
    /// Smaller than the body. For whatever **accompanies** the content without competing
    /// with it — the focused glyph's tags and the results counter.
    Small,
    /// The SYSTEM's size: the same as the footer's and the minibuffer's.
    ///
    /// For whatever is **interface** inside a buffer, and not its content —
    /// today the browser's search field, which is the same thing as the
    /// minibuffer and had no reason to have a different size.
    Base,
    /// The buffers' body text.
    #[default]
    Normal,
    /// Larger, for content that **is** the shape — the glyph in the browser.
    Large,
    /// The subject of the whole screen — the glyph on the sheet.
    Huge,
}

impl Descriptor {
    /// Refuses a descriptor that binds outside `SPC m`.
    ///
    /// # Errors
    ///
    /// Some binding leaves the local prefix. The message says which.
    pub fn validate(&self) -> Result<(), String> {
        self.states_are_sound()?;
        for l in &self.keymap {
            let first = l.keys.split_whitespace().next().unwrap_or("");
            if first != LOCAL_PREFIX {
                return Err(format!(
                    "the buffer `{}` tries to bind `{}`, outside `{LOCAL_PREFIX}`.\n\
                     Buffers only bind under the local partition (section 7.9); a \
                     global prefix would hijack the key for the whole system.",
                    self.name, l.keys
                ));
            }
        }
        Ok(())
    }

    /// The state list's three guarantees, refused AT LOAD.
    ///
    /// Same place as the keymap's prefix guard and for the same reason
    /// (section 7.9): each of these is a way for a buffer to put a word on the
    /// modeline that no key can get out of.
    ///
    /// - **Empty**: the segment would show nothing and `Esc` would have nowhere
    ///   to return to.
    /// - **A base that is not [`BASE_STATE`]**: the modeline decides "quiet or
    ///   alternative" by comparing against that id, so a buffer whose base is
    ///   called something else would draw itself as permanently not-normal.
    /// - **A repeated id or letter**: two states on one letter is a menu with an
    ///   unreachable entry, and two states on one id is a rotation that cannot
    ///   say where it stopped.
    fn states_are_sound(&self) -> Result<(), String> {
        let Some(base) = self.states.first() else {
            return Err(format!(
                "the buffer `{}` declares no state at all.\n\
                 Every buffer has at least `{BASE_STATE}`: it is where `Esc` goes back to.",
                self.name
            ));
        };
        if base.id != BASE_STATE {
            return Err(format!(
                "the buffer `{}` starts in `{}`, and the base state has to be `{BASE_STATE}`.\n\
                 And it is by that id that the modeline decides between the discreet tone and the \
                 other one, and where `Esc` goes back to.",
                self.name, base.id
            ));
        }
        for (n, s) in self.states.iter().enumerate() {
            if self.states[..n].iter().any(|o| o.id == s.id) {
                return Err(format!(
                    "the buffer `{}` declares `{}` twice.",
                    self.name, s.id
                ));
            }
            if self.states[..n].iter().any(|o| o.letter == s.letter) {
                return Err(format!(
                    "the buffer `{}` uses `{}` for two states; one of them would be \
                     inalcancavel sob `SPC b s`.",
                    self.name, s.letter
                ));
            }
        }
        Ok(())
    }
}

/// Text that crosses the boundary.
///
/// The distinction exists because invariants 8 and 11 meet here: **the
/// buffer does not translate**, but it also cannot send everything as a key — a glyph's
/// name or a file's content is not UI text and has no
/// translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Text {
    /// The buffer's data. It goes out as it is.
    Data(String),
    /// An interface label. The host resolves it through `ronoctua-i18n`.
    Key(String),
    /// An icon, by TOKEN name. The host resolves the codepoint.
    ///
    /// Without this, a buffer that wanted a magnifying glass would write the PUA
    /// character by hand — a codepoint that changes between Nerd Fonts versions,
    /// embedded in a component that has no way of knowing that (invariants 9,
    /// 10 and 11).
    Icon(String),
}

impl Text {
    /// Literal data.
    #[must_use]
    pub fn datum(s: impl Into<String>) -> Self {
        Self::Data(s.into())
    }

    /// A text key.
    #[must_use]
    pub fn key(s: impl Into<String>) -> Self {
        Self::Key(s.into())
    }

    /// An icon token's name.
    #[must_use]
    pub fn icon(s: impl Into<String>) -> Self {
        Self::Icon(s.into())
    }
}

/// A drawing command.
///
/// The list is deliberately SHORT. There is no filled rectangle, border,
/// shadow or radius: invariants 14 and 15 forbid cards and elevation, and
/// separation is by whitespace and a one-line rule. A rectangle
/// command here would be the way in for all of that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Text at a position, in cells.
    Text {
        /// The row, from the buffer's top.
        line: u16,
        /// The column, from the left.
        column: u16,
        /// What to write.
        text: Text,
        /// The colour token.
        color: Color,
        /// The size, by name. See [`Scale`].
        scale: Scale,
        /// How many cells the run may occupy. `0` = to the edge.
        ///
        /// The buffer declares the **budget**; what cuts is the host, which is what
        /// has the translated string.
        max_width: u16,
    },
    /// A ONE-line horizontal rule — the only separator that exists
    /// besides whitespace (section 5.1).
    Rule {
        /// On which row.
        line: u16,
        /// The colour token.
        color: Color,
    },
    /// A terminal grid — a FIRST-CLASS node (section 7.10).
    ///
    /// Not a `raw-surface`, on purpose: `raw-surface` would mean a
    /// second text rasteriser in the system, with its own font resolution and
    /// metrics, in the most-used buffer. Here the content arrives as
    /// already-formed lines and the host draws with the SAME font as everything else.
    Grid {
        /// The starting row, in cells.
        line: u16,
        /// The starting column, in cells.
        column: u16,
        /// One string per grid row, in order.
        lines: Vec<String>,
        /// The colour, by token name.
        color: Color,
    },
    /// The background of a list's focused row.
    ///
    /// **A change of colour, not a box and not an outline** (invariant 15). That is why
    /// there is no width and no radius: it paints the whole row, and that is all.
    Selection {
        /// On which row.
        line: u16,
        /// The colour token.
        color: Color,
    },
    /// A vector drawing, in the given rectangle of cells.
    ///
    /// **The first pixel primitive in this list.** It exists because Lucide's
    /// customisation changes the stroke's width, cap and colour, and a
    /// glyph honours none of that — the width is baked into the drawing.
    ///
    /// It requires [`Capability::Vector`]: it is the only primitive that produces arbitrary
    /// pixels, and the descriptor has to ask for it. See [`Descriptor::validate`].
    Svg {
        /// The starting row, in cells.
        line: u16,
        /// The starting column, in cells.
        column: u16,
        /// The rectangle's height, in cells.
        lines: u16,
        /// The rectangle's width, in cells.
        columns: u16,
        /// The document, as the copy would deliver it.
        svg: String,
        /// The token that replaces `currentColor`.
        ///
        /// The colour does **not** cross as a value (invariant 11), and without this
        /// Lucide's default `currentColor` would come out black over the buffer's
        /// background — an invisible drawing, indistinguishable from "not implemented".
        color: Color,
    },
}

impl Command {
    /// On which row the command draws. Used by the host for clipping.
    #[must_use]
    pub const fn line(&self) -> u16 {
        match self {
            Self::Text { line, .. }
            | Self::Rule { line, .. }
            | Self::Grid { line, .. }
            | Self::Selection { line, .. }
            | Self::Svg { line, .. } => *line,
        }
    }

    /// The capability this command requires, when it requires one.
    ///
    /// # Why the list belongs to the COMMAND, and not to the host
    ///
    /// Because the host has to refuse **at load**, before the buffer draws
    /// anything, and for that it needs to know what each primitive
    /// costs without consulting a second table. `Descriptor::validate` reads
    /// from here; a new command that needs a gate declares it here and gets the
    /// refusal for free.
    #[must_use]
    pub const fn requires(&self) -> Option<Capability> {
        match self {
            Self::Svg { .. } => Some(Capability::Vector),
            // THE OTHER FOUR COST NOTHING, and they are named rather than
            // swept up by a `_` for the reason the other two gates in this
            // file learned the hard way: `Request::requires` had a catch-all
            // under a doc comment promising exhaustiveness, and it granted the
            // SYSTEM capability to anything unlisted; `Effect::requires` had
            // the same shape and granted the right to need none at all.
            //
            // A drawing primitive is the cheapest of the three to get wrong
            // and the easiest to add — the pixel one this repository has
            // deliberately not built yet would land right here — so this is
            // where a new case has to stop and be decided.
            Self::Text { .. } | Self::Rule { .. } | Self::Grid { .. } | Self::Selection { .. } => {
                None
            }
        }
    }
}

/// What a buffer produces per frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisplayList {
    /// In drawing order: the first goes underneath.
    pub commands: Vec<Command>,
}

impl DisplayList {
    /// An empty list.
    #[must_use]
    pub fn fresh_one() -> Self {
        Self::default()
    }

    /// Adds a command.
    pub fn push(&mut self, c: Command) {
        self.commands.push(c);
    }

    /// Refuses a list that uses a primitive without the declared capability.
    ///
    /// # Why the refusal is here, and not in the descriptor
    ///
    /// Because the descriptor does not say what the buffer is going to **draw** — it says what
    /// the buffer asks for. WIT's promise (*"the host refuses whoever uses without having
    /// asked"*) is only kept by confronting the two, and the first `render`
    /// happens **before the window exists**: a buffer that emits
    /// [`Command::Svg`] without asking for [`Capability::Vector`] dies at startup, with
    /// the message saying what and why.
    ///
    /// Until now that line of the WIT was a promise: `validate` only checked the
    /// keymap's prefix, and no capability was required of anyone.
    ///
    /// # Errors
    ///
    /// Some command requires a capability the descriptor did not declare.
    pub fn validate(&self, capabilities: &[Capability]) -> Result<(), String> {
        for c in &self.commands {
            let Some(required) = c.requires() else {
                continue;
            };
            if !capabilities.contains(&required) {
                return Err(format!(
                    "the buffer draws {c:?} without declaring `{required:?}`.\n\
                     Every primitive with a gate is a surface every future buffer \
                     gains the right to use (section 7.6); whoever uses it asks for it \
                     in the descriptor, and the host checks before drawing."
                ));
            }
        }
        Ok(())
    }

    /// How many rows the list occupies.
    ///
    /// Zero when empty — not 1. The host uses this to size things, and a
    /// empty buffer must reserve no rows at all.
    #[must_use]
    pub fn height(&self) -> u16 {
        self.commands
            .iter()
            .map(|c| c.line().saturating_add(1))
            .max()
            .unwrap_or(0)
    }

    /// The rows a command claims: `(first row, how many)`.
    ///
    /// A run does NOT occupy the row it names — it occupies as many cells as
    /// its scale is worth, which is the whole reason [`Heights`] crosses the
    /// WIT. One place, so no caller has to remember that.
    fn rows_of(c: &Command, heights: Heights) -> (u16, u16) {
        match c {
            Command::Text { line, scale, .. } => (*line, heights.of(*scale).max(1)),
            Command::Rule { line, .. } | Command::Selection { line, .. } => (*line, 1),
            Command::Grid { line, lines, .. } => {
                (*line, u16::try_from(lines.len()).unwrap_or(u16::MAX).max(1))
            }
            Command::Svg { line, lines, .. } => (*line, (*lines).max(1)),
        }
    }

    /// The columns a command claims: `(first column, how many)`.
    ///
    /// # Why a KEY claims its whole budget and DATA does not
    ///
    /// Because they are two different promises. A `Text::Key` is a sentence the
    /// HOST will translate: the buffer does not know how long it comes out, so
    /// what it reserved is all anyone can go on — and a budget that reaches
    /// into the next field is a field that WILL be written over as soon as the
    /// language changes, which is exactly the fault that put an exact byte
    /// count on top of a sentence. A `Text::Data` is a string the buffer is
    /// holding: it knows the length, it usually positions the next field from
    /// that length, and the host cuts the run at whichever of the two is
    /// smaller. Charging it for a budget it demonstrably does not use would
    /// condemn every list that lets its last column run to the edge.
    ///
    /// The scale multiplies the WIDTH as well as the height: the cell is
    /// `0.600 em`, so a run at twice the body's size has twice the advance —
    /// and one glyph of `Large` covers the two cells [`Heights`] already
    /// charges it in the other direction.
    ///
    /// `Rule` and `Selection` paint the whole width by definition and have no
    /// column of their own.
    fn columns_of(c: &Command, heights: Heights, columns: u16) -> (u16, u16) {
        match c {
            Command::Text {
                column,
                text,
                scale,
                max_width,
                ..
            } => {
                let budget = if *max_width == 0 {
                    columns.saturating_sub(*column)
                } else {
                    *max_width
                };
                let wide = match text {
                    // NOTHING IS KNOWN about a key until the host looks it up.
                    Text::Key(_) => budget,
                    Text::Data(t) => u16::try_from(t.chars().count())
                        .unwrap_or(u16::MAX)
                        .saturating_mul(heights.of(*scale).max(1))
                        .min(budget),
                    Text::Icon(_) => heights.of(*scale).max(1).min(budget),
                };
                (*column, wide)
            }
            Command::Rule { .. } | Command::Selection { .. } => (0, columns),
            Command::Grid { column, lines, .. } => (
                *column,
                lines
                    .iter()
                    .map(|l| u16::try_from(l.chars().count()).unwrap_or(u16::MAX))
                    .max()
                    .unwrap_or(0),
            ),
            Command::Svg {
                column,
                columns: wide,
                ..
            } => (*column, *wide),
        }
    }

    /// A command in a few words, for a message somebody has to act on.
    ///
    /// The name of the key or the beginning of the data, because "two fields
    /// collide on row 13" is a report and not a diagnosis: what the person
    /// reading it needs is WHICH two, and every one of these commands is
    /// written in a different place in the buffer.
    fn sketch(c: &Command) -> String {
        match c {
            Command::Text { text, scale, .. } => match text {
                Text::Key(k) => format!("key `{k}` ({scale:?})"),
                Text::Data(t) => {
                    let mut short: String = t.chars().take(24).collect();
                    if t.chars().count() > 24 {
                        short.push('…');
                    }
                    format!("text `{short}` ({scale:?})")
                }
                Text::Icon(i) => format!("icon `{i}` ({scale:?})"),
            },
            Command::Rule { .. } => "rule".into(),
            Command::Selection { .. } => "selection".into(),
            Command::Grid { lines, .. } => format!("a grid of {} lines", lines.len()),
            Command::Svg { .. } => "svg".into(),
        }
    }

    /// Whether two `(first, how many)` spans touch.
    const fn meet(a: (u16, u16), b: (u16, u16)) -> bool {
        a.0 < b.0.saturating_add(b.1) && b.0 < a.0.saturating_add(a.1)
    }

    /// Refuses a frame where one command is drawn INSIDE another's box.
    ///
    /// # The fault this exists for
    ///
    /// A `Large` run is 40 px with a 58 px line box against the grid's 29, so
    /// it covers TWO cells — and three buffers put their title at that scale
    /// and then drew the rule at `line + 1`. The rule landed on the title's own
    /// baseline and came out as an underline touching the descenders. Every
    /// check they had was green: the display list said row 0 and row 1, which
    /// are different rows, and nothing anywhere multiplied a row by a height.
    ///
    /// # Why only the rows AFTER the first
    ///
    /// Because two commands on the SAME row is the ordinary case — a row of
    /// columns, a selection band under its own text, an icon beside a name —
    /// and what decides whether those collide is the width, which
    /// [`Self::nothing_overlaps`] answers. What is never ordinary is a command
    /// that begins on a row another command has already grown into.
    ///
    /// # Errors
    ///
    /// Names the covering row, how far it reaches, and the row that starts
    /// inside it.
    pub fn nothing_is_covered(&self, heights: Heights, columns: u16) -> Result<(), String> {
        let rows: Vec<(u16, u16)> = self
            .commands
            .iter()
            .map(|c| Self::rows_of(c, heights))
            .collect();
        let cols: Vec<(u16, u16)> = self
            .commands
            .iter()
            .map(|c| Self::columns_of(c, heights, columns))
            .collect();

        for (i, &(from, tall)) in rows.iter().enumerate() {
            if tall <= 1 {
                continue;
            }
            let inside = from.saturating_add(1)..from.saturating_add(tall);
            for (j, &(at, _)) in rows.iter().enumerate() {
                if i == j || !inside.contains(&at) || !Self::meet(cols[i], cols[j]) {
                    continue;
                }
                return Err(format!(
                    "on line {from}, {} takes {tall} cells and runs down to line \
                     {}, but {} starts on line {at}: the second one is drawn \
                     inside the box of the first",
                    Self::sketch(&self.commands[i]),
                    from.saturating_add(tall).saturating_sub(1),
                    Self::sketch(&self.commands[j])
                ));
            }
        }
        Ok(())
    }

    /// Refuses a frame where two runs on the same row can reach each other, or
    /// where the last one runs off the right edge.
    ///
    /// # Errors
    ///
    /// Names the row, where the field starts and how far it can reach.
    pub fn nothing_overlaps(&self, heights: Heights, columns: u16) -> Result<(), String> {
        let mut per_row: alloc::collections::BTreeMap<u16, Vec<(u16, u16, &Command)>> =
            alloc::collections::BTreeMap::new();
        for c in &self.commands {
            // ONLY TEXT. A rule and a selection paint the whole width on
            // purpose, and counting them here would make every list row collide
            // with its own cursor band.
            if !matches!(c, Command::Text { .. }) {
                continue;
            }
            let (at, wide) = Self::columns_of(c, heights, columns);
            per_row.entry(c.line()).or_default().push((at, wide, c));
        }

        for (line, mut fields) in per_row {
            fields.sort_by_key(|(at, wide, _)| (*at, *wide));
            for pair in fields.windows(2) {
                let (at, wide, what) = pair[0];
                let (next, _, after) = pair[1];
                if at.saturating_add(wide) > next {
                    return Err(format!(
                        "on line {line}, {} starts at {at} and can reach {}, \
                         mas {} comeca em {next}: os dois se sobrepoem",
                        Self::sketch(what),
                        at.saturating_add(wide),
                        Self::sketch(after)
                    ));
                }
            }
            if let Some(&(at, wide, what)) = fields.last()
                && at.saturating_add(wide) > columns
            {
                return Err(format!(
                    "on line {line}, {} starts at {at} and asks for {wide} cells \
                     in a window of {columns}",
                    Self::sketch(what)
                ));
            }
        }
        Ok(())
    }

    /// Every colour token named, in stable order and without repetition.
    ///
    /// It is what the host has to resolve before drawing, and what a check
    /// uses to prove that a buffer does not name a non-existent token.
    #[must_use]
    pub fn colors(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .commands
            .iter()
            .map(|c| match c {
                Command::Text { color, .. }
                | Command::Rule { color, .. }
                | Command::Grid { color, .. }
                | Command::Selection { color, .. }
                | Command::Svg { color, .. } => color.as_str(),
            })
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// What the host sends to the buffer.
///
/// Deliberately poor: size and input, nothing else. A buffer that
/// needed to know the time, the language or the theme would be doing something the
/// host should be doing for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// Usable width, in cells.
    pub columns: u16,
    /// Usable height, in cells.
    pub lines: u16,
    /// How many cells of height each scale occupies.
    pub heights: Heights,
}

/// How many **cells** of height each scale occupies.
///
/// This is not a token value (invariant 11): it is a cell count, the same
/// unit the buffer already counts rows and columns in. What computes it is the host,
/// which knows the font size and the line height.
///
/// Without this the buffer had to GUESS how many rows a large run
/// occupies — and it guessed wrong: the sheet's `huge` glyph came out clipped at the
/// bottom, touching the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heights {
    /// Cells of a `small` run.
    pub small: u16,
    /// Cells of a `base` run.
    pub base: u16,
    /// Cells of a `normal` run. It is the cell, so it is 1.
    pub normal: u16,
    /// Cells of a `large` run.
    pub large: u16,
    /// Cells of an `huge` run.
    pub huge: u16,
}

impl Heights {
    /// How many cells a run of this scale occupies.
    ///
    /// The record is five named fields because that is what crosses the WIT;
    /// this is the one place that turns a [`Scale`] back into its field, so a
    /// caller reserving space for a run never has to spell the mapping out
    /// again — and cannot spell it out differently.
    #[must_use]
    pub const fn of(&self, scale: Scale) -> u16 {
        match scale {
            Scale::Small => self.small,
            Scale::Base => self.base,
            Scale::Normal => self.normal,
            Scale::Large => self.large,
            Scale::Huge => self.huge,
        }
    }

    /// How many cells a run of `size` px occupies.
    ///
    /// Pure arithmetic, with no token: whoever has the numbers is the host, and what
    /// converts them into cells is this calculation. **Rounding up**, always — half a cell
    /// short is the glyph clipped at the bottom, which was the defect that gave rise to
    /// this field.
    #[must_use]
    pub fn cells(size: f32, line_height: f32, cell_height: f32) -> u16 {
        if cell_height <= 0.0 {
            return 1;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (size * line_height / cell_height).ceil() as u32;
        u16::try_from(n.max(1)).unwrap_or(1)
    }

    /// The production heights, given the five sizes and the line height.
    #[must_use]
    pub fn from_sizes(
        small: f32,
        base: f32,
        normal: f32,
        large: f32,
        huge: f32,
        line_height: f32,
    ) -> Self {
        // THE CELL IS `normal`'s: it is what defines the grid the buffer
        // counts rows and columns in. See `Context`.
        let cell = normal * line_height;
        Self {
            small: Self::cells(small, line_height, cell),
            base: Self::cells(base, line_height, cell),
            normal: Self::cells(normal, line_height, cell),
            large: Self::cells(large, line_height, cell),
            huge: Self::cells(huge, line_height, cell),
        }
    }
}

/// A key, already normalised by the host.
///
/// The buffer does NOT see compositor modifiers nor the leader sequence: those
/// are resolved beforehand (section 7.4). What arrives is what is left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A NAMED command, dispatched by the host when the buffer keymap's key
    /// sequence completes. It is the other side of [`Binding`]: the buffer
    /// declares, the host resolves the key, and the command comes back through here — the buffer
    /// never captures a key (section 7.9).
    Command(String),
    /// Go to this state — the host's half of the contract [`Effect::StateChanged`]
    /// completes.
    ///
    /// It arrives when somebody tapped `Esc`, held it and used an arrow, or
    /// chose a state under `SPC b s`. The buffer is told the ID IT DECLARED and
    /// never a key, the same rule as [`Input::Command`] and for the same reason.
    EnterState(String),
    /// A character typed.
    Char(char),
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Left arrow. A grid buffer needs all four directions.
    Left,
    /// Right arrow.
    Right,
    /// Confirm.
    Confirm,
    /// Cancel.
    Cancel,
    /// Delete the previous character.
    Delete,
    /// `Tab`: toggle whatever is toggleable on screen.
    ///
    /// An intent and not a key, like the rest — and a case of its own because, without
    /// it, `Tab` arrives as `Char('\t')` and becomes an invisible tab
    /// inside the search field.
    Toggle,
    /// The result of an [`Effect::Copy`] asked for earlier.
    ///
    /// What knows whether the copy worked is the host — it is what talks to the
    /// clipboard. Without this way back, the buffer could only show an
    /// optimistic "copied!", which lies exactly when it matters.
    Copied(bool),
    /// Text coming from the clipboard (`Ctrl+V`), already resolved by the
    /// host: the buffer does not read the clipboard, just as it does not read a file (invariant 11).
    Paste(String),
    /// A left-button click on a cell of the buffer's grid.
    ///
    /// In **cells**, and never in pixels: a pixel is a Wayland and screen-scale
    /// thing, and the buffer knows neither of the two (invariant 11). What
    /// converts, and what subtracts the window's margin, is the host — in the same way
    /// it already converts a key into [`Input::Char`].
    ///
    /// What resolves WHICH item is there is the buffer: only it knows its own
    /// layout. The host sending "item number N" would be the host reimplementing the
    /// buffer's layout.
    Click {
        /// The column, from the buffer's (0,0) cell.
        column: u16,
        /// The row, from the buffer's (0,0) cell.
        line: u16,
    },
    /// The machine's memory, already read. Only for whoever declared
    /// [`Capability::System`].
    ///
    /// The one input here that nobody pressed: it arrives on the host's clock.
    /// A screen that shows how much memory there is has to change while it is
    /// being looked at — waiting for a key would make it a photograph.
    Survey(Survey),
    /// The transfer queue, ALREADY READ. Only for whoever declared
    /// [`Capability::Transfers`].
    ///
    /// Like [`Input::Survey`] nobody pressed it — but unlike it, it is not a
    /// clock: the host is subscribed to the queue's socket on a thread, and a
    /// copy that moves no byte for a minute sends nothing in that minute.
    Queue(Queue),
    /// Every disk the machine has. Only for a buffer that declared
    /// [`Capability::Disks`].
    ///
    /// It arrives UNREQUESTED, like the survey and the queue: the host watches
    /// udisks on a thread and hands over a fresh reading whenever something
    /// moves. A buffer that did not ask for the capability gets no thread and
    /// no reading, which is what makes the absence structural rather than
    /// promised.
    Rack(Rack),
    /// What became of the format the buffer asked for.
    Formatted(Outcome),
    /// What the machine is going to do by itself, ALREADY READ. Only for a
    /// buffer that declared [`Capability::Brain`].
    ///
    /// Like [`Input::Queue`] nobody pressed it: the host is subscribed to the
    /// brain's own socket on a thread, and what arrives is the WHOLE mind — a
    /// delta needs both sides to agree on what they last saw, and what is being
    /// agreed about here is when the machine will rebuild itself.
    Mind(Mind),
}

#[cfg(test)]
mod tests {
    use super::{
        Binding, Capability, Color, Command, Demand, Descriptor, DisplayList, Effect, Knob,
        Request, Setting,
    };

    /// The SAME gate, on the way out.
    ///
    /// `DisplayList::validate` guards what the buffer DRAWS; nothing guarded what
    /// it ASKS FOR, and the first request that acts on the machine — closing a
    /// process, loosening the protection — is not a thing to leave ungated.
    #[test]
    fn a_gated_effect_requires_the_capability() {
        let ask = Effect::Ask(Demand {
            what: Request::StopConsumer,
            target: 4321,
        });
        assert_eq!(ask.requires(), Some(Capability::System));
        // THE WIDEST REACH THERE IS, gated by both of its doors. A brain
        // request comes through `Ask` and a preference comes through `Tune`,
        // and the two would be a pair of holes if only one of them were named
        // here: either alone ends with the machine rebuilding its own operating
        // system on somebody else's schedule.
        let run = Effect::Ask(Demand {
            what: Request::RunTask,
            target: 1,
        });
        assert_eq!(run.requires(), Some(Capability::Brain));
        let tune = Effect::Tune(Setting {
            task: "update".into(),
            knob: Knob::IntervalDays(7),
        });
        assert_eq!(tune.requires(), Some(Capability::Brain));
        // And everything that was already there keeps costing nothing: a buffer
        // does not declare a capability in order to redraw.
        assert_eq!(Effect::Redraw.requires(), None);
        assert_eq!(Effect::Copy("x".into()).requires(), None);
        assert_eq!(Effect::Command("mx-open".into()).requires(), None);
    }

    fn with_svg() -> DisplayList {
        let mut l = DisplayList::fresh_one();
        l.push(Command::Svg {
            line: 0,
            column: 0,
            lines: 4,
            columns: 4,
            svg: "<svg/>".into(),
            color: Color("text-primary".into()),
        });
        l
    }

    /// THE DEFECT: the gate that never had teeth.
    ///
    /// The WIT has always promised that *"the host refuses whoever uses without having
    /// asked"*, and until now `validate` checked the keymap's prefix and nothing
    /// else — **no capability was required of anyone**. A pixel
    /// primitive coming in without that gate would make "the list is short on purpose"
    /// a sentence with no consequence.
    #[test]
    fn a_gated_primitive_requires_the_capability() {
        assert!(
            with_svg().validate(&[]).is_err(),
            "it drew a vector without declaring `Vector`"
        );
        assert!(
            with_svg().validate(&[Capability::Terminal]).is_err(),
            "the WRONG capability must not open the door"
        );
        with_svg()
            .validate(&[Capability::Vector])
            .expect("whoever asked has to come through");
    }

    /// And whatever has no gate passes without asking for anything — otherwise every existing
    /// buffer would have to declare a capability in order to draw text.
    #[test]
    fn what_has_no_gate_requires_nothing() {
        let mut l = DisplayList::fresh_one();
        l.push(Command::Rule {
            line: 0,
            color: Color("rule-default".into()),
        });
        assert_eq!(l.commands[0].requires(), None);
        l.validate(&[]).expect("a rule requires no capability");
    }

    fn with_keys(t: &str) -> Descriptor {
        Descriptor {
            name: "x".into(),
            // THE BASE STATE IS PART OF A VALID DESCRIPTOR, and this fixture is
            // about the KEYMAP. Without it `validate` refuses on the state list
            // before it ever looks at a binding — which is what happened the
            // first time the guard went in, and the keymap assertions below
            // stopped being exercised while still passing.
            states: vec![BufferState {
                id: BASE_STATE.into(),
                letter: 'n',
                tone: Tone::Standard,
            }],
            keymap: vec![Binding {
                keys: t.into(),
                command: "x.y".into(),
            }],
            ..Descriptor::default()
        }
    }

    #[test]
    fn a_buffer_only_binds_under_the_local_prefix() {
        // Section 7.9: "buffers only bind under `SPC m`. An attempt to bind
        // under a global prefix is REJECTED at load with an error."
        //
        // Without this, a third-party component hijacks `SPC f` and the whole
        // system changes behaviour because of one buffer.
        assert!(with_keys("m f s").validate().is_ok());
        assert!(with_keys("m").validate().is_ok());

        for global in ["f s", "b b", "g", "q q"] {
            let e = with_keys(global).validate().unwrap_err();
            assert!(e.contains(global), "the message does not say which: {e}");
        }
    }

    #[test]
    fn the_refusal_message_names_the_buffer() {
        // Refusing without saying WHO forces a hunt among the loaded
        // components — and the load happens far from whoever wrote the buffer.
        let mut d = with_keys("f s");
        d.name = "browser".into();
        assert!(d.validate().unwrap_err().contains("browser"));
    }

    #[test]
    fn a_capability_is_asked_for_and_not_presumed() {
        // A descriptor with no capability at all is the common case: neither PTY nor
        // pixels. The host refuses whoever USES without having asked.
        assert!(Descriptor::default().capabilities.is_empty());
        let d = Descriptor {
            capabilities: vec![Capability::Terminal],
            ..Descriptor::default()
        };
        assert!(d.capabilities.contains(&Capability::Terminal));
        assert!(!d.capabilities.contains(&Capability::RawSurface));
    }

    use super::*;

    #[test]
    fn the_height_of_an_empty_list_is_zero() {
        // Zero, not 1: an empty buffer reserves no rows.
        assert_eq!(DisplayList::fresh_one().height(), 0);
    }

    #[test]
    fn the_height_counts_the_tallest_line_plus_one() {
        let mut d = DisplayList::fresh_one();
        d.push(Command::Text {
            line: 4,
            column: 0,
            text: Text::datum("x"),
            color: Color::name("text-primary"),
            scale: Scale::Normal,
            max_width: 0,
        });
        d.push(Command::Rule {
            line: 1,
            color: Color::name("rule-default"),
        });
        assert_eq!(d.height(), 5);
    }

    #[test]
    fn the_colours_come_out_sorted_and_with_no_repeats() {
        // The host resolves token by token; a repeat would be wasted work, and
        // an unstable order would make a check compare different lists of the same
        // buffer.
        let mut d = DisplayList::fresh_one();
        for name in ["text.primary", "rule.default", "text.primary"] {
            d.push(Command::Rule {
                line: 0,
                color: Color(name.into()),
            });
        }
        assert_eq!(d.colors(), ["rule.default", "text.primary"]);
    }

    /// The production heights: the cell is `normal`, and `large` is two of it.
    fn heights() -> Heights {
        Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
    }

    fn titled(scale: Scale) -> DisplayList {
        let mut d = DisplayList::fresh_one();
        d.push(Command::Text {
            line: 0,
            column: 3,
            text: Text::key("format-title"),
            color: Color::name("text-primary"),
            scale,
            max_width: 40,
        });
        d.push(Command::Rule {
            line: 1,
            color: Color::name("rule-default"),
        });
        d
    }

    #[test]
    fn a_title_at_the_glyph_scale_swallows_the_rule_under_it() {
        // The defect, exactly as it reached the screen: a 40 px run has a 58 px
        // line box on a 29 px grid, so it covers row 1 as well, and the rule
        // drawn there came out as an underline through the descenders. The
        // display list named two different rows, which is why every check that
        // read rows was green.
        assert!(
            titled(Scale::Large)
                .nothing_is_covered(heights(), 80)
                .is_err()
        );
        // And the fix is not the rule moving: it is the title being the body's
        // size, which is one cell and cannot cover anything.
        assert!(
            titled(Scale::Normal)
                .nothing_is_covered(heights(), 80)
                .is_ok()
        );
    }

    #[test]
    fn two_fields_on_the_same_row_are_the_ordinary_case() {
        // What decides whether these collide is the WIDTH, and that is the other
        // question. If this were a fault, every list row with more than one
        // column would be one.
        let mut d = DisplayList::fresh_one();
        for column in [0, 20, 40] {
            d.push(Command::Text {
                line: 7,
                column,
                text: Text::key("some-label"),
                color: Color::name("text-primary"),
                scale: Scale::Normal,
                max_width: 20,
            });
        }
        assert!(d.nothing_is_covered(heights(), 80).is_ok());
        assert!(d.nothing_overlaps(heights(), 80).is_ok());
    }

    #[test]
    fn a_sentence_claims_its_budget_and_a_datum_claims_its_length() {
        // The asymmetry that keeps this honest. A key is a sentence the host
        // will translate, so the budget is all anyone can go on — a budget that
        // reaches into the next field is a field that WILL be written over as
        // soon as the language changes. A datum is a string the buffer is
        // holding, and it usually positions the next field from that length:
        // charging it for a budget it demonstrably does not use would condemn
        // every list that lets its last column run to the edge.
        let field = |text: Text, column: u16, max_width: u16| Command::Text {
            line: 0,
            column,
            text,
            color: Color::name("text-primary"),
            scale: Scale::Normal,
            max_width,
        };

        let mut sentence = DisplayList::fresh_one();
        sentence.push(field(Text::key("a-sentence"), 0, 40));
        sentence.push(field(Text::datum("32 GiB"), 20, 10));
        assert!(sentence.nothing_overlaps(heights(), 80).is_err());

        let mut datum = DisplayList::fresh_one();
        datum.push(field(Text::datum("nixos"), 0, 40));
        datum.push(field(Text::datum("32 GiB"), 20, 10));
        assert!(datum.nothing_overlaps(heights(), 80).is_ok());
    }

    #[test]
    fn a_glyph_run_is_as_wide_as_it_is_tall() {
        // The cell is `0.600 em`, so the scale multiplies the advance the same
        // way it multiplies the line box: one `Large` glyph covers the two cells
        // `Heights` already charges it in the other direction. A neighbour one
        // cell away is under it.
        let mut d = DisplayList::fresh_one();
        d.push(Command::Text {
            line: 0,
            column: 0,
            text: Text::icon("disk"),
            color: Color::name("text-primary"),
            scale: Scale::Large,
            max_width: 2,
        });
        d.push(Command::Text {
            line: 0,
            column: 1,
            text: Text::datum("x"),
            color: Color::name("text-primary"),
            scale: Scale::Normal,
            max_width: 1,
        });
        assert!(d.nothing_overlaps(heights(), 80).is_err());
    }

    #[test]
    fn data_and_key_are_different_kinds() {
        // Invariant 11 lives in this distinction: the buffer does not translate, but
        // it also cannot send everything as a key — a glyph's name has no
        // translation.
        assert_ne!(Text::datum("nf-fa-folder"), Text::key("nf-fa-folder"));
    }
}
