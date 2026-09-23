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
    Binding, BufferState, Capability, Column, Context, Demand, Descriptor, Effect, Guest, HoldKind,
    Icon, Input, Knob, Layout, Mind, OutcomeKind, Request, Role, Run, RunRecord, Screen, Scroll,
    Setting, Standing, StepSwitch, Table, TaskKind, TaskState, Tone,
};
use ronoctua_design::{Phrases, Region, Slice};
use std::cell::RefCell;

/// Every text key this buffer names as a literal.
///
/// A module of whole `pub const` strings, because that is the shape
/// `buffer-keys-have-text` can see: a key built with `format!` is invisible to
/// it, and the first sign of a missing one is its own name in brackets in front
/// of somebody.
///
/// The keys built by hand — `brain-task-<id>`, `brain-hold-<word>` — are the
/// shape the file manager's menus already use, and what guards them is a check
/// on the Nix side that reads the ids out of `crates/ronoctua-brain/src/tasks/`.
mod key {
    pub const TITLE: &str = "brain-title";
    pub const NOTHING: &str = "brain-nothing";

    // ---- the headings -------------------------------------------------
    /// The list of everything the brain has.
    pub const TASKS: &str = "brain-tasks";
    /// The log of everything it has done.
    pub const ACTIVITY: &str = "brain-activity";
    /// When a task may run.
    pub const SCHEDULE: &str = "brain-schedule";
    /// What a task runs, step by step.
    pub const STEPS: &str = "brain-steps";
    /// How the last few went.
    pub const HISTORY: &str = "brain-history";

    // ---- the schedule's rows ------------------------------------------
    pub const ENABLED: &str = "brain-enabled";
    pub const ON: &str = "brain-on";
    pub const OFF: &str = "brain-off";
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

    // ---- the footer ---------------------------------------------------
    pub const LAST_RUN: &str = "brain-last-run";
    pub const NO_ACTIVITY: &str = "brain-no-activity";

    // ---- what the header teaches --------------------------------------
    pub const DO_OPEN: &str = "brain-do-open";
    pub const DO_BACK: &str = "brain-do-back";
    pub const DO_PANE: &str = "brain-do-pane";
    pub const DO_CHANGE: &str = "brain-do-change";
    pub const DO_KEYS: &str = "brain-do-keys";
    pub const DO_NOTICE: &str = "brain-do-notice";
}

/// The commands this buffer registers, and the keys that reach them.
///
/// Data, and under `m ` only: the host refuses at load whoever binds under a
/// global prefix. Each id needs `cmd-<id>` in both languages.
///
/// There is deliberately no key for the two numbers of the gate — how busy is
/// too busy is a policy somebody sets once, in a file they own, and a key that
/// moved it by a step would be a key that quietly made the machine less
/// careful.
const KEYMAP: &[(&str, &str)] = &[
    ("m r", "brain-run"),
    ("m e", "brain-rehearse"),
    ("m a", "brain-abort"),
    ("m p", "brain-proceed"),
    ("m d", "brain-reset-steps"),
    ("m i", "brain-interval"),
    ("m w", "brain-window"),
    ("m o", "brain-switch"),
    ("m k", "brain-keys"),
    ("m n", "brain-notice"),
];

/// One thing the header teaches: a notation, and the key of what it does.
type Teach = (&'static str, &'static str);

/// WHERE ONE TAUGHT PAIR WAS LAID, in the header's own cells.
///
/// The ladder is walked once ([`State::ladder`]) and read three times: the
/// height counts these, the drawing paints them, and the pointer hits them.
#[derive(Debug, Clone, Copy)]
struct Rung {
    row: u16,
    /// The first cell of the notation.
    at: u16,
    /// The notation, the air and the phrase.
    cells: u16,
    /// The keys it teaches, which is also how it is acted on.
    keys: &'static str,
    /// The key of the phrase beside them.
    says: &'static str,
}

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
    ("Tab", key::DO_PANE),
    ("m r", "cmd-brain-run"),
    ("m e", "cmd-brain-rehearse"),
    ("m o", "cmd-brain-switch"),
    ("m k", key::DO_KEYS),
];

/// And what a TASK's page answers to.
const TASK_TEACHES: &[Teach] = &[
    ("RET", key::DO_CHANGE),
    ("Esc", key::DO_BACK),
    ("m r", "cmd-brain-run"),
    ("m e", "cmd-brain-rehearse"),
    ("m o", "cmd-brain-switch"),
    ("m i", "cmd-brain-interval"),
    ("m w", "cmd-brain-window"),
    ("m d", "cmd-brain-reset-steps"),
    ("m k", key::DO_KEYS),
];

/// AND THE TWO THAT ANSWER A COUNTDOWN, taught only while one is running.
///
/// They are the band's two buttons. The band draws these very notations, so a
/// person reading the line under the minibuffer is told where the keyboard's
/// answer is — which is inside this buffer, since `LEADER b` became the brain
/// itself and a sequence cannot be a command and a prefix at once.
const ANSWER_TEACHES: &[Teach] = &[("m p", "cmd-brain-proceed"), ("m a", "cmd-brain-abort")];

/// AND THE ONE A STANDING NOTICE OFFERS, taught only while it is offering.
///
/// The phrase is the notice's own — a task names the command, and the command
/// names its own label — so this table cannot say what it does. It says what
/// the KEY is, and the line above it says the rest.
const NOTICE_TEACHES: &[Teach] = &[("m n", key::DO_NOTICE)];

/// How wide a taught phrase is assumed to be when nobody has said.
///
/// A FALLBACK and no longer a column. Every phrase in the header used to be
/// given exactly this many cells, because a buffer cannot measure a sentence
/// it does not hold — so `Open it` and `Other half` were each charged
/// thirteen, and the owner read the header beside the file manager's minibuffer
/// and said what it is: enormous gaps where the minibuffer is tight.
///
/// Now the host hands the words over (`Input::Phrases`, the same door the
/// three browsers' search field uses) and the header measures them. This is
/// what a phrase is worth before the first frame, or in a driver that sends
/// none: the layout comes out loose, never wrong.
const TEACH_CELLS: u16 = 13;

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

/// What a separator costs between two taught pairs: air, the dot, air.
///
/// Three, which is what `" · "` is on the host's own hint line. An icon claims
/// [`ronoctua_design::ICON`] cells and the air before it is one — the air
/// after is the next pair's notation starting where this says it does.
const SEPARATOR_CELLS: u16 = ronoctua_design::GAP + ronoctua_design::ICON;

/// Where the rows after the first one start.
///
/// THE BLOCK'S OWN FIRST CELL, flush with the glyph. It was `ICON + GAP` —
/// indented to line up under the TITLE — and the owner read the result as
/// *"aquele enorme espaço à esquerda do `m o Switch it off`"*: three cells of
/// nothing in front of a row that has nothing above it to line up with, since
/// the row above starts with a glyph and a name and not with a rung.
const NEXT_ROW_AT: u16 = 0;

/// How wide the name in the header may be.
///
/// Fixed, because it is a KEY and a blind one: the descriptor asks the host for
/// the SHORTCUT phrases, not for every task's name, so this side cannot measure
/// it. What it can do is reserve and let the host elide.
///
/// TWENTY-SIX, and the number is the longest translation with room to spare.
/// It was twenty-two, which was English's: `brain-task-emacs-daemon` comes out
/// as `Vigiar o daemon do Emacs`, twenty-four cells, and the host cut it. A
/// budget sized to one language is the same defect the listing's `note` column
/// had — see the trap the `AGENTS.md` keeps about it.
const NAME_CELLS: u16 = 26;

/// Which page is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    /// Everything the brain has, and everything it has done.
    Home,
    /// One task: what it is, when it runs, what it runs, how it has gone.
    Task,
}

/// Which half of the home page the keyboard is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    /// The list of tasks.
    Tasks,
    /// The log of what has happened.
    Log,
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
    /// One of its steps. `RET` flips it, unless the flake owns it.
    Step(usize),
    /// One of its last runs. Nothing to do; the footer says what happened.
    Ran(usize),
}

/// Everything this buffer remembers between calls.
struct State {
    /// The last mind the host pushed. Everything drawn comes from here.
    mind: Mind,
    page: Page,
    pane: Pane,
    /// Which task the cursor is on, as an index into `mind.tasks`.
    task: usize,
    /// Which row of the activity log the cursor is on.
    log: usize,
    /// The first row of the log that is drawn.
    log_from: usize,
    /// The first WRAPPED row of the footer's sentence that is drawn.
    ///
    /// The host clamps it and hands it back (`Input::Scrolled`), so what is
    /// held here is only ever a number the host produced — this side has never
    /// seen the wrap. See [`ronoctua_protocol::Command::Paragraph`].
    foot_scroll: u16,
    /// What that offset belongs to, so it does not survive the cursor moving.
    foot_of: String,
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
    /// Whether the header is showing every key it has.
    keys_open: bool,
    /// The window, as the last `render` saw it.
    lines: u16,
    columns: u16,
    /// The words the host resolved for the keys this buffer declared.
    ///
    /// It holds them to MEASURE them, never to draw them: every phrase still
    /// goes out as a `Text::Key` and the host still translates it. What the
    /// text buys is the width — the header used to reserve a fixed thirteen
    /// cells for every phrase because it could not know how long one came out,
    /// and the owner read the result as shortcuts with enormous gaps between
    /// them.
    phrases: Phrases,
    /// Which cell the pointer is resting on, if it is in the window.
    hover: ronoctua_design::Hovers,
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
}

/// Where each block of the screen was, in the last frame drawn.
///
/// One value and not five loose `Option<u16>`, because the five are one fact:
/// how the window was divided. Split apart they were five things to keep in
/// step, and the pointer only ever consulted two of them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Blocks {
    head: Region,
    warn: Region,
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
            head: none,
            warn: none,
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
            log: 0,
            log_from: 0,
            foot_scroll: 0,
            foot_of: String::new(),
            follows: true,
            spot: 0,
            body_from: 0,
            keys_open: false,
            lines: 24,
            columns: 80,
            phrases: Phrases::empty(),
            hover: ronoctua_design::Hovers::new(),
            blocks: Blocks::new(),
        }
    }

    /// The task under the cursor.
    fn here(&self) -> Option<&TaskState> {
        self.mind.tasks.get(self.task)
    }
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
        activity: Vec::new(),
        remembers: 0,
        recent: 0,
        notice: ronoctua_design::types::Notice {
            task: String::new(),
            says: String::new(),
            act: String::new(),
        },
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

/// The glyph that says whether a task repeats or happens once.
const fn kind_glyph(k: TaskKind) -> Icon {
    match k {
        TaskKind::Routine => Icon::TaskRoutine,
        TaskKind::Once => Icon::TaskOnce,
    }
}

/// The colour a task's own rows are drawn in.
///
/// A routine keeps the ordinary ramp; a one-shot does not. The owner asked for
/// both a glyph and a colour, because a glyph two cells wide is a small thing
/// to hang a distinction on.
const fn kind_role(k: TaskKind) -> Role {
    match k {
        TaskKind::Routine => Role::Body,
        TaskKind::Once => Role::Once,
    }
}

/// The key that names a task.
fn task_key(task: &TaskState) -> String {
    format!("brain-task-{}", task.id)
}

/// And the key that says what it does.
fn does_key(id: &str) -> String {
    format!("brain-task-{id}-does")
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

/// The most rows the header may take with every key showing.
const KEYS_MOST: u16 = 4;

/// Every phrase the header can teach, for the descriptor to ask the host for.
///
/// All four tables, because the descriptor is built ONCE at load and the
/// header's contents depend on the page and on whether a countdown is running.
/// Asking for the union costs a dozen strings on one message; asking per page
/// would need a second delivery the wall does not have.
fn every_taught_phrase() -> Vec<String> {
    HOME_TEACHES
        .iter()
        .chain(TASK_TEACHES)
        .chain(ANSWER_TEACHES)
        .chain(NOTICE_TEACHES)
        .map(|(_, says)| (*says).to_owned())
        // AND THE PAGE'S OWN NAME, which is not a shortcut and is measured for
        // the same reason one is: the ladder of keys starts after the title,
        // and a title reserved blind is the gap the owner photographed across
        // the top of this page. A task's page takes its name from the mind
        // (`TaskState::name_text`); the home page's is a key of this side's
        // own, so it is asked for here.
        .chain(core::iter::once(key::TITLE.to_owned()))
        // AND THE FOOTER'S FACT LABELS, measured for the same reason: the
        // value begins where the label ENDS, and a label reserved blind put
        // `Last run` eight cells from the date beside it. See
        // [`facts_columns`].
        .chain(
            [key::LAST_RUN, key::NEEDS_ROOT, key::FLAKE_OWNED]
                .into_iter()
                .map(str::to_owned),
        )
        .collect()
}

impl State {
    /// What the page in front teaches.
    ///
    /// # Why two of them are conditional
    ///
    /// Because `m a` and `m p` answer a COUNTDOWN — stop it, or start now —
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
        };
        let mut out = base.to_vec();
        if self.counting_down() {
            out.extend_from_slice(ANSWER_TEACHES);
        }
        if !self.mind.notice.act.is_empty() {
            out.extend_from_slice(NOTICE_TEACHES);
        }
        out
    }

    /// Whether a countdown is running for anything.
    fn counting_down(&self) -> bool {
        !self.mind.announcement.task.is_empty() && self.mind.announcement.deadline > self.mind.now
    }

    /// How wide one taught pair comes out: notation, air, phrase.
    ///
    /// MEASURED, not reserved. The phrase's width is the translated word's,
    /// which this side holds only because the host handed it over — and a
    /// header that guessed was a header with thirteen cells of nothing after
    /// every short word.
    fn pair_cells(&self, teach: &Teach) -> u16 {
        let (keys, says) = teach;
        let said = self.phrases.or(says, "");
        // EMPTY IS "NOT SAID", not "an empty phrase": `i18n-complete` refuses a
        // key with no value, so nothing the host sends can be blank.
        let phrase = if said.is_empty() {
            TEACH_CELLS
        } else {
            ronoctua_design::wide(&said)
        };
        ronoctua_design::wide(keys) + ronoctua_design::GAP + phrase
    }

    /// How wide the page's own name comes out, MEASURED.
    ///
    /// [`NAME_CELLS`] is the fallback now and no longer the answer. Reserved
    /// blind it was twenty-six cells for a word five cells wide, and the ladder
    /// of keys began after all twenty-six: the owner photographed the result
    /// as a header "full of disproportionate and exaggerated spaces".
    ///
    /// The home page's name is a key of this side's own, so it is asked for
    /// through `Input::Phrases`; a task's comes with the mind
    /// (`TaskState::name_text`), because the set of tasks is not known when the
    /// descriptor is written. Both are for MEASURING — what is drawn is still
    /// the key, so a language changed at run time reaches the screen.
    ///
    /// Capped at the old reservation: a name longer than that is elided by the
    /// host, and a ladder that started after a forty-cell name would have no
    /// line left to climb.
    fn title_cells(&self) -> u16 {
        let said = match self.page {
            Page::Home => self.phrases.or(key::TITLE, ""),
            Page::Task => self.here().map(|t| t.name_text.clone()).unwrap_or_default(),
        };
        if said.is_empty() {
            NAME_CELLS
        } else {
            ronoctua_design::wide(&said).min(NAME_CELLS)
        }
    }

    /// THE LADDER, laid out: every pair that fits, and where it went.
    ///
    /// One walk, and the ONLY one. The height counts these, the drawing paints
    /// them and the pointer hits them. Written as three walks — which is what
    /// it was — the three disagree the day any of the numbers moves, and the
    /// symptom is a control that answers for its neighbour.
    fn ladder(&self) -> Vec<Rung> {
        // A CELL IS KEPT FOR THE CHEVRON, always: it appears exactly when
        // something did not fit, and a pair that took its cell would be a
        // control drawn over.
        let edge = self.columns.saturating_sub(1);
        let rows = if self.keys_open { KEYS_MOST } else { 1 };
        let mut out: Vec<Rung> = Vec::new();
        let mut row = 0;
        // THE LADDER STARTS AFTER THE NAME — the name as it MEASURES, never as
        // it was reserved (see [`Self::title_cells`]) — and the air after it is
        // the air between two pairs. It was one cell, which made the title the
        // TIGHTEST gap on a line where every other gap is three, and the owner
        // read the shortcut as *"demasiadamente próximo do título"*.
        let mut column =
            ronoctua_design::ICON + ronoctua_design::GAP + self.title_cells() + SEPARATOR_CELLS;
        let mut first_of_row = true;
        for (keys, says) in self.teaches() {
            let cells = self.pair_cells(&(keys, says));
            let lead = if first_of_row { 0 } else { SEPARATOR_CELLS };
            if column + lead + cells > edge {
                // IT WRAPS, or it stops. A pair that does not fit is dropped
                // WHOLE — a notation with half a phrase after it teaches
                // nothing and reads as a defect.
                if row + 1 >= rows || NEXT_ROW_AT + cells > edge {
                    break;
                }
                row += 1;
                column = NEXT_ROW_AT;
                out.push(Rung {
                    row,
                    at: column,
                    cells,
                    keys,
                    says,
                });
                column += cells;
                first_of_row = false;
                continue;
            }
            out.push(Rung {
                row,
                at: column + lead,
                cells,
                keys,
                says,
            });
            column += lead + cells;
            first_of_row = false;
        }
        out
    }

    /// How many rows the header takes.
    ///
    /// One when it is closed. Open, as many as the pairs that did not fit
    /// actually need — and not the ceiling, which is what it was: four rows of
    /// ground with two of them empty is a band that looks like a mistake.
    fn header_rows(&self) -> u16 {
        self.ladder()
            .last()
            .map_or(1, |r| r.row.saturating_add(1))
            .max(1)
    }

    /// THE HEADER: a glyph, the name, and every key this page answers to.
    ///
    /// # Why it is generated from a table and not written out
    ///
    /// Because the last time it was written out, `m r` and `m o` — run it now,
    /// switch it off — were missing from the page somebody goes to looking for
    /// exactly those two. A key that is not on the screen is a key that does
    /// not exist, and a list written by hand is a list that goes stale the day
    /// a key moves. `the-brain-header-is-generated` refuses a pair that is not
    /// in [`KEYMAP`].
    ///
    /// # And every rung is a CONTROL
    ///
    /// It answers the pointer, because everything in this system does: the
    /// notation lights under it and a click does what the key does. The
    /// positions come from [`Self::ladder`] and nowhere else — a second walk
    /// to decide where a click landed is how a control comes to answer for its
    /// neighbour.
    fn header(&self, screen: &mut Screen<Wit>) {
        let title = match self.page {
            Page::Home => key::TITLE.to_owned(),
            Page::Task => self.here().map_or_else(|| key::TITLE.to_owned(), task_key),
        };
        let glyph = match self.page {
            Page::Home => Icon::Brain,
            Page::Task => self.here().map_or(Icon::Brain, |t| kind_glyph(t.kind)),
        };
        let ladder = self.ladder();
        let mut pen = screen.region(self.blocks.head);
        if pen.empty() {
            return;
        }
        pen.fill(Role::Header);
        pen.put(0, Run::glyph(glyph).accent().at(0));
        let from = ronoctua_design::ICON + ronoctua_design::GAP;
        pen.put(
            0,
            Run::label(&title)
                .title()
                .at(from)
                .within(self.title_cells()),
        );

        let mut row = u16::MAX;
        for rung in &ladder {
            // THE DOT BETWEEN TWO PAIRS, and the air is the dot's on both
            // sides — the same arrangement `Piece::separator` makes on the
            // host: spaces carried by the separator and never by its
            // neighbours, so a pair's own width means one thing.
            if rung.row == row {
                pen.put(
                    rung.row,
                    Run::glyph(Icon::Separator)
                        .playing(Role::Faint)
                        .at(rung.at - SEPARATOR_CELLS + ronoctua_design::GAP),
                );
            }
            row = rung.row;
            let wide = ronoctua_design::wide(rung.keys);
            // THE SYSTEM'S OWN THREE TONES. `Role::Key` and `Role::Prose`
            // resolve to `text.subtle.strong` and `text.subtle.default` — the
            // two the minibuffer's hint line is drawn in, a hue family
            // deliberately off the content ramp. This header used to reach for
            // `accent` and `dim`, which are CONTENT colours, and the owner read
            // the two lines side by side and said the brain's was not the
            // system's.
            //
            // AND UNDER THE POINTER THE NOTATION LIGHTS, because a click on it
            // does what it says: what the eye sees lit is what the click takes.
            let lit = self.on_rung(rung);
            let keys_role = if lit { Role::Once } else { Role::Key };
            pen.put(
                rung.row,
                Run::datum(rung.keys).playing(keys_role).at(rung.at),
            );
            pen.put(
                rung.row,
                Run::label(rung.says)
                    .playing(if lit { Role::Body } else { Role::Prose })
                    .at(rung.at + wide + ronoctua_design::GAP)
                    .within(rung.cells.saturating_sub(wide + ronoctua_design::GAP)),
            );
        }
        // THE CHEVRON, in the last cell, exactly where the minibuffer's is.
        //
        // IT EXISTS WHENEVER IT DOES SOMETHING, and that is two cases and not
        // one: there are more keys than the line holds, so opening it would
        // show them — or it is already OPEN, so the control is what closes it.
        // It used to be the first case alone, which meant that the moment
        // everything fitted on the open rows the chevron vanished and there
        // was no way back: the owner photographed a header expanded with no
        // control to collapse it.
        //
        // A header open on ONE row is the one case where neither is true —
        // opening it changed nothing — and there it stays away.
        let hidden = ladder.len() < self.teaches().len();
        let opened = ladder.last().is_some_and(|r| r.row > 0);
        if hidden || opened {
            // It lights under the pointer, which is what every other control
            // in the system does and what this one could not: the wall carried
            // no pointer event until now.
            let mark = if self.keys_open {
                Icon::Collapse
            } else {
                Icon::Expand
            };
            let cell = pen.columns().saturating_sub(1);
            let role = if self.hovering(self.blocks.head, 0, cell) {
                Role::Once
            } else {
                Role::Accent
            };
            pen.put(0, Run::glyph(mark).playing(role).at(cell));
        }
    }

    /// Whether ANY pointer is resting on this cell of this block — a control
    /// lights for whichever hand is on it.
    fn hovering(&self, region: Region, row: u16, column: u16) -> bool {
        self.hover
            .first(|(at, line)| region.row_at(line) == Some(row) && at == column)
            .is_some()
    }

    /// Every row of a block a pointer is on, with WHOSE hand it is — one
    /// cursor per seat rests on this buffer (`Hovers`).
    fn hovered_rows(&self, region: Region) -> Vec<(u16, u16)> {
        self.hover
            .cells()
            .filter_map(|(seat, (_, line))| region.row_at(line).map(|row| (row, seat)))
            .collect()
    }

    /// Whether any pointer is resting on this rung.
    fn on_rung(&self, rung: &Rung) -> bool {
        self.hover
            .first(|(at, line)| {
                self.blocks.head.row_at(line) == Some(rung.row)
                    && at >= rung.at
                    && at < rung.at.saturating_add(rung.cells)
            })
            .is_some()
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
        !self.mind.notice.task.is_empty()
            || self.mind.tasks.iter().any(|t| hold_word(t.hold).is_some())
    }

    fn warning(&self, screen: &mut Screen<Wit>) {
        let mut pen = screen.region(self.blocks.warn);
        if pen.empty() {
            return;
        }
        self.warned_into(&mut pen);
    }

    fn warned_into(&self, pen: &mut Pen<'_, Wit>) {
        // A STANDING NOTICE COMES FIRST. It is about something that is
        // already wrong — an editor with no daemon behind it — where a hold
        // is about something that has not happened YET and will. Between the
        // two, the one already true is the one worth the line.
        //
        // It is the same sentence the band shows, in the same words: the two
        // are one line in two places, and a person who read one has read the
        // other.
        if !self.mind.notice.task.is_empty() {
            // THE KEY IS PLACED FIRST, because it decides how much room the
            // sentence gets. Laying the sentence out and then discovering the
            // key does not fit is how a line comes to spill — the band next
            // door makes the same argument in the same words, and this screen
            // proved it: the sentence ran to the edge and `m n` landed on the
            // last three letters of it.
            let room = if self.mind.notice.act.is_empty() {
                pen.columns()
            } else {
                // THE NOTATION FROM THE TABLE, and not a second literal: the
                // header teaches this key out of `NOTICE_TEACHES`, and a
                // string written twice is two answers to "which key is it" the
                // day one of them moves.
                let keys = NOTICE_TEACHES[0].0;
                let wide = ronoctua_design::wide(keys);
                let at = pen.columns().saturating_sub(wide);
                pen.put(0, Run::datum(keys).playing(Role::Key).at(at));
                at.saturating_sub(ronoctua_design::GAP)
            };
            pen.put(
                0,
                Run::label(&self.mind.notice.says)
                    .playing(Role::Danger)
                    .within(room),
            );
            return;
        }
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
    /// A line of label-and-value pairs.
    facts: Vec<(String, String)>,
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
        [
            part(&self.name),
            part(&self.said),
            part(&self.about),
            part(&self.trouble),
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
        // THE OFFSET BELONGS TO THE SENTENCE, and this is where it is
        // forgotten: the cursor moving to another row is another sentence, and
        // one left scrolled would open past its own end.
        let of = told.subject();
        if self.foot_of != of {
            self.foot_of = of;
            self.foot_scroll = 0;
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
        //   ..last-1 the SENTENCE, taking everything that is left
        //   last-1   the values, when there are any
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
        let values_at = last.saturating_sub(1);

        if let Some(name) = &told.name {
            pen.put(0, Run::label(name).title());
        }
        // THE LINE THE BUFFER HOLDS, under the name. It is the longest thing
        // on this screen — a step's real command — and it is DATA, so it is
        // the one part of the footer this side can measure and the host does
        // not have to elide a key for.
        let mut from = 1;
        if let Some(said) = &told.said
            && from < values_at
        {
            pen.put(from, Run::datum(said).playing(Role::Body));
            from += 1;
        }
        // THE SENTENCE TAKES EVERYTHING THAT IS LEFT: the error when there is
        // one, the description when there is not. Only the values' row is held
        // back, and only when there are values to put in it.
        let until = if told.facts.is_empty() {
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
            // Paragraph`. One cell of the width is kept for that thumb, the
            // same lane every list here keeps.
            let box_wide = wide.saturating_sub(1);
            let scroll = self.foot_scroll;
            if let Some(trouble) = &told.trouble {
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
        // AND THE FIGURES ONLY WHERE THERE IS A ROW FOR THEM THAT IS NOT THE
        // NAME'S. Six is the height this is written for and `min(left)` is the
        // only thing that shortens it — but in a six-line WINDOW the footer
        // gets one row, and with `last == 1` the last row IS row zero: the
        // values were drawn on top of the name. Found by the frame's own
        // check, which is why that check is now asked at every size.
        //
        // What goes is the figures, because the name says WHAT is in focus and
        // a date with nothing naming it is a number on a line.
        if !told.facts.is_empty() && values_at < last && values_at > 0 {
            // A TABLE AND NOT A RUNNING SUM. The first shape of this measured
            // each label by its KEY's length — `brain-last-run` is fourteen
            // characters and `Last run` is eight — so every value landed on
            // top of the label before it. A key's length is not a label's
            // length, and this side cannot learn one: it reserves, and the
            // host elides.
            let widths: Vec<u16> = told
                .facts
                .iter()
                .map(|(label, _)| {
                    let said = self.phrases.or(label, "");
                    if said.is_empty() {
                        FACT_CELLS
                    } else {
                        ronoctua_design::wide(&said).min(FACT_CELLS)
                    }
                })
                .collect();
            let layout = facts_columns(wide, &widths);
            let Some(mut row) = pen.row_of(values_at, &layout) else {
                return;
            };
            for ((label, value), (lid, vid)) in told.facts.iter().zip(FACT_SLOTS) {
                // HALF A FACT IS WORSE THAN NONE. The two columns carry the
                // same rank and a tie is broken by the later declaration, so
                // the VALUE gives way first — which left `Processor` on the
                // screen with nothing after it, read off a photograph at
                // eighty columns. A pair goes whole or not at all.
                if !layout.has(vid) {
                    break;
                }
                if !label.is_empty() {
                    row.put(lid, Run::label(label).dim());
                }
                // A VALUE IS NOT THE DESCRIPTION, and it may not wear the
                // description's colour. Both were `quiet` — `text.dim` — so
                // the sentence above and the figures below read as one block
                // of grey, which is what the owner said about this footer.
                // The label names and the value is the fact: `dim` and `body`,
                // which is the pairing every list in the shell already uses.
                row.put(vid, Run::datum(value).playing(Role::Body));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The home page: everything the brain has, and everything it has done.
// ---------------------------------------------------------------------------

/// The columns of a row in the list of tasks.
///
/// The kind's glyph is to the LEFT of the moment, which is where the owner
/// asked for it: it belongs to when the task happens rather than to what it is
/// called.
fn task_columns(columns: u16) -> Layout {
    // A CELL IS KEPT ON THE RIGHT FOR THE BAR, always — the same line
    // `log_columns` carries, and for the same reason: a bar drawn over the
    // last column is the defect the working rules name in those words.
    Table::across(columns.saturating_sub(1))
        .with(Column::fill("name", 12))
        .with(Column::fixed("kind", ronoctua_design::ICON, 2))
        .with(Column::fixed("when", 17, 1))
        // TWENTY, AND THE NUMBER IS THE LONGEST TRANSLATION. A key reserves
        // the whole of what it is given — this side never sees the sentence —
        // so the budget has to be the widest language's, not English's. At
        // twelve it was English's: `brain-hold-cpu` comes out as
        // `Processador ocupado`, nineteen cells, and the host cut it. The
        // name beside it is the one column that FILLS, so the cells come out
        // of the part of the row that has them to spare.
        .with(Column::fixed("note", 20, 3))
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
        .map_or(NAME_CELLS, |w| w.min(NAME_CELLS))
}

/// The columns of a row in the activity log.
///
/// A cell is kept on the right for the bar, always: a bar drawn over the last
/// column is the defect the working rules name in those words.
fn log_columns(columns: u16, name: u16) -> Layout {
    Table::across(columns.saturating_sub(1))
        .with(Column::fixed("when", 17, 2))
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
        .with(Column::fixed("outcome", 12, 3))
        .resolve()
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
        let hovered = self.hovered_rows(self.blocks.body);
        let tasks: Vec<TaskState> = self.mind.tasks.clone();
        let cursor = self.task;
        let pane = self.pane;
        let moments: Vec<Run<Icon>> = tasks.iter().map(|t| self.moment_of(t)).collect();
        let notes: Vec<Run<Icon>> = tasks.iter().map(|t| self.note_of(t)).collect();
        let mut pen = screen.region(self.blocks.body);
        if pen.empty() {
            return;
        }
        if tasks.is_empty() {
            pen.put(0, Run::label(key::NOTHING).quiet());
            return;
        }
        pen.put(0, Run::label(key::TASKS).title());

        // THE WINDOW ONTO THE LIST, and the lane its bar takes.
        //
        // The list drew from the first task and stopped when it ran out of
        // rows — so a machine with more tasks than the block has lines had
        // some it could not reach at all, and the wheel over it moved the
        // CURSOR instead of the page. Both are the same missing thing: an
        // offset. See [`State::follows`] for which of the two moves it.
        pen.keeping(1);
        let shown = usize::from(pen.lines().saturating_sub(1)).max(1);
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

        let layout = task_columns(pen.columns());
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
            row.put("name", Run::label(&task_key(task)).playing(role));
            row.put("kind", Run::glyph(kind_glyph(task.kind)).playing(role));
            row.put("when", moments[n].clone());
            row.put("note", notes[n].clone());
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

    /// THE ACTIVITY LOG: a container with its own ground, its own title and
    /// its own scrollbar, sitting directly on top of the footer.
    ///
    /// # Why it is a block and not the end of the home page
    ///
    /// Because it was the end of the home page, taking every line that was
    /// left, and the owner read the result: a container the height of the
    /// screen with five entries in it. What a log of a hundred wants is a
    /// WINDOW onto them — eight at a time and a bar for the rest — and a
    /// window is a fixed height, which is a block.
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
        if !self.mind.activity.is_empty() {
            self.log_from = self
                .log_from
                .min(self.mind.activity.len().saturating_sub(1));
            // THE WINDOW FOLLOWS THE CURSOR, and by the shortest move: a page
            // that jumped to put the cursor in the middle would lose the
            // reader's place every time they stepped over an edge.
            //
            // AND ONLY WHEN A PERSON MOVED THE CURSOR. The wheel scrolls the
            // page and leaves the cursor alone; a follow here that did not ask
            // would undo it on the very next frame. See [`State::follows`].
            if self.follows {
                if self.log < self.log_from {
                    self.log_from = self.log;
                } else if self.log >= self.log_from + shown {
                    self.log_from = self.log.saturating_sub(shown.saturating_sub(1));
                }
            }
            self.log_from = self.log_from.min(
                self.mind
                    .activity
                    .len()
                    .saturating_sub(shown.min(self.mind.activity.len())),
            );
        }
        let hovered = self.hovered_rows(self.blocks.log);
        let from = self.log_from;
        let cursor = self.log;
        let pane = self.pane;
        let total = self.mind.activity.len();
        let remembers = self.mind.remembers;
        let seen: Vec<RunRecord> = self
            .mind
            .activity
            .iter()
            .skip(from)
            .take(shown)
            .cloned()
            .collect();

        let mut pen = screen.region(self.blocks.log);
        pen.fill(Role::Shelf);
        // THE TITLE IS INSIDE THE GROUND, which is the owner's instruction and
        // the reason this block is one line taller than its rows. It says the
        // REACH as well as the name: a window of eight onto a hundred is a
        // different thing from a list of eight, and only the title can say so.
        // THE TITLE STOPS WHERE THE COUNT BEGINS, exactly as the `Steps`
        // heading does. A key reserves the WHOLE of what it is given — this
        // side never sees the translated word — so a title given the line
        // would reserve the count's cells too, and `nothing_overlaps` says so
        // in those words.
        let room = if remembers > 0 {
            COUNT_AT.saturating_sub(ronoctua_design::GAP)
        } else {
            pen.columns().saturating_sub(1)
        };
        pen.put(0, Run::label(key::ACTIVITY).title().within(room));
        // AND HOW MANY OF HOW MANY, beside the name.
        //
        // `Activity` alone says what the rows are; it does not say that this
        // is a window onto a longer list, which is the fact that makes the
        // scrollbar beside it mean anything. The owner asked the title to say
        // its reach — *"algo como (last 100)"* — and this is the house's own
        // way of saying it: the `Steps` heading two screens away reads
        // `6 / 12` in the same column, in the same tone, for the same reason.
        //
        // A DATUM, because two numbers are not translated. The word in front
        // of them is the key, and the hundred is the daemon's own cap coming
        // across rather than a number typed into both languages.
        if remembers > 0 {
            pen.put(
                0,
                Run::datum(&format!("{total} / {remembers}"))
                    .quiet()
                    .small()
                    .at(COUNT_AT)
                    .within(pen.columns().saturating_sub(COUNT_AT + 1)),
            );
        }
        if seen.is_empty() {
            // SMALL IS ALLOWED HERE, and only here: nothing is positioned
            // after this run. A small run advances 0,81 of a cell, so anything
            // placed after one lands where the BUDGET ended and not where the
            // ink did — see `Screen::measured`. A sentence alone on its line,
            // and a count that owns the rest of its own, push nothing.
            pen.put(1, Run::label(key::NO_ACTIVITY).quiet().small());
            return;
        }
        // THE LANE THE BAR TAKES, declared before a single row is drawn: every
        // band and every hover below stops one cell short of it. Without it the
        // band runs UNDER the bar, and the owner read the result as the
        // scrollbar *"ficando por cima do item selecionado"*.
        pen.keeping(1);
        let layout = log_columns(pen.columns(), widest_name(&seen));
        for (n, run) in seen.iter().enumerate() {
            let row = 1 + u16::try_from(n).unwrap_or(0);
            let at = from + n;
            if at == cursor && pane == Pane::Log {
                pen.band(row);
            } else if let Some(seat) = seat_on(&hovered, row) {
                pen.ground(row, Role::hover_of(seat));
            }
            let Some(mut line) = pen.row_of(row, &layout) else {
                break;
            };
            line.put("when", Run::datum(&run.when_text).dim());
            line.put(
                "task",
                Run::label(&format!("brain-task-{}", run.task)).quiet(),
            );
            // WHAT WENT WRONG, BESIDE THE NAME, as far as it fits.
            //
            // The owner asked for it there and asked for it discreet: the
            // word in the right-hand column already says `Failed`, and a
            // second alarm on the same row would be the row shouting twice.
            // What this adds is the only thing that column cannot — WHICH
            // failure — and a machine's own words are data, never a key.
            if !run.trouble.is_empty() {
                line.put("trouble", Run::datum(&run.trouble).dim());
            }
            line.put(
                "outcome",
                Run::label(&format!("brain-outcome-{}", outcome_word(run.outcome))).quiet(),
            );
        }
        // THE BAR IS OVER THE ROWS AND NOT THE TITLE. The title is inside the
        // container's ground and does not move with its contents, so a track
        // that began at the block's first line would be one line longer than
        // the list it belongs to — and the host, which builds the thumb from
        // the band it is handed, would be told the window is nine rows when
        // eight are drawn.
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

    /// When this task will next be considered, or the word for why it will not.
    fn moment_of(&self, task: &TaskState) -> Run<Icon> {
        if !task.why_not.is_empty() {
            return Run::label(&format!("brain-why-{}", task.why_not)).quiet();
        }
        if task.standing == Standing::Running {
            return Run::label("brain-standing-running").accent();
        }
        if task.next_text.is_empty() {
            return Run::label(key::NEVER).quiet();
        }
        Run::datum(&task.next_text).quiet()
    }

    /// What is happening to it, in one short phrase.
    fn note_of(&self, task: &TaskState) -> Run<Icon> {
        if let Some(word) = hold_word(task.hold) {
            return Run::label(&format!("brain-hold-{word}")).playing(Role::Danger);
        }
        if task.ran {
            return Run::label(&format!(
                "brain-outcome-{}",
                outcome_word(task.last.outcome)
            ))
            .quiet();
        }
        Run::label(&format!("brain-standing-{}", standing_word(task.standing))).quiet()
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
fn field_columns(columns: u16) -> Layout {
    Table::across(columns.saturating_sub(1))
        .with(Column::fixed("label", 20, 2))
        .with(Column::fixed("value", 24, 1))
        .with(Column::fill("unit", 6))
        .with(Column::fixed("keys", 5, 3))
        .resolve()
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
fn history_columns(columns: u16) -> Layout {
    Table::across(columns.saturating_sub(1))
        .with(Column::fixed("when", 17, 2))
        .with(Column::fixed("outcome", 12, 1))
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
        let mut out = vec![
            Spot::Enabled,
            Spot::Interval,
            Spot::Window,
            Spot::Next,
            Spot::RunNow,
            Spot::Rehearse,
        ];
        out.extend((0..task.steps.len()).map(Spot::Step));
        out.extend((0..task.history.len()).map(Spot::Ran));
        out
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
    Heading(&'static str),
    /// The description, which the host wraps. The number is how many rows it
    /// was given.
    About(String, u16),
    Field(Spot),
    Step(usize),
    Ran(usize),
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
        Row::About(_, want) => *want,
        Row::Blank | Row::Heading(_) | Row::Field(_) | Row::Step(_) | Row::Ran(_) => 1,
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
        let mut out = vec![
            Row::About(does_key(&task.id), about_rows(columns)),
            Row::Blank,
            Row::Heading(key::SCHEDULE),
            Row::Field(Spot::Enabled),
            Row::Field(Spot::Interval),
            Row::Field(Spot::Window),
            Row::Field(Spot::Next),
            Row::Field(Spot::RunNow),
            Row::Field(Spot::Rehearse),
        ];
        // THE HEADING ONLY WHEN THERE IS SOMETHING UNDER IT. Two of the four
        // tasks name a unit and declare no steps, and both of them drew this
        // title over nothing.
        if !task.steps.is_empty() {
            out.push(Row::Blank);
            out.push(Row::Heading(key::STEPS));
            out.extend((0..task.steps.len()).map(Row::Step));
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
        let acts = |r: &Row| matches!(r, Row::Field(_) | Row::Step(_) | Row::Ran(_));
        if !rows.get(n).is_some_and(acts) {
            return None;
        }
        Some(rows.iter().take(n).filter(|r| acts(r)).count())
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
            Row::Step(n) => spot == Spot::Step(*n),
            Row::Ran(n) => spot == Spot::Ran(*n),
            Row::Blank | Row::Heading(_) | Row::About(_, _) => false,
        })
    }

    /// The two numbers beside a heading, when it has them.
    ///
    /// `Steps` says how many of them are on; `History` says how many runs it
    /// is showing of how many a task keeps — the owner asked for the second in
    /// those words, *"algo como (last 10)"*, and this is the house's own way
    /// of saying it: the same column, the same tone, the same shape as the
    /// `Activity` container's `14 / 100` one screen away.
    ///
    /// The keep is the DAEMON's (`journal::RECENT`, crossing in the mind), not
    /// a ten written into two translations.
    fn count_beside(&self, task: &TaskState, heading: &str) -> Option<String> {
        if heading == key::STEPS {
            let on = task.steps.iter().filter(|s| s.on).count();
            return Some(format!("{on} / {}", task.steps.len()));
        }
        if heading == key::HISTORY && self.mind.recent > 0 {
            return Some(format!("{} / {}", task.history.len(), self.mind.recent));
        }
        None
    }

    /// A task's page, drawn in the body's block.
    fn task_page(&mut self, screen: &mut Screen<Wit>) {
        let block = self.blocks.body;
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
            while at >= self.body_from + fit_from(&rows, self.body_from, height) {
                self.body_from += 1;
            }
        }
        self.body_from = self.body_from.min(last_window(&rows, height));

        let fields = field_columns(columns);
        let steps = step_columns(columns);
        let history = history_columns(columns);
        // EVERY ROW OF THIS PAGE STOPS ONE CELL SHORT, because the bar is
        // there. The three layouts above already do it; these are the runs
        // that are placed by hand.
        let body = columns.saturating_sub(1);
        let hovered = self.hovered_rows(block);
        let mut pen = screen.region(block);
        if pen.empty() {
            return;
        }
        // THE LANE THE BAR TAKES; see the same line in the activity log.
        pen.keeping(1);
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
                .filter(|r| matches!(r, Row::Field(_) | Row::Step(_) | Row::Ran(_)))
                .count()
        });
        for row in rows.iter().skip(self.body_from) {
            if line >= last {
                break;
            }
            match row {
                Row::Blank => line += 1,
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
                    let count = self.count_beside(&task, k);
                    let room = if count.is_some() {
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
                    line += 1;
                }
                Row::About(k, want) => {
                    let room = (*want).min(last.saturating_sub(line));
                    if room > 0 {
                        pen.paragraph(line, room, Run::label(k).quiet().within(body));
                    }
                    line += room;
                }
                Row::Field(spot) => {
                    let at = At {
                        row: line,
                        spot: spot_n,
                        hovered: seat_on(&hovered, line),
                    };
                    self.field(&mut pen, &task, &at, &fields, *spot);
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
/// ONE constant for `Steps  6 / 12` and `Activity  14 / 100`: the two are the
/// same thing said about two lists, and in two columns they would read as two
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
    fn field(
        &self,
        pen: &mut Pen<'_, Wit>,
        task: &TaskState,
        at: &At,
        layout: &Layout,
        spot: Spot,
    ) {
        let line = at.row;
        let n = at.spot;
        if self.spot == n {
            pen.band(line);
        } else if let Some(seat) = at.under_pointer() {
            pen.ground(line, Role::hover_of(seat));
        }
        let Some(mut row) = pen.row_of(line, layout) else {
            return;
        };
        let mut after_unit: Option<(u16, u16)> = None;
        match spot {
            Spot::Enabled => {
                row.put("label", Run::label(key::ENABLED).dim());
                row.put(
                    "value",
                    if task.enabled {
                        Run::label(key::ON).quiet()
                    } else {
                        Run::label(key::OFF).accent()
                    },
                );
                row.put("keys", Run::datum("m o").accent());
            }
            Spot::Interval => {
                row.put("label", Run::label(key::EVERY).dim());
                let days = task.interval_days.to_string();
                row.put("value", Run::datum(&days));
                row.put("keys", Run::datum("m i").accent());
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
                after_unit = layout.field("value").map(|f| {
                    let at = f.at
                        + u16::try_from(days.chars().count()).unwrap_or(1)
                        + ronoctua_design::GAP;
                    // AND IT STOPS BEFORE THE KEY. A run placed by hand
                    // inherits no budget, and with none it reaches the edge —
                    // straight through the column that says which key changes
                    // this row, and then through the bar beyond it.
                    let until = layout
                        .field("keys")
                        .map_or_else(|| layout.ends(), |k| k.at)
                        .saturating_sub(ronoctua_design::GAP);
                    (at, until.saturating_sub(at).max(1))
                });
            }
            Spot::Window => {
                row.put("label", Run::label(key::PART).dim());
                row.put(
                    "value",
                    Run::label(&format!("brain-part-{}", part_word(task.part_of_day))),
                );
                row.put("keys", Run::datum("m w").accent());
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
                row.put("label", Run::label(label).dim());
                row.put("value", value);
            }
            Spot::RunNow => {
                row.put("label", Run::label(key::RUN_NOW).dim());
                // THE VALUE IS WHAT IS HAPPENING, never when it will happen.
                // It used to fall through to the next date, so the page read
                // `Run now   22/09/2026 14:00` — a verb answered with a
                // moment, and the same moment the row above now carries.
                if let Some(doing) = self.doing_of(task) {
                    row.put("value", doing);
                }
                row.put("keys", Run::datum("m r").accent());
            }
            Spot::Rehearse => {
                row.put("label", Run::label(key::REHEARSE).dim());
                row.put("keys", Run::datum("m e").accent());
            }
            Spot::Step(_) | Spot::Ran(_) => {}
        }
        // The row's borrow of the screen ends at its last use above, which is
        // what lets the unit be placed against what it is a unit OF. There is
        // no `drop` here: `Row` owns nothing to drop, so calling it would only
        // hold the borrow OPEN to this line rather than closing it.
        if let Some((at, room)) = after_unit {
            pen.put(line, Run::label(key::DAYS).quiet().at(at).within(room));
        }
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
        // `update-<id>` AND NOT `brain-step-<id>`: the ids are not the brain's.
        // They come from `assets/updates.nix`, which already owes every one of
        // them a name in both languages — the same keys the sequence draws.
        row.put("name", Run::label(&format!("update-{}", step.id)));
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
        row.put("when", Run::datum(&run.when_text).dim());
        row.put(
            "outcome",
            Run::label(&format!("brain-outcome-{}", outcome_word(run.outcome))),
        );
        // THE SHORT REASON ONLY. What the machine said can be a paragraph, and
        // a paragraph in a column is a paragraph with its end cut off — it is
        // in the footer, whole, under the cursor.
        if !run.at.is_empty() {
            row.put("why", Run::label(&format!("update-{}", run.at)).quiet());
        } else if !run.why.is_empty() {
            row.put("why", Run::label(&format!("brain-why-{}", run.why)).quiet());
        }
    }
}

impl State {
    /// What the footer has to say about whatever is in focus.
    fn told(&self) -> Told {
        match self.page {
            Page::Home => match self.pane {
                Pane::Tasks => self.told_about_task(),
                Pane::Log => self.told_about_activity(),
            },
            Page::Task => self.told_about_spot(),
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
        // THE TWO READINGS ARE NOT HERE, and were never asked for.
        //
        // `Memory 31%` and `Processor 4.0%` sat under every task's name, on
        // every frame, and the owner read them and said they should not be
        // shown. They are not about the task in focus — they are about the
        // machine, and the machine says what it has to say on the line that
        // ALARMS: when a reading is high enough to stop something, the warning
        // block names it in words. A percentage nobody is waiting on is a
        // number that has to be read to be dismissed.
        Told {
            name: Some(task_key(task)),
            about: Some(does_key(&task.id)),
            // GUARDED BY `ran`, like the fact above it: `last` is documented
            // as meaningless without it, and an unguarded read is only
            // harmless for as long as a default `RunRecord` happens to carry
            // an empty string.
            trouble: (task.ran && !task.last.trouble.is_empty()).then(|| task.last.trouble.clone()),
            facts,
            ..Told::default()
        }
    }

    /// One row of the activity log.
    fn told_about_activity(&self) -> Told {
        let Some(run) = self.mind.activity.get(self.log) else {
            return Told::default();
        };
        Told {
            name: Some(format!("brain-task-{}", run.task)),
            // THE MOST USEFUL SENTENCE FOR THIS ROW, and the order is the
            // order somebody wants it in: what got in the way, then where it
            // stopped, then how it ended. `Waited` as a paragraph under a row
            // that already says `Waited` is a paragraph that says nothing.
            //
            // The step is drawn by the key the update's own table owes it and
            // never by its id — an id in the record is a sentence on screen.
            about: Some(if !run.why.is_empty() {
                format!("brain-why-{}", run.why)
            } else if run.at.is_empty() {
                format!("brain-outcome-{}", outcome_word(run.outcome))
            } else {
                format!("update-{}", run.at)
            }),
            trouble: (!run.trouble.is_empty()).then(|| run.trouble.clone()),
            facts: vec![(key::LAST_RUN.to_owned(), run.when_text.clone())],
            ..Told::default()
        }
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
                    // room for it. The row can only afford two words. It is
                    // the SAME fact twice — root is the whole reason the
                    // flake keeps this one — so it is the same `if`: written
                    // apart, the two drift the day either side gains a
                    // condition.
                    facts.push((key::FLAKE_OWNED.to_owned(), String::new()));
                }
                Told {
                    name: Some(format!("update-{}", step.id)),
                    // THE REAL COMMAND, ON A LINE OF ITS OWN and not in a
                    // column: it is the longest thing on this screen, and a
                    // column cuts it exactly where it stops being useful.
                    // "The machine will update itself" is a sentence somebody
                    // should be able to open, and this is where it opens.
                    said: Some(step.command.clone()),
                    about: Some(format!("update-does-{}", step.id)),
                    trouble: None,
                    facts,
                }
            }
            Some(Spot::Ran(n)) => {
                let Some(run) = task.history.get(n) else {
                    return Told::default();
                };
                Told {
                    name: Some(format!("brain-outcome-{}", outcome_word(run.outcome))),
                    // THE MOST USEFUL SENTENCE FOR THIS ROW: what got in the
                    // way, then where it stopped, then what the task is. The
                    // step is drawn by the key the update's own table owes it
                    // and never by its id — an id in the record is a sentence
                    // on the screen.
                    // WHAT GOT IN THE WAY, then where it stopped, then how
                    // it ended — and NEVER the task's own description, which
                    // this page already opens with three rows up. A run that
                    // simply worked used to fall back to it, and the owner
                    // read the result: the same paragraph printed twice on one
                    // screen. It is the same order `told_about_activity` uses
                    // for the same row on the other page.
                    about: Some(if !run.why.is_empty() {
                        format!("brain-why-{}", run.why)
                    } else if run.at.is_empty() {
                        format!("brain-outcome-{}", outcome_word(run.outcome))
                    } else {
                        format!("update-{}", run.at)
                    }),
                    trouble: (!run.trouble.is_empty()).then(|| run.trouble.clone()),
                    facts: vec![(key::LAST_RUN.to_owned(), run.when_text.clone())],
                    said: None,
                }
            }
            // EVERY OTHER SPOT IS ABOUT THE TASK, which is what the page is
            // about: the schedule rows change it, and what they change is
            // worth explaining once rather than five times.
            //
            // AND IT DOES NOT REPEAT THE DESCRIPTION. On the home page the
            // footer is the only place the description exists, so it carries
            // it; here the page opens with it, three rows up. Photographed,
            // the two read as one paragraph printed twice on one screen —
            // which is the defect the activity log has a test against, in
            // this same file, in these same words.
            //
            // What a schedule row wants instead is what the plan moved down
            // here when `The machine` stopped being a section: how it last
            // went, and how busy the machine is — the two numbers that decide
            // whether any of these rows will fire.
            _ => {
                let mut facts = Vec::new();
                if task.ran {
                    facts.push((key::LAST_RUN.to_owned(), task.last.when_text.clone()));
                }
                Told {
                    name: Some(task_key(task)),
                    trouble: (task.ran && !task.last.trouble.is_empty())
                        .then(|| task.last.trouble.clone()),
                    facts,
                    ..Told::default()
                }
            }
        }
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

    /// What a command id does.
    fn command(&mut self, id: &str) -> Vec<Effect> {
        match id {
            "brain-run" => self.verb(Request::RunTask),
            "brain-rehearse" => self.verb(Request::RehearseTask),
            "brain-abort" => self.verb(Request::AbortTask),
            "brain-proceed" => self.verb(Request::Proceed),
            "brain-reset-steps" => self.tune(Knob::ResetSteps),
            "brain-interval" => {
                let Some(days) = self.here().map(|t| t.interval_days) else {
                    return Vec::new();
                };
                self.tune(Knob::IntervalDays(self.next_cadence(days)))
            }
            "brain-window" => {
                let Some(part) = self.here().map(|t| t.part_of_day) else {
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
            // WHAT THE STANDING NOTICE OFFERED, by the id the notice named.
            // A notice with nothing to offer answers nothing — the key is
            // bound and the page does not teach it while there is no button.
            "brain-notice" => {
                if self.mind.notice.act.is_empty() {
                    return Vec::new();
                }
                vec![Effect::Command(self.mind.notice.act.clone())]
            }
            "brain-keys" => {
                self.keys_open = !self.keys_open;
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
            Page::Home => match self.pane {
                Pane::Tasks if self.here().is_some() => {
                    self.page = Page::Task;
                    self.spot = 0;
                    self.body_from = 0;
                    vec![Effect::Redraw]
                }
                // A ROW OF THE LOG OPENS ITS TASK, which is the only thing
                // there is to do with a record of something that has already
                // happened.
                Pane::Log => {
                    let Some(run) = self.mind.activity.get(self.log) else {
                        return Vec::new();
                    };
                    let Some(at) = self.mind.tasks.iter().position(|t| t.id == run.task) else {
                        return Vec::new();
                    };
                    self.task = at;
                    self.page = Page::Task;
                    self.spot = 0;
                    self.body_from = 0;
                    vec![Effect::Redraw]
                }
                Pane::Tasks => Vec::new(),
            },
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
                // NEXT AND A PAST RUN ARE THE TWO ROWS THAT ONLY ANSWER.
                // The cursor stops on them because the footer has more to say
                // about each than the row can hold; `RET` does nothing,
                // because there is nothing about a moment to do.
                Some(Spot::Next | Spot::Ran(_)) | None => Vec::new(),
            },
        }
    }

    /// How many things the cursor can walk in the place it is.
    fn walkable(&self) -> usize {
        match self.page {
            Page::Home => match self.pane {
                Pane::Tasks => self.mind.tasks.len(),
                Pane::Log => self.mind.activity.len(),
            },
            Page::Task => self.spots().len(),
        }
    }

    /// The cursor of the place the keyboard is in.
    fn cursor(&mut self) -> &mut usize {
        match self.page {
            Page::Home => match self.pane {
                Pane::Tasks => &mut self.task,
                Pane::Log => &mut self.log,
            },
            Page::Task => &mut self.spot,
        }
    }

    /// A new mind arrived.
    fn received(&mut self, mind: Mind) -> Vec<Effect> {
        self.mind = mind;
        // THE CURSORS STAY INSIDE THEIR LISTS. A task can disappear — a
        // one-shot is gone the moment it has run — and a cursor left past the
        // end would draw a band on nothing and answer keys about a row that is
        // not there.
        let tasks = self.mind.tasks.len();
        self.task = self.task.min(tasks.saturating_sub(1));
        let log = self.mind.activity.len();
        self.log = self.log.min(log.saturating_sub(1));
        let spots = self.spots().len();
        self.spot = self.spot.min(spots.saturating_sub(1));
        vec![Effect::Redraw]
    }

    /// Where a click landed, as an index into whichever list is there.
    fn clicked(&mut self, column: u16, line: u16) -> Vec<Effect> {
        // THE CHEVRON FIRST, because it is drawn over the end of the header's
        // last rung and a click there is about the control.
        if line == 0 && column + 1 >= self.columns {
            self.keys_open = !self.keys_open;
            return vec![Effect::Redraw];
        }
        // AND THEN A TAUGHT PAIR, because a shortcut drawn on the screen is a
        // control: clicking it does what the key does. Where each one landed
        // is [`Self::ladder`]'s answer and not a second walk — the rule this
        // buffer's scrollbar had to learn the hard way.
        if let Some(row) = self.blocks.head.row_at(line)
            && let Some(rung) = self
                .ladder()
                .into_iter()
                .find(|r| r.row == row && column >= r.at && column < r.at + r.cells)
        {
            return self.acts(rung.keys);
        }
        // AND THEN THE BLOCK THAT OWNS THE LINE.
        //
        // It used to be `line >= tasks_at` and `line >= log_at`, with no floor
        // under either — so a click in the FOOTER was attributed to the
        // activity log whenever the log reached far enough down the page, and
        // a click below the task list landed on the log before the log began.
        // A block answers `row_at` or it answers `None`, which is the same
        // machine the file manager's `Section::row_at` is.
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
                    let rows = self.page_rows();
                    if let Some(n) = row_at_line(&rows, self.body_from, row)
                        && let Some(spot) = Self::spot_of_row(&rows, n)
                    {
                        return self.reach(spot, self.spot == spot, |s, n| s.spot = n);
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
            if n < self.mind.activity.len() {
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

    /// What a notation drawn in the header does when it is clicked.
    ///
    /// The keys the header teaches are of two kinds and both are reachable:
    /// this buffer's own commands, which the [`KEYMAP`] names, and the three
    /// the HOST sends as inputs — there is no command id for `RET`, so the
    /// notation is what says which it is.
    fn acts(&mut self, keys: &str) -> Vec<Effect> {
        match keys {
            "RET" => self.confirm(),
            "Esc" => {
                if self.page == Page::Task {
                    self.page = Page::Home;
                    return vec![Effect::Redraw];
                }
                Vec::new()
            }
            "Tab" => {
                if self.page == Page::Home {
                    self.pane = match self.pane {
                        Pane::Tasks => Pane::Log,
                        Pane::Log => Pane::Tasks,
                    };
                }
                vec![Effect::Redraw]
            }
            other => KEYMAP
                .iter()
                .find(|(k, _)| *k == other)
                .map_or_else(Vec::new, |(_, id)| {
                    let id = (*id).to_owned();
                    self.command(&id)
                }),
        }
    }

    /// The wheel, over whatever list it was over.
    /// A BAR THAT WAS DRAGGED, in the block under the pointer.
    ///
    /// The host answered with a ROW — it built the thumb, so it owns the
    /// inverse — and this puts it on the right list. Absolute, so a long drag
    /// cannot drift away from the hand, and it clears [`State::follows`] for
    /// the reason the wheel does: what moved is the window, not the cursor.
    fn scrolled(&mut self, line: u16, first: usize) -> Vec<Effect> {
        let put = |at: &mut usize, room: usize, total: usize| {
            *at = first.min(total.saturating_sub(room.min(total)));
        };
        // THE FOOTER'S SENTENCE FIRST, because its bar is drawn inside the
        // footer's block and a line there belongs to no list. The number is
        // the HOST's — it wrapped the sentence and clamped the offset — so it
        // is taken as it comes.
        if self.blocks.foot.holds(line) {
            self.foot_scroll = u16::try_from(first).unwrap_or(u16::MAX);
            return vec![Effect::Redraw];
        }
        match self.page {
            Page::Home => {
                if self.blocks.log.holds(line) && !self.mind.activity.is_empty() {
                    put(
                        &mut self.log_from,
                        usize::from(LOG_ROWS),
                        self.mind.activity.len(),
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
                if self.blocks.log.holds(line) && !self.mind.activity.is_empty() {
                    self.log_from = by(
                        self.log_from,
                        usize::from(LOG_ROWS),
                        self.mind.activity.len(),
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
            phrases: every_taught_phrase(),
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
            let head = s.header_rows().min(left);
            left -= head;
            let warn = u16::from(s.warned()).min(left);
            left -= warn;
            // FIXED, and not `told.rows(…)`: the height no longer answers to
            // what is under the cursor. `min(left)` is the only thing that can
            // shorten it, and that is the window being too small rather than
            // the page changing shape.
            let foot = FOOT_LINES.min(left);
            left -= foot;
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
                Slice::Fixed(head),
                Slice::Fixed(warn),
                Slice::Rest,
                Slice::Fixed(log),
                Slice::Fixed(foot),
            ]);
            s.blocks = Blocks {
                head: blocks[0],
                warn: blocks[1],
                body: blocks[2],
                log: blocks[3],
                foot: blocks[4],
            };
            let _ = left;

            s.header(&mut screen);
            s.warning(&mut screen);
            match s.page {
                Page::Home => s.home(&mut screen),
                Page::Task => s.task_page(&mut screen),
            }
            s.draw_log(&mut screen);
            s.foot(&mut screen, &told);
            screen
        }
    }

    /// One input, answered.
    fn act(&mut self, e: Input) -> Vec<Effect> {
        match e {
            Input::EnterState(id) => {
                self.page = if id == "task" { Page::Task } else { Page::Home };
                self.spot = 0;
                self.body_from = 0;
                vec![Effect::Redraw]
            }
            Input::Mind(mind) => self.received(mind),
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
                    self.pane = match self.pane {
                        Pane::Tasks => Pane::Log,
                        Pane::Log => Pane::Tasks,
                    };
                }
                vec![Effect::Redraw]
            }
            Input::Confirm => self.confirm(),
            Input::Cancel => {
                if self.page == Page::Task {
                    self.page = Page::Home;
                    return vec![Effect::Redraw];
                }
                // NOT `Effect::Close`: `Esc` on the home page is the host's to
                // answer, and a buffer that closed itself would take the
                // window with it.
                Vec::new()
            }
            Input::Click(p) => self.clicked(p.column, p.line),
            Input::Wheel(w) => self.wheeled(w.at.line, w.lines),
            // A BAR DRAGGED. The host turned the pointer's height into a row;
            // this side says which LIST it belongs to, which is the half only
            // it knows. Same routing as the wheel, one line above.
            Input::Scrolled(d) => self.scrolled(d.at.line, d.first as usize),
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
            // THE WORDS FOR THE KEYS THIS BUFFER TEACHES, so the header can
            // measure them. They arrive once, before the first frame.
            Input::Phrases(said) => {
                self.phrases = Phrases::of(said.into_iter().map(|p| (p.key, p.text)).collect());
                vec![Effect::Redraw]
            }
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
            outcome,
            at: String::new(),
            why: String::new(),
            trouble: String::new(),
            steps: Vec::new(),
        }
    }

    fn a_step(id: &str, on: bool, ships_on: bool, root: bool) -> TaskStep {
        TaskStep {
            id: id.to_owned(),
            command: "nix flake update".to_owned(),
            needs_root: root,
            on,
            on_by_default: ships_on,
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
                a_task("system-update", TaskKind::Routine),
                a_task("cache-sweep", TaskKind::Once),
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
            // A NOTICE WITH A BUTTON, because that is the shape with two runs
            // on one line — and the only one where they can collide. Without
            // it in the fixture, `nothing_overlaps` never saw the row where
            // the sentence ran under the key.
            notice: ronoctua_design::types::Notice {
                task: "emacs-daemon".to_owned(),
                says: "emacs-no-daemon".to_owned(),
                act: "brain-start-emacs".to_owned(),
            },
            remembers: 100,
            recent: 10,
            activity: vec![
                a_run("system-update", OutcomeKind::Done),
                a_run("cache-sweep", OutcomeKind::Held),
            ],
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
        s
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
        for (_, id) in KEYMAP {
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
                for page in [Page::Home, Page::Task] {
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
                for page in [Page::Home, Page::Task] {
                    for open in [false, true] {
                        let mut s = a_state();
                        s.page = page;
                        s.keys_open = open;
                        let screen = drawn(&mut s, lines, columns);
                        screen.check().unwrap_or_else(|e| {
                            panic!("{page:?} at {lines}x{columns}, keys {open}: {e}")
                        });
                    }
                }
            }
        }
    }

    /// And nothing is covered, and nothing overlaps.
    #[test]
    fn nothing_overlaps_anything() {
        for lines in [10_u16, 24, 40] {
            for columns in [60_u16, 100, 140] {
                for page in [Page::Home, Page::Task] {
                    for held in [false, true] {
                        let mut s = a_state();
                        s.page = page;
                        if held {
                            // THE OTHER SHAPE OF THE WARNING LINE. The fixture
                            // carries a notice, so without this the hold branch —
                            // a different key, a different budget — was never
                            // measured at any width.
                            s.mind.notice.task.clear();
                            s.mind.tasks[0].hold = HoldKind::Cpu;
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
            let nth = rows
                .iter()
                .take(at)
                .filter(|r| matches!(r, Row::Field(_) | Row::Step(_) | Row::Ran(_)))
                .count();
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
        let mut bare = a_task("cache-sweep", TaskKind::Routine);
        bare.steps = Vec::new();
        let rows = s.rows_of(&bare, 100);
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, Row::Heading(k) if *k == key::STEPS)),
            "a task with no steps drew the steps heading"
        );
        // And one WITH steps still has it.
        let full = a_task("system-update", TaskKind::Routine);
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
        // THE FIXTURE CARRIES A NOTICE, because that is the shape with two
        // runs on one line. Here the question is the other one — does the
        // line exist when nothing is wrong — so it is cleared first.
        s.mind.notice.task.clear();
        assert!(!s.warned(), "a warning with nothing wrong");

        // AND A NOTICE IS A WARNING TOO. It is about something already true,
        // where a hold is about something that has not happened yet.
        let with_notice = a_state();
        assert!(with_notice.warned());

        s.mind.tasks[0].hold = HoldKind::Cpu;
        assert!(s.warned(), "the processor is busy and nothing said so");

        // AND THE BLOCK FOLLOWS THE ANSWER. The height is decided from
        // `warned` before the window is divided, so a frame that says there is
        // nothing wrong must not reserve a line for saying it.
        let _ = ctx;
        let mut quiet = a_state();
        quiet.mind.notice.task.clear();
        for t in &mut quiet.mind.tasks {
            t.hold = HoldKind::None;
        }
        drawn(&mut quiet, 24, 100);
        assert_eq!(quiet.blocks.warn.lines, 0, "a line reserved for no news");
        let mut loud = a_state();
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
        gone.activity.clear();
        s.received(gone);
        assert_eq!(s.task, 0);
        assert_eq!(s.log, 0);
    }

    /// `Tab` crosses the home page's two halves and does nothing on a task's.
    #[test]
    fn tab_crosses_the_two_halves() {
        let mut s = a_state();
        assert_eq!(s.pane, Pane::Tasks);
        s.act(Input::Toggle);
        assert_eq!(s.pane, Pane::Log);
        s.act(Input::Toggle);
        assert_eq!(s.pane, Pane::Tasks);
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
        for n in 0..40 {
            s.mind
                .activity
                .push(a_run(&format!("task-{n}"), OutcomeKind::Done));
        }
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

    /// Enter opens the task, Esc comes back, and Esc on the home page is the
    /// host's to answer.
    #[test]
    fn enter_opens_and_esc_comes_back() {
        let mut s = a_state();
        assert_eq!(s.state_id(), "normal");
        s.act(Input::Confirm);
        assert_eq!(s.state_id(), "task");
        s.act(Input::Cancel);
        assert_eq!(s.state_id(), "normal");
        assert!(
            s.act(Input::Cancel).is_empty(),
            "the buffer answered Esc on the home page"
        );
    }

    /// A row of the log opens the task it belongs to.
    #[test]
    fn a_log_row_opens_its_task() {
        let mut s = a_state();
        s.pane = Pane::Log;
        s.log = 1; // the cache-sweep record
        s.act(Input::Confirm);
        assert_eq!(s.page, Page::Task);
        assert_eq!(s.here().map(|t| t.id.as_str()), Some("cache-sweep"));
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

        // A wait, with a reason.
        s.mind.activity[1].outcome = OutcomeKind::Held;
        s.mind.activity[1].why = "ram".to_owned();
        s.log = 1;
        assert_eq!(s.told().about.as_deref(), Some("brain-why-ram"));

        // A failure that named a step: the step, by the key the update's own
        // table owes it, and never the bare id.
        s.mind.activity[1].why.clear();
        s.mind.activity[1].at = "a-step".to_owned();
        assert_eq!(s.told().about.as_deref(), Some("update-a-step"));

        // And with neither, the outcome is all there is to say.
        s.mind.activity[1].at.clear();
        assert_eq!(s.told().about.as_deref(), Some("brain-outcome-held"));

        // WHAT THE MACHINE SAID goes through verbatim, as a datum.
        s.mind.activity[1].trouble = "error: builder failed".to_owned();
        assert_eq!(s.told().trouble.as_deref(), Some("error: builder failed"));
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
            Some(does_key("system-update").as_str()),
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
                Some(does_key("system-update").as_str()),
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
        // AND IT KEEPS THE LANE ITS THUMB GOES IN, the same cell every list
        // here keeps: no bar draws over content and no content over a bar.
        assert_eq!(columns, s.columns - 1);

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
        let out = s.scrolled(s.blocks.foot.top + 1, 2);
        assert!(!out.is_empty(), "a drag in the footer did nothing");
        assert_eq!(s.foot_scroll, 2);
    }

    /// FAILS IF: an expanded header has no control to collapse it.
    ///
    /// The chevron used to appear only when something did NOT fit, so the
    /// moment everything fitted on the open rows it vanished — and with it the
    /// only way back. The owner photographed a header expanded with no way to
    /// close it.
    #[test]
    fn an_open_header_can_be_closed_again() {
        for columns in [60_u16, 100, 140, 200] {
            let mut s = a_state();
            s.page = Page::Task;
            s.keys_open = true;
            let screen = drawn(&mut s, 26, columns);
            let rows = s.blocks.head.lines;
            if rows <= 1 {
                // Opening it changed nothing, so there is nothing to collapse.
                continue;
            }
            let list = screen.as_protocol();
            let mark = list.commands.iter().any(|c| {
                matches!(
                    c,
                    ronoctua_design::frame::Command::Text {
                        line: 0,
                        text: ronoctua_design::frame::Text::Icon(i),
                        ..
                    } if i.contains("Collapse")
                )
            });
            assert!(
                mark,
                "at {columns} columns the header opens to {rows} rows with no way to close it"
            );
        }
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

    /// FAILS IF: a shortcut drawn in the header does not answer the pointer,
    /// or answers for its neighbour.
    ///
    /// A key that is drawn is a control, and the positions come from the same
    /// walk that paints them — see [`State::ladder`].
    #[test]
    fn a_taught_key_is_a_control() {
        let mut s = a_state();
        s.keys_open = true;
        let _ = drawn(&mut s, 40, 140);
        let ladder = s.ladder();
        floor_of(ladder.len(), 4, "rungs on the header");
        for rung in &ladder {
            if rung.keys != "Tab" {
                continue;
            }
            // `Tab` swaps the two halves of the home page, which is the one
            // rung whose effect is visible without a host.
            let line = s.blocks.head.top + rung.row;
            s.pane = Pane::Tasks;
            s.clicked(rung.at, line);
            assert_eq!(s.pane, Pane::Log, "a click on `Tab` did nothing");
            // AND THE CELL BEFORE IT IS NOT IT. The separator belongs to
            // neither pair, and a rung that claimed it would answer for the
            // one before.
            s.pane = Pane::Tasks;
            s.clicked(rung.at.saturating_sub(1), line);
            assert_eq!(
                s.pane,
                Pane::Tasks,
                "the air before a rung answered as the rung"
            );
            return;
        }
        panic!("the home page stopped teaching `Tab`");
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
}
