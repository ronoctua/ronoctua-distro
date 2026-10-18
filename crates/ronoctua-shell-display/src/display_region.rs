//! THE SCREENS' VIEW IN THE FOOTER'S REGION (`LEADER s d d`) — and the one
//! handler both its doors carry its errands out with ([`carry_out`]).
//!
//! # What is shared, and what each door keeps
//!
//! What an errand MEANS is the view's table (`display::doing`, and
//! `display::refusal` for an order the screens refused); what a door does
//! with it — a sentence on its line, an order to the reader, the view over —
//! is [`carry_out`], over the door's [`Doorway`]. What stays each door's is
//! what a door genuinely is: where the reader lives and how a close ends.
//!
//! # The reader is the footer's
//!
//! The footer starts the screens' reader at login — the brightness keys order
//! it with no view open, and every hotplug refreshes what it read — so this
//! region opens on that reading, and orders that thread through a line the
//! footer hands in (`ronoctua_display::Orders`): a second reader of its own
//! would be a second truth about one set of screens, and a preview it applied
//! could be put back only by the thread that applied it.
//!
//! # A standing preview goes back when the region goes
//!
//! A change that can blind a screen is applied provisionally and held up by a
//! question with a clock. When the region is closed under it — the footer
//! ending, above all — [`DisplayRegion`]'s `closing` puts it back through the
//! line before the footer waits for the reader (`Handle::finish`), which is
//! also why the line goes with the region: the reader's loop ends only when
//! every sender is gone.

use std::time::Instant;

use crate::display::{Doing, Errand, Press, View};
use crate::echo::Echo;
use crate::region::{Closed, Doorway, Entry, Seen, Taken};

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "display";

/// The view as the binary registers it with the footer's slot host.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// Carries out what the screens' view asked of a door — a sentence on its
/// line, an order to its reader (or why there is none), the view over — in
/// whichever door it is drawn: the footer's region, or the window
/// ([`crate::display_window`]), whose close is its own leaving and is taken
/// out before this is called.
pub fn carry_out(doing: Doing, orders: Option<&ronoctua_display::Orders>, way: &mut dyn Doorway) {
    if let Some(said) = doing.said {
        way.say(said, Echo::Normal);
    }
    if let Some(order) = doing.order {
        match orders {
            Some(line) => line.send(order),
            // A DOOR WITH NO READER SAYS SO, in the view's words: one
            // refusal, the same answer in both doors.
            None => {
                let said = crate::display::no_thread(way.texts());
                way.say(said, Echo::Normal);
            }
        }
    }
    if doing.close {
        way.close();
    }
}

/// What a command of the view does while it is CLOSED: its gestures, reachable
/// by name as well as by key, do nothing with no view to act on — exactly as
/// the key does — and are TAKEN; the window opens; `display-open` opens the
/// region.
fn closed(command: &str, _: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "display-open" => Closed::Opens,
        "display-window" => {
            way.open_window(vec!["display".to_owned()]);
            Closed::Taken
        }
        _ if press_of_command(command).is_some() => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// The view's gestures by their command ids — what `LEADER :` and the manual
/// reach them by.
fn press_of_command(id: &str) -> Option<Press> {
    Some(match id {
        "display-up" => Press::Up,
        "display-down" => Press::Down,
        "display-less" => Press::Less,
        "display-more" => Press::More,
        "display-enter" => Press::Enter,
        "display-automatic" => Press::Automatic,
        "display-forget" => Press::Forget,
        "display-arrange" => Press::Arrange,
        "display-rise" => Press::Rise,
        "display-sink" => Press::Sink,
        _ => return None,
    })
}

/// Opens the view — the footer asked whether the region is free first. What
/// the footer holds (its reading, its line to the reader, whether it watches
/// `display.json`) is handed in before the command that opened it, which
/// settles the view on it ([`DisplayRegion::opened`]).
fn open(_: &str, _: Option<&str>, _: &mut dyn Doorway) -> Box<dyn crate::region::Region> {
    Box::new(DisplayRegion {
        view: View::fresh(),
        snapshot: ronoctua_display::Snapshot::default(),
        orders: None,
        unwatched: false,
        opened: false,
    })
}

/// The screens' view, as a region of the footer.
pub struct DisplayRegion {
    view: View,
    /// The last reading the footer's reader brought.
    snapshot: ronoctua_display::Snapshot,
    /// The line to the footer's reader, when it started.
    orders: Option<ronoctua_display::Orders>,
    /// Whether the footer could not watch `display.json`.
    unwatched: bool,
    /// Whether the command that opened it has run: the view is settled on
    /// what the footer handed in once, and a second open only gives the
    /// keyboard back.
    opened: bool,
}

impl DisplayRegion {
    /// The command that opened the view, on what the footer handed in.
    ///
    /// THE READING IS ASKED FOR, not waited on: the view opens on whatever
    /// the last one said and the answer redraws it a moment later. AND THE
    /// FOOTER HAS ALREADY BEEN READ TO, all but the first time — its reader
    /// starts at login and every hotplug refreshes it — so a snapshot on hand
    /// is current, and a fresh `View` saying otherwise drew "Asking the
    /// compositor…" over a complete list. A view with no reader says so,
    /// rather than blaming the compositor; and a footer that could not watch
    /// `display.json` does not step the size of the text (`View::blind`).
    fn opened(&mut self) {
        self.opened = true;
        match self.orders.as_ref() {
            Some(line) => line.send(ronoctua_display::Order::Look { deep: true }),
            None => self.view.without_a_reader(),
        }
        if !self.snapshot.screens.is_empty() || self.snapshot.compositor {
            self.view.saw_a_reading();
        }
        if self.unwatched {
            self.view.without_a_watch();
        }
    }

    /// The rows now, at the door's width.
    fn rows_of(&self, way: &dyn Doorway, now: Instant) -> Vec<crate::display::Row> {
        self.view
            .rows(&self.snapshot, way.columns(), now, way.texts())
    }

    /// One press of the view — a key, a command by name, a second click —
    /// at the width the region draws at (see `display::View::key`); the page
    /// follows it, as after a person's key.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = self.view.key(
            press,
            &self.snapshot,
            way.columns(),
            Instant::now(),
            way.texts(),
        );
        self.errand(errand, way);
    }

    /// What the view asked for, through the table both doors read.
    fn errand(&mut self, errand: Errand, way: &mut dyn Doorway) {
        let doing = crate::display::doing(errand, way.texts());
        carry_out(doing, self.orders.as_ref(), way);
        way.repaint();
    }
}

impl crate::region::Region for DisplayRegion {
    fn id(&self) -> &'static str {
        ID
    }

    fn rows(&self, seen: &Seen<'_>) -> Vec<crate::region::Row> {
        self.view
            .rows(&self.snapshot, seen.columns, Instant::now(), seen.texts)
            .into_iter()
            .map(|r| crate::region::Row {
                takes: !r.heading,
                fields: r.fields,
            })
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.view
            .hint(&self.snapshot, seen.columns, Instant::now(), seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        Some(self.view.cursor())
    }

    /// The arrows, `RET`, `Esc` and three bare letters, read by their WHOLE
    /// notation through the reader the window calls too
    /// (`display_words::press_of_key`) — a row added to `display::OF_THE_VIEW`
    /// is a key that works, and a key here that is not there is a key nothing
    /// teaches.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match crate::display_words::press_of_key(key) {
            Some(press) => {
                // THE SURFACE FIRST, THEN THE CURSOR: the list of screens is
                // six rows and one screen's sheet eighteen, and the page
                // measured against the old height left the scale, the turn and
                // the place unreachable (`Doorway::follow`).
                self.press(press, way);
                way.follow();
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    fn command(&mut self, id: &str, _: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // A DOOR ONTO A ROOM ONE IS ALREADY IN just gives the keyboard
            // back — pressing the key twice should not throw away where one
            // had walked to. The first time, the view settles on what the
            // footer handed in.
            "display-open" => {
                if !self.opened {
                    self.opened();
                }
                way.follow();
                way.repaint();
            }
            "display-window" => way.open_window(vec!["display".to_owned()]),
            // THE VIEW'S OWN GESTURES, reachable by NAME as well as by key —
            // which is what `LEADER :` needs and what the manual lists.
            other => match press_of_command(other) {
                Some(press) => {
                    self.press(press, way);
                    way.follow();
                }
                None => return Taken::No,
            },
        }
        Taken::Yes
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19: a click on
    /// the row the cursor is already on is that row's `RET`.
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let rows = self.rows_of(way, Instant::now());
        if self.view.clicked(row, &rows) {
            self.press(Press::Enter, way);
            way.follow();
        } else {
            way.repaint();
        }
    }

    /// What the footer heard of the screens: on opening, its reading, its
    /// line to the reader and whether it watches `display.json`; then each
    /// reading and each order finished.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(snapshot) = news.downcast_ref::<ronoctua_display::Snapshot>() {
            self.snapshot = snapshot.clone();
        } else if let Some(line) = news.downcast_ref::<ronoctua_display::Orders>() {
            self.orders = Some(line.clone());
        } else if news.downcast_ref::<crate::toolkits::Unwatched>().is_some() {
            self.unwatched = true;
        } else if let Some(event) = news.downcast_ref::<ronoctua_display::Event>() {
            match event {
                // AND THE VIEW KNOWS IT HAS BEEN READ TO (`View::looked`).
                // ARRIVAL CLAMPS, a person's key follows — `settle_region`'s
                // rule: a reading that moved the page back to the cursor would
                // undo a scroll somebody made while it was in flight.
                ronoctua_display::Event::Snapshot(now) => {
                    self.snapshot = (**now).clone();
                    self.view.saw_a_reading();
                    let rows = self.rows_of(way, Instant::now());
                    self.view.settle(&self.snapshot, &rows);
                    way.repaint();
                }
                // EVERYTHING A REFUSAL DOES TO THE VIEW is `display::refusal`,
                // and the window's door reads the same one. The page follows
                // only when a question came down; a daemon's arrival does not
                // otherwise follow the cursor.
                ronoctua_display::Event::Done {
                    order,
                    refused: Some(why),
                } => {
                    let doing = crate::display::refusal(
                        &mut self.view,
                        order,
                        why,
                        &self.snapshot,
                        way.columns(),
                        Instant::now(),
                        way.texts(),
                    );
                    let follow = doing.follow;
                    carry_out(doing, self.orders.as_ref(), way);
                    if follow {
                        way.follow();
                    }
                    way.repaint();
                }
                ronoctua_display::Event::Done { refused: None, .. } => {}
            }
        }
    }

    /// The clock on a provisional change, one second at a time — the
    /// footer's own second: a view does not get a timer of its own.
    fn tick(&mut self, now: Instant, way: &mut dyn Doorway) -> bool {
        // ONLY A QUESTION WITH A CLOCK. A forget question stands until
        // somebody answers it — nothing on it counts down — so ticking for it
        // is a repaint a second, for ever, on a footer whose whole rule is
        // that at rest it wakes up for nothing.
        if !self
            .view
            .asking()
            .is_some_and(crate::display::Asking::on_the_clock)
        {
            return false;
        }
        let errand = self.view.ran_out(now);
        if !matches!(errand, Errand::Nothing) {
            // AND IT SAYS SO, as the window's door does: the VM reads it.
            tracing::info!("the countdown ran out; putting the change back");
        }
        // THE SECONDS ON THE SCREEN COUNT DOWN whether or not it ran out, so
        // the frame is drawn either way — and the cursor settles on the rows
        // that come back: `ran_out` restores the remembered TARGET, and only
        // `key` settles an INDEX. THE PAGE FOLLOWS ONLY WHEN THE ROWS
        // CHANGED, which for a tick means the countdown ran out and the sheet
        // came back.
        let rows = self.rows_of(way, now);
        self.view.settle(&self.snapshot, &rows);
        let came_back = matches!(errand, Errand::Order(_));
        self.errand(errand, way);
        if came_back {
            way.follow();
        }
        true
    }

    /// A PREVIEW STANDING AS THE REGION GOES is put back first, through the
    /// line to the reader — the footer ending, above all, which then waits
    /// for the reader (`Handle::finish`) — and the line goes with the region.
    fn closing(&mut self, _: &mut dyn Doorway) {
        let standing = self
            .view
            .asking()
            .is_some_and(crate::display::Asking::on_the_clock);
        if let (true, Some(line)) = (standing, self.orders.as_ref()) {
            tracing::info!("closing over a standing preview; putting it back first");
            line.send(ronoctua_display::Order::Undo);
        }
        self.orders = None;
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

    fn opened(line: Option<ronoctua_display::Orders>) -> DisplayRegion {
        DisplayRegion {
            view: View::fresh(),
            snapshot: ronoctua_display::Snapshot::default(),
            orders: line,
            unwatched: false,
            opened: false,
        }
    }

    /// FAILS IF: an order reaches no reader without a word, or a sentence of
    /// the view's table is not said, or a close is not asked of the door.
    #[test]
    fn what_a_door_is_asked_is_done_through_it() {
        let mut w = way();
        carry_out(
            Doing {
                order: Some(ronoctua_display::Order::Undo),
                ..Doing::default()
            },
            None,
            &mut w,
        );
        let no_thread = crate::display::no_thread(w.texts());
        assert_eq!(w.owed, vec![Effect::Say(no_thread, Echo::Normal)]);
        let (line, inbox) = ronoctua_display::Orders::detached();
        let mut w = way();
        carry_out(
            Doing {
                said: Some("said".to_owned()),
                order: Some(ronoctua_display::Order::Undo),
                close: true,
                follow: false,
            },
            Some(&line),
            &mut w,
        );
        assert_eq!(inbox.try_recv().ok(), Some(ronoctua_display::Order::Undo));
        assert_eq!(
            w.owed,
            vec![Effect::Say("said".to_owned(), Echo::Normal), Effect::Close]
        );
    }

    /// FAILS IF: the region opens with no word of a missing reader, or asks
    /// the reader nothing on opening — the view would blame the compositor,
    /// or stay on the footer's old reading.
    #[test]
    fn the_first_open_settles_on_what_the_footer_holds() {
        let mut w = way();
        let mut r = opened(None);
        assert_eq!(r.command("display-open", None, &mut w), Taken::Yes);
        assert!(r.opened);
        assert!(w.owed.contains(&Effect::Follow));
        let (line, inbox) = ronoctua_display::Orders::detached();
        let mut r = opened(Some(line));
        let mut w = way();
        r.command("display-open", None, &mut w);
        assert_eq!(
            inbox.recv_timeout(std::time::Duration::from_secs(1)).ok(),
            Some(ronoctua_display::Order::Look { deep: true })
        );
        // A SECOND OPEN only gives the keyboard back: no second reading.
        r.command("display-open", None, &mut w);
        assert!(inbox.try_recv().is_err(), "a second open asked again");
    }

    /// FAILS IF: a command of the view by name is answered with the view
    /// closed by anything but nothing, or one not its own is taken.
    #[test]
    fn a_closed_view_takes_its_gestures_and_nothing_else() {
        let mut w = way();
        assert_eq!(closed("display-open", None, &mut w), Closed::Opens);
        assert_eq!(closed("display-sink", None, &mut w), Closed::Taken);
        assert!(w.owed.is_empty());
        assert_eq!(closed("display-window", None, &mut w), Closed::Taken);
        assert_eq!(w.owed, vec![Effect::OpenWindow(vec!["display".to_owned()])]);
        assert_eq!(closed("bluetooth-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: a region closed under a standing preview does not put it
    /// back through the line — the change would stand with nothing left to
    /// undo it — or keeps the line, which holds the reader's loop open, or
    /// sends an `Undo` with nothing standing.
    #[test]
    fn a_standing_preview_goes_back_as_the_region_goes() {
        let (line, inbox) = ronoctua_display::Orders::detached();
        let mut r = opened(Some(line));
        let mut w = way();
        r.closing(&mut w);
        assert!(inbox.try_recv().is_err(), "an Undo with nothing standing");
        assert!(r.orders.is_none(), "the line stayed with a closed region");

        let (line, inbox) = ronoctua_display::Orders::detached();
        let mut r = opened(Some(line));
        r.view = View::fresh().with_a_preview_standing("eDP-1", Instant::now());
        r.closing(&mut w);
        assert_eq!(
            inbox.try_recv().ok(),
            Some(ronoctua_display::Order::Undo),
            "a preview stood as the region went"
        );
        assert!(r.orders.is_none(), "the line stayed with a closed region");
        // AND THE LINE WENT WITH THE REGION: the reader's loop is free to end.
        assert!(matches!(
            inbox.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Disconnected)
        ));
    }

    /// FAILS IF: the footer's second does not count a standing preview down
    /// — it would stand for ever with nobody looking — or ticks for nothing
    /// (a repaint a second on a footer at rest), or the countdown that ran
    /// out does not put the change back through the line and follow the
    /// sheet that comes back.
    #[test]
    fn the_footers_second_counts_a_preview_down() {
        let (line, inbox) = ronoctua_display::Orders::detached();
        let mut r = opened(Some(line));
        let mut w = way();
        assert!(
            !r.tick(Instant::now(), &mut w),
            "it ticked with nothing standing"
        );
        assert!(w.owed.is_empty());
        let began = Instant::now();
        r.view = View::fresh().with_a_preview_standing("eDP-1", began);
        assert!(r.tick(began, &mut w), "a standing preview did not count");
        assert!(inbox.try_recv().is_err(), "it went back at once");
        assert!(!w.owed.contains(&Effect::Follow), "a second moved the page");
        let mut w = way();
        let out = began + crate::display::PATIENCE;
        assert!(r.tick(out, &mut w));
        assert_eq!(inbox.try_recv().ok(), Some(ronoctua_display::Order::Undo));
        assert!(
            w.owed.contains(&Effect::Follow),
            "the sheet came back unfollowed"
        );
        assert!(r.view.asking().is_none());
    }
}
