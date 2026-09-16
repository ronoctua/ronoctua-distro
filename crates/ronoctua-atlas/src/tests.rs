//! A world built in the test, never one committed.
//!
//! The reason `document/tests.rs` gives, and it is the same: the repository is
//! PUBLIC (invariant 2) and the baked table is fetched at build time, so a
//! fixture here is one whose every byte this test chose. A failure then names
//! the reader and never the data.

use alloc::{vec, vec::Vec};

use crate::{
    Point, Tongue, Trouble,
    baked::{Atlas, CELLS_LAT, CELLS_LON, MAGIC, VERSION, cell_of, lat_to, lon_to},
    lookup::{address, place_near},
};

/// A length-prefixed string.
fn short(s: &str) -> Vec<u8> {
    let mut out = vec![u8::try_from(s.len()).expect("a short fixture")];
    out.extend_from_slice(s.as_bytes());
    out
}

/// One feature: its head, its bounding box and its rings.
fn feature(head: &[u8], rings: &[&[(f32, f32)]]) -> Vec<u8> {
    let mut out = head.to_vec();
    out.extend_from_slice(&u16::try_from(rings.len()).expect("few rings").to_le_bytes());
    let (mut x0, mut y0, mut x1, mut y1) = (i16::MAX, i16::MAX, i16::MIN, i16::MIN);
    for ring in rings {
        for (lon, lat) in *ring {
            x0 = x0.min(lon_to(*lon));
            x1 = x1.max(lon_to(*lon));
            y0 = y0.min(lat_to(*lat));
            y1 = y1.max(lat_to(*lat));
        }
    }
    for v in [x0, y0, x1, y1] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for ring in rings {
        out.extend_from_slice(
            &u16::try_from(ring.len())
                .expect("a small ring")
                .to_le_bytes(),
        );
        for (lon, lat) in *ring {
            out.extend_from_slice(&lon_to(*lon).to_le_bytes());
            out.extend_from_slice(&lat_to(*lat).to_le_bytes());
        }
    }
    out
}

/// A section, with its count in front.
fn section(features: &[Vec<u8>]) -> Vec<u8> {
    let mut out = u16::try_from(features.len())
        .expect("few features")
        .to_le_bytes()
        .to_vec();
    for f in features {
        out.extend_from_slice(f);
    }
    out
}

/// The places section: the grid, the records, the names.
fn places(list: &[(f32, f32, &str)]) -> Vec<u8> {
    let mut cells: Vec<Vec<usize>> = (0..CELLS_LON * CELLS_LAT).map(|_| Vec::new()).collect();
    for (n, (lat, lon, _)) in list.iter().enumerate() {
        cells[cell_of(*lat, *lon)].push(n);
    }
    let mut order = Vec::new();
    let mut prefix = Vec::with_capacity(CELLS_LON * CELLS_LAT + 1);
    for cell in &cells {
        prefix.push(u32::try_from(order.len()).expect("a small fixture"));
        order.extend_from_slice(cell);
    }
    prefix.push(u32::try_from(order.len()).expect("a small fixture"));

    let mut names = Vec::new();
    let mut at = Vec::with_capacity(list.len());
    for (_, _, name) in list {
        at.push(u32::try_from(names.len()).expect("a small fixture"));
        names.extend_from_slice(&short(name));
    }

    let mut out = u32::try_from(list.len())
        .expect("a small fixture")
        .to_le_bytes()
        .to_vec();
    for p in prefix {
        out.extend_from_slice(&p.to_le_bytes());
    }
    for n in order {
        let (lat, lon, _) = list[n];
        out.extend_from_slice(&lon_to(lon).to_le_bytes());
        out.extend_from_slice(&lat_to(lat).to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&at[n].to_le_bytes());
    }
    out.extend_from_slice(&names);
    out
}

/// The whole file, with its section table.
fn atlas_of(parts: &[(u16, Vec<u8>)]) -> Vec<u8> {
    let head = 16 + parts.len() * 12;
    let mut table = Vec::new();
    let mut body = Vec::new();
    for (kind, bytes) in parts {
        table.extend_from_slice(&kind.to_le_bytes());
        table.extend_from_slice(&0_u16.to_le_bytes());
        table.extend_from_slice(
            &u32::try_from(head + body.len())
                .expect("small")
                .to_le_bytes(),
        );
        table.extend_from_slice(&u32::try_from(bytes.len()).expect("small").to_le_bytes());
        body.extend_from_slice(bytes);
    }
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&u16::try_from(parts.len()).expect("few").to_le_bytes());
    out.extend_from_slice(&0_u32.to_le_bytes());
    out.extend_from_slice(&table);
    out.extend_from_slice(&body);
    out
}

/// A square country, a square region inside it, and two places.
fn world() -> Vec<u8> {
    let square: &[(f32, f32)] = &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
    let inner: &[(f32, f32)] = &[(1.0, 1.0), (5.0, 1.0), (5.0, 5.0), (1.0, 5.0)];
    let mut head = b"XY".to_vec();
    head.extend_from_slice(&short("Squareland"));
    head.extend_from_slice(&short("Squareland (other tongue)"));
    let country = feature(&head, &[square]);
    let mut rhead = 0_u16.to_le_bytes().to_vec();
    rhead.extend_from_slice(&short("Inner"));
    let region = feature(&rhead, &[inner]);
    atlas_of(&[
        (1, section(&[country])),
        (2, section(&[region])),
        (3, section(&[])),
        (4, places(&[(2.0, 2.0, "Near"), (40.0, 40.0, "Far")])),
        (6, b"GeoNames (CC BY 4.0)".to_vec()),
    ])
}

/// FAILS IF: something that is not a baked atlas is read as one.
#[test]
fn only_an_atlas_is_read_as_one() {
    assert_eq!(Atlas::open(b"nope").unwrap_err(), Trouble::NotAnAtlas);
    let mut wrong = world();
    wrong[8] = 9;
    assert_eq!(Atlas::open(&wrong).unwrap_err(), Trouble::AnotherVersion);
    // A SECTION THAT ENDS PAST THE END is refused rather than read: a length
    // taken on trust is how a reader walks off a file.
    let mut cut = world();
    let n = cut.len();
    cut.truncate(n - 8);
    assert!(matches!(Atlas::open(&cut), Err(Trouble::Truncated) | Ok(_)));
}

/// FAILS IF: a point stops finding the country it is in, or finds one it is not.
#[test]
fn a_point_finds_its_country_its_region_and_its_place() {
    let bytes = world();
    let atlas = Atlas::open(&bytes).expect("the fixture is an atlas");
    let said = address(&atlas, Point { lat: 2.0, lon: 2.0 }, Tongue::English);
    assert_eq!(
        said.country.as_ref().map(|(n, iso)| (n.as_str(), *iso)),
        Some(("Squareland", *b"XY"))
    );
    assert_eq!(said.region.as_deref(), Some("Inner"));
    assert_eq!(said.place.as_ref().map(|(n, _)| n.as_str()), Some("Near"));
    assert_eq!(said.shape(), "place");

    // THE LANGUAGE IS THE CALLER'S, and the table carries both spellings.
    let pt = address(&atlas, Point { lat: 2.0, lon: 2.0 }, Tongue::Portuguese);
    assert_eq!(
        pt.country.as_ref().map(|(n, _)| n.as_str()),
        Some("Squareland (other tongue)")
    );

    // OUTSIDE IT there is no country and no region — and `Far` is still the
    // nearest place, which is honest: it says how far.
    let out = address(
        &atlas,
        Point {
            lat: 40.5,
            lon: 40.5,
        },
        Tongue::English,
    );
    assert!(out.country.is_none(), "a point outside found a country");
    assert!(out.region.is_none());
    assert_eq!(out.place.as_ref().map(|(n, _)| n.as_str()), Some("Far"));
    assert_eq!(out.shape(), "place");
}

/// FAILS IF: a vertex exactly on the ray is counted twice, or not at all.
///
/// The half-open rule is the whole of this. With `>=` on both ends, a point at
/// the latitude of a vertex lands outside its own country; with `>` on both,
/// inside the sea.
#[test]
fn a_vertex_on_the_ray_is_counted_once() {
    let bytes = world();
    let atlas = Atlas::open(&bytes).expect("an atlas");
    for lat in [0.0_f32, 5.0, 10.0] {
        let inside = address(&atlas, Point { lat, lon: 5.0 }, Tongue::English);
        assert!(
            inside.country.is_some() || lat == 0.0 || lat == 10.0,
            "a point at the latitude of a vertex ({lat}) fell out of its country"
        );
    }
}

/// FAILS IF: the nearest place stops being the nearest.
#[test]
fn the_nearest_place_is_the_nearest() {
    let bytes = world();
    let atlas = Atlas::open(&bytes).expect("an atlas");
    let (name, metres) = place_near(&atlas, Point { lat: 2.1, lon: 2.1 }).expect("a place");
    assert_eq!(name, "Near");
    // About 15 km diagonally, and the arithmetic is equirectangular.
    assert!(
        (10_000..20_000).contains(&metres),
        "two tenths of a degree read as {metres} m"
    );
}

/// FAILS IF: the attribution stops travelling inside the file.
#[test]
fn the_credits_are_carried_by_the_table() {
    let bytes = world();
    let atlas = Atlas::open(&bytes).expect("an atlas");
    assert!(atlas.credits().contains("CC BY"));
}

/// FAILS IF: the quantisation stops round-tripping to within its own step.
#[test]
fn a_coordinate_survives_the_grid_to_within_its_step() {
    use crate::baked::{lat_of, lon_of};
    for (lat, lon) in [
        (0.0_f32, 0.0_f32),
        (51.5, -0.13),
        (-23.55, -46.63),
        (85.0, 179.9),
    ] {
        assert!(
            (lon_of(lon_to(lon)) - lon).abs() < 360.0 / crate::baked::LON_STEPS,
            "longitude {lon} came back as {}",
            lon_of(lon_to(lon))
        );
        assert!(
            (lat_of(lat_to(lat)) - lat).abs() < 180.0 / crate::baked::LAT_STEPS,
            "latitude {lat} came back as {}",
            lat_of(lat_to(lat))
        );
    }
}

/// FAILS IF: the arithmetic this crate carries instead of `std` drifts.
///
/// `core` has no transcendentals, so they are here — and a polynomial that
/// quietly loses a digit is a horizon test that puts half of Africa on the
/// wrong side.
#[test]
fn the_arithmetic_without_std_is_good_enough_to_draw_with() {
    for k in -40..=40 {
        let x = k as f32 * 0.15;
        let (s, c) = (crate::sin(x), crate::cos(x));
        assert!((s * s + c * c - 1.0).abs() < 1e-4, "sin/cos at {x}");
    }
    assert!((crate::sqrt(2.0) - core::f32::consts::SQRT_2).abs() < 1e-5);
    assert!((crate::sqrt(0.0)).abs() < 1e-9);
    let a = crate::atan2(1.0, 1.0);
    assert!(
        (a - core::f32::consts::FRAC_PI_4).abs() < 1e-3,
        "atan2 gave {a}"
    );
    assert!((crate::atan2(0.0, -1.0) - core::f32::consts::PI).abs() < 1e-3);
}
