//! THE VIDEO CATALOGUE IN THE FOOTER'S REGION (`LEADER v v`) — the view
//! (`crate::video_metrics`) as a `region::Region` of its own crate, and what
//! the footer keeps of it with the region closed (`region::Resident`).
//!
//! # One errand handler, two doors
//!
//! What the view asks of a door is carried out by the ONE function both doors
//! call ([`crate::video_metrics_feed::carry_out`]): the region here, the window
//! in `crate::video_metrics_window`. What is left to each door is what only
//! it can do: the transfers' queue and the Chrome kiosk are the footer's —
//! the window asks them down the footer's socket, and this region closes
//! first, for the footer shows one view at a time.
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
use crate::video_metrics_feed::{Arrived, Feed, Want};

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
    /// view — while the view is open, or a scan asked with it closed waits
    /// for its answer.
    feed: Option<Feed>,
    /// Where the feed reads the catalogue: this machine's
    /// (`ronoctua_video_metrics::default_path`), found when the footer starts
    /// — `None` when there is no home, and every look is refused.
    catalogue: Option<std::path::PathBuf>,
}

impl Kept {
    /// The feed and the readings, when they are not up yet — and what the
    /// door fills in the readings: the clock and how the queue is opened.
    fn awake(&mut self, way: &dyn Doorway) {
        if self.feed.is_none() {
            let wake = way.wake();
            self.feed = Some(Feed::start(self.catalogue.clone(), move || wake.call()));
        }
        self.readings.now_s = crate::video_metrics::now_s();
        self.readings.transfers_keys =
            crate::bindings::keys("transfers-open", None).unwrap_or_default();
    }

    /// `want`, asked of the feed.
    fn want(&self, want: Want) {
        if let Some(f) = self.feed.as_ref() {
            f.want(want);
        }
    }

    /// What the feed brought, taken: readings merged, and the covers the
    /// region draws read again.
    fn take(&mut self) -> Vec<Arrived> {
        let arrived = self.feed.as_ref().map(Feed::take).unwrap_or_default();
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
    let (view, first) = View::fresh(page.unwrap_or(Page::Home));
    VideoMetrics {
        kept,
        view,
        first: Some(first),
        covers,
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
}

impl VideoMetrics {
    /// The view's rows as the region's lines: fields, the caret, the lines
    /// that continue a row, and each thumbnail over its lines as a COVER —
    /// with its picture's key and size, once it is decoded — and what the
    /// pointer lights: a shelf of the grid the CELL under it, a line of a
    /// picture or a title nothing, any other row its band. A line takes a
    /// click where the view says one does (`View::takes`, the window's
    /// answer too).
    fn told_whole(&self, seen: &Seen<'_>) -> Vec<Told> {
        let kept = self.kept.borrow();
        let rows = self.view.rows(&kept.readings, seen.columns, seen.texts);
        let takes = self.view.takes(&rows);
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
                        if !o.cells.is_empty() {
                            crate::region::LineLights::UnderCell(
                                o.cells.iter().map(|c| (c.from, c.width)).collect(),
                            )
                        } else if o.heading || !o.thumbs.is_empty() {
                            crate::region::LineLights::Nothing
                        } else {
                            crate::region::LineLights::Row
                        }
                    });
                Told {
                    row: Row {
                        fields: r.fields.clone(),
                        takes: takes.get(n).copied().unwrap_or(false),
                    },
                    about: View::about(&rows, n, seen.texts),
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
    /// and the footer's own — the queue and the kiosk, which take the
    /// region's place.
    fn errand(&mut self, errand: Errand, way: &mut dyn Doorway) {
        match crate::video_metrics_feed::carry_out(errand, way) {
            Ok(Some(want)) => self.kept.borrow().want(want),
            Ok(None) => {}
            // THE QUEUE (`LEADER t t`), where a download handed over is seen
            // going — the view closes first, so the region is free.
            Err(Errand::Transfers) => {
                way.close();
                way.run("transfers-open", None);
            }
            // CLOSE BEFORE CHROME OPENS, the launcher's rule: a view left
            // open over the window that just opened keeps its keyboard.
            Err(Errand::Kiosk(url)) => {
                way.close();
                way.run("kiosk-open", Some(url));
            }
            Err(_) => {}
        }
    }

    /// A key of the view — from the keyboard, the hint, or `RET` from a
    /// second click (invariant 19).
    fn press(&mut self, press: Press, way: &mut dyn Doorway) {
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
            let (_, room) = crate::video_metrics::extent(way.columns());
            let errand = self.view.text_key(key, room);
            self.errand(errand, way);
            way.follow();
            way.repaint();
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

    /// The clipboard's text, read off the loop for `C-v` in the description.
    fn pasted(&mut self, text: &str, way: &mut dyn Doorway) {
        if self.view.text_pasted(text) {
            way.repaint();
        }
    }

    /// What the feed brought: readings merged, what an errand came to said —
    /// and a reading never moves the page: the cursor is kept on its row. A
    /// question the view put there from an errand's answer is followed, as
    /// the key it answers would have been.
    fn woken(&mut self, way: &mut dyn Doorway) {
        let arrived = self.kept.borrow_mut().take();
        if arrived.is_empty() {
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
    /// one handler (`video_metrics_feed::carry_out`), and what is the
    /// footer's alone (the queue, the kiosk) is handed back to each door.
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
        ] {
            assert!(crate::video_metrics_feed::carry_out(errand, &mut footer).is_err());
        }
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
}
