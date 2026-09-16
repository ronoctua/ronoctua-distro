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

use alloc::vec::Vec;

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
    Halo,
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

    let dot = at.and_then(|p| {
        let v = lens.space(p.lat, p.lon);
        (v.2 > 0.0).then(|| lens.flat(v))
    });
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
    }
}
