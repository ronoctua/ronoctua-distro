//! PLY: a text header that describes the bytes after it.
//!
//! # Why a property this reader does not want still has to be understood
//!
//! Because the record has no separators. A vertex in a scan from a hand-held
//! scanner is `x y z nx ny nz red green blue alpha confidence intensity`, and
//! the three floats this reader wants are followed by nine values of five
//! different widths. Skipping them means knowing that `uchar` is one byte and
//! `double` is eight — so the header is parsed into a list of TYPED fields and
//! every one of them is stepped over by its own width, wanted or not. A reader
//! that assumed a vertex is three floats would read the next vertex's `x` out
//! of this one's normal, and the piece would come out as noise rather than as
//! an error.
//!
//! # Why the element order is followed and not searched
//!
//! The header may declare `element face` before `element vertex`, and some
//! writers declare a third element between them — `edge`, or a custom one. The
//! body is written in the order the header declares, so the body is READ in
//! that order and elements nobody wants are stepped over whole.
//!
//! # The count of a list is not the type of its items
//!
//! `property list uchar int vertex_indices` is a one-byte count followed by
//! that many four-byte indices, and the two widths are declared separately
//! because they are separate. Assuming they match reads a triangle as garbage.

use super::{Mesh, Read, Shape, TRIANGLE_CEILING, find};
use crate::Refusal;

/// How the numbers after the header are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wire {
    Words,
    LittleEndian,
    BigEndian,
}

/// One of the scalar types the format defines, by its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Kind {
    /// The type a header word names.
    fn named(word: &str) -> Option<Self> {
        Some(match word {
            "char" | "int8" => Self::I8,
            "uchar" | "uint8" => Self::U8,
            "short" | "int16" => Self::I16,
            "ushort" | "uint16" => Self::U16,
            "int" | "int32" => Self::I32,
            "uint" | "uint32" => Self::U32,
            "float" | "float32" => Self::F32,
            "double" | "float64" => Self::F64,
            _ => return None,
        })
    }

    /// How many bytes it takes in a binary body.
    const fn width(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}

/// One declared property of an element.
struct Field {
    name: String,
    /// The item type — and, for a list, the type of each item.
    kind: Kind,
    /// The type of the leading count, when the property is a list.
    counted_by: Option<Kind>,
}

/// One declared element: a name, a number of records, and their fields.
struct Element {
    name: String,
    records: usize,
    fields: Vec<Field>,
}

/// Reads a PLY of either spelling.
///
/// # Errors
///
/// [`Refusal::Failed`] when the header is not a PLY header or the body runs
/// out, and [`Refusal::TooBig`] past [`TRIANGLE_CEILING`].
pub fn read(bytes: &[u8]) -> Result<Read, Refusal> {
    let (wire, elements, body_at) = header(bytes)?;
    let mut cursor = Cursor {
        bytes,
        at: body_at,
        wire,
    };
    let mut points: Vec<[f32; 3]> = Vec::new();
    let mut mesh = Mesh::empty();

    for element in &elements {
        match element.name.as_str() {
            "vertex" => {
                if element.records > TRIANGLE_CEILING * 3 {
                    return Err(Refusal::TooBig(bytes.len() as u64));
                }
                points.reserve(element.records);
                for _ in 0..element.records {
                    let mut v = [0.0_f32; 3];
                    for field in &element.fields {
                        let value = cursor.field(field)?;
                        // BY NAME, because the order is the writer's choice:
                        // `z y x` is legal and rare, and `x y z` after a
                        // confidence value is neither legal nor rare.
                        match field.name.as_str() {
                            "x" => v[0] = value.first().copied().unwrap_or(0.0),
                            "y" => v[1] = value.first().copied().unwrap_or(0.0),
                            "z" => v[2] = value.first().copied().unwrap_or(0.0),
                            _ => {}
                        }
                    }
                    points.push(v);
                }
            }
            "face" => {
                for _ in 0..element.records {
                    let mut corners: Vec<[f32; 3]> = Vec::new();
                    for field in &element.fields {
                        let value = cursor.field(field)?;
                        if field.name == "vertex_indices" || field.name == "vertex_index" {
                            for n in &value {
                                let at = usize::try_from(*n as i64).map_err(|_| Refusal::Failed)?;
                                corners.push(*points.get(at).ok_or(Refusal::Failed)?);
                            }
                        }
                    }
                    for k in 1..corners.len().saturating_sub(1) {
                        if mesh.triangles() >= TRIANGLE_CEILING {
                            return Err(Refusal::TooBig(bytes.len() as u64));
                        }
                        mesh.add(corners[0], corners[k], corners[k + 1]);
                    }
                }
            }
            // AN ELEMENT NOBODY WANTS IS STILL WALKED. Skipping it by seeking
            // would need its width, and a list property has no fixed width —
            // so it is read and thrown away, which is the only correct way to
            // arrive at the start of the element after it.
            _ => {
                for _ in 0..element.records {
                    for field in &element.fields {
                        cursor.field(field)?;
                    }
                }
            }
        }
    }
    if mesh.triangles() == 0 {
        return Err(Refusal::Failed);
    }
    let shape = if wire == Wire::Words {
        Shape::PlyAscii
    } else {
        Shape::PlyBinary
    };
    Ok(Read::of(mesh, shape))
}

/// The header: how the body is written, what is in it, and where it starts.
fn header(bytes: &[u8]) -> Result<(Wire, Vec<Element>, usize), Refusal> {
    // THE HEADER IS ASCII AND THE BODY MAY NOT BE, so the header is found as
    // bytes and only then read as text. Handing the whole file to
    // `from_utf8` would refuse every binary PLY there is.
    let end = find(bytes, b"end_header").ok_or(Refusal::Failed)?;
    let text = std::str::from_utf8(&bytes[..end]).map_err(|_| Refusal::Failed)?;
    let mut after = end + b"end_header".len();
    // The newline that closes the line, in either spelling.
    if bytes.get(after) == Some(&b'\r') {
        after += 1;
    }
    if bytes.get(after) == Some(&b'\n') {
        after += 1;
    }

    let mut lines = text.lines().map(str::trim);
    if lines.next() != Some("ply") {
        return Err(Refusal::Failed);
    }
    let mut wire = None;
    let mut elements: Vec<Element> = Vec::new();
    for line in lines {
        let mut words = line.split_ascii_whitespace();
        match words.next() {
            Some("format") => {
                wire = Some(match words.next() {
                    Some("ascii") => Wire::Words,
                    Some("binary_little_endian") => Wire::LittleEndian,
                    Some("binary_big_endian") => Wire::BigEndian,
                    _ => return Err(Refusal::Failed),
                });
            }
            Some("element") => {
                let name = words.next().ok_or(Refusal::Failed)?.to_owned();
                let records: usize = words
                    .next()
                    .ok_or(Refusal::Failed)?
                    .parse()
                    .map_err(|_| Refusal::Failed)?;
                elements.push(Element {
                    name,
                    records,
                    fields: Vec::new(),
                });
            }
            Some("property") => {
                let element = elements.last_mut().ok_or(Refusal::Failed)?;
                let first = words.next().ok_or(Refusal::Failed)?;
                let field = if first == "list" {
                    let counted_by =
                        Kind::named(words.next().ok_or(Refusal::Failed)?).ok_or(Refusal::Failed)?;
                    let kind =
                        Kind::named(words.next().ok_or(Refusal::Failed)?).ok_or(Refusal::Failed)?;
                    Field {
                        name: words.next().ok_or(Refusal::Failed)?.to_owned(),
                        kind,
                        counted_by: Some(counted_by),
                    }
                } else {
                    Field {
                        name: words.next().ok_or(Refusal::Failed)?.to_owned(),
                        kind: Kind::named(first).ok_or(Refusal::Failed)?,
                        counted_by: None,
                    }
                };
                element.fields.push(field);
            }
            // `comment`, `obj_info`, and anything a writer added.
            _ => {}
        }
    }
    Ok((wire.ok_or(Refusal::Failed)?, elements, after))
}

/// A position in the body, and how to read from it.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
    wire: Wire,
}

impl Cursor<'_> {
    /// One property's worth of numbers: one value, or a list of them.
    fn field(&mut self, field: &Field) -> Result<Vec<f32>, Refusal> {
        let Some(counted_by) = field.counted_by else {
            return Ok(vec![self.one(field.kind)?]);
        };
        let n = self.one(counted_by)?;
        if !(0.0..=1_048_576.0).contains(&n) {
            return Err(Refusal::Failed);
        }
        // `n` was just checked to be a small non-negative number.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = n as usize;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.one(field.kind)?);
        }
        Ok(out)
    }

    /// One number, in whichever spelling the header declared.
    ///
    /// Everything comes back as `f32`, which is what the mesh stores. An index
    /// fits: a `u32` beyond 2^24 would lose precision, and a file with sixteen
    /// million vertices is past [`TRIANGLE_CEILING`] before it gets here.
    fn one(&mut self, kind: Kind) -> Result<f32, Refusal> {
        if self.wire == Wire::Words {
            return self.word();
        }
        let width = kind.width();
        let end = self.at.checked_add(width).ok_or(Refusal::Failed)?;
        let slice = self.bytes.get(self.at..end).ok_or(Refusal::Failed)?;
        self.at = end;
        let mut buf = [0_u8; 8];
        buf[..width].copy_from_slice(slice);
        if self.wire == Wire::BigEndian {
            buf[..width].reverse();
        }
        let le = |n: usize| -> [u8; 8] {
            let mut out = [0_u8; 8];
            out[..n].copy_from_slice(&buf[..n]);
            out
        };
        #[allow(clippy::cast_precision_loss)]
        Ok(match kind {
            Kind::I8 => f32::from(buf[0] as i8),
            Kind::U8 => f32::from(buf[0]),
            Kind::I16 => f32::from(i16::from_le_bytes([buf[0], buf[1]])),
            Kind::U16 => f32::from(u16::from_le_bytes([buf[0], buf[1]])),
            Kind::I32 => i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as f32,
            Kind::U32 => u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as f32,
            Kind::F32 => f32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            Kind::F64 => f64::from_le_bytes(le(8)) as f32,
        })
    }

    /// One whitespace-separated number, for the ASCII spelling.
    fn word(&mut self) -> Result<f32, Refusal> {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| b.is_ascii_whitespace())
        {
            self.at += 1;
        }
        let from = self.at;
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| !b.is_ascii_whitespace())
        {
            self.at += 1;
        }
        if from == self.at {
            return Err(Refusal::Failed);
        }
        std::str::from_utf8(&self.bytes[from..self.at])
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .ok_or(Refusal::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::read;

    fn mesh(bytes: &[u8]) -> crate::mesh::Mesh {
        read(bytes).unwrap().body.mesh
    }

    const WORDS: &[u8] = b"ply\nformat ascii 1.0\nelement vertex 3\n\
property float x\nproperty float y\nproperty float z\n\
element face 1\nproperty list uchar int vertex_indices\nend_header\n\
0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n";

    #[test]
    fn a_property_this_reader_does_not_know_is_skipped_by_its_own_width() {
        // The colour is a `uchar` each and the confidence a `double`: eleven
        // bytes between this vertex's `z` and the next vertex's `x`. A reader
        // that assumed three floats would read the colour as coordinates.
        let mut bytes = b"ply\nformat binary_little_endian 1.0\nelement vertex 3\n\
property float x\nproperty float y\nproperty float z\n\
property uchar red\nproperty uchar green\nproperty uchar blue\n\
property double confidence\n\
element face 1\nproperty list uchar int vertex_indices\nend_header\n"
            .to_vec();
        for v in [[0.0_f32, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 0.0]] {
            for c in v {
                bytes.extend_from_slice(&c.to_le_bytes());
            }
            bytes.extend_from_slice(&[255, 128, 0]);
            bytes.extend_from_slice(&0.5_f64.to_le_bytes());
        }
        bytes.push(3);
        for i in [0_u32, 1, 2] {
            bytes.extend_from_slice(&i.to_le_bytes());
        }
        let m = mesh(&bytes);
        assert_eq!(m.triangles(), 1);
        assert_eq!(m.bounds.size(), [2.0, 3.0, 0.0]);
    }

    #[test]
    fn the_same_geometry_reads_the_same_in_all_three_spellings() {
        let words = mesh(WORDS);
        let mut little = b"ply\nformat binary_little_endian 1.0\nelement vertex 3\n\
property float x\nproperty float y\nproperty float z\n\
element face 1\nproperty list uchar int vertex_indices\nend_header\n"
            .to_vec();
        let mut big = little.clone();
        big = big.replace_range_of(b"binary_little_endian", b"binary_big_endian");
        for v in [[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for c in v {
                little.extend_from_slice(&c.to_le_bytes());
                big.extend_from_slice(&c.to_be_bytes());
            }
        }
        little.push(3);
        big.push(3);
        for i in [0_u32, 1, 2] {
            little.extend_from_slice(&i.to_le_bytes());
            big.extend_from_slice(&i.to_be_bytes());
        }
        assert_eq!(words.positions, mesh(&little).positions);
        assert_eq!(words.positions, mesh(&big).positions);
    }

    #[test]
    fn the_count_of_a_list_is_not_the_type_of_its_items() {
        // `uchar` count, `int` items. Reading the count as four bytes would
        // eat the first index.
        assert_eq!(mesh(WORDS).triangles(), 1);
    }

    #[test]
    fn an_element_nobody_wants_is_walked_and_not_seeked_past() {
        // An `edge` element with a list property sits between the vertices and
        // the faces. It has no fixed width, so arriving at the faces means
        // reading it.
        let bytes = b"ply\nformat ascii 1.0\nelement vertex 3\n\
property float x\nproperty float y\nproperty float z\n\
element edge 2\nproperty list uchar int ends\n\
element face 1\nproperty list uchar int vertex_indices\nend_header\n\
0 0 0\n1 0 0\n0 1 0\n2 0 1\n2 1 2\n3 0 1 2\n";
        assert_eq!(mesh(bytes).triangles(), 1);
    }

    #[test]
    fn a_quad_face_becomes_two_triangles() {
        let bytes = b"ply\nformat ascii 1.0\nelement vertex 4\n\
property float x\nproperty float y\nproperty float z\n\
element face 1\nproperty list uchar int vertex_indices\nend_header\n\
0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";
        assert_eq!(mesh(bytes).triangles(), 2);
    }

    #[test]
    fn a_body_that_stops_early_is_refused() {
        let bytes = b"ply\nformat ascii 1.0\nelement vertex 3\n\
property float x\nproperty float y\nproperty float z\n\
element face 1\nproperty list uchar int vertex_indices\nend_header\n\
0 0 0\n1 0 0\n";
        assert!(read(bytes).is_err());
    }

    /// A tiny helper so the big-endian fixture is the little-endian one with
    /// its declaration changed, rather than a second copy that can drift.
    trait Swap {
        fn replace_range_of(self, from: &[u8], to: &[u8]) -> Vec<u8>;
    }
    impl Swap for Vec<u8> {
        fn replace_range_of(self, from: &[u8], to: &[u8]) -> Vec<u8> {
            let at = super::find(&self, from).expect("the fixture lost its format line");
            let mut out = self[..at].to_vec();
            out.extend_from_slice(to);
            out.extend_from_slice(&self[at + from.len()..]);
            out
        }
    }
}
