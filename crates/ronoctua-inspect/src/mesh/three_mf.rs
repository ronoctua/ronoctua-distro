//! 3MF: a zip of XML that points at more XML.
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

use super::xml::{attr, tagged};
use super::{Bounds, Mesh, Read, Shape, TRIANGLE_CEILING};
use crate::Refusal;
use crate::zip::{member_bytes, members, part};

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
///
/// # Errors
///
/// [`Refusal::Failed`] when the zip, the relationships or the XML are not what
/// they claim, and [`Refusal::TooBig`] past a ceiling.
pub fn read(bytes: &[u8]) -> Result<Read, Refusal> {
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
        body: super::Body::of(mesh),
        shape: Shape::ThreeMf,
        unit,
        objects,
        notes,
        census: super::Census::default(),
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

#[cfg(test)]
mod tests {
    use super::{STILL, Shape, matrix, moved, read};
    use crate::mesh::fixture::{cube, model_part, zip_of};

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
        let read = read(&zip).expect("a 3MF reads");
        assert_eq!(read.shape, Shape::ThreeMf);
        assert_eq!(read.body.mesh.triangles(), 12);
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
        let read = read(&zip).expect("a 3MF reads");
        let size = read.body.bounds().size();
        assert!((size[0] - 10.0).abs() < 0.001, "x was {}", size[0]);
        assert!(
            (size[2] - 18.08).abs() < 0.001,
            "z was {}, not scaled",
            size[2]
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
