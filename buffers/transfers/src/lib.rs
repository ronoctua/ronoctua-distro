//! The transfer queue, in a window of its own — `LEADER t w`.
//!
//! # The same queue, a second face
//!
//! `LEADER t t` already draws this list inside the footer, under a ceiling of
//! twelve rows. This is the same list with room to breathe: every job at once,
//! a detail screen, and a bar wide enough to read. Both faces ask the same
//! unit over the same socket, and neither of them owns anything — a copy of
//! forty gibibytes belongs to `ronoctua-transfers`, which is why restarting
//! the footer does not touch it.
//!
//! # It reads nothing
//!
//! Not the socket, not the disk, not a clock. The rows arrive through
//! `input::queue`, already read and already cut by the host, and what leaves is
//! `effect::ask` naming a job id. Invariant 11 intact: the component has no
//! parser and is not getting one.
//!
//! # The bar is glyphs, and the glyphs are tokens
//!
//! A cell grid does not stretch, so a bar is N cells of one drawing followed by
//! the rest of another. Both come from `assets/tokens/icons.nix` through
//! `Icon::` — invariant 9 reaches the codepoint, and a block character typed
//! into this file would be a drawing chosen inside a component that has no
//! font.

#![allow(unsafe_code, missing_docs)]

use std::cell::RefCell;

wit_bindgen::generate!({
    world: "buffer",
    path: "../../crates/ronoctua-protocol/wit",
    generate_all,
    // THE QUEUE ARRIVES WHENEVER IT CHANGES, and the screen must only be
    // repainted when what is DRAWN changed — a job whose byte count moved but
    // whose row rounds to the same thing is not worth a frame. Comparing needs
    // `PartialEq` on the generated types, which `wit-bindgen` does not derive.
    additional_derives: [PartialEq],
});

use exports::ronoctua::buffer::view::Guest;
use ronoctua::buffer::types::{
    Amount, Binding, Capability, Color, Command, Context, Demand, Descriptor, Effect, Engine,
    Errand, Icon, Input, Job, LineCommand, Queue, Request, Scale, Stage, Text, TextCommand,
};

/// Every text key this buffer names.
///
/// A module, and every one of them a literal `pub const`: that is what the
/// `buffer-keys-have-text` check can see. Building a key with `format!` would
/// hide it from the check, and the first sign of a missing one would be its own
/// name drawn on screen in front of the person.
mod key {
    pub const TITLE: &str = "transfers-window-title";
    pub const EMPTY: &str = "transfers-window-empty";
    pub const HINT: &str = "transfers-window-hint";
    pub const HINT_DETAIL: &str = "transfers-window-hint-detail";
    pub const CONFIRM_CANCEL: &str = "transfers-window-confirm-cancel";
    pub const ASKED: &str = "transfers-window-asked";
    pub const DETAIL: &str = "transfers-window-detail";
    pub const UNKNOWN: &str = "transfers-window-unknown";

    pub const FIELD_ENGINE: &str = "transfers-window-field-engine";
    pub const FIELD_STAGE: &str = "transfers-window-field-stage";
    pub const FIELD_ERRAND: &str = "transfers-window-field-errand";
    pub const FIELD_SUBJECT: &str = "transfers-window-field-subject";
    pub const FIELD_DONE: &str = "transfers-window-field-done";
    pub const FIELD_TOTAL: &str = "transfers-window-field-total";
    pub const FIELD_TROUBLE: &str = "transfers-window-field-trouble";
    pub const FIELD_ANSWER: &str = "transfers-window-field-answer";
    pub const FIELD_IDENTITY: &str = "transfers-window-field-identity";
    pub const IDENTITY_MINE: &str = "transfers-identity-mine";
    pub const IDENTITY_ANONYMOUS: &str = "transfers-identity-anonymous";

    // THE QUEUE'S VOCABULARY, spelled out one constant per case instead of
    // assembled from the enum's name. Twenty-four lines against one `format!`,
    // and what they buy is that a word without a translation fails in the
    // check rather than on screen.
    pub const ENGINE_LOCAL: &str = "transfers-engine-local";
    pub const ENGINE_FETCH: &str = "transfers-engine-fetch";
    pub const ENGINE_MEDIA: &str = "transfers-engine-media";
    pub const ENGINE_TORRENT: &str = "transfers-engine-torrent";

    pub const STAGE_WEIGHING: &str = "transfers-stage-weighing";
    pub const STAGE_RUNNING: &str = "transfers-stage-running";
    pub const STAGE_PAUSED: &str = "transfers-stage-paused";
    pub const STAGE_DONE: &str = "transfers-stage-done";
    pub const STAGE_FAILED: &str = "transfers-stage-failed";
    pub const STAGE_CANCELLED: &str = "transfers-stage-cancelled";

    pub const WORK_COPY: &str = "transfers-work-copy";
    pub const WORK_MOVE: &str = "transfers-work-move";
    pub const WORK_DELETE: &str = "transfers-work-delete";
    pub const WORK_SHRED: &str = "transfers-work-shred";
    pub const WORK_TRASH: &str = "transfers-work-trash";
    pub const WORK_RESTORE: &str = "transfers-work-restore";
    pub const WORK_COMPRESS: &str = "transfers-work-compress";
    pub const WORK_EXTRACT: &str = "transfers-work-extract";
    pub const WORK_MEASURE: &str = "transfers-work-measure";
    pub const WORK_FETCH: &str = "transfers-work-fetch";
    pub const WORK_MEDIA: &str = "transfers-work-media";
    pub const WORK_AUDIO: &str = "transfers-work-audio";
    pub const WORK_SUBTITLES: &str = "transfers-work-subtitles";
    pub const WORK_TORRENT: &str = "transfers-work-torrent";
}

/// The commands this buffer registers, and the keys that reach them.
///
/// Data, and under `m ` only: the host refuses at load whoever binds under a
/// global prefix. Each id needs `cmd-<id>` in both languages.
const KEYMAP: &[(&str, &str)] = &[
    ("m p", "transfers-hold"),
    ("m c", "transfers-give-up"),
    ("m k", "transfers-drop"),
    ("m l", "transfers-clear"),
    ("m d", "transfers-detail"),
];

/// Which screen is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// Every job, one per row.
    List,
    /// One job, field by field.
    Detail,
}

/// A question waiting for `Enter`.
///
/// Inline, on the last line — never a popup (invariant 14). Any other key
/// clears it, which is why it is a field and not a modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    /// Give up on the job with this id, deleting what was already written.
    GiveUp(u64),
}

/// Everything this buffer remembers between calls.
struct State {
    queue: Queue,
    screen: Screen,
    /// The cursor, as a JOB ID and never a row number.
    ///
    /// The list reorders itself as jobs finish — what is afoot floats to the
    /// top — so a row number would act on whatever moved into that position
    /// between the draw and the key.
    on: Option<u64>,
    /// First visible job, for the scroll.
    top: usize,
    pending: Option<Pending>,
    /// Whether the last ask was accepted, so the screen can say so.
    asked: bool,
    /// The window's height, as the last `render` saw it.
    lines: u16,
    /// The line the list starts on, so a click can be resolved.
    list_top: Option<u16>,
}

impl State {
    const fn fresh() -> Self {
        Self {
            queue: Queue { jobs: Vec::new() },
            screen: Screen::List,
            on: None,
            top: 0,
            pending: None,
            asked: false,
            lines: 24,
            list_top: None,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::fresh()) };
}

// ---------------------------------------------------------------------------
// The vocabulary: one word per case, and the same word feeds text and drawing.
// ---------------------------------------------------------------------------

const fn engine_key(e: Engine) -> &'static str {
    match e {
        Engine::Local => key::ENGINE_LOCAL,
        Engine::Fetch => key::ENGINE_FETCH,
        Engine::Media => key::ENGINE_MEDIA,
        Engine::Torrent => key::ENGINE_TORRENT,
    }
}

const fn stage_key(s: Stage) -> &'static str {
    match s {
        Stage::Weighing => key::STAGE_WEIGHING,
        Stage::Running => key::STAGE_RUNNING,
        Stage::Paused => key::STAGE_PAUSED,
        Stage::Done => key::STAGE_DONE,
        Stage::Failed => key::STAGE_FAILED,
        Stage::Cancelled => key::STAGE_CANCELLED,
    }
}

const fn errand_key(e: Errand) -> &'static str {
    match e {
        Errand::Copy => key::WORK_COPY,
        Errand::MoveIt => key::WORK_MOVE,
        Errand::Delete => key::WORK_DELETE,
        Errand::Shred => key::WORK_SHRED,
        Errand::Trash => key::WORK_TRASH,
        Errand::Restore => key::WORK_RESTORE,
        Errand::Compress => key::WORK_COMPRESS,
        Errand::Extract => key::WORK_EXTRACT,
        Errand::Measure => key::WORK_MEASURE,
        Errand::Fetch => key::WORK_FETCH,
        Errand::Media => key::WORK_MEDIA,
        Errand::Audio => key::WORK_AUDIO,
        Errand::Subtitles => key::WORK_SUBTITLES,
        Errand::Torrent => key::WORK_TORRENT,
    }
}

/// The drawing for a stage.
///
/// `done` and `failed` deliberately reuse `success` and `error`: the same
/// meaning in two places gets the same drawing, which is already this system's
/// rule, and a second check mark would be a second thing to keep in step.
const fn stage_icon(s: Stage) -> Icon {
    match s {
        Stage::Weighing => Icon::TransferWeighing,
        Stage::Running => Icon::TransferRunning,
        Stage::Paused => Icon::TransferPaused,
        Stage::Done => Icon::Success,
        Stage::Failed => Icon::Error,
        Stage::Cancelled => Icon::TransferCancelled,
    }
}

/// And the colour, which is the stage's and not the engine's: what one wants to
/// find on a screenful of rows is the one that stopped.
const fn stage_colour(s: Stage) -> Color {
    match s {
        Stage::Weighing => Color::TextSecondary,
        Stage::Running => Color::StateAccent,
        Stage::Paused => Color::StateWarning,
        Stage::Done => Color::StateSuccess,
        Stage::Failed => Color::StateError,
        Stage::Cancelled => Color::TextDim,
    }
}

/// The drawing for an engine, so a torrent and a paste do not look alike.
const fn engine_icon(e: Engine) -> Icon {
    match e {
        Engine::Local => Icon::EngineLocal,
        Engine::Fetch => Icon::EngineFetch,
        Engine::Media => Icon::EngineMedia,
        Engine::Torrent => Icon::EngineTorrent,
    }
}

// ---------------------------------------------------------------------------
// Numbers a person reads.
// ---------------------------------------------------------------------------

/// Bytes as a person reads them.
///
/// One decimal from a gibibyte up, none below. The separator is a full stop
/// because a component does not know the language — it does not translate
/// (invariant 11), and a comma here would be a decision about locale taken in
/// the wrong place.
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

/// A count, when the job counts things and not bytes.
fn figure(amount: Amount, n: u64) -> String {
    match amount {
        Amount::Bytes => size(n),
        Amount::Items => n.to_string(),
    }
}

/// How far along, in per mille, or `None` when the total is not known.
const fn along(j: &Job) -> Option<u16> {
    if j.total == 0 {
        return None;
    }
    let done = if j.done > j.total { j.total } else { j.done };
    Some((done as u128 * 1000 / j.total as u128) as u16)
}

/// Per mille as a percentage with one decimal.
fn percent(per_mille: u16) -> String {
    format!("{}.{}%", per_mille / 10, per_mille % 10)
}

// ---------------------------------------------------------------------------
// Drawing.
// ---------------------------------------------------------------------------

/// How wide the bar is, in cells.
///
/// Twelve, the same as the footer's `space.transfers.bar`, so the two faces of
/// one queue do not disagree about how far along something is at a glance. It
/// is a number and not a token because a component receives no token values
/// (invariant 11) — what it receives is drawings, through `Icon`.
const BAR: u16 = 12;

/// Where each column of a job row starts, given the window's width.
struct Lay {
    errand_at: u16,
    subject_at: u16,
    subject_width: u16,
    bar_at: Option<u16>,
    percent_at: Option<u16>,
    amounts_at: Option<u16>,
}

impl Lay {
    /// Works the columns out from the right, dropping what does not fit.
    ///
    /// Nothing here demands a size: a window can be narrow, and the answer to
    /// narrow is fewer columns, never a row that runs off the screen. The order
    /// of sacrifice is the amounts first, then the bar — the percentage is the
    /// last number to go, because it is the one that answers "how long".
    fn of(columns: u16) -> Self {
        const ERRAND_AT: u16 = 4;
        const ERRAND_WIDTH: u16 = 11;
        const SUBJECT_AT: u16 = ERRAND_AT + ERRAND_WIDTH + 1;
        const AMOUNTS_WIDTH: u16 = 22;
        const PERCENT_WIDTH: u16 = 7;

        let mut right = columns;
        let mut amounts_at = None;
        let mut bar_at = None;
        let mut percent_at = None;

        if columns >= SUBJECT_AT + 12 + AMOUNTS_WIDTH {
            right -= AMOUNTS_WIDTH;
            amounts_at = Some(right);
        }
        if right >= SUBJECT_AT + 12 + PERCENT_WIDTH {
            right -= PERCENT_WIDTH;
            percent_at = Some(right);
        }
        if right >= SUBJECT_AT + 12 + BAR + 1 {
            right -= BAR + 1;
            bar_at = Some(right);
        }

        Self {
            errand_at: ERRAND_AT,
            subject_at: SUBJECT_AT,
            subject_width: right.saturating_sub(SUBJECT_AT + 1).max(1),
            bar_at,
            percent_at,
            amounts_at,
        }
    }
}

/// A piece of text at a place.
fn put(out: &mut Vec<Command>, line: u16, column: u16, content: Text, color: Color, width: u16) {
    out.push(Command::Text(TextCommand {
        line,
        column,
        content,
        color,
        scale: Scale::Normal,
        max_width: width,
    }));
}

/// The title, and the line after it.
fn title(out: &mut Vec<Command>, line: u16, icon: Icon, text: &str, ctx: &Context) -> u16 {
    out.push(Command::Text(TextCommand {
        line,
        column: 0,
        content: Text::Icon(icon),
        color: Color::StateAccent,
        scale: Scale::Large,
        max_width: 2,
    }));
    out.push(Command::Text(TextCommand {
        line,
        column: 3,
        content: Text::Key(text.to_owned()),
        color: Color::TextPrimary,
        scale: Scale::Large,
        max_width: ctx.columns.saturating_sub(3),
    }));
    out.push(Command::Rule(LineCommand {
        line: line + 1,
        color: Color::RuleDefault,
    }));
    line + 2
}

/// The bar: `done` cells of one drawing, the rest of the other.
///
/// One command per cell, because `Text::Icon` carries one drawing and a
/// repeated string would be a codepoint written into this file. The cost is a
/// dozen records per row, which is what a display list is for.
fn draw_bar(out: &mut Vec<Command>, line: u16, at: u16, job: &Job) {
    let Some(per) = along(job) else {
        return;
    };
    let done = (u32::from(per) * u32::from(BAR) / 1000) as u16;
    let colour = stage_colour(job.stage);
    for cell in 0..BAR {
        let (icon, color) = if cell < done {
            (Icon::ProgressDone, colour)
        } else {
            (Icon::ProgressLeft, Color::TextDim)
        };
        out.push(Command::Text(TextCommand {
            line,
            column: at + cell,
            content: Text::Icon(icon),
            color,
            scale: Scale::Normal,
            max_width: 1,
        }));
    }
}

impl State {
    /// The jobs, one per row.
    fn list(&mut self, ctx: &Context) -> Vec<Command> {
        let mut out = Vec::new();
        let mut line = title(&mut out, 0, Icon::Transfer, key::TITLE, ctx);

        if self.queue.jobs.is_empty() {
            self.list_top = None;
            put(
                &mut out,
                line + 1,
                2,
                Text::Key(key::EMPTY.to_owned()),
                Color::TextSecondary,
                ctx.columns.saturating_sub(2),
            );
            self.foot(&mut out, ctx);
            return out;
        }

        // The last line belongs to the hint or to the question; the one above it
        // is left blank so a row never touches the bottom edge.
        let room = usize::from(self.lines.saturating_sub(line + 2)).max(1);
        self.scroll(room);

        let lay = Lay::of(ctx.columns);
        self.list_top = Some(line);

        for job in self.queue.jobs.iter().skip(self.top).take(room) {
            if Some(job.id) == self.on {
                out.push(Command::Selection(LineCommand {
                    line,
                    color: Color::SurfaceSelection,
                }));
            }
            row(&mut out, line, job, &lay);
            line += 1;
        }

        self.foot(&mut out, ctx);
        out
    }

    /// One job, field by field — where the whole path and the whole failure
    /// fit, which a row cannot promise.
    fn detail(&mut self, ctx: &Context) -> Vec<Command> {
        let mut out = Vec::new();
        let mut line = title(&mut out, 0, Icon::List, key::DETAIL, ctx);
        self.list_top = None;

        let Some(job) = self.focused() else {
            put(
                &mut out,
                line + 1,
                2,
                Text::Key(key::EMPTY.to_owned()),
                Color::TextSecondary,
                ctx.columns.saturating_sub(2),
            );
            self.foot(&mut out, ctx);
            return out;
        };

        const AT: u16 = 16;
        let wide = ctx.columns.saturating_sub(AT + 1);
        let field = |out: &mut Vec<Command>, line: u16, label: &str, value: Text, c: Color| {
            put(
                out,
                line,
                2,
                Text::Key(label.to_owned()),
                Color::TextSecondary,
                AT - 3,
            );
            put(out, line, AT, value, c, wide);
        };

        line += 1;
        field(
            &mut out,
            line,
            key::FIELD_ENGINE,
            Text::Key(engine_key(job.engine).to_owned()),
            Color::TextPrimary,
        );
        out.push(Command::Text(TextCommand {
            line,
            column: 0,
            content: Text::Icon(engine_icon(job.engine)),
            color: Color::TextSecondary,
            scale: Scale::Normal,
            max_width: 1,
        }));
        line += 1;
        field(
            &mut out,
            line,
            key::FIELD_ERRAND,
            Text::Key(errand_key(job.errand).to_owned()),
            Color::TextPrimary,
        );
        line += 1;
        field(
            &mut out,
            line,
            key::FIELD_STAGE,
            Text::Key(stage_key(job.stage).to_owned()),
            stage_colour(job.stage),
        );
        out.push(Command::Text(TextCommand {
            line,
            column: 0,
            content: Text::Icon(stage_icon(job.stage)),
            color: stage_colour(job.stage),
            scale: Scale::Normal,
            max_width: 1,
        }));
        line += 1;
        field(
            &mut out,
            line,
            key::FIELD_IDENTITY,
            Text::Key(
                if job.identified {
                    key::IDENTITY_MINE
                } else {
                    key::IDENTITY_ANONYMOUS
                }
                .to_owned(),
            ),
            if job.identified {
                Color::StateWarning
            } else {
                Color::TextSecondary
            },
        );
        line += 1;
        field(
            &mut out,
            line,
            key::FIELD_SUBJECT,
            Text::Data(job.subject.clone()),
            Color::TextPrimary,
        );
        line += 1;
        field(
            &mut out,
            line,
            key::FIELD_DONE,
            Text::Data(figure(job.amount, job.done)),
            Color::TextPrimary,
        );
        line += 1;
        field(
            &mut out,
            line,
            key::FIELD_TOTAL,
            if job.total == 0 {
                Text::Key(key::UNKNOWN.to_owned())
            } else {
                Text::Data(figure(job.amount, job.total))
            },
            Color::TextPrimary,
        );
        if job.answer != 0 {
            line += 1;
            field(
                &mut out,
                line,
                key::FIELD_ANSWER,
                Text::Data(size(job.answer)),
                Color::StateSuccess,
            );
        }
        if !job.trouble.is_empty() {
            line += 1;
            field(
                &mut out,
                line,
                key::FIELD_TROUBLE,
                Text::Data(job.trouble.clone()),
                Color::StateError,
            );
        }

        if let Some(per) = along(job) {
            line += 2;
            draw_bar(&mut out, line, 2, job);
            put(
                &mut out,
                line,
                2 + BAR + 1,
                Text::Data(percent(per)),
                Color::TextPrimary,
                8,
            );
        }

        self.foot(&mut out, ctx);
        out
    }

    /// The last line: the question if there is one, otherwise the hint.
    fn foot(&self, out: &mut Vec<Command>, ctx: &Context) {
        let line = self.lines.saturating_sub(1);
        let (text, colour) = match (self.pending, self.asked) {
            (Some(Pending::GiveUp(_)), _) => (key::CONFIRM_CANCEL, Color::StateWarning),
            (None, true) => (key::ASKED, Color::StateSuccess),
            (None, false) => match self.screen {
                Screen::List => (key::HINT, Color::TextDim),
                Screen::Detail => (key::HINT_DETAIL, Color::TextDim),
            },
        };
        put(
            out,
            line,
            0,
            Text::Key(text.to_owned()),
            colour,
            ctx.columns,
        );
    }

    /// The job under the cursor.
    fn focused(&self) -> Option<&Job> {
        let id = self.on?;
        self.queue.jobs.iter().find(|j| j.id == id)
    }

    /// Where the cursor is in the list, as an index.
    fn cursor(&self) -> Option<usize> {
        let id = self.on?;
        self.queue.jobs.iter().position(|j| j.id == id)
    }

    /// Keeps the cursor on screen.
    fn scroll(&mut self, room: usize) {
        let Some(at) = self.cursor() else {
            self.top = 0;
            return;
        };
        if at < self.top {
            self.top = at;
        } else if at >= self.top + room {
            self.top = at + 1 - room;
        }
    }

    /// Moves the cursor by one, wrapping at neither end.
    fn step(&mut self, down: bool) -> Vec<Effect> {
        if self.queue.jobs.is_empty() {
            return Vec::new();
        }
        let at = self.cursor().unwrap_or(0);
        let next = if down {
            (at + 1).min(self.queue.jobs.len() - 1)
        } else {
            at.saturating_sub(1)
        };
        self.on = Some(self.queue.jobs[next].id);
        self.pending = None;
        self.asked = false;
        vec![Effect::Redraw]
    }

    /// A fresh queue arrived.
    fn receive(&mut self, queue: Queue) -> Vec<Effect> {
        if queue == self.queue {
            return Vec::new();
        }
        self.queue = queue;

        // THE CURSOR SURVIVES A JOB ENDING, and that is why it is an id: the
        // row it sat on may have moved, and the job it named may be gone. When
        // it is gone the cursor goes to the first row rather than to whatever
        // took the place — acting on the wrong job is the failure this whole
        // choice exists to avoid.
        let still_there = self
            .on
            .is_some_and(|id| self.queue.jobs.iter().any(|j| j.id == id));
        if !still_there {
            self.on = self.queue.jobs.first().map(|j| j.id);
            self.pending = None;
        }
        vec![Effect::Redraw]
    }

    /// A named command from the keymap.
    fn command(&mut self, id: &str) -> Vec<Effect> {
        self.asked = false;
        match id {
            "transfers-detail" => {
                self.screen = match self.screen {
                    Screen::List => Screen::Detail,
                    Screen::Detail => Screen::List,
                };
                self.pending = None;
                vec![Effect::Redraw]
            }
            "transfers-hold" => self.hold(),
            "transfers-give-up" => {
                // The one irreversible key here, and it asks first — the same
                // discipline the memory panel uses before closing a process.
                match self.focused() {
                    Some(j) if j.stage != Stage::Done => {
                        self.pending = Some(Pending::GiveUp(j.id));
                        vec![Effect::Redraw]
                    }
                    _ => Vec::new(),
                }
            }
            "transfers-drop" => self.simple(Request::ForgetJob),
            "transfers-clear" => {
                self.asked = true;
                vec![
                    Effect::Ask(Demand {
                        what: Request::SweepJobs,
                        target: 0,
                    }),
                    Effect::Redraw,
                ]
            }
            _ => Vec::new(),
        }
    }

    /// Pause what is running, let go of what is held.
    ///
    /// One key for both, because the state is on screen: a row that shows
    /// `paused` and a key called "pause" would be a question about what
    /// pressing it does.
    fn hold(&mut self) -> Vec<Effect> {
        let Some(job) = self.focused() else {
            return Vec::new();
        };
        let what = match job.stage {
            Stage::Paused => Request::ResumeJob,
            Stage::Weighing | Stage::Running => Request::PauseJob,
            // Finished: there is nothing to hold, and saying nothing is better
            // than sending a request the host will refuse.
            Stage::Done | Stage::Failed | Stage::Cancelled => return Vec::new(),
        };
        let target = job.id;
        self.asked = true;
        vec![Effect::Ask(Demand { what, target }), Effect::Redraw]
    }

    /// A request that needs the focused job and no confirmation.
    fn simple(&mut self, what: Request) -> Vec<Effect> {
        let Some(job) = self.focused() else {
            return Vec::new();
        };
        let target = job.id;
        self.asked = true;
        vec![Effect::Ask(Demand { what, target }), Effect::Redraw]
    }

    /// `Enter`: answer the question if there is one.
    fn confirm(&mut self) -> Vec<Effect> {
        match self.pending.take() {
            Some(Pending::GiveUp(target)) => {
                self.asked = true;
                vec![
                    Effect::Ask(Demand {
                        what: Request::CancelJob,
                        target,
                    }),
                    Effect::Redraw,
                ]
            }
            // No question pending: `Enter` opens the detail, which is the
            // obvious meaning of confirming a row.
            None => self.command("transfers-detail"),
        }
    }
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "transferencias".into(),
            mode: "transfers".into(),
            keymap: KEYMAP
                .iter()
                .map(|(keys, command)| Binding {
                    keys: (*keys).into(),
                    command: (*command).into(),
                })
                .collect(),
            commands: KEYMAP.iter().map(|(_, c)| (*c).into()).collect(),
            segments: vec![],
            // THE ONE THAT MATTERS: without it the host neither subscribes to
            // the queue nor forwards a demand, and this buffer is an empty
            // screen with a keymap that does nothing.
            capabilities: vec![Capability::Transfers],
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        STATE.with_borrow_mut(|s| {
            s.lines = ctx.lines;
            match s.screen {
                Screen::List => s.list(&ctx),
                Screen::Detail => s.detail(&ctx),
            }
        })
    }

    fn event(e: Input) -> Vec<Effect> {
        STATE.with_borrow_mut(|s| match e {
            Input::Queue(queue) => s.receive(queue),
            Input::Command(id) => s.command(&id),
            Input::Down => s.step(true),
            Input::Up => s.step(false),
            Input::Confirm => s.confirm(),
            Input::Cancel => {
                if s.pending.take().is_some() || s.asked {
                    s.asked = false;
                    vec![Effect::Redraw]
                } else if s.screen == Screen::Detail {
                    s.screen = Screen::List;
                    vec![Effect::Redraw]
                } else {
                    vec![Effect::Close]
                }
            }
            Input::Toggle => s.command("transfers-detail"),
            Input::Click(p) => {
                let Some(first) = s.list_top else {
                    return Vec::new();
                };
                if p.line < first {
                    return Vec::new();
                }
                let n = s.top + usize::from(p.line - first);
                match s.queue.jobs.get(n) {
                    Some(job) => {
                        s.on = Some(job.id);
                        s.pending = None;
                        s.asked = false;
                        vec![Effect::Redraw]
                    }
                    None => Vec::new(),
                }
            }
            _ => Vec::new(),
        })
    }
}

/// Draws one job's row.
fn row(out: &mut Vec<Command>, line: u16, job: &Job, lay: &Lay) {
    let colour = stage_colour(job.stage);

    out.push(Command::Text(TextCommand {
        line,
        column: 0,
        content: Text::Icon(stage_icon(job.stage)),
        color: colour,
        scale: Scale::Normal,
        max_width: 1,
    }));
    out.push(Command::Text(TextCommand {
        line,
        column: 2,
        content: Text::Icon(engine_icon(job.engine)),
        // IDENTIFIED ROWS ARE MARKED, and by colour rather than by a second
        // glyph: the column is one cell wide, and what matters is that a
        // glance over a screenful finds the ones that were not anonymous.
        color: if job.identified {
            Color::StateWarning
        } else {
            Color::TextDim
        },
        scale: Scale::Normal,
        max_width: 1,
    }));
    put(
        out,
        line,
        lay.errand_at,
        Text::Key(errand_key(job.errand).to_owned()),
        Color::TextSecondary,
        lay.subject_at - lay.errand_at - 1,
    );

    // A failure replaces the subject: what one needs to read on the row that
    // stopped is WHY, and the name of the file it stopped on is in the detail.
    let (subject, subject_colour) = if job.trouble.is_empty() {
        (job.subject.clone(), Color::TextPrimary)
    } else {
        (job.trouble.clone(), Color::StateError)
    };
    put(
        out,
        line,
        lay.subject_at,
        Text::Data(subject),
        subject_colour,
        lay.subject_width,
    );

    if let Some(at) = lay.bar_at {
        draw_bar(out, line, at, job);
    }
    if let Some(at) = lay.percent_at {
        let text = match along(job) {
            Some(per) => Text::Data(percent(per)),
            // No total: the stage is what there is to say, and it is a word the
            // host translates — not a dash pretending to be a number.
            None => Text::Key(stage_key(job.stage).to_owned()),
        };
        put(out, line, at, text, colour, 7);
    }
    if let Some(at) = lay.amounts_at {
        let text = if job.total == 0 {
            figure(job.amount, job.done)
        } else {
            format!(
                "{} / {}",
                figure(job.amount, job.done),
                figure(job.amount, job.total)
            )
        };
        put(out, line, at, Text::Data(text), Color::TextSecondary, 22);
    }
}

export!(Component);

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: u64, stage: Stage, done: u64, total: u64) -> Job {
        Job {
            id,
            engine: Engine::Local,
            errand: Errand::Copy,
            stage,
            amount: Amount::Bytes,
            done,
            total,
            subject: "file".to_owned(),
            trouble: String::new(),
            answer: 0,
            identified: false,
        }
    }

    #[test]
    fn the_cursor_is_an_id_and_survives_a_reorder() {
        let mut s = State::fresh();
        s.receive(Queue {
            jobs: vec![job(1, Stage::Running, 0, 10), job(2, Stage::Running, 0, 10)],
        });
        s.step(true);
        assert_eq!(s.on, Some(2));
        // The queue comes back with the rows swapped, which is what happens
        // when the first one finishes and sinks. The cursor must still be on 2.
        s.receive(Queue {
            jobs: vec![job(2, Stage::Running, 5, 10), job(1, Stage::Done, 10, 10)],
        });
        assert_eq!(s.on, Some(2));
        assert_eq!(s.cursor(), Some(0));
    }

    #[test]
    fn a_job_that_vanishes_takes_the_cursor_to_the_first_row() {
        let mut s = State::fresh();
        s.receive(Queue {
            jobs: vec![job(1, Stage::Done, 1, 1), job(2, Stage::Done, 1, 1)],
        });
        s.step(true);
        assert_eq!(s.on, Some(2));
        s.receive(Queue {
            jobs: vec![job(1, Stage::Done, 1, 1)],
        });
        assert_eq!(s.on, Some(1));
    }

    #[test]
    fn giving_up_asks_before_it_acts() {
        let mut s = State::fresh();
        s.receive(Queue {
            jobs: vec![job(7, Stage::Running, 1, 10)],
        });
        // The first press only puts the question up; nothing crosses.
        let out = s.command("transfers-give-up");
        assert_eq!(out, vec![Effect::Redraw]);
        assert_eq!(s.pending, Some(Pending::GiveUp(7)));
        // The second one sends it, and names the id and not the row.
        let out = s.confirm();
        assert!(out.contains(&Effect::Ask(Demand {
            what: Request::CancelJob,
            target: 7,
        })));
        assert_eq!(s.pending, None);
    }

    #[test]
    fn hold_reads_the_stage_and_never_sends_the_wrong_verb() {
        let mut s = State::fresh();
        s.receive(Queue {
            jobs: vec![job(3, Stage::Running, 1, 10)],
        });
        assert!(s.hold().contains(&Effect::Ask(Demand {
            what: Request::PauseJob,
            target: 3,
        })));
        s.receive(Queue {
            jobs: vec![job(3, Stage::Paused, 1, 10)],
        });
        assert!(s.hold().contains(&Effect::Ask(Demand {
            what: Request::ResumeJob,
            target: 3,
        })));
        // Finished: nothing is sent at all, rather than a request the host
        // would have to refuse.
        s.receive(Queue {
            jobs: vec![job(3, Stage::Done, 10, 10)],
        });
        assert!(s.hold().is_empty());
    }

    #[test]
    fn a_narrow_window_drops_columns_instead_of_overflowing() {
        let wide = Lay::of(120);
        assert!(wide.bar_at.is_some() && wide.amounts_at.is_some());
        let narrow = Lay::of(40);
        assert!(narrow.amounts_at.is_none());
        assert!(narrow.subject_at + narrow.subject_width <= 40);
        // And whatever survives, nothing starts past the right edge.
        for lay in [Lay::of(30), Lay::of(56), Lay::of(80), Lay::of(200)] {
            for at in [lay.bar_at, lay.percent_at, lay.amounts_at]
                .into_iter()
                .flatten()
            {
                assert!(at < 200);
            }
        }
    }

    #[test]
    fn a_total_of_zero_never_draws_a_bar() {
        let j = job(1, Stage::Running, 900, 0);
        assert_eq!(along(&j), None);
        let mut out = Vec::new();
        draw_bar(&mut out, 0, 0, &j);
        assert!(out.is_empty(), "uma barra sem total inventaria a fracao");
    }
}
