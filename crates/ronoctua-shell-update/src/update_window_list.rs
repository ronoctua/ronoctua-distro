//! THE UPDATE'S LIST, in both doors: what is drawn under the status — what
//! the machine wants said ([`Section::Notices`]), what one does here
//! ([`Section::Do`]), the choices that hold for every update
//! ([`Section::Options`]) and the steps ([`Section::Steps`]) — and what each
//! thing on it is, said whole where the door explains ([`View::explained`]).
//!
//! # One builder
//!
//! [`View::list`] is the one answer the window's painting, the window's
//! pointer and the footer's region read: a row is drawn, lit and clicked
//! from the same [`ListRow`], and what a row IS ([`Item`]) is its identity —
//! never its place, for rows come and go with what the machine says. The
//! keys walk the same rows ([`View::walkable`]), from the same plan.
//!
//! # Nothing is explained inside the list
//!
//! The owner's first complaint: the sentence of the thing under the cursor
//! sat in the middle of the list and pushed everything under it down as the
//! cursor moved. A row says what it is in a few words; what it does, why it
//! is off and what a click will do is said where the door explains — the
//! window's foot, the footer's explanation line — so every row is the same
//! wherever the cursor is.
//!
//! # The second file outside the module that names it
//!
//! Listed in the module's `namedBy` beside the view: the steps' rows, the
//! schedule's words and every sentence of the list are the module's.

use crate::door;
use crate::key;
use crate::update_window::{Action, Cost, Run, Status, Verdict, View, clock_of, step_of};
use ronoctua_brain::compiled::system_update as module;
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

/// The gap between one column of a row and the next — the view's own
/// (`update_window`'s), for the same reason.
const COLUMN_GAP: u16 = tokens::space::MODELINE_SEGMENTGAP;

/// The cells a mark takes at the head of what it marks: a step's box, a
/// verdict's glyph, the way back — the glyph and its cell of air.
const MARK_CELLS: u16 = tokens::space::UPDATE_BOXCELLS;

/// The four parts of the list, in their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    /// What the machine's records want said — only when there is something.
    Notices,
    /// What one does here: update, try it, when it happens by itself.
    Do,
    /// The choices that hold for every update, each a page.
    Options,
    /// What an update does, step by step, each switched on or off.
    Steps,
}

impl Section {
    /// Every one, in the list's order.
    pub const ALL: [Self; 4] = [Self::Notices, Self::Do, Self::Options, Self::Steps];

    /// Its heading's key.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Notices => module::key::SYSTEM_UPDATE_SECTION_NOTICES,
            Self::Do => module::key::SYSTEM_UPDATE_SECTION_DO,
            Self::Options => module::key::SYSTEM_UPDATE_SECTION_OPTIONS,
            Self::Steps => module::key::SYSTEM_UPDATE_SECTION_STEPS,
        }
    }

    /// What it is, for somebody who has not met it: said where the door
    /// explains when the pointer rests on its heading.
    #[must_use]
    pub const fn about(self) -> &'static str {
        match self {
            Self::Notices => module::key::SYSTEM_UPDATE_SECTION_NOTICES_ABOUT,
            Self::Do => module::key::SYSTEM_UPDATE_SECTION_DO_ABOUT,
            Self::Options => module::key::SYSTEM_UPDATE_SECTION_OPTIONS_ABOUT,
            Self::Steps => module::key::SYSTEM_UPDATE_SECTION_STEPS_ABOUT,
        }
    }
}

/// A page the view turns to, in place of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageOf {
    /// The person's binary cache (`crate::binary_cache`).
    Cache,
    /// The distro's apps that follow their releases (`crate::follow`).
    Follow,
    /// The machine's parts (`crate::parts`).
    Parts,
    /// What the last update wrote in the journals.
    Output,
}

impl PageOf {
    /// Every one, in the order the options list them.
    pub const ALL: [Self; 4] = [Self::Cache, Self::Follow, Self::Parts, Self::Output];

    /// The id of the key that turns to it — whose short word labels its row.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cache => crate::binary_cache::OPEN,
            Self::Follow => crate::follow::OPEN,
            Self::Parts => crate::parts::OPEN,
            Self::Output => crate::update_window::OUTPUT_OPEN,
        }
    }

    /// What it is, said where the door explains.
    const fn does(self) -> &'static str {
        match self {
            Self::Cache => key::BINARY_CACHE_DOES,
            Self::Follow => key::DISTRO_FOLLOW_DOES,
            Self::Parts => key::DISTRO_PARTS_DOES,
            Self::Output => module::key::SYSTEM_UPDATE_OUTPUT_DOES,
        }
    }
}

/// Which question an answer answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Answering {
    /// The run's: a battery below the floor, a metered way out.
    Run,
    /// A gesture of the machine's (`crate::machine_gesture`).
    Gesture,
    /// An approval of a brain module of the person's (`crate::approval`).
    Approval,
    /// The binary cache page's grave one: forget the token, remove the
    /// cache.
    Grave,
}

/// WHAT A ROW IS — its identity, which the cursor, the pointer, a click and
/// the door's explanation all name it by. Never its place: rows come and go
/// with what the machine says.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    /// The status line, while it says GitHub's answer: the second click
    /// asks again.
    Status,
    /// The belongings' conflicts: the second click opens them.
    Belongings,
    /// A gesture of the machine's: a click asks it, the no first.
    Gesture(crate::machine_gesture::Gesture),
    /// A brain module of the person's that waits, by its id: a click asks
    /// to approve it, the no first.
    Approve(String),
    /// One of the things one does here.
    Action(Action),
    /// When the update happens by itself: the second click opens the brain
    /// on its task.
    Schedule,
    /// A page the second click turns the view to.
    Page(PageOf),
    /// A step, by its id: the second click (or `RET`) switches it, and a
    /// click on its box switches it at once.
    Step(&'static str),
    /// The way back from a page.
    Back,
    /// An answer to a question — `true` is yes.
    Answer(Answering, bool),
    /// A row of the page the view is turned to, by the page's own index.
    Row(usize),
    /// A section's heading: nothing to click, and something to explain.
    Heading(Section),
    /// The live output under the list, while a run goes: nothing to click,
    /// and something to explain — what it is, and why the administrator's
    /// half is missing from it when this person may not read it.
    Output,
}

impl Item {
    /// What the door's explanation is about, as a word: another subject
    /// opens the explanation at its first line.
    #[must_use]
    pub fn subject(&self) -> String {
        format!("{self:?}")
    }

    /// Whether what it does answers a run — skips its countdown, puts it
    /// off, stops it: done only where the person put the focus, never where
    /// it rests by itself.
    #[must_use]
    pub const fn changes_the_run(&self) -> bool {
        matches!(
            self,
            Self::Action(Action::StartNow | Action::NotNow | Action::Stop)
        )
    }
}

/// One row of the list, placed and coloured: what it draws, what it is,
/// whether a click takes it, and the cells of its box when it has one.
#[derive(Debug, Clone, PartialEq)]
pub struct ListRow {
    /// Its pieces, already at their cells.
    pub fields: Vec<crate::fields::Field>,
    /// What it is — `None` for air, a fact and a question's sentence.
    pub item: Option<Item>,
    /// Whether a click takes it — and so whether the pointer lights it.
    pub takes: bool,
    /// The cells `[from, to)` of a step's box, which a click there takes on
    /// its own: it switches the step at once.
    pub boxed: Option<(u16, u16)>,
    /// The part of the list it is in.
    pub section: Option<Section>,
    /// It continues the row above it — a sentence wrapped over several
    /// lines, one control.
    pub continues: bool,
}

impl ListRow {
    /// A row of these pieces, about nothing.
    fn plain(fields: Vec<crate::fields::Field>, section: Option<Section>) -> Self {
        Self {
            fields,
            item: None,
            takes: false,
            boxed: None,
            section,
            continues: false,
        }
    }

    /// A line of air.
    pub(crate) const fn plain_air() -> Self {
        Self {
            fields: crate::chrome::AIR,
            item: None,
            takes: false,
            boxed: None,
            section: None,
            continues: false,
        }
    }

    /// One line of output, `room` cells from `left`.
    pub(crate) fn of_output(line: &crate::output::OutputLine, left: u16, room: u16) -> Self {
        Self::plain(output_fields(line, left, room), None)
    }

    /// Whether cell `cell` is in its box.
    #[must_use]
    pub fn on_the_box(&self, cell: u16) -> bool {
        self.boxed
            .is_some_and(|(from, to)| (from..to).contains(&cell))
    }
}

/// One thing the plan lays out.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Slot {
    /// A section's heading.
    Heading(Section),
    /// A row about `item`, which a click takes when `takes`.
    Thing { item: Item, takes: bool },
    /// A sentence of the machine's records.
    Fact(Fact),
    /// A question's sentence.
    Asked(Answering),
    /// One of its answers.
    Answer(Answering, bool),
}

/// A sentence of the machine's records that takes no click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fact {
    /// A new system waits for the next restart; its trial guards it or not.
    Staged(bool),
    /// The boot trial's outcome.
    Trial(module::TrialOutcome),
    /// The failed trial's system is tried again at the next update.
    TriedAgain,
    /// Why the last apply armed no trial.
    NoTrial(module::NoTrial),
    /// The revision the machine is held at, and since when.
    Held,
    /// The machine follows a local checkout of the distro.
    Checkout,
    /// A module of the person's waits, by its place among them.
    Waiting(usize),
    /// A module of the person's is approved as it is shown.
    Approved(usize),
}

/// How many hex digits of a revision are drawn — git's own short form.
const REV_SHOWN: usize = 7;

/// The kind of a row that acts: a setting while a click takes it, refusing
/// while a question holds the cursor.
const fn inert_or(takes: bool) -> crate::chrome::Kind {
    if takes {
        crate::chrome::Kind::Setting
    } else {
        crate::chrome::Kind::Disabled
    }
}

/// A piece at `at`, in `color`.
fn field(at: u16, text: String, color: tokens::Color) -> crate::fields::Field {
    crate::fields::Field {
        at,
        text,
        color,
        hit: crate::fields::Hit::Nothing,
        explain: None,
        ink: crate::fields::Ink::Grid,
    }
}

/// The machine's own sentence of a boot trial's outcome.
const fn trial_key(outcome: module::TrialOutcome) -> &'static str {
    match outcome {
        module::TrialOutcome::Pending => key::MACHINE_TRIAL_PENDING,
        module::TrialOutcome::Good => key::MACHINE_TRIAL_GOOD,
        module::TrialOutcome::Failed => key::MACHINE_TRIAL_FAILED,
        module::TrialOutcome::BootIdentical => key::MACHINE_TRIAL_BOOT_IDENTICAL,
    }
}

/// The machine's own sentence of why an apply armed no trial.
const fn no_trial_key(why: module::NoTrial) -> &'static str {
    match why {
        module::NoTrial::NoEfiVariables => key::MACHINE_NO_TRIAL_NO_EFI_VARIABLES,
        module::NoTrial::NotInMenu => key::MACHINE_NO_TRIAL_NOT_IN_MENU,
        module::NoTrial::NoLastGood => key::MACHINE_NO_TRIAL_NO_LAST_GOOD,
    }
}

/// The short name of a gesture of the machine's, said where the door
/// explains it and on its option's row.
const fn gesture_name(what: crate::machine_gesture::Gesture) -> &'static str {
    use crate::machine_gesture::Gesture;
    match what {
        Gesture::TryAgain => module::key::SYSTEM_UPDATE_GESTURE_TRY_AGAIN_NAME,
        Gesture::Hold => module::key::SYSTEM_UPDATE_GESTURE_HOLD_NAME,
        Gesture::LetGo => module::key::SYSTEM_UPDATE_GESTURE_LET_GO_NAME,
    }
}

/// Who decided which way a step is, as the few words its row says: a step
/// that is off always says who turned it off; one that is on says it only
/// when somebody other than the distro turned it on.
const fn why_short(why: ronoctua_brain::StepWhy) -> Option<&'static str> {
    use ronoctua_brain::StepWhy;
    match why {
        StepWhy::Shipped => None,
        StepWhy::OnHere => Some(module::key::SYSTEM_UPDATE_STEP_ON_HERE),
        StepWhy::OnByYou => Some(module::key::SYSTEM_UPDATE_STEP_ON_BY_YOU),
        StepWhy::ShippedOff => Some(module::key::SYSTEM_UPDATE_STEP_OFF_SHIPPED),
        StepWhy::OffHere => Some(module::key::SYSTEM_UPDATE_STEP_OFF_HERE),
        StepWhy::OffByYou => Some(module::key::SYSTEM_UPDATE_STEP_OFF_BY_YOU),
    }
}

/// The same, whole, where the door explains the step.
const fn why_said(why: ronoctua_brain::StepWhy) -> Option<&'static str> {
    use ronoctua_brain::StepWhy;
    match why {
        StepWhy::Shipped => None,
        StepWhy::OnHere => Some(module::key::SYSTEM_UPDATE_SWITCH_WHY_ON_HERE),
        StepWhy::OnByYou => Some(module::key::SYSTEM_UPDATE_SWITCH_WHY_ON_BY_YOU),
        StepWhy::ShippedOff => Some(module::key::SYSTEM_UPDATE_SWITCH_WHY_SHIPPED_OFF),
        StepWhy::OffHere => Some(module::key::SYSTEM_UPDATE_SWITCH_WHY_OFF_HERE),
        StepWhy::OffByYou => Some(module::key::SYSTEM_UPDATE_SWITCH_WHY_OFF_BY_YOU),
    }
}

/// What the brain says of the update's calendar — the Schedule row's value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    /// The person left it switched on.
    pub enabled: bool,
    /// Days between runs.
    pub interval_days: u16,
    /// Which part of the day it prefers.
    pub part: ronoctua_brain::PartOfDay,
    /// Its next moment, in Unix seconds; `0` when there is none.
    pub next: i64,
    /// Why it is not due, as the brain's word — empty when it is.
    pub why_not: String,
    /// The key of that word.
    pub why_key: String,
    /// The brain's clock when it said so.
    pub now: i64,
}

impl Schedule {
    /// What `mind` says of the update's calendar, when it has the task.
    #[must_use]
    pub fn of(mind: &ronoctua_brain::Mind) -> Option<Self> {
        mind.task(module::ID).map(|t| Self {
            enabled: t.enabled,
            interval_days: t.interval_days,
            part: t.part_of_day,
            next: t.next,
            why_not: t.why_not.clone(),
            why_key: t.why_key.clone(),
            now: mind.now,
        })
    }
}

/// The word the brain switches a task off with (`hub::due_of`).
const SWITCHED_OFF: &str = "off";

/// The word of a brain that starts nothing by itself (`hub::row_of`).
const QUIET: &str = "quiet";

/// The local calendar day of `secs`, counted from the epoch.
fn day_of(secs: i64, zone: &ronoctua_files::when::Zone) -> i64 {
    let c = ronoctua_files::when::civil(secs, zone);
    ronoctua_files::when::epoch_of(c.year, c.month, c.day).unwrap_or(0)
}

/// The day of the week of a day counted from the epoch, Sunday `0`: the
/// epoch was a Thursday.
const fn weekday_of(day: i64) -> i64 {
    (day + 4).rem_euclid(7)
}

impl View {
    // -----------------------------------------------------------------------
    // THE PLAN — what is laid out, and in what order; the list draws it and
    // the keys walk it
    // -----------------------------------------------------------------------

    /// The list as slots, each with its section, in order.
    fn plan(&self) -> Vec<(Section, Slot)> {
        let mut out = Vec::new();
        let notices = self.notices();
        if !notices.is_empty() {
            out.push((Section::Notices, Slot::Heading(Section::Notices)));
            out.extend(notices.into_iter().map(|s| (Section::Notices, s)));
        }
        out.push((Section::Do, Slot::Heading(Section::Do)));
        let actions: &[Action] = match self.run() {
            Run::Idle | Run::Queued => &[Action::Start],
            Run::CountingDown => &[Action::StartNow, Action::NotNow],
            Run::Running => &[Action::Stop],
        };
        for (n, action) in actions.iter().enumerate() {
            out.push((
                Section::Do,
                Slot::Thing {
                    item: Item::Action(*action),
                    takes: !self.questioned(),
                },
            ));
            // THE RUN'S QUESTION, under the row that asked it: the no first.
            if n == 0 && self.asking.is_some() {
                out.push((Section::Do, Slot::Asked(Answering::Run)));
                for yes in [false, true] {
                    out.push((Section::Do, Slot::Answer(Answering::Run, yes)));
                }
            }
        }
        // THE SECTION KEEPS ITS ROWS IN EVERY STATE OF THE RUN, so nothing
        // under it moves when a countdown starts or ends: the countdown's two
        // answers take the place of the rehearsal, which only acts while
        // nothing of the update is under way.
        let rest: &[Item] = if self.run() == Run::CountingDown {
            &[Item::Schedule]
        } else {
            &[Item::Action(Action::Rehearse), Item::Schedule]
        };
        for item in rest.iter().cloned() {
            out.push((
                Section::Do,
                Slot::Thing {
                    item,
                    takes: !self.questioned(),
                },
            ));
        }
        out.push((Section::Options, Slot::Heading(Section::Options)));
        // HOLDING AT THIS VERSION is a choice for every update while the
        // machine is not held; held, letting go is a notice.
        if self.records.hold.is_none() && self.records.revision.is_some() {
            out.extend(
                self.gesture_slots(crate::machine_gesture::Gesture::Hold)
                    .into_iter()
                    .map(|s| (Section::Options, s)),
            );
        }
        for page in PageOf::ALL {
            out.push((
                Section::Options,
                Slot::Thing {
                    item: Item::Page(page),
                    takes: !self.questioned(),
                },
            ));
        }
        out.push((Section::Steps, Slot::Heading(Section::Steps)));
        for st in &self.steps {
            if let Some(def) = step_of(&st.id) {
                out.push((
                    Section::Steps,
                    Slot::Thing {
                        item: Item::Step(def.id),
                        takes: !self.questioned(),
                    },
                ));
            }
        }
        out
    }

    /// What the machine's records say, each only when it is there: the
    /// belongings' conflicts, a system that waits for the restart, the boot
    /// trial and why there is none, a hold and its letting go, a checkout,
    /// and the modules of the person's that wait.
    ///
    /// A RECORD IS A NOTICE WHATEVER THE RUN DOES: the status that says the
    /// restart gives way to a run's, and a notice that came and went with it
    /// would move every row under it.
    fn notices(&self) -> Vec<Slot> {
        use crate::machine_gesture::Gesture;
        let r = &self.records;
        let mut out = Vec::new();
        if r.conflicts > 0 {
            out.push(Slot::Thing {
                item: Item::Belongings,
                takes: !self.questioned(),
            });
        }
        if let Some(next) = &r.staged {
            out.push(Slot::Fact(Fact::Staged(next.guarded)));
        }
        if let Some(outcome) = r.trial {
            out.push(Slot::Fact(Fact::Trial(outcome)));
            if outcome == module::TrialOutcome::Failed {
                if r.again {
                    out.push(Slot::Fact(Fact::TriedAgain));
                } else {
                    out.extend(self.gesture_slots(Gesture::TryAgain));
                }
            }
        }
        if let Some(why) = r.no_trial {
            out.push(Slot::Fact(Fact::NoTrial(why)));
        }
        if r.hold.is_some() {
            out.push(Slot::Fact(Fact::Held));
            out.extend(self.gesture_slots(Gesture::LetGo));
        }
        if r.checkout {
            out.push(Slot::Fact(Fact::Checkout));
        }
        for (n, w) in self.approvals.waiting.iter().enumerate() {
            if self.approvals.approved_as_shown(w) {
                out.push(Slot::Fact(Fact::Approved(n)));
                continue;
            }
            out.push(Slot::Fact(Fact::Waiting(n)));
            if self.approval.as_ref().is_some_and(|q| q.of == *w) {
                out.push(Slot::Asked(Answering::Approval));
                for yes in [false, true] {
                    out.push(Slot::Answer(Answering::Approval, yes));
                }
            } else {
                out.push(Slot::Thing {
                    item: Item::Approve(w.id.clone()),
                    takes: self.quiet(),
                });
            }
        }
        out
    }

    /// A gesture's row — or, while it is asked, its question in its place.
    fn gesture_slots(&self, what: crate::machine_gesture::Gesture) -> Vec<Slot> {
        if self.gesture.as_ref().is_some_and(|q| q.what == what) {
            let mut out = vec![Slot::Asked(Answering::Gesture)];
            for yes in [false, true] {
                out.push(Slot::Answer(Answering::Gesture, yes));
            }
            return out;
        }
        vec![Slot::Thing {
            item: Item::Gesture(what),
            takes: self.gesture_takes(what),
        }]
    }

    /// The things the keys walk, in the list's order: the status while it
    /// says GitHub's answer, then every row a click takes. None while a
    /// question holds the cursor — its answers are its own — or a page is
    /// open, whose rows are the page's.
    #[must_use]
    pub fn walkable(&self) -> Vec<Item> {
        if self.questioned() || self.page_open() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.status().is_online() {
            out.push(Item::Status);
        }
        out.extend(self.plan().into_iter().filter_map(|(_, s)| match s {
            Slot::Thing { item, takes: true } => Some(item),
            _ => None,
        }));
        out
    }

    /// The section each walked thing is in — the status is in none.
    fn walked_sections(&self) -> Vec<(Option<Section>, Item)> {
        let mut out = Vec::new();
        if self.questioned() || self.page_open() {
            return out;
        }
        if self.status().is_online() {
            out.push((None, Item::Status));
        }
        out.extend(self.plan().into_iter().filter_map(|(section, s)| match s {
            Slot::Thing { item, takes: true } => Some((Some(section), item)),
            _ => None,
        }));
        out
    }

    /// What the cursor is on: what the person put it on, while it is there
    /// to walk, else where it rests ([`View::resting`]). `None` while a
    /// question holds the cursor or a page is open.
    #[must_use]
    pub fn focus(&self) -> Option<Item> {
        let walk = self.walkable();
        self.focus
            .clone()
            .filter(|f| walk.contains(f))
            .or_else(|| Self::resting(self, &walk))
    }

    /// Where the cursor rests when the person put it on nothing that is
    /// there: "update now" while it can start, else the status line while it
    /// is a control, else the schedule.
    ///
    /// NEVER AN ANSWER THAT CHANGES A RUN — start now, not now, stop. The
    /// view opens while a run is under way, and a countdown or a run begins
    /// under a resting cursor: landing there by itself, a `RET`, or the
    /// click that only selects, would stop a rebuild the person never chose
    /// to answer.
    fn resting(&self, walk: &[Item]) -> Option<Item> {
        let start = Item::Action(Action::Start);
        [start.clone(), Item::Status, Item::Schedule]
            .into_iter()
            .filter(|i| *i != start || self.acts(Action::Start))
            .find(|i| walk.contains(i))
    }

    /// The row of `rows` (this view's [`View::list`]) the cursor is on: the
    /// answer of a question, the open page's row or its way back, or the
    /// thing the cursor is on — what a door's page follows.
    #[must_use]
    pub fn cursor_row(&self, rows: &[ListRow]) -> Option<usize> {
        if let Some(n) = rows.iter().position(
            |r| matches!(&r.item, Some(Item::Answer(kind, yes)) if self.answer_here(*kind, *yes)),
        ) {
            return Some(n);
        }
        if self.page_open() {
            let wanted = if self.on_the_way_back() {
                Item::Back
            } else {
                Item::Row(self.page_cursor()?)
            };
            return rows.iter().position(|r| r.item.as_ref() == Some(&wanted));
        }
        let here = self.focus()?;
        rows.iter().position(|r| r.item.as_ref() == Some(&here))
    }

    /// Whether the cursor is on `item`.
    fn focused(&self, item: &Item) -> bool {
        self.focus().as_ref() == Some(item)
    }

    /// WHAT THE CURSOR IS ON, whatever holds it: the answer of the question
    /// being asked, the open page's way back or its row, or the thing the
    /// keys walk — what a door explains while no pointer outranks it, and
    /// the item of the row [`View::cursor_row`] finds.
    #[must_use]
    pub fn cursor_item(&self) -> Option<Item> {
        for kind in [
            Answering::Run,
            Answering::Gesture,
            Answering::Approval,
            Answering::Grave,
        ] {
            for yes in [false, true] {
                if self.answer_here(kind, yes) {
                    return Some(Item::Answer(kind, yes));
                }
            }
        }
        if self.page_open() {
            return Some(if self.on_the_way_back() {
                Item::Back
            } else {
                Item::Row(self.page_cursor()?)
            });
        }
        self.focus()
    }

    /// HOW A CLICK TAKES `item` NOW — what a door says the hand will do,
    /// read off what [`View::clicked`] does: a gesture's or a module's row
    /// asks at once; the thing the cursor is on does on the second click
    /// what `RET` does — an answer that changes a run only where the person
    /// put the cursor —; anything else the first click selects.
    #[must_use]
    pub fn gesture_for(&self, item: &Item) -> door::Gesture {
        let here = match item {
            Item::Gesture(_) | Item::Approve(_) => return door::Gesture::AtOnce,
            Item::Answer(kind, yes) => self.answer_here(*kind, *yes),
            Item::Back => self.on_the_way_back(),
            Item::Row(n) => !self.on_the_way_back() && self.page_cursor() == Some(*n),
            other => {
                self.focused(other)
                    && (!other.changes_the_run() || self.focus.as_ref() == Some(other))
            }
        };
        if here {
            door::Gesture::Ret
        } else {
            door::Gesture::Selects
        }
    }

    /// An arrow: one list, from the status to the last step.
    pub(crate) fn walk(&mut self, down: bool) -> crate::update_window::Errand {
        let walk = self.walkable();
        if walk.is_empty() {
            return crate::update_window::Errand::Nothing;
        }
        let at = self
            .focus()
            .and_then(|f| walk.iter().position(|i| *i == f))
            .unwrap_or(0);
        let next = if down {
            (at + 1).min(walk.len() - 1)
        } else {
            at.saturating_sub(1)
        };
        self.focus = walk.get(next).cloned();
        crate::update_window::Errand::Moved
    }

    /// `TAB` (or, `back`, `S-TAB`): the first row of the next section (the
    /// one before), round from the last to the first.
    pub(crate) fn section(&mut self, back: bool) -> crate::update_window::Errand {
        let walked = self.walked_sections();
        let mut groups: Vec<(Option<Section>, Item)> = Vec::new();
        for (section, item) in walked {
            if groups.last().is_none_or(|(s, _)| *s != section) {
                groups.push((section, item));
            }
        }
        if groups.is_empty() {
            return crate::update_window::Errand::Nothing;
        }
        let here = self.focus();
        let section_of = |item: &Item| {
            self.walked_sections()
                .into_iter()
                .find(|(_, i)| i == item)
                .map(|(s, _)| s)
        };
        let at = here
            .as_ref()
            .and_then(section_of)
            .and_then(|s| groups.iter().position(|(g, _)| *g == s))
            .unwrap_or(0);
        let n = groups.len();
        let next = if back { (at + n - 1) % n } else { (at + 1) % n };
        self.focus = Some(groups[next].1.clone());
        crate::update_window::Errand::Moved
    }

    /// `Home`, `End`, `PgUp` and `PgDn`: the first, the last, or `page` rows
    /// up or down — a page's own rows on a page.
    pub fn leap(
        &mut self,
        leap: crate::update_window::Leap,
        page: usize,
    ) -> crate::update_window::Errand {
        use crate::update_window::{Errand, Leap, Output, Press};
        if self.output_open() {
            return Errand::Output(match leap {
                Leap::Up => Output::Up,
                Leap::Down => Output::Down,
                Leap::First => Output::Start,
                Leap::Last => Output::End,
            });
        }
        if self.page_open() {
            let (press, times) = match leap {
                Leap::Up => (Press::Up, page.max(1)),
                Leap::Down => (Press::Down, page.max(1)),
                Leap::First => (Press::Up, usize::MAX),
                Leap::Last => (Press::Down, usize::MAX),
            };
            // A PAGE'S CURSOR STOPS AT ITS ENDS: pressed until it does.
            let mut moved = Errand::Nothing;
            for _ in 0..times.min(self.page_len()) {
                let before = (self.page_cursor(), self.on_the_way_back());
                let _ = self.key(press);
                if (self.page_cursor(), self.on_the_way_back()) == before {
                    break;
                }
                moved = Errand::Moved;
            }
            return moved;
        }
        let walk = self.walkable();
        if walk.is_empty() {
            return Errand::Nothing;
        }
        let at = self
            .focus()
            .and_then(|f| walk.iter().position(|i| *i == f))
            .unwrap_or(0);
        let last = walk.len() - 1;
        let next = match leap {
            Leap::First => 0,
            Leap::Last => last,
            Leap::Up => at.saturating_sub(page.max(1)),
            Leap::Down => (at + page.max(1)).min(last),
        };
        self.focus = walk.get(next).cloned();
        Errand::Moved
    }

    // -----------------------------------------------------------------------
    // THE ROWS
    // -----------------------------------------------------------------------

    /// THE LIST, placed from `left` in `room` cells: on a page, the way back
    /// and the page's rows; else the notices, what to do, the options and
    /// the steps, each section under its heading, air before every heading
    /// but the first. Every row is the same wherever the cursor is, but for
    /// its colour: the cursor is a colour.
    #[must_use]
    pub fn list(&self, left: u16, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<ListRow> {
        if self.page_open() {
            return self.page_list(left, room, texts, 0, usize::MAX);
        }
        let mut out = Vec::new();
        let mut first = true;
        for (section, slot) in self.plan() {
            let at = out.len();
            match slot {
                Slot::Heading(s) => {
                    if !first {
                        out.push(ListRow::plain(crate::chrome::AIR, Some(s)));
                    }
                    first = false;
                    out.push(self.heading_row(s, left, room, texts));
                }
                Slot::Thing { item, takes } => {
                    out.extend(self.item_rows(&item, takes, left, room, texts));
                }
                Slot::Fact(fact) => {
                    let (text, color) = self.fact_said(fact, texts);
                    for line in crate::chrome::note_lines(&text, room) {
                        out.push(ListRow::plain(vec![field(left, line, color)], None));
                    }
                }
                Slot::Asked(kind) => {
                    for line in crate::chrome::note_lines(&self.question_said(kind, texts), room) {
                        out.push(ListRow::plain(
                            vec![field(left, line, tokens::color::QUESTION_ASKED)],
                            None,
                        ));
                    }
                }
                Slot::Answer(kind, yes) => {
                    out.push(self.answer_row(kind, yes, left, room, texts));
                }
            }
            for row in &mut out[at..] {
                row.section = Some(section);
            }
        }
        out
    }

    /// How many rows [`View::list`] has, counted without building a page's
    /// output, which keeps thousands of lines.
    #[must_use]
    pub fn list_len(&self, room: u16, texts: &ronoctua_i18n::Texts) -> usize {
        if self.output_open() {
            return self.above_the_output(0, room, texts).len()
                + self.scrollback.last_run().count();
        }
        self.list(0, room, texts).len()
    }

    /// Rows `skip..skip + take` of [`View::list`] — only those built on the
    /// output's page.
    #[must_use]
    pub fn list_from(
        &self,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
        skip: usize,
        take: usize,
    ) -> Vec<ListRow> {
        if self.output_open() {
            return self.page_list(left, room, texts, skip, take);
        }
        self.list(left, room, texts)
            .into_iter()
            .skip(skip)
            .take(take)
            .collect()
    }

    /// A section's heading: its title — and, over the steps, how many of
    /// them are on ([`View::steps_counted`]).
    fn heading_row(
        &self,
        section: Section,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> ListRow {
        let title = layout::elide_end(&texts.t(section.title()), room);
        let mut fields = vec![field(left, title.clone(), tokens::color::TEXT_HEADING)];
        if section == Section::Steps {
            let at = left + layout::width(&title) + COLUMN_GAP;
            fields.extend(self.steps_counted(at, (left + room).saturating_sub(at), texts));
        }
        ListRow {
            item: Some(Item::Heading(section)),
            ..ListRow::plain(fields, Some(section))
        }
    }

    /// How many steps are on, of all of them — quieter than the heading it
    /// stands beside, from `at` in `room` cells.
    fn steps_counted(
        &self,
        at: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<crate::fields::Field> {
        let on = self.steps.iter().filter(|s| s.on).count();
        let count = texts.t_args(
            module::key::SYSTEM_UPDATE_STEPS_COUNT,
            &[
                (
                    "on",
                    ronoctua_i18n::Arg::Number(i64::try_from(on).unwrap_or(i64::MAX)),
                ),
                (
                    "all",
                    ronoctua_i18n::Arg::Number(i64::try_from(self.steps.len()).unwrap_or(i64::MAX)),
                ),
            ],
        );
        let count = layout::elide_end(&count, room);
        (!count.is_empty()).then(|| field(at, count, tokens::color::TEXT_DIM))
    }

    /// The rows of a thing that is clicked: an action, the schedule, a page,
    /// a step — a label and its value — or a sentence of the records that
    /// acts, wrapped whole.
    fn item_rows(
        &self,
        item: &Item,
        takes: bool,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<ListRow> {
        let focused = takes && self.focused(item);
        let one = |fields| {
            vec![ListRow {
                item: Some(item.clone()),
                takes,
                ..ListRow::plain(fields, None)
            }]
        };
        match item {
            // A ROW A CLICK DOES NOT TAKE NOW — a question holds the cursor —
            // is drawn refusing: what is inert looks inert.
            Item::Action(a) => {
                let (kind, value) = if self.acts(*a) {
                    (inert_or(takes), String::new())
                } else {
                    (
                        crate::chrome::Kind::Disabled,
                        texts.t(self.why_not_short(*a)),
                    )
                };
                one(crate::chrome::field_pair(
                    left,
                    room,
                    &texts.t(a.id_in(self.rehearsal_now())),
                    &value,
                    kind,
                    focused,
                ))
            }
            Item::Schedule => one(self.opening(
                &texts.t(crate::update_window::SCHEDULE),
                &self.schedule_value(texts),
                None,
                inert_or(takes),
                focused,
                (left, room),
            )),
            Item::Page(p) => {
                let (value, tint) = self.glance_of(*p, texts);
                let kind = if *p == PageOf::Output && !self.has_output() {
                    crate::chrome::Kind::Disabled
                } else {
                    inert_or(takes)
                };
                one(self.opening(&texts.t(p.id()), &value, tint, kind, focused, (left, room)))
            }
            Item::Gesture(crate::machine_gesture::Gesture::Hold) => {
                let rev: String = self
                    .records
                    .revision
                    .as_deref()
                    .unwrap_or_default()
                    .chars()
                    .take(REV_SHOWN)
                    .collect();
                let kind = if takes {
                    crate::chrome::Kind::Setting
                } else {
                    crate::chrome::Kind::Disabled
                };
                one(crate::chrome::field_pair(
                    left,
                    room,
                    &texts.t(gesture_name(crate::machine_gesture::Gesture::Hold)),
                    &rev,
                    kind,
                    focused,
                ))
            }
            Item::Step(id) => self
                .step_row(id, takes, left, room, texts)
                .into_iter()
                .collect(),
            // A SENTENCE THAT ACTS, wrapped whole — the conflicts (a link to
            // the belongings, `›` at its end), a gesture of the machine's,
            // a module to approve — every line of it one control.
            other => {
                let (text, color) = self.acting_sentence(other, texts);
                let color = if focused {
                    crate::chrome::Kind::Setting.tones(true).0
                } else if takes {
                    color
                } else {
                    crate::chrome::Kind::Disabled.tones(false).0
                };
                let opens = matches!(other, Item::Belongings);
                let mark = tokens::icon::ROW_OPENS.to_string();
                let mark_w = if opens {
                    layout::width(&mark) + COLUMN_GAP
                } else {
                    0
                };
                // WHAT ONE DOES ABOUT A NOTICE stands under it, set in by a
                // mark's cells: the notice and its answer read as one.
                let indent = if matches!(other, Item::Gesture(_) | Item::Approve(_)) {
                    MARK_CELLS.min(room)
                } else {
                    0
                };
                crate::chrome::note_lines(&text, room.saturating_sub(mark_w + indent))
                    .into_iter()
                    .enumerate()
                    .map(|(n, line)| {
                        let mut fields = vec![field(left + indent, line, color)];
                        if opens && n == 0 {
                            fields.push(field(
                                (left + room).saturating_sub(layout::width(&mark)),
                                mark.clone(),
                                color,
                            ));
                        }
                        ListRow {
                            item: Some(other.clone()),
                            takes,
                            continues: n > 0,
                            ..ListRow::plain(fields, None)
                        }
                    })
                    .collect()
            }
        }
    }

    /// A row that opens something — the label and its value, and `›`
    /// against the right edge.
    fn opening(
        &self,
        label: &str,
        value: &str,
        tint: Option<tokens::Color>,
        kind: crate::chrome::Kind,
        focused: bool,
        (left, room): (u16, u16),
    ) -> Vec<crate::fields::Field> {
        let mark = tokens::icon::ROW_OPENS.to_string();
        let mark_w = layout::width(&mark);
        let mut fields = crate::chrome::tinted(
            crate::chrome::field_pair(
                left,
                room.saturating_sub(mark_w + COLUMN_GAP),
                label,
                value,
                kind,
                focused,
            ),
            tint,
            focused,
        );
        if room > mark_w {
            fields.push(field(left + room - mark_w, mark, kind.tones(focused).0));
        }
        fields
    }

    /// A sentence that acts, and its colour: the conflicts' warning, a
    /// gesture's or a module's row in the colour of a setting.
    fn acting_sentence(
        &self,
        item: &Item,
        texts: &ronoctua_i18n::Texts,
    ) -> (String, tokens::Color) {
        let acts = crate::chrome::Kind::Setting.tones(false).0;
        match item {
            Item::Belongings => (
                texts.t_args(
                    module::key::SYSTEM_UPDATE_RECORDS_CONFLICTS,
                    &[
                        (
                            "count",
                            ronoctua_i18n::Arg::Number(
                                i64::try_from(self.records.conflicts).unwrap_or(i64::MAX),
                            ),
                        ),
                        ("keys", ronoctua_i18n::Arg::Text(&self.belongings_keys)),
                    ],
                ),
                tokens::color::STATE_WARNING,
            ),
            Item::Gesture(g) => (texts.t_with(g.row_key(), &[]), acts),
            Item::Approve(id) => (
                texts.t_with(key::MACHINE_APPROVE_ROW, &[("id", id.as_str())]),
                acts,
            ),
            _ => (String::new(), acts),
        }
    }

    /// A fact of the records, and its colour.
    fn fact_said(&self, fact: Fact, texts: &ronoctua_i18n::Texts) -> (String, tokens::Color) {
        let quiet = tokens::color::TEXT_SECONDARY;
        match fact {
            Fact::Staged(guarded) => (
                texts.t(if guarded {
                    module::key::SYSTEM_UPDATE_RECORDS_STAGED
                } else {
                    module::key::SYSTEM_UPDATE_RECORDS_STAGED_UNGUARDED
                }),
                tokens::color::STATE_WARNING,
            ),
            Fact::Trial(outcome) => (
                texts.t(trial_key(outcome)),
                if outcome == module::TrialOutcome::Failed {
                    tokens::color::STATE_ERROR
                } else {
                    quiet
                },
            ),
            Fact::TriedAgain => (
                texts.t_with(
                    key::MACHINE_TRY_AGAIN_ASKED,
                    &[(
                        "keys",
                        &crate::update_window::view_key_of(crate::update_window::START),
                    )],
                ),
                quiet,
            ),
            Fact::NoTrial(why) => (texts.t(no_trial_key(why)), quiet),
            Fact::Held => {
                let (rev, since) = self
                    .records
                    .hold
                    .as_ref()
                    .map_or_else(Default::default, |h| {
                        (
                            h.rev.chars().take(REV_SHOWN).collect::<String>(),
                            h.since.split('T').next().unwrap_or_default().to_owned(),
                        )
                    });
                (
                    texts.t_with(
                        module::key::SYSTEM_UPDATE_RECORDS_HOLD,
                        &[("rev", &rev), ("since", &since)],
                    ),
                    quiet,
                )
            }
            Fact::Checkout => (texts.t(module::key::SYSTEM_UPDATE_RECORDS_CHECKOUT), quiet),
            Fact::Waiting(n) => {
                let w = self.approvals.waiting.get(n);
                (
                    texts.t_with(
                        key::MACHINE_APPROVE_WAITING,
                        &[
                            ("id", w.map_or("", |w| w.id.as_str())),
                            ("hash", w.map_or("", |w| w.hash.as_str())),
                        ],
                    ),
                    tokens::color::STATE_WARNING,
                )
            }
            Fact::Approved(n) => (
                texts.t_with(
                    key::MACHINE_APPROVE_APPROVED,
                    &[(
                        "id",
                        self.approvals.waiting.get(n).map_or("", |w| w.id.as_str()),
                    )],
                ),
                quiet,
            ),
        }
    }

    /// A question's sentence.
    fn question_said(&self, kind: Answering, texts: &ronoctua_i18n::Texts) -> String {
        match kind {
            Answering::Run => match self.asking.map(|q| q.cost) {
                Some(Cost::Battery(Some(percent))) => texts.t_args(
                    module::key::SYSTEM_UPDATE_ASK_ON_BATTERY,
                    &[
                        ("percent", ronoctua_i18n::Arg::Number(i64::from(percent))),
                        (
                            "floor",
                            ronoctua_i18n::Arg::Number(i64::from(
                                module::battery_floor().unwrap_or_default(),
                            )),
                        ),
                    ],
                ),
                Some(Cost::Battery(None)) => {
                    texts.t(module::key::SYSTEM_UPDATE_ASK_ON_BATTERY_UNKNOWN)
                }
                Some(Cost::Metered) => texts.t(module::key::SYSTEM_UPDATE_ASK_METERED),
                None => String::new(),
            },
            Answering::Gesture => self
                .gesture
                .as_ref()
                .map_or_else(String::new, |q| texts.t(q.what.question_key())),
            Answering::Approval => self.approval.as_ref().map_or_else(String::new, |q| {
                texts.t_with(
                    key::MACHINE_APPROVE_QUESTION,
                    &[("id", q.of.id.as_str()), ("hash", q.of.hash.as_str())],
                )
            }),
            Answering::Grave => String::new(),
        }
    }

    /// An answer's words.
    fn answer_said(&self, kind: Answering, yes: bool, texts: &ronoctua_i18n::Texts) -> String {
        match kind {
            Answering::Run => texts.t(match (self.asking.map(|q| q.cost), yes) {
                (Some(Cost::Metered), false) => module::key::SYSTEM_UPDATE_ANSWER_NO_METERED,
                (Some(Cost::Metered), true) => module::key::SYSTEM_UPDATE_ANSWER_YES_METERED,
                (_, false) => module::key::SYSTEM_UPDATE_ANSWER_NO,
                (_, true) => module::key::SYSTEM_UPDATE_ANSWER_YES,
            }),
            Answering::Gesture => self
                .gesture
                .as_ref()
                .map_or_else(String::new, |q| texts.t(q.what.answer_key(yes))),
            Answering::Approval => texts.t(if yes {
                key::MACHINE_APPROVE_YES
            } else {
                key::MACHINE_APPROVE_NO
            }),
            Answering::Grave => self.cache.as_ref().map_or_else(String::new, |c| {
                c.page
                    .answer_said(yes)
                    .map_or_else(String::new, |k| texts.t(k))
            }),
        }
    }

    /// Whether the cursor is on the answer `yes` of the question `kind`.
    fn answer_here(&self, kind: Answering, yes: bool) -> bool {
        match kind {
            Answering::Run => self.asking.is_some_and(|q| q.yes == yes),
            Answering::Gesture => self.gesture.as_ref().is_some_and(|q| q.yes() == yes),
            Answering::Approval => self.approval.as_ref().is_some_and(|q| q.yes() == yes),
            Answering::Grave => self
                .cache
                .as_ref()
                .and_then(|c| c.page.confirm())
                .is_some_and(|c| c.yes == yes),
        }
    }

    /// An answer's row: the cursor's in the cursor's colour, the yes in the
    /// colour of what it costs.
    fn answer_row(
        &self,
        kind: Answering,
        yes: bool,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> ListRow {
        let color = if self.answer_here(kind, yes) {
            tokens::color::TEXT_HERE_ACTS
        } else if yes && kind == Answering::Grave {
            tokens::color::STATE_ERROR
        } else if yes {
            tokens::color::STATE_WARNING
        } else {
            crate::chrome::Kind::Setting.tones(false).0
        };
        ListRow {
            item: Some(Item::Answer(kind, yes)),
            takes: true,
            ..ListRow::plain(
                vec![field(
                    left,
                    layout::elide_end(&self.answer_said(kind, yes, texts), room),
                    color,
                )],
                None,
            )
        }
    }

    /// A step's row: its box, its name in the colour of whether it runs —
    /// the lock after it when only the machine's configuration switches it
    /// — and on the right what became of it the last time, or who turned it
    /// the way it is.
    fn step_row(
        &self,
        id: &str,
        takes: bool,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<ListRow> {
        let st = self.steps.iter().find(|s| s.id == id)?;
        let def = step_of(id)?;
        let item = Item::Step(def.id);
        let focused = takes && self.focused(&item);
        let switchable = !st.needs_root;
        let box_cells = MARK_CELLS.min(room);
        let glyph = if st.on {
            tokens::icon::UPDATE_STEP_ON
        } else {
            tokens::icon::UPDATE_STEP_OFF
        };
        let box_colour = if switchable && takes {
            crate::chrome::Kind::Setting.tones(false).0
        } else {
            crate::chrome::Kind::Disabled.tones(false).0
        };
        let mut fields = vec![field(left, glyph.to_string(), box_colour)];

        // THE RIGHT SIDE, from the edge inwards, at most a third of the row:
        // the clock last, the word before it, the verdict's mark before that
        // — or, for a step that does not run, who turned it off.
        let (verdict, seconds) = self.verdicts.of_step(def.id);
        // (what, its colour, whether it is a mark in its own cells)
        let mut right: Vec<(String, tokens::Color, bool)> = Vec::new();
        let why = why_short(st.why());
        if !st.on {
            if let Some(k) = why {
                right.push((texts.t(k), tokens::color::TEXT_DIM, false));
            }
        } else if verdict != Verdict::Pending && verdict != Verdict::Off {
            let (mark, colour) = verdict.mark();
            right.push((mark.to_string(), colour, true));
            right.push((texts.t(verdict.key()), colour, false));
            if matches!(
                verdict,
                Verdict::Ran | Verdict::Failed | Verdict::Refused(_)
            ) {
                right.push((clock_of(seconds), tokens::color::TEXT_DIM, false));
            }
        } else if let Some(k) = why {
            right.push((texts.t(k), tokens::color::TEXT_SECONDARY, false));
        } else if st.needs_root {
            right.push((
                texts.t(module::key::SYSTEM_UPDATE_STEP_ROOT_SHORT),
                tokens::color::TEXT_DIM,
                false,
            ));
        }
        let cap = (room / 3).max(1);
        let mut used: u16 = 0;
        let mut placed: Vec<crate::fields::Field> = Vec::new();
        for (text, colour, mark) in right.into_iter().rev() {
            let (text, wide) = if mark {
                (text, MARK_CELLS)
            } else {
                let gap = if placed.is_empty() { 0 } else { COLUMN_GAP };
                let text = layout::elide_end(&text, cap.saturating_sub(used + gap));
                let wide = layout::width(&text) + gap;
                (text, wide)
            };
            if text.is_empty() || used + wide > cap {
                break;
            }
            used += wide;
            placed.push(field(left + room - used, text, colour));
        }
        let right_from = left + room - used;

        // THE NAME, in what is left: the effective state's colour, the cursor
        // the accent.
        let name_at = left + box_cells;
        let lock = tokens::icon::READONLY.to_string();
        let lock_w = if switchable {
            0
        } else {
            layout::width(&lock) + COLUMN_GAP
        };
        let name_room = right_from
            .saturating_sub(name_at)
            .saturating_sub(COLUMN_GAP + lock_w);
        let name = layout::elide_end(&texts.t(def.name_key), name_room);
        let name_colour = if focused {
            crate::chrome::Kind::Setting.tones(true).0
        } else if st.on && takes {
            tokens::color::TEXT_PRIMARY
        } else {
            tokens::color::TEXT_DIM
        };
        let name_end = name_at + layout::width(&name);
        fields.push(field(name_at, name, name_colour));
        if !switchable && name_room > 0 {
            fields.push(field(name_end + COLUMN_GAP, lock, tokens::color::TEXT_DIM));
        }
        placed.reverse();
        fields.extend(placed);
        Some(ListRow {
            item: Some(item),
            takes,
            boxed: Some((left, left + box_cells)),
            ..ListRow::plain(fields, None)
        })
    }

    /// The way back from a page — a row of its own on top of it.
    fn back_row(&self, left: u16, room: u16, texts: &ronoctua_i18n::Texts) -> ListRow {
        let takes = self
            .cache
            .as_ref()
            .is_none_or(|c| c.page.confirm().is_none());
        let colour = if takes && self.on_the_way_back() {
            crate::chrome::Kind::Setting.tones(true).0
        } else {
            crate::chrome::Kind::Setting.tones(false).0
        };
        let word = layout::elide_end(
            &texts.t(module::key::SYSTEM_UPDATE_BACK),
            room.saturating_sub(MARK_CELLS),
        );
        ListRow {
            item: Some(Item::Back),
            takes,
            ..ListRow::plain(
                vec![
                    field(left, tokens::icon::ROW_BACK.to_string(), colour),
                    field(left + MARK_CELLS, word, colour),
                ],
                None,
            )
        }
    }

    /// The page the view is turned to: the way back, then its rows — the
    /// output's from `skip`, `take` of them; the others whole.
    fn page_list(
        &self,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
        skip: usize,
        take: usize,
    ) -> Vec<ListRow> {
        if self.output_open() {
            let head = self.above_the_output(left, room, texts);
            let from = skip.saturating_sub(head.len());
            let lines = self
                .scrollback
                .last_run()
                .skip(from)
                .take(take.saturating_sub(head.len().saturating_sub(skip)))
                .map(|l| ListRow::plain(output_fields(l, left, room), None));
            return head
                .into_iter()
                .skip(skip)
                .chain(lines)
                .take(take)
                .collect();
        }
        let here = !self.on_the_way_back();
        let mut out = vec![self.back_row(left, room, texts)];
        let rows = |lines: Vec<crate::binary_cache::PageLine>| {
            lines.into_iter().map(|l| ListRow {
                item: l.row.map(Item::Row),
                takes: l.row.is_some_and(|n| self.page_takes(n)),
                ..ListRow::plain(l.fields, None)
            })
        };
        if let Some(c) = &self.cache {
            let lines = c.page.lines(&c.readings, left, room, &c.zone, here, texts);
            // THE GRAVE QUESTION, under the row that asked it: the no first.
            let asked = c.page.confirm().is_some();
            for row in rows(lines) {
                let under = asked && row.item == Some(Item::Row(c.page.cursor()));
                out.push(row);
                if under {
                    for line in crate::chrome::note_lines(&c.page.question(texts), room) {
                        out.push(ListRow::plain(
                            vec![field(left, line, tokens::color::QUESTION_ASKED)],
                            None,
                        ));
                    }
                    for yes in [false, true] {
                        out.push(self.answer_row(Answering::Grave, yes, left, room, texts));
                    }
                }
            }
        } else if let Some(f) = &self.follow {
            out.extend(rows(f.page.lines(&f.readings, left, room, here, texts)));
        } else if let Some(p) = &self.parts {
            if let Some(h) = &p.picking {
                for line in crate::parts::picking_said(&h.asked, texts)
                    .into_iter()
                    .flat_map(|said| crate::chrome::note_lines(&said, room))
                {
                    out.push(ListRow::plain(
                        vec![field(left, line, tokens::color::TEXT_SECONDARY)],
                        None,
                    ));
                }
            }
            out.extend(rows(self.parts_lines(left, room, here, texts)));
        }
        out.into_iter().skip(skip).take(take).collect()
    }

    /// The live output's heading, `room` cells from `left` — the footer's,
    /// whose window draws its own over its card: its name and, when this
    /// person may not read the system's journal, that the administrator's
    /// half is missing, at the right end while it fits. Under the pointer it
    /// is the output, explained.
    pub(crate) fn live_heading(
        &self,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> ListRow {
        let named = layout::elide_end(&texts.t(module::key::SYSTEM_UPDATE_OUTPUT_LIVE), room);
        let named_w = layout::width(&named);
        let mut fields = vec![field(left, named, tokens::color::TEXT_HEADING)];
        if !self.system_readable {
            let hidden = texts.t(module::key::SYSTEM_UPDATE_OUTPUT_SYSTEM_HIDDEN_SHORT);
            let hidden_w = layout::width(&hidden);
            if named_w + tokens::space::MODELINE_SEGMENTGAP + hidden_w <= room {
                fields.push(field(
                    left + room - hidden_w,
                    hidden,
                    tokens::color::STATE_WARNING,
                ));
            }
        }
        ListRow {
            item: Some(Item::Output),
            ..ListRow::plain(fields, None)
        }
    }

    /// The output's page above its lines: the way back, the page's name, and
    /// why the root half is not there when this person may not read the
    /// system's journal.
    fn above_the_output(&self, left: u16, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<ListRow> {
        // THE PAGE SAYS WHAT IT IS under its way back, as every page does.
        let mut out = vec![
            self.back_row(left, room, texts),
            ListRow::plain(crate::chrome::AIR, None),
            ListRow::plain(
                vec![field(
                    left,
                    layout::elide_end(&texts.t(module::key::SYSTEM_UPDATE_OUTPUT_TITLE), room),
                    tokens::color::TEXT_HEADING,
                )],
                None,
            ),
        ];
        if !self.system_readable {
            for line in crate::chrome::note_lines(
                &texts.t(module::key::SYSTEM_UPDATE_OUTPUT_SYSTEM_HIDDEN),
                room,
            ) {
                out.push(ListRow::plain(
                    vec![field(left, line, tokens::color::STATE_WARNING)],
                    None,
                ));
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // THE TITLE: the status, and when the last update worked
    // -----------------------------------------------------------------------

    /// THE TITLE'S ROWS, `room` cells wide: the status — whether an update
    /// is there, what the brain is doing with it — with `name` (the window's,
    /// while it wears no header) at the right end of its first row, and a
    /// quiet row under it saying when the last update worked.
    ///
    /// AS TALL AS THE LONGEST STATUS THE VIEW CAN SAY — the room a shorter
    /// one leaves is kept under the quiet row, as air before the list — so a
    /// status that changes never moves the list under it: only the width and
    /// the language do. The name stays beside the status only while the longest
    /// status fits beside it on one row; otherwise it is alone on the first
    /// row, and the status wraps under it at the whole width.
    #[must_use]
    pub fn status_lines(
        &self,
        room: u16,
        name: Option<&str>,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<ListRow> {
        let status = self.status();
        let said = status.sentence(texts);
        let longest = Status::all()
            .into_iter()
            .map(|s| layout::width(&s.sentence(texts)))
            .max()
            .unwrap_or(0);
        let named = name.map(|n| (n, layout::width(n)));
        let beside = named.is_some_and(|(_, w)| longest + COLUMN_GAP + w <= room);
        let focused = self.focused(&Item::Status);
        // A CONTROL WHILE IT SAYS GITHUB'S ANSWER, and while the keys walk
        // the list it heads: no question holds the cursor, no page is open.
        let takes = status.is_online() && !self.questioned() && !self.page_open();
        let color = if focused {
            tokens::color::TEXT_HERE_ACTS
        } else {
            status.color()
        };
        let mut out = Vec::new();
        if let (Some((n, w)), false) = (named, beside) {
            let _ = w;
            out.push(ListRow::plain(
                vec![field(
                    0,
                    layout::elide_end(n, room),
                    tokens::color::TEXT_DIM,
                )],
                None,
            ));
        }
        let wrap = if beside {
            named.map_or(room, |(_, w)| room.saturating_sub(w + COLUMN_GAP))
        } else {
            room
        };
        let tall = Status::all()
            .into_iter()
            .map(|s| crate::chrome::note_lines(&s.sentence(texts), wrap).len())
            .max()
            .unwrap_or(1)
            .max(1);
        let lines = crate::chrome::note_lines(&said, wrap);
        let shown = lines.len().clamp(1, tall);
        for n in 0..shown {
            let mut fields = Vec::new();
            if let Some(line) = lines.get(n) {
                fields.push(field(0, line.clone(), color));
            }
            if n == 0
                && beside
                && let Some((name, w)) = named
            {
                fields.push(field(room - w, name.to_owned(), tokens::color::TEXT_DIM));
            }
            let on_it = n < lines.len();
            out.push(ListRow {
                item: on_it.then_some(Item::Status),
                takes: takes && on_it,
                continues: n > 0,
                ..ListRow::plain(fields, None)
            });
        }
        // WHEN THE LAST UPDATE WORKED, quiet, right under the status.
        let worked = self
            .brain
            .and_then(|b| b.worked)
            .map(|at| {
                let when = crate::chrome::when_text(at, &self.zone, false, texts);
                texts.t_with(
                    module::key::SYSTEM_UPDATE_LAST_WORKED,
                    &[("when", when.as_str())],
                )
            })
            .unwrap_or_default();
        let worked = crate::chrome::note_lines(&worked, room);
        let unsaid = usize::from(worked.is_empty());
        for line in worked {
            out.push(ListRow::plain(
                vec![field(0, line, tokens::color::TEXT_DIM)],
                None,
            ));
        }
        // AND THE ROOM THE LONGEST STATUS WOULD TAKE — and the line about the
        // last update while there is none — kept under them, the air before
        // the list: a status that changes never moves the list, and nothing
        // hangs between the status and the line about it.
        for _ in 0..(tall - shown + unsaid) {
            out.push(ListRow::plain(crate::chrome::AIR, None));
        }
        out
    }

    // -----------------------------------------------------------------------
    // WHAT A ROW SAYS BESIDE ITS NAME
    // -----------------------------------------------------------------------

    /// Why `action` cannot act now, in a few words — its row's value.
    const fn why_not_short(&self, action: Action) -> &'static str {
        match action {
            Action::Rehearse if self.rehearsal_now() => {
                module::key::SYSTEM_UPDATE_REHEARSE_RUNNING_SHORT
            }
            Action::Rehearse if self.rehearsal_asked() => {
                module::key::SYSTEM_UPDATE_REHEARSE_PENDING_SHORT
            }
            Action::Rehearse => module::key::SYSTEM_UPDATE_REHEARSE_BUSY_SHORT,
            _ => module::key::SYSTEM_UPDATE_START_QUEUED_SHORT,
        }
    }

    /// Why `action` cannot act now, whole — said where the door explains it,
    /// and when it is asked for all the same.
    #[must_use]
    pub const fn why_not_said(&self, action: Action) -> &'static str {
        match action {
            Action::Rehearse if self.rehearsal_now() => module::key::SYSTEM_UPDATE_REHEARSE_RUNNING,
            Action::Rehearse if self.rehearsal_asked() => {
                module::key::SYSTEM_UPDATE_REHEARSE_PENDING
            }
            Action::Rehearse => module::key::SYSTEM_UPDATE_REHEARSE_BUSY,
            _ => module::key::SYSTEM_UPDATE_START_QUEUED,
        }
    }

    /// A page's value beside its name, and the colour of a state it says (a
    /// copy that does not read).
    fn glance_of(
        &self,
        page: PageOf,
        texts: &ronoctua_i18n::Texts,
    ) -> (String, Option<tokens::Color>) {
        use ronoctua_cachix::settings::Named;
        let unreadable = Some(tokens::color::STATE_WARNING);
        match page {
            PageOf::Cache => match &self.glance.cache {
                Named::None => (texts.t(key::BINARY_CACHE_NONE), None),
                Named::Cache(c) => (
                    texts.t_with(
                        match c.visibility {
                            ronoctua_person::options::Visibility::Public => {
                                key::BINARY_CACHE_GLANCE_PUBLIC
                            }
                            ronoctua_person::options::Visibility::Private => {
                                key::BINARY_CACHE_GLANCE_PRIVATE
                            }
                        },
                        &[("name", c.name.as_str())],
                    ),
                    None,
                ),
                Named::Unreadable => (texts.t(key::BINARY_CACHE_COPY_UNREADABLE), unreadable),
                Named::Newer => (texts.t(key::BINARY_CACHE_COPY_NEWER), unreadable),
            },
            PageOf::Follow => match self.glance.follow {
                crate::follow::Choice::On => (texts.t(key::DISTRO_FOLLOW_ON), None),
                crate::follow::Choice::Off => (texts.t(key::DISTRO_FOLLOW_OFF), None),
                crate::follow::Choice::Unreadable => {
                    (texts.t(key::DISTRO_FOLLOW_COPY_UNREADABLE), unreadable)
                }
                crate::follow::Choice::Newer => {
                    (texts.t(key::DISTRO_FOLLOW_COPY_NEWER), unreadable)
                }
            },
            PageOf::Parts => (
                texts.t_args(
                    key::DISTRO_PARTS_GLANCE,
                    &[(
                        "count",
                        ronoctua_i18n::Arg::Number(
                            i64::try_from(self.glance.parts).unwrap_or(i64::MAX),
                        ),
                    )],
                ),
                None,
            ),
            // HOW MUCH THE LAST RUN WROTE, the row's name saying only what
            // the page is: a sentence there ran into its value.
            PageOf::Output if self.has_output() => (
                texts.t_args(
                    module::key::SYSTEM_UPDATE_OUTPUT_GLANCE,
                    &[(
                        "lines",
                        ronoctua_i18n::Arg::Number(
                            i64::try_from(self.scrollback.last_run().count()).unwrap_or(i64::MAX),
                        ),
                    )],
                ),
                None,
            ),
            PageOf::Output => (texts.t(module::key::SYSTEM_UPDATE_OUTPUT_NONE_SHORT), None),
        }
    }

    /// When the update happens by itself, in a few words — the Schedule
    /// row's value: every so many days, in which part of the day and the
    /// next time; or, when the brain says it will not, why; or that no
    /// brain is answering.
    #[must_use]
    pub fn schedule_value(&self, texts: &ronoctua_i18n::Texts) -> String {
        let Some(s) = &self.schedule else {
            return texts.t(module::key::SYSTEM_UPDATE_SCHEDULE_NO_BRAIN);
        };
        if !s.enabled || s.why_not == SWITCHED_OFF {
            return texts.t(module::key::SYSTEM_UPDATE_SCHEDULE_OFF);
        }
        // A QUIET BRAIN in this row's words, beside the task switched off:
        // two reasons, each said as itself.
        if s.why_not == QUIET {
            return texts.t(module::key::SYSTEM_UPDATE_SCHEDULE_QUIET);
        }
        if !s.why_not.is_empty() {
            return texts.t(&s.why_key);
        }
        if s.next == 0 {
            return texts.t(module::key::SYSTEM_UPDATE_SCHEDULE_OVERDUE);
        }
        let every = texts.t_args(
            module::key::SYSTEM_UPDATE_SCHEDULE_EVERY,
            &[(
                "days",
                ronoctua_i18n::Arg::Number(i64::from(s.interval_days)),
            )],
        );
        let part = texts.t_with(
            module::key::SYSTEM_UPDATE_SCHEDULE_PART,
            &[("part", s.part.word())],
        );
        let choice = crate::when::prefs::current();
        let days = day_of(s.next, &self.zone) - day_of(s.now, &self.zone);
        let weekday = texts.t_args(
            module::key::SYSTEM_UPDATE_WEEKDAY_SHORT,
            &[(
                "day",
                ronoctua_i18n::Arg::Number(weekday_of(day_of(s.next, &self.zone))),
            )],
        );
        let time = crate::chrome::clock_text(s.next, &self.zone, choice.clock, texts);
        let date = crate::chrome::date_text(s.next, &self.zone, choice.order, texts);
        let next = texts.t_args(
            module::key::SYSTEM_UPDATE_SCHEDULE_NEXT,
            &[
                ("days", ronoctua_i18n::Arg::Number(days)),
                ("time", ronoctua_i18n::Arg::Text(&time)),
                ("weekday", ronoctua_i18n::Arg::Text(&weekday)),
                ("date", ronoctua_i18n::Arg::Text(&date)),
            ],
        );
        texts.t_with(
            module::key::SYSTEM_UPDATE_SCHEDULE_SUMMARY,
            &[("every", &every), ("part", &part), ("next", &next)],
        )
    }

    // -----------------------------------------------------------------------
    // WHAT A THING IS — said where the door explains
    // -----------------------------------------------------------------------

    /// WHAT `item` IS, as the door explains it — the window's foot, the
    /// footer's explanation line: its name, what `how` the hand takes it
    /// does ([`door::foot::gesture`]), and the sentences about it, each in
    /// its tone — what it is, a refusal or why it is off in warning, the
    /// step's command dim — the last saying what will happen.
    #[must_use]
    pub fn explained(
        &self,
        item: &Item,
        how: door::Gesture,
        texts: &ronoctua_i18n::Texts,
    ) -> door::Explained {
        let quiet = tokens::color::TEXT_SECONDARY;
        let warning = tokens::color::STATE_WARNING;
        let gesture = |does: &str| Some(door::foot::gesture(texts, how, &texts.t(does)));
        let (name, gesture, said): (String, Option<String>, Vec<(String, tokens::Color)>) =
            match item {
                Item::Heading(s) => (texts.t(s.title()), None, vec![(texts.t(s.about()), quiet)]),
                // WHAT IS MISSING FROM IT FIRST, when something is: it is
                // what this person needs to know of it.
                Item::Output => {
                    let mut said = Vec::new();
                    if !self.system_readable {
                        said.push((
                            texts.t(module::key::SYSTEM_UPDATE_OUTPUT_SYSTEM_HIDDEN),
                            warning,
                        ));
                    }
                    said.push((texts.t(module::key::SYSTEM_UPDATE_OUTPUT_LIVE_DOES), quiet));
                    (texts.t(module::key::SYSTEM_UPDATE_OUTPUT_LIVE), None, said)
                }
                // GITHUB'S ANSWER is asked again by a click; any other status
                // — a run, the restart, a hold — is the machine's, and says
                // only itself.
                Item::Status if self.status().is_online() => {
                    let status = self.status();
                    (
                        texts.t(module::key::SYSTEM_UPDATE_STATUS_NAME),
                        gesture(module::key::SYSTEM_UPDATE_ACT_RECHECK),
                        vec![
                            (status.sentence(texts), status.color()),
                            (texts.t(module::key::SYSTEM_UPDATE_STATUS_EXPLAINED), quiet),
                        ],
                    )
                }
                Item::Status => {
                    let status = self.status();
                    (
                        texts.t(module::key::SYSTEM_UPDATE_STATUS_STANDS),
                        None,
                        vec![(status.sentence(texts), status.color())],
                    )
                }
                Item::Action(a) if !self.acts(*a) => (
                    texts.t(a.id()),
                    None,
                    vec![(texts.t(self.why_not_said(*a)), warning)],
                ),
                // A REHEARSAL'S answers are the rehearsal's own: its stop, and a
                // "not now" that lets it go.
                Item::Action(a) => {
                    let rehearsal = self.rehearsal_now();
                    let mut said = vec![(texts.t(a.does_in(rehearsal)), quiet)];
                    if *a == Action::Rehearse
                        && let Some(r) = self.rehearsed_said(texts)
                    {
                        said.push((r, quiet));
                    }
                    (
                        texts.t(a.id_in(rehearsal)),
                        gesture(a.act_in(rehearsal)),
                        said,
                    )
                }
                Item::Schedule => (
                    texts.t(crate::update_window::SCHEDULE),
                    gesture(module::key::SYSTEM_UPDATE_ACT_OPEN_BRAIN),
                    vec![
                        (self.schedule_value(texts), tokens::color::TEXT_PRIMARY),
                        (texts.t(module::key::SYSTEM_UPDATE_SCHEDULE_DOES), quiet),
                    ],
                ),
                Item::Page(PageOf::Output) if !self.has_output() => (
                    texts.t(PageOf::Output.id()),
                    None,
                    vec![(texts.t(module::key::SYSTEM_UPDATE_OUTPUT_NONE), warning)],
                ),
                Item::Page(p) => {
                    let mut said = vec![(texts.t(p.does()), quiet)];
                    if *p == PageOf::Output && !self.system_readable {
                        said.push((
                            texts.t(module::key::SYSTEM_UPDATE_OUTPUT_SYSTEM_HIDDEN),
                            warning,
                        ));
                    }
                    (
                        texts.t(p.id()),
                        gesture(module::key::SYSTEM_UPDATE_ACT_OPEN_PAGE),
                        said,
                    )
                }
                Item::Step(id) => return self.step_explained(id, how, texts),
                Item::Back => (
                    texts.t(module::key::SYSTEM_UPDATE_BACK),
                    gesture(module::key::SYSTEM_UPDATE_ACT_BACK),
                    vec![(texts.t(module::key::SYSTEM_UPDATE_BACK_DOES), quiet)],
                ),
                Item::Belongings => (
                    texts.t(module::key::SYSTEM_UPDATE_CONFLICTS_NAME),
                    gesture(module::key::SYSTEM_UPDATE_ACT_OPEN_BELONGINGS),
                    vec![
                        (self.acting_sentence(item, texts).0, warning),
                        (
                            texts.t(module::key::SYSTEM_UPDATE_REGISTER_BELONGINGS),
                            quiet,
                        ),
                    ],
                ),
                Item::Gesture(g) => (
                    texts.t(gesture_name(*g)),
                    self.gesture_takes(*g)
                        .then(|| gesture(module::key::SYSTEM_UPDATE_ACT_ASK))
                        .flatten(),
                    vec![
                        (texts.t_with(g.row_key(), &self.gesture_args(*g)), quiet),
                        (texts.t(module::key::SYSTEM_UPDATE_REGISTER_ASKS), quiet),
                    ],
                ),
                Item::Approve(id) => (
                    texts.t_with(key::MACHINE_APPROVE_ROW, &[("id", id.as_str())]),
                    self.quiet()
                        .then(|| gesture(module::key::SYSTEM_UPDATE_ACT_ASK))
                        .flatten(),
                    vec![(texts.t(module::key::SYSTEM_UPDATE_REGISTER_ASKS), quiet)],
                ),
                Item::Answer(kind, yes) => {
                    let asked = if *kind == Answering::Grave {
                        self.cache
                            .as_ref()
                            .map_or_else(String::new, |c| c.page.question(texts))
                    } else {
                        self.question_said(*kind, texts)
                    };
                    (
                        self.answer_said(*kind, *yes, texts),
                        gesture(module::key::SYSTEM_UPDATE_ACT_ANSWER),
                        vec![(asked, quiet)],
                    )
                }
                Item::Row(n) => return self.page_explained(*n, how, texts),
            };
        door::Explained {
            subject: item.subject(),
            name,
            gesture,
            said,
        }
    }

    /// What step `id` is, whole, most needed first — the foot shows its
    /// first lines and scrolls to the rest: what it does, what switching it
    /// now will do (or why only the machine switches it), who turned it the
    /// way it is, what came of it the last time, and the command it runs.
    fn step_explained(
        &self,
        id: &str,
        how: door::Gesture,
        texts: &ronoctua_i18n::Texts,
    ) -> door::Explained {
        let Some((st, def)) = self
            .steps
            .iter()
            .find(|s| s.id == id)
            .and_then(|s| step_of(id).map(|d| (s, d)))
        else {
            return door::Explained::default();
        };
        let (verdict, _) = self.verdicts.of_step(def.id);
        let mut said = vec![(texts.t(def.does_key), tokens::color::TEXT_SECONDARY)];
        let gesture = if st.needs_root {
            said.push((
                texts.t(module::key::SYSTEM_UPDATE_SWITCH_ROOT),
                tokens::color::STATE_WARNING,
            ));
            None
        } else {
            // WHAT SWITCHING IT WILL DO — this update's own, when it runs
            // and has not reached the step yet: the runner reads the switch
            // when it comes to each step.
            let to = if st.on { "off" } else { "on" };
            let will = if self.run() == Run::Running && verdict == Verdict::Pending {
                module::key::SYSTEM_UPDATE_SWITCH_WILL_NOW
            } else {
                module::key::SYSTEM_UPDATE_SWITCH_WILL
            };
            said.push((
                texts.t_with(will, &[("on", to)]),
                tokens::color::TEXT_PRIMARY,
            ));
            Some(door::foot::gesture(
                texts,
                how,
                &texts.t(if st.on {
                    module::key::SYSTEM_UPDATE_ACT_SWITCH_OFF
                } else {
                    module::key::SYSTEM_UPDATE_ACT_SWITCH_ON
                }),
            ))
        };
        if let Some(k) = why_said(st.why()) {
            said.push((texts.t(k), tokens::color::TEXT_SECONDARY));
        }
        if let Some(last) = crate::update_window::last_run_explains(def, &verdict, texts) {
            said.push((last, tokens::color::TEXT_SECONDARY));
        }
        // THE COMMAND IS DATA, drawn as it is written: a command line is not
        // translated.
        said.push((def.command.to_owned(), tokens::color::TEXT_DIM));
        door::Explained {
            subject: Item::Step(def.id).subject(),
            name: texts.t(def.name_key),
            gesture,
            said,
        }
    }

    /// What row `n` of the open page is, as the page says it.
    fn page_explained(
        &self,
        n: usize,
        how: door::Gesture,
        texts: &ronoctua_i18n::Texts,
    ) -> door::Explained {
        let mut said = if let Some(c) = &self.cache {
            c.page.explain(n, &c.readings, how, texts)
        } else if let Some(f) = &self.follow {
            f.page.explain(n, &f.readings, how, texts)
        } else if let Some(p) = &self.parts {
            if p.picking.is_some() {
                door::Explained::default()
            } else {
                p.page.explain(n, &p.readings, &p.zone, how, texts)
            }
        } else {
            door::Explained::default()
        };
        said.subject = Item::Row(n).subject();
        said
    }

    /// What the last rehearsal measured, as a sentence — when one did.
    pub(crate) fn rehearsed_said(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let r = self.rehearsed.as_ref()?;
        let when = crate::chrome::when_text(r.at.unix_s(), &self.zone, false, texts);
        let size = ronoctua_shell_kit::sizes::size_text(r.download_bytes, texts.decimal_mark());
        Some(texts.t_args(
            module::key::SYSTEM_UPDATE_REHEARSED,
            &[
                ("when", ronoctua_i18n::Arg::Text(&when)),
                ("size", ronoctua_i18n::Arg::Text(&size)),
                (
                    "builds",
                    ronoctua_i18n::Arg::Number(i64::try_from(r.builds).unwrap_or(i64::MAX)),
                ),
            ],
        ))
    }

    /// The arguments of a gesture's row sentence.
    fn gesture_args(&self, what: crate::machine_gesture::Gesture) -> Vec<(&'static str, &str)> {
        match what {
            crate::machine_gesture::Gesture::Hold => vec![(
                "rev",
                self.records
                    .revision
                    .as_deref()
                    .map_or("", |r| &r[..r.len().min(REV_SHOWN)]),
            )],
            _ => Vec::new(),
        }
    }
}

/// One output line as a row's pieces, `room` cells from `left`: each piece
/// in its colour — a program's own, or the line's — cut at the width.
fn output_fields(
    line: &crate::output::OutputLine,
    left: u16,
    room: u16,
) -> Vec<crate::fields::Field> {
    let mut out = Vec::new();
    let mut at = left;
    let end = left + room;
    for (text, color) in line.inked() {
        if at >= end {
            break;
        }
        let text = layout::elide_end(text, end - at);
        let wide = layout::width(&text);
        if !text.is_empty() {
            out.push(field(at, text, color));
        }
        at += wide;
    }
    out
}
