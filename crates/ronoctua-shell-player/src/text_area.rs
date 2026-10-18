//! A TEXT OF SEVERAL LINES, edited where it is shown — in either door.
//!
//! # Why it is its own piece
//!
//! A several-line field — a song's lyrics, a film's description, a note —
//! could only be changed in the editor: the one-line question (`crate::line`)
//! takes a line and turns every break into a space, and a door had nothing
//! else. The owner's rule is that the two doors do the same things, typing
//! included, so this is the model both write with: what is typed, where the
//! caret is, the lines soft-wrapped at the door's width, and the question
//! that stands between `Esc` and throwing a change away.
//!
//! It is PURE: no door, no socket, no clock. A view holds one while it is
//! open and hands it every key (as a question takes every key while it is
//! asked); the rows it is drawn as are the player's [`Builder`]'s, so the
//! footer's region, a window and a column draw the same lines and a click is
//! read back from the same cells ([`hit`]).
//!
//! THE PLAYER'S VIEW HOLDS ONE for both doors (`player::View::edit_text`):
//! a key is `Press::Text` while it is open, `C-v` is the door's paste
//! (`Errand::Paste`, handed back to `View::text_pasted`), a click is read
//! through [`hit`] in `View::clicked_at`, and the hint line is [`hint`].
//! A view of its own reaches the same four through this module.
//!
//! # Keys
//!
//! The arrows move by cluster and by DRAWN line (the soft-wrapped one, the
//! column kept across short lines); `Home`/`End` and `C-a`/`C-e` go to the
//! ends of the written line; `RET` breaks it; `DEL` and the forward delete
//! take a cluster, joining two lines at their seam; `C-v`/`C-y` paste, lines
//! and all; `C-s` keeps it ([`Step::Keep`]). `Esc` or `C-g` leave — at once
//! when nothing changed, else after a question drawn with its "no" first and
//! the cursor born on it.

use crate::player::Target;
use crate::player_rows::{Builder, Row};
use ronoctua_keymap::{Key, Sym};
use ronoctua_tokens as tokens;

/// The keys a text being edited teaches on the hint line, clickable
/// (invariant 19): `Esc` leaves (asking first when something changed), `C-s`
/// keeps it. The sentence under the pointer is `about-<id>`.
pub const OF_THE_TEXT_AREA: &[(&str, &str)] = &[
    // `Esc` first, as every line of the system teaches its way back first.
    ("ESC", "text-area-discard"),
    ("C-s", "text-area-keep"),
];

/// The spot of the `n`th drawn line of a text — how a click is read back.
const LINE_SPOT: &str = "text-area-line-";
/// The spot of the question's "no".
const NO_SPOT: &str = "text-area-no";
/// The spot of the question's "yes".
const YES_SPOT: &str = "text-area-yes";

/// Where the caret is: a written line, and a byte of it that begins a
/// cluster (or its end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caret {
    /// Which written line.
    pub line: usize,
    /// Which byte of it.
    pub byte: usize,
}

/// One DRAWN line: a piece of a written one, soft-wrapped at the width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drawn {
    /// Which written line it is a piece of.
    pub line: usize,
    /// Its first byte in it.
    pub from: usize,
    /// The byte after its last.
    pub to: usize,
    /// It is the written line's last piece.
    pub last: bool,
}

/// What a key, a paste or a click did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nothing changed; no frame is owed.
    Nothing,
    /// The caret moved, or the question's cursor did: a frame is owed.
    Moved,
    /// The text changed.
    Changed,
    /// `C-v` or `C-y`: the door reads the clipboard and hands it to
    /// [`TextArea::paste`].
    WantsPaste,
    /// `C-s`: keep this — the view writes it where it belongs.
    Keep(String),
    /// Leave, and throw away what was not kept.
    Discard,
    /// `Esc` with a change not kept: the question is on screen, "no" first.
    Asking,
}

/// What a click on a text's rows landed on ([`hit`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// A drawn line — the piece of the text AS IT WAS DRAWN, read back from
    /// its row, so a click is placed where it was drawn whatever width the
    /// view is asked at — at a cell counted from where its text begins.
    Text {
        /// Which piece of which written line.
        at: Drawn,
        /// Which cell of it.
        cell: u16,
    },
    /// An answer of the question: `true` is "yes, throw it away".
    Answer(bool),
}

/// Where the text was last PLACED, written by [`Builder::text_area`] as it
/// draws — the width its lines were wrapped at, which `↑` and `↓` walk, and
/// the row its cursor is on, which a door scrolls to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Placed {
    /// The width, in cells.
    width: u16,
    /// The row of the page the caret — or the question's cursor — is on.
    row: usize,
}

/// A text being edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextArea {
    /// Who opened it — the view's own name for it, handed back with it.
    pub id: &'static str,
    /// The written lines, never empty.
    lines: Vec<String>,
    /// Where the caret is.
    caret: Caret,
    /// The column `↑` and `↓` keep across shorter lines, in cells — set by
    /// the first of them, forgotten by any other key.
    goal: Option<u16>,
    /// The text as it was opened, or as it was last kept: what "changed"
    /// is measured against.
    kept: String,
    /// The question after `Esc`, while it is up: `true` when its cursor is
    /// on "yes".
    asking: Option<bool>,
    /// What it is about, in words — drawn as its title — when the opener
    /// gives one ([`TextArea::titled`]).
    title: Option<String>,
    /// Where it was last placed: STORED WHERE IT IS MEASURED.
    placed: std::cell::Cell<Placed>,
}

impl TextArea {
    /// A text for `id`, holding `text`, the caret at its end.
    #[must_use]
    pub fn new(id: &'static str, text: &str) -> Self {
        let text = normalised(text);
        let lines: Vec<String> = text.split('\n').map(ToOwned::to_owned).collect();
        let line = lines.len() - 1;
        let byte = lines[line].len();
        Self {
            id,
            lines,
            caret: Caret { line, byte },
            goal: None,
            kept: text,
            asking: None,
            title: None,
            placed: std::cell::Cell::new(Placed { width: 80, row: 0 }),
        }
    }

    /// The same, with a title in words over its lines.
    #[must_use]
    pub fn titled(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    /// The width it was last drawn at, in cells — what a key is walked at.
    #[must_use]
    pub fn drawn_width(&self) -> u16 {
        self.placed.get().width
    }

    /// The row of the page its cursor was last drawn on — the caret's, or
    /// the question's answer — which the door keeps in sight.
    #[must_use]
    pub fn cursor_row(&self) -> usize {
        self.placed.get().row
    }

    /// What the view KEPT, once it was written where it belongs: "changed"
    /// is measured from it again. Never before — a keep that was refused
    /// leaves the text changed, and `Esc` still asks.
    pub fn kept_as(&mut self, text: &str) {
        self.kept = normalised(text);
    }

    /// What is written, its lines joined by `\n`.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Whether it differs from what was opened or last kept.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.text() != self.kept
    }

    /// Where the caret is.
    #[must_use]
    pub const fn caret(&self) -> Caret {
        self.caret
    }

    /// The question after `Esc`, while it is up: `Some(true)` with its
    /// cursor on "yes".
    #[must_use]
    pub const fn asking(&self) -> Option<bool> {
        self.asking
    }

    /// The DRAWN lines at `width` cells: each written line soft-wrapped —
    /// after a space where one is, else between two clusters, never inside
    /// one — and one cell always left for the caret at a line's end.
    #[must_use]
    pub fn drawn(&self, width: u16) -> Vec<Drawn> {
        let room = width.saturating_sub(1).max(1);
        let mut out = Vec::new();
        for (line, text) in self.lines.iter().enumerate() {
            let first = out.len();
            let mut from = 0;
            let mut used = 0u16;
            // THE LAST SPACE of the piece being filled: a break after it keeps
            // a word whole.
            let mut after_space: Option<usize> = None;
            let mut at = 0;
            for cluster in ronoctua_render::layout::cells_of(text) {
                let cells = ronoctua_protocol::cells_of_one(cluster);
                if used + cells > room && at > from {
                    let cut = after_space.filter(|&b| b > from).unwrap_or(at);
                    out.push(Drawn {
                        line,
                        from,
                        to: cut,
                        last: false,
                    });
                    from = cut;
                    used = ronoctua_protocol::cells(&text[from..at]);
                    after_space = None;
                }
                used += cells;
                at += cluster.len();
                if cluster == " " {
                    after_space = Some(at);
                }
            }
            out.push(Drawn {
                line,
                from,
                to: text.len(),
                last: true,
            });
            debug_assert!(out.len() > first);
        }
        out
    }

    /// Which drawn line the caret is on, and at which cell of it.
    #[must_use]
    pub fn caret_drawn(&self, width: u16) -> (usize, u16) {
        let drawn = self.drawn(width);
        let Caret { line, byte } = self.caret;
        let n = drawn
            .iter()
            .position(|d| d.line == line && byte >= d.from && (byte < d.to || d.last))
            .unwrap_or(0);
        let d = drawn[n];
        let cell = ronoctua_protocol::cells(&self.lines[line][d.from..byte.max(d.from)]);
        (n, cell)
    }

    /// One key, at `width` cells — the width the text is drawn at, which is
    /// what `↑` and `↓` walk.
    pub fn key(&mut self, key: Key, width: u16) -> Step {
        if let Some(yes) = self.asking {
            return self.answer_key(key, yes);
        }
        let keeps_goal = matches!(key.sym, Sym::Up | Sym::Down) && !key.mods.ctrl;
        if !keeps_goal {
            self.goal = None;
        }
        if key.mods.ctrl {
            return match key.sym {
                Sym::Char('v' | 'y') => Step::WantsPaste,
                // KEPT ONLY ONCE WRITTEN: the view calls `kept_as` when the
                // write went through, and a refused one leaves `Esc` asking.
                Sym::Char('s') => Step::Keep(self.text()),
                Sym::Char('g') => self.leave(),
                Sym::Char('a') => self.caret_to(0),
                Sym::Char('e') => self.caret_to(self.lines[self.caret.line].len()),
                _ => Step::Nothing,
            };
        }
        match key.sym {
            Sym::Esc => self.leave(),
            Sym::Enter => {
                self.insert("\n");
                Step::Changed
            }
            Sym::Space => {
                self.insert(" ");
                Step::Changed
            }
            Sym::Backspace => self.erase_back(),
            // THE FORWARD DELETE arrives as U+007F (`line.rs` says why).
            Sym::Char('\u{7f}') => self.erase_ahead(),
            Sym::Char(c) if c.is_control() => Step::Nothing,
            Sym::Char(c) => {
                self.insert(c.encode_utf8(&mut [0; 4]));
                Step::Changed
            }
            Sym::Left => self.left(),
            Sym::Right => self.right(),
            Sym::Up => self.vertical(width, false),
            Sym::Down => self.vertical(width, true),
            Sym::Home => self.caret_to(0),
            Sym::End => self.caret_to(self.lines[self.caret.line].len()),
            Sym::Tab | Sym::PageUp | Sym::PageDown => Step::Nothing,
        }
    }

    /// What was copied, typed in at the caret — its lines kept, a `\r\n`
    /// read as one break, and every other control character left out.
    pub fn paste(&mut self, text: &str) -> Step {
        if self.asking.is_some() {
            return Step::Nothing;
        }
        let text = normalised(text);
        if text.is_empty() {
            return Step::Nothing;
        }
        self.goal = None;
        self.insert(&text);
        Step::Changed
    }

    /// A click, read back by [`hit`]: the caret placed at the cluster the
    /// cell is on (past a line's end, at its end) — or, on an answer of the
    /// question, the FIRST CLICK CHOOSES it and the second, on the answer
    /// already chosen, gives it (invariant 19): one click on "yes" never
    /// throws a text away.
    pub fn clicked(&mut self, hit: Hit) -> Step {
        match (hit, self.asking) {
            (Hit::Answer(answer), Some(on)) if answer != on => {
                self.asking = Some(answer);
                Step::Moved
            }
            (Hit::Answer(true), Some(_)) => Step::Discard,
            (Hit::Answer(false), Some(_)) => {
                self.asking = None;
                Step::Moved
            }
            (Hit::Text { at, cell }, None) => {
                // A PIECE THAT IS NO LONGER THERE — the text changed between
                // the frame and the click — places nothing.
                let fits = self.lines.get(at.line).is_some_and(|text| {
                    at.from <= at.to
                        && at.to <= text.len()
                        && text.is_char_boundary(at.from)
                        && text.is_char_boundary(at.to)
                });
                if !fits {
                    return Step::Nothing;
                }
                self.goal = None;
                self.caret = Caret {
                    line: at.line,
                    byte: self.byte_at(at, cell),
                };
                Step::Moved
            }
            _ => Step::Nothing,
        }
    }

    /// `Esc`: leave at once when nothing changed; else ask, "no" first.
    fn leave(&mut self) -> Step {
        if self.changed() {
            self.asking = Some(false);
            Step::Asking
        } else {
            Step::Discard
        }
    }

    /// A key while the question is up: the arrows and `TAB` move between its
    /// answers, `RET` gives the one the cursor is on, `Esc` is "no".
    fn answer_key(&mut self, key: Key, yes: bool) -> Step {
        if key.mods.ctrl {
            return if key.sym == Sym::Char('g') {
                self.asking = None;
                Step::Moved
            } else {
                Step::Nothing
            };
        }
        match key.sym {
            Sym::Left | Sym::Right | Sym::Up | Sym::Down | Sym::Tab => {
                self.asking = Some(!yes);
                Step::Moved
            }
            Sym::Enter if yes => Step::Discard,
            Sym::Enter | Sym::Esc => {
                self.asking = None;
                Step::Moved
            }
            _ => Step::Nothing,
        }
    }

    fn caret_to(&mut self, byte: usize) -> Step {
        if self.caret.byte == byte {
            return Step::Nothing;
        }
        self.caret.byte = byte;
        Step::Moved
    }

    /// `text` typed at the caret, which goes after it; `\n` breaks lines.
    fn insert(&mut self, text: &str) {
        let Caret { line, byte } = self.caret;
        let tail = self.lines[line].split_off(byte);
        let mut pieces = text.split('\n');
        self.lines[line].push_str(pieces.next().unwrap_or_default());
        let mut at = line;
        for piece in pieces {
            at += 1;
            self.lines.insert(at, piece.to_owned());
        }
        let byte = self.lines[at].len();
        self.lines[at].push_str(&tail);
        self.caret = Caret { line: at, byte };
    }

    fn erase_back(&mut self) -> Step {
        let Caret { line, byte } = self.caret;
        if byte > 0 {
            let before = last_cluster_start(&self.lines[line][..byte]);
            self.lines[line].replace_range(before..byte, "");
            self.caret.byte = before;
            return Step::Changed;
        }
        if line == 0 {
            return Step::Nothing;
        }
        let this = self.lines.remove(line);
        let above = &mut self.lines[line - 1];
        let byte = above.len();
        above.push_str(&this);
        self.caret = Caret {
            line: line - 1,
            byte,
        };
        Step::Changed
    }

    fn erase_ahead(&mut self) -> Step {
        let Caret { line, byte } = self.caret;
        let text = &self.lines[line];
        if byte < text.len() {
            let after = byte + first_cluster_len(&text[byte..]);
            self.lines[line].replace_range(byte..after, "");
            return Step::Changed;
        }
        if line + 1 >= self.lines.len() {
            return Step::Nothing;
        }
        let below = self.lines.remove(line + 1);
        self.lines[line].push_str(&below);
        Step::Changed
    }

    fn left(&mut self) -> Step {
        let Caret { line, byte } = self.caret;
        if byte > 0 {
            self.caret.byte = last_cluster_start(&self.lines[line][..byte]);
        } else if line > 0 {
            self.caret = Caret {
                line: line - 1,
                byte: self.lines[line - 1].len(),
            };
        } else {
            return Step::Nothing;
        }
        Step::Moved
    }

    fn right(&mut self) -> Step {
        let Caret { line, byte } = self.caret;
        let text = &self.lines[line];
        if byte < text.len() {
            self.caret.byte = byte + first_cluster_len(&text[byte..]);
        } else if line + 1 < self.lines.len() {
            self.caret = Caret {
                line: line + 1,
                byte: 0,
            };
        } else {
            return Step::Nothing;
        }
        Step::Moved
    }

    /// `↑` or `↓`: the drawn line above or below, at the column kept.
    fn vertical(&mut self, width: u16, down: bool) -> Step {
        let (n, cell) = self.caret_drawn(width);
        let goal = *self.goal.get_or_insert(cell);
        let drawn = self.drawn(width);
        let to = if down {
            n + 1
        } else {
            let Some(up) = n.checked_sub(1) else {
                return Step::Nothing;
            };
            up
        };
        let Some(d) = drawn.get(to).copied() else {
            return Step::Nothing;
        };
        self.caret = Caret {
            line: d.line,
            byte: self.byte_at(d, goal),
        };
        Step::Moved
    }

    /// The byte of drawn line `d` a caret at `cell` stands before: the
    /// cluster the cell is on, or the line's end past it — never the end of
    /// a piece that is not the last, which is the next piece's start.
    fn byte_at(&self, d: Drawn, cell: u16) -> usize {
        let text = &self.lines[d.line][d.from..d.to];
        let mut used = 0u16;
        let mut at = 0;
        let mut last_start = 0;
        for cluster in ronoctua_render::layout::cells_of(text) {
            let cells = ronoctua_protocol::cells_of_one(cluster);
            if used + cells > cell {
                return d.from + at;
            }
            last_start = at;
            used += cells;
            at += cluster.len();
        }
        if d.last {
            d.from + at
        } else {
            d.from + last_start
        }
    }
}

/// Text as the area takes it: `\r\n` and a lone `\r` are one break, and every
/// other control character but the break is left out — a paste from a
/// terminal brings a tab or an escape nobody can see.
fn normalised(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|&c| c == '\n' || !c.is_control())
        .collect()
}

/// Where the last cluster of `text` begins.
fn last_cluster_start(text: &str) -> usize {
    ronoctua_render::layout::cells_of(text)
        .last()
        .map_or(0, |last| text.len() - last.len())
}

/// How long the first cluster of `text` is, in bytes.
fn first_cluster_len(text: &str) -> usize {
    ronoctua_render::layout::cells_of(text)
        .next()
        .map_or(0, str::len)
}

/// The spot of a drawn line: the piece of which written line it is, so a
/// click reads back exactly what was drawn ([`hit`]).
fn line_spot(d: Drawn) -> String {
    format!(
        "{LINE_SPOT}{}-{}-{}-{}",
        d.line,
        d.from,
        d.to,
        u8::from(d.last)
    )
}

/// A drawn line's spot, read back ([`line_spot`]).
fn drawn_of(spot: &str) -> Option<Drawn> {
    let mut parts = spot.strip_prefix(LINE_SPOT)?.split('-');
    let mut next = || parts.next()?.parse::<usize>().ok();
    let (line, from, to, last) = (next()?, next()?, next()?, next()?);
    Some(Drawn {
        line,
        from,
        to,
        last: last == 1,
    })
}

/// What a click on row `n`, at `column` (a field's `at`), landed on — a
/// line of a text or an answer of its question; `None` when the row is not
/// one of a text's. The cells a door clicks are the ones [`Builder::text_area`]
/// drew, read back from the row.
#[must_use]
pub fn hit(rows: &[Row], n: usize, column: u16) -> Option<Hit> {
    let row = rows.get(n)?;
    match row.spot.as_str() {
        NO_SPOT => Some(Hit::Answer(false)),
        YES_SPOT => Some(Hit::Answer(true)),
        spot => {
            let at = drawn_of(spot)?;
            let left = row.fields.first().map_or(0, |f| f.at);
            Some(Hit::Text {
                at,
                cell: column.saturating_sub(left),
            })
        }
    }
}

/// The hint while a text is edited: its keys, clickable.
#[must_use]
pub fn hint(texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
    crate::chrome::hint_of(OF_THE_TEXT_AREA, texts)
}

impl Builder<'_> {
    /// A text being edited, as rows: each drawn line, its words in the tone
    /// of a value that is changed here, the caret on its line — and, while
    /// `Esc` asks, the question under it, "no" first and the cursor on the
    /// answer the area's own cursor is on. Every line is a row a click is
    /// read back from ([`hit`]).
    pub fn text_area(&mut self, area: &TextArea) {
        if let Some(title) = &area.title {
            self.heading_said(title.clone());
        }
        let (caret_at, caret_cell) = area.caret_drawn(self.room);
        let value = crate::chrome::Kind::Setting.tones(false).1;
        let mut cursor_row = self.out.len();
        for (n, d) in area.drawn(self.room).into_iter().enumerate() {
            let text = area.lines[d.line][d.from..d.to].trim_end_matches(' ');
            if n == caret_at {
                cursor_row = self.out.len();
            }
            self.out.push(Row {
                fields: vec![Self::piece(self.left, text.to_owned(), value)],
                focused: false,
                heading: false,
                target: Target::Nothing,
                spot: line_spot(d),
                cells: Vec::new(),
                slider: None,
                continues: false,
                chips: Vec::new(),
                deck: None,
                schematic: None,
                covers: Vec::new(),
                globe: None,
                caret: (n == caret_at && area.asking.is_none()).then_some(self.left + caret_cell),
                lit: crate::player_rows::Lit::Row,
            });
        }
        let Some(yes) = area.asking else {
            area.placed.set(Placed {
                width: self.room,
                row: cursor_row,
            });
            return;
        };
        self.inert(crate::chrome::AIR);
        let asked = self.t(crate::key::ASK_TEXT_AREA_DISCARD);
        for line in crate::chrome::note_lines(&asked, self.room) {
            self.inert(vec![Self::piece(
                self.left,
                line,
                tokens::color::TEXT_SECONDARY,
            )]);
        }
        // "NO" FIRST, and the cursor born on it (`TextArea::leave`).
        for (answer, key, spot) in [
            (false, crate::key::TEXT_AREA_NO, NO_SPOT),
            (true, crate::key::TEXT_AREA_YES, YES_SPOT),
        ] {
            let label = crate::chrome::Kind::Setting.tones(answer == yes).0;
            if answer == yes {
                cursor_row = self.out.len();
            }
            self.out.push(Row {
                fields: vec![Self::piece(self.left, self.t(key), label)],
                focused: answer == yes,
                heading: false,
                target: Target::Nothing,
                spot: spot.to_owned(),
                cells: Vec::new(),
                slider: None,
                continues: false,
                chips: Vec::new(),
                deck: None,
                schematic: None,
                covers: Vec::new(),
                globe: None,
                caret: None,
                lit: crate::player_rows::Lit::Row,
            });
        }
        area.placed.set(Placed {
            width: self.room,
            row: cursor_row,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(notation: &str) -> Key {
        Key::parse(notation).expect(notation)
    }

    fn typed(area: &mut TextArea, text: &str) {
        for c in text.chars() {
            area.key(
                Key::simple(if c == ' ' { Sym::Space } else { Sym::Char(c) }),
                40,
            );
        }
    }

    /// FAILS IF: typing, breaking a line, erasing across a seam, moving by
    /// cluster or by line, or keeping stop doing what a text box does.
    #[test]
    fn a_text_is_edited_as_a_text_box_edits() {
        let mut a = TextArea::new("lyrics", "first\nsecond");
        assert_eq!(a.caret(), Caret { line: 1, byte: 6 });
        assert!(!a.changed());
        assert_eq!(a.key(key("RET"), 40), Step::Changed);
        typed(&mut a, "três é");
        assert_eq!(a.text(), "first\nsecond\ntrês é");
        // `DEL` takes one CLUSTER, the accented letter whole.
        assert_eq!(a.key(key("DEL"), 40), Step::Changed);
        assert_eq!(a.text(), "first\nsecond\ntrês ");
        // HOME, then `DEL`: the seam goes, the lines join.
        assert_eq!(a.key(key("<home>"), 40), Step::Moved);
        assert_eq!(a.key(key("DEL"), 40), Step::Changed);
        assert_eq!(a.text(), "first\nsecondtrês ");
        assert_eq!(a.caret(), Caret { line: 1, byte: 6 });
        // THE FORWARD DELETE at a line's end joins the next.
        a.key(key("<up>"), 40);
        a.key(key("C-e"), 40);
        assert_eq!(a.caret(), Caret { line: 0, byte: 5 });
        assert_eq!(a.key(Key::simple(Sym::Char('\u{7f}')), 40), Step::Changed);
        assert_eq!(a.text(), "firstsecondtrês ");
        // LEFT AND RIGHT cross lines; `C-a` goes to the line's start.
        let mut b = TextArea::new("x", "ab\ncd");
        b.key(key("C-a"), 40);
        assert_eq!(b.key(key("<left>"), 40), Step::Moved);
        assert_eq!(b.caret(), Caret { line: 0, byte: 2 });
        assert_eq!(b.key(key("<right>"), 40), Step::Moved);
        assert_eq!(b.caret(), Caret { line: 1, byte: 0 });
        // `C-s` ASKS TO KEEP the text; "changed" is measured from it again
        // only once the view says it was written (`kept_as`).
        typed(&mut b, "z");
        assert!(b.changed());
        assert_eq!(b.key(key("C-s"), 40), Step::Keep("ab\nzcd".into()));
        assert!(b.changed(), "not kept until it is written");
        b.kept_as("ab\nzcd");
        assert!(!b.changed());
        assert_eq!(b.key(key("C-v"), 40), Step::WantsPaste);
    }

    /// FAILS IF: a keep the view could not write marks the text as kept —
    /// then `Esc` would throw the change away without asking.
    #[test]
    fn a_refused_keep_still_asks_before_throwing_away() {
        let mut a = TextArea::new("x", "as it was");
        typed(&mut a, "!");
        assert_eq!(a.key(key("C-s"), 40), Step::Keep("as it was!".into()));
        // THE WRITE WAS REFUSED: the view never calls `kept_as`.
        assert_eq!(
            a.key(key("ESC"), 40),
            Step::Asking,
            "the change is asked about"
        );
        assert_eq!(a.asking(), Some(false));
    }

    /// FAILS IF: the player's view — what the footer's region, the window
    /// and the column all draw and hand their keys to — stops giving an open
    /// text every key (a view's own letters included), the paste, the click
    /// and its hint line; marks a text kept that nothing wrote; or keeps its
    /// letters once the text is left.
    #[test]
    fn the_players_view_hands_an_open_text_everything() {
        use crate::player::{Errand, Press, View, press_of_key};
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut v = View::blank(None);
        assert!(!v.editing_text());
        v.edit_text(TextArea::new("x", "ab").titled("Lyrics".into()));
        let press = |v: &View, n: &str| v.press_of_key(key(n)).expect(n);
        // `a` IS THE VIEW'S "add" — and a letter of the text while it is open.
        assert_ne!(press_of_key(key("a")), Some(Press::Text(key("a"))));
        assert_eq!(press(&v, "a"), Press::Text(key("a")));
        assert_eq!(v.key(press(&v, "a"), None, 40, &t), Errand::Nothing);
        let rows = v.rows(None, 40, &t);
        assert!(rows[0].heading, "its title first");
        assert_eq!(rows[1].fields[0].text, "aba");
        assert_eq!(v.cursor(), 1, "the door keeps the caret's row in sight");
        // `C-v` IS THE DOOR'S PASTE, handed back.
        assert_eq!(v.key(press(&v, "C-v"), None, 40, &t), Errand::Paste);
        assert!(v.text_pasted("c\nd"));
        let rows = v.rows(None, 40, &t);
        assert_eq!(rows[2].fields[0].text, "d");
        // ITS HINT: `Esc` first, and the keep.
        let hint = v.hint(None, 40, &t);
        assert_eq!(hint, super::hint(&t));
        // A CLICK places the caret, and never runs what `RET` does.
        let left = rows[1].fields[0].at;
        assert!(!v.clicked_at(1, left, &rows));
        assert_eq!(v.rows(None, 40, &t)[1].caret, Some(left));
        // A KEEP NOTHING WRITES is said, and the text stays open, changed.
        assert!(matches!(
            v.key(press(&v, "C-s"), None, 40, &t),
            Errand::Said(_)
        ));
        assert!(v.editing_text());
        v.key(press(&v, "ESC"), None, 40, &t);
        assert!(v.editing_text(), "asked, not thrown away");
        assert!(v.rows(None, 40, &t).iter().any(|r| r.spot == NO_SPOT));
        v.key(press(&v, "<right>"), None, 40, &t);
        v.key(press(&v, "RET"), None, 40, &t);
        assert!(!v.editing_text(), "yes, given by the keys");
        assert!(!v.text_pasted("late"), "a late paste is nobody's");
        assert_eq!(v.press_of_key(key("a")), press_of_key(key("a")));
        // A KEEP THAT WAS WRITTEN: what keeps it says so, and `Esc` leaves
        // at once — nothing is lost.
        v.edit_text(TextArea::new("x", "ab"));
        v.key(press(&v, "z"), None, 40, &t);
        v.key(press(&v, "C-s"), None, 40, &t);
        v.text_kept_as("abz");
        v.key(press(&v, "ESC"), None, 40, &t);
        assert!(!v.editing_text(), "kept, so left without a question");
    }

    /// FAILS IF: a paste loses its lines, keeps a `\r` or a tab, or does not
    /// land at the caret.
    #[test]
    fn a_paste_brings_its_lines() {
        let mut a = TextArea::new("x", "ab");
        a.key(key("<left>"), 40);
        assert_eq!(a.paste("one\r\ntwo\tthree\rfour"), Step::Changed);
        assert_eq!(a.text(), "aone\ntwothree\nfourb");
        assert_eq!(a.caret(), Caret { line: 2, byte: 4 });
        assert_eq!(a.paste("\u{1b}"), Step::Nothing);
    }

    /// FAILS IF: a long line is not wrapped at the width, is cut inside a
    /// word where a space was, splits a two-cell ideograph, or `↓` does not
    /// walk the DRAWN lines keeping its column.
    #[test]
    fn a_long_line_wraps_and_the_arrows_walk_what_is_drawn() {
        let a = TextArea::new("x", "the quick brown fox");
        // Width 11: ten cells a line and one for the caret.
        let drawn: Vec<(usize, usize)> = a.drawn(11).iter().map(|d| (d.from, d.to)).collect();
        assert_eq!(
            drawn,
            [(0, 10), (10, 19)],
            "broken after the space, the word whole"
        );
        let pieces: Vec<&str> = a
            .drawn(11)
            .iter()
            .map(|d| &"the quick brown fox"[d.from..d.to])
            .collect();
        assert_eq!(pieces, ["the quick ", "brown fox"]);
        let wide = TextArea::new("x", "日本語日本語");
        for d in wide.drawn(6) {
            let piece = &"日本語日本語"[d.from..d.to];
            assert!(ronoctua_protocol::cells(piece) <= 5, "{piece}");
            assert!(!piece.is_empty());
        }
        // ↑ from the end of the second drawn line lands on the first, at the
        // same column; ↓ comes back and keeps it.
        let mut b = TextArea::new("x", "the quick brown fox\nab");
        let second = b.drawn(11)[1];
        assert_eq!(
            b.clicked(Hit::Text {
                at: second,
                cell: 4
            }),
            Step::Moved
        );
        assert_eq!(b.caret(), Caret { line: 0, byte: 14 });
        assert_eq!(b.key(key("<up>"), 11), Step::Moved);
        assert_eq!(b.caret(), Caret { line: 0, byte: 4 });
        b.key(key("<down>"), 11);
        b.key(key("<down>"), 11);
        assert_eq!(
            b.caret(),
            Caret { line: 1, byte: 2 },
            "a short line: its end"
        );
        b.key(key("<up>"), 11);
        assert_eq!(
            b.caret(),
            Caret { line: 0, byte: 14 },
            "the column was kept"
        );
        assert_eq!(b.caret_drawn(11), (1, 4));
    }

    /// FAILS IF: a click does not place the caret at the cluster under it —
    /// past a line's end, at its end — or the rows a door draws cannot be
    /// read back into the same place, in either door's arithmetic.
    #[test]
    fn a_click_places_the_caret_where_it_lands() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut a = TextArea::new("x", "hello\nwörld");
        let rows = Builder::build(2, 30, 0, 0, &t, |b| b.text_area(&a));
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[1].caret,
            Some(2 + 5),
            "the caret at the end of `wörld`"
        );
        let hit_at = hit(&rows, 1, 2 + 2).expect("a line");
        assert_eq!(
            hit_at,
            Hit::Text {
                at: a.drawn(30)[1],
                cell: 2
            },
            "the piece drawn, read back from its row"
        );
        assert_eq!(a.drawn_width(), 30, "the width it was placed at");
        assert_eq!(a.cursor_row(), 1, "the caret's row");
        assert_eq!(a.clicked(hit_at), Step::Moved);
        assert_eq!(
            a.caret(),
            Caret { line: 1, byte: 3 },
            "after the ö, before the r"
        );
        assert_eq!(
            a.clicked(hit(&rows, 0, 2 + 20).expect("a line")),
            Step::Moved
        );
        assert_eq!(
            a.caret(),
            Caret { line: 0, byte: 5 },
            "past the end: the end"
        );
        let rows = Builder::build(2, 30, 0, 0, &t, |b| b.text_area(&a));
        assert_eq!(rows[0].caret, Some(2 + 5));
        assert_eq!(rows[1].caret, None);
        assert_eq!(a.cursor_row(), 0);
        // A PIECE THAT IS GONE — the text changed after the frame — places
        // nothing, and never a caret in the middle of a letter.
        let stale = hit(&rows, 1, 2 + 3).expect("a line");
        let mut b = TextArea::new("x", "hi");
        assert_eq!(b.clicked(stale), Step::Nothing);
        assert_eq!(b.caret(), Caret { line: 0, byte: 2 });
    }

    /// FAILS IF: a key the text teaches cannot say what it does under the
    /// pointer, in either language, or the hint stops teaching `Esc` first.
    #[test]
    fn the_keys_it_teaches_explain_themselves() {
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect(language);
            assert!(!hint(&t).is_empty(), "{language}");
            assert_eq!(OF_THE_TEXT_AREA[0].0, "ESC");
            for (_, id) in OF_THE_TEXT_AREA {
                assert!(
                    crate::chrome::hint_sentence(id, &t).is_some(),
                    "{language}: {id} explains nothing"
                );
            }
            assert!(t.has(crate::key::ABOUT_TEXT_AREA), "{language}");
        }
    }

    /// FAILS IF: `Esc` throws a change away without asking, asks when there
    /// is nothing to lose, draws "yes" first or born under the cursor, or a
    /// "no" does not bring the text back as it was.
    #[test]
    fn esc_asks_before_a_change_is_thrown_away_no_first() {
        let t = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut quiet = TextArea::new("x", "as it was");
        assert_eq!(quiet.key(key("ESC"), 40), Step::Discard, "nothing to lose");
        let mut a = TextArea::new("x", "as it was");
        typed(&mut a, "!");
        assert_eq!(a.key(key("ESC"), 40), Step::Asking);
        assert_eq!(a.asking(), Some(false), "the cursor is born on no");
        let rows = Builder::build(0, 40, 0, 0, &t, |b| b.text_area(&a));
        let answers: Vec<&str> = rows
            .iter()
            .filter(|r| r.spot == NO_SPOT || r.spot == YES_SPOT)
            .map(|r| r.spot.as_str())
            .collect();
        assert_eq!(answers, [NO_SPOT, YES_SPOT], "no is drawn first");
        assert!(
            rows.iter().all(|r| r.caret.is_none()),
            "no caret while asking"
        );
        // A LETTER is no answer; RET on "no" goes back to the text.
        assert_eq!(a.key(key("y"), 40), Step::Nothing);
        assert_eq!(a.key(key("RET"), 40), Step::Moved);
        assert_eq!(a.asking(), None);
        assert_eq!(a.text(), "as it was!");
        // AND YES, by the keys.
        a.key(key("ESC"), 40);
        a.key(key("<right>"), 40);
        assert_eq!(a.key(key("RET"), 40), Step::Discard);
        // AND BY THE POINTER: the first click on "yes" CHOOSES it, and only
        // the second throws the text away (invariant 19).
        let mut b = TextArea::new("x", "as it was");
        typed(&mut b, "!");
        b.key(key("ESC"), 40);
        let rows = Builder::build(0, 40, 0, 0, &t, |bu| bu.text_area(&b));
        let yes = rows.iter().position(|r| r.spot == YES_SPOT).expect("yes");
        let no = rows.iter().position(|r| r.spot == NO_SPOT).expect("no");
        assert_eq!(b.cursor_row(), no, "the door keeps the answer in sight");
        let on_yes = hit(&rows, yes, 0).expect("yes");
        assert_eq!(b.clicked(on_yes), Step::Moved, "one click chooses");
        assert_eq!(b.asking(), Some(true));
        assert_eq!(b.text(), "as it was!", "nothing thrown away yet");
        // A CLICK ON "NO" while "yes" is chosen chooses "no" — and a second
        // gives the text back.
        let on_no = hit(&rows, no, 0).expect("no");
        assert_eq!(b.clicked(on_no), Step::Moved);
        assert_eq!(b.asking(), Some(false));
        assert_eq!(b.clicked(on_no), Step::Moved);
        assert_eq!(b.asking(), None, "back to the text");
        b.key(key("ESC"), 40);
        assert_eq!(b.clicked(on_yes), Step::Moved);
        assert_eq!(
            b.clicked(on_yes),
            Step::Discard,
            "the second click gives it"
        );
    }
}
