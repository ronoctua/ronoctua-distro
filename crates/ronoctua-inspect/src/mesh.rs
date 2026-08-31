//! The shape inside a `.stl` or a `.3mf`, and what can be said about it.
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
//! subtractions and a multiply per facet.
//!
//! # Why 3MF is not simply "the XML in the zip"
//!
//! Because the geometry is usually somewhere else. The format's *production*
//! extension lets `3D/3dmodel.model` hold nothing but `<components>` pointing
//! at other parts by `p:path`, and slicers use it: the file the owner supplied
//! has a 1.5 KB root and its 21 386 triangles in
//! `3D/Objects/candle-holder.stl_1.model`. And both the component and the
//! `<build><item>` carry a transform — that same file scales Z by 1.808, so a
//! reader that ignored transforms would draw the piece squashed and no error
//! anywhere would say why.

use std::io::Read as _;
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

/// How many bytes a `.3mf` may inflate to.
///
/// A 3MF is a zip, and a zip is a thing somebody can build to inflate for ever
/// out of a few kilobytes. The compressed size cannot bound it — that is the
/// whole trick — so the bound is on what comes OUT, applied while inflating and
/// not after. Two hundred and fifty-six mebibytes is roughly the ceiling above
/// expressed as XML, which is about ten times bulkier than the binary form.
pub const INFLATED_CEILING: u64 = 256 * 1024 * 1024;

/// How large a mesh file is read at all.
///
/// The triangle ceiling cannot be applied before reading, because an ASCII STL
/// gives no count until it has been walked. This is the guard that exists
/// before a single byte is allocated.
pub const FILE_CEILING: u64 = 512 * 1024 * 1024;

/// Which of the three shapes the bytes turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The 80-byte header, a count, and 50 bytes per facet.
    StlBinary,
    /// `solid … facet normal … endsolid`.
    StlAscii,
    /// A zip of XML.
    ThreeMf,
}

impl Shape {
    /// The text key naming this shape in the info panel.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::StlBinary => crate::key::VALUE_STL_BINARY,
            Self::StlAscii => crate::key::VALUE_STL_ASCII,
            Self::ThreeMf => crate::key::VALUE_THREE_MF,
        }
    }
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

/// Everything read out of one mesh file.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    /// The triangles.
    pub mesh: Mesh,
    /// Which format it turned out to be.
    pub shape: Shape,
    /// The unit a 3MF declares. STL declares none — the convention is
    /// millimetres and this crate does not pretend otherwise.
    pub unit: Option<String>,
    /// How many objects the build places. One, for an STL.
    pub objects: usize,
    /// The `<metadata>` a 3MF carries, in the order it carries it.
    pub notes: Vec<(String, String)>,
}

/// Reads whatever mesh is in the file.
///
/// `mime` decides, and it has already been through
/// [`crate::settled`] — which matters here more than anywhere else in this
/// crate: a binary STL has no magic number at all (freedesktop's only rule is
/// the word `solid`, which is the ASCII form), so its type comes from the name
/// alone and the sniff correctly says nothing.
///
/// # Errors
///
/// [`Refusal::Unreadable`] when it cannot be opened, [`Refusal::TooBig`] past a
/// ceiling, and [`Refusal::Failed`] when the bytes are not the shape they claim.
pub fn read(path: &Path, mime: &str) -> Result<Read, Refusal> {
    let bytes = slurp(path, FILE_CEILING)?;
    if ronoctua_files::mime::is_a(mime, "model/3mf") {
        three_mf(&bytes)
    } else {
        let (mesh, shape) = stl(&bytes)?;
        Ok(Read {
            mesh,
            shape,
            unit: None,
            objects: 1,
            notes: Vec::new(),
        })
    }
}

/// The whole file, refused rather than read when it is past `ceiling`.
fn slurp(path: &Path, ceiling: u64) -> Result<Vec<u8>, Refusal> {
    let meta = std::fs::metadata(path).map_err(|_| Refusal::Unreadable)?;
    if meta.len() > ceiling {
        return Err(Refusal::TooBig(meta.len()));
    }
    std::fs::read(path).map_err(|_| Refusal::Unreadable)
}

/// Binary or ASCII, decided by ARITHMETIC and never by the first word.
///
/// A binary STL's 80-byte header is free-form and writers put anything in it,
/// including the word `solid` — which is exactly what the naive test looks for,
/// and it then reads a binary file as text and finds no facets at all. The
/// count at offset 80 and the file's length agree only for a real binary STL,
/// so that is the question asked. (The file the owner supplied opens with
/// eighty ZERO bytes, which the naive test survives by luck.)
fn stl(bytes: &[u8]) -> Result<(Mesh, Shape), Refusal> {
    if let Some(n) = binary_count(bytes) {
        return stl_binary(bytes, n).map(|m| (m, Shape::StlBinary));
    }
    stl_ascii(bytes).map(|m| (m, Shape::StlAscii))
}

/// The facet count, when the length says this really is a binary STL.
fn binary_count(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 84 {
        return None;
    }
    let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
    let want = 84_u64 + 50 * u64::from(n);
    (want == bytes.len() as u64).then_some(n)
}

fn stl_binary(bytes: &[u8], count: u32) -> Result<Mesh, Refusal> {
    let n = count as usize;
    if n > TRIANGLE_CEILING {
        return Err(Refusal::TooBig(bytes.len() as u64));
    }
    let mut mesh = Mesh::empty();
    mesh.positions.reserve(n * 9);
    mesh.normals.reserve(n * 3);
    for i in 0..n {
        // 12 bytes of normal we do not use, then three vertices.
        let at = 84 + i * 50 + 12;
        let f = |k: usize| -> f32 {
            let o = at + k * 4;
            f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
        };
        mesh.add([f(0), f(1), f(2)], [f(3), f(4), f(5)], [f(6), f(7), f(8)]);
    }
    Ok(mesh)
}

fn stl_ascii(bytes: &[u8]) -> Result<Mesh, Refusal> {
    let text = std::str::from_utf8(bytes).map_err(|_| Refusal::Failed)?;
    let mut mesh = Mesh::empty();
    let mut corner: Vec<[f32; 3]> = Vec::with_capacity(3);
    let mut words = text.split_ascii_whitespace();
    while let Some(w) = words.next() {
        if w != "vertex" {
            continue;
        }
        let mut v = [0.0_f32; 3];
        for slot in &mut v {
            let Some(t) = words.next() else {
                return Err(Refusal::Failed);
            };
            *slot = t.parse::<f32>().map_err(|_| Refusal::Failed)?;
        }
        corner.push(v);
        if corner.len() == 3 {
            if mesh.triangles() >= TRIANGLE_CEILING {
                return Err(Refusal::TooBig(bytes.len() as u64));
            }
            mesh.add(corner[0], corner[1], corner[2]);
            corner.clear();
        }
    }
    if mesh.triangles() == 0 {
        return Err(Refusal::Failed);
    }
    Ok(mesh)
}

// ---------------------------------------------------------------------------
// 3MF: a zip of XML that points at more XML.
// ---------------------------------------------------------------------------

/// How deep a chain of `<components>` may go before it is called a loop.
///
/// The format allows an object to be built out of other objects, and nothing
/// in it forbids one from referring back to itself. Eight is past anything a
/// slicer writes and short enough that a malicious file cannot make this walk
/// for ever.
const NESTING_CEILING: u32 = 8;

/// The relationship type that names the root model part.
const ROOT_RELATION: &str = "http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel";

/// Where the root model lives when `_rels/.rels` does not say.
const ROOT_FALLBACK: &str = "3D/3dmodel.model";

/// One member of the zip, as the central directory describes it.
struct Member {
    name: String,
    method: u16,
    compressed: u64,
    plain: u64,
    at: u64,
}

/// The members of a zip, found through its central directory.
///
/// Written here rather than taken from a crate because the whole of what is
/// needed is "walk the directory, inflate one member" — and a zip crate would
/// be a new entry in the lock for that, where `flate2` is already in the tree
/// under `png`. The workspace's rule is what decides: a dependency is welcome
/// when it is Rust and costs no store path, and it is not welcome merely
/// because it exists.
fn members(bytes: &[u8]) -> Result<Vec<Member>, Refusal> {
    // The end-of-central-directory record is last, after a comment of up to
    // 64 KiB, so it is found by scanning backwards for its signature.
    let tail = bytes.len().saturating_sub(66_000);
    let mut eocd = None;
    let mut i = bytes.len().saturating_sub(22);
    while i >= tail {
        if bytes[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06]) {
            eocd = Some(i);
            break;
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    let Some(eocd) = eocd else {
        return Err(Refusal::Failed);
    };
    let count = u16::from_le_bytes([bytes[eocd + 10], bytes[eocd + 11]]) as usize;
    let mut at = u32::from_le_bytes([
        bytes[eocd + 16],
        bytes[eocd + 17],
        bytes[eocd + 18],
        bytes[eocd + 19],
    ]) as usize;

    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if at + 46 > bytes.len() || !bytes[at..].starts_with(&[0x50, 0x4b, 0x01, 0x02]) {
            return Err(Refusal::Failed);
        }
        let u16_at = |o: usize| u16::from_le_bytes([bytes[at + o], bytes[at + o + 1]]);
        let u32_at = |o: usize| {
            u32::from_le_bytes([
                bytes[at + o],
                bytes[at + o + 1],
                bytes[at + o + 2],
                bytes[at + o + 3],
            ])
        };
        let method = u16_at(10);
        let compressed = u32_at(20);
        let plain = u32_at(24);
        let name_len = u16_at(28) as usize;
        let extra_len = u16_at(30) as usize;
        let comment_len = u16_at(32) as usize;
        let local = u32_at(42);
        // ZIP64 puts these in the extra field. A 3MF that needs it is far past
        // this crate's ceilings anyway, so it is refused rather than
        // half-understood.
        if compressed == u32::MAX || plain == u32::MAX || local == u32::MAX {
            return Err(Refusal::TooBig(u64::from(u32::MAX)));
        }
        let name_at = at + 46;
        if name_at + name_len > bytes.len() {
            return Err(Refusal::Failed);
        }
        let name = String::from_utf8_lossy(&bytes[name_at..name_at + name_len]).into_owned();
        out.push(Member {
            name,
            method,
            compressed: u64::from(compressed),
            plain: u64::from(plain),
            at: u64::from(local),
        });
        at = name_at + name_len + extra_len + comment_len;
    }
    Ok(out)
}

/// One member's bytes, inflated with a ceiling ON THE WAY OUT.
///
/// The bound is on what comes out and is applied while inflating, because that
/// is the only bound a zip bomb cannot lie about: the compressed size is a few
/// kilobytes by construction.
fn member_bytes(bytes: &[u8], m: &Member) -> Result<Vec<u8>, Refusal> {
    let at = m.at as usize;
    if at + 30 > bytes.len() || !bytes[at..].starts_with(&[0x50, 0x4b, 0x03, 0x04]) {
        return Err(Refusal::Failed);
    }
    let name_len = u16::from_le_bytes([bytes[at + 26], bytes[at + 27]]) as usize;
    let extra_len = u16::from_le_bytes([bytes[at + 28], bytes[at + 29]]) as usize;
    let from = at + 30 + name_len + extra_len;
    let to = from + m.compressed as usize;
    if to > bytes.len() {
        return Err(Refusal::Failed);
    }
    let raw = &bytes[from..to];
    match m.method {
        0 => {
            if m.plain > INFLATED_CEILING {
                return Err(Refusal::TooBig(m.plain));
            }
            Ok(raw.to_vec())
        }
        8 => {
            let mut out = Vec::new();
            let took = flate2::read::DeflateDecoder::new(raw)
                .take(INFLATED_CEILING + 1)
                .read_to_end(&mut out)
                .map_err(|_| Refusal::Failed)?;
            if took as u64 > INFLATED_CEILING {
                return Err(Refusal::TooBig(took as u64));
            }
            Ok(out)
        }
        _ => Err(Refusal::Failed),
    }
}

/// A member by name, tolerating the leading slash the relationships use.
fn part<'a>(list: &'a [Member], name: &str) -> Option<&'a Member> {
    let want = name.trim_start_matches('/');
    list.iter().find(|m| m.name.trim_start_matches('/') == want)
}

/// An element by LOCAL name, whatever namespace it is in.
///
/// `roxmltree`'s own `has_tag_name("model")` asks for an element in NO
/// namespace, and every 3MF ever written declares a default one on `<model>` —
/// so that spelling matches nothing at all, in every file, silently. The
/// namespace is not information here: a `<vertex>` is a vertex whichever URI
/// the writer chose, and the extensions are told apart by their attributes.
fn tagged(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.tag_name().name() == name
}

/// An attribute by LOCAL name, whatever namespace prefix it carries.
///
/// `p:path` and `path` are the same attribute to this reader, and which prefix
/// a writer chose for the production extension is its own business.
fn attr<'a>(node: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name() == name)
        .map(|a| a.value())
}

/// The identity transform: no rotation, no scale, no translation.
const STILL: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

/// Twelve numbers, as 3MF writes a matrix.
///
/// Row-vector convention: the first nine are the three basis rows and the last
/// three are the translation, so a point is `v · M + t`. Anything that is not
/// twelve numbers is treated as no transform at all rather than as an error —
/// a file that misspells one matrix is still a file worth looking at.
fn matrix(text: Option<&str>) -> [f32; 12] {
    let Some(text) = text else {
        return STILL;
    };
    let mut out = STILL;
    let mut n = 0;
    for w in text.split_ascii_whitespace() {
        if n == 12 {
            return STILL;
        }
        let Ok(v) = w.parse::<f32>() else {
            return STILL;
        };
        out[n] = v;
        n += 1;
    }
    if n == 12 { out } else { STILL }
}

/// One point through one matrix.
fn moved(m: &[f32; 12], v: [f32; 3]) -> [f32; 3] {
    [
        v[0] * m[0] + v[1] * m[3] + v[2] * m[6] + m[9],
        v[0] * m[1] + v[1] * m[4] + v[2] * m[7] + m[10],
        v[0] * m[2] + v[1] * m[5] + v[2] * m[8] + m[11],
    ]
}

/// `a` and then `b`, as one matrix.
fn after(a: &[f32; 12], b: &[f32; 12]) -> [f32; 12] {
    let mut out = STILL;
    for r in 0..3 {
        for c in 0..3 {
            out[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
        }
    }
    let t = moved(b, [a[9], a[10], a[11]]);
    out[9] = t[0];
    out[10] = t[1];
    out[11] = t[2];
    out
}

/// What one `<object>` holds: either geometry, or references to more objects.
struct Object {
    vertices: Vec<[f32; 3]>,
    faces: Vec<[usize; 3]>,
    parts: Vec<(Option<String>, String, [f32; 12])>,
}

/// Every object one model part declares, by id.
fn objects_of(doc: &roxmltree::Document<'_>) -> Vec<(String, Object)> {
    let mut out = Vec::new();
    for object in doc.descendants().filter(|n| tagged(*n, "object")) {
        let Some(id) = attr(object, "id") else {
            continue;
        };
        let mut o = Object {
            vertices: Vec::new(),
            faces: Vec::new(),
            parts: Vec::new(),
        };
        for v in object.descendants().filter(|n| tagged(*n, "vertex")) {
            let f = |k| {
                attr(v, k)
                    .and_then(|t| t.parse::<f32>().ok())
                    .unwrap_or(0.0)
            };
            o.vertices.push([f("x"), f("y"), f("z")]);
        }
        for t in object.descendants().filter(|n| tagged(*n, "triangle")) {
            let f = |k| attr(t, k).and_then(|x| x.parse::<usize>().ok());
            if let (Some(a), Some(b), Some(c)) = (f("v1"), f("v2"), f("v3")) {
                o.faces.push([a, b, c]);
            }
        }
        for c in object.descendants().filter(|n| tagged(*n, "component")) {
            let Some(to) = attr(c, "objectid") else {
                continue;
            };
            // THE LEADING SLASH COMES OFF HERE, once. A relationship's target
            // and a component's path are written `/3D/Objects/x.model` while
            // the zip stores `3D/Objects/x.model`, and a lookup that kept the
            // slash finds nothing, adds no triangles, and reports a perfectly
            // good package as damaged.
            o.parts.push((
                attr(c, "path").map(|p| p.trim_start_matches('/').to_owned()),
                to.to_owned(),
                matrix(attr(c, "transform")),
            ));
        }
        out.push((id.to_owned(), o));
    }
    out
}

/// Reads a 3MF: the relationships, the root model, and every part it reaches.
fn three_mf(bytes: &[u8]) -> Result<Read, Refusal> {
    let list = members(bytes)?;

    // WHERE THE ROOT MODEL IS, asked of the package rather than assumed. The
    // convention is `3D/3dmodel.model` and every writer follows it, but the
    // relationship is what the format actually specifies.
    let mut root = ROOT_FALLBACK.to_owned();
    if let Some(m) = part(&list, "_rels/.rels")
        && let Ok(raw) = member_bytes(bytes, m)
        && let Ok(text) = String::from_utf8(raw)
        && let Ok(doc) = roxmltree::Document::parse(&text)
    {
        for r in doc.descendants().filter(|n| tagged(*n, "Relationship")) {
            if attr(r, "Type") == Some(ROOT_RELATION)
                && let Some(t) = attr(r, "Target")
            {
                root = t.trim_start_matches('/').to_owned();
                break;
            }
        }
    }

    // Every part is read at most once, and only when something points at it.
    let mut texts: Vec<(String, String)> = Vec::new();
    let load = |name: &str, texts: &mut Vec<(String, String)>| -> Option<usize> {
        let want = name.trim_start_matches('/').to_owned();
        if let Some(i) = texts.iter().position(|(n, _)| *n == want) {
            return Some(i);
        }
        let m = part(&list, &want)?;
        let raw = member_bytes(bytes, m).ok()?;
        let text = String::from_utf8(raw).ok()?;
        texts.push((want, text));
        Some(texts.len() - 1)
    };
    let Some(root_at) = load(&root, &mut texts) else {
        return Err(Refusal::Failed);
    };

    // The root's own document answers the unit, the metadata and the build.
    let root_doc = roxmltree::Document::parse(&texts[root_at].1).map_err(|_| Refusal::Failed)?;
    let unit = root_doc
        .descendants()
        .find(|n| tagged(*n, "model"))
        .and_then(|n| attr(n, "unit"))
        .map(ToOwned::to_owned);
    let mut notes = Vec::new();
    for m in root_doc.descendants().filter(|n| tagged(*n, "metadata")) {
        if let (Some(name), Some(text)) = (attr(m, "name"), m.text())
            && !text.trim().is_empty()
        {
            notes.push((name.to_owned(), text.trim().to_owned()));
        }
    }
    let items: Vec<(String, [f32; 12])> = root_doc
        .descendants()
        .filter(|n| tagged(*n, "item"))
        .filter_map(|n| attr(n, "objectid").map(|id| (id.to_owned(), matrix(attr(n, "transform")))))
        .collect();

    // Every part that is reachable, loaded and parsed once.
    let mut wanted: Vec<String> = vec![texts[root_at].0.clone()];
    let mut seen = 0;
    while seen < wanted.len() {
        let name = wanted[seen].clone();
        seen += 1;
        let Some(at) = load(&name, &mut texts) else {
            continue;
        };
        let Ok(doc) = roxmltree::Document::parse(&texts[at].1) else {
            continue;
        };
        for c in doc.descendants().filter(|n| tagged(*n, "component")) {
            if let Some(p) = attr(c, "path") {
                let p = p.trim_start_matches('/').to_owned();
                if !wanted.contains(&p) {
                    wanted.push(p);
                }
            }
        }
    }
    let docs: Vec<(String, roxmltree::Document<'_>)> = texts
        .iter()
        .filter_map(|(n, t)| roxmltree::Document::parse(t).ok().map(|d| (n.clone(), d)))
        .collect();
    let table: Vec<(String, Vec<(String, Object)>)> = docs
        .iter()
        .map(|(n, d)| (n.clone(), objects_of(d)))
        .collect();

    let root_name = texts[root_at].0.clone();
    let mut mesh = Mesh::empty();
    let mut objects = 0;
    // A build with no items is a package that declares shapes and places none.
    // Drawing every object at the origin is what every viewer does, and it is
    // better than an empty panel.
    let placed: Vec<(String, [f32; 12])> = if items.is_empty() {
        table
            .iter()
            .find(|(n, _)| *n == root_name)
            .map(|(_, o)| o.iter().map(|(id, _)| (id.clone(), STILL)).collect())
            .unwrap_or_default()
    } else {
        items
    };
    for (id, m) in &placed {
        objects += 1;
        place(&table, &root_name, id, m, 0, &mut mesh)?;
    }
    if mesh.triangles() == 0 {
        return Err(Refusal::Failed);
    }
    // EVERYTHING LEAVES HERE IN MILLIMETRES. A 3MF declares its unit and the
    // spec allows six of them; a reader that ignored the declaration would
    // draw an inch-based file perfectly — the picture is scale-free — and then
    // report a 60 mm boat as 60 mm when it is 1524. The drawing never cared,
    // and the info panel is the whole reason this matters.
    let scale = millimetres_per(unit.as_deref());
    if (scale - 1.0).abs() > f32::EPSILON {
        let mut grown = Bounds::nothing();
        for v in &mut mesh.positions {
            *v *= scale;
        }
        for i in 0..mesh.triangles() {
            for c in mesh.corners(i) {
                grown.hold(c);
            }
        }
        mesh.bounds = grown;
        // The normals are areas, which scale as the square.
        for v in &mut mesh.normals {
            *v *= scale * scale;
        }
    }
    Ok(Read {
        mesh,
        shape: Shape::ThreeMf,
        unit,
        objects,
        notes,
    })
}

/// How many millimetres one of the file's units is.
///
/// The six 3MF allows, and millimetres for anything else — including a file
/// that declares nothing, which is what an STL always does and what the whole
/// 3D-printing world means when it says a number.
fn millimetres_per(unit: Option<&str>) -> f32 {
    match unit {
        Some("micron") => 0.001,
        Some("centimeter") => 10.0,
        Some("inch") => 25.4,
        Some("foot") => 304.8,
        Some("meter") => 1000.0,
        _ => 1.0,
    }
}

/// Adds one object's triangles, following its components.
fn place(
    table: &[(String, Vec<(String, Object)>)],
    part_name: &str,
    id: &str,
    m: &[f32; 12],
    depth: u32,
    into: &mut Mesh,
) -> Result<(), Refusal> {
    if depth > NESTING_CEILING {
        return Ok(());
    }
    let Some((_, objects)) = table.iter().find(|(n, _)| n == part_name) else {
        return Ok(());
    };
    let Some((_, o)) = objects.iter().find(|(k, _)| k == id) else {
        return Ok(());
    };
    for f in &o.faces {
        if into.triangles() >= TRIANGLE_CEILING {
            return Err(Refusal::TooBig(into.triangles() as u64));
        }
        let Some(a) = o.vertices.get(f[0]) else {
            continue;
        };
        let Some(b) = o.vertices.get(f[1]) else {
            continue;
        };
        let Some(c) = o.vertices.get(f[2]) else {
            continue;
        };
        into.add(moved(m, *a), moved(m, *b), moved(m, *c));
    }
    for (path, to, own) in &o.parts {
        let next = path.as_deref().unwrap_or(part_name);
        place(table, next, to, &after(own, m), depth + 1, into)?;
    }
    Ok(())
}

/// What the info panel says about a mesh.
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
/// saying none.
#[must_use]
pub fn facts(read: &Read) -> Vec<crate::facts::Fact> {
    use crate::facts::{Fact, Value};
    use crate::key;

    let count = |n: usize| Value::Phrase {
        key: key::VALUE_COUNT,
        args: vec![("n", n.to_string())],
    };
    let mut out = vec![Fact {
        label: key::FACT_FORMAT,
        value: Value::phrase(read.shape.key()),
    }];
    if let Some(unit) = &read.unit {
        out.push(Fact {
            label: key::FACT_UNIT,
            value: Value::Data(unit.clone()),
        });
    }
    // ONLY WHEN THERE IS MORE THAN ONE. A row saying "objects: 1" is a row
    // that tells nobody anything, and every STL there has ever been would
    // carry it.
    if read.objects > 1 {
        out.push(Fact {
            label: key::FACT_OBJECTS,
            value: count(read.objects),
        });
    }

    let sane = sanity(&read.mesh);
    let size = read.mesh.bounds.size();
    out.push(Fact {
        label: key::FACT_TRIANGLES,
        value: count(read.mesh.triangles()),
    });
    out.push(Fact {
        label: key::FACT_VERTICES,
        value: count(sane.vertices),
    });
    out.push(Fact {
        label: key::FACT_BOUNDS,
        value: Value::Phrase {
            key: key::VALUE_BOX,
            args: vec![
                ("w", format!("{:.1}", size[0])),
                ("d", format!("{:.1}", size[1])),
                ("h", format!("{:.1}", size[2])),
            ],
        },
    });
    // CUBIC CENTIMETRES AND NOT CUBIC MILLIMETRES. The Benchy is 15 550 mm3,
    // which is a number nobody has a feel for, and 15.6 cm3, which is what a
    // spool of filament is sold against.
    out.push(Fact {
        label: key::FACT_VOLUME,
        value: Value::Phrase {
            key: key::VALUE_CUBIC_CENTIMETRES,
            args: vec![("n", format!("{:.1}", read.mesh.volume() / 1000.0))],
        },
    });
    out.push(Fact {
        label: key::FACT_AREA,
        value: Value::Phrase {
            key: key::VALUE_SQUARE_CENTIMETRES,
            args: vec![("n", format!("{:.1}", read.mesh.area() / 100.0))],
        },
    });
    out.push(Fact {
        label: key::FACT_WATERTIGHT,
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
            label: key::FACT_FLIPPED,
            value: count(sane.flipped),
        });
    }
    if sane.degenerate > 0 {
        out.push(Fact {
            label: key::FACT_DEGENERATE,
            value: count(sane.degenerate),
        });
    }

    // THE METADATA THE FORMAT DEFINES, and only that. A 3MF may carry any name
    // at all — the owner's file has `BambuStudio:3mfVersion` and two thumbnail
    // paths — and a panel that showed every one of them would be a panel of a
    // slicer's private notes.
    for (name, label) in [
        ("Application", key::FACT_APPLICATION),
        ("Title", key::FACT_TITLE),
        ("Designer", key::FACT_DESIGNER),
        ("LicenseTerms", key::FACT_LICENCE),
        ("License", key::FACT_LICENCE),
        ("CreationDate", key::FACT_CREATED),
    ] {
        if let Some((_, v)) = read.notes.iter().find(|(k, _)| k == name)
            && !out.iter().any(|f| f.label == label)
        {
            out.push(Fact {
                label,
                value: Value::Data(v.clone()),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cube of `side`, twelve triangles, wound outwards.
    ///
    /// Chosen because its volume and area are known without measuring
    /// anything: a test that compared the reader against the reader would
    /// prove only that it is consistent.
    fn cube(side: f32) -> Mesh {
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
    fn binary_stl(mesh: &Mesh, header: &[u8]) -> Vec<u8> {
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
    fn zip_of(files: &[(&str, Vec<u8>, bool)]) -> Vec<u8> {
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
    fn model_part(id: &str, mesh: &Mesh) -> String {
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

    #[test]
    // THE DEFECT THIS CATCHES: deciding binary-versus-ASCII by the first word.
    // A binary STL's eighty-byte header is free-form and writers put anything
    // in it, `solid` included — and a reader that trusted the word would then
    // look for facets in what are actually floats, find none, and report a
    // perfectly good file as damaged.
    fn a_binary_stl_whose_header_begins_with_solid_is_still_read_as_binary() {
        let bytes = binary_stl(&cube(10.0), b"solid this is a lie written by a slicer");
        let (mesh, shape) = stl(&bytes).expect("a binary STL reads");
        assert_eq!(shape, Shape::StlBinary);
        assert_eq!(mesh.triangles(), 12);
    }

    #[test]
    // THE DEFECT THIS CATCHES: the same question from the other side — an
    // ASCII STL must not be mistaken for a binary one just because its length
    // happens to be large. The arithmetic is what decides, both ways.
    fn an_ascii_stl_is_read_as_words() {
        let text = "solid cube\n\
                    facet normal 0 0 0\n outer loop\n \
                    vertex 0 0 0\n vertex 1 0 0\n vertex 0 1 0\n \
                    endloop\n endfacet\n endsolid cube\n";
        let (mesh, shape) = stl(text.as_bytes()).expect("an ASCII STL reads");
        assert_eq!(shape, Shape::StlAscii);
        assert_eq!(mesh.triangles(), 1);
        assert_eq!(mesh.bounds.size(), [1.0, 1.0, 0.0]);
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

    #[test]
    // THE DEFECT THIS CATCHES: assuming the geometry is in `3D/3dmodel.model`.
    // The production extension lets the root hold nothing but references, and
    // slicers use it — the file the owner supplied has a 1.5 KB root and its
    // triangles in a part underneath.
    fn a_3mf_follows_its_relationships_to_the_part_that_holds_the_geometry() {
        let rels = "<?xml version=\"1.0\"?><Relationships \
             xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             <Relationship Target=\"/3D/elsewhere.model\" Id=\"rel-1\" \
             Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\
             </Relationships>";
        let root = "<?xml version=\"1.0\"?><model unit=\"millimeter\" \
             xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\" \
             xmlns:p=\"http://schemas.microsoft.com/3dmanufacturing/production/2015/06\">\
             <metadata name=\"Application\">a slicer</metadata>\
             <resources><object id=\"2\" type=\"model\"><components>\
             <component p:path=\"/3D/Objects/deep.model\" objectid=\"1\" \
             transform=\"1 0 0 0 1 0 0 0 1 0 0 0\"/>\
             </components></object></resources>\
             <build><item objectid=\"2\"/></build></model>";
        let deep = model_part("1", &cube(10.0));
        let zip = zip_of(&[
            ("_rels/.rels", rels.as_bytes().to_vec(), true),
            ("3D/elsewhere.model", root.as_bytes().to_vec(), true),
            ("3D/Objects/deep.model", deep.into_bytes(), true),
        ]);
        let read = three_mf(&zip).expect("a 3MF reads");
        assert_eq!(read.shape, Shape::ThreeMf);
        assert_eq!(read.mesh.triangles(), 12);
        assert_eq!(read.unit.as_deref(), Some("millimeter"));
        assert_eq!(read.objects, 1);
        assert_eq!(
            read.notes,
            vec![("Application".to_owned(), "a slicer".to_owned())]
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: ignoring the transforms. The file the owner
    // supplied scales Z by 1.808 on its build item, so a reader that dropped
    // the matrix would draw the piece squashed — and nothing on screen would
    // say why, because the triangles would all be there.
    fn the_build_transform_is_applied_or_the_piece_is_the_wrong_shape() {
        let root = "<?xml version=\"1.0\"?><model unit=\"millimeter\" \
             xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\" \
             xmlns:p=\"http://schemas.microsoft.com/3dmanufacturing/production/2015/06\">\
             <resources><object id=\"2\" type=\"model\"><components>\
             <component p:path=\"/3D/Objects/deep.model\" objectid=\"1\"/>\
             </components></object></resources>\
             <build><item objectid=\"2\" \
             transform=\"1 0 0 0 1 0 0 0 1.808 0 0 0\"/></build></model>";
        let deep = model_part("1", &cube(10.0));
        let zip = zip_of(&[
            ("3D/3dmodel.model", root.as_bytes().to_vec(), true),
            ("3D/Objects/deep.model", deep.into_bytes(), true),
        ]);
        let read = three_mf(&zip).expect("a 3MF reads");
        let size = read.mesh.bounds.size();
        assert!((size[0] - 10.0).abs() < 0.001, "x was {}", size[0]);
        assert!(
            (size[2] - 18.08).abs() < 0.001,
            "z was {}, not scaled",
            size[2]
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: bounding the inflation by the COMPRESSED size,
    // which is the one number a zip bomb is built to make small.
    fn a_member_that_claims_to_inflate_past_the_ceiling_is_refused() {
        let m = Member {
            name: "big".to_owned(),
            method: 0,
            compressed: 4,
            plain: INFLATED_CEILING + 1,
            at: 0,
        };
        let mut fake = vec![0x50, 0x4b, 0x03, 0x04];
        fake.extend_from_slice(&[0_u8; 26]);
        fake.extend_from_slice(b"data");
        assert_eq!(
            member_bytes(&fake, &m),
            Err(Refusal::TooBig(INFLATED_CEILING + 1))
        );
    }

    #[test]
    // THE DEFECT THIS CATCHES: a matrix that is not twelve numbers taken as
    // if it were. A file that misspells one is still worth looking at, and
    // half a matrix would put the piece somewhere nobody could find it.
    fn a_matrix_that_is_not_twelve_numbers_moves_nothing() {
        assert_eq!(matrix(None), STILL);
        assert_eq!(matrix(Some("1 0 0")), STILL);
        assert_eq!(matrix(Some("1 0 0 0 1 0 0 0 1 0 0 x")), STILL);
        let m = matrix(Some("2 0 0 0 2 0 0 0 2 1 1 1"));
        assert_eq!(moved(&m, [1.0, 1.0, 1.0]), [3.0, 3.0, 3.0]);
    }
}
