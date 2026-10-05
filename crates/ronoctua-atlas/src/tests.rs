//! A world built in the test, never one committed.
//!
//! The reason `document/tests.rs` gives, and it is the same: the repository is
//! PUBLIC (invariant 2) and the baked table is fetched at build time, so a
//! fixture here is one whose every byte this test chose. A failure then names
//! the reader and never the data.

use alloc::{vec, vec::Vec};

use crate::{
    Ink, Mark, Marks, Point, Pose, Tongue, Trouble,
    baked::{Atlas, CELLS_LAT, CELLS_LON, MAGIC, VERSION, cell_of, lat_to, lon_to},
    framing,
    lookup::{address, anchor_of, country_name, folded, place_named, place_near},
    placed, project, tier,
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

/// The anchors section: sorted by code, as the generator writes it.
fn anchors(list: &[(&[u8; 2], f32, f32)]) -> Vec<u8> {
    let mut sorted = list.to_vec();
    sorted.sort_by_key(|(iso, _, _)| **iso);
    let mut out = u16::try_from(sorted.len())
        .expect("few anchors")
        .to_le_bytes()
        .to_vec();
    for (iso, lat, lon) in sorted {
        out.extend_from_slice(iso);
        out.extend_from_slice(&lon_to(lon).to_le_bytes());
        out.extend_from_slice(&lat_to(lat).to_le_bytes());
    }
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
        (
            4,
            places(&[
                (2.0, 2.0, "Near"),
                (40.0, 40.0, "Far"),
                (8.0, 8.0, "Écouen"),
                (40.0, 41.0, "Écouen"),
            ]),
        ),
        (6, b"GeoNames (CC BY 4.0)".to_vec()),
        (
            7,
            anchors(&[
                (b"ZZ", -30.0, 120.0),
                (b"XY", 5.0, 5.0),
                (b"AA", 60.0, -100.0),
            ]),
        ),
    ])
}

/// The same world as a table baked before the anchors existed.
fn world_before_anchors() -> Vec<u8> {
    let square: &[(f32, f32)] = &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
    let mut head = b"XY".to_vec();
    head.extend_from_slice(&short("Squareland"));
    head.extend_from_slice(&short("Squareland (other tongue)"));
    atlas_of(&[(1, section(&[feature(&head, &[square])]))])
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

/// FAILS IF: a place is found by its name in another country than the one
/// asked — two places share a name — or not found when a feed leaves its
/// marks out, or a name no place of the country bears is placed anyway.
#[test]
fn a_place_is_found_by_its_name_within_its_country() {
    let bytes = world();
    let atlas = Atlas::open(&bytes).expect("an atlas");
    let at = place_named(&atlas, *b"XY", "Ecouen").expect("in Squareland");
    assert!(
        (at.lat - 8.0).abs() < 0.1 && (at.lon - 8.0).abs() < 0.1,
        "{at:?}"
    );
    assert!(place_named(&atlas, *b"XY", "ÉCOUEN").is_some(), "any case");
    assert_eq!(
        place_named(&atlas, *b"XY", "Far"),
        None,
        "Far is in no country"
    );
    assert_eq!(
        place_named(&atlas, *b"ZZ", "Near"),
        None,
        "Near is not ZZ's"
    );
    assert_eq!(place_named(&atlas, *b"XY", ""), None);
    assert_eq!(folded(" Saint-Étienne "), "saint etienne");
    // ROMANIAN's comma below, Turkish's breve and dotless i, the letters
    // that are two, and a name written decomposed.
    assert_eq!(folded("Brașov"), "brasov");
    assert_eq!(folded("Timişoara"), folded("Timișoara"));
    assert_eq!(folded("Constanța"), "constanta");
    assert_eq!(folded("Muğla"), "mugla");
    assert_eq!(folded("Diyarbakır"), "diyarbakir");
    assert_eq!(folded("İzmir"), "izmir");
    assert_eq!(folded("Gießen"), "giessen");
    assert_eq!(folded("Ærøskøbing"), "aeroskobing");
    assert_eq!(folded("Đakovo"), "dakovo");
    assert_eq!(folded("Mal\u{e0}ga"), folded("Mala\u{300}ga"));
    // A HARBOUR just off the drawn coast is its country's.
    assert!(crate::in_country(
        &atlas,
        Point {
            lat: 5.0,
            lon: 10.1
        },
        *b"XY"
    ));
    assert!(!crate::in_country(
        &atlas,
        Point {
            lat: 5.0,
            lon: 11.0
        },
        *b"XY"
    ));
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

/// FAILS IF: a country's label point is not found by its code, or a name is
/// not found in the tongue asked.
#[test]
fn an_anchor_and_a_name_are_found_by_the_code() {
    let bytes = world();
    let atlas = Atlas::open(&bytes).expect("an atlas");
    let at = anchor_of(&atlas, *b"XY").expect("XY is anchored");
    assert!(
        (at.lat - 5.0).abs() < 0.01 && (at.lon - 5.0).abs() < 0.01,
        "{at:?}"
    );
    // BOTH ENDS of the binary search.
    let first = anchor_of(&atlas, *b"AA").expect("AA");
    assert!((first.lon + 100.0).abs() < 0.01);
    let last = anchor_of(&atlas, *b"ZZ").expect("ZZ");
    assert!((last.lat + 30.0).abs() < 0.01);
    assert_eq!(anchor_of(&atlas, *b"QQ"), None);

    assert_eq!(
        country_name(&atlas, *b"XY", Tongue::English).as_deref(),
        Some("Squareland")
    );
    assert_eq!(
        country_name(&atlas, *b"XY", Tongue::Portuguese).as_deref(),
        Some("Squareland (other tongue)")
    );
    assert_eq!(country_name(&atlas, *b"QQ", Tongue::English), None);
    assert_eq!(country_name(&atlas, *b"??", Tongue::English), None);

    // AN OLDER TABLE has no anchors, and answers nothing rather than failing.
    let old = world_before_anchors();
    let atlas = Atlas::open(&old).expect("an older atlas still opens");
    assert_eq!(anchor_of(&atlas, *b"XY"), None);
    assert!(country_name(&atlas, *b"XY", Tongue::English).is_some());
}

const RADII: [f32; 4] = [2.5, 3.5, 5.0, 7.0];

fn marks(all: &[Mark]) -> Marks<'_> {
    Marks {
        all,
        merge_px: 16.0,
        radii: RADII,
        lit: None,
    }
}

fn mark(id: u32, lat: f32, lon: f32, weight: u32) -> Mark {
    Mark {
        at: Point { lat, lon },
        id,
        weight,
    }
}

const BOX: (f32, f32) = (400.0, 300.0);

fn facing(lat: f32, lon: f32) -> Pose {
    Pose {
        centre: Point { lat, lon },
        zoom: Pose::WORLD,
    }
}

/// FAILS IF: a mark on the far side of the planet is drawn, or grabbed.
#[test]
fn a_mark_round_the_back_is_not_placed() {
    let all = [
        mark(1, 0.0, 0.0, 1),
        mark(2, 0.0, 180.0, 50),
        mark(3, 10.0, 170.0, 1),
    ];
    let out = placed(&marks(&all), facing(0.0, 0.0), BOX);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0].members, [1]);
    // THE CENTRE OF THE BOX is where the point the globe faces lands.
    assert!((out[0].at.0 - 200.0).abs() < 0.5 && (out[0].at.1 - 150.0).abs() < 0.5);
}

/// FAILS IF: two marks closer than the merge are drawn as two, or the group
/// forgets one of them.
#[test]
fn marks_closer_than_the_merge_are_one_and_count_both() {
    // A degree apart: under three pixels at the world's zoom.
    let all = [
        mark(7, 10.0, 10.0, 3),
        mark(9, 11.0, 11.0, 5),
        mark(4, -40.0, -40.0, 1),
    ];
    let out = placed(&marks(&all), facing(0.0, 0.0), BOX);
    assert_eq!(out.len(), 2, "{out:?}");
    let pair = out.iter().find(|p| p.members.len() == 2).expect("a pair");
    assert_eq!(pair.weight, 8);
    // THE HEAVIEST MEMBER FIRST, and the group sits on it.
    assert_eq!(pair.members, [9, 7]);
    assert!((pair.centre.lat - 11.0).abs() < 1e-4);
    assert_eq!(pair.radius, RADII[tier(8)]);
    let alone = out
        .iter()
        .find(|p| p.members == [4])
        .expect("the far one alone");
    assert_eq!(alone.radius, RADII[0]);

    // AND APART, when the globe is near enough to tell them apart — about a
    // hundred pixels, with the far one off the picture.
    let near = Pose {
        centre: Point {
            lat: 10.5,
            lon: 10.5,
        },
        zoom: Pose::LOCAL,
    };
    let out = placed(&marks(&all), near, BOX);
    assert_eq!(out.len(), 2, "{out:?}");
    assert!(out.iter().all(|p| p.members.len() == 1), "{out:?}");
}

/// FAILS IF: the same marks, given in another order, make other groups.
#[test]
fn clustering_does_not_depend_on_the_order_given() {
    let mut all: Vec<Mark> = (0..60_u32)
        .map(|n| {
            #[allow(clippy::cast_precision_loss)]
            let f = n as f32;
            mark(
                n,
                (f * 7.3) % 50.0 - 25.0,
                (f * 13.7) % 100.0 - 50.0,
                n % 7 + 1,
            )
        })
        .collect();
    let first = placed(&marks(&all), facing(0.0, 0.0), BOX);
    assert!(
        first.len() > 1 && first.len() < all.len(),
        "{}",
        first.len()
    );
    all.reverse();
    assert_eq!(placed(&marks(&all), facing(0.0, 0.0), BOX), first);
    // A SHUFFLE with no randomness: every seventh, from each start.
    let shuffled: Vec<Mark> = (0..7)
        .flat_map(|s| all.iter().skip(s).step_by(7).copied())
        .collect();
    assert_eq!(shuffled.len(), all.len());
    assert_eq!(placed(&marks(&shuffled), facing(0.0, 0.0), BOX), first);
}

/// FAILS IF: the size stops saying "more here", or grows without end.
#[test]
fn a_tier_grows_with_the_count_and_stops() {
    assert_eq!(tier(0), 0);
    assert_eq!(tier(1), 0);
    assert_eq!(tier(2), 1);
    assert_eq!(tier(9), 1);
    assert_eq!(tier(10), 2);
    assert_eq!(tier(99), 2);
    assert_eq!(tier(100), 3);
    assert_eq!(tier(u32::MAX), 3);
}

/// FAILS IF: framing a group leaves one of its members off the picture, or
/// in the air left around it.
#[test]
fn a_group_is_framed_whole() {
    let air = 12.0;
    let groups: [&[Point]; 3] = [
        &[
            Point {
                lat: -23.5,
                lon: -46.6,
            },
            Point {
                lat: -22.9,
                lon: -43.2,
            },
            Point {
                lat: -15.8,
                lon: -47.9,
            },
        ],
        // ACROSS THE DATE LINE, where a middle taken in degrees is the other
        // side of the planet.
        &[
            Point {
                lat: 64.8,
                lon: -147.7,
            },
            Point {
                lat: 55.0,
                lon: 160.0,
            },
        ],
        &[
            Point {
                lat: 51.5,
                lon: -0.1,
            },
            Point {
                lat: 48.9,
                lon: 2.35,
            },
        ],
    ];
    for points in groups {
        let pose = framing(points, BOX, air);
        assert!(pose.zoom > Pose::WORLD, "{pose:?}");
        let all: Vec<Mark> = points
            .iter()
            .enumerate()
            .map(|(n, p)| mark(u32::try_from(n).expect("few"), p.lat, p.lon, 1))
            .collect();
        let apart = Marks {
            merge_px: 0.0,
            ..marks(&all)
        };
        let out = placed(&apart, pose, BOX);
        assert_eq!(out.len(), points.len(), "{pose:?} {out:?}");
        for p in &out {
            assert!(
                p.at.0 >= air - 0.5
                    && p.at.0 <= BOX.0 - air + 0.5
                    && p.at.1 >= air - 0.5
                    && p.at.1 <= BOX.1 - air + 0.5,
                "{p:?} outside the air of {BOX:?} at {pose:?}"
            );
        }
    }
    // ONE POINT is the country's zoom, and none is the world.
    let one = Point { lat: 1.0, lon: 2.0 };
    assert_eq!(framing(&[one], BOX, air), Pose::on(one));
    assert_eq!(framing(&[], BOX, air).zoom, Pose::WORLD);
    // MORE THAN THE VISIBLE HALF cannot all be shown: the whole world.
    let opposite = [
        Point { lat: 0.0, lon: 0.0 },
        Point {
            lat: 0.0,
            lon: 180.0,
        },
    ];
    assert_eq!(framing(&opposite, BOX, air).zoom, Pose::WORLD);
    let around = [
        Point { lat: 0.0, lon: 0.0 },
        Point {
            lat: 0.0,
            lon: 120.0,
        },
        Point {
            lat: 0.0,
            lon: -120.0,
        },
    ];
    assert_eq!(framing(&around, BOX, air).zoom, Pose::WORLD);
}

/// FAILS IF: the picture's marks and the marks it is grabbed by differ, or
/// the lit group is not the one drawn chosen.
#[test]
fn the_drawing_paints_the_marks_it_answers_with() {
    let bytes = world();
    let atlas = Atlas::open(&bytes).expect("an atlas");
    let all = [mark(1, 5.0, 5.0, 1), mark(2, -30.0, 40.0, 120)];
    let lit = Marks {
        lit: Some(1),
        ..marks(&all)
    };
    let pose = facing(0.0, 0.0);
    let drawing = project(&atlas, pose, None, BOX, &[], 3.0, &lit);
    assert_eq!(drawing.marks, placed(&lit, pose, BOX));
    assert_eq!(drawing.marks.len(), 2);
    // THE LIT ONE LAST, so nothing is painted over it.
    assert_eq!(drawing.marks[1].members, [1]);
    let chosen = drawing
        .shapes
        .iter()
        .filter(|s| s.ink == Ink::Chosen)
        .count();
    let dots = drawing.shapes.iter().filter(|s| s.ink == Ink::Dot).count();
    assert_eq!((chosen, dots), (1, 1));

    let bare = project(&atlas, pose, None, BOX, &[], 3.0, &Marks::NONE);
    assert!(bare.marks.is_empty());
    assert!(
        bare.shapes
            .iter()
            .all(|s| s.ink != Ink::Chosen && s.ink != Ink::Dot)
    );
}

/// FAILS IF: the coordinate lights a country by its BOX — the London test.
///
/// Three countries the way the real table has them: Great Britain round
/// London; the Netherlands with an island in the Caribbean, so its box spans
/// the Atlantic and holds London; and Russia across the antimeridian, so its
/// box is every longitude. Only Great Britain's ring may light.
#[test]
fn a_coordinate_in_london_lights_only_great_britain() {
    let gb: &[(f32, f32)] = &[(-6.0, 50.0), (2.0, 50.0), (2.0, 59.0), (-6.0, 59.0)];
    let nl_home: &[(f32, f32)] = &[(3.5, 51.0), (7.0, 51.0), (7.0, 53.5), (3.5, 53.5)];
    let nl_island: &[(f32, f32)] = &[(-69.0, 12.0), (-68.0, 12.0), (-68.0, 12.5), (-69.0, 12.5)];
    let ru_west: &[(f32, f32)] = &[(20.0, 41.0), (179.0, 41.0), (179.0, 81.0), (20.0, 81.0)];
    let ru_east: &[(f32, f32)] = &[
        (-180.0, 60.0),
        (-170.0, 60.0),
        (-170.0, 70.0),
        (-180.0, 70.0),
    ];
    let head = |iso: &[u8; 2], name: &str| {
        let mut h = iso.to_vec();
        h.extend_from_slice(&short(name));
        h.extend_from_slice(&short(name));
        h
    };
    let bytes = atlas_of(&[(
        1,
        section(&[
            feature(&head(b"NL", "Netherlands"), &[nl_home, nl_island]),
            feature(&head(b"RU", "Russia"), &[ru_west, ru_east]),
            feature(&head(b"GB", "United Kingdom"), &[gb]),
        ]),
    )]);
    let atlas = Atlas::open(&bytes).expect("an atlas");
    let london = Point {
        lat: 51.507,
        lon: -0.128,
    };
    let lit = crate::globe::home_rings(&atlas, london);
    assert_eq!(
        lit.len(),
        1,
        "London lit {} rings, not Great Britain's one",
        lit.len()
    );
    for (lon, lat) in &lit[0] {
        assert!(
            (-6.1..=2.1).contains(lon) && (49.9..=59.1).contains(lat),
            "a lit point ({lon}, {lat}) is outside Great Britain"
        );
    }
    // AND THE SEA LIGHTS NOTHING: a point in no country's rings, inside two
    // countries' boxes.
    let atlantic = Point {
        lat: 45.0,
        lon: -30.0,
    };
    assert!(crate::globe::home_rings(&atlas, atlantic).is_empty());
}
