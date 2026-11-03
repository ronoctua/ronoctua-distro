//! THE GROUP'S STRIP: with two terminals or more in one window, a thin line
//! per terminal across the window's top — and the only file that draws it
//! (`one-terminal-strip`).
//!
//! # The owner's exception to invariant 4 (2026-11-02)
//!
//! A window of the shell shows one thing, and the compositor places windows.
//! A terminal's window may HOLD several terminals and show one of them: the
//! terminals a person opened together stay together, go to the background
//! together and come back together. What tells them apart is this strip — a
//! line each, nothing written on it (a title is the window's, and the line
//! under the pointer lends its terminal's title to the window while it is
//! there) — and it exists only from two up: one terminal has nothing to be
//! told apart from. Splitting a window stays the compositor's.
//!
//! # Its pixels come off the grid
//!
//! The strip is laid over no row: the grid starts under it, so a group that
//! grows to two (or shrinks to one) is a grid one row shorter (or taller) —
//! one `SIGWINCH` to every terminal of the group, once, which is honest about
//! where the rows went ([`height`], `terminal_pointer::Placed::under`).
//!
//! # A click fronts
//!
//! The strip's whole height is the target, cut in as many equal parts as
//! there are lines, so a line two pixels thick is not a target two pixels
//! tall ([`Strip::line_at`]). THE FIRST CLICK FRONTS: invariant 19's first
//! click selects and the second acts, and here being in front IS what a line
//! is selected for — a workspace's number on the modeline is the same answer
//! (the exception's own wording). The wheel over the strip walks the group.
//!
//! # Its tones are roles there already
//!
//! The one in front in `text.primary`, the one under the pointer in
//! `text.secondary`, the rest in `text.dim` — the text ramp, gated on the
//! terminal's ground on every page ([`Line::tone`] says why not the rules) —
//! one that asks for the person in `state.caution`, and never the accent: the
//! window's border already says where the keyboard is, and a second accent
//! would say it twice. A progress report (OSC 9;4) fills its line from the
//! left in the lit tone, and a failed one in `state.error` ([`Line::inks`]).

use ronoctua_render::quad::Quad;
use ronoctua_terminals::state::{Progress, ProgressState};
use ronoctua_tokens as tokens;

/// The fewest terminals a strip is drawn for: one has nothing to be told
/// apart from.
pub const FEWEST: usize = 2;

/// How much of the window's top the strip takes, in LOGICAL pixels, for a
/// group of `count` — nothing below [`FEWEST`]. What the grid is laid out
/// under (`terminal_pointer::Placed::under`).
#[must_use]
pub fn height(count: usize) -> u32 {
    if count >= FEWEST {
        u32::from(tokens::space::TERMINAL_STRIP_HEIGHTPX)
    } else {
        0
    }
}

/// One line as the window tells it: what its terminal is to the group.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Line {
    /// Its terminal is the one shown.
    pub front: bool,
    /// Its terminal asks for the person (a bell, a notification, a program
    /// waiting on them) since it was last seen.
    pub attention: bool,
    /// The pointer rests on it — and a click there would front it.
    pub lit: bool,
    /// What its program last reported of its progress (OSC 9;4).
    pub progress: Option<Progress>,
}

impl Line {
    /// The line's own tone: in front, asking, lit or at rest — in that
    /// order of precedence. The one in front is never lit: a click on it
    /// fronts nothing, and the pointer lights only what the click takes.
    ///
    /// THE TEXT RAMP, AND NOT THE RULES: the lines were `rule.default` at
    /// rest and `rule.strong` lit, and on the dark chrome page a resting line
    /// measured next to nothing against the terminal's ground — a strip of
    /// three tabs that read as one. The text inks are gated on that ground
    /// on every page (`surface.buffer` on the chrome's, the desk's chrome on
    /// the others), and their three rungs are already the order the strip
    /// needs: the one shown, the one the pointer would take, the rest.
    #[must_use]
    pub const fn tone(&self) -> tokens::Color {
        if self.front {
            tokens::color::TEXT_PRIMARY
        } else if self.attention {
            tokens::color::STATE_CAUTION
        } else if self.lit {
            tokens::color::TEXT_SECONDARY
        } else {
            tokens::color::TEXT_DIM
        }
    }

    /// The line's inks from its left end, as `(per mille of its length,
    /// tone)` runs that end where the next begins and the last at 1000.
    ///
    /// A progress report fills the line FROM THE LEFT in `text.secondary` —
    /// the brighter part is what is done — and what is left is the line's
    /// own tone; the line in front fills in its own tone and leaves the
    /// rest's. A failed report fills in `state.error`, the whole
    /// line when it said no percentage: danger is a colour, never another
    /// drawing. Busy with no percentage draws the line as it is — nothing to
    /// fill, and no clock is armed to animate it.
    #[must_use]
    pub fn inks(&self) -> Vec<(u16, tokens::Color)> {
        let own = self.tone();
        let Some(Progress { state, percent }) = self.progress else {
            return vec![(1000, own)];
        };
        let left = if self.front {
            tokens::color::TEXT_DIM
        } else {
            own
        };
        // The done part: the line in front fills in its own tone, every
        // other line in the lit one.
        let done_tone = if self.front {
            own
        } else {
            tokens::color::TEXT_SECONDARY
        };
        let filled = |percent: u8, fill: tokens::Color| {
            let done = u16::from(percent.min(100)) * 10;
            match done {
                0 => vec![(1000, left)],
                1000 => vec![(1000, fill)],
                _ => vec![(done, fill), (1000, left)],
            }
        };
        match (state, percent) {
            (ProgressState::Set | ProgressState::Pause, Some(p)) => filled(p, done_tone),
            (ProgressState::Error, Some(p)) => filled(p, tokens::color::STATE_ERROR),
            (ProgressState::Error, None) => vec![(1000, tokens::color::STATE_ERROR)],
            (ProgressState::Set | ProgressState::Pause | ProgressState::Indeterminate, _) => {
                vec![(1000, own)]
            }
        }
    }
}

/// THE STRIP of a window `width` logical pixels wide holding `count`
/// terminals, from the window's left edge to its right — bled, as a rule is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Strip {
    width: u32,
    count: usize,
}

impl Strip {
    /// The strip, or `None` for a group below [`FEWEST`] or a window with
    /// no width yet.
    #[must_use]
    pub const fn of(width: u32, count: usize) -> Option<Self> {
        if count >= FEWEST && width > 0 {
            Some(Self { width, count })
        } else {
            None
        }
    }

    /// Its height, in logical pixels — [`height`] of its count.
    #[must_use]
    pub fn height(self) -> u32 {
        height(self.count)
    }

    /// Where part `i` begins, in logical pixels: the window's width in
    /// `count` equal parts, whole pixels, the first at the left edge and the
    /// last ending at the right one — so the parts tile the width exactly,
    /// and a scale applied after keeps them whole.
    fn edge(self, i: usize) -> u32 {
        let at = u64::from(self.width) * i as u64 / self.count.max(1) as u64;
        u32::try_from(at).unwrap_or(self.width)
    }

    /// PART `i` — the click target of line `i`: `[from, to)` across, the
    /// strip's whole height down.
    #[must_use]
    pub fn part(self, i: usize) -> (u32, u32) {
        (self.edge(i), self.edge(i + 1))
    }

    /// The line under a point of the surface (logical pixels), anywhere in
    /// the strip's height — `None` under the strip or off its width.
    #[must_use]
    pub fn line_at(self, x: f64, y: f64) -> Option<usize> {
        if !(0.0..f64::from(self.height())).contains(&y)
            || !(0.0..f64::from(self.width)).contains(&x)
        {
            return None;
        }
        (0..self.count).find(|&i| {
            let (from, to) = self.part(i);
            (f64::from(from)..f64::from(to)).contains(&x)
        })
    }

    /// Line `i`'s ink across, `(x, width)` in logical pixels: its part, less
    /// half of `space.terminal.strip.gapPx` on each side that has a
    /// neighbour — the gap between two lines is the whole gap, and the
    /// strip's two ends bleed to the window's edges.
    #[must_use]
    pub fn line_span(self, i: usize) -> (u32, u32) {
        let gap = u32::from(tokens::space::TERMINAL_STRIP_GAPPX);
        let (from, to) = self.part(i);
        let left = if i > 0 { gap - gap / 2 } else { 0 };
        let right = if i + 1 < self.count { gap / 2 } else { 0 };
        let x = from + left;
        (x, to.saturating_sub(right).saturating_sub(x))
    }

    /// Where a line is drawn down the strip, `(y, thickness)` in logical
    /// pixels: `space.terminal.strip.linePx` thick, in the middle of the
    /// strip's height — the air above and below it is the target's, not the
    /// ink's.
    #[must_use]
    pub fn line_band(self) -> (u32, u32) {
        let thick = u32::from(tokens::space::TERMINAL_STRIP_LINEPX).min(self.height());
        ((self.height() - thick) / 2, thick)
    }

    /// The strip's rectangles, a line per terminal in the group's order —
    /// `lines` holds one per terminal, and a count that differs is drawn as
    /// the strip's own count says (a line missing is drawn at rest).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn quads(self, lines: &[Line]) -> Vec<Quad> {
        let (y, thick) = self.line_band();
        let mut out = Vec::with_capacity(self.count);
        for i in 0..self.count {
            let line = lines.get(i).copied().unwrap_or_default();
            let (x, wide) = self.line_span(i);
            let mut from = 0u32;
            for (end, color) in line.inks() {
                let to = wide * u32::from(end) / 1000;
                if to > from {
                    out.push(Quad {
                        x: (x + from) as f32,
                        y: y as f32,
                        width: (to - from) as f32,
                        height: thick as f32,
                        color,
                    });
                }
                from = to;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_strip_for_one_terminal() {
        // FAILS IF a window with one terminal draws a strip, or takes a
        // pixel of the grid for one — a line that tells nothing apart, and a
        // row lost for it.
        assert_eq!(height(0), 0);
        assert_eq!(height(1), 0);
        assert!(Strip::of(800, 1).is_none());
        assert!(Strip::of(800, 0).is_none());
        assert!(Strip::of(0, 3).is_none(), "a window with no width yet");
        assert_eq!(height(2), u32::from(tokens::space::TERMINAL_STRIP_HEIGHTPX));
        assert_eq!(Strip::of(800, 2).map(Strip::height), Some(height(2)));
    }

    #[test]
    fn a_click_on_the_strip_fronts_the_line_under_it() {
        // FAILS IF a point of the strip answers another line than the part
        // it is in, a part is lost between two lines (the gap is the
        // ink's, not the target's), the parts do not tile the whole width,
        // or a point under the strip — on the grid — is taken for a line.
        for (width, count) in [(800u32, 2usize), (801, 3), (1920, 7), (97, 5)] {
            let s = Strip::of(width, count).unwrap();
            let h = f64::from(s.height());
            assert_eq!(s.part(0).0, 0, "the first part starts at the edge");
            assert_eq!(s.part(count - 1).1, width, "the last ends at the edge");
            for i in 0..count {
                let (from, to) = s.part(i);
                assert!(to > from, "an empty part");
                if i + 1 < count {
                    assert_eq!(to, s.part(i + 1).0, "a hole between parts");
                }
                for x in [f64::from(from), f64::from(to) - 0.5] {
                    // THE WHOLE HEIGHT is the target, not the line's two pixels.
                    for y in [0.0, h / 2.0, h - 0.5] {
                        assert_eq!(s.line_at(x, y), Some(i), "{x},{y} of {count} in {width}");
                    }
                }
            }
            assert_eq!(s.line_at(10.0, h), None, "under the strip is the grid");
            assert_eq!(s.line_at(-1.0, 1.0), None);
            assert_eq!(s.line_at(f64::from(width), 1.0), None);
        }
    }

    #[test]
    fn the_lines_keep_the_gap_and_bleed_to_the_edges() {
        // FAILS IF two lines touch or overlap (they would read as one), the
        // gap between two is not the token's, the strip's ends are inset
        // (a rule bleeds edge to edge), or a line is drawn outside the strip.
        let gap = u32::from(tokens::space::TERMINAL_STRIP_GAPPX);
        let s = Strip::of(900, 4).unwrap();
        let spans: Vec<(u32, u32)> = (0..4).map(|i| s.line_span(i)).collect();
        assert_eq!(spans[0].0, 0);
        assert_eq!(spans[3].0 + spans[3].1, 900);
        for pair in spans.windows(2) {
            assert_eq!(pair[1].0 - (pair[0].0 + pair[0].1), gap, "{pair:?}");
        }
        let (y, thick) = s.line_band();
        assert_eq!(thick, u32::from(tokens::space::TERMINAL_STRIP_LINEPX));
        assert!(y + thick <= s.height());
        let quads = s.quads(&[Line::default(); 4]);
        assert_eq!(quads.len(), 4);
        for q in &quads {
            #[allow(clippy::cast_precision_loss)]
            let bottom = s.height() as f32;
            assert!(q.y >= 0.0 && q.y + q.height <= bottom);
        }
    }

    #[test]
    fn the_front_the_asking_and_the_rest_wear_roles_and_never_the_accent() {
        // FAILS IF the line in front is not the brightest role, a terminal
        // that asks for the person is not caution, the pointer lights the
        // line in front (a click there takes nothing), or the accent — the
        // window border's colour — is worn by any line.
        let front = Line {
            front: true,
            lit: true,
            ..Line::default()
        };
        assert_eq!(front.tone(), tokens::color::TEXT_PRIMARY);
        let asking = Line {
            attention: true,
            ..Line::default()
        };
        assert_eq!(asking.tone(), tokens::color::STATE_CAUTION);
        let lit = Line {
            lit: true,
            ..Line::default()
        };
        assert_eq!(lit.tone(), tokens::color::TEXT_SECONDARY);
        assert_eq!(Line::default().tone(), tokens::color::TEXT_DIM);
        for line in [front, asking, lit, Line::default()] {
            assert_ne!(line.tone(), tokens::color::STATE_ACCENT);
        }
    }

    #[test]
    fn a_progress_fills_its_line_from_the_left() {
        // FAILS IF a progress report does not fill its line by its
        // percentage, the done part is not the lit tone, a failure is not
        // the danger colour, or a busy report with no percentage paints
        // something nobody could read as a fraction.
        let at = |state, percent| Line {
            progress: Some(Progress { state, percent }),
            ..Line::default()
        };
        assert_eq!(
            at(ProgressState::Set, Some(40)).inks(),
            vec![
                (400, tokens::color::TEXT_SECONDARY),
                (1000, tokens::color::TEXT_DIM)
            ]
        );
        assert_eq!(
            at(ProgressState::Error, Some(100)).inks(),
            vec![(1000, tokens::color::STATE_ERROR)]
        );
        assert_eq!(
            at(ProgressState::Error, None).inks(),
            vec![(1000, tokens::color::STATE_ERROR)]
        );
        assert_eq!(
            at(ProgressState::Indeterminate, None).inks(),
            vec![(1000, tokens::color::TEXT_DIM)]
        );
        // IN FRONT the line fills in its own tone, and what is left is drawn
        // at rest, or the fraction would not show.
        let front = Line {
            front: true,
            ..at(ProgressState::Set, Some(25))
        };
        assert_eq!(
            front.inks(),
            vec![
                (250, tokens::color::TEXT_PRIMARY),
                (1000, tokens::color::TEXT_DIM)
            ]
        );
        // AND THE QUADS cut the line where the inks say.
        let s = Strip::of(1000, 2).unwrap();
        let quads = s.quads(&[at(ProgressState::Set, Some(50)), Line::default()]);
        assert_eq!(quads.len(), 3);
        let (_, wide) = s.line_span(0);
        #[allow(clippy::cast_precision_loss)]
        let half = (wide / 2) as f32;
        assert!((quads[0].width - half).abs() < 0.5, "{quads:?}");
        assert_eq!(quads[0].color, tokens::color::TEXT_SECONDARY);
    }
}
