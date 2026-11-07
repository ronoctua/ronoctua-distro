//! The brain's screen — `LEADER b`, and a window of its own.
//!
//! # What it is for
//!
//! The machine does things by itself, and this is where it says what, when, and
//! what came of it. Two questions: *what is my machine doing* on the home page,
//! and *exactly what does that mean* on a task's own page.
//!
//! # It reads nothing
//!
//! Invariant 11. The host holds the socket to `ronoctua-brain` on a thread and
//! pushes `input::mind` in; this only decides what to draw. Everything that
//! touches the machine leaves as `effect::ask` — the verbs — or `effect::tune`
//! — the preferences —, and the host refuses both unless this descriptor
//! declared `capability::brain`.
//!
//! It has no clock either, which is why `mind.now` crosses: a countdown is
//! `deadline - now`, and without the second number the first is a moment this
//! side cannot place.
//!
//! # Dates are ISO, and that is not a style choice
//!
//! A component cannot translate (invariant 11), and *"19 ago"* against
//! *"19 Aug"* is exactly the kind of thing it must not invent. What crosses is
//! the number for the arithmetic this side does and the TEXT for the date it
//! cannot compute.
//!
//! # The shape of a page
//!
//! Three fixed things and a body between them:
//!
//! ```text
//! header    one line, its own ground: the glyph, the name, every key
//! warning   one line, only when something is in the way — otherwise nothing
//! body      the page, which scrolls and has a bar of its own
//! footer    its own ground: everything about the thing under the cursor
//! ```
//!
//! The header and the footer are what the owner asked for, and both replace
//! something that did not work. The header replaces a three-line heading whose
//! only content was a glyph and a title — and, more to the point, it replaces a
//! line of shortcuts that was not drawn AT ALL: `foot()` built a table with
//! five `Column::fill` in it, `Table::resolve` admits one, nothing placed, and
//! the page taught no keys to anybody. The footer replaces a dim line under the
//! cursor that could say one row's worth of a description written to be three.

use ronoctua_design::guest::Wit;
use ronoctua_design::screen::Pen;
use ronoctua_design::types::{
    ActivityRow, Binding, BrainTone, BufferState, Capability, Column, Context, Demand, Descriptor,
    Effect, Guest, HoldKind, Icon, Input, Knob, Layout, LegendSwatch, LogLevel, LogLine, Mind,
    ModuleRow, ModuleScreen, ModuleSection, Notice, NoticeLevel, OutcomeKind, Request, Role,
    RowFact, Run, RunRecord, Screen, ScreenCell, Scroll, SectionFold, SectionState, Setting,
    Standing, StepSwitch, Table, TaskKind, TaskState, Timed, Tone,
};
use ronoctua_design::{Phrases, Pointed, Region, Slice};
use std::cell::RefCell;

/// Every text key this buffer names as a literal.
///
/// A module of whole `pub const` strings, because that is the shape
/// `buffer-keys-have-text` can see: a key built with `format!` is invisible to
/// it, and the first sign of a missing one is its own name in brackets in front
/// of somebody.
///
/// The keys a MODULE owns — a task's name, what it does, why it waits, a
/// step's name — are never built here: the host sends them beside the ids
/// (`name-key`, `why-key`, `at-key`…), because a module's words are its own
/// and a key built by convention on this side is a key nobody declared. What
/// is still built by hand — `brain-hold-<word>`, `brain-outcome-<word>` — is
/// the brain's own closed vocabulary, one word per case of a WIT enum.
mod key {
    pub const TITLE: &str = "brain-title";
    pub const NOTHING: &str = "brain-nothing";

    // ---- the headings -------------------------------------------------
    /// The list of everything the brain has.
    pub const TASKS: &str = "brain-tasks";
    /// The log of everything it has done.
    pub const ACTIVITY: &str = "brain-activity";
    /// When a task may run, and what its module offers on its page.
    pub const OPTIONS: &str = "brain-options";
    /// What a task runs, step by step.
    pub const STEPS: &str = "brain-steps";
    /// How the last few went.
    pub const HISTORY: &str = "brain-history";
    /// What every module wants said, on the home page.
    pub const NOTICES: &str = "brain-notices";
    /// What a module that only listens says instead of a date.
    pub const WATCHING: &str = "brain-watching";
    /// The footer's two other facts about a folded row.
    pub const FIRST: &str = "brain-activity-first";
    pub const TIMES: &str = "brain-activity-times";
    /// Since when a notice has been true.
    pub const SINCE: &str = "brain-since";

    // ---- the schedule's rows ------------------------------------------
    pub const ENABLED: &str = "brain-enabled";
    pub const ENABLED_WATCH: &str = "brain-enabled-watch";
    /// What the switch on that row is for, beside its answer.
    pub const ENABLED_HINT: &str = "brain-enabled-hint";
    pub const ENABLED_HINT_WATCH: &str = "brain-enabled-hint-watch";
    pub const ON: &str = "brain-on";
    pub const OFF: &str = "brain-off";
    pub const OFF_WATCH: &str = "brain-off-watch";
    pub const EVERY: &str = "brain-every";
    pub const DAYS: &str = "brain-days";
    pub const PART: &str = "brain-part";
    pub const NEXT: &str = "brain-next";
    pub const SOON: &str = "brain-soon";
    pub const REHEARSAL: &str = "brain-rehearsal";
    pub const NEVER: &str = "brain-never";
    pub const NEEDS_ROOT: &str = "brain-needs-root";
    pub const FLAKE_OWNED: &str = "brain-flake-owned";
    /// The note beside a step whose switch the person moved.
    pub const CHANGED: &str = "brain-changed";

    // ---- the two things the page can do to a task ---------------------
    pub const RUN_NOW: &str = "brain-run-now";
    pub const REHEARSE: &str = "brain-rehearse-it";

    // ---- the warning line ---------------------------------------------
    /// Why nothing will start, one whole sentence per reason.
    ///
    /// A `Run` carries a key and nothing else — no arguments (invariant 11) —
    /// so a sentence with the reason interpolated is not available. What is
    /// available is a sentence per reason, and it is the better of the two
    /// anyway: *"the processor is too busy"* says the thing, *"held: cpu"*
    /// makes somebody guess.
    pub const HELD_RAM: &str = "brain-held-ram";
    pub const HELD_CPU: &str = "brain-held-cpu";
    pub const HELD_BATTERY: &str = "brain-held-battery";
    pub const HELD_OTHER: &str = "brain-held-other";

    // ---- the daemon's own log -----------------------------------------
    /// The page that follows it.
    pub const LOG: &str = "brain-log";
    /// What that page says before the first line arrives.
    pub const NO_LOG: &str = "brain-no-log";
    pub const ABOUT_LOG: &str = "about-brain-log";

    // ---- the footer ---------------------------------------------------
    pub const LAST_RUN: &str = "brain-last-run";
    /// The label of the step a record stopped at, beside the step's name.
    pub const AT_STEP: &str = "brain-record-at";
    /// What a record says when it ran to the end: no reason to give.
    pub const WENT_WELL: &str = "brain-record-went-well";
    /// What it says when it did not run and kept no reason.
    pub const UNSAID: &str = "brain-record-unsaid";
    /// And when it failed and kept none: where its programs' words went.
    pub const UNSAID_FAILURE: &str = "brain-record-unsaid-failure";
    /// What the footer says above a record of a task's own history, one per
    /// outcome — see `record_heading_key`.
    pub const RECORD_DONE: &str = "brain-record-done";
    pub const RECORD_FAILED: &str = "brain-record-failed";
    pub const RECORD_HELD: &str = "brain-record-held";
    pub const RECORD_SKIPPED: &str = "brain-record-skipped";
    pub const RECORD_NOTED: &str = "brain-record-noted";
    pub const NO_ACTIVITY: &str = "brain-no-activity";
    /// The click at the end of the footer's last row that opens the page of
    /// the task the footer is about.
    pub const OPEN_PAGE: &str = "brain-open-page";

    // ---- what the pointer rests on ------------------------------------
    /// What a task's glyph says, one sentence per kind.
    pub const KIND_ROUTINE: &str = "brain-kind-routine";
    pub const KIND_ONCE: &str = "brain-kind-once";
    pub const KIND_WATCH: &str = "brain-kind-watch";
    /// What the fold's mark on a row of the log means.
    pub const ABOUT_FOLD: &str = "about-brain-fold";
    /// What the `Next run` row answers.
    pub const ABOUT_NEXT: &str = "about-brain-next";
    /// What the footer's click does.
    pub const ABOUT_OPEN_PAGE: &str = "about-brain-open-page";
    /// What each column of a task's row says under the pointer: when it runs
    /// next — `{ $when }` the date the row shows, `{ $why }` the reason it
    /// gives instead of one —
    pub const ABOUT_NEXT_AT: &str = "about-brain-next-at";
    pub const ABOUT_NEXT_NONE: &str = "about-brain-next-none";
    pub const ABOUT_NEXT_WHY: &str = "about-brain-next-why";
    /// — how it stands while it has not run —
    pub const ABOUT_IDLE: &str = "about-brain-standing-idle";
    pub const ABOUT_QUEUED: &str = "about-brain-standing-queued";
    pub const ABOUT_RUNNING: &str = "about-brain-standing-running";
    /// — how its last run went, on `{ $when }` —
    pub const ABOUT_DONE: &str = "about-brain-last-done";
    pub const ABOUT_FAILED: &str = "about-brain-last-failed";
    pub const ABOUT_HELD: &str = "about-brain-last-held";
    pub const ABOUT_SKIPPED: &str = "about-brain-last-skipped";
    pub const ABOUT_NOTED: &str = "about-brain-last-noted";
    /// — and what it waits for, while something is in its way.
    pub const ABOUT_HOLD_RAM: &str = "about-brain-hold-ram";
    pub const ABOUT_HOLD_CPU: &str = "about-brain-hold-cpu";
    pub const ABOUT_HOLD_BATTERY: &str = "about-brain-hold-battery";
    pub const ABOUT_HOLD_BUSY: &str = "about-brain-hold-busy";
    pub const ABOUT_HOLD_NETWORK: &str = "about-brain-hold-network";
    pub const ABOUT_HOLD_ASLEEP: &str = "about-brain-hold-asleep";
    /// THE WAY BACK atop a task's page while the window wears no header, and
    /// what it does under the pointer.
    pub const BACK: &str = "brain-back";
    pub const ABOUT_WAY_BACK: &str = "about-brain-back";
    /// WHAT EVERY OTHER THING DRAWN SAYS UNDER THE POINTER: the page's own
    /// name and each title — what the rows under it are —
    pub const ABOUT_TITLE: &str = "about-brain-title";
    pub const ABOUT_TASKS: &str = "about-brain-tasks";
    pub const ABOUT_NOTICES: &str = "about-brain-notices";
    pub const ABOUT_ACTIVITY: &str = "about-brain-activity";
    pub const ABOUT_OPTIONS: &str = "about-brain-options";
    pub const ABOUT_STEPS: &str = "about-brain-steps";
    pub const ABOUT_HISTORY: &str = "about-brain-history";
    pub const ABOUT_SECTION: &str = "about-brain-section";
    /// — the triangle of a section that folds —
    pub const ABOUT_SECTION_FOLD: &str = "about-brain-section-fold";
    /// — the figures beside a title: how far back and how many, and how
    /// many steps are on —
    pub const ABOUT_REACH: &str = "about-brain-reach";
    pub const ABOUT_STEPS_ON: &str = "about-brain-steps-on";
    /// — a task's description, where it has no line of its own, the
    /// warning line, and the daemon's log with nothing in it yet —
    pub const ABOUT_DOES: &str = "about-brain-does";
    pub const ABOUT_WARNING: &str = "about-brain-held";
    pub const ABOUT_LOG_PAGE: &str = "about-brain-log-page";
    /// — and the footer: itself, a record's heading, the step it stopped
    /// at, a step's command, and each figure on its last row.
    pub const ABOUT_FOOT: &str = "about-brain-foot";
    pub const ABOUT_RECORD: &str = "about-brain-record";
    pub const ABOUT_RECORD_AT: &str = "about-brain-record-at";
    pub const ABOUT_COMMAND: &str = "about-brain-command";
    pub const ABOUT_LAST_RUN: &str = "about-brain-last-run";
    pub const ABOUT_FIRST: &str = "about-brain-first";
    pub const ABOUT_TIMES: &str = "about-brain-times";
    pub const ABOUT_SINCE: &str = "about-brain-since";
    pub const ABOUT_NEEDS_ROOT: &str = "about-brain-needs-root";
    pub const ABOUT_FLAKE_OWNED: &str = "about-brain-flake-owned";

    // ---- what the header teaches --------------------------------------
    pub const DO_OPEN: &str = "brain-do-open";
    pub const DO_BACK: &str = "brain-do-back";
    pub const DO_PANE: &str = "brain-do-pane";
    pub const DO_CHANGE: &str = "brain-do-change";

    // ---- and what each of them does, under the pointer ----------------
    pub const ABOUT_OPEN: &str = "about-brain-do-open";
    pub const ABOUT_PANE: &str = "about-brain-do-pane";
    pub const ABOUT_CHANGE: &str = "about-brain-do-change";
    pub const ABOUT_BACK: &str = "about-brain-do-back";
    pub const ABOUT_ACT: &str = "about-brain-act";
    pub const ABOUT_DISMISS: &str = "about-brain-dismiss";
    pub const ABOUT_SNOOZE: &str = "about-brain-snooze";
    pub const ABOUT_RUN: &str = "about-brain-run";
    pub const ABOUT_REHEARSE: &str = "about-brain-rehearse";
    pub const ABOUT_SWITCH: &str = "about-brain-switch";
    pub const ABOUT_INTERVAL: &str = "about-brain-interval";
    pub const ABOUT_WINDOW: &str = "about-brain-window";
    pub const ABOUT_RESET_STEPS: &str = "about-brain-reset-steps";
    pub const ABOUT_PROCEED: &str = "about-brain-proceed";
    pub const ABOUT_ABORT: &str = "about-brain-abort";
    pub const ABOUT_ROW_ALSO: &str = "about-brain-row-also";
}

/// The commands this buffer registers, and the keys that reach them.
///
/// Data, and under `, ` only: the host refuses at load whoever binds under a
/// global prefix. Each id needs `cmd-<id>` in both languages.
///
/// There is deliberately no key for the two numbers of the gate — how busy is
/// too busy is a policy somebody sets once, in a file they own, and a key that
/// moved it by a step would be a key that quietly made the machine less
/// careful.
const KEYMAP: &[(&str, &str)] = &[
    (", r", "brain-run"),
    (", e", "brain-rehearse"),
    (", a", "brain-abort"),
    (", p", "brain-proceed"),
    (", d", "brain-reset-steps"),
    (", i", "brain-interval"),
    (", w", "brain-window"),
    (", o", "brain-switch"),
    (", n", "brain-act"),
    (", x", "brain-dismiss"),
    (", z", "brain-snooze"),
    (", l", "brain-log"),
    (", v", "brain-row-also"),
];

/// One thing the header teaches: a notation, and the key of what it does.
type Teach = (&'static str, &'static str);

/// What the HOME page answers to, in the order it teaches them.
///
/// # Why the order is a decision
///
/// Because the header is CUT at the width it is given and the pairs that do
/// not fit fall off the end — the same rule the footer's hint line follows, and
/// the same reason: whatever goes last is not taught at any width anybody has.
/// Opening a task comes first because it is how the page is used.
const HOME_TEACHES: &[Teach] = &[
    ("RET", key::DO_OPEN),
    ("TAB", key::DO_PANE),
    (", r", "cmd-brain-run"),
    (", e", "cmd-brain-rehearse"),
    (", l", "cmd-brain-log"),
    (", o", "cmd-brain-switch"),
];

/// And what the LOG's page answers to: the way back, the key that is also the
/// way back, and the rest of the keys.
const LOG_TEACHES: &[Teach] = &[("ESC", key::DO_BACK), (", l", "cmd-brain-log")];

/// And what a TASK's page answers to.
const TASK_TEACHES: &[Teach] = &[
    ("RET", key::DO_CHANGE),
    ("ESC", key::DO_BACK),
    (", r", "cmd-brain-run"),
    (", e", "cmd-brain-rehearse"),
    (", o", "cmd-brain-switch"),
    (", i", "cmd-brain-interval"),
    (", w", "cmd-brain-window"),
    (", d", "cmd-brain-reset-steps"),
];

/// AND THE TWO THAT ANSWER A COUNTDOWN, taught only while one is running.
///
/// They are the band's two buttons. The band draws these very notations, so a
/// person reading the line under the minibuffer is told where the keyboard's
/// answer is — which is inside this buffer, since `LEADER b` became the brain
/// itself and a sequence cannot be a command and a prefix at once.
const ANSWER_TEACHES: &[Teach] = &[(", p", "cmd-brain-proceed"), (", a", "cmd-brain-abort")];

/// AND THE THREE A NOTICE ANSWERS TO, each taught only while the notice in
/// front offers it: its first control, putting it away, putting it off.
///
/// They are the strip's three buttons. The strip under the minibuffer draws
/// these very notations, so a person reading it is told where the keyboard's
/// answer is — here, in the brain's own buffer.
const NOTICE_TEACHES: &[Teach] = &[
    (", n", "cmd-brain-act"),
    (", x", "cmd-brain-dismiss"),
    (", z", "cmd-brain-snooze"),
];

/// AND THE ROW'S SECOND ACTION, taught on a module's page only while the row
/// under the cursor offers one (`module-row::also`): the journal's "ignore
/// this type" beside the press that marks an error seen.
const ALSO_TEACHES: &[Teach] = &[(", v", "cmd-brain-row-also")];

/// The sentence about a taught pair, for the row under the OPENED header —
/// the phrase beside the keys is a few words, and what the pointer asks for is
/// what the key DOES, as every other line of shortcuts in the system says it.
///
/// A `match` and not a third column of the tables, because the tables are
/// read by `the-brain-header-is-generated` pair by pair; a phrase with no
/// sentence falls to its own words rather than to nothing.
fn about_of(says: &str) -> &'static str {
    match says {
        key::DO_OPEN => key::ABOUT_OPEN,
        key::DO_PANE => key::ABOUT_PANE,
        key::DO_CHANGE => key::ABOUT_CHANGE,
        key::DO_BACK => key::ABOUT_BACK,
        "cmd-brain-act" => key::ABOUT_ACT,
        "cmd-brain-dismiss" => key::ABOUT_DISMISS,
        "cmd-brain-snooze" => key::ABOUT_SNOOZE,
        "cmd-brain-run" => key::ABOUT_RUN,
        "cmd-brain-rehearse" => key::ABOUT_REHEARSE,
        "cmd-brain-switch" => key::ABOUT_SWITCH,
        "cmd-brain-interval" => key::ABOUT_INTERVAL,
        "cmd-brain-window" => key::ABOUT_WINDOW,
        "cmd-brain-reset-steps" => key::ABOUT_RESET_STEPS,
        "cmd-brain-proceed" => key::ABOUT_PROCEED,
        "cmd-brain-abort" => key::ABOUT_ABORT,
        "cmd-brain-log" => key::ABOUT_LOG,
        "cmd-brain-row-also" => key::ABOUT_ROW_ALSO,
        _ => "",
    }
}

/// How many rows of the activity log are shown at once.
///
/// EIGHT, by the owner's count. It used to take every line the page had left,
/// which on a tall window was a container the height of the screen holding five
/// entries and a great deal of nothing — *"ele precisa ser compacto (mas ele
/// está enorme)"*. What is past the eighth is reached by the container's own
/// scrollbar, which is what a scrollbar is for.
const LOG_ROWS: u16 = 8;

/// The whole height of the activity block: its title and its rows.
///
/// The title is INSIDE the container's ground, which is the owner's other
/// instruction about it — a heading floating above a box is a heading that
/// belongs to the page, and this one belongs to the box.
const LOG_LINES: u16 = LOG_ROWS + 1;

/// How many notices the home page shows at once.
///
/// THREE, and the block is exactly as tall as what it holds up to that: a
/// machine with nothing to say spends no line on saying so, and one with ten
/// things to say shows the three loudest and a bar for the rest — the strip
/// under the minibuffer has already said the loudest of all.
const NOTICE_ROWS: u16 = 3;

/// The width of the count beside the fold's mark on a row of the log: four
/// digits — fifteen days of an agent counted every ten minutes fold into a
/// row of more than a thousand, and the journal keeps at most two thousand
/// and forty-eight records (`journal::HISTORY`).
const COUNT_CELLS: u16 = 4;

/// What a date column is worth before the host has said how wide its words
/// come out — the old reservation, which a measured column replaces.
const WHEN_CELLS: u16 = 17;

/// And a status's, for the same reason: the longest translation's, with air.
const NOTE_CELLS: u16 = 20;

/// And a page's schedule labels: the longest translation's —
/// `brain-enabled-watch`, in English.
const SCHEDULE_LABEL_CELLS: u16 = 22;

/// And the switch's answer while it is off: the longest translation's —
/// `brain-off`, in Portuguese.
const SWITCH_CELLS: u16 = 25;

/// How wide the name in the header may be.
///
/// Fixed, because it is a KEY and a blind one: the descriptor asks the host for
/// the SHORTCUT phrases, not for every task's name, so this side cannot measure
/// it. What it can do is reserve and let the host elide.
///
/// TWENTY-SIX, and the number is the longest translation with room to spare.
/// It was twenty-two, which was English's: the Emacs watch's name comes out as
/// `Vigiar o daemon do Emacs`, twenty-four cells, and the host cut it. A
/// budget sized to one language is the same defect the listing's `note` column
/// had — see the trap the `AGENTS.md` keeps about it.
const NAME_CELLS: u16 = 26;

/// The widest a MEASURED name is given: a name is as wide as it comes out —
/// `Vigiar a central de segurança` is twenty-nine cells, past the blind
/// reservation above, and was cut at it — up to a ceiling that leaves the
/// rest of the row something to say.
const NAME_MOST: u16 = 40;

/// What names a line of the activity log across minds — see
/// [`State::line_key`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct LogKey {
    /// The fold it belongs to.
    fold: (String, u64),
    /// For one of the fold's older records, when that record began; `None`
    /// for the row itself.
    member: Option<u64>,
}

/// Which page is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    /// Everything the brain has, and everything it has done.
    Home,
    /// One task: what it is, when it runs, what it runs, how it has gone.
    Task,
    /// The daemon's own log, followed live: every line it writes — each tick,
    /// each decision, each warning and error — as it wrote it.
    Log,
}

/// Which part of the home page the keyboard is in.
///
/// A RING, walked by `Tab`: the notices when there are any, the tasks, the
/// log, and round again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    /// What every module wants said.
    Notices,
    /// The list of tasks.
    Tasks,
    /// The log of what has happened.
    Log,
}

/// One line of the activity log as it is DRAWN: a folded row, or one of the
/// records a folded row opened into.
///
/// The cursor walks these and not the rows: a row opened is several lines,
/// each of which the footer can explain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    /// The row itself — its newest record, and the fold's mark when it folds
    /// more than one.
    Row(usize),
    /// One of the OLDER records an opened row folds (`members[n]`, never the
    /// first, which the row itself draws).
    Member(usize, usize),
}

/// One thing the cursor can rest on, on a task's page.
///
/// # Why a flat list and not three regions
///
/// Because a task's page is read top to bottom and acted on in place. Three
/// regions with `Tab` between them would be three cursors to keep in step and
/// a key that means "go somewhere else" on a page with nowhere else to go. A
/// flat list is one cursor, one `RET`, and one question for the footer — the
/// shape the notification view's `place_of` already has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    /// Whether the task runs on its own at all. `RET` switches it.
    Enabled,
    /// How often. `RET` walks the ladder.
    Interval,
    /// Which part of the day. `RET` walks the five.
    Window,
    /// When it next runs. Nothing to do; the row is the answer.
    Next,
    /// Run it now.
    RunNow,
    /// Rehearse it, changing nothing.
    Rehearse,
    /// One of the things its module offers on its page, by its index in
    /// `TaskState::actions`. `RET` asks for it.
    Action(usize),
    /// The title of a section of the module's page that folds, by its index
    /// in the page's sections. `RET` opens it or shuts it.
    Section(usize),
    /// One row of the module's own page, counted across its sections, shut
    /// ones too — the index an `act-row` names. `RET` does the row's action,
    /// when it has one.
    Screen(usize),
    /// One of its steps. `RET` flips it, unless the flake owns it.
    Step(usize),
    /// One of its last runs. Nothing to do; the footer says what happened.
    Ran(usize),
}

/// What the cursor is on, by what names it across two versions of the page —
/// the rule every list that reorders keeps, and this one reorders each time a
/// person acts on a row.
///
/// # Why the section is part of a row's name
///
/// Because a row of a module's page travels. Marked seen, an error leaves the
/// section it was in for another, with the same id: a cursor that followed it
/// would be thrown down the page by the press that was meant to clear the next
/// row. What it stays with is the place it was in, and the row after it comes
/// up to take it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Held {
    /// A spot the module's page has no hand in.
    Spot(Spot),
    /// One of the page's own actions, by its id: the module's switch reshapes
    /// the list it is counted in (what only a module switched off offers
    /// leaves it while it is on), and kept by its index the cursor would land
    /// on another action, which `RET` would then ask for.
    Action {
        /// Its id.
        id: String,
        /// How far down the page's actions it stood, for when it is gone.
        place: usize,
    },
    /// The title of a section, by the section.
    Title(String),
    /// A row of the module's page.
    Row {
        /// Its section.
        section: String,
        /// Which of the sections of that name: one with no id is named by
        /// its title, and a module may title several alike.
        twin: usize,
        /// Its id.
        id: String,
        /// How far down its section it stood.
        place: usize,
        /// Where its section began among the spots, for when it is gone.
        stood: usize,
    },
}

/// What names a section of a module's page: the id it folds by, or, for one
/// that declared none, its title.
fn section_name(section: &ModuleSection) -> &str {
    if section.id.is_empty() {
        &section.heading
    } else {
        &section.id
    }
}

/// Whether a section that folds is open: the person's word on it, or else how
/// its module ships it.
fn section_open(task: &TaskState, section: &ModuleSection) -> bool {
    task.folds
        .iter()
        .find(|f| f.id == section.id)
        .map_or(matches!(section.fold, SectionFold::Open), |f| f.open)
}

/// One section of a module's page as it is walked: what the list of spots
/// and the list of rows both read ([`State::shown_sections`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shown {
    /// Its index in the page's sections.
    section: usize,
    /// The index of its first row, counted across every section.
    ///
    /// SHUT ONES INCLUDED, and that is the trap: this is the number an
    /// `act-row` names, and the host counts the page it was sent. A count of
    /// what is drawn would make `RET` act on the wrong row of every section
    /// under a shut one.
    from: usize,
    /// How many rows it holds.
    rows: usize,
    /// Whether its title folds it.
    folds: bool,
    /// Whether its rows are drawn.
    open: bool,
}

/// Everything this buffer remembers between calls.
struct State {
    /// The last mind the host pushed. Everything drawn comes from here.
    mind: Mind,
    page: Page,
    pane: Pane,
    /// Which task the cursor is on, as an index into `mind.tasks`.
    task: usize,
    /// Which notice the cursor is on, as an index into `mind.notices` —
    /// moved with its notice when the list is ranked again (`received`).
    notice: usize,
    /// Whether a mind has arrived yet: the first one decides where the
    /// keyboard starts.
    heard: bool,
    /// The first notice that is drawn.
    notices_from: usize,
    /// Which LINE of the activity log the cursor is on — see [`Line`].
    log: usize,
    /// The first line of the log that is drawn.
    log_from: usize,
    /// The folded rows that are OPEN, each by what does not move when a newer
    /// record joins it: its task and the moment its OLDEST record began.
    opened: Vec<(String, u64)>,
    /// The module's own page, as the host last pushed it — only while a task's
    /// page is open, and only that task's.
    screen: Option<ModuleScreen>,
    /// The task whose page the host was last asked to follow
    /// (`request::watch-screen`), by id.
    ///
    /// Remembered so the ask goes out ONCE per page opened and once when it
    /// closes — a frame is not a reason to ask again, and the host re-asks by
    /// itself whenever the task's `screen-generation` moves.
    watched: Option<String>,
    /// The first WRAPPED row of the footer's sentence that is drawn.
    ///
    /// The host clamps it and hands it back (`Input::Scrolled`), so what is
    /// held here is only ever a number the host produced — this side has never
    /// seen the wrap. See [`ronoctua_protocol::Command::Paragraph`].
    foot_scroll: u16,
    /// What that offset belongs to, so it does not survive the cursor moving.
    foot_of: String,
    /// The first wrapped row of a task's DESCRIPTION that is drawn, on its
    /// page — the host's number, as [`State::foot_scroll`] is. It only moves
    /// in a window too small for the description to be given every row it
    /// wraps to ([`about_lines`]).
    about_scroll: u16,
    /// Whether the window still owes the cursor a move.
    ///
    /// # Why the window does not always follow
    ///
    /// Because the WHEEL scrolls the page and leaves the cursor where it is —
    /// which is what the owner asked for and what every window in the shell
    /// already does — and a window that followed unconditionally would put it
    /// back on the next frame. So following is what a KEY asks for: the
    /// arrows and the clicks set this, the wheel clears it, and the draw does
    /// what it says.
    ///
    /// It is the same division `Footer::settle_region` makes on the host's
    /// side, in one word: a daemon's arrival adjusts, a person's key follows.
    follows: bool,
    /// Which spot the cursor is on, on a task's page.
    spot: usize,
    /// The first row of a task's page that is drawn.
    body_from: usize,
    /// The window, as the last `render` saw it.
    lines: u16,
    columns: u16,
    /// WHETHER THE WINDOW WEARS ITS HEADER (`context.headed`). Without it —
    /// the distro's default — nothing on the screen names a task's page or
    /// leads back from it, so the page says both itself, and the home page
    /// says whose it is beside its first title.
    headed: bool,
    /// The words the host resolved for the keys this buffer declared.
    ///
    /// It holds them to MEASURE them, never to draw them: every phrase still
    /// goes out as a `Text::Key` and the host still translates it. What the
    /// text buys is the width — the header used to reserve a fixed thirteen
    /// cells for every phrase because it could not know how long one came out,
    /// and the owner read the result as shortcuts with enormous gaps between
    /// them.
    phrases: Phrases,
    /// The same for the keys of the module's page in front, which came with
    /// it (`module-screen::phrases`): this side cannot ask for them at load,
    /// because the set of modules is not known then. Empty is a host that
    /// said none of them, and the page is then laid out blind.
    page_phrases: Phrases,
    /// Which cell the pointer is resting on, if it is in the window.
    hover: ronoctua_design::Hovers,
    /// The daemon's own log, as the host last delivered it, oldest first.
    tail: Vec<LogLine>,
    /// Which line of it the cursor is on.
    tail_at: usize,
    /// The first line of it that is drawn.
    tail_from: usize,
    /// The blocks the last `render` divided the window into.
    ///
    /// Recorded rather than computed, for the reason the memory panel's own
    /// `list_top` gives: what is above a list grows with what the machine
    /// reported, and a fixed offset puts the cursor on a neighbour of the row
    /// that was clicked.
    ///
    /// They are what the pointer asks, and that is the half that was missing:
    /// a click used to be routed by `line >= log_at` with no floor under it,
    /// so a click in the FOOTER was attributed to the activity log whenever
    /// the log was long enough to reach that far.
    blocks: Blocks,
    /// WHERE THE FOOTER'S CLICK WAS DRAWN in the last frame, and whose page it
    /// opens — asked by the click and the pointer, so what they take is what
    /// the eye saw.
    open_at: Option<OpenAt>,
    /// WHERE THE WAY BACK WAS DRAWN atop a task's page in the last frame —
    /// its row inside the body's block and its cells — while the window
    /// wears no header.
    back_at: Option<(u16, u16, u16)>,
    /// WHERE A TASK'S DESCRIPTION WAS DRAWN in the last frame — its first row
    /// inside the body's block, and the cell the host answers its bar from —
    /// so a drag or a wheel on it scrolls the paragraph, never the page.
    about_at: Option<(u16, u16)>,
    /// WHERE THE LINES UNDER THE MODULE'S ROWS WERE DRAWN in the last frame —
    /// the first row inside the body's block, the cell the host answers a bar
    /// from, and the row's id — for the same reason.
    below_at: Vec<(u16, u16, String)>,
    /// How far the lines under one row are scrolled: its id and the first
    /// wrapped row, the host's number as [`State::foot_scroll`] is. Only a
    /// text past [`BELOW_MOST`] has a bar at all.
    below_scroll: (String, u16),
    /// WHERE THE PAGE'S ROWS STOPPED in the last frame, inside the body's
    /// block, when the window cut one: a row that does not fit is drawn as its
    /// first line alone, and the lines under that line are nobody's — the
    /// pointer lights none of them, says nothing there and takes nothing.
    page_cut: Option<u16>,
    /// WHERE THE FOOTER DREW WHAT A ROW SAYS UNDER ITSELF in the last frame —
    /// its first row inside the footer's block and the cell the host answers
    /// its bar from — and how far it is scrolled. A paragraph of its own
    /// beside the row's last fact, so a bar of its own: moving one never moves
    /// the other.
    foot_below_at: Option<(u16, u16)>,
    foot_below_scroll: u16,
    /// WHERE THE FOOTER DREW THE ROW'S OTHER ACTION in the last frame — its
    /// row inside the footer's block and its cells.
    also_at: Option<(u16, u16, u16)>,
    /// WHERE THE PAGE'S OWN NAME WAS DRAWN atop the list of tasks in the last
    /// frame — its first cell — while the window wears no header.
    title_from: Option<u16>,
    /// WHAT THE FOOTER DREW in the last frame, piece by piece, with what each
    /// piece says under the pointer — and whether it drew anything at all,
    /// for the rest of it.
    foot_spots: Vec<FootSpot>,
    foot_drawn: bool,
    /// WHERE EACH SWATCH OF THE LEGEND WAS DRAWN in the last frame, on its row
    /// inside the body's block, with what it says under the pointer: only
    /// the ones that fitted are here.
    legend_at: Vec<FootSpot>,
    /// THE COLUMNS THE LAST FRAME LAID the list of tasks and the activity
    /// log out in — what the pointer is asked against ([`State::pointed`]),
    /// so a glyph is explained where it was drawn.
    task_layout: Layout,
    log_layout: Layout,
}

/// A piece of a block as the last frame drew it — its row inside the block,
/// its cells — and the sentence it says under the pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FootSpot {
    row: u16,
    from: u16,
    to: u16,
    about: String,
}

/// The footer's click that opens a task's page, as the last frame drew it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenAt {
    /// Its row, inside the footer's block.
    row: u16,
    /// Its first cell, and the cell after its last.
    from: u16,
    to: u16,
    /// The task it opens, by its index in `mind.tasks`.
    task: usize,
}

/// Where each block of the screen was, in the last frame drawn.
///
/// One value and not five loose `Option<u16>`, because the five are one fact:
/// how the window was divided. Split apart they were five things to keep in
/// step, and the pointer only ever consulted two of them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Blocks {
    warn: Region,
    notices: Region,
    body: Region,
    log: Region,
    foot: Region,
}

impl Blocks {
    /// Nothing drawn yet. Every block empty, so the pointer reaches none of
    /// them — which is the right answer before the first frame.
    const fn new() -> Self {
        let none = Region::new(0, 0);
        Self {
            warn: none,
            notices: none,
            body: none,
            log: none,
            foot: none,
        }
    }
}

impl State {
    fn fresh() -> Self {
        Self {
            mind: empty_mind(),
            page: Page::Home,
            pane: Pane::Tasks,
            task: 0,
            notice: 0,
            heard: false,
            notices_from: 0,
            log: 0,
            log_from: 0,
            opened: Vec::new(),
            screen: None,
            watched: None,
            foot_scroll: 0,
            foot_of: String::new(),
            about_scroll: 0,
            follows: true,
            spot: 0,
            body_from: 0,
            lines: 24,
            columns: 80,
            headed: true,
            phrases: Phrases::empty(),
            page_phrases: Phrases::empty(),
            hover: ronoctua_design::Hovers::new(),
            tail: Vec::new(),
            tail_at: 0,
            tail_from: 0,
            blocks: Blocks::new(),
            open_at: None,
            back_at: None,
            about_at: None,
            below_at: Vec::new(),
            below_scroll: (String::new(), 0),
            page_cut: None,
            foot_below_at: None,
            foot_below_scroll: 0,
            also_at: None,
            title_from: None,
            foot_spots: Vec::new(),
            foot_drawn: false,
            legend_at: Vec::new(),
            task_layout: Layout::default(),
            log_layout: Layout::default(),
        }
    }

    /// The task under the cursor.
    fn here(&self) -> Option<&TaskState> {
        self.mind.tasks.get(self.task)
    }
}

/// What names a notice across minds: its module and its id inside it.
fn notice_key(n: &Notice) -> (String, String) {
    (n.module.clone(), n.id.clone())
}

/// Where a cursor that was at `at`, on the item named `on`, belongs in a list
/// whose items are now named `now`: on that item wherever it went, or — when
/// it is gone — at its row, kept inside the list.
fn kept<K: PartialEq>(at: usize, on: Option<&K>, now: &[K]) -> usize {
    on.and_then(|k| now.iter().position(|n| n == k))
        .unwrap_or_else(|| at.min(now.len().saturating_sub(1)))
}

/// A mind with nothing in it — what is drawn before the first one arrives.
const fn empty_mind() -> Mind {
    Mind {
        now: 0,
        pulse: ronoctua_design::types::Pulse {
            at: 0,
            signs: Vec::new(),
        },
        tasks: Vec::new(),
        queue: Vec::new(),
        running: String::new(),
        rehearsing: String::new(),
        announcement: ronoctua_design::types::Announcement {
            task: String::new(),
            total_seconds: 0,
            deadline: 0,
            why: String::new(),
        },
        gate: ronoctua_design::types::Gate {
            ram_ceiling_pct: 0,
            cpu_stall_per_mille: 0,
            abort_on_battery: false,
        },
        cadence: Vec::new(),
        activity_rows: Vec::new(),
        activity_reach: String::new(),
        history_reach: String::new(),
        notices: Vec::new(),
    }
}

thread_local! {
    /// NOT `const {}` any more: the footer's sentence remembers WHAT it is
    /// scrolled through (`State::foot_of`), and a `String` cannot be built at
    /// compile time. The first borrow pays a lazy-init check, once.
    static STATE: RefCell<State> = RefCell::new(State::fresh());
}

// ---------------------------------------------------------------------------
// The vocabulary: one word per case, and the host turns it into a sentence.
// ---------------------------------------------------------------------------

/// The word a hold is looked up by, as `brain-hold-<word>`.
const fn hold_word(h: HoldKind) -> Option<&'static str> {
    match h {
        HoldKind::None => None,
        HoldKind::Ram => Some("ram"),
        HoldKind::Cpu => Some("cpu"),
        HoldKind::Battery => Some("battery"),
        HoldKind::Busy => Some("busy"),
        HoldKind::Network => Some("network"),
        HoldKind::Asleep => Some("asleep"),
    }
}

/// The word a standing is looked up by, as `brain-standing-<word>`.
const fn standing_word(s: Standing) -> &'static str {
    match s {
        Standing::Idle => "idle",
        Standing::Queued => "queued",
        Standing::Running => "running",
    }
}

/// The word an outcome is looked up by, as `brain-outcome-<word>`.
const fn outcome_word(o: OutcomeKind) -> &'static str {
    match o {
        OutcomeKind::Skipped => "skipped",
        OutcomeKind::Done => "done",
        OutcomeKind::Failed => "failed",
        OutcomeKind::Held => "held",
        // SOMETHING A MODULE NOTICED — not a run: what it says is the
        // record's `says-text`, and this word is the column's.
        OutcomeKind::Noted => "noted",
    }
}

/// The key of an outcome's word.
fn outcome_key(o: OutcomeKind) -> String {
    format!("brain-outcome-{}", outcome_word(o))
}

/// What the footer says above a record of a task's own history: the question
/// the sentence under it answers — *why it failed*, *why it waited* — and
/// never the outcome's word, which the row it is about already says. The
/// owner, of a footer that read `Failed` over `Failed`: *"o título e conteúdo
/// do histórico são apenas uma repetição do status, não servindo para nada"*.
const fn record_heading_key(o: OutcomeKind) -> &'static str {
    match o {
        OutcomeKind::Done => key::RECORD_DONE,
        OutcomeKind::Failed => key::RECORD_FAILED,
        OutcomeKind::Held => key::RECORD_HELD,
        OutcomeKind::Skipped => key::RECORD_SKIPPED,
        OutcomeKind::Noted => key::RECORD_NOTED,
    }
}

/// THE TONE OF A RUN'S OUTCOME — discreet, in the hue of the news (the
/// owner's ask: *"o worked … um tom positivo, como é a cor verde"*). One table
/// for the list's last column, a task's history and the activity log, so a
/// word wears one colour wherever it is read.
const fn outcome_role(o: OutcomeKind) -> Role {
    match o {
        OutcomeKind::Done => Role::StatusGood,
        OutcomeKind::Failed => Role::StatusBad,
        OutcomeKind::Held => Role::StatusHeld,
        OutcomeKind::Skipped => Role::StatusSkipped,
        // WHAT A WATCH WROTE DOWN: the teal of what the brain follows.
        OutcomeKind::Noted => Role::StatusBusy,
    }
}

/// And of where a task stands now: a turn waited for, or a place in the
/// queue, is news with little meaning (*"o waiting … um tom com menos
/// significado, como é a cor roxa"*); a run under way is the teal of what is
/// being followed.
const fn standing_role(s: Standing) -> Role {
    match s {
        Standing::Idle | Standing::Queued => Role::StatusIdle,
        Standing::Running => Role::StatusBusy,
    }
}

/// What a row's status says under the pointer, when it is how the last run
/// went — on the date that run began.
const fn last_about(o: OutcomeKind) -> &'static str {
    match o {
        OutcomeKind::Done => key::ABOUT_DONE,
        OutcomeKind::Failed => key::ABOUT_FAILED,
        OutcomeKind::Held => key::ABOUT_HELD,
        OutcomeKind::Skipped => key::ABOUT_SKIPPED,
        OutcomeKind::Noted => key::ABOUT_NOTED,
    }
}

/// And when it is where the task stands, having not run yet.
const fn standing_about(s: Standing) -> &'static str {
    match s {
        Standing::Idle => key::ABOUT_IDLE,
        Standing::Queued => key::ABOUT_QUEUED,
        Standing::Running => key::ABOUT_RUNNING,
    }
}

/// And when it is what is in the task's way: what it waits for.
const fn hold_about(h: HoldKind) -> Option<&'static str> {
    match h {
        HoldKind::None => None,
        HoldKind::Ram => Some(key::ABOUT_HOLD_RAM),
        HoldKind::Cpu => Some(key::ABOUT_HOLD_CPU),
        HoldKind::Battery => Some(key::ABOUT_HOLD_BATTERY),
        HoldKind::Busy => Some(key::ABOUT_HOLD_BUSY),
        HoldKind::Network => Some(key::ABOUT_HOLD_NETWORK),
        HoldKind::Asleep => Some(key::ABOUT_HOLD_ASLEEP),
    }
}

/// What a task's glyph means, under the pointer.
const fn kind_about(k: TaskKind) -> &'static str {
    match k {
        TaskKind::Routine => key::KIND_ROUTINE,
        TaskKind::Once => key::KIND_ONCE,
        TaskKind::Watch => key::KIND_WATCH,
    }
}

/// The tone a notice is drawn in, by how loud it is: the two that cost
/// something now in the one colour that alarms, a warning in the warning's,
/// and news in the body's.
const fn level_tone(l: NoticeLevel) -> Role {
    match l {
        NoticeLevel::Alarm | NoticeLevel::Danger => Role::Danger,
        NoticeLevel::Warn => Role::Warning,
        NoticeLevel::Info => Role::Body,
    }
}

/// The tone of a module's row, by MEANING — one role each, and `watching` is
/// the gestures' glyph colour, the one the modeline marks a watch with (the
/// journal's errors wear it); `working` is the teal of a thing under way, and
/// `minor` the row's own quiet, the bottom of a list sorted by gravity.
const fn tone_role(t: BrainTone) -> Role {
    match t {
        BrainTone::Quiet => Role::Quiet,
        BrainTone::Watching => Role::Watching,
        BrainTone::Working => Role::Following,
        BrainTone::Warning => Role::Warning,
        BrainTone::Danger => Role::Danger,
        BrainTone::Critical => Role::Critical,
        BrainTone::Minor => Role::Quiet,
        BrainTone::Accent => Role::Accent,
    }
}

/// What the rest of a date takes when it is a row's value: the dim of every
/// date, and for a row its module gave a tone, that tone. It is the only
/// colour such a row has — a refused sudo among used ones, a reminder ringing
/// — so the page does not spend it on looking like the log. The day stays the
/// day's ([`put_stamp`]).
const fn dated_value_role(tone: BrainTone) -> Role {
    match tone {
        BrainTone::Quiet => Role::Dim,
        toned => tone_role(toned),
    }
}

/// A cell of a module's row, as a run — or nothing, for an empty one.
///
/// A key is drawn through the host; everything else is DATA the host already
/// formatted (the moment's text, the size's text), or a count, which is a
/// number and is not translated.
fn cell_run(c: &ScreenCell) -> Option<Run<Icon>> {
    match c {
        ScreenCell::Empty => None,
        ScreenCell::Key(k) if k.is_empty() => None,
        ScreenCell::Key(k) => Some(Run::label(k)),
        ScreenCell::Data(d) => Some(Run::datum(d)),
        ScreenCell::Moment(t) => Some(Run::datum(&t.text)),
        ScreenCell::Count(n) => Some(Run::datum(&n.to_string())),
        ScreenCell::Bytes(m) => Some(Run::datum(&m.text)),
    }
}

/// The moment in a cell of a module's row, when the host cut it where its day
/// is: it goes through [`put_stamp`], like every other date of this buffer,
/// and not through [`cell_run`] as one run in one tone.
const fn stamped(c: &ScreenCell) -> Option<&Timed> {
    match c {
        ScreenCell::Moment(t) if !t.stamp.day.is_empty() => Some(t),
        _ => None,
    }
}

/// The key a `Some` stands for, when it is one — an empty key is a key the
/// host did not have, and drawing it would put empty brackets on the screen.
fn key_of(k: &str) -> Option<String> {
    (!k.is_empty()).then(|| k.to_owned())
}

/// The word the core gives a module the PERSON switched off (`brain-why-off`).
const OFF_BY_YOU: &str = "off";

/// The key of the reason a module is off when the reason is not the person's
/// switch — the system's configuration turned it off — or `None`.
///
/// Told apart by the WORD and drawn by the KEY the host sent beside it, so a
/// new reason the core learns is drawn without this side naming it.
fn off_not_by_you(task: &TaskState) -> Option<String> {
    if task.enabled || task.why_not.is_empty() || task.why_not == OFF_BY_YOU {
        return None;
    }
    key_of(&task.why_key)
}

/// Whether the system's configuration switched this module off: then the
/// brain refuses every change and every run, so the page offers none of them
/// — neither as a row with a key nor as a rung of the header — and the home
/// row says the reason instead of a standing.
fn off_by_the_system(task: &TaskState) -> bool {
    off_not_by_you(task).is_some()
}

/// The steps a page shows: none for a module the system switched off, whose
/// steps cannot be flipped and will not run.
fn steps_shown(task: &TaskState) -> usize {
    if off_by_the_system(task) {
        0
    } else {
        task.steps.len()
    }
}

/// The word a part of the day is looked up by, as `brain-part-<word>`.
const fn part_word(p: ronoctua_design::types::DayPart) -> &'static str {
    use ronoctua_design::types::DayPart;
    match p {
        DayPart::Any => "any",
        DayPart::Night => "night",
        DayPart::Morning => "morning",
        DayPart::Afternoon => "afternoon",
        DayPart::Evening => "evening",
    }
}

/// The next part of the day, walking the five in the order of a day.
const fn next_part(p: ronoctua_design::types::DayPart) -> ronoctua_design::types::DayPart {
    use ronoctua_design::types::DayPart;
    match p {
        DayPart::Night => DayPart::Morning,
        DayPart::Morning => DayPart::Afternoon,
        DayPart::Afternoon => DayPart::Evening,
        DayPart::Evening => DayPart::Any,
        DayPart::Any => DayPart::Night,
    }
}

/// The glyph that says whether a task repeats, happens once, or only listens.
const fn kind_glyph(k: TaskKind) -> Icon {
    match k {
        TaskKind::Routine => Icon::TaskRoutine,
        TaskKind::Once => Icon::TaskOnce,
        // AN EYE: it runs nothing on a schedule, it follows something.
        TaskKind::Watch => Icon::TaskWatch,
    }
}

/// The colour a task's own rows are drawn in.
///
/// A routine keeps the ordinary ramp; a one-shot does not, and neither does a
/// watch — which wears the ordinary row's colour turned a little toward the
/// teal of a thing under way and well ([`Role::Listening`]): never the
/// alarm's colour the owner first read it in, and no longer the teal itself,
/// which read too loud among the names. The owner asked for both a glyph and
/// a colour, because a glyph two cells wide is a small thing to hang a
/// distinction on.
const fn kind_role(k: TaskKind) -> Role {
    match k {
        TaskKind::Routine => Role::Body,
        TaskKind::Once => Role::Once,
        TaskKind::Watch => Role::Listening,
    }
}

/// What the row of a task's switch is called. A watch runs nothing, so it does
/// not say it runs: `kind_about` draws the same line between the kinds.
const fn enabled_key(k: TaskKind) -> &'static str {
    match k {
        TaskKind::Routine | TaskKind::Once => key::ENABLED,
        TaskKind::Watch => key::ENABLED_WATCH,
    }
}

/// What that row answers while the person has it off: a watch has no run to
/// ask for by hand, so "only when you ask" would promise one.
const fn off_key(k: TaskKind) -> &'static str {
    match k {
        TaskKind::Routine | TaskKind::Once => key::OFF,
        TaskKind::Watch => key::OFF_WATCH,
    }
}

/// And what its switch is for, in the words of each kind.
const fn enabled_hint_key(k: TaskKind) -> &'static str {
    match k {
        TaskKind::Routine | TaskKind::Once => key::ENABLED_HINT,
        TaskKind::Watch => key::ENABLED_HINT_WATCH,
    }
}

/// Seconds as `mm:ss`.
///
/// The only duration this screen formats. The machine's own readings used to
/// be formatted here too — `Memory 31%`, `Processor 4.0%`, under every task's
/// name on every frame — and the owner said they should not be shown at all.
/// What is left is the countdown, which is about the thing in focus and is the
/// reason anybody is looking.
fn clock(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

// ---------------------------------------------------------------------------
// The three fixed regions: the header, the warning and the footer.
// ---------------------------------------------------------------------------

/// The most rows the footer may take.
///
/// Six, and it is a HEIGHT and not a ceiling: the footer is always this tall.
///
/// # Why it stopped being a ceiling
///
/// Because the owner asked for it in those words — *"quero que deixe o footer
/// do brain com um tamanho sempre fixo e não variável como ocorre agora"* —
/// and the reason is on the screen: a footer that grows and shrinks with the
/// cursor moves everything above it every time the cursor moves, which is a
/// page that will not sit still while it is being read. The argument written
/// here before ("a fixed six would spend a third of a short window on blank
/// ground") is answered by the `min(left)` at the call site: where there is
/// not room for six, the footer takes what there is.
///
/// And six is what the contents need: a name, three rows of a sentence — the
/// description, or the machine's own error — a rule, and the values. The
/// arrangement inside is fixed too; see [`State::foot`].
const FOOT_LINES: u16 = 6;

/// Every outcome a record can have, for the phrases measured below.
const OUTCOMES: [OutcomeKind; 5] = [
    OutcomeKind::Skipped,
    OutcomeKind::Done,
    OutcomeKind::Failed,
    OutcomeKind::Held,
    OutcomeKind::Noted,
];

/// Every standing a task can be in.
const STANDINGS: [Standing; 3] = [Standing::Idle, Standing::Queued, Standing::Running];

/// Every hold that has a word.
const HOLDS: [HoldKind; 6] = [
    HoldKind::Ram,
    HoldKind::Cpu,
    HoldKind::Battery,
    HoldKind::Busy,
    HoldKind::Network,
    HoldKind::Asleep,
];

/// Every part of the day.
const PARTS: [ronoctua_design::types::DayPart; 5] = [
    ronoctua_design::types::DayPart::Any,
    ronoctua_design::types::DayPart::Night,
    ronoctua_design::types::DayPart::Morning,
    ronoctua_design::types::DayPart::Afternoon,
    ronoctua_design::types::DayPart::Evening,
];

/// Every phrase this buffer MEASURES, for the descriptor to ask the host for:
/// the footer's fact labels and its click, the words of the list's two last
/// columns and of a page's schedule — and how `LEADER Alt` is spelt on this
/// machine, which a row's `⌥` turns into under the pointer
/// ([`ronoctua_design::Phrases::LEADER_ALT`]). What the header teaches is the
/// host's to draw and measure (`Screen::teach`).
///
/// # Why the columns are measured
///
/// Because they were RESERVED, each at the longest translation, and the owner
/// read the result off the screen: *"os espaços estão sendo mal utilizados"* —
/// `After an account…` cut in a column of seventeen, beside a name column
/// with forty cells of nothing in it. A column as wide as what it holds today,
/// in the person's language, gives the room back to the one that needs it.
fn every_phrase() -> Vec<String> {
    let mut out: Vec<String> = [
        key::LAST_RUN,
        key::AT_STEP,
        key::NEEDS_ROOT,
        key::FLAKE_OWNED,
        key::CHANGED,
        key::FIRST,
        key::TIMES,
        key::SINCE,
        key::OPEN_PAGE,
        key::BACK,
        key::TITLE,
        key::WATCHING,
        key::NEVER,
        key::ENABLED,
        key::ENABLED_WATCH,
        key::ENABLED_HINT,
        key::ENABLED_HINT_WATCH,
        key::ON,
        key::OFF,
        key::OFF_WATCH,
        key::EVERY,
        key::PART,
        key::NEXT,
        key::SOON,
        key::RUN_NOW,
        key::REHEARSE,
        key::REHEARSAL,
        ronoctua_design::Phrases::LEADER_ALT,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    out.extend(OUTCOMES.iter().map(|o| outcome_key(*o)));
    out.extend(STANDINGS.iter().map(|s| standing_key(*s)));
    out.extend(HOLDS.iter().filter_map(|h| hold_key(*h)));
    out.extend(PARTS.iter().map(|p| part_key(*p)));
    out
}

/// The key of a standing's word.
fn standing_key(s: Standing) -> String {
    format!("brain-standing-{}", standing_word(s))
}

/// The key of a hold's word, when it has one.
fn hold_key(h: HoldKind) -> Option<String> {
    hold_word(h).map(|w| format!("brain-hold-{w}"))
}

/// The key of a part of the day's words.
fn part_key(p: ronoctua_design::types::DayPart) -> String {
    format!("brain-part-{}", part_word(p))
}

impl State {
    /// What the page in front teaches.
    ///
    /// # Why two of them are conditional
    ///
    /// Because `, a` and `, p` answer a COUNTDOWN — stop it, or start now —
    /// and there is not always one. Taught unconditionally they would be two
    /// rungs of the ladder that do nothing most of the time, and the ladder is
    /// cut at the width of the line: two dead rungs push two live ones off the
    /// end.
    ///
    /// They have to be taught SOMEWHERE, and this is now the only place. They
    /// used to be `LEADER b a` and `LEADER b p`, registered beside the band
    /// that draws them — and `LEADER b` became the brain itself, which the
    /// keymap refuses to have as a command and a prefix at once. The band
    /// keeps its two buttons for the mouse and now prints THESE keys.
    fn teaches(&self) -> Vec<Teach> {
        let base: &[Teach] = match self.page {
            Page::Home => HOME_TEACHES,
            Page::Task => TASK_TEACHES,
            Page::Log => LOG_TEACHES,
        };
        // WHAT THE TASK IN FRONT CANNOT DO IS NOT TAUGHT: a module that only
        // listens has no run to start or rehearse, and one that keeps no
        // calendar has no cadence and no window to step — a rung that does
        // nothing pushes a live one off the end of the line.
        let here = self.here();
        // A MODULE THE SYSTEM SWITCHED OFF answers none of the task's own
        // keys — the brain refuses each of them — so none is taught: what is
        // left is the way back and the keys' list. `RET` too, on its page,
        // where it changes the row under the cursor; on the home page it
        // opens that page, which still works.
        let refused = here.is_some_and(off_by_the_system);
        let changes = self.page == Page::Task;
        let mut out: Vec<Teach> = base
            .iter()
            .copied()
            .filter(|(keys, _)| {
                !refused
                    || !(matches!(*keys, ", r" | ", e" | ", o" | ", i" | ", w" | ", d")
                        || (changes && *keys == "RET"))
            })
            .filter(|(keys, _)| match *keys {
                ", r" | ", e" => here.is_none_or(|t| t.kind != TaskKind::Watch),
                ", i" | ", w" => here.is_none_or(|t| t.calendar && t.kind != TaskKind::Watch),
                // AND A TASK WITH NO STEPS has none to put back.
                ", d" => here.is_none_or(|t| !t.steps.is_empty()),
                _ => true,
            })
            .collect();
        if self.counting_down() {
            out.extend_from_slice(ANSWER_TEACHES);
        }
        // THE ROW'S OTHER ACTION, right after the press it sits beside, and
        // only while the row under the cursor has one: a rung that does
        // nothing pushes a live one off the end.
        if self.page == Page::Task && self.also_here().is_some() {
            let at = out
                .iter()
                .position(|(k, _)| *k == "ESC")
                .map_or(0, |n| n + 1);
            out.splice(at..at, ALSO_TEACHES.iter().copied());
        }
        // EACH OF THE NOTICE'S THREE only while the notice in front offers
        // it: a rung that does nothing pushes a live one off the end.
        if let Some(n) = self.notice_here() {
            for teach in NOTICE_TEACHES {
                let offered = match teach.1 {
                    "cmd-brain-act" => !n.controls.is_empty(),
                    "cmd-brain-dismiss" => n.dismissable,
                    _ => !n.snooze.is_empty(),
                };
                if offered {
                    out.push(*teach);
                }
            }
        }
        out
    }

    /// The notice the keyboard's three notice keys act on, by its index in
    /// `mind.notices`: the one under the cursor on the home page, the first of
    /// the open task's module on its page, and otherwise the first — the one
    /// the strip under the minibuffer shows.
    fn notice_target(&self) -> Option<usize> {
        let notices = &self.mind.notices;
        if notices.is_empty() {
            return None;
        }
        match self.page {
            Page::Home if self.pane == Pane::Notices => {
                Some(self.notice.min(notices.len().saturating_sub(1)))
            }
            Page::Task => Some(
                self.here()
                    .and_then(|t| notices.iter().position(|n| n.module == t.id))
                    .unwrap_or(0),
            ),
            _ => Some(0),
        }
    }

    /// The notice those keys act on.
    fn notice_here(&self) -> Option<&Notice> {
        self.notice_target().and_then(|n| self.mind.notices.get(n))
    }

    /// Whether a countdown is running for anything.
    fn counting_down(&self) -> bool {
        !self.mind.announcement.task.is_empty() && self.mind.announcement.deadline > self.mind.now
    }

    /// THE WINDOW'S TITLE AND WHAT THE PAGE TEACHES, declared for the header
    /// the HOST draws over every window (`Screen::heading`, `Screen::teach`)
    /// — its glyph, its name and [`State::teaches`], in that order. The host
    /// draws, lights, explains and clicks the pairs with the one machinery
    /// every window has: a click on `⌥r` is the command, a click on `RET` is
    /// the key, and a pair that is not a control is drawn inert.
    fn declare(&self, screen: &mut Screen<Wit>) {
        let title = match self.page {
            Page::Home => key::TITLE.to_owned(),
            Page::Log => key::LOG.to_owned(),
            Page::Task => self
                .here()
                .and_then(|t| key_of(&t.name_key))
                .unwrap_or_else(|| key::TITLE.to_owned()),
        };
        let glyph = match self.page {
            Page::Home | Page::Log => Icon::Brain,
            Page::Task => self.here().map_or(Icon::Brain, |t| kind_glyph(t.kind)),
        };
        let _ = screen.heading(glyph, &title);
        for (keys, says) in self.teaches() {
            screen.teach(keys, says, about_of(says), is_a_control(keys));
        }
    }

    /// Every row of a block a pointer is on, with WHOSE hand it is — one
    /// cursor per seat rests on this buffer (`Hovers`).
    fn hovered_rows(&self, region: Region) -> Vec<(u16, u16)> {
        self.hover
            .cells()
            .filter_map(|(seat, (_, line))| region.row_at(line).map(|row| (row, seat)))
            .collect()
    }

    /// THE WARNING: one line, in the one colour that alarms, and ONLY when
    /// there is something to say.
    ///
    /// # Why there is no heading over it and no line when the machine is well
    ///
    /// Because it was a section — a title, two percentages and a word — and the
    /// owner read it and said what is wrong with that: a status with no subject
    /// is not a status. What a person wants to know is *why has nothing
    /// happened*, and on a machine where everything is fine the answer is
    /// nothing at all. A line reserved for it would be a line saying "no news"
    /// for ever.
    ///
    /// Returns how many rows it took: one, or none.
    /// Whether there is anything in the way worth a line of its own.
    ///
    /// Asked BEFORE the window is divided, because the warning's block has to
    /// be a line or nothing and the body's height is what is left over. The
    /// drawing below answers the same question again from the same two fields
    /// — `warning` draws whichever of them is true, and this only counts.
    fn warned(&self) -> bool {
        self.mind.tasks.iter().any(|t| hold_word(t.hold).is_some())
    }

    fn warning(&self, screen: &mut Screen<Wit>) {
        let mut pen = screen.region(self.blocks.warn);
        if pen.empty() {
            return;
        }
        self.warned_into(&mut pen);
    }

    fn warned_into(&self, pen: &mut Pen<'_, Wit>) {
        // WHAT A MODULE WANTS SAID IS NOT HERE any more: every notice has a
        // block of its own on the home page, with its controls, and the strip
        // under the minibuffer says the loudest of them everywhere else. This
        // line is left to the one question a notice does not answer — why has
        // nothing happened.
        let held = self.mind.tasks.iter().find_map(|t| hold_word(t.hold));
        let say = match held {
            Some("ram") => key::HELD_RAM,
            Some("cpu") => key::HELD_CPU,
            Some("battery") => key::HELD_BATTERY,
            Some(_) => key::HELD_OTHER,
            None => return,
        };
        pen.put(0, Run::label(say).playing(Role::Danger));
    }
}

/// What the footer says about the thing under the cursor.
///
/// Built before it is drawn, because the footer's HEIGHT is how much of this
/// there is: a footer that asked the page how tall to be after it had been
/// placed would be the layout loop `Fold::held` describes.
#[derive(Default)]
struct Told {
    /// The name of the thing, as a key.
    name: Option<String>,
    /// What the name says under the pointer, as a key: a task's one line,
    /// or what a record's heading answers. The footer's own sentence when
    /// there is none.
    name_about: Option<String>,
    /// What the held line says under the pointer, when it is one thing —
    /// a step's command.
    said_about: Option<&'static str>,
    /// The step a record stopped at, by its name's key: said on the line
    /// under the name, after its label, where `said` would go.
    at_step: Option<String>,
    /// One line of text the buffer is holding — a command, a path, a number.
    ///
    /// Its own line and not a fact, because what goes here is the longest
    /// thing on the screen and a column would cut it exactly where it stops
    /// being useful.
    said: Option<String>,
    /// A whole sentence about it, wrapped by the host.
    about: Option<String>,
    /// A sentence the machine produced, drawn as it came.
    trouble: Option<String>,
    /// A line of the daemon's log, whole, in its level's tone — data, drawn
    /// as it came, and wrapped where the row above had to cut it.
    logged: Option<(String, Role)>,
    /// A line of label-and-value pairs.
    facts: Vec<(String, String)>,
    /// The task whose page the click at the end of the last row opens, by
    /// its index in `mind.tasks` — the task this footer is about, when it is
    /// about one and that page is not the page in front.
    opens: Option<usize>,
    /// The name of the thing when it is data and no key: a row of a module's
    /// page, named by its own words.
    headline: Option<String>,
    /// What a row of a module's page says under itself.
    below: Option<ScreenCell>,
    /// What its module says of it under the cursor, a row each, in the tones
    /// the module gave. With any, there is no sentence: the last takes the
    /// rows left ([`State::foot_lines`]).
    lines: Vec<RowFact>,
    /// The key of what its other action is called on this row.
    also: Option<String>,
}

impl Told {
    /// WHAT THIS FOOTER IS ABOUT, as one string.
    ///
    /// Not for the screen: for the SCROLL. The sentence keeps an offset and
    /// the offset belongs to the sentence — left alone it would carry over to
    /// the next thing the cursor lands on, and somebody would step onto a
    /// short message already scrolled past its own end.
    fn subject(&self) -> String {
        // JOINED BY A SEPARATOR NOTHING CAN CONTAIN, so two different footers
        // cannot spell the same subject between them.
        let part = |it: &Option<String>| it.clone().unwrap_or_default();
        let lines: Vec<String> = self
            .lines
            .iter()
            .map(|l| format!("{} {}", l.label, cell_word(&l.value)))
            .collect();
        [
            part(&self.name),
            part(&self.headline),
            self.below.as_ref().map(cell_word).unwrap_or_default(),
            lines.join("\u{1e}"),
            part(&self.also),
            part(&self.at_step),
            part(&self.said),
            part(&self.about),
            part(&self.trouble),
            self.logged
                .as_ref()
                .map(|(t, _)| t.clone())
                .unwrap_or_default(),
        ]
        .join("\u{1f}")
    }
}

impl State {
    /// THE FOOTER: its own ground, at the bottom, about the thing in focus.
    ///
    /// # Why it is here and not under the cursor
    ///
    /// Because it was under the cursor — a dim line following the selection —
    /// and it could hold one row of a description written to be three. Worse,
    /// it moved: every row below it shifted when the cursor did, which is a
    /// page that will not sit still while it is being read.
    ///
    /// At the bottom it is a place, and a place can be as tall as it needs.
    fn foot(&mut self, screen: &mut Screen<Wit>, told: &Told) {
        self.foot_spots.clear();
        self.foot_drawn = false;
        self.also_at = None;
        self.foot_below_at = None;
        // THE OFFSET BELONGS TO THE SENTENCE, and this is where it is
        // forgotten: the cursor moving to another row is another sentence, and
        // one left scrolled would open past its own end.
        let of = told.subject();
        if self.foot_of != of {
            self.foot_of = of;
            self.foot_scroll = 0;
            self.foot_below_scroll = 0;
        }
        let mut pen = screen.region(self.blocks.foot);
        if pen.empty() {
            return;
        }
        // THE CHROME'S OWN GROUND, one rung above the activity container and
        // two above the page. The header wears the same one: they are the same
        // kind of thing — the frame around what is being read — and a footer
        // in the container's tone read, on the owner's screen, as more
        // container.
        pen.fill(Role::Header);
        let last = pen.lines();
        let wide = pen.columns();

        // THE ARRANGEMENT IS FIXED, and this is it:
        //
        //   0        the name of the thing
        //   1        the line this side is holding — a command, a path — when
        //            there is one, and the sentence starts here when there is not
        //   ..       what a module's row says under itself, when it says
        //            anything: two rows at most
        //   ..last-1 the SENTENCE, taking everything that is left
        //   last-1   the values, when there are any, and a click at its end:
        //            the page a task opens, or the row's other action
        //
        // Six rows, always [`FOOT_LINES`], and every row means the same thing
        // every time. Three things the owner reported come out of exactly this.
        //
        // THE ERROR CAME BACK. It was drawn LAST, after the values, and the
        // height was a sum of the parts capped at six — the sum came to seven
        // with a description, a rule, a value line and an error, so the error
        // was the part that fell off the end. *"Você removeu as mensagens
        // completas de erros que apareciam no footer ao selecionar um item do
        // 'Activity'"*. It is a PARAGRAPH, because a machine's error is a
        // sentence and one row of it is a sentence somebody cannot read.
        //
        // AND THE BLANK AFTER THE VALUES IS GONE. The values were drawn
        // wherever the running sum had got to, so a footer with no `said` drew
        // them on row four and left row five empty. They are the LAST row now,
        // counted from the bottom.
        //
        // AND THE RULE IS GONE, at the owner's word. It separated the words
        // from the figures, and it cost the sentence a row it needed: the
        // sentence was cut with two blank rows under it — *"por ter espaço
        // vazio abaixo do texto, então o texto DEVE ocupar todo o espaço
        // disponível"*. It also could not decide whether to appear: drawn on
        // the block's LAST row it takes the window's bottom edge (the residue
        // clause in `paint::Rows::bottom`), so at one height it was a line
        // against the border with nothing under it and at another it was not
        // there at all. *"Para facilitar e simplificar o visual, pode remover
        // a linha por completo."*
        //
        // A ROW OF A MODULE'S PAGE THAT SAYS FACTS OF ITSELF is the one other
        // arrangement, and it has no sentence and no values' row — its facts
        // are both ([`State::foot_lines`]):
        //
        //   0        its name, or its headline when it has none
        //   1..      what it says under itself, two rows at most
        //   ..       one fact a row, its label dim and its value in its tone
        //   ..last   the last fact, taking every row left
        //
        // with the row's other action beside the first fact, or on a last row
        // of its own where that does not leave it room.
        let values_at = last.saturating_sub(1);

        if let Some(name) = &told.name {
            pen.put(0, Run::label(name).title());
            self.foot_spots.push(FootSpot {
                row: 0,
                from: 0,
                to: wide,
                about: told
                    .name_about
                    .clone()
                    .unwrap_or_else(|| key::ABOUT_FOOT.to_owned()),
            });
        } else if let Some(headline) = &told.headline {
            pen.put(0, Run::datum(headline).title().within(wide));
        }
        // EVERYTHING ELSE IT DRAWS says what the footer is, under the pointer.
        self.foot_drawn =
            !told.subject().replace('\u{1f}', "").is_empty() || !told.facts.is_empty();
        // THE LINE THE BUFFER HOLDS, under the name. It is the longest thing
        // on this screen — a step's real command — and it is DATA, so it is
        // the one part of the footer this side can measure and the host does
        // not have to elide a key for.
        let mut from = 1;
        if let Some(said) = &told.said
            && from < values_at
        {
            pen.put(from, Run::datum(said).playing(Role::Body));
            if let Some(about) = told.said_about {
                self.foot_spots.push(FootSpot {
                    row: from,
                    from: 0,
                    to: wide,
                    about: about.to_owned(),
                });
            }
            from += 1;
        } else if let Some(step) = &told.at_step
            && from < values_at
        {
            // WHERE A RECORD STOPPED, on the same line: the label in the
            // figures' discreet tone, the step's own name after it. Measured
            // from what the host said the label is, like every figure here.
            let label = self.cells_of(key::AT_STEP, FACT_CELLS);
            let step_at = label + ronoctua_design::GAP;
            pen.put(from, Run::label(key::AT_STEP).dim().within(label));
            pen.put(
                from,
                Run::label(step)
                    .playing(Role::Body)
                    .at(step_at)
                    .within(wide.saturating_sub(step_at)),
            );
            self.foot_spots.push(FootSpot {
                row: from,
                from: 0,
                to: wide,
                about: key::ABOUT_RECORD_AT.to_owned(),
            });
            from += 1;
        }
        // WHAT THE ROW SAYS UNDER ITSELF, in the tone it wears there, leaving
        // every fact a row. The page has all of it under the row; what is
        // cut here moves by a bar of its own, as it does on the page.
        if let Some(below) = &told.below {
            let left = last.saturating_sub(from);
            let facts = u16::try_from(told.lines.len()).unwrap_or(u16::MAX);
            let rows = self
                .wraps_to(below, wide, BELOW_FOOT)
                .min(left.saturating_sub(facts))
                .max(1)
                .min(left);
            if rows > 0
                && let Some(run) = cell_run(below)
            {
                let run = run.playing(Role::Folded).within(wide);
                pen.paragraph_from(from, rows, self.foot_below_scroll, run);
                self.foot_below_at = Some((from, wide.saturating_sub(1)));
                from += rows;
            }
        }
        // A ROW THE MODULE SAID MORE OF has its facts for a sentence.
        let also = told
            .also
            .as_deref()
            .filter(|also| self.also_cells(also) < wide);
        if !told.lines.is_empty() {
            self.foot_lines(&mut pen, told, also, from, last);
            return;
        }
        // THE SENTENCE TAKES EVERYTHING THAT IS LEFT: the error when there is
        // one, the description when there is not. Only the values' row is held
        // back, and only when there are values — or a click — to put in it.
        let until = if told.facts.is_empty() && told.opens.is_none() && also.is_none() {
            last
        } else {
            values_at
        };
        let room = until.saturating_sub(from);
        if room > 0 {
            // AND IT SCROLLS. A message longer than the footer is a message
            // nobody can read to the end of, and the owner asked for the
            // obvious answer: *"o footer do brain DEVE funcionar com um
            // scrollbar própria"*.
            //
            // What this side keeps is the OFFSET; the total is the host's,
            // because the host holds the translated string and is what wraps
            // it. It draws the thumb beside the words and clamps the number
            // before handing it back — see `ronoctua_protocol::Command::
            // Paragraph`. THE WHOLE WIDTH IS GIVEN, and the host keeps the
            // thumb's lane out of the words only while there is a thumb: a cell
            // kept here as well put the thumb over the last letter of every row
            // (photographed by the owner), and a sentence that fits lost a cell
            // to a thumb it had not got.
            let box_wide = wide;
            let scroll = self.foot_scroll;
            if let Some((text, role)) = &told.logged {
                pen.paragraph_from(
                    from,
                    room,
                    scroll,
                    Run::datum(text).playing(*role).within(box_wide),
                );
            } else if let Some(trouble) = &told.trouble {
                // THE MACHINE'S OWN WORDS ARE DATA, never a key.
                pen.paragraph_from(
                    from,
                    room,
                    scroll,
                    Run::datum(trouble).playing(Role::Danger).within(box_wide),
                );
            } else if let Some(about) = &told.about {
                pen.paragraph_from(
                    from,
                    room,
                    scroll,
                    Run::label(about).quiet().within(box_wide),
                );
            }
        }
        if let Some(also) = also {
            self.foot_also(&mut pen, also, values_at);
        }
        // AND THE FIGURES ONLY WHERE THERE IS A ROW FOR THEM THAT IS NOT THE
        // NAME'S. Six is the height this is written for and `min(left)` is the
        // only thing that shortens it — but in a six-line WINDOW the footer
        // gets one row, and with `last == 1` the last row IS row zero: the
        // values were drawn on top of the name. Found by the frame's own
        // check, which is why that check is now asked at every size.
        //
        // What goes is the figures, because the name says WHAT is in focus and
        // a date with nothing naming it is a number on a line.
        //
        // THE CLICK THAT OPENS THE PAGE FIRST, flush against the right edge of
        // that row — the owner's *"do lado direito ao final da linha, em todas
        // as tarefas, exista um clicável para abrir a página da tarefa"* — so
        // the figures are laid out in what it leaves. In the tone every control
        // in this buffer's rows wears (the `⌥` of a schedule's row), and lit
        // under the pointer by a ground under it alone — the row it sits on is
        // not what the click takes.
        self.open_at = None;
        let mut click_from = None;
        if let Some(task) = told.opens
            && values_at < last
            && values_at > 0
        {
            let cells = self.cells_of(key::OPEN_PAGE, FACT_CELLS);
            let from = wide.saturating_sub(cells);
            if let Some(seat) = self.pointed_in(self.blocks.foot, values_at, from, wide) {
                pen.ground_under(values_at, from, cells, Role::hover_of(seat));
            }
            // COUNTED FROM THE LEFT like the ground under it, so the two are
            // the same cells at every width.
            pen.put(
                values_at,
                Run::label(key::OPEN_PAGE).accent().at(from).within(cells),
            );
            self.open_at = Some(OpenAt {
                row: values_at,
                from,
                to: wide,
                task,
            });
            click_from = Some(from);
        }
        if told.facts.is_empty() || values_at >= last || values_at == 0 {
            return;
        }
        // A TABLE AND NOT A RUNNING SUM. The first shape of this measured
        // each label by its KEY's length — `brain-last-run` is fourteen
        // characters and `Last run` is eight — so every value landed on top
        // of the label before it. A key's length is not a label's length, and
        // this side learns one only from the host (`Input::Phrases`): it
        // measures what it was told, and reserves what it was not.
        let widths: Vec<u16> = told
            .facts
            .iter()
            .map(|(label, _)| {
                if label.is_empty() {
                    return 0;
                }
                let said = self.phrases.or(label, "");
                if said.is_empty() {
                    FACT_CELLS
                } else {
                    ronoctua_design::wide(&said).min(FACT_CELLS)
                }
            })
            .collect();
        // THE LAST ROW IS DISCREET, label and value: the owner's *"as infos
        // mais específicas que aparecem na última linha (ex: 'Last run...'),
        // tenha uma cor discreta"*. The body's dimmest step, and never the
        // description's: the sentence above is `quiet`, and a footer that drew
        // both in one grey read as one block, which the owner said once
        // before about this footer.
        if let Some(click) = click_from {
            self.facts_against(&mut pen, told, &widths, values_at, click);
            return;
        }
        let layout = facts_columns(wide, &widths);
        let Some(mut row) = pen.row_of(values_at, &layout) else {
            return;
        };
        for ((label, value), (lid, vid)) in told.facts.iter().zip(FACT_SLOTS) {
            // HALF A FACT IS WORSE THAN NONE. The two columns carry the same
            // rank and a tie is broken by the later declaration, so the VALUE
            // gives way first — which left `Processor` on the screen with
            // nothing after it, read off a photograph at eighty columns. A
            // pair goes whole or not at all.
            if !layout.has(vid) {
                break;
            }
            if !label.is_empty() {
                row.put(lid, Run::label(label).dim());
            }
            row.put(vid, Run::datum(value).dim());
            if let Some(v) = layout.field(vid) {
                let first = layout.field(lid).filter(|_| !label.is_empty()).unwrap_or(v);
                self.foot_spots.push(FootSpot {
                    row: values_at,
                    from: first.at,
                    to: v.at + v.width,
                    about: fact_about(label).to_owned(),
                });
            }
        }
    }

    /// THE FACTS AGAINST THE CLICK: laid out to END [`FACT_AIR`] cells before
    /// `click`, where the eye already is — the owner's *"a data do 'last run'
    /// em vez de ficar no início da linha, fique logo ao lado esquerdo do
    /// 'Open its page', mas separado por um bom espaço"*. Whole pairs, the
    /// first first: a pair that does not fit goes, with every one after it.
    fn facts_against(
        &mut self,
        pen: &mut Pen<'_, Wit>,
        told: &Told,
        widths: &[u16],
        row: u16,
        click: u16,
    ) {
        let room = click.saturating_sub(FACT_AIR);
        let pairs: Vec<(u16, u16)> = told
            .facts
            .iter()
            .zip(widths)
            .map(|((_, value), label)| (*label, ronoctua_design::wide(value)))
            .collect();
        let pair_cells = |(label, value): (u16, u16)| {
            if label == 0 {
                value
            } else {
                label + ronoctua_design::GAP + value
            }
        };
        let mut many = 0;
        let mut used: u16 = 0;
        for (n, pair) in pairs.iter().enumerate() {
            let cells = pair_cells(*pair) + if n > 0 { FACT_SPACING } else { 0 };
            if used + cells > room {
                break;
            }
            used += cells;
            many += 1;
        }
        let mut at = room - used;
        for (n, ((label, value), (label_cells, value_cells))) in
            told.facts.iter().zip(&pairs).take(many).enumerate()
        {
            if n > 0 {
                at += FACT_SPACING;
            }
            let first = at;
            if *label_cells > 0 {
                pen.put(row, Run::label(label).dim().at(at).within(*label_cells));
                at += label_cells + ronoctua_design::GAP;
            }
            pen.put(row, Run::datum(value).dim().at(at).within(*value_cells));
            at += value_cells;
            self.foot_spots.push(FootSpot {
                row,
                from: first,
                to: at,
                about: fact_about(label).to_owned(),
            });
        }
    }

    /// WHAT A MODULE SAYS OF A ROW, from `from` down: each fact a row, its
    /// label dim in a column as wide as the widest and its value in the tone
    /// the module gave, so nothing here reads as one block. The last takes
    /// every row left, and scrolls: it is what a row cuts.
    ///
    /// The row's other action sits beside the first fact, whose value is
    /// short, and on a row of its own where that does not leave it room.
    fn foot_lines(
        &mut self,
        pen: &mut Pen<'_, Wit>,
        told: &Told,
        also: Option<&str>,
        from: u16,
        last: u16,
    ) {
        use ronoctua_design::GAP;
        let Some((closing, early)) = told.lines.split_last() else {
            return;
        };
        let wide = pen.columns();
        let labels = told
            .lines
            .iter()
            .map(|fact| self.cells_of(&fact.label, FACT_CELLS))
            .fold(0, u16::max)
            .min(wide / 2);
        let at = labels + GAP;
        let cells = wide.saturating_sub(at);
        let beside = also.filter(|also| {
            from + 1 < last
                && early.first().is_some_and(|first| {
                    let value = at + self.cell_cells(&first.value, FACT_CELLS);
                    value + FACT_AIR + self.also_cells(also) <= wide
                })
        });
        let until = if also.is_some() && beside.is_none() {
            last.saturating_sub(1)
        } else {
            last
        };
        let mut row = from;
        for fact in early {
            // THE LAST ROW IS THE LAST FACT'S, however few there are.
            if row + 1 >= until {
                break;
            }
            pen.put(row, Run::label(&fact.label).dim().within(labels));
            put_fact(pen, row, (at, cells), fact);
            row += 1;
        }
        if row < until {
            pen.put(row, Run::label(&closing.label).dim().within(labels));
            if stamped(&closing.value).is_some() {
                put_fact(pen, row, (at, cells), closing);
            } else if let Some(run) = cell_run(&closing.value) {
                let run = run.playing(tone_role(closing.tone));
                pen.paragraph_from(row, until - row, self.foot_scroll, run.at(at).within(cells));
            }
        }
        match (beside, also) {
            (Some(also), _) => self.foot_also(pen, also, from),
            (None, Some(also)) if until > from => self.foot_also(pen, also, until),
            _ => {}
        }
    }

    /// How wide the row's other action comes out: its key at rest and its
    /// name, `key` being the key of the name.
    fn also_cells(&self, key: &str) -> u16 {
        let keys = key_of_command("brain-row-also").unwrap_or_default();
        let spelling = self.phrases.or(Phrases::LEADER_ALT, "");
        ronoctua_design::leader_alt::cells(&spelling, keys, false)
            + ronoctua_design::GAP
            + self.cells_of(key, FACT_CELLS)
    }

    /// THE ROW'S OTHER ACTION, by what it does on this row, flush right on
    /// `row`: the one key changes its meaning from row to row, and what the
    /// key is called can only say so. A click takes it, at once, like the one
    /// that opens a page: with no header on the window this is the only place
    /// the pointer finds it.
    fn foot_also(&mut self, pen: &mut Pen<'_, Wit>, label: &str, row: u16) {
        let Some(keys) = key_of_command("brain-row-also") else {
            return;
        };
        // NEVER ON THE NAME'S ROW, in a footer too short for any other.
        if row == 0 || row >= pen.lines() {
            return;
        }
        let wide = pen.columns();
        let name = self.cells_of(label, FACT_CELLS);
        let spelling = self.phrases.or(Phrases::LEADER_ALT, "");
        let from = wide.saturating_sub(self.also_cells(label));
        let columns = |spelt| {
            Table::across(wide)
                .with(Column::fill("rest", 1))
                .with(Column::fixed(
                    "keys",
                    ronoctua_design::leader_alt::cells(&spelling, keys, spelt),
                    2,
                ))
                .with(Column::fixed("name", name, 2))
                .resolve()
        };
        // SPELT while the pointer rests where the glyph is at rest, so the
        // spelling stays under the hand that asked for it.
        let spelt = !spelling.is_empty() && self.hovering(self.blocks.foot, row, from);
        let layout = columns(spelt);
        let first = layout.field("keys").map_or(from, |f| f.at);
        if let Some(seat) = self.pointed_in(self.blocks.foot, row, first, wide) {
            pen.ground_under(row, first, wide - first, Role::hover_of(seat));
        }
        let Some(mut cells) = pen.row_of(row, &layout) else {
            return;
        };
        let _ = cells.put_shortcut(
            "keys",
            Icon::LeaderAlt,
            &spelling,
            keys,
            spelt,
            Role::Accent,
        );
        cells.put("name", Run::label(label).accent());
        self.also_at = Some((row, first, wide));
        self.foot_spots.push(FootSpot {
            row,
            from: first,
            to: wide,
            about: key::ABOUT_ROW_ALSO.to_owned(),
        });
    }
}

/// What a fact of a row says, in `room`: `(its first cell, how many)`. A date
/// in its pieces, like every date of this buffer, and all a row may change of
/// it is what its tone says of a value.
fn put_fact(pen: &mut Pen<'_, Wit>, row: u16, room: (u16, u16), fact: &RowFact) {
    if let Some(moment) = stamped(&fact.value) {
        let rest = dated_value_role(fact.tone);
        put_stamp(pen, row, room, &moment.text, &moment.stamp, rest);
    } else if let Some(run) = cell_run(&fact.value) {
        let run = run.playing(tone_role(fact.tone));
        pen.put(row, run.at(room.0).within(room.1));
    }
}

/// THE AIR BETWEEN THE FACTS AND THE CLICK they sit against: *"separado por
/// um bom espaço"* — four columns' gaps, so the date reads as the click's
/// neighbour and not as one more word of it.
const FACT_AIR: u16 = 4 * ronoctua_design::GAP;

/// What a figure of the footer's last row says under the pointer, by its
/// label.
fn fact_about(label: &str) -> &'static str {
    match label {
        key::LAST_RUN => key::ABOUT_LAST_RUN,
        key::FIRST => key::ABOUT_FIRST,
        key::TIMES => key::ABOUT_TIMES,
        key::SINCE => key::ABOUT_SINCE,
        key::NEEDS_ROOT => key::ABOUT_NEEDS_ROOT,
        key::FLAKE_OWNED => key::ABOUT_FLAKE_OWNED,
        _ => key::ABOUT_FOOT,
    }
}

/// What a heading of a task's page says under the pointer: what the rows
/// under it are.
fn heading_about(heading: &str) -> &'static str {
    match heading {
        key::OPTIONS => key::ABOUT_OPTIONS,
        key::STEPS => key::ABOUT_STEPS,
        _ => key::ABOUT_HISTORY,
    }
}

/// The air between one fact and the next, when they sit against the click:
/// wider than a label's from its value, so two pairs read as two.
const FACT_SPACING: u16 = 3 * ronoctua_design::GAP;

// ---------------------------------------------------------------------------
// The home page: everything the brain has, and everything it has done.
// ---------------------------------------------------------------------------

/// How wide each of the list's measured columns comes out today, in the
/// person's language — see [`every_phrase`] for why they are measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TaskCells {
    /// The widest name.
    name: u16,
    /// The widest moment: a date, or the words for why there is none.
    when: u16,
    /// The widest status.
    note: u16,
}

/// The narrowest the moment is given before a column gives way instead: a
/// date, whole.
const WHEN_FLOOR: u16 = 16;

/// The columns of a row in the list of tasks.
///
/// The kind's glyph is to the LEFT of the moment, which is where the owner
/// asked for it: it belongs to when the task happens rather than to what it is
/// called.
///
/// # Every column as wide as what it holds, and the moment takes the rest
///
/// It was a name column that FILLED and three reserved ones beside it — so the
/// row was the widest name and forty cells of nothing, then a moment of
/// seventeen that cut `After an account…` short. The owner read exactly that
/// off the screen. Now the name, the moment and the status are each as wide as
/// their widest entry, measured in the person's language ([`TaskCells`]); the
/// moment gives up what the row cannot hold, never below a whole date, and
/// past that the columns give way whole, the status first.
///
/// # Two cells between the three columns of words
///
/// The name, the moment and the status stand [`APART`](ronoctua_design::APART)
/// — the owner's *"algo como dois caracteres de distância"*: one cell after
/// the longest name read as one phrase with the column beside it. The glyph
/// keeps one to its moment, which it belongs to.
fn task_columns(columns: u16, cells: TaskCells) -> Layout {
    use ronoctua_design::{APART, GAP, ICON};
    // A CELL IS KEPT ON THE RIGHT FOR THE BAR, always — the same line
    // `log_columns` carries, and for the same reason: a bar drawn over the
    // last column is the defect the working rules name in those words.
    let usable = columns.saturating_sub(1);
    let others = cells.name + APART + ICON + GAP + APART + cells.note;
    let when = cells
        .when
        .min(usable.saturating_sub(others))
        .max(WHEN_FLOOR);
    Table::across(usable)
        .with(Column::fixed("name", cells.name, 1).apart())
        .with(Column::fixed("kind", ICON, 2))
        .with(Column::fixed("when", when, 1).apart())
        .with(Column::fixed("note", cells.note, 3))
        .resolve()
}

/// The widest name among the runs about to be drawn, in cells.
///
/// [`NAME_CELLS`] when nothing is known — an empty list, or a host that sends
/// no names — because that is the reservation this column had before the names
/// crossed, and a column of zero cells would draw no name at all.
fn widest_name(runs: &[RunRecord]) -> u16 {
    runs.iter()
        .map(|r| ronoctua_design::wide(&r.task_text))
        .max()
        .filter(|w| *w > 0)
        .map_or(NAME_CELLS, |w| w.min(NAME_MOST))
}

/// The columns of a row in the activity log.
///
/// A cell is kept on the right for the bar, always: a bar drawn over the last
/// column is the defect the working rules name in those words.
/// The columns of one line of the daemon's log: when, how loud, what.
fn tail_columns(columns: u16) -> Layout {
    Table::across(columns.saturating_sub(1))
        .with(Column::fixed("at", 8, 2))
        .with(Column::fixed("level", 5, 1))
        .with(Column::fill("text", 0))
        .resolve()
}

/// A level as the log prints it. DATA, like the line it heads: it is the
/// log's own word, and the page is a copy of the log.
const fn level_word(l: LogLevel) -> &'static str {
    match l {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN",
        LogLevel::Info => "INFO",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
    }
}

/// The tone a level wears: the error's red, the warning's amber, what the
/// daemon did in the body's tone, and detail below it.
const fn level_role(l: LogLevel) -> Role {
    match l {
        LogLevel::Error => Role::Danger,
        LogLevel::Warn => Role::Warning,
        LogLevel::Info => Role::Body,
        LogLevel::Debug | LogLevel::Trace => Role::Dim,
    }
}

/// The columns of one notice: what it says, and since when.
fn notice_columns(columns: u16) -> Layout {
    Table::across(columns.saturating_sub(1))
        .with(Column::fill("says", 10))
        .with(Column::fixed("since", 16, 2))
        .resolve()
}

/// The columns of a row of the activity log.
///
/// `outcome` is how wide the widest outcome comes out, MEASURED in the
/// person's language (`Input::Phrases`).
///
/// `folds` is whether ANY row of the log folds more than one record: the two
/// columns of the fold — its mark and its count — exist only then, so a log
/// where nothing repeats is as compact as it always was, and the columns do
/// not come and go as the window scrolls.
///
/// THERE IS NO `Last:` any more before a folded row's date, at the owner's
/// word: the fold's mark and its count already say the row stands for many,
/// and the date is the newest's because every row's date is.
fn log_columns(columns: u16, name: u16, outcome: u16, folds: bool) -> Layout {
    let mut table = Table::across(columns.saturating_sub(1));
    if folds {
        // THE FOLD'S MARK AND ITS COUNT come first, where the eye starts the
        // row: a row that stands for forty is not a row like the others, and
        // the reader should know it before reading the rest. They are the
        // row's extras, so they give way before the date and the name.
        table = table
            .with(Column::fixed("fold", ronoctua_design::ICON, 4))
            .with(Column::fixed("count", COUNT_CELLS, 4));
    }
    table
        .with(Column::fixed("when", WHEN_FLOOR, 2))
        // AS WIDE AS THE WIDEST NAME ON SCREEN, and no wider.
        //
        // It was [`NAME_CELLS`] — twenty-six, the longest translation — for a
        // column whose rows are five cells long on this machine, and what sat
        // in the difference was the "enormous space between the title and the
        // message" the owner photographed. The host hands the names over
        // (`RunRecord::task_text`) so this side can measure them; the caller
        // takes the widest of the rows it is about to draw.
        .with(Column::fixed("task", name, 1))
        // WHAT WENT WRONG, AND IT IS THE COLUMN THAT FILLS.
        //
        // It is the one field here whose length nobody can predict — a
        // machine's own error is as long as the machine felt like being — so
        // it takes whatever the line has left and the host cuts it there. It
        // gives way FIRST (the highest rank) because it is the one thing on
        // this row that is an extra: a row with no reason is still a row, and
        // a row with no date or no name is not.
        .with(Column::fill("trouble", 0))
        .with(Column::fixed("outcome", outcome, 3))
        .resolve()
}

/// A MOMENT IN ITS THREE PIECES (`stamp`), from cell `at` with `room` cells:
/// the rest of the date in `rest`, and its DAY above it — the body's quiet
/// step, or today's own hue.
///
/// The owner's ask, for every date of the activity log and of a task's
/// history: *"o dia da data não tenha uma cor tão discreta como ocorre com o
/// resto da data e horário, e … o dia corrente receba ainda mais destaque"*.
/// A host that sent no pieces (`day` empty) gets the whole text in `rest`,
/// which is how every date was drawn before.
fn put_stamp(
    pen: &mut Pen<'_, Wit>,
    line: u16,
    (at, room): (u16, u16),
    whole: &str,
    stamp: &ronoctua_design::types::Stamp,
    rest: Role,
) {
    if room == 0 {
        return;
    }
    if stamp.day.is_empty() {
        pen.put(line, Run::datum(whole).playing(rest).at(at).within(room));
        return;
    }
    let day = if stamp.today {
        Role::Today
    } else {
        Role::Quiet
    };
    let mut x = at;
    let end = at.saturating_add(room);
    for (text, role) in [(&stamp.lead, rest), (&stamp.day, day), (&stamp.tail, rest)] {
        let left = end.saturating_sub(x);
        if left == 0 || text.is_empty() {
            continue;
        }
        let wide = ronoctua_design::wide(text).min(left);
        pen.put(line, Run::datum(text).playing(role).at(x).within(wide));
        x = x.saturating_add(wide);
    }
}

/// HOW FAR BACK A LIST REACHES AND HOW MANY IT HOLDS, beside its heading on
/// `line`, from [`COUNT_AT`] — `last 15 days  42`: the reach in the
/// person's words (the host made the sentence, `mind.activity-reach`) and
/// the count, told apart by their tones and the air between them. The
/// owner's ask for the activity block: *"«últimos 15 dias» … ao lado do
/// título 'Activity', e também a contagem de itens"*.
///
/// NOT THE SYSTEM'S DOT between them: the dot and the hint line's tones are
/// the host's shortcut line (`the-shortcut-line-is-the-systems`), and this is
/// no line of keys.
///
/// AT THE BODY'S SIZE, and not small: something is drawn after each piece,
/// and a small run does not advance the cells it was given.
fn put_reach(pen: &mut Pen<'_, Wit>, line: u16, end: u16, reach: &str, count: usize) {
    let mut x = COUNT_AT;
    if !reach.is_empty() {
        let wide = ronoctua_design::wide(reach);
        pen.put(
            line,
            Run::datum(reach)
                .dim()
                .at(x)
                .within(wide.min(end.saturating_sub(x).max(1))),
        );
        x = x.saturating_add(wide + 2 * ronoctua_design::GAP);
    }
    if x < end {
        pen.put(
            line,
            Run::datum(&count.to_string())
                .quiet()
                .at(x)
                .within(end.saturating_sub(x)),
        );
    }
}

impl State {
    /// The home page: the list of tasks, in its own block.
    ///
    /// The activity log is NOT drawn from here any more. It used to take
    /// "whatever is left", which on a tall window was a container the height
    /// of the screen with five rows in it — and it was placed by advancing a
    /// line counter through this function, which is how it came to be measured
    /// against a number the footer had already claimed.
    fn home(&mut self, screen: &mut Screen<Wit>) {
        self.title_from = None;
        let hovered = self.hovered_rows(self.blocks.body);
        let tasks: Vec<TaskState> = self.mind.tasks.clone();
        let cursor = self.task;
        let pane = self.pane;
        let moments: Vec<Run<Icon>> = tasks.iter().map(|t| self.moment_of(t)).collect();
        let notes: Vec<Option<Run<Icon>>> = tasks.iter().map(|t| self.note_of(t)).collect();
        let cells = self.task_cells();
        let mut pen = screen.region(self.blocks.body);
        if pen.empty() {
            return;
        }
        if tasks.is_empty() {
            pen.put(0, Run::label(key::NOTHING).quiet());
            return;
        }
        // THE WINDOW ONTO THE LIST, and the lane its bar takes.
        //
        // The list drew from the first task and stopped when it ran out of
        // rows — so a machine with more tasks than the block has lines had
        // some it could not reach at all, and the wheel over it moved the
        // CURSOR instead of the page. Both are the same missing thing: an
        // offset. See [`State::follows`] for which of the two moves it.
        let shown = usize::from(pen.lines().saturating_sub(1)).max(1);
        pen.keeping_for(tasks.len() > shown);
        // THE TITLE, and — with no header over the window — whose page this
        // is, at the other end of the same line, in the quietest tone the
        // page has: the owner's *"o título 'Brain', porém em uma cor bem
        // discreta e apagada"*. Nothing else on the screen names it then.
        let mut title_room = pen.columns();
        if !self.headed {
            let cells = self.cells_of(key::TITLE, FACT_CELLS).min(pen.columns());
            let from = pen.columns().saturating_sub(cells);
            pen.put(0, Run::label(key::TITLE).dim().at(from).within(cells));
            title_room = from.saturating_sub(ronoctua_design::GAP);
            self.title_from = Some(from);
        }
        pen.put(0, Run::label(key::TASKS).title().within(title_room));
        if self.follows {
            if cursor < self.body_from {
                self.body_from = cursor;
            } else if cursor >= self.body_from + shown {
                self.body_from = cursor.saturating_sub(shown.saturating_sub(1));
            }
        }
        self.body_from = self
            .body_from
            .min(tasks.len().saturating_sub(shown.min(tasks.len())));
        let from = self.body_from;

        let layout = task_columns(pen.columns(), cells);
        self.task_layout = layout.clone();
        let mut row_at = 1;
        for (n, task) in tasks.iter().enumerate().skip(from) {
            if row_at >= pen.lines() {
                break;
            }
            if n == cursor && pane == Pane::Tasks {
                pen.band(row_at);
            } else if let Some(seat) = seat_on(&hovered, row_at) {
                // THE ROW UNDER A POINTER, in that cursor's own tone. Not the
                // cursor's band — that is where the KEYBOARD is, and the two
                // are different questions every other window in the shell has
                // always answered separately.
                pen.ground(row_at, Role::hover_of(seat));
            }
            let role = kind_role(task.kind);
            let Some(mut row) = pen.row_of(row_at, &layout) else {
                break;
            };
            if let Some(name) = key_of(&task.name_key) {
                row.put("name", Run::label(&name).playing(role));
            }
            row.put("kind", Run::glyph(kind_glyph(task.kind)).playing(role));
            row.put("when", moments[n].clone());
            if let Some(note) = notes[n].clone() {
                row.put("note", note);
            }
            row_at += row.done();
        }
        // AND THE BAR, in the lane kept for it. A list one cannot see the
        // length of is the defect `every-list-window-has-a-bar` names on the
        // host's side; a buffer owes the same thing.
        pen.bar_from(
            1,
            pen.columns().saturating_sub(1),
            Scroll {
                total: tasks.len(),
                lines: shown,
                offset: from,
                top: 0,
            },
        );
    }

    /// THE NOTICES: every sentence a module wants said, loudest first, with
    /// what each one answers to taught in the header.
    ///
    /// # Why a block of its own and not the warning line
    ///
    /// Because the warning line was ONE sentence, and the brain's modules say
    /// several things at once: a reminder ringing, a disk nearly full, a
    /// service that keeps failing. One line would have to choose, and the one
    /// it did not choose would be a notice nobody reads. The strip under the
    /// minibuffer still says the loudest; this is where all of them are.
    ///
    /// A block exactly as tall as what it holds, up to [`NOTICE_ROWS`], and
    /// none at all when nothing is said — a machine that is well spends no
    /// line saying so.
    fn draw_notices(&mut self, screen: &mut Screen<Wit>) {
        if self.blocks.notices.empty() {
            return;
        }
        let total = self.mind.notices.len();
        let shown = usize::from(self.blocks.notices.lines.saturating_sub(1)).max(1);
        if self.follows && self.pane == Pane::Notices {
            if self.notice < self.notices_from {
                self.notices_from = self.notice;
            } else if self.notice >= self.notices_from + shown {
                self.notices_from = self.notice.saturating_sub(shown.saturating_sub(1));
            }
        }
        self.notices_from = self
            .notices_from
            .min(total.saturating_sub(shown.min(total)));
        let from = self.notices_from;
        let hovered = self.hovered_rows(self.blocks.notices);
        let cursor = self.notice;
        let pane = self.pane;
        let seen: Vec<Notice> = self
            .mind
            .notices
            .iter()
            .skip(from)
            .take(shown)
            .cloned()
            .collect();
        let mut pen = screen.region(self.blocks.notices);
        let room = if total > 1 {
            COUNT_AT.saturating_sub(ronoctua_design::GAP)
        } else {
            pen.columns().saturating_sub(1)
        };
        pen.put(0, Run::label(key::NOTICES).title().within(room));
        // HOW MANY, beside the title, the way `Steps` and `Activity` say it:
        // a window of three onto seven is a different thing from three.
        if total > 1 {
            pen.put(
                0,
                Run::datum(&total.to_string())
                    .quiet()
                    .small()
                    .at(COUNT_AT)
                    .within(pen.columns().saturating_sub(COUNT_AT + 1)),
            );
        }
        pen.keeping_for(total > shown);
        let layout = notice_columns(pen.columns());
        for (n, notice) in seen.iter().enumerate() {
            let row = 1 + u16::try_from(n).unwrap_or(0);
            if from + n == cursor && pane == Pane::Notices {
                pen.band(row);
            } else if let Some(seat) = seat_on(&hovered, row) {
                pen.ground(row, Role::hover_of(seat));
            }
            let Some(mut line) = pen.row_of(row, &layout) else {
                break;
            };
            // THE SENTENCE IS DATA: the host made it, with its values, in the
            // person's language — a key with holes in it is not something
            // this side can draw.
            line.put(
                "says",
                Run::datum(&notice.says_text).playing(level_tone(notice.level)),
            );
            line.put("since", Run::datum(&notice.since_text).dim());
        }
        pen.bar_from(
            1,
            pen.columns().saturating_sub(1),
            Scroll {
                total,
                lines: shown,
                offset: from,
                top: 0,
            },
        );
    }

    /// Every line of the activity log as it is drawn: each row, and under an
    /// OPENED one the older records it folds.
    fn activity_lines(&self) -> Vec<Line> {
        let mut out = Vec::new();
        for (n, row) in self.mind.activity_rows.iter().enumerate() {
            out.push(Line::Row(n));
            if row.repeats > 1 && self.is_open(row) {
                out.extend((1..row.members.len()).map(|m| Line::Member(n, m)));
            }
        }
        out
    }

    /// What names a line of the log across minds: its row's fold, and for a
    /// member the moment its own record began.
    fn line_key(&self, line: Line) -> Option<LogKey> {
        match line {
            Line::Row(n) => self.mind.activity_rows.get(n).map(|r| LogKey {
                fold: Self::fold_of(r),
                member: None,
            }),
            Line::Member(n, m) => {
                let row = self.mind.activity_rows.get(n)?;
                Some(LogKey {
                    fold: Self::fold_of(row),
                    member: Some(row.members.get(m)?.began),
                })
            }
        }
    }

    /// What names a folded row across minds: its task, and when its OLDEST
    /// record began — which a newer record joining it does not move.
    fn fold_of(row: &ActivityRow) -> (String, u64) {
        let oldest = row.members.last().map_or(row.newest.began, |r| r.began);
        (row.newest.task.clone(), oldest)
    }

    /// Whether a folded row is open.
    fn is_open(&self, row: &ActivityRow) -> bool {
        self.opened.contains(&Self::fold_of(row))
    }

    /// The record a line of the log is about.
    fn record_of(&self, line: Line) -> Option<&RunRecord> {
        match line {
            Line::Row(n) => self.mind.activity_rows.get(n).map(|r| &r.newest),
            Line::Member(n, m) => self
                .mind
                .activity_rows
                .get(n)
                .and_then(|r| r.members.get(m)),
        }
    }

    /// THE ACTIVITY LOG: a container with its own ground, its own title and
    /// its own scrollbar, sitting directly on top of the footer.
    ///
    /// # Why it is a block and not the end of the home page
    ///
    /// Because it was the end of the home page, taking every line that was
    /// left, and the owner read the result: a container the height of the
    /// screen with five entries in it. What fifteen days of a log want is a
    /// WINDOW onto them — eight at a time and a bar for the rest — and a
    /// window is a fixed height, which is a block.
    ///
    /// # And what says the same thing twice is ONE row
    ///
    /// A watch writes the same line as often as the thing it watches happens,
    /// and a hold is written once per episode — but an episode comes back
    /// every night. Forty rows of "the memory was busy" bury the one row that
    /// says the update failed. The brain folds them (`journal::grouped`) and
    /// the row says so: the fold's triangle and HOW MANY in the first column,
    /// and `RET` on it opens every record it folds, one line each, under it —
    /// only their dates, in a cool grey of their own (`Role::Folded`): the
    /// row above already says what each of them was.
    ///
    /// # Its ground is its own
    ///
    /// Three tones, and they have to be three: the page, this container
    /// (`Role::Shelf`, `surface.container`), and the chrome that frames it —
    /// the header and the footer, one rung higher (`Role::Header`). Drawn in
    /// the footer's tone it read as part of the footer; drawn in the page's it
    /// read as nothing at all. There is no border to tell them apart with
    /// (invariant 15), so the ground is the whole of what is left.
    fn draw_log(&mut self, screen: &mut Screen<Wit>) {
        if self.blocks.log.empty() {
            return;
        }
        let shown = usize::from(LOG_ROWS);
        let lines = self.activity_lines();
        if !lines.is_empty() {
            self.log_from = self.log_from.min(lines.len().saturating_sub(1));
            // THE WINDOW FOLLOWS THE CURSOR, and by the shortest move — and
            // ONLY WHEN A PERSON MOVED IT. See [`State::follows`].
            if self.follows {
                if self.log < self.log_from {
                    self.log_from = self.log;
                } else if self.log >= self.log_from + shown {
                    self.log_from = self.log.saturating_sub(shown.saturating_sub(1));
                }
            }
            self.log_from = self
                .log_from
                .min(lines.len().saturating_sub(shown.min(lines.len())));
        }
        let hovered = self.hovered_rows(self.blocks.log);
        let from = self.log_from;
        let cursor = self.log;
        let pane = self.pane;
        let total = lines.len();
        // THE RECORDS, and not the rows, are what the title counts: forty
        // folded into one row are still forty things that happened.
        let records: u64 = self
            .mind
            .activity_rows
            .iter()
            .map(|r| u64::from(r.repeats))
            .sum();
        let reach = self.mind.activity_reach.clone();
        let seen: Vec<(Line, RunRecord)> = lines
            .iter()
            .skip(from)
            .take(shown)
            .filter_map(|l| self.record_of(*l).map(|r| (*l, r.clone())))
            .collect();
        let folds: Vec<(u32, bool)> = self
            .mind
            .activity_rows
            .iter()
            .map(|r| (r.repeats, self.is_open(r)))
            .collect();
        let outcome_cells = self.outcome_cells();
        let names: Vec<RunRecord> = seen.iter().map(|(_, r)| r.clone()).collect();

        let mut pen = screen.region(self.blocks.log);
        pen.fill(Role::Shelf);
        // THE TITLE IS INSIDE THE GROUND, and it stops where the reach begins
        // — a key reserves the whole of what it is given.
        pen.put(
            0,
            Run::label(key::ACTIVITY)
                .title()
                .within(COUNT_AT.saturating_sub(ronoctua_design::GAP)),
        );
        // AND HOW FAR BACK, AND HOW MANY — the owner's *"«últimos 15 dias» …
        // e também a contagem de itens"*, in the shape the `History` heading
        // on a task's page has too.
        let edge = pen.columns().saturating_sub(1);
        put_reach(
            &mut pen,
            0,
            edge,
            &reach,
            usize::try_from(records).unwrap_or(usize::MAX),
        );
        if seen.is_empty() {
            // SMALL IS ALLOWED HERE, and only here: nothing is positioned
            // after this run.
            pen.put(1, Run::label(key::NO_ACTIVITY).quiet().small());
            return;
        }
        // THE LANE THE BAR TAKES, declared before a single row is drawn.
        pen.keeping_for(total > shown);
        let any_fold = folds.iter().any(|(repeats, _)| *repeats > 1);
        let layout = log_columns(pen.columns(), widest_name(&names), outcome_cells, any_fold);
        self.log_layout = layout.clone();
        let when = layout.field("when").map(|f| (f.at, f.width));
        for (n, (line, run)) in seen.iter().enumerate() {
            let row = 1 + u16::try_from(n).unwrap_or(0);
            let at = from + n;
            if at == cursor && pane == Pane::Log {
                pen.band(row);
            } else if let Some(seat) = seat_on(&hovered, row) {
                pen.ground(row, Role::hover_of(seat));
            }
            let Some(mut cells) = pen.row_of(row, &layout) else {
                break;
            };
            // A RECORD AN OPENED ROW FOLDS is drawn in a tone of its own — a
            // cool grey, quieter than the row's (`Role::Folded`, the owner's
            // *"outro tom de cor discreta"*) — and with nothing the row above
            // already says: no fold's mark, no name, no outcome. It belongs
            // to the row above it, which says what every one of them was.
            let member = matches!(line, Line::Member(..));
            let rest = if member { Role::Folded } else { Role::Dim };
            if let Line::Row(g) = line
                && let Some((repeats, open)) = folds.get(*g).copied()
                && repeats > 1
            {
                // THE FOLD'S MARK: the triangle points at what it holds —
                // right while shut, down while open — and the count says how
                // many. A glyph and a number, never a word: the column is
                // four digits wide in every language.
                cells.put(
                    "fold",
                    Run::glyph(if open {
                        Icon::BrainGroupOpen
                    } else {
                        Icon::BrainGroup
                    })
                    .quiet(),
                );
                cells.put("count", Run::datum(&repeats.to_string()).quiet());
            }
            if !member && let Some(name) = key_of(&run.task_key) {
                cells.put("task", Run::label(&name).quiet());
            }
            // WHAT WENT WRONG, BESIDE THE NAME, as far as it fits — or what
            // a note says. The machine's own words and the module's sentence
            // are both DATA, never a key.
            if run.outcome == OutcomeKind::Noted && !run.says_text.is_empty() {
                cells.put("trouble", Run::datum(&run.says_text).playing(rest));
            } else if !run.trouble.is_empty() {
                cells.put("trouble", Run::datum(&run.trouble).playing(rest));
            }
            if !member {
                cells.put(
                    "outcome",
                    Run::label(&outcome_key(run.outcome)).playing(outcome_role(run.outcome)),
                );
            }
            // AND THE DATE, in its three pieces: placed after the row's last
            // use, since a moment is three runs in one column.
            if let Some(field) = when {
                put_stamp(&mut pen, row, field, &run.when_text, &run.when_stamp, rest);
            }
        }
        // THE BAR IS OVER THE ROWS AND NOT THE TITLE.
        pen.bar_from(
            1,
            pen.columns().saturating_sub(1),
            Scroll {
                total,
                lines: shown,
                offset: from,
                top: 0,
            },
        );
    }

    /// THE DAEMON'S LOG, the whole page below the header: one line per entry,
    /// newest at the bottom, and the window stays on the newest while the
    /// cursor does.
    fn log_page(&mut self, screen: &mut Screen<Wit>) {
        let hovered = self.hovered_rows(self.blocks.body);
        let mut pen = screen.region(self.blocks.body);
        if pen.empty() {
            return;
        }
        if self.tail.is_empty() {
            pen.put(0, Run::label(key::NO_LOG).quiet());
            return;
        }
        let total = self.tail.len();
        let shown = usize::from(pen.lines()).max(1);
        // THE LANE THE BAR TAKES, only while the bar is drawn.
        pen.keeping_for(total > shown);
        if self.follows {
            if self.tail_at < self.tail_from {
                self.tail_from = self.tail_at;
            } else if self.tail_at >= self.tail_from + shown {
                self.tail_from = self.tail_at.saturating_sub(shown.saturating_sub(1));
            }
        }
        self.tail_from = self.tail_from.min(total.saturating_sub(shown.min(total)));
        let from = self.tail_from;
        let layout = tail_columns(pen.columns());
        for (n, line) in self.tail.iter().enumerate().skip(from).take(shown) {
            let row = u16::try_from(n - from).unwrap_or(u16::MAX);
            if n == self.tail_at {
                pen.band(row);
            } else if let Some(seat) = seat_on(&hovered, row) {
                pen.ground(row, Role::hover_of(seat));
            }
            let Some(mut r) = pen.row_of(row, &layout) else {
                break;
            };
            let tone = level_role(line.level);
            r.put("at", Run::datum(&line.at_text).dim());
            r.put("level", Run::datum(level_word(line.level)).playing(tone));
            r.put("text", Run::datum(&line.text).playing(tone));
        }
        pen.bar_from(
            0,
            pen.columns().saturating_sub(1),
            Scroll {
                total,
                lines: shown,
                offset: from,
                top: 0,
            },
        );
    }

    /// The daemon's log arrived, whole.
    ///
    /// THE CURSOR RIDES THE NEWEST LINE while it is on it, so a page left open
    /// keeps showing what was just written; moved up to read something, it
    /// stays on THAT line — found again by its number, since older lines fall
    /// off the front of the ring.
    fn logged(&mut self, lines: Vec<LogLine>) -> Vec<Effect> {
        let at_the_end = self.tail_at + 1 >= self.tail.len();
        let held = self.tail.get(self.tail_at).map(|l| l.seq);
        self.tail = lines;
        if at_the_end {
            self.tail_at = self.tail.len().saturating_sub(1);
            self.follows = true;
        } else {
            self.tail_at = held
                .and_then(|seq| self.tail.iter().position(|l| l.seq == seq))
                .unwrap_or(0);
        }
        if self.page == Page::Log {
            vec![Effect::Redraw]
        } else {
            Vec::new()
        }
    }

    /// When this task will next be considered, or the word for why it will not.
    fn moment_of(&self, task: &TaskState) -> Run<Icon> {
        self.moment(task).0
    }

    /// The same, and how many cells it comes out in — one walk for the run
    /// and its width, so the column measured is the column drawn.
    fn moment(&self, task: &TaskState) -> (Run<Icon>, u16) {
        // A MODULE THAT ONLY LISTENS has no next moment: it is watching, and
        // the row says that instead of a date it does not have.
        if task.kind == TaskKind::Watch {
            return (
                Run::label(key::WATCHING).playing(kind_role(task.kind)),
                self.cells_of(key::WATCHING, WHEN_CELLS),
            );
        }
        if let Some(why) = key_of(&task.why_key) {
            // THE REASON IN WORDS, as the host resolved it for measuring
            // (`task-state::why-text`) — the one column of this list whose
            // words are a module's and not this buffer's.
            let cells = if task.why_text.is_empty() {
                WHEN_CELLS
            } else {
                ronoctua_design::wide(&task.why_text)
            };
            return (Run::label(&why).quiet(), cells);
        }
        if task.standing == Standing::Running {
            let running = standing_key(Standing::Running);
            let cells = self.cells_of(&running, WHEN_CELLS);
            return (Run::label(&running).accent(), cells);
        }
        if task.next_text.is_empty() {
            return (
                Run::label(key::NEVER).quiet(),
                self.cells_of(key::NEVER, WHEN_CELLS),
            );
        }
        (
            Run::datum(&task.next_text).quiet(),
            ronoctua_design::wide(&task.next_text),
        )
    }

    /// What is happening to it, in one short phrase — or nothing.
    fn note_of(&self, task: &TaskState) -> Option<Run<Icon>> {
        self.note(task).map(|(run, _)| run)
    }

    /// The same, and how many cells it comes out in.
    ///
    /// # Discreet, and in the hue of the news
    ///
    /// The owner's two asks at once: *"uma cor mais discreta"*, and the hue
    /// changing with the state — a run that worked in a quiet green, a turn
    /// waited for in a quiet purple. A HOLD is no longer the alarm's red: the
    /// machine being busy is a wait, not a fault, and the warning line above
    /// the list is where an alarm is said.
    fn note(&self, task: &TaskState) -> Option<(Run<Icon>, u16)> {
        let said = |key: String, role: Role| {
            let cells = self.cells_of(&key, NOTE_CELLS);
            (Run::label(&key).playing(role), cells)
        };
        if let Some(hold) = hold_key(task.hold) {
            return Some(said(hold, Role::StatusHeld));
        }
        if task.ran {
            return Some(said(
                outcome_key(task.last.outcome),
                outcome_role(task.last.outcome),
            ));
        }
        // NOR IS A MODULE THE SYSTEM SWITCHED OFF: it waits for nothing, and
        // the column before this one already says why.
        if off_by_the_system(task) {
            return None;
        }
        // A WATCH IS NEVER "WAITING": it has no turn to wait for. Its column
        // says nothing until it has noted something.
        if task.kind == TaskKind::Watch {
            return None;
        }
        Some(said(
            standing_key(task.standing),
            standing_role(task.standing),
        ))
    }

    /// How wide the widest outcome comes out — every outcome there is, so a
    /// history's and the log's columns do not move with what happened.
    fn outcome_cells(&self) -> u16 {
        OUTCOMES
            .iter()
            .map(|o| self.cells_of(&outcome_key(*o), 12))
            .max()
            .unwrap_or(12)
    }

    /// How wide the list's columns come out for the tasks there are now.
    fn task_cells(&self) -> TaskCells {
        let tasks = &self.mind.tasks;
        let name = tasks
            .iter()
            .map(|t| ronoctua_design::wide(&t.name_text))
            .max()
            .filter(|w| *w > 0)
            .map_or(NAME_CELLS, |w| w.min(NAME_MOST));
        let when = tasks
            .iter()
            .map(|t| self.moment(t).1)
            .max()
            .unwrap_or(WHEN_CELLS);
        // A LIST WITH NO STATUS AT ALL still keeps a narrow column, so the
        // first status to come does not move the rest of the row.
        let note = tasks
            .iter()
            .filter_map(|t| self.note(t).map(|(_, w)| w))
            .max()
            .unwrap_or(0)
            .max(4);
        TaskCells { name, when, note }
    }

    /// How many cells `key` comes out in, in the person's language — or
    /// `blind`, the reservation, while the host has not said.
    fn cells_of(&self, key: &str, blind: u16) -> u16 {
        let said = self.said_of(key);
        if said.is_empty() {
            blind
        } else {
            ronoctua_design::wide(&said)
        }
    }

    /// What `key` comes out as, to measure and never to draw: empty while the
    /// host has not said.
    fn said_of(&self, key: &str) -> String {
        let said = self.phrases.or(key, "");
        if said.is_empty() {
            self.page_phrases.or(key, "")
        } else {
            said
        }
    }

    /// How many rows, up to `most`, a cell wraps to `cells` wide — counted
    /// with the host's own wrap, and one for a key the host has not said.
    fn wraps_to(&self, cell: &ScreenCell, cells: u16, most: u16) -> u16 {
        let text = match cell {
            ScreenCell::Key(k) => self.said_of(k),
            other => cell_word(other),
        };
        let wraps = ronoctua_design::wrap(&text, cells.max(1), usize::from(most)).len();
        u16::try_from(wraps).unwrap_or(most).clamp(1, most.max(1))
    }

    /// How many cells a cell of a module's page comes out in — none for an
    /// empty one, and `blind` for a key the host has not said.
    fn cell_cells(&self, cell: &ScreenCell, blind: u16) -> u16 {
        match cell {
            ScreenCell::Empty => 0,
            ScreenCell::Key(k) if k.is_empty() => 0,
            ScreenCell::Key(k) => self.cells_of(k, blind),
            ScreenCell::Data(d) => ronoctua_design::wide(d),
            ScreenCell::Moment(t) => ronoctua_design::wide(&t.text),
            ScreenCell::Count(n) => ronoctua_design::wide(&n.to_string()),
            ScreenCell::Bytes(m) => ronoctua_design::wide(&m.text),
        }
    }

    /// How many lines a row of a module's page takes under itself.
    fn below_lines(&self, row: &ModuleRow, columns: u16) -> u16 {
        if says_nothing(&row.below) {
            0
        } else {
            self.wraps_to(&row.below, below_cells(columns), BELOW_MOST)
        }
    }

    /// How wide a section's columns come out: the widest of each among its
    /// rows. The label of a row with no value does not sit in the column
    /// ([`sentence_columns`]): it takes the value's room, and counts as one.
    fn section_cells(&self, section: &ModuleSection) -> ScreenCells {
        ScreenCells {
            label: section
                .rows
                .iter()
                .filter(|r| !says_nothing(&r.value))
                .map(|r| self.cell_cells(&r.label, LABEL_BLIND))
                .fold(0, u16::max),
            value: section
                .rows
                .iter()
                .map(|r| {
                    let held = if says_nothing(&r.value) {
                        &r.label
                    } else {
                        &r.value
                    };
                    self.cell_cells(held, VALUE_BLIND)
                })
                .fold(0, u16::max),
            note: section
                .rows
                .iter()
                .map(|r| self.cell_cells(&r.note, NOTE_BLIND))
                .fold(0, u16::max),
        }
    }
}

// ---------------------------------------------------------------------------
// A task's own page.
// ---------------------------------------------------------------------------

/// Which row of the block is being drawn, and what is true of it.
///
/// Three numbers that always travel together: the row, the cursor's own index
/// among the SPOTS (which is not the row — the list of rows has blanks and
/// headings in it), and whether the pointer is on this row. Passed one by one
/// they made three painters take eight arguments each, and eight arguments is
/// where a caller starts getting the order wrong.
#[derive(Debug, Clone, Copy)]
struct At {
    /// The row of the block.
    row: u16,
    /// Which spot this is, counted from the top of the page.
    spot: usize,
    /// Whose hand is on this row, if any — the cursor's colour number.
    hovered: Option<u16>,
}

impl At {
    /// Whose pointer is resting on this row, if any.
    const fn under_pointer(&self) -> Option<u16> {
        self.hovered
    }
}

/// Whose hand is on `row`, among every pointer's row — the first to arrive
/// on a shared row gives it its colour.
fn seat_on(hovered: &[(u16, u16)], row: u16) -> Option<u16> {
    hovered.iter().find(|(r, _)| *r == row).map(|(_, s)| *s)
}

/// The columns of a row that names a value and says which key changes it.
///
/// The unit sits immediately AFTER the number and not at the other edge of the
/// window. That is not a preference: it was at the other edge, and the owner
/// read `every` down the left of the screen and `days` down the right with a
/// bare `7` between them. A number and its unit are one phrase.
///
/// THE LABEL AND THE VALUE ARE AS WIDE AS WHAT THEY HOLD on this page, in the
/// person's language (`cells`, [`State::field_cells`]) — they were twenty and
/// twenty-four reserved, which cut a module's reason short in the `Next run`
/// row while the unit beside it had the rest of the line to itself.
fn field_columns(columns: u16, keys: u16, cells: (u16, u16)) -> Layout {
    let (label, value) = cells;
    Table::across(columns.saturating_sub(1))
        .with(Column::fixed("label", label, 2))
        .with(Column::fixed("value", value, 1))
        .with(Column::fill("unit", 6))
        // THE SHORTCUT: `⌥` against its one key, or — spelt, under the
        // pointer — `SUPER ALT` and the key, which `unit` gives the room for
        // (`ronoctua_design::leader_alt`).
        .with(Column::fixed("keys", keys, 3))
        .resolve()
}

impl State {
    /// Whether ANY pointer is resting on this cell of this block.
    fn hovering(&self, region: Region, row: u16, column: u16) -> bool {
        self.hover
            .first(|(at, line)| region.row_at(line) == Some(row) && at == column)
            .is_some()
    }

    /// WHOSE pointer is resting on this row of this block, between the two
    /// cells — a control a few cells wide — if any.
    fn pointed_in(&self, region: Region, row: u16, from: u16, to: u16) -> Option<u16> {
        self.hover
            .first(|(at, line)| region.row_at(line) == Some(row) && (from..to).contains(&at))
    }

    /// A field row's key, as it is reached: `⌥` and the one key the keymap
    /// gives the command `id`, spelt `label` while the pointer rests on it
    /// (`Row::put_shortcut`).
    fn taught_in(
        &self,
        row: &mut ronoctua_design::table::Row<'_, Wit>,
        id: &str,
        label: &str,
        spelt: bool,
    ) {
        if let Some(key) = key_of_command(id) {
            let _ = row.put_shortcut("keys", Icon::LeaderAlt, label, key, spelt, Role::Accent);
        }
    }
}

/// The key `LEADER Alt` gives the command `id` in this buffer — the one key of
/// its sequence in [`KEYMAP`] (`local_key`), so a row says the key that the
/// keymap binds and no other.
fn key_of_command(id: &str) -> Option<&'static str> {
    KEYMAP
        .iter()
        .find(|(_, command)| *command == id)
        .and_then(|(keys, _)| ronoctua_design::frame::local_key(keys))
}

/// What asks for one of the module's page actions: the row's own key.
const RET: &str = "RET";

/// How wide the column of the keys of the options is: `⌥` against its one key
/// (`label` is how that spells), and `RET`, which shares it so the eye reads
/// one column down the list.
fn key_cells(label: &str) -> u16 {
    ronoctua_design::leader_alt::cells(label, "x", false).max(ronoctua_design::wide(RET))
}

/// The columns of one of the module's page actions: what it is, and [`RET`]
/// in the column of the keys ([`key_cells`]) of the rows above it.
///
/// ITS LABEL IS NOT THE FIELDS': one that long in their column would push
/// their values out, with nothing between `Runs on its own` and its `Yes`.
/// So it fills, up to the key.
fn action_columns(columns: u16, keys: u16) -> Layout {
    Table::across(columns.saturating_sub(1))
        .with(Column::fill("label", 10))
        .with(Column::fixed("keys", keys, 3))
        .resolve()
}

/// What a label is given before anybody has said how wide its words come out
/// — the schedule's own reservation plus the air — and what a section with
/// longer labels may still count on when the row is short of room.
const LABEL_BLIND: u16 = 24;

/// And a note, by the same reasoning: what it counts on beside a value that
/// may want the whole row.
const NOTE_BLIND: u16 = 20;

/// The widest a measured label is given: past it the row is about the label.
const LABEL_MOST: u16 = 40;

/// What a value keeps of a row before a label past [`LABEL_BLIND`] may take
/// any more.
const VALUE_KEPT: u16 = 26;

/// The narrowest a value is drawn: short of it, the columns beside it give way.
const VALUE_LEAST: u16 = 10;

/// What a value is worth while the host has not said its words: all there is,
/// which leaves a note the reservation it always had.
const VALUE_BLIND: u16 = u16::MAX;

/// How wide the two measured columns of a section of a module's page come out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenCells {
    /// The widest label; `0` when no row of the section has one.
    label: u16,
    /// The widest value, or [`VALUE_BLIND`].
    value: u16,
    /// The widest note; `0` when no row of the section has one.
    note: u16,
}

/// A section laid out by the reservations, as a page with no words is.
const SCREEN_BLIND: ScreenCells = ScreenCells {
    label: LABEL_BLIND,
    value: VALUE_BLIND,
    note: NOTE_BLIND,
};

/// The columns of a row of the module's own page: what it is, its value, and
/// a quieter note.
///
/// # Every column as wide as what it holds, and the value takes the rest
///
/// A label of twenty-four and a note of twenty were RESERVED for every row of
/// every module, because a module's key is not one this side can ask the host
/// for at load. A journal's row is a date of sixteen and a count of four, so
/// twenty-four cells of a seventy-three-column window held nothing, and the
/// message beside them stopped at twenty-six. They are measured now, a section
/// at a time ([`State::section_cells`]) — a long line in one section squeezes
/// no other — and the reservations are what nothing measured is worth.
///
/// A measured label goes past [`LABEL_BLIND`] only while the value keeps
/// [`VALUE_KEPT`], and a note goes past [`NOTE_BLIND`] only into what the
/// widest value leaves empty. No column of the reservations goes missing at
/// any width, but the value is not always as wide as it was: a long label
/// takes cells from it, and so does a short note, which now stays at widths
/// where the reservation made it give way. A column with nothing in it is left
/// out. The note gives way first; it is also in the footer, whole.
fn screen_columns(columns: u16, cells: ScreenCells) -> Layout {
    use ronoctua_design::GAP;
    let usable = columns.saturating_sub(1);
    let note = cells.note.min(NOTE_BLIND);
    let room = usable.saturating_sub(note + 2 * GAP + VALUE_KEPT);
    let label = cells.label.min(LABEL_MOST).min(room.max(LABEL_BLIND));
    let taken = (label + 2 * GAP).saturating_add(cells.value.max(VALUE_LEAST));
    let note = cells.note.min(usable.saturating_sub(taken).max(note));
    let mut table = Table::across(usable);
    if label > 0 {
        table = table.with(Column::fixed("label", label, 2));
    }
    table = table.with(Column::fill("value", VALUE_LEAST));
    if note > 0 {
        table = table.with(Column::fixed("note", note, 3));
    }
    table.resolve()
}

/// The columns of a row with no value: its label takes the line up to the
/// note, `note` cells wide and `0` when it has none. In the columns of the
/// rows around it the label would be cut with the value's room blank beside
/// it — a section that has nothing to list says so in a whole sentence, and a
/// filter that caught nothing is a long snippet with nothing to show.
fn sentence_columns(columns: u16, note: u16) -> Layout {
    let mut table = Table::across(columns.saturating_sub(1)).with(Column::fill("label", 10));
    if note > 0 {
        table = table.with(Column::fixed("note", note, 3));
    }
    table.resolve()
}

/// Whether a cell of a module's row says nothing: a module fills a cell from
/// what it holds, and what it holds can be an empty text.
fn says_nothing(cell: &ScreenCell) -> bool {
    match cell {
        ScreenCell::Empty => true,
        ScreenCell::Key(text) | ScreenCell::Data(text) => text.is_empty(),
        ScreenCell::Moment(_) | ScreenCell::Count(_) | ScreenCell::Bytes(_) => false,
    }
}

/// Where the lines under a row of a module's page begin: under its label, in
/// by what a row of a list is in from its glyph.
const BELOW_AT: u16 = ronoctua_design::ICON + ronoctua_design::GAP;

/// The most lines those are given on the page. A text past it is the
/// person's own, and has a bar of its own ([`State::below_at`]).
const BELOW_MOST: u16 = 4;

/// And in the footer, whose rows the row's facts need too. The page has the
/// whole of it, right under the row.
const BELOW_FOOT: u16 = 2;

/// How wide those lines are in a window `columns` wide: what the page's bar
/// and [`BELOW_AT`] leave, the width the host wraps them at.
const fn below_cells(columns: u16) -> u16 {
    columns.saturating_sub(1).saturating_sub(BELOW_AT)
}

/// The columns of a step.
fn step_columns(columns: u16) -> Layout {
    Table::across(columns.saturating_sub(1))
        .with(Column::fixed("mark", ronoctua_design::ICON, 3))
        .with(Column::fill("name", 14))
        .with(Column::fixed("note", 14, 1))
        .resolve()
}

/// The columns of one run in the history.
///
/// Three columns that add up, with no fourth trailing off the edge. What the
/// row cannot hold — what the machine said, which step it stopped at — is in
/// the footer, whole, and that is what the footer is for.
///
/// The outcome is as wide as the widest outcome comes out in the person's
/// language (`outcome`), and the reason takes the rest.
fn history_columns(columns: u16, outcome: u16) -> Layout {
    Table::across(columns.saturating_sub(1))
        .with(Column::fixed("when", WHEN_FLOOR, 2))
        .with(Column::fixed("outcome", outcome, 1))
        .with(Column::fill("why", 10))
        .resolve()
}

impl State {
    /// Everything the cursor can rest on, in the order it is drawn.
    ///
    /// One list, built the same way the page is drawn, so the cursor cannot
    /// come to rest on a row that is not there. A task with no steps has no
    /// step spots, which is also why its page draws no `Steps` heading — the
    /// owner found one over nothing at all.
    fn spots(&self) -> Vec<Spot> {
        let Some(task) = self.here() else {
            return Vec::new();
        };
        let mut out = Self::schedule_of(task);
        out.extend(Self::page_actions(task).map(Spot::Action));
        for shown in self.shown_sections(task) {
            if shown.folds {
                out.push(Spot::Section(shown.section));
            }
            if shown.open {
                out.extend((shown.from..shown.from + shown.rows).map(Spot::Screen));
            }
        }
        out.extend((0..steps_shown(task)).map(Spot::Step));
        out.extend((0..task.history.len()).map(Spot::Ran));
        out
    }

    /// The schedule's rows a task HAS, in order.
    ///
    /// # Why they are not the same six for every task any more
    ///
    /// Because a relief that comes due when the disk is short keeps no
    /// calendar, and its page said "every 7 days, afternoon" about it — a
    /// cadence it does not keep and a key that changed a number nothing reads.
    /// The cadence and the window are drawn only for a task that keeps a
    /// calendar. And a module that only LISTENS runs nothing: it has no next
    /// moment to show, nothing to run now and nothing to rehearse — what it
    /// has is whether it listens at all.
    fn schedule_of(task: &TaskState) -> Vec<Spot> {
        let mut out = vec![Spot::Enabled];
        // SWITCHED OFF BY THE SYSTEM: the one row that says so, with its
        // reason and no key. A cadence, a window, a next moment, a run and a
        // rehearsal would be five rows the brain refuses.
        if off_by_the_system(task) {
            return out;
        }
        if task.calendar && task.kind != TaskKind::Watch {
            out.extend([Spot::Interval, Spot::Window]);
        }
        if task.kind != TaskKind::Watch {
            out.extend([Spot::Next, Spot::RunNow, Spot::Rehearse]);
        }
        out
    }

    /// The actions a task's module offers on its PAGE, by their index in
    /// `TaskState::actions` — those that act on a row are the rows' own.
    fn page_actions(task: &TaskState) -> impl Iterator<Item = usize> + '_ {
        // NONE for a module the system switched off: the brain refuses them.
        let offered = !off_by_the_system(task);
        task.actions
            .iter()
            .enumerate()
            .filter(move |(_, a)| offered && !a.row)
            .map(|(n, _)| n)
    }

    /// The module's page, when the one held is THIS task's.
    fn page_of(&self, task: &TaskState) -> Option<&ModuleScreen> {
        self.screen.as_ref().filter(|sc| sc.module == task.id)
    }

    /// The rows of the module's own page for THIS task, counted across its
    /// sections — none while the host has not pushed one for it.
    fn screen_rows(&self, task: &TaskState) -> Vec<&ModuleRow> {
        self.page_of(task)
            .map(|sc| sc.sections.iter().flat_map(|s| s.rows.iter()).collect())
            .unwrap_or_default()
    }

    /// The sections of the module's page that have rows, in order: the ONE
    /// walk [`Self::spots`] and [`Self::rows_of`] both read, so the cursor
    /// cannot come to rest on a row a shut section does not draw. A section
    /// with no rows is a heading over nothing, and is left out.
    fn shown_sections(&self, task: &TaskState) -> Vec<Shown> {
        let mut out = Vec::new();
        let mut from = 0;
        let Some(page) = self.page_of(task) else {
            return out;
        };
        for (n, section) in page.sections.iter().enumerate() {
            let rows = section.rows.len();
            if rows > 0 {
                let folds = !matches!(section.fold, SectionFold::Never);
                out.push(Shown {
                    section: n,
                    from,
                    rows,
                    folds,
                    open: !folds || section_open(task, section),
                });
            }
            from += rows;
        }
        out
    }

    /// The columns each row of the module's page is drawn in, by its index
    /// across the page's sections.
    fn screen_layouts(&self, task: &TaskState, columns: u16) -> Vec<Layout> {
        let sections = self.page_of(task).map_or(&[][..], |sc| &sc.sections);
        // A PAGE THAT CAME WITH NO WORDS is laid out as it always was: a host
        // that sends none draws the page it drew before they crossed.
        let blind = self.page_phrases.is_empty();
        let mut out = Vec::new();
        for section in sections {
            let cells = if blind {
                SCREEN_BLIND
            } else {
                self.section_cells(section)
            };
            let layout = screen_columns(columns, cells);
            let note = layout
                .field("note")
                .map_or(cells.note.min(NOTE_BLIND), |f| f.width);
            out.extend(section.rows.iter().map(|row| {
                if blind || !says_nothing(&row.value) {
                    layout.clone()
                } else if says_nothing(&row.note) {
                    sentence_columns(columns, 0)
                } else {
                    sentence_columns(columns, note)
                }
            }));
        }
        out
    }

    /// The row of the module's page under the cursor, by its index across
    /// the page's sections, when it offers a SECOND action (`also`) — on a
    /// module's page, and in a module the system did not switch off.
    fn also_here(&self) -> Option<usize> {
        if self.page != Page::Task {
            return None;
        }
        let Some(Spot::Screen(n)) = self.spot_here() else {
            return None;
        };
        let task = self.here().filter(|t| !off_by_the_system(t))?;
        self.screen_rows(task)
            .get(n)
            .filter(|r| !r.also.is_empty())
            .map(|_| n)
    }

    /// The spot under the cursor.
    fn spot_here(&self) -> Option<Spot> {
        self.spots().get(self.spot).copied()
    }
}

/// One row of a task's page, before it is placed.
///
/// The page is built as a list and then a window of it is drawn. That is what
/// gives it a scrollbar without any of the three blocks having to know how tall
/// the window is — and it is why the cursor cannot land somewhere that is not
/// drawn: the same list answers both questions.
enum Row {
    Blank,
    /// THE TASK'S NAME AND THE WAY BACK, atop its page while the window wears
    /// no header — the two things the header said, and nothing else on the
    /// screen says then.
    Title,
    Heading(&'static str),
    /// A section of the module's own page, by its index: its heading is the
    /// module's key, which this side does not hold as a constant.
    Section(usize),
    /// The same for a section that folds: a title the cursor stops on, its
    /// triangle and how many it holds.
    Fold(usize),
    /// What the colours of the rows below stand for, one line, when the
    /// module said. It only tells, so the cursor does not stop on it and the
    /// pointer lights nothing.
    Legend,
    /// The description, which the host wraps. The number is how many rows it
    /// was given.
    About(String, u16),
    /// Why the module's page is not here — the sentence the host made of the
    /// brain's refusal, wrapped like the description, in the rows' place.
    Refused(u16),
    Field(Spot),
    /// One of the module's page actions, by its index in its `actions`.
    Action(usize),
    /// One row of the module's own page, counted across its sections, and
    /// how many lines it takes with what it says under itself.
    Screen(usize, u16),
    Step(usize),
    Ran(usize),
}

/// Whether a row of the page is one the cursor stops on.
const fn is_a_spot(row: &Row) -> bool {
    matches!(
        row,
        Row::Field(_)
            | Row::Action(_)
            | Row::Fold(_)
            | Row::Screen(_, _)
            | Row::Step(_)
            | Row::Ran(_)
    )
}

/// How many LINES OF THE SCREEN one row of the page takes.
///
/// # Why this had to become a function
///
/// Because the description takes three and everything else takes one, and the
/// window was measured in ROWS OF THE LIST. The two counts are off by exactly
/// `about_rows − 1`, which is two: the owner moved the cursor from the first
/// entry of the history to the second, the second was selected and the page did
/// not scroll — nor on the third — and only on the fourth did it move. Two.
///
/// Everything that asks "how much of this page is on the screen" asks here now:
/// the window, the follow, the scrollbar and the click.
const fn row_lines(row: &Row) -> u16 {
    match row {
        Row::About(_, want) | Row::Refused(want) | Row::Screen(_, want) => *want,
        Row::Blank
        | Row::Title
        | Row::Heading(_)
        | Row::Section(_)
        | Row::Fold(_)
        | Row::Legend
        | Row::Field(_)
        | Row::Action(_)
        | Row::Step(_)
        | Row::Ran(_) => 1,
    }
}

/// How many screen lines the rows before `n` take.
fn lines_before(rows: &[Row], n: usize) -> u16 {
    rows.iter().take(n).map(row_lines).sum()
}

/// How many rows, starting at `from`, fit in a window `height` lines tall.
///
/// A row goes in WHOLE or not at all: half a description is a paragraph cut
/// where the block ends, and the pen would refuse the rest of it anyway.
fn fit_from(rows: &[Row], from: usize, height: u16) -> usize {
    let mut used = 0;
    let mut many = 0;
    for row in rows.iter().skip(from) {
        let tall = row_lines(row);
        if used + tall > height {
            break;
        }
        used += tall;
        many += 1;
    }
    many
}

/// The furthest the window may start and still end with the list.
///
/// THE LAST WINDOW, never the last row: a list scrolled to its end shows its
/// final screenful, and stopping at `len − 1` leaves a page of empty ground
/// under one line of list.
fn last_window(rows: &[Row], height: u16) -> usize {
    let mut from = rows.len();
    let mut used = 0;
    while from > 0 {
        let tall = row_lines(&rows[from - 1]);
        if used + tall > height {
            break;
        }
        used += tall;
        from -= 1;
    }
    from
}

/// Which row of the list is drawn on screen row `at`, counting from `from`.
///
/// The same walk the drawing makes, which is what keeps a click and the band
/// on the same thing: the description covers three screen rows and is ONE row
/// of the list, so a click resolved by `from + at` lands two rows late.
fn row_at_line(rows: &[Row], from: usize, at: u16) -> Option<usize> {
    let mut line = 0;
    for (n, row) in rows.iter().enumerate().skip(from) {
        let tall = row_lines(row);
        if at < line + tall {
            return Some(n);
        }
        line += tall;
    }
    None
}

impl State {
    /// The rows of the page in front, or none when there is no task.
    fn page_rows(&self) -> Vec<Row> {
        self.here()
            .map(|t| self.rows_of(t, self.columns))
            .unwrap_or_default()
    }

    /// A task's page, as a list of rows.
    fn rows_of(&self, task: &TaskState, columns: u16) -> Vec<Row> {
        // WHAT IT DOES, first and in the person's own language. It was on the
        // home page only, as a dim line under the cursor — so the page ABOUT a
        // task was the one place that did not say what the task was.
        //
        // Three rows: enough for every description in the table at eighty
        // columns, and the host elides rather than dropping the rest.
        //
        // The schedule's rows are FIXED and every task has all of them, which
        // is why they are written as a list rather than pushed one by one —
        // `spots()` says the same six in the same order, and the two lists
        // reading alike is what keeps the cursor on a row that exists.
        let mut out = Vec::new();
        // WITH NO HEADER, THE PAGE SAYS WHOSE IT IS and leads back: the
        // owner's *"quero que apareça uma linha nova no topo da página da
        // tarefa, antes do paragrafo descritivo e separado por uma linha de
        // espaço em branco"*.
        if !self.headed {
            out.push(Row::Title);
            out.push(Row::Blank);
        }
        // A DESCRIPTION ONLY WHEN THE MODULE DECLARED ONE: an empty key is a
        // key the host did not have, and three rows of brackets say nothing.
        if let Some(does) = key_of(&task.does_key) {
            out.push(Row::About(does, self.about_lines(&task.does_text, columns)));
            out.push(Row::Blank);
        }
        // ONE TITLE AND ONE LIST: what can be set about a task and what its
        // module offers are both rows `RET` acts on, and a title over each
        // was two lists of a row or two.
        out.push(Row::Heading(key::OPTIONS));
        out.extend(Self::schedule_of(task).into_iter().map(Row::Field));
        out.extend(Self::page_actions(task).map(Row::Action));
        // AND THE MODULE'S OWN PAGE, section by section, as the host last
        // pushed it: rows of DATA every module's page is drawn with the same
        // way.
        if let Some(sc) = self.page_of(task) {
            // A PAGE THE BRAIN WOULD NOT GIVE SAYS SO where its rows would
            // be: a refused page and an empty one must not look the same.
            if !sc.refused.is_empty() {
                out.push(Row::Blank);
                out.push(Row::Refused(self.about_lines(&sc.refused, columns)));
            }
            let shown = self.shown_sections(task);
            // WHAT THE COLOURS MEAN, above the first list that wears them,
            // and nothing at all with no colours to explain or no list.
            if !sc.legend.is_empty() && !shown.is_empty() {
                out.push(Row::Blank);
                out.push(Row::Legend);
            }
            let rows = self.screen_rows(task);
            for section in shown {
                out.push(Row::Blank);
                out.push(if section.folds {
                    Row::Fold(section.section)
                } else {
                    Row::Section(section.section)
                });
                if section.open {
                    out.extend((section.from..section.from + section.rows).map(|n| {
                        let under = rows.get(n).map_or(0, |r| self.below_lines(r, columns));
                        Row::Screen(n, 1 + under)
                    }));
                }
            }
        }
        // THE HEADING ONLY WHEN THERE IS SOMETHING UNDER IT. Two of the four
        // tasks name a unit and declare no steps, and both of them drew this
        // title over nothing.
        if steps_shown(task) > 0 {
            out.push(Row::Blank);
            out.push(Row::Heading(key::STEPS));
            out.extend((0..steps_shown(task)).map(Row::Step));
        }
        // A MODULE THAT ONLY LISTENS has no runs, and a `History` heading
        // over nothing is the heading the owner found over no steps: it goes
        // until it has something under it.
        if task.kind == TaskKind::Watch && task.history.is_empty() {
            return out;
        }
        out.push(Row::Blank);
        out.push(Row::Heading(key::HISTORY));
        if task.history.is_empty() {
            out.push(Row::Blank);
        } else {
            out.extend((0..task.history.len()).map(Row::Ran));
        }
        out
    }

    /// Which SPOT a row of the page is, if it is one at all.
    ///
    /// The inverse of [`Self::row_of_spot`], and the one place the conversion
    /// is made in this direction. The list of rows has blanks and headings in
    /// it and the list of spots does not, so a click resolved by the row's own
    /// index lands on a different thing from the one under the pointer — the
    /// defect `the_band_and_the_footer_agree` names on the keyboard's side.
    fn spot_of_row(rows: &[Row], n: usize) -> Option<usize> {
        if !rows.get(n).is_some_and(is_a_spot) {
            return None;
        }
        Some(rows.iter().take(n).filter(|r| is_a_spot(r)).count())
    }

    /// The row a spot is drawn on, in the list above.
    ///
    /// The window is measured in ROWS — blanks and headings included, because
    /// they take a line each — and the cursor is measured in SPOTS. This is
    /// the one place the two are converted, and it is why the conversion is a
    /// function rather than an index used twice.
    fn row_of_spot(rows: &[Row], spot: Spot) -> Option<usize> {
        rows.iter().position(|r| match r {
            Row::Field(s) => *s == spot,
            Row::Action(n) => spot == Spot::Action(*n),
            Row::Fold(n) => spot == Spot::Section(*n),
            Row::Screen(n, _) => spot == Spot::Screen(*n),
            Row::Step(n) => spot == Spot::Step(*n),
            Row::Ran(n) => spot == Spot::Ran(*n),
            Row::Blank
            | Row::Title
            | Row::Heading(_)
            | Row::Section(_)
            | Row::Legend
            | Row::About(_, _)
            | Row::Refused(_) => false,
        })
    }

    /// The two numbers beside the `Steps` heading — how many of them are on,
    /// of how many — when it is that heading.
    ///
    /// `History` says something else beside it: how far back it reaches and
    /// how many runs that holds ([`put_reach`]), the shape the `Activity`
    /// container's title has one screen away. The reach is the DAEMON's
    /// (`journal::RECENT_DAYS`, said in the mind), not a number written into
    /// two translations.
    fn count_beside(task: &TaskState, heading: &str) -> Option<String> {
        if heading == key::STEPS {
            let on = task.steps.iter().filter(|s| s.on).count();
            return Some(format!("{on} / {}", task.steps.len()));
        }
        None
    }

    /// A task's page, drawn in the body's block.
    fn task_page(&mut self, screen: &mut Screen<Wit>) {
        let block = self.blocks.body;
        self.page_cut = None;
        let Some(task) = self.here().cloned() else {
            let mut pen = screen.region(block);
            if !pen.empty() {
                pen.put(0, Run::label(key::NOTHING).quiet());
            }
            return;
        };
        let columns = self.columns;
        let rows = self.rows_of(&task, columns);
        // A ROW OF THE LIST IS NOT A ROW OF THE SCREEN — the description takes
        // three — and EVERY question about this window is asked in screen
        // lines: see [`row_lines`]. Asked in list rows, the window was two
        // short, and the owner watched the cursor step onto the second and
        // third entries of the history without the page moving at all.
        let height = block.lines.max(1);
        // ONLY WHEN A PERSON MOVED THE CURSOR; see [`State::follows`]. The
        // wheel scrolls this page and leaves the cursor where it is, and a
        // follow that did not ask would put the window back on the next frame.
        if self.follows
            && let Some(spot) = self.spot_here()
            && let Some(at) = Self::row_of_spot(&rows, spot)
        {
            if at < self.body_from {
                self.body_from = at;
            }
            // AND IT FOLLOWS BY THE SHORTEST MOVE, one row at a time, because
            // the rows are not the same height: subtracting a count of rows
            // from the cursor's index is an arithmetic that only works where
            // every row is one line.
            while at >= self.body_from + fit_from(&rows, self.body_from, height).max(1) {
                self.body_from += 1;
            }
        }
        self.body_from = self.body_from.min(last_window(&rows, height));

        let steps = step_columns(columns);
        let history = history_columns(columns, self.outcome_cells());
        // EVERY ROW OF THIS PAGE STOPS ONE CELL SHORT, because the bar is
        // there. The three layouts above already do it; these are the runs
        // that are placed by hand.
        let body = columns.saturating_sub(1);
        let hovered = self.hovered_rows(block);
        let mut pen = screen.region(block);
        if pen.empty() {
            return;
        }
        // THE LANE THE BAR TAKES; see the same line in the activity log — and
        // only while there is a bar, which is the page not fitting its block.
        pen.keeping_for(lines_before(&rows, rows.len()) > height);
        let last = pen.lines();
        let mut line = 0;
        // THE SPOT INDEX IS NOT THE ROW INDEX, and conflating them put the
        // band on one thing and the footer on another: the list of rows has
        // blanks and headings in it and the list of spots does not.
        // [`Self::spot_of_row`] is the one place that conversion is made, and
        // the click asks the same one.
        let mut spot_n = Self::spot_of_row(&rows, self.body_from).unwrap_or_else(|| {
            rows.iter()
                .take(self.body_from)
                .filter(|r| is_a_spot(r))
                .count()
        });
        let actions = action_columns(
            columns,
            key_cells(&self.phrases.or(Phrases::LEADER_ALT, "")),
        );
        let screen_layouts = self.screen_layouts(&task, columns);
        let screen_rows: Vec<ModuleRow> = self.screen_rows(&task).into_iter().cloned().collect();
        let sections_shown = self.shown_sections(&task);
        let page = self.page_of(&task).cloned();
        let sections = page.as_ref().map_or(&[][..], |sc| &sc.sections);
        let legend = page.as_ref().map_or(&[][..], |sc| &sc.legend);
        let refused = page.as_ref().map_or("", |sc| &sc.refused);
        for row in rows.iter().skip(self.body_from) {
            if line >= last {
                break;
            }
            match row {
                Row::Blank => line += 1,
                // THE TASK'S NAME, AND THE WAY BACK flush right — the same
                // shape as the footer's click, in the same tone, lit by a
                // ground under it alone: it is a control, not a row.
                Row::Title => {
                    let cells = self.cells_of(key::BACK, FACT_CELLS).min(body);
                    let from = body.saturating_sub(cells);
                    if let Some(seat) = self.pointed_in(block, line, from, body) {
                        pen.ground_under(line, from, cells, Role::hover_of(seat));
                    }
                    pen.put(line, Run::label(key::BACK).accent().at(from).within(cells));
                    self.back_at = Some((line, from, body));
                    if let Some(name) = key_of(&task.name_key) {
                        let room = from.saturating_sub(ronoctua_design::GAP);
                        pen.put(line, Run::label(&name).title().within(room));
                    }
                    line += 1;
                }
                Row::Heading(k) => {
                    // WITHIN, AND NOT TO THE EDGE. The bar down the right of
                    // this page owns the last cell, and a heading with no
                    // budget reaches it — "barra nenhuma desenha por cima do
                    // conteúdo" cuts both ways, and the side that gives is the
                    // text.
                    // THE HEADING STOPS WHERE THE COUNT BEGINS, when there is
                    // one. A key reserves its whole budget — nothing here can
                    // know how long the word comes out — so a heading that
                    // reached the edge would reserve the count's cells too.
                    let count = Self::count_beside(&task, k);
                    let history = *k == key::HISTORY;
                    let room = if count.is_some() || history {
                        COUNT_AT.saturating_sub(ronoctua_design::GAP)
                    } else {
                        body
                    };
                    pen.put(line, Run::label(k).title().within(room));
                    // AND THE COUNT BESIDE IT. A datum: it is two numbers,
                    // and a number is not translated.
                    if let Some(count) = count {
                        pen.put(
                            line,
                            Run::datum(&count)
                                .quiet()
                                .small()
                                .at(COUNT_AT)
                                .within(body.saturating_sub(COUNT_AT)),
                        );
                    }
                    // AND THE HISTORY'S REACH AND HOW MANY RUNS IT HOLDS —
                    // `last 20 days · 4`, the owner's twenty days in place
                    // of the last ten runs.
                    if history {
                        let reach = self.mind.history_reach.clone();
                        put_reach(&mut pen, line, body, &reach, task.history.len());
                    }
                    line += 1;
                }
                Row::Section(n) => {
                    if let Some(heading) = sections.get(*n).and_then(|s| key_of(&s.heading)) {
                        pen.put(line, Run::label(&heading).title().within(body));
                    }
                    line += 1;
                }
                Row::Fold(n) => {
                    let at = At {
                        row: line,
                        spot: spot_n,
                        hovered: seat_on(&hovered, line),
                    };
                    let open = sections_shown.iter().any(|s| s.section == *n && s.open);
                    if let Some(section) = sections.get(*n) {
                        self.fold_title(&mut pen, section, &at, open, body);
                    }
                    spot_n += 1;
                    line += 1;
                }
                Row::Legend => {
                    self.draw_legend(&mut pen, legend, line, body);
                    line += 1;
                }
                // THE DESCRIPTION, from the row its own bar was left at: in a
                // window too small for all of it ([`State::about_lines`]), the
                // host draws a thumb beside it and answers its drag and wheel
                // with the row (`Input::Scrolled`), which lands here.
                Row::About(k, want) => {
                    let room = (*want).min(last.saturating_sub(line));
                    if room > 0 {
                        pen.paragraph_from(
                            line,
                            room,
                            self.about_scroll,
                            Run::label(k).quiet().within(body),
                        );
                        self.about_at = Some((line, body.saturating_sub(1)));
                    }
                    line += room;
                }
                // DATA: the host already made the sentence, in the person's
                // language, out of what the brain said.
                Row::Refused(want) => {
                    let room = (*want).min(last.saturating_sub(line));
                    if room > 0 {
                        pen.paragraph(
                            line,
                            room,
                            Run::datum(refused).playing(Role::Body).within(body),
                        );
                    }
                    line += room;
                }
                Row::Action(n) => {
                    let at = At {
                        row: line,
                        spot: spot_n,
                        hovered: seat_on(&hovered, line),
                    };
                    self.action(&mut pen, &task, &at, &actions, *n);
                    spot_n += 1;
                    line += 1;
                }
                // A ROW GOES WHOLE: with the window ending under its first
                // line, what it says under itself is not begun — and what is
                // left of the window under that line is nobody's.
                Row::Screen(n, want) => {
                    let lines = if line + want <= last { *want } else { 1 };
                    if lines < *want {
                        self.page_cut = Some(line + lines);
                    }
                    let at = At {
                        row: line,
                        spot: spot_n,
                        hovered: (line..line + lines).find_map(|l| seat_on(&hovered, l)),
                    };
                    if let (Some(row), Some(layout)) = (screen_rows.get(*n), screen_layouts.get(*n))
                    {
                        self.screen_row(&mut pen, row, &at, layout, lines);
                        if lines > 1 {
                            let cell = body.saturating_sub(1);
                            self.below_at.push((line + 1, cell, row.id.clone()));
                        }
                    }
                    spot_n += 1;
                    line += *want;
                }
                Row::Field(spot) => {
                    let at = At {
                        row: line,
                        spot: spot_n,
                        hovered: seat_on(&hovered, line),
                    };
                    self.field(&mut pen, &task, &at, columns, *spot);
                    spot_n += 1;
                    line += 1;
                }
                Row::Step(n) => {
                    let at = At {
                        row: line,
                        spot: spot_n,
                        hovered: seat_on(&hovered, line),
                    };
                    self.step(&mut pen, &task, &at, &steps, *n);
                    spot_n += 1;
                    line += 1;
                }
                Row::Ran(n) => {
                    let at = At {
                        row: line,
                        spot: spot_n,
                        hovered: seat_on(&hovered, line),
                    };
                    self.ran(&mut pen, &task, &at, &history, *n);
                    spot_n += 1;
                    line += 1;
                }
            }
        }
        // AND THE BAR COUNTS LINES TOO. It counted list rows, which made a
        // three-line description one row of the track: the thumb said the page
        // was shorter than it is and moved faster than the page did.
        pen.bar(
            columns.saturating_sub(1),
            Scroll {
                total: usize::from(lines_before(&rows, rows.len())),
                lines: usize::from(height),
                offset: usize::from(lines_before(&rows, self.body_from)),
                top: 0,
            },
        );
    }
}

/// Whether a pair whose notation is `keys` does anything when it is clicked
/// on the host's header: the three keys the page handles itself, and whatever
/// the keymap binds. A pair that is none of those is drawn inert, so no pair
/// looks like a control and is not one.
fn is_a_control(keys: &str) -> bool {
    matches!(keys, "RET" | "ESC" | "TAB") || KEYMAP.iter().any(|(k, _)| *k == keys)
}

/// The footer's fact slots — a label and a value each.
///
/// A LADDER OF STATIC IDS and not names built per frame: a `Column` takes a
/// `&'static str`, and the obvious way to get one from a loop is to leak a
/// `String`, which in a function called on every frame is a leak that grows
/// for as long as the window is open.
///
/// Four, because that is the most any footer on this screen has to say at
/// once. A fifth fact is dropped, and `Layout::has` is how the drawing finds
/// that out — which is the same answer a column that gave way gives.
const FACT_SLOTS: [(&str, &str); 4] = [("l0", "v0"), ("l1", "v1"), ("l2", "v2"), ("l3", "v3")];

/// The footer's facts, laid out left to right.
///
/// A pair that does not fit gives way WHOLE, and the ones on the right give
/// way first — the rank rises along the row, and the first fact is the one the
/// caller thought most worth saying. Every column is FIXED: exactly one may
/// fill, and the version of this screen that built five drew nothing at all.
fn facts_columns(columns: u16, wide: &[u16]) -> Layout {
    let mut table = Table::across(columns);
    for (n, (label, value)) in FACT_SLOTS.iter().enumerate().take(wide.len()) {
        let rank = u8::try_from(n + 1).unwrap_or(u8::MAX);
        table = table
            // MEASURED, and FOURTEEN only for whoever was not measured.
            //
            // The label is a key, so this side cannot read it — but the host
            // hands these three over with the header's phrases
            // (`Input::Phrases`), for the same reason and by the same door:
            // the number is the arithmetic this side cannot do. Reserved
            // blind, `Last run` took fourteen cells for eight, and the value
            // began six cells past the end of the word naming it.
            //
            // The floor is the old reservation and it belongs to the longest
            // translation: `brain-needs-root` is `Roda como root`, fourteen
            // cells. A budget sized to English is the defect the listing's
            // `note` column had.
            .with(Column::fixed(label, wide[n], rank))
            .with(Column::fixed(value, 18, rank));
    }
    table.resolve()
}

/// What a fact's label is worth, measured, or the reservation when nobody said.
const FACT_CELLS: u16 = 14;

/// Where a count beside a heading begins.
///
/// A CONSTANT because two places need it on each of the two headings that have
/// one — the count is drawn there, and the heading's budget has to stop short
/// of it. A key reserves the whole of what it is given (nothing on this side
/// knows how long a translated word comes out), so a heading that reached the
/// edge would reserve the count's cells as well.
///
/// ONE constant for `Steps  6 / 12`, `History  last 20 days · 4` and
/// `Activity  last 15 days · 55`: the three are the same kind of thing said
/// about three lists, and in three columns they would read as three
/// different kinds of number.
const COUNT_AT: u16 = 20;

/// How many rows a description is given.
///
/// Three at any width worth drawing, and FOUR when the window is narrow — a
/// sentence that fits three lines at a hundred columns needs a fourth at
/// fifty. The host elides the last, so this is a ceiling and not a promise.
///
/// The comment here used to say "two when narrow" while the code returned
/// four, which is a comment asserting the opposite of what it names.
const fn about_rows(columns: u16) -> u16 {
    if columns >= 60 { 3 } else { 4 }
}

/// The most rows a description is wrapped into while it is counted — a
/// bound on the work, far past any description a module writes.
const ABOUT_MOST: usize = 200;

impl State {
    /// HOW MANY ROWS A PARAGRAPH OF A TASK'S PAGE IS GIVEN: as many as it
    /// wraps to at the page's width, counted with the host's own wrap
    /// ([`ronoctua_design::wrap`]) — the whole of it, so it is read without a
    /// bar. It was three rows, and the owner photographed a bar over a
    /// description cut at its third row in a window with half its height
    /// left: *"não deveria existir em janelas de tamanho normal, pois o texto
    /// deveria se expandir totalmente na página"*.
    ///
    /// AND IN A WINDOW TOO SMALL FOR THAT, half of the page: the rest is
    /// reached by the paragraph's own bar (`State::about_scroll`), which is
    /// what makes sense there, in his words — *"em janelas bem pequenas, faz
    /// todo sentido que o tamanho do container do texto passe a ser
    /// limitado"*.
    ///
    /// `text` is the paragraph in the person's language, as the host made it
    /// (`does-text`); without one, the old reservation.
    fn about_lines(&self, text: &str, columns: u16) -> u16 {
        let floor = about_rows(columns);
        if text.is_empty() {
            return floor;
        }
        // THE PAGE'S WIDTH, one cell short for its bar — the `body` every run
        // of the page is placed within, and the width the host wraps at.
        let wraps = ronoctua_design::wrap(text, columns.saturating_sub(1), ABOUT_MOST).len();
        let wanted = u16::try_from(wraps).unwrap_or(u16::MAX).max(1);
        let half = self.blocks.body.lines / 2;
        wanted.min(half.max(floor))
    }
}

impl State {
    /// One row of the schedule: a label, a value, and the key that changes it.
    ///
    /// # Why every row says its own key
    ///
    /// Because the owner asked twice for a way to run a task by hand and to
    /// switch one off, and both times the answer was that the keys existed —
    /// `m r` and `m o` — and neither appeared anywhere on the page. A key
    /// nobody can see is a key nobody has. The header teaches them once; this
    /// says it again beside the thing it changes, which is where somebody
    /// looks.
    fn field(&self, pen: &mut Pen<'_, Wit>, task: &TaskState, at: &At, columns: u16, spot: Spot) {
        let line = at.row;
        let n = at.spot;
        if self.spot == n {
            pen.band(line);
        } else if let Some(seat) = at.under_pointer() {
            pen.ground(line, Role::hover_of(seat));
        }
        // THE ROW'S `⌥`, SPELT while a pointer rests on it — asked of where the
        // glyph is AT REST, so the spelling stays put under the hand that
        // asked for it — and the column that fills gives the room up.
        let label = self.phrases.or(Phrases::LEADER_ALT, "");
        let cells = self.field_cells(task, columns);
        let rest = field_columns(columns, key_cells(&label), cells);
        let spelt = !label.is_empty()
            && rest
                .field("keys")
                .is_some_and(|keys| self.hovering(self.blocks.body, line, keys.at));
        let layout = if spelt {
            field_columns(
                columns,
                ronoctua_design::leader_alt::cells(&label, "x", true),
                cells,
            )
        } else {
            rest
        };
        let Some(mut row) = pen.row_of(line, &layout) else {
            return;
        };
        // WHAT SITS AGAINST THE VALUE, which comes in `value` cells: where it
        // begins, and how far it may go.
        let against = |value: u16| {
            layout.field("value").map(|f| {
                let at = f.at + value.min(f.width) + ronoctua_design::GAP;
                // AND IT STOPS BEFORE THE KEY — its glyph first. A run placed by hand
                // inherits no budget, and with none it reaches the edge —
                // straight through the column that says which key changes
                // this row, and then through the bar beyond it.
                let until = layout
                    .field("alt")
                    .or_else(|| layout.field("keys"))
                    .map_or_else(|| layout.ends(), |k| k.at)
                    .saturating_sub(ronoctua_design::GAP);
                (at, until.saturating_sub(at).max(1))
            })
        };
        let mut after: Option<(&'static str, Role, (u16, u16))> = None;
        let mut held = None;
        match spot {
            Spot::Enabled => {
                // A SETTING: its label wears the tone of a thing that can be
                // changed (`Role::LabelSetting`) — the four standards of a row.
                row.put("label", Run::label(enabled_key(task.kind)).names(true));
                // OFF FOR A REASON THAT IS NOT THE PERSON'S — the system's
                // configuration switched the module off — is said with that
                // reason and offers no key: the brain refuses the switch, and
                // "only when you ask" would promise a run it refuses too.
                if let Some(why) = off_not_by_you(task) {
                    row.put("value", Run::label(&why).quiet());
                } else {
                    let (answer, run) = if task.enabled {
                        (key::ON, Run::label(key::ON).quiet())
                    } else {
                        let off = off_key(task.kind);
                        (off, Run::label(off).accent())
                    };
                    self.taught_in(&mut row, "brain-switch", &label, spelt);
                    // AND WHAT THE SWITCH IS FOR, against the answer: it goes
                    // whole or not at all, because half of a sentence in
                    // brackets says less than none of it.
                    let hint = enabled_hint_key(task.kind);
                    let said = self.cells_of(answer, cells.1);
                    after = against(said)
                        .filter(|(_, room)| self.cells_of(hint, *room) <= *room)
                        .map(|place| (hint, Role::Dim, place));
                    // THE ANSWER IS HELD TO ITS OWN WIDTH while the hint sits
                    // against it. The column would hand it the hint's cells
                    // too, and the host cuts a key at the width it is given,
                    // never at its ink: a longer word would run under the hint.
                    match (after, layout.field("value")) {
                        (Some(_), Some(f)) => held = Some((run, f.at, said.min(f.width))),
                        _ => {
                            row.put("value", run);
                        }
                    }
                }
            }
            Spot::Interval => {
                row.put("label", Run::label(key::EVERY).names(true));
                let days = task.interval_days.to_string();
                row.put("value", Run::datum(&days).answers(true));
                self.taught_in(&mut row, "brain-interval", &label, spelt);
                // THE UNIT SITS AGAINST THE NUMBER, which is why its column
                // is worked out here and drawn below rather than being put in
                // a column of its own. In its own column it started where the
                // column did — twenty cells past a one-digit number — and the
                // owner read `every` down one edge of the screen and `days`
                // down the other with a bare `7` between them. A number and
                // its unit are one phrase.
                //
                // This side CAN do this because a number is a DATUM: its
                // length is a thing this side holds. The label beside it is a
                // key, and its length is not.
                after = against(u16::try_from(days.chars().count()).unwrap_or(1))
                    .map(|place| (key::DAYS, Role::Quiet, place));
            }
            Spot::Window => {
                row.put("label", Run::label(key::PART).names(true));
                row.put(
                    "value",
                    Run::label(&format!("brain-part-{}", part_word(task.part_of_day)))
                        .answers(true),
                );
                self.taught_in(&mut row, "brain-window", &label, spelt);
            }
            Spot::Next => {
                // THE LABEL NAMES WHAT THE VALUE IS, and the value is one of
                // two different things. A date answers *next run*; a running
                // countdown answers *starts in* — and `00:42` under a label
                // reading `Next run` is a date nobody can parse. One row, two
                // words, each naming the thing beside it.
                //
                // There is no key column: nothing here changes this. What
                // changes it is the three rows above.
                let (label, value) = self.when_of(task);
                // A FACT: nothing on this row changes it.
                row.put("label", Run::label(label).names(false));
                row.put("value", value);
            }
            Spot::RunNow => {
                row.put("label", Run::label(key::RUN_NOW).names(true));
                // THE VALUE IS WHAT IS HAPPENING, never when it will happen.
                // It used to fall through to the next date, so the page read
                // `Run now   22/09/2026 14:00` — a verb answered with a
                // moment, and the same moment the row above now carries.
                if let Some(doing) = self.doing_of(task) {
                    row.put("value", doing);
                }
                self.taught_in(&mut row, "brain-run", &label, spelt);
            }
            Spot::Rehearse => {
                row.put("label", Run::label(key::REHEARSE).names(true));
                self.taught_in(&mut row, "brain-rehearse", &label, spelt);
            }
            Spot::Action(_) | Spot::Section(_) | Spot::Screen(_) | Spot::Step(_) | Spot::Ran(_) => {
            }
        }
        // The row's borrow of the screen ends at its last use above, which is
        // what lets the unit be placed against what it is a unit OF. There is
        // no `drop` here: `Row` owns nothing to drop, so calling it would only
        // hold the borrow OPEN to this line rather than closing it.
        if let Some((run, at, room)) = held {
            pen.put(line, run.at(at).within(room));
        }
        if let Some((said, role, (at, room))) = after {
            pen.put(line, Run::label(said).playing(role).at(at).within(room));
        }
    }

    /// How wide the schedule's labels and values come out on this task's
    /// page — the widest of each among the rows it has, measured, with the
    /// value never wider than the row leaves it.
    fn field_cells(&self, task: &TaskState, columns: u16) -> (u16, u16) {
        let mut label = 0;
        let mut value = 0;
        for spot in Self::schedule_of(task) {
            let (labels, values): (Vec<&str>, Vec<u16>) = match spot {
                Spot::Enabled => (
                    vec![enabled_key(task.kind)],
                    if off_not_by_you(task).is_some() {
                        vec![if task.why_text.is_empty() {
                            WHEN_CELLS
                        } else {
                            ronoctua_design::wide(&task.why_text)
                        }]
                    } else {
                        vec![
                            self.cells_of(key::ON, 4),
                            self.cells_of(off_key(task.kind), SWITCH_CELLS),
                        ]
                    },
                ),
                Spot::Interval => (
                    vec![key::EVERY],
                    vec![u16::try_from(task.interval_days.to_string().len()).unwrap_or(3)],
                ),
                Spot::Window => (
                    vec![key::PART],
                    PARTS
                        .iter()
                        .map(|p| self.cells_of(&part_key(*p), 16))
                        .collect(),
                ),
                // A DATE OR ITS REASON, or a countdown's `mm:ss` — and either
                // of the two words that name them.
                Spot::Next => (vec![key::NEXT, key::SOON], vec![self.moment(task).1, 5]),
                Spot::RunNow => (
                    vec![key::RUN_NOW],
                    vec![
                        self.cells_of(key::REHEARSAL, 16),
                        self.cells_of(&standing_key(Standing::Running), 12),
                    ],
                ),
                Spot::Rehearse => (vec![key::REHEARSE], Vec::new()),
                Spot::Action(_)
                | Spot::Section(_)
                | Spot::Screen(_)
                | Spot::Step(_)
                | Spot::Ran(_) => (Vec::new(), Vec::new()),
            };
            for l in labels {
                label = label.max(self.cells_of(l, SCHEDULE_LABEL_CELLS));
            }
            value = values.into_iter().fold(value, u16::max);
        }
        // THE VALUE GIVES WAY to the row before the label does: a label cut
        // says nothing, a reason cut still says how it begins.
        let room = columns
            .saturating_sub(1)
            .saturating_sub(label + 6 + 3 * ronoctua_design::GAP + 8);
        (label.max(4), value.max(4).min(room.max(WHEN_FLOOR)))
    }

    /// What is happening to this task right now, beside `Run now`.
    ///
    /// `None` when nothing is: a verb with no answer beside it draws no value
    /// at all, the way `Rehearse` always has.
    fn doing_of(&self, task: &TaskState) -> Option<Run<Icon>> {
        if self.mind.rehearsing == task.id {
            return Some(Run::label(key::REHEARSAL).accent());
        }
        if task.standing == Standing::Running {
            return Some(Run::label("brain-standing-running").accent());
        }
        None
    }

    /// When this task next runs, and which of the two words names it.
    ///
    /// The date comes from [`State::moment_of`], which is what the home page's
    /// own column draws — one task cannot be given two different answers to
    /// *when* by two screens of the same buffer.
    fn when_of(&self, task: &TaskState) -> (&'static str, Run<Icon>) {
        if self.mind.announcement.task == task.id && self.mind.announcement.deadline > self.mind.now
        {
            let left = self.mind.announcement.deadline - self.mind.now;
            return (key::SOON, Run::datum(&clock(left)).accent());
        }
        (key::NEXT, self.moment_of(task))
    }

    /// One step.
    fn step(
        &self,
        pen: &mut Pen<'_, Wit>,
        task: &TaskState,
        at: &At,
        layout: &Layout,
        which: usize,
    ) {
        let line = at.row;
        let n = at.spot;
        let Some(step) = task.steps.get(which) else {
            return;
        };
        if self.spot == n {
            pen.band(line);
        } else if let Some(seat) = at.under_pointer() {
            pen.ground(line, Role::hover_of(seat));
        }
        let Some(mut row) = pen.row_of(line, layout) else {
            return;
        };
        // A MARK AND ITS ABSENCE, both from the token table: a tick typed into
        // this file would be a codepoint chosen inside a component that has no
        // font (invariants 9 and 10).
        if step.on {
            row.put("mark", Run::glyph(Icon::Success).accent());
        }
        // THE STEP'S OWN KEY, declared by the module that owns the step and
        // sent beside its id: the ids are not the brain's, and a key built
        // here out of one would be a word nobody declared.
        if let Some(name) = key_of(&step.name_key) {
            row.put("name", Run::label(&name));
        }
        if step.needs_root {
            // THE FLAKE OWNS THE ROOT HALF, and the row says so instead of
            // offering a key that does nothing. The person's switch lives in
            // their own state directory and the root half runs as root, which
            // has no way to find it — see `gen-updates.py`, which carries the
            // whole argument. It is the answer the Syncthing view already
            // gives for the options the flake fixes (invariant 6).
            row.put("note", Run::label(key::FLAKE_OWNED).dim());
        } else if step.on != step.on_by_default {
            // "CHANGED" IS A SUBTRACTION and not a field. It used to be
            // computed from a set that assumed every step ships ON — and six
            // of the update's twelve ship OFF, so six rows said "you changed"
            // on a machine nobody had touched.
            row.put("note", Run::label(key::CHANGED).dim());
        }
    }

    /// One run in the history.
    fn ran(
        &self,
        pen: &mut Pen<'_, Wit>,
        task: &TaskState,
        at: &At,
        layout: &Layout,
        which: usize,
    ) {
        let line = at.row;
        let n = at.spot;
        let Some(run) = task.history.get(which) else {
            return;
        };
        if self.spot == n {
            pen.band(line);
        } else if let Some(seat) = at.under_pointer() {
            pen.ground(line, Role::hover_of(seat));
        }
        let Some(mut row) = pen.row_of(line, layout) else {
            return;
        };
        // HOW IT ENDED, discreetly, in the hue of the news — the colours the
        // list's last column wears, so a word is one colour everywhere.
        row.put(
            "outcome",
            Run::label(&outcome_key(run.outcome)).playing(outcome_role(run.outcome)),
        );
        // THE SHORT REASON ONLY. What the machine said can be a paragraph, and
        // a paragraph in a column is a paragraph with its end cut off — it is
        // in the footer, whole, under the cursor. The keys are the ones the
        // host sent (`at-key`, `why-key`); what a note says is DATA.
        if let Some(at) = key_of(&run.at_key) {
            row.put("why", Run::label(&at).quiet());
        } else if let Some(why) = key_of(&run.why_key) {
            row.put("why", Run::label(&why).quiet());
        } else if run.outcome == OutcomeKind::Noted && !run.says_text.is_empty() {
            row.put("why", Run::datum(&run.says_text).quiet());
        }
        // AND WHEN, its day above the rest and today's in its own hue — the
        // activity log's dates, drawn by the same hand ([`put_stamp`]).
        if let Some(when) = layout.field("when") {
            put_stamp(
                pen,
                line,
                (when.at, when.width),
                &run.when_text,
                &run.when_stamp,
                Role::Dim,
            );
        }
    }

    /// One of the module's page actions: what it does, and `RET`, which asks
    /// for it.
    fn action(
        &self,
        pen: &mut Pen<'_, Wit>,
        task: &TaskState,
        at: &At,
        layout: &Layout,
        which: usize,
    ) {
        let line = at.row;
        let Some(action) = task.actions.get(which) else {
            return;
        };
        if self.spot == at.spot {
            pen.band(line);
        } else if let Some(seat) = at.under_pointer() {
            pen.ground(line, Role::hover_of(seat));
        }
        let Some(mut row) = pen.row_of(line, layout) else {
            return;
        };
        // A THING ONE CAN DO: its label wears the tone of a row that acts.
        if let Some(label) = key_of(&action.label_key) {
            row.put("label", Run::label(&label).names(true));
        }
        row.put("keys", Run::datum(RET).accent());
    }

    /// THE TITLE OF A SECTION THAT FOLDS, which is a row like any other: the
    /// band, the hover and the two clicks are a row's. Its triangle points at
    /// what it holds, the way a folded row of the log does, and the count
    /// stands at the edge with the counts of the rows under it — the module's
    /// number, or how many rows there are.
    fn fold_title(
        &self,
        pen: &mut Pen<'_, Wit>,
        section: &ModuleSection,
        at: &At,
        open: bool,
        body: u16,
    ) {
        use ronoctua_design::{GAP, ICON};
        let line = at.row;
        if self.spot == at.spot {
            pen.band(line);
        } else if let Some(seat) = at.under_pointer() {
            pen.ground(line, Role::hover_of(seat));
        }
        let glyph = if open {
            Icon::BrainGroupOpen
        } else {
            Icon::BrainGroup
        };
        pen.put(line, Run::glyph(glyph).quiet());
        let held = ScreenCell::Count(section.rows.len() as u64);
        let tally = match section.tally {
            ScreenCell::Empty => &held,
            _ => &section.tally,
        };
        let cells = self.cell_cells(tally, FACT_CELLS).min(body);
        let count_at = body.saturating_sub(cells);
        if let Some(count) = cell_run(tally) {
            pen.put(line, count.quiet().at(count_at).within(cells));
        }
        // AND THE TITLE, which stops where the count begins.
        let title_at = ICON + GAP;
        let room = count_at.saturating_sub(title_at + GAP);
        if room > 0
            && let Some(heading) = key_of(&section.heading)
        {
            pen.put(line, Run::label(&heading).title().at(title_at).within(room));
        }
    }

    /// THE LEGEND: each colour the module's rows wear, as a swatch and the
    /// word, both in that colour — the word in colour is an honest sample of
    /// how a row reads, and the glyph has the area that tells the two reds
    /// apart. No numbers: what a section counts is on its title.
    ///
    /// A SWATCH GOES WHOLE OR NOT AT ALL: half of a word explains nothing, so
    /// what does not fit is not drawn, and neither is what comes after it.
    fn draw_legend(
        &mut self,
        pen: &mut Pen<'_, Wit>,
        swatches: &[LegendSwatch],
        line: u16,
        body: u16,
    ) {
        use ronoctua_design::ICON;
        let mut x = 0;
        for swatch in swatches {
            let word = self.cells_of(&swatch.says, FACT_CELLS);
            let to = x + ICON + word;
            if to > body {
                break;
            }
            let role = tone_role(swatch.tone);
            pen.put(line, Run::glyph(Icon::BrainSwatch).playing(role).at(x));
            pen.put(
                line,
                Run::label(&swatch.says)
                    .playing(role)
                    .at(x + ICON)
                    .within(word),
            );
            self.legend_at.push(FootSpot {
                row: line,
                from: x,
                to,
                about: swatch.about.clone(),
            });
            x = to + FACT_SPACING;
        }
    }

    /// One row of the module's own page, `lines` tall: its label, its value in
    /// the tone the module gave it, the note beside it, and what it says under
    /// itself, which the band and the hover cover like the rest of the row.
    fn screen_row(
        &self,
        pen: &mut Pen<'_, Wit>,
        row: &ModuleRow,
        at: &At,
        layout: &Layout,
        lines: u16,
    ) {
        let line = at.row;
        for line in line..line + lines {
            if self.spot == at.spot {
                pen.band(line);
            } else if let Some(seat) = at.under_pointer() {
                pen.ground(line, Role::hover_of(seat));
            }
        }
        if lines > 1
            && let Some(run) = cell_run(&row.below)
        {
            let scroll = (self.below_scroll.0 == row.id).then_some(self.below_scroll.1);
            pen.paragraph_from(
                line + 1,
                lines - 1,
                scroll.unwrap_or(0),
                run.playing(Role::Folded)
                    .at(BELOW_AT)
                    .within(below_cells(self.columns)),
            );
        }
        let Some(mut cells) = pen.row_of(line, layout) else {
            return;
        };
        // A ROW THAT ACTS is labelled as one, and a row that only tells as a
        // fact — the four standards of a row, from what the row does.
        let acts = !row.act.is_empty();
        let plain = |c| {
            if stamped(c).is_some() {
                None
            } else {
                cell_run(c)
            }
        };
        if let Some(label) = plain(&row.label) {
            cells.put("label", label.names(acts));
        }
        if let Some(value) = plain(&row.value) {
            // THE TONE IS THE MODULE'S MEANING — watching, warning, danger —
            // and quiet is the value's own standard tone.
            let value = if row.tone == BrainTone::Quiet {
                value.answers(acts)
            } else {
                value.playing(tone_role(row.tone))
            };
            cells.put("value", value);
        }
        // A COUNT STANDS AT THE EDGE, in the tone of a count, so a column of
        // them reads down by their last digits; it is placed below, once the
        // row has given the pen back.
        if !matches!(row.note, ScreenCell::Count(_))
            && let Some(note) = plain(&row.note)
        {
            cells.put("note", note.dim());
        }
        if let (ScreenCell::Count(count), Some(field)) = (&row.note, layout.field("note")) {
            let count = count.to_string();
            let wide = ronoctua_design::wide(&count).min(field.width);
            let from = field.at + field.width - wide;
            pen.put(line, Run::datum(&count).quiet().at(from).within(wide));
        }
        // AND ITS DATES, in their three pieces: placed after the row's last
        // use, since a moment is three runs in one column. One date looks the
        // same on every page, the activity log's and a task's history's
        // among them, and all a row may change of it is what the module's
        // tone says of a value.
        for (id, cell, rest) in [
            ("label", &row.label, Role::Dim),
            ("value", &row.value, dated_value_role(row.tone)),
            ("note", &row.note, Role::Dim),
        ] {
            if let (Some(moment), Some(field)) = (stamped(cell), layout.field(id)) {
                put_stamp(
                    pen,
                    line,
                    (field.at, field.width),
                    &moment.text,
                    &moment.stamp,
                    rest,
                );
            }
        }
    }
}

impl State {
    /// What the footer has to say about whatever is in focus.
    fn told(&self) -> Told {
        match self.page {
            Page::Home => match self.pane {
                Pane::Notices => self.told_about_notice(),
                Pane::Tasks => self.told_about_task(),
                Pane::Log => self.told_about_activity(),
            },
            Page::Task => self.told_about_spot(),
            Page::Log => self.told_about_line(),
        }
    }

    /// The line of the daemon's log under the cursor: when, how loud, and the
    /// whole of what it says — the row above cuts it at the width, and this is
    /// where it can be read to the end.
    fn told_about_line(&self) -> Told {
        let Some(line) = self.tail.get(self.tail_at) else {
            return Told::default();
        };
        Told {
            name: Some(key::LOG.to_owned()),
            said: Some(format!("{}  {}", line.at_text, level_word(line.level))),
            logged: Some((line.text.clone(), level_role(line.level))),
            ..Told::default()
        }
    }

    /// The notice under the cursor: whose it is, the whole sentence — the row
    /// cuts it at the width — and since when.
    fn told_about_notice(&self) -> Told {
        let Some(notice) = self.mind.notices.get(self.notice) else {
            return Told::default();
        };
        let whose = self
            .mind
            .tasks
            .iter()
            .find(|t| t.id == notice.module)
            .and_then(|t| key_of(&t.name_key));
        Told {
            name: whose,
            // THE SENTENCE IS DATA — the host made it with its values — in
            // the tone of how loud it is.
            logged: Some((notice.says_text.clone(), level_tone(notice.level))),
            facts: vec![(key::SINCE.to_owned(), notice.since_text.clone())],
            // AND ITS MODULE'S PAGE, which `RET` on a notice opens too.
            opens: self.mind.tasks.iter().position(|t| t.id == notice.module),
            ..Told::default()
        }
    }

    /// The task under the cursor on the home page: what it is, what it does,
    /// and how it last went.
    fn told_about_task(&self) -> Told {
        let Some(task) = self.here() else {
            return Told::default();
        };
        let mut facts = Vec::new();
        if task.ran {
            facts.push((key::LAST_RUN.to_owned(), task.last.when_text.clone()));
        }
        // THE TWO READINGS ARE NOT HERE, and were never asked for: a
        // percentage nobody is waiting on is a number that has to be read to
        // be dismissed. The warning block names a reading in words when it is
        // high enough to stop something.
        Told {
            name: key_of(&task.name_key),
            name_about: key_of(&task.brief_key),
            about: key_of(&task.does_key),
            // GUARDED BY `ran`, like the fact above it: `last` is documented
            // as meaningless without it.
            trouble: (task.ran && !task.last.trouble.is_empty()).then(|| task.last.trouble.clone()),
            facts,
            // AND ITS PAGE, for every task — the click at the end of the row.
            opens: Some(self.task),
            ..Told::default()
        }
    }

    /// The most useful sentence about one record, and the order is the order
    /// somebody wants it in: what a note says, what the module said stopped
    /// it, what the machine said, the brain's reason — and where it stopped,
    /// on a line of its own. `Waited` as a paragraph under a row that already
    /// says `Waited` is a paragraph that says nothing, and a key built here
    /// out of an id is a sentence on screen — the keys are the host's.
    ///
    /// WITH NOTHING KEPT, it says THAT, and where to look: a failure's words
    /// went to the system's journal. Never the outcome again.
    fn told_about_record(run: &RunRecord, name: Option<String>) -> Told {
        let mut told = Told {
            name,
            at_step: key_of(&run.at_key),
            ..Told::default()
        };
        let failed = run.outcome == OutcomeKind::Failed;
        if run.outcome == OutcomeKind::Noted && !run.says_text.is_empty() {
            told.logged = Some((run.says_text.clone(), Role::Body));
        } else if failed && !run.says_text.is_empty() {
            // A REFUSAL ITS MODULE KNOWS, in the module's own sentence —
            // "one more system would push the last good one out of the boot
            // menu" — in the tone of what the machine says went wrong.
            told.logged = Some((run.says_text.clone(), Role::Danger));
        } else if !run.trouble.is_empty() {
            told.trouble = Some(run.trouble.clone());
        } else {
            told.about = Some(key_of(&run.why_key).unwrap_or_else(|| {
                match run.outcome {
                    OutcomeKind::Done => key::WENT_WELL,
                    OutcomeKind::Failed => key::UNSAID_FAILURE,
                    OutcomeKind::Held | OutcomeKind::Skipped | OutcomeKind::Noted => key::UNSAID,
                }
                .to_owned()
            }));
        }
        told
    }

    /// One line of the activity log: a record, or a folded row — and a folded
    /// row says how many it folds and when the first of them was.
    fn told_about_activity(&self) -> Told {
        let Some(line) = self.activity_lines().get(self.log).copied() else {
            return Told::default();
        };
        let Some(run) = self.record_of(line) else {
            return Told::default();
        };
        let mut told = Self::told_about_record(run, key_of(&run.task_key));
        told.name_about = self
            .mind
            .tasks
            .iter()
            .find(|t| t.id == run.task)
            .and_then(|t| key_of(&t.brief_key));
        told.facts = vec![(key::LAST_RUN.to_owned(), run.when_text.clone())];
        // A RECORD IS OF A TASK, and the click at the end of the last row
        // opens that task's page — on a folded row too, whose `RET` opens the
        // fold instead.
        told.opens = self.mind.tasks.iter().position(|t| t.id == run.task);
        if let Line::Row(n) = line
            && let Some(row) = self.mind.activity_rows.get(n)
            && row.repeats > 1
        {
            told.facts = vec![
                (key::LAST_RUN.to_owned(), row.last_text.clone()),
                (key::FIRST.to_owned(), row.first_text.clone()),
                (key::TIMES.to_owned(), row.repeats.to_string()),
            ];
        }
        told
    }

    /// Whatever the cursor is on, on a task's page.
    fn told_about_spot(&self) -> Told {
        let Some(task) = self.here() else {
            return Told::default();
        };
        match self.spot_here() {
            Some(Spot::Step(n)) => {
                let Some(step) = task.steps.get(n) else {
                    return Told::default();
                };
                let mut facts = Vec::new();
                if step.needs_root {
                    facts.push((key::NEEDS_ROOT.to_owned(), String::new()));
                    // AND WHY THE SWITCH REFUSES, said in full where there is
                    // room for it — the SAME fact twice, so the same `if`.
                    facts.push((key::FLAKE_OWNED.to_owned(), String::new()));
                }
                Told {
                    name: key_of(&step.name_key),
                    // THE REAL COMMAND, ON A LINE OF ITS OWN and not in a
                    // column: it is the longest thing on this screen.
                    said: Some(step.command.clone()),
                    said_about: Some(key::ABOUT_COMMAND),
                    about: key_of(&step.does_key),
                    trouble: None,
                    facts,
                    ..Told::default()
                }
            }
            Some(Spot::Ran(n)) => {
                let Some(run) = task.history.get(n) else {
                    return Told::default();
                };
                // NEVER the task's own description, which this page already
                // opens with three rows up — the same paragraph printed twice
                // on one screen is what the owner photographed.
                Told {
                    name_about: Some(key::ABOUT_RECORD.to_owned()),
                    ..Self::told_about_record(run, Some(record_heading_key(run.outcome).to_owned()))
                }
            }
            Some(Spot::Action(n)) => {
                let Some(action) = task.actions.get(n) else {
                    return Told::default();
                };
                Told {
                    name: key_of(&action.label_key),
                    // WHAT IT WILL ASK, when it asks first: the question is
                    // the best sentence there is about what the key does.
                    about: key_of(&action.asks_key),
                    ..Told::default()
                }
            }
            // A SECTION'S TITLE: what it is, and what is in it, whole — the
            // same sentence the pointer finds on the title.
            Some(Spot::Section(n)) => {
                let Some(section) = self.page_of(task).and_then(|sc| sc.sections.get(n)) else {
                    return Told::default();
                };
                Told {
                    name: key_of(&section.heading),
                    about: key_of(&section.about),
                    ..Told::default()
                }
            }
            Some(Spot::Screen(n)) => {
                let rows = self.screen_rows(task);
                let Some(row) = rows.get(n) else {
                    return Told::default();
                };
                if row.foot.is_empty() && says_nothing(&row.below) {
                    Self::told_of_row(row)
                } else {
                    self.told_of_foot(task, row)
                }
            }
            // EVERY OTHER SPOT IS ABOUT THE TASK, which is what the page is
            // about — and it does NOT repeat the description the page opens
            // with. What a schedule row wants is how it last went.
            _ => {
                let mut facts = Vec::new();
                if task.ran {
                    facts.push((key::LAST_RUN.to_owned(), task.last.when_text.clone()));
                }
                Told {
                    name: key_of(&task.name_key),
                    trouble: (task.ran && !task.last.trouble.is_empty())
                        .then(|| task.last.trouble.clone()),
                    facts,
                    ..Told::default()
                }
            }
        }
    }
}

impl State {
    /// A row of a module's page, from its cells.
    fn told_of_row(row: &ModuleRow) -> Told {
        let key = |c: &ScreenCell| match c {
            ScreenCell::Key(k) => key_of(k),
            _ => None,
        };
        // THE ROW, WHOLE: its label as the name when it is a key and as the
        // held line when it is data; the VALUE whole and wrapped when it is
        // data the row had to cut (a journal's message), in the row's tone,
        // with the note on the held line after the label — and otherwise the
        // note as the sentence.
        let label = cell_text(&row.label);
        let value = cell_text(&row.value).filter(|_| label.is_some());
        let note = cell_text(&row.note);
        let (said, logged) = match value {
            Some(v) => (
                Some(match (&label, &note) {
                    (Some(l), Some(n)) => format!("{l}  {n}"),
                    _ => label.clone().unwrap_or_default(),
                }),
                Some((v, tone_role(row.tone))),
            ),
            None => (
                label.or_else(|| cell_text(&row.value)),
                note.map(|t| (t, tone_role(row.tone))),
            ),
        };
        Told {
            name: key(&row.label).or_else(|| key(&row.value)),
            said,
            about: key(&row.note),
            logged,
            ..Told::default()
        }
    }

    /// A row of a module's page that says more of itself than its cells: its
    /// name, what it says under itself and each fact, each in its own colour —
    /// the owner's *"tudo está na mesma cor"* — and what its other action is
    /// called here.
    ///
    /// ITS NAME IS WHAT IT IS CALLED, and a date in the label's place calls
    /// it nothing: the row says when, and its value says what.
    fn told_of_foot(&self, task: &TaskState, row: &ModuleRow) -> Told {
        let below = (!says_nothing(&row.below)).then(|| row.below.clone());
        let also = self
            .also_here()
            .and_then(|_| task.actions.iter().find(|a| a.id == row.also))
            .and_then(|a| key_of(&a.label_key));
        // WITH NO FACTS, what its cells say is all the footer has to go on.
        if row.foot.is_empty() {
            return Told {
                below,
                also,
                ..Self::told_of_row(row)
            };
        }
        let named = match &row.label {
            ScreenCell::Moment(_) | ScreenCell::Count(_) | ScreenCell::Bytes(_) => &row.value,
            label if says_nothing(label) => &row.value,
            label => label,
        };
        let (name, headline) = match named {
            ScreenCell::Key(k) => (key_of(k), None),
            other => (None, cell_text(other)),
        };
        Told {
            name,
            headline,
            below,
            lines: row.foot.clone(),
            also,
            ..Told::default()
        }
    }

    /// WHAT THE THING UNDER THE POINTER IS, in a sentence — declared for the
    /// host (`Screen::point`), which says it where every window says what its
    /// row under the pointer adds: the header's line, or, with the header
    /// hidden — the distro's default — the footer's minibuffer.
    ///
    /// # Why the pointer and not the cursor
    ///
    /// Because the owner rested the pointer on a task and on its glyph and
    /// nothing said anything: *"ao colocar o mouse por cima de uma task do
    /// Brain, nada aparece … ou se eu coloco o cursor sobre o glifo … não é
    /// mostrado do que se trata o glifo"*. The footer of this page explains
    /// what the KEYBOARD is on; this is the same question asked by the hand,
    /// answered without moving the cursor.
    ///
    /// Asked of the blocks and the columns the last frame drew, by the first
    /// pointer to rest here: one line says it, and one hand is reading.
    fn pointed(&self) -> Option<Pointed> {
        let (_, (column, line)) = self.hover.cells().next()?;
        if let Some(open) = self.open_at
            && self.blocks.foot.row_at(line) == Some(open.row)
            && (open.from..open.to).contains(&column)
        {
            return Some(Pointed::Key(key::ABOUT_OPEN_PAGE.into()));
        }
        // EVERYTHING DRAWN SAYS SOMETHING — the owner's *"todos os elementos
        // do brain, ao colocar o cursor por cima, mostre algo no
        // minibuffer"*: each title what the rows under it are, each figure
        // what it counts. Only a line with nothing on it says nothing.
        //
        // THE FOOTER, piece by piece as it was drawn, and otherwise what the
        // footer is.
        if let Some(row) = self.blocks.foot.row_at(line) {
            if let Some(spot) = self
                .foot_spots
                .iter()
                .find(|s| s.row == row && (s.from..s.to).contains(&column))
            {
                return Some(Pointed::Key(spot.about.clone()));
            }
            return self
                .foot_drawn
                .then(|| Pointed::Key(key::ABOUT_FOOT.into()));
        }
        // THE WARNING: why nothing has started.
        if self.blocks.warn.row_at(line).is_some() {
            return Some(Pointed::Key(key::ABOUT_WARNING.into()));
        }
        // A NOTICE: its whole sentence, which its row cuts at the width.
        if let Some(row) = self.blocks.notices.row_at(line) {
            let Some(below) = row.checked_sub(1) else {
                return Some(Pointed::Key(key::ABOUT_NOTICES.into()));
            };
            let notice = self
                .mind
                .notices
                .get(self.notices_from + usize::from(below))?;
            return Some(Pointed::Datum(notice.says_text.clone()));
        }
        // A LINE OF THE ACTIVITY LOG: what the fold's mark and its count
        // mean, under them, and otherwise what the record says.
        if let Some(row) = self.blocks.log.row_at(line) {
            let Some(below) = row.checked_sub(1) else {
                return Some(Pointed::Key(
                    if column >= COUNT_AT {
                        key::ABOUT_REACH
                    } else {
                        key::ABOUT_ACTIVITY
                    }
                    .into(),
                ));
            };
            let activity = Pointed::Key(key::ABOUT_ACTIVITY.into());
            let Some(at) = self
                .activity_lines()
                .get(self.log_from + usize::from(below))
                .copied()
            else {
                return Some(activity);
            };
            if let Line::Row(g) = at
                && self
                    .mind
                    .activity_rows
                    .get(g)
                    .is_some_and(|r| r.repeats > 1)
                && (within(&self.log_layout, "fold", column)
                    || within(&self.log_layout, "count", column))
            {
                return Some(Pointed::Key(key::ABOUT_FOLD.into()));
            }
            return self
                .record_of(at)
                .and_then(|r| self.said_of_record(r))
                .or(Some(activity));
        }
        let row = self.blocks.body.row_at(line)?;
        // THE WAY BACK atop a task's page, where the window wears no header.
        if let Some((at, from, to)) = self.back_at
            && row == at
            && (from..to).contains(&column)
        {
            return Some(Pointed::Key(key::ABOUT_WAY_BACK.into()));
        }
        match self.page {
            // A TASK, COLUMN BY COLUMN: what its glyph means under the glyph,
            // when it runs next under the date, how it stands or how its last
            // run went under the status — and anywhere else what it IS, in one
            // line. It said the whole of its description, the footer's
            // paragraph run along one line of the minibuffer, and the owner
            // asked for a line that says what the task is about instead.
            Page::Home => {
                // THE TITLE, and the page's own name at the other end of it.
                let Some(below) = row.checked_sub(1) else {
                    let name = self.title_from.is_some_and(|from| column >= from);
                    return Some(Pointed::Key(
                        if name {
                            key::ABOUT_TITLE
                        } else {
                            key::ABOUT_TASKS
                        }
                        .into(),
                    ));
                };
                let Some(task) = self.mind.tasks.get(self.body_from + usize::from(below)) else {
                    return Some(Pointed::Key(key::ABOUT_TASKS.into()));
                };
                if within(&self.task_layout, "kind", column) {
                    return Some(Pointed::Key(kind_about(task.kind).into()));
                }
                if within(&self.task_layout, "when", column) {
                    return Some(when_said(task));
                }
                if within(&self.task_layout, "note", column)
                    && let Some(said) = note_said(task)
                {
                    return Some(said);
                }
                key_of(&task.brief_key)
                    .or_else(|| key_of(&task.does_key))
                    .map(Pointed::Key)
            }
            Page::Task => {
                // UNDER A ROW THE WINDOW CUT, nothing was drawn to say.
                if self.page_cut.is_some_and(|cut| row >= cut) {
                    return None;
                }
                let task = self.here()?;
                let rows = self.page_rows();
                let n = row_at_line(&rows, self.body_from, row)?;
                match rows.get(n)? {
                    Row::Field(spot) => Some(Pointed::Key(spot_about(*spot)?.into())),
                    Row::Step(k) => key_of(&task.steps.get(*k)?.does_key).map(Pointed::Key),
                    Row::Ran(k) => self.said_of_record(task.history.get(*k)?),
                    Row::Action(k) => {
                        let action = task.actions.get(*k)?;
                        key_of(&action.asks_key)
                            .or_else(|| key_of(&action.label_key))
                            .map(Pointed::Key)
                    }
                    Row::Screen(k, _) => {
                        let cells = self.screen_rows(task);
                        let cell = cells.get(*k)?;
                        // THE LINES UNDER IT say what they say; on the row
                        // itself the note, and what the row has left to say.
                        let first = lines_before(&rows, n) - lines_before(&rows, self.body_from);
                        if row > first {
                            return said_of_cell(&cell.below);
                        }
                        said_of_cell(&cell.note)
                            .or_else(|| said_of_cell(&cell.value))
                            .or_else(|| said_of_cell(&cell.label))
                    }
                    // THE PAGE'S OWN TITLE says what the task is, in one line,
                    // and so does its description, which says it at length.
                    Row::Title => key_of(&task.brief_key).map(Pointed::Key),
                    Row::About(_, _) => Some(Pointed::Key(
                        key_of(&task.brief_key).unwrap_or_else(|| key::ABOUT_DOES.to_owned()),
                    )),
                    // A TITLE says what is under it, and the figures beside
                    // it what they count.
                    Row::Heading(k) => {
                        let figures = column >= COUNT_AT;
                        Some(Pointed::Key(
                            match *k {
                                key::STEPS if figures => key::ABOUT_STEPS_ON,
                                key::HISTORY if figures => key::ABOUT_REACH,
                                k => heading_about(k),
                            }
                            .into(),
                        ))
                    }
                    // A SECTION SAYS WHAT IS IN IT, in its module's words, and
                    // its count says the same: what the number counts is the
                    // one thing that changes from section to section. Only
                    // the triangle says what a click on it is for.
                    Row::Section(n) => Some(Pointed::Key(self.section_about(task, *n))),
                    Row::Fold(_) if column < ronoctua_design::ICON => {
                        Some(Pointed::Key(key::ABOUT_SECTION_FOLD.into()))
                    }
                    Row::Fold(n) => Some(Pointed::Key(self.section_about(task, *n))),
                    Row::Legend => self
                        .legend_at
                        .iter()
                        .find(|s| s.row == row && (s.from..s.to).contains(&column))
                        .and_then(|s| key_of(&s.about))
                        .map(Pointed::Key),
                    // WHY THE MODULE'S PAGE IS NOT HERE, whole.
                    Row::Refused(_) => self
                        .page_of(task)
                        .map(|sc| Pointed::Datum(sc.refused.clone())),
                    Row::Blank => None,
                }
            }
            // A LINE OF THE DAEMON'S LOG: the whole of it, which its row cuts;
            // and before the first one, what the page is.
            Page::Log => {
                if self.tail.is_empty() {
                    return Some(Pointed::Key(key::ABOUT_LOG_PAGE.into()));
                }
                self.tail
                    .get(self.tail_from + usize::from(row))
                    .map(|l| Pointed::Datum(l.text.clone()))
            }
        }
    }

    /// What a section of the module's page says under the pointer: what its
    /// module says of it, or that it is its module's at all.
    fn section_about(&self, task: &TaskState, n: usize) -> String {
        self.page_of(task)
            .and_then(|sc| sc.sections.get(n))
            .and_then(|s| key_of(&s.about))
            .unwrap_or_else(|| key::ABOUT_SECTION.to_owned())
    }

    /// The most useful sentence about a record, under the pointer: what a
    /// watch noted, what the machine said went wrong, the brain's reason or
    /// the step it stopped at — and, for a run that simply worked, what its
    /// task does, since `Worked` beside it already says how it went.
    fn said_of_record(&self, run: &RunRecord) -> Option<Pointed> {
        if matches!(run.outcome, OutcomeKind::Noted | OutcomeKind::Failed)
            && !run.says_text.is_empty()
        {
            return Some(Pointed::Datum(run.says_text.clone()));
        }
        if !run.trouble.is_empty() {
            return Some(Pointed::Datum(run.trouble.clone()));
        }
        key_of(&run.why_key)
            .or_else(|| key_of(&run.at_key))
            .or_else(|| {
                self.mind
                    .tasks
                    .iter()
                    .find(|t| t.id == run.task)
                    .and_then(|t| key_of(&t.does_key))
            })
            .map(Pointed::Key)
    }
}

/// What a row's date column says under the pointer — the same cases, in the
/// same order, as the column itself ([`State::moment`]): a watch has no date,
/// a reason stands in for one, a task under way is running now, and a date is
/// when it runs next.
fn when_said(task: &TaskState) -> Pointed {
    if task.kind == TaskKind::Watch {
        return Pointed::Key(kind_about(task.kind).into());
    }
    if key_of(&task.why_key).is_some() {
        return if task.why_text.is_empty() {
            Pointed::Key(key::ABOUT_NEXT.into())
        } else {
            Pointed::Phrase(
                key::ABOUT_NEXT_WHY.into(),
                vec![("why".into(), task.why_text.clone())],
            )
        };
    }
    if task.standing == Standing::Running {
        return Pointed::Key(key::ABOUT_RUNNING.into());
    }
    if task.next_text.is_empty() {
        return Pointed::Key(key::ABOUT_NEXT_NONE.into());
    }
    Pointed::Phrase(
        key::ABOUT_NEXT_AT.into(),
        vec![("when".into(), task.next_text.clone())],
    )
}

/// What a row's status says under the pointer — the cases of the status
/// itself ([`State::note`]): what is in its way, how its last run went and
/// when, or where it stands having not run. `None` where the column is empty.
fn note_said(task: &TaskState) -> Option<Pointed> {
    if let Some(waits) = hold_about(task.hold) {
        return Some(Pointed::Key(waits.into()));
    }
    if task.ran {
        return Some(Pointed::Phrase(
            last_about(task.last.outcome).into(),
            vec![("when".into(), task.last.when_text.clone())],
        ));
    }
    if off_by_the_system(task) || task.kind == TaskKind::Watch {
        return None;
    }
    Some(Pointed::Key(standing_about(task.standing).into()))
}

/// Whether `column` falls in the column `id` of `layout`.
fn within(layout: &Layout, id: &str, column: u16) -> bool {
    layout
        .field(id)
        .is_some_and(|f| (f.at..f.at.saturating_add(f.width)).contains(&column))
}

/// What a schedule's row does, under the pointer — the sentences the header
/// says about the same keys.
const fn spot_about(spot: Spot) -> Option<&'static str> {
    match spot {
        Spot::Enabled => Some(key::ABOUT_SWITCH),
        Spot::Interval => Some(key::ABOUT_INTERVAL),
        Spot::Window => Some(key::ABOUT_WINDOW),
        Spot::Next => Some(key::ABOUT_NEXT),
        Spot::RunNow => Some(key::ABOUT_RUN),
        Spot::Rehearse => Some(key::ABOUT_REHEARSE),
        Spot::Action(_) | Spot::Section(_) | Spot::Screen(_) | Spot::Step(_) | Spot::Ran(_) => None,
    }
}

/// The text of a cell of a module's row, when it holds any this side can read:
/// a key is only the host's to say.
fn cell_text(c: &ScreenCell) -> Option<String> {
    match c {
        ScreenCell::Data(d) => Some(d.clone()),
        ScreenCell::Moment(t) => Some(t.text.clone()),
        ScreenCell::Count(v) => Some(v.to_string()),
        ScreenCell::Bytes(m) => Some(m.text.clone()),
        ScreenCell::Empty | ScreenCell::Key(_) => None,
    }
}

/// The same, with a key as its own name: what tells two cells apart and how
/// long what is held comes out.
fn cell_word(c: &ScreenCell) -> String {
    match c {
        ScreenCell::Key(k) => k.clone(),
        other => cell_text(other).unwrap_or_default(),
    }
}

/// A cell of a module's row as a sentence: its key, or its data.
fn said_of_cell(c: &ScreenCell) -> Option<Pointed> {
    match c {
        ScreenCell::Key(k) => key_of(k).map(Pointed::Key),
        ScreenCell::Data(d) if !d.is_empty() => Some(Pointed::Datum(d.clone())),
        ScreenCell::Moment(t) if !t.text.is_empty() => Some(Pointed::Datum(t.text.clone())),
        ScreenCell::Bytes(m) if !m.text.is_empty() => Some(Pointed::Datum(m.text.clone())),
        ScreenCell::Empty
        | ScreenCell::Data(_)
        | ScreenCell::Moment(_)
        | ScreenCell::Bytes(_)
        | ScreenCell::Count(_) => None,
    }
}

// ---------------------------------------------------------------------------
// Keys, the pointer, and the frame.
// ---------------------------------------------------------------------------

impl State {
    /// Which state id the host is told about, from the page.
    const fn state_id(&self) -> &'static str {
        match self.page {
            Page::Home => "normal",
            Page::Task => "task",
            Page::Log => "log",
        }
    }

    /// A verb about the task under the cursor.
    ///
    /// The target is ONE-BASED: zero is "no task", which is what an empty list
    /// has to be able to say.
    fn verb(&self, what: Request) -> Vec<Effect> {
        if self.mind.tasks.is_empty() {
            return Vec::new();
        }
        vec![Effect::Ask(Demand {
            what,
            target: self.task as u64 + 1,
        })]
    }

    /// A verb about the notice the three notice keys act on — target 1-based,
    /// into the list the host just sent.
    fn notice_verb(&self, what: Request) -> Vec<Effect> {
        let Some(n) = self.notice_target() else {
            return Vec::new();
        };
        let Some(notice) = self.mind.notices.get(n) else {
            return Vec::new();
        };
        // WHAT THE NOTICE DOES NOT OFFER, NOTHING ASKS FOR: the header does
        // not teach the key then, and the host would refuse it anyway.
        let offered = match what {
            Request::ActNotice => !notice.controls.is_empty(),
            Request::DismissNotice => notice.dismissable,
            Request::SnoozeNotice => !notice.snooze.is_empty(),
            _ => false,
        };
        if !offered {
            return Vec::new();
        }
        vec![Effect::Ask(Demand {
            what,
            target: n as u64 + 1,
        })]
    }

    /// A preference of the task under the cursor.
    fn tune(&self, knob: Knob) -> Vec<Effect> {
        let Some(task) = self.here() else {
            return Vec::new();
        };
        vec![Effect::Tune(Setting {
            task: task.id.clone(),
            knob,
        })]
    }

    /// The next rung of the cadence ladder, wrapping to the first.
    fn next_cadence(&self, days: u16) -> u16 {
        let ladder = &self.mind.cadence;
        if ladder.is_empty() {
            return days;
        }
        ladder
            .iter()
            .copied()
            .find(|rung| *rung > days)
            .unwrap_or_else(|| ladder[0])
    }

    /// Flipping the step under the cursor, when there is one to flip.
    fn flip_step(&self) -> Vec<Effect> {
        let Some(Spot::Step(n)) = self.spot_here() else {
            return Vec::new();
        };
        let Some(step) = self.here().and_then(|t| t.steps.get(n)) else {
            return Vec::new();
        };
        // THE FLAKE'S ROWS REFUSE, and the row already says so. Sending the
        // knob would write an entry into the person's file that nothing on the
        // machine can read — which is the defect this whole page was opened
        // for.
        if step.needs_root {
            return Vec::new();
        }
        self.tune(Knob::Step(StepSwitch {
            id: step.id.clone(),
            on: !step.on,
        }))
    }

    /// Folding the section `n` the other way from how it is drawn, by the road
    /// a step's switch takes: the mind brings the answer back, and the page is
    /// drawn from the mind, so it comes out the same in every window.
    fn flip_section(&self, n: usize) -> Vec<Effect> {
        let Some(task) = self.here() else {
            return Vec::new();
        };
        let shown = self.shown_sections(task);
        let Some(open) = shown
            .iter()
            .find(|s| s.section == n && s.folds)
            .map(|s| s.open)
        else {
            return Vec::new();
        };
        let Some(section) = self.page_of(task).and_then(|sc| sc.sections.get(n)) else {
            return Vec::new();
        };
        self.tune(Knob::Fold(SectionState {
            id: section.id.clone(),
            open: !open,
        }))
    }

    /// What a command id does.
    fn command(&mut self, id: &str) -> Vec<Effect> {
        match id {
            // A MODULE THAT ONLY LISTENS runs nothing: the page draws no row
            // for either and the header teaches neither.
            "brain-run" | "brain-rehearse"
                if self.here().is_some_and(|t| t.kind == TaskKind::Watch) =>
            {
                Vec::new()
            }
            "brain-run" => self.verb(Request::RunTask),
            "brain-rehearse" => self.verb(Request::RehearseTask),
            "brain-abort" => self.verb(Request::AbortTask),
            "brain-proceed" => self.verb(Request::Proceed),
            "brain-reset-steps" if self.here().is_some_and(|t| t.steps.is_empty()) => Vec::new(),
            "brain-reset-steps" => self.tune(Knob::ResetSteps),
            // A CADENCE AND A WINDOW ONLY FOR A TASK THAT KEEPS A CALENDAR:
            // the page draws neither for one that does not, and a key that
            // moved a number nothing reads would be a key that lies.
            "brain-interval" => {
                let Some(days) = self.here().filter(|t| t.calendar).map(|t| t.interval_days) else {
                    return Vec::new();
                };
                self.tune(Knob::IntervalDays(self.next_cadence(days)))
            }
            "brain-window" => {
                let Some(part) = self.here().filter(|t| t.calendar).map(|t| t.part_of_day) else {
                    return Vec::new();
                };
                self.tune(Knob::PartOfDay(next_part(part)))
            }
            "brain-switch" => {
                let Some(on) = self.here().map(|t| t.enabled) else {
                    return Vec::new();
                };
                self.tune(Knob::Enabled(!on))
            }
            // THE NOTICE'S THREE: its first control, putting it away, and
            // putting it off by the first snooze it offers — each an index
            // into the list the host sent, which the host checks.
            "brain-act" => self.notice_verb(Request::ActNotice),
            "brain-dismiss" => self.notice_verb(Request::DismissNotice),
            // THE ROW'S SECOND ACTION, on a module's page: by the row's index,
            // which the host checks against the page it sent.
            "brain-row-also" => match self.also_here() {
                Some(n) => vec![Effect::Ask(Demand {
                    what: Request::ActRowAlso,
                    target: n as u64 + 1,
                })],
                None => Vec::new(),
            },
            "brain-snooze" => self.notice_verb(Request::SnoozeNotice),
            // THE LOG'S PAGE, and the same key takes it away: a page that is
            // looked at and left.
            "brain-log" => {
                self.page = if self.page == Page::Log {
                    Page::Home
                } else {
                    // Opened ON THE NEWEST line, which is the one a live log
                    // is opened to read.
                    self.tail_at = self.tail.len().saturating_sub(1);
                    self.follows = true;
                    Page::Log
                };
                vec![Effect::Redraw]
            }
            _ => Vec::new(),
        }
    }

    /// What `RET` does, which depends on what it is on.
    ///
    /// # Every arm that changes the page says `Redraw`
    ///
    /// Opening a task returned NOTHING, and the only effect that reached the
    /// host was the `StateChanged` that `event` appends by comparing the state
    /// id before and after. The host took that as bookkeeping — it named the
    /// new screen on the modeline and drew none of it — so the owner pressed
    /// `RET`, watched the title change, and kept looking at the home page until
    /// an arrow key repainted it, one row already scrolled.
    ///
    /// The host now repaints on a real state change as well, which is what
    /// stops the next buffer falling here. This side still says it, because a
    /// page change is a redraw whether or not the state id happens to move —
    /// and the three other buffers that walk pages this way already do.
    fn confirm(&mut self) -> Vec<Effect> {
        match self.page {
            Page::Home => {
                // WHICH TASK'S PAGE `RET` OPENS, from the part of the page the
                // keyboard is in — or the fold it opens, which is no page.
                let at = match self.pane {
                    Pane::Tasks => self.here().map(|_| self.task),
                    // A NOTICE OPENS ITS MODULE'S PAGE: the three keys in the
                    // header act on it, and `RET` is how one goes and looks.
                    Pane::Notices => self
                        .mind
                        .notices
                        .get(self.notice)
                        .and_then(|n| self.mind.tasks.iter().position(|t| t.id == n.module)),
                    // A ROW OF THE LOG THAT FOLDS OPENS, or closes — and any
                    // other row, or a record an opened row folds, opens its
                    // task, which is the only thing there is to do with a
                    // record of something that has already happened.
                    Pane::Log => {
                        let Some(line) = self.activity_lines().get(self.log).copied() else {
                            return Vec::new();
                        };
                        if let Line::Row(n) = line
                            && let Some(row) = self.mind.activity_rows.get(n)
                            && row.repeats > 1
                        {
                            let fold = Self::fold_of(row);
                            if let Some(at) = self.opened.iter().position(|f| *f == fold) {
                                self.opened.remove(at);
                            } else {
                                self.opened.push(fold);
                            }
                            self.follows = true;
                            return vec![Effect::Redraw];
                        }
                        let Some(at) = self
                            .record_of(line)
                            .and_then(|r| self.mind.tasks.iter().position(|t| t.id == r.task))
                        else {
                            return Vec::new();
                        };
                        return self.open_task(at);
                    }
                };
                let Some(at) = at else {
                    return Vec::new();
                };
                self.open_task(at)
            }
            // ON A TASK'S PAGE, `RET` IS THE ROW'S OWN KEY. Every row that can
            // be changed says which key changes it, and this is that key
            // without having to reach for it — the same contract the locale
            // view's rows have.
            Page::Task => match self.spot_here() {
                Some(Spot::Enabled) => self.command("brain-switch"),
                Some(Spot::Interval) => self.command("brain-interval"),
                Some(Spot::Window) => self.command("brain-window"),
                Some(Spot::RunNow) => self.command("brain-run"),
                Some(Spot::Rehearse) => self.command("brain-rehearse"),
                Some(Spot::Step(_)) => self.flip_step(),
                Some(Spot::Section(n)) => self.flip_section(n),
                // ONE OF THE MODULE'S OWN: the host asks the question first
                // when the action has one, and sends the answer with it.
                Some(Spot::Action(n)) => vec![Effect::Ask(Demand {
                    what: Request::ActPage,
                    target: n as u64 + 1,
                })],
                // A ROW OF THE MODULE'S PAGE acts when it has an action, and
                // only tells when it has none — the footer already holds it.
                Some(Spot::Screen(n)) => {
                    let acts = self
                        .here()
                        .map(|t| self.screen_rows(t))
                        .and_then(|rows| rows.get(n).map(|r| !r.act.is_empty()))
                        .unwrap_or(false);
                    if acts {
                        vec![Effect::Ask(Demand {
                            what: Request::ActRow,
                            target: n as u64 + 1,
                        })]
                    } else {
                        Vec::new()
                    }
                }
                // NEXT AND A PAST RUN ARE THE TWO ROWS THAT ONLY ANSWER.
                // The cursor stops on them because the footer has more to say
                // about each than the row can hold; `RET` does nothing,
                // because there is nothing about a moment to do.
                Some(Spot::Next | Spot::Ran(_)) | None => Vec::new(),
            },
            // A LINE OF THE LOG ONLY ANSWERS: the footer already holds the
            // whole of it, and there is nothing to do to a line that has been
            // written.
            Page::Log => Vec::new(),
        }
    }

    /// Opens the page of task `n`, from its top — what `RET` does on a task,
    /// a notice or a record, and the footer's click.
    fn open_task(&mut self, n: usize) -> Vec<Effect> {
        if n >= self.mind.tasks.len() {
            return Vec::new();
        }
        self.task = n;
        self.page = Page::Task;
        self.spot = 0;
        self.body_from = 0;
        self.about_scroll = 0;
        self.follows = true;
        vec![Effect::Redraw]
    }

    /// The part of the home page `Tab` goes to next: the notices only while
    /// there are any.
    fn next_pane(&self) -> Pane {
        match self.pane {
            Pane::Notices => Pane::Tasks,
            Pane::Tasks => Pane::Log,
            Pane::Log if self.mind.notices.is_empty() => Pane::Tasks,
            Pane::Log => Pane::Notices,
        }
    }

    /// Whether the host must be told which task's page to follow, and the ask
    /// that tells it.
    ///
    /// Asked after EVERY input, and it answers only when the page in front
    /// changed: a task's page opened asks for that task's module screen, and
    /// leaving it (`0`) stops the host asking the brain for it. What is
    /// remembered is the task's ID, so a page left open while the list
    /// reorders is not a page that asks again.
    fn watch_asks(&mut self) -> Vec<Effect> {
        let wanted = if self.page == Page::Task {
            self.here().map(|t| t.id.clone())
        } else {
            None
        };
        if wanted == self.watched {
            return Vec::new();
        }
        // A PAGE THAT WAS SOMEBODY ELSE'S goes with the watch: a module's
        // rows under another task's name would be a page that lies.
        if self.screen.as_ref().map(|sc| &sc.module) != wanted.as_ref() {
            self.screen = None;
        }
        let target = match &wanted {
            Some(_) => self.task as u64 + 1,
            None => 0,
        };
        self.watched = wanted;
        vec![Effect::Ask(Demand {
            what: Request::WatchScreen,
            target,
        })]
    }

    /// A module's page arrived. Kept only when it is the page in front's, and
    /// not older than the one already held.
    fn screened(&mut self, screen: ModuleScreen) -> Vec<Effect> {
        if self.watched.as_deref() != Some(screen.module.as_str()) {
            return Vec::new();
        }
        if self
            .screen
            .as_ref()
            .is_some_and(|held| held.module == screen.module && held.generation > screen.generation)
        {
            return Vec::new();
        }
        let held = self.held();
        self.page_phrases = Phrases::of(
            screen
                .phrases
                .iter()
                .map(|p| (p.key.clone(), p.text.clone()))
                .collect(),
        );
        self.screen = Some(screen);
        self.put_back(held.as_ref());
        vec![Effect::Redraw]
    }

    /// What the cursor is on, by what names it.
    fn held(&self) -> Option<Held> {
        let spot = self.spot_here()?;
        let task = self.here()?;
        let sections = self.page_of(task).map_or(&[][..], |sc| &sc.sections);
        Some(match spot {
            Spot::Section(n) => Held::Title(section_name(sections.get(n)?).to_owned()),
            Spot::Screen(k) => {
                let shown = self
                    .shown_sections(task)
                    .into_iter()
                    .find(|s| (s.from..s.from + s.rows).contains(&k))?;
                let section = sections.get(shown.section)?;
                let name = section_name(section);
                let place = k - shown.from;
                Held::Row {
                    section: name.to_owned(),
                    twin: sections[..shown.section]
                        .iter()
                        .filter(|s| section_name(s) == name)
                        .count(),
                    id: section.rows.get(place)?.id.clone(),
                    place,
                    stood: self.spot.saturating_sub(place + usize::from(shown.folds)),
                }
            }
            Spot::Action(n) => Held::Action {
                id: task.actions.get(n)?.id.clone(),
                place: Self::page_actions(task).position(|m| m == n)?,
            },
            other => Held::Spot(other),
        })
    }

    /// The spot that stands for `held` on the page as it is now, if any does.
    ///
    /// Within its section, its row wherever it went, and when it left, the
    /// row that took its place. A section the person has shut in another
    /// window keeps the cursor on its title.
    ///
    /// AMONG SECTIONS OF ONE NAME, the one that holds the row, and with the
    /// row gone the one at its place among them. The first of that name would
    /// put the cursor on another thing's row, and `RET` would act on it.
    fn spot_of(&self, held: &Held) -> Option<Spot> {
        let task = self.here()?;
        let sections = self.page_of(task).map_or(&[][..], |sc| &sc.sections);
        let named = |name: &str| sections.iter().position(|s| section_name(s) == name);
        match held {
            Held::Spot(spot) => Some(*spot),
            // THE ACTION WHEREVER THE LIST PUT IT, and with it gone, the one
            // at its place among the page's actions — never what stands at
            // its old index among the spots, a section's title or a row.
            Held::Action { id, place } => {
                let page: Vec<usize> = Self::page_actions(task).collect();
                page.iter()
                    .copied()
                    .find(|&n| task.actions[n].id == *id)
                    .or_else(|| page.get((*place).min(page.len().checked_sub(1)?)).copied())
                    .map(Spot::Action)
            }
            Held::Title(name) => named(name).map(Spot::Section),
            Held::Row {
                section,
                twin,
                id,
                place,
                ..
            } => {
                let alike = || {
                    let of_it = |(_, s): &(usize, &ModuleSection)| section_name(s) == section;
                    sections.iter().enumerate().filter(of_it)
                };
                let (n, _) = alike()
                    .find(|(_, s)| s.rows.iter().any(|r| r.id == *id))
                    .or_else(|| alike().nth(*twin))?;
                let shown = self
                    .shown_sections(task)
                    .into_iter()
                    .find(|s| s.section == n)?;
                if !shown.open {
                    return Some(Spot::Section(n));
                }
                let rows = &sections.get(n)?.rows;
                let at = rows
                    .iter()
                    .position(|r| r.id == *id)
                    .unwrap_or_else(|| (*place).min(shown.rows - 1));
                Some(Spot::Screen(shown.from + at))
            }
        }
    }

    /// The cursor, back on what it was on — or, with that gone, at its row,
    /// inside the list; and a row whose whole section left, at what stands
    /// where the section stood.
    fn put_back(&mut self, held: Option<&Held>) {
        let spots = self.spots();
        let gone = match held {
            Some(Held::Row { stood, .. }) => *stood,
            _ => self.spot,
        };
        self.spot = held
            .and_then(|h| self.spot_of(h))
            .and_then(|wanted| spots.iter().position(|s| *s == wanted))
            .unwrap_or(gone)
            .min(spots.len().saturating_sub(1));
    }

    /// How many things the cursor can walk in the place it is.
    fn walkable(&self) -> usize {
        match self.page {
            Page::Home => match self.pane {
                Pane::Notices => self.mind.notices.len(),
                Pane::Tasks => self.mind.tasks.len(),
                Pane::Log => self.activity_lines().len(),
            },
            Page::Task => self.spots().len(),
            Page::Log => self.tail.len(),
        }
    }

    /// The cursor of the place the keyboard is in.
    fn cursor(&mut self) -> &mut usize {
        match self.page {
            Page::Home => match self.pane {
                Pane::Notices => &mut self.notice,
                Pane::Tasks => &mut self.task,
                Pane::Log => &mut self.log,
            },
            Page::Task => &mut self.spot,
            Page::Log => &mut self.tail_at,
        }
    }

    /// A new mind arrived.
    fn received(&mut self, mind: Mind) -> Vec<Effect> {
        // WHAT EACH CURSOR IS ON, by what names it — taken before the lists
        // change under it.
        let task_on = self.here().map(|t| t.id.clone());
        let notice_on = self.mind.notices.get(self.notice).map(notice_key);
        let log_on = self
            .activity_lines()
            .get(self.log)
            .and_then(|l| self.line_key(*l));
        let held = self.held();
        let first = !self.heard;
        self.heard = true;
        self.mind = mind;
        // THE CURSORS STAY ON THEIR ITEMS, and inside their lists. The host
        // RANKS the notices — an alarm first, the newest first among equals —
        // so a notice raised above the one under the cursor moved that one
        // down a row, and a cursor kept as a row put `, x` on the new one.
        // Kept as an id, it follows its notice (the rule every list that
        // reorders keeps); only an item that is GONE — a one-shot run, a
        // notice put away — leaves the cursor at its row, inside the list.
        let ids: Vec<String> = self.mind.tasks.iter().map(|t| t.id.clone()).collect();
        self.task = kept(self.task, task_on.as_ref(), &ids);
        let lines: Vec<Option<LogKey>> = self
            .activity_lines()
            .into_iter()
            .map(|l| self.line_key(l))
            .collect();
        self.log = kept(self.log, log_on.map(Some).as_ref(), &lines);
        let notices: Vec<(String, String)> = self.mind.notices.iter().map(notice_key).collect();
        self.notice = kept(self.notice, notice_on.as_ref(), &notices);
        let notices = self.mind.notices.len();
        // THE PAGE OPENS WHERE THE NEWS IS: a first mind with something to
        // say puts the keyboard on the notices — what the strip's `+N` opens
        // the page for, and what anybody opening it with news waiting came
        // for. With nothing to say it stays on the tasks.
        if first && notices > 0 && self.page == Page::Home {
            self.pane = Pane::Notices;
            self.notice = 0;
        }
        // THE NOTICES CAN ALL GO — the last one put away — and a keyboard
        // left in a part of the page that is not drawn is a keyboard that
        // moves nothing anybody can see.
        if notices == 0 && self.pane == Pane::Notices {
            self.pane = Pane::Tasks;
        }
        // AND A FOLD THAT IS NO LONGER IN THE LOG is forgotten, so the list
        // of open ones is bounded by the log itself.
        let folds: Vec<(String, u64)> = self.mind.activity_rows.iter().map(Self::fold_of).collect();
        self.opened.retain(|f| folds.contains(f));
        self.put_back(held.as_ref());
        vec![Effect::Redraw]
    }

    /// Where a click landed, as an index into whichever list is there.
    fn clicked(&mut self, column: u16, line: u16) -> Vec<Effect> {
        // THE FOOTER'S CLICK, where the last frame drew it: it opens the page
        // of the task the footer is about, at once — it is a control, as a
        // header's shortcut is, and not a row to select first.
        if let Some(open) = self.open_at
            && self.blocks.foot.row_at(line) == Some(open.row)
            && (open.from..open.to).contains(&column)
        {
            return self.open_task(open.task);
        }
        // AND THE ROW'S OTHER ACTION, drawn in the same place and the same way.
        if let Some((row, from, to)) = self.also_at
            && self.blocks.foot.row_at(line) == Some(row)
            && (from..to).contains(&column)
        {
            return self.command("brain-row-also");
        }
        // THE WAY BACK atop a task's page, one click like the header's `ESC`.
        if let Some((at, from, to)) = self.back_at
            && self.blocks.body.row_at(line) == Some(at)
            && (from..to).contains(&column)
        {
            self.page = Page::Home;
            return vec![Effect::Redraw];
        }
        // AND THEN THE BLOCK THAT OWNS THE LINE.
        //
        // It used to be `line >= tasks_at` and `line >= log_at`, with no floor
        // under either — so a click in the FOOTER was attributed to the
        // activity log whenever the log reached far enough down the page, and
        // a click below the task list landed on the log before the log began.
        // A block answers `row_at` or it answers `None`, which is the same
        // machine the file manager's `Section::row_at` is.
        // THE NOTICES, row zero being their title.
        if let Some(row) = self.blocks.notices.row_at(line)
            && row >= 1
        {
            let n = self.notices_from + usize::from(row - 1);
            if n < self.mind.notices.len() {
                return self.reach(n, self.pane == Pane::Notices && self.notice == n, |s, n| {
                    s.pane = Pane::Notices;
                    s.notice = n;
                });
            }
        }
        if let Some(row) = self.blocks.body.row_at(line) {
            match self.page {
                // Row zero of the body is the `Tasks` title.
                Page::Home if row >= 1 => {
                    // AND THE WINDOW COUNTS. Without `body_from` the click
                    // named the row's place on the SCREEN, so a scrolled list
                    // selected whatever happened to be that many rows from its
                    // top — which is every list anybody has scrolled.
                    let n = self.body_from + usize::from(row - 1);
                    if n < self.mind.tasks.len() {
                        return self.reach(
                            n,
                            self.pane == Pane::Tasks && self.task == n,
                            |s, n| {
                                s.pane = Pane::Tasks;
                                s.task = n;
                            },
                        );
                    }
                }
                Page::Task => {
                    // THE SAME LIST OF ROWS THE DRAWING WALKED, so the band
                    // and the click cannot land on two different things: the
                    // description is three rows of screen and one row of list.
                    // And under a row the window cut it lands on nothing,
                    // which is what is drawn there.
                    let rows = self.page_rows();
                    if self.page_cut.is_none_or(|cut| row < cut)
                        && let Some(n) = row_at_line(&rows, self.body_from, row)
                        && let Some(spot) = Self::spot_of_row(&rows, n)
                    {
                        return self.reach(spot, self.spot == spot, |s, n| s.spot = n);
                    }
                }
                Page::Log => {
                    let n = self.tail_from + usize::from(row);
                    if n < self.tail.len() {
                        return self.reach(n, self.tail_at == n, |s, n| s.tail_at = n);
                    }
                }
                Page::Home => {}
            }
        }
        if let Some(row) = self.blocks.log.row_at(line)
            && row >= 1
        {
            // Row zero of the log is its own title, inside its ground.
            let n = self.log_from + usize::from(row - 1);
            if n < self.activity_lines().len() {
                return self.reach(n, self.pane == Pane::Log && self.log == n, |s, n| {
                    s.pane = Pane::Log;
                    s.log = n;
                });
            }
        }
        Vec::new()
    }

    /// A click on a row: the first SELECTS, the second does what `RET` does.
    ///
    /// # Why the second click and not the first
    ///
    /// Because it is the rule the file manager already has written down —
    /// *"Clique SELECIONA; o segundo é que abre"* — and the reason is the
    /// footer: this page explains whatever is under the cursor, so the first
    /// click is how somebody reads about a thing before doing anything to it.
    /// A single click that opened would make looking impossible with a mouse.
    fn reach(&mut self, n: usize, again: bool, put: impl Fn(&mut Self, usize)) -> Vec<Effect> {
        if again {
            return self.confirm();
        }
        put(self, n);
        // A HAND MOVED IT, so the window owes it a move — the same thing an
        // arrow asks for. Clicking a row half off the bottom and having the
        // page stay put is the same defect from the other input.
        self.follows = true;
        vec![Effect::Redraw]
    }

    /// The wheel, over whatever list it was over.
    /// A BAR THAT WAS DRAGGED, in the block under the pointer.
    ///
    /// The host answered with a ROW — it built the thumb, so it owns the
    /// inverse — and this puts it on the right list. Absolute, so a long drag
    /// cannot drift away from the hand, and it clears [`State::follows`] for
    /// the reason the wheel does: what moved is the window, not the cursor.
    fn scrolled(&mut self, column: u16, line: u16, first: usize) -> Vec<Effect> {
        let put = |at: &mut usize, room: usize, total: usize| {
            *at = first.min(total.saturating_sub(room.min(total)));
        };
        // THE FOOTER'S SENTENCE FIRST, because its bar is drawn inside the
        // footer's block and a line there belongs to no list. The number is
        // the HOST's — it wrapped the sentence and clamped the offset — so it
        // is taken as it comes. What a row says under itself is a paragraph
        // of its own there, told apart by its own row and cell.
        if let Some(row) = self.blocks.foot.row_at(line) {
            let first = u16::try_from(first).unwrap_or(u16::MAX);
            if self.foot_below_at == Some((row, column)) {
                self.foot_below_scroll = first;
            } else {
                self.foot_scroll = first;
            }
            return vec![Effect::Redraw];
        }
        // AND A TASK'S DESCRIPTION, by its own row AND cell: the page's bar
        // begins on the same row when the description is the first thing on
        // it, and the CELL the host answers with is what tells the two apart
        // — the page's bar is the last, the paragraph's the one before. It
        // could not be moved at all while this was missing: the drag reached
        // the page's list, which did not move either.
        if self.page == Page::Task
            && let Some((at, cell)) = self.about_at
            && self.blocks.body.row_at(line) == Some(at)
            && column == cell
        {
            self.about_scroll = u16::try_from(first).unwrap_or(u16::MAX);
            return vec![Effect::Redraw];
        }
        // AND THE LINES UNDER A ROW, past what the page gives them.
        if self.page == Page::Task
            && let Some((_, _, id)) = self
                .below_at
                .iter()
                .find(|(at, cell, _)| self.blocks.body.row_at(line) == Some(*at) && column == *cell)
        {
            self.below_scroll = (id.clone(), u16::try_from(first).unwrap_or(u16::MAX));
            return vec![Effect::Redraw];
        }
        match self.page {
            Page::Home => {
                let lines = self.activity_lines().len();
                if self.blocks.log.holds(line) && lines > 0 {
                    put(&mut self.log_from, usize::from(LOG_ROWS), lines);
                } else if self.blocks.notices.holds(line) && !self.mind.notices.is_empty() {
                    put(
                        &mut self.notices_from,
                        usize::from(self.blocks.notices.lines.saturating_sub(1)).max(1),
                        self.mind.notices.len(),
                    );
                } else if self.blocks.body.holds(line) && !self.mind.tasks.is_empty() {
                    put(
                        &mut self.body_from,
                        usize::from(self.blocks.body.lines.saturating_sub(1)).max(1),
                        self.mind.tasks.len(),
                    );
                } else {
                    return Vec::new();
                }
            }
            Page::Log => {
                if !self.blocks.body.holds(line) || self.tail.is_empty() {
                    return Vec::new();
                }
                put(
                    &mut self.tail_from,
                    usize::from(self.blocks.body.lines).max(1),
                    self.tail.len(),
                );
            }
            Page::Task => {
                // IN LINES, like everything else about this page: the bar the
                // host built the thumb from counts lines, so the row it
                // answers with is a LINE and the window has to be found from
                // it. See [`row_lines`].
                let rows = self.page_rows();
                if rows.is_empty() {
                    return Vec::new();
                }
                let height = self.blocks.body.lines.max(1);
                let last = last_window(&rows, height);
                self.body_from = rows
                    .iter()
                    .enumerate()
                    .find(|(n, _)| usize::from(lines_before(&rows, *n)) >= first)
                    .map_or(last, |(n, _)| n)
                    .min(last);
            }
        }
        self.follows = false;
        vec![Effect::Redraw]
    }

    /// The wheel: it scrolls the WINDOW and leaves the cursor where it is.
    ///
    /// # Why it stopped moving the cursor
    ///
    /// Because the owner asked for it in one sentence — *"rola a janela"* —
    /// and because it is what every other window in this shell does. A wheel
    /// that moved the cursor moved the FOOTER with it, since the footer is
    /// about whatever the cursor is on: scrolling a list to look at something
    /// changed what the page was explaining, which is a page arguing with the
    /// hand that is reading it.
    ///
    /// The cursor stays, [`State::follows`] is cleared so the draw does not
    /// put the window back, and the last window is what a `Scroll` is built
    /// from — so the bar moves with the wheel, which is the other half of a
    /// control that works.
    fn wheeled(&mut self, line: u16, lines: i16) -> Vec<Effect> {
        let by = |at: usize, room: usize, total: usize| -> usize {
            // THE LAST WINDOW, NEVER THE LAST ROW. A list scrolled to its end
            // shows its final `room` rows; stopping at `total - 1` would
            // scroll past the end into a page of empty ground.
            let most = total.saturating_sub(room);
            if lines < 0 {
                at.saturating_sub(lines.unsigned_abs() as usize)
            } else {
                (at + lines as usize).min(most)
            }
        };
        match self.page {
            Page::Home => {
                // THE BLOCK UNDER THE POINTER SCROLLS, and only that one. The
                // wheel over the footer used to page the activity log.
                let lines = self.activity_lines().len();
                if self.blocks.log.holds(line) && lines > 0 {
                    self.log_from = by(self.log_from, usize::from(LOG_ROWS), lines);
                    self.follows = false;
                    return vec![Effect::Redraw];
                }
                if self.blocks.notices.holds(line) && !self.mind.notices.is_empty() {
                    self.notices_from = by(
                        self.notices_from,
                        usize::from(self.blocks.notices.lines.saturating_sub(1)).max(1),
                        self.mind.notices.len(),
                    );
                    self.follows = false;
                    return vec![Effect::Redraw];
                }
                if self.blocks.body.holds(line) && !self.mind.tasks.is_empty() {
                    self.body_from = by(
                        self.body_from,
                        usize::from(self.blocks.body.lines),
                        self.mind.tasks.len(),
                    );
                    self.follows = false;
                    return vec![Effect::Redraw];
                }
                Vec::new()
            }
            Page::Log => {
                if !self.blocks.body.holds(line) || self.tail.is_empty() {
                    return Vec::new();
                }
                self.tail_from = by(
                    self.tail_from,
                    usize::from(self.blocks.body.lines).max(1),
                    self.tail.len(),
                );
                self.follows = false;
                vec![Effect::Redraw]
            }
            Page::Task => {
                let rows = self.page_rows();
                if rows.is_empty() {
                    return Vec::new();
                }
                // THE WHEEL WALKS ROWS and stops at the last window, which is
                // where the list ends — `by` counts in rows and the ceiling is
                // the one the drawing uses, so the two cannot disagree about
                // where the end is.
                let last = last_window(&rows, self.blocks.body.lines.max(1));
                self.body_from = by(self.body_from, rows.len().saturating_sub(last), rows.len());
                self.follows = false;
                vec![Effect::Redraw]
            }
        }
    }
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "brain".into(),
            mode: "brain".into(),
            states: vec![
                BufferState {
                    id: "normal".into(),
                    letter: 'n',
                    tone: Tone::Standard,
                },
                BufferState {
                    id: "task".into(),
                    letter: 't',
                    tone: Tone::Standard,
                },
                BufferState {
                    id: "log".into(),
                    letter: 'l',
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
            // THE ONE THAT MATTERS: without it the host neither subscribes nor
            // acts, and this buffer is an empty screen. It is declared here,
            // once, and the host checks it on every request.
            capabilities: vec![Capability::Brain],
            // THE WORDS THE HEADER MEASURES. They are still drawn as KEYS —
            // the host translates them, this side never does (invariant 11) —
            // and what the text is for is the WIDTH. Without it a phrase has
            // to be given a fixed column, because a key's length says nothing
            // about the sentence's, and the owner read thirteen-cell columns
            // as shortcuts scattered across the line.
            phrases: every_phrase(),
        }
    }

    fn render(ctx: Context) -> Vec<ronoctua_design::types::Command> {
        STATE.with_borrow_mut(|s| s.compose(&ctx).finish_wit())
    }

    fn event(e: Input) -> Vec<Effect> {
        STATE.with_borrow_mut(|s| {
            let before = s.state_id();
            let mut out = s.act(e);
            let after = s.state_id();
            if before != after {
                out.push(Effect::StateChanged(after.into()));
            }
            // AND WHICH MODULE PAGE TO FOLLOW, when the page in front changed
            // — one place, after every input, so no door into a task's page
            // can forget to ask.
            out.extend(s.watch_asks());
            out
        })
    }
}

impl State {
    /// THE WHOLE FRAME, and the only place it is laid out.
    ///
    /// `render` calls it and so do the tests. It used to be written out twice
    /// — once in `render` and once in a test helper — and the two were four
    /// lines apart in what they did, which is a test proving a screen nobody
    /// is shown.
    fn compose(&mut self, ctx: &Context) -> Screen<Wit> {
        {
            let s = self;
            s.lines = ctx.lines;
            s.columns = ctx.columns;
            s.headed = ctx.headed;
            let mut screen = Screen::<Wit>::of(ctx);

            // THE WINDOW IS DIVIDED FIRST, AND THE BLOCKS TILE IT.
            //
            // This used to be two loose numbers — `from` and `to` — threaded
            // through six drawing functions, each of which added the block's
            // first line by hand. Every defect the owner photographed was one
            // of those additions reaching past the block it belonged to: a
            // footer drawn on top of a history list, and a strip of the page's
            // own ground between the footer and the bottom edge.
            //
            // The heights are decided here, against the window, so the sum is
            // exactly the window and no block has to be clamped afterwards.
            // The ORDER is the order of preference when the window is short:
            // the header and the warning are what the page is, the footer
            // explains what is in focus, the activity log is the first thing
            // worth losing, and the body takes what is left.
            let told = s.told();
            let mut left = ctx.lines;
            // THE HEADER IS THE HOST'S, drawn above these rows (`declare`):
            // the window begins with the warning.
            let warn = u16::from(s.warned()).min(left);
            left -= warn;
            // FIXED, and not `told.rows(…)`: the height no longer answers to
            // what is under the cursor. `min(left)` is the only thing that can
            // shorten it, and that is the window being too small rather than
            // the page changing shape.
            let foot = FOOT_LINES.min(left);
            left -= foot;
            // THE NOTICES, on the home page, as tall as what they hold up to
            // [`NOTICE_ROWS`] and their title — and nothing at all when no
            // module has anything to say. After the footer in the order of
            // preference and before the log: what a module wants said now is
            // worth more than a record of what already happened.
            let notices = if s.page == Page::Home && !s.mind.notices.is_empty() {
                let want = 1 + u16::try_from(s.mind.notices.len())
                    .unwrap_or(u16::MAX)
                    .min(NOTICE_ROWS);
                // ONE LINE IS KEPT FOR THE BODY, as the log keeps it below.
                if left > want { want } else { 0 }
            } else {
                0
            };
            left -= notices;
            // THE LOG IS ALL OR NOTHING. A container with its title and two
            // rows is not a container, it is a list with a lid; below its
            // whole height it is worth more to the body. One row is kept back
            // for the body, because a page with no body at all is a page that
            // says nothing.
            let log = if s.page == Page::Home && left > LOG_LINES {
                LOG_LINES
            } else {
                0
            };
            left -= log;

            let blocks = screen.split(&[
                Slice::Fixed(warn),
                Slice::Fixed(notices),
                Slice::Rest,
                Slice::Fixed(log),
                Slice::Fixed(foot),
            ]);
            s.blocks = Blocks {
                warn: blocks[0],
                notices: blocks[1],
                body: blocks[2],
                log: blocks[3],
                foot: blocks[4],
            };
            let _ = left;

            // WHAT THE LAST FRAME PUT WHERE, forgotten: only the page that
            // draws them now says where they are.
            s.back_at = None;
            s.about_at = None;
            s.below_at.clear();
            s.legend_at.clear();
            s.declare(&mut screen);
            s.warning(&mut screen);
            s.draw_notices(&mut screen);
            match s.page {
                Page::Home => s.home(&mut screen),
                Page::Task => s.task_page(&mut screen),
                Page::Log => s.log_page(&mut screen),
            }
            s.draw_log(&mut screen);
            s.foot(&mut screen, &told);
            // AND WHAT THE POINTER RESTS ON, asked of what was just drawn.
            if let Some(about) = s.pointed() {
                screen.point(about);
            }
            screen
        }
    }

    /// One input, answered.
    fn act(&mut self, e: Input) -> Vec<Effect> {
        match e {
            Input::EnterState(id) => {
                self.page = match id.as_str() {
                    "task" => Page::Task,
                    "log" => Page::Log,
                    _ => Page::Home,
                };
                self.spot = 0;
                self.body_from = 0;
                vec![Effect::Redraw]
            }
            Input::Mind(mind) => self.received(mind),
            Input::Log(lines) => self.logged(lines),
            Input::Command(id) => self.command(&id),
            Input::Down => {
                let most = self.walkable().saturating_sub(1);
                let at = self.cursor();
                if *at < most {
                    *at += 1;
                }
                // A PERSON MOVED IT, so the window owes it a move.
                self.follows = true;
                vec![Effect::Redraw]
            }
            Input::Up => {
                let at = self.cursor();
                *at = at.saturating_sub(1);
                self.follows = true;
                vec![Effect::Redraw]
            }
            // `Tab` CROSSES THE TWO HALVES of the home page, which is the ring
            // the file manager's sections already are. A task's page has one
            // cursor and nowhere to cross to, so it answers nothing.
            Input::Toggle => {
                if self.page == Page::Home {
                    self.pane = self.next_pane();
                }
                vec![Effect::Redraw]
            }
            Input::Confirm => self.confirm(),
            Input::Cancel => {
                if matches!(self.page, Page::Task | Page::Log) {
                    self.page = Page::Home;
                    return vec![Effect::Redraw];
                }
                // NOWHERE TO GO BACK TO: `Esc` closes the window, as it closes
                // every window — and the host asks first, a second `Esc`
                // within its time (`Ui::closing`).
                vec![Effect::Close]
            }
            Input::Click(p) => self.clicked(p.column, p.line),
            Input::Wheel(w) => self.wheeled(w.at.line, w.lines),
            // A BAR DRAGGED. The host turned the pointer's height into a row;
            // this side says which LIST it belongs to, which is the half only
            // it knows. Same routing as the wheel, one line above.
            Input::Scrolled(d) => self.scrolled(d.at.column, d.at.line, d.first as usize),
            // THE POINTER AT REST, and a frame only when it MOVED to another
            // cell. The host already sends one per cell rather than per pixel;
            // this is the second half of the same economy — a hover that
            // redrew on every arrival would repaint whenever the pointer
            // jittered inside one cell.
            //
            // PER CURSOR: `p.seat` says which hand, each keeps its own cell,
            // and one leaving puts out only its own.
            Input::Hover(p) => {
                if !self.hover.set(p.seat, (p.at.column, p.at.line)) {
                    return Vec::new();
                }
                vec![Effect::Redraw]
            }
            Input::Unhover(seat) => {
                if !self.hover.clear(seat) {
                    return Vec::new();
                }
                vec![Effect::Redraw]
            }
            // ONE SELECTION FOR EVERY KEYBOARD: this buffer keeps a single
            // cursor, which every keyboard typing here moves.
            Input::KeyboardSeat(_) | Input::KeyboardLeft(_) => Vec::new(),
            // THE WORDS FOR THE KEYS THIS BUFFER TEACHES, so the header can
            // measure them. They arrive once, before the first frame.
            Input::Phrases(said) => {
                self.phrases = Phrases::of(said.into_iter().map(|p| (p.key, p.text)).collect());
                vec![Effect::Redraw]
            }
            // A LETTER IS TEXT, and this page takes none: its keys are chords
            // (`C-r`), which the host turns into this page's commands.
            Input::Left
            | Input::Right
            | Input::Character(_)
            | Input::Delete
            | Input::Copied(_)
            | Input::Paste(_)
            | Input::Survey(_)
            | Input::Queue(_)
            | Input::Rack(_)
            | Input::Formatted(_)
            | Input::Keyboards(_) => Vec::new(),
            // A MODULE'S OWN PAGE, pushed when its generation moved.
            Input::ModuleScreen(screen) => self.screened(screen),
        }
    }
}

ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_design::types::{
        Announcement, DayPart, Gate, Heights, Pulse, RunRecord, TaskStep,
    };

    fn a_run(task: &str, outcome: OutcomeKind) -> RunRecord {
        RunRecord {
            task: task.to_owned(),
            // AS THE HOST WOULD HAVE RESOLVED IT. The field is measured, so a
            // fixture with an empty one would exercise the fallback and never
            // the arithmetic it exists for.
            task_text: task.to_owned(),
            began: 1_000,
            ended: 1_060,
            when_text: "2026-08-23 13:00".to_owned(),
            // AND CUT WHERE ITS DAY IS, as the host would have cut it — a
            // day that is not today.
            when_stamp: ronoctua_design::types::Stamp {
                lead: "2026-08-".to_owned(),
                day: "23".to_owned(),
                tail: " 13:00".to_owned(),
                today: false,
            },
            outcome,
            at: String::new(),
            why: String::new(),
            trouble: String::new(),
            steps: Vec::new(),
            // THE KEYS AS THE HOST WOULD HAVE SENT THEM. Fictional ids: the
            // buffer draws whatever key it is handed and builds none.
            task_key: format!("brain-task-{task}"),
            at_key: String::new(),
            why_key: String::new(),
            says: String::new(),
            says_text: String::new(),
        }
    }

    /// The records, NEWEST FIRST, folded the way the brain folds them — the
    /// same task, outcome, step, reason and sentence in a row are one row.
    fn folded(runs: &[RunRecord]) -> Vec<ActivityRow> {
        let mut out: Vec<ActivityRow> = Vec::new();
        for r in runs {
            let same = out.last().is_some_and(|g| {
                g.newest.task == r.task
                    && g.newest.outcome == r.outcome
                    && g.newest.at == r.at
                    && g.newest.why == r.why
                    && g.newest.says == r.says
            });
            if same {
                let g = out.last_mut().expect("a row");
                g.repeats += 1;
                g.first_text = r.when_text.clone();
                g.members.push(r.clone());
            } else {
                out.push(ActivityRow {
                    newest: r.clone(),
                    repeats: 1,
                    last_text: r.when_text.clone(),
                    first_text: r.when_text.clone(),
                    members: vec![r.clone()],
                });
            }
        }
        out
    }

    /// A notice that offers all three — a control, putting it away, and
    /// putting it off — because that is the shape with the most runs on its
    /// row and the most rungs in the header.
    fn a_notice(module: &str) -> Notice {
        Notice {
            module: module.to_owned(),
            id: "due".to_owned(),
            says: "fixture-says-due".to_owned(),
            says_text: "The fixture is due, and it has been for a while now".to_owned(),
            args: Vec::new(),
            level: NoticeLevel::Warn,
            since: 1_400,
            since_text: "2026-08-24 13:50".to_owned(),
            controls: vec![ronoctua_design::types::ModuleAction {
                id: "keep".to_owned(),
                label_key: "cmd-fixture-keep".to_owned(),
                row: false,
                asks_key: String::new(),
            }],
            dismissable: true,
            snooze: vec![10, 60],
        }
    }

    fn a_step(id: &str, on: bool, ships_on: bool, root: bool) -> TaskStep {
        TaskStep {
            id: id.to_owned(),
            command: "nix flake update".to_owned(),
            needs_root: root,
            on,
            on_by_default: ships_on,
            name_key: format!("fixture-step-{id}"),
            does_key: format!("fixture-step-{id}-does"),
        }
    }

    fn a_task(id: &str, kind: TaskKind) -> TaskState {
        TaskState {
            id: id.to_owned(),
            name_text: id.to_owned(),
            kind,
            standing: Standing::Idle,
            enabled: true,
            interval_days: 7,
            part_of_day: DayPart::Afternoon,
            next: 2_000,
            next_text: "2026-08-24 14:00".to_owned(),
            hold: HoldKind::None,
            why_not: String::new(),
            ran: true,
            last: a_run(id, OutcomeKind::Done),
            history: vec![a_run(id, OutcomeKind::Done), a_run(id, OutcomeKind::Failed)],
            steps: vec![
                a_step("a-root-step", true, true, true),
                a_step("a-step-of-mine", true, true, false),
            ],
            calendar: true,
            name_key: format!("brain-task-{id}"),
            does_key: format!("brain-task-{id}-does"),
            brief_key: format!("brain-task-{id}-brief"),
            does_text: String::new(),
            why_key: String::new(),
            why_text: String::new(),
            screen_generation: 0,
            actions: Vec::new(),
            folds: Vec::new(),
        }
    }

    fn a_mind() -> Mind {
        Mind {
            now: 1_500,
            // THE PULSE CROSSES AND THIS SCREEN NO LONGER DRAWS IT. The
            // readings were a footer fact under every task; the owner had them
            // taken out. What still reads the pulse is the daemon, which is
            // where the decision to hold a task is made.
            pulse: Pulse {
                at: 1_500,
                signs: Vec::new(),
            },
            tasks: vec![
                a_task("fixture-routine", TaskKind::Routine),
                a_task("fixture-once", TaskKind::Once),
            ],
            queue: Vec::new(),
            running: String::new(),
            rehearsing: String::new(),
            announcement: Announcement {
                task: String::new(),
                total_seconds: 0,
                deadline: 0,
                why: String::new(),
            },
            gate: Gate {
                ram_ceiling_pct: 70,
                cpu_stall_per_mille: 200,
                abort_on_battery: true,
            },
            cadence: vec![1, 7, 30],
            // A NOTICE WITH ALL THREE CONTROLS, because that is the shape with
            // the most runs on its row and the most rungs in the header.
            notices: vec![a_notice("fixture-routine")],
            // A ROW THAT STANDS ALONE AND ONE THAT FOLDS THREE, because the
            // two draw different columns: the fold's mark and its count
            // exist only for the second.
            activity_rows: folded(&[
                a_run("fixture-routine", OutcomeKind::Done),
                a_run("fixture-once", OutcomeKind::Held),
                a_run("fixture-once", OutcomeKind::Held),
                a_run("fixture-once", OutcomeKind::Held),
            ]),
            activity_reach: "last 15 days".to_owned(),
            history_reach: "last 20 days".to_owned(),
        }
    }

    /// A capture that came back suspiciously small is NOISE, not an answer —
    /// the `floor` the check prelude applies to a sweep, here.
    fn floor_of(many: usize, least: usize, what: &str) {
        assert!(
            many >= least,
            "only {many} {what}; the test stopped reaching them"
        );
    }

    fn a_state() -> State {
        let mut s = State::fresh();
        s.received(a_mind());
        // THE TESTS OF THE TASKS START ON THE TASKS. A first mind with news
        // puts the keyboard on the notices — `the_page_opens_where_the_news_is`
        // is about that — and every other test here is about something else.
        s.pane = Pane::Tasks;
        let _ = s.logged(a_tail(1, 40));
        s
    }

    /// Lines of the daemon's log numbered `from` onwards, every level among
    /// them, and some longer than any window.
    fn a_tail(from: u64, many: u64) -> Vec<LogLine> {
        (from..from + many)
            .map(|seq| LogLine {
                seq,
                at_text: format!("12:00:{:02}", seq % 60),
                level: match seq % 5 {
                    0 => LogLevel::Error,
                    1 => LogLevel::Warn,
                    2 => LogLevel::Debug,
                    _ => LogLevel::Info,
                },
                text: format!(
                    "ronoctua_brain::runner: tick ram_pct={seq} running=\"\" {}",
                    "and a long tail of fields ".repeat(usize::try_from(seq % 4).unwrap_or(0))
                ),
            })
            .collect()
    }

    /// A LIVE LOG STAYS ON ITS NEWEST LINE while the cursor is on it, and a
    /// line somebody went up to read stays under the cursor as new ones come
    /// and old ones fall off the front of the ring.
    #[test]
    fn the_log_rides_the_newest_line_until_somebody_reads_up() {
        let mut s = a_state();
        let _ = s.command("brain-log");
        assert_eq!(s.page, Page::Log);
        assert_eq!(s.tail_at, 39, "it did not open on the newest line");
        let _ = s.logged(a_tail(1, 45));
        assert_eq!(s.tail_at, 44, "a new line did not take the cursor with it");

        let _ = s.act(Input::Up);
        let _ = s.act(Input::Up);
        let read = s.tail[s.tail_at].seq;
        // TEN NEW LINES, and the ring drops the ten oldest.
        let _ = s.logged(a_tail(11, 45));
        assert_eq!(
            s.tail[s.tail_at].seq, read,
            "the line being read moved away"
        );
    }

    /// `, l` OPENS THE LOG AND CLOSES IT, and `Esc` comes back from it.
    #[test]
    fn the_log_opens_and_closes_by_one_key() {
        let mut s = a_state();
        let _ = s.command("brain-log");
        assert_eq!(s.state_id(), "log");
        let _ = s.command("brain-log");
        assert_eq!(s.page, Page::Home);
        let _ = s.command("brain-log");
        let _ = s.act(Input::Cancel);
        assert_eq!(s.page, Page::Home);
    }

    /// THE FOOTER HOLDS THE WHOLE LINE, in its level's tone: the row cuts it
    /// at the width, and this is where it is read to the end.
    #[test]
    fn the_footer_holds_the_whole_line() {
        let mut s = a_state();
        let _ = s.command("brain-log");
        s.tail_at = 3;
        let told = s.told();
        let (text, role) = told.logged.expect("the footer says nothing about the line");
        assert_eq!(text, s.tail[3].text);
        assert_eq!(role, level_role(s.tail[3].level));
        assert!(told.said.expect("no time").starts_with(&s.tail[3].at_text));
    }

    /// A CLICK SELECTS THE LINE UNDER IT, counted from the window and not from
    /// the top of the log.
    #[test]
    fn a_click_selects_a_log_line_from_the_window() {
        let mut s = a_state();
        let _ = s.command("brain-log");
        let _ = drawn(&mut s, 24, 100);
        let from = s.tail_from;
        assert!(from > 0, "the fixture is meant to scroll");
        let line = s.blocks.body.top + 2;
        let _ = s.act(Input::Click(ronoctua_design::types::Position {
            column: 20,
            line,
        }));
        assert_eq!(s.tail_at, from + 2);
    }

    /// FAILS IF: the header the HOST draws is not told the window's title and
    /// what the page teaches — in the order `teaches` gives them, each with
    /// its sentence and whether it does anything — or a bare letter typed
    /// here reaches a command.
    #[test]
    fn the_header_is_declared_for_the_host() {
        use ronoctua_design::frame::Command;
        let mut s = a_state();
        let screen = drawn(&mut s, 30, 140);
        let list = screen.as_protocol();
        assert!(
            list.commands.iter().any(|c| matches!(
                c,
                Command::Heading { title: ronoctua_design::frame::Text::Key(k), .. } if k == key::TITLE
            )),
            "the window's title was not declared"
        );
        let taught: Vec<(String, String, String, bool)> = list
            .commands
            .iter()
            .filter_map(|c| match c {
                Command::Taught {
                    keys,
                    says,
                    about,
                    control,
                } => Some((keys.clone(), says.clone(), about.clone(), *control)),
                _ => None,
            })
            .collect();
        let wanted: Vec<(String, String, String, bool)> = s
            .teaches()
            .into_iter()
            .map(|(keys, says)| {
                (
                    keys.to_owned(),
                    says.to_owned(),
                    about_of(says).to_owned(),
                    is_a_control(keys),
                )
            })
            .collect();
        assert_eq!(taught, wanted);
        assert!(
            taught
                .iter()
                .any(|(k, _, a, _)| k == ", r" && a == key::ABOUT_RUN),
            "{taught:?}"
        );
        // NOTHING OF THE HEADER IS DRAWN IN THE BUFFER'S OWN ROWS.
        assert!(
            !list.commands.iter().any(|c| matches!(
                c,
                Command::Text { text: ronoctua_design::frame::Text::Key(k), .. } if k == "cmd-brain-run"
            )),
            "the buffer drew a taught pair itself"
        );
        // AND A BARE LETTER IS TEXT, which reaches no command.
        assert!(s.act(Input::Character('r')).is_empty());
    }

    fn a_context(lines: u16, columns: u16) -> Context {
        Context {
            columns,
            lines,
            heights: Heights {
                small: 1,
                base: 1,
                normal: 1,
                large: 2,
                huge: 3,
            },
            headed: true,
        }
    }

    /// The frame, drawn exactly as `render` draws it.
    ///
    /// It returns the `Screen` rather than the list so the caller can ask
    /// `as_protocol` — which is how the host's own two questions get asked on
    /// this side, in milliseconds, with the same arithmetic.
    fn drawn(s: &mut State, lines: u16, columns: u16) -> Screen<Wit> {
        s.compose(&a_context(lines, columns))
    }

    /// Where a step sits in the spot list, found rather than counted.
    ///
    /// The two tests below used to write the index out — `spot = 5` for the
    /// first step — and the day the schedule gained a row above them both
    /// failed while the screen was right. Neither test is about how many rows
    /// come first, so neither should have to know.
    fn on_step(s: &State, n: usize) -> usize {
        s.spots()
            .iter()
            .position(|spot| *spot == Spot::Step(n))
            .expect("the page draws that step")
    }

    /// Every command in the keymap does something, and every key the header
    /// teaches is a key the keymap has.
    ///
    /// THE DEFECT THIS CATCHES is the one the owner reported twice: `m r` and
    /// `m o` existed, did what they said, and appeared nowhere on the screen.
    /// A key that is not taught is a key nobody has.
    #[test]
    fn every_key_is_taught_and_every_taught_key_exists() {
        let mut s = a_state();
        // ON A TASK'S PAGE, with the cursor on a step of the person's: every
        // command in the keymap has something to act on there, which is the
        // only place all ten of them do.
        s.page = Page::Task;
        s.spot = on_step(&s, 1);
        // THE ROW'S SECOND ACTION acts on a row of the module's page, never
        // on a step: `a_task_page_follows_its_module_screen` asks it there.
        for (_, id) in KEYMAP.iter().filter(|(_, id)| *id != "brain-row-also") {
            assert!(
                !s.command(id).is_empty(),
                "`{id}` is in the keymap and does nothing"
            );
        }
        assert!(s.command("brain-does-not-exist").is_empty());

        let taught: Vec<&str> = HOME_TEACHES
            .iter()
            .chain(TASK_TEACHES)
            .chain(ANSWER_TEACHES)
            .chain(NOTICE_TEACHES)
            .chain(ALSO_TEACHES)
            .map(|(keys, _)| *keys)
            .collect();
        for (keys, id) in KEYMAP {
            assert!(
                taught.contains(keys),
                "`{keys}` ({id}) is bound and no page teaches it"
            );
        }
        for keys in &taught {
            if keys.starts_with("m ") {
                assert!(
                    KEYMAP.iter().any(|(k, _)| k == keys),
                    "the header teaches `{keys}`, which is bound to nothing"
                );
            }
        }
    }

    /// Nothing is drawn past the last line of the window.
    ///
    /// The compositor does not clip: a run at `line >= lines` is simply drawn
    /// off the surface, and the first sign of it is a screenshot.
    #[test]
    fn nothing_is_drawn_outside_the_window() {
        for lines in [6_u16, 10, 14, 24, 40] {
            for columns in [46_u16, 80, 100, 140] {
                for page in [Page::Home, Page::Task, Page::Log] {
                    let mut s = a_state();
                    s.page = page;
                    let screen = drawn(&mut s, lines, columns);
                    let heights = screen.heights();
                    let list = screen.as_protocol();
                    // THE HEIGHT NOW COUNTS THE WHOLE EXTENT. It used to be
                    // the first row of the lowest command, so a footer whose
                    // paragraph ran four rows past the bottom answered with a
                    // height inside the window and this test was green while
                    // the owner was photographing the overflow.
                    assert!(
                        list.height(heights) <= lines,
                        "{page:?} at {lines}x{columns} drew {} lines",
                        list.height(heights)
                    );
                    list.nothing_is_drawn_below(heights, lines)
                        .unwrap_or_else(|e| panic!("{page:?} at {lines}x{columns}: {e}"));
                }
            }
        }
    }

    /// FAILS IF: the frame does not pass the screen's OWN check.
    ///
    /// It asks three things this file would otherwise ask piecemeal: that the
    /// blocks tile the window, that no pen turned a draw away, and that no run
    /// was put in a column at a size that does not advance one cell.
    ///
    /// The last is the one the owner photographed. A column budgets CELLS and
    /// `Scale::Small` advances 0,81 of one, so the next column starts where the
    /// BUDGET ends and not where the ink stopped — a gap proportional to the
    /// phrase, which across a row of shortcuts reads as *"espaços
    /// desproporcionais"* and puts more air on one side of a separator than the
    /// other. Measured on that screen: 3,2 cells before the dot, 1,6 after it.
    #[test]
    fn the_frame_passes_its_own_check() {
        for lines in [6_u16, 10, 14, 24, 40] {
            for columns in [46_u16, 80, 100, 140] {
                for page in [Page::Home, Page::Task, Page::Log] {
                    let mut s = a_state();
                    s.page = page;
                    let screen = drawn(&mut s, lines, columns);
                    screen
                        .check()
                        .unwrap_or_else(|e| panic!("{page:?} at {lines}x{columns}: {e}"));
                }
            }
        }
    }

    /// And nothing is covered, and nothing overlaps.
    #[test]
    fn nothing_overlaps_anything() {
        for lines in [10_u16, 24, 40] {
            for columns in [60_u16, 100, 140] {
                for page in [Page::Home, Page::Task, Page::Log] {
                    for held in [false, true] {
                        let mut s = a_state();
                        s.page = page;
                        if held {
                            // THE OTHER SHAPE OF THE PAGE: a warning line, a
                            // folded row opened, and a watch among the tasks —
                            // each a different key and a different budget.
                            s.mind.tasks[0].hold = HoldKind::Cpu;
                            s.mind.tasks[1].kind = TaskKind::Watch;
                            let fold = State::fold_of(&s.mind.activity_rows[1]);
                            s.opened.push(fold);
                        }
                        // `Screen::check` ASKS BOTH of the host's questions with
                        // the heights it was built from — which is the whole
                        // reason it exists on this side: the same arithmetic, in
                        // milliseconds, instead of a VM and a photograph.
                        drawn(&mut s, lines, columns).check().unwrap_or_else(|e| {
                            panic!("{page:?} at {lines}x{columns} (held={held}): {e}")
                        });
                    }
                }
            }
        }
    }

    /// THE BAND AND THE FOOTER ARE ABOUT THE SAME THING.
    ///
    /// They disagreed, and the reason is worth keeping: the page is drawn from
    /// a list of ROWS — blanks and headings included — and the cursor counts
    /// SPOTS. Comparing one index against the other put the band on the fourth
    /// row and the description of the seventh underneath it.
    #[test]
    fn the_band_and_the_footer_agree() {
        let mut s = a_state();
        s.page = Page::Task;
        let task = s.here().cloned().expect("a task");
        let rows = s.rows_of(&task, 100);
        for spot in 0..s.spots().len() {
            s.spot = spot;
            let here = s.spot_here().expect("a spot");
            let at = State::row_of_spot(&rows, here).expect("the spot is drawn");
            // The n-th spot is the n-th drawable row, counting from the front.
            let nth = rows.iter().take(at).filter(|r| is_a_spot(r)).count();
            assert_eq!(nth, spot, "spot {spot} is drawn as the {nth}-th");
        }
    }

    /// A HEADING IS NEVER DRAWN OVER NOTHING.
    ///
    /// Two of the four tasks name a unit and declare no steps, and both drew
    /// `Steps` with an empty space under it — which the owner found and called
    /// what it is.
    #[test]
    fn a_task_with_no_steps_has_no_steps_heading() {
        let s = a_state();
        let mut bare = a_task("fixture-once", TaskKind::Routine);
        bare.steps = Vec::new();
        let rows = s.rows_of(&bare, 100);
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, Row::Heading(k) if *k == key::STEPS)),
            "a task with no steps drew the steps heading"
        );
        // And one WITH steps still has it.
        let full = a_task("fixture-routine", TaskKind::Routine);
        assert!(
            s.rows_of(&full, 100)
                .iter()
                .any(|r| matches!(r, Row::Heading(k) if *k == key::STEPS))
        );
    }

    /// THE WARNING LINE EXISTS ONLY WHILE SOMETHING IS IN THE WAY.
    #[test]
    fn a_well_machine_draws_no_warning() {
        let ctx = a_context(24, 100);
        let mut s = a_state();
        // A NOTICE IS NOT A WARNING any more: it has a block of its own, with
        // its controls, and the line is left to the one question a notice
        // does not answer — why has nothing happened.
        assert!(!s.mind.notices.is_empty());
        assert!(!s.warned(), "a warning with nothing wrong");

        s.mind.tasks[0].hold = HoldKind::Cpu;
        assert!(s.warned(), "the processor is busy and nothing said so");

        // AND THE BLOCK FOLLOWS THE ANSWER. The height is decided from
        // `warned` before the window is divided, so a frame that says there is
        // nothing wrong must not reserve a line for saying it.
        let _ = ctx;
        let mut quiet = a_state();
        for t in &mut quiet.mind.tasks {
            t.hold = HoldKind::None;
        }
        drawn(&mut quiet, 24, 100);
        assert_eq!(quiet.blocks.warn.lines, 0, "a line reserved for no news");
        let mut loud = a_state();
        loud.mind.tasks[1].hold = HoldKind::Ram;
        drawn(&mut loud, 24, 100);
        assert_eq!(
            loud.blocks.warn.lines, 1,
            "something is wrong and no line says so"
        );
    }

    /// A step nobody touched is not marked as changed.
    ///
    /// The same defect the daemon's own test pins, seen from the other side:
    /// `changed` is `on != on_by_default`, and a step that ships off and is
    /// off differs from neither.
    #[test]
    fn a_step_that_ships_off_and_is_off_is_not_changed() {
        let off = a_step("a-step-that-ships-off", false, false, false);
        assert_eq!(off.on, off.on_by_default);
        let flipped = a_step("a-step-that-ships-off", true, false, false);
        assert_ne!(flipped.on, flipped.on_by_default);
    }

    /// THE FLAKE'S STEPS REFUSE, and refuse silently on the wire.
    ///
    /// The person's switch lives in their own state directory and the root
    /// half runs as root, which has no way to find it. A knob sent for one of
    /// those would write an entry nothing can read — the exact shape of the
    /// defect this page was opened to fix.
    #[test]
    fn a_root_step_does_not_send_a_knob() {
        let mut s = a_state();
        s.page = Page::Task;
        // The first step is the root one, and the second is the person's.
        s.spot = on_step(&s, 0);
        assert!(
            s.flip_step().is_empty(),
            "a step the flake owns sent a preference anyway"
        );
        s.spot = on_step(&s, 1);
        assert!(!s.flip_step().is_empty(), "a step of the person's refused");
    }

    /// The cursor never outlives the list it is in.
    #[test]
    fn the_cursor_stays_inside_its_list() {
        let mut s = a_state();
        s.task = 1;
        s.log = 1;
        let mut gone = a_mind();
        gone.tasks.truncate(1);
        gone.activity_rows.clear();
        gone.notices.clear();
        s.pane = Pane::Notices;
        s.received(gone);
        assert_eq!(s.task, 0);
        assert_eq!(s.log, 0);
        // AND THE KEYBOARD LEAVES A PART OF THE PAGE THAT IS NO LONGER DRAWN.
        assert_eq!(
            s.pane,
            Pane::Tasks,
            "the keyboard stayed on notices that went"
        );
    }

    /// `Tab` walks the home page's ring — the notices only while there are
    /// any — and does nothing on a task's.
    #[test]
    fn tab_crosses_the_two_halves() {
        let mut s = a_state();
        assert_eq!(s.pane, Pane::Tasks);
        s.act(Input::Toggle);
        assert_eq!(s.pane, Pane::Log);
        s.act(Input::Toggle);
        assert_eq!(s.pane, Pane::Notices);
        s.act(Input::Toggle);
        assert_eq!(s.pane, Pane::Tasks);
        // WITH NOTHING SAID, the ring is the two halves again.
        s.mind.notices.clear();
        s.act(Input::Toggle);
        s.act(Input::Toggle);
        assert_eq!(
            s.pane,
            Pane::Tasks,
            "Tab went to notices that are not there"
        );
        s.page = Page::Task;
        s.act(Input::Toggle);
        assert_eq!(s.pane, Pane::Tasks, "a task's page has nowhere to cross to");
    }

    /// The wheel scrolls the WINDOW of the list it is over, and leaves the
    /// cursor where it is.
    ///
    /// FAILS IF: the wheel goes back to moving the cursor. It did, and the
    /// owner asked for the other one in one sentence — *"rola a janela"*. The
    /// difference is not cosmetic: the footer is about whatever the cursor is
    /// on, so a wheel that moved the cursor changed what the page was
    /// explaining while somebody was reading it.
    #[test]
    fn the_wheel_moves_the_window_and_not_the_cursor() {
        let mut s = a_state();
        // A LIST LONGER THAN ITS WINDOW, or there is nothing to scroll: the
        // offset is clamped to the last window, and with two rows in eight
        // that is zero.
        let runs: Vec<RunRecord> = (0..40)
            .map(|n| a_run(&format!("task-{n}"), OutcomeKind::Done))
            .collect();
        s.mind.activity_rows = folded(&runs);
        drawn(&mut s, 24, 100);
        let log_at = s.blocks.log.top + 1;
        assert!(!s.blocks.log.empty(), "the log was not drawn");
        let cursor = s.log;
        s.wheeled(log_at, 1);
        assert_eq!(s.log_from, 1, "the window did not move");
        assert_eq!(s.log, cursor, "the wheel moved the cursor");
        assert!(
            !s.follows,
            "the wheel left the window owing the cursor a move"
        );
        // AND THE DRAW LEAVES IT THERE. A follow that did not ask would put
        // the window back on the very next frame, which is what made the old
        // shape of this look like a wheel that did nothing.
        drawn(&mut s, 24, 100);
        assert_eq!(s.log_from, 1, "the draw pulled the window back");
        s.wheeled(log_at, -1);
        assert_eq!(s.log_from, 0, "the wheel would not come back");
    }

    /// OFF BY THE SYSTEM IS SAID WITH ITS REASON, and the row offers no key.
    ///
    /// THE DEFECT THIS CATCHES: a module the system's configuration switched
    /// off read "No, only when you ask" beside a key to switch it back on —
    /// two promises the brain refuses. The person's own "off" still reads as
    /// it did, with its key.
    #[test]
    fn a_module_off_for_a_reason_not_yours_says_it_and_offers_no_switch() {
        let mut s = a_state();
        s.page = Page::Task;
        s.task = 0;
        let task = &mut s.mind.tasks[0];
        task.enabled = false;
        task.why_not = "fixture-reason".to_owned();
        task.why_key = "brain-why-fixture-reason".to_owned();
        assert_eq!(
            off_not_by_you(&s.mind.tasks[0]).as_deref(),
            Some("brain-why-fixture-reason")
        );
        let (keys, data) = drawn_texts(&drawn(&mut s, 30, 120));
        assert!(
            keys.iter().any(|k| k == "brain-why-fixture-reason"),
            "the reason is not on the page: {keys:?}"
        );
        assert!(
            !keys.iter().any(|k| k == key::OFF),
            "it still reads as the person's own off"
        );
        let switch = key_of_command("brain-switch").expect("the switch has a key");
        let switches = |data: &[String]| data.iter().filter(|d| *d == switch).count();
        assert_eq!(
            switches(&data),
            0,
            "the page offers a switch the brain refuses"
        );

        // NOTHING THE BRAIN REFUSES IS TAUGHT OR DRAWN: the header keeps the
        // way back, the page keeps the row that says why
        // and the history, and the home row does not call it "waiting".
        const REFUSED: [&str; 7] = ["RET", ", r", ", e", ", o", ", i", ", w", ", d"];
        let taught = s.teaches();
        assert!(
            taught.iter().all(|(k, _)| !REFUSED.contains(k)),
            "the header teaches a key the brain refuses: {taught:?}"
        );
        assert!(taught.iter().any(|(k, _)| *k == "ESC"), "no way back");
        // ON THE HOME PAGE `RET` STILL OPENS the page that says why.
        s.page = Page::Home;
        let home = s.teaches();
        assert!(
            home.iter().any(|(k, _)| *k == "RET"),
            "the page cannot be opened"
        );
        assert!(home.iter().all(|(k, _)| !REFUSED[1..].contains(k)));
        s.page = Page::Task;
        let history = s.mind.tasks[0].history.len();
        let mut only = vec![Spot::Enabled];
        only.extend((0..history).map(Spot::Ran));
        assert_eq!(s.spots(), only, "a row the brain refuses is still a spot");
        s.mind.tasks[0].ran = false;
        assert!(
            s.note_of(&s.mind.tasks[0]).is_none(),
            "the home row still gives it a standing"
        );

        // THE PERSON'S OWN OFF, as before: the word, and the key beside it —
        // on the row, and taught for the host's header.
        let task = &mut s.mind.tasks[0];
        task.why_not = OFF_BY_YOU.to_owned();
        task.why_key = "brain-why-off".to_owned();
        assert!(off_not_by_you(&s.mind.tasks[0]).is_none());
        let (keys, data) = drawn_texts(&drawn(&mut s, 30, 120));
        assert!(keys.iter().any(|k| k == key::OFF));
        assert_eq!(switches(&data), 1, "the row does not offer the switch");
        assert!(s.teaches().iter().any(|(k, _)| *k == ", o"));
        assert!(
            s.spots().len() > only.len(),
            "the person's off lost its rows"
        );
    }

    /// Enter opens the task, Esc comes back, and Esc on the home page closes
    /// the window — which the host confirms first (`Ui::closing`).
    #[test]
    fn enter_opens_and_esc_comes_back() {
        let mut s = a_state();
        assert_eq!(s.state_id(), "normal");
        s.act(Input::Confirm);
        assert_eq!(s.state_id(), "task");
        s.act(Input::Cancel);
        assert_eq!(s.state_id(), "normal");
        assert!(
            matches!(s.act(Input::Cancel).as_slice(), [Effect::Close]),
            "Esc on the home page did not close the window"
        );
    }

    /// A row of the log opens the task it belongs to.
    #[test]
    fn a_log_row_opens_its_task() {
        let mut s = a_state();
        s.pane = Pane::Log;
        s.log = 0; // the fixture-routine record, which stands alone
        s.act(Input::Confirm);
        assert_eq!(s.page, Page::Task);
        assert_eq!(s.here().map(|t| t.id.as_str()), Some("fixture-routine"));
    }

    /// The cadence ladder walks and wraps.
    #[test]
    fn the_cadence_ladder_wraps() {
        let s = a_state();
        assert_eq!(s.next_cadence(1), 7);
        assert_eq!(s.next_cadence(7), 30);
        assert_eq!(s.next_cadence(30), 1);
        assert_eq!(s.next_cadence(99), 1);
    }

    /// The verb's target is one-based, and nothing is asked with no task.
    #[test]
    fn the_target_is_one_based() {
        let mut s = a_state();
        s.task = 1;
        let Some(Effect::Ask(d)) = s.verb(Request::RunTask).into_iter().next() else {
            panic!("no demand")
        };
        assert_eq!(d.target, 2);
        s.mind.tasks.clear();
        assert!(s.verb(Request::RunTask).is_empty());
    }

    /// THE FOOTER SAYS THE MOST USEFUL THING IT HAS, and not the first.
    ///
    /// A row of the log that reads `Waited` with a paragraph under it also
    /// reading `Waited` is a paragraph that says nothing. What a person wants
    /// from that row is WHY it waited, which the record carries.
    #[test]
    fn the_log_explains_the_row_rather_than_repeating_it() {
        let mut s = a_state();
        s.pane = Pane::Log;

        // A wait, with a reason — by the KEY the host sent, never one built
        // here out of the word.
        let row = &mut s.mind.activity_rows[1].newest;
        row.why = "ram".to_owned();
        row.why_key = "brain-why-ram".to_owned();
        s.log = 1;
        assert_eq!(s.told().about.as_deref(), Some("brain-why-ram"));

        // A record that named a step: the step's own key, and never the id —
        // on its own line, so the sentence is still the reason.
        let row = &mut s.mind.activity_rows[1].newest;
        row.why.clear();
        row.why_key.clear();
        row.at = "a-step".to_owned();
        row.at_key = "fixture-step-a-step".to_owned();
        let told = s.told();
        assert_eq!(told.at_step.as_deref(), Some("fixture-step-a-step"));
        assert_eq!(told.about.as_deref(), Some(key::UNSAID));

        // And with neither, it says it kept no reason — never the outcome
        // again, which the row already reads.
        let row = &mut s.mind.activity_rows[1].newest;
        row.at.clear();
        row.at_key.clear();
        let told = s.told();
        assert_eq!(told.at_step, None);
        assert_eq!(told.about.as_deref(), Some(key::UNSAID));

        // WHAT THE MACHINE SAID goes through verbatim, as a datum.
        s.mind.activity_rows[1].newest.trouble = "error: builder failed".to_owned();
        assert_eq!(s.told().trouble.as_deref(), Some("error: builder failed"));

        // AND WHAT A MODULE NOTED is its sentence, as data, in place of any
        // key: a note is not a run, and `Noted` alone says nothing.
        let row = &mut s.mind.activity_rows[1].newest;
        row.outcome = OutcomeKind::Noted;
        row.trouble.clear();
        row.says_text = "A process was killed for memory".to_owned();
        let told = s.told();
        assert_eq!(told.about, None);
        assert_eq!(
            told.logged.map(|(t, _)| t).as_deref(),
            Some("A process was killed for memory")
        );
    }

    /// FAILS IF: a record of a task's history is told by its outcome's word —
    /// the footer that read `Failed` over `Failed`, the owner's *"o título e
    /// conteúdo do histórico são apenas uma repetição do status, não servindo
    /// para nada"* — or a refusal its module knows loses the step it stopped
    /// at or the module's sentence for it, or that step is not drawn under
    /// the heading, after its label.
    #[test]
    fn a_record_is_told_by_what_stopped_it_and_never_by_its_status() {
        let mut s = a_state();
        s.page = Page::Task;
        let task = s.task;
        let failed = s.mind.tasks[task]
            .history
            .iter()
            .position(|r| r.outcome == OutcomeKind::Failed)
            .expect("the fixture's task has a failed run");
        s.spot = s
            .spots()
            .iter()
            .position(|it| *it == Spot::Ran(failed))
            .expect("the page draws the run");

        // A FAILURE THAT KEPT NOTHING says so, and where its words went.
        let told = s.told();
        assert_eq!(told.name.as_deref(), Some(key::RECORD_FAILED));
        assert_eq!(told.at_step, None);
        assert_eq!(told.about.as_deref(), Some(key::UNSAID_FAILURE));

        // A REFUSAL ITS MODULE KNOWS: the step, and the module's sentence,
        // as data — the host made it — in the tone of what went wrong.
        let refusal = "One more system would push the last good one out of the boot menu.";
        let run = &mut s.mind.tasks[task].history[failed];
        run.at = "fixture-rebuild".to_owned();
        run.at_key = "fixture-step-rebuild".to_owned();
        run.says = "fixture-refused".to_owned();
        run.says_text = refusal.to_owned();
        let told = s.told();
        assert_eq!(told.name.as_deref(), Some(key::RECORD_FAILED));
        assert_eq!(told.at_step.as_deref(), Some("fixture-step-rebuild"));
        assert_eq!(told.logged, Some((refusal.to_owned(), Role::Danger)));
        assert_eq!(told.about, None);
        // NOR THE DATE AGAIN: the row reads it, and "last run" was false of
        // every run but the newest.
        assert!(told.facts.is_empty(), "{:?}", told.facts);
        // AND THE POINTER ON THE ROW SAYS THE SAME SENTENCE.
        assert_eq!(
            s.said_of_record(&s.mind.tasks[task].history[failed]),
            Some(Pointed::Datum(refusal.to_owned()))
        );

        // DRAWN: the step on the line under the heading, after its label.
        let screen = drawn(&mut s, 30, 100);
        let seen = runs(&screen);
        let label = seen
            .iter()
            .find(|(t, ..)| t == key::AT_STEP)
            .expect("the step's label is drawn");
        // ON THAT LINE: the history's row draws the step too, in its note.
        let step = seen
            .iter()
            .find(|(t, _, line, _)| t == "fixture-step-rebuild" && *line == label.2)
            .expect("the step is not on its label's line");
        assert!(step.3 > label.3, "the step is not after its label");
        assert!(
            seen.iter()
                .any(|(t, _, line, _)| t == key::RECORD_FAILED && *line + 1 == label.2),
            "the step is not on the line under the heading: {seen:?}"
        );

        // EVERY OUTCOME: neither the heading nor the sentence is its word.
        for outcome in OUTCOMES {
            let run = &mut s.mind.tasks[task].history[failed];
            run.outcome = outcome;
            run.says_text.clear();
            run.at_key.clear();
            let told = s.told();
            let word = outcome_key(outcome);
            assert_ne!(told.name.as_deref(), Some(word.as_str()), "{outcome:?}");
            assert_ne!(told.about.as_deref(), Some(word.as_str()), "{outcome:?}");
        }
    }

    /// THE TASK'S PAGE DOES NOT PRINT ITS DESCRIPTION TWICE.
    ///
    /// The page opens with the description and the footer is pinned three
    /// rows under it. Photographed, the two read as one paragraph set twice
    /// on one screen — the same defect the activity log has a test against,
    /// in the same words. What the footer carries instead is what a schedule
    /// row has no room for: how it last went, and the two readings that
    /// decide whether any of these rows fires at all.
    ///
    /// The home page is the OTHER way round and stays that way: there the
    /// footer is the only place the description exists.
    #[test]
    fn a_task_page_does_not_repeat_its_own_description() {
        let mut s = a_state();
        let about = s.told().about.clone();
        assert_eq!(
            about.as_deref(),
            Some("brain-task-fixture-routine-does"),
            "the home page's footer stopped describing the task"
        );

        s.page = Page::Task;
        // EVERY SPOT, and not the four that happened to be listed. The four
        // were the schedule's; the defect was on a HISTORY row, which fell
        // back to the description whenever a run had simply worked — the owner
        // read it on a row marked `worked` and said what it is: the page's own
        // description, printed a second time on the same screen. A test that
        // names the rows it checks is a test that checks the rows somebody
        // thought of.
        let every: Vec<Spot> = s.spots();
        floor_of(every.len(), 8, "spots on the page");
        for spot in every {
            s.spot = s
                .spots()
                .iter()
                .position(|it| *it == spot)
                .expect("the page draws that row");
            let told = s.told();
            assert_ne!(
                told.about.as_deref(),
                Some("brain-task-fixture-routine-does"),
                "{spot:?} put the page's own description in the footer again"
            );
            // AND THE MACHINE'S READINGS ARE NOT THERE. They were, under
            // every task's name on every frame, and the owner said they
            // should not be shown at all: what a footer is for is the thing
            // in focus, and a percentage of the machine is not about it.
            assert!(
                told.facts
                    .iter()
                    .all(|(k, _)| k != "brain-ram" && k != "brain-cpu"),
                "{spot:?} put the machine's readings back in the footer"
            );
        }
    }

    /// FAILS IF: the cursor steps onto a row the window is not showing.
    ///
    /// # The owner's report, in his numbers
    ///
    /// *"Na página da task `Update the system`, ao mover com a tecla de seta
    /// para baixo, do primeiro item do history ao segundo, o segundo item foi
    /// selecionado, porém a página não moveu para baixo … e somente após mover
    /// novamente a seleção para baixo, ao quarto item, foi que a página se
    /// moveu."*
    ///
    /// Two. The window was measured in rows of the LIST and the description is
    /// one row of the list and three lines of the screen, so everything below
    /// it was two lines further down than the arithmetic believed. It is the
    /// same two at every width where the description wraps.
    #[test]
    fn the_window_follows_the_cursor_by_lines() {
        for lines in [10_u16, 14, 20] {
            let mut s = a_state();
            s.page = Page::Task;
            let _ = drawn(&mut s, lines, 100);
            let many = s.spots().len();
            floor_of(many, 8, "spots on the page");
            for spot in 0..many {
                s.spot = spot;
                // A PERSON MOVED IT, which is what an arrow says.
                s.follows = true;
                let _ = drawn(&mut s, lines, 100);
                let rows = s.page_rows();
                let here = s.spot_here().expect("the cursor is on a spot");
                let at = State::row_of_spot(&rows, here).expect("the spot is drawn");
                let shown = fit_from(&rows, s.body_from, s.blocks.body.lines);
                assert!(
                    at >= s.body_from && at < s.body_from + shown,
                    "at {lines} lines, spot {spot} sits on row {at} and the window shows \
                     {}..{}",
                    s.body_from,
                    s.body_from + shown
                );
            }
        }
    }

    /// FAILS IF: the footer's sentence does not take the room it has, or does
    /// not scroll when there is more of it than room.
    ///
    /// # What the owner read on the screen
    ///
    /// *"o texto do footer está sendo cortado, e … por ter espaço vazio abaixo
    /// do texto, então o texto DEVE ocupar todo o espaço disponível no
    /// footer"*; and, when even that is not enough, *"o footer do brain DEVE
    /// funcionar com um scrollbar própria"*.
    ///
    /// The rows are the HOST's to count — it holds the translated string and
    /// it is what wraps — so what this side owns is the rectangle's height and
    /// the offset, and those are what this asks about.
    #[test]
    fn the_footers_sentence_takes_the_room_it_has() {
        let mut s = a_state();
        s.page = Page::Task;
        // A STEP: the one footer with all three parts — a name, the command it
        // holds, and a description under them.
        // THE SECOND STEP: the first is the flake's, and a root step carries
        // two TAGS in the footer's figures — which hold a row back, as they
        // should. This one has none, so the sentence owes the whole block.
        let rows = s.page_rows();
        s.spot = rows
            .iter()
            .position(|r| matches!(r, Row::Step(1)))
            .and_then(|n| State::spot_of_row(&rows, n))
            .expect("the page draws a step of the person's own");
        let screen = drawn(&mut s, 26, 100);
        let list = screen.as_protocol();
        let foot = s.blocks.foot;
        let said = list
            .commands
            .iter()
            .find_map(|c| match c {
                ronoctua_design::frame::Command::Paragraph {
                    line,
                    lines,
                    columns,
                    ..
                } if foot.holds(*line) => Some((*line, *lines, *columns)),
                _ => None,
            })
            .expect("the footer drew no sentence");
        let (line, lines, columns) = said;
        // EVERYTHING FROM ITS FIRST ROW TO THE END OF THE BLOCK, because this
        // footer has no figures to hold a row back for.
        assert_eq!(
            line + lines,
            foot.top + foot.lines,
            "the sentence stopped {} rows short of the footer's end",
            foot.top + foot.lines - (line + lines)
        );
        // AND IT IS GIVEN THE WHOLE WIDTH: the host keeps the thumb's lane
        // out of the words only while there is a thumb. A cell kept here as
        // well put the thumb over the last letter of every row — the host
        // drew it in the paragraph's own last cell — which the owner
        // photographed.
        assert_eq!(columns, s.columns);

        // AND WHERE THERE ARE FIGURES, it stops exactly one row short of them
        // and not two: the row held back is the values' and nothing else.
        s.spot = rows
            .iter()
            .position(|r| matches!(r, Row::Step(0)))
            .and_then(|n| State::spot_of_row(&rows, n))
            .expect("the page draws the flake's step");
        let screen = drawn(&mut s, 26, 100);
        let list = screen.as_protocol();
        let (line, lines) = list
            .commands
            .iter()
            .find_map(|c| match c {
                ronoctua_design::frame::Command::Paragraph { line, lines, .. }
                    if foot.holds(*line) =>
                {
                    Some((*line, *lines))
                }
                _ => None,
            })
            .expect("the footer drew no sentence");
        assert_eq!(
            line + lines,
            foot.top + foot.lines - 1,
            "the sentence did not stop exactly at the figures"
        );

        // AND THE OFFSET IS FORGOTTEN WITH ITS SUBJECT. Left alone it would
        // carry to the next row the cursor lands on, which opens a short
        // message already scrolled past its own end.
        s.foot_scroll = 3;
        let was = s.foot_of.clone();
        s.spot = 0;
        let _ = drawn(&mut s, 26, 100);
        assert_ne!(s.foot_of, was, "the footer is about the same thing");
        assert_eq!(s.foot_scroll, 0, "the scroll survived the cursor moving");

        // AND A DRAG ON ITS BAR MOVES IT. The host answers with a row it has
        // already clamped; this side only says which block it belongs to.
        let out = s.scrolled(0, s.blocks.foot.top + 1, 2);
        assert!(!out.is_empty(), "a drag in the footer did nothing");
        assert_eq!(s.foot_scroll, 2);
    }

    /// FAILS IF: a place the keyboard can reach cannot be reached with the
    /// pointer, or the second click does not do what `RET` does.
    ///
    /// # The rule this keeps
    ///
    /// *"O SUPORTE AO MOUSE DEVE SER SUPORTADO EM TODAS AS IMPLEMENTAÇÕES."*
    /// The owner reported this page as the worst of it: inside a task's page
    /// the pointer did nothing at all — *"nem para selecionar, nem para trocar
    /// de valores"* — because `clicked` was guarded by `page == Page::Home`.
    ///
    /// It walks EVERY spot rather than a chosen one, because the defect that
    /// makes this hard is the description: it is three rows of screen and one
    /// row of the list, so a click resolved by counting rows lands two spots
    /// late on everything under it.
    #[test]
    fn every_spot_is_reachable_by_the_pointer() {
        let mut s = a_state();
        s.page = Page::Task;
        let _ = drawn(&mut s, 40, 100);
        let rows = s.page_rows();
        assert!(rows.len() > 8, "the page drew almost nothing");
        let top = s.blocks.body.top;
        let mut reached = 0;
        for n in 0..rows.len() {
            let Some(spot) = State::spot_of_row(&rows, n) else {
                continue;
            };
            let line = top + lines_before(&rows, n);
            if line >= top + s.blocks.body.lines {
                break;
            }
            // Somewhere else first, so landing on it proves the click and not
            // the cursor happening to be there.
            s.spot = 0;
            s.clicked(2, line);
            assert_eq!(
                s.spot, spot,
                "a click on screen row {line} selected spot {} and not {spot}",
                s.spot
            );
            reached += 1;
        }
        floor_of(reached, 8, "spots reached by the pointer");
    }

    /// FAILS IF: the second click on a row does not do what `RET` does.
    #[test]
    fn a_second_click_does_what_return_does() {
        // ON THE HOME PAGE it opens the task. The SECOND task, because the
        // first is where the cursor already is — and a click on the row the
        // cursor is already on IS the second click, which is the rule and not
        // an exception to it.
        let mut s = a_state();
        let _ = drawn(&mut s, 40, 100);
        let line = s.blocks.body.top + 2;
        s.clicked(2, line);
        assert_eq!(s.page, Page::Home, "one click opened a page");
        assert_eq!(s.task, 1, "the click did not select the row it landed on");
        s.clicked(2, line);
        assert_eq!(s.page, Page::Task, "the second click did not open the task");

        // AND ON A TASK'S PAGE it does the row's own key — here, flipping a
        // step, which is the thing the owner could reach with neither `RET`
        // nor the mouse.
        let rows = s.page_rows();
        // THE SECOND STEP: the first is the flake's, and a root step REFUSES
        // to be flipped — by the key and by the pointer alike, which is the
        // point of it being drawn refusing rather than not drawn.
        let n = rows
            .iter()
            .position(|r| matches!(r, Row::Step(1)))
            .expect("the page draws a step of the person's own");
        let spot = State::spot_of_row(&rows, n).expect("a step is a spot");
        let line = s.blocks.body.top + lines_before(&rows, n);
        s.spot = 0;
        s.clicked(2, line);
        assert_eq!(s.spot, spot);
        let out = s.clicked(2, line);
        assert!(
            out.iter().any(|e| matches!(e, Effect::Tune(_))),
            "the second click on a step changed nothing"
        );
    }

    /// FAILS IF: a click on a scrolled list picks the row by its place on the
    /// SCREEN rather than in the list.
    #[test]
    fn a_click_counts_from_the_window_and_not_from_the_screen() {
        let mut s = a_state();
        // A window of three rows onto the tasks, scrolled by one.
        let _ = drawn(&mut s, 40, 100);
        s.body_from = 1;
        s.clicked(2, s.blocks.body.top + 1);
        assert_eq!(s.task, 1, "the click ignored where the window starts");
    }

    /// OPENING A PAGE ASKS FOR THE PAGE TO BE DRAWN.
    ///
    /// THE DEFECT THIS CATCHES is the one the owner read on the screen: `RET`
    /// on a task moved the modeline's title to `task` and left the home page
    /// underneath it, because the only effect this returned was the
    /// `StateChanged` that `event` appends — and the host treated that as
    /// bookkeeping. The page arrived one keystroke later, on an arrow, already
    /// scrolled a row.
    #[test]
    fn opening_a_task_asks_for_a_frame() {
        for pane in [Pane::Tasks, Pane::Log] {
            let mut s = a_state();
            s.pane = pane;
            let out = s.confirm();
            assert_eq!(s.page, Page::Task, "{pane:?} did not open the page");
            assert!(
                out.iter().any(|e| matches!(e, Effect::Redraw)),
                "{pane:?} opened a page and asked for no frame"
            );
        }
    }

    /// The clock is minutes and seconds, and nothing else on this screen
    /// formats a duration.
    #[test]
    fn a_countdown_reads_as_a_clock() {
        assert_eq!(clock(75), "01:15");
    }

    /// Every text key a frame draws, and every datum.
    /// FAILS IF: a row's `⌥` does not spell `LEADER Alt` under the pointer,
    /// as every `⌥` of the system does — or spells it with nobody on it, or
    /// grows over what the row says beside it.
    #[test]
    fn a_rows_glyph_is_spelt_under_the_pointer() {
        use ronoctua_design::frame::{Command, Text};
        let mut s = a_state();
        s.phrases = Phrases::of(vec![(
            Phrases::LEADER_ALT.to_owned(),
            "SUPER ALT".to_owned(),
        )]);
        s.page = Page::Task;
        let screen = drawn(&mut s, 30, 120);
        let (_, data) = drawn_texts(&screen);
        assert!(
            !data.iter().any(|d| d.starts_with("SUPER ALT")),
            "spelt with nobody on it: {data:?}"
        );
        let (line, column) = screen
            .as_protocol()
            .commands
            .iter()
            .find_map(|c| match c {
                Command::Text {
                    text: Text::Icon(i),
                    line,
                    column,
                    ..
                } if i.contains("LeaderAlt") => Some((*line, *column)),
                _ => None,
            })
            .expect("a row carries a ⌥");
        let _ = s.hover.set(0, (column, line));
        let screen = drawn(&mut s, 30, 120);
        screen
            .check()
            .unwrap_or_else(|e| panic!("the spelt row met something: {e}"));
        let (_, data) = drawn_texts(&screen);
        assert!(
            data.iter().any(|d| d == "SUPER ALT"),
            "the ⌥ under the pointer was not spelt: {data:?}"
        );
        // AND THE WORDS WEAR THE MARK'S DISCREET INK, not the key's: they are
        // the same mark, spelt.
        assert_eq!(
            colour_of(&runs(&screen), "SUPER ALT").as_deref(),
            Some("text-subtle-leader-alt")
        );
    }

    /// Every run the frame drew: what it says (its key, its data or its
    /// icon), the token it is drawn in, and its line and cell.
    fn runs(screen: &Screen<Wit>) -> Vec<(String, String, u16, u16)> {
        use ronoctua_design::frame::{Command, Text};
        screen
            .as_protocol()
            .commands
            .into_iter()
            .filter_map(|c| match c {
                Command::Text {
                    text,
                    color,
                    line,
                    column,
                    ..
                } => Some((
                    match text {
                        Text::Key(k) | Text::Data(k) | Text::Icon(k) => k,
                    },
                    color.as_str().to_owned(),
                    line,
                    column,
                )),
                _ => None,
            })
            .collect()
    }

    /// The token the first run saying `what` is drawn in.
    fn colour_of(drawn: &[(String, String, u16, u16)], what: &str) -> Option<String> {
        drawn
            .iter()
            .find(|(t, ..)| t == what)
            .map(|(_, c, ..)| c.clone())
    }

    /// FAILS IF: a status is drawn in one grey again, or a hold in the
    /// alarm's red — the owner's discreet tones, each in the hue of its news —
    /// or a watch wears anything but its own turned grey (`text.listening`).
    #[test]
    fn a_status_wears_the_hue_of_its_news() {
        let mut s = a_state();
        s.mind.tasks[1].ran = false;
        let mut held = a_task("fixture-held", TaskKind::Routine);
        held.ran = false;
        held.hold = HoldKind::Ram;
        let mut watch = a_task("fixture-watch", TaskKind::Watch);
        watch.ran = false;
        s.mind.tasks.extend([held, watch]);
        let seen = runs(&drawn(&mut s, 30, 120));
        for (said, wanted) in [
            ("brain-outcome-done", "text-status-good"),
            ("brain-standing-idle", "text-status-idle"),
            ("brain-hold-ram", "text-status-held"),
            ("brain-watching", "text-listening"),
            ("brain-task-fixture-watch", "text-listening"),
        ] {
            assert_eq!(
                colour_of(&seen, said).as_deref(),
                Some(wanted),
                "`{said}` in the wrong tone"
            );
        }
        // AND EVERY OUTCOME HAS A HUE OF ITS OWN.
        let mut hues: Vec<Role> = OUTCOMES.iter().map(|o| outcome_role(*o)).collect();
        hues.dedup();
        assert_eq!(hues.len(), OUTCOMES.len(), "two outcomes share a hue");
    }

    /// FAILS IF: a date's day is drawn as quietly as the rest of it, today's
    /// has no hue of its own, an opened fold's records wear the row's tone or
    /// say its outcome again — or a task's history draws its dates whole.
    #[test]
    fn a_dates_day_stands_out_and_todays_in_its_own_hue() {
        let mut s = a_state();
        s.mind.activity_rows[0].newest.when_stamp.today = true;
        s.pane = Pane::Log;
        s.log = 1;
        let _ = s.act(Input::Confirm);
        assert_eq!(s.activity_lines().len(), 4, "the fold did not open");
        let seen = runs(&drawn(&mut s, 30, 120));
        let days: Vec<&String> = seen
            .iter()
            .filter(|(t, ..)| t == "23")
            .map(|(_, c, ..)| c)
            .collect();
        assert!(
            days.iter().any(|c| *c == "text-today"),
            "today has no hue: {days:?}"
        );
        assert!(
            days.iter().any(|c| *c == "text-secondary"),
            "a day is as quiet as its date: {days:?}"
        );
        let leads: Vec<&String> = seen
            .iter()
            .filter(|(t, ..)| t == "2026-08-")
            .map(|(_, c, ..)| c)
            .collect();
        assert!(leads.iter().any(|c| *c == "text-dim"), "{leads:?}");
        assert!(
            leads.iter().any(|c| *c == "text-folded"),
            "an opened fold's records wear the row's tone: {leads:?}"
        );
        assert_eq!(
            seen.iter()
                .filter(|(t, ..)| t == "brain-outcome-held")
                .count(),
            1,
            "an opened fold's records say their outcome again"
        );
        // AND THE HISTORY ON A TASK'S PAGE, by the same hand.
        let _ = s.act(Input::Cancel);
        s.pane = Pane::Tasks;
        let _ = s.act(Input::Confirm);
        assert_eq!(s.page, Page::Task);
        let seen = runs(&drawn(&mut s, 40, 120));
        assert!(
            seen.iter()
                .any(|(t, c, ..)| t == "23" && c == "text-secondary"),
            "a history's dates are drawn whole"
        );
        assert!(
            seen.iter()
                .any(|(t, c, ..)| t == "last 20 days" && c == "text-dim"),
            "the history does not say how far back it reaches"
        );
    }

    /// FAILS IF: a module's date is not the date the log draws — the rest of
    /// it dim, its day quiet, today's in its own hue — in whichever column it
    /// is, or a date whose day the host did not cut is not drawn at all.
    #[test]
    fn a_modules_date_is_the_date_every_page_draws() {
        let stamp = |day: &str, today| ronoctua_design::types::Stamp {
            lead: "2026-08-".to_owned(),
            day: day.to_owned(),
            tail: " 14:02".to_owned(),
            today,
        };
        let moment = |stamp| {
            ScreenCell::Moment(Timed {
                at: 5,
                text: "2026-08-23 14:02".to_owned(),
                stamp,
            })
        };
        let mut s = a_state();
        let _ = s.act(Input::Confirm);
        let _ = s.watch_asks();
        let mut page = a_screen("fixture-routine", 2);
        let rows = &mut page.sections[0].rows;
        rows[0].label = moment(stamp("23", false));
        rows[0].value = ScreenCell::Data("a line the row holds".to_owned());
        rows[0].note = moment(stamp("24", true));
        let cut_by_nobody = ronoctua_design::types::Stamp {
            lead: String::new(),
            day: String::new(),
            tail: String::new(),
            today: false,
        };
        page.sections[1].rows[0].value = ScreenCell::Moment(Timed {
            at: 5,
            text: "a date in one piece".to_owned(),
            stamp: cut_by_nobody,
        });
        s.act(Input::ModuleScreen(page));
        let seen = runs(&drawn(&mut s, 40, 120));
        for (said, wanted) in [
            ("2026-08-", "text-dim"),
            ("23", "text-secondary"),
            (" 14:02", "text-dim"),
            ("24", "text-today"),
        ] {
            assert_eq!(colour_of(&seen, said).as_deref(), Some(wanted), "`{said}`");
        }
        assert!(
            colour_of(&seen, "2026-08-23 14:02").is_none(),
            "a date was drawn whole beside its pieces"
        );
        assert!(
            colour_of(&seen, "a date in one piece").is_some(),
            "a date the host did not cut is not drawn"
        );
    }

    /// FAILS IF: a row whose date is its value loses the tone its module gave
    /// it, the only colour such a row has, or the day of that date stops being
    /// the day's, or a row with no tone draws its date in anything but the dim
    /// of every date.
    #[test]
    fn a_dated_value_keeps_the_tone_of_its_row() {
        let stamp = ronoctua_design::types::Stamp {
            lead: "2026-08-".to_owned(),
            day: "23".to_owned(),
            tail: " 14:02".to_owned(),
            today: false,
        };
        let date = ScreenCell::Moment(Timed {
            at: 5,
            text: "2026-08-23 14:02".to_owned(),
            stamp,
        });
        let mut s = a_state();
        for (generation, tone, rest) in [
            (2, BrainTone::Warning, "state-warning"),
            (3, BrainTone::Danger, "state-error"),
            (4, BrainTone::Quiet, "text-dim"),
        ] {
            let mut page = a_screen("fixture-routine", generation);
            page.sections[0].rows[0].value = date.clone();
            page.sections[0].rows[0].tone = tone;
            on_the_page(&mut s, page);
            let seen = runs(&drawn(&mut s, 40, 120));
            for (said, wanted) in [
                ("2026-08-", rest),
                ("23", "text-secondary"),
                (" 14:02", rest),
            ] {
                assert_eq!(colour_of(&seen, said).as_deref(), Some(wanted), "`{said}`");
            }
        }
    }

    /// FAILS IF: what the pointer rests on is not declared for the host — a
    /// task's ONE LINE under its row (never its whole description, the
    /// footer's paragraph run along the minibuffer), what its glyph means
    /// under the glyph, when it runs next under its date, how its last run
    /// went and when under its status, what it waits for under a hold, what
    /// the fold's mark means under the mark — or something is declared with no
    /// pointer on the window.
    #[test]
    fn what_the_pointer_rests_on_is_said() {
        use ronoctua_design::frame::{Command, Text};
        let pointed = |screen: &Screen<Wit>| {
            screen
                .as_protocol()
                .commands
                .into_iter()
                .find_map(|c| match c {
                    Command::Pointed { about, args } => Some((
                        about,
                        args.into_iter()
                            .map(|a| (a.name, a.text))
                            .collect::<Vec<_>>(),
                    )),
                    _ => None,
                })
        };
        let key = |k: &str| Some((Text::Key(k.into()), Vec::new()));
        let with = |k: &str, name: &str, text: &str| {
            Some((
                Text::Key(k.into()),
                vec![(name.to_owned(), text.to_owned())],
            ))
        };
        let mut s = a_state();
        assert_eq!(
            pointed(&drawn(&mut s, 30, 120)),
            None,
            "said with nobody here"
        );
        let row = s.blocks.body.line_of(1).expect("the first task's row");
        let at = |s: &State, id: &str| s.task_layout.field(id).expect(id).at;
        let _ = s.hover.set(0, (at(&s, "name"), row));
        assert_eq!(
            pointed(&drawn(&mut s, 30, 120)),
            key("brain-task-fixture-routine-brief"),
            "a task's row does not say what the task is in one line"
        );
        let _ = s.hover.set(0, (at(&s, "kind"), row));
        assert_eq!(
            pointed(&drawn(&mut s, 30, 120)),
            key(key::KIND_ROUTINE),
            "a task's glyph does not say what it means"
        );
        let _ = s.hover.set(0, (at(&s, "when"), row));
        assert_eq!(
            pointed(&drawn(&mut s, 30, 120)),
            with(key::ABOUT_NEXT_AT, "when", "2026-08-24 14:00"),
            "a task's date does not say it is when it runs next"
        );
        let _ = s.hover.set(0, (at(&s, "note"), row));
        assert_eq!(
            pointed(&drawn(&mut s, 30, 120)),
            with(key::ABOUT_DONE, "when", "2026-08-23 13:00"),
            "a task's status does not say how its last run went, and when"
        );
        // A HOLD SAYS WHAT IT WAITS FOR; A WATCH, UNDER ITS WORD, THAT IT
        // ONLY LISTENS; A MODULE WITH NO LINE, ITS DESCRIPTION.
        // (A hold puts the warning line up, and the list one row down.)
        s.mind.tasks[0].hold = HoldKind::Battery;
        let _ = drawn(&mut s, 30, 120);
        let row = s.blocks.body.line_of(1).expect("the first task's row");
        let _ = s.hover.set(0, (at(&s, "note"), row));
        assert_eq!(
            pointed(&drawn(&mut s, 30, 120)),
            key(key::ABOUT_HOLD_BATTERY)
        );
        s.mind.tasks[0].kind = TaskKind::Watch;
        let _ = s.hover.set(0, (at(&s, "when"), row));
        assert_eq!(pointed(&drawn(&mut s, 30, 120)), key(key::KIND_WATCH));
        s.mind.tasks[0].brief_key = String::new();
        let _ = s.hover.set(0, (at(&s, "name"), row));
        assert_eq!(
            pointed(&drawn(&mut s, 30, 120)),
            key("brain-task-fixture-routine-does")
        );
        let fold = s.log_layout.field("fold").expect("the fold's column").at;
        let line = s.blocks.log.line_of(2).expect("the folded row");
        let _ = s.hover.set(0, (fold, line));
        assert_eq!(
            pointed(&drawn(&mut s, 30, 120)),
            key(key::ABOUT_FOLD),
            "the fold's mark does not say what it is"
        );
        let _ = s.hover.clear(0);
        assert_eq!(
            pointed(&drawn(&mut s, 30, 120)),
            None,
            "the pointer left and it stayed"
        );
    }

    /// FAILS IF: a watch is drawn in anything but its own turned grey — the
    /// teal it wore read too loud, the owner's word — a module's row toned
    /// `watching` loses the alarm's colour it shares with the modeline's mark
    /// (the journal's errors), or a row toned `working` is not the teal. The
    /// owner asked for the first and said the second had been taken away
    /// with it.
    #[test]
    fn a_watch_wears_its_own_grey_and_a_watching_row_keeps_its_alarm() {
        use ronoctua_design::Palette;
        let colour = |role: Role| <Wit as Palette>::color(role);
        assert_eq!(
            colour(tone_role(BrainTone::Watching)),
            colour(Role::Watching)
        );
        assert_eq!(
            colour(tone_role(BrainTone::Working)),
            colour(Role::Following)
        );
        assert_eq!(colour(kind_role(TaskKind::Watch)), colour(Role::Listening));
        assert_ne!(colour(Role::Listening), colour(Role::Following));
        assert_ne!(colour(Role::Listening), colour(Role::Body));
        assert_ne!(colour(Role::Watching), colour(Role::Following));
    }

    /// FAILS IF: the footer's figures stay at the start of its last row while
    /// the click that opens the page sits at the other end — the owner asked
    /// for the date just left of the click, *"separado por um bom espaço"* —
    /// or they touch the click, or leave it no air.
    #[test]
    fn the_footers_figures_sit_against_its_click() {
        let mut s = a_state();
        s.task = 0;
        let seen = runs(&drawn(&mut s, 30, 120));
        let (_, _, line, click) = seen
            .iter()
            .find(|(t, ..)| t == key::OPEN_PAGE)
            .cloned()
            .expect("the footer has no click");
        let on_the_row: Vec<_> = seen.iter().filter(|(_, _, l, _)| *l == line).collect();
        let date = on_the_row
            .iter()
            .find(|(t, ..)| t == "2026-08-23 13:00")
            .expect("the last run's date is not on the click's row");
        let label = on_the_row
            .iter()
            .find(|(t, ..)| t == key::LAST_RUN)
            .expect("the date's label is not on the click's row");
        let end = date.3 + ronoctua_design::wide(&date.0);
        assert_eq!(
            end + FACT_AIR,
            click,
            "the date does not end a good space before the click"
        );
        assert!(label.3 < date.3, "the label is not before its date");
        assert!(label.3 > 0, "the figures stayed at the start of the row");
    }

    /// FAILS IF: something the Brain draws says nothing under the pointer —
    /// the owner's *"todos os elementos do brain, ao colocar o cursor por
    /// cima, mostre algo no minibuffer"*: a title, the figures beside one, the
    /// page's own name, the warning, the footer and every piece of it — on the
    /// home page, on a task's page on a record and on a step, and on the
    /// daemon's log, with no header and with one. And the titles say what is
    /// under them, not the same sentence for all.
    #[test]
    fn everything_drawn_says_something_under_the_pointer() {
        use ronoctua_design::frame::{Command, Text};
        let said = |screen: &Screen<Wit>| {
            screen
                .as_protocol()
                .commands
                .into_iter()
                .find_map(|c| match c {
                    Command::Pointed { about, .. } => Some(about),
                    _ => None,
                })
        };
        let every = |s: &mut State, headed: bool, page: &str| {
            let mut ctx = a_context(30, 120);
            ctx.headed = headed;
            let _ = s.hover.set(0, (0, 0));
            let seen = runs(&s.compose(&ctx));
            assert!(seen.len() > 8, "{page}: drew too little to judge");
            let mut told = Vec::new();
            for (what, _, line, column) in seen {
                let _ = s.hover.set(0, (column, line));
                let about = said(&s.compose(&ctx));
                assert!(
                    about.is_some(),
                    "{page}: `{what}` at {column},{line} says nothing under the pointer"
                );
                told.push((what, about));
            }
            told
        };
        let mut s = a_state();
        // A HOLD PUTS UP THE WARNING LINE.
        s.mind.tasks[0].hold = HoldKind::Battery;
        for headed in [false, true] {
            let told = every(&mut s, headed, "the home page");
            let of = |k: &str| {
                told.iter()
                    .find(|(w, _)| w == k)
                    .and_then(|(_, a)| a.clone())
            };
            assert_eq!(of(key::TASKS), Some(Text::Key(key::ABOUT_TASKS.into())));
            assert_eq!(
                of(key::ACTIVITY),
                Some(Text::Key(key::ABOUT_ACTIVITY.into()))
            );
            if !headed {
                assert_eq!(of(key::TITLE), Some(Text::Key(key::ABOUT_TITLE.into())));
            }
        }
        s.mind.tasks[0].hold = HoldKind::None;
        s.page = Page::Task;
        let task = s.task;
        for spot in [Spot::Ran(0), Spot::Step(0), Spot::Enabled] {
            s.spot = s
                .spots()
                .iter()
                .position(|it| *it == spot)
                .expect("the page draws the spot");
            let told = every(&mut s, false, "a task's page");
            let of = |k: &str| {
                told.iter()
                    .find(|(w, _)| w == k)
                    .and_then(|(_, a)| a.clone())
            };
            for (heading, about) in [
                (key::OPTIONS, key::ABOUT_OPTIONS),
                (key::STEPS, key::ABOUT_STEPS),
                (key::HISTORY, key::ABOUT_HISTORY),
            ] {
                assert_eq!(of(heading), Some(Text::Key(about.into())), "{heading}");
            }
        }
        assert!(!s.mind.tasks[task].steps.is_empty());
        // AND A MODULE'S PAGE WITH ITS OWN TITLES: a swatch, a triangle, a
        // title and the count at the edge of it.
        s.watched = Some(s.mind.tasks[task].id.clone());
        let mut page = a_folding_page(2);
        page.sections[0].about = "fixture-about-first".to_owned();
        page.legend = a_legend();
        s.act(Input::ModuleScreen(page));
        let told = every(&mut s, false, "a module's page");
        let of = |k: &str| {
            told.iter()
                .find(|(w, _)| w == k)
                .and_then(|(_, a)| a.clone())
        };
        assert_eq!(
            of("fixture-heading-first"),
            Some(Text::Key("fixture-about-first".into()))
        );
        assert_eq!(
            of("fixture-worst"),
            Some(Text::Key("fixture-worst-about".into()))
        );
        s.page = Page::Log;
        let _ = every(&mut s, false, "the daemon's log");
    }

    /// FAILS IF: with no header over the window, the home page does not say
    /// whose it is at the end of its first title, quietly, or a task's page
    /// does not open with its name and a way back — a row of its own, a blank
    /// one under it, then its description — or the way back does not lead
    /// home and say so under the pointer; or any of it is drawn while the
    /// header is there to say it.
    #[test]
    fn with_no_header_the_page_says_whose_it_is_and_leads_back() {
        use ronoctua_design::frame::{Command, Text};
        let mut s = a_state();
        let mut bare = a_context(30, 120);
        bare.headed = false;
        let seen = runs(&s.compose(&bare));
        let top = s.blocks.body.top;
        let (_, colour, line, column) = seen
            .iter()
            .find(|(t, ..)| t == key::TITLE)
            .cloned()
            .expect("the home page does not say whose it is");
        assert_eq!((line, colour.as_str()), (top, "text-dim"));
        assert!(
            column > 100,
            "its name is not at the end of the line: {column}"
        );
        let tasks = seen
            .iter()
            .find(|(t, ..)| t == key::TASKS)
            .expect("the list lost its title");
        assert_eq!((tasks.2, tasks.3), (top, 0));
        assert!(
            !runs(&drawn(&mut s, 30, 120))
                .iter()
                .any(|(t, ..)| t == key::TITLE || t == key::BACK),
            "said again under a header that says it"
        );

        let _ = s.open_task(0);
        let screen = s.compose(&bare);
        let seen = runs(&screen);
        let top = s.blocks.body.top;
        let name = seen
            .iter()
            .find(|(t, ..)| t == "brain-task-fixture-routine")
            .expect("the page does not say whose it is");
        assert_eq!((name.2, name.3), (top, 0));
        let back = seen
            .iter()
            .find(|(t, ..)| t == key::BACK)
            .cloned()
            .expect("the page has no way back");
        assert_eq!((back.1.as_str(), back.2), ("state-accent", top));
        assert!(back.3 > 100, "the way back is not at the end of the line");
        let described = screen.as_protocol().commands.iter().find_map(|c| match c {
            Command::Paragraph {
                line,
                content: Text::Key(k),
                ..
            } if k.ends_with("-does") => Some(*line),
            _ => None,
        });
        assert_eq!(
            described,
            Some(top + 2),
            "no blank row between the name and the description"
        );
        screen
            .check()
            .expect("the page's own first row met something");

        let _ = s.hover.set(0, (back.3, back.2));
        let said = s
            .compose(&bare)
            .as_protocol()
            .commands
            .into_iter()
            .find_map(|c| match c {
                Command::Pointed { about, .. } => Some(about),
                _ => None,
            });
        assert_eq!(said, Some(Text::Key(key::ABOUT_WAY_BACK.into())));
        let _ = s.act(Input::Click(ronoctua_design::types::Position {
            column: back.3,
            line: back.2,
        }));
        assert_eq!(s.page, Page::Home, "the way back does not lead home");

        let _ = s.hover.clear(0);
        let _ = s.open_task(0);
        let screen = drawn(&mut s, 30, 120);
        let described = screen.as_protocol().commands.iter().find_map(|c| match c {
            Command::Paragraph { line, .. } => Some(*line),
            _ => None,
        });
        assert_eq!(
            described,
            Some(s.blocks.body.top),
            "a page under its header still opens with a row of its own"
        );
    }

    /// FAILS IF: a task's description is cut at three rows while the window
    /// has room for all of it — the owner's *"não deveria existir em janelas
    /// de tamanho normal, pois o texto deveria se expandir totalmente"* — or,
    /// in a window too small for it, is not held to part of the page with a
    /// bar that MOVES it: a drag on its bar scrolls the description, and the
    /// page's own bar still scrolls the page.
    #[test]
    fn a_description_takes_the_rows_it_wraps_to_and_scrolls_only_when_small() {
        use ronoctua_design::frame::Command;
        let long = "When the disk that holds the system or your home runs short of \
                    space, it makes room at once, without waiting for the night or for \
                    a quiet machine: first the builds of compilers nobody uses and, while \
                    every cargo build under your cache weighs more than its share of the \
                    disk, the ones built longest ago; then the store's garbage, until the \
                    disk is back where it should be. It never follows a link.";
        let mut s = a_state();
        s.mind.tasks[0].does_text = long.to_owned();
        let _ = s.open_task(0);
        let paragraph = |screen: &Screen<Wit>| {
            screen.as_protocol().commands.iter().find_map(|c| match c {
                Command::Paragraph {
                    line,
                    lines,
                    columns,
                    first_row,
                    ..
                } => Some((*line, *lines, *columns, *first_row)),
                _ => None,
            })
        };
        // A WINDOW OF A GOOD SIZE: every row the text wraps to.
        let screen = drawn(&mut s, 40, 80);
        let (_, lines, columns, _) = paragraph(&screen).expect("no description");
        let wraps = ronoctua_design::wrap(long, columns, 200).len();
        assert!(
            wraps > 3,
            "the text fits three rows; the test proves nothing"
        );
        assert_eq!(
            usize::from(lines),
            wraps,
            "the description was cut with room to spare"
        );

        // A SMALL ONE: half the page, and its own bar moves it.
        let screen = drawn(&mut s, 16, 80);
        let (line, lines, columns, first) = paragraph(&screen).expect("no description");
        assert!(
            usize::from(lines) < wraps,
            "a small window gave it every row"
        );
        assert!(lines <= s.blocks.body.lines / 2 + 1);
        assert_eq!(first, 0);
        let from = s.body_from;
        let out = s.act(Input::Scrolled(ronoctua_design::types::ScrollEvent {
            at: ronoctua_design::types::Position {
                column: columns - 1,
                line,
            },
            first: 2,
        }));
        assert!(
            !out.is_empty(),
            "a drag on the description's bar did nothing"
        );
        assert_eq!(s.about_scroll, 2);
        assert_eq!(s.body_from, from, "the description's bar moved the page");
        let (_, _, _, first) = paragraph(&drawn(&mut s, 16, 80)).expect("no description");
        assert_eq!(
            first, 2,
            "the description was not drawn from where its bar was left"
        );
        // AND THE PAGE'S OWN BAR, in the last cell, still moves the page.
        let _ = s.act(Input::Scrolled(ronoctua_design::types::ScrollEvent {
            at: ronoctua_design::types::Position {
                column: s.columns - 1,
                line: s.blocks.body.top,
            },
            first: 4,
        }));
        assert_ne!(s.body_from, from, "the page's bar no longer moves the page");
        assert_eq!(s.about_scroll, 2);
    }

    /// FAILS IF: the footer's last row offers no way to the page of the task
    /// it is about — whatever the task — its click does not open that page,
    /// or its figures are not drawn discreetly.
    #[test]
    fn the_footer_opens_the_page_of_what_it_is_about() {
        let mut s = a_state();
        s.task = 1;
        s.mind.tasks[1].ran = false;
        let screen = drawn(&mut s, 30, 120);
        let seen = runs(&screen);
        let (_, colour, line, column) = seen
            .iter()
            .find(|(t, ..)| t == key::OPEN_PAGE)
            .cloned()
            .expect("a task that never ran has no way to its page");
        assert_eq!(colour, "state-accent");
        s.task = 0;
        let seen = runs(&drawn(&mut s, 30, 120));
        assert_eq!(colour_of(&seen, key::LAST_RUN).as_deref(), Some("text-dim"));
        assert!(
            seen.iter()
                .any(|(t, c, ..)| t == "2026-08-23 13:00" && c == "text-dim"),
            "the last row's figures are not discreet"
        );
        let _ = s.act(Input::Click(ronoctua_design::types::Position {
            column,
            line,
        }));
        assert_eq!(s.page, Page::Task, "the click did not open the page");
        assert_eq!(s.task, 0);
    }

    /// FAILS IF: the list's columns are reserved blind again — a module's
    /// reason cut in a column of seventeen while the name beside it holds a
    /// fraction of its own.
    #[test]
    fn the_lists_columns_are_as_wide_as_what_they_hold() {
        let mut s = a_state();
        s.phrases = Phrases::of(vec![
            ("brain-outcome-done".to_owned(), "Worked".to_owned()),
            ("brain-standing-idle".to_owned(), "Waiting".to_owned()),
        ]);
        s.mind.tasks[1].why_key = "brain-why-when-used".to_owned();
        s.mind.tasks[1].why_text = "After an account was used".to_owned();
        let _ = drawn(&mut s, 30, 120);
        let width = |s: &State, id: &str| s.task_layout.field(id).map(|f| f.width);
        assert_eq!(
            width(&s, "name"),
            Some(15),
            "the name is not as wide as the widest"
        );
        assert_eq!(
            width(&s, "when"),
            Some(25),
            "the reason is cut, or reserved"
        );
        assert_eq!(
            width(&s, "note"),
            Some(6),
            "the status is not as wide as the widest"
        );
        // TWO CELLS BETWEEN THE COLUMNS OF WORDS, the owner's ask, and one
        // between the glyph and its moment.
        let field = |s: &State, id: &str| s.task_layout.field(id).copied().expect(id);
        let after = |s: &State, id: &str| field(s, id).at + field(s, id).width;
        assert_eq!(
            field(&s, "kind").at,
            after(&s, "name") + ronoctua_design::APART
        );
        assert_eq!(
            field(&s, "when").at,
            after(&s, "kind") + ronoctua_design::GAP
        );
        assert_eq!(
            field(&s, "note").at,
            after(&s, "when") + ronoctua_design::APART
        );
        // AND IN A NARROWER WINDOW THE REASON GIVES WAY TO THE ROW — a
        // hundred and twenty cells hold it, fifty leave it twenty-one —
        // and never below a whole date, the status going first.
        let _ = drawn(&mut s, 30, 50);
        assert_eq!(width(&s, "when"), Some(21));
        let _ = drawn(&mut s, 30, 40);
        assert_eq!(width(&s, "when"), Some(WHEN_FLOOR));
        assert_eq!(width(&s, "note"), None, "the status did not give way first");
    }

    fn drawn_texts(screen: &Screen<Wit>) -> (Vec<String>, Vec<String>) {
        let mut keys = Vec::new();
        let mut data = Vec::new();
        for c in screen.as_protocol().commands {
            if let ronoctua_design::frame::Command::Text { text, .. } = c {
                match text {
                    ronoctua_design::frame::Text::Key(k) => keys.push(k),
                    ronoctua_design::frame::Text::Data(d) => data.push(d),
                    ronoctua_design::frame::Text::Icon(_) => {}
                }
            }
        }
        (keys, data)
    }

    /// FAILS IF: what says the same thing three times is three rows, the
    /// folded row does not say how many it folds — or says `Last:` again,
    /// which the owner took off — the title does not say how far back the
    /// log reaches and how many records it holds, or `RET` on a fold does not
    /// open the records it folds — and close them again.
    #[test]
    fn a_folded_row_says_how_many_and_opens_on_return() {
        let mut s = a_state();
        assert_eq!(s.mind.activity_rows.len(), 2, "the fixture stopped folding");
        assert_eq!(s.activity_lines().len(), 2, "a shut fold drew its members");
        let (keys, data) = drawn_texts(&drawn(&mut s, 30, 120));
        assert!(
            !keys.iter().any(|k| k == "brain-activity-last"),
            "the folded row says `Last:` before its date again"
        );
        assert!(
            data.iter().any(|d| d == "3"),
            "the folded row does not say how many it folds: {data:?}"
        );
        // AND THE TITLE SAYS HOW FAR BACK, AND COUNTS RECORDS, not rows.
        assert!(data.iter().any(|d| d == "last 15 days"), "{data:?}");
        assert!(data.iter().any(|d| d == "4"), "{data:?}");

        s.pane = Pane::Log;
        s.log = 1;
        // THE FOOTER SAYS WHEN THE FIRST WAS AND HOW MANY.
        let facts: Vec<String> = s.told().facts.into_iter().map(|(k, _)| k).collect();
        assert!(facts.contains(&key::FIRST.to_owned()), "{facts:?}");
        assert!(facts.contains(&key::TIMES.to_owned()), "{facts:?}");

        s.act(Input::Confirm);
        assert_eq!(s.page, Page::Home, "RET on a fold opened a task instead");
        assert_eq!(
            s.activity_lines(),
            vec![
                Line::Row(0),
                Line::Row(1),
                Line::Member(1, 1),
                Line::Member(1, 2)
            ],
            "the fold did not open into the records it holds"
        );
        // A NEWER RECORD JOINING THE FOLD keeps it open: it is named by its
        // oldest record, which a newer one does not move.
        let mut more = a_mind();
        more.activity_rows = folded(&[
            a_run("fixture-routine", OutcomeKind::Done),
            a_run("fixture-once", OutcomeKind::Held),
            a_run("fixture-once", OutcomeKind::Held),
            a_run("fixture-once", OutcomeKind::Held),
            a_run("fixture-once", OutcomeKind::Held),
        ]);
        s.received(more);
        assert_eq!(
            s.activity_lines().len(),
            5,
            "a record arriving shut the fold"
        );
        // AND A MEMBER OPENS ITS TASK, which is what every record does.
        s.log = 2;
        s.act(Input::Confirm);
        assert_eq!(s.page, Page::Task);
        assert_eq!(s.here().map(|t| t.id.as_str()), Some("fixture-once"));
        // BACK, AND RET SHUTS IT.
        s.act(Input::Cancel);
        s.log = 1;
        s.act(Input::Confirm);
        assert_eq!(s.activity_lines().len(), 2, "RET did not shut the fold");
    }

    /// FAILS IF: a module that only listens draws a date it does not have,
    /// or its page offers a cadence, a window, a run or a rehearsal — or a
    /// task that keeps no calendar is given a cadence and a window.
    #[test]
    fn a_watch_says_watching_and_a_page_without_a_calendar_has_no_cadence() {
        let mut s = a_state();
        s.mind.tasks[1].kind = TaskKind::Watch;
        s.mind.tasks[1].steps.clear();
        let (keys, _) = drawn_texts(&drawn(&mut s, 30, 120));
        assert!(
            keys.iter().any(|k| k == key::WATCHING),
            "a watch's row does not say it is watching"
        );

        s.task = 1;
        let spots = s.spots();
        for gone in [
            Spot::Interval,
            Spot::Window,
            Spot::Next,
            Spot::RunNow,
            Spot::Rehearse,
        ] {
            assert!(!spots.contains(&gone), "a watch's page offers {gone:?}");
        }
        assert!(
            spots.contains(&Spot::Enabled),
            "a watch cannot be switched off"
        );
        // AND ITS PAGE TEACHES NO RUN, and runs none when asked by key.
        s.page = Page::Task;
        let taught: Vec<&str> = s.teaches().iter().map(|(k, _)| *k).collect();
        assert!(
            !taught.contains(&", r") && !taught.contains(&", e"),
            "{taught:?}"
        );
        assert!(s.command("brain-run").is_empty());
        // AND NO STEPS TO PUT BACK: the watch declares none.
        assert!(!taught.contains(&", d"), "{taught:?}");
        assert!(s.command("brain-reset-steps").is_empty());
        // NOR A HISTORY HEADING OVER NO RUNS.
        let mut bare = s.here().cloned().expect("the watch");
        bare.history.clear();
        assert!(
            !s.rows_of(&bare, 100)
                .iter()
                .any(|r| matches!(r, Row::Heading(k) if *k == key::HISTORY)),
            "a watch with no runs drew a history heading"
        );

        let mut s = a_state();
        s.mind.tasks[0].calendar = false;
        let spots = s.spots();
        assert!(!spots.contains(&Spot::Interval) && !spots.contains(&Spot::Window));
        assert!(spots.contains(&Spot::Next) && spots.contains(&Spot::RunNow));
        // AND ITS KEYS REFUSE, since the page draws neither row — and the
        // header does not teach them.
        assert!(s.command("brain-interval").is_empty());
        assert!(s.command("brain-window").is_empty());
        s.page = Page::Task;
        let taught: Vec<&str> = s.teaches().iter().map(|(k, _)| *k).collect();
        assert!(
            !taught.contains(&", i") && !taught.contains(&", w"),
            "{taught:?}"
        );
    }

    /// FAILS IF: the options are two lists again — a title over the fields
    /// and another over the actions, or a blank between them — on a task of
    /// any kind, or the two kinds of row keep their keys in two columns, or an
    /// action's long label pushes the fields' values out.
    #[test]
    fn the_options_are_one_list_with_one_column_of_keys() {
        let mut s = a_state();
        s.phrases = Phrases::of(vec![
            (Phrases::LEADER_ALT.to_owned(), "SUPER ALT".to_owned()),
            (key::ENABLED.to_owned(), "Runs on its own".to_owned()),
            (key::ON.to_owned(), "Yes".to_owned()),
        ]);
        s.mind.tasks[0].actions = vec![ronoctua_design::types::ModuleAction {
            id: "new".to_owned(),
            label_key: "cmd-fixture-new".to_owned(),
            row: false,
            asks_key: String::new(),
        }];
        s.page = Page::Task;
        for kind in [TaskKind::Routine, TaskKind::Once, TaskKind::Watch] {
            s.mind.tasks[0].kind = kind;
            let task = s.mind.tasks[0].clone();
            let rows = s.rows_of(&task, 100);
            let from = rows
                .iter()
                .position(|r| matches!(r, Row::Heading(k) if *k == key::OPTIONS))
                .expect("the options have no title");
            let list = &rows[from + 1..];
            let many = list
                .iter()
                .take_while(|r| matches!(r, Row::Field(_) | Row::Action(_)))
                .count();
            let wanted = State::schedule_of(&task).len() + State::page_actions(&task).count();
            assert_eq!(many, wanted, "{kind:?}");
            assert!(
                list[..many].iter().any(|r| matches!(r, Row::Action(_))),
                "{kind:?}"
            );
        }
        s.mind.tasks[0].kind = TaskKind::Routine;
        let seen = runs(&drawn(&mut s, 40, 100));
        let cell_of = |what: &str| {
            seen.iter()
                .find(|(t, ..)| t == what)
                .map(|(.., column)| *column)
        };
        // THE `⌥` AND THE `RET` BEGIN ON THE SAME CELL, in a column that holds
        // the wider of the two.
        let alt = seen
            .iter()
            .find(|(t, ..)| t.contains("LeaderAlt"))
            .map(|(.., column)| *column);
        assert_eq!(cell_of(RET), alt, "{seen:?}");
        let keys = action_columns(100, key_cells("SUPER ALT"));
        assert_eq!(
            keys.field("keys").map(|f| f.width),
            Some(ronoctua_design::wide(RET))
        );
        // AND AN ACTION'S LABEL, however long, moves no field's value.
        s.phrases = Phrases::of(vec![(
            "cmd-fixture-new".to_owned(),
            "an action with a very long label indeed".to_owned(),
        )]);
        let longer = runs(&drawn(&mut s, 40, 100));
        let moved = longer
            .iter()
            .find(|(t, ..)| t == key::ON)
            .map(|(.., column)| *column);
        s.mind.tasks[0].actions.clear();
        let none = runs(&drawn(&mut s, 40, 100));
        let bare = none
            .iter()
            .find(|(t, ..)| t == key::ON)
            .map(|(.., column)| *column);
        assert!(moved.is_some(), "{longer:?}");
        assert_eq!(moved, bare, "an action's label moved the fields' values");
    }

    /// FAILS IF: a watch is said to run, or told it runs "only when you ask"
    /// — it runs nothing, and that row's switch is about watching — or the
    /// switch's hint is not against its answer, dim, on either kind; or one
    /// that does not fit is cut; or a switch the system holds says what it
    /// is for and offers nothing.
    #[test]
    fn a_task_says_what_its_switch_is_for_in_its_own_words() {
        let mut s = a_state();
        s.page = Page::Task;
        s.phrases = Phrases::of(vec![
            (key::OFF.to_owned(), "No".to_owned()),
            (key::OFF_WATCH.to_owned(), "Not at all".to_owned()),
            (key::ON.to_owned(), "Yes".to_owned()),
            (key::ENABLED.to_owned(), "Runs".to_owned()),
            (key::ENABLED_WATCH.to_owned(), "Watches".to_owned()),
            (key::ENABLED_HINT.to_owned(), "(the schedule)".to_owned()),
            (
                key::ENABLED_HINT_WATCH.to_owned(),
                "(monitoring)".to_owned(),
            ),
        ]);
        let drew = |s: &mut State, columns| runs(&drawn(s, 40, columns));
        let seen = drew(&mut s, 100);
        assert_eq!(
            colour_of(&seen, key::ENABLED_HINT).as_deref(),
            Some("text-dim")
        );
        assert!(colour_of(&seen, key::ENABLED).is_some());
        assert!(colour_of(&seen, key::ENABLED_WATCH).is_none());
        // RIGHT AFTER THE ANSWER, whichever column the row's widest word left.
        let at = |seen: &[(String, String, u16, u16)], what| {
            seen.iter()
                .find(|(t, ..)| t == what)
                .map(|(.., column)| *column)
        };
        let yes = at(&seen, key::ON).expect("the answer");
        assert_eq!(at(&seen, key::ENABLED_HINT), Some(yes + 3 + 1));
        // AND THE ANSWER RESERVES NO MORE THAN ITS WORD: the host cuts a key
        // at the width it was given, so a column-wide answer reaches under the
        // hint beside it (`brain-driven` measures every key the same way).
        for columns in [80, 100] {
            drawn(&mut s, 40, columns)
                .check()
                .unwrap_or_else(|e| panic!("at {columns} columns: {e}"));
        }

        s.mind.tasks[0].kind = TaskKind::Watch;
        s.mind.tasks[0].enabled = false;
        s.spot = 0;
        let seen = drew(&mut s, 100);
        for said in [key::ENABLED_WATCH, key::OFF_WATCH, key::ENABLED_HINT_WATCH] {
            assert!(colour_of(&seen, said).is_some(), "`{said}`");
        }
        for not in [key::ENABLED, key::OFF, key::ENABLED_HINT] {
            assert!(colour_of(&seen, not).is_none(), "`{not}`");
        }
        let off = at(&seen, key::OFF_WATCH).expect("the answer");
        assert_eq!(at(&seen, key::ENABLED_HINT_WATCH), Some(off + 10 + 1));

        // NOT WHOLE, NOT AT ALL.
        let seen = drew(&mut s, 34);
        assert!(colour_of(&seen, key::OFF_WATCH).is_some(), "{seen:?}");
        assert!(
            colour_of(&seen, key::ENABLED_HINT_WATCH).is_none(),
            "a hint with no room was cut: {seen:?}"
        );

        // AND A SWITCH THE SYSTEM HOLDS is no switch to explain.
        s.mind.tasks[0].why_not = "system-off".to_owned();
        s.mind.tasks[0].why_key = "fixture-why-system".to_owned();
        let seen = drew(&mut s, 100);
        assert!(colour_of(&seen, "fixture-why-system").is_some());
        assert!(colour_of(&seen, key::ENABLED_HINT_WATCH).is_none());
    }

    /// A module's page, of two sections, one row acting and one telling.
    fn a_screen(module: &str, generation: u64) -> ModuleScreen {
        let row = |id: &str, act: &str| ModuleRow {
            id: id.to_owned(),
            label: ScreenCell::Key(format!("fixture-row-{id}")),
            value: ScreenCell::Count(3),
            note: ScreenCell::Data("a note the module wrote".to_owned()),
            tone: BrainTone::Watching,
            act: act.to_owned(),
            also: if act.is_empty() {
                String::new()
            } else {
                "ignore".to_owned()
            },
            below: ScreenCell::Empty,
            foot: Vec::new(),
        };
        let section = |heading: &str, rows| ronoctua_design::types::ModuleSection {
            heading: heading.to_owned(),
            rows,
            id: String::new(),
            about: String::new(),
            fold: ronoctua_design::types::SectionFold::Never,
            tally: ScreenCell::Empty,
        };
        ModuleScreen {
            module: module.to_owned(),
            generation,
            sections: vec![
                section("fixture-heading-one", vec![row("a", "forget")]),
                section("fixture-heading-two", vec![row("b", "")]),
            ],
            refused: String::new(),
            legend: Vec::new(),
            phrases: Vec::new(),
        }
    }

    /// A row of the journal's shape: a date, a line of data and a count.
    fn a_dated_row(id: &str, line: &str, count: u64) -> ModuleRow {
        ModuleRow {
            id: id.to_owned(),
            label: ScreenCell::Moment(Timed {
                at: 5,
                text: "2026-08-23 14:02".to_owned(),
                stamp: ronoctua_design::types::Stamp {
                    lead: "2026-08-".to_owned(),
                    day: "23".to_owned(),
                    tail: " 14:02".to_owned(),
                    today: false,
                },
            }),
            value: ScreenCell::Data(line.to_owned()),
            note: ScreenCell::Count(count),
            tone: BrainTone::Danger,
            act: "forget".to_owned(),
            also: String::new(),
            below: ScreenCell::Empty,
            foot: Vec::new(),
        }
    }

    /// The words a page came with, as the host resolved them: every key of
    /// `said`, spelt as `text`.
    fn with_words(mut page: ModuleScreen, said: &[(&str, &str)]) -> ModuleScreen {
        page.phrases = said
            .iter()
            .map(|(key, text)| ronoctua_design::types::Phrase {
                key: (*key).to_owned(),
                text: (*text).to_owned(),
            })
            .collect();
        page
    }

    /// The page in front, `page`, laid out for `s`.
    fn on_the_page(s: &mut State, page: ModuleScreen) {
        if s.page != Page::Task {
            let _ = s.act(Input::Confirm);
            let _ = s.watch_asks();
        }
        s.act(Input::ModuleScreen(page));
    }

    /// FAILS IF: a message stops at the twenty-six cells the reservations
    /// leave it at seventy-three columns, one section's width squeezes
    /// another's rows, or a count does not stand at the edge in a count's
    /// tone while every other note stays dim.
    #[test]
    fn a_rows_columns_are_as_wide_as_what_they_hold() {
        let mut s = a_state();
        let mut page = a_screen("fixture-routine", 2);
        page.sections[0].rows = vec![a_dated_row("a", "a line", 3536), a_dated_row("b", "a", 9)];
        page.sections[1].rows[0].label = ScreenCell::Data("x".repeat(30));
        on_the_page(&mut s, with_words(page, &[("fixture-heading-one", "One")]));
        let layouts = s.screen_layouts(&s.mind.tasks[0].clone(), 73);
        for (columns, wanted) in [(73, 50), (90, 67), (100, 77)] {
            let layouts = s.screen_layouts(&s.mind.tasks[0].clone(), columns);
            assert_eq!(layouts[0].field("value").map(|f| f.width), Some(wanted));
        }
        assert_eq!(layouts[0].field("label").map(|f| f.width), Some(16));
        assert_eq!(layouts[0].field("note").map(|f| f.width), Some(4));
        // THE OTHER SECTION is laid out by its own rows — and its long label
        // takes nothing from a value that still needs it.
        assert_eq!(layouts[2].field("label").map(|f| f.width), Some(24));
        let wide = s.screen_layouts(&s.mind.tasks[0].clone(), 100);
        assert_eq!(wide[2].field("label").map(|f| f.width), Some(30));
        let seen = runs(&drawn(&mut s, 40, 73));
        let at = |what: &str| {
            seen.iter()
                .find(|(t, ..)| t == what)
                .map(|(_, c, _, x)| (c.as_str(), *x))
        };
        // THE LAST DIGIT OF EITHER, on the row's last cell.
        assert_eq!(at("3536"), Some(("text-secondary", 68)));
        assert_eq!(at("9"), Some(("text-secondary", 71)));
        // AND A NOTE THAT IS NO COUNT stays where it starts, and dim.
        assert_eq!(
            colour_of(&seen, "a note the module wrote").as_deref(),
            Some("text-dim")
        );
    }

    /// FAILS IF: a label with nothing beside it is cut at a column the rows
    /// around it need, or widens that column for them, or a count beside it
    /// leaves the edge the other rows' stand at — an empty text is nothing
    /// too — or a column nothing in the section fills is kept empty, its
    /// width taken from the value.
    #[test]
    fn a_sentence_takes_the_line_and_an_empty_column_is_left_out() {
        let mut s = a_state();
        let mut page = a_screen("fixture-routine", 2);
        let sentence = "a sentence of fifty cells that a column would cut";
        page.sections[0].rows = vec![
            ModuleRow {
                label: ScreenCell::Key("fixture-sentence".to_owned()),
                value: ScreenCell::Empty,
                note: ScreenCell::Empty,
                ..a_dated_row("a", "", 0)
            },
            ModuleRow {
                note: ScreenCell::Empty,
                ..a_dated_row("b", "a line with no note", 0)
            },
        ];
        page.sections[1].rows = vec![
            ModuleRow {
                label: ScreenCell::Key("fixture-sentence".to_owned()),
                value: ScreenCell::Data(String::new()),
                note: ScreenCell::Count(12),
                ..a_dated_row("c", "", 0)
            },
            a_dated_row("d", "a line beside a count", 12_345),
        ];
        on_the_page(&mut s, with_words(page, &[("fixture-sentence", sentence)]));
        let layouts = s.screen_layouts(&s.mind.tasks[0].clone(), 100);
        assert_eq!(layouts[0].fields().len(), 1, "{:?}", layouts[0]);
        assert_eq!(layouts[0].field("label").map(|f| f.width), Some(99));
        assert_eq!(layouts[1].field("label").map(|f| f.width), Some(16));
        assert!(!layouts[1].has("note"), "an empty column was kept");
        assert_eq!(layouts[1].field("value").map(|f| f.width), Some(82));
        // A LABEL WITH NO VALUE takes the line up to the note, and the note
        // stands where the section's others do.
        assert_eq!(layouts[2].field("label").map(|f| f.width), Some(93));
        assert_eq!(layouts[2].field("note"), layouts[3].field("note"));
        assert_eq!(layouts[3].field("label").map(|f| f.width), Some(16));
    }

    /// FAILS IF: at some width a column that today's page has is gone from
    /// a measured section, whatever its cells hold, or a label past the
    /// reservation or a note beside a short value costs the value what it
    /// was promised — or a host that sends no words draws anything but the
    /// columns it always had.
    #[test]
    fn no_column_of_today_goes_missing_at_any_width() {
        fn todays(columns: u16) -> Layout {
            Table::across(columns.saturating_sub(1))
                .with(Column::fixed("label", 24, 2))
                .with(Column::fill("value", 10))
                .with(Column::fixed("note", 20, 3))
                .resolve()
        }
        let mut s = a_state();
        let mut compared = 0;
        let long_line = "v".repeat(200);
        for line in ["a line", long_line.as_str()] {
            for label in [1_usize, 16, 24, 25, 39, 40, 60] {
                for note in [1_usize, 4, 19, 20, 30] {
                    let mut page = a_screen("fixture-routine", 2);
                    let mut row = a_dated_row("a", line, 0);
                    row.label = ScreenCell::Key("fixture-label".to_owned());
                    row.note = ScreenCell::Key("fixture-note".to_owned());
                    page.sections[0].rows = vec![row];
                    let (spelt_label, spelt_note) = ("l".repeat(label), "n".repeat(note));
                    on_the_page(
                        &mut s,
                        with_words(
                            page,
                            &[
                                ("fixture-label", &spelt_label),
                                ("fixture-note", &spelt_note),
                            ],
                        ),
                    );
                    for columns in 30..=160 {
                        let task = s.mind.tasks[0].clone();
                        let layout = &s.screen_layouts(&task, columns)[0];
                        for field in todays(columns).fields() {
                            assert!(
                                layout.has(field.id),
                                "`{}` went missing at {columns}: {label}, {note}",
                                field.id
                            );
                            compared += 1;
                        }
                        assert!(layout.ends() < columns, "{layout:?} leaks at {columns}");
                        // A LABEL PAST THE RESERVATION leaves the value what
                        // it keeps, or what it holds when that is less.
                        let width = |id| layout.field(id).map_or(0, |f| f.width);
                        if width("label") > LABEL_BLIND {
                            let holds = ronoctua_design::wide(line).max(VALUE_LEAST);
                            assert!(
                                width("value") >= VALUE_KEPT.min(holds),
                                "{layout:?} at {columns}: the label took the value"
                            );
                        }
                    }
                }
            }
        }
        floor_of(compared, 6_000, "columns compared with today's");
        // A SHORT NOTE STAYS where the reservation gave way, and pays with
        // cells of the value: nineteen today at this width, with no note.
        let mut page = a_screen("fixture-routine", 2);
        let mut row = a_dated_row("a", &long_line, 0);
        row.label = ScreenCell::Key("fixture-label".to_owned());
        row.note = ScreenCell::Count(1_000);
        page.sections[0].rows = vec![row];
        on_the_page(
            &mut s,
            with_words(page, &[("fixture-label", &"l".repeat(24))]),
        );
        let layout = &s.screen_layouts(&s.mind.tasks[0].clone(), 45)[0];
        assert_eq!(layout.field("value").map(|f| f.width), Some(14));
        assert_eq!(layout.field("note").map(|f| f.width), Some(4));
        // AND A HOST THAT SAID NOTHING HAS TODAY'S PAGE, EXACTLY.
        let mut page = a_screen("fixture-routine", 2);
        page.sections[0].rows = vec![a_dated_row("a", "a line", 3)];
        on_the_page(&mut s, page);
        for columns in 30..=160 {
            let task = s.mind.tasks[0].clone();
            assert_eq!(s.screen_layouts(&task, columns)[0], todays(columns));
        }
    }

    /// FAILS IF: a note stops at its reservation with the rest of the row
    /// empty beside a value that is known to be short, or takes a cell a value
    /// needs, or leaves it under its floor so the note gives way, or a value
    /// whose words nobody said stops holding the row, or a sentence in the
    /// value's room is squeezed — or its note leaves the section's column.
    #[test]
    fn a_note_takes_what_the_value_leaves() {
        let date = a_dated_row("a", "", 0).label;
        let long_note = "n".repeat(60);
        let row = |id: &str, value: ScreenCell| ModuleRow {
            label: ScreenCell::Key("fixture-label".to_owned()),
            value,
            note: ScreenCell::Data(long_note.clone()),
            ..a_dated_row(id, "", 0)
        };
        let label = "l".repeat(20);
        let sentence = "s".repeat(50);
        let said: &[(&str, &str)] = &[("fixture-label", &label), ("fixture-sentence", &sentence)];
        let mut s = a_state();
        let width = |s: &mut State, rows: Vec<ModuleRow>, columns: u16, id: &str| {
            let mut page = a_screen("fixture-routine", 2);
            page.sections[0].rows = rows;
            on_the_page(s, with_words(page, said));
            let layouts = s.screen_layouts(&s.mind.tasks[0].clone(), columns);
            layouts.first().and_then(|l| l.field(id)).map(|f| f.width)
        };
        // A DATE BESIDE A LINE OF THE MODULE'S: the note has the row.
        let dated = vec![row("a", date.clone())];
        assert_eq!(width(&mut s, dated.clone(), 100, "note"), Some(60));
        // WITH NOT ENOUGH FOR IT, what the date leaves, and the date is whole.
        assert_eq!(width(&mut s, dated.clone(), 73, "note"), Some(34));
        assert_eq!(width(&mut s, dated, 73, "value"), Some(16));
        // A VALUE SHORTER THAN ITS FLOOR is not given less than the floor.
        let counted = vec![row("a", ScreenCell::Count(3))];
        assert_eq!(width(&mut s, counted.clone(), 60, "note"), Some(27));
        assert_eq!(width(&mut s, counted, 60, "value"), Some(10));
        // A LINE AS LONG AS THE ROW leaves it the reservation, as today.
        let line = ScreenCell::Data("v".repeat(200));
        assert_eq!(width(&mut s, vec![row("a", line)], 100, "note"), Some(20));
        // AND SO DOES A KEY THE HOST DID NOT SAY.
        let unsaid = ScreenCell::Key("fixture-unsaid".to_owned());
        assert_eq!(width(&mut s, vec![row("a", unsaid)], 100, "note"), Some(20));
        // A SENTENCE IS THE VALUE'S ROOM: its note stands in the column of
        // the rows around it, and it is not cut for it.
        let sentence_row = ModuleRow {
            label: ScreenCell::Key("fixture-sentence".to_owned()),
            ..row("b", ScreenCell::Empty)
        };
        let mut page = a_screen("fixture-routine", 2);
        page.sections[0].rows = vec![row("a", date), sentence_row];
        on_the_page(&mut s, with_words(page, said));
        let layouts = s.screen_layouts(&s.mind.tasks[0].clone(), 100);
        assert_eq!(layouts[0].field("note"), layouts[1].field("note"));
        assert_eq!(layouts[0].field("note").map(|f| f.width), Some(27));
        assert_eq!(layouts[1].field("label").map(|f| f.width), Some(71));
    }

    /// A page of four sections, three of which fold: the first open, the
    /// second shut and with the module's own number, the third open — and one
    /// that does not fold at the end, as every other module's do. Each row
    /// acts, by its section's letter and its place in it.
    fn a_folding_page(generation: u64) -> ModuleScreen {
        use ronoctua_design::types::SectionFold::{Never, Open, Shut};
        let section = |id: &str, fold, rows: usize| ModuleSection {
            heading: format!("fixture-heading-{id}"),
            rows: (0..rows)
                .map(|n| a_dated_row(&format!("{id}-{n}"), "a line", 3))
                .collect(),
            id: id.to_owned(),
            about: format!("fixture-about-{id}"),
            fold,
            tally: ScreenCell::Empty,
        };
        let mut page = a_screen("fixture-routine", generation);
        page.sections = vec![
            section("first", Open, 3),
            ModuleSection {
                tally: ScreenCell::Count(1_234),
                ..section("second", Shut, 2)
            },
            section("third", Open, 1),
            section("fourth", Never, 1),
        ];
        page
    }

    /// Two swatches, the second a word nobody could fit beside the first in
    /// a narrow window.
    fn a_legend() -> Vec<LegendSwatch> {
        vec![
            LegendSwatch {
                tone: BrainTone::Critical,
                says: "fixture-worst".to_owned(),
                about: "fixture-worst-about".to_owned(),
            },
            LegendSwatch {
                tone: BrainTone::Minor,
                says: "fixture-least".to_owned(),
                about: "fixture-least-about".to_owned(),
            },
        ]
    }

    /// The fold a click or a press asked for: which section, and which way.
    fn fold_asked(out: &[Effect]) -> Option<(String, bool)> {
        out.iter().find_map(|e| match e {
            Effect::Tune(Setting {
                knob: Knob::Fold(section),
                ..
            }) => Some((section.id.clone(), section.open)),
            _ => None,
        })
    }

    /// The row of the page in front that a spot is on.
    fn on_spot(s: &mut State, wanted: Spot) {
        s.spot = s
            .spots()
            .iter()
            .position(|sp| *sp == wanted)
            .unwrap_or_else(|| panic!("the page has no {wanted:?}"));
    }

    /// What names the spot under the cursor, to read a test by.
    fn cursor_on(s: &State) -> Option<(String, String)> {
        let task = s.here()?;
        let shown = s.shown_sections(task);
        let sections = &s.page_of(task)?.sections;
        match s.spot_here()? {
            Spot::Section(n) => Some((sections[n].id.clone(), String::new())),
            Spot::Screen(k) => {
                let at = shown
                    .iter()
                    .find(|sh| (sh.from..sh.from + sh.rows).contains(&k))?;
                let section = &sections[at.section];
                Some((section.id.clone(), section.rows[k - at.from].id.clone()))
            }
            _ => None,
        }
    }

    /// THE TRAP OF THIS PAGE. FAILS IF: a shut section is not counted in the
    /// index an `act-row` names — the host counts the page it was sent, so
    /// `RET` on the row after it would act on a row nobody can see.
    #[test]
    fn a_shut_section_keeps_the_index_of_the_rows_after_it() {
        let mut s = a_state();
        on_the_page(&mut s, a_folding_page(2));
        let spots = s.spots();
        // THE ROWS OF THE SHUT SECTION ARE NOT ON THE PAGE.
        for gone in [Spot::Screen(3), Spot::Screen(4)] {
            assert!(!spots.contains(&gone), "{gone:?} is under a shut title");
        }
        // AND THE ROW AFTER THEM IS STILL THE SIXTH OF WHAT THE HOST HOLDS.
        on_spot(&mut s, Spot::Screen(5));
        assert_eq!(cursor_on(&s).map(|(_, id)| id).as_deref(), Some("third-0"));
        let asks = |out: &[Effect]| {
            out.iter().find_map(|e| match e {
                Effect::Ask(d) => Some((d.what, d.target)),
                _ => None,
            })
        };
        assert_eq!(asks(&s.confirm()), Some((Request::ActRow, 6)));
        assert_eq!(
            asks(&s.command("brain-row-also")),
            None,
            "a row with no second action asked for one"
        );
        let mut page = a_folding_page(3);
        page.sections[2].rows[0].also = "ignore".to_owned();
        s.act(Input::ModuleScreen(page));
        assert_eq!(
            asks(&s.command("brain-row-also")),
            Some((Request::ActRowAlso, 6))
        );
        // OPEN, ITS ROWS TAKE THE SAME NUMBERS THEY ALWAYS HAD.
        s.mind.tasks[0].folds = vec![SectionState {
            id: "second".to_owned(),
            open: true,
        }];
        on_spot(&mut s, Spot::Screen(4));
        assert_eq!(asks(&s.confirm()), Some((Request::ActRow, 5)));
        on_spot(&mut s, Spot::Screen(5));
        assert_eq!(asks(&s.confirm()), Some((Request::ActRow, 6)));
    }

    /// FAILS IF: a heading and a row disagree about where a spot is drawn,
    /// whichever way the sections are folded, or a section that does not fold
    /// gains a spot — the sixteen modules that fold nothing keep their page.
    #[test]
    fn the_spots_and_the_rows_stay_parallel_however_it_is_folded() {
        let mut s = a_state();
        on_the_page(&mut s, a_folding_page(2));
        for (first, second, third) in [
            (true, true, true),
            (false, true, true),
            (true, false, false),
            (false, false, false),
        ] {
            let word = |id: &str, open| SectionState {
                id: id.to_owned(),
                open,
            };
            s.mind.tasks[0].folds = vec![
                word("first", first),
                word("second", second),
                word("third", third),
            ];
            let task = s.mind.tasks[0].clone();
            let rows = s.rows_of(&task, 100);
            let spots = s.spots();
            assert_eq!(
                rows.iter().filter(|r| is_a_spot(r)).count(),
                spots.len(),
                "the two lists have different lengths"
            );
            for (n, spot) in spots.iter().enumerate() {
                let row = State::row_of_spot(&rows, *spot)
                    .unwrap_or_else(|| panic!("{spot:?} is drawn on no row"));
                assert_eq!(State::spot_of_row(&rows, row), Some(n), "{spot:?}");
            }
            let titles = spots
                .iter()
                .filter(|sp| matches!(sp, Spot::Section(_)))
                .count();
            assert_eq!(titles, 3, "a title lost its spot, or a plain one won one");
            assert!(
                rows.iter().any(|r| matches!(r, Row::Section(3))),
                "the section that does not fold is not a plain title"
            );
        }
    }

    /// FAILS IF: a title folds on the first click — inv. 19: the first
    /// selects, and the footer has to be able to say what it is — or the
    /// second does not, or `RET` differs from it, or what is asked for is not
    /// the other way from how the section is DRAWN: the person's word first,
    /// and only then how the module ships it.
    #[test]
    fn a_title_selects_first_and_folds_on_the_second_click() {
        let mut s = a_state();
        on_the_page(&mut s, a_folding_page(2));
        let seen = runs(&drawn(&mut s, 70, 100));
        let (.., line, column) = seen
            .iter()
            .find(|(t, ..)| t == "fixture-heading-second")
            .cloned()
            .expect("the shut section's title");
        let first = s.clicked(column, line);
        assert_eq!(fold_asked(&first), None, "the first click folded it");
        assert_eq!(s.spot_here(), Some(Spot::Section(1)));
        let second = s.clicked(column, line);
        assert_eq!(fold_asked(&second), Some(("second".to_owned(), true)));
        assert_eq!(fold_asked(&s.confirm()), Some(("second".to_owned(), true)));
        // THE MIND'S WORD WINS, in both directions.
        s.mind.tasks[0].folds = vec![SectionState {
            id: "second".to_owned(),
            open: true,
        }];
        assert_eq!(fold_asked(&s.confirm()), Some(("second".to_owned(), false)));
        on_spot(&mut s, Spot::Section(0));
        assert_eq!(fold_asked(&s.confirm()), Some(("first".to_owned(), false)));
        // AND IT DRAWS WHAT THE MIND SAYS: the rows it holds are on the page.
        let task = s.mind.tasks[0].clone();
        let rows = s.rows_of(&task, 100);
        assert!(rows.iter().any(|r| matches!(r, Row::Screen(3, 1))));
    }

    /// FAILS IF: a title does not say whether it is open by its triangle, how
    /// many by its count at the row's edge — the module's number when it
    /// gave one, the rows when not — or its band and hover are not a row's,
    /// or it is not told in the footer and under the pointer: the triangle
    /// what a click does, everything else what the section is, the count too.
    #[test]
    fn a_title_says_what_it_holds_and_how_it_folds() {
        use ronoctua_design::frame::{Command, Text};
        let mut s = a_state();
        on_the_page(&mut s, a_folding_page(2));
        on_spot(&mut s, Spot::Section(1));
        let screen = drawn(&mut s, 70, 100);
        let seen = runs(&screen);
        let at = |what: &str| {
            seen.iter()
                .find(|(t, ..)| t == what)
                .map(|(_, c, line, column)| (c.clone(), *line, *column))
                .unwrap_or_else(|| panic!("`{what}` is not drawn"))
        };
        let (_, shut, _) = at("fixture-heading-second");
        let (_, open, _) = at("fixture-heading-first");
        let glyph = |line: u16| {
            seen.iter()
                .find(|(t, _, l, c)| *l == line && *c == 0 && t.contains("BrainGroup"))
                .map(|(t, c, ..)| (t.clone(), c.clone()))
                .expect("a title has no triangle")
        };
        assert!(glyph(shut).0.ends_with("BrainGroup"), "{:?}", glyph(shut));
        assert!(glyph(open).0.ends_with("BrainGroupOpen"));
        assert_eq!(glyph(open).1, "text-secondary");
        // THE NUMBER STANDS AT THE EDGE: its last digit on the row's last cell.
        assert_eq!(at("1234"), ("text-secondary".to_owned(), shut, 99 - 4));
        // WITH NO NUMBER OF ITS OWN, how many rows it holds.
        assert_eq!(at("3"), ("text-secondary".to_owned(), open, 99 - 1));
        // THE BAND IS A ROW'S.
        let banded = screen.as_protocol().commands.iter().any(|c| {
            matches!(c, Command::Selection { line, color, .. }
                if *line == shut && color.as_str() == "surface-selection")
        });
        assert!(banded, "the cursor's title has no band");
        // THE FOOTER.
        let told = s.told();
        assert_eq!(told.name.as_deref(), Some("fixture-heading-second"));
        assert_eq!(told.about.as_deref(), Some("fixture-about-second"));
        // AND THE POINTER.
        let pointed = |s: &mut State, column, line| {
            let _ = s.hover.set(0, (column, line));
            let screen = drawn(s, 70, 100);
            screen
                .as_protocol()
                .commands
                .into_iter()
                .find_map(|c| match c {
                    Command::Pointed { about, .. } => Some(about),
                    _ => None,
                })
        };
        let fold = Some(Text::Key(key::ABOUT_SECTION_FOLD.into()));
        let about = Some(Text::Key("fixture-about-second".into()));
        assert_eq!(pointed(&mut s, 0, shut), fold);
        assert_eq!(pointed(&mut s, 1, shut), fold);
        assert_eq!(pointed(&mut s, 3, shut), about, "the title");
        assert_eq!(pointed(&mut s, 97, shut), about, "the count");
        // A SECTION THAT DOES NOT FOLD says the module's words too, and the
        // brain's when the module has none.
        let (_, plain, _) = at("fixture-heading-fourth");
        assert_eq!(
            pointed(&mut s, 3, plain),
            Some(Text::Key("fixture-about-fourth".into()))
        );
        let mut page = a_folding_page(3);
        page.sections[3].about = String::new();
        s.act(Input::ModuleScreen(page));
        assert_eq!(
            pointed(&mut s, 3, plain),
            Some(Text::Key(key::ABOUT_SECTION.into()))
        );
    }

    /// THE RULE EVERY LIST THAT REORDERS KEEPS. FAILS IF: the cursor stays
    /// at its index while the page moves under it — a row added above,
    /// sections reordered — or follows a row into another section, which is
    /// what `RET` on "mark seen" does to a row, or ends anywhere but the next
    /// row of its section when its own row leaves.
    #[test]
    fn the_cursor_stays_on_its_row_within_its_section() {
        let row = |id: &str| a_dated_row(id, "a line", 3);
        let mut s = a_state();
        on_the_page(&mut s, a_folding_page(2));
        on_spot(&mut s, Spot::Screen(1));
        let here = |s: &State| cursor_on(s).expect("the cursor is on a row");

        // A ROW ADDED ABOVE IT.
        let mut page = a_folding_page(3);
        page.sections[0].rows.insert(0, row("first-new"));
        s.act(Input::ModuleScreen(page));
        assert_eq!(here(&s), ("first".to_owned(), "first-1".to_owned()));

        // A SECTION ADDED ABOVE IT, whose index and title move.
        let mut page = a_folding_page(4);
        let before = ModuleSection {
            id: "before".to_owned(),
            ..page.sections[0].clone()
        };
        page.sections.insert(0, before);
        s.act(Input::ModuleScreen(page));
        assert_eq!(here(&s), ("first".to_owned(), "first-1".to_owned()));

        // ITS ROW LEFT FOR ANOTHER SECTION, as a row marked seen does: the
        // row after it takes the place, and the cursor does not go with it.
        let mut page = a_folding_page(5);
        let moved = page.sections[0].rows.remove(1);
        page.sections[2].rows.push(moved);
        s.act(Input::ModuleScreen(page));
        assert_eq!(here(&s), ("first".to_owned(), "first-2".to_owned()));

        // THE LAST ONE LEFT: the row before it, still in the section.
        let mut page = a_folding_page(6);
        page.sections[0].rows.pop();
        s.act(Input::ModuleScreen(page));
        assert_eq!(here(&s), ("first".to_owned(), "first-1".to_owned()));

        // THE WHOLE SECTION LEFT: what stands where it stood, which is the
        // next title — never a row, which `RET` would act on.
        let mut page = a_folding_page(7);
        page.sections[0].rows.clear();
        s.act(Input::ModuleScreen(page));
        assert_eq!(here(&s), ("second".to_owned(), String::new()));
    }

    /// A page of the shape a module with no section ids draws: one section per
    /// thing it keeps, each titled by its kind, so the titles repeat. Each row
    /// is `<who>:<what>`, and each section holds `what`.
    fn a_page_of_twins(generation: u64, sections: &[(&str, &[&str])]) -> ModuleScreen {
        let section = |(who, what): &(&str, &[&str])| ModuleSection {
            heading: "fixture-heading-twin".to_owned(),
            rows: what
                .iter()
                .map(|w| a_dated_row(&format!("{who}:{w}"), "a line", 3))
                .collect(),
            id: String::new(),
            about: String::new(),
            fold: ronoctua_design::types::SectionFold::Never,
            tally: ScreenCell::Empty,
        };
        ModuleScreen {
            sections: sections.iter().map(section).collect(),
            ..a_screen("fixture-routine", generation)
        }
    }

    /// FAILS IF: a row of a section that shares its title with another goes
    /// to the first of them when anything arrives — a section with no id is
    /// named by its title, so the cursor would land on the row at the same
    /// place of the first one and `RET` would act on that — or the row that
    /// takes the place of one that left comes from the wrong section, or a
    /// section that left hands the cursor to its twin.
    #[test]
    fn a_cursor_keeps_its_row_among_sections_of_one_title() {
        let rows: &[&str] = &["sound", "forget"];
        let row_acted_on = |s: &mut State| {
            s.confirm().iter().find_map(|e| match e {
                Effect::Ask(d) if d.what == Request::ActRow => Some(d.target),
                _ => None,
            })
        };
        let mut s = a_state();
        on_the_page(&mut s, a_page_of_twins(2, &[("1", rows), ("2", rows)]));
        on_spot(&mut s, Spot::Screen(3));
        // A MIND AND THE SAME PAGE AGAIN, which every arrival is.
        let mind = s.mind.clone();
        s.act(Input::Mind(mind));
        assert_eq!(s.spot_here(), Some(Spot::Screen(3)), "after a mind");
        s.act(Input::ModuleScreen(a_page_of_twins(
            3,
            &[("1", rows), ("2", rows)],
        )));
        assert_eq!(s.spot_here(), Some(Spot::Screen(3)), "after a page");
        assert_eq!(row_acted_on(&mut s), Some(4), "`RET` acted on its twin");
        // THEY CHANGED PLACES, and it goes with its row.
        s.act(Input::ModuleScreen(a_page_of_twins(
            4,
            &[("2", rows), ("1", rows)],
        )));
        assert_eq!(s.spot_here(), Some(Spot::Screen(1)), "after a swap");
        // ITS ROW LEFT, and what takes the place is of the section it was in.
        on_spot(&mut s, Spot::Screen(3));
        s.act(Input::ModuleScreen(a_page_of_twins(
            5,
            &[("2", rows), ("1", &["sound", "again"])],
        )));
        assert_eq!(s.spot_here(), Some(Spot::Screen(3)), "after its row left");
        // THE WHOLE SECTION LEFT: what stands where it stood, never a row of
        // the twin at the same place.
        let stood = s.spot - 1;
        s.act(Input::ModuleScreen(a_page_of_twins(6, &[("2", rows)])));
        assert_eq!(s.spot, stood.min(s.spots().len() - 1));
        assert_ne!(s.spot_here(), Some(Spot::Screen(1)), "handed to its twin");
    }

    /// FAILS IF: a title is kept by its index when the sections move, or a
    /// section shut in another window leaves the cursor on a row that is not
    /// drawn, or the answer to a fold moves it — the mind brings every fold
    /// back, and a person pressing `RET` on a title stays on it.
    #[test]
    fn a_title_is_kept_by_its_section() {
        let mut s = a_state();
        on_the_page(&mut s, a_folding_page(2));
        on_spot(&mut s, Spot::Section(2));
        let shut = |s: &mut State, id: &str| {
            let mut mind = s.mind.clone();
            mind.tasks[0].folds = vec![SectionState {
                id: id.to_owned(),
                open: false,
            }];
            s.act(Input::Mind(mind));
        };
        shut(&mut s, "first");
        assert_eq!(
            cursor_on(&s).map(|(id, _)| id).as_deref(),
            Some("third"),
            "rows gone above it moved the cursor off its title"
        );
        let mut page = a_folding_page(3);
        page.sections.swap(0, 2);
        s.act(Input::ModuleScreen(page));
        assert_eq!(s.spot_here(), Some(Spot::Section(0)));
        assert_eq!(cursor_on(&s).map(|(id, _)| id).as_deref(), Some("third"));

        // SHUT IN ANOTHER WINDOW WITH THE CURSOR INSIDE IT.
        on_spot(&mut s, Spot::Screen(0));
        assert_eq!(cursor_on(&s).map(|(id, _)| id).as_deref(), Some("third"));
        shut(&mut s, "third");
        assert_eq!(s.spot_here(), Some(Spot::Section(0)), "under a shut title");
    }

    /// FAILS IF: the index a page's action sends is anything but its place in
    /// `actions`, row actions included, the count the host's `act_page` reads
    /// (`an_action_of_a_page_is_read_from_the_mind_it_was_counted_in`). The
    /// daemon's half, the list with what a module holds back taken out, is
    /// `an_action_for_a_module_switched_off_leaves_the_list_itself_while_it_is_on`.
    #[test]
    fn a_page_action_is_sent_by_its_place_in_the_list() {
        let action = |id: &str, row: bool| ronoctua_design::types::ModuleAction {
            id: id.to_owned(),
            label_key: format!("cmd-fixture-{id}"),
            row,
            asks_key: String::new(),
        };
        let mut s = a_state();
        // THE LIST WITH THE MODULE ON, as the mind brings it: row actions
        // before the page's own.
        s.mind.tasks[0].actions = vec![
            action("seen", true),
            action("ignore", true),
            action("unignore", true),
            action("seen-all", false),
            action("filter", false),
            action("follow", false),
        ];
        s.page = Page::Task;
        let at = s.mind.tasks[0]
            .actions
            .iter()
            .position(|a| a.id == "seen-all")
            .expect("the fixture");
        on_spot(&mut s, Spot::Action(at));
        let out = s.confirm();
        let [Effect::Ask(Demand { what, target })] = out.as_slice() else {
            panic!("{out:?}");
        };
        assert_eq!(*what, Request::ActPage);
        assert_eq!(*target, 4, "one-based, and the row actions are counted");
    }

    /// FAILS IF: a page's action under the cursor is kept by its index in
    /// `actions` — the list the module's switch reshapes, what only a module
    /// switched off offers leaving it while it is on. Switched off from
    /// "mark all seen", the cursor would land on "run", and `RET` would read
    /// the journal instead; switched back on, on a row's title.
    #[test]
    fn a_page_action_is_kept_by_its_id_when_the_switch_reshapes_the_list() {
        let action = |id: &str, row: bool| ronoctua_design::types::ModuleAction {
            id: id.to_owned(),
            label_key: format!("cmd-fixture-{id}"),
            row,
            asks_key: String::new(),
        };
        // THE LIST AS THE MIND BRINGS IT, sorted by id, `run` only while off.
        let actions = |off: bool| {
            [
                ("filter", false),
                ("filter-line", true),
                ("follow", false),
                ("ignore", true),
                ("reason", true),
                ("refilter", true),
                ("run", false),
                ("seen", true),
                ("seen-all", false),
                ("unfilter", true),
                ("unignore", true),
            ]
            .into_iter()
            .filter(|(id, _)| off || *id != "run")
            .map(|(id, row)| action(id, row))
            .collect::<Vec<_>>()
        };
        let switched = |s: &mut State, off: bool| {
            let mut mind = s.mind.clone();
            mind.tasks[0].actions = actions(off);
            s.act(Input::Mind(mind));
        };
        let on = |s: &State| match s.spot_here() {
            Some(Spot::Action(n)) => Some(s.mind.tasks[0].actions[n].id.clone()),
            _ => None,
        };
        let mut s = a_state();
        s.mind.tasks[0].actions = actions(false);
        s.page = Page::Task;
        let at = |s: &State, id: &str| {
            s.mind.tasks[0]
                .actions
                .iter()
                .position(|a| a.id == id)
                .expect("the fixture")
        };
        let wanted = at(&s, "seen-all");
        on_spot(&mut s, Spot::Action(wanted));
        switched(&mut s, true);
        assert_eq!(on(&s).as_deref(), Some("seen-all"), "switched off");
        switched(&mut s, false);
        assert_eq!(on(&s).as_deref(), Some("seen-all"), "switched on again");

        // GONE WITH THE SWITCH, the cursor stays among the page's actions.
        switched(&mut s, true);
        let wanted = at(&s, "run");
        on_spot(&mut s, Spot::Action(wanted));
        switched(&mut s, false);
        assert_eq!(on(&s).as_deref(), Some("seen-all"), "the one at its place");
    }

    /// FAILS IF: the legend is not above the first section with a blank
    /// between, is a spot, lights under the pointer, draws a word in any
    /// colour but the one it explains, cuts a swatch that does not fit, or
    /// says nothing of what the pointer rests on — or a page with no legend
    /// spends a line or a blank on it.
    #[test]
    fn the_legend_explains_its_colours_and_a_swatch_goes_whole() {
        use ronoctua_design::frame::{Command, Text};
        let mut s = a_state();
        let bare = a_folding_page(2);
        on_the_page(&mut s, bare.clone());
        let task = s.mind.tasks[0].clone();
        let without = s.rows_of(&task, 100).len();
        let mut page = a_folding_page(3);
        page.legend = a_legend();
        let page = with_words(
            page,
            &[
                ("fixture-worst", "Worst"),
                ("fixture-least", "The least of them"),
            ],
        );
        s.act(Input::ModuleScreen(page));
        let rows = s.rows_of(&task, 100);
        assert_eq!(rows.len(), without + 2, "a legend takes a line and a blank");
        let at = rows
            .iter()
            .position(|r| matches!(r, Row::Legend))
            .expect("the legend");
        assert!(matches!(rows[at - 1], Row::Blank));
        assert!(matches!(rows[at + 1], Row::Blank));
        assert!(matches!(rows[at + 2], Row::Fold(0)), "not above the first");
        assert!(!is_a_spot(&Row::Legend));

        let screen = drawn(&mut s, 70, 100);
        let seen = runs(&screen);
        let place = |seen: &[(String, String, u16, u16)], what: &str| {
            seen.iter()
                .find(|(t, ..)| t == what)
                .map(|(_, c, line, column)| (c.clone(), *line, *column))
        };
        let (colour, line, column) = place(&seen, "fixture-worst").expect("the word");
        assert_eq!(colour, "state-critical");
        assert_eq!(column, ronoctua_design::ICON);
        let glyphs: Vec<_> = seen
            .iter()
            .filter(|(t, _, l, _)| *l == line && t.contains("BrainSwatch"))
            .collect();
        assert_eq!(glyphs.len(), 2, "{seen:?}");
        assert_eq!(glyphs[0].1, "state-critical");
        assert_eq!(glyphs[1].1, "text-secondary");
        let (_, _, next) = place(&seen, "fixture-least").expect("the second word");
        assert_eq!(glyphs[1].3, column + 5 + FACT_SPACING);
        assert_eq!(next, glyphs[1].3 + ronoctua_design::ICON);

        // WHOLE OR NOT AT ALL, at every width: what is drawn has room for
        // all of its word, and nothing of a swatch that had none is left.
        let budget = |screen: &Screen<Wit>, what: &str| {
            screen.as_protocol().commands.iter().find_map(|c| match c {
                Command::Text {
                    text: Text::Key(k),
                    max_width,
                    ..
                } if k == what => Some(*max_width),
                _ => None,
            })
        };
        let mut gone = 0;
        for columns in 8..=60 {
            let screen = drawn(&mut s, 70, columns);
            let seen = runs(&screen);
            let glyphs = seen
                .iter()
                .filter(|(t, ..)| t.contains("BrainSwatch"))
                .count();
            let worst = budget(&screen, "fixture-worst");
            let least = budget(&screen, "fixture-least");
            assert_eq!(
                glyphs,
                usize::from(worst.is_some()) + usize::from(least.is_some())
            );
            if let Some(cells) = worst {
                assert_eq!(cells, 5, "at {columns}");
            }
            if let Some(cells) = least {
                assert_eq!(cells, 17, "at {columns}");
            } else {
                gone += 1;
            }
        }
        floor_of(gone, 10, "widths too narrow for the second swatch");

        // NO GROUND UNDER THE POINTER, AND ITS OWN SENTENCE PER SWATCH.
        let pointed = |s: &mut State, column| {
            let _ = s.hover.set(0, (column, line));
            let screen = drawn(s, 70, 100);
            let lit = screen
                .as_protocol()
                .commands
                .iter()
                .any(|c| matches!(c, Command::Selection { line: l, .. } if *l == line));
            assert!(!lit, "the legend lit under the pointer");
            screen
                .as_protocol()
                .commands
                .into_iter()
                .find_map(|c| match c {
                    Command::Pointed { about, .. } => Some(about),
                    _ => None,
                })
        };
        assert_eq!(
            pointed(&mut s, 0),
            Some(Text::Key("fixture-worst-about".into()))
        );
        assert_eq!(
            pointed(&mut s, next),
            Some(Text::Key("fixture-least-about".into()))
        );
        // AND THE AIR BETWEEN THEM IS NOTHING.
        assert_eq!(pointed(&mut s, next - 3), None);
        // A CLICK TAKES NOTHING: it is not a row anything happens to.
        let before = s.spot;
        assert!(s.clicked(0, line).is_empty());
        assert_eq!(s.spot, before);
    }

    /// FAILS IF: opening a task's page does not ask the host to follow its
    /// module's page, leaving it does not stop it, a page for another module
    /// is drawn, or `RET` on a row asks for anything but that row's action.
    #[test]
    fn a_task_page_follows_its_module_screen() {
        let mut s = a_state();
        let asks = |out: &[Effect]| -> Vec<(Request, u64)> {
            out.iter()
                .filter_map(|e| match e {
                    Effect::Ask(d) => Some((d.what, d.target)),
                    _ => None,
                })
                .collect()
        };
        let opened = {
            let mut out = s.act(Input::Confirm);
            out.extend(s.watch_asks());
            out
        };
        assert_eq!(asks(&opened), vec![(Request::WatchScreen, 1)]);
        assert!(s.watch_asks().is_empty(), "the ask went out twice");

        // ANOTHER MODULE'S PAGE IS REFUSED.
        assert!(
            s.act(Input::ModuleScreen(a_screen("fixture-once", 1)))
                .is_empty()
        );
        assert!(s.screen.is_none());

        s.act(Input::ModuleScreen(a_screen("fixture-routine", 2)));
        let rows_at = s
            .spots()
            .iter()
            .position(|sp| *sp == Spot::Screen(0))
            .expect("the module's rows are not on the page");
        // AN OLDER GENERATION does not replace a newer one.
        s.act(Input::ModuleScreen(a_screen("fixture-routine", 1)));
        assert_eq!(s.screen.as_ref().map(|sc| sc.generation), Some(2));
        let (keys, _) = drawn_texts(&drawn(&mut s, 40, 120));
        for k in [
            "fixture-heading-one",
            "fixture-heading-two",
            "fixture-row-a",
        ] {
            assert!(keys.iter().any(|d| d == k), "{k} is not drawn");
        }

        s.spot = rows_at;
        assert_eq!(asks(&s.confirm()), vec![(Request::ActRow, 1)]);
        // ITS SECOND ACTION, by its own key, taught while the row has one.
        assert_eq!(
            asks(&s.command("brain-row-also")),
            vec![(Request::ActRowAlso, 1)]
        );
        assert!(
            s.teaches().iter().any(|(k, _)| *k == ", v"),
            "a row's second action is not taught"
        );
        // A ROW WITH NO ACTION only tells.
        s.spot = rows_at + 1;
        assert!(s.confirm().is_empty(), "a row with no action asked for one");
        assert!(
            s.command("brain-row-also").is_empty(),
            "a row with no second action asked for one"
        );
        assert!(
            !s.teaches().iter().any(|(k, _)| *k == ", v"),
            "a second action taught on a row without one"
        );
        // THE FOOTER HOLDS THE ROW WHOLE: its label and its note.
        let told = s.told();
        assert_eq!(told.name.as_deref(), Some("fixture-row-b"));
        assert_eq!(
            told.logged.map(|(t, _)| t).as_deref(),
            Some("a note the module wrote")
        );

        // LEAVING STOPS IT, and the page goes with the watch.
        let left = {
            let mut out = s.act(Input::Cancel);
            out.extend(s.watch_asks());
            out
        };
        assert_eq!(asks(&left), vec![(Request::WatchScreen, 0)]);
        assert!(s.screen.is_none(), "a page outlived its watch");
    }

    /// FAILS IF: a row whose label is a moment and whose value is DATA the
    /// row had to cut (a journal's error) leaves the value out of the footer
    /// — a long message could then only be read whole in the journal.
    #[test]
    fn the_footer_holds_a_row_s_value_whole() {
        let mut s = a_state();
        let _ = s.act(Input::Confirm);
        let _ = s.watch_asks();
        let message = "nginx.service: bind() to 0.0.0.0:443 failed (98: Address in use) \
                       and the rest of a message far longer than a row"
            .to_owned();
        let mut screen = a_screen("fixture-routine", 2);
        screen.sections[0].rows[0] = ModuleRow {
            id: "kind".to_owned(),
            label: ScreenCell::Moment(ronoctua_design::types::Timed {
                at: 5,
                text: "14:02".to_owned(),
                stamp: ronoctua_design::types::Stamp {
                    lead: String::new(),
                    day: String::new(),
                    tail: "14:02".to_owned(),
                    today: false,
                },
            }),
            value: ScreenCell::Data(message.clone()),
            note: ScreenCell::Count(3),
            tone: BrainTone::Watching,
            act: "seen".to_owned(),
            also: "ignore".to_owned(),
            below: ScreenCell::Empty,
            foot: Vec::new(),
        };
        s.act(Input::ModuleScreen(screen));
        s.spot = s
            .spots()
            .iter()
            .position(|sp| *sp == Spot::Screen(0))
            .expect("the row");
        let told = s.told();
        assert_eq!(told.logged.map(|(t, _)| t), Some(message));
        assert_eq!(told.said.as_deref(), Some("14:02  3"));
    }

    /// FAILS IF: a module's page action is not a row `RET` asks for, by its
    /// index in the task's actions — or an action that acts on a ROW is
    /// offered as one of the page's.
    #[test]
    fn a_page_action_is_asked_for_by_its_index() {
        let mut s = a_state();
        s.mind.tasks[0].actions = vec![
            ronoctua_design::types::ModuleAction {
                id: "forget".to_owned(),
                label_key: "cmd-fixture-forget".to_owned(),
                row: true,
                asks_key: String::new(),
            },
            ronoctua_design::types::ModuleAction {
                id: "new".to_owned(),
                label_key: "cmd-fixture-new".to_owned(),
                row: false,
                asks_key: "ask-fixture-new".to_owned(),
            },
        ];
        s.page = Page::Task;
        let spots = s.spots();
        assert!(
            !spots.contains(&Spot::Action(0)),
            "a row's action is on the page"
        );
        s.spot = spots
            .iter()
            .position(|sp| *sp == Spot::Action(1))
            .expect("the page action is not on the page");
        let out = s.confirm();
        assert!(
            matches!(
                out.as_slice(),
                [Effect::Ask(Demand {
                    what: Request::ActPage,
                    target: 2
                })]
            ),
            "{out:?}"
        );
        // AND THE FOOTER SAYS WHAT IT WILL ASK.
        assert_eq!(s.told().about.as_deref(), Some("ask-fixture-new"));
    }

    /// FAILS IF: the three notice keys ask for anything but the notice in
    /// front, by its 1-based index, or ask for what the notice does not offer
    /// — or the header teaches a key the notice does not answer.
    #[test]
    fn the_notice_keys_act_on_the_notice_in_front() {
        let mut s = a_state();
        s.mind.notices.push(a_notice("fixture-once"));
        let ask = |s: &mut State, id: &str| -> Option<(Request, u64)> {
            match s.command(id).as_slice() {
                [Effect::Ask(d)] => Some((d.what, d.target)),
                [] => None,
                other => panic!("{id}: {other:?}"),
            }
        };
        assert_eq!(ask(&mut s, "brain-act"), Some((Request::ActNotice, 1)));
        assert_eq!(
            ask(&mut s, "brain-dismiss"),
            Some((Request::DismissNotice, 1))
        );
        assert_eq!(
            ask(&mut s, "brain-snooze"),
            Some((Request::SnoozeNotice, 1))
        );
        // THE ONE UNDER THE CURSOR, on the home page.
        s.pane = Pane::Notices;
        s.notice = 1;
        assert_eq!(ask(&mut s, "brain-act"), Some((Request::ActNotice, 2)));
        // AND ON A TASK'S PAGE, the first of that task's module.
        s.pane = Pane::Tasks;
        s.page = Page::Task;
        s.task = 1;
        assert_eq!(
            ask(&mut s, "brain-dismiss"),
            Some((Request::DismissNotice, 2))
        );

        // WHAT IS NOT OFFERED IS NOT ASKED FOR, AND NOT TAUGHT.
        let mut s = a_state();
        s.mind.notices[0].controls.clear();
        s.mind.notices[0].dismissable = false;
        s.mind.notices[0].snooze.clear();
        for id in ["brain-act", "brain-dismiss", "brain-snooze"] {
            assert_eq!(ask(&mut s, id), None, "{id} asked for what is not offered");
        }
        assert!(
            s.teaches()
                .iter()
                .all(|(k, _)| !NOTICE_TEACHES.iter().any(|(n, _)| n == k)),
            "the header teaches a notice key nothing answers"
        );
        let full = a_state();
        for (keys, _) in NOTICE_TEACHES {
            assert!(
                full.teaches().iter().any(|(k, _)| k == keys),
                "{keys} is offered and not taught"
            );
        }
    }

    /// FAILS IF: the notices are not drawn on the home page, or a click on
    /// one does not select it and a second open its module's page.
    #[test]
    fn a_notice_is_drawn_and_answers_the_pointer() {
        let mut s = a_state();
        let (_, data) = drawn_texts(&drawn(&mut s, 30, 120));
        assert!(
            data.iter().any(|d| d == &s.mind.notices[0].says_text),
            "the notice's sentence is not drawn"
        );
        assert_eq!(s.blocks.notices.lines, 2, "a title and one notice");
        let line = s.blocks.notices.top + 1;
        s.clicked(4, line);
        assert_eq!((s.pane, s.notice), (Pane::Notices, 0));
        s.clicked(4, line);
        assert_eq!(s.page, Page::Task, "the second click did not open its page");
        assert_eq!(s.here().map(|t| t.id.as_str()), Some("fixture-routine"));
        // AND NOTHING SAID, NO BLOCK.
        let mut quiet = a_state();
        quiet.mind.notices.clear();
        drawn(&mut quiet, 30, 120);
        assert_eq!(quiet.blocks.notices.lines, 0, "a block for nothing");
    }

    /// A notice of `level`, raised at `since`.
    fn a_notice_at(id: &str, level: NoticeLevel, since: u64) -> Notice {
        Notice {
            id: id.to_owned(),
            level,
            since,
            ..a_notice("fixture-once")
        }
    }

    /// FAILS IF: a notice raised ABOVE the one under the cursor — the host
    /// ranks them, the newest first among equals — moves the three notice
    /// keys onto a different notice; or a notice put away leaves the cursor
    /// past the end of the list.
    #[test]
    fn the_notice_cursor_follows_its_notice_when_the_list_is_ranked_again() {
        let mut s = a_state();
        let mut mind = a_mind();
        mind.notices = vec![
            a_notice_at("a", NoticeLevel::Info, 100),
            a_notice_at("b", NoticeLevel::Info, 50),
        ];
        s.received(mind.clone());
        s.pane = Pane::Notices;
        s.notice = 1;
        // A NEWER ONE, RANKED FIRST: [c, a, b].
        mind.notices
            .insert(0, a_notice_at("c", NoticeLevel::Info, 200));
        s.received(mind.clone());
        assert_eq!(
            s.mind.notices.get(s.notice).map(|n| n.id.as_str()),
            Some("b")
        );
        match s.command("brain-dismiss").as_slice() {
            [Effect::Ask(d)] => assert_eq!(
                (d.what, d.target),
                (Request::DismissNotice, 3),
                "`, x` asked for another notice than the one under the cursor"
            ),
            other => panic!("{other:?}"),
        }
        // THE ONE UNDER THE CURSOR GOES: the cursor stays at its row, inside
        // the list.
        mind.notices.remove(2);
        s.received(mind);
        assert_eq!(s.notice, 1);
    }

    /// FAILS IF: the task under the cursor, or the line of the log, changes
    /// when a task is listed above it or a newer record arrives at the top of
    /// the log — the same rule as the notices', for the other two lists.
    #[test]
    fn the_task_and_the_log_cursors_follow_their_items() {
        let mut s = a_state();
        s.task = 1;
        let mut mind = a_mind();
        mind.tasks.insert(0, a_task("fixture-new", TaskKind::Once));
        s.received(mind.clone());
        assert_eq!(s.here().map(|t| t.id.as_str()), Some("fixture-once"));

        let at = |task: &str, outcome, began| RunRecord {
            began,
            ..a_run(task, outcome)
        };
        let older = [
            at("fixture-routine", OutcomeKind::Done, 900),
            at("fixture-once", OutcomeKind::Held, 800),
            at("fixture-once", OutcomeKind::Held, 700),
        ];
        mind.activity_rows = folded(&older);
        s.received(mind.clone());
        s.pane = Pane::Log;
        s.log = 1;
        let mut newer = vec![at("fixture-new", OutcomeKind::Done, 1_000)];
        newer.extend(older);
        mind.activity_rows = folded(&newer);
        s.received(mind);
        assert_eq!(
            s.log, 2,
            "the log's cursor stayed on the row, not on its fold"
        );
    }

    /// FAILS IF: the page opened with news waiting does not put the keyboard
    /// on the notices — what the strip's `+N` opens it for — or opened with
    /// nothing to say does not leave it on the tasks.
    #[test]
    fn the_page_opens_where_the_news_is() {
        let mut s = State::fresh();
        s.received(a_mind());
        assert_eq!((s.pane, s.notice), (Pane::Notices, 0));
        // ONLY THE FIRST MIND DECIDES: the person's `Tab` is not undone by
        // the next one.
        s.pane = Pane::Tasks;
        s.received(a_mind());
        assert_eq!(s.pane, Pane::Tasks);

        let mut quiet = State::fresh();
        let mut mind = a_mind();
        mind.notices.clear();
        quiet.received(mind);
        assert_eq!(quiet.pane, Pane::Tasks);
    }

    /// FAILS IF: a module's page the brain REFUSED draws nothing where its
    /// rows would be — a refused page and an empty one look the same — or a
    /// newer page does not take its place.
    #[test]
    fn a_refused_page_says_why() {
        let mut s = a_state();
        let _ = s.act(Input::Confirm);
        let _ = s.watch_asks();
        let mut refused = a_screen("fixture-routine", 3);
        refused.sections.clear();
        refused.refused = "The brain would not: no page today".to_owned();
        s.act(Input::ModuleScreen(refused));
        let paragraphs = |screen: &Screen<Wit>| -> Vec<String> {
            screen
                .as_protocol()
                .commands
                .into_iter()
                .filter_map(|c| match c {
                    ronoctua_design::frame::Command::Paragraph {
                        content: ronoctua_design::frame::Text::Data(d),
                        ..
                    } => Some(d),
                    _ => None,
                })
                .collect()
        };
        let said = paragraphs(&drawn(&mut s, 40, 120));
        assert!(
            said.iter()
                .any(|d| d == "The brain would not: no page today"),
            "the refusal is not on the page: {said:?}"
        );
        assert!(
            !s.spots().iter().any(|sp| matches!(sp, Spot::Screen(_))),
            "a refused page left rows to walk"
        );
        // THE PAGE, when it comes, takes the sentence's place.
        s.act(Input::ModuleScreen(a_screen("fixture-routine", 4)));
        let said = paragraphs(&drawn(&mut s, 40, 120));
        assert!(said.iter().all(|d| !d.contains("no page today")));
        assert!(s.spots().iter().any(|sp| matches!(sp, Spot::Screen(_))));
    }

    /// A fact, as a module says it.
    fn a_fact(label: &str, value: ScreenCell, tone: BrainTone) -> RowFact {
        RowFact {
            label: label.to_owned(),
            value,
            tone,
        }
    }

    /// A reason long enough to wrap, a filter's and a kind's row, and a row
    /// that only says something under itself.
    const A_REASON: &str = "Two service files claim one name, the first one read wins, nothing \
                            is lost and nobody has to do anything at all about it, ever.";

    fn a_filter_row(id: &str, below: ScreenCell) -> ModuleRow {
        ModuleRow {
            id: id.to_owned(),
            label: ScreenCell::Data("a snippet".to_owned()),
            value: ScreenCell::Data("the last line".to_owned()),
            note: ScreenCell::Count(123),
            tone: BrainTone::Quiet,
            act: "unfilter".to_owned(),
            also: "reason".to_owned(),
            below,
            foot: vec![
                a_fact("fact-boot", ScreenCell::Count(123), BrainTone::Working),
                a_fact(
                    "fact-last",
                    ScreenCell::Data("the last line, whole".to_owned()),
                    BrainTone::Quiet,
                ),
            ],
        }
    }

    /// A page of `rows`, whose words the host said, on a task whose module
    /// offers the one action a filter's row names beside its press.
    fn on_a_page_of(s: &mut State, rows: Vec<ModuleRow>) {
        s.task = 0;
        s.mind.tasks[0].actions = vec![ronoctua_design::types::ModuleAction {
            id: "reason".to_owned(),
            label_key: "fixture-say-why".to_owned(),
            row: true,
            asks_key: String::new(),
        }];
        let mut page = a_screen("fixture-routine", 7);
        page.sections.truncate(1);
        page.sections[0].rows = rows;
        let page = with_words(
            page,
            &[
                ("fact-boot", "This boot"),
                ("fact-last", "Last line caught"),
                ("fixture-say-why", "Say why this filter exists"),
                ("reason-key", A_REASON),
            ],
        );
        on_the_page(s, page);
    }

    /// The line a row of the module's page is drawn on, in the frame last
    /// drawn, by its index across the sections.
    fn line_of_row(s: &State, n: usize) -> u16 {
        let rows = s.page_rows();
        let at = rows
            .iter()
            .position(|r| matches!(r, Row::Screen(k, _) if *k == n))
            .expect("the row is on the page");
        s.blocks.body.top + lines_before(&rows, at) - lines_before(&rows, s.body_from)
    }

    /// FAILS IF: what a row says under itself takes no line of the page, or
    /// more than it wraps to, or past the most it is given — or a key the host
    /// said is counted as anything but its words, or one row with nothing
    /// under it takes more than a line.
    #[test]
    fn a_row_is_as_tall_as_what_it_says_under_itself() {
        let mut s = a_state();
        let _ = drawn(&mut s, 40, 73);
        on_a_page_of(
            &mut s,
            vec![
                a_filter_row("data", ScreenCell::Data(A_REASON.to_owned())),
                a_filter_row("key", ScreenCell::Key("reason-key".to_owned())),
                a_filter_row("none", ScreenCell::Empty),
                a_filter_row("blind", ScreenCell::Key("fixture-unsaid".to_owned())),
                a_filter_row("long", ScreenCell::Data(A_REASON.repeat(4))),
            ],
        );
        let wraps = ronoctua_design::wrap(A_REASON, below_cells(73), 9).len();
        assert!(wraps > 1, "the reason fits a line; this proves nothing");
        let wraps = u16::try_from(wraps).expect("a few");
        let tall: Vec<u16> = s
            .page_rows()
            .iter()
            .filter_map(|r| match r {
                Row::Screen(_, tall) => Some(*tall),
                _ => None,
            })
            .collect();
        assert_eq!(tall, [1 + wraps, 1 + wraps, 1, 2, 1 + BELOW_MOST]);
    }

    /// FAILS IF: the band or the pointer's ground stops at a row's first
    /// line, the paragraph under it is not the row's, in its own tone, clear
    /// of the page's bar; a click under the row is not a click on it, by the
    /// rule of every row; or the pointer there says anything but what is
    /// written there.
    #[test]
    fn the_lines_under_a_row_are_the_row() {
        use ronoctua_design::frame::Command;
        let mut s = a_state();
        let _ = drawn(&mut s, 40, 73);
        on_a_page_of(
            &mut s,
            vec![
                a_filter_row("under", ScreenCell::Data(A_REASON.to_owned())),
                a_filter_row("next", ScreenCell::Empty),
            ],
        );
        let screen = drawn(&mut s, 40, 73);
        let first = line_of_row(&s, 0);
        let last = line_of_row(&s, 1) - 1;
        assert!(last > first, "the row is one line; this proves nothing");
        let under = screen
            .as_protocol()
            .commands
            .into_iter()
            .find_map(|c| match c {
                Command::Paragraph {
                    line,
                    lines,
                    column,
                    columns,
                    color,
                    content: ronoctua_design::frame::Text::Data(d),
                    ..
                } if d == A_REASON => Some((line, lines, column, columns, color)),
                _ => None,
            })
            .expect("what the row says under itself is not on the page");
        assert_eq!(under.0, first + 1);
        assert_eq!(under.1, last - first);
        assert_eq!(under.2, BELOW_AT);
        assert_eq!(under.2 + under.3, 72, "it runs under the page's bar");
        assert_eq!(under.4.as_str(), "text-folded");

        // THE CLICK SELECTS, on any of its lines, and the second acts.
        on_spot(&mut s, Spot::Enabled);
        let _ = s.clicked(2, last);
        assert_eq!(s.spot_here(), Some(Spot::Screen(0)));
        let out = s.clicked(2, last);
        assert!(
            out.iter()
                .any(|e| matches!(e, Effect::Ask(d) if d.what == Request::ActRow && d.target == 1)),
            "the second click on what a row says under itself: {out:?}"
        );
        // THE BAND COVERS EVERY LINE, and the pointer's ground the same.
        let grounded = |screen: &Screen<Wit>, line: u16| {
            screen
                .as_protocol()
                .commands
                .iter()
                .any(|c| matches!(c, Command::Selection { line: l, .. } if *l == line))
        };
        let screen = drawn(&mut s, 40, 73);
        assert!((first..=last).all(|line| grounded(&screen, line)));
        assert!(!grounded(&screen, last + 2), "the next row is not this one");
        on_spot(&mut s, Spot::Enabled);
        let _ = s.hover.set(0, (30, last));
        let screen = drawn(&mut s, 40, 73);
        assert!((first..=last).all(|line| grounded(&screen, line)));
        assert_eq!(s.pointed(), Some(Pointed::Datum(A_REASON.to_owned())));
        // AND ON THE ROW ITSELF, what the row has to say.
        let _ = s.hover.set(0, (30, first));
        let _ = drawn(&mut s, 40, 73);
        assert_eq!(
            s.pointed(),
            Some(Pointed::Datum("the last line".to_owned()))
        );
    }

    /// FAILS IF: a row is drawn with half of what it says under itself at the
    /// window's edge — a bar over a cut sentence for a row merely passing by —
    /// or the cursor reaching it does not bring all of it in.
    #[test]
    fn a_row_at_the_windows_edge_goes_whole() {
        use ronoctua_design::frame::{Command, Text};
        let mut s = a_state();
        let _ = drawn(&mut s, 30, 73);
        on_a_page_of(
            &mut s,
            (0..12)
                .map(|n| a_filter_row(&format!("row-{n}"), ScreenCell::Data(A_REASON.to_owned())))
                .collect(),
        );
        for spot in 0..s.spots().len() {
            s.spot = spot;
            s.follows = true;
            let screen = drawn(&mut s, 30, 73);
            let foot = s.blocks.foot.top;
            let cut = screen.as_protocol().commands.iter().any(|c| {
                matches!(c, Command::Paragraph { line, lines, content: Text::Data(d), .. }
                    if d == A_REASON && *line < foot && usize::from(*lines) < 2)
            });
            assert!(!cut, "a row's lines were cut by the window at spot {spot}");
            if let Some(Spot::Screen(n)) = s.spot_here() {
                let rows = s.page_rows();
                let at = rows
                    .iter()
                    .position(|r| matches!(r, Row::Screen(k, _) if *k == n))
                    .expect("the row");
                assert!(
                    at < s.body_from + fit_from(&rows, s.body_from, s.blocks.body.lines),
                    "the cursor's row is not whole in the window"
                );
            }
        }
    }

    /// FAILS IF: under a row the window cut to its first line, the pointer
    /// lights a line, says what is not drawn there, or a click there takes the
    /// row — or the row's own line stops answering.
    #[test]
    fn under_a_row_the_window_cut_nothing_answers() {
        use ronoctua_design::frame::Command;
        let mut s = a_state();
        let _ = drawn(&mut s, 30, 73);
        on_a_page_of(
            &mut s,
            (0..12)
                .map(|n| a_filter_row(&format!("row-{n}"), ScreenCell::Data(A_REASON.to_owned())))
                .collect(),
        );
        // THE WHEEL'S WINDOW, which stops wherever it is left: the first one
        // that ends two lines or more into a row.
        s.follows = false;
        let cut = (0..s.page_rows().len())
            .find_map(|from| {
                s.body_from = from;
                let _ = drawn(&mut s, 30, 73);
                s.page_cut
                    .filter(|cut| s.body_from == from && *cut < s.blocks.body.lines)
            })
            .expect("no window cuts a row with lines under it; this proves nothing");
        let line = s.blocks.body.top + cut;
        let spot = s.spot;
        let lit = |screen: &Screen<Wit>, at: u16| {
            screen
                .as_protocol()
                .commands
                .iter()
                .any(|c| matches!(c, Command::Selection { line: l, .. } if *l == at))
        };
        let _ = s.hover.set(0, (30, line));
        let screen = drawn(&mut s, 30, 73);
        assert!(!lit(&screen, line), "the pointer lit a line nothing is on");
        assert_eq!(s.pointed(), None, "the pointer said what is not drawn");
        assert!(
            s.clicked(30, line).is_empty(),
            "a click under it took the row"
        );
        assert_eq!(s.spot, spot);
        // THE ROW'S OWN LINE answers as every row does.
        let _ = s.hover.set(0, (30, line - 1));
        let screen = drawn(&mut s, 30, 73);
        assert!(lit(&screen, line - 1));
        assert!(s.pointed().is_some());
        let _ = s.clicked(30, line - 1);
        assert!(matches!(s.spot_here(), Some(Spot::Screen(_))));
    }

    /// FAILS IF: a window shorter than the cursor's row walks past it: the
    /// cursor on nothing drawn, or the frame refused for a line out of it.
    #[test]
    fn a_row_taller_than_the_window_stays_under_the_cursor() {
        let mut s = a_state();
        let _ = drawn(&mut s, 40, 73);
        on_a_page_of(
            &mut s,
            vec![
                a_filter_row("tall", ScreenCell::Data(A_REASON.repeat(3))),
                a_filter_row("next", ScreenCell::Empty),
            ],
        );
        on_spot(&mut s, Spot::Screen(0));
        s.follows = true;
        let screen = drawn(&mut s, 10, 73);
        screen.check().expect("the frame refused something");
        let rows = s.page_rows();
        assert!(
            matches!(rows[s.body_from], Row::Screen(0, _)),
            "the window went past the row under the cursor"
        );
    }

    /// FAILS IF: a line too long for what the page gives it cannot be moved
    /// by its own bar, or its bar moves the page or another row's.
    #[test]
    fn what_a_row_says_past_its_lines_has_a_bar_of_its_own() {
        use ronoctua_design::frame::{Command, Text};
        let long = A_REASON.repeat(4);
        let mut s = a_state();
        let _ = drawn(&mut s, 40, 73);
        on_a_page_of(
            &mut s,
            vec![
                a_filter_row("long", ScreenCell::Data(long.clone())),
                a_filter_row("next", ScreenCell::Data(long.clone())),
            ],
        );
        let firsts = |s: &mut State| -> Vec<u16> {
            let foot = {
                let _ = drawn(s, 40, 73);
                s.blocks.foot.top
            };
            drawn(s, 40, 73)
                .as_protocol()
                .commands
                .iter()
                .filter_map(|c| match c {
                    Command::Paragraph {
                        line,
                        first_row,
                        content: Text::Data(_),
                        ..
                    } if *line < foot => Some(*first_row),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(firsts(&mut s), [0, 0]);
        let from = s.body_from;
        let line = line_of_row(&s, 0) + 1;
        let out = s.act(Input::Scrolled(ronoctua_design::types::ScrollEvent {
            at: ronoctua_design::types::Position { column: 71, line },
            first: 2,
        }));
        assert!(!out.is_empty(), "a drag on its bar did nothing");
        assert_eq!(s.body_from, from, "its bar moved the page");
        assert_eq!(firsts(&mut s), [2, 0]);
    }

    /// FAILS IF: in the footer, what a row says under itself and its last
    /// fact are moved by one bar — a drag or a wheel on either scrolls both.
    #[test]
    fn the_footers_two_paragraphs_have_a_bar_each() {
        use ronoctua_design::frame::{Command, Text};
        let long = A_REASON.repeat(4);
        let mut s = a_state();
        let _ = drawn(&mut s, 40, 73);
        let mut row = a_filter_row("long", ScreenCell::Data(long.clone()));
        row.foot[1].value = ScreenCell::Data(long);
        on_a_page_of(&mut s, vec![row]);
        on_spot(&mut s, Spot::Screen(0));
        // Each paragraph of the footer: its first line, the cell the host
        // answers its bar from, and the row it was left at.
        let paragraphs = |s: &mut State| -> Vec<(u16, u16, u16)> {
            let screen = drawn(s, 40, 73);
            let foot = s.blocks.foot.top;
            screen
                .as_protocol()
                .commands
                .iter()
                .filter_map(|c| match c {
                    Command::Paragraph {
                        line,
                        column,
                        columns,
                        first_row,
                        content: Text::Data(_),
                        ..
                    } if *line >= foot => Some((*line, column + columns - 1, *first_row)),
                    _ => None,
                })
                .collect()
        };
        let drawn_first = paragraphs(&mut s);
        assert_eq!(drawn_first.len(), 2, "the footer: {drawn_first:?}");
        for (n, (line, column, _)) in drawn_first.iter().enumerate() {
            let _ = s.act(Input::Scrolled(ronoctua_design::types::ScrollEvent {
                at: ronoctua_design::types::Position {
                    column: *column,
                    line: *line,
                },
                first: 1 + u32::try_from(n).expect("two"),
            }));
        }
        let left_at: Vec<u16> = paragraphs(&mut s).iter().map(|p| p.2).collect();
        assert_eq!(left_at, [1, 2]);
    }

    /// FAILS IF: a row with nothing more to say stops being told by its
    /// cells, which is every other module's footer — or a row that says more
    /// keeps a sentence, or its name is a date.
    #[test]
    fn a_row_is_told_by_what_its_module_says_of_it() {
        let mut s = a_state();
        let _ = drawn(&mut s, 40, 100);
        let mut plain = a_dated_row("plain", "a line of the journal", 3);
        let filter = a_filter_row("filter", ScreenCell::Key("reason-key".to_owned()));
        let mut kind = a_dated_row("kind", "a line of the journal", 3);
        kind.foot = filter.foot.clone();
        plain.tone = BrainTone::Quiet;
        let mut key = a_filter_row("key", ScreenCell::Empty);
        key.label = ScreenCell::Key("fixture-row".to_owned());
        let mut cells = a_dated_row("cells", "a line of the journal", 3);
        cells.below = ScreenCell::Data("under it".to_owned());
        on_a_page_of(
            &mut s,
            vec![plain.clone(), filter, kind, key, cells.clone()],
        );

        on_spot(&mut s, Spot::Screen(0));
        let told = s.told();
        assert_eq!(told.subject(), State::told_of_row(&plain).subject());
        assert!(told.headline.is_none() && told.lines.is_empty() && told.below.is_none());

        on_spot(&mut s, Spot::Screen(1));
        let told = s.told();
        assert_eq!(told.headline.as_deref(), Some("a snippet"));
        assert_eq!(
            told.below.as_ref().map(cell_word).as_deref(),
            Some("reason-key")
        );
        assert_eq!(told.lines.len(), 2);
        assert_eq!(told.also.as_deref(), Some("fixture-say-why"));
        assert!(told.said.is_none() && told.logged.is_none() && told.about.is_none());

        // A DATE IN THE LABEL'S PLACE NAMES NOTHING: its line does.
        on_spot(&mut s, Spot::Screen(2));
        assert_eq!(s.told().headline.as_deref(), Some("a line of the journal"));
        // A KEY IS THE NAME EVERY FOOTER HAS.
        on_spot(&mut s, Spot::Screen(3));
        let told = s.told();
        assert_eq!(told.name.as_deref(), Some("fixture-row"));
        assert!(told.headline.is_none());
        // WITH NO FACTS, what its cells say stays, and the line under it joins.
        on_spot(&mut s, Spot::Screen(4));
        let told = s.told();
        assert_eq!(
            told.below.as_ref().map(cell_word).as_deref(),
            Some("under it")
        );
        assert_eq!(told.said, State::told_of_row(&cells).said);
        assert!(told.logged.is_some());
    }

    /// FAILS IF: the footer's facts lose their tones to one grey — the owner's
    /// *"tudo está na mesma cor"* —, the labels stop sharing a column as wide
    /// as the widest, the last fact stops taking the rows left, or the words
    /// under the name take a row a fact needed.
    #[test]
    fn the_footer_says_a_row_in_its_colours() {
        use ronoctua_design::frame::{Command, Text};
        let mut s = a_state();
        let _ = drawn(&mut s, 40, 100);
        on_a_page_of(
            &mut s,
            vec![a_filter_row("f", ScreenCell::Key("reason-key".to_owned()))],
        );
        on_spot(&mut s, Spot::Screen(0));
        let screen = drawn(&mut s, 40, 100);
        let top = s.blocks.foot.top;
        let seen = runs(&screen);
        let at = |what: &str, line: u16| {
            seen.iter()
                .find(|(t, _, l, _)| t == what && *l == line)
                .unwrap_or_else(|| panic!("`{what}` is not on line {line}: {seen:?}"))
        };
        assert_eq!(at("a snippet", top).1, "text-primary");
        let label = at("fact-boot", top + 3);
        let label_last = at("fact-last", top + 4);
        assert_eq!(label.1, "text-dim");
        assert_eq!(label_last.1, label.1);
        assert_eq!(label.3, label_last.3);
        let value = at("123", top + 3);
        assert_eq!(value.1, "modeline-following");
        assert_eq!(value.3, ronoctua_design::wide("Last line caught") + 1);

        let paragraphs: Vec<_> = screen
            .as_protocol()
            .commands
            .into_iter()
            .filter_map(|c| match c {
                Command::Paragraph {
                    line,
                    lines,
                    column,
                    color,
                    content,
                    ..
                } if line >= top => Some((line, lines, column, color, content)),
                _ => None,
            })
            .collect();
        assert_eq!(paragraphs.len(), 2, "{paragraphs:?}");
        let (line, lines, column, color, content) = &paragraphs[0];
        assert_eq!((*line, *lines, *column), (top + 1, 2, 0));
        assert_eq!(color.as_str(), "text-folded");
        assert_eq!(*content, Text::Key("reason-key".to_owned()));
        let (line, lines, column, color, content) = &paragraphs[1];
        assert_eq!(*line, top + 4);
        assert_eq!(*lines, s.blocks.foot.lines - 4, "the rows left");
        assert_eq!(*column, value.3);
        assert_ne!(color.as_str(), "text-folded");
        assert_eq!(*content, Text::Data("the last line, whole".to_owned()));
    }

    /// FAILS IF: the date of a fact is one run in one tone, unlike every other
    /// date; or a row with a date last has it wrapped as a sentence.
    #[test]
    fn a_fact_is_dated_like_every_date() {
        let mut s = a_state();
        let _ = drawn(&mut s, 40, 100);
        let moment = a_dated_row("x", "y", 1).label;
        let mut row = a_filter_row("f", ScreenCell::Empty);
        row.foot = vec![
            a_fact("fact-boot", moment.clone(), BrainTone::Quiet),
            a_fact("fact-last", moment, BrainTone::Danger),
        ];
        on_a_page_of(&mut s, vec![row]);
        on_spot(&mut s, Spot::Screen(0));
        let screen = drawn(&mut s, 40, 100);
        let top = s.blocks.foot.top;
        let seen = runs(&screen);
        let day = |line: u16| {
            seen.iter()
                .find(|(t, _, l, _)| t == "23" && *l == line)
                .unwrap_or_else(|| panic!("no day on line {line}: {seen:?}"))
        };
        let lead = |line: u16| {
            seen.iter()
                .find(|(t, _, l, _)| t == "2026-08-" && *l == line)
                .unwrap_or_else(|| panic!("no date on line {line}: {seen:?}"))
        };
        assert_eq!(day(top + 1).1, "text-secondary");
        assert_eq!(lead(top + 1).1, "text-dim");
        assert_eq!(day(top + 2).1, "text-secondary");
        assert_eq!(lead(top + 2).1, "state-error");
    }

    /// FAILS IF: the footer does not say what the row's other key does on this
    /// row, whole; a click on it does not do it at once — it is the only way
    /// to it with no header on the window —; it stops saying what it is under
    /// the pointer, or spelling its `⌥`; or it lands on anything but a row of
    /// its own where the first fact leaves it no room.
    #[test]
    fn the_rows_other_action_is_named_and_a_click_takes_it() {
        let mut s = a_state();
        s.act(Input::Phrases(vec![ronoctua_design::types::Phrase {
            key: Phrases::LEADER_ALT.to_owned(),
            text: "SUPER ALT".to_owned(),
        }]));
        let _ = drawn(&mut s, 40, 100);
        on_a_page_of(&mut s, vec![a_filter_row("f", ScreenCell::Empty)]);
        on_spot(&mut s, Spot::Screen(0));
        let seen = runs(&drawn(&mut s, 40, 100));
        let top = s.blocks.foot.top;
        let (row, from, to) = s.also_at.expect("the footer does not name it");
        assert_eq!(row, 1, "not beside the first fact");
        assert_eq!(to, 100);
        let name = seen
            .iter()
            .find(|(t, ..)| t == "fixture-say-why")
            .expect("its name is not drawn");
        assert_eq!(name.2, top + 1);
        assert_eq!(
            name.3 + ronoctua_design::wide("Say why this filter exists"),
            100
        );
        assert_eq!(name.1, "state-accent");

        let _ = s.hover.set(0, (from, top + row));
        let seen = runs(&drawn(&mut s, 40, 100));
        assert!(
            seen.iter().any(|(t, ..)| t == "SUPER ALT"),
            "its `⌥` is not spelt under the pointer"
        );
        assert_eq!(
            s.pointed(),
            Some(Pointed::Key(key::ABOUT_ROW_ALSO.to_owned()))
        );
        let out = s.clicked(from, top + row);
        assert!(
            out.iter().any(
                |e| matches!(e, Effect::Ask(d) if d.what == Request::ActRowAlso && d.target == 1)
            ),
            "a click on it: {out:?}"
        );
        // A ROW THAT OFFERS NONE draws none, and no click is left behind.
        let mut bare = a_filter_row("bare", ScreenCell::Empty);
        bare.also = String::new();
        s.act(Input::ModuleScreen(ModuleScreen {
            generation: 8,
            ..with_row(s.screen.clone(), bare)
        }));
        let _ = drawn(&mut s, 40, 100);
        assert!(s.also_at.is_none());
        // A NARROW WINDOW: its own row, the last one, and the last fact above.
        let mut narrow = a_state();
        let _ = drawn(&mut narrow, 40, 46);
        on_a_page_of(&mut narrow, vec![a_filter_row("f", ScreenCell::Empty)]);
        on_spot(&mut narrow, Spot::Screen(0));
        let _ = drawn(&mut narrow, 40, 46);
        let (row, ..) = narrow.also_at.expect("the footer does not name it");
        assert_eq!(row, narrow.blocks.foot.lines - 1);
        // TOO NARROW FOR ITS NAME, nothing: half a control is none.
        let _ = drawn(&mut narrow, 40, 20);
        assert!(narrow.also_at.is_none());
    }

    /// `page`, its first row now `row`.
    fn with_row(page: Option<ModuleScreen>, row: ModuleRow) -> ModuleScreen {
        let mut page = page.expect("a page");
        page.sections[0].rows[0] = row;
        page
    }

    /// FAILS IF: moving to another row keeps the sentence scrolled: what is
    /// new in a footer has to count in what it is about, or it opens past the
    /// end of what it says.
    #[test]
    fn a_rows_facts_are_what_its_footer_is_about() {
        let filter = |last: &str| Told {
            headline: Some("a snippet".to_owned()),
            lines: vec![a_fact(
                "fact-last",
                ScreenCell::Data(last.to_owned()),
                BrainTone::Quiet,
            )],
            ..Told::default()
        };
        let told = filter("a line");
        assert_ne!(told.subject(), filter("another line").subject());
        assert_ne!(told.subject(), Told::default().subject());
        for changed in [
            Told {
                headline: None,
                ..filter("a line")
            },
            Told {
                below: Some(ScreenCell::Data("why".to_owned())),
                ..filter("a line")
            },
            Told {
                also: Some("fixture-say-why".to_owned()),
                ..filter("a line")
            },
        ] {
            assert_ne!(told.subject(), changed.subject());
        }
    }
}
