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

// ---------------------------------------------------------------------------
// THE HINT LINE — one sentence, two registers, one decision
// ---------------------------------------------------------------------------

/// What a run of a hint line IS, which is the same as what colour it is.
///
/// Four and not two, and every one of them was asked for from the screen.
/// `Key` and `Prose` are the original pair — what the eye hunts for and what
/// it means. `Separator` is the dot between one shortcut and the next: the
/// owner asked for it to differ from both, and punctuation drawn in the tone
/// of the words it separates is read as a word. `Alternate` is a key whose
/// MEANING follows the section that has the keyboard — *"faça com que esses
/// atalhos que mudam de acordo com a seção em foco tenham cores diferentes dos
/// demais atalhos listados"*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register {
    /// A key notation that means the same thing wherever one is.
    Key,
    /// A key notation whose meaning is the focused section's, and which
    /// therefore changes under the reader without the reader pressing anything.
    Alternate,
    /// The words around a key.
    Prose,
    /// The mark between one shortcut and the next.
    Separator,
}

impl Register {
    /// The one place that says what colour a run of a hint is.
    #[must_use]
    pub const fn color(self) -> ronoctua_tokens::Color {
        match self {
            Self::Key => tokens::color::TEXT_SUBTLE_STRONG,
            Self::Alternate => tokens::color::TEXT_SUBTLE_ALTERNATE,
            Self::Prose => tokens::color::TEXT_SUBTLE_DEFAULT,
            Self::Separator => tokens::color::TEXT_SUBTLE_FAINT,
        }
    }
}

/// One run of a hint line: the text, and which register it belongs to.
///
/// A hint reads "type to filter · C-m action menu", and the parts of that
/// answer different questions: the key is what the eye hunts for, the prose is
/// what it means, the dot is neither. Where a sentence is one `.ftl` value the
/// split cannot happen in Rust — the ORDER is the language's, and what comes
/// back from the localiser is the only thing that knows where it put the
/// argument (`ronoctua_i18n::Marked`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// The run, exactly as it reads.
    pub text: String,
    /// Which of the four registers it is drawn in.
    pub register: Register,
}

impl Piece {
    /// A run of prose.
    #[must_use]
    pub fn prose(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            register: Register::Prose,
        }
    }

    /// A key notation, fixed or following the focused section.
    #[must_use]
    pub fn key(text: impl Into<String>, alternate: bool) -> Self {
        Self {
            text: text.into(),
            register: if alternate {
                Register::Alternate
            } else {
                Register::Key
            },
        }
    }

    /// The mark between one shortcut and the next, with its air on both sides.
    ///
    /// # Why the spaces travel with the dot
    ///
    /// Because they are the dot's air and nobody else's. Written into the
    /// neighbouring runs they would be drawn in the neighbour's colour, which
    /// costs nothing today and would be a bug the first time a register grows
    /// a background. And they are what keeps `·` off the words: the owner's
    /// own photograph of the seam that lost its space says `facts· TAB`.
    #[must_use]
    pub fn separator() -> Self {
        Self {
            text: format!(" {} ", tokens::mark::SEPARATOR),
            register: Register::Separator,
        }
    }
}

/// How a key is DRAWN, as against how it is written down.
///
/// # Why this is not `Key::notation`
///
/// Because they answer different questions and only one of them is read by a
/// person. `notation` is the wire form: `<down>`, `S-<left>` — round-trippable
/// through `Key::parse`, which is what a table and a `.ftl` need. On screen it
/// is six cells of markup, and a line that teaches forty-one keys would spend
/// twenty-four of them drawing angle brackets.
///
/// So the arrows are drawn as arrows, out of `mark::ARROW_*`, and everything
/// else keeps the notation it already had. One place, for the reason
/// `syntax_color` gives one module down: two places would answer differently
/// and the difference would be a shortcut printed one way in the hint and
/// another in the manual.
///
/// See [`split_at_the_modifier`] for the other half of "how a key is drawn":
/// the shaper's ligatures, which turned one of these into a key that does not
/// exist.
#[must_use]
pub fn key_text(t: &ronoctua_keymap::Key) -> String {
    use ronoctua_keymap::Sym;
    let arrow = match t.sym {
        Sym::Left => Some(tokens::mark::ARROW_LEFT),
        Sym::Right => Some(tokens::mark::ARROW_RIGHT),
        Sym::Up => Some(tokens::mark::ARROW_UP),
        Sym::Down => Some(tokens::mark::ARROW_DOWN),
        _ => None,
    };
    let Some(a) = arrow else {
        return t.notation();
    };
    // THE MODIFIERS STAY AS THEY ARE WRITTEN. `S-` is two cells and is the
    // same two cells everywhere else in the system; only the SYMBOL was
    // markup, and only the symbol changes. The four are the only syms
    // `notation` writes inside angle brackets, so the last `<` is the seam —
    // and cutting at a MEASURED position rather than a remembered length is
    // what keeps `<up>` (four bytes) and `<right>` (seven) from needing to be
    // listed here a second time.
    let mut out = t.notation();
    let Some(at) = out.rfind('<') else {
        return out;
    };
    out.truncate(at);
    out.push(a);
    out
}

impl Piece {
    /// Appends one localised sentence to a hint line that is being built.
    ///
    /// # Where this is still used, now that the manager's line is a table
    ///
    /// In every line that is a SENTENCE with a key inside it — the drawer's
    /// own teaching line, and whatever else grows one. The file manager's hint
    /// stopped being a sentence: it is one shortcut per chunk, in the order of
    /// the key table, and there the separator and the order are the code's.
    ///
    /// # What makes a run a key
    ///
    /// Being an interpolated argument is not enough: `files-panel-no-room`
    /// interpolates a NUMBER, and a number painted in the key's colour would
    /// be a lie about what to press. So a run is a key when the localiser says
    /// it is an argument AND its text is one of the notations this caller
    /// passed in. A value that grows a numeric argument corrects itself.
    pub fn extend(out: &mut Vec<Self>, m: &ronoctua_i18n::Marked, notations: &[&str]) {
        if !out.is_empty() {
            out.push(Self::prose(" "));
        }
        for (text, arg) in m.pieces() {
            if text.is_empty() {
                continue;
            }
            let key = arg && notations.contains(&text);
            out.push(if key {
                Self::key(text, false)
            } else {
                Self::prose(text)
            });
        }
    }
}

/// Where a key notation is CUT so a ligature cannot cross the modifier.
///
/// # The defect, photographed
///
/// `C->` came out on screen as `C-→`. The face ligates `->` — that is what the
/// feature is for, and in a line of code it is right — but here it turns a
/// notation into a DIFFERENT key, and worse into one this system forbids: an
/// arrow held with `Ctrl` is the footer's paging chord, taken before the view
/// ever sees it. The line exists to teach forty keys, and it was teaching one
/// that does not exist.
///
/// It was invisible until the line began to show the whole table — `C->` had
/// been bound for as long as the table has and drawn nowhere.
///
/// # Why the cut, and not a different key
///
/// Because the key is not wrong, the DRAWING is. `cosmic-text` shapes a run at
/// a time and exposes no way to switch a feature off, so the fix is to stop
/// the two characters being in one run: a hint's every glyph is one cell wide
/// and placed by this module's own arithmetic, so two runs land exactly where
/// one did.
///
/// And it is done for EVERY key rather than for `>`: which pairs a face
/// ligates is the face's business and changes when a face does. `C-` `>` and
/// `C-` `[` shape identically once they are apart.
fn split_at_the_modifier(text: &str) -> Option<(&str, &str)> {
    // THE MODIFIERS ARE EATEN FROM THE FRONT, and the rest is the symbol.
    //
    // Not `rfind('-')`, which is the same answer for every notation in the
    // table except one: `C--` is the film's rewind, and the LAST hyphen there
    // is the symbol itself — cutting after it leaves nothing on the right and
    // the two hyphens stay in one run. That is harmless against this face,
    // which does not ligate `--`, and "harmless against this face" is not a
    // rule. `Key::notation` builds the prefix out of exactly these four, so
    // reading them back is the same arithmetic in reverse.
    //
    // By `strip_prefix` and never by slicing: `key_text` has already replaced
    // an arrow's symbol with a multi-byte character, and `&text[at..at + 2]`
    // on `S-←` would cut a codepoint in half.
    let mut rest = text;
    let mut at = 0;
    while let Some(next) = rest
        .strip_prefix("C-")
        .or_else(|| rest.strip_prefix("M-"))
        .or_else(|| rest.strip_prefix("S-"))
        .or_else(|| rest.strip_prefix("s-"))
    {
        at += 2;
        rest = next;
    }
    // A CUT THAT LEAVES NOTHING IS NOT A CUT: `RET` has no modifier, and a
    // notation that is nothing BUT modifiers has no symbol to separate.
    (at > 0 && !rest.is_empty()).then(|| text.split_at(at))
}

/// Draws a hint line, and is the ONLY place that says what colour one is.
///
/// # Why it is here and not in each door
///
/// Because it was in each door, and that is precisely what the owner
/// photographed: the same line came out `text.primary` in the footer and
/// `text.secondary` in the window. Neither was a decision — the window wrote a
/// token at its push site, and the footer inherited one from the arm that
/// exists for "the user is typing in `M-x`", which the hint had been hung off
/// years later. Two doors, two answers, and nothing anywhere that could be
/// called wrong, which is the shape of defect this whole module exists to end.
///
/// # The elision happens once, across the pieces
///
/// The window elided and the footer did not, so the footer's line ran off the
/// screen with no `…` to say it had been cut. Cutting per piece would put an
/// ellipsis in the middle of the sentence, so the walk carries the room that
/// is left and only the piece that overruns is cut.
///
/// `room` is how many cells the line may occupy starting at `column`.
#[must_use]
pub fn hint_spans(
    pieces: &[Piece],
    size: ronoctua_render::Size,
    column: u16,
    line: u16,
    room: u16,
) -> Vec<ronoctua_render::Span> {
    let mut out = Vec::with_capacity(pieces.len());
    let mut at = column;
    let mut left = room;
    for p in pieces {
        if left == 0 {
            break;
        }
        // A KEY IS DRAWN IN TWO RUNS, so that no ligature can cross its
        // modifier — see `split_at_the_modifier`. Prose is one run: a sentence
        // is what the shaper is FOR.
        let runs: Vec<&str> = match p.register {
            Register::Key | Register::Alternate => split_at_the_modifier(&p.text)
                .map_or_else(|| vec![p.text.as_str()], |(a, b)| vec![a, b]),
            Register::Prose | Register::Separator => vec![p.text.as_str()],
        };
        for run in runs {
            if left == 0 {
                break;
            }
            // `layout::width`, which is the ruler the rest of both doors
            // position by: it counts CLUSTERS. An emoji advances more than one
            // cell and this would place the next run early — but every other
            // span on the line is placed the same way, and a hint that used a
            // second ruler would be the only thing on screen that did. No hint
            // value contains an emoji; if one ever does, the fix belongs to the
            // grid.
            let width = ronoctua_render::layout::width(run);
            let text = if width <= left {
                run.to_owned()
            } else {
                ronoctua_render::layout::elide_end(run, left)
            };
            let drawn = ronoctua_render::layout::width(&text);
            out.push(ronoctua_render::Span {
                size,
                text,
                column: at,
                line,
                color: p.register.color(),
                edge: false,
                foot: false,
            });
            at = at.saturating_add(drawn);
            left = left.saturating_sub(drawn);
        }
    }
    out
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
    use super::{
        FIELDS_LEAD, Frame, INSET, Piece, Register, cell_width, content_left_px, hint_spans,
        key_text, split_at_the_modifier,
    };
    use ronoctua_tokens as tokens;

    fn piece(text: &str, key: bool) -> Piece {
        if key {
            Piece::key(text, false)
        } else {
            Piece::prose(text)
        }
    }

    #[test]
    fn the_key_and_the_prose_are_two_colours_and_neither_is_a_content_colour() {
        // THE OWNER'S REPORT, in one assertion: the two halves of the line
        // used to be the same tone, and the whole line used to be one of the
        // tones the listing under it is drawn in.
        let line = [piece("type to filter · ", false), piece("C-m", true)];
        let spans = hint_spans(&line, ronoctua_render::Size::Grid, 2, 0, 80);
        // THREE SPANS FOR TWO PIECES: the key is drawn in two runs, both in
        // the key's colour — see `split_at_the_modifier`.
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].color, tokens::color::TEXT_SUBTLE_DEFAULT);
        assert_eq!(spans[1].color, tokens::color::TEXT_SUBTLE_STRONG);
        assert_eq!(spans[2].color, tokens::color::TEXT_SUBTLE_STRONG);
        assert_ne!(spans[0].color, spans[1].color);
        for s in &spans {
            for content in [
                tokens::color::TEXT_PRIMARY,
                tokens::color::TEXT_SECONDARY,
                tokens::color::TEXT_DIM,
            ] {
                assert_ne!(s.color, content, "the hint is wearing a content colour");
            }
        }
    }

    #[test]
    fn no_ligature_crosses_a_keys_modifier() {
        // THE DEFECT, PHOTOGRAPHED: `C->` came out on screen as `C-→`. The
        // face ligates `->` — right in a line of code — and here it turned a
        // notation into a key this system FORBIDS, since an arrow held with
        // `Ctrl` is the footer's paging chord and never reaches the view. It
        // was invisible until the hint began to teach the whole key table.
        //
        // The shaper works a RUN at a time, so what this asserts is that the
        // two characters are never in one run.
        for notation in [
            "C->", "C-<", "C--", "C-=", "S-TAB", "C-SPC", "RET", "S-<left>",
        ] {
            let t = ronoctua_keymap::Key::parse(notation).expect("a notation");
            let drawn = key_text(&t);
            let line = [Piece::key(drawn.clone(), false)];
            let spans = hint_spans(&line, ronoctua_render::Size::Grid, 0, 0, 40);
            // AGAINST THE DRAWN FORM and not against the notation: `S-<left>`
            // is drawn `S-←`, which is `key_text`'s other half.
            let rebuilt: String = spans.iter().map(|s| s.text.as_str()).collect();
            assert_eq!(rebuilt, drawn, "the key did not survive the cut");
            for s in &spans {
                assert!(
                    !s.text.contains("->"),
                    "`{notation}` still has `->` inside one run: {:?}",
                    s.text
                );
            }
        }
        // AND A KEY WITH NO MODIFIER IS ONE RUN, because there is nothing for a
        // ligature to cross.
        assert!(split_at_the_modifier("RET").is_none());
        assert!(split_at_the_modifier("-").is_none());
        assert!(split_at_the_modifier("C-").is_none());
        // THE SYMBOL CAN BE A HYPHEN, and it is: `C--` is the film's rewind.
        // Read from the RIGHT this one comes back whole, which is the cut that
        // does nothing — harmless against a face that does not ligate `--`,
        // and harmlessness is not a rule.
        assert_eq!(split_at_the_modifier("C--"), Some(("C-", "-")));
        assert_eq!(split_at_the_modifier("S-TAB"), Some(("S-", "TAB")));
        // AND AN ARROW IS MULTI-BYTE, which is why nothing here slices.
        let arrow = format!("S-{}", ronoctua_tokens::mark::ARROW_LEFT);
        assert_eq!(
            split_at_the_modifier(&arrow).map(|(a, _)| a.to_owned()),
            Some("S-".to_owned())
        );
    }

    #[test]
    fn each_piece_starts_where_the_one_before_it_ended() {
        // The pieces are separate spans on ONE line, so the column each starts
        // at is the sum of the widths before it. Get that wrong and the key
        // draws on top of the prose — which is what a single span never had to
        // worry about.
        let line = [
            piece("type to filter · ", false),
            piece("C-m", true),
            piece(" opens the menu", false),
        ];
        let spans = hint_spans(&line, ronoctua_render::Size::Grid, 2, 0, 80);
        // FOUR SPANS FOR THREE PIECES: a key is drawn in two runs so that no
        // ligature can cross its modifier — see `split_at_the_modifier`.
        assert_eq!(spans.len(), 4);
        assert_eq!(spans[0].column, 2);
        assert_eq!(spans[1].column, 2 + 17);
        assert_eq!(spans[2].column, 2 + 17 + 2);
        assert_eq!(spans[3].column, 2 + 17 + 3);
        // And the line they draw is the sentence, in order.
        let rebuilt: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(rebuilt, "type to filter · C-m opens the menu");
    }

    #[test]
    fn a_hint_too_long_for_the_room_is_cut_once_at_the_end() {
        // The footer never cut this line at all, so it ran off the screen with
        // nothing saying it had been. Cutting per piece would put an ellipsis
        // in the middle of the sentence.
        let line = [
            piece("type to filter · ", false),
            piece("C-m", true),
            piece(" opens the action menu", false),
        ];
        let spans = hint_spans(&line, ronoctua_render::Size::Grid, 0, 0, 22);
        let drawn: u16 = spans
            .iter()
            .map(|s| ronoctua_render::layout::width(&s.text))
            .sum();
        assert!(drawn <= 22, "{drawn} cells drawn in room for 22");
        let cut = spans.iter().filter(|s| s.text.ends_with('…')).count();
        assert_eq!(cut, 1, "the line has to be cut exactly once");
        assert!(
            spans.last().is_some_and(|s| s.text.ends_with('…')),
            "the cut has to be at the END of the line"
        );
    }

    #[test]
    fn no_room_draws_nothing_rather_than_one_cell_of_ellipsis() {
        let line = [piece("type to filter", false)];
        assert!(hint_spans(&line, ronoctua_render::Size::Grid, 0, 0, 0).is_empty());
    }

    #[test]
    fn the_sentences_are_joined_by_exactly_one_space() {
        // The join used to be written at each call site and one of the three
        // was missing its space: the owner's photograph reads `scrolls the
        // facts· TAB goes to`.
        // The manager's own hint no longer goes through here — it is one
        // shortcut per chunk now, and the seam is `Piece::separator`. What is
        // still assembled this way is every line that is a SENTENCE with a key
        // inside it, and those are the two used here.
        let t = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let mut out = Vec::new();
        let a = t.t_marked(ronoctua_i18n::key::FILES_SIDEBAR_ONLY, &[("key", "C-l")]);
        let b = t.t_marked(
            ronoctua_i18n::key::MINIBUFFER_FOLD_HINT,
            &[("keys", "RCtrl")],
        );
        Piece::extend(&mut out, &a, &["C-l"]);
        let seam = out.len();
        Piece::extend(&mut out, &b, &["RCtrl"]);
        assert_eq!(out[seam].text, " ");
        let line: String = out.iter().map(|p| p.text.as_str()).collect();
        assert!(!line.contains("·  "), "two spaces at a seam: {line}");
        assert!(
            !line.contains("e·"),
            "a seam with no space before the separator: {line}"
        );
    }

    #[test]
    fn an_argument_that_is_not_a_key_is_not_painted_as_one() {
        // `files-panel-no-room` interpolates a NUMBER. Painted in the key's
        // colour it would be a lie about what to press, so being an
        // interpolated argument is not enough — the run has to be one of the
        // notations the caller passed in.
        let t = ronoctua_i18n::Texts::fresh("en-GB").unwrap();
        let m = t.t_marked(ronoctua_i18n::key::FILES_PANEL_NO_ROOM, &[("cells", "24")]);
        let mut out = Vec::new();
        Piece::extend(&mut out, &m, &["C-m", "C-v"]);
        assert!(
            out.iter()
                .all(|p| !matches!(p.register, Register::Key | Register::Alternate)),
            "a number came out as a key"
        );
        assert!(out.iter().any(|p| p.text.contains("24")));
    }

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
