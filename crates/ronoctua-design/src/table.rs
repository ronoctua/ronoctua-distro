//! A table of columns, resolved in two phases.
//!
//! # The shape, and why it is this shape
//!
//! It is `shell::files`'s `layout_beside` and `sidebar_columns`, generalised:
//! **phase one is a PREFERENCE** — every column asked for, and the column that
//! takes the rest kept above its floor — and **phase two is what never gives
//! way**: nothing leaks off the screen, because the compositor does not clip.
//! The same two phases the modeline's `layout::resolve` uses. A table that only
//! had phase one loses its last column off the right edge on a narrow screen,
//! and nothing says so: the cells are simply not drawn.
//!
//! # What it replaces
//!
//! Three copies of a grid's geometry, each with its own constants, plus a bare
//! literal for the right-hand air in each. The constants live with the caller
//! now — a date column is eleven cells because a date is eleven cells — and the
//! ARITHMETIC lives here, once.

use alloc::vec::Vec;

use crate::screen::{Run, Screen};
use crate::{GAP, Palette};

/// How much a column wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// A known size: a date, a byte count, a glyph. It is this wide or it is
    /// not there.
    Fixed(u16),
    /// The column that takes whatever is left. **At most one per table** — two
    /// of them would be two answers to "who gets the remainder".
    Fill {
        /// The narrowest it may be squeezed to before a fixed column is dropped
        /// instead. A PREFERENCE, and the first thing to go in phase two.
        floor: u16,
    },
}

/// A column asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    /// How the caller finds its field again. A `&'static str`, so this crate
    /// carries no buffer's vocabulary.
    pub id: &'static str,
    /// How much it wants.
    pub width: Width,
    /// Where it sits in the order of giving way: **the highest gives way
    /// first**, and ties are broken by the later declaration.
    ///
    /// It is not a priority in the ordinary sense — it is a statement about
    /// what the table is FOR. `files::GIVE_WAY` reads as one: the group and the
    /// owner go before the permissions, which go before the date, which goes
    /// before the size, and the icon goes last because two cells answer "what
    /// is this" faster than any word. An order that drifts turns the narrow
    /// case into whatever the last edit happened to leave.
    pub rank: u8,
}

impl Column {
    /// A column of a known size.
    #[must_use]
    pub const fn fixed(id: &'static str, cells: u16, rank: u8) -> Self {
        Self {
            id,
            width: Width::Fixed(cells),
            rank,
        }
    }

    /// The column that takes the rest. Its `rank` is never read: it does not
    /// give way while any other column can.
    #[must_use]
    pub const fn fill(id: &'static str, floor: u16) -> Self {
        Self {
            id,
            width: Width::Fill { floor },
            rank: 0,
        }
    }
}

/// Where a column ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    /// The column's id.
    pub id: &'static str,
    /// Its first cell.
    pub at: u16,
    /// How many cells it got — and the budget every run in it declares.
    pub width: u16,
}

/// The columns that survived, left to right.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layout {
    fields: Vec<Field>,
}

impl Layout {
    /// The fields, in drawing order.
    #[must_use]
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// Where a column ended up, or `None` when it gave way.
    #[must_use]
    pub fn field(&self, id: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.id == id)
    }

    /// Did this column survive?
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.field(id).is_some()
    }

    /// The cell after the last column — where the table ends.
    #[must_use]
    pub fn ends(&self) -> u16 {
        self.fields
            .last()
            .map_or(0, |f| f.at.saturating_add(f.width))
    }
}

/// The columns asked for, and the width there is.
#[derive(Debug, Clone, Default)]
pub struct Table {
    columns: u16,
    gutter: u16,
    edge: u16,
    wanted: Vec<Column>,
}

impl Table {
    /// A table across the whole buffer.
    ///
    /// Gutter and right edge are **zero**, and that is not an omission: the
    /// host has already moved the buffer's content in by
    /// `space.buffer.marginX` on both sides (`buffer.rs`'s `context_of`
    /// subtracts it twice before reporting `columns`). A buffer that leaves a
    /// gutter of its own leaves it twice — the "every buffer with its own
    /// margin" in the report. See [`Self::inset`] for the caller that draws
    /// where the host's margin does not reach.
    #[must_use]
    pub const fn across(columns: u16) -> Self {
        Self {
            columns,
            gutter: 0,
            edge: 0,
            wanted: Vec::new(),
        }
    }

    /// Keep this much air outside the first and last columns.
    ///
    /// For a table the host's buffer margin does not cover — the footer's own
    /// grid, where `files::GUTTER` is the cell the mark lives in and
    /// `files::EDGE_RIGHT` is what keeps the date from reading as a column that
    /// was cut off rather than one that ended.
    ///
    /// Both come off BEFORE anything is decided: they are not columns and they
    /// never give way.
    #[must_use]
    pub const fn inset(mut self, gutter: u16, edge: u16) -> Self {
        self.gutter = gutter;
        self.edge = edge;
        self
    }

    /// Ask for a column. Declaration order is drawing order.
    #[must_use]
    pub fn with(mut self, column: Column) -> Self {
        self.wanted.push(column);
        self
    }

    /// Which columns fit, and where.
    ///
    /// # The two phases
    ///
    /// **Phase one is a PREFERENCE**: columns give way, in the declared order,
    /// until the fill column is above its floor. If it never gets there, every
    /// column that can give way has done so — the floor is a preference, and a
    /// preference is not what holds when the screen is smaller than it. That is
    /// `files::layout_beside`'s own loop, and it is what makes the answer
    /// MONOTONIC: a column that has gone at one width has gone at every
    /// narrower one, which is the property a person reading the screen actually
    /// relies on.
    ///
    /// **Phase two is what never gives way**: nothing leaks off the screen. It
    /// keeps dropping until the arrangement places — and "places" means the
    /// fill column got at least one cell and the last column ends at or before
    /// `columns - edge`.
    ///
    /// If not even one column places, the layout is EMPTY. A window that narrow
    /// has nothing to say, and drawing into it is what leaks.
    #[must_use]
    pub fn resolve(&self) -> Layout {
        let order = self.giving_way();
        let floor = self.fill_floor();
        let mut given = 0;
        // PHASE ONE.
        while given < order.len() && !self.contents(&order, given, floor) {
            given += 1;
        }
        // PHASE TWO.
        loop {
            if let Some((fields, _)) = self.place(&self.keeping(&order, given)) {
                return Layout { fields };
            }
            if given == order.len() {
                return Layout::default();
            }
            given += 1;
        }
    }

    /// The columns still standing after the first `given` have gone.
    fn keeping(&self, order: &[usize], given: usize) -> Vec<usize> {
        (0..self.wanted.len())
            .filter(|i| !order[..given].contains(i))
            .collect()
    }

    /// Does this arrangement place AND keep the fill column above its floor?
    ///
    /// A table with no fill column is content with placing: there is no
    /// remainder for anybody to be short of.
    fn contents(&self, order: &[usize], given: usize, floor: Option<u16>) -> bool {
        let Some((_, fill)) = self.place(&self.keeping(order, given)) else {
            return false;
        };
        fill.is_none_or(|got| floor.is_none_or(|f| got >= f))
    }

    /// The order in which columns give way: highest rank first, and among
    /// equals the one declared later.
    ///
    /// The fill column is not in it. It does not give way while any other
    /// column can — a table about files that does not say which file is not a
    /// narrower table, it is a different one.
    fn giving_way(&self) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.wanted.len())
            .filter(|i| matches!(self.wanted[*i].width, Width::Fixed(_)))
            .collect();
        // A sort by the key `(rank, index)` reversed: `sort_by_key` is stable,
        // so reversing the index too is what makes "the later declaration goes
        // first" hold instead of being whatever the sort happened to do.
        order.sort_by_key(|i| core::cmp::Reverse((self.wanted[*i].rank, *i)));
        order
    }

    /// The fill column's floor, when there is one.
    fn fill_floor(&self) -> Option<u16> {
        self.wanted.iter().find_map(|c| match c.width {
            Width::Fill { floor } => Some(floor),
            Width::Fixed(_) => None,
        })
    }

    /// Lay these columns out, or refuse.
    ///
    /// Refuses when the fill column would get nothing, and when the last column
    /// would end past `columns - edge`. Those two refusals ARE phase two: every
    /// layout this function returns has already been proved not to leak.
    fn place(&self, kept: &[usize]) -> Option<(Vec<Field>, Option<u16>)> {
        let usable = self
            .columns
            .saturating_sub(self.gutter)
            .saturating_sub(self.edge);
        // What the fixed columns cost, each with the cell of air after it. The
        // air after the LAST one is never used, and it is charged anyway: that
        // is what leaves the fill column exactly the remainder, and it is the
        // same accounting `files::layout_beside` does.
        let cost: u16 = kept
            .iter()
            .filter_map(|i| match self.wanted[*i].width {
                Width::Fixed(w) => Some(w.saturating_add(GAP)),
                Width::Fill { .. } => None,
            })
            .fold(0u16, u16::saturating_add);
        let fill = kept
            .iter()
            .any(|i| matches!(self.wanted[*i].width, Width::Fill { .. }))
            .then(|| usable.saturating_sub(cost));
        if fill == Some(0) {
            return None;
        }
        let mut fields = Vec::with_capacity(kept.len());
        let mut at = self.gutter;
        for i in kept {
            let width = match self.wanted[*i].width {
                Width::Fixed(w) => w,
                Width::Fill { .. } => fill?,
            };
            fields.push(Field {
                id: self.wanted[*i].id,
                at,
                width,
            });
            at = at.saturating_add(width).saturating_add(GAP);
        }
        let ends = fields
            .last()
            .map_or(self.gutter, |f| f.at.saturating_add(f.width));
        if ends.saturating_add(self.edge) > self.columns {
            return None;
        }
        Some((fields, fill))
    }
}

/// One row of a table: the fields go where the [`Layout`] put the columns.
///
/// It exists so that no buffer writes a column's first cell or its budget by
/// hand again. Both come from the layout, which is what makes "two fields on
/// one row never overlap" a property of the arithmetic instead of a thing to
/// remember.
pub struct Row<'a, P: Palette> {
    screen: &'a mut Screen<P>,
    line: u16,
    layout: &'a Layout,
    tall: u16,
    /// The role the block this row is in was filled with, if it was filled.
    ///
    /// A row inside a container is a row drawn ON the container, and the
    /// quietest step of the text ladder is invisible there — measured Lc
    /// 0,0000 in all nine themes. `Pen::row_of` hands the ground down so the
    /// lift reaches a table's cells too; a row taken straight off the screen
    /// has no block and so no ground, which is `None` and lifts nothing.
    ground: Option<crate::Role>,
}

impl<'a, P: Palette> Row<'a, P> {
    pub(crate) fn new(screen: &'a mut Screen<P>, line: u16, layout: &'a Layout) -> Self {
        Self::on(screen, line, layout, None)
    }

    /// The same row, told which ground it is drawn on.
    pub(crate) fn on(
        screen: &'a mut Screen<P>,
        line: u16,
        layout: &'a Layout,
        ground: Option<crate::Role>,
    ) -> Self {
        Self {
            screen,
            line,
            layout,
            tall: 1,
            ground,
        }
    }

    /// Put a run in a column.
    ///
    /// Returns `false` when the column gave way — so a caller can build the row
    /// unconditionally and let the layout decide, instead of asking twice
    /// whether each column survived.
    ///
    /// The run's own `column` and `budget` are OVERWRITTEN by the field's. A
    /// run that could contradict the layout would be a second place where the
    /// table's geometry is decided, which is the thing this crate exists to
    /// stop.
    pub fn put(&mut self, id: &str, run: Run<P::Icon>) -> bool {
        let Some(field) = self.layout.field(id) else {
            return false;
        };
        let role = crate::screen::on_ground(run.role, self.ground);
        // AND IT IS DRAWN AT THE SIZE IT WAS MEASURED IN. A column budgets
        // CELLS and `Scale::Small` advances 0,81 of one, so the next column
        // starts where the budget ends and not where the ink stopped — a gap
        // proportional to the phrase, which is what the owner read across the
        // brain's header. See `Screen::measured`.
        let run = self.screen.measured(run);
        let placed = run.at(field.at).within(field.width).in_role(role);
        let cells = self.screen.put(self.line, placed);
        self.tall = self.tall.max(cells);
        true
    }

    /// How many cells of height the row took: the tallest run in it.
    ///
    /// The row after this one starts at `line() + tall()`, never at `line + 1`
    /// — a `Large` run covers the two cells `Heights` charges it, and the row
    /// under it would be drawn inside its box.
    ///
    /// **One even when nothing was drawn.** A row occupies the cell it names
    /// whether or not any column survived, and a caller stepping by zero would
    /// draw its whole list on one line.
    #[must_use]
    pub const fn tall(&self) -> u16 {
        self.tall
    }

    /// Finish the row and say how many cells it took.
    ///
    /// It consumes the row, which is what gives the screen back to the caller —
    /// the borrow ends here rather than at the end of a block somebody has to
    /// remember to write.
    #[must_use]
    pub const fn done(self) -> u16 {
        self.tall
    }

    /// Which row this is.
    #[must_use]
    pub const fn line(&self) -> u16 {
        self.line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tokens;
    use crate::screen::Screen;
    use alloc::vec;
    use ronoctua_protocol::{Context, Heights};

    /// A file listing, in this crate's vocabulary: `files::GIVE_WAY`'s order,
    /// `files::Column`'s widths, and the name as the fill.
    fn listing(columns: u16) -> Table {
        Table::across(columns)
            .with(Column::fixed("icon", 2, 1))
            .with(Column::fill("name", 12))
            .with(Column::fixed("perms", 10, 4))
            .with(Column::fixed("owner", 8, 5))
            .with(Column::fixed("group", 8, 6))
            .with(Column::fixed("size", 7, 2))
            .with(Column::fixed("modified", 11, 3))
    }

    /// NOTHING LEAKS, at every width there is.
    #[test]
    fn no_field_ever_ends_past_the_edge() {
        for columns in 0..=240u16 {
            let layout = listing(columns).resolve();
            for f in layout.fields() {
                assert!(
                    f.at.saturating_add(f.width) <= columns,
                    "{columns}: `{}` runs from {} for {}",
                    f.id,
                    f.at,
                    f.width
                );
            }
        }
    }

    /// The same, with a gutter and a right edge: both are outside the table and
    /// neither gives way.
    #[test]
    fn an_inset_table_keeps_both_edges() {
        for columns in 0..=240u16 {
            let layout = listing(columns).inset(1, 1).resolve();
            for f in layout.fields() {
                assert!(f.at >= 1, "{columns}: `{}` at {}", f.id, f.at);
                assert!(
                    f.at.saturating_add(f.width).saturating_add(1) <= columns,
                    "{columns}: `{}` reaches {}",
                    f.id,
                    f.at.saturating_add(f.width)
                );
            }
        }
    }

    /// THE ORDER IS THE DECLARED ORDER, and it is a statement about what the
    /// table is for. As the width comes down, the group goes, then the owner,
    /// then the permissions, then the date, then the size, and the icon last.
    #[test]
    fn the_columns_give_way_in_the_declared_order() {
        let gone = |columns: u16| -> Vec<&'static str> {
            let layout = listing(columns).resolve();
            [
                "icon", "name", "perms", "owner", "group", "size", "modified",
            ]
            .into_iter()
            .filter(|id| !layout.has(id))
            .collect()
        };
        // Wide enough for everything.
        assert_eq!(gone(200), Vec::<&str>::new());
        // Narrowing it never brings a column BACK, and never drops one out of
        // turn: what is gone at a given width is gone at every smaller one.
        let order = ["group", "owner", "perms", "modified", "size", "icon"];
        let mut before: Vec<&'static str> = Vec::new();
        for columns in (0..=200u16).rev() {
            let now = gone(columns);
            for id in &before {
                assert!(now.contains(id), "{columns}: `{id}` came back");
            }
            for id in &now {
                let rank = order.iter().position(|o| o == id);
                if let Some(rank) = rank {
                    for earlier in &order[..rank] {
                        assert!(
                            now.contains(earlier),
                            "{columns}: `{id}` went before `{earlier}`"
                        );
                    }
                }
            }
            before = now;
        }
    }

    /// The fill column's floor is a PREFERENCE: it is honoured while any fixed
    /// column can still be dropped, and it is the first thing to go after that.
    #[test]
    fn the_floor_holds_while_a_column_can_still_go() {
        // 2 + 1 (icon) + 12 (the name's floor) = 15 is the narrowest listing
        // that still honours the floor with everything else gone.
        let layout = listing(15).resolve();
        assert_eq!(layout.field("name").map(|f| f.width), Some(12));
        assert!(layout.has("icon"));
        // One cell less and the ICON gives way — it is last in the order, and
        // "last" is not "never". With it gone the name takes the whole width,
        // which is the same answer `files::layout_beside` gives at 14.
        let tight = listing(14).resolve();
        assert!(!tight.has("icon"));
        assert_eq!(tight.field("name").map(|f| f.width), Some(14));
        assert!(tight.fields().iter().all(|f| f.at + f.width <= 14));
    }

    /// TWO FIELDS ON ONE ROW NEVER OVERLAP — asked of the host's own question,
    /// not of a reimplementation of it.
    #[test]
    fn a_row_of_fields_survives_the_hosts_question() {
        let heights = Heights::from_sizes(12.0, 14.0, 20.0, 28.0, 40.0, 1.45);
        for columns in 0..=200u16 {
            let layout = listing(columns).resolve();
            let ctx = Context {
                columns,
                lines: 24,
                heights,
                hide_header: false,
            };
            let mut screen = Screen::<Tokens>::new(&ctx);
            let mut row = screen.row_of(0, &layout);
            // A LABEL in every column, because a label claims its WHOLE budget:
            // if two budgets could meet, this is the frame that shows it.
            for id in [
                "icon", "name", "perms", "owner", "group", "size", "modified",
            ] {
                row.put(id, Run::label(id));
            }
            assert_eq!(row.done(), 1);
            screen
                .check()
                .unwrap_or_else(|e| panic!("{columns} columns: {e}"));
        }
    }

    /// A run cannot contradict the layout: the field's cell and budget win.
    /// FAILS IF: a run put in a column is drawn at a size that does not advance
    /// one cell, and the frame says nothing about it.
    ///
    /// The whole of the defect the owner photographed across the brain's
    /// header: a column budgets CELLS, `Scale::Small` advances 0,81 of one, and
    /// whatever follows starts where the BUDGET ends. The slack is
    /// proportional to the phrase, so a long column opens a wide gap and a
    /// short one a narrow gap — *"espaços desproporcionais entre os atalhos"*,
    /// and a separator with more air on one side than the other.
    #[test]
    fn a_column_draws_at_the_size_it_measures() {
        let heights = Heights::from_sizes(12.0, 14.0, 20.0, 28.0, 40.0, 1.45);
        let layout = Table::across(40)
            .with(Column::fixed("a", 10, 1))
            .with(Column::fill("b", 4))
            .resolve();
        let ctx = Context {
            columns: 40,
            lines: 8,
            heights,
            hide_header: false,
        };
        let mut screen = Screen::<Tokens>::new(&ctx);
        let mut row = screen.row_of(0, &layout);
        assert!(row.put("a", Run::datum("x").small()));
        assert_eq!(row.done(), 1);
        assert_eq!(screen.mismeasured(), 1, "the frame did not notice");
        assert!(
            screen.check().is_err(),
            "a frame with a mis-measured column passed its own check"
        );
        // AND THE INK IS STILL THERE, in the right place: what was wrong is the
        // SIZE, and at the grid's size the run is exactly where the layout put
        // it. Turning it away would cost the row a column to fix a gap.
        let list = screen.as_protocol();
        let ronoctua_protocol::Command::Text { scale, column, .. } = &list.commands[0] else {
            panic!("a run");
        };
        assert_eq!(*column, 0);
        assert_ne!(*scale, ronoctua_protocol::Scale::Small);
    }

    #[test]
    fn a_field_takes_its_column_and_not_the_runs_own() {
        let heights = Heights::from_sizes(12.0, 14.0, 20.0, 28.0, 40.0, 1.45);
        let layout = Table::across(40)
            .with(Column::fixed("a", 10, 1))
            .with(Column::fill("b", 4))
            .resolve();
        let ctx = Context {
            columns: 40,
            lines: 8,
            heights,
            hide_header: false,
        };
        let mut screen = Screen::<Tokens>::new(&ctx);
        let mut row = screen.row_of(0, &layout);
        assert!(row.put("b", Run::datum("x").at(99).within(99)));
        assert_eq!(row.done(), 1);
        let list = screen.as_protocol();
        let ronoctua_protocol::Command::Text {
            column, max_width, ..
        } = &list.commands[0]
        else {
            panic!("a run");
        };
        assert_eq!(*column, 11);
        assert_eq!(*max_width, layout.field("b").unwrap().width);
    }

    /// A window with nothing to say gets an empty layout rather than a field
    /// drawn off the screen.
    #[test]
    fn a_window_too_narrow_draws_nothing() {
        assert_eq!(
            Table::across(0)
                .with(Column::fill("a", 1))
                .resolve()
                .fields(),
            &[]
        );
        assert_eq!(
            Table::across(2)
                .with(Column::fixed("a", 4, 1))
                .resolve()
                .fields(),
            &[]
        );
    }

    /// A table of fixed columns only — no fill — still gives way and still does
    /// not leak.
    #[test]
    fn a_table_with_no_fill_column_still_gives_way() {
        // Eight cells hold one column of four and its cell of air, and not
        // two: the second would end at nine.
        let table = Table::across(8)
            .with(Column::fixed("a", 4, 1))
            .with(Column::fixed("b", 4, 9));
        let layout = table.resolve();
        assert_eq!(layout.fields().len(), 1);
        assert_eq!(layout.field("a").map(|f| f.width), Some(4));
        assert_eq!(
            vec!["a"],
            layout.fields().iter().map(|f| f.id).collect::<Vec<_>>()
        );
    }
}
