//! EBU STL (Tech 3264, `.stl`): broadcasters' binary subtitle exchange.
//!
//! A 1024-byte General Subtitle Information block (GSI), then one 128-byte
//! Text and Timing Information block (TTI) per subtitle, or per part of a
//! long one. ffmpeg's `stl` is Spruce's text format, not this one, so this
//! reader is ours.
//!
//! - **Characters.** The GSI's own fields are in the code page it names
//!   first (437 or 850; 860, 863 and 865 are read as far as they agree with
//!   437, and the rest becomes U+FFFD). The subtitles' text is in the table
//!   the GSI's `CCT` names: ISO 6937 (Latin, where an accent is a byte BEFORE
//!   its letter), or ISO 8859-5, -6, -7 or -8.
//! - **Codes.** `0x80`/`0x81` italics on and off, `0x82`/`0x83` underline,
//!   `0x8A` a new line, `0x8F` padding; the teletext colours `0x00`–`0x07`.
//! - **Time.** Hours, minutes, seconds and frames, at the rate the GSI names
//!   (`STL25.01`, `STL30.01`). Files made for broadcast often start at
//!   `10:00:00:00`, the programme's start (`TCP`); when every subtitle is at
//!   or after it, it is taken as zero.
//! - **Place.** A subtitle in the upper third of the rows (`VP`) asked for the
//!   top band.

use crate::subtitles::cue::{self, Band, Cue, Rgb, Span};
use crate::subtitles::{Damaged, FrameRate};

/// The General Subtitle Information block's size.
const GSI_BYTES: usize = 1024;
/// A Text and Timing Information block's size.
const TTI_BYTES: usize = 128;

/// Whether `bytes` begin like an EBU STL: a code page number in digits, then
/// `STL`.
#[must_use]
pub fn is_ebu(bytes: &[u8]) -> bool {
    bytes.len() >= GSI_BYTES && bytes[..3].iter().all(u8::is_ascii_digit) && &bytes[3..6] == b"STL"
}

/// What the GSI block says about the whole file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// The programme's title as the file gives it (`OPT`), trimmed.
    pub title: String,
    /// The rate its timecodes count frames at.
    pub rate: FrameRate,
}

/// The table the subtitles' text is in (`CCT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Table {
    Latin,
    Other(&'static encoding_rs::Encoding),
}

/// Reads an EBU STL's header and cues.
///
/// # Errors
///
/// [`Damaged`] when the bytes do not begin with a GSI block.
pub fn read(bytes: &[u8]) -> Result<(Header, Vec<Cue>), Damaged> {
    if !is_ebu(bytes) {
        return Err(Damaged);
    }
    let gsi = &bytes[..GSI_BYTES];
    let code_page = std::str::from_utf8(&gsi[..3])
        .ok()
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(850);
    let rate = match &gsi[3..11] {
        b"STL30.01" => FrameRate {
            numerator: 30,
            denominator: 1,
        },
        b"STL24.01" => FrameRate {
            numerator: 24,
            denominator: 1,
        },
        _ => FrameRate {
            numerator: 25,
            denominator: 1,
        },
    };
    let table = match &gsi[12..14] {
        b"01" => Table::Other(encoding_rs::ISO_8859_5),
        b"02" => Table::Other(encoding_rs::ISO_8859_6),
        b"03" => Table::Other(encoding_rs::ISO_8859_7),
        b"04" => Table::Other(encoding_rs::ISO_8859_8),
        _ => Table::Latin,
    };
    let title = gsi[16..48]
        .iter()
        .map(|&b| code_page_char(code_page, b))
        .collect::<String>()
        .trim()
        .to_owned();
    let rows = std::str::from_utf8(&gsi[251..253])
        .ok()
        .and_then(|r| r.trim().parse::<u8>().ok())
        .filter(|r| *r > 0)
        .unwrap_or(23);
    let programme_ms = timecode_text(&gsi[256..264], rate);

    let mut cues: Vec<Cue> = Vec::new();
    // The text of a subtitle split over several blocks, until the last.
    let mut open: Option<(u16, Vec<u8>)> = None;
    for block in bytes[GSI_BYTES..].chunks_exact(TTI_BYTES) {
        let extension = block[3];
        let comment = block[15] == 1;
        // User data extension blocks (`0xF0`–`0xFD`) hold no text.
        if comment || (0xF0..=0xFD).contains(&extension) {
            continue;
        }
        let number = u16::from_le_bytes([block[1], block[2]]);
        let text = &block[16..TTI_BYTES];
        let mut whole = match open.take() {
            Some((n, mut so_far)) if n == number => {
                so_far.extend_from_slice(text);
                so_far
            }
            _ => text.to_vec(),
        };
        if extension != 0xFF {
            open = Some((number, std::mem::take(&mut whole)));
            continue;
        }
        let start_ms = timecode(&block[5..9], rate);
        let end_ms = timecode(&block[9..13], rate);
        let row = block[13];
        let band = (row > 0 && u16::from(row) * 3 < u16::from(rows)).then_some(Band::Top);
        cues.push(Cue {
            start_ms,
            end_ms,
            lines: lines_of(&whole, table),
            band,
        });
    }
    // Timecodes counted from the programme's start.
    if let Some(programme) = programme_ms.filter(|&p| p > 0)
        && cues.iter().all(|c| c.start_ms >= programme)
    {
        for c in &mut cues {
            c.start_ms -= programme;
            c.end_ms = c.end_ms.saturating_sub(programme);
        }
    }
    Ok((Header { title, rate }, cues))
}

/// A TTI timecode: four bytes, hours, minutes, seconds, frames.
fn timecode(bytes: &[u8], rate: FrameRate) -> u64 {
    let seconds = u64::from(bytes[0]) * 3600 + u64::from(bytes[1]) * 60 + u64::from(bytes[2]);
    seconds * 1000 + cue::frames_ms(u64::from(bytes[3]), rate)
}

/// A GSI timecode: eight digits, `HHMMSSFF`.
fn timecode_text(bytes: &[u8], rate: FrameRate) -> Option<u64> {
    let digits = std::str::from_utf8(bytes).ok()?;
    if digits.len() != 8 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let part = |i: usize| digits[i..i + 2].parse::<u8>().ok();
    Some(timecode(&[part(0)?, part(2)?, part(4)?, part(6)?], rate))
}

/// The teletext colours of the alpha codes `0x00`–`0x07`; white is the
/// default, so it is none.
fn teletext(code: u8) -> Option<Rgb> {
    match code {
        0 => Some(Rgb::new(0, 0, 0)),
        1 => Some(Rgb::new(255, 0, 0)),
        2 => Some(Rgb::new(0, 255, 0)),
        3 => Some(Rgb::new(255, 255, 0)),
        4 => Some(Rgb::new(0, 0, 255)),
        5 => Some(Rgb::new(255, 0, 255)),
        6 => Some(Rgb::new(0, 255, 255)),
        _ => None,
    }
}

fn lines_of(text: &[u8], table: Table) -> Vec<Vec<Span>> {
    let mut lines: Vec<Vec<Span>> = vec![Vec::new()];
    let mut span = Span::default();
    let mut accent: Option<u8> = None;
    let push = |lines: &mut Vec<Vec<Span>>, span: &mut Span| {
        if !span.text.is_empty() {
            let next = Span {
                text: String::new(),
                ..span.clone()
            };
            if let Some(line) = lines.last_mut() {
                line.push(std::mem::replace(span, next));
            }
        }
    };
    for &byte in text {
        match byte {
            // Unused space: what fills a block after its text, and what a
            // subtitle split over blocks has between its parts.
            0x8F => {}
            0x8A => {
                push(&mut lines, &mut span);
                // Double height writes every line twice apart: one break.
                if lines.last().is_some_and(|l| !l.is_empty()) {
                    lines.push(Vec::new());
                }
            }
            0x80 | 0x81 => {
                push(&mut lines, &mut span);
                span.italic = byte == 0x80;
            }
            0x82 | 0x83 => {
                push(&mut lines, &mut span);
                span.underline = byte == 0x82;
            }
            0x00..=0x07 => {
                push(&mut lines, &mut span);
                span.colour = teletext(byte);
                // A colour code takes a cell, drawn as a space.
                span.text.push(' ');
            }
            0x08..=0x1F | 0x84..=0x89 | 0x8B..=0x8E | 0x90..=0x9F => {
                // Boxing, flashing, heights, mosaics: a space at most.
                if (0x08..=0x1F).contains(&byte) {
                    span.text.push(' ');
                }
            }
            _ => match table {
                Table::Latin => {
                    if (0xC1..=0xCF).contains(&byte) {
                        accent = Some(byte);
                        continue;
                    }
                    let base = iso6937(byte);
                    match accent.take() {
                        Some(mark) => span.text.push_str(&compose(mark, base)),
                        None => span.text.push(base),
                    }
                }
                Table::Other(encoding) => {
                    let one = [byte];
                    let (decoded, _) = encoding.decode_without_bom_handling(&one);
                    span.text.push_str(&decoded);
                }
            },
        }
    }
    push(&mut lines, &mut span);
    lines
}

/// A spacing character of ISO 6937 (`0x20`–`0x7F` and `0xA0`–`0xFF`, the
/// accents aside).
fn iso6937(byte: u8) -> char {
    const HIGH: [char; 96] = [
        '\u{a0}', '¡', '¢', '£', '$', '¥', '#', '§', '¤', '‘', '“', '«', '←', '↑', '→',
        '↓', //
        '°', '±', '²', '³', '×', 'µ', '¶', '·', '÷', '’', '”', '»', '¼', '½', '¾', '¿', //
        '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}',
        '\u{fffd}', //
        '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}',
        '\u{fffd}', //
        '―', '¹', '®', '©', '™', '♪', '¬', '¦', '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}',
        '⅛', '⅜', '⅝', '⅞', //
        'Ω', 'Æ', 'Đ', 'ª', 'Ħ', '\u{fffd}', 'Ĳ', 'Ŀ', 'Ł', 'Ø', 'Œ', 'º', 'Þ', 'Ŧ', 'Ŋ',
        'ŉ', //
        'ĸ', 'æ', 'đ', 'ð', 'ħ', 'ı', 'ĳ', 'ŀ', 'ł', 'ø', 'œ', 'ß', 'þ', 'ŧ', 'ŋ', '\u{ad}',
    ];
    match byte {
        0x20..=0x7E => char::from(byte),
        0xA0..=0xFF => HIGH[usize::from(byte - 0xA0)],
        _ => '\u{fffd}',
    }
}

/// A letter under an ISO 6937 accent: the precomposed letter when Unicode
/// has it, else the letter and the combining mark.
fn compose(accent: u8, base: char) -> String {
    let (bases, composed, combining): (&str, &str, char) = match accent {
        0xC1 => ("AEIOUaeiou", "ÀÈÌÒÙàèìòù", '\u{300}'),
        0xC2 => (
            "AEIOUYaeiouyCcLlNnRrSsZzGg",
            "ÁÉÍÓÚÝáéíóúýĆćĹĺŃńŔŕŚśŹźǴǵ",
            '\u{301}',
        ),
        0xC3 => (
            "AEIOUaeiouCcGgHhJjSsWwYy",
            "ÂÊÎÔÛâêîôûĈĉĜĝĤĥĴĵŜŝŴŵŶŷ",
            '\u{302}',
        ),
        0xC4 => ("ANOanoIiUu", "ÃÑÕãñõĨĩŨũ", '\u{303}'),
        0xC5 => ("AEIOUaeiou", "ĀĒĪŌŪāēīōū", '\u{304}'),
        0xC6 => ("AaGgUu", "ĂăĞğŬŭ", '\u{306}'),
        0xC7 => ("CcEeGgIZz", "ĊċĖėĠġİŻż", '\u{307}'),
        0xC8 => ("AEIOUYaeiouy", "ÄËÏÖÜŸäëïöüÿ", '\u{308}'),
        0xCA => ("AaUu", "ÅåŮů", '\u{30a}'),
        0xCB => ("CcGgKkLlNnRrSsTt", "ÇçĢģĶķĻļŅņŖŗŞşŢţ", '\u{327}'),
        0xCD => ("OoUu", "ŐőŰű", '\u{30b}'),
        0xCE => ("AaEeIiUu", "ĄąĘęĮįŲų", '\u{328}'),
        0xCF => ("CcDdEeLlNnRrSsTtZz", "ČčĎďĚěĽľŇňŘřŠšŤťŽž", '\u{30c}'),
        _ => return base.to_string(),
    };
    match bases.chars().position(|b| b == base) {
        Some(at) => composed
            .chars()
            .nth(at)
            .map_or_else(|| base.to_string(), |c| c.to_string()),
        None => format!("{base}{combining}"),
    }
}

/// A byte of a GSI text field in its code page.
fn code_page_char(code_page: u16, byte: u8) -> char {
    const CP437: [char; 128] = [
        'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
        'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', //
        'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', //
        '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', //
        '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', //
        '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', //
        'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', //
        '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
    ];
    const CP850: [char; 128] = [
        'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
        'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', 'ø', '£', 'Ø', '×', 'ƒ', //
        'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '®', '¬', '½', '¼', '¡', '«', '»', //
        '░', '▒', '▓', '│', '┤', 'Á', 'Â', 'À', '©', '╣', '║', '╗', '╝', '¢', '¥', '┐', //
        '└', '┴', '┬', '├', '─', '┼', 'ã', 'Ã', '╚', '╔', '╩', '╦', '╠', '═', '╬', '¤', //
        'ð', 'Ð', 'Ê', 'Ë', 'È', 'ı', 'Í', 'Î', 'Ï', '┘', '┌', '█', '▄', '¦', 'Ì', '▀', //
        'Ó', 'ß', 'Ô', 'Ò', 'õ', 'Õ', 'µ', 'þ', 'Þ', 'Ú', 'Û', 'Ù', 'ý', 'Ý', '¯', '´', //
        '\u{ad}', '±', '‗', '¾', '¶', '§', '÷', '¸', '°', '¨', '·', '¹', '³', '²', '■', '\u{a0}',
    ];
    match byte {
        0x00..=0x1F | 0x7F => ' ',
        0x20..=0x7E => char::from(byte),
        _ => {
            let at = usize::from(byte - 0x80);
            match code_page {
                850 => CP850[at],
                // 860, 863 and 865 differ from 437 only in some letters;
                // those read as 437's, the one table here.
                437 | 860 | 863 | 865 => CP437[at],
                _ => '\u{fffd}',
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GSI block: code page, disk format, table, title, rows, programme
    /// start.
    fn gsi(
        code_page: &[u8; 3],
        format: &[u8; 8],
        table: &[u8; 2],
        title: &[u8],
        start: &[u8; 8],
    ) -> Vec<u8> {
        let mut block = vec![b' '; GSI_BYTES];
        block[..3].copy_from_slice(code_page);
        block[3..11].copy_from_slice(format);
        block[11] = b'1';
        block[12..14].copy_from_slice(table);
        block[16..16 + title.len()].copy_from_slice(title);
        block[251..253].copy_from_slice(b"23");
        block[256..264].copy_from_slice(start);
        block
    }

    /// A TTI block.
    fn tti(
        number: u16,
        extension: u8,
        start: [u8; 4],
        end: [u8; 4],
        row: u8,
        text: &[u8],
    ) -> Vec<u8> {
        let mut block = vec![0x8F; TTI_BYTES];
        block[0] = 0;
        block[1..3].copy_from_slice(&number.to_le_bytes());
        block[3] = extension;
        block[4] = 0;
        block[5..9].copy_from_slice(&start);
        block[9..13].copy_from_slice(&end);
        block[13] = row;
        block[14] = 2;
        block[15] = 0;
        block[16..16 + text.len()].copy_from_slice(text);
        block
    }

    #[test]
    fn gsi_and_tti_blocks_are_read_with_their_codes() {
        let mut file = gsi(b"850", b"STL25.01", b"00", b"Cap\x82tulo", b"10000000");
        // `É` (accent + E), italics, a break, then red.
        file.extend(tti(
            1,
            0xFF,
            [10, 0, 1, 0],
            [10, 0, 2, 12],
            20,
            b"\xC2Eu \x80disse\x81\x8A\x01vermelho",
        ));
        // One subtitle over two blocks, on the top rows.
        file.extend(tti(2, 0x00, [10, 0, 3, 0], [10, 0, 4, 0], 2, b"Two "));
        file.extend(tti(
            2,
            0xFF,
            [10, 0, 3, 0],
            [10, 0, 4, 0],
            2,
            b"blocks \xC8u",
        ));
        // A comment is not shown.
        let mut comment = tti(3, 0xFF, [10, 0, 5, 0], [10, 0, 6, 0], 20, b"note");
        comment[15] = 1;
        file.extend(comment);
        let (header, cues) = read(&file).unwrap_or_else(|_| panic!("not read"));
        assert_eq!(header.title, "Capétulo", "code page 850");
        assert_eq!(
            header.rate,
            FrameRate {
                numerator: 25,
                denominator: 1
            }
        );
        assert_eq!(cues.len(), 2);
        assert_eq!(
            (cues[0].start_ms, cues[0].end_ms),
            (1000, 2480),
            "counted from 10:00:00:00"
        );
        assert_eq!(cues[0].lines[0][0].text, "Éu ");
        assert!(cues[0].lines[0][1].italic);
        assert_eq!(cues[0].lines[1][0].colour, Some(Rgb::new(255, 0, 0)));
        assert_eq!(cues[0].band, None);
        assert_eq!(cues[1].text(), "Two blocks ü");
        assert_eq!(cues[1].band, Some(Band::Top));
    }

    #[test]
    fn short_or_foreign_bytes_are_not_ebu() {
        assert_eq!(read(b"850STL25.01"), Err(Damaged));
        assert!(!is_ebu(&vec![b'x'; 2048]));
    }
}
