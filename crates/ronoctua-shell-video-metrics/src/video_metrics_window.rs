//! Video-metrics in a window of its own — the second door onto
//! `crate::video_metrics` (`LEADER v w`).
//!
//! A window and nothing else: which rows exist and what a key does to them
//! is `video_metrics.rs`, and both doors ask the same view the same
//! questions. What is here is the plumbing of THIS process: its own feed
//! (`crate::video_metrics_feed`, one thread for everything that touches the
//! catalogue or another process), the thumbnails drawn over the rows, the
//! grid's cells taken by the pointer, the line it asks on and the clipboard
//! it pastes from.
//!
//! # It does what the footer does (the owner's rule)
//!
//! It types — an address, a score, a band, words, tags on the header's line
//! (`Ui::ask`, `C-v` pastes there), and the second description on the rows
//! themselves (`text_area`). It answers the page's questions with the "no"
//! first. It opens a video — or a channel's videos — in Chrome through the
//! footer's `kiosk-open`, the brain's page and the player's window through
//! the footer's commands (`Ui::tell_the_footer`, off the loop), and in Nyxt
//! by its entry in the launcher, started here; what went wrong anywhere is
//! said on THIS window's line.
//!
//! # A foot that explains (the owner, 2026-11-09)
//!
//! Every row says what it is, in the foot the window keeps
//! (`Content::foot_lines`, the update window's): the thing the pointer rests
//! on — its name, what a click does to it, its sentence and what it follows
//! "as everywhere" — until a key comes, and then the cursor's thing, which
//! `RET` does (`View::explained_at`, `View::explained_cursor`). Nothing of
//! it goes to the footer's minibuffer: line 0 carries the keys, a message
//! and a question, never a row's sentence.

use anyhow::Result;
use smithay_client_toolkit::reexports::calloop::channel;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Canvas, Content, Door, Hand, Ui};
use crate::video_metrics::{Errand, Page, Readings, Row, Said, View};
use crate::video_metrics_feed::{Arrived, Direct, Feed, Leaving, Want};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.video-metrics";

/// The words `ronoctua-shell video-metrics <word>` opens on — the command
/// line's, so English.
pub const OPENINGS: [&str; 6] = ["home", "videos", "channels", "rules", "weights", "import"];

/// The page a word of the command line opens on; `None` for a word that
/// names none.
#[must_use]
pub fn opening_of(word: &str) -> Option<Page> {
    Some(match word.trim() {
        "" | "home" => Page::Home,
        "videos" => Page::Videos,
        "channels" => Page::Channels,
        "rules" => Page::Rules,
        "weights" => Page::Weights,
        "import" => Page::Import,
        _ => return None,
    })
}

/// Opens the view in a window, on `opening`.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(opening: Page) -> Result<()> {
    crate::door::run(move |w| {
        // ONE PING FOR EVERYTHING THE THREAD BRINGS, answered by taking all
        // of it.
        let (wake, woke) = channel::channel::<()>();
        w.handle
            .insert_source(woke, |event, (), door: &mut Door<VideoMetricsWindow>| {
                if let channel::Event::Msg(()) = event {
                    door.content.woke(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the video-metrics channel: {e}"))?;
        // THE KEYS A SENTENCE TEACHES come from the leader's own trees: this
        // window is a process of its own, and asks the tables the footer walks.
        let readings = readings_for(crate::video_metrics::transfers_keys());
        let sender = std::sync::Mutex::new(wake);
        let feed = Feed::start(ronoctua_video_metrics::default_path(), move || {
            if let Ok(s) = sender.lock() {
                let _ = s.send(());
            }
        });
        let (mut view, errand) = View::fresh(opening);
        // THE FILE PICKER'S FOLDER, watched through the door's wake
        // (`Content::woken`).
        view.watch_with(w.wake());
        // A WINDOW'S VIEW: its sentences teach a shortcut as `LEADER Alt`.
        view.in_a_window();
        let mut me = VideoMetricsWindow {
            view,
            readings,
            reach: Reach::Feed(feed),
            pointer_x: None,
            pointed: None,
            keyed: false,
            since_ms: 0,
            rested: None,
            in_the_foot: false,
            cursor_row: std::cell::Cell::new(None),
        };
        me.first(errand);
        Ok(me)
    })
}

/// The readings a door starts from: the machine's zone, now, and how the
/// queue is opened.
#[must_use]
pub fn readings_for(transfers_keys: String) -> Readings {
    Readings {
        zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        now_s: crate::video_metrics::now_s(),
        transfers_keys,
        ..Readings::default()
    }
}

/// WHERE THE POINTER RESTS, AND WHAT IS THERE — the update window's `Spot`:
/// the rest the foot keeps is the THING's, never the line's. A shelf's line
/// holds several videos, and one video spans the shelf's lines: keyed by the
/// line, a video slid past along its line was taken for the one rested on,
/// and a move down the same video was a move off it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spot {
    /// Its line and the cell across it, as a field's `at` — where the foot
    /// asks what it explains (`View::explained_at`).
    at: (usize, u16),
    /// What is there, by the identity the foot explains it under
    /// (`View::thing_at`) — a row, a video of a shelf, a glyph; `None` on a
    /// title, a note, the air, which explain nothing and are never rested
    /// on.
    thing: Option<String>,
}

/// The thing a spot is on, if any.
fn thing_of(spot: Option<&Spot>) -> Option<&str> {
    spot.and_then(|s| s.thing.as_deref())
}

/// Where the window's wants go.
enum Reach {
    /// Its thread.
    Feed(Feed),
    /// On the spot — a photograph's, which carries no errand out and prints
    /// what it would have done. Boxed: it holds the catalogue's connections
    /// and the decoded pictures, and the window holds one of the two.
    Direct(Box<Direct>),
}

/// The window.
pub struct VideoMetricsWindow {
    view: View,
    readings: Readings,
    reach: Reach,
    /// Where the pointer last was across the window, in logical pixels — the
    /// grid's cell a click takes is read from it, by the arithmetic the rows
    /// were drawn with.
    pointer_x: Option<f64>,
    /// WHAT THE POINTER RESTS ON — the foot's subject while no key came
    /// after it.
    pointed: Option<Spot>,
    /// A key came after the pointer last moved: the foot explains the
    /// cursor's thing until the pointer moves again.
    keyed: bool,
    /// When the pointer came onto the THING it is on, by the compositor's
    /// clock (a motion's `time`, in milliseconds).
    since_ms: u32,
    /// THE LAST THING THE POINTER RESTED ON — `space.window.footRestTenths`
    /// or longer — since the last key: what the foot explains once the
    /// pointer is in it, and not the things it crossed to get there.
    rested: Option<Spot>,
    /// The pointer is in the foot, which hears its motions: the next one the
    /// window hears is the pointer coming back.
    in_the_foot: bool,
    /// The cursor's row when the rows were last built — STORED WHERE IT IS
    /// MEASURED, for the door, which asks without a width.
    cursor_row: std::cell::Cell<Option<usize>>,
}

impl VideoMetricsWindow {
    /// The window over the catalogue at `catalogue`, read and written on the
    /// spot — what the photograph draws, over a copy of a fixture.
    #[must_use]
    pub fn detached(catalogue: std::path::PathBuf, opening: Page, transfers_keys: String) -> Self {
        let (mut view, errand) = View::fresh(opening);
        view.in_a_window();
        let mut me = Self {
            view,
            readings: readings_for(transfers_keys),
            reach: Reach::Direct(Box::new(Direct::at(catalogue))),
            pointer_x: None,
            pointed: None,
            keyed: false,
            since_ms: 0,
            rested: None,
            in_the_foot: false,
            cursor_row: std::cell::Cell::new(None),
        };
        me.first(errand);
        me
    }

    /// The opening's errand — a look — sent before the loop turns.
    fn first(&mut self, errand: Errand) {
        if let Errand::Look(look) = errand {
            self.want(None, Want::Look(look));
        }
    }

    /// Asks the feed; on the spot, what arrived is taken at once.
    fn want(&mut self, ui: Option<&mut Ui>, want: Want) {
        match &mut self.reach {
            Reach::Feed(f) => f.want(want),
            // WHAT A PHOTOGRAPH WOULD HAVE DONE past the catalogue is
            // printed by the feed on the spot, never done.
            Reach::Direct(d) => {
                let arrived = d.want(want);
                match ui {
                    Some(ui) => self.took(ui, arrived),
                    None => {
                        for a in arrived {
                            if let Arrived::Reading(r) = a {
                                self.readings.took(*r);
                            }
                        }
                    }
                }
            }
        }
    }

    /// The thread brought something: take all of it — and a turned page's
    /// title put first, once its reading is in ([`Self::place_the_title`]).
    fn woke(&mut self, ui: &mut Ui) {
        let arrived = match &self.reach {
            Reach::Feed(f) => f.take(),
            Reach::Direct(_) => Vec::new(),
        };
        self.took(ui, arrived);
        self.place_the_title(ui);
    }

    /// A TURNED PAGE'S TITLE PUT FIRST in the window, once its reading is in
    /// (`View::settle_on_list`): the cursor on its first video, and the door
    /// keeps the place from the follow that ends this gesture
    /// (`Ui::place_top`). Called AFTER this window's own follow, which would
    /// put the title a line back by its margin: at the end of a key and of a
    /// glyph's click, and when the thread brought the page — never inside
    /// [`Self::took`], which a photograph's reading reaches in the middle of
    /// the key that asked for it.
    fn place_the_title(&mut self, ui: &mut Ui) {
        let rows = self.view_rows(ui);
        let Some(top) = self.view.settle_on_list(&rows, &self.readings) else {
            return;
        };
        let rows = self.view_rows(ui);
        self.cursor_row.set(Some(self.cursor_of(&rows)));
        ui.place_top(top, rows.len());
    }

    /// What arrived, taken: readings merged — the cursor kept on its row, the
    /// page never moved by a reading — and what an errand came to said on
    /// this line. A question the view put on the page from an errand's
    /// answer is followed, as a key's would be: it answers one.
    fn took(&mut self, ui: &mut Ui, arrived: Vec<Arrived>) {
        if arrived.is_empty() {
            return;
        }
        self.readings.now_s = crate::video_metrics::now_s();
        for a in arrived {
            match a {
                Arrived::Reading(r) => self.readings.took(*r),
                Arrived::Done(done) => {
                    let (said, next) = self.view.took(done, &self.readings, &ui.texts);
                    match said {
                        Some(Said::Normal(text)) => ui.say(text),
                        Some(Said::Long(text)) => ui.say_for(text, crate::echo::Echo::Long),
                        None => {}
                    }
                    self.apply(ui, next);
                }
            }
        }
        let rows = self.view_rows(ui);
        self.view.settle(&rows);
        if self.view.follow_owed() {
            self.follow(ui);
        }
        ui.ask_paint();
    }

    /// The rows, at this window's width, in its language — the page told
    /// first: the room a turned page's title keeps under it is a page of this
    /// window (`View::page_is`), the one it has now.
    fn view_rows(&self, ui: &Ui) -> Vec<Row> {
        self.view.page_is(ui.room_for_rows());
        self.view.rows(&self.readings, ui.columns, &ui.texts)
    }

    /// Where the cursor is among `rows` — the caret's line while a text is
    /// edited.
    fn cursor_of(&self, rows: &[Row]) -> usize {
        self.view.drawn_cursor(rows)
    }

    /// Keeps the cursor's row in sight after a key: its whole shelf, on the
    /// grid — the last line first, then the first, so a shelf taller than
    /// the room shows its top.
    fn follow(&self, ui: &mut Ui) {
        let was = ui.offset;
        let rows = self.view_rows(ui);
        let at = self.cursor_of(&rows);
        self.cursor_row.set(Some(at));
        let last = at + Row::lines_of(&rows, at) - 1;
        ui.follow(last, rows.len());
        ui.follow(at, rows.len());
        self.keep_the_title(ui, &rows, was);
    }

    /// THE LIST'S TITLE A TURN PUT FIRST STAYS FIRST after a gesture that
    /// began with it there (`was`, the window's first row then) and left the
    /// cursor's row on the page under it (`View::title_kept`) — and is kept
    /// from the follow that ends the gesture (`Ui::place_top`): its margin
    /// would scroll the title back down a line.
    fn keep_the_title(&self, ui: &mut Ui, rows: &[Row], was: usize) {
        if let Some(top) = self.view.title_kept(rows, was, ui.room_for_rows()) {
            ui.place_top(top, rows.len());
        }
    }

    /// A key of the view — from the keyboard, the hint, or `RET` from a
    /// second click (invariant 19) — and the foot explains the cursor's
    /// thing again, until the pointer moves.
    fn pressed(&mut self, ui: &mut Ui, press: crate::video_metrics::Press) {
        ui.message = None;
        self.keyed = true;
        self.rested = None;
        let rows = self.view_rows(ui);
        let errand = self.view.press(press, &rows, &self.readings);
        self.apply(ui, errand);
        self.follow(ui);
        self.place_the_title(ui);
        ui.ask_paint();
    }

    /// Carries out what the view asked for — through the ONE handler both
    /// doors call (`video_metrics_feed::carry_out`), and what is this door's
    /// ([`Leaving`]): the queue, the kiosk, the brain's page and the player's
    /// window, the footer's, asked down its socket; Nyxt, started here.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        match crate::video_metrics_feed::carry_out(errand, ui) {
            Ok(Some(want)) => self.want(Some(ui), want),
            Ok(None) => {}
            // THE QUEUE IS THE FOOTER'S (`LEADER t t`): asked down its socket,
            // off the loop; a footer whose region holds other work says so
            // there, and one that is not there is said here.
            Err(Leaving::Transfers) => {
                if matches!(self.reach, Reach::Direct(_)) {
                    println!("would have told the footer: transfers-open");
                } else {
                    ui.tell_the_footer("transfers-open", None);
                }
            }
            // THE KIOSK IS THE FOOTER'S: it holds the bridge and the tabs.
            // The order goes down its socket off the loop, and a footer that
            // is not there is said here (`door-no-footer`).
            Err(Leaving::Kiosk(url)) => {
                if matches!(self.reach, Reach::Direct(_)) {
                    println!("would have told the footer: kiosk-open {url}");
                } else {
                    ui.tell_the_footer("kiosk-open", Some(&url));
                }
            }
            // NYXT, started by this window itself through its entry in the
            // launcher (`launch::in_nyxt`): off the loop, and a machine
            // without it said here (`door-nyxt-absent`).
            Err(Leaving::Nyxt(url)) => {
                if matches!(self.reach, Reach::Direct(_)) {
                    println!("would have opened in Nyxt: {url}");
                } else {
                    crate::region::Doorway::launch(ui, crate::launch::in_nyxt(&url));
                }
            }
            // THE BRAIN'S PAGE IS THE FOOTER'S: it opens the brain's window,
            // on this catalogue's scan.
            Err(Leaving::Brain) => {
                let module = ronoctua_brain::compiled::video_metrics::ID;
                if matches!(self.reach, Reach::Direct(_)) {
                    println!(
                        "would have told the footer: {} {module}",
                        crate::video_metrics::BRAIN_OPEN
                    );
                } else {
                    ui.tell_the_footer(crate::video_metrics::BRAIN_OPEN, Some(module));
                }
            }
            // A WINDOW OF THE PLAYER'S, of its own even with one open — the
            // player's command, which the footer runs.
            Err(Leaving::Player(word)) => {
                if matches!(self.reach, Reach::Direct(_)) {
                    println!(
                        "would have told the footer: {} {word}",
                        crate::video_metrics::PLAYER_WINDOW
                    );
                } else {
                    ui.tell_the_footer(crate::video_metrics::PLAYER_WINDOW, Some(word));
                }
            }
        }
    }

    /// The pointer left the thing it was on at `time` (`None`: a moment the
    /// compositor did not stamp): kept as what it rested on when it stayed
    /// there `space.window.footRestTenths` or longer — the update window's
    /// rule, for the foot to keep explaining it once the pointer is there.
    /// What explains nothing (a title, the air) is never kept: it would take
    /// the place of the thing rested on before it.
    fn moved_on(&mut self, time: Option<u32>) {
        let rest_ms = u32::from(ronoctua_tokens::space::WINDOW_FOOTRESTTENTHS) * 100;
        if let (Some(t), Some(left)) = (time, &self.pointed)
            && left.thing.is_some()
            && t.wrapping_sub(self.since_ms) >= rest_ms
        {
            self.rested = Some(left.clone());
        }
        if let Some(t) = time {
            self.since_ms = t;
        }
    }

    /// The cell of a row the pointer is over: the window's column less its
    /// margin — the arithmetic the rows are drawn with (`list::body` puts a
    /// field at `margin + at`).
    fn column_at(&self, ui: &Ui) -> u16 {
        let cell = f64::from(crate::chrome::cell_width());
        let x = self.pointer_x.unwrap_or(0.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let column = if cell > 0.0 {
            (x / cell).floor().max(0.0) as u16
        } else {
            0
        };
        column.saturating_sub(ui.margin)
    }
}

/// A thumbnail on screen: its file, its box's corner in pixels, its box in
/// cells and lines, and the box's lines that are on screen `(first, end)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// The picture's file.
    pub file: std::path::PathBuf,
    /// The box's left edge, in pixels.
    pub x: f32,
    /// The box's top edge, in pixels — above the screen when its first lines
    /// are scrolled away.
    pub top: f32,
    /// The box, in cells and lines.
    pub size: (u16, u16),
    /// The box's lines on screen.
    pub visible: (u16, u16),
}

/// Where each thumbnail on screen goes: one per picture, from its first line
/// on screen, cut where the rows are cut — the box
/// `player_metadata::cover_place` fits the picture into, the drawing the
/// player's covers already share between the doors. `shown` is the rows on
/// screen, the first at screen line `first`; `x_of` is where a field's cell
/// begins, in pixels.
#[must_use]
pub fn thumbs_placed(shown: &[&Row], first: u16, x_of: impl Fn(u16) -> f32) -> Vec<Placed> {
    let tall = ronoctua_tokens::metrics::line_height(ronoctua_tokens::text::size());
    let mut out = Vec::new();
    for (i, row) in shown.iter().enumerate() {
        for t in &row.thumbs {
            let same = |r: &&Row| {
                r.thumbs
                    .iter()
                    .any(|o| o.file == t.file && o.left == t.left)
            };
            // ONCE A PICTURE, from the first of its lines on screen.
            if i > 0 && shown.get(i - 1).is_some_and(same) {
                continue;
            }
            let run = shown[i..].iter().take_while(|r| same(r)).count();
            let screen = first.saturating_add(u16::try_from(i).unwrap_or(u16::MAX));
            out.push(Placed {
                file: t.file.clone(),
                x: x_of(t.left),
                top: (f32::from(screen) - f32::from(t.line)) * tall,
                size: (t.cells, t.lines),
                visible: (
                    t.line,
                    t.line
                        .saturating_add(u16::try_from(run).unwrap_or(u16::MAX)),
                ),
            });
        }
    }
    out
}

impl Content for VideoMetricsWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    /// NOT `TAB` FROM SECTION TO SECTION (`section_ring`): its question's `TAB` picks the answer.
    fn walks_sections(&self, _: &Ui) -> bool {
        false
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::VIDEO_METRICS_TITLE))
    }

    /// Its name at the right end of the first line, with no header — or, on
    /// a page below the first, the way back (`door::corner`).
    fn corner(&self, ui: &Ui) -> Option<crate::door::corner::Corner> {
        Some(crate::door::corner::Corner::named_or_back(
            &ui.texts,
            crate::key::VIDEO_METRICS_TITLE,
            self.view.below_the_first(),
        ))
    }

    /// A key, read by its whole notation through the reader the footer calls
    /// too — or, while a text is edited, the text's.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        if self.view.editing() {
            self.keyed = true;
            self.rested = None;
            let (_, room) = crate::video_metrics::extent(ui.columns);
            let errand = self.view.text_key(key, room);
            self.apply(ui, errand);
            self.follow(ui);
            ui.ask_paint();
            return;
        }
        // THE FILE PICKER, while it is up over the import page, takes what
        // it reads — every letter, and its own keys — before the view's
        // table: told first how many lines the window shows, for its
        // `PgUp` and `PgDn`.
        if self.view.picker_takes(key) {
            self.view.page_is(ui.room_for_rows());
            self.pressed(ui, crate::video_metrics::Press::Pick(key));
            return;
        }
        if let Some(press) = crate::video_metrics::press_of_key(key) {
            self.pressed(ui, press);
        }
    }

    /// THE FILE PICKER'S FOLDER MOVED, or its devices were read — through
    /// the door's wake: its rows again, when they changed. The catalogue's
    /// feed rings a channel of its own (`host_it`).
    fn woken(&mut self, ui: &mut Ui) {
        if self.view.picker_woke() {
            ui.ask_paint();
        }
    }

    /// THE FOOT THIS WINDOW KEEPS (the owner, 2026-11-09, the update
    /// window's): always this tall, so nothing above it moves — what is
    /// explained there changes, the rows do not.
    fn foot_lines(&self, _: &Ui) -> u16 {
        ronoctua_tokens::space::WINDOW_FOOTLINES
    }

    /// WHAT THE FOOT EXPLAINS: the thing the pointer rests on — a row, a
    /// video of a shelf, a glyph — as a click takes it, until a key comes;
    /// then, and with the pointer on nothing a click takes (a title, the
    /// air), the cursor's thing, which `RET` does.
    fn explained(&self, ui: &Ui) -> Option<crate::door::Explained> {
        let rows = self.view_rows(ui);
        if !self.keyed
            && let Some(p) = &self.pointed
            && let Some(e) = self.view.explained_at(&rows, p.at.0, Some(p.at.1), &ui.texts)
            // IN THE FOOT, THE THING RESTED ON, by its identity: a reading
            // that moved the rows under its old place gives the foot back
            // to the cursor's thing, and never explains another there.
            && (!self.in_the_foot || p.thing.as_deref() == Some(e.subject.as_str()))
        {
            return Some(e);
        }
        self.view.explained_cursor(&rows, &ui.texts)
    }

    /// THE POINTER WENT INTO THE FOOT: what the foot explains is what the
    /// pointer last rested on — not the rows it crossed on its way down —,
    /// held while the pointer is there, and the cursor's thing when it
    /// rested on none since the last key.
    fn pointer_went_to_the_foot(&mut self, ui: &mut Ui, _: &mut Hand<()>, time: Option<u32>) {
        self.moved_on(time);
        self.pointed.clone_from(&self.rested);
        self.in_the_foot = true;
        ui.ask_paint();
    }

    /// WHAT WAS TYPED ON THE HEADER'S LINE, to the same view call the
    /// footer's minibuffer reaches.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let errand = self.view.typed(id, text, &self.readings, &ui.texts);
        self.apply(ui, errand);
        ui.ask_paint();
    }

    fn unanswered(&mut self, ui: &mut Ui, _id: &'static str) {
        self.view.unanswered();
        ui.ask_paint();
    }

    /// The clipboard's text, read off the loop for `C-v` in the description
    /// — or in the file picker, as what was typed there.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        if self.view.text_pasted(text) || self.view.picker_pasted(text) {
            ui.ask_paint();
        }
    }

    /// CAPS LOCK MEANS CAPITALS while the second description is edited —
    /// and nowhere else here, where a letter is a key of the view (the
    /// footer's region answers the same, `region_types_capitals`).
    fn types_capitals(&self) -> bool {
        self.view.editing()
    }

    fn leader_alt_keys(&self) -> &'static [&'static [(&'static str, &'static str)]] {
        &[crate::video_metrics::LEADER_ALT_KEYS]
    }

    /// THE WHOLE TABLE, kept from the keyboard: the view reads a key on every
    /// row while the line teaches it only where it does something. Not while
    /// the second description is edited, nor while the file picker is up —
    /// every letter is its text, or the name the picker looks for.
    fn barred_keys(&self, ui: &Ui) -> Vec<ronoctua_keymap::Key> {
        if self.view.editing() || self.view.picking() {
            return crate::chrome::shortcut_keys(&self.hint(ui), self.leader_alt_keys());
        }
        crate::chrome::table_keys(crate::video_metrics::OF_THE_VIEW.iter().map(|r| r.0))
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        let rows = self.view_rows(ui);
        // THE WAY BACK FIRST, and clickable — while a text is edited its own
        // `ESC` is the way out, and the line teaches that.
        if self.view.editing() {
            return self.view.hint(&rows, &ui.texts);
        }
        crate::chrome::with_the_way_back(self.view.hint(&rows, &ui.texts), &ui.texts)
    }

    /// THE FOCUS IS A COLOUR, NOT A BAND — the same mark the footer draws.
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
    }

    /// A click takes a row the cursor stops on, every line of one, a shelf
    /// of the grid — and only those light under the pointer: the view's one
    /// answer (`View::takes`), which the footer's region gives too. Never a
    /// title, a note, the air between sections or a line of a picture.
    fn takes(&self, ui: &Ui) -> Vec<bool> {
        self.view.takes(&self.view_rows(ui))
    }

    /// Where the cursor was when the rows were last built — the door asks
    /// this with no width, and the rows depend on one.
    fn cursor(&self) -> Option<usize> {
        self.cursor_row.get()
    }

    /// THE CLICK SELECTS, AND THE SECOND ONE ACTS — invariant 19 — on a
    /// row, on a cell of the grid, and on an answer of the page's question.
    /// On the text being edited it places the caret.
    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let rows = self.view_rows(ui);
        let column = self.column_at(ui);
        if self.view.editing() {
            if let Some(hit) = crate::video_metrics::text_hit(&rows, row, column) {
                self.view.text_clicked(hit);
            }
            ui.ask_paint();
            return;
        }
        // A GLYPH ACTS ON THE FIRST CLICK — and what it asks is asked under
        // its video, which the door keeps in sight, as after a key. The foot
        // goes on explaining the glyph under the pointer, in its new state.
        if let Some(errand) = self.view.click_control(row, column, &rows, &self.readings) {
            ui.message = None;
            self.apply(ui, errand);
            self.follow(ui);
            self.place_the_title(ui);
            ui.ask_paint();
            return;
        }
        if let Some(errand) = self.view.click_answers(row, &rows) {
            self.apply(ui, errand);
            ui.ask_paint();
            return;
        }
        if self.view.clicked(row, column, &rows) {
            self.pressed(ui, crate::video_metrics::Press::Enter);
        } else {
            // A CLICK THAT ONLY CHOSE — the first video under a title a turn
            // put first — leaves the title there (`Self::keep_the_title`).
            let rows = self.view_rows(ui);
            self.keep_the_title(ui, &rows, ui.offset);
            ui.ask_paint();
        }
    }

    /// The pointer: where it is across the window is kept for the grid's
    /// cells, and the rest is the standard list's — and what it rests on is
    /// the foot's subject until a key.
    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<Self::Hand>, events: &[PointerEvent]) {
        let was = (self.pointed.clone(), self.keyed);
        // THE ROWS, built once for every motion of the frame: what is under
        // the pointer is asked of them.
        let moved = events.iter().any(|e| {
            matches!(
                e.kind,
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. }
            )
        });
        let rows = if moved {
            self.view_rows(ui)
        } else {
            Vec::new()
        };
        for e in events {
            match e.kind {
                // THE POINTER MOVED: what it rests on is the foot's again —
                // and the THING it left, when it rested there long enough to
                // be read, is what the foot keeps explaining once the pointer
                // is in it. In the order the events came: a click after the
                // motion is a key's, and gives the foot back to the cursor.
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_x = Some(e.position.0);
                    let time = match e.kind {
                        PointerEventKind::Motion { time } => Some(time),
                        _ => None,
                    };
                    let now = ui.row_at(e.position.0, e.position.1).map(|n| {
                        let column = self.column_at(ui);
                        Spot {
                            at: (n, column),
                            thing: self.view.thing_at(&rows, n, Some(column)),
                        }
                    });
                    self.keyed = false;
                    if std::mem::take(&mut self.in_the_foot) {
                        // BACK FROM THE FOOT: what it is on starts now.
                        if let Some(t) = time {
                            self.since_ms = t;
                        }
                    } else if thing_of(now.as_ref()) != thing_of(self.pointed.as_ref()) {
                        self.moved_on(time);
                    }
                    self.pointed = now;
                }
                PointerEventKind::Press { .. } => self.pointer_x = Some(e.position.0),
                PointerEventKind::Leave { .. } => {
                    self.pointer_x = None;
                    self.pointed = None;
                    self.rested = None;
                    self.in_the_foot = false;
                }
                _ => {}
            }
        }
        let before = self.pointer_x;
        crate::door::list::pointer(self, ui, hand, events);
        if self.pointed != was.0 || self.keyed != was.1 {
            ui.ask_paint();
        }
        // A POINTER MOVING ALONG A SHELF lights another cell on the same row.
        if before.is_some() && self.pointer_x != before {
            ui.ask_paint();
        }
        if events
            .iter()
            .any(|e| matches!(e.kind, PointerEventKind::Motion { .. }))
        {
            ui.ask_paint();
        }
    }

    /// The standard window — header, rows, hover, bar — and what this view
    /// adds over its rows: the thumbnails, the caret of a text being edited,
    /// and on the grid a hover that is the CELL, never a band across a shelf.
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let rows = self.view_rows(ui);
        // LINE 0: the question, a message — or the keys (`list::line_zero`'s
        // order, the footer's own). Never what the row under the pointer
        // is: the foot says that (`Content::explained`), and with no header
        // a row's sentence on line 0 went to the footer's minibuffer.
        crate::door::list::header_of(ui, canvas, None, |ui| self.hint(ui));
        // FOLLOW THE CURSOR once the width is known: the door asked
        // `cursor()` without one.
        let fields: Vec<Vec<crate::fields::Field>> =
            rows.iter().map(|r| r.fields.clone()).collect();
        let takes = self.view.takes(&rows);
        body(ui, canvas, &rows, &fields, &takes, self.pointer_x);
        let first = ui.first_of_list();
        let room = ui.room_for_rows();
        let shown: Vec<&Row> = rows.iter().skip(ui.offset).take(room).collect();
        let px = ronoctua_tokens::text::size();
        let (cell, tall) = (
            ronoctua_tokens::metrics::cell_width(px),
            ronoctua_tokens::metrics::line_height(px),
        );
        let margin = ui.margin;
        // THE CARET, on its line.
        for (i, row) in shown.iter().enumerate() {
            if let Some(at) = row.caret {
                let line = first + u16::try_from(i).unwrap_or(u16::MAX);
                canvas.under.push(crate::chrome::caret(
                    f32::from(margin) * cell,
                    f32::from(line) * tall,
                    cell,
                    tall,
                    at,
                ));
            }
        }
        // THE THUMBNAILS, each once, cut where the rows are cut.
        for placed in thumbs_placed(&shown, first, |left| f32::from(margin + left) * cell) {
            let Some(p) = self.readings.pictures.get(&placed.file) else {
                continue;
            };
            let Some((place, crop)) = crate::player_metadata::cover_place(
                placed.size,
                (cell, tall),
                p.size,
                placed.visible,
            ) else {
                continue;
            };
            let base = crate::video_metrics::picture_key(&placed.file);
            let key = if crop == (0.0, 1.0) {
                base
            } else {
                format!("{base}:{:.4}:{:.4}", crop.0, crop.1)
            };
            let rgba = std::sync::Arc::clone(&p.rgba);
            let uploaded = canvas.upload(&key, || {
                if crop == (0.0, 1.0) {
                    Some((*rgba).clone())
                } else {
                    crate::player_metadata::cropped(&rgba, crop)
                }
            });
            if uploaded {
                canvas.pictures.push((
                    key,
                    ronoctua_render::image::Area {
                        x: placed.x + place.x,
                        y: placed.top + place.y,
                        width: place.width,
                        height: place.height,
                    },
                ));
            }
        }
        self.cursor_row.set(Some(self.cursor_of(&rows)));
    }
}

/// The rows under the header: their text, the hover of every pointer — a
/// shelf's CELL on the grid, a row of several lines whole, and nothing on a
/// row a click takes nothing on (`takes`, the view's answer) — and the
/// list's bar, recorded as its own column.
fn body(
    ui: &mut Ui,
    canvas: &mut Canvas<'_>,
    rows: &[Row],
    fields: &[Vec<crate::fields::Field>],
    takes: &[bool],
    pointer_x: Option<f64>,
) {
    let size = ronoctua_tokens::text::size();
    let (under, track) = rows_laid(ui, rows, takes, canvas.lit, pointer_x);
    let first = ui.first_of_list();
    let room = ui.room_for_rows();
    for (i, line) in fields.iter().skip(ui.offset).take(room).enumerate() {
        let y = first + u16::try_from(i).unwrap_or(0);
        for f in line {
            canvas.spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text: f.text.clone(),
                column: ui.margin + f.at,
                line: y,
                color: f.color,
                edge: false,
                foot: false,
            });
        }
    }
    canvas.under.extend(under);
    canvas.bar(track);
}

/// WHAT THE ROWS LAY UNDER THEIR WORDS, in the order it is painted — later
/// on top — with the offset kept inside the list and the room measured
/// (`Ui::room`): a row's own ground, every pointer's light (`lit`, a
/// pointer's colour and row, as `Canvas::lit`), and the list's bar, with
/// its track. Apart from the painter so a test reads what a frame lays
/// with no GPU behind it.
fn rows_laid(
    ui: &mut Ui,
    rows: &[Row],
    takes: &[bool],
    lit: &[(ronoctua_tokens::Color, Option<usize>)],
    pointer_x: Option<f64>,
) -> (Vec<ronoctua_render::quad::Quad>, Option<crate::bars::Track>) {
    ui.settle_offset(rows.len());
    let first = ui.first_of_list();
    let room = ui.room_for_rows();
    let scroll = crate::chrome::list_scroll(rows.len(), room, ui.offset);
    #[allow(clippy::cast_precision_loss)]
    let (w, h) = (ui.width as f32, ui.height as f32);
    ui.room = crate::chrome::Room::of(scroll, w, h);
    let mut under = Vec::new();
    // A ROW'S OWN GROUND — a question's block — FIRST, edge to edge: the
    // hover, a cell's light and a glyph's are painted over it, so the hand
    // still reads on the block it rests on.
    let grounds: Vec<Option<ronoctua_tokens::Color>> = rows.iter().map(|r| r.ground).collect();
    under.extend(crate::chrome::row_grounds(
        &grounds,
        ui.offset,
        room,
        first,
        ui.room.band_width(),
    ));
    let px = ronoctua_tokens::text::size();
    let (cell, tall) = (
        ronoctua_tokens::metrics::cell_width(px),
        ronoctua_tokens::metrics::line_height(px),
    );
    // EVERY POINTER'S HOVER, in its own colour, and only on a row a click
    // takes: a title, a note, the air and a line of a picture light nothing.
    // A line of a shelf lights the CELL under the pointer, over the shelf's
    // lines — a band across it would light seven videos to say the pointer
    // is on one.
    let mut plain: Vec<(ronoctua_tokens::Color, Option<usize>)> = Vec::new();
    // THE CELL OF A ROW THE POINTER IS OVER: the arithmetic the rows are
    // drawn with, the click's (`VideoMetricsWindow::column_at`).
    let column = pointer_x.map_or(0, |x| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let c = (x / f64::from(cell)).floor().max(0.0) as u16;
        c.saturating_sub(ui.margin)
    });
    let (left, _) = crate::video_metrics::extent(ui.columns);
    // THE BOXES LIT ON ONE LINE, `(colour, line, from, width)`: a glyph, or
    // a row's words beside its glyphs.
    let mut boxes: Vec<(ronoctua_tokens::Color, usize, u16, u16)> = Vec::new();
    for &(colour, at) in lit {
        let Some(n) = at.filter(|n| takes.get(*n).copied().unwrap_or(false)) else {
            continue;
        };
        let lights = lights_at(rows, n, column, left);
        plain.extend(lights.bands.into_iter().map(|k| (colour, Some(k))));
        boxes.extend(
            lights
                .boxes
                .into_iter()
                .map(|(k, from, wide)| (colour, k, from, wide)),
        );
        if let Some((owner, lines, from, wide)) = lights.cell {
            let top = owner.max(ui.offset);
            let bottom = (owner + lines).min(ui.offset + room);
            if bottom > top {
                let y = f32::from(first) * tall + (top - ui.offset) as f32 * tall;
                let x = f32::from(ui.margin + from) * cell;
                let wide = (f32::from(wide) * cell).min(ui.room.band_width() - x);
                under.push(ronoctua_render::quad::Quad {
                    x,
                    y,
                    width: wide.max(0.0),
                    height: (bottom - top) as f32 * tall,
                    color: colour,
                });
            }
        }
    }
    for (colour, n, from, wide) in boxes {
        if n < ui.offset || n >= ui.offset + room {
            continue;
        }
        let y = f32::from(first) * tall + (n - ui.offset) as f32 * tall;
        let x = f32::from(ui.margin + from) * cell;
        let wide = (f32::from(wide) * cell).min(ui.room.band_width() - x);
        under.push(ronoctua_render::quad::Quad {
            x,
            y,
            width: wide.max(0.0),
            height: tall,
            color: colour,
        });
    }
    under.extend(crate::chrome::hover_bands(
        plain.into_iter(),
        ui.offset,
        room,
        rows.len(),
        first,
        ui.room.band_width(),
    ));
    under.extend(crate::chrome::list_bar(scroll, first, w, h));
    (under, crate::chrome::list_track(scroll, first, w, h))
}

/// WHAT ONE POINTER LIGHTS, over the rows of the window ([`lights_at`]).
#[derive(Debug, Default, PartialEq, Eq)]
struct Lights {
    /// Lines lit across, as a band.
    bands: Vec<usize>,
    /// Boxes lit on one line, `(line, from, width)` as a field's `at`.
    boxes: Vec<(usize, u16, u16)>,
    /// A shelf's cell, lit over the shelf's lines: `(first line, lines,
    /// from, width)`.
    cell: Option<(usize, usize, u16, u16)>,
}

/// WHAT THE POINTER AT CELL `column` OF LINE `n` LIGHTS — a line a click
/// takes (`View::takes`), `left` the rows' first cell. A line of glyphs
/// lights the ONE box under the pointer: a glyph, on its own line — a click
/// there takes no more, and the air between two takes nothing
/// (the-pointer-lights-what-the-click-takes) —, or the row's words beside
/// them, which light the row whole, as its title does: a click on either
/// takes the same video. A line of a shelf lights the CELL under the
/// pointer, over the shelf's lines — a band across it would light seven
/// videos to say the pointer is on one. A line of a picture, nothing.
///
/// A ROW OF SEVERAL LINES LIGHTS WHOLE — a wrapped label, a video's title
/// and what is said under it — as a click on any of its words takes it
/// whole (`Row::owner`), from whichever of its lines the pointer is on: a
/// line of words its band, a line with glyphs only its words' box
/// (`Row::words_box`) — the glyphs beside them act on their own, and a band
/// over them would say the click takes them. A LINE BESIDE A PICTURE — a
/// video of the list with thumbnails — lights a box from where its words
/// begin (`Row::words_at`) to the band's end, and only under a pointer
/// from there on: the picture takes no click (`View::on_the_words`), so it
/// lights nothing — the footer's `LineLights::UnderX`.
fn lights_at(rows: &[Row], n: usize, column: u16, left: u16) -> Lights {
    let mut lights = Lights::default();
    let owner = Row::owner(rows, n);
    let (Some(row), Some(line)) = (rows.get(owner), rows.get(n)) else {
        return lights;
    };
    let whole = |lights: &mut Lights| {
        let lines = Row::lines_of(rows, owner);
        for (k, l) in rows.iter().enumerate().skip(owner).take(lines) {
            if !l.controls.is_empty() {
                if let Some((from, wide)) = l.words_box(left, row.has_words()) {
                    lights.boxes.push((k, from, wide));
                }
            } else if let Some(at) = l.words_at {
                // TO THE BAND'S END: the painter cuts a box at the room's
                // edge (`Room::band_width`), as it cuts every other.
                lights.boxes.push((k, at, u16::MAX - at));
            } else {
                lights.bands.push(k);
            }
        }
    };
    // ON A SHELF, the glyph under the pointer is its own, and anywhere else
    // the cell's video ([`crate::video_metrics::View::on_the_words`]).
    let on_a_glyph = line
        .controls
        .iter()
        .any(|c| crate::chrome::in_cell(column, c.from, c.width));
    if !line.controls.is_empty() && (row.cells.is_empty() || on_a_glyph) {
        let words = line.words_box(left, row.has_words());
        match line
            .lit_cells(left, row.has_words())
            .into_iter()
            .find(|&(from, wide)| crate::chrome::in_cell(column, from, wide))
        {
            Some(b) if Some(b) == words => whole(&mut lights),
            Some((from, wide)) => lights.boxes.push((n, from, wide)),
            None => {}
        }
    } else if !row.cells.is_empty() {
        lights.cell = row
            .cells
            .iter()
            .find(|c| crate::chrome::in_cell(column, c.from, c.width))
            .map(|c| (owner, Row::lines_of(rows, owner), c.from, c.width));
    } else if let Some(at) = line.words_at {
        // OVER THE PICTURE, nothing: its click takes nothing either.
        if column >= at {
            whole(&mut lights);
        }
    } else if line.thumbs.is_empty() {
        whole(&mut lights);
    }
    lights
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_metrics::Layout;

    /// The width the window is drawn at: ten videos make two shelves of the
    /// grid, of several cells each.
    const COLUMNS: u16 = 100;

    /// A window over the fixture's readings, on `page` — its videos laid out
    /// as `layout` says — as a paint leaves it (its cursor's row measured). Its feed
    /// answers on the spot (`Direct`), over a catalogue under `blocked`, a
    /// FILE where its folder would be: what a click writes is refused and
    /// said, nothing is made, and what it would ask of another process is
    /// printed, never done.
    fn a_window(
        blocked: &std::path::Path,
        page: &Page,
        layout: Layout,
    ) -> (VideoMetricsWindow, Ui) {
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        ui.columns = COLUMNS;
        let catalogue = blocked.join(ronoctua_video_metrics::FILE);
        let mut w = VideoMetricsWindow::detached(catalogue, page.clone(), String::new());
        w.readings = crate::video_metrics::tests::readings();
        w.view.laid_out(layout);
        let rows = w.view_rows(&ui);
        w.cursor_row.set(Some(w.cursor_of(&rows)));
        (w, ui)
    }

    /// FAILS IF: the window's resting pointer lights a row a click takes
    /// nothing on — a title, a note, the air between sections, a line of a
    /// picture — or leaves unlit one it takes, a line that continues a row
    /// included; or a shelf of the grid takes nothing where a click on one
    /// of its cells lands. THE LIGHT IS THE CLICK: every row clicked once,
    /// on a window of its own, and lit exactly where something happened —
    /// on every page the fixture fills, in every layout. The footer's
    /// region has its own.
    #[test]
    fn the_windows_pointer_lights_what_the_click_takes() {
        let blocked = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-window-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&blocked);
        std::fs::write(&blocked, b"").expect("a file where the catalogue's folder would be");
        let video = Page::Video(ronoctua_video_metrics::VideoId("video000000".to_owned()));
        // THE CURSOR AS DRAWN is seen too: a first click moves it, and the
        // door's `cursor` is measured only by a paint.
        let witness = |w: &VideoMetricsWindow, ui: &Ui| {
            (crate::door::seen(w, ui), w.cursor_of(&w.view_rows(ui)))
        };
        let mut shelves = 0;
        let mut glyphs = 0;
        let mut every = Vec::new();
        // EVERY LAYOUT OF THE VIDEOS: the list, with its thumbnails or
        // without, and the grid, with the channel or without.
        let pages = [
            Page::Home,
            Page::Channels,
            Page::Import,
            Page::Rules,
            Page::Weights,
        ]
        .map(|p| (p, Layout::List))
        .into_iter()
        .chain(Layout::ALL.map(|l| (Page::Videos, l)))
        .chain([(video, Layout::List)]);
        for (page, layout) in pages {
            let fresh = || a_window(&blocked, &page, layout);
            let (first, ui) = fresh();
            let rows = first.view_rows(&ui);
            let cursor = first.cursor();
            let owner = |n: usize| Row::owner(&rows, n);
            let held: Vec<crate::region::Held> = crate::door::held_to_the_click(fresh, witness)
                .into_iter()
                .map(|mut h| {
                    // A ROW OF SEVERAL LINES IS THE CURSOR'S on every line:
                    // a click on any of them is a click on it.
                    h.here = h.here || cursor == Some(owner(h.row));
                    // A SHELF IS CLICKED WHERE A CELL IS — the pointer over
                    // it — and the generic click has no pointer: the air
                    // before the first cell, which takes nothing and lights
                    // nothing.
                    // A LINE OF GLYPHS IS CLICKED WHERE A GLYPH IS, as a
                    // shelf is where a cell is: each acts on the first click
                    // (`View::click_control`).
                    let controls = rows.get(h.row).map(|r| r.controls.clone());
                    if let Some(controls) = controls.filter(|c| !c.is_empty()) {
                        glyphs += 1;
                        h.did = controls.iter().all(|c| {
                            let (mut w, mut ui) = fresh();
                            let cell = f64::from(crate::chrome::cell_width());
                            w.pointer_x = Some(cell * (f64::from(ui.margin + c.from) + 0.5));
                            let before = witness(&w, &ui);
                            w.clicked(&mut ui, h.row);
                            witness(&w, &ui) != before
                        });
                        return h;
                    }
                    // A LINE BESIDE A PICTURE IS CLICKED ON ITS WORDS, where
                    // the pointer lights it: over the picture the generic
                    // click's cell takes nothing, and lights nothing.
                    if let Some(at) = rows.get(h.row).and_then(|r| r.words_at) {
                        let (mut w, mut ui) = fresh();
                        let cell = f64::from(crate::chrome::cell_width());
                        w.pointer_x = Some(cell * (f64::from(ui.margin + at) + 0.5));
                        let before = witness(&w, &ui);
                        w.clicked(&mut ui, h.row);
                        h.did = witness(&w, &ui) != before;
                        return h;
                    }
                    let cells = rows.get(owner(h.row)).map(|o| o.cells.clone());
                    if let Some(cells) = cells.filter(|c| !c.is_empty()) {
                        shelves += 1;
                        h.did = cells.iter().all(|c| {
                            let (mut w, mut ui) = fresh();
                            let cell = f64::from(crate::chrome::cell_width());
                            w.pointer_x = Some(cell * (f64::from(ui.margin + c.from) + 0.5));
                            let before = witness(&w, &ui);
                            w.clicked(&mut ui, h.row);
                            w.clicked(&mut ui, h.row);
                            witness(&w, &ui) != before
                        });
                    }
                    h
                })
                .collect();
            every.push((format!("{page:?}, {layout:?}"), held));
        }
        let _ = std::fs::remove_file(&blocked);
        for (what, held) in every {
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

    /// FAILS IF: the window lays a question's ground anywhere but under the
    /// light the hand gives its answers — no ground at all, a block cut in
    /// several quads, one narrower or wider than a band (the room's, up to
    /// the bar), one off the block's lines (from its sentence to the end of
    /// its "yes"), or one laid AFTER the hover band of its "yes", which it
    /// would then hide. The ground goes in both doors: the footer's region
    /// tells its own (`the_region_tells_the_question_s_ground`).
    #[test]
    fn the_window_lays_the_question_s_ground_under_its_band() {
        use crate::chrome::QUESTION_GROUND;
        use ronoctua_tokens::color::SURFACE_HOVER;
        let blocked = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-window-ground-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&blocked);
        std::fs::write(&blocked, b"").expect("a file where the catalogue's folder would be");
        let video = Page::Video(ronoctua_video_metrics::VideoId("video000000".to_owned()));
        let (mut w, mut ui) = a_window(&blocked, &video, Layout::List);
        // THE DELETE QUESTION, asked from its row.
        let rows = w.view_rows(&ui);
        let delete = rows
            .iter()
            .position(|r| r.spot == "delete")
            .expect("the delete row");
        w.view.clicked(delete, 0, &rows);
        let rows = w.view_rows(&ui);
        let _ = w
            .view
            .press(crate::video_metrics::Press::Delete, &rows, &w.readings);
        let _ = std::fs::remove_file(&blocked);
        let rows = w.view_rows(&ui);
        let at = |spot: &str| {
            rows.iter()
                .position(|r| r.spot == spot)
                .unwrap_or_else(|| panic!("no row `{spot}`"))
        };
        let (no, yes) = (at("answer-no"), at("answer-yes"));
        // THE BLOCK: from the first line on the ground to the yes's last.
        let mut start = no;
        while start > 0 && rows[start - 1].ground.is_some() {
            start -= 1;
        }
        let end = yes + Row::lines_of(&rows, yes);
        assert!(start < no, "no sentence on the ground");
        ui.offset = start - 1;
        let takes = w.view.takes(&rows);
        let (under, _) = rows_laid(&mut ui, &rows, &takes, &[(SURFACE_HOVER, Some(yes))], None);
        let tall = ronoctua_tokens::metrics::line_height(ronoctua_tokens::text::size());
        #[allow(clippy::cast_precision_loss)]
        let y_of = |n: usize| (f32::from(ui.first_of_list()) + (n - ui.offset) as f32) * tall;
        let grounds: Vec<usize> = (0..under.len())
            .filter(|&k| under[k].color == QUESTION_GROUND)
            .collect();
        assert_eq!(grounds.len(), 1, "the block is not one ground: {under:?}");
        let g = &under[grounds[0]];
        assert!(
            g.x.abs() < f32::EPSILON && (g.width - ui.room.band_width()).abs() < f32::EPSILON,
            "the ground is not as wide as a band: {g:?}"
        );
        assert!(
            (g.y - y_of(start)).abs() < 0.5 && (g.y + g.height - y_of(end)).abs() < 0.5,
            "the ground is off the block's lines: {g:?}, lines {start}..{end}"
        );
        let band = (0..under.len())
            .find(|&k| under[k].color == SURFACE_HOVER && (under[k].y - y_of(yes)).abs() < 0.5)
            .expect("no band under the hand on the yes");
        assert!(
            grounds[0] < band,
            "the ground is laid over the yes's band, and hides it"
        );
        let b = &under[band];
        assert!(
            b.x >= g.x && b.x + b.width <= g.x + g.width + 0.5,
            "the band leaves the ground: {b:?} over {g:?}"
        );
    }

    /// A pointer's event at `at`, on the window's surface.
    fn event(ui: &Ui, at: (f64, f64), kind: PointerEventKind) -> PointerEvent {
        PointerEvent {
            surface: ui.surface.clone(),
            position: at,
            kind,
        }
    }

    /// The pointer moved, at `time`, to the middle of the first cell of the
    /// row `spot` — wherever the window draws it.
    fn over(w: &mut VideoMetricsWindow, ui: &mut Ui, hand: &mut Hand<()>, spot: &str, time: u32) {
        let rows = w.view_rows(ui);
        let n = rows
            .iter()
            .position(|r| r.spot == spot)
            .unwrap_or_else(|| panic!("no row `{spot}`"));
        over_at(w, ui, hand, (n, 2), time);
    }

    /// The pointer moved, at `time`, to the middle of cell `at.1` (a field's
    /// `at`) of line `at.0`.
    fn over_at(
        w: &mut VideoMetricsWindow,
        ui: &mut Ui,
        hand: &mut Hand<()>,
        at: (usize, u16),
        time: u32,
    ) {
        let px = ronoctua_tokens::text::size();
        let line = usize::from(ui.first_of_list()) + at.0 - ui.offset;
        #[allow(clippy::cast_precision_loss)]
        let at = (
            (f64::from(ui.margin + at.1) + 0.5)
                * f64::from(ronoctua_tokens::metrics::cell_width(px)),
            (line as f64 + 0.5) * f64::from(ronoctua_tokens::metrics::line_height(px)),
        );
        let e = event(ui, at, PointerEventKind::Motion { time });
        w.pointer(ui, hand, &[e]);
    }

    /// FAILS IF: the window keeps no foot; the foot does not explain the
    /// thing the pointer rests on — its name, and that a first click
    /// selects it — over the cursor's; a key does not give it back to the
    /// cursor's thing, which `RET` does, until the pointer moves again; the
    /// pointer going down into the foot makes it explain the last row it
    /// crossed rather than the one it rested on; or the pointer gone leaves
    /// anything but the cursor's thing.
    #[test]
    fn the_foot_explains_the_pointer_then_the_cursor() {
        use crate::door::Gesture;
        let blocked = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-window-foot-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&blocked);
        std::fs::write(&blocked, b"").expect("a file where the catalogue's folder would be");
        let (mut w, mut ui) = a_window(&blocked, &Page::Home, Layout::List);
        let _ = std::fs::remove_file(&blocked);
        let mut hand = Hand::<()>::default();
        let t = ui.texts.clone();
        let gesture = |how, does| Some(crate::door::foot::gesture(&t, how, &t.t(does)));
        assert_eq!(w.foot_lines(&ui), ronoctua_tokens::space::WINDOW_FOOTLINES);
        // THE CURSOR'S THING, with nothing under the pointer.
        let rows = w.view_rows(&ui);
        let cursor = rows[w.view.drawn_cursor(&rows)].spot.clone();
        let said = w.explained(&ui).expect("the cursor's thing");
        assert_eq!(said.subject, cursor);
        // THE POINTER ON ANOTHER ROW: its thing, which a click selects.
        over(&mut w, &mut ui, &mut hand, "channels", 0);
        let said = w.explained(&ui).expect("the pointer's thing");
        assert_eq!(said.subject, "channels");
        assert_eq!(said.name, t.t(crate::key::VIDEO_METRICS_CHANNELS));
        assert_eq!(
            said.gesture,
            gesture(Gesture::Selects, crate::key::VIDEO_METRICS_ACT_OPENS)
        );
        assert!(!said.said.is_empty(), "the row says nothing");
        // A KEY gives the foot back to the cursor's thing, `RET`'s.
        let (down, mods) =
            crate::door::keys::event_of(ronoctua_keymap::Key::parse("<down>").expect("a key"));
        w.typed(&mut ui, &down, mods);
        let rows = w.view_rows(&ui);
        let cursor = rows[w.view.drawn_cursor(&rows)].spot.clone();
        assert_ne!(cursor, "channels");
        let said = w.explained(&ui).expect("the cursor's thing");
        assert_eq!(said.subject, cursor, "a key did not give the foot back");
        assert!(
            said.gesture
                .as_deref()
                .is_none_or(|g| g.contains(&crate::chrome::key_text(
                    &ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Enter)
                ))),
            "the cursor's thing does not say its key: {said:?}"
        );
        // …until the pointer moves again.
        over(&mut w, &mut ui, &mut hand, "channels", 10);
        assert_eq!(w.explained(&ui).expect("the pointer's").subject, "channels");
        // RESTED ON IT, then two rows crossed on the way down to the foot:
        // the foot explains what was rested on.
        let rest_ms = u32::from(ronoctua_tokens::space::WINDOW_FOOTRESTTENTHS) * 100;
        over(&mut w, &mut ui, &mut hand, "rules", 10 + rest_ms);
        over(&mut w, &mut ui, &mut hand, "weights", 20 + rest_ms);
        w.pointer_went_to_the_foot(&mut ui, &mut hand, Some(30 + rest_ms));
        assert_eq!(w.explained(&ui).expect("the rested").subject, "channels");
        // AND THE POINTER GONE: the cursor's thing again.
        let e = event(&ui, (0.0, 0.0), PointerEventKind::Leave { serial: 0 });
        w.pointer(&mut ui, &mut hand, &[e]);
        assert_eq!(w.explained(&ui).expect("the cursor's").subject, cursor);
    }

    /// FAILS IF: the foot, with the pointer on a video's title or on its
    /// channel, does not name the thing by that name WHOLE — a tile cuts
    /// both (the owner, 2026-11-11), and the window's foot is where this
    /// window says what is under the pointer, never the minibuffer — or the
    /// channel's name stays once the pointer went on to the picture. In the
    /// list and in the grid that says the channel.
    #[test]
    fn the_foot_names_a_title_and_a_channel_whole() {
        use crate::video_metrics::tests::{LONG_CHANNEL, LONG_TITLE, long_names};
        for layout in [Layout::List, Layout::GridWithChannel] {
            let blocked = std::env::temp_dir().join(format!(
                "ronoctua-video-metrics-window-names-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&blocked);
            std::fs::write(&blocked, b"").expect("a file where the catalogue's folder would be");
            let (mut w, mut ui) = a_window(&blocked, &Page::Videos, layout);
            let _ = std::fs::remove_file(&blocked);
            long_names(&mut w.readings);
            let mut hand = Hand::<()>::default();
            let rows = w.view_rows(&ui);
            let at = |starts: &dyn Fn(&str) -> bool| {
                rows.iter()
                    .enumerate()
                    .find_map(|(n, r)| r.fields.iter().find(|f| starts(&f.text)).map(|f| (n, f.at)))
                    .expect("a name drawn")
            };
            let title = at(&|t| t.starts_with("A video whose"));
            let channel = at(&|t| t.starts_with("A channel"));
            over_at(&mut w, &mut ui, &mut hand, title, 0);
            assert_eq!(w.explained(&ui).expect("the title").name, LONG_TITLE);
            over_at(&mut w, &mut ui, &mut hand, channel, 10);
            assert_eq!(
                w.explained(&ui).expect("the channel").name,
                LONG_CHANNEL,
                "{layout:?}"
            );
            // ON THE GRID'S PICTURE, the video again — by its title.
            if layout.grid() {
                let shelf = rows
                    .iter()
                    .position(|r| !r.cells.is_empty())
                    .expect("a shelf");
                over_at(
                    &mut w,
                    &mut ui,
                    &mut hand,
                    (shelf, rows[shelf].cells[0].from + 1),
                    20,
                );
                assert_eq!(w.explained(&ui).expect("the video").name, LONG_TITLE);
            }
        }
    }

    /// FAILS IF: the rest the foot keeps is the LINE's and not the thing's,
    /// on the grid, where a line holds several videos and a video spans its
    /// shelf's lines: a pointer that rested on one video, went down its
    /// lines and slid along one onto the next before dropping into the foot
    /// has the foot explain the video it only crossed — or none, the move
    /// down the video taken for a move off it.
    #[test]
    fn the_rest_is_the_videos_and_not_its_lines() {
        let blocked = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-window-rest-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&blocked);
        std::fs::write(&blocked, b"").expect("a file where the catalogue's folder would be");
        let (mut w, mut ui) = a_window(&blocked, &Page::Videos, Layout::Grid);
        let _ = std::fs::remove_file(&blocked);
        let mut hand = Hand::<()>::default();
        let rows = w.view_rows(&ui);
        let shelf = rows
            .iter()
            .position(|r| r.cells.len() > 2)
            .expect("a shelf of three videos");
        assert!(Row::lines_of(&rows, shelf) > 1, "a shelf of one line");
        // TWO VIDEOS SIDE BY SIDE, neither the cursor's: the cursor's thing
        // is what the foot says when it keeps no rest.
        let cursor = w.explained(&ui).expect("the cursor's thing").subject;
        let mut others = rows[shelf].cells.iter().filter(|c| c.spot != cursor);
        let (rested, crossed) = (
            others.next().expect("a video").clone(),
            others.next().expect("another").clone(),
        );
        let rest_ms = u32::from(ronoctua_tokens::space::WINDOW_FOOTRESTTENTHS) * 100;
        // ON ONE VIDEO, down its lines, then along the second line onto the
        // next — the rest is the first video's — and into the foot.
        over_at(&mut w, &mut ui, &mut hand, (shelf, rested.from + 1), 1000);
        assert_eq!(w.explained(&ui).expect("the video").subject, rested.spot);
        over_at(
            &mut w,
            &mut ui,
            &mut hand,
            (shelf + 1, rested.from + 1),
            1000 + rest_ms / 2,
        );
        over_at(
            &mut w,
            &mut ui,
            &mut hand,
            (shelf + 1, crossed.from + 1),
            1000 + rest_ms + 10,
        );
        assert_eq!(w.explained(&ui).expect("the next").subject, crossed.spot);
        w.pointer_went_to_the_foot(&mut ui, &mut hand, Some(1000 + rest_ms + 20));
        assert_eq!(
            w.explained(&ui).expect("the rested").subject,
            rested.spot,
            "the foot does not explain the video rested on"
        );
    }

    /// FAILS IF: a video of the list lights one way from its title and
    /// another from the words under it — channel, age, score —, which a
    /// click takes as the same video; or a band crosses the glyphs beside
    /// those words, which act on their own; or a glyph lights more than its
    /// own box, or the air between two lights anything. And on the grid, a
    /// glyph of a tile lights itself alone, a tile's cell its cell.
    #[test]
    fn a_video_lights_alike_from_its_title_and_its_words() {
        let r = crate::video_metrics::tests::readings();
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let (left, _) = crate::video_metrics::extent(COLUMNS);
        let (v, _) = View::fresh(Page::Videos);
        let rows = v.rows(&r, COLUMNS, &texts);
        let meta = rows
            .iter()
            .position(|row| row.continues && !row.controls.is_empty())
            .expect("a video's line of words and glyphs");
        let title = Row::owner(&rows, meta);
        assert!(title < meta, "the line of glyphs continues no title");
        let (from, wide) = rows[meta]
            .words_box(left, rows[title].has_words())
            .expect("the words beside the glyphs");
        let from_the_title = lights_at(&rows, title, left + 1, left);
        let from_the_words = lights_at(&rows, meta, from + wide - 1, left);
        assert_eq!(
            from_the_title, from_the_words,
            "the title and the words under it light different things"
        );
        assert!(
            !from_the_title.bands.contains(&meta),
            "a band crosses the glyphs: {from_the_title:?}"
        );
        assert_eq!(from_the_title.boxes, vec![(meta, from, wide)]);
        assert!(from_the_title.bands.contains(&title));
        for (i, c) in rows[meta].controls.iter().enumerate() {
            assert_eq!(
                lights_at(&rows, meta, c.from, left),
                Lights {
                    boxes: vec![(meta, c.from, c.width)],
                    ..Lights::default()
                },
                "the glyph {i} lights more than itself"
            );
            if rows[meta].controls.get(i + 1).is_some() {
                assert_eq!(
                    lights_at(&rows, meta, c.from + c.width, left),
                    Lights::default(),
                    "the air after glyph {i} lights something"
                );
            }
        }
        // THE GRID: a tile's glyph, and its cell.
        let (mut v, _) = View::fresh(Page::Videos);
        v.laid_out(Layout::Grid);
        let rows = v.rows(&r, COLUMNS, &texts);
        let shelf = rows
            .iter()
            .position(|row| !row.cells.is_empty())
            .expect("a shelf");
        let glyphs = (shelf..rows.len())
            .find(|&k| Row::owner(&rows, k) == shelf && !rows[k].controls.is_empty())
            .expect("a line of a tile's glyphs");
        let c = &rows[glyphs].controls[0];
        assert_eq!(
            lights_at(&rows, glyphs, c.from, left),
            Lights {
                boxes: vec![(glyphs, c.from, c.width)],
                ..Lights::default()
            }
        );
        let tile = &rows[shelf].cells[0];
        assert_eq!(
            lights_at(&rows, shelf, tile.from + 1, left).cell,
            Some((shelf, Row::lines_of(&rows, shelf), tile.from, tile.width))
        );
        // THE LIST WITH THUMBNAILS: from its title's words the video lights
        // from where its words begin on every line, a box and never a band
        // over the picture; the line of glyphs, its words' box from there;
        // and the pointer over the picture, on any line of it, lights nothing
        // — its click takes nothing there.
        let (mut v, _) = View::fresh(Page::Videos);
        v.laid_out(Layout::Strip);
        let rows = v.rows(&r, COLUMNS, &texts);
        let title = rows
            .iter()
            .position(|row| row.words_at.is_some())
            .expect("a video beside its picture");
        let at = rows[title].words_at.unwrap_or_default();
        let lines = Row::lines_of(&rows, title);
        assert!(lines > 1, "a video of one line");
        let over_the_words = lights_at(&rows, title, at + 1, left);
        for line in title..title + lines {
            for column in [left, left + 1, at - 1] {
                assert_eq!(
                    lights_at(&rows, line, column, left),
                    Lights::default(),
                    "line {line} lights at {column}, over the picture"
                );
            }
        }
        assert!(
            over_the_words.bands.is_empty(),
            "a band over the picture: {over_the_words:?}"
        );
        assert_eq!(over_the_words.boxes.len(), lines, "{over_the_words:?}");
        assert!(
            over_the_words.boxes.iter().all(|&(_, from, _)| from == at),
            "a box off the words: {over_the_words:?}"
        );
        let meta = (title..title + lines)
            .find(|&k| !rows[k].controls.is_empty())
            .expect("the line of glyphs");
        assert_eq!(
            rows[meta]
                .words_box(left, rows[title].has_words())
                .map(|b| b.0),
            Some(at)
        );
    }

    /// FAILS IF: what leaves the view for another program is not carried
    /// out by the window as the footer's region carries it out — Nyxt
    /// started here, by its entry in the launcher; the brain's page and the
    /// player's window asked of the footer, by the commands that open them —
    /// or a photograph's window (`Direct`) reaches any of them.
    #[test]
    fn a_window_sends_what_leaves_the_view_where_it_goes() {
        use crate::player_seam::ranking::OPEN_SET_ASIDE;
        let url = "https://www.youtube.com/watch?v=video000001".to_owned();
        let leaving = || {
            [
                Errand::Nyxt(url.clone()),
                Errand::Brain,
                Errand::Player(OPEN_SET_ASIDE),
            ]
        };
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        let (view, _) = View::fresh(Page::Home);
        let mut w = VideoMetricsWindow {
            view,
            readings: crate::video_metrics::tests::readings(),
            reach: Reach::Feed(Feed::start(None, || {})),
            pointer_x: None,
            pointed: None,
            keyed: false,
            since_ms: 0,
            rested: None,
            in_the_foot: false,
            cursor_row: std::cell::Cell::new(None),
        };
        for errand in leaving() {
            w.apply(&mut ui, errand);
        }
        assert_eq!(ui.launches(), [crate::launch::in_nyxt(&url)]);
        assert_eq!(
            ui.orders(),
            [
                format!(
                    "command {} {}",
                    crate::video_metrics::BRAIN_OPEN,
                    ronoctua_brain::compiled::video_metrics::ID
                ),
                format!(
                    "command {} {OPEN_SET_ASIDE}",
                    crate::video_metrics::PLAYER_WINDOW
                ),
            ]
        );
        assert!(!ui.quit, "the window closed for another program");
        // A PHOTOGRAPH'S WINDOW prints what it would have done, and does none.
        let blocked = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-leaving-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&blocked);
        std::fs::write(&blocked, b"").expect("a file where the catalogue's folder would be");
        let (mut w, mut ui) = a_window(&blocked, &Page::Home, Layout::List);
        for errand in leaving() {
            w.apply(&mut ui, errand);
        }
        let _ = std::fs::remove_file(&blocked);
        assert!(ui.launches().is_empty() && ui.orders().is_empty());
    }

    /// FAILS IF: the Videos page's hint stops offering its page keys after
    /// `LEADER Alt` as `[` and `]` — the quick keys taught before `PgUp` and
    /// `PgDn`, which stay direct and are offered nothing.
    #[test]
    fn the_page_keys_are_brackets_after_leader_alt() {
        let blocked = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-brackets-{}",
            std::process::id()
        ));
        let (w, ui) = a_window(&blocked, &Page::Videos, Layout::List);
        let offered = crate::chrome::bindings_of_hint(&w.hint(&ui), w.leader_alt_keys());
        let key_of = |id: &str| {
            offered
                .iter()
                .find(|b| b.command == id)
                .map(|b| b.keys.clone())
        };
        assert_eq!(key_of("video-metrics-page-back").as_deref(), Some("["));
        assert_eq!(key_of("video-metrics-page-on").as_deref(), Some("]"));
        assert_eq!(key_of("video-metrics-page-first"), None, "Home is direct");
        assert_eq!(key_of("video-metrics-page-last"), None, "End is direct");
    }

    /// FAILS IF: a page turned in the window — by `]`, by `PgDn`, by the
    /// arrow at the list's end — does not put the list's title first on the
    /// window, its first video under it and the cursor there: the follow
    /// that ends the gesture undid it, or the window stayed where it was.
    #[test]
    fn a_turned_page_shows_its_title_first_in_the_window() {
        use ronoctua_keymap::{Key, Sym};
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-turned-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let catalogue = dir.join(ronoctua_video_metrics::FILE);
        crate::video_metrics_feed::holding(&catalogue, 80);
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        ui.columns = COLUMNS;
        let window = VideoMetricsWindow::detached(catalogue, Page::Videos, String::new());
        let mut d = crate::door::Detached::new(ui, window);
        assert!(d.ui.room_for_rows() > 6, "a window too short to tell");
        for (key, page) in [
            (Key::simple(Sym::Char(']')), 1),
            (Key::simple(Sym::PageDown), 2),
            (Key::simple(Sym::Char('[')), 1),
        ] {
            d.press(key);
            assert_eq!(d.content.view.look().query.map(|q| q.page), Some(page));
            let rows = d.content.view_rows(&d.ui);
            let title = rows
                .iter()
                .position(|r| r.spot == crate::paging::LIST_TITLE)
                .expect("the list's title");
            assert_eq!(d.ui.offset, title, "{key:?}: the title is not first");
            assert_eq!(d.content.cursor(), Some(title + 1), "{key:?}");
        }
        // THE ARROW AT THE LIST'S END, clicked.
        let rows = d.content.view_rows(&d.ui);
        let end = rows
            .iter()
            .position(|r| r.spot == crate::paging::LIST_PAGER)
            .expect("the arrows at the list's end");
        let later = rows[end].controls.last().expect("its `›`").from;
        d.content.pointer_x =
            Some(f64::from(later + d.ui.margin) * f64::from(crate::chrome::cell_width()) + 1.0);
        d.content.clicked(&mut d.ui, end);
        assert_eq!(d.content.view.look().query.map(|q| q.page), Some(2));
        let rows = d.content.view_rows(&d.ui);
        let title = rows
            .iter()
            .position(|r| r.spot == crate::paging::LIST_TITLE)
            .expect("the list's title");
        assert_eq!(
            d.ui.offset, title,
            "the end's arrow did not bring the title"
        );
        drop(d);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a page turned in the window to a SHORT last page — the
    /// owner's 27 videos, two on the second page — does not put the list's
    /// title first (the window pages only as far as the list's end, and the
    /// filters above stayed in sight); or, with the title first, the next
    /// gesture that leaves the cursor under it — `]` on the last page, `↓`,
    /// `↑` back to the first video, a click that only chooses a video —
    /// scrolls it back down; or a key that takes the cursor above it is not
    /// followed as always.
    #[test]
    fn a_turned_short_page_keeps_its_title_first_in_the_window() {
        use ronoctua_keymap::{Key, Sym};
        use smithay_client_toolkit::seat::pointer::PointerEventKind;
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-short-page-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let catalogue = dir.join(ronoctua_video_metrics::FILE);
        // A SHORT LAST PAGE: two videos past the first.
        crate::video_metrics_feed::holding(&catalogue, crate::paging::PerPage::default().get() + 2);
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        ui.columns = COLUMNS;
        let window = VideoMetricsWindow::detached(catalogue, Page::Videos, String::new());
        let mut d = crate::door::Detached::new(ui, window);
        let line_of = |d: &crate::door::Detached<VideoMetricsWindow>, spot: &str| {
            d.content
                .view_rows(&d.ui)
                .iter()
                .position(|r| r.spot == spot)
                .unwrap_or_else(|| panic!("no row `{spot}`"))
        };
        d.press(Key::simple(Sym::Char(']')));
        assert_eq!(d.content.view.look().query.map(|q| q.page), Some(1));
        let title = line_of(&d, crate::paging::LIST_TITLE);
        assert!(
            line_of(&d, crate::paging::LIST_PAGER) < title + d.ui.room_for_rows(),
            "a page too long to tell"
        );
        assert_eq!(d.ui.offset, title, "the short page's title is not first");
        for key in [Sym::Char(']'), Sym::Down, Sym::Up] {
            d.press(Key::simple(key));
            assert_eq!(d.ui.offset, title, "{key:?} scrolled the title back");
        }
        // A CLICK THAT ONLY CHOOSES the second video.
        let second = title + 3;
        let px = ronoctua_tokens::text::size();
        #[allow(clippy::cast_precision_loss)]
        let at = (
            (f64::from(d.ui.margin + 4) + 0.5)
                * f64::from(ronoctua_tokens::metrics::cell_width(px)),
            ((usize::from(d.ui.first_of_list()) + second - d.ui.offset) as f64 + 0.5)
                * f64::from(ronoctua_tokens::metrics::line_height(px)),
        );
        let click = [
            PointerEventKind::Press {
                time: 0,
                button: crate::door::LEFT_BUTTON,
                serial: 0,
            },
            PointerEventKind::Release {
                time: 1,
                button: crate::door::LEFT_BUTTON,
                serial: 0,
            },
        ]
        .map(|kind| event(&d.ui, at, kind));
        d.pointer(&click);
        let rows = d.content.view_rows(&d.ui);
        assert_eq!(
            d.content.view.cursor(&rows),
            second,
            "the click did not choose the second video"
        );
        assert_eq!(
            d.ui.offset, title,
            "a click that chose scrolled the title back"
        );
        // ABOVE THE TITLE, the cursor is followed as always.
        for _ in 0..2 {
            d.press(Key::simple(Sym::Up));
        }
        assert!(
            d.ui.offset < title,
            "the row above the title is out of sight"
        );
        drop(d);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a key the window teaches has no one key after `LEADER Alt`,
    /// or lands on one another of its commands already has. Here, and not in
    /// the binary's list of every window (`leader_alt.rs`), because this
    /// view's name is its brain module's, which the binary may not say
    /// (`brain-modules-are-sealed`).
    #[test]
    fn every_shortcut_is_one_key_after_leader_alt() {
        let view: Vec<(&str, &str)> = crate::video_metrics::OF_THE_VIEW
            .iter()
            .map(|r| (r.0, r.1))
            .collect();
        if let Err(wrong) = crate::chrome::leader_alt_census(
            &[crate::chrome::OF_THE_WAY_BACK, &view],
            &[crate::video_metrics::LEADER_ALT_KEYS],
        ) {
            panic!("the window offers what is not one key after LEADER Alt:\n{wrong}");
        }
    }

    /// FAILS IF: the window does not host the file picker over its import
    /// page as the footer's region does — the second click on choosing the
    /// list brings it up; the window keeps none of the letters it looks for
    /// from it; going back puts the page back; a list pasted into it and
    /// taken is imported, said on this window's line and its channels drawn
    /// to decide.
    #[test]
    fn the_window_picks_a_list_and_imports_it() {
        use ronoctua_keymap::{Key, Sym};
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-video-metrics-window-import-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        // TWO CHANNELS, made up (invariant 13).
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
                line('p', "One"),
                line('q', "Two")
            ),
        )
        .expect("a list");
        let mut ui = Ui::for_tests(ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB"));
        ui.columns = COLUMNS;
        let mut w =
            VideoMetricsWindow::detached(dir.join("catalog.sqlite"), Page::Import, String::new());
        let key = |w: &mut VideoMetricsWindow, ui: &mut Ui, k: Key| {
            let (event, mods) = crate::door::keys::event_of(k);
            w.typed(ui, &event, mods);
        };
        let choose = |w: &mut VideoMetricsWindow, ui: &mut Ui| {
            let rows = w.view_rows(ui);
            let n = rows
                .iter()
                .position(|r| r.spot == "choose-list")
                .expect("the row that chooses the list");
            w.clicked(ui, n);
            w.clicked(ui, n);
        };
        choose(&mut w, &mut ui);
        assert!(w.view.picking(), "the second click brought no picker up");
        let letter = Key::simple(Sym::Char('n'));
        assert!(
            !crate::chrome::barred(&letter, &w.barred_keys(&ui)),
            "the window keeps a letter from the picker"
        );
        key(&mut w, &mut ui, Key::simple(Sym::Esc));
        assert!(!w.view.picking(), "the way back left the picker up");
        choose(&mut w, &mut ui);
        w.pasted(&mut ui, &list.to_string_lossy());
        key(&mut w, &mut ui, Key::simple(Sym::Enter));
        assert!(!w.view.picking(), "the picker stayed up after its answer");
        assert_eq!(
            w.readings.staged.len(),
            2,
            "the list was not staged and read"
        );
        let imported = ui.texts.t_args(
            crate::key::VIDEO_METRICS_IMPORTED,
            &[
                ("staged", ronoctua_i18n::Arg::Number(2)),
                ("already", ronoctua_i18n::Arg::Number(0)),
            ],
        );
        assert_eq!(
            ui.message.as_deref(),
            Some(imported.as_str()),
            "the import was not said"
        );
        let rows = w.view_rows(&ui);
        assert!(
            rows.iter()
                .filter(|r| r.spot.starts_with("staged:"))
                .count()
                == 2,
            "the channels to decide are not drawn"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
