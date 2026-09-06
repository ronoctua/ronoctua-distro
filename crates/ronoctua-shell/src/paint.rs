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
/// The translation lives HERE, and not in the buffer: invariant 11 says a buffer does not
/// receive a token value, and it is the same contract colour already fulfils.
///
/// The cell grid is always `Normal`'s — a `Large` run occupies more
/// than one cell, and what spaces it is whatever draws.
const fn scale_in_px(scale: ronoctua_protocol::Scale) -> f32 {
    match scale {
        // `SIZE_SMALL` is the token that already existed for "smaller than the
        // body". Everything else the host draws — the footer, its windows and
        // a buffer's body — is `SIZE_BASE`.
        ronoctua_protocol::Scale::Small => tokens::font::SIZE_SMALL,
        // THE SYSTEM'S SIZE — the same the footer and the minibuffer use.
        ronoctua_protocol::Scale::Base => tokens::font::SIZE_BASE,
        // AND THE BUFFER'S BODY, which is the same size as of the day the
        // owner asked for one measure across the system.
        //
        // The two arms now give the same pixel, and both names stay. `base` in
        // the WIT means "whatever is INTERFACE inside a buffer, not its
        // content" and `normal` means the content — two questions that
        // happened to arrive at one answer, not one question asked twice.
        // Collapsing them is a change to the WIT, which rebuilds every buffer,
        // and nothing asked for it.
        ronoctua_protocol::Scale::Normal => tokens::font::SIZE_BASE,
        ronoctua_protocol::Scale::Large => tokens::font::SIZE_XLARGE,
        ronoctua_protocol::Scale::Huge => tokens::font::SIZE_XXLARGE,
    }
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
) -> Vec<ronoctua_render::Span> {
    // `flat_map`, and not `filter_map`: the grid becomes ONE run per ROW, and it is
    // the only command that produces more than one.
    list.commands
        .iter()
        .flat_map(|c| spans_from(c, texts, columns))
        .map(with_margin)
        .collect()
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
        tokens::font::SIZE_BASE * tokens::font::CELL_WIDTH_EM,
        tokens::font::SIZE_BASE * tokens::font::LINE_HEIGHT,
    )
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
/// A boundary is the opposite: one that stopped at the text's margin would have
/// a beginning and an end, and a thing with a beginning and an end is a box —
/// which invariant 15 forbids. So these two start at `x = 0` and run the whole
/// width, and they never pass through [`with_margin`].
///
/// It used to be called `selections_of`, and the rule was not here: it went out
/// as a line of `─` glyphs, eighty of them, starting at the margin. That is
/// what the owner photographed — a gap on the left, and a spill past the right
/// edge on any window narrower than eighty-two columns.
#[must_use]
pub fn quads_of(
    list: &ronoctua_protocol::DisplayList,
    width_px: f32,
) -> Vec<ronoctua_render::quad::Quad> {
    use ronoctua_protocol::Command;
    let (_, line_height) = cell();
    let top_of =
        |line: u16| f32::from(line.saturating_add(tokens::space::CONTENT_INSETY)) * line_height;
    list.commands
        .iter()
        .filter_map(|c| match c {
            // A CHANGE OF COLOUR, not a box and not an outline (invariant 15):
            // that is why there is no width to fit and no radius — it paints
            // the whole row, and that is all.
            Command::Selection { line, color } => {
                let token = token_color(color.as_str()).or_else(|| {
                    tracing::error!(token = color.as_str(), "an invalid colour in a selection");
                    None
                })?;
                Some(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: top_of(*line),
                    width: width_px,
                    height: line_height,
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
            Command::Rule { line, color } => {
                let token = token_color(color.as_str()).or_else(|| {
                    tracing::error!(token = color.as_str(), "an invalid colour in a rule");
                    None
                })?;
                #[allow(clippy::cast_precision_loss)]
                let thickness = f32::from(tokens::space::RULE_THICKNESS);
                Some(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: top_of(*line) + line_height - thickness,
                    width: width_px,
                    height: thickness,
                    color: token,
                })
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{cell, elide_to_budget, quads_of, spans_of, token_color};
    use ronoctua_protocol::{Color, Command, DisplayList, Text};

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
        let v = spans_of(&d, &t, 80);
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

    /// The budget is in CELLS, and a cell is a cluster.
    ///
    /// Every other test here uses Latin-1, where a character and a cluster are
    /// the same thing — so all of them passed just as happily when this file
    /// measured with `chars()`. The couple is six codepoints and one cell: it
    /// fits in a budget of one, and cutting it by codepoint would not shorten
    /// it but redraw it as two different emoji.
    #[test]
    fn a_cluster_is_one_cell_however_many_codepoints_it_has() {
        let couple = "\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}";
        assert_eq!(couple.chars().count(), 6, "six codepoints");
        assert_eq!(ronoctua_render::layout::width(couple), 1, "one cell");
        assert_eq!(elide_to_budget(couple, 4, 0, 80), couple);
        assert_eq!(elide_to_budget(couple, 1, 0, 80), couple);
        assert!(!elide_to_budget(couple, 1, 0, 80).contains('\u{2026}'));
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
                color: Color::name("rule-default"),
            });
            assert!(
                spans_of(&d, &t, columns).is_empty(),
                "at {columns} columns the rule became text — it is a quad"
            );

            let width_px = f32::from(columns) * cell_w;
            let quads = quads_of(&d, width_px);
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

    #[test]
    fn the_selection_and_the_rule_take_the_same_rail() {
        // Three notions of "the whole width" used to live in this file: the
        // selection painted the window, the rule painted eighty cells from the
        // margin, and the text started at the margin. Two rails now, and the
        // two full-bleed ones have to agree with each other or the argument is
        // just a comment.
        let (cell_w, _) = cell();
        let mut d = DisplayList::fresh_one();
        d.push(Command::Selection {
            line: 0,
            color: Color::name("surface-selection"),
        });
        d.push(Command::Rule {
            line: 1,
            color: Color::name("rule-default"),
        });
        let width_px = 80.0 * cell_w;
        let quads = quads_of(&d, width_px);
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
