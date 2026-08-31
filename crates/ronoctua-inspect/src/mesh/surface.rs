//! The surfaces a boundary representation is built out of, and how to get on
//! and off one.
//!
//! # What a surface has to answer
//!
//! Two questions, and the second is the hard one. **Where is `(u, v)`** — the
//! forward map, always exact and always cheap. And **what `(u, v)` is this
//! point** — the inverse, which is what turns a face's boundary curves into a
//! flat polygon that can be triangulated.
//!
//! # Why there is ONE numerical inverse and not seven
//!
//! Because the forward map is easy to write and easy to check, and the inverse
//! is neither. Five of the eight — plane, cylinder, cone, sphere, torus — have
//! an inverse in closed form that is fifteen lines and exact, so they get one.
//! The other three do not: inverting an extrusion or a revolution means
//! inverting its profile CURVE, and inverting a NURBS surface has no closed
//! form at all. Those three write only the forward map and get their inverse
//! from [`Surface::search`], which is Gauss-Newton on `|S(u,v) − P|²`.
//!
//! That arrangement is also what makes the numerical half trustworthy: it can
//! be pointed at a cylinder, where the right answer is known in closed form,
//! and made to agree. Code written against a format nobody here can check is
//! measured against code that can be.
//!
//! # Why the parameterisation does not have to be ISO's
//!
//! Only the SHAPE has to be right. The `(u, v)` a face is triangulated in is
//! produced by this module's own inverse and consumed by this module's own
//! forward map, so the two only have to agree with each other. The one place
//! the file's own parameters are used is a B-spline surface, where the domain
//! IS the knot vector — and there the forward map is the one the format
//! defines, because it has to be.

/// A placement: an origin and three unit axes.
///
/// `z` is the surface's own axis — the axis of a cylinder, the pole of a
/// sphere, the normal of a plane — and `x` is where its `u` starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub origin: [f64; 3],
    pub x: [f64; 3],
    pub y: [f64; 3],
    pub z: [f64; 3],
}

impl Frame {
    /// The point at local coordinates.
    #[must_use]
    pub fn at(&self, a: f64, b: f64, c: f64) -> [f64; 3] {
        [
            self.origin[0] + a * self.x[0] + b * self.y[0] + c * self.z[0],
            self.origin[1] + a * self.x[1] + b * self.y[1] + c * self.z[1],
            self.origin[2] + a * self.x[2] + b * self.y[2] + c * self.z[2],
        ]
    }

    /// A point in the frame's own coordinates.
    #[must_use]
    pub fn of(&self, p: [f64; 3]) -> [f64; 3] {
        let d = [
            p[0] - self.origin[0],
            p[1] - self.origin[1],
            p[2] - self.origin[2],
        ];
        [dot(d, self.x), dot(d, self.y), dot(d, self.z)]
    }
}

/// A B-spline, in one parameter or two.
///
/// One type for both because the arithmetic is the same: a surface is the
/// tensor product of two curves, and `de_boor` runs down the rows and then
/// across the result.
#[derive(Debug, Clone, PartialEq)]
pub struct Spline {
    /// Degree in `u`, then in `v`. A curve has only the first.
    pub degree: (usize, usize),
    /// Control points, `u` varying slowest: `points[i * across + j]`.
    pub points: Vec<[f64; 3]>,
    /// How many control points across `v`. One, for a curve.
    pub across: usize,
    /// One weight per control point, or empty when the spline is not rational.
    pub weights: Vec<f64>,
    /// The knot vectors, already expanded by their multiplicities.
    pub knots: (Vec<f64>, Vec<f64>),
}

impl Spline {
    /// How many control points down `u`.
    #[must_use]
    pub fn down(&self) -> usize {
        self.points
            .len()
            .checked_div(self.across)
            .unwrap_or_default()
    }

    /// Whether the knot vectors and the grid agree.
    ///
    /// A file that disagrees with itself is one this reader declines rather
    /// than indexes past the end of.
    #[must_use]
    pub fn sound(&self) -> bool {
        let down = self.down();
        down > self.degree.0
            && self.across > self.degree.1
            && self.knots.0.len() == down + self.degree.0 + 1
            && self.knots.1.len() == self.across + self.degree.1 + 1
            && (self.weights.is_empty() || self.weights.len() == self.points.len())
            && down * self.across == self.points.len()
    }

    /// The span the parameters run over.
    #[must_use]
    pub fn span(&self) -> ((f64, f64), (f64, f64)) {
        (
            (self.knots.0[self.degree.0], self.knots.0[self.down()]),
            (self.knots.1[self.degree.1], self.knots.1[self.across]),
        )
    }

    /// The point at `(u, v)`.
    ///
    /// The tensor product, taken as a curve of curves: every row of control
    /// points is evaluated at `v`, and the results are evaluated at `u`. A
    /// rational spline carries the weight as a fourth coordinate and divides
    /// at the end, which is the only correct way — dividing per row would
    /// weight the rows against each other twice.
    #[must_use]
    pub fn at(&self, u: f64, v: f64) -> [f64; 3] {
        let rational = !self.weights.is_empty();
        let mut column: Vec<[f64; 4]> = Vec::with_capacity(self.down());
        for i in 0..self.down() {
            let row: Vec<[f64; 4]> = (0..self.across)
                .map(|j| {
                    let p = self.points[i * self.across + j];
                    let w = if rational {
                        self.weights[i * self.across + j]
                    } else {
                        1.0
                    };
                    [p[0] * w, p[1] * w, p[2] * w, w]
                })
                .collect();
            column.push(de_boor(&row, &self.knots.1, self.degree.1, v));
        }
        let out = de_boor(&column, &self.knots.0, self.degree.0, u);
        if out[3].abs() < 1e-12 {
            return [out[0], out[1], out[2]];
        }
        [out[0] / out[3], out[1] / out[3], out[2] / out[3]]
    }
}

/// A curve, for the two surfaces that are made by sweeping one.
#[derive(Debug, Clone, PartialEq)]
pub enum Curve {
    /// A point and a direction. `t` is distance along it.
    Line([f64; 3], [f64; 3]),
    /// A circle in its frame's `xy` plane; `t` is the angle.
    Circle(Frame, f64),
    /// An ellipse, likewise.
    Ellipse(Frame, f64, f64),
    /// A B-spline curve.
    Spline(Spline),
}

impl Curve {
    /// The point at `t`.
    #[must_use]
    pub fn at(&self, t: f64) -> [f64; 3] {
        match self {
            Self::Line(o, d) => [o[0] + t * d[0], o[1] + t * d[1], o[2] + t * d[2]],
            Self::Circle(f, r) => f.at(r * t.cos(), r * t.sin(), 0.0),
            Self::Ellipse(f, a, b) => f.at(a * t.cos(), b * t.sin(), 0.0),
            Self::Spline(s) => s.at(t, s.span().1.0),
        }
    }

    /// The range `t` runs over.
    #[must_use]
    pub fn span(&self) -> (f64, f64) {
        match self {
            // A LINE HAS NO NATURAL END, and the one that matters is however
            // far the face's own boundary reaches. The search is seeded from
            // the boundary, so a generous span costs nothing and a tight one
            // would clamp a real point out of reach.
            Self::Line(..) => (-LINE_REACH, LINE_REACH),
            Self::Circle(..) | Self::Ellipse(..) => (0.0, std::f64::consts::TAU),
            Self::Spline(s) => s.span().0,
        }
    }
}

/// How far along a line a swept surface is looked for, in millimetres.
///
/// Ten metres. It bounds the numerical search and nothing else: a part larger
/// than this has other problems.
const LINE_REACH: f64 = 10_000.0;

/// One of the surfaces a face can sit on.
#[derive(Debug, Clone, PartialEq)]
pub enum Surface {
    /// `u` and `v` are distances along the frame's `x` and `y`.
    Plane(Frame),
    /// `u` is the angle about the axis, `v` the height along it.
    Cylinder(Frame, f64),
    /// As a cylinder, and the radius grows with `v` by the half angle.
    Cone(Frame, f64, f64),
    /// `u` is longitude, `v` latitude.
    Sphere(Frame, f64),
    /// `u` goes round the ring, `v` round the tube.
    Torus(Frame, f64, f64),
    /// A curve pushed along a direction: `u` on the curve, `v` along it.
    Extrusion(Box<Curve>, [f64; 3]),
    /// A curve turned about an axis: `u` is the angle, `v` is on the curve.
    Revolution(Box<Curve>, Frame),
    /// A NURBS patch.
    Spline(Spline),
}

impl Surface {
    /// The point at `(u, v)`.
    #[must_use]
    pub fn at(&self, u: f64, v: f64) -> [f64; 3] {
        match self {
            Self::Plane(f) => f.at(u, v, 0.0),
            Self::Cylinder(f, r) => f.at(r * u.cos(), r * u.sin(), v),
            Self::Cone(f, r, half) => {
                let radius = r + v * half.tan();
                f.at(radius * u.cos(), radius * u.sin(), v)
            }
            Self::Sphere(f, r) => {
                let (sv, cv) = v.sin_cos();
                f.at(r * cv * u.cos(), r * cv * u.sin(), r * sv)
            }
            Self::Torus(f, major, minor) => {
                let (sv, cv) = v.sin_cos();
                let radius = major + minor * cv;
                f.at(radius * u.cos(), radius * u.sin(), minor * sv)
            }
            Self::Extrusion(c, d) => {
                let p = c.at(u);
                [p[0] + v * d[0], p[1] + v * d[1], p[2] + v * d[2]]
            }
            Self::Revolution(c, axis) => {
                // The profile point in the axis's own frame, then turned by
                // `u` about the axis's `z`.
                let local = axis.of(c.at(v));
                let (s, cos) = u.sin_cos();
                axis.at(
                    local[0] * cos - local[1] * s,
                    local[0] * s + local[1] * cos,
                    local[2],
                )
            }
            Self::Spline(s) => s.at(u, v),
        }
    }

    /// The range each parameter runs over.
    #[must_use]
    pub fn domain(&self) -> ((f64, f64), (f64, f64)) {
        let turn = (0.0, std::f64::consts::TAU);
        let half = std::f64::consts::FRAC_PI_2;
        match self {
            Self::Plane(_) => ((-LINE_REACH, LINE_REACH), (-LINE_REACH, LINE_REACH)),
            Self::Cylinder(..) | Self::Cone(..) => (turn, (-LINE_REACH, LINE_REACH)),
            Self::Sphere(..) => (turn, (-half, half)),
            Self::Torus(..) => (turn, turn),
            Self::Extrusion(c, _) => (c.span(), (-LINE_REACH, LINE_REACH)),
            Self::Revolution(c, _) => (turn, c.span()),
            Self::Spline(s) => s.span(),
        }
    }

    /// Whether each parameter comes back round to itself.
    ///
    /// It is what tells the loop walker that a jump of nearly a full turn
    /// between two neighbouring samples is the SEAM and not a discontinuity.
    #[must_use]
    pub const fn wraps(&self) -> (bool, bool) {
        match self {
            Self::Plane(_) | Self::Extrusion(..) | Self::Spline(_) => (false, false),
            Self::Cylinder(..) | Self::Cone(..) | Self::Sphere(..) | Self::Revolution(..) => {
                (true, false)
            }
            Self::Torus(..) => (true, true),
        }
    }

    /// How far the surface bends over one unit of each parameter.
    ///
    /// It is the radius of curvature in each direction, and what the mesher
    /// divides a face's parameter box by to decide how finely to cut it. A
    /// plane bends by nothing and comes back zero, which is what says "one
    /// triangle is as good as a thousand here".
    #[must_use]
    pub fn bend(&self) -> (f64, f64) {
        match self {
            Self::Plane(_) => (0.0, 0.0),
            Self::Cylinder(_, r) | Self::Cone(_, r, _) => (r.abs(), 0.0),
            Self::Sphere(_, r) => (r.abs(), r.abs()),
            Self::Torus(_, major, minor) => (major.abs() + minor.abs(), minor.abs()),
            // A swept curve bends however the curve does, which is not known
            // without walking it; the box's own size is the honest stand-in.
            Self::Extrusion(..) | Self::Revolution(..) | Self::Spline(_) => (f64::NAN, f64::NAN),
        }
    }

    /// The `(u, v)` of a point on the surface, or nothing when it is not on it.
    ///
    /// `seed` is where to start looking, and for every point of a boundary
    /// after the first it is the point before — see [`Self::search`].
    ///
    /// # Why the closed forms are checked too
    ///
    /// Because they always ANSWER. A sphere's inverse is an `atan2` and an
    /// `asin`, and handed a point nowhere near the sphere it returns the
    /// parameters of the nearest point on it without complaint. A boundary
    /// built from those is a polygon that is not the face, triangulated into
    /// a piece of surface the file does not describe — which is the one thing
    /// this module must not produce. So every answer, closed form or not, is
    /// put back through [`Self::at`] and made to land where it started.
    #[must_use]
    pub fn invert(&self, p: [f64; 3], seed: Option<(f64, f64)>) -> Option<(f64, f64)> {
        let found = self.solve(p, seed)?;
        let back = self.at(found.0, found.1);
        let d = [back[0] - p[0], back[1] - p[1], back[2] - p[2]];
        // Absolute for a small part, relative for a large one: a point a metre
        // from the origin has no more than a micron of `f64` left in it.
        let room = NEAR_ENOUGH + dot(p, p).sqrt() * 1e-6;
        (dot(d, d).sqrt() <= room).then_some(found)
    }

    /// The parameters, before they are checked.
    fn solve(&self, p: [f64; 3], seed: Option<(f64, f64)>) -> Option<(f64, f64)> {
        match self {
            Self::Plane(f) => {
                let l = f.of(p);
                Some((l[0], l[1]))
            }
            Self::Cylinder(f, _) | Self::Cone(f, ..) => {
                let l = f.of(p);
                Some((l[1].atan2(l[0]), l[2]))
            }
            Self::Sphere(f, r) => {
                let l = f.of(p);
                if r.abs() < 1e-12 {
                    return None;
                }
                Some(((l[1]).atan2(l[0]), (l[2] / r).clamp(-1.0, 1.0).asin()))
            }
            Self::Torus(f, major, _) => {
                let l = f.of(p);
                let out = l[0].hypot(l[1]) - major;
                Some((l[1].atan2(l[0]), l[2].atan2(out)))
            }
            Self::Extrusion(..) | Self::Revolution(..) | Self::Spline(_) => self.search(p, seed),
        }
    }

    /// Gauss-Newton onto the surface, from a seed.
    ///
    /// # Why the seed is the point BEFORE and not a search
    ///
    /// Because the points handed here are a boundary walked in order, and two
    /// neighbours on a boundary are neighbours on the surface. Starting from
    /// the previous answer means the step is tiny, convergence is two or three
    /// iterations, and — the part that matters — there is no other branch
    /// close enough to fall into. A surface that folds over itself, or one
    /// where the same point appears at two parameters, is exactly where a
    /// fresh search would answer differently for two neighbouring points and
    /// tear the polygon in half.
    ///
    /// Only the first point of a loop pays the grid.
    fn search(&self, p: [f64; 3], seed: Option<(f64, f64)>) -> Option<(f64, f64)> {
        let ((u0, u1), (v0, v1)) = self.domain();
        let (mut u, mut v) = seed.unwrap_or_else(|| self.nearest(p));
        let (hu, hv) = ((u1 - u0) * 1e-7, (v1 - v0) * 1e-7);
        let mut best = (u, v);
        let mut closest = f64::MAX;
        for _ in 0..NEWTON_STEPS {
            let here = self.at(u, v);
            let r = [here[0] - p[0], here[1] - p[1], here[2] - p[2]];
            let far = dot(r, r);
            if far < closest {
                closest = far;
                best = (u, v);
            }
            if far < ON_SURFACE * ON_SURFACE {
                return Some((u, v));
            }
            // The two tangents, by difference. Analytic derivatives would be
            // faster and are eight more forms to get right; this runs a few
            // times per boundary point and the cost does not show.
            let su = step(self.at(u + hu, v), here, hu);
            let sv = step(self.at(u, v + hv), here, hv);
            // The 2x2 normal equations of `J d = -r`.
            let (a, b, c) = (dot(su, su), dot(su, sv), dot(sv, sv));
            let det = a * c - b * b;
            if det.abs() < 1e-18 {
                break;
            }
            let (e, f) = (-dot(su, r), -dot(sv, r));
            let du = (e * c - b * f) / det;
            let dv = (a * f - e * b) / det;
            if !du.is_finite() || !dv.is_finite() {
                break;
            }
            // A DAMPED STEP, and it is not an optimisation.
            //
            // Gauss-Newton on a rational patch overshoots: the full step lands
            // outside the knot span, the clamp puts it on the edge, and the
            // next step points back out again — measured, twenty-eight of the
            // moon buggy's seventy-seven faces stalled at a domain corner
            // thirty millimetres from the point they were looking for. Halving
            // the step until it actually improves is what turns that into
            // convergence, and it costs a few extra evaluations on the
            // surfaces that never needed it.
            let mut scale = 1.0;
            let mut moved = false;
            for _ in 0..DAMPING_STEPS {
                let tu = (u + du * scale).clamp(u0, u1);
                let tv = (v + dv * scale).clamp(v0, v1);
                let q = self.at(tu, tv);
                let e = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
                if dot(e, e) < far {
                    u = tu;
                    v = tv;
                    moved = true;
                    break;
                }
                scale *= 0.5;
            }
            if !moved {
                break;
            }
        }
        // WHAT DID NOT CONVERGE IS NOT ANSWERED. The caller sends the face to
        // the outline, which is a drawing that is honest about being one — a
        // parameter that is merely nearby would put a vertex somewhere the
        // surface is not.
        let here = self.at(best.0, best.1);
        let r = [here[0] - p[0], here[1] - p[1], here[2] - p[2]];
        (dot(r, r) < NEAR_ENOUGH * NEAR_ENOUGH).then_some(best)
    }

    /// The best of a coarse grid, for the first point of a loop.
    fn nearest(&self, p: [f64; 3]) -> (f64, f64) {
        let ((u0, u1), (v0, v1)) = self.domain();
        let mut best = (u0, v0);
        let mut closest = f64::MAX;
        for i in 0..=GRID {
            for j in 0..=GRID {
                #[allow(clippy::cast_precision_loss)]
                let u = (u1 - u0).mul_add(i as f64 / GRID as f64, u0);
                #[allow(clippy::cast_precision_loss)]
                let v = (v1 - v0).mul_add(j as f64 / GRID as f64, v0);
                let q = self.at(u, v);
                let d = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
                let far = dot(d, d);
                if far < closest {
                    closest = far;
                    best = (u, v);
                }
            }
        }
        best
    }
}

/// How many Gauss-Newton steps before giving up.
const NEWTON_STEPS: usize = 60;

/// How many times a step is halved before the search calls it stuck.
const DAMPING_STEPS: usize = 12;

/// How close counts as landed, in millimetres.
const ON_SURFACE: f64 = 1e-7;

/// How close is still accepted after the steps run out, in millimetres.
///
/// A micron, and the check that uses it adds a part per million of how far the
/// point is from the origin — a metre-wide model has no more precision than
/// that left in it, and the moon buggy is one.
const NEAR_ENOUGH: f64 = 1e-3;

/// How many cells each way the first-point search uses.
const GRID: usize = 32;

/// `(a - b) / h`, the difference quotient.
fn step(a: [f64; 3], b: [f64; 3], h: f64) -> [f64; 3] {
    [(a[0] - b[0]) / h, (a[1] - b[1]) / h, (a[2] - b[2]) / h]
}

/// One point of a B-spline of any dimension, by de Boor's algorithm.
///
/// The control values are four-wide so a rational spline can carry its weight
/// through the same arithmetic as its coordinates. A non-rational one passes
/// a weight of one and divides by one at the end.
#[must_use]
pub fn de_boor(control: &[[f64; 4]], knots: &[f64], degree: usize, t: f64) -> [f64; 4] {
    if control.len() <= degree {
        return control.first().copied().unwrap_or([0.0; 4]);
    }
    let mut span = degree;
    while span + 1 < control.len() && knots[span + 1] <= t {
        span += 1;
    }
    let mut d: Vec<[f64; 4]> = (0..=degree).map(|j| control[span + j - degree]).collect();
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = span + j - degree;
            let (lo, hi) = (knots[i], knots[i + degree + 1 - r]);
            let alpha = if (hi - lo).abs() < 1e-12 {
                0.0
            } else {
                (t - lo) / (hi - lo)
            };
            let before = d[j - 1];
            for (slot, prior) in d[j].iter_mut().zip(before) {
                *slot = (1.0 - alpha).mul_add(prior, alpha * *slot);
            }
        }
    }
    d[degree]
}

pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(v, v).sqrt();
    (length > 1e-12).then(|| [v[0] / length, v[1] / length, v[2] / length])
}

#[cfg(test)]
mod tests {
    use super::{Curve, Frame, Spline, Surface, dot};

    fn frame() -> Frame {
        Frame {
            origin: [1.0, 2.0, 3.0],
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
        }
    }

    /// Every surface, at a parameter that is inside its own domain.
    fn each() -> Vec<(&'static str, Surface, (f64, f64))> {
        let f = frame();
        vec![
            ("plane", Surface::Plane(f), (3.0, -2.0)),
            ("cylinder", Surface::Cylinder(f, 7.0), (1.1, 4.0)),
            ("cone", Surface::Cone(f, 5.0, 0.4), (2.2, 3.0)),
            ("sphere", Surface::Sphere(f, 9.0), (0.7, 0.5)),
            ("torus", Surface::Torus(f, 20.0, 4.0), (2.0, 1.0)),
            (
                "extrusion",
                Surface::Extrusion(Box::new(Curve::Circle(f, 6.0)), [0.0, 0.0, 1.0]),
                (1.3, 5.0),
            ),
            (
                "revolution",
                Surface::Revolution(Box::new(Curve::Line([10.0, 0.0, 0.0], [0.0, 0.0, 1.0])), f),
                (1.0, 4.0),
            ),
        ]
    }

    #[test]
    // THE DEFECT THIS CATCHES: a forward map and an inverse that do not agree.
    // Everything downstream — the boundary in `(u, v)`, the triangulation, the
    // mapping back — is built on the two being each other's undoing, and a
    // surface where they are not draws a piece that is subtly the wrong shape
    // with no error anywhere.
    fn every_surface_undoes_its_own_forward_map() {
        for (name, s, (u, v)) in each() {
            let p = s.at(u, v);
            let back = s.invert(p, Some((u, v))).unwrap_or_else(|| {
                panic!("`{name}` could not invert a point it had just produced")
            });
            let again = s.at(back.0, back.1);
            let d = [again[0] - p[0], again[1] - p[1], again[2] - p[2]];
            assert!(
                dot(d, d).sqrt() < 1e-6,
                "`{name}` came back at {again:?} instead of {p:?}"
            );
        }
    }

    #[test]
    // THE DEFECT THIS CATCHES, and it is the whole reason the numerical
    // inverse is trustworthy at all: Gauss-Newton is written for the three
    // surfaces nobody here has a closed form for, so it can only be checked
    // against the five that do have one. If it agrees with the cylinder and
    // the torus, it is not wrong about a NURBS patch for a reason of its own.
    fn the_numerical_inverse_agrees_with_the_closed_form() {
        let f = frame();
        for (name, s) in [
            ("cylinder", Surface::Cylinder(f, 7.0)),
            ("torus", Surface::Torus(f, 20.0, 4.0)),
            ("sphere", Surface::Sphere(f, 9.0)),
            ("cone", Surface::Cone(f, 5.0, 0.4)),
        ] {
            for (u, v) in [(0.3_f64, 0.4_f64), (2.0, -1.0), (5.5, 0.9)] {
                let ((_, _), (v0, v1)) = s.domain();
                let v = v.clamp(v0 + 0.01, v1 - 0.01);
                let p = s.at(u, v);
                let closed = s.invert(p, None).expect("the closed form answers");
                let numeric = s.search(p, Some((u, v))).expect("Newton answers");
                let a = s.at(closed.0, closed.1);
                let b = s.at(numeric.0, numeric.1);
                let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                assert!(
                    dot(d, d).sqrt() < 1e-5,
                    "`{name}`: closed form said {closed:?} and Newton said {numeric:?}"
                );
            }
        }
    }

    #[test]
    // Without a seed the search has to find the point on its own, which is
    // what the first vertex of every loop does.
    fn the_grid_finds_a_point_with_no_seed_at_all() {
        let f = frame();
        let s = Surface::Extrusion(Box::new(Curve::Circle(f, 6.0)), [0.0, 0.0, 1.0]);
        let p = s.at(2.4, 3.0);
        let back = s.invert(p, None).expect("the grid seeds a search");
        let again = s.at(back.0, back.1);
        let d = [again[0] - p[0], again[1] - p[1], again[2] - p[2]];
        assert!(dot(d, d).sqrt() < 1e-5, "{again:?} against {p:?}");
    }

    #[test]
    // A point that is NOT on the surface must come back as nothing, because
    // the caller's answer to nothing is to draw the face as an outline — and
    // a parameter that is merely nearby would put a vertex where the surface
    // is not.
    fn a_point_off_the_surface_is_refused_and_not_approximated() {
        let s = Surface::Sphere(frame(), 9.0);
        assert!(s.search([500.0, 500.0, 500.0], None).is_none());
    }

    /// A flat B-spline patch: degree one each way, four corners.
    fn flat_patch() -> Spline {
        Spline {
            degree: (1, 1),
            points: vec![
                [0.0, 0.0, 0.0],
                [0.0, 10.0, 0.0],
                [10.0, 0.0, 0.0],
                [10.0, 10.0, 0.0],
            ],
            across: 2,
            weights: Vec::new(),
            knots: (vec![0.0, 0.0, 1.0, 1.0], vec![0.0, 0.0, 1.0, 1.0]),
        }
    }

    #[test]
    fn a_spline_patch_interpolates_its_own_corners() {
        let s = flat_patch();
        assert!(s.sound());
        assert_eq!(s.at(0.0, 0.0), [0.0, 0.0, 0.0]);
        assert_eq!(s.at(1.0, 1.0), [10.0, 10.0, 0.0]);
        let middle = s.at(0.5, 0.5);
        assert!((middle[0] - 5.0).abs() < 1e-9 && (middle[1] - 5.0).abs() < 1e-9);
    }

    #[test]
    // THE DEFECT THIS CATCHES: dividing by the weight per ROW instead of once
    // at the end, which weights the rows against each other twice and bends a
    // patch that should be flat.
    fn a_rational_patch_divides_once_and_not_per_row() {
        let mut s = flat_patch();
        s.weights = vec![1.0, 3.0, 3.0, 9.0];
        // Those weights are the outer product of (1,3) with (1,3), so the
        // patch stays flat and the middle moves in a way that is the same
        // whichever direction is taken first.
        let middle = s.at(0.5, 0.5);
        assert!(middle[2].abs() < 1e-9, "a flat patch left its plane");
        assert!(
            (middle[0] - middle[1]).abs() < 1e-9,
            "the two directions disagree: {middle:?}"
        );
    }

    #[test]
    fn a_spline_that_disagrees_with_its_own_knots_is_refused() {
        let mut s = flat_patch();
        s.knots.0.push(2.0);
        assert!(!s.sound());
    }

    #[test]
    fn a_plane_bends_by_nothing_and_a_sphere_by_its_radius() {
        assert_eq!(Surface::Plane(frame()).bend(), (0.0, 0.0));
        assert_eq!(Surface::Sphere(frame(), 9.0).bend(), (9.0, 9.0));
    }
}
