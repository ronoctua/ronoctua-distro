//! THE VIDEO CATALOGUE IN THE FOOTER'S REGION (`LEADER v v`) — the view
//! (`crate::video_metrics`) as a `region::Region` of its own crate, and what
//! the footer keeps of it with the region closed (`region::Resident`).
//!
//! # One errand handler, two doors
//!
//! What the view asks of a door is carried out by the ONE function both doors
//! call ([`crate::video_metrics_feed::carry_out`]): the region here, the window
//! in `crate::video_metrics_window`. What is left to each door is what only
//! it can do (`video_metrics_feed::Leaving`): the transfers' queue, the
//! Chrome kiosk, the brain's page and the player's window are the footer's —
//! the window asks them down the footer's socket — and Nyxt is started by
//! its entry in the launcher; this region closes first, for the footer shows
//! one view at a time and a view left open keeps the keyboard.
//!
//! # What every row is, on the explanation line
//!
//! The footer has no foot: what the window's foot explains — the thing the
//! pointer rests on, a video of a shelf, a glyph, or the cursor's thing —
//! is said on its explanation line, joined into one sentence
//! (`door::Explained::joined`): the row's own on its line (`Told::about`),
//! a cell's and a glyph's at the cell under the pointer
//! ([`Region::about_at`]), and the cursor's while the pointer rests on
//! nothing of the region's, or after a key ([`Region::note`]) — the
//! pointer outranks the cursor until a key, as in the window.
//!
//! # What outlives the region
//!
//! The catalogue's readings, so the next opening draws at once while its feed
//! reads again — and a scan asked with the view closed (`video-metrics-scan`,
//! by name), whose feed is raised for it and goes once it said where the scan
//! went: the resident ([`Kept`]), made when the footer starts.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::region::{Closed, Doorway, Entry, Region, Residence, Residing, Row, Seen, Taken, Told};
use crate::video_metrics::{Errand, Page, Press, Readings, Said, View};
use crate::video_metrics_feed::{Arrived, Direct, Feed, Leaving, Want};

/// The region's id, as the footer ranks it.
pub const ID: &str = "video-metrics";

/// The region, as the binary registers it.
pub const ENTRY: Entry = Entry {
    id: ID,
    closed,
    open,
};

/// What the footer keeps for the session, as the binary registers it.
pub const RESIDING: Residing = Residing { id: ID, make };

/// WHAT THE FOOTER KEEPS OF THE VIDEO CATALOGUE, the region open or closed.
pub struct Kept {
    /// What the catalogue said, kept with the view closed.
    readings: Readings,
    /// The feed — one thread for the catalogue and the errands that leave the
    /// view, or the catalogue on the spot where no loop runs — while the view
    /// is open, or a scan asked with it closed waits for its answer.
    feed: Option<Reach>,
    /// Where the feed reads the catalogue: this machine's
    /// (`ronoctua_video_metrics::default_path`), found when the footer starts
    /// — `None` when there is no home, and every look is refused.
    catalogue: Option<std::path::PathBuf>,
}

/// Where the region's wants go.
enum Reach {
    /// Its thread, which wakes the footer's loop.
    Thread(Feed),
    /// ON THE SPOT, for a door that runs NO LOOP a thread could wake
    /// (`Doorway::has_a_loop`: a photograph of the footer, a test's footer):
    /// each want answered in the caller's turn, and what it brought held
    /// here for the region to take at once — so a frame is never taken
    /// before the reading it shows, as the window's photograph never is.
    /// Boxed: it holds the catalogue's connections and the decoded pictures.
    Here(Box<Direct>, Vec<Arrived>),
}

impl Kept {
    /// The feed and the readings, when they are not up yet — and what the
    /// door fills in the readings: the clock and how the queue is opened.
    fn awake(&mut self, way: &dyn Doorway) {
        if self.feed.is_none() {
            self.feed = Some(if way.has_a_loop() {
                let wake = way.wake();
                Reach::Thread(Feed::start(self.catalogue.clone(), move || wake.call()))
            } else {
                Reach::Here(Box::new(Direct::of(self.catalogue.clone())), Vec::new())
            });
        }
        self.readings.now_s = crate::video_metrics::now_s();
        self.readings.transfers_keys = crate::video_metrics::transfers_keys();
    }

    /// `want`, asked of the feed — or answered now, on the spot.
    fn want(&mut self, want: Want) {
        match self.feed.as_mut() {
            Some(Reach::Thread(f)) => f.want(want),
            Some(Reach::Here(d, held)) => held.extend(d.want(want)),
            None => {}
        }
    }

    /// Whether wants are answered on the spot: what they bring is taken at
    /// once, for no wake will come.
    const fn here(&self) -> bool {
        matches!(self.feed, Some(Reach::Here(..)))
    }

    /// What the feed brought, taken: readings merged, and the covers the
    /// region draws read again.
    fn take(&mut self) -> Vec<Arrived> {
        let arrived = match self.feed.as_mut() {
            Some(Reach::Thread(f)) => f.take(),
            Some(Reach::Here(_, held)) => std::mem::take(held),
            None => Vec::new(),
        };
        if arrived.is_empty() {
            return arrived;
        }
        self.readings.now_s = crate::video_metrics::now_s();
        arrived
    }

    /// THE COVERS THE REGION DRAWS, by the key their rows name.
    fn covers(&self) -> HashMap<String, crate::player_find::Picture> {
        self.readings
            .pictures
            .iter()
            .map(|(file, p)| {
                let key = crate::video_metrics::picture_key(file);
                (key.clone(), (key, std::sync::Arc::clone(&p.rgba)))
            })
            .collect()
    }
}

impl crate::region::Resident for Kept {
    /// A SCAN ASKED WITH THE VIEW CLOSED still says where it went — and the
    /// feed raised for it goes: no view reads.
    fn woken(&mut self, way: &mut dyn Doorway) {
        for a in self.take() {
            match a {
                Arrived::Reading(r) => self.readings.took(*r),
                Arrived::Done(crate::video_metrics::Done::Scan(s)) => {
                    let (said, _) = View::default().took(
                        crate::video_metrics::Done::Scan(s),
                        &self.readings,
                        way.texts(),
                    );
                    say(said, way);
                    self.feed = None;
                }
                Arrived::Done(_) => {}
            }
        }
    }
}

/// What the footer keeps, before anything was read: the machine's zone,
/// which the footer reads once too.
fn fresh() -> Kept {
    Kept {
        readings: crate::video_metrics_window::readings_for(String::new()),
        feed: None,
        catalogue: ronoctua_video_metrics::default_path(),
    }
}

/// The resident, made when the footer starts.
fn make() -> Residence {
    Residence::of(fresh())
}

/// What `said` says, on the door's line, for as long as its class does.
fn say(said: Option<Said>, way: &mut dyn Doorway) {
    match said {
        Some(Said::Normal(t)) => way.say(t, crate::echo::Echo::Normal),
        Some(Said::Long(t)) => way.say(t, crate::echo::Echo::Long),
        None => {}
    }
}

/// The resident behind a doorway — the footer's always has one.
fn kept_of(way: &dyn Doorway) -> Option<Rc<RefCell<Kept>>> {
    way.residence()?.as_a::<Kept>()
}

/// What a command does with the region closed: its opening and its
/// questions open it; its window and a scan are done without it; a key of
/// the view does nothing — there is no view to turn.
fn closed(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Closed {
    match command {
        crate::video_metrics::OPEN_COMMAND => Closed::Opens,
        crate::video_metrics::WINDOW_COMMAND => {
            open_the_window(argument, way);
            Closed::Taken
        }
        crate::video_metrics::SCAN_COMMAND => {
            if let Some(kept) = kept_of(way) {
                let mut kept = kept.borrow_mut();
                kept.awake(&*way);
                kept.want(Want::Scan);
            }
            Closed::Taken
        }
        id if crate::video_metrics::Ask::of_command(id).is_some()
            && (argument.is_some() || crate::video_metrics::press_of(id).is_none()) =>
        {
            Closed::Opens
        }
        id if crate::video_metrics::press_of(id).is_some() => Closed::Taken,
        _ => Closed::NotMine,
    }
}

/// The window, on the page `argument` names.
fn open_the_window(argument: Option<&str>, way: &mut dyn Doorway) {
    let mut words = vec!["video-metrics".to_owned()];
    words.extend(argument.map(ToOwned::to_owned));
    way.open_window(words);
}

/// The region, opened for `command` — the view on the page the opening
/// names (`video-metrics-open [page]`), its feed up; its first look is
/// carried out by the command that opened it, handed to it next, whichever
/// that is ([`VideoMetrics::command`]).
fn open(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Box<dyn Region> {
    Box::new(opening(command, argument, way))
}

/// [`open`], unboxed.
fn opening(command: &str, argument: Option<&str>, way: &mut dyn Doorway) -> VideoMetrics {
    let page = (command == crate::video_metrics::OPEN_COMMAND)
        .then(|| argument.and_then(crate::video_metrics_window::opening_of))
        .flatten();
    // A DOORWAY WITH NO RESIDENT — a test's — keeps its own.
    let kept = kept_of(way).unwrap_or_else(|| Rc::new(RefCell::new(fresh())));
    kept.borrow_mut().awake(&*way);
    let covers = kept.borrow().covers();
    let (mut view, first) = View::fresh(page.unwrap_or(Page::Home));
    // THE FILE PICKER'S FOLDER, watched through the region's own wake — and
    // not in a photograph, which reaches nothing of the machine.
    if !way.photographing() {
        view.watch_with(way.wake());
    }
    VideoMetrics {
        kept,
        view,
        first: Some(first),
        covers,
        pointed: None,
    }
}

/// THE VIDEO CATALOGUE'S REGION.
pub struct VideoMetrics {
    /// What the footer keeps of it, shared with the resident.
    kept: Rc<RefCell<Kept>>,
    /// The view.
    view: View,
    /// The opening's errand — the fresh view's look — carried out when the
    /// command that opened the region reaches it, whichever command that is
    /// ([`VideoMetrics::first_look`]).
    first: Option<Errand>,
    /// The thumbnails of the rows on screen, by the key their rows name —
    /// what the footer draws over them ([`Region::cover_picture`]).
    covers: HashMap<String, crate::player_find::Picture>,
    /// The row the pointer rests on, and the cell across it (`Under::cell`):
    /// what it is on is said, not the cursor's, until a key — the window's
    /// foot's rule. On what explains nothing (a title, the air of a shelf
    /// between two videos) the cursor's thing is said, as the foot says it.
    pointed: Option<(usize, Option<u16>)>,
}

impl VideoMetrics {
    /// The view's rows as the region's lines: fields, the caret, the lines
    /// that continue a row, and each thumbnail over its lines as a COVER —
    /// with its picture's key and size, once it is decoded — and what the
    /// pointer lights: a shelf of the grid the CELL under it, a line of
    /// glyphs the glyph under it (and a row's words beside its glyphs), a
    /// line of a picture or a title nothing, any other row its band. A line
    /// takes a click where the view says one does (`View::takes`, the
    /// window's answer too).
    ///
    /// WHAT A LINE SAYS under the pointer is the window's foot, joined
    /// (`View::explained_at`) — and nothing on a line of cells or glyphs:
    /// the footer asks a row's sentence before the cell's
    /// ([`Region::about_at`]), and one said for the line would hide the
    /// video's, or the glyph's, under the pointer.
    fn told_whole(&self, seen: &Seen<'_>) -> Vec<Told> {
        let kept = self.kept.borrow();
        let rows = self.view.rows(&kept.readings, seen.columns, seen.texts);
        let takes = self.view.takes(&rows);
        let (left, _) = crate::video_metrics::extent(seen.columns);
        rows.iter()
            .enumerate()
            .map(|(n, r)| {
                let covers = r
                    .thumbs
                    .iter()
                    .filter_map(|t| {
                        let p = kept.readings.pictures.get(&t.file)?;
                        Some(crate::region::Cover {
                            line: t.line,
                            lines: t.lines,
                            left: t.left,
                            cells: t.cells,
                            key: Rc::from(crate::video_metrics::picture_key(&t.file)),
                            size: p.size,
                        })
                    })
                    .collect();
                let owner = crate::video_metrics::Row::owner(&rows, n);
                let lit = rows
                    .get(owner)
                    .map_or(crate::region::LineLights::Nothing, |o| {
                        if !r.controls.is_empty() {
                            crate::region::LineLights::Glyphs {
                                cells: r.lit_cells(left, o.has_words()),
                                alone: r.controls.iter().map(|c| (c.from, c.width)).collect(),
                            }
                        } else if !o.cells.is_empty() {
                            crate::region::LineLights::UnderCell(
                                o.cells.iter().map(|c| (c.from, c.width)).collect(),
                            )
                        } else if o.heading || !o.thumbs.is_empty() {
                            crate::region::LineLights::Nothing
                        } else {
                            crate::region::LineLights::Row
                        }
                    });
                let several = rows
                    .get(owner)
                    .is_some_and(|o| !o.cells.is_empty() || !o.controls.is_empty())
                    || !r.controls.is_empty();
                let about = if several {
                    None
                } else {
                    self.view
                        .explained_at(&rows, n, None, seen.texts)
                        .and_then(|e| e.joined())
                };
                Told {
                    row: Row {
                        fields: r.fields.clone(),
                        takes: takes.get(n).copied().unwrap_or(false),
                    },
                    about,
                    caret: r.caret,
                    covers,
                    continues: r.continues,
                    lights: lit,
                    ..Told::default()
                }
            })
            .collect()
    }

    /// The view's rows, from the readings, at `columns`.
    fn view_rows(
        &self,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::video_metrics::Row> {
        self.view.rows(&self.kept.borrow().readings, columns, texts)
    }

    /// THE OPENING'S LOOK, carried out by the first command the region
    /// hears — the one that opened it: its opening, a question asked by
    /// name, an answer that came back with the region closed. The fresh
    /// view's feed reads what it shows, as the footer's view did whichever
    /// command opened it; and a later opening finds no look left over, and
    /// goes to the page it names. Whether there was one to carry out.
    fn first_look(&mut self, way: &mut dyn Doorway) -> bool {
        let Some(first) = self.first.take() else {
            return false;
        };
        way.first_page();
        self.errand(first, way);
        true
    }

    /// `video-metrics-open [page]`, the region open: a second opening keeps
    /// the live view, on the page asked when one is.
    fn opened(&mut self, argument: Option<&str>, way: &mut dyn Doorway) {
        self.kept.borrow_mut().awake(&*way);
        let errand = match argument.and_then(crate::video_metrics_window::opening_of) {
            Some(p) => self.view.open(p),
            None => Errand::Look(self.view.look()),
        };
        way.first_page();
        self.errand(errand, way);
    }

    /// Carries out what the view asked for: what every door does the same,
    /// and the footer's own ([`Leaving`]) — the queue, Chrome, Nyxt, the
    /// brain's page and the player's window, which take the region's place.
    fn errand(&mut self, errand: Errand, way: &mut dyn Doorway) {
        match crate::video_metrics_feed::carry_out(errand, way) {
            Ok(Some(want)) => {
                let here = {
                    let mut kept = self.kept.borrow_mut();
                    kept.want(want);
                    kept.here()
                };
                // ON THE SPOT, what it brought is taken now: no wake comes.
                if here {
                    self.woken(way);
                }
            }
            Ok(None) => {}
            // THE QUEUE (`LEADER t t`), where a download handed over is seen
            // going — the view closes first, so the region is free.
            Err(Leaving::Transfers) => {
                way.close();
                way.run("transfers-open", None);
            }
            // CLOSE BEFORE CHROME OPENS, the launcher's rule: a view left
            // open over the window that just opened keeps its keyboard.
            Err(Leaving::Kiosk(url)) => {
                way.close();
                way.run("kiosk-open", Some(url));
            }
            // AND BEFORE NYXT, by the same rule — started by its entry in the
            // launcher, and a machine without it said (`door-nyxt-absent`).
            Err(Leaving::Nyxt(url)) => {
                way.close();
                way.launch(crate::launch::in_nyxt(&url));
            }
            // AND BEFORE THE BRAIN'S WINDOW, on this catalogue's scan.
            Err(Leaving::Brain) => {
                way.close();
                way.run(
                    crate::video_metrics::BRAIN_OPEN,
                    Some(ronoctua_brain::compiled::video_metrics::ID.to_owned()),
                );
            }
            // AND BEFORE THE PLAYER'S WINDOW on one of its lists — a window
            // of its own, which the player's command opens.
            Err(Leaving::Player(word)) => {
                way.close();
                way.run(crate::video_metrics::PLAYER_WINDOW, Some(word.to_owned()));
            }
        }
    }

    /// A key of the view — from the keyboard, the hint, or `RET` from a
    /// second click (invariant 19) — and the cursor's thing is said again,
    /// until the pointer moves.
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
        self.pointed = None;
        let rows = self.view_rows(way.columns(), way.texts());
        let errand = self.view.press(press, &rows, &self.kept.borrow().readings);
        self.errand(errand, way);
        way.follow();
        way.repaint();
    }

    /// What was typed for one of the view's questions — or, with no argument
    /// (by name, a click), the question asked.
    fn typed(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) {
        let Some(ask) = crate::video_metrics::Ask::of_command(id) else {
            return;
        };
        // BY NAME, WITH NO VALUE: asked about the row the cursor is on when
        // it is one the question fits — as `RET` there would — and refused
        // with a sentence otherwise, never asked about nothing.
        let errand = match argument {
            None => {
                let rows = self.view_rows(way.columns(), way.texts());
                self.view
                    .asked_by_name(ask, &rows, &self.kept.borrow().readings, way.texts())
            }
            Some(said) => self
                .view
                .typed(id, said, &self.kept.borrow().readings, way.texts()),
        };
        self.errand(errand, way);
        way.repaint();
    }
}

impl Region for VideoMetrics {
    fn id(&self) -> &'static str {
        ID
    }

    /// NOT `TAB` FROM SECTION TO SECTION (`section_ring`): its question's `TAB` picks the answer.
    fn walks_sections(&self) -> bool {
        false
    }

    /// The region's rows — its fields, and whether each takes a click
    /// ([`VideoMetrics::told_whole`]).
    fn rows(&self, seen: &Seen<'_>) -> Vec<Row> {
        self.told_whole(seen).into_iter().map(|t| t.row).collect()
    }

    /// The region's rows from `skip`, at most `take`, as told to the footer:
    /// with what its one painter draws beside the fields.
    fn told(&self, seen: &Seen<'_>, skip: usize, take: usize) -> Vec<Told> {
        self.told_whole(seen)
            .into_iter()
            .skip(skip)
            .take(take)
            .collect()
    }

    fn hint(&self, seen: &Seen<'_>) -> Vec<crate::chrome::Piece> {
        let rows = self.view_rows(seen.columns, seen.texts);
        self.view.hint(&rows, seen.texts)
    }

    /// What the pointer on row `row` says at cell `cell` — a video of a
    /// shelf, a glyph — as the window's foot says it, joined into one
    /// sentence; any other row says its own on its line (`Told::about`).
    fn about_at(&self, row: usize, cell: Option<u16>, seen: &Seen<'_>) -> Option<String> {
        let rows = self.view_rows(seen.columns, seen.texts);
        let glyph = self.view.on_a_glyph(&rows, row, cell);
        self.view
            .explained_at(&rows, row, cell, seen.texts)
            .and_then(|e| {
                if glyph {
                    e.joined_named(seen.texts)
                } else {
                    e.joined()
                }
            })
    }

    /// WHAT THE CURSOR IS ON, said on the explanation line — while the
    /// pointer rests on nothing the window's foot would explain (a title,
    /// the air of a shelf, the air between glyphs), or after a key: the
    /// pointer outranks the cursor until the next key, as in the window's
    /// foot. Asked by the footer BEFORE a row's own sentence and the cell's
    /// (`about_at`), so it says nothing while the pointer is on a thing.
    fn note(&self, seen: &Seen<'_>) -> Option<String> {
        let rows = self.view_rows(seen.columns, seen.texts);
        if self
            .pointed
            .is_some_and(|(n, cell)| self.view.thing_at(&rows, n, cell).is_some())
        {
            return None;
        }
        self.view
            .explained_cursor(&rows, seen.texts)
            .and_then(|e| e.joined())
    }

    /// The pointer moved: the row it rests on and the cell across it — what
    /// the explanation line says then is the thing there, not the cursor's.
    fn hovered(
        &mut self,
        under: &crate::region::Under,
        _: &mut Option<Box<dyn std::any::Any>>,
        _: &mut dyn Doorway,
    ) -> bool {
        let now = under.row.map(|n| (n, under.cell));
        std::mem::replace(&mut self.pointed, now) != now
    }

    /// The pointer left: the cursor's thing is said again.
    fn left(&mut self, _: &mut Option<Box<dyn std::any::Any>>, _: &mut dyn Doorway) {
        self.pointed = None;
    }

    /// Measured by its rows ([`Region::cursor_in`]): the caret's line while a
    /// text is edited.
    fn cursor(&self) -> Option<usize> {
        None
    }

    /// The cursor's line among the rows drawn at `seen`'s width — the
    /// caret's line while a text is edited.
    fn cursor_in(&self, seen: &Seen<'_>) -> Option<usize> {
        let rows = self.view_rows(seen.columns, seen.texts);
        Some(self.view.drawn_cursor(&rows))
    }

    /// A key with the view in the region: the text's while a description is
    /// edited, else the view's by its whole notation.
    fn key(&mut self, key: ronoctua_keymap::Key, way: &mut dyn Doorway) -> Taken {
        if self.view.editing() {
            self.pointed = None;
            let (_, room) = crate::video_metrics::extent(way.columns());
            let errand = self.view.text_key(key, room);
            self.errand(errand, way);
            way.follow();
            way.repaint();
            return Taken::Yes;
        }
        // THE FILE PICKER, while it is up over the import page, takes what
        // it reads — every letter, and its own keys — before the view's
        // table (`Region::page_is` told it how many lines are shown).
        if self.view.picker_takes(key) {
            self.press(Press::Pick(key), way);
            return Taken::Yes;
        }
        match crate::video_metrics::press_of_key(key) {
            Some(press) => {
                self.press(press, way);
                Taken::Yes
            }
            None => Taken::No,
        }
    }

    fn command(&mut self, id: &str, argument: Option<&str>, way: &mut dyn Doorway) -> Taken {
        // THE OPENING'S LOOK FIRST, whichever command opened the region; the
        // opening itself has nothing more to do — its page was the fresh
        // view's.
        if self.first_look(way) && id == crate::video_metrics::OPEN_COMMAND {
            return Taken::Yes;
        }
        match id {
            crate::video_metrics::OPEN_COMMAND => self.opened(argument, way),
            crate::video_metrics::WINDOW_COMMAND => open_the_window(argument, way),
            crate::video_metrics::SCAN_COMMAND => {
                let mut kept = self.kept.borrow_mut();
                kept.awake(&*way);
                kept.want(Want::Scan);
            }
            // A VALUE TYPED on the minibuffer comes back as its question's
            // command, with the text; a question run by name, or `/` clicked
            // on the hint, asks.
            id if crate::video_metrics::Ask::of_command(id).is_some()
                && (argument.is_some() || crate::video_metrics::press_of(id).is_none()) =>
            {
                self.typed(id, argument, way);
            }
            id if crate::video_metrics::press_of(id).is_some() => {
                if let Some(press) = crate::video_metrics::press_of(id) {
                    self.press(press, way);
                }
            }
            _ => return Taken::No,
        }
        Taken::Yes
    }

    fn clicked(&mut self, row: usize, way: &mut dyn Doorway) {
        let under = crate::region::Under {
            at: (0.0, 0.0),
            line: None,
            row: Some(row),
            cell: None,
            base: 0,
            ctrl: false,
            shift: false,
            frame: None,
        };
        self.clicked_under(row, &under, way);
    }

    /// A click on row `n` — at the cell under the pointer, for a shelf of the
    /// grid and a line being edited: the first selects, the second acts
    /// (invariant 19).
    fn clicked_under(&mut self, n: usize, under: &crate::region::Under, way: &mut dyn Doorway) {
        let column = under.cell.unwrap_or(0);
        let rows = self.view_rows(way.columns(), way.texts());
        if self.view.editing() {
            if let Some(hit) = crate::video_metrics::text_hit(&rows, n, column) {
                self.view.text_clicked(hit);
            }
            way.repaint();
            return;
        }
        // A GLYPH ACTS ON THE FIRST CLICK, as the window's does.
        let glyph = self
            .view
            .click_control(n, column, &rows, &self.kept.borrow().readings);
        if let Some(errand) = glyph {
            self.errand(errand, way);
            way.follow();
            way.repaint();
            return;
        }
        if let Some(errand) = self.view.click_answers(n, &rows) {
            self.errand(errand, way);
            way.repaint();
            return;
        }
        if self.view.clicked(n, column, &rows) {
            self.press(Press::Enter, way);
        } else {
            way.repaint();
        }
    }

    fn answered(&mut self, id: &'static str, text: &str, way: &mut dyn Doorway) {
        self.typed(id, Some(text), way);
    }

    /// The question was let go: the row a value was asked for goes with it,
    /// or an answer typed later would land on it.
    fn unanswered(&mut self, _: &'static str, way: &mut dyn Doorway) {
        self.view.unanswered();
        way.repaint();
    }

    /// The clipboard's text, read off the loop for `C-v` in the description
    /// — or in the file picker, as what was typed there.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        if self.view.text_pasted(text) || self.view.picker_pasted(text) {
            way.repaint();
        }
    }

    /// How many lines the region shows, for the file picker's `PgUp` and
    /// `PgDn`.
    fn page_is(&self, lines: usize) {
        self.view.page_is(lines);
    }

    /// What the feed brought: readings merged, what an errand came to said —
    /// and a reading never moves the page: the cursor is kept on its row. A
    /// question the view put there from an errand's answer is followed, as
    /// the key it answers would have been.
    fn woken(&mut self, way: &mut dyn Doorway) {
        // THE FILE PICKER'S FOLDER MOVED, or its devices were read — the same
        // wake as the feed's.
        let picker = self.view.picker_woke();
        let arrived = self.kept.borrow_mut().take();
        if arrived.is_empty() {
            if picker {
                way.repaint();
            }
            return;
        }
        let mut next = Vec::new();
        for a in arrived {
            match a {
                Arrived::Reading(r) => self.kept.borrow_mut().readings.took(*r),
                Arrived::Done(done) => {
                    let (said, then) =
                        self.view
                            .took(done, &self.kept.borrow().readings, way.texts());
                    say(said, way);
                    next.push(then);
                }
            }
        }
        self.covers = self.kept.borrow().covers();
        let rows = self.view_rows(way.columns(), way.texts());
        self.view.settle(&rows);
        let follow = self.view.follow_owed();
        for e in next {
            self.errand(e, way);
        }
        if follow {
            way.follow();
        }
        way.repaint();
    }

    /// CAPS LOCK MEANS CAPITALS while the second description is edited.
    fn types_capitals(&self) -> bool {
        self.view.editing()
    }

    fn cover_picture(
        &self,
        key: &str,
    ) -> Option<&(String, std::sync::Arc<ronoctua_render::image::Rgba>)> {
        self.covers.get(key)
    }

    /// The view and its feed go; the readings stay, for the next opening.
    fn closing(&mut self, _: &mut dyn Doorway) {
        self.kept.borrow_mut().feed = None;
        self.covers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::{Effect, Recorder};

    /// The width the region is drawn at — the doorway's ([`way`]).
    const COLUMNS: u16 = 120;

    fn way() -> Recorder {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        Recorder::new(
            Rc::new(texts),
            COLUMNS,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(|| {}),
        )
    }

    /// The region over the fixture's readings, on `page` — on the grid when
    /// `grid` — with NO FEED: what a click asks of the catalogue, the brain
    /// or the player reaches nothing of this machine.
    fn over(page: &Page, grid: bool) -> VideoMetrics {
        let mut kept = fresh();
        kept.feed = None;
        kept.readings = crate::video_metrics::tests::readings();
        let (view, _) = View::fresh(page.clone());
        let mut region = VideoMetrics {
            kept: Rc::new(RefCell::new(kept)),
            view,
            first: None,
            covers: HashMap::new(),
            pointed: None,
        };
        if grid {
            let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
            let rows = region.view_rows(COLUMNS, &texts);
            let kept = Rc::clone(&region.kept);
            let _ = region
                .view
                .press(Press::Layout, &rows, &kept.borrow().readings);
        }
        region
    }

    /// FAILS IF: with the region closed, its opening or a question stops
    /// opening it, its window stops opening a window, a key of the view
    /// opens it, or a command of another view is taken for its own.
    #[test]
    fn a_closed_region_opens_for_its_opening_and_its_questions() {
        let mut w = way();
        assert_eq!(
            closed(crate::video_metrics::OPEN_COMMAND, None, &mut w),
            Closed::Opens
        );
        assert_eq!(
            closed(crate::video_metrics::WINDOW_COMMAND, Some("videos"), &mut w),
            Closed::Taken
        );
        assert_eq!(
            w.owed,
            vec![Effect::OpenWindow(vec![
                "video-metrics".to_owned(),
                "videos".to_owned()
            ])]
        );
        let asked = crate::video_metrics::Ask::AddChannel.command();
        assert_eq!(closed(asked, Some("@channel"), &mut w), Closed::Opens);
        assert_eq!(closed("video-metrics-down", None, &mut w), Closed::Taken);
        assert_eq!(closed("player-open", None, &mut w), Closed::NotMine);
    }

    /// FAILS IF: the region opened by a command other than its opening — the
    /// answer to one of its questions, which opens it closed — never asks its
    /// feed for the fresh view's look, so the view draws what an earlier
    /// opening read, or nothing; or a later opening by its page is answered
    /// with that look left over, and the page asked is not the one shown.
    #[test]
    fn a_region_opened_by_a_question_reads_and_a_later_opening_goes_to_its_page() {
        use std::time::{Duration, Instant};
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let (rang, heard) = std::sync::mpsc::channel::<()>();
        let rang = std::sync::Mutex::new(rang);
        let mut w = Recorder::new(
            Rc::new(texts),
            120,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(move || {
                if let Ok(r) = rang.lock() {
                    let _ = r.send(());
                }
            }),
        );
        // NO CATALOGUE AT ALL: every look is refused (`Why::NotThere`), which
        // is a reading all the same — and nothing of this machine's is read.
        let residence = Residence::of(Kept {
            catalogue: None,
            ..fresh()
        });
        w.residence = Some(residence.clone());
        let asked = crate::video_metrics::Ask::AddChannel.command();
        assert_eq!(closed(asked, Some("@channel"), &mut w), Closed::Opens);
        let mut region = opening(asked, Some("@channel"), &mut w);
        assert_eq!(region.command(asked, Some("@channel"), &mut w), Taken::Yes);
        let kept = residence.as_a::<Kept>().expect("the resident");
        // THE CHANNEL'S ANSWER is said once it arrives — after the look,
        // which was asked first and is read first.
        let said = |w: &Recorder| w.owed.iter().any(|e| matches!(e, Effect::Say(..)));
        let deadline = Instant::now() + Duration::from_secs(30);
        while !said(&w) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(heard.recv_timeout(left).is_ok(), "the feed never answered");
            region.woken(&mut w);
        }
        assert_eq!(
            kept.borrow().readings.catalogue,
            Some(Err(ronoctua_video_metrics::Why::NotThere)),
            "the view opened by a question read nothing"
        );
        // A LATER OPENING, by its page, opens the page.
        assert_eq!(*region.view.page(), Page::Home);
        let opens = crate::video_metrics::OPEN_COMMAND;
        assert_eq!(region.command(opens, Some("videos"), &mut w), Taken::Yes);
        assert_eq!(
            *region.view.page(),
            Page::Videos,
            "the page asked was not opened"
        );
    }

    /// FAILS IF: the two doors carry one errand out differently — the
    /// footer's region and the window say, ask, paste and close through the
    /// one handler (`video_metrics_feed::carry_out`), and what leaves the
    /// view (the transfers' queue, Chrome, Nyxt, the brain's page, the
    /// player's window) is handed back to each door as a `Leaving`.
    #[test]
    fn both_doors_carry_an_errand_out_alike() {
        use crate::video_metrics::Errand;
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut ui = crate::door::Ui::for_tests(texts);
        let mut footer = way();
        let errands = || {
            vec![
                Errand::Say("said".to_owned()),
                Errand::Copy("copied".to_owned()),
                Errand::Paste,
                Errand::Scan,
                Errand::Close,
            ]
        };
        for (a, b) in errands().into_iter().zip(errands()) {
            let window = crate::video_metrics_feed::carry_out(a, &mut ui);
            let region = crate::video_metrics_feed::carry_out(b, &mut footer);
            assert_eq!(window.is_ok(), region.is_ok());
            assert_eq!(
                window.as_ref().ok().map(Option::is_some),
                region.as_ref().ok().map(Option::is_some)
            );
        }
        assert!(ui.quit, "the window did not close");
        assert!(footer.owed.contains(&Effect::Close));
        assert!(footer.owed.contains(&Effect::Paste));
        assert!(
            footer
                .owed
                .iter()
                .any(|e| matches!(e, Effect::Say(t, _) if t == "said")),
            "{:?}",
            footer.owed
        );
        for errand in [
            Errand::Transfers,
            Errand::Kiosk("https://x.example".to_owned()),
            Errand::Nyxt("https://x.example".to_owned()),
            Errand::Brain,
            Errand::Player(crate::player_seam::ranking::OPEN_RANKING),
        ] {
            assert!(crate::video_metrics_feed::carry_out(errand, &mut footer).is_err());
        }
    }

    /// FAILS IF: what leaves the view for another program — the brain's
    /// page on the scan, the player's window on one of its lists, a
    /// channel's videos in Nyxt — is not done by the footer's region, or is
    /// done before the view closes: a view left open over the window that
    /// just opened keeps its keyboard. Each from where the person asks it:
    /// `RET` on the home page's rows, a click on a channel's glyph.
    #[test]
    fn errand_closes_before_going_elsewhere() {
        use crate::player_seam::ranking::{OPEN_RANKING, OPEN_SET_ASIDE};
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let gone = |w: &Recorder, effect: Effect| {
            assert!(
                w.owed.starts_with(&[Effect::Close, effect.clone()]),
                "not closed, then {effect:?}: {:?}",
                w.owed
            );
        };
        for (spot, effect) in [
            (
                "in-the-brain",
                Effect::Run(
                    crate::video_metrics::BRAIN_OPEN,
                    Some(ronoctua_brain::compiled::video_metrics::ID.to_owned()),
                ),
            ),
            (
                "in-the-player",
                Effect::Run(
                    crate::video_metrics::PLAYER_WINDOW,
                    Some(OPEN_RANKING.to_owned()),
                ),
            ),
            (
                "set-aside-in-the-player",
                Effect::Run(
                    crate::video_metrics::PLAYER_WINDOW,
                    Some(OPEN_SET_ASIDE.to_owned()),
                ),
            ),
        ] {
            let mut region = over(&Page::Home, false);
            let mut w = way();
            let rows = region.view_rows(COLUMNS, &texts);
            let n = rows.iter().position(|r| r.spot == spot).expect(spot);
            region.clicked(n, &mut w);
            w.owed.clear();
            region.clicked(n, &mut w);
            gone(&w, effect);
        }
        let mut region = over(&Page::Channels, false);
        let mut w = way();
        let rows = region.view_rows(COLUMNS, &texts);
        let (n, nyxt) = rows
            .iter()
            .enumerate()
            .find_map(|(n, r)| {
                r.controls
                    .iter()
                    .find(|c| matches!(c.target, crate::video_metrics::Target::ChannelNyxt(_)))
                    .map(|c| (n, c.from))
            })
            .expect("a channel's Nyxt");
        let under = crate::region::Under {
            at: (0.0, 0.0),
            line: None,
            row: Some(n),
            cell: Some(nyxt),
            base: 0,
            ctrl: false,
            shift: false,
            frame: None,
        };
        region.clicked_under(n, &under, &mut w);
        let channel = crate::video_metrics::tests::readings().channels[0]
            .id
            .clone();
        gone(
            &w,
            Effect::Launch(crate::launch::in_nyxt(&format!(
                "https://www.youtube.com/channel/{}/videos",
                channel.0
            ))),
        );
    }

    /// FAILS IF: a resting pointer lights a row of the footer's region a
    /// click takes nothing on — a title, a note, the air between sections,
    /// a line of a picture — or leaves unlit one it takes, a line that
    /// continues a row included; or a shelf of the grid takes nothing where
    /// a click on one of its cells lands. THE LIGHT IS THE CLICK: every row
    /// clicked once, on a view of its own, and lit exactly where something
    /// happened — on every page the fixture fills, the list and the grid.
    #[test]
    fn the_pointer_lights_what_the_click_takes() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let mut glyphs = 0;
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: COLUMNS,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let video = Page::Video(ronoctua_video_metrics::VideoId("video000000".to_owned()));
        let witness = |r: &VideoMetrics, w: &Recorder| crate::region::seen(r, &w.owed, &seen);
        let mut shelves = 0;
        for (page, grid) in [
            (Page::Home, false),
            (Page::Channels, false),
            (Page::Import, false),
            (Page::Rules, false),
            (Page::Weights, false),
            (Page::Videos, false),
            (Page::Videos, true),
            (video, false),
        ] {
            let fresh = || (over(&page, grid), way());
            let first = over(&page, grid);
            let rows = first.view_rows(COLUMNS, &texts);
            let cursor = first.cursor_in(&seen);
            let owner = |n: usize| crate::video_metrics::Row::owner(&rows, n);
            let held: Vec<crate::region::Held> =
                crate::region::held_to_the_click(fresh, &seen, witness)
                    .into_iter()
                    .map(|mut h| {
                        // A ROW OF SEVERAL LINES IS THE CURSOR'S on every
                        // line: a click on any of them is a click on it.
                        h.here = h.here || cursor == Some(owner(h.row));
                        // A SHELF IS CLICKED WHERE A CELL IS: the footer
                        // hands the cell under the pointer, and the
                        // generic click has none — the air before the
                        // first cell, which takes nothing and lights
                        // nothing (`LineLights::UnderCell`).
                        let click_at = |cell: u16| {
                            let (mut r, mut w) = fresh();
                            let before = witness(&r, &w);
                            let under = crate::region::Under {
                                at: (0.0, 0.0),
                                line: None,
                                row: Some(h.row),
                                cell: Some(cell),
                                base: 0,
                                ctrl: false,
                                shift: false,
                                frame: None,
                            };
                            r.clicked_under(h.row, &under, &mut w);
                            witness(&r, &w) != before
                        };
                        // A LINE OF GLYPHS IS CLICKED WHERE A GLYPH IS, as a
                        // shelf is where a cell is: each acts on the first
                        // click (`View::click_control`).
                        let controls = rows.get(h.row).map(|r| r.controls.clone());
                        if let Some(controls) = controls.filter(|c| !c.is_empty()) {
                            glyphs += 1;
                            h.did = controls.iter().all(|c| click_at(c.from));
                            return h;
                        }
                        let cells = rows.get(owner(h.row)).map(|o| o.cells.clone());
                        if let Some(cells) = cells.filter(|c| !c.is_empty()) {
                            shelves += 1;
                            h.did = cells.iter().all(|c| {
                                let (mut r, mut w) = fresh();
                                let before = witness(&r, &w);
                                let under = crate::region::Under {
                                    at: (0.0, 0.0),
                                    line: None,
                                    row: Some(h.row),
                                    cell: Some(c.from),
                                    base: 0,
                                    ctrl: false,
                                    shift: false,
                                    frame: None,
                                };
                                r.clicked_under(h.row, &under, &mut w);
                                r.clicked_under(h.row, &under, &mut w);
                                witness(&r, &w) != before
                            });
                        }
                        h
                    })
                    .collect();
            let what = format!("{page:?}, grid {grid}");
            assert!(
                held.iter().any(|h| h.lit) && held.iter().any(|h| !h.lit),
                "{what}: no row lit, or every row lit: {held:?}"
            );
            let unheld = crate::region::unheld(&held);
            assert!(unheld.is_empty(), "{what}: {unheld:?}");
        }
        assert!(shelves > 0, "the grid drew no shelf");
        assert!(glyphs > 0, "no line of glyphs was clicked");
    }

    /// FAILS IF: the footer's explanation line says on a row anything but
    /// what the window's foot says of it, joined (`Explained::joined`); a
    /// line of a shelf says one thing for its several videos instead of the
    /// one under the pointer; the cursor's thing is said while the pointer
    /// rests on a thing the foot explains (it would hide that thing's), or is
    /// not said where the foot says it — the pointer off the rows, on a
    /// title or the air of a shelf, or after a key.
    #[test]
    fn the_line_says_the_pointer_then_the_cursor() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: COLUMNS,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let under = |row: Option<usize>, cell: Option<u16>| crate::region::Under {
            at: (0.0, 0.0),
            line: None,
            row,
            cell,
            base: 0,
            ctrl: false,
            shift: false,
            frame: None,
        };
        let mut w = way();
        let mut region = over(&Page::Home, false);
        let rows = region.view_rows(COLUMNS, &texts);
        let told = region.told_whole(&seen);
        let channels = rows
            .iter()
            .position(|r| r.spot == "channels")
            .expect("the channels' row");
        let foot = region
            .view
            .explained_at(&rows, channels, None, &texts)
            .expect("the row explained");
        assert_eq!(
            told[channels].about,
            foot.joined(),
            "the line and the foot differ"
        );
        assert!(
            told[channels]
                .about
                .as_deref()
                .is_some_and(|a| a.contains(&texts.t(crate::key::ABOUT_VIDEO_METRICS_CHANNELS))),
            "{:?}",
            told[channels].about
        );
        // NOTHING POINTED: the cursor's thing. On a row: that row's, alone.
        let cursor = region.note(&seen).expect("the cursor's thing");
        let mut hand = None;
        assert!(region.hovered(&under(Some(channels), None), &mut hand, &mut w));
        assert_eq!(
            region.note(&seen),
            None,
            "the cursor hides the pointer's row"
        );
        // A TITLE under the pointer: the cursor's again.
        let title = rows.iter().position(|r| r.heading).expect("a title");
        region.hovered(&under(Some(title), None), &mut hand, &mut w);
        assert_eq!(region.note(&seen).as_deref(), Some(cursor.as_str()));
        // A KEY, the pointer still: the cursor's, which moved.
        region.hovered(&under(Some(channels), None), &mut hand, &mut w);
        region.press(Press::Down, &mut w);
        let moved = region.note(&seen).expect("the cursor's thing after a key");
        assert_ne!(moved, cursor, "the key's cursor is not what is said");
        // AND A SHELF: nothing for the line, each video at its cell.
        let mut region = over(&Page::Videos, true);
        let rows = region.view_rows(COLUMNS, &texts);
        let told = region.told_whole(&seen);
        let shelf = rows
            .iter()
            .position(|r| !r.cells.is_empty())
            .expect("a shelf");
        assert_eq!(
            told[shelf].about, None,
            "a shelf said one thing for its videos"
        );
        let said: Vec<Option<String>> = rows[shelf]
            .cells
            .iter()
            .map(|c| region.about_at(shelf, Some(c.from + 1), &seen))
            .collect();
        assert!(said.iter().all(Option::is_some), "{said:?}");
        let gap = rows[shelf].cells[0].from + rows[shelf].cells[0].width;
        assert_eq!(
            region.about_at(shelf, Some(gap), &seen),
            None,
            "the air said a video"
        );
        // THE AIR OF A SHELF UNDER THE POINTER explains nothing: the line
        // says the cursor's thing, as the window's foot does there — and a
        // video under it hides the cursor's again.
        let cursor = region
            .view
            .explained_cursor(&rows, &texts)
            .and_then(|e| e.joined());
        assert!(cursor.is_some(), "the grid's cursor explains nothing");
        region.hovered(&under(Some(shelf), Some(gap)), &mut hand, &mut w);
        assert_eq!(
            region.note(&seen),
            cursor,
            "the air of a shelf said nothing"
        );
        let video = rows[shelf].cells[0].from + 1;
        assert!(region.hovered(&under(Some(shelf), Some(video)), &mut hand, &mut w));
        assert_eq!(
            region.note(&seen),
            None,
            "the cursor hides the pointer's video"
        );
    }

    /// FAILS IF: a line of glyphs says one sentence for the whole line — the
    /// footer asks a row's before the thing's under the pointer
    /// (`Region::about_at`), and it would hide every glyph's —; a glyph under
    /// the pointer is not said by its own sentence, led by its name (a glyph
    /// is a picture: which rating?), with what holds once it is off where it
    /// says that (`Control::follows`); the air between two
    /// glyphs says something; the line lights other boxes than
    /// its words' and its glyphs' (`Row::lit_cells`), or lights a glyph over
    /// the row's other lines (`LineLights::Glyphs`); or the cursor's thing
    /// is said while the pointer rests on a glyph. On the list and the grid.
    #[test]
    fn a_glyph_says_its_own_sentence() {
        use ronoctua_video_metrics::read::Forever;
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: COLUMNS,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let (left, _) = crate::video_metrics::extent(COLUMNS);
        let mut followed = 0;
        for grid in [false, true] {
            let mut region = over(&Page::Videos, grid);
            // A VIDEO SET ASIDE AND ONE KEPT BY ITS OWN MARK: their bookmark
            // and their ∞ say what holds once they are off.
            if let Some(l) = region.kept.borrow_mut().readings.listed.as_mut() {
                l.rows[0].set_aside_at_s = Some(l.rows[0].published_at_s.unwrap_or(0));
                l.rows[1].forever = Forever::ByVideo;
            }
            let rows = region.view_rows(COLUMNS, &texts);
            let told = region.told_whole(&seen);
            let lines: Vec<usize> = (0..rows.len())
                .filter(|&n| !rows[n].controls.is_empty())
                .collect();
            assert!(!lines.is_empty(), "grid {grid}: no line of glyphs");
            for &n in &lines {
                let row = &rows[n];
                let owner = &rows[crate::video_metrics::Row::owner(&rows, n)];
                assert_eq!(
                    told[n].about, None,
                    "grid {grid}: line {n} says one thing for its glyphs"
                );
                assert_eq!(
                    told[n].lights,
                    crate::region::LineLights::Glyphs {
                        cells: row.lit_cells(left, owner.has_words()),
                        alone: row.controls.iter().map(|c| (c.from, c.width)).collect(),
                    },
                    "grid {grid}: line {n} lights other boxes"
                );
                for (i, c) in row.controls.iter().enumerate() {
                    let said = region.about_at(n, Some(c.from), &seen).unwrap_or_else(|| {
                        panic!("grid {grid}: glyph {i} of line {n} says nothing")
                    });
                    assert!(
                        said.contains(&texts.t(c.about)),
                        "grid {grid}: glyph {i} of line {n} says `{said}`"
                    );
                    assert!(
                        said.starts_with(&c.name),
                        "grid {grid}: glyph {i} of line {n} does not say which: `{said}`"
                    );
                    if let Some(f) = &c.follows {
                        followed += 1;
                        assert!(
                            said.contains(f.as_str()),
                            "grid {grid}: `{said}` lacks `{f}`"
                        );
                    }
                    if row.controls.get(i + 1).is_some() {
                        assert_eq!(
                            region.about_at(n, Some(c.from + c.width), &seen),
                            None,
                            "grid {grid}: the air after glyph {i} of line {n} says something"
                        );
                    }
                }
            }
            // A GLYPH UNDER THE POINTER hides the cursor's thing.
            let n = lines[0];
            let c = rows[n].controls[0].clone();
            let mut hand = None;
            let mut w = way();
            let under = crate::region::Under {
                at: (0.0, 0.0),
                line: None,
                row: Some(n),
                cell: Some(c.from),
                base: 0,
                ctrl: false,
                shift: false,
                frame: None,
            };
            region.hovered(&under, &mut hand, &mut w);
            assert_eq!(
                region.note(&seen),
                None,
                "grid {grid}: the cursor hides the pointer's glyph"
            );
        }
        assert!(
            followed >= 4,
            "the bookmark and the ∞ of the marked videos said nothing more: {followed}"
        );
    }

    /// FAILS IF: the region's rows stop naming the pictures the footer draws
    /// over them, or a line of a picture or a title lights a band.
    #[test]
    fn the_rows_name_their_pictures_and_what_they_light() {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: 120,
            lines: 30,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let mut kept = fresh();
        kept.readings = crate::video_metrics::tests::readings();
        let (view, _) = View::fresh(Page::Home);
        let region = VideoMetrics {
            kept: Rc::new(RefCell::new(kept)),
            view,
            first: None,
            covers: HashMap::new(),
            pointed: None,
        };
        let rows = region.told_whole(&seen);
        assert!(!rows.is_empty());
        let drawn = region.view_rows(120, &texts);
        for (n, (row, r)) in rows.iter().zip(&drawn).enumerate() {
            if r.heading && r.cells.is_empty() {
                assert_eq!(row.lights, crate::region::LineLights::Nothing, "row {n}");
            }
            assert_eq!(row.row.takes, r.takes(), "row {n}");
            assert_eq!(row.continues, r.continues, "row {n}");
        }
        assert_eq!(
            region.cursor_in(&seen),
            Some(region.view.drawn_cursor(&drawn))
        );
    }

    /// A scratch folder of the test `tag`, with a list of TWO MADE-UP
    /// CHANNELS in it (invariant 13): the folder, and the list.
    fn scratch_list(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-region-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        let list = dir.join("subscriptions.csv");
        let line = |c: char, title: &str| {
            let id = ronoctua_video_metrics::ChannelId(format!("UC{}", c.to_string().repeat(22)));
            format!(
                "{},{},{title}\n",
                id.0,
                ronoctua_video_metrics::takeout::channel_url(&id)
            )
        };
        std::fs::write(
            &list,
            format!(
                "{}\n{}{}",
                ronoctua_video_metrics::takeout::HEADER,
                line('r', "One"),
                line('s', "Two")
            ),
        )
        .expect("a list");
        (dir, list)
    }

    /// FAILS IF: a footer that runs no loop — a photograph's, whose frame is
    /// taken right after its steps — draws the region before the catalogue
    /// was read: its opening's look, and a list imported, are answered in
    /// the region's own turn, the import said and its channels drawn to
    /// decide with no wake to wait for.
    #[test]
    fn a_footer_with_no_loop_reads_on_the_spot() {
        use ronoctua_keymap::{Key, Sym};
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let (dir, list) = scratch_list("here");
        let mut w = way();
        w.has_a_loop = false;
        // A SCRATCH CATALOGUE, never this machine's.
        let residence = Residence::of(Kept {
            catalogue: Some(dir.join("video-metrics").join("catalog.sqlite")),
            ..fresh()
        });
        w.residence = Some(residence.clone());
        let kept = residence.as_a::<Kept>().expect("the resident");
        let opens = crate::video_metrics::OPEN_COMMAND;
        let mut region = opening(opens, Some("import"), &mut w);
        assert_eq!(region.command(opens, Some("import"), &mut w), Taken::Yes);
        assert!(
            kept.borrow().readings.catalogue.is_some(),
            "the opening's look was not answered on the spot"
        );
        let rows = region.view_rows(COLUMNS, &texts);
        let n = rows
            .iter()
            .position(|r| r.spot == "choose-list")
            .expect("the row that chooses the list");
        region.clicked(n, &mut w);
        region.clicked(n, &mut w);
        region.pasted(&list.to_string_lossy(), &mut w);
        assert_eq!(region.key(Key::simple(Sym::Enter), &mut w), Taken::Yes);
        let imported = texts.t_args(
            crate::key::VIDEO_METRICS_IMPORTED,
            &[
                ("staged", ronoctua_i18n::Arg::Number(2)),
                ("already", ronoctua_i18n::Arg::Number(0)),
            ],
        );
        assert!(
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Say(t, _) if *t == imported)),
            "the import was not said in the region's turn: {:?}",
            w.owed
        );
        let staged = region
            .view_rows(COLUMNS, &texts)
            .iter()
            .filter(|r| r.spot.starts_with("staged:"))
            .count();
        assert_eq!(staged, 2, "the channels to decide are not drawn");
        drop(region);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the footer's region does not host the file picker over its
    /// import page as the window does — the second click on choosing the
    /// list brings it up, every key it claims is its, its rows are said on
    /// the explanation line; going back puts the page back; a list pasted
    /// into it and taken is staged by the region's feed and said — or
    /// Google Takeout does not open in Chrome after the region closes.
    #[test]
    fn the_region_picks_a_list_and_imports_it() {
        use ronoctua_keymap::{Key, Sym};
        use std::time::{Duration, Instant};
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let (dir, list) = scratch_list("import");
        let (rang, heard) = std::sync::mpsc::channel::<()>();
        let rang = std::sync::Mutex::new(rang);
        let mut w = Recorder::new(
            Rc::new(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB")),
            COLUMNS,
            true,
            crate::sudo::Gate::window(),
            crate::wake::Wake::new(move || {
                if let Ok(r) = rang.lock() {
                    let _ = r.send(());
                }
            }),
        );
        // A SCRATCH CATALOGUE, never this machine's.
        let residence = Residence::of(Kept {
            catalogue: Some(dir.join("video-metrics").join("catalog.sqlite")),
            ..fresh()
        });
        w.residence = Some(residence.clone());
        let opens = crate::video_metrics::OPEN_COMMAND;
        let mut region = opening(opens, Some("import"), &mut w);
        assert_eq!(region.command(opens, Some("import"), &mut w), Taken::Yes);
        assert_eq!(*region.view.page(), Page::Import);
        let choose = |region: &mut VideoMetrics, w: &mut Recorder| {
            let rows = region.view_rows(COLUMNS, &texts);
            let n = rows
                .iter()
                .position(|r| r.spot == "choose-list")
                .expect("the row that chooses the list");
            region.clicked(n, w);
            region.clicked(n, w);
        };
        choose(&mut region, &mut w);
        assert!(
            region.view.picking(),
            "the second click brought no picker up"
        );
        assert_eq!(region.key(Key::simple(Sym::Char('n')), &mut w), Taken::Yes);
        let zone = ronoctua_files::when::Zone::utc();
        let seen = Seen {
            texts: &texts,
            columns: COLUMNS,
            lines: 40,
            zone: &zone,
            photographing: true,
            lent: crate::region::Lent::none(),
        };
        let pick = texts.t(crate::key::ABOUT_VIDEO_METRICS_PICK);
        assert!(
            region
                .told(&seen, 0, usize::MAX)
                .iter()
                .any(|t| t.about.as_deref().is_some_and(|a| a.contains(&pick))),
            "no row of the picker is said on the explanation line"
        );
        assert_eq!(region.key(Key::simple(Sym::Esc), &mut w), Taken::Yes);
        assert_eq!(region.key(Key::simple(Sym::Esc), &mut w), Taken::Yes);
        assert!(!region.view.picking(), "the way back left the picker up");
        choose(&mut region, &mut w);
        region.pasted(&list.to_string_lossy(), &mut w);
        assert_eq!(region.key(Key::simple(Sym::Enter), &mut w), Taken::Yes);
        assert!(
            !region.view.picking(),
            "the picker stayed up after its answer"
        );
        // THE FEED'S ANSWER, said once it arrives.
        let staged = texts.t_args(
            crate::key::VIDEO_METRICS_IMPORTED,
            &[
                ("staged", ronoctua_i18n::Arg::Number(2)),
                ("already", ronoctua_i18n::Arg::Number(0)),
            ],
        );
        let said = |w: &Recorder| {
            w.owed
                .iter()
                .any(|e| matches!(e, Effect::Say(t, _) if *t == staged))
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        while !said(&w) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(heard.recv_timeout(left).is_ok(), "the feed never answered");
            region.woken(&mut w);
        }
        let kept = residence.as_a::<Kept>().expect("the resident");
        assert!(
            kept.borrow().readings.staged.len() == 2,
            "the import was not read back"
        );
        drop(region);
        let _ = std::fs::remove_dir_all(&dir);
        // GOOGLE TAKEOUT, in Chrome — the region closed first.
        let mut region = over(&Page::Import, false);
        let mut w = way();
        let rows = region.view_rows(COLUMNS, &texts);
        let n = rows
            .iter()
            .position(|r| r.spot == "takeout")
            .expect("takeout");
        region.clicked(n, &mut w);
        w.owed.clear();
        region.clicked(n, &mut w);
        assert!(
            w.owed.starts_with(&[
                Effect::Close,
                Effect::Run(
                    "kiosk-open",
                    Some(ronoctua_video_metrics::takeout::TAKEOUT_PAGE.to_owned())
                ),
            ]),
            "{:?}",
            w.owed
        );
    }
}
