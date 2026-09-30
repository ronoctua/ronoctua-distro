//! Reading our own table of the world.
//!
//! # The traps, before anything else
//!
//! ONE. **This file is LITTLE-endian throughout, and a shapefile is not.** The
//! two live next door to each other in this feature and a reader that carries
//! one habit into the other gets lengths in the billions — numbers that read as
//! corruption rather than as a byte order. The choice is little because every
//! machine this runs on is, so the arithmetic is free.
//!
//! TWO. **Every coordinate is quantised, and the step is written down where the
//! constant is.** A longitude is `i16` over 360°, which is 0.0054932° — about
//! 611 m at the equator; a latitude is `i16` over 180°, about 305 m. That is
//! not "good enough": at the 1:50 m scale this table is baked from, Natural
//! Earth's own vertex spacing is coarser than that nearly everywhere, so the
//! quantisation sits BELOW the resolution of the data. `atlas-quantisation-is-declared`
//! recomputes the metres and refuses a comment that has drifted from the code.
//!
//! THREE. **A border shared by two countries can diverge by one step.** They
//! are quantised independently, so a vertex can land on either side of a grid
//! line. It is invisible because countries are FILLED and their borders are
//! STROKED on top, so the stroke covers the seam. Say so, or somebody will
//! "fix" it with `f32` and quadruple the file.
//!
//! FOUR. **Sections are found by a table and never by walking.** A reader that
//! walked would have to know the length of everything before it, which is
//! exactly how a format grows a version it cannot skip.

use alloc::{string::String, vec::Vec};

use crate::Trouble;

/// One row of the downloadable catalogue.
///
/// A struct and not a tuple of five strings, which is what it was when it had
/// four: at five, `(String, String, String, String, String)` is a type whose
/// fields are told apart by counting commas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// Geofabrik's own id — `portugal`, `sudeste`.
    pub id: String,
    /// What it calls it.
    pub label: String,
    /// Where the shapefile archive is. **EMPTY when there is none**: the big
    /// regions are published as `.osm.pbf` only, and a row that cannot be
    /// fetched is drawn refusing rather than left out. See `gen-atlas.py`.
    pub url: String,
    /// The ISO alpha-2 codes it covers, comma-separated.
    pub isos: String,
    /// The id of the region that contains it, empty at the top level.
    pub parent: String,
}

/// The eight bytes every baked atlas starts with.
pub const MAGIC: &[u8; 8] = b"RONOATL1";

/// The layout this reader knows.
///
/// TWO since the catalogue's row gained its parent and stopped dropping the
/// regions that have no shapefile. The table and the reader are baked and
/// compiled by the same build, so they cannot disagree in practice — the
/// number is what makes a stale file on somebody's disk answer
/// `Trouble::AnotherVersion` instead of five strings read as four.
pub const VERSION: u16 = 2;

/// Degrees per step of a quantised longitude — 360° over the `i16` range.
///
/// `65_534` and not `65_536`: the range is `-32_767 ..= 32_767`, kept
/// symmetric so that zero is zero and the two poles of the mapping are the two
/// ends of the range. See the header for why 611 m is below the data.
pub const LON_STEPS: f32 = 65_534.0;

/// Degrees per step of a quantised latitude — 180° over the same range.
pub const LAT_STEPS: f32 = 65_534.0;

/// A quantised longitude back to degrees.
#[must_use]
pub fn lon_of(q: i16) -> f32 {
    f32::from(q) * (360.0 / LON_STEPS)
}

/// A quantised latitude back to degrees.
#[must_use]
pub fn lat_of(q: i16) -> f32 {
    f32::from(q) * (180.0 / LAT_STEPS)
}

/// Degrees to a quantised longitude. The generator's arithmetic, kept here so
/// there is one definition of the grid rather than two that can drift.
#[must_use]
pub fn lon_to(deg: f32) -> i16 {
    quantise(deg, 360.0 / LON_STEPS)
}

/// Degrees to a quantised latitude.
#[must_use]
pub fn lat_to(deg: f32) -> i16 {
    quantise(deg, 180.0 / LAT_STEPS)
}

fn quantise(deg: f32, step: f32) -> i16 {
    let n = deg / step;
    let r = if n < 0.0 { n - 0.5 } else { n + 0.5 } as i32;
    r.clamp(-32_767, 32_767) as i16
}

/// How many cells across the place index is, in each direction.
///
/// 2° cells: 180 across and 90 down. Small enough that a typical cell holds a
/// handful of places, large enough that the prefix array is 65 KB rather than a
/// megabyte.
pub const CELLS_LON: usize = 180;
/// See [`CELLS_LON`].
pub const CELLS_LAT: usize = 90;

/// Which cell a coordinate falls in.
#[must_use]
pub fn cell_of(lat: f32, lon: f32) -> usize {
    let x = (((lon + 180.0) / 2.0) as usize).min(CELLS_LON - 1);
    let y = (((lat + 90.0) / 2.0) as usize).min(CELLS_LAT - 1);
    y * CELLS_LON + x
}

/// One section of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Country polygons, with their names and ISO codes.
    Countries,
    /// States and provinces, each pointing at its country.
    Regions,
    /// Inland water, punched out of the land by painting.
    Lakes,
    /// Named places, with the grid that finds them.
    Places,
    /// The catalogue of downloadable street regions.
    Catalogue,
    /// The attribution text, carried inside the file so a copy of it is never
    /// separated from its licence.
    Credits,
    /// Where each country's NAME goes on a map — one point per ISO code.
    ///
    /// Section 7, and `VERSION` did not move for it: a reader that does not
    /// know a section skips it (see [`Atlas::open`]), so an older binary reads
    /// a newer table exactly as before.
    Anchors,
}

impl Kind {
    const fn of(n: u16) -> Option<Self> {
        match n {
            1 => Some(Self::Countries),
            2 => Some(Self::Regions),
            3 => Some(Self::Lakes),
            4 => Some(Self::Places),
            5 => Some(Self::Catalogue),
            6 => Some(Self::Credits),
            7 => Some(Self::Anchors),
            _ => None,
        }
    }
}

/// The baked world, borrowed.
///
/// The BYTES are handed in: this crate reads no file and no variable, which is
/// what lets a caller with no store — the installer — ask the same question,
/// and what keeps a store path out of anybody's compilation.
#[derive(Debug, Clone)]
pub struct Atlas<'a> {
    countries: &'a [u8],
    regions: &'a [u8],
    lakes: &'a [u8],
    places: &'a [u8],
    catalogue: &'a [u8],
    credits: &'a str,
    anchors: &'a [u8],
}

/// How many bytes one anchor takes: the ISO code, then a quantised longitude
/// and latitude.
pub(crate) const ANCHOR_BYTES: usize = 6;

/// A reader over a slice, little-endian, that refuses to walk off the end.
pub(crate) struct Cursor<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) at: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) const fn of(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn u8(&mut self) -> Option<u8> {
        let v = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(v)
    }

    pub(crate) fn u16(&mut self) -> Option<u16> {
        let b = self.bytes.get(self.at..self.at + 2)?;
        self.at += 2;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }

    pub(crate) fn i16(&mut self) -> Option<i16> {
        self.u16().map(|v| v as i16)
    }

    pub(crate) fn u32(&mut self) -> Option<u32> {
        let b = self.bytes.get(self.at..self.at + 4)?;
        self.at += 4;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A string with a one-byte length in front of it.
    pub(crate) fn short(&mut self) -> Option<String> {
        let n = usize::from(self.u8()?);
        let b = self.bytes.get(self.at..self.at + n)?;
        self.at += n;
        Some(String::from_utf8_lossy(b).into_owned())
    }

    pub(crate) fn skip(&mut self, n: usize) -> Option<()> {
        self.at = self.at.checked_add(n).filter(|a| *a <= self.bytes.len())?;
        Some(())
    }
}

impl<'a> Atlas<'a> {
    /// Opens a baked atlas over bytes somebody else read.
    ///
    /// # Errors
    ///
    /// [`Trouble::NotAnAtlas`] when the magic is wrong, [`Trouble::AnotherVersion`]
    /// when the layout is one this reader does not know, and
    /// [`Trouble::Truncated`] when a section claims to end past the end.
    pub fn open(bytes: &'a [u8]) -> Result<Self, Trouble> {
        if bytes.len() < 16 || &bytes[..8] != MAGIC {
            return Err(Trouble::NotAnAtlas);
        }
        let mut c = Cursor::of(bytes);
        c.skip(8).ok_or(Trouble::Truncated)?;
        let version = c.u16().ok_or(Trouble::Truncated)?;
        if version != VERSION {
            return Err(Trouble::AnotherVersion);
        }
        let sections = c.u16().ok_or(Trouble::Truncated)?;
        c.skip(4).ok_or(Trouble::Truncated)?;

        let mut out = Self {
            countries: &[],
            regions: &[],
            lakes: &[],
            places: &[],
            catalogue: &[],
            credits: "",
            anchors: &[],
        };
        for _ in 0..sections {
            let kind = c.u16().ok_or(Trouble::Truncated)?;
            c.skip(2).ok_or(Trouble::Truncated)?;
            let at = c.u32().ok_or(Trouble::Truncated)? as usize;
            let len = c.u32().ok_or(Trouble::Truncated)? as usize;
            let slice = bytes
                .get(at..at.checked_add(len).ok_or(Trouble::Truncated)?)
                .ok_or(Trouble::Truncated)?;
            match Kind::of(kind) {
                Some(Kind::Countries) => out.countries = slice,
                Some(Kind::Regions) => out.regions = slice,
                Some(Kind::Lakes) => out.lakes = slice,
                Some(Kind::Places) => out.places = slice,
                Some(Kind::Catalogue) => out.catalogue = slice,
                Some(Kind::Credits) => out.credits = core::str::from_utf8(slice).unwrap_or(""),
                Some(Kind::Anchors) => out.anchors = slice,
                // A SECTION THIS READER DOES NOT KNOW IS SKIPPED, not refused:
                // the table is what makes a newer bake readable by an older
                // binary, and refusing would throw that away for nothing.
                None => {}
            }
        }
        Ok(out)
    }

    /// Who the data belongs to, carried inside the file.
    ///
    /// GeoNames is CC BY 4.0 and the street packs are ODbL; both require
    /// attribution. Keeping the text INSIDE the table is what makes a copy of
    /// the file impossible to separate from its licence — the screen and the
    /// repository say it too, and this is the third of the four places.
    #[must_use]
    pub const fn credits(&self) -> &'a str {
        self.credits
    }

    pub(crate) const fn countries_bytes(&self) -> &'a [u8] {
        self.countries
    }

    pub(crate) const fn regions_bytes(&self) -> &'a [u8] {
        self.regions
    }

    pub(crate) const fn lakes_bytes(&self) -> &'a [u8] {
        self.lakes
    }

    pub(crate) const fn places_bytes(&self) -> &'a [u8] {
        self.places
    }

    /// The anchors section: `u16 n`, then `n` records of [`ANCHOR_BYTES`],
    /// sorted by ISO code. EMPTY in a table baked before it existed.
    pub(crate) const fn anchors_bytes(&self) -> &'a [u8] {
        self.anchors
    }

    /// Every region of the catalogue, downloadable or not.
    ///
    /// Read linearly because it is read once, by a view that is about to draw a
    /// list of it — five hundred entries and no index is the right trade.
    #[must_use]
    pub fn catalogue(&self) -> Vec<Region> {
        let mut c = Cursor::of(self.catalogue);
        let n = c.u16().unwrap_or(0);
        let mut out = Vec::with_capacity(usize::from(n));
        for _ in 0..n {
            let (Some(id), Some(label), Some(url), Some(isos), Some(parent)) =
                (c.short(), c.short(), c.short(), c.short(), c.short())
            else {
                break;
            };
            out.push(Region {
                id,
                label,
                url,
                isos,
                parent,
            });
        }
        out
    }
}
