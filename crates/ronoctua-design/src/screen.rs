//! The builder: a buffer stops pushing `Command` by hand.
//!
//! A [`Screen`] holds the context the host sent, the row it has got to, and the
//! frame so far. Everything it hands out is measured against the real
//! [`Context`], so a budget it gives is a budget that exists — which is the
//! half of "nothing leaks off the screen" a buffer used to have to remember.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use ronoctua_protocol::{Color, Command, Context, DisplayList, Heights, Scale, Text};

use crate::budget::{room, tall};
use crate::table::{Layout, Row};
use crate::{AIR, CONTENT_FLOOR, GAP, ICON, Palette, Role, Tokens};

/// What a run says.
///
/// The same three the protocol's [`Text`] distinguishes, and for the same
/// reason (invariants 8 and 11): **data** comes out literal, a **label** comes
/// out as a key and the host translates, an **icon** is a token whose codepoint
/// the host resolves. The icon is typed, because on the guest it is a generated
/// enum where a name that does not exist does not compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ink<I> {
    /// The buffer's own data: a device node, a byte count, a glyph's name.
    Datum(String),
    /// An interface label, by KEY. The buffer never sees the sentence.
    Label(String),
    /// An icon, by token.
    Glyph(I),
}

/// One run of text: what it says, what part it plays, where it sits and how
/// much room it may take.
///
/// The `budget` is the field a buffer used to compute by hand seven times over.
/// `0` means "to the right edge", which is the protocol's own spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run<I> {
    /// What it says.
    pub ink: Ink<I>,
    /// What part it plays. [`Palette`] turns it into a colour.
    pub role: Role,
    /// How big. The cell grid is always [`Scale::Normal`]'s.
    pub scale: Scale,
    /// The first cell, counted from the buffer's own left edge — which the host
    /// has already moved in by `space.buffer.marginX`.
    pub column: u16,
    /// How many cells it may occupy. `0` = to the edge.
    pub budget: u16,
}

impl<I> Run<I> {
    /// A label, by key. The host translates it.
    #[must_use]
    pub fn label(key: &str) -> Self {
        Self::of(Ink::Label(key.to_string()))
    }

    /// Literal data.
    #[must_use]
    pub fn datum(text: &str) -> Self {
        Self::of(Ink::Datum(text.to_string()))
    }

    /// An icon, by token.
    ///
    /// It claims [`ICON`](crate::ICON) cells by default — that is what a glyph
    /// on the body's grid costs, and leaving it to the edge would make every
    /// icon collide with whatever comes after it.
    #[must_use]
    pub fn glyph(icon: I) -> Self {
        let mut r = Self::of(Ink::Glyph(icon));
        r.budget = ICON;
        r
    }

    /// The common part of the three: body text at the body's size, at cell
    /// zero, with no budget of its own.
    fn of(ink: Ink<I>) -> Self {
        Self {
            ink,
            role: Role::Body,
            scale: Scale::Normal,
            column: 0,
            budget: 0,
        }
    }

    /// Put it at this cell.
    #[must_use]
    pub const fn at(mut self, column: u16) -> Self {
        self.column = column;
        self
    }

    /// Give it this many cells at most. `0` = to the edge.
    #[must_use]
    pub const fn within(mut self, budget: u16) -> Self {
        self.budget = budget;
        self
    }

    /// Put it in this register, whatever it was in.
    ///
    /// Not a builder anybody calls: it is how [`Pen`] applies the ground's
    /// own answer (see [`on_ground`]) without every draw site having to ask.
    #[must_use]
    pub(crate) const fn in_role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// Make it the subject of its block.
    #[must_use]
    pub const fn title(mut self) -> Self {
        self.role = Role::Title;
        self
    }

    /// Make it something that accompanies the content.
    #[must_use]
    pub const fn quiet(mut self) -> Self {
        self.role = Role::Quiet;
        self
    }

    /// Make it a label naming a value.
    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.role = Role::Dim;
        self
    }

    /// Mark it — by COLOUR, never by a box or an outline (invariant 15).
    #[must_use]
    pub const fn accent(mut self) -> Self {
        self.role = Role::Accent;
        self
    }

    /// Give it an explicit part to play.
    #[must_use]
    pub const fn playing(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// Draw it smaller than the body.
    #[must_use]
    pub const fn small(mut self) -> Self {
        self.scale = Scale::Small;
        self
    }

    /// Draw it at the SYSTEM's size — the footer's and the minibuffer's.
    #[must_use]
    pub const fn base(mut self) -> Self {
        self.scale = Scale::Base;
        self
    }

    /// Draw it larger: content that IS the shape.
    #[must_use]
    pub const fn large(mut self) -> Self {
        self.scale = Scale::Large;
        self
    }

    /// Draw it as the screen's subject.
    #[must_use]
    pub const fn huge(mut self) -> Self {
        self.scale = Scale::Huge;
        self
    }
}

// THERE IS NO `scaled(Scale)`, and the omission is the point.
//
// The four helpers above plus the default cover all five sizes, so a component
// never names a `Scale` at all — which matters because `types::*` hands it the
// WIT's `Scale` while the arithmetic here counts in the protocol's. Two enums
// of one name in one file is how a reader stops knowing which is which, and a
// setter taking either of them is the invitation.

/// One thing in the frame, before a palette has been consulted.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part<I> {
    /// A run of text.
    Ran { line: u16, run: Run<I> },
    /// A piece of CODE, coloured by what it is rather than by what part of
    /// the screen it plays.
    ///
    /// A part of its own and not a `Run` with a second field, because the two
    /// carry different vocabularies: a `Run` names a `Role`, which is a part
    /// of a screen, and this names a syntax role, which is what a character
    /// IS. One enum for each keeps both closed.
    #[cfg(feature = "syntax")]
    Coded {
        line: u16,
        column: u16,
        text: alloc::string::String,
        /// `None` is ORDINARY text, which is ordinary by omission: the
        /// highlighter emits only what is not, so the pieces between two
        /// ranges have no syntax role at all. They take the layout's own
        /// `Role::Body`, which is what any other line of a buffer takes.
        role: Option<ronoctua_syntax::Role>,
    },
    /// A one-line rule, across the row or across part of it.
    Ruled {
        line: u16,
        column: u16,
        columns: u16,
        /// Which rule — the one that reads against the chrome, or the one that
        /// reads against a raised ground. It travels here for the reason the
        /// ground's own role does: a rule is a SIGNAL drawn on something, and
        /// the something is not a fact this can be looked up from later.
        role: Role,
    },
    /// A ground under one line, or under part of one — the band under a
    /// focused row, a container's own, a column's.
    Banded {
        line: u16,
        column: u16,
        columns: u16,
        role: Role,
    },
    /// A sentence wrapped into a rectangle, from this wrapped row down.
    Wrapped {
        line: u16,
        lines: u16,
        /// The first WRAPPED row to draw. Only the host knows how many there
        /// are, so this side keeps the offset and the host clamps it — see
        /// [`ronoctua_protocol::Command::Paragraph`].
        from: u16,
        run: Run<I>,
    },
    /// A scrollbar down one column of a block of rows.
    Barred {
        line: u16,
        lines: u16,
        column: u16,
        scroll: crate::Scroll,
        /// The same question a rule answers, and the same answer: a bar drawn
        /// down a container is drawn ON the container.
        role: Role,
    },
}

/// The frame under construction.
///
/// It carries the [`Context`] because every budget it hands out is measured
/// against the real width — a `Screen` that did not know it would be handing
/// out numbers a buffer would then have to check, which is where the numbers
/// used to go wrong.
#[derive(Debug, Clone)]
pub struct Screen<P: Palette> {
    columns: u16,
    lines: u16,
    heights: Heights,
    row: u16,
    parts: Vec<Part<P::Icon>>,
    /// The blocks this frame was divided into, in the order they were asked
    /// for. Empty until [`Screen::split`] is called — a buffer that never
    /// divides its window keeps the flat surface it always had.
    regions: Vec<crate::Region>,
    /// The first line that is anchored to the window's FOOT, if any.
    ///
    /// Set by [`Screen::split`] when the window ends in a FIXED block, which
    /// is what a footer is. See [`ronoctua_protocol::Command::Anchor`] for the
    /// residue this exists to put somewhere nobody is looking.
    foot_from: Option<u16>,
    /// How many runs were put in a COLUMN at a size that is not the grid's.
    ///
    /// # Why this is counted at all
    ///
    /// Because a buffer lays out in CELLS and `Scale::Small` does not advance
    /// one: it is 0,81 of a cell, so a run of eleven clusters budgeted eleven
    /// cells draws nine and leaves two of slack — and whatever the layout puts
    /// after it starts where the layout says, not where the ink stopped. The
    /// slack is proportional to the PHRASE, so two columns of different
    /// lengths open different gaps, and the owner read exactly that off the
    /// brain's header: *"espaços desproporcionais entre os atalhos"*, and a
    /// separator with more air on one side than the other.
    ///
    /// Measured on that screen: `Switch step` was budgeted 118,6 px and drew
    /// 96, which put 3,2 cells before the dot and 1,6 after it.
    ///
    /// A small run that owns the rest of its line is fine — nothing follows it
    /// to be pushed. This counts the one case where something does: a column.
    mismeasured: u32,
    /// How many draws fell outside the region that was drawing them.
    ///
    /// A count and not a flag, because the number is what makes a failing test
    /// say how bad it is. [`Screen::check`] refuses any frame with one.
    refused: u32,
}

impl<P: Palette> Screen<P> {
    /// A fresh frame for this context, at row zero.
    #[must_use]
    pub fn new(ctx: &Context) -> Self {
        Self {
            columns: ctx.columns,
            lines: ctx.lines,
            heights: ctx.heights,
            row: 0,
            parts: Vec::new(),
            regions: Vec::new(),
            foot_from: None,
            mismeasured: 0,
            refused: 0,
        }
    }

    /// How wide the buffer is, in cells. The host has already taken its margin
    /// off both sides.
    #[must_use]
    pub const fn columns(&self) -> u16 {
        self.columns
    }

    /// How tall the buffer is, in cells.
    #[must_use]
    pub const fn lines(&self) -> u16 {
        self.lines
    }

    /// How many cells each scale costs. The host computed it; the buffer could
    /// not, having neither the font size nor the line height (invariant 11).
    #[must_use]
    pub const fn heights(&self) -> Heights {
        self.heights
    }

    /// The row the next line would be written on.
    #[must_use]
    pub const fn row(&self) -> u16 {
        self.row
    }

    /// Go to this row.
    pub const fn go_to(&mut self, row: u16) {
        self.row = row;
    }

    /// Leave this many rows empty.
    pub const fn skip(&mut self, rows: u16) {
        self.row = self.row.saturating_add(rows);
    }

    /// How many cells a run of this scale occupies in height.
    ///
    /// `max(1)`, because a host that reported zero would make every row of the
    /// frame land on the same line — and a run always occupies at least the
    /// cell it names.
    #[must_use]
    pub const fn tall(&self, scale: Scale) -> u16 {
        tall(&self.heights, scale)
    }

    /// Put a run on a row, and say how many cells of height it took.
    ///
    /// The row does not move: a row of columns is several runs at one line, and
    /// what advances is [`Self::line`] or a [`Row`].
    pub fn put(&mut self, line: u16, run: Run<P::Icon>) -> u16 {
        let cells = self.tall(run.scale);
        self.parts.push(Part::Ran { line, run });
        cells
    }

    /// Writes one line of code, already cut into the runs of its highlight.
    ///
    /// Returns what the NEXT line has to be given: a block comment, a raw
    /// string or a markdown fence carries across lines, and a buffer that
    /// dropped it would colour a docstring's second line as if it were code.
    ///
    /// # The byte-to-cell arithmetic is HERE, and nowhere else
    ///
    /// A highlight's ranges are byte offsets; a column is a cell. Every buffer
    /// that wanted coloured code would otherwise write that conversion again,
    /// and the wrong way of writing it — counting BYTES — puts a run one cell
    /// off the moment an accent arrives. Counting characters is what matches
    /// the two, and this crate is where the cell arithmetic already lives.
    ///
    /// `budget` is the whole line's; each piece is charged what it takes and
    /// nothing past the budget is emitted, so `nothing_is_covered` holds
    /// without the buffer checking.
    #[cfg(feature = "syntax")]
    pub fn code(
        &mut self,
        line: u16,
        column: u16,
        budget: u16,
        text: &str,
        language: ronoctua_syntax::Language,
        state: ronoctua_syntax::State,
    ) -> ronoctua_syntax::State {
        let mut runs = alloc::vec::Vec::new();
        let carried = ronoctua_syntax::line_into(text, language.grammar(), state, &mut runs);

        let room = if budget == 0 {
            self.columns.saturating_sub(column)
        } else {
            budget
        };
        let mut cut = 0usize;
        let mut cell = 0u16;

        // ORDINARY, COLOURED, ORDINARY: the ranges carry only what is not
        // ordinary, so what falls between two of them is the rest of the line.
        for range in &runs {
            if range.start < cut || range.end > text.len() {
                continue;
            }
            cell = self.coded(line, column, room, cell, &text[cut..range.start], None);
            cell = self.coded(
                line,
                column,
                room,
                cell,
                &text[range.start..range.end],
                Some(range.role),
            );
            cut = range.end;
        }
        self.coded(line, column, room, cell, &text[cut.min(text.len())..], None);
        carried
    }

    /// One piece of a coded line, placed at the cell it starts in.
    ///
    /// Returns the cell the next piece starts at, whether or not this one was
    /// emitted: the arithmetic has to go on past the budget, or every piece
    /// after a cut one would be placed at the wrong column.
    #[cfg(feature = "syntax")]
    fn coded(
        &mut self,
        line: u16,
        column: u16,
        room: u16,
        cell: u16,
        piece: &str,
        role: Option<ronoctua_syntax::Role>,
    ) -> u16 {
        // CLUSTERS, AND NOT BYTES OR CHARACTERS. A cell holds one cluster: an
        // accent is two bytes, and a COMBINING accent is two characters —
        // charging either would put every run after the first one to the right
        // of where it is drawn. And this both measures and CUTS, so it has to
        // be the ruler the rest of the system cuts with; the version that took
        // `chars` could split an emoji sequence and draw a different emoji.
        let wide = ronoctua_protocol::cells(piece);
        if wide == 0 || cell >= room {
            return cell.saturating_add(wide);
        }
        let take = usize::from(room - cell);
        let shown: alloc::string::String = ronoctua_protocol::cells_of(piece).take(take).collect();
        self.parts.push(Part::Coded {
            line,
            column: column.saturating_add(cell),
            role,
            text: shown,
        });
        cell.saturating_add(wide)
    }

    /// Put a run on the current row and move past it.
    ///
    /// Past its whole BOX, and not one row: a `Large` run covers the two cells
    /// [`Heights`] charges it, and the rule a buffer drew at `line + 1` landed
    /// on the title's own baseline and came out as an underline touching the
    /// descenders. That defect is the reason `DisplayList::nothing_is_covered`
    /// exists, and this is where it stops being possible.
    pub fn line(&mut self, run: Run<P::Icon>) -> u16 {
        let at = self.row;
        let cells = self.put(at, run);
        self.row = self.row.saturating_add(cells);
        at
    }

    /// A one-line rule, FULL BLEED.
    ///
    /// # What a rule's row means
    ///
    /// The boundary at the **end** of row `line`, and not a glyph drawn inside
    /// it. The host paints it edge to edge, outside the buffer's own horizontal
    /// margin: only text is inset. That is why this function takes a row and
    /// nothing else — there is no column and no width to get wrong, and
    /// `DisplayList::columns_of` already charges a rule the whole width by
    /// definition.
    ///
    /// It is also the answer to the rule that "left a gap on the left and
    /// spilled on the right": a buffer that insets a rule is insetting it
    /// twice, because the host has already moved everything in by
    /// `space.buffer.marginX` and a full-bleed quad must not be moved at all.
    pub fn rule(&mut self, line: u16) {
        self.rule_in(line, 0, 0);
    }

    /// The same rule, across PART of the row.
    ///
    /// `columns: 0` is to the edge, which is what [`Self::rule`] asks for. A
    /// rule under a heading inside one of two side-by-side columns has to stop
    /// where the column does — full-bleed it would run through its neighbour.
    pub fn rule_in(&mut self, line: u16, column: u16, columns: u16) {
        self.rule_as(line, column, columns, Role::Rule);
    }

    /// The same rule, in the register a RAISED ground asks for.
    ///
    /// Not public, and deliberately: nothing that draws a rule should be
    /// choosing between the two. [`Pen::rule`] is the one caller, and it
    /// chooses from the ground the block was filled with — see
    /// [`Role::RuleRaised`].
    fn rule_as(&mut self, line: u16, column: u16, columns: u16, role: Role) {
        self.parts.push(Part::Ruled {
            line,
            column,
            columns,
            role,
        });
    }

    /// The band under a list's focused row, across the whole of it.
    ///
    /// [`Self::ground_in`] is the one that takes a column and a width — a list
    /// that is one of two side by side wants its band to end where it does.
    pub fn band(&mut self, line: u16) {
        self.ground(line, Role::Band);
    }

    /// A sentence, WRAPPED, in a rectangle of cells.
    ///
    /// # Why this cannot be a `Run`
    ///
    /// Because a `Run` is one row and the host elides it at the budget, which
    /// is right for a label and useless for a description: cut at the width of
    /// a column, a description is a thing nobody can read. Wrapping needs the
    /// TRANSLATED string, and this side never has one (invariant 11).
    ///
    /// So the rectangle crosses and the host fills it, with the same
    /// `layout::wrap` a long file name already goes through. It is what lets a
    /// buffer have a footer that explains the thing under the cursor — the
    /// shape every view in the shell has and no buffer could have.
    pub fn paragraph(&mut self, line: u16, lines: u16, run: Run<P::Icon>) {
        self.paragraph_from(line, lines, 0, run);
    }

    /// The same sentence, from this wrapped row down.
    ///
    /// A sentence longer than the rectangle it was given is a sentence
    /// somebody cannot read, and the owner asked for the obvious answer: the
    /// footer scrolls. What this side can hold is the OFFSET; the total is the
    /// host's, because the host is what wraps.
    pub fn paragraph_from(&mut self, line: u16, lines: u16, from: u16, run: Run<P::Icon>) {
        self.parts.push(Part::Wrapped {
            line,
            lines,
            from,
            run,
        });
    }

    /// A scrollbar down the right of a block of rows.
    ///
    /// # Why a buffer could not have one before
    ///
    /// It was written down as a rule — *a buffer has five primitives and none
    /// of them is a scrollable region* — and the rule was wrong twice over. A
    /// buffer already draws whatever window of a list it likes: it owns the
    /// offset, it emits the rows it wants, and the host clips nothing. What it
    /// could not do was draw the THUMB, because a thumb is a fraction of a
    /// track in pixels and this side counts cells. Built out of block
    /// characters it would have been a sixth scrollbar that resembles the five
    /// real ones and matches none of them.
    ///
    /// So the bar crosses as numbers and the host draws it with [`Scroll`] —
    /// the same arithmetic, compiled on both sides of this wall since it was
    /// written, which is what makes a buffer's bar the system's bar.
    ///
    /// # What the caller owes
    ///
    /// The COLUMN. A bar is one cell wide and it must not be drawn over
    /// content, so whoever asks for one has already taken that cell out of its
    /// rows' budget. Nothing here can check it — the runs were placed before
    /// this is called — but `DisplayList::nothing_overlaps` will, and it names
    /// both the bar and what it landed on.
    ///
    /// Nothing is drawn when the list fits: [`Scroll::cut`] decides, on the
    /// host's side, and the whole shell agrees a bar over a list that fits is
    /// a control that does nothing.
    pub fn bar(&mut self, line: u16, lines: u16, column: u16, scroll: crate::Scroll) {
        self.bar_as(line, lines, column, scroll, Role::Rule);
    }

    /// The same bar, in the register a RAISED ground asks for. [`Pen::bar`] is
    /// the one caller, for the reason [`Self::rule_as`] gives.
    fn bar_as(&mut self, line: u16, lines: u16, column: u16, scroll: crate::Scroll, role: Role) {
        // A LIST THAT FITS HAS NO BAR, and it has none HERE rather than on the
        // far side of the wall.
        //
        // `Scroll::cut` was always the one place that decided it, and the host
        // honoured it — it drew nothing. But the command still crossed, and a
        // command that draws nothing still has a BOX: two lines of a column
        // that the frame's own guard then finds somebody else's text inside.
        // The keyboard buffer's list of two, in a window with room for
        // thirty-five, is where it showed.
        //
        // Deciding it on this side also means a buffer's own tests see the
        // same frame the screen does, which is the whole contract of `check`.
        if !scroll.cut() {
            return;
        }
        self.parts.push(Part::Barred {
            line,
            lines,
            column,
            scroll,
            role,
        });
    }

    /// The ground under one line, in a register the caller names.
    ///
    /// [`Self::band`] and [`Self::shelf`] are the two a list needs and are
    /// spelled out because they are the two anybody reaches for. A header is a
    /// third, and a fourth is a matter of time — this is the one door, and the
    /// two above it are the names that made the common cases readable.
    pub fn ground(&mut self, line: u16, role: Role) {
        self.ground_in(line, 0, 0, role);
    }

    /// The ground under PART of a line.
    ///
    /// `columns: 0` is to the edge, which is what [`Self::ground`] asks for.
    ///
    /// # Why a band is not always a stripe
    ///
    /// Because two columns side by side, each with its own selection, is an
    /// ordinary layout — and it is unreachable from a ground that always spans
    /// the row. The rule that said a ground "paints the whole row, and that is
    /// all" was a description of the only shape the type could carry, written
    /// down as though it were a decision.
    pub fn ground_in(&mut self, line: u16, column: u16, columns: u16, role: Role) {
        self.parts.push(Part::Banded {
            line,
            column,
            columns,
            role,
        });
    }

    /// The ground under one line of a CONTAINER.
    ///
    /// The same primitive and a different register: a band says *this one* and
    /// a shelf says *these*. See [`Role::Shelf`] for why five rows of history
    /// drawn in the band's colour would read as five selected rows.
    pub fn shelf(&mut self, line: u16) {
        self.ground(line, Role::Shelf);
    }

    /// **The** heading: a glyph, a title, and the air around them.
    ///
    /// Returns the first row of the content, and leaves the cursor there.
    ///
    /// # The block
    ///
    /// ```text
    /// row+0   air
    /// row+1   glyph (cell 0, ICON cells) + title key (cell ICON+GAP, to the edge)
    /// row+2   air
    /// row+3   the first row of content        <- what this returns
    /// ```
    ///
    /// # THERE IS NO RULE, and that is the whole of the separation
    ///
    /// What separates a title from its content is whitespace, and only
    /// whitespace. Four buffers had four answers and three of them drew a rule;
    /// the one that did not — the memory panel, where `ARCHITECTURE.md` records
    /// that "what separates is only the empty cell" — is the one that was
    /// right, and the owner has standardised on it. This is not the separation
    /// argument being given up: `colors.nix` opens by saying separation is by
    /// whitespace and a one-line rule and never by a container, and a heading
    /// that already has a colour and a glyph in front of it does not need the
    /// second half.
    ///
    /// [`Screen::rule`] stays in the vocabulary — it is still what the memory
    /// panel puts between two blocks and what the three browsers put above
    /// their grids, and it is still full bleed. What changed is that a heading
    /// never emits one. **A rule inside a heading is now the defect.**
    ///
    /// # The air, in pixels, because that is the only way to show it is equal
    ///
    /// The cell is `font.size.base × font.lineHeight = 18 × 1.45 = 26.1 px`.
    /// The font's box is `1.25 em = 22.5 px` (ascender 0.965 + descender
    /// 0.285), so every run already carries `(26.1 − 22.5) / 2 = 1.8 px` of
    /// slack above its ink and 1.8 below it. Then:
    ///
    /// - above the title's ink: one empty cell plus the title's own top slack —
    ///   `26.1 + 1.8 = 27.9 px`;
    /// - below it, down to the first content ink: the title's bottom slack plus
    ///   one empty cell — `1.8 + 26.1 = 27.9 px`.
    ///
    /// The numbers were 29.0 and 31.0 while a buffer's body was 20 px; the
    /// EQUALITY is the point and it does not depend on the size.
    ///
    /// Two equal gaps, each exactly one line height plus the line box's own
    /// slack. That is what the owner asked for, and it is why the air is not
    /// two cells on one side and none on the other — which is what every buffer
    /// shipped, each differently.
    ///
    /// # Why the title is at the BODY's size
    ///
    /// What makes a line a title is not how big it is: it is the accent glyph
    /// in front of it and the air around it — emphasis by colour, separation by
    /// whitespace, which is what `colors.nix` prescribes and what a heading
    /// wants whether or not a rule is available to it. At the body's size it also costs ONE cell
    /// instead of two, and in a footer with a ceiling that row goes back to the
    /// content.
    ///
    /// # When the window cannot pay
    ///
    /// The air is a PREFERENCE, and the two-phase shape this repository uses
    /// everywhere applies here too: what never gives way is that nothing leaks
    /// off the screen. Below `2 × AIR + 1 + CONTENT_FLOOR` rows the block gives
    /// its air up entirely — the title, and the content directly under it —
    /// rather than taking the last row the content had. It gives it up on BOTH
    /// sides at once: half the air on one side is the asymmetry this function
    /// exists to end.
    pub fn heading(&mut self, icon: P::Icon, key: &str) -> u16 {
        let start = self.row;
        let cells = self.tall(Scale::Normal);
        let (title, content) = heading_block(start, cells, self.lines);

        self.put(title, Run::glyph(icon).accent().at(0).within(ICON));
        let from = ICON.saturating_add(GAP);
        self.put(
            title,
            Run::label(key)
                .title()
                .at(from)
                .within(room(self.columns, from)),
        );
        self.row = content;
        content
    }

    /// A row of a table: the fields go where the [`Layout`] put the columns.
    pub fn row_of<'a>(&'a mut self, line: u16, layout: &'a Layout) -> Row<'a, P> {
        Row::new(self, line, layout)
    }

    /// Divide the window into blocks that TILE it, and remember them.
    ///
    /// The regions come back in the order they were asked for. From here on
    /// every draw should go through [`Self::region`] — and [`Self::check`]
    /// will say so, because once a frame has declared its blocks a part
    /// outside all of them is a part nobody owns.
    ///
    /// See [`crate::region::split`] for what happens on a window too short.
    pub fn split(&mut self, spec: &[crate::Slice]) -> Vec<crate::Region> {
        self.regions = crate::region::split(self.lines, spec);
        // AND A WINDOW THAT ENDS IN A FIXED BLOCK IS ANCHORED TO ITS FOOT.
        //
        // A footer IS a fixed block at the end — that is what makes it a
        // footer — and the residue of a surface falls under the last row,
        // which is where its last row is. The owner read it as a space after
        // the values *"que varia de tamanho conforme a altura disponível"*.
        // See [`ronoctua_protocol::Command::Anchor`].
        //
        // `Slice::Rest` at the end is a TAIL and not a footer: it is whatever
        // is left over, it has no bottom of its own to sit against, and
        // anchoring it would open the gap above a block that nothing pins.
        self.foot_from = match (spec.last(), self.regions.last()) {
            (Some(crate::Slice::Fixed(n)), Some(last)) if *n > 0 && last.lines > 0 => {
                Some(last.top)
            }
            _ => None,
        };
        self.regions.clone()
    }

    /// The first line anchored to the window's foot, if the window ends in a
    /// fixed block.
    #[must_use]
    pub const fn foot_from(&self) -> Option<u16> {
        self.foot_from
    }

    /// The blocks this frame was divided into.
    #[must_use]
    pub fn regions(&self) -> &[crate::Region] {
        &self.regions
    }

    /// How many draws were refused for falling outside their region.
    ///
    /// Zero on every frame that is right. A buffer's own tests read this: it
    /// turns "the footer did not spill" from something somebody looked at into
    /// a number.
    #[must_use]
    pub const fn refused(&self) -> u32 {
        self.refused
    }

    /// How many runs were put in a column at a size that is not the grid's.
    #[must_use]
    pub const fn mismeasured(&self) -> u32 {
        self.mismeasured
    }

    /// Count one, and hand the run back at the grid's size.
    ///
    /// # Why this CLAMPS where a pen refuses
    ///
    /// Because the two defects are not the same defect. A pen refuses a draw
    /// that falls outside its block, and it refuses because the alternative is
    /// ink in a plausible WRONG PLACE, which nobody would look for. Here the
    /// place is exactly right — what is wrong is the size, and drawn at the
    /// grid the run is where the layout says it is. Turning it away would cost
    /// the row a whole column to fix a gap of two cells.
    ///
    /// It is still a defect and it is still counted: [`Screen::check`] refuses
    /// the frame, and every buffer's own tests ask it.
    pub(crate) fn measured(&mut self, run: Run<P::Icon>) -> Run<P::Icon> {
        if run.scale == Scale::Small {
            self.mismeasured = self.mismeasured.saturating_add(1);
            return run.base();
        }
        run
    }

    /// Draw inside one block, in ITS OWN rows.
    ///
    /// Row zero is the region's first line, whatever line of the window that
    /// is. Nothing a pen draws can land outside — which is the whole point,
    /// and the reason the footer can no longer be painted over the list above
    /// it however the arithmetic above goes wrong.
    pub fn region(&mut self, region: crate::Region) -> Pen<'_, P> {
        Pen {
            screen: self,
            region,
            keeps: 0,
            ground: None,
        }
    }

    /// The frame as the protocol spells it.
    ///
    /// On the host this **is** the frame: the roles resolve through
    /// [`Tokens`] and the icons keep their token names. Inside the component it
    /// is a geometry SHADOW — the colours are still real token names, but the
    /// icon names are the generated enum's `Debug` spelling (see
    /// [`Palette::glyph_name`]). Nothing that matters differs: the two
    /// questions below read line, column, scale and budget, and charge an icon
    /// `heights.of(scale)` cells whatever the string says.
    #[must_use]
    pub fn as_protocol(&self) -> DisplayList {
        let mut list = DisplayList::fresh_one();
        // THE ANCHOR FIRST, because it is a fact about the frame and the host
        // reads it before it places anything in it.
        if let Some(foot_from) = self.foot_from {
            list.push(Command::Anchor { foot_from });
        }
        for part in &self.parts {
            list.push(match part {
                Part::Ran { line, run } => Command::Text {
                    line: *line,
                    column: run.column,
                    text: match &run.ink {
                        Ink::Datum(t) => Text::Data(t.clone()),
                        Ink::Label(k) => Text::Key(k.clone()),
                        Ink::Glyph(i) => Text::Icon(P::glyph_name(*i)),
                    },
                    color: Tokens::color(run.role),
                    scale: run.scale,
                    max_width: run.budget,
                },
                #[cfg(feature = "syntax")]
                Part::Coded {
                    line,
                    column,
                    text,
                    role,
                } => Command::Text {
                    line: *line,
                    column: *column,
                    text: Text::Data(text.clone()),
                    // A PIECE WITH NO SYNTAX ROLE IS ORDINARY, and takes the
                    // layout's own body colour — the same one every other line
                    // of a buffer takes.
                    color: role.map_or_else(|| Tokens::color(Role::Body), Tokens::syntax),
                    scale: ronoctua_protocol::Scale::Normal,
                    max_width: 0,
                },
                Part::Ruled {
                    line,
                    column,
                    columns,
                    role,
                } => Command::Rule {
                    line: *line,
                    column: *column,
                    columns: *columns,
                    color: Tokens::color(*role),
                },
                // THE ROLE TRAVELS, because `band-command` already carries a
                // colour: a container's ground and a focused row's are the same
                // primitive in two registers, and adding a sixth command for
                // the second would be a new thing every buffer has to learn.
                Part::Banded {
                    line,
                    column,
                    columns,
                    role,
                } => Command::Selection {
                    line: *line,
                    column: *column,
                    columns: *columns,
                    color: Tokens::color(*role),
                },
                // AND THIS ONE IS NOT A LINE AT ALL, which is why it is a
                // command of its own rather than a third register of
                // `line-command`: it covers a block of rows, it has a column,
                // and the three numbers it carries are what the host needs to
                // resolve a thumb in pixels.
                Part::Wrapped {
                    line,
                    lines,
                    from,
                    run,
                } => Command::Paragraph {
                    line: *line,
                    column: run.column,
                    lines: *lines,
                    columns: if run.budget == 0 {
                        self.columns.saturating_sub(run.column)
                    } else {
                        run.budget
                    },
                    content: match &run.ink {
                        Ink::Datum(t) => Text::Data(t.clone()),
                        Ink::Label(k) => Text::Key(k.clone()),
                        Ink::Glyph(i) => Text::Icon(P::glyph_name(*i)),
                    },
                    color: Tokens::color(run.role),
                    scale: run.scale,
                    first_row: *from,
                },
                Part::Barred {
                    line,
                    lines,
                    column,
                    scroll,
                    role,
                } => Command::Bar {
                    line: *line,
                    lines: *lines,
                    column: *column,
                    total: scroll.total as u32,
                    shown: scroll.lines as u32,
                    offset: scroll.offset as u32,
                    color: Tokens::color(*role),
                },
            });
        }
        list
    }

    /// The two questions the host asks of every frame, asked HERE.
    ///
    /// # Why this is worth more than a test of its own
    ///
    /// `DisplayList::nothing_is_covered` and `nothing_overlaps` are the host's,
    /// and until now a buffer only met them minutes later, in Nix, through the
    /// state-driven checks. Because this crate builds a real
    /// [`DisplayList`], a buffer's own `cargo test` calls the SAME code and
    /// gets the SAME message in milliseconds. There is no second
    /// implementation of the question to keep in step — which was the other
    /// half of the defect this crate closes.
    ///
    /// # Errors
    ///
    /// The protocol's own sentence: which two commands, on which row, and how
    /// far the first one reaches.
    pub fn check(&self) -> Result<(), String> {
        if self.mismeasured > 0 {
            return Err(alloc::format!(
                "{} run(s) were put in a column at a size that does not advance one \
                 cell — the layout budgets cells, so what follows starts where the \
                 budget ends and not where the ink did, and the gap grows with the \
                 phrase",
                self.mismeasured
            ));
        }
        self.regions_are_whole()?;
        let list = self.as_protocol();
        list.nothing_is_covered(self.heights, self.columns)?;
        list.nothing_overlaps(self.heights, self.columns)?;
        list.nothing_is_drawn_below(self.heights, self.lines)
    }

    /// The blocks tile the window, and everything drawn belongs to one.
    ///
    /// Silent on a frame that never called [`Self::split`]: the five buffers
    /// that draw on a flat surface are not wrong, they simply have one block,
    /// and demanding they say so would be a migration with nothing at the end
    /// of it.
    ///
    /// # Errors
    ///
    /// Which two blocks share a line, which line nobody owns, or how many
    /// draws a pen turned away.
    fn regions_are_whole(&self) -> Result<(), String> {
        if self.regions.is_empty() {
            return Ok(());
        }
        if self.refused > 0 {
            return Err(alloc::format!(
                "{} draw(s) fell outside the block that was drawing them — a pen \
                 refuses rather than clamps, so this is ink that is missing from \
                 the screen and not ink in the wrong place",
                self.refused
            ));
        }
        for (n, a) in self.regions.iter().enumerate() {
            for b in self.regions.iter().skip(n + 1) {
                if a.meets(b) {
                    return Err(alloc::format!(
                        "two blocks share a line: {a:?} and {b:?} — one of them is \
                         drawing over the other, which is what a footer painted on \
                         top of a history list looks like from here"
                    ));
                }
            }
        }
        // AND EVERY LINE HAS AN OWNER. The sum alone does not prove it: three
        // blocks of the right total can still leave a hole here and overlap
        // there — and a hole is the strip of the page's own ground the owner
        // photographed between the footer and the bottom edge.
        for line in 0..self.lines {
            if !self.regions.iter().any(|r| r.holds(line)) {
                return Err(alloc::format!(
                    "line {line} of {} belongs to no block — a window divided into \
                     blocks has no gaps, and a gap is the page showing through",
                    self.lines
                ));
            }
        }
        Ok(())
    }

    /// The frame, finished. Host side.
    #[must_use]
    pub fn finish(self) -> DisplayList {
        self.as_protocol()
    }

    /// Every part, for whoever converts them to the guest's own types.
    ///
    /// `pub(crate)` because the conversion is this crate's job: a buffer that
    /// walked the parts itself would be writing the eighth answer to "what
    /// closes a heading". Under `host` there is nobody to convert for, and the
    /// compiler says so.
    #[cfg(feature = "guest")]
    pub(crate) fn parts(&self) -> impl Iterator<Item = Frame<'_, P::Icon>> {
        self.parts.iter().map(|p| match p {
            Part::Ran { line, run } => Frame::Ran(*line, run),
            #[cfg(feature = "syntax")]
            Part::Coded {
                line,
                column,
                text,
                role,
            } => Frame::Coded(*line, *column, text, *role),
            Part::Ruled {
                line,
                column,
                columns,
                role,
            } => Frame::Ruled(*line, *column, *columns, *role),
            Part::Banded {
                line,
                column,
                columns,
                role,
            } => Frame::Banded(*line, *column, *columns, *role),
            Part::Wrapped {
                line,
                lines,
                from,
                run,
            } => Frame::Wrapped(*line, *lines, *from, run),
            Part::Barred {
                line,
                lines,
                column,
                scroll,
                role,
            } => Frame::Barred(*line, *lines, *column, *scroll, *role),
        })
    }
}

/// A hand drawing inside ONE block of the frame.
///
/// Every method takes a row of the REGION, not a line of the window, and every
/// one of them refuses a row the region does not own. Refuses — it does not
/// clamp: a clamped draw lands somewhere plausible and the defect that put it
/// there goes on living. A refusal is counted, [`Screen::refused`] reports it,
/// and [`Screen::check`] fails the frame.
///
/// # Why the pen owns the translation
///
/// Because the alternative is what this crate had: the block's first line
/// threaded through six functions as a bare `u16` called `from`, added by hand
/// at every draw. Every defect the owner photographed in the brain — the
/// footer over the history, the strip of page under the footer, a click in the
/// footer attributed to the log — was one of those additions reaching past the
/// block it belonged to.
pub struct Pen<'a, P: Palette> {
    screen: &'a mut Screen<P>,
    region: crate::Region,
    /// How many cells on the right edge are not this block's to band.
    ///
    /// # Why a band has to know about the bar
    ///
    /// Because a band is full-bleed and a bar is drawn after it, in the last
    /// column — so the band runs UNDER the bar and the bar reads as sitting on
    /// top of the selected row. The owner reported exactly that, in those
    /// words: *"a scrollbar … ficando por cima do item selecionado"*.
    ///
    /// It is the file manager's `EDGE_RIGHT` in one number: a list that keeps
    /// a cell for its bar says so once, and every band it draws stops there.
    /// The block's own GROUND still covers the whole width — a bar is a signal
    /// drawn on the ground, and a gap behind it would be a stripe of the page
    /// showing through.
    keeps: u16,
    /// The role this block's ground was filled with, if it was filled at all.
    ///
    /// # Why the pen remembers it
    ///
    /// Because a rule, a bar and the quietest step of the text ladder all
    /// read DIFFERENTLY on a raised ground, and until this field existed the
    /// choice was the caller's to get wrong — a thing nobody should be
    /// deciding twice. Measured across the nine themes on 2026-09-20:
    /// `rule.default` on `surface.header` is ΔE 0,025 (a line one cannot see)
    /// and `text.subtle.faint` on either raised ground is Lc 0,0000 (text
    /// one cannot see at all).
    ///
    /// So the pen answers it instead: [`Self::fill`] writes this down, and
    /// [`Self::rule`], [`Self::bar_from`] and [`Self::run`] read it. A buffer
    /// that fills a block and then draws in it gets the register that ground
    /// asks for, by construction. Cobra o `a-rule-follows-its-ground`.
    ground: Option<Role>,
}

/// Which register a role takes on a given ground.
///
/// Free of `self` and of the palette on purpose: it is a pure table, and it
/// is the ONE place the question is answered. The two raised grounds are
/// `Shelf` (`surface.container`) and `Header` (`surface.header`); every other
/// fill leaves the role as it was.
pub(crate) const fn on_ground(role: Role, ground: Option<Role>) -> Role {
    match ground {
        Some(Role::Shelf | Role::Header) => match role {
            Role::Rule => Role::RuleRaised,
            // THE QUIETEST STEP HAS NOWHERE TO GO DOWN TO. `text.subtle.faint`
            // sits a hair above the CHROME, and a raised ground eats the hair:
            // Lc 0,0000 in all nine themes. It lifts one step, which is the
            // move `files::lift_onto_a_band` already makes for a dim field on
            // a band — and `text.subtle.default` on a raised ground is
            // measured by a gate of its own, so what it lifts INTO is known.
            Role::Faint => Role::Prose,
            other => other,
        },
        _ => role,
    }
}

impl<P: Palette> Pen<'_, P> {
    /// How many lines this block owns.
    #[must_use]
    pub const fn lines(&self) -> u16 {
        self.region.lines
    }

    /// How wide the buffer is. The block's width is the window's — regions
    /// divide the window by LINE; columns are the caller's own business.
    #[must_use]
    pub const fn columns(&self) -> u16 {
        self.screen.columns
    }

    /// How many cells a run of this scale occupies in height.
    #[must_use]
    pub const fn tall(&self, scale: Scale) -> u16 {
        tall(&self.screen.heights, scale)
    }

    /// The block this pen draws in.
    #[must_use]
    pub const fn region(&self) -> crate::Region {
        self.region
    }

    /// Whether this block owns no lines, and so draws nothing.
    #[must_use]
    pub const fn empty(&self) -> bool {
        self.region.empty()
    }

    /// The window line a row of this block is, or `None` — and counted as a
    /// refusal when it is `None`, because asking is what a draw does.
    fn at(&mut self, row: u16, rows: u16) -> Option<u16> {
        // A BLOCK THAT CLAIMS ROWS DOWNWARD IS MEASURED WHOLE. A paragraph of
        // four rows starting on the block's last one is three rows outside it,
        // and the first row being inside is exactly what made that look fine.
        let last = row.saturating_add(rows.max(1)).saturating_sub(1);
        if last >= self.region.lines {
            self.screen.refused = self.screen.refused.saturating_add(1);
            return None;
        }
        self.region.line_of(row)
    }

    /// Put a run on a row of this block, and say how many cells it took.
    ///
    /// Zero when the row was refused, which is the same answer a caller gets
    /// from a column that gave way: nothing was drawn, and the cursor must not
    /// move as though something had been.
    pub fn put(&mut self, row: u16, run: Run<P::Icon>) -> u16 {
        let cells = self.tall(run.scale);
        let role = on_ground(run.role, self.ground);
        let Some(line) = self.at(row, cells) else {
            return 0;
        };
        let run = run.in_role(role);
        self.screen.put(line, run)
    }

    /// The ground under one row of this block, in a register the caller names.
    ///
    /// It stops where [`Self::keeping`] says, for the reason the band does: a
    /// hover that ran under the bar would put the pointer's row behind it.
    pub fn ground(&mut self, row: u16, role: Role) {
        let columns = self.banded();
        if let Some(line) = self.at(row, 1) {
            self.screen.ground_in(line, 0, columns, role);
        }
    }

    /// The ground under EVERY row of this block.
    ///
    /// The common case for a container, and the one that cannot be written
    /// wrong: a loop from the block's first line to its last is where an
    /// off-by-one puts a container's colour on its neighbour's first row.
    pub fn fill(&mut self, role: Role) {
        self.ground = Some(role);
        // TO THE EDGE, AND NOT TO `banded()`: a block's own ground is what the
        // bar is drawn ON, and a gap behind it would be a stripe of the page
        // showing through the container. What `keeping` holds back is the
        // BAND, which says "this row" and must not run under the control.
        for row in 0..self.region.lines {
            if let Some(line) = self.at(row, 1) {
                self.screen.ground(line, role);
            }
        }
    }

    /// Keep this many cells of the right edge for something else — a bar.
    ///
    /// Said ONCE, before the rows are drawn, and every band and every hover
    /// this pen draws afterwards stops there. See [`Self::keeps`].
    pub const fn keeping(&mut self, cells: u16) {
        self.keeps = cells;
    }

    /// How wide a band of this block is: its columns, less what is kept.
    const fn banded(&self) -> u16 {
        // ZERO IS "TO THE EDGE" in `ground_in`, so a block that keeps nothing
        // says zero rather than its own width — the two are the same picture
        // and only one of them survives the window being resized.
        if self.keeps == 0 {
            0
        } else {
            self.screen.columns.saturating_sub(self.keeps)
        }
    }

    /// The band under this block's focused row.
    pub fn band(&mut self, row: u16) {
        let columns = self.banded();
        if let Some(line) = self.at(row, 1) {
            self.screen.ground_in(line, 0, columns, Role::Band);
        }
    }

    /// The ground under one row of a container.
    pub fn shelf(&mut self, row: u16) {
        if let Some(line) = self.at(row, 1) {
            self.screen.shelf(line);
        }
    }

    /// A one-line rule at the end of a row of this block, in the register the
    /// block's own ground asks for. See [`Pen::ground`] and [`on_ground`].
    pub fn rule(&mut self, row: u16) {
        let role = on_ground(Role::Rule, self.ground);
        if let Some(line) = self.at(row, 1) {
            self.screen.rule_as(line, 0, 0, role);
        }
    }

    /// A sentence wrapped into a rectangle of this block's rows.
    pub fn paragraph(&mut self, row: u16, rows: u16, run: Run<P::Icon>) {
        self.paragraph_from(row, rows, 0, run);
    }

    /// The same sentence, from this wrapped row down. See
    /// [`Screen::paragraph_from`].
    pub fn paragraph_from(&mut self, row: u16, rows: u16, from: u16, run: Run<P::Icon>) {
        let role = on_ground(run.role, self.ground);
        let run = run.in_role(role);
        if let Some(line) = self.at(row, rows) {
            self.screen.paragraph_from(line, rows, from, run);
        }
    }

    /// A scrollbar down the right of this block.
    ///
    /// It covers the block, which is the shape a bar always wants and the one
    /// a caller kept having to spell out — `bar(from, to - from, …)` with the
    /// two numbers it had already used for everything else.
    pub fn bar(&mut self, column: u16, scroll: crate::Scroll) {
        self.bar_from(0, column, scroll);
    }

    /// The same bar, over the block from `row` down.
    ///
    /// # Why a block's first rows may not scroll
    ///
    /// Because a container's TITLE is inside its ground and does not move with
    /// its contents — so a track that began at the block's first line would be
    /// one line longer than the list it belongs to. The host builds the thumb
    /// from the band it is given and the window the `Scroll` names, and it says
    /// so out loud when the two disagree; this is how a caller keeps them
    /// agreeing. It is the file manager's `Section::above` in one argument.
    pub fn bar_from(&mut self, row: u16, column: u16, scroll: crate::Scroll) {
        let lines = self.region.lines.saturating_sub(row);
        if lines == 0 {
            return;
        }
        let role = on_ground(Role::Rule, self.ground);
        self.screen.bar_as(
            self.region.top.saturating_add(row),
            lines,
            column,
            scroll,
            role,
        );
    }

    /// A row of a table, in this block's own rows.
    ///
    /// `None` when the row is not this block's — a caller walks its own rows
    /// and so should never see one, and that is the point: it is provable
    /// rather than assumed.
    pub fn row_of<'b>(&'b mut self, row: u16, layout: &'b Layout) -> Option<Row<'b, P>> {
        let ground = self.ground;
        let line = self.at(row, 1)?;
        Some(Row::on(self.screen, line, layout, ground))
    }
}

/// A part, borrowed, for the guest's conversion.
#[cfg(feature = "guest")]
pub(crate) enum Frame<'a, I> {
    Ran(u16, &'a Run<I>),
    #[cfg(feature = "syntax")]
    Coded(u16, u16, &'a str, Option<ronoctua_syntax::Role>),
    Ruled(u16, u16, u16, Role),
    Banded(u16, u16, u16, Role),
    Wrapped(u16, u16, u16, &'a Run<I>),
    Barred(u16, u16, u16, crate::Scroll, Role),
}

/// Where the title and the content go.
///
/// Its own function, and not three lines inside [`Screen::heading`], because it
/// is the arithmetic the tests interrogate: the air has to come out equal on
/// both sides, and a test that had to read it back out of a `Vec<Command>`
/// would be proving the reading and not the sum.
///
/// See [`Screen::heading`] for the pixels.
#[must_use]
pub fn heading_block(start: u16, cells: u16, lines: u16) -> (u16, u16) {
    // PHASE ONE, the preference: one cell of air above the title's box and one
    // below it. Nothing between them — there is no rule any more.
    let title = start.saturating_add(AIR);
    let content = title.saturating_add(cells).saturating_add(AIR);
    // PHASE TWO, what does not give way: the block plus a row of content has to
    // fit in the window. The compositor does not clip.
    if content.saturating_add(CONTENT_FLOOR) <= lines {
        return (title, content);
    }
    // Tight: no air on either side. A narrower window gets the title and its
    // content adjacent rather than a heading with air on one side only.
    (start, start.saturating_add(cells))
}

/// The colour a role resolves to on the host, for whoever needs the name.
#[must_use]
pub fn token_of(role: Role) -> Color {
    Tokens::color(role)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tokens;
    use std::vec;

    /// The production heights: the five sizes of `typography.nix` against a
    /// line height of 1.45. `Normal` is the cell, so it is 1; `Large` is two,
    /// which is the case every geometry defect in this repository came from.
    fn heights() -> Heights {
        Heights::from_sizes(12.0, 14.0, 20.0, 28.0, 40.0, 1.45)
    }

    fn context(columns: u16, lines: u16) -> Context {
        Context {
            columns,
            lines,
            heights: heights(),
        }
    }

    /// THE AIR IS EQUAL ON BOTH SIDES, at every scale a title could take.
    ///
    /// Measured the way the screen shows it: above the title's box, and between
    /// that box and the first row of content.
    #[test]
    fn the_heading_breathes_the_same_on_both_sides() {
        for scale in [
            Scale::Small,
            Scale::Base,
            Scale::Normal,
            Scale::Large,
            Scale::Huge,
        ] {
            let cells = tall(&heights(), scale);
            // A window big enough that the preference is affordable.
            let (title, content) = heading_block(0, cells, 64);
            let above = title;
            let below = content - (title + cells);
            assert_eq!(above, AIR, "air above the title, {scale:?}");
            assert_eq!(below, AIR, "air below the title, {scale:?}");
        }
    }

    /// The block costs `2 × AIR` plus the title's own box, and the title's box
    /// is what `Heights` says — never one row.
    #[test]
    fn the_heading_costs_its_air_plus_the_titles_box() {
        for scale in [Scale::Normal, Scale::Large, Scale::Huge] {
            let cells = tall(&heights(), scale);
            let (_, content) = heading_block(0, cells, 64);
            assert_eq!(content, AIR * 2 + cells, "{scale:?}");
        }
    }

    /// THE HEADING EMITS A GLYPH AND A TITLE, AND NOTHING ELSE.
    ///
    /// A rule inside a heading is the defect: what separates a title from its
    /// content is whitespace and only whitespace. Three buffers drew one, the
    /// memory panel did not, and the memory panel was right.
    #[test]
    fn a_heading_draws_no_rule() {
        let mut screen = Screen::<Tokens>::new(&context(80, 24));
        let content = screen.heading("disk", "cmd-format-disk");
        let list = screen.as_protocol();
        assert_eq!(list.commands.len(), 2, "a glyph and a title, and no more");
        assert!(
            !list
                .commands
                .iter()
                .any(|c| matches!(c, Command::Rule { .. })),
            "a rule appeared in a heading"
        );
        assert_eq!(content, AIR * 2 + tall(&heights(), Scale::Normal));
    }

    /// The air gives way; "nothing leaks" does not.
    ///
    /// At every window height, the title and the first content row have to be
    /// rows the window actually has.
    #[test]
    fn a_short_window_gives_the_air_up_rather_than_the_content() {
        let cells = tall(&heights(), Scale::Normal);
        for lines in 1..=64u16 {
            let (title, content) = heading_block(0, cells, lines);
            assert!(title < lines, "the title leaks at {lines} lines");
            assert!(content <= lines, "the content leaks at {lines} lines");
        }
    }

    /// A `Large` title with a rule one row under it is exactly the frame
    /// `nothing_is_covered` was written for — and a heading has to PASS now.
    #[test]
    fn a_heading_survives_the_hosts_two_questions() {
        for columns in [12u16, 24, 80, 200] {
            for lines in [4u16, 12, 40] {
                let mut screen = Screen::<Tokens>::new(&context(columns, lines));
                let content = screen.heading("disk", "cmd-format-disk");
                if content < lines {
                    screen.put(content, Run::datum("sda1").at(0));
                }
                screen
                    .check()
                    .unwrap_or_else(|e| panic!("{columns}x{lines}: {e}"));
            }
        }
    }

    /// THE TEST THAT PROVES THE TEST BITES.
    ///
    /// The defect the headings used to have, reproduced by hand: a `Large`
    /// title with a rule one row under it. If `check` did not answer, this
    /// would pass and every test above would be decoration.
    #[test]
    fn the_old_defect_is_still_refused() {
        let mut screen = Screen::<Tokens>::new(&context(80, 24));
        screen.put(0, Run::label("cmd-format-disk").title().large().at(0));
        screen.rule(1);
        let trouble = screen.check().expect_err("a rule on the title's baseline");
        assert!(trouble.contains("inside the box"), "{trouble}");
    }

    /// Two runs on one row that reach each other are refused, and the budget
    /// this crate hands out is what stops them.
    #[test]
    fn two_labels_on_one_row_do_not_meet() {
        let mut screen = Screen::<Tokens>::new(&context(40, 8));
        screen.put(0, Run::label("a").at(0).within(20));
        screen.put(0, Run::label("b").at(20).within(20));
        screen.check().expect("adjacent budgets do not overlap");

        let mut bad = Screen::<Tokens>::new(&context(40, 8));
        bad.put(0, Run::label("a").at(0).within(21));
        bad.put(0, Run::label("b").at(20).within(20));
        assert!(bad.check().is_err(), "a budget that reaches into the next");
    }

    /// `line` moves past the run's BOX, not past one row.
    #[test]
    fn writing_a_large_run_moves_two_rows() {
        let mut screen = Screen::<Tokens>::new(&context(40, 24));
        screen.line(Run::datum("x").large());
        assert_eq!(screen.row(), tall(&heights(), Scale::Large));
        screen.line(Run::datum("y"));
        screen.check().expect("no run lands inside another's box");
    }

    /// FAILS IF: a rule stops following the ground it is drawn on.
    ///
    /// The whole of `Role::RuleRaised`'s reason in four lines: a block that
    /// was filled with a raised ground gets the raised rule, one that was not
    /// gets the default one, and no buffer chooses between them. Measured,
    /// `rule.default` on `surface.header` is ΔE 0,025 — a line one cannot see.
    #[test]
    fn a_rule_takes_the_register_of_its_ground() {
        for (fill, wanted) in [
            (Some(Role::Header), Role::RuleRaised),
            (Some(Role::Shelf), Role::RuleRaised),
            (Some(Role::Band), Role::Rule),
            (None, Role::Rule),
        ] {
            let mut screen = Screen::<Tokens>::new(&context(40, 8));
            let blocks = screen.split(&[crate::Slice::Rest]);
            let mut pen = screen.region(blocks[0]);
            if let Some(role) = fill {
                pen.fill(role);
            }
            pen.rule(2);
            pen.bar(
                39,
                crate::Scroll {
                    total: 20,
                    lines: 8,
                    offset: 0,
                    top: 0,
                },
            );
            let list = screen.as_protocol();
            let ruled = list
                .commands
                .iter()
                .find_map(|c| match c {
                    Command::Rule { color, .. } => Some(color.clone()),
                    _ => None,
                })
                .expect("a rule");
            let barred = list
                .commands
                .iter()
                .find_map(|c| match c {
                    Command::Bar { color, .. } => Some(color.clone()),
                    _ => None,
                })
                .expect("a bar");
            assert_eq!(ruled, token_of(wanted), "the rule on {fill:?}");
            assert_eq!(barred, token_of(wanted), "the bar on {fill:?}");
        }
    }

    /// FAILS IF: the quietest step of the text ladder is drawn on a ground
    /// that eats it.
    ///
    /// `text.subtle.faint` measures Lc 0,0000 on both raised grounds in all
    /// nine themes — not quiet, absent. `Pen` lifts it one step; a block with
    /// no ground of its own leaves it where it is.
    #[test]
    fn the_faintest_step_lifts_off_a_raised_ground() {
        for (fill, wanted) in [
            (Some(Role::Header), Role::Prose),
            (Some(Role::Shelf), Role::Prose),
            (None, Role::Faint),
        ] {
            let mut screen = Screen::<Tokens>::new(&context(40, 8));
            let blocks = screen.split(&[crate::Slice::Rest]);
            let mut pen = screen.region(blocks[0]);
            if let Some(role) = fill {
                pen.fill(role);
            }
            pen.put(1, Run::datum("x").playing(Role::Faint));
            let list = screen.as_protocol();
            let drawn = list
                .commands
                .iter()
                .find_map(|c| match c {
                    Command::Text { color, .. } => Some(color.clone()),
                    _ => None,
                })
                .expect("a run");
            assert_eq!(drawn, token_of(wanted), "the faint run on {fill:?}");
        }
    }

    /// A rule names a row and nothing else — full bleed, no column, no width.
    #[test]
    fn a_rule_is_only_a_row() {
        let mut screen = Screen::<Tokens>::new(&context(40, 8));
        screen.rule(3);
        let list = screen.as_protocol();
        assert_eq!(
            list.commands,
            vec![Command::Rule {
                line: 3,
                column: 0,
                columns: 0,
                color: token_of(Role::Rule),
            }]
        );
    }
    /// FAILS IF: the byte-to-cell arithmetic counts BYTES.
    ///
    /// An accent is two bytes and one cell, so a line with one in it puts
    /// every run after it a column to the right of where it is drawn — the
    /// defect this crate exists to have one copy of rather than seven.
    #[cfg(feature = "syntax")]
    #[test]
    fn a_coded_line_places_its_runs_in_cells_and_not_in_bytes() {
        use ronoctua_protocol::Command;

        let mut screen: Screen<crate::Tokens> = Screen::new(&context(40, 12));
        let text = "let ação = 1;";
        screen.code(
            0,
            0,
            40,
            text,
            ronoctua_syntax::Language::Nix,
            ronoctua_syntax::State::fresh(),
        );
        let list = screen.as_protocol();

        // The `1` is a number, and it is at the cell its CHARACTERS put it at.
        let wanted = text.chars().position(|c| c == '1').expect("a numeral");
        let found = list.commands.iter().find_map(|c| match c {
            Command::Text {
                column,
                text: ronoctua_protocol::Text::Data(t),
                ..
            } if t == "1" => Some(*column),
            _ => None,
        });
        assert_eq!(
            found,
            Some(u16::try_from(wanted).unwrap()),
            "the numeral landed off its own column: {:?}",
            list.commands
        );
        // AND NOTHING OVERLAPS, which is what the host refuses.
        let heights = heights();
        assert!(list.nothing_overlaps(heights, 40).is_ok());
    }

    /// FAILS IF: the carried state is dropped, and a docstring's second line
    /// comes out as code.
    #[cfg(feature = "syntax")]
    #[test]
    fn a_coded_line_hands_the_next_one_what_it_left_open() {
        let mut screen: Screen<crate::Tokens> = Screen::new(&context(40, 12));
        let after = screen.code(
            0,
            0,
            40,
            "/* opened here",
            ronoctua_syntax::Language::Rust,
            ronoctua_syntax::State::fresh(),
        );
        assert!(!after.is_fresh(), "the open comment was not handed on");
        let done = screen.code(
            1,
            0,
            40,
            "*/ let x = 1;",
            ronoctua_syntax::Language::Rust,
            after,
        );
        assert!(done.is_fresh());
    }

    /// FAILS IF: a piece past the budget is emitted, and a buffer's own line
    /// leaks off the surface.
    #[cfg(feature = "syntax")]
    #[test]
    fn a_coded_line_stops_at_its_budget() {
        let mut screen: Screen<crate::Tokens> = Screen::new(&context(40, 12));
        screen.code(
            0,
            0,
            8,
            "let a = 1; let b = 2; let c = 3;",
            ronoctua_syntax::Language::Rust,
            ronoctua_syntax::State::fresh(),
        );
        let list = screen.as_protocol();
        let heights = heights();
        assert!(
            list.nothing_is_covered(heights, 8).is_ok(),
            "something ran past the budget: {:?}",
            list.commands
        );
    }
}
