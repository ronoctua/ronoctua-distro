//! IGES: eighty-column records, and the curves among them.
//!
//! # The one reader here with no file of the owner's behind it
//!
//! Everything else in this module was pointed at a real file before it was
//! believed. There is no `.iges` on this machine and nothing in the tree writes
//! one, so this is proved against fixtures written from the specification —
//! which tests this reader's understanding of the format and not what a CAD
//! program actually emits.
//!
//! What that buys is the shape of the failure. Nothing here guesses: an entity
//! type it does not know is SKIPPED and a file with no curve at all is
//! [`Refusal::Failed`]. The way this reader fails is a piece with fewer edges
//! than it has, or none — never a piece that is the wrong shape.
//!
//! # Why the delimiters are read out of the file
//!
//! The Global section's first two fields ARE the delimiters, written as
//! Hollerith strings — `1H,` and `1H;`. A file from a European system may use
//! a semicolon for the parameter delimiter because the comma is its decimal
//! point, and a reader that assumed a comma would split every number in half.
//!
//! # Why a record is 80 columns and not a line
//!
//! Because the section letter is at column 73 and the sequence number after
//! it, and a file written by a mainframe-era tool has no line endings at all —
//! it is one run of bytes, divided by counting. Records are therefore taken 80
//! at a time from a stream with the line endings removed.

use std::collections::HashMap;

use super::{Census, Read, Shape, Wires};
use crate::Refusal;

/// How wide one record is.
const RECORD: usize = 80;

/// Which column carries the section letter, counting from zero.
const SECTION_AT: usize = 72;

/// How many segments a full turn of an arc is drawn with. See the same
/// constant in [`super::step`], which makes the argument.
const SEGMENTS_PER_TURN: usize = 64;

/// How many segments a spline is drawn with.
const SEGMENTS_PER_SPLINE: usize = 48;

/// How many entities are read at all.
const ENTITY_CEILING: usize = 200_000;

/// Reads an IGES file for its curves.
///
/// # Errors
///
/// [`Refusal::Failed`] when the sections are not there or hold no curve, and
/// [`Refusal::TooBig`] past [`ENTITY_CEILING`].
pub fn read(bytes: &[u8]) -> Result<Read, Refusal> {
    // Latin-1, for the reason [`super::step::lenient`] gives: the format is
    // ASCII and writers put code-page bytes in the Global section anyway.
    let flat: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|b| *b != b'\n' && *b != b'\r')
        .collect();
    let mut global = String::new();
    let mut directory: Vec<String> = Vec::new();
    let mut parameters: HashMap<usize, String> = HashMap::new();

    for record in flat.chunks(RECORD) {
        if record.len() < RECORD {
            break;
        }
        let text: String = record.iter().map(|b| char::from(*b)).collect();
        match record[SECTION_AT] {
            b'G' => global.push_str(&text[..SECTION_AT]),
            b'D' => directory.push(text[..SECTION_AT].to_owned()),
            b'P' => {
                // Columns 66..72 hold the directory entry this belongs to.
                let owner: usize = text[SECTION_AT - 8..SECTION_AT]
                    .trim()
                    .parse()
                    .unwrap_or_default();
                parameters
                    .entry(owner)
                    .or_default()
                    .push_str(&text[..SECTION_AT - 8]);
            }
            _ => {}
        }
    }
    if directory.is_empty() {
        return Err(Refusal::Failed);
    }

    let (param_delim, record_delim) = delimiters(&global);
    let fields = split(&global, param_delim, record_delim);
    let (millimetres, unit) = millimetres_per(&fields);

    let mut wires = Wires::empty();
    let mut census = Census::default();
    // The directory is two records per entity, and the SEQUENCE of the first
    // is what a parameter record points back at.
    let entries: Vec<(usize, Vec<i64>)> = directory
        .chunks(2)
        .enumerate()
        .filter(|(_, pair)| pair.len() == 2)
        .map(|(i, pair)| {
            let mut numbers: Vec<i64> = Vec::with_capacity(18);
            for record in pair {
                for k in 0..9 {
                    numbers.push(record[k * 8..k * 8 + 8].trim().parse().unwrap_or_default());
                }
            }
            (i * 2 + 1, numbers)
        })
        .collect();
    if entries.len() > ENTITY_CEILING {
        return Err(Refusal::TooBig(bytes.len() as u64));
    }

    // The placements, so a curve carried by one can be put where it belongs.
    let mut placements: HashMap<usize, [f64; 12]> = HashMap::new();
    for (sequence, de) in &entries {
        if de.first() != Some(&124) {
            continue;
        }
        let Some(text) = parameters.get(sequence) else {
            continue;
        };
        let n = numbers(text, param_delim, record_delim);
        if n.len() >= 13 {
            let mut m = [0.0_f64; 12];
            m.copy_from_slice(&n[1..13]);
            placements.insert(*sequence, m);
        }
    }

    for (sequence, de) in &entries {
        let Some(kind) = de.first().copied() else {
            continue;
        };
        // Field 7 of the first directory record is the placement, by sequence.
        #[allow(clippy::cast_sign_loss)]
        let at = de
            .get(6)
            .copied()
            .filter(|n| *n > 0)
            .map(|n| n as usize)
            .and_then(|n| placements.get(&n).copied());
        let Some(text) = parameters.get(sequence) else {
            continue;
        };
        let n = numbers(text, param_delim, record_delim);
        let points = match kind {
            110 => line(&n),
            100 => circular_arc(&n),
            126 => spline(&n),
            _ => continue,
        };
        census.edges += 1;
        let placed: Vec<[f32; 3]> = points
            .into_iter()
            .map(|p| {
                let p = at.map_or(p, |m| through(&m, p));
                #[allow(clippy::cast_possible_truncation)]
                [
                    (p[0] * millimetres) as f32,
                    (p[1] * millimetres) as f32,
                    (p[2] * millimetres) as f32,
                ]
            })
            .collect();
        wires.along(&placed);
    }
    if wires.count() == 0 {
        return Err(Refusal::Failed);
    }
    let mut out = Read {
        body: super::Body::cage(wires),
        shape: Shape::Iges,
        unit: unit.map(str::to_owned),
        objects: 1,
        notes: Vec::new(),
        census,
    };
    // THE NAMES ARE 3MF'S, so the panel has one map from name to label.
    // FIELD FOUR IS THE FILE'S OWN NAME, five the system that wrote it,
    // eighteen the timestamp and twenty-one the author — the numbering is the
    // specification's, which is why `split` leaves slot zero empty.
    for (at, name) in [(4, "SourceName"), (5, "Application"), (21, "Designer")] {
        if let Some(v) = fields
            .get(at)
            .map(|f| hollerith(f))
            .filter(|v| !v.is_empty())
        {
            out.notes.push(((*name).to_owned(), v));
        }
    }
    if let Some(v) = fields
        .get(18)
        .map(|f| hollerith(f))
        .filter(|v| !v.is_empty())
    {
        out.notes.push(("CreationDate".to_owned(), v));
    }
    Ok(out)
}

/// The parameter and record delimiters the Global section declares.
///
/// They ARE its first two fields, and both are optional: a file that begins
/// with a comma has left them out and taken the defaults.
fn delimiters(global: &str) -> (char, char) {
    let text = global.trim_start();
    if text.starts_with(',') || !text.starts_with("1H") {
        return (',', ';');
    }
    let raw: Vec<char> = text.chars().collect();
    let param = raw.get(2).copied().unwrap_or(',');
    // `1H,1H;` — the second declaration follows the first, past its delimiter.
    let after: String = raw.iter().skip(3).collect();
    let after = after.trim_start_matches(param);
    let record = if after.starts_with("1H") {
        after.chars().nth(2).unwrap_or(';')
    } else {
        ';'
    };
    (param, record)
}

/// The Global section's fields, in order and one-indexed to match the
/// specification's own numbering — slot zero is left empty on purpose, so a
/// reader of this code can check a field number against the standard without
/// subtracting one.
fn split(global: &str, param: char, record: char) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut current = String::new();
    let mut chars = global.chars().peekable();
    while let Some(c) = chars.next() {
        if c == param || c == record {
            out.push(std::mem::take(&mut current));
            if c == record {
                break;
            }
            continue;
        }
        current.push(c);
        // A HOLLERITH STRING MAY CONTAIN THE DELIMITER, which is the whole
        // reason it carries its length in front: `6H,;,;,;` is one field.
        if c == 'H'
            && let Ok(n) = current[..current.len() - 1].trim().parse::<usize>()
        {
            for _ in 0..n {
                match chars.next() {
                    Some(c) => current.push(c),
                    None => break,
                }
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// A `nHtext` field as its text, and a plain field as itself.
fn hollerith(field: &str) -> String {
    let text = field.trim();
    match text.split_once('H') {
        Some((n, rest)) if n.trim().parse::<usize>().is_ok() => rest.to_owned(),
        _ => text.to_owned(),
    }
}

/// A parameter record's numbers, in order.
///
/// The first is the entity type, which every parameter record repeats.
fn numbers(text: &str, param: char, record: char) -> Vec<f64> {
    text.split([param, record])
        .map(str::trim)
        // IGES writes a double as `1.0D+02`, which no Rust parser knows.
        .map(|w| w.replace(['D', 'd'], "e"))
        .map(|w| w.parse::<f64>().unwrap_or_default())
        .collect()
}

/// Entity 110: two points.
fn line(n: &[f64]) -> Vec<[f64; 3]> {
    if n.len() < 7 {
        return Vec::new();
    }
    vec![[n[1], n[2], n[3]], [n[4], n[5], n[6]]]
}

/// Entity 100: a circular arc, in its own plane at `z`.
///
/// The parameters are the plane's height, the centre, and the start and end
/// points as `x y` pairs in that plane. It runs ANTICLOCKWISE from start to
/// end, always — the format has no flag, because the order of the two points
/// is the flag.
fn circular_arc(n: &[f64]) -> Vec<[f64; 3]> {
    if n.len() < 8 {
        return Vec::new();
    }
    let (z, cx, cy) = (n[1], n[2], n[3]);
    let start = (n[4] - cx).hypot(n[5] - cy);
    if start <= 0.0 {
        return Vec::new();
    }
    let a = (n[5] - cy).atan2(n[4] - cx);
    let b = (n[7] - cy).atan2(n[6] - cx);
    let full = std::f64::consts::TAU;
    let mut sweep = b - a;
    while sweep <= 1e-9 {
        sweep += full;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let steps =
        ((sweep / full * SEGMENTS_PER_TURN as f64).ceil() as usize).clamp(2, SEGMENTS_PER_TURN);
    (0..=steps)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let t = a + sweep * (i as f64 / steps as f64);
            [cx + start * t.cos(), cy + start * t.sin(), z]
        })
        .collect()
}

/// Entity 126: a rational B-spline curve.
///
/// `K M PROP1..4  knots[K+M+2]  weights[K+1]  points[3*(K+1)]  V0 V1 …`
fn spline(n: &[f64]) -> Vec<[f64; 3]> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (k, degree) = match (n.get(1), n.get(2)) {
        (Some(k), Some(m)) if *k >= 1.0 && *m >= 1.0 => (*k as usize, *m as usize),
        _ => return Vec::new(),
    };
    let count = k + 1;
    let knots_at = 7;
    let knots_len = count + degree + 1;
    let weights_at = knots_at + knots_len;
    let points_at = weights_at + count;
    if n.len() < points_at + count * 3 || degree >= count {
        return Vec::new();
    }
    let knots: Vec<f64> = n[knots_at..knots_at + knots_len].to_vec();
    let control: Vec<[f64; 3]> = (0..count)
        .map(|i| {
            let at = points_at + i * 3;
            [n[at], n[at + 1], n[at + 2]]
        })
        .collect();
    let from = knots[degree];
    let to = knots[count];
    if to <= from {
        return Vec::new();
    }
    (0..=SEGMENTS_PER_SPLINE)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let t = from + (to - from) * (i as f64 / SEGMENTS_PER_SPLINE as f64);
            super::step::de_boor_3(&control, &knots, degree, t.min(to - 1e-12))
        })
        .collect()
}

/// A point through entity 124's twelve numbers: three rows, then the shift.
fn through(m: &[f64; 12], v: [f64; 3]) -> [f64; 3] {
    [
        m[0] * v[0] + m[1] * v[1] + m[2] * v[2] + m[3],
        m[4] * v[0] + m[5] * v[1] + m[6] * v[2] + m[7],
        m[8] * v[0] + m[9] * v[1] + m[10] * v[2] + m[11],
    ]
}

/// How many millimetres one of the file's units is.
///
/// Field 14 is the flag and field 15 the name; the flag decides, except when
/// it says "see the name".
fn millimetres_per(fields: &[String]) -> (f64, Option<&'static str>) {
    let flag: i64 = fields
        .get(14)
        .and_then(|f| f.trim().parse().ok())
        .unwrap_or(2);
    let by_name = || {
        let name = fields.get(15).map(|f| hollerith(f)).unwrap_or_default();
        match name.trim().to_ascii_uppercase().as_str() {
            "MM" => (1.0, Some("millimeter")),
            "IN" | "INCH" => (25.4, Some("inch")),
            "M" => (1000.0, Some("meter")),
            _ => (1.0, None),
        }
    };
    match flag {
        1 => (25.4, Some("inch")),
        2 => (1.0, Some("millimeter")),
        4 => (304.8, Some("foot")),
        5 => (1_609_344.0, Some("mile")),
        6 => (1000.0, Some("meter")),
        7 => (1_000_000.0, Some("kilometer")),
        8 => (0.0254, Some("mil")),
        9 => (0.001, Some("micrometer")),
        10 => (10.0, Some("centimeter")),
        11 => (0.000_025_4, Some("microinch")),
        _ => by_name(),
    }
}

#[cfg(test)]
mod tests {
    use super::read;

    /// One record, padded to eighty columns with its section letter and
    /// sequence number in the right place.
    fn record(body: &str, section: char, sequence: usize) -> String {
        format!("{body:<72}{section}{sequence:>7}")
    }

    /// A file whose parameter section is the entities given, one per record.
    ///
    /// Each entity is `(type, parameters)`, and the directory entry is built
    /// from it — which is the part of the format a fixture has to get right
    /// for the reader to be exercised at all.
    fn file(global: &str, entities: &[(i64, String)]) -> Vec<u8> {
        let mut out = String::new();
        out.push_str(&record("a test", 'S', 1));
        for (i, line) in global.as_bytes().chunks(72).enumerate() {
            out.push_str(&record(std::str::from_utf8(line).unwrap(), 'G', i + 1));
        }
        let mut directory = String::new();
        let mut parameters = String::new();
        for (i, (kind, args)) in entities.iter().enumerate() {
            let sequence = i * 2 + 1;
            let at = i + 1;
            // Record one: type, parameter pointer, then seven fields of zero.
            let first = format!("{kind:>8}{at:>8}{}", "       0".repeat(7));
            directory.push_str(&record(&first, 'D', sequence));
            let second = format!("{kind:>8}{}", "       0".repeat(8));
            directory.push_str(&record(&second, 'D', sequence + 1));
            parameters.push_str(&format!(
                "{:<64}{sequence:>8}P{at:>7}",
                format!("{kind},{args};")
            ));
        }
        out.push_str(&directory);
        out.push_str(&parameters);
        out.push_str(&record("terminate", 'T', 1));
        out.into_bytes()
    }

    fn wires(bytes: &[u8]) -> crate::mesh::Wires {
        read(bytes).unwrap().body.wires
    }

    /// A Global section in millimetres, with the default delimiters written
    /// out the way most writers do.
    const GLOBAL: &str = "1H,,1H;,6HTHING1,8HTHING.IG,7HA SYSTEM,4HV1.0,32,38,6,308,15,\
6HTHING1,1.,2,2HMM,1,0.08,13H260831.120000,1.E-07,100.,6HNOBODY,3HORG,11,0,;";

    #[test]
    fn a_line_entity_is_the_two_points_it_names() {
        let w = wires(&file(GLOBAL, &[(110, "0.,0.,0.,10.,0.,0.".to_owned())]));
        assert_eq!(w.count(), 1);
        assert_eq!(w.bounds.size(), [10.0, 0.0, 0.0]);
    }

    #[test]
    fn an_arc_runs_anticlockwise_from_its_start_to_its_end() {
        // A quarter arc of radius 10 about the origin, from (10,0) to (0,10).
        let w = wires(&file(GLOBAL, &[(100, "0.,0.,0.,10.,0.,0.,10.".to_owned())]));
        assert!(w.count() >= 8, "an arc is more than a chord: {}", w.count());
        for p in w.segments.chunks_exact(3) {
            assert!(
                p[0] >= -0.01 && p[1] >= -0.01,
                "the arc left the quarter it was given: {p:?}"
            );
        }
    }

    #[test]
    fn the_delimiters_come_out_of_the_file_and_are_not_assumed() {
        // A Global section that declares a semicolon for parameters and a
        // full stop would split every number in half if the comma were
        // assumed. Here it declares the defaults explicitly, and a reader that
        // mis-parsed the declaration would lose the unit.
        assert_eq!(super::delimiters("1H,,1H;,rest"), (',', ';'));
        // A file that declares a semicolon separates its own fields WITH the
        // semicolon it just declared — which is the trap: the declaration is
        // written in the delimiters it is declaring.
        assert_eq!(super::delimiters("1H;;1H.;rest"), (';', '.'));
        assert_eq!(super::delimiters(",,rest"), (',', ';'));
    }

    #[test]
    fn a_hollerith_string_may_hold_the_delimiter() {
        // `6H,;,;,;` is one field, because its length says so.
        let fields = super::split("1H,,1H;,6H,;,;,;,1.,;", ',', ';');
        assert_eq!(super::hollerith(&fields[3]), ",;,;,;", "{fields:?}");
    }

    #[test]
    fn the_unit_flag_scales_the_geometry() {
        let inches = GLOBAL.replace("1.,2,2HMM", "1.,1,2HIN");
        let w = wires(&file(&inches, &[(110, "0.,0.,0.,1.,0.,0.".to_owned())]));
        assert!(
            (w.bounds.size()[0] - 25.4).abs() < 1e-3,
            "an inch file was not scaled: {:?}",
            w.bounds
        );
    }

    #[test]
    fn the_header_gives_the_program_that_wrote_it() {
        let out = read(&file(GLOBAL, &[(110, "0.,0.,0.,10.,0.,0.".to_owned())])).unwrap();
        assert!(
            out.notes
                .iter()
                .any(|(k, v)| k == "Application" && v == "A SYSTEM"),
            "{:?}",
            out.notes
        );
        assert_eq!(out.unit.as_deref(), Some("millimeter"));
    }

    #[test]
    fn an_entity_type_this_reader_does_not_know_is_skipped_and_not_guessed() {
        // A surface of revolution among the curves. The line still draws and
        // nothing is invented for the other one.
        let w = wires(&file(
            GLOBAL,
            &[
                (120, "1.,2.,3.,4.".to_owned()),
                (110, "0.,0.,0.,10.,0.,0.".to_owned()),
            ],
        ));
        assert_eq!(w.count(), 1);
    }

    #[test]
    fn a_file_with_no_curve_in_it_is_refused() {
        assert!(read(&file(GLOBAL, &[(120, "1.,2.".to_owned())])).is_err());
    }

    #[test]
    fn bytes_that_are_not_iges_are_refused() {
        assert!(read(b"ISO-10303-21;\n").is_err());
    }
}
