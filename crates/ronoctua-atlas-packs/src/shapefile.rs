//! The shapefile pair, read for the two things a street pack keeps.
//!
//! # The traps, at the top, because every one of them is silent
//!
//! ONE — **the header is BIG-endian and the records are LITTLE-endian**, in
//! the same file. Reading one order throughout gives a file length of about
//! 1.6 billion, which reads as a corrupt download rather than as a byte order,
//! and the reader then walks off the end of a file that is perfectly good.
//!
//! TWO — **a record's content length is in 16-bit WORDS**, not in bytes.
//! Halving it or doubling it walks the records at the wrong stride, and the
//! first record still parses, so the failure appears several thousand records
//! later.
//!
//! THREE — **the `.dbf` joins the `.shp` by POSITION and by nothing else.**
//! There is no key in either file. One extra record on either side shifts
//! every attribute by one, silently, and the map then names a street with its
//! neighbour's name — which is worse than no name at all, because it reads as
//! true. This reader REFUSES when the counts differ, which is the only
//! detection there can be.
//!
//! FOUR — **a `.dbf` field is fixed-width and space-padded**, not
//! null-terminated, and a name shorter than the field carries the padding into
//! whatever is drawn.
//!
//! FIVE — **shape type 0 is the null shape**, and it is legal anywhere: a
//! record with no geometry at all. A reader that assumes every record has
//! points reads the next record's header as coordinates.
//!
//! # What it deliberately does not read
//!
//! POLYGONS. A street pack keeps ways (polylines) and places (points); the
//! area layers are the two thirds of the download that are thrown away. Rings
//! would bring the sixth trap with them — an inner ring is not marked as one,
//! so a polygon must be filled EVEN-ODD or the Caspian becomes land — and that
//! rule already has one home, in `ronoctua-atlas`.

use std::ops::ControlFlow;

use crate::Trouble;

/// The two shapes this reader knows.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// One coordinate, in degrees: `(lat, lon)`.
    Point(f64, f64),
    /// A way, as its vertices in order. Multi-part ways arrive as one run —
    /// the parts index is read and then flattened, because a street pack asks
    /// distance questions and a gap between two parts is not a segment.
    Line(Vec<(f64, f64)>),
    /// Legal, present in real files, and carrying nothing.
    Nothing,
}

/// Every record of a `.shp`, in file order, handed one at a time.
///
/// ONE AT A TIME AND NOT AS A `Vec`, because of what the caller does with
/// them: a country's roads layer is hundreds of megabytes of coordinates, and
/// collecting the geometry before turning it into segments would hold both
/// shapes of the same data in memory at once. `shapes` below is this function
/// collected, for a test that wants to look at four records.
///
/// `take` answers whether the walk goes on: a [`ControlFlow::Break`] ends it
/// at once, which is how the baking gives up between two records instead of
/// parsing the rest of a country for nobody.
///
/// # Errors
///
/// [`Trouble::Malformed`] when the file code is not 9994, a record header
/// reaches past the end, or a record claims more points than the bytes hold;
/// [`Trouble::GivenUp`] when `take` broke the walk.
pub fn each(
    bytes: &[u8],
    mut take: impl FnMut(usize, Shape) -> ControlFlow<()>,
) -> Result<usize, Trouble> {
    if bytes.len() < 100 || be_u32(bytes, 0) != 9994 {
        return Err(Trouble::Malformed);
    }
    let mut at = 100;
    let mut n = 0;
    while at + 8 <= bytes.len() {
        // THE WORDS, DOUBLED. Both numbers in this header are big-endian and
        // the length is in 16-bit words: see trap TWO.
        let words = be_u32(bytes, at + 4) as usize;
        let from = at + 8;
        let to = from.checked_add(words * 2).ok_or(Trouble::Malformed)?;
        if to > bytes.len() {
            return Err(Trouble::Malformed);
        }
        if take(n, one(&bytes[from..to])?).is_break() {
            return Err(Trouble::GivenUp);
        }
        n += 1;
        at = to;
    }
    Ok(n)
}

/// Every record of a `.shp`, collected.
///
/// # Errors
///
/// What [`each`] answers.
pub fn shapes(bytes: &[u8]) -> Result<Vec<Shape>, Trouble> {
    let mut out = Vec::new();
    each(bytes, |_, s| {
        out.push(s);
        ControlFlow::Continue(())
    })?;
    Ok(out)
}

/// One record's content, whose own fields are little-endian.
fn one(body: &[u8]) -> Result<Shape, Trouble> {
    if body.len() < 4 {
        return Err(Trouble::Malformed);
    }
    match le_u32(body, 0) {
        0 => Ok(Shape::Nothing),
        1 => {
            if body.len() < 20 {
                return Err(Trouble::Malformed);
            }
            // X IS LONGITUDE AND Y IS LATITUDE, in that order in the file and
            // in the other order in every sentence anybody says about a
            // coordinate. Swapped here, once.
            Ok(Shape::Point(f64_at(body, 12), f64_at(body, 4)))
        }
        3 | 5 => {
            if body.len() < 44 {
                return Err(Trouble::Malformed);
            }
            let parts = le_u32(body, 36) as usize;
            let points = le_u32(body, 40) as usize;
            let first = 44 + parts * 4;
            let need = first
                .checked_add(points.checked_mul(16).ok_or(Trouble::Malformed)?)
                .ok_or(Trouble::Malformed)?;
            if need > body.len() {
                return Err(Trouble::Malformed);
            }
            let mut v = Vec::with_capacity(points);
            for i in 0..points {
                let o = first + i * 16;
                v.push((f64_at(body, o + 8), f64_at(body, o)));
            }
            Ok(Shape::Line(v))
        }
        // Every other type — multipoint, and all the z and m variants — is a
        // shape this pack has no use for, and skipping it is not an error.
        _ => Ok(Shape::Nothing),
    }
}

/// One column of a `.dbf`, for every record, in file order.
///
/// Reads the WHOLE table once and hands back only the columns asked for,
/// because the alternative is walking a hundred megabytes twice.
///
/// # Errors
///
/// [`Trouble::Malformed`] when the header does not describe a table this
/// reader can walk, or a record reaches past the end.
pub fn columns(bytes: &[u8], want: &[&str]) -> Result<Vec<Vec<String>>, Trouble> {
    if bytes.len() < 32 {
        return Err(Trouble::Malformed);
    }
    let records = le_u32(bytes, 4) as usize;
    let header = usize::from(le_u16(bytes, 8));
    let width = usize::from(le_u16(bytes, 10));
    if header < 33 || width == 0 || header > bytes.len() {
        return Err(Trouble::Malformed);
    }
    // The field descriptors, thirty-two bytes each, until the 0x0D terminator.
    let mut fields: Vec<(String, usize)> = Vec::new();
    let mut at = 32;
    while at + 32 <= header && bytes[at] != 0x0D {
        let name: String = bytes[at..at + 11]
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| char::from(*b))
            .collect();
        fields.push((name.to_ascii_lowercase(), usize::from(bytes[at + 16])));
        at += 32;
    }
    let mut out: Vec<Vec<String>> = want.iter().map(|_| Vec::with_capacity(records)).collect();
    for r in 0..records {
        // THE DELETION FLAG IS THE FIRST BYTE of the record, and it is part of
        // the width: a reader that skipped it would take the flag as the first
        // letter of the first field of every row.
        let row = header + r * width;
        if row + width > bytes.len() {
            return Err(Trouble::Malformed);
        }
        let mut o = row + 1;
        for (name, len) in &fields {
            if let Some(k) = want.iter().position(|w| w == name) {
                let raw = &bytes[o..(o + len).min(bytes.len())];
                out[k].push(String::from_utf8_lossy(raw).trim().to_owned());
            }
            o += len;
        }
    }
    Ok(out)
}

const fn be_u32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

const fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

const fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn f64_at(b: &[u8], at: usize) -> f64 {
    let mut eight = [0_u8; 8];
    eight.copy_from_slice(&b[at..at + 8]);
    f64::from_le_bytes(eight)
}
