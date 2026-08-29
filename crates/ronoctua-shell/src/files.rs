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
    /// The name this column crosses to `ronoctua_design::Table` under.
    ///
    /// A `&'static str` because that crate carries no buffer's and no shell's
    /// vocabulary — it resolves ids, and the caller finds its own field again.
    const fn id(self) -> &'static str {
        match self {
            Self::Icon => "icon",
            Self::Name => "name",
            Self::Perms => "perms",
            Self::Owner => "owner",
            Self::Group => "group",
            Self::Size => "size",
            Self::Modified => "modified",
        }
    }

    /// The column an id names, if it is one of ours.
    fn of_id(id: &str) -> Option<Self> {
        [
            Self::Icon,
            Self::Name,
            Self::Perms,
            Self::Owner,
            Self::Group,
            Self::Size,
            Self::Modified,
        ]
        .into_iter()
        .find(|c| c.id() == id)
    }

    const fn width(self) -> u16 {
        match self {
            // THE GLYPH, AND NOTHING ELSE. The air is the columns' own `gap`,
            // which `layout_beside` already adds after every column.
            //
            // It used to be two — "the glyph plus one cell of air" — and the
            // gap was added on top, so the icon had TWO cells between its ink
            // and the first letter of the name while the whole table had one
            // between any other pair of columns. Measured off the owner's
            // screen: 25 px on the icon's right against 21 on its left, in a
            // grid whose cell is 10,8. *"The icon spacing is far too wide
            // in the listing."*
            //
            // One cell of air is what every other column gets, and it is what
            // this one gets now. The air itself did not go anywhere; it stopped
            // being paid twice.
            Self::Icon => 1,
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

/// The narrowest a PANEL may be squeezed to.
///
/// `Dimensions  3024 × 4032` is twenty-three cells and it is the widest
/// ordinary fact there is; twenty-four is that plus the air on its right. Below
/// it the value elides on the row that exists to carry the value, which is the
/// same argument [`NAME_FLOOR`] makes about a name and [`SIDE_FLOOR`] about a
/// place.
const PANEL_FLOOR: u16 = 24;

/// The widest a panel is ever given.
///
/// Forty cells is some 340 px at the window's font — past the headroom
/// `space.files.panelPixels` already reserves — and every cell beyond it is a
/// cell the LISTING gives up. The sidebar's ceiling makes the same argument
/// with `width / 3`; here the number is flat because a panel's content does not
/// grow with the screen: a fact is as long as it is.
const PANEL_CEILING: u16 = 40;

/// How many lines a PREVIEW wants before it is a picture rather than a smudge.
///
/// The same number the bottom strip takes, and deliberately the same: it is one
/// answer to one question — "how short may a preview get and still be worth
/// drawing" — and two numbers for it would be two answers.
fn preview_lines() -> u16 {
    ronoctua_tokens::space::FILES_STRIPLINES
}

/// What the toggle says when it turns a panel on at a width that cannot hold
/// it.
///
/// The state flips ANYWAY and the screen says why — the shape
/// `files-sidebar-only` already has, and for the same reason: from the outside,
/// a key that refuses and a key that is dead look the same.
const PANEL_NO_ROOM: &str = ronoctua_i18n::key::FILES_PANEL_NO_ROOM;

/// What the preview panel says when there is no picture in it.
///
/// Not "the file could not be read" — that is what `ronoctua_inspect`'s own
/// refusals say, and they win when there is one. This is the answer for a
/// directory, a broken link and a file the tools have not been asked about
/// yet: there is no picture, and the panel says so instead of drawing a large
/// icon that says what the listing's own icon column already said.
const NO_PREVIEW: &str = ronoctua_i18n::key::FILES_NO_PREVIEW;

/// What the facts key says when there is nothing to scroll.
///
/// A key that does nothing and says nothing is the one outcome this project
/// treats as a defect: from the outside, a refusal and a dead key look the
/// same. It is `files-sidebar-only`'s shape, once more.
const FACTS_ALL_SHOWN: &str = ronoctua_i18n::key::FILES_FACTS_ALL_SHOWN;

/// One fact, in full, on the message line.
///
/// A panel is [`PANEL_FLOOR`] cells and a path is two hundred characters, so
/// the row that had to be cut is exactly the row somebody wants to read whole.
/// It goes where which-key already puts a description; nothing floats
/// (invariant 14).
const FACT_SAYS: &str = ronoctua_i18n::key::FILES_FACT_SAYS;

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
    if width.saturating_sub(wanted + RULE) < listing_floor() {
        return 0;
    }
    wanted
}

/// One of the two panels the manager can put beside — or under — the listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// What the file LOOKS like: a picture, or the type's glyph and the reason
    /// there is none.
    Preview,
    /// What the file IS, in as much detail as the tools on the machine can say.
    Info,
}

/// One row of the info panel: what it says, and what it offers.
///
/// The label and the value are already WORDS by the time they are here —
/// `sheet_of` resolves them through the view's own bundle — because a `Choice`
/// and a filename follow the same rule: text on screen is text, and a key that
/// reached the screen is the defect invariant 8 exists to refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    /// What it is called.
    pub label: String,
    /// What it says.
    pub value: String,
    /// What acting on it reaches — and [`Doable::Nothing`] for the facts one
    /// only reads.
    pub does: Doable,
}

/// What a row of the info panel offers, if anything.
///
/// **Every variant is something the LISTING already reaches**, and that is the
/// whole point of the enum: the panel is a second place to press the same
/// targets, never a second implementation of them. A variant here with no
/// counterpart in `row_of` would be a gesture that only exists in one of the
/// two places one can see the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doable {
    /// Nothing. A date is not something one edits, a path is not somewhere one
    /// types, and a file's size is what the kernel says it is. A row that lit
    /// up and then refused would be worse than one that never promised —
    /// `menu::Applies` states that rule and this is it, one register down.
    Nothing,
    /// The permission triplet: one target per character, as `Column::Perms`.
    Mode,
    /// `owner:group`: two targets, as `Column::Owner` and `Column::Group`.
    Ownership,
    /// A directory's size, which is a walk — as `TAB` on a directory.
    Measure,
}

/// How many cells the two panels take on the RIGHT — and ZERO when they withdraw.
///
/// Two phases, the same shape as [`sidebar_columns`], [`layout_beside`] and
/// `layout::resolve` on the modeline — and mirrored deliberately, because it is
/// the same question asked at the other edge.
///
/// **Phase one is a PREFERENCE.** Below `space.files.panelsFrom` the panels are
/// not offered on the right at all. They are not refused: they move to a short
/// strip along the bottom, which is [`View::panes`]' business and not this
/// function's.
///
/// **Phase two is what does not give way** — nothing leaks off the screen. Each
/// panel is a quarter of what is left, floored at [`PANEL_FLOOR`] and capped at
/// [`PANEL_CEILING`]; and if what remains cannot hold a listing at its own floor
/// the pair withdraws ENTIRELY rather than shrinking. Half a panel beside half a
/// listing is two things neither of which can be read — the sentence the sidebar
/// already carries, applied to the other edge.
///
/// `width` is what is left AFTER the sidebar, and that is not a detail: the
/// sidebar is decided first, so turning a panel on never moves the places one
/// navigates by. `wanted` is how many panels are on — one or two.
#[must_use]
/// What a listing needs before it says anything.
///
/// The gutter, the icon and its gap, the name's floor, and the right edge.
///
/// # Why it has a name
///
/// Because it was written out twice — once in [`sidebar_columns`] and once in
/// [`right_columns`] — with a comment on the second saying "the same
/// expression `sidebar_columns` uses, and it has to be the same or the two
/// edges disagree about what a listing is". A comment saying two things have
/// to be equal is not what makes them equal.
///
/// The two edges ask the same question — *is there still a listing left?* — so
/// there is one answer to ask.
fn listing_floor() -> u16 {
    GUTTER + Column::Icon.width() + 1 + NAME_FLOOR + EDGE_RIGHT
}

pub fn right_columns(width: u16, showing: bool, wanted: u16) -> u16 {
    if !showing || wanted == 0 || width < ronoctua_tokens::space::FILES_PANELSFROM {
        return 0;
    }
    let each = (width / 4).clamp(PANEL_FLOOR, PANEL_CEILING);
    // ONE RULE BEFORE EACH PANEL — the one that separates the first from the
    // listing, and the one between the two — plus the right edge's air, which
    // is not a column and never gives way. See `EDGE_RIGHT`.
    let take = each
        .saturating_mul(wanted)
        .saturating_add(RULE.saturating_mul(wanted))
        .saturating_add(EDGE_RIGHT);
    if width.saturating_sub(take) < listing_floor() {
        return 0;
    }
    take
}

/// Whether the two panels fit side by side in a strip along the bottom.
///
/// The strip spans the whole width, so the only question is whether each panel
/// still reaches its floor. Below that they do not shrink — they do not appear,
/// and the key that turned one on SAYS so.
#[must_use]
pub fn strip_fits(width: u16, wanted: u16) -> bool {
    if wanted == 0 {
        return false;
    }
    let rules = wanted.saturating_sub(1).saturating_mul(RULE);
    width
        >= PANEL_FLOOR
            .saturating_mul(wanted)
            .saturating_add(rules)
            .saturating_add(EDGE_RIGHT)
}

/// Where every region of the manager begins, at one width.
///
/// # Why one struct and not four functions
///
/// Because `View::split` already records what happens when the division is
/// computed in more than one place: both painters and both hit-testers asked
/// `sidebar_columns` for themselves, and on a second monitor the rule and the
/// fields landed at different cells. With FOUR regions — the sidebar, the
/// listing and two panels — that argument only gets stronger, so the whole
/// division is decided once and handed over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Panes {
    /// Where the sidebar ends; zero when there is none.
    pub sidebar: u16,
    /// How many cells the panels take on the right, rules and edge included;
    /// zero when they are not there.
    pub right: u16,
    /// How many LINES the bottom strip takes; zero when there is none.
    pub strip: u16,
    /// Whether the preview panel is on.
    pub preview: bool,
    /// Whether the info panel is on.
    pub info: bool,
}

impl Panes {
    /// The listing's first cell — where the sidebar ends plus its rule, or zero.
    #[must_use]
    pub const fn listing_from(self) -> u16 {
        if self.sidebar == 0 {
            0
        } else {
            self.sidebar + RULE
        }
    }

    /// The panels that are on, left to right.
    ///
    /// The preview first, always. It is what answers "is this the file I
    /// meant" in one glance, and the facts are what one then reads.
    #[must_use]
    pub fn on(self) -> Vec<Panel> {
        let mut out = Vec::new();
        if self.preview {
            out.push(Panel::Preview);
        }
        if self.info {
            out.push(Panel::Info);
        }
        out
    }

    /// Where each panel sits: `(which, first cell, cells)`.
    ///
    /// **ONE function for both places.** The right-hand columns and the bottom
    /// strip differ in how wide they are and how many lines they have, and in
    /// nothing else — the panels themselves are the same two panels. Two
    /// builders would be two answers to "what does the info panel say", which
    /// is the divergence the two doors already exist to prevent.
    #[must_use]
    pub fn boxes(self, width: u16) -> Vec<(Panel, u16, u16)> {
        let on = self.on();
        let Ok(n) = u16::try_from(on.len()) else {
            return Vec::new();
        };
        if n == 0 {
            return Vec::new();
        }
        // A RULE BEFORE THE FIRST ONE ONLY ON THE RIGHT. Down in the strip
        // there is nothing to its left to be separated from: what separates the
        // strip from the listing is the rule ABOVE it, which is a line and not a
        // column.
        let (from, room, leading) = if self.right > 0 {
            (width.saturating_sub(self.right), self.right, 1u16)
        } else if self.strip > 0 {
            (0, width, 0u16)
        } else {
            return Vec::new();
        };
        let rules = leading.saturating_add(n - 1);
        let each = room
            .saturating_sub(rules)
            .saturating_sub(EDGE_RIGHT)
            .checked_div(n)
            .unwrap_or(0);
        let mut at = from.saturating_add(leading * RULE);
        let mut out = Vec::with_capacity(on.len());
        let last = on.len().saturating_sub(1);
        for (i, which) in on.into_iter().enumerate() {
            // THE REMAINDER GOES TO THE LAST ONE, and it is not tidiness.
            //
            // An integer division leaves up to `n - 1` cells over, and they
            // were left at the right-hand end where nothing drew them: the
            // owner measured the consequence as *"the big blank space that
            // should not exist between the scrollbar and the window's corner"*,
            // because the facts' bar sits in the LAST cell of its own box and
            // the box stopped short of the edge.
            let mine = if i == last {
                room.saturating_sub(at.saturating_sub(from))
                    .saturating_sub(EDGE_RIGHT)
            } else {
                each
            };
            out.push((which, at, mine));
            at = at.saturating_add(mine).saturating_add(RULE);
        }
        out
    }

    /// The cells the VERTICAL rules sit in, for whoever paints them.
    ///
    /// One before the first panel when they are on the right, and one between
    /// them wherever they are. It is a one-pixel rule and a cell of grid:
    /// separation by whitespace and a rule, never by a container (invariant 15).
    #[must_use]
    pub fn rules(self, width: u16) -> Vec<u16> {
        let boxes = self.boxes(width);
        let mut out = Vec::new();
        for (n, (_, at, _)) in boxes.iter().enumerate() {
            if n == 0 && self.right == 0 {
                continue;
            }
            out.push(at.saturating_sub(RULE));
        }
        out
    }

    /// The cell the LISTING's own scrollbar sits in.
    ///
    /// # Why the bar moved out of the surface's last cell
    ///
    /// Because with the panels on the right that cell is inside the INFO
    /// panel, and a thumb there says "the listing is cut" from a column that is
    /// not the listing — and drags a list it is not sitting on. The owner
    /// photographed the consequence from the other side: two bars in one
    /// column, one under the other, reading as one control.
    ///
    /// So it is the listing's own last cell: just before the panels' first
    /// rule when they are on the right.
    ///
    /// `None` when nothing is beside the listing on the right, and that is not
    /// the same as "the last cell of the region". The region's cells are the
    /// SCREEN's and every row is drawn `chrome::FIELDS_LEAD` cells further in,
    /// so the region's last cell is already past the edge — `files.edgeRight`
    /// is what absorbs it. `None` means the SURFACE's own last cell, which is
    /// exactly where every other view's bar goes and where this one has always
    /// been; the door knows that in pixels and the view does not.
    #[must_use]
    pub const fn listing_bar(self, width: u16) -> Option<u16> {
        if self.right == 0 {
            return None;
        }
        Some(width.saturating_sub(self.right).saturating_sub(RULE))
    }

    /// Which SECTION a cell belongs to.
    ///
    /// # Why this exists at all
    ///
    /// Because until now nothing asked. The manager draws one `Row` per line
    /// carrying every section's fields, and `Row::field_at` walks from the
    /// right — which answers "what did I click" without ever naming a region,
    /// and that was the whole trick. It is not enough for the three things the
    /// owner asked for next, and he named the symptom exactly: *"putting the
    /// cursor over an item of the info column, on the exact row the mouse is
    /// on, ends up giving the hover effect in the listing; that is, right now
    /// it is all one single row"* — and the same missing answer is why a scroll
    /// gesture over the facts scrolled the listing, the sidebar and the facts
    /// together.
    ///
    /// A field can say what it is. A REGION cannot, because the empty cells of
    /// a short section have no field in them at all — and the pointer spends
    /// most of its time over empty cells. So the region is arithmetic, here,
    /// once, and the three callers (the band, the wheel and the focus) ask it
    /// rather than each working it out from `sidebar` and `right`.
    /// `in_strip` is whether the pointer is BELOW the listing, in the band the
    /// panels take when they do not fit beside it. It has to be told, because
    /// down there the panels' boxes span the whole width — the same cell is the
    /// listing's above the rule and a panel's below it, and a cell alone cannot
    /// say which side of a horizontal line it is on.
    #[must_use]
    pub fn section_at(self, cell: u16, width: u16, in_strip: bool) -> Pane {
        if !in_strip && self.sidebar > 0 && cell < self.sidebar {
            return Pane::Sidebar;
        }
        if self.right == 0 && !in_strip {
            return Pane::Listing;
        }
        // THE PANELS FROM THE RIGHT, in the order `boxes` lays them out. A cell
        // on a rule belongs to the section that starts after it, which is the
        // same choice `band_of` makes about the pixel: the rule is drawn on top
        // of whatever the band left there.
        for (which, at, room) in self.boxes(width) {
            if cell >= at && cell < at.saturating_add(room) {
                return match which {
                    Panel::Preview => Pane::Preview,
                    Panel::Info => Pane::Info,
                };
            }
        }
        Pane::Listing
    }

    /// The preview's box, in cells: `(first column, columns)`.
    #[must_use]
    pub fn preview_box(self, width: u16) -> Option<(u16, u16)> {
        self.boxes(width)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Preview))
            .map(|(_, at, w)| (at, w))
    }
}

/// Where the panels' own rows sit inside a region: `(first line, how many).`
///
/// # Why this is not simply "the region"
///
/// Because the panels' fields ride the LISTING's rows, and every door puts
/// lines of its own above those — the footer two, the path and the header; the
/// window three, with the message line between them. The picture was fitted
/// into a box that started at the region's first line and ran its whole
/// height, so it was two lines taller than the panel it was in, centred in the
/// wrong box, and the difference came out as air underneath. Photographed and
/// reported in one sentence: *"the preview's picture is touching the top
/// line and has a lot of extra space underneath"*.
///
/// One function, four readers — each door's picture and each door's facts bar
/// — because that defect was one door's arithmetic written out by hand, and
/// the second door had written its own and got it right. Two answers to one
/// question is how the doors come to disagree.
///
/// `first` is the region's first line, `lines` its height INCLUDING the
/// chrome, `chrome` how many of those lines are the door's own, and `strip`
/// how many lines the bottom strip has — because down there the panels are not
/// in the listing's rows at all, they are underneath them.
#[must_use]
pub const fn panel_band(first: u16, lines: u16, strip: u16) -> (u16, u16) {
    if strip > 0 {
        return (first.saturating_add(lines), strip);
    }
    // THE REGION'S FIRST LINE, and never "minus the listing's chrome".
    //
    // It used to subtract it, and the doors computed the subtraction as
    // `2 - offset` — what was on screen right then. So the picture's box moved
    // two lines UP the moment somebody scrolled the listing, and the owner saw
    // exactly that: *"the same extra-spacing defect at the top of the preview
    // column is still happening, and it goes away only when the listing's
    // scrollbar is scrolled"*. A column whose top is a function of another column's offset
    // is not a column.
    //
    // The chrome does not scroll any more (see `View::region_rows`), and it is
    // the LISTING's: the panels start where every other section starts.
    (first, lines)
}

/// Where a RULE's line goes inside the cell reserved for it, in PIXELS.
///
/// # The same lopsidedness, one register down
///
/// [`RULE`] reserves a whole cell of grid for a line one logical pixel wide,
/// and every site drew that line at the cell's LEFT edge. So the cell became
/// air on the line's right and none at all on its left. Measured off the
/// owner's screen, in the footer door at 1280 px: the sidebar's last column
/// ended 10,8 px before the rule and the listing's icon began 21,6 px after
/// it — one cell against two, for one boundary.
///
/// The line goes in the MIDDLE of its own cell, so both neighbours get half of
/// it. Nothing else moves: every hit test, every column and every elision
/// counts the same cells they counted before, because the cell was the rule's
/// already and no field is laid out inside it.
///
/// `at` is the rule's cell, `lead` what the door owes before a row's first
/// cell, and `cell` how wide a cell is in that door.
#[must_use]
pub fn rule_x(at: u16, lead: u16, cell: f32) -> f32 {
    (f32::from(lead.saturating_add(at)) + 0.5) * cell
}

/// Where the pointer's band begins and how wide it is, in PIXELS.
///
/// # Why one function and not one per door
///
/// Because the two doors had written it out separately and had arrived at
/// three different answers between them, and every one of the three was the
/// same defect: the band began one CELL to the right of the rule it should
/// have started at, so the row lit up with a strip of unlit background between
/// it and the line beside it. The owner reported it in those words — *"the
/// item's selection background does not touch the vertical separating line
/// on the listing's left-hand side"* — about the footer, and the window door
/// had it in both of its arms: the sidebar's band started at the margin
/// instead of at the screen's edge, and the listing's ran to the window's
/// right edge, straight under the preview.
///
/// # Why PIXELS, when everything else here is cells
///
/// Because the band's two edges are RULES, and a rule sits in the middle of
/// its own cell (see [`rule_x`]). Half a cell is not a thing the integer grid
/// can say, and a band that stopped at the cell's edge would be back to a gap
/// on one side. The `cell` and the `edge` come from the door, so this is still
/// one answer for both of them and not a pixel decided here.
///
/// The band stops exactly ON the rule, and that is deliberate: the rules are
/// painted after it, so the line covers the last pixel of the band rather than
/// leaving one of its own.
///
/// `at` is the cell the pointer is on, in the ROW's own cells — `None` when it
/// is over the row but not over a field, which lights the listing, because the
/// listing is what a row mostly is.
#[must_use]
pub fn band_of(
    panes: Panes,
    lead: u16,
    width: u16,
    at: Option<u16>,
    cell: f32,
    edge: f32,
) -> (f32, f32) {
    let split = panes.sidebar;
    // WHERE THE LISTING ENDS ON THE RIGHT: the panels' first rule, or the
    // surface's own edge. A band running under the preview would light the
    // picture, and the pointer is not on the picture — it is on a row beside it.
    let right = if panes.right > 0 {
        rule_x(width.saturating_sub(panes.right), lead, cell)
    } else {
        edge
    };
    // A PANEL LIGHTS ITSELF, and never the listing beside it.
    //
    // The owner photographed this one: *"putting the cursor over an item
    // of the info column, on the exact row the mouse is on, ends up giving
    // the hover effect in the listing"*. The band was bounded on the right by the panels'
    // first rule and then drawn for whatever cell the pointer was in, so a
    // pointer inside a panel lit a row of a list it was not over.
    if let Some(c) = at
        && panes.right > 0
    {
        let which = panes.section_at(c, width, false);
        if matches!(which, Pane::Preview | Pane::Info) {
            let panel = match which {
                Pane::Preview => Panel::Preview,
                _ => Panel::Info,
            };
            return panel_px(panes, panel, lead, width, cell, edge).unwrap_or((0.0, 0.0));
        }
    }
    if split == 0 {
        return (0.0, right.max(0.0));
    }
    let line = rule_x(split, lead, cell);
    match at {
        // THE SIDEBAR'S BAND RUNS FROM THE SCREEN'S OWN EDGE. It is the one of
        // the four the owner called right, and it is right for a reason worth
        // keeping: the lead is not the sidebar's air, it is the door's, and a
        // band that began after it would leave a lit row with a dark margin.
        Some(c) if c < split => (0.0, line),
        _ => (line, (right - line).max(0.0)),
    }
}

/// One offset, moved by `by` and kept inside what there is to show.
///
/// Integer arithmetic and saturating at both ends, like every other offset in
/// this project: a list shorter than its room does not scroll at all, and the
/// last page is the floor rather than the end.
#[must_use]
pub fn slide(from: usize, by: i64, total: usize, room: usize) -> usize {
    let last = total.saturating_sub(room);
    let moved = i64::try_from(from)
        .unwrap_or(i64::MAX)
        .saturating_add(by)
        .max(0);
    usize::try_from(moved).unwrap_or(0).min(last)
}

/// Where a panel's REGION begins and ends, in pixels.
///
/// # Why this is not `at * cell` and `room * cell`
///
/// Because those are the TEXT's rails, and a picture is not text. It is the
/// two-rails rule this design system already keeps for the rule and the
/// selection band, applied to the third thing that fills a region: what fills
/// one runs from the rule on its left to the rule on its right, and where there
/// is no rule it runs to the surface's own edge.
///
/// The difference is not academic — it is half a cell at each end, and the
/// owner measured it as a picture that was not centred. In the window's strip
/// the preview's box began at the door's text margin (one cell in) while its
/// region began at the window's edge, so the air came out 60 px on the left
/// against 54 on the right for a box the code believed was centred.
#[must_use]
pub fn panel_px(
    panes: Panes,
    which: Panel,
    lead: u16,
    width: u16,
    cell: f32,
    edge: f32,
) -> Option<(f32, f32)> {
    let boxes = panes.boxes(width);
    let i = boxes.iter().position(|(p, _, _)| *p == which)?;
    let (_, at, _) = boxes[i];
    // A RULE ON THE LEFT, or the surface. Down in the strip the first panel has
    // nothing to its left — what separates it from the listing is the rule
    // ABOVE it — and `Panes::boxes` says so by giving it no leading rule.
    let left = if at >= RULE && (panes.right > 0 || i > 0) {
        rule_x(at.saturating_sub(RULE), lead, cell) + 1.0
    } else {
        0.0
    };
    let right = boxes.get(i + 1).map_or(edge, |(_, next, _)| {
        rule_x(next.saturating_sub(RULE), lead, cell)
    });
    Some((left, (right - left).max(0.0)))
}

/// ANY SECTION'S BAND, in pixels: where it begins and how wide it is.
///
/// # Why the pixels, and why one function
///
/// Because a column's edges are RULES and the surface's border, and neither is
/// on a cell boundary: `columns` is `floor(width / cell)`, so the last cell ends
/// before the border and a rule sits in the middle of its own cell. Anything
/// placed by cell index inherits that difference — and the owner measured every
/// one of its consequences: a scrollbar drawn a cell past the border and gone
/// off screen, another that answered for a cell the pointer never reached, and
/// air to the right of one column that changed with the window's width.
///
/// So the band is arithmetic in pixels, here, once. The hover's band asks it,
/// the scrollbar asks it, and the picture's box asks it — three readers, one
/// answer, which is the only arrangement in which they cannot disagree.
#[must_use]
pub fn section_px(
    panes: Panes,
    pane: Pane,
    lead: u16,
    width: u16,
    cell: f32,
    edge: f32,
) -> Option<(f32, f32)> {
    match pane {
        Pane::Sidebar => (panes.sidebar > 0).then(|| (0.0, rule_x(panes.sidebar, lead, cell))),
        Pane::Listing => {
            let from = if panes.sidebar == 0 {
                0.0
            } else {
                rule_x(panes.sidebar, lead, cell) + 1.0
            };
            // WHERE THE LISTING ENDS: the panels' first rule when they sit
            // beside it, and the surface's own border when they do not.
            let to = if panes.right > 0 {
                rule_x(width.saturating_sub(panes.right), lead, cell)
            } else {
                edge
            };
            Some((from, (to - from).max(0.0)))
        }
        Pane::Preview => panel_px(panes, Panel::Preview, lead, width, cell, edge),
        Pane::Info => panel_px(panes, Panel::Info, lead, width, cell, edge),
    }
}

/// The preview's picture, centred in its box with air on all four sides.
///
/// # What `fit_into` alone does not do
///
/// It centres, and it is not enough. The scale is the tighter of the two
/// ratios, so whichever dimension is the tight one comes out exactly as long
/// as the box — the picture is centred and touching two borders at once. Both
/// doors shipped that way and the owner photographed both: the top edge in the
/// window, the right-hand rule in the footer. What he asked for is a rule and
/// not a nudge — *"the blank space on the left has to be always equal to the
/// one on the right, and the blank space below the picture has to be always
/// equal to the space above"* — and centring already promises the halves are equal. The
/// missing half of the promise is that neither of them is zero.
///
/// So the box gives up `space.files.pictureAir` on each of its four sides
/// before anything is fitted into it. One function, two doors, because the
/// arithmetic that was written out twice is the arithmetic that came out
/// different twice.
#[must_use]
pub fn picture_in(
    space: ronoctua_render::image::Area,
    w: u32,
    h: u32,
) -> ronoctua_render::image::Area {
    let air = f32::from(ronoctua_tokens::space::FILES_PICTUREAIR);
    ronoctua_render::image::Area {
        x: space.x + air,
        y: space.y + air,
        // NEVER NEGATIVE. A panel at its floor in a two-line strip is smaller
        // than the air twice over, and a negative width would come out of
        // `fit_into` as a scale nobody can see.
        width: (space.width - air - air).max(0.0),
        height: (space.height - air - air).max(0.0),
    }
    .fit_into(w, h)
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
/// There is no `layout_beside(width, 0, 0)` beside it any more. It was this function
/// with `from` at zero, and once the header and the rows both had to ask about
/// the sidebar it had no caller left outside the tests — a second name for one
/// arithmetic is a second place to read.
///
/// `to` is what the PANELS take on the right, and zero when they are not there.
/// It is the same argument the first line already carries, applied to the other
/// edge: resolving the columns against the whole width and then reserving the
/// right-hand cells would let the last column land underneath a panel, and
/// "nothing leaks" is the one rule that never gives way. Every caller passes
/// the same `to`, which is what [`Panes`] exists to guarantee.
///
/// `details` is whether the three columns one LOOKS UP — the permission
/// triplet and the two names — are asked for at all. They are off until
/// somebody asks, and the argument is [`GIVE_WAY`]'s own, applied one step
/// earlier: a listing is navigated by NAME, and those three are read
/// deliberately, one file at a time. What the owner asked for is that the
/// deliberate case be the one that costs a key.
///
/// It is a preference removed BEFORE the loop and never a column filtered out
/// of a finished layout: removed here the cells go to the name, filtered
/// afterwards they would be a hole where the column used to be.
#[must_use]
pub fn layout_beside(width: u16, from: u16, to: u16, details: bool) -> Layout {
    let width = width.saturating_sub(from).saturating_sub(to);
    // THE ARITHMETIC IS `ronoctua_design::Table`'S, and this function is now
    // the LISTING's statement about itself.
    //
    // The two phases were written out here — a give-way loop, then a floor
    // that is a preference and a rule that is not — and written out again for
    // the sidebar, and a third time for the panels. That crate's own head says
    // it is this function generalised, and it has been since it was written;
    // what was missing was this call.
    //
    // What stays here is what is ABOUT files: which columns exist, how wide a
    // date is, and the order in which they give way. A date column is eleven
    // cells because a date is eleven cells, and that is not the design
    // system's business.
    let mut table = ronoctua_design::Table::across(width).inset(GUTTER, EDGE_RIGHT);
    for c in [
        Column::Icon,
        Column::Name,
        Column::Perms,
        Column::Owner,
        Column::Group,
        Column::Size,
        Column::Modified,
    ] {
        // THE THREE ONE LOOKS UP ARE REMOVED BEFORE THE LOOP, and never
        // filtered out of a finished layout: removed here their cells go to
        // the name, filtered afterwards they would be a hole where the column
        // used to be.
        if !details && matches!(c, Column::Perms | Column::Owner | Column::Group) {
            continue;
        }
        // THE RANK IS `GIVE_WAY` READ BACKWARDS. The first to go is the one
        // that gives way soonest, which is the highest rank over there; the
        // name is in neither list, and a column that never gives way is a
        // column with nothing to say about the order.
        #[allow(clippy::cast_possible_truncation)]
        let rank = GIVE_WAY
            .iter()
            .position(|g| *g == c)
            .map_or(0, |n| (GIVE_WAY.len() - n) as u8);
        table = table.with(ronoctua_design::Column {
            id: c.id(),
            width: if matches!(c, Column::Name) {
                ronoctua_design::table::Width::Fill { floor: NAME_FLOOR }
            } else {
                ronoctua_design::table::Width::Fixed(c.width())
            },
            rank,
        });
    }
    let columns = table
        .resolve()
        .fields()
        .iter()
        .filter_map(|f| Column::of_id(f.id).map(|c| (c, from + f.at, f.width)))
        .collect();
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

/// Which SECTION of the manager has the keyboard.
///
/// The same word the notification manager uses for the same idea, and
/// deliberately: two names for "which side am I on" is two things to learn.
///
/// # There are four now, and the order is the screen's
///
/// It was two, and the two were the two LISTS — the only regions that had a
/// cursor to move. Then the panels arrived on the right, the facts stopped
/// fitting in the lines they were given, and the owner asked for the obvious
/// consequence: *"each section being independent and having its own scrollbar
/// (sidebar/listing/preview/info). And it has to be possible to move the focus
/// between them with Tab and Shift+Tab."*
///
/// The variants are declared LEFT TO RIGHT as they are drawn, because that is
/// what [`View::cycle_pane`] walks. Down in the bottom strip the two panels are
/// underneath rather than beside, and the order still reads correctly: they
/// come after the listing either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Pane {
    /// The sidebar.
    Sidebar,
    /// The directory. Where the cursor starts, and where typing always goes.
    #[default]
    Listing,
    /// The picture.
    ///
    /// **It has nothing to scroll, and it still takes focus.** A picture is one
    /// thing and it is either in the panel or it is not, so this pane will
    /// almost never show a bar — the arrows in it move the LISTING's cursor,
    /// which is what changes the picture, and that is the honest answer to
    /// "what does moving inside a preview mean". What it must not do is be
    /// SKIPPED: a `Tab` that visits three of four sections is a key one has to
    /// count presses for.
    Preview,
    /// The facts.
    Info,
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

/// ONE COLUMN OF THE MANAGER, and everything that is true about it.
///
/// # Why this type exists
///
/// Because the manager draws four lists and had no name for any of them. Each
/// painter, each hit test and each gesture worked out where a column began and
/// what it held, and they disagreed — every one of the owner's reports was two
/// of those answers differing:
///
/// - the sidebar's first two rows were unreachable, because the LISTING's
///   chrome was refused for the whole line;
/// - clicking a folder went to the parent, because the listing's cell was
///   resolved against the path's crumbs;
/// - the preview's top moved when the listing scrolled, because the panels'
///   band was computed as "the region, minus the listing's chrome, minus how
///   much of it is still on screen";
/// - and a gesture over the facts scrolled everything, because there was one
///   offset.
///
/// A section says where it is (`from`..`to` in cells, `top` for `lines`), what
/// it holds (`rows`, already at its own `offset`), and how much of it there is
/// (`total`). Everything else asks. Nothing else decides.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// Which column this is.
    pub pane: Pane,
    /// Its first cell.
    pub from: u16,
    /// One past its last cell.
    pub to: u16,
    /// Its first line of the region.
    pub top: usize,
    /// How many lines it occupies.
    pub lines: usize,
    /// Its own first visible row.
    pub offset: usize,
    /// How many rows it has in all.
    pub total: usize,
    /// How many of `content`'s first lines are CHROME — the listing's path and
    /// header, which do not scroll. Zero for every other section.
    pub above: usize,
    /// Its own lines, from `offset`, at most `lines` of them. The listing's
    /// chrome is the first of these and does not scroll.
    ///
    /// A section's lines are ITS OWN. Nothing composes a line out of several
    /// sections' fields: that arrangement is what made a column's length, its
    /// offset and its top reach into its neighbours, and every defect the
    /// owner reported was one of those reaches.
    pub content: Vec<Row>,
}

impl Section {
    /// WHICH REGION LINE carries `row` of this section, if it is on screen.
    ///
    /// Each section counts from its own `top` and its own `offset`, and the
    /// listing's chrome sits in front of its rows. Doing this arithmetic
    /// anywhere else is how a pointer resting on the third fact lit the third
    /// row of the listing: the row number was right and the line it was turned
    /// into belonged to another column.
    #[must_use]
    pub fn line_of(&self, row: usize) -> Option<usize> {
        let i = row.checked_sub(self.offset)?;
        let at = self.above.saturating_add(i);
        (at < self.content.len()).then_some(self.top + at)
    }

    /// THE SCROLLBAR'S RECTANGLE, in pixels — or none when nothing is cut.
    ///
    /// One rectangle, drawn by the painter and tested by the pointer. They used
    /// to be two answers: a cell index for the drawing and another arithmetic
    /// for the hit test, and the owner found every place they disagreed — the
    /// facts' bar that could not be grabbed in one door, the listing's that
    /// could not be grabbed in the other, and one drawn a cell past the border.
    ///
    /// `right` is the section's own right edge and `top` the region's, both in
    /// pixels; `Section::line_of` and [`section_px`] are where those come from.
    /// The thumb is at least one line: with twenty facts in four lines the
    /// proportional answer is nought.
    #[must_use]
    pub fn bar_box(
        &self,
        right: f32,
        top: f32,
        bottom: f32,
        cell: f32,
        line_px: f32,
    ) -> Option<(f32, f32, f32, f32)> {
        // THE TRACK IS PIXELS, AND IT ENDS WHERE THE BAND DOES.
        //
        // It used to be `lines × line_px`, and a surface is not a whole number
        // of lines: `lines` is `floor(height / line_px)`, so the track stopped
        // short by the residue — up to one line of it — and at the last offset
        // the thumb stopped there too. The owner photographed the gap under
        // the facts' bar twice and said, correctly, that it was still there.
        //
        // AND THE BAND IS THE SECTION'S OWN, never the whole surface's.
        //
        // Reaching for `bottom` unconditionally was the first shape of that
        // fix, and it was too wide: down in the STRIP the panels have lines of
        // their own BELOW the listing, so the listing's band ends where the
        // strip begins. Handed the surface, the listing's thumb walked out of
        // its column and into the facts' box, which the owner watched it do.
        //
        // The residue is at most one line by construction, so a band ending
        // within one line of the surface is the LAST band in its column and
        // takes what is left over; one ending a whole line or more above it
        // has something underneath.
        let band_top = top + f32::from(u16::try_from(self.top).unwrap_or(0)) * line_px;
        self.scroll().bar(right, band_top, bottom, cell, line_px)
    }

    /// This section as a list that may not fit.
    #[must_use]
    pub const fn scroll(&self) -> ronoctua_design::Scroll {
        ronoctua_design::Scroll {
            total: self.total,
            lines: self.lines,
            offset: self.offset,
            top: self.top,
        }
    }

    /// Whether a point of the region is inside this section.
    #[must_use]
    pub const fn holds(&self, line: usize, cell: u16) -> bool {
        cell >= self.from && cell < self.to && line >= self.top && line < self.top + self.lines
    }
}

/// How many lines each kind of section is given.
///
/// Two numbers and not one, because the panels and the listing are not the same
/// height whenever the panels are down in the strip — and a bar asked about the
/// wrong one answers "nothing is cut" for a list that is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    /// The listing's, which the sidebar shares.
    pub list: usize,
    /// The panels', which is the strip's height when they are down there.
    pub panel: usize,
}

/// What a point resolves to: the section, the row inside THAT section, and the
/// field's target.
///
/// See [`View::hit`] for why the section has to come first.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    /// Which section owns the cell.
    pub pane: Pane,
    /// The row inside that section. `usize::MAX` is the listing's chrome, which
    /// both doors already spell that way.
    pub row: usize,
    /// What the field under the point does.
    pub hit: Hit,
    /// What it says about itself, for the line above.
    pub explain: Option<Explanation>,
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

/// The listing's own chrome: the path, then the header.
///
/// Two lines, and they belong to the LISTING's column — not to the region. Every
/// other section starts at the region's first line.
pub const CHROME: usize = 2;

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

/// The first cell that puts `what` cells in the middle of `room` cells at `at`.
///
/// Integer arithmetic, and it rounds DOWN: half a cell does not exist on this
/// grid, and a glyph one cell left of centre reads as centred while a glyph
/// half a cell anywhere reads as blurred.
const fn centred(at: u16, room: u16, what: u16) -> u16 {
    at.saturating_add(room.saturating_sub(what) / 2)
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
    /// What is INSIDE the file the cursor was on — the picture and the facts.
    ///
    /// `Box`ed for the same reason `Racked` is: it carries up to a mebibyte of
    /// RGBA, and a `Report` is moved by value through the channel.
    Inspected(Box<Looked>),
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
    /// *"the answers are `files-no` and `files-yes`"*.
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
    /// The sidebar's own first visible row.
    ///
    /// # Why the sidebar needed one of its own
    ///
    /// Because it did not have one, and the owner found the hole from the
    /// outside: *"using the touchpad's scroll gestures over the info region,
    /// EVERYTHING ends up scrolling (sidebar/listing/…)"*. The sidebar's
    /// rows are `Field`s on the listing's `Row`, which is what made the whole
    /// arrangement cheap — and it also meant line `n` was sidebar row `n`, so
    /// the two could not be anywhere but level with each other.
    ///
    /// One number breaks that, and only that: the rows are still the same
    /// `Row`, still one hit test, still one painter. What changed is which
    /// sidebar row line `n` carries.
    side_offset: usize,
    /// The listing's own first visible row.
    ///
    /// # Why it moved here, and what it cost while it was elsewhere
    ///
    /// It used to be a `Cell<usize>` the DOOR wrote before every question:
    /// each door kept the number and pushed it in through `set_first`. Three
    /// of the four sections already had their offset here — the sidebar, the
    /// facts, and the preview which does not scroll — so the listing being
    /// somewhere else was the one asymmetry in the four, and it produced two
    /// of the defects the owner reported by hand.
    ///
    /// `wheel` had no arm for the listing: it answered `false`, meaning "not
    /// mine, ask the door". Each door then had to implement the case again,
    /// and each got a different half right — the drag worked in the window's
    /// facts and not its listing, and in the footer's listing and not its
    /// facts. With the number here there is no arm to forget: the same
    /// `slide` moves all three.
    ///
    /// The second half was the ORDER. A wheel event moved the door's copy, and
    /// the next frame pushed that copy in before drawing — so a gesture with
    /// no redraw between the two showed up one frame late, which is what a
    /// touchpad sends dozens of times a second.
    list_offset: usize,
    /// When the sidebar's file was last read, for "has somebody changed it".
    side_stamp: Option<std::time::SystemTime>,
    /// Whether the PREVIEW panel is asked for.
    ///
    /// **ON at the start**, and the key is how one turns it OFF. It used to be
    /// the other way round, and the argument was the cost — a preview is a
    /// child process per file walked past. The owner overruled it from the
    /// screen: the panels are what the manager is FOR, and a feature that has
    /// to be switched on every time is a feature most of the time nobody has.
    /// The cost is still real and it is still bounded, by the one thread
    /// `inspector` runs and by its "only the newest" rule.
    preview_on: bool,
    /// Whether the INFO panel is asked for. On at the start, same argument.
    info_on: bool,
    /// The first fact the info panel shows.
    ///
    /// The panel is as tall as the region and a file can have twenty facts, so
    /// the list does not always fit — and a list that is cut with nothing
    /// saying so is the failure this whole file keeps naming. What says so is
    /// [`View::facts_bar`], and this is what it points at.
    ///
    /// It goes back to zero whenever the CURSOR moves: the facts are about the
    /// focused entry, and keeping a scroll position across two different files
    /// would open the next one half way down a list it does not have.
    info_offset: usize,
    /// Whether the listing shows the permission triplet, the owner and the
    /// group.
    ///
    /// Off at the start. See [`layout_beside`] for the argument; the key that
    /// turns it on is `files-columns`, and the hint line names it.
    details_on: bool,
    /// What the tools said about the file the cursor was on.
    ///
    /// ONE, and not a map. The panels are about the focused entry and there is
    /// one of those; a cache of every file walked past would be the footer's
    /// memory, which is the argument `ronoctua_render::image` already makes
    /// about its thirty-nine textures.
    looked: Option<Looked>,
    /// The stamp of what was last ASKED for.
    ///
    /// It is what stops the host asking again, and it has to be separate from
    /// [`View::looked`] for the reason `surface`'s `icons` map already
    /// records: a refusal is an ANSWER, and without somewhere to write "I have
    /// asked and the answer was no" the same file is asked about on every
    /// frame.
    asked: Option<String>,
    /// How many lines the panels were last DRAWN in.
    ///
    /// The same arrangement as [`View::drawn_at`] and for the same reason: a
    /// key arrives without a height, and how far the facts can scroll depends
    /// on one. The view cannot work it out — the footer's region has a ceiling
    /// and a window has the height somebody dragged it to — so the door that
    /// draws the bar is the one that knows, and [`View::facts_bar`] is where
    /// every frame of both of them goes through.
    drawn_lines: std::cell::Cell<u16>,
    /// The width the view was last laid out at.
    ///
    /// # Why the view remembers a width
    ///
    /// Because a KEY arrives without one and the answer to "can this panel
    /// appear" depends on one. The alternative was for each door to work it out
    /// after the key — which is two doors answering the same question, the
    /// exact divergence this module exists to prevent. The last width the view
    /// was DRAWN at is the only one that is true of what is on screen, and both
    /// doors draw before anybody types.
    drawn_at: std::cell::Cell<u16>,
}

/// What the inspector was asked about one file.
///
/// The path travels back with the answer because the cursor moves while a
/// child process runs: without it the panel would show the previous file's
/// picture under the current file's name, which is the one failure a preview
/// must not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    /// The file, absolute.
    pub path: PathBuf,
    /// The type to act on — already through `ronoctua_inspect::settled`.
    pub mime: &'static str,
    /// How many pixels across the longer side, before quantisation.
    pub target: u32,
    /// The cache key this answer will carry back.
    pub key: String,
}

/// What the inspector came back with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Looked {
    /// `Stamp::key()` — what identifies this answer in the texture map.
    pub key: String,
    /// The file it is about.
    pub path: PathBuf,
    /// The pixels, when there are any.
    pub rgba: Option<ronoctua_render::image::Rgba>,
    /// Why there are none, when there are none.
    pub why: Option<ronoctua_inspect::Refusal>,
    /// What the tools said the file is.
    pub facts: Vec<ronoctua_inspect::facts::Fact>,
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

/// A fact's value, in words.
///
/// TWO SHAPES, and the split is not tidiness. A camera's model is `Pixel 8 Pro`
/// in every language and translating it would be wrong — the same rule a
/// `Choice`'s label already follows, because a filename is text and never a
/// key. A frame rate is not: where the unit goes and what two channels are
/// called are the LANGUAGE's business, and that belongs in the `.ftl`
/// (invariant 8).
///
/// # Why an argument called `n` crosses as a NUMBER
///
/// Because `files-value-channels` and `files-value-pages` are Fluent
/// SELECTORS, and `facts::Value::Phrase` carries its arguments as `String`.
/// Given the string `"1"`, Fluent looks for a variant literally named `1` and
/// falls through to `*[other]`: the screen says **"1 páginas"** with every
/// check green. It is verbatim the defect `ronoctua_i18n::Arg::Number`'s own
/// docstring exists for.
///
/// The rule is the argument's NAME and not a list of the two keys, so a third
/// selector arriving later is right by construction — and it is `n` and not
/// "anything that parses as an integer" on purpose: Fluent groups a number by
/// locale, so `iso` would become `1,600` and a photograph's dimensions
/// `3,024 × 4,032`.
fn fact_said(value: &ronoctua_inspect::facts::Value, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::Arg;
    use ronoctua_inspect::facts::Value;
    match value {
        Value::Data(text) => text.clone(),
        Value::Phrase { key, args } => {
            let crossing: Vec<(&str, Arg<'_>)> = args
                .iter()
                .map(|(k, v)| {
                    if *k == "n"
                        && let Ok(n) = v.parse::<i64>()
                    {
                        (*k, Arg::Number(n))
                    } else {
                        (*k, Arg::Text(v.as_str()))
                    }
                })
                .collect();
            texts.t_args(key, &crossing)
        }
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
            side_offset: 0,
            list_offset: 0,
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
            // THE PANELS COME UP ON. The owner's decision, reported from the
            // screen: "prévia e info devem estar sempre habilitadas".
            preview_on: true,
            info_on: true,
            info_offset: 0,
            // AND THE THREE COLUMNS ONE LOOKS UP COME UP OFF, which is the
            // same decision seen from the other side: what the manager shows
            // without being asked is what one navigates by.
            details_on: false,
            looked: None,
            asked: None,
            drawn_at: std::cell::Cell::new(0),
            drawn_lines: std::cell::Cell::new(0),
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
        // THE SAME `panes` THE ROWS ARE BUILT WITH, and it has to be, or the
        // header names a different table. It is also why nothing of the header
        // reaches past `panes.right`: a column name drawn over a panel would
        // label a column that is somewhere else, and clicking it would sort by
        // whichever one happened to be under the word.
        let panes = self.panes(width);
        let layout = layout_beside(width, panes.listing_from(), panes.right, self.details_on);
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
    /// ONLY THE TESTS ASK — see [`View::drawn_at`], which says why.
    #[cfg(test)]
    fn drawn(&mut self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        self.drawn_at(&[], 0, usize::MAX, width, texts)
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
    /// ONLY THE TESTS ASK FOR THIS, and it is not a model of anything.
    ///
    /// # Why it is not how the manager works
    ///
    /// Because the manager has four columns, and for a long time it drew them
    /// as ONE line carrying all four's fields. That arrangement made a
    /// column's length, its offset and its top reach into its neighbours, and
    /// every defect the owner reported came out of a reach: clicking a folder
    /// resolving to the path above it, the sidebar's first rows unreachable,
    /// the facts' hover lighting the listing, one wheel gesture moving
    /// everything, the preview's top jumping when the listing scrolled.
    ///
    /// Both doors draw [`View::sections`] now — each section on its own rail,
    /// at its own `top`, from its own content. Nothing in the shell composes a
    /// line out of several sections.
    ///
    /// What remains here is an ASSERTION CONVENIENCE: a test that wants to say
    /// "at this line, this is what would be on screen" reads the sections and
    /// concatenates them. It is `cfg(test)` so it cannot become a habit.
    #[cfg(test)]
    fn drawn_at(
        &mut self,
        chrome: &[Row],
        skip: usize,
        take: usize,
        width: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        if !self.plain() {
            return self.modal_rows(skip, take, width, texts);
        }
        self.list_offset = skip;
        let sections = self.sections(chrome, take, width, texts);
        let region: Vec<&Section> = sections.iter().filter(|s| s.top == 0).collect();
        let lines = region.iter().map(|s| s.content.len()).max().unwrap_or(0);
        (0..take.min(lines))
            .map(|k| {
                let mut fields = Vec::new();
                for sec in &region {
                    if let Some(line) = sec.content.get(k) {
                        fields.extend(line.fields.iter().cloned());
                    }
                }
                let of_listing = region
                    .iter()
                    .find(|s| matches!(s.pane, Pane::Listing))
                    .and_then(|s| s.content.get(k));
                Row {
                    fields,
                    marked: of_listing.is_some_and(|r| r.marked),
                    focused: of_listing.is_some_and(|r| r.focused),
                }
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
            let panes = self.panes(width);
            let side = if panes.sidebar == 0 {
                0
            } else {
                self.side.shown(self.hidden).len()
            };
            // AND THE PANELS' OWN, when they are on the RIGHT. Down in the
            // strip they take LINES OF THEIR OWN and none of the listing's, so
            // they count for nothing here — see `Panes::strip`, and the split
            // between "height" and "scroll" the footer keeps.
            let panel = if panes.right == 0 {
                0
            } else {
                self.panel_lines()
            };
            self.shown.len().max(side).max(panel).max(1)
        })
    }

    /// How many lines the REGION needs, with the listing's chrome on top of the
    /// listing and nothing else.
    ///
    /// # Why this is not `row_count + chrome`
    ///
    /// Because the chrome is one column's, and adding it to the tallest of the
    /// three added it to whichever column happened to win. With a short listing
    /// beside a ten-row sheet that is two blank lines at the bottom of the
    /// region — the same defect as the blank at the top, upside down.
    #[must_use]
    pub fn region_len(&self, chrome: usize, width: u16) -> Option<usize> {
        let panes = self.panes(width);
        self.row_count(width).map(|_| {
            let side = if panes.sidebar == 0 {
                0
            } else {
                self.side.shown(self.hidden).len()
            };
            let panel = if panes.right == 0 {
                0
            } else {
                self.panel_lines()
            };
            self.shown
                .len()
                .max(1)
                .saturating_add(chrome)
                .max(side)
                .max(panel)
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
        self.panes(width).sidebar
    }

    /// **THE ONE PLACE THE WHOLE DIVISION IS DECIDED.**
    ///
    /// [`View::split`] used to be it, and its own note says why it had to be:
    /// both painters and both hit-testers asked `sidebar_columns` for
    /// themselves, and on a second monitor the rule and the fields landed at
    /// different cells. There are FOUR regions now — the sidebar, the listing
    /// and two panels — so `split` became a thin call into this rather than
    /// gaining siblings beside it, which would have been three more chances to
    /// disagree.
    ///
    /// Everything is zero while a menu, a question or a sheet is up: each of
    /// them is modal, and covering a question half-way is worse than covering
    /// it.
    ///
    /// **The order is the sidebar first.** It is decided against the whole
    /// width, exactly as before, and the panels get what is left — so turning a
    /// preview on never moves the places one navigates by, and no existing
    /// answer changes when both panels are off.
    ///
    /// **And the panels flip TOGETHER**: right-hand columns or bottom strip,
    /// never one of each. The owner's sentence is binary, and a mixed mode
    /// would make the listing shorter *and* narrower at once.
    #[must_use]
    pub fn panes(&self, width: u16) -> Panes {
        // THE WIDTH IS REMEMBERED HERE, because this is the one function every
        // painter and every hit test goes through. See `View::drawn_at`.
        self.drawn_at.set(width);
        if !self.plain() {
            return Panes::default();
        }
        let sidebar = sidebar_columns(width, self.sidebar_showing(), self.side_content());
        let beside = width.saturating_sub(if sidebar == 0 { 0 } else { sidebar + RULE });
        let wanted = u16::from(self.preview_on) + u16::from(self.info_on);
        let right = right_columns(beside, wanted > 0, wanted);
        Panes {
            sidebar,
            right,
            strip: if wanted > 0 && right == 0 && strip_fits(width, wanted) {
                ronoctua_tokens::space::FILES_STRIPLINES
            } else {
                0
            },
            preview: self.preview_on,
            info: self.info_on,
        }
    }

    /// Whether the preview panel is asked for.
    ///
    /// `#[cfg(test)]` because nothing on screen reads it: what the doors need
    /// is [`View::panes`], which answers where the panels ARE and not only
    /// whether they were asked for. It is here because the toggle has to be
    /// provable — the same reason `View::pane` is.
    #[cfg(test)]
    #[must_use]
    pub const fn preview_showing(&self) -> bool {
        self.preview_on
    }

    /// Whether the info panel is asked for.
    #[cfg(test)]
    #[must_use]
    pub const fn info_showing(&self) -> bool {
        self.info_on
    }

    /// How many LINES the panels want, when they sit on the right.
    ///
    /// The taller of the two, and it is what makes `row_count` and `rows_from`
    /// agree: a region that reserved lines nothing draws into is blank space
    /// below the listing, which reads as the manager having lost the rest of
    /// it. The preview asks for [`preview_lines`] whatever is in it — a picture
    /// one line tall is not a picture — and the info panel asks for as many
    /// lines as it has facts.
    ///
    /// **Asking is not getting.** The footer's region has a ceiling of twelve
    /// lines (invariant 14) and a photograph has more facts than that, so what
    /// the panel is GIVEN is the door's business and can be less than this.
    /// What says so on screen is [`View::facts_bar`]; before it there was
    /// nothing, and the facts past the ceiling were simply not there.
    fn panel_lines(&self) -> usize {
        // NOTHING UNDER THE CURSOR, NOTHING TO ASK FOR. A query that matches no
        // file leaves both panels with no subject, and a region that stayed
        // twelve lines tall to hold two empty boxes is a region that stopped
        // answering the person typing. `leader-vm` is what caught it: typing
        // `zzz` has to narrow the list, and with the panels on by default it
        // did not — 288 px before and 288 px after.
        //
        // It is the panels' own rule, not an exception to it: they are always
        // ON, and what they ask for is what they have to say.
        if self.focused().is_none() {
            return 0;
        }
        // AND THE PANELS ASK FOR WHAT THEY HAVE TO SAY, which is a picture's
        // box or a sheet's rows.
        //
        // This is what makes the region grow to its ceiling whenever the
        // manager is open, and that is deliberate rather than a side effect:
        // the panels are on from the start now, and a panel given four lines
        // out of a sheet of ten is a panel one has to scroll to read at all.
        // The ceiling still holds (invariant 14), and `View::facts_bar` is
        // what says the sheet was cut when even the ceiling is not enough —
        // which for a photograph it never is.
        //
        // The cost is that the region's HEIGHT stops being an answer to what
        // was typed; `leader-vm` used to read it that way and now reads the
        // text instead, which is what it was actually asking about.
        let mut lines = 0usize;
        if self.preview_on {
            lines = lines.max(usize::from(preview_lines()));
        }
        if self.info_on {
            lines = lines.max(self.sheet().len());
        }
        lines
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

    /// Where the path row begins and how many cells it has.
    ///
    /// # The defect this exists for
    ///
    /// The path was built without a width and drawn from cell ZERO, so on a
    /// screen with a sidebar it ran straight across the rule that separates
    /// the two lists — photographed in `10-manager-with-preview.png`, where
    /// `/root/shots/` reaches `x≈170` and the rule is at `x=151`. It was
    /// the one thing on that row that did not respect the division: the
    /// sidebar's fields stop at the split and the listing's start after it.
    ///
    /// # Why it MOVES rather than being cut short
    ///
    /// Because the path is the LISTING's — it says which directory these rows
    /// are — and the sidebar's column is somebody else's. Cutting it at the
    /// split would have squeezed it into the dozen cells the sidebar happens
    /// to be wide, leaving sixty empty cells to the right of it on the same
    /// row; starting it after the split lines it up with the HEADER, which is
    /// laid out from exactly this cell and always has been.
    ///
    /// It stops before the panels for the same reason it starts after the
    /// sidebar. What is left over is elided, by CLUSTER — a two-hundred
    /// character path in a seventy-column footer is the case that made the
    /// elision necessary at all.
    fn crumb_box(&self, width: u16) -> (u16, u16) {
        let panes = self.panes(width);
        let from = panes.listing_from();
        (from, width.saturating_sub(from).saturating_sub(panes.right))
    }

    /// The path above, as clickable pieces.
    ///
    /// `width` is the region's, in cells: the row belongs to the listing and
    /// has to stay inside it, and [`View::crumb_box`] is where that is decided.
    #[must_use]
    pub fn crumbs(&self, width: u16) -> Vec<Field> {
        let (from, room) = self.crumb_box(width);
        let parts = listing::ancestry(&self.where_);
        // WHAT WAS TYPED IS PAID FOR FIRST, and the path gives way to it.
        //
        // The query is at the END of the path (see below) and it is what the
        // hand is doing RIGHT NOW; the path is where one already is. Eliding
        // from the end without reserving for the query would make the letters
        // one is typing the first thing to disappear, which is the opposite of
        // what a query at the end of a path is for.
        let typed = width_of(&self.filter).min(room);
        let for_path = room.saturating_sub(typed);
        let mut pieces: Vec<(String, Hit, Color)> = Vec::new();
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
            pieces.push((text, hit, color));
        }
        // WHOLE PIECES FROM THE END, and an ellipsis for what was dropped.
        //
        // A path is elided at the FRONT because the tail is what tells you
        // where you are — `…/shots/` answers the question and `/root/sh…`
        // does not — and it is elided by whole crumbs because each of them is
        // a click target: half a directory's name is not somewhere to go. The
        // measuring is `layout::width`, which counts CLUSTERS, so an accented
        // folder name is not cut through the middle of a letter.
        let total: u16 = pieces.iter().map(|(t, _, _)| width_of(t)).sum();
        let mut dropped = 0usize;
        if for_path == 0 {
            pieces.clear();
        } else if total > for_path {
            let mut kept = 0u16;
            // One cell for the ellipsis, which is a piece of the row too.
            let budget = for_path.saturating_sub(1);
            dropped = pieces.len();
            for (n, (text, _, _)) in pieces.iter().enumerate().rev() {
                let w = width_of(text);
                if kept + w > budget {
                    break;
                }
                kept += w;
                dropped = n;
            }
        }
        let mut fields = Vec::new();
        let mut at = from;
        if dropped > 0 {
            fields.push(Field {
                at,
                text: "…".to_owned(),
                color: ronoctua_tokens::color::TEXT_DIM,
                hit: Hit::Nothing,
                explain: None,
            });
            at += 1;
        }
        // NOT ONE WHOLE PIECE FITS: the last one is cut instead of vanishing.
        //
        // A row that says nothing at all about where you are is worse than one
        // that says part of it, and this is where the two-phase shape the rest
        // of this file uses ends up: the preference is whole crumbs, and what
        // holds is that something identifies the directory.
        if dropped == pieces.len() && !pieces.is_empty() {
            let (text, hit, color) = pieces.remove(pieces.len() - 1);
            let text =
                ronoctua_render::layout::elide_end(&text, (from + for_path).saturating_sub(at));
            let width = width_of(&text);
            fields.push(Field {
                at,
                text,
                color,
                hit,
                explain: None,
            });
            at += width;
        } else {
            for (text, hit, color) in pieces.drain(dropped..) {
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
                // ELIDED AT THE END, and only when even the whole row cannot
                // hold what is being typed. `typed` is already the query's
                // share of the room; this is what keeps the row inside it when
                // somebody types more letters than the sidebar is wide.
                text: ronoctua_render::layout::elide_end(&self.filter, typed),
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

    /// One line of the two panels, wherever they sit.
    ///
    /// `n` is the line INSIDE the panels' block: with them on the right it is
    /// the listing's own line number, and down in the strip it is the strip's.
    /// The panels do not know which, and that is the point — one builder, two
    /// places, and no branch that could answer differently in each.
    fn panel_fields(
        &self,
        n: usize,
        boxes: &[(Panel, u16, u16)],
        sheet: &[Fact],
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Field> {
        boxes
            .iter()
            .flat_map(|(which, at, room)| match which {
                Panel::Preview => self.preview_fields(n, *at, *room, texts),
                Panel::Info => self.info_fields(n, *at, *room, sheet),
            })
            .collect()
    }

    /// The preview panel's line `n`.
    ///
    /// **THE PANEL ALWAYS DRAWS SOMETHING.** With a picture the fields are
    /// BLANK — the texture is not text, and what these carry is the click
    /// target, so that clicking a picture of a file opens it exactly as
    /// clicking its name does.
    ///
    /// # With no picture it is a SENTENCE, and no longer a glyph
    ///
    /// It used to draw the type's icon, big and centred, with the reason under
    /// it. The owner asked for the icon to go: *"when there is no preview,
    /// instead of showing an icon it should say, in a very discreet
    /// formatting, that there is no preview"* — and he is right about what the glyph was doing.
    /// It said what the file IS, which the listing's own icon column already
    /// says on the row the cursor is on, and it said it at the size of a
    /// picture, so an empty panel looked like a panel with something in it.
    ///
    /// So: one dim line, and it says the REASON when the tools gave one and
    /// the plain sentence when they did not. A blank panel and a refused panel
    /// must not look the same — that is `ronoctua-inspect`'s rule, "refusal is
    /// a VALUE, not an absence" — and now neither of them is blank.
    fn preview_fields(
        &self,
        n: usize,
        at: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Field> {
        let Some(entry) = self.focused() else {
            return Vec::new();
        };
        let path = self.where_.join(&entry.name);
        let looked = self.looked.as_ref().filter(|l| l.path == path);
        let target = Hit::Name(self.cursor);
        if looked.is_some_and(|l| l.rgba.is_some()) {
            // A FIELD OF BLANKS, and not "no field at all". A picture has no
            // text to hit-test against, and a panel one cannot click would be
            // the mouse reaching less than the keyboard — the one direction
            // `files-everything-has-a-key` rules out.
            return vec![Field {
                at,
                text: " ".repeat(usize::from(room)),
                color: ronoctua_tokens::color::TEXT_PRIMARY,
                hit: target,
                explain: None,
            }];
        }
        // ONE LINE, AND NOTHING ON THE REST: a line that does nothing must not
        // look like a line that does something, which is the rule the blank
        // rows of a short listing already follow.
        if n != 0 {
            return Vec::new();
        }
        // THE TOOLS' OWN REASON WHEN THERE IS ONE — "there is no picture
        // inside this kind of file", "`ffmpeg` is not installed" — and the
        // plain sentence when there is not. The reason is the better answer
        // and it is the one that wins; the sentence is what a directory, a
        // broken link and a file still being decoded get.
        let text = match looked.and_then(|l| l.why.as_ref()) {
            Some(why) => {
                let args: Vec<(&str, String)> = why.args();
                let borrowed: Vec<(&str, &str)> =
                    args.iter().map(|(k, v)| (*k, v.as_str())).collect();
                texts.t_with(why.key(), &borrowed)
            }
            None => texts.t(NO_PREVIEW),
        };
        let text = ronoctua_render::layout::elide_end(&text, room);
        vec![Field {
            at: centred(at, room, width_of(&text)),
            text,
            // THE DIMMEST TEXT THERE IS, which is the whole of the "very
            // discreet formatting" that was asked for: no box, no rule, no
            // second size. Hierarchy is colour and air (invariant 15).
            color: ronoctua_tokens::color::TEXT_DIM,
            hit: target,
            explain: None,
        }]
    }

    /// The info panel's line `n`: one fact, label and value.
    ///
    /// # Why a row here answers the pointer like a row of the listing
    ///
    /// Because it is the same fact about the same file. The panel used to be
    /// `Hit::Nothing` throughout, on the grounds that "a fact is not something
    /// one acts on" — and that was true of a date and false of everything the
    /// listing's own row already lets one change. The owner reported it as
    /// exactly that asymmetry: the permission letters are clickable in the
    /// table and dead in the panel that exists to show them.
    ///
    /// So the targets come from [`Fact::does`], which is decided once in
    /// [`View::sheet_of`], and every one of them is a target the LISTING
    /// already produces — the same `Hit`, the same `confirm_mode`, the same
    /// picker. There is no second machinery, which is what stops the two
    /// disagreeing about what clicking a `w` does.
    ///
    /// **And what a row cannot offer is not drawn as if it could.** A size is
    /// not a number one types and a date is not something one edits: those
    /// stay `Hit::Nothing`, with the `explain` that carries the value
    /// UN-ELIDED — the row that had to be cut to fit is exactly the row
    /// somebody wants to read in full, and the message line is where it goes.
    /// Nothing floats (invariant 14).
    fn info_fields(&self, n: usize, at: u16, room: u16, sheet: &[Fact]) -> Vec<Field> {
        // THE OFFSET IS THE PANEL'S OWN. The listing scrolls with the region
        // and the facts do not: they are about ONE entry, and there are more
        // of them than the region has lines. See `View::facts_bar`.
        let Some(fact) = sheet.get(n.saturating_add(self.info_offset)) else {
            return Vec::new();
        };
        // THE LABEL IS DIM AND THE VALUE IS NOT — hierarchy by colour, never by
        // size, which is what the two `nothing_*` rules already ask of every
        // buffer. The label gets a third of the panel and the value the rest:
        // the label is looked past and the value is read.
        let for_label = (room / 3).max(1);
        let value_at = at.saturating_add(for_label).saturating_add(1);
        // AND ONE CELL AT THE END IS THE BAR'S, always.
        //
        // Reserved whether or not there is one to draw, because a column that
        // gave a cell back when the last fact was scrolled into view would
        // re-elide every value on the row underneath it — layout that moves
        // while one reads is worse than a cell of air. See `View::facts_bar`
        // for why the bar cannot sit in the surface's own last cell.
        let for_value = room.saturating_sub(for_label + 2);
        let mut fields = vec![Field {
            at,
            text: ronoctua_render::layout::elide_end(&fact.label, for_label),
            color: ronoctua_tokens::color::TEXT_DIM,
            hit: Hit::Nothing,
            explain: None,
        }];
        let acted = match fact.does {
            Doable::Mode => self.mode_fields(value_at, for_value),
            Doable::Ownership => self.ownership_fields(value_at, for_value, &fact.value),
            Doable::Measure => self.measure_field(value_at, for_value, &fact.value),
            Doable::Nothing => Vec::new(),
        };
        if acted.is_empty() {
            fields.push(Field {
                at: value_at,
                text: ronoctua_render::layout::elide_end(&fact.value, for_value),
                color: ronoctua_tokens::color::TEXT_PRIMARY,
                hit: Hit::Nothing,
                // THE WHOLE VALUE, for the line that explains. A path of two
                // hundred characters is exactly the fact one opened the panel
                // to read, and the panel is twenty-four cells wide.
                explain: Some(mode::Explanation {
                    key: FACT_SAYS,
                    args: vec![("label", fact.label.clone()), ("value", fact.value.clone())],
                }),
            });
        } else {
            fields.extend(acted);
        }
        fields
    }

    /// The permission triplet inside the info panel, one target per character.
    ///
    /// The same ten cells the listing's `Perms` column draws, from the same
    /// `mode::cells`, with the same `Hit`s and the same sentences — and then
    /// the octal, which is not a target because there is nothing to flip in
    /// it. Anything past `room` is DROPPED rather than drawn: nothing leaks
    /// off the screen, which is the one rule that never gives way.
    fn mode_fields(&self, at: u16, room: u16) -> Vec<Field> {
        let Some(entry) = self.focused() else {
            return Vec::new();
        };
        let owner = self
            .roster
            .person(entry.uid)
            .map_or_else(|| entry.uid.to_string(), ToOwned::to_owned);
        let group = self
            .roster
            .group(entry.gid)
            .map_or_else(|| entry.gid.to_string(), ToOwned::to_owned);
        let n = self.cursor;
        let mut fields = Vec::new();
        for (k, cell) in mode::cells(entry.mode).into_iter().enumerate() {
            let Ok(k) = u16::try_from(k) else { break };
            if k >= room {
                break;
            }
            let hit = match (cell.special, cell.segment) {
                (Some(s), _) => Hit::Special(n, s),
                (None, mode::Segment::Bit(who, what)) => Hit::Bit(n, who, what),
                (None, mode::Segment::Kind) => Hit::Nothing,
            };
            fields.push(Field {
                at: at + k,
                text: cell.letter.to_string(),
                color: if cell.set {
                    ronoctua_tokens::color::TEXT_PRIMARY
                } else {
                    ronoctua_tokens::color::TEXT_DIM
                },
                hit,
                explain: Some(mode::explain(cell, entry.mode, &owner, &group)),
            });
        }
        // THE OCTAL, when there is room for it whole. Half of `(0644)` is not
        // a mode, and a mode read wrong is worse than one not shown.
        let octal = format!("({})", mode::octal(entry.mode));
        let used = u16::try_from(fields.len()).unwrap_or(room);
        let left = room.saturating_sub(used + 1);
        if left >= width_of(&octal) {
            fields.push(Field {
                at: at + used + 1,
                text: octal,
                color: ronoctua_tokens::color::TEXT_DIM,
                hit: Hit::Nothing,
                explain: None,
            });
        }
        fields
    }

    /// `owner:group` inside the info panel, as two targets and a colon.
    ///
    /// The colon is `Hit::Nothing` on purpose: it is punctuation between two
    /// answers, and a click on it could only guess which of the two was meant.
    fn ownership_fields(&self, at: u16, room: u16, value: &str) -> Vec<Field> {
        let Some((owner, group)) = value.split_once(':') else {
            return Vec::new();
        };
        let n = self.cursor;
        let mut fields = Vec::new();
        let mut used = 0u16;
        let mut put = |text: String, hit: Hit, explain: Option<Explanation>, dim: bool| {
            let left = room.saturating_sub(used);
            if left == 0 {
                return;
            }
            let text = ronoctua_render::layout::elide_end(&text, left);
            let width = width_of(&text);
            fields.push(Field {
                at: at + used,
                text,
                color: if dim {
                    ronoctua_tokens::color::TEXT_DIM
                } else {
                    ronoctua_tokens::color::TEXT_PRIMARY
                },
                hit,
                explain,
            });
            used += width;
        };
        put(
            owner.to_owned(),
            Hit::Owner(n),
            Some(Explanation {
                key: ronoctua_i18n::key::FILES_EXPLAIN_OWNER,
                args: vec![("who", owner.to_owned())],
            }),
            false,
        );
        put(":".to_owned(), Hit::Nothing, None, true);
        put(
            group.to_owned(),
            Hit::Group(n),
            Some(Explanation {
                key: ronoctua_i18n::key::FILES_EXPLAIN_GROUP,
                args: vec![("who", group.to_owned())],
            }),
            false,
        );
        fields
    }

    /// A DIRECTORY's size inside the info panel: the same walk `TAB` asks for.
    ///
    /// Only a directory gets one, and that is decided in [`View::sheet_of`]. A
    /// file's size is a fact the kernel already knows and nothing can change
    /// from here — so it is drawn, and it is not a target.
    fn measure_field(&self, at: u16, room: u16, value: &str) -> Vec<Field> {
        let Some(entry) = self.focused() else {
            return Vec::new();
        };
        vec![Field {
            at,
            text: ronoctua_render::layout::elide_end(value, room),
            color: ronoctua_tokens::color::TEXT_PRIMARY,
            hit: Hit::Measure(self.cursor),
            explain: Some(Explanation {
                key: ronoctua_i18n::key::FILES_EXPLAIN_MEASURE,
                args: vec![("bytes", entry.bytes.to_string())],
            }),
        }]
    }

    /// The panels' own lines, when they sit in the STRIP along the bottom.
    ///
    /// A separate list and not more rows of the listing, because the strip does
    /// not scroll: it is pinned under the last line of the region, and rows
    /// that scrolled would put it on the last page only. It is the same two
    /// panels through the same builder.
    #[must_use]
    pub fn strip_rows(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let panes = self.panes(width);
        if panes.strip == 0 {
            return Vec::new();
        }
        let boxes = panes.boxes(width);
        let sheet = if panes.info { self.sheet() } else { Vec::new() };
        (0..usize::from(panes.strip))
            .map(|n| Row {
                fields: self.panel_fields(n, &boxes, &sheet, texts),
                marked: false,
                focused: false,
            })
            .collect()
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
            raise("mount", move || {
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
            // thread then was `create /run/ronoctua-files/staged: Permission
            // denied`, three seconds on the message line, and a report of
            // "changing the owner did nothing".
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
                .unwrap_or_else(|| "file".to_owned());
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
            tracing::warn!(error = %e, "the transfer queue did not take the job");
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
/// Starts the thread that looks INSIDE files.
///
/// # Why one long-lived NAMED thread, and not one per ask
///
/// Because holding an arrow key down over a folder of videos is sixty asks a
/// second, and each one is `ffmpeg` seeking a frame. One thread per ask would
/// spawn sixty processes to throw away fifty-nine of them; this one takes the
/// NEWEST and drops the rest on the floor, which is the answer somebody
/// scrolling actually wants. It is the arrangement `apps::painter` already has,
/// for the same reason and with the same two channels: `mpsc` in, because only
/// it has a blocking `recv` and the thread should sleep while nobody asks, and
/// the loop's own `calloop` channel out.
///
/// **Nothing here touches the frame.** `ronoctua-inspect` is synchronous and
/// spawns children; waiting for one inside the loop that draws is the trap this
/// project paid for once with `systemd-run --scope`.
#[must_use]
pub fn inspector(out: calloop::channel::Sender<Report>) -> Option<std::sync::mpsc::Sender<Ask>> {
    let (queue, asks) = std::sync::mpsc::channel::<Ask>();
    let born = std::thread::Builder::new()
        .name("ronoctua-inspect".to_owned())
        .spawn(move || {
            while let Ok(first) = asks.recv() {
                // ONLY THE NEWEST. Everything queued behind it is a file the
                // cursor has already left, and decoding it would only make the
                // one being looked at arrive later.
                let mut ask = first;
                while let Ok(newer) = asks.try_recv() {
                    ask = newer;
                }
                let made = ronoctua_inspect::pixels::preview(&ask.path, ask.mime, ask.target);
                let facts = ronoctua_inspect::facts::facts(
                    &ask.path,
                    ask.mime,
                    // WITHHELD, and it is the default for a reason the crate
                    // spells out: the panel is what somebody opens just before
                    // sending a photograph to a stranger, and "where this was
                    // taken" is most often the house it was taken in.
                    ronoctua_inspect::facts::Place::Withheld,
                );
                let rgba = made.rgba.map(|r| ronoctua_render::image::Rgba {
                    width: r.width,
                    height: r.height,
                    pixels: r.pixels,
                });
                if let Some(r) = rgba.as_ref() {
                    tracing::debug!(
                        width = r.width,
                        height = r.height,
                        kind = ask.mime,
                        "preview decoded"
                    );
                }
                let looked = Looked {
                    key: ask.key,
                    path: ask.path,
                    rgba,
                    why: made.why,
                    facts,
                };
                if out.send(Report::Inspected(Box::new(looked))).is_err() {
                    // A closed channel = the door left.
                    return;
                }
            }
        });
    match born {
        Ok(_) => Some(queue),
        Err(e) => {
            tracing::error!(error = %e, "could not create the inspection thread");
            None
        }
    }
}

fn raise(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(e) = std::thread::Builder::new()
        .name(format!("files-{name}"))
        .spawn(work)
    {
        tracing::error!(error = %e, thread = name, "could not start the thread");
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
    // THE TWO PANELS, and both are LETTERS for the reason the sidebar's four
    // are: `Footer::press_key` sends an arrow held with the RIGHT `Ctrl` to
    // `navigate` before the view sees it, and the window door does not track
    // the right Ctrl at all — so `C-<arrow>` would cross in one door and page
    // in the other.
    //
    // `C-i` IS NOT `TAB` HERE, and somebody will assume it is: in a terminal
    // they are the same byte. They are not the same key on Wayland — `key_of`
    // maps `Keysym::Tab` and `Keysym::i` separately — and the test beside this
    // table pins it, because the day they collided `TAB` would silently stop
    // measuring a directory.
    ("C-v", "files-preview"),
    ("C-i", "files-facts"),
    // AND THE FACTS SCROLL, because the panel no longer stretches the region
    // to hold them — see `View::panel_lines`. It PAGES and it WRAPS, in one
    // key, and the shape is `files-sort`'s: a list of a dozen rows that comes
    // back round is a key one can hold down without having to know where the
    // end is. The bar says where one is.
    //
    // Punctuation and not a letter, and it is not an exception: `C-<` and
    // `C->` are already here, and every bare key is a letter of the query.
    ("C-.", "files-facts-scroll"),
    // THE THREE COLUMNS ONE LOOKS UP, hidden until asked for.
    //
    // `C-d` for the DETAIL, and a letter for the reason the sidebar's four
    // are: `Footer::press_key` sends an arrow held with the right `Ctrl` to
    // `navigate` before the view sees it.
    ("C-d", "files-columns"),
    // AND THE FOUR SECTIONS, WALKED BY `TAB`.
    //
    // `TAB` ALREADY MEANT THIS ONE ROOM AWAY: the notification manager's own
    // key line says `TAB pane`, and it has said so since it had
    // two columns. Two managers in one shell disagreeing about what `TAB` does
    // is the divergence the two doors exist to prevent, one level up.
    //
    // So `files-measure` moved off it, to `C-t` — "total", the thing it
    // answers, and a letter for the reason the sidebar's four are letters:
    // `Footer::press_key` sends an arrow held with the RIGHT `Ctrl` to
    // `navigate` before the view sees it.
    ("C-t", "files-measure"),
    ("TAB", "files-pane-next"),
    // AND BACKWARDS, which is the one key in this table that carries `S-`.
    //
    // Not an exception: `S-TAB` is not "Shift plus Tab" as far as anything here
    // is concerned, it is the keysym `ISO_Left_Tab`, which is what xkb produces
    // for it — the same "xkb has already applied it" both doors' `key_of` rely
    // on when they refuse to count Shift. The two doors read the keysym and set
    // the flag from THAT, so neither of them is guessing at a modifier.
    ("S-TAB", "files-pane-previous"),
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
    // AND THE TWO PANELS, read out of the same table.
    //
    // Neither is a command of the registry, so neither is in `M-x` or in the
    // manual: this line is the only thing on screen that says they exist, and
    // `C-o` already cost this file the lesson — implemented, working, and
    // mentioned nowhere, "which is the same as not having it". Interpolated and
    // never written into the sentence, because keys move.
    let preview = key_that_runs("files-preview").map_or_else(String::new, |t| t.notation());
    let facts = key_that_runs("files-facts").map_or_else(String::new, |t| t.notation());
    // AND THE TWO NEWEST, in a sentence of their own APPENDED to the line.
    //
    // A second key and not a longer first one, because the first one is
    // already written in two languages and the two halves are about different
    // things: what is always there, and what one asks for. The join is a
    // single space and the separator lives inside the value — a `·` written
    // here would be a visual literal in Rust, which is what invariant 9 is
    // about.
    let columns = key_that_runs("files-columns").map_or_else(String::new, |t| t.notation());
    let scroll = key_that_runs("files-facts-scroll").map_or_else(String::new, |t| t.notation());
    let more = texts.t_with(
        ronoctua_i18n::key::FILES_HINT_MORE,
        &[("columns", columns.as_str()), ("facts", scroll.as_str())],
    );
    // AND THE FOUR SECTIONS, in a third sentence.
    //
    // A sentence of its own for the reason `files-hint-more` is one: the two
    // halves are about different things, and this one is about a key that is in
    // no registry and no manual — the lesson `C-o` cost this file, said a third
    // time. BOTH keys are named, because a person who is told about `TAB` and
    // not about `S-TAB` has to guess that the way back exists.
    let next = key_that_runs("files-pane-next").map_or_else(String::new, |t| t.notation());
    let back = key_that_runs("files-pane-previous").map_or_else(String::new, |t| t.notation());
    let more = format!(
        "{more}{}",
        texts.t_with(
            ronoctua_i18n::key::FILES_HINT_PANES,
            &[("next", next.as_str()), ("back", back.as_str())],
        )
    );
    if !sidebar {
        let first = texts.t_with(
            ronoctua_i18n::key::FILES_HINT,
            &[
                ("key", menu.as_str()),
                ("preview", preview.as_str()),
                ("info", facts.as_str()),
            ],
        );
        return format!("{first} {more}");
    }
    let side = key_that_runs("files-sidebar-focus").map_or_else(String::new, |t| t.notation());
    let first = texts.t_with(
        ronoctua_i18n::key::FILES_HINT_SIDEBAR,
        &[
            ("key", menu.as_str()),
            ("side", side.as_str()),
            ("preview", preview.as_str()),
            ("info", facts.as_str()),
        ],
    );
    format!("{first} {more}")
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
            Some("files-preview") => self.toggle_panel(Panel::Preview),
            Some("files-facts") => self.toggle_panel(Panel::Info),
            Some("files-facts-scroll") => self.scroll_facts(),
            Some("files-pane-next") => self.cycle_pane(true),
            Some("files-pane-previous") => self.cycle_pane(false),
            Some("files-columns") => {
                self.details_on = !self.details_on;
                Step::Changed
            }
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
            // AND THE SIDEBAR OPENS ON THE FIRST CLICK, which is where I put
            // the rule too widely and the owner corrected it: select-then-open
            // is for the LISTING and nothing else. A place in the sidebar is a
            // destination, not a selection — asking for two clicks to reach
            // `trash` would be two clicks for every one of them.
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
            // FIRST CLICK SELECTS, SECOND OPENS.
            //
            // The owner asked for it in those words, and it is the gesture a
            // listing wants: opening on the first click means a mis-aimed press
            // plays a video, and there is no way to just LOOK at a row with the
            // mouse — which with the preview and the facts on the right is most
            // of what one is doing.
            //
            // The pane has to match as well as the row: with the focus in the
            // sidebar the listing has a cursor too, and clicking the row it
            // happens to be on would open it without ever having selected it.
            //
            // THE LISTING AND NOTHING ELSE. It was the sidebar too for one
            // round, and the owner sent it back: *"everything else I want to
            // work as it was (e.g. if I click a folder in the sidebar it has
            // to open right away on the very first click)"*.
            Hit::Name(n) => {
                if self.cursor == *n && matches!(self.pane, Pane::Listing) {
                    return self.open_focused();
                }
                self.pane = Pane::Listing;
                self.cursor = *n;
                Step::Changed
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

    /// The picker, the detail sheet and the menu, which are not columns at all.
    ///
    /// Each is MODAL: it covers the listing, and covering a question half way
    /// is worse than covering it. So none of them gets a sidebar or a panel,
    /// and none of them is a [`Section`] — they are ONE list, and the doors
    /// draw them as one. `View::plain` is what says which of the two a door is
    /// looking at.
    pub fn modal_rows(
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
            return cut(self.detail_rows(name, width));
        }
        if let Some(chosen) = self.menu {
            return cut(self.menu_rows(chosen, width, texts));
        }
        Vec::new()
    }

    /// The LISTING's own chrome: the path, then the header.
    ///
    /// The doors used to own these two rows and chain them in front of the
    /// region. They are one column's, they do not scroll, and both doors and
    /// [`View::hit`] have to agree about them — so they are built here.
    #[must_use]
    pub fn chrome_rows(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        if !self.plain() {
            return Vec::new();
        }
        let rows = vec![
            Row {
                fields: self.crumbs(width),
                marked: false,
                focused: false,
            },
            self.header(width, texts),
        ];
        debug_assert_eq!(rows.len(), CHROME, "the listing's chrome is two lines");
        rows
    }

    /// THE FOUR COLUMNS, each with its own extent, its own offset and its own
    /// rows.
    ///
    /// This is the one place any of that is decided; see [`Section`] for the
    /// four defects that came of deciding it in several.
    ///
    /// `chrome` is the LISTING's — the path and the header — and it is the only
    /// section that has any. It sits at that column's top and does not scroll:
    /// chained in front of the region, as it was, it took the first lines of
    /// every column and moved with the listing's offset.
    #[must_use]
    pub fn sections(
        &self,
        chrome: &[Row],
        take: usize,
        width: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Section> {
        self.sections_of(chrome, take, width, texts, None)
    }

    /// The sections, or ONE of them.
    ///
    /// # Why one is worth asking for
    ///
    /// Because `View::hit` runs on every pointer motion, and building all four
    /// means building the facts — `sheet_of` runs `examined` and walks them.
    /// Paying that per pixel of mouse movement is what the owner felt as the
    /// hover lagging behind the pointer across a listing. He is right that it
    /// should not happen at all: what a panel shows is about the CURSOR, and
    /// the pointer moving changes nothing it displays.
    ///
    /// So the point resolution asks for the column it is over, and the painter
    /// asks for all four — one definition, two appetites.
    #[must_use]
    pub fn sections_of(
        &self,
        chrome: &[Row],
        take: usize,
        width: u16,
        texts: &ronoctua_i18n::Texts,
        only: Option<Pane>,
    ) -> Vec<Section> {
        let panes = self.panes(width);
        let mut out = Vec::with_capacity(4);
        let side = self.side.shown(self.hidden);

        // `take` IS "AT MOST", AND NEVER "EXACTLY".
        //
        // `View::rows` asks for `usize::MAX` and means "all of them" — a
        // sentinel that was safe while `take` only ever bounded an iterator
        // over a finite range. This function MATERIALISES rows, and under that
        // reading the sentinel became a request for 2^64 of them: past the end
        // of the listing `shown.get(n)` answers `None` and an empty `Row` was
        // pushed, for ever, until the allocator gave up at four gibibytes.
        //
        // It cost the owner six killed sessions in twenty minutes — the OOM
        // killer takes the terminal's whole cgroup, so the agent died with it
        // and left nothing behind to read. His own report is what found it.
        //
        // So the ceiling is what EXISTS: the longest column, chrome included.
        // Anything past that is a line nobody would draw.
        let chrome_lines = chrome.len();
        let most = self
            .shown
            .len()
            .saturating_add(chrome_lines)
            .max(if panes.sidebar == 0 { 0 } else { side.len() })
            // ONLY WHEN THEY SIT BESIDE THE LISTING. Down in the strip the
            // panels take lines OF THEIR OWN and none of the region's — the
            // same condition `View::row_count` uses, and it has to be the same
            // or the count and the drawing disagree.
            .max(if panes.right == 0 {
                0
            } else {
                self.panel_lines()
            })
            .max(chrome_lines)
            .max(1);
        // THE REGION'S HEIGHT is what the door asked for, and the strip begins
        // where it ends. `take` is about to be clamped to what the columns
        // actually hold — a short listing gives back the lines it does not use
        // — and clamping the STRIP's top with it would float the strip up
        // under the header, drawn a second time over the listing.
        let region = take;
        let take = take.min(most);

        let wanted = |p: Pane| only.is_none_or(|o| o == p);
        if panes.sidebar > 0 && wanted(Pane::Sidebar) {
            let first = self.side_offset.min(side.len().saturating_sub(1));
            out.push(Section {
                pane: Pane::Sidebar,
                from: 0,
                to: panes.sidebar,
                top: 0,
                lines: take,
                offset: first,
                total: side.len(),
                above: 0,
                content: (0..take)
                    .map(|k| Row {
                        fields: side
                            .get(first + k)
                            .and_then(|r| r.as_ref())
                            .map_or_else(Vec::new, |place| {
                                self.side_fields(first + k, place, panes.sidebar, texts)
                            }),
                        marked: false,
                        focused: false,
                    })
                    .collect(),
            });
        }

        // THE LISTING: its chrome first, then its rows from ITS offset.
        let above = chrome_lines.min(take);
        if wanted(Pane::Listing) {
            let layout = layout_beside(width, panes.listing_from(), panes.right, self.details_on);
            let first = self.list_offset;
            let listing_to = if panes.right > 0 {
                width.saturating_sub(panes.right)
            } else {
                width
            };
            let mut rows: Vec<Row> = chrome.to_vec();
            for k in above..take {
                let n = first + k - above;
                rows.push(self.shown.get(n).map_or_else(
                    || Row {
                        fields: Vec::new(),
                        marked: false,
                        focused: false,
                    },
                    |i| self.row_of(n, *i, &layout),
                ));
            }
            // "NOTHING HERE" IS A FIELD OF THE LISTING, on its own first row.
            if self.shown.is_empty()
                && let Some(row) = rows.get_mut(above)
            {
                row.fields.push(Field {
                    at: layout.from + GUTTER,
                    text: texts.t(ronoctua_i18n::key::FILES_NOTHING_HERE),
                    color: ronoctua_tokens::color::TEXT_DIM,
                    hit: Hit::Nothing,
                    explain: None,
                });
            }
            out.push(Section {
                pane: Pane::Listing,
                from: panes.listing_from(),
                to: listing_to,
                top: 0,
                lines: take,
                offset: first,
                total: self.shown.len(),
                above,
                content: rows,
            });
        }
        // THE PANELS: beside the listing, or under it — and either way starting
        // at the top of their OWN band.
        if panes.right > 0 || panes.strip > 0 {
            let boxes = panes.boxes(width);
            let sheet = self.sheet();
            let top = if panes.strip > 0 { region } else { 0 };
            let lines = if panes.strip > 0 {
                usize::from(panes.strip)
            } else {
                take
            };
            for (which, at, room) in boxes.iter().copied() {
                let pane = match which {
                    Panel::Preview => Pane::Preview,
                    Panel::Info => Pane::Info,
                };
                if matches!(pane, Pane::Info) {
                    // HOW MANY LINES THE FACTS WERE GIVEN, remembered for the
                    // key that pages them (`C-.`). It used to be `facts_bar`
                    // that recorded it, and that function is gone: the bar is
                    // the SECTION's rectangle now.
                    self.drawn_lines
                        .set(u16::try_from(lines).unwrap_or(u16::MAX));
                }
                if !wanted(pane) {
                    continue;
                }
                out.push(Section {
                    pane,
                    from: at,
                    to: at.saturating_add(room),
                    top,
                    lines,
                    offset: if matches!(pane, Pane::Info) {
                        self.info_offset
                    } else {
                        0
                    },
                    total: if matches!(pane, Pane::Info) {
                        sheet.len()
                    } else {
                        1
                    },
                    above: 0,
                    content: (0..lines)
                        .map(|k| Row {
                            fields: match which {
                                Panel::Preview => self.preview_fields(k, at, room, texts),
                                Panel::Info => self.info_fields(k, at, room, &sheet),
                            },
                            marked: false,
                            focused: false,
                        })
                        .collect(),
                });
            }
        }
        out
    }

    /// WHAT IS UNDER A POINT — the one question both doors ask, answered per
    /// SECTION.
    ///
    /// # Why this had to exist
    ///
    /// Because the manager draws one `Row` per line carrying every section's
    /// fields, and the doors asked "which ROW is this `y`, and which field of
    /// it is this `x`". That question has no correct answer: a line is four
    /// lists at once, and a point in one of them was resolved against all four.
    /// Every symptom the owner reported came out of it —
    ///
    /// - clicking a folder went to the PARENT, because the listing's cell on a
    ///   line the listing does not own resolved to the path's crumb;
    /// - the sidebar's first two rows answered nothing, because those lines are
    ///   the LISTING's chrome and the whole line was refused;
    /// - the facts answered nothing, because the strip is a second list the
    ///   doors never asked;
    /// - and a pointer resting in an empty part of the facts described a file
    ///   far down the listing.
    ///
    /// So the point is resolved in one place, in this order: which SECTION owns
    /// the cell, which row of THAT section the line is, and then the field —
    /// among that section's fields only. Nothing else may ask.
    ///
    /// `line` counts from the region's first line. `room` is how many lines the
    /// region has, which is what says where the strip begins.
    #[must_use]
    pub fn hit(
        &self,
        width: u16,
        room: usize,
        line: usize,
        cell: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Target> {
        // THROUGH THE SECTIONS, which are the one definition of where each
        // column is and what it holds. This function used to rebuild every
        // column's content itself — a second opinion about the same four
        // lists, and second opinions are what every one of the owner's reports
        // turned out to be.
        let chrome = self.chrome_rows(width, texts);
        // ONE COLUMN, and not four: this runs on every pointer motion — see
        // `View::sections_of`.
        let panes = self.panes(width);
        let in_strip = panes.strip > 0 && line >= room;
        let which = panes.section_at(cell, width, in_strip);
        let sections = self.sections_of(&chrome, room, width, texts, Some(which));
        let sec = sections.iter().find(|s| s.holds(line, cell))?;
        let inside = line - sec.top;
        let f = sec.content.get(inside)?.field_at(cell)?;
        // WHICH ROW OF ITS OWN LIST. The listing's chrome is not one of them —
        // `usize::MAX` is what both doors already spell for the header — and
        // every other section is indexed from its own offset.
        let row = match sec.pane {
            Pane::Listing if inside < chrome.len() => usize::MAX,
            Pane::Listing => sec.offset + inside - chrome.len(),
            _ => sec.offset + inside,
        };
        Some(Target {
            pane: sec.pane,
            row,
            hit: f.hit.clone(),
            explain: f.explain.clone(),
        })
    }

    /// The listing's first visible row.
    ///
    /// A reader and not a setter: the doors used to push their own copy in
    /// before every question, which is the arrangement that let the two of them
    /// disagree. What moves this number is the wheel, the drag and
    /// [`View::follow`] — all of them here.
    #[must_use]
    pub const fn list_offset(&self) -> usize {
        self.list_offset
    }

    /// Puts the listing's first visible row at `to`, clamped to what exists.
    ///
    /// The doors' page keys and their scrollbar drag land here. Clamped ON THE
    /// WAY IN, so there is no way to hold an offset past the end — the state
    /// that used to produce a region drawn empty after a filter narrowed the
    /// list under it.
    pub fn scroll_listing_to(&mut self, to: usize) {
        self.list_offset = to.min(self.amount().saturating_sub(1));
    }

    /// Keeps the cursor inside the listing's visible rows.
    ///
    /// # Why it is one function and was three
    ///
    /// The footer had `follow_cursor`, the window had `follow`, and each
    /// carried its own margin — so holding `j` stopped at a different row in
    /// each door. Worse, the window compared against the region's whole height
    /// while its wheel used the height minus the chrome, and the last two
    /// entries of a directory were unreachable there.
    ///
    /// `room` is what the LISTING has, chrome already taken off.
    pub fn follow(&mut self, room: usize) {
        /// How many rows of context stay visible past the cursor.
        const MARGIN: usize = 1;
        let total = self.amount();
        let last = total.saturating_sub(room);
        let cursor = self.cursor.min(total.saturating_sub(1));
        if cursor < self.list_offset.saturating_add(MARGIN) {
            self.list_offset = cursor.saturating_sub(MARGIN);
        } else if cursor + MARGIN >= self.list_offset.saturating_add(room) {
            self.list_offset = cursor + MARGIN + 1 - room;
        }
        self.list_offset = self.list_offset.min(last);
    }

    /// Puts a section's first visible row where the pointer is, dragging its
    /// bar.
    ///
    /// `fraction` is how far down the region the pointer is, from nought to
    /// one.
    ///
    /// Every section that scrolls is here, the listing included. It used to
    /// answer `false` for the listing — "not mine, the door's" — and each door
    /// then wrote the case again; the owner found the halves that were missing
    /// by dragging them.
    pub fn drag_to(&mut self, pane: Pane, fraction: f64, width: u16, room: Room) -> bool {
        // EACH SECTION SCROLLS INSIDE ITS OWN HEIGHT — see [`Room`].
        let room = match pane {
            Pane::Info => room.panel,
            _ => room.list,
        };
        let total = match pane {
            Pane::Sidebar => self.side.shown(self.hidden).len(),
            Pane::Info => self.sheet().len(),
            Pane::Listing => self.amount(),
            // THE PREVIEW DOES NOT SCROLL: it is one picture, or one sentence
            // saying there is none. There is nothing to put a bar on.
            Pane::Preview => return false,
        };
        let _ = width;
        // THROUGH `Scroll::offset_at`, which is `thumb`'s own inverse.
        //
        // The mapping used to be written out here as `fraction * last`, and
        // that is not the inverse of what was DRAWN: the thumb has a length,
        // so the offsets are spread over `track - thumb` and not over the
        // track. Dragging to the end landed half way down the column, which
        // the owner reported after trying it with the mouse.
        //
        // The unit is one, which is what makes the track a pure ratio: the
        // door has already turned pixels into a fraction of the band, and
        // this side has no pixels to turn them back into.
        let bar = ronoctua_design::Scroll {
            total,
            lines: room,
            offset: 0,
            top: 0,
        };
        #[allow(clippy::cast_possible_truncation)]
        let at = fraction.clamp(0.0, 1.0) as f32;
        let to = bar.offset_at(at, 1.0, 0.0);
        match pane {
            Pane::Sidebar => self.side_offset = to,
            Pane::Listing => self.list_offset = to,
            _ => self.info_offset = to,
        }
        true
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
            // THE PREVIEW MOVES THE LISTING, because that is what moves the
            // PICTURE. See `Pane::Preview`: the panel is about the focused
            // entry, so the only thing an arrow can mean in it is "another
            // entry", and a pane whose arrows did nothing would be a pane one
            // learns to Tab straight past.
            Pane::Preview => self.move_by(by),
            Pane::Info => self.step_facts(by),
        }
    }

    /// Opens whatever the cursor is on, in whichever pane has it.
    fn open_here(&mut self) -> Step {
        match self.pane {
            Pane::Listing => self.open_focused(),
            Pane::Sidebar => self.open_side(),
            // BOTH PANELS ARE ABOUT THE FOCUSED ENTRY, so `Enter` in either of
            // them opens it — the same thing clicking the picture already does,
            // because `preview_fields` hands its blank field a `Hit::Name`.
            Pane::Preview | Pane::Info => self.open_focused(),
        }
    }

    /// The SECTIONS that are on screen, left to right.
    ///
    /// What `Tab` walks, and the only place that decides it. A pane that is not
    /// drawn must not be reachable — a cursor in a region nobody can see is the
    /// defect `files-sidebar` already fixed once, when hiding the sidebar left
    /// the keyboard inside it.
    #[must_use]
    pub fn panes_on(&self, width: u16) -> Vec<Pane> {
        let panes = self.panes(width);
        let placed = panes.right > 0 || panes.strip > 0;
        let mut out = Vec::with_capacity(4);
        if panes.sidebar > 0 {
            out.push(Pane::Sidebar);
        }
        // THE LISTING IS ALWAYS THERE, even empty: "nothing here" is a sentence
        // and it is drawn in the listing's own first row.
        out.push(Pane::Listing);
        if placed && panes.preview {
            out.push(Pane::Preview);
        }
        if placed && panes.info {
            out.push(Pane::Info);
        }
        out
    }

    /// Moves the keyboard to the next section, or to the one before it.
    ///
    /// # Why it wraps, and why it cannot land nowhere
    ///
    /// It wraps for the reason `files-sort` cycles: a key one can press again
    /// without having to know where the end is. And it walks [`View::panes_on`]
    /// rather than the enum, so a section that is not on screen is not a stop —
    /// with no sidebar and both panels off, `Tab` has exactly one place to be
    /// and stays there, which is a key doing nothing VISIBLE rather than a key
    /// putting the cursor somewhere nobody can see.
    ///
    /// The width is the last one DRAWN, for the reason every other key that
    /// needs one uses it: a key arrives without a width. See [`View::panes`].
    fn cycle_pane(&mut self, forward: bool) -> Step {
        let on = self.panes_on(self.drawn_at.get());
        let Some(here) = on.iter().position(|p| *p == self.pane) else {
            // THE CURRENT PANE WENT OFF SCREEN — the sidebar hidden while the
            // cursor was in it, a panel turned off. Land on the listing, which
            // is the one section that is always there.
            self.pane = Pane::Listing;
            return Step::Changed;
        };
        let n = on.len();
        let next = if forward {
            (here + 1) % n
        } else {
            (here + n - 1) % n
        };
        self.pane = on[next];
        if matches!(self.pane, Pane::Sidebar) {
            self.clamp_side();
        }
        // The MENU is about the listing's focused entry and an armed action is
        // about the row it was armed on — the same two things `cross_over`
        // leaves behind, and for the same reason.
        self.arming = None;
        Step::Changed
    }

    /// Scrolls the facts ONE line, for the arrows inside [`Pane::Info`].
    ///
    /// `files-facts-scroll` pages and wraps; this steps and STOPS. They are the
    /// two gestures a list gets everywhere else in this shell — the arrow walks
    /// and does not wrap, the page key turns and does — and having both is what
    /// makes the pane's focus worth having at all.
    fn step_facts(&mut self, by: i32) -> Step {
        let room = usize::from(self.drawn_lines.get());
        let total = self.sheet().len();
        if room == 0 || total <= room {
            return Step::Say(does(FACTS_ALL_SHOWN));
        }
        let last = total - room;
        let to = if by < 0 {
            self.info_offset.saturating_sub(by.unsigned_abs() as usize)
        } else {
            self.info_offset
                .saturating_add(by.unsigned_abs() as usize)
                .min(last)
        };
        if to == self.info_offset {
            return Step::Nothing;
        }
        self.info_offset = to;
        Step::Changed
    }

    /// Scrolls the section under the pointer, and only it.
    ///
    /// # Why the door does not do this itself
    ///
    /// Because the door owns exactly one offset — the listing's — and until now
    /// that was the only one there was. A wheel event over the facts moved it,
    /// which moved the listing, the sidebar and the panels together: one list
    /// with three names. The owner reported it as the facts' scrollbar not
    /// working, and he was reading it right, because a bar that cannot be moved
    /// alone is not that section's bar.
    ///
    /// Returns `true` when this took the gesture. `false` means the pointer was
    /// over the LISTING, whose offset is the door's — the one thing here that
    /// genuinely belongs to the door, because paging and the scrollbar's drag
    /// are its.
    ///
    /// A picture does not scroll, and saying so is the point: the gesture is
    /// swallowed rather than passed on, so a hand resting over the preview does
    /// not move a list somewhere else on the screen.
    pub fn wheel(&mut self, which: Pane, by: i64, width: u16, room: usize) -> bool {
        match which {
            // THE ARM THAT WAS NOT HERE. It answered `false`, which read as
            // "nothing happened" to one door and as "it is yours to do" to the
            // other — and the two doors implemented different halves of it.
            // The owner reported the shape exactly: the gesture worked over
            // the listing in the footer and over the facts in the window, and
            // in neither of the other two.
            Pane::Listing => {
                let total = self.amount();
                self.list_offset = slide(self.list_offset, by, total, room);
                true
            }
            Pane::Preview => true,
            Pane::Sidebar => {
                let total = self.side.shown(self.hidden).len();
                self.side_offset = slide(self.side_offset, by, total, room);
                true
            }
            Pane::Info => {
                if !self.panes(width).info {
                    return false;
                }
                let total = self.sheet().len();
                self.info_offset = slide(self.info_offset, by, total, room);
                true
            }
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
            // FROM ANYWHERE THAT IS NOT THE SIDEBAR, this key goes TO the
            // sidebar. It is not `Tab` with a different name: `Tab` walks the
            // four sections in order and this one has a destination, which is
            // what "go to the sidebar, and back" has always said. From a panel
            // it therefore crosses the whole screen in one press, which is the
            // point of having a key with a destination at all.
            Pane::Listing | Pane::Preview | Pane::Info => {
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
        // THE FACTS GO BACK TO THE TOP. They are about the entry the cursor is
        // on, and a scroll position kept across two different files opens the
        // next one half way down a list it does not have.
        self.info_offset = 0;
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
    pub fn menu_at(&mut self, line: usize, cell: u16, width: u16, room: usize) -> Step {
        // THROUGH `View::hit`, the same resolution the primary button uses.
        // It reads the SECTIONS, so a right-click lands in the column it is
        // over rather than on whatever the flattened line happened to hold.
        //
        // `room` is the door's, because a section's extent depends on it — the
        // strip begins where the region ends, and a menu opened without it
        // would resolve against a region one line tall.
        let hit = self
            .hit(width, room, line, cell, &self.texts.clone())
            .map(|t| t.hit);
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
                if matches!(self.focus_row(line), Step::Nothing) && !self.is_empty() {
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
    /// It used to, and it was reported from the screen: *"pressing an item
    /// of the sidebar that is not mounted should not mount it
    /// automatically"*. Mounting is asking the system for something — it can
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
            tracing::warn!(error = %e, "could not save the sidebar");
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
    /// The layout only. What is IN it comes from [`View::sheet_of`], which is
    /// the one builder the right-hand column, the bottom strip and this modal
    /// sheet all read — three homes, one answer to "what is known about this
    /// file".
    ///
    /// It takes no bundle any more: the labels are already words by the time
    /// they leave `sheet_of`, which reads the view's OWN `texts` — the same
    /// `Rc` both doors handed it.
    fn detail_rows(&self, name: &str, width: u16) -> Vec<Row> {
        let lines = self.sheet_of(name);
        // The label column is as wide as the widest label, so the values line
        // up — a sheet whose values start at seven different columns is a
        // sheet one reads seven times.
        let widest = lines.iter().map(|f| width_of(&f.label)).max().unwrap_or(0);
        lines
            .into_iter()
            .map(|f| Row {
                // THE SHEET IS READ AND NOT ACTED ON, and it stays that way:
                // every key closes it, so a target here would be a promise the
                // next keystroke breaks. What `Fact::does` reaches is the
                // PANEL, which is beside a listing that is still live.
                fields: vec![
                    Field {
                        at: 0,
                        text: f.label,
                        color: ronoctua_tokens::color::TEXT_DIM,
                        hit: Hit::Nothing,
                        explain: None,
                    },
                    Field {
                        at: widest + 2,
                        text: ronoctua_render::layout::elide_end(
                            &f.value,
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

    /// Everything known about the entry the cursor is on.
    fn sheet(&self) -> Vec<Fact> {
        let Some(name) = self.focused().map(|e| e.name.clone()) else {
            return Vec::new();
        };
        self.sheet_of(&name)
    }

    /// Everything known about one entry, as [`Fact`]s already in words.
    ///
    /// # Why ONE builder for THREE homes
    ///
    /// Because the right-hand column, the bottom strip and the modal sheet the
    /// menu's *information* row opens are three places showing the same list,
    /// and a second builder is a second answer to "what does this file say".
    /// It is the same argument that keeps the two doors on one `View`.
    ///
    /// What the FILESYSTEM knows comes first — the path, the type, the mode,
    /// the owner, the size, the date — and what the CONTENT says follows, from
    /// `ronoctua_inspect::facts`. The order is the reading order: one answers
    /// "which file is this", the other "what is inside it".
    ///
    /// **A fact with no answer is ABSENT.** Never a row saying "unknown": a
    /// panel of unknowns is a panel nobody reads twice, and the crate that
    /// produces them already refuses to emit one.
    fn sheet_of(&self, name: &str) -> Vec<Fact> {
        let texts = std::rc::Rc::clone(&self.texts);
        let Some(entry) = self.entries.iter().find(|e| e.name == name) else {
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

        // WHAT EACH ROW OFFERS IS DECIDED HERE, once, beside the value it is
        // about — and not in the drawing, which would have to work out what
        // kind of fact it was holding from the text of it.
        let mut lines: Vec<(&'static str, String, Doable)> = vec![
            (
                ronoctua_i18n::key::FILES_DETAIL_PATH,
                path.to_string_lossy().into_owned(),
                Doable::Nothing,
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_TYPE,
                match sniffed {
                    Some(found) if found != named => format!("{named} / {found}"),
                    _ => named.to_owned(),
                },
                Doable::Nothing,
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_MODE,
                format!(
                    "{} ({})",
                    ronoctua_files::mode::shown(entry.mode),
                    ronoctua_files::mode::octal(entry.mode)
                ),
                Doable::Mode,
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_OWNER,
                format!("{owner}:{group}"),
                Doable::Ownership,
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_SIZE,
                measured.unwrap_or_else(|| size_text(entry.bytes)),
                // A DIRECTORY'S SIZE IS A WALK and a file's is already known:
                // one of the two is a thing to ask for and the other is a
                // thing to read. See `measure_field`.
                if matches!(entry.kind, Kind::Directory) {
                    Doable::Measure
                } else {
                    Doable::Nothing
                },
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_MODIFIED,
                format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}",
                    c.year, c.month, c.day, c.hour, c.minute
                ),
                Doable::Nothing,
            ),
            (
                ronoctua_i18n::key::FILES_DETAIL_LINKS,
                entry.links.to_string(),
                Doable::Nothing,
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
                Doable::Nothing,
            ));
        }
        let mut out: Vec<Fact> = lines
            .into_iter()
            .map(|(k, value, does)| Fact {
                label: texts.t(k),
                value,
                does,
            })
            .collect();
        // AND WHAT THE TOOLS SAID, when they have been asked about THIS file.
        //
        // The path is compared and not only the name: the cursor moves while a
        // child process runs, and facts about the previous file under the
        // current file's name is the one failure this panel must not have.
        //
        // NONE OF THEM IS A TARGET, and that is not an oversight: a
        // photograph's ISO and a video's frame rate are what the file IS, and
        // nothing this manager does could change one.
        if let Some(l) = self.looked.as_ref().filter(|l| l.path == path) {
            out.extend(l.facts.iter().map(|f| Fact {
                label: texts.t(f.label),
                value: fact_said(&f.value, &texts),
                does: Doable::Nothing,
            }));
        }
        out
    }

    /// Turns a panel on, or off.
    ///
    /// **They TOGGLE; they do not take focus.** There is no third [`Pane`], and
    /// that is a decision and not an omission: a panel with nothing to choose
    /// in it is not a list, and a cursor that could walk into a place where
    /// every key answers [`Step::Nothing`] is exactly what `cross_over`
    /// refuses in writing. So the `Esc` ladder is untouched and `Esc` goes on
    /// meaning what it means everywhere else.
    ///
    /// **A toggle that turns a panel on where it cannot appear flips the state
    /// ANYWAY and says why** — the shape `files-sidebar-only` already has. The
    /// alternative is a key that does nothing and says nothing, which is the
    /// one outcome this project treats as a defect: from the outside, a refusal
    /// and a dead key look the same.
    fn toggle_panel(&mut self, which: Panel) -> Step {
        match which {
            Panel::Preview => self.preview_on = !self.preview_on,
            Panel::Info => {
                self.info_on = !self.info_on;
                // TURNED OFF AND ON AGAIN IS A FRESH LOOK. Coming back to a
                // panel scrolled to where it was three files ago is the same
                // defect `move_by` clears, one gesture up.
                self.info_offset = 0;
            }
        }
        // ASKING AGAIN COSTS NOTHING AND ANSWERS EVERYTHING: the width is the
        // one the view was last DRAWN at, which is the only width that is true
        // of what is on screen. See `View::drawn_at`.
        let width = self.drawn_at.get();
        let panes = self.panes(width);
        if (panes.preview || panes.info) && panes.right == 0 && panes.strip == 0 {
            return Step::Say(mode::Explanation {
                key: PANEL_NO_ROOM,
                args: vec![("cells", PANEL_FLOOR.to_string())],
            });
        }
        Step::Changed
    }

    /// Turns the info panel to its next page of facts, and round to the start.
    ///
    /// # Why it pages, and why it wraps
    ///
    /// A page because the panel is a block one reads, not a line one steps
    /// through: a fact list of a dozen rows in four lines is three presses,
    /// and three presses of one key is less to learn than two keys. It wraps
    /// for the reason `files-sort` cycles — a key one can press again without
    /// having to know where the end is — and [`View::facts_bar`] is what says
    /// where one has got to, so the wrap is never a surprise.
    ///
    /// **With everything already on screen it SAYS so.** From the outside a
    /// key that refuses and a key that is dead look the same, and that is the
    /// one outcome this project treats as a defect.
    fn scroll_facts(&mut self) -> Step {
        // THE WIDTH AND THE HEIGHT ARE BOTH THE LAST ONES DRAWN, which are
        // the only ones true of what is on screen. A key arrives with neither
        // — see `View::drawn_at` and `View::drawn_lines`.
        let width = self.drawn_at.get();
        let room = usize::from(self.drawn_lines.get());
        let total = self.sheet().len();
        if !self.panes(width).info || room == 0 || total <= room {
            return Step::Say(does(FACTS_ALL_SHOWN));
        }
        self.info_offset = self.info_offset.saturating_add(room);
        if self.info_offset >= total {
            self.info_offset = 0;
        }
        Step::Changed
    }

    /// What the inspector should be asked for, or `None` when nothing is due.
    ///
    /// # Why the ASK is remembered and not only the answer
    ///
    /// Because `None` is an ANSWER — "this file has no picture" — and without
    /// somewhere to write "I have asked, and that was the reply" the host asks
    /// again on every frame, which is a child process per frame. It is the rule
    /// `surface`'s `icons` map already learned, and the reason
    /// `ronoctua_inspect::Made` carries the refusal alongside the pixels.
    ///
    /// `target` is `space.files.panelPixels`; the crate quantises it, so
    /// dragging a window's edge re-decodes at most twice.
    pub fn wants_look(&mut self, target: u32) -> Option<Ask> {
        if !self.preview_on && !self.info_on {
            return None;
        }
        // COPIED OUT BEFORE ANYTHING IS WRITTEN. `focused()` borrows the whole
        // view and the line that records the ask writes to it; taking the five
        // facts first is what lets the two coexist without cloning the entry.
        let (kind, name, bytes, modified, broken) = {
            let entry = self.focused()?;
            (
                // WHAT IT OPENS AS, and never `kind`. A symlink to a
                // photograph is a photograph when you look at it — following
                // the link is the one operation that should — and `kind` would
                // have refused every one of them with nothing on screen saying
                // why. Found by pointing the panels at a directory of symlinks.
                entry.opens_as(),
                entry.name.clone(),
                entry.bytes,
                entry.modified,
                entry.broken(),
            )
        };
        // A DIRECTORY HAS NO CONTENT TO LOOK INTO. Neither has a broken link,
        // and asking about one would spawn a child to be told what `lstat`
        // already said.
        if !matches!(kind, Kind::File) || broken {
            return None;
        }
        let path = self.where_.join(&name);
        // THE SECONDS AS THE ENTRY CARRIES THEM. `Entry::modified` is already an
        // `i64` of seconds since the epoch — the listing formats it with
        // `when::civil` — so there is no `SystemTime` in the middle to convert
        // and get wrong.
        let stamp = ronoctua_inspect::Stamp::new(path.clone(), modified, bytes, target);
        let key = stamp.key();
        if self.asked.as_deref() == Some(key.as_str()) {
            return None;
        }
        self.asked = Some(key.clone());
        // THE NAME MAY BE FINER THAN THE SNIFF, and `settled` is where that is
        // decided: a `.dng` sniffs as `image/tiff` because it IS one, and the
        // TIFF decoder reads the RAW planes and refuses.
        let (named, sniffed) = mime::examined(&path, kind);
        let mime = ronoctua_inspect::settled(named, sniffed);
        Some(Ask {
            path,
            mime,
            target,
            key,
        })
    }

    /// What the inspector came back with.
    ///
    /// A stale answer is DROPPED, not drawn: the cursor moves while a child
    /// process runs, and one file's picture under another file's name is the
    /// one failure a preview must not have.
    pub fn looked(&mut self, l: Looked) {
        if self.asked.as_deref() != Some(l.key.as_str()) {
            return;
        }
        self.looked = Some(l);
    }

    /// The texture key of the picture the preview is showing, if there is one.
    ///
    /// The door asks `Images::missing` with it BEFORE uploading — a preview is
    /// up to a mebibyte of RGBA, and putting that on the bus every frame is the
    /// transfer the key exists to avoid.
    #[must_use]
    pub fn preview_key(&self) -> Option<&str> {
        self.showing_now()?;
        let l = self.looked.as_ref()?;
        l.rgba.as_ref().map(|_| l.key.as_str())
    }

    /// The pixels behind that key, for the door that has to upload them.
    #[must_use]
    pub fn preview_pixels(&self) -> Option<&ronoctua_render::image::Rgba> {
        self.showing_now()?;
        self.looked.as_ref()?.rgba.as_ref()
    }

    /// `Some` when what was looked at is still what the cursor is on.
    fn showing_now(&self) -> Option<()> {
        if !self.preview_on {
            return None;
        }
        let path = self.focused_path()?;
        (self.looked.as_ref()?.path == path).then_some(())
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
        std::fs::create_dir_all(&dir).expect("create the test directory");
        dir
    }

    /// A view with NO sidebar, which is what almost every test wants: they are
    /// about the listing, and a sidebar would move every column two dozen cells
    /// to the right for no reason.
    fn view_at(dir: &Path) -> View {
        plainly(View::at(
            dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            no_sidebar(),
        ))
    }

    /// A view as the tests below assume one: the panels OFF and the three
    /// columns one looks up ON.
    ///
    /// # Why the fixture disagrees with the default
    ///
    /// Because the default changed and what these tests are about did not.
    /// They are about the LISTING's own arithmetic at a given width — where
    /// the name ends, which column gives way first, which cell a permission
    /// bit is in — and a panel takes a third of that width away while the
    /// three columns are the ones half of them measure. Left as they come, a
    /// test written about the table would be a test about the panels.
    ///
    /// The defaults themselves are pinned, and by their own tests:
    /// `the_panels_are_on_the_moment_the_manager_opens` and
    /// `the_three_columns_one_looks_up_start_hidden`. Those two build a view
    /// through `View::at` and never come through here.
    fn plainly(mut view: View) -> View {
        view.preview_on = false;
        view.info_on = false;
        view.details_on = true;
        view
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
        plainly(View::at(
            dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            a_sidebar(rows),
        ))
    }

    /// The same, with a trash whose root is under the test's own tree.
    ///
    /// A parameter and not `$XDG_DATA_HOME`: two tests setting the same
    /// environment variable race, and the one that loses puts a file in the
    /// other's trash.
    fn view_in_trash(root: &Path) -> View {
        let bin = ronoctua_files::trash::Bin::at(root.join("Trash"));
        bin.ensure().expect("create the test trash");
        plainly(View::at(
            &bin.files(),
            Roster::default(),
            Zone::utc(),
            Some(bin),
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            no_sidebar(),
        ))
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
        Key::parse(notation).unwrap_or_else(|_| panic!("`{notation}` is not a key"))
    }

    #[test]
    fn the_columns_give_way_in_the_stated_order() {
        // THE DEFECT THIS CATCHES: an order that drifts with the last edit, so
        // the narrow case shows whatever happened to survive. Wide first, then
        // each width just below the point where one more column stops fitting.
        let wide = layout_beside(120, 0, 0, true);
        for c in [
            Column::Icon,
            Column::Name,
            Column::Perms,
            Column::Owner,
            Column::Group,
            Column::Size,
            Column::Modified,
        ] {
            assert!(wide.find(c).is_some(), "{c:?} did not fit in 120 columns");
        }
        // Narrowing drops them in the declared order and never out of it.
        let mut gone = Vec::new();
        for width in (1..=120).rev() {
            let l = layout_beside(width, 0, 0, true);
            for c in GIVE_WAY {
                if l.find(c).is_none() && !gone.contains(&c) {
                    gone.push(c);
                }
            }
        }
        assert_eq!(gone, GIVE_WAY.to_vec(), "the order of giving way changed");
    }

    #[test]
    fn the_name_never_gives_way_however_narrow_it_gets() {
        // FROM TWO, AND THE CHANGE IS DELIBERATE.
        //
        // At ONE column there is no listing at all now. The gutter and the
        // right edge take that cell between them, so the name's old `.max(1)`
        // put a cell where there was none and the row leaked — which this file
        // used to pin as a known exception. `ronoctua_design::Table` does not
        // have the exception: if not even one column places, the layout is
        // empty, because drawing into a window that narrow IS the leak.
        //
        // "Nothing leaks" is the rule that never gives way, and this was the
        // one place in the shell that broke it.
        //
        // The first width that holds a listing is therefore the two edges plus
        // one cell for the name, spelled from the tokens rather than as a
        // literal that would have to be found again when one of them moves.
        for width in (GUTTER + EDGE_RIGHT + 1)..=200u16 {
            let l = layout_beside(width, 0, 0, true);
            assert!(
                l.find(Column::Name).is_some(),
                "no name column at {width} columns"
            );
            // And nothing leaks off the edge, which is the one rule the
            // compositor will not enforce for us.
            for (c, at, w) in &l.columns {
                assert!(
                    at + w <= width.max(1) + 1,
                    "{c:?} leaks: starts at {at}, is {w} wide, the screen is {width}"
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
        let mut v = view_at(&root);
        let rows = v.drawn(120, &bundle());
        assert_eq!(rows.len(), 1);
        let (at, _) = layout_beside(120, 0, 0, true).find(Column::Perms).unwrap();
        let mut sentences = Vec::new();
        for k in 0..10u16 {
            let f = rows[0].field_at(at + k).expect("a cell with no field");
            assert_eq!(
                width_of(&f.text),
                1,
                "cell {k} does not hold a single character"
            );
            sentences.push(f.explain.clone().expect("a cell with no explanation").key);
        }
        sentences.sort_unstable();
        sentences.dedup();
        assert_eq!(sentences.len(), 10, "two cells explain the same thing");
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
            "the mark followed the index instead of the name"
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
        assert_eq!(v.amount(), 2, "the hidden ones did not appear");
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
        let small_dir = scratch("frame-small");
        for n in 0..20 {
            std::fs::write(small_dir.join(format!("f-{n:05}.txt")), b"x").unwrap();
        }
        let big = scratch("frame-big");
        for n in 0..3000 {
            std::fs::write(big.join(format!("f-{n:05}.txt")), b"x").unwrap();
        }
        let b = bundle();
        let mut v_small = view_at(&small_dir);
        let mut v_big = view_at(&big);

        // WARM FIRST: the first call through this path pays for the layout
        // table and the text bundle, and that cost belongs to neither side.
        for _ in 0..20 {
            let _ = v_small.drawn_at(&[], 0, 12, 174, &b);
            let _ = v_big.drawn_at(&[], 0, 12, 174, &b);
        }

        let rounds = 200;
        let t = std::time::Instant::now();
        for _ in 0..rounds {
            let _ = v_small.drawn_at(&[], 0, 12, 174, &b);
        }
        let small = t.elapsed();
        let t = std::time::Instant::now();
        for _ in 0..rounds {
            let _ = v_big.drawn_at(&[], 0, 12, 174, &b);
        }
        let large = t.elapsed();

        assert_eq!(
            v_big.drawn_at(&[], 0, 12, 174, &b).len(),
            12,
            "the window leaked"
        );
        // 150x the entries. A ceiling of 8x is loose enough for a debug build
        // and a loaded machine, and still an order of magnitude below the 40x
        // the whole-list version would show.
        assert!(
            large < small * 8,
            "the frame grows with the directory: 20 entries {small:?}, 3000 entries {large:?}"
        );
        let _ = std::fs::remove_dir_all(&small_dir);
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
        std::fs::create_dir(root.join("empty")).unwrap();
        std::fs::create_dir(root.join("cheia")).unwrap();
        std::fs::write(root.join("cheia/um.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        let where_at = v.where_().to_path_buf();

        // Directories come first, so rows 0 and 1 are the two folders.
        for row in [0usize, 1] {
            let step = v.focus_row(row);
            assert!(matches!(step, Step::Changed), "{step:?}");
            assert_eq!(v.cursor(), row, "the focus did not go to the clicked line");
            assert_eq!(
                v.where_(),
                where_at,
                "the right button went into the folder instead of only focusing"
            );
            assert!(!v.menu_open(), "focusing does not open the menu on its own");
        }

        // And then the menu opens, on the row that was clicked.
        let t = key_that_runs("files-menu").expect("files-menu has a key");
        v.key(t);
        assert!(v.menu_open(), "the menu did not open");
        assert_eq!(v.where_(), where_at, "the menu opened in another folder");
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
        let t = key_that_runs("files-menu").expect("files-menu has to have a key in the table");
        let step = v.key(t);
        assert!(matches!(step, Step::Changed), "{step:?}");
        assert!(v.menu_open(), "the menu key did not open the menu");
        assert!(
            v.filter().is_empty(),
            "the menu key ended up in the filter: {:?}",
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

        let plain = v.crumbs(120);
        for c in "rel".chars() {
            v.key(press(c));
        }
        let typed = v.crumbs(120);
        assert_eq!(
            typed.len(),
            plain.len() + 1,
            "the query did not become a field of the path"
        );
        let last = typed.last().expect("the path has fields");
        assert_eq!(last.text, "rel");
        // A DIFFERENT COLOUR IS THE WHOLE FORMATTING: no box, no separator
        // (invariants 14 and 15). It has to differ from the piece before it,
        // which is the directory one is in.
        let before = &typed[typed.len() - 2];
        assert_ne!(
            last.color, before.color,
            "the query came out in the same colour as the path"
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
        std::fs::create_dir(root.join("inside")).unwrap();
        std::fs::write(root.join("inside/one.txt"), b"x").unwrap();
        let mut v = view_at(&root.join("inside"));
        let del = Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Backspace,
        };

        v.key(press('u'));
        assert_eq!(v.filter(), "u");
        let step = v.key(del);
        assert!(matches!(step, Step::Changed), "{step:?}");
        assert_eq!(
            v.where_(),
            root.join("inside"),
            "it deleted a letter and WENT UP"
        );

        let step = v.key(del);
        assert!(matches!(step, Step::Changed), "{step:?}");
        assert_eq!(
            v.where_(),
            root,
            "backspace with nothing typed did not go up"
        );
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
        let rows = v.drawn(120, &b);
        let title = rows[0].fields[0].text.clone();
        assert!(!title.is_empty(), "the question came up with no title");
        assert!(
            !title.starts_with("files-"),
            "the raw key went to the screen: {title}"
        );
        assert!(
            rows[1].fields[0].text.starts_with('>'),
            "the typing box should be the SECOND line: {:?}",
            rows[1].fields[0].text
        );

        // AND THE CONFIRMATION IS ANOTHER QUESTION, with a title of its own.
        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Enter,
        });
        let asking = v.drawn(120, &b)[0].fields[0].text.clone();
        assert!(!asking.is_empty(), "a confirmacao subiu sem titulo");
        assert_ne!(
            asking, title,
            "the confirmation repeated the question's title"
        );
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
                .unwrap_or_else(|_| panic!("`{notation}` is not a key"));
            assert!(
                !t.mods.alt,
                "`{notation}` (`{id}`) uses Alt, which in the nested loop is the family \
                 of the compositor's chords — the key does not reach the view"
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
        std::fs::create_dir_all(root.join("empty")).unwrap();
        let mut v = view_at(&root.join("empty"));
        assert_eq!(v.amount(), 0);
        assert!(v.is_empty());

        let t = key_that_runs("files-menu").expect("files-menu has a key");
        let step = v.key(t);
        assert!(matches!(step, Step::Changed), "{step:?}");
        assert!(v.menu_open(), "an empty folder was left with no menu");

        let rows = v.drawn(120, &bundle());
        assert!(rows.len() > 1, "the menu came up with no rows: {rows:?}");
        // AND THE FIRST LINE SAYS WHO IT IS ABOUT — here, the folder itself.
        let title = &rows[0].fields[0].text;
        assert!(
            title.contains("empty"),
            "the menu does not say the target: {title}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_menu_says_which_entry_it_is_about() {
        // The menu COVERS the listing, so the row it was opened on is no longer
        // visible: twenty verbs with nothing saying what they act on. With
        // marks it is worse, because the answer is not the row under the
        // pointer — it is all of them.
        let root = scratch("menutitle");
        std::fs::write(root.join("chosen.txt"), b"x").unwrap();
        std::fs::write(root.join("other.txt"), b"x").unwrap();
        let mut v = view_at(&root);
        let t = key_that_runs("files-menu").expect("files-menu has a key");

        v.key(t);
        let title = v.drawn(120, &bundle())[0].fields[0].text.clone();
        assert!(title.contains("chosen.txt"), "{title}");

        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Esc,
        });
        v.key(of_view("C-a")); // mark everything
        v.key(t);
        let title = v.drawn(120, &bundle())[0].fields[0].text.clone();
        assert!(
            !title.contains("chosen.txt"),
            "with marks the target is not a single entry: {title}"
        );
        assert!(title.contains('2'), "it did not say how many: {title}");
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
        let rows = v.drawn(120, &b);
        let question = rows[0].fields[0].text.clone();
        assert!(
            question.contains("um.txt"),
            "the question does not say what it is about: {question}"
        );
        assert!(
            question.contains("0664") || question.contains("0644"),
            "the question does not say what it will become: {question}"
        );

        let answers: Vec<String> = rows[2..]
            .iter()
            .filter_map(|r| r.fields.first().map(|f| f.text.clone()))
            .collect();
        assert!(!answers.is_empty(), "the question came up with no answers");
        for a in &answers {
            assert!(
                !a.starts_with("files-"),
                "an answer with the raw key on screen: {a}"
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
        v.key(key_that_runs("files-menu").expect("the menu key"));
        v.key(press('m'));
        for r in v.drawn(120, &bundle()).iter().skip(2) {
            let Some(f) = r.fields.first() else { continue };
            assert!(
                !f.text.starts_with("files-"),
                "a preset with the raw key on screen: {}",
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
        let mut v = plainly(View::at(
            &root,
            roster,
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            no_sidebar(),
        ));

        v.key(key_that_runs("files-menu").expect("the menu key"));
        v.key(press('O')); // the menu's key for "owner"
        assert!(v.showing_detail(), "the owner question did not open");

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
        // "Changing the owner did nothing" — and it had: the request could not even
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
            eprintln!("skipping: this machine HAS /run/ronoctua-files");
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
            other => panic!("without the unit, changing owner gave back {other:?}"),
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
        assert_eq!(v.filter(), "rela", "the typing did not reach the filter");
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
            "Esc with a filter on screen must not close the view: {step:?}"
        );
        assert!(!v.filtering());
        assert_eq!(v.amount(), 2);

        // And with nothing typed, Esc closes — as every view in this system does.
        let step = v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Esc,
        });
        assert!(matches!(step, Step::Closed), "an empty Esc has to close");
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
        assert_eq!(v.filter(), "setup", "`C-s` became a filter letter");
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
            "clicking a bit must not apply straight away: {step:?}"
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
        assert!(v.menu_open(), "the menu closed before acting");
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
        let drawn = |v: &mut View| -> Vec<(String, ronoctua_tokens::Color)> {
            v.drawn(120, &bundle())
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| (f.text.clone(), f.color)))
                .collect()
        };
        let before = drawn(&mut v);
        let step = v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Down,
        });
        assert!(
            matches!(step, Step::Changed),
            "the arrow did not move the cursor"
        );
        assert_ne!(
            before,
            drawn(&mut v),
            "the cursor moved and the drawing stayed the same"
        );
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
        let name_at = layout_beside(120, 0, 0, true).find(Column::Name).unwrap().0;

        let rows = v.drawn(120, &bundle());
        let focused = rows[0].field_at(name_at).unwrap().color;
        let other = rows[1].field_at(name_at).unwrap().color;
        assert_ne!(
            focused, other,
            "the focused line has the colour of the others"
        );

        // MARKING DOES NOT TOUCH THE NAME'S COLOUR; it puts a glyph in the
        // gutter, which is cell zero.
        v.key(of_view("C-SPC"));
        let rows = v.drawn(120, &bundle());
        let mark = &rows[0].fields[0];
        assert_eq!(mark.at, 0);
        assert!(
            !mark.text.is_empty(),
            "the mark does not show in the gutter"
        );
        assert!(
            rows[1].fields[0].text.is_empty(),
            "a line with no mark has a mark"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_is_drawn_against_the_screens_edge() {
        // The icon was flush against the border and read as clipped. The gutter
        // is the same `modeline.edgeLeft` the rest of the footer leaves, and it
        // is also where the mark goes — one cell doing two jobs.
        for width in 20..=200u16 {
            let l = layout_beside(width, 0, 0, true);
            for (c, at, _) in &l.columns {
                assert!(
                    *at >= GUTTER,
                    "{c:?} starts at cell {at}, flush against the edge (screen of {width})"
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
            "choosing a preset must not apply straight away: {step:?}"
        );
        let step = v.key(enter);
        assert!(
            matches!(step, Step::Changed),
            "the confirmation's default answer has to be NO: {step:?}"
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
            other => panic!("changing the permission of three marked ones gave {other:?}"),
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
        std::fs::create_dir(root.join("inside")).unwrap();
        std::fs::write(root.join("um.txt"), b"x").unwrap();
        let mut v = view_at(&root);

        v.menu = Some(0);
        match v.run(menu::by_id("terminal").unwrap()) {
            Step::Do(Doing::Terminal(dir)) => assert_eq!(dir, root.join("inside")),
            other => panic!("a terminal in a folder gave {other:?}"),
        }

        // On a FILE there is no inside, so it is the directory one is looking
        // at — which is the other half of the same rule.
        v.cursor = 1;
        v.menu = Some(0);
        match v.run(menu::by_id("terminal").unwrap()) {
            Step::Do(Doing::Terminal(dir)) => assert_eq!(dir, root),
            other => panic!("a terminal on a file gave {other:?}"),
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
        std::fs::create_dir(root.join("folder")).unwrap();
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
                assert!(!matches!(step, Step::Nothing), "`{}` does nothing", a.id);
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
                    "the menu's `{}` row does nothing",
                    a.id
                );
                reached.insert(a.id);
            }
        }
        // And between the two entries, every row of the table was reachable.
        for a in menu::all_of() {
            assert!(
                reached.contains(a.id),
                "no entry reached the `{}` row",
                a.id
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_breadcrumb_leads_where_it_says() {
        let v = view_at(Path::new("/usr/share"));
        let crumbs = v.crumbs(120);
        assert!(crumbs.len() >= 3);
        // The last piece is where one IS, and is not a place to go.
        assert_eq!(crumbs.last().unwrap().hit, Hit::Nothing);
        assert_eq!(v.crumb_target(0), Some(PathBuf::from("/")));
        assert_eq!(v.crumb_target(1), Some(PathBuf::from("/usr")));
    }

    #[test]
    fn a_broken_link_does_not_look_like_a_working_one() {
        let root = scratch("broken");
        std::os::unix::fs::symlink("does-not-exist", root.join("broken-link")).unwrap();
        std::fs::write(root.join("target"), b"x").unwrap();
        std::os::unix::fs::symlink("target", root.join("inteiro")).unwrap();
        let mut v = view_at(&root);
        let rows = v.drawn(120, &bundle());
        let (at, _) = layout_beside(120, 0, 0, true).find(Column::Name).unwrap();
        let names: Vec<_> = rows
            .iter()
            .map(|r| r.field_at(at).unwrap())
            .map(|f| (f.text.clone(), f.color))
            .collect();
        let broken = names
            .iter()
            .find(|(t, _)| t.starts_with("broken-link"))
            .unwrap();
        let whole = names
            .iter()
            .find(|(t, _)| t.starts_with("inteiro"))
            .unwrap();
        assert_ne!(
            broken.1, whole.1,
            "the broken link has the same colour as the whole one"
        );
        // And both say where they point, which is the one thing a listing
        // cannot leave out about a link.
        assert!(broken.0.contains("does-not-exist"));
        assert!(whole.0.contains("target"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directorys_own_size_is_left_blank_until_somebody_asks() {
        // Four kibibytes is the size of the directory ENTRY and answers a
        // question nobody asked. A number that answers a different question is
        // worse than no number.
        let root = scratch("size");
        std::fs::create_dir(root.join("folder")).unwrap();
        let mut v = view_at(&root);
        let (at, _) = layout_beside(120, 0, 0, true).find(Column::Size).unwrap();
        assert_eq!(v.drawn(120, &bundle())[0].field_at(at).unwrap().text, "");
        v.measured("folder", 5 * 1024 * 1024);
        assert_eq!(
            v.drawn(120, &bundle())[0].field_at(at).unwrap().text,
            "5,0 MiB"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn clicking_a_header_sorts_by_it_and_clicking_again_reverses() {
        // The gesture every table in the world has. The `s` key goes round the
        // same cycle; neither reaches anything the other does not.
        let root = scratch("header");
        std::fs::write(root.join("large.txt"), vec![b'x'; 5000]).unwrap();
        std::fs::write(root.join("small.txt"), b"x").unwrap();
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
        assert_eq!(names(&v), ["small.txt", "large.txt"]);
        assert_eq!(v.focused().map(|e| e.name.clone()), watching);

        // Again on the same column: it reverses, and does not go back to name.
        v.click(usize::MAX, &Hit::Sort(Column::Size));
        assert_eq!(names(&v), ["large.txt", "small.txt"]);

        // A DIFFERENT column starts ascending, rather than inheriting the
        // reversal — otherwise clicking `name` after reversing `size` would
        // give Z to A, which nobody asked for.
        v.click(usize::MAX, &Hit::Sort(Column::Name));
        assert_eq!(names(&v), ["large.txt", "small.txt"]);
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
        assert_eq!(marked.len(), 1, "{} columns marked", marked.len());
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

        let rows = v.drawn(120, &bundle());
        let sheet: Vec<String> = rows
            .iter()
            .filter_map(|r| r.fields.last())
            .map(|f| f.text.clone())
            .collect();
        let joined = sheet.join(" | ");
        assert!(joined.contains("text/plain"), "{joined}");
        assert!(
            joined.contains("image/png"),
            "the content was not examined: {joined}"
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
        std::fs::create_dir(root.join("folder")).unwrap();
        let mut v = view_at(&root);
        for row in v.drawn(120, &bundle()) {
            for f in &row.fields {
                if matches!(f.hit, Hit::Nothing | Hit::Name(_) | Hit::Mark(_)) {
                    continue;
                }
                assert!(
                    f.explain.is_some(),
                    "the field `{}` ({:?}) lights up and explains nothing",
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
        // AND IT LEAKS NOWHERE, which it did not always.
        //
        // At one column it used to: the name's `.max(1)` put a cell where the
        // gutter and the edge had already taken the only one there was. That
        // exception is gone with `ronoctua_design::Table` — a width that holds
        // no column holds no row — so the list is empty rather than short.
        let leaks: Vec<u16> = (1..=120u16)
            .filter(|w| rightmost(&v.drawn(*w, &b)) > *w)
            .collect();
        assert_eq!(
            leaks,
            Vec::<u16>::new(),
            "the listing alone leaks at {leaks:?}"
        );
    }

    #[test]
    fn an_empty_sidebar_takes_no_width_at_all() {
        // A fresh install, a test's home, the nested loop before the
        // directories exist: none of the declared places is there, so the
        // sidebar has nothing to draw. Left "showing", it would take a quarter
        // of the screen and a vertical rule for nothing, and the listing would
        // give up columns to hold it.
        let dir = scratch("side-no-places");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = plainly(View::at(
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
        ));
        v.reread();
        let b = bundle();
        assert!(!v.sidebar_showing());
        assert_eq!(v.split(120), 0);
        // Compared through the DRAWING: the mark is back at cell zero, which is
        // the same thing as "the listing got the whole width".
        assert_eq!(cell_of(&v.drawn(120, &b), &Hit::Mark(0)), Some(0));
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
            let rows = v.drawn(width, &b);
            // THE SIDEBAR NEVER MAKES IT WORSE. That is the claim this test
            // owns, and it is the stronger one: the listing on its own already
            // leaks at a width of one cell, because the name's floor is
            // `.max(1)` and a table about files that does not say which is
            // worse than one cell over the edge. `the_listing_alone_leaks_…`
            // pins that, so this cannot be misread as the sidebar's doing.
            assert!(
                rightmost(&rows) <= rightmost(&alone.drawn(width, &b)).max(width),
                "at {width} columns the bar pushed something to {}",
                rightmost(&rows)
            );
            // AND THE NAME SURVIVES WHEREVER THERE IS A LISTING AT ALL. A
            // sidebar that took its quarter and left a table of unreadable
            // names would have its priorities backwards.
            //
            // "Wherever there is one" is asked of the LAYOUT and not of the
            // drawn fields: at a width that holds no column the listing is
            // empty, and an empty listing has not lost a name — see
            // `the_name_never_gives_way_however_narrow_it_gets` for why that
            // stopped being an exception. Asking the rows instead answered
            // about the sidebar's own fields, which are not the listing's.
            let split = v.split(width);
            let listing = layout_beside(width, split, 0, true);
            assert!(
                listing.columns.is_empty()
                    || rows
                        .iter()
                        .any(|r| r.fields.iter().any(|f| matches!(f.hit, Hit::Name(_)))),
                "at {width} columns the listing lost its name"
            );
            let side = cell_of(&rows, &Hit::Sidebar(0)).is_some();
            if width < ronoctua_tokens::space::FILES_SIDEBARFROM {
                assert!(
                    !side,
                    "the bar showed up at {width} columns, below the token"
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
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_with_devices(&dir);
        v.reread();
        let b = bundle();

        for width in ronoctua_tokens::space::FILES_SIDEBARFROM..=200u16 {
            let split = v.split(width);
            if split == 0 {
                continue;
            }
            let rows = v.drawn(width, &b);
            for f in rows.iter().flat_map(|r| r.fields.iter()) {
                match f.hit {
                    // THE AIR MOVED AND DID NOT GO AWAY. `space.files
                    // .sidebarGutter` is zero now, because `chrome::FIELDS_LEAD`
                    // already pays that cell for every row of the manager — the
                    // sidebar's included — and a row that bought one of its own
                    // was paying twice: the owner measured 21 px of it against
                    // 14 on the glyph's other side. What has to hold is the
                    // distance from the SURFACE's border, and that is the sum of
                    // the two; the `const` block below is what keeps the sum
                    // from becoming nought whatever either half becomes.
                    Hit::Sidebar(_) => assert_ne!(
                        f.at + crate::chrome::FIELDS_LEAD,
                        0,
                        "at {width} columns a field of the bar sat flush against the edge"
                    ),
                    // AND THE RIGHT EDGE: the glyph must not touch the rule at
                    // `split`, or the two read as one smudge instead of as a
                    // target and a boundary.
                    Hit::Eject(_) => assert!(
                        f.at + width_of(&f.text) < split,
                        "at {width} columns eject ended at {} and the rule is at {split}",
                        f.at + width_of(&f.text)
                    ),
                    _ => {}
                }
            }
        }
        // AND IT IS THE COMPILER THAT REFUSES A ZERO, not a runtime assertion
        // over two constants — the shape `chrome`'s own test already uses.
        const { assert!(crate::chrome::FIELDS_LEAD + SIDE_GUTTER >= 1) };
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

        let mut long = plainly(View::at(
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
                    label: Some("a considerably longer name".to_owned()),
                });
                s
            },
        ));
        long.reread();

        let narrow = short.split(200);
        let wide = long.split(200);
        assert!(
            wide > narrow,
            "the bar did not follow the label: {narrow} vs {wide}"
        );
        // And the short one is nowhere near a quarter of the screen.
        assert!(
            narrow < 200 / 4,
            "the bar took {narrow} columns of 200 for six-letter names"
        );
    }

    #[test]
    fn the_hint_names_the_key_that_reaches_the_sidebar() {
        // `C-o` is not a command of the registry, so it is in neither `M-x` nor
        // the manual. This line is the ONLY thing that teaches it — and it did
        // not, which is the same as the key not existing. Reported as "it is not
        // clear how I jump from the listing to the sidebar".
        let b = bundle();
        let alone = hint_of(&b, false);
        let beside = hint_of(&b, true);
        assert_ne!(alone, beside);
        let side = key_that_runs("files-sidebar-focus").unwrap().notation();
        assert!(
            beside.contains(&side),
            "the hint with a sidebar does not name `{side}`: {beside:?}"
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
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 4);
        v.reread();
        let b = bundle();
        assert!(v.plain() && v.split(120) > 0);

        v.key(of_view("C-m"));
        assert!(v.menu_open());
        // ONE ANSWER, and everything that draws beside the rows asks it.
        assert!(!v.plain());
        assert_eq!(
            v.split(120),
            0,
            "the rule would have been drawn over the menu"
        );
        assert!(
            !v.drawn(120, &b)
                .iter()
                .flat_map(|r| r.fields.iter())
                .any(|f| matches!(f.hit, Hit::Sidebar(_) | Hit::Eject(_))),
            "a field of the bar was left under the menu"
        );
    }

    #[test]
    fn the_sidebars_menu_is_the_sidebars_and_not_the_listings() {
        // TWO TABLES, TWO SUBJECTS. The listing's menu asks questions only an
        // entry can answer; a place or a disk answers none of them. Sharing one
        // table would have meant twenty rows saying "not for this kind".
        let dir = scratch("side-menu-table");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_with_devices(&dir);
        v.reread();
        let b = bundle();

        v.key(of_view("C-o"));
        v.key(of_view("C-m"));
        let hits: Vec<Hit> = v
            .drawn(120, &b)
            .iter()
            .flat_map(|r| r.fields.iter())
            .map(|f| f.hit.clone())
            .collect();
        assert!(hits.iter().any(|h| matches!(h, Hit::SideMenu(_))));
        assert!(
            !hits.iter().any(|h| matches!(h, Hit::Menu(_))),
            "the listing's menu appeared over a line of the sidebar"
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
            .drawn(120, &b)
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
        // "navigating the options with the arrows" — the answer has to be about
        // the row about to be chosen, or it is about nothing.
        let dir = scratch("menu-second-record");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_at(&dir);
        v.reread();
        let b = bundle();

        // WITH NO MENU, NOTHING. On the listing a row is a record with columns,
        // and picking one of its columns to explain because the cursor is
        // somewhere on the line would be picking at random — that is what the
        // pointer's per-field path is for, and it still is.
        assert!(
            v.cursor_explains().is_none(),
            "the listing explained a column nobody pointed at"
        );

        v.key(of_view("C-m"));
        let first = v.cursor_explains().expect("the menu's first line explains");
        assert!(
            first.key.starts_with("files-does-"),
            "the explanation is not the technical one: {}",
            first.key
        );
        // AND IT IS A SENTENCE, not a key drawn raw. The bundle has to answer
        // it, or invariant 8 would put `files-does-open` on screen in front of
        // somebody.
        let said = b.t_with(first.key, &first.borrowed());
        assert_ne!(said, first.key, "key `{}` has no text", first.key);

        // AND THE ARROW MOVES IT.
        v.key(Key {
            mods: ronoctua_keymap::Mods::default(),
            sym: Sym::Down,
        });
        let second = v.cursor_explains().expect("the second line explains too");
        assert_ne!(
            first.key, second.key,
            "the technical line did not change with the arrow: it is a caption, not a detail"
        );
    }

    #[test]
    fn the_sidebars_menu_explains_itself_too_and_with_its_own_words() {
        // THE SAME SECOND REGISTER, and it has to be the SIDEBAR's: `mount`
        // talks to udisks and `abrir` opens a folder, and an explanation taken
        // from the listing's table would describe the wrong machine entirely.
        let dir = scratch("side-menu-second-record");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_with_devices(&dir);
        v.reread();
        let b = bundle();

        v.key(of_view("C-o"));
        v.key(of_view("C-m"));
        let e = v.cursor_explains().expect("the bar's line explains");
        assert!(
            e.key.starts_with("sidebar-does-"),
            "the bar took its explanation from the wrong table: {}",
            e.key
        );
        assert_ne!(
            b.t_with(e.key, &e.borrowed()),
            e.key,
            "key `{}` has no text",
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
            label: Some("the system".to_owned()),
            section: Section::Devices,
            mounted: Some(true),
            at: Some(PathBuf::from("/")),
            quiet: false,
            system: true,
            needed: Some("/".to_owned()),
            removable: false,
            built_in: true,
        }]);
        let mut v = plainly(View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        ));
        v.reread();
        v.key(of_view("C-o"));
        v.key(of_view("C-m"));
        let b = bundle();
        // THE ROW IS NOT DRAWN AT ALL. A file manager that puts "format" one
        // key away from the disk it booted from is a file manager that erases
        // somebody's system — and a row offered that then refuses teaches that
        // the refusal is arbitrary.
        assert!(
            !v.drawn(120, &b)
                .iter()
                .flat_map(|r| r.fields.iter())
                .any(|f| f.hit == Hit::SideMenu("format") || f.hit == Hit::SideMenu("unmount")),
            "the menu offered to format or unmount the root"
        );
        // And the facts SAY why the list is short, instead of leaving it to be
        // guessed from rows that are missing.
        let said = v
            .drawn(120, &b)
            .iter()
            .flat_map(|r| r.fields.iter())
            .any(|f| f.text.contains('/'));
        assert!(said, "the sheet did not say where the system lives");
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
        v.menu_at(1, SIDE_GUTTER, 120, 12);
        assert!(v.menu_open());
        assert!(
            matches!(v.pane(), Pane::Sidebar),
            "the menu went to the wrong panel"
        );
        v.key(of_view("C-m"));

        // And a cell inside the listing.
        v.menu_at(1, split + RULE + GUTTER + 1, 120, 12);
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
            label: Some("the system".to_owned()),
            section: Section::Devices,
            mounted: Some(true),
            at: Some(PathBuf::from("/")),
            quiet: false,
            system: true,
            needed: Some("/".to_owned()),
            removable: false,
            built_in: true,
        }]);
        let mut v = plainly(View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        ));
        v.reread();
        let b = bundle();
        // NO TARGET AT ALL. A row offered that then refuses is worse than one
        // not offered: the gesture was spent to be told no, and nothing says
        // which part of the state decided.
        assert!(
            !v.drawn(120, &b)
                .iter()
                .flat_map(|r| r.fields.iter())
                .any(|f| matches!(f.hit, Hit::Eject(_))),
            "it offered to eject the system root"
        );
        // And the key refuses NAMING what decided.
        v.key(of_view("C-o"));
        match v.key(of_view("C-e")) {
            Step::Say(e) => {
                assert_eq!(e.key, ronoctua_i18n::key::FILES_UNMOUNT_NEEDED);
                assert!(e.args.iter().any(|(k, val)| *k == "why" && val == "/"));
            }
            other => panic!("unmounting the root should be refused with a reason: {other:?}"),
        }
    }

    #[test]
    fn the_mark_travels_with_the_listing() {
        let dir = scratch("side-mark");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();

        let mut beside = view_beside(&dir, 3);
        beside.reread();
        let mut alone = view_at(&dir);
        alone.reread();
        let b = bundle();

        let split = beside.split(120);
        assert!(split > 0, "the bar should fit in 120 columns");
        // THE MARK IS THE LISTING'S, so it moves with it. Left at cell zero it
        // sits under the sidebar's last letter, and clicking it marks the wrong
        // thing — which nothing else in this file would have caught.
        assert_eq!(
            cell_of(&beside.drawn(120, &b), &Hit::Mark(0)),
            Some(split + RULE)
        );
        assert_eq!(cell_of(&alone.drawn(120, &b), &Hit::Mark(0)), Some(0));
        // And the columns themselves are resolved identically — the sidebar is
        // not a column and does not degrade with them.
        assert_eq!(
            layout_beside(120, 0, 0, true),
            layout_beside(120, 0, 0, true)
        );
    }

    #[test]
    fn hiding_the_sidebar_gives_the_width_back_to_the_listing() {
        let dir = scratch("side-toggle");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
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
            "hiding the sidebar took columns from the listing"
        );
        assert!(
            with.iter().zip(&without).any(|(a, b)| b > a),
            "hiding the sidebar gave no width back at any of the {} widths",
            widths.len()
        );
    }

    #[test]
    fn a_sidebar_longer_than_the_listing_still_draws_every_row() {
        let dir = scratch("side-longer");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        std::fs::write(dir.join("dois.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 6);
        v.reread();
        let b = bundle();

        assert_eq!(v.row_count(120), Some(6));
        let rows = v.drawn_at(&[], 0, 12, 120, &b);
        assert_eq!(rows.len(), 6);
        // The rows past the listing's end carry the sidebar and NOTHING else —
        // no empty listing fields pretending to be entries.
        for (n, row) in rows.iter().enumerate().skip(2) {
            assert!(
                row.fields.iter().any(|f| f.hit == Hit::Sidebar(n)),
                "line {n} lost the bar"
            );
            assert!(
                !row.fields.iter().any(|f| matches!(f.hit, Hit::Name(_))),
                "line {n} invented an entry"
            );
        }
        // And a click inside the sidebar's cells of row five finds row five.
        let inside = rows[5].field_at(1).expect("a cell of the bar");
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
        let rows = v.drawn_at(&[], 0, 12, 120, &b);
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
        let mut v = plainly(View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        ));
        v.reread();
        let b = bundle();
        let rows = v.drawn(120, &b);

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
            "the middle line should be blank: {:?}",
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
            "an arrow should have reached the device, not the gap"
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
        let accent = |v: &mut View| -> Vec<Hit> {
            v.drawn(120, &b)
                .iter()
                .flat_map(|r| r.fields.iter())
                .filter(|f| f.color == ronoctua_tokens::color::STATE_ACCENT && !f.text.is_empty())
                .map(|f| f.hit.clone())
                .collect()
        };

        v.key(of_view("C-n"));
        let in_listing = accent(&mut v);
        v.key(of_view("C-o"));
        v.key(of_view("C-n"));
        v.key(of_view("C-n"));
        assert!(
            accent(&mut v).contains(&Hit::Sidebar(2)),
            "the accent did not go to the bar: {:?}",
            accent(&mut v)
        );
        v.key(of_view("C-o"));
        assert_eq!(
            accent(&mut v),
            in_listing,
            "going back did not land where it was"
        );
        v.key(of_view("C-o"));
        assert!(
            accent(&mut v).contains(&Hit::Sidebar(2)),
            "crossing again did not land where it was"
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
        let query = v.crumbs(120).last().expect("the path has fields").clone();
        assert_eq!(query.text, "a");
        assert_eq!(query.color, ronoctua_tokens::color::STATE_ACCENT);
        assert!(v.filtering());
    }

    #[test]
    fn esc_leaves_the_sidebar_before_it_leaves_the_view() {
        let dir = scratch("side-esc");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
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
            "the query should come out first"
        );
        assert!(
            matches!(v.key(esc), Step::Changed),
            "the bar should have gone away afterwards"
        );
        assert!(
            matches!(v.key(esc), Step::Closed),
            "only then does the view close"
        );
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
        plainly(View::at(
            dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        ))
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
        let mut v = plainly(View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        ));
        v.reread();
        let b = bundle();
        let rows = v.drawn(120, &b);
        let glyph = |n: usize| -> String {
            rows.iter()
                .flat_map(|r| r.fields.iter())
                .find(|f| f.hit == Hit::Sidebar(n) && f.text.chars().count() == 1)
                .expect("the glyph")
                .text
                .clone()
        };
        // THE STICK IS A STICK WHETHER OR NOT IT IS MOUNTED, and an internal
        // partition is an internal partition. Deriving the drawing from
        // `mounted` — which is what a `mounted.is_none()` test would do — would
        // redraw the internal one as a stick the moment somebody unmounted it,
        // and the two rows below would come out identical.
        assert_ne!(glyph(0), glyph(1));
        assert_eq!(
            glyph(1),
            glyph(2),
            "mounting does not change what kind of disk it is"
        );
    }

    #[test]
    fn an_unmounted_device_reads_differently_from_a_mounted_one() {
        let dir = scratch("side-devices");
        let mut v = view_with_devices(&dir);
        v.reread();
        let b = bundle();
        let rows = v.drawn(120, &b);

        let field = |n: usize| -> Field {
            rows.iter()
                .flat_map(|r| r.fields.iter())
                .find(|f| f.hit == Hit::Sidebar(n) && !f.text.is_empty() && f.text.len() > 4)
                .expect("the device's name")
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
        // opposite: *"pressing an item of the sidebar that is not mounted
        // should not mount it automatically; instead, show information
        // about the item, and offer the option of mounting it"*.
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
            "expected the device's sheet, got {step:?}"
        );

        // AND MOUNTING IS THERE, with its key beside it — which is the other
        // half of what was asked for.
        let b = bundle();
        let rows = v.drawn(120, &b);
        assert!(
            rows.iter()
                .flat_map(|r| r.fields.iter())
                .any(|f| f.hit == Hit::SideMenu("mount")),
            "the sheet did not offer to mount"
        );
        // The facts are on the same surface: the top line names the device.
        assert!(rows[0].fields[0].text.contains("MISO_EFI"));

        // And pressing that row is what mounts.
        match v.key(press('m')) {
            Step::Do(Doing::Mount(ref id)) => assert_eq!(id, "/block/sda2"),
            other => panic!("the `mount` row should mount, got {other:?}"),
        }
    }

    #[test]
    fn a_system_volume_offers_mounting_and_then_names_what_it_needs() {
        use ronoctua_files::sidebar::{Preferences, Row as Side, Section, Sidebar, Target};
        let dir = scratch("side-system");
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
        let mut v = plainly(View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            side,
        ));
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
                panic!("it tried to mount a system volume, which polkit refuses with nobody to ask")
            }
            other => panic!("expected the sentence offering the root terminal, got {other:?}"),
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
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 2);
        v.reread();
        // Pressed from the LISTING, where neither has a row to act on. A key
        // that did nothing and said nothing is indistinguishable from a key
        // that never arrived — the report this project has had three times,
        // with three different causes and the same screen.
        for notation in ["C-e", "C-u"] {
            assert!(
                matches!(v.key(of_view(notation)), Step::Say(_)),
                "`{notation}` stayed silent in the listing"
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
        for f in v.drawn(120, &b).iter().flat_map(|r| r.fields.iter()) {
            if matches!(f.hit, Hit::Eject(_))
                || (matches!(f.hit, Hit::Sidebar(_)) && f.text.len() > 4)
            {
                assert!(f.explain.is_some(), "{f:?} does not explain itself");
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
        assert_eq!(v.row_count(120), Some(1), "the bar should have emptied");
    }

    /// A view with BOTH panels on, laid out once at a generous width so the
    /// toggle's own answer is not the one a narrow screen gives.
    fn with_panels(v: &mut View) {
        let _ = v.panes(200);
        v.key(of_view("C-v"));
        v.key(of_view("C-i"));
        assert!(v.preview_showing() && v.info_showing());
    }

    #[test]
    fn the_chrome_is_the_listings_and_the_other_columns_start_at_the_top() {
        // THE OWNER PHOTOGRAPHED THE BLANK: *"there is a blank space above the
        // sidebar, preview and info columns, and that is caused precisely by
        // something that ought to be 100% isolated in the listing
        // column"* — the path and the header, chained in front of the whole
        // region instead of sitting on the listing's own rail.
        let dir = scratch("chrome-rail");
        for n in 0..10 {
            std::fs::write(dir.join(format!("f{n}.txt")), b"x").unwrap();
        }
        let mut v = view_beside(&dir, 8);
        v.reread();
        let width = 200u16;
        let chrome = vec![
            Row {
                fields: v.crumbs(width),
                marked: false,
                focused: false,
            },
            v.header(width, &bundle()),
        ];
        let panes = v.panes(width);
        let rows = v.drawn_at(&chrome, 0, 6, width, &bundle());

        // LINE 0 CARRIES BOTH: the listing's path AND the sidebar's first row.
        let first = &rows[0];
        assert!(
            first.fields.iter().any(|f| f.at < panes.sidebar),
            "the sidebar left its first line blank: {:?}",
            first.fields
        );
        assert!(
            first
                .fields
                .iter()
                .any(|f| f.at >= panes.listing_from() && f.text.contains('/')),
            "the path is not on the listing's first line"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scrolling_the_listing_leaves_the_other_columns_where_they_were() {
        // THE OTHER HALF OF THE SAME REPORT: *"scrolling the listing column's
        // scrollbar downwards ends up interfering with every one of the other
        // columns"*. Chained in front, the chrome was `skip`ped with the rows,
        // so the top of three columns went up with the fourth.
        let dir = scratch("scroll-isolated");
        for n in 0..20 {
            std::fs::write(dir.join(format!("f{n:02}.txt")), b"x").unwrap();
        }
        let mut v = view_beside(&dir, 8);
        v.reread();
        let width = 200u16;
        let chrome = vec![
            Row {
                fields: v.crumbs(width),
                marked: false,
                focused: false,
            },
            v.header(width, &bundle()),
        ];
        let panes = v.panes(width);
        let side_of = |rows: &[Row]| -> Vec<String> {
            rows.iter()
                .map(|r| {
                    r.fields
                        .iter()
                        .filter(|f| f.at < panes.sidebar)
                        .map(|f| f.text.as_str())
                        .collect::<String>()
                })
                .collect()
        };
        let top = v.drawn_at(&chrome, 0, 6, width, &bundle());
        let down = v.drawn_at(&chrome, 5, 6, width, &bundle());
        assert_eq!(
            side_of(&top),
            side_of(&down),
            "the sidebar moved along with the listing"
        );
        // AND THE LISTING DID MOVE, or the assertion above would pass on a
        // region that scrolls nothing at all.
        let name_of = |rows: &[Row]| -> Option<String> {
            rows.get(2).and_then(|r| {
                r.fields
                    .iter()
                    .find(|f| matches!(f.hit, Hit::Name(_)))
                    .map(|f| f.text.clone())
            })
        };
        assert_ne!(name_of(&top), name_of(&down), "the listing did not scroll");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_strip_carries_the_same_targets_the_side_panels_do() {
        // *"This column simply is not working (the mouse clicks and the
        // scrollbar do not work)"* — the strip was DRAWN and never
        // hit-tested. The rows themselves always had the targets; nothing in
        // the window door asked them, so a press went through to the listing's
        // arithmetic and came out as a row that happened to exist.
        //
        // This asserts the half that lives here: the strip's rows carry the
        // same `Hit`s the right-hand panels do. The door asking them is
        // `files_window::click`, through `strip_rows`.
        let dir = scratch("strip-targets");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_at(&dir);
        v.reread();
        v.preview_on = true;
        v.info_on = true;
        // NARROW ENOUGH that the panels cannot sit beside the listing.
        let narrow = 70u16;
        let panes = v.panes(narrow);
        assert!(
            panes.strip > 0 && panes.right == 0,
            "this width does not put the panels below: {panes:?}"
        );
        let rows = v.strip_rows(narrow, &bundle());
        let hits: Vec<&Hit> = rows
            .iter()
            .flat_map(|r| r.fields.iter())
            .map(|f| &f.hit)
            .filter(|h| !matches!(h, Hit::Nothing))
            .collect();
        assert!(
            hits.iter().any(|h| matches!(h, Hit::Bit(..))),
            "the band has no clickable permissions: {hits:?}"
        );
        assert!(
            hits.iter()
                .any(|h| matches!(h, Hit::Owner(_) | Hit::Group(_))),
            "the band has no clickable owner or group: {hits:?}"
        );
        // AND `field_at` FINDS THEM, which is what the door calls.
        // ON ITS OWN ROW, which is what the door asks: `strip_rows[k]`, where
        // `k` is the line the pointer is on.
        let (line, bit) = rows
            .iter()
            .enumerate()
            .find_map(|(k, r)| {
                r.fields
                    .iter()
                    .find(|f| matches!(f.hit, Hit::Bit(..)))
                    .map(|f| (k, f))
            })
            .expect("one cell of permission");
        let found = rows[line]
            .field_at(bit.at)
            .expect("field_at finds the cell");
        assert!(matches!(found.hit, Hit::Bit(..)), "{:?}", found.hit);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_listings_offset_reaches_no_other_column() {
        // *"YOU DID NOT ISOLATE THE COLUMNS FROM EVERYTHING ELSE, because if
        // the column were isolated, its scrollbar would not affect the result
        // of other columns"*. He is right, and this is the assertion that says
        // it: with the listing at the top and the listing scrolled, everything
        // that decides where ANOTHER column begins has to come out identical.
        let dir = scratch("offset-reaches-nothing");
        for n in 0..40 {
            std::fs::write(dir.join(format!("f{n:02}.txt")), b"x").unwrap();
        }
        let mut v = view_beside(&dir, 6);
        v.reread();
        v.preview_on = true;
        v.info_on = true;
        let w = 200u16;
        let b = bundle();
        let panes = v.panes(w);
        let chrome = vec![
            Row {
                fields: v.crumbs(w),
                marked: false,
                focused: false,
            },
            v.header(w, &b),
        ];

        // THE PICTURE'S BAND, which is what the owner photographed: its top
        // must not be a function of the listing's offset. `panel_band` no
        // longer takes one, and that is the point — there is no parameter to
        // pass an offset through any more.
        assert_eq!(
            panel_band(0, 12, panes.strip),
            panel_band(0, 12, panes.strip),
            "the panel's band changed with nothing changing"
        );

        // AND THE COLUMNS' OWN FIRST ROWS, at the top and scrolled.
        let of = |rows: &[Row], from: u16, to: u16| -> Vec<String> {
            rows.iter()
                .map(|r| {
                    r.fields
                        .iter()
                        .filter(|f| f.at >= from && f.at < to)
                        .map(|f| f.text.as_str())
                        .collect::<String>()
                })
                .collect()
        };
        let top = v.drawn_at(&chrome, 0, 8, w, &b);
        let down = v.drawn_at(&chrome, 12, 8, w, &b);
        assert_eq!(
            of(&top, 0, panes.sidebar),
            of(&down, 0, panes.sidebar),
            "the sidebar followed the listing"
        );
        let panels_from = w.saturating_sub(panes.right);
        assert_eq!(
            of(&top, panels_from, w),
            of(&down, panels_from, w),
            "the panels followed the listing"
        );
        // AND THE LISTING ITSELF DID MOVE, or none of the above means anything.
        assert_ne!(
            of(&top, panes.listing_from(), panels_from),
            of(&down, panes.listing_from(), panels_from),
            "the listing did not scroll"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_facts_bar_is_where_the_pointer_can_reach_it_and_it_moves() {
        // THE OWNER TRIED IT WITH THE MOUSE IN BOTH DOORS, and each moved in
        // one and refused in the other. The cause was the same both times: the
        // bar was DRAWN by one arithmetic and LOOKED FOR by another. Now there
        // is one rectangle — `Section::bar_box` — and the painter and the
        // pointer both read it.
        let dir = scratch("facts-drag");
        std::fs::write(dir.join("um.png"), b"x").unwrap();
        let mut v = view_beside(&dir, 1);
        v.reread();
        v.preview_on = true;
        v.info_on = true;
        let w = 53u16;
        let b = bundle();
        let panes = v.panes(w);
        assert!(panes.strip > 0, "this width does not put the panels below");
        let (cell, line_px) = (12.0f32, 26.0f32);
        let edge = f32::from(w) * cell;
        let sections = v.sections(&v.chrome_rows(w, &b), 21, w, &b);
        let info = sections
            .iter()
            .find(|s| matches!(s.pane, Pane::Info))
            .expect("the facts are a section");
        let (x, wide) =
            section_px(panes, Pane::Info, 1, w, cell, edge).expect("the facts have a box");

        // THE THUMB IS INSIDE ITS OWN COLUMN, and against its right edge —
        // never a cell past the border, which is where it went off screen.
        // A SURFACE THAT IS NOT A WHOLE NUMBER OF LINES, which is every real
        // one: the band's last line plus four tenths of another. That residue
        // is what the old arithmetic left under the thumb.
        let surface_bottom = line_px + (info.top + info.lines) as f32 * line_px + line_px * 0.4;
        let (bx, by, bw, bh) = info
            .bar_box(x + wide, line_px, surface_bottom, cell, line_px)
            .expect("the sheet does not fit in four lines: there is a bar");
        assert!(
            bx >= x && bx + bw <= x + wide + 0.5,
            "the bar ({bx}..{}) left its column ({x}..{})",
            bx + bw,
            x + wide
        );
        assert!(bx + bw <= edge, "the bar went past the surface's edge");

        // AND IT LIVES INSIDE ITS OWN BAND, never over the listing above it.
        // THE TRACK ENDS AT THE SURFACE, and not at the last whole line.
        //
        // THESE ASSERTIONS USED TO MEASURE `lines × line_px`, and that was the
        // defect written down: a surface is not a whole number of lines, so the
        // track stopped short by the residue and so did the thumb. The owner
        // photographed the gap under this very bar, twice.
        let band_top = line_px + info.top as f32 * line_px;
        assert!(by >= band_top - 0.5, "the bar rose out of the track");
        assert!(
            by + bh <= surface_bottom + 0.5,
            "the bar went past the end of the track"
        );

        // AND DRAGGING MOVES IT: the last page puts the thumb at the BOTTOM,
        // which is where the owner's stopped half way.
        let room = Room {
            list: 21,
            panel: usize::from(panes.strip),
        };
        assert!(v.drag_to(Pane::Info, 1.0, w, room), "the drag was refused");
        let down = v.sections(&v.chrome_rows(w, &b), 21, w, &b);
        let info = down
            .iter()
            .find(|s| matches!(s.pane, Pane::Info))
            .expect("the facts are still there");
        let (_, by2, _, bh2) = info
            .bar_box(x + wide, line_px, surface_bottom, cell, line_px)
            .expect("the bar is still there");
        // AND IT LANDS ON THE SURFACE'S BOTTOM. Dragged to the end, the thumb
        // ends where the window does — there is no strip of background left
        // under it, which is what the owner's photograph showed.
        assert!(
            (by2 + bh2 - surface_bottom).abs() < 1.0,
            "dragging to the end stopped at {} and the surface ends at {surface_bottom}",
            by2 + bh2
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_listings_bar_stops_where_the_strip_begins() {
        // THE OWNER DRAGGED IT TO THE END AND IT KEPT GOING: the listing's
        // thumb ran down to the bottom of the WINDOW, across the boundary and
        // into the box the facts own.
        //
        // It is the other half of the bar above. Giving every band the
        // surface's bottom fixed the residue under the facts — the facts are
        // the last band in their column — and broke the listing, which is not:
        // below it sits the strip. A band ends where ITS lines end, and only
        // the last one in a column takes the residue.
        let dir = scratch("listing-bar-strip");
        for n in 0..60 {
            std::fs::write(dir.join(format!("f{n:02}.txt")), b"x").unwrap();
        }
        let mut v = view_beside(&dir, 1);
        v.reread();
        v.preview_on = true;
        v.info_on = true;
        let w = 53u16;
        let b = bundle();
        let panes = v.panes(w);
        assert!(panes.strip > 0, "this width does not put the panels below");
        let (cell, line_px) = (12.0f32, 26.0f32);
        let edge = f32::from(w) * cell;
        let region = 21usize;
        let chrome = v.chrome_rows(w, &b);
        // THE SAME SURFACE THE FACTS' TEST USES: the region, the strip below
        // it, and a residue that is not a whole line.
        let top = line_px;
        let strip_top = top + region as f32 * line_px;
        let surface_bottom = strip_top + f32::from(panes.strip) * line_px + line_px * 0.4;

        let (x, wide) = section_px(panes, Pane::Listing, 1, w, cell, edge).expect("a listing box");
        for to in [0.0f64, 0.5, 1.0] {
            let room = Room {
                list: region,
                panel: usize::from(panes.strip),
            };
            assert!(
                v.drag_to(Pane::Listing, to, w, room),
                "the drag was refused"
            );
            let sections = v.sections(&chrome, region, w, &b);
            let listing = sections
                .iter()
                .find(|s| matches!(s.pane, Pane::Listing))
                .expect("the listing is a section");
            let (_, by, _, bh) = listing
                .bar_box(x + wide, top, surface_bottom, cell, line_px)
                .expect("sixty files do not fit: there is a bar");
            assert!(by >= top - 0.5, "at {to}: the bar rose out of the track");
            assert!(
                by + bh <= strip_top + 0.5,
                "at {to}: the bar ends at {} and the strip begins at {strip_top}",
                by + bh
            );
        }

        // AND THE LAST PAGE STILL REACHES THE BOTTOM OF ITS OWN BAND, or the
        // fix would have traded one gap for another.
        let sections = v.sections(&chrome, region, w, &b);
        let listing = sections
            .iter()
            .find(|s| matches!(s.pane, Pane::Listing))
            .expect("the listing is a section");
        let (_, by, _, bh) = listing
            .bar_box(x + wide, top, surface_bottom, cell, line_px)
            .expect("there is a bar");
        assert!(
            (by + bh - strip_top).abs() < 1.0,
            "dragged to the end the bar stops at {} and its band ends at {strip_top}",
            by + bh
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_row_of_the_strip_is_a_line_of_the_strip() {
        // THE OWNER PHOTOGRAPHED IT: the pointer on the THIRD fact, and the
        // band on the THIRD row of the listing. The row number was right; the
        // line it was turned into belonged to another column, because the
        // arithmetic added the region's top and knew nothing of the strip's.
        let dir = scratch("strip-line");
        std::fs::write(dir.join("um.png"), b"x").unwrap();
        let mut v = view_beside(&dir, 1);
        v.reread();
        v.preview_on = true;
        v.info_on = true;
        let narrow = 53u16;
        let panes = v.panes(narrow);
        assert!(panes.strip > 0, "this width does not put the panels below");
        let b = bundle();
        let room = 21usize;
        let sections = v.sections(&v.chrome_rows(narrow, &b), room, narrow, &b);

        let info = sections
            .iter()
            .find(|s| matches!(s.pane, Pane::Info))
            .expect("the facts are a section");
        // ITS THIRD ROW IS ITS THIRD LINE, counted from its OWN top.
        let line = info.line_of(2).expect("the facts' third line");
        assert_eq!(line, info.top + 2, "the facts answered for another line");
        assert!(
            line >= room,
            "the facts line ({line}) landed inside the listing's region"
        );

        // AND THE LISTING'S THIRD ROW IS SOMEWHERE ELSE ENTIRELY.
        let listing = sections
            .iter()
            .find(|s| matches!(s.pane, Pane::Listing))
            .expect("the listing is a section");
        assert_ne!(
            listing.line_of(2),
            Some(line),
            "the listing and the facts matched on the same line"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_section_answers_for_itself_and_for_nothing_else() {
        // THE FOUR DEFECTS THE OWNER REPORTED, in one place — because they were
        // one defect. The doors asked "which ROW is this `y`" and then read a
        // field off the flattened line, so a point in one section was resolved
        // against all four:
        //
        //   - clicking a folder went to the PARENT (the listing's cell resolved
        //     to the path's crumb);
        //   - the sidebar's items answered nothing;
        //   - the facts' answered nothing;
        //   - and hovering the sidebar's first two rows did nothing at all,
        //     because those lines are the LISTING's chrome and the whole line
        //     was refused.
        let dir = scratch("sections-answer");
        std::fs::create_dir(dir.join("a-folder")).unwrap();
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 6);
        v.reread();
        v.preview_on = true;
        v.info_on = true;
        let w = 200u16;
        let panes = v.panes(w);
        assert!(
            panes.sidebar > 0 && panes.right > 0,
            "this width does not have the four sections: {panes:?}"
        );
        let b = bundle();
        let room = 12usize;
        v.scroll_listing_to(0);

        // 1. THE SIDEBAR, ON THE LISTING'S CHROME LINES. Line 0 is the path's
        //    line for the listing and the sidebar's FIRST row for the sidebar.
        for line in 0..2usize {
            let t = v
                .hit(w, room, line, 1, &b)
                .unwrap_or_else(|| panic!("the sidebar did not answer on line {line}"));
            assert_eq!(t.pane, Pane::Sidebar, "line {line}: {t:?}");
            assert_eq!(t.row, line, "the sidebar answered for the wrong line");
        }

        // 2. THE LISTING'S OWN CHROME, at a listing cell on the same lines.
        let listing_cell = panes.listing_from() + 2;
        let path = v.hit(w, room, 0, listing_cell, &b).expect("the path");
        assert!(
            matches!(path.hit, Hit::Crumb(_)),
            "the path line did not answer with a piece of it: {path:?}"
        );

        // 3. A FOLDER IN THE LISTING SELECTS — it does not go to the parent.
        let first = CHROME;
        let name = v
            .hit(w, room, first, listing_cell, &b)
            .expect("the listing's first line");
        assert_eq!(name.pane, Pane::Listing);
        assert!(
            matches!(name.hit, Hit::Name(_) | Hit::Mark(_)),
            "clicking in the listing landed on {:?}",
            name.hit
        );
        assert!(
            !matches!(name.hit, Hit::Crumb(_)),
            "clicking a folder resolved to the path — which is why it went up a level"
        );

        // 4. THE FACTS ANSWER, and they answer with their own targets.
        let (_, info_at, info_room) = panes
            .boxes(w)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Info))
            .expect("the facts panel");
        let mut reached = false;
        for line in 0..6usize {
            for cell in info_at..info_at + info_room {
                if let Some(t) = v.hit(w, room, line, cell, &b)
                    && !matches!(t.hit, Hit::Nothing)
                {
                    assert_eq!(
                        t.pane,
                        Pane::Info,
                        "a target of the facts came from another section"
                    );
                    reached = true;
                }
            }
        }
        assert!(reached, "no target in the facts column");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pointer_in_a_panel_lights_the_panel_and_not_the_listing() {
        // THE OWNER'S WORDS: *"putting the cursor over an item of the info
        // column, on the exact row the mouse is on, ends up giving the hover
        // effect in the listing; that is, right now it is all one single row"*.
        let dir = scratch("hover-panel");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_at(&dir);
        v.reread();
        with_panels(&mut v);
        let width = 200u16;
        let panes = v.panes(width);
        let cell = 10.0;
        let edge = f32::from(width) * cell;
        let (info_at, _) = panes
            .boxes(width)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Info))
            .map(|(_, a, r)| (a, r))
            .expect("the information panel has a box");
        // A cell INSIDE the facts.
        let (x, wide) = band_of(panes, 1, width, Some(info_at + 1), cell, edge);
        let (px, pwide) =
            panel_px(panes, Panel::Info, 1, width, cell, edge).expect("the panel's region");
        assert!(
            (x - px).abs() < 1.0 && (wide - pwide).abs() < 1.0,
            "the panel's band came out at {x}..{} and its region is {px}..{}",
            x + wide,
            px + pwide
        );
        // AND THE LISTING'S BAND IS SOMEWHERE ELSE ENTIRELY: if the two were
        // the same rectangle the assertion above would pass on a band that
        // still lit the listing, because the listing would BE the panel.
        let (lx, lwide) = band_of(panes, 1, width, Some(panes.listing_from() + 1), cell, edge);
        assert!(
            lx + lwide <= x + 1.0,
            "the listing's band ({lx}..{}) invades the panel ({x}..)",
            lx + lwide
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wheel_alone_moves_what_the_next_frame_draws() {
        // DEFECT (c), AS AN EXECUTABLE SENTENCE.
        //
        // A wheel event, and then the very next question the door asks before
        // drawing — with nothing in between. It used to fail because the
        // listing's offset lived in the DOOR: the wheel moved the door's copy
        // and the door pushed that copy in on the following frame, so a
        // gesture with no redraw between the two showed up one frame late.
        // A touchpad sends dozens of those a second, which is why the owner
        // described it as the gesture not working rather than as lag.
        let dir = scratch("wheel-one-frame");
        for n in 0..40 {
            std::fs::write(dir.join(format!("f-{n:02}.txt")), b"x").unwrap();
        }
        let mut v = view_at(&dir);
        v.reread();
        let b = bundle();
        let width = 120u16;
        let panes = v.panes(width);

        let before = v.sections(&[], 8, width, &b);
        assert!(v.wheel(Pane::Listing, 4, width, 8), "the wheel was refused");
        let after = v.sections(&[], 8, width, &b);

        let listing = |secs: &[Section]| -> String {
            secs.iter()
                .find(|s| matches!(s.pane, Pane::Listing))
                .and_then(|s| s.content.first())
                .map(|r| first_name_text(std::slice::from_ref(r), panes))
                .unwrap_or_default()
        };
        assert_ne!(
            listing(&before),
            listing(&after),
            "the wheel moved nothing the NEXT frame would draw"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn holding_the_key_reaches_the_last_entry() {
        // THE FIFTH DEFECT, which the owner had not reported yet.
        //
        // The window's `follow` compared the cursor against the region's whole
        // height while its wheel used the height minus the chrome, so the last
        // two entries of a directory could not be reached by holding `j`. One
        // function now, and it takes the LISTING's room.
        let dir = scratch("follow-last");
        for n in 0..30 {
            std::fs::write(dir.join(format!("f-{n:02}.txt")), b"x").unwrap();
        }
        let mut v = view_at(&dir);
        v.reread();
        let room = 8usize;
        let total = v.amount();
        for _ in 0..total + 4 {
            v.key(Key {
                mods: ronoctua_keymap::Mods::default(),
                sym: Sym::Down,
            });
            v.follow(room);
        }
        assert_eq!(v.cursor(), total - 1, "the cursor did not reach the end");
        assert!(
            v.cursor() >= v.list_offset() && v.cursor() < v.list_offset() + room,
            "the last entry is outside the visible rows: cursor {} in {}..{}",
            v.cursor(),
            v.list_offset(),
            v.list_offset() + room
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_wheel_moves_the_section_it_is_over_and_nothing_else() {
        // *"Using the touchpad's scroll gestures over the info region,
        // EVERYTHING ends up scrolling (sidebar/listing/…)"* — one list with
        // three names. Each section keeps its own place now, and the LISTING's
        // stays with the door, which is what `wheel` returning `false` says.
        let dir = scratch("wheel-sections");
        for n in 0..40 {
            std::fs::write(dir.join(format!("f{n:02}.txt")), b"x").unwrap();
        }
        let mut v = view_beside(&dir, 30);
        v.reread();
        with_panels(&mut v);
        let width = 200u16;
        let panes = v.panes(width);

        // OVER THE SIDEBAR: it moves, and it is the only thing that does.
        let before = v.drawn_at(&[], 0, 6, width, &bundle());
        assert!(v.wheel(Pane::Sidebar, 3, width, 6), "the sidebar scrolled");
        let after = v.drawn_at(&[], 0, 6, width, &bundle());
        assert_ne!(
            first_side_text(&before, panes),
            first_side_text(&after, panes),
            "the sidebar did not move"
        );
        assert_eq!(
            first_name_text(&before, panes),
            first_name_text(&after, panes),
            "the listing moved along with the sidebar"
        );

        // AND THE LISTING'S GESTURE IS TAKEN HERE TOO.
        //
        // THIS ASSERTION USED TO SAY THE OPPOSITE, and it was the defect
        // written down: `!v.wheel(Pane::Listing, …)`, commented "the view
        // swallowed the listing's gesture, which belongs to the door". The
        // `false` meant "not mine" — so each door implemented the case again,
        // and each got a different half right. The owner found the halves by
        // dragging them: it worked in the window's facts and not its listing,
        // and the other way round in the footer.
        //
        // Read through the DRAWING and not through the number, because what
        // was wrong was never the number — it was which list moved.
        let before = v.drawn_at(&[], v.list_offset(), 6, width, &bundle());
        assert!(
            v.wheel(Pane::Listing, 3, width, 6),
            "the listing's own gesture is still refused"
        );
        let after = v.drawn_at(&[], v.list_offset(), 6, width, &bundle());
        assert_ne!(
            first_name_text(&before, panes),
            first_name_text(&after, panes),
            "the wheel over the listing did not move it"
        );
        v.scroll_listing_to(0);
        // A PICTURE DOES NOT SCROLL, and swallowing the gesture is the point.
        assert!(
            v.wheel(Pane::Preview, 3, width, 6),
            "the gesture over the preview leaked into another list"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The text of the first row's leftmost field — the sidebar's, when there
    /// is one.
    fn first_side_text(rows: &[Row], panes: Panes) -> String {
        rows.first().map_or_else(String::new, |r| {
            r.fields
                .iter()
                .filter(|f| f.at < panes.sidebar)
                .map(|f| f.text.as_str())
                .collect()
        })
    }

    /// The text of the first row's first field inside the LISTING.
    fn first_name_text(rows: &[Row], panes: Panes) -> String {
        rows.first()
            .and_then(|r| {
                r.fields
                    .iter()
                    .find(|f| f.at >= panes.listing_from() && matches!(f.hit, Hit::Name(_)))
            })
            .map(|f| f.text.clone())
            .unwrap_or_default()
    }

    #[test]
    fn a_click_selects_and_only_the_second_one_opens() {
        // THE OWNER ASKED FOR THIS ONE: *"if I click an item that is not
        // selected, it should only become selected; and if I click an item
        // that is already selected, then it is opened"*.
        let dir = scratch("click-twice");
        // TWO directories, because the cursor opens on the first one and
        // clicking THAT is the second click by the very rule being asserted.
        std::fs::create_dir(dir.join("first-folder")).unwrap();
        std::fs::create_dir(dir.join("second-folder")).unwrap();
        let mut v = view_at(&dir);
        v.reread();
        // A ROW THE CURSOR IS NOT ALREADY ON. It opens at zero, and clicking
        // the row it opened on is by this very rule the SECOND click.
        assert_eq!(
            v.cursor(),
            0,
            "the view opens with the cursor on the first line"
        );
        let target = v
            .drawn_at(&[], 0, 10, 200, &bundle())
            .iter()
            .flat_map(|r| r.fields.clone())
            .find_map(|f| match f.hit {
                Hit::Name(n) if f.text.contains("second-folder") => Some(n),
                _ => None,
            })
            .expect("the folder is in the listing");
        let was = v.where_().to_path_buf();

        // FIRST CLICK: the cursor moves and NOTHING opens.
        let step = v.click(target, &Hit::Name(target));
        assert!(
            matches!(step, Step::Changed),
            "the first click did more than select: {step:?}"
        );
        assert_eq!(v.cursor(), target, "the first click did not select");
        assert_eq!(
            v.where_(),
            was,
            "the first click already went into the folder"
        );

        // SECOND CLICK on the same row: now it opens.
        let _ = v.click(target, &Hit::Name(target));
        assert_ne!(
            v.where_(),
            was,
            "the second click on the already selected line did not open"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_panels_leak_nowhere_the_listing_did_not_already() {
        // The listing alone leaks at ONE cell and has since before there was a
        // sidebar — `the_listing_alone_leaks_only_where_a_name_cannot_fit`
        // pins that. What this asserts is that the two panels add nothing to
        // the list, at any width, beside any sidebar.
        let dir = scratch("panels-leak");
        for n in 0..4 {
            std::fs::write(dir.join(format!("f{n}.txt")), b"x").unwrap();
        }
        let b = bundle();
        for sidebar in [0usize, 3, 8] {
            let mut v = if sidebar == 0 {
                view_at(&dir)
            } else {
                view_beside(&dir, sidebar)
            };
            v.reread();
            with_panels(&mut v);
            let leaks: Vec<u16> = (1..=240u16)
                .filter(|w| rightmost(&v.drawn(*w, &b)) > *w)
                .collect();
            assert_eq!(
                leaks,
                Vec::<u16>::new(),
                "with {sidebar} places, it leaked at {leaks:?}"
            );
            // AND THE STRIP TOO, which is drawn from the same boxes and would
            // be the half nobody looked at.
            let strip_leaks: Vec<u16> = (1..=240u16)
                .filter(|w| rightmost(&v.strip_rows(*w, &b)) > *w)
                .collect();
            assert!(
                strip_leaks.is_empty(),
                "a faixa vazou em {strip_leaks:?} com {sidebar} lugares"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn asking_for_every_line_asks_for_what_exists() {
        // THE SIX KILLED SESSIONS.
        //
        // `View::rows` asks for `usize::MAX` and means "all of them". While
        // `take` only bounded an iterator over a finite range that was free;
        // once `sections` began MATERIALISING rows it became a request for
        // 2^64 of them, and past the end of the listing an empty `Row` was
        // pushed for ever. Twenty-seven tests grew that `Vec` at once, the
        // machine's fourteen gibibytes went in seconds, and `systemd-oomd`
        // took the terminal's whole cgroup — the agent with it, six times,
        // leaving nothing in the session to read. The owner found it from the
        // journal and wrote it up; this is the assertion that keeps it found.
        //
        // The rule in one line: `take` is AT MOST, never EXACTLY.
        let dir = scratch("sentinel");
        for n in 0..3 {
            std::fs::write(dir.join(format!("f{n}.txt")), b"x").unwrap();
        }
        let mut v = view_beside(&dir, 4);
        v.reread();
        let b = bundle();
        let all = v.drawn(200, &b);
        assert!(
            all.len() <= 64,
            "asking for every line materialised {} — the sentinel became a count",
            all.len()
        );
        // AND IT IS THE SAME ANSWER the region's height gives, which is the
        // other half: a count that disagrees with the drawing is blank space
        // nobody can explain.
        assert_eq!(
            all.len(),
            v.row_count(200).expect("a listing counts"),
            "the lines drawn are not the ones counted"
        );
        // AND ASKING FOR MORE THAN EXISTS ANSWERS WITH WHAT EXISTS.
        assert_eq!(v.drawn_at(&[], 0, 10_000, 200, &b).len(), all.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_count_is_exactly_what_the_rows_produce() {
        // A DISAGREEMENT HERE IS THE REGION RESERVING LINES NOTHING DRAWS INTO
        // — blank space below the listing, which reads as the manager having
        // lost the rest of it. It is the same rule the sidebar's `.max(1)`
        // already had to obey, with a third term in the maximum.
        let b = bundle();
        for how_many in [0usize, 1, 3, 20] {
            let dir = scratch(&format!("count-{how_many}"));
            for n in 0..how_many {
                std::fs::write(dir.join(format!("f{n}.txt")), b"x").unwrap();
            }
            for sidebar in [0usize, 6] {
                let mut v = if sidebar == 0 {
                    view_at(&dir)
                } else {
                    view_beside(&dir, sidebar)
                };
                v.reread();
                with_panels(&mut v);
                for w in [40u16, 80, 96, 140, 200] {
                    let count = v.row_count(w).expect("a listing counts");
                    let rows = v.drawn_at(&[], 0, usize::MAX, w, &b).len();
                    assert_eq!(
                        count, rows,
                        "{how_many} files, {sidebar} places, {w} columns"
                    );
                }
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn the_panels_are_never_one_on_the_right_and_one_at_the_bottom() {
        // The owner's sentence is binary — beside, or underneath — and a mixed
        // mode would make the listing shorter AND narrower at once.
        let dir = scratch("panels-flip");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 4);
        v.reread();
        with_panels(&mut v);
        let mut saw_right = false;
        let mut saw_strip = false;
        for w in 1..=240u16 {
            let p = v.panes(w);
            assert!(
                p.right == 0 || p.strip == 0,
                "at {w} columns both places at once: {p:?}"
            );
            saw_right |= p.right > 0;
            saw_strip |= p.strip > 0;
        }
        assert!(saw_right, "it never fitted on the right");
        assert!(saw_strip, "nunca desceu para a faixa");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_header_never_names_a_column_under_a_panel() {
        // The header used to be laid out without the sidebar and named columns
        // that were somewhere else; this is the same defect at the other edge,
        // and clicking one of those names would sort by whatever happened to be
        // under the word.
        let dir = scratch("panels-header");
        for n in 0..3 {
            std::fs::write(dir.join(format!("f{n}.txt")), b"x").unwrap();
        }
        let b = bundle();
        let mut v = view_beside(&dir, 3);
        v.reread();
        with_panels(&mut v);
        for w in 20..=240u16 {
            let panes = v.panes(w);
            if panes.right == 0 {
                continue;
            }
            let from = w.saturating_sub(panes.right);
            for f in &v.header(w, &b).fields {
                assert!(
                    f.at + width_of(&f.text) <= from,
                    "at {w} columns the header invades the panel: {f:?}"
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cell_in_the_info_panel_is_a_fact_and_one_left_of_the_rule_is_the_listing() {
        // `Row::field_at` walks from the RIGHT, which is why the sidebar needed
        // no boundary arithmetic when it was prepended. The panels are appended
        // for the same reason, and this is the assertion that says so: one cell
        // either side of the same rule answers with two different regions.
        let dir = scratch("panels-hit");
        std::fs::write(dir.join("relatorio.pdf"), b"x").unwrap();
        let b = bundle();
        let mut v = view_at(&dir);
        v.reread();
        with_panels(&mut v);
        let w = 200u16;
        let panes = v.panes(w);
        assert!(
            panes.right > 0,
            "at 200 columns the panels sit on the right"
        );
        let info = panes
            .boxes(w)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Info))
            .expect("the information panel has a box");
        let rows = v.drawn(w, &b);
        let row = rows.first().expect("a row");
        // INSIDE THE INFO PANEL: a field of the panel, and never of the table.
        let there = row.field_at(info.1).expect("something in the box");
        assert!(
            there.at >= info.1,
            "cell {} found a field at {}",
            info.1,
            there.at
        );
        assert_eq!(there.hit, Hit::Nothing, "a fact is not an action");
        // AND ONE CELL LEFT OF THE FIRST RULE: the listing, still.
        let from = w.saturating_sub(panes.right);
        let last = row
            .fields
            .iter()
            .filter(|f| f.at < from)
            .map(|f| f.at)
            .max()
            .expect("the listing drew something");
        let here = row.field_at(last).expect("the listing's field");
        assert!(here.at < from, "cell {last} landed in the panel");
    }

    #[test]
    fn the_information_key_is_not_the_tab_key() {
        // In a terminal `C-i` and `TAB` are the same byte and somebody will
        // assume they are the same key here. They are not: `key_of` maps
        // `Keysym::Tab` and `Keysym::i` separately, and the day they collided
        // one of the two would silently stop working.
        let tab = of_view("TAB");
        let ci = of_view("C-i");
        assert_ne!(tab, ci, "TAB and C-i became the same key");
        assert_eq!(of_the_view(tab), Some("files-pane-next"));
        assert_eq!(of_the_view(ci), Some("files-facts"));
        assert_eq!(of_the_view(of_view("C-v")), Some("files-preview"));
        // AND `S-TAB` IS A THIRD KEY, which is the half that is new: it reaches
        // the view only because both doors read `ISO_Left_Tab` off the keysym
        // and set the flag from THERE. Sharing `TAB`'s notation would have made
        // Shift+Tab walk forwards, in silence.
        let back = of_view("S-TAB");
        assert_ne!(back, tab, "S-TAB and TAB became the same key");
        assert_eq!(of_the_view(back), Some("files-pane-previous"));
        // AND MEASURING KEPT A KEY. It moved off `TAB` — the notification
        // manager's own key line has said `TAB pane` since it had two columns —
        // and a gesture the mouse reaches (`Hit::Measure`) with no key at all
        // is the one direction `files-everything-has-a-key` rules out.
        assert_eq!(of_the_view(of_view("C-t")), Some("files-measure"));
    }

    #[test]
    fn a_panel_that_cannot_appear_flips_anyway_and_says_why() {
        // A key that does nothing and says nothing is the one outcome this
        // project treats as a defect: from the outside, a refusal and a dead
        // key look the same. The shape is `files-sidebar-only`'s.
        let dir = scratch("panels-narrow");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_at(&dir);
        v.reread();
        // A SCREEN TOO NARROW FOR EVEN ONE PANEL.
        let _ = v.panes(20);
        let step = v.key(of_view("C-v"));
        assert!(v.preview_showing(), "the state has to turn anyway");
        assert!(
            matches!(step, Step::Say(_)),
            "a recusa tem de dizer alguma coisa: {step:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_hint_names_the_two_panel_keys() {
        // `C-o` was implemented, worked, and nothing on screen said it existed
        // — "which is the same as not having it". These two are in no registry
        // and no manual either, so this line is all there is.
        let b = bundle();
        for sidebar in [false, true] {
            let hint = hint_of(&b, sidebar);
            for id in ["files-preview", "files-facts"] {
                let t = key_that_runs(id).expect("the key is in the table");
                assert!(
                    hint.contains(&t.notation()),
                    "the hint does not name `{}` ({id}): {hint}",
                    t.notation()
                );
            }
        }
    }

    #[test]
    fn the_panels_are_on_the_moment_the_manager_opens() {
        // THE OWNER'S DECISION, and it is a default and not a preference: the
        // panels are what the manager is for, and a feature that has to be
        // switched on every time is a feature most of the time nobody has.
        //
        // Through `View::at` and NOT through `view_at`, which is the fixture
        // the listing's own tests use and which turns them off. A test that
        // came through the fixture would be asserting the fixture.
        let dir = scratch("panels-default");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let v = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            no_sidebar(),
        );
        // ON THE SCREEN and not only in the state: at a width that holds them,
        // both boxes have to be there. `panes` is what every painter and every
        // hit test goes through.
        let panes = v.panes(200);
        assert!(panes.right > 0, "the panels did not appear: {panes:?}");
        assert_eq!(
            panes.boxes(200).len(),
            2,
            "both boxes have to be there: {panes:?}"
        );
        // AND THE KEYS STILL HIDE THEM — the owner asked for them to be on,
        // not for the keys to go.
        let mut v = v;
        v.key(of_view("C-v"));
        v.key(of_view("C-i"));
        assert_eq!(v.panes(200).boxes(200).len(), 0, "the keys do not hide");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_query_that_matches_nothing_gives_the_lines_back() {
        // WHAT `leader-vm` CAUGHT, and it is the reason this test exists at
        // this level: typing narrows the list, and the region has to narrow
        // with it. With the panels on by default it stopped doing that — the
        // preview asked for its twelve lines whether or not there was a file
        // to draw in them, so `zzz` left the footer 288 px tall before and
        // 288 px after, and the person typing got no answer.
        let dir = scratch("query-nothing");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            no_sidebar(),
        );
        // THROUGH `row_count`, which is what the region's height is made of:
        // `Surface::expansion_len` asks it and nothing else. Asserting on
        // `rows_from` would count the rows drawn and miss the lines ASKED FOR,
        // which is the half that was wrong.
        let with_a_file = v.row_count(200).expect("a listing counts");
        for letter in "zzz".chars() {
            v.key(of_view(&letter.to_string()));
        }
        let with_none = v.row_count(200).expect("a listing counts");
        assert!(
            with_none < with_a_file,
            "the region did not shrink: {with_a_file} lines before, {with_none} after"
        );
        // AND THE PANELS ASK FOR NOTHING, which is the half a comparison would
        // miss: with no sidebar and no match there is one line left — the
        // listing's own "nothing here" — and not the picture's box holding
        // four open around an empty frame.
        assert_eq!(
            with_none, 1,
            "the panels held lines with nothing to say: {with_none}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_three_columns_one_looks_up_start_hidden_and_a_key_shows_them() {
        // The other half of the same decision: what the listing shows without
        // being asked is what one navigates BY, and the permission triplet and
        // the two names are what one looks UP.
        let dir = scratch("columns-default");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let b = bundle();
        let mut v = View::at(
            &dir,
            Roster::default(),
            Zone::utc(),
            None,
            std::sync::Arc::new(Vec::new()),
            std::rc::Rc::new(bundle()),
            no_sidebar(),
        );
        v.key(of_view("C-v"));
        v.key(of_view("C-i"));
        let named = |v: &View| -> Vec<Column> {
            v.header(120, &b)
                .fields
                .iter()
                .filter_map(|f| match f.hit {
                    Hit::Sort(c) => Some(c),
                    _ => None,
                })
                .collect()
        };
        let hidden = named(&v);
        for c in [Column::Perms, Column::Owner, Column::Group] {
            assert!(
                !hidden.contains(&c),
                "{c:?} apareceu sem ninguem pedir: {hidden:?}"
            );
        }
        // AND THE KEY BRINGS THEM BACK — with their targets, which is what
        // makes them worth showing.
        assert!(matches!(v.key(of_view("C-d")), Step::Changed));
        let shown = named(&v);
        for c in [Column::Perms, Column::Owner, Column::Group] {
            assert!(shown.contains(&c), "{c:?} did not come back: {shown:?}");
        }
        let row = v.drawn(120, &b).remove(0);
        assert!(
            row.fields
                .iter()
                .any(|f| matches!(f.hit, Hit::Bit(_, _, _))),
            "the letters came back with no target"
        );
        // AND THE HINT LINE NAMES THE KEY. It is in no registry and no manual:
        // `C-o` already cost this file the lesson.
        for sidebar in [false, true] {
            let hint = hint_of(&b, sidebar);
            let t = key_that_runs("files-columns").expect("the key is in the table");
            assert!(
                hint.contains(&t.notation()),
                "the hint does not name the key: {hint}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn there_is_no_glyph_where_there_is_no_picture_and_a_sentence_instead() {
        // THE OWNER'S WORDS: "when there is no preview, instead of showing an
        // icon it should say, in a very discreet formatting, that there is no
        // preview". The glyph said what the listing's own icon column already
        // said, at the size of a picture, so an empty panel looked full.
        let dir = scratch("no-preview");
        std::fs::create_dir(dir.join("folder")).unwrap();
        let b = bundle();
        let mut v = view_at(&dir);
        v.reread();
        with_panels(&mut v);
        let w = 200u16;
        let (at, room) = v.panes(w).preview_box(w).expect("the preview has a box");
        let rows = v.drawn(w, &b);
        let inside: Vec<&Field> = rows
            .iter()
            .flat_map(|r| r.fields.iter())
            .filter(|f| f.at >= at && f.at < at + room)
            .collect();
        assert_eq!(
            inside.len(),
            1,
            "the panel should have ONE line: {inside:?}"
        );
        let said = &inside[0];
        assert_eq!(
            said.text,
            b.t(NO_PREVIEW),
            "the panel does not say there is no preview"
        );
        assert_eq!(
            said.color,
            ronoctua_tokens::color::TEXT_DIM,
            "the formatting has to be the most discreet there is"
        );
        // AND NOTHING IS A GLYPH OF THE PRIVATE USE AREA any more — which is
        // where every icon of this system lives, and the range is what
        // `layout::is_chrome_glyph` already answers with.
        assert!(
            !said
                .text
                .chars()
                .any(ronoctua_render::layout::is_chrome_glyph),
            "there is still an icon in the panel: {}",
            said.text
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_picture_sits_between_the_rules_and_not_inside_the_margin() {
        // THE OWNER MEASURED THIS ONE OFF THE SCREEN: 60 px of air on the
        // picture's left against 54 on its right, in a box the code believed
        // was centred. The box was the TEXT's — one cell in from the window's
        // edge — while the region it looked like was the window's edge to the
        // rule. Half a cell at each end, which is exactly the difference.
        //
        // In the strip the preview is the first panel and has no rule to its
        // left, so its region starts at ZERO. That is the whole assertion.
        let panes = Panes {
            sidebar: 0,
            right: 0,
            strip: ronoctua_tokens::space::FILES_STRIPLINES,
            preview: true,
            info: true,
        };
        let cell = 12.0;
        let (x, wide) = panel_px(panes, Panel::Preview, 1, 60, cell, 60.0 * cell)
            .expect("the preview has a region");
        assert!(
            x.abs() < f32::EPSILON,
            "the preview started inside the text margin, at {x} px"
        );
        // AND IT ENDS AT THE RULE, never half a cell short of it.
        let (info_at, _) = panes
            .boxes(60)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Info))
            .map(|(_, a, r)| (a, r))
            .expect("the information panel has a box");
        let rule = rule_x(info_at.saturating_sub(RULE), 1, cell);
        assert!(
            (x + wide - rule).abs() < 1.0,
            "the preview ends at {} and the rule is at {rule}",
            x + wide
        );
    }

    #[test]
    fn the_picture_gets_the_same_air_above_it_as_below() {
        // THE PANELS START WHERE EVERY SECTION STARTS: the region's first
        // line. The band used to subtract the LISTING's chrome, and the doors
        // computed that subtraction as `2 - offset` — so the picture's box
        // jumped two lines up the moment the listing was scrolled. The owner
        // saw it as extra air at the top of the preview that "disappears only
        // when you scroll the file list".
        let (top, lines) = panel_band(3, 12, 0);
        assert_eq!(
            (top, lines),
            (3, 12),
            "the band does not start at the top of the region"
        );
        // AND WITH THE STRIP the panels are UNDER the listing, not in it.
        assert_eq!(panel_band(3, 12, 4), (15, 4));
        // WHAT WOULD BE DRAWN: a landscape picture in that band has the same
        // air above it as below. `Area::fit_into` centres, and the box is what
        // it centres in — which is the half that was wrong.
        let area = ronoctua_render::image::Area {
            x: 0.0,
            y: f32::from(top),
            width: 40.0,
            height: f32::from(lines),
        }
        .fit_into(480, 320);
        let above = area.y - f32::from(top);
        let below = (f32::from(top) + f32::from(lines)) - (area.y + area.height);
        assert!(
            (above - below).abs() < 0.001,
            "{above} above and {below} below"
        );
    }

    #[test]
    fn the_info_column_says_when_it_has_been_cut() {
        // The panel no longer stretches the region to fit its facts — a
        // photograph has a dozen and the footer's whole ceiling is twelve
        // lines — so it has to say when it has been cut. A list cut with
        // nothing saying so is this file's oldest complaint.
        let dir = scratch("facts-bar");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let b = bundle();
        let mut v = view_at(&dir);
        v.reread();
        with_panels(&mut v);
        let w = 200u16;
        let facts = v.sheet().len();
        assert!(facts > 4, "the test needs more facts than lines");
        let (cell, line_px) = (10.0f32, 26.0f32);
        let edge = f32::from(w) * cell;
        let panes = v.panes(w);
        let (x, wide_px) =
            section_px(panes, Pane::Info, 1, w, cell, edge).expect("the facts have a box");
        let (_, at, wide) = panes
            .boxes(w)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Info))
            .expect("the information panel has a box");

        // WITH ROOM FOR EVERY FACT there is no bar: chrome about nothing is
        // what the listing's own bar refuses to be.
        let roomy = v.sections(&[], facts, w, &b);
        let roomy = roomy
            .iter()
            .find(|s| matches!(s.pane, Pane::Info))
            .expect("the facts are a section");
        assert_eq!(
            roomy.bar_box(x + wide_px, 0.0, line_px * 12.0, cell, line_px),
            None,
            "a bar where everything fits"
        );

        // WITH FOUR LINES there is one, against the column's own right edge.
        let cut = v.sections(&[], 4, w, &b);
        let cut = cut
            .iter()
            .find(|s| matches!(s.pane, Pane::Info))
            .expect("the facts are a section");
        let (bx, _, bw, _) = cut
            .bar_box(x + wide_px, 0.0, line_px * 12.0, cell, line_px)
            .expect("the bar is missing");
        assert!(bx >= x, "the bar left the panel on the left");
        assert!(
            (bx + bw - (x + wide_px)).abs() < 0.5,
            "the bar does not reach the column's edge"
        );
        assert!(bx + bw <= edge, "the bar went past the surface's edge");
        // AND NO VALUE REACHES IT: the bar's cell is the column's last, and
        // `info_fields` keeps it clear.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let bar_cell = ((bx / cell) as u16).saturating_sub(1);
        let rows = v.drawn(w, &b);
        for f in rows.iter().flat_map(|r| r.fields.iter()) {
            if f.at < at || f.at >= at + wide {
                continue;
            }
            assert!(
                f.at + width_of(&f.text) <= bar_cell + 1,
                "the `{}` field invades the bar's cell",
                f.text
            );
        }

        // AND THE KEY MOVES IT, and comes round to the start.
        // ASKED AGAIN, because `drawn()` above wanted every line and that is
        // what the view now remembers as the panel's height. `C-.` pages what
        // was last drawn.
        let cut = v.sections(&[], 4, w, &b);
        let cut = cut
            .iter()
            .find(|s| matches!(s.pane, Pane::Info))
            .expect("the facts are a section");
        let top_y = cut
            .bar_box(x + wide_px, 0.0, line_px * 12.0, cell, line_px)
            .expect("the bar")
            .1;
        assert!(matches!(v.key(of_view("C-.")), Step::Changed));
        let after = v.sections(&[], 4, w, &b);
        let after = after
            .iter()
            .find(|s| matches!(s.pane, Pane::Info))
            .expect("os fatos continuam la");
        let moved_y = after
            .bar_box(x + wide_px, 0.0, line_px * 12.0, cell, line_px)
            .expect("the bar is still there")
            .1;
        assert!(moved_y > top_y, "the key did not move the thumb");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_panels_stay_put_while_the_listing_scrolls() {
        // THE DEFECT: the panels' fields ride the listing's rows, and they
        // were indexed by the ABSOLUTE row. So the preview's sentence and the
        // first fact belonged to line zero of the DIRECTORY — scroll to row
        // twenty and both panels went blank, because sheet index twenty does
        // not exist. Nothing said so, because a blank panel says nothing.
        //
        // It only mattered a little while the panels were off by default. Now
        // that they are on, it is what a long directory looks like.
        let dir = scratch("panels-scroll");
        for n in 0..40 {
            std::fs::write(dir.join(format!("f{n:02}.txt")), b"x").unwrap();
        }
        let b = bundle();
        let mut v = view_at(&dir);
        v.reread();
        with_panels(&mut v);
        let w = 200u16;
        let (at, room) = v.panes(w).preview_box(w).expect("the preview has a box");
        let said = |rows: &[Row]| -> Option<String> {
            rows.iter()
                .flat_map(|r| r.fields.iter())
                .find(|f| f.at >= at && f.at < at + room)
                .map(|f| f.text.clone())
        };
        let top = said(&v.drawn_at(&[], 0, 8, w, &b));
        assert!(top.is_some(), "the panel drew nothing at the top");
        assert_eq!(
            said(&v.drawn_at(&[], 20, 8, w, &b)),
            top,
            "the panel emptied when the listing scrolled"
        );
        // AND WHAT THE POINTER FINDS IS WHAT WAS DRAWN. The hit test used to
        // rebuild the row from the top of the directory, so with the region
        // scrolled it answered with a fact that was somewhere else — or with
        // none at all.
        let (at, room) = v
            .panes(w)
            .boxes(w)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Info))
            .map(|(_, a, r)| (a, r))
            .expect("the information panel has a box");
        let cell = at + room / 2;
        // THROUGH `hit`, which is what both doors ask now. The panel's line is
        // the line ON SCREEN whatever the listing's offset is, so the region's
        // first line answers with the same fact either way.
        v.scroll_listing_to(20);
        let scrolled = v.hit(w, 12, CHROME, cell, &b).map(|t| t.explain);
        v.scroll_listing_to(0);
        let top = v.hit(w, 12, CHROME, cell, &b).map(|t| t.explain);
        assert_eq!(
            scrolled, top,
            "the panel's first line really changed on scroll"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fact_in_the_info_panel_reaches_the_same_target_the_listing_does() {
        // THE OWNER'S REPORT: the permission letters are clickable in the
        // table and dead in the panel that exists to show them. Every target
        // here is one the listing already produces — the same `Hit`, the same
        // question — because a second machinery is how the two come to
        // disagree about what clicking a `w` does.
        let dir = scratch("facts-hit");
        let file = dir.join("one.txt");
        std::fs::write(&file, b"x").unwrap();
        let b = bundle();
        let mut v = view_at(&dir);
        v.reread();
        with_panels(&mut v);
        let w = 200u16;
        let (at, room) = v
            .panes(w)
            .boxes(w)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Info))
            .map(|(_, a, r)| (a, r))
            .expect("the information panel has a box");
        let rows = v.drawn(w, &b);
        let inside: Vec<&Field> = rows
            .iter()
            .flat_map(|r| r.fields.iter())
            .filter(|f| f.at >= at && f.at < at + room)
            .collect();
        // THE PERMISSION TRIPLET IS TEN TARGETS, exactly as in the table.
        let bits = inside
            .iter()
            .filter(|f| matches!(f.hit, Hit::Bit(_, _, _) | Hit::Special(_, _)))
            .count();
        assert_eq!(bits, 9, "the nine letters have to be targets: {inside:?}");
        // AND THE OWNER AND THE GROUP ARE ONE EACH.
        assert!(
            inside.iter().any(|f| matches!(f.hit, Hit::Owner(_))),
            "the owner is not a target"
        );
        assert!(
            inside.iter().any(|f| matches!(f.hit, Hit::Group(_))),
            "the group is not a target"
        );
        // WHAT CANNOT BE OFFERED IS NOT DRAWN AS IF IT COULD: a file's size
        // and a date are read, never edited.
        assert!(
            !inside.iter().any(|f| matches!(f.hit, Hit::Measure(_))),
            "the size of a FILE is not a walk"
        );
        // AND EVERY ONE OF THEM SAYS SOMETHING, which is what the listing's
        // own `every_field_that_lights_up_says_something` asks of a target.
        for f in &inside {
            if matches!(f.hit, Hit::Nothing) {
                continue;
            }
            assert!(f.explain.is_some(), "o campo `{}` acende e cala", f.text);
        }
        // AND PRESSING ONE REACHES THE SAME QUESTION THE TABLE REACHES.
        let bit = inside
            .iter()
            .find(|f| matches!(f.hit, Hit::Bit(_, _, _)))
            .expect("a letter");
        let hit = bit.hit.clone();
        assert!(
            matches!(v.click(0, &hit), Step::Changed),
            "clicking the panel's bit did not open the question"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directorys_size_in_the_panel_is_the_walk_and_a_files_is_not() {
        // `Doable` is the whole of this: what a row offers is decided beside
        // the value it is about, and a row that lit up and then refused would
        // be worse than one that never promised.
        let dir = scratch("facts-measure");
        std::fs::create_dir(dir.join("folder")).unwrap();
        let b = bundle();
        let mut v = view_at(&dir);
        v.reread();
        with_panels(&mut v);
        let w = 200u16;
        let (at, room) = v
            .panes(w)
            .boxes(w)
            .into_iter()
            .find(|(p, _, _)| matches!(p, Panel::Info))
            .map(|(_, a, r)| (a, r))
            .expect("the information panel has a box");
        let rows = v.drawn(w, &b);
        let measure = rows
            .iter()
            .flat_map(|r| r.fields.iter())
            .filter(|f| f.at >= at && f.at < at + room)
            .find(|f| matches!(f.hit, Hit::Measure(_)));
        assert!(measure.is_some(), "the size of a FOLDER is a walk");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_path_never_crosses_the_sidebars_rule() {
        // THE DEFECT THE OWNER PHOTOGRAPHED, in
        // `10-manager-with-preview.png`: `/root/shots/` runs to `x≈170`
        // and the rule between the two lists is at `x=151`. Everything else on
        // that row already respects the division; the path was drawn from cell
        // zero and measured against nothing.
        let dir = scratch("path-rule");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let b = bundle();
        let mut v = view_beside(&dir, 6);
        v.reread();
        with_panels(&mut v);
        for w in 1..=240u16 {
            let panes = v.panes(w);
            let from = panes.listing_from();
            let ends = w.saturating_sub(panes.right);
            for f in v.crumbs(w) {
                assert!(
                    f.at >= from,
                    "at {w} columns the path starts at {} and the bar runs to {from}",
                    f.at
                );
                assert!(
                    f.at + width_of(&f.text) <= ends,
                    "at {w} columns the path goes past {ends}: {f:?}"
                );
            }
        }
        // AND IT STARTS WHERE THE HEADER DOES, which is the whole of "respects
        // the split": both are the LISTING's, and the sidebar's column is
        // somebody else's.
        let w = 120u16;
        let panes = v.panes(w);
        assert!(panes.sidebar > 0, "the test needs the sidebar");
        let head = v.header(w, &b).fields.first().map(|f| f.at);
        let path = v.crumbs(w).first().map(|f| f.at);
        assert_eq!(
            path,
            Some(panes.listing_from()),
            "the path does not respect the split"
        );
        assert!(head.is_some_and(|h| h >= panes.listing_from()));
        // AND WHAT IS BEING TYPED SURVIVES THE CUT: the query is at the end of
        // the path and it is what the hand is doing now.
        for c in "picture".chars() {
            v.key(press(c));
        }
        let crumbs = v.crumbs(w);
        let last = crumbs.last().expect("the path has fields");
        assert_eq!(
            last.color,
            ronoctua_tokens::color::STATE_ACCENT,
            "the last field should be the query: {crumbs:?}"
        );
        assert!(!last.text.is_empty(), "a consulta desapareceu");
        let _ = std::fs::remove_dir_all(&dir);
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
        std::fs::create_dir(root.join("folder")).unwrap();
        for (notation, label) in OF_THE_VIEW {
            let mut v = view_at(&root);
            // THE CURSOR IN THE MIDDLE. At the top, `k` moves nowhere and
            // answers `Nothing` — correctly — and the test would read that as
            // "the key is not handled". The question here is whether the table
            // and the `match` agree, not whether a clamp clamps.
            v.cursor = 1;
            let t = ronoctua_keymap::Key::parse(notation)
                .unwrap_or_else(|_| panic!("`{notation}` is not a valid key"));
            let step = v.key(t);
            assert!(
                !matches!(step, Step::Nothing),
                "the key `{notation}` (`{label}`) does nothing"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    // ------------------------------------------------------------------
    // THE FIVE THINGS THE OWNER MEASURED OFF ONE SCREENSHOT
    // ------------------------------------------------------------------

    #[test]
    fn a_rule_gives_half_its_own_cell_to_each_side() {
        // `RULE` reserves a whole cell of grid for a line one pixel wide, and
        // every site drew that line at the cell's LEFT edge — so the cell was
        // air on the listing's side and nothing at all on the sidebar's.
        // Measured off the owner's screen at 1280 px: 10,8 px before the line
        // and 21,6 px after it, for one boundary.
        let cell = 10.8_f32;
        let (at, lead) = (13u16, 1u16);
        let x = rule_x(at, lead, cell);
        let left = f32::from(lead + at) * cell;
        let right = f32::from(lead + at + 1) * cell;
        assert!(
            ((x - left) - (right - x)).abs() < 0.001,
            "the line is not in the middle of its own cell: {x} in [{left}, {right}]"
        );
    }

    #[test]
    fn the_band_reaches_the_rule_beside_it() {
        // THE DEFECT THE OWNER PHOTOGRAPHED, in his own words: *"the item's
        // selection background does not touch the vertical separating line on
        // the listing's left-hand side"*. The footer began the listing's band at
        // `lead + split + 1` — one whole CELL past the rule — so a lit row had
        // a strip of unlit background between it and the line beside it.
        let cell = 10.8_f32;
        let (lead, width, edge) = (1u16, 118u16, 1280.0_f32);
        let panes = Panes {
            sidebar: 13,
            right: 50,
            strip: 0,
            preview: true,
            info: true,
        };
        let line = rule_x(panes.sidebar, lead, cell);

        // OVER THE SIDEBAR: the screen's own edge to the rule. The one arm of
        // the four the owner called right, and it stays as it is.
        let (at, wide) = band_of(panes, lead, width, Some(3), cell, edge);
        assert!(
            (at - 0.0).abs() < f32::EPSILON,
            "the bar's track does not start at the edge"
        );
        assert!(
            (at + wide - line).abs() < 0.001,
            "the bar's track stops at {} and the rule is at {line}",
            at + wide
        );

        // OVER THE LISTING: the same rule to the panels' first one. The two
        // bands meet ON the line, which is what a rule painted afterwards
        // covers.
        let (at, wide) = band_of(panes, lead, width, Some(40), cell, edge);
        assert!(
            (at - line).abs() < 0.001,
            "the listing's band starts at {at}, not at the rule {line}"
        );
        let panels = rule_x(width - panes.right, lead, cell);
        assert!(
            (at + wide - panels).abs() < 0.001,
            "the listing's band runs under the panels: it stops at {}, their rule is at {panels}",
            at + wide
        );

        // AND WITH NOTHING BESIDE IT, edge to edge — never `lead + width`,
        // which is past the border by the door's own lead.
        let (at, wide) = band_of(Panes::default(), lead, width, None, cell, edge);
        assert!((at - 0.0).abs() < f32::EPSILON && (wide - edge).abs() < f32::EPSILON);
    }

    #[test]
    fn the_picture_never_touches_the_box_it_is_in() {
        use ronoctua_render::image::Area;
        // `fit_into` centres, and centring was never the whole promise:
        // whichever dimension is the tight one comes out exactly as long as
        // the box. Photographed in both doors — against the top edge in the
        // window, against the right-hand rule in the footer.
        let air = f32::from(ronoctua_tokens::space::FILES_PICTUREAIR);
        let space = Area {
            x: 100.0,
            y: 50.0,
            width: 260.0,
            height: 106.0,
        };
        let sides = |got: &Area| {
            (
                got.x - space.x,
                (space.x + space.width) - (got.x + got.width),
                got.y - space.y,
                (space.y + space.height) - (got.y + got.height),
            )
        };

        // A PHOTOGRAPH WIDER THAN ITS BOX: the width is the tight dimension,
        // which is the footer's case.
        let got = picture_in(space, 4000, 1000);
        let (l, r, t, b) = sides(&got);
        assert!(l >= air && r >= air, "no air on the sides: {l} and {r}");
        assert!((l - r).abs() < 0.001, "the left {l} is not the right {r}");
        assert!((t - b).abs() < 0.001, "the top {t} is not the bottom {b}");

        // AND A TALL ONE, which is the window's case.
        let got = picture_in(space, 1000, 4000);
        let (l, r, t, b) = sides(&got);
        assert!(t >= air && b >= air, "no air above and below: {t} and {b}");
        assert!((l - r).abs() < 0.001 && (t - b).abs() < 0.001);

        // AND A BOX SMALLER THAN THE AIR TWICE OVER gives back nothing, and
        // never a negative picture.
        let got = picture_in(
            Area {
                x: 0.0,
                y: 0.0,
                width: 4.0,
                height: 4.0,
            },
            100,
            100,
        );
        assert!(
            got.width >= 0.0 && got.height >= 0.0,
            "an image of negative width"
        );
    }

    #[test]
    fn the_icon_gets_the_one_cell_of_air_every_other_column_gets() {
        // *"The icon spacing is far too wide in the listing."* Measured
        // off the owner's screen: the icon's ink ended at x=183 and the name
        // began at x=208 — 25 px, in a grid whose cell is 10,8. The column
        // asked for two cells and `layout_beside` added the gap on top.
        let l = layout_beside(120, 0, 0, true);
        let (icon_at, icon_wide) = l.find(Column::Icon).expect("the icon");
        let (name_at, _) = l.find(Column::Name).expect("the name");
        assert_eq!(
            icon_wide, 1,
            "the icon asked for more than one cell of drawing"
        );
        assert_eq!(
            name_at - icon_at,
            2,
            "the icon and the name are not one cell of air apart"
        );
        // AND IT IS THE SAME DISTANCE AS BETWEEN ANY OTHER PAIR, which is what
        // "proportional" means here: one gap, one number, every column.
        let (size_at, size_wide) = l.find(Column::Size).expect("the size");
        let (when_at, _) = l.find(Column::Modified).expect("a data");
        assert_eq!(when_at - (size_at + size_wide), 1);
    }

    #[test]
    fn the_sidebar_pays_the_doors_lead_once() {
        // The sidebar's glyph was at `chrome::FIELDS_LEAD + files.sidebarGutter`
        // — two cells, 21 px — while its own name was one cell away on the
        // other side. The gutter and the lead are the same job, and the row was
        // paying for it twice.
        let dir = scratch("sidebar-air");
        std::fs::write(dir.join("one.txt"), b"x").unwrap();
        let mut v = view_beside(&dir, 4);
        v.reread();
        let texts = bundle();
        let split = v.split(120);
        assert!(split > 0, "the sidebar did not appear");
        let rows = v.drawn(120, &texts);
        let side: Vec<u16> = rows
            .first()
            .expect("a row")
            .fields
            .iter()
            .filter(|f| f.at < split)
            .map(|f| f.at)
            .collect();
        assert_eq!(
            side.first(),
            Some(&0),
            "the sidebar's glyph does not start in the line's first cell"
        );
        // AND ITS NAME IS TWO CELLS ON — the glyph and ONE cell of air, which
        // is exactly what the listing's icon gets.
        assert_eq!(
            side.get(1),
            Some(&2),
            "the sidebar's name has no cell of air"
        );
    }

    #[test]
    fn tab_walks_the_sections_that_are_on_screen() {
        // *"It has to be possible to move the focus between them with Tab and Shift+Tab."*
        let dir = scratch("panes-tab");
        for n in ["a.txt", "b.txt"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        // THE PANELS ON, which is how the manager opens: `view_beside` goes
        // through `plainly` and turns them off.
        let mut v = view_beside(&dir, 4);
        v.preview_on = true;
        v.info_on = true;
        v.reread();
        let wide = 160u16;
        assert_eq!(
            v.panes_on(wide),
            vec![Pane::Sidebar, Pane::Listing, Pane::Preview, Pane::Info],
            "the four sections are not on screen"
        );

        // AND THE DRAWN EVIDENCE OF WHERE THE KEYBOARD IS: the sidebar's row
        // carries `state.accent` only while the sidebar has it.
        let texts = bundle();
        let lit = |v: &mut View| {
            let split = v.split(wide);
            v.drawn(wide, &texts)
                .first()
                .expect("a row")
                .fields
                .iter()
                .any(|f| f.at < split && f.color == ronoctua_tokens::color::STATE_ACCENT)
        };
        assert!(!lit(&mut v), "the sidebar was already lit");

        // FORWARDS, and round. `panes(wide)` is what remembers the width a key
        // arrives without.
        let tab = of_view("TAB");
        let _ = v.panes(wide);
        assert!(matches!(v.key(tab), Step::Changed));
        assert!(!lit(&mut v), "Tab in the listing should go to the preview");
        assert!(matches!(v.key(tab), Step::Changed));
        assert!(!lit(&mut v), "Tab in the preview should go to the facts");
        assert!(matches!(v.key(tab), Step::Changed));
        assert!(
            lit(&mut v),
            "Tab in the facts should go back to the sidebar"
        );
        assert!(matches!(v.key(tab), Step::Changed));
        assert!(
            !lit(&mut v),
            "Tab in the sidebar should go back to the listing"
        );

        // AND BACKWARDS, which is the whole of `S-TAB`: one press from the
        // listing lands in the sidebar.
        let back = of_view("S-TAB");
        assert!(matches!(v.key(back), Step::Changed));
        assert!(
            lit(&mut v),
            "Shift+Tab in the listing should go to the sidebar"
        );

        // AND A SECTION THAT IS NOT DRAWN IS NOT A STOP. With no sidebar and
        // both panels off there is one place to be, and `Tab` stays there
        // rather than putting the cursor where nobody can see it.
        let mut alone = view_at(&dir);
        alone.reread();
        assert_eq!(alone.panes_on(wide), vec![Pane::Listing]);
        assert!(matches!(alone.key(tab), Step::Changed));
        assert_eq!(alone.pane(), Pane::Listing);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_sections_bar_sits_in_its_own_column() {
        // The owner photographed the facts' bar reading as if it sat BELOW the
        // listing's rather than inside its own section, and the cause was that
        // the listing's was in the SURFACE's last cell — which, with the panels
        // on the right, is inside the info panel.
        let dir = scratch("panes-bars");
        for n in 0..40 {
            std::fs::write(dir.join(format!("f{n:02}.txt")), b"x").unwrap();
        }
        let mut v = view_beside(&dir, 30);
        v.preview_on = true;
        v.info_on = true;
        v.reread();
        let wide = 160u16;
        let panes = v.panes(wide);
        assert!(
            panes.sidebar > 0 && panes.right > 0,
            "the test screen does not have the four sections"
        );

        // EVERY SECTION'S BAR IS INSIDE ITS OWN COLUMN, and no two share one.
        //
        // The owner photographed the consequence of them not being: the facts'
        // thumb off the side of the screen, and two of them that could not be
        // grabbed at all because the drawing and the pointer test disagreed.
        let (cell, line_px) = (10.0f32, 26.0f32);
        let edge = f32::from(wide) * cell;
        let b = bundle();
        let sections = v.sections(&[], 8, wide, &b);
        let mut boxes: Vec<(Pane, f32, f32)> = Vec::new();
        for sec in &sections {
            let Some((x, w)) = section_px(panes, sec.pane, 1, wide, cell, edge) else {
                continue;
            };
            let Some((bx, _, bw, bh)) = sec.bar_box(x + w, 0.0, line_px * 12.0, cell, line_px)
            else {
                continue;
            };
            assert!(
                bx >= x && bx + bw <= x + w + 0.5,
                "{:?}: the bar ({bx}..{}) left its column ({x}..{})",
                sec.pane,
                bx + bw,
                x + w
            );
            assert!(
                bx + bw <= edge,
                "{:?}: the bar went past the edge",
                sec.pane
            );
            assert!(bh >= line_px - 0.5, "{:?}: the thumb vanished", sec.pane);
            boxes.push((sec.pane, bx, bx + bw));
        }
        assert!(boxes.len() >= 2, "expected bars in more than one section");
        for (i, (pane, from, to)) in boxes.iter().enumerate() {
            for (other, ofrom, oto) in boxes.iter().skip(i + 1) {
                assert!(
                    to <= ofrom || from >= oto,
                    "{pane:?} and {other:?} draw the bar in the same column"
                );
            }
        }

        // AND A SECTION WITH NOTHING TO SCROLL SHOWS NO BAR, which is what a
        // bar means everywhere else in this shell.
        let roomy = v.sections(&[], 200, wide, &b);
        for sec in &roomy {
            let Some((x, w)) = section_px(panes, sec.pane, 1, wide, cell, edge) else {
                continue;
            };
            assert_eq!(
                sec.bar_box(x + w, 0.0, line_px * 12.0, cell, line_px),
                None,
                "{:?}: a bar over a whole list",
                sec.pane
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bar_over_a_big_directory_stays_inside_the_region() {
        // The facts are a dozen rows and this arithmetic fitted in sixteen
        // bits by accident. A LISTING is however many files are in the folder,
        // and `offset * room` passes 65 535 at six thousand entries — a panic
        // in the frame, in a debug build, on a directory nobody would think to
        // test with.
        let line_px = 26.0f32;
        for (total, offset) in [(200_000usize, 199_000usize), (6_000, 5_500), (13, 1)] {
            let sec = Section {
                pane: Pane::Listing,
                from: 0,
                to: 40,
                top: 0,
                lines: 12,
                offset,
                total,
                above: 0,
                content: Vec::new(),
            };
            let (_, y, _, tall) = sec
                .bar_box(400.0, 0.0, line_px * 12.0, 10.0, line_px)
                .expect("the list was cut");
            assert!(tall >= line_px - 0.5, "the thumb vanished");
            assert!(
                y + tall <= 12.0 * line_px + 0.5,
                "the thumb went past the end of the region: {y} + {tall}"
            );
        }
    }

    #[test]
    fn the_hint_line_names_both_keys_that_walk_the_sections() {
        // `TAB` and `S-TAB` are in no registry and in no manual, so this line
        // is the only thing on screen that says they exist — the lesson `C-o`
        // already cost this file. And BOTH of them: somebody told only about
        // the way forward has to guess that there is a way back.
        let t = bundle();
        let next = key_that_runs("files-pane-next").expect("the key that goes forward");
        let back = key_that_runs("files-pane-previous").expect("the key that goes back");
        for sidebar in [false, true] {
            let line = hint_of(&t, sidebar);
            assert!(
                line.contains(&back.notation()),
                "the hint does not say {}: {line}",
                back.notation()
            );
            // `TAB` is a substring of `S-TAB`, so counting is what proves the
            // forward key is named as well as the backward one.
            assert!(
                line.matches(&next.notation()).count() >= 2,
                "the hint does not say {}: {line}",
                next.notation()
            );
        }
    }
}
