//! The file manager's view — `LEADER .` in the footer, and the window.
//!
//! # One implementation, two doors
//!
//! This module holds STATE and produces [`Row`]s. It draws nothing, talks to no
//! compositor and opens no process, which is what lets the same code serve the
//! footer's expanded region (twelve lines, by `space.expansion`) and a window of
//! its own with no ceiling at all. It is the shape the rescue console already
//! uses, and for the same reason: a manager that only works inside the footer
//! is unusable for the job people open a file manager to do.
//!
//! # The pointer and the keyboard reach the same places
//!
//! A [`Field`] carries a [`Hit`], and every `Hit` is something a key also does.
//! That is not a convention to remember — `files-everything-has-a-key` compares
//! the two lists in both directions, which is the owner's rule ("whatever the
//! mouse can do the keyboard can do") made mechanical.
//!
//! # And a field explains itself
//!
//! A `Field` also carries an [`Explanation`]: a text KEY plus arguments, which
//! whoever holds the language bundle turns into the sentence that appears on
//! the message line when the pointer rests on it. There is no floating tooltip
//! and there will not be one — invariant 14 — so the explanation goes where
//! which-key already puts a description.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ronoctua_files::{
    Entry, Kind,
    listing::{self, Order, Sort},
    menu::{self, Action},
    mime,
    mode::{self, Explanation},
    size,
    when::{self, Zone},
    who::Roster,
};
use ronoctua_keymap::{Key, Sym};
use ronoctua_tokens::Color;
use ronoctua_transfers::Work;
use smithay_client_toolkit::reexports::calloop;

/// A column of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// The glyph for what this is.
    Icon,
    /// The name. Never dropped, whatever the width.
    Name,
    /// `drwxr-xr-x`, one cell per fact.
    Perms,
    /// Who owns it.
    Owner,
    /// Which group.
    Group,
    /// How big.
    Size,
    /// When it last changed.
    Modified,
}

impl Column {
    /// The text key naming this column in the header.
    #[must_use]
    pub const fn label(self) -> &'static str {
        use ronoctua_i18n::key;
        match self {
            // The icon column has no header: a one-cell label is not a word,
            // and the drawing under it says what it is.
            Self::Icon => key::FILES_COLUMN_ICON,
            Self::Name => key::FILES_COLUMN_NAME,
            Self::Perms => key::FILES_COLUMN_PERMS,
            Self::Owner => key::FILES_COLUMN_OWNER,
            Self::Group => key::FILES_COLUMN_GROUP,
            Self::Size => key::FILES_COLUMN_SIZE,
            Self::Modified => key::FILES_COLUMN_MODIFIED,
        }
    }

    /// Which sort order clicking this column asks for.
    ///
    /// Three of the seven have no order of their own — one cannot sort by
    /// permission bits in any way a person means — and they fall back to the
    /// kind, which is the closest thing to what somebody clicking there wants:
    /// the files grouped by what they are.
    #[must_use]
    pub const fn sorts_by(self) -> Order {
        match self {
            Self::Size => Order::Size,
            Self::Modified => Order::Modified,
            Self::Icon | Self::Perms | Self::Owner | Self::Group => Order::Kind,
            Self::Name => Order::Name,
        }
    }

    /// How many cells this column wants.
    ///
    /// Fixed for everything but the name, which takes what is left. `Modified`
    /// is sixteen because that is `2026-08-20 14:32` exactly, and a date column
    /// that elides is a date column that lies about the year.
    const fn width(self) -> u16 {
        match self {
            // The glyph plus one cell of air. Without the air the icon touches
            // the first letter and reads as part of the name.
            Self::Icon => 2,
            Self::Name => 0,
            Self::Perms => 10,
            Self::Owner | Self::Group => 8,
            // `1023,9 KiB` is the longest a size gets.
            Self::Size => 10,
            Self::Modified => 16,
        }
    }
}

/// THE ORDER IN WHICH COLUMNS GIVE WAY, first to go at the front.
///
/// It is a statement about what a file listing is FOR. The name is what one
/// navigates by and never goes; the icon survives almost everything because it
/// costs two cells and answers "what is this" faster than any word; the size
/// and the date are what people compare; the permission triplet and the two
/// names are what they look up, deliberately, which is exactly the thing one
/// can afford to widen the window for.
///
/// Pinned by `files-columns-degrade`, because an order that drifts turns the
/// narrow case into whatever the last edit happened to leave.
pub const GIVE_WAY: [Column; 6] = [
    Column::Group,
    Column::Owner,
    Column::Perms,
    Column::Modified,
    Column::Size,
    // THE ICON GOES LAST, and it does go.
    //
    // It survives almost everything because it costs two cells and answers
    // "what is this" faster than any word. But "almost" is not "never": at a
    // width of one cell the icon alone pushed the name off the screen, and the
    // compositor does not clip — the name simply was not there. What never
    // gives way is the name, and the rule that decides is "nothing leaks",
    // exactly as in `layout::resolve`.
    Column::Icon,
];

/// Where a column sits, once the width is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// `(column, first cell, width)`, left to right.
    pub columns: Vec<(Column, u16, u16)>,
    /// The listing's first cell — where the sidebar ends, or zero.
    ///
    /// THE MARK GOES HERE, and not at cell zero. That one line is the whole of
    /// "the mark travels with the listing"; left at zero it sits under the
    /// sidebar's last letter, and clicking it marks something else.
    pub from: u16,
}

impl Layout {
    /// Where a column starts, if it survived.
    ///
    /// Only the tests ask: the drawing WALKS the columns in order, because it
    /// has to draw all of them anyway and looking each one up would be the same
    /// list traversed seven times.
    #[cfg(test)]
    fn find(&self, which: Column) -> Option<(u16, u16)> {
        self.columns
            .iter()
            .find(|(c, _, _)| *c == which)
            .map(|(_, at, w)| (*at, *w))
    }
}

/// The cell before every column: the mark's, and the air the icon needs.
///
/// From the token, and not a `1` with a comment saying which token it equals —
/// invariant 9 is about the number having ONE home, and a copy with a note
/// beside it is still a copy.
const GUTTER: u16 = ronoctua_tokens::space::FILES_GUTTER;

/// The air AFTER the last column.
///
/// Without it the date ends flush against the edge and reads as a column that
/// was cut off rather than one that ended. It is not the modeline's
/// `edgeRight`, which is zero on purpose so the notification icon can sit
/// flush: a table is not a status line.
const EDGE_RIGHT: u16 = ronoctua_tokens::space::FILES_EDGERIGHT;

/// The narrowest a name may be squeezed to before a column is dropped instead.
///
/// Twelve cells hold `relatorio.pd…`. Below that the name stops identifying
/// anything, and a table of unreadable names with a tidy date column has its
/// priorities backwards.
const NAME_FLOOR: u16 = 12;

/// The narrowest the sidebar may be squeezed to.
///
/// Two cells of glyph and air, plus eleven for a name: `screenshot` is ten and
/// `Downloads` nine. Below that the row stops identifying the place it leads
/// to, which is the same argument [`NAME_FLOOR`] makes about a file's name.
const SIDE_FLOOR: u16 = 13;

/// The cell the rule between the sidebar and the listing sits in.
///
/// A one-pixel rule and one cell of grid: separation by whitespace and a rule,
/// never by a container (invariant 15).
const RULE: u16 = 1;

/// THE AIR BEFORE THE SIDEBAR'S GLYPH, at the screen's own edge.
///
/// A glyph flush against a border does not read as a glyph at the edge: it
/// reads as one that was CUT OFF. This file's own `EDGE_RIGHT` says it about
/// the date column and `space.nix` says it twice more, and the sidebar shipped
/// with its icon at cell zero anyway — because the footer's files `Grid` is the
/// one grid in the shell built without a `start()`, so the `modeline.edgeLeft`
/// every other list gets never reaches a file row.
const SIDE_GUTTER: u16 = ronoctua_tokens::space::FILES_SIDEBARGUTTER;

/// The air BEFORE the eject glyph, so it does not touch the rule.
///
/// Same defect, other edge. Without it the glyph sits in the last cell of the
/// sidebar with the rule painted immediately after it, and the two read as one
/// smudge rather than as a target and a boundary.
const SIDE_EJECT: u16 = ronoctua_tokens::space::FILES_SIDEBAREJECT;

/// How many cells a sidebar row wants: the glyph, its air, the name, and the
/// eject glyph when there is one.
///
/// The name is measured by CLUSTER (`layout::width`) and never by byte —
/// `MJRO1812` and `Área de trabalho` are eight and sixteen cells, not eight and
/// eighteen bytes.
fn side_row_width(row: &ronoctua_files::sidebar::Row) -> u16 {
    // The glyph plus one cell of air, which is the same two cells
    // `Column::Icon` costs in the listing and for the same reason.
    2 + width_of(row.label.as_deref().unwrap_or_default()) + eject_room(row)
}

/// How many cells the sidebar takes — and ZERO when it withdraws.
///
/// Two phases, the same shape as [`layout_beside`] and as `layout::resolve` on
/// the modeline.
///
/// **Phase one is a PREFERENCE.** Below `space.files.sidebarFrom` it is not
/// offered at all: at that width the listing would be nothing but names.
///
/// **Phase two is what does not give way** — nothing leaks off the screen. A
/// quarter of the width, floored at [`SIDE_FLOOR`], and if what is left cannot
/// hold a listing at its own floor the sidebar withdraws ENTIRELY rather than
/// shrinking further. Half a sidebar beside half a listing is two things
/// neither of which can be read.
#[must_use]
pub fn sidebar_columns(width: u16, showing: bool, content: u16) -> u16 {
    if !showing || width < ronoctua_tokens::space::FILES_SIDEBARFROM {
        return 0;
    }
    // THE CONTENT DECIDES, and not a fraction of the screen.
    //
    // It used to be `width / 4`, which on a 245-column screen gave sixty-one
    // cells to hold the word `screenshot`. A sidebar is a list of short names;
    // it should be as wide as its longest one and not one cell more, because
    // every cell it takes is a cell the listing gives up — and the listing is
    // what one opened the manager for.
    //
    // The ceiling is not taste either: a stick can carry a forty-character
    // label, and without it one badly named pendrive would swallow the table.
    let ceiling = (width / 3).max(SIDE_FLOOR);
    let wanted = (SIDE_GUTTER + content + EDGE_RIGHT).clamp(SIDE_FLOOR, ceiling);
    // What a listing needs before it says anything: the gutter, the icon and
    // its gap, the name's floor, and the right edge.
    let listing_floor = GUTTER + Column::Icon.width() + 1 + NAME_FLOOR + EDGE_RIGHT;
    if width.saturating_sub(wanted + RULE) < listing_floor {
        return 0;
    }
    wanted
}

/// Which columns fit, and where.
///
/// Integer arithmetic in two phases, the same shape as `layout::resolve`:
/// everything is asked for, then the lowest-priority column gives way until
/// what is left fits. What never gives way is "nothing leaks off the screen" —
/// the compositor does not clip.
///
/// `from` is where the sidebar ends, and zero when there is none. The columns
/// are resolved against `width - from` — the listing's OWN width — and only
/// then shifted: resolving against the whole width and shifting afterwards
/// would let the last column fall off the right edge, and "nothing leaks" is
/// the one rule that never gives way. The compositor does not clip.
///
/// There is no `layout_beside(width, 0)` beside it any more. It was this function
/// with `from` at zero, and once the header and the rows both had to ask about
/// the sidebar it had no caller left outside the tests — a second name for one
/// arithmetic is a second place to read.
#[must_use]
pub fn layout_beside(width: u16, from: u16) -> Layout {
    let width = width.saturating_sub(from);
    let mut wanted = vec![
        Column::Icon,
        Column::Name,
        Column::Perms,
        Column::Owner,
        Column::Group,
        Column::Size,
        Column::Modified,
    ];
    // One cell of separation between columns, which is the whole of the
    // separation: no rule, no box (invariant 15).
    let gap = 1u16;
    let fixed = |cols: &[Column]| -> u16 {
        cols.iter()
            .filter(|c| !matches!(c, Column::Name))
            .map(|c| c.width() + gap)
            .sum()
    };
    // BOTH EDGES COME OFF BEFORE ANYTHING IS DECIDED. They are not columns and
    // they never give way: a table whose last column is flush against the
    // screen reads as cut off, whatever else fits.
    let usable = |w: u16| w.saturating_sub(GUTTER).saturating_sub(EDGE_RIGHT);
    for giving in GIVE_WAY {
        if usable(width).saturating_sub(fixed(&wanted)) >= NAME_FLOOR {
            break;
        }
        wanted.retain(|c| *c != giving);
    }
    // PHASE TWO: the floor gives way, and the name takes whatever is left.
    //
    // `NAME_FLOOR` is a PREFERENCE — twelve cells is where a name stops
    // identifying anything — and a preference is not what holds when the screen
    // is smaller than it. What holds is that nothing leaks, which is the same
    // two-phase shape `layout::resolve` uses on the modeline.
    // The name takes what is left, and at least one cell: a listing with no
    // name column at all would be a table about files that does not say which.
    let for_name = usable(width).saturating_sub(fixed(&wanted)).max(1);

    // Drawn left to right: icon, name, then the fixed block. The name comes
    // early because it is what the eye scans; the facts one looks UP sit in a
    // column that always starts at the same cell, which is what makes them
    // comparable between rows.
    let mut columns = Vec::new();
    // THE FIRST CELL IS NOT A COLUMN. It is where the mark goes, and it is the
    // air between the icon and the screen's edge — without it the glyph touches
    // the border and reads as clipped. It is the same `modeline.edgeLeft` the
    // rest of the footer already leaves.
    let mut at = GUTTER;
    for c in [
        Column::Icon,
        Column::Name,
        Column::Perms,
        Column::Owner,
        Column::Group,
        Column::Size,
        Column::Modified,
    ] {
        if !wanted.contains(&c) {
            continue;
        }
        let w = if matches!(c, Column::Name) {
            for_name
        } else {
            c.width()
        };
        columns.push((c, from + at, w));
        at += w + gap;
    }
    Layout { columns, from }
}

/// What a click on a cell means.
///
/// Every variant is something a key also reaches — that is the invariant this
/// enum exists to make checkable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hit {
    /// The name: open it, as `Enter` does.
    Name(usize),
    /// The icon: mark it, as `m` does.
    Mark(usize),
    /// One permission cell: flip that bit, as the menu's `m` reaches.
    Bit(usize, mode::Who, mode::What),
    /// A special bit's cell.
    Special(usize, mode::Special),
    /// The owner: pick another, as the menu's `O` does.
    Owner(usize),
    /// The group: pick another, as the menu's `G` does.
    Group(usize),
    /// The size of a directory: measure it, as `TAB` on a directory does.
    Measure(usize),
    /// A piece of the path above: go there, as `-` does one level at a time.
    Crumb(usize),
    /// A column's header: sort by it, as `s` cycles.
    Sort(Column),
    /// A row of the open menu.
    Menu(&'static str),
    /// A row of the SIDEBAR's open menu.
    ///
    /// Its own variant and not `Menu` with a flag: the two tables have separate
    /// ids and separate keys, and one variant carrying "which table" is a place
    /// for the wrong table to be consulted with no error.
    SideMenu(&'static str),
    /// A choice of the open picker.
    Choice(usize),
    /// A row of the sidebar: go there, open it, run it — or mount it, when it
    /// is a device that is not mounted. As `Enter` does on a listing row, which
    /// is why it costs no key of its own.
    Sidebar(usize),
    /// A mounted device's eject glyph, in the sidebar's last cell. As `C-e`.
    ///
    /// TAKING A ROW OFF THE SIDEBAR HAS NO TARGET HERE, and that is a decision:
    /// a little cross on every row is chrome on a surface that has none
    /// (invariant 14), and `C-u` reaches it. The rule is one-directional —
    /// everything the mouse can do the keyboard must do, never the reverse —
    /// which is why `files-mark-all` has no target either.
    ///
    /// A DRAWN target and not a click on the row, for the reason the
    /// notification manager already gives about its bin: a click on a row says
    /// nothing about which of two things it would do, and a click on an eject
    /// glyph is not a guess.
    Eject(usize),
    /// Nothing happens here.
    Nothing,
}

/// Which of the two lists has the keyboard.
///
/// The same word the notification manager uses for the same idea, and
/// deliberately: two names for "which side am I on" is two things to learn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    /// The directory. Where the cursor starts, and where typing always goes.
    #[default]
    Listing,
    /// The sidebar.
    Sidebar,
}

/// One drawn piece of a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// First cell.
    pub at: u16,
    /// What is drawn.
    pub text: String,
    /// Which token colours it.
    pub color: Color,
    /// What a click does.
    pub hit: Hit,
    /// What the pointer resting here says on the message line.
    pub explain: Option<Explanation>,
}

/// One line of the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The pieces, left to right.
    pub fields: Vec<Field>,
    /// Whether this entry is marked for a batch operation.
    pub marked: bool,
    /// Whether the cursor is on it.
    pub focused: bool,
}

impl Field {
    /// Does this field own that cell?
    ///
    /// **THE ONE RULER**, and there were three: `Row::field_at` counted the gap
    /// after a field as belonging to it (`+ 1`), while both painters' "is the
    /// pointer on this field" test did not. So the last cell of a field lit
    /// nothing and yet clicked something — one cell wide, on every field, in
    /// both doors.
    ///
    /// The `+ 1` is the right answer and it is the one kept: a one-cell dead
    /// zone between columns is invisible and feels like a broken click.
    #[must_use]
    pub fn covers(&self, cell: u16) -> bool {
        cell >= self.at && cell < self.at + width_of(&self.text) + 1
    }
}

impl Row {
    /// Which field a cell belongs to.
    ///
    /// Walks from the right, so a click in the gap after a field belongs to
    /// that field rather than to nothing: a one-cell dead zone between columns
    /// is invisible and feels like a broken click.
    #[must_use]
    pub fn field_at(&self, cell: u16) -> Option<&Field> {
        self.fields.iter().rev().find(|f| f.covers(cell))
    }
}

/// How many cells the eject glyph needs on a sidebar row — two, or none.
///
/// Only a MOUNTED device gets one. An unmounted one has nothing to unmount, and
/// a folder is not something one ejects: a glyph offered where it does nothing
/// is the shape `menu::Applies` exists to refuse.
fn eject_room(row: &ronoctua_files::sidebar::Row) -> u16 {
    // AND NOT ON WHAT THE SYSTEM NEEDS. `menu::Applies` states the rule this
    // follows: a row that is offered and then refuses is worse than one that
    // is not offered, because the person spent a gesture to be told no and
    // nothing says which part of the state made the difference. Unmounting `/`
    // is not a thing to confirm — it is a thing not to draw a target for.
    if row.mounted == Some(true) && row.needed.is_none() {
        2
    } else {
        0
    }
}

/// Text width in CELLS, by cluster.
fn width_of(s: &str) -> u16 {
    ronoctua_render::layout::width(s)
}

/// Something the host has to carry out — a process, a thread, or the disk.
///
/// The view decides WHAT; it never spawns, never blocks and never writes. It is
/// the same split the memory panel uses, and the reason is the trap this
/// project already paid for: `systemd-run --scope` is synchronous, and waiting
/// inside the loop that draws freezes the footer for the life of the child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doing {
    /// Hand it to whatever opens this type.
    Open(PathBuf),
    /// Open it with the application of this id.
    OpenWith(PathBuf, String),
    /// A terminal in this directory.
    Terminal(PathBuf),
    /// A root terminal in this directory, through polkit.
    RootTerminal(PathBuf),
    /// Put this text on the clipboard.
    Copy(String),
    /// Move these to the trash, in the bin at this root.
    Trash(PathBuf, Vec<PathBuf>),
    /// Put these back where they came from, by their name in the trash.
    ///
    /// The bin's root travels along: the door that carries this out has no
    /// business asking the environment where the trash is a second time and
    /// possibly getting a different answer.
    Restore(PathBuf, Vec<String>),
    /// Remove these outright.
    Delete(Vec<PathBuf>),
    /// Overwrite and remove these.
    Shred(Vec<PathBuf>),
    /// Copy or move what was cut or copied into this directory.
    Paste(PathBuf),
    /// Pack these into an archive.
    Compress(Vec<PathBuf>, &'static str),
    /// Unpack this one here.
    Extract(PathBuf),
    /// Set the permission bits, on every one of them.
    Mode(Vec<PathBuf>, u32),
    /// Set the group, on every one of them.
    Group(Vec<PathBuf>, u32),
    /// Set the owner — the one that needs the privileged unit.
    Owner(Vec<PathBuf>, u32),
    /// Walk a directory and report how big it is.
    Measure(PathBuf),
    /// Mount this volume, by the id the disk border gave it, and go in when it
    /// comes up.
    Mount(String),
    /// Unmount it.
    ///
    /// The ID and not the mount point: the point is the thing that disappears,
    /// and a request naming it would name what is no longer there.
    Unmount(String),
    /// Run this application, by its freedesktop id.
    Launch(String),
    /// Unmount this volume and then cut the power to its drive.
    ///
    /// TWO STEPS AND IN THIS ORDER, and the host does the second only when the
    /// first came back: ejecting with a write still in flight is how a stick
    /// leaves with half a file on it. It is the same reason `Unmount` never
    /// passes `force`.
    Eject(String),
    /// Hand this volume to the formatting buffer.
    ///
    /// The BUFFER asks the questions and the HOST does the work — and the host
    /// re-checks that the volume is one the running system does not need, so
    /// the guard holds even if the buffer names something else.
    Format(String),
}

/// A question the view needs answered before it can act.
///
/// Every one of them is a LIST to choose from, never a field to type into —
/// which is what the owner asked for: changing a permission or an owner should
/// not require knowing the syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asking {
    /// Which application, out of those that declare this type.
    Application(PathBuf),
    /// Which owner, for all of these.
    Owner(Vec<PathBuf>),
    /// Which group, for all of these.
    Group(Vec<PathBuf>),
    /// Which permission preset, for all of these — or a typed octal for
    /// whoever knows one.
    ///
    /// # Why these three carry a LIST
    ///
    /// Because marks exist. Having marked six files, "change the permission"
    /// means all six; acting on one of them and saying nothing is a silent
    /// partial action, which is worse than refusing. It read the FIRST marked
    /// path and the FOCUSED entry's current mode — two different files — until
    /// this was written out.
    Mode(Vec<PathBuf>, u32),
    /// Which archive format.
    Format(Vec<PathBuf>),
    /// The new name. The one place typing is unavoidable, because the answer
    /// is not on any list.
    Name(PathBuf),
    /// Which hidden sidebar row to bring back.
    ///
    /// It carries nothing: the choices ARE the answer, and the row's id is what
    /// comes back. Every other question here names what it is about because the
    /// row that opened it has left the screen; this one is about the sidebar,
    /// which is still there.
    Sidebar,
    /// "Are you sure?", holding what will happen if the answer is yes.
    ///
    /// # Why a question and not a second key
    ///
    /// The menu already arms its irreversible rows — press the key twice — and
    /// that works there because the row is on screen saying what it is. A
    /// permission change is not that: by the time it applies, the answer came
    /// from a LIST OF PRESETS, and what the list said was "leitura e escrita
    /// para o dono", not `0644` on six files. The reported gesture is a person
    /// picking from a list and the mode changing before they had read it.
    ///
    /// So it is a question, in the same picker as every other one: same keys,
    /// same narrowing, same `Esc`. What it holds is the finished [`Doing`],
    /// which is what makes it reusable — nothing about it is about modes.
    ///
    /// AND THE SENTENCE, already resolved. "Tem certeza?" was the first text,
    /// and it was reported as what it is: vague. A confirmation that does not
    /// name what will happen asks the reader to remember, and the whole reason
    /// this exists is that by then they are two screens away from the row they
    /// clicked. So it says the mode and it says on what.
    Confirm(Box<Doing>, String),
}

/// What a key or a click produced.
///
/// There is no `Ask`: a question the view needs answered is answered INSIDE
/// it, by [`Picker`]. It used to leave through here and be handled by the
/// footer's locator, and that was one implementation too many — the window has
/// no locator, so the same gesture would have needed a second one, and two
/// pickers is how "change the owner" comes to behave differently depending on
/// which door you opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nothing happened; do not redraw.
    Nothing,
    /// The state changed; redraw.
    Changed,
    /// The view closed.
    Closed,
    /// Carry this out.
    Do(Doing),
    /// Put this on the message line.
    Say(Explanation),
}

/// One row of the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// What the answer is: a uid, a gid, an octal, an extension.
    pub id: String,
    /// What is shown.
    pub label: String,
}

/// A question, on screen, with its answers.
///
/// Typing narrows it by the SAME subsequence match the locator uses
/// (`minibuffer::match_up`), so the gesture is the one this system already
/// teaches everywhere else. What it does not share is the surface: it draws in
/// the manager's own region, which is what lets the window have it too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picker {
    /// What is being asked.
    pub asking: Asking,
    /// Everything on offer.
    pub choices: Vec<Choice>,
    /// What has been typed.
    pub query: String,
    /// Which of the surviving choices is focused.
    pub cursor: usize,
    /// Whether the TYPED text is the answer, rather than a choice.
    ///
    /// True for a rename, which is the one question whose answer is not on any
    /// list. Everything else is a list, which is what the owner asked for:
    /// changing a permission or an owner should not require knowing the syntax.
    pub typed: bool,
}

impl Asking {
    /// The text key that says what is being asked.
    ///
    /// On `Asking` and not in the drawing, because it is a property of the
    /// QUESTION: a second `match` beside the one that builds the choices is a
    /// second place to forget a variant, and this one the compiler would not
    /// catch — it would just draw the wrong sentence.
    #[must_use]
    pub fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::Application(_) => texts.t(ronoctua_i18n::key::FILES_ASK_APPLICATION),
            Self::Owner(_) => texts.t(ronoctua_i18n::key::FILES_ASK_OWNER),
            Self::Group(_) => texts.t(ronoctua_i18n::key::FILES_ASK_GROUP),
            Self::Mode(_, _) => texts.t(ronoctua_i18n::key::FILES_ASK_MODE),
            Self::Format(_) => texts.t(ronoctua_i18n::key::FILES_ASK_FORMAT),
            Self::Name(_) => texts.t(ronoctua_i18n::key::FILES_ASK_NAME),
            Self::Sidebar => texts.t(ronoctua_i18n::key::FILES_SIDEBAR_HIDDEN_ASK),
            // ALREADY A SENTENCE, built where the `Doing` was known.
            Self::Confirm(_, sentence) => sentence.clone(),
        }
    }
}

impl Picker {
    /// The choices the query leaves, as indices.
    #[must_use]
    pub fn shown(&self) -> Vec<usize> {
        self.choices
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                self.query.is_empty()
                    || crate::minibuffer::match_up(&self.query, &c.label).is_some()
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// What confirming answers.
    #[must_use]
    pub fn answer(&self) -> Option<String> {
        if self.typed {
            return (!self.query.is_empty()).then(|| self.query.clone());
        }
        let shown = self.shown();
        shown
            .get(self.cursor)
            .and_then(|i| self.choices.get(*i))
            .map(|c| c.id.clone())
    }
}

/// What a background thread came back with.
///
/// Three cases and no progress: an operation on a handful of files finishes
/// before a progress bar would be legible, and one on ten thousand needs
/// cancellation rather than a number. The walk that DOES take minutes reports
/// its result, and `ronoctua_memory::scan` is where a cancellable version with
/// progress already lives if the day comes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// It worked; read the directory again.
    Done,
    /// How big that directory turned out to be.
    Measured(String, u64),
    /// It did not work, and this is what to say.
    Failed(String),
    /// It mounted, and this is where. The path comes back because going in is
    /// what the person meant by pressing the row.
    Mounted(PathBuf),
    /// udisks said no, and this is WHICH no — plus its own words, for the one
    /// case that has nothing better to say.
    Refused(ronoctua_disks::Refusal, String),
    /// A disk was plugged in, unplugged, mounted or unmounted — and this is the
    /// machine as it is now.
    ///
    /// The whole reading and not a nudge: plugging in ONE stick emits four
    /// signals in a few milliseconds, and a nudge would make the loop that
    /// draws do four round trips of its own. Reading on the watching thread
    /// means the bus is never touched while a frame is being painted, and the
    /// last message wins.
    ///
    /// `Box`ed because it is by far the largest variant and a `Report` is
    /// cloned per message.
    Racked(Box<ronoctua_disks::Rack>),
}

/// What is on the clipboard, and whether it moves or copies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Clip {
    /// The paths.
    pub paths: Vec<PathBuf>,
    /// `true` when they were CUT: pasting removes the originals.
    pub moves: bool,
}

/// The manager's whole state.
#[derive(Debug)]
pub struct View {
    /// The language bundle, SHARED with the door that opened this view.
    ///
    /// # Why the view holds one now
    ///
    /// It used not to, on purpose: the drawing received the bundle and the view
    /// stayed free of it. That worked while every label on screen came from the
    /// listing — but a QUESTION's choices are built here, and some of their
    /// labels are text keys (`files-preset-file`, `files-yes`). Written into
    /// `Choice.label` and drawn verbatim, the raw key went to the screen — which
    /// is exactly the shape invariant 8 exists to refuse, reported as
    /// *"as respostas são `files-no` e `files-yes`"*.
    ///
    /// It was not only the drawing: `Picker::shown` FILTERS on the label, so
    /// typing narrowed a list of keys and not of sentences.
    ///
    /// An `Arc`, because `Texts` is a Fluent bundle and does not clone, and
    /// because both doors already own one and there is no reason for a second.
    texts: std::rc::Rc<ronoctua_i18n::Texts>,
    where_: PathBuf,
    entries: Vec<Entry>,
    /// The type of each entry, by NAME only — no file is opened to draw a list.
    types: Vec<&'static str>,
    /// Which rows survive the filter, as indices into `entries`.
    shown: Vec<usize>,
    cursor: usize,
    /// Marked entries, BY NAME. An index would move under the marks the moment
    /// the directory is re-read or the sort changes.
    marks: BTreeSet<String>,
    /// What was typed. TYPING ALWAYS GOES HERE — there is no mode.
    ///
    /// It used to be a mode entered with `/`, guarded by a `filtering` flag,
    /// and the reasoning was sound as far as it went: a key that is sometimes
    /// text and sometimes a command is a key nobody can use for either. What it
    /// got wrong was WHICH of the two a letter should be. In a list of file
    /// NAMES the letter is text — that is what the locator and the `M-x` do,
    /// and it is what the owner asked for. So the letters stopped being
    /// commands and the commands moved behind modifiers; the ambiguity is gone
    /// by the same argument, resolved the other way.
    filter: String,
    sort: Sort,
    hidden: bool,
    /// The action menu, when it is open, and which of its rows is focused.
    menu: Option<usize>,
    /// An action waiting for its key a second time.
    arming: Option<&'static str>,
    /// The question on screen, when there is one.
    picking: Option<Picker>,
    /// The entry whose detail sheet is open, by NAME.
    ///
    /// By name for the same reason the marks are: a re-read moves every index,
    /// and a sheet that then describes a different file is worse than no sheet.
    detail: Option<String>,
    /// Every application the machine offers, for `open with`.
    ///
    /// An `Arc` of the SAME catalogue the launcher swept at startup: no
    /// directory is walked here, and answering "who opens this" costs a filter
    /// over memory.
    catalogue: crate::apps::Catalogue,
    /// The measured size of a directory, once somebody asked.
    measured: Option<(String, u64)>,
    clip: Clip,
    roster: Roster,
    zone: Zone,
    /// Where the trash is.
    ///
    /// A PARAMETER, like the roster and the zone, for the reason
    /// `ronoctua-files` gives everywhere: every root is one, so a fake tree in
    /// a test is worth more than an environment variable two tests race on —
    /// and the one that loses puts a file in the other's trash. Read once;
    /// `$XDG_DATA_HOME` does not move while a listing is up.
    bin: Option<ronoctua_files::trash::Bin>,
    /// The directory's mtime when it was read, for "has it changed".
    stamp: Option<std::time::SystemTime>,
    /// Where one starts from: the places, the devices and the trash.
    ///
    /// It is `ronoctua_files::sidebar`'s, and a PARAMETER like the roster, the
    /// zone and the bin — the whole crate is built that way. What it does not
    /// know is devices: those come from the disk border, and the host splices
    /// them in with `devices_are`. See that module for why the two are not one
    /// crate.
    side: ronoctua_files::sidebar::Sidebar,
    /// Which of the two lists has the keyboard.
    pane: Pane,
    /// The cursor INSIDE the sidebar, so crossing over and back lands where it
    /// left rather than at the top.
    side_cursor: usize,
    /// When the sidebar's file was last read, for "has somebody changed it".
    side_stamp: Option<std::time::SystemTime>,
}

/// An action's technical answer, as an [`Explanation`] with nothing to fill in.
///
/// A function and not an inline literal because it is written at five sites,
/// and the empty `args` is the half that would drift: an `Explanation` whose
/// key expects an argument and whose `args` is empty draws the placeholder in
/// front of somebody, which is the shape invariant 8 exists to refuse.
fn does(key: &'static str) -> mode::Explanation {
    mode::Explanation {
        key,
        args: Vec::new(),
    }
}

impl View {
    /// Opens a directory.
    #[must_use]
    pub fn at(
        where_: &Path,
        roster: Roster,
        zone: Zone,
        bin: Option<ronoctua_files::trash::Bin>,
        catalogue: crate::apps::Catalogue,
        texts: std::rc::Rc<ronoctua_i18n::Texts>,
        side: ronoctua_files::sidebar::Sidebar,
    ) -> Self {
        let mut view = Self {
            texts,
            pane: Pane::Listing,
            side_cursor: 0,
            side_stamp: side.stamp(),
            side,
            where_: where_.to_path_buf(),
            entries: Vec::new(),
            types: Vec::new(),
            shown: Vec::new(),
            cursor: 0,
            marks: BTreeSet::new(),
            filter: String::new(),
            sort: Sort::default(),
            // The owner's rule, and Unix's own accident refused: the whole
            // directory is shown, and `.` hides the dotfiles for whoever wants
            // that. A manager that hides half of `$HOME` cannot be used to fix
            // a dotfile, which is most of why one opens it here.
            hidden: true,
            menu: None,
            arming: None,
            picking: None,
            detail: None,
            catalogue,
            measured: None,
            clip: Clip::default(),
            roster,
            zone,
            bin,
            stamp: None,
        };
        view.reread();
        view
    }

    /// Where it is looking.
    #[must_use]
    pub fn where_(&self) -> &Path {
        &self.where_
    }

    /// The entry under the cursor.
    #[must_use]
    pub fn focused(&self) -> Option<&Entry> {
        self.shown
            .get(self.cursor)
            .and_then(|i| self.entries.get(*i))
    }

    /// Its full path.
    #[must_use]
    pub fn focused_path(&self) -> Option<PathBuf> {
        self.focused().map(|e| self.where_.join(&e.name))
    }

    /// How many rows the filter left.
    #[must_use]
    pub fn amount(&self) -> usize {
        self.shown.len()
    }

    /// Where the cursor is, among the shown rows.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether the action menu is open.
    #[must_use]
    pub const fn menu_open(&self) -> bool {
        self.menu.is_some()
    }

    /// Whether the detail sheet or a question is on screen.
    ///
    /// Both replace the listing, so the header — which names the listing's
    /// columns — must not be drawn over them.
    #[must_use]
    pub const fn showing_detail(&self) -> bool {
        self.detail.is_some() || self.picking.is_some()
    }

    /// What is typed into the filter.
    ///
    /// `#[cfg(test)]` because nothing on screen reads it any more: the query is
    /// drawn as the last field of the path (`crumbs`), so the drawing gets it
    /// from there. What still needs the raw string is the tests, which assert
    /// on what was typed rather than on how it came out.
    #[cfg(test)]
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Whether there is a query to show.
    ///
    /// The field appears with the first letter and goes away with the last: it
    /// is not a mode any more, so there is no state to report other than
    /// whether anything was typed.
    #[must_use]
    pub fn filtering(&self) -> bool {
        !self.filter.is_empty()
    }

    /// Reads the directory again, keeping the cursor on the same NAME.
    ///
    /// By name and not by index: a re-read after a rename, a delete or a change
    /// of sort moves every index, and a cursor that lands somewhere else is how
    /// a person deletes the wrong file.
    pub fn reread(&mut self) {
        let was = self.focused().map(|e| e.name.clone());
        self.stamp = listing::stamp(&self.where_);
        self.entries = listing::read_dir_at(&self.where_, self.sort).unwrap_or_default();
        self.types = self
            .entries
            .iter()
            .map(|e| mime::of_path(&self.where_.join(&e.name), e.kind, false))
            .collect();
        // A mark on a name that is gone goes with it, rather than quietly
        // applying to whatever takes the name later.
        let present: BTreeSet<String> = self.entries.iter().map(|e| e.name.clone()).collect();
        self.marks.retain(|m| present.contains(m));
        self.refilter();
        if let Some(name) = was
            && let Some(at) = self
                .shown
                .iter()
                .position(|i| self.entries[*i].name == name)
        {
            self.cursor = at;
        }
        self.clamp();
    }

    /// Whether the directory changed since it was read.
    #[must_use]
    pub fn stale(&self) -> bool {
        listing::stamp(&self.where_) != self.stamp
    }

    /// Goes into a directory.
    pub fn enter(&mut self, into: &Path) {
        self.where_ = into.to_path_buf();
        self.cursor = 0;
        self.filter.clear();
        self.marks.clear();
        self.measured = None;
        self.detail = None;
        self.menu = None;
        self.reread();
    }

    /// Goes up one level, landing the cursor on the directory just left.
    ///
    /// Landing on it and not at the top is what makes `-` and `Enter` inverse
    /// gestures: without it, going up and changing one's mind costs a hunt
    /// through the parent.
    pub fn up(&mut self) {
        let Some(parent) = self.where_.parent().map(Path::to_path_buf) else {
            return;
        };
        let came_from = self
            .where_
            .file_name()
            .map(|n| n.to_string_lossy().into_owned());
        self.enter(&parent);
        if let Some(name) = came_from
            && let Some(at) = self
                .shown
                .iter()
                .position(|i| self.entries[*i].name == name)
        {
            self.cursor = at;
        }
    }

    /// The header, naming the columns and saying which one sorts.
    ///
    /// # Why it costs a row of a region that only has twelve
    ///
    /// Because without it the columns are unlabelled: `rwxr-xr-x  ana  users`
    /// is legible to whoever already knows, and a table whose columns are only
    /// legible to whoever knows is a table for nobody else. It also carries the
    /// one piece of state a listing cannot show any other way — WHICH column
    /// is sorting, and in which direction.
    ///
    /// And it is where the sort is CLICKED, which is the gesture every table in
    /// the world has. The key `s` cycles the same thing; neither reaches
    /// anything the other does not.
    #[must_use]
    pub fn header(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Row {
        // BESIDE THE SIDEBAR, like the rows underneath it. A layout that
        // ignored `from` put the column names at cells the listing no longer
        // occupies — over the sidebar, naming columns that were somewhere else
        // — and clicking one would have sorted by whichever column happened to
        // be under the word. It is the same `split` the rows are built with,
        // and it has to be, or the header names a different table.
        let split = self.split(width);
        let layout = layout_beside(width, if split == 0 { 0 } else { split + RULE });
        let fields = layout
            .columns
            .iter()
            .filter(|(c, _, _)| !matches!(c, Column::Icon))
            .map(|(c, at, w)| {
                let sorting = self.sort.by == c.sorts_by();
                // The ARROW says the direction, and only on the column that is
                // actually sorting. On all of them it would be four arrows
                // claiming four orders.
                let mark = if sorting {
                    if self.sort.ascending { "↑" } else { "↓" }
                } else {
                    ""
                };
                let label = texts.t(c.label());
                Field {
                    at: *at,
                    text: ronoctua_render::layout::elide_end(&format!("{label}{mark}"), *w),
                    color: if sorting {
                        ronoctua_tokens::color::STATE_ACCENT
                    } else {
                        ronoctua_tokens::color::TEXT_DIM
                    },
                    hit: Hit::Sort(*c),
                    explain: None,
                }
            })
            .collect();
        Row {
            fields,
            marked: false,
            focused: false,
        }
    }

    /// The rows, for a region of this width.
    ///
    /// It takes the bundle because a row carries TEXT, and some of that text is
    /// a key: a menu row's label, a permission preset's name. Resolving it here
    /// keeps `Field::text` a finished string, which is what both doors draw —
    /// and it is why the menu once showed `files-open` instead of "abrir".
    #[must_use]
    pub fn rows(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        self.rows_from(0, usize::MAX, width, texts)
    }

    /// Moves the cursor to a row, and NOTHING else.
    ///
    /// # Why this is not `click(row, &Hit::Name(row))`
    ///
    /// Because that one ACTIVATES: it focuses the row and then opens it. The
    /// right button used it to "focus first", and the result was reported from
    /// the screen — right-clicking an EMPTY directory entered it and showed no
    /// menu (there was nothing left to hang one on), and right-clicking a full
    /// one showed a menu over the contents of the directory it had just walked
    /// into.
    ///
    /// The code even said so, in a comment explaining how to detect the case it
    /// had itself created: *"the click above may have ENTERED a directory"*. A
    /// gesture that has to be undone afterwards was the wrong gesture.
    pub fn focus_row(&mut self, row: usize) -> Step {
        if row >= self.shown.len() {
            return Step::Nothing;
        }
        self.cursor = row;
        Step::Changed
    }

    /// Whether the listing has any row at all.
    ///
    /// The right button asks before deciding what to focus: with no rows there
    /// is nothing to focus, and the menu is about the directory instead. See
    /// `menu_subject`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    /// The same, but only the slice that is going to be DRAWN.
    ///
    /// # Why the window exists
    ///
    /// Because building a row is not free — a mode's nine cells, a size, a
    /// date, an owner and a group, each one a finished string — and the region
    /// draws twelve of them. Building the whole directory to throw away all but
    /// twelve is work paid on every frame, and a frame happens on every pointer
    /// movement that changes the row under it.
    ///
    /// MEASURED, in release, on this machine:
    ///
    /// | entries | whole list | window of 12 |
    /// |--------:|-----------:|-------------:|
    /// |     200 |    0.55 ms |      0.03 ms |
    /// |   2 000 |    6.37 ms |      0.03 ms |
    /// |  10 000 |   53.31 ms |      0.03 ms |
    ///
    /// And the frame did it several times over, because the height, the page
    /// count and the offset each asked for the list again. At ten thousand
    /// entries that is the hover visibly trailing the pointer — which is what
    /// the owner reported, and it is not the compositor: it is arithmetic, and
    /// it would have followed the system onto real hardware.
    ///
    /// The rows that are NOT a listing — the picker, the detail sheet, the menu
    /// — are built whole and then cut, because they are a dozen lines by
    /// construction. They are still CUT: the window has one meaning for every
    /// caller, and a branch that quietly returned more than was asked for would
    /// draw a menu over the rows below it.
    #[must_use]
    pub fn rows_from(
        &self,
        skip: usize,
        take: usize,
        width: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let cut = |v: Vec<Row>| v.into_iter().skip(skip).take(take).collect();
        if let Some(picker) = &self.picking {
            return cut(picker_rows(picker, width, texts));
        }
        if let Some(name) = &self.detail {
            return cut(self.detail_rows(name, width, texts));
        }
        if let Some(chosen) = self.menu {
            return cut(self.menu_rows(chosen, width, texts));
        }
        // THE SIDEBAR IS NOT A SECOND LIST OF LINES: it is extra `Field`s on
        // the SAME `Row`. Line `n` carries sidebar row `n` in the cells before
        // the split and listing row `n` after it, which is what leaves the
        // scrolling, the paging, the hit testing and both doors' painters
        // untouched — `Row::field_at` walks from the right, so a cell below the
        // split finds a sidebar field and one above it finds a listing field,
        // with no boundary arithmetic anywhere.
        //
        // Neither the picker, the detail sheet nor the menu gets one: each is
        // modal, and covering a question half-way is worse than covering it.
        let split = self.split(width);
        let layout = layout_beside(width, if split == 0 { 0 } else { split + RULE });
        let side = self.side.shown(self.hidden);
        // AT LEAST ONE LINE, because "nothing here" is a sentence and a
        // sentence needs somewhere to be. It is the same `.max(1)` as
        // `row_count`, and the two have to agree or the region asks for a
        // height it then does not fill.
        let lines = self
            .shown
            .len()
            .max(if split == 0 { 0 } else { side.len() })
            .max(1);
        (skip..lines.min(skip.saturating_add(take)))
            .map(|n| {
                // `n` STAYS ABSOLUTE. It is what says which row has the cursor
                // and which is marked, and a window that renumbered from zero
                // would put the focus on the first visible line whatever the
                // offset.
                let mut row = self.shown.get(n).map_or_else(
                    || Row {
                        fields: Vec::new(),
                        marked: false,
                        focused: false,
                    },
                    |i| self.row_of(n, *i, &layout),
                );
                // "NOTHING HERE" IS A FIELD OF THE LISTING, on its first line.
                //
                // It used to be a row the FOOTER built, which had two
                // consequences: the window door drew nothing at all for an
                // empty directory and said nothing either, and with a sidebar
                // beside it the sentence would have disappeared altogether,
                // because the region is no longer empty. Here it is beside the
                // sidebar, where it belongs, and both doors get it.
                if n == 0 && self.shown.is_empty() {
                    row.fields.push(Field {
                        at: layout.from + GUTTER,
                        text: texts.t(ronoctua_i18n::key::FILES_NOTHING_HERE),
                        color: ronoctua_tokens::color::TEXT_DIM,
                        hit: Hit::Nothing,
                        explain: None,
                    });
                }
                // A GAP DRAWS NOTHING. It is what separates the blocks, and
                // an empty line is exactly what it should look like.
                if split > 0
                    && let Some(Some(place)) = side.get(n)
                {
                    let mut fields = self.side_fields(n, place, split, texts);
                    fields.append(&mut row.fields);
                    row.fields = fields;
                }
                row
            })
            .collect()
    }

    /// How many rows a listing has, WITHOUT building one.
    ///
    /// The count is what the height, the page count and the scrollbar need, and
    /// they needed it several times per frame. See [`View::rows_from`].
    #[must_use]
    pub fn row_count(&self, width: u16) -> Option<usize> {
        // `None` for what is NOT a listing — the picker, the detail sheet, the
        // menu. They are a dozen rows by construction, so counting them means
        // building them, and the caller may as well. An `Option` and not a
        // sentinel: a `usize::MAX` meaning "ask again" is the kind of number
        // that ends up in arithmetic.
        (self.picking.is_none() && self.detail.is_none() && self.menu.is_none()).then(|| {
            // THE TALLER OF THE TWO, and at least one line. A directory of two
            // files beside a sidebar of six is six lines, not two — and an
            // EMPTY directory is still one line, because "nothing here" is a
            // sentence that has to be somewhere. That `.max(1)` used to live in
            // the footer's `expansion_len`, where the window door could not
            // reach it: an empty directory drew nothing at all over there and
            // said nothing either.
            // THE WIDTH DECIDES, and that is why it is a parameter now. The
            // sidebar withdraws on a narrow screen, and a count that assumed it
            // was there would ask the region for lines nothing draws into —
            // blank space below a two-line directory, which reads as the
            // manager having lost the rest of it.
            let side = if self.split(width) == 0 {
                0
            } else {
                self.side.shown(self.hidden).len()
            };
            self.shown.len().max(side).max(1)
        })
    }

    /// Which pane has the keyboard.
    ///
    /// Only the tests ask: the drawing colours the focused field as it walks,
    /// and asking would be the same list traversed twice. It is here because
    /// `menu_at` has to be provable — it decides which menu opens, and the
    /// pane is the half of that decision nothing on screen spells out.
    #[cfg(test)]
    const fn pane(&self) -> Pane {
        self.pane
    }

    /// Is an ordinary LISTING on screen, rather than a menu, a question or a
    /// sheet?
    ///
    /// **Everything that is drawn beside the rows has to ask this**, and the
    /// three things that did not — the vertical rule, the pointer's band and
    /// the scrollbar — are why a menu opened over a line running down the
    /// middle of it, with a hover band lighting half a row of a list that was
    /// no longer there. They were painted outside `expansion()`, which is the
    /// only place that used to know.
    ///
    /// It lives here and not in either door for the reason the whole view
    /// does: two doors answering this separately is how they come to disagree.
    #[must_use]
    pub const fn plain(&self) -> bool {
        self.menu.is_none() && self.detail.is_none() && self.picking.is_none()
    }

    /// Where the sidebar ends, at this width — and zero when it is not drawn.
    ///
    /// Zero while a menu, a question or a sheet is up: none of them has a
    /// sidebar beside it, so nothing that keys off the split gets drawn either.
    ///
    /// **THE ONE PLACE THE SPLIT IS COMPUTED.** Both painters, both hit-testers
    /// and the rule all need it, and they used to ask `sidebar_columns` for
    /// themselves. That was already wrong on a second monitor: the rows were
    /// laid out with the FIRST screen's width and the rule with the width of
    /// the screen being painted, so on a narrower one the line and the fields
    /// landed at different cells. One method, one width, one answer.
    #[must_use]
    pub fn split(&self, width: u16) -> u16 {
        if !self.plain() {
            return 0;
        }
        sidebar_columns(width, self.sidebar_showing(), self.side_content())
    }

    /// How many cells the widest sidebar row wants.
    fn side_content(&self) -> u16 {
        self.side
            .shown(self.hidden)
            .into_iter()
            .flatten()
            .map(side_row_width)
            .max()
            .unwrap_or(0)
    }

    /// Is the sidebar asking to be drawn?
    ///
    /// It answers the PREFERENCE and not the arithmetic: whether it FITS is
    /// `sidebar_columns`' answer, and the two doors ask that themselves with
    /// their own width. A single method answering both would need a width and
    /// would be the third place the split is computed.
    ///
    /// **An EMPTY sidebar does not ask.** Otherwise a machine where none of the
    /// declared directories exists — a fresh install, a test's home, the VM —
    /// gives a quarter of the screen and a vertical rule to nothing at all, and
    /// the listing loses its columns to hold it.
    #[must_use]
    pub fn sidebar_showing(&self) -> bool {
        self.side.showing() && self.side.how_many(self.hidden) > 0
    }

    /// The path above, as clickable pieces.
    #[must_use]
    pub fn crumbs(&self) -> Vec<Field> {
        let parts = listing::ancestry(&self.where_);
        let mut fields = Vec::new();
        let mut at = 0u16;
        for (n, (label, _)) in parts.iter().enumerate() {
            let text = if n == 0 {
                label.clone()
            } else {
                format!("{label}/")
            };
            // The last piece is where one IS, and it is not a place to go.
            let hit = if n + 1 == parts.len() {
                Hit::Nothing
            } else {
                Hit::Crumb(n)
            };
            let color = if n + 1 == parts.len() {
                ronoctua_tokens::color::TEXT_PRIMARY
            } else {
                ronoctua_tokens::color::TEXT_SECONDARY
            };
            let width = width_of(&text);
            fields.push(Field {
                at,
                text,
                color,
                hit,
                explain: None,
            });
            at += width;
        }
        // WHAT WAS TYPED GOES AT THE END OF THE PATH, in the accent colour.
        //
        // # Why here, and not on the message line
        //
        // Because the two things are different in kind. The path says WHERE you
        // are and the query narrows what is shown THERE — read together they
        // are one sentence, and the query reads as what it is: a suffix that is
        // not a directory yet. The message line meanwhile has a job of its own —
        // what the pointer is resting on, what the last action said — and while
        // the query lived there, hovering a permission bit replaced what was
        // being typed, or was replaced by it, depending on the order.
        //
        // The colour is the whole formatting. No box, no separator, no prefix
        // (invariants 14 and 15): the accent already says "this part is not the
        // path".
        if !self.filter.is_empty() {
            fields.push(Field {
                at,
                text: self.filter.clone(),
                color: ronoctua_tokens::color::STATE_ACCENT,
                hit: Hit::Nothing,
                explain: None,
            });
        }
        fields
    }

    /// Where a crumb leads.
    #[must_use]
    pub fn crumb_target(&self, n: usize) -> Option<PathBuf> {
        listing::ancestry(&self.where_)
            .get(n)
            .map(|(_, p)| p.clone())
    }

    /// The sidebar's half of line `n`.
    ///
    /// A glyph, a name, and — on a mounted device only — an eject glyph against
    /// the sidebar's right edge. Nothing else: no heading, no rule, no box
    /// (invariants 14 and 15). What separates the blocks is a BLANK LINE, which
    /// is a row with no place in it and costs no text key.
    fn side_fields(
        &self,
        n: usize,
        row: &ronoctua_files::sidebar::Row,
        split: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Field> {
        use ronoctua_files::sidebar::Target;
        use ronoctua_tokens::{color, icon};

        let focused = matches!(self.pane, Pane::Sidebar) && n == self.side_cursor;
        let glyph = match &row.target {
            Target::Place(_) => icon::FOLDER,
            Target::File(_) => icon::FILE,
            // THE CATALOGUE THE LAUNCHER ALREADY SWEPT, and no directory walked
            // here. An id that is not in it — an application uninstalled since
            // the row was pinned — falls back to the generic glyph rather than
            // to nothing: a row with no drawing reads as a row that failed to
            // load, and this one still opens the picker that says what happened.
            Target::Application(id) => self
                .catalogue
                .iter()
                .find(|e| e.id == *id)
                .map_or(icon::GENERIC, crate::apps::glyph_of),
            Target::Trash => icon::TRASH,
            // THE DRAWING SAYS WHICH KIND OF DISK, and the colour says whether
            // it is mounted. Two facts, two channels — the split the listing's
            // mark and focus already keep, and for the same reason. Deriving
            // the glyph from `mounted` instead would redraw an internal
            // partition as a stick the moment somebody unmounted it.
            Target::Device(_) => {
                if row.removable {
                    icon::DISK_REMOVABLE
                } else {
                    icon::DISK
                }
            }
        };
        // An unmounted device is DIM: it is a place one cannot walk into yet.
        // Everything else is ordinary text, and the cursor is the accent — the
        // same three tones the listing uses.
        let tone = if focused {
            color::STATE_ACCENT
        } else if row.mounted == Some(false) {
            color::TEXT_DIM
        } else {
            color::TEXT_SECONDARY
        };

        let name = row
            .label
            .clone()
            .unwrap_or_else(|| texts.t(ronoctua_i18n::key::FILES_SIDEBAR_TRASH));

        // AIR ON BOTH SIDES, and both numbers come from a token.
        //
        // The glyph used to sit at cell zero and the eject glyph in the last
        // cell before the rule: flush against the screen's border on one side
        // and against the rule on the other. A glyph against a border does not
        // read as a glyph at the edge, it reads as one that was CUT OFF — which
        // is the same sentence `space.nix` already carries twice, about the
        // notification counter and about the date column.
        let name_at = SIDE_GUTTER + 2;
        // What is left for the name once both edges and the eject glyph are
        // paid for. `EDGE_RIGHT` is the sidebar's own right air, the same cell
        // the listing leaves after its last column.
        let room = split
            .saturating_sub(name_at)
            .saturating_sub(EDGE_RIGHT)
            .saturating_sub(if eject_room(row) > 0 {
                eject_room(row) + SIDE_EJECT
            } else {
                0
            });
        let mut fields = vec![
            Field {
                at: SIDE_GUTTER,
                text: glyph.to_string(),
                color: tone,
                hit: Hit::Sidebar(n),
                explain: None,
            },
            Field {
                at: name_at,
                // Elided by CLUSTER and not by codepoint — cutting a codepoint
                // invents a drawing, which is a trap this project has paid for.
                text: ronoctua_render::layout::elide_end(&name, room),
                color: tone,
                hit: Hit::Sidebar(n),
                explain: Some(self.side_explanation(row, texts)),
            },
        ];
        if eject_room(row) > 0 {
            fields.push(Field {
                // Against the RIGHT air and not against the rule: `EDGE_RIGHT`
                // is what stands between this glyph and the line, and without
                // it the two read as one smudge instead of as a target and a
                // boundary.
                at: split.saturating_sub(EDGE_RIGHT + 1),
                text: ronoctua_tokens::icon::EJECT.to_string(),
                color: color::TEXT_DIM,
                hit: Hit::Eject(n),
                explain: Some(mode::Explanation {
                    key: ronoctua_i18n::key::FILES_SIDEBAR_UNMOUNT,
                    args: vec![("name", name)],
                }),
            });
        }
        fields
    }

    /// What the pointer resting on a sidebar row says on the message line.
    fn side_explanation(
        &self,
        row: &ronoctua_files::sidebar::Row,
        _texts: &ronoctua_i18n::Texts,
    ) -> mode::Explanation {
        use ronoctua_files::sidebar::Target;
        let name = row
            .label
            .clone()
            .unwrap_or_else(|| String::from(ronoctua_i18n::key::FILES_SIDEBAR_TRASH));
        let key = match (&row.target, row.mounted) {
            // AN UNMOUNTED DEVICE SAYS SO, and a mounted one does not — the one
            // difference a person has to know before pressing the key, because
            // one of the two takes a moment and asks the system for something.
            (Target::Device(_), Some(false)) => ronoctua_i18n::key::FILES_SIDEBAR_MOUNT,
            (Target::Application(_), _) => ronoctua_i18n::key::FILES_SIDEBAR_LAUNCH,
            _ => ronoctua_i18n::key::FILES_SIDEBAR_OPEN,
        };
        mode::Explanation {
            key,
            args: vec![("name", name)],
        }
    }

    fn row_of(&self, n: usize, i: usize, layout: &Layout) -> Row {
        let entry = &self.entries[i];
        let of_type = self.types[i];
        let owner = self
            .roster
            .person(entry.uid)
            .map_or_else(|| entry.uid.to_string(), ToOwned::to_owned);
        let group = self
            .roster
            .group(entry.gid)
            .map_or_else(|| entry.gid.to_string(), ToOwned::to_owned);
        let marked = self.marks.contains(&entry.name);
        let focused = n == self.cursor;
        let mut fields = Vec::new();

        // THE MARK, IN THE GUTTER. A glyph and not a colour, because the colour
        // is already saying something else: focus. Two facts on one channel
        // leaves "which one is the cursor on?" unanswered the moment a second
        // row is marked — the defect the app finder's own note describes.
        //
        // It is the same place and the same idea as the clipboard's pin.
        fields.push(Field {
            // THE LISTING'S FIRST CELL, and not the screen's. With a sidebar
            // beside it, cell zero belongs to the sidebar — a mark left there
            // sits under a place's last letter, and clicking it marks
            // something else entirely. `Layout::from` is the one place that
            // knows where the listing begins.
            at: layout.from,
            text: if marked {
                ronoctua_tokens::icon::SELECTION.to_string()
            } else {
                String::new()
            },
            color: ronoctua_tokens::color::STATE_ACCENT,
            hit: Hit::Mark(n),
            explain: None,
        });

        for (column, at, width) in &layout.columns {
            match column {
                Column::Icon => {
                    let glyph = entry.kind.icon().unwrap_or_else(|| mime::icon_of(of_type));
                    fields.push(Field {
                        at: *at,
                        text: glyph.to_string(),
                        color: ronoctua_tokens::color::TEXT_SECONDARY,
                        hit: Hit::Mark(n),
                        explain: None,
                    });
                }
                Column::Name => {
                    // A symlink says where it goes, because that is the one
                    // thing about it a listing cannot leave out.
                    let shown = match (&entry.target, entry.broken()) {
                        (Some(t), false) => format!("{} → {t}", entry.name),
                        (Some(t), true) => format!("{} ⇢ {t}", entry.name),
                        (None, _) => entry.name.clone(),
                    };
                    let text = ronoctua_render::layout::elide_end(&shown, *width);
                    fields.push(Field {
                        at: *at,
                        text,
                        color: colour_of(entry, focused),
                        hit: Hit::Name(n),
                        explain: None,
                    });
                }
                Column::Perms => {
                    // ONE FIELD PER CHARACTER. Ten facts that look like one
                    // word: without a field each, the pointer could only
                    // explain the whole column, which is the same as
                    // explaining nothing.
                    for (k, cell) in mode::cells(entry.mode).into_iter().enumerate() {
                        let hit = match (cell.special, cell.segment) {
                            (Some(s), _) => Hit::Special(n, s),
                            (None, mode::Segment::Bit(who, what)) => Hit::Bit(n, who, what),
                            (None, mode::Segment::Kind) => Hit::Nothing,
                        };
                        fields.push(Field {
                            at: at + u16::try_from(k).unwrap_or(0),
                            text: cell.letter.to_string(),
                            color: if cell.set {
                                ronoctua_tokens::color::TEXT_SECONDARY
                            } else {
                                ronoctua_tokens::color::TEXT_DIM
                            },
                            hit,
                            explain: Some(mode::explain(cell, entry.mode, &owner, &group)),
                        });
                    }
                }
                Column::Owner => fields.push(Field {
                    at: *at,
                    text: ronoctua_render::layout::elide_end(&owner, *width),
                    color: ronoctua_tokens::color::TEXT_SECONDARY,
                    hit: Hit::Owner(n),
                    // A FIELD THAT LIGHTS UP SAYS WHY. Without this the owner
                    // column would highlight under the pointer and explain
                    // nothing, which promises an action and describes none.
                    explain: Some(Explanation {
                        key: ronoctua_i18n::key::FILES_EXPLAIN_OWNER,
                        args: vec![("who", owner.clone())],
                    }),
                }),
                Column::Group => fields.push(Field {
                    at: *at,
                    text: ronoctua_render::layout::elide_end(&group, *width),
                    color: ronoctua_tokens::color::TEXT_SECONDARY,
                    hit: Hit::Group(n),
                    explain: Some(Explanation {
                        key: ronoctua_i18n::key::FILES_EXPLAIN_GROUP,
                        args: vec![("who", group.clone())],
                    }),
                }),
                Column::Size => {
                    let measured = self
                        .measured
                        .as_ref()
                        .filter(|(name, _)| *name == entry.name)
                        .map(|(_, b)| *b);
                    // A directory's own size is four kibibytes and means
                    // nothing to anybody. Left blank until asked, because a
                    // number that answers a different question is worse than
                    // no number.
                    let text = match (entry.kind, measured) {
                        (_, Some(bytes)) => size_text(bytes),
                        (Kind::Directory, None) => String::new(),
                        (_, None) => size_text(entry.bytes),
                    };
                    // A DIRECTORY'S SIZE IS A WALK, and the empty cell has to
                    // say so — an empty column reads as "zero bytes", which is
                    // a different and false answer.
                    let explain = Some(Explanation {
                        key: if matches!(entry.kind, Kind::Directory) {
                            ronoctua_i18n::key::FILES_EXPLAIN_MEASURE
                        } else {
                            ronoctua_i18n::key::FILES_EXPLAIN_SIZE
                        },
                        args: vec![("bytes", entry.bytes.to_string())],
                    });
                    fields.push(Field {
                        at: *at,
                        text,
                        color: ronoctua_tokens::color::TEXT_SECONDARY,
                        hit: Hit::Measure(n),
                        explain,
                    });
                }
                Column::Modified => {
                    let c = when::civil(entry.modified, &self.zone);
                    fields.push(Field {
                        at: *at,
                        text: format!(
                            "{:04}-{:02}-{:02} {:02}:{:02}",
                            c.year, c.month, c.day, c.hour, c.minute
                        ),
                        color: ronoctua_tokens::color::TEXT_DIM,
                        hit: Hit::Nothing,
                        // THE SECONDS, which the column has no room for — and
                        // whether this is local time at all. When
                        // `/etc/localtime` could not be read the column is UTC,
                        // and saying so is the difference between a date one
                        // can trust and one that is an hour out for ever.
                        explain: Some(Explanation {
                            key: if self.zone.is_utc() {
                                ronoctua_i18n::key::FILES_EXPLAIN_WHEN_UTC
                            } else {
                                ronoctua_i18n::key::FILES_EXPLAIN_WHEN
                            },
                            args: vec![(
                                "seconds",
                                entry
                                    .modified
                                    .saturating_add(i64::from(self.zone.offset_at(entry.modified)))
                                    .rem_euclid(60)
                                    .to_string(),
                            )],
                        }),
                    });
                }
            }
        }
        Row {
            fields,
            marked,
            focused,
        }
    }

    fn menu_rows(&self, chosen: usize, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        // THE PANE DECIDES WHICH MENU, because the two are about different
        // kinds of thing: one is about an entry in a directory, the other about
        // a place or a disk. The pane is already tracked, so there is no second
        // piece of state saying which menu is open — and no way for the two to
        // disagree.
        if matches!(self.pane, Pane::Sidebar) {
            return self.side_menu_rows(chosen, width, texts);
        }
        let Some((entry, of_type)) = self.menu_subject() else {
            return Vec::new();
        };
        let offered = menu::for_entry(
            &entry,
            of_type,
            !self.clip.paths.is_empty(),
            self.in_trash(),
        );
        // WHO THE MENU IS ABOUT, on its first line.
        //
        // Asked for from the screen, and the reason is that the menu covers the
        // listing: the row it was opened on is no longer visible, so twenty
        // verbs sit there with nothing saying what they would act on. With
        // marks it is worse — the actions apply to all of them, and the entry
        // under the pointer is not the answer.
        let target = if self.marks.is_empty() {
            texts.t_with(
                ronoctua_i18n::key::FILES_MENU_ON,
                &[("name", entry.name.as_str())],
            )
        } else {
            texts.t_with(
                ronoctua_i18n::key::FILES_MENU_ON_MANY,
                &[("n", &self.marks.len().to_string())],
            )
        };
        let title = Row {
            fields: vec![Field {
                at: 0,
                text: ronoctua_render::layout::elide_end(&target, width),
                color: ronoctua_tokens::color::TEXT_SECONDARY,
                hit: Hit::Nothing,
                explain: None,
            }],
            marked: false,
            focused: false,
        };
        std::iter::once(title)
            .chain(offered.iter().enumerate().map(|(n, a)| {
                let armed = self.arming == Some(a.id);
                Row {
                    fields: vec![
                        Field {
                            at: 0,
                            // The key sits flush right in its own column, with
                            // the colon aligned — the same arrangement the
                            // which-key list uses, so the two read alike.
                            text: format!("{:>4} :", a.key),
                            color: ronoctua_tokens::color::STATE_ACCENT,
                            hit: Hit::Menu(a.id),
                            explain: Some(does(a.does)),
                        },
                        Field {
                            at: 7,
                            // THE TRANSLATED LABEL, and not the key. It drew
                            // `files-open` until this argument existed — a raw
                            // text key on screen, which is the exact shape
                            // invariant 8 exists to refuse.
                            text: ronoctua_render::layout::elide_end(
                                &texts.t(a.label),
                                width.saturating_sub(7),
                            ),
                            color: if armed {
                                ronoctua_tokens::color::STATE_WARNING
                            } else {
                                ronoctua_tokens::color::TEXT_PRIMARY
                            },
                            hit: Hit::Menu(a.id),
                            explain: Some(does(a.does)),
                        },
                    ],
                    marked: false,
                    focused: n == chosen,
                }
            }))
            .collect()
    }

    /// The sidebar's menu: what the row IS, then what can be done with it.
    ///
    /// # Why the facts share the surface with the actions
    ///
    /// Because pressing an unmounted device does not mount it any more — it
    /// opens this. That was asked for from the screen: *"mostrar informações
    /// sobre o item, e mostrar a opção de montá-lo"*. A separate sheet with a
    /// separate key would be a second mechanism for what a menu that names its
    /// subject already does, and the listing's menu has named its subject since
    /// the day it was reported not to.
    fn side_menu_rows(&self, chosen: usize, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        use ronoctua_files::sidebar::Target;
        let Some(row) = self.focused_side() else {
            return Vec::new();
        };
        let name = self.name_of_side(&row);
        let plain = |text: String, color| Row {
            fields: vec![Field {
                at: 0,
                text: ronoctua_render::layout::elide_end(&text, width),
                color,
                hit: Hit::Nothing,
                explain: None,
            }],
            marked: false,
            focused: false,
        };

        let mut rows = vec![plain(
            texts.t_with(
                ronoctua_i18n::key::SIDEBAR_MENU_ON,
                &[("name", name.as_str())],
            ),
            ronoctua_tokens::color::TEXT_SECONDARY,
        )];

        // THE FACTS, and only for a device — a folder's facts are the listing.
        if matches!(row.target, Target::Device(_)) {
            let mut fact = |k: &'static str, v: String| {
                rows.push(plain(
                    format!("{:>16}  {v}", texts.t(k)),
                    ronoctua_tokens::color::TEXT_DIM,
                ));
            };
            fact(
                ronoctua_i18n::key::SIDEBAR_FACT_WHERE,
                row.at.as_ref().map_or_else(
                    || texts.t(ronoctua_i18n::key::SIDEBAR_NOT_MOUNTED),
                    |p| p.to_string_lossy().into_owned(),
                ),
            );
            // WHY IT CANNOT BE FORMATTED OR UNMOUNTED, said here rather than
            // left to be guessed from rows that are missing. A list that is
            // shorter for an unstated reason is a list nobody trusts.
            if let Some(why) = &row.needed {
                fact(ronoctua_i18n::key::SIDEBAR_FACT_NEEDED, why.clone());
            }
            rows.push(plain(String::new(), ronoctua_tokens::color::TEXT_DIM));
        }

        let offered = ronoctua_files::sidebar_menu::for_row(&row);
        rows.extend(offered.iter().enumerate().map(|(n, a)| Row {
            fields: vec![
                Field {
                    at: 0,
                    // The same arrangement as the listing's menu and as
                    // which-key: the key flush right, the colon aligned.
                    text: format!("{:>4} :", a.key),
                    color: ronoctua_tokens::color::STATE_ACCENT,
                    hit: Hit::SideMenu(a.id),
                    explain: Some(does(a.does)),
                },
                Field {
                    at: 7,
                    text: ronoctua_render::layout::elide_end(
                        &texts.t(a.label),
                        width.saturating_sub(7),
                    ),
                    color: ronoctua_tokens::color::TEXT_PRIMARY,
                    hit: Hit::SideMenu(a.id),
                    explain: Some(does(a.does)),
                },
            ],
            marked: false,
            focused: n == chosen,
        }));
        rows
    }

    /// What the row under the CURSOR does, in the technical register.
    ///
    /// # Why this exists beside `Field::explain` and does not replace it
    ///
    /// `explain` is answered per FIELD, because pointing at the `w` of a group
    /// says something different from pointing at the `x` beside it. A cursor
    /// has no column — it is on a ROW — so it could never use that path, and
    /// until this existed every explanation in this file was something only a
    /// mouse could ask for. On a menu the distinction does not arise: every
    /// field of the row carries the same answer, because the row IS one choice.
    ///
    /// Only while a menu is open, and deliberately. On the listing a row is a
    /// record with columns, and choosing one of its columns to explain because
    /// the cursor is somewhere on the line would be choosing at random.
    ///
    /// THE PANE DECIDES WHICH MENU, exactly as `menu_rows` decides — one place
    /// answering both questions is what keeps the explanation on the same row
    /// the keys act on.
    #[must_use]
    pub fn cursor_explains(&self) -> Option<mode::Explanation> {
        let chosen = self.menu?;
        if matches!(self.pane, Pane::Sidebar) {
            let row = self.focused_side()?;
            return ronoctua_files::sidebar_menu::for_row(&row)
                .get(chosen)
                .map(|a| does(a.does));
        }
        let (entry, of_type) = self.menu_subject()?;
        menu::for_entry(
            &entry,
            of_type,
            !self.clip.paths.is_empty(),
            self.in_trash(),
        )
        .get(chosen)
        .map(|a| does(a.does))
    }

    /// The rows the filter leaves, recomputed.
    fn refilter(&mut self) {
        self.shown = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| self.hidden || !e.hidden())
            .filter(|(_, e)| {
                self.filter.is_empty()
                    || crate::minibuffer::match_up(&self.filter, &e.name).is_some()
            })
            .map(|(i, _)| i)
            .collect();
    }

    fn clamp(&mut self) {
        if self.shown.is_empty() {
            self.cursor = 0;
        } else if self.cursor >= self.shown.len() {
            self.cursor = self.shown.len() - 1;
        }
    }

    /// The focused entry, when it is a place one can be inside.
    ///
    /// It follows a link into a directory, for the reason opening does: that is
    /// what a link into a directory is for.
    fn inside(&self) -> Option<PathBuf> {
        let entry = self.focused()?;
        matches!(entry.opens_as(), Kind::Directory).then(|| self.where_.join(&entry.name))
    }

    /// The paths an action applies to: the marks, or the focused row.
    ///
    /// Marks win when there are any, which is the rule every file manager
    /// follows and the one that surprises when it is not followed: having
    /// marked six files, an action on the seventh is not what anybody meant.
    #[must_use]
    pub fn chosen(&self) -> Vec<PathBuf> {
        if self.marks.is_empty() {
            return self.focused_path().into_iter().collect();
        }
        self.marks.iter().map(|n| self.where_.join(n)).collect()
    }
}

/// What is left to say after an operation was carried out.
///
/// The manager has TWO doors — the footer's region and a window of its own —
/// and everything below this line is the same code for both. What differs is
/// only how a process is started: the footer already has `open_foreign`, with
/// its scope and its Xwayland satellite, and the window uses the same crate a
/// step lower down. Duplicating the two hundred lines above that difference is
/// how the two doors would come to behave differently, which is the one thing
/// "one implementation" has to mean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// It was done, or handed to a thread. Read the directory again when the
    /// report arrives.
    Done,
    /// Put this on the message line.
    Say(Explanation),
}

/// Carries out one operation, minus the spawning.
///
/// Split by COST and not by kind: a chmod is one syscall and answers before the
/// hand leaves the key; a copy, a trash and a walk are unbounded and go to a
/// named thread whose result comes back through `sender`. It is the lesson
/// `systemd-run --scope` already taught this project — waiting inside the loop
/// that draws freezes the keyboard, the painting and the compositor's events
/// for as long as the work takes.
#[must_use]
pub fn carry_out(doing: Doing, sender: &calloop::channel::Sender<Report>, clip: &Clip) -> Errand {
    match doing {
        // The four that need a process are the door's business.
        Doing::Open(_) | Doing::OpenWith(_, _) | Doing::Terminal(_) | Doing::RootTerminal(_) => {
            Errand::Done
        }
        Doing::Copy(_) => Errand::Done,
        // Launching is a process too, and it goes through the launcher's own
        // path — never a second reader of a `.desktop` file's `Exec`.
        Doing::Launch(_) => Errand::Done,
        // MOUNTING GOES TO A THREAD, and it is not a preference.
        //
        // udisks goes through polkit, and polkit may be deciding — with no
        // authentication agent in this session it answers at once, but that is
        // a property of the session and not of the call. Waiting inside the
        // loop that draws freezes the screen for as long as it takes, which is
        // the `systemd-run --scope` lesson this file already carries. The
        // volume comes back mounted through `Report::Mounted`, and going in is
        // what the person meant by pressing the row.
        Doing::Mount(id) => {
            let sender = sender.clone();
            raise("montar", move || {
                let report = match ronoctua_disks::Manager::fresh().and_then(|m| m.mount(&id)) {
                    Ok(at) => Report::Mounted(std::path::PathBuf::from(at)),
                    // THE NAME OF THE ERROR, and not its prose. `NotSupported`
                    // and `NotAuthorized` are two answers a person can act on,
                    // and udisks' English is upstream's to reword whenever it
                    // likes. The prose travels alongside as an ARGUMENT — the
                    // concession the transfer queue already makes, because
                    // "could not mount" alone does not say "unknown filesystem
                    // type".
                    Err(e) => Report::Refused(ronoctua_disks::refusal(&e), e.to_string()),
                };
                let _ = sender.send(report);
            });
            Errand::Done
        }
        // TWO CALLS, AND THE SECOND ONLY IF THE FIRST WORKED. Cutting the
        // power to a drive whose filesystem is still mounted is how a stick
        // leaves with a write half done — so this unmounts, and powers off only
        // when that came back clean. A refusal on either half is reported as
        // itself.
        Doing::Eject(id) => {
            let sender = sender.clone();
            raise("ejetar", move || {
                let report = match ronoctua_disks::Manager::fresh() {
                    Ok(m) => match m.unmount(&id).and_then(|()| m.power_off(&id)) {
                        Ok(()) => Report::Done,
                        Err(e) => Report::Refused(ronoctua_disks::refusal(&e), e.to_string()),
                    },
                    Err(e) => Report::Failed(e.to_string()),
                };
                let _ = sender.send(report);
            });
            Errand::Done
        }
        // THE DOOR'S BUSINESS, like the other four that start a process: it
        // opens the formatting buffer with the volume as its argument.
        Doing::Format(_) => Errand::Done,
        Doing::Unmount(id) => {
            let sender = sender.clone();
            raise("desmontar", move || {
                let report = match ronoctua_disks::Manager::fresh().and_then(|m| m.unmount(&id)) {
                    Ok(()) => Report::Done,
                    Err(e) => Report::Refused(ronoctua_disks::refusal(&e), e.to_string()),
                };
                let _ = sender.send(report);
            });
            Errand::Done
        }
        // ONE SYSCALL EACH: they answer now, and the listing shows the new
        // value before the hand leaves the key.
        // A LIST EACH, and the first refusal STOPS the rest.
        //
        // Carrying on after one path was refused would leave a set half
        // changed, with one message about one file — and no way to tell which
        // half. Stopping names the one that refused and leaves the rest as it
        // was, which is a state one can act on.
        Doing::Mode(paths, mode) => said(
            paths
                .iter()
                .try_for_each(|p| ronoctua_files::act::chmod(p, mode)),
        ),
        Doing::Group(paths, gid) => said(
            paths
                .iter()
                .try_for_each(|p| ronoctua_files::act::chgrp(p, gid)),
        ),
        // THE ONE THAT NEEDS ROOT. Written into a directory this user owns and
        // handed to a unit that VALIDATES before acting — the same shape
        // `ronoctua_rescue::edit` uses for `host.nix`.
        Doing::Owner(paths, uid) => {
            // THE WHOLE SET, one staged request at a time.
            //
            // The unit installs ONE request — that is what lets its name be
            // fixed and the polkit rule name it — so the set means starting it
            // once per path. polkit's `auth_self_keep` is what keeps that from
            // being one password per file.
            //
            // On the thread, because `systemctl start` on a system unit blocks
            // until the oneshot finishes and polkit may be asking a password in
            // the middle of it. In the loop that draws, that is a frozen screen
            // for as long as somebody takes to type.
            let Some(systemctl) = option_env!("RONOCTUA_SYSTEMCTL") else {
                return Errand::Say(Explanation {
                    key: ronoctua_i18n::key::FILES_NO_SYSTEMCTL,
                    args: Vec::new(),
                });
            };
            // AND THE PLACE THE REQUEST IS WRITTEN HAS TO EXIST — asked HERE,
            // not discovered in the thread.
            //
            // `/run/ronoctua-files/` is a `RuntimeDirectory` of the unit that
            // `nix/modules/files.nix` installs, so on a machine that is not
            // running this distro — the nested loop, above all, where most of
            // the work happens — it is simply not there. What came out of the
            // thread then was `criar /run/ronoctua-files/staged: Permission
            // denied`, three seconds on the message line, and a report of
            // "trocar o dono não fez nada".
            //
            // Answering before starting turns that into a sentence that says
            // WHY, and it says it synchronously, which is the difference
            // between a message and a message somebody has to catch.
            if !std::path::Path::new(ronoctua_files::act::STAGING)
                .parent()
                .is_some_and(std::path::Path::is_dir)
            {
                return Errand::Say(Explanation {
                    key: ronoctua_i18n::key::FILES_NO_ROOT_HELPER,
                    args: Vec::new(),
                });
            }
            let systemctl = systemctl.to_owned();
            in_a_thread(sender, "chown", move || {
                let staging = std::path::PathBuf::from(ronoctua_files::act::STAGING);
                for path in paths {
                    ronoctua_files::act::Staged {
                        verb: ronoctua_files::act::Verb::Chown,
                        path,
                        value: uid.to_string(),
                    }
                    .write_into(&staging)?;
                    let out = std::process::Command::new(&systemctl)
                        .arg("start")
                        .arg(ronoctua_files::act::UNIT)
                        .output()?;
                    // THE FIRST REFUSAL STOPS THE REST, for the reason the
                    // syscalls above give: a set half changed with one message
                    // about one file is a state nobody can act on.
                    if !out.status.success() {
                        return Err(anyhow::anyhow!(
                            "{}",
                            String::from_utf8_lossy(&out.stderr).trim().to_owned()
                        ));
                    }
                }
                Ok(())
            });
            Errand::Done
        }
        // UNBOUNDED, EVERY ONE OF THEM — and they no longer go to a thread of
        // this process. They go to the QUEUE.
        //
        // # What changed, and why
        //
        // Each of these used to raise a named thread here, work in silence and
        // report once at the end. Seven operations, and the only thing the
        // screen ever said about any of them was the listing refreshing when it
        // was over — including `shred`, which writes three times the file and
        // is the slowest of the lot.
        //
        // Now the request crosses to `ronoctua-transfers`, which owns them.
        // That buys three things this side could not have: the SAME list is
        // visible from the footer, from this window and from the panel; the
        // work survives the footer being restarted, which in the nested loop
        // happens on every file edit; and every one of them can be held and
        // given up, because the switch lives with the thread rather than with
        // whoever asked.
        //
        // What comes back is not a `Report` any more — it is the list, through
        // the watching thread, which is the only thing that has to be right.
        Doing::Trash(root, paths) => queued(Work::Trash { root, paths }),
        Doing::Restore(root, names) => queued(Work::Restore { root, names }),
        Doing::Delete(paths) => queued(Work::Delete { paths }),
        Doing::Shred(paths) => queued(Work::Shred { paths }),
        Doing::Paste(into) => queued(Work::Paste {
            paths: clip.paths.clone(),
            into,
            moves: clip.moves,
        }),
        Doing::Compress(paths, extension) => {
            let Some(first) = paths.first().cloned() else {
                return Errand::Done;
            };
            let Some(directory) = first.parent().map(Path::to_path_buf) else {
                return Errand::Done;
            };
            let names: Vec<String> = paths
                .iter()
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .collect();
            let stem = first
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "arquivo".to_owned());
            let archive = directory.join(format!("{stem}.{extension}"));
            queued(Work::Compress {
                directory,
                names,
                archive,
            })
        }
        Doing::Extract(archive) => {
            let Some(into) = archive.parent().map(Path::to_path_buf) else {
                return Errand::Done;
            };
            queued(Work::Extract { archive, into })
        }
        // THE WALK. It is in the queue for the same reason as the rest: it is
        // unbounded, and it was the one operation somebody watches the screen
        // during with nothing to watch. The answer comes back in the list —
        // see `Footer::measurements_of`.
        Doing::Measure(path) => queued(Work::Measure { path }),
    }
}

/// Hands one piece of work to the queue.
///
/// It is `tell` and not `ask`: nothing here needs the id back, and waiting for
/// an answer would put a socket round trip inside the frame that draws — the
/// lesson `systemd-run --scope` already taught this project twice.
///
/// A queue that is not up says so ON THE MESSAGE LINE. It used to be
/// unthinkable for a paste to fail before it started, and the honest handling
/// of a new failure mode is a sentence where the person is looking, not a line
/// in a journal.
fn queued(work: Work) -> Errand {
    match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
        Ok(()) => Errand::Done,
        Err(e) => {
            tracing::warn!(error = %e, "a fila de transferencias nao aceitou o trabalho");
            Errand::Say(Explanation {
                key: ronoctua_i18n::key::TRANSFERS_OFFLINE,
                args: Vec::new(),
            })
        }
    }
}

/// An immediate result, as something to say or nothing at all.
fn said(outcome: anyhow::Result<()>) -> Errand {
    match outcome {
        Ok(()) => Errand::Done,
        Err(e) => Errand::Say(refused(&e.to_string())),
    }
}

/// The system's own words, shown as they came.
///
/// A message written here instead would be a second explanation of a refusal
/// the kernel already explained — and ours would be the one that goes stale.
fn refused(why: &str) -> Explanation {
    Explanation {
        key: ronoctua_i18n::key::FILES_REFUSED,
        args: vec![("why", why.to_owned())],
    }
}

/// Runs something unbounded on a named thread, and reports back.
fn in_a_thread(
    sender: &calloop::channel::Sender<Report>,
    name: &str,
    work: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
) {
    let sender = sender.clone();
    raise(name, move || {
        let report = match work() {
            Ok(()) => Report::Done,
            Err(e) => Report::Failed(e.to_string()),
        };
        let _ = sender.send(report);
    });
}

/// A NAMED thread, always: an unnamed one in `top` says `ronoctua-shell` and
/// nothing about what it is doing, on the one screen that exists to answer
/// that.
fn raise(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(e) = std::thread::Builder::new()
        .name(format!("arquivos-{name}"))
        .spawn(work)
    {
        tracing::error!(error = %e, thread = name, "nao consegui subir a thread");
    }
}

/// The picker, drawn in the manager's own region.
///
/// The first line is the question and what has been typed; the rest are the
/// choices. Two lines glued and a list below them is the arrangement the whole
/// system uses — it is the footer's own shape, and the reason it is repeated
/// here is that a person should not have to learn a second one.
fn picker_rows(picker: &Picker, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
    // THE QUESTION, ON THE SCREEN THAT ASKS IT.
    //
    // It was missing, and the report was exactly the shape of the gap: a list
    // of names came up and nothing said it was about the OWNER. The choices
    // themselves cannot say it — a list of people looks the same whether it is
    // about who owns a file or who to send it to — and the menu row that opened
    // it is long gone from the screen by then.
    let mut rows = vec![
        Row {
            fields: vec![Field {
                at: 0,
                text: ronoctua_render::layout::elide_end(&picker.asking.title(texts), width),
                color: ronoctua_tokens::color::TEXT_SECONDARY,
                hit: Hit::Nothing,
                explain: None,
            }],
            marked: false,
            focused: false,
        },
        Row {
            fields: vec![Field {
                at: 0,
                text: ronoctua_render::layout::elide_end(&format!("> {}", picker.query), width),
                color: ronoctua_tokens::color::TEXT_PRIMARY,
                hit: Hit::Nothing,
                explain: None,
            }],
            marked: false,
            focused: false,
        },
    ];
    for (n, i) in picker.shown().into_iter().enumerate() {
        let Some(c) = picker.choices.get(i) else {
            continue;
        };
        rows.push(Row {
            fields: vec![Field {
                at: 2,
                text: ronoctua_render::layout::elide_end(&c.label, width.saturating_sub(2)),
                color: if n == picker.cursor {
                    ronoctua_tokens::color::TEXT_PRIMARY
                } else {
                    ronoctua_tokens::color::TEXT_SECONDARY
                },
                hit: Hit::Choice(n),
                explain: None,
            }],
            marked: false,
            focused: n == picker.cursor,
        });
    }
    rows
}

/// The colour a name is drawn in.
///
/// By what it IS, and only three of them: a place, something that runs, and
/// everything else — plus the broken link, which is the one state that must not
/// look normal. More colours would be a legend nobody reads.
///
/// # And FOCUS wins all of them
///
/// Because a cursor nobody can see is indistinguishable from a key that does
/// not work. That is not a guess: the arrows moved the cursor, every test
/// agreed, and the screen showed nothing — so the report was "the keyboard does
/// not work in the file manager", and the keyboard was fine.
///
/// The focus takes the TEXT's colour and the mark takes a glyph in the gutter,
/// which is the split the rest of the footer already uses (invariant 15: never
/// a box, never an outline).
fn colour_of(entry: &Entry, focused: bool) -> Color {
    if focused {
        return ronoctua_tokens::color::STATE_ACCENT;
    }
    if entry.broken() {
        return ronoctua_tokens::color::STATE_ERROR;
    }
    match entry.opens_as() {
        Kind::Directory => ronoctua_tokens::color::TEXT_PRIMARY,
        _ if entry.executable() => ronoctua_tokens::color::STATE_SUCCESS,
        _ => ronoctua_tokens::color::TEXT_SECONDARY,
    }
}

/// A size, already assembled.
///
/// Assembled here rather than through the bundle because a listing draws two
/// hundred of these per frame and the language only decides the separator —
/// which the caller passes in when it has the bundle. The plain form is what
/// the tests and the window use.
pub(crate) fn size_text(bytes: u64) -> String {
    let h = size::human(bytes);
    if matches!(h.unit, size::Unit::Bytes) {
        format!("{} B", h.whole)
    } else {
        let unit = match h.unit {
            size::Unit::Kibi => "KiB",
            size::Unit::Mebi => "MiB",
            size::Unit::Gibi => "GiB",
            size::Unit::Tebi => "TiB",
            size::Unit::Bytes => "B",
        };
        format!("{},{} {unit}", h.whole, h.tenth)
    }
}

/// The sidebar the manager comes up with — the SAME one in both doors.
///
/// Its defaults come from `RONOCTUA_FILES_SIDEBAR`, which `nix/modules/files.nix`
/// resolves out of `nix/files-sidebar.nix` and `nix/user-dirs.nix` and the
/// nested loop exports from the same two files. Names relative to home, never
/// paths: a path here would name somebody's machine (invariant 13).
///
/// Whatever the person then changed lives in
/// `$XDG_CONFIG_HOME/ronoctua/files-sidebar.json` and wins over the list. A
/// directory that is not on the machine is dropped in silence, which is what
/// `listing::start_at` already does with the start directory.
pub fn sidebar_of(
    home: Option<&std::path::Path>,
    bin: Option<&ronoctua_files::trash::Bin>,
    store: Option<PathBuf>,
) -> ronoctua_files::sidebar::Sidebar {
    let declared: Vec<String> = std::env::var("RONOCTUA_FILES_SIDEBAR")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let prefs = store
        .as_deref()
        .map_or_else(ronoctua_files::sidebar::Preferences::fresh, |d| {
            ronoctua_files::sidebar::store::read_from(d)
        });
    ronoctua_files::sidebar::Sidebar::of(
        &declared,
        home,
        bin.map(ronoctua_files::trash::Bin::files).as_deref(),
        prefs,
        store,
    )
}

/// The view's own keys, as DATA.
///
/// Declared here and not hidden in the `match` below, for the reason
/// `leader::OF_CLIPBOARD_VIEW` gives: the list is what the manual and the check
/// read. A key that only exists inside a `match` arm is a key nothing can
/// enumerate — and this table is one half of `files-everything-has-a-key`.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    // MOVING: the arrows, and the Emacs pair the keymap crate calls canonical.
    ("<down>", "files-down"),
    ("C-n", "files-down"),
    ("<up>", "files-up"),
    ("C-p", "files-up"),
    ("RET", "files-enter"),
    ("<right>", "files-enter"),
    ("C-f", "files-enter"),
    ("<left>", "files-parent"),
    ("C-b", "files-parent"),
    ("C-<", "files-first"),
    ("C->", "files-last"),
    // THE VIEW'S OWN COMMANDS, all under `C-`. NEVER UNDER `M-`.
    //
    // They were bare letters, and a bare letter is now a letter of the query.
    // The first attempt moved them to `M-`, and that was wrong for a reason
    // that has nothing to do with this view: **the compositor sees the key
    // first**. In the nested loop the leader's chord family IS `Alt` — the
    // config says so in as many words — so `M-RET` never reached the footer at
    // all, it opened a terminal. Reported from the screen, exactly like that.
    //
    // `Ctrl` is the family nothing above us claims: the leader is a sequence,
    // the chords are `Mod+…`, and the emergency binds are `Ctrl+Alt+…`. The
    // check `files-keys-are-not-the-compositors` is what keeps it that way.
    ("C-h", "files-hidden"),
    ("C-s", "files-sort"),
    ("C-S", "files-sort-reverse"),
    ("C-SPC", "files-mark"),
    ("C-a", "files-mark-all"),
    ("C-A", "files-unmark-all"),
    ("C-m", "files-menu"),
    // THE SIDEBAR, and its four keys are LETTERS on purpose.
    //
    // The obvious choice for crossing panes was `C-<left>`, and it would have
    // worked SOMETIMES. `Footer::press_key` intercepts an arrow while the RIGHT
    // `Ctrl` is held and sends it to `navigate` — paging — BEFORE the view sees
    // the key. With the left Ctrl it would cross; with the right it would page.
    // Worse, the window door does not track the right Ctrl at all, so the two
    // doors would disagree — which is the divergence the whole two-door
    // arrangement exists to prevent, and it would be reported as "the shortcut
    // does not work", one of the three causes with the same screen.
    ("C-l", "files-sidebar"),
    ("C-o", "files-sidebar-focus"),
    ("C-e", "files-eject"),
    ("C-u", "files-unpin"),
    // TAB IS NOT A LETTER, so it stays where it was.
    ("TAB", "files-measure"),
];

/// The hint line: what typing does, and how to reach the menu without a mouse.
///
/// # Why the key is INTERPOLATED and not written into the text
///
/// Because it is a key, and keys move. It moved once already in this file — the
/// menu was `,` until a bare comma became a letter of the query — and a hint
/// that named the old one would be worse than no hint: it would be a sentence
/// on screen telling you to press something that types a comma.
///
/// The menu is the one thing that has no other way in. Every other command in
/// the view is a chord one can look up in the manual; the menu is where the
/// twenty actions live, and until this line existed the only way to find its
/// key was to read `OF_THE_VIEW`.
/// The hint line, saying how to reach the sidebar when there is one.
///
/// # Why a second key and not one sentence with an empty half
///
/// Because the difference is "is there a sidebar on screen", not a difference
/// of language — a Fluent selector would put a UI decision in the `.ftl` and
/// still need a flag to select on. Two keys, and the caller knows which.
///
/// **This line is the only thing that teaches the crossing key.** `C-o` is not
/// a command of the registry, so it is in neither `M-x` nor the manual; it was
/// implemented, it worked, and nothing on screen said it existed — which is
/// the same as not having it.
#[must_use]
pub fn hint_of(texts: &ronoctua_i18n::Texts, sidebar: bool) -> String {
    let menu = key_that_runs("files-menu").map_or_else(String::new, |t| t.notation());
    if !sidebar {
        return texts.t_with(ronoctua_i18n::key::FILES_HINT, &[("key", menu.as_str())]);
    }
    let side = key_that_runs("files-sidebar-focus").map_or_else(String::new, |t| t.notation());
    texts.t_with(
        ronoctua_i18n::key::FILES_HINT_SIDEBAR,
        &[("key", menu.as_str()), ("side", side.as_str())],
    )
}

/// The KEY that runs a given id, read out of the same table.
///
/// # Why this exists
///
/// Because the right button opens the menu by PRESSING ITS KEY — the discipline
/// that keeps the mouse from having a path of its own — and both doors had the
/// key written out by hand, as `Char(',')`. The day the bare letters became
/// query text, `,` became a comma in the filter and the right button quietly
/// started typing punctuation instead of opening the menu. Nothing failed; the
/// menu simply stopped appearing.
///
/// Reading the notation back out of `OF_THE_VIEW` means a rebinding moves the
/// mouse with it, which is the only arrangement in which the two cannot drift.
#[must_use]
pub fn key_that_runs(id: &str) -> Option<Key> {
    OF_THE_VIEW
        .iter()
        .find(|(_, command)| *command == id)
        .and_then(|(notation, _)| Key::parse(notation).ok())
}

/// The id a key runs, if the table has one for it.
///
/// The table DISPATCHES; it is not a comment about what the `match` below
/// does. That distinction is the whole reason `leader::OF_CLIPBOARD_VIEW`
/// exists in the shape it does: a key handled in a `match` and missing from the
/// table is a key nothing can enumerate — not the manual, not the check — and a
/// key in the table that no arm answers is a line promising something that does
/// not happen. With one of them driving the other, neither can happen.
#[must_use]
pub fn of_the_view(t: Key) -> Option<&'static str> {
    let notation = notation_of(t);
    OF_THE_VIEW
        .iter()
        .find(|(k, _)| *k == notation)
        .map(|(_, id)| *id)
}

impl View {
    /// One key.
    ///
    /// The menu, when open, takes the key first: it is modal, and a key that
    /// fell through to the listing underneath would move a cursor the person
    /// cannot see.
    pub fn key(&mut self, t: Key) -> Step {
        // THE PICKER FIRST. It is the innermost thing on screen, and a key that
        // fell past it would move a cursor nobody can see while a question is
        // being answered.
        if self.picking.is_some() {
            return self.picker_key(t);
        }
        // THE DETAIL SHEET IS READ, not acted on. Every key closes it, which is
        // what one wants from something opened to look at: no gesture is lost
        // waiting for the right one.
        if self.detail.is_some() {
            self.detail = None;
            return Step::Changed;
        }
        if self.menu.is_some() {
            return self.menu_key(t);
        }
        // THE TABLE FIRST, and typing after it.
        //
        // The order is what makes both possible: everything the view answers to
        // carries a modifier or is not a letter, so it is asked first and never
        // reaches the query; a bare letter finds nothing in the table and falls
        // through to the filter.
        //
        // ESC IS NOT IN THE TABLE, and that is deliberate: every view in this
        // system closes with it. A line for it would say what the whole system
        // already says. Here it does one thing first: a query on screen is what
        // it clears, and only an empty one closes the view. Anything else would
        // throw away what was typed AND the view in a single press.
        if matches!(t.sym, Sym::Esc) {
            if !self.filter.is_empty() {
                self.filter.clear();
                self.refilter();
                self.clamp();
                return Step::Changed;
            }
            // AND THE SIDEBAR IS THE SECOND LEVEL. `Esc` goes back ONE level,
            // which is what it does everywhere else in this system: with a
            // query it clears the query, with the cursor across it comes back,
            // and only from the listing with nothing typed does it close. Any
            // other order takes away two things for one press.
            if matches!(self.pane, Pane::Sidebar) {
                self.pane = Pane::Listing;
                return Step::Changed;
            }
            return Step::Closed;
        }
        if of_the_view(t).is_none() {
            // TYPING IS NOT A COMMAND, so it has no line in the table — the
            // same shape as the locator and the `M-x`, where what is typed is
            // the query and never a key.
            match crate::minibuffer::gesture_of(t) {
                crate::minibuffer::Gesture::Insert(c) => {
                    self.filter.push(c);
                    self.refilter();
                    self.clamp();
                    return Step::Changed;
                }
                crate::minibuffer::Gesture::Delete => {
                    // WITH NOTHING TYPED, BACKSPACE GOES UP A LEVEL.
                    //
                    // It reads as one gesture with the query living at the end
                    // of the path: you erase what you typed, and then you keep
                    // erasing — and what is behind the query is the directory
                    // you are in. Every browser's address bar behaves this way,
                    // and it is the cheapest way out of a folder entered by
                    // mistake.
                    //
                    // It is NOT "close the view": backspace held down would
                    // otherwise take the view with it, which is the thing one
                    // is least likely to have meant.
                    if self.filter.pop().is_none() {
                        self.up();
                        return Step::Changed;
                    }
                    self.refilter();
                    self.clamp();
                    return Step::Changed;
                }
                // THESE THREE FOLLOW THE PANE, and typing above does NOT.
                //
                // The filter belongs to the listing, and the listing is still
                // on screen: a query that emptied a six-row sidebar would take
                // away the way out of wherever the letter was mistyped. So the
                // rule the whole keyboard was rebuilt around — in a list of
                // names the letter is text — holds in both panes, and only the
                // three gestures that MOVE change hands.
                crate::minibuffer::Gesture::Down => return self.step_by(1),
                crate::minibuffer::Gesture::Up => return self.step_by(-1),
                crate::minibuffer::Gesture::Confirm => return self.open_here(),
                crate::minibuffer::Gesture::Cancel | crate::minibuffer::Gesture::Nothing => {}
            }
        }
        match of_the_view(t) {
            Some("files-down") => self.step_by(1),
            Some("files-up") => self.step_by(-1),
            Some("files-enter") => self.open_here(),
            Some("files-parent") => {
                self.up();
                Step::Changed
            }
            Some("files-hidden") => {
                self.hidden = !self.hidden;
                self.refilter();
                self.clamp();
                Step::Changed
            }
            Some("files-sort") => {
                self.sort.by = self.sort.by.next();
                self.reread();
                Step::Changed
            }
            Some("files-sort-reverse") => {
                self.sort.ascending = !self.sort.ascending;
                self.reread();
                Step::Changed
            }
            Some("files-sidebar") => {
                let showing = self.side.toggle();
                self.save_sidebar();
                // COMING BACK OUT when it is hidden: the cursor cannot stay in
                // a pane that is not drawn, and a key pressed against an
                // invisible cursor is the defect the manager was once reported
                // for with the listing.
                if !showing {
                    self.pane = Pane::Listing;
                }
                Step::Changed
            }
            Some("files-sidebar-focus") => self.cross_over(),
            Some("files-eject") => self.eject_focused(),
            Some("files-unpin") => self.unpin_focused(),
            Some("files-mark") => self.mark_focused(),
            Some("files-mark-all") => {
                for i in &self.shown {
                    self.marks.insert(self.entries[*i].name.clone());
                }
                Step::Changed
            }
            Some("files-unmark-all") => {
                self.marks.clear();
                Step::Changed
            }
            Some("files-first") => {
                self.cursor = 0;
                Step::Changed
            }
            Some("files-last") => {
                self.cursor = self.shown.len().saturating_sub(1);
                Step::Changed
            }
            Some("files-menu") => {
                self.arming = None;
                self.open_menu()
            }
            Some("files-measure") => self.measure_focused(),
            _ => Step::Nothing,
        }
    }

    /// A key while a question is on screen.
    ///
    /// The same gestures as the locator, through the same `gesture_of`: typing
    /// narrows, the arrows move, `Enter` confirms, `Esc` gives up. Sharing the
    /// function is what keeps the two from drifting — muscle memory does not
    /// care which of them is on screen.
    fn picker_key(&mut self, t: Key) -> Step {
        use crate::minibuffer::Gesture;
        let Some(picker) = self.picking.as_mut() else {
            return Step::Nothing;
        };
        match crate::minibuffer::gesture_of(t) {
            Gesture::Cancel => {
                self.picking = None;
                Step::Changed
            }
            Gesture::Insert(c) => {
                picker.query.push(c);
                picker.cursor = 0;
                Step::Changed
            }
            Gesture::Delete => {
                picker.query.pop();
                picker.cursor = 0;
                Step::Changed
            }
            Gesture::Down => {
                let last = picker.shown().len().saturating_sub(1);
                picker.cursor = (picker.cursor + 1).min(last);
                Step::Changed
            }
            Gesture::Up => {
                picker.cursor = picker.cursor.saturating_sub(1);
                Step::Changed
            }
            Gesture::Confirm => self.answered(),
            Gesture::Nothing => Step::Nothing,
        }
    }

    /// Opens a question.
    ///
    /// Every one of them is a LIST except the rename, whose answer is not on
    /// any list — and that one is marked `typed`, so the query itself is the
    /// answer.
    fn ask(&mut self, asking: Asking) -> Step {
        let (choices, typed) = match &asking {
            // BUILT BY `ask_hidden`, which already has the list — and which
            // refuses to open at all when there is nothing hidden, because a
            // question with no answers is a screen one has to press `Esc` to
            // get out of.
            Asking::Sidebar => (Vec::new(), false),
            Asking::Owner(_) => (
                self.roster
                    .people()
                    .map(|(id, name)| Choice {
                        id: id.to_string(),
                        label: name.to_owned(),
                    })
                    .collect(),
                false,
            ),
            // ONLY THE GROUPS THIS SESSION IS IN. Offering the rest would be
            // offering rows the kernel refuses: the list would look as if it
            // worked and the file would not change.
            Asking::Group(_) => (
                self.roster
                    .mine()
                    .into_iter()
                    .map(|(id, label)| Choice {
                        id: id.to_string(),
                        label,
                    })
                    .collect(),
                false,
            ),
            Asking::Mode(_, _) => (
                ronoctua_files::mode::PRESETS
                    .iter()
                    .map(|(octal, label)| Choice {
                        id: format!("{octal:o}"),
                        // RESOLVED HERE. It used to be the KEY, "resolved by
                        // whoever draws" — and nobody drew it: `picker_rows`
                        // wrote `label` verbatim, so `files-preset-file` went
                        // to the screen, and `Picker::shown` filtered on it.
                        label: self.texts.t(label),
                    })
                    .collect(),
                false,
            ),
            Asking::Format(_) => (
                ronoctua_files::act::FORMATS
                    .iter()
                    .map(|(extension, label)| Choice {
                        id: (*extension).to_owned(),
                        label: self.texts.t(label),
                    })
                    .collect(),
                false,
            ),
            Asking::Application(path) => {
                let of_type = ronoctua_files::mime::of_path(path, Kind::File, true);
                (
                    self.catalogue
                        .iter()
                        .filter(|e| {
                            e.mime_types
                                .iter()
                                .any(|m| ronoctua_files::mime::is_a(of_type, m))
                        })
                        .map(|e| Choice {
                            id: e.id.clone(),
                            label: e.name.clone(),
                        })
                        .collect(),
                    false,
                )
            }
            // The one question whose answer is not on any list.
            Asking::Name(_) => (Vec::new(), true),
            // YES AND NO, and `no` FIRST.
            //
            // The cursor starts on the first row, so the first row is what a
            // stray `Enter` chooses. On a question that exists because the
            // action is not obviously reversible, that row must be the one that
            // does nothing.
            Asking::Confirm(_, _) => (
                vec![
                    Choice {
                        id: "no".to_owned(),
                        label: self.texts.t(ronoctua_i18n::key::FILES_NO),
                    },
                    Choice {
                        id: "yes".to_owned(),
                        label: self.texts.t(ronoctua_i18n::key::FILES_YES),
                    },
                ],
                false,
            ),
        };
        // A question with nothing to answer it is not a question. Saying so is
        // better than a list with one empty line that swallows the keyboard.
        if choices.is_empty() && !typed {
            return Step::Say(Explanation {
                key: ronoctua_i18n::key::FILES_NOTHING_TO_CHOOSE,
                args: Vec::new(),
            });
        }
        let query = match &asking {
            // A rename starts with the CURRENT name, so the common case —
            // changing an extension, fixing a letter — is not retyping.
            Asking::Name(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.picking = Some(Picker {
            asking,
            choices,
            query,
            cursor: 0,
            typed,
        });
        Step::Changed
    }

    /// Asks before changing a mode, with the change SPELLED OUT.
    ///
    /// One function for both ways in — the preset list and the click on a
    /// letter — because a confirmation that covers one of the two is not a
    /// confirmation, and the one it missed was the one people use.
    fn confirm_mode(&mut self, paths: Vec<PathBuf>, mode: u32) -> Step {
        // WHAT it is about: the name when it is one, the count when it is many.
        // "3 itens" and not the first of three: acting on a set and naming one
        // of them is how a batch operation gets mistaken for a single one.
        let what = if let [only] = paths.as_slice() {
            only.file_name().map_or_else(
                || only.to_string_lossy().into_owned(),
                |n| n.to_string_lossy().into_owned(),
            )
        } else {
            self.texts.t_with(
                ronoctua_i18n::key::FILES_ITEMS,
                &[("n", &paths.len().to_string())],
            )
        };
        // THE MODE AS ONE READS IT — `0644`, four digits, the form every
        // `chmod` in the world takes. `rw-r--r--` would be the other honest
        // answer, and it is the one already on the row being pointed at.
        let sentence = self.texts.t_with(
            ronoctua_i18n::key::FILES_CONFIRM_MODE,
            &[("what", &what), ("mode", &format!("{:04o}", mode & 0o7777))],
        );
        self.ask(Asking::Confirm(
            Box::new(Doing::Mode(paths, mode)),
            sentence,
        ))
    }

    /// What confirming a question produces.
    fn answered(&mut self) -> Step {
        let Some(picker) = self.picking.take() else {
            return Step::Nothing;
        };
        let Some(chosen) = picker.answer() else {
            return Step::Changed;
        };
        match picker.asking {
            Asking::Sidebar => {
                self.side.unhide(&chosen);
                self.save_sidebar();
                self.clamp_side();
                Step::Changed
            }
            Asking::Owner(paths) => chosen
                .parse::<u32>()
                .map_or(Step::Changed, |uid| Step::Do(Doing::Owner(paths, uid))),
            Asking::Group(paths) => chosen
                .parse::<u32>()
                .map_or(Step::Changed, |gid| Step::Do(Doing::Group(paths, gid))),
            // THE PRESET DOES NOT APPLY YET: it asks first. See
            // `Asking::Confirm`.
            Asking::Mode(paths, _) => u32::from_str_radix(&chosen, 8)
                .map_or(Step::Changed, |mode| self.confirm_mode(paths, mode)),
            Asking::Confirm(what, _) => {
                if chosen == "yes" {
                    Step::Do(*what)
                } else {
                    Step::Changed
                }
            }
            Asking::Format(paths) => ronoctua_files::act::FORMATS
                .iter()
                .find(|(extension, _)| *extension == chosen)
                .map_or(Step::Changed, |(extension, _)| {
                    Step::Do(Doing::Compress(paths, extension))
                }),
            Asking::Application(path) => Step::Do(Doing::OpenWith(path, chosen)),
            // A rename is one syscall and answers now: the listing has to show
            // the new name before the hand leaves the key.
            Asking::Name(path) => {
                let to = path.with_file_name(&chosen);
                match ronoctua_files::act::rename(&path, &to) {
                    Ok(()) => {
                        self.reread();
                        Step::Changed
                    }
                    Err(e) => Step::Say(Explanation {
                        key: ronoctua_i18n::key::FILES_REFUSED,
                        args: vec![("why", e.to_string())],
                    }),
                }
            }
        }
    }

    /// A key while the menu is open.
    fn menu_key(&mut self, t: Key) -> Step {
        let Some(chosen) = self.menu else {
            return Step::Nothing;
        };
        // THE PANE DECIDES WHICH TABLE, exactly as it decides which rows are
        // drawn. Two tables consulted from one `match` on the key is how `d`
        // comes to mean "delete" over a disk.
        if matches!(self.pane, Pane::Sidebar) {
            return self.side_menu_key(t, chosen);
        }
        let offered = self.offered_now();
        match t.sym {
            Sym::Esc => {
                self.menu = None;
                self.arming = None;
                return Step::Changed;
            }
            Sym::Down => {
                self.menu = Some((chosen + 1).min(offered.len().saturating_sub(1)));
                self.arming = None;
                return Step::Changed;
            }
            Sym::Up => {
                self.menu = Some(chosen.saturating_sub(1));
                self.arming = None;
                return Step::Changed;
            }
            Sym::Enter => {
                if let Some(a) = offered.get(chosen) {
                    return self.run(a);
                }
                return Step::Nothing;
            }
            _ => {}
        }
        let typed = notation_of(t);
        if let Some(a) = offered.iter().find(|a| a.key == typed) {
            return self.run(a);
        }
        Step::Nothing
    }

    /// One key, with the SIDEBAR's menu open.
    ///
    /// The same four gestures as the listing's — `Esc`, the arrows, `Enter` —
    /// and then the row's own letter, read from the sidebar's own table. Not
    /// one function branching on the table: the two have different ids and
    /// different `Applies`, and a single body would have to carry both.
    fn side_menu_key(&mut self, t: Key, chosen: usize) -> Step {
        let Some(row) = self.focused_side() else {
            self.menu = None;
            return Step::Changed;
        };
        let offered = ronoctua_files::sidebar_menu::for_row(&row);
        match t.sym {
            Sym::Esc => {
                self.menu = None;
                return Step::Changed;
            }
            Sym::Down => {
                self.menu = Some((chosen + 1).min(offered.len().saturating_sub(1)));
                return Step::Changed;
            }
            Sym::Up => {
                self.menu = Some(chosen.saturating_sub(1));
                return Step::Changed;
            }
            Sym::Enter => {
                return offered
                    .get(chosen)
                    .copied()
                    .map_or(Step::Nothing, |a| self.run_side(a));
            }
            _ => {}
        }
        let typed = notation_of(t);
        offered
            .iter()
            .find(|a| a.key == typed)
            .copied()
            .map_or(Step::Nothing, |a| self.run_side(a))
    }

    /// A click on a cell of a row.
    ///
    /// Every branch here goes through the SAME code the key goes through. That
    /// is not tidiness: it is what keeps the two from drifting, which is how a
    /// file manager ends up with a mouse gesture that does something no key
    /// does — and then somebody has to reach for the mouse.
    pub fn click(&mut self, row: usize, hit: &Hit) -> Step {
        match hit {
            Hit::Nothing => Step::Nothing,
            // THE MOUSE GOES THROUGH THE SAME CODE THE KEY GOES THROUGH: it
            // puts the cursor where it clicked and then presses the key. That
            // is what keeps `files-everything-has-a-key` honest — the mouse
            // never reaches anything the keyboard cannot.
            Hit::Sidebar(n) => {
                self.pane = Pane::Sidebar;
                self.side_cursor = *n;
                self.open_side()
            }
            Hit::Eject(n) => {
                self.pane = Pane::Sidebar;
                self.side_cursor = *n;
                self.eject_focused()
            }
            Hit::Choice(n) => {
                if let Some(p) = self.picking.as_mut() {
                    p.cursor = *n;
                }
                // THROUGH THE SAME `Enter` as the keyboard, for the reason the
                // locator already gives: an answer reached by the mouse and an
                // answer reached by the key have to be the same answer.
                self.answered()
            }
            Hit::Sort(column) => {
                let wanted = column.sorts_by();
                // CLICKING THE COLUMN THAT IS ALREADY SORTING REVERSES IT,
                // which is what every table does and what people try first.
                if self.sort.by == wanted {
                    self.sort.ascending = !self.sort.ascending;
                } else {
                    self.sort.by = wanted;
                    self.sort.ascending = true;
                }
                self.reread();
                Step::Changed
            }
            Hit::SideMenu(id) => {
                let offered = self
                    .focused_side()
                    .map(|r| ronoctua_files::sidebar_menu::for_row(&r));
                offered
                    .and_then(|o| o.iter().find(|a| a.id == *id).copied())
                    .map_or(Step::Nothing, |a| self.run_side(a))
            }
            Hit::Menu(id) => {
                let offered = self.offered_now();
                offered
                    .iter()
                    .find(|a| a.id == *id)
                    .map_or(Step::Nothing, |a| self.run(a))
            }
            Hit::Name(n) => {
                self.cursor = *n;
                self.open_focused()
            }
            Hit::Mark(n) => {
                self.cursor = *n;
                self.mark_focused()
            }
            Hit::Measure(n) => {
                self.cursor = *n;
                self.measure_focused()
            }
            Hit::Owner(n) => {
                self.cursor = *n;
                match self.focused_path() {
                    Some(p) => self.ask(Asking::Owner(vec![p])),
                    None => Step::Nothing,
                }
            }
            Hit::Group(n) => {
                self.cursor = *n;
                match self.focused_path() {
                    Some(p) => self.ask(Asking::Group(vec![p])),
                    None => Step::Nothing,
                }
            }
            Hit::Bit(n, who, what) => {
                self.cursor = *n;
                let Some(entry) = self.focused() else {
                    return Step::Nothing;
                };
                let wanted = mode::toggled(entry.mode, *who, *what);
                // A CLICK IS ABOUT THE ROW IT LANDED ON, marks or no marks:
                // pointing at one bit of one row and changing six files is not
                // what anybody means by that gesture.
                //
                // AND IT ASKS FIRST, like the preset does. This path did not,
                // and it is the one people actually use — reported from the
                // screen: *"continuo conseguindo clicar nas letras rwx e as
                // permissões se alteram imediatamente"*. A confirmation that
                // covers one of the two ways in is not a confirmation; it is a
                // speed bump on the road nobody takes.
                self.focused_path()
                    .map_or(Step::Nothing, |p| self.confirm_mode(vec![p], wanted))
            }
            Hit::Special(n, special) => {
                self.cursor = *n;
                let Some(entry) = self.focused() else {
                    return Step::Nothing;
                };
                let wanted = mode::toggled_special(entry.mode, *special);
                // The same question as the ordinary bits: setuid is if anything
                // the one you least want to flip by pointing at it.
                self.focused_path()
                    .map_or(Step::Nothing, |p| self.confirm_mode(vec![p], wanted))
            }
            Hit::Crumb(n) => {
                let _ = row;
                match self.crumb_target(*n) {
                    Some(target) => {
                        self.enter(&target);
                        Step::Changed
                    }
                    None => Step::Nothing,
                }
            }
        }
    }

    /// What the pointer resting on a field says, if anything.
    #[must_use]
    pub fn explanation(
        &self,
        row: usize,
        cell: u16,
        width: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Explanation> {
        self.rows(width, texts)
            .get(row)
            .and_then(|r| r.field_at(cell))
            .and_then(|f| f.explain.clone())
    }

    /// The rows the menu would offer right now.
    fn offered_now(&self) -> Vec<&'static Action> {
        let Some((entry, of_type)) = self.menu_subject() else {
            return Vec::new();
        };
        menu::for_entry(
            &entry,
            of_type,
            !self.clip.paths.is_empty(),
            self.in_trash(),
        )
    }

    /// What the menu is ABOUT: the focused entry, or the directory itself.
    ///
    /// # Why an empty directory still has a menu
    ///
    /// Because "there is nothing here" is not the same as "there is nothing to
    /// do". In an empty folder the useful actions are precisely the ones about
    /// the folder — paste what was copied, open a terminal in it — and the
    /// right button answered with nothing at all. Reported from the screen.
    ///
    /// The subject is then an `Entry` for the directory one is IN, read from
    /// disk like any other. Synthesising it here rather than teaching the menu
    /// about a second kind of subject is what keeps `menu::for_entry` a single
    /// function with a single rule.
    fn menu_subject(&self) -> Option<(Entry, &'static str)> {
        if let Some(entry) = self.focused() {
            let of_type = self
                .shown
                .get(self.cursor)
                .and_then(|i| self.types.get(*i))
                .copied()
                .unwrap_or(mime::UNKNOWN);
            return Some((entry.clone(), of_type));
        }
        let name = self.where_.file_name().map_or_else(
            || self.where_.to_string_lossy().into_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let entry = Entry::of_path(name, &self.where_)?;
        let of_type = mime::of_path(&self.where_, entry.kind, false);
        Some((entry, of_type))
    }

    /// Whether the directory being looked at IS the trash's `files/`.
    ///
    /// By PATH and not by a flag, so navigating into the trash — which is how
    /// one finds something to put back — offers `restore` without any state
    /// having been set.
    fn in_trash(&self) -> bool {
        self.bin
            .as_ref()
            .is_some_and(|bin| self.where_ == bin.files())
    }

    /// Carries out one menu row.
    fn run(&mut self, a: &'static Action) -> Step {
        // THE SAME KEY TWICE, for what cannot be undone. The first press arms
        // and says so on the row; the second acts. A separate yes/no screen
        // would be a second surface (invariant 14), and a dialog nobody reads
        // is what a confirmation becomes when it is easy to dismiss.
        if a.confirms && self.arming != Some(a.id) {
            self.arming = Some(a.id);
            return Step::Changed;
        }
        self.arming = None;
        self.menu = None;
        // TWO KINDS OF ROW, and the difference is not a detail.
        //
        // `chosen()` is the MARKS when there are any, and the focused entry
        // otherwise — that is the rule every file manager follows, and the one
        // that surprises when it is not followed. But some rows have no plural:
        // renaming six files to one name is meaningless, and "open with" opens
        // one thing. Those take the FOCUSED entry, always, whatever is marked.
        let paths = self.chosen();
        let Some(focused) = self.focused_path() else {
            return Step::Changed;
        };
        let here = self.where_.clone();
        match a.id {
            "open" => self.open_focused(),
            "open-with" => self.ask(Asking::Application(focused)),
            "pin" => self.pin(&paths),
            "unhide" => self.ask_hidden(),
            // "HERE" IS THE FOCUSED DIRECTORY when one is focused, and the
            // current one otherwise — which is what every file manager does and
            // what right-clicking a folder means. Always using the current
            // directory would make "terminal here" open one level above the
            // thing being pointed at.
            "terminal" => Step::Do(Doing::Terminal(self.inside().unwrap_or(here))),
            "root-terminal" => Step::Do(Doing::RootTerminal(self.inside().unwrap_or(here))),
            "copy-path" => Step::Do(Doing::Copy(
                paths
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
            "copy" => {
                self.clip = Clip {
                    paths,
                    moves: false,
                };
                Step::Changed
            }
            "cut" => {
                self.clip = Clip { paths, moves: true };
                Step::Changed
            }
            "paste" => Step::Do(Doing::Paste(here)),
            "rename" => self.ask(Asking::Name(focused)),
            "info" => self.inspect(),
            "mode" => {
                // The focused entry's mode is what the presets are compared
                // against; the change lands on everything chosen. With one
                // entry the two are the same thing, which is why the mismatch
                // went unseen.
                let current = self.focused().map_or(0o644, |e| e.mode);
                self.ask(Asking::Mode(paths, current))
            }
            "owner" => self.ask(Asking::Owner(paths)),
            "group" => self.ask(Asking::Group(paths)),
            "compress" => self.ask(Asking::Format(paths)),
            "extract" | "extract-tar" => Step::Do(Doing::Extract(focused)),
            "trash" => match &self.bin {
                Some(bin) => Step::Do(Doing::Trash(bin.root.clone(), paths)),
                // NO BIN, NO TRASH — and it says so rather than falling back to
                // deleting, which is the fallback that would cost somebody a
                // file they thought was recoverable.
                None => Step::Say(Explanation {
                    key: ronoctua_i18n::key::FILES_NO_TRASH,
                    args: Vec::new(),
                }),
            },
            "restore" => match &self.bin {
                Some(bin) => Step::Do(Doing::Restore(
                    bin.root.clone(),
                    paths
                        .iter()
                        .filter_map(|p| p.file_name())
                        .map(|n| n.to_string_lossy().into_owned())
                        .collect(),
                )),
                None => Step::Changed,
            },
            "delete" => Step::Do(Doing::Delete(paths)),
            "shred" => Step::Do(Doing::Shred(paths)),
            // A row in the table with no arm here would be a menu line that
            // does nothing — the exact defect `WITH_ACTION` exists to refuse in
            // the leader, and `files-everything-has-a-key` refuses it here.
            _ => Step::Nothing,
        }
    }

    /// Moves the cursor of whichever pane has the keyboard.
    fn step_by(&mut self, by: i32) -> Step {
        match self.pane {
            Pane::Listing => self.move_by(by),
            Pane::Sidebar => self.side_by(by),
        }
    }

    /// Opens whatever the cursor is on, in whichever pane has it.
    fn open_here(&mut self) -> Step {
        match self.pane {
            Pane::Listing => self.open_focused(),
            Pane::Sidebar => self.open_side(),
        }
    }

    /// Crosses into the sidebar, and back out.
    ///
    /// ONE key that toggles, and not two. Two — one in, one out — would leave
    /// the "in" key doing nothing when it is already in, and
    /// `every_view_key_in_the_table_actually_does_something` refuses a key that
    /// answers `Step::Nothing` from the state it is pressed in. A toggle always
    /// does something, which is also what one wants from a key one presses
    /// without looking.
    fn cross_over(&mut self) -> Step {
        match self.pane {
            Pane::Listing => {
                // NOTHING TO CROSS INTO is not silence: a key that did nothing
                // and said nothing is indistinguishable from a key that never
                // arrived, which is the report this project has had three times
                // with three different causes.
                if self.side.how_many(self.hidden) == 0 {
                    return Step::Say(mode::Explanation {
                        key: ronoctua_i18n::key::FILES_SIDEBAR_ONLY,
                        args: vec![("key", self.notation_for("files-sidebar"))],
                    });
                }
                self.pane = Pane::Sidebar;
                self.clamp_side();
            }
            Pane::Sidebar => self.pane = Pane::Listing,
        }
        // The MENU is about the listing's focused entry, and an armed action
        // is about the row it was armed on. Crossing panes leaves both behind.
        self.arming = None;
        Step::Changed
    }

    /// The cursor inside the sidebar.
    ///
    /// IT STEPS OVER THE GAPS, and crossing one costs nothing: from the last
    /// place, one press of the arrow lands on the first device rather than in
    /// the blank between them. A cursor that stopped on a separator would be an
    /// arrow press that appears to do nothing, which is the same complaint as a
    /// key that never arrived.
    fn side_by(&mut self, by: i32) -> Step {
        let lines = self.side.shown(self.hidden);
        if lines.iter().all(Option::is_none) {
            return Step::Nothing;
        }
        let step = i64::from(if by < 0 { -1i32 } else { 1i32 });
        let mut at = i64::from(i32::try_from(self.side_cursor).unwrap_or(0));
        let mut left = by.abs();
        while left > 0 {
            let next = at + step;
            if next < 0 || usize::try_from(next).is_ok_and(|n| n >= lines.len()) {
                break;
            }
            at = next;
            if usize::try_from(at).is_ok_and(|n| lines[n].is_some()) {
                left -= 1;
            }
        }
        let new = usize::try_from(at).unwrap_or(0);
        // Landing on a gap means there was nothing past it in that direction.
        if new == self.side_cursor || lines.get(new).is_none_or(Option::is_none) {
            return Step::Nothing;
        }
        self.side_cursor = new;
        Step::Changed
    }

    /// Keeps the sidebar's cursor inside the sidebar.
    ///
    /// A device unplugged, a row hidden, `C-h` pressed — any of them shortens
    /// the list under a cursor that was already past the new end.
    fn clamp_side(&mut self) {
        let lines = self.side.shown(self.hidden);
        self.side_cursor = self.side_cursor.min(lines.len().saturating_sub(1));
        // AND OFF A GAP. Shortening the list can leave the cursor on a blank
        // line, where every key would answer "nothing" for a reason nobody can
        // see on screen.
        while self.side_cursor > 0 && lines.get(self.side_cursor).is_none_or(Option::is_none) {
            self.side_cursor -= 1;
        }
    }

    /// The notation of the key that runs an id, for a sentence that names it.
    ///
    /// Read back out of `OF_THE_VIEW` rather than written into the text, for
    /// the reason [`hint`] gives: a rebinding has to move the sentence with it,
    /// or the screen ends up telling somebody to press something else.
    fn notation_for(&self, id: &str) -> String {
        key_that_runs(id).map_or_else(String::new, |t| t.notation())
    }

    fn move_by(&mut self, by: i32) -> Step {
        if self.shown.is_empty() {
            return Step::Nothing;
        }
        let last = self.shown.len() - 1;
        let wanted = i64::from(i32::try_from(self.cursor).unwrap_or(0) + by);
        let clamped = wanted.clamp(0, i64::try_from(last).unwrap_or(0));
        let new = usize::try_from(clamped).unwrap_or(0);
        if new == self.cursor {
            return Step::Nothing;
        }
        self.cursor = new;
        self.arming = None;
        Step::Changed
    }

    fn mark_focused(&mut self) -> Step {
        let Some(name) = self.focused().map(|e| e.name.clone()) else {
            return Step::Nothing;
        };
        if !self.marks.remove(&name) {
            self.marks.insert(name);
        }
        // The cursor moves on, so marking a run of files is one key repeated —
        // which is what `m` is for in every listing that has it.
        self.move_by(1);
        Step::Changed
    }

    /// Puts these on the sidebar.
    ///
    /// The MARKS when there are any, like every other row of the menu: six
    /// folders marked put six places there in one press.
    fn pin(&mut self, paths: &[PathBuf]) -> Step {
        use ronoctua_files::sidebar::{Item, Target};
        let mut put = 0usize;
        let mut last = String::new();
        for path in paths {
            let directory = path.is_dir();
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            // THE ID CARRIES THE PATH, so pinning the same folder twice is
            // refused by `add` rather than making a second identical row.
            let id = if directory {
                format!("place:{}", path.display())
            } else {
                format!("file:{}", path.display())
            };
            let target = if directory {
                Target::Place(path.clone())
            } else {
                Target::File(path.clone())
            };
            if self.side.add(Item {
                id,
                target,
                label: None,
            }) {
                put += 1;
                last = name;
            }
        }
        if put == 0 {
            return Step::Nothing;
        }
        self.save_sidebar();
        Step::Say(mode::Explanation {
            key: ronoctua_i18n::key::FILES_SIDEBAR_PINNED,
            args: vec![(
                "name",
                if put == 1 {
                    last
                } else {
                    // A COUNT and not a list: six names on the message line is
                    // a sentence nobody finishes reading.
                    put.to_string()
                },
            )],
        })
    }

    /// Asks which hidden row to bring back.
    ///
    /// A question and not a key, because a hidden row is not on screen for a
    /// key to be pressed on. It is the same `Picker` as every other question
    /// here: same narrowing, same `Esc`, same `Enter`.
    fn ask_hidden(&mut self) -> Step {
        let hidden = self.side.hidden();
        if hidden.is_empty() {
            return Step::Say(mode::Explanation {
                key: ronoctua_i18n::key::FILES_SIDEBAR_NOTHING_HIDDEN,
                args: Vec::new(),
            });
        }
        self.picking = Some(Picker {
            asking: Asking::Sidebar,
            choices: hidden
                .into_iter()
                .map(|(id, label)| Choice { id, label })
                .collect(),
            query: String::new(),
            cursor: 0,
            typed: false,
        });
        Step::Changed
    }

    /// What the SECONDARY button opens on this line and this cell.
    ///
    /// **The one place that decides**, because the two doors used to decide it
    /// separately and both got it the same way wrong: they resolved the target
    /// from the LINE and never looked at the cell, so a right-click anywhere in
    /// the sidebar's cells opened the listing's menu — about whatever file
    /// happened to be on that line — and moved the listing's cursor to it.
    ///
    /// The primary button never had the defect because it goes through
    /// `Row::field_at` and acts on the `Hit` it finds. This does the same, and
    /// now both buttons answer one question: what is under the pointer?
    pub fn menu_at(&mut self, row: usize, cell: u16, width: u16) -> Step {
        let hit = self
            .rows_from(row, 1, width, &self.texts.clone())
            .first()
            .and_then(|r| r.field_at(cell).map(|f| f.hit.clone()));
        match hit {
            // A SIDEBAR CELL OPENS THE SIDEBAR'S MENU. The eject glyph counts:
            // right-clicking a target is asking what else that row can do.
            Some(Hit::Sidebar(n) | Hit::Eject(n)) => {
                self.pane = Pane::Sidebar;
                self.side_cursor = n;
                self.open_menu()
            }
            // ANY LISTING CELL, and the empty directory — which has no field
            // anywhere and still has a menu, about the directory itself.
            _ => {
                self.pane = Pane::Listing;
                if matches!(self.focus_row(row), Step::Nothing) && !self.is_empty() {
                    return Step::Nothing;
                }
                self.open_menu()
            }
        }
    }

    /// Opens the menu for whichever pane has the cursor.
    fn open_menu(&mut self) -> Step {
        self.menu = Some(0);
        Step::Changed
    }

    /// The sidebar row the cursor is on, cloned so the borrow ends here.
    ///
    /// `None` when the cursor is on a GAP. It cannot get there with the
    /// keyboard — `side_by` steps over them — but it can with a click, because
    /// a blank line is still a line the pointer can land on.
    fn focused_side(&self) -> Option<ronoctua_files::sidebar::Row> {
        self.side
            .shown(self.hidden)
            .get(self.side_cursor)
            .and_then(|r| r.map(Clone::clone))
    }

    /// Runs one row of the sidebar's menu.
    fn run_side(&mut self, a: &'static ronoctua_files::sidebar_menu::Action) -> Step {
        use ronoctua_files::sidebar::Target;
        self.menu = None;
        let Some(row) = self.focused_side() else {
            return Step::Changed;
        };
        match a.id {
            "open" => self.walk_into(&row),
            "mount" => match &row.target {
                // THE ADMIN GUARD STAYS HERE. The row IS offered for a system
                // volume — an unmounted internal partition with no mount row
                // and nothing saying why would be worse — but pressing it does
                // not try: `filesystem-mount-system` is `auth_admin_keep` and
                // this session has no authentication agent, so the call would
                // come back refused. It names what it needs and offers the root
                // terminal instead.
                Target::Device(_) if row.system => Step::Say(self.needs_admin(&row)),
                Target::Device(id) => Step::Do(Doing::Mount(id.clone())),
                _ => Step::Nothing,
            },
            "unmount" => self.eject_focused(),
            "eject" => match &row.target {
                // EJECT IS THE DRIVE'S, and unmount is the volume's. Ejecting
                // without unmounting first is how a stick leaves with a write
                // still in flight, so this asks for both in order and the host
                // does the second only when the first came back.
                Target::Device(id) => Step::Do(Doing::Eject(id.clone())),
                _ => Step::Nothing,
            },
            // ALREADY ON SCREEN. The facts are the top of this very menu, so
            // "information" reopens it rather than building a second surface
            // that says the same thing.
            "information" => self.open_menu(),
            "unpin" => self.unpin_focused(),
            "format" => match &row.target {
                Target::Device(id) => Step::Do(Doing::Format(id.clone())),
                _ => Step::Nothing,
            },
            _ => Step::Nothing,
        }
    }

    /// Goes where a sidebar row leads, without asking anything of the system.
    fn walk_into(&mut self, row: &ronoctua_files::sidebar::Row) -> Step {
        use ronoctua_files::sidebar::Target;
        match (&row.target, &row.at) {
            (Target::File(path), _) => Step::Do(Doing::Open(path.clone())),
            (Target::Application(id), _) => Step::Do(Doing::Launch(id.clone())),
            (Target::Place(path), _) | (_, Some(path)) => {
                let path = path.clone();
                self.enter(&path);
                // BACK TO THE LISTING, because arriving somewhere is the point
                // of pressing the row. Left in the sidebar, the next arrow
                // would move through places rather than through what was just
                // opened.
                self.pane = Pane::Listing;
                Step::Changed
            }
            (Target::Trash, None) => Step::Say(mode::Explanation {
                key: ronoctua_i18n::key::FILES_NO_TRASH,
                args: Vec::new(),
            }),
            (Target::Device(_), None) => Step::Nothing,
        }
    }

    /// What pressing a sidebar row does.
    ///
    /// # Why an unmounted device does NOT get mounted here
    ///
    /// It used to, and it was reported from the screen: *"ao clicar em algum
    /// item da sidebar que não está montado, não deve-se montá-lo
    /// automaticamente"*. Mounting is asking the system for something — it can
    /// take a moment, it can be refused, and it changes what is on the machine.
    /// A row one presses to LOOK at a disk should not do that.
    ///
    /// So an unmounted device opens its menu, which carries the facts at the
    /// top and `mount` among the rows with its key beside it. Everything that
    /// is already a place — a folder, a mounted device, the trash — is walked
    /// into, because that is what pressing a place means.
    fn open_side(&mut self) -> Step {
        use ronoctua_files::sidebar::Target;
        let Some(row) = self.focused_side() else {
            return Step::Nothing;
        };
        if matches!(row.target, Target::Device(_)) && row.mounted == Some(false) {
            return self.open_menu();
        }
        self.walk_into(&row)
    }

    /// Unmounts the device the sidebar's cursor is on.
    fn eject_focused(&mut self) -> Step {
        use ronoctua_files::sidebar::Target;
        // NOT ON THE SIDEBAR AT ALL is a different refusal from ON A ROW THAT
        // HAS NOTHING TO UNMOUNT, and telling somebody standing on a folder to
        // "go to the sidebar" when they are already in it is the sort of
        // sentence that teaches people to stop reading the message line.
        let Some(row) = self
            .focused_side()
            .filter(|_| matches!(self.pane, Pane::Sidebar))
        else {
            return Step::Say(mode::Explanation {
                key: ronoctua_i18n::key::FILES_SIDEBAR_ONLY,
                args: vec![("key", self.notation_for("files-sidebar-focus"))],
            });
        };
        // WHAT THE SYSTEM NEEDS IS REFUSED, and the refusal NAMES what decided.
        // "This cannot be unmounted" is a rule nobody can check; "`/boot` is
        // mounted here" is a fact anybody can go and look at.
        if let Some(why) = &row.needed {
            return Step::Say(mode::Explanation {
                key: ronoctua_i18n::key::FILES_UNMOUNT_NEEDED,
                args: vec![("name", self.name_of_side(&row)), ("why", why.clone())],
            });
        }
        match (&row.target, row.mounted) {
            (Target::Device(id), Some(true)) => Step::Do(Doing::Unmount(id.clone())),
            // A row one cannot unmount: an unmounted device, a folder, the
            // trash. It says WHAT the row is rather than what the key wants.
            _ => Step::Say(self.side_explanation(&row, &self.texts)),
        }
    }

    /// Takes the sidebar's focused row off the sidebar.
    fn unpin_focused(&mut self) -> Step {
        use ronoctua_files::sidebar::Gone;
        let Some(row) = self
            .focused_side()
            .filter(|_| matches!(self.pane, Pane::Sidebar))
        else {
            return Step::Say(mode::Explanation {
                key: ronoctua_i18n::key::FILES_SIDEBAR_ONLY,
                args: vec![("key", self.notation_for("files-sidebar-focus"))],
            });
        };
        let name = self.name_of_side(&row);
        let gone = self.side.drop_row(&row.id);
        self.save_sidebar();
        self.clamp_side();
        match gone {
            // THE TWO SENTENCES DIFFER because the two facts differ. A row the
            // distro ships is hidden and can come back; one somebody added is
            // removed and cannot. Saying "removed" for both would be a promise
            // the next start breaks.
            Gone::Hidden => Step::Say(mode::Explanation {
                key: ronoctua_i18n::key::FILES_SIDEBAR_HIDDEN,
                args: vec![("name", name), ("key", self.notation_for("files-menu"))],
            }),
            Gone::Removed => Step::Say(mode::Explanation {
                key: ronoctua_i18n::key::FILES_SIDEBAR_GONE,
                args: vec![("name", name)],
            }),
            Gone::Missing => Step::Nothing,
        }
    }

    /// What to call a sidebar row on the message line.
    fn name_of_side(&self, row: &ronoctua_files::sidebar::Row) -> String {
        row.label
            .clone()
            .unwrap_or_else(|| self.texts.t(ronoctua_i18n::key::FILES_SIDEBAR_TRASH))
    }

    /// Writes the sidebar to disk, and says so when it cannot.
    ///
    /// A read-only home or a full disk is not a reason for the manager to stop
    /// working — everything here degrades to "it did not stick" — but it IS a
    /// reason to say something: a row pinned that is gone after a restart, with
    /// nothing having said why, is worse than a refusal.
    fn save_sidebar(&mut self) {
        // THROUGH THE SIDEBAR, which was told where to write — or told nowhere.
        // Reaching for `$XDG_CONFIG_HOME` here is what made `cargo test` write
        // into the real configuration of whoever ran it: the tests press the
        // key that hides the sidebar and the key that pins a file, and they
        // left a `/tmp` path pinned and the sidebar switched off. See
        // `sidebar::Sidebar::store`.
        if let Err(e) = self.side.save() {
            tracing::warn!(error = %e, "nao consegui salvar a barra lateral");
        }
        // OUR OWN WRITE MOVES THE STAMP. Without this the next tick sees a file
        // newer than the reading, re-reads it, and throws away the sidebar's
        // cursor — once a second, for ever, after any change.
        self.side_stamp = self.side.stamp();
    }

    /// Has somebody edited the sidebar's file since it was read?
    ///
    /// It is asked wherever `stale()` is, on the same one-second tick, and for
    /// the same reason: the file is written by things this view does not know
    /// about. `LEADER a` pins an application without the manager being open,
    /// the window door is a SEPARATE PROCESS with its own copy, and a person
    /// may edit the file by hand. Without this, a row pinned in one place
    /// appears in the other only after a restart — and nothing says why.
    #[must_use]
    pub fn sidebar_stale(&self) -> bool {
        // A SIDEBAR THAT DOES NOT PERSIST IS NEVER STALE. That is every one a
        // test builds, and without this each of them would re-read on the tick
        // and throw the cursor away.
        self.side.store_dir().is_some() && self.side.stamp() != self.side_stamp
    }

    /// Re-reads it, keeping the devices — they came from the bus, not the file.
    pub fn reread_sidebar(&mut self) {
        let Some(dir) = self.side.store_dir().map(Path::to_path_buf) else {
            return;
        };
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let devices = self.side.device_rows();
        self.side = sidebar_of(home.as_deref(), self.bin.as_ref(), Some(dir));
        self.side.devices_are(devices);
        self.side_stamp = self.side.stamp();
        self.clamp_side();
    }

    /// "This one belongs to the system" — and what to do about it.
    ///
    /// It NAMES the way out rather than only stating the obstacle. The root
    /// terminal is the door this repository already uses for privileged work
    /// (`nix/modules/files.nix`: "whoever needs that opens the root terminal
    /// the menu offers, where what is happening is visible"), and a message
    /// that said only "needs the administrator" would leave somebody looking
    /// for a password prompt that this session has no agent to draw.
    fn needs_admin(&self, row: &ronoctua_files::sidebar::Row) -> mode::Explanation {
        mode::Explanation {
            key: ronoctua_i18n::key::FILES_MOUNT_NEEDS_ADMIN,
            args: vec![
                ("name", self.name_of_side(row)),
                ("key", self.root_terminal_key()),
            ],
        }
    }

    /// The menu key that opens a root terminal, read out of the menu's own
    /// table so a rebinding moves the sentence with it.
    fn root_terminal_key(&self) -> String {
        menu::by_id("root-terminal").map_or_else(String::new, |a| a.key.to_owned())
    }

    /// What to say about a refusal, as a key and its arguments.
    ///
    /// The whole point of carrying the NAME rather than the prose: four of
    /// these are sentences a person can act on, and only the last has nothing
    /// better than udisks' own words.
    #[must_use]
    pub fn refusal_said(&self, which: ronoctua_disks::Refusal, why: &str) -> mode::Explanation {
        use ronoctua_disks::Refusal;
        let name = self
            .focused_side()
            .map_or_else(String::new, |r| self.name_of_side(&r));
        match which {
            Refusal::NoDriver => mode::Explanation {
                key: ronoctua_i18n::key::FILES_MOUNT_NO_DRIVER,
                args: vec![("type", why.to_owned())],
            },
            Refusal::NeedsAdmin => mode::Explanation {
                key: ronoctua_i18n::key::FILES_MOUNT_NEEDS_ADMIN,
                args: vec![("name", name), ("key", self.root_terminal_key())],
            },
            Refusal::Busy => mode::Explanation {
                key: ronoctua_i18n::key::FILES_UNMOUNT_BUSY,
                args: vec![("name", name)],
            },
            // ALREADY MOUNTED IS NOT A REFUSAL TO A PERSON: something else
            // mounted it between the reading and the press, and where they
            // wanted to go is exactly where it now is. The host re-reads.
            Refusal::AlreadyMounted | Refusal::Other => mode::Explanation {
                key: ronoctua_i18n::key::FILES_MOUNT_FAILED,
                args: vec![("name", name), ("why", why.to_owned())],
            },
        }
    }

    /// A volume came up; go into it.
    pub fn mounted(&mut self, at: &Path) {
        self.enter(at);
        self.pane = Pane::Listing;
    }

    /// The machine's disks, as they are now.
    ///
    /// The whole reading replaces the device rows in one go, and the sidebar's
    /// cursor is clamped: a stick unplugged while the cursor was on it would
    /// otherwise leave the cursor past the end of a list that just got shorter.
    pub fn racked(&mut self, rack: &ronoctua_disks::Rack) {
        // EVERY volume that is a row, quiet ones included: whether a quiet row
        // is DRAWN is `C-h`'s answer and it is asked at drawing time, so
        // filtering here would make the key stop working on devices while it
        // went on working on dotfiles.
        let uid = ronoctua_files::who::my_uid();
        let listed = ronoctua_disks::listed(&rack.volumes, uid, true);
        let rows = listed
            .iter()
            .map(|v| {
                let mounted = v.mounted();
                ronoctua_files::sidebar::Row {
                    id: format!("device:{}", v.node),
                    target: ronoctua_files::sidebar::Target::Device(v.at.clone()),
                    // The label or the device node — NEVER the object path,
                    // which carries the drive's serial number (invariant 13).
                    label: Some(v.name().to_owned()),
                    section: ronoctua_files::sidebar::Section::Devices,
                    mounted: Some(mounted),
                    at: v.place().map(PathBuf::from),
                    quiet: v.quiet,
                    system: v.system,
                    // RESOLVED HERE, where the volume is. The sidebar carries
                    // the answer and not the volume, because it does not know
                    // what a volume is — and the guard has to hold even if
                    // something else builds the row.
                    needed: ronoctua_disks::needed(v).map(|why| match why {
                        ronoctua_disks::Needed::At(at) => at,
                        ronoctua_disks::Needed::InFstab => "fstab".to_owned(),
                        ronoctua_disks::Needed::Swap => "swap".to_owned(),
                    }),
                    // FROM THE DRIVE and not from the volume: "can this be
                    // pulled out" is a property of the thing it sits on, and a
                    // volume has no such field. A volume whose drive is gone —
                    // a device-mapper node, a loop — is not removable, which is
                    // the right answer for both.
                    removable: ronoctua_disks::attached_to(v, &rack.volumes)
                        .and_then(|at| rack.drives.iter().find(|d| d.at == at))
                        .is_some_and(|d| d.removable),
                    built_in: true,
                }
            })
            .collect();
        self.side.devices_are(rows);
        self.clamp_side();
    }

    fn open_focused(&mut self) -> Step {
        let Some(entry) = self.focused().cloned() else {
            return Step::Nothing;
        };
        let path = self.where_.join(&entry.name);
        // Opening FOLLOWS a link — that is what a link is for. Everything else
        // in this module acts on the link itself.
        if matches!(entry.opens_as(), Kind::Directory) {
            self.enter(&path);
            return Step::Changed;
        }
        Step::Do(Doing::Open(path))
    }

    fn measure_focused(&mut self) -> Step {
        let Some(entry) = self.focused().cloned() else {
            return Step::Nothing;
        };
        let path = self.where_.join(&entry.name);
        if matches!(entry.opens_as(), Kind::Directory) {
            return Step::Do(Doing::Measure(path));
        }
        // For a file the size is already on screen; what `TAB` adds is what the
        // CONTENT says it is, which is the one place a name that lies gets
        // caught.
        let (named, sniffed) = mime::examined(&path, entry.kind);
        Step::Say(Explanation {
            key: match sniffed {
                Some(found) if found != named => ronoctua_i18n::key::FILES_TYPE_DISAGREES,
                _ => ronoctua_i18n::key::FILES_TYPE_IS,
            },
            args: vec![
                ("named", named.to_owned()),
                ("found", sniffed.unwrap_or(named).to_owned()),
            ],
        })
    }

    /// The detail sheet for the focused entry.
    ///
    /// It replaces the listing while it is open, rather than sitting beside it:
    /// the footer's region is twelve lines and a panel next to a listing would
    /// give six lines to each. `Esc` closes it, and the listing is exactly
    /// where it was.
    fn inspect(&mut self) -> Step {
        let Some(name) = self.focused().map(|e| e.name.clone()) else {
            return Step::Nothing;
        };
        self.detail = Some(name);
        Step::Changed
    }

    /// The detail sheet's rows.
    ///
    /// Everything a listing cannot fit, and one thing it deliberately does not
    /// show: what the CONTENT says the file is, next to what the name claims.
    /// A `.txt` holding a PNG is a real thing, and this is where it is caught.
    fn detail_rows(&self, name: &str, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let Some((i, entry)) = self
            .entries
            .iter()
            .enumerate()
            .find(|(_, e)| e.name == name)
        else {
            return Vec::new();
        };
        let path = self.where_.join(&entry.name);
        let (named, sniffed) = ronoctua_files::mime::examined(&path, entry.kind);
        let owner = self
            .roster
            .person(entry.uid)
            .map_or_else(|| entry.uid.to_string(), ToOwned::to_owned);
        let group = self
            .roster
            .group(entry.gid)
            .map_or_else(|| entry.gid.to_string(), ToOwned::to_owned);
        let c = when::civil(entry.modified, &self.zone);
        let measured = self
            .measured
            .as_ref()
            .filter(|(n, _)| n == name)
            .map(|(_, b)| size_text(*b));

        let mut lines: Vec<(&'static str, String)> = vec![
            (
                ronoctua_i18n::key::FILES_DETAIL_PATH,
                path.to_string_lossy().into_owned(),
            ),
            (ronoctua_i18n::key::FILES_DETAIL_TYPE, {
                match sniffed {
                    Some(found) if found != named => format!("{named} / {found}"),
                    _ => named.to_owned(),
                }
            }),
            (
                ronoctua_i18n::key::FILES_DETAIL_MODE,
                format!(
                    "{} ({})",
                    ronoctua_files::mode::shown(entry.mode),
                    ronoctua_files::mode::octal(entry.mode)
                ),
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_OWNER,
                format!("{owner}:{group}"),
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_SIZE,
                measured.unwrap_or_else(|| size_text(entry.bytes)),
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_MODIFIED,
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}",
                    c.year, c.month, c.day, c.hour, c.minute
                ),
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_LINKS,
                entry.links.to_string(),
            ),
        ];
        // A LINK SAYS WHERE IT GOES, and whether it goes anywhere at all.
        if let Some(target) = &entry.target {
            lines.push((
                if entry.broken() {
                    ronoctua_i18n::key::FILES_DETAIL_BROKEN
                } else {
                    ronoctua_i18n::key::FILES_DETAIL_TARGET
                },
                target.clone(),
            ));
        }
        let _ = i;
        // The label column is as wide as the widest label, so the values line
        // up — a sheet whose values start at seven different columns is a
        // sheet one reads seven times.
        let widest = lines
            .iter()
            .map(|(k, _)| width_of(&texts.t(k)))
            .max()
            .unwrap_or(0);
        lines
            .into_iter()
            .map(|(k, v)| Row {
                fields: vec![
                    Field {
                        at: 0,
                        text: texts.t(k),
                        color: ronoctua_tokens::color::TEXT_DIM,
                        hit: Hit::Nothing,
                        explain: None,
                    },
                    Field {
                        at: widest + 2,
                        text: ronoctua_render::layout::elide_end(
                            &v,
                            width.saturating_sub(widest + 2),
                        ),
                        color: ronoctua_tokens::color::TEXT_PRIMARY,
                        hit: Hit::Nothing,
                        explain: None,
                    },
                ],
                marked: false,
                focused: false,
            })
            .collect()
    }

    /// What a measurement came back with.
    pub fn measured(&mut self, name: &str, bytes: u64) {
        self.measured = Some((name.to_owned(), bytes));
    }

    /// The clipboard, for whoever carries out a paste.
    #[must_use]
    pub const fn clip(&self) -> &Clip {
        &self.clip
    }

    /// Empties the clipboard after a move, so the second paste does not fail
    /// on paths that are no longer there.
    pub fn clip_used(&mut self) {
        if self.clip.moves {
            self.clip = Clip::default();
        }
    }
}

/// A key in the notation the menu table uses.
fn notation_of(t: Key) -> String {
    // THE KEYMAP'S OWN WRITER, and not a second one beside it.
    //
    // This used to be a `match` that spelled the symbols itself and DROPPED the
    // modifiers, which was harmless only while every key in the table was a
    // bare letter: `C-n` and `n` both came out as `n`, so Ctrl+N moved the
    // cursor and nothing said why. With the table under `C-` and `M-` that
    // silence would have been the whole feature failing.
    //
    // `Key::notation` is what `Key::parse` reads back, which is what the unit
    // test beside the table relies on: every entry has to round-trip.
    t.notation()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-view-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("criar diretorio de teste");
        dir
    }

    /// A view with NO sidebar, which is what almost every test wants: they are
    /// about the listing, and a sidebar would move every column two dozen cells
    /// to the right for no reason.
    fn view_at(dir: &Path) -> View {
        View::at(
            dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            no_sidebar(),
        )
    }

    /// An empty sidebar, hidden.
    fn no_sidebar() -> ronoctua_files::sidebar::Sidebar {
        let mut prefs = ronoctua_files::sidebar::Preferences::fresh();
        prefs.showing = false;
        ronoctua_files::sidebar::Sidebar::of(&[], None, None, prefs, None)
    }

    /// A sidebar with `rows` places in it, showing. The paths need not exist:
    /// they are added rather than declared, and only a DECLARED default is
    /// dropped for not being there.
    fn a_sidebar(rows: usize) -> ronoctua_files::sidebar::Sidebar {
        use ronoctua_files::sidebar::{Item, Preferences, Sidebar, Target};
        let mut side = Sidebar::of(&[], None, None, Preferences::fresh(), None);
        for n in 0..rows {
            side.add(Item {
                id: format!("place:/lugar{n}"),
                target: Target::Place(PathBuf::from(format!("/lugar{n}"))),
                label: Some(format!("lugar{n}")),
            });
        }
        side
    }

    /// A view with a sidebar of `rows` places.
    fn view_beside(dir: &Path, rows: usize) -> View {
        View::at(
            dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            a_sidebar(rows),
        )
    }

    /// The same, with a trash whose root is under the test's own tree.
    ///
    /// A parameter and not `$XDG_DATA_HOME`: two tests setting the same
    /// environment variable race, and the one that loses puts a file in the
    /// other's trash.
    fn view_in_trash(root: &Path) -> View {
        let bin = ronoctua_files::trash::Bin::at(root.join("Trash"));
        bin.ensure().expect("criar a lixeira de teste");
        View::at(
            &bin.files(),
            Roster::default(),
            Zone::utc(),
            Some(bin),
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            no_sidebar(),
        )
    }

    /// The bundle the tests draw with. Loading it is what makes a row's text a
    /// finished string, which is what `field_at` compares.
    fn bundle() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::from_env()
    }

    fn press(c: char) -> Key {
        Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Char(c),
        }
    }

    /// A key of the VIEW, which since the query took the bare letters all
    /// carry a modifier.
    fn of_view(notation: &str) -> Key {
        Key::parse(notation).unwrap_or_else(|_| panic!("`{notation}` nao e uma tecla"))
    }

    #[test]
    fn the_columns_give_way_in_the_stated_order() {
        // THE DEFECT THIS CATCHES: an order that drifts with the last edit, so
        // the narrow case shows whatever happened to survive. Wide first, then
        // each width just below the point where one more column stops fitting.
        let wide = layout_beside(120, 0);
        for c in [
            Column::Icon,
            Column::Name,
            Column::Perms,
            Column::Owner,
            Column::Group,
            Column::Size,
            Column::Modified,
        ] {
            assert!(wide.find(c).is_some(), "{c:?} nao coube em 120 colunas");
        }
        // Narrowing drops them in the declared order and never out of it.
        let mut gone = Vec::new();
        for width in (1..=120).rev() {
            let l = layout_beside(width, 0);
            for c in GIVE_WAY {
                if l.find(c).is_none() && !gone.contains(&c) {
                    gone.push(c);
                }
            }
        }
        assert_eq!(gone, GIVE_WAY.to_vec(), "a ordem de ceder mudou");
    }

    #[test]
    fn the_name_never_gives_way_however_narrow_it_gets() {
        for width in 1..=200u16 {
            let l = layout_beside(width, 0);
            assert!(
                l.find(Column::Name).is_some(),
                "sem coluna de nome em {width} colunas"
            );
            // And nothing leaks off the edge, which is the one rule the
            // compositor will not enforce for us.
            for (c, at, w) in &l.columns {
                assert!(
                    at + w <= width.max(1) + 1,
                    "{c:?} vaza: comeca em {at}, tem {w}, a tela tem {width}"
                );
            }
        }
    }

    #[test]
    fn every_permission_character_is_its_own_field_with_its_own_sentence() {
        // The whole reason the tooltip exists. Ten characters that look like
        // one word: with a single field the pointer could only explain the
        // column, which is the same as explaining nothing.
        let root = scratch("perms");
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let v = view_at(&root);
        let rows = v.rows(120, &bundle());
        assert_eq!(rows.len(), 1);
        let (at, _) = layout_beside(120, 0).find(Column::Perms).unwrap();
        let mut sentences = Vec::new();
        for k in 0..10u16 {
            let f = rows[0].field_at(at + k).expect("celula sem campo");
            assert_eq!(width_of(&f.text), 1, "a celula {k} nao tem um caractere so");
            sentences.push(f.explain.clone().expect("celula sem explicacao").key);
        }
        sentences.sort_unstable();
        sentences.dedup();
        assert_eq!(sentences.len(), 10, "duas celulas explicam a mesma coisa");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_mark_follows_the_name_and_not_the_row() {
        // THE DEFECT THIS CATCHES: marks kept by index. A re-read after a
        // rename, a delete or a change of sort moves every index — and an
        // action then applies to a file nobody chose.
        let root = scratch("marks");
        for n in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(root.join(n), b"x").unwrap();
        }
        let mut v = view_at(&root);
        v.key(of_view("C-SPC")); // marks a.txt, cursor moves to b.txt
        assert_eq!(v.chosen(), vec![root.join("a.txt")]);

        // Sorting the other way round moves every index.
        v.key(of_view("C-S"));
        assert_eq!(
            v.chosen(),
            vec![root.join("a.txt")],
            "a marca acompanhou o indice em vez do nome"
        );

        // And a mark on a file that is gone goes with it.
        std::fs::remove_file(root.join("a.txt")).unwrap();
        v.reread();
        assert!(v.chosen() != vec![root.join("a.txt")]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn hidden_files_are_shown_and_the_key_hides_them() {
        let root = scratch("hidden");
        std::fs::write(root.join(".oculto"), b"x").unwrap();
        std::fs::write(root.join("visivel"), b"x").unwrap();
        let mut v = view_at(&root);
        assert_eq!(v.amount(), 2, "os ocultos nao apareceram");
        v.key(of_view("C-h"));
        assert_eq!(v.amount(), 1);
        v.key(of_view("C-h"));
        assert_eq!(v.amount(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_frame_costs_the_same_in_a_big_directory_as_in_a_small_one() {
        // THE DEFECT THIS LOCKS DOWN, and it was reported as "the hover lags
        // behind the pointer".
        //
        // Every frame built a row for EVERY entry and then drew twelve. Worse,
        // the frame did it several times over: the height, the page count and
        // the offset each asked for the list again. Measured in release on the
        // machine that reported it:
        //
        //     entries   whole list   window of 12
        //         200      0.61 ms        0.03 ms
        //       2 000      8.06 ms        0.03 ms
        //      10 000     56.87 ms        0.03 ms
        //
        // The window is FLAT, which is the property worth a test: not "it is
        // fast" but "it does not grow". So the assertion is a ratio against a
        // small directory and not a millisecond count — a slow machine makes
        // both sides slow, and the shape is what has to hold.
        let small = scratch("frame-small");
        for n in 0..20 {
            std::fs::write(small.join(format!("f-{n:05}.txt")), b"x").unwrap();
        }
        let big = scratch("frame-big");
        for n in 0..3000 {
            std::fs::write(big.join(format!("f-{n:05}.txt")), b"x").unwrap();
        }
        let b = bundle();
        let v_small = view_at(&small);
        let v_big = view_at(&big);

        // WARM FIRST: the first call through this path pays for the layout
        // table and the text bundle, and that cost belongs to neither side.
        for _ in 0..20 {
            let _ = v_small.rows_from(0, 12, 174, &b);
            let _ = v_big.rows_from(0, 12, 174, &b);
        }

        let rounds = 200;
        let t = std::time::Instant::now();
        for _ in 0..rounds {
            let _ = v_small.rows_from(0, 12, 174, &b);
        }
        let pequeno = t.elapsed();
        let t = std::time::Instant::now();
        for _ in 0..rounds {
            let _ = v_big.rows_from(0, 12, 174, &b);
        }
        let grande = t.elapsed();

        assert_eq!(v_big.rows_from(0, 12, 174, &b).len(), 12, "a janela vazou");
        // 150x the entries. A ceiling of 8x is loose enough for a debug build
        // and a loaded machine, and still an order of magnitude below the 40x
        // the whole-list version would show.
        assert!(
            grande < pequeno * 8,
            "o quadro cresce com o diretorio: 20 entradas {pequeno:?}, 3000 entradas {grande:?}"
        );
        let _ = std::fs::remove_dir_all(&small);
        let _ = std::fs::remove_dir_all(&big);
    }

    #[test]
    fn the_right_button_does_not_walk_into_the_directory() {
        // REPORTED FROM THE SCREEN, and in two shapes that look unrelated:
        // right-clicking an EMPTY folder walked into it and showed no menu, and
        // right-clicking a full one showed a menu over the contents of the
        // folder it had just entered. One cause: the right button "focused"
        // with the ACTIVATING click.
        let root = scratch("rightdir");
        std::fs::create_dir(root.join("vazia")).unwrap();
        std::fs::create_dir(root.join("cheia")).unwrap();
        std::fs::write(root.join("cheia/um.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        let onde = v.where_().to_path_buf();

        // Directories come first, so rows 0 and 1 are the two folders.
        for row in [0usize, 1] {
            let step = v.focus_row(row);
            assert!(matches!(step, Step::Changed), "{step:?}");
            assert_eq!(v.cursor(), row, "o foco nao foi para a linha clicada");
            assert_eq!(
                v.where_(),
                onde,
                "o botao direito entrou na pasta em vez de so focar"
            );
            assert!(!v.menu_open(), "focar nao abre o menu sozinho");
        }

        // And then the menu opens, on the row that was clicked.
        let t = key_that_runs("files-menu").expect("files-menu tem tecla");
        v.key(t);
        assert!(v.menu_open(), "o menu nao abriu");
        assert_eq!(v.where_(), onde, "o menu abriu noutra pasta");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_right_button_presses_the_menus_own_key() {
        // THE DEFECT THIS CATCHES: both doors opened the menu by synthesising
        // `Char(',')`, written out by hand. When the bare letters became query
        // text, the right button started typing a comma into the filter and the
        // menu stopped appearing — silently, because typing a character is a
        // perfectly good thing for a key to do.
        let root = scratch("rightclick");
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        let t = key_that_runs("files-menu").expect("files-menu tem de ter tecla na tabela");
        let step = v.key(t);
        assert!(matches!(step, Step::Changed), "{step:?}");
        assert!(v.menu_open(), "a tecla do menu nao abriu o menu");
        assert!(
            v.filter().is_empty(),
            "a tecla do menu foi parar no filtro: {:?}",
            v.filter()
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn what_is_typed_goes_at_the_end_of_the_path() {
        // ASKED FOR FROM THE SCREEN, and the reason is that the two read as one
        // sentence: the path says where you are, the query narrows what is
        // shown there. It also gives the message line back its own job — while
        // the query lived there, hovering a permission bit and typing took
        // turns erasing each other.
        let root = scratch("crumbquery");
        std::fs::write(root.join("relatorio.txt"), b"x").unwrap();
        let mut v = view_at(&root);

        let plain = v.crumbs();
        for c in "rel".chars() {
            v.key(press(c));
        }
        let typed = v.crumbs();
        assert_eq!(
            typed.len(),
            plain.len() + 1,
            "a query nao virou um campo do caminho"
        );
        let last = typed.last().expect("o caminho tem campos");
        assert_eq!(last.text, "rel");
        // A DIFFERENT COLOUR IS THE WHOLE FORMATTING: no box, no separator
        // (invariants 14 and 15). It has to differ from the piece before it,
        // which is the directory one is in.
        let before = &typed[typed.len() - 2];
        assert_ne!(
            last.color, before.color,
            "a query saiu com a mesma cor do caminho"
        );
        // And it leads nowhere: it is not a directory yet.
        assert!(matches!(last.hit, Hit::Nothing), "{:?}", last.hit);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn backspace_with_nothing_typed_goes_up_a_level() {
        // It reads as one gesture with the query at the end of the path: erase
        // what you typed, keep erasing, and what is behind it is the folder you
        // are in. And it must NOT close the view — backspace held down would
        // take it away, which is the thing one is least likely to have meant.
        let root = scratch("backup");
        std::fs::create_dir(root.join("dentro")).unwrap();
        std::fs::write(root.join("dentro/um.txt"), b"x").unwrap();
        let mut v = view_at(&root.join("dentro"));
        let del = Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Backspace,
        };

        v.key(press('u'));
        assert_eq!(v.filter(), "u");
        let step = v.key(del);
        assert!(matches!(step, Step::Changed), "{step:?}");
        assert_eq!(v.where_(), root.join("dentro"), "apagou uma letra e SUBIU");

        let step = v.key(del);
        assert!(matches!(step, Step::Changed), "{step:?}");
        assert_eq!(v.where_(), root, "backspace com nada digitado nao subiu");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_question_on_screen_says_what_it_is_asking() {
        // REPORTED FROM THE SCREEN: a list of people came up and nothing said
        // it was about the OWNER. The choices cannot say it — a list of names
        // looks the same whatever it is for — and the menu row that opened it
        // is gone by then.
        let root = scratch("asktitle");
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        let b = bundle();

        // The PERMISSIONS one, because its choices are a table in the code and
        // do not depend on the machine's `/etc/passwd` — the owner list is
        // empty in a test and the question would refuse to open at all.
        v.key(of_view("C-m"));
        v.key(press('m'));
        let rows = v.rows(120, &b);
        let title = rows[0].fields[0].text.clone();
        assert!(!title.is_empty(), "a pergunta subiu sem titulo");
        assert!(
            !title.starts_with("files-"),
            "a chave crua foi para a tela: {title}"
        );
        assert!(
            rows[1].fields[0].text.starts_with('>'),
            "a caixa de digitacao devia ser a SEGUNDA linha: {:?}",
            rows[1].fields[0].text
        );

        // AND THE CONFIRMATION IS ANOTHER QUESTION, with a title of its own.
        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Enter,
        });
        let asking = v.rows(120, &b)[0].fields[0].text.clone();
        assert!(!asking.is_empty(), "a confirmacao subiu sem titulo");
        assert_ne!(asking, title, "a confirmacao repetiu o titulo da pergunta");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn no_key_of_the_view_is_the_compositors() {
        // THE COMPOSITOR SEES THE KEY FIRST, and that is the whole of it.
        //
        // The view's commands were moved to `M-` when the bare letters became
        // query text, and `M-RET` never arrived: in the nested loop the chord
        // family IS `Alt`, so niri opened a terminal instead. It was reported
        // from the screen as "M-RET opens the terminal, not the menu" — and the
        // view was blameless, because the key never reached it.
        //
        // `Ctrl` is the family nothing above us claims: the leader is a
        // sequence, the chords are `Mod+…`, and the emergency binds are
        // `Ctrl+Alt+…`. So the rule is simple enough to check: no `M-`, and no
        // `C-M-` either.
        for (notation, id) in OF_THE_VIEW {
            let t = ronoctua_keymap::Key::parse(notation)
                .unwrap_or_else(|_| panic!("`{notation}` nao e uma tecla"));
            assert!(
                !t.mods.alt,
                "`{notation}` (`{id}`) usa Alt, que no laco aninhado e a familia \
                 dos acordes do compositor — a tecla nao chega na vista"
            );
        }
    }

    #[test]
    fn an_empty_directory_still_has_a_menu_and_it_says_about_what() {
        // "There is nothing here" is not "there is nothing to do": in an empty
        // folder the useful actions are the ones about the FOLDER — paste what
        // was copied, open a terminal in it. The right button answered with
        // nothing at all, and it was reported as the button not working.
        let root = scratch("emptymenu");
        std::fs::create_dir_all(root.join("vazia")).unwrap();
        let mut v = view_at(&root.join("vazia"));
        assert_eq!(v.amount(), 0);
        assert!(v.is_empty());

        let t = key_that_runs("files-menu").expect("files-menu tem tecla");
        let step = v.key(t);
        assert!(matches!(step, Step::Changed), "{step:?}");
        assert!(v.menu_open(), "pasta vazia ficou sem menu");

        let rows = v.rows(120, &bundle());
        assert!(rows.len() > 1, "o menu subiu sem linhas: {rows:?}");
        // AND THE FIRST LINE SAYS WHO IT IS ABOUT — here, the folder itself.
        let title = &rows[0].fields[0].text;
        assert!(title.contains("vazia"), "o menu nao diz o alvo: {title}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_menu_says_which_entry_it_is_about() {
        // The menu COVERS the listing, so the row it was opened on is no longer
        // visible: twenty verbs with nothing saying what they act on. With
        // marks it is worse, because the answer is not the row under the
        // pointer — it is all of them.
        let root = scratch("menutitle");
        std::fs::write(root.join("alvo.txt"), b"x").unwrap();
        std::fs::write(root.join("outro.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        let t = key_that_runs("files-menu").expect("files-menu tem tecla");

        v.key(t);
        let title = v.rows(120, &bundle())[0].fields[0].text.clone();
        assert!(title.contains("alvo.txt"), "{title}");

        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Esc,
        });
        v.key(of_view("C-a")); // mark everything
        v.key(t);
        let title = v.rows(120, &bundle())[0].fields[0].text.clone();
        assert!(
            !title.contains("alvo.txt"),
            "com marcas o alvo nao e uma entrada so: {title}"
        );
        assert!(title.contains('2'), "nao disse quantos: {title}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_question_names_what_will_happen_and_the_answers_are_words() {
        // BOTH HALVES REPORTED FROM THE SCREEN. The question was "Tem certeza?",
        // which does not say of WHAT — and by then the row that was clicked is
        // two screens away. And the answers were `files-no` and `files-yes`:
        // raw text keys on the screen, which is the exact shape invariant 8
        // exists to refuse.
        //
        // The cause of the second was that `Choice.label` held a KEY "to be
        // resolved by whoever draws", and nobody drew it: `picker_rows` wrote
        // the label verbatim, and `Picker::shown` filtered on it.
        let root = scratch("askwords");
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        let b = bundle();

        v.click(0, &Hit::Bit(0, mode::Who::Group, mode::What::Write));
        let rows = v.rows(120, &b);
        let question = rows[0].fields[0].text.clone();
        assert!(
            question.contains("um.txt"),
            "a pergunta nao diz sobre o que e: {question}"
        );
        assert!(
            question.contains("0664") || question.contains("0644"),
            "a pergunta nao diz o que vai virar: {question}"
        );

        let answers: Vec<String> = rows[2..]
            .iter()
            .filter_map(|r| r.fields.first().map(|f| f.text.clone()))
            .collect();
        assert!(!answers.is_empty(), "a pergunta subiu sem respostas");
        for a in &answers {
            assert!(
                !a.starts_with("files-"),
                "resposta com a chave crua na tela: {a}"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_preset_is_offered_by_its_sentence_and_not_by_its_key() {
        // The same defect, on the list that has been there since the beginning:
        // the permission presets carry text keys as labels.
        let root = scratch("presets");
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        v.key(key_that_runs("files-menu").expect("tecla do menu"));
        v.key(press('m'));
        for r in v.rows(120, &bundle()).iter().skip(2) {
            let Some(f) = r.fields.first() else { continue };
            assert!(
                !f.text.starts_with("files-"),
                "preset com a chave crua na tela: {}",
                f.text
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn choosing_an_owner_produces_the_operation() {
        // REPORTED FROM THE SCREEN: choosing `nobody`, then `mail`, and the
        // owner staying `noctua` with nothing said. There are three places that
        // could swallow it — the question, the answer, and the carrying out —
        // and only the first two are ours to test without a system bus.
        let root = scratch("ownerpick");
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let roster = Roster::parse(
            "root:x:0:0::/root:/bin/sh\nnoctua:x:1000:100::/home/noctua:/bin/sh\nnobody:x:65534:65534::/:/bin/sh\n",
            "root:x:0:\nusers:x:100:\n",
        );
        let mut v = View::at(
            &root,
            roster,
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            no_sidebar(),
        );

        v.key(key_that_runs("files-menu").expect("tecla do menu"));
        v.key(press('O')); // the menu's key for "owner"
        assert!(v.showing_detail(), "a pergunta de dono nao abriu");

        // Narrow to `nobody` by typing, exactly as a person would, and confirm.
        for c in "nobody".chars() {
            v.key(press(c));
        }
        let step = v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Enter,
        });
        match step {
            Step::Do(Doing::Owner(paths, uid)) => {
                assert_eq!(paths.len(), 1);
                assert_eq!(uid, 65534, "escolheu `nobody` e saiu outro uid");
            }
            other => panic!("escolher um dono devolveu {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn changing_the_owner_without_the_unit_says_so() {
        // "Trocar o dono não fez nada" — and it had: the request could not even
        // be WRITTEN, because `/run/ronoctua-files/` belongs to a unit this
        // distro installs and the nested loop has no NixOS. What came out was
        // `Permission denied` from a thread, three seconds on the message line.
        //
        // Answered synchronously now, and with a sentence that says why. The
        // test runs where that directory does not exist, which is the case it
        // is about; where it does, the branch is not taken and this asserts
        // nothing — so it checks the precondition first and says so.
        use smithay_client_toolkit::reexports::calloop::channel;
        if std::path::Path::new(ronoctua_files::act::STAGING)
            .parent()
            .is_some_and(std::path::Path::is_dir)
        {
            eprintln!("pulando: esta maquina TEM /run/ronoctua-files");
            return;
        }
        let (tx, _rx) = channel::channel::<Report>();
        let errand = carry_out(
            Doing::Owner(vec![std::path::PathBuf::from("/tmp/qualquer")], 65534),
            &tx,
            &Clip::default(),
        );
        match errand {
            Errand::Say(e) => assert_eq!(e.key, ronoctua_i18n::key::FILES_NO_ROOT_HELPER),
            other => panic!("sem a unidade, trocar dono devolveu {other:?}"),
        }
    }

    #[test]
    fn typing_filters_with_no_key_before_it() {
        // THE WHOLE POINT: the list narrows from the FIRST letter. It used to
        // need a `/` first, and the owner asked for what every finder in this
        // system already does — the locator and the `M-x` both take the letter
        // as the query.
        let root = scratch("filter");
        std::fs::write(root.join("relatorio.txt"), b"x").unwrap();
        std::fs::write(root.join("outro.md"), b"x").unwrap();
        let mut v = view_at(&root);
        assert!(!v.filtering());

        for c in "rela".chars() {
            v.key(press(c));
        }
        assert_eq!(v.filter(), "rela", "a digitacao nao chegou ao filtro");
        assert!(v.filtering());
        assert_eq!(v.amount(), 1);

        // Backspace takes one letter back, and the list widens again.
        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Backspace,
        });
        assert_eq!(v.filter(), "rel");

        // Esc clears the query and gives the whole directory back — WITHOUT
        // closing the view, which would throw away both in one press.
        let step = v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Esc,
        });
        assert!(
            matches!(step, Step::Changed),
            "Esc com filtro na tela nao pode fechar a vista: {step:?}"
        );
        assert!(!v.filtering());
        assert_eq!(v.amount(), 2);

        // And with nothing typed, Esc closes — as every view in this system does.
        let step = v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Esc,
        });
        assert!(matches!(step, Step::Closed), "Esc vazio tem de fechar");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_letter_that_is_a_command_key_does_not_reach_the_query() {
        // The other direction, and the reason the commands moved to `M-`: with
        // `s` still bound to sorting, typing the name of a file starting with
        // `s` would sort the list instead of narrowing it.
        let root = scratch("letter");
        std::fs::write(root.join("setup.sh"), b"x").unwrap();
        std::fs::write(root.join("mapa.txt"), b"x").unwrap();
        let mut v = view_at(&root);

        for c in "setup".chars() {
            v.key(press(c));
        }
        assert_eq!(v.filter(), "setup");
        assert_eq!(v.amount(), 1);

        // And the command itself still exists, one modifier away.
        let before = v.amount();
        v.key(Key {
            mods: ronoctua_keymap::Mods {
                ctrl: true,
                ..Default::default()
            },
            sym: Sym::Char('s'),
        });
        assert_eq!(v.filter(), "setup", "`C-s` virou letra do filtro");
        assert_eq!(v.amount(), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clicking_a_permission_cell_asks_for_exactly_that_bit() {
        let root = scratch("click");
        let file = root.join("um.txt");
        std::fs::write(&file, b"x").unwrap();
        let mut v = view_at(&root);
        // THE CLICK ASKS FIRST — see `Asking::Confirm`. What it asks is on
        // screen, and the answer is what carries the change.
        let step = v.click(0, &Hit::Bit(0, mode::Who::Group, mode::What::Write));
        assert!(
            matches!(step, Step::Changed),
            "clicar num bit nao pode aplicar direto: {step:?}"
        );
        let enter = Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Enter,
        };
        // The cursor starts on "no": a stray Enter must change nothing.
        assert!(matches!(v.key(enter), Step::Changed));
        let step = v.click(0, &Hit::Bit(0, mode::Who::Group, mode::What::Write));
        assert!(matches!(step, Step::Changed), "{step:?}");
        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Down,
        });
        let step = v.key(enter);
        match step {
            Step::Do(Doing::Mode(paths, wanted)) => {
                // ONE path: a click is about the row it landed on, whatever is
                // marked.
                assert_eq!(paths, vec![file]);
                // The bit flipped, and the file TYPE bits did not: flipping a
                // permission must never turn a file into a socket.
                assert_eq!(wanted & 0o170_000, 0o100_000);
                assert_ne!(wanted & 0o020, 0);
            }
            other => panic!("clicar no bit devolveu {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn going_up_lands_the_cursor_on_where_one_came_from() {
        // Without it, going up and changing one's mind costs a hunt through
        // the parent — and `-` stops being the inverse of `Enter`.
        let root = scratch("up");
        std::fs::create_dir_all(root.join("aaa")).unwrap();
        std::fs::create_dir_all(root.join("bbb")).unwrap();
        std::fs::create_dir_all(root.join("ccc")).unwrap();
        let mut v = view_at(&root.join("bbb"));
        v.up();
        assert_eq!(v.focused().map(|e| e.name.as_str()), Some("bbb"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_irreversible_row_needs_the_key_twice() {
        let root = scratch("arm");
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        v.key(of_view("C-m"));
        assert!(v.menu_open());
        // First press arms and changes nothing on disk.
        let first = v.key(press('D'));
        assert!(matches!(first, Step::Changed));
        assert!(v.menu_open(), "o menu fechou antes de agir");
        // Second press acts.
        let second = v.key(press('D'));
        assert!(matches!(second, Step::Do(Doing::Delete(_))), "{second:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn moving_disarms_a_pending_confirmation() {
        // THE DEFECT THIS CATCHES: arming `delete` on one row, moving to
        // another, and the second press acting on the row one moved TO.
        let root = scratch("disarm");
        std::fs::write(root.join("a.txt"), b"x").unwrap();
        std::fs::write(root.join("b.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        v.key(of_view("C-m"));
        v.key(press('D'));
        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Down,
        });
        let after = v.key(press('D'));
        assert!(
            matches!(after, Step::Changed),
            "a confirmacao sobreviveu ao movimento: {after:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn moving_the_cursor_changes_what_is_drawn() {
        // THE DEFECT THIS CATCHES, and it is the one that shipped: the arrows
        // moved the cursor, every unit test agreed, and the SCREEN showed
        // nothing — the focus was not drawn at all. The report was "the
        // keyboard does not work in the file manager", and the keyboard was
        // fine. A cursor nobody can see is indistinguishable from a key that
        // does nothing.
        //
        // So the assertion is not "the state changed". It is "what would be
        // DRAWN changed".
        let root = scratch("visible");
        for n in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(root.join(n), b"x").unwrap();
        }
        let mut v = view_at(&root);
        let drawn = |v: &View| -> Vec<(String, ronoctua_tokens::Color)> {
            v.rows(120, &bundle())
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| (f.text.clone(), f.color)))
                .collect()
        };
        let before = drawn(&v);
        let step = v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Down,
        });
        assert!(matches!(step, Step::Changed), "a seta nao mexeu o cursor");
        assert_ne!(before, drawn(&v), "o cursor mexeu e o desenho ficou igual");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_focus_is_a_colour_and_the_mark_is_a_glyph() {
        // Two facts on one channel leaves "which one is the cursor on?"
        // unanswered the moment a second row is marked. So the focus takes the
        // text's colour and the mark takes a glyph in the gutter — never a box
        // and never an outline (invariant 15).
        let root = scratch("channels");
        for n in ["a.txt", "b.txt"] {
            std::fs::write(root.join(n), b"x").unwrap();
        }
        let mut v = view_at(&root);
        let name_at = layout_beside(120, 0).find(Column::Name).unwrap().0;

        let rows = v.rows(120, &bundle());
        let focused = rows[0].field_at(name_at).unwrap().color;
        let other = rows[1].field_at(name_at).unwrap().color;
        assert_ne!(focused, other, "a linha em foco tem a cor das outras");

        // MARKING DOES NOT TOUCH THE NAME'S COLOUR; it puts a glyph in the
        // gutter, which is cell zero.
        v.key(of_view("C-SPC"));
        let rows = v.rows(120, &bundle());
        let mark = &rows[0].fields[0];
        assert_eq!(mark.at, 0);
        assert!(!mark.text.is_empty(), "a marca nao aparece na calha");
        assert!(
            rows[1].fields[0].text.is_empty(),
            "linha sem marca tem marca"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_is_drawn_against_the_screens_edge() {
        // The icon was flush against the border and read as clipped. The gutter
        // is the same `modeline.edgeLeft` the rest of the footer leaves, and it
        // is also where the mark goes — one cell doing two jobs.
        for width in 20..=200u16 {
            let l = layout_beside(width, 0);
            for (c, at, _) in &l.columns {
                assert!(
                    *at >= GUTTER,
                    "{c:?} comeca na celula {at}, colada na borda (tela de {width})"
                );
            }
        }
    }

    #[test]
    fn a_change_of_metadata_reaches_every_marked_file() {
        // THE DEFECT THIS CATCHES, and it was there: with six files marked,
        // "change the permission" acted on the FIRST marked path while reading
        // the FOCUSED entry's current mode — two different files — and said
        // nothing about the other five. A silent partial action is worse than
        // a refusal, because nothing tells you which half happened.
        let root = scratch("batch");
        for n in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(root.join(n), b"x").unwrap();
        }
        let mut v = view_at(&root);
        v.key(of_view("C-a")); // mark everything in sight
        assert_eq!(v.chosen().len(), 3);

        v.key(of_view("C-m"));
        // `m` here is a MENU row's key, not the view's: the menu is on screen.
        let step = v.key(press('m'));
        assert!(matches!(step, Step::Changed), "{step:?}");
        // The picker is up; choosing a preset ASKS FIRST, and the question
        // starts on "no" — a stray `Enter` must not change six files.
        let enter = Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Enter,
        };
        let step = v.key(enter);
        assert!(
            matches!(step, Step::Changed),
            "escolher um preset nao pode aplicar direto: {step:?}"
        );
        let step = v.key(enter);
        assert!(
            matches!(step, Step::Changed),
            "a resposta padrao da confirmacao tem de ser NAO: {step:?}"
        );

        // And now, on purpose: the preset again, then `yes`.
        v.key(of_view("C-m"));
        v.key(press('m'));
        v.key(enter);
        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Down,
        });
        match v.key(enter) {
            Step::Do(Doing::Mode(paths, _)) => assert_eq!(paths.len(), 3, "{paths:?}"),
            other => panic!("mudar permissao de tres marcados deu {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_terminal_opens_inside_the_folder_one_is_pointing_at() {
        // Right-clicking a folder and asking for a terminal means a terminal in
        // THAT folder. Using the current directory would open one level above
        // the thing being pointed at, which is a gesture that looks like it
        // worked and lands somewhere else.
        let root = scratch("terminal");
        std::fs::create_dir(root.join("dentro")).unwrap();
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let mut v = view_at(&root);

        v.menu = Some(0);
        match v.run(menu::by_id("terminal").unwrap()) {
            Step::Do(Doing::Terminal(dir)) => assert_eq!(dir, root.join("dentro")),
            other => panic!("terminal numa pasta deu {other:?}"),
        }

        // On a FILE there is no inside, so it is the directory one is looking
        // at — which is the other half of the same rule.
        v.cursor = 1;
        v.menu = Some(0);
        match v.run(menu::by_id("terminal").unwrap()) {
            Step::Do(Doing::Terminal(dir)) => assert_eq!(dir, root),
            other => panic!("terminal num arquivo deu {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_row_with_no_plural_ignores_the_marks() {
        // Renaming six files to one name is meaningless, and "open with" opens
        // one thing. Those take the FOCUSED entry whatever is marked — and
        // taking the first MARKED one instead would rename a file the person
        // is not looking at.
        let root = scratch("singular");
        for n in ["a.txt", "b.txt"] {
            std::fs::write(root.join(n), b"x").unwrap();
        }
        let mut v = view_at(&root);
        v.cursor = 1; // b.txt
        let watching = v.focused_path();
        v.marks.insert("a.txt".to_owned());

        v.menu = Some(0);
        v.run(menu::by_id("rename").unwrap());
        let asking = v.picking.as_ref().map(|p| p.asking.clone());
        match asking {
            Some(Asking::Name(path)) => assert_eq!(Some(path), watching),
            other => panic!("renomear pediu {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn every_menu_row_reaches_an_arm_that_does_something() {
        // THE DEFECT THIS CATCHES: a row in the table with no arm in `run`.
        // It would draw, be translated, be checked — and do nothing, which is
        // indistinguishable from a key that is not bound. It is the same
        // defect `WITH_ACTION` refuses in the leader.
        let root = scratch("arms");
        std::fs::write(root.join("um.zip"), b"PK\x03\x04").unwrap();
        // A plain `.tar` too: `extract-tar` applies to `application/x-tar`,
        // and a `.zip` is not one — the two rows exist precisely because the
        // database says the formats are unrelated.
        std::fs::write(root.join("dois.tar"), b"").unwrap();
        std::fs::create_dir(root.join("pasta")).unwrap();
        // A FRESH VIEW PER ROW, and the reason is a defect this test found:
        // running `open` on a directory ENTERS it, so every row after that one
        // was being asked of an empty directory — and `open-with` came back
        // "never reached" while being perfectly reachable.
        let mut reached = std::collections::BTreeSet::new();
        // AND ONE ENTRY IN THE TRASH, so `restore` — the row that closes the
        // trash's loop — is asked the question too. Without it the row would be
        // unreachable and this test would say so, which is what it did.
        {
            let trash = scratch("arms-trash");
            let bin = ronoctua_files::trash::Bin::at(trash.join("Trash"));
            bin.ensure().unwrap();
            std::fs::write(bin.files().join("devolvivel.txt"), b"x").unwrap();
            let mut v = view_in_trash(&trash);
            for a in v.offered_now() {
                v.menu = Some(0);
                v.arming = if a.confirms { Some(a.id) } else { None };
                let step = v.run(a);
                assert!(!matches!(step, Step::Nothing), "`{}` nao faz nada", a.id);
                reached.insert(a.id);
            }
            let _ = std::fs::remove_dir_all(&trash);
        }
        let how_many = view_at(&root).amount();
        for cursor in 0..how_many {
            let offered: Vec<_> = {
                let mut v = view_at(&root);
                v.cursor = cursor;
                v.clip = Clip {
                    paths: vec![root.join("um.zip")],
                    moves: false,
                };
                v.offered_now()
            };
            for a in offered {
                let mut v = view_at(&root);
                v.cursor = cursor;
                v.clip = Clip {
                    paths: vec![root.join("um.zip")],
                    moves: false,
                };
                v.menu = Some(0);
                v.arming = if a.confirms { Some(a.id) } else { None };
                let step = v.run(a);
                assert!(
                    !matches!(step, Step::Nothing),
                    "a linha `{}` do menu nao faz nada",
                    a.id
                );
                reached.insert(a.id);
            }
        }
        // And between the two entries, every row of the table was reachable.
        for a in menu::all_of() {
            assert!(
                reached.contains(a.id),
                "a linha `{}` nao foi alcancada por nenhuma entrada",
                a.id
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_breadcrumb_leads_where_it_says() {
        let v = view_at(Path::new("/usr/share"));
        let crumbs = v.crumbs();
        assert!(crumbs.len() >= 3);
        // The last piece is where one IS, and is not a place to go.
        assert_eq!(crumbs.last().unwrap().hit, Hit::Nothing);
        assert_eq!(v.crumb_target(0), Some(PathBuf::from("/")));
        assert_eq!(v.crumb_target(1), Some(PathBuf::from("/usr")));
    }

    #[test]
    fn a_broken_link_does_not_look_like_a_working_one() {
        let root = scratch("broken");
        std::os::unix::fs::symlink("nao-existe", root.join("quebrado")).unwrap();
        std::fs::write(root.join("alvo"), b"x").unwrap();
        std::os::unix::fs::symlink("alvo", root.join("inteiro")).unwrap();
        let v = view_at(&root);
        let rows = v.rows(120, &bundle());
        let (at, _) = layout_beside(120, 0).find(Column::Name).unwrap();
        let names: Vec<_> = rows
            .iter()
            .map(|r| r.field_at(at).unwrap())
            .map(|f| (f.text.clone(), f.color))
            .collect();
        let broken = names
            .iter()
            .find(|(t, _)| t.starts_with("quebrado"))
            .unwrap();
        let whole = names
            .iter()
            .find(|(t, _)| t.starts_with("inteiro"))
            .unwrap();
        assert_ne!(
            broken.1, whole.1,
            "o link quebrado tem a mesma cor do inteiro"
        );
        // And both say where they point, which is the one thing a listing
        // cannot leave out about a link.
        assert!(broken.0.contains("nao-existe"));
        assert!(whole.0.contains("alvo"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directorys_own_size_is_left_blank_until_somebody_asks() {
        // Four kibibytes is the size of the directory ENTRY and answers a
        // question nobody asked. A number that answers a different question is
        // worse than no number.
        let root = scratch("size");
        std::fs::create_dir(root.join("pasta")).unwrap();
        let mut v = view_at(&root);
        let (at, _) = layout_beside(120, 0).find(Column::Size).unwrap();
        assert_eq!(v.rows(120, &bundle())[0].field_at(at).unwrap().text, "");
        v.measured("pasta", 5 * 1024 * 1024);
        assert_eq!(
            v.rows(120, &bundle())[0].field_at(at).unwrap().text,
            "5,0 MiB"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clicking_a_header_sorts_by_it_and_clicking_again_reverses() {
        // The gesture every table in the world has. The `s` key goes round the
        // same cycle; neither reaches anything the other does not.
        let root = scratch("header");
        std::fs::write(root.join("grande.txt"), vec![b'x'; 5000]).unwrap();
        std::fs::write(root.join("pequeno.txt"), b"x").unwrap();
        let mut v = view_at(&root);

        // THE ORDER is what changes, and the CURSOR stays on the same file —
        // that is what `reread` keeps by name, and it is right: one sorts to
        // see where a file stands, not to lose it.
        let watching = v.focused().map(|e| e.name.clone());
        let names = |v: &View| -> Vec<String> {
            (0..v.amount())
                .map(|i| v.entries[v.shown[i]].name.clone())
                .collect()
        };

        let step = v.click(usize::MAX, &Hit::Sort(Column::Size));
        assert!(matches!(step, Step::Changed));
        assert_eq!(names(&v), ["pequeno.txt", "grande.txt"]);
        assert_eq!(v.focused().map(|e| e.name.clone()), watching);

        // Again on the same column: it reverses, and does not go back to name.
        v.click(usize::MAX, &Hit::Sort(Column::Size));
        assert_eq!(names(&v), ["grande.txt", "pequeno.txt"]);

        // A DIFFERENT column starts ascending, rather than inheriting the
        // reversal — otherwise clicking `name` after reversing `size` would
        // give Z to A, which nobody asked for.
        v.click(usize::MAX, &Hit::Sort(Column::Name));
        assert_eq!(names(&v), ["grande.txt", "pequeno.txt"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_header_marks_the_column_that_is_sorting_and_only_it() {
        // Four arrows would be four columns each claiming to be the order.
        let root = scratch("mark");
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        v.click(usize::MAX, &Hit::Sort(Column::Modified));
        let header = v.header(120, &bundle());
        let marked: Vec<_> = header
            .fields
            .iter()
            .filter(|f| f.text.contains('↑') || f.text.contains('↓'))
            .collect();
        assert_eq!(marked.len(), 1, "{} colunas marcadas", marked.len());
        assert_eq!(marked[0].hit, Hit::Sort(Column::Modified));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_detail_sheet_says_what_the_content_is_and_not_only_the_name() {
        // A `.txt` holding a PNG is a real thing, and the listing's fast path —
        // which never opens a file — would go on calling it text for ever.
        let root = scratch("detail");
        let lying = root.join("foto.txt");
        std::fs::write(&lying, [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();
        let mut v = view_at(&root);
        let step = v.run(menu::by_id("info").unwrap());
        assert!(matches!(step, Step::Changed));

        let rows = v.rows(120, &bundle());
        let sheet: Vec<String> = rows
            .iter()
            .filter_map(|r| r.fields.last())
            .map(|f| f.text.clone())
            .collect();
        let joined = sheet.join(" | ");
        assert!(joined.contains("text/plain"), "{joined}");
        assert!(
            joined.contains("image/png"),
            "o conteudo nao foi examinado: {joined}"
        );
        // AND EVERY KEY CLOSES IT: it is opened to be looked at, so no gesture
        // is lost waiting for the right one.
        v.key(press('j'));
        assert!(!v.showing_detail());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn every_field_that_lights_up_says_something() {
        // THE DEFECT THIS CATCHES: a clickable field with no explanation. It
        // would highlight under the pointer, promising an action, and the
        // message line would go on showing whatever was there before.
        let root = scratch("explains");
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        std::fs::create_dir(root.join("pasta")).unwrap();
        let v = view_at(&root);
        for row in v.rows(120, &bundle()) {
            for f in &row.fields {
                if matches!(f.hit, Hit::Nothing | Hit::Name(_) | Hit::Mark(_)) {
                    continue;
                }
                assert!(
                    f.explain.is_some(),
                    "o campo `{}` ({:?}) acende e nao explica nada",
                    f.text,
                    f.hit
                );
            }
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    // =====================================================================
    // THE SIDEBAR
    //
    // The load-bearing ones compare WHAT WOULD BE DRAWN, and not the state.
    // That is the lesson this file already paid for once: the manager shipped
    // with the arrows working and the screen showing nothing, twenty-five unit
    // tests agreeing with the cursor, because every one of them looked at the
    // state.
    // =====================================================================

    /// Which cell a field with this hit was drawn at, if it was drawn.
    fn cell_of(rows: &[Row], want: &Hit) -> Option<u16> {
        rows.iter()
            .flat_map(|r| r.fields.iter())
            .find(|f| f.hit == *want)
            .map(|f| f.at)
    }

    /// The rightmost cell anything was drawn into.
    fn rightmost(rows: &[Row]) -> u16 {
        rows.iter()
            .flat_map(|r| r.fields.iter())
            .map(|f| f.at + width_of(&f.text))
            .max()
            .unwrap_or(0)
    }

    /// A probe, not an assertion: where does the LISTING alone leak?
    #[test]
    fn the_listing_alone_leaks_only_where_a_name_cannot_fit() {
        let dir = scratch("leak-probe");
        std::fs::write(dir.join("relatorio.pdf"), b"x").unwrap();
        let mut v = view_at(&dir);
        v.reread();
        let b = bundle();
        // THE FLOOR IS `.max(1)` ON THE NAME, and it is deliberate: "a listing
        // with no name column at all would be a table about files that does not
        // say which". Below the gutter plus the edge plus one cell there is
        // nowhere for that one cell to go, so it leaks — and has since long
        // before there was a sidebar. Pinned here so the next person does not
        // read the sidebar's own test as an accusation against the sidebar.
        let leaks: Vec<u16> = (1..=120u16)
            .filter(|w| rightmost(&v.rows(*w, &b)) > *w)
            .collect();
        assert_eq!(leaks, vec![1], "a listagem sozinha vaza em {leaks:?}");
    }

    #[test]
    fn an_empty_sidebar_takes_no_width_at_all() {
        // A fresh install, a test's home, the nested loop before the
        // directories exist: none of the declared places is there, so the
        // sidebar has nothing to draw. Left "showing", it would take a quarter
        // of the screen and a vertical rule for nothing, and the listing would
        // give up columns to hold it.
        let dir = scratch("side-vazia");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        let mut v = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            // Showing, and with nothing in it.
            ronoctua_files::sidebar::Sidebar::of(
                &[],
                None,
                None,
                ronoctua_files::sidebar::Preferences::fresh(),
                None,
            ),
        );
        v.reread();
        let b = bundle();
        assert!(!v.sidebar_showing());
        assert_eq!(v.split(120), 0);
        // Compared through the DRAWING: the mark is back at cell zero, which is
        // the same thing as "the listing got the whole width".
        assert_eq!(cell_of(&v.rows(120, &b), &Hit::Mark(0)), Some(0));
    }

    #[test]
    fn the_sidebar_withdraws_before_the_name_does() {
        let dir = scratch("side-narrow");
        std::fs::write(dir.join("relatorio.pdf"), b"x").unwrap();
        let mut v = view_beside(&dir, 4);
        v.reread();
        let b = bundle();

        let mut alone = view_at(&dir);
        alone.reread();

        for width in 1..=120u16 {
            let rows = v.rows(width, &b);
            // THE SIDEBAR NEVER MAKES IT WORSE. That is the claim this test
            // owns, and it is the stronger one: the listing on its own already
            // leaks at a width of one cell, because the name's floor is
            // `.max(1)` and a table about files that does not say which is
            // worse than one cell over the edge. `the_listing_alone_leaks_…`
            // pins that, so this cannot be misread as the sidebar's doing.
            assert!(
                rightmost(&rows) <= rightmost(&alone.rows(width, &b)).max(width),
                "em {width} colunas a barra empurrou algo para {}",
                rightmost(&rows)
            );
            // AND THE NAME SURVIVES. A sidebar that took its quarter and left
            // a table of unreadable names would have its priorities backwards.
            assert!(
                rows.iter()
                    .any(|r| r.fields.iter().any(|f| matches!(f.hit, Hit::Name(_)))),
                "em {width} colunas a listagem ficou sem nome"
            );
            let side = cell_of(&rows, &Hit::Sidebar(0)).is_some();
            if width < ronoctua_tokens::space::FILES_SIDEBARFROM {
                assert!(
                    !side,
                    "a barra apareceu em {width} colunas, abaixo do token"
                );
            }
        }
    }

    #[test]
    fn the_sidebar_has_air_on_both_sides_at_every_width() {
        // THE DEFECT THIS EXISTS FOR, and it was reported from the screen: the
        // sidebar's glyph sat at cell zero, against the border, and the eject
        // glyph in the last cell before the rule. A glyph against a border does
        // not read as a glyph at the edge — it reads as one that was CUT OFF,
        // which is the sentence `space.nix` already carries twice about other
        // glyphs. No test looked, because no test looks at white space.
        let dir = scratch("side-ar");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        let mut v = view_with_devices(&dir);
        v.reread();
        let b = bundle();

        for width in ronoctua_tokens::space::FILES_SIDEBARFROM..=200u16 {
            let split = v.split(width);
            if split == 0 {
                continue;
            }
            let rows = v.rows(width, &b);
            for f in rows.iter().flat_map(|r| r.fields.iter()) {
                match f.hit {
                    Hit::Sidebar(_) => assert!(
                        f.at >= SIDE_GUTTER,
                        "em {width} colunas um campo da barra ficou em {} — colado na borda",
                        f.at
                    ),
                    // AND THE RIGHT EDGE: the glyph must not touch the rule at
                    // `split`, or the two read as one smudge instead of as a
                    // target and a boundary.
                    Hit::Eject(_) => assert!(
                        f.at + width_of(&f.text) < split,
                        "em {width} colunas o ejetar terminou em {} e a regua esta em {split}",
                        f.at + width_of(&f.text)
                    ),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn the_sidebar_is_as_wide_as_its_longest_name_and_no_wider() {
        // It used to be `width / 4`: on a 245-column screen, sixty-one cells to
        // hold the word `screenshot`. Every cell the sidebar takes is a cell
        // the listing gives up, and the listing is what one opened the manager
        // for.
        let dir = scratch("side-compacta");
        let mut short = view_beside(&dir, 2);
        short.reread();

        let mut long = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            {
                use ronoctua_files::sidebar::{Item, Preferences, Sidebar, Target};
                let mut s = Sidebar::of(&[], None, None, Preferences::fresh(), None);
                s.add(Item {
                    id: "place:/x".to_owned(),
                    target: Target::Place(PathBuf::from("/x")),
                    label: Some("um nome consideravelmente mais longo".to_owned()),
                });
                s
            },
        );
        long.reread();

        let narrow = short.split(200);
        let wide = long.split(200);
        assert!(
            wide > narrow,
            "a barra nao acompanhou o rotulo: {narrow} vs {wide}"
        );
        // And the short one is nowhere near a quarter of the screen.
        assert!(
            narrow < 200 / 4,
            "a barra tomou {narrow} colunas de 200 para nomes de seis letras"
        );
    }

    #[test]
    fn the_hint_names_the_key_that_reaches_the_sidebar() {
        // `C-o` is not a command of the registry, so it is in neither `M-x` nor
        // the manual. This line is the ONLY thing that teaches it — and it did
        // not, which is the same as the key not existing. Reported as "não está
        // claro como eu pulo da listagem para a sidebar".
        let b = bundle();
        let alone = hint_of(&b, false);
        let beside = hint_of(&b, true);
        assert_ne!(alone, beside);
        let side = key_that_runs("files-sidebar-focus").unwrap().notation();
        assert!(
            beside.contains(&side),
            "a dica com barra nao nomeia `{side}`: {beside:?}"
        );
        // And the key is read from the table, so a rebinding moves the sentence
        // with it — the reason `key_that_runs` exists at all.
        assert!(!alone.contains(&side));
    }

    #[test]
    fn a_menu_leaves_no_sidebar_behind_it() {
        // REPORTED FROM THE SCREEN: the menu opened over a vertical rule running
        // down the middle of it, with the hover band lighting half a row of a
        // list that was no longer there. The rule, the band and the scrollbar
        // are painted OUTSIDE `expansion()` and never consulted the flag that
        // removes the path and the header.
        let dir = scratch("side-menu");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 4);
        v.reread();
        let b = bundle();
        assert!(v.plain() && v.split(120) > 0);

        v.key(of_view("C-m"));
        assert!(v.menu_open());
        // ONE ANSWER, and everything that draws beside the rows asks it.
        assert!(!v.plain());
        assert_eq!(v.split(120), 0, "a regua teria sido desenhada sobre o menu");
        assert!(
            !v.rows(120, &b)
                .iter()
                .flat_map(|r| r.fields.iter())
                .any(|f| matches!(f.hit, Hit::Sidebar(_) | Hit::Eject(_))),
            "sobrou campo da barra por baixo do menu"
        );
    }

    #[test]
    fn the_sidebars_menu_is_the_sidebars_and_not_the_listings() {
        // TWO TABLES, TWO SUBJECTS. The listing's menu asks questions only an
        // entry can answer; a place or a disk answers none of them. Sharing one
        // table would have meant twenty rows saying "not for this kind".
        let dir = scratch("side-menu-tabela");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        let mut v = view_with_devices(&dir);
        v.reread();
        let b = bundle();

        v.key(of_view("C-o"));
        v.key(of_view("C-m"));
        let hits: Vec<Hit> = v
            .rows(120, &b)
            .iter()
            .flat_map(|r| r.fields.iter())
            .map(|f| f.hit.clone())
            .collect();
        assert!(hits.iter().any(|h| matches!(h, Hit::SideMenu(_))));
        assert!(
            !hits.iter().any(|h| matches!(h, Hit::Menu(_))),
            "o menu da listagem apareceu sobre uma linha da barra"
        );

        // And back in the listing it is the other one. `Esc` closes the menu:
        // it is modal, and a key that fell through to the list underneath would
        // move a cursor nobody can see — which is what the listing's menu has
        // always done, and the two behave alike on purpose.
        let esc = Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Esc,
        };
        v.key(esc);
        assert!(!v.menu_open());
        v.key(of_view("C-o"));
        v.key(of_view("C-m"));
        let hits: Vec<Hit> = v
            .rows(120, &b)
            .iter()
            .flat_map(|r| r.fields.iter())
            .map(|f| f.hit.clone())
            .collect();
        assert!(hits.iter().any(|h| matches!(h, Hit::Menu(_))));
        assert!(!hits.iter().any(|h| matches!(h, Hit::SideMenu(_))));
    }

    #[test]
    fn the_menu_row_under_the_cursor_says_what_it_actually_does() {
        // ASKED FOR FROM THE SCREEN: the rows are written for somebody starting
        // out, and somebody who already knows what they want had nowhere to
        // read which call it makes and what can be undone.
        //
        // WHAT IS ASSERTED IS THAT IT FOLLOWS THE CURSOR. A second sentence
        // that sat still would be a caption near a list, and the request said
        // "ao navegar pelas opções com as setas" — the answer has to be about
        // the row about to be chosen, or it is about nothing.
        let dir = scratch("menu-segundo-registro");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        let mut v = view_at(&dir);
        v.reread();
        let b = bundle();

        // WITH NO MENU, NOTHING. On the listing a row is a record with columns,
        // and picking one of its columns to explain because the cursor is
        // somewhere on the line would be picking at random — that is what the
        // pointer's per-field path is for, and it still is.
        assert!(
            v.cursor_explains().is_none(),
            "a listagem explicou uma coluna que ninguem apontou"
        );

        v.key(of_view("C-m"));
        let first = v
            .cursor_explains()
            .expect("a primeira linha do menu explica");
        assert!(
            first.key.starts_with("files-does-"),
            "a explicacao nao e a tecnica: {}",
            first.key
        );
        // AND IT IS A SENTENCE, not a key drawn raw. The bundle has to answer
        // it, or invariant 8 would put `files-does-open` on screen in front of
        // somebody.
        let said = b.t_with(first.key, &first.borrowed());
        assert_ne!(said, first.key, "a chave `{}` nao tem texto", first.key);

        // AND THE ARROW MOVES IT.
        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Down,
        });
        let second = v.cursor_explains().expect("a segunda linha tambem explica");
        assert_ne!(
            first.key, second.key,
            "a linha tecnica nao mudou com a seta: e legenda, nao detalhe"
        );
    }

    #[test]
    fn the_sidebars_menu_explains_itself_too_and_with_its_own_words() {
        // THE SAME SECOND REGISTER, and it has to be the SIDEBAR's: `montar`
        // talks to udisks and `abrir` opens a folder, and an explanation taken
        // from the listing's table would describe the wrong machine entirely.
        let dir = scratch("side-menu-segundo-registro");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        let mut v = view_with_devices(&dir);
        v.reread();
        let b = bundle();

        v.key(of_view("C-o"));
        v.key(of_view("C-m"));
        let e = v.cursor_explains().expect("a linha da barra explica");
        assert!(
            e.key.starts_with("sidebar-does-"),
            "a barra pegou a explicacao da tabela errada: {}",
            e.key
        );
        assert_ne!(
            b.t_with(e.key, &e.borrowed()),
            e.key,
            "a chave `{}` nao tem texto",
            e.key
        );
    }

    #[test]
    fn formatting_the_running_system_is_not_on_any_menu() {
        use ronoctua_files::sidebar::{Preferences, Row as Side, Section, Sidebar, Target};
        let dir = scratch("side-formatar");
        let mut side = Sidebar::of(&[], None, None, Preferences::fresh(), None);
        side.devices_are(vec![Side {
            id: "device:/dev/dm-0".to_owned(),
            target: Target::Device("/block/dm-0".to_owned()),
            label: Some("o sistema".to_owned()),
            section: Section::Devices,
            mounted: Some(true),
            at: Some(PathBuf::from("/")),
            quiet: false,
            system: true,
            needed: Some("/".to_owned()),
            removable: false,
            built_in: true,
        }]);
        let mut v = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        );
        v.reread();
        v.key(of_view("C-o"));
        v.key(of_view("C-m"));
        let b = bundle();
        // THE ROW IS NOT DRAWN AT ALL. A file manager that puts "format" one
        // key away from the disk it booted from is a file manager that erases
        // somebody's system — and a row offered that then refuses teaches that
        // the refusal is arbitrary.
        assert!(
            !v.rows(120, &b)
                .iter()
                .flat_map(|r| r.fields.iter())
                .any(|f| f.hit == Hit::SideMenu("format") || f.hit == Hit::SideMenu("unmount")),
            "o menu ofereceu formatar ou desmontar a raiz"
        );
        // And the facts SAY why the list is short, instead of leaving it to be
        // guessed from rows that are missing.
        let said = v
            .rows(120, &b)
            .iter()
            .flat_map(|r| r.fields.iter())
            .any(|f| f.text.contains('/'));
        assert!(said, "a ficha nao disse onde o sistema mora");
    }

    #[test]
    fn the_right_button_opens_the_menu_of_the_pane_it_was_pressed_in() {
        // BOTH DOORS HAD THIS, and the same way: they resolved the target from
        // the LINE and never looked at the cell, so a right-click in the
        // sidebar opened the listing's menu about whatever file shared the
        // line — and moved the listing's cursor to it.
        let dir = scratch("side-direito");
        for n in 0..3 {
            std::fs::write(dir.join(format!("f{n}.txt")), b"x").unwrap();
        }
        let mut v = view_beside(&dir, 3);
        v.reread();
        let split = v.split(120);
        assert!(split > 0);

        // A cell inside the sidebar.
        v.menu_at(1, SIDE_GUTTER, 120);
        assert!(v.menu_open());
        assert!(
            matches!(v.pane(), Pane::Sidebar),
            "o menu foi para o painel errado"
        );
        v.key(of_view("C-m"));

        // And a cell inside the listing.
        v.menu_at(1, split + RULE + GUTTER + 1, 120);
        assert!(v.menu_open());
        assert!(matches!(v.pane(), Pane::Listing));
    }

    #[test]
    fn what_the_system_needs_gets_no_eject_target_and_says_why() {
        use ronoctua_files::sidebar::{Preferences, Row as Side, Section, Sidebar, Target};
        let dir = scratch("side-essencial");
        let mut side = Sidebar::of(&[], None, None, Preferences::fresh(), None);
        side.devices_are(vec![Side {
            id: "device:/dev/dm-0".to_owned(),
            target: Target::Device("/block/dm-0".to_owned()),
            label: Some("o sistema".to_owned()),
            section: Section::Devices,
            mounted: Some(true),
            at: Some(PathBuf::from("/")),
            quiet: false,
            system: true,
            needed: Some("/".to_owned()),
            removable: false,
            built_in: true,
        }]);
        let mut v = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        );
        v.reread();
        let b = bundle();
        // NO TARGET AT ALL. A row offered that then refuses is worse than one
        // not offered: the gesture was spent to be told no, and nothing says
        // which part of the state decided.
        assert!(
            !v.rows(120, &b)
                .iter()
                .flat_map(|r| r.fields.iter())
                .any(|f| matches!(f.hit, Hit::Eject(_))),
            "ofereceu ejetar a raiz do sistema"
        );
        // And the key refuses NAMING what decided.
        v.key(of_view("C-o"));
        match v.key(of_view("C-e")) {
            Step::Say(e) => {
                assert_eq!(e.key, ronoctua_i18n::key::FILES_UNMOUNT_NEEDED);
                assert!(e.args.iter().any(|(k, val)| *k == "why" && val == "/"));
            }
            other => panic!("desmontar a raiz devia ser recusado com motivo: {other:?}"),
        }
    }

    #[test]
    fn the_mark_travels_with_the_listing() {
        let dir = scratch("side-mark");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();

        let mut beside = view_beside(&dir, 3);
        beside.reread();
        let mut alone = view_at(&dir);
        alone.reread();
        let b = bundle();

        let split = beside.split(120);
        assert!(split > 0, "a barra deveria caber em 120 colunas");
        // THE MARK IS THE LISTING'S, so it moves with it. Left at cell zero it
        // sits under the sidebar's last letter, and clicking it marks the wrong
        // thing — which nothing else in this file would have caught.
        assert_eq!(
            cell_of(&beside.rows(120, &b), &Hit::Mark(0)),
            Some(split + RULE)
        );
        assert_eq!(cell_of(&alone.rows(120, &b), &Hit::Mark(0)), Some(0));
        // And the columns themselves are resolved identically — the sidebar is
        // not a column and does not degrade with them.
        assert_eq!(layout_beside(120, 0), layout_beside(120, 0));
    }

    #[test]
    fn hiding_the_sidebar_gives_the_width_back_to_the_listing() {
        let dir = scratch("side-toggle");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 3);
        v.reread();
        let b = bundle();

        let columns = |v: &View, w: u16| -> usize {
            v.header(w, &b)
                .fields
                .iter()
                .filter(|f| matches!(f.hit, Hit::Sort(_)))
                .count()
        };
        let widths: Vec<u16> = (ronoctua_tokens::space::FILES_SIDEBARFROM..=140).collect();
        let with: Vec<usize> = widths.iter().map(|w| columns(&v, *w)).collect();
        v.key(of_view("C-l"));
        let without: Vec<usize> = widths.iter().map(|w| columns(&v, *w)).collect();

        // COMPARED THROUGH THE DRAWING and not through the flag: a toggle that
        // flipped a boolean and changed nothing on screen is exactly the defect
        // this file records having shipped once, with the arrows working and
        // the screen showing nothing.
        assert!(
            with.iter().zip(&without).all(|(a, b)| b >= a),
            "esconder a barra tirou colunas da listagem"
        );
        assert!(
            with.iter().zip(&without).any(|(a, b)| b > a),
            "esconder a barra nao devolveu largura em nenhuma das {} larguras",
            widths.len()
        );
    }

    #[test]
    fn a_sidebar_longer_than_the_listing_still_draws_every_row() {
        let dir = scratch("side-longer");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        std::fs::write(dir.join("dois.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 6);
        v.reread();
        let b = bundle();

        assert_eq!(v.row_count(120), Some(6));
        let rows = v.rows_from(0, 12, 120, &b);
        assert_eq!(rows.len(), 6);
        // The rows past the listing's end carry the sidebar and NOTHING else —
        // no empty listing fields pretending to be entries.
        for (n, row) in rows.iter().enumerate().skip(2) {
            assert!(
                row.fields.iter().any(|f| f.hit == Hit::Sidebar(n)),
                "a linha {n} perdeu a barra"
            );
            assert!(
                !row.fields.iter().any(|f| matches!(f.hit, Hit::Name(_))),
                "a linha {n} inventou uma entrada"
            );
        }
        // And a click inside the sidebar's cells of row five finds row five.
        let inside = rows[5].field_at(1).expect("celula da barra");
        assert_eq!(inside.hit, Hit::Sidebar(5));
    }

    #[test]
    fn an_empty_directory_keeps_its_sidebar() {
        let dir = scratch("side-empty");
        let mut v = view_beside(&dir, 3);
        v.reread();
        let b = bundle();
        // The count is the SIDEBAR's, not one. Before this the footer answered
        // `1` from `expansion_len` and the window door drew nothing at all and
        // said nothing either.
        assert_eq!(v.row_count(120), Some(3));
        let rows = v.rows_from(0, 12, 120, &b);
        assert_eq!(rows.len(), 3);
        assert!(rows[0].fields.iter().any(|f| f.hit == Hit::Sidebar(0)));
    }

    #[test]
    fn a_blank_line_separates_the_blocks_and_the_arrow_steps_over_it() {
        use ronoctua_files::sidebar::{Item, Preferences, Row as Side, Section, Sidebar, Target};
        let dir = scratch("side-gap");
        let mut side = Sidebar::of(&[], None, None, Preferences::fresh(), None);
        side.add(Item {
            id: "place:/um".to_owned(),
            target: Target::Place(PathBuf::from("/um")),
            label: Some("um".to_owned()),
        });
        side.devices_are(vec![Side {
            id: "device:/dev/sda1".to_owned(),
            target: Target::Device("/block/sda1".to_owned()),
            label: Some("stick".to_owned()),
            section: Section::Devices,
            mounted: Some(true),
            at: Some(PathBuf::from("/run/media/alguem/stick")),
            quiet: false,
            system: false,
            needed: None,
            removable: true,
            built_in: true,
        }]);
        let mut v = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        );
        v.reread();
        let b = bundle();
        let rows = v.rows(120, &b);

        // THREE LINES FOR TWO ROWS: the place, a blank, the device. That blank
        // is the ONLY separation there is — no heading, no rule (invariants 14
        // and 15) — so if it is not drawn there is none at all.
        assert_eq!(v.row_count(120), Some(3));
        assert!(rows[0].fields.iter().any(|f| f.hit == Hit::Sidebar(0)));
        assert!(
            !rows[1]
                .fields
                .iter()
                .any(|f| matches!(f.hit, Hit::Sidebar(_))),
            "a linha do meio devia estar em branco: {:?}",
            rows[1].fields
        );
        assert!(rows[2].fields.iter().any(|f| f.hit == Hit::Sidebar(2)));

        // AND ONE PRESS OF THE ARROW CROSSES IT. Stopping on a separator would
        // be an arrow that appears to do nothing.
        v.key(of_view("C-o"));
        assert!(matches!(v.key(of_view("C-n")), Step::Changed));
        let step = v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Enter,
        });
        assert!(
            matches!(step, Step::Changed) && v.where_().ends_with("stick"),
            "uma seta devia ter chegado ao dispositivo, nao ao vao"
        );
    }

    #[test]
    fn crossing_into_the_sidebar_and_back_lands_where_it_left() {
        let dir = scratch("side-cross");
        for n in 0..4 {
            std::fs::write(dir.join(format!("f{n}.txt")), b"x").unwrap();
        }
        let mut v = view_beside(&dir, 4);
        v.reread();
        let b = bundle();

        // Where the accent is, read off the DRAWING.
        let accent = |v: &View| -> Vec<Hit> {
            v.rows(120, &b)
                .iter()
                .flat_map(|r| r.fields.iter())
                .filter(|f| f.color == ronoctua_tokens::color::STATE_ACCENT && !f.text.is_empty())
                .map(|f| f.hit.clone())
                .collect()
        };

        v.key(of_view("C-n"));
        let in_listing = accent(&v);
        v.key(of_view("C-o"));
        v.key(of_view("C-n"));
        v.key(of_view("C-n"));
        assert!(
            accent(&v).contains(&Hit::Sidebar(2)),
            "o acento nao foi para a barra: {:?}",
            accent(&v)
        );
        v.key(of_view("C-o"));
        assert_eq!(accent(&v), in_listing, "voltar nao caiu onde estava");
        v.key(of_view("C-o"));
        assert!(
            accent(&v).contains(&Hit::Sidebar(2)),
            "atravessar de novo nao caiu onde estava"
        );
    }

    #[test]
    fn typing_still_goes_to_the_query_with_the_sidebar_focused() {
        let dir = scratch("side-typing");
        std::fs::write(dir.join("alfa.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 3);
        v.reread();

        v.key(of_view("C-o"));
        v.key(press('a'));
        // THE RULE THE WHOLE KEYBOARD WAS REBUILT AROUND: in a list of names
        // the letter is text. A pane that swallowed letters would break it in
        // the one place nobody would think to test.
        let query = v.crumbs().last().expect("o caminho tem campos").clone();
        assert_eq!(query.text, "a");
        assert_eq!(query.color, ronoctua_tokens::color::STATE_ACCENT);
        assert!(v.filtering());
    }

    #[test]
    fn esc_leaves_the_sidebar_before_it_leaves_the_view() {
        let dir = scratch("side-esc");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 3);
        v.reread();
        let esc = Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Esc,
        };

        v.key(of_view("C-o"));
        v.key(press('u'));
        // Three levels, one press each, and never two at a time.
        assert!(
            matches!(v.key(esc), Step::Changed),
            "a query devia sair primeiro"
        );
        assert!(
            matches!(v.key(esc), Step::Changed),
            "a barra devia sair depois"
        );
        assert!(matches!(v.key(esc), Step::Closed), "so entao a vista fecha");
    }

    /// A sidebar holding one mounted device and one that is not.
    fn view_with_devices(dir: &Path) -> View {
        use ronoctua_files::sidebar::{Preferences, Row as Side, Section, Sidebar, Target};
        let mut side = Sidebar::of(&[], None, None, Preferences::fresh(), None);
        side.devices_are(vec![
            Side {
                id: "device:/dev/sda1".to_owned(),
                target: Target::Device("/block/sda1".to_owned()),
                label: Some("MJRO1812".to_owned()),
                section: Section::Devices,
                mounted: Some(true),
                at: Some(PathBuf::from("/run/media/alguem/MJRO1812")),
                quiet: false,
                system: false,
                needed: None,
                removable: true,
                built_in: true,
            },
            Side {
                id: "device:/dev/sda2".to_owned(),
                target: Target::Device("/block/sda2".to_owned()),
                label: Some("MISO_EFI".to_owned()),
                section: Section::Devices,
                mounted: Some(false),
                at: None,
                quiet: false,
                system: false,
                needed: None,
                removable: true,
                built_in: true,
            },
        ]);
        View::at(
            dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        )
    }

    #[test]
    fn the_glyph_says_which_kind_of_disk_and_not_whether_it_is_mounted() {
        use ronoctua_files::sidebar::{Preferences, Row as Side, Section, Sidebar, Target};
        let dir = scratch("side-glyph");
        let a_row = |id: &str, mounted: bool, removable: bool| Side {
            id: format!("device:{id}"),
            target: Target::Device(id.to_owned()),
            label: Some(id.to_owned()),
            section: Section::Devices,
            mounted: Some(mounted),
            at: mounted.then(|| PathBuf::from("/run/media/alguem").join(id)),
            quiet: false,
            system: false,
            needed: None,
            removable,
            built_in: true,
        };
        let mut side = Sidebar::of(&[], None, None, Preferences::fresh(), None);
        side.devices_are(vec![
            a_row("stick-desmontado", false, true),
            a_row("interna-montada", true, false),
            a_row("interna-desmontada", false, false),
        ]);
        let mut v = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        );
        v.reread();
        let b = bundle();
        let rows = v.rows(120, &b);
        let glyph = |n: usize| -> String {
            rows.iter()
                .flat_map(|r| r.fields.iter())
                .find(|f| f.hit == Hit::Sidebar(n) && f.text.chars().count() == 1)
                .expect("o glifo")
                .text
                .clone()
        };
        // THE STICK IS A STICK WHETHER OR NOT IT IS MOUNTED, and an internal
        // partition is an internal partition. Deriving the drawing from
        // `mounted` — which is what a `mounted.is_none()` test would do — would
        // redraw the internal one as a stick the moment somebody unmounted it,
        // and the two rows below would come out identical.
        assert_ne!(glyph(0), glyph(1));
        assert_eq!(glyph(1), glyph(2), "montar nao muda que tipo de disco e");
    }

    #[test]
    fn an_unmounted_device_reads_differently_from_a_mounted_one() {
        let dir = scratch("side-devices");
        let mut v = view_with_devices(&dir);
        v.reread();
        let b = bundle();
        let rows = v.rows(120, &b);

        let field = |n: usize| -> Field {
            rows.iter()
                .flat_map(|r| r.fields.iter())
                .find(|f| f.hit == Hit::Sidebar(n) && !f.text.is_empty() && f.text.len() > 4)
                .expect("o nome do dispositivo")
                .clone()
        };
        let mounted = field(0);
        let not = field(1);
        // TWO CHANNELS, TWO FACTS. The colour says whether one can walk in; the
        // sentence says what pressing it would do. Either alone leaves the
        // other unanswered.
        assert_ne!(mounted.color, not.color);
        assert_ne!(
            mounted.explain.as_ref().map(|e| e.key),
            not.explain.as_ref().map(|e| e.key)
        );
        assert_eq!(
            not.explain.as_ref().map(|e| e.key),
            Some(ronoctua_i18n::key::FILES_SIDEBAR_MOUNT)
        );

        // AND THE EJECT GLYPH IS ON THE MOUNTED ONE ONLY. A target offered
        // where it does nothing is what `menu::Applies` exists to refuse.
        assert!(cell_of(&rows, &Hit::Eject(0)).is_some());
        assert!(cell_of(&rows, &Hit::Eject(1)).is_none());
    }

    #[test]
    fn pressing_an_unmounted_device_shows_it_instead_of_mounting_it() {
        // THE BEHAVIOUR THAT WAS ASKED FOR, and this test used to assert the
        // opposite: *"ao clicar em algum item da sidebar que não está montado,
        // não deve-se montá-lo automaticamente; em vez disso, mostrar
        // informações sobre o item, e mostrar a opção de montá-lo"*.
        //
        // Mounting asks the system for something — it takes a moment, it can be
        // refused, and it changes what is on the machine. A row one presses to
        // LOOK at a disk must not do that.
        let dir = scratch("side-mount");
        let mut v = view_with_devices(&dir);
        v.reread();
        v.key(of_view("C-o"));
        v.key(of_view("C-n"));

        let step = v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Enter,
        });
        assert!(
            matches!(step, Step::Changed) && v.menu_open(),
            "esperava a ficha do dispositivo, veio {step:?}"
        );

        // AND MOUNTING IS THERE, with its key beside it — which is the other
        // half of what was asked for.
        let b = bundle();
        let rows = v.rows(120, &b);
        assert!(
            rows.iter()
                .flat_map(|r| r.fields.iter())
                .any(|f| f.hit == Hit::SideMenu("mount")),
            "a ficha nao ofereceu montar"
        );
        // The facts are on the same surface: the top line names the device.
        assert!(rows[0].fields[0].text.contains("MISO_EFI"));

        // And pressing that row is what mounts.
        match v.key(press('m')) {
            Step::Do(Doing::Mount(ref id)) => assert_eq!(id, "/block/sda2"),
            other => panic!("a linha `montar` devia montar, veio {other:?}"),
        }
    }

    #[test]
    fn a_system_volume_offers_mounting_and_then_names_what_it_needs() {
        use ronoctua_files::sidebar::{Preferences, Row as Side, Section, Sidebar, Target};
        let dir = scratch("side-sistema");
        let mut side = Sidebar::of(&[], None, None, Preferences::fresh(), None);
        side.devices_are(vec![Side {
            id: "device:/dev/nvme0n1p3".to_owned(),
            target: Target::Device("/block/nvme0n1p3".to_owned()),
            label: Some("uma particao interna".to_owned()),
            section: Section::Devices,
            mounted: Some(false),
            at: None,
            quiet: false,
            system: true,
            // NOT `needed`: it is unmounted and not in fstab, so it is spare —
            // what makes it refuse is that mounting it costs the admin.
            needed: None,
            removable: false,
            built_in: true,
        }]);
        let mut v = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        );
        v.reread();
        v.key(of_view("C-o"));
        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Enter,
        });

        // THE ROW IS OFFERED, and that is deliberate: an unmounted internal
        // partition with no `mount` row and nothing saying why would be worse
        // than one that says what it needs. Pressing it does NOT try —
        // `filesystem-mount-system` is `auth_admin_keep` and this session has
        // no authentication agent, so the call would come back refused.
        match v.key(press('m')) {
            Step::Say(e) => assert_eq!(e.key, ronoctua_i18n::key::FILES_MOUNT_NEEDS_ADMIN),
            Step::Do(Doing::Mount(_)) => {
                panic!(
                    "tentou montar um volume de sistema, que o polkit recusa sem ter a quem perguntar"
                )
            }
            other => panic!("esperava a frase que oferece o terminal root, veio {other:?}"),
        }
    }

    #[test]
    fn each_refusal_gets_its_own_sentence() {
        use ronoctua_disks::Refusal;
        let dir = scratch("side-recusa");
        let v = view_with_devices(&dir);
        // FOUR ANSWERS A PERSON CAN ACT ON, and they have to be four different
        // sentences — carrying the D-Bus error's NAME instead of its prose is
        // pointless if they all collapse into "could not mount".
        let keys: Vec<&str> = [
            Refusal::NoDriver,
            Refusal::NeedsAdmin,
            Refusal::Busy,
            Refusal::Other,
        ]
        .into_iter()
        .map(|r| v.refusal_said(r, "seja lá o que for").key)
        .collect();
        let unique: std::collections::BTreeSet<&&str> = keys.iter().collect();
        assert_eq!(
            unique.len(),
            keys.len(),
            "duas recusas dizem a mesma coisa: {keys:?}"
        );
    }

    #[test]
    fn every_sidebar_key_says_something_when_it_does_not_apply() {
        let dir = scratch("side-says");
        std::fs::write(dir.join("um.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 2);
        v.reread();
        // Pressed from the LISTING, where neither has a row to act on. A key
        // that did nothing and said nothing is indistinguishable from a key
        // that never arrived — the report this project has had three times,
        // with three different causes and the same screen.
        for notation in ["C-e", "C-u"] {
            assert!(
                matches!(v.key(of_view(notation)), Step::Say(_)),
                "`{notation}` ficou calada na listagem"
            );
        }
    }

    #[test]
    fn the_sidebars_rows_explain_themselves() {
        let dir = scratch("side-explains");
        let mut v = view_with_devices(&dir);
        v.reread();
        let b = bundle();
        // Invariant 14's other half: there is no floating tooltip and there
        // will not be one, so what the pointer is over goes on the message
        // line — and a field that explains nothing has nothing to put there.
        for f in v.rows(120, &b).iter().flat_map(|r| r.fields.iter()) {
            if matches!(f.hit, Hit::Eject(_))
                || (matches!(f.hit, Hit::Sidebar(_)) && f.text.len() > 4)
            {
                assert!(f.explain.is_some(), "{f:?} nao se explica");
            }
        }
    }

    #[test]
    fn taking_a_default_off_the_sidebar_says_something_different_from_removing_one() {
        let dir = scratch("side-unpin");
        let mut v = view_beside(&dir, 2);
        v.reread();
        v.key(of_view("C-o"));
        let first = v.key(of_view("C-u"));
        let second = v.key(of_view("C-u"));
        // Both are the person's own rows here, so both are removals — what is
        // being pinned down is that the key ACTS and says so, twice running,
        // with the cursor clamped in between rather than left past the end.
        assert!(matches!(first, Step::Say(_)));
        assert!(matches!(second, Step::Say(_)));
        assert_eq!(v.row_count(120), Some(1), "a barra devia ter esvaziado");
    }

    #[test]
    fn every_view_key_in_the_table_actually_does_something() {
        // The other half of `files-everything-has-a-key`, as a unit test: a
        // key listed in `OF_THE_VIEW` and missing from the `match` is a line in
        // the manual promising something that does not happen.
        let root = scratch("keys");
        for n in ["a.txt", "b.txt", "c.txt"] {
            std::fs::write(root.join(n), b"x").unwrap();
        }
        std::fs::create_dir(root.join("pasta")).unwrap();
        for (notation, label) in OF_THE_VIEW {
            let mut v = view_at(&root);
            // THE CURSOR IN THE MIDDLE. At the top, `k` moves nowhere and
            // answers `Nothing` — correctly — and the test would read that as
            // "the key is not handled". The question here is whether the table
            // and the `match` agree, not whether a clamp clamps.
            v.cursor = 1;
            let t = ronoctua_keymap::Key::parse(notation)
                .unwrap_or_else(|_| panic!("`{notation}` nao e uma tecla valida"));
            let step = v.key(t);
            assert!(
                !matches!(step, Step::Nothing),
                "a tecla `{notation}` (`{label}`) nao faz nada"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
