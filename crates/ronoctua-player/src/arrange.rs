//! Where each picture of a set goes on a screen.
//!
//! # One placement for every door
//!
//! The window, the column, the stage, the picture-in-picture and the footer's
//! panel all show the same pictures, and they place them with [`place`] — so
//! "side by side" is the same shape wherever it is drawn, and a test of it is
//! a test of all of them. It is pure and has no tokens: the door hands in its
//! own area and air ([`Air`]) and draws what comes back.
//!
//! # A picture is fitted, never squashed
//!
//! Every rectangle that comes back has its tile's own shape. When the area is
//! too small for all of them at [`Air::floor_px`], the LAST ones are dropped
//! — a picture too small to see says nothing, and a stretched one lies — and
//! the first is always kept.

use ronoctua_playlist::own::{Arrangement, Form, Lane, LaneId, LaneKind};

/// The shape a picture is given before its size is known.
pub const DEFAULT_ASPECT: f32 = 16.0 / 9.0;

/// A picture to place: its lane, and its width over its height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tile {
    /// Whose picture.
    pub lane: LaneId,
    /// Width over height.
    pub aspect: f32,
}

/// A rectangle, in the unit of the area it was placed in (a door's logical
/// pixels).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// The right edge.
    #[must_use]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// The bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    /// How much of the screen it covers.
    #[must_use]
    pub fn area(&self) -> f32 {
        self.width * self.height
    }
}

/// Where each picture went: its lane and its rectangle, in the tiles' order.
pub type Placement = Vec<(LaneId, Rect)>;

/// The air a door keeps between pictures, from its tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Air {
    /// Between two pictures, and around the corner pictures.
    pub gap_px: f32,
    /// How wide (and tall) a corner picture's box is, in percent of the
    /// area's.
    pub corner_pct: u8,
    /// The smallest side a picture may have; below it, it is dropped.
    pub floor_px: f32,
}

/// A width over a height that means something, else [`DEFAULT_ASPECT`].
fn sane(aspect: f32) -> f32 {
    if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        DEFAULT_ASPECT
    }
}

/// How many, as a float.
fn count(n: usize) -> f32 {
    u16::try_from(n).map_or(f32::from(u16::MAX), f32::from)
}

/// The pictures of `lanes` that `screen` shows, in the arrangement's order.
///
/// A picture is a lane that is on, not hidden, routed to this screen, and
/// either a video (shaped by `aspect_of`, else [`DEFAULT_ASPECT`] until it is
/// known) or a sound whose cover the person chose to show and that HAS one —
/// `aspect_of` answers `None` for a sound with no cover. `screen` is `None`
/// for a door that does not know its screen yet, which shows every picture.
#[must_use]
pub fn tiles(
    lanes: &[Lane],
    arrangement: &Arrangement,
    screen: Option<&str>,
    aspect_of: impl Fn(LaneId) -> Option<f32>,
) -> Vec<Tile> {
    let found = lanes
        .iter()
        .filter(|lane| {
            lane.enabled && !arrangement.hidden.contains(&lane.id) && lane.screens.shows_on(screen)
        })
        .filter_map(|lane| match lane.kind {
            LaneKind::Video => Some(Tile {
                lane: lane.id,
                aspect: aspect_of(lane.id).map_or(DEFAULT_ASPECT, sane),
            }),
            LaneKind::Audio if arrangement.covers.contains(&lane.id) => {
                aspect_of(lane.id).map(|aspect| Tile {
                    lane: lane.id,
                    aspect: sane(aspect),
                })
            }
            LaneKind::Audio | LaneKind::Subtitle => None,
        })
        .collect();
    ordered(arrangement, found)
}

/// `tiles` in the arrangement's order: the ones it names first, as it names
/// them, then the rest as they came.
#[must_use]
pub fn ordered(arrangement: &Arrangement, mut tiles: Vec<Tile>) -> Vec<Tile> {
    tiles.sort_by_key(|tile| {
        arrangement
            .order
            .iter()
            .position(|id| *id == tile.lane)
            .map_or((1, 0), |at| (0, at))
    });
    tiles
}

/// The shape — width over height — the pictures want together, for a door
/// that chooses how tall its picture band is.
///
/// `Auto` asks for the grid's shape, the square-ish middle of the three it
/// chooses from, and then chooses within the band it is given.
#[must_use]
pub fn natural_aspect(form: Form, tiles: &[Tile]) -> f32 {
    let Some(first) = tiles.first().map(|tile| sane(tile.aspect)) else {
        return DEFAULT_ASPECT;
    };
    let n = tiles.len();
    let aspects = tiles.iter().map(|tile| sane(tile.aspect));
    match form {
        Form::Single | Form::Corner => first,
        _ if n == 1 => first,
        Form::SideBySide => aspects.sum(),
        Form::Stacked => 1.0 / aspects.map(|a| 1.0 / a).sum::<f32>(),
        // THE MAIN PICTURE is two thirds of the height, the strip one.
        Form::Strip => first * 2.0 / 3.0,
        Form::Grid | Form::Auto => {
            let (columns, rows) = square_ish(n);
            count(columns) * (aspects.sum::<f32>() / count(n)) / count(rows)
        }
    }
}

/// The columns and rows of the grid closest to a square for `n`.
fn square_ish(n: usize) -> (usize, usize) {
    let mut columns = 1;
    while columns * columns < n {
        columns += 1;
    }
    (columns, n.div_ceil(columns))
}

/// Where each picture goes in `area`: at most one rectangle per tile, in the
/// tiles' order, each with its tile's shape.
///
/// Tiles are dropped from the END when a picture would be smaller than
/// [`Air::floor_px`] or would not fit; the first is always placed (unless
/// the area is empty). `Corner` is the one form whose pictures overlap: the
/// others sit ON the first, in its bottom-right corner, because that is what
/// the person chose it for.
#[must_use]
pub fn place(form: Form, tiles: &[Tile], area: Rect, air: Air) -> Placement {
    let tiles: Vec<Tile> = tiles
        .iter()
        .map(|tile| Tile {
            lane: tile.lane,
            aspect: sane(tile.aspect),
        })
        .collect();
    let usable =
        area.width.is_finite() && area.height.is_finite() && area.width > 0.0 && area.height > 0.0;
    if tiles.is_empty() || !usable {
        return Vec::new();
    }
    let air = Air {
        gap_px: if air.gap_px.is_finite() {
            air.gap_px.max(0.0)
        } else {
            0.0
        },
        corner_pct: air.corner_pct.min(100),
        floor_px: if air.floor_px.is_finite() {
            air.floor_px.max(0.0)
        } else {
            0.0
        },
    };
    match form {
        Form::Auto => auto(&tiles, area, air),
        form => within_the_floor(form, &tiles, area, air),
    }
}

/// Whichever of side by side, stacked and grid shows the most: the most
/// pictures, then the most picture.
fn auto(tiles: &[Tile], area: Rect, air: Air) -> Placement {
    let mut best: Option<Placement> = None;
    for form in [Form::SideBySide, Form::Stacked, Form::Grid] {
        let placed = within_the_floor(form, tiles, area, air);
        let better = best.as_ref().is_none_or(|best| {
            placed.len() > best.len()
                || (placed.len() == best.len() && covered(&placed) > covered(best) + 0.5)
        });
        if better {
            best = Some(placed);
        }
    }
    best.unwrap_or_default()
}

/// How much picture a placement shows.
fn covered(placed: &[(LaneId, Rect)]) -> f32 {
    placed.iter().map(|(_, rect)| rect.area()).sum()
}

/// `form` with as many of the tiles as fit above the floor.
fn within_the_floor(form: Form, tiles: &[Tile], area: Rect, air: Air) -> Placement {
    let mut kept = tiles.len();
    loop {
        let placed = raw(form, &tiles[..kept], area, air);
        let fine = placed.iter().all(|(_, rect)| {
            rect.width > 0.0
                && rect.height > 0.0
                && rect.width >= air.floor_px
                && rect.height >= air.floor_px
                && inside(rect, area)
        });
        if fine || kept == 1 {
            return placed;
        }
        kept -= 1;
    }
}

/// Whether `rect` is inside `area`, forgiving a rounding.
fn inside(rect: &Rect, area: Rect) -> bool {
    const SLACK: f32 = 0.01;
    rect.x >= area.x - SLACK
        && rect.y >= area.y - SLACK
        && rect.right() <= area.right() + SLACK
        && rect.bottom() <= area.bottom() + SLACK
}

/// The largest rectangle of `aspect` inside `room`, centred in it.
fn fitted(room: Rect, aspect: f32) -> Rect {
    if room.width <= 0.0 || room.height <= 0.0 {
        return Rect {
            x: room.x,
            y: room.y,
            width: 0.0,
            height: 0.0,
        };
    }
    let (width, height) = if room.width / room.height > aspect {
        (room.height * aspect, room.height)
    } else {
        (room.width, room.width / aspect)
    };
    Rect {
        x: room.x + (room.width - width) / 2.0,
        y: room.y + (room.height - height) / 2.0,
        width,
        height,
    }
}

/// Every tile of `form` placed, with no floor.
fn raw(form: Form, tiles: &[Tile], area: Rect, air: Air) -> Placement {
    let gap = air.gap_px;
    match form {
        Form::Single => tiles
            .first()
            .map(|tile| (tile.lane, fitted(area, tile.aspect)))
            .into_iter()
            .collect(),
        Form::SideBySide => row(tiles, area, gap),
        Form::Stacked => column(tiles, area, gap),
        // `Auto` never reaches here (`place` chooses first); the grid is its
        // middle answer.
        Form::Grid | Form::Auto => grid(tiles, area, gap),
        Form::Strip => strip(tiles, area, gap),
        Form::Corner => corner(tiles, area, air),
    }
}

/// In a row: one height for all, each as wide as its shape asks.
fn row(tiles: &[Tile], area: Rect, gap: f32) -> Placement {
    let gaps = gap * (count(tiles.len()) - 1.0);
    let shapes: f32 = tiles.iter().map(|tile| tile.aspect).sum();
    let height = ((area.width - gaps).max(0.0) / shapes).min(area.height);
    let total = shapes * height + gaps;
    let mut x = area.x + (area.width - total) / 2.0;
    let y = area.y + (area.height - height) / 2.0;
    tiles
        .iter()
        .map(|tile| {
            let width = tile.aspect * height;
            let rect = Rect {
                x,
                y,
                width,
                height,
            };
            x += width + gap;
            (tile.lane, rect)
        })
        .collect()
}

/// In a column: one width for all, each as tall as its shape asks.
fn column(tiles: &[Tile], area: Rect, gap: f32) -> Placement {
    let gaps = gap * (count(tiles.len()) - 1.0);
    let shapes: f32 = tiles.iter().map(|tile| 1.0 / tile.aspect).sum();
    let width = ((area.height - gaps).max(0.0) / shapes).min(area.width);
    let total = shapes * width + gaps;
    let x = area.x + (area.width - width) / 2.0;
    let mut y = area.y + (area.height - total) / 2.0;
    tiles
        .iter()
        .map(|tile| {
            let height = width / tile.aspect;
            let rect = Rect {
                x,
                y,
                width,
                height,
            };
            y += height + gap;
            (tile.lane, rect)
        })
        .collect()
}

/// In rows and columns of equal cells, each picture fitted in its cell and
/// the last row centred; the number of columns is the one whose SMALLEST
/// picture is largest, then whose pictures cover the most.
fn grid(tiles: &[Tile], area: Rect, gap: f32) -> Placement {
    let n = tiles.len();
    let mut best: Option<(f32, f32, Placement)> = None;
    for columns in 1..=n {
        let rows = n.div_ceil(columns);
        let cell_width = (area.width - gap * (count(columns) - 1.0)) / count(columns);
        let cell_height = (area.height - gap * (count(rows) - 1.0)) / count(rows);
        if cell_width <= 0.0 || cell_height <= 0.0 {
            continue;
        }
        let placed: Placement = tiles
            .iter()
            .enumerate()
            .map(|(i, tile)| {
                let (row, col) = (i / columns, i % columns);
                let in_row = (n - row * columns).min(columns);
                let shift = count(columns - in_row) * (cell_width + gap) / 2.0;
                let cell = Rect {
                    x: area.x + shift + count(col) * (cell_width + gap),
                    y: area.y + count(row) * (cell_height + gap),
                    width: cell_width,
                    height: cell_height,
                };
                (tile.lane, fitted(cell, tile.aspect))
            })
            .collect();
        let smallest = placed
            .iter()
            .map(|(_, rect)| rect.area())
            .fold(f32::INFINITY, f32::min);
        let total = covered(&placed);
        let better = best.as_ref().is_none_or(|(least, most, _)| {
            smallest > least + 0.5 || ((smallest - least).abs() <= 0.5 && total > most + 0.5)
        });
        if better {
            best = Some((smallest, total, placed));
        }
    }
    best.map_or_else(|| degenerate(tiles, area), |(_, _, placed)| placed)
}

/// Every tile at the area's corner with no size: what a layout that cannot
/// fit anything gives, so the floor drops the tiles.
fn degenerate(tiles: &[Tile], area: Rect) -> Placement {
    tiles
        .iter()
        .map(|tile| {
            (
                tile.lane,
                Rect {
                    x: area.x,
                    y: area.y,
                    width: 0.0,
                    height: 0.0,
                },
            )
        })
        .collect()
}

/// The first large on top, the others side by side in a strip a third of the
/// height under it.
fn strip(tiles: &[Tile], area: Rect, gap: f32) -> Placement {
    let Some((first, others)) = tiles.split_first() else {
        return Vec::new();
    };
    if others.is_empty() {
        return vec![(first.lane, fitted(area, first.aspect))];
    }
    let strip_height = (area.height - gap) / 3.0;
    let main = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height - gap - strip_height,
    };
    let k = count(others.len());
    let cell_width = (area.width - gap * (k - 1.0)) / k;
    let top = main.bottom() + gap;
    std::iter::once((first.lane, fitted(main, first.aspect)))
        .chain(others.iter().enumerate().map(|(i, tile)| {
            let cell = Rect {
                x: area.x + count(i) * (cell_width + gap),
                y: top,
                width: cell_width,
                height: strip_height,
            };
            (tile.lane, fitted(cell, tile.aspect))
        }))
        .collect()
}

/// The first filling the area; the others each fitted in a box
/// [`Air::corner_pct`] of the area, stacked up from the bottom-right corner,
/// `gap` from its edges and from each other.
fn corner(tiles: &[Tile], area: Rect, air: Air) -> Placement {
    let Some((first, others)) = tiles.split_first() else {
        return Vec::new();
    };
    let share = f32::from(air.corner_pct) / 100.0;
    let room = Rect {
        x: 0.0,
        y: 0.0,
        width: area.width * share,
        height: area.height * share,
    };
    let mut bottom = area.bottom() - air.gap_px;
    std::iter::once((first.lane, fitted(area, first.aspect)))
        .chain(others.iter().map(|tile| {
            let size = fitted(room, tile.aspect);
            let rect = Rect {
                x: area.right() - air.gap_px - size.width,
                y: bottom - size.height,
                width: size.width,
                height: size.height,
            };
            bottom = rect.y - air.gap_px;
            (tile.lane, rect)
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use ronoctua_playlist::own::Screens;

    use super::*;

    /// A tile of lane `n`.
    fn tile(n: u32, aspect: f32) -> Tile {
        Tile {
            lane: LaneId(n),
            aspect,
        }
    }

    /// An area at the origin.
    fn area(width: f32, height: f32) -> Rect {
        Rect {
            x: 10.0,
            y: 20.0,
            width,
            height,
        }
    }

    /// The air a door might keep.
    const AIR: Air = Air {
        gap_px: 4.0,
        corner_pct: 25,
        floor_px: 48.0,
    };

    /// How far apart two rectangles are: the larger of the horizontal and
    /// the vertical gap, negative when they overlap.
    fn apart(a: &Rect, b: &Rect) -> f32 {
        let across = (b.x - a.right()).max(a.x - b.right());
        let down = (b.y - a.bottom()).max(a.y - b.bottom());
        across.max(down)
    }

    /// Checks every promise of [`place`] for one case, and says which broke.
    fn promises(form: Form, tiles: &[Tile], area: Rect, air: Air) -> Result<(), String> {
        let placed = place(form, tiles, area, air);
        let case = format!("{form:?} n={} area={area:?} air={air:?}", tiles.len());
        if placed.is_empty() {
            return Err(format!("nothing placed: {case}"));
        }
        // A PREFIX: what is dropped is dropped from the end.
        for ((lane, _), tile) in placed.iter().zip(tiles) {
            if *lane != tile.lane {
                return Err(format!("not a prefix of the order: {case}"));
            }
        }
        if form == Form::Single && placed.len() != 1 {
            return Err(format!("single placed several: {case}"));
        }
        for ((_, rect), tile) in placed.iter().zip(tiles) {
            if ![rect.x, rect.y, rect.width, rect.height]
                .iter()
                .all(|v| v.is_finite())
            {
                return Err(format!("not a number: {case}"));
            }
            if !inside(rect, area) {
                return Err(format!("outside the area {rect:?}: {case}"));
            }
            if rect.width <= 0.0 || rect.height <= 0.0 {
                return Err(format!("no size {rect:?}: {case}"));
            }
            // FITTED, NEVER SQUASHED.
            let shape = rect.width / rect.height;
            if (shape - tile.aspect).abs() > tile.aspect * 0.001 {
                return Err(format!("squashed {shape} for {}: {case}", tile.aspect));
            }
            if placed.len() > 1
                && (rect.width < air.floor_px - 0.01 || rect.height < air.floor_px - 0.01)
            {
                return Err(format!("below the floor {rect:?}: {case}"));
            }
        }
        for (i, (_, a)) in placed.iter().enumerate() {
            for (_, b) in placed.iter().skip(i + 1) {
                // THE CORNER'S PICTURES sit on the first by design; among
                // themselves they keep the air.
                if form == Form::Corner && i == 0 {
                    continue;
                }
                if apart(a, b) < air.gap_px - 0.01 {
                    return Err(format!("{a:?} and {b:?} are {} apart: {case}", apart(a, b)));
                }
            }
        }
        if form == Form::Corner {
            for (_, rect) in placed.iter().skip(1) {
                if area.right() - rect.right() < air.gap_px - 0.01
                    || area.bottom() - rect.bottom() < air.gap_px - 0.01
                {
                    return Err(format!("a corner picture touches the edge: {case}"));
                }
            }
        }
        Ok(())
    }

    /// Shapes pictures really have: portrait phone, 4:3, square, wide, cinema.
    const SHAPES: [f32; 7] = [9.0 / 16.0, 0.75, 1.0, 4.0 / 3.0, 16.0 / 9.0, 2.39, 1.5];

    /// Areas doors really have: a screen each way, a window, a column, a
    /// picture-in-picture and one too small for two pictures.
    const AREAS: [(f32, f32); 7] = [
        (1920.0, 1080.0),
        (1080.0, 1920.0),
        (800.0, 450.0),
        (400.0, 900.0),
        (320.0, 180.0),
        (300.0, 120.0),
        (60.0, 40.0),
    ];

    /// FAILS IF: any form, for one to eight pictures of any shape in any
    /// area, places a picture outside the area, on another (the corner's
    /// excepted), closer than the gap, squashed, or below the floor — or
    /// drops anything but the last.
    #[test]
    fn every_form_keeps_its_promises_for_one_to_eight_pictures() {
        let airs = [
            AIR,
            Air {
                gap_px: 0.0,
                corner_pct: 30,
                floor_px: 0.0,
            },
            Air {
                gap_px: 12.0,
                corner_pct: 20,
                floor_px: 96.0,
            },
        ];
        for n in 1..=8usize {
            for rotate in 0..SHAPES.len() {
                let tiles: Vec<Tile> = (0..n)
                    .map(|i| {
                        let lane = u32::try_from(i).unwrap_or(0) + 1;
                        tile(lane, SHAPES[(i * 3 + rotate) % SHAPES.len()])
                    })
                    .collect();
                for (width, height) in AREAS {
                    for air in airs {
                        for form in Form::ALL {
                            if let Err(broken) = promises(form, &tiles, area(width, height), air) {
                                panic!("{broken}");
                            }
                        }
                    }
                }
            }
        }
    }

    /// FAILS IF: `Auto` ever shows fewer pictures, or less picture, than one
    /// of the three forms it chooses from.
    #[test]
    fn auto_shows_the_most() {
        for n in 1..=8usize {
            let tiles: Vec<Tile> = (0..n)
                .map(|i| tile(u32::try_from(i).unwrap_or(0) + 1, SHAPES[i % SHAPES.len()]))
                .collect();
            for (width, height) in AREAS {
                let a = area(width, height);
                let auto = place(Form::Auto, &tiles, a, AIR);
                for form in [Form::SideBySide, Form::Stacked, Form::Grid] {
                    let other = place(form, &tiles, a, AIR);
                    assert!(
                        auto.len() > other.len()
                            || (auto.len() == other.len()
                                && covered(&auto) + 0.5 >= covered(&other)),
                        "{form:?} beats auto for n={n} in {width}x{height}"
                    );
                }
            }
        }
        // TWO WIDE PICTURES on a tall screen go one over the other.
        let wide = [tile(1, 16.0 / 9.0), tile(2, 16.0 / 9.0)];
        let tall = place(Form::Auto, &wide, area(1080.0, 1920.0), AIR);
        assert!(tall[1].1.y > tall[0].1.bottom(), "{tall:?}");
        // AND ON A WIDE SCREEN, two tall ones side by side.
        let portrait = [tile(1, 9.0 / 16.0), tile(2, 9.0 / 16.0)];
        let side = place(Form::Auto, &portrait, area(1920.0, 1080.0), AIR);
        assert!(side[1].1.x > side[0].1.right(), "{side:?}");
    }

    /// FAILS IF: a picture is squashed to keep a tile the area has no room
    /// for, or the first picture is ever dropped.
    #[test]
    fn below_the_floor_the_last_is_dropped_and_the_first_kept() {
        let tiles = [
            tile(1, 16.0 / 9.0),
            tile(2, 16.0 / 9.0),
            tile(3, 16.0 / 9.0),
        ];
        let air = Air {
            gap_px: 4.0,
            corner_pct: 25,
            floor_px: 100.0,
        };
        let placed = place(Form::SideBySide, &tiles, area(400.0, 300.0), air);
        assert_eq!(placed.len(), 2, "{placed:?}");
        let tiny = place(Form::SideBySide, &tiles, area(60.0, 40.0), air);
        assert_eq!(tiny.len(), 1);
        assert_eq!(tiny[0].0, LaneId(1));
        assert!(place(Form::Grid, &tiles, area(0.0, 300.0), air).is_empty());
        assert!(place(Form::Grid, &[], area(400.0, 300.0), air).is_empty());
    }

    /// FAILS IF: one form stops having its own shape.
    #[test]
    fn each_form_has_its_shape() {
        let a = area(1600.0, 900.0);
        let three = [
            tile(1, 16.0 / 9.0),
            tile(2, 16.0 / 9.0),
            tile(3, 16.0 / 9.0),
        ];
        // SINGLE: the first, filling.
        let single = place(Form::Single, &three, a, AIR);
        assert_eq!(single.len(), 1);
        assert!((single[0].1.width - 1600.0).abs() < 0.01);
        // SIDE BY SIDE: one row.
        let side = place(Form::SideBySide, &three, a, AIR);
        assert!(side.iter().all(|(_, r)| (r.y - side[0].1.y).abs() < 0.01));
        // STACKED: one column.
        let stacked = place(Form::Stacked, &three, a, AIR);
        assert!(
            stacked
                .iter()
                .all(|(_, r)| (r.x - stacked[0].1.x).abs() < 0.01)
        );
        // GRID: two over one, the one centred.
        let grid = place(Form::Grid, &three, a, AIR);
        assert!((grid[0].1.y - grid[1].1.y).abs() < 0.01 && grid[2].1.y > grid[0].1.bottom());
        let middle = a.x + a.width / 2.0;
        assert!((grid[2].1.x + grid[2].1.width / 2.0 - middle).abs() < 0.01);
        // STRIP: the first over a strip of the others.
        let strip = place(Form::Strip, &three, a, AIR);
        assert!(strip[0].1.area() > strip[1].1.area() * 3.0);
        assert!(strip[1].1.y >= strip[0].1.bottom() + AIR.gap_px - 0.01);
        // CORNER: the first filling, the other small at the bottom right.
        let corner = place(Form::Corner, &three[..2], a, AIR);
        assert!((corner[0].1.width - 1600.0).abs() < 0.01);
        assert!((corner[1].1.right() - (a.right() - AIR.gap_px)).abs() < 0.01);
        assert!((corner[1].1.width - 400.0).abs() < 0.01);
    }

    /// FAILS IF: the arrangement's order, its hidden pictures, its covers or
    /// a lane's screens stop deciding which pictures a door shows.
    #[test]
    fn only_the_pictures_routed_here_are_tiles_in_the_arrangement_s_order() {
        let mut on_one = Lane::new(LaneId(2), LaneKind::Video, "/b.mkv");
        on_one.screens = Screens::Only(vec!["DP-1".to_owned()]);
        let mut off = Lane::new(LaneId(5), LaneKind::Video, "/e.mkv");
        off.enabled = false;
        let lanes = vec![
            Lane::new(LaneId(1), LaneKind::Video, "/a.mkv"),
            on_one,
            Lane::new(LaneId(3), LaneKind::Audio, "/c.flac"),
            Lane::new(LaneId(4), LaneKind::Audio, "/d.flac"),
            off,
            Lane::new(LaneId(6), LaneKind::Video, "/f.mkv"),
            Lane::new(LaneId(7), LaneKind::Subtitle, "/g.srt"),
        ];
        let arrangement = Arrangement {
            form: Form::Grid,
            order: vec![LaneId(3), LaneId(2)],
            hidden: vec![LaneId(6)],
            covers: vec![LaneId(3), LaneId(4)],
        };
        // LANE 4 has no cover, whatever the arrangement says.
        let aspect_of = |id: LaneId| match id.0 {
            1 => Some(2.0),
            3 => Some(1.0),
            _ => None,
        };
        let ids = |screen| {
            tiles(&lanes, &arrangement, screen, aspect_of)
                .into_iter()
                .map(|t| t.lane.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(Some("DP-1")), vec![3, 2, 1]);
        assert_eq!(ids(Some("HDMI-A-1")), vec![3, 1]);
        // A DOOR THAT DOES NOT KNOW ITS SCREEN shows everything routed anywhere.
        assert_eq!(ids(None), vec![3, 2, 1]);
        let shown = tiles(&lanes, &arrangement, None, aspect_of);
        assert_eq!(shown[2].aspect, 2.0);
        assert_eq!(
            shown[1].aspect, DEFAULT_ASPECT,
            "an unknown size is not its shape"
        );
    }

    /// FAILS IF: the band a door sizes for its pictures stops matching what
    /// the form will put in it.
    #[test]
    fn the_natural_shape_is_the_form_s() {
        let two = [tile(1, 16.0 / 9.0), tile(2, 4.0 / 3.0)];
        assert!((natural_aspect(Form::SideBySide, &two) - (16.0 / 9.0 + 4.0 / 3.0)).abs() < 1e-4);
        let stacked = 1.0 / (9.0 / 16.0 + 3.0 / 4.0);
        assert!((natural_aspect(Form::Stacked, &two) - stacked).abs() < 1e-4);
        assert!((natural_aspect(Form::Single, &two) - 16.0 / 9.0).abs() < 1e-4);
        assert!((natural_aspect(Form::Grid, &[tile(1, 1.0); 4]) - 1.0).abs() < 1e-4);
        assert!((natural_aspect(Form::Auto, &[]) - DEFAULT_ASPECT).abs() < 1e-4);
        // A NONSENSE SHAPE is taken as the default rather than poisoning the
        // sum.
        assert!(natural_aspect(Form::SideBySide, &[tile(1, f32::NAN)]).is_finite());
    }
}
