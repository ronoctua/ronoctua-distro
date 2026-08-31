//! OFF: a word, three counts, the points, and the faces.
//!
//! # The two spellings of the header
//!
//! The counts may be on the line after the magic word or on the SAME line —
//! `OFF 8 6 12` is as common as `OFF\n8 6 12`, and both are in the wild. This
//! reader does not care, because it stops treating the file as lines the
//! moment the header word is behind it and reads numbers in order.
//!
//! # Why the third count is thrown away
//!
//! It is the edge count, and almost every writer puts `0` there. Trusting it
//! would mean a reader that fails on the majority of correct files.
//!
//! # The prefixes
//!
//! `COFF` carries a colour per vertex, `NOFF` a normal, `STOFF` a texture
//! coordinate, and they combine. All of them add numbers AFTER `x y z` on the
//! vertex line, and the vertex line is read as a line for exactly that reason:
//! the extra numbers are skipped by ending the line, not by counting them.

use super::{Mesh, Read, Shape, TRIANGLE_CEILING};
use crate::Refusal;

/// Reads an OFF.
///
/// # Errors
///
/// [`Refusal::Failed`] when the bytes are not UTF-8, the header is not one of
/// the OFF words, or the counts and the body disagree; [`Refusal::TooBig`]
/// past [`TRIANGLE_CEILING`].
pub fn read(bytes: &[u8]) -> Result<Read, Refusal> {
    let text = std::str::from_utf8(bytes).map_err(|_| Refusal::Failed)?;
    let mut lines = text.lines().map(strip).filter(|l| !l.is_empty());

    let first = lines.next().ok_or(Refusal::Failed)?;
    let mut head = first.split_ascii_whitespace();
    let word = head.next().ok_or(Refusal::Failed)?;
    if !word.ends_with("OFF") {
        return Err(Refusal::Failed);
    }
    // The counts, from the rest of the header line if it has them and from the
    // next line if it does not.
    let rest: Vec<&str> = head.collect();
    let counts: Vec<&str> = if rest.is_empty() {
        lines
            .next()
            .ok_or(Refusal::Failed)?
            .split_ascii_whitespace()
            .collect()
    } else {
        rest
    };
    if counts.len() < 2 {
        return Err(Refusal::Failed);
    }
    let points_wanted: usize = counts[0].parse().map_err(|_| Refusal::Failed)?;
    let faces_wanted: usize = counts[1].parse().map_err(|_| Refusal::Failed)?;
    // BEFORE ALLOCATING. A header may claim four billion vertices in twenty
    // bytes, and `Vec::with_capacity` on that is the whole machine.
    if points_wanted > TRIANGLE_CEILING * 3 || faces_wanted > TRIANGLE_CEILING {
        return Err(Refusal::TooBig(bytes.len() as u64));
    }

    let mut points: Vec<[f32; 3]> = Vec::with_capacity(points_wanted);
    for _ in 0..points_wanted {
        let line = lines.next().ok_or(Refusal::Failed)?;
        let mut n = line.split_ascii_whitespace();
        let mut v = [0.0_f32; 3];
        for slot in &mut v {
            *slot = n
                .next()
                .ok_or(Refusal::Failed)?
                .parse()
                .map_err(|_| Refusal::Failed)?;
        }
        points.push(v);
    }

    let mut mesh = Mesh::empty();
    for _ in 0..faces_wanted {
        let line = lines.next().ok_or(Refusal::Failed)?;
        let mut n = line.split_ascii_whitespace();
        let degree: usize = n
            .next()
            .ok_or(Refusal::Failed)?
            .parse()
            .map_err(|_| Refusal::Failed)?;
        let mut face: Vec<[f32; 3]> = Vec::with_capacity(degree);
        for _ in 0..degree {
            let at: usize = n
                .next()
                .ok_or(Refusal::Failed)?
                .parse()
                .map_err(|_| Refusal::Failed)?;
            face.push(*points.get(at).ok_or(Refusal::Failed)?);
        }
        for k in 1..face.len().saturating_sub(1) {
            if mesh.triangles() >= TRIANGLE_CEILING {
                return Err(Refusal::TooBig(bytes.len() as u64));
            }
            mesh.add(face[0], face[k], face[k + 1]);
        }
    }
    if mesh.triangles() == 0 {
        return Err(Refusal::Failed);
    }
    Ok(Read::of(mesh, Shape::Off))
}

/// A line without its comment, trimmed.
fn strip(line: &str) -> &str {
    line.split('#').next().unwrap_or("").trim()
}

#[cfg(test)]
mod tests {
    use super::read;

    const SQUARE: &str = "OFF\n4 1 0\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n";

    fn triangles(bytes: &[u8]) -> usize {
        read(bytes).unwrap().body.mesh.triangles()
    }

    #[test]
    fn the_counts_may_be_on_the_header_line_or_the_next_one() {
        let apart = triangles(SQUARE.as_bytes());
        let together = triangles(b"OFF 4 1 0\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n");
        assert_eq!(apart, 2);
        assert_eq!(apart, together);
    }

    #[test]
    fn a_vertex_line_with_a_colour_after_it_is_still_three_numbers() {
        // COFF. The colour is skipped by ending the line, not by counting.
        let out = read(
            b"COFF\n4 1 0\n0 0 0 255 0 0 255\n1 0 0 255 0 0 255\n\
              1 1 0 255 0 0 255\n0 1 0 255 0 0 255\n4 0 1 2 3\n",
        )
        .unwrap();
        assert_eq!(out.body.bounds().size()[0], 1.0);
    }

    #[test]
    fn a_comment_is_not_a_line_of_numbers() {
        assert_eq!(
            triangles(b"OFF\n# made by something\n4 1 0\n0 0 0\n1 0 0\n1 1 0\n0 1 0\n4 0 1 2 3\n"),
            2
        );
    }

    #[test]
    fn a_header_claiming_more_vertices_than_the_ceiling_is_refused_before_allocating() {
        // The point is that this RETURNS rather than trying to reserve.
        assert!(read(b"OFF\n4000000000 1 0\n").is_err());
    }

    #[test]
    fn an_index_outside_the_points_is_refused() {
        assert!(read(b"OFF\n3 1 0\n0 0 0\n1 0 0\n0 1 0\n3 0 1 9\n").is_err());
    }
}
