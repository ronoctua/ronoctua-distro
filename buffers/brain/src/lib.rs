//! The brain's screen — `LEADER s b`, and a window of its own.
//!
//! # What it is for
//!
//! The machine does things by itself, and until this screen nothing said what,
//! when, or what came of it. Two questions live here: *what is my machine going
//! to do*, on the home page, and *exactly what does that mean*, on the page
//! behind it.
//!
//! # It reads nothing
//!
//! Invariant 11. The host holds the socket to `ronoctua-brain` on a thread and
//! pushes `input::mind` in; the component only decides what to draw. Everything
//! that touches the machine leaves as `effect::ask` — the verbs — or
//! `effect::tune` — the preferences —, and the host refuses both unless this
//! descriptor declared `capability::brain`.
//!
//! It has no clock either, which is why `mind.now` crosses: a countdown is
//! `deadline - now`, and without the second number the first one is a moment
//! this side cannot place.
//!
//! # Dates are ISO, and that is not a style choice
//!
//! A component cannot translate (invariant 11), and *"19 ago"* against
//! *"19 Aug"* is exactly the kind of thing it must not invent. `2026-08-23
//! 13:00` is the same in both languages, and the host — which has the zone and
//! the bundle — is what formatted it: the record carries the number for the
//! arithmetic this side does, and the TEXT for the date it cannot compute.
//!
//! # Two pages, and the second one is the update's
//!
//! `normal` is the home page: one or two lines about the update, and a line
//! about the brain itself. `update` is the page behind it — what it runs, when
//! it may run, and what happened last time. The state is called `update`
//! because the update is the one task with a page of its own; the day a second
//! one grows one, it declares a state of its own and the match below grows an
//! arm.

use ronoctua_design::guest::Wit;
use ronoctua_design::types::{
    Binding, BufferState, Capability, Column, Context, Demand, Descriptor, Effect, Guest, HoldKind,
    Icon, Input, Knob, Layout, Mind, OutcomeKind, Request, Role, Run, Screen, Setting, Standing,
    StepSwitch, Table, TaskState, Tone, Vital,
};
use std::cell::RefCell;

/// Every text key this buffer names as a literal.
///
/// A module of whole `pub const` strings, because that is the shape
/// `buffer-keys-have-text` can see: a key built with `format!` is invisible to
/// it, and the first sign of a missing one would be its own name drawn in
/// brackets in front of the person.
///
/// The keys this screen builds by hand — `brain-task-<id>`,
/// `brain-step-<id>`, `brain-hold-<word>` — are the shape the file manager's
/// menus already use (`{prefix}-does-<id>`), and what guards THEM is a check on
/// the Nix side that reads the ids out of `crates/ronoctua-brain/src/tasks/`.
mod key {
    pub const TITLE: &str = "brain-title";
    pub const NOTHING: &str = "brain-nothing";
    pub const HINT: &str = "brain-hint";
    pub const HINT_UPDATE: &str = "brain-hint-update";

    pub const STEPS: &str = "brain-steps";
    pub const WHEN: &str = "brain-when";
    /// The label beside what is happening RIGHT NOW.
    ///
    /// It was `WHEN`, and `WHEN` is the block's heading — so the rehearsal row
    /// read "when it runs — a rehearsal", which is a heading pretending to be
    /// a label. One key cannot be both the question and the field's name.
    pub const DOING: &str = "brain-doing";
    /// The label beside how the last run ended.
    ///
    /// Also `WHEN` once, and the sentence it produced was "when it runs — it
    /// worked".
    pub const OUTCOME: &str = "brain-outcome";
    pub const LAST: &str = "brain-last";

    pub const EVERY: &str = "brain-every";
    pub const DAYS: &str = "brain-days";
    pub const PART: &str = "brain-part";
    pub const NEXT: &str = "brain-next";
    pub const WAITING: &str = "brain-waiting";
    pub const SOON: &str = "brain-soon";
    pub const REHEARSAL: &str = "brain-rehearsal";
    pub const NEVER: &str = "brain-never";
    pub const NEEDS_ROOT: &str = "brain-needs-root";
    pub const CHANGED: &str = "brain-changed";
    pub const FAILED_AT: &str = "brain-failed-at";
}

/// The commands this buffer registers, and the keys that reach them.
///
/// Data, and under `m ` only: the host refuses at load whoever binds under a
/// global prefix. Each id needs `cmd-<id>` in both languages.
///
/// Six of them act, three choose. There is deliberately no key for the two
/// numbers of the gate — how busy is too busy is a policy somebody sets once,
/// in a file they own, and a key that changed it by a step would be a key that
/// silently made the machine less careful.
const KEYMAP: &[(&str, &str)] = &[
    ("m r", "brain-run"),
    ("m e", "brain-rehearse"),
    ("m a", "brain-abort"),
    ("m p", "brain-proceed"),
    ("m t", "brain-toggle-step"),
    ("m d", "brain-reset-steps"),
    ("m i", "brain-interval"),
    ("m w", "brain-window"),
    ("m o", "brain-switch"),
];

/// Which page is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    /// The home page: what the machine is going to do.
    Home,
    /// The update's own page: what it runs, when, and what happened.
    Behind,
}

/// Everything this buffer remembers between calls.
struct State {
    /// The last mind the host pushed. Everything drawn comes from here.
    mind: Mind,
    page: Page,
    /// Which task the cursor is on, as an index into `mind.tasks`.
    task: usize,
    /// Which step the cursor is on, on the page behind.
    step: usize,
    /// The window's height, as the last `render` saw it.
    lines: u16,
    /// The row the clickable list started on in the last `render`.
    ///
    /// Recorded rather than computed, for the reason the memory panel's own
    /// `list_top` gives: the block above the list grows with what the machine
    /// reported, and a fixed offset puts the cursor on a neighbour of the row
    /// that was clicked. `None` when the page has no list to click.
    list_top: Option<u16>,
}

impl State {
    const fn fresh() -> Self {
        Self {
            mind: empty_mind(),
            page: Page::Home,
            task: 0,
            step: 0,
            lines: 24,
            list_top: None,
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
    }
}

thread_local! {
    /// `const {}`: the initialiser runs at compile time, so the first access
    /// costs nothing and there is no lazy-init check on every borrow.
    static STATE: RefCell<State> = const { RefCell::new(State::fresh()) };
}

// ---------------------------------------------------------------------------
// The vocabulary: one word per case, and the host turns it into a sentence.
// ---------------------------------------------------------------------------

/// The word a hold is looked up by, as `brain-hold-<word>`.
const fn hold_word(h: HoldKind) -> Option<&'static str> {
    match h {
        // NOT a hold, and not a word either: nothing is in the way, so there is
        // no line to draw. `none` is a case rather than an absence for the
        // reason `guard-mode::unknown` is one — a record cannot hold nothing.
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

/// The next part of the day, cycling — what one key press does.
///
/// The order is the day's own, with `any` at the end: stepping through night,
/// morning, afternoon and evening is a person walking forward through their
/// day, and "whenever" is not a time of day at all.
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

/// A task's own key, as `brain-task-<id>`.
fn task_key(task: &TaskState) -> String {
    format!("brain-task-{}", task.id)
}

/// Per mille as a percentage with one decimal — `123` becomes `12.3%`.
///
/// The separator is a full stop because a component does not know the language
/// (invariant 11), and a comma here would be a decision about locale taken in
/// the wrong place. It is the memory panel's own function, and it is copied
/// rather than shared for one reason: `ronoctua-design` is the layout
/// vocabulary and a number's spelling is not layout.
fn percent(per_mille: u16) -> String {
    format!("{}.{}%", per_mille / 10, per_mille % 10)
}

/// A stretch of seconds as a clock reads it — `03:20`.
///
/// Digits and a colon, which is the same in every language this system speaks.
/// A sentence — *"in three minutes"* — would need arguments the `text` variant
/// does not carry, and inventing one here would be the component translating.
fn clock(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// One reading of the pulse, when the machine measured it.
///
/// `None` is not zero: a kernel with no `/proc/pressure/cpu` reports nothing,
/// and the screen draws a dash rather than a number nobody measured.
fn vital(mind: &Mind, of: Vital) -> Option<u16> {
    mind.pulse
        .signs
        .iter()
        .find(|s| s.of == of)
        .map(|s| s.amount)
}

// ---------------------------------------------------------------------------
// The two pages.
// ---------------------------------------------------------------------------

/// Where the home page's columns sit.
///
/// Three, and the order they give way in is the order of what the row is FOR:
/// the name is what one came to read, so it never goes; the moment is next; the
/// note — a hold, or how the last run ended — goes first, because it is the one
/// the row can be read without.
fn home_columns(columns: u16) -> Layout {
    Table::across(columns)
        .with(Column::fill("name", 12))
        // `2026-08-23 13:00` is sixteen cells, and one of air after it.
        .with(Column::fixed("when", 17, 1))
        .with(Column::fixed("note", 14, 2))
        .resolve()
}

/// Where the page behind puts a label, its value, and the unit of it.
///
/// The unit is a column of its own because it is a KEY beside a number that is
/// DATA: `7` is the same in every language and "days" is not, and the two
/// cannot be one run. It is the pattern the memory panel already draws with.
fn field_columns(columns: u16) -> Layout {
    Table::across(columns)
        .with(Column::fixed("label", 16, 2))
        .with(Column::fill("value", 12))
        .with(Column::fixed("unit", 10, 1))
        .resolve()
}

impl State {
    /// The home page: what the machine is going to do.
    fn home(&mut self, screen: &mut Screen<Wit>) {
        let line = screen.heading(Icon::Brain, key::TITLE);
        screen.go_to(line);

        if self.mind.tasks.is_empty() {
            screen.line(Run::label(key::NOTHING).quiet());
            self.list_top = None;
            self.foot(screen, key::HINT);
            return;
        }

        let layout = home_columns(screen.columns());
        self.list_top = Some(screen.row());
        // A COPY of what is drawn, because the borrow of `self.mind` would
        // outlive the `&mut screen` the row needs. The list is a handful of
        // short records; the alternative is an index dance that says nothing.
        let tasks: Vec<TaskState> = self.mind.tasks.clone();
        for (n, task) in tasks.iter().enumerate() {
            if screen.row() + 2 > screen.lines() {
                break;
            }
            let at = screen.row();
            if n == self.task {
                screen.band(at);
            }
            let mut row = screen.row_of(at, &layout);
            row.put("name", Run::label(&task_key(task)));
            row.put("when", self.moment_of(task));
            row.put("note", self.note_of(task));
            let tall = row.tall();
            screen.go_to(at + tall);
            // THE LINE UNDER THE CURSOR, and only under the cursor: what this
            // task really does, in the person's own language. It is the rule
            // the disk formatter's menus already follow — every line has two
            // registers, and the quiet one is what the cursor is for.
            if n == self.task && screen.row() < screen.lines() {
                screen.line(
                    Run::label(&format!("{}-does", task_key(task)))
                        .dim()
                        .small(),
                );
            }
        }

        // The brain's own line: what it is doing, and how busy the machine is.
        if screen.row() + 2 < screen.lines() {
            screen.skip(ronoctua_design::AIR);
            let rule = screen.row();
            screen.rule(rule);
            screen.go_to(rule + 1);
            self.pulse_line(screen);
        }
        self.foot(screen, key::HINT);
    }

    /// When this task will next be considered, or the word for why it will not.
    fn moment_of(&self, task: &TaskState) -> Run<Icon> {
        if !task.why_not.is_empty() {
            return Run::label(&format!("brain-why-{}", task.why_not)).quiet();
        }
        if task.standing == Standing::Running {
            return Run::label("brain-standing-running").accent();
        }
        // THE HOST FORMATTED IT. A component has no zone and no calendar, so
        // the record carries the number for the arithmetic this side does and
        // the text for the date it cannot compute.
        if task.next_text.is_empty() {
            return Run::label(key::NEVER).quiet();
        }
        Run::datum(&task.next_text).quiet()
    }

    /// The note beside a task: what is holding it, or how it last ended.
    fn note_of(&self, task: &TaskState) -> Run<Icon> {
        if let Some(word) = hold_word(task.hold) {
            return Run::label(&format!("brain-hold-{word}")).playing(Role::Quiet);
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

    /// What the brain is doing, and how busy the machine is.
    ///
    /// The readings are ICONS and numbers, and no words: a glyph from the token
    /// table says "memory" in every language, and the alternative was three
    /// more keys for three labels nobody reads twice.
    fn pulse_line(&self, screen: &mut Screen<Wit>) {
        let layout = Table::across(screen.columns())
            .with(Column::fill("doing", 10))
            .with(Column::fixed("ram-of", ronoctua_design::ICON, 4))
            .with(Column::fixed("ram", 7, 3))
            .with(Column::fixed("cpu-of", ronoctua_design::ICON, 2))
            .with(Column::fixed("cpu", 7, 1))
            .with(Column::fixed("power", ronoctua_design::ICON, 5))
            .resolve();
        let at = screen.row();
        let doing = if self.mind.running.is_empty() {
            Run::label(&format!(
                "brain-standing-{}",
                if self.mind.queue.is_empty() {
                    "idle"
                } else {
                    "queued"
                }
            ))
            .quiet()
        } else {
            Run::label("brain-standing-running").accent()
        };
        let dash = || Run::datum("—").dim();
        let ram = vital(&self.mind, Vital::RamUsed)
            .map_or_else(dash, |p| Run::datum(&format!("{p}%")).quiet());
        let cpu = vital(&self.mind, Vital::CpuStall)
            .map_or_else(dash, |p| Run::datum(&percent(p)).quiet());
        let mut row = screen.row_of(at, &layout);
        row.put("doing", doing);
        row.put("ram-of", Run::glyph(Icon::Memory).dim());
        row.put("ram", ram);
        row.put("cpu-of", Run::glyph(Icon::Cpu).dim());
        row.put("cpu", cpu);
        // ONLY WHEN IT IS ON BATTERY. A machine with none, and a machine that
        // is plugged in, both have nothing to say here — and a glyph that is
        // always there says nothing when it matters.
        if vital(&self.mind, Vital::OnBattery) == Some(1) {
            row.put("power", Run::glyph(Icon::Battery).accent());
        }
        let tall = row.tall();
        screen.go_to(at + tall);
    }

    /// The page behind: what it runs, when it may, and what happened.
    fn behind(&mut self, screen: &mut Screen<Wit>) {
        let Some(task) = self.here().cloned() else {
            let line = screen.heading(Icon::Brain, key::TITLE);
            screen.go_to(line);
            screen.line(Run::label(key::NOTHING).quiet());
            self.list_top = None;
            return;
        };
        let line = screen.heading(Icon::Sync, &task_key(&task));
        screen.go_to(line);

        self.steps_block(screen, &task);
        self.when_block(screen, &task);
        self.last_block(screen, &task);
        self.foot(screen, key::HINT_UPDATE);
    }

    /// What it runs, in order.
    fn steps_block(&mut self, screen: &mut Screen<Wit>, task: &TaskState) {
        if screen.row() + 2 > screen.lines() {
            self.list_top = None;
            return;
        }
        screen.line(Run::label(key::STEPS).quiet().small());
        let layout = Table::across(screen.columns())
            .with(Column::fixed("mark", ronoctua_design::ICON, 3))
            .with(Column::fill("name", 12))
            .with(Column::fixed("note", 12, 1))
            .resolve();
        self.list_top = Some(screen.row());
        for (n, step) in task.steps.iter().enumerate() {
            if screen.row() + 2 > screen.lines() {
                break;
            }
            let at = screen.row();
            if n == self.step {
                screen.band(at);
            }
            let mut row = screen.row_of(at, &layout);
            // A MARK AND ITS ABSENCE, both from the token table: a tick typed
            // into this file would be a codepoint chosen inside a component
            // that has no font (invariants 9 and 10).
            if step.on {
                row.put("mark", Run::glyph(Icon::Success).accent());
            }
            // `update-<id>` AND NOT `brain-step-<id>`, and the reason is that
            // the ids are not the brain's. They come from
            // `assets/updates.nix`, which already owes every one of them a
            // name and a sentence in both languages — the same keys the
            // sequence itself draws. A second family for the same twelve steps
            // would be twelve more sentences to keep in step with these.
            row.put("name", Run::label(&format!("update-{}", step.id)));
            if step.changed {
                row.put("note", Run::label(key::CHANGED).dim().small());
            }
            let tall = row.tall();
            screen.go_to(at + tall);
            if n == self.step && screen.row() < screen.lines() {
                // THE REAL COMMAND, under the cursor. "The machine will update
                // itself" is a sentence somebody should be able to open, and
                // this is where it opens: what runs, and whether it is root.
                screen.line(Run::datum(&step.command).dim().small());
                if step.needs_root && screen.row() < screen.lines() {
                    screen.line(Run::label(key::NEEDS_ROOT).dim().small());
                }
            }
        }
    }

    /// When it may run.
    fn when_block(&mut self, screen: &mut Screen<Wit>, task: &TaskState) {
        if screen.row() + 4 > screen.lines() {
            return;
        }
        screen.skip(ronoctua_design::AIR);
        let rule = screen.row();
        screen.rule(rule);
        screen.go_to(rule + 1);
        screen.line(Run::label(key::WHEN).quiet().small());

        let layout = field_columns(screen.columns());
        let days = format!("{}", task.interval_days);
        self.field_of(
            screen,
            &layout,
            key::EVERY,
            Run::datum(&days),
            Some(key::DAYS),
        );
        // The unit as a KEY beside the number as DATA, which is the pattern the
        // memory panel already uses: `7` is a number in every language and
        // "days" is not.
        self.field(
            screen,
            &layout,
            key::PART,
            Run::label(&format!("brain-part-{}", part_word(task.part_of_day))),
        );
        if !task.next_text.is_empty() {
            self.field(screen, &layout, key::NEXT, Run::datum(&task.next_text));
        }
        if let Some(word) = hold_word(task.hold) {
            self.field(
                screen,
                &layout,
                key::WAITING,
                Run::label(&format!("brain-hold-{word}")).accent(),
            );
        }
        // The warning, while it is in flight: the deadline crossed once, and
        // the clock is this side's own arithmetic. The brain does not stream a
        // countdown, because whoever draws already beats a second.
        if self.mind.announcement.task == task.id {
            let left = self
                .mind
                .announcement
                .deadline
                .saturating_sub(self.mind.now);
            self.field(
                screen,
                &layout,
                key::SOON,
                Run::datum(&clock(left)).accent(),
            );
        }
        if self.mind.rehearsing == task.id {
            self.field(
                screen,
                &layout,
                key::DOING,
                Run::label(key::REHEARSAL).accent(),
            );
        }
    }

    /// What happened last time.
    fn last_block(&mut self, screen: &mut Screen<Wit>, task: &TaskState) {
        if screen.row() + 3 > screen.lines() {
            return;
        }
        screen.skip(ronoctua_design::AIR);
        let rule = screen.row();
        screen.rule(rule);
        screen.go_to(rule + 1);
        screen.line(Run::label(key::LAST).quiet().small());

        let layout = field_columns(screen.columns());
        if !task.ran {
            screen.line(Run::label(key::NEVER).quiet());
            return;
        }
        let run = &task.last;
        self.field(
            screen,
            &layout,
            key::LAST,
            Run::datum(&run.when_text).quiet(),
        );
        self.field(
            screen,
            &layout,
            key::OUTCOME,
            Run::label(&format!("brain-outcome-{}", outcome_word(run.outcome))),
        );
        if !run.at.is_empty() {
            self.field(
                screen,
                &layout,
                key::FAILED_AT,
                // THE SAME FAMILY, and this row is why it matters. `run.at`
                // is an id out of the report the UNIT wrote, so it is one of
                // the table's twelve whatever the brain believes. Built as
                // `brain-step-…` it resolved to nothing and drew a bracketed
                // key exactly where the reason for a failure belongs.
                Run::label(&format!("update-{}", run.at)),
            );
        }
        // A WORD OF THE BRAIN'S OWN and a sentence from the machine are
        // different fields, and they are drawn differently: one goes through
        // the bundle, the other comes out exactly as it arrived. Putting both
        // in one field is how `brain-why-runaway` ends up on screen in
        // brackets.
        if !run.why.is_empty() {
            self.field(
                screen,
                &layout,
                key::WAITING,
                Run::label(&format!("brain-why-{}", run.why)),
            );
        }
        if !run.trouble.is_empty() && screen.row() < screen.lines() {
            screen.line(Run::datum(&run.trouble).dim().small());
        }
    }

    /// A label and its value, on one row.
    fn field(&self, screen: &mut Screen<Wit>, layout: &Layout, label: &str, value: Run<Icon>) {
        self.field_of(screen, layout, label, value, None);
    }

    /// The same, with a unit after the value.
    fn field_of(
        &self,
        screen: &mut Screen<Wit>,
        layout: &Layout,
        label: &str,
        value: Run<Icon>,
        unit: Option<&str>,
    ) {
        if screen.row() >= screen.lines() {
            return;
        }
        let at = screen.row();
        let mut row = screen.row_of(at, layout);
        row.put("label", Run::label(label).dim());
        row.put("value", value);
        if let Some(unit) = unit {
            row.put("unit", Run::label(unit).dim());
        }
        let tall = row.tall();
        screen.go_to(at + tall);
    }

    /// The last line: what the keys do.
    ///
    /// On the LAST row of the window and not after the content, so it is in the
    /// same place on every page — a hint that moves is a hint nobody learns
    /// where to look for.
    fn foot(&self, screen: &mut Screen<Wit>, hint: &str) {
        let last = screen.lines().saturating_sub(1);
        if screen.row() >= last {
            return;
        }
        screen.put(last, Run::label(hint).dim().small());
    }
}

// ---------------------------------------------------------------------------
// What the keys do.
// ---------------------------------------------------------------------------

impl State {
    /// Which of the declared states this is, right now.
    ///
    /// DERIVED and never stored: `page` already answers it, and a stored copy
    /// is a second truth that drifts on the first path that forgets it.
    const fn state_id(&self) -> &'static str {
        match self.page {
            Page::Home => "normal",
            Page::Behind => "update",
        }
    }

    /// A verb, aimed at the task under the cursor.
    ///
    /// The target is a **1-based index into the task list of the last mind** —
    /// never a name this side built. The host checks it against the list it
    /// just sent, which is the same rule the disk rack and the transfer queue
    /// already follow.
    fn verb(&self, what: Request) -> Vec<Effect> {
        if self.mind.tasks.is_empty() {
            return Vec::new();
        }
        vec![Effect::Ask(Demand {
            what,
            target: self.task as u64 + 1,
        })]
    }

    /// A preference, aimed at the task under the cursor.
    fn tune(&self, knob: Knob) -> Vec<Effect> {
        let Some(task) = self.here() else {
            return Vec::new();
        };
        vec![Effect::Tune(Setting {
            task: task.id.clone(),
            knob,
        })]
    }

    /// The next rung of the cadence, out of the ladder the HOST sent.
    ///
    /// The ladder is not this side's: a step from thirty days to twenty-nine
    /// changes nothing anybody can feel, and which steps are worth having is a
    /// policy that lives beside its reason in `ronoctua-brain`. The screen only
    /// walks a list it was given — the same rule as naming an id it was given.
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

    /// What a command id does.
    fn command(&mut self, id: &str) -> Vec<Effect> {
        match id {
            "brain-run" => self.verb(Request::RunTask),
            "brain-rehearse" => self.verb(Request::RehearseTask),
            "brain-abort" => self.verb(Request::AbortTask),
            "brain-proceed" => self.verb(Request::Proceed),
            "brain-toggle-step" => {
                let Some(step) = self.here().and_then(|t| t.steps.get(self.step)) else {
                    return Vec::new();
                };
                let switch = StepSwitch {
                    id: step.id.clone(),
                    on: !step.on,
                };
                self.tune(Knob::Step(switch))
            }
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
            _ => Vec::new(),
        }
    }

    /// A new mind arrived.
    ///
    /// A method and not a branch inside `event`, so the tests can drive it
    /// without a thread-local — and so the "keep the cursor inside the list"
    /// rule has ONE place instead of one in the code and a copy in the test.
    ///
    /// It always redraws. The memory panel compares first because a survey
    /// arrives every second; this one arrives when something changed and, at
    /// worst, on the brain's half-minute tick — a frame every thirty seconds is
    /// not worth the comparison, and the generated records carry no `PartialEq`
    /// to compare with.
    fn received(&mut self, mind: Mind) -> Vec<Effect> {
        self.mind = mind;
        // The list can shrink under the cursor: a task that was switched off is
        // still listed, but a rename or a removal is not.
        if self.task >= self.mind.tasks.len() {
            self.task = self.mind.tasks.len().saturating_sub(1);
        }
        let steps = self.here().map_or(0, |t| t.steps.len());
        if self.step >= steps {
            self.step = steps.saturating_sub(1);
        }
        vec![Effect::Redraw]
    }

    /// How many rows the cursor may walk on this page.
    fn walkable(&self) -> usize {
        match self.page {
            Page::Home => self.mind.tasks.len(),
            Page::Behind => self.here().map_or(0, |t| t.steps.len()),
        }
    }

    /// Where the cursor is on this page.
    const fn cursor(&mut self) -> &mut usize {
        match self.page {
            Page::Home => &mut self.task,
            Page::Behind => &mut self.step,
        }
    }

    /// One input, before the state is compared.
    fn act(&mut self, e: Input) -> Vec<Effect> {
        match e {
            // THE HOST SENDS AN ID THIS BUFFER DECLARED. Landing somewhere
            // predictable matters: a page entered by name has to start at its
            // first row, not wherever the cursor happened to be left.
            Input::EnterState(id) => {
                self.page = if id == "update" {
                    Page::Behind
                } else {
                    Page::Home
                };
                self.step = 0;
                vec![Effect::Redraw]
            }
            Input::Mind(mind) => self.received(mind),
            Input::Command(id) => self.command(&id),
            Input::Down => {
                let last = self.walkable().saturating_sub(1);
                let at = self.cursor();
                if *at < last {
                    *at += 1;
                }
                vec![Effect::Redraw]
            }
            Input::Up => {
                let at = self.cursor();
                *at = at.saturating_sub(1);
                vec![Effect::Redraw]
            }
            // `Enter` on a task opens its page. It is the one thing the owner
            // asked for by name: the home page says one or two lines about the
            // update, and choosing that line is what opens the rest.
            Input::Confirm => {
                if self.page == Page::Home && self.here().is_some() {
                    self.page = Page::Behind;
                    self.step = 0;
                    return vec![Effect::Redraw];
                }
                Vec::new()
            }
            // `Esc` walks back ONE level, and does not close: `Effect::Close`
            // exists and stays in the protocol, but tying it to the key one
            // presses by reflex to leave anything at all would destroy the
            // buffer.
            Input::Cancel => {
                if self.page == Page::Home {
                    return Vec::new();
                }
                self.page = Page::Home;
                vec![Effect::Redraw]
            }
            // A click puts the cursor on the row that was clicked, when there
            // is one there: the buffer knows its own layout, and the host does
            // not get to guess it. A click does what the key does and nothing
            // more (invariant 14) — it moves the cursor, it does not act.
            Input::Click(p) => {
                let Some(first) = self.list_top else {
                    return Vec::new();
                };
                if p.line >= first {
                    let n = usize::from(p.line - first);
                    if n < self.walkable() {
                        *self.cursor() = n;
                    }
                }
                vec![Effect::Redraw]
            }
            // WHAT THIS BUFFER DECLARED NO CAPABILITY FOR, and the keys it has
            // no use for. The `match` is exhaustive and the WIT is a contract
            // shared by eight components: an `input` case added for one of them
            // has to be answered by all, and here "answered" is "ignored".
            Input::Character(_)
            | Input::Left
            | Input::Right
            | Input::Delete
            | Input::Toggle
            | Input::Copied(_)
            | Input::Paste(_)
            | Input::Survey(_)
            | Input::Queue(_)
            | Input::Rack(_)
            | Input::Formatted(_) => Vec::new(),
        }
    }
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "brain".into(),
            mode: "brain".into(),
            // TWO STATES: the home page, and the update's own. The first is
            // `normal` — the host refuses a descriptor whose base is called
            // anything else, because the colour rule reads it.
            states: vec![
                BufferState {
                    id: "normal".into(),
                    letter: 'n',
                    tone: Tone::Standard,
                },
                BufferState {
                    id: "update".into(),
                    letter: 'u',
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
        }
    }

    fn render(ctx: Context) -> Vec<ronoctua_design::types::Command> {
        STATE.with_borrow_mut(|s| {
            s.lines = ctx.lines;
            let mut screen = Screen::<Wit>::of(&ctx);
            match s.page {
                Page::Home => s.home(&mut screen),
                Page::Behind => s.behind(&mut screen),
            }
            screen.finish_wit()
        })
    }

    fn event(e: Input) -> Vec<Effect> {
        // THE STATE IS REPORTED FROM ONE PLACE, and not at each site that
        // assigns `page` — there are three, between `Enter`, `Esc` and the
        // host's own `enter-state`. A report per site is a report somebody
        // forgets.
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

ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_design::types::{
        Announcement, DayPart, Gate, Pulse, RunRecord, TaskStep, VitalSign,
    };

    fn a_run() -> RunRecord {
        RunRecord {
            began: 100,
            ended: 200,
            when_text: "2026-08-15 13:00".into(),
            outcome: OutcomeKind::Done,
            at: String::new(),
            why: String::new(),
            trouble: String::new(),
            steps: Vec::new(),
        }
    }

    fn a_task() -> TaskState {
        TaskState {
            id: "system-update".into(),
            standing: Standing::Idle,
            enabled: true,
            interval_days: 7,
            part_of_day: DayPart::Afternoon,
            next: 1_700_000_000,
            next_text: "2026-08-23 13:00".into(),
            hold: HoldKind::None,
            why_not: String::new(),
            ran: true,
            last: a_run(),
            steps: vec![
                TaskStep {
                    id: "update-inputs".into(),
                    command: "nix flake update --flake /etc/nixos".into(),
                    needs_root: true,
                    on: true,
                    changed: false,
                },
                TaskStep {
                    id: "build".into(),
                    command: "nixos-rebuild boot --flake /etc/nixos".into(),
                    needs_root: true,
                    on: true,
                    changed: false,
                },
            ],
        }
    }

    fn a_mind() -> Mind {
        Mind {
            now: 1_699_999_000,
            pulse: Pulse {
                at: 1_699_999_000,
                signs: vec![VitalSign {
                    of: Vital::RamUsed,
                    amount: 41,
                }],
            },
            tasks: vec![a_task()],
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
            cadence: vec![1, 2, 3, 7, 14, 30],
        }
    }

    fn a_state() -> State {
        let mut s = State::fresh();
        s.received(a_mind());
        s
    }

    fn a_context(lines: u16) -> Context {
        Context {
            columns: 90,
            lines,
            heights: ronoctua_design::types::Heights {
                small: 1,
                base: 1,
                normal: 1,
                large: 2,
                huge: 3,
            },
        }
    }

    #[test]
    fn every_declared_command_has_an_arm() {
        // THE DEFECT THIS CATCHES: a key in the descriptor that the `match`
        // does not answer. It binds, it dispatches, and nothing happens — the
        // one failure a person cannot tell from "the feature is broken".
        let mut s = a_state();
        for (_, id) in KEYMAP {
            assert!(
                !s.command(id).is_empty(),
                "`{id}` is in the keymap and does nothing"
            );
        }
        // And something that is not ours is ignored rather than guessed at.
        assert!(s.command("brain-does-not-exist").is_empty());
    }

    #[test]
    fn enter_opens_the_page_behind_and_escape_walks_back_one_level() {
        let mut s = a_state();
        assert_eq!(s.state_id(), "normal");
        s.act(Input::Confirm);
        assert_eq!(
            s.state_id(),
            "update",
            "`Enter` on the row did not open the page"
        );
        s.act(Input::Cancel);
        assert_eq!(s.state_id(), "normal", "`Esc` did not go back one level");
        // And on the home page `Esc` does nothing at all: the buffer is the
        // last level, and closing it is not what that key means.
        assert!(s.act(Input::Cancel).is_empty());
    }

    #[test]
    fn every_state_is_reachable_by_name() {
        // The host sends an id the descriptor declared — `SPC b s`, or the
        // modeline clicked. A state that could only be reached by a key would
        // leave the modeline offering a screen nobody enters.
        let mut s = a_state();
        for id in ["update", "normal"] {
            s.act(Input::EnterState(id.into()));
            assert_eq!(s.state_id(), id);
        }
    }

    #[test]
    fn the_step_under_the_cursor_is_the_one_that_toggles() {
        let mut s = a_state();
        s.act(Input::Confirm);
        s.act(Input::Down);
        let out = s.command("brain-toggle-step");
        match out.as_slice() {
            [Effect::Tune(Setting { task, knob })] => {
                assert_eq!(task, "system-update");
                match knob {
                    Knob::Step(switch) => {
                        assert_eq!(switch.id, "build", "it touched the wrong step");
                        assert!(!switch.on, "it switched on a step that was already on");
                    }
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_cadence_walks_the_ladder_the_host_sent() {
        // THE LADDER IS NOT THIS SIDE'S. A screen that stepped by one would
        // take twenty-three key presses to get from a month to a week, and the
        // steps that are worth having are a policy with its reason written in
        // `ronoctua-brain`.
        let s = a_state();
        assert_eq!(s.next_cadence(7), 14);
        assert_eq!(
            s.next_cadence(30),
            1,
            "the top of the ladder did not return to the start"
        );
        // A value nobody put on the ladder climbs to the next rung above it.
        assert_eq!(s.next_cadence(9), 14);
        // And with no ladder at all nothing moves, instead of a number this
        // side invented.
        let mut bare = State::fresh();
        bare.received(Mind {
            cadence: Vec::new(),
            ..a_mind()
        });
        assert_eq!(bare.next_cadence(7), 7);
    }

    #[test]
    fn a_verb_names_the_row_by_index_and_the_index_is_one_based() {
        // `matches!` and not `assert_eq!`: the generated types carry no
        // `PartialEq` — `ronoctua-design` owns the one `generate!` and does not
        // ask for it — so what a test compares is the fields it cares about.
        let s = a_state();
        match s.verb(Request::RunTask).as_slice() {
            [Effect::Ask(d)] => {
                assert_eq!(d.target, 1, "the line is not 1-based");
                assert!(matches!(d.what, Request::RunTask));
            }
            other => panic!("{} effects", other.len()),
        }
        // With no tasks at all there is nothing to aim at, and nothing is sent:
        // a target of zero would be a row the host has to guess about.
        let empty = State::fresh();
        assert!(empty.verb(Request::RunTask).is_empty());
    }

    #[test]
    fn the_cursor_does_not_outlive_the_list() {
        let mut s = a_state();
        s.act(Input::Confirm);
        s.act(Input::Down);
        assert_eq!(s.step, 1);
        // A mind whose task lost a step: the cursor comes back with it.
        let mut smaller = a_mind();
        smaller.tasks[0].steps.truncate(1);
        s.received(smaller);
        assert_eq!(s.step, 0);
        // And no tasks at all leaves nothing to point at.
        s.received(Mind {
            tasks: Vec::new(),
            ..a_mind()
        });
        assert_eq!(s.task, 0);
    }

    #[test]
    fn nothing_is_drawn_outside_the_window() {
        // THE ONE THING THAT NEVER GIVES WAY. The footer's ceiling is twelve
        // rows and the window's is whatever it is; a screen that drew past
        // either would be the overlap `DisplayList::nothing_overlaps` exists to
        // catch, on somebody's real display.
        for lines in [6_u16, 12, 24, 40] {
            for page in [Page::Home, Page::Behind] {
                let mut s = a_state();
                s.page = page;
                let ctx = a_context(lines);
                let mut screen = Screen::<Wit>::of(&ctx);
                match page {
                    Page::Home => s.home(&mut screen),
                    Page::Behind => s.behind(&mut screen),
                }
                // `as_protocol` is the geometry shadow the design crate
                // describes: the colours are real token names and the icons
                // are the enum's `Debug` spelling, which is exactly enough for
                // the one question asked here — how tall did this come out.
                // The overlap and coverage checks are the HOST's, at load, and
                // they take `ronoctua_protocol`'s own `Heights`, which a
                // component cannot name.
                let list = screen.as_protocol();
                assert!(
                    list.height() <= lines,
                    "{page:?} drew {} rows in {lines}",
                    list.height()
                );
            }
        }
    }

    #[test]
    fn a_reading_the_machine_never_took_is_a_dash_and_not_a_zero() {
        // A processor with no PSI reports nothing, and a screen that drew `0%`
        // would say the machine is idle when nobody ever looked.
        let s = a_state();
        assert_eq!(vital(&s.mind, Vital::RamUsed), Some(41));
        assert_eq!(vital(&s.mind, Vital::CpuStall), None);
    }

    #[test]
    fn a_clock_reads_the_same_in_every_language() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(75), "01:15");
        assert_eq!(percent(123), "12.3%");
    }
}
