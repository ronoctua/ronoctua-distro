//! Where the grid is on the window, and what the pointer does over it — the
//! arithmetic, apart from the window, so a test holds it.
//!
//! # The grid's place
//!
//! The grid is whole cells of the TERMINAL's face, in physical pixels, inset
//! from the window's left and right edges by the terminal's own padding
//! (`space.terminal.paddingPx`, Ghostty's two pixels), and what is left over
//! at the right and the bottom is ground. The scrollbar takes no room: it is
//! drawn OVER the grid's right edge, and only while it is used (the owner's
//! exception, 2026-11-03, `crate::terminal`) — so it never changes the
//! program's width. A group's strip takes its pixels off the grid's TOP
//! (`crate::terminal_strip`): the grid starts under it, and the bar's track
//! does too ([`Placed::under`], [`thumb`]).
//!
//! # The wheel has three owners
//!
//! A program that tracks the mouse is told the wheel as buttons 4 and 5; one
//! on the alternate screen that asked for it (mode 1007) is sent arrows,
//! which is how a pager scrolls; and otherwise the wheel moves the window's
//! view of the history, which the program never sees ([`wheel`]).

use ronoctua_render::cells::Geometry;
use ronoctua_terminals::wire;
use ronoctua_tokens as tokens;

/// The evdev code of the middle button (`BTN_MIDDLE`); the door names the
/// left, the right and back (`door::LEFT_BUTTON` …).
pub const MIDDLE_BUTTON: u32 = 0x112;

/// The evdev code of the forward button (`BTN_EXTRA`).
pub const FORWARD_BUTTON: u32 = 0x114;

/// WHERE THE GRID IS on the window, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    /// The grid's top-left corner on the surface.
    pub origin: (u32, u32),
    /// One cell.
    pub cell: (u32, u32),
    /// Its size in cells.
    pub columns: u16,
    /// Its size in rows.
    pub rows: u16,
    /// Physical pixels per logical one.
    pub scale: u32,
}

/// The margin at each side of the grid, in PHYSICAL pixels: the terminal's
/// padding (`space.terminal.paddingPx`, logical pixels) at the screen's
/// scale.
#[must_use]
pub fn inset_px(scale: u32) -> u32 {
    u32::from(tokens::space::TERMINAL_PADDINGPX) * scale.max(1)
}

/// How many of `geometry`'s cells fit a window of `width` × `height`
/// LOGICAL pixels at `scale`, a margin of `inset` physical pixels off each
/// side — never fewer than one of each, so the engine is never handed a grid
/// of nothing (a window narrower than its margins is one the compositor may
/// still hand over).
#[must_use]
pub fn cells_that_fit(
    width: u32,
    height: u32,
    scale: u32,
    inset: u32,
    geometry: &Geometry,
) -> (u16, u16) {
    cells_under(width, height, scale, inset, 0, geometry)
}

/// [`cells_that_fit`] under a band `top` LOGICAL pixels tall across the
/// window's top — a group's strip: its pixels are the grid's.
#[must_use]
pub fn cells_under(
    width: u32,
    height: u32,
    scale: u32,
    inset: u32,
    top: u32,
    geometry: &Geometry,
) -> (u16, u16) {
    let across = (width * scale).saturating_sub(2 * inset);
    let down = (height * scale).saturating_sub(top * scale);
    (
        geometry.columns_in(across).max(1),
        geometry.lines_in(down).max(1),
    )
}

/// The window that holds a grid of `columns` × `rows` cells of `geometry`,
/// margins included, in LOGICAL pixels — the inverse of [`cells_that_fit`]:
/// the smallest size whose count is exactly that grid.
#[must_use]
pub fn size_of_cells(
    columns: u16,
    rows: u16,
    scale: u32,
    inset: u32,
    geometry: &Geometry,
) -> (u32, u32) {
    let scale = scale.max(1);
    let wide = u32::from(columns) * geometry.cell_width + 2 * inset;
    let tall = u32::from(rows) * geometry.cell_height;
    (wide.div_ceil(scale), tall.div_ceil(scale))
}

/// The size a terminal's window asks for first: `space.terminal`'s columns
/// and lines of `geometry`'s cells, at the scale they were measured at
/// (`Geometry::scale`), one until the window is laid out on a screen.
#[must_use]
pub fn first_size(geometry: &Geometry) -> (u32, u32) {
    let scale = u32::from(geometry.scale.max(1));
    size_of_cells(
        tokens::space::TERMINAL_COLUMNS,
        tokens::space::TERMINAL_LINES,
        scale,
        inset_px(scale),
        geometry,
    )
}

impl Placed {
    /// The grid of a window `width` × `height` logical pixels at `scale`.
    #[must_use]
    pub fn of(width: u32, height: u32, scale: u32, geometry: &Geometry) -> Self {
        Self::under(width, height, scale, 0, geometry)
    }

    /// The grid of the same window under a band `top` LOGICAL pixels tall
    /// across its top — the group's strip (`crate::terminal_strip::height`):
    /// the grid begins under it, and every row the band covers is a row the
    /// program does not have.
    #[must_use]
    pub fn under(width: u32, height: u32, scale: u32, top: u32, geometry: &Geometry) -> Self {
        let scale = scale.max(1);
        let inset = inset_px(scale);
        let (columns, rows) = cells_under(width, height, scale, inset, top, geometry);
        Self {
            origin: (inset, top * scale),
            cell: (geometry.cell_width.max(1), geometry.cell_height.max(1)),
            columns,
            rows,
            scale,
        }
    }

    /// A point of the surface (LOGICAL pixels) in physical pixels from the
    /// grid's corner — what a program tracking the mouse is told, and what a
    /// selection is pressed at. Negative is left of or above the grid.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn from_corner(&self, x: f64, y: f64) -> (f64, f64) {
        let s = f64::from(self.scale);
        (
            x * s - f64::from(self.origin.0),
            y * s - f64::from(self.origin.1),
        )
    }

    /// The cell under a point of the surface, `None` off the grid.
    #[must_use]
    pub fn cell_at(&self, x: f64, y: f64) -> Option<(u16, u16)> {
        let (px, py) = self.from_corner(x, y);
        if px < 0.0 || py < 0.0 {
            return None;
        }
        let (c, r) = self.cell_of(px, py);
        (c < self.columns && r < self.rows).then_some((c, r))
    }

    /// The cell nearest a point of the surface — on the grid's edge for a
    /// point past it, which a drag that left the grid still selects to.
    #[must_use]
    pub fn cell_near(&self, x: f64, y: f64) -> (u16, u16) {
        let (px, py) = self.from_corner(x, y);
        let (c, r) = self.cell_of(px.max(0.0), py.max(0.0));
        (
            c.min(self.columns.saturating_sub(1)),
            r.min(self.rows.saturating_sub(1)),
        )
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn cell_of(&self, px: f64, py: f64) -> (u16, u16) {
        let c = (px / f64::from(self.cell.0))
            .floor()
            .min(f64::from(u16::MAX)) as u16;
        let r = (py / f64::from(self.cell.1))
            .floor()
            .min(f64::from(u16::MAX)) as u16;
        (c, r)
    }

    /// The grid's geometry as the engine's mouse encoder and selection take
    /// it: positions from the grid's corner, so no padding.
    #[must_use]
    pub fn engine(&self) -> ronoctua_term::ghostty::Geometry {
        ronoctua_term::ghostty::Geometry {
            width_px: u32::from(self.columns) * self.cell.0,
            height_px: u32::from(self.rows) * self.cell.1,
            cell_width_px: self.cell.0,
            cell_height_px: self.cell.1,
            padding_top_px: 0,
            padding_bottom_px: 0,
            padding_left_px: 0,
            padding_right_px: 0,
        }
    }

    /// The terminal's size, as the wire says it.
    #[must_use]
    pub fn size(&self) -> wire::Size {
        wire::Size {
            columns: self.columns,
            rows: self.rows,
            cell_width_px: u16::try_from(self.cell.0).unwrap_or(u16::MAX),
            cell_height_px: u16::try_from(self.cell.1).unwrap_or(u16::MAX),
        }
    }

    /// One row's height in LOGICAL pixels — what a touchpad's pixels are
    /// counted in lines by.
    #[must_use]
    pub fn row_logical(&self) -> f64 {
        f64::from(self.cell.1) / f64::from(self.scale)
    }
}

/// Where the wheel goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wheel {
    /// To the program, as presses of the wheel's button, one a line.
    Report(wire::Button, u32),
    /// To the program, as arrows (mode 1007 on the alternate screen):
    /// `true` is up.
    Arrows(bool, u32),
    /// The window's view of the history, by lines (up negative).
    View(isize),
}

/// What whole `lines` of the wheel do (up negative) — see the head of the
/// module. `None` for nothing to do.
#[must_use]
pub fn wheel(lines: i64, tracking: bool, alternate: bool, alt_scroll: bool) -> Option<Wheel> {
    if lines == 0 {
        return None;
    }
    let up = lines < 0;
    let times = u32::try_from(lines.unsigned_abs()).unwrap_or(u32::MAX);
    Some(if tracking {
        Wheel::Report(
            if up {
                wire::Button::WheelUp
            } else {
                wire::Button::WheelDown
            },
            times,
        )
    } else if alternate && alt_scroll {
        Wheel::Arrows(up, times)
    } else {
        Wheel::View(isize::try_from(lines).unwrap_or(isize::MAX))
    })
}

/// The button an evdev code is, for a program that tracks the mouse.
#[must_use]
pub const fn button_of(code: u32) -> Option<wire::Button> {
    use ronoctua_shell_door::door::{BACK_BUTTON, LEFT_BUTTON, RIGHT_BUTTON};
    match code {
        LEFT_BUTTON => Some(wire::Button::Left),
        RIGHT_BUTTON => Some(wire::Button::Right),
        MIDDLE_BUTTON => Some(wire::Button::Middle),
        BACK_BUTTON => Some(wire::Button::Back),
        FORWARD_BUTTON => Some(wire::Button::Forward),
        _ => None,
    }
}

/// The history as a list's scroll: every row there is, the viewport's, and
/// the first one shown — what the bar is drawn and grabbed from.
#[must_use]
pub fn scroll_of(bar: ronoctua_term::ghostty::Scrollbar) -> ronoctua_design::Scroll {
    let at = |n: u64| usize::try_from(n).unwrap_or(usize::MAX);
    ronoctua_design::Scroll {
        total: at(bar.total),
        lines: at(bar.len),
        offset: at(bar.offset),
        top: 0,
    }
}

/// THE BAR'S THUMB on the window's right margin, `width` × `height` logical
/// pixels and from `top` down (under the group's strip), its track counted
/// in the TERMINAL's rows of `row` logical pixels.
///
/// Not a list's bar (`chrome::list_bar`), which counts its rows in the
/// interface's lines: a terminal's row is its own face's, and a window zoomed
/// past the interface's line (`C-=`) would have its track end at the
/// viewport's rows times the interface's line — the thumb drawn on the top of
/// the margin only, and a press under it taken for the last page.
#[must_use]
pub fn thumb(
    scroll: ronoctua_design::Scroll,
    width: f32,
    top: f32,
    height: f32,
    row: f32,
) -> Option<ronoctua_shell_kit::chrome::Rect> {
    bar(scroll, width, top, height, row).1
}

/// THE BAR, track and thumb — [`thumb`] and the column a press grabs it in
/// (`None` while the history fits), from the one `chrome::band_bar`: what
/// is drawn and what is grabbed are one answer.
#[must_use]
pub fn bar(
    scroll: ronoctua_design::Scroll,
    width: f32,
    top: f32,
    height: f32,
    row: f32,
) -> (
    Option<ronoctua_shell_kit::chrome::Rect>,
    Option<ronoctua_shell_kit::chrome::Rect>,
) {
    use ronoctua_shell_kit::chrome;
    if !scroll.cut() {
        return (None, None);
    }
    let (track, thumb) = chrome::band_bar(scroll, width, top, height, chrome::cell_width(), row);
    (Some(track), thumb)
}

/// Where a press at height `y` on the bar puts the view — [`thumb`]'s own
/// inverse, over the same track from `top`, so the bar is grabbed where it
/// is drawn.
#[must_use]
pub fn offset_at(
    scroll: ronoctua_design::Scroll,
    top: f32,
    height: f32,
    row: f32,
    y: f32,
) -> usize {
    let track = scroll.track(top, height, row);
    scroll.offset_at(y - top, track, row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_render::sprite::Stroke;

    fn geometry(cell_width: u32, cell_height: u32) -> Geometry {
        let stroke = Stroke {
            top: 0,
            thickness: 1,
        };
        Geometry {
            cell_width,
            cell_height,
            baseline: cell_height * 3 / 4,
            stroke: 1,
            underline: stroke,
            strike: stroke,
            overline: stroke,
            size: 14.0,
            scale: 1,
        }
    }

    #[test]
    fn the_grid_counts_only_the_cells_inside_its_margins() {
        // FAILS IF the grid takes columns from the margins (the last ones
        // would be written past the right edge, where the scrollbar is, and
        // the compositor does not clip), or ever counts none.
        for scale in [1u32, 2] {
            let inset = inset_px(scale);
            let g = geometry(9 * scale, 19 * scale);
            for width in [200u32, 640, 1280, 1920] {
                for height in [100u32, 600, 1080] {
                    let (columns, rows) = cells_that_fit(width, height, scale, inset, &g);
                    let used = u32::from(columns) * g.cell_width + 2 * inset;
                    assert!(
                        used <= width * scale,
                        "{columns} columns need {used} px of {}",
                        width * scale
                    );
                    assert!(u32::from(rows) * g.cell_height <= height * scale);
                    // AND NO WHOLE CELL IS LEFT OVER on either axis.
                    assert!(used + g.cell_width > width * scale);
                    assert!((u32::from(rows) + 1) * g.cell_height > height * scale);
                }
            }
            assert_eq!(cells_that_fit(1, 1, scale, inset, &g), (1, 1));
        }
    }

    #[test]
    fn the_first_size_is_exactly_the_grid_asked_for() {
        // FAILS IF a terminal born at 80×24 comes up at 79 or 81 — the
        // size of cells and the count of cells are two arithmetics that
        // must be each other's inverse.
        for scale in [1u32, 2, 3] {
            let inset = inset_px(scale);
            let g = geometry(9 * scale + 1, 19 * scale);
            for (c, r) in [(80u16, 24u16), (2, 2), (132, 43)] {
                let (w, h) = size_of_cells(c, r, scale, inset, &g);
                assert_eq!(
                    cells_that_fit(w, h, scale, inset, &g),
                    (c, r),
                    "{w}x{h} at {scale}"
                );
            }
        }
    }

    #[test]
    fn a_point_is_the_cell_under_it_and_off_the_grid_is_none() {
        // FAILS IF a click lands one cell off (the margin not taken off, or
        // the scale applied twice), or a click on the margin selects a cell.
        let p = Placed::of(400, 200, 2, &geometry(20, 40));
        let inset = inset_px(2);
        assert_eq!(p.origin, (inset, 0));
        // Logical (inset/2 + 1, 1) is just inside the first cell.
        let x0 = f64::from(inset) / 2.0;
        assert_eq!(p.cell_at(x0 + 0.5, 0.5), Some((0, 0)));
        assert_eq!(p.cell_at(x0 + 10.5, 20.5), Some((1, 1)));
        assert_eq!(p.cell_at(x0 - 1.0, 5.0), None, "the margin is no cell");
        assert_eq!(p.cell_at(399.0, 199.0), None, "past the last cell");
        assert_eq!(p.cell_near(399.0, 199.0), (p.columns - 1, p.rows - 1));
        assert_eq!(p.cell_near(-5.0, -5.0), (0, 0));
    }

    #[test]
    fn the_wheel_goes_to_whoever_owns_it() {
        // FAILS IF a program tracking the mouse is not told the wheel, a
        // pager on the alternate screen gets no arrows, or the history moves
        // under a program that asked for the wheel.
        assert_eq!(
            wheel(-3, true, false, false),
            Some(Wheel::Report(wire::Button::WheelUp, 3))
        );
        assert_eq!(
            wheel(2, true, true, true),
            Some(Wheel::Report(wire::Button::WheelDown, 2))
        );
        assert_eq!(wheel(-1, false, true, true), Some(Wheel::Arrows(true, 1)));
        assert_eq!(wheel(4, false, true, false), Some(Wheel::View(4)));
        assert_eq!(wheel(-2, false, false, true), Some(Wheel::View(-2)));
        assert_eq!(wheel(0, true, true, true), None);
    }

    #[test]
    fn the_bar_is_the_history_and_where_it_is_grabbed_is_where_it_scrolls() {
        // FAILS IF the bar is drawn for a screen with no history, its thumb
        // is not where the viewport is, or a press on it scrolls elsewhere
        // than the thumb it drew.
        let none = scroll_of(ronoctua_term::ghostty::Scrollbar {
            total: 24,
            offset: 0,
            len: 24,
        });
        assert!(!none.cut(), "no history, no bar");
        let some = scroll_of(ronoctua_term::ghostty::Scrollbar {
            total: 1024,
            offset: 1000,
            len: 24,
        });
        assert!(some.cut());
        // Through `bar` and `offset_at`, the pair a window draws and grabs
        // with — never the thumb's own arithmetic beside them.
        let (height, unit) = (480.0f32, 20.0f32);
        let middle = |s: ronoctua_design::Scroll| {
            let thumb = bar(s, 400.0, 0.0, height, unit).1.expect("a thumb");
            offset_at(s, 0.0, height, unit, thumb.1 + thumb.3 / 2.0)
        };
        assert_eq!(middle(some), 1000);
        assert_eq!(middle(ronoctua_design::Scroll { offset: 0, ..some }), 0);
    }

    #[test]
    fn the_bar_runs_the_whole_margin_at_any_size_of_the_terminals_text() {
        // FAILS IF the bar's track is counted in the interface's lines and
        // not the terminal's rows: a window zoomed past the interface's line
        // draws its last page's thumb short of the bottom, and a press at the
        // bottom of the margin is not the last page.
        let line = tokens::metrics::line_height(tokens::text::size());
        for row in [line * 0.7, line * 2.0] {
            let rows = 20usize;
            #[allow(clippy::cast_precision_loss)]
            let height = rows as f32 * row + row / 3.0;
            let last = ronoctua_design::Scroll {
                total: 1000,
                lines: rows,
                offset: 980,
                top: 0,
            };
            let (_, y, _, tall) = thumb(last, 800.0, 0.0, height, row).expect("a thumb");
            assert!(
                (y + tall - height).abs() < 0.5,
                "the last page's thumb ends at {} of {height} (row {row})",
                y + tall
            );
            assert_eq!(offset_at(last, 0.0, height, row, height), 980);
            let first = ronoctua_design::Scroll { offset: 0, ..last };
            let (_, y, _, tall) = thumb(first, 800.0, 0.0, height, row).expect("a thumb");
            assert!(y.abs() < 0.5);
            assert_eq!(offset_at(first, 0.0, height, row, y + tall / 2.0), 0);
            // AND THE MIDDLE IS WHERE IT IS DRAWN.
            let middle = ronoctua_design::Scroll {
                offset: 490,
                ..last
            };
            let (_, y, _, tall) = thumb(middle, 800.0, 0.0, height, row).unwrap();
            assert_eq!(offset_at(middle, 0.0, height, row, y + tall / 2.0), 490);
        }
        // NO HISTORY, NO BAR.
        let fits = ronoctua_design::Scroll {
            total: 20,
            lines: 20,
            offset: 0,
            top: 0,
        };
        assert_eq!(thumb(fits, 800.0, 0.0, 400.0, line), None);
        assert_eq!(bar(fits, 800.0, 0.0, 400.0, line), (None, None));
    }

    #[test]
    fn the_strip_takes_its_height_off_the_top_of_the_grid_and_the_bar() {
        // FAILS IF the grid is laid out over a group's strip (its first row
        // under the lines, a click on a line taken for a cell), the rows it
        // covers are still counted (the program told a row it cannot see),
        // or the bar's track runs up into the strip — grabbed there, it
        // would be both a line and the bar.
        for scale in [1u32, 2] {
            let g = geometry(9 * scale, 19 * scale);
            let top = crate::terminal_strip::height(2);
            assert!(top > 0);
            let free = Placed::of(640, 400, scale, &g);
            let under = Placed::under(640, 400, scale, top, &g);
            assert_eq!(under.origin, (free.origin.0, top * scale));
            assert_eq!(under.columns, free.columns);
            assert_eq!(
                u32::from(under.rows),
                (400 * scale - top * scale) / g.cell_height,
                "the rows under the strip"
            );
            // A point on the strip is on no cell; one just under it is row 0.
            let x = f64::from(free.origin.0) / f64::from(scale) + 0.5;
            assert_eq!(under.cell_at(x, f64::from(top) - 0.5), None);
            assert_eq!(under.cell_at(x, f64::from(top) + 0.5), Some((0, 0)));
        }
        let row = 20.0f32;
        let some = ronoctua_design::Scroll {
            total: 1000,
            lines: 20,
            offset: 0,
            top: 0,
        };
        let top = 7.0f32;
        let (track, first) = bar(some, 800.0, top, 410.0, row);
        let (_, y, _, tall) = first.expect("a thumb");
        assert!(
            (track.expect("a track").1 - top).abs() < 0.01,
            "the track starts under the strip"
        );
        assert!(
            (y - top).abs() < 0.01,
            "the first page's thumb at the strip's foot"
        );
        assert_eq!(offset_at(some, top, 410.0, row, y + tall / 2.0), 0);
        let last = ronoctua_design::Scroll {
            offset: 980,
            ..some
        };
        assert_eq!(offset_at(last, top, 410.0, row, 410.0), 980);
    }

    #[test]
    fn every_button_a_program_can_be_told_is_one() {
        // FAILS IF a button a program tracking the mouse asks for is dropped
        // (the middle one is how a pager pastes), or one is told for another.
        use ronoctua_shell_door::door::{BACK_BUTTON, LEFT_BUTTON, RIGHT_BUTTON};
        assert_eq!(button_of(LEFT_BUTTON), Some(wire::Button::Left));
        assert_eq!(button_of(RIGHT_BUTTON), Some(wire::Button::Right));
        assert_eq!(button_of(MIDDLE_BUTTON), Some(wire::Button::Middle));
        assert_eq!(button_of(BACK_BUTTON), Some(wire::Button::Back));
        assert_eq!(button_of(FORWARD_BUTTON), Some(wire::Button::Forward));
        assert_eq!(button_of(0x200), None);
    }
}
