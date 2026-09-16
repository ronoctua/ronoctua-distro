//! A download turned into a pack.
//!
//! # What is kept, and what the arithmetic of keeping it is
//!
//! A Geofabrik `*-free.shp.zip` holds a hundred and one files. TWO are opened
//! (`gis_osm_roads_free_1` and `gis_osm_places_free_1`) and of their ten
//! columns, TWO are read (`name`, `fclass`). Everything else — land use,
//! buildings, points of interest, waterways, traffic, natural features — is
//! two thirds of the download and none of an address.
//!
//! A way without a name is dropped, which is most of them: 27.8% of Andorra's
//! 8 799 ways are named, measured. An unnamed way is a line the panel could
//! only say "a road" about, and the place is a better answer than that.
//!
//! # Why the segments are spread over the cells they TOUCH
//!
//! The question a pack answers is a distance to a LINE, and a line's nearest
//! point can be in a cell the line only passes through. Filing a segment under
//! the cell of its midpoint would make a long rural road invisible from the
//! ground beside it — the failure would look like "this pack has no streets
//! here", which is indistinguishable from a region genuinely without any.

use std::collections::HashMap;

use crate::{CELL_DEGREES, MAGIC, META, MOST_CELLS, MOST_PLACES, Trouble, VERSION, shapefile};

/// How much one member of the download may inflate to.
///
/// Two gibibytes. `ronoctua_zip::INFLATED_CEILING` is 256 MiB, which is right
/// for a document and far too small here: the roads layer of a large region is
/// bigger than every document this system has ever opened put together. The
/// ceiling still exists — a zip is a thing somebody can build to inflate for
/// ever — and it is raised where the caller knows what it is opening, which is
/// why `ronoctua_zip` takes it as an argument rather than deciding.
pub const LAYER_CEILING: u64 = 2 * 1024 * 1024 * 1024;

/// The two layers, by the suffix of their names inside the zip.
const ROADS: &str = "gis_osm_roads_free_1";
/// See [`ROADS`].
const PLACES: &str = "gis_osm_places_free_1";

/// Bakes the download into a pack.
///
/// `id`, `label` and `dated` come from the catalogue and from what the
/// download redirected to — the dated name IS the version, because the
/// `-latest-` URL is a redirect and what it points at is a date.
///
/// # Errors
///
/// [`Trouble::NotARegionPack`] when the two layers are not in the zip,
/// [`Trouble::Mismatched`] when a layer's `.shp` and `.dbf` disagree about how
/// many records there are, and whatever the zip or the shapefile answers.
pub fn pack(zip: &[u8], id: &str, label: &str, dated: &str) -> Result<Vec<u8>, Trouble> {
    let list = ronoctua_zip::members(zip)?;
    let find = |layer: &str, ext: &str| {
        list.iter()
            .find(|m| m.name().ends_with(&format!("{layer}.{ext}")))
    };
    let (Some(rs), Some(rd)) = (find(ROADS, "shp"), find(ROADS, "dbf")) else {
        return Err(Trouble::NotARegionPack);
    };

    let mut names = Table::new();
    let mut segments: Vec<(u32, i32, i32, i32, i32)> = Vec::new();
    let mut ways = 0_u32;
    {
        let dbf = ronoctua_zip::member_up_to(zip, rd, LAYER_CEILING)?;
        let columns = shapefile::columns(&dbf, &["name"])?;
        let named = columns.into_iter().next().unwrap_or_default();
        drop(dbf);
        let shp = ronoctua_zip::member_up_to(zip, rs, LAYER_CEILING)?;
        let mut trouble = None;
        let count = shapefile::each(&shp, |i, shape| {
            let Some(name) = named.get(i).filter(|n| !n.is_empty()) else {
                return;
            };
            let shapefile::Shape::Line(points) = shape else {
                return;
            };
            if points.len() < 2 {
                return;
            }
            if names.len() >= u32::MAX as usize {
                trouble = Some(Trouble::Malformed);
                return;
            }
            let n = names.id(name);
            ways += 1;
            for pair in points.windows(2) {
                segments.push((
                    n,
                    micro(pair[0].0),
                    micro(pair[0].1),
                    micro(pair[1].0),
                    micro(pair[1].1),
                ));
            }
        })?;
        if let Some(t) = trouble {
            return Err(t);
        }
        // THE ONE DETECTABLE FORM OF THE JOIN BEING WRONG. See `shapefile`'s
        // trap THREE: there is no key, so equal counts is all the agreement
        // there can be — and unequal counts means every name after the first
        // difference belongs to another way.
        if count != named.len() {
            return Err(Trouble::Mismatched);
        }
    }
    if segments.is_empty() {
        return Err(Trouble::NotARegionPack);
    }

    let mut places: Vec<(i32, i32, u32)> = Vec::new();
    if let (Some(ps), Some(pd)) = (find(PLACES, "shp"), find(PLACES, "dbf")) {
        let dbf = ronoctua_zip::member_up_to(zip, pd, LAYER_CEILING)?;
        let named = shapefile::columns(&dbf, &["name"])?
            .into_iter()
            .next()
            .unwrap_or_default();
        drop(dbf);
        let shp = ronoctua_zip::member_up_to(zip, ps, LAYER_CEILING)?;
        let count = shapefile::each(&shp, |i, shape| {
            let (Some(name), shapefile::Shape::Point(lat, lon)) = (named.get(i), shape) else {
                return;
            };
            if name.is_empty() || places.len() >= MOST_PLACES {
                return;
            }
            places.push((micro(lat), micro(lon), names.id(name)));
        })?;
        if count != named.len() {
            return Err(Trouble::Mismatched);
        }
    }

    Ok(write(&Baked {
        id,
        label,
        dated,
        ways,
        segments,
        places,
        names: names.taken(),
    }))
}

/// Degrees to micro-degrees, which is the pack's unit.
///
/// A millionth of a degree is about eleven centimetres, which is finer than
/// OpenStreetMap's own coordinates — so, unlike the baked world's `i16`, this
/// quantisation throws nothing away. Four bytes a number and eight a point.
#[allow(clippy::cast_possible_truncation)]
fn micro(deg: f64) -> i32 {
    (deg * 1e6)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

/// Names, deduplicated as they arrive.
struct Table {
    seen: HashMap<String, u32>,
    out: Vec<String>,
}

impl Table {
    fn new() -> Self {
        Self {
            seen: HashMap::new(),
            out: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.out.len()
    }

    fn id(&mut self, name: &str) -> u32 {
        if let Some(n) = self.seen.get(name) {
            return *n;
        }
        #[allow(clippy::cast_possible_truncation)]
        let n = self.out.len() as u32;
        self.out.push(name.to_owned());
        self.seen.insert(name.to_owned(), n);
        n
    }

    fn taken(self) -> Vec<String> {
        self.out
    }
}

/// Everything a pack holds, before it is a file.
struct Baked<'a> {
    id: &'a str,
    label: &'a str,
    dated: &'a str,
    ways: u32,
    segments: Vec<(u32, i32, i32, i32, i32)>,
    places: Vec<(i32, i32, u32)>,
    names: Vec<String>,
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn write(b: &Baked) -> Vec<u8> {
    // THE GRID, from the data's own extent.
    let (mut lat0, mut lon0, mut lat1, mut lon1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (_, a, b2, c, d) in &b.segments {
        for (lat, lon) in [(*a, *b2), (*c, *d)] {
            let (lat, lon) = (f64::from(lat) / 1e6, f64::from(lon) / 1e6);
            lat0 = lat0.min(lat);
            lon0 = lon0.min(lon);
            lat1 = lat1.max(lat);
            lon1 = lon1.max(lon);
        }
    }
    // THE GRID IS GROWN BY THE REACH ON EVERY SIDE.
    //
    // Not a hair of margin for the edge cell: the question is asked from where
    // somebody STOOD, and that can be outside the extent of the region's own
    // roads — a photograph taken on the beach is a few hundred metres west of
    // the westernmost street in the pack. Without the margin `cell_of` answers
    // "outside this pack" for a coordinate with a street two hundred metres
    // away, which reads as a pack that was not installed.
    let pad = crate::REACH_METRES / 111_320.0;
    lat0 -= pad;
    lon0 -= pad;
    lat1 += pad;
    lon1 += pad;
    let (cell, cols, rows) = {
        let mut cell = CELL_DEGREES;
        loop {
            let cols = (((lon1 - lon0) / cell).ceil() as usize).max(1);
            let rows = (((lat1 - lat0) / cell).ceil() as usize).max(1);
            if cols.saturating_mul(rows) <= MOST_CELLS {
                break (cell, cols, rows);
            }
            cell *= 2.0;
        }
    };

    // EVERY CELL A SEGMENT TOUCHES, which is what the header argues for.
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); cols * rows];
    for (i, (_, a, b2, c, d)) in b.segments.iter().enumerate() {
        let (alat, alon) = (f64::from(*a) / 1e6, f64::from(*b2) / 1e6);
        let (blat, blon) = (f64::from(*c) / 1e6, f64::from(*d) / 1e6);
        let x0 = (((alon.min(blon) - lon0) / cell) as usize).min(cols - 1);
        let x1 = (((alon.max(blon) - lon0) / cell) as usize).min(cols - 1);
        let y0 = (((alat.min(blat) - lat0) / cell) as usize).min(rows - 1);
        let y1 = (((alat.max(blat) - lat0) / cell) as usize).min(rows - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                buckets[y * cols + x].push(i as u32);
            }
        }
    }

    let mut index: Vec<u32> = Vec::with_capacity(cols * rows + 1);
    let mut body: Vec<u8> = Vec::new();
    let mut at = 0_u32;
    for bucket in &buckets {
        index.push(at);
        for i in bucket {
            let (n, a, b2, c, d) = b.segments[*i as usize];
            body.extend_from_slice(&n.to_le_bytes());
            body.extend_from_slice(&a.to_le_bytes());
            body.extend_from_slice(&b2.to_le_bytes());
            body.extend_from_slice(&c.to_le_bytes());
            body.extend_from_slice(&d.to_le_bytes());
            at += 1;
        }
    }
    index.push(at);

    let mut strings: Vec<u8> = (b.names.len() as u32).to_le_bytes().to_vec();
    for name in &b.names {
        let bytes = name.as_bytes();
        let len = bytes.len().min(usize::from(u16::MAX));
        strings.extend_from_slice(&(len as u16).to_le_bytes());
        strings.extend_from_slice(&bytes[..len]);
    }

    let mut heads: Vec<u8> = Vec::new();
    for s in [b.id, b.label, b.dated] {
        let bytes = s.as_bytes();
        let len = bytes.len().min(usize::from(u16::MAX));
        heads.extend_from_slice(&(len as u16).to_le_bytes());
        heads.extend_from_slice(&bytes[..len]);
    }

    let index_at = (MAGIC.len() + 2 + META + heads.len()) as u32;
    let segments_at = index_at + (index.len() * 4) as u32;
    let places_at = segments_at + body.len() as u32;
    let strings_at = places_at + (b.places.len() * 12) as u32;

    let mut out: Vec<u8> = Vec::with_capacity(strings_at as usize + strings.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&lat0.to_le_bytes());
    out.extend_from_slice(&lon0.to_le_bytes());
    out.extend_from_slice(&cell.to_le_bytes());
    out.extend_from_slice(&cell.to_le_bytes());
    out.extend_from_slice(&(cols as u32).to_le_bytes());
    out.extend_from_slice(&(rows as u32).to_le_bytes());
    out.extend_from_slice(&b.ways.to_le_bytes());
    out.extend_from_slice(&index_at.to_le_bytes());
    out.extend_from_slice(&segments_at.to_le_bytes());
    out.extend_from_slice(&strings_at.to_le_bytes());
    out.extend_from_slice(&places_at.to_le_bytes());
    out.extend_from_slice(&(b.places.len() as u32).to_le_bytes());
    out.extend_from_slice(&heads);
    for i in &index {
        out.extend_from_slice(&i.to_le_bytes());
    }
    out.extend_from_slice(&body);
    for (lat, lon, name) in &b.places {
        out.extend_from_slice(&lat.to_le_bytes());
        out.extend_from_slice(&lon.to_le_bytes());
        out.extend_from_slice(&name.to_le_bytes());
    }
    out.extend_from_slice(&strings);
    out
}
