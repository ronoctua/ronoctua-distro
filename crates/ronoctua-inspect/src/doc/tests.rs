//! A Word 97 document, built in the test rather than committed.

use super::plain;
use crate::{Refusal, ole::fixture::vault};

/// A `WordDocument` stream: a FIB with the two fields this reader reads, then
/// the characters at `at`.
///
/// The FIB is 1 024 bytes of fields nothing here looks at, which is itself the
/// point — what a reader of this format needs is two numbers out of a header
/// that has two hundred.
fn word(which: bool, clx_at: usize, clx: &[u8], at: usize, text: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 0x0400];
    out[..2].copy_from_slice(&0xA5ECu16.to_le_bytes());
    let flags: u16 = if which { 0x0200 } else { 0 };
    out[0x0A..0x0C].copy_from_slice(&flags.to_le_bytes());
    out[0x01A2..0x01A6].copy_from_slice(&(u32::try_from(clx_at).unwrap()).to_le_bytes());
    out[0x01A6..0x01AA].copy_from_slice(&(u32::try_from(clx.len()).unwrap()).to_le_bytes());
    out.resize(at.max(out.len()), 0);
    out.resize(at, 0);
    out.extend_from_slice(text);
    // PAST THE CUTOFF, so this stream lands in the ordinary sectors — which is
    // where a real `WordDocument` is, being a megabyte of characters.
    out.resize(out.len().max(4097), 0);
    out
}

/// A piece table: one `Pcdt` holding `n` descriptors.
///
/// `pieces` is `(first character, last character, offset, one byte a
/// character)`.
fn table(pieces: &[(u32, u32, u32, bool)]) -> Vec<u8> {
    let mut plc: Vec<u8> = Vec::new();
    for (from, _, _, _) in pieces {
        plc.extend_from_slice(&from.to_le_bytes());
    }
    plc.extend_from_slice(&pieces.last().expect("a piece").1.to_le_bytes());
    for (_, _, fc, compressed) in pieces {
        plc.extend_from_slice(&0u16.to_le_bytes());
        let marked = if *compressed { fc | 0x4000_0000 } else { *fc };
        plc.extend_from_slice(&marked.to_le_bytes());
        plc.extend_from_slice(&0u16.to_le_bytes());
    }
    let mut out = vec![2u8];
    out.extend_from_slice(&(u32::try_from(plc.len()).unwrap()).to_le_bytes());
    out.extend_from_slice(&plc);
    out
}

/// FAILS IF: the text stops being taken through the piece table.
///
/// A reader that took `WordDocument` from `fcMin` to `fcMac` gets Word 6 right
/// and Word 97 wrong, which is the whole reason this module is more than ten
/// lines. Both spellings of a piece are here: one UTF-16 and one a byte a
/// character, which is what a real document mixes.
#[test]
fn a_word_document_is_read_through_its_piece_table() {
    let wide: Vec<u8> = "Lorem ipsum\r"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let narrow = b"dolor sit amet\r".to_vec();
    let at = 0x0800;
    let mut text = wide.clone();
    text.extend_from_slice(&narrow);
    let clx = table(&[
        (0, 12, u32::try_from(at).unwrap(), false),
        (12, 27, u32::try_from((at + wide.len()) * 2).unwrap(), true),
    ]);
    let stream = word(true, 0x40, &clx, at, &text);
    let mut sheet = vec![0u8; 0x40];
    sheet.extend_from_slice(&clx);
    let raw = vault(&[("WordDocument", &stream), ("1Table", &sheet)]);
    assert_eq!(
        plain(&raw),
        Ok(vec!["Lorem ipsum".to_owned(), "dolor sit amet".to_owned()])
    );
}

/// FAILS IF: the header's bit stops choosing which table stream is live.
///
/// A document carries BOTH: Word writes the new table beside the old and flips
/// this bit, so reading the wrong one gives a piece table for a version of the
/// file that no longer exists.
#[test]
fn the_header_says_which_of_the_two_table_streams_is_live() {
    let at = 0x0800;
    let text: Vec<u8> = "the live one\r"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let clx = table(&[(0, 13, u32::try_from(at).unwrap(), false)]);
    let mut sheet = vec![0u8; 0x40];
    sheet.extend_from_slice(&clx);
    // A DEAD TABLE IN THE OTHER STREAM, pointing somewhere there is nothing.
    let dead = {
        let mut out = vec![0u8; 0x40];
        out.extend_from_slice(&table(&[(0, 13, 0x00FF_0000, false)]));
        out
    };
    for which in [false, true] {
        let stream = word(which, 0x40, &clx, at, &text);
        let raw = vault(&[
            ("WordDocument", &stream),
            (if which { "1Table" } else { "0Table" }, &sheet),
            (if which { "0Table" } else { "1Table" }, &dead),
        ]);
        assert_eq!(
            plain(&raw),
            Ok(vec!["the live one".to_owned()]),
            "which={which}"
        );
    }
}

/// FAILS IF: the cp1252 characters of a compressed piece come back as Latin-1.
///
/// A curly quote is `0x92` in cp1252 and a control character in Latin-1, so a
/// reader that took the easy route turns every apostrophe Word wrote into a
/// codepoint that draws nothing — a word with a hole in it.
#[test]
fn a_compressed_piece_is_read_as_the_codepage_it_is() {
    let at = 0x0800;
    let text = b"it\x92s \x93quoted\x94\r".to_vec();
    let clx = table(&[(0, 14, u32::try_from(at * 2).unwrap(), true)]);
    let stream = word(true, 0x40, &clx, at, &text);
    let mut sheet = vec![0u8; 0x40];
    sheet.extend_from_slice(&clx);
    let raw = vault(&[("WordDocument", &stream), ("1Table", &sheet)]);
    assert_eq!(
        plain(&raw),
        Ok(vec!["it\u{2019}s \u{201C}quoted\u{201D}".to_owned()])
    );
}

/// FAILS IF: a field instruction reaches the panel as if it were prose.
///
/// `PAGEREF _Toc123 \h` and its like sit between `0x13` and `0x14`: they are
/// the document's machinery, the same judgement `rtf::plain` makes about
/// `\listtext`.
#[test]
fn a_field_instruction_is_not_the_document() {
    let at = 0x0800;
    let text: Vec<u8> = "see \u{0013}PAGEREF _Toc1 \\h\u{0014}page 3\u{0015}.\r"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let n = u32::try_from(text.len() / 2).unwrap();
    let clx = table(&[(0, n, u32::try_from(at).unwrap(), false)]);
    let stream = word(true, 0x40, &clx, at, &text);
    let mut sheet = vec![0u8; 0x40];
    sheet.extend_from_slice(&clx);
    let raw = vault(&[("WordDocument", &stream), ("1Table", &sheet)]);
    assert_eq!(plain(&raw), Ok(vec!["see page 3.".to_owned()]));
}

/// FAILS IF: a file that is not a Word document is read as one, or a piece
/// table that does not add up walks off the end of the stream.
#[test]
fn a_malformed_document_is_a_refusal_and_not_a_walk_off_the_end() {
    assert_eq!(plain(b"not a document"), Err(Refusal::Failed));
    let at = 0x0800;
    let text: Vec<u8> = "short\r"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    // A PIECE THAT CLAIMS MORE CHARACTERS THAN THE STREAM HOLDS.
    let clx = table(&[(0, 1_000_000, u32::try_from(at).unwrap(), false)]);
    let stream = word(true, 0x40, &clx, at, &text);
    let mut sheet = vec![0u8; 0x40];
    sheet.extend_from_slice(&clx);
    let raw = vault(&[("WordDocument", &stream), ("1Table", &sheet)]);
    assert_eq!(plain(&raw), Err(Refusal::Failed));
    // AND A DOCUMENT WITH NO TABLE STREAM AT ALL.
    let alone = vault(&[("WordDocument", &word(true, 0x40, &clx, at, &text))]);
    assert_eq!(plain(&alone), Err(Refusal::Failed));
}
