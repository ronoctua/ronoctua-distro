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
    let esq = left_over - dir;
    let mut out: String = units[..esq].concat();
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

    // A set's cost: the sum of the natural widths plus the gaps.
    let custo = |v: &[&Segment]| -> u32 {
        if v.is_empty() {
            return 0;
        }
        let text: u32 = v.iter().map(|s| u32::from(width(&s.text))).sum();
        text + u32::from(gap) * (v.len() as u32 - 1)
    };

    // Step 2: it drops the lowest-priority one until it fits. Whoever has
    // `removable = false` never goes — it shrinks to `min_cols` in step 3.
    while custo(&alive) > u32::from(columns) {
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

    let total = |ws: &[u16], n: usize| -> u32 {
        ws.iter().map(|&w| u32::from(w)).sum::<u32>()
            + u32::from(gap) * (n.saturating_sub(1) as u32)
    };

    // Phase A: shrink to the floor, from the least prioritised to the most.
    //
    // An UNREMOVABLE segment's floor is at least 1, even if it declares
    // `min_cols: 0`. `removable: false` with a zero floor is a contradiction —
    // "never disappears" and "zero columns will do" cannot hold together, and
    // phase A resolved the contradiction by making the segment disappear, which is the opposite of what
    // the declaration says. Found by the property test, not by reading.
    for &i in &order {
        let current = total(&widths, alive.len());
        if current <= u32::from(columns) {
            break;
        }
        let excess = current - u32::from(columns);
        let pode = widths[i].saturating_sub(effective_floor(alive[i]));
        widths[i] -= u16::try_from(excess).unwrap_or(u16::MAX).min(pode);
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
        let current = total(&widths, alive.len());
        if current <= u32::from(columns) {
            break;
        }
        let excess = current - u32::from(columns);
        widths[i] -= u16::try_from(excess).unwrap_or(u16::MAX).min(widths[i]);
    }

    // A segment that reached zero is not drawn: an empty column with a
    // gap beside it is noise, not information.
    let sobreviventes: Vec<usize> = (0..alive.len()).filter(|&i| widths[i] > 0).collect();
    let alive: Vec<&Segment> = sobreviventes.iter().map(|&i| alive[i]).collect();
    let widths: Vec<u16> = sobreviventes.iter().map(|&i| widths[i]).collect();

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
    let mut direita: Vec<usize> = (0..alive.len())
        .filter(|&i| alive[i].side == Side::Right)
        .collect();
    direita.sort_by_key(|&i| alive[i].priority);

    let mut dir = columns.saturating_sub(tokens::space::MODELINE_EDGERIGHT);
    for i in direita {
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
        let presentes: Vec<bool> = IDS
            .iter()
            .enumerate()
            .map(|(i, _)| i < 2 || r.up_to(4) > 0)
            .collect();
        IDS.iter()
            .enumerate()
            .filter(|(i, _)| presentes[*i]) // the two fixed ones always go in
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
        for iteracao in 0..5_000 {
            let columns = u16::try_from(r.up_to(200)).unwrap_or(0);
            let segs = draw_segments(&mut r);
            let postos = resolve(columns, &segs);

            // 1. NOTHING spills off the screen. This is section 7's hard invariant: the
            //    `min_cols` floors give way before it does.
            for c in &postos {
                assert!(
                    c.column.saturating_add(width(&c.text)) <= columns,
                    "iteracao {iteracao}: `{}` vaza com {columns} colunas",
                    c.id
                );
            }

            // 2. Nothing OVERLAPS. Two segments in the same cell draw one
            //    over the other, and the result is illegible without being an error.
            let mut ordenados: Vec<_> = postos.iter().collect();
            ordenados.sort_by_key(|c| c.column);
            for pair in ordenados.windows(2) {
                let end = pair[0].column.saturating_add(width(&pair[0].text));
                assert!(
                    end <= pair[1].column,
                    "iteracao {iteracao}: `{}` invade `{}`",
                    pair[0].id,
                    pair[1].id
                );
            }

            // 3. The non-removable ones survive WHENEVER there is width for
            //    their floors. Without that condition the property would be false
            //    with 1 column, and false for a legitimate reason.
            // The EFFECTIVE floor, not the raw `min_cols`: an unremovable one keeps at
            // least one cell (see `effective_floor`).
            let piso: u16 = segs
                .iter()
                .filter(|s| !s.removable)
                .map(effective_floor)
                .sum();
            if columns >= piso.saturating_add(4) {
                for id in ["mode", "notif"] {
                    assert!(
                        postos.iter().any(|c| c.id == id),
                        "iteracao {iteracao}: `{id}` sumiu com {columns} colunas"
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
            assert!(width(&e) <= max, "`{e}` passa de {max}");
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
                "projeto/src/muito/fundo/arquivo.rs",
                4,
                8,
                Side::Left,
                Shrink::InTheMiddle,
                false,
            ),
            seg("estado", "*", 5, 1, Side::Left, Shrink::Cut, true),
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
        assert_eq!(r.len(), 8, "nada deveria sair com 200 colunas");
    }

    #[test]
    fn the_mode_indicator_and_the_notification_never_vanish() {
        // They are the two section 7.2 of the plan marks as "never disappears": without
        // the first the user does not know what state the keyboard is in; without the
        // second there is no way to know something is pending.
        for columns in [8_u16, 12, 20, 40, 80] {
            let r = resolve(columns, &typical_modeline());
            let ids: Vec<_> = r.iter().map(|c| c.id).collect();
            assert!(ids.contains(&"mode"), "'mode' sumiu com {columns} colunas");
            assert!(
                ids.contains(&"notif"),
                "'notif' sumiu com {columns} colunas"
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
    fn nothing_goes_past_the_width() {
        // The symptom this catches: a segment spilling off the screen, which in a
        // compositor with no clipping becomes visual rubbish at the edge.
        for columns in [10_u16, 24, 37, 80, 120, 200] {
            for c in resolve(columns, &typical_modeline()) {
                let end = c.column + width(&c.text);
                assert!(
                    end <= columns,
                    "'{}' termina na coluna {end} numa largura de {columns}",
                    c.id
                );
            }
        }
    }

    #[test]
    fn the_buffers_name_elides_in_the_middle_keeping_the_ends() {
        let e = elide_middle("projeto/src/muito/fundo/arquivo.rs", 16);
        assert_eq!(width(&e), 16);
        assert!(e.contains('…'));
        assert!(e.starts_with("projeto"), "perdeu o comeco: {e}");
        assert!(e.ends_with(".rs"), "perdeu a extensao: {e}");
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
        let notif = r.iter().find(|c| c.id == "notif").expect("notif presente");
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
        let casal = "\u{1F469}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}";
        assert_eq!(width(casal), 1, "seis codepoints, um cluster");
        assert_eq!(elide_end(casal, 4), casal, "coube: nao ha o que cortar");
        assert_eq!(
            elide_end(casal, 1),
            casal,
            "uma celula ainda cabe UM cluster"
        );
        assert!(!elide_end(casal, 1).contains('…'));
    }

    /// The flag, the skin tone and the combining mark — the other three ways
    /// a cluster can have more than one codepoint in this database.
    #[test]
    fn the_other_sequences_are_also_one_unit() {
        for (name, s) in [
            ("bandeira", "\u{1F1E7}\u{1F1F7}"),
            ("tom de pele", "\u{1F468}\u{1F3FF}"),
            ("marca sobre o circulo", "\u{25CC}\u{0301}"),
            ("teclado", "1\u{FE0F}\u{20E3}"),
        ] {
            assert_eq!(width(s), 1, "{name}");
            assert_eq!(elide_end(s, 2), s, "{name}");
        }
    }

    /// And the cut goes on cutting when there is something to cut.
    #[test]
    fn several_clusters_still_elide() {
        let tres = "\u{1F600}\u{1F601}\u{1F602}";
        assert_eq!(width(tres), 3);
        assert_eq!(elide_end(tres, 2), "\u{1F600}…");
        assert_eq!(elide_end("abcdef", 4), "abc…");
    }
}
