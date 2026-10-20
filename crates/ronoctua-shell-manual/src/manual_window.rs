//! The manuals in a window of their own — the second door onto
//! [`crate::manual`], opened by `ronoctua-shell manual` on their list, or on
//! one of them from the footer's `C-w`.
//!
//! It DECIDES nothing: which manuals there are, what the filter leaves, what
//! a key or a click does on each level and which lines a manual has are all
//! `manual.rs` (`manual::View`), and the footer drives the same view. It is
//! `history_window.rs` with the view swapped for the manuals, deliberately:
//! eleven doors that drifted apart would be eleven answers to how a wheel
//! scrolls.
//!
//! # The same two levels as the footer
//!
//! The LIST: the arrows walk it, what is typed narrows it (`DEL` takes a
//! cluster off, `C-v` pastes), `RET` or a second click opens the manual under
//! the cursor, and `Esc` closes. A MANUAL: the arrows, `SPC`, the page keys,
//! the wheel and the bar move it, and `Esc` goes back to the list. The one
//! key this door has not got is `C-w`: it takes a manual out of the footer
//! into a window, and this is the window.

use crate::door::{Canvas, Content, Ui};
use crate::fields::Field;
use crate::manual::{Errand, FIRST_OF_FOUND, Line, Scroll, Tone, Turn, View, Which};
use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.manual";

/// Opens the manuals in a window: on manual `which`, or on their list.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(which: Option<Which>) -> Result<()> {
    crate::door::run(|_| Ok(Manual::at(which)))
}

/// The window: the manuals' view, and the size of the drawings.
struct Manual {
    view: View,
    /// The size each drawing went up at, by its key: it is centred in its
    /// box by that size.
    art_sizes: std::collections::HashMap<String, (u32, u32)>,
}

impl Manual {
    /// On manual `which`, or on the list.
    fn at(which: Option<Which>) -> Self {
        Self {
            view: which.map_or_else(View::fresh, View::on),
            art_sizes: std::collections::HashMap::new(),
        }
    }

    /// The open manual's lines, in the language of now, WRAPPED to this
    /// window — the text column starts two past the key column, and the bar
    /// takes the last cell. Empty on the list.
    fn manual(&self, ui: &Ui) -> Vec<Line> {
        self.view.which().map_or_else(Vec::new, |which| {
            crate::manual::laid_out(
                which.lines(&ui.texts, &crate::bindings::chord_label()),
                ui.columns,
                3,
                1,
            )
        })
    }

    /// Moves by `by` lines, inside the manual.
    fn scroll_by(&self, ui: &mut Ui, by: i64) {
        let last = self.manual(ui).len().saturating_sub(ui.room_for_rows());
        ui.offset = usize::try_from(
            i64::try_from(ui.offset)
                .unwrap_or(0)
                .saturating_add(by)
                .max(0),
        )
        .unwrap_or(0)
        .min(last);
        ui.ask_paint();
    }

    /// Carries out what the view asked for — through the one handler the
    /// footer's region calls too (`manual::carry_out`), on this door's page.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        // THE ONE KEY THIS DOOR HAS NOT GOT: `C-w` takes a manual into a
        // window, and this is it.
        if matches!(errand, Errand::Nothing | Errand::Window(_)) {
            return;
        }
        match crate::manual::carry_out(errand, ui) {
            Turn::Stay => {}
            Turn::Follow => {
                if let Some(cursor) = self.cursor() {
                    let rows = self.rows(ui).len();
                    ui.follow(cursor, rows);
                }
            }
            Turn::Top => ui.offset = 0,
            Turn::Scroll(how) => {
                let page = i64::try_from(ui.room_for_rows().max(1)).unwrap_or(1);
                self.scroll_by(
                    ui,
                    match how {
                        Scroll::Lines(n) => i64::from(n),
                        Scroll::Pages(n) => i64::from(n).saturating_mul(page),
                        Scroll::Top => i64::MIN / 2,
                        Scroll::Bottom => i64::MAX / 2,
                    },
                );
            }
        }
        ui.ask_paint();
    }
}

/// The list's rows, placed in cells: a heading, what has been typed — or how
/// to type — and every manual the filter leaves, the name in the key column,
/// lit where the query matched, the sentence beside it. The footer's list,
/// row for row.
fn list_rows(view: &View, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Vec<Field>> {
    let piece = |at: u16, text: String, color: tokens::Color| Field {
        at,
        text,
        color,
        hit: crate::fields::Hit::Nothing,
        explain: None,
        ink: crate::fields::Ink::Grid,
    };
    let mut rows = vec![
        vec![piece(
            0,
            texts.t(crate::key::MANUAL_LIST),
            tokens::color::TEXT_HEADING,
        )],
        if view.query().is_empty() {
            vec![piece(
                0,
                texts.t(crate::key::MANUAL_FILTER_EMPTY),
                tokens::color::TEXT_DIM,
            )]
        } else {
            // THE TYPED TEXT KEEPS ITS END, where the next letter goes.
            vec![piece(
                0,
                ronoctua_render::layout::elide_start(view.query(), columns),
                tokens::color::TEXT_PRIMARY,
            )]
        },
    ];
    let found = view.found(texts);
    if found.is_empty() {
        rows.push(vec![piece(
            0,
            texts.t(crate::key::MANUAL_NONE),
            tokens::color::TEXT_DIM,
        )]);
    }
    debug_assert_eq!(rows.len(), FIRST_OF_FOUND + usize::from(found.is_empty()));
    // THE NAMES' COLUMN is as wide as the widest name of every manual, and
    // not of what the filter left: the sentences do not jump as one types.
    let names = Which::ALL
        .iter()
        .map(|w| ronoctua_render::layout::width(&texts.t(w.name())))
        .max()
        .unwrap_or(0);
    for (i, w) in found.into_iter().enumerate() {
        // THE CURSOR IS A COLOUR (`the-cursor-is-a-colour`): the name takes
        // the accent where the keyboard is.
        let here = i == view.cursor();
        let (name_ink, about_ink) = crate::chrome::Kind::ListedSetting.tones(here);
        let name = texts.t(w.name());
        let lit = view.lit(&name);
        let mut fields = crate::chrome::lit_letters(&piece(0, name, name_ink), &lit, here);
        let at = names + 2;
        fields.push(piece(
            at,
            ronoctua_render::layout::elide_end(&texts.t(w.about()), columns.saturating_sub(at)),
            about_ink,
        ));
        rows.push(fields);
    }
    rows
}

/// Which of `rows` rows a click takes, row by row — the view's one answer
/// ([`View::takes`]), which the footer's region gives too: a manual of the
/// list, and never its heading, the filter's line, the sentence that says
/// nothing matched or a line of an open manual.
fn takes_of(view: &View, rows: usize, texts: &ronoctua_i18n::Texts) -> Vec<bool> {
    let takes = view.takes(texts);
    (0..rows).map(|row| takes.contains(&row)).collect()
}

/// The manual's lines as the rows of a list, placed in cells.
///
/// THE KEY COLUMN is as wide as the widest key, or the drawings' column when
/// the manual has drawings — the footer's arithmetic, by the same numbers.
fn rows_of(manual: &[Line], columns: u16) -> Vec<Vec<Field>> {
    let key_cells = crate::manual::key_column(manual);
    let piece = |at: u16, text: String, color: tokens::Color| Field {
        at,
        text,
        color,
        hit: crate::fields::Hit::Nothing,
        explain: None,
        ink: crate::fields::Ink::Grid,
    };
    manual
        .iter()
        .map(|l| {
            let color = match l.tone {
                Tone::Heading => tokens::color::TEXT_HEADING,
                Tone::Plain | Tone::Pose => tokens::color::TEXT_SECONDARY,
                // WHAT A GESTURE DOES, the loudest — the footer's rule.
                Tone::Function => tokens::color::TEXT_PRIMARY,
                Tone::Detail => tokens::color::TEXT_DIM,
                // AIR: nothing is written on it.
                Tone::Air => return Vec::new(),
            };
            let mut fields = Vec::with_capacity(2);
            if !l.key.is_empty() {
                fields.push(piece(0, l.key.clone(), tokens::color::TEXT_KEY));
            }
            // A HEADING TAKES THE WHOLE LINE from the margin; everything else
            // starts after the key column.
            let (at, room_cells) = if l.title() {
                (0, columns)
            } else {
                (key_cells + 2, columns.saturating_sub(key_cells + 2))
            };
            fields.push(piece(
                at,
                ronoctua_render::layout::elide_end(&l.description, room_cells),
                color,
            ));
            fields
        })
        .collect()
}

impl Content for Manual {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    /// The open manual's name, or the manuals' own on their list.
    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        let name = self.view.which().map_or(crate::key::CMD_HELP, Which::name);
        format!("*{}*", texts.t(name))
    }

    /// A key, read by the view the footer reads its keys with.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        let errand = self.view.key(key, &ui.texts);
        self.apply(ui, errand);
    }

    /// What was copied, into the list's filter.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        let errand = self.view.pasted(text);
        self.apply(ui, errand);
    }

    /// THE LEVEL'S KEYS, from the table the footer teaches — every one a
    /// click — less `C-w`, which this door has not got.
    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        let here: Vec<(&'static str, &'static str)> = self
            .view
            .taught()
            .iter()
            .filter(|(_, id)| *id != crate::manual::TO_A_WINDOW)
            .copied()
            .collect();
        crate::chrome::hint_of(&here, &ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<Field>> {
        if self.view.which().is_some() {
            rows_of(&self.manual(ui), ui.columns)
        } else {
            list_rows(&self.view, ui.columns, &ui.texts)
        }
    }

    /// A click takes a manual of the list, and only that row lights under
    /// the pointer ([`takes_of`]).
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        takes_of(&self.view, self.rows(ui).len(), &ui.texts)
    }

    /// The list's cursor, as a row — a manual has none: it is read.
    fn cursor(&self) -> Option<usize> {
        self.view
            .which()
            .is_none()
            .then(|| FIRST_OF_FOUND + self.view.cursor())
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE OPENS — invariant 19, by the
    /// view's own `clicked`, which the footer's click reaches too, and which
    /// takes nothing on a row [`View::takes`] leaves out.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let errand = self.view.clicked(row, &ui.texts);
        self.apply(ui, errand);
    }

    /// The standard list, under a header that NAMES the manual, and the
    /// gestures' drawings beside their lines.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        // LINE 0: on a manual, `MANUAL:` and its name, drawn the way every
        // title of the system is — the label in the cool grey kept for what is
        // said ABOUT something, the name in the heading's ink — and on the
        // list no title, whose first row is its heading. Then the level's
        // keys, every one a click, in the header every window of this system
        // has, drawer and all.
        let label = ui.texts.t(crate::key::MANUAL_WINDOW_LEAD);
        let name = self.view.which().map(|w| ui.texts.t(w.name()));
        let hint = self.hint(ui);
        crate::door::list::header_of(
            ui,
            canvas,
            name.as_deref().map(|name| crate::chrome::Title {
                glyph: None,
                label: Some(&label),
                name: Some(name),
            }),
            |_| hint,
        );
        let rows = self.rows(ui);
        // LIT ONLY WHERE A CLICK TAKES: a manual of the list, never its
        // heading nor a line of an open manual.
        let takes = takes_of(&self.view, rows.len(), &ui.texts);
        crate::door::list::body(ui, canvas, &rows, &takes, None);
        let manual = self.manual(ui);

        // THE DRAWING, when every line of its gesture is on screen — see the
        // footer's reason in `Footer::paint`. It goes up once per key (size
        // and theme are in it), and is centred in its box by its own size.
        let size = tokens::text::size();
        let cell = tokens::metrics::cell_width(size);
        let line_height = tokens::metrics::line_height(size);
        let first = ui.first_of_list();
        let visible: Vec<&Line> = manual
            .iter()
            .skip(ui.offset)
            .take(ui.room_for_rows())
            .collect();
        for (i, l) in visible.iter().enumerate() {
            let Some(g) = l.picture else {
                continue;
            };
            if i + crate::manual::GESTURE_LINES > visible.len() {
                continue;
            }
            let y = first + u16::try_from(i).unwrap_or(0);
            let key = crate::manual::gesture_art_key(g);
            let sizes = &mut self.art_sizes;
            canvas.upload(&key, || {
                let rgba = crate::manual::gesture_art(g)?;
                sizes.insert(key.clone(), (rgba.width, rgba.height));
                Some(rgba)
            });
            let Some(&(w, h)) = self.art_sizes.get(&key) else {
                continue;
            };
            let (bw, bh) = crate::manual::gesture_art_box();
            let (x, top) = (f32::from(ui.margin) * cell, f32::from(y) * line_height);
            #[allow(clippy::cast_precision_loss)]
            let (w, h) = (w as f32, h as f32);
            canvas.pictures.push((
                key,
                ronoctua_render::image::Area {
                    x: x + (bw - w).max(0.0) / 2.0,
                    y: top + (bh - h).max(0.0) / 2.0,
                    width: w.min(bw),
                    height: h.min(bh),
                },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_keymap::Key;

    fn ui() -> Ui {
        Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"))
    }

    /// One key, as the keyboard sends it.
    fn key(w: &mut Manual, ui: &mut Ui, notation: &str) {
        let (event, mods) = crate::door::keys::event_of(Key::parse(notation).expect("a key"));
        w.typed(ui, &event, mods);
    }

    fn said(rows: &[Vec<Field>]) -> Vec<String> {
        rows.iter()
            .map(|r| r.iter().map(|f| f.text.as_str()).collect::<String>())
            .collect()
    }

    /// THE WINDOW HAS THE FOOTER'S LIST, AND ITS FILTER IS TYPED HERE.
    ///
    /// THE DEFECT THIS CATCHES: the window opened ONE manual, dropped every
    /// letter, and `Esc` closed it — no list, no filter, no other manual.
    #[test]
    fn the_window_opens_on_the_list_and_types_its_filter() {
        let mut w = Manual::at(None);
        let mut ui = ui();
        let rows = said(&w.rows(&ui));
        assert!(rows[0].contains(&ui.texts.t(crate::key::MANUAL_LIST)));
        assert_eq!(rows.len(), FIRST_OF_FOUND + Which::ALL.len());
        for c in ["h", "a", "n", "d", "SPC", "g"] {
            key(&mut w, &mut ui, c);
        }
        let rows = said(&w.rows(&ui));
        assert_eq!(rows[1], "hand g", "the typed text is drawn");
        assert_eq!(rows.len(), FIRST_OF_FOUND + 1);
        assert!(rows[FIRST_OF_FOUND].contains(&ui.texts.t(Which::Gestures.name())));
        // `RET` OPENS IT HERE, on its first line.
        ui.offset = 3;
        key(&mut w, &mut ui, "RET");
        assert_eq!(w.view.which(), Some(Which::Gestures));
        assert_eq!(ui.offset, 0);
        assert_eq!(w.cursor(), None, "a manual is read, and has no cursor");
        // `Esc` GOES BACK TO THE LIST, and then closes.
        key(&mut w, &mut ui, "ESC");
        assert_eq!(w.view.which(), None);
        assert!(!ui.quit, "Esc out of a manual closed the window");
        key(&mut w, &mut ui, "ESC");
        assert!(ui.quit);
        // AND NOTHING WENT TO THE FOOTER.
        assert!(ui.orders().is_empty());
    }

    /// A NAME THAT MATCHES NOTHING IS SAID, as the footer says it.
    #[test]
    fn a_filter_that_leaves_nothing_says_so() {
        let mut w = Manual::at(None);
        let mut ui = ui();
        for c in ["z", "z", "z"] {
            key(&mut w, &mut ui, c);
        }
        let rows = said(&w.rows(&ui));
        assert_eq!(rows.len(), FIRST_OF_FOUND + 1);
        assert_eq!(rows[FIRST_OF_FOUND], ui.texts.t(crate::key::MANUAL_NONE));
        w.clicked(&mut ui, FIRST_OF_FOUND);
        assert_eq!(w.view.which(), None, "the sentence opened something");
    }

    /// THE FIRST CLICK SELECTS, AND THE SECOND OPENS — invariant 19, through
    /// the view's `clicked`, which the footer's click reaches too.
    #[test]
    fn the_first_click_selects_and_the_second_opens() {
        let mut w = Manual::at(None);
        let mut ui = ui();
        let second = FIRST_OF_FOUND + 1;
        w.clicked(&mut ui, second);
        assert_eq!(w.cursor(), Some(second));
        assert_eq!(w.view.which(), None);
        w.clicked(&mut ui, second);
        assert_eq!(w.view.which(), Some(w.view.found(&ui.texts)[1]));
        // THE HEADING AND THE FILTER'S LINE ARE NO MANUAL.
        let mut w = Manual::at(None);
        w.clicked(&mut ui, 0);
        w.clicked(&mut ui, 0);
        assert_eq!(w.view.which(), None);
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — the list's heading, the filter's line, the sentence that
    /// says nothing matched, a line of an open manual, its air included — or
    /// leaves unlit one it takes. THE LIGHT IS THE CLICK: every row clicked
    /// once, on a window of its own, and lit exactly where something
    /// happened — on the list, on a list the filter emptied, and in an open
    /// manual. The footer's region has its own.
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let list = crate::door::held_to_the_click(|| (Manual::at(None), ui()), crate::door::seen);
        assert!(
            list.iter().any(|h| h.lit) && list.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {list:?}"
        );
        let unheld = crate::region::unheld(&list);
        assert!(unheld.is_empty(), "the list: {unheld:?}");
        let emptied = || {
            let mut w = Manual::at(None);
            let mut ui = ui();
            for c in ["z", "z", "z"] {
                key(&mut w, &mut ui, c);
            }
            (w, ui)
        };
        let manual = || (Manual::at(Some(Which::Shortcuts)), ui());
        for (what, held) in [
            (
                "a list the filter emptied",
                crate::door::held_to_the_click(emptied, crate::door::seen),
            ),
            (
                "an open manual",
                crate::door::held_to_the_click(manual, crate::door::seen),
            ),
        ] {
            assert!(held.len() > FIRST_OF_FOUND, "{what}: {held:?}");
            assert!(held.iter().all(|h| !h.lit), "{what}: {held:?}");
            let unheld = crate::region::unheld(&held);
            assert!(unheld.is_empty(), "{what}: {unheld:?}");
        }
    }

    /// `C-v` ASKS THE DOOR FOR THE CLIPBOARD, read off the loop, and what
    /// arrives narrows the list as one line.
    #[test]
    fn a_paste_is_asked_of_the_door_and_filters_the_list() {
        let mut w = Manual::at(None);
        let mut ui = ui();
        key(&mut w, &mut ui, "C-v");
        assert!(ui.wants_the_clipboard());
        w.pasted(&mut ui, "Short\n");
        assert_eq!(w.view.query(), "Short");
        assert_eq!(w.view.found(&ui.texts), vec![Which::Shortcuts]);
    }

    /// THE HEADER TEACHES THE LEVEL'S KEYS, every one a click — and not
    /// `C-w`, which takes a manual out of the footer into a window: this is
    /// the window. The footer teaches the same table with it.
    #[test]
    fn the_header_teaches_the_levels_keys_but_the_window_key() {
        let mut w = Manual::at(None);
        let ui = ui();
        let keys = |w: &Manual| -> String { w.hint(&ui).iter().map(|p| p.text.clone()).collect() };
        let list = keys(&w);
        assert!(list.contains("RET"), "{list}");
        assert!(!list.contains("C-w"), "{list}");
        assert!(!w.hint(&ui).is_empty());
        w.view = View::on(Which::Shortcuts);
        let open = keys(&w);
        assert!(!open.contains("C-w"), "{open}");
        assert!(!open.contains("RET"), "{open}");
        for (taught, id) in crate::manual::OF_THE_LIST
            .iter()
            .chain(crate::manual::OF_A_MANUAL)
        {
            assert!(Key::parse(taught).is_ok(), "{taught} ({id})");
        }
    }

    /// THE PAGE KEYS AND `SPC` MOVE AN OPEN MANUAL, and `<home>` and `<end>`
    /// take it to its ends — the same errands the footer's page carries out.
    #[test]
    fn an_open_manual_pages() {
        let mut w = Manual::at(Some(Which::Shortcuts));
        let mut ui = ui();
        ui.lines = 10;
        let room = ui.room_for_rows();
        let last = w.manual(&ui).len().saturating_sub(room);
        assert!(last > room, "the manual is not long enough to page");
        key(&mut w, &mut ui, "<next>");
        assert_eq!(ui.offset, room);
        key(&mut w, &mut ui, "SPC");
        assert_eq!(ui.offset, 2 * room);
        key(&mut w, &mut ui, "<prior>");
        assert_eq!(ui.offset, room);
        key(&mut w, &mut ui, "<down>");
        assert_eq!(ui.offset, room + 1);
        key(&mut w, &mut ui, "<end>");
        assert_eq!(ui.offset, last);
        key(&mut w, &mut ui, "<home>");
        assert_eq!(ui.offset, 0);
    }
}
