//! THE ICONS APP'S ERRANDS IN EITHER DOOR ([`carry_out`]) — and its region of
//! the footer ([`IconsRegion`], [`ENTRY`]), which lays out its own cells.
//!
//! # One errand handler, two doors
//!
//! What a key of the app asks — copy a glyph, ask the Lucide colour, read the
//! clipboard into the search, close — is done by [`carry_out`] over the
//! door's [`Doorway`]: said on that door's line, asked on its question line.
//! The window and the region used to write it out apart.
//!
//! # Its own cells, every hand its own aim
//!
//! The region is [`Shape::Cells`]: the footer gives it blank lines and the
//! app draws its tab line, its grid or its page over them, with the layout
//! its pointer reads — the window's drawing. Each hand keeps what it points
//! at (its AIM, a target's code), and each is lit in its own colour.

use std::sync::{Arc, mpsc};

use crate::door::{BACK_BUTTON, LEFT_BUTTON};
use crate::icons::{Area, Errand, Target, View};
use crate::region::{
    Closed, Doorway, Entry, Painted, Pointed, Pointing, Region, RegionEasel, Row, Seen, Shape,
    Source, Taken,
};
use ronoctua_iconset::{Collection, Refusal, Set};
use ronoctua_tokens as tokens;

/// The region's id, as the footer ranks it.
pub const ID: &str = "icons";

/// WHAT THE APP ASKED, done in either door — a copy said on that door's line,
/// a question asked on its line — and a frame.
pub fn carry_out(errand: Errand, way: &mut dyn Doorway) {
    match errand {
        Errand::Nothing => {}
        Errand::Close => {
            way.close();
            return;
        }
        Errand::Copy(text) => {
            let said = crate::chrome::copied(&text, way.texts());
            way.say(said, ronoctua_shell_kit::echo::Echo::Normal);
        }
        // THIS DOOR ASKS ON ITS OWN LINE — the minibuffer, a window's
        // header — starting from the colour as it is.
        Errand::AskColour(initial) => way.ask(crate::line::Line::new(
            crate::icons::COLOUR_COMMAND,
            false,
            Some(&initial),
        )),
        Errand::Paste => way.paste(),
    }
    way.repaint();
}

/// Where the app is drawn in the footer's region: its tab line first, then
/// the view's own area — from where the region is, never from what it holds
/// (`the-footer-does-not-ask-itself`).
#[must_use]
pub fn area_of(region: &crate::region::Area) -> Area {
    let first = region.first;
    let line = tokens::metrics::line_height(tokens::text::size());
    let top = Area::y_of(first);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let lines = ((region.bottom - top).max(0.0) / line).floor() as u16;
    let columns = crate::chrome::Frame::of(region.width, region.bottom, first).columns();
    Area {
        left: crate::chrome::INSET,
        top: first,
        columns: columns.saturating_sub(2 * crate::chrome::INSET),
        lines,
        right_px: region.width,
        bottom_px: region.bottom,
    }
}

/// THE ICONS APP IN THE FOOTER'S REGION (`LEADER m i`).
pub struct IconsRegion {
    view: View,
    /// Where the tables' thread leaves them — ~9 MB is never read on a loop.
    tables: mpsc::Receiver<Result<Arc<Set>, Refusal>>,
    /// What the hand whose frame [`Region::pointer`] last took rests on — a
    /// target's code, nothing, or `None` for a frame that did not move it
    /// ([`Region::rests_on`]): the footer keeps each hand's, and lights each
    /// in its own colour.
    rests: Option<Option<usize>>,
}

impl std::fmt::Debug for IconsRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IconsRegion")
            .field("screen", &self.view.screen())
            .finish_non_exhaustive()
    }
}

impl IconsRegion {
    /// The app on `screen`, its tables read on a thread that wakes the door
    /// when they arrive — none in a photograph's footer, which has no loop
    /// to hear them on: the view says it is reading, as it always did there.
    fn opened(screen: Collection, way: &mut dyn Doorway) -> Self {
        let view = View::new(screen, way.texts());
        let (sender, tables) = mpsc::channel();
        if !way.photographing() {
            let wake = way.wake();
            crate::icons::read_the_tables(move |answer| {
                if sender.send(answer).is_ok() {
                    wake.call();
                }
            });
        }
        Self {
            view,
            tables,
            rests: None,
        }
    }
}

impl Region for IconsRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// No rows: blank lines, the app draws its tabs, grid and page over them.
    fn rows(&self, _: &Seen<'_>) -> Vec<Row> {
        Vec::new()
    }

    /// The grid's keys or the page's, `Esc` first.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        None
    }

    fn shape(&self) -> Shape {
        Shape::Cells
    }

    /// A key, read by its whole notation through the reader the window calls
    /// too (`icons::press_of_key`), walked against the area the region has
    /// on the screen the pointer is on.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        let Some(region) = way.area() else {
            return Taken::No;
        };
        let area = area_of(&region).below(1);
        let Some(press) = crate::icons::press_of_key(key, self.view.in_page()) else {
            return Taken::No;
        };
        let errand = self.view.key(press, area, way.texts());
        carry_out(errand, way);
        Taken::Yes
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // OPEN ALREADY: the app stays on the screen it shows.
            "icons-open" => way.repaint(),
            "icons-window" => {
                way.open_window(vec!["icons".to_owned(), self.view.screen().id().to_owned()])
            }
            _ if crate::icons::SCREEN_COMMANDS.iter().any(|(c, _)| *c == id) => {
                if let Some(screen) = crate::icons::screen_named(id) {
                    self.view.show(screen);
                }
                way.repaint();
            }
            // THE LUCIDE COLOUR: its answer comes back with the text; run by
            // its name (`LEADER :`), it ASKS.
            crate::icons::COLOUR_COMMAND => {
                match argument {
                    Some(text) => self.view.colour_answered(text),
                    None => way.ask(crate::line::Line::new(
                        crate::icons::COLOUR_COMMAND,
                        false,
                        Some(&self.view.colour()),
                    )),
                }
                way.repaint();
            }
            _ => return Taken::No,
        }
        Taken::Yes
    }

    fn clicked(&mut self, _: usize, _: &mut dyn Doorway) {}

    /// The Lucide colour, typed on the door's line.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        if id == crate::icons::COLOUR_COMMAND {
            self.view.colour_answered(text);
        }
        way.repaint();
    }

    /// The clipboard's text, into what is typed.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        let errand = self.view.pasted(text);
        carry_out(errand, way);
    }

    /// The tables arrived: handed to the view.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let mut took = false;
        while let Ok(answer) = self.tables.try_recv() {
            self.view.took(answer);
            took = true;
        }
        if took {
            way.repaint();
        }
    }

    /// Its tab line, the line that names the icon, the grid or the page —
    /// the drawing the window paints too, from the one layout its pointer
    /// reads; every hand's target in its own colour.
    fn paint(&self, easel: &mut RegionEasel<'_>) {
        let area = area_of(&easel.area);
        let lit: Vec<(tokens::Color, Target)> = easel
            .lit
            .iter()
            .map(|(c, n)| (*c, Target::of_code(*n)))
            .collect();
        let mut drawing = crate::icons::Drawing::default();
        self.view.draw(area, true, &lit, easel.texts, &mut drawing);
        easel.under.extend(drawing.under);
        easel.under.extend(drawing.thumbs);
        for track in drawing.tracks {
            easel.bars.record(track);
        }
        easel.spans.extend(drawing.spans);
        // A FIGURE ON ITS STAGE goes up only when its key — the document,
        // its side and the palette — is missing.
        for p in drawing.pictures {
            let key = p.key.clone();
            let area = p.area;
            easel.pictures.push(Painted {
                key,
                area,
                source: Source::Drawn(Arc::new(move || p.rgba())),
            });
        }
    }

    /// Its tabs, its cells, its rows and its bars, by the layout its drawing
    /// used — and what the hand rests on, kept for [`Region::rests_on`]. A
    /// frame that leaves the region, or crosses out of it, is the footer's
    /// own — with the hover gone with it.
    fn pointer(
        &mut self,
        events: &[Pointed],
        region: &crate::region::Area,
        way: &mut dyn Doorway,
    ) -> bool {
        // THE HAND'S AIM: unmoved until an event says otherwise.
        self.rests = None;
        let aim = &mut self.rests;
        let whole = area_of(region);
        let area = whole.below(1);
        let top = f64::from(Area::y_of(whole.top));
        let bottom = f64::from(region.bottom);
        let held = self.view.grabbed.is_some();
        let inside =
            |e: &Pointed| !matches!(e.pointing, Pointing::Left) && e.at.1 >= top && e.at.1 < bottom;
        if !held && !events.iter().all(inside) {
            // THE HOVER GOES WITH THE POINTER, and the footer takes the rest
            // — the hand's aim with it, which the footer lets go and repaints.
            *aim = Some(None);
            if self.view.hovered(None) | self.view.tab_hovered(None) {
                way.repaint();
            }
            return false;
        }
        let texts = way.texts();
        let cell_px = f64::from(crate::chrome::cell_width());
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let mut errands: Vec<Errand> = Vec::new();
        let mut keys: Vec<ronoctua_keymap::Key> = Vec::new();
        let mut turned: Option<f32> = None;
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (px, py) = (e.at.0 as f32, e.at.1 as f32);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let (column, line) = ((e.at.0 / cell_px) as u16, (e.at.1 / line_px) as u16);
            let v = &mut self.view;
            match e.pointing {
                Pointing::Moved => {
                    if v.grabbed.is_some() {
                        v.dragged(area, py, texts);
                    } else if line == whole.top {
                        // A TAB LIGHTS AND SAYS WHAT IT DOES, on the search
                        // line right under it — the line below the shortcuts
                        // every header explains them on.
                        let tab = v.tab_at(whole, texts, column, line);
                        v.tab_hovered(tab);
                        v.hovered(None);
                        *aim = Some(None);
                    } else {
                        v.tab_hovered(None);
                        let target = v.target_at(area, texts, px, py);
                        v.hovered(target);
                        // EACH HAND KEEPS ITS OWN TARGET, as a window's does,
                        // and is lit in its own colour (`seats`).
                        *aim = Some(target.map(Target::code));
                    }
                }
                Pointing::Left => {
                    v.hovered(None);
                    v.tab_hovered(None);
                    *aim = Some(None);
                }
                Pointing::Released(button) if button == LEFT_BUTTON => {
                    v.grabbed = None;
                }
                Pointing::Pressed(button) if button == LEFT_BUTTON => {
                    if line == whole.top {
                        if let Some(crate::chrome::Act::Key(k)) =
                            v.tab_press_at(whole, texts, column, line)
                        {
                            keys.push(k);
                        }
                    } else if let Some(t) = v.target_at(area, texts, px, py) {
                        errands.push(v.pressed(t, area, py, texts));
                    }
                }
                // THE BACK BUTTON IS `Esc` over the region, as everywhere.
                Pointing::Pressed(button) if button == BACK_BUTTON => {
                    keys.push(ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Esc));
                }
                Pointing::Wheel {
                    vertical,
                    horizontal,
                } => {
                    crate::wheel::wheel_arrived(&mut v.lines_owed, &vertical, &horizontal);
                    turned = Some(py);
                }
                Pointing::Pressed(_) | Pointing::Released(_) => {}
            }
        }
        if let Some(y) = turned {
            let by = crate::wheel::wheel_lines(&mut self.view.lines_owed);
            if by != 0 {
                self.view.wheeled(by, area, y, texts);
            }
        }
        for k in keys {
            let _ = self.key(k, way);
        }
        for e in errands {
            carry_out(e, way);
        }
        way.repaint();
        true
    }

    fn rests_on(&self) -> Option<Option<usize>> {
        self.rests
    }
}

/// What a command does with the app closed.
fn closed(command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "icons-open" => Closed::Opens,
        _ if crate::icons::SCREEN_COMMANDS
            .iter()
            .any(|(c, _)| *c == command) =>
        {
            Closed::Opens
        }
        "icons-window" => {
            way.open_window(vec![
                "icons".to_owned(),
                Collection::NerdFonts.id().to_owned(),
            ]);
            Closed::Taken
        }
        // WITH THE APP SHUT there is nothing to colour, which is SAID, never
        // silence — with the keys that open it.
        crate::icons::COLOUR_COMMAND => {
            let keys = crate::bindings::taught("icons-open", None).unwrap_or_default();
            let said = way
                .texts()
                .t_with(crate::key::ICONS_COLOUR_CLOSED, &[("keys", &keys)]);
            way.say(said, ronoctua_shell_kit::echo::Echo::Normal);
            Closed::Taken
        }
        _ => Closed::NotMine,
    }
}

/// `icons-open`, or a screen's command: the app on that screen.
fn open(command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let screen = crate::icons::screen_named(command).unwrap_or(Collection::NerdFonts);
    Box::new(IconsRegion::opened(screen, way))
}

/// The icons app, as the binary registers it with the footer.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        Recorder::new(
            std::rc::Rc::new(texts),
            80,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
    }

    /// FAILS IF: the Lucide colour asked with the app shut is silence, or
    /// opens an app nobody asked for.
    #[test]
    fn the_colour_with_the_app_shut_is_said() {
        let mut w = way();
        assert_eq!(
            closed(crate::icons::COLOUR_COMMAND, None, &mut w),
            Closed::Taken
        );
        assert!(
            matches!(w.owed.as_slice(), [Effect::Say(_, _)]),
            "{:?}",
            w.owed
        );
        assert_eq!(closed("icons-open", None, &mut w), Closed::Opens);
        assert_eq!(closed("stick-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: a screen's command does not open the app ON that screen.
    #[test]
    fn a_screens_command_opens_that_screen() {
        let mut w = way();
        let (command, screen) = crate::icons::SCREEN_COMMANDS[1];
        assert_eq!(closed(command, None, &mut w), Closed::Opens);
        let r = IconsRegion::opened(screen, &mut w);
        assert_eq!(r.view.screen(), screen);
        assert_eq!(r.shape(), Shape::Cells);
    }

    /// FAILS IF: an errand of the app is done differently in the two doors —
    /// the question it asks is not on the door's line, under the colour's
    /// id, starting from the colour as it is.
    #[test]
    fn the_colour_is_asked_on_the_doors_line() {
        let mut w = way();
        carry_out(Errand::AskColour("#ffffff".to_owned()), &mut w);
        match w.owed.as_slice() {
            [Effect::Ask(line), Effect::Repaint] => {
                assert_eq!(line.id, crate::icons::COLOUR_COMMAND);
            }
            other => panic!("{other:?}"),
        }
    }

    /// FAILS IF: the tables the region's thread reads never reach it — the
    /// door's wake not called, or `woken` not handing them to the view — or
    /// a photograph's footer, which has no loop to hear them on, starts the
    /// thread at all.
    #[test]
    fn the_tables_wake_the_door_and_a_photograph_reads_none() {
        let (woke, heard) = std::sync::mpsc::channel::<()>();
        let woke = std::sync::Mutex::new(woke);
        let texts = std::rc::Rc::new(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        let mut w = Recorder::new(
            std::rc::Rc::clone(&texts),
            80,
            false,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(move || {
                if let Ok(s) = woke.lock() {
                    let _ = s.send(());
                }
            }),
        );
        let mut r = IconsRegion::opened(Collection::NerdFonts, &mut w);
        heard
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("the tables' thread never woke the door");
        r.woken(&mut w);
        assert!(w.owed.contains(&Effect::Repaint), "{:?}", w.owed);
        assert!(
            r.tables.try_recv().is_err(),
            "`woken` left the tables on the channel"
        );

        let mut shot = way();
        let mut p = IconsRegion::opened(Collection::NerdFonts, &mut shot);
        p.woken(&mut shot);
        assert!(
            matches!(
                p.tables.try_recv(),
                Err(std::sync::mpsc::TryRecvError::Disconnected)
            ),
            "a photograph's footer started the tables' thread"
        );
        assert!(shot.owed.is_empty(), "{:?}", shot.owed);
    }

    /// FAILS IF: a frame that leaves the region keeps the hand's aim — a
    /// cell would stay lit under a pointer that is gone.
    #[test]
    fn a_hand_that_leaves_lets_its_aim_go() {
        let mut w = way();
        let mut r = IconsRegion::opened(Collection::NerdFonts, &mut w);
        let area = crate::region::Area {
            width: 1280.0,
            bottom: 400.0,
            first: 2,
            lines: 12,
            columns: 200,
        };
        let left = [Pointed {
            at: (10.0, 10.0),
            pointing: Pointing::Left,
        }];
        assert!(!r.pointer(&left, &area, &mut w));
        assert_eq!(r.rests_on(), Some(None), "the hand kept its aim");
    }
}
