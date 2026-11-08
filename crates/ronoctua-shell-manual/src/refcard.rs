//! THE EMACS REFERENCE CARD — GNU's own, opened from `LEADER h` and by
//! `LEADER e` (the owner, 2026-11-08), on the whole screen in as many columns
//! as fit.
//!
//! # GNU's card, read from the Emacs this system installs
//!
//! Not the PDF of gnu.org, and no longer typeset at all: the card's source
//! ships with Emacs (`etc/refcards/refcard.tex`, and the Brazilian translation
//! beside it), and `nix/emacs-refcard.nix` reads its WORDS out of it as data —
//! the sections, each key and what it does, the tables, the notes, the
//! examples and the copyright notice (`<language>.json`). The window lays
//! them out itself, in the system's face and the theme's inks: a picture of
//! Computer Modern at whatever size the screen gave was hard to read, and in
//! no font of this system.
//!
//! # The card's words are data
//!
//! What the card says is GNU's text, read out of the installed source like
//! the content of a file (invariant 8) — in the person's language when Emacs
//! ships the card in it, in English otherwise. The frame around it (the
//! window's name, its keys, the line that filters) is this crate's keys.
//!
//! # The layout is arithmetic
//!
//! [`columns`] flows the sections into columns of one width and one height:
//! a section goes whole into the column that holds it, or starts a new one,
//! and only one taller than a column is cut — at a line, with its title said
//! again on top of the next column. Each line is a list of [`Piece`]s placed
//! in cells, toned by what they are ([`Tone`]); the window only paints them.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Where the read card is: `<dir>/<language>.json`.
pub const ENV: &str = "RONOCTUA_EMACS_REFCARD";

/// The language whose card stands in for every other.
const FALLBACK: &str = "en";

/// The format `gen-refcard.py` writes; any other is refused, never guessed.
const FORMAT: u32 = 1;

/// A stretch of the card's text in one style: `[style, text]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Run(pub Style, pub String);

/// How a stretch was set on paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Style {
    /// Running text.
    #[serde(rename = "p")]
    Plain,
    /// The card's bold: the word a row is about.
    #[serde(rename = "b")]
    Bold,
    /// A variable — `char`, `arg`, `n`.
    #[serde(rename = "i")]
    Variable,
    /// Keys, typed as written.
    #[serde(rename = "k")]
    Keys,
    /// A line of code.
    #[serde(rename = "c")]
    Code,
}

/// The card, as `gen-refcard.py` read it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Card {
    format: u32,
    /// The Emacs it was read from (its major version).
    pub version: String,
    /// The card's title.
    pub title: Vec<Run>,
    /// The line under it: the version it is for.
    pub subtitle: Vec<Run>,
    /// The sections, in the card's order.
    pub sections: Vec<Section>,
    /// The copyright notice, which goes everywhere the card goes.
    pub notice: Vec<Block>,
}

/// A section: its title and its blocks.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    /// What it is about.
    pub title: Vec<Run>,
    /// What it says, in order.
    pub blocks: Vec<Block>,
}

/// One thing a section says.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Block {
    /// What a key does, and the key.
    Key {
        /// What it does.
        does: Vec<Run>,
        /// The key, or keys.
        keys: Vec<Run>,
        /// Under a line that groups the keys after it.
        indent: bool,
    },
    /// Rows of three: what moves, and the keys backward and forward.
    Table {
        /// The rows, heads included.
        rows: Vec<TableRow>,
    },
    /// Running text.
    Note {
        /// What it says.
        text: Vec<Run>,
    },
    /// Code, line by line, its indentation kept.
    Example {
        /// The lines.
        lines: Vec<Vec<Run>>,
    },
    /// A line of the notice, centred.
    Centred {
        /// What it says.
        text: Vec<Run>,
    },
    /// The air between two groups.
    Gap,
}

/// A row of a table; one with no cells is the air between two parts of it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableRow {
    /// The row that names the columns.
    pub head: bool,
    /// Its three cells, none for the air.
    pub cells: Vec<Vec<Run>>,
    /// Under a line that groups the rows after it.
    pub indent: bool,
}

/// Why there is no card to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Missing {
    /// The machine has none (no Emacs, or no variable).
    Absent,
    /// A card is there and could not be read: the file and why.
    Unreadable(String),
}

/// The card for `language` — its own when Emacs ships one (`pt-BR`), the
/// English otherwise.
///
/// # Errors
///
/// When the card is not on this machine, or cannot be read.
pub fn card(language: &str) -> Result<Card, Missing> {
    let root = std::env::var_os(ENV).ok_or(Missing::Absent)?;
    card_in(Path::new(&root), language)
}

/// [`card`], under `root`.
///
/// # Errors
///
/// As [`card`].
pub fn card_in(root: &Path, language: &str) -> Result<Card, Missing> {
    let file: PathBuf = [language, FALLBACK]
        .iter()
        .map(|l| root.join(format!("{l}.json")))
        .find(|f| f.is_file())
        .ok_or(Missing::Absent)?;
    let text = std::fs::read_to_string(&file)
        .map_err(|e| Missing::Unreadable(format!("{}: {e}", file.display())))?;
    let card: Card = serde_json::from_str(&text)
        .map_err(|e| Missing::Unreadable(format!("{}: {e}", file.display())))?;
    if card.format != FORMAT {
        return Err(Missing::Unreadable(format!(
            "{}: format {} where {FORMAT} is read",
            file.display(),
            card.format
        )));
    }
    Ok(card)
}

/// What a piece of a line is, which its ink follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// The card's title, and a section's.
    Heading,
    /// The version under the title, and the notice.
    Quiet,
    /// What a key does.
    Does,
    /// The word a row is about — the card's bold.
    Strong,
    /// A key.
    Key,
    /// A variable, and the `or` between two keys.
    Aside,
    /// A line of code.
    Code,
}

/// Text placed in a column, `at` cells from its left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// Cells from the column's left.
    pub at: u16,
    /// What it says.
    pub text: String,
    /// What it is.
    pub tone: Tone,
}

/// One line of a column — empty for air.
pub type Line = Vec<Piece>;

/// The tone a run is drawn in, by where it stands: in a key's place a plain
/// run is the `or` between two keys.
const fn tone_of(style: Style, keys: bool) -> Tone {
    match (style, keys) {
        (Style::Keys, _) => Tone::Key,
        (Style::Bold, _) => Tone::Strong,
        (Style::Variable, _) | (Style::Plain, true) => Tone::Aside,
        (Style::Code, _) => Tone::Code,
        (Style::Plain, false) => Tone::Does,
    }
}

/// A run's text, its tone, and nothing else.
fn toned(runs: &[Run], keys: bool) -> Vec<(String, Tone)> {
    runs.iter()
        .map(|Run(style, text)| (text.clone(), tone_of(*style, keys)))
        .collect()
}

/// How wide some toned text is, in cells.
fn cells(parts: &[(String, Tone)]) -> u16 {
    parts
        .iter()
        .map(|(t, _)| ronoctua_render::layout::width(t))
        .fold(0, u16::saturating_add)
}

/// Toned text as pieces on ONE line from `at`, each run after the last —
/// two neighbours of one tone a single piece.
fn placed(parts: Vec<(String, Tone)>, mut at: u16) -> Line {
    let mut line: Line = Vec::with_capacity(parts.len());
    for (text, tone) in parts {
        let wide = ronoctua_render::layout::width(&text);
        match line.last_mut() {
            Some(last) if last.tone == tone => last.text.push_str(&text),
            _ => line.push(Piece { at, text, tone }),
        }
        at = at.saturating_add(wide);
    }
    line
}

/// Toned text WRAPPED into `room` cells, its tones kept: words are filled
/// line by line — a word is what stands between two spaces, whatever runs
/// it is made of — and one wider than the room alone is cut at its
/// clusters (`layout::wrap`).
fn wrapped(parts: &[(String, Tone)], room: u16) -> Vec<Vec<(String, Tone)>> {
    let room = room.max(1);
    // THE WORDS, each a list of toned stretches.
    let mut words: Vec<Vec<(String, Tone)>> = vec![Vec::new()];
    for (text, tone) in parts {
        for (k, bit) in text.split(' ').enumerate() {
            if k > 0 {
                words.push(Vec::new());
            }
            if !bit.is_empty()
                && let Some(word) = words.last_mut()
            {
                word.push((bit.to_owned(), *tone));
            }
        }
    }
    words.retain(|w| !w.is_empty());
    let mut lines: Vec<Vec<(String, Tone)>> = Vec::new();
    let mut line: Vec<(String, Tone)> = Vec::new();
    let mut wide: u16 = 0;
    for word in words {
        let w = cells(&word);
        if w > room {
            // A WORD WIDER THAN THE LINE: on lines of its own, cut at its
            // clusters, in its first stretch's tone.
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                wide = 0;
            }
            let tone = word[0].1;
            let text: String = word.into_iter().map(|(t, _)| t).collect();
            for cut in ronoctua_render::layout::wrap(&text, room, usize::MAX) {
                lines.push(vec![(cut, tone)]);
            }
            continue;
        }
        let space = u16::from(!line.is_empty());
        if wide + space + w > room {
            lines.push(std::mem::take(&mut line));
            wide = 0;
        } else if space == 1 {
            // THE SPACE takes the tone of what it follows.
            if let Some(last) = line.last_mut() {
                last.0.push(' ');
            }
            wide += 1;
        }
        wide += w;
        line.extend(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Toned text wrapped into `room` and placed from `at`, a line each.
fn paragraph(parts: &[(String, Tone)], at: u16, room: u16) -> Vec<Line> {
    wrapped(parts, room)
        .into_iter()
        .map(|l| placed(l, at))
        .collect()
}

/// The indent of a key row or a table row inside an indented group, in cells.
const INDENT: u16 = 2;

/// The air between what a key does and the key, in cells.
const BESIDE: u16 = 2;

/// The plain text of runs, lower-cased — what a filter is matched against.
fn said(runs: &[Run]) -> String {
    runs.iter()
        .map(|Run(_, t)| t.as_str())
        .collect::<String>()
        .to_lowercase()
}

/// Does a block say `query` (lower-cased)? Air never does.
fn block_says(block: &Block, query: &str) -> bool {
    match block {
        Block::Key { does, keys, .. } => said(does).contains(query) || said(keys).contains(query),
        Block::Table { rows } => rows.iter().any(|r| row_says(r, query)),
        Block::Note { text } | Block::Centred { text } => said(text).contains(query),
        Block::Example { lines } => lines.iter().any(|l| said(l).contains(query)),
        Block::Gap => false,
    }
}

/// Does a table's row say `query`?
fn row_says(row: &TableRow, query: &str) -> bool {
    !row.head && row.cells.iter().any(|c| said(c).contains(query))
}

/// A section, kept to what says `query` — the whole of it for no query, and
/// `None` when nothing in it says it. A table keeps its head over the rows
/// that matched.
fn narrowed(section: &Section, query: &str) -> Option<Section> {
    if query.is_empty() {
        return Some(section.clone());
    }
    if said(&section.title).contains(query) {
        return Some(section.clone());
    }
    let blocks: Vec<Block> = section
        .blocks
        .iter()
        .filter(|b| block_says(b, query))
        .map(|b| match b {
            Block::Table { rows } => {
                // EACH PART of the table keeps its head when a row under it
                // matched.
                let mut kept = Vec::new();
                let mut head: Option<&TableRow> = None;
                for r in rows {
                    if r.head {
                        head = Some(r);
                    } else if row_says(r, query) {
                        if let Some(h) = head.take() {
                            kept.push(h.clone());
                        }
                        kept.push(r.clone());
                    }
                }
                Block::Table { rows: kept }
            }
            other => other.clone(),
        })
        .collect();
    (!blocks.is_empty()).then(|| Section {
        title: section.title.clone(),
        blocks,
    })
}

/// A section's lines at `width` cells: its title, then its blocks.
fn section_lines(section: &Section, width: u16) -> Vec<Line> {
    let mut lines = paragraph(&toned_as(&section.title, Tone::Heading), 0, width);
    // THE KEYS' COLUMN of a section: as wide as its widest key that leaves
    // three quarters of the line to what it does — the paper card's
    // proportion; a wider one, an `M-x` and its command, is placed at the
    // line's right edge, under what it does when the two do not fit side by
    // side, and never narrows the others' room.
    let key_cells = section
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Key { keys, .. } => Some(cells(&toned(keys, true))),
            _ => None,
        })
        .filter(|w| *w <= width / 4)
        .max()
        .unwrap_or(0);
    for block in &section.blocks {
        lines.extend(block_lines(block, width, key_cells));
    }
    lines
}

/// Runs in one tone, whatever their style.
fn toned_as(runs: &[Run], tone: Tone) -> Vec<(String, Tone)> {
    runs.iter().map(|Run(_, t)| (t.clone(), tone)).collect()
}

/// A block's lines at `width` cells, the keys in a column `key_cells` wide.
fn block_lines(block: &Block, width: u16, key_cells: u16) -> Vec<Line> {
    match block {
        Block::Gap => vec![Vec::new()],
        Block::Note { text } => paragraph(&toned(text, false), 0, width),
        Block::Centred { text } => {
            let parts = toned_as(text, Tone::Quiet);
            wrapped(&parts, width)
                .into_iter()
                .map(|l| {
                    let at = width.saturating_sub(cells(&l)) / 2;
                    placed(l, at)
                })
                .collect()
        }
        Block::Example { lines } => lines
            .iter()
            .flat_map(|l| {
                // CODE IS CUT, not filled: its spaces are its indentation.
                let text: String = l.iter().map(|Run(_, t)| t.as_str()).collect();
                if ronoctua_render::layout::width(&text) <= width {
                    vec![placed(toned(l, false), 0)]
                } else {
                    ronoctua_render::layout::wrap(&text, width, usize::MAX)
                        .into_iter()
                        .map(|cut| placed(vec![(cut, Tone::Code)], 0))
                        .collect()
                }
            })
            .collect(),
        Block::Key { does, keys, indent } => {
            let lead = if *indent { INDENT } else { 0 };
            let keys = toned(keys, true);
            let wide = cells(&keys);
            let column = width.saturating_sub(key_cells.max(1));
            let room = column.saturating_sub(lead + BESIDE).max(1);
            let mut lines = paragraph(&toned(does, false), lead, room);
            if wide <= key_cells {
                // IN THE KEYS' COLUMN, on the first line of what it does.
                match lines.first_mut() {
                    Some(first) => first.extend(placed(keys, column)),
                    None => lines.push(placed(keys, column)),
                }
            } else {
                // A WIDE KEY — `M-x` and a command — at the right edge, on
                // the last line when it fits there, else on its own.
                let at = width.saturating_sub(wide);
                let fits = lines.last().is_some_and(|l| {
                    l.last()
                        .map_or(0, |p| p.at + ronoctua_render::layout::width(&p.text))
                        + BESIDE
                        <= at
                });
                if fits {
                    if let Some(last) = lines.last_mut() {
                        last.extend(placed(keys, at));
                    }
                } else {
                    lines.push(placed(keys, at));
                }
            }
            lines
        }
        Block::Table { rows } => {
            // TWO KEY COLUMNS as wide as their widest cell, and what moves
            // in the rest.
            let col = |k: usize| {
                rows.iter()
                    .filter_map(|r| r.cells.get(k))
                    .map(|c| cells(&toned(c, true)))
                    .max()
                    .unwrap_or(0)
            };
            let (second, third) = (col(1), col(2));
            let third_at = width.saturating_sub(third);
            let second_at = third_at.saturating_sub(second + BESIDE);
            let mut lines = Vec::new();
            for row in rows {
                if row.cells.is_empty() {
                    lines.push(Vec::new());
                    continue;
                }
                let lead = if row.indent { INDENT } else { 0 };
                let room = second_at.saturating_sub(lead + BESIDE).max(1);
                let tone = |k: usize| {
                    let cell = row.cells.get(k).map_or(&[][..], Vec::as_slice);
                    if row.head {
                        toned_as(cell, Tone::Strong)
                    } else {
                        toned(cell, k > 0)
                    }
                };
                let mut out = paragraph(&tone(0), lead, room);
                let keys = [placed(tone(1), second_at), placed(tone(2), third_at)].concat();
                match out.first_mut() {
                    Some(first) => first.extend(keys),
                    None => out.push(keys),
                }
                lines.extend(out);
            }
            lines
        }
    }
}

/// The card laid out in columns `width` cells wide and `height` lines tall,
/// kept to what says `query` (any case; empty for all of it).
///
/// The title and its version open the first column; the notice closes the
/// last. A section goes whole where it fits, or opens the next column; one
/// taller than a column is cut at a line, and its title said again on top of
/// the next. No column opens on air.
#[must_use]
pub fn columns(card: &Card, width: u16, height: u16, query: &str) -> Vec<Vec<Line>> {
    let height = usize::from(height.max(2));
    let query = query.to_lowercase();
    let mut columns: Vec<Vec<Line>> = vec![Vec::new()];
    let mut opening = paragraph(&toned_as(&card.title, Tone::Heading), 0, width);
    opening.extend(paragraph(&toned_as(&card.subtitle, Tone::Quiet), 0, width));
    place(&mut columns, height, &[], opening);
    for section in card.sections.iter().filter_map(|s| narrowed(s, &query)) {
        let lines = section_lines(&section, width);
        let title_lines = paragraph(&toned_as(&section.title, Tone::Heading), 0, width).len();
        let (title, body) = lines.split_at(title_lines.min(lines.len()));
        place(&mut columns, height, title, body.to_vec());
    }
    let notice: Vec<Line> = card
        .notice
        .iter()
        .flat_map(|b| match b {
            Block::Note { text } => paragraph(&toned_as(text, Tone::Quiet), 0, width),
            other => block_lines(other, width, 0),
        })
        .collect();
    place(&mut columns, height, &[], notice);
    columns.retain(|c| !c.is_empty());
    columns
}

/// Puts a section — its title's lines and its body's — into the columns,
/// `height` lines each, by [`columns`]' rules.
fn place(columns: &mut Vec<Vec<Line>>, height: usize, title: &[Line], body: Vec<Line>) {
    // A TITLE NEVER FILLS A COLUMN: a line of it at least is left for the body.
    let title = &title[..title.len().min(height - 1)];
    let mut body = body.into_iter().peekable();
    let mut first = true;
    while body.peek().is_some() || first {
        let column = columns.last_mut().expect("there is always a column");
        let air = usize::from(!column.is_empty());
        let left = height.saturating_sub(column.len() + air);
        let rest = title.len() + body.len();
        // WHOLE, where there is room for it — or a fresh column, when it
        // fits one and this one has started.
        if first && rest > left && !column.is_empty() && rest <= height {
            columns.push(Vec::new());
            continue;
        }
        // NOT A TITLE ALONE at a column's foot: it and a line at least.
        if left < title.len() + 1 && !column.is_empty() {
            columns.push(Vec::new());
            continue;
        }
        if !column.is_empty() {
            column.push(Vec::new());
        }
        let top = column.len() + title.len();
        column.extend(title.iter().cloned());
        first = false;
        while column.len() < height {
            match body.next() {
                // NO COLUMN OPENS ON AIR, and no section's lines do.
                Some(line) if line.is_empty() && column.len() == top => {}
                Some(line) => column.push(line),
                None => break,
            }
        }
        while column.last().is_some_and(Vec::is_empty) {
            column.pop();
        }
        if body.peek().is_some() {
            columns.push(Vec::new());
        }
    }
}

/// Does the card hold anything that says `query` — a section's title or a
/// line of it?
#[must_use]
pub fn says(card: &Card, query: &str) -> bool {
    let query = query.to_lowercase();
    query.is_empty() || card.sections.iter().any(|s| narrowed(s, &query).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(style: Style, text: &str) -> Run {
        Run(style, text.to_owned())
    }

    /// A card of `sections` sections of `rows` key rows each.
    fn planted(sections: usize, rows: usize) -> Card {
        Card {
            format: FORMAT,
            version: "31".into(),
            title: vec![run(Style::Plain, "GNU Emacs Reference Card")],
            subtitle: vec![run(Style::Plain, "(for version 31)")],
            sections: (0..sections)
                .map(|s| Section {
                    title: vec![run(Style::Plain, &format!("Section {s}"))],
                    blocks: (0..rows)
                        .map(|r| Block::Key {
                            does: vec![
                                run(Style::Bold, "save"),
                                run(Style::Plain, &format!(" the file number {r} back to disk")),
                            ],
                            keys: vec![run(Style::Keys, "C-x C-s")],
                            indent: false,
                        })
                        .collect(),
                })
                .collect(),
            notice: vec![Block::Centred {
                text: vec![run(
                    Style::Plain,
                    "Copyright © 2026 Free Software Foundation, Inc.",
                )],
            }],
        }
    }

    fn text_of(line: &Line) -> String {
        let mut out = String::new();
        for p in line {
            while (ronoctua_render::layout::width(&out) as usize) < p.at as usize {
                out.push(' ');
            }
            out.push_str(&p.text);
        }
        out
    }

    /// FAILS IF: the card is not found by its language, another language's
    /// is not the English, a format this window does not read is taken, or a
    /// field the reader does not know is let through.
    #[test]
    fn the_card_is_found_by_language_and_a_strange_one_is_refused() {
        let root = std::env::temp_dir().join(format!("ronoctua-refcard-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let doc = |title: &str, format: u32| {
            format!(
                r#"{{"format": {format}, "version": "31", "title": [["p", "{title}"]], "subtitle": [],
                "sections": [{{"title": [["p", "Files"]], "blocks": [
                  {{"kind": "key", "does": [["b", "read"], ["p", " a file"]], "keys": [["k", "C-x C-f"]], "indent": false}},
                  {{"kind": "gap"}},
                  {{"kind": "table", "rows": [{{"head": true, "cells": [[["b", "entity"]], [["b", "backward"]], [["b", "forward"]]], "indent": false}}]}}
                ]}}], "notice": []}}"#
            )
        };
        std::fs::write(root.join("en.json"), doc("Card", 1)).unwrap();
        std::fs::write(root.join("pt-BR.json"), doc("Cartão", 1)).unwrap();
        assert_eq!(
            card_in(&root, "de-DE").unwrap().title,
            vec![run(Style::Plain, "Card")]
        );
        assert_eq!(
            card_in(&root, "pt-BR").unwrap().title,
            vec![run(Style::Plain, "Cartão")]
        );
        std::fs::write(root.join("pt-BR.json"), doc("Cartão", 2)).unwrap();
        assert!(matches!(
            card_in(&root, "pt-BR"),
            Err(Missing::Unreadable(_))
        ));
        let strange =
            doc("Cartão", 1).replacen("\"indent\": false", "\"indent\": false, \"colour\": 1", 1);
        std::fs::write(root.join("pt-BR.json"), strange).unwrap();
        assert!(
            matches!(card_in(&root, "pt-BR"), Err(Missing::Unreadable(_))),
            "an unknown field was read"
        );
        assert_eq!(card_in(&root.join("nowhere"), "en"), Err(Missing::Absent));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a column is taller than asked, a line wider, a section cut
    /// that fits a column whole, a cut section's title not said again on
    /// the next column, a column opens on air, or a row is lost.
    #[test]
    fn the_columns_hold_every_row_inside_their_size() {
        let card = planted(6, 7);
        let (width, height) = (40, 24);
        let cols = columns(&card, width, height, "");
        let mut rows = 0;
        for col in &cols {
            assert!(
                col.len() <= usize::from(height),
                "a column of {} lines",
                col.len()
            );
            assert!(!col[0].is_empty(), "a column opens on air");
            for line in col {
                assert!(
                    ronoctua_render::layout::width(&text_of(line)) <= width,
                    "a line wider than the column: {:?}",
                    text_of(line)
                );
                rows += usize::from(line.iter().any(|p| p.tone == Tone::Key));
            }
        }
        assert_eq!(rows, 6 * 7, "a row was lost, or said twice");
        // EVERY SECTION STARTS whole in one column (each fits one).
        for s in 0..6 {
            let title = format!("Section {s}");
            let holding = cols
                .iter()
                .filter(|c| c.iter().any(|l| l.iter().any(|p| p.text == title)))
                .count();
            assert_eq!(holding, 1, "{title} was cut though it fits a column");
        }
        // A SECTION TALLER THAN A COLUMN is cut, and its title said again.
        let tall = planted(1, 40);
        let cols = columns(&tall, width, height, "");
        let titled = cols
            .iter()
            .filter(|c| c.iter().any(|l| l.iter().any(|p| p.text == "Section 0")))
            .count();
        assert!(titled >= 2, "the cut section's title is not said again");
    }

    /// FAILS IF: what a key does is not wrapped inside its room, its bold
    /// word loses its tone, or the key is not in the keys' column.
    #[test]
    fn a_row_wraps_its_words_and_keeps_its_tones() {
        let card = planted(1, 1);
        let lines = section_lines(&card.sections[0], 40);
        let key = lines
            .iter()
            .flatten()
            .find(|p| p.tone == Tone::Key)
            .expect("the key");
        assert_eq!(key.at, 40 - ronoctua_render::layout::width("C-x C-s"));
        assert!(
            lines
                .iter()
                .flatten()
                .any(|p| p.tone == Tone::Strong && p.text.starts_with("save"))
        );
        for line in &lines {
            let does: Vec<_> = line.iter().filter(|p| p.tone != Tone::Key).collect();
            if let Some(last) = does.last() {
                assert!(last.at + ronoctua_render::layout::width(&last.text) <= key.at);
            }
        }
        assert!(lines.len() > 2, "nothing was wrapped: {lines:?}");
    }

    /// FAILS IF: the filter keeps a section that does not say what was
    /// typed, loses one that does, or forgets a matched table's head.
    #[test]
    fn the_filter_keeps_only_what_says_what_was_typed() {
        let mut card = planted(3, 2);
        card.sections[1].blocks = vec![Block::Table {
            rows: vec![
                TableRow {
                    head: true,
                    cells: vec![
                        vec![run(Style::Bold, "entity")],
                        vec![run(Style::Bold, "back")],
                        vec![run(Style::Bold, "fwd")],
                    ],
                    indent: false,
                },
                TableRow {
                    head: false,
                    cells: vec![
                        vec![run(Style::Plain, "word")],
                        vec![run(Style::Keys, "M-b")],
                        vec![run(Style::Keys, "M-f")],
                    ],
                    indent: false,
                },
            ],
        }];
        let cols = columns(&card, 40, 30, "M-F");
        let all: Vec<String> = cols.iter().flatten().map(text_of).collect();
        assert!(
            all.iter().any(|l| l.contains("entity")),
            "the head went: {all:?}"
        );
        assert!(all.iter().any(|l| l.contains("M-f")));
        assert!(
            !all.iter().any(|l| l.contains("Section 0")),
            "a section that says nothing stayed"
        );
        assert!(says(&card, "m-f") && !says(&card, "nothing at all"));
    }
}
