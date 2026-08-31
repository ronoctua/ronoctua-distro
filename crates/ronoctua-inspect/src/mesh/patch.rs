//! Turning a trimmed parameter domain into triangles.
//!
//! A face is a surface plus the loops of curve that cut a piece out of it. Once
//! those loops are in the surface's own `(u, v)` — which is
//! [`super::surface::Surface::invert`]'s whole job — what is left is a flat
//! problem: a polygon, possibly with holes in it, to be cut into triangles.
//!
//! # Why holes are bridged and not handled
//!
//! Because ear clipping knows one polygon. A hole is spliced into the outer
//! loop by a BRIDGE — a segment out to the hole and back, walked in the
//! opposite direction — which turns two loops into one that has a slit in it.
//! The slit is zero-width, so it costs no area and no triangle, and every
//! algorithm downstream sees a simple polygon.
//!
//! # Why the refinement is uniform, and what that costs
//!
//! Ear clipping answers "which triangles", not "how many", and on a face whose
//! surface bends it will happily lay one triangle across a whole fillet. So
//! the result is subdivided: every triangle into four, every edge at its
//! middle, a fixed number of times.
//!
//! **It was not always uniform, and the version that was not did not work.**
//! Splitting only the INTERIOR edges keeps the boundary exactly where the two
//! faces either side of it agree it is — which is watertight, and which is
//! what one wants. But ear clipping produces FANS, and a fan's triangles are
//! slivers whose longest edge is the boundary one. Refusing to split that and
//! splitting the short interior edge instead makes a thinner sliver and a new
//! long spoke, for ever: measured, the worst interior edge stayed between 1.7
//! and 9.3 times the target no matter how many passes were spent, while the
//! triangle count kept doubling. A refinement that cannot converge is not a
//! refinement.
//!
//! Uniform subdivision converges by construction — `4^k` triangles, every edge
//! halved `k` times — and has no T-junctions because every edge is split.
//!
//! **The price is that two faces no longer meet exactly.** Each splits the
//! boundary chord they share at its middle in ITS OWN parameters, and the two
//! points land on two different surfaces, each off the true curve by the
//! chord's sagitta. The boundary arrives sampled at under six degrees an arc,
//! where that is about a thousandth of the radius — under a pixel at the size
//! this is ever drawn. What it is NOT is exactly zero, and that is why a
//! boundary representation reports no volume and no "surface closed": those
//! would be numbers about a mesh that is a thousandth of a millimetre open.

/// A trimmed domain: one outer loop and any number of holes, in `(u, v)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    /// The outer boundary, in order and not closed.
    pub outer: Vec<[f64; 2]>,
    /// The holes, likewise.
    pub holes: Vec<Vec<[f64; 2]>>,
}

/// What came out: the points, and the triangles over them.
#[derive(Debug, Clone, PartialEq)]
pub struct Cut {
    pub points: Vec<[f64; 2]>,
    pub triangles: Vec<[usize; 3]>,
}

/// How many triangles one face may come to.
///
/// A face past this is one whose parameter box and curvature ask for more
/// detail than a 512-pixel panel can show. It is a resource limit, like
/// [`super::TRIANGLE_CEILING`] one file over.
pub const FACE_CEILING: usize = 40_000;

/// How many bridge candidates are tried before a hole is given up on.
const BRIDGE_TRIES: usize = 4_000;

/// Cuts the domain into triangles.
///
/// `depth` is how many times to subdivide afterwards — see the note at the top
/// of the file. `None` when the loops do not describe a polygon this can work
/// with, and the caller's answer to that is to draw the face as an outline.
#[must_use]
pub fn tessellate(patch: &Patch, depth: u32) -> Option<Cut> {
    if patch.outer.len() < 3 {
        return None;
    }
    let points = merge(patch)?;
    let triangles = clip(&points)?;
    Some(refine(points, triangles, depth))
}

/// How many times a face must be halved for its chord to stay under `sag`.
///
/// `bend` is the radius the surface turns through, `span` how much parameter
/// the face covers, and the sagitta of a chord over an angle `a` on a radius
/// `r` is `r(1 − cos(a/2))`. Solving for the number of halvings is one
/// logarithm, and a surface that does not bend comes back zero.
#[must_use]
pub fn depth_for(bend: f64, span: f64, sag: f64) -> u32 {
    if !bend.is_finite() || bend <= 0.0 || span <= 0.0 || sag <= 0.0 {
        return 0;
    }
    let mut k = 0;
    let mut angle = span;
    while bend * (1.0 - (angle * 0.5).cos()) > sag && k < DEPTH_CEILING {
        angle *= 0.5;
        k += 1;
    }
    k
}

/// How deep the subdivision may ever go.
///
/// Four is sixteen times the triangles, which is past anything a 512-pixel
/// panel resolves; [`FACE_CEILING`] is what actually stops a big face first.
pub const DEPTH_CEILING: u32 = 4;

/// The outer loop with every hole spliced into it.
fn merge(patch: &Patch) -> Option<Vec<[f64; 2]>> {
    // THE OUTER LOOP GOES ANTICLOCKWISE AND A HOLE THE OTHER WAY. Ear clipping
    // asks a question about which side is inside, and the two loops answering
    // it differently is what makes a slit a slit.
    let mut ring: Vec<[f64; 2]> = patch.outer.clone();
    if area_of(&ring) < 0.0 {
        ring.reverse();
    }
    for hole in &patch.holes {
        if hole.len() < 3 {
            continue;
        }
        let mut inner: Vec<[f64; 2]> = hole.clone();
        if area_of(&inner) > 0.0 {
            inner.reverse();
        }
        let Some((at, from)) = bridge(&ring, &inner) else {
            // A HOLE THAT CANNOT BE REACHED is a hole this reader does not
            // understand, and filling over it would draw material where the
            // file says there is none.
            return None;
        };
        // The slit: out along the bridge, round the hole, and back through
        // the same outer vertex. It is zero-width, so it costs no area and no
        // triangle, and everything downstream sees one simple polygon.
        let mut joined: Vec<[f64; 2]> = ring[..=at].to_vec();
        for k in 0..=inner.len() {
            joined.push(inner[(from + k) % inner.len()]);
        }
        joined.push(ring[at]);
        joined.extend_from_slice(&ring[at + 1..]);
        ring = joined;
    }
    Some(ring)
}

/// Where to cut the outer loop, and which vertex of the hole to enter at.
///
/// The nearest visible pair. A bridge that crossed an edge would fold the
/// polygon over itself, so every candidate is checked against every edge
/// before it is taken — in order of distance, so the first that passes is
/// almost always the first tried.
fn bridge(outer: &[[f64; 2]], hole: &[[f64; 2]]) -> Option<(usize, usize)> {
    let mut pairs: Vec<(f64, usize, usize)> = Vec::with_capacity(outer.len() * hole.len());
    for (i, a) in outer.iter().enumerate() {
        for (j, b) in hole.iter().enumerate() {
            pairs.push(((a[0] - b[0]).hypot(a[1] - b[1]), i, j));
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, i, j) in pairs.into_iter().take(BRIDGE_TRIES) {
        if clear(outer[i], hole[j], outer, i, usize::MAX)
            && clear(outer[i], hole[j], hole, usize::MAX, j)
        {
            return Some((i, j));
        }
    }
    None
}

/// Whether a segment crosses none of a ring's edges.
///
/// `skip_a` and `skip_b` are the ring positions the segment starts or ends at,
/// whose own edges touch it by construction and are not crossings.
fn clear(a: [f64; 2], b: [f64; 2], ring: &[[f64; 2]], skip_a: usize, skip_b: usize) -> bool {
    for i in 0..ring.len() {
        let j = (i + 1) % ring.len();
        if i == skip_a || j == skip_a || i == skip_b || j == skip_b {
            continue;
        }
        if crosses(a, b, ring[i], ring[j]) {
            return false;
        }
    }
    true
}

/// Whether two segments properly cross.
fn crosses(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let s1 = side(a, b, c);
    let s2 = side(a, b, d);
    let s3 = side(c, d, a);
    let s4 = side(c, d, b);
    (s1 * s2 < 0.0) && (s3 * s4 < 0.0)
}

/// Which side of `a`→`b` the point `p` falls on.
fn side(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Twice the signed area of a ring: positive anticlockwise.
fn area_of(ring: &[[f64; 2]]) -> f64 {
    let mut sum = 0.0;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        sum += a[0] * b[1] - b[0] * a[1];
    }
    sum
}

/// Ear clipping, on an anticlockwise simple polygon.
fn clip(ring: &[[f64; 2]]) -> Option<Vec<[usize; 3]>> {
    let mut left: Vec<usize> = (0..ring.len()).collect();
    let mut out = Vec::with_capacity(ring.len());
    // A pass that removes nothing means the polygon is not simple, and going
    // round again would go round for ever.
    let mut stuck = 0;
    while left.len() > 3 {
        let n = left.len();
        // THE BEST EAR, and not the first one that works.
        //
        // It costs one full scan instead of an early break, and it is what
        // makes the result usable. A cylindrical face's domain is a long thin
        // rectangle with its two long sides sampled densely — and taking the
        // first ear walks along one side, producing triangles that reach the
        // whole way across. Those are the triangles that cut a chord through
        // the middle of the cylinder, and the subdivision that then has to
        // rescue them multiplies the WHOLE face: measured, first-ear clipping
        // put the owner's part at 359 thousand triangles.
        //
        // The score is the triangle's area over its longest side squared —
        // biggest for an equilateral one, near zero for a sliver — so the ear
        // that is chosen is the one that leaves the rest in the best shape.
        let mut best: Option<(f64, usize)> = None;
        for k in 0..n {
            let (a, b, c) = (left[(k + n - 1) % n], left[k], left[(k + 1) % n]);
            let twice = side(ring[a], ring[b], ring[c]);
            if twice <= 0.0 {
                continue;
            }
            if left
                .iter()
                .any(|&p| p != a && p != b && p != c && inside(ring[p], ring[a], ring[b], ring[c]))
            {
                continue;
            }
            let longest = [(a, b), (b, c), (c, a)]
                .into_iter()
                .map(|(i, j)| (ring[i][0] - ring[j][0]).powi(2) + (ring[i][1] - ring[j][1]).powi(2))
                .fold(0.0_f64, f64::max);
            let score = if longest > 0.0 { twice / longest } else { 0.0 };
            if best.is_none_or(|(had, _)| score > had) {
                best = Some((score, k));
            }
        }
        let cut = best.is_some();
        if let Some((_, k)) = best {
            let (a, b, c) = (left[(k + n - 1) % n], left[k], left[(k + 1) % n]);
            out.push([a, b, c]);
            left.remove(k);
        }
        if !cut {
            stuck += 1;
            if stuck > 1 {
                return None;
            }
            // ONE SECOND CHANCE, by dropping the sharpest vertex. A polygon
            // with a repeated point — which a boundary sampled from two curves
            // that meet can produce — has no ear at that corner and is
            // otherwise fine.
            let worst = (0..n)
                .min_by(|&i, &j| {
                    let f = |k: usize| {
                        let (a, b, c) = (left[(k + n - 1) % n], left[k], left[(k + 1) % n]);
                        side(ring[a], ring[b], ring[c]).abs()
                    };
                    f(i).total_cmp(&f(j))
                })
                .unwrap_or(0);
            left.remove(worst);
        } else {
            stuck = 0;
        }
        if out.len() > FACE_CEILING {
            return None;
        }
    }
    if left.len() == 3 {
        out.push([left[0], left[1], left[2]]);
    }
    (!out.is_empty()).then_some(out)
}

/// Whether `p` is inside the triangle `a b c`.
fn inside(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    side(a, b, p) > 0.0 && side(b, c, p) > 0.0 && side(c, a, p) > 0.0
}

/// The key an edge is remembered by, whichever way round it is given.
const fn key(a: usize, b: usize) -> (usize, usize) {
    if a < b { (a, b) } else { (b, a) }
}

/// Every triangle into four, `depth` times.
///
/// Splitting every edge at its middle and rebuilding each triangle as four is
/// the one refinement that terminates with a count known in advance and cannot
/// leave a T-junction: an edge is either split for both of the triangles that
/// share it or for neither, because it is split for ALL of them.
fn refine(mut points: Vec<[f64; 2]>, mut triangles: Vec<[usize; 3]>, depth: u32) -> Cut {
    for _ in 0..depth.min(DEPTH_CEILING) {
        if triangles.len() * 4 > FACE_CEILING {
            break;
        }
        let mut middle: std::collections::HashMap<(usize, usize), usize> =
            std::collections::HashMap::new();
        let mut halve = |points: &mut Vec<[f64; 2]>, a: usize, b: usize| -> usize {
            *middle.entry(key(a, b)).or_insert_with(|| {
                points.push([
                    f64::midpoint(points[a][0], points[b][0]),
                    f64::midpoint(points[a][1], points[b][1]),
                ]);
                points.len() - 1
            })
        };
        let mut grown = Vec::with_capacity(triangles.len() * 4);
        for t in &triangles {
            let a = halve(&mut points, t[0], t[1]);
            let b = halve(&mut points, t[1], t[2]);
            let c = halve(&mut points, t[2], t[0]);
            grown.push([t[0], a, c]);
            grown.push([a, t[1], b]);
            grown.push([c, b, t[2]]);
            grown.push([a, b, c]);
        }
        triangles = grown;
    }
    Cut { points, triangles }
}

#[cfg(test)]
mod tests {
    use super::{Cut, Patch, depth_for, tessellate};

    /// The area the triangles cover, which is what a triangulation must
    /// preserve and the one number that catches a fold, a gap or a lost hole.
    fn covered(cut: &Cut) -> f64 {
        cut.triangles
            .iter()
            .map(|t| {
                let (a, b, c) = (cut.points[t[0]], cut.points[t[1]], cut.points[t[2]]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
            })
            .sum()
    }

    fn square(from: f64, to: f64) -> Vec<[f64; 2]> {
        vec![[from, from], [to, from], [to, to], [from, to]]
    }

    fn flat(outer: Vec<[f64; 2]>, holes: Vec<Vec<[f64; 2]>>) -> Cut {
        tessellate(&Patch { outer, holes }, 0).expect("it triangulates")
    }

    #[test]
    fn a_square_becomes_two_triangles_covering_its_area() {
        let cut = flat(square(0.0, 10.0), Vec::new());
        assert_eq!(cut.triangles.len(), 2);
        assert!((covered(&cut) - 100.0).abs() < 1e-9);
    }

    #[test]
    // THE DEFECT THIS CATCHES: filling over a hole. A face with a bore in it
    // drawn solid is a part that looks right and is not the part.
    fn a_square_with_a_square_hole_keeps_the_hole() {
        let cut = flat(square(0.0, 10.0), vec![square(3.0, 7.0)]);
        assert!(
            (covered(&cut) - 84.0).abs() < 1e-6,
            "covered {} instead of 84",
            covered(&cut)
        );
    }

    #[test]
    fn the_winding_a_loop_arrives_in_does_not_change_the_answer() {
        let mut backwards = square(0.0, 10.0);
        backwards.reverse();
        let mut hole = square(3.0, 7.0);
        hole.reverse();
        let a = flat(square(0.0, 10.0), vec![square(3.0, 7.0)]);
        let b = flat(backwards, vec![hole]);
        assert!((covered(&a) - covered(&b)).abs() < 1e-6);
    }

    #[test]
    fn two_holes_are_both_kept() {
        let cut = flat(
            square(0.0, 20.0),
            vec![square(2.0, 5.0), square(12.0, 16.0)],
        );
        assert!(
            (covered(&cut) - (400.0 - 9.0 - 16.0)).abs() < 1e-6,
            "covered {}",
            covered(&cut)
        );
    }

    #[test]
    fn a_concave_polygon_is_not_filled_across_its_notch() {
        // An L. Ear clipping that ignored convexity would cover 100.
        let cut = flat(
            vec![
                [0.0, 0.0],
                [10.0, 0.0],
                [10.0, 4.0],
                [4.0, 4.0],
                [4.0, 10.0],
                [0.0, 10.0],
            ],
            Vec::new(),
        );
        assert!(
            (covered(&cut) - 64.0).abs() < 1e-9,
            "covered {}",
            covered(&cut)
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: a subdivision that loses or folds area. It is
    // the one thing uniform splitting can still get wrong, and it gets it
    // wrong silently — a folded triangle draws over its neighbour and nothing
    // says so.
    fn subdividing_multiplies_the_triangles_and_keeps_the_area() {
        for depth in 0..=3 {
            let cut = tessellate(
                &Patch {
                    outer: square(0.0, 10.0),
                    holes: vec![square(3.0, 7.0)],
                },
                depth,
            )
            .unwrap();
            assert!(
                (covered(&cut) - 84.0).abs() < 1e-6,
                "depth {depth} covered {}",
                covered(&cut)
            );
            let flat_count = flat(square(0.0, 10.0), vec![square(3.0, 7.0)])
                .triangles
                .len();
            assert_eq!(
                cut.triangles.len(),
                flat_count * 4_usize.pow(depth),
                "depth {depth} did not quadruple"
            );
        }
    }

    #[test]
    // THE DEFECT THIS CATCHES: a T-junction, which is a hairline crack that
    // only shows on a screen. Every edge of every triangle must be an edge of
    // exactly one or two triangles — never a segment that another triangle's
    // vertex sits in the middle of.
    fn subdividing_leaves_no_edge_hanging() {
        let cut = tessellate(
            &Patch {
                outer: square(0.0, 10.0),
                holes: Vec::new(),
            },
            2,
        )
        .unwrap();
        let mut seen: std::collections::HashMap<(usize, usize), usize> =
            std::collections::HashMap::new();
        for t in &cut.triangles {
            for k in 0..3 {
                *seen.entry(super::key(t[k], t[(k + 1) % 3])).or_default() += 1;
            }
        }
        for ((a, b), times) in &seen {
            assert!(*times <= 2, "edge {a}-{b} belongs to {times} triangles");
            // And no other point lies strictly on it.
            let (p, q) = (cut.points[*a], cut.points[*b]);
            for (i, r) in cut.points.iter().enumerate() {
                if i == *a || i == *b {
                    continue;
                }
                let cross = (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
                let between = (r[0] - p[0]) * (r[0] - q[0]) + (r[1] - p[1]) * (r[1] - q[1]);
                assert!(
                    cross.abs() > 1e-9 || between > -1e-9,
                    "point {r:?} sits inside edge {p:?}-{q:?}"
                );
            }
        }
    }

    #[test]
    fn a_surface_that_does_not_bend_is_not_subdivided() {
        assert_eq!(depth_for(0.0, 3.0, 0.01), 0);
        assert_eq!(depth_for(f64::NAN, 3.0, 0.01), 0);
    }

    #[test]
    fn a_tighter_curve_asks_for_more_subdivision_than_a_looser_one() {
        let tight = depth_for(2.0, std::f64::consts::TAU, 0.02);
        let loose = depth_for(2.0, 0.2, 0.02);
        assert!(tight > loose, "{tight} against {loose}");
        assert!(tight <= super::DEPTH_CEILING);
    }

    #[test]
    fn a_loop_of_fewer_than_three_points_is_refused() {
        assert!(
            tessellate(
                &Patch {
                    outer: vec![[0.0, 0.0], [1.0, 1.0]],
                    holes: Vec::new()
                },
                0
            )
            .is_none()
        );
    }

    #[test]
    fn a_repeated_point_does_not_send_the_clipper_round_for_ever() {
        // Two curves that meet at a vertex can each contribute it.
        let mut ring = square(0.0, 10.0);
        ring.insert(1, [10.0, 0.0]);
        assert!(
            tessellate(
                &Patch {
                    outer: ring,
                    holes: Vec::new()
                },
                0
            )
            .is_some(),
            "a repeated point stalled the clipper"
        );
    }
}
