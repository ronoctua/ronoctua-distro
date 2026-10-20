//! THE PERSON'S CONTAINERS IN THE FOOTER'S REGION (`LEADER s o o`) — the view
//! as a [`crate::region::Region`] the footer hosts by its id, beside its
//! window (`LEADER s o w`, [`crate::containers_window`]).
//!
//! # One view, two doors, one errand handler
//!
//! The region holds the view open ([`Open`]) as the window holds it, in the
//! stacked layout the footer's width draws, and hands it every key, click,
//! command, answer and thread's answer; what the view asks for after them
//! goes through [`crate::containers_open::carry_out`] in both doors, over
//! each door's [`Doorway`].
//!
//! # What only a region has to say
//!
//! - A row of several stretches — a kind on the strip, a tab — takes the
//!   click under the pointer's cell, and the pointer lights that stretch and
//!   nothing else ([`Region::clicked_at`], [`Region::lit`], one answer:
//!   `View::hover_span` and `View::clicked` read the same zones).
//! - A log followed at its end stays at its end while its end is in sight —
//!   the one time data moves the page, because the person asked for the tail
//!   ([`Doorway::follow_if_seen`]).
//! - The queue's jobs it gave are what the footer heard of the queue
//!   ([`Region::heard`]), told to it when it opens and every time the list
//!   changes.
//!
//! # What a photograph's footer reads
//!
//! Nothing: no feed, no podman asked — the view says it is asking.

use std::cell::Cell;

use crate::containers::{
    BUILD, LOGIN, LOGIN_PASSWORD, PULL, Press, View, answered, press_of, press_of_key,
};
use crate::containers_feed::Feed;
use crate::containers_open::{Open, carry_out, closed_press};
use crate::places::Layout;
use crate::region::{Closed, Doorway, Entry, Lit, Region, Row, Seen, Taken};

/// The id the footer ranks the view under (`surface::PRECEDENCE`).
pub const ID: &str = "containers";

/// The command that opens the view in the region (`LEADER s o o`).
const OPEN_COMMAND: &str = "containers-open";
/// The command that opens its window (`LEADER s o w`).
const WINDOW_COMMAND: &str = "containers-window";

/// The region the binary registers (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The footer draws the stacked layout, and pages it itself.
const LAYOUT: Layout = Layout::Stacked;

/// What a command of the view does while it is CLOSED: the view and the
/// login open it; its window opens without it. A typed question asked by
/// name — an image to pull, a folder to build — is asked all the same, and
/// its answer queued, with the view closed; any other key by name does
/// nothing with nothing to act on.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        OPEN_COMMAND => Closed::Opens,
        WINDOW_COMMAND => {
            way.open_window(vec![ID.to_owned()]);
            Closed::Taken
        }
        // NO LOGIN WAITS with the view closed: a password — typed or not —
        // is said to have nobody to go to.
        LOGIN_PASSWORD => {
            let said = way.texts().t(crate::key::CONTAINERS_REFUSED_NO_LOGIN);
            way.say(said, crate::echo::Echo::Normal);
            Closed::Taken
        }
        PULL | BUILD | LOGIN if argument.is_some_and(|a| !a.trim().is_empty()) => {
            let errand = answered_by_name(command, argument.unwrap_or_default(), way);
            carry_out(errand, None, LAYOUT, 0, way);
            Closed::Taken
        }
        other => match press_of(other) {
            Some(Press::Login) => Closed::Opens,
            Some(press) => {
                carry_out(closed_press(press), None, LAYOUT, 0, way);
                Closed::Taken
            }
            None => Closed::NotMine,
        },
    }
}

/// A typed question's answer, as the view hears it: a build's folder is
/// read under the person's home.
fn answered_by_name(id: &str, said: &str, way: &mut dyn Doorway) -> crate::containers::Errand {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    answered(id, said, home.as_deref(), way.texts())
}

/// Opens the view: its threads and podman's events alive while it is open,
/// and never before — a person who never opens it runs no `podman events`.
/// A photograph's footer asks podman nothing.
fn open(_command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let feed = (!way.photographing()).then(|| {
        let wake = way.wake();
        let mut feed = Feed::new(ronoctua_containers::Podman::from_env(), move || wake.call());
        feed.start();
        feed
    });
    let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
    Box::new(ContainersRegion {
        open: Open::new(feed, zone, crate::queue::queue_keys()),
        cursor_at: Cell::new(None),
    })
}

/// The view, open in the footer's region.
struct ContainersRegion {
    open: Open,
    /// The drawn line the cursor is on — counted where the width is known
    /// (after every call, and whenever the rows are drawn), since the
    /// footer's page asks for it with no width in hand.
    cursor_at: Cell<Option<usize>>,
}

impl ContainersRegion {
    /// The cursor's drawn line, counted again at `columns`.
    fn recount(&self, columns: u16, texts: &ronoctua_i18n::Texts) {
        let lines = self.open.lines(columns, LAYOUT, 0, texts);
        self.cursor_at
            .set(self.open.view.cursor_line(&lines, &self.open.readings));
    }

    /// The cursors in step, the streams made what is shown, and the cursor's
    /// line counted — never the page moved.
    fn settle(&mut self, way: &mut dyn Doorway) {
        self.open.settle(LAYOUT, 0, way);
        self.recount(way.columns(), way.texts());
        way.repaint();
    }

    /// A key, pressed — and the cursor's line counted after it.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        self.open.press(press, LAYOUT, 0, way);
        self.recount(way.columns(), way.texts());
    }
}

impl Region for ContainersRegion {
    fn id(&self) -> &'static str {
        ID
    }

    /// The stacked layout's lines, each clickable where it has something to
    /// take — which stretch, the click's cell says ([`Region::clicked_at`]).
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        let lines = self.open.lines(seen.columns, LAYOUT, 0, seen.texts);
        self.cursor_at
            .set(self.open.view.cursor_line(&lines, &self.open.readings));
        lines
            .into_iter()
            .map(|l| Row {
                takes: l.takes(),
                fields: l.fields,
            })
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        self.open.view.hint(&self.open.readings, LAYOUT, seen.texts)
    }

    fn cursor(&self) -> Option<usize> {
        self.cursor_at.get()
    }

    /// Every key read by its whole notation out of the view's table, through
    /// the reader the window calls too (`containers::press_of_key`). A key
    /// outside the table does nothing and does not close the view.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        match press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    /// The region's door (a second opening keeps the live view), the
    /// window's, the view's keys by name — and a typed question's answer by
    /// name. A command it takes is a gesture by name, after which the page
    /// follows the cursor, as after a key (`Doorway::follow`).
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            OPEN_COMMAND => {}
            WINDOW_COMMAND => way.open_window(vec![ID.to_owned()]),
            LOGIN_PASSWORD => self.open.password(argument, way),
            PULL | BUILD | LOGIN if argument.is_some_and(|a| !a.trim().is_empty()) => {
                let errand = answered_by_name(id, argument.unwrap_or_default(), way);
                carry_out(errand, Some(&mut self.open), LAYOUT, 0, way);
                self.recount(way.columns(), way.texts());
            }
            other => match press_of(other) {
                Some(press) => self.press(press, way),
                None => return Taken::No,
            },
        }
        way.follow();
        Taken::Yes
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.clicked_at(row, None, way);
    }

    /// What is under the pointer's cell on line `row` — a kind on the strip,
    /// a row, a tab, an answer — and the second click is `RET` (invariant 19).
    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let lines = self.open.lines(way.columns(), LAYOUT, 0, way.texts());
        let again = self
            .open
            .view
            .clicked(&lines, row, cell, LAYOUT, &self.open.readings);
        if again {
            self.press(Press::Enter, way);
        } else {
            self.settle(way);
        }
    }

    /// THE STRETCH A CLICK TAKES: one kind or one tab of a strip, never the
    /// whole line — the same zones the click reads (`View::hover_span`).
    fn lit(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Lit {
        let lines = self.open.lines(seen.columns, LAYOUT, 0, seen.texts);
        match cell.and_then(|c| View::hover_span(&lines, row, c)) {
            Some((from, to)) => Lit::Cells(from, to),
            None => Lit::Nothing,
        }
    }

    /// What was typed on the footer's line, under the id that asked: the
    /// login's password to podman's stdin, or the view's answer — and the
    /// page follows the cursor after it.
    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        if id == LOGIN_PASSWORD {
            self.open.password(Some(text), way);
        } else {
            let errand = answered_by_name(id, text, way);
            carry_out(errand, Some(&mut self.open), LAYOUT, 0, way);
            self.recount(way.columns(), way.texts());
        }
        way.follow();
    }

    /// The line was let go: a login waiting for its password goes with it.
    fn unanswered(&mut self, id: &'static str, _way: &mut dyn Doorway) {
        if id == LOGIN_PASSWORD {
            self.open.login = None;
        }
    }

    /// The threads brought something: take all of it; an action's end is said
    /// on the line. A LOG FOLLOWED AT ITS END stays at its end, if its end was
    /// in sight — wheeled away from it, the page stays.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let arrived = self.open.feed.as_ref().map(Feed::take).unwrap_or_default();
        if arrived.is_empty() {
            return;
        }
        let tail = if self.open.view.follows_log(&self.open.readings, LAYOUT) {
            let lines = self.open.lines(way.columns(), LAYOUT, 0, way.texts());
            self.open.view.cursor_line(&lines, &self.open.readings)
        } else {
            None
        };
        for a in arrived {
            if let Some(said) = self.open.readings.took(a, way.texts()) {
                way.say(said, crate::echo::Echo::Normal);
            }
        }
        self.settle(way);
        if let Some(was) = tail {
            way.follow_if_seen(was);
        }
    }

    /// The queue's list changed — or is told as the region opens: the
    /// containers' jobs, and a listing again when one of them ended.
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(jobs) = news.downcast_ref::<Vec<ronoctua_transfers::Job>>()
            && self.open.jobs_arrived(jobs)
        {
            self.settle(way);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB exists")
    }

    fn way() -> Recorder {
        Recorder::new(
            std::rc::Rc::new(texts()),
            100,
            true,
            crate::sudo::Gate::shut(),
            crate::wake::Wake::new(|| {}),
        )
    }

    /// The region as a photograph's footer opens it: no feed.
    fn region() -> ContainersRegion {
        let mut w = way();
        let mut r = ContainersRegion {
            open: Open::new(None, ronoctua_files::when::Zone::utc(), String::new()),
            cursor_at: Cell::new(None),
        };
        r.settle(&mut w);
        r
    }

    /// FAILS IF: a command of the view does to the CLOSED view something
    /// other than what the footer did when it held the view itself — the
    /// view and the login open it, its window opens without it, a pull by
    /// name is asked on the footer's line with the view closed, a key by
    /// name with nothing to act on does nothing, and another view's command
    /// is not its.
    #[test]
    fn a_closed_view_opens_for_its_own_commands() {
        let mut w = way();
        assert_eq!(closed(OPEN_COMMAND, None, &mut w), Closed::Opens);
        assert_eq!(closed("containers-login", None, &mut w), Closed::Opens);
        assert_eq!(closed(WINDOW_COMMAND, None, &mut w), Closed::Taken);
        assert_eq!(
            w.owed,
            vec![Effect::OpenWindow(vec!["containers".to_owned()])]
        );
        let mut w = way();
        assert_eq!(closed("containers-pull", None, &mut w), Closed::Taken);
        assert!(
            matches!(w.owed.as_slice(), [Effect::Ask(line), Effect::Repaint] if line.id == PULL),
            "{:?}",
            w.owed
        );
        let mut w = way();
        assert_eq!(closed("containers-down", None, &mut w), Closed::Taken);
        assert_eq!(w.owed, vec![Effect::Repaint]);
        assert_eq!(closed("incus-open", None, &mut w), Closed::NotMine);
        // A PASSWORD WITH NO LOGIN WAITING is said to reach nobody, typed or
        // not — as the footer said it when it held the view itself.
        for typed in [None, Some(""), Some("hunter2")] {
            let mut w = way();
            assert_eq!(closed(LOGIN_PASSWORD, typed, &mut w), Closed::Taken);
            assert!(
                matches!(w.owed.as_slice(), [Effect::Say(..)]),
                "{typed:?}: {:?}",
                w.owed
            );
        }
    }

    /// The region over three containers listed — and, `opened`, with the
    /// first one's detail open over the list: its title, its tabs, its
    /// facts, headings and notes.
    fn listed(opened: bool) -> ContainersRegion {
        let mut r = region();
        let mut w = way();
        r.open.readings.listing = crate::containers::tests::readings().listing;
        r.settle(&mut w);
        if opened {
            r.press(Press::Enter, &mut w);
        }
        r
    }

    /// FAILS IF: a resting pointer lights, at any cell of a line of the
    /// footer's region — the strip of kinds, a row of the list, the detail's
    /// title, a tab, a fact, a heading, a note, the air between them, the
    /// margin before them — a cell where a click takes nothing, or leaves
    /// unlit one where a click takes something; or lights anything with no
    /// cell under it. THE LIGHT IS THE CLICK, CELL BY CELL, as the footer
    /// lights a row (`Row::takes`, then `Region::lit`): every cell where a
    /// stretch begins or ends clicked once, on a view of its own, and lit
    /// exactly where something happened. The cursor's own place — the kind
    /// shown, the tab shown, the line the cursor is on — lights all the
    /// same, a click there having nothing left to do.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        use crate::containers::tests::{own_place, probes};
        let t = texts();
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 100,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        for opened in [false, true] {
            let first = listed(opened);
            let lines = first.open.lines(seen.columns, LAYOUT, 0, &t);
            let rows = first.rows(&seen);
            let cursor = first.cursor_in(&seen);
            let (mut lit_once, mut dark_once) = (false, false);
            let mut wrong = Vec::new();
            for (n, line) in lines.iter().enumerate() {
                assert_eq!(first.lit(n, None, &seen), Lit::Nothing, "line {n}");
                for cell in probes(line) {
                    let lit = rows[n].takes && first.lit(n, Some(cell), &seen) != Lit::Nothing;
                    lit_once |= lit;
                    dark_once |= !lit;
                    let mut r = listed(opened);
                    let mut w = way();
                    let before = crate::region::seen(&r, &w.owed, &seen);
                    r.clicked_at(n, Some(cell), &mut w);
                    let did = crate::region::seen(&r, &w.owed, &seen) != before;
                    let own = cursor == Some(n) || own_place(&first.open.view, line, cell);
                    if lit != did && !(lit && own) {
                        wrong.push(format!(
                            "opened {opened}, line {n}, cell {cell}: lit {lit}, did {did}"
                        ));
                    }
                }
            }
            assert!(lit_once && dark_once, "opened {opened}: all lit, or none");
            assert!(wrong.is_empty(), "{wrong:#?}");
        }
    }

    /// FAILS IF: an empty answer to one of the view's typed questions — an
    /// image to pull, a folder to build, a registry to log in to — asks the
    /// question again, says or queues anything in the footer, or does in the
    /// window something else: both doors read the view's own rule
    /// (`containers::answered`), and `Enter` on an emptied line is let go.
    #[test]
    fn an_empty_answer_is_nothing_in_either_door() {
        let t = texts();
        for id in [PULL, BUILD, LOGIN] {
            for said in ["", "   "] {
                let mut r = region();
                let mut w = way();
                r.answered(id, said, &mut w);
                assert!(
                    !w.owed.iter().any(|e| matches!(
                        e,
                        Effect::Ask(_) | Effect::Say(..) | Effect::Hold(_) | Effect::Queue(_)
                    )),
                    "{id} {said:?}: {:?}",
                    w.owed
                );
                assert_eq!(
                    answered(id, said, None, &t),
                    crate::containers::Errand::Nothing,
                    "{id} {said:?}: the window's answer"
                );
            }
        }
    }

    /// FAILS IF: the queue's list the footer heard is not taken as the
    /// view's jobs, or another kind of news is mistaken for it.
    #[test]
    fn the_queue_the_footer_heard_is_the_views() {
        let mut r = region();
        let mut w = way();
        let work = ronoctua_transfers::Work::Container {
            job: ronoctua_containers::Job::Pull {
                image: "alpine".to_owned(),
            },
            label: "alpine".to_owned(),
        };
        let jobs = vec![ronoctua_transfers::Job::fresh_one(1, work.engine(), work)];
        r.heard(&jobs, &mut w);
        assert_eq!(r.open.readings.jobs, jobs);
        r.heard(&"something else", &mut w);
        assert_eq!(r.open.readings.jobs, jobs);
    }
}
