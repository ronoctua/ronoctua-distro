//! The Incus view's SECOND door — a window of its own (`LEADER s x w`,
//! `ronoctua-shell incus`).
//!
//! # One implementation, two doors
//!
//! Everything the view knows is `crate::incus::View`; the footer draws the
//! same lines. What this holds is a window and the view open in it
//! (`crate::incus_open::Open`, whose `crate::incus_feed::Feed` lives exactly
//! as long as the window — the daemon's events included), the queue's list
//! for the jobs the view gave, and the pointer's part in three columns side
//! by side: which column a cell is in, a wheel and a bar per column, and a
//! hover that lights the column under the pointer and never the whole line
//! (invariant 15). What the view asks for is carried out by
//! `crate::incus_open::carry_out`, the function the footer's region calls
//! too.
//!
//! # What it does, and where
//!
//! A deed (start, stop, a snapshot, a limit) on the feed's thread; a launch,
//! an image copied, an export, an import, a file sent or taken and a
//! publication on the queue (`Work::Incus`); a text console and a shell in
//! OUR terminal (`ronoctua-shell terminal <incus> …`), the screen of a VM in
//! remote-viewer, which the CLI opens. Every value of the draft, a path, a
//! snapshot's name is typed on this window's own line (`Ui::ask`).

use std::collections::BTreeMap;

use anyhow::Result;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::channel,
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

use crate::door::{Canvas, Content, Door, Hand, Ui};
use crate::incus_feed::{Feed, Scene};
use crate::incus_open::{Open, carry_out};
use crate::{
    incus::{Errand, View},
    places::{Layout, Pane, layout_of},
};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.incus";

/// The glyph before the window's name, as text — the `Title`'s shape.
static GLYPH: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| tokens::icon::INCUS.to_string());

/// What the window opens on: the instances, the images, or the draft of a
/// new instance — the three leader keys that reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Opening {
    /// The instances.
    #[default]
    Instances,
    /// The images.
    Images,
    /// The draft of a new instance.
    New,
}

impl Opening {
    /// The opening a command line's words say.
    ///
    /// # Errors
    ///
    /// A word that is not one, or a word too many.
    pub fn of_words(words: &[String]) -> Result<Self> {
        match words {
            [] => Ok(Self::Instances),
            [w] if w == "images" => Ok(Self::Images),
            [w] if w == "new" => Ok(Self::New),
            other => anyhow::bail!(
                "usage: ronoctua-shell incus [images|new] — `{}` is not one of them",
                other.join(" ")
            ),
        }
    }
}

/// Opens the Incus view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it(opening: Opening) -> Result<()> {
    crate::door::run(|w| {
        // ONE PING FOR EVERYTHING THE THREADS BRING.
        let (wake, woke) = channel::channel::<()>();
        w.handle
            .insert_source(woke, |event, (), door: &mut Door<IncusWindow>| {
                if let channel::Event::Msg(()) = event {
                    door.content.take_now(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang Incus's channel: {e}"))?;
        // THE QUEUE'S LIST: a launch, an import or a copy that ended changed
        // what there is.
        let (jobs_sender, jobs) = channel::channel::<Vec<ronoctua_transfers::Job>>();
        crate::queue::listen(jobs_sender);
        w.handle
            .insert_source(jobs, |event, (), door: &mut Door<IncusWindow>| {
                if let channel::Event::Msg(jobs) = event {
                    door.content.jobs_arrived(&mut door.ui, jobs);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the queue channel: {e}"))?;
        let sender = std::sync::Mutex::new(wake);
        let mut feed = Feed::new(move || {
            if let Ok(s) = sender.lock() {
                let _ = s.send(());
            }
        });
        feed.start();
        Ok(IncusWindow::with(feed, opening))
    })
}

/// A rectangle in logical pixels: `(x, y, width, height)`.
type Rect = (f32, f32, f32, f32);

/// One scrolling column's bar, placed: its column, its track, its thumb when
/// there is one, and the scroll it stands for.
type Bar = (Pane, Rect, Option<Rect>, ronoctua_design::Scroll);

/// What one pointer keeps of its own over this window.
#[derive(Debug, Default)]
pub struct Pointed {
    /// Which pointer this is, given on its first frame — `0` is "not yet".
    number: u32,
    /// The cell along the line under it: three columns share a line.
    cell: Option<u16>,
    /// The column whose bar this pointer holds.
    dragging: Option<Pane>,
    /// Lines of wheel asked for and not yet spent over a column.
    owed: f64,
}

/// The window.
pub struct IncusWindow {
    /// The view, open in this window.
    open: Open,
    /// The cell the last press was on — what `clicked` reads.
    click_cell: Option<u16>,
    /// Every pointer's line and cell, by its number — for the hover bands.
    pointers: BTreeMap<u32, (Option<usize>, Option<u16>)>,
    hands: u32,
    /// The drawn line the keyboard's cursor is on, stacked.
    cursor_at: Option<usize>,
    /// What it opens on, once the first listing is here.
    opening: Option<Opening>,
}

impl IncusWindow {
    fn with(feed: Feed, opening: Opening) -> Self {
        // THE KEYS ITS SENTENCES TEACH, read from the leader's own trees: this
        // window is a process of its own, and asks the tables the footer walks.
        let queue_keys = crate::incus_words::queue_keys();
        let zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
        let view = match opening {
            Opening::Images => View::on(crate::incus::Kind::Images),
            Opening::Instances | Opening::New => View::fresh(),
        };
        Self {
            open: Open::new(view, feed, zone, queue_keys),
            click_cell: None,
            pointers: BTreeMap::new(),
            hands: 0,
            cursor_at: None,
            opening: (opening == Opening::New).then_some(opening),
        }
    }

    /// A window over a scene, for a photograph: the daemon's answers handed
    /// in whole, every date in UTC, and nothing of this machine's touched.
    #[must_use]
    pub fn detached(scene: Scene, opening: Opening) -> Self {
        let mut feed = Feed::detached(Some(scene));
        feed.start();
        let mut me = Self::with(feed, opening);
        me.open.readings.zone = ronoctua_files::when::Zone::default();
        me
    }

    /// Takes everything the feed brought — a photograph calls it after every
    /// step, as the loop does on the bell. `true` when something arrived.
    pub fn take_now(&mut self, ui: &mut Ui) -> bool {
        if !self.open.take(ui) {
            return false;
        }
        self.settled(ui, false);
        true
    }

    /// What a photograph reports beside its picture: whether the daemon's
    /// events are followed, what would have gone to the queue, and the
    /// instances whose state the clock would read again.
    #[must_use]
    pub fn report(&self) -> (bool, &[ronoctua_transfers::Work], Vec<String>) {
        (
            self.open.feed.following_events(),
            self.open.feed.queued(),
            self.open.feed.rereading(),
        )
    }

    fn layout(ui: &Ui) -> Layout {
        layout_of(ui.columns, false)
    }

    fn lines(&self, ui: &Ui) -> Vec<crate::incus::Line> {
        self.open
            .lines(ui.columns, Self::layout(ui), ui.room_for_rows(), &ui.texts)
    }

    /// The queue's list: Incus's jobs, and a listing again when one of them
    /// ended.
    fn jobs_arrived(&mut self, ui: &mut Ui, jobs: Vec<ronoctua_transfers::Job>) {
        if self.open.jobs_arrived(&jobs) {
            self.settled(ui, false);
        }
    }

    /// The cursors put back in step with what is drawn, what is read made
    /// what is shown, and a frame. `person`: after a key, a click, an
    /// answer or a resize of the person's — only then do the panes follow
    /// their cursors: data that arrived never moves a column the person
    /// scrolled with the wheel or the bar.
    fn settled(&mut self, ui: &mut Ui, person: bool) {
        self.open.settle(Self::layout(ui), ui);
        self.after(ui, person);
    }

    /// What this window does once the view settled: the draft asked for on
    /// the command line, the panes kept on their cursors after a person's
    /// gesture, and the cursor's line counted for the door's page.
    fn after(&mut self, ui: &mut Ui, person: bool) {
        let layout = Self::layout(ui);
        // THE DRAFT ASKED FOR ON THE COMMAND LINE, once there is a listing
        // to check a name against — looked at AFTER the rounds of the
        // settling, which may be the ones that took the listing: a resize can
        // settle before the bell rings, and the bell then finds nothing new.
        if self.opening == Some(Opening::New) && self.open.readings.listing.is_some() {
            self.opening = None;
            let errand = self.open.view.key(
                crate::incus::Press::Launch,
                &self.open.readings,
                ui.columns,
                layout,
                &ui.texts,
            );
            self.apply(ui, errand);
            return;
        }
        if person {
            self.open.view.keep_in_sight(
                &self.open.readings,
                ui.columns,
                layout,
                ui.room_for_rows(),
                &ui.texts,
            );
        }
        self.cursor_at = match layout {
            Layout::Panes => None,
            Layout::Stacked => self
                .open
                .view
                .cursor_line(&self.lines(ui), &self.open.readings),
        };
        ui.ask_paint();
    }

    /// Does what the view asked for — through the one function the footer's
    /// region calls too (`incus_open::carry_out`), which settles the view
    /// after it.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        carry_out(errand, &mut self.open, Self::layout(ui), ui);
        if !ui.quit {
            self.after(ui, true);
        }
    }

    /// What one key means.
    fn pressed(&mut self, ui: &mut Ui, press: crate::incus::Press) {
        ui.message = None;
        let errand = self.open.view.key(
            press,
            &self.open.readings,
            ui.columns,
            Self::layout(ui),
            &ui.texts,
        );
        self.apply(ui, errand);
    }

    /// The cell a pixel is in, counted as the fields are.
    fn cell_at(ui: &Ui, x: f64) -> Option<u16> {
        let cell = f64::from(tokens::metrics::cell_width(tokens::text::size()));
        if cell <= 0.0 || x < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let which = (x / cell) as u16;
        which.checked_sub(ui.margin)
    }

    /// Each scrolling column's bar, placed — one function for the paint and
    /// the pointer, so a bar is taken where it is drawn.
    fn bars(&self, ui: &Ui) -> Vec<Bar> {
        if Self::layout(ui) != Layout::Panes {
            return Vec::new();
        }
        let first = ui.first_of_list();
        let f = ui.frame(first);
        let (cell, line) = (f.cell(), f.line());
        let room = ui.room_for_rows();
        self.open
            .view
            .scrolls(&self.open.readings, ui.columns, room, &ui.texts)
            .into_iter()
            .filter(crate::places::PaneScroll::scrolls)
            .map(|s| {
                let right = f32::from(ui.margin + s.column + 1) * cell;
                let top = f.y_of(first + u16::try_from(s.first).unwrap_or(0));
                #[allow(clippy::cast_precision_loss)]
                let bottom = (top + line * s.lines as f32).min(f.bottom());
                let (track, thumb) =
                    crate::chrome::band_bar(s.scroll(), right, top, bottom, cell, line);
                (s.pane, track, thumb, s.scroll())
            })
            .collect()
    }

    /// The pointer's part in the panes: a wheel over a column scrolls that
    /// column, and a column's bar is grabbed and dragged. `true` when it
    /// took the events.
    #[allow(clippy::cast_possible_truncation)]
    fn panes_pointer(
        &mut self,
        ui: &mut Ui,
        hand: &mut Hand<Pointed>,
        events: &[PointerEvent],
    ) -> bool {
        if Self::layout(ui) != Layout::Panes {
            hand.extra.dragging = None;
            return false;
        }
        let line = ui.frame(0).line();
        let mut took = false;
        for e in events {
            let (x, y) = (e.position.0 as f32, e.position.1 as f32);
            let bar_offset = |track: Rect, scroll: ronoctua_design::Scroll| {
                scroll.offset_at(y - track.1, track.3, line)
            };
            match e.kind {
                PointerEventKind::Press { button, .. } if button == crate::door::LEFT_BUTTON => {
                    if let Some((pane, track, _, scroll)) = self.bars(ui).into_iter().find(|b| {
                        let (tx, ty, tw, th) = b.1;
                        x >= tx && x < tx + tw && y >= ty && y < ty + th
                    }) {
                        hand.extra.dragging = Some(pane);
                        self.open.view.scroll_to(pane, bar_offset(track, scroll));
                        took = true;
                    }
                }
                PointerEventKind::Motion { .. } => {
                    if let Some(pane) = hand.extra.dragging
                        && let Some((_, track, _, scroll)) =
                            self.bars(ui).into_iter().find(|b| b.0 == pane)
                    {
                        self.open.view.scroll_to(pane, bar_offset(track, scroll));
                        took = true;
                    }
                }
                PointerEventKind::Release { .. } | PointerEventKind::Leave { .. } => {
                    took |= hand.extra.dragging.take().is_some();
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    // OVER THE HEADER, the header's wheel.
                    if ui.row_at(e.position.0, e.position.1).is_none() {
                        continue;
                    }
                    crate::wheel::wheel_arrived(&mut hand.extra.owed, &vertical, &horizontal);
                    let by = crate::wheel::wheel_lines(&mut hand.extra.owed);
                    let pane = crate::places::places(ui.columns)
                        .pane_at(Self::cell_at(ui, e.position.0).unwrap_or(0));
                    if by != 0
                        && let Some(s) = self
                            .open
                            .view
                            .scrolls(
                                &self.open.readings,
                                ui.columns,
                                ui.room_for_rows(),
                                &ui.texts,
                            )
                            .into_iter()
                            .find(|s| s.pane == pane)
                    {
                        let last = s.total.saturating_sub(s.lines);
                        let to = usize::try_from(
                            i64::try_from(s.offset)
                                .unwrap_or(0)
                                .saturating_add(by)
                                .max(0),
                        )
                        .unwrap_or(0)
                        .min(last);
                        self.open.view.scroll_to(pane, to);
                    }
                    took = true;
                }
                _ => {}
            }
        }
        if took {
            ui.ask_paint();
        }
        took
    }
}

impl Content for IncusWindow {
    type Hand = Pointed;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(crate::key::INCUS_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::incus::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let errand =
            self.open
                .view
                .answered(id, text, &self.open.readings, home.as_deref(), &ui.texts);
        self.apply(ui, errand);
    }

    /// A question let go: what it was for goes with it.
    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        let _ = (ui, id);
        self.open.view.let_go();
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.open
            .view
            .hint(&self.open.readings, ui.columns, Self::layout(ui), &ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::fields::Field>> {
        self.lines(ui).into_iter().map(|l| l.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        // THE PANES SCROLL INSIDE THEMSELVES; stacked, the door follows the
        // cursor's line — after a key or a click.
        self.cursor_at
    }

    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let lines = self.lines(ui);
        let layout = Self::layout(ui);
        let again =
            self.open
                .view
                .clicked(&lines, row, self.click_cell, layout, &self.open.readings);
        if again {
            self.pressed(ui, crate::incus::Press::Enter);
        } else {
            self.settled(ui, true);
        }
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<Pointed>, events: &[PointerEvent]) {
        if hand.extra.number == 0 {
            self.hands = self.hands.wrapping_add(1).max(1);
            hand.extra.number = self.hands;
        }
        let before = hand.extra.cell;
        for e in events {
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    hand.extra.cell = Self::cell_at(ui, e.position.0);
                }
                PointerEventKind::Press { .. } => {
                    hand.extra.cell = Self::cell_at(ui, e.position.0);
                    self.click_cell = hand.extra.cell;
                }
                PointerEventKind::Leave { .. } => hand.extra.cell = None,
                _ => {}
            }
        }
        if !self.panes_pointer(ui, hand, events) {
            crate::door::list::pointer(self, ui, hand, events);
        }
        self.pointers
            .insert(hand.extra.number, (hand.row_under_pointer, hand.extra.cell));
        // A NEW CELL CAN BE A NEW COLUMN: the band follows the column.
        if before != hand.extra.cell {
            ui.ask_paint();
        }
    }

    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let name = ui.texts.t(crate::key::INCUS_TITLE);
        let hint = self.hint(ui);
        crate::door::list::header_of(
            ui,
            canvas,
            Some(crate::chrome::Title {
                glyph: Some(GLYPH.as_str()),
                label: None,
                name: Some(&name),
            }),
            |_| hint,
        );
        let layout = Self::layout(ui);
        let lines = self.lines(ui);
        let rows: Vec<Vec<crate::fields::Field>> = lines.iter().map(|l| l.fields.clone()).collect();
        if layout == Layout::Panes {
            ui.offset = 0;
        } else {
            // A PAINT ONLY KEEPS THE PAGE INSIDE A LIST THAT CHANGED SIZE
            // (`Ui::settle_offset`); it never brings it back to the cursor.
            ui.settle_offset(rows.len());
        }
        // THE LINES WITHOUT THEIR HOVER, which is drawn below by zone.
        let lit = canvas.lit;
        canvas.lit = &[];
        crate::door::list::body(ui, canvas, &rows);
        canvas.lit = lit;
        let first = ui.first_of_list();
        let f = ui.frame(first);
        let cell = f.cell();
        // THE DIVIDERS between the columns, the one before the keyboard's
        // column lit — the file manager's mark (`chrome::divider_lit`).
        if layout == Layout::Panes {
            let places = crate::places::places(ui.columns);
            for pane in [Pane::List, Pane::Detail] {
                if let Some(at) = places.divider_before(pane) {
                    let x = f32::from(ui.margin + at) * cell + cell / 2.0;
                    canvas.under.push(if self.open.view.focus() == pane {
                        crate::chrome::divider_lit(x, f.y_of(first), f.bottom())
                    } else {
                        crate::chrome::divider(x, f.y_of(first), f.bottom())
                    });
                }
            }
            // AND EACH COLUMN'S BAR, where it scrolls.
            for (_, track, thumb, _) in self.bars(ui) {
                canvas.bar(Some(crate::bars::Track::of(track)));
                if let Some((x, y, width, height)) = thumb {
                    canvas.under.push(ronoctua_render::quad::Quad {
                        x,
                        y,
                        width,
                        height,
                        color: tokens::color::RULE_STRONG,
                    });
                }
            }
        }
        // EVERY CURSOR'S HOVER, in its own colour, over the stretch of the
        // line the pointer is in: a column, a kind on the strip, a tab.
        let line_px = f.line();
        let room = ui.room_for_rows();
        for &(color, row) in lit {
            let Some(row) = row else {
                continue;
            };
            if row < ui.offset || row >= ui.offset + room {
                continue;
            }
            for (r, c) in self.pointers.values() {
                if *r != Some(row) {
                    continue;
                }
                let Some((from, to)) = c.and_then(|c| View::hover_span(&lines, row, c)) else {
                    continue;
                };
                let y = f.y_of(first + u16::try_from(row - ui.offset).unwrap_or(0));
                canvas.under.push(ronoctua_render::quad::Quad {
                    x: f32::from(ui.margin + from) * cell,
                    y,
                    width: f32::from(to.saturating_sub(from)) * cell,
                    height: line_px,
                    color,
                });
            }
        }
    }

    fn resized(&mut self, ui: &mut Ui) {
        self.settled(ui, true);
    }

    fn language_changed(&mut self, ui: &mut Ui) {
        self.settled(ui, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Scene {
        let r = crate::incus::tests::readings();
        let l = r.listed().cloned().unwrap();
        Scene {
            instances: l.instances,
            images: l.images,
            profiles: l.profiles,
            networks: l.networks,
            pools: l.pools,
            projects: l.projects,
            remotes: r.remotes_read().to_vec(),
            cli: Some("/nix/store/x-incus/bin/incus".to_owned()),
            ..Scene::default()
        }
    }

    fn a_window(scene: Scene, opening: Opening) -> (IncusWindow, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        (IncusWindow::detached(scene, opening), Ui::for_tests(texts))
    }

    fn drawn(w: &IncusWindow, ui: &Ui) -> String {
        w.rows(ui)
            .iter()
            .flatten()
            .map(|f| f.text.clone())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// WITHOUT INCUS the window opens and says why.
    #[test]
    fn without_incus_the_window_says_why() {
        let (mut w, mut ui) = a_window(
            Scene {
                refused: Some(ronoctua_incus::Why::NoIncus),
                ..Scene::default()
            },
            Opening::Instances,
        );
        w.take_now(&mut ui);
        let why = ui
            .texts
            .t(crate::incus_words::why_key(ronoctua_incus::Why::NoIncus));
        assert!(drawn(&w, &ui).contains(&why), "{}", drawn(&w, &ui));
    }

    /// FAILS IF: the window starts a text console or a shell anywhere but in
    /// our terminal, or puts the screen in one.
    #[test]
    fn a_console_starts_our_terminal_and_the_screen_starts_by_itself() {
        let (mut w, mut ui) = a_window(scene(), Opening::Instances);
        w.take_now(&mut ui);
        w.pressed(&mut ui, crate::incus::Press::Console);
        let me = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let launched = ui.launches().to_vec();
        assert_eq!(launched.len(), 1, "{launched:?}");
        let said = format!("{launched:?}");
        assert!(
            said.contains(&me) && said.contains("\"terminal\""),
            "{said}"
        );
        assert!(
            said.contains("\"console\"") && said.contains("\"web\""),
            "{said}"
        );
    }

    /// A TYPED QUESTION is asked on this window's own line, and a launch
    /// from a scene is kept, never queued on this machine.
    #[test]
    fn the_draft_is_typed_on_the_window_line_and_kept_from_the_queue() {
        let (mut w, mut ui) = a_window(scene(), Opening::New);
        w.take_now(&mut ui);
        assert!(w.open.view.drafting(), "`incus new` opens the draft");
        w.pressed(&mut ui, crate::incus::Press::Enter);
        assert_eq!(ui.asking.as_ref().map(|l| l.id), Some(crate::incus::NAME));
        w.answered(&mut ui, crate::incus::NAME, "box");
        w.pressed(&mut ui, crate::incus::Press::Down);
        w.pressed(&mut ui, crate::incus::Press::Enter);
        w.answered(&mut ui, crate::incus::IMAGE, "debian/13");
        // DOWN TO THE ROW THAT LAUNCHES.
        for _ in 0..20 {
            w.pressed(&mut ui, crate::incus::Press::Down);
        }
        w.pressed(&mut ui, crate::incus::Press::Enter);
        let (_, queued, _) = w.report();
        assert_eq!(queued.len(), 1, "{queued:?}");
        assert!(matches!(
            &queued[0],
            ronoctua_transfers::Work::Incus { job, .. } if matches!(**job, ronoctua_incus::Job::Launch { .. })
        ));
    }

    /// FAILS IF: `ronoctua-shell incus new` (`LEADER s x n` in a window) loses
    /// its draft when the window settles — a resize — before the threads'
    /// bell: those rounds take the listing, and the bell then brings nothing.
    #[test]
    fn the_draft_asked_for_opens_whichever_settling_takes_the_listing() {
        let (mut w, mut ui) = a_window(scene(), Opening::New);
        w.resized(&mut ui);
        w.take_now(&mut ui);
        assert!(w.open.view.drafting(), "a resize before the bell");
        let (mut w, mut ui) = a_window(scene(), Opening::Instances);
        w.resized(&mut ui);
        assert!(!w.open.view.drafting(), "only when it was asked for");
    }

    /// THE CONSENT ROW, pressed in a photograph, lets the hosts in the scene
    /// and nowhere else — and the catalogue is asked for then.
    #[test]
    fn the_consent_row_lets_the_scene_s_hosts_and_the_catalogue_follows() {
        let mut s = scene();
        s.catalogues.insert(
            "images".to_owned(),
            vec![ronoctua_incus::Offered {
                alias: "alpine/3.22".to_owned(),
                description: "Alpine".to_owned(),
                kind: ronoctua_incus::Kind::Container,
                fingerprint: "f00d".to_owned(),
                size_bytes: 3 << 20,
            }],
        );
        let (mut w, mut ui) = a_window(s, Opening::Instances);
        w.take_now(&mut ui);
        w.pressed(&mut ui, crate::incus::Press::NextKind);
        w.pressed(&mut ui, crate::incus::Press::NextKind);
        w.pressed(&mut ui, crate::incus::Press::Right);
        assert!(!drawn(&w, &ui).contains("alpine/3.22"));
        w.pressed(&mut ui, crate::incus::Press::Allow);
        assert!(drawn(&w, &ui).contains("alpine/3.22"), "{}", drawn(&w, &ui));
    }
}
