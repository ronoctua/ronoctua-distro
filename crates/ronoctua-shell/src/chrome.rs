//! Where content begins, on every surface this shell draws.
//!
//! # Why this module exists
//!
//! Because the same view was drawn at two different distances from the screen's
//! edge depending on which door it came through, and nothing anywhere said so.
//! The owner reported it from the screen: *"when something new is drawn in the
//! minibuffer region it comes out glued to the edge and has different spacing
//! from the same thing in its own window; there is no standard at all."*
//!
//! He was right, and the reason was that there was no single number to be
//! right about. The footer's expanded region indented its field rows by
//! `space.files.gutter` — one cell, which is the MARK's cell and not a margin
//! at all — while both own-window doors added `space.content.inset` minus that
//! same gutter on top. One cell against two — and at the time the two doors
//! did not even have the same cell: the footer drew at `font.size.base` and a
//! window at `font.size.large`, ten point eight pixels against twelve.
//!
//! # The rule, in one sentence
//!
//! **Content is inset by [`INSET`] cells on every surface. The modeline and the
//! minibuffer are not content and keep `space.modeline.edgeLeft`.**
//!
//! That exception is not a leak in the rule, it is the rule's other half, and
//! `space.nix` argues it where the token lives: those two lines are a status
//! line — peripheral reference one glances at — and two cells there "would
//! start to look like a margin". Everything else on screen is something one
//! reads, and it gets the inset.
//!
//! **The exception has exactly four sites, and they are named here so a fifth
//! has to be argued for**: the minibuffer's line (`surface.rs`, the one
//! `column: MODELINE_EDGELEFT` left in that file), the modeline's segments
//! (`ronoctua_render::layout`), the notification band (`band.rs`) and the
//! brain's line (`brain.rs`). The band is on the list because it is the same
//! kind of thing as the modeline — one line of peripheral reference, above the
//! pair, that one glances at and does not read; the brain's line is the same
//! kind of thing one row below, and it was argued for on exactly those terms.
//! The notification MANAGER is not: it is a list in the expanded region, and
//! it takes the inset like every other list.
//!
//! # What is left over, and why it is nothing
//!
//! Nothing is left over, and a fifth of a cell used to be: two cells of
//! `font.size.base` are 21,6 px and two of `font.size.large` were 24,0 px.
//! That residue belonged to the GRID and not to the layout, because the two
//! doors drew text at different sizes on purpose, and the test kept it inside
//! one cell so it could not grow back into a factor of two.
//!
//! The owner has since asked for ONE size across the system — see [`SIZE`] —
//! so the residue is zero and
//! [`tests::both_doors_put_content_in_the_same_place`] says so exactly, with
//! no tolerance at all.

use ronoctua_tokens as tokens;

/// How far in content starts, in cells, on every surface.
///
/// One number, and the reason it is re-exported here rather than read from
/// `tokens::space` at each site is the module header: the sites that got this
/// wrong got it wrong by reaching for a DIFFERENT token, and a name that says
/// `chrome::INSET` is a name somebody has to deliberately not use.
pub const INSET: u16 = tokens::space::CONTENT_INSET;

/// The inset owed by a row that already carries its own first cell.
///
/// A file manager's row, a notification's row and a transfer's row all begin
/// at `space.files.gutter` — the mark's cell, and the air an icon needs so its
/// glyph does not touch the border and read as clipped. That cell is part of
/// the TABLE, not part of the margin, so a surface drawing such a row owes the
/// difference and not the whole inset.
///
/// Both windows worked this out for themselves and agreed; the footer never
/// did it at all, which is exactly the defect. One constant, three readers.
pub const FIELDS_LEAD: u16 = INSET.saturating_sub(tokens::space::FILES_GUTTER);

/// The size everything this shell draws is drawn at.
///
/// # Why there is one of these
///
/// There were two. The footer drew at `font.size.base` and every window of
/// ours at `font.size.large`, and a `Door` enum existed so that a test could
/// say the doors differed in that one thing and nothing could quietly add a
/// second difference. The difference itself was deliberate — "the footer is
/// peripheral reference, the buffer is what one reads".
///
/// The owner photographed the file manager's two doors side by side and asked
/// for the footer's size everywhere, buffers included: it is the one he reads
/// all day, and a system whose two halves are 10% apart looks assembled out of
/// parts of different origins. So the enum had nothing left to name and went,
/// and this is what replaced it — which is smaller than the enum was, and is
/// the point.
pub const SIZE: f32 = tokens::font::SIZE_BASE;

/// How wide a cell is, in pixels.
///
/// Every hit test on every surface comes through here or through
/// [`Frame::cell`]. One answer, so a site that reached for a font size
/// directly would be a second grid nobody named.
#[must_use]
pub fn cell_width() -> f32 {
    tokens::metrics::cell_width(SIZE)
}

/// How many pixels from the surface's left edge a row of fields begins.
///
/// # Why this is behind `cfg(test)`
///
/// Because nothing in the drawing needs it, and that is the point rather than
/// an omission. The layout is integer arithmetic over the CELL — the whole
/// footer's one-frame budget is bought with that — so production counts cells
/// and never pixels.
///
/// The pixel value exists for exactly one question: whether the surfaces put
/// content in the same place. With one size that is now arithmetic rather than
/// a measurement, and the test that asks it says so.
#[cfg(test)]
#[must_use]
pub fn content_left_px() -> f32 {
    f32::from(FIELDS_LEAD + tokens::space::FILES_GUTTER) * cell_width()
}

// ---------------------------------------------------------------------------
// THE FRAME — where a surface ends, in the two units it is measured in
// ---------------------------------------------------------------------------

/// A surface's measurements, carried instead of recomputed.
///
/// # Why this exists
///
/// Because the same numbers were worked out at each site that needed them, and
/// the sites disagreed. Two units are in play and both are legitimate:
///
/// - **the grid** — `u16` cells and lines, which is where TEXT goes
///   (`ronoctua_render::Span` carries a `column` and a `line` and nothing else);
/// - **the surface** — `f32` pixels, which is where EDGES go: rules, dividers,
///   hover bands, a scrollbar's track and thumb, a picture's box.
///
/// They coincide only when the surface is an exact multiple of the cell and the
/// line, and it never is: `columns` is `floor(width / cell)`, so a remainder of
/// less than one cell sits at the right border and less than one line at the
/// bottom. Every geometry defect this file has been asked to fix lived in that
/// remainder — a rule that stopped short, a scrollbar drawn past the border, a
/// strip of blank pixels under a panel that changed size with the window.
///
/// So the conversion happens **here**, once, and the two units stop being a
/// convention that each site has to keep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The surface, in pixels. Never a count of anything.
    pub width: f32,
    /// The surface's height, in pixels. This is the number the compositor gave
    /// and the shell may not argue with it (invariant 4).
    pub height: f32,
    /// The region's first line, in the grid.
    pub first: u16,
}

impl Frame {
    /// A frame at this surface.
    #[must_use]
    pub const fn of(width: f32, height: f32, first: u16) -> Self {
        Self {
            width,
            height,
            first,
        }
    }

    /// The arithmetic, which lives in the design system.
    ///
    /// # Why the conversion is not written here any more
    ///
    /// Because it is not this door's, nor this shell's: it is the same
    /// conversion for the footer, for each window, and for anything drawn
    /// later. It was written out here and had begun to be written out again
    /// beside its callers — which is the shape every defect in this file
    /// started as.
    ///
    /// What stays on this side is what a cell and a line MEASURE.
    /// `ronoctua-design` takes those two as values because it has no tokens
    /// and may not have any (invariant 11).
    fn rail(self) -> ronoctua_design::rails::Frame {
        ronoctua_design::rails::Frame::of(
            cell_width(),
            tokens::metrics::line_height(SIZE),
            self.width,
            self.height,
            self.first,
        )
    }

    /// How wide a cell is, in pixels.
    #[must_use]
    pub fn cell(self) -> f32 {
        self.rail().cell
    }

    /// How tall a line is, in pixels.
    #[must_use]
    pub fn line(self) -> f32 {
        self.rail().line
    }

    /// How many whole cells the surface holds.
    #[must_use]
    pub fn columns(self) -> u16 {
        self.rail().columns()
    }

    /// How many whole lines the surface holds.
    #[must_use]
    pub fn lines(self) -> u16 {
        self.rail().lines()
    }

    /// The pixel a column begins at, counted from the LEFT.
    #[must_use]
    pub fn x_of(self, column: u16) -> f32 {
        self.rail().x_of(column)
    }

    /// The pixel a line begins at.
    #[must_use]
    pub fn y_of(self, line: u16) -> f32 {
        self.rail().y_of(line)
    }

    /// The surface's bottom edge, in pixels. The TRUE one.
    #[must_use]
    pub const fn bottom(self) -> f32 {
        self.height
    }

    /// Which column a pixel is in.
    #[must_use]
    pub fn column_at(self, x: f32) -> u16 {
        self.rail().column_at(x)
    }

    /// Which line a pixel is on, or `None` above the surface.
    #[must_use]
    pub fn line_at(self, y: f32) -> Option<u16> {
        self.rail().line_at(y)
    }
}

// ---------------------------------------------------------------------------
// SEPARATORS — the second defect the owner photographed, and the same cause
// ---------------------------------------------------------------------------

/// A rule across the whole surface.
///
/// # Why this takes the surface's WIDTH and not a rectangle
///
/// Because every site that got this wrong got it wrong the same way, and the
/// shape of the mistake is worth naming: they wrote `x: margin * cell` and
/// `width: self.width`. That is an inset on the left and the FULL width on the
/// right — a rule that starts one cell in, runs past the right edge and is
/// clipped there. On screen it reads as a line glued to the right border with
/// a gap on the left, which is exactly what the owner reported and what
/// `12-update-window.png` shows at `y=377`, starting at `x=12`.
///
/// The owner's rule is *"sangrada, de borda a borda"* — bled, edge to edge —
/// and it is decided once, here. A caller hands in how wide the surface is and
/// which line the rule sits on; it cannot express an inset, because there is
/// no parameter for one.
///
/// The Y is the caller's, in pixels, because the Y was never the defect: every
/// one of these sat on the right row and started in the wrong column.
#[must_use]
pub fn rule(width_px: f32, y_px: f32) -> ronoctua_render::quad::Quad {
    ronoctua_render::quad::Quad {
        x: 0.0,
        y: y_px,
        width: width_px,
        height: f32::from(tokens::space::RULE_THICKNESS),
        color: tokens::color::RULE_DEFAULT,
    }
}

/// A vertical separator between two columns, from a top edge to the surface's
/// BOTTOM.
///
/// # Why it takes the bottom and not a height
///
/// Same defect, turned ninety degrees. The five sites that drew one computed
/// `height: lines * line_height`, and a line height is fractional — 26,5 px
/// here — so the product lands short of where the surface actually ends. In
/// `10-manager-with-preview.png` all three dividers stop at `y=798` on an
/// 800-pixel surface, and on the owner's own screen the shortfall is a whole
/// row: *"the vertical line that separates the preview from the info is also
/// not touching the bottom of the screen, which is wrong"*.
///
/// A count of lines is a guess at where the bottom is. The bottom is not a
/// guess, so this asks for it.
#[must_use]
pub fn divider(x_px: f32, from_px: f32, bottom_px: f32) -> ronoctua_render::quad::Quad {
    ronoctua_render::quad::Quad {
        x: x_px,
        y: from_px,
        width: f32::from(tokens::space::RULE_THICKNESS),
        height: (bottom_px - from_px).max(0.0),
        color: tokens::color::RULE_DEFAULT,
    }
}

/// What colour a syntax role is drawn in.
///
/// # Why the map lives HERE, and why there is one of it
///
/// Because both of its callers are in this crate — the clipboard's preview in
/// `surface.rs` and the file manager's preview in `files.rs` — and two copies
/// would be the two panels colouring the same Rust differently. That is the
/// defect `Section` and `Field::covers` were each written to end, and this
/// file is already the one allowed to make a visual decision
/// (`separators-come-from-chrome`).
///
/// And it does not live in `ronoctua-syntax`: that crate is `no_std` and
/// receives no token VALUES, which is the same wall invariant 11 draws around
/// a wasm buffer. It gives back a role; turning a role into a colour is the
/// business of whoever draws.
///
/// # Fourteen roles, twelve tokens
///
/// `Constant` shares `number` and `Heading` shares `attribute`, and they stay
/// two roles each for the reason `ronoctua_design::Palette` gives about
/// `Quiet` and `Dim`: the day the theme parts them, the change is one line
/// here rather than a hunt through every call site.
///
/// The `match` is exhaustive, so a role added to the crate is a compile error
/// in exactly one file — which is the property that makes the wall worth
/// having.
#[must_use]
pub(crate) const fn syntax_color(role: ronoctua_syntax::Role) -> tokens::Color {
    use ronoctua_syntax::Role;

    match role {
        Role::Comment => tokens::color::SYNTAX_COMMENT,
        Role::String => tokens::color::SYNTAX_STRING,
        Role::Escape => tokens::color::SYNTAX_ESCAPE,
        Role::Number | Role::Constant => tokens::color::SYNTAX_NUMBER,
        Role::Keyword => tokens::color::SYNTAX_KEYWORD,
        Role::Type => tokens::color::SYNTAX_TYPE,
        Role::Function => tokens::color::SYNTAX_FUNCTION,
        Role::Attribute | Role::Heading => tokens::color::SYNTAX_ATTRIBUTE,
        Role::Property => tokens::color::SYNTAX_PROPERTY,
        Role::Tag => tokens::color::SYNTAX_TAG,
        Role::Added => tokens::color::SYNTAX_ADDED,
        Role::Removed => tokens::color::SYNTAX_REMOVED,
    }
}

#[cfg(test)]
mod tests {
    use super::{FIELDS_LEAD, Frame, INSET, cell_width, content_left_px};
    use ronoctua_tokens as tokens;

    #[test]
    fn the_residue_is_named_and_it_is_smaller_than_one_step() {
        // THE NUMBER THAT HAD NO NAME.
        //
        // `columns` is `floor(width / cell)` and `lines` is `floor(height /
        // line)`, so between the last whole cell and the border there is a
        // remainder, and the same at the bottom. It is never zero for long: it
        // is a function of the surface, and the compositor picks the surface.
        //
        // Nameless, it became a strip of blank pixels under the manager's facts
        // panel that grew and shrank as the window was resized — the owner
        // photographed it three times before it had a name.
        for width in [640.0_f32, 800.0, 1280.0, 1366.0, 1920.0] {
            for height in [200.0_f32, 747.0, 800.0, 1080.0, 1200.5] {
                let f = Frame::of(width, height, 1);
                let over_x = f.width - f.x_of(f.columns());
                let over_y = f.height - f.y_of(f.lines());
                assert!(
                    over_x >= 0.0 && over_x < f.cell(),
                    "{width}x{height}: horizontal residue of {over_x} in a cell of {}",
                    f.cell()
                );
                assert!(
                    over_y >= 0.0 && over_y < f.line(),
                    "{width}x{height}: vertical residue of {over_y} in a line of {}",
                    f.line()
                );
            }
        }
    }

    #[test]
    fn the_border_is_reached_from_the_border_and_never_by_counting() {
        // WHAT COUNTING GETS YOU. A run placed at the last cell counted from
        // the origin stops `residue_x` short of the border; the same run placed
        // FROM the border lands on it exactly. Both are legitimate — one is for
        // text read left to right, the other for anything that must sit against
        // the edge — and the defect was never having said which is which.
        let f = Frame::of(1366.0, 768.0, 1);
        // THE LAST VALID CELL is `columns() - 1`; `columns()` is one past it,
        // which is the off-by-one this very assertion caught while being
        // written.
        let counted = f.x_of(f.columns().saturating_sub(1)) + f.cell();
        let anchored = f.width - f.cell() + f.cell();
        assert!(
            (anchored - f.width).abs() < f32::EPSILON,
            "anchored to the edge ended at {anchored} and the edge is {}",
            f.width
        );
        assert!(
            counted <= f.width,
            "counted from the origin it went past the edge: {counted} > {}",
            f.width
        );
        // And the difference between them IS the residue — said once, so
        // nobody has to rediscover it from a screenshot.
        let over = f.width - f.x_of(f.columns());
        assert!(
            ((f.width - counted) - over).abs() < 0.01,
            "the difference between the two rails is not the residue"
        );
    }

    #[test]
    fn a_point_lands_on_the_cell_it_was_drawn_in() {
        // THE INVERSE PAIR. `x_of`/`column_at` and `y_of`/`line_at` are the
        // drawing and the pointer, and they have to be each other's inverse or
        // a click lands on one thing while the screen shows another. The design
        // crate's `Grid::at`/`index_at` carry this lesson for the buffers; this
        // is the same law for the host.
        let f = Frame::of(1280.0, 800.0, 2);
        for column in 0..f.columns() {
            assert_eq!(f.column_at(f.x_of(column)), column, "column {column}");
            // AND ANYWHERE INSIDE THE CELL, not only at its first pixel.
            assert_eq!(f.column_at(f.x_of(column) + f.cell() * 0.5), column);
        }
        for line in 0..f.lines() {
            assert_eq!(f.line_at(f.y_of(line)), Some(line), "line {line}");
            assert_eq!(f.line_at(f.y_of(line) + f.line() * 0.5), Some(line));
        }
        assert_eq!(f.line_at(-1.0), None, "above the surface is not a line");
    }

    #[test]
    fn both_doors_put_content_in_the_same_place() {
        // THE DEFECT THE OWNER PHOTOGRAPHED. The footer drew a file row one
        // cell in and the window drew the same row two cells in — 10,8 px
        // against 24,0 px, a factor of 2,2, for one table.
        //
        // It used to allow a difference of up to one footer cell, because the
        // doors drew at two font sizes on purpose. They draw at one now, so
        // the tolerance is gone and the two are the SAME NUMBER — which is a
        // stronger statement than the one this test was written to make.
        let cells = FIELDS_LEAD + tokens::space::FILES_GUTTER;
        assert_eq!(cells, INSET, "the sum has to be the inset, in both places");

        // AND IN PIXELS THEY ARE NOW THE SAME NUMBER, which the old form of
        // this test could not say: a cell was a different distance in each
        // door, so it allowed a difference of up to one footer cell. Two
        // surfaces of very different shapes, one column, one pixel.
        let footer = Frame::of(1920.0, 53.0, 0);
        let window = Frame::of(900.0, 700.0, 0);
        assert!(
            (footer.x_of(INSET) - window.x_of(INSET)).abs() < f32::EPSILON,
            "content begins at {} px in one surface and {} px in the other",
            footer.x_of(INSET),
            window.x_of(INSET)
        );
        assert!((footer.cell() - window.cell()).abs() < f32::EPSILON);
        // And the two ways of arriving at that pixel agree, which is what
        // stops a third one being written beside them.
        assert!((footer.x_of(INSET) - content_left_px()).abs() < f32::EPSILON);
        assert!((cell_width() - footer.cell()).abs() < f32::EPSILON);
    }

    #[test]
    fn a_row_that_carries_its_gutter_never_pays_it_twice() {
        // The window's old constant was `CONTENT_INSET - FILES_GUTTER` written
        // out at two call sites, with a comment at each explaining the
        // subtraction. The comment was right and the duplication was the
        // problem: the footer was the third site and it simply did not have
        // one.
        assert_eq!(
            FIELDS_LEAD + tokens::space::FILES_GUTTER,
            INSET,
            "whatever already brings the gutter pays only the difference"
        );
        // And it never goes negative, whatever the two tokens become. A
        // `const` block, because both sides are constants and clippy is right
        // that a runtime assertion over two of them proves nothing at runtime
        // — this way it is the COMPILER that refuses.
        const { assert!(FIELDS_LEAD <= INSET) };
    }
}
