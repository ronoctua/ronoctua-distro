//! Wavefront OBJ: `v` for a point, `f` for a face, and everything else ignored.
//!
//! # The index that counts backwards
//!
//! An OBJ index is one-based, and a NEGATIVE one counts back from the end of
//! the list AS IT STANDS AT THAT LINE — `-1` is the vertex written just above.
//! The form is in the original specification and exporters use it, Blender's
//! among them, because it lets a writer emit a group of vertices and its faces
//! without keeping a running total. A reader that treats `-1` as out of range
//! finds no faces at all and reports a file with nothing in it.
//!
//! # Why the `.mtl` beside it is not opened
//!
//! Because the piece is drawn in ONE colour, which is a token — `mtllib` names
//! a file of materials this reader would parse in order to throw away. It is
//! also a second path off the disk for no pixel, and the rule for the glTF
//! reader next door applies here too.
//!
//! # What an OBJ does not say
//!
//! A unit. The format has none, so the numbers are taken as millimetres, which
//! is what the STL reader already assumes and what a slicer assumes of both.

use super::{Mesh, Read, Shape, TRIANGLE_CEILING};
use crate::Refusal;

/// Reads an OBJ.
///
/// # Errors
///
/// [`Refusal::Failed`] when the bytes are not UTF-8 or there is not a face in
/// them, and [`Refusal::TooBig`] past [`TRIANGLE_CEILING`].
pub fn read(bytes: &[u8]) -> Result<Read, Refusal> {
    let text = std::str::from_utf8(bytes).map_err(|_| Refusal::Failed)?;
    let mut points: Vec<[f32; 3]> = Vec::new();
    let mut mesh = Mesh::empty();
    // Reused across faces so an n-gon does not allocate per line.
    let mut face: Vec<[f32; 3]> = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        let mut words = line.split_ascii_whitespace();
        match words.next() {
            Some("v") => {
                // A fourth number is a weight for rational curves. It is not a
                // coordinate and dividing by it is not what it means.
                let mut v = [0.0_f32; 3];
                for slot in &mut v {
                    let Some(w) = words.next() else {
                        return Err(Refusal::Failed);
                    };
                    *slot = w.parse::<f32>().map_err(|_| Refusal::Failed)?;
                }
                points.push(v);
            }
            Some("f") => {
                face.clear();
                for w in words {
                    let Some(v) = corner(w, &points) else {
                        return Err(Refusal::Failed);
                    };
                    face.push(v);
                }
                // A FAN, which is right for the convex faces a writer emits and
                // is the only choice available: deciding a concave polygon's
                // triangulation needs its plane and an ear-clip, and an OBJ
                // n-gon that is concave is rare enough that paying for that on
                // every face would be paying for nobody.
                for k in 1..face.len().saturating_sub(1) {
                    if mesh.triangles() >= TRIANGLE_CEILING {
                        return Err(Refusal::TooBig(bytes.len() as u64));
                    }
                    mesh.add(face[0], face[k], face[k + 1]);
                }
            }
            // `vn`, `vt`, `vp`, `o`, `g`, `s`, `usemtl`, `mtllib`, `#` and
            // anything a writer invented. Normals are recomputed and the rest
            // is not geometry.
            _ => {}
        }
    }
    if mesh.triangles() == 0 {
        return Err(Refusal::Failed);
    }
    Ok(Read::of(mesh, Shape::Obj))
}

/// One corner of a face: `v`, `v/vt`, `v//vn` or `v/vt/vn`.
///
/// Only the first field is read. The texture coordinate is not geometry and
/// the normal is recomputed, so both are skipped without being parsed — a file
/// with a broken `vt` still draws.
fn corner(word: &str, points: &[[f32; 3]]) -> Option<[f32; 3]> {
    let first = word.split('/').next()?;
    let n = first.parse::<i64>().ok()?;
    let at = if n > 0 {
        usize::try_from(n - 1).ok()?
    } else if n < 0 {
        // BACK FROM THE END OF THE LIST AS IT STANDS NOW, which is why this
        // takes `points` and not a length decided in advance.
        let back = usize::try_from(-n).ok()?;
        points.len().checked_sub(back)?
    } else {
        // Zero is not an index in a one-based list.
        return None;
    };
    points.get(at).copied()
}

#[cfg(test)]
mod tests {
    use super::read;

    fn triangles(read: &crate::mesh::Read) -> usize {
        read.body.mesh.triangles()
    }

    #[test]
    fn a_negative_index_counts_back_from_the_end_of_the_list() {
        // The same triangle written both ways. If the negative form were read
        // as an error, or as an absolute index, the two would differ.
        let absolute = read(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
        let relative = read(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n").unwrap();
        assert_eq!(triangles(&absolute), 1);
        assert_eq!(absolute.body, relative.body);
    }

    #[test]
    fn a_negative_index_is_relative_to_the_line_it_is_on() {
        // Six vertices, and the second face's `-3 -2 -1` has to mean the last
        // three — not the last three of the first group.
        let out = read(
            b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n\
              v 0 0 9\nv 1 0 9\nv 0 1 9\nf -3 -2 -1\n",
        )
        .unwrap();
        assert_eq!(triangles(&out), 2);
        assert!(
            (out.body.bounds().size()[2] - 9.0).abs() < 1e-4,
            "the second face did not land at z = 9: {:?}",
            out.body.bounds()
        );
    }

    #[test]
    fn a_face_carries_only_its_first_field() {
        // `v/vt/vn` and `v//vn` and a bare `v` are the same geometry.
        let plain = read(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
        let full =
            read(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvn 0 0 1\nf 1/1/1 2/1/1 3/1/1\n").unwrap();
        let slashed = read(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nf 1//1 2//1 3//1\n").unwrap();
        assert_eq!(plain.body, full.body);
        assert_eq!(plain.body, slashed.body);
    }

    #[test]
    fn a_quad_becomes_two_triangles_and_a_pentagon_three() {
        let quad = read(b"v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n").unwrap();
        assert_eq!(triangles(&quad), 2);
        let five = read(b"v 0 0 0\nv 1 0 0\nv 2 1 0\nv 1 2 0\nv 0 2 0\nf 1 2 3 4 5\n").unwrap();
        assert_eq!(triangles(&five), 3);
    }

    #[test]
    fn the_normal_in_the_file_is_ignored_and_recomputed() {
        // A normal pointing the wrong way must not reach the rasteriser: the
        // cross product of the edges is what lights the face.
        let out = read(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 -1\nf 1//1 2//1 3//1\n").unwrap();
        let m = &out.body.mesh;
        assert!(
            m.normals[2] > 0.0,
            "the file's normal survived: {:?}",
            m.normals
        );
    }

    #[test]
    fn a_file_with_no_face_in_it_is_refused_and_not_drawn_empty() {
        assert!(read(b"v 0 0 0\nv 1 0 0\n# nothing else\n").is_err());
    }
}
