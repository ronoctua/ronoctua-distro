//! XML that comes from outside — a file on the disk, a radio's server, a
//! program's output — parsed only after its nesting is measured.
//!
//! # Why a crate for one function
//!
//! `roxmltree` recurses once per nested element and has no depth limit of its
//! own. A file of a hundred thousand `<a>` fits under any byte ceiling and
//! runs the reading thread out of stack: in a debug build 200 levels already
//! overflowed a 2 MiB test thread (100 did not). An overflow is not a panic a
//! thread boundary catches — it aborts the PROCESS: the player daemon for a
//! radio's playlist, the shell for a file preview.
//!
//! The guard was first written inside the player's subtitle readers. Three
//! crates read XML from outside (the playlists, the subtitles, Matroska's
//! tags), and a guard copied into each is a guard the next reader forgets, so
//! they all parse through [`parse`].
//!
//! # What it measures
//!
//! [`shallow`] counts the nesting in the TEXT, before anything is parsed:
//! tags are counted; comments, CDATA, processing instructions and quoted
//! attribute values are skipped.
//!
//! What the text cannot show is nesting an ENTITY brings. With a DTD allowed,
//! `<!ENTITY a "<x><x>…">` expands into elements wherever `&a;` is written,
//! and an entity can name another, ten deep — so a document that measures 48
//! could parse ten times deeper. A declaration whose quoted value holds markup
//! is therefore refused: every expansion that is left is text. (A character
//! reference such as `&#60;` in an entity's value comes out as TEXT in this
//! reader, never as markup — `a_character_reference_in_an_entity_is_text`
//! holds it to that.)

pub use roxmltree;

/// How deep a document may nest.
///
/// No playlist, subtitle file or tag list nests past a dozen; 200 levels
/// already overflowed a 2 MiB debug thread.
pub const DEPTH_MOST: usize = 48;

/// Why a text was not parsed.
///
/// Two shapes and no sentence: what a refusal MEANS is the caller's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// It nests deeper than [`DEPTH_MOST`], or it declares an entity whose
    /// value is markup.
    TooDeep,
    /// It is not well-formed XML, or it breaks one of the caller's
    /// [`roxmltree::ParsingOptions`] (a DTD where none is allowed, too many
    /// nodes).
    Malformed,
}

/// `text` parsed with `options`, once it is known to nest no deeper than
/// [`DEPTH_MOST`].
///
/// # Errors
///
/// [`Refusal::TooDeep`] before parsing; [`Refusal::Malformed`] from the
/// parser.
pub fn parse<'input>(
    text: &'input str,
    options: roxmltree::ParsingOptions<'input>,
) -> Result<roxmltree::Document<'input>, Refusal> {
    if !shallow(text, DEPTH_MOST) {
        return Err(Refusal::TooDeep);
    }
    roxmltree::Document::parse_with_options(text, options).map_err(|_| Refusal::Malformed)
}

/// Whether `text`'s elements nest no deeper than `most`, measured without
/// parsing it — and whether none of its declarations holds markup in a quoted
/// value, the one way an entity could nest where the text does not.
#[must_use]
pub fn shallow(text: &str, most: usize) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut at = 0;
    while let Some(open) = bytes
        .get(at..)
        .and_then(|b| b.iter().position(|&c| c == b'<'))
    {
        let tag = at + open;
        let rest = &bytes[tag..];
        at = if rest.starts_with(b"<!--") {
            past(bytes, tag, b"-->")
        } else if rest.starts_with(b"<![CDATA[") {
            past(bytes, tag, b"]]>")
        } else if rest.starts_with(b"<?") {
            past(bytes, tag, b"?>")
        } else if rest.starts_with(b"<!") {
            match declaration_end(bytes, tag) {
                Some(end) => end,
                None => return false,
            }
        } else if rest.starts_with(b"</") {
            depth = depth.saturating_sub(1);
            past(bytes, tag, b">")
        } else {
            let end = tag_end(bytes, tag);
            // `<a/>` opens nothing.
            if bytes.get(end - 1) != Some(&b'/') {
                depth += 1;
                if depth > most {
                    return false;
                }
            }
            end + 1
        };
    }
    true
}

/// Just past the first `end` at or after `from`, or the end of the text.
fn past(bytes: &[u8], from: usize, end: &[u8]) -> usize {
    bytes[from..]
        .windows(end.len())
        .position(|w| w == end)
        .map_or(bytes.len(), |p| from + p + end.len())
}

/// The `>` that closes the tag opened at `tag`, quoted values skipped; the
/// end of the text when there is none.
fn tag_end(bytes: &[u8], tag: usize) -> usize {
    let mut quote: Option<u8> = None;
    let mut end = tag + 1;
    while end < bytes.len() {
        match (quote, bytes[end]) {
            (None, q @ (b'"' | b'\'')) => quote = Some(q),
            (Some(q), b) if b == q => quote = None,
            (None, b'>') => break,
            _ => {}
        }
        end += 1;
    }
    end
}

/// Just past the `<!…>` declaration at `tag` — a DOCTYPE with its whole
/// internal subset — or `None` when a quoted value inside it holds `<`.
///
/// Comments and processing instructions inside the subset are skipped whole,
/// so an apostrophe in one opens no quote.
fn declaration_end(bytes: &[u8], tag: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    let mut brackets = 0usize;
    let mut at = tag + 2;
    while at < bytes.len() {
        let b = bytes[at];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            } else if b == b'<' {
                return None;
            }
            at += 1;
            continue;
        }
        let rest = &bytes[at..];
        if rest.starts_with(b"<!--") {
            at = past(bytes, at, b"-->");
            continue;
        }
        if rest.starts_with(b"<?") {
            at = past(bytes, at, b"?>");
            continue;
        }
        match b {
            b'"' | b'\'' => quote = Some(b),
            b'[' => brackets += 1,
            b']' => brackets = brackets.saturating_sub(1),
            b'>' if brackets == 0 => return Some(at + 1),
            _ => {}
        }
        at += 1;
    }
    Some(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dtd<'a>() -> roxmltree::ParsingOptions<'a> {
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..roxmltree::ParsingOptions::default()
        }
    }

    fn nested(depth: usize) -> String {
        let mut text = "<a>".repeat(depth);
        text.push('x');
        text.push_str(&"</a>".repeat(depth));
        text
    }

    #[test]
    fn a_document_at_the_ceiling_is_read_and_one_past_it_is_refused() {
        let at_the_ceiling = nested(DEPTH_MOST);
        let doc = parse(&at_the_ceiling, roxmltree::ParsingOptions::default())
            .expect("a document at the ceiling is read");
        assert_eq!(
            doc.descendants()
                .filter(roxmltree::Node::is_element)
                .count(),
            DEPTH_MOST
        );
        assert_eq!(
            parse(
                &nested(DEPTH_MOST + 1),
                roxmltree::ParsingOptions::default()
            )
            .err(),
            Some(Refusal::TooDeep)
        );
    }

    #[test]
    fn a_hundred_thousand_levels_are_refused_before_the_parser_sees_them() {
        // On the test's own small stack: the parser would abort the process.
        let deep = nested(100_000);
        assert_eq!(
            parse(&deep, roxmltree::ParsingOptions::default()).err(),
            Some(Refusal::TooDeep)
        );
        // Unclosed, the way a truncated or hostile file is.
        assert!(!shallow(&"<a>".repeat(100_000), DEPTH_MOST));
    }

    #[test]
    fn what_is_not_an_element_is_not_counted() {
        let mut text = String::from("<?xml version=\"1.0\"?><!-- <a><a><a> -->");
        text.push_str("<r t=\"a>a>a\" u='>'>");
        text.push_str("<![CDATA[<a><a><a>]]>");
        text.push_str("<?pi <a><a><a> ?>");
        text.push_str(&"<e/>".repeat(10));
        text.push_str("<e x='1' />");
        text.push_str("</r>");
        assert!(shallow(&text, 1));
        assert!(!shallow("<r><a>x</a></r>", 1));
        // A quoted value is skipped whole, whatever is in it.
        assert!(shallow("<r t=\"<a><a><a>\" u='<b>'/>", 0));
        parse(&text, roxmltree::ParsingOptions::default()).expect("well-formed");
    }

    #[test]
    fn closing_tags_give_the_depth_back() {
        let wide = "<r>".to_owned() + &"<a><b>x</b></a>".repeat(1000) + "</r>";
        assert!(shallow(&wide, 3));
        assert!(!shallow(&wide, 2));
    }

    #[test]
    fn an_entity_that_is_markup_is_refused_and_one_that_is_text_is_read() {
        // Ten entities, each nesting the next in 40 elements, measure 1 in the
        // text and would parse 400 deep.
        let mut bomb = String::from("<!DOCTYPE r [\n");
        for n in 0..10 {
            let inner = if n == 9 {
                "x".to_owned()
            } else {
                format!("&e{};", n + 1)
            };
            bomb.push_str(&format!(
                "<!ENTITY e{n} \"{}{inner}{}\">\n",
                "<a>".repeat(40),
                "</a>".repeat(40)
            ));
        }
        bomb.push_str("]>\n<r>&e0;</r>");
        assert!(!shallow(&bomb, DEPTH_MOST));
        assert_eq!(parse(&bomb, dtd()).err(), Some(Refusal::TooDeep));

        let text = "<!DOCTYPE r [\n<!-- it's a comment -->\n<!ENTITY nbsp \"&#160;\">\n<!ENTITY who 'the \"owner\"'>\n]>\n<r>a&nbsp;b &who;</r>";
        let doc = parse(text, dtd()).expect("an entity that is text is read");
        assert_eq!(doc.root_element().text(), Some("a\u{a0}b the \"owner\""));
    }

    #[test]
    fn a_character_reference_in_an_entity_is_text() {
        // The one other road from an entity to markup, and the reader does not
        // take it: `&#60;` in a value stays a character. If an upgrade of
        // `roxmltree` turned it into markup, the guard above would need a
        // second rule; this is where that would show.
        let text = "<!DOCTYPE r [<!ENTITY e \"&#60;a>&#60;a>x&#60;/a>&#60;/a>\">]><r>&e;</r>";
        assert!(shallow(text, 1));
        if let Ok(doc) = parse(text, dtd()) {
            assert_eq!(
                doc.descendants()
                    .filter(roxmltree::Node::is_element)
                    .count(),
                1,
                "a character reference became an element"
            );
        }
    }

    #[test]
    fn a_doctype_without_a_subset_and_a_broken_text_are_told_apart() {
        let mkv = "<?xml version=\"1.0\"?>\n<!DOCTYPE Tags SYSTEM \"matroskatags.dtd\">\n<Tags><Tag/></Tags>";
        parse(mkv, dtd()).expect("mkvextract's header");
        assert_eq!(
            parse(mkv, roxmltree::ParsingOptions::default()).err(),
            Some(Refusal::Malformed),
            "a DTD where the caller allows none"
        );
        assert_eq!(parse("<r><a></r>", dtd()).err(), Some(Refusal::Malformed));
    }
}
