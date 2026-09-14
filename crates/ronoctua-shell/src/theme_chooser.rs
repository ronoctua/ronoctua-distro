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
//! EITHER CURSOR PREVIEWS THE WHOLE PAIR, and it did not: moving in the theme
//! column repainted the footer alone, so a theme was shown worn by one strip
//! of the screen while every window, the desk and the terminals kept the old
//! one. The owner read that off the screen. A preview now writes the FILE
//! (`theme::show_everywhere`), which is the channel every other door already
//! watches, and `Esc` puts the file back — or takes it away, when there was
//! none. `RET` applies the PAIR under the two cursors, as before.
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

/// What row `i` of the FIRST column is — the themes' own list.
///
/// The mirror of [`DeskRow`], and it exists for the same reason: the row on
/// screen and the item's index in its list stopped being the same number the
/// day the column gained a title, and the two must not be confused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeRow {
    /// The list's name, over the themes. Not a place for the cursor.
    Header,
    /// One theme, by its index in [`tokens::theme::all`].
    Theme(usize),
    /// Nothing: the right list is longer than the left one here.
    Empty,
}

/// What moving the cursor changed.
///
/// # Why both arms carry the whole PAIR now
///
/// Because a preview is a pair, and it was half of one. Moving in the theme
/// column repainted the FOOTER and nothing else — every window kept the old
/// colours, and the desk kept them too, so what the person was shown was a
/// theme applied to one strip of the screen. The owner read that off the
/// screen and asked for the whole thing to move together.
///
/// The two arms survive because the LISTS are two — the drawing and the tests
/// still ask which cursor moved — but what is previewed no longer depends on
/// the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// The theme cursor moved; the pair it lands on.
    Theme(Choice),
    /// The desk cursor moved; the pair it lands on.
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
    /// Was there a file at all when this opened?
    ///
    /// A preview WRITES the file — that is how every other door hears about
    /// it — so by the time `Esc` runs, "does the file exist" can no longer
    /// answer "did the person ever choose". It is answered here, once, before
    /// the first preview: a machine that never chose must not be given a file
    /// by having looked at the list.
    chosen_before: bool,
}

impl View {
    /// The rows of the second column: the header and one per answer.
    pub const DESK_ROWS: usize = 1 + Backdrop::ALL.len();

    /// Opens on the theme the process is in and the desk the file says, which
    /// together are the committed pair. The keyboard starts on the themes.
    #[must_use]
    pub fn fresh_one(backdrop: Backdrop, chosen_before: bool) -> Self {
        let now = theme::current();
        Self {
            column: Column::Themes,
            cursor: now.position(),
            desk_cursor: backdrop.position(),
            committed: Choice {
                theme: now,
                backdrop,
            },
            chosen_before,
        }
    }

    /// Whether a choice was already written when this opened. See the field.
    #[must_use]
    pub const fn chosen_before(&self) -> bool {
        self.chosen_before
    }

    /// How many themes there are.
    #[must_use]
    pub fn theme_count() -> usize {
        theme::all().len()
    }

    /// How many rows the LEFT list takes: a title and one row per theme.
    #[must_use]
    pub fn theme_rows() -> usize {
        1 + Self::theme_count()
    }

    /// How many rows the region shows: the longer of the two lists.
    #[must_use]
    pub fn rows() -> usize {
        Self::theme_rows().max(Self::DESK_ROWS)
    }

    /// What row `i` of the FIRST column is.
    ///
    /// # Why the themes gained a title
    ///
    /// Because only one of the two lists had one, and a column with no name
    /// beside a column with one does not read as the other half of a pair —
    /// it reads as the thing the named column is about. The owner said so of
    /// this region in one line: it was not built to the shape the rest of the
    /// system uses. Every other list in this distro opens with a title in
    /// `text.group`, and now this one does too, on both sides of the rule.
    #[must_use]
    pub fn theme_row(i: usize) -> ThemeRow {
        match i {
            0 => ThemeRow::Header,
            n if n <= Self::theme_count() => ThemeRow::Theme(n - 1),
            _ => ThemeRow::Empty,
        }
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
            Column::Themes => 1 + self.cursor,
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
                Moved::Theme(self.focused_choice())
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
    /// The pointer landed on row `row` of the FIRST column.
    ///
    /// By ROW and not by theme index, which is what `focus_theme` takes: the
    /// two differ by the title now, and a click that did not know it would
    /// choose the theme one above the one under the pointer.
    pub fn focus_theme_row(&mut self, row: usize) -> Option<Choice> {
        match Self::theme_row(row) {
            ThemeRow::Theme(n) => {
                self.focus_theme(n);
                Some(self.focused_choice())
            }
            ThemeRow::Header | ThemeRow::Empty => None,
        }
    }

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
        let v = View::fresh_one(Backdrop::Black, true);
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
        // AND THE THEMES START UNDER THEIR OWN HEADER TOO. This line read
        // `v.row() == v.cursor()` while the left list had no title; the two
        // lists are now the same shape, and the drawing, the pointer and the
        // scroll all count rows rather than items.
        assert_eq!(v.row(), 1 + v.cursor());
    }

    #[test]
    fn both_lists_open_with_a_header_and_neither_header_takes_a_cursor() {
        // THE TWO LISTS ARE THE SAME SHAPE, which is what the owner asked for
        // when they said this region was not built like the rest of the
        // system: a title, then the items, on both sides of the rule.
        assert_eq!(View::theme_row(0), ThemeRow::Header);
        for i in 0..View::theme_count() {
            assert_eq!(View::theme_row(i + 1), ThemeRow::Theme(i));
        }
        assert_eq!(View::theme_row(View::theme_rows()), ThemeRow::Empty);
        assert!(View::rows() >= View::theme_rows());
        let mut v = View::fresh_one(Backdrop::Black, true);
        assert_eq!(v.focus_theme_row(0), None, "the title is not a row");
        assert!(
            v.focus_theme_row(1).is_some(),
            "the first theme sits under its title"
        );
        assert_eq!(v.cursor(), 0, "row 1 is theme 0");

        assert!(View::rows() >= View::theme_count());
        assert!(View::rows() >= View::DESK_ROWS);
        assert_eq!(View::desk_row(0), DeskRow::Header);
        for (i, b) in Backdrop::ALL.iter().enumerate() {
            assert_eq!(View::desk_row(i + 1), DeskRow::Answer(*b));
        }
        assert_eq!(View::desk_row(View::DESK_ROWS), DeskRow::Empty);

        let mut v = View::fresh_one(Backdrop::Black, true);
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
    fn either_cursor_previews_the_whole_pair() {
        // THE DEFECT THE OWNER READ OFF THE SCREEN: moving in the theme column
        // answered with a THEME, so the door repainted itself and told nobody
        // else — every window, the desk and the terminals kept the old
        // colours, and the preview was a theme worn by one strip of the
        // screen. Both arms carry the pair now, so what is previewed does not
        // depend on which list the keyboard happens to be in.
        let mut v = View::fresh_one(Backdrop::Black, true);
        // Put the DESK cursor somewhere that is not the committed answer, and
        // then move in the THEME column: what comes back has to carry it.
        v.cross(1);
        let Moved::Desk(_) = v.move_by(1) else {
            panic!("the desk column moves the desk");
        };
        v.cross(-1);
        assert_eq!(v.column(), Column::Themes);
        let Moved::Theme(pair) = v.move_by(1) else {
            panic!("the theme column moves the theme");
        };
        assert_eq!(
            pair.backdrop,
            Backdrop::Theme,
            "the theme column previewed a pair that forgot the desk"
        );
        assert_eq!(
            pair,
            v.focused_choice(),
            "the preview is not what is focused"
        );
        // AND IT IS STILL A PREVIEW. Neither move touches what `Esc` goes
        // back to — which is the whole reason the file can be written.
        assert_eq!(v.committed().backdrop, Backdrop::Black);
    }

    #[test]
    fn commit_moves_the_committed_pair_and_nothing_else_does() {
        let mut v = View::fresh_one(Backdrop::Black, true);
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
        let hint = View::fresh_one(Backdrop::Black, true).hint(&texts);
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
