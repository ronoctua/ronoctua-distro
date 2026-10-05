//! The containers' view's SECOND door — a window of its own
//! (`LEADER s o w`, `ronoctua-shell containers`).
//!
//! # One implementation, two doors
//!
//! Everything the view knows is `crate::containers::View`; the footer draws
//! the same lines. What this holds is a window and its threads
//! (`crate::containers_feed::Feed`, alive exactly as long as the window), the
//! queue's list for the jobs the view gave, and the pointer's part in three
//! columns side by side: which column a cell is in, a wheel and a bar per
//! column, and a hover that lights the column under the pointer and never the
//! whole line (invariant 15).
//!
//! # What it does, and where
//!
//! An action (start, stop, remove, prune) on the feed's thread; a pull, a
//! build or compose on the queue (`Work::Container`); a shell inside a
//! container in the terminal (`ronoctua-shell terminal <podman> exec …`); the
//! firewall's rules are the footer's command (`security-firewall`), handed to
//! it down the control socket. A typed question — an image to pull, a folder
//! to build — is asked on this window's own line (`Ui::ask`).

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

use crate::containers::{Errand, Layout, Pane, Readings, View, layout_of};
use crate::containers_feed::Feed;
use crate::door::{Canvas, Content, Door, Hand, Ui};

/// The window's `app_id`.
pub const APP_ID: &str = "br.ronoctua.containers";

/// The glyph before the window's name, as text — the `Title`'s shape.
static GLYPH: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| tokens::icon::CONTAINERS.to_string());

/// Opens the containers' view in a window.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    crate::door::run(|w| {
        // ONE PING FOR EVERYTHING THE THREADS BRING.
        let (wake, woke) = channel::channel::<()>();
        w.handle
            .insert_source(woke, |event, (), door: &mut Door<ContainersWindow>| {
                if let channel::Event::Msg(()) = event {
                    door.content.woke(&mut door.ui);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the containers' channel: {e}"))?;
        // THE QUEUE'S LIST: a pull, a build or compose that ended changed
        // what there is.
        let (jobs_sender, jobs) = channel::channel::<Vec<ronoctua_transfers::Job>>();
        crate::transfers::listen(jobs_sender);
        w.handle
            .insert_source(jobs, |event, (), door: &mut Door<ContainersWindow>| {
                if let channel::Event::Msg(jobs) = event {
                    door.content.jobs_arrived(&mut door.ui, jobs);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the queue channel: {e}"))?;
        let sender = std::sync::Mutex::new(wake);
        let mut feed = Feed::new(ronoctua_containers::Podman::from_env(), move || {
            if let Ok(s) = sender.lock() {
                let _ = s.send(());
            }
        });
        feed.start();
        Ok(ContainersWindow::with(feed))
    })
}

/// How many times [`ContainersWindow::settled`] takes what arrived and
/// follows it again, at most.
const SETTLE_ROUNDS: usize = 4;

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
    /// The cell along the line under it: three columns share a line, and
    /// the line alone cannot say which is under the pointer.
    cell: Option<u16>,
    /// The column whose bar this pointer holds.
    dragging: Option<Pane>,
    /// Lines of wheel asked for and not yet spent over a column.
    owed: f64,
}

/// The window.
pub struct ContainersWindow {
    view: View,
    readings: Readings,
    feed: Feed,
    /// The cell the last press was on — what `clicked` reads.
    click_cell: Option<u16>,
    /// Every pointer's line and cell, by its number — for the hover bands.
    pointers: BTreeMap<u32, (Option<usize>, Option<u16>)>,
    hands: u32,
    /// The drawn line the keyboard's cursor is on, stacked — counted where
    /// the width is known ([`ContainersWindow::settled`]) and handed to the
    /// door by [`Content::cursor`], which follows it after a key or a click
    /// and never from a paint.
    cursor_at: Option<usize>,
    /// A registry's login waiting for its password (`containers::LOGIN`).
    login: Option<crate::containers::Login>,
}

impl ContainersWindow {
    fn with(feed: Feed) -> Self {
        // THE KEYS ITS SENTENCES TEACH, read from the leader's own trees: this
        // window is a process of its own, and asks the tables the footer walks.
        let queue_keys = crate::leader::default_of()
            .map(|(trees, _)| crate::containers_words::queue_keys(&trees))
            .unwrap_or_default();
        let readings = Readings {
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            queue_keys,
            ..Readings::default()
        };
        Self {
            view: View::fresh(),
            readings,
            feed,
            click_cell: None,
            pointers: BTreeMap::new(),
            hands: 0,
            cursor_at: None,
            login: None,
        }
    }

    /// A window with a feed that answers on the spot, for a photograph —
    /// its podman the one given, its firewall the one given, and every date
    /// in UTC.
    #[must_use]
    pub fn detached(
        podman: Option<ronoctua_containers::Podman>,
        firewall: crate::containers::Firewall,
    ) -> Self {
        let mut feed = Feed::detached(podman, firewall);
        feed.start();
        let mut me = Self::with(feed);
        me.readings.zone = ronoctua_files::when::Zone::default();
        me
    }

    /// Takes everything the feed brought — a photograph calls it after every
    /// step, as the loop does on the bell. `true` when something arrived.
    pub fn take_now(&mut self, ui: &mut Ui) -> bool {
        let arrived = self.feed.take();
        if arrived.is_empty() {
            return false;
        }
        // A LOG FOLLOWED AT ITS END stays at its end, if its end was in
        // sight: the person asked for the tail. Wheeled away from it, the
        // page stays where the wheel put it — data never moves it otherwise.
        let tail = self.tail_in_sight(ui);
        for a in arrived {
            if let Some(said) = self.readings.took(a, &ui.texts) {
                ui.say(said);
            }
        }
        self.settled(ui);
        if tail && let Some(at) = self.cursor_at {
            let rows = self.lines(ui).len();
            ui.follow(at, rows);
        }
        true
    }

    /// Whether a stacked window follows a log whose last line is on screen.
    fn tail_in_sight(&self, ui: &Ui) -> bool {
        Self::layout(ui) == Layout::Stacked
            && self.view.follows_log(&self.readings, Layout::Stacked)
            && self
                .cursor_at
                .is_some_and(|at| at >= ui.offset && at < ui.offset + ui.room_for_rows())
    }

    /// The streams alive now — the log followed, and whether the stats
    /// are: what a photograph reports, so "a child only while its tab is
    /// shown" is seen on the frame it is true of.
    #[must_use]
    pub fn following(&self) -> (Option<&str>, bool) {
        (self.feed.following_logs(), self.feed.following_stats())
    }

    fn layout(ui: &Ui) -> Layout {
        layout_of(ui.columns, false)
    }

    fn lines(&self, ui: &Ui) -> Vec<crate::containers::Line> {
        self.view.lines(
            &self.readings,
            ui.columns,
            Self::layout(ui),
            ui.room_for_rows(),
            &ui.texts,
        )
    }

    /// The threads brought something.
    fn woke(&mut self, ui: &mut Ui) {
        self.take_now(ui);
    }

    /// The queue's list: the containers' jobs, and a listing again when one
    /// of them ended.
    fn jobs_arrived(&mut self, ui: &mut Ui, jobs: Vec<ronoctua_transfers::Job>) {
        let mine: Vec<ronoctua_transfers::Job> = jobs
            .into_iter()
            .filter(|j| matches!(j.work, ronoctua_transfers::Work::Container { .. }))
            .collect();
        let running = |js: &[ronoctua_transfers::Job]| {
            js.iter()
                .filter(|j| {
                    matches!(
                        j.stage,
                        ronoctua_transfers::Stage::Running | ronoctua_transfers::Stage::Weighing
                    )
                })
                .count()
        };
        if running(&mine) < running(&self.readings.jobs) {
            self.feed.relist();
        }
        if mine != self.readings.jobs {
            self.readings.jobs = mine;
            ui.ask_paint();
        }
    }

    /// The cursors put back in step with what is drawn, the streams made
    /// what is shown, and a frame.
    fn settled(&mut self, ui: &mut Ui) {
        let layout = Self::layout(ui);
        // UNTIL NOTHING MORE ARRIVES: what arrived here was taken off the
        // bell's list, so the streams are made what it shows before leaving
        // — a listing taken now is a row whose inspect is asked now. A
        // detached feed answers on the spot; a live one rarely has anything
        // yet. Bounded, since every round asks at most one inspect.
        for _ in 0..SETTLE_ROUNDS {
            self.view.settle(
                &self.readings,
                ui.columns,
                layout,
                ui.room_for_rows(),
                &ui.texts,
            );
            let wants = self.view.wants(&self.readings, layout);
            self.feed.follow(&wants, &mut self.readings);
            let more = self.feed.take();
            if more.is_empty() {
                break;
            }
            for a in more {
                if let Some(said) = self.readings.took(a, &ui.texts) {
                    ui.say(said);
                }
            }
        }
        self.view.settle(
            &self.readings,
            ui.columns,
            layout,
            ui.room_for_rows(),
            &ui.texts,
        );
        // THE PANES SCROLL INSIDE THEMSELVES; stacked, the door's page
        // follows this line after a key or a click.
        self.cursor_at = match layout {
            Layout::Panes => None,
            Layout::Stacked => self.view.cursor_line(&self.lines(ui), &self.readings),
        };
        ui.ask_paint();
    }

    /// Does what the view asked for.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        match errand {
            Errand::Nothing => {}
            Errand::Close => {
                ui.quit = true;
                return;
            }
            Errand::Copy(text) => ui.say(crate::chrome::copied(&text, &ui.texts)),
            Errand::Act(action, name) => {
                let what = ui.texts.t(crate::containers_feed::action_key(&action));
                ui.say(ui.texts.t_with(
                    ronoctua_i18n::key::CONTAINERS_ECHO_ASKED,
                    &[("what", &what), ("name", &name)],
                ));
                self.feed.act(action, name);
            }
            Errand::Queue(job, label) => {
                let said = crate::containers_words::queued_said(
                    &job,
                    &label,
                    &self.readings.queue_keys,
                    &ui.texts,
                );
                let work = ronoctua_transfers::Work::Container { job, label };
                match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
                    Ok(_) => ui.say(said),
                    Err(e) => {
                        tracing::warn!(error = %e, "the transfer queue did not take the job");
                        ui.say(ui.texts.t(ronoctua_i18n::key::CONTAINERS_NO_QUEUE));
                    }
                }
            }
            Errand::Ask(id, initial) => ui.ask(crate::line::Line::new(
                id,
                crate::containers::secret(id),
                initial.as_deref(),
            )),
            // THE PASSWORD ON A SECRET LINE, the login kept until it comes.
            Errand::Password(login) => {
                self.login = Some(login);
                ui.ask(crate::line::Line::new(
                    crate::containers::LOGIN_PASSWORD,
                    true,
                    None,
                ));
            }
            Errand::Shell(id) => match (self.feed.exec_line(&id), std::env::current_exe()) {
                (Some((podman, args)), Ok(me)) => {
                    let mut words = vec!["terminal".to_owned(), podman];
                    words.extend(args);
                    ui.launch(&me.to_string_lossy(), &words, None);
                }
                _ => ui.say(crate::containers_words::refused(
                    ronoctua_containers::Refusal {
                        why: ronoctua_containers::Why::NoPodman,
                    },
                    &ui.texts,
                )),
            },
            // A REGISTRY COMMAND is the footer's: handed to it by name.
            Errand::Run(id) => ui.tell_the_footer(id, None),
            Errand::Say(text) => ui.say(text),
        }
        self.settled(ui);
    }

    /// The login's password arrived: to podman's stdin, off the loop, with
    /// the login it was asked for — or, with none waiting, a sentence.
    fn password_answered(&mut self, ui: &mut Ui, text: &str) {
        match self.login.take() {
            Some(login) if !text.is_empty() => {
                ui.say(ui.texts.t_with(
                    ronoctua_i18n::key::CONTAINERS_ECHO_LOGGING_IN,
                    &[("registry", &login.registry), ("user", &login.user)],
                ));
                self.feed
                    .login(login, ronoctua_consent::keys::Secret::new(text));
            }
            Some(_) => {}
            None => ui.say(ui.texts.t(ronoctua_i18n::key::CONTAINERS_REFUSED_NO_LOGIN)),
        }
    }

    /// What one key means.
    fn pressed(&mut self, ui: &mut Ui, press: crate::containers::Press) {
        ui.message = None;
        let errand = self.view.key(
            press,
            &self.readings,
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
        self.view
            .scrolls(&self.readings, ui.columns, room, &ui.texts)
            .into_iter()
            .filter(crate::containers::PaneScroll::scrolls)
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
            let bar_offset = |track: (f32, f32, f32, f32), scroll: ronoctua_design::Scroll| {
                scroll.offset_at(y - track.1, track.3, line)
            };
            match e.kind {
                PointerEventKind::Press { button, .. } if button == crate::door::LEFT_BUTTON => {
                    if let Some((pane, track, _, scroll)) = self.bars(ui).into_iter().find(|b| {
                        let (tx, ty, tw, th) = b.1;
                        x >= tx && x < tx + tw && y >= ty && y < ty + th
                    }) {
                        hand.extra.dragging = Some(pane);
                        self.view.scroll_to(pane, bar_offset(track, scroll));
                        took = true;
                    }
                }
                PointerEventKind::Motion { .. } => {
                    if let Some(pane) = hand.extra.dragging
                        && let Some((_, track, _, scroll)) =
                            self.bars(ui).into_iter().find(|b| b.0 == pane)
                    {
                        self.view.scroll_to(pane, bar_offset(track, scroll));
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
                    crate::surface::wheel_arrived(&mut hand.extra.owed, &vertical, &horizontal);
                    let by = crate::surface::wheel_lines(&mut hand.extra.owed);
                    let pane = crate::containers::places(ui.columns)
                        .pane_at(Self::cell_at(ui, e.position.0).unwrap_or(0));
                    if by != 0
                        && let Some(s) = self
                            .view
                            .scrolls(&self.readings, ui.columns, ui.room_for_rows(), &ui.texts)
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
                        self.view.scroll_to(pane, to);
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

impl Content for ContainersWindow {
    type Hand = Pointed;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::CONTAINERS_TITLE))
    }

    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) =
            crate::door::keys::key_of(event, mods).and_then(crate::containers::press_of_key)
        {
            self.pressed(ui, press);
        }
    }

    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        if id == crate::containers::LOGIN_PASSWORD {
            self.password_answered(ui, text);
            return;
        }
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let errand = crate::containers::answered(id, text, home.as_deref(), &ui.texts);
        self.apply(ui, errand);
    }

    /// A question let go: a login waiting for its password is let go too.
    fn unanswered(&mut self, ui: &mut Ui, id: &'static str) {
        let _ = ui;
        if id == crate::containers::LOGIN_PASSWORD {
            self.login = None;
        }
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.view.hint(&self.readings, Self::layout(ui), &ui.texts)
    }

    fn rows(&self, ui: &Ui) -> Vec<Vec<crate::files::Field>> {
        self.lines(ui).into_iter().map(|l| l.fields).collect()
    }

    fn cursor(&self) -> Option<usize> {
        // THE PANES SCROLL INSIDE THEMSELVES; stacked, the door follows the
        // cursor's line — after a key or a click, so a wheel or a dragged bar
        // moves the page and the next paint leaves it there.
        self.cursor_at
    }

    fn clicked(&mut self, ui: &mut Ui, row: usize) {
        let lines = self.lines(ui);
        let layout = Self::layout(ui);
        let again = self
            .view
            .clicked(&lines, row, self.click_cell, layout, &self.readings);
        if again {
            self.pressed(ui, crate::containers::Press::Enter);
        } else {
            self.settled(ui);
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
        let name = ui.texts.t(ronoctua_i18n::key::CONTAINERS_TITLE);
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
        let rows: Vec<Vec<crate::files::Field>> = lines.iter().map(|l| l.fields.clone()).collect();
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
            let places = crate::containers::places(ui.columns);
            for pane in [Pane::List, Pane::Detail] {
                if let Some(at) = places.divider_before(pane) {
                    let x = f32::from(ui.margin + at) * cell + cell / 2.0;
                    canvas.under.push(if self.view.focus() == pane {
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
        self.settled(ui);
    }

    fn language_changed(&mut self, ui: &mut Ui) {
        self.settled(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_window() -> (ContainersWindow, Ui) {
        let texts = ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB");
        let me = ContainersWindow::detached(None, crate::containers::Firewall::default());
        (me, Ui::for_tests(texts))
    }

    /// WITHOUT PODMAN the window opens and says why, and every key that would
    /// act says it cannot, rather than doing nothing.
    #[test]
    fn without_podman_the_window_says_why() {
        let (mut w, mut ui) = a_window();
        w.settled(&mut ui);
        let said: String = w
            .rows(&ui)
            .iter()
            .flatten()
            .map(|f| f.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        let why = ui.texts.t(crate::containers_words::why_key(
            ronoctua_containers::Why::NoPodman,
        ));
        assert!(said.contains(&why), "{said}");
    }

    /// A TYPED QUESTION is asked on this window's own line, and its answer
    /// goes where the footer's goes.
    #[test]
    fn a_pull_is_asked_on_the_window_line() {
        let (mut w, mut ui) = a_window();
        w.pressed(&mut ui, crate::containers::Press::Pull);
        assert_eq!(
            ui.asking.as_ref().map(|l| l.id),
            Some(crate::containers::PULL)
        );
    }

    /// NARROW, THE PAGE FOLLOWS THE CURSOR AFTER A KEY — and only then: a page
    /// the wheel or the bar moved stays where they put it when the window
    /// settles again, which is what a paint and an arrival do.
    #[test]
    fn a_narrow_window_follows_the_cursor_after_a_key_and_not_after_data() {
        let (mut w, mut ui) = a_window();
        // WHAT THE FEED BROUGHT FIRST (no podman here) is taken, and the
        // listing is this one.
        w.take_now(&mut ui);
        let containers: Vec<ronoctua_containers::Container> = (0..40)
            .map(|n| ronoctua_containers::Container {
                id: format!("{n:04}"),
                names: vec![format!("c{n}")],
                image: "localhost/tiny:1".to_owned(),
                state: ronoctua_containers::State::Exited,
                status: String::new(),
                created_s: 0,
                ports: Vec::new(),
                pod: None,
                compose: None,
                labels: std::collections::BTreeMap::new(),
            })
            .collect();
        w.readings.listing = Some(Ok(ronoctua_containers::Listing {
            containers,
            ..ronoctua_containers::Listing::default()
        }));
        w.settled(&mut ui);
        assert_eq!(ContainersWindow::layout(&ui), Layout::Stacked);
        // THIRTY KEYS DOWN, each followed as the door follows a key.
        for _ in 0..30 {
            w.pressed(&mut ui, crate::containers::Press::Down);
            let at = w
                .cursor()
                .expect("a stacked window says where its cursor is");
            let rows = w.rows(&ui).len();
            ui.follow(at, rows);
        }
        assert!(ui.offset > 0, "the page followed the keys");
        // THE WHEEL takes the page back to the top; the window settles (an
        // arrival, a resize) and the page stays.
        ui.offset = 0;
        w.settled(&mut ui);
        assert_eq!(ui.offset, 0, "settling never brings the page back");
        assert!(!w.tail_in_sight(&ui), "no log is followed here");
    }

    /// THE FIREWALL is the footer's command, handed to it by name.
    #[test]
    fn the_firewall_is_asked_of_the_footer() {
        let (mut w, mut ui) = a_window();
        w.apply(&mut ui, Errand::Run(crate::containers::FIREWALL));
        assert_eq!(ui.orders(), ["command security-firewall".to_owned()]);
    }
}
