//! From a boundary representation's faces to triangles.
//!
//! This is the half of the STEP reader that fills the piece in. It takes the
//! entity graph [`super::step`] parsed, walks every `ADVANCED_FACE` to the
//! surface it lies on and the loops of curve that trim it, puts those loops
//! into the surface's own parameters, and hands the flat problem to
//! [`super::patch`].
//!
//! # Why a face that will not come out is drawn as an outline
//!
//! Because the alternatives are worse in both directions. Filling it anyway
//! means guessing at geometry, and a part with an invented face looks exactly
//! like a part. Skipping it means a hole in the solid with nothing saying it
//! is not the file's own hole. An outline is neither: it is visibly a
//! different kind of drawing, it is made only of curves that ARE in the file,
//! and the panel says how many faces came out that way.
//!
//! It is also what makes this module safe to have written at all. Every step
//! below — a surface this reader does not know, an inverse that will not
//! converge, a loop that comes out self-crossing, a triangulation that fails —
//! has the same answer, and that answer is a drawing that already worked
//! before any of this existed.
//!
//! # Why every edge is sampled once and shared
//!
//! Two faces that meet share the edge between them, and if each sampled it for
//! itself the two would disagree in the last bits and the solid would come
//! apart along every seam. The cache is keyed by the `EDGE_CURVE`'s own entity
//! number, so the second face gets the FIRST face's points, reversed.

use std::collections::HashMap;

use super::patch::{self, Patch};
use super::step::{Arg, Model, frame, point, sample, vertex};
use super::surface::{Curve, Frame, Spline, Surface, cross, unit};
use super::{Mesh, Wires};

/// How far a chord may sag away from the surface it stands in for, as a
/// fraction of the whole part.
///
/// # Why a fraction and not a length
///
/// Because what "fine enough" means is a PIXEL, and a pixel is the part
/// divided by the panel. The panel is at most 512 across (`crate::STEPS`), so
/// half a pixel on a part that fills it is about a thousandth of its size —
/// and that is the same answer for a five-millimetre bracket and a
/// one-metre buggy.
///
/// The first version was an absolute twenty-five microns, and on the owner's
/// 220 mm part that is seventeen times finer than the screen can show: it cost
/// 174 thousand triangles and 39 milliseconds a frame, for detail nobody can
/// see. As a fraction the same part comes to a tenth of that.
const SAG_OF_THE_PART: f64 = 1.0 / 1024.0;

/// The smallest sag ever asked for, in millimetres.
///
/// A part whose own size is unknown, or a degenerate one, must not ask for an
/// infinitely fine mesh.
const SAG_FLOOR: f64 = 0.005;

/// How many triangles a whole part may come to.
///
/// Not a memory limit — the mesh reader's own ceiling is fifty times this. It
/// is a FRAME limit: measured, the rasteriser does 225 thousand triangles in
/// 44 milliseconds, and a drag has to stay under a person's patience. A part
/// with two and a half thousand faces gets a hundred and twenty triangles
/// each, which at 512 pixels is more than the screen can tell apart anyway.
const PART_CEILING: usize = 300_000;

/// How many faces are walked at all.
///
/// The largest real file this was pointed at has 2 536. Ten times that is a
/// guard against a generated file, not against a part.
const FACE_LIMIT: usize = 25_000;

/// What came of walking the solid.
pub struct Built {
    /// The faces that filled.
    pub mesh: Mesh,
    /// The outlines of the faces that did not.
    pub wires: Wires,
    /// How many faces those were.
    pub outlined: usize,
    /// How many were walked in all.
    pub faces: usize,
}

/// Every face of every solid, filled where it can be.
#[must_use]
pub fn build(model: &Model, scale: f64) -> Built {
    let mut out = Built {
        mesh: Mesh::empty(),
        wires: Wires::empty(),
        outlined: 0,
        faces: 0,
    };
    let mut cache: HashMap<u64, Vec<[f64; 3]>> = HashMap::new();
    let mut ids: Vec<&u64> = model.at.keys().collect();
    ids.sort_unstable();
    let sag = (reach(model) * SAG_OF_THE_PART).max(SAG_FLOOR);
    // THE SHARE OF THE WHOLE each face may spend. A part with three faces can
    // afford a hundred thousand triangles apiece; one with two and a half
    // thousand cannot, and the same arithmetic says so for both.
    let faces = model
        .at
        .values()
        .filter(|e| e.is("ADVANCED_FACE") || e.is("FACE_SURFACE"))
        .count()
        .clamp(1, FACE_LIMIT);
    let allowance = (PART_CEILING / faces).max(8);

    for id in ids {
        let Some(entity) = model.at.get(id) else {
            continue;
        };
        let Some(args) = entity
            .part("ADVANCED_FACE")
            .or_else(|| entity.part("FACE_SURFACE"))
        else {
            continue;
        };
        if out.faces >= FACE_LIMIT {
            break;
        }
        out.faces += 1;
        // `ADVANCED_FACE(name, (bounds), surface, same_sense)`.
        let (Some(bounds), Some(on)) = (args.get(1).and_then(Arg::list), args.get(2)) else {
            continue;
        };
        let loops: Vec<Vec<[f64; 3]>> = bounds
            .iter()
            .filter_map(|b| face_bound(model, &mut cache, b))
            .collect();
        if loops.is_empty() {
            // A face whose bounds this reader could not read is a face that
            // did not fill, and the panel is told so.
            out.outlined += 1;
            continue;
        }
        match fill(model, on, &loops, scale, sag, allowance) {
            Some(mesh) => {
                for i in 0..mesh.triangles() {
                    let c = mesh.corners(i);
                    out.mesh.add(c[0], c[1], c[2]);
                }
            }
            None => {
                out.outlined += 1;
                for ring in &loops {
                    let drawn: Vec<[f32; 3]> = ring.iter().map(|p| shrink(*p, scale)).collect();
                    out.wires.along(&drawn);
                    // A loop is closed, and `along` only joins what it is
                    // given — so the last point back to the first is a segment
                    // nobody would otherwise draw.
                    if let (Some(a), Some(b)) = (drawn.last(), drawn.first()) {
                        out.wires.add(*a, *b);
                    }
                }
            }
        }
    }
    out
}

/// How far across the whole part is.
///
/// From its vertices, which is one pass over the graph and is what the sag
/// tolerance is a fraction of.
fn reach(model: &Model) -> f64 {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for e in model.at.values() {
        let Some(a) = e.part("CARTESIAN_POINT") else {
            continue;
        };
        let Some(v) = a.get(1).and_then(Arg::list) else {
            continue;
        };
        for k in 0..3 {
            let Some(n) = v.get(k).and_then(Arg::number) else {
                continue;
            };
            lo[k] = lo[k].min(n);
            hi[k] = hi[k].max(n);
        }
    }
    let mut widest = 0.0_f64;
    for k in 0..3 {
        if hi[k] >= lo[k] {
            widest = widest.max(hi[k] - lo[k]);
        }
    }
    widest
}

/// One face, filled — or `None`, which the caller draws as an outline.
fn fill(
    model: &Model,
    on: &Arg,
    loops: &[Vec<[f64; 3]>],
    scale: f64,
    sag: f64,
    allowance: usize,
) -> Option<Mesh> {
    let surface = surface_of(model, on)?;
    let (wu, wv) = surface.wraps();
    let ((u0, u1), (v0, v1)) = surface.domain();

    // EVERY LOOP INTO THE SURFACE'S OWN PARAMETERS, and the first is the one
    // that goes round the outside.
    let mut rings: Vec<Vec<[f64; 2]>> = Vec::with_capacity(loops.len());
    for ring in loops {
        rings.push(to_uv(&surface, ring, wu, wv)?);
    }
    // WHICH RING IS THE OUTER ONE, by area and not by the order the file
    // listed them. `FACE_OUTER_BOUND` says so and most writers use it, but a
    // `FACE_BOUND` on its own is legal and then nothing does.
    let biggest = rings
        .iter()
        .enumerate()
        .max_by(|a, b| span_of(a.1).total_cmp(&span_of(b.1)))
        .map(|(i, _)| i)?;
    let outer = rings.swap_remove(biggest);

    // HOW FINELY, and the answer comes from the TRIANGLES and not from the
    // face.
    //
    // The first version asked how much parameter the whole face covered, and
    // it was wrong by the width of the boundary: a cylindrical face going the
    // whole way round spans a full turn, but its rim arrives already sampled
    // at under six degrees a step, so ear clipping produces triangles that are
    // six degrees wide and not three hundred and sixty. Measured, that put
    // every cylinder at the subdivision ceiling and the owner's part at 359
    // thousand triangles for 63 faces.
    //
    // So the domain is cut flat first — which is cheap, and is work the real
    // cut would do anyway — and the depth comes from the widest triangle in
    // it.
    let patch = Patch {
        outer,
        holes: rings,
    };
    let flat = patch::tessellate(&patch, 0)?;
    let (bu, bv) = surface.bend();
    let (mut wu, mut wv) = (0.0_f64, 0.0_f64);
    for t in &flat.triangles {
        for k in 0..3 {
            let (a, b) = (flat.points[t[k]], flat.points[t[(k + 1) % 3]]);
            wu = wu.max((a[0] - b[0]).abs());
            wv = wv.max((a[1] - b[1]).abs());
        }
    }
    // AND NOT DEEPER THAN THE FACE'S SHARE. Each level is four times the
    // triangles, so the cap is a logarithm of what is left to spend.
    let mut roof = 0;
    let mut would = flat.triangles.len().max(1);
    while would * 4 <= allowance && roof < patch::DEPTH_CEILING {
        would *= 4;
        roof += 1;
    }
    let depth = if bu.is_nan() {
        // A swept or splined surface does not say how it bends. The fraction
        // of its own domain one triangle covers is the honest stand-in.
        let fraction = (wu / (u1 - u0)).max(wv / (v1 - v0));
        if fraction.is_finite() && fraction > 0.02 {
            patch::DEPTH_CEILING.min(2 + u32::from(fraction > 0.25))
        } else {
            1
        }
    } else {
        patch::depth_for(bu, wu, sag).max(patch::depth_for(bv, wv, sag))
    }
    .min(roof);

    let cut = if depth == 0 {
        flat
    } else {
        patch::tessellate(&patch, depth)?
    };
    let mut mesh = Mesh::empty();
    let back: Vec<[f32; 3]> = cut
        .points
        .iter()
        .map(|uv| shrink(surface.at(uv[0], uv[1]), scale))
        .collect();
    for t in &cut.triangles {
        mesh.add(back[t[0]], back[t[1]], back[t[2]]);
    }
    (mesh.triangles() > 0).then_some(mesh)
}

/// How far a ring reaches along one parameter.
fn extent(ring: &[[f64; 2]], axis: usize) -> f64 {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for p in ring {
        lo = lo.min(p[axis]);
        hi = hi.max(p[axis]);
    }
    if hi < lo { 0.0 } else { hi - lo }
}

/// A ring's reach, for deciding which is the outer one.
fn span_of(ring: &[[f64; 2]]) -> f64 {
    extent(ring, 0) * extent(ring, 1)
}

/// A boundary polyline in the surface's own parameters.
///
/// # Why the seam is walked and not detected
///
/// A cylindrical face that goes the whole way round has an edge at `u = 0`
/// which is also `u = 2π`, and the inverse has no way to know which it is
/// looking at — `atan2` answers in one turn and nothing else. So the loop is
/// WALKED: every step that would jump more than half a turn is taken as having
/// gone round rather than back, and the parameter accumulates. A face that
/// covers the full circle then comes out as a rectangle from `0` to `2π`
/// instead of two pieces at either end of one.
fn to_uv(
    surface: &Surface,
    ring: &[[f64; 3]],
    wraps_u: bool,
    wraps_v: bool,
) -> Option<Vec<[f64; 2]>> {
    let turn = std::f64::consts::TAU;
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(ring.len());
    let mut seed: Option<(f64, f64)> = None;
    for p in ring {
        let (mut u, mut v) = surface.invert(*p, seed)?;
        if let Some(last) = out.last() {
            if wraps_u {
                u = unwrap(last[0], u, turn);
            }
            if wraps_v {
                v = unwrap(last[1], v, turn);
            }
        }
        // THE SANITY GATE, and it asks only about the parameters that are
        // ANGLES.
        //
        // Those are the ones that wrap, and after unwrapping a step of more
        // than half a turn means the inverse landed on another branch of the
        // surface — a polygon built from that crosses itself and triangulates
        // into something that is not the face.
        //
        // The first version asked it of every parameter, and a cylinder's `v`
        // is a HEIGHT: comparing millimetres against pi rejected every
        // cylindrical face taller than 3.14 mm. Measured, that was 126 of the
        // faces in the largest file here.
        //
        // A non-angle needs no gate of its own: `Surface::invert` has already
        // put the answer back through the forward map and made it land where
        // it started, which is a stronger check than any distance.
        if let Some(last) = out.last()
            && ((wraps_u && (u - last[0]).abs() > turn * 0.5)
                || (wraps_v && (v - last[1]).abs() > turn * 0.5))
        {
            return None;
        }
        seed = Some((u, v));
        out.push([u, v]);
    }
    // TWO ENDS IN THE SAME PLACE ARE ONE POINT. A closed loop's last sample is
    // its first, and ear clipping would see a zero-area spur.
    while out.len() > 3 {
        let (a, b) = (out[0], out[out.len() - 1]);
        if (a[0] - b[0]).abs() < 1e-12 && (a[1] - b[1]).abs() < 1e-12 {
            out.pop();
        } else {
            break;
        }
    }
    (out.len() >= 3).then_some(out)
}

/// `now`, moved by whole turns to be the nearest to `last`.
fn unwrap(last: f64, now: f64, turn: f64) -> f64 {
    let mut moved = now;
    while moved - last > turn * 0.5 {
        moved -= turn;
    }
    while last - moved > turn * 0.5 {
        moved += turn;
    }
    moved
}

/// One `FACE_OUTER_BOUND` or `FACE_BOUND`, as a closed polyline.
fn face_bound(
    model: &Model,
    cache: &mut HashMap<u64, Vec<[f64; 3]>>,
    r: &Arg,
) -> Option<Vec<[f64; 3]>> {
    let e = model.get(r)?;
    let args = e
        .part("FACE_OUTER_BOUND")
        .or_else(|| e.part("FACE_BOUND"))?;
    // `FACE_BOUND(name, loop, orientation)` — the NAME is argument zero, and
    // reading the loop out of it is a face that silently has no boundary.
    let mut ring = edge_loop(model, cache, args.get(1)?)?;
    if matches!(args.get(2), Some(Arg::Enum(s)) if s == ".F.") {
        ring.reverse();
    }
    Some(ring)
}

/// An `EDGE_LOOP`, walked into one polyline.
fn edge_loop(
    model: &Model,
    cache: &mut HashMap<u64, Vec<[f64; 3]>>,
    r: &Arg,
) -> Option<Vec<[f64; 3]>> {
    let args = model.get(r)?.part("EDGE_LOOP")?;
    let mut out: Vec<[f64; 3]> = Vec::new();
    for oriented in args.get(1)?.list()? {
        let e = model.get(oriented)?;
        // `ORIENTED_EDGE(name, *, *, edge_element, orientation)`.
        let inner = e.part("ORIENTED_EDGE")?;
        let forward = !matches!(inner.get(4), Some(Arg::Enum(s)) if s == ".F.");
        let mut points = edge_points(model, cache, inner.get(3)?)?;
        if !forward {
            points.reverse();
        }
        // The end of one edge is the start of the next, written twice.
        if out.is_empty() {
            out = points;
        } else {
            out.extend(points.into_iter().skip(1));
        }
    }
    (out.len() >= 3).then_some(out)
}

/// One `EDGE_CURVE`, sampled once and remembered.
fn edge_points(
    model: &Model,
    cache: &mut HashMap<u64, Vec<[f64; 3]>>,
    r: &Arg,
) -> Option<Vec<[f64; 3]>> {
    let id = r.reference()?;
    if let Some(had) = cache.get(&id) {
        return Some(had.clone());
    }
    let args = model.get(r)?.part("EDGE_CURVE")?;
    let a = vertex(model, args.get(1)?)?;
    let b = vertex(model, args.get(2)?)?;
    let same_sense = !matches!(args.get(4), Some(Arg::Enum(s)) if s == ".F.");
    let points = sample(model, args.get(3)?, a, b, same_sense, 0);
    cache.insert(id, points.clone());
    Some(points)
}

/// The surface an entity describes.
pub(super) fn surface_of(model: &Model, r: &Arg) -> Option<Surface> {
    let e = model.get(r)?;
    let named = |n: &str| e.part(n);
    if let Some(a) = named("PLANE") {
        return Some(Surface::Plane(frame(model, a.get(1)?)?));
    }
    if let Some(a) = named("CYLINDRICAL_SURFACE") {
        return Some(Surface::Cylinder(
            frame(model, a.get(1)?)?,
            a.get(2)?.number()?,
        ));
    }
    if let Some(a) = named("CONICAL_SURFACE") {
        return Some(Surface::Cone(
            frame(model, a.get(1)?)?,
            a.get(2)?.number()?,
            a.get(3)?.number()?,
        ));
    }
    if let Some(a) = named("SPHERICAL_SURFACE") {
        return Some(Surface::Sphere(
            frame(model, a.get(1)?)?,
            a.get(2)?.number()?,
        ));
    }
    if let Some(a) = named("TOROIDAL_SURFACE") {
        return Some(Surface::Torus(
            frame(model, a.get(1)?)?,
            a.get(2)?.number()?,
            a.get(3)?.number()?,
        ));
    }
    if let Some(a) = named("SURFACE_OF_LINEAR_EXTRUSION") {
        let curve = curve_of(model, a.get(1)?)?;
        // `extrusion_axis` is a VECTOR: a direction and how long it is.
        let v = model.get(a.get(2)?)?.part("VECTOR")?;
        let d = point(model, v.get(1)?)?;
        let length = v.get(2).and_then(Arg::number).unwrap_or(1.0);
        return Some(Surface::Extrusion(
            Box::new(curve),
            [d[0] * length, d[1] * length, d[2] * length],
        ));
    }
    if let Some(a) = named("SURFACE_OF_REVOLUTION") {
        return Some(Surface::Revolution(
            Box::new(curve_of(model, a.get(1)?)?),
            axis1(model, a.get(2)?)?,
        ));
    }
    // A B-spline surface, in either of its two spellings.
    let s = spline_of(model, e, true)?;
    Some(Surface::Spline(s))
}

/// An `AXIS1_PLACEMENT`: a point and ONE direction.
///
/// The other two axes are this reader's own, because the entity does not carry
/// them — anything perpendicular will do, since the surface is a revolution
/// about the one that is given and where its zero angle sits does not change
/// the shape.
fn axis1(model: &Model, r: &Arg) -> Option<Frame> {
    let a = model.get(r)?.part("AXIS1_PLACEMENT")?;
    let origin = point(model, a.get(1)?)?;
    let z = unit(
        a.get(2)
            .and_then(|d| point(model, d))
            .unwrap_or([0.0, 0.0, 1.0]),
    )?;
    let aside = if z[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let x = unit(cross(aside, z))?;
    Some(Frame {
        origin,
        x,
        y: cross(z, x),
        z,
    })
}

/// The curve an entity describes, for the two swept surfaces.
fn curve_of(model: &Model, r: &Arg) -> Option<Curve> {
    curve_within(model, r, 0)
}

/// As [`curve_of`], counting how many wrappers deep it has gone.
fn curve_within(model: &Model, r: &Arg, depth: u32) -> Option<Curve> {
    let e = model.get(r)?;
    if depth > 8 {
        return None;
    }
    // A TRIMMED_CURVE is a curve with two ends put on it, and what is swept is
    // the curve underneath. Every `SURFACE_OF_REVOLUTION` in the largest file
    // here is written that way, and not unwrapping it lost all sixty-five.
    if let Some(a) = e
        .part("TRIMMED_CURVE")
        .or_else(|| e.part("SURFACE_CURVE"))
        .or_else(|| e.part("SEAM_CURVE"))
    {
        return curve_within(model, a.get(1)?, depth + 1);
    }
    if let Some(a) = e.part("LINE") {
        let origin = point(model, a.get(1)?)?;
        let v = model.get(a.get(2)?)?.part("VECTOR")?;
        let d = point(model, v.get(1)?)?;
        let length = v.get(2).and_then(Arg::number).unwrap_or(1.0);
        return Some(Curve::Line(
            origin,
            [d[0] * length, d[1] * length, d[2] * length],
        ));
    }
    if let Some(a) = e.part("CIRCLE") {
        return Some(Curve::Circle(frame(model, a.get(1)?)?, a.get(2)?.number()?));
    }
    if let Some(a) = e.part("ELLIPSE") {
        return Some(Curve::Ellipse(
            frame(model, a.get(1)?)?,
            a.get(2)?.number()?,
            a.get(3)?.number()?,
        ));
    }
    spline_of(model, e, false).map(Curve::Spline)
}

/// A B-spline, curve or surface, in either spelling.
///
/// # The two spellings, and why one reader has to know both
///
/// A plain one is a single entity that carries everything. A RATIONAL one is a
/// COMPLEX entity — several named parts at one number, which is how the format
/// writes multiple inheritance — and the pieces are split across them: the
/// degrees and the control points in `B_SPLINE_SURFACE`, the knots in
/// `B_SPLINE_SURFACE_WITH_KNOTS`, the weights in `RATIONAL_B_SPLINE_SURFACE`.
/// The two also number their arguments differently, because only the simple
/// form carries a name.
///
/// Both are in the wild in the same file: the moon buggy has 47 rational
/// surfaces and 30 plain ones. A reader that knew one spelling would read the
/// other's degree out of its control points.
pub(super) fn spline_of(model: &Model, e: &super::step::Entity, two_d: bool) -> Option<Spline> {
    let (plain, knotted) = if two_d {
        ("B_SPLINE_SURFACE", "B_SPLINE_SURFACE_WITH_KNOTS")
    } else {
        ("B_SPLINE_CURVE", "B_SPLINE_CURVE_WITH_KNOTS")
    };
    let complex = e.parts.len() > 1;
    let (degree, grid, mults, knots, weights) = if complex {
        let a = e.part(plain)?;
        let k = e.part(knotted)?;
        let w = e
            .part(if two_d {
                "RATIONAL_B_SPLINE_SURFACE"
            } else {
                "RATIONAL_B_SPLINE_CURVE"
            })
            .and_then(|r| r.first());
        if two_d {
            (
                (a.first()?.number()?, a.get(1)?.number()?),
                a.get(2)?,
                (k.first()?, k.get(1)),
                (k.get(2)?, k.get(3)),
                w,
            )
        } else {
            (
                (a.first()?.number()?, 0.0),
                a.get(1)?,
                (k.first()?, None),
                (k.get(1)?, None),
                w,
            )
        }
    } else {
        let a = e.part(knotted)?;
        if two_d {
            (
                (a.get(1)?.number()?, a.get(2)?.number()?),
                a.get(3)?,
                (a.get(8)?, a.get(9)),
                (a.get(10)?, a.get(11)),
                None,
            )
        } else {
            (
                (a.get(1)?.number()?, 0.0),
                a.get(2)?,
                (a.get(6)?, None),
                (a.get(7)?, None),
                None,
            )
        }
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let degree = (degree.0 as usize, degree.1 as usize);
    // The control points, as a grid. A curve is one column of it.
    let rows = grid.list()?;
    let (points, across) = if two_d {
        let mut flat = Vec::new();
        let mut across = 0;
        for row in rows {
            let cells = row.list()?;
            across = cells.len();
            for c in cells {
                flat.push(point(model, c)?);
            }
        }
        (flat, across)
    } else {
        let mut flat = Vec::new();
        for c in rows {
            flat.push(point(model, c)?);
        }
        (flat, 1)
    };
    let weights = match weights {
        None => Vec::new(),
        Some(w) => {
            let mut flat = Vec::new();
            if two_d {
                for row in w.list()? {
                    for c in row.list()? {
                        flat.push(c.number()?);
                    }
                }
            } else {
                for c in w.list()? {
                    flat.push(c.number()?);
                }
            }
            flat
        }
    };
    let spline = Spline {
        degree,
        points,
        across,
        weights,
        knots: (
            expand(mults.0, knots.0)?,
            match (mults.1, knots.1) {
                (Some(m), Some(k)) => expand(m, k)?,
                // A curve has no second parameter; one knot pair keeps
                // `Spline::sound` arithmetic honest for a single column.
                _ => vec![0.0, 1.0],
            },
        ),
    };
    spline.sound().then_some(spline)
}

/// A knot vector, written out from its multiplicities.
fn expand(mults: &Arg, knots: &Arg) -> Option<Vec<f64>> {
    let (mults, knots) = (mults.list()?, knots.list()?);
    let mut out = Vec::with_capacity(mults.len() * 2);
    for (m, k) in mults.iter().zip(knots) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let times = (m.number()? as usize).min(64);
        let value = k.number()?;
        for _ in 0..times {
            out.push(value);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// A point at the precision the mesh keeps, scaled to millimetres.
#[allow(clippy::cast_possible_truncation)]
fn shrink(p: [f64; 3], scale: f64) -> [f32; 3] {
    [
        (p[0] * scale) as f32,
        (p[1] * scale) as f32,
        (p[2] * scale) as f32,
    ]
}

#[cfg(test)]
mod tests {
    use super::{to_uv, unwrap};
    use crate::mesh::Shape;
    use crate::mesh::surface::{Frame, Surface};

    fn frame() -> Frame {
        Frame {
            origin: [0.0, 0.0, 0.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn a_value_is_moved_by_whole_turns_to_the_nearest() {
        let turn = std::f64::consts::TAU;
        assert!((unwrap(0.1, turn - 0.1, turn) - -0.1).abs() < 1e-12);
        assert!((unwrap(turn - 0.1, 0.1, turn) - (turn + 0.1)).abs() < 1e-12);
        assert!((unwrap(1.0, 1.2, turn) - 1.2).abs() < 1e-12);
    }

    #[test]
    // THE DEFECT THIS CATCHES: a face that goes the whole way round coming out
    // as two pieces at either end of the parameter. `atan2` answers in one
    // turn and cannot say which side of the seam it is on, so the loop is
    // walked and the parameter accumulates.
    fn a_loop_all_the_way_round_a_cylinder_unwraps_to_a_full_turn() {
        let s = Surface::Cylinder(frame(), 10.0);
        let turn = std::f64::consts::TAU;
        // A circle at height zero, then back along the seam and round again.
        let ring: Vec<[f64; 3]> = (0..=36)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let a = turn * (i as f64) / 36.0;
                [10.0 * a.cos(), 10.0 * a.sin(), 0.0]
            })
            .collect();
        let uv = to_uv(&s, &ring, true, false).expect("a circle inverts");
        let lo = uv.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        let hi = uv.iter().map(|p| p[0]).fold(f64::MIN, f64::max);
        assert!(
            (hi - lo - turn).abs() < 1e-6,
            "the seam did not unwrap: u ran from {lo} to {hi}"
        );
    }

    #[test]
    fn a_loop_that_does_not_cross_the_seam_is_left_alone() {
        let s = Surface::Cylinder(frame(), 10.0);
        let ring: Vec<[f64; 3]> = (0..=8)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let a = 0.1 + 0.1 * (i as f64);
                [10.0 * a.cos(), 10.0 * a.sin(), 0.0]
            })
            .collect();
        let uv = to_uv(&s, &ring, true, false).unwrap();
        for p in &uv {
            assert!(p[0] > 0.0 && p[0] < 1.0, "u wandered to {}", p[0]);
        }
    }

    #[test]
    // The last sample of a closed loop is its first, and ear clipping would
    // see a spur of no area there.
    fn a_loop_that_ends_where_it_began_loses_the_repeat() {
        let s = Surface::Plane(frame());
        let ring = vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        let uv = to_uv(&s, &ring, false, false).unwrap();
        assert_eq!(uv.len(), 3, "the repeated point survived: {uv:?}");
    }

    #[test]
    fn a_point_that_is_not_on_the_surface_gives_no_loop_at_all() {
        let s = Surface::Sphere(frame(), 5.0);
        let ring = vec![
            [5.0, 0.0, 0.0],
            [0.0, 5.0, 0.0],
            // Nowhere near a sphere of radius five.
            [900.0, 900.0, 900.0],
        ];
        assert!(to_uv(&s, &ring, true, false).is_none());
    }

    /// A ten-millimetre cube, written out as a part-21 file.
    ///
    /// # Why a fixture and not one of the owner's files
    ///
    /// Because a cube's volume is a thousand and nobody has to measure it. A
    /// test against a real part can only compare this reader against itself —
    /// it would pass just as happily on a mesher that got every face subtly
    /// wrong, as long as it went on getting them wrong.
    const CUBE: &str = r"ISO-10303-21;
HEADER;
FILE_DESCRIPTION (( 'a cube' ), '1');
FILE_NAME ('CUBE.STEP','2026-08-31T00:00:00',( '' ),( '' ),'a test','a test','');
FILE_SCHEMA (( 'AUTOMOTIVE_DESIGN' ));
ENDSEC;
DATA;
#1 = CARTESIAN_POINT ( 'NONE', ( 0.0, 0.0, 0.0 ) );
#2 = CARTESIAN_POINT ( 'NONE', ( 10.0, 0.0, 0.0 ) );
#3 = CARTESIAN_POINT ( 'NONE', ( 10.0, 10.0, 0.0 ) );
#4 = CARTESIAN_POINT ( 'NONE', ( 0.0, 10.0, 0.0 ) );
#5 = CARTESIAN_POINT ( 'NONE', ( 0.0, 0.0, 10.0 ) );
#6 = CARTESIAN_POINT ( 'NONE', ( 10.0, 0.0, 10.0 ) );
#7 = CARTESIAN_POINT ( 'NONE', ( 10.0, 10.0, 10.0 ) );
#8 = CARTESIAN_POINT ( 'NONE', ( 0.0, 10.0, 10.0 ) );
#9 = VERTEX_POINT ( 'NONE', #1 );
#10 = VERTEX_POINT ( 'NONE', #2 );
#11 = VERTEX_POINT ( 'NONE', #3 );
#12 = VERTEX_POINT ( 'NONE', #4 );
#13 = VERTEX_POINT ( 'NONE', #5 );
#14 = VERTEX_POINT ( 'NONE', #6 );
#15 = VERTEX_POINT ( 'NONE', #7 );
#16 = VERTEX_POINT ( 'NONE', #8 );
#17 = DIRECTION ( 'NONE', ( 0.0, 1.0, 0.0 ) );
#18 = VECTOR ( 'NONE', #17, 10.0 );
#19 = LINE ( 'NONE', #1, #18 );
#20 = EDGE_CURVE ( 'NONE', #9, #12, #19, .T. );
#21 = ORIENTED_EDGE ( 'NONE', *, *, #20, .T. );
#22 = DIRECTION ( 'NONE', ( -1.0, 0.0, 0.0 ) );
#23 = VECTOR ( 'NONE', #22, 10.0 );
#24 = LINE ( 'NONE', #3, #23 );
#25 = EDGE_CURVE ( 'NONE', #11, #12, #24, .T. );
#26 = ORIENTED_EDGE ( 'NONE', *, *, #25, .F. );
#27 = DIRECTION ( 'NONE', ( 0.0, 1.0, 0.0 ) );
#28 = VECTOR ( 'NONE', #27, 10.0 );
#29 = LINE ( 'NONE', #2, #28 );
#30 = EDGE_CURVE ( 'NONE', #10, #11, #29, .T. );
#31 = ORIENTED_EDGE ( 'NONE', *, *, #30, .F. );
#32 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#33 = VECTOR ( 'NONE', #32, 10.0 );
#34 = LINE ( 'NONE', #1, #33 );
#35 = EDGE_CURVE ( 'NONE', #9, #10, #34, .T. );
#36 = ORIENTED_EDGE ( 'NONE', *, *, #35, .F. );
#37 = EDGE_LOOP ( 'NONE', ( #21, #26, #31, #36 ) );
#38 = FACE_OUTER_BOUND ( 'NONE', #37, .T. );
#39 = DIRECTION ( 'NONE', ( 0.0, 0.0, -1.0 ) );
#40 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#41 = CARTESIAN_POINT ( 'NONE', ( 0.0, 0.0, 0.0 ) );
#42 = AXIS2_PLACEMENT_3D ( 'NONE', #41, #39, #40 );
#43 = PLANE ( 'NONE', #42 );
#44 = ADVANCED_FACE ( 'NONE', ( #38 ), #43, .T. );
#45 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#46 = VECTOR ( 'NONE', #45, 10.0 );
#47 = LINE ( 'NONE', #5, #46 );
#48 = EDGE_CURVE ( 'NONE', #13, #14, #47, .T. );
#49 = ORIENTED_EDGE ( 'NONE', *, *, #48, .T. );
#50 = DIRECTION ( 'NONE', ( 0.0, 1.0, 0.0 ) );
#51 = VECTOR ( 'NONE', #50, 10.0 );
#52 = LINE ( 'NONE', #6, #51 );
#53 = EDGE_CURVE ( 'NONE', #14, #15, #52, .T. );
#54 = ORIENTED_EDGE ( 'NONE', *, *, #53, .T. );
#55 = DIRECTION ( 'NONE', ( -1.0, 0.0, 0.0 ) );
#56 = VECTOR ( 'NONE', #55, 10.0 );
#57 = LINE ( 'NONE', #7, #56 );
#58 = EDGE_CURVE ( 'NONE', #15, #16, #57, .T. );
#59 = ORIENTED_EDGE ( 'NONE', *, *, #58, .T. );
#60 = DIRECTION ( 'NONE', ( 0.0, 1.0, 0.0 ) );
#61 = VECTOR ( 'NONE', #60, 10.0 );
#62 = LINE ( 'NONE', #5, #61 );
#63 = EDGE_CURVE ( 'NONE', #13, #16, #62, .T. );
#64 = ORIENTED_EDGE ( 'NONE', *, *, #63, .F. );
#65 = EDGE_LOOP ( 'NONE', ( #49, #54, #59, #64 ) );
#66 = FACE_OUTER_BOUND ( 'NONE', #65, .T. );
#67 = DIRECTION ( 'NONE', ( 0.0, 0.0, 1.0 ) );
#68 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#69 = CARTESIAN_POINT ( 'NONE', ( 0.0, 0.0, 10.0 ) );
#70 = AXIS2_PLACEMENT_3D ( 'NONE', #69, #67, #68 );
#71 = PLANE ( 'NONE', #70 );
#72 = ADVANCED_FACE ( 'NONE', ( #66 ), #71, .T. );
#73 = ORIENTED_EDGE ( 'NONE', *, *, #35, .T. );
#74 = DIRECTION ( 'NONE', ( 0.0, 0.0, 1.0 ) );
#75 = VECTOR ( 'NONE', #74, 10.0 );
#76 = LINE ( 'NONE', #2, #75 );
#77 = EDGE_CURVE ( 'NONE', #10, #14, #76, .T. );
#78 = ORIENTED_EDGE ( 'NONE', *, *, #77, .T. );
#79 = ORIENTED_EDGE ( 'NONE', *, *, #48, .F. );
#80 = DIRECTION ( 'NONE', ( 0.0, 0.0, 1.0 ) );
#81 = VECTOR ( 'NONE', #80, 10.0 );
#82 = LINE ( 'NONE', #1, #81 );
#83 = EDGE_CURVE ( 'NONE', #9, #13, #82, .T. );
#84 = ORIENTED_EDGE ( 'NONE', *, *, #83, .F. );
#85 = EDGE_LOOP ( 'NONE', ( #73, #78, #79, #84 ) );
#86 = FACE_OUTER_BOUND ( 'NONE', #85, .T. );
#87 = DIRECTION ( 'NONE', ( 0.0, -1.0, 0.0 ) );
#88 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#89 = CARTESIAN_POINT ( 'NONE', ( 0.0, 0.0, 0.0 ) );
#90 = AXIS2_PLACEMENT_3D ( 'NONE', #89, #87, #88 );
#91 = PLANE ( 'NONE', #90 );
#92 = ADVANCED_FACE ( 'NONE', ( #86 ), #91, .T. );
#93 = ORIENTED_EDGE ( 'NONE', *, *, #30, .T. );
#94 = DIRECTION ( 'NONE', ( 0.0, 0.0, 1.0 ) );
#95 = VECTOR ( 'NONE', #94, 10.0 );
#96 = LINE ( 'NONE', #3, #95 );
#97 = EDGE_CURVE ( 'NONE', #11, #15, #96, .T. );
#98 = ORIENTED_EDGE ( 'NONE', *, *, #97, .T. );
#99 = ORIENTED_EDGE ( 'NONE', *, *, #53, .F. );
#100 = ORIENTED_EDGE ( 'NONE', *, *, #77, .F. );
#101 = EDGE_LOOP ( 'NONE', ( #93, #98, #99, #100 ) );
#102 = FACE_OUTER_BOUND ( 'NONE', #101, .T. );
#103 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#104 = DIRECTION ( 'NONE', ( 0.0, 1.0, 0.0 ) );
#105 = CARTESIAN_POINT ( 'NONE', ( 10.0, 0.0, 0.0 ) );
#106 = AXIS2_PLACEMENT_3D ( 'NONE', #105, #103, #104 );
#107 = PLANE ( 'NONE', #106 );
#108 = ADVANCED_FACE ( 'NONE', ( #102 ), #107, .T. );
#109 = ORIENTED_EDGE ( 'NONE', *, *, #25, .T. );
#110 = DIRECTION ( 'NONE', ( 0.0, 0.0, 1.0 ) );
#111 = VECTOR ( 'NONE', #110, 10.0 );
#112 = LINE ( 'NONE', #4, #111 );
#113 = EDGE_CURVE ( 'NONE', #12, #16, #112, .T. );
#114 = ORIENTED_EDGE ( 'NONE', *, *, #113, .T. );
#115 = ORIENTED_EDGE ( 'NONE', *, *, #58, .F. );
#116 = ORIENTED_EDGE ( 'NONE', *, *, #97, .F. );
#117 = EDGE_LOOP ( 'NONE', ( #109, #114, #115, #116 ) );
#118 = FACE_OUTER_BOUND ( 'NONE', #117, .T. );
#119 = DIRECTION ( 'NONE', ( 0.0, 1.0, 0.0 ) );
#120 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#121 = CARTESIAN_POINT ( 'NONE', ( 10.0, 10.0, 0.0 ) );
#122 = AXIS2_PLACEMENT_3D ( 'NONE', #121, #119, #120 );
#123 = PLANE ( 'NONE', #122 );
#124 = ADVANCED_FACE ( 'NONE', ( #118 ), #123, .T. );
#125 = ORIENTED_EDGE ( 'NONE', *, *, #20, .F. );
#126 = ORIENTED_EDGE ( 'NONE', *, *, #83, .T. );
#127 = ORIENTED_EDGE ( 'NONE', *, *, #63, .T. );
#128 = ORIENTED_EDGE ( 'NONE', *, *, #113, .F. );
#129 = EDGE_LOOP ( 'NONE', ( #125, #126, #127, #128 ) );
#130 = FACE_OUTER_BOUND ( 'NONE', #129, .T. );
#131 = DIRECTION ( 'NONE', ( -1.0, 0.0, 0.0 ) );
#132 = DIRECTION ( 'NONE', ( 0.0, 1.0, 0.0 ) );
#133 = CARTESIAN_POINT ( 'NONE', ( 0.0, 10.0, 0.0 ) );
#134 = AXIS2_PLACEMENT_3D ( 'NONE', #133, #131, #132 );
#135 = PLANE ( 'NONE', #134 );
#136 = ADVANCED_FACE ( 'NONE', ( #130 ), #135, .T. );
#137 = CLOSED_SHELL ( 'NONE', ( #44, #72, #92, #108, #124, #136 ) );
#138 = MANIFOLD_SOLID_BREP ( 'a cube', #137 );
#139 = ( LENGTH_UNIT ( ) NAMED_UNIT ( * ) SI_UNIT ( .MILLI., .METRE. ) );
ENDSEC;
END-ISO-10303-21;
";

    #[test]
    // THE DEFECT THIS CATCHES: everything, at once. If the faces are walked,
    // the loops assembled, the boundary put into the surface's parameters, the
    // domain triangulated and the result mapped back — and every one of those
    // done right — then the volume is a thousand. If any is wrong the number
    // is not, and it is not close either.
    fn a_cube_of_six_planar_faces_encloses_the_volume_a_cube_does() {
        let read = crate::mesh::step::read(CUBE.as_bytes()).expect("the cube reads");
        assert_eq!(read.shape, Shape::Step);
        assert_eq!(read.census.faces, 6);
        assert_eq!(read.census.edges, 12);
        assert_eq!(read.census.vertices, 8);
        assert_eq!(read.census.outlined, 0, "a planar face did not fill");
        let mesh = &read.body.mesh;
        assert_eq!(mesh.triangles(), 12, "six squares are twelve triangles");
        assert!(
            (mesh.volume() - 1000.0).abs() < 1e-6,
            "the cube came out at {}",
            mesh.volume()
        );
        assert!(
            (mesh.area() - 600.0).abs() < 1e-6,
            "the surface came out at {}",
            mesh.area()
        );
        assert_eq!(mesh.bounds.size(), [10.0, 10.0, 10.0]);
    }

    #[test]
    // A face made only of planes needs no subdivision at all, and a mesher
    // that subdivided anyway would pay for detail that cannot exist.
    fn a_flat_face_is_not_subdivided() {
        let read = crate::mesh::step::read(CUBE.as_bytes()).unwrap();
        assert_eq!(read.body.mesh.triangles(), 12);
        assert_eq!(
            read.body.wires.count(),
            0,
            "nothing fell back to an outline"
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: two faces meeting at an edge each sampling it
    // for themselves. The cube is closed, so every one of its edges belongs to
    // exactly two triangles — and if the two faces disagreed by a bit in the
    // last place, none of them would.
    fn the_faces_meet_exactly_where_they_share_an_edge() {
        let read = crate::mesh::step::read(CUBE.as_bytes()).unwrap();
        let sane = crate::mesh::sanity(&read.body.mesh);
        assert!(
            sane.watertight(),
            "the cube came out with {} open edges",
            sane.open
        );
    }
}
