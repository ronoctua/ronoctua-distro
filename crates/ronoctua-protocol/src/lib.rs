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

/// The clusters of a piece of literal text, one per cell.
///
/// **The one ruler.** It counts grapheme CLUSTERS, and it lives here — in the
/// crate both sides compile — because the buffer reserves with it
/// (`ronoctua_design::budget::wide`), the host confers with it
/// ([`DisplayList::columns_of`]) and the host cuts with it
/// (`ronoctua_render::layout::elide_end`, which walks the same clusters). Three
/// jobs, one definition; before this there were six functions and they
/// disagreed.
///
/// # Why clusters and not `char`s
///
/// Because a cluster is what the renderer draws as ONE thing. `👩‍❤️‍👨` is six
/// codepoints and one glyph; `◌́` is two and one; a flag is two and one. Counting
/// codepoints made a buffer reserve six cells for something drawn in two, and
/// made the cut produce ANOTHER drawing rather than a shorter one — with a
/// budget of 4 cells the couple became `👩‍❤` + `…`, which reshapes as two emoji.
///
/// # What it is NOT
///
/// It is where the text may be CUT, and it is not the width. Those were the
/// same answer for as long as the only face was the Iosevka — monospaced at
/// `0.600 em`, one cluster one cell — and [`cells`] simply counted these. They
/// stopped being the same answer the day the fallback chain reached the CJK
/// faces: 日本語 is three clusters and six cells. Cutting still happens here,
/// because cutting anywhere else invents a drawing; measuring is [`cells`].
pub fn cells_of(text: &str) -> impl Iterator<Item = &str> {
    unicode_segmentation::UnicodeSegmentation::graphemes(text, true)
}

include!(concat!(env!("OUT_DIR"), "/width.rs"));

/// Does this code point take two cells rather than one?
///
/// A binary search over the generated table: `East_Asian_Width` W or F, plus
/// `Emoji_Presentation`. The table is sorted and disjoint, which is what makes
/// `partition_point` the whole of the lookup.
fn wide_char(c: char) -> bool {
    let c = c as u32;
    let i = WIDE.partition_point(|(_, hi)| *hi < c);
    WIDE.get(i).is_some_and(|(lo, hi)| *lo <= c && c <= *hi)
}

/// How many cells one grapheme cluster occupies.
///
/// # Why ANY wide code point makes the cluster wide
///
/// Because a cluster is drawn as one thing, and what decides its advance is
/// its base character — but the base is not always first, and finding it means
/// implementing the grapheme-break rules a second time. Asking whether any
/// member is wide gives the same answer for everything that exists: the
/// combining marks that follow a narrow base are all narrow themselves, and a
/// sequence that contains a wide member (an emoji with a variation selector, a
/// flag, an ideograph with a combining mark) is drawn wide.
///
/// A cluster is never ZERO cells here, and that is deliberate: a lone
/// combining mark is drawn on a dotted circle, which occupies one, and a run
/// of zero cells would let two pieces of text be placed at the same column.
#[must_use]
pub fn cells_of_one(cluster: &str) -> u16 {
    if cluster.chars().any(wide_char) { 2 } else { 1 }
}

/// How many cells a piece of literal text occupies, at the body's size.
///
/// **The one ruler.** The buffer reserves with it, the host confers with it,
/// and the renderer places with it — `ronoctua_render::layout::width` is this
/// function, reached through this crate rather than written again.
///
/// # Why it is not the cluster count any more
///
/// Because an ideograph is designed on the em square and advances twice what a
/// Latin letter does, in every monospaced CJK cut there is. With the count,
/// 日本語 measured three and drew six, and every column after it on the line
/// was placed at half the distance it needed — not a rounding error, half the
/// width of the text. The emoji were the same defect with a smaller blast
/// radius, and had been known about long enough to be written in this
/// crate's own comments.
#[must_use]
pub fn cells(text: &str) -> u16 {
    cells_of(text)
        .map(cells_of_one)
        .fold(0u16, |total, one| total.saturating_add(one))
}

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
    /// Make a key on ONE keyboard mean something else. Needs
    /// [`Capability::Keyboard`].
    ///
    /// NOT an [`Effect::Ask`]: a demand is a verb plus an index and carries no
    /// value, and this carries three. The buffer touches nothing — it says
    /// what it wants, and the host writes the file, asks keyd whether it can
    /// read it, and restarts the daemon.
    Remap(Remap),
    /// Switch how the keyboards work together, at once. Needs
    /// [`Capability::Keyboard`].
    KeyboardMode(KeyboardMode),
    /// One setting of one keyboard unit, in Independent mode.
    UnitChange(UnitChange),
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
            Self::Remap(_) | Self::KeyboardMode(_) | Self::UnitChange(_) => {
                Some(Capability::Keyboard)
            }
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
    /// Receiving [`Input::Keyboards`] and emitting [`Effect::Remap`].
    ///
    /// Its own, and not folded into [`Capability::System`], because of what it
    /// reaches: the file a daemon between the keyboard and everything else
    /// reads. A mistake there is a keyboard that does not type, on a machine
    /// somebody is holding. The buffer still touches nothing — it hands back
    /// three strings out of the list it was given.
    Keyboard,
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

/// One keyboard, as the kernel reports it and as the person has decided.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Board {
    /// `vendor:product`, which is what keyd matches on.
    ///
    /// NOT unique: two of the same model report the same pair, and keyd has no
    /// finer grain. The screen says so rather than pretending.
    pub id: String,
    /// What the kernel calls it.
    pub name: String,
    /// Built into this machine, rather than plugged or paired into it.
    pub built_in: bool,
    /// What this keyboard has been told: keyd's name for a key, and for what
    /// it becomes.
    pub decided: Vec<(String, String)>,
}

/// A word of the vocabulary: what keyd calls it, and what names it on screen.
///
/// It TRAVELS rather than being compiled into the buffer, because invariant 11
/// stands: the buffer holds no table of ours and does not translate. It
/// receives the ids and hands them back, and the host resolves both.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Word {
    /// keyd's own word. Empty, among the choices, means "leave it alone".
    pub keyd: String,
    /// The text key. The host resolves it; the buffer never reads it.
    pub text: String,
    /// Does the meaning depend on how long the key is held?
    pub holds: bool,
}

/// Every keyboard, and what a key can be made into.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Boards {
    /// Which keys are offered, in the order they are drawn.
    pub keys: Vec<Word>,
    /// What each can become. The FIRST is the way back.
    pub choices: Vec<Word>,
    /// The keyboards themselves.
    pub boards: Vec<Board>,
    /// The rules could not be written, and this says why — already a text
    /// key. Empty when the last write went through.
    pub trouble: String,
    /// keyd's name for the key the leader is made of.
    ///
    /// It travels because the screen has to warn before that one is moved: a
    /// rule written after the leader's wins, so remapping it is how somebody
    /// loses the key that opens everything, on that keyboard.
    pub leader: String,
    /// Together or Independent.
    pub mode: KeyboardMode,
    /// Every unit, plugged or remembered, for Independent mode.
    pub units: Vec<UnitBoard>,
    /// The cursor numbers a unit can be put on.
    pub cursors: Vec<u16>,
    /// The layouts this machine has.
    pub layouts: Vec<LayoutWord>,
}

/// How the keyboards work together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyboardMode {
    /// As one, through keyd, on the first cursor's seat.
    #[default]
    Together,
    /// Each unit its own virtual twin, on its cursor's seat.
    Independent,
}

/// One keyboard UNIT, for Independent mode — a device, not a model.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnitBoard {
    /// The host's hash of where it is plugged and its address.
    pub identity: String,
    /// What the kernel calls it.
    pub name: String,
    /// Its model, `vendor:product`.
    pub model: String,
    /// Built into this machine.
    pub built_in: bool,
    /// Plugged in right now.
    pub plugged: bool,
    /// The cursor it types through.
    pub cursor: u16,
    /// Its xkb layout, empty for the machine's.
    pub layout: String,
    /// Its xkb variant, empty for the layout's default.
    pub variant: String,
    /// keyd's name for the key whose tap is its leader.
    pub leader: String,
    /// Repeat rate per second; zero is the compositor's.
    pub repeat_rate: u16,
    /// Repeat delay in milliseconds; zero is the compositor's.
    pub repeat_delay: u16,
    /// What its keys become, as [`Board::decided`].
    pub decided: Vec<(String, String)>,
}

/// An xkb layout the machine has.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LayoutWord {
    /// Its code — `gb`.
    pub code: String,
    /// What xkeyboard-config calls it — data, like a device's name.
    pub name: String,
    /// The variant codes it offers.
    pub variants: Vec<String>,
}

/// One setting of a unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnitSetting {
    /// The cursor it types through.
    Cursor(u16),
    /// Its layout; empty for the machine's.
    Layout(String),
    /// Its variant; empty for the layout's default.
    Variant(String),
    /// The key whose tap is its leader.
    Leader(String),
    /// Repeat rate; zero is the compositor's.
    RepeatRate(u16),
    /// Repeat delay; zero is the compositor's.
    RepeatDelay(u16),
    /// keyd's key name, and what it becomes; empty undoes.
    Remap(String, String),
}

/// Which unit, and what changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitChange {
    /// [`UnitBoard::identity`].
    pub unit: String,
    /// What changes.
    pub setting: UnitSetting,
}

/// What a key on a keyboard should become.
///
/// A [`Demand`] cannot carry this: it is a verb plus an INDEX, and this is
/// three values. The same reason [`Setting`] exists beside it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Remap {
    /// `vendor:product`, out of the [`Board`] this came from.
    pub device: String,
    /// keyd's name for the key, out of [`Boards::keys`].
    pub key: String,
    /// keyd's name for what it becomes, out of [`Boards::choices`]. Empty
    /// leaves the key alone, and is how a remap is undone.
    pub becomes: String,
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
    /// Write it inside a LUKS2 container. The passphrase is the HOST's to ask
    /// — twice, on its own line — and is never a field of this record.
    pub encrypt: bool,
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
    /// Open what the finished install the target names put in the launcher.
    /// The HOST finds the entry: the buffer knows only that there is one
    /// ([`Job::opens`]).
    LaunchJob,

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

    /// THE MODULES'. Each is still a verb plus ONE index into a list the host
    /// just sent, and the host checks it against that list.
    ///
    /// Show this task's module screen: the target is the 1-based task index,
    /// `0` stops watching. The host asks the brain for the screen whenever the
    /// task's `screen_generation` moves, and pushes it
    /// ([`Input::ModuleScreen`]).
    WatchScreen,
    /// The action on a row: the target is the 1-based index of the row in the
    /// last [`ModuleScreen`], counted across its sections in order.
    ActRow,
    /// The row's SECOND action ([`ModuleRow::also`]), by the same index.
    ActRowAlso,
    /// One of the watched task's [`TaskState::actions`], by its 1-based index.
    /// One that asks first opens the host's question line; the answer goes
    /// with it.
    ActPage,
    /// Put away the notice at this 1-based index of the last
    /// [`Mind::notices`].
    DismissNotice,
    /// Put it off by the first snooze it offers.
    SnoozeNotice,
    /// Do the FIRST control the notice at this 1-based index offers — the
    /// strip's own button, and `, n` on the page.
    ActNotice,
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
            | Self::SweepJobs
            | Self::LaunchJob => Capability::Transfers,
            Self::RunTask
            | Self::RehearseTask
            | Self::AbortTask
            | Self::Proceed
            | Self::Postpone
            | Self::WatchScreen
            | Self::ActRow
            | Self::ActRowAlso
            | Self::ActPage
            | Self::DismissNotice
            | Self::SnoozeNotice
            | Self::ActNotice => Capability::Brain,
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
    /// A region's streets: a download, then a baking.
    Atlas,
    /// The person's programs: the software center's command line.
    Software,
    /// A file sent to VirusTotal.
    Scan,
    /// The person's containers: a pull, a build, compose.
    Container,
    /// The person's belongings: their sync.
    Belongings,
    /// Encrypt anything: a file sealed or opened.
    Seal,
    /// Incus: a long operation of the daemon.
    Incus,
    /// The installer image for a stick.
    Stick,
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
    /// Put files into an archive that exists.
    ArchiveAdd,
    /// Take members out of an archive, rewriting it without them.
    ArchiveRemove,
    /// Read every member of an archive and check it.
    ArchiveTest,
    /// Write an archive again in another format.
    ArchiveConvert,
    /// Encrypt a file.
    Seal,
    /// Open an encrypted file.
    Unseal,
    /// An Incus job: an instance made, exported, imported or published, an
    /// image copied, a file sent in or taken out.
    Incus,
    /// Write the installer to a stick.
    StickWrite,
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
    /// Whether it can be held and let go — a program's order is one run that
    /// nothing can freeze halfway, and a window that offered to hold it would
    /// be offering a key the queue refuses.
    pub pauses: bool,
    /// Whether it is a finished install that put an entry in the launcher,
    /// which [`Request::LaunchJob`] opens.
    pub opens: bool,
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

/// How loud one line of the brain's own log is — the log's own levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    /// Something failed.
    Error,
    /// Something is wrong, and the daemon carried on.
    Warn,
    /// What it did. The default, because a line that lost its level is still
    /// a line and not an alarm.
    #[default]
    Info,
    /// Detail.
    Debug,
    /// More detail.
    Trace,
}

/// One line of the brain's own log, as the page that follows it draws it.
///
/// The TEXT is data — the daemon's words as it wrote them, like a file's
/// contents — and the time is formatted on the host, which has the zone.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogLine {
    /// Its place in the daemon's log, counted from the daemon's start.
    pub seq: u64,
    /// When, as the host's clock reads it: `HH:MM:SS`.
    pub at_text: String,
    /// How loud.
    pub level: LogLevel,
    /// `target: message key=value …`, as the log prints it.
    pub text: String,
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
    /// Nothing ran: the machine was in no state for it.
    ///
    /// Not a failure and not a run. It is written ONCE per episode — the
    /// runner looks every thirty seconds, and a line per look would be a wall
    /// of "RAM is busy" with everything that actually happened buried in it.
    Held,
    /// Nothing ran: a module wrote down something it noticed. Not a run
    /// either, and it moves no schedule.
    Noted,
}

/// Whether a task repeats, or happens once and is then gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskKind {
    /// It comes round again.
    #[default]
    Routine,
    /// It happens once and then it is gone.
    Once,
    /// It runs nothing on a schedule: it listens, and publishes what it heard.
    Watch,
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
    /// Which task this was.
    ///
    /// It crosses because the home page shows one list of everything the
    /// brain has done, and there is no other way there to say whose run a row
    /// is. Inside a task's own page it is redundant and costs a string.
    pub task: String,
    /// Its name, translated, for the reason `TaskState::name_text` carries one.
    pub task_text: String,
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
    /// THE KEYS, so the component never builds one out of an id — a module's
    /// words are its own (`<module>-why-<word>`), and a key built by
    /// convention on this side is a key nobody declared.
    ///
    /// The task's name.
    pub task_key: String,
    /// The name of the step it stopped at. Empty when [`RunRecord::at`] is.
    pub at_key: String,
    /// The key of [`RunRecord::why`]. Empty when `why` is.
    pub why_key: String,
    /// What a [`OutcomeKind::Noted`] record says, as a key. Empty for every
    /// other kind.
    pub says: String,
    /// The sentence the host made of it with its values — the component
    /// cannot format a size or a moment.
    pub says_text: String,
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
    /// Whether it will run.
    pub on: bool,
    /// Whether the distro ships it on — the DECLARED fact.
    ///
    /// It was `changed: bool` (does `on` differ from the default), worked out
    /// on the daemon's side. The default never crossed, so the screen could
    /// say "you changed this" and could not say what it had been changed
    /// FROM, and the host had no way to know what flipping a step meant.
    /// Sending the default makes "changed" an `!=` anybody can do and leaves
    /// one authority for it.
    pub on_by_default: bool,
    /// The key of its name, declared by the module that owns the step — never
    /// built out of the id on this side.
    pub name_key: String,
    /// The key of the line under the cursor.
    pub does_key: String,
}

/// SOMETHING A PERSON CAN ASK OF A MODULE BY NAME — the registry command
/// `<module>-<id>`, offered on its page, beside a notice, or on a row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleAction {
    /// Its id inside the module: what [`Request::ActPage`], [`Request::ActRow`]
    /// and a notice's control hand back.
    pub id: String,
    /// `cmd-<module>-<id>`.
    pub label_key: String,
    /// Whether it acts on a ROW of the module's screen.
    pub row: bool,
    /// The key of the question it asks first; empty when it asks none.
    pub asks_key: String,
}

/// One task, as the screen draws it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskState {
    /// English, kebab-case. `brain-task-<id>` names it on screen.
    pub id: String,
    /// The same name, ALREADY TRANSLATED, and it is for measuring.
    ///
    /// The component draws the key and reserves a column for what it becomes;
    /// blind, that column was twenty-six cells for a word five cells wide.
    /// The rule `next_text` follows, applied to a width instead of a date.
    pub name_text: String,
    /// Whether it repeats or happens once.
    pub kind: TaskKind,
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
    /// The last few runs, NEWEST FIRST.
    ///
    /// `last` is `history[0]` when there is one. Both cross, and not one
    /// derived from the other: a row on the list draws `last` and an empty list
    /// would make every reader check a length before it could.
    ///
    /// How many is the brain's decision and not this record's — what crosses is
    /// what it kept.
    pub history: Vec<RunRecord>,
    /// What it runs.
    pub steps: Vec<TaskStep>,
    /// Whether it keeps a calendar. A page draws the cadence and the window
    /// only when it does: a relief that comes due when the disk is short has
    /// neither.
    pub calendar: bool,
    /// The key of its name — `brain-task-<id>`, declared by its module.
    pub name_key: String,
    /// The key of the sentence that says what it does.
    pub does_key: String,
    /// The key of [`TaskState::why_not`]. Empty when that is.
    pub why_key: String,
    /// THE MODULE'S PAGE MOVED when this does: a page that shows the module's
    /// screen asks for it again ([`Request::WatchScreen`]), and never on a
    /// clock. The screen itself is not in the mind.
    pub screen_generation: u64,
    /// What a person can ask of it by name, in the manifest's order.
    pub actions: Vec<ModuleAction>,
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
    /// Everything the brain has done, newest first, across every task —
    /// FOLDED: consecutive records that say the same thing are one row, and
    /// the row opens into them (`ronoctua_brain::journal::grouped`).
    ///
    /// A task's own `history` answers *how has this one been going*; this
    /// answers *what has this machine been doing*, which is the question the
    /// page is opened with. Two lists and not one view of one: the per-task
    /// one has a FLOOR (a monthly task keeps its last ten however much else
    /// happens) and this one has a CEILING. Neither can be cut out of the
    /// other.
    pub activity_rows: Vec<ActivityRow>,
    /// How many records that list folds at most — not how many it holds today.
    ///
    /// A policy of the daemon's, crossing for the same reason `cadence` does.
    /// The activity block's title is a window onto this number, and a screen
    /// that guessed it would drift from the one doing the cutting.
    pub remembers: u32,
    /// How many of its OWN runs a task keeps, which its `history` is all of.
    ///
    /// The same argument as `remembers`, one list down: a task's `History`
    /// heading is a window onto this number and the owner asked it to say so.
    /// It is `journal::RECENT`, the brain's floor on pruning.
    pub recent: u32,
    /// EVERY NOTICE every module raised, in the order the strip ranks them:
    /// an alarm first, then danger, warn and info, the newest first among
    /// equals. Empty is a machine with nothing to say.
    pub notices: Vec<Notice>,
}

/// SOMETHING A MODULE WANTS SAID, without anybody having asked.
///
/// A key and the sentence the host made of it: a component cannot format a
/// size or a moment, and a key with holes in it is not something it can draw.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Notice {
    /// Whose it is, by the module's id.
    pub module: String,
    /// Its id inside the module.
    pub id: String,
    /// What it says, as a text key.
    pub says: String,
    /// The sentence, already made in the person's language.
    pub says_text: String,
    /// Each value, already formatted.
    pub args: Vec<NoticeArg>,
    /// How loud.
    pub level: NoticeLevel,
    /// Since when it has been true, in epoch seconds.
    pub since: u64,
    /// The same moment, already formatted.
    pub since_text: String,
    /// Actions offered beside it.
    pub controls: Vec<ModuleAction>,
    /// Whether it may be put away.
    pub dismissable: bool,
    /// The snoozes it offers, in minutes; the first is the strip's.
    pub snooze: Vec<u16>,
}

/// HOW LOUD A NOTICE IS. The order is the strip's precedence, quietest first:
/// an alarm beats even the countdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum NoticeLevel {
    /// Worth saying.
    #[default]
    Info,
    /// Something is wrong.
    Warn,
    /// Something is wrong and costs something now.
    Danger,
    /// It rings, and it stays until a person answers it.
    Alarm,
}

/// One value of a notice's sentence, by its name, formatted by the host.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoticeArg {
    /// The Fluent variable.
    pub name: String,
    /// The value, as the person reads it.
    pub text: String,
}

/// ONE ROW OF THE ACTIVITY LOG: consecutive records that say the same thing,
/// folded. Forty holds for a busy RAM are one row — "Last: <when>", how many,
/// and the first — that opens into all of them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivityRow {
    /// What the row draws: the newest of them.
    pub newest: RunRecord,
    /// How many records it folds; `1` is one that stands alone.
    pub repeats: u32,
    /// When the newest of them happened, formatted.
    pub last_text: String,
    /// When the oldest of them happened, formatted.
    pub first_text: String,
    /// All of them, NEWEST FIRST, `newest` included.
    pub members: Vec<RunRecord>,
}

// ---------------------------------------------------------------------------
// A MODULE'S OWN PAGE, as rows of DATA — asked for when its task's
// `screen_generation` moves ([`Request::WatchScreen`]) and pushed by the host
// through [`Input::ModuleScreen`]. Never carried in the [`Mind`]: a page nobody
// is looking at costs nothing. Field order is the WIT's, for the reason written
// above the survey's structs.
// ---------------------------------------------------------------------------

/// The colour of a row, by MEANING — one token each on the host's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrainTone {
    /// Nothing to worry about.
    #[default]
    Quiet,
    /// Something is being followed.
    Watching,
    /// Look soon.
    Warning,
    /// Look now.
    Danger,
    /// The module's own emphasis.
    Accent,
}

/// A moment: the number for the arithmetic, the text for the date.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Timed {
    /// Epoch seconds.
    pub at: u64,
    /// Already formatted, in the machine's zone.
    pub text: String,
}

/// A size: the number for the arithmetic, the text in the person's units and
/// decimal mark.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Measured {
    /// How many bytes.
    pub bytes: u64,
    /// Already formatted.
    pub text: String,
}

/// One cell of a module's row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ScreenCell {
    /// Nothing.
    #[default]
    Empty,
    /// A text key.
    Key(String),
    /// Data, drawn as it is — including what a person typed.
    Data(String),
    /// A moment.
    Moment(Timed),
    /// A count.
    Count(u64),
    /// A size.
    Bytes(Measured),
}

/// One row of a module's page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleRow {
    /// Its id inside the module — DATA; the host hands it back to the module
    /// when the row's action is asked for.
    pub id: String,
    /// What it is.
    pub label: ScreenCell,
    /// Its value.
    pub value: ScreenCell,
    /// A quieter line about it.
    pub note: ScreenCell,
    /// Its colour, by meaning.
    pub tone: BrainTone,
    /// The id of the action a press runs; empty is a row that only tells.
    pub act: String,
    /// The id of its SECOND action, on a key of its own; empty when it has
    /// none.
    pub also: String,
}

/// One titled block of a module's page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleSection {
    /// Its title, as a key.
    pub heading: String,
    /// Its rows.
    pub rows: Vec<ModuleRow>,
}

/// A module's own page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleScreen {
    /// Whose, by the task's id in the last [`Mind`].
    pub module: String,
    /// The `screen_generation` it answers.
    pub generation: u64,
    /// In order.
    pub sections: Vec<ModuleSection>,
    /// Why there is no page, as a sentence the host made — the brain refused
    /// it, or did not answer. Empty when the page came.
    pub refused: String,
}

impl ModuleScreen {
    /// Its rows, counted across the sections in order — the list a
    /// [`Request::ActRow`] index points into.
    pub fn rows(&self) -> impl Iterator<Item = &ModuleRow> {
        self.sections.iter().flat_map(|s| s.rows.iter())
    }
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
/// "Buffers only bind under the local leader. An attempt to bind under a
/// global prefix is **rejected at load with an error**." Without that, a
/// third-party component hijacks `SPC f` and the whole system changes
/// behaviour because of one buffer.
///
/// `,` — Spacemacs's major-mode leader, the counterpart of Doom's `SPC m`.
/// It WAS `m`, Doom's own, and the system's MISCELLANEOUS group is `m` too:
/// with a buffer focused its keymap took the letter, and the owner opened
/// Lucide's window, pressed `SPC m` for the screenshot and got Lucide's
/// menu. A local leader is a prefix no group of the system may ever want,
/// and punctuation is where letters run out last.
pub const LOCAL_PREFIX: &str = ",";

/// The transfer queue's keys — ONE table for its two doors.
///
/// The footer's panel (`LEADER t t`) reads a key by these notations. The
/// window (`LEADER t w`) is a buffer: it declares every chord here under
/// [`LOCAL_PREFIX`] ([`local_binding`]) — `C-c` is `, c` there, which its
/// host turns back into `C-c` — and receives the rest, the arrows, `RET`,
/// `TAB` and `SPC`, as the intents its host already sends. So a key means the
/// same thing in both doors, and a key added here is added to both; the two
/// used to disagree about `RET` (it held a job in one and opened it in the
/// other) and about `C-p` (up, and hold).
///
/// `RET` is what a second click on a row does, in both: it opens the job, one
/// field a line — and, with the give-up question on screen, answers it.
pub const TRANSFERS_KEYS: &[(&str, &str)] = &[
    ("<down>", "transfers-down"),
    ("C-n", "transfers-down"),
    ("<up>", "transfers-up"),
    ("C-p", "transfers-up"),
    ("RET", "transfers-detail"),
    ("TAB", "transfers-detail"),
    ("C-d", "transfers-detail"),
    ("SPC", "transfers-pause"),
    ("C-c", "transfers-cancel"),
    ("C-k", "transfers-forget"),
    ("C-o", "transfers-launch"),
    ("C-l", "transfers-sweep"),
];

/// The key that answers the give-up question in both doors of the queue —
/// the arrows walk its two rows, `no` first, and `RET` takes the one the
/// cursor is on.
pub const TRANSFERS_ANSWER: &[(&str, &str)] = &[("RET", "transfers-answer")];

/// What a row of the transfer queue allows — the facts [`transfers_taught`]
/// reads, which each door works out from its own copy of the job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TransferRow {
    /// Still under way: weighing, running or held.
    pub afoot: bool,
    /// Held, and so let go by the same key.
    pub held: bool,
    /// Its engine can hold it at all.
    pub pauses: bool,
    /// A finished install that put an entry in the launcher.
    pub opens: bool,
}

/// The keys a door of the queue TEACHES, in [`TRANSFERS_KEYS`]'s order:
/// `(notation, id, contextual)` — contextual when only the row under the
/// cursor makes it do something. ONE answer for both doors, so the two lines
/// of keys cannot disagree about what a row allows.
///
/// The walk is not taught (the arrows are every list's), and on a job's own
/// page neither is the key that opened it: `Esc` is the way back there.
#[must_use]
pub fn transfers_taught(
    detail: bool,
    row: Option<TransferRow>,
) -> Vec<(&'static str, &'static str, bool)> {
    let mut out: Vec<(&'static str, &'static str, bool)> = Vec::new();
    for (notation, id) in TRANSFERS_KEYS {
        if out.iter().any(|(_, said, _)| said == id) {
            continue;
        }
        let taught = match *id {
            "transfers-detail" => row.is_some() && !detail,
            "transfers-pause" => row.is_some_and(|r| r.pauses && r.afoot),
            "transfers-cancel" => row.is_some_and(|r| r.afoot),
            "transfers-forget" => row.is_some_and(|r| !r.afoot),
            "transfers-launch" => row.is_some_and(|r| r.opens),
            "transfers-sweep" => true,
            _ => false,
        };
        if taught {
            let contextual = !matches!(*id, "transfers-detail" | "transfers-sweep");
            out.push((notation, id, contextual));
        }
    }
    out
}

/// A chord of a view's table as a buffer binds it: `C-c` is `, c` — the local
/// leader and the letter, which the buffer's host turns back into `C-c` in
/// its window. `None` for a key that is not a chord.
#[must_use]
pub fn local_binding(notation: &str) -> Option<String> {
    notation
        .strip_prefix("C-")
        .filter(|rest| rest.chars().count() == 1)
        .map(|rest| alloc::format!("{LOCAL_PREFIX} {rest}"))
}

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
    /// The key that selects it directly under `SPC w s`.
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
    /// Keys whose TEXT the buffer needs as STATE, not as drawing.
    ///
    /// Everything a buffer only SHOWS goes out as [`Text::Key`] and is
    /// translated when the host draws it. This is the other case — text the
    /// buffer holds, parses or writes back — and the host answers it with
    /// [`Input::Phrases`] before the first frame. See [`Phrase`].
    pub phrases: Vec<String>,
}

/// One key a buffer declared, with the text the host resolved for it.
///
/// # Why a buffer receives a translated string at all
///
/// Because [`Text::Key`] only ever reaches the PERSON: it is resolved when the
/// host draws, and the buffer never holds what came out. That covers everything
/// a buffer only shows, and it does not cover text the buffer also HOLDS and
/// READS BACK.
///
/// The three browsers' search field is exactly that: the buffer keeps the
/// string, parses it, and the person edits it. As [`Text::Data`] it opened
/// `familia:emoji` in an en-GB session; as a [`Text::Key`] the buffer could not
/// have re-read what it drew.
///
/// So the host hands the text over, the same way it hands a clipboard over in
/// [`Input::Paste`]. `docs/ARCHITECTURE.md` says a buffer cannot write a phrase
/// of UI in any language, and it still cannot: it can only echo one it was
/// given.
///
/// THE FIELD ORDER IS THE WIT'S — see the note at the top of this file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    /// The key, as the buffer declared it in [`Descriptor::phrases`].
    pub key: String,
    /// The text, in the person's language.
    pub text: String,
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
                     inalcancavel sob `SPC w s`.",
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
        /// Counted from the RIGHT edge — see `edge` in the WIT's
        /// `text-command`. The column still names the cell from the left.
        edge: bool,
    },
    /// A sentence, WRAPPED, in a rectangle of cells.
    ///
    /// # Why a buffer cannot do this itself
    ///
    /// Because it holds a key and not a sentence. [`Command::Text`] draws one
    /// row and the host elides it at the budget — right for a label, useless
    /// for a description, which cut at a column's width is a description
    /// nobody can read. Wrapping needs the TRANSLATED string, and a buffer
    /// never sees one.
    ///
    /// The host looks the key up, wraps with the same `layout::wrap` the file
    /// manager's names use, draws up to `lines` rows and elides the last if
    /// there is more. Nothing new is decided on the guest's side.
    Paragraph {
        /// The first row.
        line: u16,
        /// The first column.
        column: u16,
        /// How many rows it may fill.
        lines: u16,
        /// How wide, in cells.
        columns: u16,
        /// What to write.
        content: Text,
        /// The colour token.
        color: Color,
        /// The size, by name.
        scale: Scale,
        /// The first WRAPPED row to draw, which is how a sentence scrolls.
        ///
        /// Only the host knows how many rows the sentence came to — it holds
        /// the translated string and it is the one that wraps — so the buffer
        /// keeps the OFFSET and the host clamps it, draws the window, and
        /// draws a thumb beside it when there is more. `0` is the top.
        first_row: u16,
    },
    /// A ONE-line horizontal rule, across the row or across part of it.
    ///
    /// It is a SEPARATOR, and separators are allowed — the preference for a
    /// layout that does without one is a preference, not a ban. What it could
    /// not do before was stop short: a rule under a heading inside one of two
    /// side-by-side columns has to end where the column does, and a full-bleed
    /// stripe would run through its neighbour.
    ///
    /// `columns: 0` is to the edge.
    Rule {
        /// On which row.
        line: u16,
        /// The first column it covers.
        column: u16,
        /// How many columns. `0` is to the edge.
        columns: u16,
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
    /// The ground under a row, or under part of one.
    ///
    /// **A change of colour, and no radius.** It used to have no column and no
    /// width either, and the reason written beside it was that a ground
    /// "paints the whole row, and that is all". That was never the owner's
    /// rule: two columns side by side, each with its own selection, is an
    /// ordinary layout, and it is unreachable from a stripe that always spans
    /// the line.
    ///
    /// `columns: 0` is "to the edge" — the same thing `0` means for a text
    /// run's budget — so every band that was full-bleed still is.
    Selection {
        /// On which row.
        line: u16,
        /// The first column it covers.
        column: u16,
        /// How many columns. `0` is to the edge.
        columns: u16,
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
    /// A scrollbar, for a list the buffer is showing a window onto.
    ///
    /// # Why a buffer cannot draw its own
    ///
    /// Because a thumb is a fraction of a track in PIXELS, and a buffer counts
    /// in cells (invariant 11). Built out of block characters it would be a
    /// second scrollbar that looks nothing like the five the shell draws.
    /// What the host does with this is exactly what it does for its own lists
    /// — [`ronoctua_design::Scroll`], compiled on both sides of the wall since
    /// it was written, plus the pixel residue only the host can know.
    ///
    /// **Nothing is drawn when the list fits.** `Scroll::cut` answers that,
    /// and the whole shell agrees that a bar over a list that fits is a
    /// control which does nothing.
    Bar {
        /// The first row of the track.
        line: u16,
        /// How many rows it covers.
        lines: u16,
        /// Which column it sits in.
        ///
        /// The CALLER subtracts it from its own content. A bar drawn over text
        /// is the defect the working rules name: *"barra nenhuma desenha por
        /// cima do conteúdo"*.
        column: u16,
        /// How many rows the list has.
        total: u32,
        /// How many of them are shown.
        shown: u32,
        /// Which row is the first one drawn.
        offset: u32,
        /// The colour token.
        color: Color,
    },
    /// From this line down, the frame is anchored to the window's FOOT.
    ///
    /// # What it is for
    ///
    /// A surface holds `floor(height / line)` whole rows plus a leftover of
    /// less than one, and that leftover falls under the last row — which is
    /// where a buffer that ends in a footer puts the one thing that has to sit
    /// at the bottom. The owner read it as *"um espaço após o `Last run` …
    /// que varia de tamanho conforme a altura disponível para o buffer"*;
    /// measured on that screen, nine pixels of it.
    ///
    /// The ground already took the residue — `Scroll::track`'s rule, applied
    /// to the last band. The INK never did. With this the host places these
    /// lines counting UP from the bottom, so the leftover falls between the
    /// block above and the block at the foot, inside the page's own ground,
    /// where nothing is drawn.
    ///
    /// It draws NOTHING itself, which is why it claims no rows and no columns:
    /// it is a fact about the frame and not a thing in it.
    Anchor {
        /// The first line that is anchored.
        foot_from: u16,
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
            | Self::Svg { line, .. }
            | Self::Bar { line, .. }
            | Self::Paragraph { line, .. } => *line,
            // The line it anchors FROM, which is the only line it knows.
            Self::Anchor { foot_from } => *foot_from,
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
            // THE OTHER FIVE COST NOTHING, and they are named rather than
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
            //
            // `Bar` is among the free ones on purpose: it produces no pixels
            // of its own, it carries no content, and what it draws is
            // arithmetic over numbers the buffer already had. A gate on it
            // would be a gate on "this list is longer than its window", which
            // is not a privilege.
            Self::Text { .. }
            | Self::Rule { .. }
            | Self::Grid { .. }
            | Self::Selection { .. }
            | Self::Bar { .. }
            | Self::Anchor { .. }
            | Self::Paragraph { .. } => None,
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
    ///
    /// # It counts the whole EXTENT, not the first row
    ///
    /// This was `line() + 1`, and a command that claims rows DOWNWARD — a
    /// paragraph of six, a bar down a block of twenty — reported the row it
    /// began on. So a frame whose footer ran four rows past the bottom edge
    /// answered with a height inside the window, and the buffer's own
    /// `nothing_is_drawn_outside_the_window` test was green the whole time the
    /// owner was looking at a footer drawn over a history list.
    ///
    /// [`Self::rows_of`] already knew the answer; nothing here is new
    /// arithmetic, only the arithmetic that was already written being asked.
    #[must_use]
    pub fn height(&self, heights: Heights) -> u16 {
        self.commands
            .iter()
            .map(|c| Self::reach_of(c, heights))
            .max()
            .unwrap_or(0)
    }

    /// Nothing reaches past the last line of the window.
    ///
    /// # Why the host could not be the one to notice
    ///
    /// Because it did not clip and it did not complain: a line past the bottom
    /// was measured, laid out, submitted to the rasteriser and dropped there,
    /// with nothing in any log. It was the one defect in this tree with no
    /// symptom at all — the ink simply was not on the screen, and every guard
    /// that might have said so was looking at something else.
    ///
    /// # Errors
    ///
    /// Which command, how far it reaches, and how tall the window is.
    pub fn nothing_is_drawn_below(&self, heights: Heights, lines: u16) -> Result<(), String> {
        for c in &self.commands {
            let (from, _) = Self::rows_of(c, heights);
            let reaches = Self::reach_of(c, heights);
            if reaches > lines {
                return Err(format!(
                    "a {} beginning on row {from} reaches row {} of a window {lines} rows tall                      — the host draws it and the rasteriser throws it away, silently",
                    Self::sketch(c),
                    reaches.saturating_sub(1),
                ));
            }
        }
        Ok(())
    }

    /// One row past the last a command reaches.
    ///
    /// The public half of [`Self::rows_of`], because the host asks the same
    /// question when it decides whether a command lands inside the window at
    /// all — and two answers to "how far down does this go" is how a frame
    /// came to be measured one way and drawn another.
    #[must_use]
    pub fn reach_of(c: &Command, heights: Heights) -> u16 {
        let (from, tall) = Self::rows_of(c, heights);
        from.saturating_add(tall)
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
            // A BAR CLAIMS ITS WHOLE TRACK, which is what makes the overlap
            // guard useful for it: the one mistake it invites is a bar drawn
            // down a column somebody else is writing in, and that is exactly
            // what `nothing_overlaps` is looking for.
            Command::Bar { line, lines, .. } => (*line, (*lines).max(1)),
            // A PARAGRAPH CLAIMS ITS WHOLE RECTANGLE, however few rows the
            // sentence turns out to need: the caller reserved them, and what
            // the overlap guard is looking for is something else drawn where
            // this one MAY reach.
            Command::Paragraph { line, lines, .. } => (*line, (*lines).max(1)),
            // AN ANCHOR CLAIMS NOTHING. It is a fact about where the frame
            // sits, not a thing in it, and a height of zero is what lets every
            // guard in this file walk past it without an exemption anybody has
            // to remember: `nothing_is_covered` skips a span one row tall or
            // less, and `nothing_overlaps` looks only at text.
            Command::Anchor { foot_from } => (*foot_from, 0),
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
    /// column of their own. A `Bar` claims exactly ONE — a scrollbar in this
    /// system is one cell wide, which is what `AGENTS.md` means by "a bar that
    /// scrolls vertically takes a CELL".
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
                    Text::Data(t) => cells(t)
                        .saturating_mul(heights.of(*scale).max(1))
                        .min(budget),
                    Text::Icon(_) => heights.of(*scale).max(1).min(budget),
                };
                (*column, wide)
            }
            // A BAND CLAIMS WHAT IT WAS GIVEN, and the whole line when it was
            // given nothing — which is what `0` means everywhere else here.
            Command::Rule {
                column,
                columns: wide,
                ..
            }
            | Command::Selection {
                column,
                columns: wide,
                ..
            } => (
                *column,
                if *wide == 0 {
                    columns.saturating_sub(*column)
                } else {
                    *wide
                },
            ),
            Command::Bar { column, .. } => (*column, 1),
            Command::Anchor { .. } => (0, 0),
            Command::Paragraph {
                column,
                columns: wide,
                ..
            } => (*column, *wide),
            Command::Grid { column, lines, .. } => {
                (*column, lines.iter().map(|l| cells(l)).max().unwrap_or(0))
            }
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
            Command::Paragraph { content, .. } => match content {
                Text::Key(k) => format!("a paragraph of `{k}`"),
                Text::Data(t) => format!(
                    "a paragraph of `{}`",
                    t.chars().take(24).collect::<String>()
                ),
                Text::Icon(i) => format!("a paragraph of the icon `{i}`"),
            },
            // THE NUMBERS, because a bar that collides is a bar in the wrong
            // column, and "a bar" alone would not say which of the two on the
            // screen it was.
            Command::Bar {
                column,
                total,
                shown,
                ..
            } => format!("a bar in column {column} over {shown} of {total}"),
            Command::Anchor { foot_from } => {
                format!("the frame anchored to the foot from row {foot_from}")
            }
        }
    }

    /// Whether a command is a GROUND — a full-width colour drawn underneath.
    ///
    /// # Why only the selection, and not the rule or the bar
    ///
    /// Because a ground is the one thing whose whole job is to be inside
    /// somebody else's box: a list row sits on its cursor band, a footer's
    /// text sits on its own ground, and a paragraph three rows tall sits on
    /// three of them. Reporting that as a collision is reporting that the
    /// drawing works.
    ///
    /// A RULE IS NOT A GROUND. It is an edge at the bottom of a row, and a
    /// tall title whose box reaches it is exactly the defect this guard was
    /// written for — the heading that drew a rule on its own baseline, which
    /// `the_old_defect_is_still_refused` reproduces by hand.
    ///
    /// A BAR IS NOT ONE EITHER. It is a control in a column, and a run whose
    /// box reaches down through that column is a run drawn over it — which is
    /// the same thing said the other way round by the working rules: no bar
    /// draws over content, and no content draws over a bar. What keeps an
    /// honest bar out of this is the COLUMN test, not an exemption.
    const fn is_ground(c: &Command) -> bool {
        match c {
            Command::Selection { .. } => true,
            Command::Text { .. }
            | Command::Paragraph { .. }
            | Command::Grid { .. }
            | Command::Svg { .. }
            | Command::Rule { .. }
            | Command::Anchor { .. }
            | Command::Bar { .. } => false,
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
                // A GROUND IS NEVER COVERED — being underneath is its whole
                // job. Without this, a footer with a ground and a paragraph on
                // it reported itself as a defect: the paragraph claims the
                // rows it was given, and the ground is one selection per row
                // inside them. Every one of those is the drawing working.
                if Self::is_ground(&self.commands[j]) {
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
                         but {} starts at {next}: the two overlap",
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
            .filter_map(|c| match c {
                Command::Text { color, .. }
                | Command::Rule { color, .. }
                | Command::Grid { color, .. }
                | Command::Selection { color, .. }
                | Command::Svg { color, .. }
                | Command::Bar { color, .. }
                | Command::Paragraph { color, .. } => Some(color.as_str()),
                // AN ANCHOR HAS NO INK, so it has no colour to check. Named
                // rather than swept into a `_`: a primitive that DOES paint
                // may not be added without this list being read.
                Command::Anchor { .. } => None,
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
    /// chose a state under `SPC w s`. The buffer is told the ID IT DECLARED and
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
    /// The keys the buffer declared in [`Descriptor::phrases`], resolved.
    ///
    /// ALL of them at once, and before the first frame: a buffer that has to
    /// draw with them cannot be handed them one at a time and redraw in
    /// between. The declaration is the gate — a buffer that declared none gets
    /// none, which is the shape a [`Capability`] gives the inputs below without
    /// needing a capability of its own. A phrase is the translation of a key
    /// the buffer itself named, not a reading of the machine.
    Phrases(Vec<Phrase>),
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
    /// The wheel, over a cell of the buffer's grid.
    ///
    /// The other half of [`Command::Bar`]: a scrollbar nobody can turn is a
    /// picture of a control, and until this existed a buffer received no
    /// pointer event at all except the click.
    ///
    /// **In lines, never in pixels**, the same wall [`Input::Click`] respects.
    /// The arithmetic that turns a wheel into lines is the host's and there is
    /// one of it — it carries the leftover fraction between events, which is
    /// what five windows each got wrong by writing `(pixels / line) as i64 * 3`
    /// and throwing the fraction away.
    Wheel {
        /// The column the pointer was over.
        column: u16,
        /// The row the pointer was over.
        line: u16,
        /// How many lines, and which way.
        ///
        /// NEGATIVE is towards the top of the list: every caller subtracts
        /// this from an offset, and a sign that flipped on the way in would
        /// flip there.
        lines: i16,
    },
    /// A BAR THAT WAS DRAGGED, answered as the first row to draw.
    ///
    /// The other half of a scrollbar. One that is drawn and cannot be grabbed
    /// is a picture of a control — the sentence this repository already keeps
    /// about the wheel — and the owner reported exactly that: *"o clicar e
    /// arrastar a scrollbar com o mouse não está funcionando"*.
    ///
    /// The host is the only side that can turn a pointer at a height into a
    /// row: it built the thumb out of the three numbers the `Bar` carried, and
    /// the inverse is `Scroll::offset_at`. What it cannot know is WHICH list —
    /// a page may have several — so the position crosses for the same reason
    /// it does on a wheel.
    ///
    /// ABSOLUTE, and never a delta: a drag is where the hand IS, and a sum of
    /// deltas drifts away from it over a long gesture.
    Scrolled {
        /// The column the pointer is over.
        column: u16,
        /// The row the pointer is over.
        line: u16,
        /// The row that should now be the first one drawn in that block.
        first: u32,
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
    /// Every keyboard, and what a key can be made into.
    ///
    /// Requires [`Capability::Keyboard`]. Like [`Input::Rack`], nobody
    /// pressed it: the host reads the kernel and the person's file, and what
    /// arrives is the whole picture.
    Keyboards(Boards),
    /// What became of the format the buffer asked for.
    Formatted(Outcome),
    /// What the machine is going to do by itself, ALREADY READ. Only for a
    /// buffer that declared [`Capability::Brain`].
    ///
    /// Like [`Input::Queue`] nobody pressed it: the host is subscribed to the
    /// brain's own socket on a thread, and what arrives is the WHOLE mind — a
    /// delta needs both sides to agree on what they last saw, and what is being
    /// agreed about here is when the machine will rebuild itself.
    ///
    /// BOXED, and every other variant of this enum is not. `Mind` is three
    /// hundred bytes — a list of tasks, a list of everything the brain has
    /// done, a notice — and an enum is as big as its largest variant, so
    /// unboxed it would make every `Input` in the system that size, including
    /// the arrow keys. One allocation on a path that already crossed a socket
    /// is the cheaper half of that trade by a wide margin.
    Mind(Box<Mind>),
    /// The brain's own log, WHOLE: every line the host holds, oldest first.
    /// Only for a buffer that declared [`Capability::Brain`].
    ///
    /// Whole for the reason the mind is: the buffer holds nothing that could
    /// go out of step, and a few hundred short lines cost less than being
    /// wrong about which ones it has.
    Log(Vec<LogLine>),
    /// A MODULE'S OWN PAGE, for a buffer that declared [`Capability::Brain`]
    /// and asked for one ([`Request::WatchScreen`]): pushed when its
    /// generation moves, and never on a clock.
    ///
    /// BOXED for the reason [`Input::Mind`] is: an enum is as big as its
    /// largest variant, and the arrow keys would pay for a page of rows.
    ModuleScreen(Box<ModuleScreen>),
    /// Where the pointer is resting, in the buffer's own cells.
    ///
    /// FLATTENED here and a `position` record on the wire, exactly as
    /// [`Input::Click`] is — the runtime re-nests it, and the comment beside
    /// the wheel's lowering says what happens to anything that forgets.
    ///
    /// The host sends one only when the CELL changes, which is what makes this
    /// cheap: the pointer moves in pixels, a buffer counts cells, and a whole
    /// traversal of a wide screen is a few dozen of these.
    Hover {
        /// The column, from the buffer's (0,0) cell.
        column: u16,
        /// The row, from the buffer's (0,0) cell.
        line: u16,
        /// Which cursor — its colour number, from 0 (`pointed` in the WIT).
        seat: u16,
    },
    /// The pointer left the window.
    ///
    /// Its own case rather than a `Hover` with no position: "gone" and
    /// "somewhere I cannot place" are different answers, and only the first one
    /// should put a highlight out. It names the cursor that left, so another
    /// cursor's highlight stays.
    Unhover {
        /// Which cursor left.
        seat: u16,
    },
    /// The next keys are this seat's — sent whenever the seat typing into
    /// the window changes, so a buffer can keep one selection per keyboard.
    KeyboardSeat {
        /// The cursor's colour number, as [`Input::Hover`]'s.
        seat: u16,
    },
    /// That seat's keyboard left the window.
    KeyboardLeft {
        /// Which one.
        seat: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::{
        Binding, Capability, Color, Command, Demand, Descriptor, DisplayList, Effect, Knob,
        Request, Setting,
    };

    /// FAILS IF: the queue's table stops being one the window can bind — a
    /// chord that is not `C-` and one letter would be dropped from the buffer
    /// in silence — or a chord stops becoming the local leader and its letter.
    #[test]
    fn every_chord_of_the_queue_is_a_local_key_of_its_window() {
        use super::{LOCAL_PREFIX, TRANSFERS_KEYS, local_binding};
        for (notation, id) in TRANSFERS_KEYS {
            if notation.starts_with("C-") {
                let bound = local_binding(notation).unwrap_or_default();
                assert!(bound.starts_with(LOCAL_PREFIX), "{notation} → {bound}");
                assert_eq!(
                    bound.chars().count(),
                    3,
                    "{notation} ({id}) is not one letter"
                );
            } else {
                assert!(
                    matches!(*notation, "<up>" | "<down>" | "RET" | "TAB" | "SPC"),
                    "{notation} ({id}) reaches the window as no intent its host sends"
                );
            }
        }
        assert_eq!(local_binding("C-c").as_deref(), Some(", c"));
        assert_eq!(local_binding("RET"), None);
        // ONE MEANING A KEY: no notation is listed twice.
        let mut seen: Vec<&str> = TRANSFERS_KEYS.iter().map(|(k, _)| *k).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), TRANSFERS_KEYS.len());
    }

    /// FAILS IF: a door teaches a key its row does nothing with — holding a
    /// program's install, giving up what has ended, forgetting what is afoot,
    /// opening what made nothing — or stops teaching the page and the sweep.
    #[test]
    fn a_row_teaches_what_it_allows_and_nothing_else() {
        use super::{TransferRow, transfers_taught};
        let ids = |detail, row| -> Vec<&'static str> {
            transfers_taught(detail, row)
                .into_iter()
                .map(|(_, id, _)| id)
                .collect()
        };
        let running = TransferRow {
            afoot: true,
            held: false,
            pauses: true,
            opens: false,
        };
        assert_eq!(
            ids(false, Some(running)),
            [
                "transfers-detail",
                "transfers-pause",
                "transfers-cancel",
                "transfers-sweep"
            ]
        );
        let install = TransferRow {
            pauses: false,
            ..running
        };
        assert!(!ids(false, Some(install)).contains(&"transfers-pause"));
        let installed = TransferRow {
            afoot: false,
            held: false,
            pauses: false,
            opens: true,
        };
        assert_eq!(
            ids(false, Some(installed)),
            [
                "transfers-detail",
                "transfers-forget",
                "transfers-launch",
                "transfers-sweep"
            ]
        );
        // ON THE JOB'S OWN PAGE the key that opened it is not taught.
        assert!(!ids(true, Some(running)).contains(&"transfers-detail"));
        assert_eq!(ids(false, None), ["transfers-sweep"]);
    }

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
            column: 0,
            columns: 0,
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
        // Section 7.9: "buffers only bind under the local leader. An attempt
        // to bind under a global prefix is REJECTED at load with an error."
        //
        // Without this, a third-party component hijacks `SPC f` and the whole
        // system changes behaviour because of one buffer.
        assert!(with_keys(", f s").validate().is_ok());
        assert!(with_keys(",").validate().is_ok());

        // `m` INCLUDED: it is the system's miscellaneous group, and a buffer
        // binding there took the letter from it.
        for global in ["f s", "b b", "g", "q q", "m f s"] {
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
        assert_eq!(DisplayList::fresh_one().height(heights()), 0);
    }

    /// A COMMAND THAT CLAIMS ROWS DOWNWARD IS COUNTED WHOLE.
    ///
    /// THE DEFECT THIS CATCHES: the height was `line() + 1`, so a paragraph of
    /// six rows beginning on row 20 answered 21. A buffer whose footer ran off
    /// the bottom therefore reported a height inside the window, and the test
    /// that was meant to prove nothing is drawn outside it stayed green while
    /// the owner was photographing a footer drawn over a history list.
    #[test]
    fn a_block_that_reaches_down_is_counted_to_its_last_row() {
        let mut d = DisplayList::fresh_one();
        d.push(Command::Paragraph {
            line: 20,
            column: 0,
            columns: 40,
            content: Text::key("about"),
            color: Color::name("text-dim"),
            scale: Scale::Normal,
            lines: 6,
            first_row: 0,
        });
        assert_eq!(
            d.height(heights()),
            26,
            "the paragraph reported its first row"
        );
        assert!(d.nothing_is_drawn_below(heights(), 26).is_ok());
        let trouble = d
            .nothing_is_drawn_below(heights(), 24)
            .expect_err("six rows from row twenty do not fit in twenty-four");
        assert!(
            trouble.contains("25"),
            "the sentence does not say how far it reaches: {trouble}"
        );
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
            edge: false,
        });
        d.push(Command::Rule {
            line: 1,
            column: 0,
            columns: 0,
            color: Color::name("rule-default"),
        });
        assert_eq!(d.height(heights()), 5);
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
                column: 0,
                columns: 0,
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
            edge: false,
        });
        d.push(Command::Rule {
            line: 1,
            column: 0,
            columns: 0,
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
                edge: false,
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
            edge: false,
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
            edge: false,
        });
        d.push(Command::Text {
            line: 0,
            column: 1,
            text: Text::datum("x"),
            color: Color::name("text-primary"),
            scale: Scale::Normal,
            max_width: 1,
            edge: false,
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

    /// THE DEFECT THIS CATCHES: the ruler going back to counting things.
    ///
    /// An ideograph is designed on the em square and advances twice what a Latin
    /// letter does, in every monospaced CJK cut there is. With the count,
    /// 日本語 measured three and drew six, and every column after it on the
    /// line was placed at half the distance it needed.
    #[test]
    fn an_ideograph_is_two_cells_and_a_letter_is_one() {
        assert_eq!(cells("abc"), 3);
        assert_eq!(
            cells("\u{65E5}\u{672C}\u{8A9E}"),
            6,
            "three clusters, six cells"
        );
        assert_eq!(
            cells_of("\u{65E5}\u{672C}\u{8A9E}").count(),
            3,
            "still three to cut on"
        );
        // Mixed, which is the case a line of code or a file name actually is.
        assert_eq!(cells("a\u{65E5}b"), 4);
    }

    /// Kana and Hangul are wide too, and they are the two that a table of
    /// "the CJK blocks" written by hand tends to miss.
    #[test]
    fn the_other_wide_scripts_are_wide() {
        for (name, s) in [
            ("hiragana", "\u{3042}"),
            ("katakana", "\u{30A2}"),
            ("hangul syllable", "\u{D55C}"),
            ("fullwidth A", "\u{FF21}"),
            ("ideographic space", "\u{3000}"),
        ] {
            assert_eq!(cells(s), 2, "{name}");
        }
    }

    /// AMBIGUOUS IS NARROW, and that is a decision rather than an oversight.
    ///
    /// `East_Asian_Width=A` covers Cyrillic, Greek and the box drawing this
    /// interface is built out of. Treating those as two cells is what a
    /// terminal does for a CJK legacy encoding, and here it would double the
    /// width of every rule and every arrow on screen.
    #[test]
    fn the_ambiguous_ones_stay_narrow() {
        for (name, s) in [
            ("greek alpha", "\u{03B1}"),
            ("cyrillic a", "\u{0430}"),
            ("box drawing", "\u{2500}"),
            ("bullet", "\u{2022}"),
            ("degree", "\u{00B0}"),
        ] {
            assert_eq!(cells(s), 1, "{name}");
        }
    }

    /// An emoji is one thing and two cells; a combining mark adds neither a
    /// cluster nor a cell; and nothing is ever zero cells wide.
    #[test]
    fn a_cluster_is_never_nothing() {
        assert_eq!(cells("\u{1F600}"), 2, "emoji");
        assert_eq!(cells("e\u{0301}"), 1, "e with a combining acute");
        assert_eq!(cells("\u{1F1E7}\u{1F1F7}"), 2, "a flag is one cluster");
        // A LONE COMBINING MARK is drawn on a dotted circle and occupies one.
        // Zero would let two runs be placed at the same column.
        assert_eq!(cells("\u{0301}"), 1);
    }

    /// The ruler is the same one the CUT walks, so a text can always be cut
    /// into pieces that add up to it.
    #[test]
    fn the_pieces_add_up_to_the_whole() {
        for s in ["abc", "\u{65E5}\u{672C}\u{8A9E}", "a\u{1F600}b\u{D55C}", ""] {
            let sum: u16 = cells_of(s).map(cells_of_one).sum();
            assert_eq!(sum, cells(s), "{s:?}");
        }
    }
}
