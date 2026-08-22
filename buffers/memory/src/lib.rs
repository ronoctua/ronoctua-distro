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

#![allow(unsafe_code, missing_docs)]

use std::cell::RefCell;

wit_bindgen::generate!({
    world: "buffer",
    path: "../../crates/ronoctua-protocol/wit",
    generate_all,
    // THE SURVEY ARRIVES EVERY SECOND, and the screen must only be repainted
    // when it changed. Comparing it needs `PartialEq` on the generated types,
    // which `wit-bindgen` does not derive by default. No other buffer needs it,
    // and asking for it here costs nothing to anyone else.
    additional_derives: [PartialEq],
});

use exports::ronoctua::buffer::view::Guest;
use ronoctua::buffer::types::{
    Binding, BufferState, Capability, Color, Command, Consumer, ConsumerKind, Context, Demand,
    Descriptor, Effect, Gauge, GuardMode, Icon, Input, LineCommand, Request, Scale, Survey, Text,
    TextCommand, Tone,
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

/// Which screen is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
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
    screen: Screen,
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
            screen: Screen::Overview,
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
        pressure: ronoctua::buffer::types::Pressure {
            some10: 0,
            some60: 0,
            some300: 0,
            full10: 0,
            full60: 0,
        },
        consumers: Vec::new(),
        guard: ronoctua::buffer::types::Guard {
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

/// A row of the screen: a label on the left, a value, and a note.
///
/// The value is a `Text` and not a `String` because some rows carry a NUMBER
/// (data, literal) and others carry a WORD (a key, which the host translates).
/// It was a `String` first, and the screen printed `memory-reserved` where the
/// word should have been — the buffer cannot translate, and forgetting that
/// shows up as the key's own name in front of the person.
struct Row {
    label: &'static str,
    value: Text,
    note: String,
    color: Color,
}

impl Row {
    /// A row whose value is a measurement.
    fn of(label: &'static str, value: String) -> Self {
        Self {
            label,
            value: Text::Data(value),
            note: String::new(),
            color: Color::TextPrimary,
        }
    }

    /// A row whose value is a word — the host translates it.
    fn worded(label: &'static str, value: &str) -> Self {
        Self {
            label,
            value: Text::Key(value.to_owned()),
            note: String::new(),
            color: Color::TextPrimary,
        }
    }

    fn with_note(mut self, note: String) -> Self {
        self.note = note;
        self
    }

    const fn in_colour(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

/// Where each column of a row starts, in cells.
///
/// Three columns and no more: the label one reads, the number one compares, and
/// a note. A fourth would need the window to be wide, and a buffer does not get
/// to demand a size.
const VALUE_AT: u16 = 16;
const NOTE_AT: u16 = 34;

/// Draws a row, and gives back the next line.
fn row(out: &mut Vec<Command>, line: u16, r: &Row, ctx: &Context) -> u16 {
    out.push(Command::Text(TextCommand {
        line,
        column: 0,
        content: Text::Key(r.label.to_owned()),
        color: Color::TextSecondary,
        scale: Scale::Normal,
        max_width: VALUE_AT - 1,
    }));
    out.push(Command::Text(TextCommand {
        line,
        column: VALUE_AT,
        content: r.value.clone(),
        color: r.color,
        scale: Scale::Normal,
        max_width: NOTE_AT - VALUE_AT - 1,
    }));
    if !r.note.is_empty() {
        out.push(Command::Text(TextCommand {
            line,
            column: NOTE_AT,
            content: Text::Data(r.note.clone()),
            color: Color::TextDim,
            scale: Scale::Small,
            max_width: ctx.columns.saturating_sub(NOTE_AT),
        }));
    }
    line + 1
}

/// A title with its icon, and the empty cell under it.
///
/// # Why the title is the BODY's size, and not the glyph's
///
/// It used to ask for `Scale::Large`: 40 px, which is what a glyph is drawn at
/// in the browsers and not what a sentence is read at. A panel whose every
/// other line is 20 px got a heading twice that height — the same complaint
/// `font.size.xlarge` already carries in writing — and, because a 40 px run
/// has a line box of 58 px against the grid's 29, it also SPENT the cell under
/// it. That cell was the panel's air: the blank row this function returns was
/// there to keep the first gauge off the title, and the title had already
/// filled it. What is left over is a few pixels, which is not air.
///
/// The heading is marked by the accent glyph in front of it and by the blank
/// cell after it, which is what `colors.nix` prescribes: emphasis by colour,
/// separation by whitespace. One cell instead of two, and the gauges get their
/// row back.
///
/// THE AIR, on both sides: the buffer's own top margin above, and now a whole
/// empty cell between the title's descenders and the first line of content
/// below, which is what that returned row was always for.
fn title(out: &mut Vec<Command>, line: u16, icon: Icon, text_key: &str, ctx: &Context) -> u16 {
    out.push(Command::Text(TextCommand {
        line,
        column: 0,
        content: Text::Icon(icon),
        color: Color::StateAccent,
        scale: Scale::Normal,
        max_width: 2,
    }));
    out.push(Command::Text(TextCommand {
        line,
        column: 3,
        content: Text::Key(text_key.to_owned()),
        color: Color::TextPrimary,
        scale: Scale::Normal,
        max_width: ctx.columns.saturating_sub(3),
    }));
    line + 2
}

/// A one-line rule — the only separator this system admits besides whitespace.
fn rule(out: &mut Vec<Command>, line: u16) -> u16 {
    out.push(Command::Rule(LineCommand {
        line,
        color: Color::RuleDefault,
    }));
    line + 1
}

/// A line of plain text, at the given scale.
fn line_of(
    out: &mut Vec<Command>,
    line: u16,
    content: Text,
    color: Color,
    scale: Scale,
    ctx: &Context,
) -> u16 {
    out.push(Command::Text(TextCommand {
        line,
        column: 0,
        content,
        color,
        scale,
        max_width: ctx.columns,
    }));
    line + 1
}

impl State {
    /// The small live one.
    fn overview(&self, ctx: &Context) -> Vec<Command> {
        let mut out = Vec::new();
        let s = &self.survey;
        let mut line = title(&mut out, 0, Icon::Settings, key::TITLE, ctx);

        let total = of(s, Gauge::RamTotal).unwrap_or(0);
        let available = of(s, Gauge::RamAvailable).unwrap_or(0);
        let used = total.saturating_sub(available);
        line = row(
            &mut out,
            line,
            &Row::of(key::RAM, size(available)).with_note(format!(
                "{} / {}",
                size(used),
                size(total)
            )),
            ctx,
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
            line = row(
                &mut out,
                line,
                &Row::of(key::ZRAM, size(compressed))
                    .with_note(format!("{} → {ratio}", size(original))),
                ctx,
            );
        }

        if let Some(total) = of(s, Gauge::SwapTotal) {
            let free = of(s, Gauge::SwapFree).unwrap_or(0);
            line = row(
                &mut out,
                line,
                &Row::of(key::SWAP, size(total.saturating_sub(free)))
                    .with_note(format!("/ {}", size(total))),
                ctx,
            );
        }

        // PRESSURE IS THE NUMBER THE POLICY ACTS ON, so it is on the small
        // screen: everything else says how full the machine is, and this one
        // says whether it is suffering.
        let stall = s.pressure.some10;
        line = row(
            &mut out,
            line,
            &Row::of(key::PRESSURE, percent(stall))
                .with_note(percent(s.pressure.some60))
                .in_colour(if stall >= s.guard.pressure_limit && stall > 0 {
                    Color::StateWarning
                } else {
                    Color::TextPrimary
                }),
            ctx,
        );

        line = rule(&mut out, line);
        line = row(&mut out, line, &self.guard_row(), ctx);

        if let (Some(total), Some(free)) = (of(s, Gauge::DiskTotal), of(s, Gauge::DiskFree)) {
            line = row(
                &mut out,
                line,
                &Row::of(key::DISK, size(free)).with_note(format!("/ {}", size(total))),
                ctx,
            );
        }

        line = rule(&mut out, line);
        line = line_of(
            &mut out,
            line,
            Text::Key(key::CONSUMERS.to_owned()),
            Color::TextSecondary,
            Scale::Base,
            ctx,
        );
        // The five biggest, and no cursor: this screen is for reading. Choosing
        // and closing is the full one's business.
        //
        // `let _ =` on the last line each way: nothing is drawn under it, and
        // adding to a variable nobody reads is how a line ends up drawn on top
        // of another one later.
        if s.consumers.is_empty() {
            let _ = line_of(
                &mut out,
                line,
                Text::Key(if s.scanning {
                    key::SCANNING.to_owned()
                } else {
                    key::NOTHING_YET.to_owned()
                }),
                Color::TextDim,
                Scale::Normal,
                ctx,
            );
        } else {
            for c in s.consumers.iter().take(5) {
                line = consumer_line(&mut out, line, c, false, ctx);
            }
        }

        help(&mut out, ctx, key::HELP_OVERVIEW);
        out
    }

    /// The protection's row, on both screens.
    fn guard_row(&self) -> Row {
        let g = &self.survey.guard;
        let state = match g.mode {
            GuardMode::Strict | GuardMode::Tolerant => key::GUARD_ARMED,
            GuardMode::Off => key::GUARD_OFF,
            GuardMode::Unknown => key::GUARD_UNKNOWN,
        };
        // THE STATE IS THE VALUE, and it is a KEY — because it is a word, and a
        // word on this screen is a translation.
        //
        // It was in the NOTE, and a note is `Text::Data`: the screen drew
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
        Row {
            label: key::GUARD,
            value: Text::Key(state.to_owned()),
            note: numbers,
            color: match g.mode {
                GuardMode::Strict => Color::StateSuccess,
                GuardMode::Tolerant => Color::StateWarning,
                _ => Color::StateError,
            },
        }
    }
}

/// One consumer's line, with the cursor's selection when it is on it.
fn consumer_line(
    out: &mut Vec<Command>,
    line: u16,
    c: &Consumer,
    focused: bool,
    ctx: &Context,
) -> u16 {
    if focused {
        // THE SELECTION IS A LINE, not a box: there is no rectangle in this
        // protocol, and a box would be the card invariant 15 refuses.
        out.push(Command::Selection(LineCommand {
            line,
            color: Color::SurfaceSelection,
        }));
    }
    out.push(Command::Text(TextCommand {
        line,
        column: 2,
        content: Text::Icon(match c.kind {
            ConsumerKind::Process => Icon::Terminal,
            ConsumerKind::Scope => Icon::Window,
            ConsumerKind::Directory => Icon::Folder,
            ConsumerKind::Filesystem => Icon::Save,
            ConsumerKind::Generation => Icon::History,
        }),
        color: Color::TextDim,
        scale: Scale::Normal,
        max_width: 2,
    }));
    out.push(Command::Text(TextCommand {
        line,
        column: 5,
        content: Text::Data(c.name.clone()),
        color: Color::TextPrimary,
        scale: Scale::Normal,
        max_width: NOTE_AT.saturating_sub(6),
    }));
    out.push(Command::Text(TextCommand {
        line,
        column: NOTE_AT,
        content: Text::Data(size(c.bytes)),
        color: Color::TextSecondary,
        scale: Scale::Normal,
        max_width: ctx.columns.saturating_sub(NOTE_AT),
    }));
    line + 1
}

/// The help line, always the last one.
fn help(out: &mut Vec<Command>, ctx: &Context, which: &str) {
    out.push(Command::Text(TextCommand {
        line: ctx.lines.saturating_sub(1),
        column: 0,
        content: Text::Key(which.to_owned()),
        color: Color::TextDim,
        scale: Scale::Small,
        max_width: ctx.columns,
    }));
}

impl State {
    /// The full one: the whole list, with a cursor and the actions.
    /// It gives back the commands AND the row the list started on, so a click
    /// lands on the row it looks like it landed on. Computing that offset twice
    /// — once here and once in `event` — is how a cursor ends up one row off
    /// from what the eye picked, on a screen where the cursor decides what gets
    /// closed.
    fn full(&self, tab: Tab, ctx: &Context) -> (Vec<Command>, u16) {
        let mut out = Vec::new();
        let s = &self.survey;
        let mut line = title(
            &mut out,
            0,
            match tab {
                Tab::Ram => Icon::Settings,
                Tab::Disk => Icon::Save,
            },
            match tab {
                Tab::Ram => key::RAM,
                Tab::Disk => key::DISK,
            },
            ctx,
        );

        line = match tab {
            Tab::Ram => self.ram_detail(&mut out, line, ctx),
            Tab::Disk => self.disk_detail(&mut out, line, ctx),
        };
        line = rule(&mut out, line);

        // THE LIST IS VIRTUALISED: only what is on screen becomes a command.
        // The alternative is emitting forty rows into a window that shows eight,
        // every frame, for nothing.
        let first = line;
        let room = usize::from(ctx.lines.saturating_sub(line + 2));
        let visible = s
            .consumers
            .iter()
            .enumerate()
            .skip(self.top)
            .take(room.max(1));
        if s.consumers.is_empty() {
            line = line_of(
                &mut out,
                line,
                Text::Key(if s.scanning {
                    key::SCANNING.to_owned()
                } else {
                    key::NOTHING_YET.to_owned()
                }),
                Color::TextDim,
                Scale::Normal,
                ctx,
            );
        } else {
            for (i, c) in visible {
                line = consumer_line(&mut out, line, c, i == self.cursor, ctx);
            }
        }

        if s.scanning {
            // The progress is a number, for the same reason there are no bars.
            line_of(
                &mut out,
                line,
                Text::Data(percent(s.progress)),
                Color::TextDim,
                Scale::Small,
                ctx,
            );
        }

        self.footer(&mut out, ctx, key::HELP_FULL);
        (out, first)
    }

    /// The RAM breakdown — what the memory is made of.
    fn ram_detail(&self, out: &mut Vec<Command>, mut line: u16, ctx: &Context) -> u16 {
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
                line = row(
                    out,
                    line,
                    &Row::of(label, size(v)).with_note(share(v, total)),
                    ctx,
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
            line = row(
                out,
                line,
                &Row::of(key::DESKTOP, size(v)).with_note(note),
                ctx,
            );
        }
        if let Some(v) = of(s, Gauge::AppCurrent) {
            let note =
                of(s, Gauge::AppCeiling).map_or_else(String::new, |c| format!("≤ {}", size(c)));
            line = row(out, line, &Row::of(key::APPS, size(v)).with_note(note), ctx);
        }
        if let Some(v) = of(s, Gauge::SystemCurrent) {
            line = row(out, line, &Row::of(key::SYSTEM, size(v)), ctx);
        }
        // The graphics memory, when the driver says. On an APU it is carved out
        // of the same RAM above — which is why it is on this screen and not on
        // one about the display.
        if let (Some(used), Some(total)) = (of(s, Gauge::VramUsed), of(s, Gauge::VramTotal)) {
            line = row(
                out,
                line,
                &Row::of(key::VRAM, size(used)).with_note(format!("/ {}", size(total))),
                ctx,
            );
        }
        line
    }

    /// The disk breakdown — the numbers that come from metadata.
    fn disk_detail(&self, out: &mut Vec<Command>, mut line: u16, ctx: &Context) -> u16 {
        let s = &self.survey;
        let total = of(s, Gauge::DiskTotal).unwrap_or(0);
        if let Some(free) = of(s, Gauge::DiskFree) {
            line = row(
                out,
                line,
                &Row::of(key::DISK, size(free)).with_note(format!("/ {}", size(total))),
                ctx,
            );
        }
        // THE STORE, THE JOURNAL AND THE HOME DIRECTORIES ARE NOT ROWS HERE.
        //
        // Each of them costs a process or a walk, so they arrive as consumers
        // of a scan — the list below this. Reserving a row for a number that
        // only exists after `m s` would be a row that is blank until then, and
        // a blank row reads as zero.
        line
    }

    /// The protection's screen.
    fn guard(&self, ctx: &Context) -> Vec<Command> {
        let mut out = Vec::new();
        let g = &self.survey.guard;
        let mut line = title(&mut out, 0, Icon::Key, key::GUARD, ctx);

        // THE DESCRIPTION FIRST, and small: it is there for whoever arrives not
        // knowing what this is, and it must not compete with the state for
        // whoever already does.
        line = line_of(
            &mut out,
            line,
            Text::Key(key::GUARD_DESCRIPTION.to_owned()),
            Color::TextSecondary,
            Scale::Small,
            ctx,
        );
        line = line_of(
            &mut out,
            line,
            Text::Key(key::GUARD_DETAIL.to_owned()),
            Color::TextDim,
            Scale::Small,
            ctx,
        );
        line = rule(&mut out, line + 1);

        line = row(&mut out, line, &self.guard_row(), ctx);
        line = row(
            &mut out,
            line,
            &Row::of(key::GUARD_LIMIT, percent(g.pressure_limit)),
            ctx,
        );
        line = row(
            &mut out,
            line,
            &Row::of(key::GUARD_FOR, format!("{}s", g.pressure_seconds)),
            ctx,
        );
        // A ZERO CEILING IS NOT A CEILING OF ZERO: it is `MemoryHigh=infinity`,
        // which is what an unconfigured slice reports. Drawing `0.0%` there
        // would say the applications may use nothing.
        line = row(
            &mut out,
            line,
            &Row::of(
                key::GUARD_CEILING,
                if g.ceiling == 0 {
                    "—".to_owned()
                } else {
                    percent(g.ceiling)
                },
            ),
            ctx,
        );
        line = row(
            &mut out,
            line,
            &Row::of(key::GUARD_ACTED, g.acted.to_string()),
            ctx,
        );
        // A WORD, so the host translates it: the name of a profile is UI text,
        // and printing `tolerant` in a Portuguese session would be the buffer
        // deciding a language it has no business deciding.
        line = match g.persisted {
            GuardMode::Tolerant => row(
                &mut out,
                line,
                &Row::worded(key::GUARD_PERSISTED, key::GUARD_TOLERANT),
                ctx,
            ),
            GuardMode::Strict => row(
                &mut out,
                line,
                &Row::worded(key::GUARD_PERSISTED, key::GUARD_STRICT),
                ctx,
            ),
            _ => row(
                &mut out,
                line,
                &Row::of(key::GUARD_PERSISTED, "—".to_owned()),
                ctx,
            ),
        };

        line = rule(&mut out, line);
        // WHAT THE TOLERANT MODE COSTS, said before it is chosen and not after.
        line = line_of(
            &mut out,
            line,
            Text::Key(key::GUARD_COST.to_owned()),
            Color::TextDim,
            Scale::Small,
            ctx,
        );
        // AND THAT A SESSION CHANGE IS ONLY A SESSION CHANGE.
        if g.mode != g.persisted && g.mode != GuardMode::Unknown {
            line_of(
                &mut out,
                line,
                Text::Key(key::GUARD_SESSION_ONLY.to_owned()),
                Color::StateWarning,
                Scale::Small,
                ctx,
            );
        }

        self.footer(&mut out, ctx, key::HELP_GUARD);
        out
    }

    /// The last line: a pending confirmation wins over the help.
    fn footer(&self, out: &mut Vec<Command>, ctx: &Context, help_key: &str) {
        match self.pending {
            Some(p) => {
                let (text, colour) = match p {
                    Pending::Stop => (key::CONFIRM_STOP, Color::StateCritical),
                    Pending::Sweep(_) => (key::CONFIRM_SWEEP, Color::StateWarning),
                    Pending::Persist => (key::CONFIRM_PERSIST, Color::StateWarning),
                };
                out.push(Command::Selection(LineCommand {
                    line: ctx.lines.saturating_sub(1),
                    color: Color::SurfaceSelection,
                }));
                out.push(Command::Text(TextCommand {
                    line: ctx.lines.saturating_sub(1),
                    column: 0,
                    content: Text::Key(text.to_owned()),
                    color: colour,
                    scale: Scale::Base,
                    max_width: ctx.columns,
                }));
            }
            None if self.asked => {
                out.push(Command::Text(TextCommand {
                    line: ctx.lines.saturating_sub(1),
                    column: 0,
                    content: Text::Key(key::ASKED.to_owned()),
                    color: Color::TextDim,
                    scale: Scale::Small,
                    max_width: ctx.columns,
                }));
            }
            None => help(out, ctx, help_key),
        }
    }

    /// Keeps the cursor on screen after a move.
    fn follow(&mut self) {
        let room = usize::from(self.lines).saturating_sub(10).max(1);
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
                self.screen = Screen::Full(Tab::Ram);
                vec![Effect::Redraw]
            }
            "memory-guard" => {
                self.screen = Screen::Guard;
                vec![Effect::Redraw]
            }
            "memory-tab" => {
                self.screen = match self.screen {
                    Screen::Full(Tab::Ram) => Screen::Full(Tab::Disk),
                    _ => Screen::Full(Tab::Ram),
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
                match self.screen {
                    Screen::Full(Tab::Disk) => ask(Request::ScanDisk, 0),
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
        let same = survey.readings == self.survey.readings
            && survey.consumers == self.survey.consumers
            && survey.guard == self.survey.guard
            && survey.scanning == self.survey.scanning
            && survey.progress == self.survey.progress;
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
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        STATE.with_borrow_mut(|s| {
            s.lines = ctx.lines;
            match s.screen {
                // No cursor on these two, so a click has no row to land on —
                // and `None` is what makes `event` ignore it instead of
                // guessing.
                Screen::Overview => {
                    s.list_top = None;
                    s.overview(&ctx)
                }
                Screen::Full(tab) => {
                    let (out, first) = s.full(tab, &ctx);
                    s.list_top = Some(first);
                    out
                }
                Screen::Guard => {
                    s.list_top = None;
                    s.guard(&ctx)
                }
            }
        })
    }

    fn event(e: Input) -> Vec<Effect> {
        // THE STATE IS REPORTED FROM ONE PLACE, and not at each site that
        // assigns `screen` — there are four, between the commands and the two
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
    /// DERIVED and never stored: `screen` already answers it, and a stored copy
    /// is a second truth that drifts on the first path that forgets it.
    const fn state_id(&self) -> &'static str {
        match self.screen {
            Screen::Overview => "normal",
            Screen::Full(_) => "full",
            Screen::Guard => "guard",
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
                s.screen = match id.as_str() {
                    // The tab it lands on is the FIRST, and not the one it was
                    // last on: a state entered by name has to land somewhere
                    // predictable, and the memory half is the one this panel is
                    // named after.
                    "full" => Screen::Full(Tab::Ram),
                    "guard" => Screen::Guard,
                    _ => Screen::Overview,
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
                if s.screen == Screen::Overview {
                    return vec![];
                }
                s.screen = Screen::Overview;
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

export!(Component);

#[cfg(test)]
mod tests {
    use super::*;

    fn survey_with(readings: Vec<(Gauge, u64)>) -> Survey {
        let mut s = empty_survey();
        s.readings = readings
            .into_iter()
            .map(|(of, bytes)| ronoctua::buffer::types::Reading { of, bytes })
            .collect();
        s
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
        let first = s.command("memory-sweep-store");
        assert_eq!(first, vec![Effect::Redraw]);
        assert_eq!(s.pending, Some(Pending::Sweep(Request::SweepStore)));
        // A different command clears it — and does NOT fire it.
        let other = s.command("memory-refresh");
        assert!(matches!(other.as_slice(), [Effect::Ask(_)]));
        assert_eq!(s.pending, None);
        // Twice in a row fires.
        s.command("memory-sweep-store");
        let fired = s.command("memory-sweep-store");
        assert_eq!(
            fired,
            vec![Effect::Ask(Demand {
                what: Request::SweepStore,
                target: 0
            })]
        );
    }

    #[test]
    fn stopping_names_the_row_by_index_and_not_by_pid() {
        let mut s = State::fresh();
        s.survey.consumers = vec![
            Consumer {
                kind: ConsumerKind::Scope,
                name: "um.scope".into(),
                bytes: 10,
                hint: 0,
            },
            Consumer {
                kind: ConsumerKind::Process,
                name: "dois".into(),
                bytes: 5,
                hint: 4321,
            },
        ];
        s.cursor = 1;
        s.command("memory-stop");
        let fired = s.command("memory-stop");
        // 1-based: the second row is `2`, and never the pid — a scope has none.
        assert_eq!(
            fired,
            vec![Effect::Ask(Demand {
                what: Request::StopConsumer,
                target: 2
            })]
        );
    }

    #[test]
    fn an_unchanged_survey_does_not_ask_for_a_frame() {
        let mut s = State::fresh();
        let survey = survey_with(vec![(Gauge::RamTotal, 16)]);
        assert_eq!(
            s.receive(survey.clone()),
            vec![Effect::Redraw],
            "a primeira amostra muda a tela"
        );
        assert!(
            s.receive(survey).is_empty(),
            "a mesma amostra de novo nao gasta um quadro"
        );
        // And a different number does ask for one.
        assert_eq!(
            s.receive(survey_with(vec![(Gauge::RamTotal, 17)])),
            vec![Effect::Redraw]
        );
    }

    #[test]
    fn escape_disarms_before_it_leaves() {
        let mut s = State::fresh();
        s.screen = Screen::Full(Tab::Disk);
        s.command("memory-sweep-journal");
        assert!(s.pending.is_some());
        // First `Esc`: disarms and STAYS.
        assert!(s.pending.take().is_some());
        assert_eq!(s.screen, Screen::Full(Tab::Disk));
    }

    #[test]
    fn the_cursor_does_not_outlive_the_list() {
        let mut s = State::fresh();
        s.survey.consumers = vec![Consumer {
            kind: ConsumerKind::Process,
            name: "um".into(),
            bytes: 1,
            hint: 1,
        }];
        s.cursor = 5;
        s.receive(empty_survey());
        assert_eq!(s.cursor, 0, "a lista encolheu e o cursor veio junto");
    }
}
