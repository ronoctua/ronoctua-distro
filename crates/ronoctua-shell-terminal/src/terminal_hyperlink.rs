//! THE LINKS ON A TERMINAL'S SCREEN: what `Ctrl` under the pointer shows and
//! `Ctrl` with a click opens.
//!
//! # Two kinds, one answer
//!
//! - **A hyperlink a program wrote** (OSC 8): its address is the cell's own,
//!   and the mirror says it (`Mirror::link_at`) — the text shown may be
//!   anything, the address is what opens. Its reach is every cell beside it
//!   that names the same address ([`osc8_at`]).
//! - **An address written as text**: found on the rows the window shows
//!   ([`plain_at`]), by its scheme and the characters an address is made of —
//!   Ghostty's own rule for what a link is, written out here so no engine
//!   of patterns is linked for one sentence of it.
//!
//! Both answer a [`Link`]: the address and the cells it covers, which the
//! window underlines while `Ctrl` is held over it ([`underline`]) and opens
//! on `Ctrl` and a click — by the opener every door opens a page with, and
//! only for a scheme of [`SCHEMES`]: a program decides what a hyperlink's
//! address is, and an address this system has no business opening is
//! refused with a sentence, not handed to whatever claims it.
//!
//! # A row that runs into the next
//!
//! The screen says nothing of which rows a long line wrapped across, so an
//! address that reaches the last column and goes on, without a space, at
//! the first column of the next row is read as one address — the only way
//! a long URL in an 80-column terminal is a link at all.

use std::ops::Range;

use ronoctua_render::cells::{Grapheme, Sheet, Underline, Width};

/// The schemes a link may have, written as they begin an address: what is
/// found as text, and the only ones a hyperlink opens with.
pub const SCHEMES: &[&str] = &[
    "https://",
    "http://",
    "ftp://",
    "file://",
    "mailto:",
    "ssh://",
    "git://",
    "gemini://",
    "gopher://",
    "magnet:",
    "news:",
    "tel:",
    "ipfs://",
    "ipns://",
];

/// A link: its address, and the cells of the screen it covers — one span a
/// row, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The address, as it opens.
    pub url: String,
    /// The cells it covers.
    pub spans: Vec<Span>,
}

/// One row's cells of a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The row.
    pub line: u16,
    /// Its columns.
    pub columns: Range<u16>,
}

impl Link {
    /// Whether the cell `(column, line)` is the link's.
    #[must_use]
    pub fn covers(&self, column: u16, line: u16) -> bool {
        self.spans
            .iter()
            .any(|s| s.line == line && s.columns.contains(&column))
    }
}

/// Whether `url` has a scheme this system opens.
#[must_use]
pub fn openable(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    SCHEMES.iter().any(|s| lower.starts_with(s)) && url.len() > scheme_len(&lower).unwrap_or(0)
}

/// How long the scheme `lower` begins with is, if it begins with one.
fn scheme_len(lower: &str) -> Option<usize> {
    SCHEMES
        .iter()
        .find(|s| lower.starts_with(*s))
        .map(|s| s.len())
}

/// A character an address is made of: a letter or a digit of any script (an
/// address may be an IRI), and RFC 3986's own punctuation.
fn of_an_address(c: char) -> bool {
    c.is_alphanumeric() || "-._~:/?#[]@!$&'()*+,;=%".contains(c)
}

/// One row of the sheet as characters, each with the column it is drawn
/// in: a wide character's spacer is no character, and a cell of several
/// code points (an emoji's sequence) is no character an address has —
/// it ends one, as a space does.
fn chars_of(sheet: &Sheet, line: u16) -> Vec<(u16, char)> {
    let mut out = Vec::new();
    for (column, cell) in sheet.row(line).iter().enumerate() {
        let Ok(column) = u16::try_from(column) else {
            break;
        };
        if cell.width == Width::Spacer {
            continue;
        }
        let c = match &cell.grapheme {
            Grapheme::One(c) => *c,
            Grapheme::Many(_) => ' ',
        };
        out.push((column, if c == '\0' { ' ' } else { c }));
    }
    out
}

/// The addresses of one row: `[start, end)` indices into its characters —
/// each beginning with a scheme where no letter or digit is right before it,
/// running over the characters of an address, and trimmed of what ends a
/// sentence (`.`, `,`, a closing bracket the address did not open).
fn addresses(chars: &[(u16, char)]) -> Vec<(usize, usize)> {
    let text: Vec<char> = chars.iter().map(|&(_, c)| c).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let boundary = i == 0 || !text[i - 1].is_alphanumeric();
        let rest: String = text[i..text.len().min(i + 12)]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        let scheme = boundary.then(|| scheme_len(&rest)).flatten();
        let Some(scheme) = scheme else {
            i += 1;
            continue;
        };
        let mut end = i + scheme;
        while end < text.len() && of_an_address(text[end]) {
            end += 1;
        }
        let end = trimmed(&text[i..end]) + i;
        if end > i + scheme {
            out.push((i, end));
            i = end;
        } else {
            i += scheme;
        }
    }
    out
}

/// Where an address ends once what ends a sentence is taken off it: a
/// trailing `.,;:!?'"`, and a closing bracket more than it opened.
fn trimmed(address: &[char]) -> usize {
    let mut end = address.len();
    loop {
        let Some(&last) = end.checked_sub(1).and_then(|e| address.get(e)) else {
            return end;
        };
        let unbalanced = |open: char, close: char| {
            last == close
                && address[..end].iter().filter(|&&c| c == close).count()
                    > address[..end].iter().filter(|&&c| c == open).count()
        };
        if ".,;:!?'\"".contains(last) || unbalanced('(', ')') || unbalanced('[', ']') {
            end -= 1;
        } else {
            return end;
        }
    }
}

/// THE ADDRESS WRITTEN AS TEXT under `(column, line)` of what the sheet
/// shows, if one is — across rows, when it runs on from the last column
/// (see the head of the module). `lines` is how many rows to read: a row the
/// window wrote its own line over is no row of the program's.
#[must_use]
pub fn plain_at(sheet: &Sheet, lines: u16, column: u16, line: u16) -> Option<Link> {
    let last_column = sheet.columns().checked_sub(1)?;
    let mut row = 0u16;
    while row < lines.min(sheet.lines()) {
        // A LOGICAL LINE: this row, and every next one an address runs into.
        let mut joined: Vec<(u16, u16, char)> = chars_of(sheet, row)
            .into_iter()
            .map(|(c, ch)| (row, c, ch))
            .collect();
        let mut end = row;
        while end + 1 < lines.min(sheet.lines())
            && joined
                .last()
                .is_some_and(|&(_, c, ch)| c == last_column && of_an_address(ch))
            && chars_of(sheet, end + 1)
                .first()
                .is_some_and(|&(c, ch)| c == 0 && of_an_address(ch))
        {
            end += 1;
            joined.extend(chars_of(sheet, end).into_iter().map(|(c, ch)| (end, c, ch)));
        }
        if (row..=end).contains(&line) {
            let flat: Vec<(u16, char)> = joined.iter().map(|&(_, c, ch)| (c, ch)).collect();
            for (from, to) in addresses(&flat) {
                let cells = &joined[from..to];
                if cells.iter().any(|&(l, c, _)| l == line && c == column) {
                    return Some(Link {
                        url: cells.iter().map(|&(_, _, ch)| ch).collect(),
                        spans: spans_of(cells.iter().map(|&(l, c, _)| (l, c))),
                    });
                }
            }
            return None;
        }
        row = end + 1;
    }
    None
}

/// Cells in reading order, as one span a row — a wide character's spacer
/// taken in with the character before it.
fn spans_of(cells: impl Iterator<Item = (u16, u16)>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for (line, column) in cells {
        match out.last_mut() {
            Some(s) if s.line == line && column >= s.columns.end => s.columns.end = column + 1,
            _ => out.push(Span {
                line,
                columns: column..column + 1,
            }),
        }
    }
    out
}

/// THE HYPERLINK A PROGRAM WROTE under `(column, line)`, if one is — `at`
/// answering a cell's address (`Mirror::link_at`) — and its reach: every
/// cell around it naming the same address, across rows where it runs on.
/// `columns` × `lines` is the screen.
#[must_use]
pub fn osc8_at(
    columns: u16,
    lines: u16,
    column: u16,
    line: u16,
    at: impl Fn(u16, u16) -> Option<String>,
) -> Option<Link> {
    let url = at(column, line)?;
    let same = |c: u16, l: u16| at(c, l).as_deref() == Some(url.as_str());
    let (columns, lines) = (columns.max(1), lines.max(1));
    let index = |c: u16, l: u16| u32::from(l) * u32::from(columns) + u32::from(c);
    let cell = |i: u32| {
        (
            u16::try_from(i % u32::from(columns)).unwrap_or(0),
            u16::try_from(i / u32::from(columns)).unwrap_or(0),
        )
    };
    let here = index(column, line);
    let mut first = here;
    while first > 0 {
        let (c, l) = cell(first - 1);
        if !same(c, l) {
            break;
        }
        first -= 1;
    }
    let mut end = here + 1;
    while end < index(0, lines) {
        let (c, l) = cell(end);
        if !same(c, l) {
            break;
        }
        end += 1;
    }
    Some(Link {
        url,
        spans: spans_of((first..end).map(|i| {
            let (c, l) = cell(i);
            (l, c)
        })),
    })
}

/// Underlines `link`'s cells on `sheet`, a single line in each cell's own
/// colour — writing only the rows whose cells change, so the GPU rebuilds
/// nothing a frame did not move.
pub fn underline(sheet: &mut Sheet, link: &Link) {
    for span in &link.spans {
        if span.line >= sheet.lines() {
            continue;
        }
        let row = sheet.row(span.line);
        let end = usize::from(span.columns.end).min(row.len());
        let from = usize::from(span.columns.start).min(end);
        if row[from..end]
            .iter()
            .all(|c| c.style.underline != Underline::None)
        {
            continue;
        }
        for cell in &mut sheet.row_mut(span.line)[from..end] {
            if cell.style.underline == Underline::None {
                cell.style.underline = Underline::Single;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_render::cells::{Cell, Defaults, Geometry, Rgba};

    fn sheet_of(rows: &[&str]) -> Sheet {
        let stroke = ronoctua_render::sprite::Stroke {
            top: 0,
            thickness: 1,
        };
        let geometry = Geometry {
            cell_width: 10,
            cell_height: 20,
            baseline: 15,
            stroke: 1,
            underline: stroke,
            strike: stroke,
            overline: stroke,
            size: 14.0,
            scale: 1,
        };
        let columns = 20u16;
        let mut sheet = Sheet::new(
            geometry,
            columns,
            u16::try_from(rows.len()).unwrap(),
            Defaults {
                foreground: Rgba::opaque(200, 200, 200),
                background: Rgba::opaque(0, 0, 0),
                faint: 128,
            },
        );
        for (l, text) in rows.iter().enumerate() {
            let line = u16::try_from(l).unwrap();
            let cells = sheet.row_mut(line);
            let mut x = 0;
            for g in ronoctua_protocol::cells_of(text) {
                let wide = ronoctua_protocol::cells_of_one(g) == 2;
                cells[x] = Cell {
                    grapheme: Grapheme::of(g),
                    width: if wide { Width::Wide } else { Width::Narrow },
                    ..Cell::default()
                };
                if wide {
                    cells[x + 1] = Cell {
                        width: Width::Spacer,
                        ..Cell::default()
                    };
                }
                x += 1 + usize::from(wide);
            }
        }
        sheet
    }

    #[test]
    fn an_address_written_as_text_is_found_under_the_pointer() {
        // FAILS IF an address on screen is not a link where the pointer is,
        // its end takes the sentence's full stop, a closing bracket it did
        // not open is taken with it, or a word that only contains a scheme
        // (`xhttps://`) is read as one.
        let s = sheet_of(&["see https://a.b/c.", "(http://x.y/z) ok", "xhttps://no.pe"]);
        let link = plain_at(&s, 3, 6, 0).expect("a link");
        assert_eq!(link.url, "https://a.b/c");
        assert_eq!(
            link.spans,
            vec![Span {
                line: 0,
                columns: 4..17
            }]
        );
        assert!(plain_at(&s, 3, 17, 0).is_none(), "the full stop is no link");
        assert!(plain_at(&s, 3, 2, 0).is_none(), "`see` is no link");
        let bracketed = plain_at(&s, 3, 3, 1).expect("a link");
        assert_eq!(bracketed.url, "http://x.y/z");
        assert!(plain_at(&s, 3, 5, 2).is_none(), "a scheme inside a word");
        assert!(openable(&link.url));
    }

    #[test]
    fn a_wide_character_before_an_address_keeps_its_columns() {
        // FAILS IF the columns of an address after a wide character are
        // counted in characters (the underline lands a cell to the left).
        let s = sheet_of(&["日本 ftp://h/x"]);
        let link = plain_at(&s, 1, 6, 0).expect("a link");
        assert_eq!(link.url, "ftp://h/x");
        assert_eq!(
            link.spans,
            vec![Span {
                line: 0,
                columns: 5..14
            }]
        );
    }

    #[test]
    fn an_address_that_runs_off_the_last_column_goes_on_below() {
        // FAILS IF a long address an 80-column terminal wrapped is only its
        // first row (the link opens half an address), or a row that merely
        // ends at the edge swallows the next row's words.
        let s = sheet_of(&[
            "go https://example.o",
            "rg/path and more",
            "the very last cell x",
            "y is no address",
        ]);
        let link = plain_at(&s, 4, 2, 1).expect("the second row is the same link");
        assert_eq!(link.url, "https://example.org/path");
        assert_eq!(
            link.spans,
            vec![
                Span {
                    line: 0,
                    columns: 3..20
                },
                Span {
                    line: 1,
                    columns: 0..7
                }
            ]
        );
        assert_eq!(plain_at(&s, 4, 5, 0), Some(link));
        assert!(plain_at(&s, 4, 0, 3).is_none());
        // AND A ROW THE WINDOW WROTE ITS OWN LINE OVER IS READ AS NOTHING.
        assert!(plain_at(&s, 1, 2, 1).is_none());
    }

    #[test]
    fn a_hyperlink_reaches_every_cell_that_names_it() {
        // FAILS IF a program's hyperlink is only the cell under the pointer
        // (an underline of one letter), runs into a neighbour with another
        // address, or stops at the end of a row it goes on from.
        let at = |c: u16, l: u16| match (l, c) {
            (0, 3..=9) | (1, 0..=2) => Some("https://one.example/".to_owned()),
            (1, 3..=5) => Some("https://two.example/".to_owned()),
            _ => None,
        };
        let link = osc8_at(10, 3, 5, 0, at).expect("a hyperlink");
        assert_eq!(link.url, "https://one.example/");
        assert_eq!(
            link.spans,
            vec![
                Span {
                    line: 0,
                    columns: 3..10
                },
                Span {
                    line: 1,
                    columns: 0..3
                }
            ]
        );
        assert!(link.covers(1, 1) && !link.covers(3, 1));
        assert!(osc8_at(10, 3, 0, 0, at).is_none());
    }

    #[test]
    fn only_a_scheme_this_system_opens_is_opened() {
        // FAILS IF an address a program wrote with a scheme nobody vetted
        // (`javascript:`, a desktop file's) is handed to the opener, or one
        // of ours is refused.
        assert!(openable("https://example.org"));
        assert!(openable("MAILTO:someone@example.org"));
        assert!(openable("file:///home/a/x.txt"));
        assert!(!openable("javascript:alert(1)"));
        assert!(!openable("https://"), "a scheme with nothing after it");
        assert!(!openable("/home/a/x"));
    }

    #[test]
    fn the_underline_touches_only_the_rows_it_changes() {
        // FAILS IF the hovered link is not underlined cell by cell, or a row
        // already underlined is written again — every hover frame a GPU
        // rebuild of rows nothing moved.
        let mut s = sheet_of(&["see https://a.b/c.", "plain"]);
        let link = plain_at(&s, 2, 6, 0).unwrap();
        underline(&mut s, &link);
        assert!(
            s.row(0)[4..17]
                .iter()
                .all(|c| c.style.underline == Underline::Single)
        );
        assert_eq!(s.row(0)[3].style.underline, Underline::None);
        let (first, second) = (s.generation_of(0), s.generation_of(1));
        underline(&mut s, &link);
        assert_eq!(
            s.generation_of(0),
            first,
            "an unchanged row was written again"
        );
        assert_eq!(s.generation_of(1), second);
    }
}
