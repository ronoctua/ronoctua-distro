//! glTF 2, in both of its containers.
//!
//! # Why the node graph has to be walked
//!
//! Because a glTF scene is a TREE and the geometry is at its leaves. A file
//! exported from a CAD tool has one node per part, each with its own transform,
//! and the mesh under it is written in the part's own coordinates. A reader
//! that took the meshes and ignored the nodes would draw every part on top of
//! every other part at the origin — a recognisable pile, and wrong, with
//! nothing saying so.
//!
//! # Why a `.gltf` is given the file's path and nothing else here is
//!
//! Because it is the only format whose geometry may be in a SECOND file: a
//! buffer is named by URI, and a relative URI is a sibling on disk. That makes
//! this reader the only one that opens a path it was not handed, so it is also
//! the only one that has to refuse the paths it will not open — see [`beside`].
//!
//! # Why Draco is a refusal and not a partial drawing
//!
//! `KHR_draco_mesh_compression` replaces a primitive's accessors with a
//! compressed blob, and decoding it is a library this crate does not have. The
//! primitives that are NOT compressed would still read — and drawing those is
//! showing part of a model with nothing saying which part is missing. A file
//! that says it REQUIRES an extension is refused whole, with a sentence that
//! says the file is fine and this reader is not.
//!
//! # The unit
//!
//! glTF's is the metre, by the specification and not by convention of a tool.
//! Everything else in this module is millimetres, so the numbers are scaled on
//! the way in and the panel says the unit came from the format.

use std::path::Path;

use serde_json::Value;

use super::{Mesh, Read, Shape, TRIANGLE_CEILING};
use crate::Refusal;

/// How many millimetres one glTF unit is.
const MILLIMETRES_PER_UNIT: f32 = 1000.0;

/// How deep the node tree may go before it is called a loop.
///
/// Nothing in the format forbids a node from being its own descendant, and a
/// file that is one would walk for ever. Sixty-four is past any exporter.
const NESTING_CEILING: u32 = 64;

/// The chunk type that holds the JSON, as the container spells it.
const CHUNK_JSON: u32 = 0x4E4F_534A;

/// The chunk type that holds the geometry.
const CHUNK_BIN: u32 = 0x004E_4942;

/// Reads a `.gltf`: JSON, and buffers that may be beside it.
///
/// # Errors
///
/// [`Refusal::Failed`] when the JSON is not a glTF or a buffer cannot be
/// resolved, [`Refusal::Unsupported`] when the file requires an extension this
/// reader does not have, and [`Refusal::TooBig`] past [`TRIANGLE_CEILING`].
pub fn read(bytes: &[u8], path: &Path) -> Result<Read, Refusal> {
    let doc: Value = serde_json::from_slice(bytes).map_err(|_| Refusal::Failed)?;
    build(&doc, None, path, Shape::Gltf)
}

/// Reads a `.glb`: a header, a JSON chunk, and usually a binary one.
///
/// # Errors
///
/// As [`read`], plus [`Refusal::Failed`] when the container's own header does
/// not describe the bytes that follow it.
pub fn read_binary(bytes: &[u8]) -> Result<Read, Refusal> {
    if bytes.len() < 12 || &bytes[..4] != b"glTF" {
        return Err(Refusal::Failed);
    }
    let mut at = 12;
    let mut json: Option<&[u8]> = None;
    let mut binary: Option<&[u8]> = None;
    while at + 8 <= bytes.len() {
        let length = u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        let kind = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]]);
        let from = at + 8;
        let to = from
            .checked_add(usize::try_from(length).map_err(|_| Refusal::Failed)?)
            .ok_or(Refusal::Failed)?;
        let chunk = bytes.get(from..to).ok_or(Refusal::Failed)?;
        match kind {
            CHUNK_JSON => json = Some(chunk),
            CHUNK_BIN => binary = Some(chunk),
            // A chunk type this reader does not know is skipped, which the
            // specification asks for by name.
            _ => {}
        }
        // Every chunk is padded to a multiple of four.
        at = to.next_multiple_of(4);
    }
    let doc: Value =
        serde_json::from_slice(json.ok_or(Refusal::Failed)?).map_err(|_| Refusal::Failed)?;
    build(&doc, binary, Path::new(""), Shape::Glb)
}

/// The triangles a parsed document describes.
fn build(doc: &Value, embedded: Option<&[u8]>, path: &Path, shape: Shape) -> Result<Read, Refusal> {
    // WHAT THE FILE SAYS IT NEEDS, before anything is decoded. `extensionsUsed`
    // is a courtesy and `extensionsRequired` is a contract: a file that names
    // one this reader does not have cannot be drawn correctly, and drawing it
    // incorrectly is what this arm exists to prevent.
    if let Some(list) = doc.get("extensionsRequired").and_then(Value::as_array) {
        for want in list.iter().filter_map(Value::as_str) {
            if want == "KHR_draco_mesh_compression" {
                return Err(Refusal::Unsupported(crate::key::PREVIEW_DRACO));
            }
            // Every other required extension: the geometry may still be plain,
            // and the ones in the wild that are not — mesh quantisation and
            // texture transforms — either change the numbers or do not touch
            // them at all. `KHR_mesh_quantization` changes them, so it is
            // refused with the same sentence rather than drawn scaled wrong.
            if want == "KHR_mesh_quantization" {
                return Err(Refusal::Unsupported(crate::key::PREVIEW_DRACO));
            }
        }
    }

    let buffers = load_buffers(doc, embedded, path)?;
    let mut mesh = Mesh::empty();
    let mut objects = 0;

    let nodes = doc.get("nodes").and_then(Value::as_array);
    let roots = scene_roots(doc);
    match (nodes, roots) {
        (Some(nodes), Some(roots)) => {
            for root in roots {
                walk(
                    doc,
                    nodes,
                    &buffers,
                    root,
                    &IDENTITY,
                    0,
                    &mut mesh,
                    &mut objects,
                )?;
            }
        }
        // NO SCENE AND NO NODES is a legal file that a converter writes: the
        // meshes are there and nothing places them. They are drawn where they
        // are written, which is the only reading available.
        _ => {
            let meshes = doc.get("meshes").and_then(Value::as_array);
            for (at, _) in meshes.into_iter().flatten().enumerate() {
                objects += 1;
                add_mesh(doc, &buffers, at, &IDENTITY, &mut mesh)?;
            }
        }
    }
    if mesh.triangles() == 0 {
        return Err(Refusal::Failed);
    }
    // TO MILLIMETRES, and the box is rebuilt rather than scaled: `Bounds` is
    // two corners and scaling them is right only while the scale is positive,
    // which is a condition nobody would remember to keep.
    let mut scaled = Mesh::empty();
    scaled.positions.reserve(mesh.positions.len());
    for t in 0..mesh.triangles() {
        let c = mesh.corners(t);
        let s = |v: [f32; 3]| {
            [
                v[0] * MILLIMETRES_PER_UNIT,
                v[1] * MILLIMETRES_PER_UNIT,
                v[2] * MILLIMETRES_PER_UNIT,
            ]
        };
        scaled.add(s(c[0]), s(c[1]), s(c[2]));
    }

    let mut out = Read::of(scaled, shape);
    out.objects = objects;
    out.unit = Some(crate::key::VALUE_UNIT_BY_CONVENTION.to_owned());
    if let Some(g) = doc
        .get("asset")
        .and_then(|a| a.get("generator"))
        .and_then(Value::as_str)
    {
        out.notes.push(("Application".to_owned(), g.to_owned()));
    }
    if let Some(c) = doc
        .get("asset")
        .and_then(|a| a.get("copyright"))
        .and_then(Value::as_str)
    {
        out.notes.push(("LicenseTerms".to_owned(), c.to_owned()));
    }
    Ok(out)
}

/// The nodes a scene starts from, when the document names one.
fn scene_roots(doc: &Value) -> Option<Vec<usize>> {
    let scenes = doc.get("scenes")?.as_array()?;
    let which = doc
        .get("scene")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0);
    let scene = scenes.get(which).or_else(|| scenes.first())?;
    Some(
        scene
            .get("nodes")?
            .as_array()?
            .iter()
            .filter_map(Value::as_u64)
            .filter_map(|n| usize::try_from(n).ok())
            .collect(),
    )
}

/// The identity transform, column-major as glTF writes one.
const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// One node, its transform composed with its parent's, and its children.
#[allow(clippy::too_many_arguments)]
fn walk(
    doc: &Value,
    nodes: &[Value],
    buffers: &[Vec<u8>],
    at: usize,
    parent: &[f32; 16],
    depth: u32,
    mesh: &mut Mesh,
    objects: &mut usize,
) -> Result<(), Refusal> {
    if depth > NESTING_CEILING {
        return Err(Refusal::Failed);
    }
    let Some(node) = nodes.get(at) else {
        return Ok(());
    };
    let here = compose(parent, &local(node));
    if let Some(which) = node
        .get("mesh")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
    {
        *objects += 1;
        add_mesh(doc, buffers, which, &here, mesh)?;
    }
    for child in node
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .filter_map(|n| usize::try_from(n).ok())
    {
        walk(doc, nodes, buffers, child, &here, depth + 1, mesh, objects)?;
    }
    Ok(())
}

/// A node's own transform: a matrix, or translation-rotation-scale.
fn local(node: &Value) -> [f32; 16] {
    if let Some(m) = node.get("matrix").and_then(Value::as_array)
        && m.len() == 16
    {
        let mut out = IDENTITY;
        for (slot, v) in out.iter_mut().zip(m) {
            *slot = number(v);
        }
        return out;
    }
    let t = triple(node.get("translation"), [0.0, 0.0, 0.0]);
    let s = triple(node.get("scale"), [1.0, 1.0, 1.0]);
    let r = node
        .get("rotation")
        .and_then(Value::as_array)
        .filter(|a| a.len() == 4)
        .map_or([0.0, 0.0, 0.0, 1.0], |a| {
            [number(&a[0]), number(&a[1]), number(&a[2]), number(&a[3])]
        });
    // The quaternion's rotation matrix, then the scale down its columns and
    // the translation in the last one. Column-major, as the format writes it.
    let (x, y, z, w) = (r[0], r[1], r[2], r[3]);
    let m = [
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
    ];
    [
        m[0] * s[0],
        m[1] * s[0],
        m[2] * s[0],
        0.0,
        m[3] * s[1],
        m[4] * s[1],
        m[5] * s[1],
        0.0,
        m[6] * s[2],
        m[7] * s[2],
        m[8] * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ]
}

/// Three numbers, or a default.
fn triple(v: Option<&Value>, or: [f32; 3]) -> [f32; 3] {
    v.and_then(Value::as_array)
        .filter(|a| a.len() == 3)
        .map_or(or, |a| [number(&a[0]), number(&a[1]), number(&a[2])])
}

/// A JSON number as an `f32`, and zero for anything else.
#[allow(clippy::cast_possible_truncation)]
fn number(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

/// `a` then `b`, both column-major.
fn compose(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0_f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = sum;
        }
    }
    out
}

/// A point through a column-major matrix.
fn through(m: &[f32; 16], v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[4] * v[1] + m[8] * v[2] + m[12],
        m[1] * v[0] + m[5] * v[1] + m[9] * v[2] + m[13],
        m[2] * v[0] + m[6] * v[1] + m[10] * v[2] + m[14],
    ]
}

/// Every primitive of one mesh, placed by `at`.
fn add_mesh(
    doc: &Value,
    buffers: &[Vec<u8>],
    which: usize,
    at: &[f32; 16],
    mesh: &mut Mesh,
) -> Result<(), Refusal> {
    let Some(source) = doc
        .get("meshes")
        .and_then(Value::as_array)
        .and_then(|m| m.get(which))
    else {
        return Ok(());
    };
    for prim in source
        .get("primitives")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        // MODE 4 IS THE DEFAULT and the one exporters write; 5 and 6 are the
        // strip and the fan. Everything below 4 is points and lines, which
        // have no surface and are not drawn here.
        let mode = prim.get("mode").and_then(Value::as_u64).unwrap_or(4);
        if !matches!(mode, 4..=6) {
            continue;
        }
        let Some(slot) = prim
            .get("attributes")
            .and_then(|a| a.get("POSITION"))
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
        else {
            continue;
        };
        let points = accessor(doc, buffers, slot)?;
        let corners: Vec<[f32; 3]> = points
            .chunks_exact(3)
            .map(|c| through(at, [c[0], c[1], c[2]]))
            .collect();
        let order: Vec<usize> = match prim
            .get("indices")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
        {
            Some(slot) => accessor(doc, buffers, slot)?
                .into_iter()
                .map(|n| n as usize)
                .collect(),
            None => (0..corners.len()).collect(),
        };
        let take = |i: usize| -> Result<[f32; 3], Refusal> {
            corners
                .get(*order.get(i).ok_or(Refusal::Failed)?)
                .copied()
                .ok_or(Refusal::Failed)
        };
        let triangles: Vec<[usize; 3]> = match mode {
            5 => (0..order.len().saturating_sub(2))
                // A STRIP ALTERNATES ITS WINDING, and a reader that did not
                // would hand every other triangle a backwards normal — which
                // the rasteriser turns to face the viewer, so the piece would
                // look right and the "faces reversed" fact would be a lie.
                .map(|i| {
                    if i % 2 == 0 {
                        [i, i + 1, i + 2]
                    } else {
                        [i + 1, i, i + 2]
                    }
                })
                .collect(),
            6 => (1..order.len().saturating_sub(1))
                .map(|i| [0, i, i + 1])
                .collect(),
            _ => (0..order.len() / 3)
                .map(|i| [i * 3, i * 3 + 1, i * 3 + 2])
                .collect(),
        };
        for t in triangles {
            if mesh.triangles() >= TRIANGLE_CEILING {
                return Err(Refusal::TooBig(mesh.positions.len() as u64));
            }
            mesh.add(take(t[0])?, take(t[1])?, take(t[2])?);
        }
    }
    Ok(())
}

/// One accessor's numbers, flattened.
///
/// Everything comes back as `f32`: a position is one, and an index fits — the
/// triangle ceiling is reached long before 2^24, which is where an `f32` stops
/// counting whole numbers.
fn accessor(doc: &Value, buffers: &[Vec<u8>], which: usize) -> Result<Vec<f32>, Refusal> {
    let a = doc
        .get("accessors")
        .and_then(Value::as_array)
        .and_then(|v| v.get(which))
        .ok_or(Refusal::Failed)?;
    let count = usize::try_from(
        a.get("count")
            .and_then(Value::as_u64)
            .ok_or(Refusal::Failed)?,
    )
    .map_err(|_| Refusal::Failed)?;
    let per = match a
        .get("type")
        .and_then(Value::as_str)
        .ok_or(Refusal::Failed)?
    {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        // A matrix is not geometry this reader asks for.
        _ => return Err(Refusal::Failed),
    };
    let kind = a
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or(Refusal::Failed)?;
    let width = component_width(kind)?;
    if count.saturating_mul(per) > TRIANGLE_CEILING * 4 {
        return Err(Refusal::TooBig(count as u64));
    }

    // AN ACCESSOR WITHOUT A VIEW IS ZEROS, which is what the specification
    // says and is only ever useful under a sparse overlay.
    let mut out = vec![0.0_f32; count * per];
    if let Some(view) = a
        .get("bufferView")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
    {
        let (bytes, stride) = view_of(doc, buffers, view)?;
        let start = offset(a);
        // The stride is between ELEMENTS and not between components; zero
        // means tightly packed, which is `per * width`.
        let step = if stride == 0 { per * width } else { stride };
        for i in 0..count {
            for k in 0..per {
                let at = start + i * step + k * width;
                out[i * per + k] = scalar(bytes, at, kind, width)?;
            }
        }
    }

    // THE SPARSE OVERLAY, and it is read rather than ignored. Ignoring it
    // gives back the base values, which are the WRONG positions with no error
    // anywhere — the failure this module is written to avoid.
    if let Some(sparse) = a.get("sparse") {
        let n = usize::try_from(
            sparse
                .get("count")
                .and_then(Value::as_u64)
                .ok_or(Refusal::Failed)?,
        )
        .map_err(|_| Refusal::Failed)?;
        let where_ = sparse.get("indices").ok_or(Refusal::Failed)?;
        let what = sparse.get("values").ok_or(Refusal::Failed)?;
        let index_kind = where_
            .get("componentType")
            .and_then(Value::as_u64)
            .ok_or(Refusal::Failed)?;
        let index_width = component_width(index_kind)?;
        let (index_bytes, _) = view_of(
            doc,
            buffers,
            usize::try_from(
                where_
                    .get("bufferView")
                    .and_then(Value::as_u64)
                    .ok_or(Refusal::Failed)?,
            )
            .map_err(|_| Refusal::Failed)?,
        )?;
        let (value_bytes, _) = view_of(
            doc,
            buffers,
            usize::try_from(
                what.get("bufferView")
                    .and_then(Value::as_u64)
                    .ok_or(Refusal::Failed)?,
            )
            .map_err(|_| Refusal::Failed)?,
        )?;
        let index_from = offset(where_);
        let value_from = offset(what);
        for i in 0..n {
            let at = scalar(
                index_bytes,
                index_from + i * index_width,
                index_kind,
                index_width,
            )?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let at = at as usize;
            for k in 0..per {
                let v = scalar(value_bytes, value_from + (i * per + k) * width, kind, width)?;
                *out.get_mut(at * per + k).ok_or(Refusal::Failed)? = v;
            }
        }
    }
    Ok(out)
}

/// An element's `byteOffset`, which is optional and defaults to none.
fn offset(v: &Value) -> usize {
    v.get("byteOffset")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0)
}

/// How wide one component of this type is.
const fn component_width(kind: u64) -> Result<usize, Refusal> {
    Ok(match kind {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return Err(Refusal::Failed),
    })
}

/// One number out of a buffer.
fn scalar(bytes: &[u8], at: usize, kind: u64, width: usize) -> Result<f32, Refusal> {
    let end = at.checked_add(width).ok_or(Refusal::Failed)?;
    let b = bytes.get(at..end).ok_or(Refusal::Failed)?;
    #[allow(clippy::cast_precision_loss)]
    Ok(match kind {
        5120 => f32::from(b[0] as i8),
        5121 => f32::from(b[0]),
        5122 => f32::from(i16::from_le_bytes([b[0], b[1]])),
        5123 => f32::from(u16::from_le_bytes([b[0], b[1]])),
        5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
        5126 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        _ => return Err(Refusal::Failed),
    })
}

/// The bytes a buffer view covers, and the stride declared over them.
fn view_of<'a>(
    doc: &Value,
    buffers: &'a [Vec<u8>],
    which: usize,
) -> Result<(&'a [u8], usize), Refusal> {
    let v = doc
        .get("bufferViews")
        .and_then(Value::as_array)
        .and_then(|a| a.get(which))
        .ok_or(Refusal::Failed)?;
    let buffer = usize::try_from(v.get("buffer").and_then(Value::as_u64).unwrap_or(0))
        .map_err(|_| Refusal::Failed)?;
    let bytes = buffers.get(buffer).ok_or(Refusal::Failed)?;
    let from = offset(v);
    let length = usize::try_from(
        v.get("byteLength")
            .and_then(Value::as_u64)
            .ok_or(Refusal::Failed)?,
    )
    .map_err(|_| Refusal::Failed)?;
    let to = from.checked_add(length).ok_or(Refusal::Failed)?;
    let slice = bytes.get(from..to).ok_or(Refusal::Failed)?;
    let stride = usize::try_from(v.get("byteStride").and_then(Value::as_u64).unwrap_or(0))
        .map_err(|_| Refusal::Failed)?;
    Ok((slice, stride))
}

/// Every buffer the document declares, in order.
fn load_buffers(
    doc: &Value,
    embedded: Option<&[u8]>,
    path: &Path,
) -> Result<Vec<Vec<u8>>, Refusal> {
    let mut out = Vec::new();
    for b in doc
        .get("buffers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match b.get("uri").and_then(Value::as_str) {
            // THE FIRST BUFFER WITH NO URI IS THE CONTAINER'S OWN, which is
            // how a `.glb` says "the geometry is in the chunk after me".
            None => out.push(embedded.map(<[u8]>::to_vec).ok_or(Refusal::Failed)?),
            Some(uri) if uri.starts_with("data:") => {
                let after = uri.split_once(";base64,").ok_or(Refusal::Failed)?.1;
                out.push(unbase64(after).ok_or(Refusal::Failed)?);
            }
            Some(uri) => out.push(beside(path, uri)?),
        }
    }
    Ok(out)
}

/// A sibling file, and only a sibling.
///
/// # Why a path from the file is refused rather than resolved
///
/// Because this process reads as the person looking at the folder, and a glTF
/// is a document somebody else wrote. A URI of `../../.ssh/id_ed25519` would
/// otherwise be a file this reader opens because a stranger asked it to. It
/// would find no triangles and draw nothing — but it would have read the file,
/// and "it did not draw what it read" is not a security property.
///
/// So: no absolute path, no parent segment, and no scheme. What is left is a
/// name under the folder the document is in, which is what every exporter in
/// the world writes.
fn beside(path: &Path, uri: &str) -> Result<Vec<u8>, Refusal> {
    let decoded = unpercent(uri);
    let candidate = Path::new(&decoded);
    if decoded.contains("://") || candidate.is_absolute() {
        return Err(Refusal::Failed);
    }
    if candidate
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(Refusal::Failed);
    }
    let folder = path.parent().ok_or(Refusal::Failed)?;
    super::slurp(&folder.join(candidate), super::FILE_CEILING)
}

/// A URI's `%20`s turned back into what they stand for.
fn unpercent(text: &str) -> String {
    let raw = text.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%'
            && i + 2 < raw.len()
            && let (Some(hi), Some(lo)) = (
                (raw[i + 1] as char).to_digit(16),
                (raw[i + 2] as char).to_digit(16),
            )
        {
            #[allow(clippy::cast_possible_truncation)]
            out.push((hi * 16 + lo) as u8);
            i += 3;
            continue;
        }
        out.push(raw[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Base64, decoded.
///
/// Twenty lines rather than a crate: this is the only place in the workspace
/// that needs it, and a lock entry for twenty lines is a dependency taken
/// because it exists rather than because it is needed — the rule the crate's
/// own `Cargo.toml` already states about zip.
fn unbase64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0_u32;
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            #[allow(clippy::cast_possible_truncation)]
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{read, read_binary, unbase64};
    use crate::Refusal;

    /// One triangle at the origin, as a buffer of positions and indices.
    fn geometry() -> Vec<u8> {
        let mut bytes = Vec::new();
        for v in [[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for c in v {
                bytes.extend_from_slice(&c.to_le_bytes());
            }
        }
        for i in [0_u16, 1, 2] {
            bytes.extend_from_slice(&i.to_le_bytes());
        }
        bytes
    }

    fn base64(bytes: &[u8]) -> String {
        const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let mut n = 0_u32;
            for (i, b) in chunk.iter().enumerate() {
                n |= u32::from(*b) << (16 - 8 * i);
            }
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(ABC[((n >> (18 - 6 * i)) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    /// A document around one node, with whatever transform is given.
    fn document(node: &str) -> String {
        format!(
            r#"{{"asset":{{"version":"2.0","generator":"a test"}},
              "scene":0,"scenes":[{{"nodes":[0]}}],
              "nodes":[{{"mesh":0{node}}}],
              "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"mode":4}}]}}],
              "accessors":[
                {{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"}},
                {{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}],
              "bufferViews":[
                {{"buffer":0,"byteOffset":0,"byteLength":36}},
                {{"buffer":0,"byteOffset":36,"byteLength":6}}],
              "buffers":[{{"byteLength":42,"uri":"data:application/octet-stream;base64,{}"}}]}}"#,
            base64(&geometry())
        )
    }

    fn corners(text: &str) -> [[f32; 3]; 3] {
        let out = read(text.as_bytes(), Path::new("/nowhere/a.gltf")).unwrap();
        let m = out.body.mesh;
        assert_eq!(m.triangles(), 1);
        m.corners(0)
    }

    #[test]
    fn a_node_transform_reaches_the_triangles_underneath_it() {
        // THE DEFECT THIS CATCHES: taking the meshes and ignoring the nodes,
        // which draws every part of a scene at the origin.
        let plain = corners(&document(""));
        let moved = corners(&document(r#","translation":[5,0,0]"#));
        assert!((plain[0][0] - 0.0).abs() < 1e-3);
        // Five metres, in millimetres.
        assert!(
            (moved[0][0] - 5000.0).abs() < 1e-1,
            "the translation did not reach the geometry: {moved:?}"
        );
    }

    #[test]
    fn a_matrix_and_the_same_transform_as_trs_agree() {
        let by_matrix = corners(&document(
            r#","matrix":[2,0,0,0, 0,2,0,0, 0,0,2,0, 1,0,0,1]"#,
        ));
        let by_parts = corners(&document(r#","translation":[1,0,0],"scale":[2,2,2]"#));
        for (a, b) in by_matrix.iter().flatten().zip(by_parts.iter().flatten()) {
            assert!((a - b).abs() < 1e-2, "{by_matrix:?} against {by_parts:?}");
        }
    }

    #[test]
    fn a_quarter_turn_about_z_sends_x_to_y() {
        // The quaternion for 90 degrees about Z, as an exporter writes it.
        let s = (0.5_f32).sqrt();
        let out = corners(&document(&format!(r#","rotation":[0,0,{s},{s}]"#)));
        // The corner at (1, 0, 0) metres becomes (0, 1000, 0) millimetres.
        assert!(
            (out[1][1] - 1000.0).abs() < 1.0 && out[1][0].abs() < 1.0,
            "the rotation did not turn the piece: {out:?}"
        );
    }

    #[test]
    fn a_required_extension_this_reader_lacks_is_refused_and_not_drawn_in_part() {
        let text = document("").replace(
            r#""asset":"#,
            r#""extensionsRequired":["KHR_draco_mesh_compression"],"asset":"#,
        );
        assert_eq!(
            read(text.as_bytes(), Path::new("/nowhere/a.gltf")),
            Err(Refusal::Unsupported(crate::key::PREVIEW_DRACO))
        );
    }

    #[test]
    fn a_buffer_uri_that_climbs_out_of_the_folder_is_refused() {
        for uri in [
            "../../../etc/passwd",
            "/etc/passwd",
            "https://example.invalid/x.bin",
            "%2E%2E/%2E%2E/etc/passwd",
        ] {
            let text = document("").replace(
                &format!(
                    "data:application/octet-stream;base64,{}",
                    base64(&geometry())
                ),
                uri,
            );
            assert!(
                read(text.as_bytes(), Path::new("/nowhere/a.gltf")).is_err(),
                "`{uri}` was not refused"
            );
        }
    }

    #[test]
    fn a_glb_reads_its_geometry_out_of_the_chunk_and_not_a_uri() {
        let json = String::from(
            r#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
              "nodes":[{"mesh":0}],
              "meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}],
              "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3"},
                {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}],
              "bufferViews":[
                {"buffer":0,"byteOffset":0,"byteLength":36},
                {"buffer":0,"byteOffset":36,"byteLength":6}],
              "buffers":[{"byteLength":42}]}"#,
        );
        let mut json = json.into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut bin = geometry();
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&0_u32.to_le_bytes());
        glb.extend_from_slice(&u32::try_from(json.len()).unwrap().to_le_bytes());
        glb.extend_from_slice(&super::CHUNK_JSON.to_le_bytes());
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&u32::try_from(bin.len()).unwrap().to_le_bytes());
        glb.extend_from_slice(&super::CHUNK_BIN.to_le_bytes());
        glb.extend_from_slice(&bin);
        let out = read_binary(&glb).unwrap();
        assert_eq!(out.shape, crate::mesh::Shape::Glb);
        assert!(!out.body.nothing());
    }

    #[test]
    fn a_sparse_accessor_is_applied_and_not_read_past() {
        // THE DEFECT THIS CATCHES: ignoring `sparse`, which gives back the
        // base positions — the wrong shape, with no error anywhere.
        let mut buffer = geometry();
        let overlay_at = buffer.len();
        buffer.extend_from_slice(&2_u16.to_le_bytes()); // index 2 ...
        while !buffer.len().is_multiple_of(4) {
            buffer.push(0);
        }
        let values_at = buffer.len();
        for c in [0.0_f32, 7.0, 0.0] {
            buffer.extend_from_slice(&c.to_le_bytes());
        }
        let text = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],
              "nodes":[{{"mesh":0}}],
              "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1}}]}}],
              "accessors":[
                {{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3",
                  "sparse":{{"count":1,
                    "indices":{{"bufferView":2,"componentType":5123}},
                    "values":{{"bufferView":3}}}}}},
                {{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}}],
              "bufferViews":[
                {{"buffer":0,"byteOffset":0,"byteLength":36}},
                {{"buffer":0,"byteOffset":36,"byteLength":6}},
                {{"buffer":0,"byteOffset":{overlay_at},"byteLength":2}},
                {{"buffer":0,"byteOffset":{values_at},"byteLength":12}}],
              "buffers":[{{"byteLength":{},"uri":"data:application/octet-stream;base64,{}"}}]}}"#,
            buffer.len(),
            base64(&buffer)
        );
        let out = corners(&text);
        assert!(
            (out[2][1] - 7000.0).abs() < 1.0,
            "the sparse overlay did not reach the third corner: {out:?}"
        );
    }

    #[test]
    fn base64_round_trips_through_the_reader_s_own_decoder() {
        for n in 0..8_usize {
            let bytes: Vec<u8> = (0..n)
                .map(|i| u8::try_from(i * 37 % 251).unwrap())
                .collect();
            assert_eq!(unbase64(&base64(&bytes)).unwrap(), bytes, "at length {n}");
        }
    }
}
