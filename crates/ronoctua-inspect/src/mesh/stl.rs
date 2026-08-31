//! STL, in both of its two spellings.
//!
//! The one thing worth knowing before reading it is in [`stl`]: which spelling
//! a file is written in is decided by ARITHMETIC and never by the first word.

use super::{Mesh, Read, Shape, TRIANGLE_CEILING};
use crate::Refusal;

/// Reads an STL of either spelling.
///
/// # Errors
///
/// [`Refusal::Failed`] when there is not a facet in it, [`Refusal::TooBig`]
/// past [`TRIANGLE_CEILING`].
pub fn read(bytes: &[u8]) -> Result<Read, Refusal> {
    let (mesh, shape) = stl(bytes)?;
    Ok(Read::of(mesh, shape))
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

#[cfg(test)]
mod tests {
    use super::{Shape, stl};
    use crate::mesh::fixture::{binary_stl, cube};

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
}
