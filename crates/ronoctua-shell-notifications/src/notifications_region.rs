//! THE NOTIFICATION MANAGER IN THE FOOTER'S REGION (`LEADER n n`) — the
//! manager as a [`Region`] the footer hosts by its id.
//!
//! # What the footer keeps, and what this is
//!
//! The footer keeps the notification DESK: the queue its band draws, the
//! person's filter book that decides what the band shows, the history it
//! writes and the bus every sender waits on. The manager is a view of it,
//! and holds a cursor, never a record (`notifications.rs`): the queue and the
//! book are LENT to it for each call ([`crate::region::Lent`]), and what it
//! changes it ASKS of the desk ([`crate::desk::Ask`]), which the footer does
//! once the region is back in its slot — and right after each ask tells the
//! manager the lists moved ([`crate::desk::Settle`]), so its cursor is back
//! on a row that exists before anything it asked after the ask is done.
//!
//! Its other door is a window of its own (`notifications_window.rs`), which
//! keeps its desk in the two files the footer watches; what an errand comes
//! to is the one handler both call (`notifications::carry_out`).

use crate::desk::{Ask, Deed, Settle};
use crate::notifications::{Desk, Half, Lists, Pane, Press, View};
use crate::region::{
    Area, Closed, Doorway, Entry, Grip, Lent, Paging, Region, RegionEasel, Row, Seen, Taken,
};
use ronoctua_notify::{Book, Queue, RecordKey};
use ronoctua_tokens as tokens;

/// The id the footer ranks the manager under (`PRECEDENCE`).
pub const ID: &str = "notifications";

/// The manager, as the binary registers it (`regions.rs`).
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// The manager, open in the footer.
struct Manager {
    view: View,
    /// Whether the command that opened it has been handed to it: the slot
    /// host hands the opening command to the region it just opened, and the
    /// SAME command later is the toggle that closes it.
    shown: bool,
}

/// The grip an open record's bar is held by — the half it scrolls — and
/// back. Which half a POINTER holds is the footer's to keep, one per seat
/// ([`Grip`]): two pointers may hold the two bars at once.
const fn grip_of(half: Half) -> Grip {
    Grip(match half {
        Half::First => 0,
        Half::Second => 1,
    })
}

/// The half grip `grip` holds, `None` for a grip that is no half's.
const fn half_of(grip: Grip) -> Option<Half> {
    match grip.0 {
        0 => Some(Half::First),
        1 => Some(Half::Second),
        _ => None,
    }
}

/// The queue's records and the book, as the footer lends them.
fn lists_of<'a>(lent: &Lent<'a>) -> Option<(Lists<'a>, &'a Book)> {
    let queue = lent.get::<Queue>()?;
    let book = lent.get::<Book>()?;
    Some((Lists::of(queue.history(), book), book))
}

/// THE FOOTER'S DESK, from inside the region: everything a record or the
/// book is asked is ASKED of the footer, which keeps them.
struct ToTheFooter;

impl Desk for ToTheFooter {
    fn read(&mut self, key: RecordKey, read: bool, way: &mut dyn Doorway) {
        way.desk(if read {
            Ask::Read(key)
        } else {
            Ask::Unread(key)
        });
    }

    fn read_all(&mut self, way: &mut dyn Doorway) {
        way.desk(Ask::ReadAll);
    }

    fn forget(&mut self, key: RecordKey, way: &mut dyn Doorway) {
        way.desk(Ask::Forget(key));
    }

    /// A COMMAND OF OURS MAY WANT THE REGION, so the manager gets out of its
    /// way first — the defect the owner reported: `[b] bluetooth` on a
    /// disconnection did nothing at all, because the command asked for the
    /// region and the manager was holding it. Only for a command: copying a
    /// path and running a program leave the screen where it is.
    fn act(&mut self, key: RecordKey, which: usize, way: &mut dyn Doorway) {
        let commands = way
            .lent()
            .get::<Queue>()
            .and_then(|q| q.history().iter().find(|r| r.key() == key))
            .and_then(|r| crate::desk::deed_of(r, which))
            .is_some_and(|d| matches!(d, Deed::Command(_)));
        if commands {
            way.close();
        }
        way.desk(Ask::Act(key, which));
    }

    fn adjust(
        &mut self,
        id: String,
        verdict: Option<ronoctua_notify::presentation::Verdict>,
        way: &mut dyn Doorway,
    ) {
        way.desk(Ask::Adjusted(id, verdict));
    }

    fn filter(&mut self, was: Option<String>, rule: ronoctua_notify::Rule, way: &mut dyn Doorway) {
        way.desk(Ask::Filter { was, rule });
    }

    fn unfilter(&mut self, id: String, way: &mut dyn Doorway) {
        way.desk(Ask::Unfilter(id));
    }

    /// The footer writes it, and says how it went.
    fn share(&mut self, id: String, way: &mut dyn Doorway) {
        way.desk(Ask::Share(id));
    }
}

/// The commands that walk and act on the manager's own rows, by name: with
/// the manager closed there is no focused row, and that is the honest
/// answer — they do nothing (the shape `network-connect` has).
fn of_the_view(command: &str) -> bool {
    matches!(
        command,
        "notif-filter-name"
            | "notif-filter-value"
            | "notif-new"
            | "notif-drop"
            | "notif-share"
            | "notif-share-answer"
            | "notif-draft-type"
            | "notif-draft-add"
            | "notif-draft-drop"
            | "notif-draft-save"
            | "notif-read"
            | "notif-forget"
            | "notif-adjust"
            | "notif-up"
            | "notif-down"
            | "notif-pane-next"
            | "notif-pane-previous"
            | "notif-knob-next"
            | "notif-knob-previous"
            | "notif-see"
            | "notif-restore"
    )
}

/// What a command does with the manager CLOSED.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        "notif-open" | "notif-filters" => Closed::Opens,
        // THE SECOND DOOR, in a window of its own — the same `View`, and the
        // same reason the file manager has two: the footer's region has a
        // ceiling in lines (invariant 14), which is right for a footer and
        // not for reading a fortnight of history.
        "notif-window" => {
            way.open_window(vec!["notifications".to_owned()]);
            Closed::Taken
        }
        // THROUGH THE DESK, as `r` in the manager is: each one closed on the
        // bus, and never a suppressed one.
        "notif-read-all" => {
            way.desk(Ask::ReadAll);
            Closed::Taken
        }
        // AND WITH A RECORD NAMED, the action of THAT record — what the window
        // sends for a deed only this process can do. An argument that names
        // nothing is refused, never taken for a row.
        crate::desk::ACT => {
            match argument.map(crate::desk::act_of) {
                Some(Some((key, which))) => way.desk(Ask::Act(key, which)),
                Some(None) => {
                    tracing::warn!(order = argument, "an action order that names no record")
                }
                None => {}
            }
            Closed::Taken
        }
        other if of_the_view(other) => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// The manager, opened on the column its command names — `n f` opens on the
/// filters — in the footer's clock: an open record shows when it arrived.
fn open(command: &str, _argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    let zone = way
        .lent()
        .get::<ronoctua_files::when::Zone>()
        .cloned()
        .unwrap_or_else(ronoctua_files::when::Zone::utc);
    let pane = if command == "notif-filters" {
        Pane::Catalogue
    } else {
        Pane::Happened
    };
    Box::new(Manager {
        view: View::fresh().in_zone(zone).reopened(pane),
        shown: false,
    })
}

impl Manager {
    /// THE LISTS CAN SHRINK UNDER THE CURSOR — `d` forgets the row it is on
    /// — so after anything that can change them the cursor is put back on a
    /// row that exists, and (after a person's gesture) the page follows it.
    fn settle(&mut self, follow: bool, way: &mut dyn Doorway) {
        {
            let lent = way.lent();
            let Some((lists, _)) = lists_of(&lent) else {
                return;
            };
            self.view.settle(&lists);
        }
        // THE FOOTER FINDS THE CURSOR as it is drawn (`Region::cursor_in`).
        if follow {
            way.page(Paging::Follow);
        }
    }

    /// What the manager asked for, carried out by the handler the window
    /// calls too, with the footer's desk — and, after a person's gesture,
    /// the page following the cursor.
    ///
    /// THE FOLLOW IS ASKED AFTER THE DESK'S ASK, and the footer does them in
    /// that order, telling the manager the lists moved between the two
    /// (`Settle`): so the page follows a cursor already back on a row that
    /// exists. `d` on the last record of a list scrolled to its end leaves
    /// the page where it was, as before the manager was a region; asked the
    /// other way round, the page followed a cursor past the end of the
    /// shrunken list — row 0 — and jumped.
    fn carry(&mut self, errand: crate::notifications::Errand, way: &mut dyn Doorway) {
        let typing = self.view.typing();
        if crate::notifications::carry_out(errand, typing, &mut ToTheFooter, way) {
            self.settle(true, way);
        }
        way.repaint();
    }

    /// One press — of a key, or of a command by name, which goes through the
    /// SAME `View::key` the keyboard does: a second path to "what is the
    /// focused row" is a second answer.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        let errand = {
            let lent = way.lent();
            let Some((lists, book)) = lists_of(&lent) else {
                return;
            };
            self.view.key(press, &lists, book, way.columns())
        };
        self.carry(errand, way);
    }

    /// Runs action `which` of the record — a command of ours closing the
    /// manager first. The order of a record NAMED — the band's button, the
    /// window's — is no gesture over the manager's rows: the page stays
    /// where it is.
    fn act(&mut self, key: RecordKey, which: usize, way: &mut dyn Doorway) {
        ToTheFooter.act(key, which, way);
        way.repaint();
    }

    /// The open record's bars, placed in the surface's pixels — the ones
    /// painted and the ones grabbed, from the same call
    /// (`notifications::bar_boxes`).
    fn bars(&self, area: &Area, lent: &Lent<'_>) -> Vec<crate::notifications::BarBox> {
        let Some((lists, _)) = lists_of(lent) else {
            return Vec::new();
        };
        let columns = tokens::metrics::columns(area.width, tokens::text::size());
        let bars = self.view.open_bars(&lists, columns);
        if bars.is_empty() {
            return Vec::new();
        }
        // FROM ROW 0: each bar says the row its half starts on.
        let f = area.frame();
        crate::notifications::bar_boxes(
            &bars,
            crate::chrome::FIELDS_LEAD,
            f.cell(),
            f.line(),
            f.y_of(area.first),
            f.bottom(),
        )
    }

    /// Puts one half of the open record at what the bar under `y` says.
    fn drag_to(&mut self, half: Half, y: f64, area: &Area, way: &mut dyn Doorway) -> bool {
        #[allow(clippy::cast_possible_truncation)]
        let py = y as f32;
        let lent = way.lent();
        let Some(to) = self
            .bars(area, &lent)
            .into_iter()
            .find(|b| b.half == half)
            .map(|b| b.offset_at(py))
        else {
            return false;
        };
        let Some((lists, _)) = lists_of(&lent) else {
            return false;
        };
        let columns = tokens::metrics::columns(area.width, tokens::text::size());
        self.view.open_scroll_to(half, to, &lists, columns);
        true
    }
}

impl Region for Manager {
    fn id(&self) -> &'static str {
        ID
    }

    /// Its own `fields` and not the key/text pair: it lays out two columns of
    /// its own — the file manager's listing's shape. A drawn row takes a
    /// click where the view says one does along it (`View::takes`, the
    /// window's answer too) — never a title, the blank between the columns,
    /// an empty column's sentence or a fact — and the band then lights only
    /// where the click at the pointer's cell takes ([`Self::lit`]).
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        let Some((lists, book)) = lists_of(&seen.lent) else {
            return Vec::new();
        };
        // THE ROOM AN OPEN RECORD WINDOWS ITS HALVES TO: the region's
        // ceiling, which is the most it can be given — never the height it
        // was last drawn at (`notifications::View::open_room`).
        self.view.set_room(usize::from(seen.lines));
        let rows = self.view.rows(&lists, book, seen.columns, seen.texts);
        let takes = self
            .view
            .takes(&lists, book, seen.columns, rows.len(), seen.texts);
        rows.into_iter()
            .zip(takes)
            .map(|(r, takes)| Row {
                fields: r.fields,
                takes,
            })
            .collect()
    }

    /// WHERE A CLICK AT THE CELL TAKES SOMETHING — the column the cell is in
    /// side by side, an action of an open record — the band lights, within
    /// the half [`Self::hover_span`] gives; nothing where the click there
    /// takes nothing (`View::takes_at`, which the click's own answer is).
    fn lit(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> crate::region::Lit {
        let Some((lists, book)) = lists_of(&seen.lent) else {
            return crate::region::Lit::Nothing;
        };
        if self
            .view
            .takes_at(row, cell, &lists, book, seen.columns, seen.texts)
        {
            crate::region::Lit::Row
        } else {
            crate::region::Lit::Nothing
        }
    }

    /// THE SAME TABLE the window draws on ITS first line, counter included —
    /// the window has no scrollbar and says where it is in text, and the two
    /// doors show the same thing.
    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        let Some((lists, _)) = lists_of(&seen.lent) else {
            return Vec::new();
        };
        self.view.hint_of(
            Some((self.view.cursor(), lists.len(self.view.pane()))),
            &lists,
            seen.texts,
        )
    }

    /// Asked with what is lent (`cursor_in`).
    fn cursor(&self) -> Option<usize> {
        None
    }

    /// THE MANAGER'S, plus the headings it draws above it — the drawn row,
    /// which the view knows (`View::visual_cursor`).
    fn cursor_in(&self, seen: &Seen<'_>) -> Option<usize> {
        let (lists, book) = lists_of(&seen.lent)?;
        Some(
            self.view
                .visual_cursor(&lists, book, seen.columns, seen.texts),
        )
    }

    /// One key: the translation from a symbol to a [`Press`] is the ONLY part
    /// of the manager that needs a seat — everything below it is arithmetic
    /// over a list.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        use ronoctua_keymap::Sym;
        let press = match key.sym {
            Sym::Up => Press::Up,
            Sym::Down => Press::Down,
            Sym::Left => Press::Left,
            Sym::Right => Press::Right,
            // `S-TAB` arrives as `Tab` with the shift the key reader kept.
            Sym::Tab if key.mods.shift => Press::BackTab,
            Sym::Tab => Press::Tab,
            Sym::Enter => Press::Enter,
            Sym::Space => Press::Space,
            Sym::Esc => Press::Escape,
            Sym::Char(c) => Press::Letter(c),
            _ => return Taken::No,
        };
        self.press(press, way);
        Taken::Yes
    }

    #[allow(clippy::too_many_lines)]
    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        match id {
            // OPEN, OR CLOSED AGAIN: the command that opened the manager is
            // handed to it once it is open, and the next one is the toggle.
            // With a record named — the band's `open` — it OPENS on that one,
            // whole, and never toggles: read first, as the band's envelope
            // reads it, since it is being read now.
            "notif-open" => {
                if let Some(key) = argument.and_then(crate::desk::record_named) {
                    self.shown = true;
                    way.desk(Ask::Read(key));
                    {
                        let lent = way.lent();
                        if let Some((lists, _)) = lists_of(&lent) {
                            self.view.open_on(key, &lists);
                        }
                    }
                    way.first_page();
                    way.repaint();
                } else if self.shown {
                    way.close();
                } else {
                    self.shown = true;
                    let unread = way.lent().get::<Queue>().map_or(0, Queue::unread);
                    tracing::info!(open = true, unread, "notification manager");
                }
            }
            // THE FILTERS HAVE A DOOR OF THEIR OWN: the same manager, opened
            // on the column that holds them.
            "notif-filters" => {
                if self.shown {
                    self.press(Press::Letter('f'), way);
                } else {
                    self.shown = true;
                }
            }
            // THE ANSWER FROM THE LINE, named: the VIEW keeps which question
            // it asked, and takes the answer (`View::typed`: an empty one
            // keeps what the draft had), as it takes the window's.
            "notif-filter-name" | "notif-filter-value" => {
                if let Some(text) = argument {
                    self.answered_with(id, text, way);
                }
            }
            "notif-window" => way.open_window(vec!["notifications".to_owned()]),
            "notif-read-all" => self.carry(crate::notifications::Errand::ReadAll, way),
            "notif-new" | "notif-drop" | "notif-forget" => {
                self.press(
                    Press::Letter(if id == "notif-new" { 'n' } else { 'd' }),
                    way,
                );
            }
            "notif-share" => self.press(Press::Letter('p'), way),
            "notif-draft-add" => self.press(Press::Letter('a'), way),
            "notif-draft-drop" => self.press(Press::Letter('x'), way),
            "notif-draft-save" => self.press(Press::Letter('s'), way),
            "notif-share-answer" | "notif-draft-type" | "notif-adjust" | "notif-see" => {
                self.press(Press::Enter, way);
            }
            "notif-read" => self.press(Press::Space, way),
            // AND WITH A RECORD NAMED, the action of THAT record.
            crate::desk::ACT => match argument {
                None => self.press(Press::Enter, way),
                Some(order) => match crate::desk::act_of(order) {
                    Some((key, which)) => self.act(key, which, way),
                    None => tracing::warn!(order, "an action order that names no record"),
                },
            },
            "notif-up" => self.press(Press::Up, way),
            "notif-down" => self.press(Press::Down, way),
            "notif-pane-next" => self.press(Press::Tab, way),
            "notif-pane-previous" => self.press(Press::BackTab, way),
            "notif-knob-next" => self.press(Press::Right, way),
            "notif-knob-previous" => self.press(Press::Left, way),
            // `r` IS TWO THINGS AT TWO LEVELS — all read on a list, the
            // distro's default in the sheet — so the command that names the
            // second one only presses when the sheet is open.
            "notif-restore" => {
                if self.view.in_sheet() {
                    self.press(Press::Letter('r'), way);
                }
            }
            _ => return Taken::No,
        }
        Taken::Yes
    }

    /// The row alone cannot say what a click took — the row has two targets
    /// on it — so it comes with the cell (`View::clicked`).
    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        self.clicked_at(row, None, way);
    }

    fn clicked_at(&mut self, row: usize, cell: Option<u16>, way: &mut dyn Doorway) {
        let errand = {
            let lent = way.lent();
            let Some((lists, book)) = lists_of(&lent) else {
                return;
            };
            self.view
                .clicked(row, cell, &lists, book, way.columns(), way.texts())
        };
        self.carry(errand, way);
    }

    /// Side by side one drawn row is TWO records — one per column — so the
    /// sentence depends on the cell as well (`View::about_at`).
    fn about_at(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Option<String> {
        let (lists, _) = lists_of(&seen.lent)?;
        self.view
            .about_at(&lists, seen.columns, row, cell, seen.texts)
    }

    /// What the focused row has to add — the reason a switch does nothing.
    fn note(&self, seen: &Seen<'_>) -> Option<String> {
        self.view.note(seen.texts)
    }

    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        self.answered_with(id, text, way);
    }

    /// The lists moved under the manager: a record read by another door or
    /// by the band, or what the manager itself asked of the desk (which the
    /// footer says with the page left where it is: the manager asks the
    /// follow itself, after the ask — `carry`).
    fn heard(&mut self, news: &dyn std::any::Any, way: &mut dyn Doorway) {
        if let Some(settle) = news.downcast_ref::<Settle>() {
            self.settle(settle.follow, way);
            way.repaint();
        }
    }

    /// The band inside ONE half of an open record, so it never crosses the
    /// rule into the other.
    fn hover_span(&self, row: usize, cell: u16, seen: &Seen<'_>) -> Option<(u16, u16)> {
        let (lists, _) = lists_of(&seen.lent)?;
        self.view.hover_span(&lists, seen.columns, row, cell)
    }

    /// An open record's half, under the pointer, and only it.
    fn wheel(&mut self, row: usize, cell: u16, lines: i64, way: &mut dyn Doorway) -> bool {
        let lent = way.lent();
        let Some((lists, _)) = lists_of(&lent) else {
            return false;
        };
        self.view
            .open_wheel(row, cell, lines, &lists, way.columns())
    }

    /// AN OPEN NOTIFICATION IS TWO TEXTS, and the sweep stays in its half.
    fn text_area(
        &self,
        row: usize,
        cell: u16,
        area: &Area,
        seen: &Seen<'_>,
    ) -> Option<crate::text_select::Area> {
        let (lists, _) = lists_of(&seen.lent)?;
        self.view.sweep_area(
            &lists,
            seen.columns,
            row,
            cell,
            crate::chrome::FIELDS_LEAD,
            area.first,
        )
    }

    /// The rule between the two halves — lit while the second has the
    /// keyboard, the file manager's focus mark — and a bar each: two lists
    /// side by side, each saying where its own is, from the same
    /// `Scroll::bar` every other bar asks.
    fn paint(&self, easel: &mut RegionEasel<'_>) {
        let columns = tokens::metrics::columns(easel.area.width, tokens::text::size());
        let f = easel.area.frame();
        if let Some((at, lit)) = self.view.divider(columns) {
            let x = f32::from(crate::chrome::FIELDS_LEAD + at) * f.cell() + f.cell() / 2.0;
            easel.under.push(if lit {
                crate::chrome::divider_lit(x, f.y_of(easel.area.first), f.bottom())
            } else {
                crate::chrome::divider(x, f.y_of(easel.area.first), f.bottom())
            });
        }
        for b in self.bars(&easel.area, &easel.lent) {
            easel.bars.record(crate::bars::Track::of(b.track));
            if let Some((bx, by, bw, bh)) = b.thumb {
                easel.under.push(ronoctua_render::quad::Quad {
                    x: bx,
                    y: by,
                    width: bw,
                    height: bh,
                    color: tokens::color::RULE_STRONG,
                });
            }
        }
    }

    /// A press on one of an open notification's bars, one per half, each in
    /// its own column: the half under it is the grip.
    fn grab(&mut self, at: (f64, f64), area: &Area, way: &mut dyn Doorway) -> Option<Grip> {
        #[allow(clippy::cast_possible_truncation)]
        let (px, py) = (at.0 as f32, at.1 as f32);
        let half = {
            let lent = way.lent();
            self.bars(area, &lent)
                .into_iter()
                .find(|b| b.holds(px, py))
                .map(|b| b.half)
        }?;
        self.drag_to(half, at.1, area, way);
        Some(grip_of(half))
    }

    /// The half that pointer holds, dragged to `at` — whichever half another
    /// pointer holds.
    fn drag(&mut self, grip: Grip, at: (f64, f64), area: &Area, way: &mut dyn Doorway) -> bool {
        half_of(grip).is_some_and(|half| self.drag_to(half, at.1, area, way))
    }
}

impl Manager {
    /// What was typed for the draft's question, back into it.
    fn answered_with(&mut self, id: &str, text: &str, way: &mut dyn Doorway) {
        if self.view.typing().is_some_and(|(asked, _)| asked == id) {
            self.view.typed(text);
            way.repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

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

    /// A queue holding one record, from `app`, with one action of each kind
    /// that matters here: a foreign app's button and a command of ours.
    fn a_queue() -> (Queue, RecordKey) {
        let mut n = ronoctua_notify::Notification::fresh_one(
            7,
            "app".into(),
            None,
            "summary".into(),
            "body".into(),
            ronoctua_notify::Urgency::Normal,
        );
        n.actions = vec![
            ronoctua_notify::Action {
                id: "reply".into(),
                label: ronoctua_notify::Label::Key("k".into()),
                doing: ronoctua_notify::Doing::Invoke,
            },
            ronoctua_notify::Action {
                id: "see".into(),
                label: ronoctua_notify::Label::Key("k".into()),
                doing: ronoctua_notify::Doing::Command("bluetooth-open".into()),
            },
        ];
        let book = Book::distro();
        let p = book.decide(&n);
        let mut q = Queue::fresh_one();
        q.enqueue(n, p, std::time::Instant::now());
        let key = q.history()[0].key();
        (q, key)
    }

    /// FAILS IF: the manager changes the records it is lent instead of asking
    /// the desk the footer keeps, or a command of ours run from a row leaves
    /// the manager holding the region the command wants.
    #[test]
    fn what_the_manager_changes_it_asks_of_the_desk() {
        let t = texts();
        let (queue, key) = a_queue();
        let book = Book::distro();
        let mut w = way(&t).lending(Lent::of(vec![&queue, &book]));
        let mut m = open("notif-open", None, &mut w);
        assert_eq!(m.command("notif-open", None, &mut w), Taken::Yes);
        w.owed.clear();
        let _ = m.key(ronoctua_keymap::Key::parse("SPC").expect("a key"), &mut w);
        assert!(
            w.owed.contains(&Effect::Desk(Ask::Read(key))),
            "{:?}",
            w.owed
        );
        w.owed.clear();
        let order = crate::desk::act_order(key, 0);
        let _ = m.command(crate::desk::ACT, Some(&order), &mut w);
        assert_eq!(
            w.owed.first(),
            Some(&Effect::Desk(Ask::Act(key, 0))),
            "the bus's button closed the manager"
        );
        w.owed.clear();
        let order = crate::desk::act_order(key, 1);
        let _ = m.command(crate::desk::ACT, Some(&order), &mut w);
        assert_eq!(
            &w.owed[..2],
            &[Effect::Close, Effect::Desk(Ask::Act(key, 1))],
            "a command of ours did not get the manager out of its way first"
        );
    }

    /// FAILS IF: `notif-open` stops being a toggle once the manager is open,
    /// or with a record named stops opening on it (read first), or the
    /// filters' door stops opening on the catalogue.
    #[test]
    fn open_toggles_and_a_record_named_opens_on_it() {
        let t = texts();
        let (queue, key) = a_queue();
        let book = Book::distro();
        let mut w = way(&t).lending(Lent::of(vec![&queue, &book]));
        assert_eq!(closed("notif-open", None, &mut w), Closed::Opens);
        let mut m = open("notif-open", None, &mut w);
        let _ = m.command("notif-open", None, &mut w);
        assert!(w.owed.is_empty(), "the opening did something: {:?}", w.owed);
        let _ = m.command("notif-open", None, &mut w);
        assert_eq!(w.owed, vec![Effect::Close]);
        w.owed.clear();
        let mut m = open("notif-open", None, &mut w);
        let named = crate::desk::record_order(key);
        let _ = m.command("notif-open", Some(&named), &mut w);
        assert_eq!(w.owed.first(), Some(&Effect::Desk(Ask::Read(key))));
        assert!(w.owed.contains(&Effect::FirstPage));
        assert!(
            !w.owed.contains(&Effect::Close),
            "a record named toggled the manager shut"
        );
        let filters = open("notif-filters", None, &mut w);
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 120,
            lines: 12,
            zone: &zone,
            photographing: true,
            lent: Lent::of(vec![&queue, &book]),
        };
        assert!(!filters.rows(&seen).is_empty());
        assert!(filters.cursor_in(&seen).is_some());
    }

    /// FAILS IF: a closed manager takes a command that is not its own, loses
    /// one of its own to the footer's "does nothing yet", or a window's
    /// action order with the manager closed is not asked of the desk.
    #[test]
    fn the_closed_manager_answers_its_own_commands_only() {
        let t = texts();
        let (_, key) = a_queue();
        let mut w = way(&t);
        assert_eq!(closed("notif-up", None, &mut w), Closed::Taken);
        assert_eq!(closed("notif-dismiss", None, &mut w), Closed::NotMine);
        assert_eq!(closed("stick-open", None, &mut w), Closed::NotMine);
        assert!(w.owed.is_empty());
        let order = crate::desk::act_order(key, 1);
        assert_eq!(
            closed(crate::desk::ACT, Some(&order), &mut w),
            Closed::Taken
        );
        assert_eq!(w.owed, vec![Effect::Desk(Ask::Act(key, 1))]);
        w.owed.clear();
        assert_eq!(
            closed(crate::desk::ACT, Some("nothing"), &mut w),
            Closed::Taken
        );
        assert!(w.owed.is_empty(), "an order naming no record was acted on");
        assert_eq!(closed("notif-read-all", None, &mut w), Closed::Taken);
        assert_eq!(w.owed, vec![Effect::Desk(Ask::ReadAll)]);
    }

    /// FAILS IF: the page the manager asks to follow its cursor after a
    /// gesture is asked BEFORE the ask of the desk — the footer does what is
    /// asked in order and settles the manager between the two, so a follow
    /// asked first follows a cursor past the end of the shrunken list (`d` on
    /// the last record of a scrolled list jumped the page) — or the action
    /// order of a record named, the band's button or the window's, which is
    /// no gesture over the manager's rows, moves the page.
    #[test]
    fn the_page_follows_after_the_desk_and_never_for_an_order() {
        let t = texts();
        let (queue, key) = a_queue();
        let book = Book::distro();
        let mut w = way(&t).lending(Lent::of(vec![&queue, &book]));
        let mut m = open("notif-open", None, &mut w);
        let _ = m.command("notif-open", None, &mut w);
        w.owed.clear();
        let _ = m.key(ronoctua_keymap::Key::parse("d").expect("a key"), &mut w);
        let asked = w
            .owed
            .iter()
            .position(|e| *e == Effect::Desk(Ask::Forget(key)));
        let follows = w
            .owed
            .iter()
            .position(|e| matches!(e, Effect::Page(Paging::Follow)));
        assert!(
            matches!((asked, follows), (Some(a), Some(f)) if a < f),
            "the follow is not asked after the desk: {:?}",
            w.owed
        );
        w.owed.clear();
        let order = crate::desk::act_order(key, 0);
        let _ = m.command(crate::desk::ACT, Some(&order), &mut w);
        assert!(w.owed.contains(&Effect::Desk(Ask::Act(key, 0))));
        assert!(
            !w.owed.iter().any(|e| matches!(e, Effect::Page(_))),
            "an action order moved the page: {:?}",
            w.owed
        );
    }

    /// FAILS IF: which half of an open record a drag moves is kept by the
    /// region and not handed back by the pointer that pressed — with two
    /// pointers (two seats) holding the two bars at once, one would move the
    /// other's half, and the first to let go would stop the other's drag.
    #[test]
    fn two_pointers_hold_the_two_halves_apart() {
        let t = texts();
        let mut n = ronoctua_notify::Notification::fresh_one(
            7,
            "app".into(),
            None,
            "summary".into(),
            "a line of words that goes on ".repeat(40),
            ronoctua_notify::Urgency::Normal,
        );
        n.actions = (0..6)
            .map(|i| ronoctua_notify::Action {
                id: format!("act-{i}"),
                label: ronoctua_notify::Label::Text(format!("action {i}")),
                doing: ronoctua_notify::Doing::Invoke,
            })
            .collect();
        let book = Book::distro();
        let p = book.decide(&n);
        let mut queue = Queue::fresh_one();
        queue.enqueue(n, p, std::time::Instant::now());
        let mut w = way(&t).lending(Lent::of(vec![&queue, &book]));
        let mut m = Manager {
            view: View::fresh().reopened(Pane::Happened),
            shown: true,
        };
        let _ = m.key(ronoctua_keymap::Key::parse("RET").expect("a key"), &mut w);
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 200,
            lines: 5,
            zone: &zone,
            photographing: true,
            lent: Lent::of(vec![&queue, &book]),
        };
        let _ = m.rows(&seen);
        let area = Area {
            width: tokens::metrics::cell_width(tokens::text::size()) * 200.5,
            bottom: 2000.0,
            first: 0,
            lines: 5,
            columns: 200,
        };
        let lent = Lent::of(vec![&queue, &book]);
        let bar = |m: &Manager, half: Half| {
            m.bars(&area, &lent)
                .into_iter()
                .find(|b| b.half == half)
                .unwrap_or_else(|| panic!("no bar for {half:?}"))
        };
        let top = |b: &crate::notifications::BarBox| {
            let (x, y, w, _) = b.track;
            (f64::from(x + w / 2.0), f64::from(y))
        };
        let end = |b: &crate::notifications::BarBox| {
            let (x, y, w, h) = b.track;
            (f64::from(x + w / 2.0), f64::from(y + h))
        };
        let (said, facts) = (bar(&m, Half::First), bar(&m, Half::Second));
        let a = m
            .grab(top(&said), &area, &mut w)
            .expect("the message's bar");
        let b = m.grab(top(&facts), &area, &mut w).expect("the facts' bar");
        assert_ne!(a, b, "the two bars are one grip");
        // POINTER A, holding the message, goes to its end: the facts stay.
        assert!(m.drag(a, end(&said), &area, &mut w));
        assert!(
            bar(&m, Half::First).scroll.offset > 0,
            "the message did not move"
        );
        assert_eq!(bar(&m, Half::Second).scroll.offset, 0, "A moved B's facts");
        // AND POINTER B's drag still answers the facts.
        assert!(m.drag(b, end(&facts), &area, &mut w));
        assert!(
            bar(&m, Half::Second).scroll.offset > 0,
            "the facts did not move"
        );
    }

    /// FAILS IF: the footer's word that the lists moved does not settle the
    /// manager, or its page does not follow after a person's gesture.
    #[test]
    fn a_settle_is_heard() {
        let t = texts();
        let (queue, _) = a_queue();
        let book = Book::distro();
        let mut w = way(&t).lending(Lent::of(vec![&queue, &book]));
        let mut m = open("notif-open", None, &mut w);
        let _ = m.command("notif-open", None, &mut w);
        m.heard(&Settle { follow: true }, &mut w);
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Page(Paging::Follow)))
        );
        w.owed.clear();
        m.heard(&Settle { follow: false }, &mut w);
        assert_eq!(w.owed, vec![Effect::Repaint]);
    }

    /// A queue of three records, from three apps, none filtered out.
    fn three_records() -> Queue {
        let book = Book::distro();
        let mut q = Queue::fresh_one();
        for (id, app) in [(1, "mail"), (2, "chat"), (3, "build")] {
            let mut n = ronoctua_notify::Notification::fresh_one(
                id,
                app.into(),
                None,
                format!("{app} said something"),
                "body".into(),
                ronoctua_notify::Urgency::Normal,
            );
            // TWO ACTIONS, each a line of the open record a click selects and
            // a second click runs.
            n.actions = vec![
                ronoctua_notify::Action {
                    id: "reply".into(),
                    label: ronoctua_notify::Label::Key("k".into()),
                    doing: ronoctua_notify::Doing::Invoke,
                },
                ronoctua_notify::Action {
                    id: "see".into(),
                    label: ronoctua_notify::Label::Key("k".into()),
                    doing: ronoctua_notify::Doing::Command("bluetooth-open".into()),
                },
            ];
            let p = book.decide(&n);
            q.enqueue(n, p, std::time::Instant::now());
        }
        q
    }

    /// The manager, open on `pane`, the keys of `steps` pressed on it — a
    /// level reached as the keyboard reaches it — and nothing of the way
    /// there left on the doorway.
    fn reached<'a>(
        t: &std::rc::Rc<ronoctua_i18n::Texts>,
        columns: u16,
        lent: (&'a Queue, &'a Book),
        pane: Pane,
        steps: &[&str],
    ) -> (Manager, crate::region::Lending<'a>) {
        let mut m = Manager {
            view: View::fresh()
                .in_zone(ronoctua_files::when::Zone::utc())
                .reopened(pane),
            shown: true,
        };
        let mut w = Recorder::new(
            std::rc::Rc::clone(t),
            columns,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
        .lending(Lent::of(vec![lent.0, lent.1]));
        for step in steps {
            let _ = m.key(ronoctua_keymap::Key::parse(step).expect("a key"), &mut w);
        }
        w.way.owed.clear();
        (m, w)
    }

    /// FAILS IF: on the list, stacked or side by side, a resting pointer
    /// lights a row — or a column of it — a click there takes nothing on: a
    /// column's title, the blank between them, an empty column's sentence;
    /// or leaves unlit an item a click takes. THE LIGHT IS THE CLICK: every
    /// row, in each half, clicked once on a manager of its own and lit
    /// exactly where something happened. The region said every row took a
    /// click, titles included.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let t = texts();
        let queue = three_records();
        let book = Book::distro();
        let zone = ronoctua_files::when::Zone::utc();
        for columns in [60, 160] {
            let seen = Seen {
                texts: &t,
                columns,
                lines: 30,
                zone: &zone,
                photographing: true,
                lent: Lent::of(vec![&queue, &book]),
            };
            let split = crate::notifications::split_at(columns);
            let (first, _) = reached(&t, columns, (&queue, &book), Pane::Happened, &[]);
            let rows = first.rows(&seen);
            let here = first.cursor_in(&seen);
            let (mut lit_any, mut dark_any) = (false, false);
            for (n, row) in rows.iter().enumerate() {
                for cell in [2, split + 2] {
                    let lit =
                        row.takes && first.lit(n, Some(cell), &seen) != crate::region::Lit::Nothing;
                    let (mut m, mut w) = reached(&t, columns, (&queue, &book), Pane::Happened, &[]);
                    let before = crate::region::seen(&m, &w.owed, &seen);
                    m.clicked_at(n, Some(cell), &mut w);
                    let did = crate::region::seen(&m, &w.owed, &seen) != before;
                    assert!(
                        lit == did || (lit && here == Some(n)),
                        "{columns} columns: row {n} at cell {cell} is lit {lit} and a click did {did}"
                    );
                    lit_any |= lit;
                    dark_any |= !lit;
                }
            }
            assert!(
                lit_any && dark_any,
                "{columns} columns: no row lit, or every row lit"
            );
        }
    }

    /// FAILS IF: on the sheet of a kind, or the draft of a filter, a
    /// resting pointer lights what a click takes nothing on — the sheet's
    /// name — or a knob or a line a click does not select. The two had no
    /// pointer at all: every row lit, and a click on any did nothing.
    #[test]
    fn the_sheet_and_the_draft_light_what_the_click_takes() {
        let t = texts();
        let queue = three_records();
        let book = Book::distro();
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &t,
            columns: 100,
            lines: 30,
            zone: &zone,
            photographing: true,
            lent: Lent::of(vec![&queue, &book]),
        };
        for (steps, level) in [(&["RET"][..], "the sheet"), (&["n"][..], "the draft")] {
            let held = crate::region::held_to_the_click(
                || reached(&t, 100, (&queue, &book), Pane::Catalogue, steps),
                &seen,
                |r, w: &crate::region::Lending<'_>| crate::region::seen(r, &w.owed, &seen),
            );
            assert!(
                held.iter().filter(|h| h.lit).count() >= 2,
                "{level}: fewer than two rows lit: {held:?}"
            );
            let unheld = crate::region::unheld(&held);
            assert!(unheld.is_empty(), "{level}: {unheld:?}");
        }
    }

    /// FAILS IF: on an open record, side by side or stacked, the pointer
    /// lights a line a click there takes nothing on — a line of the message
    /// in its own half, a fact while the keyboard is in the facts — or
    /// leaves unlit an action, or the half the keyboard is not in (a click
    /// gives it the keyboard).
    #[test]
    fn an_open_record_lights_what_the_click_takes() {
        let t = texts();
        let queue = three_records();
        let book = Book::distro();
        let zone = ronoctua_files::when::Zone::utc();
        for columns in [60, 200] {
            let seen = Seen {
                texts: &t,
                columns,
                lines: 30,
                zone: &zone,
                photographing: true,
                lent: Lent::of(vec![&queue, &book]),
            };
            let split = crate::notifications::split_at(columns);
            let open = || reached(&t, columns, (&queue, &book), Pane::Happened, &["RET"]);
            let (first, _) = open();
            assert!(first.view.is_open(), "{columns} columns: no record opened");
            let rows = first.rows(&seen);
            for (n, row) in rows.iter().enumerate() {
                for cell in [2, split + 2] {
                    let lit =
                        row.takes && first.lit(n, Some(cell), &seen) != crate::region::Lit::Nothing;
                    // AND WHICH HALF HAS THE KEYBOARD: a click there gives it
                    // — which the lit divider says side by side, and the
                    // arrows' next turn stacked.
                    let (mut m, mut w) = open();
                    let before = (
                        crate::region::seen(&m, &w.owed, &seen),
                        m.view.facts_have_the_keyboard(),
                    );
                    m.clicked_at(n, Some(cell), &mut w);
                    let did = (
                        crate::region::seen(&m, &w.owed, &seen),
                        m.view.facts_have_the_keyboard(),
                    ) != before;
                    assert_eq!(
                        lit, did,
                        "{columns} columns: row {n} at cell {cell} is lit {lit} and a click did {did}"
                    );
                }
            }
        }
    }
}
