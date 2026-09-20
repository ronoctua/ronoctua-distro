//! The bridge between a component's `DisplayList` and the renderer's runs.
//!
//! # Why it is a module of its own, and not part of `buffer`
//!
//! Because none of it touches Wayland. What is here resolves the two things a
//! wasip2 component is forbidden to know (invariant 11) — the colour token's
//! value and the translation of a key — and turns cells into runs. It lived
//! next to the `xdg_toplevel` that draws it, which was fine until something
//! else needed to draw the same list somewhere else: the terminal already
//! borrowed `spans_of` across the module wall, and the offscreen tool that
//! writes a buffer to a PNG needs the WHOLE bridge.
//!
//! Sharing it is not tidiness. A tool that rebuilt this conversion would be a
//! second renderer, and a picture it produced would prove what the second
//! renderer does — not what the screen does. There is one of these, and
//! everything that draws a display list goes through it.

use ronoctua_tokens as tokens;

/// How many pixels each scale is worth.
///
/// IT IS ALSO WHERE THE CELL TABLE COMES FROM. `Heights::from_sizes` used to
/// be handed the five tokens directly, in two places, with a comment in each
/// saying they had to agree with this function or "the buffer reserves one
/// number of cells and the run occupies another". They stopped agreeing the
/// day the base became something a person could move — measured: thirteen
/// runs at 100% and eleven at 80%, on a grid that had not changed. Now both
/// call this, and agreeing is not something anybody has to remember.
///
/// The translation lives HERE, and not in the buffer: invariant 11 says a buffer does not
/// receive a token value, and it is the same contract colour already fulfils.
///
/// The cell grid is always `Normal`'s — a `Large` run occupies more
/// than one cell, and what spaces it is whatever draws.
pub(crate) fn scale_in_px(scale: ronoctua_protocol::Scale) -> f32 {
    match scale {
        // `SIZE_SMALL` is the token that already existed for "smaller than the
        // body", and it goes through `scaled` so it moves with the size a
        // person chose. Everything else the host draws — the footer, its
        // windows and a buffer's body — asks `text::size()`; naming the anchor
        // here would be the defect `one-text-size` exists to refuse, which is
        // why the two lines below do not.
        ronoctua_protocol::Scale::Small => tokens::text::scaled(tokens::font::SIZE_SMALL),
        // THE SYSTEM'S SIZE — the same the footer and the minibuffer use.
        ronoctua_protocol::Scale::Base => tokens::text::size(),
        // AND THE BUFFER'S BODY, which is the same size as of the day the
        // owner asked for one measure across the system.
        //
        // The two arms now give the same pixel, and both names stay. `base` in
        // the WIT means "whatever is INTERFACE inside a buffer, not its
        // content" and `normal` means the content — two questions that
        // happened to arrive at one answer, not one question asked twice.
        // Collapsing them is a change to the WIT, which rebuilds every buffer,
        // and nothing asked for it.
        ronoctua_protocol::Scale::Normal => tokens::text::size(),
        ronoctua_protocol::Scale::Large => tokens::text::scaled(tokens::font::SIZE_XLARGE),
        ronoctua_protocol::Scale::Huge => tokens::text::scaled(tokens::font::SIZE_XXLARGE),
    }
}

/// The cell table, from the one place that resolves a size.
///
/// A run occupies as many CELLS as its scale is worth, and the buffer is told
/// that so it can reserve them. See [`scale_in_px`] for why this is not five
/// tokens read again here.
pub(crate) fn heights() -> ronoctua_protocol::Heights {
    use ronoctua_protocol::Scale;
    ronoctua_protocol::Heights::from_sizes(
        scale_in_px(Scale::Small),
        scale_in_px(Scale::Base),
        scale_in_px(Scale::Normal),
        scale_in_px(Scale::Large),
        scale_in_px(Scale::Huge),
        tokens::font::LINE_HEIGHT,
    )
}

/// Resolves the colour name that came from the buffer.
///
/// # Why the table is no longer here
///
/// It used to be a hand-written `match` of eight names, with `otherwise =>` falling into
/// `TEXT_DIM`. Two things wrong: the list could diverge from the actual tokens
/// with nothing flagging it, and the fallback made a buffer with a wrong name
/// draw nicely — the defect only showed up when someone found the colour odd.
///
/// Now the table is GENERATED from the same `assets/tokens/colors.nix` that generates the
/// WIT enum (section 7.6). On the guest's side a wrong name **does not compile**;
/// on this side, a name that does not resolve is a protocol violation — and the buffer
/// that commits it is refused, not papered over.
#[must_use]
pub fn token_color(name: &str) -> Option<tokens::Color> {
    tokens::color_by_wit_name(name)
}

/// Translates a component's `DisplayList` into runs for the renderer.
///
/// It is here that the two invariants are resolved: the `Text::Key` goes through the
/// i18n, and the `Color` by name becomes a value.
#[must_use]
/// Shifts a run inside the buffer's margin.
///
/// # Why the margin is applied HERE, and not in the component
///
/// The wasip2 buffer does not know where its window is nor how much is left
/// (invariant 11): it counts rows and columns from zero. The margin is a
/// presentation decision, and presentation belongs to the host — the same reason
/// it is the host that resolves colour and translation.
///
/// Without it the first character touches the window's edge and looks like clipping.
fn with_margin(mut t: ronoctua_render::Span) -> ronoctua_render::Span {
    t.column = t.column.saturating_add(tokens::space::CONTENT_INSET);
    t.line = t.line.saturating_add(tokens::space::CONTENT_INSETY);
    t
}

pub fn spans_of(
    list: &ronoctua_protocol::DisplayList,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
    lines: u16,
) -> Vec<ronoctua_render::Span> {
    // AND NOTHING BELOW THE LAST LINE, SAID OUT LOUD.
    //
    // This used to lay out every command it was given. A run past the bottom
    // edge was measured, shaped, submitted, and thrown away by the rasteriser
    // — the surface's bounds are the whole window and nothing clips before
    // that. It cost work and it produced no pixel, and the only reason anybody
    // found out was the owner photographing a footer that had been drawn on
    // top of a history list: the rows that ran off the end simply were not
    // there, with nothing in any log to say a frame had asked for them.
    //
    // The refusal is a `tracing::error!` and not a silent skip, because a
    // buffer that reaches past the edge has a defect in its arithmetic and the
    // host is the only place in production that can see it — `Screen::check`
    // runs in the buffer's own tests, which is exactly where a frame that has
    // already been drawn is not.
    if let Err(e) = list.nothing_is_drawn_below(heights(), lines) {
        tracing::error!(trouble = %e, "a buffer drew below the window");
    }
    // AND THE BLOCK AT THE FOOT IS PLACED FROM THE FOOT.
    //
    // `Span::foot` is the renderer's own answer to this residue, written for
    // the file manager's transport in the owner's words — *"os controles
    // continuam elevados (com espaço abaixo deles)"* — and it counts back from
    // the same `floor(height / line)` this side counts forward from. The
    // buffer says where its footer begins; nothing here guesses.
    let anchored = list.commands.iter().find_map(|c| match c {
        ronoctua_protocol::Command::Anchor { foot_from } => Some(*foot_from),
        _ => None,
    });
    list.commands
        .iter()
        .filter(|c| inside(c, lines))
        // `flat_map`, and not `filter_map`: the grid becomes ONE run per ROW, and it is
        // the only command that produces more than one.
        .flat_map(|c| spans_from(c, texts, columns))
        .map(|mut s| {
            s.foot = matches!(anchored, Some(first) if s.line >= first);
            with_margin(s)
        })
        .collect()
}

/// Whether a command's whole extent lands inside a window this tall.
///
/// The EXTENT, never the first row: a paragraph of six rows beginning on the
/// last one is five rows of ink nobody will see, and the row it begins on being
/// inside is what made that look like a frame that fitted.
fn inside(c: &ronoctua_protocol::Command, lines: u16) -> bool {
    ronoctua_protocol::DisplayList::reach_of(c, heights()) <= lines
}

/// Cuts a text to the budget the buffer declared, and to the window's edge.
///
/// # Why BOTH limits
///
/// The budget is what the buffer reserved for that run — it stops
/// two texts on the same row from overlapping. The edge is the host's guarantee:
/// **nothing spills out of the window**, whatever the buffer asked for. Either one
/// alone lets a case through: with no budget, two runs collide; with no
/// edge, a careless buffer writes off screen and the compositor does not clip.
fn elide_to_budget(s: &str, max_width: u16, column: u16, columns: u16) -> String {
    let ate_a_borda = columns.saturating_sub(column);
    // `0` = no budget of its own: only the edge applies.
    let ceiling = if max_width == 0 {
        ate_a_borda
    } else {
        max_width.min(ate_a_borda)
    };
    ronoctua_render::layout::elide_end(s, ceiling)
}

/// Turns a display list command into the runs the renderer draws.
///
/// It returns empty when the command produces no text — or when the colour does not
/// resolve, which is a protocol violation: the command is REFUSED, not papered over
/// with a default colour. See `token_color`.
fn spans_from(
    c: &ronoctua_protocol::Command,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
) -> Vec<ronoctua_render::Span> {
    use ronoctua_protocol::{Command, Text};

    match c {
        Command::Text {
            line,
            column,
            text,
            color,
            scale,
            max_width,
        } => {
            let Some(color) = token_color(color.as_str()) else {
                tracing::error!(token = color.as_str(), "an invalid colour in a buffer");
                return Vec::new();
            };
            vec![ronoctua_render::Span {
                size: ronoctua_render::Size::Px(scale_in_px(*scale)),
                text: {
                    let s = match text {
                        Text::Data(s) => s.clone(),
                        Text::Key(k) => texts.t(k),
                        // THE ICON IS RESOLVED HERE, and not in the buffer: the codepoint
                        // is in the PUA and changes between Nerd Fonts versions. A name
                        // that does not exist in the table becomes a VISIBLE `?` — the buffer
                        // named it through a generated enum, so arriving here without resolving
                        // means the table and the WIT have diverged, and that has
                        // to show.
                        Text::Icon(name) => tokens::icon_by_wit_name(name)
                            .map_or_else(|| "?".to_owned(), |c| c.to_string()),
                    };
                    // THE BUDGET IS ENFORCED HERE, and not in the buffer.
                    //
                    // Whoever has the translated string is the host: the buffer knows
                    // how much space there is, and the sentence's length changes with the
                    // language. Without this, the browser's invitation ran over
                    // the counter in a narrow window.
                    elide_to_budget(&s, *max_width, *column, columns)
                },
                column: *column,
                line: *line,
                color,
                edge: false,
                foot: false,
            }]
        }
        // THE RULE IS NOT TEXT, and it took a photograph of the screen to
        // finish saying so. It went out through here as `"─".repeat(80)` at
        // column 0 — a glyph, which `with_margin` then pushed two cells in.
        // Three defects in one line, and the owner reported all three: a gap on
        // the left, a spill past the right edge below eighty-two columns, and
        // eighty cells of width that had nothing to do with the window.
        //
        // It goes out through the quad pipeline now — see [`quads_of`], which
        // draws it EDGE TO EDGE. The comment that used to be here said the
        // renderer had no quad pipeline; it has had one since the selection
        // band needed it.
        Command::Rule { .. } => Vec::new(),
        // THE GRID: one line of text per grid row, drawn with the
        // SAME font and the SAME tokens as everything else. That is why
        // it is a first-class node and not a `raw-surface` (section 7.10).
        Command::Grid {
            line,
            column,
            lines,
            color,
        } => {
            let Some(color) = token_color(color.as_str()) else {
                tracing::error!(token = color.as_str(), "an invalid colour in a grid");
                return Vec::new();
            };
            lines
                .iter()
                .enumerate()
                .map(|(n, content)| ronoctua_render::Span {
                    size: ronoctua_render::Size::Grid,
                    text: content.clone(),
                    column: *column,
                    // `saturating_add`: a grid too tall for the screen
                    // should truncate, never wrap around the u16.
                    line: line.saturating_add(u16::try_from(n).unwrap_or(u16::MAX)),
                    color,
                    edge: false,
                    foot: false,
                })
                .collect()
        }
        // THE SELECTION IS NOT TEXT. It goes out through the quad pipeline —
        // see `Host::selections`. It used to return an empty vector here and
        // nothing drew it, in silence.
        Command::Selection { .. } => Vec::new(),
        // THE SVG IS NOT TEXT. It goes out through the image pipeline — see
        // `Host::areas_de_svg`.
        Command::Svg { .. } => Vec::new(),
        // A SENTENCE, WRAPPED HERE because only this side has one.
        //
        // The buffer holds a key; what comes back from the i18n is the
        // translated string, and `layout::wrap` is the same one the file
        // manager breaks a long file name with. The last row it may fill is
        // ELIDED rather than dropped, so a description that does not fit says
        // so with an ellipsis instead of simply stopping mid-word.
        Command::Paragraph {
            line,
            column,
            color,
            scale,
            ..
        } => {
            let Some(token) = token_color(color.as_str()) else {
                tracing::error!(token = color.as_str(), "an invalid colour in a paragraph");
                return Vec::new();
            };
            // THE WRAP IS ASKED FOR, never done here: the thumb beside this
            // sentence is a fraction of the same wrap, and two of them would
            // be a bar that says the text is one length while the rows say
            // another. See [`wrapped`].
            let Some(said) = wrapped(c, texts, columns) else {
                return Vec::new();
            };
            said.rows
                .iter()
                .enumerate()
                .map(|(n, text)| ronoctua_render::Span {
                    line: line.saturating_add(u16::try_from(n).unwrap_or(0)),
                    column: *column,
                    text: text.clone(),
                    color: token,
                    size: ronoctua_render::Size::Px(scale_in_px(*scale)),
                    edge: false,
                    foot: false,
                })
                .collect()
        }
        // A BAR HAS NO INK. It is a rectangle, and `quads_of` draws it —
        // named here rather than swept into a `_` so a primitive that DOES
        // carry text cannot be added without this file being read.
        Command::Bar { .. } => Vec::new(),
        // AND AN ANCHOR IS NOT A THING AT ALL. It says where the frame sits,
        // which `spans_of` above has already read; there is nothing to draw.
        Command::Anchor { .. } => Vec::new(),
    }
}

/// The cell, in pixels: `(width, height)`.
///
/// # Why one function and not two constants
///
/// Because the drawing, the click's arithmetic, the selection band and the
/// count of how many columns fit all have to agree to the pixel. When the
/// cursor's hit test computed the cell one way and the drawing another, the
/// error did not show at the origin — it grew with the distance from it, which
/// is the hardest kind to see and the hardest to attribute.
///
/// THE GRID IS ALWAYS `Normal`'s. A `Large` run occupies more than one cell,
/// and what spaces it is whatever draws.
#[must_use]
pub fn cell() -> (f32, f32) {
    (
        tokens::text::size() * tokens::font::CELL_WIDTH_EM,
        tokens::text::size() * tokens::font::LINE_HEIGHT,
    )
}

/// Where a band starts and how wide it is, in pixels.
///
/// # The two shapes, and why the first one ignores the margin
///
/// A band asked for with no column and no width is the one this list always
/// had: FULL BLEED, from the window's edge to the window's edge, past the
/// margin that text is inset by. That is not a detail — a selection that
/// stopped at the text rail would leave a stripe of page down both sides of
/// the highlighted row, and a rule that did it would be a line with two ends
/// floating in the middle of the window.
///
/// Anything else is a band inside the CONTENT rail, measured in the same cells
/// the runs on that row are measured in — because what it is bounding is a
/// column of content, and it has to line up with it.
fn band_px(column: u16, columns: u16, width_px: f32) -> (f32, f32) {
    if column == 0 && columns == 0 {
        return (0.0, width_px);
    }
    let (cell_w, _) = cell();
    let from = f32::from(column.saturating_add(tokens::space::CONTENT_INSET)) * cell_w;
    let wide = if columns == 0 {
        width_px - from
    } else {
        f32::from(columns) * cell_w
    };
    (from, wide.max(0.0))
}

/// WHERE A BUFFER'S LINES LAND IN ITS WINDOW.
///
/// # One arithmetic, because five things read it
///
/// The ink, the grounds, the rules, the scrollbars and the pointer's inverse.
/// Written out at each of them it was a constant waiting to disagree with
/// itself — which is the defect this whole module has now been bitten by
/// twice, once on the horizontal and once here.
///
/// # And the residue is not always at the bottom
///
/// A surface holds `floor(height / line)` whole rows plus a leftover of less
/// than one, nought to twenty-five pixels, changing with the window. It falls
/// under the last row — and a buffer that ends in a FOOTER puts there the one
/// thing that has to sit at the bottom. The owner read it as *"um espaço após
/// o `Last run` … que varia de tamanho conforme a altura disponível"*;
/// measured on that screen, nine pixels.
///
/// A buffer whose window ends in a fixed block says so
/// ([`ronoctua_protocol::Command::Anchor`]), and from that line down the lines
/// are counted UP from the bottom edge. The leftover then falls between the
/// block above and the block at the foot, inside the page's own ground, where
/// nothing is drawn.
#[derive(Debug, Clone, Copy)]
pub struct Rows {
    /// The first line anchored to the window's foot, if the buffer said.
    foot_from: Option<u16>,
    /// How many whole lines the surface holds — the same `floor` the component
    /// was told its height by, and the same one `Span::foot` counts back from.
    lines: u16,
    /// The surface's height in pixels: the number the compositor gave, never a
    /// count of anything.
    height: f32,
}

impl Rows {
    /// Read from the frame itself, which is where the anchor travels.
    #[must_use]
    pub fn of(commands: &[ronoctua_protocol::Command], height_px: f32) -> Self {
        let (_, line_height) = cell();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the same floor `context_of` and `render::down` both make"
        )]
        let lines = if line_height > 0.0 {
            (height_px / line_height) as u16
        } else {
            0
        };
        let foot_from = commands.iter().find_map(|c| match c {
            ronoctua_protocol::Command::Anchor { foot_from } => Some(*foot_from),
            _ => None,
        });
        Self {
            foot_from,
            lines,
            height: height_px,
        }
    }

    /// Whether this line is counted from the window's foot.
    #[must_use]
    pub const fn anchored(&self, line: u16) -> bool {
        matches!(self.foot_from, Some(first) if line >= first)
    }

    /// The top of a line, in the surface's pixels.
    #[must_use]
    pub fn top(&self, line: u16) -> f32 {
        let (_, line_height) = cell();
        if self.anchored(line) && self.lines > 0 {
            // COUNTED UP FROM THE BOTTOM, the same sum `Span::foot` makes on
            // the renderer's side of the same number — `lines` is `floor` of
            // the height there too, so the ink and the ground land together.
            return f32::from(self.lines.saturating_sub(line)).mul_add(-line_height, self.height);
        }
        f32::from(line.saturating_add(tokens::space::CONTENT_INSETY)) * line_height
    }

    /// Where the ground on this line ENDS.
    ///
    /// Two residues, and both belong to a ground rather than to a gap:
    ///
    /// * the LAST line takes what is under it, which is `Scroll::track`'s rule
    ///   and was already applied here — the owner photographed its absence as
    ///   a strip of the page's colour between the footer and the border;
    /// * and the line just ABOVE an anchor takes the seam, because the block
    ///   at the foot has moved down by the residue. Without this the same
    ///   strip comes back in the middle of the window, between the activity
    ///   container and the footer sitting on it.
    #[must_use]
    pub fn bottom(&self, line: u16) -> f32 {
        let (_, line_height) = cell();
        let natural = self.top(line) + line_height;
        if self.height - natural < line_height {
            return self.height;
        }
        match self.foot_from {
            Some(first) if line.saturating_add(1) == first => self.top(first),
            _ => natural,
        }
    }

    /// Which line a point of the surface is on.
    ///
    /// `None` above the first line, which is the margin — the same answer
    /// `cell_at` gives on the horizontal, and for the same reason: saying
    /// "line zero" there lights the first row whenever a hand crosses the
    /// border.
    #[must_use]
    pub fn line_at(&self, y: f32) -> Option<u16> {
        let (_, line_height) = cell();
        if line_height <= 0.0 || y < 0.0 {
            return None;
        }
        if let Some(first) = self.foot_from
            && self.lines > 0
            && y >= self.top(first)
        {
            // The inverse of `top`, and it has to be the inverse: a click in
            // the footer that answered a line the footer is not on is the
            // whole class of defect this type exists to end.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "bounded by the branch: y is inside the anchored block"
            )]
            let back = ((self.height - y) / line_height).ceil() as u16;
            return Some(self.lines.saturating_sub(back).max(first));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "guarded above; the same floor `cell_at` makes on the other axis"
        )]
        let line = (y / line_height) as u16;
        line.checked_sub(tokens::space::CONTENT_INSETY)
    }
}

/// A SENTENCE, WRAPPED — where its rows landed and how many there are.
///
/// # Why this is one function and not two
///
/// Because the host is the only side that can answer it: the buffer holds a
/// KEY, the translation is here, and the wrap is here. The ink reads this and
/// so does the thumb beside it, which is the same arrangement [`Grab`] makes
/// for a bar — written apart, the rows drawn and the fraction the thumb shows
/// would answer to two different wraps.
#[derive(Debug, Clone)]
pub struct Sentence {
    /// The rows that are DRAWN, elided where the text runs past the last one.
    pub rows: Vec<String>,
    /// How many rows the whole sentence came to.
    pub total: usize,
    /// The first one drawn — the buffer's offset, CLAMPED here.
    pub first: usize,
    /// How many rows the rectangle holds.
    pub lines: usize,
}

impl Sentence {
    /// The window on it, as a `Scroll` — the system's one scrollbar.
    #[must_use]
    pub const fn scroll(&self) -> ronoctua_design::Scroll {
        ronoctua_design::Scroll {
            total: self.total,
            lines: self.lines,
            offset: self.first,
            top: 0,
        }
    }
}

/// This command's sentence, wrapped, or `None` when it is not a paragraph.
#[must_use]
pub fn wrapped(
    c: &ronoctua_protocol::Command,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
) -> Option<Sentence> {
    let ronoctua_protocol::Command::Paragraph {
        column,
        lines,
        columns: wide,
        content,
        first_row,
        ..
    } = c
    else {
        return None;
    };
    let whole = match content {
        ronoctua_protocol::Text::Key(k) => texts.t(k),
        ronoctua_protocol::Text::Data(t) => t.clone(),
        ronoctua_protocol::Text::Icon(i) => i.clone(),
    };
    // THE BUDGET IS THE SMALLER of what was asked for and what is left, the
    // same pair `elide_to_budget` applies to a run: the first stops two blocks
    // colliding and the second is the host's promise that nothing leaves the
    // window.
    let room = columns.saturating_sub(*column).min(*wide).max(1);
    let lines = usize::from((*lines).max(1));
    // THE WHOLE OF IT, because the TOTAL is what a scrollbar is a fraction of.
    // The ceiling is a bound on the work and not on the answer: a sentence
    // longer than this is one nobody reads to the end of either way.
    let all = ronoctua_render::layout::wrap(&whole, room, SENTENCE_MOST);
    let total = all.len();
    // AND THE OFFSET IS CLAMPED HERE, which is what lets the buffer keep one
    // at all: it has never seen the wrap, so the number it holds is only ever
    // a number this side handed it back.
    let first = usize::from(*first_row).min(total.saturating_sub(lines));
    // AND EVERY ROW IS CUT TO THE ROOM, which is the host's promise and not a
    // refinement: `wrap` breaks a single cluster wider than the line onto a
    // row of its own rather than splitting it, and that row is the one case
    // where a wrapped line can still be too wide.
    //
    // THERE IS NO "AND MORE" ELLIPSIS. There used to be, because there was no
    // other way to say a sentence went on: it was cut and that was the end of
    // it. What says it now is the THUMB beside the words — a control, which is
    // what the owner asked for — and an ellipsis on a row somebody is about to
    // scroll past would be a second answer to the same question.
    let rows: Vec<String> = all
        .into_iter()
        .skip(first)
        .take(lines)
        .map(|row| ronoctua_render::layout::elide_end(&row, room))
        .collect();
    Some(Sentence {
        rows,
        total,
        first,
        lines,
    })
}

/// The most rows a sentence is wrapped into before the host stops counting.
///
/// A bound on the WORK and not on the answer. Two thousand rows is the
/// preview's own ceiling on a text file, and a message longer than that is one
/// nobody reads to the end of by scrolling a footer.
const SENTENCE_MOST: usize = 2_000;

/// WHERE A SCROLLBAR WAS PUT, in the surface's own pixels.
///
/// # Why a rectangle, and why both doors read this one
///
/// Because the bar was PAINTED in pixels from the window's right edge and
/// GRABBED in cells from the content margin, and the two never met: a bar in
/// the last column is painted past the last cell — that is what makes it touch
/// the border — so the cell the pointer reports there is two columns beyond
/// the one the component drew in. The comparison `bar.column == pointer.column`
/// could not match, and a press on the thumb fell through to the row behind it.
///
/// Nothing about that was a mistake anybody made twice; it is what happens when
/// one control is placed by two arithmetics. So there is one: [`bar_box`]
/// answers where the bar went, [`quads_of`] paints the thumb INSIDE the answer,
/// and [`bar_grabbed`] reads the same answer to decide a press. It is the shape
/// `chrome::hint_spans` and `chrome::hint_marks` already have, for the same
/// reason and in the same words: two readings of one placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grab {
    /// The TRACK, not the thumb: a press anywhere down it is a press on this
    /// bar. A thumb is one cell wide and a few lines tall, and a control that
    /// only answers where its thumb happens to be is a control that has to be
    /// hunted for. `Scroll::offset_at` already centres the thumb on the point,
    /// so pressing the track jumps there — which is what every scrollbar does.
    pub x: f32,
    /// The top of the band, in the surface's pixels.
    pub y: f32,
    /// One cell, which is the bar's whole thickness.
    pub width: f32,
    /// The track's length, residue included where this band owns it.
    pub height: f32,
    /// The cell the COMPONENT drew it in. It travels back with every answer,
    /// because only the component knows which of its lists this bar is.
    pub column: u16,
    /// The first LINE of the band, in the component's own rows.
    pub first_line: u16,
    /// The window the bar was drawn against, in the component's rows.
    pub scroll: ronoctua_design::Scroll,
}

impl Grab {
    /// Whether a point of the surface is on this bar.
    #[must_use]
    pub fn holds(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Which row a press at this height puts at the top of the list.
    ///
    /// The inverse of the thumb, asked of the same track the thumb was drawn
    /// in — which is the whole point of the rectangle being one answer.
    #[must_use]
    pub fn row_at(&self, y: f32) -> usize {
        let (_, line_height) = cell();
        self.scroll.offset_at(y - self.y, self.height, line_height)
    }

    /// THE THUMB INSIDE THE TRACK, as the rectangle that is painted.
    ///
    /// It asks `Scroll::bar` and does not assemble one out of the design
    /// crate's `thumb`: there is ONE scrollbar arithmetic in this system and
    /// that is where it lives — nine views share it, and the defects the
    /// argument is made of were the differences between the copies.
    /// `one-scrollbar` refuses a second, and refuses it BY THE NAME: nothing
    /// outside that crate may call `thumb`, which is why this one is not
    /// called that.
    ///
    /// The track is handed back as its own bottom, which is idempotent: the
    /// height this carries already came from `Scroll::track`.
    #[must_use]
    pub fn painted(&self) -> Option<(f32, f32, f32, f32)> {
        let (_, line_height) = cell();
        self.scroll.bar(
            self.x + self.width,
            self.y,
            self.y + self.height,
            self.width,
            line_height,
        )
    }
}

/// Where this command's bar was put, or `None` when it is not one or when the
/// list it belongs to fits.
///
/// `Scroll::cut` is the one place that decides whether a bar exists at all: a
/// bar over a list that fits is a control that does nothing, and one that could
/// be grabbed would be worse than one that is merely drawn.
#[must_use]
pub fn bar_box(
    c: &ronoctua_protocol::Command,
    rows: &Rows,
    width_px: f32,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
) -> Option<Grab> {
    // A SENTENCE HAS ONE TOO, and the buffer could not have asked for it: only
    // this side knows how many rows the text came to. The rectangle is the
    // paragraph's own, and the lane is the last cell of it — which is why a
    // buffer that draws a scrollable sentence budgets one cell fewer, exactly
    // as it does for a list.
    let (line, lines, column, scroll) = match c {
        ronoctua_protocol::Command::Bar {
            line,
            lines,
            column,
            total,
            offset,
            ..
        } => (
            *line,
            *lines,
            *column,
            ronoctua_design::Scroll {
                total: *total as usize,
                lines: *lines as usize,
                offset: *offset as usize,
                top: 0,
            },
        ),
        ronoctua_protocol::Command::Paragraph {
            line,
            column,
            lines,
            columns: wide,
            ..
        } => {
            let said = wrapped(c, texts, columns)?;
            (
                *line,
                *lines,
                column.saturating_add(wide.saturating_sub(1)),
                said.scroll(),
            )
        }
        _ => return None,
    };
    let (line, lines, column) = (&line, &lines, &column);
    if !scroll.cut() {
        return None;
    }
    let (cell_width_px, line_height) = cell();
    // AND A BAR IN THE LAST COLUMN REACHES THE EDGE. A bar is CHROME, like a
    // rule: it belongs to the surface and not to the text, so it does not stop
    // where the right margin does. Inset on both sides it stopped a whole cell
    // short of the window, and the owner read the result as a scrollbar
    // *"extremamente afastada da borda"*. The same argument `chrome::rule`
    // makes about a separator.
    //
    // THE LAST COLUMN, and not every column: the file manager draws a bar down
    // a SECTION, in the middle of the window, and one pushed to the edge would
    // be a bar beside the wrong list. Which is the last is derived the same way
    // the component was told its width — `buffer::context_of`, the window's
    // cells less the margin on both sides — so the two cannot disagree.
    let inset = f32::from(tokens::space::CONTENT_INSET) * cell_width_px;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the same cast `context_of` makes, on the same number"
    )]
    let across = ((width_px / cell_width_px) as u32)
        .saturating_sub(u32::from(tokens::space::CONTENT_INSET) * 2);
    let last = u32::from(*column).saturating_add(1) >= across;
    let right = if last {
        width_px
    } else {
        f32::from(column.saturating_add(1)) * cell_width_px + inset
    };
    // AND THE BAND'S BOTTOM IS THE ONE [`Rows`] GIVES IT, never a count of
    // lines: a surface holds `floor(height / line)` whole rows plus a residue
    // under one, so `rows × line` stops short of it. Which band may take that
    // residue is `Scroll::track`'s decision and not this module's — asked with
    // the band's own bottom, it takes the leftover exactly where a ground
    // does, and nowhere else.
    let top = rows.top(*line);
    let bottom = rows.bottom(line.saturating_add(lines.saturating_sub(1)));
    Some(Grab {
        x: right - cell_width_px,
        y: top,
        width: cell_width_px,
        height: scroll.track(top, bottom, line_height),
        column: *column,
        first_line: *line,
        scroll,
    })
}

/// A WHEEL OVER A SENTENCE THAT IS CUT, answered as a clamped offset.
///
/// # Why the host answers this one and the buffer answers every other wheel
///
/// Because a list's total is the buffer's — it holds the rows — and a
/// sentence's is not: only this side has wrapped it. A buffer told to "scroll
/// by three" would keep a number it cannot bound, and wheeling past the end
/// would leave it counting rows that are not there while the screen sat still.
///
/// So the wheel over a paragraph comes back as `Input::Scrolled`, exactly as a
/// drag on its bar does, and the number the buffer keeps is always one this
/// side produced. Answers `None` when the point is not over a sentence that
/// scrolls, which is when the wheel means what it always meant.
///
/// The answer carries the PARAGRAPH's own cell and line — never the pointer's,
/// which is the same rule a bar's drag follows and for the same reason: what
/// the component does with them is route the number to one of its blocks.
#[must_use]
pub fn sentence_wheeled(
    commands: &[ronoctua_protocol::Command],
    x: f32,
    y: f32,
    height_px: f32,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
    lines: i16,
) -> Option<Scrolled> {
    let (cell_width_px, _) = cell();
    let rows = Rows::of(commands, height_px);
    commands.iter().find_map(|c| {
        let ronoctua_protocol::Command::Paragraph {
            line,
            column,
            lines: tall,
            columns: wide,
            ..
        } = c
        else {
            return None;
        };
        let said = wrapped(c, texts, columns)?;
        if said.total <= said.lines {
            return None;
        }
        // THE WHOLE RECTANGLE ANSWERS, not just the lane the thumb is in: a
        // wheel is aimed at what one is reading, and what one is reading is
        // the words.
        let left = f32::from(column.saturating_add(tokens::space::CONTENT_INSET)) * cell_width_px;
        let right = left + f32::from(*wide) * cell_width_px;
        let top = rows.top(*line);
        // AND THE BOTTOM IS THE ONE [`Rows`] GIVES IT, never `top + rows ×
        // line`: a surface is `floor(height / line)` lines plus a residue of
        // less than one, and a rectangle that ends at the window's edge ends
        // at the EDGE. `Rows::bottom` is where that rule lives, and it is the
        // same one the grounds and the rules read.
        let bottom = rows.bottom(line.saturating_add(tall.saturating_sub(1)));
        if x < left || x >= right || y < top || y >= bottom {
            return None;
        }
        let most = said.total.saturating_sub(said.lines);
        let first = if lines < 0 {
            said.first.saturating_sub(lines.unsigned_abs() as usize)
        } else {
            (said.first + lines as usize).min(most)
        };
        Some(Scrolled {
            column: column.saturating_add(wide.saturating_sub(1)),
            line: *line,
            first: u32::try_from(first).unwrap_or(u32::MAX),
        })
    })
}

/// WHERE A SCROLL LANDED, as the three numbers that cross the wall.
///
/// A struct and not a tuple because the three are carried to the component
/// unchanged, and the one thing that must never happen is the POINTER's cell
/// going out in place of the CONTROL's — see `a-bar-is-grabbed-where-it-is-
/// drawn`, which reads the call site to say so. Named fields are what make
/// that readable at the call site and checkable from outside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scrolled {
    /// The control's own cell.
    pub column: u16,
    /// The first line of what it scrolls.
    pub line: u16,
    /// The row it was moved to, already clamped.
    pub first: u32,
}

/// The bar of a display list under a point of the surface, if any.
///
/// The point is in PIXELS and never in cells, and that is the fix: the cell a
/// pointer reports in the right margin is not the cell the bar was drawn in.
#[must_use]
pub fn bar_grabbed(
    commands: &[ronoctua_protocol::Command],
    x: f32,
    y: f32,
    width_px: f32,
    height_px: f32,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
) -> Option<Grab> {
    let rows = Rows::of(commands, height_px);
    commands
        .iter()
        .filter_map(|c| bar_box(c, &rows, width_px, texts, columns))
        .find(|g| g.holds(x, y))
}

/// Everything that is drawn EDGE TO EDGE, as quads, for a window `width_px` wide.
///
/// # Two rails, and only two
///
/// A display list leaves this module by one of two routes, and which one a
/// command takes is the whole of the layout standard:
///
/// | rail | origin | what rides it |
/// |---|---|---|
/// | inset | [`spans_of`] → [`with_margin`] | every `Text`, every `Grid` |
/// | full-bleed | here | `Rule`, `Selection` |
///
/// Text is inset because a glyph against the window's edge reads as clipping.
/// A boundary across the page is the opposite: it starts at `x = 0` and runs
/// the whole width, and it never passes through [`with_margin`].
///
/// # And it stops short when it is told to
///
/// The reason written here used to be that a boundary "with a beginning and an
/// end is a box — which invariant 15 forbids". That was not the owner's rule
/// and it is not the invariant's: cards and separators are allowed, and two
/// columns side by side each want a band and a rule of their own. `column` and
/// `columns` carry that, with `0` meaning to the edge — so a full-bleed band
/// is still one number, and a bounded one is now possible at all.
///
/// It used to be called `selections_of`, and the rule was not here: it went out
/// as a line of `─` glyphs, eighty of them, starting at the margin. That is
/// what the owner photographed — a gap on the left, and a spill past the right
/// edge on any window narrower than eighty-two columns.
#[must_use]
pub fn quads_of(
    list: &ronoctua_protocol::DisplayList,
    width_px: f32,
    height_px: f32,
    texts: &ronoctua_i18n::Texts,
    columns: u16,
) -> Vec<ronoctua_render::quad::Quad> {
    use ronoctua_protocol::Command;
    let rows = Rows::of(&list.commands, height_px);
    list.commands
        .iter()
        .filter_map(|c| match c {
            // A CHANGE OF COLOUR, and no radius. It is not always the whole
            // row: `columns` says how far it goes and `0` says to the edge.
            Command::Selection {
                line,
                column,
                columns,
                color,
            } => {
                let token = token_color(color.as_str()).or_else(|| {
                    tracing::error!(token = color.as_str(), "an invalid colour in a selection");
                    None
                })?;
                // AND THE LAST GROUND TAKES THE RESIDUE.
                //
                // A surface holds `floor(height / line)` whole rows plus a
                // leftover of less than one, and a ground of exactly one
                // `line_height` on the last row stops short of the bottom edge
                // by that leftover. The owner photographed it as a strip of the
                // page's own colour between the footer and the border.
                //
                // The rule is not new and it is not this module's: it is
                // `Scroll::track`'s, written out there for the thumb — *"only
                // the LAST band in a column may take it… a band ending within
                // one unit of the surface is the last one"* — and applied by
                // `files::Section::bar_box`. Scrollbars had it and grounds
                // never did, which is why the gap survived the fix that was
                // meant to be the same fix.
                let top = rows.top(*line);
                let height = rows.bottom(*line) - top;
                // AND IT IS AS WIDE AS IT WAS ASKED TO BE. Asked for nothing,
                // it is the full-bleed stripe this command used to be the only
                // shape of — two columns side by side, each with its own
                // selection, could not be drawn at all, not because anybody
                // decided against it but because there was nowhere to put the
                // number.
                let (from, wide) = band_px(*column, *columns, width_px);
                Some(ronoctua_render::quad::Quad {
                    x: from,
                    y: top,
                    width: wide,
                    height,
                    color: token,
                })
            }
            // THE RULE SITS AT THE BOTTOM EDGE OF THE ROW IT NAMES, which is
            // what changes `Command::Rule { line }` from "a glyph in row N"
            // into "the boundary at the END of row N" — which is what a rule
            // has always meant on screen and never meant in the type.
            //
            // Drawn in the middle of its row it would be a line with text
            // above and below it inside one cell; drawn at the edge it is the
            // seam between two rows, and the air on either side of it is the
            // rows' own line boxes rather than something anybody has to add.
            //
            // AND IT STOPS WHERE IT WAS TOLD TO. `columns: 0` is to the edge,
            // which is what a boundary across the page asks for and what this
            // used to be the only shape of — a separator under a heading
            // inside one of two side-by-side columns has to end where the
            // column does, or it runs through its neighbour.
            Command::Rule {
                line,
                column,
                columns,
                color,
            } => {
                let token = token_color(color.as_str()).or_else(|| {
                    tracing::error!(token = color.as_str(), "an invalid colour in a rule");
                    None
                })?;
                #[allow(clippy::cast_precision_loss)]
                let thickness = f32::from(tokens::space::RULE_THICKNESS);
                let (from, wide) = band_px(*column, *columns, width_px);
                Some(ronoctua_render::quad::Quad {
                    x: from,
                    // AT THE END OF THE ROW, which is where its ground ends
                    // and not a line's worth below its top: on the last row
                    // and at the seam above an anchor those are not the same
                    // pixel, and a rule floating a residue above the block it
                    // separates is the same strip said with one line.
                    y: rows.bottom(*line) - thickness,
                    width: wide,
                    height: thickness,
                    color: token,
                })
            }
            // A BUFFER'S SCROLLBAR IS THE SYSTEM'S SCROLLBAR.
            //
            // `ronoctua_design::Scroll` is the same arithmetic the manager,
            // the footer's eight views and every window's list bar use — it
            // has compiled on both sides of the wall since it was written,
            // and the reason it is asked HERE rather than in the component is
            // that a thumb is a fraction of a track in PIXELS. A buffer
            // counts cells. One built out of block characters would be a
            // second scrollbar that resembles the five real ones and matches
            // none of them.
            //
            // The residue is the host's for the same reason it always was: a
            // surface holds `floor(height / line)` whole rows plus a leftover
            // of less than one, and only this side knows the surface.
            // A BUFFER'S SCROLLBAR IS THE SYSTEM'S SCROLLBAR.
            //
            // `ronoctua_design::Scroll` is the same arithmetic the manager,
            // the footer's eight views and every window's list bar use — it
            // has compiled on both sides of the wall since it was written,
            // and the reason it is asked HERE rather than in the component is
            // that a thumb is a fraction of a track in PIXELS. A buffer
            // counts cells. One built out of block characters would be a
            // second scrollbar that resembles the five real ones and matches
            // none of them.
            //
            // WHERE IT GOES IS [`bar_box`]'s answer and not this arm's, and
            // the press reads the same one — see `Grab`.
            Command::Bar {
                lines,
                shown,
                color,
                ..
            } => {
                let token = token_color(color.as_str()).or_else(|| {
                    tracing::error!(token = color.as_str(), "an invalid colour in a bar");
                    None
                })?;
                // THE BAND AND THE WINDOW ARE ONE NUMBER in a text grid: a
                // row IS a line, and `Screen::bar`'s callers set the two from
                // the same count. They cross separately because the record
                // says what each of them means, so the host checks rather
                // than assumes — a buffer that ever sent two different
                // numbers would draw a thumb against a track it does not
                // have, and nothing on the screen would say so.
                if u32::from(*lines) != *shown {
                    tracing::error!(
                        lines = *lines,
                        shown = *shown,
                        "a bar whose band and window disagree"
                    );
                }
                let (x, y, width, height) =
                    bar_box(c, &rows, width_px, texts, columns)?.painted()?;
                Some(ronoctua_render::quad::Quad {
                    x,
                    y,
                    width,
                    height,
                    color: token,
                })
            }
            // AND A SENTENCE LONGER THAN ITS RECTANGLE HAS ONE TOO. The buffer
            // could not ask for it — it has never seen the wrap — so the host
            // draws it where it drew the words, in the same tone a list's bar
            // takes on this ground.
            Command::Paragraph { color, .. } => {
                let token = token_color(color.as_str())?;
                let _ = token;
                let (x, y, width, height) =
                    bar_box(c, &rows, width_px, texts, columns)?.painted()?;
                Some(ronoctua_render::quad::Quad {
                    x,
                    y,
                    width,
                    height,
                    color: tokens::color::RULE_RAISED,
                })
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        Rows, bar_box, bar_grabbed, cell, elide_to_budget, quads_of, sentence_wheeled, spans_of,
        token_color, wrapped,
    };
    use ronoctua_protocol::{Color, Command, DisplayList, Text};

    /// A window with a RESIDUE: ten whole lines and a third of one left over,
    /// which is every real window — a compositor hands out pixels and the grid
    /// is `floor` of them.
    fn a_window() -> (f32, f32, u16) {
        let (cell_w, line_h) = cell();
        (cell_w * 100.0, line_h.mul_add(10.0, line_h * 0.37), 10)
    }

    /// A page whose last six lines are a footer, with a ground and a value on
    /// the last of them.
    fn a_footer(lines: u16) -> DisplayList {
        let mut d = DisplayList::fresh_one();
        d.push(Command::Anchor {
            foot_from: lines - 6,
        });
        for line in lines - 6..lines {
            d.push(Command::Selection {
                line,
                column: 0,
                columns: 0,
                color: Color::name("surface-header"),
            });
        }
        d.push(Command::Rule {
            line: lines - 2,
            column: 0,
            columns: 0,
            color: Color::name("rule-raised"),
        });
        d.push(Command::Text {
            line: lines - 1,
            column: 0,
            text: Text::datum("2026-09-20 18:04"),
            color: Color::name("text-primary"),
            scale: ronoctua_protocol::Scale::Normal,
            max_width: 0,
        });
        d
    }

    /// FAILS IF: a sentence longer than its rectangle does not scroll, or its
    /// thumb says a different length from the rows beside it.
    ///
    /// # What the owner asked for
    ///
    /// *"se o texto for maior que o espaço livre disponível no footer, então o
    /// footer do brain DEVE funcionar com um scrollbar própria"*.
    ///
    /// The buffer cannot own this: it holds a KEY, the translation is here and
    /// the wrap is here, so it keeps the OFFSET and this side keeps everything
    /// else — which is why the clamp, the thumb and the wheel are all asked of
    /// one function.
    #[test]
    fn a_sentence_longer_than_its_rectangle_scrolls() {
        let (cell_w, line_h) = cell();
        let (width, height) = (cell_w * 44.0, line_h * 12.0);
        let texts = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
        let long = "pnpm update --global: pnpm keeps a global store of its own, which is not \
                    npm's — which is why these are two steps and not one. Without its \
                    directory the step is skipped, and the sequence says so.";
        let say = |first_row: u16| {
            let mut d = DisplayList::fresh_one();
            d.push(Command::Paragraph {
                line: 4,
                column: 0,
                lines: 3,
                columns: 39,
                content: Text::datum(long),
                color: Color::name("state-error"),
                scale: ronoctua_protocol::Scale::Normal,
                first_row,
            });
            d
        };

        let d = say(0);
        let said = wrapped(&d.commands[0], &texts, 40).expect("a paragraph");
        assert!(
            said.total > said.lines,
            "the sentence fits; the test proves nothing"
        );
        assert_eq!(said.rows.len(), 3);
        assert_eq!(said.first, 0);

        // THE OFFSET IS CLAMPED HERE, which is the whole reason the buffer may
        // keep one: it has never seen the wrap.
        let far = wrapped(&say(200).commands[0], &texts, 40).expect("a paragraph");
        assert_eq!(
            far.first,
            far.total - far.lines,
            "an offset past the end was kept"
        );
        // AND THE WINDOW MOVED: the rows at the end are not the rows at the
        // start, which is the whole of what scrolling is.
        assert_ne!(
            said.rows, far.rows,
            "the offset changed nothing on the screen"
        );
        // AND NOTHING SPILLS, whatever the wrap had to do with a word longer
        // than the line.
        for row in said.rows.iter().chain(&far.rows) {
            assert!(
                ronoctua_render::layout::width(row) <= 39,
                "a wrapped row is wider than the rectangle: {row:?}"
            );
        }

        // AND A THUMB IS DRAWN BESIDE IT, in the rectangle's last cell, out of
        // the SAME wrap: a bar that counted the rows a second time would say
        // the text is one length while the rows say another.
        let quads = quads_of(&d, width, height, &texts, 40);
        let rows = Rows::of(&d.commands, height);
        let grab = bar_box(&d.commands[0], &rows, width, &texts, 40).expect("a thumb");
        assert_eq!(
            grab.column, 38,
            "the thumb is not in the rectangle's last cell"
        );
        assert_eq!(grab.scroll.total, said.total);
        assert!(
            quads.iter().any(|q| (q.x - grab.x).abs() < 0.01),
            "the sentence's thumb was not painted"
        );
        // AND IT IS GRABBED WHERE IT IS DRAWN, like every other control.
        assert!(
            bar_grabbed(
                &d.commands,
                grab.x + 1.0,
                grab.y + 1.0,
                width,
                height,
                &texts,
                40
            )
            .is_some(),
            "the sentence's thumb cannot be pressed"
        );

        // AND THE WHEEL OVER THE WORDS IS ANSWERED BY THIS SIDE, already
        // clamped — a buffer told to "scroll by three" would be counting rows
        // it has never seen.
        let inside = (cell_w * 4.0, rows.top(4) + 1.0);
        let moved = sentence_wheeled(&d.commands, inside.0, inside.1, height, &texts, 40, 2)
            .expect("the wheel over a cut sentence");
        assert_eq!((moved.column, moved.line), (38, 4));
        assert_eq!(moved.first, 2);
        let stop = sentence_wheeled(&d.commands, inside.0, inside.1, height, &texts, 40, 99)
            .expect("the wheel over a cut sentence");
        assert_eq!(
            usize::try_from(stop.first).unwrap(),
            said.total - said.lines
        );
        // AND A POINT OUTSIDE THE RECTANGLE IS NOT IT.
        assert!(
            sentence_wheeled(&d.commands, inside.0, rows.top(0), height, &texts, 40, 2).is_none(),
            "a wheel above the sentence moved it"
        );
    }

    /// FAILS IF: the block a buffer anchored to its foot does not touch the
    /// foot.
    ///
    /// # What the owner saw
    ///
    /// *"há um espaço após o `Last run`, o qual não deveria existir e varia de
    /// tamanho conforme a altura disponível para o buffer"*. It is the
    /// surface's residue: `floor(height / line)` whole rows and a leftover of
    /// less than one, falling under the last row — which is where a footer
    /// puts the one thing that has to sit at the bottom. Nine pixels on that
    /// screen, nought to twenty-five on any other.
    #[test]
    fn the_block_at_the_foot_touches_the_foot() {
        let (_, line_h) = cell();
        let (width, height, lines) = a_window();
        let list = a_footer(lines);
        let rows = Rows::of(&list.commands, height);

        // THE LAST ROW ENDS AT THE BOTTOM EDGE, to the pixel.
        let last = lines - 1;
        assert!(
            (rows.top(last) + line_h - height).abs() < 0.01,
            "the last row's box ends {} px above the edge",
            height - (rows.top(last) + line_h)
        );
        // AND ITS GROUND REACHES IT.
        assert!((rows.bottom(last) - height).abs() < 0.001);
        // AND THE SEAM IS TAKEN BY THE GROUND ABOVE IT, so the residue does
        // not come back as a strip between the block above and the footer —
        // which is the same strip, moved.
        assert!(
            (rows.bottom(lines - 7) - rows.top(lines - 6)).abs() < 0.001,
            "the residue opened a gap above the footer"
        );

        // THE INK IS PLACED FROM THE FOOT TOO, and the renderer counts back
        // from the same `floor`.
        let texts = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
        let spans = spans_of(&list, &texts, 100, lines);
        assert!(!spans.is_empty(), "the footer drew nothing");
        assert!(
            spans.iter().all(|s| s.foot),
            "a run in the anchored block is placed from the top"
        );

        // AND THE INVERSE ANSWERS THE SAME LINE, every one of them: a click in
        // the footer that named a line the footer is not on is the whole class
        // of defect `Rows` exists to end.
        for line in 0..lines {
            assert_eq!(
                rows.line_at(rows.top(line) + 1.0),
                Some(line),
                "the pointer on row {line} answered somewhere else"
            );
        }

        // AND A SURFACE BUILT TO HOLD `n` LINES HOLDS THEM. A window of
        // exactly `n × line` pixels, truncated to a whole number, is a hair
        // short — and `floor(height / line)` then answers `n − 1`, which puts
        // the last row at the pixel after the last one there is. That is what
        // `render-png` builds, and it is what made a photograph come out with
        // the footer's values missing.
        let (_, lh) = cell();
        for many in 1..40_u16 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let built = (f32::from(many) * lh).ceil() as u32;
            let rows = Rows::of(&[], built as f32);
            assert!(
                rows.lines >= many,
                "a surface built for {many} lines holds {}",
                rows.lines
            );
        }

        // AND THE GROUND PAINTED FOR THE LAST ROW REACHES THE EDGE — the rule
        // that was already here, asked again now that the row itself has moved.
        let texts = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
        let quads = quads_of(&list, width, height, &texts, 100);
        assert!(
            quads.iter().any(|q| (q.y + q.height - height).abs() < 0.01),
            "no ground reaches the bottom of the surface"
        );
        // AND THE RULE SITS AT THE END OF ITS ROW, which after the move is a
        // line's worth above the values and not a residue more.
        let seam = rows.bottom(lines - 2);
        assert!(
            quads
                .iter()
                .any(|q| q.height < line_h / 2.0 && (q.y + q.height - seam).abs() < 0.01),
            "the rule floats above the row it closes"
        );
    }

    /// AND A BUFFER THAT NEVER DIVIDES ITS WINDOW IS UNCHANGED.
    ///
    /// Five of the six buffers draw on a flat surface. They send no anchor, and
    /// every line of theirs has to land exactly where it always did — a change
    /// here that moved them would be this fix paid for by everybody.
    #[test]
    fn a_frame_with_no_anchor_is_placed_from_the_top() {
        let (_, line_h) = cell();
        let (_, height, lines) = a_window();
        let mut d = a_footer(lines);
        d.commands.retain(|c| !matches!(c, Command::Anchor { .. }));
        let rows = Rows::of(&d.commands, height);
        for line in 0..lines {
            assert!((rows.top(line) - f32::from(line) * line_h).abs() < 0.001);
            assert_eq!(rows.line_at(rows.top(line) + 1.0), Some(line));
        }
        // AND THE LAST GROUND STILL TAKES THE RESIDUE, which it always did.
        assert!((rows.bottom(lines - 1) - height).abs() < 0.001);
    }

    #[test]
    fn the_key_is_translated_and_the_data_is_not() {
        // Invariant 11 and 8 meet here: the component sends a key
        // for a label and a literal for data, and what translates is the host.
        let t = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
        let mut d = DisplayList::fresh_one();
        d.push(Command::Text {
            line: 0,
            column: 0,
            text: Text::key("cmd-mx-open"),
            color: Color::name("text-primary"),
            scale: ronoctua_protocol::Scale::Normal,
            max_width: 0,
        });
        d.push(Command::Text {
            line: 1,
            column: 0,
            text: Text::datum("cmd-mx-open"),
            color: Color::name("text-primary"),
            scale: ronoctua_protocol::Scale::Normal,
            max_width: 0,
        });
        let v = spans_of(&d, &t, 80, 24);
        // pt-BR: the VALUE of `cmd-mx-open` in pt-BR.ftl, which is what the
        // screen says — the key beside it is the name.
        assert_eq!(v[0].text, "Executar comando…");
        assert_eq!(v[1].text, "cmd-mx-open");
    }

    #[test]
    fn nothing_leaks_off_the_edge_nor_over_the_neighbour() {
        // THE DEFECT THIS CATCHES, and it showed up in a photo of the screen: the
        // invitation "Type to search for a Nerd Fonts glyph" running OVER
        // the counter `10473 / 10473`. Text over text is the worst possible
        // result — neither of the two stays legible, and there is no error
        // anywhere.
        //
        // There are TWO limits, and each one alone lets a case through: with no
        // budget, two runs collide; with no edge, a careless buffer
        // writes outside the window and the compositor does not clip.
        for columns in [10_u16, 20, 40, 80, 200] {
            // A long invitation with a short budget, and a counter at the end.
            let budget = columns.saturating_sub(12).max(1);
            let clipped =
                elide_to_budget("Type to search for a Nerd Fonts glyph", budget, 2, columns);
            let width = ronoctua_render::layout::width(&clipped);
            assert!(
                width <= budget,
                "at {columns} columns the text went past its budget: \
                 {width} > {budget}"
            );
            assert!(
                2 + width <= columns,
                "at {columns} columns the text leaked past the edge"
            );
        }
    }

    /// The budget is in CELLS, the cut is on CLUSTERS, and they are not the
    /// same number.
    ///
    /// Every other test here uses Latin-1, where a character, a cluster and a
    /// cell are all one thing — so all of them passed just as happily when
    /// this file measured with `chars()`. The couple is six codepoints, ONE
    /// cluster and TWO cells: cutting it by codepoint would not shorten it but
    /// redraw it as two different emoji, and budgeting it as one cell would
    /// draw it over the column next door.
    #[test]
    fn a_cluster_is_one_thing_and_may_be_two_cells() {
        let couple = "\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}";
        assert_eq!(couple.chars().count(), 6, "six codepoints");
        assert_eq!(
            ronoctua_render::layout::cells_of(couple).count(),
            1,
            "one cluster"
        );
        assert_eq!(ronoctua_render::layout::width(couple), 2, "two cells");
        assert_eq!(elide_to_budget(couple, 4, 0, 80), couple);
        // A BUDGET OF ONE CANNOT HOLD IT, and used to be handed it anyway.
        assert_eq!(elide_to_budget(couple, 1, 0, 80), "\u{2026}");
        // And the ideograph, which is the case this ruler was taught for.
        assert_eq!(ronoctua_render::layout::width("\u{65E5}\u{672C}"), 4);
        assert_eq!(
            elide_to_budget("\u{65E5}\u{672C}", 4, 0, 80),
            "\u{65E5}\u{672C}"
        );
    }

    #[test]
    fn a_zero_budget_means_up_to_the_edge() {
        // `0` is "no budget of its own" — the common case, for whoever is the only
        // thing on the line. The edge still applies.
        let s = "x".repeat(100);
        let clipped = elide_to_budget(&s, 0, 5, 20);
        assert_eq!(
            ronoctua_render::layout::width(&clipped),
            15,
            "it should reach the edge, and stopped"
        );
    }

    #[test]
    fn the_column_counts_up_to_the_edge() {
        // A run starting at column 70 of an 80-wide screen has ten
        // cells, and not eighty. Ignoring the column is like having no edge.
        let s = "y".repeat(50);
        assert_eq!(
            ronoctua_render::layout::width(&elide_to_budget(&s, 0, 70, 80)),
            10
        );
        // And at the exact edge, nothing is left — without panicking.
        assert!(elide_to_budget(&s, 0, 80, 80).is_empty());
        assert!(elide_to_budget(&s, 0, 90, 80).is_empty());
    }

    #[test]
    fn an_unknown_token_is_refused_and_not_papered_over() {
        // This used to return `TEXT_DIM` and carry on. The effect was a buffer with a
        // wrong name drawing nicely, and the defect only showing up when
        // someone found the colour odd.
        //
        // Today the guest cannot even produce an invalid name (over there it is an enum), and
        // if one arrives anyway, it does NOT become any colour.
        assert_eq!(token_color("does-not-exist"), None);
        assert_eq!(token_color(""), None);
    }

    #[test]
    fn the_wit_names_resolve_on_the_host_side() {
        // Both sides come from the same generator. This test catches the divergence
        // that would appear if one of them started converting the name differently —
        // a dot instead of a hyphen, for example.
        for name in [
            "text-primary",
            "text-secondary",
            "rule-default",
            "surface-selection",
        ] {
            assert!(
                token_color(name).is_some(),
                "`{name}` does not resolve, but it is a case of the WIT enum"
            );
        }
        // And the OLD form, with a dot, no longer resolves.
        assert_eq!(token_color("text.primary"), None);
    }

    #[test]
    fn the_rule_reaches_both_edges_and_is_never_a_glyph() {
        // THE DEFECT, and the owner photographed it: the rule went out as
        // `"─".repeat(80)` at column 0, and `with_margin` then pushed it two
        // cells in. So it began two cells from the left edge whatever the
        // window measured, and it was eighty cells long — short of the right
        // edge on a wide window, and spilling past it on anything narrower
        // than eighty-two columns. The compositor does not clip.
        //
        // Two assertions, and the second is the one that would have caught it:
        // a rule produces NO span at all, and the quad it produces starts at
        // zero and is exactly as wide as the window.
        let t = ronoctua_i18n::Texts::fresh("pt-BR").unwrap();
        let (cell_w, line_h) = cell();
        for columns in [10_u16, 46, 80, 110, 200] {
            let mut d = DisplayList::fresh_one();
            d.push(Command::Rule {
                line: 3,
                column: 0,
                columns: 0,
                color: Color::name("rule-default"),
            });
            assert!(
                spans_of(&d, &t, columns, 24).is_empty(),
                "at {columns} columns the rule became text — it is a quad"
            );

            let width_px = f32::from(columns) * cell_w;
            let quads = quads_of(
                &d,
                width_px,
                24.0 * line_h,
                &ronoctua_i18n::Texts::fresh("pt-BR").unwrap(),
                80,
            );
            assert_eq!(quads.len(), 1, "one rule, one quad");
            let q = &quads[0];
            assert!(
                (q.x - 0.0).abs() < f32::EPSILON,
                "at {columns} columns the rule started at {}, and not at the edge",
                q.x
            );
            assert!(
                (q.width - width_px).abs() < f32::EPSILON,
                "at {columns} columns the rule measures {} px in a window of {width_px}",
                q.width
            );
            // AND IT SITS AT THE BOTTOM EDGE OF ITS ROW — which is what makes
            // `Command::Rule { line }` mean "the boundary at the end of row N".
            let bottom = f32::from(3_u16 + ronoctua_tokens::space::CONTENT_INSETY + 1) * line_h;
            assert!(
                (q.y + q.height - bottom).abs() < 0.01,
                "the rule did not reach the end of line 3: {} against {bottom}",
                q.y + q.height
            );
        }
    }

    /// A BAND CAN STOP SHORT, and two of them can share a row.
    ///
    /// THE DEFECT THIS CATCHES is not a defect at all — it is a thing that was
    /// impossible. `Command::Selection` carried a row and a colour and nothing
    /// else, and the reason written beside it was that a ground "paints the
    /// whole row, and that is all". So two columns side by side, each with its
    /// own selection, could not be drawn: not because anybody decided against
    /// it, but because there was nowhere to put the number. The owner said
    /// that arrangement is permitted, and this is what makes the sentence true.
    #[test]
    fn two_bands_can_share_a_row_without_touching() {
        let (cell_w, line_h) = cell();
        let mut d = DisplayList::fresh_one();
        // The left column, ten cells of it, and the right one beside it.
        d.push(Command::Selection {
            line: 0,
            column: 0,
            columns: 10,
            color: Color::name("surface-selection"),
        });
        d.push(Command::Selection {
            line: 0,
            column: 12,
            columns: 8,
            color: Color::name("surface-container"),
        });
        let quads = quads_of(
            &d,
            80.0 * cell_w,
            24.0 * line_h,
            &ronoctua_i18n::Texts::fresh("pt-BR").unwrap(),
            80,
        );
        assert_eq!(quads.len(), 2, "two bands on one row, and one came out");
        let (a, b) = (&quads[0], &quads[1]);
        assert!(
            a.x + a.width <= b.x,
            "the two bands overlap: {} reaches {} and the next starts at {}",
            a.x,
            a.x + a.width,
            b.x
        );
        // AND NEITHER BLEEDS. A band that was asked for a width must not take
        // the window — which is what it always did.
        assert!(
            a.width < 80.0 * cell_w,
            "the first band took the whole line"
        );
        assert!(
            b.width < 80.0 * cell_w,
            "the second band took the whole line"
        );
        // AND THE ONE ASKED FOR NOTHING STILL DOES. `0, 0` is the full-bleed
        // stripe every list row has always drawn, margins included.
        let mut whole = DisplayList::fresh_one();
        whole.push(Command::Selection {
            line: 0,
            column: 0,
            columns: 0,
            color: Color::name("surface-selection"),
        });
        let one = quads_of(
            &whole,
            80.0 * cell_w,
            24.0 * line_h,
            &ronoctua_i18n::Texts::fresh("pt-BR").unwrap(),
            80,
        );
        assert!(
            (one[0].x - 0.0).abs() < f32::EPSILON,
            "a full band left the edge"
        );
        assert!(
            (one[0].width - 80.0 * cell_w).abs() < f32::EPSILON,
            "a full band stopped short"
        );
    }

    #[test]
    fn the_selection_and_the_rule_take_the_same_rail() {
        // Three notions of "the whole width" used to live in this file: the
        // selection painted the window, the rule painted eighty cells from the
        // margin, and the text started at the margin. Two rails now, and the
        // two full-bleed ones have to agree with each other or the argument is
        // just a comment.
        let (cell_w, line_h) = cell();
        let mut d = DisplayList::fresh_one();
        d.push(Command::Selection {
            line: 0,
            column: 0,
            columns: 0,
            color: Color::name("surface-selection"),
        });
        d.push(Command::Rule {
            line: 1,
            column: 0,
            columns: 0,
            color: Color::name("rule-default"),
        });
        let width_px = 80.0 * cell_w;
        let quads = quads_of(
            &d,
            width_px,
            24.0 * line_h,
            &ronoctua_i18n::Texts::fresh("pt-BR").unwrap(),
            80,
        );
        assert_eq!(quads.len(), 2);
        for q in &quads {
            assert!(
                (q.x - 0.0).abs() < f32::EPSILON,
                "one of the two does not bleed"
            );
            assert!(
                (q.width - width_px).abs() < f32::EPSILON,
                "different widths"
            );
        }
    }
}
