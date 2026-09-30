//! A globe, as geometry in the box's own pixels.
//!
//! # The horizon is the whole of the hard part
//!
//! An orthographic projection shows one hemisphere, and the obvious way to draw
//! a ring that straddles the edge — drop the vertices whose `z` is negative —
//! is WRONG in a way that does not look like a bug in the clipping. It joins
//! the last visible vertex straight to the next one, drawing a CHORD through
//! the middle of the planet; the symptom is Africa growing a flat side, which
//! reads as bad data.
//!
//! So there are two halves. Each EDGE that crosses is cut at the horizon, by
//! bisecting along the great circle — eight halvings put the crossing within
//! 1/256 of the edge, which at 611 m data is far under a pixel. And between
//! leaving and coming back the path walks the RIM, in the direction that keeps
//! the feature's interior inside; that is what closes Africa when half of it
//! has gone round the back.
//!
//! # Zoom is scale and pan is centre, and they are not the same thing
//!
//! `zoom` multiplies the radius; above 1 the disc is bigger than the box and
//! the box is a window onto it. The centre is what decides where that window
//! is. Conflating them is how a zoomed globe leaves the screen in one flick,
//! which is why the pan step divides by the zoom — and why it divides by
//! `cos(latitude)` as well, so dragging near a pole feels attached rather than
//! geared.

use alloc::{collections::BTreeMap, vec::Vec};

use crate::{
    Point, Tongue,
    baked::Atlas,
    lookup::{country_at, lakes, rings_of},
};

/// WHAT A PIECE OF THE GLOBE IS, which becomes a colour in whoever draws.
///
/// [`ronoctua_syntax::Role`]'s twin, and the same wall: this crate is compiled
/// where a token value does not exist, so it names the PART and never the ink.
/// The one place that translates is `chrome::ink_color`, beside
/// `chrome::syntax_color`, and its `match` is exhaustive — so a variant added
/// here is a compile error in exactly one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    /// The disc, where there is no land.
    Ocean,
    /// Land.
    Land,
    /// The country the coordinate is in.
    Here,
    /// Inland water, painted over the land.
    Lake,
    /// A border between two countries.
    Border,
    /// A border between two regions of one country.
    Region,
    /// The edge of the disc.
    Rim,
    /// A named way, when a region's streets are installed.
    Street,
    /// The coordinate itself.
    Dot,
    /// The ring around the dot, so it reads over land and over sea both.
    ///
    /// Every mark wears one too.
    Halo,
    /// The mark somebody chose or is pointing at. Every other mark is a
    /// [`Ink::Dot`].
    Chosen,
}

/// One closed or open path, in the box's own pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    /// What part of the globe it is.
    pub ink: Ink,
    /// Whether it is filled (closed) or stroked (open).
    pub closed: bool,
    /// Its points, already projected.
    pub points: Vec<(f32, f32)>,
}

/// A whole view of the globe, ready to be painted.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawing {
    /// In painting order: later covers earlier, because there is no depth test.
    pub shapes: Vec<Shape>,
    /// Where the coordinate landed, when it is on this side.
    ///
    /// It is here and not worked out again by the caller because WHERE A THING
    /// IS DRAWN IS WHERE IT IS GRABBED — the same rule the scrollbars keep. One
    /// projection, one answer, and the door turns these pixels into the cell
    /// that answers a click.
    pub dot: Option<(f32, f32)>,
    /// The radius of the disc, in pixels.
    pub rim: f32,
    /// The marks that are on this side and inside the box, grouped, in
    /// painting order. Painted from these same numbers, and grabbed by them.
    pub marks: Vec<Placed>,
}

/// One thing to put on the globe: a station, a country, a city.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mark {
    /// Where it is.
    pub at: Point,
    /// The caller's name for it, handed back in [`Placed::members`].
    pub id: u32,
    /// How much it counts — a country's number of stations, or one.
    pub weight: u32,
}

/// The marks of one picture, and how they are drawn.
///
/// Every number is the caller's (a token, in pixels): this crate is compiled
/// where there is no token.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marks<'a> {
    /// Every mark, in any order: the grouping does not depend on it.
    pub all: &'a [Mark],
    /// Marks closer than this on screen, in pixels, are one.
    pub merge_px: f32,
    /// The radius of a group, in pixels, by its [`tier`].
    pub radii: [f32; 4],
    /// The id of the mark that is lit; the group that holds it is drawn
    /// [`Ink::Chosen`].
    pub lit: Option<u32>,
}

impl Marks<'_> {
    /// No marks at all.
    pub const NONE: Marks<'static> = Marks {
        all: &[],
        merge_px: 0.0,
        radii: [0.0; 4],
        lit: None,
    };
}

/// A group of marks as it lands on the picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// Where its centre is, in the box's own pixels: the heaviest member's.
    pub at: (f32, f32),
    /// Its radius, in pixels, by the tier of its weight.
    pub radius: f32,
    /// Every member's weight, added.
    pub weight: u32,
    /// Its members' ids, the heaviest first.
    pub members: Vec<u32>,
    /// Where its centre is on the planet: the heaviest member's.
    pub centre: Point,
}

/// How many marks one picture considers — the heaviest, when there are more.
///
/// A guard and not a preference, like [`VERTEX_CEILING`]: every list the
/// player draws is bounded far below it, and a caller that handed over a
/// whole directory would otherwise group it on every frame.
pub const MARKS_CEILING: usize = 4096;

/// Which of four sizes a group of this weight is drawn at: one; two to nine;
/// ten to ninety-nine; a hundred or more. Pure.
///
/// The picture has no text (the rasteriser draws none), so the SIZE is what
/// says "many here"; four steps are what the eye tells apart at a glance.
#[must_use]
pub const fn tier(weight: u32) -> usize {
    match weight {
        0..=1 => 0,
        2..=9 => 1,
        10..=99 => 2,
        _ => 3,
    }
}

/// Where the globe is pointed.
///
/// A plain value, compared as one — `solid::Pose` is the shape and the reason
/// is the same: the host compares two of them and knows whether a new picture
/// is wanted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// What is in the middle of the box.
    pub centre: Point,
    /// 1.0 is the whole disc inside the box.
    pub zoom: f32,
}

impl Pose {
    /// The whole world, pointed at nothing in particular.
    pub const WORLD: f32 = 1.0;

    /// As close as the baked table can honestly go.
    ///
    /// Past this the 611 m quantisation is visible as a staircase, and there is
    /// nothing finer to show until a region's streets are installed. The view
    /// says so rather than zooming into a lie.
    pub const LOCAL: f32 = 40.0;

    /// The furthest out anything is worth drawing.
    pub const FURTHEST: f32 = 0.6;

    /// Pointed at a coordinate, at the zoom a country fills.
    #[must_use]
    pub const fn on(at: Point) -> Self {
        Self {
            centre: at,
            zoom: 6.0,
        }
    }

    /// Moved by a drag, in pixels.
    ///
    /// The step divides by the zoom, so a close view moves slowly, and by the
    /// cosine of the latitude, so a drag near a pole moves the same amount of
    /// SCREEN as one at the equator. The cosine is clamped at a quarter or the
    /// gearing becomes a catapult.
    #[must_use]
    pub fn panned(self, right: f32, down: f32, radius: f32) -> Self {
        let r = (radius * self.zoom).max(1.0);
        let cos = crate::cos(self.centre.lat * core::f32::consts::PI / 180.0).clamp(0.25, 1.0);
        let lon = self.centre.lon - (right / r) * 180.0 / core::f32::consts::PI / cos;
        let lat = self.centre.lat + (down / r) * 180.0 / core::f32::consts::PI;
        Self {
            centre: Point {
                // THE LONGITUDE WRAPS and the latitude CLAMPS, and neither is a
                // detail: a globe that runs off the top has no orientation left
                // and spins on the last digit, which is what `PITCH_CEILING`
                // already says one figure along.
                lon: wrap(lon),
                lat: lat.clamp(-85.0, 85.0),
            },
            zoom: self.zoom,
        }
    }

    /// Nearer or further, within what the data can show.
    #[must_use]
    pub fn scaled(self, by: f32) -> Self {
        Self {
            centre: self.centre,
            zoom: (self.zoom * by).clamp(Self::FURTHEST, Self::LOCAL),
        }
    }
}

fn wrap(lon: f32) -> f32 {
    let mut l = lon;
    while l > 180.0 {
        l -= 360.0;
    }
    while l < -180.0 {
        l += 360.0;
    }
    l
}

/// How many vertices one picture may project.
///
/// A guard and not a preference: the baked table is bounded by the file, but a
/// street pack is not, and a caller that asked for a city at street zoom would
/// otherwise project a quarter of a million ways into a box four hundred pixels
/// across.
pub const VERTEX_CEILING: usize = 400_000;

/// The projection itself: a point on the sphere to the box, or nothing when it
/// is round the back.
struct Lens {
    sin_lat: f32,
    cos_lat: f32,
    lon0: f32,
    half: (f32, f32),
    radius: f32,
}

impl Lens {
    fn of(pose: Pose, box_px: (f32, f32)) -> Self {
        let r = box_px.0.min(box_px.1) * 0.5;
        let phi = pose.centre.lat * core::f32::consts::PI / 180.0;
        Self {
            sin_lat: crate::sin(phi),
            cos_lat: crate::cos(phi),
            lon0: pose.centre.lon,
            half: (box_px.0 * 0.5, box_px.1 * 0.5),
            radius: r * pose.zoom,
        }
    }

    /// The three-space point, in the frame where `z > 0` is this side.
    fn space(&self, lat: f32, lon: f32) -> (f32, f32, f32) {
        let p = lat * core::f32::consts::PI / 180.0;
        let d = wrap(lon - self.lon0) * core::f32::consts::PI / 180.0;
        let (sp, cp) = (crate::sin(p), crate::cos(p));
        let (sd, cd) = (crate::sin(d), crate::cos(d));
        (
            cp * sd,
            self.cos_lat * sp - self.sin_lat * cp * cd,
            self.sin_lat * sp + self.cos_lat * cp * cd,
        )
    }

    /// Where a point lands, when it is on this side.
    fn onto(&self, at: Point) -> Option<(f32, f32)> {
        let v = self.space(at.lat, at.lon);
        (v.2 > 0.0).then(|| self.flat(v))
    }

    fn flat(&self, v: (f32, f32, f32)) -> (f32, f32) {
        (
            self.half.0 + self.radius * v.0,
            self.half.1 - self.radius * v.1,
        )
    }

    /// Where an edge crosses the horizon, by bisecting along the great circle.
    fn horizon(&self, a: (f32, f32, f32), b: (f32, f32, f32)) -> (f32, f32) {
        let (mut lo, mut hi) = (a, b);
        for _ in 0..8 {
            let mid = normal((
                (lo.0 + hi.0) * 0.5,
                (lo.1 + hi.1) * 0.5,
                (lo.2 + hi.2) * 0.5,
            ));
            if (mid.2 > 0.0) == (lo.2 > 0.0) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        self.flat(normal(((lo.0 + hi.0) * 0.5, (lo.1 + hi.1) * 0.5, 0.0)))
    }
}

fn normal(v: (f32, f32, f32)) -> (f32, f32, f32) {
    let n = crate::sqrt(v.0 * v.0 + v.1 * v.1 + v.2 * v.2);
    if n <= 0.0 {
        return (0.0, 0.0, 1.0);
    }
    (v.0 / n, v.1 / n, v.2 / n)
}

/// One ring of degrees, clipped at the horizon and walked round the rim.
fn ring_to(
    lens: &Lens,
    ring: &[(f32, f32)],
    closed: bool,
    spent: &mut usize,
) -> Vec<Vec<(f32, f32)>> {
    let mut out = Vec::new();
    let mut run: Vec<(f32, f32)> = Vec::new();
    let mut exit: Option<(f32, f32)> = None;
    let mut last: Option<(f32, f32, f32)> = None;
    let n = ring.len();
    let steps = if closed { n + 1 } else { n };
    for k in 0..steps {
        let (lon, lat) = ring[k % n];
        *spent += 1;
        if *spent > VERTEX_CEILING {
            break;
        }
        let here = lens.space(lat, lon);
        match (last, here.2 > 0.0) {
            (None, true) => run.push(lens.flat(here)),
            (None, false) => {}
            (Some(prev), true) => {
                if prev.2 <= 0.0 {
                    // COMING BACK: cut the edge, and walk the rim from where it
                    // left. Without the walk the path leaps across the disc.
                    let enter = lens.horizon(prev, here);
                    if let Some(from) = exit.take() {
                        out_rim(lens, from, enter, &mut run);
                    }
                    run.push(enter);
                }
                run.push(lens.flat(here));
            }
            (Some(prev), false) => {
                if prev.2 > 0.0 {
                    let at = lens.horizon(prev, here);
                    run.push(at);
                    exit = Some(at);
                    if !closed {
                        out.push(core::mem::take(&mut run));
                    }
                }
            }
        }
        last = Some(here);
    }
    if !run.is_empty() {
        out.push(run);
    }
    out
}

/// The arc of the rim between two points on it.
fn out_rim(lens: &Lens, from: (f32, f32), to: (f32, f32), into: &mut Vec<(f32, f32)>) {
    let a0 = crate::atan2(from.1 - lens.half.1, from.0 - lens.half.0);
    let a1 = crate::atan2(to.1 - lens.half.1, to.0 - lens.half.0);
    let mut d = a1 - a0;
    // THE SHORT WAY ROUND. A feature that left the disc and came back has
    // travelled the near side of the rim; the long way would draw a path round
    // the planet nobody asked for.
    while d > core::f32::consts::PI {
        d -= 2.0 * core::f32::consts::PI;
    }
    while d < -core::f32::consts::PI {
        d += 2.0 * core::f32::consts::PI;
    }
    let steps = 16;
    for k in 1..steps {
        let a = a0 + d * (k as f32 / steps as f32);
        into.push((
            lens.half.0 + lens.radius * crate::cos(a),
            lens.half.1 + lens.radius * crate::sin(a),
        ));
    }
}

/// A circle, as a path.
fn circle(at: (f32, f32), r: f32) -> Vec<(f32, f32)> {
    let steps = 48;
    (0..steps)
        .map(|k| {
            let a = k as f32 / steps as f32 * 2.0 * core::f32::consts::PI;
            (at.0 + r * crate::cos(a), at.1 + r * crate::sin(a))
        })
        .collect()
}

/// Everything to draw, for one pose.
///
/// `ways` are extra polylines in degrees — a region's streets, when they are
/// installed. They arrive as geometry rather than as a type of this crate's
/// because whoever holds them also holds a filesystem, and this crate does not.
#[must_use]
pub fn project(
    atlas: &Atlas<'_>,
    pose: Pose,
    at: Option<Point>,
    box_px: (f32, f32),
    ways: &[Vec<(f32, f32)>],
    dot_radius: f32,
    marks: &Marks<'_>,
) -> Drawing {
    let lens = Lens::of(pose, box_px);
    let mut shapes = Vec::new();
    let mut spent = 0_usize;

    // THE DISC FIRST, because everything else is painted on top of it and there
    // is no depth test: the order IS the answer.
    shapes.push(Shape {
        ink: Ink::Ocean,
        closed: true,
        points: circle(lens.half, lens.radius),
    });

    let home = at.and_then(|p| country_at(atlas, p, Tongue::English).map(|(_, _, n)| n));
    let mut borders: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut n = 0_u16;
    let countries = rings_of(
        atlas.countries_bytes(),
        |c| {
            let _iso = (c.u8()?, c.u8()?);
            let _en = c.short()?;
            let _pt = c.short()?;
            Some(())
        },
        |(), _| true,
    );
    // WHICH FEATURE EACH RING BELONGS TO is not in `rings_of`'s answer, so the
    // country the coordinate is in is filled in a second pass over the same
    // section — cheap, and it keeps `rings_of` a walk rather than a query.
    for ring in &countries {
        for path in ring_to(&lens, ring, true, &mut spent) {
            if path.len() > 2 {
                shapes.push(Shape {
                    ink: Ink::Land,
                    closed: true,
                    points: path.clone(),
                });
                borders.push(path);
            }
        }
    }
    let _ = &mut n;
    if let (Some(which), Some(p)) = (home, at) {
        let _ = which;
        // THE COUNTRY UNDER THE COORDINATE, filled again in its own ink. It is
        // what "framed on the country" SHOWS as well as frames.
        let mine = rings_of(
            atlas.countries_bytes(),
            |c| {
                let _iso = (c.u8()?, c.u8()?);
                let _en = c.short()?;
                let _pt = c.short()?;
                Some(())
            },
            |(), bbox| {
                let qx = crate::baked::lon_to(p.lon);
                let qy = crate::baked::lat_to(p.lat);
                qx >= bbox.0 && qx <= bbox.2 && qy >= bbox.1 && qy <= bbox.3
            },
        );
        for ring in &mine {
            for path in ring_to(&lens, ring, true, &mut spent) {
                if path.len() > 2 {
                    shapes.push(Shape {
                        ink: Ink::Here,
                        closed: true,
                        points: path,
                    });
                }
            }
        }
    }

    for ring in lakes(atlas) {
        for path in ring_to(&lens, &ring, true, &mut spent) {
            if path.len() > 2 {
                shapes.push(Shape {
                    ink: Ink::Lake,
                    closed: true,
                    points: path,
                });
            }
        }
    }

    // THE STREETS, when a region's are installed. Open paths, stroked.
    for way in ways {
        for path in ring_to(&lens, way, false, &mut spent) {
            if path.len() > 1 {
                shapes.push(Shape {
                    ink: Ink::Street,
                    closed: false,
                    points: path,
                });
            }
        }
    }

    for path in borders {
        shapes.push(Shape {
            ink: Ink::Border,
            closed: false,
            points: path,
        });
    }

    // THE RIM LAST of the lines, so a coastline that reaches the edge is cut
    // cleanly by it.
    shapes.push(Shape {
        ink: Ink::Rim,
        closed: false,
        points: circle(lens.half, lens.radius),
    });

    // THE MARKS, after the rim and before the dot: a mark reaching the edge
    // is still whole, and the coordinate asked about stays on top of them.
    let placed = group(&lens, box_px, marks);
    for p in &placed {
        let lit = marks.lit.is_some_and(|id| p.members.contains(&id));
        shapes.push(Shape {
            ink: if lit { Ink::Chosen } else { Ink::Dot },
            closed: true,
            points: circle(p.at, p.radius),
        });
        shapes.push(Shape {
            ink: Ink::Halo,
            closed: false,
            points: circle(p.at, p.radius + 1.0),
        });
    }

    let dot = at.and_then(|p| lens.onto(p));
    if let Some(d) = dot {
        shapes.push(Shape {
            ink: Ink::Dot,
            closed: true,
            points: circle(d, dot_radius),
        });
        shapes.push(Shape {
            ink: Ink::Halo,
            closed: false,
            points: circle(d, dot_radius + 1.0),
        });
    }

    Drawing {
        shapes,
        dot,
        rim: lens.radius,
        marks: placed,
    }
}

/// The marks of `marks`, grouped as they land on a picture of `pose` in a box
/// of `box_px` — exactly what [`project`] puts in [`Drawing::marks`]. Pure.
#[must_use]
pub fn placed(marks: &Marks<'_>, pose: Pose, box_px: (f32, f32)) -> Vec<Placed> {
    group(&Lens::of(pose, box_px), box_px, marks)
}

/// A group while it is being made.
struct Growing {
    at: (f32, f32),
    weight: u32,
    members: Vec<u32>,
    centre: Point,
}

/// Groups the marks that are on this side and inside the box, by distance ON
/// SCREEN.
///
/// # Why the answer does not depend on the order given
///
/// The marks are SORTED first — heaviest first, then by id, then by where
/// they are — and walked in that order. A mark joins the nearest group whose
/// centre is closer than `merge_px`; otherwise it starts one, centred on
/// itself. So a group's centre is always its heaviest member, and the same
/// marks in any order are the same groups.
///
/// A grid of `merge_px` cells finds the nearby groups: a mark looks at its
/// own cell and the eight around it, which is every centre that can be closer
/// than one cell.
///
/// The lit group is painted last, so it is never under another.
fn group(lens: &Lens, box_px: (f32, f32), marks: &Marks<'_>) -> Vec<Placed> {
    let mut seen: Vec<(Mark, (f32, f32))> = marks
        .all
        .iter()
        .filter_map(|m| {
            let at = lens.onto(m.at)?;
            let inside = at.0 >= 0.0 && at.0 < box_px.0 && at.1 >= 0.0 && at.1 < box_px.1;
            inside.then_some((*m, at))
        })
        .collect();
    seen.sort_by(|(a, _), (b, _)| {
        b.weight
            .cmp(&a.weight)
            .then(a.id.cmp(&b.id))
            .then(a.at.lat.total_cmp(&b.at.lat))
            .then(a.at.lon.total_cmp(&b.at.lon))
    });
    seen.truncate(MARKS_CEILING);

    let merge = marks.merge_px;
    let cell = |at: (f32, f32)| -> (i32, i32) {
        if merge > 0.0 {
            ((at.0 / merge) as i32, (at.1 / merge) as i32)
        } else {
            (0, 0)
        }
    };
    let mut groups: Vec<Growing> = Vec::new();
    let mut grid: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for (mark, at) in seen {
        let (cx, cy) = cell(at);
        let mut best: Option<(f32, usize)> = None;
        if merge > 0.0 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let Some(list) = grid.get(&(cx + dx, cy + dy)) else {
                        continue;
                    };
                    for &n in list {
                        let g = &groups[n];
                        let (ex, ey) = (g.at.0 - at.0, g.at.1 - at.1);
                        let d = crate::sqrt(ex * ex + ey * ey);
                        // THE LOWER INDEX WINS A TIE, and the index is the
                        // order groups were made in — which the sort fixed.
                        let nearer = match best {
                            None => true,
                            Some((bd, bn)) => d < bd || (d == bd && n < bn),
                        };
                        if d < merge && nearer {
                            best = Some((d, n));
                        }
                    }
                }
            }
        }
        match best {
            Some((_, n)) => {
                let g = &mut groups[n];
                g.weight = g.weight.saturating_add(mark.weight);
                g.members.push(mark.id);
            }
            None => {
                grid.entry((cx, cy)).or_default().push(groups.len());
                groups.push(Growing {
                    at,
                    weight: mark.weight,
                    members: alloc::vec![mark.id],
                    centre: mark.at,
                });
            }
        }
    }

    let mut out: Vec<Placed> = groups
        .into_iter()
        .map(|g| Placed {
            at: g.at,
            radius: marks.radii[tier(g.weight)],
            weight: g.weight,
            members: g.members,
            centre: g.centre,
        })
        .collect();
    if let Some(id) = marks.lit
        && let Some(n) = out.iter().position(|p| p.members.contains(&id))
    {
        let lit = out.remove(n);
        out.push(lit);
    }
    out
}

/// A pose that shows every point of `points` inside a box of `box_px`, with
/// `air_px` to spare on every side. Pure.
///
/// Centred on their middle on the sphere (the mean of their directions,
/// which does not care about the date line), and as near as the farthest
/// point allows — within [`Pose::WORLD`] and [`Pose::LOCAL`]. One point is
/// [`Pose::on`] it. Points spread over more than the visible half cannot all
/// be shown, and are given the whole world around their middle; no points is
/// the whole world.
#[must_use]
pub fn framing(points: &[Point], box_px: (f32, f32), air_px: f32) -> Pose {
    use core::f32::consts::PI;
    let world = |centre: Point| Pose {
        centre,
        zoom: Pose::WORLD,
    };
    let Some(first) = points.first() else {
        return world(Point { lat: 0.0, lon: 0.0 });
    };
    if points.len() == 1 {
        return Pose::on(*first);
    }
    let (mut x, mut y, mut z) = (0.0_f32, 0.0_f32, 0.0_f32);
    for p in points {
        let (la, lo) = (p.lat * PI / 180.0, p.lon * PI / 180.0);
        x += crate::cos(la) * crate::cos(lo);
        y += crate::cos(la) * crate::sin(lo);
        z += crate::sin(la);
    }
    let across = crate::sqrt(x * x + y * y);
    let centre = if crate::sqrt(across * across + z * z) < 1e-4 {
        *first
    } else {
        Point {
            lat: (crate::atan2(z, across) * 180.0 / PI).clamp(-85.0, 85.0),
            lon: wrap(crate::atan2(y, x) * 180.0 / PI),
        }
    };

    let lens = Lens::of(
        Pose {
            centre,
            zoom: Pose::WORLD,
        },
        box_px,
    );
    let half = (box_px.0 * 0.5 - air_px, box_px.1 * 0.5 - air_px);
    if half.0 <= 0.0 || half.1 <= 0.0 || lens.radius <= 0.0 {
        return world(centre);
    }
    let mut zoom = Pose::LOCAL;
    for p in points {
        let v = lens.space(p.lat, p.lon);
        // ROUND THE BACK, or on the rim: no zoom brings it to this side.
        if v.2 <= 0.0 {
            return world(centre);
        }
        if v.0 != 0.0 {
            zoom = zoom.min(half.0 / (lens.radius * v.0.abs()));
        }
        if v.1 != 0.0 {
            zoom = zoom.min(half.1 / (lens.radius * v.1.abs()));
        }
    }
    Pose {
        centre,
        zoom: zoom.clamp(Pose::WORLD, Pose::LOCAL),
    }
}
