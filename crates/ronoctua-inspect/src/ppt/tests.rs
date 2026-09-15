//! A presentation, built in the test rather than committed.
//!
//! The reason `document/tests.rs` gives, and it is the same one: the
//! repository is PUBLIC (invariant 2) and nothing of the owner's goes into it
//! (invariant 13), so his own `sample.ppt` cannot be the fixture. A compound
//! file written here is one whose every byte this test chose, so a failure
//! names the reader and never the file.

use super::plain;
use crate::{Refusal, ole::fixture::vault};

/// A record: eight bytes of header, then the body.
///
/// `container` decides the version nibble, which is the whole of what tells a
/// container from an atom.
fn record(container: bool, kind: u16, body: &[u8]) -> Vec<u8> {
    let version: u16 = if container { 0xF } else { 0 };
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&(u32::try_from(body.len()).expect("a small fixture")).to_le_bytes());
    out.extend_from_slice(body);
    out
}

/// A `TextCharsAtom`: UTF-16, two bytes a unit.
fn chars(text: &str) -> Vec<u8> {
    let body: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    record(false, 0x0FA0, &body)
}

/// A `TextBytesAtom`: one byte a character, the low half of the UTF-16 unit.
fn bytes(text: &str) -> Vec<u8> {
    let body: Vec<u8> = text.encode_utf16().map(|u| (u & 0xFF) as u8).collect();
    record(false, 0x0FA8, &body)
}

/// The same bytes, padded past the mini-stream cutoff.
///
/// WHICH OF THE TWO TABLES A STREAM LANDS IN IS ITS SIZE, so this is how a
/// fixture asks for the ordinary sectors — which is where a real presentation's
/// stream is. The test below it asks for the other half by not padding.
fn big(body: &[u8]) -> Vec<u8> {
    let mut out = body.to_vec();
    out.resize(out.len().max(4097), 0);
    out
}

/// A presentation: one master with the template's prose, one slide with the
/// author's.
fn presentation() -> Vec<u8> {
    let mut master = chars("Click to edit the title text format");
    master.extend_from_slice(&bytes("Second Outline Level"));
    let mut slide = bytes("Lorem ipsum");
    slide.extend_from_slice(&chars(
        "A line\u{000B}and its second half\rAnd a new paragraph",
    ));
    let mut body = record(true, 0x03F8, &master);
    // THE SLIDE'S TEXT IS FOUR CONTAINERS DOWN, which is where a real file
    // puts it: the drawing, its group, the shape and the shape's text.
    let nested = record(true, 0xF003, &record(true, 0xF004, &slide));
    body.extend_from_slice(&record(true, 0x03EE, &nested));
    body
}

/// FAILS IF: the template's placeholder prose reaches the panel, or the
/// author's does not.
#[test]
fn a_presentation_says_what_its_slides_say() {
    let raw = vault(&[("PowerPoint Document", &big(&presentation()))]);
    assert_eq!(
        plain(&raw),
        Ok(vec![
            "Lorem ipsum".to_owned(),
            "A line".to_owned(),
            "and its second half".to_owned(),
            "And a new paragraph".to_owned(),
        ])
    );
}

/// FAILS IF: a stream under the cutoff stops being looked for in the mini
/// table.
///
/// It is the other half of the format and nothing about a `.ppt` says which
/// half it will use: the size decides, and a presentation of two words is
/// under four kilobytes.
#[test]
fn a_small_stream_is_read_out_of_the_mini_table() {
    let raw = vault(&[("PowerPoint Document", &presentation())]);
    assert_eq!(
        plain(&raw).map(|l| l.first().cloned()),
        Ok(Some("Lorem ipsum".to_owned()))
    );
}

/// FAILS IF: something that is not a compound file is read as one.
#[test]
fn a_file_that_is_not_a_compound_file_is_refused() {
    assert_eq!(plain(b"not a presentation"), Err(Refusal::Failed));
    assert_eq!(plain(&[]), Err(Refusal::Failed));
}

/// FAILS IF: a compound file with no presentation in it comes back as one.
#[test]
fn a_compound_file_without_the_stream_is_refused() {
    let raw = vault(&[("WordDocument", &big(&presentation()))]);
    assert_eq!(plain(&raw), Err(Refusal::Failed));
}

/// FAILS IF: a record that declares more body than the file holds is walked
/// past the end, or a tree of containers is followed without a bound.
#[test]
fn a_malformed_record_is_a_refusal_and_not_a_walk_off_the_end() {
    let mut lying = record(false, 0x0FA0, &[]);
    lying[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        plain(&vault(&[("PowerPoint Document", &big(&lying))])),
        Err(Refusal::Empty)
    );
    // A CONTAINER INSIDE ITSELF, as deep as the ceiling allows.
    let mut nest = bytes("deep");
    for _ in 0..64 {
        nest = record(true, 0x03EE, &nest);
    }
    assert_eq!(
        plain(&vault(&[("PowerPoint Document", &big(&nest))])),
        Err(Refusal::Empty)
    );
}
