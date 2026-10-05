//! COLLADA: a scene graph, and geometry addressed by name from inside it.
//!
//! # The two things `<asset>` decides
//!
//! `<unit meter="0.001"/>` says what one number in this document is worth, and
//! `<up_axis>` says which way is up. Both are easy to skip and both are
//! visible when they are: a document in metres drawn as millimetres is a piece
//! a thousand times too large, and a `Y_UP` document — which is most of them,
//! because it is the default and what every art tool writes — drawn as though
//! it were `Z_UP` lies on its side. Neither is an error anywhere; both are
//! just wrong on screen.
//!
//! # Why `<p>` cannot be read as a list of indices
//!
//! Because it is INTERLEAVED. A `<triangles>` with a vertex input at offset 0
//! and a normal input at offset 1 writes two numbers per corner, and only
//! every other one is a vertex. The stride is the largest offset plus one, and
//! it is declared nowhere else — a reader that assumed one number per corner
//! would take normals for positions and draw noise.
//!
//! # Why the transforms are applied in document order
//!
//! A `<node>` may carry `<translate>`, `<rotate>` and `<scale>` in any order
//! and more than one of each, and the format says they compose in the order
//! they are written. Collecting them by name and applying them in a fixed
//! order gives a different placement for a rotate-then-translate node.

use super::xml::{attr, parse, tagged};
use super::{Mesh, Read, Shape, TRIANGLE_CEILING};
use crate::Refusal;
use ronoctua_xml::roxmltree;

/// How deep the node tree may go before it is called a loop.
const NESTING_CEILING: u32 = 64;

/// How deep a COLLADA document's XML may nest: the node tree at its ceiling,
/// and what sits around it.
///
/// A scene's `<node>`s ARE its nesting — a rig's skeleton passes twenty levels
/// — so the shared ceiling of 48 would refuse real files. Three elements hold
/// the first node (`COLLADA > library_visual_scenes > visual_scene`), and the
/// deepest thing a node holds is a skinned instance's five
/// (`instance_controller`, `bind_material`, `technique_common`,
/// `instance_material`, `bind_vertex_input`): at the walk's ceiling that is
/// 73, and 16 over the node tree leaves seven to spare.
///
/// MEASURED against the stack, not only the format: a scene at exactly this
/// depth is read whole on a 2 MiB thread in a debug build
/// (`a_scene_at_the_xml_ceiling_is_read_on_a_small_thread`) — the parse and
/// the walk's own recursion together.
const XML_DEPTH: usize = NESTING_CEILING as usize + 16;

/// Reads a COLLADA document.
///
/// # Errors
///
/// [`Refusal::Failed`] when the bytes are not XML, the root is not `<COLLADA>`
/// or there is no geometry in it, and [`Refusal::TooBig`] past
/// [`TRIANGLE_CEILING`].
pub fn read(bytes: &[u8]) -> Result<Read, Refusal> {
    let text = std::str::from_utf8(bytes).map_err(|_| Refusal::Failed)?;
    let doc = parse(text, XML_DEPTH)?;
    let root = doc.root_element();
    if !tagged(root, "COLLADA") {
        return Err(Refusal::Failed);
    }

    let asset = root.children().find(|n| tagged(*n, "asset"));
    let millimetres = asset
        .and_then(|a| a.children().find(|n| tagged(*n, "unit")))
        .and_then(|u| attr(u, "meter"))
        .and_then(|m| m.parse::<f32>().ok())
        .map_or(1.0, |metres| metres * 1000.0);
    let up = asset
        .and_then(|a| a.children().find(|n| tagged(*n, "up_axis")))
        .and_then(|n| n.text())
        .unwrap_or("Y_UP")
        .trim()
        .to_owned();

    let mut mesh = Mesh::empty();
    let mut objects = 0;

    // THE SCENE FIRST, and the bare geometry only when there is no scene: a
    // document with a `<library_visual_scenes>` places its geometry, and
    // drawing the library as well would draw every part twice — once placed
    // and once at the origin.
    let scenes: Vec<roxmltree::Node<'_, '_>> = doc
        .descendants()
        .filter(|n| tagged(*n, "visual_scene"))
        .collect();
    if scenes.is_empty() {
        for geometry in doc.descendants().filter(|n| tagged(*n, "geometry")) {
            objects += 1;
            add_geometry(geometry, &IDENTITY, &mut mesh)?;
        }
    } else {
        for scene in &scenes {
            for node in scene.children().filter(|n| tagged(*n, "node")) {
                walk(&doc, node, &IDENTITY, 0, &mut mesh, &mut objects)?;
            }
        }
    }
    if mesh.triangles() == 0 {
        return Err(Refusal::Failed);
    }

    // THE UNIT AND THE AXIS, in one pass over the finished triangles. Both are
    // linear, and doing them here rather than per corner keeps the readers
    // above free of the document's conventions.
    let mut out = Mesh::empty();
    out.positions.reserve(mesh.positions.len());
    for t in 0..mesh.triangles() {
        let c = mesh.corners(t);
        let fix = |v: [f32; 3]| {
            let v = standing(v, &up);
            [v[0] * millimetres, v[1] * millimetres, v[2] * millimetres]
        };
        out.add(fix(c[0]), fix(c[1]), fix(c[2]));
    }

    let mut read = Read::of(out, Shape::Collada);
    read.objects = objects;
    read.unit = Some(
        asset
            .and_then(|a| a.children().find(|n| tagged(*n, "unit")))
            .and_then(|u| attr(u, "name"))
            .unwrap_or("millimeter")
            .to_owned(),
    );
    if let Some(a) = asset {
        for (tag, name) in [
            ("authoring_tool", "Application"),
            ("author", "Designer"),
            ("copyright", "LicenseTerms"),
        ] {
            if let Some(v) = a
                .descendants()
                .find(|n| tagged(*n, tag))
                .and_then(|n| n.text())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                read.notes.push(((*name).to_owned(), v.to_owned()));
            }
        }
        if let Some(v) = a
            .children()
            .find(|n| tagged(*n, "created"))
            .and_then(|n| n.text())
        {
            read.notes
                .push(("CreationDate".to_owned(), v.trim().to_owned()));
        }
    }
    Ok(read)
}

/// A point turned so that the document's up axis is `+Z`, which is the one the
/// rasteriser draws against.
fn standing(v: [f32; 3], up: &str) -> [f32; 3] {
    match up {
        // A quarter turn about X sends `+Y` to `+Z`.
        "Y_UP" => [v[0], -v[2], v[1]],
        // A quarter turn about Y sends `+X` to `+Z`.
        "X_UP" => [-v[2], v[1], v[0]],
        _ => v,
    }
}

/// The identity, row-major as COLLADA writes a `<matrix>`.
const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// One node, its transforms, its instances and its children.
fn walk(
    doc: &roxmltree::Document<'_>,
    node: roxmltree::Node<'_, '_>,
    parent: &[f32; 16],
    depth: u32,
    mesh: &mut Mesh,
    objects: &mut usize,
) -> Result<(), Refusal> {
    if depth > NESTING_CEILING {
        return Err(Refusal::Failed);
    }
    let mut here = *parent;
    for child in node.children() {
        // IN DOCUMENT ORDER, which is what the format says composition means.
        let step = match child.tag_name().name() {
            "matrix" => numbers(child.text()).try_into().ok().map(|m: [f32; 16]| m),
            "translate" => three(child.text()).map(|t| {
                [
                    1.0, 0.0, 0.0, t[0], 0.0, 1.0, 0.0, t[1], 0.0, 0.0, 1.0, t[2], 0.0, 0.0, 0.0,
                    1.0,
                ]
            }),
            "scale" => three(child.text()).map(|s| {
                [
                    s[0], 0.0, 0.0, 0.0, 0.0, s[1], 0.0, 0.0, 0.0, 0.0, s[2], 0.0, 0.0, 0.0, 0.0,
                    1.0,
                ]
            }),
            "rotate" => {
                let n = numbers(child.text());
                (n.len() == 4).then(|| turn([n[0], n[1], n[2]], n[3]))
            }
            _ => None,
        };
        if let Some(step) = step {
            here = compose(&here, &step);
        }
    }
    for child in node.children() {
        match child.tag_name().name() {
            "instance_geometry" | "instance_node" => {
                let Some(url) = attr(child, "url") else {
                    continue;
                };
                let want = url.trim_start_matches('#');
                let Some(target) = doc.descendants().find(|n| attr(*n, "id") == Some(want)) else {
                    continue;
                };
                if tagged(target, "node") {
                    walk(doc, target, &here, depth + 1, mesh, objects)?;
                } else {
                    *objects += 1;
                    add_geometry(target, &here, mesh)?;
                }
            }
            "node" => walk(doc, child, &here, depth + 1, mesh, objects)?,
            _ => {}
        }
    }
    Ok(())
}

/// `a` then `b`, both row-major and applied to a column vector.
fn compose(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0_f32; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row * 4 + k] * b[k * 4 + col];
            }
            out[row * 4 + col] = sum;
        }
    }
    out
}

/// A turn of `degrees` about an axis, row-major.
fn turn(axis: [f32; 3], degrees: f32) -> [f32; 16] {
    let length = axis[0].hypot(axis[1]).hypot(axis[2]);
    if length <= 0.0 {
        return IDENTITY;
    }
    let (x, y, z) = (axis[0] / length, axis[1] / length, axis[2] / length);
    let (s, c) = degrees.to_radians().sin_cos();
    let k = 1.0 - c;
    [
        x * x * k + c,
        x * y * k - z * s,
        x * z * k + y * s,
        0.0,
        y * x * k + z * s,
        y * y * k + c,
        y * z * k - x * s,
        0.0,
        z * x * k - y * s,
        z * y * k + x * s,
        z * z * k + c,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}

/// A point through a row-major matrix.
fn through(m: &[f32; 16], v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[1] * v[1] + m[2] * v[2] + m[3],
        m[4] * v[0] + m[5] * v[1] + m[6] * v[2] + m[7],
        m[8] * v[0] + m[9] * v[1] + m[10] * v[2] + m[11],
    ]
}

/// Whitespace-separated numbers, in order.
fn numbers(text: Option<&str>) -> Vec<f32> {
    text.unwrap_or("")
        .split_ascii_whitespace()
        .filter_map(|w| w.parse::<f32>().ok())
        .collect()
}

/// Whitespace-separated whole numbers, in order.
fn indices(text: Option<&str>) -> Vec<usize> {
    text.unwrap_or("")
        .split_ascii_whitespace()
        .filter_map(|w| w.parse::<usize>().ok())
        .collect()
}

/// Exactly three numbers, or none.
fn three(text: Option<&str>) -> Option<[f32; 3]> {
    let n = numbers(text);
    (n.len() == 3).then(|| [n[0], n[1], n[2]])
}

/// One `<geometry>`, placed by `at`.
fn add_geometry(
    geometry: roxmltree::Node<'_, '_>,
    at: &[f32; 16],
    mesh: &mut Mesh,
) -> Result<(), Refusal> {
    let Some(source) = geometry.children().find(|n| tagged(*n, "mesh")) else {
        return Ok(());
    };
    for primitive in source.children() {
        let kind = primitive.tag_name().name();
        if !matches!(kind, "triangles" | "polylist" | "polygons") {
            continue;
        }
        // THE VERTEX INPUT AND THE STRIDE, which are the same question asked
        // twice: which of the interleaved numbers is a position, and how many
        // there are per corner.
        let mut stride = 0;
        let mut vertex_at = None;
        let mut vertex_source = None;
        for input in primitive.children().filter(|n| tagged(*n, "input")) {
            let offset = attr(input, "offset")
                .and_then(|o| o.parse::<usize>().ok())
                .unwrap_or(0);
            stride = stride.max(offset + 1);
            if attr(input, "semantic") == Some("VERTEX") {
                vertex_at = Some(offset);
                vertex_source = attr(input, "source").map(|s| s.trim_start_matches('#').to_owned());
            }
        }
        let (Some(vertex_at), Some(vertex_source)) = (vertex_at, vertex_source) else {
            continue;
        };
        let Some(points) = positions(source, &vertex_source) else {
            continue;
        };

        let corner = |i: usize| -> Option<[f32; 3]> { points.get(i).map(|v| through(at, *v)) };
        // `<polygons>` writes one `<p>` per face; the other two write one for
        // the whole primitive, with `<vcount>` saying where the faces divide.
        let faces: Vec<Vec<usize>> = if kind == "polygons" {
            primitive
                .children()
                .filter(|n| tagged(*n, "p"))
                .map(|p| indices(p.text()))
                .collect()
        } else {
            let flat = primitive
                .children()
                .find(|n| tagged(*n, "p"))
                .map(|p| indices(p.text()))
                .unwrap_or_default();
            let counts: Vec<usize> = primitive
                .children()
                .find(|n| tagged(*n, "vcount"))
                .map(|c| indices(c.text()))
                .unwrap_or_else(|| vec![3; flat.len() / (3 * stride.max(1))]);
            let mut out = Vec::with_capacity(counts.len());
            let mut from = 0;
            for n in counts {
                let to = from + n * stride;
                if to > flat.len() {
                    break;
                }
                out.push(flat[from..to].to_vec());
                from = to;
            }
            out
        };
        for face in faces {
            let picked: Vec<[f32; 3]> = face
                .chunks_exact(stride.max(1))
                .filter_map(|c| c.get(vertex_at).copied().and_then(corner))
                .collect();
            for k in 1..picked.len().saturating_sub(1) {
                if mesh.triangles() >= TRIANGLE_CEILING {
                    return Err(Refusal::TooBig(mesh.positions.len() as u64));
                }
                mesh.add(picked[0], picked[k], picked[k + 1]);
            }
        }
    }
    Ok(())
}

/// The points a `<vertices>` element names, through the `<source>` it points at.
fn positions(mesh: roxmltree::Node<'_, '_>, id: &str) -> Option<Vec<[f32; 3]>> {
    // `<vertices>` is one level of indirection the format insists on: it names
    // a `<source>`, which holds the `<float_array>`.
    let vertices = mesh
        .children()
        .find(|n| tagged(*n, "vertices") && attr(*n, "id") == Some(id));
    let source_id = match vertices {
        Some(v) => v
            .children()
            .find(|n| tagged(*n, "input") && attr(*n, "semantic") == Some("POSITION"))
            .and_then(|n| attr(n, "source"))
            .map(|s| s.trim_start_matches('#').to_owned())?,
        // A primitive may point at a `<source>` directly, which older
        // documents do.
        None => id.to_owned(),
    };
    let source = mesh
        .children()
        .find(|n| tagged(*n, "source") && attr(*n, "id") == Some(source_id.as_str()))?;
    let array = source.descendants().find(|n| tagged(*n, "float_array"))?;
    let flat = numbers(array.text());
    // THE STRIDE COMES FROM THE ACCESSOR, not from the assumption that a point
    // is three numbers: a source that also carries a `W` has a stride of four,
    // and reading it as three would shear the piece.
    let stride = source
        .descendants()
        .find(|n| tagged(*n, "accessor"))
        .and_then(|a| attr(a, "stride"))
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(3)
        .max(3);
    Some(
        flat.chunks_exact(stride)
            .map(|c| [c[0], c[1], c[2]])
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{NESTING_CEILING, XML_DEPTH, read};
    use crate::Refusal;

    /// A scene whose outer node holds `nodes` more, one inside the other, the
    /// deepest holding a skinned instance's chain — the deepest a node goes.
    fn deep(nodes: usize) -> String {
        let mut inner = "<node>".repeat(nodes);
        inner.push_str(
            r##"<instance_geometry url="#g"><bind_material><technique_common><instance_material symbol="m" target="#m"><bind_vertex_input semantic="UV" input_semantic="TEXCOORD"/></instance_material></technique_common></bind_material></instance_geometry>"##,
        );
        inner.push_str(&"</node>".repeat(nodes));
        document("<up_axis>Z_UP</up_axis>", &inner)
    }

    /// `read` on a thread with the stack a test thread and the inspector's
    /// get by default, so an overflow is this test's and not a pass.
    fn read_on_a_small_thread(text: String) -> Result<usize, Refusal> {
        std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || read(text.as_bytes()).map(|r| r.body.mesh.triangles()))
            .expect("a thread")
            .join()
            .expect("the read came back")
    }

    #[test]
    fn a_scene_at_the_xml_ceiling_is_read_on_a_small_thread() {
        // THE MEASUREMENT `XML_DEPTH` CITES: the walk's deepest node, holding
        // the deepest thing a node holds, parsed and walked on 2 MiB.
        let text = deep(NESTING_CEILING as usize);
        assert!(
            ronoctua_xml::shallow(&text, XML_DEPTH),
            "the fixture is under the ceiling"
        );
        assert!(
            !ronoctua_xml::shallow(&text, XML_DEPTH - 9),
            "the fixture is near the ceiling, or it measures nothing"
        );
        assert_eq!(read_on_a_small_thread(text), Ok(2));
    }

    #[test]
    fn a_scene_past_its_ceiling_is_refused_before_it_is_parsed() {
        // One level past the ceiling is a refusal, and a document made to run
        // the parser out of stack is the same refusal — on the same small
        // thread, so a parse that recursed would abort this test binary.
        let past = format!(
            "<COLLADA>{}{}</COLLADA>",
            "<a>".repeat(XML_DEPTH),
            "</a>".repeat(XML_DEPTH)
        );
        assert_eq!(read_on_a_small_thread(past), Err(Refusal::Failed));
        let bomb = format!("<COLLADA>{}</COLLADA>", "<node>".repeat(100_000));
        assert_eq!(read_on_a_small_thread(bomb), Err(Refusal::Failed));
    }

    /// One triangle, with whatever `<asset>` and node body is given.
    fn document(asset: &str, node: &str) -> String {
        format!(
            r##"<?xml version="1.0"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
  <asset>{asset}</asset>
  <library_geometries>
    <geometry id="g"><mesh>
      <source id="s"><float_array id="a" count="9">0 0 0  1 0 0  0 1 0</float_array>
        <technique_common><accessor source="#a" count="3" stride="3">
          <param name="X" type="float"/><param name="Y" type="float"/><param name="Z" type="float"/>
        </accessor></technique_common>
      </source>
      <vertices id="v"><input semantic="POSITION" source="#s"/></vertices>
      <triangles count="1">
        <input semantic="VERTEX" source="#v" offset="0"/>
        <input semantic="NORMAL" source="#s" offset="1"/>
        <p>0 0 1 1 2 2</p>
      </triangles>
    </mesh></geometry>
  </library_geometries>
  <library_visual_scenes><visual_scene id="scene">
    <node id="n">{node}<instance_geometry url="#g"/></node>
  </visual_scene></library_visual_scenes>
  <scene><instance_visual_scene url="#scene"/></scene>
</COLLADA>"##
        )
    }

    fn corners(text: &str) -> [[f32; 3]; 3] {
        let out = read(text.as_bytes()).unwrap();
        let m = out.body.mesh;
        assert_eq!(m.triangles(), 1, "the interleaved `p` was misread");
        m.corners(0)
    }

    #[test]
    fn a_document_that_says_y_is_up_stands_the_piece_up() {
        // THE DEFECT THIS CATCHES: ignoring `<up_axis>`, which lays every
        // document written by an art tool on its side.
        let lying = corners(&document("<up_axis>Z_UP</up_axis>", ""));
        let standing = corners(&document("<up_axis>Y_UP</up_axis>", ""));
        // The corner at (0, 1, 0) is up the Y axis in the document. Under
        // `Z_UP` it stays there; under `Y_UP` it becomes up the Z axis.
        assert!((lying[2][1] - 1.0).abs() < 1e-4, "{lying:?}");
        assert!((standing[2][2] - 1.0).abs() < 1e-4, "{standing:?}");
    }

    #[test]
    fn the_unit_the_document_declares_is_applied() {
        let millimetres = corners(&document(r#"<unit meter="0.001" name="millimeter"/>"#, ""));
        let metres = corners(&document(r#"<unit meter="1" name="meter"/>"#, ""));
        assert!((millimetres[1][0] - 1.0).abs() < 1e-4);
        assert!((metres[1][0] - 1000.0).abs() < 1e-2, "{metres:?}");
    }

    #[test]
    fn the_transforms_compose_in_the_order_they_are_written() {
        // Rotate then translate is not translate then rotate. Under `Z_UP` so
        // the axis conversion is out of the way.
        let up = "<up_axis>Z_UP</up_axis>";
        let first = corners(&document(
            up,
            "<rotate>0 0 1 90</rotate><translate>10 0 0</translate>",
        ));
        let second = corners(&document(
            up,
            "<translate>10 0 0</translate><rotate>0 0 1 90</rotate>",
        ));
        // Rotate-then-translate puts the origin corner at the rotated (10,0,0);
        // translate-then-rotate puts it at (10,0,0) unrotated.
        assert!(
            (first[0][1] - 10.0).abs() < 1e-3,
            "rotate then translate: {first:?}"
        );
        assert!(
            (second[0][0] - 10.0).abs() < 1e-3,
            "translate then rotate: {second:?}"
        );
    }

    #[test]
    fn an_interleaved_p_takes_only_the_vertex_offset() {
        // With a normal input at offset 1, half the numbers in `<p>` are not
        // vertices. Reading them all would make three triangles out of one.
        let out = corners(&document("<up_axis>Z_UP</up_axis>", ""));
        assert!((out[1][0] - 1.0).abs() < 1e-4, "{out:?}");
    }

    #[test]
    fn a_polylist_divides_its_indices_by_vcount() {
        let text = document("<up_axis>Z_UP</up_axis>", "").replace(
            r##"<triangles count="1">
        <input semantic="VERTEX" source="#v" offset="0"/>
        <input semantic="NORMAL" source="#s" offset="1"/>
        <p>0 0 1 1 2 2</p>
      </triangles>"##,
            r##"<polylist count="1">
        <input semantic="VERTEX" source="#v" offset="0"/>
        <vcount>3</vcount><p>0 1 2</p>
      </polylist>"##,
        );
        assert_eq!(corners(&text)[1][0], 1.0);
    }

    #[test]
    fn a_document_with_no_geometry_is_refused() {
        assert!(read(br#"<?xml version="1.0"?><COLLADA><asset/></COLLADA>"#).is_err());
    }

    #[test]
    fn a_root_that_is_not_collada_is_refused() {
        assert!(read(br#"<?xml version="1.0"?><svg/>"#).is_err());
    }
}
