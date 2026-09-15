//! The documents, built in the test rather than committed.
//!
//! # Why the fixtures are written here and not checked in
//!
//! Two reasons, and the first is the repository's: it is PUBLIC (invariant 2)
//! and nothing of the owner's goes into it (invariant 13), so his own
//! `sample.docx` cannot be the fixture. The second is better: a zip written
//! here is a zip whose every byte this test chose, so a failure names the
//! reader and never the file.
//!
//! Every archive below is STORED (method 0) — no deflate. What is under test
//! is which member holds the words and what the XML in it means; the inflate
//! has its own tests next door, in `crate::zip`.

use super::{Kind, kind_of, of};
use crate::Refusal;

/// A zip of `(name, bytes)`, stored.
///
/// Thirty lines of little-endian, and the format is the one `crate::zip`
/// reads: local headers, then the central directory, then the end record.
fn zipped(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut directory: Vec<u8> = Vec::new();
    for (name, body) in parts {
        let at = u32::try_from(out.len()).expect("a small fixture");
        let size = u32::try_from(body.len()).expect("a small member");
        let crc = crc32(body);
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(
            &u16::try_from(name.len())
                .expect("a short name")
                .to_le_bytes(),
        );
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);

        directory.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        directory.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        directory.extend_from_slice(&crc.to_le_bytes());
        directory.extend_from_slice(&size.to_le_bytes());
        directory.extend_from_slice(&size.to_le_bytes());
        directory.extend_from_slice(
            &u16::try_from(name.len())
                .expect("a short name")
                .to_le_bytes(),
        );
        directory.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        directory.extend_from_slice(&at.to_le_bytes());
        directory.extend_from_slice(name.as_bytes());
    }
    let at = u32::try_from(out.len()).expect("a small fixture");
    let n = u16::try_from(parts.len()).expect("a few members");
    let size = u32::try_from(directory.len()).expect("a small directory");
    out.extend_from_slice(&directory);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&at.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out
}

/// The checksum the format carries. Nothing this crate reads checks it; it is
/// here so the fixture is a real zip rather than one only our reader accepts.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for b in bytes {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            let take = crc & 1 == 1;
            crc >>= 1;
            if take {
                crc ^= 0xedb8_8320;
            }
        }
    }
    !crc
}

/// Writes the bytes to a file and asks the reader about them.
fn read(name: &str, mime: &str, bytes: &[u8]) -> Result<Vec<String>, Refusal> {
    let dir = std::env::temp_dir().join(format!("ronoctua-doc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the test directory");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write the fixture");
    let answer = of(&path, mime).expect("this type is a document");
    let _ = std::fs::remove_file(&path);
    answer.map(|read| read.lines.into_iter().map(|l| l.text).collect())
}

const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const PPTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
const ODT: &str = "application/vnd.oasis.opendocument.text";
const ODS: &str = "application/vnd.oasis.opendocument.spreadsheet";

/// THE ORDER OF THE TYPES IS THE WHOLE OF `kind_of`.
///
/// Every one of these IS an `application/zip` by the freedesktop database's
/// reckoning, so a reader that asked the generic question first would answer
/// "an archive" for a Word document — and the panel would list `[Content_Types]
/// .xml` where the words should be. `text/rtf` has the same shape one level
/// up: it descends from `text/plain`, and that is exactly why it was being
/// drawn as markup.
#[test]
fn a_document_is_never_taken_for_the_container_it_travels_in() {
    assert_eq!(kind_of(DOCX), Some(Kind::Words));
    assert_eq!(kind_of(ODT), Some(Kind::Words));
    assert_eq!(kind_of(PPTX), Some(Kind::Slides));
    assert_eq!(kind_of(XLSX), Some(Kind::Sheet));
    assert_eq!(kind_of(ODS), Some(Kind::Sheet));
    assert_eq!(kind_of("application/epub+zip"), Some(Kind::Book));
    assert_eq!(kind_of("text/rtf"), Some(Kind::Rich));
    assert_eq!(kind_of("application/zip"), Some(Kind::Archive));
    // AND NOTHING ELSE IS ONE. A `.txt` and a `.png` have their own readers,
    // and a `kind_of` that claimed them would take the panel away from both.
    assert_eq!(kind_of("text/plain"), None);
    assert_eq!(kind_of("image/png"), None);
    assert_eq!(kind_of("model/step"), None);
}

#[test]
fn a_word_document_says_what_it_says_and_not_what_it_is_made_of() {
    let xml = br#"<?xml version="1.0"?>
<w:document xmlns:w="x"><w:body>
<w:p><w:r><w:t>The first paragraph</w:t></w:r></w:p>
<w:p><w:r><w:t xml:space="preserve">split </w:t></w:r><w:r><w:t>over two runs</w:t></w:r></w:p>
<w:p><w:r><w:instrText>PAGE \* MERGEFORMAT</w:instrText></w:r></w:p>
</w:body></w:document>"#;
    let out = read(
        "a.docx",
        DOCX,
        &zipped(&[("[Content_Types].xml", b"<x/>"), ("word/document.xml", xml)]),
    )
    .expect("it reads");
    // TWO RUNS ARE ONE PARAGRAPH — a `.docx` splits a sentence at every change
    // of formatting, and a reader that made a line per run would draw a column
    // of fragments.
    assert_eq!(out, vec!["The first paragraph", "split over two runs"]);
    // AND THE FIELD CODE IS NOT THE DOCUMENT. `PAGE \* MERGEFORMAT` is how
    // Word writes a page number, and it is the layout this module does not
    // draw.
}

#[test]
fn an_open_document_reads_from_the_part_its_own_format_names() {
    let xml = br#"<?xml version="1.0"?>
<office:document-content xmlns:office="x" xmlns:text="y">
<office:body><office:text>
<text:h>A heading</text:h>
<text:p>A paragraph with <text:span>a span</text:span> inside it</text:p>
</office:text></office:body></office:document-content>"#;
    let out = read(
        "a.odt",
        ODT,
        &zipped(&[
            ("mimetype", b"application/vnd.oasis.opendocument.text"),
            ("content.xml", xml),
        ]),
    )
    .expect("it reads");
    assert_eq!(out, vec!["A heading", "A paragraph with a span inside it"]);
}

/// THE OWNER'S CHOICE, MEASURED: *"planilha vira tabela"*.
///
/// What makes a sheet readable in a monospaced panel is that the same column
/// starts at the same cell on every row, and that is the whole of what `align`
/// does. A dump of the same values separated by commas is the file and not the
/// sheet.
#[test]
fn a_spreadsheet_comes_out_as_a_table_with_its_columns_lined_up() {
    let sheet = br#"<?xml version="1.0"?>
<worksheet xmlns="x"><sheetData>
<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row>
<row r="2"><c r="A2"><v>7</v></c><c r="C2" t="s"><v>2</v></c></row>
</sheetData></worksheet>"#;
    let strings = br#"<?xml version="1.0"?>
<sst xmlns="x"><si><t>a long heading</t></si><si><t>b</t></si><si><t>third</t></si></sst>"#;
    let out = read(
        "a.xlsx",
        XLSX,
        &zipped(&[
            ("xl/sharedStrings.xml", strings),
            ("xl/worksheets/sheet1.xml", sheet),
        ]),
    )
    .expect("it reads");
    assert_eq!(out.len(), 2);
    // THE STRINGS LIVE SOMEWHERE ELSE: a cell of type `s` holds an INDEX into
    // `sharedStrings.xml`, so without that part a sheet of words is a sheet of
    // small integers.
    assert!(
        out[0].starts_with("a long heading"),
        "the shared strings were not followed: {:?}",
        out[0]
    );
    // AND A SKIPPED COLUMN STAYS SKIPPED. `<c r="C2">` straight after
    // `<c r="A2">` means B2 is empty, and a reader that just pushed would
    // slide `third` one column to the left.
    let heading = out[0].find('b').expect("the second column is drawn");
    let under = out[1].find('7').expect("the first column is drawn");
    assert_eq!(under, 0, "the first column did not start at the first cell");
    assert!(
        out[1].len() > heading,
        "the third column was drawn under the second: {:?}",
        out[1]
    );
    assert!(out[1].contains("third"));
}

#[test]
fn an_open_document_spreadsheet_honours_a_repeated_run_of_cells() {
    let xml = br#"<?xml version="1.0"?>
<office:document-content xmlns:office="x" xmlns:table="t" xmlns:text="y">
<office:body><office:spreadsheet><table:table>
<table:table-row><table:table-cell><text:p>one</text:p></table:table-cell>
<table:table-cell table:number-columns-repeated="2"/>
<table:table-cell><text:p>four</text:p></table:table-cell></table:table-row>
</table:table></office:spreadsheet></office:body></office:document-content>"#;
    let out = read("a.ods", ODS, &zipped(&[("content.xml", xml)])).expect("it reads");
    assert_eq!(out.len(), 1);
    // ODF SAYS "THIS CELL, N TIMES", and a reader that ignored the count would
    // put `four` in the second column instead of the fourth.
    assert!(out[0].starts_with("one"));
    assert!(out[0].ends_with("four"));
    assert!(
        out[0].find("four").unwrap_or(0) > 6,
        "the empty run was not honoured: {:?}",
        out[0]
    );
}

/// THE SLIDES ARE READ IN THE ORDER THE FILE NUMBERS THEM, and that order is
/// not the order the names sort in: `slide10.xml` comes before `slide2.xml` as
/// a string, and a presentation read that way is a presentation shuffled.
#[test]
fn a_presentation_is_read_in_its_own_order_and_not_in_the_names_order() {
    let slide = |said: &str| {
        format!(r#"<?xml version="1.0"?><p:sld xmlns:p="x" xmlns:a="y"><p:cSld><a:p><a:r><a:t>{said}</a:t></a:r></a:p></p:cSld></p:sld>"#)
            .into_bytes()
    };
    let (two, ten) = (slide("second"), slide("tenth"));
    let out = read(
        "a.pptx",
        PPTX,
        &zipped(&[
            ("ppt/slides/slide10.xml", &ten),
            ("ppt/slides/slide2.xml", &two),
        ]),
    )
    .expect("it reads");
    assert_eq!(out, vec!["second", "tenth"]);
}

#[test]
fn a_zip_that_is_only_a_zip_lists_what_is_in_it() {
    let out = read(
        "a.zip",
        "application/zip",
        &zipped(&[("one.txt", b"hello"), ("two/three.bin", b"xy")]),
    )
    .expect("it reads");
    assert_eq!(out.len(), 2);
    assert!(out[0].contains("one.txt") && out[0].contains('5'));
    assert!(out[1].contains("two/three.bin"));
}

/// A DOCUMENT OF NOTHING SAYS SO, rather than drawing a blank panel.
///
/// The rule this whole crate keeps: **a refusal is a value, not an absence**.
/// An empty panel and a refused one look the same on screen, and only one of
/// them has a sentence.
#[test]
fn a_document_with_no_words_in_it_is_refused_and_not_left_blank() {
    let xml = br#"<?xml version="1.0"?><w:document xmlns:w="x"><w:body></w:body></w:document>"#;
    assert_eq!(
        read("b.docx", DOCX, &zipped(&[("word/document.xml", xml)])),
        Err(Refusal::Empty)
    );
    // AND ONE WHOSE CONTAINER IS NOT A ZIP AT ALL.
    assert!(read("c.docx", DOCX, b"not a zip at all").is_err());
}

/// THE THREE OLD FORMATS: two of them work, and the third says so.
///
/// MEASURED with the packaged binaries, on real OLE compound files:
/// `catdoc` answered 8 377 bytes, `xls2csv` answered 364 057, and `catppt`
/// answered **zero bytes with exit status zero and nothing on stderr**. An
/// empty answer is therefore a FAILURE and not an empty document — a blank
/// panel says nothing, and a refusal has a sentence.
///
/// Run only where the environment names the programs, which is the devshell,
/// the `tests` derivation and an installed machine. Everywhere else it is the
/// nothing it should be: a test that quietly passes because the tool was
/// missing is the trap this repository already records against ffmpeg.
#[test]
fn a_program_that_answers_nothing_is_a_failure_and_not_an_empty_document() {
    use crate::tools::Tool;
    if !Tool::Catppt.there() {
        return;
    }
    // A FILE THAT IS NOT A PRESENTATION AT ALL. `catppt` answers nothing for
    // it exactly as it does for a real `.ppt`, which is the behaviour under
    // test — what must not happen is a blank panel with no sentence.
    let dir = std::env::temp_dir().join(format!("ronoctua-ole-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the test directory");
    let path = dir.join("nothing.ppt");
    std::fs::write(&path, b"not a presentation").expect("write the fixture");
    let answer = of(&path, "application/vnd.ms-powerpoint").expect("this type is a document");
    let _ = std::fs::remove_file(&path);
    assert!(
        matches!(answer, Err(Refusal::Failed | Refusal::TimedOut)),
        "an empty answer came back as a document"
    );
}

/// AND THE SPREADSHEET IS A TABLE WHICHEVER LIBRARY OPENED IT.
///
/// `xls2csv` answers CSV, which is the file and not the sheet. The owner's
/// choice — *"planilha vira tabela"* — is about spreadsheets, so the cells go
/// through the same `align` the `.xlsx` and the `.ods` do. This is the CSV
/// half, which is the part that has to understand quoting.
#[test]
fn a_comma_separated_row_becomes_cells_and_the_quotes_do_not_reach_the_screen() {
    assert_eq!(
        super::csv_row(r#""1","Dulce","Abril","#),
        vec!["1", "Dulce", "Abril", ""]
    );
    // A QUOTE INSIDE A QUOTED FIELD IS DOUBLED, which is the format, and a
    // comma inside one is not a separator.
    assert_eq!(
        super::csv_row(r#""a,b","he said ""no""",7"#),
        vec!["a,b", r#"he said "no""#, "7"]
    );
}
