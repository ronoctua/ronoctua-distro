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

/// Which columns fit, and where.
///
/// Integer arithmetic in two phases, the same shape as `layout::resolve`:
/// everything is asked for, then the lowest-priority column gives way until
/// what is left fits. What never gives way is "nothing leaks off the screen" —
/// the compositor does not clip.
#[must_use]
pub fn layout_for(width: u16) -> Layout {
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
        columns.push((c, at, w));
        at += w + gap;
    }
    Layout { columns }
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
    /// A choice of the open picker.
    Choice(usize),
    /// Nothing happens here.
    Nothing,
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

impl Row {
    /// Which field a cell belongs to.
    ///
    /// Walks from the right, so a click in the gap after a field belongs to
    /// that field rather than to nothing: a one-cell dead zone between columns
    /// is invisible and feels like a broken click.
    #[must_use]
    pub fn field_at(&self, cell: u16) -> Option<&Field> {
        self.fields
            .iter()
            .rev()
            .find(|f| cell >= f.at && cell < f.at + width_of(&f.text) + 1)
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
    ) -> Self {
        let mut view = Self {
            texts,
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
        let layout = layout_for(width);
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
        let layout = layout_for(width);
        self.shown
            .iter()
            .enumerate()
            .skip(skip)
            .take(take)
            // `n` STAYS ABSOLUTE. It is what says which row has the cursor and
            // which is marked, and a window that renumbered from zero would put
            // the focus on the first visible line whatever the offset.
            .map(|(n, i)| self.row_of(n, *i, &layout))
            .collect()
    }

    /// How many rows a listing has, WITHOUT building one.
    ///
    /// The count is what the height, the page count and the scrollbar need, and
    /// they needed it several times per frame. See [`View::rows_from`].
    #[must_use]
    pub fn row_count(&self) -> Option<usize> {
        // `None` for what is NOT a listing — the picker, the detail sheet, the
        // menu. They are a dozen rows by construction, so counting them means
        // building them, and the caller may as well. An `Option` and not a
        // sentinel: a `usize::MAX` meaning "ask again" is the kind of number
        // that ends up in arithmetic.
        (self.picking.is_none() && self.detail.is_none() && self.menu.is_none())
            .then_some(self.shown.len())
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
            at: 0,
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
                            explain: None,
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
                            explain: None,
                        },
                    ],
                    marked: false,
                    focused: n == chosen,
                }
            }))
            .collect()
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
        // UNBOUNDED: to a thread, every one of them.
        Doing::Trash(root, paths) => {
            in_a_thread(sender, "lixeira", move || {
                let bin = ronoctua_files::trash::Bin::at(root);
                let zone =
                    ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
                for p in &paths {
                    bin.put(p, now, &zone)?;
                }
                Ok(())
            });
            Errand::Done
        }
        Doing::Restore(root, names) => {
            in_a_thread(sender, "restaurar", move || {
                let bin = ronoctua_files::trash::Bin::at(root);
                for n in &names {
                    bin.restore(n)?;
                }
                Ok(())
            });
            Errand::Done
        }
        Doing::Delete(paths) => {
            in_a_thread(sender, "apagar", move || {
                for p in &paths {
                    ronoctua_files::act::remove(p)?;
                }
                Ok(())
            });
            Errand::Done
        }
        Doing::Shred(paths) => {
            in_a_thread(sender, "sobrescrever", move || {
                for p in &paths {
                    ronoctua_files::act::shred(p, ronoctua_files::act::PASSES)?;
                }
                Ok(())
            });
            Errand::Done
        }
        Doing::Paste(into) => {
            let clip = clip.clone();
            in_a_thread(sender, "colar", move || {
                for p in &clip.paths {
                    if clip.moves {
                        ronoctua_files::act::move_into(p, &into)?;
                    } else {
                        ronoctua_files::act::copy_into(p, &into)?;
                    }
                }
                Ok(())
            });
            Errand::Done
        }
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
            in_a_thread(sender, "comprimir", move || {
                ran(ronoctua_files::act::packing(&directory, &names, &archive)?)
            });
            Errand::Done
        }
        Doing::Extract(archive) => {
            let Some(directory) = archive.parent().map(Path::to_path_buf) else {
                return Errand::Done;
            };
            in_a_thread(sender, "extrair", move || {
                ran(ronoctua_files::act::unpacking(&archive, &directory)?)
            });
            Errand::Done
        }
        // THE WALK. Unbounded by definition, and the one operation somebody
        // will be looking at the screen during.
        Doing::Measure(path) => {
            let sender = sender.clone();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            raise("medir-pasta", move || {
                let bytes = ronoctua_files::act::weigh(&path);
                let _ = sender.send(Report::Measured(name, bytes));
            });
            Errand::Done
        }
    }
}

/// Waits for a child and turns its failure into its own words.
fn ran(mut cmd: std::process::Command) -> anyhow::Result<()> {
    let out = cmd.output()?;
    if out.status.success() {
        return Ok(());
    }
    Err(anyhow::anyhow!(
        "{}",
        String::from_utf8_lossy(&out.stderr).trim().to_owned()
    ))
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
fn size_text(bytes: u64) -> String {
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
#[must_use]
pub fn hint(texts: &ronoctua_i18n::Texts) -> String {
    let menu = key_that_runs("files-menu").map_or_else(String::new, |t| t.notation());
    texts.t_with(ronoctua_i18n::key::FILES_HINT, &[("key", menu.as_str())])
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
            if self.filter.is_empty() {
                return Step::Closed;
            }
            self.filter.clear();
            self.refilter();
            self.clamp();
            return Step::Changed;
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
                crate::minibuffer::Gesture::Down => return self.move_by(1),
                crate::minibuffer::Gesture::Up => return self.move_by(-1),
                crate::minibuffer::Gesture::Confirm => return self.open_focused(),
                crate::minibuffer::Gesture::Cancel | crate::minibuffer::Gesture::Nothing => {}
            }
        }
        match of_the_view(t) {
            Some("files-down") => self.move_by(1),
            Some("files-up") => self.move_by(-1),
            Some("files-enter") => self.open_focused(),
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
                self.menu = Some(0);
                self.arming = None;
                Step::Changed
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

    /// A click on a cell of a row.
    ///
    /// Every branch here goes through the SAME code the key goes through. That
    /// is not tidiness: it is what keeps the two from drifting, which is how a
    /// file manager ends up with a mouse gesture that does something no key
    /// does — and then somebody has to reach for the mouse.
    pub fn click(&mut self, row: usize, hit: &Hit) -> Step {
        match hit {
            Hit::Nothing => Step::Nothing,
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

    fn view_at(dir: &Path) -> View {
        View::at(
            dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
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
        let wide = layout_for(120);
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
            let l = layout_for(width);
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
            let l = layout_for(width);
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
        let (at, _) = layout_for(120).find(Column::Perms).unwrap();
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
        let name_at = layout_for(120).find(Column::Name).unwrap().0;

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
            let l = layout_for(width);
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
        let (at, _) = layout_for(120).find(Column::Name).unwrap();
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
        let (at, _) = layout_for(120).find(Column::Size).unwrap();
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
