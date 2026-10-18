//! THREE COLUMNS SIDE BY SIDE — the kinds, a list and the detail of the
//! one under the cursor — where each is at a width, and which column a
//! cell is in.
//!
//! # Why a module of its own
//!
//! The containers' view laid itself out this way first, and Incus took the
//! same arithmetic by naming the containers' view: one screen recompiling
//! with another for a shape both draw. The shape is shared; what each lists
//! is its own.

use ronoctua_tokens as tokens;

/// Which column the keyboard is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Pane {
    /// The kinds.
    Kinds,
    /// The list of the kind.
    #[default]
    List,
    /// The detail of the row chosen.
    Detail,
}

/// How the lines are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Three columns side by side.
    Panes,
    /// One list, the kinds on a strip, the detail over the list.
    Stacked,
}

/// The layout a door of `columns` cells draws. The footer's region is always
/// stacked — its height is a few lines, and three columns of four lines each
/// would be three lists nobody can read; a window is side by side from
/// `space.containers.panesFrom` on.
#[must_use]
pub const fn layout_of(columns: u16, footer: bool) -> Layout {
    if !footer && columns >= tokens::space::CONTAINERS_PANESFROM {
        Layout::Panes
    } else {
        Layout::Stacked
    }
}

/// Where each column is on a row, in the row's cells: `(first, width)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Places {
    /// The kinds.
    pub kinds: (u16, u16),
    /// The list.
    pub list: (u16, u16),
    /// The detail.
    pub detail: (u16, u16),
}

impl Places {
    /// Where a column is.
    #[must_use]
    pub const fn of(&self, pane: Pane) -> (u16, u16) {
        match pane {
            Pane::Kinds => self.kinds,
            Pane::List => self.list,
            Pane::Detail => self.detail,
        }
    }

    /// The cell of the divider BEFORE a column — `None` for the first.
    #[must_use]
    pub const fn divider_before(&self, pane: Pane) -> Option<u16> {
        match pane {
            Pane::Kinds => None,
            Pane::List => Some(self.list.0.saturating_sub(DIVIDER_AIR + 1)),
            Pane::Detail => Some(self.detail.0.saturating_sub(DIVIDER_AIR + 1)),
        }
    }

    /// The column a cell is in.
    #[must_use]
    pub const fn pane_at(&self, cell: u16) -> Pane {
        if cell < self.list.0.saturating_sub(DIVIDER_AIR) {
            Pane::Kinds
        } else if cell < self.detail.0.saturating_sub(DIVIDER_AIR) {
            Pane::List
        } else {
            Pane::Detail
        }
    }
}

/// The cells of air each side of a divider.
const DIVIDER_AIR: u16 = 1;

/// The cell a scrolling column keeps for its bar, at its right end: taken
/// whether the bar is drawn or not, so a column that grows past its room
/// does not move its text.
pub const BAR_CELLS: u16 = 1;

/// Where the rows start and how wide they are — the arithmetic every view of
/// this shape shares (`chrome::extent`).
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    crate::chrome::extent(columns)
}

/// Where the three columns are at this width. Pure.
#[must_use]
pub fn places(columns: u16) -> Places {
    let (left, room) = extent(columns);
    let gap = 2 * DIVIDER_AIR + 1;
    let kinds = tokens::space::CONTAINERS_KINDSCELLS.min(room / 4);
    let rest = room.saturating_sub(kinds + 2 * gap);
    let list = rest * tokens::space::CONTAINERS_LISTPCT / 100;
    let detail = rest.saturating_sub(list);
    let list_at = left + kinds + gap;
    Places {
        kinds: (left, kinds),
        list: (list_at, list),
        detail: (list_at + list + gap, detail),
    }
}

/// One scrolling column of the panes, for the door's bars and wheel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneScroll {
    /// Which.
    pub pane: Pane,
    /// Its first line among the region's lines (the detail's strip is
    /// pinned above its rows).
    pub first: usize,
    /// How many lines it has.
    pub lines: usize,
    /// How many rows it holds.
    pub total: usize,
    /// Its first row on screen.
    pub offset: usize,
    /// The cell its bar is drawn in, counted as the fields are.
    pub column: u16,
}

impl PaneScroll {
    /// As the design crate's scroll reads it.
    #[must_use]
    pub const fn scroll(&self) -> ronoctua_design::Scroll {
        ronoctua_design::Scroll {
            total: self.total,
            lines: self.lines,
            offset: self.offset,
            top: 0,
        }
    }

    /// Whether it holds more than it shows — the only time a bar is drawn.
    #[must_use]
    pub const fn scrolls(&self) -> bool {
        self.total > self.lines
    }
}
