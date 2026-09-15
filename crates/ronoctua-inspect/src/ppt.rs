//! What a `.ppt` says, out of the records that hold its text.
//!
//! # The measurement this exists for
//!
//! `catppt` — the packaged reader, and the only one short of installing an
//! office suite — answered **zero bytes, exit status zero and nothing on
//! stderr** on the owner's own `sample.ppt`. The file is a real OLE compound
//! document and its text is in plain sight: *Lorem ipsum*, *Chart*, *Table*,
//! *Column 1*. So the preview refused a file that had something to show, and
//! the owner asked for the other thing — *"o preview também não foi capaz de
//! abrir o arquivo sample.ppt"*.
//!
//! # What a presentation is made of
//!
//! One stream of the compound file, `PowerPoint Document`, holding a tree of
//! records. Every record is eight bytes of header — a version-and-instance
//! `u16`, a type `u16`, a length `u32` — followed by its body. A record whose
//! version nibble is `0xF` is a CONTAINER and its body is more records; every
//! other one is an atom.
//!
//! Two atoms carry text, and the difference between them is the alphabet:
//! `TextCharsAtom` is UTF-16 and `TextBytesAtom` is one byte a character, each
//! byte being the LOW half of the UTF-16 unit it stands for. Both are read.
//!
//! # And only the SLIDES count
//!
//! The same stream holds the master's own placeholder prose — *Click to edit
//! the title text format*, *Second Outline Level*, and a `*` for every bullet
//! level — and a reader that took every text atom would put a template's
//! instructions on the panel as if the author had written them. Measured on
//! the owner's file: fourteen of the twenty-four text atoms are the master's.
//!
//! So text counts when it is inside a `Slide` container and nowhere else,
//! which is also what the `.pptx` reader next door does: `document::slides`
//! walks `ppt/slides/slideN.xml` and never the masters or the notes. One
//! presentation read two ways has to say the same thing.

use crate::{Refusal, ole};

/// The stream that holds the presentation.
const STREAM: &str = "PowerPoint Document";

/// A record's header, in bytes.
const HEADER: usize = 8;

/// The version nibble that means "this record contains other records".
const CONTAINER: u16 = 0xF;

/// A slide. Its subtree is the only place text on the panel comes from.
const SLIDE: u16 = 0x03EE;

/// Text as UTF-16.
const TEXT_CHARS: u16 = 0x0FA0;

/// Text as one byte a character, each the low half of a UTF-16 unit.
const TEXT_BYTES: u16 = 0x0FA8;

/// How deep the walk may go.
///
/// A malformed file can declare containers inside containers to any depth, and
/// a recursive walk with no bound is a stack overflow — which this project has
/// paid for twice, both times with no diagnostic beyond `process::abort`.
const DEPTH_CEILING: usize = 32;

/// The presentation's text, a line at a time.
///
/// # Errors
///
/// [`Refusal::Failed`] when the bytes are not a compound file or carry no
/// `PowerPoint Document` stream; [`Refusal::Empty`] when the slides say
/// nothing.
pub fn plain(raw: &[u8]) -> Result<Vec<String>, Refusal> {
    let stream = ole::stream(raw, STREAM)?;
    let mut said = String::new();
    walk(&stream, false, 0, &mut said);
    let out = lines_of(&said);
    if out.is_empty() {
        return Err(Refusal::Empty);
    }
    Ok(out)
}

/// Walks one run of records, gathering the text of the slides.
///
/// `on_a_slide` says whether this run is inside a slide's subtree. It is
/// carried down rather than looked up, because a text atom is four containers
/// below the slide it belongs to and none of the four says which slide that
/// is.
fn walk(body: &[u8], on_a_slide: bool, depth: usize, out: &mut String) {
    if depth > DEPTH_CEILING {
        return;
    }
    let mut at = 0usize;
    while at + HEADER <= body.len() {
        let version = u16::from_le_bytes([body[at], body[at + 1]]) & 0xF;
        let kind = u16::from_le_bytes([body[at + 2], body[at + 3]]);
        let length = u32::from_le_bytes([body[at + 4], body[at + 5], body[at + 6], body[at + 7]]);
        at += HEADER;
        // A LENGTH PAST THE END IS A FILE THAT LIES, and the rest of this run
        // cannot be found from here — the next header would be wherever this
        // record said it ended.
        let Ok(length) = usize::try_from(length) else {
            return;
        };
        let Some(inner) = body.get(at..at + length) else {
            return;
        };
        if version == CONTAINER {
            walk(inner, on_a_slide || kind == SLIDE, depth + 1, out);
        } else if on_a_slide {
            match kind {
                TEXT_CHARS => {
                    for c in char::decode_utf16(
                        inner
                            .chunks_exact(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]])),
                    ) {
                        out.push(c.unwrap_or(char::REPLACEMENT_CHARACTER));
                    }
                    out.push('\r');
                }
                TEXT_BYTES => {
                    // THE LOW HALF OF A UTF-16 UNIT, which is what this atom
                    // stores: byte `n` is `U+00nn`, not a codepage index.
                    out.extend(inner.iter().map(|&b| char::from(b)));
                    out.push('\r');
                }
                _ => {}
            }
        }
        at += length;
    }
}

/// The gathered text, broken where a presentation breaks it.
///
/// A paragraph ends at a carriage return and a line inside one ends at a
/// vertical tab — `U+000D` and `U+000B`, which is PowerPoint's own pair and
/// not a convention this reader chose. Everything else that is a control
/// character goes: a bullet's own `U+0013`, and the `U+000A` some writers put
/// beside the return.
fn lines_of(said: &str) -> Vec<String> {
    let mut out: Vec<String> = said
        .split(['\r', '\u{000B}'])
        .map(|line| {
            line.chars()
                .filter(|c| !c.is_control())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect();
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests;
