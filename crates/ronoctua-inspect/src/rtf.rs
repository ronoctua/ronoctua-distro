//! What a `.rtf` says, with the control words taken off.
//!
//! # The defect, from the screen
//!
//! `text/rtf` descends from `text/plain` in the freedesktop database, and it
//! genuinely is: the file is ASCII. So the preview read it as text and drew
//! `{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fnil\fcharset0 Calibri;}}` —
//! the file, and not the document. It is the playlist's lesson again, written
//! in that module's own words: *what decides is not what a file is made of but
//! what it is FOR*.
//!
//! # Why it is written here and not taken from a crate
//!
//! Because RTF's syntax is four things — a group, a control word, a control
//! symbol and text — and the whole of what a preview needs is to drop the
//! first three. A crate that did this properly would bring the layout with it,
//! and this module draws no layout for any other format either.
//!
//! What it does NOT do is rewrite the document: no fonts, no tables, no
//! images. `\par` ends a line, `\tab` is a tab, the `\uN` escapes become the
//! character they name, and everything else that is a control word goes.

use crate::Refusal;

/// How many characters of a `\*\...` group are skipped before giving up.
///
/// A destination group whose brace never closes is a malformed file, and this
/// is what stops one turning the walk into a scan of the whole document for
/// every group it opens. The walk is linear either way; this bounds the
/// NESTING, which a file can otherwise declare to any depth.
const DEPTH_CEILING: usize = 64;

/// The document's text, a `String` with newlines in it.
///
/// # Errors
///
/// [`Refusal::Failed`] when the bytes do not begin `{\rtf`, which is the one
/// thing every RTF file has; [`Refusal::Empty`] when there is nothing but
/// markup in it.
pub fn plain(raw: &[u8]) -> Result<Vec<String>, Refusal> {
    // LOSSY, because an RTF is bytes in whatever codepage its header names and
    // this reader does not translate them: a stray high byte should not lose
    // the document. The `\uN` escapes — which is how anything outside the
    // codepage is actually written — are read exactly.
    let text = String::from_utf8_lossy(raw);
    if !text.trim_start().starts_with("{\\rtf") {
        return Err(Refusal::Failed);
    }
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    // HOW DEEP INSIDE A GROUP THIS READER IS NOT INTERESTED IN. A destination
    // group (`{\*\generator …}`, `{\fonttbl …}`, `{\stylesheet …}`) carries
    // the document's machinery and not its words, so its whole subtree is
    // skipped — and the depth is what says where it ends.
    let mut skip_from: Option<usize> = None;
    let mut depth = 0_usize;
    // HOW MANY CHARACTERS A `\uN` ESCAPE ASKS TO BE IGNORED AFTER IT. RTF
    // writes a Unicode character as the escape FOLLOWED by a codepage
    // approximation for readers that cannot do better, and `\ucN` says how
    // many of those to drop. Without this every accented letter comes out
    // twice.
    let mut skip_after = 1_usize;
    let mut pending = 0_usize;

    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                depth += 1;
                if depth > DEPTH_CEILING {
                    return Err(Refusal::Failed);
                }
            }
            '}' => {
                if skip_from == Some(depth) {
                    skip_from = None;
                }
                depth = depth.saturating_sub(1);
            }
            '\\' => {
                let Some(next) = chars.next() else { break };
                if !next.is_ascii_alphabetic() {
                    // A CONTROL SYMBOL. `\\`, `\{` and `\}` are the three that
                    // are text; `\*` opens a destination nobody has to
                    // understand, which is the whole point of it.
                    match next {
                        '\\' | '{' | '}' if skip_from.is_none() => line.push(next),
                        // THE OUTERMOST SKIP WINS, and this was a real defect
                        // measured against a LibreOffice file: the font table
                        // has `{\*\falt Times New Roman}` inside it, and a
                        // `skip_from` that took the inner group's depth was
                        // cleared by the inner group's brace — so the rest of
                        // the font table came out as the document's first line,
                        // `;Liberation Sans;Symbol;OpenSymbol;…`.
                        '*' if skip_from.is_none() => skip_from = Some(depth),
                        '\'' => {
                            // A BYTE IN THE DOCUMENT'S OWN CODEPAGE, as two
                            // hex digits. Not translated — this reader does not
                            // carry the codepage tables — so it is dropped,
                            // which is what `\uN` is there to make rare.
                            let _ = chars.next();
                            let _ = chars.next();
                        }
                        _ => {}
                    }
                    continue;
                }
                let mut word = String::from(next);
                while chars.peek().is_some_and(char::is_ascii_alphabetic) {
                    word.push(chars.next().unwrap_or_default());
                }
                let mut number = String::new();
                if chars.peek() == Some(&'-') {
                    number.push(chars.next().unwrap_or_default());
                }
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    number.push(chars.next().unwrap_or_default());
                }
                // A SINGLE SPACE AFTER A CONTROL WORD IS ITS TERMINATOR and
                // not a space in the document.
                if chars.peek() == Some(&' ') {
                    let _ = chars.next();
                }
                if skip_from.is_some() {
                    continue;
                }
                match word.as_str() {
                    "par" | "line" | "sect" | "page" => {
                        // TRIMMED AT THE END: an RTF puts a space before its
                        // own `\par`, and a trailing space is invisible on
                        // screen and a difference in a test.
                        out.push(line.trim_end().to_owned());
                        line.clear();
                    }
                    "tab" => line.push('\t'),
                    "uc" => skip_after = number.parse().unwrap_or(1),
                    "u" => {
                        if let Some(c) = number
                            .parse::<i32>()
                            .ok()
                            // NEGATIVE MEANS ABOVE 32767, written as a signed
                            // 16-bit number: the format is older than
                            // `u32`-sized escapes.
                            .map(|n| if n < 0 { n + 65_536 } else { n })
                            .and_then(|n| u32::try_from(n).ok())
                            .and_then(char::from_u32)
                        {
                            line.push(c);
                        }
                        pending = skip_after;
                    }
                    // EVERY OTHER CONTROL WORD IS MACHINERY: fonts, colours,
                    // margins, styles. The document is what is left.
                    "fonttbl" | "colortbl" | "stylesheet" | "listtable" | "info" | "pict"
                    | "header" | "footer"
                        if skip_from.is_none() =>
                    {
                        skip_from = Some(depth);
                    }
                    _ => {}
                }
            }
            '\r' | '\n' => {}
            _ => {
                if skip_from.is_some() {
                    continue;
                }
                if pending > 0 {
                    pending -= 1;
                    continue;
                }
                line.push(c);
            }
        }
    }
    if !line.trim().is_empty() {
        out.push(line.trim_end().to_owned());
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    if out.is_empty() {
        return Err(Refusal::Empty);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::plain;
    use crate::Refusal;

    #[test]
    fn a_file_that_is_not_rtf_is_refused_rather_than_drawn() {
        assert_eq!(plain(b"hello"), Err(Refusal::Failed));
    }

    /// THE DEFECT THIS EXISTS FOR: the machinery reaching the screen.
    #[test]
    fn the_control_words_do_not_reach_the_screen() {
        let raw = br"{\rtf1\ansi{\fonttbl{\f0\froman Liberation Serif{\*\falt Times New Roman};}{\f1 Symbol;}}\pard The document\par}";
        assert_eq!(plain(raw).expect("it reads"), vec!["The document"]);
    }

    /// AND AN ESCAPED CHARACTER IS THE CHARACTER, once.
    ///
    /// `\uN` is followed by an approximation for readers that cannot do
    /// better, and `\ucN` says how many characters of it to drop. Without that
    /// every accented letter comes out twice — once right and once wrong.
    #[test]
    fn a_unicode_escape_is_read_once_and_not_twice() {
        let raw = br"{\rtf1\ansi\uc1 caf\u233?  fim\par}";
        let out = plain(raw).expect("it reads");
        assert_eq!(out, vec!["café  fim"]);
    }

    #[test]
    fn a_document_of_nothing_but_markup_says_so() {
        assert_eq!(
            plain(br"{\rtf1\ansi{\fonttbl{\f0 Calibri;}}}"),
            Err(Refusal::Empty)
        );
    }
}
