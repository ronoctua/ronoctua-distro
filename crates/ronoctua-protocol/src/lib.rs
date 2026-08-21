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
}

impl Effect {
    /// The capability this effect requires, when it requires one.
    ///
    /// The twin of [`Command::requires`], and for the same reason: the host has
    /// to refuse BEFORE acting, and a second table of what costs what is a
    /// second table to keep in step.
    #[must_use]
    pub const fn requires(&self) -> Option<Capability> {
        match self {
            Self::Ask(d) => Some(d.what.requires()),
            _ => None,
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
}

impl Request {
    /// The capability this request needs.
    ///
    /// It is the request and NOT the effect that decides, because
    /// [`Effect::Ask`] carries every one of them and two capabilities now meet
    /// inside it. A `match` and not a table: a request added without an arm
    /// does not compile, where a table would let it through and the host would
    /// refuse it at runtime for the wrong reason.
    #[must_use]
    pub const fn requires(self) -> Capability {
        match self {
            Self::PauseJob
            | Self::ResumeJob
            | Self::CancelJob
            | Self::ForgetJob
            | Self::SweepJobs => Capability::Transfers,
            _ => Capability::System,
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
                    "o buffer `{}` tenta bindar `{}`, fora de `{LOCAL_PREFIX}`.\n\
                     Buffers so bindam sob a particao local (secao 7.9); um \
                     prefixo global sequestraria a tecla para todo o sistema.",
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
                "o buffer `{}` nao declara estado nenhum.\n\
                 Todo buffer tem pelo menos `{BASE_STATE}`: e para onde o `Esc` volta.",
                self.name
            ));
        };
        if base.id != BASE_STATE {
            return Err(format!(
                "o buffer `{}` comeca em `{}`, e o estado base tem de ser `{BASE_STATE}`.\n\
                 E por esse id que a modeline decide entre o tom discreto e o \
                 alternativo, e para onde o `Esc` volta.",
                self.name, base.id
            ));
        }
        for (n, s) in self.states.iter().enumerate() {
            if self.states[..n].iter().any(|o| o.id == s.id) {
                return Err(format!(
                    "o buffer `{}` declara `{}` duas vezes.",
                    self.name, s.id
                ));
            }
            if self.states[..n].iter().any(|o| o.letter == s.letter) {
                return Err(format!(
                    "o buffer `{}` usa `{}` para dois estados; um deles seria \
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
            _ => None,
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
            let Some(exigida) = c.requires() else {
                continue;
            };
            if !capabilities.contains(&exigida) {
                return Err(format!(
                    "o buffer desenha {c:?} sem declarar `{exigida:?}`.\n\
                     Cada primitiva com trava e superficie que todo buffer \
                     futuro passa a poder usar (secao 7.6); quem a usa a pede \
                     no descritor, e o host confere antes de desenhar."
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
    /// How many cells a run of `size` px occupies.
    ///
    /// Pure arithmetic, with no token: whoever has the numbers is the host, and what
    /// converts them into cells is this calculation. **Rounding up**, always — half a cell
    /// short is the glyph clipped at the bottom, which was the defect that gave rise to
    /// this field.
    #[must_use]
    pub fn cells(size: f32, line_height: f32, altura_da_celula: f32) -> u16 {
        if altura_da_celula <= 0.0 {
            return 1;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (size * line_height / altura_da_celula).ceil() as u32;
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
}

#[cfg(test)]
mod tests {
    use super::{
        Binding, Capability, Color, Command, Demand, Descriptor, DisplayList, Effect, Request,
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
            "desenhou vetor sem declarar `Vetor`"
        );
        assert!(
            with_svg().validate(&[Capability::Terminal]).is_err(),
            "a capacidade ERRADA nao pode abrir a porta"
        );
        with_svg()
            .validate(&[Capability::Vector])
            .expect("quem pediu tem de passar");
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
        l.validate(&[]).expect("regua nao exige capacidade");
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
            assert!(e.contains(global), "a mensagem nao diz qual: {e}");
        }
    }

    #[test]
    fn the_refusal_message_names_the_buffer() {
        // Refusing without saying WHO forces a hunt among the loaded
        // components — and the load happens far from whoever wrote the buffer.
        let mut d = with_keys("f s");
        d.name = "navegador".into();
        assert!(d.validate().unwrap_err().contains("navegador"));
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

    #[test]
    fn data_and_key_are_different_kinds() {
        // Invariant 11 lives in this distinction: the buffer does not translate, but
        // it also cannot send everything as a key — a glyph's name has no
        // translation.
        assert_ne!(Text::datum("nf-fa-folder"), Text::key("nf-fa-folder"));
    }
}
