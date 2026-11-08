//! The line a terminal's window says something on: over the grid's last
//! row, without a resize — the program keeps its size while the window
//! speaks. A message, a question, or what is being looked for
//! (`crate::terminal_search`, `C-S-f`).
//!
//! # Why over the grid, and in its cells
//!
//! A terminal has no header and no message line of its own: every row is the
//! program's. So the window's line is laid OVER the last row while it is up —
//! a message the door said (`Ui::say`), or a question — and the program's
//! row comes back when it goes. It is written into the grid's own cells
//! ([`lay_line`]): a line drawn as text over the cells would be a second
//! face at a second size crossing the program's glyphs, and the grid's cells
//! are exactly the row they cover.
//!
//! # A question that appears by itself draws its no first
//!
//! Neither of the two questions here was asked for by a click: a paste that
//! could run a command, a program asking to read the clipboard. Both are
//! born with the cursor on the NO (`crate::aim::Aim::born`), the keys walk
//! it, `Esc` is the no, and a click only picks — the second click on the
//! person's pick answers (invariant 19).

use ronoctua_render::cells::{Cell, Grapheme, Rgba, Sheet, Width};
use ronoctua_shell_kit::aim::Aim;
use ronoctua_shell_kit::chrome::{Piece, Register};
use ronoctua_terminals::wire;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::keyboard::Keysym;

/// What is asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// A paste that could run a command as it lands — its text, and where
    /// it came from (a clipboard, or a drop's paths), which the program is
    /// told as it lands.
    Paste(String, wire::PasteSource),
    /// A program asking to read a clipboard.
    ClipboardRead(wire::Consent),
}

/// A question on the line, and the answer its cursor is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// What is asked.
    pub asked: Asked,
    aim: Aim<bool>,
}

/// What a key or a click did to a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Nothing the window has to do but draw again.
    Moved,
    /// It is answered: yes or no.
    Answered(bool),
    /// The key was not the question's — and still taken: a key meant for a
    /// question never reaches the program under it.
    Kept,
}

impl Question {
    /// A question, its cursor born on the no.
    #[must_use]
    pub const fn new(asked: Asked) -> Self {
        Self {
            asked,
            aim: Aim::born(false),
        }
    }

    /// The answer the cursor is on: `true` is yes.
    #[must_use]
    pub fn on(&self) -> bool {
        self.aim.on().copied().unwrap_or(false)
    }

    /// A key, while the question is up: the arrows and `TAB` walk the two
    /// answers, `RET` gives the one the cursor is on, `Esc` is the no.
    pub fn key(&mut self, sym: Keysym) -> Step {
        match sym {
            Keysym::Left
            | Keysym::Right
            | Keysym::Up
            | Keysym::Down
            | Keysym::Tab
            | Keysym::ISO_Left_Tab => {
                self.aim.moved(!self.on());
                Step::Moved
            }
            Keysym::Return | Keysym::KP_Enter => Step::Answered(self.on()),
            Keysym::Escape => Step::Answered(false),
            _ => Step::Kept,
        }
    }

    /// A click on an answer: it answers only when it is the person's own
    /// pick clicked again.
    pub fn clicked(&mut self, yes: bool) -> Step {
        if self.aim.clicked(yes) {
            Step::Answered(yes)
        } else {
            Step::Moved
        }
    }

    /// The line's runs: the sentence, then the no and the yes — the one the
    /// cursor is on in the cursor's ink, the one under the pointer lit.
    #[must_use]
    pub fn runs(&self, texts: &ronoctua_i18n::Texts, lit: Option<Click>) -> Vec<Run> {
        use crate::key;
        let (said, no, yes) = match &self.asked {
            Asked::Paste(..) => (
                texts.t(key::TERMINAL_PASTE_UNSAFE),
                texts.t(key::TERMINAL_PASTE_REFUSE),
                texts.t(key::TERMINAL_PASTE_ALLOW),
            ),
            Asked::ClipboardRead(c) => (
                if c.program.trim().is_empty() {
                    texts.t(key::TERMINAL_CLIPBOARD_READ_UNNAMED)
                } else {
                    texts.t_with(key::TERMINAL_CLIPBOARD_READ, &[("program", &c.program)])
                },
                texts.t(key::TERMINAL_CLIPBOARD_REFUSE),
                texts.t(key::TERMINAL_CLIPBOARD_ALLOW),
            ),
        };
        let answer = |label: String, which: bool| {
            let register = if self.on() == which {
                Register::ActionHere
            } else if lit == Some(Click::Answer(which)) {
                Register::ActionLit
            } else {
                Register::Action
            };
            Run {
                text: label,
                ink: register.color(),
                does: Some(Click::Answer(which)),
            }
        };
        let separator = || Run::of(&Piece::separator());
        vec![
            Run::of(&Piece::asked(said)),
            separator(),
            answer(no, false),
            separator(),
            answer(yes, true),
        ]
    }
}

/// WHAT A CLICK ON A RUN OF THE LINE DOES — every key the line takes has
/// its run a click takes too (invariant 19): a question's two answers, and
/// the search's walk and its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// A question's answer: `true` is yes.
    Answer(bool),
    /// The search's next match toward older output (`RET`).
    Older,
    /// The search's next match toward newer output (`S-RET`).
    Newer,
    /// The search ends (`Esc`).
    Close,
}

/// One run of the line: its text, its ink, and what a click on it does,
/// when it does something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The text.
    pub text: String,
    /// Its ink — a role of the chrome, which the line's ground is.
    pub ink: tokens::Color,
    /// What a click on it does.
    pub does: Option<Click>,
}

impl Run {
    /// A hint line's piece, in its register's ink.
    #[must_use]
    pub fn of(piece: &Piece) -> Self {
        Self {
            text: piece.text.clone(),
            ink: piece.register.color(),
            does: None,
        }
    }

    /// A run a click takes: `label`, in the action's ink — lit when the
    /// pointer rests on it.
    #[must_use]
    pub fn action(label: String, does: Click, lit: Option<Click>) -> Self {
        let register = if lit == Some(does) {
            Register::ActionLit
        } else {
            Register::Action
        };
        Self {
            text: label,
            ink: register.color(),
            does: Some(does),
        }
    }
}

/// The message the door said, as the line's one run.
#[must_use]
pub fn said(text: &str) -> Vec<Run> {
    vec![Run::of(&Piece::said(text))]
}

/// The runs laid out across `columns` cells: each run's columns, the first
/// run — the sentence — cut at its end when the line is too narrow for the
/// whole of it, so the answers are never the ones that go.
#[must_use]
pub fn laid(runs: &[Run], columns: u16) -> Vec<(u16, Run)> {
    use ronoctua_render::layout::{elide_end, width};
    let rest: u16 = runs.iter().skip(1).map(|r| width(&r.text)).sum();
    let mut out = Vec::with_capacity(runs.len());
    let mut at = 0u16;
    for (n, run) in runs.iter().enumerate() {
        let room = columns.saturating_sub(at);
        let text = if n == 0 {
            elide_end(&run.text, columns.saturating_sub(rest).max(1).min(room))
        } else {
            elide_end(&run.text, room)
        };
        let wide = width(&text);
        if wide == 0 {
            continue;
        }
        out.push((
            at,
            Run {
                text,
                ..run.clone()
            },
        ));
        at = at.saturating_add(wide);
    }
    out
}

/// What a click at `column` of the line does, if anything.
#[must_use]
pub fn click_at(laid: &[(u16, Run)], column: u16) -> Option<Click> {
    use ronoctua_render::layout::width;
    laid.iter()
        .find(|(at, run)| run.does.is_some() && (*at..*at + width(&run.text)).contains(&column))
        .and_then(|(_, run)| run.does)
}

/// Writes `laid` into line `line` of `sheet`, on `ground` from edge to edge,
/// a grapheme a cell — a wide one two. Only when the line would change: an
/// unchanged line keeps its generation, and the GPU rebuilds nothing.
pub fn lay_line(sheet: &mut Sheet, line: u16, laid: &[(u16, Run)], ground: Rgba) {
    let columns = usize::from(sheet.columns());
    let blank = Cell {
        fg: Rgba::of(Register::Said.color()),
        bg: Some(ground),
        ..Cell::default()
    };
    let mut cells = vec![blank; columns];
    for (at, run) in laid {
        let mut x = usize::from(*at);
        let fg = Rgba::of(run.ink);
        for g in ronoctua_protocol::cells_of(&run.text) {
            let wide = ronoctua_protocol::cells_of_one(g) == 2;
            if x + usize::from(wide) >= columns {
                break;
            }
            cells[x] = Cell {
                grapheme: Grapheme::of(g),
                fg,
                bg: Some(ground),
                width: if wide { Width::Wide } else { Width::Narrow },
                ..Cell::default()
            };
            if wide {
                cells[x + 1] = Cell {
                    width: Width::Spacer,
                    ..cells[x].clone()
                };
            }
            x += 1 + usize::from(wide);
        }
    }
    if sheet.row(line) != cells.as_slice() {
        sheet.row_mut(line).clone_from_slice(&cells);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn consent(program: &str) -> wire::Consent {
        wire::Consent {
            id: 9,
            clipboard: wire::ClipboardKind::Standard,
            mimes: vec!["text/plain".to_owned()],
            program: program.to_owned(),
        }
    }

    #[test]
    fn a_question_is_born_on_its_no_and_a_click_only_picks() {
        // FAILS IF the question's cursor is born on the yes (a RET meant for
        // the program would paste what could run a command), or one click
        // answers — the second press of a double click meant for the window.
        let mut q = Question::new(Asked::Paste(
            "rm -rf ~\n".to_owned(),
            wire::PasteSource::Clipboard,
        ));
        assert!(!q.on(), "born on the no");
        assert_eq!(q.key(Keysym::Return), Step::Answered(false));
        assert_eq!(q.clicked(true), Step::Moved, "one click picks");
        assert_eq!(q.clicked(true), Step::Answered(true));
        let mut q = Question::new(Asked::ClipboardRead(consent("vim")));
        assert_eq!(
            q.clicked(false),
            Step::Moved,
            "the born cursor is not a pick"
        );
        assert_eq!(q.clicked(false), Step::Answered(false));
    }

    #[test]
    fn the_keys_walk_the_answers_esc_says_no_and_nothing_reaches_the_program() {
        // FAILS IF the arrows do not reach the yes, `Esc` answers anything
        // but no, or a letter typed at the question is not kept from the
        // program under it.
        let mut q = Question::new(Asked::Paste("x\n".to_owned(), wire::PasteSource::Clipboard));
        assert_eq!(q.key(Keysym::Right), Step::Moved);
        assert!(q.on());
        assert_eq!(q.key(Keysym::Return), Step::Answered(true));
        assert_eq!(q.key(Keysym::Escape), Step::Answered(false));
        assert_eq!(q.key(Keysym::a), Step::Kept);
    }

    #[test]
    fn the_line_draws_the_no_before_the_yes_and_keeps_the_answers_when_narrow() {
        // FAILS IF the yes is drawn first, a click on a drawn answer lands on
        // the other one, or a narrow window cuts an answer away instead of
        // the sentence.
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let q = Question::new(Asked::Paste("x\n".to_owned(), wire::PasteSource::Clipboard));
        let runs = q.runs(&texts, None);
        let wide = laid(&runs, 200);
        let no = wide
            .iter()
            .find(|(_, r)| r.does == Some(Click::Answer(false)))
            .unwrap()
            .0;
        let yes = wide
            .iter()
            .find(|(_, r)| r.does == Some(Click::Answer(true)))
            .unwrap()
            .0;
        assert!(no < yes, "the no is drawn first");
        assert_eq!(click_at(&wide, no), Some(Click::Answer(false)));
        assert_eq!(click_at(&wide, yes), Some(Click::Answer(true)));
        assert_eq!(click_at(&wide, 0), None, "the sentence answers nothing");
        let narrow = laid(&runs, 30);
        assert!(
            narrow
                .iter()
                .any(|(_, r)| r.does == Some(Click::Answer(false)))
        );
        assert!(
            narrow
                .iter()
                .any(|(_, r)| r.does == Some(Click::Answer(true)))
        );
        let end = narrow
            .iter()
            .map(|(at, r)| *at + ronoctua_render::layout::width(&r.text))
            .max()
            .unwrap();
        assert!(end <= 30, "the line ran to {end}");
    }

    #[test]
    fn the_program_is_named_when_it_named_itself() {
        // FAILS IF a program's own name is dropped from the question, or an
        // empty one is spelt as a name.
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let named = Question::new(Asked::ClipboardRead(consent("nvim")));
        assert!(named.runs(&texts, None)[0].text.contains("nvim"));
        let unnamed = Question::new(Asked::ClipboardRead(consent(" ")));
        assert_eq!(
            unnamed.runs(&texts, None)[0].text,
            texts.t(crate::key::TERMINAL_CLIPBOARD_READ_UNNAMED)
        );
    }
}
