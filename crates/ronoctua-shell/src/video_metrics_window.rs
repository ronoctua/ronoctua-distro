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
//! first. It opens a video in Chrome through the footer's `kiosk-open`
//! (`Ui::tell_the_footer`, off the loop), and what went wrong anywhere is
//! said on THIS window's line.

use anyhow::Result;
use smithay_client_toolkit::reexports::calloop::channel;
use smithay_client_toolkit::seat::keyboard::{KeyEvent, Modifiers};
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

use crate::door::{Canvas, Content, Door, Hand, Ui};
use crate::video_metrics::{Errand, Page, Readings, Row, Said, View};
use crate::video_metrics_feed::{Arrived, Direct, Feed, Want};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.video-metrics";

/// The words `ronoctua-shell video-metrics <word>` opens on — the command
/// line's, so English.
pub const OPENINGS: [&str; 5] = ["home", "videos", "channels", "rules", "weights"];

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
        let (trees, _) = crate::leader::default_of()?;
        let readings = readings_for(
            crate::leader::taught_keys(&trees, "transfers-open", None).unwrap_or_default(),
        );
        let sender = std::sync::Mutex::new(wake);
        let feed = Feed::start(ronoctua_video_metrics::default_path(), move || {
            if let Ok(s) = sender.lock() {
                let _ = s.send(());
            }
        });
        let (view, errand) = View::fresh(opening);
        let mut me = VideoMetricsWindow {
            view,
            readings,
            reach: Reach::Feed(feed),
            pointer_x: None,
            pointed: None,
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
    /// The row under the pointer, whose sentence line 0 says — what the
    /// footer says on its message line under the pointer.
    pointed: Option<usize>,
    /// The cursor's row when the rows were last built — STORED WHERE IT IS
    /// MEASURED, for the door, which asks without a width.
    cursor_row: std::cell::Cell<Option<usize>>,
}

impl VideoMetricsWindow {
    /// The window over the catalogue at `catalogue`, read and written on the
    /// spot — what the photograph draws, over a copy of a fixture.
    #[must_use]
    pub fn detached(catalogue: std::path::PathBuf, opening: Page, transfers_keys: String) -> Self {
        let (view, errand) = View::fresh(opening);
        let mut me = Self {
            view,
            readings: readings_for(transfers_keys),
            reach: Reach::Direct(Box::new(Direct::at(catalogue))),
            pointer_x: None,
            pointed: None,
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
            Reach::Direct(d) => {
                // WHAT A PHOTOGRAPH WOULD HAVE DONE past the catalogue is
                // printed, never done: its door reaches nothing.
                match &want {
                    Want::Scan => println!("would have asked the brain for a scan"),
                    Want::Play(p) => println!("asked the player: put {} ({})", p.url, p.title),
                    Want::Download { url, .. } => println!("would have downloaded {url}"),
                    Want::Edit(crate::video_metrics::Edit::AddChannel(input)) => {
                        println!("would have asked yt-dlp who {input} is, and added it");
                    }
                    Want::Edit(crate::video_metrics::Edit::Consent(on)) => {
                        println!("kept in memory, not in this machine's file: ask YouTube = {on}");
                    }
                    Want::Look(_) | Want::Edit(_) => {}
                }
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

    /// The thread brought something: take all of it.
    fn woke(&mut self, ui: &mut Ui) {
        let arrived = match &self.reach {
            Reach::Feed(f) => f.take(),
            Reach::Direct(_) => Vec::new(),
        };
        self.took(ui, arrived);
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

    /// The rows, at this window's width, in its language.
    fn view_rows(&self, ui: &Ui) -> Vec<Row> {
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
        let rows = self.view_rows(ui);
        let at = self.cursor_of(&rows);
        self.cursor_row.set(Some(at));
        let last = at + Row::lines_of(&rows, at) - 1;
        ui.follow(last, rows.len());
        ui.follow(at, rows.len());
    }

    /// A key of the view — from the keyboard, the hint, or `RET` from a
    /// second click (invariant 19).
    fn pressed(&mut self, ui: &mut Ui, press: crate::video_metrics::Press) {
        ui.message = None;
        let rows = self.view_rows(ui);
        let errand = self.view.press(press, &rows, &self.readings);
        self.apply(ui, errand);
        self.follow(ui);
        ui.ask_paint();
    }

    /// Carries out what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        match errand {
            Errand::Nothing => {}
            Errand::Close => ui.quit = true,
            Errand::Look(look) => self.want(Some(ui), Want::Look(look)),
            Errand::Edit(edit) => self.want(Some(ui), Want::Edit(edit)),
            // ASKED ON THIS WINDOW'S OWN LINE; the answer comes back in
            // `answered`, to the same `View::typed` the footer's reaches.
            Errand::Ask { ask, initial } => ui.ask(crate::line::Line::new(
                ask.command(),
                false,
                initial.as_deref(),
            )),
            Errand::Paste => ui.paste(),
            Errand::Copy(text) => {
                let said = crate::chrome::copied(&text, &ui.texts);
                ui.say(said);
            }
            Errand::Scan => self.want(Some(ui), Want::Scan),
            Errand::Play(play) => self.want(Some(ui), Want::Play(play)),
            Errand::Download { url, title } => {
                self.want(Some(ui), Want::Download { url, title });
            }
            // THE QUEUE IS THE FOOTER'S (`LEADER t t`): asked down its socket,
            // off the loop; a footer whose region holds other work says so
            // there, and one that is not there is said here.
            Errand::Transfers => {
                if matches!(self.reach, Reach::Direct(_)) {
                    println!("would have told the footer: transfers-open");
                } else {
                    ui.tell_the_footer("transfers-open", None);
                }
            }
            // THE KIOSK IS THE FOOTER'S: it holds the bridge and the tabs.
            // The order goes down its socket off the loop, and a footer that
            // is not there is said here (`door-no-footer`).
            Errand::Kiosk(url) => {
                if matches!(self.reach, Reach::Direct(_)) {
                    println!("would have told the footer: kiosk-open {url}");
                } else {
                    ui.tell_the_footer("kiosk-open", Some(&url));
                }
            }
            Errand::Say(text) => ui.say_for(text, crate::echo::Echo::Long),
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

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::VIDEO_METRICS_TITLE))
    }

    /// A key, read by its whole notation through the reader the footer calls
    /// too — or, while a text is edited, the text's.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        if self.view.editing() {
            let (_, room) = crate::video_metrics::extent(ui.columns);
            let errand = self.view.text_key(key, room);
            self.apply(ui, errand);
            self.follow(ui);
            ui.ask_paint();
            return;
        }
        if let Some(press) = crate::video_metrics::press_of_key(key) {
            self.pressed(ui, press);
        }
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

    /// The clipboard's text, read off the loop for `C-v` in the description.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        if self.view.text_pasted(text) {
            ui.ask_paint();
        }
    }

    /// CAPS LOCK MEANS CAPITALS while the second description is edited —
    /// and nowhere else here, where a letter is a key of the view (the
    /// footer's region answers the same, `region_types_capitals`).
    fn types_capitals(&self) -> bool {
        self.view.editing()
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
    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.view_rows(ui).into_iter().map(|r| r.fields).collect()
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
        if let Some(errand) = self.view.click_answers(row, &rows) {
            self.apply(ui, errand);
            ui.ask_paint();
            return;
        }
        if self.view.clicked(row, column, &rows) {
            self.pressed(ui, crate::video_metrics::Press::Enter);
        } else {
            ui.ask_paint();
        }
    }

    /// The pointer: where it is across the window is kept for the grid's
    /// cells, and the rest is the standard list's.
    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<Self::Hand>, events: &[PointerEvent]) {
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. }
                | PointerEventKind::Motion { .. }
                | PointerEventKind::Press { .. } => self.pointer_x = Some(e.position.0),
                PointerEventKind::Leave { .. } => self.pointer_x = None,
                _ => {}
            }
        }
        let before = self.pointer_x;
        crate::door::list::pointer(self, ui, hand, events);
        // THE ROW UNDER THE POINTER says its sentence on line 0.
        if self.pointed != hand.row_under_pointer {
            self.pointed = hand.row_under_pointer;
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
        // LINE 0: the question, a message, the pointed row's sentence — or
        // the keys (`list::line_zero`'s order, the footer's own).
        let about = self
            .pointed
            .and_then(|n| crate::video_metrics::View::about(&rows, n, &ui.texts));
        crate::door::list::header_about(ui, canvas, None, about, |ui| self.hint(ui));
        // FOLLOW THE CURSOR once the width is known: the door asked
        // `cursor()` without one.
        let fields: Vec<Vec<crate::files::Field>> = rows.iter().map(|r| r.fields.clone()).collect();
        body(ui, canvas, &rows, &fields, self.pointer_x);
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
/// shelf's CELL on the grid, and no band on a line of a picture — and the
/// list's bar, recorded as its own column.
fn body(
    ui: &mut Ui,
    canvas: &mut Canvas<'_>,
    rows: &[Row],
    fields: &[Vec<crate::files::Field>],
    pointer_x: Option<f64>,
) {
    let size = ronoctua_tokens::text::size();
    ui.settle_offset(rows.len());
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
    let scroll = crate::chrome::list_scroll(rows.len(), room, ui.offset);
    #[allow(clippy::cast_precision_loss)]
    let (w, h) = (ui.width as f32, ui.height as f32);
    ui.room = crate::chrome::Room::of(scroll, w, h);
    let px = ronoctua_tokens::text::size();
    let (cell, tall) = (
        ronoctua_tokens::metrics::cell_width(px),
        ronoctua_tokens::metrics::line_height(px),
    );
    // EVERY POINTER'S HOVER, in its own colour. A line of a picture is a
    // figure and lights nothing; a line of a shelf lights the CELL under the
    // pointer, over the shelf's lines — a band across it would light seven
    // videos to say the pointer is on one.
    let mut plain: Vec<(ronoctua_tokens::Color, Option<usize>)> = Vec::new();
    for &(colour, at) in canvas.lit {
        let Some(n) = at.filter(|n| *n < rows.len()) else {
            continue;
        };
        let owner = Row::owner(rows, n);
        let Some(row) = rows.get(owner) else {
            continue;
        };
        if !row.cells.is_empty() {
            let column = pointer_x.map_or(0, |x| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let c = (x / f64::from(cell)).floor().max(0.0) as u16;
                c.saturating_sub(ui.margin)
            });
            if let Some(c) = row
                .cells
                .iter()
                .find(|c| crate::chrome::in_cell(column, c.from, c.width))
            {
                let lines = Row::lines_of(rows, owner);
                let top = owner.max(ui.offset);
                let bottom = (owner + lines).min(ui.offset + room);
                if bottom > top {
                    let y = f32::from(first) * tall + (top - ui.offset) as f32 * tall;
                    let x = f32::from(ui.margin + c.from) * cell;
                    let wide = (f32::from(c.width) * cell).min(ui.room.band_width() - x);
                    canvas.under.push(ronoctua_render::quad::Quad {
                        x,
                        y,
                        width: wide.max(0.0),
                        height: (bottom - top) as f32 * tall,
                        color: colour,
                    });
                }
            }
            continue;
        }
        if !rows[n].thumbs.is_empty() || rows[n].heading {
            continue;
        }
        plain.push((colour, Some(n)));
    }
    canvas.under.extend(crate::chrome::hover_bands(
        plain.into_iter(),
        ui.offset,
        room,
        rows.len(),
        first,
        ui.room.band_width(),
    ));
    canvas
        .under
        .extend(crate::chrome::list_bar(scroll, first, w, h));
    canvas.bar(crate::chrome::list_track(scroll, first, w, h));
}
