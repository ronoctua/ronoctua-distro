//! THE MANUALS IN THE FOOTER'S REGION (`LEADER h`) — their list and the
//! manual open from it, as a [`Region`] the footer hosts by its id.
//!
//! What a key or a click means is the view's (`manual::View`), which the
//! window drives too; what it asks is carried out by the one handler both
//! doors call (`manual::carry_out`). What is the footer's here is its page:
//! a manual is running text it scrolls a line at a time, and its rows are the
//! footer's own two columns ([`Pair`]), laid by the footer's grid as the
//! shortcuts are — the colon at the widest key of the whole manual.

use crate::manual::{Scroll, Tone, Turn, View, Which};
use crate::region::{
    Closed, Doorway, Entry, Paging, Pair, Picture, Region, Row, Seen, Shape, Taken, Told,
};
use ronoctua_tokens as tokens;

/// The id the footer ranks the manuals under (`PRECEDENCE`).
pub const ID: &str = "manual";

/// The manuals, as the binary registers them (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The manuals, open in the footer.
struct Manuals {
    view: View,
    /// Whether the command that opened them has been handed to them — the
    /// same command later is the person asking for a region already taken.
    shown: bool,
}

/// What a command does with the manuals closed: `help` opens their list.
fn closed(command: &str, _argument: Option<&str>, _way: &mut dyn Doorway) -> Closed {
    if command == "help" {
        Closed::Opens
    } else {
        Closed::NotMine
    }
}

/// The list, with nothing typed.
fn open(_command: &str, _argument: Option<&str>, _way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(Manuals {
        view: View::fresh(),
        shown: false,
    })
}

/// A drawing of a gesture, as a pair's [`Picture`]: the box it is fitted in
/// is the manual's picture column, as tall as a gesture's lines.
fn picture_of(gesture: &'static str) -> Picture {
    Picture {
        key: crate::manual::gesture_art_key(gesture),
        cells: tokens::space::GESTURES_PICTURECELLS,
        lines: tokens::space::GESTURES_PICTURELINES,
    }
}

impl Manuals {
    /// What a key, a click or a paste asked, carried out by the handler the
    /// window calls too — and the footer's page moved as it says.
    fn carry(&mut self, errand: crate::manual::Errand, way: &mut dyn Doorway) {
        match crate::manual::carry_out(errand, way) {
            // THE FOOTER FOLLOWS NO CURSOR OF THE MANUALS': a manual is read,
            // and the list is two lines long.
            Turn::Stay | Turn::Follow => {}
            Turn::Top | Turn::Scroll(Scroll::Top) => way.first_page(),
            // ONE LINE AT A TIME for the arrows: a manual is read straight
            // through, and a screen per arrow loses the context between what
            // has just been read and what comes. The page keys are the other
            // way of reading, and the window's.
            Turn::Scroll(Scroll::Lines(n)) => way.page(Paging::Lines(n)),
            Turn::Scroll(Scroll::Pages(n)) => way.page(Paging::Pages(n)),
            Turn::Scroll(Scroll::Bottom) => way.page(Paging::End),
        }
    }

    /// Every line it shows — the list, or the manual open from it.
    ///
    /// A LINE TAKES A CLICK, and so lights under a resting pointer, only
    /// where the view says one does (`View::takes`, the window's answer
    /// too): a manual of the list, and no heading, sentence or line of an
    /// open manual.
    fn every_line(&self, seen: &Seen<'_>) -> Vec<Told> {
        let mut lines = match self.view.which() {
            None => self.list(seen),
            Some(which) => Self::manual(which, seen),
        };
        let takes = self.view.takes(seen.texts);
        for (n, line) in lines.iter_mut().enumerate() {
            line.row.takes = takes.contains(&n);
        }
        lines
    }

    /// The list's rows: a heading, what has been typed, and every manual it
    /// leaves — the name in the key column, lit where the query matched, the
    /// sentence beside it.
    fn list(&self, seen: &Seen<'_>) -> Vec<Told> {
        let t = seen.texts;
        let line = |text: String, text_tone: ronoctua_tokens::Color| Told {
            pair: Some(Pair {
                text,
                text_tone,
                ..Pair::default()
            }),
            ..Told::default()
        };
        let mut rows = vec![
            line(t.t(crate::key::MANUAL_LIST), tokens::color::TEXT_HEADING),
            if self.view.query().is_empty() {
                line(
                    t.t(crate::key::MANUAL_FILTER_EMPTY),
                    tokens::color::TEXT_DIM,
                )
            } else {
                line(self.view.query().to_owned(), tokens::color::TEXT_PRIMARY)
            },
        ];
        let found = self.view.found(t);
        if found.is_empty() {
            rows.push(line(t.t(crate::key::MANUAL_NONE), tokens::color::TEXT_DIM));
        }
        for (i, w) in found.into_iter().enumerate() {
            let name = t.t(w.name());
            let about = t.t(w.about());
            // THE CURSOR IS A COLOUR (`the-cursor-is-a-colour`): the name
            // takes the accent where the keyboard is — ONE ITEM OF A LIST OF
            // DOORS, whose two tones are `chrome::Kind::ListedSetting`'s.
            let here = i == self.view.cursor();
            let (key_tone, text_tone) = crate::chrome::Kind::ListedSetting.tones(here);
            rows.push(Told {
                about: Some(about.clone()),
                pair: Some(Pair {
                    lit: self.view.lit(&name),
                    here,
                    key: name,
                    flush_right: false,
                    colon: false,
                    key_tone,
                    text: about,
                    text_tone,
                    ..Pair::default()
                }),
                ..Told::default()
            });
        }
        rows
    }

    /// An open manual's lines, and nothing else — the way to a window is
    /// `C-w`, on the hint line. WRAPPED TO THE ROOM the grid leaves the
    /// text: the colon column is the key's width plus the inset and a cell,
    /// the text two past it, and the bar takes the last.
    fn manual(which: Which, seen: &Seen<'_>) -> Vec<Told> {
        let lines = crate::manual::laid_out(
            which.lines(seen.texts, &crate::bindings::chord_label()),
            seen.columns,
            crate::chrome::INSET + 4,
            crate::chrome::INSET + 1,
        );
        lines
            .into_iter()
            .map(|l| {
                let text_tone = match l.tone {
                    // A section title in the group tone: it organizes, it
                    // does not run.
                    Tone::Heading => tokens::color::TEXT_HEADING,
                    Tone::Plain | Tone::Pose => tokens::color::TEXT_SECONDARY,
                    // WHAT A GESTURE DOES, the loudest: it is what the eye
                    // looks for down the column, the owner's ask.
                    Tone::Function => tokens::color::TEXT_PRIMARY,
                    Tone::Detail | Tone::Air => tokens::color::TEXT_DIM,
                };
                Told {
                    about: l.beside_picture.then(|| l.description.clone()),
                    pair: Some(Pair {
                        key: l.key,
                        text: l.description,
                        text_tone,
                        picture: l.picture.map(picture_of),
                        beside_picture: l.beside_picture,
                        colon: !l.beside_picture,
                        // THE TEXT STAYS IN THE RIGHT COLUMN, past the
                        // drawing: a line with no key otherwise takes the
                        // whole width, and it was drawn under the picture.
                        aside: l.beside_picture,
                        ..Pair::default()
                    }),
                    ..Told::default()
                }
            })
            .collect()
    }
}

impl Region for Manuals {
    fn id(&self) -> &'static str {
        ID
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.every_line(seen).into_iter().map(|t| t.row).collect()
    }

    /// Every line in the footer's own two columns ([`Pair`]).
    fn told(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Told> {
        self.every_line(seen)
            .into_iter()
            .skip(skip)
            .take(take)
            .collect()
    }

    /// Which keys walk the list — or the manual — and `C-w`, the window,
    /// which is on this line and not a line of the page (the owner's rule: a
    /// page is what it shows).
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        crate::chrome::hint_of(self.view.taught(), seen.texts)
    }

    /// None: a manual is read, and its page is moved by its own keys.
    fn cursor(&self) -> Option<usize> {
        None
    }

    fn shape(&self) -> Shape {
        Shape::Pairs
    }

    /// The manuals are read through the keyboard — the arrows move the page
    /// — and close when the footer really loses it, as the `M-x` does.
    fn closes_without_the_keyboard(&self) -> bool {
        true
    }

    /// The view's own reading of a key — the window's too.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let errand = self.view.key(key, way.texts());
        let took = errand != crate::manual::Errand::Nothing;
        self.carry(errand, way);
        Taken::of(took)
    }

    fn command(&mut self, id: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        if id != "help" {
            return Taken::No;
        }
        if self.shown {
            // THE REGION IS TAKEN — by the manuals, or by a view above them.
            let said = way.texts().t(crate::key::ECHO_VIEW_BUSY);
            way.say(said, crate::echo::Echo::Normal);
            way.repaint();
        }
        self.shown = true;
        Taken::Yes
    }

    /// SELECT, AND THE SECOND CLICK OPENS (invariant 19): the click is the
    /// cursor, and `RET` on the cursor is what opens — on a row the view
    /// says a click takes (`View::takes`), and nothing on any other.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let errand = self.view.clicked(row, way.texts());
        self.carry(errand, way);
    }

    /// The clipboard's text it asked for, into the filter.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        let errand = self.view.pasted(text);
        self.carry(errand, way);
    }

    /// The gestures' drawings, while their manual is open.
    fn pictures(&self, _seen: &Seen<'_>) -> Vec<String> {
        if self.view.which() != Some(Which::Gestures) {
            return Vec::new();
        }
        ronoctua_gestures::GESTURES
            .iter()
            .map(|g| crate::manual::gesture_art_key(g))
            .collect()
    }

    /// A gesture's drawing, rasterised to fit its box, in the manual's ink.
    fn picture(&self, key: &str) -> Option<ronoctua_render::image::Rgba> {
        ronoctua_gestures::GESTURES
            .iter()
            .find(|g| crate::manual::gesture_art_key(g) == key)
            .and_then(|g| crate::manual::gesture_art(g))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manual::FIRST_OF_FOUND;
    use crate::region::{Effect, Lent, Recorder};

    fn texts() -> std::rc::Rc<ronoctua_i18n::Texts> {
        std::rc::Rc::new(
            ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language"),
        )
    }

    fn way(texts: &std::rc::Rc<ronoctua_i18n::Texts>) -> Recorder {
        Recorder::new(
            std::rc::Rc::clone(texts),
            120,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    fn seen<'a>(texts: &'a ronoctua_i18n::Texts, zone: &'a ronoctua_files::when::Zone) -> Seen<'a> {
        Seen {
            texts,
            columns: 120,
            lines: 12,
            zone,
            photographing: true,
            lent: Lent::none(),
        }
    }

    fn key(notation: &str) -> ronoctua_keymap::Key {
        ronoctua_keymap::Key::parse(notation).expect("a key")
    }

    /// FAILS IF: the list stops being the heading, the filter's line and a
    /// manual a row — the click's arithmetic (`FIRST_OF_FOUND`) would open
    /// another manual than the one under the pointer — or a manual's row
    /// stops saying what it is under the pointer.
    #[test]
    fn the_list_is_two_lines_and_a_manual_a_row() {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let mut w = way(&t);
        let mut m = open("help", None, &mut w);
        assert_eq!(m.command("help", None, &mut w), Taken::Yes);
        assert!(
            w.owed.is_empty(),
            "the opening said something: {:?}",
            w.owed
        );
        let rows = m.told(&seen(&t, &zone), 0, 1000);
        assert_eq!(rows.len(), FIRST_OF_FOUND + Which::ALL.len());
        assert_eq!(m.rows(&seen(&t, &zone)).len(), rows.len());
        assert!(rows[..FIRST_OF_FOUND].iter().all(|r| !r.row.takes));
        for (r, w) in rows[FIRST_OF_FOUND..].iter().zip(View::fresh().found(&t)) {
            let pair = r.pair.as_ref().expect("a pair");
            assert_eq!(pair.key, t.t(w.name()));
            assert_eq!(r.about.as_deref(), Some(t.t(w.about()).as_str()));
            assert!(r.row.takes && !pair.colon && !pair.flush_right);
        }
        // THE FIRST CLICK SELECTS, AND THE SECOND OPENS.
        let second = FIRST_OF_FOUND + 1;
        m.clicked(second, &mut w);
        m.clicked(second, &mut w);
        assert!(w.owed.contains(&Effect::FirstPage), "{:?}", w.owed);
        assert_eq!(m.shape(), Shape::Pairs);
        // AND ASKED FOR AGAIN, the region is taken.
        w.owed.clear();
        let _ = m.command("help", None, &mut w);
        assert!(matches!(&w.owed[0], Effect::Say(..)));
    }

    /// FAILS IF: the arrows of an open manual stop moving the footer's page
    /// a LINE, the page keys a page, or `<end>` to its end — or `C-w` does
    /// not open the window AND leave the footer.
    #[test]
    fn an_open_manual_moves_the_footers_page() {
        let t = texts();
        let mut w = way(&t);
        let mut m = open("help", None, &mut w);
        let _ = m.command("help", None, &mut w);
        let _ = m.key(key("RET"), &mut w);
        w.owed.clear();
        for (notation, page) in [
            ("<down>", Effect::Page(Paging::Lines(1))),
            ("<up>", Effect::Page(Paging::Lines(-1))),
            ("<next>", Effect::Page(Paging::Pages(1))),
            ("SPC", Effect::Page(Paging::Pages(1))),
            ("<prior>", Effect::Page(Paging::Pages(-1))),
            ("<home>", Effect::FirstPage),
            ("<end>", Effect::Page(Paging::End)),
        ] {
            w.owed.clear();
            assert_eq!(m.key(key(notation), &mut w), Taken::Yes, "{notation}");
            assert_eq!(w.owed, vec![Effect::Repaint, page], "{notation}");
        }
        w.owed.clear();
        let _ = m.key(key("C-w"), &mut w);
        let opened = View::fresh().found(&t)[0];
        assert_eq!(
            w.owed,
            vec![
                Effect::OpenWindow(vec!["manual".to_owned(), opened.word().to_owned()]),
                Effect::Close
            ]
        );
    }

    /// FAILS IF: the gestures' manual stops putting each drawing beside its
    /// three lines, in a box the footer can fit it in, or the drawing a row
    /// names is not one the region can draw.
    #[test]
    fn the_gestures_draw_beside_their_lines() {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let mut w = way(&t);
        let mut m = open("help", None, &mut w);
        let _ = m.command("help", None, &mut w);
        assert!(
            m.pictures(&seen(&t, &zone)).is_empty(),
            "the list draws no gesture"
        );
        for letter in ["h", "a", "n", "d"] {
            let _ = m.key(key(letter), &mut w);
        }
        let _ = m.key(key("RET"), &mut w);
        let rows = m.told(&seen(&t, &zone), 0, 1000);
        let drawn: Vec<&Picture> = rows
            .iter()
            .filter_map(|r| r.pair.as_ref()?.picture.as_ref())
            .collect();
        assert_eq!(drawn.len(), ronoctua_gestures::GESTURES.len());
        let keys = m.pictures(&seen(&t, &zone));
        for p in drawn {
            assert_eq!(usize::from(p.lines), crate::manual::GESTURE_LINES);
            assert!(keys.contains(&p.key), "{} is drawn and never put up", p.key);
        }
        assert!(m.picture("gesture:none").is_none());
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — the list's heading, the filter's line, the
    /// sentence that says nothing matched, a line of an open manual, its air
    /// included — or leaves unlit one it takes. THE LIGHT IS THE CLICK:
    /// every row clicked once, on a view of its own, and lit exactly where
    /// something happened — on the list, on a list the filter emptied, and
    /// in an open manual.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let at = seen(&t, &zone);
        let held = |view: View| {
            crate::region::held_to_the_click(
                || {
                    let region = Manuals {
                        view: view.clone(),
                        shown: true,
                    };
                    (region, way(&t))
                },
                &at,
                // THE VIEW IS SEEN TOO: the region's rows are the footer's
                // pairs, with no fields, and its cursor a colour of them.
                |r: &Manuals, w: &Recorder| (r.view.clone(), crate::region::seen(r, &w.owed, &at)),
            )
        };
        let list = held(View::fresh());
        assert!(
            list.iter().any(|h| h.lit) && list.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {list:?}"
        );
        let unheld = crate::region::unheld(&list);
        assert!(unheld.is_empty(), "the list: {unheld:?}");
        let mut emptied = View::fresh();
        for letter in ["z", "z", "z"] {
            let _ = emptied.key(key(letter), &t);
        }
        assert!(emptied.found(&t).is_empty(), "the filter left a manual");
        for (what, view) in [
            ("a list the filter emptied", emptied),
            ("an open manual", View::on(Which::Shortcuts)),
        ] {
            let rows = held(view);
            assert!(rows.len() > FIRST_OF_FOUND, "{what}: {rows:?}");
            assert!(rows.iter().all(|h| !h.lit), "{what}: {rows:?}");
            let unheld = crate::region::unheld(&rows);
            assert!(unheld.is_empty(), "{what}: {unheld:?}");
        }
    }

    /// FAILS IF: what was pasted no longer narrows the list, or a closed
    /// region answers a command that is not its own.
    #[test]
    fn a_paste_filters_and_only_help_opens() {
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let mut w = way(&t);
        assert_eq!(closed("help", None, &mut w), Closed::Opens);
        assert_eq!(closed("help-me", None, &mut w), Closed::NotMine);
        let mut m = open("help", None, &mut w);
        let _ = m.command("help", None, &mut w);
        let _ = m.key(key("C-v"), &mut w);
        assert_eq!(w.owed, vec![Effect::Paste]);
        m.pasted("Hand", &mut w);
        let rows = m.told(&seen(&t, &zone), 0, 1000);
        assert_eq!(rows.len(), FIRST_OF_FOUND + 1);
        assert_eq!(rows[1].pair.as_ref().map(|p| p.text.as_str()), Some("Hand"));
    }
}
