//! The memory panel — `LEADER s m`.
//!
//! # What it is for
//!
//! The machine has a policy against freezing (`nix/modules/memory.nix`) and
//! nothing on screen ever said so. This buffer is where four questions live:
//! how much memory there is and of what kind, who is eating it, where the disk
//! went, and whether the protection is armed right now.
//!
//! # It reads nothing
//!
//! Invariant 11, and this buffer is the first to test it against live data: the
//! HOST samples `/proc`, the cgroups and systemd and pushes a
//! `types::Survey` in; the component only decides what to draw. Every request
//! that touches the machine — closing something, loosening the policy, sweeping
//! the disk — goes out as `effect::ask`, which the host refuses unless this
//! descriptor declared `capability::system`.
//!
//! # No bars
//!
//! There is no rectangle primitive, and a block character would be a codepoint
//! chosen by hand inside a component that has no font — invariant 9. Numbers
//! and percentages, which is the language the rest of the system already speaks.
//!
//! # THE HEADING THIS PANEL DREW IS NOW EVERYBODY'S
//!
//! `ronoctua-design` — the crate invariant 12 names — holds one implementation
//! of each decision the screen makes, and this file draws through it. Four
//! buffers had four answers to "what closes a heading" and three of them drew a
//! rule; this one left a blank cell and drew none, which is what
//! `docs/ARCHITECTURE.md` records. The owner has standardised on exactly that:
//! `Screen::heading` emits a glyph, a title and NO RULE. The panel that was
//! already right about its heading is the one the other six now copy.
//!
//! [`Screen::rule`](ronoctua_design::Screen::rule) stays in the vocabulary and
//! this file still calls it — between BLOCKS, which is what a rule is for. What
//! changed is what it draws: a hairline at the bottom edge of the row it names,
//! full bleed, instead of a run of glyphs inset by a margin this buffer used to
//! add on top of the host's.
//!
//! # And it no longer generates its own bindings
//!
//! `wit_bindgen::generate!` is gone, and with it the crate-wide
//! `#![allow(unsafe_code, missing_docs)]` this file used to need: the exemption
//! belongs to the one crate that generates bindings, and that is no longer this
//! one.

use std::cell::RefCell;

use ronoctua_design::guest::Wit;
use ronoctua_design::sheet_split;
use ronoctua_design::types::{
    AIR, Binding, BufferState, Capability, Command, Consumer, ConsumerKind, Context, Demand,
    Descriptor, Effect, GAP, Gauge, Guard, GuardMode, Guest, ICON, Icon, Input, Pressure, Request,
    Role, Run, Screen, Survey, Tone,
};

/// Every text key this buffer names.
///
/// A module, so the `.ftl` and the code can be compared by eye and by check:
/// nothing else guards a key that does not exist, and the host draws `[key]`
/// when it cannot find one.
mod key {
    pub const TITLE: &str = "memory-title";
    pub const RAM: &str = "memory-ram";
    pub const ZRAM: &str = "memory-zram";
    pub const SWAP: &str = "memory-swap";
    pub const PRESSURE: &str = "memory-pressure";
    pub const DISK: &str = "memory-disk";
    pub const GUARD: &str = "memory-guard";
    pub const KERNEL: &str = "memory-kernel";
    pub const CACHED: &str = "memory-cached";
    pub const ANONYMOUS: &str = "memory-anonymous";
    pub const SHARED: &str = "memory-shared";
    pub const DIRTY: &str = "memory-dirty";
    pub const DESKTOP: &str = "memory-desktop";
    pub const APPS: &str = "memory-apps";
    pub const SYSTEM: &str = "memory-system";
    pub const VRAM: &str = "memory-vram";

    pub const CONSUMERS: &str = "memory-consumers";
    pub const SCANNING: &str = "memory-scanning";
    pub const NOTHING_YET: &str = "memory-nothing-yet";
    pub const HELP_OVERVIEW: &str = "memory-help-overview";
    pub const HELP_FULL: &str = "memory-help-full";
    pub const HELP_GUARD: &str = "memory-help-guard";

    pub const GUARD_DESCRIPTION: &str = "memory-guard-description";
    pub const GUARD_DETAIL: &str = "memory-guard-detail";
    pub const GUARD_ARMED: &str = "memory-guard-armed";
    pub const GUARD_OFF: &str = "memory-guard-off";
    pub const GUARD_UNKNOWN: &str = "memory-guard-unknown";
    pub const GUARD_STRICT: &str = "memory-guard-strict";
    pub const GUARD_TOLERANT: &str = "memory-guard-tolerant";
    pub const GUARD_LIMIT: &str = "memory-guard-limit";
    pub const GUARD_FOR: &str = "memory-guard-for";
    pub const GUARD_CEILING: &str = "memory-guard-ceiling";
    pub const GUARD_ACTED: &str = "memory-guard-acted";
    pub const GUARD_PERSISTED: &str = "memory-guard-persisted";
    pub const GUARD_SESSION_ONLY: &str = "memory-guard-session-only";
    pub const GUARD_COST: &str = "memory-guard-cost";

    pub const CONFIRM_STOP: &str = "memory-confirm-stop";
    pub const CONFIRM_SWEEP: &str = "memory-confirm-sweep";
    pub const CONFIRM_PERSIST: &str = "memory-confirm-persist";
    pub const ASKED: &str = "memory-asked";
}

/// The commands this buffer registers, and the keys that reach them.
///
/// Data, and under `m ` only: the host refuses at load whoever binds under a
/// global prefix. Each id needs `cmd-<id>` in both languages — which a check
/// already demands of every buffer.
const KEYMAP: &[(&str, &str)] = &[
    ("m f", "memory-full"),
    ("m o", "memory-guard"),
    ("m v", "memory-tab"),
    ("m r", "memory-refresh"),
    ("m s", "memory-scan"),
    ("m x", "memory-scan-cancel"),
    ("m k", "memory-stop"),
    ("m g", "memory-sweep-store"),
    ("m n", "memory-sweep-generations"),
    ("m j", "memory-sweep-journal"),
    ("m t", "memory-tolerant"),
    ("m d", "memory-default"),
    ("m p", "memory-persist"),
];

/// How many of the biggest the small screen shows.
///
/// An EDITORIAL number and not a layout one: the overview answers "is
/// everything all right", and five names is as much as anybody reads without
/// choosing. What decides how many FIT is the arithmetic below, which stops at
/// the footer's row whatever this says.
const BIGGEST: usize = 5;

/// Which panel is on.
///
/// Named `Panel` and not `Screen`: [`Screen`] is the design crate's builder
/// now, and one name for two things is how a reader stops knowing which is
/// which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    /// The small live one: is everything all right?
    Overview,
    /// The full one: where exactly?
    Full(Tab),
    /// The protection.
    Guard,
}

/// Which half of the full screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Ram,
    Disk,
}

/// An irreversible action, waiting for the same key again.
///
/// Inline, on the last line — never a popup (invariant 14). Any other key
/// clears it: nothing irreversible stays armed on screen by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Stop,
    Sweep(Request),
    Persist,
}

struct State {
    /// The last survey the host pushed. Everything drawn comes from here.
    survey: Survey,
    panel: Panel,
    /// The row under the cursor, in the consumer list.
    cursor: usize,
    /// The first visible row.
    top: usize,
    pending: Option<Pending>,
    /// A request was sent; the answer is the next survey.
    asked: bool,
    /// The last `render`'s context, because `event` does not get one.
    lines: u16,
    /// The row the consumer list started on in the last `render`.
    ///
    /// `None` when the screen has no list to click. It is recorded rather than
    /// computed because the block above the list varies with what the machine
    /// reported — and a click that lands on the wrong row here closes the wrong
    /// thing.
    list_top: Option<u16>,
}

impl State {
    const fn fresh() -> Self {
        Self {
            survey: empty_survey(),
            panel: Panel::Overview,
            cursor: 0,
            top: 0,
            pending: None,
            asked: false,
            lines: 24,
            list_top: None,
        }
    }
}

/// A survey with nothing in it — what is drawn before the first tick.
const fn empty_survey() -> Survey {
    Survey {
        readings: Vec::new(),
        pressure: Pressure {
            some10: 0,
            some60: 0,
            some300: 0,
            full10: 0,
            full60: 0,
        },
        consumers: Vec::new(),
        guard: Guard {
            mode: GuardMode::Unknown,
            pressure_limit: 0,
            pressure_seconds: 0,
            ceiling: 0,
            acted: 0,
            zram: false,
            persisted: GuardMode::Unknown,
        },
        scanning: false,
        progress: 0,
    }
}

thread_local! {
    /// `const {}`: the initialiser runs at compile time, so the first access
    /// costs nothing and there is no lazy-init check on every borrow.
    static STATE: RefCell<State> = const { RefCell::new(State::fresh()) };
}

/// A reading, when the machine has it.
fn of(s: &Survey, g: Gauge) -> Option<u64> {
    s.readings.iter().find(|r| r.of == g).map(|r| r.bytes)
}

/// Bytes as a person reads them.
///
/// One decimal from a gibibyte up, none below: `14.3 GiB`, `812 MiB`. The
/// separator is a full stop because a component does not know the language —
/// it does not translate (invariant 11), and a comma here would be a decision
/// about locale taken in the wrong place.
fn size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if bytes >= GIB {
        let tenths = bytes * 10 / GIB;
        format!("{}.{} GiB", tenths / 10, tenths % 10)
    } else if bytes >= MIB {
        format!("{} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{} KiB", bytes / KIB)
    } else {
        format!("{bytes} B")
    }
}

/// Per mille as a percentage with one decimal.
fn percent(per_mille: u16) -> String {
    format!("{}.{}%", per_mille / 10, per_mille % 10)
}

/// What fraction of `total` `part` is, as a percentage with one decimal.
fn share(part: u64, total: u64) -> String {
    if total == 0 {
        return "—".to_owned();
    }
    let per_mille = u16::try_from(part.saturating_mul(1000) / total).unwrap_or(1000);
    percent(per_mille)
}

/// What part an alarming value plays — and the four colours this panel lost.
///
/// # The gap
///
/// A buffer may no longer name a colour: it names a ROLE and the palette
/// resolves it, which is what makes "one heading" and "one accent" enforceable
/// instead of agreed. There are seven roles, and none of them is a SEVERITY.
/// This panel used to name four directly — `state-success` for a protection in
/// strict mode, `state-warning` for a tolerant one and for pressure above the
/// limit, `state-error` for one that is off or unreadable, `state-critical` for
/// the confirmation that closes a process.
///
/// So the four collapse into two: what is as it should be is the BODY, and
/// what is not is the ACCENT — the one colour the system reserves for "this
/// one". Nothing becomes unreadable, because the WORDS still say which:
/// `memory-guard-armed`, `memory-guard-off` and `memory-guard-unknown` are
/// three different sentences, and so are the three confirmations. What is gone
/// is green against amber against red telling them apart without reading, on a
/// panel whose whole job is to be read at a glance.
///
/// It is one function and one call per site, so the day `Role` grows a severity
/// this is the only place that changes.
const fn alarm(alarming: bool) -> Role {
    if alarming { Role::Accent } else { Role::Body }
}

/// The glyph that says what kind of thing is eating the memory.
const fn kind_glyph(kind: ConsumerKind) -> Icon {
    match kind {
        ConsumerKind::Process => Icon::Terminal,
        ConsumerKind::Scope => Icon::Window,
        ConsumerKind::Directory => Icon::Folder,
        ConsumerKind::Filesystem => Icon::Save,
        ConsumerKind::Generation => Icon::History,
    }
}

/// One line of the panel: a label, a measurement, and what it is read against.
///
/// The value is a [`Run`] and not a `String` because some lines carry a NUMBER
/// (data, literal) and others carry a WORD (a key, which the host translates).
/// It was a `String` first, and the screen printed `memory-reserved` where the
/// word should have been — the buffer cannot translate, and forgetting that
/// shows up as the key's own name in front of the person.
struct Entry {
    /// The key naming what this line measures.
    label: &'static str,
    /// The measurement itself.
    value: Run<Icon>,
    /// What the measurement is read against: a total, a ratio, a floor, a
    /// ceiling. Always DATA — a note that carried a key would need a component
    /// that translates, and there is none (invariant 11).
    note: String,
    /// What part the value plays. See [`alarm`].
    role: Role,
}

impl Entry {
    /// A line whose value is a measurement.
    fn of(label: &'static str, value: &str) -> Self {
        Self {
            label,
            value: Run::datum(value),
            note: String::new(),
            role: Role::Body,
        }
    }

    /// A line whose value is a WORD — the host translates it.
    fn worded(label: &'static str, value: &str) -> Self {
        Self {
            label,
            value: Run::label(value),
            note: String::new(),
            role: Role::Body,
        }
    }

    fn with_note(mut self, note: String) -> Self {
        self.note = note;
        self
    }

    const fn playing(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
}

/// How the value and the note share what the sheet left over.
///
/// # Two of this panel's three columns are the SHEET's, and the third is not
///
/// `sheet_split` decides where the label ends and the value begins — one
/// implementation, shared with the three icon browsers. It is what this file's
/// own `VALUE_AT = 16` used to say on its own, and saying it in one place is
/// the whole point of the crate.
///
/// A line here carries a NOTE as well, and the sheet models two fields: it
/// gives the value everything from its first cell to the right edge, so there
/// is no room in it for a third. Splitting what it left is therefore decided
/// HERE — and it is the sheet's OWN RULE one level down, so that there is still
/// no constant of this panel's in it. The sheet's label is `SHEET_LABEL` cells
/// or half the window, whichever is smaller; the value column is the width of
/// that label, or half of what is left, whichever is smaller.
///
/// The three columns then sit one sheet apart — label at 0, value at
/// `Sheet::value`, note at twice it — and at a footer's width they shrink
/// together instead of the note drifting off towards the right edge, which is
/// what dividing the remainder in half did.
///
/// `None` when there is not room for both, and then the line is exactly the
/// sheet's two fields.
const fn beside(label: u16, room: u16) -> Option<(u16, u16)> {
    let half = room.saturating_sub(GAP) / 2;
    let value = if label < half { label } else { half };
    let note = room.saturating_sub(value).saturating_sub(GAP);
    if value == 0 || note == 0 {
        return None;
    }
    Some((value, note))
}

/// Draws one [`Entry`] at the screen's current row, and moves past it.
fn entry(screen: &mut Screen<Wit>, e: Entry) {
    if !above_the_footer(screen) {
        return;
    }
    let at = screen.row();
    let split = sheet_split(screen.columns());
    let three = if e.note.is_empty() {
        None
    } else {
        beside(split.label, split.room)
    };
    let Some((value, note)) = three else {
        // TWO FIELDS: the sheet's own, unchanged. `marked` is the sheet's one
        // flag and it colours the label as well as the value — which is not
        // what this panel means by an alarm, and is why the three-field path
        // below places its own runs. See `alarm`.
        let cells = screen.sheet(at, e.label, e.value, matches!(e.role, Role::Accent));
        // A ROW OCCUPIES THE CELL IT NAMES even when nothing was drawn in it —
        // the same `max(1)` the design crate's own `Row::tall` applies, and for
        // the same reason: stepping by zero draws the whole panel on one line.
        screen.skip(cells.max(AIR));
        return;
    };
    let mut cells = screen.put(
        at,
        Run::label(e.label).small().dim().at(0).within(split.label),
    );
    cells = cells.max(screen.put(at, e.value.playing(e.role).at(split.value).within(value)));
    cells = cells.max(
        screen.put(
            at,
            Run::datum(&e.note)
                .small()
                .quiet()
                .at(split.value.saturating_add(value).saturating_add(GAP))
                .within(note),
        ),
    );
    screen.skip(cells.max(AIR));
}

/// One consumer's line, with the cursor's band when it is on it.
///
/// # It is the panel's OWN three columns, and not a list of its own
///
/// A consumer's line has three fields too — what kind of thing it is, who it
/// is, and how much — so it goes in the three columns everything else on this
/// panel goes in: the glyph at cell zero, the name across the first two, and
/// the number in the third, where every number that is read against something
/// else already is. The name takes two columns because it is DATA of no fixed
/// length and it is what one came to read; a path cut at twenty cells with
/// sixty empty ones beside it is the spacing complaint this whole change came
/// from, seen from the other side.
fn consumer(screen: &mut Screen<Wit>, c: &Consumer, focused: bool) {
    if !above_the_footer(screen) {
        return;
    }
    let at = screen.row();
    if focused {
        // THE SELECTION IS A LINE, not a box: there is no rectangle in this
        // protocol, and a box would be the card invariant 15 refuses.
        screen.band(at);
    }
    let split = sheet_split(screen.columns());
    let name_at = ICON.saturating_add(GAP);
    // Where the number goes: the third column when there is one, and the
    // second when the window only has two — the same fallback an [`Entry`]
    // makes, so a narrow window narrows both the same way.
    let (bytes_at, bytes) = match beside(split.label, split.room) {
        Some((value, note)) => (split.value.saturating_add(value).saturating_add(GAP), note),
        None => (split.value, split.room),
    };
    let mut cells = screen.put(at, Run::glyph(kind_glyph(c.kind)).dim().at(0));
    let for_the_name = bytes_at.saturating_sub(GAP).saturating_sub(name_at);
    if for_the_name > 0 {
        cells = cells.max(screen.put(at, Run::datum(&c.name).at(name_at).within(for_the_name)));
    }
    if bytes > 0 {
        cells = cells.max(screen.put(at, Run::datum(&size(c.bytes)).at(bytes_at).within(bytes)));
    }
    screen.skip(cells.max(AIR));
}

/// The seam between two blocks: an empty cell, with the hairline at the bottom
/// of it.
///
/// A rule names the boundary at the END of the row it is given, full bleed and
/// outside the buffer's margin — so a separator costs ONE row, the air is above
/// the line, and the block below starts against it. It used to be a row of
/// glyphs inset by a margin this file added on top of the host's, which is the
/// rule that "left a gap on one side and spilled on the other".
fn seam(screen: &mut Screen<Wit>) {
    if !above_the_footer(screen) {
        return;
    }
    let at = screen.row();
    screen.rule(at);
    screen.skip(AIR);
}

/// Is there still a row above the footer's?
///
/// EVERY line of this panel asks, and the footer never does. A window with a
/// ceiling — the footer's expansion has one, and it is twelve rows — is shorter
/// than what the machine has to report, and the compositor does not clip: a
/// line that ran on would be drawn on top of the help. The block that gives way
/// is always the last one, which is the same order a person reads in.
fn above_the_footer(screen: &Screen<Wit>) -> bool {
    screen.row() < last_row(screen)
}

/// A line of text, unless the footer's row is already here.
fn line_of(screen: &mut Screen<Wit>, run: Run<Icon>) {
    if above_the_footer(screen) {
        screen.line(run);
    }
}

/// The last row, which belongs to the footer on every screen.
fn last_row(screen: &Screen<Wit>) -> u16 {
    screen.lines().saturating_sub(1)
}

/// The help line, always the last one.
fn help(screen: &mut Screen<Wit>, which: &str) {
    let at = last_row(screen);
    screen.put(at, Run::label(which).small().quiet());
}

impl State {
    /// The small live one.
    fn overview(&self, screen: &mut Screen<Wit>) {
        let s = &self.survey;
        screen.heading(Icon::Settings, key::TITLE);

        let total = of(s, Gauge::RamTotal).unwrap_or(0);
        let available = of(s, Gauge::RamAvailable).unwrap_or(0);
        let used = total.saturating_sub(available);
        entry(
            screen,
            Entry::of(key::RAM, &size(available)).with_note(format!(
                "{} / {}",
                size(used),
                size(total)
            )),
        );

        if let (Some(original), Some(compressed)) =
            (of(s, Gauge::ZramOriginal), of(s, Gauge::ZramCompressed))
        {
            // THE RATIO IS MEASURED, not the 50% the module asks for — that one
            // is the ceiling. Zero written is not a ratio of zero: it is nothing
            // to divide.
            let ratio = (original * 10).checked_div(compressed).map_or_else(
                || "—".to_owned(),
                |tenths| format!("{}.{}x", tenths / 10, tenths % 10),
            );
            entry(
                screen,
                Entry::of(key::ZRAM, &size(compressed))
                    .with_note(format!("{} → {ratio}", size(original))),
            );
        }

        if let Some(total) = of(s, Gauge::SwapTotal) {
            let free = of(s, Gauge::SwapFree).unwrap_or(0);
            entry(
                screen,
                Entry::of(key::SWAP, &size(total.saturating_sub(free)))
                    .with_note(format!("/ {}", size(total))),
            );
        }

        // PRESSURE IS THE NUMBER THE POLICY ACTS ON, so it is on the small
        // screen: everything else says how full the machine is, and this one
        // says whether it is suffering.
        let stall = s.pressure.some10;
        entry(
            screen,
            Entry::of(key::PRESSURE, &percent(stall))
                .with_note(percent(s.pressure.some60))
                .playing(alarm(stall >= s.guard.pressure_limit && stall > 0)),
        );

        seam(screen);
        entry(screen, self.guard_entry());

        if let (Some(total), Some(free)) = (of(s, Gauge::DiskTotal), of(s, Gauge::DiskFree)) {
            entry(
                screen,
                Entry::of(key::DISK, &size(free)).with_note(format!("/ {}", size(total))),
            );
        }

        seam(screen);
        line_of(screen, Run::label(key::CONSUMERS).base());
        // The five biggest, and no cursor: this screen is for reading. Choosing
        // and closing is the full one's business.
        if s.consumers.is_empty() {
            line_of(screen, Run::label(waiting(s)).quiet());
        } else {
            for c in s.consumers.iter().take(BIGGEST) {
                consumer(screen, c, false);
            }
        }

        help(screen, key::HELP_OVERVIEW);
    }

    /// The protection's line, on both screens.
    fn guard_entry(&self) -> Entry {
        let g = &self.survey.guard;
        let state = match g.mode {
            GuardMode::Strict | GuardMode::Tolerant => key::GUARD_ARMED,
            GuardMode::Off => key::GUARD_OFF,
            GuardMode::Unknown => key::GUARD_UNKNOWN,
        };
        // THE STATE IS THE VALUE, and it is a KEY — because it is a word, and a
        // word on this screen is a translation.
        //
        // It was in the NOTE, and a note is data: the screen drew
        // `memory-guard-unknown`, the key's own name, in front of the person.
        // The OCR of `memory-vm` is what caught it — no unit test could, because
        // the key was there and correct; what was wrong was WHICH SLOT it went
        // into. The numbers moved to the note, where data belongs.
        let numbers = match g.mode {
            GuardMode::Tolerant | GuardMode::Strict => {
                format!("{} · {}s", percent(g.pressure_limit), g.pressure_seconds)
            }
            _ => String::new(),
        };
        Entry::worded(key::GUARD, state)
            .with_note(numbers)
            .playing(alarm(!matches!(g.mode, GuardMode::Strict)))
    }
}

/// What to say where the list would be, when there is nothing in it yet.
const fn waiting(s: &Survey) -> &'static str {
    if s.scanning {
        key::SCANNING
    } else {
        key::NOTHING_YET
    }
}

impl State {
    /// The full one: the whole list, with a cursor and the actions.
    ///
    /// It gives back the row the list started on, so a click lands on the row
    /// it looks like it landed on. Computing that offset twice — once here and
    /// once in `event` — is how a cursor ends up one row off from what the eye
    /// picked, on a screen where the cursor decides what gets closed.
    fn full(&self, tab: Tab, screen: &mut Screen<Wit>) -> u16 {
        let s = &self.survey;
        screen.heading(
            match tab {
                Tab::Ram => Icon::Settings,
                Tab::Disk => Icon::Save,
            },
            match tab {
                Tab::Ram => key::RAM,
                Tab::Disk => key::DISK,
            },
        );

        match tab {
            Tab::Ram => self.ram_detail(screen),
            Tab::Disk => self.disk_detail(screen),
        }
        seam(screen);

        // THE LIST IS VIRTUALISED: only what is on screen becomes a command.
        // The alternative is emitting forty rows into a window that shows eight,
        // every frame, for nothing.
        //
        // The last row belongs to the footer, and one more to the progress while
        // a scan is running — both are drawn below, and a list that grew into
        // them would be two runs on one row.
        let first = screen.row();
        let until = last_row(screen).saturating_sub(u16::from(s.scanning));
        if first < until {
            if s.consumers.is_empty() {
                line_of(screen, Run::label(waiting(s)).quiet());
            } else {
                let room = usize::from(until.saturating_sub(first));
                for (i, c) in s.consumers.iter().enumerate().skip(self.top).take(room) {
                    consumer(screen, c, i == self.cursor);
                }
            }
        }

        if s.scanning {
            // The progress is a number, for the same reason there are no bars.
            line_of(screen, Run::datum(&percent(s.progress)).small().quiet());
        }

        self.footer(screen, key::HELP_FULL);
        first
    }

    /// The RAM breakdown — what the memory is made of.
    fn ram_detail(&self, screen: &mut Screen<Wit>) {
        let s = &self.survey;
        let total = of(s, Gauge::RamTotal).unwrap_or(0);
        for (label, gauge) in [
            (key::ANONYMOUS, Gauge::RamAnonymous),
            (key::CACHED, Gauge::RamCached),
            (key::SHARED, Gauge::RamShmem),
            (key::KERNEL, Gauge::RamKernel),
            (key::DIRTY, Gauge::RamDirty),
        ] {
            if let Some(v) = of(s, gauge) {
                entry(
                    screen,
                    Entry::of(label, &size(v)).with_note(share(v, total)),
                );
            }
        }
        // What the policy reserves, and what it lets the applications take.
        //
        // `≥` and `≤` and not a word: a note is DATA, and a word there would
        // have to be translated by a component that does not translate. The
        // symbols say floor and ceiling in any language.
        if let Some(v) = of(s, Gauge::DesktopCurrent) {
            let note =
                of(s, Gauge::DesktopMin).map_or_else(String::new, |m| format!("≥ {}", size(m)));
            entry(screen, Entry::of(key::DESKTOP, &size(v)).with_note(note));
        }
        if let Some(v) = of(s, Gauge::AppCurrent) {
            let note =
                of(s, Gauge::AppCeiling).map_or_else(String::new, |c| format!("≤ {}", size(c)));
            entry(screen, Entry::of(key::APPS, &size(v)).with_note(note));
        }
        if let Some(v) = of(s, Gauge::SystemCurrent) {
            entry(screen, Entry::of(key::SYSTEM, &size(v)));
        }
        // The graphics memory, when the driver says. On an APU it is carved out
        // of the same RAM above — which is why it is on this screen and not on
        // one about the display.
        if let (Some(used), Some(total)) = (of(s, Gauge::VramUsed), of(s, Gauge::VramTotal)) {
            entry(
                screen,
                Entry::of(key::VRAM, &size(used)).with_note(format!("/ {}", size(total))),
            );
        }
    }

    /// The disk breakdown — the numbers that come from metadata.
    fn disk_detail(&self, screen: &mut Screen<Wit>) {
        let s = &self.survey;
        let total = of(s, Gauge::DiskTotal).unwrap_or(0);
        if let Some(free) = of(s, Gauge::DiskFree) {
            entry(
                screen,
                Entry::of(key::DISK, &size(free)).with_note(format!("/ {}", size(total))),
            );
        }
        // THE STORE, THE JOURNAL AND THE HOME DIRECTORIES ARE NOT LINES HERE.
        //
        // Each of them costs a process or a walk, so they arrive as consumers
        // of a scan — the list below this. Reserving a row for a number that
        // only exists after `m s` would be a row that is blank until then, and
        // a blank row reads as zero.
    }

    /// The protection's screen.
    fn guard(&self, screen: &mut Screen<Wit>) {
        let g = &self.survey.guard;
        screen.heading(Icon::Key, key::GUARD);

        // THE DESCRIPTION FIRST, and small: it is there for whoever arrives not
        // knowing what this is, and it must not compete with the state for
        // whoever already does.
        line_of(screen, Run::label(key::GUARD_DESCRIPTION).small());
        line_of(screen, Run::label(key::GUARD_DETAIL).small().quiet());
        seam(screen);

        entry(screen, self.guard_entry());
        entry(
            screen,
            Entry::of(key::GUARD_LIMIT, &percent(g.pressure_limit)),
        );
        entry(
            screen,
            Entry::of(key::GUARD_FOR, &format!("{}s", g.pressure_seconds)),
        );
        // A ZERO CEILING IS NOT A CEILING OF ZERO: it is `MemoryHigh=infinity`,
        // which is what an unconfigured slice reports. Drawing `0.0%` there
        // would say the applications may use nothing.
        entry(
            screen,
            Entry::of(
                key::GUARD_CEILING,
                &if g.ceiling == 0 {
                    "—".to_owned()
                } else {
                    percent(g.ceiling)
                },
            ),
        );
        entry(screen, Entry::of(key::GUARD_ACTED, &g.acted.to_string()));
        // A WORD, so the host translates it: the name of a profile is UI text,
        // and printing `tolerant` in a Portuguese session would be the buffer
        // deciding a language it has no business deciding.
        entry(
            screen,
            match g.persisted {
                GuardMode::Tolerant => Entry::worded(key::GUARD_PERSISTED, key::GUARD_TOLERANT),
                GuardMode::Strict => Entry::worded(key::GUARD_PERSISTED, key::GUARD_STRICT),
                _ => Entry::of(key::GUARD_PERSISTED, "—"),
            },
        );

        seam(screen);
        // WHAT THE TOLERANT MODE COSTS, said before it is chosen and not after.
        line_of(screen, Run::label(key::GUARD_COST).small().quiet());
        // AND THAT A SESSION CHANGE IS ONLY A SESSION CHANGE.
        if g.mode != g.persisted && g.mode != GuardMode::Unknown {
            line_of(screen, Run::label(key::GUARD_SESSION_ONLY).small().accent());
        }

        self.footer(screen, key::HELP_GUARD);
    }

    /// The last line: a pending confirmation wins over the help.
    fn footer(&self, screen: &mut Screen<Wit>, help_key: &str) {
        let at = last_row(screen);
        match self.pending {
            Some(p) => {
                let text = match p {
                    Pending::Stop => key::CONFIRM_STOP,
                    Pending::Sweep(_) => key::CONFIRM_SWEEP,
                    Pending::Persist => key::CONFIRM_PERSIST,
                };
                screen.band(at);
                // THE ONE ROLE LEFT FOR "THIS ONE": the three confirmations
                // were critical, warning and warning, and the sentences are
                // what tell them apart now. See `alarm`.
                screen.put(at, Run::label(text).base().accent());
            }
            None if self.asked => {
                screen.put(at, Run::label(key::ASKED).small().quiet());
            }
            None => help(screen, help_key),
        }
    }

    /// Keeps the cursor on screen after a move.
    fn follow(&mut self) {
        // HOW MANY ROWS THE LIST ACTUALLY HAS, from the last `render`. It used
        // to be `lines - 10`, the block above the list measured once by hand
        // and then wrong on every screen whose detail grew — and `list_top` is
        // already recorded, for the click, for exactly the same reason.
        let first = self.list_top.unwrap_or(0);
        let room = usize::from(self.lines.saturating_sub(1).saturating_sub(first)).max(1);
        if self.cursor < self.top {
            self.top = self.cursor;
        } else if self.cursor >= self.top + room {
            self.top = self.cursor + 1 - room;
        }
    }

    /// What a command id does. `None` when it is not ours.
    fn command(&mut self, id: &str) -> Vec<Effect> {
        // ANY COMMAND CLEARS A PENDING CONFIRMATION, except the one that
        // repeats it: nothing irreversible stays armed while the person is
        // doing something else.
        let repeated = self.pending;
        self.pending = None;
        self.asked = false;

        let ask = |what: Request, target: u64| vec![Effect::Ask(Demand { what, target })];

        match id {
            "memory-full" => {
                self.panel = Panel::Full(Tab::Ram);
                vec![Effect::Redraw]
            }
            "memory-guard" => {
                self.panel = Panel::Guard;
                vec![Effect::Redraw]
            }
            "memory-tab" => {
                self.panel = match self.panel {
                    Panel::Full(Tab::Ram) => Panel::Full(Tab::Disk),
                    _ => Panel::Full(Tab::Ram),
                };
                self.cursor = 0;
                self.top = 0;
                vec![Effect::Redraw]
            }
            "memory-refresh" => {
                self.asked = true;
                ask(Request::Refresh, 0)
            }
            "memory-scan" => {
                self.cursor = 0;
                self.top = 0;
                self.asked = true;
                match self.panel {
                    Panel::Full(Tab::Disk) => ask(Request::ScanDisk, 0),
                    _ => ask(Request::ScanRam, 0),
                }
            }
            "memory-scan-cancel" => ask(Request::CancelScan, 0),
            // The three that need a second press. The FIRST one arms; the same
            // key again fires.
            "memory-stop" => self.arm(repeated, Pending::Stop),
            "memory-sweep-store" => self.arm(repeated, Pending::Sweep(Request::SweepStore)),
            "memory-sweep-generations" => {
                self.arm(repeated, Pending::Sweep(Request::SweepGenerations))
            }
            "memory-sweep-journal" => self.arm(repeated, Pending::Sweep(Request::SweepJournal)),
            "memory-persist" => self.arm(repeated, Pending::Persist),
            // The switch does NOT ask: it is reversible by the next key, and a
            // confirmation for something one undoes with one press is a
            // confirmation that teaches people to press through them.
            "memory-tolerant" => {
                self.asked = true;
                ask(Request::GuardTolerant, 0)
            }
            "memory-default" => {
                self.asked = true;
                ask(Request::GuardDefault, 0)
            }
            _ => Vec::new(),
        }
    }

    /// A survey arrived.
    ///
    /// A method and not a branch inside `event` so the tests can drive it
    /// without a thread-local — and so the "did anything change" rule has ONE
    /// place, instead of one in the code and a copy in the test.
    fn receive(&mut self, survey: Survey) -> Vec<Effect> {
        // IT ONLY REDRAWS WHEN SOMETHING CHANGED. A screen that repaints every
        // second to show identical numbers spends a frame per second for
        // nothing — and this buffer is open exactly when the machine has none
        // to spare.
        let same = same_survey(&survey, &self.survey);
        self.survey = survey;
        // The list can shrink under the cursor — a scan that found less, or
        // something that was closed.
        if self.cursor >= self.survey.consumers.len() {
            self.cursor = self.survey.consumers.len().saturating_sub(1);
            self.top = self.top.min(self.cursor);
        }
        if same { vec![] } else { vec![Effect::Redraw] }
    }

    /// Arms a confirmation, or fires it when it is the same one again.
    fn arm(&mut self, repeated: Option<Pending>, what: Pending) -> Vec<Effect> {
        if repeated != Some(what) {
            self.pending = Some(what);
            return vec![Effect::Redraw];
        }
        self.asked = true;
        match what {
            Pending::Stop => {
                // 1-BASED, and it is the index of the list the screen is
                // showing: the host holds the same list and checks it before
                // acting. A pid would not name a scope, and a scope is what one
                // usually wants to close.
                let target = u64::try_from(self.cursor + 1).unwrap_or(0);
                vec![Effect::Ask(Demand {
                    what: Request::StopConsumer,
                    target,
                })]
            }
            Pending::Sweep(request) => vec![Effect::Ask(Demand {
                what: request,
                target: 0,
            })],
            Pending::Persist => vec![Effect::Ask(Demand {
                what: Request::GuardPersist,
                target: 0,
            })],
        }
    }
}

/// Did anything the screen shows change?
///
/// Field by field, and by hand. The bindings belong to `ronoctua-design` now —
/// generated ONCE for the seven buffers — and its `generate!` does not ask for
/// `additional_derives: [PartialEq]`, which this buffer used to ask for in its
/// own. Asking for it there would give every buffer a comparison none of the
/// others needs; asking for it here is not possible, because the types are not
/// this crate's. So the comparison is written out, and it is three lines.
fn same_survey(a: &Survey, b: &Survey) -> bool {
    a.scanning == b.scanning
        && a.progress == b.progress
        && same_guard(&a.guard, &b.guard)
        && a.readings.len() == b.readings.len()
        && a.consumers.len() == b.consumers.len()
        && (a.readings.iter().zip(&b.readings)).all(|(x, y)| x.of == y.of && x.bytes == y.bytes)
        && (a.consumers.iter().zip(&b.consumers)).all(|(x, y)| {
            x.kind == y.kind && x.bytes == y.bytes && x.hint == y.hint && x.name == y.name
        })
}

/// The protection, field by field. See [`same_survey`].
fn same_guard(a: &Guard, b: &Guard) -> bool {
    a.mode == b.mode
        && a.pressure_limit == b.pressure_limit
        && a.pressure_seconds == b.pressure_seconds
        && a.ceiling == b.ceiling
        && a.acted == b.acted
        && a.zram == b.zram
        && a.persisted == b.persisted
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "memory".into(),
            mode: "memory".into(),
            // THE THREE SCREENS this panel always had: the live overview, the
            // full breakdown and the protection. `Esc` already walked back from
            // the last two to the first — what it lacked was the modeline
            // saying which of the three it had walked back from.
            states: vec![
                BufferState {
                    id: "normal".into(),
                    letter: 'n',
                    tone: Tone::Standard,
                },
                BufferState {
                    id: "full".into(),
                    letter: 'f',
                    tone: Tone::Standard,
                },
                BufferState {
                    id: "guard".into(),
                    letter: 'g',
                    tone: Tone::Standard,
                },
            ],
            keymap: KEYMAP
                .iter()
                .map(|(keys, command)| Binding {
                    keys: (*keys).into(),
                    command: (*command).into(),
                })
                .collect(),
            commands: KEYMAP.iter().map(|(_, c)| (*c).into()).collect(),
            segments: vec![],
            // THE ONE THAT MATTERS: without it the host neither samples nor
            // performs, and this buffer is an empty screen. It is declared
            // here, once, and the host checks it on every request.
            capabilities: vec![Capability::System],
            // Nothing it holds is text of the interface: everything this buffer
            // shows goes out as a key and the host translates it when it draws.
            phrases: vec![],
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        STATE.with_borrow_mut(|s| {
            s.lines = ctx.lines;
            let mut screen = Screen::<Wit>::of(&ctx);
            match s.panel {
                // No cursor on these two, so a click has no row to land on —
                // and `None` is what makes `event` ignore it instead of
                // guessing.
                Panel::Overview => {
                    s.list_top = None;
                    s.overview(&mut screen);
                }
                Panel::Full(tab) => {
                    s.list_top = Some(s.full(tab, &mut screen));
                }
                Panel::Guard => {
                    s.list_top = None;
                    s.guard(&mut screen);
                }
            }
            screen.finish_wit()
        })
    }

    fn event(e: Input) -> Vec<Effect> {
        // THE STATE IS REPORTED FROM ONE PLACE, and not at each site that
        // assigns `panel` — there are four, between the commands and the two
        // layers of `Cancel`. A report per site is a report somebody forgets.
        STATE.with_borrow_mut(|s| {
            let before = s.state_id();
            let mut out = s.act(e);
            let after = s.state_id();
            if before != after {
                out.push(Effect::StateChanged(after.into()));
            }
            out
        })
    }
}

impl State {
    /// Which of the declared states this is, right now.
    ///
    /// DERIVED and never stored: `panel` already answers it, and a stored copy
    /// is a second truth that drifts on the first path that forgets it.
    const fn state_id(&self) -> &'static str {
        match self.panel {
            Panel::Overview => "normal",
            Panel::Full(_) => "full",
            Panel::Guard => "guard",
        }
    }

    /// One input, before the state is compared.
    fn act(&mut self, e: Input) -> Vec<Effect> {
        let s = self;
        match e {
            // THE HOST SENDS AN ID THIS BUFFER DECLARED. It also clears what is
            // armed: leaving a `pending` alive under another screen is a key
            // waiting for an `Enter` nobody can see — and on this panel what is
            // armed CLOSES a process.
            Input::EnterState(id) => {
                s.pending = None;
                s.panel = match id.as_str() {
                    // The tab it lands on is the FIRST, and not the one it was
                    // last on: a state entered by name has to land somewhere
                    // predictable, and the memory half is the one this panel is
                    // named after.
                    "full" => Panel::Full(Tab::Ram),
                    "guard" => Panel::Guard,
                    _ => Panel::Overview,
                };
                vec![Effect::Redraw]
            }
            // THE SURVEY, which is the only input nobody pressed. It redraws
            // only when something CHANGED: a screen that repaints every second
            // for identical numbers spends a frame to show the same thing.
            Input::Survey(survey) => s.receive(survey),
            Input::Command(id) => s.command(&id),
            Input::Down => {
                if s.cursor + 1 < s.survey.consumers.len() {
                    s.cursor += 1;
                    s.follow();
                }
                vec![Effect::Redraw]
            }
            Input::Up => {
                s.cursor = s.cursor.saturating_sub(1);
                s.follow();
                vec![Effect::Redraw]
            }
            Input::Toggle => s.command("memory-tab"),
            // `Enter` confirms what is armed, and does nothing otherwise —
            // never "confirms the obvious action", which is how a confirmation
            // becomes a formality.
            Input::Confirm => match s.pending {
                Some(p) => {
                    let again = Some(p);
                    s.pending = p.into();
                    s.arm(again, p)
                }
                None => vec![],
            },
            // `Esc` in two layers: it clears a pending confirmation first, and
            // only then goes back — the key one presses by reflex must not
            // leave the screen while something irreversible is armed.
            Input::Cancel => {
                if s.pending.take().is_some() {
                    return vec![Effect::Redraw];
                }
                if s.panel == Panel::Overview {
                    return vec![];
                }
                s.panel = Panel::Overview;
                vec![Effect::Redraw]
            }
            // A click puts the cursor on the row that was clicked, when there
            // is one there: the buffer knows its own layout, and the host does
            // not get to guess it.
            Input::Click(p) => {
                // THE ROW THE LIST STARTED ON, recorded by the last `render` —
                // never a constant. The detail above the list has a different
                // height on each screen and grows with what the machine
                // reports, so a fixed offset would put the cursor on a
                // neighbour of the row that was clicked. On this panel the
                // cursor decides what gets CLOSED.
                let Some(first) = s.list_top else {
                    return vec![];
                };
                if p.line >= first {
                    let n = s.top + usize::from(p.line - first);
                    if n < s.survey.consumers.len() {
                        s.cursor = n;
                    }
                }
                vec![Effect::Redraw]
            }
            _ => vec![],
        }
    }
}

ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_design::types::{Heights, Reading, Text, TextCommand};

    /// The production heights: `typography.nix`'s five sizes against a line
    /// height of 1.45, as `Heights::from_sizes` computes them. `Normal` is the
    /// cell, so it is one; `Large` and `Huge` are two, which is the case every
    /// geometry defect in this repository came from.
    const fn heights() -> Heights {
        Heights {
            small: 1,
            base: 1,
            normal: 1,
            large: 2,
            huge: 2,
        }
    }

    fn context(columns: u16, lines: u16) -> Context {
        Context {
            columns,
            lines,
            heights: heights(),
        }
    }

    fn survey_with(readings: Vec<(Gauge, u64)>) -> Survey {
        let mut s = empty_survey();
        s.readings = readings
            .into_iter()
            .map(|(of, bytes)| Reading { of, bytes })
            .collect();
        s
    }

    fn consumers(n: usize) -> Vec<Consumer> {
        (0..n)
            .map(|i| Consumer {
                kind: ConsumerKind::Scope,
                name: format!("app-{i}.scope which has a rather long name"),
                bytes: 1024 * 1024 * (i as u64 + 1),
                hint: i as u64,
            })
            .collect()
    }

    /// A machine that reports everything it can, so the drawing below has every
    /// optional line in it.
    fn a_full_house() -> State {
        let mut s = State::fresh();
        s.survey = survey_with(vec![
            (Gauge::RamTotal, 16 * 1024 * 1024 * 1024),
            (Gauge::RamAvailable, 7 * 1024 * 1024 * 1024),
            (Gauge::RamAnonymous, 5 * 1024 * 1024 * 1024),
            (Gauge::RamCached, 3 * 1024 * 1024 * 1024),
            (Gauge::RamShmem, 512 * 1024 * 1024),
            (Gauge::RamKernel, 700 * 1024 * 1024),
            (Gauge::RamDirty, 40 * 1024 * 1024),
            (Gauge::ZramOriginal, 2 * 1024 * 1024 * 1024),
            (Gauge::ZramCompressed, 700 * 1024 * 1024),
            (Gauge::SwapTotal, 8 * 1024 * 1024 * 1024),
            (Gauge::SwapFree, 6 * 1024 * 1024 * 1024),
            (Gauge::DiskTotal, 1024 * 1024 * 1024 * 1024),
            (Gauge::DiskFree, 300 * 1024 * 1024 * 1024),
            (Gauge::DesktopCurrent, 2 * 1024 * 1024 * 1024),
            (Gauge::DesktopMin, 1024 * 1024 * 1024),
            (Gauge::AppCurrent, 6 * 1024 * 1024 * 1024),
            (Gauge::AppCeiling, 12 * 1024 * 1024 * 1024),
            (Gauge::SystemCurrent, 900 * 1024 * 1024),
            (Gauge::VramUsed, 800 * 1024 * 1024),
            (Gauge::VramTotal, 2 * 1024 * 1024 * 1024),
        ]);
        s.survey.pressure.some10 = 123;
        s.survey.pressure.some60 = 45;
        s.survey.guard = Guard {
            mode: GuardMode::Tolerant,
            pressure_limit: 100,
            pressure_seconds: 20,
            ceiling: 800,
            acted: 3,
            zram: true,
            persisted: GuardMode::Strict,
        };
        s.survey.consumers = consumers(40);
        s
    }

    /// Every panel there is, with the survey and the pending confirmation that
    /// make the most lines appear.
    fn every_panel() -> Vec<(&'static str, State)> {
        let mut out = Vec::new();
        for (name, panel) in [
            ("normal", Panel::Overview),
            ("full/ram", Panel::Full(Tab::Ram)),
            ("full/disk", Panel::Full(Tab::Disk)),
            ("guard", Panel::Guard),
        ] {
            // As it comes up, with nothing measured yet.
            let mut empty = State::fresh();
            empty.panel = panel;
            out.push((name, empty));
            // Measured, with the cursor down the list.
            let mut full = a_full_house();
            full.panel = panel;
            full.cursor = 12;
            full.top = 6;
            out.push((name, full));
            // Scanning, with something irreversible armed on the last row.
            let mut armed = a_full_house();
            armed.panel = panel;
            armed.survey.scanning = true;
            armed.survey.progress = 421;
            armed.pending = Some(Pending::Stop);
            out.push((name, armed));
            // Waiting for the host to answer.
            let mut asked = a_full_house();
            asked.panel = panel;
            asked.asked = true;
            asked.survey.consumers.clear();
            out.push((name, asked));
        }
        out
    }

    fn draw(s: &State, columns: u16, lines: u16) -> Screen<Wit> {
        let ctx = context(columns, lines);
        let mut screen = Screen::<Wit>::of(&ctx);
        match s.panel {
            Panel::Overview => s.overview(&mut screen),
            Panel::Full(tab) => {
                s.full(tab, &mut screen);
            }
            Panel::Guard => s.guard(&mut screen),
        }
        screen
    }

    /// THE HOST'S OWN TWO QUESTIONS, asked here in milliseconds instead of in
    /// Nix in minutes: `nothing_is_covered` and `nothing_overlaps`, on the real
    /// `DisplayList` this panel produces, in every state and at the three
    /// widths the footer, a half screen and a whole one actually have.
    #[test]
    fn every_screen_answers_the_hosts_two_questions() {
        for (name, s) in every_panel() {
            for columns in [46u16, 80, 110] {
                for lines in [8u16, 12, 24, 40] {
                    draw(&s, columns, lines)
                        .check()
                        .unwrap_or_else(|e| panic!("{name} em {columns}x{lines}: {e}"));
                }
            }
        }
    }

    /// And at every width from a footer's to a wide window's, which is where a
    /// column that was placed by hand used to leak.
    #[test]
    fn no_width_between_twenty_and_two_hundred_leaks() {
        for (name, s) in every_panel() {
            for columns in 20..=200u16 {
                draw(&s, columns, 24)
                    .check()
                    .unwrap_or_else(|e| panic!("{name} at {columns} columns: {e}"));
            }
        }
    }

    /// NO FIELD ENDS PAST THE RIGHT EDGE, read off the commands themselves.
    ///
    /// `check` already refuses it, through the host's own arithmetic; this says
    /// the same thing about the budget a run DECLARES, which is the number the
    /// host cuts by and the one this file used to compute at each call site.
    #[test]
    fn no_budget_reaches_past_the_edge() {
        for (name, s) in every_panel() {
            for columns in [46u16, 80, 110] {
                for c in draw(&s, columns, 24).finish_wit() {
                    let Command::Text(TextCommand {
                        column, max_width, ..
                    }) = c
                    else {
                        continue;
                    };
                    // `0` is the protocol's own spelling for "to the edge", and
                    // the host applies the edge itself.
                    assert!(
                        max_width == 0 || column + max_width <= columns,
                        "{name}: a field at {column} asks for {max_width} in a window of {columns}"
                    );
                }
            }
        }
    }

    /// THE HEADING IS A GLYPH, A TITLE AND NO RULE — which is what this panel
    /// always drew and what every buffer draws now.
    ///
    /// A rule inside a heading is the defect: what separates a title from its
    /// content is whitespace, and only whitespace.
    #[test]
    fn the_heading_draws_no_rule() {
        let ctx = context(80, 24);
        let mut screen = Screen::<Wit>::of(&ctx);
        let content = screen.heading(Icon::Settings, key::TITLE);
        let drawn = screen.finish_wit();
        assert_eq!(drawn.len(), 2, "a glyph and a title, and nothing else");
        assert!(
            !drawn.iter().any(|c| matches!(c, Command::Rule(_))),
            "a rule showed up in a header"
        );
        // A glyph, and then a title that is a KEY — never a sentence.
        assert!(matches!(
            &drawn[0],
            Command::Text(TextCommand {
                content: Text::Icon(_),
                column: 0,
                ..
            })
        ));
        assert!(matches!(
            &drawn[1],
            Command::Text(TextCommand {
                content: Text::Key(_),
                ..
            })
        ));
        // The air above and below is one cell each: the content starts on the
        // fourth row of the block.
        assert_eq!(content, AIR + 1 + AIR);
    }

    /// THE RULES THAT ARE LEFT SEPARATE BLOCKS, and there are no others.
    ///
    /// Two on the overview, one on the full screen and two on the protection —
    /// the same five call sites, and not one of them inside a heading.
    #[test]
    fn the_only_rules_are_between_blocks() {
        let rules = |s: &State| {
            draw(s, 80, 24)
                .finish_wit()
                .iter()
                .filter(|c| matches!(c, Command::Rule(_)))
                .count()
        };
        let mut s = a_full_house();
        s.panel = Panel::Overview;
        assert_eq!(rules(&s), 2);
        s.panel = Panel::Full(Tab::Ram);
        assert_eq!(rules(&s), 1);
        s.panel = Panel::Guard;
        assert_eq!(rules(&s), 2);
    }

    /// THE MEASUREMENTS ALL BEGIN IN ONE COLUMN, the sheet's — the readings',
    /// the protection's and the consumer list's alike. Three different answers
    /// to "where does a value start" is what this panel's `VALUE_AT = 16`
    /// became.
    #[test]
    fn every_value_starts_where_the_sheet_says() {
        let split = sheet_split(80);
        let mut s = a_full_house();
        s.panel = Panel::Overview;
        let value_columns: Vec<u16> = draw(&s, 80, 24)
            .finish_wit()
            .iter()
            .filter_map(|c| match c {
                Command::Text(t) => Some(t.column),
                _ => None,
            })
            .filter(|column| *column >= split.value)
            .collect();
        assert!(!value_columns.is_empty(), "no value was drawn");
        // Nothing between the label's edge and the sheet's value column.
        assert!(
            value_columns.iter().all(|c| *c >= split.value),
            "a value started before the sheet's column"
        );
    }

    #[test]
    fn a_size_reads_like_a_person_writes_it() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1536), "1 KiB");
        assert_eq!(size(14_969_096 * 1024), "14.2 GiB");
        assert_eq!(size(812 * 1024 * 1024), "812 MiB");
    }

    #[test]
    fn a_share_of_nothing_is_not_zero_percent() {
        // Dividing by a total the machine did not report would print `0.0%` for
        // everything — a screen that looks measured and is not.
        assert_eq!(share(100, 0), "—");
        assert_eq!(share(500, 1000), "50.0%");
    }

    #[test]
    fn the_confirmation_takes_two_presses_and_a_stray_key_disarms_it() {
        let mut s = State::fresh();
        // The first press only arms: nothing goes out.
        assert!(matches!(
            s.command("memory-sweep-store").as_slice(),
            [Effect::Redraw]
        ));
        assert_eq!(s.pending, Some(Pending::Sweep(Request::SweepStore)));
        // A different command clears it — and does NOT fire it.
        let other = s.command("memory-refresh");
        assert!(matches!(other.as_slice(), [Effect::Ask(_)]));
        assert_eq!(s.pending, None);
        // Twice in a row fires.
        s.command("memory-sweep-store");
        assert!(matches!(
            s.command("memory-sweep-store").as_slice(),
            [Effect::Ask(Demand {
                what: Request::SweepStore,
                target: 0
            })]
        ));
    }

    #[test]
    fn stopping_names_the_row_by_index_and_not_by_pid() {
        let mut s = State::fresh();
        s.survey.consumers = consumers(2);
        s.cursor = 1;
        s.command("memory-stop");
        // 1-based: the second row is `2`, and never the pid — a scope has none.
        assert!(matches!(
            s.command("memory-stop").as_slice(),
            [Effect::Ask(Demand {
                what: Request::StopConsumer,
                target: 2
            })]
        ));
    }

    #[test]
    fn an_unchanged_survey_does_not_ask_for_a_frame() {
        let mut s = State::fresh();
        let survey = survey_with(vec![(Gauge::RamTotal, 16)]);
        assert!(
            matches!(s.receive(survey.clone()).as_slice(), [Effect::Redraw]),
            "the first sample changes the screen"
        );
        assert!(
            s.receive(survey).is_empty(),
            "the same sample again does not spend a frame"
        );
        // And a different number does ask for one.
        assert!(matches!(
            s.receive(survey_with(vec![(Gauge::RamTotal, 17)]))
                .as_slice(),
            [Effect::Redraw]
        ));
    }

    /// A consumer that changed only its NAME is a change too — the comparison
    /// is written out by hand now, and a field left out of it is a screen that
    /// stops repainting.
    #[test]
    fn a_survey_that_differs_in_any_field_asks_for_a_frame() {
        let mut s = State::fresh();
        s.receive(empty_survey());
        for change in [
            &|v: &mut Survey| v.scanning = true as _,
            &|v: &mut Survey| v.progress = 1 as _,
            &|v: &mut Survey| v.guard.acted = 9 as _,
            &|v: &mut Survey| v.consumers = consumers(1) as _,
            &|v: &mut Survey| v.readings = survey_with(vec![(Gauge::RamTotal, 1)]).readings as _,
        ] as [&dyn Fn(&mut Survey); 5]
        {
            let mut before = State::fresh();
            let mut after = empty_survey();
            change(&mut after);
            before.receive(empty_survey());
            assert!(
                matches!(before.receive(after).as_slice(), [Effect::Redraw]),
                "a different field asked for no frame"
            );
        }
        assert!(s.receive(empty_survey()).is_empty());
    }

    #[test]
    fn escape_disarms_before_it_leaves() {
        let mut s = State::fresh();
        s.panel = Panel::Full(Tab::Disk);
        s.command("memory-sweep-journal");
        assert!(s.pending.is_some());
        // First `Esc`: disarms and STAYS.
        assert!(matches!(s.act(Input::Cancel).as_slice(), [Effect::Redraw]));
        assert!(s.pending.is_none());
        assert_eq!(s.panel, Panel::Full(Tab::Disk));
        // The second one goes back.
        s.act(Input::Cancel);
        assert_eq!(s.panel, Panel::Overview);
    }

    #[test]
    fn the_cursor_does_not_outlive_the_list() {
        let mut s = State::fresh();
        s.survey.consumers = consumers(1);
        s.cursor = 5;
        s.receive(empty_survey());
        assert_eq!(s.cursor, 0, "the list shrank and the cursor came with it")
    }

    /// THE CURSOR FOLLOWS THE LIST THE LAST FRAME ACTUALLY DREW.
    ///
    /// It used to follow `lines - 10`, a guess at the block above it. The row
    /// the list starts on is already recorded for the click, and it is the same
    /// number.
    #[test]
    fn the_cursor_follows_the_list_that_was_drawn() {
        let mut s = a_full_house();
        s.panel = Panel::Full(Tab::Ram);
        s.lines = 24;
        s.list_top = Some(draw(&s, 80, 24).row().min(23));
        let first = s.list_top.expect("the list was drawn");
        s.cursor = 30;
        s.follow();
        // Whatever the block above cost, the cursor is on a row the window has.
        let room = usize::from(23 - first).max(1);
        assert!(s.top <= s.cursor, "the top went past the cursor");
        assert!(s.cursor < s.top + room, "the cursor landed off the page");
    }
}
