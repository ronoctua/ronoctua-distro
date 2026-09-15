//! What a `.doc` says, out of the piece table that says where its text is.
//!
//! # The measurement this exists for, and it corrects an earlier one
//!
//! This module's work used to be `catdoc`'s, and a comment in three files said
//! the program answered **8 377 bytes** on the owner's own `sample.doc`. That
//! number was measured against the wrong binary — a `catdoc` fetched from a
//! different nixpkgs than the one this flake pins. Measured against the
//! binary the distro actually installs, on the same file:
//!
//! ```text
//! $ catdoc sample.doc ; echo $?
//! 69
//! ```
//!
//! Zero bytes, **exit 69** and nothing at all on stderr. So the preview said
//! *"what came back was not a picture — the file may be damaged"* about a
//! perfectly good Word document, and the owner reported it twice. Its sibling
//! `xls2csv` from the same package reads his `.xls` fine (364 057 bytes), so
//! the package stays for that; what leaves is the dependency of `.doc` on a
//! program that does not read one.
//!
//! # Where the text of a Word 97 document is
//!
//! Not in one place, and that is the whole difficulty. The `WordDocument`
//! stream holds the characters, but they are in PIECES, and the piece table
//! that says where each one starts lives in ANOTHER stream — `1Table` or
//! `0Table`, and which of the two is a BIT in the header. A reader that took
//! the stream from `fcMin` to `fcMac` gets Word 6 right and Word 97 wrong.
//!
//! So: the FIB says which table stream and where the `Clx` is inside it; the
//! `Clx` holds a `Pcdt`; the `Pcdt` holds a `PlcPcd`, which is a run of
//! character positions followed by one descriptor per piece. Each descriptor
//! carries an offset into `WordDocument` and a flag that says whether that
//! piece is UTF-16 or one byte a character.
//!
//! # What it deliberately does NOT do
//!
//! Layout, styles, tables, images, or the revision history. It answers "what
//! does this document SAY", which is what a preview panel asks — the same
//! contract the zip-of-XML readers next door keep.

use crate::{Refusal, ole};

/// The stream that holds the characters.
const TEXT: &str = "WordDocument";

/// Where the flags live in the FIB.
const FLAGS: usize = 0x0A;

/// The bit of those flags that says which table stream is the live one.
const WHICH_TABLE: u16 = 0x0200;

/// Where the FIB points at the piece table's container.
const FC_CLX: usize = 0x01A2;

/// One piece descriptor, in bytes.
const PCD: usize = 8;

/// The bit of a piece's offset that says its characters are one byte each.
const COMPRESSED: u32 = 0x4000_0000;

/// What is left of the offset once the flags are off.
const OFFSET: u32 = 0x3FFF_FFFF;

/// How many pieces one document may be read in.
///
/// A malformed table can declare any number, and every one of them is a slice
/// of the text stream. Sixty-five thousand is far past a real document — the
/// owner's is ONE piece — and it bounds the work rather than the file.
const PIECE_CEILING: usize = 65_536;

/// The thirty-two characters Windows-1252 puts where Latin-1 has controls.
///
/// A compressed piece is one byte a character and the bytes are cp1252, not
/// Latin-1: a curly quote is `0x92` there and a control character in Latin-1.
/// Reading them as Latin-1 turns every apostrophe Word wrote into a codepoint
/// that draws nothing, which on a panel is a word with a hole in it.
const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

/// The document's text, a line at a time.
///
/// # Errors
///
/// [`Refusal::Failed`] when the bytes are not a compound file, carry no
/// `WordDocument` stream, or hold a piece table that does not add up;
/// [`Refusal::Empty`] when the document says nothing.
pub fn plain(raw: &[u8]) -> Result<Vec<String>, Refusal> {
    let text = ole::stream(raw, TEXT)?;
    let flags = u16(&text, FLAGS)?;
    // THE HEADER SAYS WHICH OF TWO STREAMS IS THE LIVE ONE, and a document
    // carries both: Word writes the new table beside the old and flips this
    // bit. Reading the wrong one gives a piece table for a version of the file
    // that no longer exists.
    let table = ole::stream(
        raw,
        if flags & WHICH_TABLE == 0 {
            "0Table"
        } else {
            "1Table"
        },
    )?;
    let at = u32(&text, FC_CLX)? as usize;
    let length = u32(&text, FC_CLX + 4)? as usize;
    let clx = table
        .get(at..at.saturating_add(length))
        .ok_or(Refusal::Failed)?;
    let said = pieces(&text, pcdt(clx)?)?;
    let out = lines_of(&said);
    if out.is_empty() {
        return Err(Refusal::Empty);
    }
    Ok(out)
}

/// The piece table, out of the container the FIB points at.
///
/// The container is a run of parts: `0x01` is a run of properties this reader
/// has no use for, and `0x02` is the piece table. Walking past the first is
/// what finds the second; a reader that assumed the table came first would work
/// on documents with no properties and on no others.
fn pcdt(clx: &[u8]) -> Result<&[u8], Refusal> {
    let mut at = 0usize;
    while at < clx.len() {
        match clx[at] {
            1 => {
                let n = u16(clx, at + 1)? as usize;
                at = at.saturating_add(3).saturating_add(n);
            }
            2 => {
                let n = u32(clx, at + 1)? as usize;
                let from = at + 5;
                return clx.get(from..from.saturating_add(n)).ok_or(Refusal::Failed);
            }
            _ => return Err(Refusal::Failed),
        }
    }
    Err(Refusal::Failed)
}

/// The characters, piece by piece.
fn pieces(text: &[u8], pcdt: &[u8]) -> Result<String, Refusal> {
    // A RUN OF `n + 1` POSITIONS AND THEN `n` DESCRIPTORS, which is what makes
    // the count arithmetic rather than declared: `4(n+1) + 8n = len`.
    let n = pcdt.len().checked_sub(4).ok_or(Refusal::Failed)? / (4 + PCD);
    if n == 0 || n > PIECE_CEILING {
        return Err(Refusal::Failed);
    }
    let mut out = String::new();
    for k in 0..n {
        let from = u32(pcdt, k * 4)? as usize;
        let to = u32(pcdt, (k + 1) * 4)? as usize;
        let characters = to.checked_sub(from).ok_or(Refusal::Failed)?;
        let fc = u32(pcdt, 4 * (n + 1) + k * PCD + 2)?;
        let offset = (fc & OFFSET) as usize;
        if fc & COMPRESSED == 0 {
            let bytes = characters.saturating_mul(2);
            let piece = text
                .get(offset..offset.saturating_add(bytes))
                .ok_or(Refusal::Failed)?;
            out.extend(
                char::decode_utf16(
                    piece
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]])),
                )
                .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER)),
            );
        } else {
            // THE OFFSET IS IN HALVES when the piece is compressed, which is
            // the format saying "this used to be two bytes a character".
            let at = offset / 2;
            let piece = text
                .get(at..at.saturating_add(characters))
                .ok_or(Refusal::Failed)?;
            out.extend(piece.iter().map(|&b| match b {
                0x80..=0x9F => CP1252_HIGH[usize::from(b) - 0x80],
                _ => char::from(b),
            }));
        }
    }
    Ok(out)
}

/// The gathered text, broken where a document breaks it.
///
/// A paragraph ends at a carriage return; a line inside one ends at a vertical
/// tab, and a page at a form feed. What sits between `0x13` and `0x14` is a
/// FIELD INSTRUCTION — `PAGEREF _Toc123 \h` and its like — which is the
/// document's machinery and not its words, the same judgement `rtf::plain`
/// makes about `\listtext`.
fn lines_of(said: &str) -> Vec<String> {
    let mut kept = String::with_capacity(said.len());
    let mut in_a_field = false;
    for c in said.chars() {
        match c {
            '\u{0013}' => in_a_field = true,
            '\u{0014}' | '\u{0015}' => in_a_field = false,
            _ if in_a_field => {}
            '\r' | '\u{000B}' | '\u{000C}' | '\u{0007}' => kept.push('\n'),
            '\t' => kept.push('\t'),
            _ if c.is_control() => {}
            _ => kept.push(c),
        }
    }
    let mut out: Vec<String> = kept.split('\n').map(|l| l.trim_end().to_owned()).collect();
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out
}

/// A little-endian `u16` at `at`, or a refusal when the bytes are not there.
fn u16(b: &[u8], at: usize) -> Result<u16, Refusal> {
    let s = b.get(at..at + 2).ok_or(Refusal::Failed)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

/// A little-endian `u32` at `at`, or a refusal when the bytes are not there.
fn u32(b: &[u8], at: usize) -> Result<u32, Refusal> {
    let s = b.get(at..at + 4).ok_or(Refusal::Failed)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

#[cfg(test)]
mod tests;
