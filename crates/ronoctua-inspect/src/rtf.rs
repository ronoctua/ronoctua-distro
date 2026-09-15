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
            // A BRACE ENDS ANY DEBT THE `\uc` COUNTER IS STILL CARRYING.
            //
            // The approximation a `\uN` asks to be skipped is written BESIDE
            // it, inside the same group; a `\u` at the end of one with no
            // fallback after it must not take the first character of whatever
            // comes next. That is the shape the defect above had, and clearing
            // it here means a file that simply omits the fallback cannot
            // produce it again by another route.
            '{' => {
                pending = 0;
                depth += 1;
                if depth > DEPTH_CEILING {
                    return Err(Refusal::Failed);
                }
            }
            '}' => {
                pending = 0;
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
                            // AND IT IS ONE OF THE CHARACTERS `\uc` IS
                            // COUNTING, which this arm did not know.
                            //
                            // THE DEFECT, read off the owner's screen: every
                            // bullet of `sample.rtf`'s list had its first
                            // letter missing — *aecenas non lorem*, *ulla
                            // facilisi*, *enean congue*. LibreOffice writes the
                            // bullet as `\u61623\'3f`, so `\u` asked for one
                            // character to be skipped and the `\'3f` that WAS
                            // that character went past without paying. The
                            // debt stayed on the counter and was settled by
                            // the next ordinary letter in the file, which is
                            // the `M` of the paragraph after the group.
                            pending = pending.saturating_sub(1);
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
                    // AND `listtext` IS MACHINERY TOO, which is a decision and
                    // not an omission.
                    //
                    // It holds the marker a list item is drawn WITH — the
                    // bullet or the number — and a writer generates it rather
                    // than a person typing it. Two things say it does not
                    // belong on this panel. It is written as a codepoint of
                    // the Symbol font's PRIVATE USE AREA (`U+F0B7` here), and
                    // the private use area is where OUR font keeps its icons
                    // (invariant 10): the owner's screen drew a Nerd Fonts
                    // glyph in front of every bullet, because that is what
                    // `U+F0B7` is in the face this system loads. And the other
                    // document readers already drop it — `words()` walks
                    // `w:t`, and a `.docx` list comes out as its sentences —
                    // so keeping it here would make the same list read two
                    // ways depending on which spelling of the file one has.
                    "fonttbl" | "colortbl" | "stylesheet" | "listtable" | "listtext" | "info"
                    | "pict" | "header" | "footer"
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

    /// FAILS IF: a `\'hh` fallback stops paying the `\uc` counter, or a brace
    /// stops clearing it.
    ///
    /// THE DEFECT THIS EXISTS FOR, read off the owner's screen: every bullet
    /// of his `sample.rtf` was missing its first letter — *aecenas non
    /// lorem*, *ulla facilisi*, *enean congue*. LibreOffice writes the marker
    /// as `\u61623\'3f`, and the `\'3f` went past without settling the debt
    /// the `\u` had opened; the next ordinary letter in the file paid it, and
    /// that letter was in the paragraph AFTER the group.
    #[test]
    fn the_skip_a_unicode_escape_asks_for_is_paid_by_the_fallback_beside_it() {
        // NO BRACE BETWEEN THE ESCAPE AND THE TEXT, so the counter is the
        // only thing that can be paying: without the fix the `\'3f` goes past
        // for free and the space after it settles the debt instead.
        let raw = br"{\rtf1\ansi\uc1 caf\u233\'3f fim\par}";
        assert_eq!(plain(raw).expect("it reads"), vec!["café fim"]);
        // AND WITH NO FALLBACK WRITTEN AT ALL, the brace is what ends it —
        // which is the shape the owner's file has, the escape sitting at the
        // end of the marker's own group.
        let raw = br"{\rtf1\ansi\uc1{\b caf\u233}Maecenas\par}";
        assert_eq!(plain(raw).expect("it reads"), vec!["caféMaecenas"]);
    }

    /// FAILS IF: the generated list marker comes back to the panel.
    ///
    /// It is a Symbol-font codepoint in the PRIVATE USE AREA, which is where
    /// our own font keeps its icons — so the owner's screen drew a Nerd Fonts
    /// glyph in front of every item — and the other document readers drop it,
    /// so keeping it would make one list read two ways.
    #[test]
    fn the_generated_list_marker_is_not_the_document() {
        let raw = br"{\rtf1\ansi{\listtext \u61623\'3f\tab}Nulla facilisi.\par}";
        assert_eq!(plain(raw).expect("it reads"), vec!["Nulla facilisi."]);
    }

    #[test]
    fn a_document_of_nothing_but_markup_says_so() {
        assert_eq!(
            plain(br"{\rtf1\ansi{\fonttbl{\f0 Calibri;}}}"),
            Err(Refusal::Empty)
        );
    }
}
