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
    /// A one-line rule.
    Ruled { line: u16 },
    /// The band under a focused row.
    Banded { line: u16 },
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
        // CHARACTERS AND NOT BYTES. A cell holds one character, and an accent
        // is two bytes: charging bytes would put every run after the first
        // accented one to the right of where it is drawn.
        let wide = u16::try_from(piece.chars().count()).unwrap_or(u16::MAX);
        if wide == 0 || cell >= room {
            return cell.saturating_add(wide);
        }
        let take = usize::from(room - cell);
        let shown: alloc::string::String = piece.chars().take(take).collect();
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
        self.parts.push(Part::Ruled { line });
    }

    /// The band under a list's focused row.
    ///
    /// A change of colour, not a box and not an outline (invariant 15), which
    /// is why it has neither width nor column either.
    pub fn band(&mut self, line: u16) {
        self.parts.push(Part::Banded { line });
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
    /// The cell is `font.size.large × font.lineHeight = 20 × 1.45 = 29.0 px`.
    /// The font's box is `1.25 em = 25 px` (ascender 0.965 + descender 0.285),
    /// so every run already carries `(29.0 − 25.0) / 2 = 2.0 px` of slack above
    /// its ink and 2.0 below it. Then:
    ///
    /// - above the title's ink: one empty cell plus the title's own top slack —
    ///   `29.0 + 2.0 = 31.0 px`;
    /// - below it, down to the first content ink: the title's bottom slack plus
    ///   one empty cell — `2.0 + 29.0 = 31.0 px`.
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
    /// whitespace, which is what `colors.nix` prescribes and invariant 15
    /// leaves as the only option. At the body's size it also costs ONE cell
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
                Part::Ruled { line } => Command::Rule {
                    line: *line,
                    color: Tokens::color(Role::Rule),
                },
                Part::Banded { line } => Command::Selection {
                    line: *line,
                    color: Tokens::color(Role::Band),
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
        let list = self.as_protocol();
        list.nothing_is_covered(self.heights, self.columns)?;
        list.nothing_overlaps(self.heights, self.columns)
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
            Part::Ruled { line } => Frame::Ruled(*line),
            Part::Banded { line } => Frame::Banded(*line),
        })
    }
}

/// A part, borrowed, for the guest's conversion.
#[cfg(feature = "guest")]
pub(crate) enum Frame<'a, I> {
    Ran(u16, &'a Run<I>),
    #[cfg(feature = "syntax")]
    Coded(u16, u16, &'a str, Option<ronoctua_syntax::Role>),
    Ruled(u16),
    Banded(u16),
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
