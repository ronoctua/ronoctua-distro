//! The notification manager — one implementation, two doors.
//!
//! # What it holds, and what it does not
//!
//! It holds a cursor, which pane has it, and whether a sheet is open. It holds
//! no notifications: the records live in the [`ronoctua_notify::Queue`] and the
//! catalogue in the [`ronoctua_notify::Book`], and both are handed in when rows
//! are asked for. That is what lets the footer's region and the window draw the
//! same thing without a second copy going stale.
//!
//! It is the shape `files::View` already has, and for the same reason — see the
//! header of `files_window.rs`.
//!
//! # Three lists, and why the third is not a fourth pane
//!
//! The owner asked for two columns: unread (with the read ones under them, in a
//! quieter tone) on the left, and what a filter suppressed on the right. The
//! third list — every notification and filter the distro ships with, so each can
//! be adjusted — is a different KIND of thing: it is not something that
//! happened, it is something that is configured. It takes the whole width, and
//! `TAB` reaches it.
//!
//! # One column or two
//!
//! Two when there is room, one when there is not — the same layout in the
//! footer's region and in the window, deciding by the width it is given. The
//! cut comes from a token; a number written here would be a visual value
//! outside the tokens (invariant 9).

use ronoctua_notify::presentation::Verdict;
use ronoctua_notify::{Book, Life, Presentation, Record, RecordKey, Show, TOP_LEVEL};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::fields::{Field, Hit as FilesHit, Ink};

/// WHICH COLUMN a key means something in.
///
/// The line is generated from the table, and the table used to be flat: all
/// three columns taught `SPC read`, `d forget` and `RET run its action`. On
/// **Filtered out**, `SPC` returns `Nothing` on purpose — a suppressed
/// notification is already off the count — and `RET` has nothing to run. On
/// the **catalogue**, `SPC` and `d` do nothing and `RET` opens the entry's
/// sheet, which is not "run its action".
///
/// So the line taught, on two of the three columns, keys that do nothing —
/// against this file's own rule, written over [`OF_THE_SHEET`]: one key, one
/// meaning per level, and the line says which level it is on by teaching only
/// that level's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// Every column: the arrows, `TAB`, and `r`, which is about the queue and
    /// not about the row under the cursor.
    Anywhere,
    /// What happened — the only column whose rows can be read or run.
    Happened,
    /// What happened, and what a filter kept out: both hold records, and both
    /// can forget one.
    ARecord,
    /// The catalogue, where a row is a KIND of notification and not one.
    Catalogue,
}

impl Column {
    /// Does a key of this kind mean anything in that column?
    const fn shown(self, pane: Pane) -> bool {
        match self {
            Self::Anywhere => true,
            Self::Happened => matches!(pane, Pane::Happened),
            Self::ARecord => matches!(pane, Pane::Happened | Pane::Suppressed),
            Self::Catalogue => matches!(pane, Pane::Catalogue),
        }
    }
}

/// The commands the MANAGER reaches, with each one's key.
///
/// # Why the keys are a table and not only a `match`
///
/// Because the manual reads it and the orphan check reads it: a command with no
/// path at all is a command nobody uses, and without this list the check would
/// flag these three — rightly, because they have no key in the leader's tree.
///
/// They cannot have one, and that is not an oversight: they act on the FOCUSED
/// ROW, and outside the view there is no focused row to act on. It is exactly
/// the shape `leader::OF_NETWORK_VIEW` already has.
///
/// # And why the letters are bare
///
/// Because with the view open the whole keyboard is its — `d` here forgets a
/// notification and does not fire `LEADER d`, the same way `d` in the clipboard
/// removes an entry. The `C-` family belongs to the file manager, where a bare
/// letter is a QUERY and cannot also be a command.
///
/// # And why the arrows and `TAB` are in it
///
/// Because THE HINT LINE IS THIS TABLE — `View::hint` is generated from it,
/// the way the file manager's is from its own. The window used to carry a
/// sentence of its own in the `.ftl` (`TAB pane · ↑↓ move …`) and the footer
/// carried nothing, which the owner photographed: two doors, two answers. A
/// gesture the line teaches has to be a row the line can read, so the walking
/// keys are here with an id each, and the ORDER is the order the line teaches.
// THE ORDER IS WHAT SURVIVES A NARROW LINE. `chrome::hint_spans` elides across
// pieces, so the tail of this table is what a person never reads — and the two
// that MUST be read are the ones nothing else on screen hints at: the side
// arrows, and what `RET` would do. `TAB` has a column heading lighting up to
// speak for it; `r` is a convenience.
pub const OF_THE_VIEW: &[(&str, &str, Column)] = &[
    ("<down>", "notif-down", Column::Anywhere),
    ("<up>", "notif-up", Column::Anywhere),
    // `RET` OPENS THE RECORD, and that is all it does here. It used to run
    // action ZERO, blind; then it ran whichever button the side arrows had
    // left focused ON the row. The buttons left the row — a notification may
    // carry several with labels that are sentences, and a row is one line —
    // so what reaches an action is the cursor in the open record, where they
    // are listed one under the other.
    ("RET", "notif-see", Column::Happened),
    ("SPC", "notif-read", Column::Happened),
    ("d", "notif-forget", Column::ARecord),
    ("TAB", "notif-pane-next", Column::Anywhere),
    // `S-TAB` is the keysym `ISO_Left_Tab`, which is what xkb produces for
    // it — the same "xkb has already applied it" both doors' key readers
    // rely on when they refuse to count Shift. See `files::OF_THE_VIEW`.
    ("S-TAB", "notif-pane-previous", Column::Anywhere),
    ("r", "notif-read-all", Column::Anywhere),
    // `f` FOR THE FILTERS, from either column of records — on the filters
    // themselves it would be a key that says "go where you are". The catalogue
    // was the third stop of `TAB` and nothing on screen named it, so the whole
    // of it — every kind of notification the distro ships, every filter, and
    // the person's own — existed only for somebody who pressed `TAB` twice and
    // looked.
    ("f", "notif-filters", Column::ARecord),
    // THE SAME KEY, THE OTHER MEANING. On the catalogue `RET` opens the
    // entry's sheet, and calling that "run its action" was the label lying at
    // one of the two levels — the same reason `notif-restore` exists.
    ("RET", "notif-adjust", Column::Catalogue),
    // AND THE TWO THAT MAKE THE FILTERS THE PERSON'S. Writing one had no key
    // at all — the only way in was the JSON — and deleting one had none
    // either, so a filter written by hand could never be taken back by hand.
    ("n", "notif-new", Column::Catalogue),
    ("d", "notif-drop", Column::Catalogue),
    // AND THE ONE THAT SENDS IT TO EVERYBODY, which is a door out of this
    // machine and therefore a question rather than a key that acts.
    ("p", "notif-share", Column::Catalogue),
];

/// The FILTER SHEET's keys — what the line teaches while one is being written.
///
/// A table of its own, like the sheet's and the open record's: the arrows turn
/// what the focused line holds, and three letters do what no arrow can.
pub const OF_THE_DRAFT: &[(&str, &str)] = &[
    ("<down>", "notif-down"),
    ("<up>", "notif-up"),
    ("<right>", "notif-knob-next"),
    ("<left>", "notif-knob-previous"),
    ("RET", "notif-draft-type"),
    ("a", "notif-draft-add"),
    ("x", "notif-draft-drop"),
    ("s", "notif-draft-save"),
];

/// The QUESTION's keys — two answers and a way out.
pub const OF_THE_QUESTION: &[(&str, &str)] = &[
    ("<down>", "notif-down"),
    ("<up>", "notif-up"),
    ("RET", "notif-share-answer"),
];

/// The OPEN record's keys — what the line teaches while one record fills the
/// screen.
///
/// A table of its own, like the sheet's, because two of these mean something
/// else here: the arrows scroll the record instead of walking a list, and `RET`
/// runs the focused button rather than opening anything — there is nothing left
/// to open.
pub const OF_THE_OPEN: &[(&str, &str)] = &[
    ("<down>", "notif-down"),
    ("<up>", "notif-up"),
    ("RET", "notif-act"),
    ("SPC", "notif-read"),
    ("d", "notif-forget"),
];

/// The SHEET's keys — what the line teaches while one entry is being adjusted.
///
/// A second table and not a second half of the first, because two of its keys
/// mean something else here: `r` puts the distro's default back rather than
/// marking everything read, and the arrows turn a knob rather than doing
/// nothing. One key, one meaning per level, and the line says which level it
/// is on by teaching only that level's keys.
pub const OF_THE_SHEET: &[(&str, &str)] = &[
    ("<down>", "notif-down"),
    ("<up>", "notif-up"),
    ("<right>", "notif-knob-next"),
    ("<left>", "notif-knob-previous"),
    ("r", "notif-restore"),
];

/// What ONE drawn row is, when the rows are one column of them.
///
/// # Why the arrangement is written down instead of walked twice
///
/// Because the drawing and the pointer used to work it out separately, and
/// each was right about a different number of titles: both assumed exactly one
/// (`n - 1` in each door), which was true while a narrow screen drew a single
/// pane. Narrow now STACKS the two — "Received" and everything it holds, then
/// "Filtered out" and everything it holds — so there are two titles, a blank
/// line between them, and a row's index says nothing on its own.
///
/// One plan, three readers: [`View::one_column`] draws it, [`View::place_of`]
/// answers the pointer with it, and [`View::visual_cursor`] tells the region
/// which line to scroll to. It is the rule this file's header already states
/// about the two halves of a list disagreeing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// A column's title.
    Heading(Pane),
    /// The blank line before the second title. Air before a heading, which is
    /// the rule the other views already follow.
    Air,
    /// Item `n` of that pane.
    Item(Pane, usize),
    /// The line an EMPTY pane draws in place of its items. It has to be drawn:
    /// a column with nothing under its title is indistinguishable from a key
    /// that did not work.
    Empty(Pane),
}

/// What a row of the catalogue IS.
///
/// It used to be `is a rule: bool`, which answered the wrong question. The one
/// that matters on screen is WHOSE it is: the distro ships entries and filters
/// that can be adjusted and never deleted, and the person writes filters of
/// their own that can be edited, deleted, and — the owner asked for this by
/// name — told apart at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A KIND of notification the distro recognises.
    Entry,
    /// A filter the distro ships.
    Rule,
    /// A filter the person wrote. Theirs to change and to drop.
    Mine,
    /// A filter of theirs that has been SENT to the repository and has not
    /// come back yet.
    ///
    /// It is still local — it has to be, or it would stop working the moment
    /// it was shared — and it stops being this the day a build brings it back
    /// as one of the distro's. See `ronoctua_store::Shared::landed`.
    Shared,
}

impl Kind {
    /// Is it the person's own?
    #[must_use]
    pub const fn is_mine(self) -> bool {
        matches!(self, Self::Mine | Self::Shared)
    }
}

/// Which list has the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    /// What happened: newest first, the read ones in a quieter tone.
    #[default]
    Happened,
    /// What a filter kept off the screen.
    Suppressed,
    /// Every notification and filter the distro ships with.
    Catalogue,
}

impl Pane {
    /// The heading's text key.
    #[must_use]
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Happened => crate::key::NOTIF_COLUMN_HAPPENED,
            Self::Suppressed => crate::key::NOTIF_COLUMN_SUPPRESSED,
            Self::Catalogue => crate::key::NOTIF_COLUMN_CATALOGUE,
        }
    }

    /// The next one, cycling. It is what `TAB` does.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Happened => Self::Suppressed,
            Self::Suppressed => Self::Catalogue,
            Self::Catalogue => Self::Happened,
        }
    }

    /// The one before, cycling. It is what `S-TAB` does.
    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Happened => Self::Catalogue,
            Self::Suppressed => Self::Happened,
            Self::Catalogue => Self::Suppressed,
        }
    }
}

/// A field of the sheet, in the order it is drawn.
///
/// The sheet is how "use the distro's default, or customise" is reached: each
/// row is one field of the presentation, the side arrows change it, and `r`
/// puts it back to what the distro decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    /// Whether and how it shows.
    Show,
    /// How long it stays.
    Life,
    /// Loud or quiet.
    Loud,
    /// `0..=100`.
    Level,
    /// The text alternates once a second.
    Blink,
    /// Declared, and nothing does it. See [`ronoctua_notify::INERT`].
    Vibrate,
    /// Declared, and nothing does it. See [`ronoctua_notify::INERT`].
    Audio,
    /// It arrives already read: it shows, and the bell never counts it.
    AutoRead,
}

impl Knob {
    /// Every one, in the order the sheet draws them.
    pub const ALL: [Self; 8] = [
        Self::Show,
        Self::Life,
        Self::Loud,
        Self::Level,
        Self::Blink,
        Self::Vibrate,
        Self::Audio,
        Self::AutoRead,
    ];

    /// The label's text key.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Show => crate::key::NOTIF_KNOB_SHOW,
            Self::Life => crate::key::NOTIF_KNOB_LIFE,
            Self::Loud => crate::key::NOTIF_LOUD,
            Self::Level => crate::key::NOTIF_LEVEL,
            Self::Blink => crate::key::NOTIF_BLINK,
            Self::Vibrate => crate::key::NOTIF_VIBRATE,
            Self::Audio => crate::key::NOTIF_AUDIO,
            Self::AutoRead => crate::key::NOTIF_AUTO_READ,
        }
    }

    /// The field's name, as the tables and [`ronoctua_notify::INERT`] spell it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Life => "life",
            Self::Loud => "loud",
            Self::Level => "level",
            Self::Blink => "blink",
            Self::Vibrate => "vibrate",
            Self::Audio => "audio",
            Self::AutoRead => "auto-read",
        }
    }

    /// Why this switch does nothing, when it does nothing.
    ///
    /// `None` for the ones that work. It is read from
    /// [`ronoctua_notify::INERT`] and not from a second list here — a switch
    /// that switches nothing off and says nothing is the trap this project has
    /// already paid for once.
    #[must_use]
    pub fn inert(self) -> Option<&'static str> {
        ronoctua_notify::INERT
            .iter()
            .find(|i| i.field == self.id())
            .map(|i| i.why)
    }
}

/// One row the manager draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The pieces, left to right, already placed in cells.
    ///
    /// # Why there is no `focused` beside them
    ///
    /// There was, and NEITHER door read it: the footer's region takes
    /// `r.fields` and the window takes `f.color`. What says the cursor is
    /// here is the colour of the FIELD — `record_line` paints the focused row
    /// in the accent — and a second answer that no pixel comes from is a
    /// second answer that can be wrong without anybody noticing. It was: the
    /// only assertion about it read `!rows[1].focused` and called that "the
    /// cursor lights nothing", which is not what a person sees.
    pub fields: Vec<Field>,
}

/// What the manager wants done, after a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Mark this notification read.
    Read(RecordKey),
    /// Mark it UNREAD again, and put it back on the bell's count.
    Unread(RecordKey),
    /// Forget this notification: out of the record for good.
    Forget(RecordKey),
    /// Mark everything read.
    ReadAll,
    /// Run action `n` of this notification.
    Act(RecordKey, usize),
    /// The person adjusted `id`. `None` puts it back to the distro's default.
    ///
    /// The verdict travels WITH the errand rather than being written here: this
    /// module holds no book, and giving it one would be the second copy that
    /// goes stale — see the header.
    Adjusted(String, Option<Verdict>),
    /// The draft wants something TYPED. Each door asks on its own line —
    /// the footer's minibuffer, the window's header — the question
    /// [`View::typing`] gives, and hands the answer to [`View::typed`].
    Ask(Asking),
    /// Save this filter of the person's own. `was` is the id it replaces.
    Filter {
        /// The id being replaced, when this was an edit.
        was: Option<String>,
        /// What to save.
        rule: ronoctua_notify::Rule,
    },
    /// Drop this filter of the person's own.
    Unfilter(String),
    /// Put this filter of theirs in the REPOSITORY, for everybody.
    ///
    /// Answered `yes` on a screen that said what it means and showed exactly
    /// what would go. What the door does is write one file in the repository —
    /// what publishes it is the owner's own commit.
    Share(String),
    /// Close the manager.
    Close,
}

/// What a draft is waiting for somebody to type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asking {
    /// What the filter is called.
    Name,
    /// What condition `n` compares against.
    Value(usize),
}

impl Asking {
    /// The line a door opens for it — the command whose argument the answer
    /// is, and whose `ask-<id>` is the question drawn on the line.
    ///
    /// TWO IDS AND NOT ONE: the line said the command's label, "answer the
    /// filter sheet's question", which does not say whether a name or a text
    /// is wanted.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Name => "notif-filter-name",
            Self::Value(_) => "notif-filter-value",
        }
    }
}

/// WHERE A DOOR KEEPS THE RECORDS AND THE BOOK — what the manager's errands
/// change, each door its own way: the footer's region asks the notification
/// desk the footer keeps (the queue its band draws, the bus every sender
/// waits on), and a window writes the two files the footer watches. Which
/// errand comes to which of these is written ONCE, in [`carry_out`].
pub trait Desk {
    /// Marks the record read — BY HAND: off the band, and closed on the bus —
    /// or, `read` false, unread again.
    fn read(&mut self, key: RecordKey, read: bool, way: &mut dyn crate::region::Doorway);
    /// Marks every record read that was shown — never a suppressed one.
    fn read_all(&mut self, way: &mut dyn crate::region::Doorway);
    /// Forgets the record: out of the history for good.
    fn forget(&mut self, key: RecordKey, way: &mut dyn crate::region::Doorway);
    /// Runs action `which` of the record ([`deed_of`]).
    fn act(&mut self, key: RecordKey, which: usize, way: &mut dyn crate::region::Doorway);
    /// The person adjusted catalogue entry `id`; `None` puts it back to the
    /// distro's.
    fn adjust(
        &mut self,
        id: String,
        verdict: Option<Verdict>,
        way: &mut dyn crate::region::Doorway,
    );
    /// Saves a filter of the person's own — `was` is the id it replaces.
    fn filter(
        &mut self,
        was: Option<String>,
        rule: ronoctua_notify::Rule,
        way: &mut dyn crate::region::Doorway,
    );
    /// Drops a filter of the person's own, and the adjustment keyed to it.
    fn unfilter(&mut self, id: String, way: &mut dyn crate::region::Doorway);
    /// Puts a filter of the person's in the repository, and says how it went.
    fn share(&mut self, id: String, way: &mut dyn crate::region::Doorway);
}

/// Carries out what the manager asked for, in EITHER door — the question on
/// the door's own line, the close, and what the records and the book are
/// asked, of the door's [`Desk`]. `typing` is what the draft waits for
/// ([`View::typing`], asked before the view is lent). Whether the manager is
/// still open after it.
pub fn carry_out(
    errand: Errand,
    typing: Option<(&'static str, String)>,
    desk: &mut dyn Desk,
    way: &mut dyn crate::region::Doorway,
) -> bool {
    match errand {
        Errand::Nothing => {}
        Errand::Close => {
            way.close();
            return false;
        }
        // THE TEXT IS TYPED ON THE DOOR'S OWN LINE — the footer's minibuffer,
        // a window's header — starting from what the draft holds, and comes
        // back as the answer to the line's own id.
        Errand::Ask(_) => {
            if let Some((id, now)) = typing {
                way.ask(crate::line::Line::new(id, false, Some(&now)));
            }
        }
        Errand::Read(key) => desk.read(key, true, way),
        // PUTTING ONE BACK SAYS NOTHING ON THE BUS: `NotificationClosed` is
        // final, and the sender has acted on it long ago.
        Errand::Unread(key) => desk.read(key, false, way),
        Errand::ReadAll => desk.read_all(way),
        Errand::Forget(key) => desk.forget(key, way),
        Errand::Act(key, which) => desk.act(key, which, way),
        Errand::Adjusted(id, verdict) => desk.adjust(id, verdict, way),
        // THE PERSON'S OWN FILTERS, saved where they already live: the
        // preferences file both doors watch. Editing REPLACES rather than
        // adds — `was` is the id it had before it was renamed.
        Errand::Filter { was, rule } => desk.filter(was, rule, way),
        Errand::Unfilter(id) => desk.unfilter(id, way),
        // INTO THE REPOSITORY, one filter at a time, by name, after a screen
        // that said what it means.
        Errand::Share(id) => desk.share(id, way),
    }
    true
}

// WHAT A BUTTON COMES TO, the action's wire and the shared file are the
// desk's words (`crate::desk`): the footer's band presses the same buttons,
// and the footer names no feature.
pub use crate::desk::{ACT, Deed, SHARED_NOTIFICATIONS, act_of, act_order, copy_it, deed_of};

/// The manager's state.
#[derive(Debug, Clone, Default)]
pub struct View {
    pane: Pane,
    happened: usize,
    suppressed: usize,
    catalogue: usize,
    /// Which of the manager's levels is on screen.
    level: Level,
    /// How far this machine is ahead of UTC, for the dates an open record
    /// shows.
    zone: ronoctua_files::when::Zone,
    /// Where each half of an OPEN record starts: what it says, and its facts.
    ///
    /// TWO, because the halves are two lists side by side and each scrolls on
    /// its own — the owner's words: the message and the data are isolated,
    /// each with its own bar. Clamped when written, against the half's total
    /// and the room (`open_room`).
    open_offsets: [usize; 2],
    /// Where each column of the LIST starts — received and filtered out —
    /// when the two are side by side, each scrolling on its own.
    list_offsets: [usize; 2],
    /// Does the keyboard scroll the FACTS, rather than the message?
    open_facts: bool,
    /// How many lines the door's region can give the open record, heading
    /// included — REMEMBERED where the door measured it (`set_room`), never
    /// asked from here. Zero is "not measured yet", and means no ceiling.
    ///
    /// The region's CAPACITY and not the height it is drawn at: a record
    /// windowed to the height it was last drawn at would never grow again
    /// (a ratchet), and a height that followed the offsets would move the
    /// modeline while somebody scrolls.
    open_room: std::cell::Cell<usize>,
    /// What the draft is waiting for somebody to type, while a door's line
    /// asks for it.
    ///
    /// The VIEW keeps it and not either door: the answer comes back as text
    /// alone, and both doors hand it to [`View::typed`]. The next key that
    /// reaches the view lets it go — while a line asks, every key is the
    /// line's, in both doors.
    asking: Option<Asking>,
}

/// Which of two lists side by side (or stacked): an open record's message
/// and its facts, or the list's "received" and "filtered out".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    /// The left one — what a record says, or what was received.
    First,
    /// The right one — what the manager knows, or what was filtered out.
    Second,
}

impl Half {
    const fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }
}

/// One of an open record's bars, where it is drawn and what it scrolls.
///
/// ONE ANSWER for the door that paints it and the door that grabs it — the
/// rule `paint::bar_box` was written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenBar {
    /// Which half it scrolls.
    pub half: Half,
    /// Its cell, in the rows' own coordinates: a door adds its lead exactly
    /// as it does to a field's `at`, so the bar sits where the fields put it.
    pub column: u16,
    /// Its thumb's arithmetic. `top` is zero: the track starts at [`row`].
    ///
    /// [`row`]: OpenBar::row
    pub scroll: ronoctua_design::Scroll,
    /// The drawn row the half's lines start on, under its heading.
    pub row: usize,
    /// Does the half run to the bottom of the region? Then its track takes
    /// the leftover under the last line, as every list bar does.
    pub last: bool,
}

/// Where one half of an open record is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalfPlace {
    /// Which half.
    pub half: Half,
    /// The drawn row its lines start on (its heading is the row above).
    pub row: usize,
    /// How many of its lines are on screen.
    pub room: usize,
    /// How many lines it has.
    pub total: usize,
    /// Its first cell, in the rows' own coordinates.
    pub from: u16,
    /// One past its last cell, bar included.
    pub to: u16,
}

/// How many cells a half of an open record keeps at its right end for its
/// bar: one of air and the bar's own.
const HALF_BAR: u16 = 2;

/// An open record's bar in PIXELS — where it is painted and where it is
/// grabbed, which are the same rectangle because both doors read this.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarBox {
    /// Which half it scrolls.
    pub half: Half,
    /// The whole track: a press anywhere on it moves the half.
    pub track: (f32, f32, f32, f32),
    /// The thumb, which is what is painted.
    pub thumb: Option<(f32, f32, f32, f32)>,
    /// What it scrolls, for the drag's inverse.
    pub scroll: ronoctua_design::Scroll,
    /// One line, in pixels.
    pub line: f32,
}

impl BarBox {
    /// Is `(x, y)` on the track?
    #[must_use]
    pub fn holds(&self, x: f32, y: f32) -> bool {
        let (tx, ty, tw, th) = self.track;
        x >= tx && x < tx + tw && y >= ty && y < ty + th
    }

    /// Where a pointer at height `y` puts the half: the thumb's inverse.
    #[must_use]
    pub fn offset_at(&self, y: f32) -> usize {
        let (_, top, _, tall) = self.track;
        self.scroll.offset_at(y - top, tall, self.line)
    }
}

/// The bars of an open record, placed.
///
/// `lead` is what the door adds to a field's `at` to reach its cell, `top`
/// the pixel where the region's first row (the headings) begins, and
/// `bottom` the surface's — the last track takes the leftover, as every list
/// bar does (`Scroll::track`).
#[must_use]
pub fn bar_boxes(
    bars: &[OpenBar],
    lead: u16,
    cell: f32,
    line: f32,
    top: f32,
    bottom: f32,
) -> Vec<BarBox> {
    bars.iter()
        .map(|b| {
            let right = f32::from(lead + b.column + 1) * cell;
            #[allow(clippy::cast_precision_loss)]
            let top = top + line * b.row as f32;
            // A HALF WITH ANOTHER UNDER IT stops at its own last line.
            #[allow(clippy::cast_precision_loss)]
            let bottom = if b.last {
                bottom
            } else {
                top + line * b.scroll.lines as f32
            };
            let (track, thumb) = crate::chrome::band_bar(b.scroll, right, top, bottom, cell, line);
            BarBox {
                half: b.half,
                track,
                thumb,
                scroll: b.scroll,
                line,
            }
        })
        .collect()
}

/// Which level of the manager is on screen.
///
/// The shape is the Bluetooth view's, and for the same reason: one field says
/// where you are, one `match` in [`View::rows`] draws it, and `Esc` walks back
/// one step. Neither door knows a level exists — both ask for rows and paint
/// what they get.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Level {
    /// The columns.
    #[default]
    List,
    /// ONE record, whole: its body, its buttons and its facts.
    ///
    /// `line` is which line of it the region is asked to keep on screen — the
    /// arrows move it, because a body that does not fit is exactly the reason
    /// this level exists.
    Open {
        /// Which record.
        at: RecordKey,
        /// Which of its lines the region keeps on screen.
        line: usize,
    },
    /// A catalogue entry being adjusted; the number is the focused knob.
    Sheet(usize),
    /// A filter of the person's OWN being written or edited.
    ///
    /// It is the only level that holds anything but a cursor, and it has to:
    /// a filter half-written is not saved anywhere yet, and a person naming it
    /// on a door's line is a round trip through the door and back.
    Draft(Draft),
    /// The question before a filter goes to EVERYBODY.
    ///
    /// It shows the filter whole — every condition, every verdict — and says
    /// in one sentence that the repository is public and that there is no way
    /// back. `yes` is which of the two answers the cursor is on, and it starts
    /// on `no`: the format buffer's rule, for the same reason, at a gesture
    /// that cannot be undone either.
    Publish {
        /// Which filter of theirs.
        id: String,
        /// Is the cursor on "yes"?
        yes: bool,
    },
}

/// A filter of the person's own, being written.
///
/// # Why this exists at all
///
/// Because there was NO way to write one. The rule language has been complete
/// since the day it was generated — seven fields, five comparisons, the person's
/// own filters tried before the distro's — and the only path to it was editing
/// `$XDG_CONFIG_HOME/ronoctua/notifications.json` by hand, in a JSON of serde
/// variant names (`{"Contains": "backup"}`). That is a feature nobody has.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Draft {
    /// What the person is calling it. It is the rule's id, and it is DATA:
    /// text they typed, not an identifier this repository writes.
    pub name: String,
    /// The conditions. Every one has to hold.
    pub when: Vec<ronoctua_notify::Term>,
    /// What it decides.
    pub apply: Verdict,
    /// Which line of it the arrows are on.
    pub line: usize,
    /// The filter it REPLACES, when this is an edit rather than a new one.
    pub editing: Option<String>,
}

impl Draft {
    /// A blank one, with a first condition already there.
    ///
    /// With none, the first screen is a name and nothing else, and "add a
    /// condition" is a step everybody takes: a filter with no condition matches
    /// EVERYTHING, which is the one thing nobody means to write by accident.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            when: vec![ronoctua_notify::Term {
                field: ronoctua_notify::Field::Summary,
                test: ronoctua_notify::Test::Contains(String::new()),
            }],
            ..Self::default()
        }
    }

    /// The one that edits an existing filter of theirs.
    #[must_use]
    pub fn of(rule: &ronoctua_notify::Rule) -> Self {
        Self {
            name: rule.id.clone(),
            when: rule.when.clone(),
            apply: rule.apply.clone(),
            line: 0,
            editing: Some(rule.id.clone()),
        }
    }

    /// What it would be saved as. `None` while it is not ready.
    ///
    /// A filter needs a NAME — it is how the screen and the adjustment file
    /// both refer to it — and at least one condition with something to compare
    /// against. Half a filter saved is a filter that catches everything.
    #[must_use]
    pub fn ready(&self) -> Option<ronoctua_notify::Rule> {
        let name = self.name.trim();
        if name.is_empty() || self.when.is_empty() {
            return None;
        }
        if self
            .when
            .iter()
            .any(|t| !t.test.is_numeric() && t.test.against().trim().is_empty())
        {
            return None;
        }
        Some(ronoctua_notify::Rule {
            id: name.to_owned(),
            when: self.when.clone(),
            apply: self.apply.clone(),
        })
    }
}

/// What ONE line of the filter sheet is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    /// What the filter is called.
    Name,
    /// What condition `n` looks at.
    Field(usize),
    /// How condition `n` compares.
    Test(usize),
    /// What condition `n` compares against.
    Value(usize),
    /// One of the eight knobs of what it decides.
    Knob(usize),
}

/// What goes in each pane, resolved once so the drawing and the keys agree.
///
/// # Why it is built and not walked twice
///
/// Because the two halves used to disagree: the drawing filtered the records
/// one way and the cursor counted them another, and the row a key acted on was
/// not the row the eye was on. It is the same defect `ExpansionRow` carries a
/// comment about, one level up.
pub struct Lists<'a> {
    /// Newest first — one list, and the read ones in a quieter tone.
    pub happened: Vec<&'a Record>,
    /// What a filter kept off the screen.
    pub suppressed: Vec<&'a Record>,
    /// `(id, what it is)` — the distro's kinds, the distro's filters, and the
    /// person's own filters, in that order.
    pub catalogue: Vec<(String, Kind)>,
}

impl<'a> Lists<'a> {
    /// Splits the records and lays the catalogue out.
    #[must_use]
    pub fn of(records: &'a [Record], book: &Book) -> Self {
        let (suppressed, shown): (Vec<&Record>, Vec<&Record>) = records
            .iter()
            .rev()
            .partition(|r| matches!(r.presentation.show, Show::Hidden));
        // AS THEY ARRIVED, NEWEST AT THE TOP, and nothing else sorts them.
        //
        // The unread were grouped above the read, which was the owner's
        // earlier arrangement and which they replaced from the screen: one
        // read five seconds ago sat BELOW one from yesterday that had not
        // been, so the list did not answer "what just happened" — which is the
        // question somebody opens it with. The two ages are still told apart,
        // by TONE (`record_line`), and a tone costs no position.
        let happened = shown;
        // THE DISTRO'S ENTRIES, THE DISTRO'S RULES, AND THE PERSON'S RULES.
        //
        // `book.mine` was missing, and it is the half a person can actually
        // change: an open record names the filter that decided it, and if that
        // filter was one of theirs the catalogue could neither show it nor
        // adjust it — the screen named something the screen could not reach.
        let catalogue = book
            .entries
            .iter()
            .map(|e| (e.id.clone(), Kind::Entry))
            .chain(book.rules.iter().map(|r| (r.id.clone(), Kind::Rule)))
            .chain(book.mine.iter().map(|r| {
                // ON ITS WAY, or simply theirs. The question is answered
                // against the table this binary was COMPILED with, and that is
                // the whole of the check: distro content reaches a machine by
                // one road, a build from the flake, so being in that table IS
                // having arrived. Nothing here asks the network or git.
                if book.shared.contains(&r.id) {
                    (r.id.clone(), Kind::Shared)
                } else {
                    (r.id.clone(), Kind::Mine)
                }
            }))
            .collect();
        Self {
            happened,
            suppressed,
            catalogue,
        }
    }

    /// The record with this name, in whichever column holds it.
    ///
    /// The two columns are one history split by a filter's verdict, so a name
    /// belongs to at most one of them.
    #[must_use]
    pub fn record(&self, at: RecordKey) -> Option<&'a Record> {
        self.happened
            .iter()
            .chain(&self.suppressed)
            .copied()
            .find(|r| r.key() == at)
    }

    /// How many rows one pane has.
    #[must_use]
    pub fn len(&self, pane: Pane) -> usize {
        match pane {
            Pane::Happened => self.happened.len(),
            Pane::Suppressed => self.suppressed.len(),
            Pane::Catalogue => self.catalogue.len(),
        }
    }
}

/// Where a pane's rows begin and how wide they are, in cells.
///
/// # Why this is one function and not four
///
/// The drawing worked it out inline and the pointer worked it out again, and
/// the two agreed by luck: a bin drawn at one cell and hit-tested at another is
/// a control that does nothing where it appears and something where it does
/// not. It is the divergence `ExpansionRow` carries a comment about, one level
/// up.
///
/// # And why the left pane stops short of the split
///
/// Because its two targets sit at its right end, and with the pane ending
/// exactly at the split they landed against the other column's first cell —
/// reported from the screen as bins that looked like they belonged to the
/// notifications on the right.
///
/// The gap is `space.modeline.segmentGap`, which is the number this system
/// already uses for "two things on one line are two things". The right pane
/// stops `space.files.edgeRight` short of the screen for the reason that
/// token gives: a glyph against the edge reads as a glyph that was cut off.
///
/// # In the ROW's cells, not the screen's
///
/// Both doors draw a row `chrome::FIELDS_LEAD` in and take the same lead off
/// under the pointer, so what the row itself carries is only the GUTTER —
/// the cell a file row begins at. It used to carry the whole inset, and the
/// two doors added the lead on top: every notification landed three cells
/// in, one more than every file, which the owner reported as content sitting
/// "far from the corners". `both_doors_put_content_in_the_same_place` is the
/// arithmetic; [`tests::the_rows_land_where_the_file_managers_do`] is this
/// file's half of it.
#[must_use]
pub fn extent_of(pane: Pane, columns: u16) -> (u16, u16) {
    let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
    // The lead is only on the LEFT; the air at the right end is the row's.
    let end = columns.saturating_sub(crate::chrome::FIELDS_LEAD + tokens::space::FILES_EDGERIGHT);
    if pane == Pane::Catalogue || !two_columns(columns) {
        return (left, end.saturating_sub(left));
    }
    let split = split_at(columns);
    match pane {
        Pane::Suppressed => (split, end.saturating_sub(split)),
        _ => (
            left,
            split.saturating_sub(left + tokens::space::MODELINE_SEGMENTGAP),
        ),
    }
}

/// Where a LIST row's text and glyphs go: the pane's extent, less the cells
/// its own bar takes at the right end when the two columns sit side by side
/// — so no bar is ever drawn over the bin.
#[must_use]
pub fn list_extent(pane: Pane, columns: u16) -> (u16, u16) {
    let (at, room) = extent_of(pane, columns);
    if pane != Pane::Catalogue && two_columns(columns) {
        (at, room.saturating_sub(HALF_BAR))
    } else {
        (at, room)
    }
}

/// Does the width afford two columns?
///
/// The token is the cut and the arithmetic is here: below it the panes stack,
/// above it they sit side by side. It is the same answer in the footer's region
/// and in the window, which is what "one implementation, two doors" has to mean
/// for a layout.
#[must_use]
pub const fn two_columns(columns: u16) -> bool {
    columns >= tokens::space::NOTIFICATIONS_TWOCOLUMNSFROM
}

/// Where the right-hand column starts, in cells.
///
/// The left one is wider: it holds sentences, and the right one holds what was
/// filtered out — which is a reference list, read rarely and never at length.
#[must_use]
pub fn split_at(columns: u16) -> u16 {
    columns * tokens::space::NOTIFICATIONS_SPLITATPCT / 100
}

impl View {
    /// A fresh manager, on the first pane.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// Keeps the cursor on a row that EXISTS.
    ///
    /// # Why it is not enough to clamp when the arrows move
    ///
    /// Because the list shrinks under the cursor without anybody pressing an
    /// arrow: `d` forgets the focused record, and a notification arriving or
    /// expiring changes the length too. With two records and the cursor on
    /// the second, `d` left the cursor at 1 over a list of length 1: no row
    /// was lit, and `d`, `SPC` and `RET` were all dead until an arrow was
    /// pressed. Nothing on screen said why.
    ///
    /// Both doors call it after anything that can change the lists, which is
    /// the same shape `bluetooth::View::settle` has for the same reason.
    pub fn settle(&mut self, lists: &Lists<'_>) {
        let there = lists.len(self.pane);
        if there == 0 {
            self.set_cursor(0);
            return;
        }
        self.set_cursor(self.cursor().min(there - 1));
    }

    /// The panes this arrangement draws, in the order it draws them.
    ///
    /// The CATALOGUE is alone on the screen: it is not something that
    /// happened, it is what is configured, and it has two columns of its own.
    /// The other two travel together — narrow, one under the other.
    const fn stack(&self) -> &'static [Pane] {
        match self.pane {
            Pane::Catalogue => &[Pane::Catalogue],
            _ => &[Pane::Happened, Pane::Suppressed],
        }
    }

    /// What each drawn row is, in the ONE-column arrangement.
    fn plan(&self, lists: &Lists<'_>) -> Vec<Place> {
        let mut out = Vec::new();
        for (i, pane) in self.stack().iter().copied().enumerate() {
            if i > 0 {
                out.push(Place::Air);
            }
            out.push(Place::Heading(pane));
            let n = lists.len(pane);
            if n == 0 {
                out.push(Place::Empty(pane));
            }
            out.extend((0..n).map(|k| Place::Item(pane, k)));
        }
        out
    }

    /// Which pane and which item the DRAWN row `row` holds, if any.
    ///
    /// `cell` is where along the row the pointer is, and it only matters in the
    /// two-column arrangement, where one drawn row carries two notifications.
    /// A title, the blank line and an empty column's sentence answer `None`.
    #[must_use]
    pub fn place_of(
        &self,
        lists: &Lists<'_>,
        columns: u16,
        row: usize,
        cell: Option<u16>,
    ) -> Option<(Pane, usize)> {
        if self.pane != Pane::Catalogue && two_columns(columns) {
            // WIDE: one title, then one row per pair. Which of the two the
            // pointer means is the cell's answer, and the keyboard's is the
            // pane it is already in.
            let pane = match cell {
                Some(c) if c >= split_at(columns) => Pane::Suppressed,
                Some(_) => Pane::Happened,
                None => self.pane,
            };
            // EACH COLUMN AT ITS OWN OFFSET, so a drawn row is a different
            // item in each.
            let k = usize::from(pane == Pane::Suppressed);
            let i = self.list_offsets_now(lists, columns)[k] + row.checked_sub(1)?;
            return (i < lists.len(pane)).then_some((pane, i));
        }
        match self.plan(lists).get(row)? {
            Place::Item(pane, i) => Some((*pane, *i)),
            _ => None,
        }
    }

    /// Which DRAWN row the cursor is on.
    ///
    /// It is what the region scrolls to, through both doors. They each used to
    /// add one for "the heading", which was the whole arrangement written as a
    /// constant — and wrong the moment there were two headings.
    #[must_use]
    pub fn visual_cursor(
        &self,
        lists: &Lists<'_>,
        book: &Book,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> usize {
        match &self.level {
            // THE SHEET IS SHORT BY CONSTRUCTION and has a cursor of its own;
            // the catalogue's index would send the region thirty rows down a
            // list of eight.
            Level::Sheet(knob) => return *knob,
            Level::Draft(d) => return d.line,
            // THE QUESTION IS READ FROM THE ANSWERS. What it is about is
            // drawn above them, and what has to be on screen is the sentence
            // and the two answers — a question one has to scroll to answer is
            // a question that gets answered by whichever key is nearest.
            Level::Publish { id, yes } => {
                let (at, room) = extent_of(Pane::Catalogue, columns);
                return self
                    .publish_rows(id, *yes, book, at, room, texts)
                    .len()
                    .saturating_sub(1);
            }
            // AN OPEN RECORD HAS NO CURSOR — there is nothing to choose. What
            // the arrows move is which of its lines the region keeps on
            // screen, which is what a body that does not fit needs.
            // SPLIT, the region draws a window of each half under one row
            // of headings, and the line the arrows moved is in that window
            // (`open_key` keeps it there) — so the region never scrolls.
            Level::Open { .. } if self.open_split(columns) => return 0,
            Level::Open { line, .. } => return *line,
            Level::List => {}
        }
        if self.pane != Pane::Catalogue && two_columns(columns) {
            let k = usize::from(self.pane == Pane::Suppressed);
            return self.cursor() - self.list_offsets_now(lists, columns)[k].min(self.cursor()) + 1;
        }
        self.plan(lists)
            .iter()
            .position(|p| *p == Place::Item(self.pane, self.cursor()))
            .unwrap_or(0)
    }

    /// Which column has the keyboard.
    #[must_use]
    pub const fn pane(&self) -> Pane {
        self.pane
    }

    /// Where the cursor is, in the pane that has it.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        match self.pane {
            Pane::Happened => self.happened,
            Pane::Suppressed => self.suppressed,
            Pane::Catalogue => self.catalogue,
        }
    }

    /// Can this row be marked read AT ALL?
    ///
    /// # Why the column decides and not the record
    ///
    /// A suppressed record is stored `read: false` — it never reached the
    /// band, so nothing could have read it — and the envelope used to be
    /// drawn for any record that was not read. But `Press::Space` refuses it
    /// on that column ON PURPOSE: it is already off the count, so marking it
    /// read is a gesture with no effect.
    ///
    /// So the mouse did what the keyboard would not, and it did more than
    /// nothing: in the footer it reached `dbus::notification_was_closed(id,
    /// ByHand)`, telling an application the person had dismissed a
    /// notification they were never shown. "The click only does what the key
    /// does" is the rule; this is the one place it was broken.
    const fn can_be_read(pane: Pane) -> bool {
        matches!(pane, Pane::Happened)
    }

    /// Puts the cursor on row `n`, and acts if the click landed on a target.
    ///
    /// # Why the mouse acts here, when it only aims in the transfer panel
    ///
    /// Because these two targets are DRAWN, one per row, and each says what it
    /// does: an open envelope and a bin. The transfer panel's rule — the keys
    /// act, the mouse aims — is about a click on the ROW, where nothing on
    /// screen says which of five destructive things it would run. A click on a
    /// bin is not a guess.
    ///
    /// Clicking anywhere else on the row still only aims.
    pub fn clicked(
        &mut self,
        row: usize,
        cell: Option<u16>,
        lists: &Lists<'_>,
        book: &Book,
        columns: u16,
    ) -> Errand {
        // THE SHEET IS NOT A LIST, and a click in it moves nothing.
        //
        // With an entry open the drawn rows are the SHEET's — one per knob —
        // and this function went on writing `self.catalogue`, which is the
        // index that says WHICH ENTRY is being adjusted. So clicking the
        // sheet's fourth row silently switched the sheet to the fourth entry
        // of the catalogue: the title changed, and `←`/`→` then wrote the
        // adjustment onto a notification kind nobody had opened. The errand
        // was `Nothing`, so nothing on screen said a click had done anything
        // at all.
        if let Level::Open { at, line } = self.level {
            return self.open_clicked(at, line, row, cell, lists, columns);
        }
        if !matches!(self.level, Level::List) {
            return Errand::Nothing;
        }
        // WHAT IS UNDER THE POINTER, asked of the one plan the drawing used.
        //
        // A title, the blank line between the two stacked columns and an empty
        // column's sentence are not rows of a list, and a click on one of them
        // moves nothing — it used to move the cursor to whatever item happened
        // to share the index.
        let Some((pane, i)) = self.place_of(lists, columns, row, cell) else {
            return Errand::Nothing;
        };
        // A CLICK SELECTS, and it selects the thing under the pointer — which
        // includes the column it is in. With two columns on screen a click in
        // the right one used to move the LEFT pane's cursor, and the right
        // column's envelope and bin were out of reach for the mouse until
        // `TAB` had been pressed.
        self.pane = pane;
        self.set_cursor(i);
        let (at, room) = self.row_extent(columns);
        let Some(hit) = cell.and_then(|c| target_at(c, at, room)) else {
            // A CLICK ON THE ROW ITSELF IS ITS `RET` — the owner's rule, the
            // same one the Bluetooth page follows. The two controls at the
            // right edge are asked FIRST, because a click on one of them means
            // that control; everything else on the line is the notification,
            // and clicking a notification opens it.
            return self.open_here(lists, book);
        };
        // THE RECORD IS THE ONE JUST NAMED, not the one at the raw row index.
        // The cursor was clamped and the lookup was not, so a click below the
        // shorter column's last item acted on nothing while the bin under the
        // pointer looked as if it had been pressed.
        let record = match pane {
            Pane::Happened => lists.happened.get(i),
            Pane::Suppressed => lists.suppressed.get(i),
            Pane::Catalogue => None,
        };
        let Some(record) = record else {
            return Errand::Nothing;
        };
        let key = record.key();
        match hit {
            Hit::ToggleRead if !Self::can_be_read(pane) => Errand::Nothing,
            Hit::ToggleRead if record.read => Errand::Unread(key),
            Hit::ToggleRead => Errand::Read(key),
            Hit::Delete => Errand::Forget(key),
        }
    }

    /// A click on an open record: it SELECTS an action, and the second click
    /// on the same one runs it — the `RET` of that line (invariant 19). A
    /// click in a half gives that half the keyboard.
    fn open_clicked(
        &mut self,
        at: RecordKey,
        line: usize,
        row: usize,
        cell: Option<u16>,
        lists: &Lists<'_>,
        columns: u16,
    ) -> Errand {
        let Some(r) = lists.record(at) else {
            return Errand::Nothing;
        };
        let plan = self.open_plan(r, columns);
        let half = self
            .half_at(lists, columns, row, cell.unwrap_or(0))
            .unwrap_or(Half::First);
        self.open_facts = half == Half::Second;
        let [said, _] = self.places_of(r, columns);
        if half == Half::Second {
            return Errand::Nothing;
        }
        let told = row
            .checked_sub(said.row)
            .filter(|i| *i < said.room)
            .map(|i| self.half_offset(Half::First, lists, columns) + i)
            .and_then(|n| plan.get(n).map(|t| (n, *t)));
        match told {
            Some((n, Told::Act(k))) if n == line => Errand::Act(at, k),
            Some((n, Told::Act(_))) => {
                self.level = Level::Open { at, line: n };
                Errand::Nothing
            }
            _ => Errand::Nothing,
        }
    }

    /// What `RET` opens on the list — and what a CLICK on a row opens.
    ///
    /// One function and not two. The owner's rule is that a click on a row is
    /// that row's `RET`, and two copies of "what Enter does here" are two
    /// answers waiting to differ: the day the catalogue learns a third kind of
    /// entry, one of the copies learns it.
    fn open_here(&mut self, lists: &Lists<'_>, book: &Book) -> Errand {
        match self.pane {
            // On the catalogue it opens the sheet: that is where the distro's
            // default is either kept or changed.
            Pane::Catalogue if self.cursor() < lists.len(Pane::Catalogue) => {
                // A FILTER OF THEIRS OPENS WHOLE, and the distro's opens its
                // knobs. The distro's cannot be rewritten — it comes from
                // `assets/notifications.nix` and a rebuild would put it back —
                // so what a person does to one is adjust it.
                self.level = match self.mine_of(lists, book) {
                    Some(rule) => Level::Draft(Draft::of(&rule)),
                    None => Level::Sheet(0),
                };
                Errand::Nothing
            }
            Pane::Happened => match lists.happened.get(self.cursor()) {
                // THE RECORD OPENS. It is the answer to the row that did not
                // fit: the body wrapped, every action listed one under the
                // other, and the facts beside it.
                Some(r) => {
                    self.open(r.key());
                    Errand::Nothing
                }
                None => Errand::Nothing,
            },
            // A SUPPRESSED ONE DOES NOT OPEN, by `RET` or by click: what there
            // is to say about it is which filter kept it off the screen, and
            // that is the catalogue's column.
            _ => Errand::Nothing,
        }
    }

    /// What the message line says about the row the POINTER is on.
    ///
    /// The two things a record's row cannot hold: WHEN it arrived — a line
    /// with a sentence on it has no room for a date — and the WHOLE of that
    /// sentence, which `elide_end` cut to fit. The owner asked for both from
    /// the screen.
    ///
    /// # Why this is not a field of [`Row`]
    ///
    /// Because one drawn row is not one record. Side by side the two panes
    /// SHARE a line — `happened[i]` on the left, `suppressed[i]` on the right
    /// — so what the pointer is over depends on the CELL as much as on the
    /// row, and a row carrying one answer would say the left column's date
    /// over the right column's notification. [`Self::place_of`] is the one
    /// plan that resolves both, and it is what a click already asks.
    #[must_use]
    pub fn about_at(
        &self,
        lists: &Lists<'_>,
        columns: u16,
        row: usize,
        cell: Option<u16>,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<String> {
        // ONLY THE LIST. With a record open, a sheet up or a question waiting,
        // the drawn rows are not the list's and `place_of` would resolve them
        // against a plan nobody drew.
        if !matches!(self.level, Level::List) {
            return None;
        }
        let (pane, i) = self.place_of(lists, columns, row, cell)?;
        let r = match pane {
            Pane::Happened => lists.happened.get(i)?,
            Pane::Suppressed => lists.suppressed.get(i)?,
            // A FILTER IS NOT A NOTIFICATION: it has no arrival and its name
            // is drawn whole. Saying nothing is the honest answer.
            Pane::Catalogue => return None,
        };
        // ON ONE OF THE GLYPHS, what it is for — the owner's ask: three
        // drawings at a row's end and nothing saying which does what.
        let (at, room) = list_extent(pane, columns);
        let drawn = |g: Glyph| match g {
            Glyph::Read => Self::can_be_read(pane),
            Glyph::Delete => true,
        };
        if let Some(g) = cell
            .and_then(|c| glyph_at(c, at, room))
            .filter(|g| drawn(*g))
        {
            return Some(texts.t(match g {
                Glyph::Read if r.read => crate::key::NOTIF_GLYPH_UNREAD,
                Glyph::Read => crate::key::NOTIF_GLYPH_READ,
                Glyph::Delete => crate::key::NOTIF_GLYPH_DELETE,
            }));
        }
        Some(self.about(r, texts))
    }

    /// The sentence itself. Split out so a test can ask for it without a
    /// pointer, and so the two doors cannot word it differently.
    fn about(&self, r: &Record, texts: &ronoctua_i18n::Texts) -> String {
        let when = crate::chrome::when_text(arrived_secs(r), &self.zone, true, texts);
        texts.t_with(
            crate::key::NOTIF_ABOUT_RECORD,
            &[("when", &when), ("line", &r.notification.line())],
        )
    }

    /// What the message line says about the focused row.
    ///
    /// `None` when there is nothing to add. Today it is the reason a switch
    /// does nothing — see the note in `sheet_rows`. It is NOT the hint line:
    /// that is [`View::hint`], and this wins over it on the same line, the
    /// way a pointer's explanation wins over the keys everywhere else.
    #[must_use]
    pub fn note(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        // AND WHY `s` DID NOTHING, when the draft is not ready to be saved.
        // Same line, same reason it exists: a key that answers nothing has to
        // say why, or the answer is "this screen is broken".
        if let Some(said) = self.draft_note(texts) {
            return Some(said);
        }
        let Level::Sheet(knob) = self.level else {
            return None;
        };
        Some(texts.t(Knob::ALL.get(knob)?.inert()?))
    }

    /// The hint line, as pieces the two doors paint identically — and WHERE
    /// IN THE LIST the cursor is.
    ///
    /// GENERATED from the level's table (`OF_THE_VIEW` on a list,
    /// `OF_THE_SHEET` with an entry open), for the reason `files::View::hint`
    /// gives at length: a sentence kept by hand beside the table is a
    /// sentence that falls behind, and this one had — the window taught six
    /// keys out of a `.ftl` value and the footer taught none.
    ///
    /// # Why the counter is on the line
    ///
    /// The footer's region draws a scrollbar when the content is taller than
    /// the page (`Footer::has_bar`). The window draws no rectangle at all —
    /// it is text and a colour, one render pass — so with four hundred
    /// records in the history it gave no sign there was anything below the
    /// last drawn row. Both doors pass it, because "the two doors show the
    /// same thing" is the owner's requirement and a counter on one of them is
    /// two answers again.
    ///
    /// A COUNTER AND NOT A BAR: adding a quad pipeline to this window for one
    /// rectangle is a lot of machinery for "there is more", and the band's
    /// own `2/5` is the convention this system already has for saying it in
    /// text. Both doors call this, so the line reads the same in each.
    #[must_use]
    pub fn hint_of(
        &self,
        of: Option<(usize, usize)>,
        lists: &Lists<'_>,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let mut out = self.taught(lists, texts);
        // MORE THAN ONE, or there is nothing to place: the band's rule, and
        // the same reason — `1/1` is a column spent saying nothing.
        //
        // AND ONLY WHERE THERE IS A LIST. With one record open or one entry
        // being adjusted, "3 of 12" is about a list nobody is looking at.
        let of = of.filter(|_| matches!(self.level, Level::List));
        if let Some((at, total)) = of.filter(|(_, total)| *total > 1) {
            out.push(crate::chrome::Piece::separator());
            out.push(crate::chrome::Piece::prose(texts.t_args(
                crate::key::NOTIF_WHERE,
                &[
                    (
                        "at",
                        ronoctua_i18n::Arg::Number(i64::try_from(at + 1).unwrap_or(1)),
                    ),
                    (
                        "total",
                        ronoctua_i18n::Arg::Number(i64::try_from(total).unwrap_or(0)),
                    ),
                ],
            )));
        }
        out
    }

    /// The keys the level teaches, and nothing else.
    fn taught(&self, lists: &Lists<'_>, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        match &self.level {
            Level::Sheet(_) => crate::chrome::hint_of(OF_THE_SHEET, texts),
            Level::Open { .. } => crate::chrome::hint_of(OF_THE_OPEN, texts),
            Level::Draft(_) => crate::chrome::hint_of(OF_THE_DRAFT, texts),
            Level::Publish { .. } => crate::chrome::hint_of(OF_THE_QUESTION, texts),
            Level::List => {
                let here: Vec<(&str, &str)> = OF_THE_VIEW
                    .iter()
                    .filter(|(_, _, c)| c.shown(self.pane))
                    .map(|(k, id, _)| (*k, *id))
                    .collect();
                crate::chrome::hint_of_with(&here, |id| self.means(id, lists), texts)
            }
        }
    }

    /// What a key of MOVING meaning says right now.
    ///
    /// `None` for every key that means one thing. `RET` is the one that does
    /// not: on the row itself it opens the record, and on a button — where the
    /// side arrows may have left it — it runs that button. The hint line draws
    /// what this returns, and draws the key in the register that says its
    /// meaning moves.
    fn means(&self, id: &str, lists: &Lists<'_>) -> Option<&'static str> {
        match id {
            // ADJUSTING IS FOR THE DISTRO'S; a filter the person WROTE is
            // opened and rewritten whole, conditions and all.
            "notif-adjust" => self.mine_here(lists).is_some().then_some("notif-edit"),
            _ => None,
        }
    }

    /// The filter behind the focused catalogue row, when it is theirs.
    fn mine_of(&self, lists: &Lists<'_>, book: &Book) -> Option<ronoctua_notify::Rule> {
        let id = self.mine_here(lists)?;
        book.mine.iter().find(|r| r.id == id).cloned()
    }

    /// The focused catalogue row, when it is a filter of the person's own.
    fn mine_here<'a>(&self, lists: &'a Lists<'_>) -> Option<&'a str> {
        if self.pane != Pane::Catalogue {
            return None;
        }
        match lists.catalogue.get(self.cursor()) {
            Some((id, Kind::Mine)) => Some(id.as_str()),
            _ => None,
        }
    }

    /// Is an entry's sheet open?
    ///
    /// Read by the door that has to know whether `notif-restore` has anything
    /// to restore: outside the sheet the same letter marks everything read,
    /// and a command run by name must not do the other thing.
    #[must_use]
    pub const fn in_sheet(&self) -> bool {
        matches!(self.level, Level::Sheet(_))
    }

    /// A fresh view at a chosen column, keeping what the DOOR gave this one.
    ///
    /// Opening the manager builds a new view — the cursor should not be where
    /// it was three days ago — and the clock is not part of that: it came from
    /// the door at start-up, and a plain `fresh()` here quietly threw it away,
    /// so every date an open record showed would have read UTC.
    ///
    /// The column is a parameter because `n f` opens ON the filters: a feature
    /// reachable only by pressing `TAB` twice inside a screen one has already
    /// opened is a feature nobody finds.
    #[must_use]
    pub fn reopened(&self, pane: Pane) -> Self {
        Self {
            pane,
            zone: self.zone.clone(),
            ..Self::fresh()
        }
    }

    /// Opens on ONE record, whole — the band's `Hit::Open`.
    ///
    /// The cursor goes to its row too, so `Esc` lands on it in the list and
    /// not on whatever row the manager was last left at. A record the list
    /// does not have (it aged out between the click and this) leaves the list
    /// as it is.
    pub fn open_on(&mut self, at: RecordKey, lists: &Lists<'_>) {
        let Some(n) = lists.happened.iter().position(|r| r.key() == at) else {
            return;
        };
        self.pane = Pane::Happened;
        self.set_cursor(n);
        self.open(at);
    }

    /// Opens a record at its top, with the keyboard on what it says.
    fn open(&mut self, at: RecordKey) {
        self.level = Level::Open { at, line: 0 };
        self.open_offsets = [0, 0];
        self.open_facts = false;
    }

    /// How many lines the door's region can give an open record, heading
    /// included. See `open_room`.
    pub fn set_room(&self, lines: usize) {
        self.open_room.set(lines);
    }

    /// How many lines the door can give, or no ceiling before it measured.
    fn capacity(&self) -> usize {
        match self.open_room.get() {
            0 => usize::MAX,
            n => n,
        }
    }

    /// Is an open record on screen? Its two halves then scroll on their own
    /// and the region does not.
    #[must_use]
    pub fn open_split(&self, columns: u16) -> bool {
        match self.level {
            Level::Open { .. } => true,
            // THE LIST'S TWO COLUMNS, side by side: received and filtered
            // out are two lists, each with its own offset and bar.
            Level::List => self.pane != Pane::Catalogue && two_columns(columns),
            _ => false,
        }
    }

    /// The list's own layout when its two columns sit side by side.
    fn list_places(&self, lists: &Lists<'_>, columns: u16) -> [HalfPlace; 2] {
        // UNDER THE ROW OF HEADINGS, as tall as the longer column up to the
        // room — never a function of the offsets.
        let room = lists
            .happened
            .len()
            .max(lists.suppressed.len())
            .max(1)
            .min(self.capacity().saturating_sub(1).max(1));
        let place = |half: Half, pane: Pane| {
            let (from, wide) = extent_of(pane, columns);
            HalfPlace {
                half,
                row: 1,
                room,
                total: lists.len(pane).max(1),
                from,
                to: from + wide,
            }
        };
        [
            place(Half::First, Pane::Happened),
            place(Half::Second, Pane::Suppressed),
        ]
    }

    /// The offsets of whichever two lists are on screen.
    const fn offsets(&self) -> &[usize; 2] {
        match self.level {
            Level::List => &self.list_offsets,
            _ => &self.open_offsets,
        }
    }

    /// The same, to write.
    const fn offsets_mut(&mut self) -> &mut [usize; 2] {
        match self.level {
            Level::List => &mut self.list_offsets,
            _ => &mut self.open_offsets,
        }
    }

    /// Each list column's first row as drawn, clamped to what it has.
    fn list_offsets_now(&self, lists: &Lists<'_>, columns: u16) -> [usize; 2] {
        let p = self.list_places(lists, columns);
        [
            self.list_offsets[0].min(p[0].total.saturating_sub(p[0].room)),
            self.list_offsets[1].min(p[1].total.saturating_sub(p[1].room)),
        ]
    }

    /// The list keeps the cursor inside its own column's window — the region
    /// does not scroll, the column does.
    fn follow_list(&mut self, lists: &Lists<'_>, columns: u16) {
        if !(matches!(self.level, Level::List) && self.open_split(columns)) {
            return;
        }
        let half = if self.pane == Pane::Suppressed {
            Half::Second
        } else {
            Half::First
        };
        let p = self.list_places(lists, columns)[half.index()];
        let now = self.half_offset(half, lists, columns);
        let c = self.cursor();
        let to = if c < now {
            c
        } else if c >= now + p.room {
            c + 1 - p.room
        } else {
            now
        };
        self.open_scroll_to(half, to, lists, columns);
    }

    /// Where each list's COLUMN divider goes when two sit side by side, and
    /// whether it is lit — lit when the keyboard is in the SECOND one, which
    /// is the file manager's own mark (`chrome::divider_lit`).
    #[must_use]
    pub fn divider(&self, columns: u16) -> Option<(u16, bool)> {
        let side_by_side = match self.level {
            Level::Open { .. } => facts_beside(columns),
            Level::List => self.open_split(columns),
            _ => false,
        };
        let lit = match self.level {
            Level::Open { .. } => self.open_facts,
            _ => self.pane == Pane::Suppressed,
        };
        side_by_side.then(|| (split_at(columns).saturating_sub(1), lit))
    }

    /// The cells a hover band may cover on drawn row `row` under cell
    /// `column`: the half the pointer is in, side by side — a band across
    /// both would say the two lists were one — and the whole row otherwise.
    #[must_use]
    pub fn hover_span(
        &self,
        lists: &Lists<'_>,
        columns: u16,
        row: usize,
        column: u16,
    ) -> Option<(u16, u16)> {
        let (split, _) = self.divider(columns)?;
        let half = self.half_at(lists, columns, row, column)?;
        let p = self.places(lists, columns)?[half.index()];
        Some(match half {
            Half::First => (0, split),
            Half::Second => (split + 1, p.to.max(split + 1)),
        })
    }

    /// Where each half of the open record is drawn: SIDE BY SIDE under one
    /// row of headings, or STACKED — the message first, the facts under it —
    /// each under its own. One answer for the drawing, the bars, the wheel,
    /// the keys and the clicks.
    #[must_use]
    pub fn places(&self, lists: &Lists<'_>, columns: u16) -> Option<[HalfPlace; 2]> {
        match self.level {
            Level::Open { at, .. } => Some(self.places_of(lists.record(at)?, columns)),
            Level::List if self.open_split(columns) => Some(self.list_places(lists, columns)),
            _ => None,
        }
    }

    /// [`Self::places`] for a record already in hand.
    fn places_of(&self, r: &Record, columns: u16) -> [HalfPlace; 2] {
        let said = self.open_plan(r, columns).len();
        let (s_from, s_wide) = self.open_extent(columns, false);
        let (f_from, f_wide) = self.open_extent(columns, true);
        let capacity = self.capacity();
        if facts_beside(columns) {
            // ONE ROW OF HEADINGS, then as many as the taller half, up to the
            // room — never a function of the offsets, or the region would
            // shrink under the wheel and move the modeline.
            let room = said.max(FACTS).min(capacity.max(1));
            return [
                HalfPlace {
                    half: Half::First,
                    row: 0,
                    room: room.min(said.max(1)),
                    total: said,
                    from: s_from,
                    to: s_from + s_wide,
                },
                HalfPlace {
                    half: Half::Second,
                    row: 0,
                    room: room.min(FACTS),
                    total: FACTS,
                    from: f_from,
                    to: f_from + f_wide,
                },
            ];
        }
        // STACKED: two headings, and the lines between them shared. The
        // facts keep what they need when there is room for them, and the
        // message takes the rest — it is what the record was opened to read.
        // ONE LINE OF AIR between them, and no titles: the owner's word.
        let lines = capacity.saturating_sub(1).max(2);
        let facts_room = if said + FACTS <= lines {
            FACTS
        } else {
            FACTS.min(lines.saturating_sub(3).max(1))
        };
        let said_room = said.min(lines.saturating_sub(facts_room)).max(1);
        [
            HalfPlace {
                half: Half::First,
                row: 0,
                room: said_room,
                total: said,
                from: s_from,
                to: s_from + s_wide,
            },
            HalfPlace {
                half: Half::Second,
                row: said_room + 1,
                room: facts_room,
                total: FACTS,
                from: f_from,
                to: f_from + f_wide,
            },
        ]
    }

    /// Which half of an open record drawn row `row`, cell `column` is in.
    #[must_use]
    pub fn half_at(
        &self,
        lists: &Lists<'_>,
        columns: u16,
        row: usize,
        column: u16,
    ) -> Option<Half> {
        let [_, second] = self.places(lists, columns)?;
        // SIDE BY SIDE by the column, STACKED by the row — the divider is
        // the one answer to which of the two it is.
        Some(if self.divider(columns).is_some() {
            if column >= split_at(columns) {
                Half::Second
            } else {
                Half::First
            }
        } else if row + 1 >= second.row {
            Half::Second
        } else {
            Half::First
        })
    }

    /// Where a sweep of text that starts on drawn row `row`, cell `column` of
    /// an open record may go: the lines of the half it started in, and only
    /// them — the message and its facts are two texts. `lead` is the cell a
    /// field's `at` counts from, and `first` the surface line of row 0.
    /// `None` off an open record, where the door's whole list is the area.
    #[must_use]
    pub fn sweep_area(
        &self,
        lists: &Lists<'_>,
        columns: u16,
        row: usize,
        column: u16,
        lead: u16,
        first: u16,
    ) -> Option<crate::text_select::Area> {
        let half = self.half_at(lists, columns, row, column.saturating_sub(lead))?;
        let p = self.places(lists, columns)?[half.index()];
        let top = first.saturating_add(u16::try_from(p.row).unwrap_or(u16::MAX));
        Some(crate::text_select::Area {
            left: lead + p.from,
            right: lead + p.to,
            top,
            bottom: top.saturating_add(u16::try_from(p.room).unwrap_or(u16::MAX)),
        })
    }

    /// Where a half starts, clamped to what it has and what is on screen.
    fn half_offset(&self, half: Half, lists: &Lists<'_>, columns: u16) -> usize {
        self.places(lists, columns).map_or(0, |p| {
            let p = p[half.index()];
            self.offsets()[half.index()].min(p.total.saturating_sub(p.room))
        })
    }

    /// Puts a half's first line at `to`, clamped. The bar's drag.
    pub fn open_scroll_to(&mut self, half: Half, to: usize, lists: &Lists<'_>, columns: u16) {
        let Some(p) = self.places(lists, columns) else {
            return;
        };
        let p = p[half.index()];
        self.offsets_mut()[half.index()] = to.min(p.total.saturating_sub(p.room));
    }

    /// The wheel over drawn row `row`, cell `column` of an open record: the
    /// half under the pointer scrolls, and only it. `true` if the wheel was
    /// the open record's to take.
    pub fn open_wheel(
        &mut self,
        row: usize,
        column: u16,
        lines: i64,
        lists: &Lists<'_>,
        columns: u16,
    ) -> bool {
        let Some(half) = self.half_at(lists, columns, row, column) else {
            return false;
        };
        let now = self.half_offset(half, lists, columns);
        let to = usize::try_from(
            i64::try_from(now)
                .unwrap_or(i64::MAX)
                .saturating_add(lines)
                .max(0),
        )
        .unwrap_or(0);
        self.open_scroll_to(half, to, lists, columns);
        true
    }

    /// The bars of an open record, one per half that does not fit.
    #[must_use]
    pub fn open_bars(&self, lists: &Lists<'_>, columns: u16) -> Vec<OpenBar> {
        let Some(places) = self.places(lists, columns) else {
            return Vec::new();
        };
        let side_by_side = facts_beside(columns);
        places
            .into_iter()
            .filter(|p| p.total > p.room)
            .map(|p| OpenBar {
                half: p.half,
                column: p.to.saturating_sub(1),
                scroll: ronoctua_design::Scroll {
                    total: p.total,
                    lines: p.room,
                    offset: self.half_offset(p.half, lists, columns),
                    top: 0,
                },
                row: p.row,
                last: side_by_side || p.half == Half::Second,
            })
            .collect()
    }

    /// Hands it the machine's clock, for the dates an open record shows.
    ///
    /// Carried and not found, which is the rule this repository already
    /// follows for the trash bin and the sidebar's store: a `View` built by a
    /// test reads nobody's clock, and the two doors hand it the real one.
    #[must_use]
    pub fn in_zone(mut self, zone: ronoctua_files::when::Zone) -> Self {
        self.zone = zone;
        self
    }

    /// Where the focused pane's rows begin and how wide they are.
    ///
    /// One place, read by the drawing and by the pointer — two answers to
    /// "where does this row end" is how a bin comes to be one cell off.
    #[must_use]
    pub fn row_extent(&self, columns: u16) -> (u16, u16) {
        list_extent(self.pane, columns)
    }

    fn set_cursor(&mut self, n: usize) {
        match self.pane {
            Pane::Happened => self.happened = n,
            Pane::Suppressed => self.suppressed = n,
            Pane::Catalogue => self.catalogue = n,
        }
    }
}

/// The presentation an id has in force, and the distro's for the same id.
///
/// Two, and not one: the manager draws BOTH, because "use the default or
/// customise" is a comparison, and a screen that only shows the result cannot
/// say which of the two you are looking at.
#[must_use]
pub fn in_force(book: &Book, id: &str) -> (Presentation, bool) {
    let mut p = Presentation::from_urgency(ronoctua_notify::Urgency::Normal);
    // AND THE PERSON'S OWN FILTERS, which were missing. The catalogue lists
    // them — it has to, or a record could name a filter the screen could not
    // show — and every one of them drew the plain arrival defaults instead of
    // what it actually does, on the row and in the sheet both.
    if let Some(e) = book.entry(id) {
        e.apply.over(&mut p);
    } else if let Some(r) = book.mine.iter().chain(&book.rules).find(|r| r.id == id) {
        r.apply.over(&mut p);
    }
    let adjusted = book.adjustments.get(id).is_some_and(|a| !a.says_nothing());
    if let Some(a) = book.adjustments.get(id) {
        a.over(&mut p);
    }
    (p, adjusted)
}

/// Turns a knob one step, writing the result into the verdict to save.
///
/// `step` is `+1` or `-1` — the side arrows, the same gesture the Lucide sheet
/// already teaches. Cycling and not a free field: every one of these has a
/// handful of values, and a text box would be a second way of typing.
pub fn turn(now: &Presentation, knob: Knob, step: i32, into: &mut Verdict) {
    let forward = step >= 0;
    match knob {
        Knob::Show => {
            let i = Show::ALL.iter().position(|s| *s == now.show).unwrap_or(0);
            into.show = Some(Show::ALL[wrap(i, Show::ALL.len(), forward)]);
        }
        Knob::Life => {
            let i = Life::ALL.iter().position(|l| *l == now.life).unwrap_or(0);
            into.life = Some(Life::ALL[wrap(i, Life::ALL.len(), forward)]);
        }
        Knob::Loud => into.loud = Some(!now.loud),
        Knob::Blink => into.blink = Some(!now.blink),
        Knob::Vibrate => into.vibrate = Some(!now.vibrate),
        Knob::Audio => into.audio = Some(!now.audio),
        Knob::AutoRead => into.auto_read = Some(!now.auto_read),
        Knob::Level => {
            // TEN AT A TIME. One at a time would be eleven presses to cross the
            // range, and nobody distinguishes level 63 from level 64.
            let step = if forward { 10_i32 } else { -10 };
            let v = i32::from(now.level) + step;
            into.level = Some(v.clamp(0, i32::from(TOP_LEVEL)) as u8);
        }
    }
}

const fn wrap(i: usize, len: usize, forward: bool) -> usize {
    if forward {
        (i + 1) % len
    } else {
        (i + len - 1) % len
    }
}

/// How a knob's value reads on screen.
#[must_use]
pub fn value_of(p: &Presentation, knob: Knob, texts: &ronoctua_i18n::Texts) -> String {
    let yes_no = |v: bool| {
        texts.t(if v {
            crate::key::NOTIF_YES
        } else {
            crate::key::NOTIF_NO
        })
    };
    match knob {
        Knob::Show => texts.t(match p.show {
            Show::Hidden => crate::key::NOTIF_SHOW_HIDDEN,
            Show::Brief => crate::key::NOTIF_SHOW_BRIEF,
            Show::Full => crate::key::NOTIF_SHOW_FULL,
        }),
        Knob::Life => texts.t(match p.life {
            Life::Quick => crate::key::NOTIF_LIFE_QUICK,
            Life::Normal => crate::key::NOTIF_LIFE_NORMAL,
            Life::Long => crate::key::NOTIF_LIFE_LONG,
        }),
        Knob::Loud => yes_no(p.loud),
        Knob::Blink => yes_no(p.blink),
        Knob::Vibrate => yes_no(p.vibrate),
        Knob::Audio => yes_no(p.audio),
        Knob::AutoRead => yes_no(p.auto_read),
        Knob::Level => p.level.to_string(),
    }
}

/// A piece of a row, with nothing clickable of the file manager's kind.
///
/// The manager answers pointer questions from its OWN row model — see
/// [`View::at`] — so the field's `hit` is always `Nothing`. Reusing
/// [`crate::fields::Field`] is what lets the expansion draw this without a
/// second kind of row.
fn piece(at: u16, text: String, color: tokens::Color) -> Field {
    Field {
        at,
        text,
        color,
        hit: FilesHit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

/// How many cells a row keeps at its right end for the two mouse targets.
///
/// Two glyphs and the air around them. They are the only clickable things in
/// the list, and they sit at the RIGHT for the reason the band's own control
/// does: the left margin is where the sentence starts.
pub const TARGETS: u16 = 4;

/// Which of a row's two targets is at cell `column`.
///
/// The row's own arithmetic, and the only place it is written — the drawing
/// reads the same function, so what the pointer hits is what was drawn.
#[must_use]
pub fn target_at(column: u16, at: u16, room: u16) -> Option<Hit> {
    match glyph_at(column, at, room)? {
        Glyph::Read => Some(Hit::ToggleRead),
        Glyph::Delete => Some(Hit::Delete),
    }
}

/// One of the three glyphs at a row's right end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// The envelope: read, or back to unread.
    Read,
    /// The bin: forget it.
    Delete,
}

/// Where each of a row's glyphs sits, counted from its right end: the bin at
/// the edge and the envelope before it.
///
/// ONE ANSWER for the drawing, the click and the sentence under the pointer.
const fn glyph_cell(glyph: Glyph) -> u16 {
    match glyph {
        Glyph::Delete => 1,
        Glyph::Read => 3,
    }
}

/// Which glyph is at cell `column` of a row that starts at `at`, `room` wide.
#[must_use]
pub fn glyph_at(column: u16, at: u16, room: u16) -> Option<Glyph> {
    let back = (at + room).checked_sub(column)?;
    [Glyph::Read, Glyph::Delete]
        .into_iter()
        .find(|g| glyph_cell(*g) == back)
}

/// What a click on a row of the manager does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Turn read on or off, without taking it out of the record.
    ///
    /// It is one target and not two because it is one control: the envelope on
    /// the row is open or closed, and pressing it goes the other way. A name
    /// that said `MarkRead` while sometimes unmarking would be a name that
    /// lies to the next reader.
    ToggleRead,
    /// Forget it. The history is the record of what happened, and this is the
    /// person deciding one line of it is not worth keeping.
    Delete,
}

/// The lines of a filter sheet, in the order they are drawn.
///
/// ONE ANSWER, read by the drawing and by the keys — the rule this file's
/// header states about a list's two halves, at a third level.
fn draft_lines(d: &Draft) -> Vec<Line> {
    let mut out = vec![Line::Name];
    for i in 0..d.when.len() {
        out.extend([Line::Field(i), Line::Test(i), Line::Value(i)]);
    }
    out.extend((0..Knob::ALL.len()).map(Line::Knob));
    out
}

/// A filter sheet, line by line.
fn draft_rows(d: &Draft, left: u16, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
    // WHAT IT WOULD DECIDE, resolved once: the knobs draw the verdict laid
    // over the arrival defaults, which is what the filter would actually do to
    // a notification it caught.
    let mut now = Presentation::from_urgency(ronoctua_notify::Urgency::Normal);
    d.apply.over(&mut now);
    draft_lines(d)
        .into_iter()
        .enumerate()
        .map(|(n, line)| {
            let (label, value) = match line {
                Line::Name => (
                    crate::key::NOTIF_DRAFT_NAME.to_owned(),
                    if d.name.is_empty() {
                        texts.t(crate::key::NOTIF_DRAFT_UNNAMED)
                    } else {
                        d.name.clone()
                    },
                ),
                Line::Field(i) => (
                    crate::key::NOTIF_DRAFT_FIELD.to_owned(),
                    texts.t(&format!("notif-field-{}", d.when[i].field.id())),
                ),
                Line::Test(i) => (
                    crate::key::NOTIF_DRAFT_TEST.to_owned(),
                    texts.t(&format!("notif-test-{}", d.when[i].test.id())),
                ),
                Line::Value(i) => {
                    let against = d.when[i].test.against();
                    (
                        crate::key::NOTIF_DRAFT_VALUE.to_owned(),
                        if against.trim().is_empty() {
                            texts.t(crate::key::NOTIF_DRAFT_NOTHING)
                        } else {
                            against
                        },
                    )
                }
                Line::Knob(k) => (
                    Knob::ALL[k].label().to_owned(),
                    value_of(&now, Knob::ALL[k], texts),
                ),
            };
            let focused = n == d.line;
            Row {
                // EVERY ROW OF THIS DRAFT IS TURNED BY THE ARROWS, so it is a
                // SETTING — label and value. Its label was drawn in the FACT's
                // tone beside a setting's value, which is the pair
                // contradicting itself.
                fields: crate::chrome::field_pair(
                    left,
                    room,
                    &texts.t(&label),
                    &value,
                    crate::chrome::Kind::Setting,
                    focused,
                ),
            }
        })
        .collect()
}

/// Does an open record put its facts in a column of their own?
///
/// A HIGHER BAR THAN THE LIST'S. `two_columns` asks whether two elided
/// sentences fit side by side, and eighty cells is enough for that; a fact is a
/// label AND a value on one line, and its column gets `100 - splitAtPct` of the
/// width. Below `factsFrom` they go underneath the message and its actions —
/// the same answer the list gives about itself, one level down.
#[must_use]
pub const fn facts_beside(columns: u16) -> bool {
    two_columns(columns) && columns >= tokens::space::NOTIFICATIONS_FACTSFROM
}

/// What ONE row of an open record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Told {
    /// Line `n` of what it says — the title, then the body.
    Said(usize),
    /// A blank line.
    Air,
    /// Action `n`. It is what `RET` runs when the cursor is here.
    Act(usize),
}

/// How many lines of a body an open record will wrap. A bound on WORK.
const BODY_CEILING: usize = tokens::space::NOTIFICATIONS_BODYLINES as usize;

/// How many facts an open record shows. A closed set, and the height of that
/// column — see [`facts_of`].
const FACTS: usize = 6;

/// A record's buttons, as they read: `[k] label`, the band's own shape.
///
/// The letter is inside the brackets and the letter is what presses it, here,
/// on the band and in the open record — one control, one drawing.
fn button_texts<'a>(
    r: &'a Record,
    texts: &'a ronoctua_i18n::Texts,
) -> impl Iterator<Item = (usize, String)> + 'a {
    r.notification.actions.iter().enumerate().map(|(i, a)| {
        let label = match &a.label {
            ronoctua_notify::Label::Key(k) => texts.t(k),
            // A FOREIGN APP'S, in whatever language it chose. We do not know
            // what it says, so we do not pretend to translate it.
            ronoctua_notify::Label::Text(t) => t.clone(),
        };
        (i, label)
    })
}

/// What the manager KNOWS about a record, as label and value.
///
/// Six, and the number is a constant because it is the height of this column —
/// see [`FACTS`]. They are what the row cannot say and the person cannot ask:
/// who sent it, when, how long it stayed, how it was shown, whether it has
/// been read, and WHICH FILTER DECIDED.
///
/// # The filter is here and not on the row
///
/// It used to be appended to the sentence, raw and untranslated, and the owner
/// read `bluetooth: Bluetooth Keyboard connected by small-things-are-quick`
/// off his screen as a claim about who had connected the keyboard. It is a
/// fact about the record and it belongs with the facts — named, not spelt as
/// an id, and the `ARCHITECTURE.md` promise that the manager says which rule
/// decided is kept where it can be read.
fn facts_of(
    r: &Record,
    zone: &ronoctua_files::when::Zone,
    at: u16,
    room: u16,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Vec<Field>> {
    let secs = arrived_secs(r);
    let yes_no = |v: bool| {
        texts.t(if v {
            crate::key::NOTIF_YES
        } else {
            crate::key::NOTIF_NO
        })
    };
    let filter = match &r.presentation.by {
        Some(id) => rule_name(id, texts),
        None => texts.t(crate::key::NOTIF_FACT_NO_FILTER),
    };
    [
        (crate::key::NOTIF_FACT_ORIGIN, r.notification.app.clone()),
        (
            crate::key::NOTIF_FACT_ARRIVED,
            crate::chrome::when_text(secs, zone, true, texts),
        ),
        (
            crate::key::NOTIF_FACT_LIFE,
            value_of(&r.presentation, Knob::Life, texts),
        ),
        (
            crate::key::NOTIF_FACT_SHOW,
            value_of(&r.presentation, Knob::Show, texts),
        ),
        (crate::key::NOTIF_FACT_READ, yes_no(r.read)),
        (crate::key::NOTIF_FACT_FILTER, filter),
    ]
    .into_iter()
    .map(|(label, value)| {
        // THE FILE MANAGER'S INFO PANEL'S TONES, which is the other panel of
        // facts in this system: the label quiet and the value read — the
        // owner asked for these two to change from the list's fact pair.
        let mut pair = crate::chrome::field_pair(
            at,
            room,
            &texts.t(label),
            &value,
            crate::chrome::Kind::Fact,
            false,
        );
        for (i, f) in pair.iter_mut().enumerate() {
            f.color = if i == 0 {
                tokens::color::TEXT_DIM
            } else {
                tokens::color::TEXT_PRIMARY
            };
        }
        pair
    })
    .collect()
}

/// What a catalogue row is CALLED.
///
/// The distro's rows have a text key the generator mints for every one of them.
/// A filter the PERSON wrote has none and never will: its id is text they
/// typed, which is data and not an identifier — see [`rule_name`].
fn name_of(id: &str, kind: Kind, texts: &ronoctua_i18n::Texts) -> String {
    match kind {
        // A KIND, and not one that happened, so the sentence has nothing to
        // put in its blanks. Three of the distro's summaries name the thing
        // they are about (`{ $device } connected`), and the catalogue drew the
        // blank verbatim: the screen read `{$device} connected`, which is a
        // template leaking through a list of English sentences.
        //
        // The word goes IN rather than the blank coming out, because a
        // sentence with a hole in it does not read in either language: "A
        // device connected" is what this row is about, and " connected" is not
        // a row.
        Kind::Entry => texts.t_with(
            &format!("notif-{id}-summary"),
            &[("device", &texts.t(crate::key::NOTIF_ANY_DEVICE))],
        ),
        Kind::Rule | Kind::Mine | Kind::Shared => rule_name(id, texts),
    }
}

/// A filter's name, as a person reads it.
///
/// The distro's filters have a text key the generator mints for every one of
/// them (`gen-notifications.py`). A filter the PERSON wrote has no key and
/// never will: its id is text they typed, which is data and not an identifier,
/// so it is shown as they typed it. Without this the manager drew
/// `[notif-rule-my-filter]`, the missing-key marker, at every one of their own.
fn rule_name(id: &str, texts: &ronoctua_i18n::Texts) -> String {
    let said = texts.t(&format!("notif-rule-{id}"));
    if said.starts_with('[') {
        id.to_owned()
    } else {
        said
    }
}

/// One record, as one line.
///
/// `focused` is drawn as a COLOUR and not as a box: it is the file manager's
/// own answer to the same question, and invariant 15 leaves no other.
/// When a record arrived, in seconds since the epoch.
///
/// `_secs` because the test module has an `arrived` of its own that BUILDS a
/// record at a given second, and two things called `arrived` in one file, one
/// shadowing the other, is a reader's trap for nothing.
///
/// One reading of the clock, because there are two askers now: the open
/// record's facts, and what the pointer says over a row of the list. A second
/// `duration_since` written out beside the first is a second answer waiting to
/// differ about the same notification.
fn arrived_secs(r: &Record) -> i64 {
    r.notification
        .at
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn record_line(r: &Record, at: u16, room: u16, focused: bool, readable: bool) -> Vec<Field> {
    let tone = if focused {
        tokens::color::STATE_ACCENT
    } else if matches!(r.presentation.show, Show::Hidden) {
        tokens::color::NOTIFICATION_SUPPRESSED
    } else if r.read {
        tokens::color::NOTIFICATION_READ
    } else if r.presentation.loud {
        tokens::color::NOTIFICATION_LOUD
    } else {
        tokens::color::TEXT_SECONDARY
    };
    // WHAT IT IS, in the margin — its own glyph or the generic one for its
    // severity. It used to be a read/unread circle; what says read here is the
    // TONE, and the margin is worth more as "which of these is the one I am
    // looking for".
    let mut out = vec![piece(at, crate::band::glyph_of_record(r).to_string(), tone)];
    let text_at = at + 2;
    // THE LINE IS THE NOTIFICATION, and nothing else.
    //
    // Which filter decided it used to be appended here, and the owner read it
    // off the screen as part of the sentence: `bluetooth: Bluetooth Keyboard
    // connected by small-things-are-quick` — a filter's kebab id, untranslated,
    // glued with a space onto a sentence that had already ended, saying that
    // something called `small-things-are-quick` had connected the keyboard.
    //
    // It is a FACT about the record, so it went where the facts are: the open
    // record's own column (`Level::Open`), under a label that says it is a
    // filter, with the filter's NAME rather than its id. What is true stayed
    // true — `ARCHITECTURE.md` promises the manager says which rule decided —
    // and only the place changed.
    let room_for_text = room.saturating_sub(2 + TARGETS);
    out.push(piece(
        text_at,
        layout::elide_end(&r.notification.line(), room_for_text),
        tone,
    ));

    // NO MARK SAYING THERE IS SOMETHING TO DO — the owner took the hand
    // out. What a notification can do is listed when it is opened.
    let end = at + room;

    // AND THE TWO TARGETS, at the row's right end.
    //
    // Each has a key — `Espaço` turns read on and off, `d` forgets — which is
    // the rule this footer holds the mouse to: a second way to the same door,
    // never a door of its own.
    //
    // THE ENVELOPE IS THERE EITHER WAY, and the DRAWING says which way it goes:
    // open on an unread row (press it and it is read), closed on a read one
    // (press it and it is unread again). It used to be drawn only while unread,
    // on the argument that marking read what is already read does nothing —
    // true, and it made the gesture one-way: a row read by a slip of the hand
    // stayed read for fifteen days, with no control anywhere saying otherwise.
    //
    // AND A SUPPRESSED ONE NEVER GETS IT (`readable`), although it is stored
    // unread: it never reached the band, so nothing could have read it, and
    // `Press::Space` refuses that column for exactly that reason. Drawn, the
    // envelope was a control the keyboard would not press and the mouse
    // would — and pressing it told the application the person had dismissed
    // something they were never shown.
    if readable {
        out.push(piece(
            end.saturating_sub(glyph_cell(Glyph::Read)),
            if r.read {
                tokens::icon::NOTIFICATION_MARK_UNREAD
            } else {
                tokens::icon::NOTIFICATION_MARK_READ
            }
            .to_string(),
            tone,
        ));
    }
    out.push(piece(
        end.saturating_sub(glyph_cell(Glyph::Delete)),
        tokens::icon::NOTIFICATION_DELETE.to_string(),
        tone,
    ));
    out
}

/// One catalogue row: what it is, and whether it is yours or the distro's.
fn catalogue_line(
    id: &str,
    kind: Kind,
    book: &Book,
    at: u16,
    room: u16,
    focused: bool,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Field> {
    let (p, adjusted) = in_force(book, id);
    let name = name_of(id, kind, texts);
    // WHOSE IT IS, and it is three answers rather than two. "Your adjustment"
    // used to be the only thing that could be said, so a filter the person had
    // WRITTEN read as "the distro's default" — the screen calling their own
    // work somebody else's.
    let source = texts.t(match (kind, adjusted) {
        (Kind::Shared, _) => crate::key::NOTIF_SHARED,
        (Kind::Mine, _) => crate::key::NOTIF_MINE,
        (_, true) => crate::key::NOTIF_YOURS,
        (_, false) => crate::key::NOTIF_DEFAULT,
    });
    let summary = format!(
        "{} · {} · {}",
        value_of(&p, Knob::Show, texts),
        value_of(&p, Knob::Life, texts),
        source
    );
    let half = room / 2;
    vec![
        piece(
            at,
            layout::elide_end(&name, half.saturating_sub(1)),
            if focused {
                tokens::color::STATE_ACCENT
            } else if kind == Kind::Shared {
                tokens::color::TEXT_SHARED
            } else if kind.is_mine() {
                // A TONE OF ITS OWN for what the person wrote, which is what
                // the owner asked for: in a list of forty rows, three of them
                // are theirs and the rest are the distro's.
                tokens::color::TEXT_MINE
            } else if adjusted {
                tokens::color::TEXT_PRIMARY
            } else {
                tokens::color::TEXT_SECONDARY
            },
        ),
        piece(
            at + half,
            layout::elide_end(&summary, room.saturating_sub(half)),
            tokens::color::TEXT_DIM,
        ),
    ]
}

/// A key, already reduced to what the manager cares about.
///
/// # Why not the keysym
///
/// So the whole manager can be driven by a test with no compositor. Mapping
/// a key to this is a few lines in each door (`notifications_region`,
/// `notifications_window`) and it is the only part that needs a seat;
/// everything below is arithmetic over a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// Left — inside the sheet it turns the knob back.
    Left,
    /// Right — inside the sheet it turns the knob forward.
    Right,
    /// Next pane.
    Tab,
    /// Previous pane — `S-TAB`, which xkb delivers as its own keysym.
    BackTab,
    /// Act on the focused row.
    Enter,
    /// Mark the focused notification read.
    Space,
    /// One level back: the sheet closes, then the manager.
    Escape,
    /// A letter.
    Letter(char),
}

impl View {
    /// The rows to draw, for the width given.
    ///
    /// # Why the headings are rows
    ///
    /// Because the region draws a list, and a heading that lived outside it
    /// would need its own arithmetic for where the list starts — which is the
    /// divergence this file's header warns about, one level down.
    #[must_use]
    pub fn rows(
        &self,
        lists: &Lists<'_>,
        book: &Book,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        // ONE ANSWER TO "WHERE DOES A ROW BEGIN": the full-width extent, which
        // is what the catalogue and the sheet and the stacked panes all take.
        // It used to be worked out again here as the whole inset, and the two
        // doors added their lead on top — see `extent_of`.
        let (left, room) = extent_of(Pane::Catalogue, columns);

        // THE SHEET takes the whole region: it is one thing being adjusted, and
        // a list beside it would be a second place for the eye to be.
        if let Level::Sheet(knob) = self.level {
            return self.sheet_rows(lists, book, left, room, knob, texts);
        }
        if let Level::Draft(d) = &self.level {
            return draft_rows(d, left, room, texts);
        }
        if let Level::Publish { id, yes } = &self.level {
            return self.publish_rows(id, *yes, book, left, room, texts);
        }
        if let Level::Open { at, .. } = self.level {
            // A RECORD THE LIST NO LONGER HAS is a level with nothing in it —
            // forgotten through the other door, or aged out while it was open.
            // The list is what there still is, so it decides.
            if let Some(r) = lists.record(at) {
                return self.open_rows(r, columns, texts);
            }
        }

        // THE CATALOGUE takes the whole width too — it is not something that
        // happened, it is something that is configured, and it has two columns
        // of its own.
        if self.pane == Pane::Catalogue || !two_columns(columns) {
            return self.one_column(lists, book, left, room, texts);
        }

        let split = split_at(columns);
        // THE HEADING OF THE COLUMN THAT HAS THE KEYBOARD IS LIT.
        //
        // Both used to be `TEXT_HEADING` always, and the only sign of which
        // column the keyboard was on was the OTHER column's row losing its
        // accent. On an empty column there is no row to lose it: `TAB` onto
        // "Filtered out" with nothing filtered left the screen saying
        // "nothing here" in the dim tone, no cursor anywhere, and — before
        // the hint line learned its column — four keys taught that did
        // nothing. Three ways of saying nothing, and no way of saying where.
        //
        // The file manager solved the same thing with `chrome::divider_lit`;
        // there is no rule here, so what carries it is the heading, which is
        // the one thing a column always has.
        let heading = |pane: Pane| {
            if self.pane == pane {
                tokens::color::STATE_ACCENT
            } else {
                tokens::color::TEXT_HEADING
            }
        };
        let mut out = vec![Row {
            fields: vec![
                piece(
                    left,
                    texts.t(Pane::Happened.heading()),
                    heading(Pane::Happened),
                ),
                piece(
                    split,
                    texts.t(Pane::Suppressed.heading()),
                    heading(Pane::Suppressed),
                ),
            ],
        }];
        // AT LEAST ONE ROW UNDER THE HEADINGS, so that an EMPTY column can say
        // so — in ITS column. It used to say it only when both were empty, and
        // only under the left heading: with three notifications received and
        // none filtered, the right-hand column was blank, which the owner
        // reported as a column that had lost its "nothing here". An empty
        // region is indistinguishable from a key that did not work.
        // EACH COLUMN WINDOWED AT ITS OWN OFFSET, the room shared: the two
        // are two lists, and one scrolling does not move the other.
        let [first, _] = self.list_places(lists, columns);
        let from = self.list_offsets_now(lists, columns);
        for row in 0..first.room {
            let mut fields = Vec::new();
            // THE CURSOR IS IN ONE PANE, and only that pane's half lights up.
            // Lighting the whole row would say the two items were one thing —
            // and they are two different notifications, in two lists.
            //
            // And an empty pane has no row to light: the cursor sits at zero
            // there with nothing under it, and lighting the "nothing here" as
            // if it were an item would promise a key that does nothing.
            for (k, pane) in [Pane::Happened, Pane::Suppressed].into_iter().enumerate() {
                let i = from[k] + row;
                let (at, room) = list_extent(pane, columns);
                let record = match pane {
                    Pane::Happened => lists.happened.get(i),
                    _ => lists.suppressed.get(i),
                };
                match record {
                    Some(r) => {
                        let focused = self.pane == pane && self.cursor() == i;
                        fields.extend(record_line(r, at, room, focused, View::can_be_read(pane)));
                    }
                    None if i == 0 => fields.push(piece(
                        at,
                        texts.t(crate::key::NOTIF_NOTHING_HERE),
                        tokens::color::TEXT_DIM,
                    )),
                    None => {}
                }
            }
            out.push(Row { fields });
        }
        out
    }

    /// The panes stacked, one under the other. The narrow case, and the
    /// catalogue's.
    fn one_column(
        &self,
        lists: &Lists<'_>,
        book: &Book,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let plan = self.plan(lists);
        let several = plan
            .iter()
            .filter(|p| matches!(p, Place::Heading(_)))
            .count()
            > 1;
        plan.iter()
            .map(|place| Row {
                fields: match *place {
                    Place::Air => Vec::new(),
                    Place::Heading(pane) => vec![piece(
                        left,
                        texts.t(pane.heading()),
                        // LIT WHEN THERE IS SOMETHING TO TELL IT FROM. Stacked,
                        // the two titles are the only thing saying which list
                        // `TAB` left the keyboard in — an empty column has no
                        // row to light. Alone on the screen, a lit title would
                        // be an accent answering a question nobody asked.
                        if several && self.pane == pane {
                            tokens::color::STATE_ACCENT
                        } else {
                            tokens::color::TEXT_HEADING
                        },
                    )],
                    Place::Empty(_) => vec![piece(
                        left,
                        texts.t(crate::key::NOTIF_NOTHING_HERE),
                        tokens::color::TEXT_DIM,
                    )],
                    Place::Item(pane, i) => {
                        let focused = self.pane == pane && self.cursor() == i;
                        match pane {
                            Pane::Happened => record_line(
                                lists.happened[i],
                                left,
                                room,
                                focused,
                                Self::can_be_read(Pane::Happened),
                            ),
                            Pane::Suppressed => record_line(
                                lists.suppressed[i],
                                left,
                                room,
                                focused,
                                Self::can_be_read(Pane::Suppressed),
                            ),
                            Pane::Catalogue => {
                                let (id, kind) = &lists.catalogue[i];
                                catalogue_line(id, *kind, book, left, room, focused, texts)
                            }
                        }
                    }
                },
            })
            .collect()
    }

    /// One catalogue entry, field by field, with the distro's answer beside it.
    fn sheet_rows(
        &self,
        lists: &Lists<'_>,
        book: &Book,
        left: u16,
        room: u16,
        knob: usize,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let Some((id, kind)) = lists.catalogue.get(self.catalogue) else {
            return Vec::new();
        };
        let (p, adjusted) = in_force(book, id);
        let name = name_of(id, *kind, texts);
        let mut out = vec![Row {
            fields: vec![
                piece(left, name, tokens::color::TEXT_PRIMARY),
                piece(
                    left + room / 2,
                    texts.t(match (kind, adjusted) {
                        (Kind::Mine, _) => crate::key::NOTIF_MINE,
                        (_, true) => crate::key::NOTIF_YOURS,
                        (_, false) => crate::key::NOTIF_DEFAULT,
                    }),
                    tokens::color::TEXT_DIM,
                ),
            ],
        }];
        for (i, k) in Knob::ALL.iter().enumerate() {
            // THE FOCUSED KNOB IS THE ONE THE ARROWS TURN, and it says so in
            // colour — the same mark the lists use, and the only one invariant
            // 15 leaves. Without it the sheet is seven rows and no cursor, and
            // pressing an arrow changes a line nobody was looking at.
            let focused = i == knob;
            // A KNOB IS A SETTING, label and value — its label was drawn in the
            // content's grey, the one row of the sheet that said nothing about
            // being changeable. `chrome::Kind` is the one place that answers.
            let (label, value) = crate::chrome::Kind::Setting.tones(focused);
            // A SWITCH THAT SWITCHES NOTHING OFF STILL SAYS SO — but on the
            // MESSAGE LINE, when the row is focused, and not as a third column
            // beside it. See [`View::hint`].
            //
            // It was drawn inline and the owner asked for it gone: two rows out
            // of seven carrying a sentence made the sheet read as a list of
            // problems. The honesty is not negotiable — the trap has a name in
            // `AGENTS.md` and this project has paid for it twice — so it moved
            // to where this footer already puts explanations, which is the same
            // line the file manager's fields explain themselves on.
            //
            // What stays on the row is the value being `no`, which is true.
            let fields = vec![
                piece(left, texts.t(k.label()), label),
                piece(left + room / 3, value_of(&p, *k, texts), value),
            ];
            out.push(Row { fields });
        }
        out
    }

    /// Acts on a key.
    #[must_use]
    /// `columns` is the width the rows were drawn at: an open record's height
    /// depends on it, and the arrows stop at the last line of it.
    pub fn key(&mut self, press: Press, lists: &Lists<'_>, book: &Book, columns: u16) -> Errand {
        self.asking = None;
        let errand = self.key_here(press, lists, book, columns);
        // THE COLUMN FOLLOWS THE CURSOR, side by side: the region does not
        // scroll, so what keeps the cursor on screen is its own column.
        self.follow_list(lists, columns);
        errand
    }

    fn key_here(&mut self, press: Press, lists: &Lists<'_>, book: &Book, columns: u16) -> Errand {
        if let Level::Sheet(knob) = self.level {
            return self.sheet_key(press, lists, book, knob);
        }
        if let Level::Open { at, line } = self.level {
            return self.open_key(press, lists, at, line, columns);
        }
        if matches!(self.level, Level::Draft(_)) {
            return self.draft_key(press);
        }
        if let Level::Publish { id, yes } = &self.level {
            let (id, yes) = (id.clone(), *yes);
            return self.publish_key(press, &id, yes);
        }
        let n = lists.len(self.pane);
        match press {
            Press::Up => {
                self.set_cursor(self.cursor().saturating_sub(1));
                Errand::Nothing
            }
            Press::Down => {
                self.set_cursor((self.cursor() + 1).min(n.saturating_sub(1)));
                Errand::Nothing
            }
            Press::Tab | Press::BackTab => {
                self.pane = if press == Press::Tab {
                    self.pane.next()
                } else {
                    self.pane.previous()
                };
                // THE NEW PANE MAY BE SHORTER. Each pane keeps its own cursor —
                // coming back to a list should come back to where you were —
                // but a cursor past the end would light a row that is not there
                // and act on nothing.
                let there = lists.len(self.pane);
                self.set_cursor(self.cursor().min(there.saturating_sub(1)));
                Errand::Nothing
            }
            Press::Escape => Errand::Close,
            // `SPC` TURNS IT ROUND, whichever way it is: read becomes unread
            // and unread becomes read. One key for one control, and the line
            // of hints says which of the two this row would do — the phrase
            // moves under the reader, so the key is drawn as one that does.
            Press::Space => match self.pane {
                Pane::Happened => lists
                    .happened
                    .get(self.cursor())
                    .map_or(Errand::Nothing, |r| {
                        if r.read {
                            Errand::Unread(r.key())
                        } else {
                            Errand::Read(r.key())
                        }
                    }),
                // A SUPPRESSED ONE IS ALREADY OFF THE COUNT, so marking it read
                // would be a gesture with no effect — and a key that does
                // nothing is worse than a key that is not bound.
                _ => Errand::Nothing,
            },
            Press::Enter => self.open_here(lists, book),
            // AND THE ONE THAT OFFERS IT TO EVERYBODY. It opens a question
            // and nothing else: what is published is published.
            Press::Letter('p') if self.pane == Pane::Catalogue => {
                self.mine_here(lists).map_or(Errand::Nothing, |id| {
                    self.level = Level::Publish {
                        id: id.to_owned(),
                        yes: false,
                    };
                    Errand::Nothing
                })
            }
            // A NEW FILTER, from the column that holds them.
            Press::Letter('n') if self.pane == Pane::Catalogue => {
                self.level = Level::Draft(Draft::fresh());
                Errand::Nothing
            }
            Press::Letter('r') => Errand::ReadAll,
            // STRAIGHT TO THE FILTERS, wherever the cursor is. The letter is
            // reserved against a notification's own actions, like `r` and `d`.
            Press::Letter('f') => {
                self.pane = Pane::Catalogue;
                let there = lists.len(Pane::Catalogue);
                self.set_cursor(self.cursor().min(there.saturating_sub(1)));
                Errand::Nothing
            }
            // `d` FORGETS the focused one. Reserved, and the generator refuses
            // an action that asks for the same letter — an action shadowed by a
            // view key is an action nobody can reach, and the way to find out
            // would be pressing it and watching something else happen.
            // `d` ON THE CATALOGUE DROPS A FILTER OF THEIRS, and only ever
            // one of theirs: the distro's are declared in the flake, and
            // "deleting" one would be a lie the next rebuild tells.
            Press::Letter('d') if self.pane == Pane::Catalogue => self
                .mine_here(lists)
                .map_or(Errand::Nothing, |id| Errand::Unfilter(id.to_owned())),
            Press::Letter('d') => match self.pane {
                Pane::Happened => lists
                    .happened
                    .get(self.cursor())
                    .map_or(Errand::Nothing, |r| Errand::Forget(r.key())),
                Pane::Suppressed => lists
                    .suppressed
                    .get(self.cursor())
                    .map_or(Errand::Nothing, |r| Errand::Forget(r.key())),
                Pane::Catalogue => Errand::Nothing,
            },
            Press::Letter(_) | Press::Left | Press::Right => Errand::Nothing,
        }
    }

    /// What each row of an OPEN record is.
    ///
    /// One plan, three readers — the drawing, the keys and the region's scroll
    /// — for the reason the list's own [`Place`] gives: two answers to "what
    /// is on row n" is how a cursor comes to act on something else.
    fn open_plan(&self, r: &Record, columns: u16) -> Vec<Told> {
        let (_, room) = self.text_extent(columns, false);
        let mut out: Vec<Told> = Vec::new();
        let said = layout::wrap(&r.notification.line(), room, BODY_CEILING).len()
            + layout::wrap(&r.notification.body, room, BODY_CEILING).len();
        out.extend((0..said).map(Told::Said));
        if !r.notification.actions.is_empty() {
            // AIR BEFORE THEM. They are a different kind of thing from the
            // sentence above, and this system says that with space.
            out.push(Told::Air);
            out.extend((0..r.notification.actions.len()).map(Told::Act));
        }
        if out.is_empty() {
            out.push(Told::Air);
        }
        // THE FACTS ARE A HALF OF THEIR OWN, beside or under — with their own
        // offset and bar (`places`). They rode on these rows once, and the two
        // scrolled as one: a long message took its facts off the screen.
        out
    }

    /// Where one half of an open record begins and how wide it is.
    ///
    /// The SAME split the two columns of the list use, so the eye finds the
    /// facts where it found the second column — and narrow, both halves take
    /// the whole width, one under the other. One token decides both.
    fn open_extent(&self, columns: u16, facts: bool) -> (u16, u16) {
        if !facts_beside(columns) {
            return extent_of(Pane::Catalogue, columns);
        }
        extent_of(
            if facts {
                Pane::Suppressed
            } else {
                Pane::Happened
            },
            columns,
        )
    }

    /// Where a half's TEXT goes: its extent less the bar's cells at the
    /// right end, so no bar is ever drawn over a word.
    fn text_extent(&self, columns: u16, facts: bool) -> (u16, u16) {
        let (at, room) = self.open_extent(columns, facts);
        (at, room.saturating_sub(HALF_BAR))
    }

    /// The facts of an open record, one row each.
    fn facts_rows(
        &self,
        r: &Record,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Vec<Field>> {
        let (at, room) = self.text_extent(columns, true);
        facts_of(r, &self.zone, at, room, texts)
    }

    /// ONE record, whole.
    ///
    /// # Why this level exists
    ///
    /// Because a row is one line, and a line that does not fit was the end of
    /// the road: the owner had notifications he could not read and nothing to
    /// press. Here the body is WRAPPED rather than cut, the actions are a LIST
    /// — one under the other, with room for their words — and what the manager
    /// knows about the record is a column of facts beside it.
    fn open_rows(&self, r: &Record, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let (left, room) = self.text_extent(columns, false);
        let facts = self.facts_rows(r, columns, texts);

        // WHAT IT SAYS. The title carries the tone the row carried, so a loud
        // one is still loud with its body open.
        let tone = if r.presentation.loud {
            tokens::color::NOTIFICATION_LOUD
        } else {
            tokens::color::TEXT_PRIMARY
        };
        let mut said: Vec<Field> = layout::wrap(&r.notification.line(), room, BODY_CEILING)
            .into_iter()
            .map(|line| piece(left, line, tone))
            .collect();
        said.extend(
            layout::wrap(&r.notification.body, room, BODY_CEILING)
                .into_iter()
                .map(|line| piece(left, line, tokens::color::TEXT_SECONDARY)),
        );

        let lines: Vec<Vec<Field>> = self
            .open_plan(r, columns)
            .into_iter()
            .enumerate()
            .map(|(n, told)| match told {
                Told::Air => Vec::new(),
                Told::Said(i) => said.get(i).cloned().into_iter().collect(),
                // AN ACTION IS A ROW, and the cursor lights the one `RET`
                // would run. It is the file manager's own answer to the
                // same question, and invariant 15 leaves no other.
                Told::Act(i) => button_texts(r, texts)
                    .nth(i)
                    .map(|(_, text)| {
                        vec![piece(
                            left,
                            text,
                            // AN ACTION IS A SETTING'S LABEL: a thing a
                            // click does.
                            crate::chrome::Kind::Setting
                                .tones(self.open_line() == n && !self.open_facts)
                                .0,
                        )]
                    })
                    .unwrap_or_default(),
            })
            .collect();
        let [said_place, facts_place] = self.places_of(r, columns);

        // TWO HALVES, each a list of its own windowed at its own offset, and
        // NO TITLES — the owner took them out: a message and its facts are
        // told apart by where they are and how they are drawn.
        let from = [
            self.open_offsets[0].min(said_place.total.saturating_sub(said_place.room)),
            self.open_offsets[1].min(facts_place.total.saturating_sub(facts_place.room)),
        ];
        let window = |all: &[Vec<Field>], place: HalfPlace, at: usize| -> Vec<Vec<Field>> {
            (0..place.room)
                .map(|i| all.get(at + i).cloned().unwrap_or_default())
                .collect()
        };
        let said_rows = window(&lines, said_place, from[0]);
        let facts_rows = window(&facts, facts_place, from[1]);
        if facts_beside(columns) {
            let mut out = Vec::new();
            let tall = said_place.room.max(facts_place.room);
            for i in 0..tall {
                let mut fields = said_rows.get(i).cloned().unwrap_or_default();
                fields.extend(facts_rows.get(i).cloned().unwrap_or_default());
                out.push(Row { fields });
            }
            return out;
        }
        // STACKED: the message, a line of air, then the facts.
        let mut out: Vec<Row> = said_rows.into_iter().map(|fields| Row { fields }).collect();
        out.push(Row { fields: Vec::new() });
        out.extend(facts_rows.into_iter().map(|fields| Row { fields }));
        out
    }

    /// Which row of an open record the cursor is on.
    const fn open_line(&self) -> usize {
        match self.level {
            Level::Open { line, .. } => line,
            _ => usize::MAX,
        }
    }

    /// The open record's keys: the arrows scroll it, `d` forgets it, `Esc`
    /// goes back to the list.
    fn open_key(
        &mut self,
        press: Press,
        lists: &Lists<'_>,
        at: RecordKey,
        line: usize,
        columns: u16,
    ) -> Errand {
        let Some(r) = lists.record(at) else {
            // It was forgotten through the other door while it was open.
            self.level = Level::List;
            return Errand::Nothing;
        };
        let plan = self.open_plan(r, columns);
        let step = |to: usize| Level::Open {
            at,
            line: to.min(plan.len().saturating_sub(1)),
        };
        let split = facts_beside(columns);
        match press {
            Press::Escape => {
                self.level = Level::List;
                Errand::Nothing
            }
            // THE KEYBOARD GOES FROM HALF TO HALF: `TAB` round them as it
            // goes round the list's panes, and — side by side — `←`/`→` to
            // the one on that side, as the Bluetooth sheet's two halves do.
            Press::Tab | Press::BackTab => {
                self.open_facts = !self.open_facts;
                Errand::Nothing
            }
            Press::Left | Press::Right if split => {
                self.open_facts = press == Press::Right;
                Errand::Nothing
            }
            // THE FACTS HAVE NOTHING TO DO, so the arrows scroll them.
            Press::Up | Press::Down if self.open_facts => {
                let now = self.half_offset(Half::Second, lists, columns);
                let to = if press == Press::Down {
                    now + 1
                } else {
                    now.saturating_sub(1)
                };
                self.open_scroll_to(Half::Second, to, lists, columns);
                Errand::Nothing
            }
            // THE CURSOR STOPS ONLY WHERE THERE IS SOMETHING TO DO.
            //
            // It used to walk every row, and the owner read what that is like:
            // several presses to cross the message, one line at a time, with
            // nothing lighting up on the way — because a line of the body is
            // not a thing one can act on and had nothing to show for being
            // under the cursor. So the arrows step from ACTION to ACTION.
            //
            // With no action there is nothing to step to, and then they scroll
            // the record line by line: a body that does not fit is the reason
            // this level exists, and the wheel must not be the only way down
            // it.
            Press::Up | Press::Down => {
                let forward = press == Press::Down;
                let acts: Vec<usize> = plan
                    .iter()
                    .enumerate()
                    .filter_map(|(n, told)| matches!(told, Told::Act(_)).then_some(n))
                    .collect();
                self.level = if acts.is_empty() {
                    step(if forward {
                        line + 1
                    } else {
                        line.saturating_sub(1)
                    })
                } else if forward {
                    step(acts.iter().copied().find(|n| *n > line).unwrap_or(acts[0]))
                } else {
                    step(
                        acts.iter()
                            .copied()
                            .rev()
                            .find(|n| *n < line)
                            .unwrap_or_else(|| acts[acts.len() - 1]),
                    )
                };
                // THE HALF FOLLOWS THE LINE, since the region
                // does not scroll: with actions, the one lit is kept on
                // screen; without, the line IS the half's first line.
                if let Level::Open { line, .. } = self.level {
                    let room = self.places_of(r, columns)[0].room.max(1);
                    let now = self.half_offset(Half::First, lists, columns);
                    let to = if acts.is_empty() || line < now {
                        line
                    } else if line >= now.saturating_add(room) {
                        line + 1 - room
                    } else {
                        now
                    };
                    self.open_scroll_to(Half::First, to, lists, columns);
                    // AND A LINE PAST THE LAST PAGE COMES BACK TO IT, or
                    // the way up would take as many presses as went down
                    // past the end.
                    if acts.is_empty() {
                        let top = self.open_offsets[Half::First.index()];
                        self.level = Level::Open { at, line: top };
                    }
                }
                Errand::Nothing
            }
            // AND `RET` RUNS THE ONE UNDER IT. On any other row there is
            // nothing to run, and the hint line says which of the two this row
            // is — the same moving key the list draws.
            Press::Enter => match plan.get(line) {
                Some(Told::Act(i)) => Errand::Act(at, *i),
                _ => Errand::Nothing,
            },
            // FORGETTING IT CLOSES IT: the level is about a record, and there
            // would be no record left to be at.
            Press::Letter('d') => {
                self.level = Level::List;
                Errand::Forget(at)
            }
            Press::Space => {
                if r.read {
                    Errand::Unread(at)
                } else {
                    Errand::Read(at)
                }
            }
            _ => Errand::Nothing,
        }
    }

    /// The question before a filter goes to everybody.
    ///
    /// It draws the filter and, in one line, what it does — not the eight
    /// knobs the sheet draws. The question here is "shall this go", and the
    /// answers have to be ON SCREEN with the sentence that explains them:
    /// drawn whole, the two fell below the footer's ceiling, which is a
    /// question one has to scroll to answer.
    fn publish_rows(
        &self,
        id: &str,
        yes: bool,
        book: &Book,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let Some(rule) = book.mine.iter().find(|r| r.id == id) else {
            return Vec::new();
        };
        let draft = Draft::of(rule);
        let mut out: Vec<Row> = draft_rows(&draft, left, room, texts)
            .into_iter()
            .take(1 + draft.when.len() * 3)
            .collect();
        let mut now = Presentation::from_urgency(ronoctua_notify::Urgency::Normal);
        rule.apply.over(&mut now);
        out.push(Row {
            fields: crate::chrome::field_pair(
                left,
                room,
                &texts.t(crate::key::NOTIF_SHARE_DOES),
                &format!(
                    "{} {} {}",
                    value_of(&now, Knob::Show, texts),
                    tokens::mark::SEPARATOR,
                    value_of(&now, Knob::Life, texts),
                ),
                crate::chrome::Kind::Fact,
                false,
            ),
        });
        out.push(Row { fields: Vec::new() });
        // THE SENTENCE, and it is the whole reason this screen exists. The
        // repository is public (invariant 2) and no personal datum belongs in
        // it (invariant 13); a filter can carry a person's name, an internal
        // application, a project.
        for line in layout::wrap(&texts.t(crate::key::NOTIF_SHARE_PUBLIC), room, BODY_CEILING) {
            out.push(Row {
                fields: vec![piece(left, line, tokens::color::TEXT_SECONDARY)],
            });
        }
        out.push(Row { fields: Vec::new() });
        // A NAME IN THE REPOSITORY IS ENGLISH KEBAB (invariant 18), and it is
        // the id an adjustment is keyed by. Mangling `silenciar cópias` behind
        // somebody's back would be worse than saying so.
        if !ronoctua_notify::is_shareable_name(id) {
            for line in layout::wrap(
                &texts.t(crate::key::NOTIF_SHARE_NOT_A_NAME),
                room,
                BODY_CEILING,
            ) {
                out.push(Row {
                    fields: vec![piece(left, line, tokens::color::STATE_ERROR)],
                });
            }
            return out;
        }
        // NO FIRST, and the cursor starts on it.
        for (says, here) in [
            (crate::key::NOTIF_SHARE_NO, !yes),
            (crate::key::NOTIF_SHARE_YES, yes),
        ] {
            out.push(Row {
                fields: vec![piece(
                    left,
                    texts.t(says),
                    if here {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_SECONDARY
                    },
                )],
            });
        }
        out
    }

    /// The question's keys: the arrows choose, `RET` answers, `Esc` is no.
    fn publish_key(&mut self, press: Press, id: &str, yes: bool) -> Errand {
        match press {
            Press::Escape => {
                self.level = Level::List;
                Errand::Nothing
            }
            // A RING OF TWO: either arrow goes to the other answer. There are
            // exactly two, so an arrow that stops dead at one of them is an
            // arrow somebody presses twice wondering whether it arrived.
            Press::Up | Press::Down => {
                self.level = Level::Publish {
                    id: id.to_owned(),
                    yes: !yes,
                };
                Errand::Nothing
            }
            Press::Enter => {
                let go = yes && ronoctua_notify::is_shareable_name(id);
                self.level = Level::List;
                if go {
                    Errand::Share(id.to_owned())
                } else {
                    Errand::Nothing
                }
            }
            _ => Errand::Nothing,
        }
    }

    /// The filter sheet's keys.
    ///
    /// The arrows turn what is on the focused line, `RET` asks for the text
    /// that cannot be turned, `a` adds a condition, `x` takes one away, `s`
    /// saves, and `Esc` leaves without saving. Every one of them is on the
    /// hint line, which is generated from [`OF_THE_DRAFT`].
    fn draft_key(&mut self, press: Press) -> Errand {
        let Level::Draft(d) = &mut self.level else {
            return Errand::Nothing;
        };
        let lines = draft_lines(d);
        let here = lines.get(d.line).copied();
        match press {
            Press::Escape => {
                // NOTHING IS SAVED HERE. The file is written by the errand,
                // through the door, exactly as an adjustment is — so leaving
                // is leaving, and a half-written filter never reaches disk.
                self.level = Level::List;
                Errand::Nothing
            }
            Press::Up => {
                d.line = d.line.saturating_sub(1);
                Errand::Nothing
            }
            Press::Down => {
                d.line = (d.line + 1).min(lines.len().saturating_sub(1));
                Errand::Nothing
            }
            Press::Left | Press::Right => {
                let forward = press == Press::Right;
                match here {
                    Some(Line::Field(i)) => {
                        if let Some(t) = d.when.get_mut(i) {
                            let now = ronoctua_notify::Field::ALL
                                .iter()
                                .position(|f| *f == t.field)
                                .unwrap_or(0);
                            t.field = ronoctua_notify::Field::ALL
                                [wrap(now, ronoctua_notify::Field::ALL.len(), forward)];
                        }
                    }
                    Some(Line::Test(i)) => {
                        if let Some(t) = d.when.get_mut(i) {
                            let kinds = ronoctua_notify::Test::KINDS;
                            let now = kinds.iter().position(|k| *k == t.test.id()).unwrap_or(0);
                            t.test = t.test.of_kind(kinds[wrap(now, kinds.len(), forward)]);
                        }
                    }
                    // A NUMBER IS TURNED, not typed: `at-least` takes `0..=100`
                    // and the level knob already moves in tens for the reason
                    // nobody tells 63 from 64.
                    Some(Line::Value(i)) => {
                        if let Some(t) = d.when.get_mut(i)
                            && t.test.is_numeric()
                        {
                            let now = t.test.against().parse::<i32>().unwrap_or(0);
                            let next = (now + if forward { 10 } else { -10 })
                                .clamp(0, i32::from(TOP_LEVEL))
                                .to_string();
                            t.test = ronoctua_notify::Test::of_kind(
                                &ronoctua_notify::Test::Is(next),
                                t.test.id(),
                            );
                        }
                    }
                    Some(Line::Knob(k)) => {
                        let mut now = Presentation::from_urgency(ronoctua_notify::Urgency::Normal);
                        d.apply.over(&mut now);
                        let mut into = d.apply.clone();
                        turn(&now, Knob::ALL[k], if forward { 1 } else { -1 }, &mut into);
                        d.apply = into;
                    }
                    Some(Line::Name) | None => {}
                }
                Errand::Nothing
            }
            // WHAT CANNOT BE TURNED IS TYPED — on the line of whichever door
            // this is: the footer's minibuffer, or the window's header. The
            // view remembers what it asked, and the answer comes back to
            // `typed`.
            Press::Enter => {
                let asking = match here {
                    Some(Line::Name) => Asking::Name,
                    Some(Line::Value(i)) if !d.when[i].test.is_numeric() => Asking::Value(i),
                    _ => return Errand::Nothing,
                };
                self.asking = Some(asking);
                Errand::Ask(asking)
            }
            Press::Letter('a') => {
                d.when.push(ronoctua_notify::Term {
                    field: ronoctua_notify::Field::Summary,
                    test: ronoctua_notify::Test::Contains(String::new()),
                });
                d.line = draft_lines(d).len().saturating_sub(1) - Knob::ALL.len();
                Errand::Nothing
            }
            // TAKING ONE AWAY, and never the last one: a filter with no
            // condition matches EVERYTHING, and nobody means to write that by
            // pressing a key twice.
            Press::Letter('x') => {
                if let Some(Line::Field(i) | Line::Test(i) | Line::Value(i)) = here
                    && d.when.len() > 1
                {
                    d.when.remove(i);
                    d.line = d.line.min(draft_lines(d).len().saturating_sub(1));
                }
                Errand::Nothing
            }
            Press::Letter('s') => match d.ready() {
                Some(rule) => {
                    let was = d.editing.clone();
                    self.level = Level::List;
                    Errand::Filter { was, rule }
                }
                // IT SAYS WHY, on the line the message wins. A key that
                // silently does nothing is the defect this repository has a
                // check about.
                None => Errand::Nothing,
            },
            Press::Letter(_) | Press::Tab | Press::BackTab | Press::Space => Errand::Nothing,
        }
    }

    /// The line a door opens for what the draft asked: its id, and what the
    /// line starts from — the name or the text the draft holds now, so that
    /// fixing one letter is one `DEL` and not the whole of it typed again.
    #[must_use]
    pub fn typing(&self) -> Option<(&'static str, String)> {
        let what = self.asking?;
        let Level::Draft(d) = &self.level else {
            return None;
        };
        let now = match what {
            Asking::Name => d.name.clone(),
            Asking::Value(i) => d.when.get(i)?.test.against(),
        };
        Some((what.id(), now))
    }

    /// What the person typed on a door's line, back into the draft — the
    /// one call both doors make with the answer.
    ///
    /// AN EMPTY ANSWER IS A NO: the line was left with nothing on it, and
    /// the draft keeps what it had. An answer nobody asked for (no question
    /// under way) is nothing too.
    pub fn typed(&mut self, text: &str) {
        let Some(what) = self.asking.take() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        let Level::Draft(d) = &mut self.level else {
            return;
        };
        match what {
            Asking::Name => d.name = text.trim().to_owned(),
            Asking::Value(i) => {
                if let Some(t) = d.when.get_mut(i) {
                    t.test = ronoctua_notify::Test::of_kind(
                        &ronoctua_notify::Test::Is(text.to_owned()),
                        t.test.id(),
                    );
                }
            }
        }
    }

    /// Why `s` did nothing, when it did nothing.
    ///
    /// The message line, not the hint line — the same place the sheet says why
    /// a switch is inert.
    fn draft_note(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let Level::Draft(d) = &self.level else {
            return None;
        };
        d.ready()
            .is_none()
            .then(|| texts.t(crate::key::NOTIF_DRAFT_UNREADY))
    }

    /// The sheet's keys: the arrows turn, `r` restores, `Esc` closes it.
    fn sheet_key(&mut self, press: Press, lists: &Lists<'_>, book: &Book, knob: usize) -> Errand {
        let Some((id, _)) = lists.catalogue.get(self.catalogue) else {
            self.level = Level::List;
            return Errand::Nothing;
        };
        match press {
            Press::Escape => {
                self.level = Level::List;
                Errand::Nothing
            }
            Press::Up => {
                self.level = Level::Sheet(knob.saturating_sub(1));
                Errand::Nothing
            }
            Press::Down => {
                self.level = Level::Sheet((knob + 1).min(Knob::ALL.len() - 1));
                Errand::Nothing
            }
            Press::Left | Press::Right => {
                let (p, _) = in_force(book, id);
                let mut v = book.adjustments.get(id).cloned().unwrap_or_default();
                turn(
                    &p,
                    Knob::ALL[knob],
                    if press == Press::Right { 1 } else { -1 },
                    &mut v,
                );
                Errand::Adjusted(id.clone(), Some(v))
            }
            // BACK TO THE DISTRO'S. The adjustment is REMOVED, not set to the
            // default value: "yours" and "the distro's, which happens to be the
            // same" are different answers to "who decided this?", and the
            // manager draws the difference.
            Press::Letter('r') => Errand::Adjusted(id.clone(), None),
            Press::Tab | Press::BackTab | Press::Enter | Press::Space | Press::Letter(_) => {
                Errand::Nothing
            }
        }
    }
}

/// The sequence that opens the notification manager in the footer.
///
/// A constant for the reason the brain's are, and the one every sequence a
/// sentence names keeps: a sentence once carried these keys as TEXT, and went
/// on teaching `n h` after the key became `n n`. What a sentence names is an
/// argument, never text (`keys-are-never-text`) — and since the leader has
/// three sets, it is [`crate::bindings::taught`]'s answer and not the full tree's
/// sequence: in the default set `n` alone is the manager.
pub const NOTIF_OPEN_KEYS: &str = "n n";

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "notifications",
    commands: &[
        // THE NOTIFICATION MANAGER, and its second door.
        //
        // `notif-open`, the verb every other footer view answers to —
        // `memory-open`, `network-open`, `theme-open`. It was `notif-history`
        // for as long as its key was `n h`; the owner moved the key to `n n`
        // (the doubled letter every prefix's own view uses) and the id followed.
        ronoctua_cmd::Command {
            id: "notif-open",
            label: crate::key::CMD_NOTIF_OPEN,
        },
        // The same manager in a window of its own — the same `View`, and the
        // same reason the file manager has two doors: the footer's region has a
        // ceiling in lines (invariant 14), which is right for a footer and not
        // for reading a fortnight of history.
        ronoctua_cmd::Command {
            id: "notif-window",
            label: crate::key::CMD_NOTIF_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "notif-read-all",
            label: crate::key::CMD_NOTIF_READ_ALL,
        },
        // THE THREE THAT ACT ON THE FOCUSED ROW, and that is why they have no
        // key in the tree: outside the manager there is no focused row to act
        // on. They come in through `notifications::OF_THE_VIEW`, which is what
        // the manual and the orphan check read — the same arrangement the
        // network list already has.
        ronoctua_cmd::Command {
            id: "notif-read",
            label: crate::key::CMD_NOTIF_READ,
        },
        ronoctua_cmd::Command {
            id: "notif-forget",
            label: crate::key::CMD_NOTIF_FORGET,
        },
        ronoctua_cmd::Command {
            id: "notif-act",
            label: crate::key::CMD_NOTIF_ACT,
        },
        // `RET` ON A ROW opens the record whole — body wrapped, buttons drawn,
        // facts beside it. It is a name of its own for `notif-adjust`'s
        // reason: the same key runs the FOCUSED BUTTON a moment later, and one
        // id with two meanings is one label that lies at one of the two.
        ronoctua_cmd::Command {
            id: "notif-see",
            label: crate::key::CMD_NOTIF_SEE,
        },
        ronoctua_cmd::Command {
            id: "notif-filters",
            label: crate::key::CMD_NOTIF_FILTERS,
        },
        ronoctua_cmd::Command {
            id: "notif-new",
            label: crate::key::CMD_NOTIF_NEW,
        },
        ronoctua_cmd::Command {
            id: "notif-drop",
            label: crate::key::CMD_NOTIF_DROP,
        },
        ronoctua_cmd::Command {
            id: "notif-share",
            label: crate::key::CMD_NOTIF_SHARE,
        },
        ronoctua_cmd::Command {
            id: "notif-share-answer",
            label: crate::key::CMD_NOTIF_SHARE_ANSWER,
        },
        ronoctua_cmd::Command {
            id: "notif-draft-type",
            label: crate::key::CMD_NOTIF_DRAFT_TYPE,
        },
        ronoctua_cmd::Command {
            id: "notif-draft-add",
            label: crate::key::CMD_NOTIF_DRAFT_ADD,
        },
        ronoctua_cmd::Command {
            id: "notif-draft-drop",
            label: crate::key::CMD_NOTIF_DRAFT_DROP,
        },
        ronoctua_cmd::Command {
            id: "notif-draft-save",
            label: crate::key::CMD_NOTIF_DRAFT_SAVE,
        },
        // `RET` ON THE CATALOGUE: the entry's sheet. A command of its own for
        // `notif-restore`'s reason — the same key means the other thing on
        // the other column, and one id with two meanings is one label that
        // lies at one of the two levels.
        ronoctua_cmd::Command {
            id: "notif-adjust",
            label: crate::key::CMD_NOTIF_ADJUST,
        },
        // AND THE SIX THAT WALK IT. They are gestures — arrows, `TAB` — and
        // they are in the registry for the reason the three above are: the
        // manager's hint line is GENERATED from `notifications::OF_THE_VIEW`,
        // and a key the line teaches is a key the manual and the orphan check
        // have to be able to name.
        ronoctua_cmd::Command {
            id: "notif-up",
            label: crate::key::CMD_NOTIF_UP,
        },
        ronoctua_cmd::Command {
            id: "notif-down",
            label: crate::key::CMD_NOTIF_DOWN,
        },
        ronoctua_cmd::Command {
            id: "notif-pane-next",
            label: crate::key::CMD_NOTIF_PANE_NEXT,
        },
        ronoctua_cmd::Command {
            id: "notif-pane-previous",
            label: crate::key::CMD_NOTIF_PANE_PREVIOUS,
        },
        ronoctua_cmd::Command {
            id: "notif-knob-next",
            label: crate::key::CMD_NOTIF_KNOB_NEXT,
        },
        ronoctua_cmd::Command {
            id: "notif-knob-previous",
            label: crate::key::CMD_NOTIF_KNOB_PREVIOUS,
        },
        // `r` in the sheet: back to the distro's default. A command of its own
        // because the same letter on a list marks everything read, and one id
        // with two meanings is one label that lies at one of the two levels.
        ronoctua_cmd::Command {
            id: "notif-restore",
            label: crate::key::CMD_NOTIF_RESTORE,
        },
        ronoctua_cmd::Command {
            id: "notif-dismiss",
            label: crate::key::CMD_NOTIF_DISMISS,
        },
    ],
    bindings: &[
        // NOTIFICATIONS. Both were socket orders with no key at all: the
        // history only opened through `ronoctua-shell history`, typed in a
        // terminal. A feature with no path is a feature nobody
        // finds.
        //
        // `n n` — the doubled letter, which is what `s n n`, `t t` and `. .`
        // already do for the view that belongs to a prefix. It was `n h` for
        // "history", and the owner asked for the convention instead.
        (NOTIF_OPEN_KEYS, "notif-open", None),
        ("n d", "notif-dismiss", None),
        // `w` for WINDOW, the same letter every view with a second door uses
        // — see the note over `s u w`.
        ("n w", "notif-window", None),
        ("n r", "notif-read-all", None),
        // `n f` — THE FILTERS, which had no door at all. They are the third
        // column of the manager and the only thing that named them was that
        // column's own title, so the feature existed for whoever pressed `TAB`
        // twice inside a screen they had already opened.
        ("n f", "notif-filters", None),
    ],
    groups: &[("n", crate::key::GROUP_NOTIFICATION)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = crate::notifications::OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .collect();
    out.extend(crate::notifications::OF_THE_SHEET.iter().map(|(_, id)| *id));
    out.extend(crate::notifications::OF_THE_OPEN.iter().map(|(_, id)| *id));
    out.extend(crate::notifications::OF_THE_DRAFT.iter().map(|(_, id)| *id));
    out.extend(
        crate::notifications::OF_THE_QUESTION
            .iter()
            .map(|(_, id)| *id),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_notify::{Notification, Urgency};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn record(id: u32, show: Show, read: bool) -> Record {
        arrived(id, show, read, u64::from(id))
    }

    /// The same, with the ARRIVAL NAMED — which is the other half of a record's
    /// name, and the half two records can differ in while sharing an id.
    ///
    /// The clock is written down and not read: `SystemTime::now()` would make
    /// every key in these tests a different one on every run, and the case that
    /// matters here is precisely two records agreeing on the id.
    fn arrived(id: u32, show: Show, read: bool, secs: u64) -> Record {
        let mut p = Presentation::from_urgency(Urgency::Normal);
        p.show = show;
        let mut n = Notification::fresh_one(
            id,
            "app".into(),
            None,
            format!("r{id}"),
            String::new(),
            Urgency::Normal,
        );
        n.at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        Record {
            notification: n,
            presentation: p,
            read,
        }
    }

    /// The name of a record `record(id, …)` built.
    fn named(id: u32) -> RecordKey {
        RecordKey {
            id,
            at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(u64::from(id)),
        }
    }

    #[test]
    fn the_unread_come_first_and_the_read_follow() {
        // The owner's arrangement: one list, two tones — they are the same
        // thing at two ages, and two lists would make the eye look twice.
        let records = vec![
            record(1, Show::Brief, true),
            record(2, Show::Brief, false),
            record(3, Show::Hidden, false),
            record(4, Show::Brief, false),
        ];
        let l = Lists::of(&records, &Book::distro());
        let ids: Vec<u32> = l.happened.iter().map(|r| r.notification.id).collect();
        // Newest first inside each half: 4 then 2 (unread), then 1 (read).
        assert_eq!(ids, vec![4, 2, 1]);
        // And the suppressed one is not among them — it has a column of its own.
        assert_eq!(l.suppressed.len(), 1);
        assert_eq!(l.suppressed[0].notification.id, 3);
    }

    #[test]
    fn the_catalogue_lists_everything_the_distro_ships() {
        // The owner asked for it by name: one list with all of them, so each
        // can be managed on its own.
        let b = Book::distro();
        let l = Lists::of(&[], &b);
        assert_eq!(l.catalogue.len(), b.entries.len() + b.rules.len());
        assert!(
            l.catalogue
                .iter()
                .any(|(id, kind)| id == "screenshot" && *kind == Kind::Entry)
        );
        assert!(
            l.catalogue
                .iter()
                .any(|(id, kind)| id == "important-gets-room" && *kind == Kind::Rule)
        );
    }

    #[test]
    fn one_column_when_narrow_and_two_when_wide() {
        // The same answer in the footer's region and in the window: what
        // decides is the width it is given, not which door it came through.
        assert!(!two_columns(60));
        assert!(two_columns(tokens::space::NOTIFICATIONS_TWOCOLUMNSFROM));
        assert!(two_columns(200));
        // The left column is the wider one: it holds sentences.
        assert!(split_at(100) > 100 - split_at(100));
    }

    #[test]
    fn a_knob_cycles_and_the_level_moves_by_ten() {
        let mut p = Presentation::from_urgency(Urgency::Normal);
        p.show = Show::Brief;
        let mut v = Verdict::default();
        turn(&p, Knob::Show, 1, &mut v);
        assert_eq!(v.show, Some(Show::Full));
        turn(&p, Knob::Show, -1, &mut v);
        assert_eq!(v.show, Some(Show::Hidden));
        // Ten at a time: one would be eleven presses to cross the range, and
        // nobody tells level 63 from 64.
        p.level = 50;
        turn(&p, Knob::Level, 1, &mut v);
        assert_eq!(v.level, Some(60));
        p.level = 95;
        turn(&p, Knob::Level, 1, &mut v);
        assert_eq!(v.level, Some(TOP_LEVEL), "it went past the ceiling");
        p.level = 5;
        turn(&p, Knob::Level, -1, &mut v);
        assert_eq!(v.level, Some(0), "it went past the floor");
    }

    #[test]
    fn the_two_dead_switches_say_why() {
        // `vibrate` and `audio` are declared and inert. The list comes from
        // `ronoctua_notify::INERT` and not from a second one here — two lists
        // would be one more thing to keep in step, and the day they diverge is
        // the day a switch goes quiet again.
        assert!(Knob::Vibrate.inert().is_some());
        assert!(Knob::Audio.inert().is_some());
        assert!(Knob::Show.inert().is_none());
        assert!(Knob::Loud.inert().is_none());
    }

    #[test]
    fn the_manager_shows_whose_decision_it_is() {
        // "use the distro's default, or customise" is a COMPARISON: a screen
        // that only shows the result cannot say which of the two you are on.
        let mut b = Book::distro();
        let (_, adjusted) = in_force(&b, "screenshot");
        assert!(!adjusted, "no preference, so the distro's default");
        b.adjustments.insert(
            "screenshot".into(),
            Verdict {
                loud: Some(true),
                ..Verdict::default()
            },
        );
        let (p, adjusted) = in_force(&b, "screenshot");
        assert!(adjusted);
        assert!(p.loud);
    }

    #[test]
    fn an_empty_adjustment_is_not_an_adjustment() {
        let mut b = Book::distro();
        b.adjustments
            .insert("screenshot".into(), Verdict::default());
        assert!(
            !in_force(&b, "screenshot").1,
            "an empty preference is not a preference"
        );
    }

    #[test]
    fn the_line_is_the_notification_and_nothing_else() {
        // WHICH FILTER DECIDED IS NOT PART OF THE SENTENCE. It used to be
        // appended to it, raw and untranslated, and the owner read
        // `bluetooth: Bluetooth Keyboard connected by small-things-are-quick`
        // off the screen as a claim that something called
        // `small-things-are-quick` had connected the keyboard. It is a fact
        // about the record, and it went where the facts are.
        let mut r = record(1, Show::Hidden, false);
        r.presentation.by = Some("my-rule".into());
        let fields = record_line(&r, 0, 120, false, true);
        let text: String = fields.iter().map(|f| f.text.as_str()).collect();
        assert!(!text.contains("my-rule"), "{text}");
        assert!(text.contains("app: r1"), "{text}");
    }

    #[test]
    fn two_columns_draw_both_lists_on_one_row() {
        // The owner's arrangement: what happened on the left, what a filter
        // suppressed on the right, side by side while there is room.
        let t = texts();
        let b = Book::distro();
        let records = vec![
            record(1, Show::Brief, false),
            record(2, Show::Hidden, false),
        ];
        let lists = Lists::of(&records, &b);
        let rows = View::fresh().rows(&lists, &b, 200, &t);
        // A heading row, then one row carrying BOTH items.
        assert_eq!(rows.len(), 2);
        let text: String = rows[1].fields.iter().map(|f| f.text.as_str()).collect();
        assert!(text.contains("r1") && text.contains("r2"), "{text}");
        // And the right-hand one starts past the split.
        let split = split_at(200);
        assert!(rows[1].fields.iter().any(|f| f.at >= split));
    }

    #[test]
    fn one_column_stacks_them_instead() {
        // THE OWNER'S ASK: narrow, "Filtered out" goes UNDER "Received"
        // instead of vanishing. It used to draw only the pane with the
        // keyboard, so half the screen's content was reachable solely by
        // pressing `TAB` — a key nothing on screen asked for.
        let t = texts();
        let b = Book::distro();
        let records = vec![
            record(1, Show::Brief, false),
            record(2, Show::Hidden, false),
        ];
        let lists = Lists::of(&records, &b);
        let said = |r: &Row| -> String { r.fields.iter().map(|f| f.text.as_str()).collect() };
        // AND AT 90 TOO, which is about the width the owner photographed: the
        // two columns fitted and cost the sentence, so nine rows all read
        // `bluetooth: Bluetooth Keyboard connect…` and none of them said which
        // was which. Two columns that cost the sentence are two columns that
        // destroy what the list is for.
        for columns in [50_u16, 90] {
            let rows = View::fresh().rows(&lists, &b, columns, &t);
            assert_eq!(
                rows.len(),
                5,
                "at {columns}: {:?}",
                rows.iter().map(said).collect::<Vec<_>>()
            );
        }
        let rows = View::fresh().rows(&lists, &b, 50, &t);
        // Title, its one record, air, the other title, its one record.
        assert!(said(&rows[1]).contains("r1"), "{}", said(&rows[1]));
        assert!(said(&rows[2]).is_empty(), "no air before the second title");
        assert!(said(&rows[4]).contains("r2"), "{}", said(&rows[4]));
        // AND THE TITLE WITH THE KEYBOARD IS THE LIT ONE: stacked, that is the
        // only thing saying which of the two lists the arrows are walking.
        let lit = |r: &Row| r.fields[0].color;
        assert_eq!(lit(&rows[0]), tokens::color::STATE_ACCENT);
        assert_eq!(lit(&rows[3]), tokens::color::TEXT_HEADING);
    }

    #[test]
    fn an_empty_pane_says_so() {
        // An empty region is indistinguishable from a key that did not work,
        // and each column says it for ITSELF — stacked, both are on screen.
        let t = texts();
        let b = Book::distro();
        let rows = View::fresh().rows(&Lists::of(&[], &b), &b, 50, &t);
        assert_eq!(rows.len(), 5);
        assert!(!rows[1].fields.is_empty(), "the first column said nothing");
        assert!(!rows[4].fields.is_empty(), "the second column said nothing");
    }

    #[test]
    fn space_marks_the_focused_one_read() {
        let b = Book::distro();
        let records = vec![record(9, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        assert_eq!(v.key(Press::Space, &lists, &b, 200), Errand::Read(named(9)));
        // A SUPPRESSED one is already off the count, so the same key on the
        // other column does nothing — a key with no effect is worse than a key
        // that is not bound.
        v.pane = Pane::Suppressed;
        assert_eq!(v.key(Press::Space, &lists, &b, 200), Errand::Nothing);
    }

    #[test]
    fn the_sheet_opens_on_the_catalogue_and_esc_backs_out_one_level() {
        // `Esc` walks back ONE level, which is the same contract every buffer's
        // state has: the sheet first, then the view.
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        assert_eq!(v.key(Press::Enter, &lists, &b, 200), Errand::Nothing);
        assert!(v.in_sheet(), "Enter did not open the sheet");
        assert_eq!(v.key(Press::Escape, &lists, &b, 200), Errand::Nothing);
        assert!(!v.in_sheet(), "Esc did not close the sheet");
        assert_eq!(v.key(Press::Escape, &lists, &b, 200), Errand::Close);
    }

    #[test]
    fn the_sheets_row_says_where_the_keyboard_is_and_what_moves() {
        // TWO HALVES, TWO ANSWERS. Both were `state.accent`, so a row stopped
        // saying whether its value moves at the very moment somebody was
        // looking at it — the shape `network.rs` left behind, with its reason
        // written there.
        //
        // A TOKEN TEST AND NOT A PIXEL ONE, and that is measured: `state.accent`
        // and `text.here.acts` are both `accent.solid` and resolve to the SAME
        // tint (`0x5FB975FF` in the default theme), so a screenshot cannot tell
        // the two halves apart. What the split buys is that the row says the
        // right thing the day a theme separates them — `text.here.read`
        // already differs — and that is a statement about tokens, which is
        // where it can be held.
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let t = texts();
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        let _ = v.key(Press::Enter, &lists, &b, 200);
        assert!(v.in_sheet(), "Enter did not open the sheet");
        let rows = v.rows(&lists, &b, 200, &t);
        let lit: Vec<&Row> = rows
            .iter()
            .filter(|r| {
                r.fields
                    .iter()
                    .any(|f| f.color == tokens::color::STATE_ACCENT)
            })
            .collect();
        assert_eq!(lit.len(), 1, "the sheet lights {} rows, not one", lit.len());
        let row = lit[0];
        assert_eq!(
            row.fields[0].color,
            tokens::color::STATE_ACCENT,
            "the label does not say the keyboard is here"
        );
        assert!(
            row.fields
                .iter()
                .skip(1)
                .any(|f| f.color == tokens::color::TEXT_HERE_ACTS),
            "the value wears the label's accent instead of saying it moves"
        );
    }

    #[test]
    fn an_arrow_in_the_sheet_writes_an_adjustment() {
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        let _ = v.key(Press::Enter, &lists, &b, 200);
        let Errand::Adjusted(id, verdict) = v.key(Press::Right, &lists, &b, 200) else {
            panic!("the arrow adjusted nothing");
        };
        assert_eq!(id, lists.catalogue[0].0);
        assert!(verdict.is_some_and(|x| !x.says_nothing()));
        // And `r` puts it back to the distro's — REMOVING the adjustment, not
        // setting it to the same value: "yours" and "the distro's, which
        // happens to agree" are different answers to "who decided this?".
        assert_eq!(
            v.key(Press::Letter('r'), &lists, &b, 200),
            Errand::Adjusted(lists.catalogue[0].0.clone(), None)
        );
    }

    #[test]
    fn every_key_in_the_table_does_something() {
        // THE TABLE IS THE DECLARATION and the `match` is the behaviour, so
        // they can drift. A key that is listed and answers nothing is a line in
        // the manual promising something that does not happen — and it is the
        // check `files-everything-has-a-key` exists for, one view over.
        //
        // A FILTER OF THE PERSON'S OWN, because two of the catalogue's keys
        // are about exactly that — opening one and dropping one — and a
        // catalogue with none of theirs would make both look dead.
        let mut b = Book::distro();
        for id in ["mine", "mine-too"] {
            b.mine.push(ronoctua_notify::Rule {
                id: id.into(),
                when: vec![ronoctua_notify::Term {
                    field: ronoctua_notify::Field::Summary,
                    test: ronoctua_notify::Test::Contains("backup".into()),
                }],
                apply: Verdict::default(),
            });
        }
        let mut r = record(1, Show::Brief, false);
        // TWO buttons, because one of the keys the line teaches is "the next
        // button" — with a single one there is no next, and the arrow would
        // look dead in a test whose whole point is that no taught key is.
        r.notification.actions = vec![
            ronoctua_notify::Action {
                id: "open".into(),
                label: ronoctua_notify::Label::Text("open".into()),
                doing: ronoctua_notify::Doing::Invoke,
            },
            ronoctua_notify::Action {
                id: "copy".into(),
                label: ronoctua_notify::Label::Text("copy".into()),
                doing: ronoctua_notify::Doing::Invoke,
            },
        ];
        // THREE, so that from the middle row both `<up>` and `<down>` have
        // somewhere to go — and the middle one is the one with the action,
        // because that is the row the cursor stands on.
        //
        // AND THREE SUPPRESSED ONES BESIDE THEM, for the same reason on the
        // other column: this test walks every column the line teaches a key
        // on, and an empty column would make every key there look dead.
        let records = vec![
            record(2, Show::Brief, false),
            r,
            record(3, Show::Brief, false),
            record(4, Show::Hidden, false),
            record(5, Show::Hidden, false),
            record(6, Show::Hidden, false),
        ];
        let lists = Lists::of(&records, &b);
        let press_of = |k: &str| match k {
            "SPC" => Press::Space,
            "RET" => Press::Enter,
            "TAB" => Press::Tab,
            "S-TAB" => Press::BackTab,
            "<up>" => Press::Up,
            "<down>" => Press::Down,
            "<left>" => Press::Left,
            "<right>" => Press::Right,
            k => Press::Letter(k.chars().next().unwrap()),
        };
        // ON EVERY COLUMN THE LINE TEACHES IT. A walking key answers
        // `Nothing` and MOVES instead, so "does something" is either an
        // errand or a cursor that is not where it was — from row one, so that
        // `<up>` has somewhere to go.
        //
        // THE COLUMN IS THE POINT. This used to start from `View::fresh()`
        // every time, which is `Pane::Happened` and nothing else — so it
        // could not see that `SPC` and `RET` do nothing on the other two,
        // which is exactly what the line was teaching there.
        for (key, id, column) in OF_THE_VIEW {
            for pane in [Pane::Happened, Pane::Suppressed, Pane::Catalogue] {
                if !column.shown(pane) {
                    continue;
                }
                let mut v = View::fresh();
                v.pane = pane;
                if pane == Pane::Catalogue {
                    // THE PERSON'S OWN FILTERS ARE THE LAST ROWS — the distro
                    // speaks first, then you — and two of this column's keys
                    // are about nothing else. The second-to-last, so that both
                    // arrows still have somewhere to go.
                    v.set_cursor(lists.len(pane).saturating_sub(2));
                } else {
                    let _ = v.key(Press::Down, &lists, &b, 200);
                }
                let (was, cursor) = (v.pane, v.cursor());
                let did = v.key(press_of(key), &lists, &b, 200);
                assert!(
                    did != Errand::Nothing
                        || (v.pane, v.cursor()) != (was, cursor)
                        || v.level != Level::List,
                    "`{key}` ({id}) does nothing on {pane:?}, and the line teaches it there"
                );
            }
        }
        // AND IN THE QUESTION before a filter goes to everybody: the arrows
        // choose between the two answers and `RET` gives the one they left.
        for (key, id) in OF_THE_QUESTION {
            let mut v = View::fresh();
            v.pane = Pane::Catalogue;
            v.set_cursor(lists.len(Pane::Catalogue).saturating_sub(2));
            let _ = v.key(Press::Letter('p'), &lists, &b, 200);
            assert!(
                matches!(v.level, Level::Publish { .. }),
                "`p` did not ask the question"
            );
            let before = v.level.clone();
            let did = v.key(press_of(key), &lists, &b, 200);
            assert!(
                did != Errand::Nothing || v.level != before,
                "`{key}` ({id}) does nothing in the sharing question"
            );
        }
        // AND IN AN OPEN RECORD, where the arrows scroll it and `RET` runs
        // the button they left focused.
        for (key, id) in OF_THE_OPEN {
            let mut v = View::fresh();
            let _ = v.key(Press::Down, &lists, &b, 200);
            let _ = v.key(Press::Enter, &lists, &b, 200);
            assert!(
                matches!(v.level, Level::Open { .. }),
                "`RET` did not open the record"
            );
            // ONTO THE FIRST ACTION. `RET` runs the row under the cursor —
            // on the title there is nothing to run — and from an action both
            // arrows still have somewhere to go.
            let first = v
                .open_plan(&records[1], 200)
                .iter()
                .position(|t| matches!(t, Told::Act(_)))
                .expect("the record has no action to walk to");
            while !matches!(&v.level, Level::Open { line, .. } if *line == first) {
                let _ = v.key(Press::Down, &lists, &b, 200);
            }
            let before = v.level.clone();
            let did = v.key(press_of(key), &lists, &b, 200);
            assert!(
                did != Errand::Nothing || v.level != before,
                "`{key}` ({id}) does nothing in an open record"
            );
        }
        // AND IN THE SHEET, where the same keys mean the sheet's things.
        for (key, id) in OF_THE_SHEET {
            let mut v = View::fresh();
            v.pane = Pane::Catalogue;
            let _ = v.key(Press::Enter, &lists, &b, 200);
            let _ = v.key(Press::Down, &lists, &b, 200);
            let before = v.level.clone();
            let did = v.key(press_of(key), &lists, &b, 200);
            assert!(
                did != Errand::Nothing || v.level != before,
                "`{key}` ({id}) does nothing in the sheet"
            );
        }
    }

    #[test]
    fn every_id_the_line_teaches_has_a_short_text_and_a_label() {
        // The hint line reads the id as a TEXT KEY (`notif-read` → "read"),
        // the way the file manager's does; the manual and which-key read
        // `cmd-<id>`. Both have to exist, in the base language at least — the
        // `i18n-complete` check carries it to every other.
        let t = texts();
        let every = OF_THE_VIEW
            .iter()
            .map(|(_, id, _)| *id)
            .chain(OF_THE_SHEET.iter().map(|(_, id)| *id))
            .chain(OF_THE_OPEN.iter().map(|(_, id)| *id));
        for id in every {
            for key in [id.to_owned(), format!("cmd-{id}")] {
                // `[key]` is what a missing one resolves to; comparing
                // against the key itself could never fail.
                assert!(!t.t(&key).starts_with('['), "`{key}` has no text");
            }
        }
    }

    #[test]
    fn the_hint_line_teaches_the_level_it_is_on() {
        // One table per level, and the line is whichever the cursor is on:
        // with the sheet open `r` is "back to the distro's" and not "all
        // read", and the arrows turn a knob rather than doing nothing.
        let t = texts();
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        let (pane, restore) = (t.t("notif-pane-next"), t.t("notif-restore"));
        let on_list: String = v
            .hint_of(None, &lists, &t)
            .iter()
            .map(|p| p.text.as_str())
            .collect();
        assert!(on_list.contains(&pane), "{on_list}");
        assert!(!on_list.contains(&restore), "{on_list}");
        v.pane = Pane::Catalogue;
        let _ = v.key(Press::Enter, &lists, &b, 200);
        let in_sheet: String = v
            .hint_of(None, &lists, &t)
            .iter()
            .map(|p| p.text.as_str())
            .collect();
        assert!(in_sheet.contains(&restore), "{in_sheet}");
        assert!(!in_sheet.contains(&pane), "{in_sheet}");
        // And the separator between shortcuts is the code's, never a value's.
        assert!(
            v.hint_of(None, &lists, &t)
                .iter()
                .any(|p| p.register == crate::chrome::Register::Separator)
        );
        // EVERY ID REACHES THE LINE: `chrome::hint_of` skips a notation it
        // cannot parse, so a typo in a table would vanish from the screen in
        // silence. Three pieces per id — key, words, and the separator before
        // every one but the first.
        let count = |ids: Vec<&str>| {
            let mut seen: Vec<&str> = Vec::new();
            for id in ids {
                if !seen.contains(&id) {
                    seen.push(id);
                }
            }
            seen.len()
        };
        assert_eq!(
            v.hint_of(None, &lists, &t).len(),
            3 * count(OF_THE_SHEET.iter().map(|(_, id)| *id).collect()) - 1
        );
        // AND ON A COLUMN, only that column's keys — which is what the line
        // teaches now, and is the reason this is not the whole table.
        for pane in [Pane::Happened, Pane::Suppressed, Pane::Catalogue] {
            let mut one = View::fresh();
            one.pane = pane;
            let here: Vec<&str> = OF_THE_VIEW
                .iter()
                .filter(|(_, _, c)| c.shown(pane))
                .map(|(_, id, _)| *id)
                .collect();
            assert_eq!(
                one.hint_of(None, &lists, &t).len(),
                3 * count(here) - 1,
                "{pane:?}"
            );
        }
    }

    #[test]
    fn the_cursor_is_inside_the_list_after_the_row_it_was_on_is_forgotten() {
        // Two records, the cursor on the second, and the second forgotten.
        // The list is one long and the cursor was at 1: no row lit, and `d`,
        // `SPC` and `RET` all dead until an arrow was pressed.
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, false), record(2, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let _ = v.key(Press::Down, &lists, &b, 200);
        assert_eq!(v.cursor(), 1);
        drop(lists);
        let shorter = vec![record(1, Show::Brief, false)];
        let lists = Lists::of(&shorter, &b);
        v.settle(&lists);
        assert_eq!(
            v.cursor(),
            0,
            "the cursor points at a row that is not there"
        );
    }

    #[test]
    fn a_click_in_the_sheet_does_not_change_which_entry_is_open() {
        // The sheet's rows are the knobs, and this used to write the
        // CATALOGUE's index: clicking the sheet's fourth row switched the
        // sheet to the fourth entry, and the next `←` adjusted a kind nobody
        // had opened.
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        let _ = v.key(Press::Enter, &lists, &b, 200);
        assert!(v.in_sheet());
        let was = v.cursor();
        let did = v.clicked(3, Some(4), &lists, &b, 120);
        assert_eq!(did, Errand::Nothing);
        assert_eq!(v.cursor(), was, "the click moved which entry is open");
    }

    #[test]
    fn a_suppressed_row_has_no_envelope_because_the_key_refuses_it() {
        // `Press::Space` on that column returns `Nothing` on purpose. The
        // envelope was drawn anyway, and clicking it told the application the
        // person had dismissed a notification they were never shown.
        let t = texts();
        let b = Book::distro();
        let records = vec![record(1, Show::Hidden, false)];
        let lists = Lists::of(&records, &b);
        let rows = View::fresh().rows(&lists, &b, 120, &t);
        let envelope = tokens::icon::NOTIFICATION_MARK_READ.to_string();
        assert!(
            !rows
                .iter()
                .flat_map(|r| &r.fields)
                .any(|f| f.text == envelope),
            "a suppressed row draws a control the keyboard refuses"
        );
        // AND THE CLICK REFUSES IT TOO, wherever the drawing came from.
        let mut v = View::fresh();
        v.pane = Pane::Suppressed;
        let (at, room) = v.row_extent(120);
        assert_eq!(
            v.clicked(0, Some(at + room - 3), &lists, &b, 120),
            Errand::Nothing
        );
    }

    #[test]
    fn each_empty_column_says_so() {
        // Reported from the screen: with something received and nothing
        // filtered, the right-hand column was blank — "nothing here" only came
        // when BOTH were empty, and only under the left heading.
        let t = texts();
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let rows = View::fresh().rows(&lists, &b, 120, &t);
        assert_eq!(rows.len(), 2);
        let nothing = t.t(crate::key::NOTIF_NOTHING_HERE);
        let (right, _) = extent_of(Pane::Suppressed, 120);
        assert!(
            rows[1]
                .fields
                .iter()
                .any(|f| f.at == right && f.text == nothing),
            "the empty column does not say so: {:?}",
            rows[1].fields
        );
        // And the other way round: the left column says it, the right draws.
        let records = vec![record(2, Show::Hidden, false)];
        let lists = Lists::of(&records, &b);
        let rows = View::fresh().rows(&lists, &b, 120, &t);
        let (left, _) = extent_of(Pane::Happened, 120);
        assert!(
            rows[1]
                .fields
                .iter()
                .any(|f| f.at == left && f.text == nothing)
        );
        assert!(rows[1].fields.iter().any(|f| f.at >= right));
    }

    #[test]
    fn the_column_with_the_keyboard_says_so_even_when_it_is_empty() {
        // `TAB` onto a column with nothing in it used to leave no positive
        // mark anywhere: the only sign was the OTHER column's row losing its
        // accent, and with both columns empty there was no sign at all.
        let t = texts();
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let (left, _) = extent_of(Pane::Happened, 120);
        let split = split_at(120);

        let lit = |v: &View, at: u16| {
            v.rows(&lists, &b, 120, &t)[0]
                .fields
                .iter()
                .find(|f| f.at == at)
                .map(|f| f.color)
                .expect("a heading at that column")
        };
        let mut v = View::fresh();
        assert_eq!(lit(&v, left), tokens::color::STATE_ACCENT);
        assert_eq!(lit(&v, split), tokens::color::TEXT_HEADING);

        // AND ON THE EMPTY ONE, which is the case that had nothing to show.
        let _ = v.key(Press::Tab, &lists, &b, 200);
        assert_eq!(v.pane, Pane::Suppressed);
        assert_eq!(lit(&v, split), tokens::color::STATE_ACCENT);
        assert_eq!(lit(&v, left), tokens::color::TEXT_HEADING);
    }

    #[test]
    fn the_rows_land_where_the_file_managers_do() {
        // Both doors draw a row `chrome::FIELDS_LEAD` in, so what the row
        // carries has to be the gutter and no more — or every notification
        // lands one cell further in than every file, which is what the owner
        // photographed.
        let (at, _) = extent_of(Pane::Happened, 120);
        assert_eq!(
            at + crate::chrome::FIELDS_LEAD,
            crate::chrome::INSET,
            "a notification row does not begin at the inset"
        );
        let (at, _) = extent_of(Pane::Catalogue, 50);
        assert_eq!(at + crate::chrome::FIELDS_LEAD, crate::chrome::INSET);
        // And every path of `rows` — stacked, two columns, the sheet — draws
        // its first field there, not one cell further.
        let t = texts();
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        for (columns, pane) in [
            (50_u16, Pane::Happened),
            (120, Pane::Happened),
            (120, Pane::Catalogue),
        ] {
            let mut v = View::fresh();
            v.pane = pane;
            for row in v.rows(&lists, &b, columns, &t) {
                // The air between two stacked columns draws nothing, and a row
                // with no field has no left edge to be wrong about.
                let Some(first) = row.fields.iter().map(|f| f.at).min() else {
                    continue;
                };
                assert_eq!(
                    first + crate::chrome::FIELDS_LEAD,
                    crate::chrome::INSET,
                    "{pane:?} at {columns}"
                );
            }
            let _ = v.key(Press::Enter, &lists, &b, 200);
            for row in v.rows(&lists, &b, columns, &t) {
                let Some(first) = row.fields.iter().map(|f| f.at).min() else {
                    continue;
                };
                assert_eq!(
                    first + crate::chrome::FIELDS_LEAD,
                    crate::chrome::INSET,
                    "sheet at {columns}"
                );
            }
        }
        // And the right end leaves the file manager's air, in both layouts.
        for columns in [50_u16, 120] {
            let (at, room) = extent_of(Pane::Suppressed, columns);
            assert_eq!(
                at + room + crate::chrome::FIELDS_LEAD + tokens::space::FILES_EDGERIGHT,
                columns
            );
        }
    }

    #[test]
    fn the_left_column_stops_short_of_the_right_one() {
        // Reported from the screen: the bins at the end of the left column sat
        // against the right column's first cell, and read as if they belonged
        // to the notifications over there.
        let split = split_at(200);
        let (at, room) = extent_of(Pane::Happened, 200);
        let gap = split - (at + room);
        assert_eq!(gap, tokens::space::MODELINE_SEGMENTGAP);
        // And the right column does not end against the screen's edge either.
        let (at, room) = extent_of(Pane::Suppressed, 200);
        assert_eq!(at, split);
        assert!(at + room < 200);
    }

    #[test]
    fn the_drawing_and_the_pointer_measure_the_same_row() {
        // They worked it out in two places and agreed by luck. A bin drawn at
        // one cell and hit-tested at another is a control that does nothing
        // where it appears and something where it does not.
        let t = texts();
        let b = Book::distro();
        let records = vec![
            record(1, Show::Brief, false),
            record(2, Show::Hidden, false),
        ];
        let lists = Lists::of(&records, &b);
        for pane in [Pane::Happened, Pane::Suppressed] {
            let mut v = View::fresh();
            v.pane = pane;
            let (at, room) = v.row_extent(200);
            let rows = v.rows(&lists, &b, 200, &t);
            // The row carrying THIS pane's item has a bin at the pane's end.
            assert!(
                rows[1].fields.iter().any(|f| f.at == at + room - 1),
                "{pane:?}: a trash can drawn outside {:?}",
                (at, room)
            );
            assert_eq!(target_at(at + room - 1, at, room), Some(Hit::Delete));
        }
    }

    #[test]
    fn the_two_targets_sit_at_the_row_s_right_end() {
        // Drawn, one per row, each saying what it does — an open envelope and a
        // bin. The pointer's answer and the drawing's come from ONE function,
        // so a bin cannot end up one cell from where it was drawn.
        let t = texts();
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let v = View::fresh();
        let (at, room) = v.row_extent(200);
        let rows = v.rows(&lists, &b, 200, &t);
        let drawn: Vec<u16> = rows[1].fields.iter().map(|f| f.at).collect();
        let end = at + room;
        assert!(drawn.contains(&(end - 1)), "no trash icon at {}", end - 1);
        assert!(drawn.contains(&(end - 3)), "no envelope at {}", end - 3);
        assert_eq!(target_at(end - 1, at, room), Some(Hit::Delete));
        assert_eq!(target_at(end - 3, at, room), Some(Hit::ToggleRead));
        // And the sentence's cells are not targets: a click there only aims.
        assert_eq!(target_at(at + 5, at, room), None);
    }

    #[test]
    fn a_read_one_offers_the_closed_envelope() {
        // The control is there either way, and the DRAWING says which way it
        // goes. It used to be drawn only while unread — which made marking
        // read a one-way door, and a row read by a slip of the hand stayed
        // read for a fortnight with nothing on screen offering otherwise.
        let t = texts();
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, true), record(2, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let (at, room) = v.row_extent(200);
        let rows = v.rows(&lists, &b, 200, &t);
        let glyph = |row: &Row| {
            row.fields
                .iter()
                .find(|f| f.at == at + room - 3)
                .map(|f| f.text.clone())
        };
        let drawn = |c: char| Some(c.to_string());
        // Newest first: row 1 is the unread one, row 2 the read one.
        assert_eq!(
            glyph(&rows[1]),
            drawn(tokens::icon::NOTIFICATION_MARK_READ),
            "the unread row lost its open envelope"
        );
        assert_eq!(
            glyph(&rows[2]),
            drawn(tokens::icon::NOTIFICATION_MARK_UNREAD),
            "the read row has no closed envelope"
        );
        // And the same cell, pressed, goes the other way on each of them.
        // Newest first, so drawn row 1 is id 2 — which is also the unread one
        // here, and was so under the old order for a different reason.
        assert_eq!(
            v.clicked(1, Some(at + room - 3), &lists, &b, 200),
            Errand::Read(named(2))
        );
        assert_eq!(
            v.clicked(2, Some(at + room - 3), &lists, &b, 200),
            Errand::Unread(named(1))
        );
    }

    #[test]
    fn clicking_a_control_acts_and_clicking_the_row_opens_it() {
        // THE OWNER'S RULE: a click on the row IS that row's `RET`. It used to
        // only aim, so reaching a notification meant a click and then a key —
        // while the two controls at the right edge already acted on one press,
        // which is the inconsistency read off the screen.
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, false), record(2, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let (at, room) = v.row_extent(200);
        // Row 0 of the drawing is the heading; newest first, so drawn row 1
        // is id 2 and drawn row 2 is id 1.
        assert_eq!(v.clicked(1, Some(at + 5), &lists, &b, 200), Errand::Nothing);
        assert_eq!(v.cursor(), 0, "the click on the row did not aim");
        let opened = Level::Open {
            at: named(2),
            line: 0,
        };
        assert_eq!(v.level, opened, "clicking the row did not open it");

        // AND IT IS THE SAME THING THE KEY DOES, which is what `open_here`
        // exists for: two answers to "what does this row do" is two answers
        // waiting to differ.
        v.level = Level::List;
        let _ = v.key(Press::Enter, &lists, &b, 200);
        assert_eq!(v.level, opened, "`RET` and the click disagree");
        v.level = Level::List;

        // THE TWO CONTROLS AT THE RIGHT EDGE ARE ASKED FIRST: a click there is
        // that control, and not the row it sits on.
        assert_eq!(
            v.clicked(2, Some(at + room - 3), &lists, &b, 200),
            Errand::Read(named(1))
        );
        assert_eq!(v.level, Level::List, "the envelope opened the record too");
        assert_eq!(
            v.clicked(2, Some(at + room - 1), &lists, &b, 200),
            Errand::Forget(named(1))
        );
        assert_eq!(v.level, Level::List, "the bin opened the record too");
        // AND THE HEADING IS NOT A ROW OF THE LIST: a click on it does nothing
        // and moves nothing, rather than acting on whatever shares its index.
        assert_eq!(
            v.clicked(0, Some(at + room - 1), &lists, &b, 200),
            Errand::Nothing
        );
        assert_eq!(v.level, Level::List, "a heading opened something");
    }

    #[test]
    fn the_list_is_in_arrival_order_with_the_newest_on_top() {
        // THE OWNER'S ORDER, and it replaced their earlier one. The unread
        // were grouped above the read, so one read five seconds ago sat BELOW
        // one from yesterday that had not been — and the list did not answer
        // "what just happened", which is the question one opens it with.
        let b = Book::distro();
        let records = vec![
            arrived(1, Show::Brief, false, 10),
            arrived(2, Show::Brief, true, 20),
            arrived(3, Show::Brief, false, 30),
            arrived(4, Show::Brief, true, 40),
        ];
        let lists = Lists::of(&records, &b);
        let order: Vec<u32> = lists.happened.iter().map(|r| r.notification.id).collect();
        assert_eq!(
            order,
            vec![4, 3, 2, 1],
            "the list is not in arrival order, newest first"
        );
        // AND THE TWO AGES ARE STILL TOLD APART, by tone — which is what made
        // it safe to stop spending POSITION on the difference.
        let read = record_line(lists.happened[0], 0, 80, false, true);
        let unread = record_line(lists.happened[1], 0, 80, false, true);
        assert_ne!(
            read[0].color, unread[0].color,
            "read and unread are drawn the same, and now nothing tells them apart"
        );
    }

    #[test]
    fn the_pointer_says_when_it_arrived_and_the_whole_line() {
        // TWO THINGS THE ROW CANNOT HOLD, both asked for from the screen: a
        // line with a sentence on it has no room for a date, and the sentence
        // itself is cut by `elide_end` to fit.
        let t = texts();
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let v = View::fresh();
        // Drawn row 0 is the heading and says nothing; row 1 is the record.
        assert_eq!(v.about_at(&lists, 200, 0, Some(4), &t), None);
        let said = v
            .about_at(&lists, 200, 1, Some(4), &t)
            .expect("the record says nothing under the pointer");
        assert!(
            said.contains(&records[0].notification.line()),
            "the whole line is not in it: {said}"
        );
        assert!(
            said.contains(&crate::chrome::when_text(
                arrived_secs(&records[0]),
                &v.zone,
                true,
                &t
            )),
            "when it arrived is not in it: {said}"
        );
        // AND IT IS THE CELL THAT SAYS WHICH COLUMN, not the row: side by side
        // the two panes SHARE a line, so a row-only answer would put the left
        // column's date over the right column's notification.
        let records = vec![
            arrived(1, Show::Brief, false, 10),
            arrived(2, Show::Hidden, false, 20),
        ];
        let lists = Lists::of(&records, &b);
        let v = View::fresh();
        let right = extent_of(Pane::Suppressed, 120).0;
        let left = v.about_at(&lists, 120, 1, Some(3), &t).expect("the left");
        let across = v
            .about_at(&lists, 120, 1, Some(right + 1), &t)
            .expect("the right");
        assert_ne!(
            left, across,
            "both halves of one line say the same thing, so the cell was not read"
        );
        // AND A FILTER IS NOT A NOTIFICATION: it has no arrival, and its name
        // is drawn whole.
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        assert_eq!(v.about_at(&lists, 50, 1, Some(3), &t), None);
    }

    #[test]
    fn a_click_in_the_right_column_reaches_it() {
        // Two columns, the cursor on the left, a click on the right column's
        // bin: the pane follows the pointer, and the bin answers.
        let b = Book::distro();
        let records = vec![
            record(1, Show::Brief, false),
            record(2, Show::Hidden, false),
        ];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let (at, room) = list_extent(Pane::Suppressed, 120);
        // Drawn row 0 is the pair of headings; row 1 holds both columns' first
        // item, and the CELL says which of the two the pointer means.
        assert_eq!(
            v.clicked(1, Some(at + room - 1), &lists, &b, 120),
            Errand::Forget(named(2)),
            "the right column's bin did not answer"
        );
        assert_eq!(v.pane, Pane::Suppressed);
        // And a click back on the left column brings the cursor home — and
        // opens what it landed on, which is what a click on a row now does.
        assert_eq!(v.clicked(1, Some(3), &lists, &b, 120), Errand::Nothing);
        assert_eq!(v.pane, Pane::Happened);
        assert!(
            matches!(v.level, Level::Open { .. }),
            "the click did not open the row it landed on"
        );
        v.level = Level::List;

        // STACKED, the columns are one under the other and it is the ROW that
        // says which — clicking in "Filtered out" moves the keyboard there,
        // which is the same rule read down instead of across.
        let mut v = View::fresh();
        // 0 title, 1 the record, 2 air, 3 title, 4 the suppressed one.
        assert_eq!(v.clicked(4, Some(3), &lists, &b, 50), Errand::Nothing);
        assert_eq!((v.pane, v.cursor()), (Pane::Suppressed, 0));
        // And a title is not a row of a list: it moves nothing.
        assert_eq!(v.clicked(0, Some(3), &lists, &b, 50), Errand::Nothing);
        assert_eq!(v.pane, Pane::Suppressed, "a title took the keyboard");

        // AND A CLICK BELOW THE SHORTER COLUMN'S LAST ITEM DOES NOTHING.
        //
        // Wide, the rows run to the LONGER of the two columns, so the right
        // half of the last rows is blank. It used to clamp the cursor to that
        // column's last item — the pointer was on nothing and the keyboard
        // moved two rows away — and before that it acted on the raw index,
        // which named nothing at all. There is no item there; that is the
        // whole answer.
        let records = vec![
            record(1, Show::Brief, false),
            record(2, Show::Brief, false),
            record(3, Show::Brief, false),
            record(4, Show::Hidden, false),
        ];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        assert_eq!(v.clicked(3, Some(at), &lists, &b, 120), Errand::Nothing);
        assert_eq!((v.pane, v.cursor()), (Pane::Happened, 0), "it moved anyway");
    }

    #[test]
    fn the_bells_sentence_says_none_and_not_one_for_zero() {
        // Portuguese's `one` plural category covers 0 as well, so without an
        // exact `[0]` branch, which reads `0 filtered`, came out as the ONE form — a
        // sentence claiming a notification was filtered when none was. Found
        // by the review; measured against the locked `intl_pluralrules`.
        for language in ronoctua_i18n::LANGUAGES {
            let t = ronoctua_i18n::Texts::fresh(language).unwrap();
            let zero = t.t_args(
                crate::key::HINT_NOTIF_BELL,
                &[
                    ("unread", ronoctua_i18n::Arg::Number(2)),
                    ("filtered", ronoctua_i18n::Arg::Number(0)),
                ],
            );
            // THE SAME SENTENCE WITH ONE FILTERED: the two differ only in
            // that half, so "they are the same" is exactly the defect — and
            // in pt-BR they were.
            let one = t.t_args(
                crate::key::HINT_NOTIF_BELL,
                &[
                    ("unread", ronoctua_i18n::Arg::Number(2)),
                    ("filtered", ronoctua_i18n::Arg::Number(1)),
                ],
            );
            assert_ne!(zero, one, "{language}: zero filtered reads as one");
        }
    }

    #[test]
    fn space_puts_a_read_one_back_to_unread() {
        // One key, one control, and the row decides which way it goes. Marking
        // read used to be a one-way door: nothing anywhere undid it, so a row
        // read by a slip of the hand was read for a fortnight.
        let b = Book::distro();
        let records = vec![record(9, Show::Brief, true)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Space, &lists, &b, 200),
            Errand::Unread(named(9))
        );
    }

    #[test]
    fn a_key_names_one_row_when_two_share_an_id() {
        // The owner's report: `Espaço` and `d` acted on notifications BELOW the
        // selected one. The cursor was never wrong — the errand named a record
        // by the protocol's id, which repeats across restarts and, with
        // `replaces_id`, inside one session too. Newest first, so the other
        // ones were always below.
        let b = Book::distro();
        let records = vec![
            arrived(7, Show::Brief, false, 10),
            arrived(7, Show::Brief, false, 30),
        ];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let newest = RecordKey {
            id: 7,
            at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(30),
        };
        let oldest = RecordKey {
            id: 7,
            at: std::time::UNIX_EPOCH + std::time::Duration::from_secs(10),
        };
        assert_eq!(v.key(Press::Space, &lists, &b, 200), Errand::Read(newest));
        assert_eq!(
            v.key(Press::Letter('d'), &lists, &b, 200),
            Errand::Forget(newest)
        );
        // And the row below is a DIFFERENT errand, which is the whole point.
        v.set_cursor(1);
        assert_eq!(v.key(Press::Space, &lists, &b, 200), Errand::Read(oldest));
        assert_eq!(
            v.key(Press::Letter('d'), &lists, &b, 200),
            Errand::Forget(oldest)
        );
    }

    /// A record with a body long enough to need wrapping and two buttons.
    fn wordy() -> Record {
        let mut r = record(1, Show::Brief, false);
        r.notification.summary = "the disk is nearly full".into();
        r.notification.body = "There is less than a gigabyte left on the root filesystem. \
             Something will stop working soon, and it will not say why."
            .into();
        r.presentation.by = Some("small-things-are-quick".into());
        r.notification.actions = vec![
            ronoctua_notify::Action {
                id: "open".into(),
                label: ronoctua_notify::Label::Text("open it".into()),
                doing: ronoctua_notify::Doing::Invoke,
            },
            ronoctua_notify::Action {
                id: "copy".into(),
                label: ronoctua_notify::Label::Text("copy the path".into()),
                doing: ronoctua_notify::Doing::Invoke,
            },
        ];
        r
    }

    #[test]
    fn enter_opens_the_record_whole() {
        // THE OWNER'S ASK: a row that does not fit had nothing to press. Now
        // `RET` gives the body WRAPPED — not cut — every button drawn, and the
        // facts beside it.
        let t = texts();
        let b = Book::distro();
        let records = vec![wordy()];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        assert_eq!(v.key(Press::Enter, &lists, &b, 200), Errand::Nothing);
        assert!(matches!(v.level, Level::Open { .. }), "{:?}", v.level);

        // HALF BY HALF: side by side a drawn row carries a line of each, and
        // what the message says is read down its own half.
        let split = split_at(200);
        let said = |rows: &[Row]| -> String {
            let (left, right): (Vec<String>, Vec<String>) = rows
                .iter()
                .flat_map(|r| r.fields.iter())
                .map(|f| (f.at < split, f.text.clone()))
                .fold((Vec::new(), Vec::new()), |(mut l, mut r), (is_left, t)| {
                    if is_left {
                        l.push(t);
                    } else {
                        r.push(t);
                    }
                    (l, r)
                });
            format!("{} {}", left.join(" "), right.join(" "))
        };
        let rows = v.rows(&lists, &b, 200, &t);
        let text = said(&rows);
        // The whole of the body is there, in pieces no wider than its column.
        assert!(text.contains("will not say why"), "{text}");
        assert!(!text.contains('…'), "the body was cut: {text}");
        // Both actions, as rows of their own.
        assert!(
            text.contains("open it") && text.contains("copy the path"),
            "{text}"
        );
        // AND THE FACTS, including the filter that decided — which is what
        // used to be glued to the end of the row's sentence as a raw id.
        assert!(text.contains(&t.t(crate::key::NOTIF_FACT_ORIGIN)), "{text}");
        assert!(text.contains("Small things go by quickly"), "{text}");

        // WIDE: the facts are the SECOND column, where the list's second
        // column is. NARROW: they are underneath, which is the same answer the
        // list gives and the same token deciding it.
        let (facts_at, _) = extent_of(Pane::Suppressed, 200);
        assert!(
            rows.iter()
                .any(|r| r.fields.iter().any(|f| f.at == facts_at)),
            "no fact in the second column"
        );
        // AND THE BAR IS HIGHER THAN THE LIST'S. A fact is a label AND a value
        // on one line, so its column needs about forty cells — at 90 the two
        // columns of the LIST still fit and the facts no longer do, and they
        // go underneath.
        assert!(
            two_columns(100),
            "the list would already have stacked at 100"
        );
        for columns in [50_u16, 100] {
            let narrow = v.rows(&lists, &b, columns, &t);
            let (left, _) = extent_of(Pane::Catalogue, columns);
            assert!(
                narrow.iter().all(|r| r.fields.iter().all(|f| f.at >= left)),
                "at {columns} a fact stayed in a column that is not there"
            );
            assert!(
                narrow.len() > rows.len(),
                "at {columns} stacked is not taller than side by side"
            );
        }

        // AND `Esc` GOES BACK ONE LEVEL, not out.
        assert_eq!(v.key(Press::Escape, &lists, &b, 200), Errand::Nothing);
        assert_eq!(v.level, Level::List);
        assert_eq!(v.key(Press::Escape, &lists, &b, 200), Errand::Close);
    }

    #[test]
    fn the_row_draws_no_action_and_the_open_record_lists_them() {
        // A notification may carry several actions with labels that are
        // sentences, and a row is one line. The row says nothing of them — the
        // owner took the hand that marked them out — and opening it lists
        // them, one under the other.
        let t = texts();
        let b = Book::distro();
        let records = vec![wordy()];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();

        let row: String = v.rows(&lists, &b, 200, &t)[1]
            .fields
            .iter()
            .map(|f| f.text.as_str())
            .collect();
        assert!(!row.contains("open it"), "the row draws a button: {row}");

        // OPENED, they are ROWS — one each, and in order.
        assert_eq!(v.key(Press::Enter, &lists, &b, 200), Errand::Nothing);
        let rows = v.rows(&lists, &b, 200, &t);
        let said = |r: &Row| -> String { r.fields.iter().map(|f| f.text.as_str()).collect() };
        let first = rows
            .iter()
            .position(|r| said(r).contains("open it"))
            .unwrap();
        let second = rows
            .iter()
            .position(|r| said(r).contains("copy the path"))
            .unwrap();
        assert_eq!(second, first + 1, "the actions are not one under the other");

        // AND ONE CURSOR REACHES THEM. It scrolls the record on the way, which
        // is the other thing a body that does not fit needs. With no titles,
        // the drawn row IS the line of the message it shows.
        while !matches!(v.level, Level::Open { line, .. } if line == first) {
            let before = v.level.clone();
            let _ = v.key(Press::Down, &lists, &b, 200);
            assert_ne!(v.level, before, "the cursor stopped short of the actions");
        }
        assert_eq!(
            v.key(Press::Enter, &lists, &b, 200),
            Errand::Act(named(1), 0)
        );
        let _ = v.key(Press::Down, &lists, &b, 200);
        assert_eq!(
            v.key(Press::Enter, &lists, &b, 200),
            Errand::Act(named(1), 1)
        );

        // AND THE ONE UNDER THE CURSOR IS LIT, which is how the eye knows
        // which of them `RET` would run.
        let rows = v.rows(&lists, &b, 200, &t);
        assert_eq!(
            rows[second].fields[0].color,
            tokens::color::STATE_ACCENT,
            "{}",
            said(&rows[second])
        );
        // AND IT NEVER LANDS ANYWHERE ELSE. A line of the message is not
        // something one can act on, and stepping through them one at a time
        // with nothing lighting up is what the owner read off his screen.
        let plan = v.open_plan(&records[0], 200);
        for press in [Press::Down, Press::Up, Press::Up, Press::Down, Press::Down] {
            let _ = v.key(press, &lists, &b, 200);
            let Level::Open { line, .. } = v.level else {
                panic!("the record closed")
            };
            assert!(
                matches!(plan[line], Told::Act(_)),
                "the cursor stopped on {:?}, which is not an action",
                plan[line]
            );
        }
    }

    #[test]
    fn the_hint_says_which_of_the_two_ret_would_do() {
        // `RET` ON THE CATALOGUE MOVES: the distro's rows are ADJUSTED and a
        // filter the person wrote is opened and rewritten whole. The text
        // changes and so does the colour — the register this system spends on
        // a key whose meaning moves under the reader.
        let t = texts();
        let mut b = Book::distro();
        b.mine.push(ronoctua_notify::Rule {
            id: "quiet-backups".into(),
            when: Vec::new(),
            apply: Verdict::default(),
        });
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        let says = |v: &View| -> Vec<(String, crate::chrome::Register)> {
            v.hint_of(None, &lists, &t)
                .into_iter()
                .map(|p| (p.text, p.register))
                .collect()
        };
        let on_the_distros = says(&v);
        assert!(
            on_the_distros
                .iter()
                .any(|(text, _)| text.contains(&t.t("notif-adjust"))),
            "{on_the_distros:?}"
        );
        assert!(
            on_the_distros
                .iter()
                .all(|(_, r)| *r != crate::chrome::Register::Alternate),
            "nothing moves yet, and the line says something does"
        );

        v.set_cursor(lists.catalogue.len() - 1);
        let on_mine = says(&v);
        assert!(
            on_mine
                .iter()
                .any(|(text, _)| text.contains(&t.t("notif-edit"))),
            "{on_mine:?}"
        );
        assert!(
            on_mine
                .iter()
                .any(|(_, r)| *r == crate::chrome::Register::Alternate),
            "the key changed meaning and kept its colour"
        );
    }
    /// A record whose message is far taller than a small room.
    fn long_one() -> Record {
        let mut r = wordy();
        r.notification.body = "a line of words that goes on ".repeat(40);
        r
    }

    #[test]
    fn the_two_halves_of_an_open_record_scroll_apart() {
        // THE OWNER'S ASK: the message and its facts are isolated, each with
        // its own bar. A long message scrolled its facts off the screen when
        // the two rode on the same rows.
        let t = texts();
        let b = Book::distro();
        let records = vec![long_one()];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let _ = v.key(Press::Enter, &lists, &b, 200);
        v.set_room(5);
        let facts_of_row = |v: &View| -> Vec<String> {
            v.rows(&lists, &b, 200, &t)
                .iter()
                .flat_map(|r| r.fields.iter())
                .filter(|f| f.at >= split_at(200))
                .map(|f| f.text.clone())
                .collect()
        };
        let facts_before = facts_of_row(&v);
        let rows_before = v.rows(&lists, &b, 200, &t).len();
        assert_eq!(rows_before, 5, "the room, and no titles");

        // THE WHEEL OVER THE MESSAGE moves the message and only it.
        assert!(v.open_wheel(2, 2, 3, &lists, 200));
        assert_eq!(
            facts_of_row(&v),
            facts_before,
            "the facts moved with the message"
        );
        assert_eq!(
            v.rows(&lists, &b, 200, &t).len(),
            rows_before,
            "the region moved"
        );
        let bars = v.open_bars(&lists, 200);
        let said = bars
            .iter()
            .find(|b| b.half == Half::First)
            .expect("the message's bar");
        assert_eq!(said.scroll.offset, 3);
        assert!(
            said.column < split_at(200),
            "the message's bar is in its own half"
        );

        // AND OVER THE FACTS, the facts.
        assert!(v.open_wheel(2, split_at(200) + 1, 1, &lists, 200));
        assert_ne!(facts_of_row(&v), facts_before);
        let facts = v.open_bars(&lists, 200);
        let facts = facts
            .iter()
            .find(|b| b.half == Half::Second)
            .expect("the facts' bar");
        assert_eq!(facts.scroll.offset, 1);
        assert!(facts.column >= split_at(200));

        // NEVER PAST THE LAST PAGE.
        assert!(v.open_wheel(2, 2, 10_000, &lists, 200));
        let said = v.open_bars(&lists, 200)[0];
        assert_eq!(said.scroll.offset, said.scroll.total - said.scroll.lines);
    }

    #[test]
    fn a_half_that_fits_has_no_bar_and_no_text_under_one() {
        let t = texts();
        let b = Book::distro();
        let records = vec![wordy()];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let _ = v.key(Press::Enter, &lists, &b, 200);
        v.set_room(40);
        assert!(
            v.open_bars(&lists, 200).is_empty(),
            "a record that fits has no bar"
        );
        // THE BAR'S CELLS ARE KEPT even when there is none, so the day one
        // appears it lands on air and not on a word.
        v.set_room(4);
        for bar in v.open_bars(&lists, 200) {
            for row in v.rows(&lists, &b, 200, &t) {
                for f in &row.fields {
                    let end = f.at + ronoctua_render::layout::width(&f.text);
                    assert!(
                        !(f.at <= bar.column && bar.column < end),
                        "{:?} is drawn under the {:?} bar at {}",
                        f.text,
                        bar.half,
                        bar.column
                    );
                }
            }
        }
    }

    #[test]
    fn narrow_the_halves_are_stacked_and_still_scroll_apart() {
        // TOO NARROW FOR THE FACTS BESIDE: the message under its title and
        // the facts under theirs — and each still has its own window and bar.
        let columns = tokens::space::NOTIFICATIONS_FACTSFROM
            .saturating_sub(1)
            .max(40);
        assert!(!facts_beside(columns));
        let t = texts();
        let b = Book::distro();
        let records = vec![long_one()];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let _ = v.key(Press::Enter, &lists, &b, columns);
        v.set_room(12);
        let rows = v.rows(&lists, &b, columns, &t);
        assert!(rows.len() <= 12, "{} rows in a room of 12", rows.len());
        let [said, facts] = v.places(&lists, columns).unwrap();
        assert_eq!(said.row, 0, "no title above it");
        assert_eq!(
            facts.row,
            said.row + said.room + 1,
            "a line of air sits between"
        );
        assert_eq!(facts.room, FACTS, "the facts keep what they need");
        assert!(
            rows[facts.row - 1].fields.is_empty(),
            "the air is air, not a title"
        );
        // THE WHEEL OVER THE MESSAGE moves the message, and the facts stay.
        let facts_before: Vec<String> = rows[facts.row..]
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(v.open_wheel(2, 2, 3, &lists, columns));
        let rows = v.rows(&lists, &b, columns, &t);
        let facts_after: Vec<String> = rows[facts.row..]
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert_eq!(facts_before, facts_after);
        let bars = v.open_bars(&lists, columns);
        let said_bar = bars
            .iter()
            .find(|b| b.half == Half::First)
            .expect("the message's bar");
        assert_eq!(said_bar.scroll.offset, 3);
        assert!(
            !said_bar.last,
            "the message's track stops where the facts start"
        );
        // AND THE WHEEL OVER THE FACTS is the facts'.
        assert_eq!(
            v.half_at(&lists, columns, facts.row + 1, 2),
            Some(Half::Second)
        );
    }

    #[test]
    fn the_keyboard_goes_from_half_to_half() {
        let t = texts();
        let b = Book::distro();
        let records = vec![long_one()];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let _ = v.key(Press::Enter, &lists, &b, 200);
        v.set_room(5);
        let _ = &t;
        assert!(!v.open_facts, "the message has the keys");
        let _ = v.key(Press::Tab, &lists, &b, 200);
        assert!(v.open_facts, "TAB gave them to the facts");
        // THE ARROWS SCROLL THE FACTS NOW, and the message stays.
        let _ = v.key(Press::Down, &lists, &b, 200);
        let bars = v.open_bars(&lists, 200);
        let facts = bars.iter().find(|b| b.half == Half::Second).unwrap();
        assert_eq!(facts.scroll.offset, 1);
        let said = bars.iter().find(|b| b.half == Half::First).unwrap();
        assert_eq!(said.scroll.offset, 0);
        let _ = v.key(Press::Left, &lists, &b, 200);
        assert!(!v.open_facts, "← went back to the message");
    }

    #[test]
    fn a_click_selects_an_action_and_the_second_runs_it() {
        // INVARIANT 19: the click SELECTS, and the second does what `RET` does.
        let t = texts();
        let b = Book::distro();
        let records = vec![wordy()];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let _ = v.key(Press::Enter, &lists, &b, 200);
        v.set_room(40);
        let rows = v.rows(&lists, &b, 200, &t);
        let row = rows
            .iter()
            .position(|r| r.fields.iter().any(|f| f.text.contains("copy the path")))
            .unwrap();
        assert_eq!(
            v.clicked(row, Some(2), &lists, &b, 200),
            Errand::Nothing,
            "the first selects"
        );
        assert_eq!(
            v.clicked(row, Some(2), &lists, &b, 200),
            Errand::Act(named(1), 1),
            "the second runs it"
        );
        // AND A CLICK IN THE FACTS GIVES THEM THE KEYBOARD, and runs nothing.
        assert_eq!(
            v.clicked(row, Some(split_at(200) + 1), &lists, &b, 200),
            Errand::Nothing
        );
        assert!(v.open_facts);
    }

    #[test]
    fn the_facts_wear_the_info_panel_s_tones_and_no_title_is_drawn() {
        let t = texts();
        let b = Book::distro();
        let records = vec![wordy()];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let _ = v.key(Press::Enter, &lists, &b, 200);
        let rows = v.rows(&lists, &b, 200, &t);
        let all: Vec<&Field> = rows.iter().flat_map(|r| r.fields.iter()).collect();
        assert!(
            !all.iter().any(|f| f.color == tokens::color::TEXT_HEADING),
            "a title was drawn"
        );
        let origin = t.t(crate::key::NOTIF_FACT_ORIGIN);
        let label = all
            .iter()
            .find(|f| f.text.starts_with(&origin[..3]))
            .expect("the origin fact");
        assert_eq!(label.color, tokens::color::TEXT_DIM);
        let value = all
            .iter()
            .find(|f| {
                f.at > label.at && f.at >= split_at(200) && f.text == records[0].notification.app
            })
            .expect("its value");
        assert_eq!(value.color, tokens::color::TEXT_PRIMARY);
    }

    #[test]
    fn the_band_opens_the_record_it_was_showing() {
        let b = Book::distro();
        let records = vec![record(1, Show::Brief, false), record(2, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        let second = records[0].key();
        v.open_on(second, &lists);
        assert!(
            matches!(v.level, Level::Open { at, .. } if at == second),
            "{:?}",
            v.level
        );
        // AND THE LIST'S CURSOR IS ON IT, so `Esc` lands on it.
        let row = lists
            .happened
            .iter()
            .position(|r| r.key() == second)
            .unwrap();
        assert_eq!(v.cursor(), row);
    }

    #[test]
    fn an_open_record_is_as_tall_as_it_says_it_is() {
        // TWO ANSWERS TO "HOW TALL IS THIS" is how a cursor comes to stop three
        // lines short of the end: the arrows ask `open_height` before there is
        // anything drawn to count, and the drawing has to agree with it.
        let t = texts();
        let records = [wordy(), record(2, Show::Brief, false)];
        let mut v = View::fresh();
        v.open(records[0].key());
        for r in [&records[0], &records[1]] {
            for columns in [40_u16, 50, 80, 120, 200] {
                // SIDE BY SIDE: one row of headings, then as many as the
                // taller half — the facts are a closed set of their own.
                // STACKED: each half under its own heading.
                let said = v.open_plan(r, columns).len();
                let tall = if facts_beside(columns) {
                    said.max(FACTS)
                } else {
                    said + 1 + FACTS
                };
                assert_eq!(
                    v.open_rows(r, columns, &t).len(),
                    tall,
                    "at {columns} columns"
                );
            }
        }
    }

    #[test]
    fn a_filter_can_be_written_from_the_screen() {
        // THE OWNER'S ASK. The rule language was complete and unreachable: the
        // only way to a filter of one's own was editing the preferences JSON,
        // in serde's variant spelling.
        let t = texts();
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        assert_eq!(v.key(Press::Letter('n'), &lists, &b, 200), Errand::Nothing);
        assert!(matches!(v.level, Level::Draft(_)), "`n` opened nothing");

        // IT REFUSES TO SAVE HALF A FILTER, and says why on the line that
        // exists for saying why.
        assert_eq!(v.key(Press::Letter('s'), &lists, &b, 200), Errand::Nothing);
        assert!(
            matches!(v.level, Level::Draft(_)),
            "half a filter was saved"
        );
        assert!(v.note(&t).is_some(), "and it said nothing about refusing");

        // NAMED AND FILLED IN — the two things that are typed come in on a
        // door's own line, which is what `Errand::Ask` sends the door to open,
        // and go back through `typed`.
        assert_eq!(
            v.key(Press::Enter, &lists, &b, 200),
            Errand::Ask(Asking::Name)
        );
        assert_eq!(v.typing(), Some(("notif-filter-name", String::new())));
        v.typed("quiet backups");
        // Down to the value of the first condition: name, field, test, value.
        for _ in 0..3 {
            let _ = v.key(Press::Down, &lists, &b, 200);
        }
        assert_eq!(
            v.key(Press::Enter, &lists, &b, 200),
            Errand::Ask(Asking::Value(0))
        );
        assert_eq!(v.typing(), Some(("notif-filter-value", String::new())));
        v.typed("backup");

        // AND THE ARROWS TURN WHAT CANNOT BE TYPED. Up to the field, one step
        // on, and the condition is about the sender instead of the title.
        let _ = v.key(Press::Up, &lists, &b, 200);
        let _ = v.key(Press::Up, &lists, &b, 200);
        let _ = v.key(Press::Left, &lists, &b, 200);
        let Level::Draft(d) = &v.level else {
            panic!("the draft closed")
        };
        assert_eq!(d.when[0].field, ronoctua_notify::Field::App);
        assert_eq!(d.when[0].test.against(), "backup");

        // SAVED, and it goes back to the list it came from.
        let Errand::Filter { was, rule } = v.key(Press::Letter('s'), &lists, &b, 200) else {
            panic!("`s` did not save a filter that is ready")
        };
        assert_eq!(was, None, "a new filter replaces nothing");
        assert_eq!(rule.id, "quiet backups");
        assert_eq!(rule.when.len(), 1);
        assert_eq!(v.level, Level::List);
    }

    /// THE LINE STARTS FROM WHAT THE DRAFT HOLDS, in both doors; an empty
    /// answer keeps it; and an answer nobody asked for — or asked before a
    /// key let the line go — writes nothing.
    #[test]
    fn a_typed_answer_starts_from_the_draft_and_an_empty_one_keeps_it() {
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        let _ = v.key(Press::Letter('n'), &lists, &b, 200);
        let _ = v.key(Press::Enter, &lists, &b, 200);
        v.typed("backups");
        v.typed("nobody asked for this");
        let name = |v: &View| match &v.level {
            Level::Draft(d) => d.name.clone(),
            other => panic!("the draft closed: {other:?}"),
        };
        assert_eq!(name(&v), "backups");

        let _ = v.key(Press::Enter, &lists, &b, 200);
        assert_eq!(
            v.typing(),
            Some(("notif-filter-name", "backups".to_owned()))
        );
        v.typed("");
        assert_eq!(name(&v), "backups", "an empty answer wiped the name");

        // A LINE LET GO: the next key reaches the view, and the question is
        // over.
        let _ = v.key(Press::Enter, &lists, &b, 200);
        let _ = v.key(Press::Down, &lists, &b, 200);
        assert_eq!(v.typing(), None);
        v.typed("late");
        assert_eq!(name(&v), "backups");

        // AND A VALUE starts from its own text.
        for _ in 0..2 {
            let _ = v.key(Press::Down, &lists, &b, 200);
        }
        assert_eq!(
            v.key(Press::Enter, &lists, &b, 200),
            Errand::Ask(Asking::Value(0))
        );
        v.typed("backup");
        let _ = v.key(Press::Enter, &lists, &b, 200);
        assert_eq!(
            v.typing(),
            Some(("notif-filter-value", "backup".to_owned()))
        );
    }

    /// A record with one action of each kind, about a file.
    fn acting() -> Record {
        use ronoctua_notify::action::{Action, Arg, Doing, Label};
        let mut r = record(7, Show::Brief, false);
        r.notification.icon = Some("file:///home/a/shots/one%20shot.png".into());
        let one = |id: &str, doing: Doing| Action {
            id: id.into(),
            label: Label::Text(id.into()),
            doing,
        };
        r.notification.actions = vec![
            one("reply", Doing::Invoke),
            one("see", Doing::Command("bluetooth-open".into())),
            one("path", Doing::CopyPath),
            one("image", Doing::CopyImage),
            one(
                "folder",
                Doing::Run {
                    program: ronoctua_notify::Program::Open,
                    args: vec![Arg::Literal("--".into()), Arg::Folder, Arg::File],
                },
            ),
        ];
        r
    }

    /// EVERY KIND OF ACTION COMES TO ONE DEED, the same in both doors — and
    /// only the bus's button and a command are the footer's to do.
    #[test]
    fn every_action_comes_to_one_deed() {
        let r = acting();
        let file = std::path::PathBuf::from("/home/a/shots/one shot.png");
        let deeds: Vec<Deed> = (0..5).filter_map(|i| deed_of(&r, i)).collect();
        assert_eq!(
            deeds,
            [
                Deed::Invoke("reply".into()),
                Deed::Command("bluetooth-open".into()),
                Deed::CopyPath("/home/a/shots/one shot.png".into()),
                Deed::CopyImage(file),
                Deed::Run(
                    ronoctua_notify::Program::Open,
                    vec![
                        "--".into(),
                        "/home/a/shots".into(),
                        "/home/a/shots/one shot.png".into()
                    ]
                ),
            ]
        );
        let footers: Vec<bool> = deeds.iter().map(Deed::the_footers).collect();
        assert_eq!(footers, [true, true, false, false, false]);
        assert_eq!(deed_of(&r, 5), None, "an action that is not there");

        // WITHOUT THE FILE, what needs it is not run at all.
        let mut bare = acting();
        bare.notification.icon = None;
        let kept: Vec<Deed> = (0..5).filter_map(|i| deed_of(&bare, i)).collect();
        assert_eq!(
            kept,
            [
                Deed::Invoke("reply".into()),
                Deed::Command("bluetooth-open".into())
            ]
        );
    }

    /// A COPY IS ONE CALL, from either door, and says how it went.
    #[test]
    fn a_copying_deed_copies_and_says_so() {
        crate::clipboard_copy::COPIED.with(|c| c.borrow_mut().clear());
        assert_eq!(
            copy_it(&Deed::CopyPath("/tmp/a b".into())),
            Some(crate::key::ECHO_COPIED)
        );
        let copied = crate::clipboard_copy::COPIED.with(|c| c.borrow().clone());
        assert_eq!(copied, [(None, b"/tmp/a b".to_vec())]);
        assert_eq!(copy_it(&Deed::Command("x".into())), None);
    }

    /// THE WINDOW'S ORDER NAMES THE RECORD WHOLE and reads back as it was
    /// written; anything else is refused, not taken for the focused row.
    #[test]
    fn an_action_order_names_the_record_and_reads_back() {
        let key = RecordKey {
            id: 42,
            at: std::time::UNIX_EPOCH + std::time::Duration::new(1_790_000_000, 123_456_789),
        };
        let order = act_order(key, 3);
        assert_eq!(act_of(&order), Some((key, 3)), "{order}");
        for wrong in ["", "42", "42 1", "x 1 2", "42 1 2 3", "42 -1 2"] {
            assert_eq!(act_of(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn a_filter_of_yours_is_told_apart_edited_and_dropped() {
        // Three things the screen could not do: SAY that a filter is yours,
        // open it to change it, and take it away.
        let t = texts();
        let mut b = Book::distro();
        b.mine.push(ronoctua_notify::Rule {
            id: "quiet backups".into(),
            when: vec![ronoctua_notify::Term {
                field: ronoctua_notify::Field::Summary,
                test: ronoctua_notify::Test::Contains("backup".into()),
            }],
            apply: Verdict {
                show: Some(Show::Hidden),
                ..Verdict::default()
            },
        });
        let lists = Lists::of(&[], &b);
        let last = lists.catalogue.len() - 1;
        assert_eq!(lists.catalogue[last], ("quiet backups".into(), Kind::Mine));

        let mut v = View::fresh();
        v.pane = Pane::Catalogue;

        // ITS NAME IS THE TEXT THEY TYPED, not a key that does not exist —
        // the row used to read `[notif-rule-quiet backups]`.
        let rows = v.rows(&lists, &b, 200, &t);
        let said: String = rows[last + 1]
            .fields
            .iter()
            .map(|f| f.text.as_str())
            .collect();
        assert!(said.contains("quiet backups"), "{said}");
        assert!(
            !said.contains('['),
            "the missing-key marker is on screen: {said}"
        );
        assert!(said.contains(&t.t(crate::key::NOTIF_MINE)), "{said}");
        // AND ITS OWN TONE, which is what the owner asked for: in forty rows,
        // the handful that are yours are found without reading. Read with the
        // cursor ELSEWHERE, because the cursor wins over provenance — a
        // focused row is the accent whoever wrote it.
        assert_eq!(rows[last + 1].fields[0].color, tokens::color::TEXT_MINE);
        v.set_cursor(last);

        // AND WHAT IT DOES IS DRAWN. `in_force` looked in the distro's filters
        // only, so a filter of theirs showed the plain arrival defaults.
        let (p, _) = in_force(&b, "quiet backups");
        assert_eq!(p.show, Show::Hidden, "the row does not say what it does");

        // `RET` OPENS IT WHOLE — conditions included, which is the difference
        // between editing a filter and adjusting a kind.
        assert_eq!(v.key(Press::Enter, &lists, &b, 200), Errand::Nothing);
        let Level::Draft(d) = &v.level else {
            panic!("`RET` did not open the filter")
        };
        assert_eq!(d.editing.as_deref(), Some("quiet backups"));
        assert_eq!(d.when.len(), 1);
        let _ = v.key(Press::Escape, &lists, &b, 200);

        // AND `d` TAKES IT AWAY.
        assert_eq!(
            v.key(Press::Letter('d'), &lists, &b, 200),
            Errand::Unfilter("quiet backups".to_owned())
        );
        // But never one of the distro's: those come from the flake, and the
        // next rebuild would put back whatever the screen pretended to delete.
        v.set_cursor(0);
        assert_eq!(v.key(Press::Letter('d'), &lists, &b, 200), Errand::Nothing);
    }

    #[test]
    fn sharing_asks_first_and_answers_no_by_default() {
        // A GESTURE THAT CANNOT BE UNDONE gets a question, and the question
        // opens on `no` — the format buffer's rule, for the same reason. What
        // lands in a public repository is published, and git keeps it.
        let t = texts();
        let mut b = Book::distro();
        b.mine.push(ronoctua_notify::Rule {
            id: "quiet-backups".into(),
            when: vec![ronoctua_notify::Term {
                field: ronoctua_notify::Field::Summary,
                test: ronoctua_notify::Test::Contains("backup".into()),
            }],
            apply: Verdict::default(),
        });
        let lists = Lists::of(&[], &b);
        let last = lists.catalogue.len() - 1;
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        v.set_cursor(last);
        assert_eq!(v.key(Press::Letter('p'), &lists, &b, 200), Errand::Nothing);
        assert_eq!(
            v.level,
            Level::Publish {
                id: "quiet-backups".to_owned(),
                yes: false
            },
            "the question did not open on `no`"
        );

        // IT SHOWS WHAT WOULD GO, and it says what it means.
        let said: String = v
            .rows(&lists, &b, 200, &t)
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(said.contains("quiet-backups"), "{said}");
        assert!(
            said.contains("backup"),
            "the condition is not on screen: {said}"
        );
        assert!(
            said.contains("PUBLIC"),
            "it does not say what it means: {said}"
        );

        // `RET` ON `no` IS NO, and it goes back with nothing sent.
        assert_eq!(v.key(Press::Enter, &lists, &b, 200), Errand::Nothing);
        assert_eq!(v.level, Level::List);

        // AND `yes` IS THE ONE THAT SENDS IT.
        let _ = v.key(Press::Letter('p'), &lists, &b, 200);
        let _ = v.key(Press::Down, &lists, &b, 200);
        assert_eq!(
            v.key(Press::Enter, &lists, &b, 200),
            Errand::Share("quiet-backups".to_owned())
        );
    }

    #[test]
    fn a_name_that_cannot_be_a_name_in_the_repository_is_refused() {
        // Invariant 18: every name in this repository is English kebab, and the
        // id is what an adjustment is keyed by. Mangling `silenciar cópias`
        // into something else behind somebody's back would be worse than the
        // screen saying so.
        let t = texts();
        let mut b = Book::distro();
        b.mine.push(ronoctua_notify::Rule {
            id: "silenciar cópias".into(),
            when: vec![ronoctua_notify::Term {
                field: ronoctua_notify::Field::Summary,
                test: ronoctua_notify::Test::Contains("backup".into()),
            }],
            apply: Verdict::default(),
        });
        let lists = Lists::of(&[], &b);
        let last = lists.catalogue.len() - 1;
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        v.set_cursor(last);
        let _ = v.key(Press::Letter('p'), &lists, &b, 200);
        let said: String = v
            .rows(&lists, &b, 200, &t)
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            said.contains(&t.t(crate::key::NOTIF_SHARE_NOT_A_NAME)[..20]),
            "{said}"
        );
        // And there is no `yes` to press: the two answers are not offered.
        assert!(!said.contains(&t.t(crate::key::NOTIF_SHARE_YES)), "{said}");
        // Even `yes` forced in answers nothing, which is what makes the
        // refusal a refusal rather than a drawing.
        v.level = Level::Publish {
            id: "silenciar cópias".to_owned(),
            yes: true,
        };
        assert_eq!(v.key(Press::Enter, &lists, &b, 200), Errand::Nothing);
    }

    #[test]
    fn what_came_back_as_the_distros_retires_the_copy_that_travelled() {
        // THE "HAS IT ARRIVED" QUESTION, and the whole of the answer: the table
        // this binary was COMPILED with either has it or does not. No network,
        // no git, no clock — a build from the flake is the only road distro
        // content travels.
        let mine = vec![ronoctua_notify::Rule {
            id: "quiet-backups".into(),
            when: Vec::new(),
            apply: Verdict::default(),
        }];
        let shared: std::collections::BTreeSet<String> =
            std::iter::once("quiet-backups".to_owned()).collect();

        // STILL TRAVELLING: it is not in the distro's rules, so it stays, and
        // the catalogue says so.
        let mut b = Book::distro();
        b.took(mine.clone(), shared.clone());
        assert_eq!(b.mine.len(), 1);
        let lists = Lists::of(&[], &b);
        let last = lists.catalogue.len() - 1;
        assert_eq!(lists.catalogue[last].1, Kind::Shared);

        // ARRIVED: the build brought it in, so the local copy has done its job.
        let mut b = Book::distro();
        b.rules.push(ronoctua_notify::Rule {
            id: "quiet-backups".into(),
            when: Vec::new(),
            apply: Verdict::default(),
        });
        b.took(mine, shared);
        assert!(b.mine.is_empty(), "the copy that travelled is still here");
        assert!(b.shared.is_empty(), "and it is still called travelling");
        let lists = Lists::of(&[], &b);
        assert_eq!(
            lists
                .catalogue
                .iter()
                .filter(|(id, _)| id == "quiet-backups")
                .count(),
            1,
            "it is in the catalogue twice"
        );
    }

    #[test]
    fn d_forgets_the_focused_one() {
        // The keyboard's half of the bin. The generator refuses an action that
        // asks for `d`, so nothing can shadow it.
        let b = Book::distro();
        let records = vec![record(7, Show::Brief, false)];
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Letter('d'), &lists, &b, 200),
            Errand::Forget(named(7))
        );
        // On the catalogue there is nothing to forget: those are rows of a
        // table, not things that happened.
        v.pane = Pane::Catalogue;
        assert_eq!(v.key(Press::Letter('d'), &lists, &b, 200), Errand::Nothing);
    }

    #[test]
    fn the_sheet_says_why_a_switch_is_dead_on_the_message_line() {
        // It was a third column on the row and the owner asked for it gone. The
        // honesty had to stay — the trap has a name in `AGENTS.md` — so it
        // moved to the line this footer already explains things on.
        let t = texts();
        let b = Book::distro();
        let lists = Lists::of(&[], &b);
        let mut v = View::fresh();
        v.pane = Pane::Catalogue;
        let _ = v.key(Press::Enter, &lists, &b, 200);
        // The first knob works, so it has nothing to add.
        assert_eq!(v.note(&t), None);
        // Walk down to `vibrate`.
        let dead = Knob::ALL.iter().position(|k| k.inert().is_some()).unwrap();
        for _ in 0..dead {
            let _ = v.key(Press::Down, &lists, &b, 200);
        }
        assert!(v.note(&t).is_some(), "the dead switch stayed silent");
        // And the row itself carries no third column any more.
        let rows = v.rows(&lists, &b, 200, &t);
        for (i, r) in rows.iter().enumerate().skip(1) {
            let texts: Vec<&str> = r.fields.iter().map(|f| f.text.as_str()).collect();
            assert!(r.fields.len() <= 2, "line {i} with {texts:?}");
        }
    }

    #[test]
    fn changing_pane_brings_the_cursor_inside_it() {
        // Each pane keeps its own cursor — coming back to a list should come
        // back to where you were — but a cursor past the end would light a row
        // that is not there and act on nothing.
        let b = Book::distro();
        let records: Vec<Record> = (0..5).map(|i| record(i, Show::Brief, false)).collect();
        let lists = Lists::of(&records, &b);
        let mut v = View::fresh();
        for _ in 0..4 {
            let _ = v.key(Press::Down, &lists, &b, 200);
        }
        assert_eq!(v.cursor(), 4);
        // The suppressed list is empty here.
        let _ = v.key(Press::Tab, &lists, &b, 200);
        assert_eq!(v.pane, Pane::Suppressed);
        assert_eq!(v.cursor(), 0);
        // And the one it left is where it was.
        let _ = v.key(Press::Tab, &lists, &b, 200);
        let _ = v.key(Press::Tab, &lists, &b, 200);
        assert_eq!(v.pane, Pane::Happened);
        assert_eq!(v.cursor(), 4);
    }

    #[test]
    fn tab_goes_round_the_three_panes() {
        let mut p = Pane::default();
        let mut seen = vec![p];
        for _ in 0..3 {
            p = p.next();
            seen.push(p);
        }
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[0], seen[3], "TAB has to go back to the start");
    }
}
