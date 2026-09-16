//! Point to place: which country, which region, which named place.
//!
//! # The two things a point-in-polygon test gets wrong
//!
//! ONE. **A vertex exactly on the ray.** The test counts crossings of a ray
//! going east along the parallel, and a vertex whose latitude is exactly the
//! point's is on two edges at once — counted twice, it puts the point outside
//! its own country; counted zero times, it puts it in the sea. The half-open
//! rule `(y0 > y) != (y1 > y)` counts it exactly once, and it is the whole of
//! why the comparison is written that way rather than with `>=`.
//!
//! TWO. **Rings that are holes.** Nothing in the data says which ring is an
//! island and which is a lake inside it, so the count runs over ALL of a
//! feature's rings and the answer is its parity. That is the same rule the
//! drawing uses (`FillRule::EvenOdd`), and it is what makes an enclave right
//! without anybody deciding which ring is which.

use alloc::{string::String, vec::Vec};

use crate::{
    Address, Named, Point, Tongue,
    baked::{Atlas, CELLS_LAT, CELLS_LON, Cursor, cell_of, lat_of, lat_to, lon_of, lon_to},
};

/// How far out the nearest-place search will walk, in rings of 2° cells.
///
/// Eight is about 1 800 km. Past that the honest answer is *no place near*: a
/// photograph taken mid-Pacific has no city, and naming one two thousand
/// kilometres away would be the `pretend_film()` defect in another costume.
const RINGS: i32 = 8;

/// One feature's rings, walked without allocating them.
struct Feature<'a> {
    bbox: (i16, i16, i16, i16),
    rings: &'a [u8],
}

/// Walks a section of features, handing each to `see`.
fn each<'a, T>(
    bytes: &'a [u8],
    mut head: impl FnMut(&mut Cursor<'a>) -> Option<T>,
    mut see: impl FnMut(T, &Feature<'a>) -> bool,
) {
    let mut c = Cursor::of(bytes);
    let Some(n) = c.u16() else { return };
    for _ in 0..n {
        let Some(extra) = head(&mut c) else { return };
        let (Some(rings), Some(x0), Some(y0), Some(x1), Some(y1)) =
            (c.u16(), c.i16(), c.i16(), c.i16(), c.i16())
        else {
            return;
        };
        let from = c.at;
        // WALK THE RINGS TO FIND WHERE THE NEXT FEATURE STARTS, before handing
        // this one over: the callback may stop, and the cursor has to be right
        // either way.
        for _ in 0..rings {
            let Some(points) = c.u16() else { return };
            if c.skip(usize::from(points) * 4).is_none() {
                return;
            }
        }
        let feature = Feature {
            bbox: (x0, y0, x1, y1),
            rings: &bytes[from..c.at],
        };
        if !see(extra, &feature) {
            return;
        }
    }
}

/// Whether a point is inside a feature, by even-odd over all its rings.
fn inside(f: &Feature<'_>, at: Point) -> bool {
    let (x0, y0, x1, y1) = f.bbox;
    let qx = lon_to(at.lon);
    let qy = lat_to(at.lat);
    if qx < x0 || qx > x1 || qy < y0 || qy > y1 {
        return false;
    }
    let mut crossings = 0_u32;
    let mut c = Cursor::of(f.rings);
    while let Some(points) = c.u16() {
        let mut first: Option<(f32, f32)> = None;
        let mut last: Option<(f32, f32)> = None;
        for _ in 0..points {
            let (Some(qlon), Some(qlat)) = (c.i16(), c.i16()) else {
                return crossings % 2 == 1;
            };
            let here = (lon_of(qlon), lat_of(qlat));
            if first.is_none() {
                first = Some(here);
            }
            if let Some(prev) = last {
                crossings += u32::from(crosses(prev, here, at));
            }
            last = Some(here);
        }
        // AND THE RING IS CLOSED whether the data closed it or not: a ring
        // whose last point is not its first is a ring with one edge missing,
        // and that edge is as likely to be crossed as any other.
        if let (Some(a), Some(b)) = (last, first) {
            crossings += u32::from(crosses(a, b, at));
        }
    }
    crossings % 2 == 1
}

/// Whether the ray east from `at` crosses the edge `a`–`b`.
fn crosses(a: (f32, f32), b: (f32, f32), at: Point) -> bool {
    // THE HALF-OPEN RULE. See the header: `>=` here counts a vertex twice.
    if (a.1 > at.lat) == (b.1 > at.lat) {
        return false;
    }
    let t = (at.lat - a.1) / (b.1 - a.1);
    at.lon < a.0 + t * (b.0 - a.0)
}

/// Which country a point is in.
#[must_use]
pub fn country_at(atlas: &Atlas<'_>, at: Point, tongue: Tongue) -> Option<(Named, [u8; 2], u16)> {
    let mut found = None;
    let mut n = 0_u16;
    each(
        atlas.countries_bytes(),
        |c| {
            let iso = [c.u8()?, c.u8()?];
            let english = c.short()?;
            let portuguese = c.short()?;
            Some((iso, english, portuguese))
        },
        |(iso, english, portuguese), f| {
            let here = n;
            n = n.wrapping_add(1);
            if inside(f, at) {
                let name = match tongue {
                    Tongue::English => english,
                    Tongue::Portuguese => portuguese,
                };
                found = Some((name, iso, here));
                return false;
            }
            true
        },
    );
    found
}

/// Which region of that country a point is in.
#[must_use]
pub fn region_at(atlas: &Atlas<'_>, at: Point, country: u16) -> Option<Named> {
    let mut found = None;
    each(
        atlas.regions_bytes(),
        |c| {
            let owner = c.u16()?;
            let name = c.short()?;
            Some((owner, name))
        },
        |(owner, name), f| {
            if owner == country && inside(f, at) {
                found = Some(name);
                return false;
            }
            true
        },
    );
    found
}

/// The nearest named place, and how far away it is.
///
/// Walks the cell the point is in, then rings of neighbours outward, and stops
/// as soon as the best distance found is smaller than the distance to the near
/// edge of the next ring — so a place in the same cell costs one cell.
#[must_use]
pub fn place_near(atlas: &Atlas<'_>, at: Point) -> Option<(Named, u32)> {
    let bytes = atlas.places_bytes();
    let mut c = Cursor::of(bytes);
    let count = c.u32()? as usize;
    let grid_at = c.at;
    let records_at = grid_at + (CELLS_LON * CELLS_LAT + 1) * 4;
    let names_at = records_at + count * 12;
    let read_grid = |n: usize| -> Option<usize> {
        let b = bytes.get(grid_at + n * 4..grid_at + n * 4 + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };

    let home = cell_of(at.lat, at.lon);
    let (hx, hy) = ((home % CELLS_LON) as i32, (home / CELLS_LON) as i32);
    let mut best: Option<(String, u32, u16)> = None;
    for ring in 0..=RINGS {
        // THE RING IS ALREADY TOO FAR AWAY. Each cell is 2° of latitude, which
        // is about 222 km; a ring further out cannot hold anything nearer than
        // the ring before it starts.
        if let Some((_, metres, _)) = &best {
            let reach = (ring - 1).max(0) as u32 * 222_000;
            if *metres < reach {
                break;
            }
        }
        for dy in -ring..=ring {
            for dx in -ring..=ring {
                // ONLY THE EDGE of this ring: the inside was walked already.
                if ring > 0 && dx.abs() != ring && dy.abs() != ring {
                    continue;
                }
                let (x, y) = (hx + dx, hy + dy);
                if y < 0 || y >= CELLS_LAT as i32 {
                    continue;
                }
                // AND THE WORLD WRAPS EAST TO WEST, which the grid does not do
                // for free: a point at 179° has neighbours at -179°.
                let x = x.rem_euclid(CELLS_LON as i32) as usize;
                let n = y as usize * CELLS_LON + x;
                let (Some(from), Some(to)) = (read_grid(n), read_grid(n + 1)) else {
                    continue;
                };
                for k in from..to.min(count) {
                    let b = match bytes.get(records_at + k * 12..records_at + k * 12 + 12) {
                        Some(b) => b,
                        None => continue,
                    };
                    let lon = lon_of(i16::from_le_bytes([b[0], b[1]]));
                    let lat = lat_of(i16::from_le_bytes([b[2], b[3]]));
                    let pop = u16::from_le_bytes([b[6], b[7]]);
                    let name_at = u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize;
                    let metres = crate::metres_between(at, Point { lat, lon });
                    // POPULATION BREAKS A TIE and does nothing else: a
                    // photograph between a hamlet and a city names the city.
                    // It is never shown.
                    let better = match &best {
                        None => true,
                        Some((_, m, p)) => metres < *m || (metres == *m && pop > *p),
                    };
                    if better {
                        let mut nc = Cursor::of(bytes);
                        nc.at = names_at + name_at;
                        if let Some(name) = nc.short() {
                            best = Some((name, metres, pop));
                        }
                    }
                }
            }
        }
    }
    best.map(|(name, metres, _)| (name, metres))
}

/// Everything about where a coordinate is.
#[must_use]
pub fn address(atlas: &Atlas<'_>, at: Point, tongue: Tongue) -> Address {
    let mut out = Address::default();
    if let Some((name, iso, n)) = country_at(atlas, at, tongue) {
        out.region = region_at(atlas, at, n);
        out.country = Some((name, iso));
    }
    out.place = place_near(atlas, at);
    out
}

/// Every lake, as rings of degrees — for whoever draws.
#[must_use]
pub fn lakes(atlas: &Atlas<'_>) -> Vec<Vec<(f32, f32)>> {
    rings_of(atlas.lakes_bytes(), |_| Some(()), |(), _| true)
}

/// The rings of a whole section, in degrees.
pub(crate) fn rings_of<'a, T>(
    bytes: &'a [u8],
    head: impl FnMut(&mut Cursor<'a>) -> Option<T>,
    mut want: impl FnMut(&T, &(i16, i16, i16, i16)) -> bool,
) -> Vec<Vec<(f32, f32)>> {
    let mut out = Vec::new();
    each(bytes, head, |extra, f| {
        if want(&extra, &f.bbox) {
            let mut c = Cursor::of(f.rings);
            while let Some(points) = c.u16() {
                let mut ring = Vec::with_capacity(usize::from(points));
                for _ in 0..points {
                    let (Some(x), Some(y)) = (c.i16(), c.i16()) else {
                        break;
                    };
                    ring.push((lon_of(x), lat_of(y)));
                }
                out.push(ring);
            }
        }
        true
    });
    out
}
