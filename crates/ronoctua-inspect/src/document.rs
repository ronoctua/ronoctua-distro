//! What is inside a document, as lines a panel can show.
//!
//! # What it is for
//!
//! The owner's own folder has eleven documents and the preview opened one of
//! them: the PDF. He asked for the rest — *"há alguns arquivos temporários de
//! exemplo da pasta '~/main/temp', como alguns documentos, que ainda não
//! possuem suporte a preview, mas implementar o suporte para eles talvez não
//! seja tão difícil"* — and it is not, because most of them are the SAME file
//! format wearing different extensions.
//!
//! `.docx`, `.odt`, `.odp`, `.ods`, `.xlsx`, `.pptx` and `.epub` are every one
//! of them a ZIP OF XML, and this crate has had a zip reader and an XML parser
//! since the day it learnt to draw a 3MF. What was missing was the twenty
//! lines that say which member holds the words.
//!
//! # What comes back, and why it is the text module's type
//!
//! [`crate::text::Read`], which is what a `.rs` comes back as. That is the
//! whole of the integration: the panel already scrolls text, already pages it,
//! already moves it sideways, already hangs a bar on it. A `Document` type of
//! its own would have been a second thing for the panel to learn and a second
//! thing for both doors to draw.
//!
//! No syntax highlighting: `language` is `None`. A document has prose in it,
//! and prose is not coloured — the same answer `text::of` gives a `.txt`.
//!
//! # And the spreadsheet is a TABLE
//!
//! The owner's choice, in his own words: *"planilha vira tabela"*. Cells
//! padded into columns, which is what makes a sheet legible in a
//! monospaced panel — a comma-separated dump of the same numbers is the file
//! and not the sheet.
//!
//! # What it deliberately does NOT do
//!
//! Layout. A `.docx` is a page description and this draws none of it: no
//! fonts, no columns, no images, no tables-within-tables. It answers "what
//! does this document SAY", which is the question a preview panel asks, and
//! the file opens in a real editor one key away.

use std::path::Path;

use ronoctua_xml::roxmltree;

use crate::{Refusal, text, tools, zip};

/// How deep a document's XML may nest: the shared ceiling.
///
/// MEASURED on seventeen real files (ODF text and spreadsheets, Word and Excel
/// parts, an EPUB's XHTML): the deepest nests eighteen levels, an ODF with
/// lists inside lists, and every Word, Excel and EPUB part ten or fewer. A
/// list or a table nested ten deep more is still under 48.
const XML_DEPTH: usize = ronoctua_xml::DEPTH_MOST;

/// One part's XML, parsed once its nesting is measured.
///
/// `roxmltree` recurses once per level and has no limit of its own: a part
/// made for it ran the inspector's thread out of stack, which aborts the
/// footer rather than refusing a preview.
fn xml_of(text: &str) -> Result<roxmltree::Document<'_>, Refusal> {
    ronoctua_xml::parse_within(text, roxmltree::ParsingOptions::default(), XML_DEPTH)
        .map_err(|_| Refusal::Failed)
}

/// How many bytes of a document are read at all.
///
/// Sixty-four mebibytes, against the text reader's one: a `.docx` is a zip, so
/// the bytes on disk are not the bytes of text, and the owner's own
/// `sample.docx` is a megabyte of which the words are twelve kilobytes. What
/// bounds the TEXT is `zip::INFLATED_CEILING` on the way out of the inflate,
/// which is the only bound a zip bomb cannot lie about.
const BYTE_CEILING: u64 = 64 * 1024 * 1024;

/// How many bytes ONE part of a document may come to.
///
/// `zip::INFLATED_CEILING` is 256 MiB, and it is right for a mesh: a `.3mf`
/// really is that much XML and the reader walks it once, in a thread of its
/// own. A document is not that. This is read on the inspector's thread to fill
/// a panel of four lines, and `roxmltree` builds a tree over the whole part —
/// so a quarter of a gigabyte of XML would be something over a gigabyte of
/// allocation for a preview nobody asked to load.
///
/// Thirty-two mebibytes is far past anything a person writes: the owner's own
/// `sample.docx` is one megabyte on disk and twelve kilobytes of words. Past
/// it the panel says the file is too big, which is a sentence, rather than
/// taking the footer down, which is not.
const PART_CEILING: usize = 32 * 1024 * 1024;

/// How many rows of a spreadsheet are laid out.
///
/// The same two thousand lines the text reader keeps, and for the same reason:
/// past it a panel of four rows is scrolling through something nobody reads
/// this way.
const ROW_CEILING: usize = text::LINE_CEILING;

/// How many columns of a spreadsheet are laid out.
///
/// A sheet is `A`..`XFD` by the format — sixteen thousand — and a panel is
/// forty cells. This is what stops one row of a malformed file becoming a
/// megabyte of padding.
const COLUMN_CEILING: usize = 64;

/// The widest a single cell is drawn, in bytes.
///
/// `text::LINE_BYTES` is the whole LINE's ceiling; a cell has to be narrower
/// than that or one cell could fill a row on its own and the columns after it
/// would never be reached.
const CELL_BYTES: usize = 64;

/// Every type this module claims, in the order [`kind_of`] asks about them.
///
/// # Why a list as well as the function
///
/// Because the function answers by SUBCLASS — `is_a`, not `==` — so what it
/// claims is a set nobody can read off it. `document-formats-are-total`
/// compares the two in both directions: a type matched on and not declared is
/// a claim nobody wrote down, and a type declared and not matched on is a
/// promise the panel does not keep. It is `playlist::TYPES`' argument, one
/// format family along.
pub const TYPES: [&str; 13] = [
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.oasis.opendocument.presentation",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.oasis.opendocument.spreadsheet",
    "application/epub+zip",
    "application/rtf",
    "text/rtf",
    "application/msword",
    "application/vnd.ms-excel",
    "application/vnd.ms-powerpoint",
    // LAST, AND IT IS THE FALL-THROUGH. Every type above descends from it, so
    // a `kind_of` that asked this first would answer "an archive" for a Word
    // document — and the panel would list `[Content_Types].xml` where the
    // words should be.
    "application/zip",
];

/// What kind of document this is, once the type is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A word processor's file: `.docx`, `.odt`.
    Words,
    /// A presentation: `.pptx`, `.odp`.
    Slides,
    /// A spreadsheet: `.xlsx`, `.ods`.
    Sheet,
    /// An e-book: `.epub`.
    Book,
    /// A zip that is only a zip: its entries, with their sizes.
    Archive,
    /// Rich text: `.rtf`.
    Rich,
    /// A PowerPoint 97 presentation: `.ppt`. Read in process, out of the
    /// records of its `PowerPoint Document` stream.
    Show,
    /// A Word 97 document: `.doc`. Read in process, through the piece table
    /// that says where its text is.
    Word,
    /// The one OLE format that still needs a program: `.xls`.
    Legacy(tools::Tool),
}

/// Which kind a type is, or `None` when this module has no opinion.
///
/// # Why the OOXML types are named and `application/zip` is a fall-through
///
/// Because every one of them IS an `application/zip` by the freedesktop
/// database's reckoning, so asking `is_a(mime, "application/zip")` first would
/// answer "an archive" for a Word document. The specific types are matched
/// exactly and the generic one is what is left.
#[must_use]
pub fn kind_of(mime: &str) -> Option<Kind> {
    let is = |t: &str| ronoctua_files::mime::is_a(mime, t);
    // THE EXACT TYPES FIRST, most specific to least.
    if is("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        || is("application/vnd.oasis.opendocument.text")
    {
        return Some(Kind::Words);
    }
    if is("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        || is("application/vnd.oasis.opendocument.presentation")
    {
        return Some(Kind::Slides);
    }
    if is("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        || is("application/vnd.oasis.opendocument.spreadsheet")
    {
        return Some(Kind::Sheet);
    }
    if is("application/epub+zip") {
        return Some(Kind::Book);
    }
    // RICH TEXT IS TEXT BY THE DATABASE, and that is exactly the defect: an
    // `.rtf` fell through to the text reader and the panel showed
    // `{\rtf1\ansi\deff0{\fonttbl…` — the file, and not the document.
    if is("application/rtf") || is("text/rtf") {
        return Some(Kind::Rich);
    }
    // AND A WORD DOCUMENT IS READ HERE TOO, for the same measured reason.
    //
    // `catdoc` was its reader, and the packaged binary — the one this flake
    // pins, not the one an earlier comment was measured against — answers ZERO
    // BYTES and exit 69 on the owner's own `sample.doc`, with nothing on
    // stderr. He reported the panel's sentence twice. See `crate::doc`.
    if is("application/msword") {
        return Some(Kind::Word);
    }
    if is("application/vnd.ms-excel") {
        return Some(Kind::Legacy(tools::Tool::Xls2csv));
    }
    // AND A PRESENTATION IS READ HERE, not by a program.
    //
    // It was `catppt`'s, and the program answered ZERO BYTES with exit status
    // zero on the owner's own `sample.ppt` — a real compound document whose
    // text is in plain sight. The refusal that earned was honest and it was
    // still a panel with nothing on it, so the records are walked instead:
    // see `crate::ppt`, which is the measurement written down.
    if is("application/vnd.ms-powerpoint") {
        return Some(Kind::Show);
    }
    // AND A ZIP THAT IS ONLY A ZIP, last, for the reason above.
    is("application/zip").then_some(Kind::Archive)
}

/// What the document says, as lines.
///
/// `None` — not a document, and whoever asked should go on to its own answer.
/// `Some(Err)` — it IS one and here is why there is nothing to show.
/// `Some(Ok)` — the lines.
///
/// The shape is [`crate::text::of`]'s, deliberately: the two are asked one
/// after the other at the same call site, and an answer that had to be folded
/// two different ways would be a place for the folding to differ.
///
/// `tab` is how wide a tab stop is, and it travels for the reason it travels
/// into [`crate::text::of`]: it is `space.files.tabWidth`, a token, and this
/// crate receives no token values (invariant 9 is the host's to keep).
#[must_use]
pub fn of(path: &Path, mime: &str, tab: usize) -> Option<Result<text::Read, Refusal>> {
    let kind = kind_of(mime)?;
    Some(read_it(path, kind, tab))
}

fn read_it(path: &Path, kind: Kind, tab: usize) -> Result<text::Read, Refusal> {
    lines_of(said(path, kind)?, tab)
}

/// What the document says, as the lines of its text — before any of the
/// panel's cuts, de-tabbing or trimming. The panel's reader ([`of`]) and the
/// index's (`crate::plain`) are this one reading, so a document is found by
/// the words the preview shows.
pub(crate) fn said(path: &Path, kind: Kind) -> Result<Vec<String>, Refusal> {
    if let Kind::Legacy(tool) = kind {
        return from_a_program(path, tool);
    }
    let raw = read_capped(path)?;
    if let Kind::Rich = kind {
        return crate::rtf::plain(&raw);
    }
    if let Kind::Show = kind {
        return crate::ppt::plain(&raw);
    }
    if let Kind::Word = kind {
        return crate::doc::plain(&raw);
    }
    let list = zip::members(&raw)?;
    match kind {
        Kind::Words => words(&raw, &list),
        Kind::Slides => slides(&raw, &list),
        Kind::Sheet => sheet(&raw, &list),
        Kind::Book => book(&raw, &list),
        Kind::Archive => Ok(archive(&list)),
        Kind::Rich | Kind::Show | Kind::Word | Kind::Legacy(_) => {
            unreachable!("answered above")
        }
    }
}

/// The lines, as the text module's `Read`.
///
/// Every cut this makes is the text reader's own: the same ceiling on lines,
/// the same ceiling on ONE line, and the same `cut` flag that puts "there is
/// more" on the panel's last row rather than replacing the content with a
/// sentence about it.
fn lines_of(mut lines: Vec<String>, tab: usize) -> Result<text::Read, Refusal> {
    // A DOCUMENT OF NOTHING SAYS SO, rather than drawing a blank panel: an
    // empty panel and a refused one look the same on screen, and only one of
    // them has a sentence.
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return Err(Refusal::Empty);
    }
    let cut = lines.len() > text::LINE_CEILING;
    lines.truncate(text::LINE_CEILING);
    Ok(text::Read {
        lines: lines
            .into_iter()
            .map(|text| text::Line {
                // TRIMMED AT THE END, wherever the line came from. Trailing
                // whitespace is invisible on screen and never the document's:
                // `catdoc` puts a space before every line break, an XML puts
                // its own indentation there, and the aligned table pads.
                //
                // AND DE-TABBED BY THE TEXT READER'S OWN FUNCTION, which is
                // the whole of the fix for the line that ran over the column
                // beside it. A tab measures ONE cell and is DRAWN to the next
                // stop, so a document holding one was budgeted short and
                // painted long — photographed on `sample.rtf`, whose bullet
                // list is `\tab`-indented, crossing into the facts. A `.rs`
                // never had the defect because `text::read_it` has flattened
                // since the day it was written; this reader simply was not
                // asking the same question of its lines.
                text: cut_to(&text::flatten(text.trim_end(), tab), text::LINE_BYTES),
                runs: Vec::new(),
            })
            .collect(),
        cut,
        // NO LANGUAGE, so nothing is coloured. A document is prose, and the
        // syntax crate answers about source code.
        language: None,
    })
}

/// A string cut to at most `max` bytes, on a `char` boundary.
fn cut_to(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_owned();
    }
    let mut at = max;
    while at > 0 && !s.is_char_boundary(at) {
        at -= 1;
    }
    s[..at].to_owned()
}

/// The text of one XML member, a line per paragraph.
///
/// `breaks` are the elements that END a line — a paragraph, a heading, a row.
/// `skip` are the elements whose text is NOT the document's: Word's field
/// codes, which are `PAGE \* MERGEFORMAT` and belong to the layout this module
/// does not draw.
fn flow(xml: &str, breaks: &[&str], skip: &[&str]) -> Result<Vec<String>, Refusal> {
    let doc = xml_of(xml)?;
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for node in doc.descendants() {
        if node.is_element() {
            // A PARAGRAPH BEGINNING ENDS THE ONE BEFORE IT. Asked on the way
            // IN rather than on the way out because `descendants` is
            // pre-order, and the flush before the first one is a no-op.
            if breaks.contains(&node.tag_name().name()) && !line.trim().is_empty() {
                // TRIMMED AT THE END, because the whitespace BETWEEN two
                // elements is the XML's own: `<w:t>a</w:t>\n</w:p>` carries a
                // newline that is indentation and not a space in the document.
                out.push(line.trim_end().to_owned());
                line.clear();
            }
            continue;
        }
        let Some(said) = node.text() else { continue };
        if said.is_empty() {
            continue;
        }
        if node
            .ancestors()
            .any(|a| skip.contains(&a.tag_name().name()))
        {
            continue;
        }
        push_cell(&mut line, said);
        if out.len() > text::LINE_CEILING {
            break;
        }
    }
    if !line.trim().is_empty() {
        out.push(line.trim_end().to_owned());
    }
    Ok(out)
}

/// Adds a run of text to a line, bounded.
fn push_cell(line: &mut String, said: &str) {
    if line.len() >= text::LINE_BYTES {
        return;
    }
    // THE WHITESPACE OF A DOCUMENT IS NOT THE WHITESPACE OF A LINE. A `.docx`
    // carries newlines and indentation from its own XML, and letting them
    // through would draw the markup's shape rather than the document's.
    for part in said.split_whitespace() {
        if !line.is_empty() && !line.ends_with(' ') {
            line.push(' ');
        }
        line.push_str(part);
    }
    if said.ends_with(char::is_whitespace) && !line.is_empty() {
        line.push(' ');
    }
}

/// A word processor's document: OOXML's `word/document.xml`, or ODF's
/// `content.xml`.
fn words(raw: &[u8], list: &[zip::Member]) -> Result<Vec<String>, Refusal> {
    let xml = one_of(raw, list, &["word/document.xml", "content.xml"])?;
    // `w:p` and `text:p` are both `p` once the namespace is off; `text:h` is
    // ODF's heading, and `w:instrText` is Word's field code.
    flow(&xml, &["p", "h"], &["instrText"])
}

/// A presentation: every slide, in the order the file numbers them.
fn slides(raw: &[u8], list: &[zip::Member]) -> Result<Vec<String>, Refusal> {
    // ODF PUTS THE WHOLE PRESENTATION IN ONE PART, like its text documents.
    if let Some(m) = zip::part(list, "content.xml") {
        let xml = utf8(zip::member_bytes(raw, m)?)?;
        return flow(&xml, &["p", "h"], &[]);
    }
    // OOXML PUTS ONE PART PER SLIDE, and the ORDER is the number in the name:
    // `slide10.xml` sorts before `slide2.xml` as a string, and a presentation
    // read in that order is a presentation shuffled.
    let mut parts: Vec<(usize, &zip::Member)> = list
        .iter()
        .filter_map(|m| {
            let name = m.name();
            let rest = name.strip_prefix("ppt/slides/slide")?;
            let n = rest.strip_suffix(".xml")?.parse::<usize>().ok()?;
            Some((n, m))
        })
        .collect();
    if parts.is_empty() {
        return Err(Refusal::Nothing);
    }
    parts.sort_by_key(|(n, _)| *n);
    let mut out = Vec::new();
    for (_, m) in parts {
        let xml = utf8(zip::member_bytes(raw, m)?)?;
        out.extend(flow(&xml, &["p"], &[])?);
        if out.len() > text::LINE_CEILING {
            break;
        }
    }
    Ok(out)
}

/// An e-book: its chapters, in the order the spine gives.
fn book(raw: &[u8], list: &[zip::Member]) -> Result<Vec<String>, Refusal> {
    // THE SPINE IS THE READING ORDER, and the file names are not it: an EPUB
    // may call its chapters anything at all. `META-INF/container.xml` names the
    // package, the package's `manifest` maps ids to names, and its `spine`
    // gives the order.
    let container = utf8(zip::member_bytes(
        raw,
        zip::part(list, "META-INF/container.xml").ok_or(Refusal::Failed)?,
    )?)?;
    let doc = xml_of(&container)?;
    let opf = doc
        .descendants()
        .find(|n| n.tag_name().name() == "rootfile")
        .and_then(|n| n.attribute("full-path"))
        .ok_or(Refusal::Failed)?
        .to_owned();
    let root = opf.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
    let package = utf8(zip::member_bytes(
        raw,
        zip::part(list, &opf).ok_or(Refusal::Failed)?,
    )?)?;
    let doc = xml_of(&package)?;
    let by_id: Vec<(&str, &str)> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "item")
        .filter_map(|n| Some((n.attribute("id")?, n.attribute("href")?)))
        .collect();
    let order: Vec<&str> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "itemref")
        .filter_map(|n| n.attribute("idref"))
        .collect();
    let mut out = Vec::new();
    for id in order {
        let Some((_, href)) = by_id.iter().find(|(i, _)| *i == id) else {
            continue;
        };
        let name = if root.is_empty() {
            (*href).to_owned()
        } else {
            format!("{root}/{href}")
        };
        let Some(m) = zip::part(list, &name) else {
            continue;
        };
        let xml = utf8(zip::member_bytes(raw, m)?)?;
        // `head` carries the title and the stylesheet links, and neither is
        // the chapter.
        out.extend(flow(
            &xml,
            &["p", "h1", "h2", "h3", "div", "li"],
            &["head"],
        )?);
        if out.len() > text::LINE_CEILING {
            break;
        }
    }
    if out.is_empty() {
        return Err(Refusal::Empty);
    }
    Ok(out)
}

/// A zip that is only a zip: what is in it.
fn archive(list: &[zip::Member]) -> Vec<String> {
    // NO SENTENCE AND NO HEADING. This panel is the file's content, and the
    // content of an archive is its entries; a line saying "this is a zip"
    // would be the panel talking about itself.
    list.iter()
        .take(text::LINE_CEILING)
        .map(|m| format!("{:>12}  {}", m.plain(), m.name()))
        .collect()
}

/// A spreadsheet, laid out as a table.
fn sheet(raw: &[u8], list: &[zip::Member]) -> Result<Vec<String>, Refusal> {
    let grid = if zip::part(list, "content.xml").is_some() {
        odf_sheet(raw, list)?
    } else {
        ooxml_sheet(raw, list)?
    };
    if grid.is_empty() {
        return Err(Refusal::Empty);
    }
    Ok(align(&grid))
}

/// The cells of an ODF spreadsheet.
fn odf_sheet(raw: &[u8], list: &[zip::Member]) -> Result<Vec<Vec<String>>, Refusal> {
    let xml = utf8(zip::member_bytes(
        raw,
        zip::part(list, "content.xml").ok_or(Refusal::Failed)?,
    )?)?;
    let doc = xml_of(&xml)?;
    let mut grid = Vec::new();
    for row in doc
        .descendants()
        .filter(|n| n.tag_name().name() == "table-row")
    {
        let mut cells: Vec<String> = Vec::new();
        for cell in row.children().filter(|n| n.is_element()) {
            // AN EMPTY RUN IS DECLARED AND NOT WRITTEN OUT: ODF says "this
            // cell, N times", and a reader that ignored the count would put
            // every value in the wrong column.
            let times = cell
                .attributes()
                .find(|a| a.name() == "number-columns-repeated")
                .and_then(|a| a.value().parse::<usize>().ok())
                .unwrap_or(1)
                .min(COLUMN_CEILING);
            let said = cell
                .descendants()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .collect::<Vec<_>>()
                .join(" ");
            let said = cut_to(said.trim(), CELL_BYTES);
            for _ in 0..times {
                if cells.len() >= COLUMN_CEILING {
                    break;
                }
                cells.push(said.clone());
            }
        }
        while cells.last().is_some_and(String::is_empty) {
            cells.pop();
        }
        grid.push(cells);
        if grid.len() >= ROW_CEILING {
            break;
        }
    }
    while grid.last().is_some_and(Vec::is_empty) {
        grid.pop();
    }
    Ok(grid)
}

/// The cells of an OOXML spreadsheet.
fn ooxml_sheet(raw: &[u8], list: &[zip::Member]) -> Result<Vec<Vec<String>>, Refusal> {
    // THE STRINGS LIVE SOMEWHERE ELSE. An `.xlsx` keeps every repeated string
    // once, in `sharedStrings.xml`, and a cell of type `s` holds an INDEX into
    // it — read without this, a sheet of words is a sheet of small integers.
    let shared: Vec<String> = match zip::part(list, "xl/sharedStrings.xml") {
        Some(m) => {
            let xml = utf8(zip::member_bytes(raw, m)?)?;
            let doc = xml_of(&xml)?;
            doc.descendants()
                .filter(|n| n.tag_name().name() == "si")
                .map(|n| {
                    n.descendants()
                        .filter(|t| t.is_text())
                        .filter_map(|t| t.text())
                        .collect::<String>()
                })
                .collect()
        }
        None => Vec::new(),
    };
    let mut names: Vec<&zip::Member> = list
        .iter()
        .filter(|m| m.name().starts_with("xl/worksheets/") && m.name().ends_with(".xml"))
        .collect();
    names.sort_by_key(|m| m.name().to_owned());
    let first = names.first().ok_or(Refusal::Nothing)?;
    let xml = utf8(zip::member_bytes(raw, first)?)?;
    let doc = xml_of(&xml)?;
    let mut grid = Vec::new();
    for row in doc.descendants().filter(|n| n.tag_name().name() == "row") {
        let mut cells: Vec<String> = Vec::new();
        for cell in row.children().filter(|n| n.tag_name().name() == "c") {
            // THE COLUMN IS IN THE REFERENCE, and a row may skip columns
            // outright: `<c r="C1">` after `<c r="A1">` means B1 is empty, and
            // a reader that just pushed would slide every value one to the
            // left.
            if let Some(at) = cell.attribute("r").and_then(column_of) {
                while cells.len() < at.min(COLUMN_CEILING) {
                    cells.push(String::new());
                }
            }
            if cells.len() >= COLUMN_CEILING {
                break;
            }
            let said = ooxml_cell(cell, &shared);
            cells.push(cut_to(said.trim(), CELL_BYTES));
        }
        while cells.last().is_some_and(String::is_empty) {
            cells.pop();
        }
        grid.push(cells);
        if grid.len() >= ROW_CEILING {
            break;
        }
    }
    while grid.last().is_some_and(Vec::is_empty) {
        grid.pop();
    }
    Ok(grid)
}

/// One OOXML cell's text, through the shared table when it is an index.
fn ooxml_cell(cell: roxmltree::Node<'_, '_>, shared: &[String]) -> String {
    let inline = || {
        cell.descendants()
            .filter(|n| n.is_text())
            .filter_map(|n| n.text())
            .collect::<String>()
    };
    match cell.attribute("t") {
        Some("s") => cell
            .descendants()
            .find(|n| n.tag_name().name() == "v")
            .and_then(|n| n.text())
            .and_then(|t| t.parse::<usize>().ok())
            .and_then(|i| shared.get(i).cloned())
            .unwrap_or_default(),
        _ => inline(),
    }
}

/// Which column a cell reference names, counting from zero.
///
/// `A1` is 0, `Z9` is 25, `AA1` is 26. Letters only: the row number is not
/// this question, and a reference with none is not one.
fn column_of(reference: &str) -> Option<usize> {
    let mut n = 0_usize;
    let mut any = false;
    for c in reference.chars() {
        if !c.is_ascii_alphabetic() {
            break;
        }
        any = true;
        n = n
            .checked_mul(26)?
            .checked_add(usize::from(c.to_ascii_uppercase() as u8 - b'A' + 1))?;
    }
    any.then(|| n - 1)
}

/// The grid, padded into columns.
///
/// The owner's choice — *"planilha vira tabela"* — and the reason a table
/// rather than a dump: a monospaced panel is a grid already, and the whole of
/// what makes a sheet readable is that the same column starts at the same cell
/// on every row.
///
/// Measured in CELLS and not bytes, through `ronoctua_protocol::cells`: an
/// accented name and an ideograph each occupy something other than their byte
/// count, and a table padded by bytes is a table that leans.
fn align(grid: &[Vec<String>]) -> Vec<String> {
    let columns = grid.iter().map(Vec::len).max().unwrap_or(0);
    let mut wide = vec![0_usize; columns];
    for row in grid {
        for (n, cell) in row.iter().enumerate() {
            let w = usize::from(ronoctua_protocol::cells(cell));
            if w > wide[n] {
                wide[n] = w;
            }
        }
    }
    grid.iter()
        .map(|row| {
            let mut line = String::new();
            for (n, cell) in row.iter().enumerate() {
                if n > 0 {
                    line.push_str("  ");
                }
                line.push_str(cell);
                // THE LAST COLUMN IS NOT PADDED: trailing spaces are invisible
                // and would only push the line past the ceiling.
                if n + 1 < row.len() {
                    let pad = wide[n].saturating_sub(usize::from(ronoctua_protocol::cells(cell)));
                    for _ in 0..pad {
                        line.push(' ');
                    }
                }
            }
            line
        })
        .collect()
}

/// The first member of `wanted` this file has.
fn one_of(raw: &[u8], list: &[zip::Member], wanted: &[&str]) -> Result<String, Refusal> {
    for name in wanted {
        if let Some(m) = zip::part(list, name) {
            return utf8(zip::member_bytes(raw, m)?);
        }
    }
    Err(Refusal::Nothing)
}

/// A member's bytes as a string, bounded.
///
/// The bound is here and not at the inflate because the inflate's is the zip
/// reader's, and the zip reader serves meshes too — see [`PART_CEILING`].
fn utf8(bytes: Vec<u8>) -> Result<String, Refusal> {
    if bytes.len() > PART_CEILING {
        return Err(Refusal::TooBig(bytes.len() as u64));
    }
    String::from_utf8(bytes).map_err(|_| Refusal::Failed)
}

/// The file, up to [`BYTE_CEILING`].
///
/// `take` and not a `metadata` check afterwards, for the reason `text::of`
/// gives: the ceiling is the READ, so a sixty-five-mebibyte file never
/// allocates itself into the inspector's thread before anything gets to
/// object.
fn read_capped(path: &Path) -> Result<Vec<u8>, Refusal> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path).map_err(|_| Refusal::Unreadable)?;
    let mut raw = Vec::new();
    file.by_ref()
        .take(BYTE_CEILING)
        .read_to_end(&mut raw)
        .map_err(|_| Refusal::Unreadable)?;
    Ok(raw)
}

/// What a program says about one of the three OLE formats.
///
/// # What each of the three actually does, MEASURED
///
/// On the owner's own files, with the packaged binaries:
/// `catdoc sample.doc` answered **8 377 bytes**, `xls2csv sample.xls` answered
/// **364 057 bytes**, and `catppt sample.ppt` answered **zero bytes, exit
/// status zero, nothing on stderr** — on a file `file` calls a real OLE
/// compound document. Two of the three work; the third fails in the one way
/// nothing downstream can see.
///
/// So an empty answer is treated as a FAILURE and not as an empty document. A
/// blank panel and a refused one look the same on screen, and only one of them
/// has a sentence — which is this crate's rule and the whole reason `.ppt` is
/// shipped at all rather than left saying "no preview".
///
/// # And the spreadsheet becomes a TABLE, like the other two
///
/// `xls2csv` answers CSV, which is the file and not the sheet: a row of
/// `"Dulce","Abril","Female"` is harder to read than the `.xls` was. The
/// owner's choice — *"planilha vira tabela"* — is about spreadsheets and not
/// about which library opened one, so the cells go through the same [`align`]
/// the `.xlsx` and the `.ods` do.
fn from_a_program(path: &Path, tool: tools::Tool) -> Result<Vec<String>, Refusal> {
    let out = tools::text_of(tool, path)?;
    if out.trim().is_empty() {
        return Err(Refusal::Failed);
    }
    if matches!(tool, tools::Tool::Xls2csv) {
        let grid: Vec<Vec<String>> = out
            .lines()
            .take(ROW_CEILING)
            .map(|line| {
                let mut row = csv_row(line);
                row.truncate(COLUMN_CEILING);
                row
            })
            .collect();
        return Ok(align(&grid));
    }
    Ok(out.lines().map(str::to_owned).collect())
}

/// One line of CSV, as cells.
///
/// Enough of the format for what `xls2csv` writes: comma-separated, every
/// field that needs it quoted, and a quote inside a quoted field doubled.
/// Newlines inside a field are NOT handled — `xls2csv` writes one record per
/// line — and a reader that pretended otherwise would be claiming to
/// understand a file it has never been handed.
fn csv_row(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                let _ = chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(cut_to(cell.trim(), CELL_BYTES));
                cell.clear();
            }
            _ => cell.push(c),
        }
    }
    out.push(cut_to(cell.trim(), CELL_BYTES));
    out
}

#[cfg(test)]
mod tests;
