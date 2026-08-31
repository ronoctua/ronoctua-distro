//! STEP: ISO 10303-21, read for its EDGES.
//!
//! # Why there are no triangles here
//!
//! Because there are none in the file. A STEP part is a boundary
//! representation: planes, cylinders, cones, tori and NURBS surfaces, each
//! trimmed by loops of curves. The file the owner supplied has 106 faces over
//! 42 planes, 32 cylinders, 18 tori and 14 cones, and not one triangle
//! anywhere. Turning that into a surface means, for every face, walking its
//! loops into the surface's own parameter space, unwrapping the seam where the
//! parameter wraps, and triangulating a domain with holes — a geometry kernel.
//! A kernel that handled four of the surface kinds would draw a piece with the
//! other ones missing, which looks like a model rather than like an error.
//!
//! So this reads the one thing that IS written down: the curves. See
//! [`super::Wires`].
//!
//! # Where an arc's ends come from
//!
//! A `CIRCLE` in STEP is a whole circle — centre, radius, and the frame it
//! lies in. What makes it an ARC is the `EDGE_CURVE` that references it, whose
//! two `VERTEX_POINT`s are where it starts and stops, and whose `same_sense`
//! flag says which of the two ways round the edge goes. A reader that drew the
//! circle would put a full ring where a quarter round is, on every fillet in
//! the part.
//!
//! # Why the header is not read as UTF-8
//!
//! Because it is not. Part 21 escapes non-ASCII as `\X2\…\X0\`, and writers
//! ignore that and emit the bytes of whatever code page the machine was in —
//! the owner's file carries `'Windows \xa8\xcf\xa5\xce\xaa\xcc'`, which is Big5
//! for a Chinese phrase. `from_utf8` refuses the whole file over it. So the
//! escapes are decoded and anything left is read as Latin-1, which maps every
//! byte to something rather than to a question mark.

use std::collections::HashMap;

use super::surface::{Frame, cross, dot, unit};
use super::{Census, Read, Shape, Wires};
use crate::Refusal;

/// How many segments a full turn of an arc is drawn with.
///
/// It is a resource limit and not a visual value — the same argument
/// [`super::TRIANGLE_CEILING`] makes. Sixty-four over a full circle is one
/// segment every 5.6 degrees, which at the panel's 512 pixels is under half a
/// pixel of sag on a circle that fills the frame; more would be segments
/// landing inside a pixel that is already drawn.
///
/// Measured on the owner's file: its 251 edges come to 2 621 segments, which
/// the rasteriser draws in 2.3 ms — an order of magnitude under what a mesh of
/// the same part would cost, so there was never a reason to sample less.
const SEGMENTS_PER_TURN: usize = 64;

/// How many segments a spline is drawn with.
const SEGMENTS_PER_SPLINE: usize = 48;

/// How many edges are sampled at all.
///
/// Each one is at most [`SEGMENTS_PER_TURN`] segments of six floats, so this
/// is about 74 MiB at the ceiling. A part with more edges than this is an
/// assembly, and an assembly's wire cage is a grey rectangle either way.
const EDGE_CEILING: usize = 200_000;

/// How deep a curve may delegate to another curve.
const NESTING_CEILING: u32 = 8;

/// One value inside an entity's argument list.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Arg {
    /// `#123`.
    Ref(u64),
    /// A number, integer or real.
    Num(f64),
    /// `'text'`, and whether this reader had to GUESS at any of its bytes.
    ///
    /// The flag is what keeps a mojibake author's name off the panel. Part 21
    /// says a literal is ASCII with `\X2\…\X0\` for anything else, and
    /// writers put raw bytes of whatever code page the machine was in — the
    /// owner's file carries Big5. Those bytes are read as Latin-1, because it
    /// is the only decoding that loses nothing, and the result is not what the
    /// writer meant. A fact the panel is not sure of is not a fact.
    Text(String, bool),
    /// `.T.`, `.UNSPECIFIED.`, and the rest.
    Enum(String),
    /// A parenthesised list.
    List(Vec<Arg>),
    /// `$` — not given — or `*` — inherited.
    Absent,
}

impl Arg {
    /// The entity this points at, if it points at one.
    pub(super) const fn reference(&self) -> Option<u64> {
        match self {
            Self::Ref(n) => Some(*n),
            _ => None,
        }
    }

    /// The number this is, if it is one.
    pub(super) const fn number(&self) -> Option<f64> {
        match self {
            Self::Num(n) => Some(*n),
            _ => None,
        }
    }

    /// The list this is, if it is one.
    pub(super) fn list(&self) -> Option<&[Arg]> {
        match self {
            Self::List(v) => Some(v),
            _ => None,
        }
    }

    /// The text this is, if it is text and this reader is sure of it.
    pub(super) fn text(&self) -> Option<&str> {
        match self {
            Self::Text(t, false) => Some(t),
            _ => None,
        }
    }
}

/// One `#N = …` line. A COMPLEX entity is several names at one number, which
/// is how the format expresses multiple inheritance — the unit declarations
/// are all written that way.
#[derive(Debug, Default)]
pub(super) struct Entity {
    pub(super) parts: Vec<(String, Vec<Arg>)>,
}

impl Entity {
    /// The arguments of the part with this name.
    pub(super) fn part(&self, name: &str) -> Option<&[Arg]> {
        self.parts
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, a)| a.as_slice())
    }

    /// Whether any of its parts has this name.
    pub(super) fn is(&self, name: &str) -> bool {
        self.parts.iter().any(|(n, _)| n == name)
    }

    /// Its first name, which is the one a simple entity has.
    pub(super) fn name(&self) -> &str {
        self.parts.first().map_or("", |(n, _)| n.as_str())
    }
}

/// Everything a STEP file is read into.
pub(super) struct Model {
    pub(super) at: HashMap<u64, Entity>,
}

impl Model {
    pub(super) fn get(&self, r: &Arg) -> Option<&Entity> {
        self.at.get(&r.reference()?)
    }
}

/// Reads a STEP file: the census from the graph, the shape from the faces.
///
/// # Errors
///
/// [`Refusal::Failed`] when the bytes do not open as a part-21 file or hold
/// nothing to draw, and [`Refusal::TooBig`] past [`EDGE_CEILING`].
pub fn read(bytes: &[u8]) -> Result<Read, Refusal> {
    let text = lenient(bytes);
    if !text.trim_start().starts_with("ISO-10303-21") {
        return Err(Refusal::Failed);
    }
    let model = parse(&text)?;
    let (millimetres, unit) = millimetres_per(&model);

    let mut census = census_of(&model);
    // THE FACES, FILLED. Every one this reader cannot mesh comes back as its
    // own outline instead, and `outlined` says how many — see `super::brep`,
    // which is also where the reason for that arrangement is written down.
    let built = super::brep::build(&model, millimetres);
    census.outlined = built.outlined;
    let body = if built.mesh.triangles() == 0 && built.wires.count() == 0 {
        // NO FACES AT ALL. A part-21 file is allowed to hold curves and
        // nothing else, and the cage is the only reading of one.
        super::Body::cage(cage_of(&model, bytes, millimetres)?)
    } else {
        super::Body {
            mesh: built.mesh,
            wires: built.wires,
        }
    };
    if body.nothing() {
        return Err(Refusal::Failed);
    }

    let mut out = Read {
        body,
        shape: Shape::Step,
        unit: unit.map(str::to_owned),
        objects: census.solids.max(1),
        notes: Vec::new(),
        census,
    };
    header(&text, &mut out);
    Ok(out)
}

/// What the FILE says it holds, which is not what the mesher made of it.
///
/// Walked by entity number and not in whatever order the map hands them over:
/// the counts would come out the same either way, but the two functions below
/// share that discipline and one of them needs it.
fn census_of(model: &Model) -> Census {
    let mut census = Census::default();
    let mut ids: Vec<&u64> = model.at.keys().collect();
    ids.sort_unstable();
    let mut touched = std::collections::HashSet::new();
    for entity in ids.into_iter().filter_map(|id| model.at.get(id)) {
        if entity.is("MANIFOLD_SOLID_BREP") || entity.is("BREP_WITH_VOIDS") {
            census.solids += 1;
        }
        if entity.is("CLOSED_SHELL") || entity.is("OPEN_SHELL") {
            census.shells += 1;
        }
        if entity.is("ADVANCED_FACE") || entity.is("FACE_SURFACE") {
            census.faces += 1;
        }
        if entity.is("VERTEX_POINT") {
            census.vertices += 1;
        }
        if let Some(args) = entity.part("EDGE_CURVE") {
            census.edges += 1;
            for slot in [1, 2] {
                if let Some(r) = args.get(slot).and_then(Arg::reference) {
                    touched.insert(r);
                }
            }
        }
    }
    // The distinct vertices an edge actually touched, which is a truer count
    // than every `VERTEX_POINT` in the file — a part carries orphans.
    census.vertices = census.vertices.max(touched.len());
    census
}

/// Every edge of the file, sampled — the drawing for a part-21 file that has
/// no faces to fill.
fn cage_of(model: &Model, bytes: &[u8], scale: f64) -> Result<Wires, Refusal> {
    let mut wires = Wires::empty();
    let mut ids: Vec<&u64> = model.at.keys().collect();
    ids.sort_unstable();
    for entity in ids.into_iter().filter_map(|id| model.at.get(id)) {
        let Some(args) = entity.part("EDGE_CURVE") else {
            continue;
        };
        if wires.count() > EDGE_CEILING * SEGMENTS_PER_TURN {
            return Err(Refusal::TooBig(bytes.len() as u64));
        }
        // `EDGE_CURVE ( name, start_vertex, end_vertex, curve, same_sense )`.
        let (Some(from), Some(to), Some(curve)) = (args.get(1), args.get(2), args.get(3)) else {
            continue;
        };
        let same_sense = !matches!(args.get(4), Some(Arg::Enum(e)) if e == ".F.");
        let (Some(a), Some(b)) = (vertex(model, from), vertex(model, to)) else {
            continue;
        };
        #[allow(clippy::cast_possible_truncation)]
        let scaled: Vec<[f32; 3]> = sample(model, curve, a, b, same_sense, 0)
            .iter()
            .map(|p| {
                [
                    (p[0] * scale) as f32,
                    (p[1] * scale) as f32,
                    (p[2] * scale) as f32,
                ]
            })
            .collect();
        wires.along(&scaled);
    }
    Ok(wires)
}

/// The bytes as text, without refusing the file over its own header.
///
/// Part 21 is an ASCII format with `\X2\…\X0\` for anything else, and writers
/// put raw high bytes in anyway. UTF-8 first, because a modern writer produces
/// it; Latin-1 otherwise, because it maps every byte to a character and
/// therefore never loses one.
fn lenient(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(t) => t.to_owned(),
        Err(_) => bytes.iter().map(|b| char::from(*b)).collect(),
    }
}

/// Every `#N = …;` in the data section.
fn parse(text: &str) -> Result<Model, Refusal> {
    let raw = text.as_bytes();
    let mut at = HashMap::new();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] != b'#' {
            i += 1;
            continue;
        }
        let from = i + 1;
        let mut j = from;
        while j < raw.len() && raw[j].is_ascii_digit() {
            j += 1;
        }
        if j == from {
            i += 1;
            continue;
        }
        // A `#12` inside an argument list is a reference and not a
        // declaration; only one followed by `=` opens an entity.
        let mut k = j;
        while k < raw.len() && (raw[k] as char).is_ascii_whitespace() {
            k += 1;
        }
        if raw.get(k) != Some(&b'=') {
            i = j;
            continue;
        }
        let Ok(id) = text[from..j].parse::<u64>() else {
            i = j;
            continue;
        };
        let (entity, next) = entity(raw, k + 1);
        at.insert(id, entity);
        i = next;
    }
    if at.is_empty() {
        return Err(Refusal::Failed);
    }
    Ok(Model { at })
}

/// One entity's body, from just after its `=` to just after its `;`.
fn entity(raw: &[u8], mut at: usize) -> (Entity, usize) {
    let mut out = Entity::default();
    at = skip(raw, at);
    // A COMPLEX ENTITY is a parenthesised run of named parts, which is how the
    // format writes multiple inheritance — and how every unit is declared.
    let complex = raw.get(at) == Some(&b'(');
    if complex {
        at += 1;
    }
    loop {
        at = skip(raw, at);
        let from = at;
        while at < raw.len()
            && (raw[at].is_ascii_alphanumeric() || raw[at] == b'_' || raw[at] == b'-')
        {
            at += 1;
        }
        if at == from {
            break;
        }
        let name = String::from_utf8_lossy(&raw[from..at]).into_owned();
        at = skip(raw, at);
        if raw.get(at) != Some(&b'(') {
            break;
        }
        let (args, next) = list(raw, at + 1);
        out.parts.push((name, args));
        at = next;
        if !complex {
            break;
        }
    }
    // To the end of the statement.
    while at < raw.len() && raw[at] != b';' {
        at += 1;
    }
    (out, (at + 1).min(raw.len()))
}

/// Whitespace, from `at`.
fn skip(raw: &[u8], mut at: usize) -> usize {
    loop {
        while at < raw.len() && raw[at].is_ascii_whitespace() {
            at += 1;
        }
        // A part-21 comment is `/* … */` and may sit between any two tokens.
        if raw.get(at) == Some(&b'/') && raw.get(at + 1) == Some(&b'*') {
            at += 2;
            while at + 1 < raw.len() && !(raw[at] == b'*' && raw[at + 1] == b'/') {
                at += 1;
            }
            at = (at + 2).min(raw.len());
            continue;
        }
        return at;
    }
}

/// A comma-separated list, from just after its `(` to just after its `)`.
fn list(raw: &[u8], mut at: usize) -> (Vec<Arg>, usize) {
    let mut out = Vec::new();
    loop {
        at = skip(raw, at);
        match raw.get(at) {
            None | Some(b')') => return (out, (at + 1).min(raw.len())),
            Some(b',') => {
                at += 1;
                continue;
            }
            Some(b'(') => {
                let (inner, next) = list(raw, at + 1);
                out.push(Arg::List(inner));
                at = next;
            }
            Some(b'\'') => {
                let (text, guessed, next) = quoted(raw, at + 1);
                out.push(Arg::Text(text, guessed));
                at = next;
            }
            Some(b'#') => {
                let from = at + 1;
                let mut j = from;
                while j < raw.len() && raw[j].is_ascii_digit() {
                    j += 1;
                }
                let n = std::str::from_utf8(&raw[from..j])
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok());
                out.push(n.map_or(Arg::Absent, Arg::Ref));
                at = j;
            }
            Some(b'.') => {
                let from = at;
                at += 1;
                while at < raw.len() && raw[at] != b'.' {
                    at += 1;
                }
                at = (at + 1).min(raw.len());
                out.push(Arg::Enum(
                    String::from_utf8_lossy(&raw[from..at]).into_owned(),
                ));
            }
            Some(b'$' | b'*') => {
                out.push(Arg::Absent);
                at += 1;
            }
            Some(_) => {
                let from = at;
                while at < raw.len()
                    && !matches!(raw[at], b',' | b')' | b'(')
                    && !raw[at].is_ascii_whitespace()
                {
                    at += 1;
                }
                let word = String::from_utf8_lossy(&raw[from..at]);
                // A NUMBER, or a keyword this reader does not care about —
                // `.T.` is handled above, and what reaches here that is not a
                // number is a typed constructor nobody asks for.
                out.push(word.parse::<f64>().map_or(Arg::Absent, Arg::Num));
            }
        }
    }
}

/// A `'…'` literal, with `''` for an apostrophe and `\X2\…\X0\` for anything
/// outside ASCII.
fn quoted(raw: &[u8], mut at: usize) -> (String, bool, usize) {
    let mut out = String::new();
    let mut guessed = false;
    while at < raw.len() {
        match raw[at] {
            b'\'' if raw.get(at + 1) == Some(&b'\'') => {
                out.push('\'');
                at += 2;
            }
            b'\'' => {
                at += 1;
                break;
            }
            b'\\' if raw[at..].starts_with(b"\\X2\\") => {
                at += 4;
                let from = at;
                while at < raw.len() && raw[at].is_ascii_hexdigit() {
                    at += 1;
                }
                let hex = String::from_utf8_lossy(&raw[from..at]).into_owned();
                // UTF-16 big-endian, four hex digits at a time.
                let units: Vec<u16> = hex
                    .as_bytes()
                    .chunks(4)
                    .filter(|c| c.len() == 4)
                    .filter_map(|c| u16::from_str_radix(&String::from_utf8_lossy(c), 16).ok())
                    .collect();
                out.push_str(&String::from_utf16_lossy(&units));
                if raw[at..].starts_with(b"\\X0\\") {
                    at += 4;
                }
            }
            b => {
                // A BYTE THIS FORMAT SAYS CANNOT BE HERE. It is read as
                // Latin-1 so nothing is lost, and marked so nothing is
                // claimed.
                guessed |= b >= 0x80;
                out.push(char::from(b));
                at += 1;
            }
        }
    }
    (out, guessed, at)
}

/// The three numbers of a `CARTESIAN_POINT`, through a `VERTEX_POINT`.
pub(super) fn vertex(model: &Model, r: &Arg) -> Option<[f64; 3]> {
    let e = model.get(r)?;
    if let Some(args) = e.part("VERTEX_POINT") {
        return point(model, args.get(1)?);
    }
    point(model, r)
}

/// The three numbers of a `CARTESIAN_POINT`.
pub(super) fn point(model: &Model, r: &Arg) -> Option<[f64; 3]> {
    let e = model.get(r)?;
    let args = e.part("CARTESIAN_POINT").or_else(|| e.part("DIRECTION"))?;
    let v = args.get(1)?.list()?;
    Some([
        v.first()?.number()?,
        v.get(1).and_then(Arg::number).unwrap_or(0.0),
        v.get(2).and_then(Arg::number).unwrap_or(0.0),
    ])
}

/// A placement's origin and its two named axes, with the third by cross
/// product.
///
/// `AXIS2_PLACEMENT_3D ( name, location, axis, ref_direction )`, where `axis`
/// is the local Z and `ref_direction` the local X. Both are OPTIONAL, and the
/// defaults are the global ones — a file that leaves them out is a file whose
/// arcs would otherwise be drawn in a random plane.
pub(super) fn frame(model: &Model, r: &Arg) -> Option<Frame> {
    let args = model.get(r)?.part("AXIS2_PLACEMENT_3D")?;
    let origin = point(model, args.get(1)?)?;
    let z = args
        .get(2)
        .and_then(|a| point(model, a))
        .unwrap_or([0.0, 0.0, 1.0]);
    let x = args
        .get(3)
        .and_then(|a| point(model, a))
        .unwrap_or([1.0, 0.0, 0.0]);
    let z = unit(z)?;
    // The reference direction is only a HINT: the format allows it to be
    // off-axis, and the true local X is its component perpendicular to Z.
    let d = dot(x, z);
    let x = unit([x[0] - d * z[0], x[1] - d * z[1], x[2] - d * z[2]])?;
    let y = cross(z, x);
    Some(Frame { origin, x, y, z })
}

/// The points along one edge's curve, from `a` to `b`.
///
/// The two ends are always the file's own vertices, so an edge whose curve
/// this reader does not know still joins the two points that ARE written down
/// — a chord where there is an arc, and never a gap in the cage.
pub(super) fn sample(
    model: &Model,
    curve: &Arg,
    a: [f64; 3],
    b: [f64; 3],
    same_sense: bool,
    depth: u32,
) -> Vec<[f64; 3]> {
    let straight = || vec![a, b];
    if depth > NESTING_CEILING {
        return straight();
    }
    let Some(e) = model.get(curve) else {
        return straight();
    };
    match e.name() {
        // A curve that lives on a surface delegates to the one underneath it,
        // which is the shape most STEP writers emit for a trimmed edge.
        "SURFACE_CURVE" | "SEAM_CURVE" | "INTERSECTION_CURVE" | "BOUNDED_CURVE"
        | "TRIMMED_CURVE" => {
            let Some(args) = e.parts.first().map(|(_, a)| a) else {
                return straight();
            };
            match args.get(1) {
                Some(inner) => sample(model, inner, a, b, same_sense, depth + 1),
                None => straight(),
            }
        }
        "LINE" => straight(),
        "CIRCLE" | "ELLIPSE" => {
            let Some(args) = e.parts.first().map(|(_, a)| a) else {
                return straight();
            };
            let Some(f) = args.get(1).and_then(|p| frame(model, p)) else {
                return straight();
            };
            let (origin, ex, ey) = (f.origin, f.x, f.y);
            let (rx, ry) = if e.name() == "CIRCLE" {
                let r = args.get(2).and_then(Arg::number).unwrap_or(0.0);
                (r, r)
            } else {
                (
                    args.get(2).and_then(Arg::number).unwrap_or(0.0),
                    args.get(3).and_then(Arg::number).unwrap_or(0.0),
                )
            };
            if rx <= 0.0 || ry <= 0.0 {
                return straight();
            }
            arc(origin, ex, ey, rx, ry, a, b, same_sense)
        }
        "B_SPLINE_CURVE_WITH_KNOTS" | "RATIONAL_B_SPLINE_CURVE" | "B_SPLINE_CURVE" => {
            spline(model, e).unwrap_or_else(straight)
        }
        _ => straight(),
    }
}

/// The points of an arc between two of its own points.
///
/// # Which of the two ways round
///
/// A `CIRCLE` is a whole circle; what makes it an arc is the edge that uses
/// it. The edge runs from `a` to `b`, and `same_sense` says whether that
/// agrees with the curve's own direction — which is anticlockwise about the
/// frame's Z, from its X. So the sweep is taken in the direction the flag
/// names and wrapped into `(0, 2π]`, and two coincident vertices mean the
/// whole way round.
#[allow(clippy::too_many_arguments)]
pub(super) fn arc(
    origin: [f64; 3],
    ex: [f64; 3],
    ey: [f64; 3],
    rx: f64,
    ry: f64,
    a: [f64; 3],
    b: [f64; 3],
    same_sense: bool,
) -> Vec<[f64; 3]> {
    let angle = |p: [f64; 3]| {
        let v = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
        dot(v, ey).atan2(dot(v, ex))
    };
    let start = angle(a);
    let end = angle(b);
    let full = std::f64::consts::TAU;
    let mut sweep = if same_sense { end - start } else { start - end };
    while sweep <= 1e-9 {
        sweep += full;
    }
    if !same_sense {
        sweep = -sweep;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let steps = ((sweep.abs() / full * SEGMENTS_PER_TURN as f64).ceil() as usize)
        .clamp(2, SEGMENTS_PER_TURN);
    (0..=steps)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let t = start + sweep * (i as f64 / steps as f64);
            let (s, c) = t.sin_cos();
            [
                origin[0] + rx * c * ex[0] + ry * s * ey[0],
                origin[1] + rx * c * ex[1] + ry * s * ey[1],
                origin[2] + rx * c * ex[2] + ry * s * ey[2],
            ]
        })
        .collect()
}

/// A B-spline curve, sampled over its whole knot span.
///
/// The edge's two vertices trim it in principle, and finding where they fall
/// on the curve means inverting it — a search per edge for a difference that
/// is, in every file this has been pointed at, nothing: writers emit a spline
/// that already spans exactly its edge. The whole span is what the file draws.
///
/// # It reads BOTH spellings, and reading one was a defect that showed up
/// three modules away
///
/// A rational B-spline curve is a COMPLEX entity, and the first version of
/// this read only the plain one — so a rational edge came back as `None` and
/// was drawn as the straight CHORD between its two vertices. On the wire
/// drawing that is a visible approximation and little more. On the filled one
/// it is fatal: the chord's points are not on the face's surface, the inverse
/// correctly refuses to place them, and the face falls back to an outline.
/// Measured, that was twenty-eight of the moon buggy's seventy-seven faces —
/// every one of its NURBS patches — and the symptom pointed at the surface
/// code, which was right all along.
fn spline(model: &Model, e: &Entity) -> Option<Vec<[f64; 3]>> {
    let s = super::brep::spline_of(model, e, false)?;
    let ((from, to), _) = s.span();
    if to <= from {
        return None;
    }
    Some(
        (0..=SEGMENTS_PER_SPLINE)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let t = from + (to - from) * (i as f64 / SEGMENTS_PER_SPLINE as f64);
                s.at(t.min(to - 1e-12), 0.0)
            })
            .collect(),
    )
}

pub(super) fn de_boor_3(control: &[[f64; 3]], knots: &[f64], degree: usize, t: f64) -> [f64; 3] {
    // The shared one carries a weight; a non-rational curve's is one.
    let carried: Vec<[f64; 4]> = control.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect();
    let out = super::surface::de_boor(&carried, knots, degree, t);
    [out[0], out[1], out[2]]
}

/// How many millimetres one of the file's length units is.
///
/// The unit is declared as a COMPLEX entity — `( LENGTH_UNIT() NAMED_UNIT(*)
/// SI_UNIT(.MILLI., .METRE.) )` — which is why entities carry a list of parts
/// rather than one name. A file that declares nothing is taken as millimetres,
/// which is what every mechanical STEP in the world is.
fn millimetres_per(model: &Model) -> (f64, Option<&'static str>) {
    // BY ENTITY NUMBER, and not in whatever order the map hands them over: a
    // file may declare the unit twice — the owner's declares it at #162 and
    // again at #379 — and a reader that picked a different one on different
    // runs would draw the same file at two sizes.
    let mut ids: Vec<&u64> = model.at.keys().collect();
    ids.sort_unstable();
    for e in ids.into_iter().filter_map(|id| model.at.get(id)) {
        if !e.is("LENGTH_UNIT") {
            continue;
        }
        if let Some(args) = e.part("SI_UNIT") {
            let prefix = args.first().and_then(Arg::text).map_or_else(
                || match args.first() {
                    Some(Arg::Enum(p)) => p.clone(),
                    _ => String::new(),
                },
                str::to_owned,
            );
            return match prefix.as_str() {
                ".MILLI." => (1.0, Some("millimeter")),
                ".CENTI." => (10.0, Some("centimeter")),
                ".MICRO." => (0.001, Some("micrometer")),
                ".KILO." => (1_000_000.0, Some("kilometer")),
                // No prefix: the metre itself.
                _ => (1000.0, Some("meter")),
            };
        }
        // An imperial file declares a conversion from an SI unit.
        if let Some(args) = e.part("CONVERSION_BASED_UNIT")
            && let Some(name) = args.first().and_then(Arg::text)
        {
            return match name.to_ascii_uppercase().as_str() {
                "INCH" => (25.4, Some("inch")),
                "FOOT" => (304.8, Some("foot")),
                _ => (1.0, None),
            };
        }
    }
    (1.0, None)
}

/// The `HEADER;` section's facts, as notes under the names 3MF uses.
fn header(text: &str, out: &mut Read) {
    let raw = text.as_bytes();
    let take = |name: &str| -> Vec<Arg> {
        let Some(at) = text.find(name) else {
            return Vec::new();
        };
        let mut i = at + name.len();
        while i < raw.len() && raw[i] != b'(' {
            i += 1;
        }
        list(raw, i + 1).0
    };
    let named = take("FILE_NAME");
    // `FILE_NAME ( name, time_stamp, author, organization, preprocessor,
    //             originating_system, authorisation )`.
    let first_of = |a: Option<&Arg>| -> Option<String> {
        match a? {
            Arg::Text(t, false) => Some(t.clone()),
            Arg::List(v) => v.iter().find_map(|x| x.text()).map(str::to_owned),
            _ => None,
        }
    };
    for (value, key) in [
        (first_of(named.first()), "SourceName"),
        (first_of(named.get(1)), "CreationDate"),
        (first_of(named.get(2)), "Designer"),
        (first_of(named.get(5)), "Application"),
    ] {
        if let Some(v) = value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) {
            out.notes.push((key.to_owned(), v));
        }
    }
    if let Some(v) = first_of(take("FILE_SCHEMA").first())
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
    {
        out.notes.push(("Schema".to_owned(), v));
    }
}

#[cfg(test)]
mod tests {
    use super::read;

    /// A part-21 file around the data given.
    fn file(data: &str) -> String {
        format!(
            "ISO-10303-21;\nHEADER;\n\
             FILE_DESCRIPTION (( 'a test' ), '1');\n\
             FILE_NAME ('THING.STEP','2026-08-31T00:00:00',( 'somebody' ),( '' ),\
             'a preprocessor','a system','');\n\
             FILE_SCHEMA (( 'AUTOMOTIVE_DESIGN' ));\nENDSEC;\nDATA;\n{data}\nENDSEC;\n\
             END-ISO-10303-21;\n"
        )
    }

    fn wires(text: &str) -> crate::mesh::Wires {
        read(text.as_bytes()).unwrap().body.wires
    }

    /// Two points, a straight edge between them, and the unit in millimetres.
    const LINE: &str = "\
#1 = CARTESIAN_POINT ( 'NONE', ( 0.0, 0.0, 0.0 ) );
#2 = CARTESIAN_POINT ( 'NONE', ( 10.0, 0.0, 0.0 ) );
#3 = VERTEX_POINT ( 'NONE', #1 );
#4 = VERTEX_POINT ( 'NONE', #2 );
#5 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#6 = VECTOR ( 'NONE', #5, 1.0 );
#7 = LINE ( 'NONE', #1, #6 );
#8 = EDGE_CURVE ( 'NONE', #3, #4, #7, .T. );
";

    #[test]
    fn a_straight_edge_is_two_points_and_not_a_curve() {
        let w = wires(&file(LINE));
        assert_eq!(w.count(), 1);
        assert_eq!(w.bounds.size(), [10.0, 0.0, 0.0]);
    }

    #[test]
    fn an_arc_runs_between_its_two_vertices_and_not_the_whole_circle() {
        // THE DEFECT THIS CATCHES: drawing the CIRCLE the edge references,
        // which puts a full ring where every fillet has a quarter round.
        //
        // A quarter arc of radius 10 in the XY plane, from (10,0) to (0,10).
        let quarter = "\
#1 = CARTESIAN_POINT ( 'NONE', ( 10.0, 0.0, 0.0 ) );
#2 = CARTESIAN_POINT ( 'NONE', ( 0.0, 10.0, 0.0 ) );
#3 = VERTEX_POINT ( 'NONE', #1 );
#4 = VERTEX_POINT ( 'NONE', #2 );
#5 = CARTESIAN_POINT ( 'NONE', ( 0.0, 0.0, 0.0 ) );
#6 = DIRECTION ( 'NONE', ( 0.0, 0.0, 1.0 ) );
#7 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#8 = AXIS2_PLACEMENT_3D ( 'NONE', #5, #6, #7 );
#9 = CIRCLE ( 'NONE', #8, 10.0 );
#10 = EDGE_CURVE ( 'NONE', #3, #4, #9, .T. );
";
        let w = wires(&file(quarter));
        // A quarter of a circle stays in the first quadrant: nothing negative.
        let low = w.bounds.size();
        assert!(w.count() >= 8, "an arc is more than a chord: {}", w.count());
        assert!(
            (low[0] - 10.0).abs() < 0.2 && (low[1] - 10.0).abs() < 0.2,
            "the arc left the quadrant, so the whole circle was drawn: {:?}",
            w.bounds
        );
        for s in w.segments.chunks_exact(3) {
            assert!(
                s[0] >= -0.01 && s[1] >= -0.01,
                "a point outside the quarter: {s:?}"
            );
        }
    }

    #[test]
    fn the_flag_on_the_edge_decides_which_way_round_the_arc_goes() {
        let one_way = "\
#1 = CARTESIAN_POINT ( 'NONE', ( 10.0, 0.0, 0.0 ) );
#2 = CARTESIAN_POINT ( 'NONE', ( 0.0, 10.0, 0.0 ) );
#3 = VERTEX_POINT ( 'NONE', #1 );
#4 = VERTEX_POINT ( 'NONE', #2 );
#5 = CARTESIAN_POINT ( 'NONE', ( 0.0, 0.0, 0.0 ) );
#6 = DIRECTION ( 'NONE', ( 0.0, 0.0, 1.0 ) );
#7 = DIRECTION ( 'NONE', ( 1.0, 0.0, 0.0 ) );
#8 = AXIS2_PLACEMENT_3D ( 'NONE', #5, #6, #7 );
#9 = CIRCLE ( 'NONE', #8, 10.0 );
#10 = EDGE_CURVE ( 'NONE', #3, #4, #9, .F. );
";
        let w = wires(&file(one_way));
        // Going the other way round is three quarters of the circle, so it
        // reaches into the negative quadrants.
        assert!(
            w.segments.chunks_exact(3).any(|s| s[0] < -1.0),
            "the reversed edge did not take the long way: {:?}",
            w.bounds
        );
    }

    #[test]
    fn a_header_that_is_not_utf8_still_gives_the_program_that_wrote_it() {
        // The owner's file carries Big5 bytes inside a quoted string, which
        // `from_utf8` refuses — and refusing the file over its own header is
        // refusing a file whose geometry is perfectly readable.
        let mut bytes = file(LINE).into_bytes();
        let at = crate::mesh::find(&bytes, b"'somebody'").expect("the fixture has an author");
        bytes.splice(at..at + 10, b"'\xa8\xcf\xa5\xce\xaa\xcc'".iter().copied());
        let out = read(&bytes).expect("a file with a Big5 header still reads");
        assert!(
            out.notes
                .iter()
                .any(|(k, v)| k == "Application" && v == "a system"),
            "{:?}",
            out.notes
        );
        // AND THE FIELD THIS READER HAD TO GUESS AT IS WITHHELD. Latin-1 over
        // Big5 gives `Windows ¨Ï¥Î ªÌ`, which on the panel reads as a defect
        // in the file manager rather than as what the file says.
        assert!(
            !out.notes.iter().any(|(k, _)| k == "Designer"),
            "a guessed author reached the panel: {:?}",
            out.notes
        );
        assert!(
            out.notes
                .iter()
                .any(|(k, v)| k == "Schema" && v == "AUTOMOTIVE_DESIGN")
        );
        assert!(
            out.notes
                .iter()
                .any(|(k, v)| k == "SourceName" && v == "THING.STEP")
        );
    }

    #[test]
    fn a_complex_entity_declares_the_unit_and_it_is_applied() {
        // The unit is only ever written as a complex entity, which is why an
        // entity holds a LIST of named parts.
        let inches =
            format!("{LINE}#20 = ( LENGTH_UNIT ( ) NAMED_UNIT ( * ) SI_UNIT ( $, .METRE. ) );\n");
        let w = wires(&file(&inches));
        assert!(
            (w.bounds.size()[0] - 10_000.0).abs() < 1.0,
            "a file in metres was not scaled: {:?}",
            w.bounds
        );
    }

    #[test]
    fn the_census_counts_what_the_file_holds() {
        let solid = format!(
            "{LINE}\
#30 = ADVANCED_FACE ( 'NONE', ( ), #31, .T. );
#32 = CLOSED_SHELL ( 'NONE', ( #30 ) );
#33 = MANIFOLD_SOLID_BREP ( 'a part', #32 );
"
        );
        let out = read(file(&solid).as_bytes()).unwrap();
        assert_eq!(out.census.solids, 1);
        assert_eq!(out.census.shells, 1);
        assert_eq!(out.census.faces, 1);
        assert_eq!(out.census.edges, 1);
        assert_eq!(out.census.vertices, 2);
    }

    #[test]
    fn a_file_that_does_not_open_as_part_21_is_refused() {
        assert!(read(b"solid something\nendsolid\n").is_err());
    }

    #[test]
    fn a_part_21_file_with_no_edge_in_it_is_refused() {
        assert!(
            read(file("#1 = CARTESIAN_POINT ( 'NONE', ( 0.0, 0.0, 0.0 ) );").as_bytes()).is_err()
        );
    }
}
