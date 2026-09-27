//! The transfer queue, in a window of its own — `LEADER t w`.
//!
//! # The same queue, a second face
//!
//! `LEADER t t` already draws this list inside the footer, under a ceiling of
//! twelve rows. This is the same list with room to breathe: every job at once,
//! and a bar wide enough to read. Both faces ask the same unit over the same
//! socket, and neither of them owns anything — a copy of forty gibibytes
//! belongs to `ronoctua-transfers`, which is why restarting the footer does
//! not touch it.
//!
//! # The same keys, the same pages, the same question
//!
//! What a key does is the footer panel's too: ONE table
//! (`ronoctua_protocol::TRANSFERS_KEYS`), bound here under the local leader,
//! and one answer about which keys a row teaches (`transfers_taught`). Both
//! doors open a job's own page with `RET` — the second click's meaning — and
//! both ask before giving up, `no` first.
//!
//! # It reads nothing
//!
//! Not the socket, not the disk, not a clock. The rows arrive through
//! `input::queue`, already read and already cut by the host, and what leaves is
//! `effect::ask` naming a job id. Invariant 11 intact: the component has no
//! parser and is not getting one.
//!
//! # It no longer generates its own bindings, and it no longer decides geometry
//!
//! `wit_bindgen::generate!` is gone: the bindings come from
//! `ronoctua-design`, the crate invariant 12 names, which generates them ONCE
//! for every buffer. Three things follow, and all three are the point of it:
//!
//!   - the crate-wide `#![allow(unsafe_code, missing_docs)]` this file used to
//!     need is gone with it. The exemption belongs to the one crate that
//!     generates bindings, and that is no longer this one;
//!   - the heading is not written here. [`Screen::heading`] is THE heading, so
//!     this file cannot get its air wrong and cannot put a rule under it — and
//!     it used to draw one. What separates a title from its content is
//!     whitespace, and only whitespace;
//!   - the row's columns are a [`Table`], resolved in the same two phases the
//!     rest of the repository uses. The constants that are genuinely this
//!     buffer's — how wide a bar is, how wide two byte counts are — stay here,
//!     with the reason beside each; the arithmetic that used to be `Lay::of`
//!     lives in the crate.
//!
//! What the generation also took with it is `additional_derives: [PartialEq]`,
//! which this buffer used to ask for so it could tell whether a fresh queue
//! would draw the same. Hence [`same_queue`] below, by hand.
//!
//! # The bar is glyphs, and the glyphs are tokens
//!
//! A cell grid does not stretch, so a bar is N cells of one drawing followed by
//! the rest of another. Both come from `assets/tokens/icons.nix` through
//! `Icon::` — invariant 9 reaches the codepoint, and a block character typed
//! into this file would be a drawing chosen inside a component that has no
//! font.

use std::cell::RefCell;

use ronoctua_design::frame::{
    TRANSFERS_ANSWER, TRANSFERS_KEYS, TransferRow, local_binding, transfers_taught,
};
use ronoctua_design::guest::Wit;
use ronoctua_design::types::{
    AIR, Amount, Binding, BufferState, Capability, Column, Command, Context, Demand, Descriptor,
    Effect, Engine, Errand, Guest, ICON, Icon, Input, Job, Layout, Queue, Request, Role, Run,
    Screen, Scroll, Stage, Table, Tone,
};

/// The frame this buffer draws into, with the palette that resolves a role to
/// the generated colour enum.
///
/// An alias so no line below has to spell the palette out, and so the word
/// `Screen` in this file means the design crate's builder and nothing else —
/// what used to be called `Screen` here is now [`Face`].
type Frame = Screen<Wit>;

/// Every text key this buffer names.
///
/// A module, and every one of them a literal `pub const`: that is what the
/// `buffer-keys-have-text` check can see. Building a key with `format!` would
/// hide it from the check, and the first sign of a missing one would be its own
/// name drawn on screen in front of the person.
mod key {
    pub const TITLE: &str = "transfers-window-title";
    pub const EMPTY: &str = "transfers-window-empty";
    pub const CONFIRM_CANCEL: &str = "transfers-window-confirm-cancel";
    pub const GIVE_UP_NO: &str = "transfers-give-up-no";
    pub const GIVE_UP_YES: &str = "transfers-give-up-yes";
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

/// The keys this window answers to — the queue's ONE table, the footer
/// panel's too (`TRANSFERS_KEYS`). Every chord of it is bound under the local
/// leader (`, c` is `C-c` in this window); the arrows, `RET`, `TAB` and `SPC`
/// arrive as the intents the host sends. So a key means the same thing in both
/// doors — they used to disagree about `RET` and about `C-p`.
fn keymap() -> Vec<Binding> {
    TRANSFERS_KEYS
        .iter()
        .filter_map(|(notation, id)| {
            local_binding(notation).map(|keys| Binding {
                keys,
                command: (*id).into(),
            })
        })
        .collect()
}

/// The commands the keymap names, each once.
fn commands() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in keymap() {
        if !out.contains(&b.command) {
            out.push(b.command);
        }
    }
    out
}

/// Every phrase the foot may teach, asked of the host so the pairs can be
/// MEASURED — a phrase reserved blind is a gap the eye reads as a defect.
fn taught_phrases() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (_, id) in TRANSFERS_KEYS.iter().chain(TRANSFERS_ANSWER) {
        if !out.iter().any(|o| o == id) {
            out.push((*id).to_owned());
        }
    }
    out
}

/// How wide a phrase is reserved before the host has said it.
const TEACH_CELLS: u16 = 13;

/// A separator between two pairs of the foot: the air, and the dot.
const SEPARATOR_CELLS: u16 = ronoctua_design::GAP + ICON;

/// Which screen is on.
///
/// It was called `Screen` until this buffer took the design crate's builder of
/// that name. The word it wanted all along is the one the header uses: the
/// queue has two FACES, and this says which of them is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Face {
    /// Every job, one per row.
    List,
    /// One job, field by field.
    Detail,
}

/// The give-up question on screen: which job, and whether the cursor is on
/// `yes`. It is born on `no`, as the footer's panel's is — giving up deletes
/// what was already written, and a question about something that cannot be
/// undone starts on the answer that keeps it. Its own screen, never a popup
/// (invariant 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GiveUp {
    id: u64,
    yes: bool,
}

/// One pair of the foot, as it was DRAWN: where, and what a click on it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pair {
    at: u16,
    cells: u16,
    keys: &'static str,
    id: &'static str,
}

/// Everything this buffer remembers between calls.
struct State {
    queue: Queue,
    face: Face,
    /// The cursor, as a JOB ID and never a row number.
    ///
    /// The list reorders itself as jobs finish — what is afoot floats to the
    /// top — so a row number would act on whatever moved into that position
    /// between the draw and the key.
    on: Option<u64>,
    /// First visible job, for the scroll.
    top: usize,
    /// The give-up question, while it is on screen.
    asking: Option<GiveUp>,
    /// The line the list starts on, so a click can be resolved.
    list_top: Option<u16>,
    /// How many rows of it the last frame drew, for the wheel and the bar.
    list_lines: u16,
    /// The line the question's `no` was drawn on; `yes` is the one under it.
    answers_at: Option<u16>,
    /// The foot's pairs as the last frame drew them, and its line.
    foot: Vec<Pair>,
    foot_line: u16,
    /// The cell the pointer is on, if it is on one.
    hover: ronoctua_design::Hovers,
    /// The words the host resolved for the phrases the foot teaches.
    phrases: ronoctua_design::Phrases,
}

impl State {
    const fn fresh() -> Self {
        Self {
            queue: Queue { jobs: Vec::new() },
            face: Face::List,
            on: None,
            top: 0,
            asking: None,
            list_top: None,
            list_lines: 0,
            answers_at: None,
            foot: Vec::new(),
            foot_line: 0,
            hover: ronoctua_design::Hovers::new(),
            phrases: ronoctua_design::Phrases::empty(),
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

/// What part a stage plays on the screen — never which colour it is.
///
/// The stage's and not the engine's: what one wants to find on a screenful of
/// rows is the one that stopped, so `failed` is the loudest part there is and
/// the glyph beside it says the same thing again.
///
/// # THREE TOKENS THIS BUFFER CANNOT NAME ANY MORE, and they are the report
///
/// It used to write `Color::` directly, and three of the six colours it named
/// have no part in [`Role`]'s seven: `state-warning` (paused), `state-success`
/// (done) and `state-error` (failed). The vocabulary has `title`, `body`,
/// `quiet`, `dim`, `accent`, `rule` and `band` — a scale of ATTENTION, with no
/// word for "this went well" or "this went wrong". So the six stages map onto
/// the scale by how much they ask to be looked at, and the drawing carries the
/// rest:
///
/// | stage       | was              | plays    |
/// |-------------|------------------|----------|
/// | `weighing`  | `text-secondary` | `body`   |
/// | `running`   | `state-accent`   | `accent` |
/// | `paused`    | `state-warning`  | `quiet`  |
/// | `done`      | `state-success`  | `body`   |
/// | `failed`    | `state-error`    | `title`  |
/// | `cancelled` | `text-dim`       | `dim`    |
///
/// `quiet` and `dim` resolve to one token today and are two roles on purpose:
/// the day the theme separates them, `paused` and `cancelled` separate with it
/// and nothing here is touched. What is genuinely lost is the HUE — amber,
/// green and red — and getting it back is `Role` gaining the `state-*` family,
/// which is a decision about the vocabulary and not one this buffer may take.
const fn stage_role(s: Stage) -> Role {
    match s {
        Stage::Weighing | Stage::Done => Role::Body,
        Stage::Running => Role::Accent,
        Stage::Paused => Role::Quiet,
        Stage::Failed => Role::Title,
        Stage::Cancelled => Role::Dim,
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

/// A job that went out as the PERSON is marked, and by colour rather than by a
/// second glyph: the column is two cells wide, and what matters is that a
/// glance over a screenful finds the ones that were not anonymous.
///
/// `accent` is the one part the system reserves for "this one", which is
/// exactly what an identified job is among a queue of anonymous ones. It used
/// to be `state-warning`; see [`stage_role`] for why that name is gone.
const fn identity_role(identified: bool) -> Role {
    if identified { Role::Accent } else { Role::Dim }
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

/// Whether a fresh queue would draw exactly what is on screen.
///
/// By hand, and that is the price of one binding set for seven buffers: the
/// `generate!` this file used to own asked for `additional_derives:
/// [PartialEq]`, and the design crate's does not — a derive taken there is
/// taken for every component. A record from `wit-bindgen` gets `Clone` and a
/// `Debug`, and nothing else.
///
/// It compares every field, which is what the derive did: a job whose byte
/// count moved has moved on screen too, because the bar and the amounts are
/// drawn from it.
fn same_queue(a: &Queue, b: &Queue) -> bool {
    a.jobs.len() == b.jobs.len()
        && a.jobs
            .iter()
            .zip(b.jobs.iter())
            .all(|(x, y)| same_job(x, y))
}

/// Field for field, in the order the WIT declares them.
fn same_job(a: &Job, b: &Job) -> bool {
    a.id == b.id
        && a.engine == b.engine
        && a.errand == b.errand
        && a.stage == b.stage
        && a.amount == b.amount
        && a.done == b.done
        && a.total == b.total
        && a.subject == b.subject
        && a.trouble == b.trouble
        && a.answer == b.answer
        && a.identified == b.identified
}

// ---------------------------------------------------------------------------
// The columns.
// ---------------------------------------------------------------------------

/// How the tables below name their columns.
///
/// `&'static str`, because that is what [`Column`] takes: the crate carries no
/// buffer's vocabulary, so the words live here.
///
/// # `pub(crate)`, and it is not stylistic
///
/// A COLUMN ID IS NOT A TEXT KEY. It never reaches the screen: it is how this
/// file finds a field in the [`Layout`] again, and the words the person reads
/// are in [`key`], every one of them with a line in both `.ftl`s. But
/// `buffer-keys-have-text` reads the SHAPE `pub const NAME: &str = "..."`
/// anywhere under `buffers/` as a key, so ten column ids declared `pub` would
/// have the check demanding ten translations for `stage`, `bar` and `value` —
/// words nobody would ever see. `pub(crate)` is what this module's visibility
/// actually is, and it also keeps a layout id out of the i18n's ledger.
mod id {
    pub(crate) const STAGE: &str = "stage";
    pub(crate) const ENGINE: &str = "engine";
    pub(crate) const ERRAND: &str = "errand";
    pub(crate) const SUBJECT: &str = "subject";
    pub(crate) const BAR: &str = "bar";
    pub(crate) const PERCENT: &str = "percent";
    pub(crate) const AMOUNTS: &str = "amounts";

    pub(crate) const MARK: &str = "mark";
    pub(crate) const LABEL: &str = "label";
    pub(crate) const VALUE: &str = "value";
}

/// How wide each column is, in cells, and why.
///
/// These are the caller's constants and they stay the caller's: a bar is twelve
/// cells because a bar is twelve cells, and the crate has no way of knowing
/// that. What moved out of this file is the ARITHMETIC around them — where each
/// one starts, which one goes when the window will not pay, and whether the
/// last of them ends inside the screen.
mod width {
    /// The bar.
    ///
    /// Twelve, the same as the footer's `space.transfers.bar`, so the two faces
    /// of one queue do not disagree about how far along something is at a
    /// glance. It is a number and not a token because a component receives no
    /// token values (invariant 11) — what it receives is drawings, through
    /// `Icon`.
    pub const BAR: u16 = 12;

    /// The errand: which of fourteen operations this row is.
    ///
    /// Eleven cells, which is what it has always had. The longest errand runs
    /// to twenty-three (`put back from the trash`) and the HOST elides it — a
    /// buffer never cuts a sentence it cannot read (invariant 8). Eleven is
    /// what it takes for the first word to arrive whole.
    pub const ERRAND: u16 = 11;

    /// The percentage, and the stage's WORD when there is no total to divide by.
    ///
    /// Twelve, and it used to be seven — `100.0%` is six cells and the seventh
    /// was described as the one the stage's word "borrows". It does not borrow
    /// it: five of the six stage words are longer than seven in Portuguese, and
    /// `em andamento` came out as `em an…`, which is a preposition and an
    /// ellipsis. Unlike the errand beside it, whose first word arrives whole
    /// inside eleven, there is no useful prefix of a two-word phrase at seven.
    ///
    /// TWELVE IS THE LONGEST ONE (`em andamento`), and the number is not a
    /// guess any more: `driven::every_label_fits` measures every stage word in
    /// every language against this constant on every run of `transfers-driven`.
    ///
    /// The five extra cells come off the SUBJECT, which is the fill column and
    /// has a floor of its own — a name gives way before a word that would
    /// otherwise say nothing.
    pub const PERCENT: u16 = 12;

    /// Two figures and the ` / ` between them: `12.3 GiB / 456.7 GiB`.
    pub const AMOUNTS: u16 = 22;

    /// The narrowest a name may be squeezed to before a column is dropped
    /// instead. A PREFERENCE, in the crate's sense: below it a fixed column
    /// gives way, and when none is left the name takes whatever there is.
    pub const SUBJECT: u16 = 12;

    /// The detail's label column.
    ///
    /// Thirteen. The longest of the nine field labels is ten cells in either
    /// language (`no momento`, `quem pediu`, `download subtitles` is a value
    /// and not a label), and the three left over are what keeps a label from
    /// reading as though it had been cut.
    pub const LABEL: u16 = 13;

    /// And the narrowest its value may become: a path, a byte count, a failure.
    pub const VALUE: u16 = 12;
}

/// The order the columns give way in: the HIGHEST goes first.
///
/// It is a statement about what a row is FOR, and it is the order this
/// buffer's own comment has always claimed: **the amounts first, then the bar,
/// and the percentage last, because the percentage is the one that answers "how
/// long"**. After the three numbers go the words, and the two glyphs go last —
/// two cells answer "what is this" faster than any word, and the stage's glyph
/// is the very last thing to leave, because finding the one that stopped is
/// what one opens this window for.
///
/// (The code this replaces contradicted that comment. It handed the columns out
/// from the right, greedily, so at fifty cells it kept the amounts and dropped
/// the percentage — the opposite of what it said. The order is now declared
/// once, in one place, and the table obeys it.)
mod rank {
    pub const AMOUNTS: u8 = 6;
    pub const BAR: u8 = 5;
    pub const PERCENT: u8 = 4;
    pub const ERRAND: u8 = 3;
    pub const ENGINE: u8 = 2;
    pub const STAGE: u8 = 1;

    /// The detail's mark: decoration on two of its nine lines, so it is the
    /// first thing to go. The label never gives way while it can be helped —
    /// a column of values with nothing saying what they are is not a narrower
    /// detail, it is a different one.
    pub const MARK: u8 = 2;
    pub const LABEL: u8 = 1;
}

/// One job's row, resolved for a window this wide.
///
/// No gutter and no right edge: `buffer.rs`'s `context_of` already takes
/// `space.buffer.marginX` off BOTH sides before it says how many columns there
/// are, and a buffer that adds its own pays the margin twice.
fn row_layout(columns: u16) -> Layout {
    Table::across(columns)
        .with(Column::fixed(id::STAGE, ICON, rank::STAGE))
        .with(Column::fixed(id::ENGINE, ICON, rank::ENGINE))
        .with(Column::fixed(id::ERRAND, width::ERRAND, rank::ERRAND))
        .with(Column::fill(id::SUBJECT, width::SUBJECT))
        .with(Column::fixed(id::BAR, width::BAR, rank::BAR))
        .with(Column::fixed(id::PERCENT, width::PERCENT, rank::PERCENT))
        .with(Column::fixed(id::AMOUNTS, width::AMOUNTS, rank::AMOUNTS))
        .resolve()
}

/// One line of the detail: a mark, a label and the value it names.
fn detail_layout(columns: u16) -> Layout {
    Table::across(columns)
        .with(Column::fixed(id::MARK, ICON, rank::MARK))
        .with(Column::fixed(id::LABEL, width::LABEL, rank::LABEL))
        .with(Column::fill(id::VALUE, width::VALUE))
        .resolve()
}

/// The detail's own bar line: the bar, and the number beside it.
///
/// The same two widths the row uses, so one queue does not measure itself twice
/// — and the same order of giving way, for the same reason.
fn bar_layout(columns: u16) -> Layout {
    Table::across(columns)
        .with(Column::fixed(id::BAR, width::BAR, rank::BAR))
        .with(Column::fixed(id::PERCENT, width::PERCENT, rank::PERCENT))
        .resolve()
}

// ---------------------------------------------------------------------------
// Drawing.
// ---------------------------------------------------------------------------

/// The bar: `done` cells of one drawing, the rest of the other.
///
/// One run per cell, because an icon carries one drawing and a repeated string
/// would be a codepoint written into this file. The cost is a dozen records per
/// row, which is what a display list is for.
///
/// # Why the cells may sit next to each other
///
/// Because a glyph claims `heights.of(scale)` cells and nothing else —
/// `DisplayList::columns_of` charges it that whatever budget it declared, so
/// twelve glyphs one cell apart neither overlap nor reach past the edge. The
/// budget itself is the crate's [`ICON`], which `Run::glyph` puts there: this
/// file used to write `max_width: 1`, which is a number about a font it cannot
/// see.
fn draw_bar(screen: &mut Frame, line: u16, at: u16, job: &Job) {
    let Some(per) = along(job) else {
        return;
    };
    let done = (u32::from(per) * u32::from(width::BAR) / 1000) as u16;
    let role = stage_role(job.stage);
    for cell in 0..width::BAR {
        let (icon, role) = if cell < done {
            (Icon::ProgressDone, role)
        } else {
            (Icon::ProgressLeft, Role::Dim)
        };
        screen.put(
            line,
            Run::glyph(icon).at(at.saturating_add(cell)).playing(role),
        );
    }
}

/// Draws one job's row, and says how many cells of height it took.
fn row(screen: &mut Frame, line: u16, job: &Job, layout: &Layout) -> u16 {
    let stage = stage_role(job.stage);
    let cells = {
        let mut fields = screen.row_of(line, layout);
        fields.put(id::STAGE, Run::glyph(stage_icon(job.stage)).playing(stage));
        fields.put(
            id::ENGINE,
            Run::glyph(engine_icon(job.engine)).playing(identity_role(job.identified)),
        );
        fields.put(id::ERRAND, Run::label(errand_key(job.errand)));

        // A failure replaces the subject: what one needs to read on the row that
        // stopped is WHY, and the name of the file it stopped on is in the
        // detail. It is drawn LOUDER than a subject, which is the whole of what
        // `state-error` used to say here — see `stage_role`.
        let (subject, role) = if job.trouble.is_empty() {
            (&job.subject, Role::Body)
        } else {
            (&job.trouble, Role::Title)
        };
        fields.put(id::SUBJECT, Run::datum(subject).playing(role));

        if let Some(per) = along(job) {
            fields.put(id::PERCENT, Run::datum(&percent(per)).playing(stage));
        } else {
            // No total: the stage is what there is to say, and it is a word the
            // host translates — not a dash pretending to be a number.
            fields.put(id::PERCENT, Run::label(stage_key(job.stage)).playing(stage));
        }

        let amounts = if job.total == 0 {
            figure(job.amount, job.done)
        } else {
            format!(
                "{} / {}",
                figure(job.amount, job.done),
                figure(job.amount, job.total)
            )
        };
        fields.put(id::AMOUNTS, Run::datum(&amounts));
        fields.done()
    };

    // THE BAR IS NOT ONE RUN, so it cannot go through the row: it is twelve,
    // and what it needs from the layout is where the column starts. It never
    // changes the row's height — every cell of it is a glyph at the body's
    // size, which is the one the row is already as tall as.
    if let Some(field) = layout.field(id::BAR) {
        draw_bar(screen, line, field.at, job);
    }
    cells
}

/// One line of the detail, before it is drawn: an optional glyph, the label's
/// key, and the value it names.
///
/// A name and not the tuple written out, because the whole screen is built as a
/// list of these and then drawn in one loop — which is what keeps the "stop
/// before the last line" rule in ONE place instead of at nine call sites.
type Field<'a> = (Option<Run<Icon>>, &'a str, Run<Icon>);

/// One line of the detail: an optional glyph, a label, and the value it names.
fn field(
    screen: &mut Frame,
    line: u16,
    layout: &Layout,
    mark: Option<Run<Icon>>,
    label: &str,
    value: Run<Icon>,
) -> u16 {
    let mut fields = screen.row_of(line, layout);
    if let Some(mark) = mark {
        fields.put(id::MARK, mark);
    }
    // THE LABEL OF A FACT: nothing on a job's sheet is changed by editing it
    // there, and it is the same part the browsers' sheet draws
    // (`chrome::Kind::Fact` on the host's side).
    fields.put(id::LABEL, Run::label(label).names(false));
    fields.put(id::VALUE, value);
    fields.done()
}

impl State {
    /// The jobs, one per row.
    fn list(&mut self, screen: &mut Frame) {
        let mut line = screen.heading(Icon::Transfer, key::TITLE);
        // The last line belongs to the hint or to the question, and the one
        // above it is left blank so a row never touches it.
        let foot = screen.lines().saturating_sub(1);

        if self.queue.jobs.is_empty() {
            self.list_top = None;
            self.list_lines = 0;
            if line < foot {
                screen.put(line, Run::label(key::EMPTY));
            }
            return;
        }

        let room = usize::from(foot.saturating_sub(AIR).saturating_sub(line)).max(1);
        self.scroll(room);

        // A CELL IS KEPT FOR THE BAR, and every band and every column stops
        // before it: the band is full-bleed and the bar is drawn after it, so
        // without this the control sits on top of the selected row.
        let body = screen.columns().saturating_sub(1);
        let layout = row_layout(body);
        self.list_top = Some(line);
        let first = line;
        // THE BANDS STOP AT THE BAR ONLY WHILE THERE IS ONE: a queue that fits
        // its window is banded to the edge (zero is "to the edge").
        let jobs = self.queue.jobs.len();
        let band = if jobs > room.min(jobs.saturating_sub(self.top)) {
            body
        } else {
            0
        };

        for (n, job) in self.queue.jobs.iter().enumerate().skip(self.top).take(room) {
            // `room` is at least one even in a window that has no room at all;
            // this is what keeps that last row off the hint.
            if line >= foot {
                break;
            }
            if Some(job.id) == self.on {
                screen.ground_in(line, 0, band, Role::Band);
            } else if let Some(seat) = self.hover_on(n) {
                // THE ROW UNDER THE POINTER, in a tone of its own: the cursor
                // is where the KEYBOARD is and the hover is where a click
                // would land.
                screen.ground_in(line, 0, band, Role::hover_of(seat));
            }
            line = line.saturating_add(row(screen, line, job, &layout));
        }
        self.list_lines = line.saturating_sub(first);
        // AND THE BAR, over the rows it belongs to: a queue longer than its
        // window is a queue whose end the mouse could not reach at all.
        // THE BAND AND THE WINDOW ARE ONE NUMBER, and it is how many rows were
        // DRAWN: the host says so out loud when they differ, and with a queue
        // shorter than the room they do.
        screen.bar(
            first,
            self.list_lines,
            screen.columns().saturating_sub(1),
            Scroll {
                total: self.queue.jobs.len(),
                lines: usize::from(self.list_lines),
                offset: self.top,
                top: 0,
            },
        );
    }

    /// WHOSE HAND is on row `n`, if any: a cursor per seat rests on this
    /// buffer, and the row takes the colour of the first one on it
    /// (`Hovers::first`).
    fn hover_on(&self, n: usize) -> Option<u16> {
        self.hover
            .first(|(_, line)| self.row_of_line(line) == Some(n))
    }

    /// Which row of the list a line of the window is, if it is one.
    fn row_of_line(&self, line: u16) -> Option<usize> {
        let first = self.list_top?;
        if line < first || line >= first.saturating_add(self.list_lines) {
            return None;
        }
        let n = self.top + usize::from(line - first);
        (n < self.queue.jobs.len()).then_some(n)
    }

    /// How many rows of the list the window holds.
    fn room(&self) -> usize {
        usize::from(self.list_lines).max(1)
    }

    /// One job, field by field — where the whole path and the whole failure
    /// fit, which a row cannot promise.
    fn detail(&mut self, screen: &mut Frame) {
        let mut line = screen.heading(Icon::List, key::DETAIL);
        let foot = screen.lines().saturating_sub(1);
        self.list_top = None;
        self.list_lines = 0;

        let Some(job) = self.focused() else {
            if line < foot {
                screen.put(line, Run::label(key::EMPTY));
            }
            return;
        };

        let layout = detail_layout(screen.columns());
        let stage = stage_role(job.stage);
        let mut lines: Vec<Field<'_>> = vec![
            (
                Some(Run::glyph(engine_icon(job.engine))),
                key::FIELD_ENGINE,
                Run::label(engine_key(job.engine)),
            ),
            (None, key::FIELD_ERRAND, Run::label(errand_key(job.errand))),
            (
                Some(Run::glyph(stage_icon(job.stage)).playing(stage)),
                key::FIELD_STAGE,
                Run::label(stage_key(job.stage)).playing(stage),
            ),
            (
                None,
                key::FIELD_IDENTITY,
                Run::label(if job.identified {
                    key::IDENTITY_MINE
                } else {
                    key::IDENTITY_ANONYMOUS
                })
                .playing(identity_role(job.identified)),
            ),
            (None, key::FIELD_SUBJECT, Run::datum(&job.subject)),
            (
                None,
                key::FIELD_DONE,
                Run::datum(&figure(job.amount, job.done)),
            ),
            (
                None,
                key::FIELD_TOTAL,
                if job.total == 0 {
                    Run::label(key::UNKNOWN)
                } else {
                    Run::datum(&figure(job.amount, job.total))
                },
            ),
        ];
        if job.answer != 0 {
            lines.push((None, key::FIELD_ANSWER, Run::datum(&size(job.answer))));
        }
        if !job.trouble.is_empty() {
            lines.push((
                None,
                key::FIELD_TROUBLE,
                Run::datum(&job.trouble).playing(Role::Title),
            ));
        }

        for (mark, label, value) in lines {
            if line >= foot {
                return;
            }
            line = line.saturating_add(field(screen, line, &layout, mark, label, value));
        }

        let Some(per) = along(job) else {
            return;
        };
        line = line.saturating_add(AIR);
        if line >= foot {
            return;
        }
        let bar = bar_layout(screen.columns());
        if let Some(field) = bar.field(id::BAR) {
            draw_bar(screen, line, field.at, job);
        }
        let mut fields = screen.row_of(line, &bar);
        fields.put(id::PERCENT, Run::datum(&percent(per)));
    }

    /// The question, on a screen of its own: the job it is about, what giving
    /// up does, and `no` before `yes` — the footer panel's question, line for
    /// line.
    fn question(&mut self, screen: &mut Frame, q: GiveUp) {
        let mut line = screen.heading(Icon::Transfer, key::TITLE);
        let foot = screen.lines().saturating_sub(1);
        self.list_top = None;
        self.list_lines = 0;
        self.answers_at = None;
        let body = screen.columns().saturating_sub(1);
        if let Some(job) = self.queue.jobs.iter().find(|j| j.id == q.id)
            && line < foot
        {
            let layout = row_layout(body);
            line = line.saturating_add(row(screen, line, job, &layout));
        }
        // THE SENTENCE WRAPS, and the host wraps it: this side has only the
        // key (invariant 11).
        let sentence = 2;
        if line.saturating_add(sentence) >= foot {
            return;
        }
        screen.paragraph(
            line,
            sentence,
            Run::label(key::CONFIRM_CANCEL).playing(Role::Body),
        );
        line = line.saturating_add(sentence);
        if line.saturating_add(2) >= foot {
            return;
        }
        self.answers_at = Some(line);
        for (yes, said) in [(false, key::GIVE_UP_NO), (true, key::GIVE_UP_YES)] {
            if q.yes == yes {
                screen.ground_in(line, 0, 0, Role::Band);
            } else if let Some(seat) = self.hover.first(|(_, at)| at == line) {
                screen.ground_in(line, 0, 0, Role::hover_of(seat));
            }
            screen.put(line, Run::label(said).names(true));
            line = line.saturating_add(1);
        }
    }

    /// The foot's pairs, laid out from the left: every one that fits, WHOLE,
    /// and where it went — the ONE walk the drawing and the click both read.
    fn lay_foot(&self, columns: u16) -> Vec<Pair> {
        let edge = columns.saturating_sub(1);
        let mut out: Vec<Pair> = Vec::new();
        let mut column = 0;
        for (keys, id) in self.taught() {
            let said = self.phrases.or(id, "");
            let phrase = if said.is_empty() {
                TEACH_CELLS
            } else {
                ronoctua_design::wide(&said)
            };
            let cells = ronoctua_design::wide(keys) + ronoctua_design::GAP + phrase;
            let lead = if out.is_empty() { 0 } else { SEPARATOR_CELLS };
            if column + lead + cells > edge {
                break;
            }
            out.push(Pair {
                at: column + lead,
                cells,
                keys,
                id,
            });
            column += lead + cells;
        }
        out
    }

    /// The keys the foot teaches — the question's answer, or what this page
    /// and the row under the cursor allow: `transfers_taught`, the answer the
    /// footer's panel reads too.
    fn taught(&self) -> Vec<(&'static str, &'static str)> {
        if self.asking.is_some() {
            return TRANSFERS_ANSWER.to_vec();
        }
        transfers_taught(self.face == Face::Detail, self.focused().map(facts))
            .into_iter()
            .map(|(keys, id, _)| (keys, id))
            .collect()
    }

    /// The last line: the keys this page answers to, every one a CONTROL —
    /// the notation lights under the pointer, and a click does what the key
    /// does.
    fn foot(&mut self, screen: &mut Frame) {
        let line = screen.lines().saturating_sub(1);
        let pairs = self.lay_foot(screen.columns());
        for (n, pair) in pairs.iter().enumerate() {
            if n > 0 {
                screen.put(
                    line,
                    Run::glyph(Icon::Separator)
                        .playing(Role::Faint)
                        .at(pair.at - SEPARATOR_CELLS + ronoctua_design::GAP),
                );
            }
            let lit = self
                .hover
                .first(|(column, at)| {
                    at == line && column >= pair.at && column < pair.at + pair.cells
                })
                .is_some();
            let wide = ronoctua_design::wide(pair.keys);
            screen.put(
                line,
                Run::datum(pair.keys)
                    .playing(if lit { Role::KeyLit } else { Role::Key })
                    .at(pair.at),
            );
            screen.put(
                line,
                Run::label(pair.id)
                    .playing(if lit { Role::ProseLit } else { Role::Prose })
                    .at(pair.at + wide + ronoctua_design::GAP)
                    .within(pair.cells.saturating_sub(wide + ronoctua_design::GAP)),
            );
        }
        self.foot = pairs;
        self.foot_line = line;
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
        vec![Effect::Redraw]
    }

    /// A fresh queue arrived.
    fn receive(&mut self, queue: Queue) -> Vec<Effect> {
        if same_queue(&queue, &self.queue) {
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
        }
        // A QUESTION ABOUT A JOB THAT ENDED goes: there is nothing left to
        // give up, and a `yes` taken later would be about nothing.
        if let Some(q) = self.asking
            && !self
                .queue
                .jobs
                .iter()
                .any(|j| j.id == q.id && facts(j).afoot)
        {
            self.asking = None;
        }
        vec![Effect::Redraw]
    }

    /// What a command of the queue's table does, on the row under the
    /// cursor — the same answers the footer's panel gives.
    fn run(&mut self, id: &str) -> Vec<Effect> {
        let row = self.focused().map(facts);
        let target = self.on.unwrap_or(0);
        let ask = |what| vec![Effect::Ask(Demand { what, target }), Effect::Redraw];
        match id {
            "transfers-down" => self.step(true),
            "transfers-up" => self.step(false),
            // THE JOB'S OWN PAGE, and back — what a second click on a row does.
            "transfers-detail" => match self.face {
                Face::List if row.is_some() => {
                    self.face = Face::Detail;
                    vec![Effect::Redraw]
                }
                Face::List => Vec::new(),
                Face::Detail => {
                    self.face = Face::List;
                    vec![Effect::Redraw]
                }
            },
            // PAUSE WHAT IS RUNNING, LET GO OF WHAT IS HELD — one key for
            // both, because the state is on screen. Only where the engine can
            // hold: a program's install is one run nothing freezes halfway.
            "transfers-pause" => match row {
                Some(r) if r.held => ask(Request::ResumeJob),
                Some(r) if r.afoot && r.pauses => ask(Request::PauseJob),
                _ => Vec::new(),
            },
            // THE ONE IRREVERSIBLE KEY HERE, and it asks first.
            "transfers-cancel" => match (self.on, row) {
                (Some(id), Some(r)) if r.afoot => {
                    self.asking = Some(GiveUp { id, yes: false });
                    vec![Effect::Redraw]
                }
                _ => Vec::new(),
            },
            "transfers-forget" => match row {
                Some(r) if !r.afoot => ask(Request::ForgetJob),
                _ => Vec::new(),
            },
            "transfers-sweep" => ask(Request::SweepJobs),
            // WHAT AN INSTALL MADE, opened by the host — which alone knows the
            // entry, and hands it to the footer that starts every program.
            "transfers-launch" => match row {
                Some(r) if r.opens => ask(Request::LaunchJob),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// The question's answer, the one the cursor is on.
    fn answer(&mut self) -> Vec<Effect> {
        match self.asking.take() {
            Some(GiveUp { id, yes: true }) => vec![
                Effect::Ask(Demand {
                    what: Request::CancelJob,
                    target: id,
                }),
                Effect::Redraw,
            ],
            Some(GiveUp { yes: false, .. }) => vec![Effect::Redraw],
            None => Vec::new(),
        }
    }

    /// A key that walks while the question is up: between its two answers.
    fn walk_the_answers(&mut self, down: bool) -> Vec<Effect> {
        match self.asking.as_mut() {
            Some(q) if q.yes != down => {
                q.yes = down;
                vec![Effect::Redraw]
            }
            _ => Vec::new(),
        }
    }
}

/// What a job allows, as the queue's shared table reads it.
const fn facts(job: &Job) -> TransferRow {
    TransferRow {
        afoot: matches!(job.stage, Stage::Weighing | Stage::Running | Stage::Paused),
        held: matches!(job.stage, Stage::Paused),
        pauses: job.pauses,
        opens: job.opens,
    }
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "transfers".into(),
            mode: "transfers".into(),
            // THE TWO SCREENS, and they were already two: the `Face` enum in
            // this file has said so since the panel existed. What is new is that
            // the modeline gets to say which one is up — before this, the queue
            // and one job field by field were the same word on screen.
            states: vec![
                BufferState {
                    id: "normal".into(),
                    letter: 'n',
                    tone: Tone::Standard,
                },
                BufferState {
                    id: "detail".into(),
                    letter: 'd',
                    tone: Tone::Standard,
                },
            ],
            keymap: keymap(),
            commands: commands(),
            segments: vec![],
            // THE ONE THAT MATTERS: without it the host neither subscribes to
            // the queue nor forwards a demand, and this buffer is an empty
            // screen with a keymap that does nothing.
            capabilities: vec![Capability::Transfers],
            // THE PHRASES THE FOOT TEACHES, asked so its pairs can be MEASURED.
            // Everything else this buffer shows goes out as a key and the host
            // translates it when it draws.
            phrases: taught_phrases(),
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        STATE.with_borrow_mut(|s| {
            let mut screen = Frame::of(&ctx);
            match (s.asking, s.face) {
                (Some(q), _) => s.question(&mut screen, q),
                (None, Face::List) => s.list(&mut screen),
                (None, Face::Detail) => s.detail(&mut screen),
            }
            // ONE PLACE, and after both faces: the last line says the same
            // thing whichever of them is up, and a face that forgot to draw it
            // would be a window with no way out written on it.
            s.foot(&mut screen);
            screen.finish_wit()
        })
    }

    fn event(e: Input) -> Vec<Effect> {
        // THE STATE IS REPORTED FROM ONE PLACE, and not at each site that
        // assigns `face`. There are five of those and there will be more; a
        // report per site is a report somebody forgets, and what it costs is the
        // modeline going on saying `normal` with a job's fields on screen.
        //
        // Comparing before against after also gets the case no site could report
        // by itself: `Cancel` leaves `Detail` only SOMETIMES — a pending
        // question eats it first.
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
    /// DERIVED and never stored: a stored copy is a second source of truth for
    /// something `face` already answers, and the two would drift on the first
    /// path that forgot to update it.
    const fn state_id(&self) -> &'static str {
        match self.face {
            Face::List => "normal",
            Face::Detail => "detail",
        }
    }

    /// One input, before the state is compared.
    fn act(&mut self, e: Input) -> Vec<Effect> {
        let s = self;
        match e {
            // THE HOST SENDS THE ID THIS BUFFER DECLARED, so an id from anywhere
            // else is a bug on the other side and is ignored rather than guessed
            // at. A state entered also lets the question go: a `yes` waiting
            // under a page nobody is looking at is a key nobody can see.
            Input::EnterState(id) => {
                s.face = match id.as_str() {
                    "detail" => Face::Detail,
                    _ => Face::List,
                };
                s.asking = None;
                vec![Effect::Redraw]
            }
            Input::Queue(queue) => s.receive(queue),
            // WHILE THE QUESTION IS UP it takes the keys: the arrows walk its
            // two answers, `RET` takes one, `Esc` is `no`, and nothing else
            // reaches the list under it.
            Input::Command(id) if s.asking.is_some() => match id.as_str() {
                "transfers-down" => s.walk_the_answers(true),
                "transfers-up" => s.walk_the_answers(false),
                _ => Vec::new(),
            },
            Input::Down if s.asking.is_some() => s.walk_the_answers(true),
            Input::Up if s.asking.is_some() => s.walk_the_answers(false),
            Input::Confirm if s.asking.is_some() => s.answer(),
            Input::Cancel if s.asking.is_some() => {
                s.asking = None;
                vec![Effect::Redraw]
            }
            Input::Toggle | Input::Character(_) if s.asking.is_some() => Vec::new(),
            Input::Command(id) => s.run(&id),
            Input::Down => s.step(true),
            Input::Up => s.step(false),
            // `RET` IS THE JOB'S PAGE, the second click's meaning — the
            // table's (`TRANSFERS_KEYS`), in both doors.
            Input::Confirm | Input::Toggle => s.run("transfers-detail"),
            Input::Character(' ') => s.run("transfers-pause"),
            Input::Cancel => {
                if s.face == Face::Detail {
                    s.face = Face::List;
                    vec![Effect::Redraw]
                } else {
                    vec![Effect::Close]
                }
            }
            // THE WORDS FOR THE KEYS THE FOOT TEACHES, so its pairs can be
            // measured. They arrive once, before the first frame.
            Input::Phrases(said) => {
                s.phrases = ronoctua_design::Phrases::of(
                    said.into_iter().map(|p| (p.key, p.text)).collect(),
                );
                vec![Effect::Redraw]
            }
            // THE POINTER AT REST, and the two gestures that move the window.
            Input::Hover(p) => {
                // PER CURSOR: `p.seat` says which hand, and each keeps its own cell.
                if !s.hover.set(p.seat, (p.at.column, p.at.line)) {
                    return Vec::new();
                }
                vec![Effect::Redraw]
            }
            Input::Unhover(seat) => {
                // ONLY THAT HAND'S: another cursor resting here keeps its row lit.
                if !s.hover.clear(seat) {
                    return Vec::new();
                }
                vec![Effect::Redraw]
            }
            // ONE SELECTION FOR EVERY KEYBOARD: this buffer keeps a single
            // cursor, which every keyboard typing here moves.
            Input::KeyboardSeat(_) | Input::KeyboardLeft(_) => Vec::new(),
            // THE WHEEL SCROLLS THE WINDOW and leaves the cursor where it is:
            // this page explains whatever the cursor is on, and a wheel that
            // moved it would change the explanation under the hand reading it.
            Input::Wheel(w) => {
                let most = s.queue.jobs.len().saturating_sub(s.room());
                s.top = if w.lines < 0 {
                    s.top.saturating_sub(w.lines.unsigned_abs() as usize)
                } else {
                    (s.top + w.lines as usize).min(most)
                };
                vec![Effect::Redraw]
            }
            // AND THE BAR IS DRAGGED.
            Input::Scrolled(d) => {
                let room = s.room();
                let total = s.queue.jobs.len();
                s.top = (d.first as usize).min(total.saturating_sub(room.min(total)));
                vec![Effect::Redraw]
            }
            Input::Click(p) => s.clicked(p.line, p.column),
            _ => Vec::new(),
        }
    }

    /// A click: a pair of the foot does what its key does; a row, or an
    /// answer, is selected by the FIRST click and taken by the SECOND — its
    /// `RET` (invariant 19), the footer panel's rule too.
    fn clicked(&mut self, line: u16, column: u16) -> Vec<Effect> {
        if line == self.foot_line
            && let Some(pair) = self
                .foot
                .iter()
                .find(|p| column >= p.at && column < p.at + p.cells)
                .copied()
        {
            return match pair.id {
                "transfers-answer" => self.answer(),
                id => self.run(id),
            };
        }
        if let (Some(q), Some(at)) = (self.asking, self.answers_at) {
            let yes = match line.checked_sub(at) {
                Some(0) => false,
                Some(1) => true,
                _ => return Vec::new(),
            };
            if q.yes == yes {
                return self.answer();
            }
            return self.walk_the_answers(yes);
        }
        let Some(first) = self.list_top else {
            return Vec::new();
        };
        if line < first {
            return Vec::new();
        }
        let n = self.top + usize::from(line - first);
        match self.queue.jobs.get(n) {
            Some(job) if self.on == Some(job.id) => self.run("transfers-detail"),
            Some(job) => {
                self.on = Some(job.id);
                vec![Effect::Redraw]
            }
            None => Vec::new(),
        }
    }
}

ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_design::types::{Heights, Text};

    /// The production heights: `typography.nix`'s five sizes against a line
    /// height of 1.45, on a cell of `20 × 1.45 = 29 px`. `Large` is TWO cells,
    /// which is the case every geometry defect in this repository came from.
    const fn heights() -> Heights {
        Heights {
            small: 1,
            base: 1,
            normal: 1,
            large: 2,
            huge: 2,
        }
    }

    const fn context(columns: u16, lines: u16) -> Context {
        Context {
            columns,
            lines,
            heights: heights(),
        }
    }

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
            pauses: true,
            opens: false,
        }
    }

    /// One of every shape a row can take: a bar, no total, a failure, an
    /// identified job, and an answered measurement.
    fn every_shape() -> Vec<Job> {
        let mut running = job(1, Stage::Running, 3 << 30, 9 << 30);
        running.subject = "/home/noctua/videos/a really long thing here.mkv".to_owned();
        let mut unknown = job(2, Stage::Weighing, 900, 0);
        unknown.engine = Engine::Torrent;
        unknown.errand = Errand::Torrent;
        let mut broken = job(3, Stage::Failed, 1, 10);
        broken.trouble = "the server closed the connection".to_owned();
        let mut mine = job(4, Stage::Paused, 5, 10);
        mine.identified = true;
        mine.engine = Engine::Media;
        mine.errand = Errand::Media;
        let mut measured = job(5, Stage::Done, 10, 10);
        measured.errand = Errand::Measure;
        measured.amount = Amount::Items;
        measured.answer = 42 << 20;
        vec![running, unknown, broken, mine, measured]
    }

    /// Draws one face of one state and hands the frame back.
    fn drawn(face: Face, jobs: Vec<Job>, on: Option<u64>, pending: bool, ctx: &Context) -> Frame {
        let mut s = State::fresh();
        s.receive(Queue { jobs });
        s.face = face;
        if on.is_some() {
            s.on = on;
        }
        if pending {
            s.asking = s.on.map(|id| GiveUp { id, yes: false });
        }
        let mut screen = Frame::of(ctx);
        match (s.asking, s.face) {
            (Some(q), _) => s.question(&mut screen, q),
            (None, Face::List) => s.list(&mut screen),
            (None, Face::Detail) => s.detail(&mut screen),
        }
        s.foot(&mut screen);
        screen
    }

    /// THE HOST'S TWO QUESTIONS, ASKED HERE — in every state, at three widths.
    ///
    /// `Screen::check` runs `DisplayList::nothing_is_covered` and
    /// `nothing_overlaps`, which are the host's own code and not a second copy
    /// of the question. Forty-six cells is the footer's region, eighty a
    /// comfortable window, a hundred and ten a wide one.
    #[test]
    fn every_state_survives_the_hosts_two_questions() {
        for columns in [46u16, 80, 110] {
            for lines in [3u16, 8, 24, 40] {
                for face in [Face::List, Face::Detail] {
                    for jobs in [Vec::new(), every_shape()] {
                        for pending in [false, true] {
                            let ctx = context(columns, lines);
                            let screen = drawn(face, jobs.clone(), Some(3), pending, &ctx);
                            screen.check().unwrap_or_else(|e| {
                                panic!("{face:?} {columns}x{lines} pending={pending}: {e}")
                            });
                        }
                    }
                }
            }
        }
    }

    /// THE HEADING IS A GLYPH AND A TITLE, AND THERE IS NO RULE.
    ///
    /// This buffer drew one — a `Command::Rule` at `line + 1` — and it was one
    /// of the three answers to "what closes a heading" that the design crate
    /// exists to end. What separates a title from its content is whitespace.
    #[test]
    fn the_heading_is_a_glyph_and_a_title_and_nothing_else() {
        let mut screen = Frame::of(&context(80, 24));
        let content = screen.heading(Icon::Transfer, key::TITLE);
        let out = screen.finish_wit();
        assert_eq!(out.len(), 2, "a glyph and a title, and no more");
        assert!(
            !out.iter().any(|c| matches!(c, Command::Rule(_))),
            "a rule appeared under a heading"
        );
        assert!(
            out.iter()
                .any(|c| matches!(c, Command::Text(t) if matches!(t.content, Text::Icon(_)))),
            "no glyph"
        );
        assert!(
            out.iter()
                .any(|c| matches!(c, Command::Text(t) if matches!(t.content, Text::Key(_)))),
            "no title"
        );
        // The air is equal on both sides, and the title's box is what `Heights`
        // says it is — never one row.
        assert_eq!(content, AIR + heights().normal + AIR);
    }

    /// NOTHING LEAKS, at every width there is, in all three tables.
    #[test]
    fn no_column_ever_ends_past_the_edge() {
        for columns in 0..=240u16 {
            for layout in [
                row_layout(columns),
                detail_layout(columns),
                bar_layout(columns),
            ] {
                for f in layout.fields() {
                    assert!(
                        f.at.saturating_add(f.width) <= columns,
                        "{columns}: `{}` runs from {} for {}",
                        f.id,
                        f.at,
                        f.width
                    );
                }
            }
        }
    }

    /// THE ORDER OF SACRIFICE IS THE DECLARED ONE: the amounts, then the bar,
    /// and the percentage last. Narrowing the window never brings a column
    /// back, and never drops one out of turn.
    #[test]
    fn a_narrow_window_drops_columns_in_the_declared_order() {
        let all = [
            id::STAGE,
            id::ENGINE,
            id::ERRAND,
            id::SUBJECT,
            id::BAR,
            id::PERCENT,
            id::AMOUNTS,
        ];
        let gone = |columns: u16| -> Vec<&'static str> {
            let layout = row_layout(columns);
            all.into_iter().filter(|id| !layout.has(id)).collect()
        };
        assert_eq!(gone(200), Vec::<&str>::new(), "a wide window keeps all");

        let order = [
            id::AMOUNTS,
            id::BAR,
            id::PERCENT,
            id::ERRAND,
            id::ENGINE,
            id::STAGE,
        ];
        let mut before: Vec<&'static str> = Vec::new();
        for columns in (0..=200u16).rev() {
            let now = gone(columns);
            for id in &before {
                assert!(now.contains(id), "{columns}: `{id}` came back");
            }
            for id in &now {
                if let Some(rank) = order.iter().position(|o| o == id) {
                    for earlier in &order[..rank] {
                        assert!(
                            now.contains(earlier),
                            "{columns}: `{id}` went before `{earlier}`"
                        );
                    }
                }
            }
            before = now;
        }
        // And the name is never what goes: it shrinks instead.
        assert!(row_layout(20).has(id::SUBJECT));
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

    /// A queue that says the same thing costs no frame — the comparison this
    /// buffer writes by hand now that the derive is the design crate's.
    #[test]
    fn a_queue_that_did_not_change_asks_for_no_redraw() {
        let mut s = State::fresh();
        assert!(
            !s.receive(Queue {
                jobs: vec![job(1, Stage::Running, 1, 10)]
            })
            .is_empty()
        );
        assert!(
            s.receive(Queue {
                jobs: vec![job(1, Stage::Running, 1, 10)]
            })
            .is_empty()
        );
        // One byte further along IS a change: the bar and the amounts read it.
        assert!(
            !s.receive(Queue {
                jobs: vec![job(1, Stage::Running, 2, 10)]
            })
            .is_empty()
        );
    }

    /// FAILS IF: giving up stops asking first, or the question opens on
    /// `yes`, or `Esc` gives up, or the answer names the row and not the id,
    /// or a click takes an answer it did not select first.
    #[test]
    fn giving_up_asks_first_with_no_under_the_cursor() {
        let mut s = State::fresh();
        s.receive(Queue {
            jobs: vec![job(7, Stage::Running, 1, 10), job(8, Stage::Paused, 1, 10)],
        });
        // The first press only puts the question up; nothing crosses.
        let out = s.act(Input::Command("transfers-cancel".into()));
        assert!(matches!(out.as_slice(), [Effect::Redraw]));
        assert_eq!(s.asking, Some(GiveUp { id: 7, yes: false }));
        // `RET` ON NO keeps it, and so does `Esc`.
        assert!(
            !s.act(Input::Confirm)
                .iter()
                .any(|e| matches!(e, Effect::Ask(_)))
        );
        assert_eq!(s.asking, None);
        s.act(Input::Command("transfers-cancel".into()));
        s.act(Input::Cancel);
        assert_eq!(s.asking, None);
        // `↓` AND `RET` send it, naming the id and not the row.
        s.act(Input::Command("transfers-cancel".into()));
        s.act(Input::Down);
        assert!(asks(&s.act(Input::Confirm), Request::CancelJob, 7));
        // BY THE POINTER, on the drawn answers: the first click selects.
        s.act(Input::Command("transfers-cancel".into()));
        let ctx = context(80, 24);
        let mut screen = Frame::of(&ctx);
        s.question(&mut screen, s.asking.expect("asking"));
        let at = s.answers_at.expect("the answers were drawn");
        assert!(
            s.clicked(at + 1, 2)
                .iter()
                .all(|e| !matches!(e, Effect::Ask(_)))
        );
        assert_eq!(s.asking, Some(GiveUp { id: 7, yes: true }));
        assert!(asks(&s.clicked(at + 1, 2), Request::CancelJob, 7));
        // WHAT HAS ENDED is not asked about.
        s.receive(Queue {
            jobs: vec![job(7, Stage::Done, 10, 10), job(8, Stage::Paused, 1, 10)],
        });
        assert!(s.run("transfers-cancel").is_empty());
    }

    /// FAILS IF: the one key that holds and lets go sends the wrong verb, holds
    /// what its engine cannot hold, or holds what has ended — the footer
    /// panel's answers, key for key.
    #[test]
    fn hold_reads_the_stage_and_never_sends_the_wrong_verb() {
        let mut s = State::fresh();
        s.receive(Queue {
            jobs: vec![job(3, Stage::Running, 1, 10)],
        });
        assert!(asks(&s.act(Input::Character(' ')), Request::PauseJob, 3));
        s.receive(Queue {
            jobs: vec![job(3, Stage::Paused, 1, 10)],
        });
        assert!(asks(&s.run("transfers-pause"), Request::ResumeJob, 3));
        // Finished: nothing is sent at all, rather than a request the host
        // would have to refuse.
        s.receive(Queue {
            jobs: vec![job(3, Stage::Done, 10, 10)],
        });
        assert!(s.run("transfers-pause").is_empty());
        // A PROGRAM'S INSTALL cannot be held.
        let mut install = job(4, Stage::Running, 1, 10);
        install.pauses = false;
        s.receive(Queue {
            jobs: vec![install],
        });
        assert!(s.run("transfers-pause").is_empty());
    }

    /// FAILS IF: the window loses the footer panel's launch — a finished
    /// install's row opening what it made, through the host — or teaches it
    /// on a row that made nothing.
    #[test]
    fn a_finished_install_opens_from_its_row_here_too() {
        let mut s = State::fresh();
        let mut installed = job(5, Stage::Done, 1, 1);
        installed.opens = true;
        s.receive(Queue {
            jobs: vec![installed],
        });
        assert!(asks(
            &s.act(Input::Command("transfers-launch".into())),
            Request::LaunchJob,
            5
        ));
        assert!(
            s.taught()
                .iter()
                .any(|(k, id)| *k == "C-o" && *id == "transfers-launch")
        );
        s.receive(Queue {
            jobs: vec![job(6, Stage::Done, 1, 1)],
        });
        assert!(s.run("transfers-launch").is_empty());
        assert!(!s.taught().iter().any(|(_, id)| *id == "transfers-launch"));
    }

    /// FAILS IF: the window's keys stop being the footer panel's table — a
    /// chord of it unbound here — or `RET` stops opening the job's page, or
    /// the second click stops doing what `RET` does.
    #[test]
    fn the_keys_are_the_panels_and_ret_is_the_page() {
        let bound: Vec<String> = keymap().into_iter().map(|b| b.keys).collect();
        for (notation, id) in TRANSFERS_KEYS {
            if let Some(local) = local_binding(notation) {
                assert!(
                    bound.contains(&local),
                    "{notation} ({id}) is not bound here"
                );
            }
        }
        assert!(commands().contains(&"transfers-cancel".to_owned()));
        let mut s = State::fresh();
        s.receive(Queue {
            jobs: vec![job(1, Stage::Running, 1, 10), job(2, Stage::Running, 1, 10)],
        });
        s.act(Input::Confirm);
        assert_eq!(s.face, Face::Detail);
        s.act(Input::Cancel);
        assert_eq!(s.face, Face::List);
        // THE FIRST CLICK SELECTS, the second opens.
        let ctx = context(80, 24);
        let mut screen = Frame::of(&ctx);
        s.list(&mut screen);
        let first = s.list_top.expect("the list was drawn");
        s.clicked(first + 1, 4);
        assert_eq!((s.on, s.face), (Some(2), Face::List));
        s.clicked(first + 1, 4);
        assert_eq!(s.face, Face::Detail);
    }

    /// FAILS IF: a pair of the foot stops being a control — a click on it
    /// not doing what its key does — or the foot teaches what the row does
    /// not allow.
    #[test]
    fn the_foot_is_the_keys_and_every_pair_is_a_click() {
        let mut s = State::fresh();
        s.receive(Queue {
            jobs: vec![job(1, Stage::Running, 1, 10)],
        });
        let ctx = context(110, 24);
        let mut screen = Frame::of(&ctx);
        s.list(&mut screen);
        s.foot(&mut screen);
        let ids: Vec<&str> = s.foot.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            [
                "transfers-detail",
                "transfers-pause",
                "transfers-cancel",
                "transfers-sweep"
            ]
        );
        let cancel = s.foot[2];
        s.clicked(s.foot_line, cancel.at);
        assert_eq!(s.asking, Some(GiveUp { id: 1, yes: false }));
    }

    /// `Effect` is a WIT variant and `wit-bindgen` gives it `Clone` and a
    /// `Debug`, so this is what `contains(&Effect::Ask(..))` used to be.
    fn asks(out: &[Effect], what: Request, target: u64) -> bool {
        out.iter()
            .any(|e| matches!(e, Effect::Ask(d) if d.what == what && d.target == target))
    }

    #[test]
    fn a_total_of_zero_never_draws_a_bar() {
        let j = job(1, Stage::Running, 900, 0);
        assert_eq!(along(&j), None);
        let mut screen = Frame::of(&context(80, 24));
        draw_bar(&mut screen, 0, 0, &j);
        assert!(
            screen.finish_wit().is_empty(),
            "a bar with no total would invent the fraction"
        );
    }
}
