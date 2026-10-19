//! A pack built in the test rather than downloaded.
//!
//! The reason is the one `ppt/tests.rs` gives: the repository is PUBLIC
//! (invariant 2) and a Geofabrik download is two hundred megabytes that nobody
//! should have to fetch to run a test. A shapefile written here is one whose
//! every byte this test chose, so a failure names the reader and never the
//! file.

use std::io::Write as _;

use crate::{Pack, bake, shapefile};

/// A `.shp` of the given shapes, with the header this reader checks.
fn shp(records: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0_u8; 100];
    out[0..4].copy_from_slice(&9994_u32.to_be_bytes());
    for (i, body) in records.iter().enumerate() {
        out.extend_from_slice(&((i + 1) as u32).to_be_bytes());
        // IN SIXTEEN-BIT WORDS, which is trap TWO written from the other side.
        out.extend_from_slice(&((body.len() / 2) as u32).to_be_bytes());
        out.extend_from_slice(body);
    }
    out
}

/// One polyline record's content: type, box, parts, points.
fn line(points: &[(f64, f64)]) -> Vec<u8> {
    let mut b = 3_u32.to_le_bytes().to_vec();
    b.extend_from_slice(&[0_u8; 32]);
    b.extend_from_slice(&1_u32.to_le_bytes());
    b.extend_from_slice(&(points.len() as u32).to_le_bytes());
    b.extend_from_slice(&0_u32.to_le_bytes());
    for (lat, lon) in points {
        b.extend_from_slice(&lon.to_le_bytes());
        b.extend_from_slice(&lat.to_le_bytes());
    }
    b
}

/// One point record's content.
fn point(lat: f64, lon: f64) -> Vec<u8> {
    let mut b = 1_u32.to_le_bytes().to_vec();
    b.extend_from_slice(&lon.to_le_bytes());
    b.extend_from_slice(&lat.to_le_bytes());
    b
}

/// A `.dbf` with one `name` column of the given values.
fn dbf(names: &[&str]) -> Vec<u8> {
    const WIDTH: usize = 40;
    let header = 32 + 32 + 1;
    let mut out = vec![0_u8; 32];
    out[0] = 0x03;
    out[4..8].copy_from_slice(&(names.len() as u32).to_le_bytes());
    out[8..10].copy_from_slice(&(header as u16).to_le_bytes());
    out[10..12].copy_from_slice(&((WIDTH + 1) as u16).to_le_bytes());
    let mut field = vec![0_u8; 32];
    field[..4].copy_from_slice(b"name");
    field[11] = b'C';
    field[16] = WIDTH as u8;
    out.extend_from_slice(&field);
    out.push(0x0D);
    for n in names {
        // THE DELETION FLAG FIRST, then the field, SPACE-padded: trap FOUR.
        out.push(b' ');
        let mut cell = vec![b' '; WIDTH];
        let bytes = n.as_bytes();
        cell[..bytes.len()].copy_from_slice(bytes);
        out.extend_from_slice(&cell);
    }
    out
}

/// A zip of the members, stored rather than deflated.
fn zip_of(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for (name, body) in files {
        let at = out.len() as u32;
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        out.extend_from_slice(&[0_u8; 14]);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);

        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        central.extend_from_slice(&[0_u8; 16]);
        central.extend_from_slice(&(body.len() as u32).to_le_bytes());
        central.extend_from_slice(&(body.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0_u8; 12]);
        central.extend_from_slice(&at.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let dir_at = out.len() as u32;
    let count = files.len() as u16;
    out.extend_from_slice(&central);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
    out.extend_from_slice(&[0_u8; 6]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&dir_at.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out
}

/// A download of two ways and one place, around a coordinate in Lisbon.
fn download() -> Vec<u8> {
    let roads = shp(&[
        line(&[(38.7100, -9.1400), (38.7100, -9.1300)]),
        line(&[(38.7200, -9.1400), (38.7200, -9.1300)]),
        // AND ONE WITHOUT A NAME, which is most of a real layer.
        line(&[(38.7150, -9.1400), (38.7150, -9.1300)]),
    ]);
    let places = shp(&[point(38.7150, -9.1350)]);
    zip_of(&[
        ("gis_osm_roads_free_1.shp", roads),
        (
            "gis_osm_roads_free_1.dbf",
            dbf(&["Rua Augusta", "Rua da Prata", ""]),
        ),
        ("gis_osm_places_free_1.shp", places),
        ("gis_osm_places_free_1.dbf", dbf(&["Baixa"])),
    ])
}

/// A baked pack on disk, and the directory it is in.
fn baked(name: &str) -> (std::path::PathBuf, Pack) {
    let dir = std::env::temp_dir().join(format!("ronoctua-packs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let bytes = bake::pack(
        &download(),
        "portugal",
        "Portugal",
        "portugal-260914-free.shp.zip",
        &mut || true,
    )
    .expect("the download bakes");
    let path = dir.join(crate::FILE);
    let mut f = std::fs::File::create(&path).expect("write the pack");
    f.write_all(&bytes).expect("write the pack");
    drop(f);
    let pack = Pack::open(&path).expect("the pack opens");
    (dir, pack)
}

/// FAILS IF: the nearest named way is not the nearest named way.
///
/// The coordinate is a hundred metres south of `Rua Augusta` and roughly a
/// kilometre from `Rua da Prata`, so an answer of either "the other one" or
/// "nothing" is a reader that is not measuring distance to a LINE.
#[test]
fn the_nearest_named_way_is_the_one_beside_the_coordinate() {
    let (dir, mut pack) = baked("near");
    let near = pack
        .street_near(38.7091, -9.1350)
        .expect("the segments read")
        .expect("something is near");
    assert_eq!(near.0, "Rua Augusta");
    assert!(near.1 < 200, "it answered {} metres", near.1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: an unnamed way reaches a pack, or a name is counted twice.
#[test]
fn a_way_with_no_name_is_not_a_street() {
    let (dir, mut pack) = baked("unnamed");
    // The unnamed way passes exactly through this coordinate; the named ones
    // are forty metres away on either side.
    let near = pack
        .street_near(38.7150, -9.1350)
        .expect("the segments read")
        .expect("something is near");
    assert_ne!(near.0, "", "an unnamed way was given a name");
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: a coordinate outside the region's own extent is given a street.
#[test]
fn a_coordinate_outside_the_pack_has_no_street() {
    let (dir, mut pack) = baked("outside");
    assert_eq!(
        pack.street_near(-23.5, -46.6).expect("the segments read"),
        None
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: the place layer stops being read, which is the district a baked
/// world of towns cannot give.
#[test]
fn the_pack_knows_the_district() {
    let (dir, pack) = baked("district");
    assert_eq!(
        pack.place_near(38.7150, -9.1350).map(|p| p.0),
        Some("Baixa".to_owned())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: the `.shp` and the `.dbf` are allowed to disagree about how many
/// records they have.
///
/// This is the one detectable form of the join being wrong — there is no key
/// in either file — and letting it through names every way after the
/// difference with its neighbour's name, which READS AS TRUE.
#[test]
fn a_dbf_that_does_not_match_the_shp_is_refused() {
    let roads = shp(&[line(&[(38.71, -9.14), (38.71, -9.13)])]);
    let zip = zip_of(&[
        ("gis_osm_roads_free_1.shp", roads),
        ("gis_osm_roads_free_1.dbf", dbf(&["One", "Two"])),
    ]);
    assert_eq!(
        bake::pack(&zip, "x", "X", "x.zip", &mut || true),
        Err(crate::Trouble::Mismatched)
    );
}

/// FAILS IF: the baking runs on past a no, at any of its boundaries, or asks
/// too seldom for a pause to hold it — before each layer is inflated, before
/// each record of a walk and before the pack is written. A NO AT EVERY ASK IN
/// TURN, and each one ends the baking there.
#[test]
fn the_baking_stops_at_every_boundary_it_is_told_to() {
    let zip = download();
    let mut asked = 0;
    bake::pack(&zip, "portugal", "Portugal", "p.zip", &mut || {
        asked += 1;
        true
    })
    .expect("the download bakes");
    // FOUR LAYERS, FOUR RECORDS (three ways and a place), ONE WRITE.
    assert_eq!(asked, 4 + 4 + 1, "the baking's boundaries");
    for no_at in 1..=asked {
        let mut n = 0;
        let given = bake::pack(&zip, "portugal", "Portugal", "p.zip", &mut || {
            n += 1;
            n < no_at
        });
        assert_eq!(
            given,
            Err(crate::Trouble::GivenUp),
            "a no at ask {no_at} of {asked}"
        );
        assert_eq!(n, no_at, "the baking asked again after a no at ask {no_at}");
    }
}

/// FAILS IF: something that is not a region download is baked anyway.
#[test]
fn a_zip_without_the_roads_layer_is_not_a_region() {
    let zip = zip_of(&[("readme.txt", b"nothing here".to_vec())]);
    assert_eq!(
        bake::pack(&zip, "x", "X", "x.zip", &mut || true),
        Err(crate::Trouble::NotARegionPack)
    );
}

/// FAILS IF: the header is read in one byte order throughout.
///
/// The `.shp` header is BIG-endian and its records are LITTLE-endian, which is
/// trap ONE. A reader that carries one habit into the other gets a record
/// length in the billions.
#[test]
fn a_shapefile_with_the_wrong_file_code_is_refused() {
    let mut bytes = shp(&[point(1.0, 2.0)]);
    bytes[0..4].copy_from_slice(&9994_u32.to_le_bytes());
    assert_eq!(shapefile::shapes(&bytes), Err(crate::Trouble::Malformed));
}

/// FAILS IF: the null shape stops being legal.
#[test]
fn a_record_with_no_geometry_is_not_an_error() {
    let bytes = shp(&[0_u32.to_le_bytes().to_vec(), point(1.0, 2.0)]);
    assert_eq!(
        shapefile::shapes(&bytes),
        Ok(vec![
            shapefile::Shape::Nothing,
            shapefile::Shape::Point(1.0, 2.0)
        ])
    );
}
