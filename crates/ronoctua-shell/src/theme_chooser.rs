//! `LEADER s t` — the themes, as a list in the footer, and beside them the
//! choices ABOUT a theme.
//!
//! # What it does, and what it deliberately does not
//!
//! It is the network list's shape: a cursor over a few rows, `RET` on the row,
//! `Esc` out. What differs is that MOVING THE CURSOR PREVIEWS: the theme under
//! it becomes the process's the moment the cursor lands, so the footer — the
//! surface this list is drawn on — is the swatch. There is no swatch drawn in
//! a theme's own colours beside its id, and there cannot be one: a `Color` is
//! a role that resolves through the CURRENT theme, and a `Color` bound to
//! another theme is exactly the thing the tokens crate has no way to express.
//! The row is the id alone, an identifier as the `M-x` list shows command ids;
//! a name in the person's language was a second spelling of the same thing.
//!
//! The preview is the footer's alone. The other doors — the file manager's
//! window, a buffer's host — follow the FILE, and the file is written by
//! `theme-apply`, not by the cursor: a person browsing themes should not have
//! every window on the screen flicker through them. `Esc` puts the committed
//! theme back.
//!
//! # The second column
//!
//! Beside the themes, the choices about a theme — one, today: the desk the
//! compositor paints where no window is, black or the theme's own colour
//! ([`Backdrop`]). It is a list too, with its own cursor: `<left>`/`<right>`
//! cross between the columns, `<up>`/`<down>` walk the one that has the
//! focus, and the header row of the second is not a row the cursor lands on.
//! Its preview cannot be the footer's — the footer never paints the desk — so
//! it is the compositor's: the fragment is written when the cursor moves
//! (`theme::show_to_compositor`), and written again with the committed pair
//! on `Esc`. The theme column keeps its preview to the footer, as before;
//! `RET` applies the PAIR under the two cursors.
//!
//! # Why the state is three numbers and a pair
//!
//! Two cursors and which column has the focus are where the person is;
//! `committed` is what the file says. They differ while a preview is on, and
//! `Esc` is "back to committed". An external write to the file during a
//! preview moves `committed` (the door's `theme_changed` says so): what is on
//! disk wins over what is being tried.

use ronoctua_tokens::theme::{self, Theme};

use crate::theme::{Backdrop, Choice};

/// Which of the two lists has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// The themes, on the left.
    Themes,
    /// The desk under the windows, on the right.
    Desk,
}

/// What a row of the second column is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeskRow {
    /// The option's name, over its answers. Not a place for the cursor.
    Header,
    /// One answer.
    Answer(Backdrop),
    /// Nothing: the left list is longer than the right one here.
    Empty,
}

/// What moving the cursor changed, so the door previews the right thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// The theme under the cursor: the footer previews it.
    Theme(Theme),
    /// The desk under the cursor: the compositor previews it, with the theme
    /// under the other cursor.
    Desk(Choice),
}

/// The chooser's state while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// Which list has the keyboard.
    column: Column,
    /// The focused theme row: an index into [`theme::all`].
    cursor: usize,
    /// The focused answer: an index into [`Backdrop::ALL`].
    desk_cursor: usize,
    /// What the file says — what `Esc` returns to.
    committed: Choice,
}

impl View {
    /// The rows of the second column: the header and one per answer.
    pub const DESK_ROWS: usize = 1 + Backdrop::ALL.len();

    /// Opens on the theme the process is in and the desk the file says, which
    /// together are the committed pair. The keyboard starts on the themes.
    #[must_use]
    pub fn fresh_one(backdrop: Backdrop) -> Self {
        let now = theme::current();
        Self {
            column: Column::Themes,
            cursor: now.position(),
            desk_cursor: backdrop.position(),
            committed: Choice {
                theme: now,
                backdrop,
            },
        }
    }

    /// How many themes there are.
    #[must_use]
    pub fn theme_count() -> usize {
        theme::all().len()
    }

    /// How many rows the region shows: the longer of the two lists.
    #[must_use]
    pub fn rows() -> usize {
        Self::theme_count().max(Self::DESK_ROWS)
    }

    /// What row `i` of the second column is.
    #[must_use]
    pub fn desk_row(i: usize) -> DeskRow {
        match i {
            0 => DeskRow::Header,
            n => Backdrop::ALL
                .get(n - 1)
                .map_or(DeskRow::Empty, |b| DeskRow::Answer(*b)),
        }
    }

    /// Which list has the keyboard.
    #[must_use]
    pub const fn column(&self) -> Column {
        self.column
    }

    /// The focused theme row.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The row on screen the focused cursor is on — what the region scrolls
    /// to. The desk's answers start under their header.
    #[must_use]
    pub const fn row(&self) -> usize {
        match self.column {
            Column::Themes => self.cursor,
            Column::Desk => 1 + self.desk_cursor,
        }
    }

    /// The theme under its cursor — what the footer previews and `RET` applies.
    #[must_use]
    pub fn focused(&self) -> Theme {
        theme::all()
            .nth(self.cursor)
            .unwrap_or_else(theme::default_one)
    }

    /// The desk under its cursor.
    #[must_use]
    pub fn focused_backdrop(&self) -> Backdrop {
        Backdrop::ALL
            .get(self.desk_cursor)
            .copied()
            .unwrap_or_default()
    }

    /// The pair under the two cursors — what `RET` writes.
    #[must_use]
    pub fn focused_choice(&self) -> Choice {
        Choice {
            theme: self.focused(),
            backdrop: self.focused_backdrop(),
        }
    }

    /// The pair the file says.
    #[must_use]
    pub const fn committed(&self) -> Choice {
        self.committed
    }

    /// Moves the focused list's cursor without leaving the ends, and says what
    /// is under it now. It does not wrap, for the reason the network list
    /// gives: going back to the start after the end makes the eye lose its
    /// place.
    pub fn move_by(&mut self, step: i32) -> Moved {
        match self.column {
            Column::Themes => {
                self.cursor = clamped(self.cursor, step, Self::theme_count());
                Moved::Theme(self.focused())
            }
            Column::Desk => {
                self.desk_cursor = clamped(self.desk_cursor, step, Backdrop::ALL.len());
                Moved::Desk(self.focused_choice())
            }
        }
    }

    /// Crosses to the other list — `<left>` and `<right>` — and says which
    /// has the keyboard now. Two lists, so either arrow reaches the other;
    /// at an end the arrow that points outward does nothing, as in the file
    /// manager's sections.
    pub fn cross(&mut self, step: i32) -> Column {
        self.column = match (self.column, step.signum()) {
            (Column::Themes, 1) => Column::Desk,
            (Column::Desk, -1) => Column::Themes,
            (same, _) => same,
        };
        self.column
    }

    /// Puts the focus on a theme row — what the pointer does on the left
    /// column — and says what is under it.
    pub fn focus_theme(&mut self, i: usize) -> Theme {
        self.column = Column::Themes;
        self.cursor = i.min(Self::theme_count().saturating_sub(1));
        self.focused()
    }

    /// Puts the focus on a row of the right column — what the pointer does
    /// there. `None` on the header or on nothing: the cursor did not move and
    /// there is nothing to preview.
    pub fn focus_desk_row(&mut self, row: usize) -> Option<Choice> {
        match Self::desk_row(row) {
            DeskRow::Answer(b) => {
                self.column = Column::Desk;
                self.desk_cursor = b.position();
                Some(self.focused_choice())
            }
            DeskRow::Header | DeskRow::Empty => None,
        }
    }

    /// The pair under the cursors becomes the committed one: `theme-apply`
    /// ran and the file is written.
    pub fn commit(&mut self) {
        self.committed = self.focused_choice();
    }

    /// The file changed under the chooser: what is on disk is now the thing
    /// `Esc` returns to.
    pub fn committed_became(&mut self, c: Choice) {
        self.committed = c;
    }

    /// The hint line: the key that applies, the arrows that cross between the
    /// columns and the key that goes back, in the localiser's sentence, with
    /// the keys marked so the chrome colours them.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        use ronoctua_keymap::{Key, Sym};
        let apply = key_that_runs("theme-apply").map_or_else(String::new, |k| k.notation());
        let left = Key::simple(Sym::Left).notation();
        let right = Key::simple(Sym::Right).notation();
        let close = Key::simple(Sym::Esc).notation();
        // ONE ARGUMENT PER KEY: the chrome marks a piece as a key only when the
        // whole piece is a notation, so `<left>/<right>` as one argument came
        // out in the prose register. The slash is the sentence's.
        let marked = texts.t_marked(
            ronoctua_i18n::key::THEME_HINT,
            &[
                ("apply", apply.as_str()),
                ("left", left.as_str()),
                ("right", right.as_str()),
                ("close", close.as_str()),
            ],
        );
        let mut out = Vec::new();
        crate::chrome::Piece::extend(
            &mut out,
            &marked,
            &[
                apply.as_str(),
                left.as_str(),
                right.as_str(),
                close.as_str(),
            ],
        );
        out
    }
}

/// `at + step`, held inside `0..len`.
fn clamped(at: usize, step: i32, len: usize) -> usize {
    let last = i64::try_from(len.saturating_sub(1)).unwrap_or(0);
    let fresh = i64::try_from(at).unwrap_or(0) + i64::from(step);
    usize::try_from(fresh.clamp(0, last)).unwrap_or(0)
}

/// The KEY that runs a given id, read out of the view's own table — the
/// reason `files::key_that_runs` gives: the hint and the mouse read the table
/// rather than writing a key out by hand.
#[must_use]
pub fn key_that_runs(id: &str) -> Option<ronoctua_keymap::Key> {
    crate::leader::OF_THEME_VIEW
        .iter()
        .find(|(_, command)| *command == id)
        .and_then(|(notation, _)| ronoctua_keymap::Key::parse(notation).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_opens_on_the_current_pair_and_the_cursors_stay_inside() {
        let v = View::fresh_one(Backdrop::Black);
        assert_eq!(v.focused(), theme::current());
        assert_eq!(v.focused_backdrop(), Backdrop::Black);
        assert_eq!(v.committed(), v.focused_choice());
        assert_eq!(v.column(), Column::Themes);
        let mut v = v;
        for _ in 0..10 {
            v.move_by(1);
        }
        assert_eq!(v.cursor(), View::theme_count() - 1);
        for _ in 0..20 {
            v.move_by(-1);
        }
        assert_eq!(v.cursor(), 0);
        assert_eq!(
            v.focus_theme(usize::MAX).position(),
            View::theme_count() - 1
        );
        // The other list, the same way.
        assert_eq!(v.cross(1), Column::Desk);
        assert_eq!(v.cross(1), Column::Desk, "no third column");
        for _ in 0..10 {
            v.move_by(1);
        }
        assert_eq!(v.focused_backdrop().position(), Backdrop::ALL.len() - 1);
        assert_eq!(
            v.row(),
            Backdrop::ALL.len(),
            "the answers start under the header"
        );
        for _ in 0..10 {
            v.move_by(-1);
        }
        assert_eq!(v.focused_backdrop().position(), 0);
        assert_eq!(v.cross(-1), Column::Themes);
        assert_eq!(v.cross(-1), Column::Themes);
        assert_eq!(v.row(), v.cursor());
    }

    #[test]
    fn the_rows_cover_both_lists_and_the_header_takes_no_cursor() {
        assert!(View::rows() >= View::theme_count());
        assert!(View::rows() >= View::DESK_ROWS);
        assert_eq!(View::desk_row(0), DeskRow::Header);
        for (i, b) in Backdrop::ALL.iter().enumerate() {
            assert_eq!(View::desk_row(i + 1), DeskRow::Answer(*b));
        }
        assert_eq!(View::desk_row(View::DESK_ROWS), DeskRow::Empty);

        let mut v = View::fresh_one(Backdrop::Black);
        assert_eq!(v.focus_desk_row(0), None, "the header is not a row");
        assert_eq!(v.column(), Column::Themes, "and the focus did not move");
        let shown = v.focus_desk_row(1 + Backdrop::Theme.position());
        assert_eq!(
            shown,
            Some(Choice {
                theme: theme::current(),
                backdrop: Backdrop::Theme
            })
        );
        assert_eq!(v.column(), Column::Desk);
        assert_eq!(v.focus_desk_row(usize::MAX), None);
    }

    #[test]
    fn commit_moves_the_committed_pair_and_nothing_else_does() {
        let mut v = View::fresh_one(Backdrop::Black);
        let before = v.committed();
        v.cross(1);
        let Moved::Desk(shown) = v.move_by(1) else {
            panic!("the desk column moves the desk");
        };
        assert_eq!(shown.backdrop, Backdrop::Theme);
        assert_eq!(v.committed(), before, "moving is a preview, not a choice");
        v.commit();
        assert_eq!(v.committed().backdrop, Backdrop::Theme);
        let other = Choice {
            theme: theme::default_one(),
            backdrop: Backdrop::Black,
        };
        v.committed_became(other);
        assert_eq!(v.committed(), other);
    }

    #[test]
    fn the_hint_names_the_keys_that_apply_cross_and_close() {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let hint = View::fresh_one(Backdrop::Black).hint(&texts);
        let joined: String = hint.iter().map(|p| p.text.as_str()).collect();
        assert!(joined.contains("RET"), "{joined}");
        assert!(joined.contains("<left>/<right>"), "{joined}");
        assert!(joined.contains("ESC"), "{joined}");
        // AND THE ARROWS ARE KEYS, not prose: each in the register `RET` is
        // in, and the words between them in another.
        let register_of = |text: &str| {
            hint.iter()
                .find(|p| p.text == text)
                .unwrap_or_else(|| panic!("no piece `{text}` in {joined}"))
                .register
        };
        assert_eq!(register_of("<left>"), register_of("RET"));
        assert_eq!(register_of("<right>"), register_of("RET"));
        assert_ne!(
            register_of("/"),
            register_of("RET"),
            "the slash is the sentence's"
        );
        assert_eq!(
            key_that_runs("theme-apply")
                .map(|k| k.notation())
                .as_deref(),
            Some("RET")
        );
        assert!(key_that_runs("theme-open").is_none());
    }

    #[test]
    fn every_answer_has_its_name_in_the_texts() {
        for b in Backdrop::ALL {
            assert!(
                ronoctua_i18n::ALL_KEYS.contains(&b.text_key()),
                "{} has no text key",
                b.text_key()
            );
        }
        assert!(ronoctua_i18n::ALL_KEYS.contains(&ronoctua_i18n::key::THEME_OPTION_BACKDROP));
    }
}
