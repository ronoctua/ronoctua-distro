//! AMF: the same geometry as a 3MF, in a container that may not be one.
//!
//! # The file may be a zip and may be the XML itself
//!
//! The specification allows both, under the same extension and with nothing in
//! the name to tell them apart — so the bytes decide: a zip begins `PK`, and
//! anything else is read as the document. Slic3r and Meshmixer write the
//! compressed form and hand-written files are plain, and a reader that
//! supported only one of them would fail on half the files in the world with
//! "this may be damaged".
//!
//! # Why the triangles are read as indices into the object's own vertices
//!
//! Because an AMF `<volume>` refers to `<vertices>` by position, and the
//! positions are per OBJECT. A file with two objects has two independent
//! numberings, and a reader that pooled them would draw the second object out
//! of the first object's points.

use super::xml::tagged;
use super::zip::{member_bytes, members};
use super::{Mesh, Read, Shape, TRIANGLE_CEILING};
use crate::Refusal;

/// Reads an AMF, compressed or not.
///
/// # Errors
///
/// [`Refusal::Failed`] when neither container holds an `<amf>` document with
/// geometry in it, and [`Refusal::TooBig`] past a ceiling.
pub fn read(bytes: &[u8]) -> Result<Read, Refusal> {
    if bytes.starts_with(b"PK") {
        // THE FIRST MEMBER THAT PARSES, and not the first member: a zip from a
        // slicer carries a thumbnail and a content-types part beside the
        // document, and which one is the model is not written down anywhere.
        let list = members(bytes)?;
        for m in &list {
            let Ok(inner) = member_bytes(bytes, m) else {
                continue;
            };
            if let Ok(read) = document(&inner) {
                return Ok(read);
            }
        }
        return Err(Refusal::Failed);
    }
    document(bytes)
}

/// One `<amf>` document.
fn document(bytes: &[u8]) -> Result<Read, Refusal> {
    let text = std::str::from_utf8(bytes).map_err(|_| Refusal::Failed)?;
    let doc = roxmltree::Document::parse(text).map_err(|_| Refusal::Failed)?;
    let root = doc.root_element();
    if !tagged(root, "amf") {
        return Err(Refusal::Failed);
    }
    let declared = root
        .attribute("unit")
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "millimeter".to_owned());
    let scale = millimetres_per(&declared);

    let mut mesh = Mesh::empty();
    let mut objects = 0;
    for object in root.children().filter(|n| tagged(*n, "object")) {
        objects += 1;
        for source in object.children().filter(|n| tagged(*n, "mesh")) {
            let points: Vec<[f32; 3]> = source
                .children()
                .filter(|n| tagged(*n, "vertices"))
                .flat_map(|v| v.children().filter(|n| tagged(*n, "vertex")))
                .filter_map(|v| {
                    let c = v.children().find(|n| tagged(*n, "coordinates"))?;
                    Some([
                        axis(c, "x")? * scale,
                        axis(c, "y")? * scale,
                        axis(c, "z")? * scale,
                    ])
                })
                .collect();
            for volume in source.children().filter(|n| tagged(*n, "volume")) {
                for t in volume.children().filter(|n| tagged(*n, "triangle")) {
                    let corner = |name: &str| -> Option<[f32; 3]> {
                        let at = t
                            .children()
                            .find(|n| tagged(*n, name))?
                            .text()?
                            .trim()
                            .parse::<usize>()
                            .ok()?;
                        points.get(at).copied()
                    };
                    let (Some(a), Some(b), Some(c)) = (corner("v1"), corner("v2"), corner("v3"))
                    else {
                        continue;
                    };
                    if mesh.triangles() >= TRIANGLE_CEILING {
                        return Err(Refusal::TooBig(bytes.len() as u64));
                    }
                    mesh.add(a, b, c);
                }
            }
        }
    }
    if mesh.triangles() == 0 {
        return Err(Refusal::Failed);
    }
    let mut out = Read::of(mesh, Shape::Amf);
    out.objects = objects;
    out.unit = Some(declared);
    // THE NAMES ARE 3MF'S, so the panel has one map from name to label. AMF
    // spells the same three facts in its own `<metadata type="…">`.
    for m in root.children().filter(|n| tagged(*n, "metadata")) {
        let Some(kind) = m.attribute("type") else {
            continue;
        };
        let Some(value) = m.text().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let name = match kind {
            "name" => "Title",
            "author" => "Designer",
            "producer" => "Application",
            _ => continue,
        };
        out.notes.push((name.to_owned(), value.to_owned()));
    }
    Ok(out)
}

/// One coordinate of a `<coordinates>` element.
fn axis(node: roxmltree::Node<'_, '_>, name: &str) -> Option<f32> {
    node.children()
        .find(|n| tagged(*n, name))?
        .text()?
        .trim()
        .parse()
        .ok()
}

/// How many millimetres one of the file's units is.
///
/// The five the specification names. Anything else is left alone rather than
/// guessed: a factor invented here would be a piece silently the wrong size.
fn millimetres_per(unit: &str) -> f32 {
    match unit {
        "inch" => 25.4,
        "foot" | "feet" => 304.8,
        "meter" | "metre" => 1000.0,
        "micron" | "micrometer" => 0.001,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::read;
    use crate::mesh::fixture::zip_of;

    const PLAIN: &str = r#"<?xml version="1.0"?>
<amf unit="millimeter">
  <metadata type="producer">a test</metadata>
  <object id="0"><mesh>
    <vertices>
      <vertex><coordinates><x>0</x><y>0</y><z>0</z></coordinates></vertex>
      <vertex><coordinates><x>1</x><y>0</y><z>0</z></coordinates></vertex>
      <vertex><coordinates><x>0</x><y>1</y><z>0</z></coordinates></vertex>
    </vertices>
    <volume><triangle><v1>0</v1><v2>1</v2><v3>2</v3></triangle></volume>
  </mesh></object>
</amf>"#;

    fn body(bytes: &[u8]) -> crate::mesh::Mesh {
        read(bytes).unwrap().body.mesh
    }

    #[test]
    fn an_amf_is_read_whether_it_is_zipped_or_not() {
        // THE DEFECT THIS CATCHES: supporting one container, which fails on
        // half the files with a sentence saying they are damaged.
        let plain = body(PLAIN.as_bytes());
        let zipped = body(&zip_of(&[
            ("[Content_Types].xml", b"<Types/>".to_vec(), false),
            ("model.amf", PLAIN.as_bytes().to_vec(), true),
        ]));
        assert_eq!(plain.triangles(), 1);
        assert_eq!(plain.positions, zipped.positions);
    }

    #[test]
    fn the_unit_the_document_declares_is_applied() {
        let inches = PLAIN.replace(r#"unit="millimeter""#, r#"unit="inch""#);
        assert!((body(inches.as_bytes()).bounds.size()[0] - 25.4).abs() < 1e-3);
    }

    #[test]
    fn a_volume_indexes_its_own_object_s_vertices() {
        // Two objects, each with three vertices numbered from zero. Pooling
        // them would draw the second object out of the first one's points.
        let two = PLAIN.replace(
            "</object>\n</amf>",
            r#"</object>
  <object id="1"><mesh>
    <vertices>
      <vertex><coordinates><x>10</x><y>10</y><z>10</z></coordinates></vertex>
      <vertex><coordinates><x>11</x><y>10</y><z>10</z></coordinates></vertex>
      <vertex><coordinates><x>10</x><y>11</y><z>10</z></coordinates></vertex>
    </vertices>
    <volume><triangle><v1>0</v1><v2>1</v2><v3>2</v3></triangle></volume>
  </mesh></object>
</amf>"#,
        );
        let m = body(two.as_bytes());
        assert_eq!(m.triangles(), 2);
        assert!((m.bounds.size()[2] - 10.0).abs() < 1e-4, "{:?}", m.bounds);
    }

    #[test]
    fn a_document_that_is_not_an_amf_is_refused() {
        assert!(read(br#"<?xml version="1.0"?><COLLADA/>"#).is_err());
    }
}
