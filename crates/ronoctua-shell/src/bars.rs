//! WHERE EVERY SCROLLBAR OF A FRAME WAS DRAWN — and so which cells are the
//! bars' and not the rows'.
//!
//! # Why this is one registry and not a question each list answers
//!
//! Because each list answered it, and each answered it its own way. The owner
//! asked for one rule: a row lights and takes a click across the whole width
//! while there is no bar, and once a bar is DRAWN its column is the bar's
//! alone — the pointer resting on it lights no row, and a row's band stops
//! before it. Written list by list, the rule was kept in some doors and not in
//! others, and inside one door it was kept by CELL where the bar is drawn by
//! PIXEL: a bar straddles two cells, and its right half answered the row
//! beside it. The owner photographed exactly that on the file manager, and
//! asked for the answer to be global and definitive.
//!
//! So a frame records the TRACK of every bar it draws, here, in the pixels it
//! drew it in; and two things read it, whatever drew the bar:
//!
//! - the POINTER: a motion on a track, with no button held, reaches the
//!   content as the pointer LEAVING the rows (`door::Door`, and the footer's
//!   own twin) — so no content can light a row under a bar, because none is
//!   ever told the pointer is on one;
//! - the BANDS: every band of the frame — the selection and every cursor's
//!   hover, a closed set of tokens ([`is_a_band`]) — is cut where a track
//!   beside it begins ([`Drawn::clip`]), so no painter can run one under a bar.
//!
//! What a painter has to do is one thing: record the track of the bar it
//! draws. `every-bar-is-recorded` refuses a painter that draws a bar and does
//! not.

use ronoctua_tokens as tokens;

/// One bar's TRACK, in the surface's pixels: the whole column it can be
/// grabbed in, not only its thumb.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Track {
    /// The left edge.
    pub x: f32,
    /// The top.
    pub y: f32,
    /// The width — one cell.
    pub width: f32,
    /// The length of the track.
    pub height: f32,
}

impl Track {
    /// A track from an `(x, y, width, height)` rectangle.
    #[must_use]
    pub const fn of((x, y, width, height): (f32, f32, f32, f32)) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether a point of the surface is on it.
    #[must_use]
    pub fn holds(self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && x < f64::from(self.x + self.width)
            && y >= f64::from(self.y)
            && y < f64::from(self.y + self.height)
    }
}

/// Every track a frame drew.
#[derive(Debug, Clone, Default)]
pub struct Drawn {
    tracks: Vec<Track>,
}

impl Drawn {
    /// A new frame: nothing drawn yet.
    pub fn clear(&mut self) {
        self.tracks.clear();
    }

    /// A bar was drawn with this track.
    pub fn record(&mut self, track: Track) {
        if track.width > 0.0 && track.height > 0.0 {
            self.tracks.push(track);
        }
    }

    /// The same, when there may be no bar.
    pub fn record_opt(&mut self, track: Option<Track>) {
        if let Some(t) = track {
            self.record(t);
        }
    }

    /// Whether a point is on any bar the last frame drew.
    #[must_use]
    pub fn on(&self, x: f64, y: f64) -> bool {
        self.tracks.iter().any(|t| t.holds(x, y))
    }

    /// Cuts every BAND of `quads` where a track beside it begins.
    ///
    /// A band that reaches into a track stops at the track's left edge; a
    /// band that starts inside one is not drawn at all. Anything that is not a
    /// band — a ground, a rule, a bar — is left alone: a bar is a signal drawn
    /// ON its ground, and a gap behind it would be a stripe of the page.
    pub fn clip(&self, quads: &mut [ronoctua_render::quad::Quad]) {
        for q in quads.iter_mut().filter(|q| is_a_band(q.color)) {
            for t in &self.tracks {
                let beside = q.y < t.y + t.height && q.y + q.height > t.y;
                if !beside {
                    continue;
                }
                if q.x >= t.x && q.x < t.x + t.width {
                    q.width = 0.0;
                } else if t.x > q.x && t.x < q.x + q.width {
                    q.width = t.x - q.x;
                }
            }
        }
    }
}

/// Whether a colour is a BAND's: the keyboard's selection, or a pointer's
/// hover — neutral, or in one cursor's colour (`crate::seats`).
#[must_use]
pub fn is_a_band(c: tokens::Color) -> bool {
    c == tokens::color::SURFACE_SELECTION
        || c == tokens::color::SURFACE_HOVER
        || crate::seats::is_a_cursor_hover(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_render::quad::Quad;

    fn band(x: f32, width: f32) -> Quad {
        Quad {
            x,
            y: 20.0,
            width,
            height: 20.0,
            color: tokens::color::SURFACE_HOVER,
        }
    }

    fn drawn() -> Drawn {
        let mut d = Drawn::default();
        d.record(Track::of((90.0, 0.0, 10.0, 100.0)));
        d
    }

    /// THE WHOLE TRACK IS THE BAR'S: both halves of the cell it straddles,
    /// which is where the owner saw the right half answer the row.
    #[test]
    fn every_pixel_of_the_track_is_the_bar() {
        let d = drawn();
        assert!(d.on(90.0, 50.0));
        assert!(d.on(94.9, 50.0));
        assert!(d.on(99.9, 50.0));
        assert!(!d.on(89.9, 50.0));
        assert!(!d.on(95.0, 120.0), "below the track is not the bar");
    }

    /// A BAND STOPS WHERE THE BAR BEGINS, and one without a bar beside it
    /// goes to the edge.
    #[test]
    fn a_band_stops_at_the_bar_and_only_there() {
        let mut quads = [band(0.0, 100.0)];
        drawn().clip(&mut quads);
        assert!((quads[0].width - 90.0).abs() < f32::EPSILON);
        let mut free = [band(0.0, 100.0)];
        Drawn::default().clip(&mut free);
        assert!((free[0].width - 100.0).abs() < f32::EPSILON);
    }

    /// A GROUND IS NOT A BAND: the bar is drawn on it.
    #[test]
    fn a_ground_runs_under_the_bar() {
        let mut quads = [Quad {
            color: tokens::color::SURFACE_CONTAINER,
            ..band(0.0, 100.0)
        }];
        drawn().clip(&mut quads);
        assert!((quads[0].width - 100.0).abs() < f32::EPSILON);
    }
}
