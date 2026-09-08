//! Formatting a disk, in three questions — `LEADER s f`.
//!
//! # Why this is three lists and never a form
//!
//! Every other way of formatting a disk asks somebody to know things: what a
//! partition table is, which of nine filesystems, what `-F 32` means. This asks
//! three questions, each answered by moving a cursor and pressing `Enter`:
//! **which disk**, **what for**, and **are you sure**. There is exactly one
//! place to type, and it is the disk's name.
//!
//! The words are the point. A row says *"opens on every computer — pendrive,
//! camera, television"*, not *"FAT32"*. Somebody who knows it is FAT32 loses
//! nothing; somebody who does not gets an answer instead of a word to look up.
//!
//! # It reads nothing and it decides nothing
//!
//! Not the bus, not the disk, not a clock. The devices arrive through
//! `input::rack`, already read, already cut and already judged by the host —
//! including the judgement that matters, which is which of them the running
//! system needs. What leaves is `effect::format` naming an id this buffer was
//! GIVEN, and the host checks it again against the list it sent. Invariant 11
//! intact: the component has no parser and is not getting one.
//!
//! # Two things are drawn and not omitted
//!
//! A disk the system needs is **on the list**, greyed, with the reason beside
//! it. And XFS is **on the list of filesystems**, greyed, saying that this
//! distro does not carry the program that writes it. Both could have been left
//! out; both would then be somebody searching a screen for something that is
//! not there, which is the failure this repository refuses everywhere else.
//!
//! # And the geometry is no longer decided here
//!
//! Every number that put something on the screen used to live in this file: a
//! gutter of two cells, a cell of air at the right border, a gap of two between
//! columns, and a `title()` that drew a rule and left a blank cell under it.
//! `ronoctua-design` owns all four now, and two of them turned out to be the
//! SAME MARGIN TWICE.
//!
//!   - `buffer.rs`'s `context_of` already takes `space.buffer.marginX` off BOTH
//!     sides before it reports `columns`, so a gutter here was an indent inside
//!     an indent — and the cell reserved at the right border was air outside air.
//!     This buffer was the concrete instance of the "margin of its own, usually
//!     excessive" the screen was photographed for. Both are gone; nothing below
//!     starts anywhere but at cell zero.
//!   - the rule under the title is gone with them. What separates a heading from
//!     its content is whitespace, and `Screen::heading` is the one place that
//!     says how much of it.
//!
//! What is NOT delegated is a single refusal below. They are what this buffer
//! IS, and a change of who decides the geometry does not touch them.

use std::cell::RefCell;

use ronoctua_design::guest::Wit;
use ronoctua_design::types::{
    AIR, BufferState, Capability, Color, Column, Command, Context, Descriptor, Device, Effect, GAP,
    Guest, ICON, Icon, Input, Layout, Plan, Rack, Recipe, Role, Run, Screen, Table, Tone,
};
use ronoctua_design::wide;

/// Every text key this buffer names.
///
/// A module, and every one of them a literal `pub const`: that is what
/// `buffer-keys-have-text` can see. A key built with `format!` is a key the
/// check cannot find, and the first sign of a missing one would be its own name
/// drawn on screen in front of somebody who was about to erase a disk.
mod key {
    pub const TITLE: &str = "format-title";
    pub const EMPTY: &str = "format-empty";
    pub const HINT_LIST: &str = "format-hint-list";
    pub const HINT_CHOOSE: &str = "format-hint-choose";
    pub const HINT_CONFIRM: &str = "format-hint-confirm";

    /// What a row IS, in the two words that decide everything downstream.
    pub const WHOLE: &str = "format-whole";
    pub const PART: &str = "format-part";
    pub const BLANK: &str = "format-blank";

    pub const CHOOSE_TITLE: &str = "format-choose-title";
    pub const RECIPE_UNIVERSAL: &str = "format-recipe-universal";
    pub const RECIPE_LARGE: &str = "format-recipe-large";
    pub const RECIPE_WINDOWS: &str = "format-recipe-windows";
    pub const RECIPE_LINUX: &str = "format-recipe-linux";
    pub const RECIPE_MODERN: &str = "format-recipe-modern";
    pub const RECIPE_MISSING: &str = "format-recipe-missing";
    pub const NAME: &str = "format-name";
    pub const NAME_LIMIT: &str = "format-name-limit";

    pub const CONFIRM_TITLE: &str = "format-confirm-title";
    pub const CONFIRM_WHOLE: &str = "format-confirm-whole";
    pub const CONFIRM_PART: &str = "format-confirm-part";
    pub const CONFIRM_NO: &str = "format-confirm-no";
    pub const CONFIRM_YES: &str = "format-confirm-yes";

    pub const WORKING: &str = "format-working";
    pub const DONE: &str = "format-done";

    /// THE DETAIL LINE, which is the same screen answering a second person.
    ///
    /// The rows above are written for somebody who does not know what FAT32 is
    /// and should not have to. These are for somebody who does: the real name
    /// of the filesystem, its limits, and what the gesture is actually going to
    /// do to the disk. One line, under the cursor, in the dimmest colour there
    /// is — a beginner never has to read it, and an expert never has to guess.
    pub const TECH_UNIVERSAL: &str = "format-tech-universal";
    pub const TECH_LARGE: &str = "format-tech-large";
    pub const TECH_WINDOWS: &str = "format-tech-windows";
    pub const TECH_LINUX: &str = "format-tech-linux";
    pub const TECH_MODERN: &str = "format-tech-modern";
    pub const TECH_MISSING: &str = "format-tech-missing";
    pub const TECH_WHOLE: &str = "format-tech-whole";
    pub const TECH_PART: &str = "format-tech-part";
    pub const TECH_NEEDED: &str = "format-tech-needed";
    pub const TECH_NOTHING: &str = "format-tech-nothing";
}

/// The states this buffer declares, and what the modeline shows for each.
///
/// # There is deliberately no `working`
///
/// Waiting for a disk to be written was a fourth screen in the first draft, and
/// it had to go: every state a buffer declares is offered under `SPC w s`, and
/// a `working` somebody could pick would put "writing to the disk" on the
/// modeline with nothing being written. `states-driven` is what said so, by
/// refusing a state that cannot be entered from the base.
///
/// So the wait happens on the LIST, with the message line saying what is going
/// on — which is also where it belongs: the row being formatted is about to
/// change under the cursor, and that is the thing worth looking at.
const STATES: &[(&str, char, Tone)] = &[
    ("normal", 'n', Tone::Standard),
    ("choose", 'c', Tone::Accent),
    ("confirm", 'y', Tone::Warning),
];

/// The longest label this buffer will send.
///
/// ELEVEN, WHICH IS vfat's LIMIT AND THE STRICTEST OF THE FIVE. One number for
/// every filesystem rather than five, and the reason is that the number is
/// SHOWN: "up to 11 letters" beside the field is a rule somebody can follow,
/// where a limit that changed as the cursor moved through the list above would
/// be a rule that moves. The cost is a btrfs volume that cannot be called
/// something long, which nobody has ever wanted from this screen.
const NAME_LIMIT: usize = 11;

/// What the line above the hint says, once something has happened.
///
/// TWO CASES AND NOT A STRING, because they come from different places: what
/// went well is a word this buffer owns and looks up by key, and what went
/// wrong was worded by the host — udisks' refusals are its to name, and a
/// component has no bundle to look one up in (invariant 11).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Said {
    /// It is being written right now.
    Working,
    /// It was written.
    Done,
    /// It was refused, in the host's words.
    Trouble(String),
}

/// Which of the three questions is on.
///
/// `Question` and not `Screen`: the frame under construction is
/// [`ronoctua_design::types::Screen`] now, and one name for two things is how a
/// reader stops knowing which is which. The three cases are the three questions
/// this buffer asks, which is what the type was always about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Question {
    /// Every disk, one per row. Which one?
    List,
    /// Five filesystems and a name. What for?
    Choose,
    /// Two rows, and `no` is the first. Are you sure?
    Confirm,
}

/// The five, in the order they are drawn.
///
/// The order is not the enum's and is not alphabetical: it is most-likely
/// first. Somebody formatting a stick almost always wants the one that opens
/// everywhere, and it is the row the cursor starts on.
/// The five, in the order they are drawn, each with the two ways of saying it.
///
/// # Why the technical name is a KEY and not a string in this file
///
/// Because `vfat` is a fact about disks, and a component with an opinion about
/// disks is a component that has stopped being a drawing (invariant 11). It is
/// also CONTENT — what to call FAT32 in front of somebody, how much of its
/// limit is worth naming — and content lives in the `.ftl` with everything
/// else, in both languages, where the check can see it.
const RECIPES: [(Recipe, &str, &str); 5] = [
    (
        Recipe::Universal,
        key::RECIPE_UNIVERSAL,
        key::TECH_UNIVERSAL,
    ),
    (Recipe::LargeFiles, key::RECIPE_LARGE, key::TECH_LARGE),
    (Recipe::Windows, key::RECIPE_WINDOWS, key::TECH_WINDOWS),
    (Recipe::Linux, key::RECIPE_LINUX, key::TECH_LINUX),
    (Recipe::Modern, key::RECIPE_MODERN, key::TECH_MODERN),
];

/// Where the cursor sits when it is on the row that cannot be chosen.
///
/// # Why XFS is a POSITION and not a caption
///
/// It was drawn as a fixed line under the five at first, and the cursor could
/// not reach it. That made it the one thing on any of these screens that
/// explains itself only to somebody who happened to read it — where the whole
/// rule of this buffer is the opposite: the disk the running system needs is
/// ON the list, reachable, refusing when pressed, and saying why on the line
/// under the cursor. The row that names a filesystem this build cannot write
/// is the same case, and it gets the same shape.
const MISSING: usize = RECIPES.len();

/// Everything this buffer remembers between calls.
struct State {
    rack: Rack,
    screen: Question,
    /// The cursor on the list, as a device ID and never a row number.
    ///
    /// The rack is re-read and re-sorted whenever udisks says anything — a
    /// stick plugged in reorders it — so a row number would point at whatever
    /// moved into that position between the draw and the key. On a screen whose
    /// next question is "erase this?", that is the whole ballgame.
    on: Option<String>,
    /// Which of [`RECIPES`] the cursor is on.
    recipe: usize,
    /// `false` for "no, go back", which is where the cursor starts.
    yes: bool,
    /// What to call it. Empty is ordinary and means no name.
    name: String,
    /// The device the question is about, once one has been chosen.
    ///
    /// A COPY AND NOT A LOOKUP. Between choosing and confirming the rack may
    /// arrive again with this device gone; the screen has to go on naming what
    /// was chosen, and the host has the last word on whether it is still there.
    target: Option<Device>,
    /// What became of the last format.
    said: Option<Said>,
    /// The window's height, as the last `render` saw it.
    lines: u16,
    /// The line the list starts on, so a click can be resolved.
    list_top: Option<u16>,
}

impl State {
    const fn fresh() -> Self {
        Self {
            rack: Rack {
                devices: Vec::new(),
            },
            screen: Question::List,
            on: None,
            recipe: 0,
            yes: false,
            name: String::new(),
            target: None,
            said: None,
            lines: 24,
            list_top: None,
        }
    }

    fn state_id(&self) -> String {
        match self.screen {
            Question::List => STATES[0].0,
            Question::Choose => STATES[1].0,
            Question::Confirm => STATES[2].0,
        }
        .to_owned()
    }
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::fresh()) };
}

// ---------------------------------------------------------------------------
// Numbers a person reads.
// ---------------------------------------------------------------------------

/// Bytes as a person reads them.
///
/// The separator is a full stop because a component does not know the language
/// and does not translate (invariant 11) — a comma here would be a decision
/// about locale taken in the one place that cannot make it.
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

/// The size again, and this time without rounding.
///
/// `32.0 GiB` is what somebody reads; `34359738368 B` is what somebody
/// COMPARES — against what the vendor printed on the stick, against what
/// `lsblk` said, against the disk they meant to grab. Rounding is the right
/// answer on the row and the wrong one on this line, and that is the whole
/// reason both exist.
///
/// No thousands separator: the character depends on the language and a
/// component does not know one (invariant 11).
fn bytes_exactly(n: u64) -> String {
    format!("{n} B")
}

/// Two readings the screen cannot tell apart.
///
/// # Why this is written out and not `==`
///
/// Because the bindings are `ronoctua-design`'s now, generated ONCE for every
/// buffer, and this is the only one that ever compared a whole record. Asking
/// the shared generation for `additional_derives: [PartialEq]` would put the
/// derive on all thirty-odd records of the WIT for the sake of one comparison
/// in one component; the comparison is nine fields, and here they are.
///
/// The list is ORDERED, and comparing it in order is right: udisks sorts by
/// device node, so two readings that differ only in order are two different
/// screens.
fn same_rack(a: &Rack, b: &Rack) -> bool {
    a.devices.len() == b.devices.len() && a.devices.iter().zip(&b.devices).all(same_device)
}

/// The same, for one device. Every field, because every field is drawn or
/// decides something that is.
fn same_device((a, b): (&Device, &Device)) -> bool {
    a.id == b.id
        && a.name == b.name
        && a.node == b.node
        && a.kind == b.kind
        && a.bytes == b.bytes
        && a.mounted == b.mounted
        && a.removable == b.removable
        && a.needed == b.needed
        && a.whole == b.whole
}

/// May this device be written over?
///
/// The host's answer, carried on the row. This buffer does not second-guess it
/// and could not: it has no bus and no `/etc/fstab`.
const fn spare(d: &Device) -> bool {
    d.needed.is_empty()
}

// ---------------------------------------------------------------------------
// Drawing.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// How wide one drawing is — the only geometry a caller still decides.
//
// `GUTTER` and `EDGE_RIGHT` used to open this section, and both are gone. They
// were the host's own margin paid a second time: `buffer.rs`'s `context_of`
// subtracts `space.buffer.marginX` from BOTH sides before it reports
// `columns`, so cell zero is already inside the window and `columns` already
// stops short of its right border. `GAP` is gone too — the air between two
// columns is `ronoctua_design::GAP`, once, for every buffer.
//
// What is left below is how wide a THING is: a device node is fourteen cells
// because `/dev/nvme0n1p1` is fourteen. Those belong to whoever draws the
// thing; the arithmetic that places them is `Table`'s.
// ---------------------------------------------------------------------------

/// The columns of this buffer's three tables, by name.
///
/// `&'static str` ids and not numbers, because a `Layout` answers by name: a
/// column that gave way is a column that is not in the answer, and every row
/// below is built as though every column were there.
mod field {
    /// The list: what kind of disk, what it is called, where it lives.
    pub const ICON: &str = "icon";
    pub const NAME: &str = "name";
    pub const NODE: &str = "node";
    /// The list's facts: the filesystem on it, how big, and whole or part.
    pub const KIND: &str = "kind";
    pub const SIZE: &str = "size";
    pub const WHAT: &str = "what";
    /// The one place to type: the word `name`, the field, and the limit.
    pub const LABEL: &str = "label";
    pub const VALUE: &str = "value";
    pub const HINT: &str = "hint";
    /// The detail line: the sentence, and the number that qualifies it.
    pub const SAID: &str = "said";
    pub const FACTS: &str = "facts";
}

/// The order the columns give way in — **the highest goes first**.
///
/// It is a statement about what each screen is FOR, and not a priority in the
/// ordinary sense. On the list: the word naming the filesystem goes before the
/// size, because two sticks are told apart by how big they are and not by both
/// saying `vfat`; then the node, then the name, and the glyph last, because two
/// cells answer "may I write to this" faster than any sentence. Nowhere does
/// the reason a disk must not be touched give way — it is not a column, it is
/// [`rest_from`].
mod give {
    pub const KIND: u8 = 9;
    pub const SIZE: u8 = 8;
    pub const NODE: u8 = 5;
    pub const NAME: u8 = 3;
    pub const ICON: u8 = 1;
    /// The limit beside the field is precision about a rule the field already
    /// enforces: it goes before either of the other two.
    pub const HINT: u8 = 9;
    pub const LABEL: u8 = 2;
    pub const VALUE: u8 = 1;
    /// And so does the exact byte count under the cursor, for the same reason.
    pub const FACTS: u8 = 9;
}

/// A device node: `/dev/nvme0n1p1`, and there is nothing longer.
const NODE: u16 = 14;

/// A filesystem's name. `exfat` is the longest of the five this build writes,
/// and `format-blank` is the word an unwritten partition gets instead.
const KIND: u16 = 8;

/// A size as a person reads it: `1023.9 GiB` is the widest [`size`] returns.
const SIZE: u16 = 10;

/// The narrowest the last column may be squeezed to before a fixed one is
/// dropped instead. `o disco inteiro` is fifteen cells, and it is the longer of
/// the two languages.
const WHAT: u16 = 15;

/// How narrow and how wide the name column may be.
///
/// It is worked out from the widest name ON THE SCREEN and then clamped, rather
/// than fixed: a fixed column would either cut `SanDisk Cruzer Blade` or leave a
/// hand's width of nothing in front of `/dev/sda`, and both read as a bug in
/// the drawing.
const NAME_MIN: u16 = 8;
/// See [`NAME_MIN`].
const NAME_MAX: u16 = 24;

/// The word `name` in front of the one place to type. `nome` and `name`.
const NAME_LABEL: u16 = 8;

/// The field itself: everything [`NAME_LIMIT`] allows, plus the cursor's own
/// cell — the cursor is a glyph and not a colour, so it is visible on a
/// screenshot and to somebody who cannot tell two greys apart.
const NAME_FIELD: u16 = NAME_LIMIT as u16 + 1;

/// The sentence beside the field, at its longest: `up to 11 letters or numbers`
/// is twenty-seven cells and `até 11 letras ou números` is twenty-four.
///
/// A number this file DECLARES, the way `SHEET_LABEL` does: the buffer sends a
/// key and the host translates it (invariant 11), so what it reserved is all
/// anyone can go on. A third language longer than this elides, and there is no
/// check comparing the two — see the report this migration came with.
const NAME_HINT: u16 = 27;

/// How little room the technical sentence may be left with before the number
/// beside it gives way.
///
/// Below this the sentence is elided to a stub and says nothing, which is worse
/// than a screen with no exact byte count: the count is precision added to
/// something the sentence already carries, and the sentence has no second place
/// to be. It is the FLOOR of a fill column now, which is the same two phases
/// spelled out once instead of a comparison written here by hand.
const SENTENCE: u16 = 24;

/// How many rows at the bottom belong to the foot.
///
/// Three: what just happened, what the cursor is on, and what the keys do. The
/// list stops here — it used to reserve two, and the row it took was the one
/// that says a disk is being written.
const FOOT: u16 = 3;

/// The first row the foot owns — where the content has to stop.
const fn foot_top(lines: u16) -> u16 {
    lines.saturating_sub(FOOT)
}

/// The foot's three rows, in order: what happened, what the cursor is on, and
/// what the keys do.
///
/// Derived from [`FOOT`] rather than counted back from the last line by hand,
/// so the rows the foot writes on and the reservation the list makes cannot
/// drift apart — they are the same number, read twice.
const fn foot_rows(lines: u16) -> (u16, u16, u16) {
    let last = lines.saturating_sub(1);
    (
        last.saturating_sub(FOOT - 1),
        last.saturating_sub(FOOT - 2),
        last,
    )
}

/// A run whose colour the design crate's vocabulary has no name for.
///
/// # THE ONE THING THAT WOULD NOT GO INTO `Role`
///
/// [`Role`] is seven parts wide — title, body, quiet, dim, accent, rule, band —
/// and every one of them is a shade of "how loud". This screen needs three that
/// are not: what is HAPPENING, what WENT WELL and what BROKE. They are not
/// decoration here. The reason a disk must not be touched, the line saying a
/// disk is being written, and the word `yes` on the last question before a disk
/// is erased are three places where the colour IS part of the message, and
/// putting them on `accent` would have made `yes` the same colour as `no`.
///
/// So the geometry — every line, every column, every budget and every scale —
/// is the design crate's, and only the colour is put back afterwards. The runs
/// still go through `Screen::put`, so `Screen::check` sees the whole frame. The
/// day `Role` grows a warning, a success and an error, this struct and the loop
/// in [`Draw::finish`] are one deletion.
struct Repaint {
    line: u16,
    column: u16,
    color: Color,
}

/// The frame under construction: the design crate's screen, and the colours it
/// cannot name.
struct Draw {
    screen: Screen<Wit>,
    tints: Vec<Repaint>,
}

impl Draw {
    /// A fresh frame for the context the host just sent.
    fn of(ctx: &Context) -> Self {
        Self {
            screen: Screen::<Wit>::of(ctx),
            tints: Vec::new(),
        }
    }

    /// A run on a given row, in a colour [`Role`] does not have.
    ///
    /// The row does not move: a row of columns is several runs at one line.
    fn tint_at(&mut self, line: u16, run: Run<Icon>, color: Color) {
        self.tints.push(Repaint {
            line,
            column: run.column,
            color,
        });
        self.screen.put(line, run);
    }

    /// The same, on the current row, moving past its whole box afterwards.
    fn tinted(&mut self, run: Run<Icon>, color: Color) {
        let at = self.screen.row();
        self.tints.push(Repaint {
            line: at,
            column: run.column,
            color,
        });
        let cells = self.screen.put(at, run);
        self.screen.skip(cells);
    }

    /// Is there still a row above the foot?
    ///
    /// # PHASE TWO, ON THE VERTICAL
    ///
    /// The rows are a PREFERENCE, exactly as the heading's air is; what never
    /// gives way is that nothing leaks off the screen, because the compositor
    /// does not clip. A row drawn under this line is a row drawn ON the
    /// sentence that says a disk is being written — which is the one line on
    /// this screen that must never be covered.
    ///
    /// `Table` resolves the same two phases across; this is the same question
    /// down, and it is the buffer's because only the buffer knows which of its
    /// rows may be left out.
    fn has_room(&self) -> bool {
        self.screen.row() < foot_top(self.screen.lines())
    }

    /// The frame, as the component returns it.
    ///
    /// A run is found again by `(line, column)`, which identifies one uniquely:
    /// `nothing_overlaps` is exactly the promise that no two runs on a row can
    /// reach each other, so no two can start on the same cell either.
    fn finish(self) -> Vec<Command> {
        let mut out = self.screen.finish_wit();
        for c in &mut out {
            let Command::Text(t) = c else {
                continue;
            };
            if let Some(p) = self
                .tints
                .iter()
                .find(|p| p.line == t.line && p.column == t.column)
            {
                t.color = p.color;
            }
        }
        out
    }
}

/// Where a row's reason begins, and how much of the row is left for it.
///
/// # A SPAN, which is the one thing `Table` cannot declare
///
/// The sentence saying why a disk must not be touched takes the room the
/// filesystem, the size and the word after them would have used — none of the
/// three matters about a disk nobody may write to, and the sentence is the one
/// column that is never dropped. `Row::put` gives a run exactly one column and
/// overwrites anything else, on purpose, so this is arithmetic over the LAYOUT:
/// the first of the three that survived, through to where the table ends.
///
/// The order below is the order they give way in, so the first survivor is
/// always the leftmost one there is, and the span is never a hole.
fn rest_from(layout: &Layout) -> Option<(u16, u16)> {
    let at = [field::KIND, field::SIZE, field::WHAT]
        .into_iter()
        .find_map(|id| layout.field(id))?
        .at;
    Some((at, layout.ends().saturating_sub(at)))
}

/// The disk this question is about, said the way the row above said it.
///
/// One function for two screens: `choose` and `confirm` are asking about the
/// same device, and two `format!`s would be two answers to "what do we call it"
/// that drift the first time either is edited.
fn told(d: &Device) -> String {
    format!("{}  {}  {}", d.name, d.node, size(d.bytes))
}

/// What the cursor is on, said twice — a label above, and the truth below.
///
/// A key for the vocabulary and a string for the machine's own identifiers: a
/// device node and a byte count are the same in every language, and a sentence
/// that had to interpolate them would need the host to word it. This way the
/// buffer draws two fields and neither of them is assembled prose.
struct Detail {
    /// The technical vocabulary, by key.
    said: &'static str,
    /// The node, the exact size, the name to be written — whatever the screen
    /// under the cursor has that is not already on it. Empty is ordinary.
    facts: String,
}

/// The last three lines: what happened, what this is, and what the keys do.
///
/// # Why the detail sits between the two and not beside the hint
///
/// Because it CHANGES as the cursor moves and the hint does not. Two things on
/// one line, one of them still and one of them moving, is a line whose motion
/// reads as a glitch — and the eye that learns to ignore the hint would learn
/// to ignore the detail with it.
///
/// # The sentence and the number are a TABLE now
///
/// They used to be two fields placed by hand, right to left: the number's
/// column computed first because it BOUNDED the other, a constant for the air
/// at the window's border, and a comparison deciding whether the sentence had
/// been left enough room. That is a fill column with a floor and a fixed column
/// that gives way first — the same two phases every grid in this repository
/// resolves in, written once. `EDGE_RIGHT` went with it: the host's margin is
/// already outside `columns`, and a cell reserved inside it was air twice.
///
/// The screenshot that found the original defect (`…dos até 2 §1BO88p4 B`)
/// cannot come back: the number is a column of its own, and two columns of a
/// table do not meet in any language or at any width.
fn foot(draw: &mut Draw, hint: &str, said: Option<&Said>, detail: &Detail) {
    let (message, at, hint_at) = foot_rows(draw.screen.lines());

    // THE DETAIL, IN THE DIMMEST COLOUR THERE IS. That is the whole compromise:
    // somebody who does not know what FAT32 is never has to read this line, and
    // somebody who does never has to guess which of five rows means it.
    let mut table = Table::across(draw.screen.columns()).with(Column::fill(field::SAID, SENTENCE));
    // AN EMPTY COUNT IS NOT A COLUMN OF ZERO CELLS. A column still costs the
    // cell of air after it, so asking for one that will never be drawn lays the
    // sentence out against a width the screen does not have.
    let facts = wide(&detail.facts);
    if facts > 0 {
        table = table.with(Column::fixed(field::FACTS, facts, give::FACTS));
    }
    let layout = table.resolve();
    let mut row = draw.screen.row_of(at, &layout);
    row.put(field::SAID, Run::label(detail.said).quiet());
    row.put(field::FACTS, Run::datum(&detail.facts).quiet());
    let _ = row.done();

    if let Some(what) = said {
        let (run, colour) = match what {
            Said::Working => (Run::label(key::WORKING), Color::StateWarning),
            Said::Done => (Run::label(key::DONE), Color::StateSuccess),
            Said::Trouble(why) => (Run::datum(why), Color::StateError),
        };
        draw.tint_at(message, run, colour);
    }
    draw.screen.put(hint_at, Run::label(hint).quiet());
}

/// A row that is chosen, drawn as chosen — by COLOUR, never by a box or an
/// outline (invariant 15).
fn mark(draw: &mut Draw, line: u16, chosen: bool) {
    if chosen {
        draw.screen.band(line);
    }
}

impl State {
    /// The disks, one per row. *Which one?*
    fn list(&mut self, draw: &mut Draw) {
        let line = draw.screen.heading(Icon::Disk, key::TITLE);

        if self.rack.devices.is_empty() {
            self.list_top = None;
            draw.screen
                .put(line, Run::label(key::EMPTY).playing(Role::Body));
            foot(draw, key::HINT_LIST, self.said(), &self.detail());
            return;
        }

        self.list_top = Some(line);
        // The last three lines belong to the message, the detail and the hint.
        let room = usize::from(foot_top(draw.screen.lines()).saturating_sub(line)).max(1);

        // HOW WIDE THE NAME COLUMN IS, worked out from the widest name on the
        // screen rather than from a constant — see [`NAME_MIN`].
        let name = self
            .rack
            .devices
            .iter()
            .take(room)
            .map(|d| wide(&d.name))
            .max()
            .unwrap_or(0)
            .clamp(NAME_MIN, NAME_MAX);

        // AND WHAT GETS SACRIFICED WHEN THE WINDOW IS NARROW, which is a
        // decision and not an accident.
        //
        // The first draft laid five columns out at fixed offsets and let the
        // last one run off the right edge. On the screen that found it, the
        // machine's own root was a row whose ONLY signal that it could not be
        // touched was a grey glyph — the sentence saying why had fallen off the
        // window. A colour alone is a rule somebody has to already know, which
        // is exactly what this screen exists not to require.
        //
        // So the reason is the one thing that is never dropped: it is not a
        // column at all, it is [`rest_from`], and it takes the room the facts
        // would have used. What the table decides is the order the FACTS give
        // way in, and that order is [`give`].
        let layout = Table::across(draw.screen.columns())
            .with(Column::fixed(field::ICON, ICON, give::ICON))
            .with(Column::fixed(field::NAME, name, give::NAME))
            .with(Column::fixed(field::NODE, NODE, give::NODE))
            .with(Column::fixed(field::KIND, KIND, give::KIND))
            .with(Column::fixed(field::SIZE, SIZE, give::SIZE))
            .with(Column::fill(field::WHAT, WHAT))
            .resolve();

        for (n, d) in self.rack.devices.iter().take(room).enumerate() {
            let at = line + u16::try_from(n).unwrap_or(0);
            mark(draw, at, self.on.as_deref() == Some(d.id.as_str()));

            // MUTED MEANS "NOT ON OFFER", and it is never the only thing that
            // says so: the reason is spelled out on the same row.
            //
            // `Role::Body` and NOT `Role::Quiet`, which is what it was until a
            // VM read the screen back: the dimmest grey under the cursor's own
            // band is the lowest-contrast pair the palette has, and the row went
            // unreadable exactly when somebody had moved onto it to find out
            // why. A row nobody may use still has to be a row somebody can
            // READ.
            let tint = if spare(d) { Role::Title } else { Role::Body };
            let glyph = if !spare(d) {
                Icon::Readonly
            } else if d.removable {
                Icon::DiskRemovable
            } else {
                Icon::Disk
            };

            let mut row = draw.screen.row_of(at, &layout);
            row.put(field::ICON, Run::glyph(glyph).playing(tint));
            row.put(field::NAME, Run::datum(&d.name).playing(tint));
            row.put(field::NODE, Run::datum(&d.node).playing(Role::Body));
            if spare(d) {
                // AN EMPTY `kind` IS A SENTENCE AND NOT A BLANK. A partition
                // with nothing written on it is the most ordinary thing this
                // screen is for, and a gap where the others have a word reads
                // as a row that failed to load.
                let kind = if d.kind.is_empty() {
                    Run::label(key::BLANK)
                } else {
                    Run::datum(&d.kind)
                };
                row.put(field::KIND, kind.playing(Role::Body));
                row.put(field::SIZE, Run::datum(&size(d.bytes)).playing(Role::Body));
                // AND THE LAST COLUMN SAYS WHAT THE ROW IS. `whole` decides
                // which question comes next — a whole disk gets a new partition
                // table and a partition cannot — so it is on the row rather
                // than discovered one screen later.
                let k = if d.whole { key::WHOLE } else { key::PART };
                row.put(field::WHAT, Run::label(k).quiet());
                let _ = row.done();
                continue;
            }
            let _ = row.done();

            // THE REASON, TAKING THE WHOLE REST OF THE ROW — and in the one
            // colour the seven roles do not have. See [`Repaint`].
            if let Some((from, width)) = rest_from(&layout) {
                draw.tint_at(
                    at,
                    Run::datum(&d.needed).at(from).within(width),
                    Color::StateWarning,
                );
            }
        }

        foot(draw, key::HINT_LIST, self.said(), &self.detail());
    }

    /// Five filesystems and a name. *What for?*
    fn choose(&self, draw: &mut Draw) {
        draw.screen.heading(Icon::Disk, key::CHOOSE_TITLE);

        // WHICH DISK, ON THE SCREEN THAT ASKS ABOUT IT. The title cannot say it
        // — a key is looked up whole, with no arguments (invariant 11) — so the
        // name goes on a line of its own, right under it, where somebody who
        // arrived here by pressing `Enter` twice can still see what they chose.
        if let Some(t) = self.target.as_ref()
            && draw.has_room()
        {
            draw.screen.line(Run::datum(&told(t)).playing(Role::Body));
            draw.screen.skip(AIR);
        }

        for (n, (_, text, _)) in RECIPES.iter().enumerate() {
            if !draw.has_room() {
                break;
            }
            let at = draw.screen.row();
            mark(draw, at, n == self.recipe);
            draw.screen.line(Run::label(text).playing(Role::Title));
        }

        // XFS, DRAWN AND REFUSED — and reachable, which is the part that took
        // a second pass. It could have been left off the list and nobody would
        // have written a bug report; what they would have done is look for it,
        // not find it, and wonder whether they were on the wrong screen.
        //
        // The row says the program that writes it is not installed, which is a
        // fact about this distro and not about their disk; the line under the
        // cursor names the program.
        if draw.has_room() {
            let at = draw.screen.row();
            mark(draw, at, self.recipe == MISSING);
            draw.screen
                .put(at, Run::glyph(Icon::Readonly).playing(Role::Body));
            draw.screen.line(
                Run::label(key::RECIPE_MISSING)
                    .playing(Role::Body)
                    .at(ICON + GAP),
            );
            draw.screen.skip(AIR);
        }

        // THE ONE PLACE TO TYPE. The cursor is a glyph and not a colour, so it
        // is visible on a screenshot and to somebody who cannot tell the two
        // greys apart.
        if draw.has_room() {
            let layout = Table::across(draw.screen.columns())
                .with(Column::fixed(field::LABEL, NAME_LABEL, give::LABEL))
                .with(Column::fixed(field::VALUE, NAME_FIELD, give::VALUE))
                .with(Column::fixed(field::HINT, NAME_HINT, give::HINT))
                .resolve();
            let at = draw.screen.row();
            let mut row = draw.screen.row_of(at, &layout);
            row.put(field::LABEL, Run::label(key::NAME).playing(Role::Body));
            row.put(
                field::VALUE,
                Run::datum(&format!("{}_", self.name)).playing(Role::Title),
            );
            row.put(field::HINT, Run::label(key::NAME_LIMIT).quiet());
            let _ = row.done();
        }

        foot(draw, key::HINT_CHOOSE, None, &self.detail());
    }

    /// Two rows, and `no` is the first. *Are you sure?*
    fn confirm(&self, draw: &mut Draw) {
        draw.screen.heading(Icon::Warning, key::CONFIRM_TITLE);

        if let Some(t) = self.target.as_ref()
            && draw.has_room()
        {
            draw.screen.line(Run::datum(&told(t)).playing(Role::Title));
            // AND WHAT "EVERYTHING" MEANS HERE. On a whole disk it is every
            // partition on it, which is more than the row above shows and is
            // exactly the surprise this screen exists to prevent.
            let scope = if t.whole {
                key::CONFIRM_WHOLE
            } else {
                key::CONFIRM_PART
            };
            draw.tinted(Run::label(scope), Color::StateWarning);
            draw.screen.skip(AIR);
        }

        // NO IS FIRST, AND IT IS WHERE THE CURSOR STARTS. Somebody who arrived
        // here by pressing `Enter` three times and presses it a fourth out of
        // rhythm goes back, not ahead. It costs one deliberate keystroke to
        // erase a disk, which is the right price.
        for (n, (text, colour)) in [
            (key::CONFIRM_NO, None),
            (key::CONFIRM_YES, Some(Color::StateError)),
        ]
        .into_iter()
        .enumerate()
        {
            if !draw.has_room() {
                break;
            }
            let at = draw.screen.row();
            mark(draw, at, self.yes == (n == 1));
            let run = Run::label(text).playing(Role::Title);
            // AND `yes` IS RED, which no role names. See [`Repaint`].
            match colour {
                Some(c) => draw.tinted(run, c),
                None => {
                    draw.screen.line(run);
                }
            }
        }

        foot(draw, key::HINT_CONFIRM, None, &self.detail());
    }

    /// Whichever question is on, drawn into the frame.
    ///
    /// One place, so a test asks exactly what `render` asks: the precedence
    /// between the three lives here and nowhere else.
    fn draw(&mut self, draw: &mut Draw) {
        match self.screen {
            Question::List => self.list(draw),
            Question::Choose => self.choose(draw),
            Question::Confirm => self.confirm(draw),
        }
    }

    /// The technical line for whichever row the cursor is on.
    ///
    /// A lookup and never an index, so the row that refuses cannot be a panic:
    /// `MISSING` is deliberately one past the end of [`RECIPES`], and every
    /// reader of `self.recipe` has to cope with it.
    fn tech(&self) -> &'static str {
        RECIPES.get(self.recipe).map_or(key::TECH_MISSING, |r| r.2)
    }

    /// What the cursor is on, in the words of somebody who already knows.
    ///
    /// # One method for three screens, and that is the point
    ///
    /// The detail line is not a caption on the list — it is the answer to "what
    /// does the thing under the cursor DO", and every screen here has a thing
    /// under a cursor. Written per screen, the three would drift: the `choose`
    /// row would name a filesystem and the `confirm` row would name a different
    /// one, because the two would be reading the cursor from different places.
    fn detail(&self) -> Detail {
        match self.screen {
            Question::List => match self.focused() {
                None => Detail {
                    said: key::TECH_NOTHING,
                    facts: String::new(),
                },
                // A DISK THE SYSTEM NEEDS GETS THE TECHNICAL ANSWER TOO. The
                // row already says the system lives there; this says what would
                // happen if the row were pressed anyway — udisks itself would
                // refuse, and this screen does not even get that far.
                Some(d) if !spare(d) => Detail {
                    said: key::TECH_NEEDED,
                    facts: bytes_exactly(d.bytes),
                },
                Some(d) => Detail {
                    said: if d.whole {
                        key::TECH_WHOLE
                    } else {
                        key::TECH_PART
                    },
                    facts: bytes_exactly(d.bytes),
                },
            },
            // THE ONE THE QUESTION WAS ASKED FOR. `abre em qualquer computador`
            // is the honest label and `vfat (FAT32), até 4 GiB por arquivo` is
            // the honest answer; neither replaces the other, and they are one
            // arrow key apart.
            Question::Choose => Detail {
                said: self.tech(),
                facts: self
                    .target
                    .as_ref()
                    .map(|t| t.node.clone())
                    .unwrap_or_default(),
            },
            // AND ON THE LAST SCREEN IT SPELLS OUT THE WHOLE GESTURE: which
            // filesystem, onto which node, under which name. It is the closest
            // thing to the command somebody would have typed, and it is on the
            // screen where being sure is the entire question.
            Question::Confirm => Detail {
                said: self.tech(),
                facts: match self.target.as_ref() {
                    None => String::new(),
                    Some(t) if self.name.is_empty() => t.node.clone(),
                    Some(t) => format!("{} · \"{}\"", t.node, self.name),
                },
            },
        }
    }

    /// The message under the hint, if anything has happened yet.
    const fn said(&self) -> Option<&Said> {
        self.said.as_ref()
    }

    // -----------------------------------------------------------------------
    // Keys.
    // -----------------------------------------------------------------------

    /// Which device the cursor is on, if the rack still has it.
    fn focused(&self) -> Option<&Device> {
        let on = self.on.as_deref()?;
        self.rack.devices.iter().find(|d| d.id == on)
    }

    fn move_by(&mut self, down: bool) {
        match self.screen {
            Question::List => {
                if self.rack.devices.is_empty() {
                    return;
                }
                let at = self
                    .on
                    .as_deref()
                    .and_then(|on| self.rack.devices.iter().position(|d| d.id == on))
                    .unwrap_or(0);
                let last = self.rack.devices.len() - 1;
                let to = if down {
                    if at >= last { 0 } else { at + 1 }
                } else if at == 0 {
                    last
                } else {
                    at - 1
                };
                self.on = Some(self.rack.devices[to].id.clone());
            }
            Question::Choose => {
                // `MISSING` AND NOT `len() - 1`: the walk covers the row that
                // refuses, exactly as the list's walk covers the disk that
                // refuses.
                let last = MISSING;
                self.recipe = if down {
                    if self.recipe >= last {
                        0
                    } else {
                        self.recipe + 1
                    }
                } else if self.recipe == 0 {
                    last
                } else {
                    self.recipe - 1
                };
            }
            // TWO ROWS, SO EITHER ARROW SWAPS THEM. Clamping instead would make
            // `up` on the first row do nothing, and a key that does nothing on
            // a two-row list reads as a key that did not arrive.
            Question::Confirm => self.yes = !self.yes,
        }
    }

    /// `Enter`.
    fn forward(&mut self) -> Vec<Effect> {
        match self.screen {
            Question::List => {
                let Some(d) = self.focused() else {
                    return vec![];
                };
                // A DISK THE SYSTEM NEEDS DOES NOT MOVE FORWARD, and it says
                // nothing new when pressed: the reason is already at the end of
                // the row it is sitting on. A message here would be the same
                // sentence twice, one of them somewhere it has to be dismissed.
                if !spare(d) {
                    return vec![];
                }
                self.target = Some(d.clone());
                self.recipe = 0;
                self.name.clear();
                self.said = None;
                self.screen = Question::Choose;
                vec![Effect::Redraw]
            }
            Question::Choose => {
                // THE THIRD REFUSAL, and the same one twice: a row that names
                // something this build cannot do does not move forward, and it
                // says nothing new when pressed — the reason is already on the
                // row and on the line under the cursor.
                if RECIPES.get(self.recipe).is_none() {
                    return vec![];
                }
                self.yes = false;
                self.screen = Question::Confirm;
                vec![Effect::Redraw]
            }
            Question::Confirm => {
                if !self.yes {
                    self.screen = Question::Choose;
                    return vec![Effect::Redraw];
                }
                let Some(t) = self.target.as_ref() else {
                    self.screen = Question::List;
                    return vec![Effect::Redraw];
                };
                let Some(recipe) = RECIPES.get(self.recipe) else {
                    // UNREACHABLE THROUGH THE KEYS, and refused anyway. `Enter`
                    // on the row that refuses never gets past `choose`, but a
                    // state entered by name from the modeline arrives here
                    // without having walked, and an index one past the end
                    // would be a panic in the component rather than a refusal.
                    self.screen = Question::Choose;
                    return vec![Effect::Redraw];
                };
                let plan = Plan {
                    // THE ID THE HOST GAVE, passed back untouched. This buffer
                    // never builds one and could not: it has no idea what the
                    // string means, which is the point of it being opaque.
                    device: t.id.clone(),
                    recipe: recipe.0,
                    label: self.name.clone(),
                    // A WHOLE DISK GETS A NEW PARTITION TABLE, a partition does
                    // not — and the answer comes off the row rather than off a
                    // question, because it is a fact about the device and not a
                    // preference.
                    whole_disk: t.whole,
                };
                // BACK TO THE LIST, AND NOT TO A SCREEN OF ITS OWN. The row
                // being written is on that list and is about to change; a
                // waiting screen would hide the one thing worth watching. What
                // says a disk is being written is the message line.
                self.screen = Question::List;
                self.said = Some(Said::Working);
                vec![Effect::Format(plan), Effect::Redraw]
            }
        }
    }

    /// `Esc`.
    fn back(&mut self) -> Vec<Effect> {
        match self.screen {
            // ONE LEVEL, AND THE BUFFER IS WHAT WALKS. `Esc` on the first
            // screen is the host's to act on, and it closes the window.
            Question::List => vec![],
            Question::Choose => {
                self.screen = Question::List;
                self.target = None;
                vec![Effect::Redraw]
            }
            Question::Confirm => {
                self.screen = Question::Choose;
                vec![Effect::Redraw]
            }
        }
    }

    /// A letter, which only ever means the name.
    fn letter(&mut self, c: char) -> Vec<Effect> {
        if self.screen != Question::Choose {
            return vec![];
        }
        // WHAT A LABEL MAY HOLD, decided here and not by udisks refusing later.
        // A space or an accent is legal on some of the five and not on others,
        // and a refusal arriving after the confirmation screen would be a
        // refusal for something typed three screens ago.
        if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
            return vec![];
        }
        if self.name.chars().count() >= NAME_LIMIT {
            return vec![];
        }
        self.name.push(c.to_ascii_uppercase());
        vec![Effect::Redraw]
    }

    /// A fresh reading arrived.
    fn racked(&mut self, rack: Rack) -> Vec<Effect> {
        if same_rack(&self.rack, &rack) {
            return vec![];
        }
        self.rack = rack;
        // THE CURSOR SURVIVES BY ID, or lands on the first row somebody can
        // actually use. A stick pulled out while this screen is open takes its
        // row with it, and a cursor pointing at nothing would make `Enter` do
        // nothing with no reason given.
        //
        // FIRST SPARE, AND NOT SIMPLY FIRST. udisks sorts by device node, so
        // on an ordinary machine the first row is the internal disk the system
        // booted from — the one row on the screen that goes nowhere. Starting
        // there means the first key everybody presses does nothing.
        let still = self
            .on
            .as_deref()
            .is_some_and(|on| self.rack.devices.iter().any(|d| d.id == on));
        if !still {
            self.on = self
                .rack
                .devices
                .iter()
                .find(|d| spare(d))
                .or_else(|| self.rack.devices.first())
                .map(|d| d.id.clone());
        }
        vec![Effect::Redraw]
    }

    /// The host answered about the format.
    ///
    /// SUCCESS IS SAID OUT LOUD, even though the row is about to change under
    /// the cursor on its own. A screen that speaks only when something breaks
    /// teaches people that silence means failure, and this one has just spent
    /// three questions earning the opposite.
    fn formatted(&mut self, done: bool, trouble: String) -> Vec<Effect> {
        self.screen = Question::List;
        self.target = None;
        self.said = Some(if done {
            Said::Done
        } else {
            Said::Trouble(trouble)
        });
        vec![Effect::Redraw]
    }

    /// The host asked for a screen by name.
    ///
    /// # Why `choose` and `confirm` take the cursor's disk when they have none
    ///
    /// Because a screen entered from the modeline has nothing behind it, and
    /// the two of them are ABOUT a disk. Falling back to the one under the
    /// cursor is what somebody meant; falling back to nothing would draw a
    /// question with no subject, and `Enter` on it would go back to the list
    /// having said nothing.
    fn enter(&mut self, id: &str) -> Vec<Effect> {
        let to = match id {
            "normal" => Question::List,
            "choose" => Question::Choose,
            "confirm" => Question::Confirm,
            // AN ID THIS BUFFER DID NOT DECLARE. The host only ever sends one
            // it was told, so this is a defect on the other side rather than a
            // person's mistake — and guessing at the nearest screen would hide
            // it.
            _ => return vec![],
        };
        if to != Question::List && self.target.is_none() {
            self.target = self.focused().filter(|d| spare(d)).cloned();
        }
        if to == Question::Confirm {
            self.yes = false;
        }
        self.screen = to;
        vec![Effect::Redraw]
    }

    /// A click does what the key on that row does, and nothing else.
    fn clicked(&mut self, line: u16) -> Vec<Effect> {
        let Some(top) = self.list_top else {
            return vec![];
        };
        if self.screen != Question::List || line < top {
            return vec![];
        }
        let n = usize::from(line - top);
        let Some(d) = self.rack.devices.get(n) else {
            return vec![];
        };
        self.on = Some(d.id.clone());
        self.forward()
    }

    fn act(&mut self, e: Input) -> Vec<Effect> {
        match e {
            Input::Up | Input::Left => {
                self.move_by(false);
                vec![Effect::Redraw]
            }
            Input::Down | Input::Right => {
                self.move_by(true);
                vec![Effect::Redraw]
            }
            Input::Confirm => self.forward(),
            Input::Cancel => self.back(),
            Input::Character(c) => self.letter(c),
            Input::Delete => {
                if self.screen == Question::Choose && self.name.pop().is_some() {
                    vec![Effect::Redraw]
                } else {
                    vec![]
                }
            }
            Input::Click(p) => self.clicked(p.line),
            Input::Rack(r) => self.racked(r),
            Input::Formatted(o) => self.formatted(o.done, o.trouble),
            // THE REST OF THE CONTRACT, answered by ignoring it. The `match` is
            // exhaustive and the WIT is shared by seven components: an `input`
            // case added for one of them has to be answered by all, and here
            // "answered" is "this screen has no use for it".
            // THE HOST ASKS FOR A SCREEN BY NAME — `SPC w s`, or the modeline
            // clicked. Answering it is what makes a declared state a state
            // somebody can reach; a buffer that ignored it would leave the
            // modeline offering screens nobody enters, which is the failure
            // `states-driven` exists to catch.
            Input::EnterState(id) => self.enter(&id),
            Input::Command(_)
            | Input::Copied(_)
            | Input::Paste(_)
            | Input::Phrases(_)
            | Input::Toggle
            | Input::Survey(_)
            | Input::Queue(_)
            | Input::Mind(_)
            | Input::Keyboards(_) => vec![],
        }
    }
}

struct Component;

impl Guest for Component {
    fn init() -> Descriptor {
        Descriptor {
            name: "format".into(),
            mode: "format".into(),
            states: STATES
                .iter()
                .map(|(id, letter, tone)| BufferState {
                    id: (*id).into(),
                    letter: *letter,
                    tone: *tone,
                })
                .collect(),
            // NO KEYMAP, and that is the design rather than an omission. Every
            // answer on every screen is an arrow and `Enter`; a command under
            // `m ` would be a second way to do what `Enter` already does, and a
            // second way is a second thing to keep in step.
            keymap: vec![],
            commands: vec![],
            segments: vec![],
            // THE ONE THAT MATTERS: without it the host neither subscribes to
            // udisks nor forwards a plan, and this is an empty screen.
            capabilities: vec![Capability::Disks],
            // Nothing it holds is text of the interface: everything this buffer
            // shows goes out as a key and the host translates it when it draws.
            phrases: vec![],
        }
    }

    fn render(ctx: Context) -> Vec<Command> {
        STATE.with_borrow_mut(|s| {
            s.lines = ctx.lines;
            let mut draw = Draw::of(&ctx);
            s.draw(&mut draw);
            draw.finish()
        })
    }

    fn event(e: Input) -> Vec<Effect> {
        // THE STATE IS REPORTED FROM ONE PLACE, and not at each site that
        // assigns `screen`. There are nine of those; a report per site is a
        // report somebody forgets, and what it costs is the modeline saying
        // `normal` while a disk is being written.
        STATE.with_borrow_mut(|s| {
            let before = s.state_id();
            let mut out = s.act(e);
            let after = s.state_id();
            if before != after {
                out.push(Effect::StateChanged(after));
            }
            out
        })
    }
}

ronoctua_design::bindings::export!(Component with_types_in ronoctua_design::bindings);

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_design::types::{Heights, Text};

    /// The production heights, as the host computes them.
    ///
    /// `typography.nix`'s five sizes against a line height of 1.45, in cells of
    /// the body's own: `20 × 1.45 = 29 px` is one cell, so `Small`, `Base` and
    /// `Normal` are one and `Large` and `Huge` are TWO. Two is where every
    /// geometry defect in this repository came from, and it is why the tests
    /// below ask the host's own questions instead of counting rows.
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

    /// The three widths the report asks about: a footer's expansion, an
    /// ordinary window, and a wide one.
    const WIDTHS: [u16; 3] = [46, 80, 110];

    /// The id of the disk the running system needs — the one that refuses.
    const NEEDED: &str = "/org/freedesktop/UDisks2/block_devices/root";

    fn device(id: &str, name: &str, node: &str, kind: &str, needed: &str, whole: bool) -> Device {
        Device {
            id: id.into(),
            name: name.into(),
            node: node.into(),
            kind: kind.into(),
            bytes: 32 * 1024 * 1024 * 1024,
            mounted: false,
            removable: !needed.is_empty(),
            needed: needed.into(),
            whole,
        }
    }

    /// The three rows this screen is about: a stick, a partition with nothing
    /// written on it, and the disk the running system lives on.
    fn rack() -> Rack {
        Rack {
            devices: vec![
                device("id-1", "SanDisk Cruzer Blade", "/dev/sda", "vfat", "", true),
                device("id-2", "disk", "/dev/sdb1", "", "", false),
                device(NEEDED, "system", "/dev/nvme0n1p2", "btrfs", "/", false),
            ],
        }
    }

    /// A buffer that has read a rack and is on one of the three questions.
    fn ready(question: Question) -> State {
        let mut s = State::fresh();
        s.racked(rack());
        s.target = s.focused().cloned();
        s.screen = question;
        s
    }

    /// Everything the buffer would send for this state and this window.
    fn frame(s: &mut State, columns: u16, lines: u16) -> Vec<Command> {
        s.lines = lines;
        let mut draw = Draw::of(&context(columns, lines));
        s.draw(&mut draw);
        draw.screen
            .check()
            .unwrap_or_else(|e| panic!("{:?} at {columns}x{lines}: {e}", s.screen));
        draw.finish()
    }

    /// THE TWO QUESTIONS THE HOST ASKS, ASKED OF ALL THREE OF THIS BUFFER'S.
    ///
    /// `Screen::check` is `DisplayList::nothing_is_covered` and
    /// `nothing_overlaps` — the host's own code, not a second copy of it — and
    /// it answers in milliseconds instead of minutes in Nix.
    ///
    /// # Why the sweep starts at seven lines
    ///
    /// Because the list insists on ONE row even when the window has no room
    /// for it (`.max(1)`, which is what the buffer always did): at six lines
    /// the heading's content row and the message line are the same row, and
    /// the disk lands on `format-working`. Seven is the first height where
    /// every state places, and the footer's own expansion ceiling is twelve —
    /// so nothing real is below the floor. See the report this came with.
    #[test]
    fn every_question_survives_the_hosts_two_questions() {
        for columns in WIDTHS {
            for lines in 7..=40u16 {
                for question in [Question::List, Question::Choose, Question::Confirm] {
                    for said in [
                        None,
                        Some(Said::Working),
                        Some(Said::Done),
                        Some(Said::Trouble("udisks refused the request".into())),
                    ] {
                        let mut s = ready(question);
                        s.said = said;
                        s.name = "PENDRIVE".into();
                        let _ = frame(&mut s, columns, lines);
                        // And with the cursor on the row that refuses, which is
                        // where the detail line and the row itself both change.
                        let mut on_refusal = ready(question);
                        on_refusal.on = Some(NEEDED.into());
                        on_refusal.recipe = MISSING;
                        let _ = frame(&mut on_refusal, columns, lines);
                    }
                }
                // A rack that has not arrived yet is a screen too.
                let mut empty = State::fresh();
                let _ = frame(&mut empty, columns, lines);
            }
        }
    }

    /// THE HEADING IS A GLYPH AND A TITLE, AND THERE IS NO RULE.
    ///
    /// It used to be three commands and a blank row: an icon, a key, a
    /// `Command::Rule` on line 1, and content from line 3. What separates a
    /// title from its content is whitespace now, and this file no longer has an
    /// opinion about how much.
    #[test]
    fn the_heading_is_a_glyph_and_a_title_and_no_rule() {
        for question in [Question::List, Question::Choose, Question::Confirm] {
            for columns in WIDTHS {
                let mut s = ready(question);
                let out = frame(&mut s, columns, 24);
                assert!(
                    !out.iter().any(|c| matches!(c, Command::Rule(_))),
                    "{question:?}: a rule came back"
                );
                let Command::Text(glyph) = &out[0] else {
                    panic!("{question:?}: the first command is the heading's glyph");
                };
                let Command::Text(title) = &out[1] else {
                    panic!("{question:?}: the second command is the heading's title");
                };
                assert!(matches!(glyph.content, Text::Icon(_)), "{question:?}");
                assert!(matches!(title.content, Text::Key(_)), "{question:?}");
                assert_eq!(glyph.line, title.line, "{question:?}: one row");
                // AT CELL ZERO, which is the whole of `GUTTER`'s death: the
                // host's margin already moved the buffer in on both sides.
                assert_eq!(glyph.column, 0, "{question:?}");
                assert_eq!(title.column, ICON + GAP, "{question:?}");
            }
        }
    }

    /// NOTHING LEAKS OFF THE SCREEN, field by field.
    ///
    /// `nothing_overlaps` only asks it of the LAST run on a row; this asks it
    /// of every one, which is what catches a budget handed out against a width
    /// the window does not have.
    #[test]
    fn no_field_ends_past_the_columns() {
        for question in [Question::List, Question::Choose, Question::Confirm] {
            for columns in WIDTHS {
                let mut s = ready(question);
                s.name = "PENDRIVE".into();
                s.said = Some(Said::Trouble("udisks refused the request".into()));
                for c in &frame(&mut s, columns, 24) {
                    let Command::Text(t) = c else {
                        continue;
                    };
                    // The host's own accounting: a key claims its whole budget,
                    // data claims its length, a glyph claims its scale in cells.
                    // The length comes from `wide`, which IS the function the
                    // host charges with — one definition, both sides. Writing
                    // the count out again here is how a test comes to agree
                    // with itself and with nothing else.
                    let budget = if t.max_width == 0 {
                        columns.saturating_sub(t.column)
                    } else {
                        t.max_width
                    };
                    let wide = match &t.content {
                        Text::Key(_) => budget,
                        Text::Data(x) => wide(x).min(budget),
                        Text::Icon(_) => budget.min(1),
                    };
                    assert!(
                        t.column.saturating_add(wide) <= columns,
                        "{question:?} at {columns}: {:?} starts at {} and reaches {}",
                        t.content,
                        t.column,
                        t.column.saturating_add(wide)
                    );
                }
            }
        }
    }

    /// NOTHING STARTS AT A GUTTER OF ITS OWN.
    ///
    /// The buffer used to indent everything by two cells inside a window the
    /// host had already indented. Every row now begins at cell zero.
    #[test]
    fn the_left_edge_is_cell_zero() {
        for question in [Question::List, Question::Choose, Question::Confirm] {
            let mut s = ready(question);
            let out = frame(&mut s, 80, 24);
            let leftmost = out
                .iter()
                .filter_map(|c| match c {
                    Command::Text(t) => Some(t.column),
                    _ => None,
                })
                .min();
            assert_eq!(leftmost, Some(0), "{question:?}");
        }
    }

    /// THE FIRST REFUSAL: a disk the running system needs is DRAWN, reachable,
    /// and its `Enter` does not move.
    #[test]
    fn a_disk_the_system_needs_is_drawn_and_refuses() {
        let mut s = ready(Question::List);
        s.on = Some(NEEDED.into());
        // It is on the screen, with the reason beside it — never omitted.
        let out = frame(&mut s, 80, 24);
        assert!(
            out.iter().any(|c| matches!(
                c,
                Command::Text(t) if matches!(&t.content, Text::Data(x) if x == "system")
            )),
            "the disk the system needs fell off the list"
        );
        assert!(
            out.iter().any(|c| matches!(
                c,
                Command::Text(t)
                    if matches!(&t.content, Text::Data(x) if x == "/")
                        && matches!(t.color, Color::StateWarning)
            )),
            "the reason is not on the row, or lost its colour"
        );
        // And pressing it does nothing at all.
        assert!(s.forward().is_empty(), "a needed disk moved forward");
        assert_eq!(s.screen, Question::List);
    }

    /// THE SECOND REFUSAL: the first `Enter` on the confirmation goes BACK,
    /// because `no` is the first row and where the cursor starts.
    #[test]
    fn the_first_enter_on_the_confirmation_goes_back() {
        let mut s = ready(Question::Confirm);
        assert!(!s.yes, "the cursor starts on `no`");
        let out = s.forward();
        assert_eq!(s.screen, Question::Choose, "it went ahead");
        assert!(
            !out.iter().any(|e| matches!(e, Effect::Format(_))),
            "a disk was ordered erased by the first Enter"
        );
        // And `yes` is drawn in the one colour that says what it costs.
        let mut ahead = ready(Question::Confirm);
        ahead.yes = true;
        let frame = frame(&mut ahead, 80, 24);
        assert!(
            frame.iter().any(|c| matches!(
                c,
                Command::Text(t)
                    if matches!(&t.content, Text::Key(k) if k == key::CONFIRM_YES)
                        && matches!(t.color, Color::StateError)
            )),
            "`yes` lost its colour"
        );
    }

    /// THE THIRD REFUSAL: the row naming a filesystem this build cannot write
    /// is on the list, reachable, and does not move forward.
    #[test]
    fn the_row_for_a_missing_program_refuses() {
        let mut s = ready(Question::Choose);
        s.recipe = MISSING;
        let out = frame(&mut s, 80, 24);
        assert!(
            out.iter().any(|c| matches!(
                c,
                Command::Text(t) if matches!(&t.content, Text::Key(k) if k == key::RECIPE_MISSING)
            )),
            "the row that refuses is not drawn"
        );
        assert!(s.forward().is_empty(), "a missing program moved forward");
        assert_eq!(s.screen, Question::Choose);
    }

    /// A rack that has not changed does not redraw, without `PartialEq` on the
    /// generated records.
    #[test]
    fn the_same_reading_twice_draws_once() {
        let mut s = State::fresh();
        assert!(!s.racked(rack()).is_empty(), "the first reading draws");
        assert!(s.racked(rack()).is_empty(), "the same reading draws again");
        let mut other = rack();
        other.devices[0].bytes += 1;
        assert!(
            !s.racked(other).is_empty(),
            "a byte of difference is a redraw"
        );
    }
}
