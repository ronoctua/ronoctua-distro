//! A scrollbar, as arithmetic — the same one on both sides of the wall.
//!
//! # Why this is here and not in the shell
//!
//! Because it was in the shell three times. The manager drew its bar with one
//! sum and looked for it with another; the footer had a third for the eight
//! views that are not the manager; and each door of the manager had written its
//! own half of the drag. Every one of those was the same six lines, and the
//! defects the owner reported were the differences between them: a thumb that
//! moved in one door and refused in the other, one that stopped half way down
//! its own track, and one that ran past the bottom of the column it belonged
//! to.
//!
//! # Why it crosses to the guest side
//!
//! Because a buffer has lists too, and nothing about a track is about pixels.
//! [`Scroll`] counts in whatever unit it is handed: give it a track in cells
//! and it answers in cells, give it one in pixels and it answers in pixels.
//! That is what lets the host measure a surface the compositor gave — where
//! the residue lives — and a component measure a grid, from one implementation.
//!
//! Invariant 11 is not bent by this: no token value crosses, and the caller
//! brings its own unit.

/// A list that does not fit, and where its thumb goes.
///
/// `total` is how many rows exist, `lines` how many are shown, `offset` which
/// row is first. `top` is how far down the band starts, in lines, for a section
/// that does not begin at the top of its column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scroll {
    /// How many rows the list has.
    pub total: usize,
    /// How many of them are on screen.
    pub lines: usize,
    /// Which row is the first one drawn.
    pub offset: usize,
    /// How many lines above this band belong to something else.
    pub top: usize,
}

impl Scroll {
    /// Whether there is anything to scroll.
    ///
    /// A bar over a list that fits is a control that does nothing, and the
    /// whole shell agrees on that: no bar is drawn.
    #[must_use]
    pub const fn cut(&self) -> bool {
        self.total > self.lines && self.lines > 0
    }

    /// How long this band's track is.
    ///
    /// `band_top` is where the section starts and `surface_bottom` where the
    /// SURFACE ends — the number the compositor gave, not a count of anything.
    ///
    /// # The residue, and who owns it
    ///
    /// A surface holds `floor(height / unit)` whole rows plus a leftover of
    /// less than one. A track of `rows × unit` stops short by that leftover,
    /// and so does the thumb at the last offset: the strip of background under
    /// it is what the owner photographed twice.
    ///
    /// But only the LAST band in a column may take it. Handing every band the
    /// surface's bottom was the first shape of that fix and it was too wide:
    /// where a strip of panels sits below a listing, the listing's thumb ran
    /// down into it.
    ///
    /// The leftover is under one unit by construction, so a band ending within
    /// one unit of the surface is the last one and takes what is left; a band
    /// ending a whole unit or more above it has something underneath.
    #[must_use]
    pub fn track(&self, band_top: f32, surface_bottom: f32, unit: f32) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let natural = band_top + self.lines as f32 * unit;
        let bottom = if surface_bottom - natural < unit {
            surface_bottom
        } else {
            natural
        };
        (bottom - band_top).max(unit)
    }

    /// The thumb: how far down it starts and how long it is, in the track's
    /// own unit.
    ///
    /// `track` is the whole band, from the top of this section to its bottom —
    /// and its bottom is the CALLER's business, because only the caller knows
    /// whether anything sits underneath.
    ///
    /// # Two defects, written down
    ///
    /// The length is `lines / total` of the track and never less than one
    /// unit, or a long list produces a thumb too small to grab.
    ///
    /// The travel is measured over `track - thumb` and never over the whole
    /// track: with the whole track, the last page put the thumb's TOP at the
    /// bottom, so it hung off the end — and, clamped, it stopped half way down
    /// its own column. The owner dragged one to its end and watched it stop in
    /// the middle.
    #[must_use]
    pub fn thumb(&self, track: f32, unit: f32) -> Option<(f32, f32)> {
        if !self.cut() || track <= 0.0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        let (total, lines, offset) = (self.total as f32, self.lines as f32, self.offset as f32);
        let tall = (lines / total * track).max(unit);
        let travel = (track - tall).max(0.0);
        let last = (total - lines).max(1.0);
        let from = (offset / last).min(1.0) * travel;
        Some((from, tall))
    }

    /// The bar itself: where the thumb goes, as a rectangle.
    ///
    /// `right` is the pixel the band's LAST cell ends at — the bar is that
    /// cell — and `surface_bottom` is where the surface ends, so [`Self::track`]
    /// can decide whether this band owns the leftover.
    ///
    /// # Why the rectangle is here and not at the two callers
    ///
    /// Because there were two, and the difference between them was a defect
    /// each time. The manager built one per section and the footer built one
    /// for its region, and between them they managed: a thumb drawn where it
    /// could not be grabbed, a thumb that ran into the box below its own
    /// column, and a column so tall that pressing the modeline dragged a list.
    ///
    /// Nine views, one rectangle. What still differs between them is how many
    /// bands they have — the manager four, everything else one — and that is a
    /// fact about the views, not about the bar.
    #[must_use]
    pub fn bar(
        &self,
        right: f32,
        band_top: f32,
        surface_bottom: f32,
        cell: f32,
        unit: f32,
    ) -> Option<(f32, f32, f32, f32)> {
        let track = self.track(band_top, surface_bottom, unit);
        let (from, tall) = self.thumb(track, unit)?;
        Some((right - cell, band_top + from, cell, tall))
    }

    /// Which offset a point in the track means.
    ///
    /// The inverse of [`Scroll::thumb`], and it has to be: where a bar is drawn
    /// is where it is grabbed. `at` is measured from the top of the band, in
    /// the same unit as `track`.
    #[must_use]
    pub fn offset_at(&self, at: f32, track: f32, unit: f32) -> usize {
        if !self.cut() || track <= 0.0 {
            return 0;
        }
        let Some((_, tall)) = self.thumb(track, unit) else {
            return 0;
        };
        let travel = (track - tall).max(0.0);
        let last = self.total.saturating_sub(self.lines);
        if travel <= 0.0 {
            return last;
        }
        // THE POINT IS THE THUMB'S CENTRE, and not its top. Grabbing a bar
        // puts the pointer somewhere inside it, and a caller that hands in
        // where the pointer IS wants the thumb to sit under the pointer —
        // which is what makes the bottom of the track the last page.
        let fraction = ((at - tall / 2.0) / travel).clamp(0.0, 1.0);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let n = (fraction * last as f32).round() as usize;
        n.min(last)
    }

    /// Whether a point inside the band is on the thumb.
    #[must_use]
    pub fn holds(&self, at: f32, track: f32, unit: f32) -> bool {
        self.thumb(track, unit)
            .is_some_and(|(from, tall)| at >= from && at < from + tall)
    }
}

#[cfg(test)]
mod tests {
    use super::Scroll;

    /// WHERE IT DRAWS IS WHERE IT IS GRABBED — the law the plan names, and the
    /// one no grep can see.
    ///
    /// The manager's bar was drawn by one sum and looked for by another, in
    /// both doors. It moved in one and refused in the other, and the owner
    /// tried it with the mouse in each.
    #[test]
    fn where_it_draws_is_where_it_is_grabbed() {
        let (track, unit) = (520.0f32, 26.0f32);
        for total in [13usize, 40, 200, 6_000] {
            for lines in [4usize, 12, 20] {
                if total <= lines {
                    continue;
                }
                let last = total - lines;
                for offset in [0, 1, last / 2, last] {
                    let s = Scroll {
                        total,
                        lines,
                        offset,
                        top: 0,
                    };
                    let (from, tall) = s.thumb(track, unit).expect("the list is cut");
                    let centre = from + tall / 2.0;
                    assert_eq!(
                        s.offset_at(centre, track, unit),
                        offset,
                        "total={total} lines={lines} offset={offset}: the thumb's own \
                         centre came back as another offset"
                    );
                    assert!(
                        s.holds(centre, track, unit),
                        "the centre is not on the thumb"
                    );
                }
            }
        }
    }

    /// THE LAST PAGE PUTS THE THUMB AT THE BOTTOM, and never half way.
    #[test]
    fn the_end_of_the_track_is_the_last_page() {
        let (track, unit) = (400.0f32, 26.0f32);
        let s = Scroll {
            total: 500,
            lines: 10,
            offset: 490,
            top: 0,
        };
        let (from, tall) = s.thumb(track, unit).expect("the list is cut");
        assert!(
            (from + tall - track).abs() < 0.001,
            "the thumb ends at {} and the track at {track}",
            from + tall
        );
        assert_eq!(
            s.offset_at(track, track, unit),
            490,
            "the bottom is the last page"
        );
    }

    /// A THUMB IS ALWAYS BIG ENOUGH TO GRAB.
    #[test]
    fn a_very_long_list_still_has_something_to_hold() {
        let (track, unit) = (300.0f32, 26.0f32);
        let s = Scroll {
            total: 200_000,
            lines: 11,
            offset: 199_000,
            top: 0,
        };
        let (from, tall) = s.thumb(track, unit).expect("the list is cut");
        assert!(tall >= unit - 0.001, "the thumb vanished: {tall}");
        assert!(from + tall <= track + 0.001, "the thumb left the track");
    }

    /// ONLY THE LAST BAND TAKES THE LEFTOVER.
    #[test]
    fn a_band_with_something_under_it_stops_where_its_rows_stop() {
        let unit = 26.0f32;
        let s = Scroll {
            total: 80,
            lines: 20,
            offset: 0,
            top: 0,
        };
        // A surface of 20 rows plus four tenths: this band is the last one, so
        // the leftover is its.
        let alone = s.track(0.0, 20.0 * unit + unit * 0.4, unit);
        assert!(
            (alone - (20.0 * unit + unit * 0.4)).abs() < 0.001,
            "the last band did not take the leftover: {alone}"
        );
        // The same band with a strip of four rows under it: the track is its
        // own rows and not one pixel more.
        let above = s.track(0.0, 24.0 * unit + unit * 0.4, unit);
        assert!(
            (above - 20.0 * unit).abs() < 0.001,
            "the band ran into what is under it: {above}"
        );
    }

    /// THE RECTANGLE IS THE THUMB, IN THE BAND'S LAST CELL.
    #[test]
    fn the_bar_is_one_cell_at_the_bands_right_edge() {
        let (cell, unit) = (10.8f32, 26.1f32);
        let s = Scroll {
            total: 90,
            lines: 10,
            offset: 40,
            top: 0,
        };
        let (x, y, w, h) = s
            .bar(700.0, unit, 12.0 * unit + unit * 0.3, cell, unit)
            .expect("ninety rows in ten do not fit");
        assert!(
            (x - (700.0 - cell)).abs() < 0.001,
            "the bar is not the last cell"
        );
        assert!((w - cell).abs() < 0.001, "the bar is one cell wide");
        assert!(y >= unit - 0.001, "the thumb rose out of its band");
        let track = s.track(unit, 12.0 * unit + unit * 0.3, unit);
        assert!(
            y + h <= unit + track + 0.001,
            "the thumb ran past its own track"
        );
    }

    /// AND A LIST THAT FITS HAS NO BAR AT ALL.
    #[test]
    fn a_whole_list_has_no_thumb() {
        let s = Scroll {
            total: 4,
            lines: 10,
            offset: 0,
            top: 0,
        };
        assert!(!s.cut());
        assert_eq!(s.thumb(400.0, 26.0), None);
        assert!(!s.holds(10.0, 400.0, 26.0));
        assert_eq!(s.bar(700.0, 0.0, 400.0, 10.8, 26.0), None);
    }
}
