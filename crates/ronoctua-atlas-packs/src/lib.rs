//! A region's streets: where a pack lives, how one is baked, and what it
//! answers.
//!
//! # Why this is not `ronoctua-atlas`
//!
//! Because that crate is PURE — `no_std`, zero dependencies, no filesystem —
//! and this one is the opposite by necessity: it names
//! `$XDG_DATA_HOME/ronoctua/atlas/`, it opens files, it seeks in them. The
//! division is the one `ronoctua-syntax` already draws one subject along: the
//! thing that could be asked by a buffer stays pure, and everything that needs
//! a disk lives beside it rather than inside it.
//!
//! It does NOT depend on `ronoctua-atlas` either, and that is deliberate: the
//! transfers daemon bakes a pack, and a daemon that is extracted precisely so
//! the desktop's closure stays out of it must not gain the globe's geometry to
//! do it. The two meet in the SHELL, which holds both and puts a street into
//! an `Address`.
//!
//! # What a pack is
//!
//! One file per region, `streets.pack`, in our own layout — little-endian
//! throughout, sections found by a table, both for the reasons
//! `ronoctua_atlas::baked` writes out. It holds SEGMENTS in a grid and the
//! names they belong to, because the question asked of it is "what is the
//! nearest named way to this coordinate" and a grid answers that by reading
//! three cells instead of three million ways.
//!
//! The download it is baked from holds a hundred and one layers; two are kept
//! (`roads` and `places`) and of those, two columns (`name`, `fclass`).
//! Measured on Andorra, the smallest region Geofabrik publishes: 6.3 MB on the
//! wire, 13.8 MB unpacked, of which the two layers are 34%, and 27.8% of the
//! 8 799 ways carry a name.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::{
    fs::File,
    io::{Read as _, Seek as _, SeekFrom},
    path::{Path, PathBuf},
};

pub mod bake;
pub mod shapefile;

#[cfg(test)]
mod tests;

/// The eight bytes every baked pack starts with.
pub const MAGIC: &[u8; 8] = b"RONOPCK1";

/// The layout this reader knows.
pub const VERSION: u16 = 1;

/// The file inside a region's directory.
pub const FILE: &str = "streets.pack";

/// Degrees of one grid cell, before the cap.
///
/// A hundredth of a degree is about 1.1 km north to south, which is the scale
/// at which "the nearest street" stops being one street. Smaller cells would
/// make the index bigger than the segments it indexes for a rural region.
pub const CELL_DEGREES: f64 = 0.01;

/// The most cells a pack's grid may have.
///
/// The index is four bytes a cell and is read whole when a pack is opened, so
/// this is the memory a pack costs before it answers anything: a megabyte.
/// A region larger than this gets coarser cells rather than a bigger index.
pub const MOST_CELLS: usize = 262_144;

/// How far from a coordinate a way may be and still be its answer.
///
/// Two kilometres. Past that, "the nearest street" is a street somebody would
/// not recognise as theirs, and the honest answer is the place instead — the
/// same argument `ronoctua_atlas::lookup` makes for its eight-ring ceiling.
pub const REACH_METRES: f64 = 2_000.0;

/// The same, for a named place.
///
/// Wider, because a place is a POINT standing for an area: the coordinate of a
/// district is somewhere in the middle of it, so a photograph taken at its edge
/// is kilometres from the point and still in the district. Twenty kilometres
/// is the distance at which the baked world's own town would be the better
/// answer anyway.
pub const REACH_PLACE_METRES: f64 = 20_000.0;

/// How many bytes of header follow the magic and the version.
///
/// Fixed, and read in one go: a header walked field by field is a header that
/// grows a version it cannot skip, which is trap FOUR of
/// `ronoctua_atlas::baked` said one file along.
pub const META: usize = 64;

/// The most named places a pack may carry, because they are held in memory.
///
/// Two hundred thousand is twelve bytes each, which is two and a half
/// megabytes — and it is far above what a region has: Andorra has 167.
pub const MOST_PLACES: usize = 200_000;

/// What can go wrong with a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// The bytes are not a pack, a shapefile or a `.dbf` this reader walks.
    Malformed,
    /// A pack of a version this reader does not know.
    AnotherVersion,
    /// The download does not carry the two layers a pack is made of.
    NotARegionPack,
    /// The `.shp` and the `.dbf` disagree about how many records there are —
    /// which is the one detectable form of the join being wrong. See
    /// `shapefile`'s trap THREE.
    Mismatched,
    /// The zip could not be walked.
    Zip(ronoctua_zip::Trouble),
    /// The filesystem said no. Carries what it said, as a string, because the
    /// caller writes it into a transfer's own trouble and never matches on it.
    Disk(String),
}

impl From<ronoctua_zip::Trouble> for Trouble {
    fn from(t: ronoctua_zip::Trouble) -> Self {
        Self::Zip(t)
    }
}

impl From<std::io::Error> for Trouble {
    fn from(e: std::io::Error) -> Self {
        Self::Disk(e.to_string())
    }
}

/// Where the packs live under a data root.
///
/// The ROOT is an argument and never found here, which is the rule
/// `ronoctua-xdg` states: a function that found `$XDG_DATA_HOME` on its own
/// would let a unit test write into whoever ran it.
#[must_use]
pub fn root_of(data: &Path) -> PathBuf {
    data.join("atlas")
}

/// One installed region, as the directory has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// Geofabrik's id — `portugal`, `brazil/sudeste`, flattened to one segment.
    pub id: String,
    /// What it is called, as the catalogue calls it.
    pub label: String,
    /// The DATED name the download redirected to — `portugal-260914-free.shp.zip`.
    /// This is a pack's version, and it is the only one there is: the
    /// `-latest-` URL is a redirect, and what it redirects to is the date.
    pub dated: String,
    /// How much disk it takes.
    pub bytes: u64,
    /// How many named ways it holds.
    pub ways: u32,
}

/// Every region installed under `root`, by id.
///
/// A directory with no readable pack in it is not listed and is not an error:
/// a transfer that was given up half way leaves one, and the queue is where
/// that is said.
#[must_use]
pub fn installed(root: &Path) -> Vec<Installed> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in dir.flatten() {
        let path = entry.path().join(FILE);
        let Ok(bytes) = std::fs::metadata(&path).map(|m| m.len()) else {
            continue;
        };
        if let Ok(pack) = Pack::open(&path) {
            out.push(Installed {
                id: pack.id,
                label: pack.label,
                dated: pack.dated,
                bytes,
                ways: pack.ways,
            });
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// Removes an installed region, whole.
///
/// # Errors
///
/// The directory could not be removed.
pub fn remove(root: &Path, id: &str) -> Result<(), Trouble> {
    let dir = root.join(flat(id));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// The path a region's pack is written to and read from.
#[must_use]
pub fn path_of(root: &Path, id: &str) -> PathBuf {
    root.join(flat(id)).join(FILE)
}

/// A catalogue id as ONE directory name.
///
/// Geofabrik's ids carry slashes (`brazil/sudeste`), and a slash in a path is
/// a second directory: `atlas/brazil/sudeste/streets.pack` would make `brazil`
/// look like an installed region with no pack in it. Flattened, the id is
/// still what the file says it is — `Pack::id` is read from inside the file,
/// never from the directory's name.
#[must_use]
pub fn flat(id: &str) -> String {
    id.replace('/', "-")
}

/// An installed region, open and ready to be asked.
///
/// The SEGMENTS stay on disk. A pack for a large country is a hundred
/// megabytes and the answer to one question is three cells of it, so what is
/// held in memory is the header, the names and the cell index — and the
/// segments are read where the grid says they are.
pub struct Pack {
    file: File,
    id: String,
    label: String,
    dated: String,
    ways: u32,
    /// `(lat_min, lon_min)` in degrees, and the size of a cell.
    origin: (f64, f64),
    cell: (f64, f64),
    cols: usize,
    rows: usize,
    /// One more than the number of cells: each cell's segments are the run
    /// between two of these.
    index: Vec<u32>,
    /// Where the segment table starts in the file.
    segments_at: u64,
    names: Vec<String>,
    /// Named places — a quantised coordinate and a name — read whole. This is
    /// what gives a district where the baked world gives a town.
    places: Vec<(i32, i32, u32)>,
}

/// One named way near a coordinate: its name and how far away it is in metres.
pub type Near = (String, u32);

impl Pack {
    /// Opens a baked pack.
    ///
    /// # Errors
    ///
    /// [`Trouble::Malformed`] when the file does not start with [`MAGIC`] or a
    /// section reaches past the end, [`Trouble::AnotherVersion`] for a layout
    /// this reader does not know, [`Trouble::Disk`] when it cannot be read.
    pub fn open(path: &Path) -> Result<Self, Trouble> {
        let mut file = File::open(path)?;
        let mut head = [0_u8; 10];
        file.read_exact(&mut head)?;
        if &head[..8] != MAGIC {
            return Err(Trouble::Malformed);
        }
        if u16::from_le_bytes([head[8], head[9]]) != VERSION {
            return Err(Trouble::AnotherVersion);
        }
        let mut meta = [0_u8; META];
        file.read_exact(&mut meta)?;
        let f64_at = |o: usize| {
            let mut e = [0_u8; 8];
            e.copy_from_slice(&meta[o..o + 8]);
            f64::from_le_bytes(e)
        };
        let u32_at =
            |o: usize| u32::from_le_bytes([meta[o], meta[o + 1], meta[o + 2], meta[o + 3]]);
        let origin = (f64_at(0), f64_at(8));
        let cell = (f64_at(16), f64_at(24));
        let cols = u32_at(32) as usize;
        let rows = u32_at(36) as usize;
        let ways = u32_at(40);
        let index_at = u64::from(u32_at(44));
        let segments_at = u64::from(u32_at(48));
        let strings_at = u64::from(u32_at(52));
        let places_at = u64::from(u32_at(56));
        let places_count = u32_at(60) as usize;
        let (id, label, dated) = read_three(&mut file)?;

        let cells = cols.checked_mul(rows).ok_or(Trouble::Malformed)?;
        if cells == 0 || cells > MOST_CELLS {
            return Err(Trouble::Malformed);
        }
        file.seek(SeekFrom::Start(index_at))?;
        let mut raw = vec![0_u8; (cells + 1) * 4];
        file.read_exact(&mut raw)?;
        let index: Vec<u32> = raw
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        // THE PLACES ARE HELD AND THE SEGMENTS ARE NOT, and the difference is
        // arithmetic rather than taste: a region has tens of thousands of
        // named places and millions of segments, so one is a scan of a few
        // hundred kilobytes and the other is why there is a grid at all.
        if places_count > MOST_PLACES {
            return Err(Trouble::Malformed);
        }
        file.seek(SeekFrom::Start(places_at))?;
        let mut raw = vec![0_u8; places_count * 12];
        file.read_exact(&mut raw)?;
        let places: Vec<(i32, i32, u32)> = raw
            .chunks_exact(12)
            .map(|c| {
                let at = |o: usize| i32::from_le_bytes([c[o], c[o + 1], c[o + 2], c[o + 3]]);
                #[allow(clippy::cast_sign_loss)]
                (at(0), at(4), at(8) as u32)
            })
            .collect();

        file.seek(SeekFrom::Start(strings_at))?;
        let mut rest = Vec::new();
        file.read_to_end(&mut rest)?;
        let names = strings_of(&rest)?;

        Ok(Self {
            file,
            id,
            label,
            dated,
            ways,
            origin,
            cell,
            cols,
            rows,
            index,
            segments_at,
            names,
            places,
        })
    }

    /// Geofabrik's id for this region.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What it is called.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The dated name it was baked from, which is its version.
    #[must_use]
    pub fn dated(&self) -> &str {
        &self.dated
    }

    /// The nearest named way, and how far away it is in metres.
    ///
    /// Reads the coordinate's cell and its eight neighbours — a way is a line
    /// and its nearest point can be in the cell next door — and nothing
    /// further: past [`REACH_METRES`] the answer is `None`, which is what lets
    /// the panel fall back to the place instead of naming a road two valleys
    /// away.
    ///
    /// # Errors
    ///
    /// [`Trouble::Disk`] when the segments cannot be read.
    pub fn street_near(&mut self, lat: f64, lon: f64) -> Result<Option<Near>, Trouble> {
        let Some((cx, cy)) = self.cell_of(lat, lon) else {
            return Ok(None);
        };
        let mut best: Option<(f64, u32)> = None;
        for dy in -1_i64..=1 {
            for dx in -1_i64..=1 {
                let (Ok(x), Ok(y)) = (
                    usize::try_from(cx as i64 + dx),
                    usize::try_from(cy as i64 + dy),
                ) else {
                    continue;
                };
                if x >= self.cols || y >= self.rows {
                    continue;
                }
                for (name, d) in self.segments_in(y * self.cols + x, lat, lon)? {
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, name));
                    }
                }
            }
        }
        Ok(best.and_then(|(d, n)| {
            (d <= REACH_METRES).then(|| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let metres = d.round() as u32;
                (
                    self.names.get(n as usize).cloned().unwrap_or_default(),
                    metres,
                )
            })
        }))
    }

    /// Which cell a coordinate falls in, when it falls in this pack at all.
    fn cell_of(&self, lat: f64, lon: f64) -> Option<(usize, usize)> {
        let x = (lon - self.origin.1) / self.cell.1;
        let y = (lat - self.origin.0) / self.cell.0;
        if x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (x, y) = (x as usize, y as usize);
        (x < self.cols && y < self.rows).then_some((x, y))
    }

    /// Every segment of one cell, as `(name, metres)` from the coordinate.
    fn segments_in(&mut self, cell: usize, lat: f64, lon: f64) -> Result<Vec<(u32, f64)>, Trouble> {
        let (from, to) = (
            *self.index.get(cell).unwrap_or(&0),
            *self.index.get(cell + 1).unwrap_or(&0),
        );
        if to <= from {
            return Ok(Vec::new());
        }
        let count = (to - from) as usize;
        self.file
            .seek(SeekFrom::Start(self.segments_at + u64::from(from) * 20))?;
        let mut raw = vec![0_u8; count * 20];
        self.file.read_exact(&mut raw)?;
        Ok(raw
            .chunks_exact(20)
            .map(|c| {
                let n = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                let at = |o: usize| {
                    f64::from(i32::from_le_bytes([c[o], c[o + 1], c[o + 2], c[o + 3]])) / 1e6
                };
                (n, to_segment(lat, lon, (at(4), at(8)), (at(12), at(16))))
            })
            .collect())
    }
}

impl Pack {
    /// The nearest named place, and how far away it is in metres.
    ///
    /// A LINEAR SCAN, and the reason is the count: a region has tens of
    /// thousands of these, an index over them would be bigger than they are,
    /// and the scan is microseconds. What it gives that the baked world does
    /// not is the DISTRICT — `cities500` stops at towns of five hundred
    /// people, and a pack knows the neighbourhood.
    #[must_use]
    pub fn place_near(&self, lat: f64, lon: f64) -> Option<Near> {
        let mut best: Option<(f64, u32)> = None;
        for (plat, plon, name) in &self.places {
            let (a, b) = (f64::from(*plat) / 1e6, f64::from(*plon) / 1e6);
            let d = to_segment(lat, lon, (a, b), (a, b));
            if best.is_none_or(|(x, _)| d < x) {
                best = Some((d, *name));
            }
        }
        best.and_then(|(d, n)| {
            (d <= REACH_PLACE_METRES).then(|| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let metres = d.round() as u32;
                (
                    self.names.get(n as usize).cloned().unwrap_or_default(),
                    metres,
                )
            })
        })
    }
}

/// Metres from a coordinate to a segment, on the flat.
///
/// FLAT ON PURPOSE, and the reason is the scale: a segment of a street is tens
/// of metres long and the question is asked within two kilometres of it, where
/// a sphere and a plane differ by less than the quantisation. Longitude is
/// scaled by `cos(lat)` — without that, a degree of longitude in Reykjavík
/// would be measured as a degree of latitude, and every street there would
/// come out twice as far away as it is.
#[must_use]
pub fn to_segment(lat: f64, lon: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
    const METRES_PER_DEGREE: f64 = 111_320.0;
    let k = lat.to_radians().cos();
    let (px, py) = ((lon - a.1) * k, lat - a.0);
    let (vx, vy) = ((b.1 - a.1) * k, b.0 - a.0);
    let len = vx.mul_add(vx, vy * vy);
    let t = if len <= f64::EPSILON {
        0.0
    } else {
        (px.mul_add(vx, py * vy) / len).clamp(0.0, 1.0)
    };
    let (dx, dy) = (t.mul_add(-vx, px), t.mul_add(-vy, py));
    dx.hypot(dy) * METRES_PER_DEGREE
}

/// Three short strings, one after another, each a `u16` length and its bytes.
fn read_three(file: &mut File) -> Result<(String, String, String), Trouble> {
    let mut one = || -> Result<String, Trouble> {
        let mut n = [0_u8; 2];
        file.read_exact(&mut n)?;
        let mut s = vec![0_u8; usize::from(u16::from_le_bytes(n))];
        file.read_exact(&mut s)?;
        Ok(String::from_utf8_lossy(&s).into_owned())
    };
    Ok((one()?, one()?, one()?))
}

/// The name table: a count, then that many short strings.
fn strings_of(bytes: &[u8]) -> Result<Vec<String>, Trouble> {
    if bytes.len() < 4 {
        return Err(Trouble::Malformed);
    }
    let n = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let mut out = Vec::with_capacity(n.min(1_000_000));
    let mut at = 4;
    for _ in 0..n {
        if at + 2 > bytes.len() {
            return Err(Trouble::Malformed);
        }
        let len = usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
        at += 2;
        if at + len > bytes.len() {
            return Err(Trouble::Malformed);
        }
        out.push(String::from_utf8_lossy(&bytes[at..at + len]).into_owned());
        at += len;
    }
    Ok(out)
}
