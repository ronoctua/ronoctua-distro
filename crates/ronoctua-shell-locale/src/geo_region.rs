//! THE REGIONS' VIEW IN THE FOOTER'S REGION (`LEADER s g g`) — and the one
//! errand handler both its doors carry out ([`carry_out`]).
//!
//! # Why the errands are here, and handled once
//!
//! What the view asks — close, say a sentence, ask for the filter, put a
//! region in the queue — was written twice: the footer's `geo_errand` and the
//! window's `apply`, each over its own door, and the two had drifted (the
//! window read the directory again after queuing a region, the footer did
//! not). It is written once now, over the door's [`Doorway`]: the window's is
//! its `Ui`, the footer's a recorder that does what was asked once the region
//! is back in its slot. "The two doors do the same things" is a fact of the
//! types.
//!
//! # What the footer still decides
//!
//! Whether the region is free to open, where it is drawn and how many lines it
//! takes — the footer's; it knows this view by its id ([`ID`]) and the entry
//! the binary registers ([`ENTRY`]), and nothing else.

use crate::echo::Echo;
use crate::geo::{Errand, FIND, Press, View};
use crate::line::Line;
use crate::region::{Closed, Doorway, Entry, Seen, Taken};

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "geo";

/// The view as the binary registers it with the footer's slot host.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// Carries out what the view asked for, in whichever door it is drawn — the
/// footer's region or the window ([`crate::geo_window`]).
///
/// The install goes through the QUEUE and not through a thread here: it is
/// between six megabytes and two thousand, and `LEADER t` already survives
/// this process restarting, already knows how to give a job up, and already
/// answers "how far along".
pub fn carry_out(errand: Errand, view: &mut View, way: &mut dyn Doorway) {
    match errand {
        Errand::Nothing => {}
        Errand::Close => {
            way.close();
            return;
        }
        Errand::Speak(key) => {
            let said = way.texts().t(key);
            way.say(said, Echo::Normal);
        }
        // THE FILTER IS ASKED ON THE DOOR'S OWN LINE — the footer's
        // minibuffer, a window's header — under the same id, and the answer
        // comes back into the same `View::took_a_filter`. `false`: a filter is
        // read back and corrected, and dots are for a password.
        Errand::Ask => way.ask(Line::new(FIND, false, None)),
        Errand::Install { region, label, url } => {
            let Some(into) = ronoctua_xdg::data_dir().map(|d| ronoctua_atlas_packs::root_of(&d))
            else {
                let said = way.texts().t(crate::key::GEO_NO_HOME);
                way.say(said, Echo::Normal);
                way.repaint();
                return;
            };
            let work = ronoctua_transfers::Work::Atlas {
                region,
                label,
                url,
                into,
            };
            // THE QUEUE IS A DAEMON WITH A SOCKET, reachable from any process,
            // so either door puts the region in it the same way — and the
            // sentence says which happened.
            let said =
                match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
                    Ok(()) => way.texts().t(crate::key::GEO_QUEUED),
                    Err(e) => {
                        tracing::warn!(error = %e, "the transfer queue did not take the region");
                        way.texts().t(crate::key::TRANSFERS_OFFLINE)
                    }
                };
            way.say(said, Echo::Normal);
            view.reread(way.columns());
        }
    }
    way.repaint();
}

/// What a command of the view does while it is CLOSED.
///
/// The view's own gestures do nothing with no view to act on — exactly as the
/// key does — and are TAKEN, so no other part of the footer answers them;
/// the filter is ASKED even so, the way it always was (its answer finds no
/// view and is dropped); the window opens; and only `geo-open` opens the
/// region.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "geo-open" => Closed::Opens,
        "geo-window" => {
            way.open_window(vec!["geo".to_owned()]);
            Closed::Taken
        }
        "geo-find" if argument.is_none() => {
            way.ask(Line::new(FIND, false, None));
            Closed::Taken
        }
        "geo-up" | "geo-down" | "geo-enter" | "geo-remove" | "geo-find" => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// Opens the view — the footer asked whether the region is free first — at
/// the width the footer draws at: the cursor settles on the first row that
/// acts, and which row that is follows the width.
fn open(_: &str, _: Option<&str>, way: &mut dyn Doorway) -> Box<dyn crate::region::Region> {
    Box::new(GeoRegion {
        view: View::fresh(way.columns()),
        opened: false,
    })
}

/// The regions' view, as a region of the footer.
pub struct GeoRegion {
    view: View,
    /// Whether the command that opened it has run: a second open is
    /// refused, the first one is the opening itself.
    opened: bool,
}

impl GeoRegion {
    /// One press, from a key, a command or a second click.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let columns = way.columns();
        let errand = self.view.press(press, columns, way.texts());
        carry_out(errand, &mut self.view, way);
    }
}

impl crate::region::Region for GeoRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// A heading is not a row to click on, and neither is a note: the
    /// view's one answer ([`crate::geo::Row::takes`]) skips both here for the
    /// reason the cursor skips them.
    fn rows(&self, seen: &Seen<'_>) -> Vec<crate::region::Row> {
        self.view
            .rows(seen.columns, seen.texts)
            .into_iter()
            .map(|r| crate::region::Row {
                takes: r.takes(),
                fields: r.fields,
            })
            .collect()
    }

    /// Two of its five keys are bare letters, so without the line there is
    /// nothing on screen saying that `d` removes one.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view.hint(seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// Two arrows, `RET`, and two bare letters — the letters are free here:
    /// the filter is typed on a line of its own (the minibuffer, which takes
    /// every key while it asks), so a letter on this screen is not a query.
    /// Read by the whole notation through the reader the window calls too
    /// (`geo::press_of_key`): `C-f` is not `f` in either door.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match crate::geo::press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // THE COMMAND THAT OPENED IT, the first time: nothing more to do.
            "geo-open" if !self.opened => self.opened = true,
            // A SECOND OPEN IS REFUSED, as every view that asks whether the
            // region is free refuses with the region held — here, by itself:
            // the live view is kept, with its cursor.
            "geo-open" => {
                tracing::info!(
                    view = id,
                    held = ID,
                    "refused: another view is open in the footer"
                );
                let said = way.texts().t(crate::key::ECHO_VIEW_BUSY);
                way.say(said, Echo::Normal);
                way.repaint();
            }
            "geo-window" => way.open_window(vec!["geo".to_owned()]),
            "geo-up" => self.press(Press::Up, way),
            "geo-down" => self.press(Press::Down, way),
            "geo-enter" => self.press(Press::Enter, way),
            "geo-remove" => self.press(Press::Remove, way),
            // ASKED AND THEN TOLD: the command runs twice, once to open the
            // question and once with what was typed — the same
            // `took_a_filter` a window's line answers into.
            "geo-find" => match argument {
                Some(said) => {
                    self.view.took_a_filter(said, way.columns());
                    way.repaint();
                }
                None => carry_out(Errand::Ask, &mut self.view, way),
            },
            _ => return Taken::No,
        }
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let rows = self.view.rows(way.columns(), way.texts());
        if self.view.clicked(row, &rows) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    /// THE FILTER, typed on the footer's line.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        if id == FIND {
            self.view.took_a_filter(text, way.columns());
            way.repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder, Region};

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        Recorder::new(
            std::rc::Rc::new(texts),
            80,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        )
    }

    fn region() -> GeoRegion {
        let root = std::env::temp_dir().join("ronoctua-geo-region-tests-none");
        GeoRegion {
            view: View::at(root, 80),
            opened: true,
        }
    }

    /// FAILS IF: the command that opened the view is refused as a second
    /// open — the view would open saying another view holds the region — or
    /// a second open is not refused.
    #[test]
    fn the_first_open_is_the_opening_and_a_second_is_refused() {
        let mut w = way();
        let mut r = region();
        r.opened = false;
        assert_eq!(r.command("geo-open", None, &mut w), Taken::Yes);
        assert!(
            w.owed.is_empty(),
            "the opening said something: {:?}",
            w.owed
        );
        assert_eq!(r.command("geo-open", None, &mut w), Taken::Yes);
        let busy = w.texts().t(crate::key::ECHO_VIEW_BUSY);
        assert_eq!(w.owed.first(), Some(&Effect::Say(busy, Echo::Normal)));
    }

    /// FAILS IF: a closed view answers a command that is not its own, opens
    /// on one of its gestures, or stops asking the filter by name — the
    /// footer would open a view nobody asked for, or `M-x geo-find` would do
    /// nothing.
    #[test]
    fn a_closed_view_takes_its_gestures_and_opens_on_its_open() {
        let mut w = way();
        assert_eq!(closed("geo-open", None, &mut w), Closed::Opens);
        assert_eq!(closed("geo-down", None, &mut w), Closed::Taken);
        assert_eq!(closed("geo-find", Some("x"), &mut w), Closed::Taken);
        assert!(w.owed.is_empty(), "a gesture with no view did something");
        assert_eq!(closed("geo-find", None, &mut w), Closed::Taken);
        assert_eq!(w.owed, vec![Effect::Ask(Line::new(FIND, false, None))]);
        assert_eq!(closed("locale-open", None, &mut w), Closed::NotMine);
        assert_eq!(closed("geo-window", None, &mut w), Closed::Taken);
        assert_eq!(
            w.owed.last(),
            Some(&Effect::OpenWindow(vec!["geo".to_owned()]))
        );
    }

    /// FAILS IF: the errand handler does a thing in one door that it does
    /// not ask of the door — a close that does not close, a question not
    /// asked on the door's line, a sentence not said.
    #[test]
    fn the_errands_are_asked_of_the_door() {
        let mut w = way();
        let mut v = region().view;
        carry_out(Errand::Ask, &mut v, &mut w);
        carry_out(Errand::Speak(crate::key::GEO_QUEUED), &mut v, &mut w);
        carry_out(Errand::Close, &mut v, &mut w);
        let said = w.texts().t(crate::key::GEO_QUEUED);
        assert_eq!(
            w.owed,
            vec![
                Effect::Ask(Line::new(FIND, false, None)),
                Effect::Repaint,
                Effect::Say(said, Echo::Normal),
                Effect::Repaint,
                Effect::Close,
            ]
        );
    }

    /// FAILS IF: the filter typed on the footer's line, or by name with an
    /// argument, does not reach the view, or a key not in the view's table
    /// is taken.
    #[test]
    fn the_filter_and_the_keys_reach_the_view() {
        let mut w = way();
        let mut r = region();
        assert_eq!(r.command("geo-find", Some(" brazil "), &mut w), Taken::Yes);
        assert_eq!(r.view.filter(), "brazil");
        r.answered(FIND, "", &mut w);
        assert_eq!(r.view.filter(), "");
        let chord = ronoctua_keymap::Key::parse("C-f").expect("a key");
        assert_eq!(r.key(chord, &mut w), Taken::No);
        let down = ronoctua_keymap::Key::parse("<down>").expect("a key");
        assert_eq!(r.key(down, &mut w), Taken::Yes);
        assert_eq!(r.command("network-open", None, &mut w), Taken::No);
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, the air between sections, a note —
    /// or leaves unlit one it takes. THE LIGHT IS THE CLICK: every row
    /// clicked, on a view of its own over a planted catalogue
    /// (`View::planted`), and lit exactly where something was seen to happen.
    ///
    /// CLICKED ONCE, AND NEVER ON THE CURSOR'S ROW — as
    /// `region::held_to_the_click` clicks too: a second click would be the `RET`, and
    /// a `RET` on a region puts a download in this machine's transfer queue
    /// (`carry_out`), which no test may do. One click is enough for the
    /// light: from any other row it selects, and the cursor moving is seen;
    /// on the cursor's row it is already the `RET`, and that row is lit by
    /// being the cursor's (`Held::here`).
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let fresh = || GeoRegion {
            view: View::planted(
                std::env::temp_dir().join("ronoctua-geo-region-tests-none"),
                80,
            ),
            opened: true,
        };
        let t = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 80,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let first = fresh();
        let cursor = first.cursor_in(&seen);
        let held: Vec<crate::region::Held> = first
            .rows(&seen)
            .iter()
            .enumerate()
            .map(|(row, drawn)| {
                let here = cursor == Some(row);
                let (mut r, mut w) = (fresh(), way());
                let before = crate::region::seen(&r, &w.owed, &seen);
                if !here {
                    r.clicked(row, &mut w);
                }
                crate::region::Held {
                    row,
                    lit: drawn.takes,
                    did: crate::region::seen(&r, &w.owed, &seen) != before,
                    here,
                }
            })
            .collect();
        assert!(
            held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
            "no row lit, or every row lit: {held:?}"
        );
        assert!(
            held.iter().all(|h| !h.here || h.lit),
            "the cursor stands on a row a click does not take: {held:?}"
        );
        let unheld = crate::region::unheld(&held);
        assert!(unheld.is_empty(), "{unheld:?}");
    }
}
