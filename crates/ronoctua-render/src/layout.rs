//! Modeline layout: priority and elision — section 4.2.
//!
//! "When a notification is on display, it takes the modeline's message
//! region and the lower-priority segments of the buffer's state
//! shrink or disappear."
//!
//! # Why this is a pure function
//!
//! Everything here is `(width_in_cells, segments) -> positions`. Node GPU, no
//! text measurement, no state. Two consequences:
//!
//! 1. it can be tested headless, and this module's tests run in
//!    milliseconds inside `nix flake check`;
//! 2. it fits section 5.2's "one frame" budget — which only holds because the
//!    cell is exactly 0.600 at and the arithmetic is integer.

use ronoctua_tokens as tokens;

/// Which side of the footer the segment lives on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The focused buffer's state.
    Left,
    /// Tray and notifications. The notification icon sits flush against the right edge.
    Right,
}

/// **How** the segment shortens — not *whether* it can disappear.
///
/// The two are independent, and confusing them was a actual bug: the buffer's
/// name "elides, never disappears entirely" (section 7.2 of the plan), which is
/// [`Shrink::InTheMiddle`] **with** [`Segment::removable`] false. A single enum cannot
/// say that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shrink {
    /// A hard cut, with no ellipsis. For short text where the ellipsis
    /// would cost more than the character it replaces.
    Cut,
    /// Elides in the MIDDLE, preserving the ends (`proj/…/file.rs`).
    /// Only the buffer's name uses this.
    InTheMiddle,
    /// Elides at the end. The message region uses this.
    AtTheEnd,
}

/// A modeline segment.
#[derive(Debug, Clone)]
pub struct Segment {
    /// A stable identifier, for the tests and for the log.
    pub id: &'static str,
    /// The already-resolved text (i18n applied, tokens resolved).
    pub text: String,
    /// The lower number wins — that is, survives longest.
    pub priority: u8,
    /// Below this the segment is no use to anyone.
    pub min_cols: u16,
    /// Which side.
    pub side: Side,
    /// **How** it shortens.
    pub shrink: Shrink,
    /// Whether it can be **removed entirely** when space runs out.
    ///
    /// False for the three section 7.2 of the plan marks: the mode
    /// indicator (without it one does not know the keyboard's state), the notification
    /// icon (the only sign that something is pending) and the buffer's name
    /// (it elides, but never disappears). Those shrink to `min_cols` and stop.
    pub removable: bool,
}

/// Where a segment ended up, after elision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// The segment's `id`.
    pub id: &'static str,
    /// The starting column, from the left.
    pub column: u16,
    /// The **already elided** text, ready to draw.
    pub text: String,
    /// Whether the segment was placed counting from the RIGHT edge.
    ///
    /// The layout resolves both sides in cells, and the cells do not reach the
    /// border: `columns` is `floor(width / cell)` and the remainder sits past
    /// the last one. A run placed from the left never notices; one that should
    /// sit AGAINST the border inherits that remainder as a gap which changes
    /// with the window's width — which is what the owner measured by taking a
    /// window fullscreen. Carried here so the painter can place it in pixels
    /// from the border instead. See `ronoctua_render::Span::edge`.
    pub edge: bool,
}

/// The width floor the segment is entitled to in phase A.
///
/// An unremovable segment keeps at least one cell: it declared that it
/// never disappears. See phase A's comment in [`resolve`].
fn effective_floor(s: &Segment) -> u16 {
    if s.removable {
        s.min_cols
    } else {
        s.min_cols.max(1)
    }
}

/// The units the cut may separate: CLUSTERS, not codepoints.
///
/// # Why not `chars()`
///
/// Because a cluster is what the renderer draws as ONE thing, and there are far
/// more than one codepoint in them since the emoji face came in. `👩‍❤️‍👨` is
/// six codepoints and one glyph; `◌́` is two and one; a flag is two and one.
///
/// Cutting by codepoint did not "shorten too much": it produced ANOTHER drawing. With
/// a budget of 4 cells, the couple became `👩‍❤` + `…` — which reshapes as
/// **two** emoji, a woman and a heart, plus the ellipsis. It showed up on the
/// symbol browser's screen in hundreds of cells at once, and none of
/// them looked like a cutting mistake: they looked like the wrong emoji.
fn units(s: &str) -> impl Iterator<Item = &str> {
    unicode_segmentation::UnicodeSegmentation::graphemes(s, true)
}

/// The clusters of a text, one per cell.
///
/// The public face of [`units`], for the two places outside this module that
/// have to walk cells rather than count them: the highlight runs and the word
/// wrap. Both used to walk `chars()` and both were wrong in the same way —
/// see `surface::runs_of`.
pub fn cells_of(s: &str) -> impl Iterator<Item = &str> {
    units(s)
}

/// Width in cells. It counts CLUSTERS, not bytes and not codepoints.
///
/// It holds because the font is genuinely monospaced: every cluster takes one
/// cell, including the 10,473 Nerd Fonts glyphs (Mono variant). In a
/// proportional font this would be wrong, and that is why the `fonts-metrics` check
/// reasserts the monospacing on every build.
///
/// Emoji are the known exception: their advance is `1.245 at` against the cell's
/// `0.600`, so an emoji cluster takes more than one. What knows that is the
/// database's generator, which marks `width: 2` — here the count is still
/// "how many things there are", which is what the cut needs to know.
#[must_use]
pub fn width(s: &str) -> u16 {
    u16::try_from(units(s).count()).unwrap_or(u16::MAX)
}

/// Is this a CHROME GLYPH — an icon from the font, and not a letter?
///
/// The Nerd Fonts glyphs all live in a private use area: the BMP's
/// (`U+E000..U+F8FF`) and plane 15's (`U+F0000..U+FFFFD`). Nothing a person
/// types and nothing an application puts in a window title lands there, so the
/// range answers the question without a table to keep in step with
/// `icons.nix`.
///
/// It exists because a glyph and the text beside it are drawn at DIFFERENT
/// sizes (see `font.size.icon`), and the drawing has to know where one ends.
#[must_use]
pub const fn is_chrome_glyph(c: char) -> bool {
    matches!(c, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}')
}

/// Splits a leading chrome glyph off the rest.
///
/// `Some((glyph, rest))` when the text starts with one — the bell and its
/// count, the transfer's arrow and its percentage. `None` when it does not, and
/// then the run is drawn whole, at one size.
///
/// It splits ONE cluster, not a run of them: what the modeline puts in front
/// of text is a single icon, and a second one would be a second thing to line
/// up against the grid.
#[must_use]
pub fn leading_glyph(s: &str) -> Option<(&str, &str)> {
    let first = units(s).next()?;
    if !first.chars().next().is_some_and(is_chrome_glyph) {
        return None;
    }
    Some(s.split_at(first.len()))
}

/// Elides in the middle, preserving both ends.
#[must_use]
pub fn elide_middle(s: &str, max: u16) -> String {
    let n = width(s);
    if n <= max {
        return s.to_owned();
    }
    if max <= 1 {
        return "…".repeat(max as usize);
    }
    let units: Vec<&str> = units(s).collect();
    // -1 for the ellipsis. An uneven remainder goes to the LEFT: the start of
    // the path identifies better than the end.
    let left_over = (max - 1) as usize;
    let dir = left_over / 2;
    let left = left_over - dir;
    let mut out: String = units[..left].concat();
    out.push('…');
    out.push_str(&units[units.len() - dir..].concat());
    out
}

/// Elides at the end.
#[must_use]
pub fn elide_end(s: &str, max: u16) -> String {
    let n = width(s);
    if n <= max {
        return s.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = units(s).take((max - 1) as usize).collect();
    out.push('…');
    out
}

/// The window of `max` cells starting at cell `from`.
///
/// # Why this is not `&s[from..from + max]`
///
/// Because those are BYTES, and this counts CLUSTERS — the same ruler
/// [`width`] and [`elide_end`] use. Cutting by byte lands inside a character
/// and invents a drawing; cutting by codepoint splits a cluster and invents a
/// different one, which is the trap `AGENTS.md` records about eliding.
///
/// What it is for: a preview panel is twenty-four cells at its floor and a
/// line of code is not, so the panel shows a window onto the line and the
/// owner moves it sideways. Everything from `from` on is taken, up to `max`
/// cells; nothing is added and nothing is marked, because the bar and the hint
/// line are what say there is more.
#[must_use]
pub fn window(s: &str, from: u16, max: u16) -> String {
    if max == 0 {
        return String::new();
    }
    units(s).skip(from as usize).take(max as usize).collect()
}

/// Breaks a text into lines of up to `width` cells.
///
/// It breaks by WORD when it can, and in the middle of the word when the word
/// is longer than the line — the case of a long address, which is precisely
/// what gets copied most. Always cutting at the limit would split short words
/// for nothing; never cutting would leave the address invisible.
///
/// # Why it returns EVERYTHING, and what `ceiling` is for
///
/// It lived in `surface.rs` and stopped the moment it had `ceiling` lines,
/// which is all its one caller — the clipboard's preview — ever wanted. A
/// caller that puts a SCROLLBAR on the result needs the other half of the
/// answer: how many lines there are in all, which is a bar's `total`. Stopping
/// early cannot tell you that.
///
/// So the cut moved to the caller (`.take(…)`), and `ceiling` stayed as what
/// it always really was: a bound on work, not a layout decision. A clipboard
/// entry can be a megabyte, and wrapping a megabyte to answer a question about
/// six lines is the kind of cost that only shows up on somebody else's
/// machine. When it bites, the last line is not the last line of the text —
/// the caller is expected to say so, exactly as the preview panel already says
/// it cut.
#[must_use]
pub fn wrap(text: &str, width: u16, ceiling: usize) -> Vec<String> {
    let width = usize::from(width.max(1));
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut line = String::new();
        for word in raw.split_whitespace() {
            if line.is_empty() && usize::from(self::width(word)) > width {
                // WORD LONGER THAN THE LINE: split it into pieces the size of the
                // line. By CLUSTER, and not by character: this is a cut, and a
                // cut between the two halves of an emoji sequence does not
                // shorten the word, it draws a different one — the same fault
                // `elide_end` exists to avoid.
                let mut rest: Vec<&str> = cells_of(word).collect();
                while rest.len() > width && out.len() < ceiling {
                    out.push(rest.drain(..width).collect());
                }
                line = rest.into_iter().collect();
                continue;
            }
            let fits =
                usize::from(self::width(&line)) + 1 + usize::from(self::width(word)) <= width;
            if line.is_empty() {
                line.push_str(word);
            } else if fits {
                line.push(' ');
                line.push_str(word);
            } else {
                out.push(std::mem::take(&mut line));
                line.push_str(word);
            }
            if out.len() >= ceiling {
                return out;
            }
        }
        out.push(line);
        if out.len() >= ceiling {
            return out;
        }
    }
    out
}

/// Resolves the modeline's layout.
///
/// # The rule (section 7.3 of the plan)
///
/// 1. The desired width is summed.
/// 2. While it overflows: the lowest-priority present segment (highest
///    number) is **removed entirely**. A segment either fits in `min_cols`, or it goes.
/// 3. When only segments that cannot go are left, they shrink to
///    `min_cols`, in reverse priority order.
///
/// The result is deterministic: same input, same layout. It is what
/// makes it possible to test elision without bringing anything up.
#[must_use]
pub fn resolve(columns: u16, segments: &[Segment]) -> Vec<Placed> {
    let gap = tokens::space::MODELINE_SEGMENTGAP;
    let mut alive: Vec<&Segment> = segments.iter().collect();

    // THE TWO EDGES ARE NOT THE LINE'S TO SPEND. The left side starts
    // `modeline.edgeLeft` in and the right side ends `modeline.edgeRight`
    // before the screen, so what the segments and their gaps may take is the
    // width less both. It used to compare against the whole width, and the
    // arithmetic only held because a right-hand segment always survived: a
    // line whose only survivor was on the LEFT was shrunk to exactly
    // `columns`, then drawn one cell in — one cell off the screen. The bell
    // leaving the line at rest is what made such a line possible.
    let budget = u32::from(
        columns
            .saturating_sub(tokens::space::MODELINE_EDGELEFT + tokens::space::MODELINE_EDGERIGHT),
    );

    // A set's cost: the sum of the natural widths plus the gaps.
    let cost = |v: &[&Segment]| -> u32 {
        if v.is_empty() {
            return 0;
        }
        let text: u32 = v.iter().map(|s| u32::from(width(&s.text))).sum();
        text + u32::from(gap) * (v.len() as u32 - 1)
    };

    // Step 2: it drops the lowest-priority one until it fits. Whoever has
    // `removable = false` never goes — it shrinks to `min_cols` in step 3.
    while cost(&alive) > budget {
        let Some((idx, _)) = alive
            .iter()
            .enumerate()
            .filter(|(_, s)| s.removable)
            .max_by_key(|(_, s)| s.priority)
        else {
            break; // only unremovable ones are left; move on to shrinking
        };
        alive.remove(idx);
    }

    // Step 3: if it still overflows, shrink from the least prioritised to the most.
    let mut widths: Vec<u16> = alive.iter().map(|s| width(&s.text)).collect();
    let mut order: Vec<usize> = (0..alive.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(alive[i].priority));

    // A GAP ONLY BETWEEN SEGMENTS THAT ARE STILL DRAWN. One that phase B took
    // to zero is not drawn (see below), and charging the line for the air
    // beside a segment that is not there took the bell away at eight columns
    // while three cells sat empty — measured, once the edges were reserved.
    let total = |ws: &[u16]| -> u32 {
        let live = ws.iter().filter(|&&w| w > 0).count();
        ws.iter().map(|&w| u32::from(w)).sum::<u32>()
            + u32::from(gap) * (live.saturating_sub(1) as u32)
    };

    // Phase A: shrink to the floor, from the least prioritised to the most.
    //
    // An UNREMOVABLE segment's floor is at least 1, even if it declares
    // `min_cols: 0`. `removable: false` with a zero floor is a contradiction —
    // "never disappears" and "zero columns will do" cannot hold together, and
    // phase A resolved the contradiction by making the segment disappear, which is the opposite of what
    // the declaration says. Found by the property test, not by reading.
    for &i in &order {
        let current = total(&widths);
        if current <= budget {
            break;
        }
        let excess = current - budget;
        let can = widths[i].saturating_sub(effective_floor(alive[i]));
        widths[i] -= u16::try_from(excess).unwrap_or(u16::MAX).min(can);
    }

    // Phase B: if it STILL overflows, the floor gives way — including the unremovable ones'.
    //
    // That happens when only unremovable segments are left and not even
    // their floors fit — on a very narrow screen, for example. `min_cols` is a
    // PREFERENCE floor; what cannot give way is "nothing spills off the screen", which
    // in a compositor with no clipping would become visual rubbish at the edge.
    //
    // Here phase A's minimum floor of 1 gives way too: with 3 columns and 4
    // segments that "never disappear", somebody disappears anyway. The choice
    // is deliberate and the order is by priority, so the one that falls is always the
    // least important.
    //
    // Without this phase, the `nothing_goes_past_the_width` test fails from
    // ~14 columns on — which is exactly how this bug showed up.
    for &i in &order {
        let current = total(&widths);
        if current <= budget {
            break;
        }
        let excess = current - budget;
        widths[i] -= u16::try_from(excess).unwrap_or(u16::MAX).min(widths[i]);
    }

    // A segment that reached zero is not drawn: an empty column with a
    // gap beside it is noise, not information.
    let survivors: Vec<usize> = (0..alive.len()).filter(|&i| widths[i] > 0).collect();
    let alive: Vec<&Segment> = survivors.iter().map(|&i| alive[i]).collect();
    let widths: Vec<u16> = survivors.iter().map(|&i| widths[i]).collect();

    // Positioning: the left from the left edge, the right flush against the right.
    let mut out = Vec::with_capacity(alive.len());
    let mut x = tokens::space::MODELINE_EDGELEFT;

    for (i, s) in alive.iter().enumerate() {
        if s.side != Side::Left {
            continue;
        }
        let w = widths[i];
        out.push(Placed {
            id: s.id,
            column: x,
            text: match s.shrink {
                Shrink::InTheMiddle => elide_middle(&s.text, w),
                _ => elide_end(&s.text, w),
            },
            edge: false,
        });
        x += w + gap;
    }

    // The right side is laid out from the edge inwards, and the ORDER is by PRIORITY,
    // not by declaration order.
    //
    // Section 4.2 is literal: "From right to left: 1. the notifications
    // icon, flush against the right edge. 2. Tray, immediately to the
    // left of that icon." Iterating the list backwards put the
    // tray at the edge when it was declared later — which went
    // unnoticed until a test measured the final column.
    let mut right: Vec<usize> = (0..alive.len())
        .filter(|&i| alive[i].side == Side::Right)
        .collect();
    right.sort_by_key(|&i| alive[i].priority);

    let mut dir = columns.saturating_sub(tokens::space::MODELINE_EDGERIGHT);
    for i in right {
        let s = alive[i];
        let w = widths[i];
        dir = dir.saturating_sub(w);
        out.push(Placed {
            id: s.id,
            column: dir,
            text: match s.shrink {
                Shrink::InTheMiddle => elide_middle(&s.text, w),
                _ => elide_end(&s.text, w),
            },
            // FROM THE BORDER, in pixels, and not from the last cell — see
            // `Placed::edge`. This side was laid out from the edge inwards
            // already; what was missing is that the edge of the GRID and the
            // edge of the SURFACE are not the same place.
            edge: true,
        });
        dir = dir.saturating_sub(gap);
    }

    out
}

#[cfg(test)]
mod tests {

    // ------------------------------------------------------------------
    // PROPERTY test — milestone 5's acceptance criterion
    // ------------------------------------------------------------------
    //
    // The example tests above cover cases somebody thought of. These
    // cover the ones nobody thought of: thousands of combinations of width,
    // text and priority, checking section 7 of the plan's invariants in
    // all of them.
    //
    // Our own PRNG, 4 lines long, instead of `proptest` or `quickcheck`. It is not
    // asceticism: `ronoctua-render` is a dependency of the shell, and a dev
    // dependency here enters everybody's build closure. Xorshift with a fixed seed
    // also makes the failure **reproducible** without recording anything — the same
    // case breaks on the same iteration.

    #[test]
    fn a_leading_glyph_comes_off_the_text() {
        use super::{is_chrome_glyph, leading_glyph};
        // A chrome glyph followed by digits — the shape a segment with a
        // number takes (a transfer's `62%`, a device's `12%`).
        let bell = ronoctua_tokens::icon::NOTIFICATION_PENDING;
        let line = format!("{bell}12");
        let (g, rest) = leading_glyph(&line).expect("the bell is a chrome glyph");
        assert_eq!(g, bell.to_string());
        assert_eq!(rest, "12");
        // Plane 15 counts too: the countdown pie lives there.
        assert!(is_chrome_glyph(ronoctua_tokens::icon::COUNTDOWN_EIGHT));
        // And ordinary text is left whole — otherwise a buffer's name would
        // have its first letter drawn at another size.
        assert!(leading_glyph("file.rs").is_none());
        assert!(leading_glyph("").is_none());
        // An emoji is NOT chrome: its colour is inside the glyph and it is
        // content, not an icon of ours.
        assert!(!is_chrome_glyph('\u{1F600}'));
    }

    /// Xorshift64*. Deterministic on purpose.
    struct Draw(u64);

    impl Draw {
        fn next_one(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn up_to(&mut self, n: u64) -> u64 {
            self.next_one() % n.max(1)
        }

        /// Text of arbitrary length, with accents and a Nerd Fonts glyph —
        /// precisely the cases where "byte" and "cell" diverge.
        ///
        /// NEVER empty. A segment with no text is not a segment: the resolve
        /// does not place it, and rightly so — there is nothing to draw. Letting the
        /// draw produce an empty string made the property "the
        /// non-removable ones survive" fail for a reason that is not a defect.
        fn text(&mut self, max: u64) -> String {
            const ALPHABET: &[char] = &['a', 'z', 'ç', 'á', '/', '.', '-', '\u{f07b}', ' '];
            let n = self.up_to(max).max(1) as usize;
            (0..n)
                .map(|_| ALPHABET[self.up_to(ALPHABET.len() as u64) as usize])
                .collect()
        }
    }

    fn draw_segments(r: &mut Draw) -> Vec<Segment> {
        const IDS: &[&str] = &["mode", "notif", "buffer", "kind", "state", "position"];
        // Drawing the presence BEFORE assembling: two closures borrowing `r`
        // at the same time does not pass the borrow checker, and the
        // compiler's message does not suggest this.
        let present: Vec<bool> = IDS
            .iter()
            .enumerate()
            .map(|(i, _)| i < 2 || r.up_to(4) > 0)
            .collect();
        IDS.iter()
            .enumerate()
            .filter(|(i, _)| present[*i]) // the two fixed ones always go in
            .map(|(i, id)| Segment {
                id,
                text: r.text(60),
                priority: u8::try_from(i).unwrap_or(u8::MAX),
                min_cols: u16::try_from(r.up_to(10)).unwrap_or(0),
                side: if *id == "notif" {
                    Side::Right
                } else {
                    Side::Left
                },
                shrink: match r.up_to(3) {
                    0 => Shrink::Cut,
                    1 => Shrink::InTheMiddle,
                    _ => Shrink::AtTheEnd,
                },
                // `mode` and `notif` never disappear (section 7.2 of the plan).
                removable: i >= 2,
            })
            .collect()
    }

    #[test]
    fn property_nothing_leaks_and_nothing_overlaps() {
        let mut r = Draw(0x2026_0801);
        for iteration in 0..5_000 {
            let columns = u16::try_from(r.up_to(200)).unwrap_or(0);
            let segs = draw_segments(&mut r);
            let placed = resolve(columns, &segs);

            // 1. NOTHING spills off the screen. This is section 7's hard invariant: the
            //    `min_cols` floors give way before it does.
            for c in &placed {
                assert!(
                    c.column.saturating_add(width(&c.text)) <= columns,
                    "iteration {iteration}: `{}` leaks at {columns} columns",
                    c.id
                );
            }

            // 2. Nothing OVERLAPS. Two segments in the same cell draw one
            //    over the other, and the result is illegible without being an error.
            let mut sorted: Vec<_> = placed.iter().collect();
            sorted.sort_by_key(|c| c.column);
            for pair in sorted.windows(2) {
                let end = pair[0].column.saturating_add(width(&pair[0].text));
                assert!(
                    end <= pair[1].column,
                    "iteration {iteration}: `{}` invades `{}`",
                    pair[0].id,
                    pair[1].id
                );
            }

            // 3. The non-removable ones survive WHENEVER there is width for
            //    their floors. Without that condition the property would be false
            //    with 1 column, and false for a legitimate reason.
            // The EFFECTIVE floor, not the raw `min_cols`: an unremovable one keeps at
            // least one cell (see `effective_floor`).
            let floor: u16 = segs
                .iter()
                .filter(|s| !s.removable)
                .map(effective_floor)
                .sum();
            if columns >= floor.saturating_add(4) {
                for id in ["mode", "notif"] {
                    assert!(
                        placed.iter().any(|c| c.id == id),
                        "iteration {iteration}: `{id}` went away at {columns} columns"
                    );
                }
            }
        }
    }

    #[test]
    fn property_it_is_deterministic() {
        // Two identical calls have to give the same layout. If it depended on
        // hash order or on time, the modeline would shimmer between frames with
        // nothing having changed.
        let mut r = Draw(0xC0FF_EE01);
        for _ in 0..1_000 {
            let columns = u16::try_from(r.up_to(200)).unwrap_or(0);
            let segs = draw_segments(&mut r);
            let a = resolve(columns, &segs);
            let b = resolve(columns, &segs);
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(&b) {
                assert_eq!((x.id, x.column, &x.text), (y.id, y.column, &y.text));
            }
        }
    }

    #[test]
    fn property_eliding_keeps_the_ends() {
        // `elide_middle` exists for the buffer's name: `proj/…/file.rs`. If
        // one of the ends were lost, the segment would stop serving its
        // purpose — and an example test would only cover one length.
        let mut r = Draw(0xBEEF_0501);
        for _ in 0..2_000 {
            let s = r.text(40);
            let max = u16::try_from(r.up_to(40)).unwrap_or(0);
            let e = elide_middle(&s, max);
            assert!(width(&e) <= max, "`{e}` runs past {max}");
            // With room for both ends plus the ellipsis, the original's first
            // and last letter have to still be there.
            if width(&s) > max && max >= 5 {
                let mut orig = s.chars();
                let mut elid = e.chars();
                assert_eq!(orig.next(), elid.next());
                assert_eq!(orig.next_back(), elid.next_back());
            }
        }
    }
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn seg(
        id: &'static str,
        text: &str,
        prio: u8,
        min: u16,
        side: Side,
        e: Shrink,
        removable: bool,
    ) -> Segment {
        Segment {
            id,
            text: text.to_owned(),
            priority: prio,
            min_cols: min,
            side,
            shrink: e,
            removable,
        }
    }

    /// Section 7.2 of the plan's priority order, from what survives
    /// longest to what dies first.
    fn typical_modeline() -> Vec<Segment> {
        // The three with `removable = false` are the ones section 7.2 of the plan
        // marks as "never disappears" / "never disappears entirely".
        vec![
            seg("mode", "NORMAL", 1, 3, Side::Left, Shrink::Cut, false),
            seg("notif", "\u{EAA2}3", 2, 3, Side::Right, Shrink::Cut, false),
            seg(
                "buffer",
                "project/src/very/deep/file.rs",
                4,
                8,
                Side::Left,
                Shrink::InTheMiddle,
                false,
            ),
            seg("state", "*", 5, 1, Side::Left, Shrink::Cut, true),
            seg("kind", "rust", 6, 4, Side::Left, Shrink::Cut, true),
            seg(
                "tray",
                "\u{F0}\u{F1}\u{F2}",
                7,
                0,
                Side::Right,
                Shrink::AtTheEnd,
                true,
            ),
            seg("position", "42:17", 8, 5, Side::Left, Shrink::Cut, true),
            seg(
                "state",
                "permission",
                9,
                6,
                Side::Left,
                Shrink::AtTheEnd,
                true,
            ),
        ]
    }

    #[test]
    fn with_room_to_spare_nothing_is_removed() {
        let r = resolve(200, &typical_modeline());
        assert_eq!(r.len(), 8, "nothing should go away at 200 columns");
    }

    #[test]
    fn the_mode_indicator_and_the_notification_never_vanish() {
        // They are the two section 7.2 of the plan marks as "never disappears": without
        // the first the user does not know what state the keyboard is in; without the
        // second there is no way to know something is pending.
        for columns in [8_u16, 12, 20, 40, 80] {
            let r = resolve(columns, &typical_modeline());
            let ids: Vec<_> = r.iter().map(|c| c.id).collect();
            assert!(
                ids.contains(&"mode"),
                "'mode' went away at {columns} columns"
            );
            assert!(
                ids.contains(&"notif"),
                "'notif' went away at {columns} columns"
            );
        }
    }

    #[test]
    fn the_lowest_priority_dies_before_the_buffers_name() {
        // Priority 9 against 4: the lowest is the first to go.
        let r = resolve(40, &typical_modeline());
        let ids: Vec<_> = r.iter().map(|c| c.id).collect();
        assert!(!ids.contains(&"state"));
        assert!(ids.contains(&"buffer"));
    }

    #[test]
    fn a_lone_segment_on_the_left_respects_the_edge() {
        // THE CASE THE PROPERTY TEST CANNOT DRAW: it always keeps a right-hand
        // segment alive, and with one there the arithmetic happened to hold.
        // With the bell gone at rest the line can be one long name and nothing
        // else, and that name was shrunk to the whole width and drawn one
        // cell in — one cell off the screen.
        let name = "a".repeat(300);
        for columns in [8_u16, 20, 40, 80] {
            let only = vec![seg(
                "buffer",
                &name,
                4,
                8,
                Side::Left,
                Shrink::InTheMiddle,
                false,
            )];
            for c in resolve(columns, &only) {
                assert!(
                    c.column + width(&c.text) <= columns - tokens::space::MODELINE_EDGERIGHT,
                    "'{}' ends at {} in a width of {columns}",
                    c.id,
                    c.column + width(&c.text)
                );
            }
        }
    }

    #[test]
    fn nothing_goes_past_the_width() {
        // The symptom this catches: a segment spilling off the screen, which in a
        // compositor with no clipping becomes visual rubbish at the edge.
        for columns in [10_u16, 24, 37, 80, 120, 200] {
            for c in resolve(columns, &typical_modeline()) {
                let end = c.column + width(&c.text);
                assert!(
                    end <= columns,
                    "'{}' ends at column {end} in a width of {columns}",
                    c.id
                );
            }
        }
    }

    #[test]
    fn the_buffers_name_elides_in_the_middle_keeping_the_ends() {
        let e = elide_middle("project/src/very/deep/file.rs", 16);
        assert_eq!(width(&e), 16);
        assert!(e.contains('…'));
        assert!(e.starts_with("project"), "it lost the start: {e}");
        assert!(e.ends_with(".rs"), "it lost the extension: {e}");
    }

    #[test]
    fn eliding_never_goes_past_the_limit() {
        for max in 0_u16..40 {
            assert!(width(&elide_middle("abcdefghijklmnopqrstuvwxyz", max)) <= max);
            assert!(width(&elide_end("abcdefghijklmnopqrstuvwxyz", max)) <= max);
        }
    }

    #[test]
    fn text_that_already_fits_is_not_touched() {
        assert_eq!(elide_middle("curto", 20), "curto");
        assert_eq!(elide_end("curto", 20), "curto");
    }

    #[test]
    fn the_width_counts_a_nerd_fonts_glyph_as_one_cell() {
        // They are one cell wide in the Mono variant — measured in the inventory. If
        // they counted as two, the whole right-hand layout would come out shifted.
        assert_eq!(width("\u{F0}\u{F1}\u{F2}"), 3);
        assert_eq!(width("\u{EAA2}3"), 2);
    }

    #[test]
    fn the_notification_is_flush_with_the_right_edge() {
        // Section 4.2: "flush against the right edge".
        let columns = 80;
        let r = resolve(columns, &typical_modeline());
        let notif = r
            .iter()
            .find(|c| c.id == "notif")
            .expect("notif is present");
        assert_eq!(
            notif.column + width(&notif.text),
            columns - tokens::space::MODELINE_EDGERIGHT
        );
    }

    #[test]
    fn is_deterministic() {
        let a = resolve(37, &typical_modeline());
        let b = resolve(37, &typical_modeline());
        assert_eq!(a, b);
    }

    /// THE COUPLE WITH A HEART, which appeared wrong on screen in hundreds of cells.
    ///
    /// `👩‍❤️‍👨` is SIX codepoints and ONE glyph. Cutting by codepoint with
    /// a budget of 4 left `👩‍❤` + `…` — which reshapes as TWO emoji, a
    /// woman and a heart. It did not look like a cut: it looked like a wrong emoji.
    #[test]
    fn an_emoji_cluster_is_not_split() {
        let couple = "\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}";
        assert_eq!(width(couple), 1, "six codepoints, one cluster");
        assert_eq!(
            elide_end(couple, 4),
            couple,
            "it fitted: there is nothing to cut"
        );
        assert_eq!(
            elide_end(couple, 1),
            couple,
            "one cell still fits ONE cluster"
        );
        assert!(!elide_end(couple, 1).contains('…'));
    }

    /// The flag, the skin tone and the combining mark — the other three ways
    /// a cluster can have more than one codepoint in this database.
    #[test]
    fn the_other_sequences_are_also_one_unit() {
        for (name, s) in [
            ("flag", "\u{1F1E7}\u{1F1F7}"),
            ("tom de pele", "\u{1F468}\u{1F3FF}"),
            ("a mark over the circle", "\u{25CC}\u{0301}"),
            ("keyboard", "1\u{FE0F}\u{20E3}"),
        ] {
            assert_eq!(width(s), 1, "{name}");
            assert_eq!(elide_end(s, 2), s, "{name}");
        }
    }

    /// And the cut goes on cutting when there is something to cut.
    #[test]
    fn several_clusters_still_elide() {
        let three = "\u{1F600}\u{1F601}\u{1F602}";
        assert_eq!(width(three), 3);
        assert_eq!(elide_end(three, 2), "\u{1F600}…");
        assert_eq!(elide_end("abcdef", 4), "abc…");
    }

    #[test]
    fn a_word_longer_than_the_line_is_cut_by_cluster() {
        // The long address, which is what gets copied most. Cutting between the
        // halves of an emoji sequence does not shorten the word, it draws a
        // different one — the fault `elide_end` exists to avoid.
        let out = wrap("\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}", 2, 99);
        assert_eq!(
            out,
            vec!["\u{1F600}\u{1F600}", "\u{1F600}\u{1F600}", "\u{1F600}"]
        );
        for l in &out {
            assert!(width(l) <= 2, "{l} is wider than the line");
        }
    }

    #[test]
    fn it_breaks_by_word_when_it_can() {
        let out = wrap("one two three four", 9, 99);
        assert_eq!(out, vec!["one two", "three", "four"]);
    }

    #[test]
    fn it_says_how_many_lines_there_are_in_all() {
        // THE REASON IT MOVED HERE. It used to stop at the ceiling, which
        // cannot answer "how many are there" — and that number is a
        // scrollbar's `total`. The ceiling now bounds the WORK, and the
        // caller cuts.
        let text = "a b c d e f g h";
        assert_eq!(wrap(text, 1, 99).len(), 8);
        assert_eq!(wrap(text, 3, 99), vec!["a b", "c d", "e f", "g h"]);
    }

    #[test]
    fn the_ceiling_bounds_the_work_and_nothing_else() {
        let text = "a b c d e f g h";
        let cut = wrap(text, 1, 3);
        assert_eq!(cut.len(), 3);
        assert_eq!(cut, wrap(text, 1, 99)[..3]);
    }

    #[test]
    fn a_line_of_its_own_survives_the_wrap() {
        // `text.lines()` first, then words: a paragraph break is the author's
        // and the wrapper does not join it away.
        assert_eq!(wrap("one\ntwo", 40, 99), vec!["one", "two"]);
    }

    #[test]
    fn a_width_of_zero_does_not_spin() {
        assert!(!wrap("abc def", 0, 99).is_empty());
    }
}
