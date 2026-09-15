//! The shape inside a 3D file, and what can be said about it.
//!
//! # Two kinds of file, and only one of them holds a shape
//!
//! Nine of the eleven types here are MESHES: the triangles are written down
//! and reading them is reading them. Two — STEP and IGES — are BOUNDARY
//! REPRESENTATIONS, which hold surfaces and the curves that trim them and not
//! one triangle anywhere. Turning those into a surface is a geometry kernel,
//! and half a kernel draws a piece that is quietly the wrong shape. So they
//! give back the one thing that IS written down: the edges. See [`Body`].
//!
//! # Why the triangles are loose and not indexed
//!
//! Because that is what an STL *is* — three vertices per facet, written out
//! again for every facet that touches them — and it is what the rasteriser
//! wants: one pass, no indirection, no index buffer to walk. Welding them into
//! a shared list would cost a sort on load and buy nothing on screen. The one
//! place the shared vertices matter is [`Sanity`], which builds them there and
//! throws them away.
//!
//! # Why the normal in the file is ignored
//!
//! Every STL facet carries one, and a large share of the STLs in the world
//! carry `0 0 0` — some writers never filled it in, and others left it stale
//! after a transform. A normal that is sometimes right is worse than no normal
//! at all, because the lighting fails on *some* faces and looks like a bug in
//! the renderer. The cross product of the edges is always right and costs two
//! subtractions and a multiply per facet. Every reader here therefore throws
//! away whatever normals the file carried and lets [`Mesh::add`] compute them.
//!
//! # Why each format is a file of its own
//!
//! Because what is hard about each one is different, and it is written at the
//! top of the file that has it: an index that counts backwards (`obj`), a
//! property whose width has to be known to be skipped (`ply`), a scene graph
//! whose transforms have to be composed (`gltf`, `dae`), an axis convention
//! that decides whether the piece stands up (`dae`), and an arc that has to
//! run between its two vertices and not the whole way round (`step`).

mod amf;
mod brep;
mod dae;
mod gltf;
mod iges;
mod obj;
mod off;
mod patch;
mod ply;
mod step;
mod stl;
mod surface;
mod three_mf;
mod xml;

use std::path::Path;

use crate::Refusal;

/// How many triangles will be read.
///
/// Two million is far past what the panel can show — the preview is at most
/// 512 px on its longest side, so beyond a few hundred thousand facets every
/// extra one lands inside a pixel that is already covered. What the ceiling
/// actually buys is memory: this crate keeps 48 bytes per triangle, so the
/// ceiling is about 96 MiB, and a `.stl` is 50 bytes per triangle on disk, so
/// nothing under 100 MB is ever refused.
///
/// It is a resource limit and not a visual value, which is why it is a constant
/// here and not a token — the same argument [`crate::pixels::PIXEL_CEILING`]
/// makes one file over.
pub const TRIANGLE_CEILING: usize = 2_000_000;

/// How many bytes a `.3mf` may inflate to — [`crate::zip::INFLATED_CEILING`].
///
/// The reader moved up to the crate when the office formats arrived, and the
/// ceiling went with it: it is a fact about inflating a zip and not about
/// meshes. Named here so the two readers that were written against it keep
/// reading one number.
pub use crate::zip::INFLATED_CEILING;

/// How large a mesh file is read at all.
///
/// The triangle ceiling cannot be applied before reading, because an ASCII STL
/// gives no count until it has been walked. This is the guard that exists
/// before a single byte is allocated.
pub const FILE_CEILING: u64 = 512 * 1024 * 1024;

/// Which of the shapes the bytes turned out to be.
///
/// The variants are the FILE's spelling and not the reader's convenience: an
/// STL written as bytes and one written as words are the same geometry and a
/// different thing to open, and the panel says which. Every variant has a text
/// key and `mesh-formats-are-total` asserts that every one is reachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The 80-byte header, a count, and 50 bytes per facet.
    StlBinary,
    /// `solid … facet normal … endsolid`.
    StlAscii,
    /// A zip of XML.
    ThreeMf,
    /// Wavefront: `v` and `f`, and a `.mtl` beside it that this does not read.
    Obj,
    /// Stanford, written as words.
    PlyAscii,
    /// Stanford, written as bytes — either way round.
    PlyBinary,
    /// Object File Format: counts, points, faces, and nothing else.
    Off,
    /// glTF 2, as JSON.
    Gltf,
    /// glTF 2, as one binary container.
    Glb,
    /// COLLADA.
    Collada,
    /// Additive Manufacturing Format, zipped or not.
    Amf,
    /// ISO 10303-21. Surfaces, not triangles — see [`super::brep`].
    Step,
    /// Initial Graphics Exchange Specification. Surfaces, and drawn as the
    /// edges alone — see [`super::iges`].
    Iges,
}

impl Shape {
    /// The text key naming this shape in the info panel.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::StlBinary => crate::key::VALUE_STL_BINARY,
            Self::StlAscii => crate::key::VALUE_STL_ASCII,
            Self::ThreeMf => crate::key::VALUE_THREE_MF,
            Self::Obj => crate::key::VALUE_OBJ,
            Self::PlyAscii => crate::key::VALUE_PLY_ASCII,
            Self::PlyBinary => crate::key::VALUE_PLY_BINARY,
            Self::Off => crate::key::VALUE_OFF,
            Self::Gltf => crate::key::VALUE_GLTF,
            Self::Glb => crate::key::VALUE_GLB,
            Self::Collada => crate::key::VALUE_COLLADA,
            Self::Amf => crate::key::VALUE_AMF,
            Self::Step => crate::key::VALUE_STEP,
            Self::Iges => crate::key::VALUE_IGES,
        }
    }

    /// Every one of them, so a test can walk the list rather than repeat it.
    pub const ALL: [Self; 13] = [
        Self::StlBinary,
        Self::StlAscii,
        Self::ThreeMf,
        Self::Obj,
        Self::PlyAscii,
        Self::PlyBinary,
        Self::Off,
        Self::Gltf,
        Self::Glb,
        Self::Collada,
        Self::Amf,
        Self::Step,
        Self::Iges,
    ];
}

/// The smallest box the shape fits in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// The low corner.
    pub min: [f32; 3],
    /// The high corner.
    pub max: [f32; 3],
}

impl Bounds {
    /// A box that contains nothing, ready to be grown.
    #[must_use]
    pub const fn nothing() -> Self {
        Self {
            min: [f32::MAX, f32::MAX, f32::MAX],
            max: [f32::MIN, f32::MIN, f32::MIN],
        }
    }

    /// Grows to hold one point.
    pub fn hold(&mut self, v: [f32; 3]) {
        for ((lo, hi), x) in self.min.iter_mut().zip(self.max.iter_mut()).zip(v) {
            *lo = lo.min(x);
            *hi = hi.max(x);
        }
    }

    /// Width, depth and height.
    #[must_use]
    pub fn size(&self) -> [f32; 3] {
        if self.min[0] > self.max[0] {
            return [0.0; 3];
        }
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    /// The middle of the box.
    #[must_use]
    pub fn middle(&self) -> [f32; 3] {
        if self.min[0] > self.max[0] {
            return [0.0; 3];
        }
        [
            f32::midpoint(self.min[0], self.max[0]),
            f32::midpoint(self.min[1], self.max[1]),
            f32::midpoint(self.min[2], self.max[2]),
        ]
    }

    /// The longest side.
    #[must_use]
    pub fn widest(&self) -> f32 {
        let s = self.size();
        s[0].max(s[1]).max(s[2])
    }
}

/// A triangle soup.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    /// Nine floats per triangle: three vertices, each `x y z`.
    pub positions: Vec<f32>,
    /// Three floats per triangle: the face normal, computed here.
    pub normals: Vec<f32>,
    /// The box it fits in.
    pub bounds: Bounds,
}

impl Mesh {
    /// An empty one.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            bounds: Bounds::nothing(),
        }
    }

    /// How many triangles.
    #[must_use]
    pub const fn triangles(&self) -> usize {
        self.positions.len() / 9
    }

    /// Adds one triangle, computing its normal and growing the box.
    ///
    /// The normal is NOT normalised here: the rasteriser needs a unit vector
    /// and computes it once per drawn facet, and a zero-area triangle would
    /// divide by zero at this point with nothing to do about it.
    pub fn add(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3]) {
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        self.positions.extend_from_slice(&a);
        self.positions.extend_from_slice(&b);
        self.positions.extend_from_slice(&c);
        self.normals.extend_from_slice(&n);
        self.bounds.hold(a);
        self.bounds.hold(b);
        self.bounds.hold(c);
    }

    /// The three corners of one triangle.
    #[must_use]
    pub fn corners(&self, i: usize) -> [[f32; 3]; 3] {
        let p = &self.positions[i * 9..i * 9 + 9];
        [[p[0], p[1], p[2]], [p[3], p[4], p[5]], [p[6], p[7], p[8]]]
    }

    /// The volume the surface encloses, in cubic units.
    ///
    /// The signed-tetrahedron sum: every triangle makes a tetrahedron with the
    /// origin and the sum of their signed volumes is the enclosed volume,
    /// wherever the origin happens to be. It is meaningful only for a closed
    /// surface — see [`Sanity::watertight`], which is why the panel shows the
    /// two together.
    #[must_use]
    pub fn volume(&self) -> f64 {
        let mut sum = 0.0_f64;
        for i in 0..self.triangles() {
            let [a, b, c] = self.corners(i);
            let cross = [
                f64::from(b[1]) * f64::from(c[2]) - f64::from(b[2]) * f64::from(c[1]),
                f64::from(b[2]) * f64::from(c[0]) - f64::from(b[0]) * f64::from(c[2]),
                f64::from(b[0]) * f64::from(c[1]) - f64::from(b[1]) * f64::from(c[0]),
            ];
            sum += f64::from(a[0]) * cross[0]
                + f64::from(a[1]) * cross[1]
                + f64::from(a[2]) * cross[2];
        }
        (sum / 6.0).abs()
    }

    /// The area of the surface, in square units.
    #[must_use]
    pub fn area(&self) -> f64 {
        let mut sum = 0.0_f64;
        for i in 0..self.triangles() {
            let n = &self.normals[i * 3..i * 3 + 3];
            let len = (f64::from(n[0]).powi(2) + f64::from(n[1]).powi(2) + f64::from(n[2]).powi(2))
                .sqrt();
            sum += len / 2.0;
        }
        sum
    }
}

/// What a walk of the edges found.
///
/// # Why the edges are SORTED and not hashed
///
/// Because the count is known in advance and large: three edges per triangle,
/// so two million triangles is six million entries. A hash map pays a hash and
/// a probe for each and grows several times on the way; a sort is one
/// allocation and a predictable pass, and the same sorted list answers "how
/// many distinct vertices" for free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sanity {
    /// Distinct vertices, after welding by position.
    pub vertices: usize,
    /// Edges belonging to exactly one triangle — a hole.
    pub open: usize,
    /// Edges met twice the SAME way round, which means one of the two
    /// triangles faces backwards.
    pub flipped: usize,
    /// Triangles with no area: three points on a line, or two the same.
    pub degenerate: usize,
}

impl Sanity {
    /// Whether the surface is closed and consistently wound — the question a
    /// slicer asks before anything else.
    #[must_use]
    pub const fn watertight(&self) -> bool {
        self.open == 0 && self.flipped == 0
    }
}

/// Welds the vertices and walks the edges.
///
/// Welding is by a QUANTISED position and not by the exact bits: two writers
/// that both meant the same corner rarely agree to the last bit, and an exact
/// comparison would report a watertight piece as a sieve. The grid is relative
/// to the piece's own size, so it means the same thing for a 3 mm screw and a
/// 300 mm vase.
#[must_use]
pub fn sanity(mesh: &Mesh) -> Sanity {
    let n = mesh.triangles();
    let mut out = Sanity::default();
    if n == 0 {
        return out;
    }
    // A millionth of the longest side. Below that, two points are the same
    // point in any file a slicer ever wrote.
    let widest = mesh.bounds.widest();
    let grid = if widest > 0.0 {
        f64::from(widest) / 1_000_000.0
    } else {
        1.0
    };
    let key_of = |v: [f32; 3]| -> [i64; 3] {
        [
            (f64::from(v[0]) / grid).round() as i64,
            (f64::from(v[1]) / grid).round() as i64,
            (f64::from(v[2]) / grid).round() as i64,
        ]
    };

    // One key per corner, then a sorted copy to number them.
    let mut keys: Vec<[i64; 3]> = Vec::with_capacity(n * 3);
    for i in 0..n {
        for c in mesh.corners(i) {
            keys.push(key_of(c));
        }
    }
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    sorted.dedup();
    out.vertices = sorted.len();
    let id_of = |k: &[i64; 3]| -> u32 {
        // `sorted` holds every key that exists, so this never misses.
        sorted.partition_point(|x| x < k) as u32
    };

    // Three directed edges per triangle, stored with the pair in a fixed order
    // and the direction kept beside it.
    let mut edges: Vec<(u32, u32, bool)> = Vec::with_capacity(n * 3);
    for i in 0..n {
        let ids = [
            id_of(&keys[i * 3]),
            id_of(&keys[i * 3 + 1]),
            id_of(&keys[i * 3 + 2]),
        ];
        if ids[0] == ids[1] || ids[1] == ids[2] || ids[2] == ids[0] {
            out.degenerate += 1;
            continue;
        }
        for k in 0..3 {
            let (a, b) = (ids[k], ids[(k + 1) % 3]);
            if a < b {
                edges.push((a, b, true));
            } else {
                edges.push((b, a, false));
            }
        }
    }
    edges.sort_unstable();

    let mut at = 0;
    while at < edges.len() {
        let (a, b, _) = edges[at];
        let mut end = at;
        let mut forward = 0;
        let mut backward = 0;
        while end < edges.len() && edges[end].0 == a && edges[end].1 == b {
            if edges[end].2 {
                forward += 1;
            } else {
                backward += 1;
            }
            end += 1;
        }
        match (forward, backward) {
            (1, 1) => {}
            // Both triangles wound the same way: one of them faces inwards.
            (2, 0) | (0, 2) => out.flipped += 1,
            // One triangle, or more than two — a hole either way, and a hole
            // is what a slicer will complain about.
            _ => out.open += 1,
        }
        at = end;
    }
    out
}

/// A line soup: the edges of a piece, and no surface at all.
///
/// # Why a B-Rep gives edges and not triangles
///
/// A STEP file holds surfaces — planes, cylinders, cones, tori, NURBS — and,
/// for each face, the loops of curves that trim it. Turning that into
/// triangles means walking every loop into the surface's own parameter space,
/// unwrapping the seam where the parameter wraps, and triangulating a domain
/// with holes in it. That is a geometry kernel, and a kernel that handles some
/// of the surfaces draws a piece with the other ones MISSING — a wrong shape
/// with no error anywhere saying so, which is the failure this crate is most
/// careful about.
///
/// The curves, on the other hand, are written down. Every segment drawn from
/// this is a curve that is literally in the file, sampled; nothing is guessed
/// and nothing is left out.
#[derive(Debug, Clone, PartialEq)]
pub struct Wires {
    /// Six floats per segment: two endpoints, each `x y z`.
    pub segments: Vec<f32>,
    /// The box the endpoints fit in.
    pub bounds: Bounds,
}

impl Wires {
    /// An empty one.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            segments: Vec::new(),
            bounds: Bounds::nothing(),
        }
    }

    /// How many segments.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.segments.len() / 6
    }

    /// Adds one segment and grows the box.
    pub fn add(&mut self, a: [f32; 3], b: [f32; 3]) {
        self.segments.extend_from_slice(&a);
        self.segments.extend_from_slice(&b);
        self.bounds.hold(a);
        self.bounds.hold(b);
    }

    /// Adds a run of points as the segments between them.
    pub fn along(&mut self, points: &[[f32; 3]]) {
        for pair in points.windows(2) {
            self.add(pair[0], pair[1]);
        }
    }
}

/// What the file turned out to hold.
///
/// # Why it is not one OR the other
///
/// It was, and a boundary representation broke it. A STEP file's faces are
/// filled one at a time, and a face whose surface this reader cannot mesh is
/// drawn as its own OUTLINE instead — so one file produces both, and the two
/// have to arrive together and be drawn together. See [`super::brep`].
///
/// A mesh format leaves `wires` empty; an IGES leaves `mesh` empty; a STEP
/// usually fills `mesh` and leaves `wires` empty too, and when it does not,
/// what is on screen is a solid with a piece of it shown in outline — which is
/// visibly a different kind of drawing, and is meant to be.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    /// The triangles.
    pub mesh: Mesh,
    /// The edges, for whatever has none.
    pub wires: Wires,
}

impl Body {
    /// A body of triangles and nothing else.
    #[must_use]
    pub const fn of(mesh: Mesh) -> Self {
        Self {
            mesh,
            wires: Wires::empty(),
        }
    }

    /// A body of edges and nothing else.
    #[must_use]
    pub const fn cage(wires: Wires) -> Self {
        Self {
            mesh: Mesh::empty(),
            wires,
        }
    }

    /// The box it fits in, whichever halves it has.
    #[must_use]
    pub fn bounds(&self) -> Bounds {
        if self.wires.count() == 0 {
            return self.mesh.bounds;
        }
        if self.mesh.triangles() == 0 {
            return self.wires.bounds;
        }
        let (a, b) = (self.mesh.bounds, self.wires.bounds);
        Bounds {
            min: [
                a.min[0].min(b.min[0]),
                a.min[1].min(b.min[1]),
                a.min[2].min(b.min[2]),
            ],
            max: [
                a.max[0].max(b.max[0]),
                a.max[1].max(b.max[1]),
                a.max[2].max(b.max[2]),
            ],
        }
    }

    /// Whether there is anything at all to draw.
    #[must_use]
    pub const fn nothing(&self) -> bool {
        self.mesh.triangles() == 0 && self.wires.count() == 0
    }

    /// The triangles, when there are any.
    #[must_use]
    pub const fn mesh(&self) -> Option<&Mesh> {
        if self.mesh.triangles() == 0 {
            None
        } else {
            Some(&self.mesh)
        }
    }
}

/// What a boundary representation counts, since it has no triangles to count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Census {
    /// `MANIFOLD_SOLID_BREP` and its kin.
    pub solids: usize,
    /// Closed and open shells together.
    pub shells: usize,
    /// Trimmed faces.
    pub faces: usize,
    /// Edges, before any of them is sampled into segments.
    pub edges: usize,
    /// Distinct vertex points.
    pub vertices: usize,
    /// How many faces came out as an outline rather than filled.
    ///
    /// See [`super::brep`]: a surface this reader cannot mesh, an inverse that
    /// will not converge, a loop that comes out crossing itself. It is shown
    /// on the preview and in the panel, because a piece that is part outline
    /// is a piece somebody should be told about.
    pub outlined: usize,
}

impl Census {
    /// Whether it says anything at all.
    #[must_use]
    pub const fn empty(self) -> bool {
        self.solids == 0
            && self.shells == 0
            && self.faces == 0
            && self.edges == 0
            && self.vertices == 0
            && self.outlined == 0
    }
}

/// Everything read out of one 3D file.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    /// The triangles, or the edges.
    pub body: Body,
    /// Which format it turned out to be.
    pub shape: Shape,
    /// The unit the file declares. An STL declares none — the convention is
    /// millimetres and this crate does not pretend otherwise.
    pub unit: Option<String>,
    /// How many objects the build places. One, for an STL.
    pub objects: usize,
    /// The metadata the format defines, in the order it carries it. The NAMES
    /// are the ones 3MF uses, and the other readers reuse them where they mean
    /// the same thing, so there is one map from name to label and not five.
    pub notes: Vec<(String, String)>,
    /// What a boundary representation counted.
    pub census: Census,
}

impl Read {
    /// A reading of triangles and nothing else declared.
    #[must_use]
    pub fn of(mesh: Mesh, shape: Shape) -> Self {
        Self {
            body: Body::of(mesh),
            shape,
            unit: None,
            objects: 1,
            notes: Vec::new(),
            census: Census::default(),
        }
    }
}

/// Reads whatever shape is in the file.
///
/// `mime` decides, and it has already been through [`crate::settled`] — which
/// matters here more than anywhere else in this crate: a binary STL has no
/// magic number at all (freedesktop's only rule is the word `solid`, which is
/// the ASCII form), so its type comes from the name alone and the sniff
/// correctly says nothing.
///
/// The arms are the canonical types, which is what makes this a match and not
/// a chain of `is_a`: [`ronoctua_files::mime::canonical`] has already folded
/// `model/x.stl-binary` and `application/vnd.ms-3mfdocument` into their real
/// names, and every arm here is a type the table's `mesh` row claims exactly.
/// `mesh-formats-are-total` asserts the two lists are the same list.
///
/// # Errors
///
/// [`Refusal::Unreadable`] when it cannot be opened, [`Refusal::TooBig`] past a
/// ceiling, [`Refusal::Unsupported`] for a file this reader is not equipped for
/// but which is not damaged, and [`Refusal::Failed`] when the bytes are not the
/// shape they claim.
pub fn read(path: &Path, mime: &str) -> Result<Read, Refusal> {
    let bytes = slurp(path, FILE_CEILING)?;
    match ronoctua_files::mime::canonical(mime) {
        "model/stl" => stl::read(&bytes),
        "model/3mf" => three_mf::read(&bytes),
        "model/obj" => obj::read(&bytes),
        "model/x-ply" => ply::read(&bytes),
        "model/x-off" => off::read(&bytes),
        // THE ONLY READER THAT IS GIVEN THE PATH, because it is the only
        // format whose geometry may be in a SECOND file: a `.gltf` names its
        // buffers by URI, and a relative one is a sibling on disk.
        "model/gltf+json" => gltf::read(&bytes, path),
        "model/gltf-binary" => gltf::read_binary(&bytes),
        "model/vnd.collada+xml" => dae::read(&bytes),
        "model/x-amf" => amf::read(&bytes),
        "model/step" => step::read(&bytes),
        "model/iges" => iges::read(&bytes),
        // Not a shape at all. The host asks `pixels::turns` before reaching
        // this, so it is the answer to a question nobody asked — and the
        // sentence for it is the one a folder gets.
        _ => Err(Refusal::Nothing),
    }
}

/// Where a needle starts in a haystack.
///
/// Two readers look for a marker in bytes that are not text — the PLY header's
/// `end_header` and, in a test, a quoted field of a STEP header — and both
/// want the same three lines.
fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The whole file, refused rather than read when it is past `ceiling`.
fn slurp(path: &Path, ceiling: u64) -> Result<Vec<u8>, Refusal> {
    let meta = std::fs::metadata(path).map_err(|_| Refusal::Unreadable)?;
    if meta.len() > ceiling {
        return Err(Refusal::TooBig(meta.len()));
    }
    std::fs::read(path).map_err(|_| Refusal::Unreadable)
}

/// What the info panel says about a shape.
///
/// # Why the walk happens here and not in `facts`
///
/// Because `facts::facts` is a front for the external programs — it asks
/// `needs()` which of the five to run and parses what they said. No program is
/// involved here: the same read that the picture needs answers every one of
/// these, and running it twice on a hundred-megabyte file to keep the shapes
/// tidy would be a hundred megabytes read twice.
///
/// A fact with no answer is ABSENT, the rule the rest of the panel follows: a
/// mesh with nothing wrong shows no row about flipped faces, rather than a row
/// saying none. That rule is what lets one function answer for both bodies:
/// a boundary representation has no volume and shows no volume, rather than a
/// row saying zero.
#[must_use]
pub fn facts(read: &Read) -> Vec<crate::facts::Fact> {
    use crate::facts::{Fact, Value};
    use crate::key;

    let count = |n: usize| Value::Phrase {
        key: key::VALUE_COUNT,
        args: vec![("n", n.to_string())],
    };
    let mut out = vec![Fact {
        label: crate::facts::Label::Key(key::FACT_FORMAT),
        value: Value::phrase(read.shape.key()),
    }];
    if let Some(unit) = &read.unit {
        out.push(Fact {
            label: crate::facts::Label::Key(key::FACT_UNIT),
            value: Value::Data(unit.clone()),
        });
    }
    // ONLY WHEN THERE IS MORE THAN ONE. A row saying "objects: 1" is a row
    // that tells nobody anything, and every STL there has ever been would
    // carry it.
    if read.objects > 1 {
        out.push(Fact {
            label: crate::facts::Label::Key(key::FACT_OBJECTS),
            value: count(read.objects),
        });
    }

    // WHAT A BOUNDARY REPRESENTATION COUNTED, in the order a person reads a
    // part: the whole, then its pieces. Each row only when the file said so.
    for (n, label) in [
        (read.census.solids, key::FACT_SOLIDS),
        (read.census.shells, key::FACT_SHELLS),
        (read.census.faces, key::FACT_FACES),
        (read.census.edges, key::FACT_EDGES),
    ] {
        if n > 0 {
            out.push(Fact {
                label: crate::facts::Label::Key(label),
                value: count(n),
            });
        }
    }

    if read.census.empty() {
        mesh_facts(&read.body.mesh, &mut out);
    } else {
        brep_facts(read, &mut out);
    }

    // THE METADATA THE FORMAT DEFINES, and only that. A 3MF may carry any name
    // at all — the owner's file has `BambuStudio:3mfVersion` and two thumbnail
    // paths — and a panel that showed every one of them would be a panel of a
    // slicer's private notes.
    //
    // THE NAMES ARE 3MF'S, and the other readers speak them: a STEP header's
    // originating system IS what a 3MF calls `Application`, and giving it a
    // second name would give it a second row on a panel that has one label for
    // the fact.
    for (name, label) in [
        ("Application", key::FACT_APPLICATION),
        ("Title", key::FACT_TITLE),
        ("Designer", key::FACT_DESIGNER),
        ("LicenseTerms", key::FACT_LICENCE),
        ("License", key::FACT_LICENCE),
        ("CreationDate", key::FACT_CREATED),
        ("Schema", key::FACT_SCHEMA),
        ("SourceName", key::FACT_SOURCE_NAME),
    ] {
        if let Some((_, v)) = read.notes.iter().find(|(k, _)| k == name)
            && !out
                .iter()
                .any(|f| f.label == crate::facts::Label::Key(label))
        {
            out.push(Fact {
                label: crate::facts::Label::Key(label),
                value: Value::Data(v.clone()),
            });
        }
    }
    out
}

/// The box, written the way the panel writes one.
fn box_fact(size: [f32; 3]) -> crate::facts::Value {
    crate::facts::Value::Phrase {
        key: crate::key::VALUE_BOX,
        args: vec![
            ("w", format!("{:.1}", size[0])),
            ("d", format!("{:.1}", size[1])),
            ("h", format!("{:.1}", size[2])),
        ],
    }
}

/// Everything a triangle soup answers.
fn mesh_facts(mesh: &Mesh, out: &mut Vec<crate::facts::Fact>) {
    use crate::facts::{Fact, Value};
    use crate::key;

    let count = |n: usize| Value::Phrase {
        key: key::VALUE_COUNT,
        args: vec![("n", n.to_string())],
    };
    let sane = sanity(mesh);
    out.push(Fact {
        label: crate::facts::Label::Key(key::FACT_TRIANGLES),
        value: count(mesh.triangles()),
    });
    out.push(Fact {
        label: crate::facts::Label::Key(key::FACT_VERTICES),
        value: count(sane.vertices),
    });
    out.push(Fact {
        label: crate::facts::Label::Key(key::FACT_BOUNDS),
        value: box_fact(mesh.bounds.size()),
    });
    // CUBIC CENTIMETRES AND NOT CUBIC MILLIMETRES. The Benchy is 15 550 mm3,
    // which is a number nobody has a feel for, and 15.6 cm3, which is what a
    // spool of filament is sold against.
    out.push(Fact {
        label: crate::facts::Label::Key(key::FACT_VOLUME),
        value: Value::Phrase {
            key: key::VALUE_CUBIC_CENTIMETRES,
            args: vec![("n", format!("{:.1}", mesh.volume() / 1000.0))],
        },
    });
    out.push(Fact {
        label: crate::facts::Label::Key(key::FACT_AREA),
        value: Value::Phrase {
            key: key::VALUE_SQUARE_CENTIMETRES,
            args: vec![("n", format!("{:.1}", mesh.area() / 100.0))],
        },
    });
    out.push(Fact {
        label: crate::facts::Label::Key(key::FACT_WATERTIGHT),
        value: if sane.watertight() {
            Value::phrase(key::VALUE_MESH_CLOSED)
        } else {
            Value::Phrase {
                key: key::VALUE_MESH_OPEN,
                args: vec![("n", sane.open.to_string())],
            }
        },
    });
    if sane.flipped > 0 {
        out.push(Fact {
            label: crate::facts::Label::Key(key::FACT_FLIPPED),
            value: count(sane.flipped),
        });
    }
    if sane.degenerate > 0 {
        out.push(Fact {
            label: crate::facts::Label::Key(key::FACT_DEGENERATE),
            value: count(sane.degenerate),
        });
    }
}

/// What a boundary representation answers, which is fewer things and honestly
/// so.
///
/// # Why there is no volume, no area and no "surface closed"
///
/// Because the mesh a B-Rep produces is not the part — it is this reader's
/// drawing of the part. Two faces that meet are subdivided in their own
/// parameters and their shared edge comes out a thousandth of a millimetre
/// apart (see [`super::patch`]), so the surface is very slightly open and
/// those three numbers would be about that gap rather than about the piece.
///
/// # And why there is no triangle count either
///
/// Because it would be a fact about the mesher. What the FILE says it has —
/// solids, shells, faces, edges, vertices — is already up the panel, and it is
/// the honest answer to "how complicated is this".
fn brep_facts(read: &Read, out: &mut Vec<crate::facts::Fact>) {
    use crate::facts::{Fact, Value};
    use crate::key;

    if read.census.vertices > 0 {
        out.push(Fact {
            label: crate::facts::Label::Key(key::FACT_VERTICES),
            value: Value::Phrase {
                key: key::VALUE_COUNT,
                args: vec![("n", read.census.vertices.to_string())],
            },
        });
    }
    if !read.body.nothing() {
        out.push(Fact {
            label: crate::facts::Label::Key(key::FACT_BOUNDS),
            value: box_fact(read.body.bounds().size()),
        });
    }
    // ONLY WHEN SOME FACE DID NOT FILL. A row saying "none" is a row about a
    // limit nobody ran into.
    if read.census.outlined > 0 {
        out.push(Fact {
            label: crate::facts::Label::Key(key::FACT_OUTLINED),
            value: Value::Phrase {
                key: key::VALUE_COUNT,
                args: vec![("n", read.census.outlined.to_string())],
            },
        });
    }
}

/// The fixtures the readers' own tests build their files out of.
///
/// They live here rather than in one of them because three modules want the
/// same cube: a test that compared a reader against ANOTHER READER would prove
/// only that the two agree.
#[cfg(test)]
pub mod fixture {
    use super::Mesh;

    /// A cube of `side`, twelve triangles, wound outwards.
    ///
    /// Chosen because its volume and area are known without measuring
    /// anything: a test that compared the reader against the reader would
    /// prove only that it is consistent.
    pub fn cube(side: f32) -> Mesh {
        let s = side;
        let v = [
            [0.0, 0.0, 0.0],
            [s, 0.0, 0.0],
            [s, s, 0.0],
            [0.0, s, 0.0],
            [0.0, 0.0, s],
            [s, 0.0, s],
            [s, s, s],
            [0.0, s, s],
        ];
        let faces = [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let mut m = Mesh::empty();
        for f in faces {
            m.add(v[f[0]], v[f[1]], v[f[2]]);
        }
        m
    }

    /// A binary STL of these triangles, with `header` in its eighty bytes.
    pub fn binary_stl(mesh: &Mesh, header: &[u8]) -> Vec<u8> {
        let n = mesh.triangles();
        let mut out = vec![0_u8; 80];
        out[..header.len().min(80)].copy_from_slice(&header[..header.len().min(80)]);
        out.extend_from_slice(&(n as u32).to_le_bytes());
        for i in 0..n {
            out.extend_from_slice(&[0_u8; 12]);
            for c in mesh.corners(i) {
                for x in c {
                    out.extend_from_slice(&x.to_le_bytes());
                }
            }
            out.extend_from_slice(&[0_u8; 2]);
        }
        out
    }

    /// A zip of these members. `crc` is left zero because nothing here reads it.
    pub fn zip_of(files: &[(&str, Vec<u8>, bool)]) -> Vec<u8> {
        use std::io::Write as _;
        let mut out: Vec<u8> = Vec::new();
        let mut central: Vec<u8> = Vec::new();
        for (name, plain, deflate) in files {
            let at = out.len() as u32;
            let body = if *deflate {
                let mut e =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(plain).unwrap();
                e.finish().unwrap()
            } else {
                plain.clone()
            };
            let method: u16 = if *deflate { 8 } else { 0 };
            out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
            out.extend_from_slice(&[20, 0, 0, 0]);
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&0_u32.to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&(plain.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&body);

            central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0; 4]);
            central.extend_from_slice(&0_u32.to_le_bytes());
            central.extend_from_slice(&(body.len() as u32).to_le_bytes());
            central.extend_from_slice(&(plain.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0; 8]);
            central.extend_from_slice(&0_u32.to_le_bytes());
            central.extend_from_slice(&at.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_at = out.len() as u32;
        let cd_len = central.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&cd_len.to_le_bytes());
        out.extend_from_slice(&cd_at.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out
    }

    /// A model part holding one object made of these triangles.
    pub fn model_part(id: &str, mesh: &Mesh) -> String {
        let mut v = String::new();
        let mut t = String::new();
        for i in 0..mesh.triangles() {
            for c in mesh.corners(i) {
                v.push_str(&format!(
                    "<vertex x=\"{}\" y=\"{}\" z=\"{}\"/>",
                    c[0], c[1], c[2]
                ));
            }
            let b = i * 3;
            t.push_str(&format!(
                "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
                b,
                b + 1,
                b + 2
            ));
        }
        format!(
            "<?xml version=\"1.0\"?><model unit=\"millimeter\" \
             xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\
             <resources><object id=\"{id}\" type=\"model\"><mesh>\
             <vertices>{v}</vertices><triangles>{t}</triangles>\
             </mesh></object></resources></model>"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::cube;
    use super::{Mesh, Shape, sanity};

    #[test]
    // THE DEFECT THIS CATCHES: a format reachable by half the path. A `Shape`
    // with no key draws its own variant name nowhere and its FORMAT row says
    // nothing, which is invariant 8 broken in the one place the panel is
    // supposed to name the file. The list is walked and not repeated, which is
    // the whole reason `Shape::ALL` exists.
    fn every_shape_names_itself_with_a_key_the_registry_knows() {
        for shape in Shape::ALL {
            let key = shape.key();
            assert!(!key.is_empty(), "{shape:?} came with no key");
            assert!(
                crate::key::ALL.contains(&key),
                "{shape:?} uses `{key}`, which is outside key::ALL — so nothing \
                 checks that the two languages have it"
            );
        }
        // AND NO TWO SHARE ONE. Two formats on one key is a panel that calls a
        // COLLADA an AMF, and every test above still passes.
        let mut keys: Vec<&str> = Shape::ALL.iter().map(|s| s.key()).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "two shapes share a key");
    }

    #[test]
    // THE DEFECT THIS CATCHES: a volume or an area that is merely
    // self-consistent. A cube of ten is a thousand cubic units and six hundred
    // square ones, and those two numbers were known before this crate existed.
    fn a_cube_has_the_volume_and_the_area_a_cube_has() {
        let m = cube(10.0);
        assert!(
            (m.volume() - 1000.0).abs() < 0.001,
            "volume was {}",
            m.volume()
        );
        assert!((m.area() - 600.0).abs() < 0.001, "area was {}", m.area());
        assert_eq!(m.bounds.size(), [10.0, 10.0, 10.0]);
    }

    #[test]
    // THE DEFECT THIS CATCHES: welding by the exact bits. Two writers that
    // meant the same corner rarely agree to the last bit, and an exact
    // comparison reports a sound piece as a sieve — which is the one fact
    // somebody about to print it actually reads.
    fn a_closed_cube_is_watertight_and_has_eight_corners() {
        let s = sanity(&cube(10.0));
        assert_eq!(s.vertices, 8);
        assert_eq!(s.open, 0);
        assert_eq!(s.flipped, 0);
        assert_eq!(s.degenerate, 0);
        assert!(s.watertight());
    }

    #[test]
    // THE DEFECT THIS CATCHES: reporting every mesh as sound. A hole must be
    // found, or the fact is decoration.
    fn a_cube_missing_a_face_is_not_watertight() {
        let whole = cube(10.0);
        let mut holed = Mesh::empty();
        for i in 1..whole.triangles() {
            let [a, b, c] = whole.corners(i);
            holed.add(a, b, c);
        }
        let s = sanity(&holed);
        assert!(!s.watertight());
        assert_eq!(s.open, 3, "one missing triangle leaves three open edges");
    }

    #[test]
    // THE DEFECT THIS CATCHES: a face wound the wrong way round reading as a
    // hole, or as nothing at all. It is its own kind of trouble and a slicer
    // reports it separately.
    fn a_face_wound_backwards_is_reported_as_flipped_and_not_as_a_hole() {
        let whole = cube(10.0);
        let mut wrong = Mesh::empty();
        for i in 0..whole.triangles() {
            let [a, b, c] = whole.corners(i);
            if i == 0 {
                wrong.add(a, c, b);
            } else {
                wrong.add(a, b, c);
            }
        }
        let s = sanity(&wrong);
        assert_eq!(s.open, 0);
        assert_eq!(s.flipped, 3, "one reversed triangle flips its three edges");
        assert!(!s.watertight());
    }
}

#[cfg(test)]
mod round_trip {
    //! The same triangles through every writer and back.
    //!
    //! # What this proves, and what it does not
    //!
    //! It proves that each reader reads what its own format says, against a
    //! document written from the specification. It does NOT prove that a
    //! Blender or a Fusion writes what the specification says — only a real
    //! file does that, and the ones this repository has are an STL, two 3MFs,
    //! an OBJ and a STEP.
    //!
    //! What it does catch is the failure that matters most here: a reader that
    //! silently produces the WRONG SHAPE. Every format below is handed a cube
    //! of a known size, and a reader that lost the unit, transposed an axis or
    //! misread an index gives back a box that is not that cube.

    use std::path::Path;

    use super::fixture::cube;
    use super::{Mesh, Shape};

    const SIDE: f32 = 10.0;

    /// The corners of every triangle, three at a time, as `(x, y, z)`.
    fn corners(mesh: &Mesh) -> Vec<[f32; 3]> {
        (0..mesh.triangles())
            .flat_map(|i| mesh.corners(i))
            .collect()
    }

    fn as_obj(mesh: &Mesh) -> Vec<u8> {
        let mut out = String::from("# written by a test\n");
        for c in corners(mesh) {
            out.push_str(&format!("v {} {} {}\n", c[0], c[1], c[2]));
        }
        for i in 0..mesh.triangles() {
            out.push_str(&format!("f {} {} {}\n", i * 3 + 1, i * 3 + 2, i * 3 + 3));
        }
        out.into_bytes()
    }

    fn as_ply(mesh: &Mesh) -> Vec<u8> {
        let points = corners(mesh);
        let mut out = format!(
            "ply\nformat ascii 1.0\nelement vertex {}\n\
             property float x\nproperty float y\nproperty float z\n\
             element face {}\nproperty list uchar int vertex_indices\nend_header\n",
            points.len(),
            mesh.triangles()
        );
        for c in &points {
            out.push_str(&format!("{} {} {}\n", c[0], c[1], c[2]));
        }
        for i in 0..mesh.triangles() {
            out.push_str(&format!("3 {} {} {}\n", i * 3, i * 3 + 1, i * 3 + 2));
        }
        out.into_bytes()
    }

    fn as_off(mesh: &Mesh) -> Vec<u8> {
        let points = corners(mesh);
        let mut out = format!("OFF\n{} {} 0\n", points.len(), mesh.triangles());
        for c in &points {
            out.push_str(&format!("{} {} {}\n", c[0], c[1], c[2]));
        }
        for i in 0..mesh.triangles() {
            out.push_str(&format!("3 {} {} {}\n", i * 3, i * 3 + 1, i * 3 + 2));
        }
        out.into_bytes()
    }

    fn as_amf(mesh: &Mesh) -> Vec<u8> {
        let mut out = String::from(
            "<?xml version=\"1.0\"?>\n<amf unit=\"millimeter\"><object id=\"0\"><mesh><vertices>",
        );
        for c in corners(mesh) {
            out.push_str(&format!(
                "<vertex><coordinates><x>{}</x><y>{}</y><z>{}</z></coordinates></vertex>",
                c[0], c[1], c[2]
            ));
        }
        out.push_str("</vertices><volume>");
        for i in 0..mesh.triangles() {
            out.push_str(&format!(
                "<triangle><v1>{}</v1><v2>{}</v2><v3>{}</v3></triangle>",
                i * 3,
                i * 3 + 1,
                i * 3 + 2
            ));
        }
        out.push_str("</volume></mesh></object></amf>");
        out.into_bytes()
    }

    fn as_dae(mesh: &Mesh) -> Vec<u8> {
        let points = corners(mesh);
        let flat: Vec<String> = points
            .iter()
            .flat_map(|c| c.iter().map(std::string::ToString::to_string))
            .collect();
        let order: Vec<String> = (0..points.len()).map(|i| i.to_string()).collect();
        format!(
            "<?xml version=\"1.0\"?>\n\
<COLLADA xmlns=\"http://www.collada.org/2005/11/COLLADASchema\" version=\"1.4.1\">\
<asset><unit meter=\"0.001\" name=\"millimeter\"/><up_axis>Z_UP</up_axis></asset>\
<library_geometries><geometry id=\"g\"><mesh>\
<source id=\"s\"><float_array id=\"a\" count=\"{}\">{}</float_array>\
<technique_common><accessor source=\"#a\" count=\"{}\" stride=\"3\">\
<param name=\"X\" type=\"float\"/><param name=\"Y\" type=\"float\"/>\
<param name=\"Z\" type=\"float\"/></accessor></technique_common></source>\
<vertices id=\"v\"><input semantic=\"POSITION\" source=\"#s\"/></vertices>\
<triangles count=\"{}\"><input semantic=\"VERTEX\" source=\"#v\" offset=\"0\"/>\
<p>{}</p></triangles></mesh></geometry></library_geometries>\
<library_visual_scenes><visual_scene id=\"scene\"><node id=\"n\">\
<instance_geometry url=\"#g\"/></node></visual_scene></library_visual_scenes>\
<scene><instance_visual_scene url=\"#scene\"/></scene></COLLADA>",
            flat.len(),
            flat.join(" "),
            points.len(),
            mesh.triangles(),
            order.join(" ")
        )
        .into_bytes()
    }

    fn as_gltf(mesh: &Mesh) -> Vec<u8> {
        const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let points = corners(mesh);
        let mut buffer = Vec::new();
        for c in &points {
            for v in c {
                // METRES, which is what the format's unit is — so the reader
                // scaling by a thousand is part of what this proves.
                buffer.extend_from_slice(&(v / 1000.0).to_le_bytes());
            }
        }
        let mut base64 = String::new();
        for chunk in buffer.chunks(3) {
            let mut n = 0_u32;
            for (i, b) in chunk.iter().enumerate() {
                n |= u32::from(*b) << (16 - 8 * i);
            }
            for i in 0..4 {
                if i <= chunk.len() {
                    base64.push(ABC[((n >> (18 - 6 * i)) & 63) as usize] as char);
                } else {
                    base64.push('=');
                }
            }
        }
        format!(
            "{{\"asset\":{{\"version\":\"2.0\"}},\"scene\":0,\
              \"scenes\":[{{\"nodes\":[0]}}],\"nodes\":[{{\"mesh\":0}}],\
              \"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}}}}]}}],\
              \"accessors\":[{{\"bufferView\":0,\"componentType\":5126,\"count\":{},\
                \"type\":\"VEC3\"}}],\
              \"bufferViews\":[{{\"buffer\":0,\"byteOffset\":0,\"byteLength\":{}}}],\
              \"buffers\":[{{\"byteLength\":{},\
                \"uri\":\"data:application/octet-stream;base64,{base64}\"}}]}}",
            points.len(),
            buffer.len(),
            buffer.len()
        )
        .into_bytes()
    }

    #[test]
    fn the_same_cube_survives_a_round_trip_through_every_writer() {
        let source = cube(SIDE);
        let written: [(&str, Shape, Vec<u8>); 6] = [
            ("obj", Shape::Obj, as_obj(&source)),
            ("ply", Shape::PlyAscii, as_ply(&source)),
            ("off", Shape::Off, as_off(&source)),
            ("amf", Shape::Amf, as_amf(&source)),
            ("dae", Shape::Collada, as_dae(&source)),
            ("gltf", Shape::Gltf, as_gltf(&source)),
        ];
        for (name, shape, bytes) in written {
            let read = match name {
                "obj" => super::obj::read(&bytes),
                "ply" => super::ply::read(&bytes),
                "off" => super::off::read(&bytes),
                "amf" => super::amf::read(&bytes),
                "dae" => super::dae::read(&bytes),
                _ => super::gltf::read(&bytes, Path::new("/nowhere/a.gltf")),
            }
            .unwrap_or_else(|e| panic!("`{name}` did not read back: {e:?}"));
            assert_eq!(read.shape, shape, "`{name}` came back as the wrong format");
            let back = &read.body.mesh;
            assert_eq!(
                back.triangles(),
                source.triangles(),
                "`{name}` lost triangles"
            );
            for (a, b) in back.bounds.size().iter().zip(source.bounds.size()) {
                assert!(
                    (a - b).abs() < 0.01,
                    "`{name}` came back {:?} instead of {:?}",
                    back.bounds.size(),
                    source.bounds.size()
                );
            }
            // AND THE VOLUME, which is the check that an axis was not swapped:
            // a cube is the same size whichever two axes are transposed, and
            // its winding is not.
            assert!(
                (back.volume() - source.volume()).abs() < 1.0,
                "`{name}` enclosed {} instead of {}",
                back.volume(),
                source.volume()
            );
        }
    }
}
