//! Where a coordinate is, and how to draw a globe of it.
//!
//! # What this crate is, and whose it is
//!
//! It is nobody's. The file manager asks it where a photograph was taken and
//! draws a globe of the answer, and that makes the manager its FIRST CALLER —
//! never its owner. Three other places in this system already hold a latitude
//! and a longitude and have nothing to show for them: the glance's places
//! (`assets/world.nix` carries a coordinate per place, deliberately, because
//! *"a name would need a geocoder, which is a second network service"*), the
//! locale view (`locale::took_a_coordinate` parses a pair a person typed and
//! then cannot confirm it means what they meant), and the installer, whose own
//! comment says the country codes and coordinates of `zone1970.tab` "are for a
//! map this installer does not draw".
//!
//! So the shape is `ronoctua-syntax`'s, one subject along: **it gives back a
//! ROLE and never a colour** ([`Ink`]), and geometry in the box's own pixels
//! and never pixels. Whoever draws turns one into the other, in one place.
//!
//! # And why a geocoder is allowed here
//!
//! `assets/world.nix` refused one in writing — *"a second network service and a
//! second thing to be wrong about"*. That argument is about the NETWORK, and
//! this one never opens a socket: the whole world it knows arrives as a slice
//! of bytes the caller read, and the answer is arithmetic over it.
//!
//! # What it does not have
//!
//! No `std`, no allocator beyond `alloc`, no dependency, no filesystem, no
//! environment, no clock and no thread. `Atlas::open` takes BYTES, which is
//! what lets the installer — where there is no store yet — ask the same
//! question, and what keeps a store path out of anybody's compilation.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;

mod baked;
mod globe;
mod lookup;

pub use baked::Atlas;
pub use globe::{
    Drawing, Ink, MARKS_CEILING, Mark, Marks, Placed, Pose, Shape, VERTEX_CEILING, framing, placed,
    project, tier,
};
pub use lookup::{
    address, anchor_of, country_at, country_name, folded, in_country, lakes, place_named,
    place_near, places, region_at,
};

/// A coordinate, in signed decimal degrees.
///
/// `f32` and not `f64`: five decimals is about a metre, which is as much as a
/// phone knows, and everything downstream of this — the projection, the
/// rasteriser — works in `f32` anyway.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// North is positive.
    pub lat: f32,
    /// East is positive.
    pub lon: f32,
}

/// What the world data could not answer.
///
/// A refusal is a VALUE and not an absence, which is the rule the preview panel
/// already keeps: a blank panel and a refused one look identical on screen, and
/// only one of them has a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// These bytes are not a baked atlas.
    NotAnAtlas,
    /// They are, but of a version this reader does not know.
    AnotherVersion,
    /// A section ends past the end of the file.
    Truncated,
}

/// A name, and nothing else about it.
///
/// It is DATA and not a key: a place is called what it is called, and looking
/// for `[world-place-…]` would be looking for a translation of a name somebody
/// wrote down. The structure of the sentence around it is a key, and that lives
/// one layer up, in whoever draws.
pub type Named = String;

/// Where a coordinate is.
///
/// Every field is optional and the reason is the same for all of them: the
/// answer is what the data has, and saying "unknown" is worse than saying
/// nothing — a panel of "unknown" is a panel that wastes its lines.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Address {
    /// The country, by name in the language asked for, and its ISO alpha-2.
    pub country: Option<(Named, [u8; 2])>,
    /// The state, province or region it falls in.
    pub region: Option<Named>,
    /// The nearest named place, and how far away it is in metres.
    pub place: Option<(Named, u32)>,
    /// The nearest named way, when a region's streets are installed.
    pub street: Option<(Named, u32)>,
}

impl Address {
    /// Whether anything at all was found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.country.is_none()
            && self.region.is_none()
            && self.place.is_none()
            && self.street.is_none()
    }

    /// Which shape the sentence around it takes.
    ///
    /// The four are a Fluent selector's branches, and which one is chosen is
    /// arithmetic over what was found — so the ORDER of the words stays the
    /// language's and the CHOICE of sentence stays ours.
    #[must_use]
    pub fn shape(&self) -> &'static str {
        if self.street.is_some() {
            "street"
        } else if self.place.is_some() {
            "place"
        } else if self.region.is_some() {
            "region"
        } else {
            "country"
        }
    }
}

/// Which language a name is wanted in.
///
/// TWO, and they are the two this system declares (`assets/i18n/languages.nix`).
/// It is an enum and not a string because the baked table carries exactly these
/// two spellings per country: a third would be a re-bake, not a lookup that
/// silently finds nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tongue {
    /// `en-GB`.
    English,
    /// `pt-BR`.
    Portuguese,
}

/// How far apart two coordinates are, in metres.
///
/// Equirectangular with a cosine correction, and NOT haversine: at the scales
/// this answers — a place within a few hundred kilometres — the error is
/// metres, and the panel rounds to kilometres. Haversine would be four
/// transcendentals per candidate in a loop that walks thousands of them.
#[must_use]
pub fn metres_between(a: Point, b: Point) -> u32 {
    const EARTH: f32 = 6_371_000.0;
    let mid = (a.lat + b.lat) * 0.5 * core::f32::consts::PI / 180.0;
    let dx = (b.lon - a.lon) * core::f32::consts::PI / 180.0 * cos(mid);
    let dy = (b.lat - a.lat) * core::f32::consts::PI / 180.0;
    let d = EARTH * sqrt(dx * dx + dy * dy);
    if d <= 0.0 { 0 } else { d as u32 }
}

/// `cos`, without `std`.
///
/// `core` has no floating-point transcendentals — they live in `std`, which
/// this crate does not have and will not take for four functions. A Chebyshev-
/// style polynomial over the reduced argument is exact to about 1e-7, which is
/// a millimetre at the radius of the Earth and a fraction of a pixel on a
/// globe.
#[must_use]
pub fn cos(x: f32) -> f32 {
    sin(x + core::f32::consts::FRAC_PI_2)
}

/// `sin`, without `std`. See [`cos`].
///
/// # Why the argument is folded TWICE
///
/// The series below is a Taylor one, and a Taylor series is only good near
/// zero. Reduced to `[-PI, PI]` alone it is off by **8 per cent** at the ends —
/// measured, not feared: `(PI^9)/9!` is 0.082, and a horizon test that wrong
/// puts half of a continent on the other side of the planet. The second fold,
/// `sin(x) = sin(PI - x)`, brings the argument into `[-PI/2, PI/2]`, where the
/// same series is off by about 5e-6: a millimetre at the radius of the Earth
/// and a thousandth of a pixel on a globe.
#[must_use]
pub fn sin(x: f32) -> f32 {
    use core::f32::consts::{FRAC_PI_2, PI};
    // ONE: into [-PI, PI].
    let mut a = x % (2.0 * PI);
    if a > PI {
        a -= 2.0 * PI;
    } else if a < -PI {
        a += 2.0 * PI;
    }
    // TWO: into [-PI/2, PI/2], where the series is honest.
    if a > FRAC_PI_2 {
        a = PI - a;
    } else if a < -FRAC_PI_2 {
        a = -PI - a;
    }
    let x2 = a * a;
    a * (1.0 - x2 * (1.0 / 6.0 - x2 * (1.0 / 120.0 - x2 * (1.0 / 5040.0 - x2 / 362_880.0))))
}

/// `sqrt`, without `std` — two Newton steps off a bit-twiddled seed.
///
/// The seed is the classic exponent halving, which is exact for powers of four
/// and within 3% everywhere else; two steps take that under 1e-6.
#[must_use]
pub fn sqrt(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut g = f32::from_bits((x.to_bits() >> 1) + 0x1fc0_0000);
    g = 0.5 * (g + x / g);
    g = 0.5 * (g + x / g);
    g
}

/// `atan2`, without `std`.
///
/// Used once per ring that crosses the horizon, to say where on the rim it
/// left. The polynomial is the usual one for `atan` on `[0, 1]` with the
/// octant folded in; it is good to about 1e-5 radians, which at a 200-pixel
/// radius is a fiftieth of a pixel.
#[must_use]
pub fn atan2(y: f32, x: f32) -> f32 {
    use core::f32::consts::PI;
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    let ax = if x < 0.0 { -x } else { x };
    let ay = if y < 0.0 { -y } else { y };
    let (n, d, swap) = if ax >= ay {
        (ay, ax, false)
    } else {
        (ax, ay, true)
    };
    let t = n / d;
    let t2 = t * t;
    // atan(t) on [0,1] — the five-term minimax, and FIVE and not four: with
    // the last coefficient left off it is out by 0.021 rad at t = 1, which is
    // more than a degree of rim. Measured against `FRAC_PI_4` in the tests.
    let mut a =
        t * (0.999_866 + t2 * (-0.330_299 + t2 * (0.180_141 + t2 * (-0.085_133 + t2 * 0.020_835))));
    if swap {
        a = PI * 0.5 - a;
    }
    if x < 0.0 {
        a = PI - a;
    }
    if y < 0.0 { -a } else { a }
}

/// Everything a `Vec` of these is, for the caller that wants one line.
#[must_use]
pub fn joined(parts: &[&str], between: &str) -> String {
    let mut out = String::new();
    for (n, p) in parts.iter().enumerate() {
        if n > 0 {
            out.push_str(between);
        }
        out.push_str(p);
    }
    out
}

#[cfg(test)]
mod tests;
